// SPDX-License-Identifier: AGPL-3.0-only
//! The context's kinds, rules, records, held-image and branch-of-record readings, and the place door's adapters
//! (kinds.ts, rules.ts, records.ts, held.ts, thread-ends.ts, branch-of-record.ts, runtime.ts's `placeDoor` reads): most
//! of them one call on `opts`, the core's maps or the door, the rest a forward to the area that holds the rule.
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_account::account::core_types::Entry as AccountEntry;
use wsp_account::account::rules::{RulesCtx, TreeFacts};
use wsp_adapters::codex::command::{AccountModels, CodexBroker};
use wsp_adapters::pending::CodexClaims;
use wsp_core::devices::{MintMore, PairedDevice};
use wsp_core::host_id::hostname;
use wsp_core::project_landing::{self, BrokeredClone, CloneAsk, GitEnvRepo};
use wsp_core::project_sources::project_source;
use wsp_core::types::internal::{released_backend, EndThread, ImportLanded, ImportReport, ThreadRecord};
use wsp_core::types::pending::{Clock, MachineExecOptions, Store};
use wsp_core::types::wiring::{at_provider, LiveWorkspace, ProjectImportOptions, WorkspaceRecord};
use wsp_engine::local_backend::LOCAL_MACHINE_ID;
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineBackend};
use wsp_machines::agents_read::{AgentsPlaces, GhStep};
use wsp_machines::images::builders::ImageDoor;
use wsp_machines::images::held::HeldCtx;
use wsp_machines::images::records::RecordsCtx;
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::brokered::{self, Placeholders};
use wsp_machines::machines::kinds::{BrokerSaid, KindsCtx};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_life::BackendFor;
use wsp_machines::machines::thread_ends::{OwedEnd, ThreadEndsDoor, ThreadEndsRuntime};
use wsp_machines::places::types::{FolderComputer, ForwardAsk, PlaceDoor, PlaceExecOpts};
use wsp_machines::projects::branch_of_record::{BranchCtx, BranchOfRecordArea, MarketplaceFile, OnRecordEntry};
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::workspace::{BranchOfRecord, Caller, DaemonReachView, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceAgents};
use wsp_wire::wire::capabilities::forks_no_machines;
use wsp_wire::wire::machine_link::ExecResult as WireExecResult;
use wsp_wire::wire::place_link::PlaceReport;
use wsp_wire::wire::runtime::BrokerStatus;
use wsp_words::exit::HostError;
use wsp_words::place_name::codex_login_unread_refusal;
use wsp_words::place_word::provider_key_name;
use wsp_words::words::computer::no_kind_line;
use wsp_words::workspace_state::machine_word;

use super::Ctx;

thread_local! {
    static RELEASED: Arc<dyn MachineBackend> = released_backend();
}

/// The TypeError a `placeDoor.<call>` leaves where the host wired no door.
fn no_door(call: &str) -> HostError {
    HostError::new(format!("Cannot read properties of undefined (reading '{call}')"))
}

fn count_ms(ms: f64) -> i64 {
    ms as i64
}

/// Node's words for a file this computer would not read.
fn read_failure(e: &std::io::Error, path: &str) -> HostError {
    let (code, text) = match e.raw_os_error() {
        Some(2) => ("ENOENT", "no such file or directory"),
        Some(13) => ("EACCES", "permission denied"),
        Some(21) => ("EISDIR", "illegal operation on a directory"),
        _ => return HostError::new(e.to_string()),
    };
    HostError::new(format!("{code}: {text}, open '{path}'"))
}

impl Ctx {
    /// `ctx.placeName` (builders.ts:242): the door's name for the place, else this computer's hostname or the
    /// provider's own name. Read here and not through the builders area, whose context is a later packet's and which
    /// every create reads.
    fn place_name_of(&self, place: &str) -> String {
        match self.wired_door() {
            Some(door) => door.name_of(place),
            None if place == HERE_PLACE_ID => hostname(),
            None => provider_key_name(place),
        }
    }

    fn door_or(&self, call: &str) -> Result<Rc<dyn PlaceDoor>, HostError> {
        self.wired_door().ok_or_else(|| no_door(call))
    }

    fn local_wiring(&self) -> Result<&wsp_core::types::wiring::LocalWiring, HostError> {
        self.opts.local.as_ref().ok_or_else(|| HostError::new(no_kind_line("local")))
    }

