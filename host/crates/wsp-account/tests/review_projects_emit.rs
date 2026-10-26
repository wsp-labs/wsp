// SPDX-License-Identifier: AGPL-3.0-only
//! Review of what the projects area does with a failure of the bus. `bus.emit` calls its listeners in line and does not
//! catch them (types/events.ts:81-87), so a listener that throws fails the `report(...)` that emitted: the add
//! (projects.ts:287-290), the import (:489-491) and the export (:525-527) all fail with it, and the spec says the same
//! (seam.rs `emit`: "a listener's failure reaches the caller"). The Rust reports through closures of type `Fn(stage,
//! &str)` that drop the `Result` (`let _ = emitter.emit(..)`: add.rs:653, moves.rs:605 and :720).
//!
//! `Lossy` is the projects fake with an `emit` that refuses one event type; every other method is the fake's own.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/projects.rs"]
mod projects;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use base::{person, record, run};
use indexmap::IndexMap;
use projects::FakeProjects;
use serde_json::{json, Value};
use wsp_account::projects::projects::add::{AddAsk, ComputerRow, JoinedComputer, LandingKind, LandingRoad, ProjectSourceModule};
use wsp_account::projects::projects::moves::{LandHooks, MoveRoads, Progress};
use wsp_account::projects::projects::{projects_area, ProjectsCtx};
use wsp_account::projects::workspaces::seam::{Base, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld};
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::internal::{ImportLanded, ImportReport, ThreadRecord};
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::{ProjectExportOptions, ProjectImportOptions, SeedWiring, WorkspaceRecord};
use wsp_engine::machine::MachineBackend;
use wsp_wire::types::thread_defaults::ThreadDefaults;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::project_bundle::ProjectAgentResult;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectSource, ProjectView, SeedChoice, SeedPlan};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

struct Lossy(Rc<FakeProjects>, RefCell<Option<String>>);

