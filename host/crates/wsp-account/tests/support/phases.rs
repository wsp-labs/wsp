// SPDX-License-Identifier: AGPL-3.0-only
//! `FakePhases`: the phases half's context over a `FakeBase`. Every method logs `self.base.call(<its name>, <args>)`.
//! Scalars and refusals come from the base script under the method's name (`$error` is the refusal); the engine's own
//! answers (`ws_wake`, `ws_rebuild`, `ws_upgrade`, `machine_state`, `machine_kill`, `unfork`) come from queues of
//! `Result<_, EngineError>` the test sets, the last answer staying. A wake can be held on a oneshot, and a held wake
//! ends when the entry's stop token is pulled, as the engine's call ends on an abort. `sleeps` waits on the test clock,
//! and a sleep dropped before it fires cancels its timer. Every call out checks that no borrow of a live entry is held.
#![allow(dead_code)]
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use async_trait::async_trait;
use futures::channel::oneshot;
use serde_json::{json, Value};
use wsp_account::projects::workspaces::phases::{MachineRead, PhasesCtx, ResumeAsks};
use wsp_account::projects::workspaces::seam::{
    Base, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld,
};
use wsp_core::clock::{Cancel, Clock};
use wsp_core::ids::Ids;
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::{SizeAsk, WorkspaceRecord};
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_engine::lifecycle::WakeResult;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView, WorkspacePhase};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::capabilities::Capabilities;
use wsp_wire::wire::machine_link::{ExecResult, MachineShape};
use wsp_words::exit::{HostError, Kind};

pub use crate::base::{person as the_person, thread as asker};
use crate::base::{kinded, merge, view_of, BaseOf, ClockOf, FakeBase};

/// `refusalLine(happened, fix)` (thread.ts:393).
pub fn refusal_line(happened: &str, fix: &str) -> String {
    let said = happened.trim_end();
    let stop = said.ends_with(['.', '!', '?', ':']);
    format!("{said}{} {fix}", if stop { "" } else { "." })
}

/// Capabilities offering two sizes, 2x4 at $0.05 an hour and 4x8 at $0.10 (what `backend_capabilities` answers).
pub fn caps_with_sizes() -> Capabilities {
    serde_json::from_value(json!({
        "liveCloneForks": false, "replacesMachine": true, "previewUrls": false, "signedUrls": false, "callbackRelay": false,
        "diskSnapshots": false, "images": true, "snapshotsAnyLife": false, "snapshotListing": false, "templates": false,
        "kept": false, "copies": false, "ownNetwork": false,
        "sizes": [{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.05 }, { "cpu": 4, "memMb": 8192, "rateUsdPerHour": 0.1 }],
    }))
    .expect("capabilities read")
}

pub fn size(cpu: f64, mem_mb: f64) -> SizeAsk {
    SizeAsk { cpu: JsNumber(cpu), mem_mb: JsNumber(mem_mb) }
}

pub fn shape(cpu: f64, mem_mb: f64) -> MachineShape {
    serde_json::from_value(json!({ "cpu": cpu, "memMb": mem_mb })).expect("a shape reads")
}

// ----------------------------------------------------------------------------------------------- engine errors

/// The provider answered 404 for the machine, in the words it said.
pub fn missing() -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::Missing,
        status: 404,
        code: None,
        message: "machine not found".to_owned(),
        request_id: None,
        window: None,
        said: Some("the sandbox is gone".to_owned()),
    })
}

/// A resume the provider never took inside its cap (errors.ts:36).
pub fn unanswered() -> EngineError {
    EngineError::ResumeUnanswered { message: "the provider has not answered the resume request; reading the machine".to_owned(), cause: None }
}

/// The provider deleted the paused machine's disk (errors.ts:95).
pub fn disk_gone_error(machine_id: &str, line: &str, said: &str) -> EngineError {
    EngineError::DiskGone { machine_id: machine_id.to_owned(), line: line.to_owned(), said: said.to_owned() }
}

