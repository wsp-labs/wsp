// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/daemon-channel.ts at 65f6b89c7. Spec: spec/daemon_channel.md.
//!
//! One dial of one daemon for one client of this host, with no redial and no subscribe ritual: the page that opened
//! the channel owns the retry and asks for ports.watch and the rest itself. The host's own held links (reach.rs) keep
//! a machine warm and re-dial it, which is a different concern.
use std::cell::{Cell, RefCell};
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{client_async_with_config, WebSocketStream};
use wsp_words::daemon_contract::daemon_refused_token;
use wsp_words::js::js_trim;

use crate::clock::{real_clock, Clock};
use crate::reach::daemon_ws_url;

/// A daemon frame or reply: a JSON object, keys in the order written.
pub type Frame = Map<String, Value>;

// export interface DaemonChannelOptions {
//   /** The road daemonReach answered: a minted route or a loopback url; the host turns http(s) into ws(s). */
//   url: string;
//   token: string;
//   /** Every frame that is not the answer to a send, the hello included, in the order the daemon sent them. */
//   onEvent(frame: Record<string, unknown>): void;
//   connectTimeoutMs?: number;
// }
pub struct DaemonChannelOptions {
    /// The road daemonReach answered: a minted route or a loopback url; http(s) becomes ws(s).
    pub url: String,
    pub token: String,
    /// Every frame that is not the answer to a send, the hello included, in the order the daemon sent them.
    pub on_event: Rc<dyn Fn(Value)>,
    pub connect_timeout_ms: Option<i64>,
    /// Defaults to the real clock (rule 3.15).
    pub clock: Option<Rc<dyn Clock>>,
}

/// How the daemon socket ended: the close code and its reason as UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEnd {
    pub code: u16,
    pub reason: String,
}

// /** The door answered the upgrade with a status: the socket never opened. kind is what the wire carries. The message
//  * is shown under the pane's own sentence, so it carries the status and the door's words and none of ours: a person
//  * never reads machine or daemon. */
// export class DaemonDoorError extends Error {
//   readonly kind = "refused";
//   readonly status: number;
//   constructor(status: number, body: string) {
//     // One line of the body: a door that answers an HTML page must not become the pane's sentence.
//     const line = body.split("\n", 1)[0]!.trim();
//     super(line === "" ? `the connection was refused with ${status}` : `the connection was refused with ${status}: ${line}`);
//     this.name = "DaemonDoorError";
//     this.status = status;
//   }
// }
/// The door answered the upgrade with a status: the socket never opened. kind "refused".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonDoorError {
    pub status: u16,
    pub message: String,
}

impl DaemonDoorError {
    /// One line of the body: `the connection was refused with <status>[: <first line>]`.
    pub fn new(status: u16, body: &str) -> Self {
        let line = js_trim(body.split('\n').next().unwrap_or_default());
        let message = if line.is_empty() {
            format!("the connection was refused with {status}")
        } else {
            format!("the connection was refused with {status}: {line}")
        };
        DaemonDoorError { status, message }
    }
}

// /** The daemon took the upgrade and closed 4401 before answering the auth frame. */
// export class DaemonTokenError extends Error {
//   readonly kind = "reauth";
//   constructor(reason: string) {
//     super(`the machine refused this host's daemon token (4401 ${reason})`);
//     this.name = "DaemonTokenError";
//   }
// }
/// The daemon took the upgrade and closed 4401 before answering the auth frame. kind "reauth".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonTokenError {
    pub message: String,
}

impl DaemonTokenError {
    /// `the machine refused this host's daemon token (4401 <reason>)`.
    pub fn new(reason: &str) -> Self {
        DaemonTokenError { message: format!("the machine refused this host's daemon token (4401 {reason})") }
    }
}

