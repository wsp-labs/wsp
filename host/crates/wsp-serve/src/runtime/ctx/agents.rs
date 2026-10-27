// SPDX-License-Identifier: AGPL-3.0-only
//! The context's agents, launch and threads readings (runtime.ts:463-465; agents.ts, launch.ts, threads.ts): each is a
//! read of the core, a call on the area that holds the rule, or the few lines no crate has (`exec_factory_for`).
//! The one `LaunchArea` is the slot's value, so its `warming` map is the one every road reads.
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::{FutureExt as _, StreamExt as _};
use indexmap::IndexMap;
use wsp_account::account::core_types::AccountError;
use wsp_adapters::port::{AgentLaunch, Env, ExecStreamFactory, HarnessAdapter, SessionRenameWrite};
use wsp_core::clock::Clock;
use wsp_core::context::Joined;
use wsp_core::ids::Ids;
use wsp_core::machine_exec::{KilledWhy, MachineExecOptions, TurnWaiting};
use wsp_core::types::events::EventBus;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::internal::{EndThread, KeptLaunch, ServerValuesRoad, Taken, ThreadStamps};
use wsp_core::types::wiring::{at_provider, LiveWorkspace, SessionHandle, WorkspaceRecord};
use wsp_machines::machines::boot::rows::Stamped;
use wsp_machines::machines::kinds::BrokeredLaunch;
use wsp_machines::places::types::{PlaceEvent, PlaceExecOpts};
use wsp_store::store::Store;
use wsp_threads::threads::agents::launch::{AdapterAsk, BuiltAdapter, DefaultsOn, LaunchServers};
use wsp_threads::threads::agents::{AgentsArea, AgentsCtx, SetupLaunch, SignsIn, TurnLaunch};
use wsp_threads::threads::launch::{LaunchCtx, ThreadLaunch};
use wsp_threads::threads::threads::{Entry, KeptHand, PlaceMove, Sessions, ThreadRecords, ThreadsCtx};
use wsp_wire::types::harness_picks::RanPicks;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::types::usage::AccountLimit;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::{Caller, ProjectView, ReachState};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_wire::wire::runtime::SessionInterruptResult;
use wsp_words::exit::HostError;
use wsp_words::words::computer::no_kind_line;

use super::Ctx;
use crate::runtime::join::spawn_detached;

/// `os.homedir()`: `$HOME` where it is set, else the login's own entry.
fn process_home() -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => home,
        _ => std::env::home_dir().map_or_else(|| "/".to_string(), |home| home.to_string_lossy().into_owned()),
    }
}

/// What the kinds area reads of an entry across an await, held by value so no borrow of the entry stays open (rule
/// 3.3): the record, the workspace and its machine, and nothing of the moves in flight.
fn agents_snapshot(entry: &Entry) -> LiveWorkspace {
    let live = entry.borrow();
    LiveWorkspace {
        record: live.record.clone(),
        opens_thread: None,
        of_record_held: false,
        ws: live.ws.clone(),
        machine: live.machine.clone(),
        generation: live.generation,
        waking: None,
        unchecked: false,
        napping: None,
        adopting: None,
        deleting: None,
        late_read: None,
        wake_stop: None,
        resize_to: None,
        nap_wait_ms: None,
        wake_said: None,
        delete_said: None,
        wake_ask: None,
        creating: false,
        checkout: None,
        checkout_reading: None,
        tree: None,
        tree_reading: None,
        pr: None,
        pr_reading: None,
        pr_poll: None,
        pr_poll_ms: None,
        pr_none_at: None,
    }
}

/// The exec road of a kind the host wired no module for: the TS throws at `moduleOf`, where the trait answers a factory,
/// so every launch through it fails with the same words.
struct Refused(String);

struct RefusedStream(String);

impl ExecStreamFactory for Refused {
    fn launch(&self, _command: &str, _options: wsp_adapters::port::ExecOptions) -> Arc<dyn wsp_adapters::port::ExecStream> {
        Arc::new(RefusedStream(self.0.clone()))
    }
}