/// The provider still said not yet at the restore deadline (errors.ts:41); the message is the cause's.
pub fn restore_unfinished(machine_id: &str, message: &str) -> EngineError {
    EngineError::RestoreUnfinished { machine_id: machine_id.to_owned(), message: message.to_owned(), carried: Default::default() }
}

/// Any other thrown Error.
pub fn other(message: &str) -> EngineError {
    EngineError::Other { message: message.to_owned(), code: None }
}

/// golden.ts:45 `MachineAliveError`: a machine that answered three kills and is still there, kind "machineAlive".
/// `EngineError` has no variant for it yet, so it is one place here: this is the one line to change when it has.
pub fn alive(machine_id: &str, state: &str) -> EngineError {
    EngineError::Other {
        message: format!("machine {machine_id} is still {state} after three kills; it bills until reap or a kill by hand takes"),
        code: Some("machineAlive".to_owned()),
    }
}

pub fn done(reason: Option<&str>) -> Result<WakeResult, EngineError> {
    Ok(WakeResult { reason: reason.map(str::to_owned) })
}

/// The last answer of a queue stays once the others are taken.
pub struct Seq<T: Clone> {
    q: RefCell<VecDeque<T>>,
}

impl<T: Clone> Seq<T> {
    pub fn new() -> Seq<T> {
        Seq { q: RefCell::new(VecDeque::new()) }
    }
    pub fn set(&self, items: Vec<T>) {
        *self.q.borrow_mut() = items.into();
    }
    pub fn next(&self) -> Option<T> {
        let mut q = self.q.borrow_mut();
        if q.len() > 1 { q.pop_front() } else { q.front().cloned() }
    }
}

#[derive(Clone)]
pub enum WakePlan {
    Done(Result<WakeResult, EngineError>),
    /// Waits for the sender of `hold_wake`, or for the entry's stop token.
    Hold,
}

pub struct FakePhases {
    pub base: Rc<FakeBase>,
    pub wakes: Seq<WakePlan>,
    pub wake_hold: RefCell<VecDeque<oneshot::Receiver<Result<WakeResult, EngineError>>>>,
    pub rebuilds: Seq<Result<(), EngineError>>,
    pub upgrades: Seq<Result<(), EngineError>>,
    pub states: Seq<Result<MachineRead, EngineError>>,
    pub kills: Seq<Result<(), EngineError>>,
    pub unforks: Seq<Result<(), EngineError>>,
    pub shapes: Seq<Option<MachineShape>>,
    /// The machine id the engine's workspace holds after a rebuild or an upgrade, applied by `follow_machine`.
    pub engine_machine: RefCell<Option<String>>,
    /// What the replacing machine's id is where the engine replaced it.
    pub replacement: RefCell<String>,
    /// `ws.currentPhase`.
    pub engine_phase: Cell<WorkspacePhase>,
    pub caps: RefCell<Capabilities>,
    /// Paths `path_exists` answers true for.
    pub paths: RefCell<Vec<String>>,
    /// Thread ids `thread_runs` answers true for.
    pub running_threads: RefCell<Vec<String>>,
    /// Held until the test sends: what `cut_off` waits on, so a delete stands in flight.
    pub cut_gate: RefCell<Option<oneshot::Receiver<()>>>,
}

impl FakePhases {
    pub fn new() -> Rc<FakePhases> {
        Rc::new(FakePhases {
            base: FakeBase::new(),
            wakes: Seq::new(),
            wake_hold: RefCell::new(VecDeque::new()),
            rebuilds: Seq::new(),
            upgrades: Seq::new(),
            states: Seq::new(),
            kills: Seq::new(),
            unforks: Seq::new(),
            shapes: Seq::new(),
            engine_machine: RefCell::new(None),
            replacement: RefCell::new("m_new".to_owned()),
            engine_phase: Cell::new(WorkspacePhase::Napping),
            caps: RefCell::new(caps_with_sizes()),
            paths: RefCell::new(vec![]),
            running_threads: RefCell::new(vec![]),
            cut_gate: RefCell::new(None),
        })
    }

