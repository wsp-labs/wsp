// SPDX-License-Identifier: AGPL-3.0-only
//! What the projects areas ask of the runtime: the base every one of them shares (`Base`), the folders, start-from and
//! create halves (`FoldersCtx`, `StartFromCtx`, `CreateCtx`) and the whole they make together (`WorkspacesCtx`)
//! (workspaces.ts, folders.ts, start-from.ts, runtime.ts): the context's own state, a forward to the area that holds
//! each call, and the few lines no area has.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_account::account::channels::copy::ChildRoad;
use wsp_account::account::core_types::{AccountError, Ask};
use wsp_account::account::rules::like_of;
use wsp_account::projects::folders::tree::ListedWorktree;
use wsp_account::projects::folders::FoldersCtx;
use wsp_account::projects::projects::add::LandingKind;
use wsp_account::projects::start_from::{OpenedSession, ResolvedPicks, StartFromCtx};
use wsp_account::projects::workspaces::create::{CreateCtx, LeadStart};
use wsp_account::projects::workspaces::seam::{
    Base, DaemonAsk, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld, Queued,
};
use wsp_core::clock::Clock;
use wsp_core::context::{RuntimeState, StateCell};
use wsp_core::harness_catalog::{harness_catalog, harness_catalogs, not_run_refused};
use wsp_core::ids::Ids;
use wsp_core::types::api::{StartAs, StartOptions, WorkspacesApi};
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::internal::{FolderAdmit, ThreadRecord, WorkspaceLike};
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::{CreateWorkspaceOptions, CreatedWorkspace, HereDaemon, StageReport, SubAsk, WorkspaceRecord, NO_COPIER_HERE};
use wsp_engine::copier::{Copier, WorktreeAsk, WorktreeAt, WorktreeRemove, WorktreeReport};
use wsp_engine::errors::{EngineError, ErrorKind};
use wsp_engine::local_backend::LOCAL_MACHINE_ID;
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::machines::area_error::AreaError;
use wsp_machines::machines::create::CreateArea;
use wsp_machines::machines::sub_spawn::SpawnAsk;
use wsp_machines::projects::branch_of_record::brokered_start;
use wsp_threads::threads::agents::launch::AdapterAsk;
use wsp_threads::threads::agents::AgentsArea;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::sub_thread::SubStarted;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::harness::{start_picks, HarnessCatalog, StartPicks};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{scope_of, BranchOfRecord, Caller, ProjectView, WorkspaceAgents, WorkspaceAgentsPatch};
use wsp_wire::views::workspace_events::WorkspaceCreatingEvent;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceView};
use wsp_wire::wire::capabilities::{Capabilities, WorkspaceSize};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::{HostError, Kind};
use wsp_words::thread_defaults::{pick_refusal, PickError};
use wsp_words::words::computer::{AcrossAct, SpawnAct};

use super::{Ctx, Joined};
use crate::runtime::join::spawn_detached;

/// The refusal a host with no daemon binary beside it gives where a worktree is asked of its copier (folders.ts:75).
fn no_copier() -> HostError {
    HostError::with_kind(NO_COPIER_HERE, Kind::Invalid)
}

fn engine_said(e: EngineError) -> HostError {
    AreaError::from(e).into_host()
}

/// A host error as the engine's error a caller reads `is_missing` off: only the kind is kept.
fn engine_error_of(e: HostError) -> EngineError {
    let kind = (e.kind == Some(Kind::Missing)).then_some(ErrorKind::Missing);
    EngineError::Refused { message: e.message, kind, status: e.status.map(|s| s as u16), code: e.code }
}

/// `SPAWN_ACTS` keys as the TS callers spell them ("fork", "bring_back", "size").
fn spawn_act(word: &str) -> Result<SpawnAct, HostError> {
    serde_json::from_value(Value::String(word.to_string())).map_err(|_| HostError::new(format!("no such act: {word}")))
}

fn across_act(word: &str) -> Result<AcrossAct, HostError> {
    serde_json::from_value(Value::String(word.to_string())).map_err(|_| HostError::new(format!("no such act: {word}")))
}

