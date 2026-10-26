// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/reach.ts at 65f6b89c7. Spec: spec/reach.md.
//!
//! Resilient client for the in-VM daemon over a previewUrl. The edge idle-sweeps quiet connections (~30s), so
//! liveness is an app-level ping op; every (re)connect re-subscribes and rescans the inbox because events pushed
//! during a gap are gone for good.
use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::rc::Rc;

use async_trait::async_trait;
use futures::sink::Sink;
use futures::stream::LocalBoxStream;
use futures::{SinkExt, StreamExt};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::WebSocketStream;
use wsp_store::keys::{open_frame, Frame as RawFrame, Seal, SEAL_REFUSAL};
use wsp_wire::wire::daemon::DaemonLinkStatus;
use wsp_wire::wire::daemon_events::{parse_daemon_event, DaemonEvent};
use wsp_wire::wire::limits::link_backoff_ms;
use wsp_wire::wire::machine_link::MachineErrorKind;
use wsp_words::daemon_contract::daemon_refused_token;

use crate::clock::{real_clock, Cancel, Clock};
use crate::daemon_channel::{js_string, reply_id, socket_config, ChannelEnd, CLOSE_GRACE_MS};
use crate::ids::{Ids, OsIds};

/// A daemon frame or reply: a JSON object, keys in the order written.
pub type Frame = Map<String, Value>;

/// A socket somebody else opened and authenticated (a place's link), handed over whole.
pub struct TakenSocket {
    pub sink: std::pin::Pin<Box<dyn Sink<Message, Error = WsError>>>,
    pub stream: LocalBoxStream<'static, Result<Message, WsError>>,
}

impl TakenSocket {
    pub fn new<S>(ws: WebSocketStream<S>) -> Self
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + 'static,
    {
        let (sink, stream) = futures::StreamExt::split(ws);
        TakenSocket { sink: Box::pin(sink), stream: Box::pin(stream) }
    }
}

// export interface ReachOptions {
//   /** Solari previewUrl (https, pt_token already embedded) or a ws:// url in tests. One of this and socket. */
//   previewUrl?: string;
//   /** A socket that is already open and already authenticated by the road that opened it: the link a place dialled
//    * out on. Nothing redials it, since only that computer can open one, and a close is the end of this reach. */
//   socket?: WebSocket;
//   /** The daemon's own token, sent as the first frame on every dial; never in the URL. Not read on a socket, whose
//    * handshake was the auth. */
//   token?: string;
//   onEvent: (e: DaemonEvent) => void;
//   /** Fires on every transition; reauth-needed and dead are terminal. */
//   onStatus?: (s: DaemonLinkStatus) => void;
//   heartbeatMs?: number;
//   /** The seal a place link agreed in its handshake: every frame this reach sends goes out inside it and every
//    * frame it reads is opened with it, so whoever carries the bytes reads nothing and writes nothing. Absent on
//    * every road that is not a place link, which is dialled with a token over the provider's own route. */
//   seal?: Seal;
//   /** The wait before each re-dial; the link rule's unless a test pins one. A dial that keeps failing is made again
//    * for as long as this link is held, since only the daemon's own word ends anything running behind it. */
//   backoffMs?: (attempt: number) => number;
// }
pub struct ReachOptions {
    /// A previewUrl (https, pt_token embedded) or a ws:// url in tests. One of this and `socket`.
    pub preview_url: Option<String>,
    /// A socket already open and already authenticated: nothing redials it, and its close ends this reach.
    pub socket: Option<TakenSocket>,
    /// The daemon's own token, sent as the first frame on every dial; never in the URL; not read on a socket.
    pub token: Option<String>,
    pub on_event: Rc<dyn Fn(DaemonEvent)>,
    /// Fires on every transition; reauth-needed and dead are terminal.
    pub on_status: Option<Rc<dyn Fn(DaemonLinkStatus)>>,
    pub heartbeat_ms: Option<i64>,
    /// The seal a place link agreed: every frame out sealed, every frame in opened.
    pub seal: Option<Seal>,
    /// The wait before each re-dial; `link_backoff_ms` with a draw from `ids` unless a test pins one.
    pub backoff_ms: Option<Rc<dyn Fn(i64) -> i64>>,
    /// Defaults to the real clock (rule 3.15).
    pub clock: Option<Rc<dyn Clock>>,
    /// Defaults to the OS's randomness (rule 3.16).
    pub ids: Option<Rc<dyn Ids>>,
}

