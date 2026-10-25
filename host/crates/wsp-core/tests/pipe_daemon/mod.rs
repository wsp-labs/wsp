// SPDX-License-Identifier: AGPL-3.0-only
//! The fake daemon of packages/runtime/test/pipe.test.ts (`fakeDaemon`), a websocket server on tokio-tungstenite, and
//! the polling helper `until`, shared by tests/pipe_pool.rs and tests/pipe_area.rs. Single-threaded: every task is
//! `spawn_local`, so it runs inside the test's `LocalSet`.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use futures::stream::SplitStream;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, watch, Notify};
use tokio::task::{spawn_local, AbortHandle};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};
use wsp_wire::types::daemon_contract::{DAEMON_TOKEN_ROTATED, PIPE_POOL_FULL};

pub use tokio_tungstenite::tungstenite::Bytes;

pub const TOKEN: &str = "pipe-token";

/// Polls `cond` every 10 ms until it holds; fails once `ms` passes (until.ts).
pub async fn until_for(ms: u64, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_millis(ms);
    while !cond() {
        assert!(Instant::now() <= deadline, "condition not met in time");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// `until` with its 2000 ms default.
pub async fn until(cond: impl FnMut() -> bool) {
    until_for(2000, cond).await;
}

pub async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// Races `fut` against `ms`; `None` when the time ran out first.
pub async fn within<T>(ms: u64, fut: impl std::future::Future<Output = T>) -> Option<T> {
    tokio::time::timeout(Duration::from_millis(ms), fut).await.ok()
}

pub fn sha_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut v);
    v
}

/// What the daemon answers a pipe's auth frame with: `Old` answers the socket as a link, with its hello; `Refuse`
/// refuses the token; `Deny` answers `ok: false`; `Grab` takes every pipe the moment it is authed; `Full` refuses it as
/// past its parked pipes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Pipes,
    Old,
    Refuse,
    Deny,
    Grab,
    Full,
}

enum Cmd {
    Send(Message, Option<oneshot::Sender<()>>),
    Close(u16, String),
}

/// One socket the daemon holds, and the daemon's end of a pipe once it is taken (`Far`).
pub struct Sock {
    cmd: mpsc::UnboundedSender<Cmd>,
    paused: watch::Sender<bool>,
    closed: watch::Sender<bool>,
    authed_with: RefCell<Option<String>>,
    frames: RefCell<VecDeque<Vec<u8>>>,
    keep: Cell<bool>,
    total: Cell<usize>,
    sha: RefCell<Sha256>,
    arrived: Notify,
    tasks: RefCell<Vec<AbortHandle>>,
}

pub type Far = Rc<Sock>;

impl Sock {
    /// Sends a binary frame; the answer completes once the frame is flushed to the socket, which a peer that reads
    /// nothing holds back.
    pub fn send_bytes(&self, data: Bytes) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd.send(Cmd::Send(Message::Binary(data), Some(tx)));
        rx
    }

    pub fn send(&self, data: Vec<u8>) -> oneshot::Receiver<()> {
        self.send_bytes(Bytes::from(data))
    }

    pub fn send_text(&self, text: String) {
        let _ = self.cmd.send(Cmd::Send(Message::text(text), None));
    }

    /// Closes with a code and no reason, as `ws.close(code)`.
    pub fn close(&self, code: u16) {
        self.close_with(code, "");
    }

    pub fn close_with(&self, code: u16, reason: &str) {
        let _ = self.cmd.send(Cmd::Close(code, reason.to_string()));
    }

    /// Cuts the socket without a close frame, as `ws.terminate()`.
    pub fn terminate(&self) {
        for h in self.tasks.borrow().iter() {
            h.abort();
        }
        self.closed.send_replace(true);
    }

    /// Stops reading the socket: what the peer sends piles up in the kernel's buffers.
    pub fn pause(&self) {
        self.paused.send_replace(true);
    }

    pub fn resume(&self) {
        self.paused.send_replace(false);
    }

    pub fn is_open(&self) -> bool {
        !*self.closed.borrow()
    }

    /// Completes once the socket has closed, by either end.
    pub fn closed(&self) -> impl std::future::Future<Output = ()> + 'static {
        let mut rx = self.closed.subscribe();
        async move {
            let _ = rx.wait_for(|c| *c).await;
        }
    }

    /// The next binary frame this end receives.
    pub async fn next_frame(&self) -> Vec<u8> {
        loop {
            let frame = self.frames.borrow_mut().pop_front();
            if let Some(frame) = frame {
                return frame;
            }
            self.arrived.notified().await;
        }
    }

    /// Every binary byte received so far.
    pub fn received(&self) -> usize {
        self.total.get()
    }

    /// The sha256, in hex, of every binary byte received so far.
    pub fn digest_hex(&self) -> String {
        hex::encode(self.sha.borrow().clone().finalize())
    }

    /// Stops keeping frames for `next_frame`; the count and the digest go on.
    pub fn discard(&self) {
        self.keep.set(false);
        self.frames.borrow_mut().clear();
    }

    fn record(&self, bytes: &[u8]) {
        self.total.set(self.total.get() + bytes.len());
        self.sha.borrow_mut().update(bytes);
        if self.keep.get() {
            self.frames.borrow_mut().push_back(bytes.to_vec());
        }
        self.arrived.notify_one();
    }
}

