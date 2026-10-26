// SPDX-License-Identifier: AGPL-3.0-only
//! `ReachCtx`, `DaemonRuntime`, `MachinesRuntime` and `IdleDoor` over the one context (reach.ts, daemon.ts,
//! machines.ts:15-906, runtime.ts:491-992).
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_core::clock::Cancel;
use wsp_core::console;
use wsp_core::reach::{connect_daemon, DaemonReach, ReachOptions};
use wsp_core::status::machine_state_of;
use wsp_core::types::internal::{KindModule, SessionEntry, ThreadRecord};
use wsp_core::types::pending::{Clock, KillConfirm, Store};
use wsp_core::types::wiring::{at_provider, GoldenExec, GoneOutcome, PlaceBackends, WorkspaceRecord};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_engine::vault::{export_paths, VaultOptions};
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::boot::entries::EntryAsk;
use wsp_machines::machines::daemon::DaemonArea;
use wsp_machines::machines::daemon::{DetachedStart, Started};
use wsp_machines::machines::machines::MachinesArea;
use wsp_machines::machines::machines::{Entry, IdleDoor, Ids, ImageRead, Row, Unreached};
use wsp_machines::machines::reach::ForMachine;
use wsp_machines::machines::reach::ReachArea;
use wsp_threads::threads::threads::ThreadsArea;
use wsp_threads::threads::transcripts::kept::ThreadsNamed;
use wsp_threads::threads::transcripts::TranscriptsArea;
use wsp_wire::types::JsNumber;
use wsp_wire::views::workspace::{Caller, ProjectView, ReachState, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::machine_link::MachineShape;
use wsp_words::exit::HostError;
use wsp_words::words::computer::no_kind_line;

use super::{Ctx, ExecHeld};
use crate::runtime::core::vault_paths_of;
use crate::runtime::join::spawn_detached;

/// The areas and shared maps the packet 4 tests put state into and read it back from, since the context's own
/// fields are crate-private.
#[doc(hidden)]
pub struct MachinesSeams {
    pub reach: Rc<ReachArea>,
    pub daemon: Rc<DaemonArea>,
    pub machines: Rc<MachinesArea>,
    pub transcripts: Rc<TranscriptsArea>,
    pub threads: Rc<ThreadsArea>,
    pub projects_held: Rc<RefCell<IndexMap<String, ProjectView>>>,
    pub execs: Rc<RefCell<IndexMap<u64, ExecHeld>>>,
}

impl Ctx {
    #[doc(hidden)]
    pub fn machines_seams(&self) -> MachinesSeams {
        MachinesSeams {
            reach: self.reach.get(),
            daemon: self.daemon.get(),
            machines: self.machines.get(),
            transcripts: self.transcripts.get(),
            threads: self.threads.get(),
            projects_held: self.projects_held.clone(),
            execs: self.execs.clone(),
        }
    }
}

/// `ctx.moduleOf(kind)` for a kind written as the word a record carries it as.
fn kind_of(word: &str) -> Result<wsp_wire::views::workspace_kind::WorkspaceKind, HostError> {
    serde_json::from_value(Value::String(word.to_string())).map_err(|_| HostError::new(no_kind_line(word)))
}

/// The row a copy of `row` is: `SessionEntry` carries no `Clone`, and every field it holds does.
fn copy_of(row: &SessionEntry) -> SessionEntry {
    SessionEntry {
        view: row.view.clone(),
        turn_id: row.turn_id.clone(),
        replaces: row.replaces.clone(),
        notify: row.notify.clone(),
        notify_by: row.notify_by.clone(),
        notify_road: row.notify_road,
        turn_token: row.turn_token.clone(),
        scope_device_id: row.scope_device_id.clone(),
        handle: row.handle.clone(),
        end: row.end.clone(),
        turn_live: row.turn_live.clone(),
        run: row.run.clone(),
        from: row.from,
        asked: row.asked.clone(),
        snapshot: row.snapshot.clone(),
        pid: row.pid,
        launch: row.launch.clone(),
        calls: row.calls.clone(),
        unanswered: row.unanswered,
        queued: row.queued,
    }
}

/// `async_trait` fixes the lifetime of the `&str` in `ReachCtx::on_this_computer`'s `Fn(&str)` to one that outlives the
/// call, so a home read off the area is handed on as `'static`: each distinct home is kept once.
fn interned(home: &str) -> &'static str {
    thread_local! {
        static HOMES: RefCell<std::collections::HashSet<&'static str>> = RefCell::default();
    }
    HOMES.with(|homes| {
        let mut homes = homes.borrow_mut();
        if let Some(kept) = homes.get(home) {
            return *kept;
        }
        let kept: &'static str = Box::leak(home.to_string().into_boxed_str());
        homes.insert(kept);
        kept
    })
}