#[async_trait(?Send)]
impl Base for Lossy {
    async fn ready(&self) -> Result<(), HostError> { self.0.ready().await }
    fn clock(&self) -> Rc<dyn Clock> { self.0.clock() }
    fn ids(&self) -> Rc<dyn Ids> { self.0.ids() }
    fn store(&self) -> Rc<dyn Store> { self.0.store() }
    fn live(&self) -> Live { self.0.live() }
    fn projects_held(&self) -> ProjectsHeld { self.0.projects_held() }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> { self.0.project_held(project_id) }
    fn session_views(&self) -> Vec<SessionView> { self.0.session_views() }
    fn thread_records(&self) -> Vec<(String, ThreadRecord)> { self.0.thread_records() }
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError> { self.0.persist(record).await }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView { self.0.view(record) }
    fn emit(&self, event: Event) -> Result<(), HostError> { if let Some(t) = self.1.borrow().as_deref() { if event.get("type").and_then(Value::as_str) == Some(t) { return Err(HostError::new("a listener failed")); } } self.0.emit(event) }
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError> { self.0.entry_of(id, origin, ask).await }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError> { self.0.spawn_guard(act, origin) }
    fn kind_of(&self, computer: &str) -> WorkspaceKind { self.0.kind_of(computer) }
    fn place_name(&self, place_id: &str) -> String { self.0.place_name(place_id) }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> { self.0.copy_blocked(entry).await }
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<serde_json::Value, HostError> { self.0.with_daemon(entry, work).await }
    async fn on_this_computer(&self, work: HereWork) -> Result<serde_json::Value, HostError> { self.0.on_this_computer(work).await }
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError> { self.0.git_here(ask).await }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String { self.0.checkout_of(record) }
    async fn preferences(&self) -> Result<Preferences, HostError> { self.0.preferences().await }
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError> { self.0.write_daemon_roots(entry, strict).await }
    fn home_dir(&self) -> String { self.0.home_dir() }
    fn machine_id(&self, entry: &Entry) -> String { self.0.machine_id(entry) }
}
#[async_trait(?Send)]
impl MoveRoads for Lossy {
    async fn agents_on_machine(&self, entry: &Entry, agents: &[String]) -> Result<Vec<String>, HostError> { self.0.agents_on_machine(entry, agents).await }
    fn guest_agent_homes(&self) -> indexmap::IndexMap<String, String> { self.0.guest_agent_homes() }
    fn scratch(&self, entry: &Entry) -> String { self.0.scratch(entry) }
    async fn land_bundle(&self, entry: &Entry, tar: Vec<u8>, dest: &str, replace: Option<bool>, hooks: LandHooks) -> Result<u64, HostError> { self.0.land_bundle(entry, tar, dest, replace, hooks).await }
    async fn import_into(&self, entry: &Entry, tar: Vec<u8>, hooks: LandHooks) -> Result<(), HostError> { self.0.import_into(entry, tar, hooks).await }
    async fn merge_on_machine(&self, entry: &Entry, script: &str, agent: ProjectAgentResult) -> Result<ProjectAgentResult, HostError> { self.0.merge_on_machine(entry, script, agent).await }
    fn project_of(&self, dest: &str, bytes: u64) -> Option<serde_json::Value> { self.0.project_of(dest, bytes) }
    async fn provider_row(&self, entry: &Entry) -> Result<bool, HostError> { self.0.provider_row(entry).await }
    async fn export_folder( &self, entry: &Entry, source: &str, caches: &wsp_core::types::pending::CacheRule, archive: &str, timeout_ms: u64, on_progress: Rc<dyn Fn(Progress)>, ) -> Result<Vec<String>, HostError> { self.0.export_folder(entry, source, caches, archive, timeout_ms, on_progress).await }
    async fn export_paths_into( &self, entry: &Entry, groups: Vec<(String, Vec<String>)>, archive: &str, timeout_ms: u64, on_progress: Rc<dyn Fn(Progress)>, ) -> Result<(), HostError> { self.0.export_paths_into(entry, groups, archive, timeout_ms, on_progress).await }
    async fn machine_run(&self, entry: &Entry, script: &str, deadline_ms: u64) -> Result<ExecResult, HostError> { self.0.machine_run(entry, script, deadline_ms).await }
    async fn machine_exec(&self, entry: &Entry, cmd: &str, timeout_ms: u64) -> Result<ExecResult, HostError> { self.0.machine_exec(entry, cmd, timeout_ms).await }
    async fn kind_import(&self, entry: &Entry, o: ProjectImportOptions, report: ImportReport) -> Result<ImportLanded, HostError> { self.0.kind_import(entry, o, report).await }
    fn temp_dir(&self) -> String { self.0.temp_dir() }
}
#[async_trait(?Send)]
impl ProjectsCtx for Lossy {
    fn state_folder(&self) -> String { self.0.state_folder() }
    fn has_state_path(&self) -> bool { self.0.has_state_path() }
    fn seed_wiring(&self) -> Option<Rc<dyn SeedWiring>> { self.0.seed_wiring() }
    fn cloud_projects(&self) -> Option<bool> { self.0.cloud_projects() }
    fn vault_github_token(&self) -> Option<String> { self.0.vault_github_token() }
    fn local_claude_home(&self) -> Option<String> { self.0.local_claude_home() }
    async fn homes_here(&self) -> Result<IndexMap<String, String>, HostError> { self.0.homes_here().await }
    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError> { self.0.refuse_recording(word, origin) }
    async fn computer_rows(&self) -> Result<Vec<ComputerRow>, HostError> { self.0.computer_rows().await }
    fn name_of_computer(&self, id: &str, rows: &[ComputerRow]) -> String { self.0.name_of_computer(id, rows) }
    async fn clone_here(&self, word: &str, source: &ProjectSource, into: &str) -> Result<String, HostError> { self.0.clone_here(word, source, into).await }
    async fn git_top_of(&self, path: &str) -> Result<Option<String>, HostError> { self.0.git_top_of(path).await }
    async fn remote_here(&self, folder: &str) -> Result<String, HostError> { self.0.remote_here(folder).await }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> { self.0.landing_place(computer).await }
    async fn forking_at(&self, place_id: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError> { self.0.forking_at(place_id).await }
    async fn landing_backend(&self, place_id: &str) -> Result<Arc<dyn MachineBackend>, HostError> { self.0.landing_backend(place_id).await }
    fn folder_computer(&self, place_id: &str) -> Option<JoinedComputer> { self.0.folder_computer(place_id) }
    fn github_from_vault(&self, place_id: &str) -> bool { self.0.github_from_vault(place_id) }
    fn landing_road(&self, kind: LandingKind) -> Rc<dyn LandingRoad> { self.0.landing_road(kind) }
    fn project_source(&self, kind: &ProjectSource) -> Rc<dyn ProjectSourceModule> { self.0.project_source(kind) }
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> { self.0.remember_project(project).await }
    fn project_reached(&self, origin: Option<&Caller>, project_id: &str) -> bool { self.0.project_reached(origin, project_id) }
    fn project_of_scope(&self, origin: Option<&Caller>) -> Option<String> { self.0.project_of_scope(origin) }
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &ProjectView, word: &str) -> Option<HostError> { self.0.elsewhere_refusal(origin, project, word) }
    fn of_threads_repository(&self, origin: Option<&Caller>, project_id: &str) -> bool { self.0.of_threads_repository(origin, project_id) }
    fn holds_thread(&self, workspace_id: &str) -> bool { self.0.holds_thread(workspace_id) }
    async fn delete_workspace(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> { self.0.delete_workspace(id, origin).await }
    async fn copier_worktree_forget(&self, from: &str, home: &str, project: &str) -> Option<Result<(), HostError>> { self.0.copier_worktree_forget(from, home, project).await }
    async fn forget_icon(&self, project_id: &str) -> Result<(), HostError> { self.0.forget_icon(project_id).await }
    async fn set_icon(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, HostError> { self.0.set_icon(project_id, png).await }
    async fn read_icons(&self, hashes: Vec<String>) -> Result<IndexMap<String, Option<String>>, HostError> { self.0.read_icons(hashes).await }
    fn runs_agent(&self, agent: &str) -> bool { self.0.runs_agent(agent) }
    async fn agent_lists(&self, computer: &str, origin: Option<&Caller>) -> Result<Vec<HarnessCatalog>, HostError> { self.0.agent_lists(computer, origin).await }
    fn thread_defaults(&self, project_id: &str, first_agent: &str, lists: &[HarnessCatalog]) -> ThreadDefaults { self.0.thread_defaults(project_id, first_agent, lists) }
    fn first_agent(&self) -> String { self.0.first_agent() }
}


fn lossy(refuses: &str) -> (Rc<FakeProjects>, Rc<wsp_account::projects::projects::Projects>) {
    let fake = FakeProjects::new();
    let ctx = Rc::new(Lossy(fake.clone(), RefCell::new(Some(refuses.to_owned()))));
    (fake, projects_area(ctx))
}

// R-PRJ-E1. The add's first line is said outside its `try`, so a bus that refuses it fails the add (projects.ts:313).
#[test]
fn an_add_fails_with_the_bus_that_refuses_its_stage_line() {
    run(async {
        let (fake, area) = lossy("project.add");
        fake.base.kinds.borrow_mut().insert("here".to_owned(), WorkspaceKind::Place);
        let err = area.add(AddAsk { source: "/home/me/code/lab".to_owned(), seed: Some(SeedChoice { files: vec![], memory: false, commits: false, remember: None }), ..Default::default() }, None).await;
        assert_eq!(err.map(|added| added.project.id).unwrap_err().message, "a listener failed");
    });
}

// R-PRJ-E2. The export's stage lines are said inside its `try`; a bus that refuses them fails the export, and the
// machine and the scratch are cleaned as for any failure.
#[test]
fn an_export_fails_with_the_bus_that_refuses_its_stage_line() {
    run(async {
        let (fake, area) = lossy("project.export");
        fake.base.hold(record("ws_1", "cloud", "pr_x", json!({ "name": "task-1" })));
        let lander = projects::FakeLander::new(&fake.base);
        let opts = ProjectExportOptions {
            workspace_id: "ws_1".to_owned(),
            source: "/root/work/proj".to_owned(),
            dest: "/Users/dev/code/proj".to_owned(),
            replace: None,
            agents: None,
            lander,
        };
        let got = area.export(opts, Some(person())).await;
        assert_eq!(got.map(|done| done.dest).unwrap_err().message, "a listener failed");
    });
}

// R-PRJ-E3. The import's last line, `report("done", ...)`, is said after the folder landed and inside the `try`
// (projects.ts:511), so a bus that refuses it fails the import.
#[test]
fn an_import_fails_with_the_bus_that_refuses_its_done_line() {
    run(async {
        let (fake, area) = lossy("project.import");
        fake.base.hold(record("ws_1", "cloud", "pr_x", json!({ "name": "task-1" })));
        let plan = json!({ "source": "/Users/dev/code/proj", "repo": true, "files": 6, "bytes": 4321, "secrets": [], "values": [], "excluded": [], "skipped": [], "agents": [] });
        let opts = ProjectImportOptions {
            workspace_id: "ws_1".to_owned(),
            source: "/Users/dev/code/proj".to_owned(),
            dest: "/root/work/proj".to_owned(),
            replace: None,
            carry: None,
            rewrite: None,
            agents: None,
            bundler: projects::FakeBundler::new(&fake.base, plan),
        };
        let got = area.import(opts, Some(person())).await;
        assert_eq!(got.map(|done| done.dest).unwrap_err().message, "a listener failed");
    });
}