// export interface ReachStats {
//   pingsSent: number;
//   pongsReceived: number;
//   reconnects: number;
// }
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReachStats {
    pub pings_sent: u64,
    pub pongs_received: u64,
    pub reconnects: u64,
}

/// A request that failed: the daemon's refusal with what it carried, or the link's own failure (`daemon
/// unreachable`, `connection lost`, `client closed`, and the readiness failures) with only a message.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct ReachError {
    pub message: String,
    pub kind: Option<MachineErrorKind>,
    pub status: Option<f64>,
    pub code: Option<String>,
}

// /** previewUrl → dialable ws(s) url; the edge's pt_token rides along, ours never does. */
// export function daemonWsUrl(previewUrl: string): string {
//   const url = new URL(previewUrl);
//   if (url.protocol === "https:") url.protocol = "wss:";
//   else if (url.protocol === "http:") url.protocol = "ws:";
//   return url.toString();
// }
pub fn daemon_ws_url(preview_url: &str) -> Result<String, ReachError> {
    let mut url =
        url::Url::parse(preview_url).map_err(|_| ReachError { message: "Invalid URL".into(), kind: None, status: None, code: None })?;
    match url.scheme() {
        "https" => {
            let _ = url.set_scheme("wss");
        }
        "http" => {
            let _ = url.set_scheme("ws");
        }
        _ => {}
    }
    Ok(url.to_string())
}

// const DEFAULT_HEARTBEAT_MS = 10_000;
pub const DEFAULT_HEARTBEAT_MS: i64 = 10_000;

// reach.ts:39
// export interface DaemonReach {
//   /** Resolves after the first successful connect + subscribe + rescan; rejects on 4401. */
//   readonly ready: Promise<void>;
//   request(op: string, params?: Record<string, unknown>): Promise<Record<string, unknown>>;
//   status(): DaemonLinkStatus;
//   stats(): ReachStats;
//   close(): void;
// }
/// One held link; tests fake it (seam table, rule 1.3).
#[async_trait(?Send)]
pub trait DaemonReach {
    /// Resolves after the first connect, auth, subscribe and rescan; rejects on 4401, on close, on a taken socket's end.
    async fn ready(&self) -> Result<(), ReachError>;
    /// `{...params, id, op}`; the reply when `ok === true`, else the daemon's refusal.
    async fn request(&self, op: &str, params: Frame) -> Result<Frame, ReachError>;
    /// `None` until the first status is set.
    fn status(&self) -> Option<DaemonLinkStatus>;
    fn stats(&self) -> ReachStats;
    fn close(&self);
}

