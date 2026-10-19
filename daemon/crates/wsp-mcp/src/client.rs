// SPDX-License-Identifier: AGPL-3.0-only
//! One socket to the host, as the command line's dial holds it: the token rides in the first frame and never in the
//! address, then each request carries an id its reply comes back under; a frame that answers no request goes to every
//! listener open at the time, the events a turn or an exec pushes. The socket opening and the answer to the token
//! share one deadline, so a port that accepts and never answers fails in one line. When the socket goes, every
//! request still waiting fails with the words for how it went: the host letting it go as it stopped, or the host
//! gone. packages/host/src/verbs.ts `dialOnce` is the rule. A host somewhere else first proves the key this computer
//! pinned for it at `seal.open`, and every frame after that reply rides inside the seal both ends agreed, so the token
//! and everything the line asks for cross a road whose carrier reads nothing and writes nothing into it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{Map, Value};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;
use wsp_frames::{
    numbers, place_link_transcript, Base64Bytes, LinkEphemerals, LinkRole, PlaceEphemeral, PlaceNonce, PlacePublicKey, PlaceSignature,
};
use wsp_seal::Seal;

use crate::failure::Failure;
use crate::record::{self, fill};
use crate::release::{self, ReleaseHost};

type Waiting = HashMap<u64, oneshot::Sender<Result<String, Failure>>>;

/// The reply's own fields, read before its body is: whether it is a reply, and whether the host said yes.
#[derive(Deserialize)]
struct Head {
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    ok: Option<bool>,
    #[serde(default)]
    error: Option<Value>,
    #[serde(default)]
    kind: Option<Value>,
    #[serde(default)]
    errors: Option<Box<RawValue>>,
    #[serde(default)]
    warnings: Option<Box<RawValue>>,
    #[serde(default)]
    fix: Option<Value>,
    #[serde(default)]
    left: Option<Value>,
}

struct Shared {
    waiting: Mutex<Waiting>,
    /// Each open `Frames` by the number it was handed, taken out when it is dropped.
    listeners: Mutex<Vec<(u64, mpsc::UnboundedSender<String>)>>,
    heard: AtomicU64,
    /// Set once the host proved the pinned key: from then on every frame either way is sealed, and one in the clear
    /// or one that will not open ends the socket.
    seal: Mutex<Option<Seal>>,
    /// Empty while the socket is open; the close code the host sent, or none, once it is gone.
    gone: watch::Sender<Option<Option<u16>>>,
}

pub struct Client {
    send: mpsc::UnboundedSender<Message>,
    shared: Arc<Shared>,
    next: AtomicU64,
    subscribed: tokio::sync::OnceCell<Result<(), Failure>>,
    tasks: [JoinHandle<()>; 2],
}

/// The frames the host pushes from the moment this was opened, held until read: a reply that names what to follow
/// may land with the first of its frames right behind it. None once the socket is gone.
pub struct Frames {
    told: mpsc::UnboundedReceiver<String>,
    id: u64,
    shared: Arc<Shared>,
}

impl Frames {
    pub async fn next(&mut self) -> Option<String> {
        self.told.recv().await
    }

    /// The next frame already here, without waiting for one.
    pub fn try_next(&mut self) -> Option<String> {
        self.told.try_recv().ok()
    }
}

