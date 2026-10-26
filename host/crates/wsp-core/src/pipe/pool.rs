// SPDX-License-Identifier: AGPL-3.0-only
//! The host's pool of pipes to each brokered machine's daemon: extra websockets the daemon parks until a process on
//! the machine connects to one of its doors, each then carrying that one connection's bytes. Port of
//! packages/runtime/src/pipe.ts:1-392 at 65f6b89c7. Spec: spec/pipe_pool.md.
//!
//! Host state on the owner thread (rule 3.1): `Rc`, `RefCell` and `spawn_local`, and no borrow held across an
//! `.await` or a call out (rule 3.3).
use std::cell::{Cell, RefCell};
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{ready, Context, Poll};

use futures::future::LocalBoxFuture;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use indexmap::IndexMap;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::task::spawn_local;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use wsp_wire::types::daemon_contract::PIPE_POOL_MAX;
use wsp_wire::types::req_id::ReqId;
use wsp_wire::wire::daemon::{parse_daemon_response, pipe_open_of};
pub use wsp_wire::wire::daemon::{PipeDoor, PipeOpen};
use wsp_wire::wire::daemon_version::PIPE_DAEMON_VERSION;
use wsp_wire::wire::limits::link_backoff_ms;
use wsp_wire::wire::machine_link::DaemonResponse;
use wsp_words::daemon_contract::daemon_refused_token;
use wsp_words::js::js_round;

use crate::clock::{real_clock, Cancel, Clock};
use crate::console;
use crate::ids::{Ids, OsIds};
use crate::reach::daemon_ws_url;

/// Idle pipes kept dialled per brokered workspace whose link is up.
pub const PIPE_IDLE: usize = 4;
/// A pipe idle this long is replaced.
pub const PIPE_IDLE_MAX_MS: u64 = 10 * 60_000;
/// The bound on one dial.
pub const PIPE_DIAL_MS: u64 = 15_000;
/// How long before a daemon too old for pipes is asked its version again.
pub const PIPE_BEHIND_AGAIN_MS: u64 = 60_000;
/// The most pipes dialled to one workspace in any second.
pub const PIPE_DIAL_RATE: usize = 4;
/// The longest wait between two writes of the host's token to a machine whose daemon keeps refusing it.
pub const TOKEN_REFUSED_MAX_MS: u64 = 30 * 60_000;

/// One road's count of a daemon refusing the host's token.
// TS pipe.ts:32-48
// /** One road's count of a daemon refusing the host's token. The host writes its own token there again and dials after
//  * PIPE_BEHIND_AGAIN_MS, and twice as long for each refusal that follows its own write: two writers of one file cut each
//  * other's sockets, and backing off is what keeps that from happening every minute. */
// export function tokenRefusals(firstMs = PIPE_BEHIND_AGAIN_MS): { refused(now: number): { count: number; waitMs: number }; wrote(now: number): void } {
//   let count = 0;
//   let wroteAt: number | undefined;
//   return {
//     refused(now) {
//       count = wroteAt !== undefined && now - wroteAt < 2 * TOKEN_REFUSED_MAX_MS ? count + 1 : 1;
//       wroteAt = undefined;
//       return { count, waitMs: Math.min(firstMs * 2 ** (count - 1), TOKEN_REFUSED_MAX_MS) };
//     },
//     wrote(now) {
//       wroteAt = now;
//     },
//   };
// }
#[derive(Clone, Debug, PartialEq)]
pub struct TokenRefusals {
    first_ms: u64,
    count: u32,
    wrote_at: Option<i64>,
}

impl TokenRefusals {
    /// Answers the refusals in a row and the wait before the next dial.
    pub fn refused(&mut self, now: i64) -> (u32, u64) {
        self.count = match self.wrote_at {
            Some(wrote) if now - wrote < 2 * TOKEN_REFUSED_MAX_MS as i64 => self.count + 1,
            _ => 1,
        };
        self.wrote_at = None;
        let doubled = self.first_ms.saturating_mul(1u64 << (self.count - 1).min(40));
        (self.count, doubled.min(TOKEN_REFUSED_MAX_MS))
    }
    pub fn wrote(&mut self, now: i64) {
        self.wrote_at = Some(now);
    }
}

/// `first_ms` is `PIPE_BEHIND_AGAIN_MS` at every TS call site but the pool's own, which passes `behind_again_ms`.
pub fn token_refusals(first_ms: u64) -> TokenRefusals {
    TokenRefusals { first_ms, count: 0, wrote_at: None }
}

/// `<n> s` under two minutes, else `<n> min`, each rounded as `Math.round` does.
// TS pipe.ts:50-50
// const waitWords = (ms: number): string => (ms < 120_000 ? `${Math.round(ms / 1000)} s` : `${Math.round(ms / 60_000)} min`);
pub fn wait_words(ms: u64) -> String {
    if ms < 120_000 {
        format!("{} s", js_round(ms as f64 / 1000.0) as i64)
    } else {
        format!("{} min", js_round(ms as f64 / 60_000.0) as i64)
    }
}