    /// The live entry holding this record: what a kind's area reads where the trait hands a borrow of one.
    fn live_entry_of(&self, entry: &LiveWorkspace) -> Option<Entry> {
        self.live.borrow().values().find(|held| Rc::ptr_eq(&held.borrow().record, &entry.record)).cloned()
    }

    /// The branch-of-record roads over this context (runtime.ts:551-552): one area per runtime, filled with the core,
    /// which holds the context weakly so a dropped runtime is freed.
    pub fn branch_area(&self) -> Rc<BranchOfRecordArea> {
        self.branch_of_record.get()
    }

    /// `branchOfRecord(ctx)` for the slot `build_core` fills.
    pub(super) fn build_branch_area(&self) -> Rc<BranchOfRecordArea> {
        BranchOfRecordArea::new(Rc::new(BranchOver(self.me.clone())))
    }
}

/// `isBrokered(record, atProvider(record.place, places) ? { kind: "provider" } : undefined)`.
fn brokered_at_provider(ctx: &Ctx, record: &WorkspaceRecord) -> bool {
    record.kind == WorkspaceKind::Cloud && (record.place.is_none() || at_provider(record.place.as_deref(), &*ctx.places))
}

#[async_trait(?Send)]
impl KindsCtx for Ctx {
    fn now(&self) -> f64 {
        self.clock.now() as f64
    }
    fn state_path(&self) -> Option<String> {
        self.opts.state_path.clone()
    }
    async fn local_machine(&self) -> AreaResult<Arc<dyn Machine>> {
        let backend = self.local_wiring()?.backend.clone();
        Ok(backend.get(LOCAL_MACHINE_ID).await?)
    }
    fn own_backend(&self) -> Arc<dyn MachineBackend> {
        self.backend.clone()
    }
    fn released_backend(&self) -> Arc<dyn MachineBackend> {
        RELEASED.with(Arc::clone)
    }
    fn place_name(&self, place: &str) -> String {
        self.place_name_of(place)
    }
    fn wired_place(&self) -> String {
        self.places.wired()
    }
    fn project_held(&self, id: &str) -> AreaResult<ProjectView> {
        Ok(self.channels.get().project_held(id)?)
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> AreaResult<String> {
        Ok(self.channels.get().checkout_of(record)?)
    }
    fn computer_of(&self, entry: &LiveWorkspace) -> String {
        let record = entry.record.borrow();
        match (&record.place, self.wired_door()) {
            (Some(place), Some(door)) => door.name_of(place),
            (Some(place), None) => place.clone(),
            (None, _) => machine_word(&record.kind),
        }
    }
    fn served_by_its_computer(&self, entry: &LiveWorkspace) -> bool {
        entry.machine.offers().daemon_frame
    }
    fn has_place_door(&self) -> bool {
        self.place_door.get().is_some()
    }
    fn door_backend_of(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.wired_door()?.backend_of(place)
    }
    fn door_name_of(&self, place: &str) -> String {
        self.wired_door().map_or_else(|| place.to_string(), |door| door.name_of(place))
    }
    fn door_folder_computer(&self, place: &str) -> Option<FolderComputer> {
        self.wired_door()?.folder_computer(place)
    }
    fn door_joined(&self, place: &str) -> bool {
        self.wired_door().is_some_and(|door| door.joined(place))
    }
    fn door_logins_of(&self, place: &str) -> Option<String> {
        self.wired_door()?.backend_of(place)?.logins()
    }
    fn door_vault_at(&self, place: &str, vault: &IndexMap<String, String>) -> Option<IndexMap<String, String>> {
        Some(self.wired_door()?.vault_at(place, vault))
    }
    fn door_sign_ins_at(&self, place: &str) -> Option<IndexMap<String, AgentSignInState>> {
        self.wired_door()?.sign_ins_at(place, None)
    }
    fn door_github_from_vault(&self, place: &str) -> bool {
        self.wired_door().is_some_and(|door| door.github_from_vault(place))
    }
    async fn door_exec(&self, place: &str, cmd: &str, timeout_ms: u64) -> AreaResult<ExecResult> {
        let door = self.door_or("exec")?;
        Ok(door.exec(place, cmd, PlaceExecOpts { timeout_ms: Some(timeout_ms), stdin: None }).await?)
    }
    async fn door_forward(&self, place: &str, port: u16, pane: Option<(String, String)>, standing: bool) -> AreaResult<u16> {
        let door = self.door_or("forward")?;
        Ok(door.forward(place, port, ForwardAsk { pane, standing }).await?)
    }
    async fn door_report_daemon_version(&self, place: &str) -> Option<u32> {
        let report: PlaceReport = self.wired_door()?.report_of(place).await.ok()??;
        u32::try_from(report.daemon_version.get()).ok()
    }
    fn vault(&self) -> IndexMap<String, String> {
        self.opts.vault.as_ref().map(|vault| vault()).unwrap_or_default()
    }
    fn broker_ready(&self) -> bool {
        self.opts.broker.as_ref().is_some_and(|broker| broker.ready())
    }
    fn has_broker_status(&self) -> bool {
        self.opts.broker.as_ref().is_some_and(|broker| broker.status().is_some())
    }
    async fn broker_status(&self) -> AreaResult<BrokerStatus> {
        let read = self.opts.broker.as_ref().and_then(|broker| broker.status());
        match read {
            Some(read) => Ok(read.await?),
            None => Err(HostError::new("broker.status is not a function").into()),
        }
    }
    async fn broker_codex_claims(&self) -> AreaResult<Option<CodexClaims>> {
        match self.opts.broker.clone() {
            Some(broker) => Ok(broker.codex_claims().await?),
            None => Ok(None),
        }
    }
    fn broker_codex_models(&self) -> Option<AccountModels> {
        let broker = self.opts.broker.clone()?;
        Some(Arc::new(move |client_version: String| {
            let read = broker.codex_models(&client_version);
            async move {
                match read {
                    Some(read) => read.await.map(Some).map_err(|e| e.message),
                    None => Ok(None),
                }
            }
            .boxed_local()
        }))
    }
    fn is_brokered(&self, record: &WorkspaceRecord) -> bool {
        brokered_at_provider(self, record)
    }
    fn codex_broker_of(&self, said: &BrokerSaid) -> Option<CodexBroker> {
        brokered::codex_broker_of(said.status.as_ref(), said.claims.as_ref(), self.broker_codex_models())
    }
    fn unbrokered_refusal(&self, harness: &str) -> Option<HostError> {
        brokered::unbrokered_refusal(harness)
    }
    fn codex_login_unread_refusal(&self) -> HostError {
        codex_login_unread_refusal()
    }
    fn placeholders_of(&self, said: &BrokerSaid, token: &str) -> Placeholders {
        brokered::placeholders_of(said.status.as_ref(), token)
    }
    fn brokered_env(&self, token: &str, record: &WorkspaceRecord) -> IndexMap<String, String> {
        let repo = record.branch_of_record.as_ref().map(|of_record| brokered::RepoOfRecord {
            owner: of_record.owner.clone(),
            name: of_record.name.clone(),
            public: of_record.public.is_some(),
        });
        brokered::brokered_env(token, repo.as_ref(), &self.branch_area().public_marketplaces())
    }
    fn land_on_record(
        &self,
        entry: &LiveWorkspace,
        project: ProjectView,
        lines: Option<Vec<String>>,
    ) -> LocalBoxFuture<'static, AreaResult<()>> {
        let on = OnRecordEntry {
            record: entry.record.clone(),
            opens_thread: entry.opens_thread.clone(),
            of_record_held: entry.of_record_held,
            machine: entry.machine.clone(),
        };
        let area = self.branch_area();
        async move { Ok(area.land_on_record(&on, &project, lines.as_deref()).await?) }.boxed_local()
    }
    fn clone_lines(&self, o: Value) -> Vec<String> {
        let text = |key: &str| o.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
        let source = project_source(&text("source")).unwrap_or_else(|e| panic!("{}", e.message));
        let brokered = o.get("brokered").and_then(|b| {
            Some(BrokeredClone {
                of_record: serde_json::from_value(b.get("ofRecord")?.clone()).ok()?,
                origin: b.get("origin").and_then(Value::as_str).unwrap_or_default().to_string(),
                held: b.get("held").and_then(Value::as_bool).unwrap_or(false),
            })
        });
        let ask = CloneAsk {
            source,
            remote: text("remote"),
            checkout: text("checkout"),
            computer: text("computer"),
            branch: o.get("branch").and_then(Value::as_str).map(str::to_string),
            setup_git: o.get("setupGit").and_then(Value::as_bool),
            brokered,
        };
        project_landing::clone_lines(&ask)
    }
    async fn daemon_token_of(&self, machine: &Arc<dyn Machine>) -> AreaResult<Option<String>> {
        let read = self.transcripts.get().daemon_token_of(machine.clone(), Some(DAEMON_TOKEN_PATH.to_string()));
        Ok(read.await?)
    }
    fn forget_daemon_token(&self, machine_id: &str) {
        self.transcripts.get().daemon_tokens.borrow_mut().shift_remove(machine_id);
    }
    fn has_deploy(&self) -> bool {
        self.opts.golden_recipe.as_ref().is_some_and(|recipe| recipe.deploy_daemon.is_some())
    }
    async fn deploy_daemon(&self, machine: &Arc<dyn Machine>, token: Option<String>) -> AreaResult<()> {
        let deploy = self.opts.golden_recipe.as_ref().and_then(|recipe| recipe.deploy_daemon.clone());
        match deploy {
            Some(deploy) => {
                deploy(machine.clone(), token).await?;
                Ok(())
            }
            None => Err(HostError::new("opts.goldenRecipe.deployDaemon is not a function").into()),
        }
    }
    async fn local_daemon_road(&self) -> Option<AreaResult<DaemonReachView>> {
        let road = self.opts.local.as_ref()?.daemon_road.clone()?;
        Some(road().await.map_err(AreaError::from))
    }
    fn machine_exec_opts(&self) -> Option<MachineExecOptions> {
        self.opts.machine_exec.clone()
    }
    fn copy_import(
        &self,
        entry: &LiveWorkspace,
        o: ProjectImportOptions,
        report: ImportReport,
    ) -> LocalBoxFuture<'static, AreaResult<ImportLanded>> {
        let (held, projects) = (self.live_entry_of(entry), self.projects.get());
        async move {
            let held = held.ok_or_else(|| HostError::new("the workspace is not live, so nothing was imported"))?;
            Ok(projects.copy_import(&held, o, report).await?)
        }
        .boxed_local()
    }
    fn hello_version(&self, entry: &LiveWorkspace) -> LocalBoxFuture<'static, Option<u32>> {
        let (held, reach) = (self.live_entry_of(entry), self.reach.get());
        async move {
            let version = reach.hello_version(&held?).await?;
            u32::try_from(version as i64).ok()
        }
        .boxed_local()
    }
    fn has_local(&self) -> bool {
        self.opts.local.is_some()
    }
    fn local_backend(&self) -> Option<Arc<dyn MachineBackend>> {
        Some(self.opts.local.as_ref()?.backend.clone())
    }
    fn local_home(&self, agent: &str) -> String {
        (self.opts.local.as_ref().expect("a local wiring").home)(agent)
    }
    fn local_home_dir(&self) -> String {
        self.opts.local.as_ref().expect("a local wiring").home_dir.clone()
    }
    fn local_env(&self) -> IndexMap<String, String> {
        (self.opts.local.as_ref().expect("a local wiring").env)()
    }
    fn local_roots_path(&self) -> Option<String> {
        Some(self.opts.local.as_ref()?.roots_path.clone())
    }
    fn has_local_daemon_road(&self) -> bool {
        self.opts.local.as_ref().is_some_and(|local| local.daemon_road.is_some())
    }
    fn has_local_restart(&self) -> bool {
        self.opts.local.as_ref().is_some_and(|local| local.restart_daemon.is_some())
    }
    async fn here_daemon_version(&self) -> Option<AreaResult<Option<u32>>> {
        let here = self.opts.local.as_ref()?.here_daemon.clone()?;
        Some(here.version().await.map(Some).map_err(AreaError::from))
    }
    fn local_scratch(&self) -> String {
        self.opts.local.as_ref().expect("a local wiring").folder.clone()
    }
    fn agents_wsp_mcp(&self) -> Option<McpServerSpec> {
        self.opts.agents.as_ref()?.wsp_mcp.clone()
    }
    fn agents_here_url(&self) -> Option<String> {
        self.opts.agents.as_ref()?.here_url.clone()
    }
    async fn door_login_user(&self, place: &str) -> AreaResult<String> {
        let door = self.door_or("folderComputer")?;
        let Some(computer) = door.folder_computer(place) else {
            return Err(no_door("machine").into());
        };
        Ok(computer.machine.login_of().await?.user)
    }
    async fn end_thread(&self, place: &str, thread_id: &str, o: EndThread) -> AreaResult<Option<String>> {
        let Some(ends) = self.thread_ends.get() else { return Err(HostError::new(no_kind_line("place")).into()) };
        Ok(ends.end(place, thread_id, o).await?)
    }
    async fn end_owed(&self, place: &str, thread_id: &str) -> AreaResult<Option<OwedEnd>> {
        let Some(ends) = self.thread_ends.get() else { return Err(HostError::new(no_kind_line("place")).into()) };
        Ok(ends.owed(place, thread_id).await?)
    }
    fn sleep(&self, ms: f64) -> LocalBoxFuture<'static, ()> {
        let (done, wait) = futures::channel::oneshot::channel::<()>();
        let _ = self.clock.schedule(
            Box::new(move || {
                let _ = done.send(());
            }),
            count_ms(ms),
        );
        async move {
            let _ = wait.await;
        }
        .boxed_local()
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
}