    pub fn ctx(self: &Rc<Self>) -> Rc<dyn PhasesCtx> {
        self.clone()
    }

    /// The next `ws_wake` waits until the returned sender answers it (or the stop is pulled).
    pub fn hold_wake(&self) -> oneshot::Sender<Result<WakeResult, EngineError>> {
        let (tx, rx) = oneshot::channel();
        self.wake_hold.borrow_mut().push_back(rx);
        tx
    }

    fn reenter(&self) {
        let held: Vec<Entry> = self.base.live.borrow().values().cloned().collect();
        for e in held {
            assert!(e.try_borrow_mut().is_ok(), "a borrow of a live entry is held across a call out");
            let record = e.borrow().record.clone();
            assert!(record.try_borrow_mut().is_ok(), "a borrow of an entry's record is held across a call out");
        }
    }

    fn id_of(entry: &Entry) -> String {
        entry.borrow().record.borrow().id.clone()
    }

    fn with_record<T>(entry: &Entry, f: impl FnOnce(&mut WorkspaceRecord) -> T) -> T {
        let record = entry.borrow().record.clone();
        let mut r = record.borrow_mut();
        f(&mut r)
    }

    fn flag(&self, name: &str, default: bool) -> bool {
        self.base.answer(name).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    fn refusal(&self, name: &str) -> Result<(), HostError> {
        self.base.answer_as::<Value>(name, || Value::Null).map(|_| ())
    }

    fn view(&self, entry: &Entry) -> WorkspaceView {
        let record = entry.borrow().record.borrow().clone();
        view_of(&self.base, &record)
    }
}

#[async_trait(?Send)]
impl Base for FakePhases {
    async fn ready(&self) -> Result<(), HostError> {
        BaseOf(self.base.clone()).ready().await
    }
    fn clock(&self) -> Rc<dyn Clock> {
        BaseOf(self.base.clone()).clock()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        BaseOf(self.base.clone()).ids()
    }
    fn store(&self) -> Rc<dyn Store> {
        BaseOf(self.base.clone()).store()
    }
    fn live(&self) -> Live {
        BaseOf(self.base.clone()).live()
    }
    fn projects_held(&self) -> ProjectsHeld {
        BaseOf(self.base.clone()).projects_held()
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        BaseOf(self.base.clone()).project_held(project_id)
    }
    fn session_views(&self) -> Vec<SessionView> {
        BaseOf(self.base.clone()).session_views()
    }
    fn thread_workspaces(&self) -> Vec<(String, String)> {
        BaseOf(self.base.clone()).thread_workspaces()
    }
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError> {
        self.reenter();
        BaseOf(self.base.clone()).persist(record).await
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        BaseOf(self.base.clone()).view(record)
    }
    fn emit(&self, event: Event) -> Result<(), HostError> {
        self.reenter();
        BaseOf(self.base.clone()).emit(event)
    }
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        BaseOf(self.base.clone()).entry_of(id, origin, ask).await
    }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError> {
        BaseOf(self.base.clone()).spawn_guard(act, origin)
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        BaseOf(self.base.clone()).kind_of(computer)
    }
    fn place_name(&self, place_id: &str) -> String {
        BaseOf(self.base.clone()).place_name(place_id)
    }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        BaseOf(self.base.clone()).copy_blocked(entry).await
    }
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<Value, HostError> {
        BaseOf(self.base.clone()).with_daemon(entry, work).await
    }
    async fn on_this_computer(&self, work: HereWork) -> Result<Value, HostError> {
        BaseOf(self.base.clone()).on_this_computer(work).await
    }
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError> {
        BaseOf(self.base.clone()).git_here(ask).await
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        BaseOf(self.base.clone()).checkout_of(record)
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        BaseOf(self.base.clone()).preferences().await
    }
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError> {
        BaseOf(self.base.clone()).write_daemon_roots(entry, strict).await
    }
    fn home_dir(&self) -> String {
        BaseOf(self.base.clone()).home_dir()
    }
    fn machine_id(&self, entry: &Entry) -> String {
        BaseOf(self.base.clone()).machine_id(entry)
    }
}

