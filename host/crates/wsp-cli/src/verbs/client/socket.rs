// SPDX-License-Identifier: AGPL-3.0-only
//! The socket to the host (`packages/host/src/verbs/client.ts:172-452`): one websocket per dial, the seal a host that
//! holds a pinned key and this computer agree, requests and replies by id, and the waits that dial again. Spec:
//! `spec/client-dial.md`. The types it answers with live in the parent module.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures::{SinkExt, StreamExt};
use rand_core::{OsRng, RngCore};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use wsp_store::keys::{
    fresh_ephemeral, key_fingerprint, make_seal, open_frame, seal_keys, shared_secret, sign_place_bytes, verify_place_bytes,
    Frame as WireFrame, Seal, Side, SEAL_REFUSAL,
};
use wsp_wire::types::daemon_contract::HOST_CLOSED_LINE;
use wsp_wire::wire::devices::{device_auth_old_host_line, pair_key_refusal};
use wsp_wire::wire::limits::HOST_STOPPING_CLOSE;
use wsp_wire::wire::place_link::{place_link_transcript, LinkEphemerals, LinkRole, SealOpenReply, PLACE_LINK_NONCE_BYTES, SEAL_CLIENT};
use wsp_words::app_ports::{authority, is_loopback, ws_url_of};
use wsp_words::exit::{auth_refusal, usage_refusal, HostError, Kind};
use wsp_words::words::thread::HOST_STOPPING_LINE;

use crate::account::read_device_key_pair;
use crate::host_lock::{dial_address, held_or_started, host_token_for, host_token_path, serving_host, POLL_MS, SERVICE_WAIT_MS};
use crate::hosts::{
    address_not_paired_line, aim_address, aim_holds, aim_name, aimed_host, device_refused_line, dial_window_ms, no_answer_refusal,
    no_answer_within, process_env, read_the_hosts, write_host, wsp_home, HostAim, HostPick, HostRecord,
};
use crate::json_whole::parse_frame;
use crate::verbs::client::{
    host_token_missing_line, no_host_serving_line, release_refusal, Admit, Again, BoxFuture, DialOpts, DialWithin, Frame, FrameOff,
    FrameSink, HostAddress, HostClient, Paired, ReleaseHost, VerbDeps, CLOSE_GRACE_MS, UNAUTHORIZED_CLOSE,
};
use crate::version::VERSION;

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// `hostAddress(statePath, pick)` (client.ts:158-167). A url aim: its ws url and its token or "". An alias: the
/// record's url and device token. Here: the lock's dial address and the token beside the state; no lock is a plain
/// error `no_host_serving_line` (provider), no token an auth refusal `host_token_missing_line(token path)`.
pub fn host_address(state_path: &str, pick: &HostPick, aim: Option<&HostAim>) -> Result<HostAddress, HostError> {
    let aim = match aim {
        Some(aim) => aim.clone(),
        None => aimed_host(state_path, pick)?,
    };
    match &aim {
        HostAim::Url { url, token, .. } => Ok(HostAddress { url: ws_url_of(url)?, token: token.clone().unwrap_or_default() }),
        HostAim::Alias { record, .. } => Ok(HostAddress { url: ws_url_of(&record.url)?, token: record.device_token.clone() }),
        HostAim::Here => {
            let lock = serving_host(state_path).ok_or_else(|| HostError::new(no_host_serving_line(state_path)))?;
            let token = host_token_for(state_path).ok_or_else(|| auth_refusal(&host_token_missing_line(&host_token_path(state_path))))?;
            let address = dial_address(lock.address.as_deref());
            let url = ws_url_of(&format!("http://{}", authority(&address, lock.port.0)))?;
            Ok(HostAddress { url, token })
        }
    }
}