// export function connectDaemon(opts: ReachOptions): DaemonReach {
//   if (opts.previewUrl === undefined && opts.socket === undefined) throw new Error("connectDaemon needs a previewUrl to dial or an open socket to take over");
//   if (opts.previewUrl !== undefined && opts.token === undefined) throw new Error("connectDaemon needs the daemon's token to dial a previewUrl");
//   const url = opts.previewUrl === undefined ? "" : daemonWsUrl(opts.previewUrl);
//   const heartbeatMs = opts.heartbeatMs ?? DEFAULT_HEARTBEAT_MS;
//   const backoff = opts.backoffMs ?? linkBackoffMs;
//
//   let ws: WebSocket | null = null;
//   let closed = false;
//   let nextId = 1;
//   let attempt = 0;
//   let connections = 0;
//   let awaitingPong = false;
//   let heartbeatTimer: NodeJS.Timeout | null = null;
//   let retryTimer: NodeJS.Timeout | null = null;
//   const pending = new Map<number, Pending>();
//   const stats: ReachStats = { pingsSent: 0, pongsReceived: 0, reconnects: 0 };
//
//   let status!: DaemonLinkStatus;
//   function setStatus(s: DaemonLinkStatus): void {
//     if (s === status) return;
//     status = s;
//     opts.onStatus?.(s);
//   }
//
//   let readyResolve!: () => void;
//   let readyReject!: (e: Error) => void;
//   const ready = new Promise<void>((resolve, reject) => {
//     readyResolve = resolve;
//     readyReject = reject;
//   });
//   ready.catch(() => {}); // a caller that never awaits ready must not crash the process
//
//   function send(op: string, params: Record<string, unknown> = {}): Promise<Record<string, unknown>> {
//     const sock = ws;
//     if (!sock || sock.readyState !== WebSocket.OPEN) return Promise.reject(new Error("daemon unreachable"));
//     const id = nextId++;
//     return new Promise((resolve, reject) => {
//       pending.set(id, { resolve, reject });
//       const text = JSON.stringify({ ...params, id, op });
//       sock.send(opts.seal === undefined ? text : opts.seal.seal(text));
//     });
//   }
//
//   function flushPending(reason: string): void {
//     for (const p of pending.values()) p.reject(new Error(reason));
//     pending.clear();
//   }
//
//   function stopHeartbeat(): void {
//     if (heartbeatTimer) clearInterval(heartbeatTimer);
//     heartbeatTimer = null;
//     awaitingPong = false;
//   }
//
//   function startHeartbeat(sock: WebSocket): void {
//     stopHeartbeat();
//     heartbeatTimer = setInterval(() => {
//       if (awaitingPong) {
//         sock.terminate(); // no ack within a full beat: the socket is half-dead
//         return;
//       }
//       awaitingPong = true;
//       stats.pingsSent++;
//       send("ping").then(
//         () => {
//           stats.pongsReceived++;
//           awaitingPong = false;
//         },
//         () => {}, // the close handler owns recovery
//       );
//     }, heartbeatMs);
//   }
//
//   function handleMessage(raw: unknown): void {
//     let msg: Record<string, unknown>;
//     try {
//       // A frame that does not open under the key both ends agreed is a carrier writing into the link: the socket
//       // is cut and the link redials, rather than a frame nobody proved being read as the computer's own.
//       msg = JSON.parse(openFrame(opts.seal, raw)) as Record<string, unknown>;
//     } catch {
//       if (opts.seal !== undefined) ws?.close(1002, SEAL_REFUSAL);
//       return;
//     }
//     const id = msg["id"];
//     if (typeof id === "number" && pending.has(id)) {
//       const p = pending.get(id)!;
//       pending.delete(id);
//       if (msg["ok"] === true) p.resolve(msg);
//       else p.reject(refused(msg));
//       return;
//     }
//     const parsed = DaemonEvent.safeParse(msg);
//     if (parsed.success) opts.onEvent(parsed.data);
//   }
//
//   /** A refused frame as the caller's error. The fields the daemon answers with ride along, so a refusal keeps its
//    * meaning on the computer that asked: a machine the far side lost reads missing here, and the code a files or git
//    * frame was refused with is what tells a checkout with no remote from a computer with no signed-in gh. */
//   function refused(msg: Record<string, unknown>): Error {
//     const kind = MachineErrorKind.safeParse(msg["kind"]);
//     const status = msg["status"];
//     const code = msg["code"];
//     return Object.assign(new Error(String(msg["error"] ?? "daemon error")), {
//       ...(kind.success ? { kind: kind.data } : {}),
//       ...(typeof status === "number" ? { status } : {}),
//       ...(typeof code === "string" ? { code } : {}),
//     });
//   }
//
//   function scheduleReconnect(): void {
//     if (closed) return;
//     // A socket somebody else opened cannot be opened again from here: the place dials this host, so the link ending
//     // is this reach ending, and the place's own redial is what brings the next one.
//     if (opts.socket !== undefined) {
//       setStatus("dead");
//       readyReject(new Error("the place's link closed")); // no-op once ready resolved
//       return;
//     }
//     setStatus("connecting");
//     attempt++;
//     retryTimer = setTimeout(dial, backoff(attempt));
//   }
//
//   async function ritual(sock: WebSocket): Promise<void> {
//     try {
//       // The auth frame is the dial's; a socket the far side opened proved itself by the handshake that opened it.
//       if (opts.previewUrl !== undefined) await send("auth", { token: opts.token });
//       await send("ports.watch");
//       await send("inbox.watch");
//       await send("inbox.rescan");
//     } catch {
//       return; // connection died mid-ritual; the close handler redials
//     }
//     if (ws !== sock || closed) return;
//     attempt = 0;
//     connections++;
//     stats.reconnects = connections - 1;
//     startHeartbeat(sock);
//     setStatus("live");
//     readyResolve();
//   }
//
//   function dial(): void {
//     if (closed) return;
//     setStatus("connecting");
//     const sock = new WebSocket(url);
//     ws = sock;
//     sock.addEventListener("open", () => void ritual(sock));
//     sock.addEventListener("message", ev => handleMessage(ev.data));
//     sock.addEventListener("error", () => {}); // a close event always follows
//     sock.addEventListener("close", ev => {
//       if (ws !== sock) return; // an already-replaced socket
//       stopHeartbeat();
//       flushPending("connection lost");
//       if (closed) return;
//       if (daemonRefusedToken(ev.code, String(ev.reason))) {
//         // The host owns the token it sent; a guest refusing it will refuse the same one again.
//         setStatus("reauth-needed");
//         readyReject(new Error(`daemon rejected token (4401 ${String(ev.reason)})`)); // no-op once ready resolved
//         return;
//       }
//       scheduleReconnect();
//     });
//   }
//
//   /** A socket already open: the same listeners a dial installs, and the ritual as soon as the loop turns. A
//    * socket handed over paused is read again here, once this listener stands, so no frame the other end sent in
//    * the gap is lost. */
//   function take(sock: WebSocket): void {
//     ws = sock;
//     sock.addEventListener("message", ev => handleMessage(ev.data));
//     sock.resume();
//     sock.addEventListener("error", () => {});
//     sock.addEventListener("close", () => {
//       if (ws !== sock) return;
//       stopHeartbeat();
//       flushPending("connection lost");
//       if (closed) return;
//       scheduleReconnect();
//     });
//     void ritual(sock);
//   }
//
//   if (opts.socket !== undefined) take(opts.socket);
//   else dial();
//
//   return {
//     ready,
//     request: send,
//     status: () => status,
//     stats: () => ({ ...stats }),
//     close() {
//       closed = true;
//       if (retryTimer) clearTimeout(retryTimer);
//       stopHeartbeat();
//       flushPending("client closed");
//       ws?.close(1000);
//       setStatus("dead");
//       readyReject(new Error("client closed")); // no-op once ready resolved
//     },
//   };
// }
/// Starts the link on the owner thread's `LocalSet`; refuses synchronously a reach with no road.
pub fn connect_daemon(opts: ReachOptions) -> Result<Rc<dyn DaemonReach>, ReachError> {
    let ReachOptions { preview_url, socket, token, on_event, on_status, heartbeat_ms, seal, backoff_ms, clock, ids } = opts;
    if preview_url.is_none() && socket.is_none() {
        return Err(link_error("connectDaemon needs a previewUrl to dial or an open socket to take over"));
    }
    if preview_url.is_some() && token.is_none() {
        return Err(link_error("connectDaemon needs the daemon's token to dial a previewUrl"));
    }
    let url = match &preview_url {
        Some(preview) => daemon_ws_url(preview)?,
        None => String::new(),
    };
    let ids: Rc<dyn Ids> = ids.unwrap_or_else(|| -> Rc<dyn Ids> { Rc::new(OsIds) });
    let backoff: Rc<dyn Fn(i64) -> i64> = match backoff_ms {
        Some(f) => f,
        None => Rc::new(move |attempt| link_backoff_ms(attempt, ids.random())),
    };
    let core = Rc::new(Core {
        dials: socket.is_none(),
        auth: preview_url.is_some(),
        url,
        token: token.unwrap_or_default(),
        on_event,
        on_status,
        heartbeat_ms: heartbeat_ms.unwrap_or(DEFAULT_HEARTBEAT_MS),
        backoff,
        clock: clock.unwrap_or_else(real_clock),
        seal: RefCell::new(seal),
        ready: watch::channel(None).0,
        state: RefCell::new(State::default()),
    });
    match socket {
        Some(taken) => take(&core, taken),
        None => dial(core.clone()),
    }
    Ok(Rc::new(Link(core)))
}

