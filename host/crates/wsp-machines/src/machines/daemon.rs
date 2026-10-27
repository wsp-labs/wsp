// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/daemon.ts` at 65f6b89c7: what the runtime settles with a machine's daemon, its
//! revive, and the detached send. Spec: spec/daemon.md.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;

use wsp_core::types::internal::{KindModule, MachineMoment};
use wsp_core::types::pending::Clock;
use wsp_core::types::wiring::{WorkspaceRecord, DAEMON_LACKS_AGAIN_MS, DAEMON_REVIVE_AGAIN_MS};
use wsp_engine::land_bytes::lands_bytes;
use wsp_engine::machine::MachineBackend;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::SessionStatus;
use wsp_wire::views::session::{SessionView, ThreadCapWait};
use wsp_wire::views::workspace::{Caller, ReachState, WorkspacePhase};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{WorkspaceStatus, WorkspaceViewDaemonRefusedAt};
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::date_parse::date_parse;
use wsp_words::exit::HostError;
use wsp_words::js::number_string;
use wsp_words::js::utf16::tail;
use wsp_words::js_date::iso_string;
use wsp_words::projects::runs_in_folder;
use wsp_words::words::computer::{machine_lacks_line, machine_never_answered, thread_word};
use wsp_words::words::thread::{
    DAEMON_INSTALLING, DAEMON_INSTALL_FAILED, DAEMON_RESTARTING, DAEMON_RESTART_FAILED,
    DAEMON_UPDATE_FAILED, DAEMON_UPDATING,
};

use super::machines::{Entry, Ids, Row, Unreached};
use crate::unwound::unwound_or_failed;

pub use wsp_core::context::{DetachedOutcome, DetachedSend};

/// What `sessions.start` is asked by a detached send (daemon.ts:32).
pub struct DetachedStart {
    pub prompt: String,
    pub thread: Option<String>,
    pub request_id: String,
    /// `onHeld`: the row and the wait, the moment the computer holds the message back.
    pub on_held: Box<dyn Fn(SessionView, ThreadCapWait)>,
}

/// What `sessions.start` answers once the turn is on its way (its `outcome` and `view()`).
#[derive(Clone, Debug, PartialEq)]
pub struct Started {
    pub outcome: SessionStartOutcome,
    pub view: SessionView,
}

/// Every call the daemon area makes on the rest of the runtime.
#[async_trait(?Send)]
pub trait DaemonRuntime {
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    fn warn(&self, line: &str);
    /// `ctx.daemonHelloTimeoutMs`.
    fn daemon_hello_timeout_ms(&self) -> u64;
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>>;
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>>;
    /// `ctx.daemonNotes`.
    fn daemon_notes(&self) -> Rc<RefCell<IndexMap<String, String>>>;
    /// `ctx.state.closed`.
    fn closed(&self) -> bool;
    async fn sessions_start(
        &self,
        workspace_id: &str,
        ask: DetachedStart,
        origin: Option<&Caller>,
    ) -> Result<Started, HostError>;
    async fn push_status(&self, entry: &Entry) -> Result<(), HostError>;
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError>;
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String>;
    fn module_of(&self, kind: &str) -> Result<Rc<dyn KindModule>, HostError>;
    /// `ctx.projectHeld(id).path`.
    fn project_path(&self, project_id: &str) -> Result<String, HostError>;
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError>;
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError>;
}

/// A bus wait: told each event, and it answers true once it is done and should go.
type Listener = Box<dyn Fn(&EventUnion) -> bool>;