struct State {
    mode: Mode,
    file: String,
    parked: Vec<Far>,
    taken: Vec<Far>,
    first_frames: Vec<Value>,
    clients: Vec<Far>,
    stop: Option<oneshot::Sender<()>>,
    stopped: Option<oneshot::Receiver<()>>,
}

struct Inner {
    port: u16,
    state: RefCell<State>,
}

/// A daemon that answers a pipe's auth frame as a link's and parks it, and hands one over when the test plays a local
/// connection arriving at a door.
#[derive(Clone)]
pub struct Daemon {
    inner: Rc<Inner>,
}

struct End {
    inner: Rc<Inner>,
    sock: Far,
}

impl Drop for End {
    fn drop(&mut self) {
        {
            let mut s = self.inner.state.borrow_mut();
            s.clients.retain(|c| !Rc::ptr_eq(c, &self.sock));
            s.parked.retain(|c| !Rc::ptr_eq(c, &self.sock));
        }
        self.sock.authed_with.borrow_mut().take();
        self.sock.closed.send_replace(true);
        for h in self.sock.tasks.borrow().iter() {
            h.abort();
        }
    }
}

async fn accept_loop(inner: Rc<Inner>, listener: TcpListener, mut stop: oneshot::Receiver<()>, stopped: oneshot::Sender<()>) {
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                if let Ok((tcp, _)) = accepted {
                    spawn_local(serve(inner.clone(), tcp));
                }
            }
            _ = &mut stop => break,
        }
    }
    drop(listener);
    let _ = stopped.send(());
}

async fn serve(inner: Rc<Inner>, tcp: TcpStream) {
    let Ok(ws) = accept_async(tcp).await else { return };
    let (mut sink, stream) = ws.split();
    let (cmd, mut cmd_rx) = mpsc::unbounded_channel::<Cmd>();
    let sock: Far = Rc::new(Sock {
        cmd,
        paused: watch::channel(false).0,
        closed: watch::channel(false).0,
        authed_with: RefCell::new(None),
        frames: RefCell::new(VecDeque::new()),
        keep: Cell::new(true),
        total: Cell::new(0),
        sha: RefCell::new(Sha256::new()),
        arrived: Notify::new(),
        tasks: RefCell::new(Vec::new()),
    });
    let writer = spawn_local(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                Cmd::Send(message, ack) => {
                    let _ = sink.send(message).await;
                    if let Some(ack) = ack {
                        let _ = ack.send(());
                    }
                }
                Cmd::Close(code, reason) => {
                    let frame = CloseFrame { code: CloseCode::from(code), reason: reason.into() };
                    let _ = sink.send(Message::Close(Some(frame))).await;
                }
            }
        }
    });
    let reader = spawn_local(read_side(inner.clone(), sock.clone(), stream));
    sock.tasks.borrow_mut().extend([writer.abort_handle(), reader.abort_handle()]);
    inner.state.borrow_mut().clients.push(sock);
}