#[async_trait(?Send)]
impl PhasesCtx for FakePhases {
    async fn unfork(&self, entry: &Entry) -> Result<(), EngineError> {
        self.reenter();
        self.base.call("unfork", json!({ "id": Self::id_of(entry) }));
        self.unforks.next().unwrap_or(Ok(()))
    }

    async fn sub_lost(&self, entry: &Entry, line: &str) -> Result<(), HostError> {
        self.reenter();
        self.base.call("sub_lost", json!({ "id": Self::id_of(entry), "line": line }));
        self.refusal("sub_lost")
    }

    fn tell_threads(&self, workspace_id: &str, line: &str) {
        self.base.call("tell_threads", json!({ "id": workspace_id, "line": line }));
    }

    async fn emit_status(&self, entry: &Entry, reach: &str, words: Option<&str>) -> Result<(), HostError> {
        self.reenter();
        let (ask, said, phase) = {
            let e = entry.borrow();
            let phase = e.record.borrow().phase;
            (e.wake_ask, e.wake_said.clone(), phase)
        };
        self.base.call(
            "emit_status",
            json!({
                "id": Self::id_of(entry), "reach": reach, "words": words, "phase": serde_json::to_value(phase).unwrap(),
                "wakeAsk": ask.map(|a| json!({ "ask": a.ask, "of": a.of })), "wakeSaid": said,
            }),
        );
        self.refusal("emit_status")
    }

    fn follow_machine(&self, entry: &Entry) {
        self.reenter();
        let next = self.engine_machine.borrow().clone();
        Self::with_record(entry, |r| {
            r.phase = WorkspacePhase::Running;
            if let Some(m) = next {
                r.machine_id = m;
            }
            r.gone = None;
        });
        entry.borrow_mut().unchecked = false;
        self.base.call("follow_machine", json!({ "id": Self::id_of(entry) }));
    }

    fn reach_of(&self, entry: &Entry) -> String {
        self.base.call("reach_of", json!({ "id": Self::id_of(entry) }));
        self.base.answer("reach_of").and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_else(|| "reachable".to_owned())
    }

    async fn nap_with(&self, id: &str) -> Result<WorkspaceView, HostError> {
        self.reenter();
        let entry = self.base.live.borrow().get(id).cloned().expect("nap_with a live entry");
        let wait = entry.borrow().nap_wait_ms;
        self.base.call("nap_with", json!({ "id": id, "napWaitMs": wait }));
        self.refusal("nap_with")?;
        Self::with_record(&entry, |r| r.phase = WorkspacePhase::Napping);
        Ok(self.view(&entry))
    }

    fn refuse_pauseless(&self, entry: &Entry, act: &str) -> Result<(), HostError> {
        self.base.call("refuse_pauseless", json!({ "id": Self::id_of(entry), "act": act }));
        self.refusal("refuse_pauseless")
    }

    fn talks_to_tree_on(&self, id: &str, origin: Option<&Caller>) -> bool {
        self.base.call("talks_to_tree_on", json!({ "id": id }));
        origin.is_some_and(|c| matches!(c, Caller::Scoped { .. })) && self.flag("talks_to_tree_on", false)
    }

    fn refuse_sub_wake(&self, entry: &Entry) -> Result<(), HostError> {
        self.base.call("refuse_sub_wake", json!({ "id": Self::id_of(entry) }));
        self.refusal("refuse_sub_wake")
    }

    async fn recover_gone(&self, entry: &Entry) -> Result<Option<WorkspacePhase>, HostError> {
        self.reenter();
        self.base.call("recover_gone", json!({ "id": Self::id_of(entry) }));
        let left = match self.base.answer_as::<Value>("recover_gone", || Value::Null)? {
            Value::String(s) if s == "running" => Some(WorkspacePhase::Running),
            Value::String(s) if s == "napping" => Some(WorkspacePhase::Napping),
            _ => None,
        };
        if let Some(phase) = left {
            Self::with_record(entry, |r| {
                r.phase = phase;
                r.gone = None;
            });
        }
        Ok(left)
    }