// /** The link to the daemon gave out with no answer from it: a dial that timed out, a socket that was not open, one
//  * that closed under a frame. Its own class and no kind, since a kind is what the wire carries and these say nothing
//  * to anyone but the host, which tells them from the daemon's own refusal by the class. */
// class DaemonUnanswered extends Error {}
// const daemonUnanswered = (message: string, cause?: unknown): Error => new DaemonUnanswered(message, cause === undefined ? undefined : { cause });
// export const isDaemonUnanswered = (e: unknown): boolean => e instanceof DaemonUnanswered;
/// Every way a dial or a send fails. `Unanswered` is the TS's `DaemonUnanswered` class (no kind); `Refused` is the
/// plain Error a refused auth reply rejects with (no kind).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChannelError {
    #[error("{message}")]
    Unanswered { message: String, cause: Option<ChannelEnd> },
    #[error("{}", .0.message)]
    Door(DaemonDoorError),
    #[error("{}", .0.message)]
    Token(DaemonTokenError),
    #[error("{message}")]
    Refused { message: String },
}

impl ChannelError {
    /// The kind the wire carries: "refused" for a door, "reauth" for a token, none for the rest.
    pub fn kind(&self) -> Option<&'static str> {
        match self {
            ChannelError::Door(_) => Some("refused"),
            ChannelError::Token(_) => Some("reauth"),
            ChannelError::Unanswered { .. } | ChannelError::Refused { .. } => None,
        }
    }
}

// daemon-channel.ts:33
// export const isDaemonUnanswered = (e: unknown): boolean => e instanceof DaemonUnanswered;
pub fn is_daemon_unanswered(e: &ChannelError) -> bool {
    matches!(e, ChannelError::Unanswered { .. })
}

// /** The dial, the auth frame and the reply that proves it, before a connect is given up. Named here rather than
//  * imported from the host: the dependency arrows run runtime to host, never back. */
// const CONNECT_TIMEOUT_MS = 15_000;
pub const CONNECT_TIMEOUT_MS: i64 = 15_000;

// /** The bytes of a refusing door's body kept for its sentence; the first line is what a pane shows. */
// const DOOR_BODY_CAP = 512;
pub const DOOR_BODY_CAP: usize = 512;

/// How long a socket that was asked to close may wait for the peer's close frame before it is dropped and reads 1006:
/// `ws`'s `closeTimeout` (30 s), which the TS's `ws.close(1000)` runs under.
pub(crate) const CLOSE_GRACE_MS: i64 = 30_000;

/// The most one `ws` message may weigh, which the TS's sockets and the daemon's door (`door.rs:29`) both take.
const MAX_PAYLOAD: usize = 100 * 1024 * 1024;

/// What every dial of a daemon reads a frame under: `ws`'s 100 MiB, not tungstenite's 16 MiB frame and 64 MiB message.
pub(crate) fn socket_config() -> WebSocketConfig {
    WebSocketConfig::default().max_message_size(Some(MAX_PAYLOAD)).max_frame_size(Some(MAX_PAYLOAD))
}

// daemon-channel.ts:21
// export interface DaemonChannel {
//   send(frame: DaemonFrame): Promise<DaemonResponse>;
//   close(): void;
//   /** Settles once, when the daemon socket ends for any reason, close() included. */
//   readonly closed: Promise<{ code: number; reason: string }>;
// }
/// One open channel; tests fake it (seam table, rule 1.3).
#[async_trait(?Send)]
pub trait DaemonChannel {
    /// The frame with `id` set (the caller's keys, then `id`); the daemon's reply as it wrote it, `ok: false`
    /// included. A socket that is not open fails `daemon unreachable`; one that closes under the frame `connection lost`.
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError>;
    /// Closes with 1000.
    fn close(&self);
    /// Settles once, when the daemon socket ends for any reason, close() included; every await reads the same end.
    async fn closed(&self) -> ChannelEnd;
}

