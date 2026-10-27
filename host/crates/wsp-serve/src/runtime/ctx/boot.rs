// SPDX-License-Identifier: AGPL-3.0-only
//! What the boot area asks of the runtime (`BootRuntime`, boot.ts:29-1071) and the road a create reads a project's
//! landing through: the context's own state, a forward to the area that holds each call, and the few lines no area has.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt as _;
use indexmap::{IndexMap, IndexSet};
use wsp_core::clock::Cancel;
use wsp_core::project_landing::{project_landing, LandingDeps, ProjectLanding};
use wsp_core::types::internal::{turn_written, KindModule, Reopened, SessionEntry, ThreadRecord};
use wsp_core::types::pending::{Clock, Store};
use wsp_core::types::wiring::{GoneOutcome, PlaceBackends, WorkspaceRecord};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::machines::boot::{BootRuntime, CardAsk, ComputerRow, HeldEnd, LimitRow, ScopedDevice};
use wsp_machines::machines::create::LandingRoad;
use wsp_machines::machines::machines::{Entry, Ids, Row};
use wsp_machines::machines::sub_life::BackendFor;
use wsp_machines::usage::accounts::{account_on_computer, ComputerName};
use wsp_machines::usage::Vaulted;
use wsp_threads::threads::threads::tree::{CallsOf, TellAs};
use wsp_threads::threads::threads::turn_end::TurnChangesAsk;
use wsp_threads::threads::threads::ThreadsArea;
use wsp_wire::types::usage::LIMIT_RESUME_PROMPT;
use wsp_wire::views::golden_image::{golden_head, MachineKind};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::SessionEvent;
use wsp_wire::views::sub_row::SubRow;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::machine_link::MachineShape;
use wsp_words::exit::HostError;
use wsp_words::words::computer::{no_kind_line, AcrossAct};

use super::Ctx;
use crate::runtime::join::spawn_detached;