impl BackendFor for Ctx {
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.kinds.get().backend_for(record)
    }
}

#[async_trait(?Send)]
impl ThreadEndsRuntime for Ctx {
    async fn store_get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.store.get(collection, id).await
    }
    async fn store_put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.store.put(collection, id, value).await
    }
    async fn store_delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.store.delete(collection, id).await
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    fn running_on(&self, thread_id: &str) -> bool {
        self.threads.get().running_on(thread_id).is_some()
    }
    fn launching_on(&self, thread_id: &str) -> bool {
        self.threads.get().launching_on(thread_id).is_some()
    }
    fn stop_holds(&self, thread_id: &str) -> bool {
        self.sessions_area.get().stop_holds(thread_id)
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
}

#[async_trait(?Send)]
impl ThreadEndsDoor for Ctx {
    async fn exec(&self, place_id: &str, script: &str, timeout_ms: u64) -> Result<ExecResult, HostError> {
        let door = self.door_or("exec")?;
        door.exec(place_id, script, PlaceExecOpts { timeout_ms: Some(timeout_ms), stdin: None }).await
    }
    fn link(&self, place_id: &str) -> bool {
        self.wired_door().is_some_and(|door| door.link(place_id).is_some())
    }
    fn name_of(&self, place_id: &str) -> String {
        self.wired_door().map_or_else(|| place_id.to_string(), |door| door.name_of(place_id))
    }
}

