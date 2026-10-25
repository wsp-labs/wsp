// SPDX-License-Identifier: AGPL-3.0-only
//! `FakeCreate`: the create and lookup half's context over a `FakeBase`. Every method logs `self.base.call(<its name>,
//! <args>)` and answers from the script under that name (an `$error` answer is the refusal), with a default that lets
//! the happy path through. Entries the fake makes (`create_staged`, `project_folder`) are held in `base.live`, and every
//! call out checks that no borrow of a live entry is held (rule 3.3).
#![allow(dead_code)]
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::channel::oneshot;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_account::projects::workspaces::create::{CreateCtx, LeadStart};
use wsp_account::projects::workspaces::seam::{
    Base, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld,
};
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::pending::Store;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::wiring::{CreateWorkspaceOptions, CreatedWorkspace, StageReport, WorkspaceRecord};
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView, WorkspaceAgents};
use wsp_wire::views::workspace_events::WorkspaceCreatingEvent;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceView};
use wsp_wire::wire::capabilities::Capabilities;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::{HostError, Kind};

pub use crate::base::{person as the_person, thread as asker};
use crate::base::{kinded, record, refused, view_of, BaseOf, FakeBase};

/// A thread's scope: `{kind: thread, threadId, workspaceId, rootThreadId}`.
pub fn scope(thread_id: &str, workspace_id: &str, root: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread_id, "workspaceId": workspace_id, "rootThreadId": root }))
        .expect("a scope reads")
}

/// `refusalLine(happened, fix)` (thread.ts:393), the message a `refusal()` carries.
pub fn refusal_line(happened: &str, fix: &str) -> String {
    let said = happened.trim_end();
    let stop = said.ends_with(['.', '!', '?', ':']);
    format!("{said}{} {fix}", if stop { "" } else { "." })
}

/// A refusal as `refusal(happened, fix, kind)` builds it.
pub fn refusal(happened: &str, fix: &str, kind: Kind) -> HostError {
    refused(&refusal_line(happened, fix), fix, kind)
}

/// Capabilities told apart by their one size: `tag` cpus.
pub fn caps(tag: u32) -> Capabilities {
    serde_json::from_value(json!({
        "liveCloneForks": false, "replacesMachine": true, "previewUrls": false, "signedUrls": false, "callbackRelay": false,
        "diskSnapshots": false, "images": true, "snapshotsAnyLife": false, "snapshotListing": false, "templates": false,
        "kept": false, "copies": false, "ownNetwork": false,
        "sizes": [{ "cpu": tag, "memMb": 1024, "rateUsdPerHour": 0.5 }],
    }))
    .expect("capabilities read")
}

fn text(v: Option<Value>) -> Option<String> {
    v.and_then(|v| v.as_str().map(str::to_owned))
}

fn missing_machine() -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::Missing,
        status: 404,
        code: None,
        message: "machine not found".to_owned(),
        request_id: None,
        window: None,
        said: None,
    })
}

pub struct FakeCreate {
    pub base: Rc<FakeBase>,
    pub shown: Rc<RefCell<IndexMap<String, WorkspaceView>>>,
    pub failed: Rc<RefCell<IndexMap<String, WorkspaceView>>>,
    /// Names a create holds right now: `name_refusal` reads it as the TS reads `ctx.forking`.
    pub forking: RefCell<Vec<String>>,
    pub stages: RefCell<Vec<WorkspaceCreatingEvent>>,
    /// Held until the test sends: what `boot_work` waits on.
    pub boot_gate: RefCell<Option<oneshot::Receiver<()>>>,
    /// The ids `listed_for` answers; none means every live entry.
    pub listed: RefCell<Option<Vec<String>>>,
    /// Ids of views the person's list does not drive.
    pub undriven: RefCell<Vec<String>>,
    /// Ids a thread's word reaches through its tree (`talks_to_tree_on`).
    pub tree_ids: RefCell<Vec<String>>,
    /// Ids `refusal_for` refuses.
    pub hidden: RefCell<Vec<String>>,
    pub scope_project: RefCell<Option<String>>,
    /// Project ids `of_threads_repository` answers false for.
    pub other_repo: RefCell<Vec<String>>,
    /// Project ids `project_reached` answers true for.
    pub reached: RefCell<Vec<String>>,
    /// What `places_wired` answers.
    pub wired: RefCell<String>,
    /// A failing `create_staged` leaves its machine's entry held, as a fork that landed before it failed does.
    pub stage_holds: Cell<bool>,
    /// A held entry's record keys the staged entry takes (a branch of record, for one).
    pub staged_over: RefCell<Value>,
    pub yield_in_guard: Cell<bool>,
}

