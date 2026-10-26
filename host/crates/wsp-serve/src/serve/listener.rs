// SPDX-License-Identifier: AGPL-3.0-only
//! The listener, the upgrades and each websocket's transport (serve.ts:375-492, 1899-1934). Spec: spec/serve_wire.md.
//! One task owns each connection's websocket: it reads frames, writes what the socket sends, and takes the socket
//! over for the place link once a place proves (`FrameSink::pause`, `FrameSink::link`).
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::sink::Sink;
use futures::{FutureExt, StreamExt};
use indexmap::IndexMap;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt, WriteHalf};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use tokio::time::Sleep;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
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
use super::wsframe::{
    binary_frame, check_upgrade, close_frame, is_upgrade, ping_frame, pong_frame, refusal_bytes, switching, text_frame, Accepted, Fault,
    Head, Inbound, Receiver,
};
use super::{Authed, RuntimeServer, UpgradeHead, UpgradeIo, STOP_GRACE_MS};

const STOPPING: &str = "stopping";
const ACCEPT_RETRY_MS: u64 = 10;
/// ws's `closeTimeout`: how long after its close frame a peer that never answers is waited on (spec 04's limits).
const CLOSE_TIMEOUT_MS: u64 = 30_000;
/// A request head past this is no upgrade; node's own limit is 16 KiB.
const MAX_HEAD: usize = 16 * 1024;
const READ_CHUNK: usize = 64 * 1024;
const BAD_REQUEST: &str = "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n";

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
    mut io: Box<dyn UpgradeIo>,
) {
    let Some((upgrade, first)) = handshake(shared, &mut rx, arrival, &mut io).await else {
        return;
    };
    let sink = Rc::new(WsSink { out: tx, open: Cell::new(true), seq: Cell::new(0), listeners: RefCell::new(IndexMap::new()) });
    let dyn_sink: Rc<dyn FrameSink> = sink.clone();
    let socket = Socket::opened(&shared.served, upgrade, dyn_sink);
    if shared.stopping.get() {
        FrameSink::close(&*sink, HOST_STOPPING_CLOSE as u16, STOPPING);
    }
    pump(socket, sink, io, first, rx).await;
}

/// The handshake of one connection: a cut or a stop that comes first ends it before it is a socket. Answers the upgrade
/// and the bytes that came in after its head.
async fn handshake(
    shared: &Rc<Shared>,
    rx: &mut mpsc::UnboundedReceiver<Out>,
    arrival: Arrival,
    io: &mut Box<dyn UpgradeIo>,
) -> Option<(UpgradeHead, Vec<u8>)> {
    match arrival {
        Arrival::Own(peer) => own(shared, rx, io, peer).await,
        Arrival::Attached(upgrade) => attached(shared, io, upgrade).await.map(|upgrade| (upgrade, Vec::new())),
    }
}

/// An upgrade on the runtime's own port, read here as node's HTTP server and `ws` read it (serve.ts:470): the checks
/// `handleUpgrade` makes, then `verifyClient`'s origin (401), then the 101. Whatever the client sent after the head
/// is its first frames.
async fn own(
    shared: &Rc<Shared>,
    rx: &mut mpsc::UnboundedReceiver<Out>,
    io: &mut Box<dyn UpgradeIo>,
    peer: SocketAddr,
) -> Option<(UpgradeHead, Vec<u8>)> {
    let (head, first) = read_head(rx, io).await?;
    if !is_upgrade(&head.headers) {
        refuse(io, &plain_refusal()).await;
        return None;
    }
    let accepted = match check_upgrade(&head.method, &head.headers) {
        Ok(accepted) => accepted,
        Err(refusal) => {
            refuse(io, &refusal.bytes()).await;
            return None;
        }
    };
    let upgrade = UpgradeHead { path_and_query: head.target, headers: head.headers, peer: Some(peer) };
    if !origin_allowed(&shared.served, &upgrade) {
        refuse(io, &refusal_bytes(401, "Unauthorized", &[])).await;
        return None;
    }
    open(io, &accepted).await?;
    Some((upgrade, first))
}