#[async_trait(?Send)]
impl RulesCtx for Ctx {
    fn no_machines_line(&self) -> Option<String> {
        self.opts.no_machines_line.clone()
    }
    async fn store_put_project(&self, project: &ProjectView) -> Result<(), HostError> {
        let value = serde_json::to_value(project).map_err(|e| HostError::new(e.to_string()))?;
        self.store.put(wsp_core::types::internal::PROJECTS, &project.id, value).await
    }
    fn door_name_of(&self, place_id: &str) -> Option<String> {
        Some(self.wired_door()?.name_of(place_id))
    }
    fn landing(&self) -> Rc<RefCell<IndexMap<String, i64>>> {
        self.landing.clone()
    }
    fn live(&self) -> Vec<AccountEntry> {
        self.live.borrow().values().cloned().collect()
    }
    fn live_get(&self, id: &str) -> Option<AccountEntry> {
        self.live.borrow().get(id).cloned()
    }
    fn projects_held(&self) -> Rc<RefCell<IndexMap<String, ProjectView>>> {
        self.projects_held.clone()
    }
    fn sessions_size(&self) -> usize {
        self.sessions.borrow().len()
    }
    fn thread_records_size(&self) -> usize {
        self.thread_records.borrow().len()
    }
    fn thread_record(&self, thread_id: &str) -> Option<ThreadRecord> {
        self.thread_records.borrow().get(thread_id).cloned()
    }
    fn rows_on(&self, thread_id: &str) -> Vec<TreeFacts> {
        let rows = self.agents.get().rows_on(thread_id);
        rows.into_iter()
            .map(|view| TreeFacts {
                workspace_id: Some(view.workspace_id),
                parent_thread_id: view.parent_thread_id,
                root_thread_id: view.root_thread_id,
            })
            .collect()
    }
    async fn target_workspace(&self) -> Result<Option<String>, HostError> {
        let held = self.preferences.get().get().await?;
        Ok(held.target.map(|target| target.workspace))
    }
    async fn set_preferences(&self, patch: Value) -> Result<(), HostError> {
        let patch = serde_json::from_value(patch).map_err(|e| HostError::new(e.to_string()))?;
        self.preferences.get().set(patch).await?;
        Ok(())
    }
    fn capability(&self, record: &WorkspaceRecord, can: &str) -> bool {
        let Ok(backend) = self.kinds.get().backend_for(record) else { return false };
        let capabilities = serde_json::to_value(backend.capabilities()).unwrap_or(Value::Null);
        capabilities.get(can) == Some(&Value::Bool(true))
    }
    fn pause_mode_declared(&self, record: &WorkspaceRecord) -> bool {
        self.kinds.get().backend_for(record).is_ok_and(|backend| backend.capabilities().pause_mode.is_some())
    }
    fn forks_no_machines(&self, record: &WorkspaceRecord) -> bool {
        self.kinds.get().backend_for(record).is_ok_and(|backend| forks_no_machines(&backend.capabilities()))
    }
    fn relayed(&self, kind: WorkspaceKind, machine_id: Option<&str>) -> bool {
        self.kinds.get().module_of(kind).is_ok_and(|module| module.relayed(machine_id))
    }
    fn turn_reach(&self, entry: &AccountEntry) -> Option<Map<String, Value>> {
        let kind = entry.borrow().record.borrow().kind;
        let reach = self.kinds.get().module_of(kind).ok()?.turn_reach(&entry.borrow())?;
        let mut object = Map::new();
        if let Some(url) = reach {
            object.insert("url".to_string(), json!(url));
        }
        Some(object)
    }
    fn wsp_mcp(&self, entry: &AccountEntry) -> Option<Value> {
        let kind = entry.borrow().record.borrow().kind;
        let spec = self.kinds.get().module_of(kind).ok()?.wsp_mcp(&entry.borrow())?;
        serde_json::to_value(spec).ok()
    }
    fn same_repository(&self, a: &ProjectView, b: &ProjectView, caller: Option<&Caller>) -> bool {
        self.channels.get().same_repository(a, b, caller)
    }
    fn project_held(&self, id: &str) -> Result<ProjectView, HostError> {
        self.channels.get().project_held(id)
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        self.kinds.get().kind_of(computer)
    }
    fn place_name(&self, id: &str) -> String {
        self.place_name_of(id)
    }
    fn agents_held(&self, record: &WorkspaceRecord) -> WorkspaceAgents {
        self.channels.get().agents_held(record)
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().place_id_of(record)
    }
}