impl Ctx {
    fn folders_copier(&self) -> Result<Rc<dyn Copier>, HostError> {
        self.opts.local.as_ref().and_then(|local| local.copier.clone()).ok_or_else(no_copier)
    }

    /// `ctx.backendOfKind("local").get(LOCAL_MACHINE_ID)`, this computer's own machine.
    async fn folders_local_machine(&self) -> Result<Arc<dyn Machine>, HostError> {
        let backend = self.kinds.get().backend_of_kind(WorkspaceKind::Local, None)?;
        backend.get(LOCAL_MACHINE_ID).await.map_err(engine_said)
    }
}

/// The work a daemon ask takes, over the frame the road speaks: a request that is no JSON object is refused where the
/// TS would send it and be answered with the daemon's own refusal.
fn ask_over(ask: Ask) -> DaemonAsk {
    Rc::new(move |frame: Value| match frame {
        Value::Object(frame) => {
            let sent = ask(frame);
            async move { sent.await.map(Value::Object).map_err(HostError::from) }.boxed_local()
        }
        other => async move { Err(HostError::new(format!("a daemon frame is an object, not {other}"))) }.boxed_local(),
    })
}

#[async_trait::async_trait(?Send)]
impl Base for Ctx {
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn live(&self) -> Live {
        self.live.clone()
    }
    fn projects_held(&self) -> ProjectsHeld {
        self.projects_held.clone()
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        self.channels.get().project_held(project_id)
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.sessions.borrow().values().map(|row| row.borrow().view.clone()).collect()
    }
    fn thread_records(&self) -> Vec<(String, ThreadRecord)> {
        self.thread_records.borrow().iter().map(|(id, record)| (id.clone(), record.clone())).collect()
    }
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError> {
        let records = self.records.get();
        records.persist(&record).await
    }
    fn view(&self, record: &WorkspaceRecord) -> Result<WorkspaceView, HostError> {
        self.records.get().view(record)
    }
    fn emit(&self, event: Event) -> Result<(), HostError> {
        let event = serde_json::from_value(event).map_err(|e| HostError::new(e.to_string()))?;
        self.bus.emit(event).map(|_| ())
    }
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        let boot = self.boot.get();
        boot.entry_of(id, origin.as_ref(), ask).await
    }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError> {
        self.rules.get().spawn_guard(spawn_act(act)?, origin)
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        self.kinds.get().kind_of(computer)
    }
    fn place_name(&self, place_id: &str) -> String {
        wsp_machines::machines::kinds::KindsCtx::place_name(self, place_id)
    }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        let channels = self.channels.get();
        channels.copy_blocked(entry).await
    }
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<Value, HostError> {
        let channels = self.channels.get();
        let work: wsp_account::account::core_types::Work<Value> =
            Box::new(move |ask: Ask| work(ask_over(ask)).map(|done| done.map_err(AccountError::from)).boxed_local());
        channels.with_daemon(entry, work).await.map_err(HostError::from)
    }
    async fn on_this_computer(&self, work: HereWork) -> Result<Value, HostError> {
        let channels = self.channels.get();
        let work: wsp_account::account::channels::daemon::HomeWork<Value> =
            Box::new(move |ask: Ask, home: String| work(ask_over(ask), home).map(|done| done.map_err(AccountError::from)).boxed_local());
        channels.on_this_computer(work).await.map_err(HostError::from)
    }
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError> {
        let kinds = self.kinds.get();
        let args: Vec<&str> = ask.args.iter().map(String::as_str).collect();
        kinds.git_here(&ask.cwd, &args, ask.timeout_ms, ask.input.as_deref()).await.map_err(|e| e.into_host())
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        self.channels.get().checkout_of(record)
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        let preferences = self.preferences.get();
        preferences.get().await.map_err(HostError::from)
    }
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError> {
        let daemon = self.daemon.get();
        daemon.write_daemon_roots(entry, strict).await
    }
    fn home_dir(&self) -> String {
        match &self.opts.local {
            Some(local) => local.home_dir.clone(),
            None => std::env::var("HOME").unwrap_or_default(),
        }
    }
    fn machine_id(&self, entry: &Entry) -> String {
        entry.borrow().machine.id().to_string()
    }
}

