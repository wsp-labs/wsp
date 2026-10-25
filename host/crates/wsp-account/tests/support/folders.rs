// SPDX-License-Identifier: AGPL-3.0-only
//! The folders area's context for tests: `FakeFolders` wraps a `FakeBase` (support/base.rs), implements `Base` by
//! delegating to it and `FoldersCtx` by logging each call on the base's list and answering from its script. State
//! a test reads back: the sweep state, the turns that run, the repos that are not repos, the branch each path holds
//! and the landings a test holds open.
#![allow(dead_code)]
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::task::Poll;

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};
use wsp_account::projects::folders::{FoldersCtx, HereDaemon, SweepState, WorktreeAsk, WorktreeRemoveAsk};
use wsp_account::projects::workspaces::seam::{
    Base, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, LiveEntry, ProjectsHeld,
};
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::machine::{ExecOptions, Machine, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_wire::types::pull_request::PullRequestRecord;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{WorkspaceView, WorktreeReport};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use crate::base::{plain, BaseOf, FakeBase};

/// A machine handle that only carries its id: the folders area never runs anything on it (every exec goes through
/// `FoldersCtx::local_exec`).
pub struct FakeMachine {
    pub id: String,
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> EngineResult<ExecResult> {
        Ok(ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() })
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> EngineResult<ExecResult> {
        Ok(ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() })
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> EngineResult<String> {
        Ok("s".into())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> EngineResult<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&wsp_engine::cancel::Cancel>) -> EngineResult<()> {
        Ok(())
    }
    async fn kill(&self) -> EngineResult<()> {
        Ok(())
    }
    async fn state(&self) -> EngineResult<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> EngineResult<String> {
        Ok("u".into())
    }
    async fn upload_url(&self, _path: &str) -> EngineResult<String> {
        Ok("u".into())
    }
}

/// Returns pending once, so a second caller started in the same `join!` gets its turn mid-flight (the TS awaits there).
pub async fn yield_once() {
    let mut done = false;
    futures::future::poll_fn(move |cx| {
        if done {
            Poll::Ready(())
        } else {
            done = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

/// A `Preferences` from the keys the schema requires, `over` written on top (e.g. `{"projectDefaults": {...}}`).
pub fn preferences(over: Value) -> Preferences {
    let mut v = json!({
        "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
        "projectLook": {}, "projectOrder": [], "labs": false, "projectDefaults": {},
    });
    crate::base::merge(&mut v, over);
    serde_json::from_value(v).expect("test preferences read")
}

type Joined = Shared<LocalBoxFuture<'static, Result<Entry, HostError>>>;

/// The folders context: `Base` from the shared fake, the folders calls logged and scripted by name.
pub struct FakeFolders {
    pub base: Rc<FakeBase>,
    /// `local?.copier !== undefined`.
    pub copier: Cell<bool>,
    /// `ctx.opts.statePath !== undefined`.
    pub state_path: Cell<bool>,
    /// `local.hereDaemon`.
    pub here: RefCell<Option<HereDaemon>>,
    pub sweep: Rc<RefCell<SweepState>>,
    /// Workspace ids a turn runs in.
    pub running: RefCell<IndexSet<String>>,
    /// Workspace ids `notARepo` answers true for.
    pub not_repo: RefCell<IndexSet<String>>,
    /// `branchAt(path)` by path; a path not here answers the `branch_at` script, else none.
    pub branches: RefCell<IndexMap<String, Option<String>>>,
    /// A landing a test holds open, by thread id: `checkpoints_landed` waits on it.
    pub landing: RefCell<IndexMap<String, oneshot::Receiver<()>>>,
    joined: RefCell<IndexMap<String, Joined>>,
}

impl FakeFolders {
    pub fn new(base: Rc<FakeBase>) -> Rc<FakeFolders> {
        *base.prefs.borrow_mut() = Some(preferences(json!({})));
        Rc::new(FakeFolders {
            base,
            copier: Cell::new(true),
            state_path: Cell::new(true),
            here: RefCell::new(None),
            sweep: Rc::new(RefCell::new(SweepState::default())),
            running: RefCell::new(IndexSet::new()),
            not_repo: RefCell::new(IndexSet::new()),
            branches: RefCell::new(IndexMap::new()),
            landing: RefCell::new(IndexMap::new()),
            joined: RefCell::new(IndexMap::new()),
        })
    }

    fn b(&self) -> BaseOf {
        BaseOf(self.base.clone())
    }

    /// Holds a landing open for a thread; send on the returned sender to let it land.
    pub fn hold_landing(&self, thread_id: &str) -> oneshot::Sender<()> {
        let (tx, rx) = oneshot::channel();
        self.landing.borrow_mut().insert(thread_id.to_owned(), rx);
        tx
    }
}

#[async_trait(?Send)]
impl Base for FakeFolders {
    async fn ready(&self) -> Result<(), HostError> {
        self.b().ready().await
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.b().clock()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.b().ids()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.b().store()
    }
    fn live(&self) -> Live {
        self.b().live()
    }
    fn projects_held(&self) -> ProjectsHeld {
        self.b().projects_held()
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        self.b().project_held(project_id)
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.b().session_views()
    }
    fn thread_workspaces(&self) -> Vec<(String, String)> {
        self.b().thread_workspaces()
    }
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError> {
        self.b().persist(record).await
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        self.b().view(record)
    }
    fn emit(&self, event: Event) -> Result<(), HostError> {
        self.b().emit(event)
    }
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        self.b().entry_of(id, origin, ask).await
    }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError> {
        self.b().spawn_guard(act, origin)
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        self.b().kind_of(computer)
    }
    fn place_name(&self, place_id: &str) -> String {
        self.b().place_name(place_id)
    }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        self.b().copy_blocked(entry).await
    }
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<Value, HostError> {
        self.b().with_daemon(entry, work).await
    }
    async fn on_this_computer(&self, work: HereWork) -> Result<Value, HostError> {
        self.b().on_this_computer(work).await
    }
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError> {
        self.b().git_here(ask).await
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        self.b().checkout_of(record)
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        self.b().preferences().await
    }
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError> {
        self.b().write_daemon_roots(entry, strict).await
    }
    fn home_dir(&self) -> String {
        self.b().home_dir()
    }
    fn machine_id(&self, entry: &Entry) -> String {
        self.b().machine_id(entry)
    }
}

fn record_id(entry: &Entry) -> String {
    entry.borrow().record.borrow().id.clone()
}

#[async_trait(?Send)]
impl FoldersCtx for FakeFolders {
    fn state_folder(&self) -> String {
        format!("{}/.wsp", self.base.home)
    }
    fn has_state_path(&self) -> bool {
        self.state_path.get()
    }
    fn has_copier(&self) -> bool {
        self.copier.get()
    }
    async fn copier_worktree(&self, ask: WorktreeAsk) -> Result<WorktreeReport, HostError> {
        self.base.call(
            "copier_worktree",
            json!({
                "from": ask.from, "home": ask.home, "project": ask.project, "branch": ask.branch,
                "modules": serde_json::to_value(&ask.modules).unwrap(), "checkpoint": ask.checkpoint,
            }),
        );
        yield_once().await;
        let slug = ask.branch.replace('/', "-");
        let path = format!("{}/worktrees/{}/{}", ask.home, ask.project, slug);
        self.base.answer_as("copier_worktree", || {
            serde_json::from_value(json!({
                "path": path, "branch": ask.branch, "made": true, "carried": [], "fresh": true, "modules": [], "ms": 1,
            }))
            .expect("the default report reads")
        })
    }
    async fn copier_worktree_mount(&self, from: &str, home: &str, path: &str) -> Result<(), HostError> {
        self.base.call("copier_worktree_mount", json!({ "from": from, "home": home, "path": path }));
        self.base.answer_as("copier_worktree_mount", || ())
    }
    async fn copier_worktree_remove(&self, ask: WorktreeRemoveAsk) -> Result<(), HostError> {
        self.base.call("copier_worktree_remove", json!({ "from": ask.from, "home": ask.home, "path": ask.path, "force": ask.force }));
        self.base.answer_as("copier_worktree_remove", || ())
    }
    async fn here_daemon(&self) -> Result<Option<HereDaemon>, HostError> {
        self.base.call("here_daemon", json!(null));
        Ok(self.here.borrow().clone())
    }
    async fn admit_folder(&self, kind: WorkspaceKind, name: &str, place: Option<&str>) -> Result<(), HostError> {
        self.base.call("admit_folder", json!({ "kind": serde_json::to_value(kind).unwrap(), "name": name, "place": place }));
        yield_once().await;
        self.base.answer_as("admit_folder", || ())
    }
    async fn folder_machine(&self, kind: WorkspaceKind, place: Option<&str>) -> Result<(Arc<dyn Machine>, WorkspaceSize), HostError> {
        self.base.call("folder_machine", json!({ "kind": serde_json::to_value(kind).unwrap(), "place": place }));
        yield_once().await;
        let machine: Arc<dyn Machine> = Arc::new(FakeMachine { id: place.unwrap_or("local").to_owned() });
        Ok((machine, WorkspaceSize { cpu: 2.0.into(), mem_mb: 4096.0.into() }))
    }
    fn attach(&self, record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry {
        self.base.call("attach", json!({ "id": record.id, "machineId": machine.id() }));
        let id = record.id.clone();
        let entry = LiveEntry::new(record);
        self.base.live.borrow_mut().insert(id, entry.clone());
        entry
    }
    fn folders_of(&self, project_id: &str) -> Vec<Entry> {
        self.base
            .live
            .borrow()
            .values()
            .filter(|e| {
                let e = e.borrow();
                let r = e.record.borrow();
                r.project == project_id && r.kind != WorkspaceKind::Cloud
            })
            .cloned()
            .collect()
    }
    async fn one_folder(&self, key: &str, make: wsp_account::projects::workspaces::seam::Queued<Entry>) -> Result<Entry, HostError> {
        self.base.call("one_folder", json!({ "key": key }));
        let joined = {
            let mut held = self.joined.borrow_mut();
            match held.get(key) {
                Some(j) => j.clone(),
                None => {
                    let j: Joined = make.boxed_local().shared();
                    held.insert(key.to_owned(), j.clone());
                    j
                }
            }
        };
        let answer = joined.clone().await;
        let mut held = self.joined.borrow_mut();
        if held.get(key).is_some_and(|j| j.ptr_eq(&joined)) {
            held.shift_remove(key);
        }
        answer
    }
    async fn resolve_project(&self, word: &str, origin: Option<Caller>) -> Result<ProjectView, HostError> {
        self.base.call("resolve_project", json!({ "word": word, "scoped": matches!(origin, Some(Caller::Scoped { .. })) }));
        self.base.answer_as("resolve_project_error", || ())?;
        self.base
            .projects
            .borrow()
            .values()
            .find(|p| p.id == word || p.name == word)
            .cloned()
            .ok_or_else(|| plain(&format!("no project {word}")))
    }
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &ProjectView, word: &str) -> Option<HostError> {
        self.base.call("elsewhere_refusal", json!({ "project": project.id, "word": word, "scoped": matches!(origin, Some(Caller::Scoped { .. })) }));
        self.base.answer_as::<()>("elsewhere_refusal", || ()).err()
    }
    async fn picks_hold(&self, project: &ProjectView, picks: &StartPicksAsked) -> Result<(), HostError> {
        self.base.call("picks_hold", json!({ "project": project.id, "harness": picks.harness }));
        self.base.answer_as("picks_hold", || ())
    }
    async fn branch_at(&self, path: &str) -> Result<Option<String>, HostError> {
        self.base.call("branch_at", json!({ "path": path }));
        if let Some(b) = self.branches.borrow().get(path) {
            return Ok(b.clone());
        }
        self.base.answer_as("branch_at", || None)
    }
    fn turn_runs(&self, workspace_id: &str) -> bool {
        self.base.call("turn_runs", json!({ "id": workspace_id }));
        self.running.borrow().contains(workspace_id)
    }
    async fn drop_workspace(&self, workspace_id: &str) -> Result<(), HostError> {
        self.base.call("drop", json!({ "id": workspace_id }));
        self.base.answer_as("drop", || ())?;
        self.base.live.borrow_mut().shift_remove(workspace_id);
        Ok(())
    }
    async fn checkpoints_landed(&self, thread_id: &str) {
        self.base.call("checkpoints_landed", json!({ "thread": thread_id }));
        let held = self.landing.borrow_mut().shift_remove(thread_id);
        if let Some(rx) = held {
            let _ = rx.await;
        }
    }
    fn not_a_repo(&self, record: &WorkspaceRecord) -> bool {
        self.base.call("not_a_repo", json!({ "id": record.id }));
        self.not_repo.borrow().contains(&record.id)
    }
    async fn read_pull_request(&self, entry: &Entry, force: bool) -> Result<(), HostError> {
        self.base.call("read_pull_request", json!({ "id": record_id(entry), "force": force }));
        let seen = self.base.answer_as::<Option<PullRequestRecord>>("read_pull_request", || None)?;
        if let Some(pr) = seen {
            entry.borrow().record.borrow_mut().pr = Some(pr);
        }
        Ok(())
    }
    async fn status_now(&self, entry: &Entry) -> Result<(), HostError> {
        self.base.call("status_now", json!({ "id": record_id(entry) }));
        self.base.answer_as("status_now", || ())
    }
    async fn local_exec(&self, cmd: &str, timeout_ms: u64, stdin: Option<Vec<u8>>) -> Result<ExecResult, HostError> {
        self.base.call("local_exec", json!({ "cmd": cmd, "timeoutMs": timeout_ms, "stdin": stdin.as_ref().map(Vec::len) }));
        self.base.answer_as("local_exec", || ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() })
    }
    fn sweep_state(&self) -> Rc<RefCell<SweepState>> {
        self.sweep.clone()
    }
}