impl FakeCreate {
    pub fn new() -> Rc<FakeCreate> {
        Rc::new(FakeCreate {
            base: FakeBase::new(),
            shown: Rc::new(RefCell::new(IndexMap::new())),
            failed: Rc::new(RefCell::new(IndexMap::new())),
            forking: RefCell::new(vec![]),
            stages: RefCell::new(vec![]),
            boot_gate: RefCell::new(None),
            listed: RefCell::new(None),
            undriven: RefCell::new(vec![]),
            tree_ids: RefCell::new(vec![]),
            hidden: RefCell::new(vec![]),
            scope_project: RefCell::new(None),
            other_repo: RefCell::new(vec![]),
            reached: RefCell::new(vec![]),
            wired: RefCell::new("boat".to_owned()),
            stage_holds: Cell::new(false),
            staged_over: RefCell::new(json!({})),
            yield_in_guard: Cell::new(false),
        })
    }

    pub fn ctx(self: &Rc<Self>) -> Rc<dyn CreateCtx> {
        self.clone()
    }

    /// Fails if a borrow of a live entry (or its record) is held: every call out of the unit runs this.
    fn reenter(&self) {
        let held: Vec<Entry> = self.base.live.borrow().values().cloned().collect();
        for e in held {
            assert!(e.try_borrow_mut().is_ok(), "a borrow of a live entry is held across a call out");
            let record = e.borrow().record.clone();
            assert!(record.try_borrow_mut().is_ok(), "a borrow of an entry's record is held across a call out");
        }
    }

    fn scoped(origin: &Option<Caller>) -> bool {
        matches!(origin, Some(Caller::Scoped { .. }))
    }

    fn of_json(&self, v: Value) -> WorkspaceView {
        serde_json::from_value(v).expect("a test view reads")
    }
}

fn project_ref(p: &ProjectView) -> Value {
    json!({ "id": p.id, "name": p.name, "path": p.path, "computer": p.computer })
}