// export function openDaemonChannel(o: DaemonChannelOptions): Promise<DaemonChannel> {
//   const timeoutMs = o.connectTimeoutMs ?? CONNECT_TIMEOUT_MS;
//   return new Promise((resolve, reject) => {
//     const ws = new WebSocket(daemonWsUrl(o.url));
//     const pending = new Map<number, Pending>();
//     let nextId = 1;
//     let settled = false;
//     /** The door's own answer, kept so the close that follows it reads as a refusal rather than a dead machine. */
//     let refusal: DaemonDoorError | undefined;
//     let resolveClosed: (end: { code: number; reason: string }) => void = () => {};
//     const closed = new Promise<{ code: number; reason: string }>(r => (resolveClosed = r));
//
//     const connectTimer = setTimeout(() => {
//       if (settled) return;
//       settled = true;
//       ws.terminate();
//       reject(daemonUnanswered(`daemon connect timed out after ${timeoutMs}ms`));
//     }, timeoutMs);
//
//     const send = (frame: DaemonFrame): Promise<DaemonResponse> => {
//       if (ws.readyState !== ws.OPEN) return Promise.reject(daemonUnanswered("daemon unreachable"));
//       const id = nextId++;
//       return new Promise((res, rej) => {
//         pending.set(id, { resolve: res, reject: rej });
//         ws.send(JSON.stringify({ ...frame, id }));
//       });
//     };
//
//     const channel: DaemonChannel = {
//       send,
//       close: () => ws.close(1000),
//       closed,
//     };
//
//     // ws folds a non-101 answer into an error and a 1006 close without this listener, which is how a refusing door
//     // and a machine that is not there would read the same. The response's body is the door's own sentence.
//     ws.on("unexpected-response", (_req, res) => {
//       let body = "";
//       res.on("data", (chunk: Buffer) => {
//         if (body.length < DOOR_BODY_CAP) body += chunk.toString("utf8");
//       });
//       const done = (): void => {
//         refusal ??= new DaemonDoorError(res.statusCode ?? 0, body.slice(0, DOOR_BODY_CAP));
//         if (settled) return;
//         settled = true;
//         clearTimeout(connectTimer);
//         reject(refusal);
//       };
//       res.on("end", done);
//       res.on("error", done);
//     });
//
//     ws.on("open", () => {
//       send({ op: "auth", token: o.token } as unknown as DaemonFrame).then(
//         reply => {
//           if (settled) return;
//           settled = true;
//           clearTimeout(connectTimer);
//           if (reply.ok === true) resolve(channel);
//           else reject(new Error(String((reply as { error?: unknown }).error ?? "the machine refused the daemon link")));
//         },
//         () => {}, // the close handler owns the rejection
//       );
//     });
//
//     ws.on("message", raw => {
//       let msg: Record<string, unknown>;
//       try {
//         msg = JSON.parse(String(raw)) as Record<string, unknown>;
//       } catch {
//         return;
//       }
//       const id = msg["id"];
//       if (typeof id === "number" && pending.has(id)) {
//         const p = pending.get(id)!;
//         pending.delete(id);
//         p.resolve(msg as DaemonResponse);
//         return;
//       }
//       o.onEvent(msg);
//     });
//
//     ws.on("error", () => {}); // a close event always follows
//
//     ws.on("close", (code: number, reasonBuf: Buffer) => {
//       clearTimeout(connectTimer);
//       const reason = reasonBuf.toString("utf8");
//       for (const p of pending.values()) p.reject(daemonUnanswered("connection lost"));
//       pending.clear();
//       resolveClosed({ code, reason });
//       if (settled) return;
//       settled = true;
//       if (refusal !== undefined) reject(refusal);
//       else if (daemonRefusedToken(code, reason)) reject(new DaemonTokenError(reason));
//       // The close code is for the log, never for the person's eyes.
//       else reject(daemonUnanswered("the link to the computer ended before it opened", { code, reason }));
//     });
//   });
// }
/// Dials once, sends the auth frame first, resolves on its reply. Runs on the owner thread's `LocalSet`.
pub async fn open_daemon_channel(o: DaemonChannelOptions) -> Result<Rc<dyn DaemonChannel>, ChannelError> {
    let clock = o.clock.clone().unwrap_or_else(real_clock);
    let timeout_ms = o.connect_timeout_ms.unwrap_or(CONNECT_TIMEOUT_MS);
    let (expired_tx, expired) = oneshot::channel::<()>();
    let cancel_deadline = clock.schedule(
        Box::new(move || {
            let _ = expired_tx.send(());
        }),
        timeout_ms,
    );
    let (outbound, outbound_rx) = mpsc::unbounded_channel();
    let link = Rc::new(Link {
        outbound,
        pending: RefCell::default(),
        next_id: Cell::new(1),
        closing: Cell::new(false),
        end: watch::channel(None).0,
        on_event: o.on_event.clone(),
    });
    let opened = tokio::select! {
        biased;
        r = connect(&o.url, &o.token, link.clone(), outbound_rx, clock.clone()) => r,
        _ = expired => Err(unanswered(&format!("daemon connect timed out after {timeout_ms}ms"))),
    };
    cancel_deadline();
    opened?;
    let channel: Rc<dyn DaemonChannel> = Rc::new(Channel { link });
    Ok(channel)
}