/// One socket to the host, and for a host on the account the one re-admission it may need on the way: a record
/// whose token that host no longer takes is a computer the account still trusts, so this computer proves its device
/// key once, writes the token the host answers into the record and carries on.
pub async fn dial_host(state_path: &str, opts: DialOpts) -> Result<Arc<dyn HostClient>, HostError> {
    let aim = match &opts.aim {
        Some(aim) => aim.clone(),
        None => aimed_host(state_path, &opts.pick)?,
    };
    let account = match &aim {
        HostAim::Alias { alias, record } => Some((alias.clone(), record.clone())),
        _ => None,
    };
    // A record off the account's listing has no token yet, so its first dial admits this computer's key.
    let admit = match &account {
        Some((_, record)) if record.device_token.is_empty() => admitting(&opts.pick, record),
        _ => None,
    };
    let mut proved = admit.clone();
    let first = dial_once(state_path, DialOpts { aim: Some(aim.clone()), admit: admit.clone(), ..opts.clone() }).await;
    let client: Arc<dyn HostClient> = match first {
        Ok(client) => client as Arc<dyn HostClient>,
        Err(failure) => {
            if !failure.refused_token || failure.error.kind != Some(Kind::Auth) || admit.is_some() {
                return Err(failure.error);
            }
            proved = account.as_ref().and_then(|(_, record)| admitting(&opts.pick, record));
            let Some(again) = proved.clone() else { return Err(failure.error) };
            dial_once(state_path, DialOpts { aim: Some(aim.clone()), admit: Some(again), ..opts.clone() })
                .await
                .map(|client| client as Arc<dyn HostClient>)
                .map_err(|failure| failure.error)?
        }
    };
    if let (Some((alias, record)), Some(_), Some(paired)) = (&account, &proved, client.paired()) {
        let rewritten = HostRecord { device_id: paired.device_id, device_token: paired.device_token, ..record.clone() };
        write_host(&home_of(&opts.pick), alias, &rewritten)?;
    }
    Ok(client)
}

/// `stoppedUnder(client)` (client.ts:393-395): whether the socket is gone with close code 4001. Never waits.
pub fn stopped_under(client: &dyn HostClient) -> bool {
    client.gone() == Some(Some(HOST_STOPPING_CLOSE as u16))
}

/// `hostAgain(dial, windowMs)` (client.ts:404-414): dial with `max(1, left)` as each try's deadline; on failure,
/// if the window has passed, the last failure stands; else sleep `min(POLL_MS, max(0, left))` and try again.
pub async fn host_again(dial: DialWithin<'_>, window_ms: u64) -> Result<Arc<dyn HostClient>, HostError> {
    let until = tokio::time::Instant::now() + Duration::from_millis(window_ms);
    loop {
        let left = millis_until(until);
        match dial(left.max(1)).await {
            Ok(client) => return Ok(client),
            Err(e) if tokio::time::Instant::now() >= until => return Err(e),
            Err(_) => {}
        }
        tokio::time::sleep(Duration::from_millis(POLL_MS.min(millis_until(until)))).await;
    }
}

/// `hostBack(deps, withinMs)` (client.ts:418-420): `host_again` over `deps.client(Some(Again { within_ms }))`,
/// within `within_ms`, else `deps.host_wait_ms`, else `SERVICE_WAIT_MS`.
pub async fn host_back(deps: &VerbDeps, within_ms: Option<u64>) -> Result<Arc<dyn HostClient>, HostError> {
    let window = within_ms.or(deps.host_wait_ms).unwrap_or(SERVICE_WAIT_MS);
    let dial = |left: u64| dial_again(deps, left);
    host_again(&dial, window).await
}

/// `untilSettled(client, work)` (client.ts:423-430): the work's answer, or a plain error `close_words()` when the
/// socket goes first.
pub async fn until_settled<T: Send>(client: &dyn HostClient, work: BoxFuture<'_, Result<T, HostError>>) -> Result<T, HostError> {
    tokio::select! {
        biased;
        answer = work => answer,
        _ = client.closed() => Err(HostError::new(client.close_words())),
    }
}

/// `pushedFrames(client)` (client.ts:435-451): frames held from the moment it is made, replayed into `follow`'s sink
/// in arrival order, then passed straight through; `stop` removes the sink and drops whatever is still held.
pub struct PushedFrames {
    shared: Arc<Mutex<Held>>,
    off: Mutex<Option<FrameOff>>,
}