impl wsp_adapters::port::ExecStream for RefusedStream {
    fn lines(&self) -> futures::stream::LocalBoxStream<'static, Result<String, wsp_adapters::port::ExecFailure>> {
        let failure = wsp_adapters::port::ExecFailure { kind: wsp_adapters::port::ExecFailureKind::Other, message: self.0.clone() };
        futures::stream::iter([Err(failure)]).boxed_local()
    }
    fn teardown(&self) {}
    fn kill(&self) {}
    fn write(&self, _line: String) -> LocalBoxFuture<'static, Result<wsp_adapters::port::WriteOutcome, wsp_adapters::port::ExecFailure>> {
        let failure = wsp_adapters::port::ExecFailure { kind: wsp_adapters::port::ExecFailureKind::Other, message: self.0.clone() };
        async move { Err(failure) }.boxed_local()
    }
    fn close_input(&self) {}
    fn exited(&self) -> wsp_adapters::port::SharedFuture<Option<i32>> {
        async { None }.boxed_local().shared()
    }
}

impl Ctx {
    /// `placeDoorOf()` (kinds.ts:221): the door, or the refusal that names the kind of machine no host wired.
    fn agents_door(&self) -> Result<Rc<dyn wsp_machines::places::types::PlaceDoor>, HostError> {
        self.wired_door().ok_or_else(|| HostError::new(no_kind_line("place")))
    }

    /// The live entry this id names, as `live.get(id)` reads it.
    fn agents_entry(&self, workspace_id: &str) -> Option<Entry> {
        self.live.borrow().get(workspace_id).cloned()
    }
}

