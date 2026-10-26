// SPDX-License-Identifier: AGPL-3.0-only
//! The listener, the upgrades and each websocket's transport (serve.ts:375-492, 1899-1934). Spec: spec/serve_wire.md.
//! One task owns each connection's websocket: it reads frames, writes what the socket sends, and takes the socket
//! over for the place link once a place proves (`FrameSink::pause`, `FrameSink::link`).
use std::cell::{Cell, RefCell};
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;

use async_trait::async_trait;
use futures::sink::Sink;
use futures::{SinkExt, StreamExt};
use indexmap::IndexMap;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::error::ProtocolError;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Role};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};
use url::Url;

use wsp_core::console::warn;
use wsp_core::reach::{connect_daemon, ReachOptions, TakenSocket};
use wsp_machines::places::helpers::LinkSocket;
use wsp_store::keys::Seal;
use wsp_wire::types::app_ports::WS_PATH;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::limits::HOST_STOPPING_CLOSE;
use wsp_words::exit::HostError;

use super::door::Served;
use super::socket::{FrameSink, Incoming, Socket};
use super::{Authed, RuntimeServer, UpgradeHead, UpgradeIo, STOP_GRACE_MS};

const STOPPING: &str = "stopping";
const ACCEPT_RETRY_MS: u64 = 10;

/// What a connection's task is told: a frame to send, a close, a cut, the pause before a place link, the link's
/// inbound frames.
enum Out {
    Msg(Message),
    Close(u16, String),
    Cut,
    Pause,
    Link(mpsc::UnboundedSender<Result<Message, WsError>>),
}

/// What every connection of one server shares. A connection is registered the moment it arrives, so a stop reaches
/// one still in its handshake.
struct Shared {
    served: Rc<Served>,
    conns: RefCell<IndexMap<u64, mpsc::UnboundedSender<Out>>>,
    next: Cell<u64>,
    stopping: Cell<bool>,
    /// Told when the last connection leaves while a stop waits on them.
    idle: Notify,
}

/// The runtime's server: its own listener where it was given a port, and the upgrades the host hands it.
pub struct Wire {
    shared: Rc<Shared>,
    port: u16,
    accept: RefCell<Option<JoinHandle<()>>>,
}

/// Where an upgraded connection came from: the runtime's own port, or an upgrade the host read and handed over.
enum Arrival {
    Own(SocketAddr),
    Attached(UpgradeHead),
}

type Ws = WebSocketStream<Counting>;
type Tap = Rc<RefCell<Option<Rc<Socket>>>>;
/// A close listener, told the code and reason the socket ended with.
type Leave = Box<dyn FnOnce(u16, String)>;

impl Wire {
    /// Binds `bind` when given and starts answering its upgrades (serve.ts:470-476, 1901-1911).
    pub async fn start(served: Rc<Served>, bind: Option<(String, u16)>) -> Result<Wire, HostError> {
        let shared = Rc::new(Shared {
            served,
            conns: RefCell::new(IndexMap::new()),
            next: Cell::new(0),
            stopping: Cell::new(false),
            idle: Notify::new(),
        });
        let (port, accept) = match bind {
            None => (0, None),
            Some((host, port)) => {
                let listener = TcpListener::bind((host.as_str(), port)).await.map_err(|e| HostError::new(e.to_string()))?;
                let bound = listener.local_addr().map_err(|e| HostError::new(e.to_string()))?.port();
                let accepting = shared.clone();
                let handle = tokio::task::spawn_local(async move {
                    loop {
                        match listener.accept().await {
                            Ok((tcp, peer)) => spawn_conn(&accepting, Arrival::Own(peer), Box::new(tcp)),
                            Err(_) => tokio::time::sleep(Duration::from_millis(ACCEPT_RETRY_MS)).await,
                        }
                    }
                });
                (bound, Some(handle))
            }
        };
        Ok(Wire { shared, port, accept: RefCell::new(accept) })
    }
}

#[async_trait(?Send)]
impl RuntimeServer for Wire {
    fn port(&self) -> u16 {
        self.port
    }

    async fn upgrade(&self, head: UpgradeHead, io: Box<dyn UpgradeIo>) {
        spawn_conn(&self.shared, Arrival::Attached(head), io);
    }

    async fn authorize(&self, token: Option<&str>) -> Option<Authed> {
        match token {
            None | Some("") => None,
            Some(token) => self.shared.served.who_is(token).await.ok().flatten(),
        }
    }

    async fn revoke_device(&self, id: &str) -> Result<bool, HostError> {
        self.shared.served.revoke_device(id, None).await
    }