fn host_of(e: AreaError) -> HostError {
    e.into_host()
}

impl Ctx {
    /// `ctx.view(record)`: the TS throws where the record's project is not held, and the trait answers a plain view.
    fn machines_view_of(&self, record: &WorkspaceRecord) -> WorkspaceView {
        self.records.get().view(record).unwrap_or_else(|e| panic!("{}", e.message))
    }

    /// `bus.emit(event)` for an event written as the JSON a TS object literal is: a listener that fails is said, since
    /// the caller has no one to hand the failure to.
    fn machines_emit_json(&self, event: Value) {
        match serde_json::from_value::<EventUnion>(event) {
            Ok(event) => {
                if let Err(e) = self.bus.emit(event) {
                    console::warn(&e.message);
                }
            }
            Err(e) => console::warn(&format!("an event was not emitted: {e}")),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::reach::ReachCtx for Ctx {
    fn live(&self, id: &str) -> Option<Entry> {
        self.live.borrow().get(id).cloned()
    }
    fn live_all(&self) -> Vec<Entry> {
        self.live.borrow().values().cloned().collect()
    }
    fn session_entries(&self) -> Vec<SessionEntry> {
        self.sessions.borrow().values().map(|row| copy_of(&row.borrow())).collect()
    }
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.kinds.get().backend_for(record)
    }
    fn pause_keeps_disk(&self, backend: &dyn MachineBackend) -> bool {
        self.kinds.get().pause_keeps_disk(backend)
    }
    async fn sweep_leftovers(&self, machine: &Arc<dyn Machine>) -> AreaResult<()> {
        wsp_engine::sweep_leftovers::sweep_leftovers(machine.as_ref()).await.map_err(AreaError::from)
    }
    fn closing(&self) -> bool {
        self.state.with(|s| s.closing)
    }
    fn provider_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().provider_of(record)
    }
    fn row_reason(&self, entry: &Entry) -> Option<String> {
        self.usage.get().row_reason(entry)
    }
    fn unreached_of(&self, entry: &Entry) -> Option<String> {
        self.daemon.get().unreached_of(entry)
    }
    fn kind_has_daemon(&self, entry: &Entry) -> AreaResult<bool> {
        let kind = entry.borrow().record.borrow().kind;
        let module = self.kinds.get().module_of(kind)?;
        let has = module.has_daemon(&entry.borrow());
        Ok(has)
    }
    fn view(&self, record: &WorkspaceRecord) -> Value {
        serde_json::to_value(self.machines_view_of(record)).expect("a view serializes as JSON")
    }
    fn machine_state_of(&self, phase: &WorkspacePhase) -> String {
        serde_json::to_value(machine_state_of(*phase)).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
    }
    fn idle_at(&self, workspace_id: &str) -> Option<f64> {
        self.idle.get().idle_at(workspace_id).map(|at| at as f64)
    }
    fn daemon_answers_ms(&self, entry: &Entry) -> AreaResult<f64> {
        let lifecycle = self.kinds.get().lifecycle_of(&entry.borrow())?;
        Ok(lifecycle.budgets().daemon_answers_ms.0)
    }
    fn daemon_hello_timeout_ms(&self) -> f64 {
        self.defaults.daemon_hello_timeout_ms as f64
    }
    async fn daemon_token_of(&self, machine: &Arc<dyn Machine>) -> AreaResult<Option<String>> {
        let token = self.transcripts.get().daemon_token_of(machine.clone(), None);
        token.await.map_err(AreaError::from)
    }
    fn connect_daemon(
        &self,
        preview_url: &str,
        token: &str,
        on_event: Rc<dyn Fn(DaemonEvent)>,
        heartbeat_ms: Option<i64>,
    ) -> AreaResult<Rc<dyn DaemonReach>> {
        connect_daemon(ReachOptions {
            preview_url: Some(preview_url.to_string()),
            socket: None,
            token: Some(token.to_string()),
            on_event,
            on_status: None,
            heartbeat_ms,
            seal: None,
            backoff_ms: None,
            clock: None,
            ids: None,
        })
        .map_err(|e| AreaError::Host(HostError::new(e.message)))
    }
    fn project_held(&self, id: &str) -> AreaResult<ProjectView> {
        self.channels.get().project_held(id).map_err(AreaError::from)
    }
    async fn with_daemon(&self, entry: &Entry, frame: Value) -> AreaResult<Value> {
        let Value::Object(frame) = frame else {
            return Err(AreaError::Host(HostError::new("a daemon frame is a JSON object")));
        };
        let asked = self.channels.get().with_daemon(entry, Box::new(move |ask| ask(frame))).await;
        asked.map(Value::Object).map_err(|e| AreaError::Host(e.into()))
    }
    async fn on_this_computer(&self, frame: Box<dyn Fn(&str) -> Value>) -> AreaResult<Value> {
        // The frame is built before the road is up, since `async_trait` leaves `frame` borrowed past the call; it
        // reads the home alone, which is `local.homeDir`, a field the area reads at the same point.
        let home = self.opts.local.as_ref().map(|local| local.home_dir.clone()).unwrap_or_default();
        let Value::Object(frame) = frame(interned(&home)) else {
            return Err(AreaError::Host(HostError::new("a daemon frame is a JSON object")));
        };
        let asked = self.channels.get().on_this_computer(Box::new(move |ask, _home| ask(frame))).await;
        asked.map(Value::Object).map_err(|e| AreaError::Host(e.into()))
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> AreaResult<String> {
        self.channels.get().checkout_of(record).map_err(AreaError::from)
    }
    fn served_by_its_computer(&self, entry: &Entry) -> bool {
        self.channels.get().served_by_its_computer(entry).is_some()
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()> {
        self.records.get().persist(record).await.map_err(AreaError::from)
    }
    fn emit(&self, event: Value) {
        self.machines_emit_json(event)
    }
    fn record(&self, event: Value) {
        match serde_json::from_value(event) {
            Ok(event) => self.transcripts.get().record(event),
            Err(e) => console::warn(&format!("a session event was not recorded: {e}")),
        }
    }
    fn now(&self) -> f64 {
        self.clock.now() as f64
    }
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(f, ms as i64)
    }
    fn sleep(&self, ms: f64) -> LocalBoxFuture<'static, ()> {
        let (done, wait) = futures::channel::oneshot::channel::<()>();
        let _ = self.clock.schedule(
            Box::new(move || {
                let _ = done.send(());
            }),
            ms as i64,
        );
        async move {
            let _ = wait.await;
        }
        .boxed_local()
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn warn(&self, line: &str) {
        console::warn(line)
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::daemon::DaemonRuntime for Ctx {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn warn(&self, line: &str) {
        console::warn(line)
    }
    fn daemon_hello_timeout_ms(&self) -> u64 {
        self.defaults.daemon_hello_timeout_ms
    }
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>> {
        self.live.clone()
    }
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>> {
        self.sessions.clone()
    }
    fn daemon_notes(&self) -> Rc<RefCell<IndexMap<String, String>>> {
        self.channels.get().daemon_notes.clone()
    }
    fn closed(&self) -> bool {
        self.state.with(|s| s.closed)
    }
    async fn sessions_start(&self, workspace_id: &str, ask: DetachedStart, origin: Option<&Caller>) -> Result<Started, HostError> {
        use wsp_core::types::api::{SessionsApi, StartOptions};
        let on_held = ask.on_held;
        let opts = StartOptions {
            prompt: ask.prompt,
            thread: ask.thread,
            request_id: Some(ask.request_id),
            on_held: Some(Rc::new(move |held: wsp_core::types::api::HeldStart| on_held(held.view, held.wait))),
            ..Default::default()
        };
        let handle = SessionsApi::start(&*self.sessions_area.get(), workspace_id, opts, origin.cloned()).await?;
        Ok(Started { outcome: handle.outcome(), view: handle.view() })
    }
    async fn push_status(&self, entry: &Entry) -> Result<(), HostError> {
        self.reach.get().push_status(entry).await.map_err(host_of)
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.records.get().persist(record).await
    }
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String> {
        let channels = self.channels.get();
        channels.served_by_its_computer(entry).map(|_| channels.computer_of(entry))
    }
    fn module_of(&self, kind: &str) -> Result<Rc<dyn KindModule>, HostError> {
        self.kinds.get().module_of(kind_of(kind)?)
    }
    fn project_path(&self, project_id: &str) -> Result<String, HostError> {
        Ok(self.channels.get().project_held(project_id)?.path)
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        self.channels.get().checkout_of(record)
    }
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.kinds.get().backend_for(record)
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::machines::MachinesRuntime for Ctx {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn wsp_core::ids::Ids> {
        self.ids.clone()
    }
    fn backend(&self) -> Arc<dyn MachineBackend> {
        self.backend.clone()
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn emit(&self, event: EventUnion) -> Result<(), HostError> {
        self.bus.emit(event).map(|_| ())
    }
    fn warn(&self, line: &str) {
        console::warn(line)
    }
    fn owner(&self) -> String {
        self.state.with(|s| s.owner.clone())
    }
    fn kill_confirm(&self) -> Option<KillConfirm> {
        self.opts.kill_confirm
    }
    fn on_exec(&self) -> Option<Rc<dyn Fn(&GoldenExec) -> Result<(), String>>> {
        let on_exec = self.opts.golden_recipe.as_ref()?.on_exec.clone()?;
        Some(Rc::new(move |exec: &GoldenExec| {
            on_exec(exec.clone());
            Ok(())
        }))
    }
    fn idle(&self) -> Rc<dyn IdleDoor> {
        Rc::new(self.idle.get())
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
    fn unreached(&self) -> Rc<RefCell<IndexMap<String, Unreached>>> {
        self.daemon.get().unreached()
    }
    fn nap_refusals(&self) -> Rc<RefCell<IndexMap<String, ForMachine<String>>>> {
        self.reach.get().nap_refusals.clone()
    }
    fn stop_refusals(&self) -> Rc<RefCell<IndexMap<String, ForMachine<String>>>> {
        self.reach.get().stop_refusals.clone()
    }
    fn project_held(&self, project_id: &str) -> bool {
        self.projects_held.borrow().contains_key(project_id)
    }
    fn kept_agents_on(&self, workspace_id: &str) -> Vec<String> {
        let threads = self.threads.get();
        let kept = threads.kept_agents.borrow();
        kept.iter().filter(|(_, held)| held.borrow().workspace_id == workspace_id).map(|(id, _)| id.clone()).collect()
    }
    fn end_execs(&self, workspace_id: &str, reason: &str) {
        let ends: Vec<Rc<dyn Fn(&str)>> =
            self.execs.borrow().values().filter(|held| held.workspace_id == workspace_id).map(|held| held.end.clone()).collect();
        for end in ends {
            end(reason);
        }
    }
    async fn entry_of(&self, id: &str) -> Result<Entry, HostError> {
        self.boot.get().entry_of(id, None, EntryAsk::default()).await
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        self.machines_view_of(record)
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.records.get().persist(record).await
    }
    async fn emit_status(&self, entry: &Entry, reach: ReachState, said: Option<&str>) -> Result<(), HostError> {
        self.reach.get().emit_status(entry, reach, said).await.map_err(host_of)
    }
    fn reach_of(&self, entry: &Entry) -> Result<ReachState, HostError> {
        self.reach.get().reach_of(entry).map_err(host_of)
    }
    fn nap_refused_of(&self, entry: &Entry) -> Option<String> {
        self.reach.get().nap_refused_of(entry)
    }
    fn nap_refused_reason(&self, entry: &Entry) -> Option<String> {
        self.reach.get().nap_refused_reason(entry)
    }
    fn stop_refused_reason(&self, entry: &Entry) -> Option<String> {
        self.reach.get().stop_refused_reason(entry)
    }
    async fn says_waking(&self, entry: &Entry, line: &str) -> Result<(), HostError> {
        self.reach.get().says_waking(entry, line).await.map_err(host_of)
    }
    async fn ping_daemon(&self, entry: &Entry) -> Option<String> {
        self.reach.get().ping_daemon(entry).await
    }
    fn sync_daemon(&self, entry: &Entry) {
        let synced = self.daemon.get().sync_daemon(entry);
        spawn_detached(async move {
            let _ = synced.await;
        });
    }
    async fn write_daemon_roots(&self, entry: &Entry) -> Result<(), HostError> {
        self.daemon.get().write_daemon_roots(entry, false).await
    }
    async fn sub_lost(&self, entry: &Entry, reason: &str) -> Result<GoneOutcome, HostError> {
        self.sub_life.get().sub_lost(entry, reason).await.map(|_| GoneOutcome::Settled).map_err(host_of)
    }
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.kinds.get().backend_for(record)
    }
    fn module_of(&self, kind: &str) -> Result<Rc<dyn KindModule>, HostError> {
        self.kinds.get().module_of(kind_of(kind)?)
    }
    fn keeps_images(&self, backend: &Arc<dyn MachineBackend>) -> bool {
        self.kinds.get().keeps_images(backend.as_ref())
    }
    fn names_workspace(&self, backend: &Arc<dyn MachineBackend>) -> bool {
        self.kinds.get().names_workspace(backend.as_ref())
    }
    fn pause_keeps_disk(&self, backend: &Arc<dyn MachineBackend>) -> bool {
        self.kinds.get().pause_keeps_disk(backend.as_ref())
    }
    async fn image_of(&self, golden: &str) -> Result<ImageRead, HostError> {
        Ok(ImageRead { version: self.records.get().image_of(golden).await?.version })
    }
    async fn recorded_images(&self) -> Result<Vec<String>, HostError> {
        Ok(self.records.get().recorded_images().await?.into_iter().collect())
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().place_id_of(record)
    }
    fn at_provider(&self, place_id: Option<&str>) -> bool {
        at_provider(place_id, &*self.places)
    }
    fn place_name_of(&self, place: &str) -> String {
        match self.place_door.get() {
            Some(door) => door.name_of(place),
            None => place.to_string(),
        }
    }
    fn place_nap_ms(&self, place_id: Option<&str>) -> Option<Option<f64>> {
        let settings = self.channels.get().settings_at(place_id)?;
        settings.nap_ms.map(|nap| nap.map(|ms| ms.0 .0))
    }
    async fn shape_of(&self, machine: &Arc<dyn Machine>) -> Option<MachineShape> {
        self.records.get().shape_of(machine.clone()).await
    }
    fn size_built(&self, shape: Option<&MachineShape>, asked: WorkspaceSize) -> WorkspaceSize {
        self.records.get().size_built(shape, &asked)
    }
    async fn read_memory(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: &Arc<dyn Machine>, sizes: &[WorkspaceSize]) -> bool {
        let mut read = record.borrow().clone();
        let offers: Vec<MachineSizeOffer> =
            sizes.iter().map(|s| MachineSizeOffer { cpu: s.cpu, mem_mb: s.mem_mb, rate_usd_per_hour: JsNumber(0.0), name: None }).collect();
        let counted = self.records.get().read_memory(&mut read, machine.clone(), &offers).await;
        if counted {
            record.borrow_mut().size.mem_mb = read.size.mem_mb;
        }
        counted
    }
    async fn vault_export(&self, machine: &Arc<dyn Machine>, max_bytes: u64, leave: Option<Vec<String>>) -> Result<Vec<u8>, HostError> {
        let paths = vault_paths_of(machine.as_ref(), self.opts.vault_paths.as_deref()).await?;
        let asked = VaultOptions {
            max_bytes: Some(max_bytes),
            leave: leave.unwrap_or_default(),
            exclude: self.opts.vault_caches.clone(),
            ..Default::default()
        };
        export_paths(machine.as_ref(), &paths, &asked).await.map_err(|e| host_of(AreaError::Engine(e)))
    }
    async fn gone_stop(&self, at: &Arc<dyn MachineBackend>, machine: &Arc<dyn Machine>) -> Result<(), HostError> {
        self.gone.stop(at.clone(), machine.clone()).await.map_err(|e| host_of(AreaError::Engine(e)))
    }
    fn forget_workspace_maps(&self, workspace_id: &str) {
        let daemon = self.daemon.get();
        daemon.revived_at_held().borrow_mut().shift_remove(workspace_id);
        daemon.unread_at().borrow_mut().shift_remove(workspace_id);
        daemon.unreached().borrow_mut().shift_remove(workspace_id);
        let reach = self.reach.get();
        reach.polled_reach.borrow_mut().shift_remove(workspace_id);
        reach.nap_refusals.borrow_mut().shift_remove(workspace_id);
        self.transcript_state.with(|t| {
            t.pending.shift_remove(workspace_id);
            t.pending_bytes.shift_remove(workspace_id);
            t.index.shift_remove(workspace_id);
        });
        self.channels.get().daemon_notes.borrow_mut().shift_remove(workspace_id);
    }
    fn reap_kept(&self, thread_id: &str) {
        self.threads.get().reap_kept(thread_id, false)
    }
    async fn forget_slate(&self, thread_id: &str) -> Result<(), HostError> {
        let slates = self.slates.get();
        slates.forget(thread_id, false).await
    }
    fn forget_viewed(&self, workspace_id: &str) {
        self.transcripts.get().viewed_marks.borrow_mut().shift_remove(workspace_id);
    }
    fn cancel_flush(&self, workspace_id: &str) {
        self.transcripts.get().cancel_flush(workspace_id)
    }
    async fn settle_flushes(&self, workspace_id: &str) {
        let transcripts = self.transcripts.get();
        let queued = transcripts.transcript_queue.borrow().get(workspace_id).cloned();
        if let Some(queued) = queued {
            queued.await;
        }
        transcripts.transcript_queue.borrow_mut().shift_remove(workspace_id);
        let flushing = self.index_flushes.with(|f| f.get(workspace_id).cloned());
        if let Some(flushing) = flushing {
            flushing.await;
        }
        self.index_flushes.with(|f| f.shift_remove(workspace_id));
    }
    fn clear_rows(&self, workspace_id: &str) {
        if let Some(rows) = self.store.transcripts() {
            if let Err(e) = rows.clear(workspace_id) {
                console::warn(&e.message);
            }
        }
        self.transcript_state.with(|t| t.unread.shift_remove(workspace_id));
    }
    async fn drop_sent_images(&self, workspace_id: &str) -> Result<(), HostError> {
        self.transcripts.get().drop_sent_images(workspace_id, ThreadsNamed::All).await;
        Ok(())
    }
    fn forget_kept_writes(&self, workspace_id: &str) {
        self.transcripts.get().kept_writes.borrow_mut().shift_remove(workspace_id);
    }
}

impl IdleDoor for Ctx {
    fn touch(&self, workspace_id: &str) {
        self.idle.get().touch(workspace_id)
    }
    fn forget(&self, workspace_id: &str) {
        self.idle.get().forget(workspace_id)
    }
}