impl AgentsCtx for Ctx {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn bus(&self) -> Rc<dyn EventBus> {
        self.bus.clone()
    }
    fn sessions(&self) -> Sessions {
        self.sessions.clone()
    }
    fn thread_records(&self) -> ThreadRecords {
        self.thread_records.clone()
    }
    fn entry(&self, workspace_id: &str) -> Option<Entry> {
        self.agents_entry(workspace_id)
    }
    fn closing(&self) -> bool {
        self.state.with(|s| s.closing)
    }
    fn adapters(&self) -> IndexMap<String, HarnessAdapterFactory> {
        self.opts.adapters.clone()
    }
    fn local_home_dir(&self) -> Option<String> {
        self.opts.local.as_ref().map(|local| local.home_dir.clone())
    }
    fn local_home(&self, agent: &str) -> Option<String> {
        self.opts.local.as_ref().map(|local| (local.home)(agent))
    }
    fn process_home(&self) -> String {
        process_home()
    }
    fn state_path(&self) -> Option<String> {
        self.opts.state_path.clone()
    }
    fn setup_off(&self, place: &str, agent: &str) -> bool {
        self.setups.off(place, agent)
    }
    fn setup_config_dir(&self, place: &str, agent: &str) -> Option<String> {
        self.setups.get(place, agent).and_then(|setup| setup.config_dir)
    }
    fn setup_launch(&self, place: &str, agent: &str) -> SetupLaunch {
        let launch = self.setups.launch_of(place, agent);
        SetupLaunch { env: launch.env, launch: launch.launch }
    }
    fn setup_refusal_moved(&self, place: &str, harness: &str) {
        self.boot.get().setup_refusal_moved(place, harness);
    }
    fn place_exec(&self, place: &str, script: &str, timeout_ms: u64) -> LocalBoxFuture<'static, Result<ExecResult, HostError>> {
        let door = self.agents_door();
        let (place, script) = (place.to_string(), script.to_string());
        async move { door?.exec(&place, &script, PlaceExecOpts { timeout_ms: Some(timeout_ms), stdin: None }).await }.boxed_local()
    }
    fn place_name(&self, place: &str) -> String {
        self.wired_door().map_or_else(|| place.to_string(), |door| door.name_of(place))
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().place_id_of(record)
    }
    fn at_provider(&self, place_id: Option<&str>) -> bool {
        at_provider(place_id, &*self.places)
    }
    fn place_agent_home(&self, place: &str, home: &str, agent: &str) -> Result<String, HostError> {
        self.kinds.get().place_agent_home(Some(place), home, agent)
    }
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let ctx = self.rc();
        async move { ctx.booted().await }.boxed_local()
    }
    fn know_broker(&self, entry: &Entry, harness: &str) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let kinds = self.kinds.get();
        let (held, harness) = (agents_snapshot(entry), harness.to_string());
        async move { kinds.know_broker(&held, &harness).await }.boxed_local()
    }
    fn unreached_of(&self, entry: &Entry) -> Option<String> {
        self.daemon.get().unreached_of(entry)
    }
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        self.transcripts.get().persist_sessions(workspace_id)
    }
    fn push_head(&self, thread_id: &str) {
        if let Err(e) = self.boot.get().push_head(thread_id) {
            wsp_core::console::warn(&e.message);
        }
    }
    fn write_session(
        &self,
        session: &str,
        write: LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>>,
    ) -> LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>> {
        self.threads.get().write_session(session, write)
    }
    fn kind_home(&self, entry: &Entry, agent: &str) -> String {
        let live = entry.borrow();
        let kind = live.record.borrow().kind;
        self.kinds.get().module_of(kind).map(|module| module.home(&live, agent)).unwrap_or_default()
    }
    fn kind_env(&self, entry: &Entry, harness: Option<&str>, home: &dyn Fn(&str) -> String) -> Env {
        let live = entry.borrow();
        let kind = live.record.borrow().kind;
        self.kinds.get().module_of(kind).map(|module| module.env(&live, harness, home)).unwrap_or_default()
    }
    fn kind_server_values(&self, entry: &Entry) -> Option<ServerValuesRoad> {
        let kind = entry.borrow().record.borrow().kind;
        self.kinds.get().module_of(kind).ok().map(|module| module.server_values())
    }
    fn kind_ends_threads(&self, entry: &Entry) -> bool {
        // `endThread !== undefined`: the module answers `None` where the kind has none, and a future it has not polled
        // where it has, which ends nothing.
        let live = entry.borrow();
        let kind = live.record.borrow().kind;
        self.kinds.get().module_of(kind).is_ok_and(|module| module.end_thread(&live, "", EndThread::default()).is_some())
    }
    fn kind_memory_key(&self, entry: &Entry, harness: &str) -> Option<String> {
        let live = entry.borrow();
        let kind = live.record.borrow().kind;
        self.kinds.get().module_of(kind).ok().and_then(|module| module.memory_key(&live, harness).ok().flatten())
    }
    fn kind_login_stands(&self, entry: &Entry, agent: &str) -> bool {
        let live = entry.borrow();
        let kind = live.record.borrow().kind;
        self.kinds.get().module_of(kind).is_ok_and(|module| module.login_stands(&live, agent))
    }
    fn computer_runs_in_folder(&self, computer: &str) -> bool {
        wsp_words::projects::runs_in_folder(&self.kinds.get().kind_of(computer))
    }
    fn brokered_launch(&self, entry: &Entry, harness: &str, token: Option<&str>) -> Result<Option<BrokeredLaunch>, HostError> {
        self.kinds.get().brokered_launch(&entry.borrow(), harness, token)
    }
    fn vault(&self) -> Env {
        self.opts.vault.as_ref().map(|vault| vault()).unwrap_or_default()
    }
    fn vault_on(&self, entry: &Entry) -> Env {
        self.kinds.get().vault_on(&entry.borrow())
    }
    fn exec_factory_for(&self, entry: &Entry, turn: TurnLaunch, waiting: Option<TurnWaiting>) -> Arc<dyn ExecStreamFactory> {
        // agents.ts:643-651: the place's turn limit with what only a turn's launch adds laid over it: the thread a kind that
        // groups processes groups under, a line's try, and for a sub workspace the kernel's kill reading.
        let record = entry.borrow().record.borrow().clone();
        let limit = self.channels.get().turn_limit_of(&record).map(|wall| wall.deadline_ms);
        let killed_why: Option<KilledWhy> = turn.killed_why.then(|| {
            let (sub_spawn, entry) = (self.sub_spawn.get(), entry.clone());
            let why: KilledWhy = Arc::new(move |started_at: i64| {
                let (sub_spawn, entry) = (sub_spawn.clone(), entry.clone());
                async move { sub_spawn.sub_killed_why(&entry, started_at as f64).await }.boxed_local()
            });
            why
        });
        let adds = turn.thread.is_some() || turn.asks_until_stopped || killed_why.is_some();
        let options = (limit.is_some() || adds).then(|| MachineExecOptions {
            deadline_ms: limit,
            thread: turn.thread,
            asks_until_stopped: turn.asks_until_stopped,
            killed_why,
            ..MachineExecOptions::default()
        });
        let held = agents_snapshot(entry);
        self.kinds.get().exec_factory_for(&held, options, waiting).unwrap_or_else(|refused| Arc::new(Refused(refused.message)))
    }
    fn signs_in(&self, _addresses: Vec<String>) -> LocalBoxFuture<'static, Option<SignsIn>> {
        // `opts.agentsReader?.signsIn?.(addresses)`: the reader is a host port no crate holds yet (ruling 3), so none answers.
        async { None }.boxed_local()
    }
    fn resolve_project(&self, reference: &str, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<ProjectView, HostError>> {
        let (projects, reference) = (self.projects.get(), reference.to_string());
        async move { projects.resolve(&reference, origin).await }.boxed_local()
    }
    fn list_projects(&self, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<Vec<ProjectView>, HostError>> {
        let projects = self.projects.get();
        async move { projects.list(origin).await }.boxed_local()
    }
    fn project_folder(&self, project: &ProjectView) -> LocalBoxFuture<'static, Result<Entry, HostError>> {
        let (folders, project) = (self.folders.get(), project.clone());
        async move { folders.project_folder(&project).await }.boxed_local()
    }
    fn spawn(&self, work: LocalBoxFuture<'static, ()>) {
        spawn_detached(work);
    }
}