    async fn close(&self) -> Result<(), HostError> {
        let shared = &self.shared;
        shared.stopping.set(true);
        for tx in shared.conns.borrow().values() {
            let _ = tx.send(Out::Close(HOST_STOPPING_CLOSE as u16, STOPPING.to_string()));
        }
        idle(shared, Duration::from_millis(STOP_GRACE_MS)).await;
        for tx in shared.conns.borrow().values() {
            let _ = tx.send(Out::Cut);
        }
        idle(shared, Duration::from_millis(STOP_GRACE_MS)).await;
        let accepting = self.accept.borrow_mut().take();
        if let Some(handle) = accepting {
            handle.abort();
            let _ = handle.await;
        }
        Ok(())
    }
}

/// Waits until no connection is left, or the grace runs out.
async fn idle(shared: &Shared, grace: Duration) {
    let _ = tokio::time::timeout(grace, async {
        loop {
            if shared.conns.borrow().is_empty() {
                break;
            }
            shared.idle.notified().await;
        }
    })
    .await;
}

/// Registers one upgraded connection and runs it on its own task. An upgrade the host hands over during a stop is
/// dropped unanswered, as the TS removes its upgrade listener then.
fn spawn_conn(shared: &Rc<Shared>, arrival: Arrival, io: Box<dyn UpgradeIo>) {
    if shared.stopping.get() && matches!(arrival, Arrival::Attached(_)) {
        return;
    }
    let id = shared.next.get();
    shared.next.set(id + 1);
    let (tx, rx) = mpsc::unbounded_channel();
    shared.conns.borrow_mut().insert(id, tx.clone());
    let shared = shared.clone();
    tokio::task::spawn_local(async move {
        connection(&shared, tx, rx, arrival, io).await;
        shared.conns.borrow_mut().shift_remove(&id);
        if shared.stopping.get() && shared.conns.borrow().is_empty() {
            shared.idle.notify_one();
        }
    });
}

/// One connection from its handshake to its end. The socket is let in by `Socket::opened`, which may refuse it.
async fn connection(
    shared: &Rc<Shared>,
    tx: mpsc::UnboundedSender<Out>,
    mut rx: mpsc::UnboundedReceiver<Out>,
    arrival: Arrival,
    io: Box<dyn UpgradeIo>,
) {
    let tap: Tap = Rc::new(RefCell::new(None));
    let io = Counting { io, tap: tap.clone() };
    let Some((ws, upgrade)) = handshake(shared, &mut rx, arrival, io).await else {
        return;
    };
    let sink = Rc::new(WsSink { out: tx, open: Cell::new(true), seq: Cell::new(0), listeners: RefCell::new(IndexMap::new()) });
    let dyn_sink: Rc<dyn FrameSink> = sink.clone();
    let socket = Socket::opened(&shared.served, upgrade, dyn_sink);
    *tap.borrow_mut() = Some(socket.clone());
    if shared.stopping.get() {
        FrameSink::close(&*sink, HOST_STOPPING_CLOSE as u16, STOPPING);
    }
    pump(socket, sink, ws, rx).await;
}

/// The handshake of one connection: a cut or a stop that comes first ends it before it is a socket.
async fn handshake(
    shared: &Rc<Shared>,
    rx: &mut mpsc::UnboundedReceiver<Out>,
    arrival: Arrival,
    io: Counting,
) -> Option<(Ws, UpgradeHead)> {
    match arrival {
        Arrival::Own(peer) => {
            let slot: Rc<RefCell<Option<UpgradeHead>>> = Rc::new(RefCell::new(None));
            let read = slot.clone();
            let served = shared.served.clone();
            #[allow(clippy::result_large_err)]
            let callback = move |req: &Request, reply: Response| -> Result<Response, ErrorResponse> {
                let upgrade = head_of(req, peer);
                if !origin_allowed(&served, &upgrade) {
                    let mut refused = ErrorResponse::new(None);
                    *refused.status_mut() = StatusCode::UNAUTHORIZED;
                    return Err(refused);
                }
                *read.borrow_mut() = Some(upgrade);
                Ok(reply)
            };
            let accepting = accept_hdr_async(io, callback);
            tokio::pin!(accepting);
            let ws = loop {
                tokio::select! {
                    res = &mut accepting => break res.ok()?,
                    cmd = rx.recv() => {
                        if matches!(cmd, None | Some(Out::Cut)) {
                            return None;
                        }
                    }
                }
            };
            let upgrade = slot.borrow_mut().take()?;
            Some((ws, upgrade))
        }
        Arrival::Attached(upgrade) => attached(shared, io, upgrade).await,
    }
}