#[async_trait(?Send)]
impl Base for FakeCreate {
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
    fn thread_records(&self) -> Vec<(String, ThreadRecord)> {
        BaseOf(self.base.clone()).thread_records()
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
impl CreateCtx for FakeCreate {
    fn places_wired(&self) -> String {
        self.wired.borrow().clone()
    }

    async fn resolve_project(&self, word: &str, origin: Option<Caller>) -> Result<ProjectView, HostError> {
        self.base.call("resolve_project", json!({ "word": word, "scoped": Self::scoped(&origin) }));
        self.base.answer_as::<Value>("resolve_project", || Value::Null)?;
        let held = self.base.projects.borrow().values().find(|p| p.id == word || p.name == word).cloned();
        held.ok_or_else(|| kinded(&format!("no project {word}"), Kind::NotFound))
    }

    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        self.base.call("landing_place", json!({ "computer": computer }));
        self.base.answer_as::<Option<String>>("landing_place", || None)
    }

    async fn landing_capabilities(&self, place_id: Option<&str>) -> Result<Capabilities, HostError> {
        self.base.call("landing_capabilities", json!({ "place": place_id }));
        self.base.answer_as::<Capabilities>("landing_capabilities", || caps(100))
    }

    fn backend_capabilities(&self, kind: WorkspaceKind, place: Option<&str>) -> Capabilities {
        self.base.call("backend_capabilities", json!({ "kind": serde_json::to_value(kind).unwrap(), "place": place }));
        let tag = match kind {
            WorkspaceKind::Local => 1,
            WorkspaceKind::Place => 2,
            _ => 3,
        };
        self.base.answer_as::<Capabilities>("backend_capabilities", || caps(tag)).expect("capabilities answer")
    }

    fn act_refusal(&self, scope: &ThreadScope, act: &str) -> HostError {
        self.base.call("act_refusal", json!({ "threadId": scope.thread_id, "act": act }));
        refused(&format!("act refusal: {act} for {}", scope.thread_id), "ask the person", Kind::Usage)
    }

    async fn spawn_sub(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError> {
        self.base.call("spawn_sub", json!({ "name": opts.name, "project": opts.project, "on": opts.sub.as_ref().map(|s| s.on.clone()), "scoped": Self::scoped(&origin) }));
        self.base.answer_as::<Value>("spawn_sub", || Value::Null)?;
        let rec = record("ws_sub", "cloud", &opts.project, json!({ "name": opts.name }));
        Ok(CreatedWorkspace { view: view_of(&self.base, &rec), notice: Some("sub started".to_owned()), sub: None })
    }

    fn place_of_thread(&self, scope: &ThreadScope) -> Option<String> {
        self.base.call("place_of_thread", json!({ "threadId": scope.thread_id }));
        text(self.base.answer("place_of_thread"))
    }

    fn name_given(&self, name: &str) -> String {
        self.base.call("name_given", json!({ "name": name }));
        name.trim().to_owned()
    }

    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.base.call("refuse_recording", json!({ "word": word, "scoped": origin.is_some_and(|c| matches!(c, Caller::Scoped { .. })) }));
        self.base.answer_as::<Value>("refuse_recording", || Value::Null).map(|_| ())
    }

    fn refuse_relayed(&self, kind: WorkspaceKind, name: &str, project: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.base.call(
            "refuse_relayed",
            json!({ "kind": serde_json::to_value(kind).unwrap(), "name": name, "project": project, "scoped": origin.is_some_and(|c| matches!(c, Caller::Scoped { .. })) }),
        );
        self.base.answer_as::<Value>("refuse_relayed", || Value::Null).map(|_| ())
    }

    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError> {
        self.base.call("project_folder", json!({ "project": project.id }));
        self.base.answer_as::<Value>("project_folder", || Value::Null)?;
        let id = format!("ws_folder_{}", project.id);
        let held = self.base.live.borrow().get(&id).cloned();
        match held {
            Some(e) => Ok(e),
            None => Ok(self.base.hold(record(&id, "local", &project.id, json!({ "name": project.name, "machineId": "" })))),
        }
    }

    fn agents_from(&self, place: &str, agents: &Map<String, Value>) -> WorkspaceAgents {
        self.base.call("agents_from", json!({ "place": place, "agents": Value::Object(agents.clone()) }));
        serde_json::from_value(json!({
            "spawn": agents.get("spawn").and_then(Value::as_bool).unwrap_or(false),
            "maxMachines": agents.get("maxMachines").cloned().unwrap_or(json!(5)),
            "maxDepth": agents.get("maxDepth").cloned().unwrap_or(json!(4)),
        }))
        .expect("agents read")
    }

    fn same_repository(&self, a: &ProjectView, b: &ProjectView, origin: Option<&Caller>) -> bool {
        self.base.call("same_repository", json!({ "a": a.id, "b": b.id, "scoped": origin.is_some_and(|c| matches!(c, Caller::Scoped { .. })) }));
        self.base.answer("same_repository").and_then(|v| v.as_bool()).unwrap_or(false)
    }

    async fn place_guard(&self, place_id: &str) -> Result<(), HostError> {
        self.base.call("place_guard", json!({ "place": place_id }));
        if self.yield_in_guard.get() {
            tokio::task::yield_now().await;
        }
        self.base.answer_as::<Value>("place_guard", || Value::Null).map(|_| ())
    }

    fn name_refusal(&self, name: &str) -> Option<String> {
        self.base.call("name_refusal", json!({ "name": name }));
        if self.forking.borrow().iter().any(|n| n == name) {
            return Some(format!("{name} is already a workspace; pick another name, or delete it first"));
        }
        text(self.base.answer("name_refusal"))
    }

    fn forking_add(&self, name: &str) {
        self.base.call("forking_add", json!({ "name": name }));
        self.forking.borrow_mut().push(name.to_owned());
    }

    fn forking_delete(&self, name: &str) {
        self.base.call("forking_delete", json!({ "name": name }));
        let mut f = self.forking.borrow_mut();
        if let Some(i) = f.iter().position(|n| n == name) {
            f.remove(i);
        }
    }

    fn supersede_failed(&self, name: &str) {
        self.base.call("supersede_failed", json!({ "name": name }));
    }

    fn stage_reporter(&self, id: &str, name: &str, began: i64, scope: Option<&ThreadScope>) -> wsp_core::types::wiring::StageReport {
        self.base.call("stage_reporter", json!({ "id": id, "name": name, "began": began, "thread": scope.map(|s| s.thread_id.clone()) }));
        let base = self.base.clone();
        let id = id.to_owned();
        let report: StageReport = Rc::new(move |stage, line, said| {
            base.call(
                "report",
                json!({ "id": id, "stage": serde_json::to_value(stage).unwrap(), "line": line, "notice": said.and_then(|s| s.notice) }),
            );
        });
        report
    }

    async fn lead_start(&self, parent: &Entry, name: &str, how: &str) -> Result<LeadStart, HostError> {
        self.reenter();
        let parent_id = parent.borrow().record.borrow().id.clone();
        self.base.call("lead_start", json!({ "parent": parent_id, "name": name, "how": how }));
        let v = self.base.answer_as::<Value>("lead_start", || json!({ "base": "feedface", "onLeads": false, "lines": [] }))?;
        Ok(LeadStart {
            base: v.get("base").and_then(Value::as_str).map(str::to_owned),
            on_leads: v.get("onLeads").and_then(Value::as_bool).unwrap_or(false),
            lines: v.get("lines").and_then(Value::as_array).map(|a| a.iter().filter_map(|l| l.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
        })
    }

    fn show_create(&self, view: WorkspaceView) {
        self.base.call("show_create", json!({ "view": serde_json::to_value(&view).unwrap() }));
        self.shown.borrow_mut().insert(view.id.clone(), view);
    }

    fn shown_view(
        &self,
        id: &str,
        name: &str,
        kind: WorkspaceKind,
        golden: &str,
        created_at: &str,
        project: &ProjectView,
        scope: &ThreadScope,
        parent: Option<&str>,
    ) -> WorkspaceView {
        let mut v = json!({
            "id": id, "name": name, "machineId": "", "phase": "queued", "kind": serde_json::to_value(kind).unwrap(), "golden": golden,
            "createdAt": created_at, "project": project_ref(project), "parentThreadId": scope.thread_id, "rootThreadId": scope.root_thread_id,
        });
        if let Some(p) = parent {
            v["parentWorkspaceId"] = json!(p);
        }
        self.of_json(v)
    }

    async fn create_staged(
        &self,
        opts: CreateWorkspaceOptions,
        project: &ProjectView,
        id: &str,
        report: StageReport,
        scope: Option<ThreadScope>,
        free: FreeSlot,
        base: Option<String>,
    ) -> Result<CreatedWorkspace, HostError> {
        let _ = (&report, &free);
        if self.yield_in_guard.get() {
            tokio::task::yield_now().await;
        }
        self.reenter();
        self.base.call(
            "create_staged",
            json!({
                "name": opts.name, "parent": opts.parent, "golden": opts.golden, "project": project.id, "id": id,
                "agents": opts.agents.as_ref().map(|a| Value::Object(a.clone())), "thread": scope.as_ref().map(|s| s.thread_id.clone()), "base": base,
            }),
        );
        let mut over = json!({ "name": opts.name, "machineId": format!("m_{id}") });
        crate::base::merge(&mut over, self.staged_over.borrow().clone());
        match self.base.answer_as::<Value>("create_staged", || Value::Null) {
            Err(e) => {
                if self.stage_holds.get() {
                    let entry = self.base.hold(record(id, "cloud", &project.id, over));
                    entry.borrow_mut().creating = true;
                }
                Err(e)
            }
            Ok(answer) => {
                let entry = self.base.hold(record(id, "cloud", &project.id, over));
                let rec = entry.borrow().record.borrow().clone();
                Ok(CreatedWorkspace { view: view_of(&self.base, &rec), notice: answer.get("notice").and_then(Value::as_str).map(str::to_owned), sub: None })
            }
        }
    }

    fn child_started_line(&self, name: &str, branch: &str) -> String {
        format!("{name} started on {branch}")
    }

    async fn start_child_on(&self, child: &Entry, base: &str, parent_name: &str) -> Result<String, HostError> {
        self.reenter();
        let child_id = child.borrow().record.borrow().id.clone();
        self.base.call("start_child_on", json!({ "child": child_id, "base": base, "parent": parent_name }));
        Ok(text(self.base.answer("start_child_on")).unwrap_or_else(|| format!("{parent_name} started {child_id} on {base}")))
    }

    async fn unfork(&self, entry: &Entry) -> Result<(), EngineError> {
        self.reenter();
        let id = entry.borrow().record.borrow().id.clone();
        self.base.call("unfork", json!({ "id": id }));
        match text(self.base.answer("unfork")).as_deref() {
            None | Some("ok") => Ok(()),
            Some("missing") => Err(missing_machine()),
            Some(other) => Err(EngineError::Other { message: other.to_owned(), code: None }),
        }
    }

    fn shown_creates(&self) -> Rc<RefCell<IndexMap<String, WorkspaceView>>> {
        self.shown.clone()
    }

    fn failed_creates(&self) -> Rc<RefCell<IndexMap<String, WorkspaceView>>> {
        self.failed.clone()
    }

    fn failed_view(&self, id: &str, name: &str, kind: WorkspaceKind, golden: &str, began: i64, project: &ProjectView, said: &str) -> WorkspaceView {
        self.base.call("failed_view", json!({ "id": id, "name": name, "kind": serde_json::to_value(kind).unwrap(), "golden": golden, "began": began, "project": project.id, "said": said }));
        self.of_json(json!({
            "id": id, "name": name, "machineId": "", "phase": "gone", "kind": serde_json::to_value(kind).unwrap(), "golden": golden,
            "createdAt": format!("began-{began}"), "project": project_ref(project), "gone": said,
        }))
    }

    async fn boot_work(&self, id: &str) {
        self.base.call("boot_work", json!({ "id": id }));
        let gate = self.boot_gate.borrow_mut().take();
        if let Some(rx) = gate {
            let _ = rx.await;
        }
    }

    fn listed_for(&self, origin: Option<&Caller>) -> Vec<Entry> {
        self.base.call("listed_for", json!({ "scoped": origin.is_some_and(|c| matches!(c, Caller::Scoped { .. })) }));
        let all: Vec<Entry> = self.base.live.borrow().values().cloned().collect();
        match &*self.listed.borrow() {
            None => all,
            Some(ids) => all.into_iter().filter(|e| ids.contains(&e.borrow().record.borrow().id)).collect(),
        }
    }

    fn held(&self) -> Vec<Entry> {
        self.base.call("held", json!(null));
        self.base.live.borrow().values().cloned().collect()
    }

    fn drives(&self, view: &WorkspaceView, _origin: Option<&Caller>) -> bool {
        self.base.call("drives", json!({ "id": view.id }));
        !self.undriven.borrow().contains(&view.id)
    }

    fn talks_to_tree_on(&self, id: &str, _origin: Option<&Caller>) -> bool {
        self.base.call("talks_to_tree_on", json!({ "id": id }));
        self.tree_ids.borrow().iter().any(|t| t == id)
    }

    fn project_of_scope(&self, _origin: Option<&Caller>) -> Option<String> {
        self.scope_project.borrow().clone()
    }

    fn refusal_for(&self, record: Option<&WorkspaceRecord>, _origin: Option<&Caller>) -> Option<String> {
        let record = record?;
        self.base.call("refusal_for", json!({ "id": record.id }));
        self.hidden.borrow().contains(&record.id).then(|| "outside the tree".to_owned())
    }

    fn of_threads_repository(&self, _origin: Option<&Caller>, project_id: &str) -> bool {
        self.base.call("of_threads_repository", json!({ "project": project_id }));
        !self.other_repo.borrow().iter().any(|p| p == project_id)
    }

    fn elsewhere_refusal(&self, _origin: Option<&Caller>, project: &ProjectView, word: &str) -> Option<HostError> {
        self.base.call("elsewhere_refusal", json!({ "project": project.id, "word": word }));
        self.base.answer_as::<Value>("elsewhere_refusal", || Value::Null).err()
    }

    fn away_for(&self, record: &WorkspaceRecord, _origin: Option<&Caller>, act: &str, word: &str) -> Option<HostError> {
        self.base.call("away_for", json!({ "id": record.id, "act": act, "word": word }));
        self.base.answer_as::<Value>("away_for", || Value::Null).err()
    }

    fn project_reached(&self, _origin: Option<&Caller>, project_id: &str) -> bool {
        self.base.call("project_reached", json!({ "project": project_id }));
        self.reached.borrow().iter().any(|p| p == project_id)
    }

    async fn find_places(&self, word: &str) -> Result<Vec<String>, HostError> {
        self.base.call("find_places", json!({ "word": word }));
        self.base.answer_as::<Vec<String>>("find_places", Vec::new)
    }

    fn create_stages(&self) -> Vec<WorkspaceCreatingEvent> {
        self.stages.borrow().clone()
    }
}