struct Held {
    frames: Vec<Frame>,
    sink: Option<FrameSink>,
}

impl PushedFrames {
    pub fn new(client: &dyn HostClient) -> PushedFrames {
        let shared = Arc::new(Mutex::new(Held { frames: Vec::new(), sink: None }));
        let held = shared.clone();
        let off = client.on_frame(Arc::new(move |frame: &Frame| {
            let sink = {
                let mut state = held.lock().unwrap();
                match &state.sink {
                    Some(sink) => Some(sink.clone()),
                    None => {
                        state.frames.push(frame.clone());
                        None
                    }
                }
            };
            if let Some(sink) = sink {
                sink(frame);
            }
        }));
        PushedFrames { shared, off: Mutex::new(Some(off)) }
    }

    /// Replays every held frame that `pick` takes into `on`, then sends each later one there too. A `stop` from
    /// inside `on` drops the held frames not yet replayed.
    pub fn follow(&self, pick: Arc<dyn Fn(&Frame) -> bool + Send + Sync>, on: FrameSink) {
        let sink: FrameSink = Arc::new(move |frame: &Frame| {
            if pick(frame) {
                on(frame);
            }
        });
        let held = {
            let mut state = self.shared.lock().unwrap();
            state.sink = Some(sink);
            std::mem::take(&mut state.frames)
        };
        for frame in held {
            let current = self.shared.lock().unwrap().sink.clone();
            if let Some(current) = current {
                current(&frame);
            }
        }
    }

    pub fn stop(&self) {
        if let Some(off) = self.off.lock().unwrap().take() {
            off.off();
        }
        let mut state = self.shared.lock().unwrap();
        state.sink = Some(Arc::new(|_: &Frame| {}));
        state.frames.clear();
    }
}

/// The concrete socket `dial_host` answers with (client.ts:207-389).
pub struct SocketClient {
    inner: Arc<Inner>,
    paired: Option<Paired>,
    subscribed: tokio::sync::Mutex<Option<Result<(), HostError>>>,
}

#[async_trait]
impl HostClient for SocketClient {
    async fn request(&self, op: &str, params: Frame) -> Result<Frame, HostError> {
        send_request(&self.inner, op, params).await
    }

    async fn events(&self) -> Result<(), HostError> {
        let mut subscribed = self.subscribed.lock().await;
        if subscribed.is_none() {
            *subscribed = Some(send_request(&self.inner, "events.subscribe", Frame::new()).await.map(|_| ()));
        }
        subscribed.clone().expect("the answer is set above")
    }

    fn on_frame(&self, sink: FrameSink) -> FrameOff {
        let id = {
            let mut link = self.inner.link.lock().unwrap();
            let id = link.listener_ids + 1;
            link.listener_ids = id;
            link.listeners.push((id, sink));
            id
        };
        let inner = self.inner.clone();
        FrameOff(Box::new(move || inner.link.lock().unwrap().listeners.retain(|(n, _)| *n != id)))
    }

    async fn closed(&self) -> Option<u16> {
        let mut gone = self.inner.gone.subscribe();
        let code = match gone.wait_for(Option::is_some).await {
            Ok(state) => (*state).flatten(),
            Err(_) => None,
        };
        code
    }

    fn gone(&self) -> Option<Option<u16>> {
        *self.inner.gone.borrow()
    }

    fn close_words(&self) -> String {
        closed_words(&self.inner).to_owned()
    }

    fn paired(&self) -> Option<Paired> {
        self.paired.clone()
    }

    fn close(&self) {
        let _ = self.inner.out.send(Command::Close);
    }

    fn terminate(&self) {
        let _ = self.inner.out.send(Command::Terminate);
    }
}

struct Inner {
    link: Mutex<Link>,
    gone: watch::Sender<Option<Option<u16>>>,
    next: AtomicU64,
    out: mpsc::UnboundedSender<Command>,
}