#[async_trait(?Send)]
impl RecordsCtx for Ctx {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn wired(&self) -> String {
        self.places.wired()
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
    fn module_folder(&self, r: &WorkspaceRecord) -> Option<String> {
        self.kinds.get().module_of(r.kind).ok()?.folder(r).ok()?
    }
    fn home_of(&self, r: &WorkspaceRecord) -> Option<String> {
        self.channels.get().home_of(r)
    }
    fn project_ref(&self, project: &str) -> Result<ProjectView, HostError> {
        self.channels.get().project_held(project)
    }
    fn daemon_note(&self, workspace_id: &str) -> Option<String> {
        self.channels.get().daemon_notes.borrow().get(workspace_id).cloned()
    }
    fn agents_of(&self, r: &WorkspaceRecord) -> Option<Value> {
        serde_json::to_value(self.rules.get().agents_of(r)?).ok()
    }
    fn provider_of(&self, r: &WorkspaceRecord) -> Option<String> {
        self.channels.get().provider_of(r)
    }
    fn backend_for(&self, r: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.kinds.get().backend_for(r)
    }
    fn live_records(&self) -> Vec<Rc<RefCell<WorkspaceRecord>>> {
        self.live.borrow().values().map(|entry| entry.borrow().record.clone()).collect()
    }
    fn bump_generation(&self, workspace_id: &str) -> bool {
        let live = self.live.borrow();
        let Some(entry) = live.get(workspace_id) else { return false };
        entry.borrow_mut().generation += 1;
        true
    }
}

#[async_trait(?Send)]
impl HeldCtx for Ctx {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn wired(&self) -> String {
        self.places.wired()
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    async fn drop_copy_recipe(&self, place: &str, name: &str, version: u32) -> Result<(), HostError> {
        self.records.get().drop_copy_recipe(place, name, version).await
    }
}

#[async_trait(?Send)]
impl BranchCtx for Ctx {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
    fn broker_ready(&self) -> bool {
        self.opts.broker.as_ref().is_some_and(|broker| broker.ready())
    }
    async fn exec_here(&self, cmd: &str, timeout_ms: u64) -> Result<WireExecResult, HostError> {
        let backend = self.local_wiring()?.backend.clone();
        let machine = backend.get(LOCAL_MACHINE_ID).await.map_err(|e| AreaError::from(e).into_host())?;
        let opts = ExecOptions { timeout_ms: Some(timeout_ms), ..ExecOptions::default() };
        machine.exec(cmd, opts).await.map_err(|e| AreaError::from(e).into_host())
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.records.get().persist(record).await
    }
    fn live_records(&self) -> Vec<Rc<RefCell<WorkspaceRecord>>> {
        self.live.borrow().values().map(|entry| entry.borrow().record.clone()).collect()
    }
    async fn mint(&self, name: &str, scope: ThreadScope, now: i64, more: MintMore) -> Result<PairedDevice, HostError> {
        self.device_door.mint(name, scope, now, more).await
    }
    async fn revoke(&self, id: &str) -> Result<bool, HostError> {
        self.device_door.revoke(id).await
    }
    fn turn_road_of(&self, record: &WorkspaceRecord) -> Result<MintMore, HostError> {
        let module = self.kinds.get().module_of(record.kind)?;
        Ok(MintMore { road: Some(module.turn_road()), aside: false })
    }
    fn place_name(&self, place: &str) -> String {
        self.place_name_of(place)
    }
    fn wired(&self) -> String {
        self.places.wired()
    }
    fn home_dir(&self) -> String {
        match self.opts.local.as_ref() {
            Some(local) => local.home_dir.clone(),
            None => std::env::var("HOME").unwrap_or_default(),
        }
    }
    async fn read_file(&self, path: &str) -> Result<String, HostError> {
        tokio::fs::read_to_string(path).await.map_err(|e| read_failure(&e, path))
    }
    fn marketplace_files(&self) -> Vec<MarketplaceFile> {
        wsp_catalog::catalog::catalog_agents()
            .into_iter()
            .filter_map(|agent| agent.plugins)
            .map(|road| MarketplaceFile {
                path: road.marketplaces().strip_prefix("~/").unwrap_or(road.marketplaces()).to_string(),
                github_repos: Rc::new(move |text: &str| road.github_repos(text)),
            })
            .collect()
    }
    fn brokered_git_env(&self, token: &str, of_record: Option<&BranchOfRecord>) -> IndexMap<String, String> {
        let repo = of_record.map(|b| GitEnvRepo { owner: b.owner.clone(), name: b.name.clone(), public: b.public.is_some() });
        project_landing::brokered_git_env(token, repo.as_ref(), &[])
    }
    fn on_record_lines(&self, checkout: &str, of_record: &BranchOfRecord, origin: &str, held: bool) -> Vec<String> {
        project_landing::on_record_lines(checkout, of_record, origin, held)
    }
    async fn env_exec(
        &self,
        machine: Arc<dyn Machine>,
        script: &str,
        env: IndexMap<String, String>,
        timeout_ms: u64,
    ) -> Result<WireExecResult, HostError> {
        wsp_engine::run_env::env_exec(&*machine, script, &env, Some(timeout_ms)).await.map_err(|e| AreaError::from(e).into_host())
    }
}

/// `BranchCtx` over a weak handle: the branch-of-record area lives outside the context, so it must not keep one alive.
struct BranchOver(Weak<Ctx>);

impl BranchOver {
    fn ctx(&self) -> Rc<Ctx> {
        self.0.upgrade().expect("the context is alive while the branch roads run")
    }
}

#[async_trait(?Send)]
impl BranchCtx for BranchOver {
    fn clock(&self) -> Rc<dyn Clock> {
        BranchCtx::clock(&*self.ctx())
    }
    fn warn(&self, line: &str) {
        BranchCtx::warn(&*self.ctx(), line)
    }
    fn broker_ready(&self) -> bool {
        BranchCtx::broker_ready(&*self.ctx())
    }
    async fn exec_here(&self, cmd: &str, timeout_ms: u64) -> Result<WireExecResult, HostError> {
        let ctx = self.ctx();
        BranchCtx::exec_here(&*ctx, cmd, timeout_ms).await
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        let ctx = self.ctx();
        BranchCtx::persist(&*ctx, record).await
    }
    fn live_records(&self) -> Vec<Rc<RefCell<WorkspaceRecord>>> {
        BranchCtx::live_records(&*self.ctx())
    }
    async fn mint(&self, name: &str, scope: ThreadScope, now: i64, more: MintMore) -> Result<PairedDevice, HostError> {
        let ctx = self.ctx();
        BranchCtx::mint(&*ctx, name, scope, now, more).await
    }
    async fn revoke(&self, id: &str) -> Result<bool, HostError> {
        let ctx = self.ctx();
        BranchCtx::revoke(&*ctx, id).await
    }
    fn turn_road_of(&self, record: &WorkspaceRecord) -> Result<MintMore, HostError> {
        BranchCtx::turn_road_of(&*self.ctx(), record)
    }
    fn place_name(&self, place: &str) -> String {
        BranchCtx::place_name(&*self.ctx(), place)
    }
    fn wired(&self) -> String {
        BranchCtx::wired(&*self.ctx())
    }
    fn home_dir(&self) -> String {
        BranchCtx::home_dir(&*self.ctx())
    }
    async fn read_file(&self, path: &str) -> Result<String, HostError> {
        let ctx = self.ctx();
        BranchCtx::read_file(&*ctx, path).await
    }
    fn marketplace_files(&self) -> Vec<MarketplaceFile> {
        BranchCtx::marketplace_files(&*self.ctx())
    }
    fn brokered_git_env(&self, token: &str, of_record: Option<&BranchOfRecord>) -> IndexMap<String, String> {
        BranchCtx::brokered_git_env(&*self.ctx(), token, of_record)
    }
    fn on_record_lines(&self, checkout: &str, of_record: &BranchOfRecord, origin: &str, held: bool) -> Vec<String> {
        BranchCtx::on_record_lines(&*self.ctx(), checkout, of_record, origin, held)
    }
    async fn env_exec(
        &self,
        machine: Arc<dyn Machine>,
        script: &str,
        env: IndexMap<String, String>,
        timeout_ms: u64,
    ) -> Result<WireExecResult, HostError> {
        let ctx = self.ctx();
        BranchCtx::env_exec(&*ctx, machine, script, env, timeout_ms).await
    }
}

#[async_trait(?Send)]
impl ImageDoor for Ctx {
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        self.wired_door()?.backend_of(place_id)
    }
    async fn forking_backend(&self, place_id: &str) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.door_or("forkingBackend")?.forking_backend(place_id).await
    }
    fn name_of(&self, place_id: &str) -> Option<String> {
        Some(self.wired_door()?.name_of(place_id))
    }
    async fn place_for(&self, word: &str) -> Result<Option<String>, HostError> {
        Ok(self.door_or("placeFor")?.place_for(word).await?.place_id)
    }
    async fn default_place(&self) -> Result<Option<String>, HostError> {
        Ok(self.door_or("defaultPlace")?.default_place().await?.place_id)
    }
    async fn mark_default_if_none(&self, place_id: &str) -> Result<(), HostError> {
        self.door_or("markDefaultIfNone")?.mark_default_if_none(place_id).await
    }
    async fn list(&self, now: i64) -> Result<Vec<wsp_wire::views::place_more::PlaceView>, HostError> {
        self.door_or("list")?.list(now.max(0) as u64).await
    }
}