#[async_trait::async_trait(?Send)]
impl BootRuntime for Ctx {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn emit(&self, event: EventUnion) -> Result<(), HostError> {
        self.bus.emit(event).map(|_| ())
    }
    fn pass(&self, event: EventUnion) -> Result<(), HostError> {
        self.bus.pass(event)
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
    fn owner(&self) -> String {
        self.state.with(|s| s.owner.clone())
    }
    fn set_owner(&self, owner: &str) {
        self.state.with(|s| s.owner = owner.to_string());
    }
    fn reads_since(&self) -> f64 {
        self.state.with(|s| s.reads_since)
    }
    fn set_reads_since(&self, at: f64) {
        self.state.with(|s| s.reads_since = at);
    }
    fn closed(&self) -> bool {
        self.state.with(|s| s.closed)
    }
    fn closing(&self) -> bool {
        self.state.with(|s| s.closing)
    }
    fn hostname(&self) -> String {
        self.hostname.clone()
    }
    fn local_wired(&self) -> bool {
        self.opts.local.is_some()
    }
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>> {
        self.live.clone()
    }
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>> {
        self.sessions.clone()
    }
    fn thread_records(&self) -> Rc<RefCell<IndexMap<String, ThreadRecord>>> {
        self.thread_records.clone()
    }
    fn builders(&self) -> wsp_machines::images::Builders {
        self.builders.clone()
    }
    fn projects_held(&self) -> Rc<RefCell<IndexMap<String, ProjectView>>> {
        self.projects_held.clone()
    }
    fn boot_work(&self) -> Rc<RefCell<IndexMap<String, super::Joined>>> {
        self.boot_work.clone()
    }
    fn rereading(&self) -> Rc<RefCell<IndexSet<String>>> {
        self.machines.get().rereading_held()
    }
    fn viewed_marks(&self) -> Rc<RefCell<IndexMap<String, IndexMap<String, String>>>> {
        self.transcripts.get().viewed_marks.clone()
    }
    fn has_transcript_index(&self, workspace_id: &str) -> bool {
        self.transcript_state.with(|t| t.index.contains_key(workspace_id))
    }
    fn index_pos(&self, workspace_id: &str) -> u64 {
        self.transcript_state.with(|t| t.index.get(workspace_id).map_or(0, |i| i.pos))
    }
    fn index_children(&self, workspace_id: &str, thread_id: &str) -> Vec<serde_json::Value> {
        self.transcript_state.with(|t| {
            let Some(children) = t.index.get(workspace_id).and_then(|i| i.children.get(thread_id)) else {
                return Vec::new();
            };
            children
                .values()
                .filter_map(|child| match serde_json::to_value(child) {
                    Ok(serde_json::Value::Object(held)) => {
                        Some(serde_json::Value::Object(held.into_iter().filter(|(key, _)| key != "turnId" && key != "startRow").collect()))
                    }
                    _ => None,
                })
                .collect()
        })
    }
    fn observed(&self, machine: Arc<dyn Machine>) -> Arc<dyn Machine> {
        self.machines.get().observed(machine)
    }
    fn attach(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>) -> Result<Entry, HostError> {
        self.machines.get().attach(record, machine)
    }
    async fn mark_gone(&self, entry: &Entry, machine_id: &str, reason: &str) -> Result<GoneOutcome, HostError> {
        let machines = self.machines.get();
        machines.mark_gone(entry, machine_id, reason).await
    }
    fn idle_touch(&self, workspace_id: &str) {
        self.idle.get().touch(workspace_id);
    }
    fn sync_daemon(&self, entry: &Entry) {
        let synced = self.daemon.get().sync_daemon(entry);
        spawn_detached(async move {
            let _ = synced.await;
        });
    }
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.kinds.get().backend_for(record)
    }
    fn module_of(&self, kind: &str) -> Result<Rc<dyn KindModule>, HostError> {
        let known: WorkspaceKind =
            serde_json::from_value(serde_json::Value::String(kind.to_string())).map_err(|_| HostError::new(no_kind_line(kind)))?;
        self.kinds.get().module_of(known)
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        let records = self.records.get();
        records.persist(record).await
    }
    async fn shape_of(&self, machine: &Arc<dyn Machine>) -> Option<MachineShape> {
        let records = self.records.get();
        records.shape_of(machine.clone()).await
    }
    fn size_built(&self, shape: Option<&MachineShape>, asked: WorkspaceSize) -> WorkspaceSize {
        self.records.get().size_built(shape, &asked)
    }
    fn arm_grace(&self, builder_id: &str, sealed_at: &str) {
        self.builders_area.get().arm_grace(builder_id, sealed_at);
    }
    fn door_backend_of(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.place_door.get()?.backend_of(place)
    }
    async fn image_kind(&self, golden: &str) -> Result<Option<MachineKind>, HostError> {
        let records = self.records.get();
        Ok(records.image_of(golden).await?.version.and_then(|v| v.kind))
    }
    fn attach_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Result<(), HostError> {
        self.sub_queue.get().attach_queued(record, None)
    }
    async fn migrate_copies(&self) -> Result<(), HostError> {
        Ok(())
    }
    async fn place_door_load(&self) -> Result<(), HostError> {
        match self.place_door.get() {
            Some(door) => door.load().await,
            None => Ok(()),
        }
    }
    async fn git_top_of(&self, path: &str) -> Result<String, HostError> {
        let kinds = self.kinds.get();
        match kinds.git_top_of(path).await.map_err(|e| e.into_host())? {
            Some(top) => Ok(top),
            None => Err(HostError::new(format!("{path} is not inside a git repository"))),
        }
    }
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> {
        let rules = self.rules.get();
        rules.remember_project(project).await
    }
    async fn setups_load(&self) -> Result<(), HostError> {
        self.setups.load().await
    }
    async fn move_transcript(&self, _record: serde_json::Value) -> Result<(), HostError> {
        Ok(())
    }
    fn move_blobs(&self) {}
    async fn load_index(&self, workspace_id: &str) -> Result<(), HostError> {
        self.transcripts.get().load_index(workspace_id);
        Ok(())
    }
    fn wake_at(&self, thread_id: &str, until: f64) {
        self.core.get().wake_at(thread_id, until);
    }
    fn resume_on_reset(&self, thread_id: &str, at: f64) {
        self.core.get().resume_on_reset(thread_id, at);
    }
    fn settle_cut(&self, row: &Row, reason: &str, line: &dyn Fn(f64) -> String, stopped: Option<bool>, unreached: Option<bool>) {
        let kept = self.sessions.borrow().iter().find(|(_, held)| Rc::ptr_eq(held, row)).map(|(key, _)| key.clone());
        let id = kept.unwrap_or_else(|| row.borrow().view.id.clone());
        self.threads.get().settle_cut(&id, reason, &|at| line(at as f64), stopped.unwrap_or(false), unreached.unwrap_or(false));
    }
    async fn persist_sessions(&self, workspace_id: &str) {
        self.transcripts.get().persist_sessions(workspace_id).await;
    }
    async fn turn_changes_written(&self, workspace_id: &str, thread_id: &str, turn_id: &str) -> Result<bool, HostError> {
        let events = self.transcripts.get().open_thread(workspace_id, thread_id)?;
        Ok(turn_written(&events, turn_id).changes)
    }
    async fn read_turn_changes(&self, entry: &Entry, ask: CardAsk) -> Result<bool, HostError> {
        let threads = self.threads.get();
        let ask = TurnChangesAsk {
            session_id: ask.session_id,
            turn_id: ask.turn_id,
            thread_id: ask.thread_id,
            cwd: ask.cwd,
            from: ask.from,
            started_at: ask.started_at as i64,
            wrote: None,
        };
        Ok(threads.read_turn_changes(entry, ask).await)
    }
    async fn reattach(&self, row: &Row, again: bool) -> Reopened {
        let turns = self.turns.get();
        turns.reattach(row.clone(), again).await
    }
    async fn devices(&self) -> Result<Vec<ScopedDevice>, HostError> {
        let listed = self.device_door.list().await?;
        Ok(listed.into_iter().map(|d| ScopedDevice { id: d.id, thread_id: d.scope.map(|s| s.thread_id) }).collect())
    }
    async fn revoke_device(&self, device_id: &str) -> Result<(), HostError> {
        self.device_door.revoke(device_id).await.map(|_| ())
    }
    async fn revoke_asides(&self) -> Result<(), HostError> {
        self.device_door.revoke_asides().await
    }
    async fn shell_held_devices(&self) -> Result<IndexSet<String>, HostError> {
        let channels = self.channels.get();
        Ok(channels.shell_held_devices().await?.into_iter().collect())
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        self.threads.get().thread_runs(thread_id)
    }
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError> {
        let folders = self.folders.get();
        folders.worktree_mounted(entry).await
    }
    async fn sweep_runs(&self, entry: &Entry) -> Result<(), HostError> {
        let turns = self.turns.get();
        turns.sweep_runs(entry).await;
        Ok(())
    }
    async fn resume_queue(&self) -> Result<(), HostError> {
        let queue = self.sub_queue.get();
        queue.resume_queue().await.map_err(|e| e.into_host())
    }
    fn notify_end(&self, end: HeldEnd) {
        let held = Rc::new(RefCell::new(SessionEntry {
            view: end.view,
            turn_id: end.turn_id,
            replaces: None,
            notify: None,
            notify_by: None,
            notify_road: None,
            turn_token: None,
            scope_device_id: None,
            handle: None,
            end: None,
            turn_live: None,
            run: None,
            from: None,
            asked: None,
            snapshot: None,
            pid: None,
            launch: None,
            calls: None,
            unanswered: false,
            queued: false,
        }));
        let named = TellAs { by: end.notify_by, road: end.notify_road };
        self.threads.get().notify_end(&held, &end.notify, &named, &end.result, false);
    }
    fn record(&self, event: EventUnion) {
        let Ok(event) = serde_json::to_value(&event).and_then(serde_json::from_value::<SessionEvent>) else {
            return;
        };
        self.transcripts.get().record(event);
    }
    fn arm_subs(&self) {
        self.sub_life.get().arm_subs();
    }
    async fn deliver_owed(&self) -> Result<(), HostError> {
        let threads = self.threads.get();
        threads.deliver_owed().await
    }
    async fn move_old_copies(&self) -> Result<(), HostError> {
        Ok(())
    }
    fn arm_sweep(&self) {
        self.folders.get().arm_sweep();
    }
    async fn slates_ready(&self) -> Result<(), HostError> {
        let slates = self.slates.get();
        slates.ready().await
    }
    fn drives_thread(&self, thread_id: Option<&str>, caller: Option<&Caller>) -> bool {
        self.threads.get().drives_thread(thread_id, caller)
    }
    fn acts_on(&self, record: &WorkspaceRecord, caller: Option<&Caller>) -> bool {
        self.rules.get().acts_on(&wsp_account::account::rules::like_of(record), caller)
    }
    fn talks_to_its_tree(&self, caller: Option<&Caller>) -> Result<bool, HostError> {
        self.rules.get().talks_to_its_tree(caller)
    }
    fn refuse_relayed(
        &self,
        record: &WorkspaceRecord,
        origin: Option<&Caller>,
        act: Option<&str>,
        thread: Option<&str>,
    ) -> Result<(), HostError> {
        let act = match act {
            Some("start") => AcrossAct::Start,
            Some("exec") => AcrossAct::Exec,
            Some("commit") => AcrossAct::Commit,
            Some("update") => AcrossAct::Update,
            Some("fix") => AcrossAct::Fix,
            Some("wake") => AcrossAct::Wake,
            _ => AcrossAct::Work,
        };
        let like = wsp_account::account::rules::like_of(record);
        self.rules.get().refuse_relayed(Some(&like), origin, act, thread)
    }
    async fn resolve_workspace(&self, reference: &str, origin: Option<&Caller>) -> Result<String, HostError> {
        use wsp_core::types::api::WorkspacesApi as _;
        let workspaces = self.workspaces.get();
        Ok(workspaces.resolve(reference, origin.cloned()).await?.id)
    }
    async fn place_rows(&self) -> Option<Vec<ComputerRow>> {
        let door = self.place_door.get()?;
        let rows = door.rows().await.ok()?;
        Some(rows.into_iter().map(|p| ComputerRow { id: p.id, name: p.name, kind: p.kind }).collect())
    }
    async fn default_image_head(&self) -> Result<Option<String>, HostError> {
        let (builders, records) = (self.builders_area.get(), self.records.get());
        let place = builders.image_place("default").await?;
        let copy = records.copy_of(&place, "default").await?;
        Ok(golden_head(copy.as_ref()).map(|version| version.snapshot_id.clone()))
    }
    async fn place_for(&self, place: &str) -> Result<Option<String>, HostError> {
        let Some(door) = self.place_door.get() else {
            return Err(HostError::new(no_kind_line("place")));
        };
        Ok(door.place_for(place).await?.place_id)
    }
    fn restarts(&self) -> IndexMap<String, String> {
        self.threads.get().restarts()
    }
    fn stopped_behind(&self, row: &Row) -> Option<serde_json::Value> {
        let calls = {
            let held = row.borrow();
            CallsOf { view: held.view.clone(), calls: held.calls.clone() }
        };
        let waiting = self.threads.get().stopped_behind(&calls)?;
        serde_json::to_value(waiting).ok()
    }
    fn cap_wait(&self, turn_id: &str) -> Option<serde_json::Value> {
        let threads = self.threads.get();
        let held = threads.cap_held.borrow();
        serde_json::to_value(&held.get(turn_id)?.waiting.as_ref()?.wait).ok()
    }
    fn agent_pid_of(&self, run: &str) -> Option<u32> {
        wsp_core::local_exec::reap::agent_pid_of(run).and_then(|pid| u32::try_from(pid).ok())
    }
    fn setup_refusal(&self, workspace_id: &str, harness: &str) -> Option<String> {
        let entry = self.live.borrow().get(workspace_id).cloned()?;
        let place = wsp_threads::threads::agents::AgentsArea::setup_place(&entry)?;
        let agents = self.agents.get();
        let refused = agents.setup_refusals.borrow().get(&wsp_core::agent_setup::key_of(&place, harness)).cloned();
        refused
    }
    fn setup_place(&self, entry: &Entry) -> Option<String> {
        wsp_threads::threads::agents::AgentsArea::setup_place(entry)
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.threads.get().latest_on(thread_id)
    }
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView> {
        self.agents.get().rows_on(thread_id)
    }
    fn sub_row_of(&self, record: &WorkspaceRecord) -> Option<SubRow> {
        wsp_machines::machines::sub_life::reads::sub_row_of(record)
    }
    fn disk_kept_until(&self, record: &WorkspaceRecord) -> Option<f64> {
        wsp_machines::machines::sub_life::reads::disk_kept_until(self as &dyn BackendFor, record, None)
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().place_id_of(record)
    }
    async fn sub_read(&self, workspace_id: &str, since: f64) -> Result<(), HostError> {
        let life = self.sub_life.get();
        life.sub_read(workspace_id, since).await;
        Ok(())
    }
    fn title_rows(&self, views: Vec<SessionView>) -> Vec<SessionView> {
        wsp_threads::threads::agents::lists::title_rows(&views)
    }
    fn refresh_title(&self, view: SessionView) {
        let refreshed = self.agents.get().refresh_title(&view, false);
        spawn_detached(refreshed);
    }
    fn reap_kept(&self, thread_id: &str) {
        self.threads.get().reap_kept(thread_id, false);
    }
    fn cancel_resume(&self, thread_id: &str) {
        let cancel: Option<Cancel> = self.resume_timers.borrow().get(thread_id).cloned();
        if let Some(cancel) = cancel {
            cancel();
        }
    }
    fn lead_asks(&self, thread_id: &str) -> bool {
        self.threads.get().lead_asks.borrow().contains_key(thread_id)
    }
    async fn ledger_limits(&self) -> Vec<LimitRow> {
        let limits = wsp_account::usage_reset::UsageResetDeps::limits(self).await.unwrap_or_default();
        limits.into_iter().map(|l| LimitRow { key: l.key, windows: l.windows }).collect()
    }
    /// `accountOnComputer({..., limits, ...}).key` (boot.ts:1009-1011). The trait hands no limits, so the ledger is read
    /// here: its read answers from the store at once, and a read that does not stands for a ledger holding none.
    fn account_key_on(&self, entry: &Entry, harness: &str) -> String {
        let limits = wsp_account::usage_reset::UsageResetDeps::limits(self).now_or_never().and_then(Result::ok).unwrap_or_default();
        let held = entry.borrow();
        let record = held.record.borrow().clone();
        let stands = self.kinds.get().module_of(record.kind).map(|module| module.login_stands(&held, harness)).ok();
        let vault = self.kinds.get().vault_on(&held);
        let vaulted =
            self.threads
                .get()
                .vaulted_for(harness, stands, Some(&vault))
                .map(|word| if word == "token" { Vaulted::Token } else { Vaulted::Key });
        let computer = ComputerName { id: ThreadsArea::usage_computer_of(&record), name: self.channels.get().computer_of(entry) };
        let agent_name = wsp_threads::threads::agents::AgentsArea::agent_label(harness);
        account_on_computer(harness, &agent_name, &computer, &limits, vaulted).0
    }
    async fn resume_turn(&self, workspace_id: &str, thread_id: &str, after_limit: f64) -> Result<(), HostError> {
        use wsp_core::types::api::SessionsApi as _;
        let sessions = self.sessions_area.get();
        let opts = wsp_core::types::api::StartOptions {
            prompt: LIMIT_RESUME_PROMPT.to_string(),
            thread: Some(thread_id.to_string()),
            after_limit: Some(after_limit),
            ..Default::default()
        };
        sessions.start(workspace_id, opts, None).await.map(|_| ())
    }
}