struct Link {
    pending: HashMap<u64, oneshot::Sender<Result<Frame, HostError>>>,
    listeners: Vec<(u64, FrameSink)>,
    listener_ids: u64,
    seal: Option<Seal>,
}

enum Command {
    Send(Message),
    Close,
    Terminate,
}

impl Inner {
    fn new(out: mpsc::UnboundedSender<Command>) -> Inner {
        let (gone, _) = watch::channel(None);
        Inner {
            link: Mutex::new(Link { pending: HashMap::new(), listeners: Vec::new(), listener_ids: 0, seal: None }),
            gone,
            next: AtomicU64::new(1),
            out,
        }
    }
}

fn refusal(op: &str, frame: &Frame) -> HostError {
    let message = frame.get("error").and_then(Value::as_str).map_or_else(|| format!("{op} failed"), str::to_owned);
    let mut e = HostError::new(message);
    e.kind = frame.get("kind").and_then(Value::as_str).map(Kind::parse);
    e.errors = frame.get("errors").and_then(Value::as_array).cloned();
    e.warnings = frame.get("warnings").and_then(Value::as_array).cloned();
    e
}

fn close_words_of(code: Option<u16>) -> &'static str {
    if code == Some(HOST_STOPPING_CLOSE as u16) {
        HOST_STOPPING_LINE
    } else {
        HOST_CLOSED_LINE
    }
}

fn closed_words(inner: &Inner) -> &'static str {
    close_words_of((*inner.gone.borrow()).flatten())
}

async fn send_request(inner: &Inner, op: &str, params: Frame) -> Result<Frame, HostError> {
    let id = inner.next.fetch_add(1, Ordering::SeqCst);
    let mut body = params;
    body.insert("id".to_owned(), json!(id));
    body.insert("op".to_owned(), json!(op));
    let text = crate::json_whole::stringify(&Value::Object(body));
    let (tx, rx) = oneshot::channel();
    {
        let mut link = inner.link.lock().unwrap();
        if let Some(code) = *inner.gone.borrow() {
            return Err(HostError::new(close_words_of(code)));
        }
        let message = match link.seal.as_mut() {
            Some(seal) => Message::Binary(seal.seal(&text).into()),
            None => Message::text(text),
        };
        link.pending.insert(id, tx);
        if inner.out.send(Command::Send(message)).is_err() {
            link.pending.remove(&id);
            return Err(HostError::new(closed_words(inner)));
        }
    }
    let answer = match rx.await {
        Ok(answer) => answer?,
        Err(_) => return Err(HostError::new(closed_words(inner))),
    };
    if answer.get("ok") != Some(&Value::Bool(true)) {
        return Err(refusal(op, &answer));
    }
    Ok(answer)
}

async fn run(mut ws: Ws, mut commands: mpsc::UnboundedReceiver<Command>, inner: Arc<Inner>) {
    let code = loop {
        tokio::select! {
            incoming = ws.next() => match incoming {
                Some(Ok(Message::Text(text))) => match receive(&inner, WireFrame::Text(text.as_str())) {
                    Ok(frame) => dispatch(&inner, frame),
                    Err(true) => {
                        let _ = ws.close(Some(seal_refusal_frame())).await;
                    }
                    Err(false) => {}
                },
                Some(Ok(Message::Binary(bytes))) => match receive(&inner, WireFrame::Binary(&bytes)) {
                    Ok(frame) => dispatch(&inner, frame),
                    Err(true) => {
                        let _ = ws.close(Some(seal_refusal_frame())).await;
                    }
                    Err(false) => {}
                },
                Some(Ok(Message::Close(frame))) => break frame.map(|f| u16::from(f.code)),
                Some(Ok(_)) => {}
                Some(Err(_)) | None => break None,
            },
            command = commands.recv() => match command {
                Some(Command::Send(message)) => {
                    if ws.send(message).await.is_err() {
                        break None;
                    }
                }
                Some(Command::Close) => {
                    let _ = ws.close(None).await;
                }
                Some(Command::Terminate) | None => break None,
            },
        }
    };
    finish(&inner, code);
}