/// An upgrade the host read: origin, then path, then the 101 this side writes itself (serve.ts:473-491).
async fn attached(shared: &Rc<Shared>, mut io: Counting, upgrade: UpgradeHead) -> Option<(Ws, UpgradeHead)> {
    if !origin_allowed(&shared.served, &upgrade) {
        refuse(&mut io, "HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n").await;
        return None;
    }
    let path_ok = Url::parse(&format!("ws://localhost{}", upgrade.path_and_query)).is_ok_and(|url| url.path() == WS_PATH);
    let accept = header(&upgrade, "sec-websocket-key").map(|key| derive_accept_key(key.as_bytes()));
    let (true, Some(accept)) = (path_ok, accept) else {
        refuse(&mut io, "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n").await;
        return None;
    };
    let reply =
        format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n");
    io.write_all(reply.as_bytes()).await.ok()?;
    let ws = WebSocketStream::from_raw_socket(io, Role::Server, None).await;
    Some((ws, upgrade))
}

/// Writes a refusal and ends the connection; a peer that already went cannot end the process, so write errors are read
/// as nothing.
async fn refuse(io: &mut Counting, raw: &str) {
    let _ = io.write_all(raw.as_bytes()).await;
    let _ = io.shutdown().await;
}

/// Carries one connection's frames until it ends, then tells its socket and its close listeners how it ended.
async fn pump(socket: Rc<Socket>, sink: Rc<WsSink>, mut ws: Ws, mut rx: mpsc::UnboundedReceiver<Out>) {
    let mut paused = false;
    let mut linked: Option<mpsc::UnboundedSender<Result<Message, WsError>>> = None;
    let mut ended: Option<(u16, String)> = None;
    loop {
        tokio::select! {
            biased;
            cmd = rx.recv() => {
                match cmd {
                    None | Some(Out::Cut) => break,
                    Some(Out::Msg(message)) => {
                        let _ = ws.send(message).await;
                    }
                    Some(Out::Close(code, reason)) => {
                        let frame = CloseFrame { code: CloseCode::from(code), reason: reason.into() };
                        let _ = ws.send(Message::Close(Some(frame))).await;
                    }
                    Some(Out::Pause) => paused = true,
                    Some(Out::Link(inbound)) => {
                        linked = Some(inbound);
                        paused = false;
                    }
                }
            }
            item = ws.next(), if !paused => {
                match item {
                    Some(Ok(message)) => {
                        let closing = matches!(message, Message::Close(_));
                        if let Message::Close(frame) = &message {
                            ended = Some(frame.as_ref().map_or((1005, String::new()), |f| (u16::from(f.code), f.reason.to_string())));
                        }
                        match &linked {
                            Some(inbound) => {
                                let _ = inbound.send(Ok(message));
                            }
                            None => match message {
                                Message::Text(text) => {
                                    tokio::task::spawn_local(socket.on_message(Incoming::Text(text.to_string())));
                                }
                                Message::Binary(bytes) => {
                                    tokio::task::spawn_local(socket.on_message(Incoming::Binary(bytes.to_vec())));
                                }
                                _ => {}
                            },
                        }
                        if closing {
                            let _ = ws.flush().await;
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        if let Some(sentence) = wire_fault(&error) {
                            warn(&format!("socket from {} dropped on a wire fault: {sentence}", socket.from));
                        }
                        if let Some(inbound) = &linked {
                            let _ = inbound.send(Err(error));
                        }
                        break;
                    }
                    None => break,
                }
            }
        }
    }
    sink.open.set(false);
    socket.closed();
    let (code, reason) = ended.unwrap_or((1006, String::new()));
    let leaves: Vec<_> = sink.listeners.borrow_mut().drain(..).map(|(_, leave)| leave).collect();
    for leave in leaves {
        leave(code, reason.clone());
    }
}

/// The sentence a wire fault is warned with. A peer that vanishes without its close frame is a close, not a fault.
fn wire_fault(error: &WsError) -> Option<String> {
    match error {
        WsError::Protocol(ProtocolError::UnmaskedFrameFromClient) => Some("Invalid WebSocket frame: MASK must be set".to_string()),
        WsError::Protocol(ProtocolError::ResetWithoutClosingHandshake) => None,
        WsError::Protocol(_) | WsError::Capacity(_) | WsError::Utf8(_) => Some(error.to_string()),
        _ => None,
    }
}

fn origin_allowed(served: &Served, upgrade: &UpgradeHead) -> bool {
    served.opts.origin_allowed.as_ref().is_none_or(|rule| rule(upgrade))
}

fn head_of(req: &Request, peer: SocketAddr) -> UpgradeHead {
    UpgradeHead {
        path_and_query: req.uri().path_and_query().map_or("/", |pq| pq.as_str()).to_string(),
        headers: req
            .headers()
            .iter()
            .filter_map(|(name, value)| Some((name.as_str().to_string(), value.to_str().ok()?.to_string())))
            .collect(),
        peer: Some(peer),
    }
}

fn header<'a>(upgrade: &'a UpgradeHead, name: &str) -> Option<&'a str> {
    upgrade.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
}