/// The road of this host's own provider: the one a project with no joined computer lands on. A create over a box or
/// this Mac reads its own road, whose deps this trait's JSON cannot carry (project-landing.ts:52).
fn provider_road() -> Rc<dyn ProjectLanding> {
    project_landing("provider").expect("the provider landing road is built in")
}

/// The deps a provider road's reads ask nothing of.
fn provider_deps(clock: Rc<dyn Clock>) -> LandingDeps {
    LandingDeps {
        land: Rc::new(|_, _, _| async { Err(HostError::new("a project is landed through its create")) }.boxed_local()),
        scratch: Rc::new(|_| String::new()),
        computer: None,
        mac_state_home: String::new(),
        branch_here: Rc::new(|_| async { None }.boxed_local()),
        computer_name: String::new(),
        clone_env: None,
        now: Rc::new(move || clock.now()),
    }
}

impl LandingRoad for Ctx {
    fn kind(&self) -> String {
        provider_road().kind().to_string()
    }
    fn refusal(&self, project: &ProjectView, _deps: &serde_json::Value) -> Option<String> {
        provider_road().refusal(project, &provider_deps(self.clock.clone()))
    }
    fn memory_dir(&self, project: &ProjectView, memory_key: &str, _deps: &serde_json::Value) -> Result<String, HostError> {
        Ok(provider_road().places(project, memory_key, &provider_deps(self.clock.clone()))?.memory_dir)
    }
    fn workspace_binds(&self, project: &ProjectView) -> Vec<wsp_wire::views::place::MachineBind> {
        provider_road().workspace_binds(project)
    }
}