fn seal_refusal_frame() -> CloseFrame {
    CloseFrame { code: CloseCode::from(1002), reason: SEAL_REFUSAL.into() }
}

/// `Err(true)`: the frame broke the seal, so the socket must end.
fn receive(inner: &Inner, raw: WireFrame<'_>) -> Result<Frame, bool> {
    let mut link = inner.link.lock().unwrap();
    let sealed = link.seal.is_some();
    let text = open_frame(link.seal.as_mut(), raw).map_err(|_| sealed)?;
    match parse_frame(&text) {
        Some(Value::Object(frame)) => Ok(frame),
        Some(_) => Err(false),
        None => Err(sealed),
    }
}

fn dispatch(inner: &Inner, frame: Frame) {
    let waiter = {
        let mut link = inner.link.lock().unwrap();
        match frame.get("id").and_then(whole_number).and_then(|id| link.pending.remove(&id)) {
            Some(waiter) => Ok(waiter),
            None => Err(link.listeners.iter().map(|(_, sink)| sink.clone()).collect::<Vec<FrameSink>>()),
        }
    };
    match waiter {
        Ok(waiter) => {
            let _ = waiter.send(Ok(frame));
        }
        Err(sinks) => {
            for sink in sinks {
                sink(&frame);
            }
        }
    }
}

fn whole_number(value: &Value) -> Option<u64> {
    let n = value.as_f64()?;
    (n.fract() == 0.0 && n >= 0.0).then_some(n as u64)
}

fn finish(inner: &Inner, code: Option<u16>) {
    let waiters: Vec<oneshot::Sender<Result<Frame, HostError>>> = {
        let mut link = inner.link.lock().unwrap();
        if inner.gone.borrow().is_some() {
            return;
        }
        inner.gone.send_replace(Some(code));
        link.pending.drain().map(|(_, waiter)| waiter).collect()
    };
    let words = close_words_of(code);
    for waiter in waiters {
        let _ = waiter.send(Err(HostError::new(words)));
    }
}

struct DialFailure {
    error: HostError,
    refused_token: bool,
}

impl From<HostError> for DialFailure {
    fn from(error: HostError) -> DialFailure {
        DialFailure { error, refused_token: false }
    }
}

async fn dial_once(state_path: &str, opts: DialOpts) -> Result<Arc<SocketClient>, DialFailure> {
    let aim = match &opts.aim {
        Some(aim) => aim.clone(),
        None => aimed_host(state_path, &opts.pick)?,
    };
    if aim == HostAim::Here {
        let fallback: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|line: &str| eprintln!("{line}"));
        let say = opts.say.clone().unwrap_or(fallback);
        held_or_started(state_path, opts.start.as_ref(), &*say).await?;
    }
    // A token is held only under a name, so an address without one opens nothing.
    if let HostAim::Url { url, token: None, .. } = &aim {
        if opts.admit.is_none() {
            return Err(usage_refusal(&address_not_paired_line(url), &read_the_hosts()).into());
        }
    }
    let address = host_address(state_path, &opts.pick, Some(&aim))?;
    let deadline_ms = opts.deadline_ms.unwrap_or_else(|| dial_window_ms(&aim));
    let deadline = Duration::from_millis(deadline_ms);
    let started = tokio::time::Instant::now();
    // The person reads the authority here, or the address as given: never the ws url the dial builds.
    let where_ = if aim == HostAim::Here { authority_of(&address.url) } else { aim_address(&aim).unwrap_or_default() };
    let (ws, _) = match tokio::time::timeout(deadline, tokio_tungstenite::connect_async(address.url.as_str())).await {
        Err(_) => return Err(no_answer_within(&where_, deadline_ms).into()),
        Ok(Err(e)) => return Err(no_answer_refusal(&where_, &e.to_string()).into()),
        Ok(Ok(pair)) => pair,
    };
    let (out, commands) = mpsc::unbounded_channel();
    let inner = Arc::new(Inner::new(out));
    tokio::spawn(run(ws, commands, inner.clone()));
    let refused = AtomicBool::new(false);
    let remaining = deadline.saturating_sub(started.elapsed());
    let shake = handshake(&inner, &opts, &aim, state_path, &where_, &address.token, &refused);
    match tokio::time::timeout(remaining, shake).await {
        Ok(Ok(paired)) => Ok(Arc::new(SocketClient { inner, paired, subscribed: tokio::sync::Mutex::new(None) })),
        Ok(Err(error)) => {
            let _ = inner.out.send(Command::Terminate);
            Err(DialFailure { error, refused_token: refused.load(Ordering::SeqCst) })
        }
        Err(_) => {
            let _ = inner.out.send(Command::Terminate);
            Err(no_answer_within(&where_, deadline_ms).into())
        }
    }
}