/// The link's state, shared by its timers, its socket tasks and the handle.
struct Core {
    /// False for a socket somebody else opened: its close is the end of the reach.
    dials: bool,
    /// The ritual sends the auth frame first (a previewUrl was given).
    auth: bool,
    url: String,
    token: String,
    on_event: Rc<dyn Fn(DaemonEvent)>,
    on_status: Option<Rc<dyn Fn(DaemonLinkStatus)>>,
    heartbeat_ms: i64,
    backoff: Rc<dyn Fn(i64) -> i64>,
    clock: Rc<dyn Clock>,
    seal: RefCell<Option<Seal>>,
    ready: watch::Sender<Option<Result<(), ReachError>>>,
    state: RefCell<State>,
}

#[derive(Default)]
struct State {
    sock: Option<Rc<Socket>>,
    closed: bool,
    next_id: u64,
    attempt: i64,
    connections: u64,
    awaiting_pong: bool,
    heartbeat: Option<Cancel>,
    retry: Option<Cancel>,
    pending: IndexMap<u64, oneshot::Sender<Result<Frame, ReachError>>>,
    stats: ReachStats,
    status: Option<DaemonLinkStatus>,
}

/// One socket's outbound queue. `open` is the TS readyState being OPEN.
struct Socket {
    out: mpsc::UnboundedSender<Out>,
    open: Cell<bool>,
}