/// What a road says the count'th time in a row a daemon refuses the host's token; `None` past the second.
// TS pipe.ts:52-59
// /** What a road says the count'th time in a row a daemon refuses the host's token: the first refusal and the first one
//  * after the host's own write, and nothing after that until a refusal stops following a write. */
// export function tokenRefusedLine(who: string, road: string, count: number, waitMs: number): string | undefined {
//   if (count === 1) return `${who}: its daemon refused the token ${road}, so the host writes its own there again and dials in ${waitWords(waitMs)}`;
//   if (count === 2)
//     return `${who}: its daemon refused the host's token again right after the host wrote it, so something else writes that file too; the host writes its own there again and dials in ${waitWords(waitMs)}, waiting twice as long each time up to ${waitWords(TOKEN_REFUSED_MAX_MS)}`;
//   return undefined;
// }
pub fn token_refused_line(who: &str, road: &str, count: u32, wait_ms: u64) -> Option<String> {
    match count {
        1 => Some(format!("{who}: its daemon refused the token {road}, so the host writes its own there again and dials in {}", wait_words(wait_ms))),
        2 => Some(format!(
            "{who}: its daemon refused the host's token again right after the host wrote it, so something else writes that file too; the host writes its own there again and dials in {}, waiting twice as long each time up to {}",
            wait_words(wait_ms),
            wait_words(TOKEN_REFUSED_MAX_MS)
        )),
        _ => None,
    }
}

/// A taken pipe's bytes, both ways, over the socket's binary frames.
pub trait PipeIo: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> PipeIo for T {}
pub type PipeDuplex = Box<dyn PipeIo>;

/// Where each taken pipe goes: the workspace whose link it was dialled on. An `Err` is the TS handler's throw.
// TS pipe.ts:61-62
// /** Where each taken pipe goes: the workspace whose link it was dialled on, never anything the machine said. */
// export type PipeHandler = (workspaceId: string, open: PipeOpen, duplex: Duplex) => void;
pub type PipeHandler = Rc<dyn Fn(&str, PipeOpen, PipeDuplex) -> Result<(), String>>;

// TS pipe.ts:64-69
// export interface PipeReach {
//   /** The daemon's route, as the link dials it. */
//   url: string;
//   /** The daemon's own token, sent in the first frame and never in the url. */
//   token: string;
// }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeReach {
    /// The daemon's route, as the link dials it.
    pub url: String,
    /// The daemon's own token, sent in the first frame and never in the url.
    pub token: String,
}

// TS pipe.ts:71-89
// export interface PipePoolOptions {
//   /** The link's own reach, read at each dial; undefined while the guest holds no daemon token. */
//   reach(workspaceId: string): Promise<PipeReach | undefined>;
//   /** The version the workspace's daemon announces, null when it does not answer: pipes are dialled only to one at or
//    * above PIPE_DAEMON_VERSION. */
//   version(workspaceId: string): Promise<number | null>;
//   /** Read at each take, so a handler wired after the pool started still gets every pipe. */
//   handler(): PipeHandler | undefined;
//   /** The switch: nothing is dialled while it reads false. */
//   ready(): boolean;
//   /** The daemon refused the token: the next reach writes the host's token on the machine again. */
//   rewrite(workspaceId: string): void;
//   clock?: Clock;
//   backoffMs?: (attempt: number) => number;
//   idleMaxMs?: number;
//   behindAgainMs?: number;
//   dialRate?: number;
//   log?: (line: string) => void;
// }
#[derive(Clone)]
pub struct PipePoolOptions {
    /// The link's own reach, read at each dial; `Ok(None)` while the guest holds no daemon token; `Err` fails the dial.
    pub reach: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<Option<PipeReach>, String>>>,
    /// The daemon's version, `Ok(None)` when it does not answer; an `Err` reads as `None`.
    pub version: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<Option<i64>, String>>>,
    /// Read at each take.
    pub handler: Rc<dyn Fn() -> Option<PipeHandler>>,
    /// The switch: nothing is dialled while it reads false.
    pub ready: Rc<dyn Fn() -> bool>,
    /// The daemon refused the token: the next reach writes the host's token on the machine again.
    pub rewrite: Rc<dyn Fn(&str)>,
    /// `real_clock()` when `None`.
    pub clock: Option<Rc<dyn Clock>>,
    /// `link_backoff_ms` with a real random draw when `None`.
    pub backoff_ms: Option<Rc<dyn Fn(u32) -> u64>>,
    pub idle_max_ms: Option<u64>,
    pub behind_again_ms: Option<u64>,
    pub dial_rate: Option<usize>,
    /// `crate::console::warn` when `None`.
    pub log: Option<Rc<dyn Fn(&str)>>,
}

// TS pipe.ts:91-99
// export interface PipePool {
//   /** The workspace's link is up: keep PIPE_IDLE pipes dialled to it. */
//   up(workspaceId: string): void;
//   /** The link stopped: every pipe of the workspace closes, the taken ones too, and nothing is dialled again until up. */
//   down(workspaceId: string): void;
//   /** Pipes open and waiting for the daemon to take one. */
//   idle(workspaceId: string): number;
//   close(): void;
// }
pub struct PipePool {
    core: Rc<Core>,
}

impl PipePool {
    /// The workspace's link is up: keep `PIPE_IDLE` pipes dialled to it.
    pub fn up(&self, workspace_id: &str) {
        if self.core.held.borrow().contains_key(workspace_id) {
            return;
        }
        let h = Rc::new(Held::new(workspace_id));
        self.core.held.borrow_mut().insert(workspace_id.to_string(), h.clone());
        fill(&self.core, &h);
    }
    /// The link stopped: every pipe of the workspace closes, taken ones too, and nothing is dialled until `up`.
    pub fn down(&self, workspace_id: &str) {
        down(&self.core, workspace_id);
    }
    /// Pipes open and waiting for the daemon to take one.
    pub fn idle(&self, workspace_id: &str) -> usize {
        let held = self.core.held.borrow().get(workspace_id).cloned();
        held.map_or(0, |h| h.idle.borrow().iter().filter(|pipe| pipe.open.get()).count())
    }
    pub fn close(&self) {
        let ids: Vec<String> = self.core.held.borrow().keys().cloned().collect();
        for id in ids {
            down(&self.core, &id);
        }
    }
}