/// A keyed run's shared future and the sequence number it was started under, so a run ends only its own key.
type Join<T> = (u64, Shared<LocalBoxFuture<'static, T>>);

/// The daemon area: the maps and joins daemon.ts keeps in its closure.
pub struct DaemonArea {
    pub(crate) rt: Rc<dyn DaemonRuntime>,
    /// `rootsWrites`: the tail of each machine's roots chain.
    pub(crate) roots_writes: RefCell<IndexMap<String, Join<()>>>,
    /// `daemonSyncs`: the sync each machine has in flight.
    pub(crate) daemon_syncs: RefCell<IndexMap<String, Join<Result<(), HostError>>>>,
    /// `daemonRevivals`.
    pub(crate) daemon_revivals: RefCell<IndexMap<String, Join<()>>>,
    pub(crate) revived_at: Rc<RefCell<IndexMap<String, MachineMoment>>>,
    pub(crate) unread_at: Rc<RefCell<IndexMap<String, MachineMoment>>>,
    pub(crate) unreached: Rc<RefCell<IndexMap<String, Unreached>>>,
    /// The waits on a turn's end and on a queued start's event, woken from `on_event`.
    pub(crate) listeners: RefCell<Vec<Listener>>,
    pub(crate) seq: Cell<u64>,
}

enum First {
    Started(Result<Started, HostError>),
    Queued(String),
    Held(SessionView, ThreadCapWait),
}

/// The queued and held answers are two of three ways a send first answers: a dropped sender means that way did not
/// happen (the start returned, or another way won), so it stays pending for good, as a promise nobody settles, and
/// the other arms decide.
async fn answered<T>(rx: oneshot::Receiver<T>) -> T {
    match rx.await {
        Ok(value) => value,
        Err(_) => std::future::pending().await,
    }
}

/// The start's own answer: its sender is dropped unsent only when the task ended without answering (aborted, or a
/// panic past `panic_words`), which the TS shows as a rejected promise, so it is an error and never a wait.
async fn started_or_ended(
    rx: oneshot::Receiver<Result<Started, HostError>>,
) -> Result<Started, HostError> {
    match rx.await {
        Ok(answer) => answer,
        Err(_) => Err(HostError::new("the send ended before it was answered")),
    }
}

/// What a panic carried, as the message of the error a thrown one is in the TS.
fn panic_words(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(words) => *words,
        Err(payload) => payload.downcast_ref::<&str>().map_or_else(
            || "the send panicked".to_string(),
            |words| (*words).to_string(),
        ),
    }
}

/// Runs a keyed run's synchronous part now, as an async function does, then keeps it running on the local task set.
fn start<T: Clone + 'static>(
    join: Shared<LocalBoxFuture<'static, T>>,
    forget: Box<dyn FnOnce()>,
) -> Shared<LocalBoxFuture<'static, T>> {
    let mut first = join.clone();
    let _ = (&mut first).now_or_never();
    let keep = join.clone();
    wsp_engine::local_task::try_spawn_local(async move {
        let _ = keep.await;
        forget();
    });
    join
}

fn forget_join<T>(map: &RefCell<IndexMap<String, Join<T>>>, key: &str, seq: u64) {
    let mut map = map.borrow_mut();
    if map.get(key).is_some_and(|(current, _)| *current == seq) {
        map.shift_remove(key);
    }
}

