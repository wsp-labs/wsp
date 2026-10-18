// SPDX-License-Identifier: AGPL-3.0-only
//! A daemon channel to one computer (`daemon.open`), shared by every drawer and panel tab on it: the shells it lists,
//! the frames each shell sends, the output it pushes back, and how the channel stands. Ported from today's
//! `terminal/daemon-link.ts` (dial, ping, backoff) and `terminal/link.ts` (adopt, re-attach, lost shells).

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::channel::{mpsc, oneshot};
use futures::future::Either;
use futures::{FutureExt, StreamExt};
use gpui_kit::{App, AppContext, Context, Entity, EventEmitter, Global, SharedString, Subscription, Task, WeakEntity};
use serde_json::{json, Value};
use wsp_host_client::protocol::{DaemonCloseRequest, DaemonOpenRequest, DaemonSendRequest, WorkspacesTouchRequest};
use wsp_host_client::{Event, Failure, HostClient};

use super::session::{Changed, Mode, Out, Session};
use super::words::{Memory, SHELL_ENDED_LINE, SHELL_STOPPED_LINE};
use crate::local_store::LocalStore;
use crate::screens::Host;

/// Whose daemon a channel opens: a workspace's, or this computer's own (`placeId: "here"`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Workspace(String),
    Here,
}

/// How the channel stands, by the web client's words (`daemon-link.ts`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Opening,
    /// No first answer within its bound.
    Unanswered,
    Live,
    /// Live before, lost, trying again since.
    Connecting {
        since: Instant,
    },
    Refused(SharedString),
    Reauth,
    Dead(SharedString),
}

/// Who a new shell belongs to: a thread's drawer by the host's tag, or a panel tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tag {
    Thread(String),
    Panel(SharedString),
    None,
}

/// A shell the channel holds and who it belongs to.
#[derive(Clone)]
pub struct Pty {
    pub session: Entity<Session>,
    pub tag: Tag,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkEvent {
    Changed,
}

/// DEFAULT_HEARTBEAT_MS: the host's edge sweeps quiet sockets, so liveness is an app-level ping.
const HEARTBEAT: Duration = Duration::from_secs(10);
/// DEFAULT_FIRST_ANSWER_MS: long enough for a daemon the host starts on the first dial.
pub(crate) const FIRST_ANSWER: Duration = Duration::from_secs(20);
/// REFUSED_ATTEMPT: a door that answered with a status is dialled again only at the backoff's ceiling.
const REFUSED_ATTEMPT: u32 = 32;
/// The kinds a dial at the usual pace can mend; any other kind is an answer the link holds with its sentence.
const PASSING_KINDS: [&str; 3] = ["transient", "concurrency", "unknown"];
/// The close code of a refused token: the host dials again with the one it holds now.
const REAUTH_CLOSE: i64 = 4401;
/// Keystrokes reach the runtime as one `workspaces.touch` per this window; its idle window is minutes.
const TOUCH_EVERY: Duration = Duration::from_secs(30);
/// SIZE_KEY in `terminal/link.ts`: a shell started at another width than its view's redraws its prompt.
const SIZE_KEY: &str = "wsp.terminal.size";
const DEFAULT_SIZE: (u16, u16) = (80, 24);
/// A link no drawer or panel tab holds for this long lets its channel go; the daemon keeps the shells for the next.
const PARK_AFTER: Duration = Duration::from_secs(30);

/// `linkBackoffMs`: half a second doubling to ten, with up to a quarter second of jitter.
fn backoff(attempt: u32) -> Duration {
    let base = 500u64.saturating_mul(1 << attempt.saturating_sub(1).min(16)).min(10_000);
    let jitter = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as u64 % 250).unwrap_or(0);
    Duration::from_millis(base + jitter)
}

/// What the host's socket says about one channel, carried to the window's thread.
enum Pushed {
    Opened(String),
    OpenFailed(Failure),
    Frame(Value),
    /// Every frame the socket held when a pty's attach was answered has been passed on: its replay is over.
    Replayed(String),
    /// The host's socket closed under the channel.
    Gone,
}

/// One daemon frame on its way down the channel, and who waits for the daemon's answer.
struct Req {
    frame: Value,
    reply: Option<oneshot::Sender<Result<Value, SharedString>>>,
}

type Answer = oneshot::Receiver<Result<Value, SharedString>>;

async fn answered(answer: Answer) -> Result<Value, SharedString> {
    answer.await.unwrap_or_else(|_| Err("connection lost".into()))
}

/// A channel the host opened for one dial: once nothing here holds it, the host is asked to close it, unless the host
/// closed it first (`daemon-link.ts` closes one that opens after the link let go).
struct Opened {
    client: Arc<HostClient>,
    channel: String,
    open: bool,
}