/// The pool's state (TS `held`) and its options, shared by every dial and timer of the pool.
struct Core {
    reach: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<Option<PipeReach>, String>>>,
    version: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<Option<i64>, String>>>,
    handler: Rc<dyn Fn() -> Option<PipeHandler>>,
    ready: Rc<dyn Fn() -> bool>,
    rewrite: Rc<dyn Fn(&str)>,
    clock: Rc<dyn Clock>,
    backoff: Rc<dyn Fn(u32) -> u64>,
    idle_max_ms: u64,
    behind_again_ms: u64,
    dial_rate: usize,
    log: Rc<dyn Fn(&str)>,
    held: RefCell<IndexMap<String, Rc<Held>>>,
}

/// One workspace's held pool (TS `Held`).
struct Held {
    id: String,
    idle: RefCell<Vec<Rc<Pipe>>>,
    taken: RefCell<Vec<Rc<Pipe>>>,
    attempt: Cell<u32>,
    dialled: RefCell<Vec<i64>>,
    stopped: Cell<bool>,
    retry: RefCell<Option<Cancel>>,
    pace: RefCell<Option<Cancel>>,
    refusals: RefCell<TokenRefusals>,
    serves: Cell<bool>,
    asking: Cell<bool>,
    said_behind: Cell<bool>,
    said_dialling: Cell<bool>,
    said_full: Cell<bool>,
}

impl Held {
    fn new(id: &str) -> Self {
        Held {
            id: id.to_string(),
            idle: RefCell::new(Vec::new()),
            taken: RefCell::new(Vec::new()),
            attempt: Cell::new(0),
            dialled: RefCell::new(Vec::new()),
            stopped: Cell::new(false),
            retry: RefCell::new(None),
            pace: RefCell::new(None),
            refusals: RefCell::new(token_refusals(PIPE_BEHIND_AGAIN_MS)),
            serves: Cell::new(false),
            asking: Cell::new(false),
            said_behind: Cell::new(false),
            said_dialling: Cell::new(false),
            said_full: Cell::new(false),
        }
    }
}

/// What a dial's task is told to do: end at once, or send the daemon a 1000 close first.
enum Ctl {
    Terminate,
    Retire,
}

/// One dial of a workspace's pool (TS `Pipe`): idle until a `pipe.open` frame takes it, then carried by a taken task.
struct Pipe {
    open: Cell<bool>,
    authed: Cell<bool>,
    done: Cell<bool>,
    cancel_idle: RefCell<Option<Cancel>>,
    dial_timer: RefCell<Option<Cancel>>,
    ctl: RefCell<Option<oneshot::Sender<Ctl>>>,
}

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type SinkSlot = Arc<Mutex<Option<SplitSink<WsStream, Message>>>>;
/// Frames a taken pipe holds for its handler before the daemon's reads wait.
const TAKEN_FRAMES: usize = 8;

/// A taken pipe's bytes as the handler's duplex: reads come from the taken task, writes go to the socket's sink.
struct PipeSocket {
    frames: mpsc::Receiver<Vec<u8>>,
    buffered: Vec<u8>,
    at: usize,
    sink: SinkSlot,
    dropped: Option<oneshot::Sender<()>>,
}

impl AsyncRead for PipeSocket {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        loop {
            if self.at < self.buffered.len() {
                let (from, to) = (self.at, (self.at + buf.remaining()).min(self.buffered.len()));
                buf.put_slice(&self.buffered[from..to]);
                self.at = to;
                return Poll::Ready(Ok(()));
            }
            match ready!(self.frames.poll_recv(cx)) {
                Some(frame) => {
                    self.buffered = frame;
                    self.at = 0;
                }
                None => return Poll::Ready(Ok(())),
            }
        }
    }
}