/// The socket's outbound queue, the sends waiting on a reply, and how the socket ended.
struct Link {
    outbound: mpsc::UnboundedSender<Outbound>,
    pending: RefCell<IndexMap<u64, oneshot::Sender<Result<Frame, ChannelError>>>>,
    next_id: Cell<u64>,
    closing: Cell<bool>,
    end: watch::Sender<Option<ChannelEnd>>,
    on_event: Rc<dyn Fn(Value)>,
}

enum Outbound {
    Text(String),
    Close,
}

impl Link {
    fn take_id(&self) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        id
    }

    /// A reply to a pending send settles it; every other frame goes to `on_event`, in arrival order.
    fn deliver(&self, value: Value) {
        if let Some(id) = reply_id(&value) {
            let waiter = self.pending.borrow_mut().shift_remove(&id);
            if let Some(waiter) = waiter {
                if let Value::Object(reply) = value {
                    let _ = waiter.send(Ok(reply));
                }
                return;
            }
        }
        (self.on_event)(value);
    }

    async fn request(&self, frame: Frame) -> Result<Frame, ChannelError> {
        if self.closing.get() || self.end.borrow().is_some() {
            return Err(unanswered("daemon unreachable"));
        }
        let id = self.take_id();
        let (tx, rx) = oneshot::channel();
        self.pending.borrow_mut().insert(id, tx);
        if self.outbound.send(Outbound::Text(with_id(frame, id))).is_err() {
            self.pending.borrow_mut().shift_remove(&id);
            return Err(unanswered("daemon unreachable"));
        }
        match rx.await {
            Ok(reply) => reply,
            Err(_) => Err(unanswered("connection lost")),
        }
    }

    /// Records how the socket ended before it opened and gives the error a dial reports for it.
    fn ended(&self, end: ChannelEnd) -> ChannelError {
        self.end.send_replace(Some(end.clone()));
        ended_before_open(end)
    }
}

struct Channel {
    link: Rc<Link>,
}

#[async_trait(?Send)]
impl DaemonChannel for Channel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.link.request(frame).await
    }

    fn close(&self) {
        self.link.closing.set(true);
        let _ = self.link.outbound.send(Outbound::Close);
    }

    async fn closed(&self) -> ChannelEnd {
        let mut rx = self.link.end.subscribe();
        let ended = rx.wait_for(Option::is_some).await.expect("the channel holds the end's sender").clone();
        ended.expect("wait_for returned a settled end")
    }
}