impl Drop for Frames {
    fn drop(&mut self) {
        self.shared.listeners.lock().unwrap().retain(|(id, _)| *id != self.id);
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// What a dial presents once any seal is open: the token this computer holds, or its device key for a host on the
/// account that has handed it none yet, or a pairing code the host issued, which a socket that presents no token
/// spends for a device token of its own.
pub enum Presents<'a> {
    Token(&'a str),
    Admit(&'a Admit),
    Redeem { code: &'a str, name: &'a str },
}

/// A computer the account admitted: the key that host was told to trust, and what its listing calls this computer.
pub struct Admit {
    pub name: String,
    pub public_key: String,
    pub private_key_pem: String,
}

/// The device token a host answered an admission with.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paired {
    pub device_id: String,
    pub device_token: String,
}

/// What a host names of itself on the answer that lets a socket in: its release, and the road it comes back by.
struct Released {
    version: Option<String>,
    road: Option<String>,
}

impl Released {
    fn of(answer: &Value) -> Released {
        let word = |key: &str| answer.get(key).and_then(Value::as_str).map(str::to_owned);
        Released { version: word("version"), road: word("road") }
    }
}

/// Why a dial ended, and whether it was the answer to the frame that carried the token or the admission: the one
/// refusal a second dial can do anything about, since a host that never proved the pinned key refuses the same way
/// however often it is asked.
pub struct Refused {
    pub failure: Failure,
    pub token: bool,
}

/// Where a dial goes and what it holds the host to: the socket's url, the address as the person reads it, the time
/// open and the first answer share, the fingerprint of the key the host must prove, and the alias a host on the
/// account is held under, whose refusal of the token means this computer was taken away.
pub struct Dial<'a> {
    pub url: &'a str,
    pub at: &'a str,
    pub window: Duration,
    pub pinned: Option<&'a str>,
    pub alias: Option<&'a str>,
    /// The host a release refusal names, where this line holds the host to its own release.
    pub release: Option<&'a ReleaseHost>,
}

/// The host's answer to seal.open: the key it proves, its nonce, its half of the agreement and its signature.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SealOpened {
    nonce: PlaceNonce,
    host_public_key: PlacePublicKey,
    signature: PlaceSignature,
    ephemeral: PlaceEphemeral,
}

/// Dials the host and presents what `presents` names, after the host proved the pinned key where one is pinned.
pub async fn dial(to: &Dial<'_>, presents: Presents<'_>) -> Result<(Client, Option<Paired>), Refused> {
    let words = record::words();
    let at = to.at;
    let not_proved = || Refused { failure: Failure::auth(fill(&words.pair_key, &[("url", at)])), token: false };
    let authed = async {
        let (ws, _) = tokio_tungstenite::connect_async(to.url).await.map_err(|e| Refused {
            failure: Failure::of_kind(fill(&words.no_answer, &[("where", at), ("why", &e.to_string())]), "unreachable"),
            token: false,
        })?;
        let client = Client::over(ws);
        // Before the token or the key: a host on this computer is reached over its own loopback and pins nothing.
        let expect = match to.pinned {
            Some(pinned) => Some(client.open_seal(pinned).await.ok_or_else(not_proved)?),
            None => None,
        };
        let admit = matches!(presents, Presents::Admit(_));
        let redeem = matches!(presents, Presents::Redeem { .. });
        let answered = match presents {
            Presents::Token(token) => {
                let mut params = Map::new();
                params.insert("token".to_owned(), Value::from(token));
                client.request::<Value>("auth", params).await.map(|answer| (None, Released::of(&answer)))
            }
            Presents::Admit(key) => {
                // The key is proved over this socket's own handshake, so the signature stands for this dial alone.
                let Some(expect) = expect else { return Err(not_proved()) };
                let signature =
                    wsp_seal::sign(&key.private_key_pem, &expect).map_err(|e| Refused { failure: Failure::new(e), token: false })?;
                let mut params = Map::new();
                params.insert("publicKey".to_owned(), Value::from(key.public_key.as_str()));
                params.insert("name".to_owned(), Value::from(key.name.as_str()));
                params.insert("signature".to_owned(), Value::from(Base64Bytes::<64>::from_bytes(&signature).as_str()));
                client.request::<Value>("device.auth", params).await.and_then(|answer| {
                    let released = Released::of(&answer);
                    serde_json::from_value::<Paired>(answer).map(|paired| (Some(paired), released)).map_err(|e| Failure::new(e.to_string()))
                })
            }
            Presents::Redeem { code, name } => {
                let mut params = Map::new();
                params.insert("code".to_owned(), Value::from(code));
                params.insert("name".to_owned(), Value::from(name));
                client.request::<Value>("pair.redeem", params).await.and_then(|answer| {
                    let released = Released::of(&answer);
                    serde_json::from_value::<Paired>(answer).map(|paired| (Some(paired), released)).map_err(|e| Failure::new(e.to_string()))
                })
            }
        };
        match answered {
            Ok((paired, released)) => {
                // A host that names no release is one from before the answer carried it, and is not judged.
                if let (Some(host), Some(theirs)) = (to.release, released.version.as_deref()) {
                    if let Some(failure) = release::refusal(theirs, released.road.as_deref(), host) {
                        return Err(Refused { failure, token: false });
                    }
                }
                Ok((client, paired))
            }
            // The host's one sentence for every code it will not spend is the whole answer: there is no token to pair again for.
            Err(refused) if redeem => Err(Refused { failure: Failure::auth(refused.message), token: true }),
            Err(refused) => Err(Refused { failure: client.token_refused(refused, to.alias, admit, at).await, token: true }),
        }
    };
    match tokio::time::timeout(to.window, authed).await {
        Ok(dialled) => dialled,
        Err(_) => Err(Refused {
            failure: Failure::of_kind(
                fill(&words.no_answer_within, &[("where", at), ("ms", &to.window.as_millis().to_string())]),
                "unreachable",
            ),
            token: false,
        }),
    }
}