    async fn machine_state(&self, entry: &Entry) -> Result<MachineRead, EngineError> {
        self.reenter();
        self.base.call("machine_state", json!({ "id": Self::id_of(entry) }));
        self.states.next().unwrap_or(Ok(MachineRead::Running))
    }

    fn provider_said(&self, e: &EngineError) -> String {
        match e {
            EngineError::Provider(w) => w.said.clone().unwrap_or_else(|| w.message.clone()),
            other => other.to_string(),
        }
    }

    async fn settle_gone(&self, entry: &Entry, by: &str, at: i64, answer: Option<String>) -> Result<bool, HostError> {
        self.reenter();
        let id = Self::id_of(entry);
        self.base.call("settle_gone", json!({ "id": id, "by": by, "at": at, "answer": answer }));
        let settled = self.base.answer_as::<bool>("settle_gone", || false)?;
        if settled {
            let machine = entry.borrow().record.borrow().machine_id.clone();
            let when = format_seconds(at);
            let said = answer.as_deref().filter(|a| !a.is_empty()).map(|a| format!(" ({a})")).unwrap_or_default();
            Self::with_record(entry, |r| {
                r.phase = WorkspacePhase::Gone;
                r.gone = Some(format!("machine {machine} is gone at the provider: the {by} found it gone at {when}{said}"));
            });
        }
        Ok(settled)
    }

    async fn adopt_pause(&self, entry: &Entry) -> Result<(), HostError> {
        self.reenter();
        self.base.call("adopt_pause", json!({ "id": Self::id_of(entry) }));
        Self::with_record(entry, |r| r.phase = WorkspacePhase::Napping);
        Ok(())
    }

    async fn runs_under_napping(&self, entry: &Entry) -> Result<bool, HostError> {
        self.reenter();
        self.base.call("runs_under_napping", json!({ "id": Self::id_of(entry) }));
        self.base.answer_as::<bool>("runs_under_napping", || false)
    }

    async fn adopt_running(&self, entry: &Entry) -> Result<(), HostError> {
        self.reenter();
        self.base.call("adopt_running", json!({ "id": Self::id_of(entry) }));
        Self::with_record(entry, |r| r.phase = WorkspacePhase::Running);
        Ok(())
    }

    async fn proven(&self, entry: &Entry) -> Result<WorkspaceView, HostError> {
        self.reenter();
        self.base.call("proven", json!({ "id": Self::id_of(entry) }));
        self.refusal("proven")?;
        entry.borrow_mut().unchecked = false;
        Ok(self.view(entry))
    }

    fn resume_asks(&self, entry: &Entry) -> Option<ResumeAsks> {
        self.base.call("resume_asks", json!({ "id": Self::id_of(entry) }));
        let v = self.base.answer("resume_asks")?;
        Some(ResumeAsks { for_ms: v.get("forMs")?.as_i64()?, every_ms: v.get("everyMs")?.as_i64()? })
    }

    async fn sleeps(&self, ms: i64) {
        self.base.call("sleeps", json!({ "ms": ms, "now": ClockOf(self.base.clock.clone()).now() }));
        let (tx, rx) = oneshot::channel::<()>();
        let tx = RefCell::new(Some(tx));
        let cancel = ClockOf(self.base.clock.clone()).schedule(
            Box::new(move || {
                if let Some(t) = tx.borrow_mut().take() {
                    let _ = t.send(());
                }
            }),
            ms,
        );
        struct Guard(Cancel);
        impl Drop for Guard {
            fn drop(&mut self) {
                (self.0)()
            }
        }
        let _guard = Guard(cancel);
        let _ = rx.await;
    }

    async fn took_the_resume(&self, entry: &Entry) -> bool {
        self.reenter();
        self.base.call("took_the_resume", json!({ "id": Self::id_of(entry) }));
        self.flag("took_the_resume", false)
    }