impl Drop for Opened {
    fn drop(&mut self) {
        if !self.open {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else { return };
        let (client, channel) = (self.client.clone(), std::mem::take(&mut self.channel));
        runtime.spawn(async move {
            let req: Result<DaemonCloseRequest, _> = serde_json::from_value(json!({ "channel": channel }));
            if let Ok(req) = req {
                _ = client.call(req).await;
            }
        });
    }
}

/// A frame the host pushed for `channel`, read as today's client routes it: by its channel alone, so a close that does
/// not read as one still closes.
fn on_channel(next: Result<Event, Failure>, channel: &str) -> Option<Value> {
    let frame = match next {
        Ok(Event::Daemon(pushed)) => serde_json::to_value(&pushed).ok()?,
        Ok(Event::Other(frame)) if frame["type"].as_str().is_some_and(|t| t.starts_with("daemon.")) => frame,
        _ => return None,
    };
    (frame["channel"] == channel).then_some(frame)
}

pub struct Link {
    target: Target,
    host: Host,
    status: Status,
    /// The shells listed, in the order they were opened or adopted.
    ptys: Vec<Pty>,
    /// Shells created or adopted whose attach is still out: their output is routed, but they are not listed yet.
    attaching: Vec<Entity<Session>>,
    watched: HashMap<SharedString, Subscription>,
    ever_live: bool,
    /// The computer stopped since the link was last live, so the shells the next attach misses ended with it.
    stopped: bool,
    waited: bool,
    attempt: u32,
    /// Bumped on every dial: answers and timers from an older dial are dropped.
    dial: u64,
    /// The channel a ping has come back on: what a frame may be put on.
    proven: bool,
    writer: Option<mpsc::UnboundedSender<Req>>,
    awaiting_pong: bool,
    tasks: Vec<Task<()>>,
    retry: Option<Task<()>>,
    _first_answer: Task<()>,
    touched: Option<Instant>,
    /// Asks the pump to pass on every frame already read before it says a pty's replay is over.
    barrier: Option<mpsc::UnboundedSender<String>>,
    /// The tag a create wanted for a pty the listing adopted first.
    wanted: HashMap<SharedString, Tag>,
    /// Shells to end once the channel is up, asked to end while it was not.
    doomed: Vec<SharedString>,
    holds: Rc<Cell<usize>>,
    unheld_since: Option<Instant>,
    _parking: Task<()>,
    /// Whether the computer's readings come down this channel (`sys.watch`), asked again each time it goes live.
    watching_sys: bool,
    /// The last `sys.sample`, kept across a drop: what the pane says a channel lost to a full memory was about.
    memory: Option<Memory>,
}

impl EventEmitter<LinkEvent> for Link {}

/// A drawer's or a panel tab's claim on a link; a link with none lets its channel go after a while.
pub struct LinkHold(Rc<Cell<usize>>);

impl Drop for LinkHold {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_sub(1));
    }
}

/// Each channel in this app, and the panel tags of shells whose link let go, which only this window knows.
#[derive(Default)]
struct Links(Vec<(Target, Entity<Link>)>, HashMap<SharedString, Tag>);

impl Global for Links {}

impl Link {
    /// The channel to `target` if one is open in this app, dialling none (`getTerminals`).
    pub fn find(target: &Target, cx: &App) -> Option<Entity<Link>> {
        cx.try_global::<Links>()?.0.iter().find(|(t, _)| t == target).map(|(_, link)| link.clone())
    }

    /// The one channel to `target` in this app, dialled on first ask.
    pub fn get(host: &Host, target: Target, cx: &mut App) -> Entity<Link> {
        if let Some((_, link)) = cx.default_global::<Links>().0.iter().find(|(t, _)| *t == target) {
            return link.clone();
        }
        let link = cx.new(|cx| {
            let first_answer = cx.spawn(async move |this: WeakEntity<Link>, cx| {
                cx.background_executor().timer(FIRST_ANSWER).await;
                _ = this.update(cx, |link, cx| {
                    link.waited = true;
                    if link.status == Status::Opening {
                        link.set_status(Status::Unanswered, cx);
                    }
                });
            });
            let parking = cx.spawn(async move |this: WeakEntity<Link>, cx| loop {
                cx.background_executor().timer(HEARTBEAT).await;
                if this.update(cx, |link, cx| link.park_when_unheld(cx)).is_err() {
                    return;
                }
            });
            let mut link = Link {
                target: target.clone(),
                host: host.clone(),
                status: Status::Opening,
                ptys: Vec::new(),
                attaching: Vec::new(),
                watched: HashMap::new(),
                ever_live: false,
                stopped: false,
                waited: false,
                attempt: 0,
                dial: 0,
                proven: false,
                writer: None,
                awaiting_pong: false,
                tasks: Vec::new(),
                retry: None,
                _first_answer: first_answer,
                touched: None,
                barrier: None,
                wanted: HashMap::new(),
                doomed: Vec::new(),
                holds: Rc::new(Cell::new(0)),
                unheld_since: None,
                _parking: parking,
                watching_sys: false,
                memory: None,
            };
            link.connect(cx);
            link
        });
        cx.global_mut::<Links>().0.push((target, link.clone()));
        link
    }