/// One connection's websocket as a socket's sink. Every write goes through its task; `open` is what the socket reads
/// as `readyState === OPEN`.
struct WsSink {
    out: mpsc::UnboundedSender<Out>,
    open: Cell<bool>,
    seq: Cell<u64>,
    listeners: RefCell<IndexMap<u64, Leave>>,
}

impl WsSink {
    fn shut(&self, code: u16, reason: &str) {
        if self.open.replace(false) {
            let _ = self.out.send(Out::Close(code, reason.to_string()));
        }
    }
}

impl FrameSink for WsSink {
    fn is_open(&self) -> bool {
        self.open.get()
    }
    fn send_text(&self, text: String) {
        let _ = self.out.send(Out::Msg(Message::Text(text.into())));
    }
    fn send_binary(&self, bytes: Vec<u8>) {
        let _ = self.out.send(Out::Msg(Message::Binary(bytes.into())));
    }
    fn close(&self, code: u16, reason: &str) {
        self.shut(code, reason);
    }
    fn terminate(&self) {
        self.open.set(false);
        let _ = self.out.send(Out::Cut);
    }
    fn pause(&self) {
        let _ = self.out.send(Out::Pause);
    }
    fn link(self: Rc<Self>) -> Rc<dyn LinkSocket> {
        self
    }
}

impl LinkSocket for WsSink {
    fn is_open(&self) -> bool {
        self.open.get()
    }
    fn close(&self, code: u16, reason: &str) {
        self.shut(code, reason);
    }
    fn on_close(&self, f: Box<dyn FnOnce(u16, String)>) -> u64 {
        let id = self.seq.get() + 1;
        self.seq.set(id);
        self.listeners.borrow_mut().insert(id, f);
        id
    }
    fn off_close(&self, id: u64) {
        self.listeners.borrow_mut().shift_remove(&id);
    }
    fn connect(self: Rc<Self>, seal: Option<Seal>, on_event: Rc<dyn Fn(&DaemonEvent)>) -> Rc<dyn wsp_core::reach::DaemonReach> {
        let (inbound, mut received) = mpsc::unbounded_channel::<Result<Message, WsError>>();
        let _ = self.out.send(Out::Link(inbound));
        let taken = TakenSocket {
            sink: Box::pin(LinkOut(self.out.clone())),
            stream: futures::stream::poll_fn(move |cx| received.poll_recv(cx)).boxed_local(),
        };
        let on_event: Rc<dyn Fn(DaemonEvent)> = Rc::new(move |event: DaemonEvent| on_event(&event));
        let opts = ReachOptions {
            preview_url: None,
            socket: Some(taken),
            token: None,
            on_event,
            on_status: None,
            heartbeat_ms: None,
            seal,
            backoff_ms: None,
            clock: None,
            ids: None,
        };
        connect_daemon(opts).expect("a taken socket is never refused")
    }
}

/// The link's writes, sent through the connection's task.
struct LinkOut(mpsc::UnboundedSender<Out>);

impl Sink<Message> for LinkOut {
    type Error = WsError;
    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), WsError>> {
        Poll::Ready(Ok(()))
    }
    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), WsError> {
        self.0.send(Out::Msg(item)).map_err(|_| WsError::ConnectionClosed)
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), WsError>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), WsError>> {
        Poll::Ready(Ok(()))
    }
}

/// The bytes a connection reads after its handshake are counted for its socket before the websocket library reads
/// them (spec D4). Nothing is counted until the socket exists.
struct Counting {
    io: Box<dyn UpgradeIo>,
    tap: Tap,
}

impl AsyncRead for Counting {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        let polled = Pin::new(&mut this.io).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = polled {
            let read = buf.filled().len() - before;
            let socket = this.tap.borrow().clone();
            if let (Some(socket), true) = (socket, read > 0) {
                socket.bytes_arrived(read);
            }
        }
        polled
    }
}

impl AsyncWrite for Counting {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().io).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_shutdown(cx)
    }
}