/// The socket is dialled, the auth frame goes out first, and its reply is read before anything else. The socket is
/// handed to `run` even when the reply refuses the link, as the TS leaves it open.
async fn connect(
    url: &str,
    token: &str,
    link: Rc<Link>,
    outbound: mpsc::UnboundedReceiver<Outbound>,
    clock: Rc<dyn Clock>,
) -> Result<(), ChannelError> {
    let ws_url = daemon_ws_url(url).map_err(|e| ChannelError::Refused { message: e.message })?;
    let mut ws = dial(&ws_url).await?;
    let mut auth = Frame::new();
    auth.insert("op".into(), json!("auth"));
    auth.insert("token".into(), json!(token));
    if ws.send(Message::text(with_id(auth, link.take_id()))).await.is_err() {
        return Err(link.ended(end_1006()));
    }
    let reply = loop {
        let text = match ws.next().await {
            Some(Ok(Message::Text(t))) => t.to_string(),
            Some(Ok(Message::Binary(b))) => String::from_utf8_lossy(&b).into_owned(),
            Some(Ok(Message::Close(frame))) => return Err(link.ended(close_end(frame))),
            Some(Ok(_)) => continue,
            Some(Err(_)) | None => return Err(link.ended(end_1006())),
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
        if reply_id(&value) == Some(1) {
            if let Value::Object(reply) = value {
                break reply;
            }
        }
        link.deliver(value);
    };
    let accepted = reply.get("ok") == Some(&Value::Bool(true));
    tokio::task::spawn_local(run(ws, outbound, link, clock));
    if accepted {
        Ok(())
    } else {
        Err(ChannelError::Refused { message: refusal_words(reply.get("error")) })
    }
}

/// Reads the socket until it ends, routing each frame, then settles the end and fails every send still waiting.
async fn run(mut ws: WebSocketStream<Shared>, mut outbound: mpsc::UnboundedReceiver<Outbound>, link: Rc<Link>, clock: Rc<dyn Clock>) {
    let mut outbound_open = true;
    let mut gave_up: Option<oneshot::Receiver<()>> = None;
    let end = loop {
        tokio::select! {
            msg = ws.next() => match msg {
                Some(Ok(Message::Text(t))) => route(&link, &t),
                Some(Ok(Message::Binary(b))) => route(&link, &String::from_utf8_lossy(&b)),
                Some(Ok(Message::Close(frame))) => break close_end(frame),
                Some(Ok(_)) => {}
                Some(Err(_)) | None => break end_1006(),
            },
            out = outbound.recv(), if outbound_open => match out {
                Some(Outbound::Text(text)) => {
                    if ws.send(Message::text(text)).await.is_err() {
                        break end_1006();
                    }
                }
                Some(Outbound::Close) => {
                    let _ = ws.send(Message::Close(Some(CloseFrame { code: CloseCode::Normal, reason: "".into() }))).await;
                    if gave_up.is_none() {
                        let (tx, rx) = oneshot::channel();
                        clock.schedule(Box::new(move || {
                            let _ = tx.send(());
                        }), CLOSE_GRACE_MS);
                        gave_up = Some(rx);
                    }
                }
                None => outbound_open = false,
            },
            _ = async { match gave_up.as_mut() { Some(rx) => drop(rx.await), None => std::future::pending::<()>().await } } => break end_1006(),
        }
    };
    link.end.send_replace(Some(end));
    let waiters = std::mem::take(&mut *link.pending.borrow_mut());
    for (_, waiter) in waiters {
        let _ = waiter.send(Err(unanswered("connection lost")));
    }
}

fn route(link: &Link, text: &str) {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        link.deliver(value);
    }
}