    pub fn target(&self) -> &Target {
        &self.target
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn ptys(&self) -> &[Pty] {
        &self.ptys
    }

    /// The workspace napped on a computer whose nap ends every process (`feedStop`).
    pub fn stopped(&mut self) {
        self.stopped = true;
    }

    /// Whether a nap that ends processes came since the link was last live.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Has the daemon send its computer's readings down this channel from now on, as today's `wiring.ts` asks where
    /// they are read off the daemon: on a computer whose readings the host reads, a second watch is a second sampler.
    pub fn watch_sys(&mut self, cx: &mut Context<Self>) {
        if !std::mem::replace(&mut self.watching_sys, true) && self.status == Status::Live {
            self.ask_sys_watch(cx);
        }
    }

    fn ask_sys_watch(&self, cx: &mut Context<Self>) {
        if let Some(answer) = self.ask("sys.watch", json!({})) {
            cx.background_spawn(async move { _ = answer.await }).detach();
        }
    }

    /// The computer's last reading, kept once the channel is down.
    pub fn memory(&self) -> Option<Memory> {
        self.memory
    }

    fn sampled(&mut self, event: &Value, cx: &mut Context<Self>) {
        let read = |v: &Value| v.as_f64().unwrap_or(0.);
        let memory = Memory { used: read(&event["mem"]["used"]), total: read(&event["mem"]["total"]), load1: read(&event["load1"]) };
        self.keep_reading(memory, cx);
    }

    fn keep_reading(&mut self, memory: Memory, cx: &mut Context<Self>) {
        let was = self.memory.replace(memory);
        if was.is_some_and(|m| m.near_full()) != memory.near_full() {
            cx.emit(LinkEvent::Changed);
            cx.notify();
        }
    }

    /// Keeps the link's channel up while the returned claim lives.
    pub fn hold(&mut self) -> LinkHold {
        self.holds.set(self.holds.get() + 1);
        self.unheld_since = None;
        LinkHold(self.holds.clone())
    }

    fn park_when_unheld(&mut self, cx: &mut Context<Self>) {
        if self.holds.get() > 0 {
            self.unheld_since = None;
            return;
        }
        let now = cx.background_executor().now();
        match self.unheld_since {
            None => self.unheld_since = Some(now),
            Some(since) if now.duration_since(since) >= PARK_AFTER => self.park(cx),
            Some(_) => {}
        }
    }

    /// Lets the channel and every shell's screen go; the next ask dials again and the daemon replays each shell.
    fn park(&mut self, cx: &mut Context<Self>) {
        let panel: Vec<(SharedString, Tag)> = self
            .ptys
            .iter()
            .filter(|p| matches!(p.tag, Tag::Panel(_)))
            .map(|p| (p.session.read(cx).pty().clone(), p.tag.clone()))
            .collect();
        self.drop_channel(cx);
        self.retry = None;
        self.ptys.clear();
        self.attaching.clear();
        self.watched.clear();
        let me = cx.entity_id();
        let links = cx.default_global::<Links>();
        links.1.extend(panel);
        links.0.retain(|(_, link)| link.entity_id() != me);
    }

    /// The grid size the last view measured, kept across launches; a new shell opens at it.
    pub fn last_size(cx: &App) -> (u16, u16) {
        let saved = cx.try_global::<LocalStore>().and_then(|s| s.get(SIZE_KEY)).cloned().unwrap_or(Value::Null);
        match (saved["cols"].as_u64(), saved["rows"].as_u64()) {
            (Some(cols @ 1..=65535), Some(rows @ 1..=65535)) => (cols as u16, rows as u16),
            _ => DEFAULT_SIZE,
        }
    }

    fn keep_size(cols: u16, rows: u16, cx: &mut App) {
        if cx.has_global::<LocalStore>() {
            _ = cx.global_mut::<LocalStore>().set(SIZE_KEY, json!({ "cols": cols, "rows": rows }));
        }
    }

    /// Starts a shell at the grid size the pane measured (the last measured one for 0 x 0) and attaches to it.
    pub fn create(&mut self, tag: Tag, cols: u16, rows: u16, cx: &mut Context<Self>) -> Task<Result<Entity<Session>, SharedString>> {
        let (cols, rows) = if cols == 0 || rows == 0 { Link::last_size(cx) } else { (cols, rows) };
        let mut params = json!({ "cols": cols, "rows": rows });
        if let Tag::Thread(thread) = &tag {
            params["thread"] = thread.clone().into();
        }
        let Some(created) = self.ask("pty.create", params) else {
            return Task::ready(Err("not answering".into()));
        };
        cx.spawn(async move |this, cx| {
            let created = answered(created).await?;
            let pty: SharedString =
                created["ptyId"].as_str().ok_or_else(|| SharedString::from("the daemon answered no ptyId"))?.to_owned().into();
            // A listing answered between the create and this read may have adopted the shell already.
            let adopted = this
                .update(cx, |link, cx| {
                    let held = link.session(&pty, cx)?;
                    if link.ptys.iter().any(|p| p.session == held) {
                        link.retag(&pty, tag.clone(), cx);
                    } else {
                        link.wanted.insert(pty.clone(), tag.clone());
                    }
                    Some(held)
                })
                .map_err(|_| SharedString::from("the terminal closed"))?;
            if let Some(held) = adopted {
                return Ok(held);
            }
            let (session, attached) = this
                .update(cx, |link, cx| {
                    let session = cx.new(|_| Session::new(pty.clone(), "", cols, rows));
                    link.attaching.push(session.clone());
                    (session, link.ask("pty.attach", json!({ "ptyId": pty })))
                })
                .map_err(|_| SharedString::from("the terminal closed"))?;
            let attached = match attached {
                Some(answer) => answered(answer).await,
                None => Err("not answering".into()),
            };
            this.update(cx, |link, cx| {
                link.attaching.retain(|s| *s != session);
                attached?;
                let title = link.numbered(cx);
                session.update(cx, |s, cx| s.set_title(title, cx));
                link.list(session.clone(), tag, cx);
                Ok(session)
            })
            .map_err(|_| SharedString::from("the terminal closed"))?
        })
    }

    /// Ends a shell (`pty.kill`) and drops it from the list; nothing of it is kept.
    pub fn kill(&mut self, pty: &str, cx: &mut Context<Self>) {
        self.ptys.retain(|p| p.session.read(cx).pty() != pty);
        self.watched.remove(pty);
        match self.ask("pty.kill", json!({ "ptyId": pty })) {
            Some(answer) => cx.background_spawn(async move { _ = answer.await }).detach(),
            None => self.doomed.push(pty.to_owned().into()),
        }
        cx.emit(LinkEvent::Changed);
        cx.notify();
    }

    /// Hands a shell to another thread's drawer, here at once and on the host for every window that lists it.
    pub fn retag(&mut self, pty: &str, tag: Tag, cx: &mut Context<Self>) {
        let Some(held) = self.ptys.iter_mut().find(|p| p.session.read(cx).pty() == pty) else { return };
        if held.tag == tag {
            return;
        }
        held.tag = tag.clone();
        if let Tag::Thread(thread) = tag {
            if let Some(answer) = self.ask("pty.thread", json!({ "ptyId": pty, "thread": thread })) {
                cx.background_spawn(async move { _ = answer.await }).detach();
            }
        }
        cx.emit(LinkEvent::Changed);
        cx.notify();
    }

    /// The name a new tab reads under: "shell" with the lowest number no listed tab holds.
    fn numbered(&self, cx: &App) -> String {
        let taken: Vec<SharedString> = self.ptys.iter().map(|p| p.session.read(cx).title().clone()).collect();
        (1..).map(|n| format!("shell {n}")).find(|t| !taken.iter().any(|s| s.as_ref() == t)).unwrap_or_default()
    }

    fn list(&mut self, session: Entity<Session>, tag: Tag, cx: &mut Context<Self>) {
        let pty = session.read(cx).pty().clone();
        // An observer misses a notify made while a window draws the session, so the outbox is drained on an event.
        let drained = cx.subscribe(&session, |link, session, changed: &Changed, cx| {
            if *changed == Changed::Owed {
                link.drain(&session, cx);
            }
        });
        self.watched.insert(pty, drained);
        // A reply the screen owed while the attach was out was emitted with no one subscribed.
        self.drain(&session, cx);
        self.ptys.push(Pty { session, tag });
        cx.emit(LinkEvent::Changed);
        cx.notify();
    }

    /// What a shell owes its pty, down the channel: its keys and replies as `pty.write`, its size as `pty.resize`.
    fn drain(&mut self, session: &Entity<Session>, cx: &mut Context<Self>) {
        let outs = session.update(cx, |s, _| s.take_out());
        if outs.is_empty() {
            return;
        }
        let pty = session.read(cx).pty().clone();
        for out in outs {
            match out {
                Out::Write(data) => {
                    self.put(json!({ "op": "pty.write", "ptyId": pty, "data": data }));
                    self.touch(cx);
                }
                Out::Resize { cols, rows } => {
                    Link::keep_size(cols, rows, cx);
                    self.put(json!({ "op": "pty.resize", "ptyId": pty, "cols": cols, "rows": rows }));
                }
            }
        }
    }

    fn touch(&mut self, cx: &mut Context<Self>) {
        let Target::Workspace(id) = &self.target else { return };
        if self.touched.is_some_and(|at| at.elapsed() < TOUCH_EVERY) {
            return;
        }
        self.touched = Some(Instant::now());
        let id = id.clone();
        let touched = self.host.run(move |client| async move {
            let req: WorkspacesTouchRequest =
                serde_json::from_value(json!({ "workspaceId": id })).map_err(|e| Failure::new(e.to_string()))?;
            client.call(req).await.map(|_| ())
        });
        cx.background_spawn(async move { _ = touched.await }).detach();
    }

    /// A frame down the proven channel whose answer nobody waits for; dropped while the channel is not up.
    fn put(&self, frame: Value) {
        if let (true, Some(writer)) = (self.proven, &self.writer) {
            _ = writer.unbounded_send(Req { frame, reply: None });
        }
    }

    /// A frame down the proven channel and the daemon's answer; none while the channel is not up.
    fn ask(&self, op: &str, params: Value) -> Option<Answer> {
        if !self.proven {
            return None;
        }
        self.send(op, params)
    }

    fn send(&self, op: &str, mut params: Value) -> Option<Answer> {
        let writer = self.writer.as_ref()?;
        params["op"] = op.into();
        let (tx, rx) = oneshot::channel();
        writer.unbounded_send(Req { frame: params, reply: Some(tx) }).ok()?;
        Some(rx)
    }

    fn set_status(&mut self, status: Status, cx: &mut Context<Self>) {
        if self.status == status || matches!((&self.status, &status), (Status::Connecting { .. }, Status::Connecting { .. })) {
            return;
        }
        self.status = status;
        cx.emit(LinkEvent::Changed);
        cx.notify();
    }

    /// What a link that is not open reads as: only one that was open once is reconnecting.
    fn dialing(&self) -> Status {
        if self.ever_live {
            Status::Connecting { since: Instant::now() }
        } else if self.waited {
            Status::Unanswered
        } else {
            Status::Opening
        }
    }

    fn open_params(&self) -> Value {
        match &self.target {
            Target::Workspace(id) => json!({ "workspaceId": id }),
            Target::Here => json!({ "placeId": "here" }),
        }
    }

    /// After a nap, as today dials the moment the phase reads running (`wiring.ts`): a link waiting out its backoff
    /// dials now; one live or dialling already is left as it is.
    pub fn redial(&mut self, cx: &mut Context<Self>) {
        if self.retry.is_some() {
            self.connect(cx);
        }
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        self.retry = None;
        self.dial += 1;
        let dial = self.dial;
        if !matches!(self.status, Status::Refused(_)) {
            let status = self.dialing();
            self.set_status(status, cx);
        }
        let (tx, mut rx) = mpsc::unbounded::<Pushed>();
        let (barrier, mut barred) = mpsc::unbounded::<String>();
        self.barrier = Some(barrier);
        let open = self.open_params();
        let pump = self.host.run(move |client| async move {
            // Before the open: the host pushes a channel's frames only to the socket that opened it, from then on.
            let mut events = match client.events().await {
                Ok(events) => events,
                Err(failure) => {
                    _ = tx.unbounded_send(Pushed::OpenFailed(failure));
                    return Ok(());
                }
            };
            // Apart from the pump, which a link letting go stops: what the host opens after that is still closed.
            let (handed, opened) = oneshot::channel();
            let opener = client.clone();
            tokio::spawn(async move {
                let req: Result<DaemonOpenRequest, _> = serde_json::from_value(open).map_err(|e| Failure::new(e.to_string()));
                let answer = match req {
                    Ok(req) => opener.call(req).await.map(|o| Opened { client: opener.clone(), channel: o.channel, open: true }),
                    Err(failure) => Err(failure),
                };
                _ = handed.send(answer);
            });
            let mut opened = match opened.await {
                Ok(Ok(opened)) => opened,
                Ok(Err(failure)) => {
                    _ = tx.unbounded_send(Pushed::OpenFailed(failure));
                    return Ok(());
                }
                Err(_) => return Ok(()),
            };
            let channel = opened.channel.clone();
            if tx.unbounded_send(Pushed::Opened(channel.clone())).is_err() {
                return Ok(());
            }
            let mut pass = |next: Result<Event, Failure>| {
                let Some(frame) = on_channel(next, &channel) else { return true };
                if frame["type"] != "daemon.event" {
                    opened.open = false;
                }
                tx.unbounded_send(Pushed::Frame(frame)).is_ok()
            };
            let mut barring = true;
            loop {
                let next = if barring {
                    match futures::future::select(std::pin::pin!(events.next()), barred.next()).await {
                        Either::Left((next, _)) => Ok(next),
                        Either::Right((pty, _)) => Err(pty),
                    }
                } else {
                    Ok(events.next().await)
                };
                match next {
                    Ok(None) => break,
                    Ok(Some(next)) => {
                        if !pass(next) {
                            return Ok(());
                        }
                    }
                    Err(None) => barring = false,
                    // The socket's one reader queues a push before it settles the answer after it, so an attach's
                    // replay is queued here by the time its answer is: passing on what is queued keeps the order.
                    Err(Some(pty)) => {
                        while let Some(Some(next)) = events.next().now_or_never() {
                            if !pass(next) {
                                return Ok(());
                            }
                        }
                        if tx.unbounded_send(Pushed::Replayed(pty)).is_err() {
                            return Ok(());
                        }
                    }
                }
            }
            opened.open = false;
            _ = tx.unbounded_send(Pushed::Gone);
            Ok(())
        });
        let pumping = cx.background_spawn(async move { _ = pump.await });
        let reading = cx.spawn(async move |this, cx| {
            while let Some(first) = rx.next().await {
                let mut batch = vec![first];
                while let Ok(more) = rx.try_recv() {
                    batch.push(more);
                }
                if this.update(cx, |link, cx| link.pushed(dial, batch, cx)).is_err() {
                    return;
                }
            }
        });
        self.tasks = vec![pumping, reading];
    }

    /// Everything the socket said since the last batch, at one notify per shell for the whole batch's output.
    fn pushed(&mut self, dial: u64, batch: Vec<Pushed>, cx: &mut Context<Self>) {
        if dial != self.dial {
            return;
        }
        let mut output: Vec<(SharedString, Vec<u8>)> = Vec::new();
        for pushed in batch {
            if dial != self.dial {
                return;
            }
            match pushed {
                Pushed::Opened(channel) => self.opened(channel, cx),
                Pushed::OpenFailed(failure) => self.open_failed(failure, cx),
                Pushed::Gone => {
                    self.flush(&mut output, cx);
                    self.lost_channel(false, cx);
                }
                Pushed::Replayed(pty) => {
                    self.flush(&mut output, cx);
                    if let Some(session) = self.session(&pty, cx) {
                        session.update(cx, |s, cx| s.end_replay(cx));
                    }
                }
                Pushed::Frame(frame) if frame["type"] != "daemon.event" => {
                    self.flush(&mut output, cx);
                    self.lost_channel(frame["code"].as_i64() == Some(REAUTH_CLOSE), cx);
                }
                Pushed::Frame(frame) => {
                    let event = &frame["event"];
                    if event["type"] == "sys.sample" {
                        self.sampled(event, cx);
                        continue;
                    }
                    let Some(pty) = event["ptyId"].as_str() else { continue };
                    match event["type"].as_str() {
                        Some("pty.data") => {
                            let data = event["data"].as_str().unwrap_or_default().as_bytes();
                            match output.iter_mut().find(|(p, _)| p.as_ref() == pty) {
                                Some((_, bytes)) => bytes.extend_from_slice(data),
                                None => output.push((pty.to_owned().into(), data.to_vec())),
                            }
                        }
                        Some("pty.exit") => {
                            self.flush(&mut output, cx);
                            let code = event["exitCode"].as_i64().unwrap_or(-1) as i32;
                            if let Some(session) = self.session(pty, cx) {
                                session.update(cx, |s, cx| s.set_exited(code, cx));
                                cx.emit(LinkEvent::Changed);
                            }
                        }
                        Some("pty.mode") => {
                            self.flush(&mut output, cx);
                            let mode = if event["mode"] == "line" { Mode::Line } else { Mode::Raw };
                            let echo = event["echo"].as_bool().unwrap_or(true);
                            if let Some(session) = self.session(pty, cx) {
                                session.update(cx, |s, cx| s.set_mode(mode, echo, cx));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        self.flush(&mut output, cx);
    }

    /// An attach was answered: once the frames read before its answer have reached the screen, its replay is over.
    fn attach_answered(&self, session: &Entity<Session>, ok: bool, cx: &mut Context<Self>) {
        let pty = session.read(cx).pty().to_string();
        if !ok || self.barrier.as_ref().is_none_or(|b| b.unbounded_send(pty).is_err()) {
            session.update(cx, |s, cx| s.end_replay(cx));
        }
    }

    fn session(&self, pty: &str, cx: &App) -> Option<Entity<Session>> {
        let listed = self.ptys.iter().map(|p| &p.session);
        listed.chain(self.attaching.iter()).find(|s| s.read(cx).pty() == pty).cloned()
    }

    fn flush(&mut self, output: &mut Vec<(SharedString, Vec<u8>)>, cx: &mut Context<Self>) {
        for (pty, bytes) in output.drain(..) {
            if let Some(session) = self.session(&pty, cx) {
                session.update(cx, |s, cx| s.feed(&bytes, cx));
            }
        }
    }

    fn opened(&mut self, channel: String, cx: &mut Context<Self>) {
        let (writer, mut queue) = mpsc::unbounded::<Req>();
        let writing = self.host.run(move |client| async move {
            let mut next: Option<Req> = None;
            loop {
                let mut req = match next.take() {
                    Some(req) => req,
                    None => match queue.next().await {
                        Some(req) => req,
                        None => return Ok(()),
                    },
                };
                // Keys typed faster than a round trip go down as one write, never past a frame queued between them.
                if req.reply.is_none() && req.frame["op"] == "pty.write" {
                    while let Ok(more) = queue.try_recv() {
                        if more.reply.is_none() && more.frame["op"] == "pty.write" && more.frame["ptyId"] == req.frame["ptyId"] {
                            let joined = format!(
                                "{}{}",
                                req.frame["data"].as_str().unwrap_or_default(),
                                more.frame["data"].as_str().unwrap_or_default()
                            );
                            req.frame["data"] = joined.into();
                        } else {
                            next = Some(more);
                            break;
                        }
                    }
                }
                let sent = async {
                    let asked: DaemonSendRequest = serde_json::from_value(json!({ "channel": channel, "frame": req.frame }))
                        .map_err(|e| SharedString::from(e.to_string()))?;
                    let sent = client.call(asked).await.map_err(|f| SharedString::from(f.message))?;
                    if sent.reply["ok"] == true {
                        Ok(sent.reply)
                    } else {
                        Err(SharedString::from(sent.reply["error"].as_str().unwrap_or("refused").to_owned()))
                    }
                }
                .await;
                if let Some(reply) = req.reply {
                    _ = reply.send(sent);
                }
            }
        });
        self.tasks.push(cx.background_spawn(async move { _ = writing.await }));
        self.writer = Some(writer);
        // A ping before live, as a direct dial always did: a channel that dies on opening never flaps through live.
        let Some(pong) = self.send("ping", json!({})) else { return };
        let dial = self.dial;
        self.tasks.push(cx.spawn(async move |this, cx| {
            let ponged = answered(pong).await;
            _ = this.update(cx, |link, cx| {
                if link.dial != dial {
                    return;
                }
                match ponged {
                    Ok(_) => link.proven(cx),
                    Err(_) => link.abandon(cx),
                }
            });
        }));
    }

    fn proven(&mut self, cx: &mut Context<Self>) {
        self.proven = true;
        self.attempt = 0;
        let relive = self.ever_live;
        self.ever_live = true;
        let dial = self.dial;
        self.tasks.push(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(HEARTBEAT).await;
            let beat = this.update(cx, |link, cx| link.beat(dial, cx));
            if !matches!(beat, Ok(true)) {
                return;
            }
        }));
        self.tasks.push(cx.spawn(async move |this, cx| Link::go_live(this, dial, relive, cx).await));
    }

    /// One heartbeat: a ping still unanswered at the next beat drops the channel as half dead.
    fn beat(&mut self, dial: u64, cx: &mut Context<Self>) -> bool {
        if dial != self.dial {
            return false;
        }
        if self.awaiting_pong {
            self.abandon(cx);
            return false;
        }
        self.awaiting_pong = true;
        let Some(pong) = self.ask("ping", json!({})) else { return false };
        cx.spawn(async move |this, cx| {
            if answered(pong).await.is_ok() {
                _ = this.update(cx, |link, _| {
                    if link.dial == dial {
                        link.awaiting_pong = false;
                    }
                });
            }
        })
        .detach();
        true
    }

    /// The live ritual (`#goLive`): a new channel lost the daemon's pty subscriptions, so every shell is wiped and
    /// attached again for the daemon to replay its scrollback; then shells this link never opened are adopted.
    async fn go_live(this: WeakEntity<Link>, dial: u64, relive: bool, cx: &mut gpui_kit::AsyncApp) {
        let current = |link: &Link| link.dial == dial;
        if relive {
            let Ok(held) = this.update(cx, |link, _| link.ptys.iter().map(|p| p.session.clone()).collect::<Vec<_>>()) else { return };
            for session in held {
                let Ok(attached) = this.update(cx, |link, cx| {
                    if !current(link) || session.read(cx).exited().is_some() {
                        return None;
                    }
                    let pty = session.read(cx).pty().clone();
                    session.update(cx, |s, cx| s.reset_screen(cx));
                    link.ask("pty.attach", json!({ "ptyId": pty }))
                }) else {
                    return;
                };
                let Some(attached) = attached else { continue };
                let attached = answered(attached).await;
                let Ok(still) = this.update(cx, |link, cx| {
                    if !current(link) {
                        return false;
                    }
                    link.attach_answered(&session, attached.is_ok(), cx);
                    // A refusal on a live channel means the daemon no longer holds that pty; the rest still attach.
                    if attached.is_err() {
                        let why = if link.stopped { SHELL_STOPPED_LINE } else { SHELL_ENDED_LINE };
                        session.update(cx, |s, cx| s.set_lost(why, cx));
                        cx.emit(LinkEvent::Changed);
                    } else {
                        // A size the view measured while the channel was down was dropped on the way.
                        session.update(cx, |s, cx| s.tell_size_again(cx));
                    }
                    true
                }) else {
                    return;
                };
                if !still {
                    return;
                }
            }
        }
        let Ok(Some(listed)) = this.update(cx, |link, cx| {
            link.stopped = false;
            for pty in std::mem::take(&mut link.doomed) {
                link.kill(&pty, cx);
            }
            link.ask("pty.list", json!({}))
        }) else {
            return;
        };
        if let Ok(listed) = answered(listed).await {
            for row in listed["ptys"].as_array().cloned().unwrap_or_default() {
                let Some(pty) = row["id"].as_str().map(str::to_owned) else { continue };
                // A reply's pty is its block's, which takes it back by the thread's record.
                if row["reply"] == true {
                    continue;
                }
                let Ok(attach) = this.update(cx, |link, cx| {
                    if !current(link) || link.session(&pty, cx).is_some() || link.doomed.iter().any(|d| d.as_ref() == pty) {
                        return None;
                    }
                    let (cols, rows) = (row["cols"].as_u64().unwrap_or(80) as u16, row["rows"].as_u64().unwrap_or(24) as u16);
                    let session = cx.new(|_| Session::new(pty.clone(), "", cols.max(1), rows.max(1)));
                    session.update(cx, |s, _| s.begin_replay());
                    link.attaching.push(session.clone());
                    Some((session, link.ask("pty.attach", json!({ "ptyId": pty }))))
                }) else {
                    return;
                };
                let Some((session, attached)) = attach else { continue };
                let attached = match attached {
                    Some(answer) => answered(answer).await.is_ok(),
                    None => false,
                };
                let Ok(()) = this.update(cx, |link, cx| {
                    link.attaching.retain(|s| *s != session);
                    link.attach_answered(&session, attached, cx);
                    if !attached || !current(link) {
                        return;
                    }
                    let title = link.numbered(cx);
                    session.update(cx, |s, cx| {
                        s.set_title(title, cx);
                        if row["exited"] == true {
                            s.set_exited(-1, cx);
                        }
                    });
                    let pty = SharedString::from(pty.clone());
                    let kept = cx.default_global::<Links>().1.remove(&pty);
                    let tag = link
                        .wanted
                        .remove(&pty)
                        .or(kept)
                        .unwrap_or_else(|| row["thread"].as_str().map_or(Tag::None, |t| Tag::Thread(t.to_owned())));
                    link.list(session, tag, cx);
                }) else {
                    return;
                };
            }
        }
        _ = this.update(cx, |link, cx| {
            if current(link) {
                if link.watching_sys {
                    link.ask_sys_watch(cx);
                }
                link.set_status(Status::Live, cx);
            }
        });
    }

    fn open_failed(&mut self, failure: Failure, cx: &mut Context<Self>) {
        let kind = failure.kind.as_deref();
        if kind == Some("reauth") {
            self.set_status(Status::Reauth, cx);
            self.schedule_retry(None, cx);
        } else if kind.is_some_and(|k| !PASSING_KINDS.contains(&k)) {
            self.set_status(Status::Refused(failure.message.into()), cx);
            self.schedule_retry(Some(REFUSED_ATTEMPT), cx);
        } else if matches!(self.status, Status::Refused(_)) {
            // Nothing got past the door, so a failure of the host's own does not turn a refusal into a reconnect.
            self.schedule_retry(Some(REFUSED_ATTEMPT), cx);
        } else {
            let status = self.dialing();
            self.set_status(status, cx);
            self.schedule_retry(None, cx);
        }
    }

    /// Lets go of the channel; its pump stops, and with it asks the host to end the channel unless the host ended it.
    fn drop_channel(&mut self, cx: &mut Context<Self>) {
        self.proven = false;
        self.writer = None;
        self.barrier = None;
        self.awaiting_pong = false;
        let tasks = std::mem::take(&mut self.tasks);
        // The batch being handled may come from one of these tasks, so they stop once this update is over.
        cx.defer(move |_| drop(tasks));
        self.dial += 1;
    }

    /// The host or the daemon closed the channel: dial again, the token refused or not.
    fn lost_channel(&mut self, reauth: bool, cx: &mut Context<Self>) {
        self.drop_channel(cx);
        let status = if reauth { Status::Reauth } else { self.dialing() };
        self.set_status(status, cx);
        self.schedule_retry(None, cx);
    }

    /// A channel no longer trusted: the host is asked to end it, and the next dial opens a fresh one.
    fn abandon(&mut self, cx: &mut Context<Self>) {
        self.drop_channel(cx);
        let status = self.dialing();
        self.set_status(status, cx);
        self.schedule_retry(None, cx);
    }

    fn schedule_retry(&mut self, at: Option<u32>, cx: &mut Context<Self>) {
        let attempt = at.unwrap_or_else(|| {
            self.attempt += 1;
            self.attempt
        });
        self.retry = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(backoff(attempt)).await;
            _ = this.update(cx, |link, cx| link.connect(cx));
        }));
    }

    /// Drops the channel at once as a socket that closed would, for a test that checks the re-attach.
    #[doc(hidden)]
    pub fn drop_for_test(&mut self, cx: &mut Context<Self>) {
        self.abandon(cx);
    }

    /// Keeps `memory` as if the daemon had sent it, for a test of a reading no test machine reaches.
    #[doc(hidden)]
    pub fn sample_for_test(&mut self, memory: Memory, cx: &mut Context<Self>) {
        self.keep_reading(memory, cx);
    }

    /// Lets the channel go at once as a link nothing held for its release time would, for a test of what it leaves.
    #[doc(hidden)]
    pub fn release_for_test(&mut self, cx: &mut Context<Self>) {
        self.park(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backoff_doubles_from_half_a_second_to_ten() {
        let ms = |a| backoff(a).as_millis() as u64;
        assert!((500..750).contains(&ms(1)), "{}", ms(1));
        assert!((1000..1250).contains(&ms(2)));
        assert!((8000..8250).contains(&ms(5)));
        assert!((10_000..10_250).contains(&ms(6)));
        assert!((10_000..10_250).contains(&ms(REFUSED_ATTEMPT)));
    }

    #[test]
    fn a_close_that_does_not_read_still_closes_its_channel() {
        let closed = json!({ "type": "daemon.closed", "channel": "c1" });
        assert_eq!(on_channel(Ok(Event::Other(closed.clone())), "c1"), Some(closed.clone()));
        assert_eq!(on_channel(Ok(Event::Other(closed)), "c2"), None, "another channel's");
        assert_eq!(on_channel(Ok(Event::Other(json!({ "type": "thread.head", "channel": "c1" }))), "c1"), None);
    }
}