fn kind_word(kind: &WorkspaceKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn snapshot(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

fn machine_id_of(entry: &Entry) -> String {
    entry.borrow().machine.id().to_string()
}

fn turn_on_sessions(
    sessions: &RefCell<IndexMap<String, Row>>,
    workspace_id: &str,
    count_queued: bool,
) -> bool {
    sessions.borrow().values().any(|row| {
        let row = row.borrow();
        row.view.workspace_id == workspace_id
            && matches!(row.view.status, SessionStatus::Running)
            && (count_queued || !row.queued)
    })
}

impl DaemonArea {
    /// `daemonArea(ctx)` (daemon.ts:9).
    pub fn new(rt: Rc<dyn DaemonRuntime>) -> Rc<Self> {
        Rc::new_cyclic(|_: &Weak<DaemonArea>| DaemonArea {
            rt,
            roots_writes: RefCell::default(),
            daemon_syncs: RefCell::default(),
            daemon_revivals: RefCell::default(),
            revived_at: Rc::default(),
            unread_at: Rc::default(),
            unreached: Rc::default(),
            listeners: RefCell::default(),
            seq: Cell::new(0),
        })
    }

    /// The bus events this area waits on (`session.queued`, `session.done`, `session.end`), called by the wiring for
    /// every event in emit order.
    pub fn on_event(&self, event: &EventUnion) {
        let mut current = std::mem::take(&mut *self.listeners.borrow_mut());
        current.retain(|listener| !listener(event));
        let added = std::mem::take(&mut *self.listeners.borrow_mut());
        current.extend(added);
        *self.listeners.borrow_mut() = current;
    }

    fn next_seq(&self) -> u64 {
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        seq
    }

    fn turn_on_machine(&self, workspace_id: &str) -> bool {
        turn_on_sessions(&self.rt.sessions(), workspace_id, false)
    }

    /// `sendDetached` (daemon.ts:14), spec rule 1.
    pub async fn send_detached(
        self: &Rc<Self>,
        workspace_id: &str,
        prompt: &str,
        thread: Option<&str>,
        origin: Option<&Caller>,
    ) -> Result<DetachedSend, HostError> {
        let request_id = self.rt.ids().uuid();
        let (queued_tx, queued_rx) = oneshot::channel::<String>();
        let queued_tx = Rc::new(RefCell::new(Some(queued_tx)));
        let (held_tx, held_rx) = oneshot::channel::<(SessionView, ThreadCapWait)>();
        let held_tx = Rc::new(RefCell::new(Some(held_tx)));
        let closed = Rc::new(Cell::new(false));
        {
            let (queued_tx, closed, id) = (queued_tx.clone(), closed.clone(), request_id.clone());
            self.listeners
                .borrow_mut()
                .push(Box::new(move |event: &EventUnion| {
                    if closed.get() {
                        return true;
                    }
                    let EventUnion::SessionQueued(queued) = event else {
                        return false;
                    };
                    if queued.event.request_id.as_deref() != Some(id.as_str()) {
                        return false;
                    }
                    let harness = queued.event.harness.clone();
                    let sender = queued_tx.borrow_mut().take();
                    if let Some(sender) = sender {
                        let _ = sender.send(harness);
                    }
                    true
                }));
        }
        let on_held_tx = held_tx.clone();
        let on_held: Box<dyn Fn(SessionView, ThreadCapWait)> = Box::new(move |view, wait| {
            let sender = on_held_tx.borrow_mut().take();
            if let Some(sender) = sender {
                let _ = sender.send((view, wait));
            }
        });
        let (started_tx, started_rx) = oneshot::channel::<Result<Started, HostError>>();
        let rt = self.rt.clone();
        let ask = DetachedStart {
            prompt: prompt.to_string(),
            thread: thread.map(str::to_string),
            request_id: request_id.clone(),
            on_held,
        };
        let caller = origin.cloned();
        let ws = workspace_id.to_string();
        let thread_said = thread_word(thread.unwrap_or(""));
        wsp_engine::local_task::try_spawn_local(async move {
            let run = std::panic::AssertUnwindSafe(rt.sessions_start(&ws, ask, caller.as_ref()))
                .catch_unwind();
            let answer = match run.await {
                Ok(answer) => answer,
                Err(payload) => Err(HostError::new(panic_words(payload))),
            };
            if started_tx.is_canceled() {
                if let Err(e) = &answer {
                    rt.warn(&format!(
                        "a message waiting in thread {thread_said} was not sent: {}",
                        e.message
                    ));
                }
            }
            let _ = started_tx.send(answer);
        });
        let first = tokio::select! {
            biased;
            answer = started_or_ended(started_rx) => First::Started(answer),
            harness = answered(queued_rx) => First::Queued(harness),
            (view, wait) = answered(held_rx) => First::Held(view, wait),
        };
        closed.set(true);
        match first {
            First::Started(answer) => {
                let started = answer?;
                let outcome = match started.outcome {
                    SessionStartOutcome::Steered => DetachedOutcome::Steered,
                    _ => DetachedOutcome::Started,
                };
                let view = started.view;
                Ok(DetachedSend {
                    outcome,
                    thread_id: view.thread_id.clone().unwrap_or(view.id.clone()),
                    harness: view.harness.clone(),
                    capped: None,
                })
            }
            First::Queued(harness) => Ok(DetachedSend {
                outcome: DetachedOutcome::Queued,
                thread_id: thread.unwrap_or("").to_string(),
                harness,
                capped: None,
            }),
            First::Held(view, wait) => Ok(DetachedSend {
                outcome: DetachedOutcome::Held,
                thread_id: view.thread_id.clone().unwrap_or(view.id.clone()),
                harness: view.harness.clone(),
                capped: Some(wait),
            }),
        }
    }

    /// `toFirstThread` (daemon.ts:48), spec rule 2.
    pub async fn to_first_thread(
        self: &Rc<Self>,
        workspace_id: &str,
        prompt: &str,
        origin: Option<&Caller>,
    ) -> Result<DetachedSend, HostError> {
        let first = {
            let sessions = self.rt.sessions();
            let rows = sessions.borrow();
            let mut first: Option<SessionView> = None;
            for row in rows.values() {
                let view = row.borrow().view.clone();
                if view.workspace_id != workspace_id || view.thread_id.is_none() {
                    continue;
                }
                let at = view.started_at.map_or(0.0, |at| at.0);
                let earlier = first
                    .as_ref()
                    .is_none_or(|best| at < best.started_at.map_or(0.0, |best_at| best_at.0));
                if earlier {
                    first = Some(view);
                }
            }
            first
        };
        let thread = first.and_then(|view| view.thread_id);
        self.send_detached(workspace_id, prompt, thread.as_deref(), origin)
            .await
    }

    /// `noteDaemon` (daemon.ts:54): sets or clears the note, then pushes the row.
    async fn note_daemon(&self, entry: &Entry, note: Option<&str>) -> Result<(), HostError> {
        let id = entry.borrow().record.borrow().id.clone();
        let notes = self.rt.daemon_notes();
        {
            let mut notes = notes.borrow_mut();
            match note {
                None => {
                    notes.shift_remove(&id);
                }
                Some(note) => {
                    notes.insert(id, note.to_string());
                }
            }
        }
        self.rt.push_status(entry).await
    }

    /// `flashDaemon` (daemon.ts:63): a note that rides one status and no more.
    async fn flash_daemon(&self, entry: &Entry, note: &str) -> Result<(), HostError> {
        let id = entry.borrow().record.borrow().id.clone();
        let notes = self.rt.daemon_notes();
        notes.borrow_mut().insert(id.clone(), note.to_string());
        self.rt.push_status(entry).await?;
        notes.borrow_mut().shift_remove(&id);
        Ok(())
    }

    /// `rootsWrite` (daemon.ts:72), spec rule 4: each write of a machine's roots starts after the last one settled.
    fn roots_write(
        self: &Rc<Self>,
        machine_id: &str,
        work: LocalBoxFuture<'static, Result<(), HostError>>,
    ) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let prev = self
            .roots_writes
            .borrow()
            .get(machine_id)
            .map(|(_, tail)| tail.clone());
        let (tx, rx) = oneshot::channel::<Result<(), HostError>>();
        let body: LocalBoxFuture<'static, ()> = async move {
            if let Some(prev) = prev {
                prev.await;
            }
            let _ = tx.send(unwound_or_failed(work).await);
        }
        .boxed_local();
        let seq = self.next_seq();
        let tail = body.shared();
        self.roots_writes
            .borrow_mut()
            .insert(machine_id.to_string(), (seq, tail.clone()));
        let me = Rc::downgrade(self);
        let key = machine_id.to_string();
        drop(start(
            tail,
            Box::new(move || {
                if let Some(area) = me.upgrade() {
                    forget_join(&area.roots_writes, &key, seq);
                }
            }),
        ));
        async move {
            match rx.await {
                Ok(answer) => answer,
                Err(_) => Err(HostError::new("the roots write was dropped")),
            }
        }
        .boxed_local()
    }

    /// The roots file's content for one write (daemon.ts:88-110), each step a return point.
    async fn roots_work(&self, entry: &Entry, strict: bool) -> Result<(), HostError> {
        if self.rt.closed() {
            return Ok(());
        }
        let record = snapshot(entry);
        if self.rt.served_by_its_computer(entry).is_some() && !runs_in_folder(&record.kind) {
            return Ok(());
        }
        let sharing: Vec<Entry> = self.rt.live().borrow().values().cloned().collect();
        let mut dests: Vec<String> = Vec::new();
        for other in &sharing {
            let other = snapshot(other);
            if other.machine_id != record.machine_id {
                continue;
            }
            let project_dest = self.rt.project_path(&other.project)?;
            let checkout_dest = self.rt.checkout_of(&other)?;
            for dest in [project_dest, checkout_dest]
                .into_iter()
                .chain(other.landed.clone().unwrap_or_default())
            {
                if !dests.contains(&dest) {
                    dests.push(dest);
                }
            }
        }
        let module = self.rt.module_of(&kind_word(&record.kind))?;
        let written = module.roots(&entry.borrow(), &dests);
        if strict {
            return written.await;
        }
        if let Err(e) = written.await {
            let machine = machine_id_of(entry);
            self.rt.warn(&format!(
                "browsable folders for {} not written on {machine}: {}",
                record.id,
                tail(&e.message, 200)
            ));
        }
        Ok(())
    }

    /// `writeDaemonRoots` (daemon.ts:88), spec rule 5.
    pub async fn write_daemon_roots(
        self: &Rc<Self>,
        entry: &Entry,
        strict: bool,
    ) -> Result<(), HostError> {
        let machine_id = snapshot(entry).machine_id;
        let me = self.clone();
        let work_entry = entry.clone();
        let work = async move { me.roots_work(&work_entry, strict).await }.boxed_local();
        self.roots_write(&machine_id, work).await
    }

    /// `turnRuns` (daemon.ts:111): a queued row counts.
    pub fn turn_runs(&self, workspace_id: &str) -> bool {
        turn_on_sessions(&self.rt.sessions(), workspace_id, true)
    }

    /// `whenNoTurnRuns` (daemon.ts:118), spec rule 6: resolves once no turn runs on the machine.
    async fn when_no_turn_runs(&self, workspace_id: &str) {
        if !self.turn_on_machine(workspace_id) {
            return;
        }
        let (tx, rx) = oneshot::channel::<()>();
        let tx = Rc::new(RefCell::new(Some(tx)));
        let ws = workspace_id.to_string();
        let sessions = self.rt.sessions();
        self.listeners
            .borrow_mut()
            .push(Box::new(move |event: &EventUnion| {
                if !matches!(
                    event,
                    EventUnion::SessionDone(_) | EventUnion::SessionEnd(_)
                ) {
                    return false;
                }
                if turn_on_sessions(&sessions, &ws, false) {
                    return false;
                }
                let sender = tx.borrow_mut().take();
                if let Some(sender) = sender {
                    let _ = sender.send(());
                }
                true
            }));
        let _ = rx.await;
    }

    /// `canDeployDaemon` (daemon.ts:135), spec rule 7.
    fn can_deploy(&self, entry: &Entry) -> Result<bool, HostError> {
        if self.rt.served_by_its_computer(entry).is_some() {
            return Ok(false);
        }
        let record = snapshot(entry);
        let module = self.rt.module_of(&kind_word(&record.kind))?;
        let has_deploy = module.deploy_daemon(&entry.borrow()).is_some();
        if !has_deploy {
            return Ok(false);
        }
        let backend: Arc<dyn MachineBackend> = self.rt.backend_for(&record)?;
        let machine = entry.borrow().machine.clone();
        Ok(lands_bytes(&backend.capabilities(), machine.as_ref()))
    }

    /// `deployDaemonOn` (daemon.ts:147), spec rule 8: the deploy is the kind module's, called on the entry.
    pub async fn deploy_daemon_on(&self, entry: &Entry) -> Result<Option<String>, HostError> {
        let record = snapshot(entry);
        let module = self.rt.module_of(&kind_word(&record.kind))?;
        let deploy = module
            .deploy_daemon(&entry.borrow())
            .ok_or_else(|| HostError::new("this kind has no daemon to deploy"))?;
        match deploy.await {
            Ok(detail) => {
                self.forget_refusal(entry).await?;
                Ok(detail)
            }
            Err(e) => {
                if let Some(lacks) = machine_lacks_line(&e) {
                    let at = iso_string(self.rt.clock().now() as f64).unwrap_or_default();
                    let refused = WorkspaceViewDaemonRefusedAt {
                        machine_id: machine_id_of(entry),
                        at,
                        why: lacks,
                    };
                    entry.borrow().record.borrow_mut().daemon_refused_at = Some(refused);
                    self.rt.persist(&snapshot(entry)).await?;
                } else if !machine_never_answered(&e) {
                    self.forget_refusal(entry).await?;
                }
                Err(e)
            }
        }
    }

    /// The `forget` of `deployDaemonOn` (daemon.ts:150): a mark the record holds comes off.
    async fn forget_refusal(&self, entry: &Entry) -> Result<(), HostError> {
        let record = entry.borrow().record.clone();
        if record.borrow().daemon_refused_at.is_none() {
            return Ok(());
        }
        record.borrow_mut().daemon_refused_at = None;
        self.rt.persist(&snapshot(entry)).await
    }

    /// `lacksSaid` (daemon.ts:174), spec rule 9.
    async fn lacks_said(
        &self,
        entry: &Entry,
    ) -> Result<Option<WorkspaceViewDaemonRefusedAt>, HostError> {
        let record = entry.borrow().record.clone();
        let refused = record.borrow().daemon_refused_at.clone();
        let Some(refused) = refused else {
            return Ok(None);
        };
        if refused.machine_id == machine_id_of(entry) {
            return Ok(Some(refused));
        }
        record.borrow_mut().daemon_refused_at = None;
        self.rt.persist(&snapshot(entry)).await?;
        Ok(None)
    }

    /// `syncDaemon` (daemon.ts:190), spec rule 10; joined per machine. It never rejects for a deploy that failed.
    pub fn sync_daemon(
        self: &Rc<Self>,
        entry: &Entry,
    ) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let key = machine_id_of(entry);
        if let Some(held) = self
            .daemon_syncs
            .borrow()
            .get(&key)
            .map(|(_, join)| join.clone())
        {
            return held.boxed_local();
        }
        let seq = self.next_seq();
        let me = Rc::downgrade(self);
        let work_entry = entry.clone();
        let body: LocalBoxFuture<'static, Result<(), HostError>> = async move {
            match me.upgrade() {
                Some(area) => unwound_or_failed(area.run_sync(&work_entry)).await,
                None => Ok(()),
            }
        }
        .boxed_local();
        let join = body.shared();
        self.daemon_syncs
            .borrow_mut()
            .insert(key.clone(), (seq, join.clone()));
        let me = Rc::downgrade(self);
        let forget_key = key.clone();
        let join = start(
            join,
            Box::new(move || {
                if let Some(area) = me.upgrade() {
                    forget_join(&area.daemon_syncs, &forget_key, seq);
                }
            }),
        );
        join.boxed_local()
    }

    /// The steps of a sync (daemon.ts:190-260), each a return point.
    async fn run_sync(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        let record = snapshot(entry);
        let (id, key) = (record.id.clone(), machine_id_of(entry));
        let module = self.rt.module_of(&kind_word(&record.kind))?;
        if self.rt.served_by_its_computer(entry).is_some() {
            if runs_in_folder(&record.kind) {
                self.write_daemon_roots(entry, false).await?;
            }
            return Ok(());
        }
        let placing = !module.has_daemon(&entry.borrow());
        if let Some(said) = self.lacks_said(entry).await? {
            let now = self.rt.clock().now() as f64;
            if now - date_parse(&said.at) < DAEMON_LACKS_AGAIN_MS as f64 {
                return Ok(());
            }
        }
        self.write_daemon_roots(entry, false).await?;
        if !self.can_deploy(entry)? {
            return Ok(());
        }
        let version_read = module.daemon_version(&entry.borrow());
        let version = version_read.await?;
        let Some(version) = version else {
            let now = self.rt.clock().now() as f64;
            self.unread_at.borrow_mut().insert(
                id.clone(),
                MachineMoment {
                    machine_id: key.clone(),
                    at: JsNumber(now),
                },
            );
            let seconds = self.rt.daemon_hello_timeout_ms() as f64 / 1000.0;
            self.rt.warn(&format!("daemon on {key} (workspace {id}): version not read within {} s; asking again at the next reach probe", number_string(seconds)));
            return Ok(());
        };
        self.unread_at.borrow_mut().shift_remove(&id);
        if i64::from(version) >= DAEMON_VERSION {
            return Ok(());
        }
        if self.turn_on_machine(&id) {
            self.rt.warn(&format!(
                "daemon on {key} (workspace {id}): update waits for the running turn"
            ));
        }
        self.when_no_turn_runs(&id).await;
        if entry.borrow().record.borrow().phase != WorkspacePhase::Running {
            return Ok(());
        }
        let note = if placing {
            DAEMON_INSTALLING
        } else {
            DAEMON_UPDATING
        };
        self.note_daemon(entry, Some(note)).await?;
        while self.turn_on_machine(&id) {
            self.when_no_turn_runs(&id).await;
        }
        if entry.borrow().record.borrow().phase != WorkspacePhase::Running {
            self.note_daemon(entry, None).await?;
            return Ok(());
        }
        let deployed: Result<(), HostError> = async {
            self.deploy_daemon_on(entry).await?;
            self.write_daemon_roots(entry, false).await?;
            self.note_daemon(entry, None).await
        }
        .await;
        if let Err(e) = deployed {
            self.note_daemon(entry, None).await?;
            if entry.borrow().record.borrow().phase != WorkspacePhase::Running {
                return Ok(());
            }
            let verb = if placing {
                "not installed"
            } else {
                "not updated"
            };
            self.rt.warn(&format!(
                "daemon on {} (workspace {id}) {verb}: {}",
                machine_id_of(entry),
                e.message
            ));
            let fallback = if placing {
                DAEMON_INSTALL_FAILED
            } else {
                DAEMON_UPDATE_FAILED
            };
            let line = machine_lacks_line(&e).unwrap_or_else(|| fallback.to_string());
            self.flash_daemon(entry, &line).await?;
        }
        Ok(())
    }

    /// `daemonSettled` (daemon.ts:262), spec rule 11.
    pub async fn daemon_settled(&self, entry: &Entry) {
        let (id, key) = (snapshot(entry).id, machine_id_of(entry));
        let sync = self
            .daemon_syncs
            .borrow()
            .get(&key)
            .map(|(_, join)| join.clone());
        if let Some(sync) = sync {
            if !self.turn_on_machine(&id) {
                let _ = sync.await;
            }
        }
    }

    pub fn revived_at(&self) -> IndexMap<String, MachineMoment> {
        self.revived_at.borrow().clone()
    }

    /// The map itself, which a deleted workspace's id leaves (machines.ts:540).
    pub fn revived_at_held(&self) -> Rc<RefCell<IndexMap<String, MachineMoment>>> {
        self.revived_at.clone()
    }

    pub fn unread_at(&self) -> Rc<RefCell<IndexMap<String, MachineMoment>>> {
        self.unread_at.clone()
    }

    /// `unreached`: the map the machines area writes and this one reads.
    pub fn unreached(&self) -> Rc<RefCell<IndexMap<String, Unreached>>> {
        self.unreached.clone()
    }

    /// The machines with a sync in flight (`daemonSyncs.keys()`).
    pub fn daemon_syncs(&self) -> Vec<String> {
        self.daemon_syncs.borrow().keys().cloned().collect()
    }

    /// `unreachedOf` (daemon.ts:274), spec rule 12.
    pub fn unreached_of(&self, entry: &Entry) -> Option<String> {
        let id = snapshot(entry).id;
        let mark = self.unreached.borrow().get(&id).cloned()?;
        if mark.machine_id == machine_id_of(entry) {
            return Some(mark.line);
        }
        self.unreached.borrow_mut().shift_remove(&id);
        None
    }

    /// `reviveDaemon` (daemon.ts:292), spec rule 13: starts its work and returns.
    pub fn revive_daemon(
        self: &Rc<Self>,
        entry: &Entry,
        reach: ReachState,
    ) -> Result<(), HostError> {
        if reach != ReachState::NoDaemon
            || entry.borrow().record.borrow().phase != WorkspacePhase::Running
        {
            return Ok(());
        }
        if !self.can_deploy(entry)? || self.unreached_of(entry).is_some() {
            return Ok(());
        }
        let record = snapshot(entry);
        let key = machine_id_of(entry);
        if self.daemon_revivals.borrow().contains_key(&key) {
            return Ok(());
        }
        let now = self.rt.clock().now() as f64;
        let recent = self
            .revived_at
            .borrow()
            .get(&record.id)
            .is_some_and(|last| {
                last.machine_id == key && now - last.at.0 < DAEMON_REVIVE_AGAIN_MS as f64
            });
        if recent {
            return Ok(());
        }
        self.revived_at.borrow_mut().insert(
            record.id.clone(),
            MachineMoment {
                machine_id: key.clone(),
                at: JsNumber(now),
            },
        );
        let seq = self.next_seq();
        let me = Rc::downgrade(self);
        let work_entry = entry.clone();
        let body: LocalBoxFuture<'static, ()> = async move {
            if let Some(area) = me.upgrade() {
                let _ = unwound_or_failed(area.run_revive(&work_entry)).await;
            }
        }
        .boxed_local();
        let join = body.shared();
        self.daemon_revivals
            .borrow_mut()
            .insert(key.clone(), (seq, join.clone()));
        let me = Rc::downgrade(self);
        let forget_key = key.clone();
        drop(start(
            join,
            Box::new(move || {
                if let Some(area) = me.upgrade() {
                    forget_join(&area.daemon_revivals, &forget_key, seq);
                }
            }),
        ));
        Ok(())
    }

    /// The work of a revive (daemon.ts:309-330).
    async fn run_revive(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        let (id, key) = (snapshot(entry).id, machine_id_of(entry));
        self.note_daemon(entry, Some(DAEMON_RESTARTING)).await?;
        if entry.borrow().record.borrow().phase != WorkspacePhase::Running {
            self.note_daemon(entry, None).await?;
            return Ok(());
        }
        let restarted: Result<(), HostError> = async {
            self.deploy_daemon_on(entry).await?;
            self.write_daemon_roots(entry, false).await?;
            self.note_daemon(entry, None).await
        }
        .await;
        if let Err(e) = restarted {
            self.note_daemon(entry, None).await?;
            if entry.borrow().record.borrow().phase != WorkspacePhase::Running {
                return Ok(());
            }
            self.rt.warn(&format!(
                "daemon on {key} (workspace {id}) not restarted: {}",
                e.message
            ));
            self.flash_daemon(entry, DAEMON_RESTART_FAILED).await?;
        }
        Ok(())
    }

    /// `offerDaemonAgain` (daemon.ts:343), spec rule 14.
    pub fn offer_daemon_again(self: &Rc<Self>, entry: &Entry, polled: &WorkspaceStatus) {
        let refused = entry.borrow().record.borrow().daemon_refused_at.is_some();
        if !refused || polled.facts.is_none() {
            return;
        }
        drop(self.sync_daemon(entry));
    }

    /// `readVersionAgain` (daemon.ts:349), spec rule 15.
    pub fn read_version_again(self: &Rc<Self>, entry: &Entry, polled: &WorkspaceStatus) {
        let id = snapshot(entry).id;
        let unread = self.unread_at.borrow().get(&id).cloned();
        let Some(unread) = unread else { return };
        if unread.machine_id != machine_id_of(entry) {
            self.unread_at.borrow_mut().shift_remove(&id);
            return;
        }
        let now = self.rt.clock().now() as f64;
        if polled.phase != WorkspacePhase::Running
            || polled.reach.state != ReachState::Reachable
            || now - unread.at.0 < DAEMON_REVIVE_AGAIN_MS as f64
        {
            return;
        }
        drop(self.sync_daemon(entry));
    }
}