    async fn ws_wake(&self, entry: &Entry, landed: bool) -> Result<WakeResult, EngineError> {
        self.reenter();
        let (resize_to, size, shape) = {
            let e = entry.borrow();
            let r = e.record.borrow();
            (
                e.resize_to.map(|s| json!({ "cpu": serde_json::to_value(s.cpu).unwrap(), "memMb": serde_json::to_value(s.mem_mb).unwrap() })),
                serde_json::to_value(&r.size).unwrap(),
                r.shape.as_ref().map(|s| serde_json::to_value(s).unwrap()),
            )
        };
        self.base.call("ws_wake", json!({ "id": Self::id_of(entry), "landed": landed, "resizeTo": resize_to, "size": size, "shape": shape }));
        match self.wakes.next().unwrap_or(WakePlan::Done(done(None))) {
            WakePlan::Done(r) => r,
            WakePlan::Hold => {
                let rx = self.wake_hold.borrow_mut().pop_front().expect("a held wake has its sender");
                let stop = entry.borrow().wake_stop.clone();
                let stopped = async {
                    match stop {
                        Some(t) => t.cancelled().await,
                        None => futures::future::pending::<()>().await,
                    }
                };
                tokio::select! {
                    answered = rx => answered.unwrap_or_else(|_| Err(other("the held wake was dropped"))),
                    _ = stopped => Err(other("resume of the machine was aborted")),
                }
            }
        }
    }

    async fn ws_rebuild(&self, entry: &Entry) -> Result<(), EngineError> {
        self.reenter();
        self.base.call("ws_rebuild", json!({ "id": Self::id_of(entry) }));
        let r = self.rebuilds.next().unwrap_or(Ok(()));
        if r.is_ok() {
            *self.engine_machine.borrow_mut() = Some(self.replacement.borrow().clone());
        }
        r
    }

    async fn ws_upgrade(&self, entry: &Entry) -> Result<(), EngineError> {
        self.reenter();
        self.base.call("ws_upgrade", json!({ "id": Self::id_of(entry) }));
        let r = self.upgrades.next().unwrap_or(Ok(()));
        if r.is_ok() {
            *self.engine_machine.borrow_mut() = Some(self.replacement.borrow().clone());
        }
        r
    }

    fn ws_current_phase(&self, entry: &Entry) -> WorkspacePhase {
        self.base.call("ws_current_phase", json!({ "id": Self::id_of(entry) }));
        self.engine_phase.get()
    }

    fn ws_note_paused(&self, entry: &Entry) {
        self.base.call("ws_note_paused", json!({ "id": Self::id_of(entry) }));
        self.engine_phase.set(WorkspacePhase::Napping);
    }

    fn arm_late_read(&self, entry: &Entry) {
        self.base.call("arm_late_read", json!({ "id": Self::id_of(entry) }));
    }

    fn can_resume_at(&self, entry: &Entry) -> bool {
        self.base.call("can_resume_at", json!({ "id": Self::id_of(entry) }));
        self.flag("can_resume_at", true)
    }

    fn provider_of(&self, entry: &Entry) -> Option<String> {
        self.base.call("provider_of", json!({ "id": Self::id_of(entry) }));
        match self.base.answer("provider_of") {
            Some(Value::Null) => None,
            Some(v) => v.as_str().map(str::to_owned),
            None => Some("box".to_owned()),
        }
    }

    async fn backend_capabilities(&self, entry: &Entry) -> Capabilities {
        self.base.call("backend_capabilities", json!({ "id": Self::id_of(entry) }));
        self.caps.borrow().clone()
    }

    fn thread_runs(&self, thread_id: &str) -> bool {
        self.base.call("thread_runs", json!({ "thread": thread_id }));
        self.running_threads.borrow().iter().any(|t| t == thread_id)
    }

    async fn shape_of(&self, entry: &Entry) -> Option<MachineShape> {
        self.reenter();
        self.base.call("shape_of", json!({ "id": Self::id_of(entry) }));
        self.shapes.next().flatten()
    }