/// Reads a request's head off the connection; a request that is no request is answered 400.
async fn read_head(rx: &mut mpsc::UnboundedReceiver<Out>, io: &mut Box<dyn UpgradeIo>) -> Option<(Head, Vec<u8>)> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match Head::read(&buf) {
            Ok(Some((head, took))) => return Some((head, buf.split_off(took))),
            Ok(None) if buf.len() <= MAX_HEAD => {}
            _ => {
                refuse(io, BAD_REQUEST.as_bytes()).await;
                return None;
            }
        }
        tokio::select! {
            read = io.read(&mut chunk) => match read {
                Ok(0) | Err(_) => return None,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            },
            cmd = rx.recv() => {
                if matches!(cmd, None | Some(Out::Cut)) {
                    return None;
                }
            }
        }
    }
}

/// An upgrade the host read: origin, then path, then `handleUpgrade`'s checks (the method is the host's, which has
/// refused a method other than GET before it hands the upgrade over), then the 101 this side writes itself
/// (serve.ts:473-491).
async fn attached(shared: &Rc<Shared>, io: &mut Box<dyn UpgradeIo>, upgrade: UpgradeHead) -> Option<UpgradeHead> {
    if !origin_allowed(&shared.served, &upgrade) {
        refuse(io, "HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n".as_bytes()).await;
        return None;
    }
    if !Url::parse(&format!("ws://localhost{}", upgrade.path_and_query)).is_ok_and(|url| url.path() == WS_PATH) {
        refuse(io, BAD_REQUEST.as_bytes()).await;
        return None;
    }
    let accepted = match check_upgrade("GET", &upgrade.headers) {
        Ok(accepted) => accepted,
        Err(refusal) => {
            refuse(io, &refusal.bytes()).await;
            return None;
        }
    };
    open(io, &accepted).await?;
    Some(upgrade)
}

/// Writes the 101.
async fn open(io: &mut Box<dyn UpgradeIo>, accepted: &Accepted) -> Option<()> {
    let accept = derive_accept_key(accepted.key.as_bytes());
    io.write_all(&switching(&accept, accepted.protocol.as_deref())).await.ok()
}

/// Writes a refusal and ends the connection; a peer that already went cannot end the process, so write errors are read
/// as nothing.
async fn refuse(io: &mut Box<dyn UpgradeIo>, raw: &[u8]) {
    let _ = io.write_all(raw).await;
    let _ = io.shutdown().await;
}