#[async_trait(?Send)]
impl LaunchCtx for Ctx {
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    async fn entry_of(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Entry, HostError> {
        self.boot.get().entry_of(workspace_id, origin.as_ref(), Default::default()).await
    }
    fn keeps_agents(&self, entry: &Entry) -> bool {
        let kind = entry.borrow().record.borrow().kind;
        self.kinds.get().module_of(kind).is_ok_and(|module| module.keeps_agents())
    }
    fn kept_box_cap(&self, workspace_id: &str) -> Option<u32> {
        self.threads.get().kept_box(workspace_id).map(|kept| kept.cap as u32)
    }
    fn closing(&self) -> bool {
        self.state.with(|s| s.closing)
    }
    fn preferences_held(&self) -> Option<Preferences> {
        self.state.with(|s| s.preferences_held.clone())
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        Ok(self.preferences.get().get().await?)
    }
    fn default_agent_of(&self, prefs: &Preferences, entry: &Entry) -> String {
        self.agents.get().default_agent_of(prefs, Some(entry))
    }
    fn agent_off(&self, entry: &Entry, harness: &str) -> bool {
        self.agents.get().agent_off(entry, harness)
    }
    fn adapter_for(&self, entry: &Entry, ask: AdapterAsk) -> Result<BuiltAdapter, HostError> {
        self.agents.get().adapter_for(entry, ask)
    }
    fn adapter_warms(&self, adapter: &dyn HarnessAdapter) -> bool {
        adapter.warms()
    }
    async fn confine_setup(&self, entry: &Entry, harness: &str) -> Result<(), HostError> {
        self.agents.get().confine_setup(entry, Some(harness)).await
    }
    async fn catalog_on(&self, harness: &str, entry: &Entry, adapter: &dyn HarnessAdapter) -> Result<Option<HarnessCatalog>, HostError> {
        // `harnessCatalog(harness) === undefined ? undefined : ctx.catalogOn(table, entry, adapter)`. The area keeps the
        // adapter in the read it holds for the next ask, which the trait's borrow cannot give it, so the read goes to the
        // adapter the same factory builds for this entry and harness.
        let Some(table) = wsp_core::harness_catalog::harness_catalog(harness) else {
            return Ok(None);
        };
        let _ = adapter;
        let ask = AdapterAsk {
            named: Some(harness.to_string()),
            turn_env: None,
            waiting: None,
            servers: Env::new(),
            thread: None,
            asks_until_stopped: false,
        };
        let built = self.agents.get().adapter_for(entry, ask)?;
        Ok(Some(self.agents.get().catalog_on(&table, entry, built.adapter).await))
    }
    fn defaults_on(&self, catalog: HarnessCatalog, prefs: &Preferences, project: &str) -> DefaultsOn {
        let overrides = prefs.project_defaults.get(project).map(|fields| (**fields).clone());
        AgentsArea::defaults_on(&catalog, prefs, overrides.as_ref())
    }
    fn named_mode(&self, catalog: Option<&HarnessCatalog>, harness: &str, access: &AccessChoice) -> Result<Option<String>, HostError> {
        AgentsArea::named_mode(catalog, harness, *access).map(Some)
    }
    fn access_of(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> Option<String> {
        self.channels.get().access_of(workspace_id, thread_id, session)
    }
    fn ran_on(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> RanPicks {
        self.transcripts.get().ran_on(workspace_id, thread_id, session)
    }
    fn resumed_fact(&self, workspace_id: &str, session: &str, fact: &str) -> Option<String> {
        self.channels.get().resumed_fact(workspace_id, session, fact)
    }
    fn not_run_refused(&self, catalog: Option<&HarnessCatalog>, model: Option<&str>) -> Option<HostError> {
        wsp_core::harness_catalog::not_run_refused(catalog, model)
    }
    async fn thread_folder(&self, entry: &Entry, cwd: Option<&str>) -> Result<String, HostError> {
        let held = agents_snapshot(entry);
        self.kinds.get().thread_folder(&held, cwd).await
    }
    fn folder_exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
    fn runs_in(&self, entry: &Entry, ran_in: Option<&str>, folder: &str) -> String {
        self.folders.get().runs_in(entry, ran_in, folder)
    }
    fn setup_place(&self, entry: &Entry) -> Option<String> {
        AgentsArea::setup_place(entry)
    }
    fn launch_of(&self, place: &str, harness: &str) -> Option<AgentLaunch> {
        self.setups.launch_of(place, harness).launch
    }
    fn warm_on(&self, workspace_id: &str, harness: &str) -> Option<(String, KeptLaunch)> {
        self.threads.get().warm_on(workspace_id, harness).map(|(thread, kept)| (thread, kept.borrow().launch.clone()))
    }
    fn rewarm(&self, thread_id: &str) {
        self.threads.get().rewarm(thread_id);
    }
    fn reap_kept(&self, thread_id: &str) {
        self.threads.get().reap_kept(thread_id, false);
    }
    async fn thread_launch(&self, entry: &Entry, thread_id: &str, root_thread_id: &str, aside: bool) -> Result<ThreadLaunch, HostError> {
        let (workspace_id, kind) = {
            let live = entry.borrow();
            let record = live.record.borrow();
            (record.id.clone(), record.kind)
        };
        let launch = self.core.get().thread_launch(&workspace_id, kind, thread_id, root_thread_id, aside).await?;
        Ok(ThreadLaunch { scoped_device_id: launch.scoped.map(|device| device.device_id), env: launch.env, wsp: launch.wsp })
    }
    fn vault(&self) -> Env {
        self.opts.vault.as_ref().map(|vault| vault()).unwrap_or_default()
    }
    async fn launch_servers_for(
        &self,
        entry: &Entry,
        harness: &str,
        adapter: &dyn HarnessAdapter,
        cwd: &str,
    ) -> Result<LaunchServers, HostError> {
        self.agents.get().launch_servers_for(entry, harness, adapter, cwd).await
    }
    fn hold_warm(&self, thread_id: &str, hold: KeptHand) {
        self.threads.get().hold_warm(thread_id, hold);
    }
    async fn revoke_device(&self, device_id: &str) -> Result<(), HostError> {
        self.device_door.revoke(device_id).await.map(|_| ())
    }
    fn spawn(&self, work: LocalBoxFuture<'static, ()>) {
        spawn_detached(work);
    }
}

impl ThreadsCtx for Ctx {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn bus(&self) -> Rc<dyn EventBus> {
        self.bus.clone()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn sessions(&self) -> Sessions {
        self.sessions.clone()
    }
    fn thread_records(&self) -> ThreadRecords {
        self.thread_records.clone()
    }
    fn closing(&self) -> bool {
        self.state.with(|s| s.closing)
    }
    fn entry(&self, workspace_id: &str) -> Option<Entry> {
        self.agents_entry(workspace_id)
    }
    fn live_records(&self) -> Vec<WorkspaceRecord> {
        self.live.borrow().values().map(|entry| entry.borrow().record.borrow().clone()).collect()
    }
    fn taken(&self, workspace_id: &str, request_id: &str) -> Option<Taken> {
        self.transcript_state.with(|s| s.index.get(workspace_id).and_then(|index| index.taken.get(request_id).cloned()))
    }
    fn has_place_door(&self) -> bool {
        self.wired_door().is_some()
    }
    fn place_linked(&self, place: &str) -> bool {
        self.wired_door().is_some_and(|door| door.link(place).is_some())
    }
    fn on_place_move(&self, listener: Rc<dyn Fn(&PlaceMove)>) -> Option<Box<dyn Fn()>> {
        let door = self.wired_door()?;
        let id = door.on(Rc::new(move |event: &PlaceEvent| {
            listener(&match event {
                PlaceEvent::Present(present) => PlaceMove::Present(present.place_id.clone()),
                PlaceEvent::Removed(removed) => PlaceMove::Removed(removed.place_id.clone()),
                _ => PlaceMove::Other,
            })
        }));
        Some(Box::new(move || door.off(id)))
    }
    fn place_mem_mb(&self, place: &str) -> Option<f64> {
        self.wired_door()?.folder_computer(place).map(|computer| computer.mem_mb as f64)
    }
    fn place_threads_at(&self, place: &str) -> Option<i64> {
        self.wired_door()?.threads_at(place).map(|threads| threads as i64)
    }
    fn place_name(&self, place: &str) -> String {
        self.wired_door().map_or_else(|| place.to_string(), |door| door.name_of(place))
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().place_id_of(record)
    }
    fn provider_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().provider_of(record)
    }
    fn revoke_device(&self, device_id: &str) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let (devices, device_id) = (self.device_door.clone(), device_id.to_string());
        async move { devices.revoke(&device_id).await.map(|_| ()) }.boxed_local()
    }
    fn proc_scopes_moved(&self) {
        self.proc_scopes.get().proc_scopes_moved();
    }
    fn root_of(&self, thread_id: &str) -> Option<String> {
        Some(self.rules.get().root_of(thread_id))
    }
    fn interrupt(&self, row_id: &str, caller: Option<Caller>) -> LocalBoxFuture<'static, Result<SessionInterruptResult, HostError>> {
        use wsp_core::types::api::SessionsApi as _;
        let (sessions, row_id) = (self.sessions_area.get(), row_id.to_string());
        async move { sessions.interrupt(&row_id, caller, None).await }.boxed_local()
    }
    fn start(
        &self,
        workspace_id: &str,
        options: wsp_core::types::api::StartOptions,
        caller: Option<Caller>,
    ) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>> {
        use wsp_core::types::api::SessionsApi as _;
        let (sessions, workspace_id) = (self.sessions_area.get(), workspace_id.to_string());
        async move { sessions.start(&workspace_id, options, caller).await }.boxed_local()
    }
    fn stop_try(&self, row_id: &str) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let (sessions, row_id) = (self.sessions_area.get(), row_id.to_string());
        async move { sessions.stop_try(&row_id).await }.boxed_local()
    }
    fn mark(&self, threads: Vec<String>, mark: ThreadStamps, caller: Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let boot = self.boot.get();
        async move { boot.mark(&threads, Stamped::Same(mark), caller.as_ref()).await }.boxed_local()
    }
    fn sub_line(&self, workspace_id: &str, since: i64) -> LocalBoxFuture<'static, Option<String>> {
        let (sub_life, workspace_id) = (self.sub_life.get(), workspace_id.to_string());
        async move { sub_life.sub_line(&workspace_id, since as f64).await }.boxed_local()
    }
    fn boot_work(&self, workspace_id: &str) -> Option<Joined> {
        self.boot_work.borrow().get(workspace_id).cloned()
    }
    fn kept_writes(&self, workspace_id: &str) -> Option<Joined> {
        self.transcripts.get().kept_writes.borrow().get(workspace_id).cloned()
    }
    fn keep_sent_images(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: Option<String>,
        attachments: Vec<wsp_wire::types::attachments::Attachment>,
    ) -> LocalBoxFuture<'static, ()> {
        self.transcripts.get().keep_sent_images(workspace_id, thread_id, request_id.as_deref(), &attachments)
    }
    fn record(&self, event: SessionEvent) {
        self.transcripts.get().record(event);
    }
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        self.transcripts.get().persist_sessions(workspace_id)
    }
    fn open_thread(&self, workspace_id: &str, thread_id: &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>> {
        let opened = self.transcripts.get().open_thread(workspace_id, thread_id);
        async move { opened }.boxed_local()
    }
    fn idle_hold(&self, workspace_id: &str) {
        self.idle.get().hold(workspace_id);
    }
    fn idle_release(&self, workspace_id: &str) {
        self.idle.get().release(workspace_id);
    }
    fn port_roots_moved(&self, workspace_id: &str) {
        self.channels.get().port_roots_moved(workspace_id);
    }
    fn end_snooze_for(&self, view: &SessionView) {
        self.boot.get().end_snooze_for(view.thread_id.as_deref(), view.root_thread_id.as_deref());
    }
    fn with_daemon(&self, entry: &Entry, frame: serde_json::Value) -> LocalBoxFuture<'static, Result<serde_json::Value, AccountError>> {
        let (channels, entry) = (self.channels.get(), entry.clone());
        let serde_json::Value::Object(frame) = frame else {
            return async { Err(AccountError::from(HostError::new("a daemon frame is a JSON object"))) }.boxed_local();
        };
        async move { channels.with_daemon(&entry, Box::new(move |ask| ask(frame))).await.map(serde_json::Value::Object) }.boxed_local()
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        self.channels.get().checkout_of(record)
    }
    fn project_held(&self, project: &str) -> Result<ProjectView, HostError> {
        self.channels.get().project_held(project)
    }
    fn reach_of(&self, entry: &Entry) -> ReachState {
        self.reach.get().reach_of(entry).unwrap_or(ReachState::Unreachable)
    }
    fn turn_snapshot_ms(&self) -> Option<i64> {
        self.opts.turn_snapshot_ms.map(|ms| ms as i64)
    }
    fn vault(&self) -> IndexMap<String, String> {
        self.opts.vault.as_ref().map(|vault| vault()).unwrap_or_default()
    }
    fn vault_on(&self, entry: &Entry) -> IndexMap<String, String> {
        self.kinds.get().vault_on(&entry.borrow())
    }
    fn login_stands(&self, entry: &Entry, harness: &str) -> bool {
        let live = entry.borrow();
        let kind = live.record.borrow().kind;
        self.kinds.get().module_of(kind).is_ok_and(|module| module.login_stands(&live, harness))
    }
    fn computer_of(&self, entry: &Entry) -> String {
        self.channels.get().computer_of(entry)
    }
    fn ledger_limits(&self) -> LocalBoxFuture<'static, Result<Vec<AccountLimit>, HostError>> {
        let ctx = self.rc();
        async move { wsp_account::usage_reset::UsageResetDeps::limits(&*ctx).await }.boxed_local()
    }
    fn spawn(&self, work: LocalBoxFuture<'static, ()>) {
        spawn_detached(work);
    }
}