    fn size_built(&self, shape: Option<&MachineShape>, asked: SizeAsk) -> SizeAsk {
        SizeAsk {
            cpu: shape.and_then(|s| s.cpu).unwrap_or(asked.cpu),
            mem_mb: shape.and_then(|s| s.mem_mb).unwrap_or(asked.mem_mb),
        }
    }

    async fn status_now(&self, entry: &Entry) -> Result<(), HostError> {
        self.reenter();
        self.base.call("status_now", json!({ "id": Self::id_of(entry) }));
        Ok(())
    }

    fn refuse_cannot(&self, entry: &Entry, cap: &str, act: &str) -> Result<(), HostError> {
        self.base.call("refuse_cannot", json!({ "id": Self::id_of(entry), "cap": cap, "act": act }));
        self.refusal("refuse_cannot")
    }

    fn keeps_images(&self, entry: &Entry) -> bool {
        self.base.call("keeps_images", json!({ "id": Self::id_of(entry) }));
        self.flag("keeps_images", false)
    }

    fn pause_keeps_disk(&self, entry: &Entry) -> bool {
        self.base.call("pause_keeps_disk", json!({ "id": Self::id_of(entry) }));
        self.flag("pause_keeps_disk", false)
    }

    fn name_given(&self, name: &str) -> String {
        self.base.call("name_given", json!({ "name": name }));
        name.trim().to_owned()
    }

    fn name_refusal(&self, name: &str) -> Option<String> {
        self.base.call("name_refusal", json!({ "name": name }));
        self.base.answer("name_refusal").and_then(|v| v.as_str().map(str::to_owned))
    }

    fn put_look(&self, entry: &Entry, key: &str, value: Option<Value>) {
        self.base.call("put_look", json!({ "id": Self::id_of(entry), "key": key, "value": value }));
        Self::with_record(entry, |r| match (key, value) {
            (_, None) => {}
            ("theme", Some(Value::Null)) => r.theme = None,
            ("glyph", Some(Value::Null)) => r.glyph = None,
            ("theme", Some(v)) => r.theme = Some(serde_json::from_value(v).expect("a theme reads")),
            ("glyph", Some(v)) => r.glyph = Some(serde_json::from_value(v).expect("a glyph reads")),
            (other, _) => panic!("a look has no {other}"),
        });
    }

    fn served_by_its_computer(&self, entry: &Entry) -> Option<String> {
        self.base.call("served_by_its_computer", json!({ "id": Self::id_of(entry) }));
        self.base.answer("served_by_its_computer").and_then(|v| v.as_str().map(str::to_owned))
    }

    fn computer_of(&self, entry: &Entry) -> String {
        self.base.call("computer_of", json!({ "id": Self::id_of(entry) }));
        self.base.answer("computer_of").and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_else(|| "Mac".to_owned())
    }

    fn has_deploy_daemon(&self, entry: &Entry) -> bool {
        self.base.call("has_deploy_daemon", json!({ "id": Self::id_of(entry) }));
        self.flag("has_deploy_daemon", true)
    }

    async fn deploy_daemon_on(&self, entry: &Entry) -> Result<(), HostError> {
        self.reenter();
        self.base.call("deploy_daemon_on", json!({ "id": Self::id_of(entry) }));
        self.refusal("deploy_daemon_on")
    }

    async fn delete_queued(&self, id: &str) -> Option<Result<(), HostError>> {
        self.base.call("delete_queued", json!({ "id": id }));
        match self.base.answer_as::<Value>("delete_queued", || Value::Null) {
            Err(e) => Some(Err(e)),
            Ok(Value::Null) => None,
            Ok(_) => Some(Ok(())),
        }
    }

    async fn refuse_changed(&self, path: &str) -> Result<(), HostError> {
        self.reenter();
        self.base.call("refuse_changed", json!({ "path": path }));
        self.refusal("refuse_changed")
    }

    fn path_exists(&self, path: &str) -> bool {
        self.base.call("path_exists", json!({ "path": path }));
        self.paths.borrow().iter().any(|p| p == path)
    }