/// What ws's own HTTP server answers a request that is no upgrade.
fn plain_refusal() -> Vec<u8> {
    let body = "Upgrade Required";
    format!(
        "HTTP/1.1 426 Upgrade Required\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// Why a connection's loop ended.
#[derive(Clone, Copy, PartialEq)]
enum Exit {
    /// The close handshake is done: this side ends the connection (`socket.end()`).
    Graceful,
    /// Cut, a wire fault, a peer that went, a close nobody answered (`socket.destroy()`).
    Terminate,
}

/// What a connection's loop holds beside its bytes.
struct Flow {
    receiver: Receiver,
    /// What the receiver read and the loop has not handed on: held while the socket is paused for a place link.
    backlog: VecDeque<Result<Inbound, Fault>>,
    /// Bytes that came in and are not yet counted against the pre-auth cap: counted once the frames of what came in
    /// are handled, as ws's receiver handles a chunk before the door's counter hears of it (serve.ts:579-603).
    counted: usize,
    paused: bool,
    linked: Option<mpsc::UnboundedSender<Result<Message, WsError>>>,
    ended: Option<(u16, String)>,
    close_sent: bool,
    exit: Option<Exit>,
}

type CloseTimer = Option<Pin<Box<Sleep>>>;

impl Flow {
    fn new() -> Flow {
        Flow {
            receiver: Receiver::new(),
            backlog: VecDeque::new(),
            counted: 0,
            paused: false,
            linked: None,
            ended: None,
            close_sent: false,
            exit: None,
        }
    }

    /// `readyState === OPEN`: no close frame sent and none received.
    fn open(&self) -> bool {
        !self.close_sent && self.ended.is_none()
    }

    /// `ws.close(code, reason)`: one close frame, and the timer that cuts a peer that never answers it.
    fn close(&mut self, code: Option<u16>, reason: &str, out: &mut Vec<u8>, timer: &mut CloseTimer) {
        if self.close_sent {
            return;
        }
        self.close_sent = true;
        out.extend(close_frame(code, reason));
        *timer = Some(Box::pin(tokio::time::sleep(Duration::from_millis(CLOSE_TIMEOUT_MS))));
    }

    fn apply(&mut self, cmd: Out, out: &mut Vec<u8>, timer: &mut CloseTimer) {
        match cmd {
            Out::Msg(message) => match message {
                Message::Close(frame) => {
                    let (code, reason) = frame.map_or((None, String::new()), |f| (Some(u16::from(f.code)), f.reason.to_string()));
                    self.close(code, &reason, out, timer);
                }
                _ if self.close_sent => {}
                Message::Text(text) => out.extend(text_frame(&text)),
                Message::Binary(bytes) => out.extend(binary_frame(&bytes)),
                Message::Ping(bytes) => out.extend(ping_frame(&bytes)),
                Message::Pong(bytes) => out.extend(pong_frame(&bytes)),
                Message::Frame(_) => {}
            },
            Out::Close(code, reason) => self.close(Some(code), &reason, out, timer),
            Out::Cut => self.exit = Some(Exit::Terminate),
            Out::Pause => self.paused = true,
            Out::Link(inbound) => {
                self.linked = Some(inbound);
                self.paused = false;
            }
        }
    }

    /// What the socket and its sink have sent the loop so far, in order.
    fn commands(&mut self, rx: &mut mpsc::UnboundedReceiver<Out>, out: &mut Vec<u8>, timer: &mut CloseTimer) {
        while let Ok(cmd) = rx.try_recv() {
            self.apply(cmd, out, timer);
        }
    }

    /// Hands on what the receiver read, one at a time, with what each handler said sent before the next is handled,
    /// then counts what came in.
    async fn drain(&mut self, socket: &Rc<Socket>, rx: &mut mpsc::UnboundedReceiver<Out>, out: &mut Vec<u8>, timer: &mut CloseTimer) {
        self.commands(rx, out, timer);
        while self.exit.is_none() && !self.paused {
            let Some(item) = self.backlog.pop_front() else { break };
            self.handle(item, socket, out, timer).await;
            self.commands(rx, out, timer);
        }
        if self.exit.is_none() && self.backlog.is_empty() && self.counted > 0 {
            socket.bytes_arrived(std::mem::take(&mut self.counted));
            self.commands(rx, out, timer);
        }
    }

    async fn handle(&mut self, item: Result<Inbound, Fault>, socket: &Rc<Socket>, out: &mut Vec<u8>, timer: &mut CloseTimer) {
        match item {
            Ok(Inbound::Text(text)) => match &self.linked {
                Some(inbound) => {
                    let _ = inbound.send(Ok(Message::Text(text.into())));
                }
                None => inline(socket.on_message(Incoming::Text(text))).await,
            },
            Ok(Inbound::Binary(bytes)) => match &self.linked {
                Some(inbound) => {
                    let _ = inbound.send(Ok(Message::Binary(bytes.into())));
                }
                None => inline(socket.on_message(Incoming::Binary(bytes))).await,
            },
            Ok(Inbound::Ping(bytes)) => {
                if self.open() {
                    out.extend(pong_frame(&bytes));
                }
                if let Some(inbound) = &self.linked {
                    let _ = inbound.send(Ok(Message::Ping(bytes.into())));
                }
            }
            Ok(Inbound::Pong(bytes)) => {
                if let Some(inbound) = &self.linked {
                    let _ = inbound.send(Ok(Message::Pong(bytes.into())));
                }
            }
            Ok(Inbound::Close(code, reason)) => {
                if let Some(inbound) = &self.linked {
                    let frame = (code != 1005).then(|| CloseFrame { code: CloseCode::from(code), reason: reason.clone().into() });
                    let _ = inbound.send(Ok(Message::Close(frame)));
                }
                self.ended = Some((code, reason.clone()));
                if code == 1005 {
                    self.close(None, "", out, timer);
                } else {
                    self.close(Some(code), &reason, out, timer);
                }
                self.exit = Some(Exit::Graceful);
            }
            Err(fault) => {
                warn(&format!("socket from {} dropped on a wire fault: {}", socket.from, fault.sentence));
                if let Some(inbound) = &self.linked {
                    let _ = inbound.send(Err(WsError::Io(io::Error::new(io::ErrorKind::InvalidData, fault.sentence.clone()))));
                }
                self.close(Some(fault.code), "", out, timer);
                self.exit = Some(Exit::Terminate);
            }
        }
    }
}

/// Runs a frame's handler now, as ws emits its message synchronously, and leaves what it awaits to its own task.
async fn inline(handler: LocalBoxFuture<'static, ()>) {
    let mut handler = handler;
    if std::future::poll_fn(|cx| Poll::Ready(handler.as_mut().poll(cx))).await.is_pending() {
        tokio::task::spawn_local(handler);
    }
}

/// Writes what the connection takes at once, as `socket.write` does; the rest stays queued.
fn flush_now(wr: &mut WriteHalf<Box<dyn UpgradeIo>>, out: &mut Vec<u8>) -> io::Result<()> {
    let mut cx = Context::from_waker(Waker::noop());
    while !out.is_empty() {
        match Pin::new(&mut *wr).poll_write(&mut cx, out) {
            Poll::Ready(Ok(0)) => return Err(io::ErrorKind::WriteZero.into()),
            Poll::Ready(Ok(n)) => {
                out.drain(..n);
            }
            Poll::Ready(Err(error)) => return Err(error),
            Poll::Pending => return Ok(()),
        }
    }
    let _ = Pin::new(&mut *wr).poll_flush(&mut cx);
    Ok(())
}

/// Carries one connection's frames until it ends, then tells its socket and its close listeners how it ended.
async fn pump(socket: Rc<Socket>, sink: Rc<WsSink>, io: Box<dyn UpgradeIo>, first: Vec<u8>, mut rx: mpsc::UnboundedReceiver<Out>) {
    let (mut rd, mut wr) = tokio::io::split(io);
    let mut flow = Flow::new();
    let mut out: Vec<u8> = Vec::new();
    let mut timer: CloseTimer = None;
    let mut chunk = vec![0u8; READ_CHUNK];
    if !first.is_empty() {
        flow.counted += first.len();
        flow.backlog.extend(flow.receiver.push(&first));
    }
    loop {
        flow.drain(&socket, &mut rx, &mut out, &mut timer).await;
        if flush_now(&mut wr, &mut out).is_err() {
            flow.exit = Some(Exit::Terminate);
        }
        if flow.exit.is_some() {
            break;
        }
        tokio::select! {
            biased;
            cmd = rx.recv() => match cmd {
                Some(cmd) => flow.apply(cmd, &mut out, &mut timer),
                None => flow.exit = Some(Exit::Terminate),
            },
            written = wr.write(&out), if !out.is_empty() => match written {
                Ok(n) if n > 0 => {
                    out.drain(..n);
                }
                _ => flow.exit = Some(Exit::Terminate),
            },
            () = async { timer.as_mut().expect("a timer is set").await }, if timer.is_some() => flow.exit = Some(Exit::Terminate),
            read = rd.read(&mut chunk), if !flow.paused => match read {
                Ok(n) if n > 0 => {
                    flow.counted += n;
                    flow.backlog.extend(flow.receiver.push(&chunk[..n]));
                }
                _ => flow.exit = Some(Exit::Terminate),
            },
        }
    }
    let _ = flush_now(&mut wr, &mut out);
    if flow.exit == Some(Exit::Graceful) {
        let _ = wr.shutdown().now_or_never();
    }
    drop((rd, wr));
    sink.open.set(false);
    socket.closed();
    let (code, reason) = flow.ended.unwrap_or((1006, String::new()));
    let leaves: Vec<_> = sink.listeners.borrow_mut().drain(..).map(|(_, leave)| leave).collect();
    for leave in leaves {
        leave(code, reason.clone());
    }
}

fn origin_allowed(served: &Served, upgrade: &UpgradeHead) -> bool {
    served.opts.origin_allowed.as_ref().is_none_or(|rule| rule(upgrade))
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