async fn handshake(
    inner: &Arc<Inner>,
    opts: &DialOpts,
    aim: &HostAim,
    state_path: &str,
    where_: &str,
    token: &str,
    refused: &AtomicBool,
) -> Result<Option<Paired>, HostError> {
    let pinned = match &opts.admit {
        Some(admit) => Some(admit.host_key.clone()),
        None if *aim == HostAim::Here => None,
        None => aim_holds(aim).host_key,
    };
    let expect = match pinned {
        Some(host_key) => Some(open_seal(inner, &host_key, where_).await?),
        None => None,
    };
    if let Some(admit) = &opts.admit {
        let Some(expect) = expect else { return Err(auth_refusal(&pair_key_refusal(where_))) };
        let signature = sign_place_bytes(&admit.key.private_key_pem, &expect).map_err(|e| HostError::new(e.0))?;
        let params = frame_of(json!({ "publicKey": admit.key.public_key, "name": admit.name, "signature": signature }));
        let admitted = match send_request(inner, "device.auth", params).await {
            Ok(frame) => frame,
            Err(e) => return Err(token_refused(inner, e, opts, aim, where_, refused).await),
        };
        same_release(&admitted, opts, aim, state_path)?;
        return Ok(Some(Paired { device_id: text_field(&admitted, "deviceId"), device_token: text_field(&admitted, "deviceToken") }));
    }
    let answer = match send_request(inner, "auth", frame_of(json!({ "token": token }))).await {
        Ok(frame) => frame,
        Err(e) => return Err(token_refused(inner, e, opts, aim, where_, refused).await),
    };
    same_release(&answer, opts, aim, state_path)?;
    Ok(None)
}

async fn open_seal(inner: &Arc<Inner>, host_key: &str, where_: &str) -> Result<Vec<u8>, HostError> {
    let refusal = || auth_refusal(&pair_key_refusal(where_));
    let (mine, mine_public) = fresh_ephemeral();
    let mut nonce_bytes = [0u8; PLACE_LINK_NONCE_BYTES as usize];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = STANDARD.encode(nonce_bytes);
    let params = frame_of(json!({ "nonce": nonce, "ephemeral": mine_public }));
    let answer: SealOpenReply = send_request(inner, "seal.open", params)
        .await
        .ok()
        .and_then(|frame| serde_json::from_value(Value::Object(frame)).ok())
        .ok_or_else(refusal)?;
    let host_nonce = answer.nonce.as_str().to_owned();
    let host_public = answer.host_public_key.as_str();
    let host_ephemeral = answer.ephemeral.as_str();
    if key_fingerprint(host_public) != host_key {
        return Err(refusal());
    }
    let transcript = place_link_transcript(
        LinkRole::Host,
        SEAL_CLIENT,
        &nonce,
        &host_nonce,
        LinkEphemerals { challenger: &mine_public, answerer: host_ephemeral },
    );
    if !verify_place_bytes(host_public, &transcript, answer.signature.as_str()) {
        return Err(refusal());
    }
    let secret = shared_secret(&mine, host_ephemeral).map_err(|e| HostError::new(e.0))?;
    inner.link.lock().unwrap().seal = Some(make_seal(&seal_keys(&secret, SEAL_CLIENT), Side::Place));
    Ok(place_link_transcript(
        LinkRole::Place,
        SEAL_CLIENT,
        &host_nonce,
        &nonce,
        LinkEphemerals { challenger: host_ephemeral, answerer: &mine_public },
    ))
}