enum Out {
    Text(String),
    Binary(Vec<u8>),
    Close(u16, &'static str),
    Terminate,
}

struct Link(Rc<Core>);

#[async_trait(?Send)]
impl DaemonReach for Link {
    async fn ready(&self) -> Result<(), ReachError> {
        let mut rx = self.0.ready.subscribe();
        let settled = rx.wait_for(Option::is_some).await.expect("the reach holds the ready sender").clone();
        settled.expect("ready settled")
    }
    async fn request(&self, op: &str, params: Frame) -> Result<Frame, ReachError> {
        send_request(&self.0, op, params).await
    }
    fn status(&self) -> Option<DaemonLinkStatus> {
        self.0.state.borrow().status
    }
    fn stats(&self) -> ReachStats {
        self.0.state.borrow().stats
    }
    fn close(&self) {
        let (retry, sock) = {
            let mut st = self.0.state.borrow_mut();
            st.closed = true;
            (st.retry.take(), st.sock.clone())
        };
        if let Some(cancel) = retry {
            cancel();
        }
        stop_heartbeat(&self.0);
        flush_pending(&self.0, "client closed");
        if let Some(sock) = sock {
            sock.open.set(false);
            let _ = sock.out.send(Out::Close(1000, ""));
        }
        set_status(&self.0, DaemonLinkStatus::Dead);
        settle_ready(&self.0, Err(link_error("client closed")));
    }
}

fn link_error(message: &str) -> ReachError {
    ReachError { message: message.into(), kind: None, status: None, code: None }
}

fn set_status(core: &Rc<Core>, status: DaemonLinkStatus) {
    let changed = {
        let mut st = core.state.borrow_mut();
        let changed = st.status != Some(status);
        st.status = Some(status);
        changed
    };
    if changed {
        if let Some(on_status) = &core.on_status {
            on_status(status);
        }
    }
}

fn settle_ready(core: &Rc<Core>, outcome: Result<(), ReachError>) {
    if core.ready.borrow().is_none() {
        core.ready.send_replace(Some(outcome));
    }
}

fn flush_pending(core: &Rc<Core>, reason: &str) {
    let waiters = std::mem::take(&mut core.state.borrow_mut().pending);
    for (_, waiter) in waiters {
        let _ = waiter.send(Err(link_error(reason)));
    }
}

fn stop_heartbeat(core: &Rc<Core>) {
    let cancel = {
        let mut st = core.state.borrow_mut();
        st.awaiting_pong = false;
        st.heartbeat.take()
    };
    if let Some(cancel) = cancel {
        cancel();
    }
}

fn start_heartbeat(core: &Rc<Core>, sock: Rc<Socket>) {
    stop_heartbeat(core);
    schedule_beat(core, sock);
}

fn schedule_beat(core: &Rc<Core>, sock: Rc<Socket>) {
    let next = core.clone();
    let cancel = core.clock.schedule(Box::new(move || beat(next, sock)), core.heartbeat_ms);
    core.state.borrow_mut().heartbeat = Some(cancel);
}

/// One beat: the next one is armed first, as the TS interval runs on regardless of what this beat finds.
fn beat(core: Rc<Core>, sock: Rc<Socket>) {
    schedule_beat(&core, sock.clone());
    let awaiting = core.state.borrow().awaiting_pong;
    if awaiting {
        let _ = sock.out.send(Out::Terminate);
        return;
    }
    {
        let mut st = core.state.borrow_mut();
        st.awaiting_pong = true;
        st.stats.pings_sent += 1;
    }
    tokio::task::spawn_local(async move {
        if send_request(&core, "ping", Frame::new()).await.is_ok() {
            let mut st = core.state.borrow_mut();
            st.stats.pongs_received += 1;
            st.awaiting_pong = false;
        }
    });
}

async fn send_request(core: &Rc<Core>, op: &str, params: Frame) -> Result<Frame, ReachError> {
    let (tx, rx) = oneshot::channel();
    let sent = {
        let mut st = core.state.borrow_mut();
        match st.sock.clone() {
            Some(sock) if sock.open.get() => {
                st.next_id += 1;
                let id = st.next_id;
                st.pending.insert(id, tx);
                Some((sock, id))
            }
            _ => None,
        }
    };
    let Some((sock, id)) = sent else { return Err(link_error("daemon unreachable")) };
    let mut frame = params;
    frame.insert("id".into(), json!(id));
    frame.insert("op".into(), json!(op));
    let text = Value::Object(frame).to_string();
    let out = match core.seal.borrow_mut().as_mut() {
        Some(seal) => Out::Binary(seal.seal(&text)),
        None => Out::Text(text),
    };
    if sock.out.send(out).is_err() {
        core.state.borrow_mut().pending.shift_remove(&id);
        return Err(link_error("daemon unreachable"));
    }
    match rx.await {
        Ok(reply) => reply,
        Err(_) => Err(link_error("connection lost")),
    }
}

/// The ritual on every socket: the auth frame when dialling a previewUrl, then the three watches, each awaited.
async fn ritual(core: Rc<Core>, sock: Rc<Socket>) {
    if core.auth {
        let mut auth = Frame::new();
        auth.insert("token".into(), json!(core.token));
        if send_request(&core, "auth", auth).await.is_err() {
            return;
        }
    }
    for op in ["ports.watch", "inbox.watch", "inbox.rescan"] {
        if send_request(&core, op, Frame::new()).await.is_err() {
            return;
        }
    }
    let current = {
        let st = core.state.borrow();
        !st.closed && st.sock.as_ref().is_some_and(|s| Rc::ptr_eq(s, &sock))
    };
    if !current {
        return;
    }
    {
        let mut st = core.state.borrow_mut();
        st.attempt = 0;
        st.connections += 1;
        st.stats.reconnects = st.connections - 1;
    }
    start_heartbeat(&core, sock);
    set_status(&core, DaemonLinkStatus::Live);
    settle_ready(&core, Ok(()));
}

fn dial(core: Rc<Core>) {
    if core.state.borrow().closed {
        return;
    }
    set_status(&core, DaemonLinkStatus::Connecting);
    let (out, outbound) = mpsc::unbounded_channel();
    let sock = Rc::new(Socket { out, open: Cell::new(false) });
    core.state.borrow_mut().sock = Some(sock.clone());
    tokio::task::spawn_local(async move {
        let ws = match connect_async_with_config(core.url.as_str(), Some(socket_config()), false).await {
            Ok((ws, _)) => ws,
            Err(_) => {
                on_socket_end(&core, &sock, end_1006());
                return;
            }
        };
        let TakenSocket { sink, stream } = TakenSocket::new(ws);
        sock.open.set(true);
        tokio::task::spawn_local(ritual(core.clone(), sock.clone()));
        let end = run_socket(&core, outbound, sink, stream).await;
        on_socket_end(&core, &sock, end);
    });
}

fn take(core: &Rc<Core>, taken: TakenSocket) {
    let (out, outbound) = mpsc::unbounded_channel();
    let sock = Rc::new(Socket { out, open: Cell::new(true) });
    core.state.borrow_mut().sock = Some(sock.clone());
    let core = core.clone();
    tokio::task::spawn_local(ritual(core.clone(), sock.clone()));
    let TakenSocket { sink, stream } = taken;
    tokio::task::spawn_local(async move {
        let end = run_socket(&core, outbound, sink, stream).await;
        on_socket_end(&core, &sock, end);
    });
}

/// Reads and writes one socket until it ends, and says how it ended.
async fn run_socket(
    core: &Rc<Core>,
    mut outbound: mpsc::UnboundedReceiver<Out>,
    mut sink: Pin<Box<dyn Sink<Message, Error = WsError>>>,
    mut stream: LocalBoxStream<'static, Result<Message, WsError>>,
) -> ChannelEnd {
    let mut gave_up: Option<oneshot::Receiver<()>> = None;
    loop {
        tokio::select! {
            _ = async { match gave_up.as_mut() { Some(rx) => drop(rx.await), None => std::future::pending::<()>().await } } => return end_1006(),
            msg = stream.next() => match msg {
                Some(Ok(Message::Text(text))) => handle_message(core, RawFrame::Text(text.as_str())),
                Some(Ok(Message::Binary(bytes))) => handle_message(core, RawFrame::Binary(&bytes[..])),
                Some(Ok(Message::Close(frame))) => return close_end(frame),
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return end_1006(),
            },
            out = outbound.recv() => {
                let sent = match out {
                    Some(Out::Text(text)) => sink.send(Message::text(text)).await,
                    Some(Out::Binary(bytes)) => sink.send(Message::binary(bytes)).await,
                    Some(Out::Close(code, reason)) => {
                        if gave_up.is_none() {
                            let (tx, rx) = oneshot::channel();
                            core.clock.schedule(Box::new(move || {
                            let _ = tx.send(());
                        }), CLOSE_GRACE_MS);
                            gave_up = Some(rx);
                        }
                        sink.send(Message::Close(Some(CloseFrame { code: CloseCode::from(code), reason: reason.into() }))).await
                    }
                    Some(Out::Terminate) | None => return end_1006(),
                };
                if sent.is_err() {
                    return end_1006();
                }
            }
        }
    }
}

/// The close handler: only the current socket's close counts; a dialled one may be refused its token, and any other
/// end redials (or, for a socket somebody else opened, ends the reach).
fn on_socket_end(core: &Rc<Core>, sock: &Rc<Socket>, end: ChannelEnd) {
    let current = core.state.borrow().sock.as_ref().is_some_and(|s| Rc::ptr_eq(s, sock));
    if !current {
        return;
    }
    sock.open.set(false);
    stop_heartbeat(core);
    flush_pending(core, "connection lost");
    if core.state.borrow().closed {
        return;
    }
    if core.dials && daemon_refused_token(f64::from(end.code), &end.reason) {
        set_status(core, DaemonLinkStatus::ReauthNeeded);
        settle_ready(core, Err(link_error(&format!("daemon rejected token (4401 {})", end.reason))));
        return;
    }
    schedule_reconnect(core);
}

fn schedule_reconnect(core: &Rc<Core>) {
    if core.state.borrow().closed {
        return;
    }
    if !core.dials {
        set_status(core, DaemonLinkStatus::Dead);
        settle_ready(core, Err(link_error("the place's link closed")));
        return;
    }
    set_status(core, DaemonLinkStatus::Connecting);
    let attempt = {
        let mut st = core.state.borrow_mut();
        st.attempt += 1;
        st.attempt
    };
    let delay = (core.backoff)(attempt);
    let next = core.clone();
    let cancel = core.clock.schedule(Box::new(move || dial(next)), delay);
    core.state.borrow_mut().retry = Some(cancel);
}

/// One frame in: opened under the seal when there is one, settles its pending request or goes to `on_event`.
fn handle_message(core: &Rc<Core>, raw: RawFrame<'_>) {
    let opened = {
        let mut seal = core.seal.borrow_mut();
        open_frame(seal.as_mut(), raw)
    };
    let parsed = opened.ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let Some(msg) = parsed else {
        if core.seal.borrow().is_some() {
            let current = core.state.borrow().sock.clone();
            if let Some(sock) = current {
                let _ = sock.out.send(Out::Close(1002, SEAL_REFUSAL));
            }
        }
        return;
    };
    if let Some(id) = reply_id(&msg) {
        let waiter = core.state.borrow_mut().pending.shift_remove(&id);
        if let Some(waiter) = waiter {
            let outcome = if msg.get("ok") == Some(&Value::Bool(true)) {
                match msg {
                    Value::Object(reply) => Ok(reply),
                    _ => Err(refused(&msg)),
                }
            } else {
                Err(refused(&msg))
            };
            let _ = waiter.send(outcome);
            return;
        }
    }
    if let Ok(event) = parse_daemon_event(&msg) {
        (core.on_event)(event);
    }
}

/// A refused request as the caller's error: the daemon's words, and the kind, status and code it carried.
fn refused(msg: &Value) -> ReachError {
    let message = match msg.get("error") {
        None | Some(Value::Null) => "daemon error".to_string(),
        Some(v) => js_string(v),
    };
    ReachError {
        message,
        kind: msg.get("kind").and_then(|k| serde_json::from_value::<MachineErrorKind>(k.clone()).ok()),
        status: msg.get("status").and_then(Value::as_f64),
        code: msg.get("code").and_then(Value::as_str).map(str::to_string),
    }
}

fn end_1006() -> ChannelEnd {
    ChannelEnd { code: 1006, reason: String::new() }
}

fn close_end(frame: Option<CloseFrame>) -> ChannelEnd {
    match frame {
        Some(f) => ChannelEnd { code: u16::from(f.code), reason: f.reason.to_string() },
        None => ChannelEnd { code: 1005, reason: String::new() },
    }
}