impl Client {
    fn over<S>(ws: tokio_tungstenite::WebSocketStream<S>) -> Client
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (mut sink, mut stream) = ws.split();
        let (send, mut outgoing) = mpsc::unbounded_channel::<Message>();
        let shared = Arc::new(Shared {
            waiting: Mutex::new(HashMap::new()),
            listeners: Mutex::new(Vec::new()),
            heard: AtomicU64::new(0),
            seal: Mutex::new(None),
            gone: watch::channel(None).0,
        });
        let closer = send.clone();
        let writer = tokio::spawn(async move {
            while let Some(message) = outgoing.recv().await {
                if sink.send(message).await.is_err() {
                    break;
                }
            }
        });
        let reading = shared.clone();
        let reader = tokio::spawn(async move {
            let mut code = None;
            while let Some(read) = stream.next().await {
                let text = match read {
                    Ok(Message::Text(text)) if reading.seal.lock().unwrap().is_none() => Ok(text.to_string()),
                    Ok(Message::Text(_)) => Err(()),
                    Ok(Message::Binary(bytes)) => match reading.seal.lock().unwrap().as_mut() {
                        Some(seal) => seal.unseal(&bytes).ok_or(()),
                        None => Ok(String::from_utf8_lossy(&bytes).into_owned()),
                    },
                    Ok(Message::Close(frame)) => {
                        code = frame.map(|f| u16::from(f.code));
                        break;
                    }
                    Ok(_) => continue,
                    Err(_) => break,
                };
                match text {
                    Ok(text) => reading.take(&text),
                    // A frame in the clear after the seal began and one that will not open are the same thing:
                    // somebody carrying the bytes writing into the socket. It ends, and every waiting reply with it.
                    Err(()) => {
                        let refusal = CloseFrame { code: CloseCode::Protocol, reason: record::host().seal_refusal.clone().into() };
                        let _ = closer.send(Message::Close(Some(refusal)));
                        code = Some(u16::from(CloseCode::Protocol));
                        break;
                    }
                }
            }
            reading.went(code);
        });
        Client { send, shared, next: AtomicU64::new(1), subscribed: tokio::sync::OnceCell::new(), tasks: [writer, reader] }
    }

    /// Every frame that answers no request from now on.
    pub fn frames(&self) -> Frames {
        let (tell, told) = mpsc::unbounded_channel();
        let id = self.shared.heard.fetch_add(1, Ordering::Relaxed);
        let mut listeners = self.shared.listeners.lock().unwrap();
        if self.shared.gone.borrow().is_none() {
            listeners.push((id, tell));
        }
        Frames { told, id, shared: self.shared.clone() }
    }

    /// The host's events on this socket, asked for once however many calls follow them.
    pub async fn events(&self) -> Result<(), Failure> {
        self.subscribed.get_or_init(|| async { self.request::<Value>("events.subscribe", Map::new()).await.map(|_| ()) }).await.clone()
    }

    /// Whether the host let this socket go as it stopped, which a wait dials through; read once the socket is gone.
    pub fn stopped_under(&self) -> bool {
        self.shared.gone.borrow().flatten() == Some(record::host().stopping_close)
    }

    /// Once the socket is gone, however it went.
    pub async fn closed(&self) {
        let mut gone = self.shared.gone.subscribe();
        let _ = gone.wait_for(Option::is_some).await;
    }

    /// One op and its reply, the reply's body read as `T`. A reply that is not ok is its own sentence, or the op's
    /// name where it carried none, with the kind the host stamped on it.
    pub async fn request<T: DeserializeOwned>(&self, op: &str, params: Map<String, Value>) -> Result<T, Failure> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (settle, settled) = oneshot::channel();
        self.shared.waiting.lock().unwrap().insert(id, settle);
        if self.shared.gone.borrow().is_some() {
            self.shared.waiting.lock().unwrap().remove(&id);
            return Err(Failure::new(self.close_words()));
        }
        let mut frame = params;
        frame.insert("id".to_owned(), Value::from(id));
        frame.insert("op".to_owned(), Value::from(op));
        let text = Value::Object(frame).to_string();
        {
            // Sealed and queued under one lock, so the frames reach the wire in the order their counters were taken.
            let mut seal = self.shared.seal.lock().unwrap();
            let message = match seal.as_mut() {
                Some(seal) => Message::Binary(seal.seal(&text).into()),
                None => Message::text(text),
            };
            let _ = self.send.send(message);
        }
        let text = settled.await.unwrap_or_else(|_| Err(Failure::new(self.close_words())))?;
        let head: Head = serde_json::from_str(&text).map_err(|e| Failure::new(e.to_string()))?;
        if head.ok != Some(true) {
            let said = head.error.as_ref().and_then(Value::as_str).map_or_else(|| format!("{op} failed"), str::to_owned);
            // Only a list is carried, as problemListsOf carries one.
            let list = |raw: Option<Box<RawValue>>| raw.filter(|r| r.get().starts_with('[')).map(|r| r.get().to_owned());
            let kind = head.kind.as_ref().and_then(Value::as_str).map(str::to_owned);
            // The host sends a list only beside a processes-left refusal (`refusalFields`).
            let left = head.left.filter(Value::is_array).map(Box::new);
            return Err(Failure {
                message: said,
                kind,
                errors: list(head.errors),
                warnings: list(head.warnings),
                fix: head.fix.as_ref().and_then(Value::as_str).map(Box::from),
                left,
            });
        }
        serde_json::from_str(&text).map_err(|e| Failure::new(format!("{op}: {e}")))
    }

    /// The first frame of a dial that pins a key: this computer's nonce and its half of a fresh key agreement,
    /// answered by the host with the key it proves. The fingerprint is read before the signature, since anything
    /// answering at this address signs for itself perfectly well; both stand before a token or a key has crossed.
    /// Answers the bytes the host expects this end to sign for an admission, or none where it proved nothing, which
    /// is one refusal whichever check caught it.
    async fn open_seal(&self, pinned: &str) -> Option<Vec<u8>> {
        let word = &record::host().seal_client;
        let (private, mine) = wsp_seal::fresh_ephemeral()?;
        let mine: PlaceEphemeral = Base64Bytes::from_bytes(&mine);
        let mut nonce = [0u8; numbers::PLACE_LINK_NONCE_BYTES];
        getrandom::fill(&mut nonce).ok()?;
        let nonce: PlaceNonce = Base64Bytes::from_bytes(&nonce);
        let mut params = Map::new();
        params.insert("nonce".to_owned(), Value::from(nonce.as_str()));
        params.insert("ephemeral".to_owned(), Value::from(mine.as_str()));
        let opened: SealOpened = self.request("seal.open", params).await.ok()?;
        let key = opened.host_public_key.to_bytes();
        if wsp_seal::fingerprint(&key) != pinned {
            return None;
        }
        let asked = LinkEphemerals { challenger: mine.as_str(), answerer: opened.ephemeral.as_str() };
        let host_bytes = place_link_transcript(LinkRole::Host, word, nonce.as_str(), opened.nonce.as_str(), asked);
        if !wsp_seal::verify(&key, &host_bytes, &opened.signature.to_bytes()) {
            return None;
        }
        let secret = wsp_seal::agree(private, &opened.ephemeral.to_bytes())?;
        *self.shared.seal.lock().unwrap() = Some(Seal::place(&wsp_seal::seal_keys(&secret, word)));
        let answered = LinkEphemerals { challenger: opened.ephemeral.as_str(), answerer: mine.as_str() };
        Some(place_link_transcript(LinkRole::Place, word, opened.nonce.as_str(), nonce.as_str(), answered))
    }

    pub fn is_closed(&self) -> bool {
        self.shared.gone.borrow().is_some()
    }

    /// Why the socket is gone, in the words the person reads: a host that let it go as it stopped says the turn goes
    /// on, since the run is the machine's; anything else is a host that went.
    pub fn close_words(&self) -> String {
        close_words(self.shared.gone.borrow().flatten())
    }

    /// What a refusal of the token means to the person. One whose frame carries no kind is classed by the close code
    /// that follows it, so an older host that sends the code alone still reads as auth.
    async fn token_refused(&self, refused: Failure, alias: Option<&str>, admit: bool, at: &str) -> Failure {
        let host = record::host();
        match refused.kind.as_deref() {
            None => {
                let mut gone = self.shared.gone.subscribe();
                let _ = tokio::time::timeout(Duration::from_millis(host.close_grace_ms), gone.wait_for(Option::is_some)).await;
                if self.shared.gone.borrow().flatten() != Some(host.unauthorized_close) {
                    return refused;
                }
                // An admission refused with no kind behind an unauthorized close: that door does not know the frame.
                if admit {
                    return Failure::auth(fill(&record::words().device_auth_old_host, &[("where", at)]));
                }
            }
            Some("auth") => {}
            Some(_) => return refused,
        }
        // A host that refused an admission said why in its own sentence, and there is no token to pair again for.
        if admit {
            return Failure::auth(refused.message);
        }
        match alias {
            Some(alias) => Failure::auth(fill(&record::words().device_refused, &[("alias", alias)])),
            None => Failure::auth(refused.message),
        }
    }
}