/// Dials the door and returns the open socket, or the refusal the door answered with. `wss` is TLS under the web's
/// roots, as `ws` opens it.
async fn dial(ws_url: &str) -> Result<WebSocketStream<Shared>, ChannelError> {
    let parsed = url::Url::parse(ws_url).map_err(|e| ChannelError::Refused { message: e.to_string() })?;
    let addrs = parsed.socket_addrs(|| parsed.port_or_known_default()).map_err(|_| ended_before_open(end_1006()))?;
    let tcp = TcpStream::connect(&addrs[..]).await.map_err(|_| ended_before_open(end_1006()))?;
    let wire = if parsed.scheme() == "wss" { Wire::Tls(Box::new(tls_over(&parsed, tcp).await?)) } else { Wire::Plain(tcp) };
    let sock = Shared(Rc::new(RefCell::new(wire)));
    let request = ws_url.into_client_request().map_err(|e| ChannelError::Refused { message: e.to_string() })?;
    match client_async_with_config(request, sock.clone(), Some(socket_config())).await {
        Ok((ws, _)) => Ok(ws),
        Err(WsError::Http(response)) => {
            let response = *response;
            let (parts, tail) = response.into_parts();
            let chunked = parts
                .headers
                .get_all("transfer-encoding")
                .iter()
                .any(|v| v.to_str().is_ok_and(|v| v.to_ascii_lowercase().contains("chunked")));
            let length = parts.headers.get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<usize>().ok());
            let framing = if chunked { Framing::Chunked } else { length.map_or(Framing::UntilClose, Framing::Length) };
            let body = door_body(tail.unwrap_or_default(), framing, sock).await;
            Err(ChannelError::Door(DaemonDoorError::new(parts.status.as_u16(), &body)))
        }
        Err(_) => Err(ended_before_open(end_1006())),
    }
}

/// The TLS session over `tcp` for the host the url names; a handshake that fails reads as a socket that never opened.
async fn tls_over(url: &url::Url, tcp: TcpStream) -> Result<tokio_rustls::client::TlsStream<TcpStream>, ChannelError> {
    let name = match url.host() {
        Some(url::Host::Domain(domain)) => ServerName::try_from(domain.to_string()).ok(),
        Some(url::Host::Ipv4(ip)) => Some(ServerName::IpAddress(ip.into())),
        Some(url::Host::Ipv6(ip)) => Some(ServerName::IpAddress(ip.into())),
        None => None,
    };
    let name = name.ok_or_else(|| ChannelError::Refused { message: format!("no host to dial in {url}") })?;
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map(|b| b.with_root_certificates(RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() }).with_no_client_auth())
        .map_err(|_| ended_before_open(end_1006()))?;
    TlsConnector::from(Arc::new(config)).connect(name, tcp).await.map_err(|_| ended_before_open(end_1006()))
}

/// How the door's body ends: node's http parser reads these three the same way.
enum Framing {
    Chunked,
    Length(usize),
    UntilClose,
}

/// The door's body up to the cap, as the http parser would give it: what the handshake buffered, then reads until the
/// body ends where its framing says or the cap is full.
async fn door_body(tail: Vec<u8>, framing: Framing, mut sock: Shared) -> String {
    let mut dechunk = Dechunk::default();
    let mut body = Vec::new();
    let mut input = tail;
    loop {
        match framing {
            Framing::Chunked => {
                dechunk.feed(&input);
                body = dechunk.body.clone();
                if dechunk.done {
                    break;
                }
            }
            Framing::Length(want) => {
                body.extend_from_slice(&input);
                if body.len() >= want {
                    body.truncate(want);
                    break;
                }
            }
            Framing::UntilClose => body.extend_from_slice(&input),
        }
        if utf16_len(&body) >= DOOR_BODY_CAP {
            break;
        }
        let mut chunk = [0u8; 512];
        match sock.read(&mut chunk).await {
            Ok(n) if n > 0 => input = chunk[..n].to_vec(),
            _ => break,
        }
    }
    first_units(&String::from_utf8_lossy(&body), DOOR_BODY_CAP).to_string()
}

/// A chunked body decoded as it arrives: size line, that many bytes, a CRLF, and the zero chunk that ends it.
#[derive(Default)]
struct Dechunk {
    raw: Vec<u8>,
    body: Vec<u8>,
    left: Option<usize>,
    done: bool,
}