#[async_trait::async_trait(?Send)]
impl FoldersCtx for Ctx {
    fn state_folder(&self) -> Result<String, HostError> {
        self.kinds.get().state_folder()
    }
    fn has_state_path(&self) -> bool {
        self.opts.state_path.is_some()
    }
    fn has_copier(&self) -> bool {
        self.opts.local.as_ref().is_some_and(|local| local.copier.is_some())
    }
    async fn copier_worktree(&self, ask: WorktreeAsk) -> Result<WorktreeReport, HostError> {
        let copier = self.folders_copier()?;
        copier.worktree(&ask).await.map_err(engine_said)
    }
    async fn copier_worktree_mount(&self, from: &str, home: &str, path: &str) -> Result<(), HostError> {
        let copier = self.folders_copier()?;
        copier.worktree_mount(&WorktreeAt { from: from.into(), home: home.into(), path: path.into() }).await.map_err(engine_said)
    }
    async fn copier_worktree_remove(&self, ask: WorktreeRemove) -> Result<(), HostError> {
        let copier = self.folders_copier()?;
        copier.worktree_remove(&ask).await.map(|_| ()).map_err(engine_said)
    }
    fn here_daemon(&self) -> Option<Rc<dyn HereDaemon>> {
        self.opts.local.as_ref().and_then(|local| local.here_daemon.clone())
    }
    async fn admit_folder(&self, kind: WorkspaceKind, name: &str, place: Option<&str>) -> Result<(), HostError> {
        let module = self.kinds.get().module_of(kind)?;
        match module.admit_folder(FolderAdmit { kind, place: place.map(str::to_string), name: name.to_string() }) {
            Some(admitted) => admitted.await,
            None => Ok(()),
        }
    }
    async fn folder_machine(&self, kind: WorkspaceKind, place: Option<&str>) -> Result<(Arc<dyn Machine>, WorkspaceSize), HostError> {
        let backend: Arc<dyn MachineBackend> = self.kinds.get().backend_of_kind(kind, place)?;
        let machine = backend.get(place.unwrap_or(LOCAL_MACHINE_ID)).await.map_err(engine_said)?;
        let size = backend.pricing().default_size();
        Ok((machine, WorkspaceSize { cpu: size.cpu.into(), mem_mb: size.mem_mb.into() }))
    }
    fn attach(&self, record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Result<Entry, HostError> {
        let machines = self.machines.get();
        machines.attach(&Rc::new(RefCell::new(record)), machine)
    }
    fn folders_of(&self, project_id: &str) -> Vec<Entry> {
        self.create.get().folders_of(project_id)
    }
    async fn one_folder(&self, key: &str, make: Queued<Entry>) -> Result<Entry, HostError> {
        let create = self.create.get();
        create.one_folder(key, Box::new(move || make)).await
    }
    async fn resolve_project(&self, word: &str, origin: Option<Caller>) -> Result<ProjectView, HostError> {
        let projects = self.projects.get();
        projects.resolve(word, origin).await
    }
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &ProjectView, word: &str) -> Result<Option<HostError>, HostError> {
        self.rules.get().elsewhere_refusal(origin, project, word)
    }
    async fn picks_hold(&self, project: &ProjectView, picks: &StartPicksAsked) -> Result<(), HostError> {
        let start_from = self.start_from.get();
        start_from.picks_hold(project, picks).await
    }
    async fn branch_at(&self, path: &str) -> Result<Option<String>, HostError> {
        let kinds = self.kinds.get();
        kinds.branch_at(path).await.map_err(|e| e.into_host())
    }
    fn turn_runs(&self, workspace_id: &str) -> bool {
        self.daemon.get().turn_runs(workspace_id)
    }
    async fn drop_workspace(&self, workspace_id: &str) -> Result<(), HostError> {
        let machines = self.machines.get();
        machines.drop(workspace_id).await
    }
    async fn checkpoints_landed(&self, thread_id: &str) {
        let landing = self.threads.get().checkpoints_landing.borrow().get(thread_id).cloned();
        if let Some(landing) = landing {
            landing.await;
        }
    }
    fn not_a_repo(&self, record: &WorkspaceRecord) -> Result<bool, HostError> {
        self.threads.get().not_a_repo(record)
    }
    async fn read_pull_request(&self, entry: &Entry, force: bool) -> Result<(), HostError> {
        let pull_requests = self.pull_requests.get();
        pull_requests.read_pull_request(entry, force, false).await;
        Ok(())
    }
    async fn status_now(&self, entry: &Entry) -> Result<(), HostError> {
        let reach = self.reach.get();
        reach.status_now(entry).await.map_err(|e| e.into_host())
    }
    async fn local_exec(&self, cmd: &str, timeout_ms: u64, stdin: Option<Vec<u8>>) -> Result<ExecResult, HostError> {
        let machine = self.folders_local_machine().await?;
        let opts = wsp_engine::machine::ExecOptions { timeout_ms: Some(timeout_ms), stdin, ..Default::default() };
        machine.exec(cmd, opts).await.map_err(engine_said)
    }
    fn state(&self) -> StateCell<RuntimeState> {
        self.state.clone()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
}

#[async_trait::async_trait(?Send)]
impl StartFromCtx for Ctx {
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn places_wired(&self) -> String {
        self.places.wired()
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        let projects = self.projects.get();
        Ok(projects.landing_deps(computer).await?.place_id)
    }
    fn landing_kind(&self, computer: &str) -> LandingKind {
        self.projects.get().landing_kind(computer)
    }
    fn brokered_start(&self, provider: bool) -> bool {
        brokered_start(self.opts.broker.as_ref().is_some_and(|broker| broker.ready()), provider)
    }
    async fn pull_request_of_record(
        &self,
        remote: &str,
        number: i64,
        project: &str,
        computer: &str,
        start: bool,
    ) -> Result<Option<BranchOfRecord>, HostError> {
        let branch = self.branch_area();
        branch.pull_request_of_record(remote, number as u64, project, computer, start).await.map(Some)
    }
    async fn create_workspace(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError> {
        let workspaces = self.workspaces.get();
        workspaces.create(opts, origin).await
    }
    async fn delete_workspace(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        let workspaces = self.workspaces.get();
        workspaces.delete(id, origin).await
    }
    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError> {
        let folders = self.folders.get();
        folders.project_folder(project).await
    }
    async fn worktree_folder(&self, project: &ProjectView, top: &str, branch: &str) -> Result<Entry, HostError> {
        let folders = self.folders.get();
        folders.worktree_folder(project, top, branch, None, None, None).await
    }
    async fn worktrees_of(&self, top: &str) -> Result<Vec<ListedWorktree>, HostError> {
        let folders = self.folders.get();
        folders.worktrees_of(top).await
    }
    fn take_setup_line(&self, workspace_id: &str) -> Option<String> {
        self.folders.get().take_setup_line(workspace_id)
    }
    fn preferences_held(&self) -> Option<Preferences> {
        self.state.with(|state| state.preferences_held.clone())
    }
    fn default_agent_of(&self, prefs: &Preferences, entry: &Entry) -> String {
        self.agents.get().default_agent_of(prefs, Some(entry))
    }
    async fn resolved_picks(
        &self,
        home: &Entry,
        harness: &str,
        prefs: &Preferences,
        project_id: &str,
    ) -> Result<Option<ResolvedPicks>, HostError> {
        let Some(table) = harness_catalog(harness) else {
            return Ok(None);
        };
        let agents = self.agents.get();
        let built = agents.launch_adapter_for(home, AdapterAsk { named: Some(harness.to_string()), ..Default::default() }).await?;
        let catalog = agents.catalog_on(&table, home, built.adapter).await;
        let defaults = AgentsArea::defaults_on(&catalog, prefs, prefs.project_defaults.get(project_id).map(|overrides| &**overrides));
        Ok(Some(ResolvedPicks { catalog: defaults.catalog, open_model: defaults.open_model, open_effort: defaults.open_effort }))
    }
    fn access_word_refusal(&self, catalog: &HarnessCatalog, access: &AccessChoice) -> Option<HostError> {
        wsp_words::thread_defaults::access_word_refusal(catalog, *access)
    }
    fn named_mode(&self, catalog: &HarnessCatalog, harness: &str, access: &AccessChoice) -> Result<Option<String>, HostError> {
        AgentsArea::named_mode(Some(catalog), harness, *access).map(Some)
    }
    fn start_picks_refusal(&self, catalog: &HarnessCatalog, picks: &StartPicksAsked) -> Option<HostError> {
        let asked = StartPicks {
            model: picks.model.clone(),
            effort: picks.effort.clone(),
            permission_mode: picks.permission_mode.clone(),
            fast: (picks.fast == Some(true)).then_some(true),
        };
        let refused = start_picks(Some(catalog), &asked, true, None).err()?;
        let said = PickError { message: refused.message, kind: Some(refused.kind.to_string()), offered: refused.offered };
        Some(not_run_refused(Some(catalog), picks.model.as_deref()).unwrap_or_else(|| pick_refusal(&said, Some(catalog))))
    }
    async fn start_session(&self, workspace_id: &str, opts: Value, origin: Option<Caller>) -> Result<OpenedSession, HostError> {
        let word = |key: &str| opts.get(key).and_then(Value::as_str).map(str::to_string);
        let started_as = opts.get("as").and_then(|as_| {
            Some(StartAs { thread_id: as_.get("threadId")?.as_str()?.to_string(), turn_id: as_.get("turnId")?.as_str()?.to_string() })
        });
        let start = StartOptions {
            prompt: word("prompt").unwrap_or_default(),
            harness: word("harness"),
            model: word("model"),
            effort: word("effort"),
            permission_mode: word("permissionMode"),
            access: opts.get("access").and_then(|access| serde_json::from_value(access.clone()).ok()),
            r#as: started_as,
            behind: word("behind"),
            setup: word("setup"),
            ..Default::default()
        };
        let sessions = self.sessions_area.get();
        let handle = wsp_core::types::api::SessionsApi::start(&*sessions, workspace_id, start, origin).await?;
        Ok(handle.view())
    }
    fn read_only_modes(&self) -> Vec<(String, Option<String>)> {
        harness_catalogs().into_iter().map(|catalog| (catalog.harness, catalog.read_only_mode)).collect()
    }
    async fn send_detached(&self, workspace_id: &str, prompt: &str, thread: &str) -> Result<(), HostError> {
        let daemon = self.daemon.get();
        daemon.send_detached(workspace_id, prompt, Some(thread), None).await.map(|_| ())
    }
}

#[async_trait::async_trait(?Send)]
impl CreateCtx for Ctx {
    fn places_wired(&self) -> String {
        self.places.wired()
    }
    async fn resolve_project(&self, word: &str, origin: Option<Caller>) -> Result<ProjectView, HostError> {
        let projects = self.projects.get();
        projects.resolve(word, origin).await
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        let boot = self.boot.get();
        boot.landing_place(computer).await
    }
    async fn landing_capabilities(&self, place_id: Option<&str>) -> Result<Capabilities, HostError> {
        let builders = self.builders_area.get();
        Ok(builders.landing_backend(place_id).await?.capabilities())
    }
    fn backend_capabilities(&self, kind: WorkspaceKind, place: Option<&str>) -> Result<Capabilities, HostError> {
        Ok(self.kinds.get().backend_of_kind(kind, place)?.capabilities())
    }
    fn act_refusal(&self, scope: &ThreadScope, act: &str) -> Result<HostError, HostError> {
        self.rules.get().act_refusal(scope, spawn_act(act)?)
    }
    async fn spawn_sub(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError> {
        let SubAsk { on, size, from, branch, harness } = opts.sub.clone().unwrap_or_default();
        let ask = SpawnAsk { name: opts.name.clone(), project: opts.project.clone(), on, size, from, branch, harness };
        let sub_spawn = self.sub_spawn.get();
        let answer = sub_spawn.spawn_sub(ask, origin.as_ref()).await?;
        let view = serde_json::from_value(answer.view).map_err(|e| HostError::new(e.to_string()))?;
        let spawned = answer.sub;
        let sub = Some(SubStarted { out: spawned.out, folder: spawned.folder, preamble: spawned.preamble, harness: spawned.harness });
        Ok(CreatedWorkspace { view, notice: answer.notice, sub })
    }
    fn place_of_thread(&self, scope: &ThreadScope) -> Result<Option<String>, HostError> {
        self.rules.get().place_of_thread(scope)
    }
    fn name_given(&self, name: &str) -> String {
        CreateArea::name_given(name)
    }
    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.rules.get().refuse_recording(word, origin)
    }
    fn refuse_relayed(&self, kind: WorkspaceKind, name: &str, project: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        let record = WorkspaceLike {
            id: None,
            kind,
            name: name.to_string(),
            machine_id: None,
            root_thread_id: None,
            project: Some(project.to_string()),
            worktree: None,
        };
        self.rules.get().refuse_relayed(Some(&record), origin, AcrossAct::Work, None)
    }
    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError> {
        let folders = self.folders.get();
        folders.project_folder(project).await
    }
    fn agents_from(&self, place: &str, agents: &Map<String, Value>) -> WorkspaceAgents {
        let held = self.channels.get().spawn_at(Some(place));
        let field = |key: &str| agents.get(key).and_then(Value::as_i64).map(JsInt::from);
        let patch = WorkspaceAgentsPatch { max_machines: field("maxMachines"), max_depth: field("maxDepth") };
        wsp_wire::views::workspace::agents_from(Some(&held), &patch)
    }
    fn same_repository(&self, a: &ProjectView, b: &ProjectView, origin: Option<&Caller>) -> bool {
        self.channels.get().same_repository(a, b, origin)
    }
    async fn place_guard(&self, place_id: &str) -> Result<(), HostError> {
        let channels = self.channels.get();
        channels.place_guard(place_id).await
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        self.create.get().name_refusal(name)
    }
    fn forking_add(&self, name: &str) {
        self.create.get().forking.borrow_mut().insert(name.to_string());
    }
    fn forking_delete(&self, name: &str) {
        self.create.get().forking.borrow_mut().shift_remove(name);
    }
    fn supersede_failed(&self, name: &str) {
        self.create.get().supersede_failed(name);
    }
    fn stage_reporter(&self, id: &str, name: &str, began: i64, scope: Option<&ThreadScope>) -> StageReport {
        self.create.get().stage_reporter(id, name, began, scope)
    }
    async fn lead_start(&self, parent: &Entry, name: &str, how: &str) -> Result<LeadStart, HostError> {
        let road = if how == "fork" { ChildRoad::Fork } else { ChildRoad::Run };
        let channels = self.channels.get();
        let started = channels.lead_start(parent, name, road).await.map_err(HostError::from)?;
        Ok(LeadStart { base: started.base, on_leads: started.on_leads, lines: started.lines })
    }
    fn show_create(&self, view: WorkspaceView) {
        self.create.get().show_create(view);
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
        let tree = self.rules.get().tree_of(Some(scope));
        let mut view = Map::new();
        view.insert("id".into(), json!(id));
        view.insert("name".into(), json!(name));
        view.insert("machineId".into(), json!(""));
        view.insert("phase".into(), json!("queued"));
        view.insert("kind".into(), json!(kind));
        view.insert("golden".into(), json!(golden));
        view.insert("createdAt".into(), json!(created_at));
        view.insert("project".into(), json!(self.channels.get().ref_of(project)));
        if let Some(thread) = tree.parent_thread_id {
            view.insert("parentThreadId".into(), json!(thread));
        }
        if let Some(thread) = tree.root_thread_id {
            view.insert("rootThreadId".into(), json!(thread));
        }
        if let Some(parent) = parent {
            view.insert("parentWorkspaceId".into(), json!(parent));
        }
        serde_json::from_value(Value::Object(view)).expect("a queued workspace view is what this object spells")
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
        let create = self.create.get();
        let landed: Option<Box<dyn Fn()>> = Some(Box::new(move || free()));
        create.create_staged(&opts, project, id, report, scope.as_ref(), landed, base.as_deref()).await
    }
    fn child_started_line(&self, name: &str, branch: &str) -> String {
        wsp_words::tree::child_started_line(name, branch)
    }
    async fn start_child_on(&self, child: &Entry, base: &str, parent_name: &str) -> Result<String, HostError> {
        let channels = self.channels.get();
        channels.start_child_on(child, base, parent_name).await.map_err(HostError::from)
    }
    async fn unfork(&self, entry: &Entry) -> Result<(), EngineError> {
        let machines = self.machines.get();
        machines.unfork(entry).await.map_err(engine_error_of)
    }
    fn shown_creates(&self) -> Rc<RefCell<IndexMap<String, WorkspaceView>>> {
        self.create.get().shown_creates.clone()
    }
    fn failed_creates(&self) -> Rc<RefCell<IndexMap<String, WorkspaceView>>> {
        self.create.get().failed_creates.clone()
    }
    fn failed_view(
        &self,
        id: &str,
        name: &str,
        kind: WorkspaceKind,
        golden: &str,
        began: i64,
        project: &ProjectView,
        said: &str,
    ) -> WorkspaceView {
        self.create.get().failed_view(id, name, kind, golden, began, project, said)
    }
    async fn boot_work(&self, id: &str) {
        let work: Option<Joined> = self.boot_work.borrow().get(id).cloned();
        if let Some(work) = work {
            work.await;
        }
    }
    fn listed_for(&self, origin: Option<&Caller>) -> Result<Vec<Entry>, HostError> {
        self.rules.get().listed_for(origin)
    }
    fn held(&self) -> Vec<Entry> {
        self.rules.get().held()
    }
    fn drives(&self, view: &WorkspaceView, origin: Option<&Caller>) -> bool {
        self.rules.get().drives(view.kind.unwrap_or(WorkspaceKind::Cloud), Some(&view.machine_id), origin)
    }
    fn talks_to_tree_on(&self, id: &str, origin: Option<&Caller>) -> bool {
        self.boot.get().talks_to_tree_on(id, origin).unwrap_or(false)
    }
    fn project_of_scope(&self, origin: Option<&Caller>) -> Option<String> {
        self.rules.get().project_of_scope(&scope_of(origin)?)
    }
    fn refusal_for(&self, record: Option<&WorkspaceRecord>, origin: Option<&Caller>) -> Result<Option<String>, HostError> {
        self.rules.get().refusal_for(record.map(like_of).as_ref(), origin)
    }
    fn of_threads_repository(&self, origin: Option<&Caller>, project_id: &str) -> Result<bool, HostError> {
        self.rules.get().of_threads_repository(origin, project_id)
    }
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &ProjectView, word: &str) -> Result<Option<HostError>, HostError> {
        self.rules.get().elsewhere_refusal(origin, project, word)
    }
    fn away_for(&self, record: &WorkspaceRecord, origin: Option<&Caller>, act: &str, word: &str) -> Result<Option<HostError>, HostError> {
        self.rules.get().away_for(&like_of(record), origin, across_act(act)?, word)
    }
    fn project_reached(&self, origin: Option<&Caller>, project_id: &str) -> Result<bool, HostError> {
        self.rules.get().project_reached(origin, project_id)
    }
    async fn find_places(&self, word: &str) -> Result<Vec<String>, HostError> {
        match self.wired_door() {
            Some(door) => Ok(door.find(word).await?.into_iter().map(|place| place.name).collect()),
            None => Ok(Vec::new()),
        }
    }
    fn create_stages(&self) -> Vec<WorkspaceCreatingEvent> {
        self.create.get().create_stages.borrow().values().cloned().collect()
    }
}

impl wsp_account::projects::workspaces::WorkspacesCtx for Ctx {}