#[async_trait(?Send)]
impl AgentsPlaces for Ctx {
    async fn list(&self, now: i64) -> Result<Vec<wsp_wire::views::place_more::PlaceView>, HostError> {
        self.door_or("list")?.list(now.max(0) as u64).await
    }
    async fn report_of(&self, place_id: &str) -> Result<Option<PlaceReport>, HostError> {
        self.door_or("reportOf")?.report_of(place_id).await
    }
    async fn logins_again(&self, place_id: &str) -> Result<(), HostError> {
        self.door_or("loginsAgain")?.logins_again(place_id).await
    }
    fn sign_ins_at(&self, place_id: &str, agents: &[String]) -> Option<IndexMap<String, AgentSignInState>> {
        self.wired_door()?.sign_ins_at(place_id, Some(agents))
    }
    async fn exec(&self, place_id: &str, cmd: &str, timeout_ms: Option<u64>, stdin: Option<Vec<u8>>) -> Result<WireExecResult, HostError> {
        self.door_or("exec")?.exec(place_id, cmd, PlaceExecOpts { timeout_ms, stdin }).await
    }
    async fn gh_there(&self, place_id: &str, on_step: GhStep) -> Result<Vec<wsp_wire::views::place::PlaceProvisionRow>, HostError> {
        let stage: wsp_engine::provision::ProvisionStage = Arc::new(move |detail, at, row| {
            on_step(detail.to_string(), at.map(|stage| stage.label.clone()), row.cloned());
        });
        self.door_or("ghThere")?.gh_there(place_id, stage).await
    }
    async fn key_landed(&self, agent: &str) -> Result<(), HostError> {
        self.door_or("keyLanded")?.key_landed(agent).await
    }
    async fn login_landed(&self, place_id: &str, agent: &str) -> Result<(), HostError> {
        self.door_or("loginLanded")?.login_landed(place_id, agent).await
    }
}