    async fn cut_off(&self, entry: &Entry) -> Result<(), HostError> {
        self.reenter();
        self.base.call("cut_off", json!({ "id": Self::id_of(entry) }));
        let gate = self.cut_gate.borrow_mut().take();
        if let Some(rx) = gate {
            let _ = rx.await;
        }
        self.refusal("cut_off")
    }

    fn end_sessions(&self, id: &str, reason: &str) {
        self.base.call("end_sessions", json!({ "id": id, "reason": reason }));
    }

    async fn drop_checkpoints(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError> {
        self.reenter();
        self.base.call("drop_checkpoints", json!({ "id": Self::id_of(entry), "thread": thread_id }));
        self.refusal("drop_checkpoints")
    }

    async fn remove_worktree_ending(&self, entry: &Entry) -> Result<(), HostError> {
        self.reenter();
        self.base.call("remove_worktree_ending", json!({ "id": Self::id_of(entry) }));
        self.refusal("remove_worktree_ending")
    }

    async fn drop_thread_files(&self, entry: &Entry, thread_ids: Vec<String>) -> Result<(), HostError> {
        self.reenter();
        self.base.call("drop_thread_files", json!({ "id": Self::id_of(entry), "threads": thread_ids }));
        self.refusal("drop_thread_files")
    }

    fn driven(&self, entry: &Entry) -> bool {
        self.base.call("driven", json!({ "id": Self::id_of(entry) }));
        entry.borrow().record.borrow().kind == WorkspaceKind::Cloud
    }

    async fn machine_kill(&self, entry: &Entry) -> Result<(), EngineError> {
        self.reenter();
        self.base.call("machine_kill", json!({ "id": Self::id_of(entry) }));
        self.kills.next().unwrap_or(Ok(()))
    }

    async fn drop_workspace(&self, id: &str) -> Result<(), HostError> {
        self.reenter();
        self.base.call("drop_workspace", json!({ "id": id }));
        self.refusal("drop_workspace")?;
        self.base.live.borrow_mut().shift_remove(id);
        Ok(())
    }

    async fn read_gone(&self, entry: &Entry) -> Result<String, HostError> {
        self.reenter();
        self.base.call("read_gone", json!({ "id": Self::id_of(entry), "machine": entry.borrow().record.borrow().machine_id.clone() }));
        self.base.answer_as::<String>("read_gone", || "gone".to_owned())
    }

    fn idle_touch(&self, id: &str) {
        self.base.call("idle_touch", json!({ "id": id }));
    }

    fn has_restart_daemon(&self, entry: &Entry) -> bool {
        self.base.call("has_restart_daemon", json!({ "id": Self::id_of(entry) }));
        self.flag("has_restart_daemon", true)
    }

    async fn restart_daemon_of(&self, entry: &Entry) -> Result<(), HostError> {
        self.reenter();
        self.base.call("restart_daemon_of", json!({ "id": Self::id_of(entry) }));
        self.refusal("restart_daemon_of")
    }

    fn shared_daemon(&self, entry: &Entry) -> bool {
        self.base.call("shared_daemon", json!({ "id": Self::id_of(entry) }));
        self.flag("shared_daemon", false)
    }

    fn shared_daemon_replaced(&self) {
        self.base.call("shared_daemon_replaced", json!(null));
    }

    fn forget_polled_reach(&self, id: &str) {
        self.base.call("forget_polled_reach", json!({ "id": id }));
    }

    async fn push_status(&self, entry: &Entry) -> Result<(), HostError> {
        self.reenter();
        self.base.call("push_status", json!({ "id": Self::id_of(entry) }));
        Ok(())
    }

    fn forget_failed_create(&self, id: &str) -> bool {
        self.base.call("forget_failed_create", json!({ "id": id }));
        self.flag("forget_failed_create", false)
    }
}

/// `isoSeconds(at)` (thread.ts): UTC to the second.
pub fn format_seconds(at: i64) -> String {
    let secs = at.div_euclid(1000);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

#[allow(dead_code)]
fn unused() {
    let _ = (kinded("", Kind::Usage), merge as fn(&mut Value, Value));
}