fn close_words(code: Option<u16>) -> String {
    let words = record::words();
    if code == Some(record::host().stopping_close) {
        words.host_stopping.clone()
    } else {
        words.host_closed.clone()
    }
}

/// JavaScript escapes half an emoji as a lone surrogate, which serde refuses with the whole frame, so each becomes U+FFFD.
fn lossy(text: &str) -> std::borrow::Cow<'_, str> {
    let surrogate = |at: usize| {
        let hex = text.get(at + 2..at + 6).filter(|_| text[at..].starts_with("\\u"))?;
        u16::from_str_radix(hex, 16).ok().filter(|unit| (0xD800..0xE000).contains(unit))
    };
    let bytes = text.as_bytes();
    let (mut out, mut kept, mut at) = (String::new(), 0, 0);
    while at < bytes.len() {
        if bytes[at] != b'\\' {
            at += 1;
            continue;
        }
        match surrogate(at) {
            Some(0xD800..0xDC00) if surrogate(at + 6).is_some_and(|low| low >= 0xDC00) => at += 12,
            Some(_) => {
                out.push_str(&text[kept..at]);
                out.push_str("\\uFFFD");
                at += 6;
                kept = at;
            }
            None => at += 2,
        }
    }
    if kept == 0 {
        return text.into();
    }
    out.push_str(&text[kept..]);
    out.into()
}