async fn token_refused(inner: &Arc<Inner>, e: HostError, opts: &DialOpts, aim: &HostAim, where_: &str, refused: &AtomicBool) -> HostError {
    refused.store(true, Ordering::SeqCst);
    match e.kind.as_ref().map(Kind::as_str) {
        None => {
            let mut gone = inner.gone.subscribe();
            let _ = tokio::time::timeout(Duration::from_millis(CLOSE_GRACE_MS), gone.wait_for(Option::is_some)).await;
            let code = (*inner.gone.borrow()).flatten();
            if code != Some(UNAUTHORIZED_CLOSE) {
                return e;
            }
            if opts.admit.is_some() {
                return auth_refusal(&device_auth_old_host_line(where_));
            }
        }
        Some("auth") => {}
        Some(_) => return e,
    }
    if opts.admit.is_some() {
        return auth_refusal(&e.message);
    }
    match aim {
        HostAim::Alias { alias, .. } => auth_refusal(&device_refused_line(alias)),
        _ => auth_refusal(&e.message),
    }
}

fn same_release(answer: &Frame, opts: &DialOpts, aim: &HostAim, state_path: &str) -> Result<(), HostError> {
    let Some(theirs) = answer.get("version").and_then(Value::as_str) else { return Ok(()) };
    if opts.any_release || theirs == VERSION {
        return Ok(());
    }
    let road = answer.get("road").and_then(Value::as_str);
    let host = match aim {
        HostAim::Here => ReleaseHost::State(state_path.to_owned()),
        _ => {
            let address = aim_address(aim).unwrap_or_default();
            ReleaseHost::Where { r#where: aim_name(aim), here: is_loopback(&url_hostname(&address)) }
        }
    };
    let argv: Vec<String> = std::env::args_os().map(|arg| arg.to_string_lossy().into_owned()).collect();
    Err(release_refusal(VERSION, theirs, &host, road, &argv))
}

fn admitting(pick: &HostPick, record: &HostRecord) -> Option<Admit> {
    let host_key = record.host_key.clone()?;
    let key = read_device_key_pair(&home_of(pick))?;
    Some(Admit { name: hostname(), host_key, key })
}

fn home_of(pick: &HostPick) -> String {
    match &pick.home {
        Some(home) => home.clone(),
        None => wsp_home(&pick.env.clone().unwrap_or_else(process_env)),
    }
}

fn hostname() -> String {
    let mut buffer = [0u8; 256];
    // SAFETY: `buffer` is 256 bytes long, and gethostname writes a NUL-terminated name into it.
    unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) };
    let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
    String::from_utf8_lossy(&buffer[..end]).into_owned()
}

fn authority_of(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed) => {
            let host = parsed.host_str().unwrap_or_default();
            match parsed.port() {
                Some(port) => format!("{host}:{port}"),
                None => host.to_owned(),
            }
        }
        Err(_) => String::new(),
    }
}

fn url_hostname(address: &str) -> String {
    url::Url::parse(address).ok().and_then(|parsed| parsed.host_str().map(str::to_owned)).unwrap_or_default()
}

fn frame_of(value: Value) -> Frame {
    match value {
        Value::Object(frame) => frame,
        _ => Frame::new(),
    }
}

fn text_field(frame: &Frame, key: &str) -> String {
    frame.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

fn millis_until(until: tokio::time::Instant) -> u64 {
    until.saturating_duration_since(tokio::time::Instant::now()).as_millis() as u64
}

fn dial_again<'a>(deps: &'a VerbDeps, left: u64) -> BoxFuture<'a, Result<Arc<dyn HostClient>, HostError>> {
    Box::pin(async move { deps.client(Some(Again { within_ms: left })).await })
}