impl AsyncWrite for PipeSocket {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let sink = self.sink.clone();
        let mut slot = sink.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(sink) = slot.as_mut() else { return Poll::Ready(Err(io::Error::from(io::ErrorKind::BrokenPipe))) };
        // The frame before this one is on the wire before another is queued, so a daemon that reads nothing holds the writer.
        ready!(sink.poll_flush_unpin(cx)).map_err(io::Error::other)?;
        ready!(sink.poll_ready_unpin(cx)).map_err(io::Error::other)?;
        sink.start_send_unpin(Message::Binary(buf.to_vec().into())).map_err(io::Error::other)?;
        Poll::Ready(Ok(buf.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let sink = self.sink.clone();
        let mut slot = sink.lock().unwrap_or_else(PoisonError::into_inner);
        match slot.as_mut() {
            Some(sink) => sink.poll_flush_unpin(cx).map_err(io::Error::other),
            None => Poll::Ready(Ok(())),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let sink = self.sink.clone();
        let mut slot = sink.lock().unwrap_or_else(PoisonError::into_inner);
        match slot.as_mut() {
            Some(sink) => sink.poll_close_unpin(cx).map_err(io::Error::other),
            None => Poll::Ready(Ok(())),
        }
    }
}

impl Drop for PipeSocket {
    fn drop(&mut self) {
        if let Some(dropped) = self.dropped.take() {
            let _ = dropped.send(());
        }
    }
}

fn say(core: &Core, line: &str) {
    (core.log)(line);
}

fn send_ctl(pipe: &Pipe, ctl: Ctl) {
    let tx = pipe.ctl.borrow_mut().take();
    if let Some(tx) = tx {
        let _ = tx.send(ctl);
    }
}

fn cancel_timer(slot: &RefCell<Option<Cancel>>) {
    let cancel = slot.borrow_mut().take();
    if let Some(cancel) = cancel {
        cancel();
    }
}

/// Ends one dial: its timers stop and its task is told to terminate (TS `end`).
fn end(pipe: &Pipe) {
    pipe.done.set(true);
    cancel_timer(&pipe.cancel_idle);
    send_ctl(pipe, Ctl::Terminate);
}

fn end_idle(h: &Held) {
    let drained: Vec<Rc<Pipe>> = h.idle.borrow_mut().drain(..).collect();
    for pipe in drained {
        end(&pipe);
    }
}

fn again(core: &Rc<Core>, h: &Rc<Held>, ms: u64) {
    if h.retry.borrow().is_some() {
        return;
    }
    let (core2, h2) = (core.clone(), h.clone());
    let cancel = core.clock.schedule(
        Box::new(move || {
            *h2.retry.borrow_mut() = None;
            fill(&core2, &h2);
        }),
        ms as i64,
    );
    *h.retry.borrow_mut() = Some(cancel);
}

fn wait(core: &Rc<Core>, h: &Rc<Held>) {
    let attempt = h.attempt.get() + 1;
    h.attempt.set(attempt);
    again(core, h, (core.backoff)(attempt));
}

fn link_down(core: &Rc<Core>, h: &Rc<Held>) {
    if h.attempt.get() == 0 {
        say(
            core,
            &format!(
                "{}: a pipe could not be dialled, so its others were closed and the pool is dialled again as the link comes back",
                h.id
            ),
        );
    }
    end_idle(h);
    if !h.stopped.get() {
        wait(core, h);
    }
}

fn behind(core: &Rc<Core>, h: &Rc<Held>, line: &str) {
    if !h.said_behind.get() {
        say(core, line);
    }
    h.said_behind.set(true);
    h.serves.set(false);
    end_idle(h);
    again(core, h, core.behind_again_ms);
}

/// A dial that failed or closed while idle: the link is down, so the rest of the pool goes too (TS `failed`).
fn fail_dial(core: &Rc<Core>, h: &Rc<Held>, pipe: &Rc<Pipe>) {
    cancel_timer(&pipe.dial_timer);
    if pipe.done.get() {
        return;
    }
    end(pipe);
    h.idle.borrow_mut().retain(|p| !Rc::ptr_eq(p, pipe));
    if !h.stopped.get() {
        link_down(core, h);
    }
}

fn refused(core: &Rc<Core>, h: &Rc<Held>) {
    end_idle(h);
    let (count, wait_ms) = h.refusals.borrow_mut().refused(core.clock.now());
    if let Some(line) = token_refused_line(&h.id, "a pipe was dialled with", count, wait_ms) {
        say(core, &line);
    }
    (core.rewrite)(&h.id);
    cancel_timer(&h.retry);
    let (core2, h2) = (core.clone(), h.clone());
    let cancel = core.clock.schedule(
        Box::new(move || {
            *h2.retry.borrow_mut() = None;
            h2.refusals.borrow_mut().wrote(core2.clock.now());
            fill(&core2, &h2);
        }),
        wait_ms as i64,
    );
    *h.retry.borrow_mut() = Some(cancel);
}

fn retire(core: &Rc<Core>, h: &Rc<Held>, pipe: &Rc<Pipe>) {
    if pipe.done.get() {
        return;
    }
    h.idle.borrow_mut().retain(|p| !Rc::ptr_eq(p, pipe));
    pipe.done.set(true);
    send_ctl(pipe, Ctl::Retire);
    fill(core, h);
}

fn taken_closed(core: &Rc<Core>, h: &Rc<Held>, pipe: &Rc<Pipe>) {
    h.taken.borrow_mut().retain(|p| !Rc::ptr_eq(p, pipe));
    if h.taken.borrow().len() < PIPE_POOL_MAX as usize {
        h.said_full.set(false);
    }
    fill(core, h);
}

fn answered(core: &Rc<Core>, h: &Rc<Held>, version: Option<i64>) {
    h.asking.set(false);
    if h.stopped.get() {
        return;
    }
    match version {
        None => wait(core, h),
        Some(v) if v >= PIPE_DAEMON_VERSION => {
            h.serves.set(true);
            fill(core, h);
        }
        Some(v) => behind(core, h, &format!("{}: its daemon ({v}) serves no pipes yet, so it is asked again each minute", h.id)),
    }
}

/// Asks a daemon its version before any pipe is dialled to it (TS `ask`).
fn ask(core: &Rc<Core>, h: &Rc<Held>) {
    h.asking.set(true);
    let version = (core.version)(&h.id);
    let (core2, h2) = (core.clone(), h.clone());
    spawn_local(async move {
        let version = version.await.ok().flatten();
        answered(&core2, &h2, version);
    });
}

/// Dials up to `PIPE_IDLE` idle pipes inside `PIPE_POOL_MAX` sockets (TS `fill`).
fn fill(core: &Rc<Core>, h: &Rc<Held>) {
    if h.stopped.get() || h.asking.get() || h.retry.borrow().is_some() || h.pace.borrow().is_some() {
        return;
    }
    if !(core.ready)() {
        return wait(core, h);
    }
    if !h.serves.get() {
        return ask(core, h);
    }
    let max = PIPE_POOL_MAX as i64;
    let taken = h.taken.borrow().len() as i64;
    if taken >= max && !h.said_full.get() {
        h.said_full.set(true);
        say(core, &format!("{}: {max} pipes are in use, so the next connection waits for one to close", h.id));
    }
    let idle = h.idle.borrow().len() as i64;
    let short = (PIPE_IDLE as i64 - idle).min(max - taken - idle);
    if short <= 0 {
        return;
    }
    let now = core.clock.now();
    h.dialled.borrow_mut().retain(|&at| now - at < 1000);
    let dialled = h.dialled.borrow().len() as i64;
    let allowed = short.min(core.dial_rate as i64 - dialled);
    if !h.said_dialling.get() {
        say(core, &format!("{}: dialling {short} pipe{}, {PIPE_IDLE} kept idle", h.id, if short == 1 { "" } else { "s" }));
    }
    h.said_dialling.set(true);
    for _ in 0..allowed.max(0) {
        h.dialled.borrow_mut().push(now);
        dial(core, h);
    }
    if allowed < short {
        let oldest = h.dialled.borrow().first().copied().unwrap_or(now);
        let (core2, h2) = (core.clone(), h.clone());
        let cancel = core.clock.schedule(
            Box::new(move || {
                *h2.pace.borrow_mut() = None;
                fill(&core2, &h2);
            }),
            oldest + 1000 - now,
        );
        *h.pace.borrow_mut() = Some(cancel);
    }
}

/// Starts one dial: an idle entry, a timer that fails it, and a task that connects and authenticates (TS `dial`).
fn dial(core: &Rc<Core>, h: &Rc<Held>) {
    let pipe = Rc::new(Pipe {
        open: Cell::new(false),
        authed: Cell::new(false),
        done: Cell::new(false),
        cancel_idle: RefCell::new(None),
        dial_timer: RefCell::new(None),
        ctl: RefCell::new(None),
    });
    h.idle.borrow_mut().push(pipe.clone());
    let (ctl_tx, ctl_rx) = oneshot::channel();
    *pipe.ctl.borrow_mut() = Some(ctl_tx);
    let (core2, h2, pipe2) = (core.clone(), h.clone(), pipe.clone());
    let timer = core.clock.schedule(Box::new(move || fail_dial(&core2, &h2, &pipe2)), PIPE_DIAL_MS as i64);
    *pipe.dial_timer.borrow_mut() = Some(timer);
    let reach = (core.reach)(&h.id);
    spawn_local(dial_task(core.clone(), h.clone(), pipe, ctl_rx, reach));
}

async fn dial_task(
    core: Rc<Core>,
    h: Rc<Held>,
    pipe: Rc<Pipe>,
    mut ctl: oneshot::Receiver<Ctl>,
    reach: LocalBoxFuture<'static, Result<Option<PipeReach>, String>>,
) {
    let reach = reach.await;
    if pipe.done.get() {
        return;
    }
    let reach = match reach {
        Ok(Some(reach)) => reach,
        _ => return fail_dial(&core, &h, &pipe),
    };
    let Ok(url) = daemon_ws_url(&reach.url) else { return };
    let mut ws = tokio::select! {
        biased;
        _ = &mut ctl => return,
        dialled = connect_async(url) => match dialled {
            Ok((ws, _)) => ws,
            Err(_) => return fail_dial(&core, &h, &pipe),
        },
    };
    if pipe.done.get() {
        return;
    }
    cancel_timer(&pipe.dial_timer);
    pipe.open.set(true);
    if ws.send(Message::text(auth_frame(&reach.token))).await.is_err() {
        return fail_dial(&core, &h, &pipe);
    }
    let (core2, h2, pipe2) = (core.clone(), h.clone(), pipe.clone());
    let idle = core.clock.schedule(Box::new(move || retire(&core2, &h2, &pipe2)), core.idle_max_ms as i64);
    *pipe.cancel_idle.borrow_mut() = Some(idle);
    loop {
        let next = tokio::select! {
            biased;
            cmd = &mut ctl => {
                if let Ok(Ctl::Retire) = cmd {
                    let _ = ws.send(Message::Close(Some(CloseFrame { code: CloseCode::Normal, reason: "".into() }))).await;
                }
                return;
            }
            next = ws.next() => next,
        };
        match next {
            None | Some(Err(_)) => return fail_dial(&core, &h, &pipe),
            Some(Ok(Message::Close(frame))) => {
                if pipe.done.get() {
                    return;
                }
                let (code, reason) = match frame {
                    Some(frame) => (u16::from(frame.code), frame.reason.to_string()),
                    None => (1005, String::new()),
                };
                if daemon_refused_token(f64::from(code), &reason) {
                    pipe.done.set(true);
                    h.idle.borrow_mut().retain(|p| !Rc::ptr_eq(p, &pipe));
                    return refused(&core, &h);
                }
                return fail_dial(&core, &h, &pipe);
            }
            Some(Ok(Message::Text(text))) => match auth_answer(&text) {
                Some(true) => {
                    pipe.authed.set(true);
                    let authed = h.idle.borrow().iter().filter(|p| p.authed.get()).count();
                    if h.attempt.get() > 0 && authed == PIPE_IDLE {
                        h.attempt.set(0);
                        say(&core, &format!("{}: {PIPE_IDLE} pipes idle", h.id));
                    }
                }
                Some(false) => return refused(&core, &h),
                None => match pipe_open_of(&text) {
                    Some(open) => return take(&core, &h, pipe, ws, open, ctl),
                    None => behind(&core, &h, &format!("{}: its daemon answered a pipe as a link, so it is asked again each minute", h.id)),
                },
            },
            Some(Ok(Message::Binary(_))) => {
                behind(&core, &h, &format!("{}: its daemon answered a pipe as a link, so it is asked again each minute", h.id));
            }
            Some(Ok(_)) => {}
        }
    }
}

/// Moves a pipe a connection took out of idle, splits its socket and hands the duplex to the handler (TS `take`).
fn take(core: &Rc<Core>, h: &Rc<Held>, pipe: Rc<Pipe>, ws: WsStream, open: PipeOpen, ctl: oneshot::Receiver<Ctl>) {
    h.idle.borrow_mut().retain(|p| !Rc::ptr_eq(p, &pipe));
    pipe.done.set(true);
    cancel_timer(&pipe.cancel_idle);
    h.attempt.set(0);
    h.said_behind.set(false);
    h.taken.borrow_mut().push(pipe.clone());
    let (sink, stream) = ws.split();
    let (frames_tx, frames_rx) = mpsc::channel(TAKEN_FRAMES);
    let sink: SinkSlot = Arc::new(Mutex::new(Some(sink)));
    let (dropped_tx, dropped_rx) = oneshot::channel();
    let duplex = PipeSocket { frames: frames_rx, buffered: Vec::new(), at: 0, sink: sink.clone(), dropped: Some(dropped_tx) };
    spawn_local(taken_task(core.clone(), h.clone(), pipe.clone(), stream, frames_tx, sink, ctl, dropped_rx));
    let handler = (core.handler)();
    let room = h.taken.borrow().len() + h.idle.borrow().len() < PIPE_POOL_MAX as usize;
    let door = match open.door {
        PipeDoor::Http => "http",
        PipeDoor::Gh => "gh",
    };
    let held = open.held_ms.get();
    let held = if held > 0 { format!(" after {held} ms held") } else { String::new() };
    let replacement = if room { "dialling its replacement" } else { "its replacement waits for a pipe to close" };
    say(core, &format!("{}: a pipe opened on the {door} door{held}, {replacement}", h.id));
    match handler {
        None => {
            say(core, &format!("{}: nothing here serves pipes yet, so that connection was closed", h.id));
            drop(duplex);
        }
        Some(handler) => {
            if let Err(message) = handler(&h.id, open, Box::new(duplex)) {
                say(core, &format!("{}: the pipe's handler failed ({message})", h.id));
            }
        }
    }
    fill(core, h);
}

/// Carries a taken socket until the daemon closes it, the duplex drops, or the pool terminates it (TS `ws.once("close")`).
#[allow(clippy::too_many_arguments)]
async fn taken_task(
    core: Rc<Core>,
    h: Rc<Held>,
    pipe: Rc<Pipe>,
    mut stream: SplitStream<WsStream>,
    frames: mpsc::Sender<Vec<u8>>,
    sink: SinkSlot,
    mut ctl: oneshot::Receiver<Ctl>,
    mut dropped: oneshot::Receiver<()>,
) {
    loop {
        let permit = tokio::select! {
            biased;
            _ = &mut ctl => break,
            _ = &mut dropped => break,
            permit = frames.clone().reserve_owned() => match permit {
                Ok(permit) => permit,
                Err(_) => break,
            },
        };
        let next = tokio::select! {
            biased;
            _ = &mut ctl => break,
            _ = &mut dropped => break,
            next = stream.next() => next,
        };
        match next {
            Some(Ok(Message::Binary(bytes))) => {
                permit.send(bytes.to_vec());
            }
            Some(Ok(Message::Text(text))) => {
                permit.send(text.as_bytes().to_vec());
            }
            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
            Some(Ok(_)) => {}
        }
    }
    let socket = sink.lock().unwrap_or_else(PoisonError::into_inner).take();
    drop(socket);
    taken_closed(&core, &h, &pipe);
}

/// Stops one workspace's pool: its idle pipes end, its taken ones are terminated, and nothing is dialled until `up`
/// (TS `down`).
fn down(core: &Rc<Core>, id: &str) {
    let held = core.held.borrow_mut().shift_remove(id);
    let Some(h) = held else { return };
    h.stopped.set(true);
    cancel_timer(&h.retry);
    cancel_timer(&h.pace);
    end_idle(&h);
    let taken: Vec<Rc<Pipe>> = h.taken.borrow_mut().drain(..).collect();
    for pipe in taken {
        send_ctl(&pipe, Ctl::Terminate);
    }
}

// TS pipe.ts:146-392
// export function pipePool(o: PipePoolOptions): PipePool {
//   const clock = o.clock ?? realClock;
//   const backoff = o.backoffMs ?? linkBackoffMs;
//   const idleMaxMs = o.idleMaxMs ?? PIPE_IDLE_MAX_MS;
//   const behindAgainMs = o.behindAgainMs ?? PIPE_BEHIND_AGAIN_MS;
//   const dialRate = o.dialRate ?? PIPE_DIAL_RATE;
//   const log = o.log ?? ((line: string) => console.warn(line));
//   const held = new Map<string, Held>();
//
//   const end = (pipe: Pipe): void => {
//     pipe.done = true;
//     pipe.cancelIdle?.();
//     pipe.ws?.terminate();
//   };
//   const endIdle = (h: Held): void => {
//     for (const pipe of h.idle) end(pipe);
//     h.idle.clear();
//   };
//
//   const again = (h: Held, ms: number): void => {
//     if (h.retry !== undefined) return;
//     h.retry = clock.schedule(
//       () => {
//         h.retry = undefined;
//         fill(h);
//       },
//       ms,
//       { unref: true },
//     );
//   };
//   const wait = (h: Held): void => again(h, backoff(++h.attempt));
//
//   /** Dials up to PIPE_IDLE idle pipes, with every socket of the workspace, taken or idle, inside PIPE_POOL_MAX: past
//    * it the daemon holds the next connection until a taken pipe closes. */
//   const fill = (h: Held): void => {
//     if (h.stopped || h.asking || h.retry !== undefined || h.pace !== undefined) return;
//     if (!o.ready()) return wait(h);
//     if (!h.serves) return ask(h);
//     if (h.taken.size >= PIPE_POOL_MAX && !h.saidFull) {
//       h.saidFull = true;
//       log(`${h.id}: ${PIPE_POOL_MAX} pipes are in use, so the next connection waits for one to close`);
//     }
//     const short = Math.min(PIPE_IDLE - h.idle.size, PIPE_POOL_MAX - h.taken.size - h.idle.size);
//     if (short <= 0) return;
//     const now = clock.now();
//     h.dialled = h.dialled.filter(at => now - at < 1000);
//     const allowed = Math.min(short, dialRate - h.dialled.length);
//     if (!h.saidDialling) log(`${h.id}: dialling ${short} pipe${short === 1 ? "" : "s"}, ${PIPE_IDLE} kept idle`);
//     h.saidDialling = true;
//     for (let i = 0; i < allowed; i++) {
//       h.dialled.push(now);
//       dial(h);
//     }
//     if (allowed < short) {
//       h.pace = clock.schedule(
//         () => {
//           h.pace = undefined;
//           fill(h);
//         },
//         h.dialled[0]! + 1000 - now,
//         { unref: true },
//       );
//     }
//   };
//
//   /** A daemon below PIPE_DAEMON_VERSION reads `pipe` as nothing and answers the socket as a link, so it is asked its
//    * version first and dialled only once it is new enough. */
//   const ask = (h: Held): void => {
//     h.asking = true;
//     void o.version(h.id).then(
//       v => answered(h, v),
//       () => answered(h, null),
//     );
//   };
//   const answered = (h: Held, version: number | null): void => {
//     h.asking = false;
//     if (h.stopped) return;
//     if (version === null) return wait(h);
//     if (version >= PIPE_DAEMON_VERSION) {
//       h.serves = true;
//       return fill(h);
//     }
//     behind(h, `${h.id}: its daemon (${version}) serves no pipes yet, so it is asked again each minute`);
//   };
//   const behind = (h: Held, line: string): void => {
//     if (!h.saidBehind) log(line);
//     h.saidBehind = true;
//     h.serves = false;
//     endIdle(h);
//     again(h, behindAgainMs);
//   };
//
//   /** One pipe that could not be dialled, or closed while it waited: the link is down, so the rest go too, since an idle
//    * pipe sends nothing and would learn it only when the daemon took it, and the pool is dialled again on the link's
//    * own backoff. */
//   const linkDown = (h: Held): void => {
//     if (h.attempt === 0) log(`${h.id}: a pipe could not be dialled, so its others were closed and the pool is dialled again as the link comes back`);
//     endIdle(h);
//     if (!h.stopped) wait(h);
//   };
//
//   /** The pipes of one dial go together: the rest are ended here, so their own refusals are nobody's news. */
//   const refused = (h: Held): void => {
//     endIdle(h);
//     const { count, waitMs } = h.refusals.refused(clock.now());
//     const line = tokenRefusedLine(h.id, "a pipe was dialled with", count, waitMs);
//     if (line !== undefined) log(line);
//     o.rewrite(h.id);
//     h.retry?.();
//     h.retry = clock.schedule(
//       () => {
//         h.retry = undefined;
//         h.refusals.wrote(clock.now());
//         fill(h);
//       },
//       waitMs,
//       { unref: true },
//     );
//   };
//
//   const retire = (h: Held, pipe: Pipe): void => {
//     if (pipe.done) return;
//     h.idle.delete(pipe);
//     pipe.done = true;
//     pipe.ws?.close(1000);
//     fill(h);
//   };
//
//   const take = (h: Held, pipe: Pipe, ws: WebSocket, open: PipeOpen): void => {
//     h.idle.delete(pipe);
//     pipe.done = true;
//     pipe.cancelIdle?.();
//     h.attempt = 0;
//     h.saidBehind = false;
//     h.taken.add(ws);
//     ws.once("close", () => {
//       h.taken.delete(ws);
//       if (h.taken.size < PIPE_POOL_MAX) h.saidFull = false;
//       fill(h);
//     });
//     // Wrapped before anything else runs, so the binary frames right behind pipe.open reach the stream.
//     const duplex = createWebSocketStream(ws);
//     duplex.on("error", () => {});
//     const handler = o.handler();
//     const room = h.taken.size + h.idle.size < PIPE_POOL_MAX;
//     log(`${h.id}: a pipe opened on the ${open.door} door${open.heldMs > 0 ? ` after ${open.heldMs} ms held` : ""}, ${room ? "dialling its replacement" : "its replacement waits for a pipe to close"}`);
//     if (handler === undefined) {
//       log(`${h.id}: nothing here serves pipes yet, so that connection was closed`);
//       duplex.destroy();
//     } else {
//       try {
//         handler(h.id, open, duplex);
//       } catch (e) {
//         log(`${h.id}: the pipe's handler failed (${e instanceof Error ? e.message : String(e)})`);
//         duplex.destroy();
//       }
//     }
//     fill(h);
//   };
//
//   const dial = (h: Held): void => {
//     const pipe: Pipe = { open: false, authed: false, done: false };
//     h.idle.add(pipe);
//     const failed = (): void => {
//       if (pipe.done) return;
//       end(pipe);
//       h.idle.delete(pipe);
//       if (!h.stopped) linkDown(h);
//     };
//     let timer: (() => void) | undefined = clock.schedule(failed, PIPE_DIAL_MS, { unref: true });
//     void o.reach(h.id).then(reach => {
//       if (pipe.done) return;
//       if (reach === undefined) return failed();
//       const ws = new WebSocket(daemonWsUrl(reach.url), { handshakeTimeout: PIPE_DIAL_MS });
//       pipe.ws = ws;
//       ws.on("error", () => {});
//       ws.once("open", () => {
//         timer?.();
//         timer = undefined;
//         if (pipe.done) return;
//         pipe.open = true;
//         ws.send(JSON.stringify({ id: 0, op: "auth", token: reach.token, pipe: true }));
//         pipe.cancelIdle = clock.schedule(() => retire(h, pipe), idleMaxMs, { unref: true });
//       });
//       // The daemon answers the auth frame as it answers a link's, then says nothing until a connection takes the
//       // pipe. Any other frame is a daemon that serves no pipes answering the socket as a link, with its hello.
//       const frame = (data: Buffer, isBinary: boolean): void => {
//         if (pipe.done) return;
//         const text = isBinary ? undefined : data.toString();
//         const auth = text === undefined ? undefined : authAnswer(text);
//         if (auth === true) {
//           pipe.authed = true;
//           // Back once the daemon has taken the token on a full pool, not on a socket that opened: a daemon refusing
//           // pipes past its own cap closes them after they open, and its refusals keep backing off.
//           if (h.attempt > 0 && [...h.idle].filter(p => p.authed).length === PIPE_IDLE) {
//             h.attempt = 0;
//             log(`${h.id}: ${PIPE_IDLE} pipes idle`);
//           }
//           return;
//         }
//         ws.off("message", frame);
//         if (auth === false) return refused(h);
//         const open = text === undefined ? undefined : pipeOpenOf(text);
//         if (open === undefined) return behind(h, `${h.id}: its daemon answered a pipe as a link, so it is asked again each minute`);
//         take(h, pipe, ws, open);
//       };
//       ws.on("message", frame);
//       ws.once("close", (code: number, reason: Buffer) => {
//         timer?.();
//         timer = undefined;
//         if (pipe.done) return;
//         if (daemonRefusedToken(code, reason.toString())) {
//           pipe.done = true;
//           h.idle.delete(pipe);
//           return refused(h);
//         }
//         failed();
//       });
//     }, failed);
//   };
//
//   const down = (id: string): void => {
//     const h = held.get(id);
//     if (h === undefined) return;
//     held.delete(id);
//     h.stopped = true;
//     h.retry?.();
//     h.pace?.();
//     endIdle(h);
//     for (const ws of h.taken) ws.terminate();
//     h.taken.clear();
//   };
//
//   return {
//     up(id) {
//       if (held.has(id)) return;
//       const h: Held = { id, idle: new Set(), taken: new Set(), attempt: 0, dialled: [], stopped: false, refusals: tokenRefusals(behindAgainMs), serves: false, asking: false, saidBehind: false, saidDialling: false, saidFull: false };
//       held.set(id, h);
//       fill(h);
//     },
//     down,
//     idle: id => [...(held.get(id)?.idle ?? [])].filter(p => p.open).length,
//     close() {
//       for (const id of [...held.keys()]) down(id);
//     },
//   };
// }
/// Spawns its dials with `spawn_local`: call it inside a `LocalSet`.
pub fn pipe_pool(o: PipePoolOptions) -> Rc<PipePool> {
    let backoff: Rc<dyn Fn(u32) -> u64> =
        o.backoff_ms.unwrap_or_else(|| Rc::new(|attempt: u32| link_backoff_ms(i64::from(attempt), OsIds.random()) as u64));
    let log: Rc<dyn Fn(&str)> = o.log.unwrap_or_else(|| Rc::new(|line: &str| console::warn(line)));
    let core = Core {
        reach: o.reach,
        version: o.version,
        handler: o.handler,
        ready: o.ready,
        rewrite: o.rewrite,
        clock: o.clock.unwrap_or_else(real_clock),
        backoff,
        idle_max_ms: o.idle_max_ms.unwrap_or(PIPE_IDLE_MAX_MS),
        behind_again_ms: o.behind_again_ms.unwrap_or(PIPE_BEHIND_AGAIN_MS),
        dial_rate: o.dial_rate.unwrap_or(PIPE_DIAL_RATE),
        log,
        held: RefCell::new(IndexMap::new()),
    };
    Rc::new(PipePool { core: Rc::new(core) })
}

/// The daemon's answer to a pipe's auth frame: `Some(ok)` for a `DaemonResponse` with id 0, `None` for any other
/// frame.
// TS pipe.ts:133-144
// /** The daemon's answer to a pipe's auth frame: true where it took the token, false where it refused it, undefined for
//  * any other frame. */
// function authAnswer(text: string): boolean | undefined {
//   let frame: unknown;
//   try {
//     frame = JSON.parse(text);
//   } catch {
//     return undefined;
//   }
//   const reply = DaemonResponse.safeParse(frame);
//   return reply.success && reply.data.id === 0 ? reply.data.ok : undefined;
// }
pub fn auth_answer(text: &str) -> Option<bool> {
    let frame = wsp_wire::wire::json_text::parse_json(text)?;
    let zero = |id: &Option<ReqId>| matches!(id, Some(ReqId::Num(n)) if n.0 == 0.0);
    match parse_daemon_response(&frame).ok()? {
        DaemonResponse::V0(ok) => zero(&ok.id).then_some(true),
        DaemonResponse::V1(err) => zero(&err.id).then_some(false),
    }
}

/// The pipe's first frame: `{"id":0,"op":"auth","token":<token>,"pipe":true}`, in that key order.
pub fn auth_frame(token: &str) -> String {
    format!("{{\"id\":0,\"op\":\"auth\",\"token\":{},\"pipe\":true}}", serde_json::Value::String(token.to_string()))
}