impl Shared {
    fn take(&self, text: &str) {
        let text = &*lossy(text);
        let id = serde_json::from_str::<Head>(text).ok().and_then(|head| head.id).and_then(|id| id.as_u64());
        if let Some(waiter) = id.and_then(|id| self.waiting.lock().unwrap().remove(&id)) {
            let _ = waiter.send(Ok(text.to_owned()));
            return;
        }
        for (_, listener) in self.listeners.lock().unwrap().iter() {
            let _ = listener.send(text.to_owned());
        }
    }

    fn went(&self, code: Option<u16>) {
        {
            let mut listeners = self.listeners.lock().unwrap();
            self.gone.send_replace(Some(code));
            listeners.clear();
        }
        let words = close_words(code);
        for (_, waiter) in self.waiting.lock().unwrap().drain() {
            let _ = waiter.send(Err(Failure::new(words.clone())));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A delete refused over processes still running, as the host frames it: the sentence with its fix joined on, the
    /// fix again on its own, and the threads with what each still runs. No host here runs a thread's processes
    /// without a wsp-daemon, so the frame is the host's shape, served by hand.
    #[tokio::test]
    async fn a_refusal_keeps_its_fix_and_the_processes_it_names() {
        let (near, far) = tokio::io::duplex(1 << 16);
        let fix = "delete it again with processes stop to end them first, or processes keep to leave them running (--processes on the command line).";
        let left = serde_json::json!([{ "threadId": "t1", "title": "dev", "workspaceId": "w1", "memory": 1024, "cpu": 0.5,
            "processes": [{ "pid": 41, "name": "vite", "kind": "server", "ports": [5173] }] }]);
        let said = "the thread to delete still runs a process: 41 vite (server, :5173) of thread t1:";
        let frame =
            serde_json::json!({ "ok": false, "error": format!("{said} {fix}"), "kind": "processes-left", "fix": fix, "left": left });
        let serving = tokio::spawn(async move {
            let mut ws = tokio_tungstenite::accept_async(far).await.unwrap();
            let Some(Ok(Message::Text(asked))) = ws.next().await else { panic!("no request") };
            let mut reply = frame;
            reply["id"] = serde_json::from_str::<Value>(&asked).unwrap()["id"].clone();
            ws.send(Message::text(reply.to_string())).await.unwrap();
            ws
        });
        let (ws, _) = tokio_tungstenite::client_async("ws://host/ws", near).await.unwrap();
        let client = Client::over(ws);
        let refused = client.request::<Value>("sessions.delete", Map::new()).await.unwrap_err();
        let _server = serving.await.unwrap();
        assert_eq!((refused.message.as_str(), refused.said()), (format!("{said} {fix}").as_str(), said), "the message stays whole");
        assert_eq!((refused.kind.as_deref(), refused.fix.as_deref()), (Some("processes-left"), Some(fix)));
        assert_eq!(refused.left, Some(Box::new(left)));
    }

    #[tokio::test]
    async fn a_listener_goes_when_its_frames_is_dropped() {
        let (near, far) = tokio::io::duplex(1024);
        let serving = tokio::spawn(async move { tokio_tungstenite::accept_async(far).await.unwrap() });
        let (ws, _) = tokio_tungstenite::client_async("ws://host/ws", near).await.unwrap();
        let _server = serving.await.unwrap();
        let client = Client::over(ws);
        let (kept, dropped) = (client.frames(), client.frames());
        assert_eq!(client.shared.listeners.lock().unwrap().len(), 2);
        drop(dropped);
        let left: Vec<u64> = client.shared.listeners.lock().unwrap().iter().map(|(id, _)| *id).collect();
        assert_eq!(left, [kept.id]);
    }

    #[tokio::test]
    async fn half_an_emoji_blanks_only_its_own_answer() {
        let (near, far) = tokio::io::duplex(4096);
        let serving = tokio::spawn(async move {
            let mut server = tokio_tungstenite::accept_async(far).await.unwrap();
            let asked = server.next().await.unwrap().unwrap();
            let id = serde_json::from_str::<Value>(asked.to_text().unwrap()).unwrap()["id"].clone();
            let reply = format!(r#"{{"id":{id},"ok":true,"values":["a\ud83d","b","\\ud83d","\ud83d\ude00","\ude00c"]}}"#);
            server.send(Message::text(reply)).await.unwrap();
            server
        });
        let (ws, _) = tokio_tungstenite::client_async("ws://host/ws", near).await.unwrap();
        let client = Client::over(ws);
        let asked = tokio::time::timeout(Duration::from_secs(5), client.request::<Value>("slates.resolve", Map::new()));
        let reply = asked.await.expect("the reply never reached its request").unwrap();
        let _server = serving.await.unwrap();
        assert_eq!(reply["values"], serde_json::json!(["a\u{FFFD}", "b", "\\ud83d", "\u{1F600}", "\u{FFFD}c"]));
    }
}