async fn read_side(inner: Rc<Inner>, sock: Far, mut stream: SplitStream<WebSocketStream<TcpStream>>) {
    let _end = End { inner: inner.clone(), sock: sock.clone() };
    let first = loop {
        match stream.next().await {
            Some(Ok(Message::Text(text))) => break text.as_str().to_string(),
            Some(Ok(_)) => continue,
            _ => return,
        }
    };
    let frame: Value = serde_json::from_str(&first).expect("the first frame is JSON");
    let (mode, file) = {
        let mut s = inner.state.borrow_mut();
        s.first_frames.push(frame.clone());
        (s.mode, s.file.clone())
    };
    if frame.get("token").and_then(Value::as_str) != Some(file.as_str()) || mode == Mode::Refuse {
        sock.close_with(4401, "unauthorized");
    } else if mode == Mode::Deny {
        sock.send_text(json!({ "id": 0, "ok": false, "error": "unauthorized" }).to_string());
    } else if mode == Mode::Full {
        sock.close_with(1008, PIPE_POOL_FULL);
    } else {
        sock.send_text(json!({ "id": 0, "ok": true }).to_string());
        *sock.authed_with.borrow_mut() = Some(file);
        match mode {
            Mode::Old => sock.send_text(json!({ "type": "daemon.hello", "root": "/root", "version": 157 }).to_string()),
            Mode::Grab => {
                inner.state.borrow_mut().taken.push(sock.clone());
                sock.send_text(json!({ "op": "pipe.open", "door": "http", "heldMs": 0 }).to_string());
            }
            _ => inner.state.borrow_mut().parked.push(sock.clone()),
        }
    }
    let mut paused = sock.paused.subscribe();
    loop {
        if *paused.borrow_and_update() {
            if paused.changed().await.is_err() {
                break;
            }
            continue;
        }
        tokio::select! {
            message = stream.next() => match message {
                Some(Ok(Message::Binary(bytes))) => sock.record(&bytes),
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            _ = paused.changed() => {}
        }
    }
}

impl Daemon {
    pub async fn start(first: Mode) -> Daemon {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind a port");
        let port = listener.local_addr().expect("a local address").port();
        let inner = Rc::new(Inner {
            port,
            state: RefCell::new(State {
                mode: first,
                file: TOKEN.to_string(),
                parked: Vec::new(),
                taken: Vec::new(),
                first_frames: Vec::new(),
                clients: Vec::new(),
                stop: None,
                stopped: None,
            }),
        });
        let d = Daemon { inner };
        d.listen(listener);
        d
    }

    fn listen(&self, listener: TcpListener) {
        let (stop_tx, stop_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        {
            let mut s = self.inner.state.borrow_mut();
            s.stop = Some(stop_tx);
            s.stopped = Some(done_rx);
        }
        spawn_local(accept_loop(self.inner.clone(), listener, stop_rx, done_tx));
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.inner.port)
    }

    /// The pipes parked and waiting for a connection, oldest first.
    pub fn parked(&self) -> Vec<Far> {
        self.inner.state.borrow().parked.clone()
    }

    pub fn parked_len(&self) -> usize {
        self.inner.state.borrow().parked.len()
    }

    /// Every pipe the daemon took, still open or not.
    pub fn taken(&self) -> Vec<Far> {
        self.inner.state.borrow().taken.clone()
    }

    pub fn first_frames(&self) -> Vec<Value> {
        self.inner.state.borrow().first_frames.clone()
    }

    pub fn first_frame_count(&self) -> usize {
        self.inner.state.borrow().first_frames.len()
    }

    /// Sockets the daemon holds open, of any kind.
    pub fn open(&self) -> usize {
        self.inner.state.borrow().clients.iter().filter(|c| c.is_open()).count()
    }

    /// Another writer puts a token in the daemon's file: every socket authed with other bytes is cut, as the daemon
    /// cuts them.
    pub fn write(&self, token: &str) {
        let clients = {
            let mut s = self.inner.state.borrow_mut();
            s.file = token.to_string();
            s.clients.clone()
        };
        for c in clients {
            let held = c.authed_with.borrow().clone();
            if held.is_some_and(|h| h != token) {
                c.close_with(4401, DAEMON_TOKEN_ROTATED);
            }
        }
    }

    /// The token the daemon's file holds now.
    pub fn file(&self) -> String {
        self.inner.state.borrow().file.clone()
    }

    /// The daemon replaced by one of another kind, as an update does (the TS `become`).
    pub fn become_mode(&self, mode: Mode) {
        self.inner.state.borrow_mut().mode = mode;
    }

    /// Plays a local connection arriving at a door: the oldest parked pipe gets its `pipe.open`.
    pub fn take(&self, door: &str, held_ms: u64) -> Far {
        let far = {
            let mut s = self.inner.state.borrow_mut();
            assert!(!s.parked.is_empty(), "no pipe parked");
            let far = s.parked.remove(0);
            s.taken.push(far.clone());
            far
        };
        far.send_text(json!({ "op": "pipe.open", "door": door, "heldMs": held_ms }).to_string());
        far
    }

    /// The machine falls off the network: every socket cut, nothing listening (the TS `drop`).
    pub async fn drop_network(&self) {
        let (stop, stopped, clients) = {
            let mut s = self.inner.state.borrow_mut();
            (s.stop.take(), s.stopped.take(), s.clients.clone())
        };
        for c in &clients {
            c.terminate();
        }
        {
            let mut s = self.inner.state.borrow_mut();
            s.clients.clear();
            s.parked.clear();
        }
        if let Some(stop) = stop {
            let _ = stop.send(());
        }
        if let Some(stopped) = stopped {
            let _ = stopped.await;
        }
        sleep(10).await;
    }

    /// Listens again on the same port.
    pub async fn back(&self) {
        let listener = TcpListener::bind(("127.0.0.1", self.inner.port)).await.expect("bind the same port again");
        self.listen(listener);
    }

    pub async fn close(&self) {
        self.drop_network().await;
    }
}