impl Dechunk {
    fn feed(&mut self, bytes: &[u8]) {
        self.raw.extend_from_slice(bytes);
        while !self.done {
            match self.left {
                None => {
                    let Some(end) = self.raw.iter().position(|&b| b == b'\n') else { return };
                    let line: Vec<u8> = self.raw.drain(..=end).collect();
                    let size = String::from_utf8_lossy(&line);
                    let size = size.split(';').next().unwrap_or_default().trim();
                    match usize::from_str_radix(size, 16) {
                        Ok(0) | Err(_) => self.done = true,
                        Ok(n) => self.left = Some(n),
                    }
                }
                Some(0) => {
                    if self.raw.len() < 2 {
                        return;
                    }
                    self.raw.drain(..2);
                    self.left = None;
                }
                Some(n) => {
                    if self.raw.is_empty() {
                        return;
                    }
                    let take = n.min(self.raw.len());
                    self.body.extend(self.raw.drain(..take));
                    self.left = Some(n - take);
                }
            }
        }
    }
}

/// The socket as the dial opened it, kept on this side too so a refusing door's body can be read after the handshake
/// gave the stream up.
enum Wire {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

#[derive(Clone)]
struct Shared(Rc<RefCell<Wire>>);

impl AsyncRead for Wire {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Wire::Plain(s) => Pin::new(s).poll_read(cx, buf),
            Wire::Tls(s) => Pin::new(s.as_mut()).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Wire {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Wire::Plain(s) => Pin::new(s).poll_write(cx, buf),
            Wire::Tls(s) => Pin::new(s.as_mut()).poll_write(cx, buf),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Wire::Plain(s) => Pin::new(s).poll_flush(cx),
            Wire::Tls(s) => Pin::new(s.as_mut()).poll_flush(cx),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Wire::Plain(s) => Pin::new(s).poll_shutdown(cx),
            Wire::Tls(s) => Pin::new(s.as_mut()).poll_shutdown(cx),
        }
    }
}

impl AsyncRead for Shared {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.0.borrow_mut()).poll_read(cx, buf)
    }
}

impl AsyncWrite for Shared {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.0.borrow_mut()).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.0.borrow_mut()).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.0.borrow_mut()).poll_shutdown(cx)
    }
}

fn unanswered(message: &str) -> ChannelError {
    ChannelError::Unanswered { message: message.into(), cause: None }
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

/// A socket that ended before it opened: a refused token reads as the token's error, anything else as the link's.
fn ended_before_open(end: ChannelEnd) -> ChannelError {
    if daemon_refused_token(f64::from(end.code), &end.reason) {
        ChannelError::Token(DaemonTokenError::new(&end.reason))
    } else {
        ChannelError::Unanswered { message: "the link to the computer ended before it opened".into(), cause: Some(end) }
    }
}

fn refusal_words(error: Option<&Value>) -> String {
    match error {
        None | Some(Value::Null) => "the machine refused the daemon link".into(),
        Some(v) => js_string(v),
    }
}

/// JavaScript's `String(value)` for what a daemon sends as its `error`.
pub(crate) fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        Value::Array(items) => items.iter().map(|i| if i.is_null() { String::new() } else { js_string(i) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
        other => other.to_string(),
    }
}

/// The id of a reply: a JSON number equal to a whole number from 0 up (`2.0` is 2, `"2"` is no id).
pub(crate) fn reply_id(v: &Value) -> Option<u64> {
    let n = v.get("id")?.as_f64()?;
    (n >= 0.0 && n.fract() == 0.0 && n <= u64::MAX as f64).then_some(n as u64)
}

fn with_id(mut frame: Frame, id: u64) -> String {
    frame.insert("id".to_string(), json!(id));
    Value::Object(frame).to_string()
}

fn utf16_len(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes).encode_utf16().count()
}

/// The first `cap` UTF-16 units of `text`, cut on a character.
fn first_units(text: &str, cap: usize) -> &str {
    let mut units = 0;
    for (at, c) in text.char_indices() {
        units += c.len_utf16();
        if units > cap {
            return &text[..at];
        }
    }
    text
}
