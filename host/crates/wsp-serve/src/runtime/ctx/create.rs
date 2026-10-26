// SPDX-License-Identifier: AGPL-3.0-only
//! The context's create and sub-spawn readings (create.ts, sub-spawn.ts, and the `ctx.` members they call): most of
//! them one call on `opts`, the core's maps or the place door, the rest a forward to the area that holds the rule.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::mpsc;
use futures::future::LocalBoxFuture;
use futures::{FutureExt as _, StreamExt as _};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_account::account::core_types::{AccountError, Ask, Work};
use wsp_account::projects::projects::add::{LandingDeps as AccountDeps, LandingKind};
use wsp_core::project_landing::{self, LandingComputer, LandingDeps, ProjectLanding};
use wsp_core::types::internal::{dead_machine, is_absent_machine, CREATES, SEED_CHOICES};
use wsp_core::types::pending::Clock;
use wsp_core::types::wiring::{at_provider, StageReport, WorkspaceRecord};
use wsp_engine::box_backend::BoxLimits;
use wsp_engine::gone::kill_until_gone;
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::images::image::Announce;
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::boot::ComputerRow;
use wsp_machines::machines::create::{CreateRuntime, KeptBuilder, LandingRoad, RecordBranchAsk};
use wsp_machines::machines::kinds::KindsCtx;
use wsp_machines::machines::machines::{Entry, Ids};
use wsp_machines::machines::sub_life::SessionFile;
use wsp_machines::machines::sub_spawn::{ImageRecord, Packed, ProviderRow, SubSpawnCtx};
use wsp_machines::machines::{brokered, create::CreateArea};
use wsp_machines::places::types::PlaceDoor;
use wsp_machines::projects::branch_of_record::{self, NamedAsk};
use wsp_threads::threads::agents::launch::AdapterAsk;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::GoldenManifest;
use wsp_wire::views::session::ThreadCapWait;
use wsp_wire::views::workspace::{BranchOfRecord, Caller, ProjectView, ReachState, SeedChoice, SeedPlan};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ProjectRef, ThreadScope, WorkspaceAgents, WorkspaceView};
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;
use wsp_words::words::computer::{no_kind_line, SpawnAct};

use super::Ctx;
use crate::runtime::join::{spawn_detached, RuntimeRetryClock};

/// `randomUUID()` and `randomBytes(n)` for the machines' areas, which name their own `Ids`.
struct IdsOf(Rc<dyn wsp_core::ids::Ids>);

impl Ids for IdsOf {
    fn uuid(&self) -> String {
        self.0.uuid()
    }
    fn bytes(&self, n: usize) -> Vec<u8> {
        self.0.bytes(n)
    }
    fn random(&self) -> f64 {
        self.0.random()
    }
}

/// The landing road a create reads, over wsp-core's: the deps the create passes between its calls ride as the JSON
/// `Ctx::landing_deps` writes, which carries what a road reads without a machine (`computerName`, `macStateHome`, and
/// `computer.home` for a joined computer).
struct RoadOver(Rc<dyn ProjectLanding>);

/// The deps a road reads for a refusal, a memory folder or the binds, off the JSON `landing_deps` wrote. The calls
/// that need a machine (`land`, `remove`, `branch`) are not the create's and never see these.
fn deps_of(value: &Value) -> LandingDeps {
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let computer = value
        .get("computer")
        .and_then(|c| c.get("home"))
        .and_then(Value::as_str)
        .map(|home| LandingComputer { machine: dead_machine(""), home: home.to_string() });
    LandingDeps {
        land: Rc::new(|_, _, _| async { Err(HostError::new("a create lands nothing through a landing road's deps")) }.boxed_local()),
        scratch: Rc::new(|_| String::new()),
        computer,
        mac_state_home: text("macStateHome"),
        branch_here: Rc::new(|_| async { None }.boxed_local()),
        computer_name: text("computerName"),
        clone_env: None,
        now: Rc::new(|| 0),
    }
}

impl LandingRoad for RoadOver {
    fn kind(&self) -> String {
        self.0.kind().to_string()
    }
    fn refusal(&self, project: &ProjectView, deps: &Value) -> Option<String> {
        self.0.refusal(project, &deps_of(deps))
    }
    fn memory_dir(&self, project: &ProjectView, memory_key: &str, deps: &Value) -> String {
        match self.0.places(project, memory_key, &deps_of(deps)) {
            Ok(places) => places.memory_dir,
            Err(_) => project.memory_dir.clone(),
        }
    }
    fn workspace_binds(&self, project: &ProjectView) -> Vec<wsp_wire::views::place::MachineBind> {
        self.0.workspace_binds(project)
    }
}

/// The deps a road reads, as the JSON `RoadOver` takes back.
fn deps_value(deps: &AccountDeps) -> Value {
    let mut out = json!({ "computerName": deps.computer_name, "macStateHome": deps.mac_state_home });
    if let Some(computer) = &deps.computer {
        out["computer"] = json!({ "home": computer.home });
    }
    out
}

fn kind_of_word(kind: &str) -> Option<WorkspaceKind> {
    serde_json::from_value(Value::String(kind.to_string())).ok()
}

fn tree_map(scope: Option<&ThreadScope>, rules: &wsp_account::account::rules::RulesArea) -> Map<String, Value> {
    let tree = rules.tree_of(scope);
    let mut out = Map::new();
    if let Some(parent) = tree.parent_thread_id {
        out.insert("parentThreadId".into(), Value::String(parent));
    }
    if let Some(root) = tree.root_thread_id {
        out.insert("rootThreadId".into(), Value::String(root));
    }
    out
}

impl Ctx {
    fn create_place_door(&self) -> Option<Rc<dyn PlaceDoor>> {
        self.place_door.get()
    }

    fn create_broker_ready(&self) -> bool {
        self.opts.broker.as_ref().is_some_and(|broker| broker.ready())
    }

    /// `ctx.brokered({kind, place})`: the host's broker is ready and the workspace is a fork at a provider.
    fn create_brokered_at(&self, kind: &str, place: Option<&str>) -> bool {
        self.create_broker_ready()
            && kind_of_word(kind) == Some(WorkspaceKind::Cloud)
            && (place.is_none() || at_provider(place, &*self.places))
    }

    /// The `Entry`'s workspace and record, held past an await without the borrow.
    fn create_live_records(&self) -> Vec<Rc<RefCell<WorkspaceRecord>>> {
        self.live.borrow().values().map(|entry| entry.borrow().record.clone()).collect()
    }
}

/// `async_trait` ties the `&str` of `building` to the borrow of `building` itself, so a line said here, which no caller
/// keeps past its call, is handed over at that lifetime.
fn said_to<'a, 'b>(building: &'b dyn Fn(&'a str, f64), place: &str, rate: f64) {
    let place: &'a str = unsafe { std::mem::transmute::<&str, &'a str>(place) };
    building(place, rate);
}

fn host_of(e: AccountError) -> HostError {
    e.into()
}

#[async_trait(?Send)]
impl CreateRuntime for Ctx {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        Rc::new(IdsOf(self.ids.clone()))
    }
    fn emit(&self, event: EventUnion) -> Result<(), HostError> {
        self.bus.emit(event).map(|_| ())
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>> {
        self.live.clone()
    }
    fn wired(&self) -> String {
        self.places.wired()
    }
    fn place_door(&self) -> Option<Rc<dyn PlaceDoor>> {
        self.create_place_door()
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        self.boot.get().landing_place(computer).await
    }
    async fn place_refuses(&self, place_id: Option<&str>) -> Result<(), HostError> {
        self.channels.get().place_refuses(place_id).await
    }
    async fn landing_backend(&self, place_id: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.builders_area.get().landing_backend(place_id).await
    }
    fn landing_kind(&self, computer: &str, _at: &Arc<dyn MachineBackend>) -> String {
        match self.projects.get().landing_kind(computer) {
            LandingKind::Mac => "mac",
            LandingKind::Box => "box",
            LandingKind::Provider => "provider",
        }
        .to_string()
    }
    async fn landing_deps(&self, computer: &str) -> Result<Value, HostError> {
        Ok(deps_value(&self.projects.get().landing_deps(computer).await?.deps))
    }
    fn project_landing(&self, kind: &str) -> Result<Rc<dyn LandingRoad>, HostError> {
        let road = project_landing::project_landing(kind)?;
        Ok(Rc::new(RoadOver(road)))
    }
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> {
        self.rules.get().remember_project(project).await
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        self.channels.get().project_held(project_id)
    }
    fn keeps_images(&self, at: &Arc<dyn MachineBackend>) -> bool {
        self.kinds.get().keeps_images(&**at)
    }
    async fn copy_for_fork(&self, golden: &str, place_id: Option<&str>, building: &dyn Fn(&str, f64)) -> Result<String, HostError> {
        let (said, mut heard) = mpsc::unbounded::<(String, f64)>();
        let announce: Announce = Rc::new(move |place: &str, rate: f64| {
            let _ = said.unbounded_send((place.to_string(), rate));
        });
        let image = self.image.get();
        let copy = image.copy_for_fork(golden, place_id, Some(announce)).fuse();
        futures::pin_mut!(copy);
        let mut open = true;
        let made = loop {
            if !open {
                break copy.await;
            }
            futures::select! {
                line = heard.next() => match line {
                    Some((place, rate)) => said_to(building, &place, rate),
                    None => open = false,
                },
                made = copy => break made,
            }
        };
        while let Ok((place, rate)) = heard.try_recv() {
            said_to(building, &place, rate);
        }
        made.map_err(|e| HostError::new(e.message()))
    }
    async fn image_head(&self) -> Result<String, HostError> {
        self.boot.get().image_head().await
    }
    async fn image_size(&self, golden: &str) -> Result<Option<WorkspaceSize>, HostError> {
        Ok(self.records.get().image_of(golden).await?.version.and_then(|version| version.size))
    }
    fn brokered_start(&self, provider: bool) -> bool {
        branch_of_record::brokered_start(self.create_broker_ready(), provider)
    }
    async fn named_of_record(&self, ask: RecordBranchAsk) -> Result<BranchOfRecord, HostError> {
        let ask = NamedAsk { remote: ask.remote, name: ask.name, project: ask.project, on: ask.on, base: ask.base, carried: ask.carried };
        self.branch_area().named_of_record(&ask).await
    }
    fn place_name(&self, place: &str) -> String {
        KindsCtx::place_name(self, place)
    }
    fn place_door_name(&self, place: &str) -> String {
        self.create_place_door().map_or_else(|| place.to_string(), |door| door.name_of(place))
    }
    fn spawn_at(&self, place: &str) -> WorkspaceAgents {
        self.channels.get().spawn_at(Some(place))
    }
    fn tree_of(&self, spawned: Option<&ThreadScope>) -> Map<String, Value> {
        tree_map(spawned, &self.rules.get())
    }
    fn attach(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>) -> Result<Entry, HostError> {
        self.machines.get().attach(record, machine)
    }
    async fn fork(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        bind: &dyn for<'m, 'r> Fn(&'m Arc<dyn Machine>, &'r WorkspaceRecord) -> Result<(), HostError>,
        report: StageReport,
    ) -> Result<Arc<dyn Machine>, HostError> {
        self.machines.get().fork(record, bind, None, Some(report), false).await.map_err(AreaError::into_host)
    }
    async fn refresh_builders(&self) -> Result<(), HostError> {
        self.boot.get().refresh_builders().await
    }
    fn builders(&self) -> Vec<KeptBuilder> {
        self.builders
            .borrow()
            .values()
            .map(|held| {
                let held = held.borrow();
                KeptBuilder {
                    id: held.record.id.clone(),
                    name: held.record.name.clone(),
                    life: held.life,
                    sealed_version: held.record.sealed.as_ref().map(|sealed| sealed.version),
                    place: held.record.place.clone(),
                }
            })
            .collect()
    }
    fn cancel_grace(&self, builder_id: &str) {
        let cancel = self.builders_area.get().grace_timers.borrow_mut().shift_remove(builder_id);
        if let Some(cancel) = cancel {
            cancel();
        }
    }
    async fn kill_builder(&self, builder_id: &str) -> Result<(), HostError> {
        let held = self.builders.borrow().values().find(|held| held.borrow().record.id == builder_id).cloned();
        let Some(held) = held else {
            return Err(HostError::new("Cannot read properties of undefined (reading 'record')"));
        };
        let (machine, place) = {
            let held = held.borrow();
            (held.builder.machine.clone(), held.record.place.clone().unwrap_or_else(|| self.places.wired()))
        };
        let at = self.builders_area.get().backend_at(&place)?;
        let confirm = self.opts.kill_confirm.unwrap_or_default();
        kill_until_gone(&*at, &*machine, confirm, &RuntimeRetryClock(self.clock.clone())).await.map_err(|e| HostError::new(e.to_string()))
    }
    async fn forget_builder(&self, builder_id: &str) -> Result<(), HostError> {
        self.builders_area.get().forget_builder(builder_id).await
    }
    fn holds_slot(&self, record: &WorkspaceRecord) -> bool {
        self.rules.get().holds_slot(record)
    }
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String> {
        self.channels.get().served_by_its_computer(entry).map(|_| String::new())
    }
    fn daemon_answers_ms(&self, entry: &Entry) -> Result<u64, HostError> {
        let lifecycle = self.kinds.get().lifecycle_of(&entry.borrow())?;
        Ok(lifecycle.budgets().daemon_answers_ms.0 as u64)
    }
    async fn daemon_reach(&self, entry: &Entry) -> Result<(), HostError> {
        let ws = entry.borrow().ws.clone();
        ws.daemon_reach().await.map(|_| ()).map_err(|e| HostError::new(e.to_string()))
    }
    async fn ping_daemon(&self, entry: &Entry) -> Option<String> {
        self.reach.get().ping_daemon(entry).await
    }
    fn sync_daemon(&self, entry: &Entry) {
        let sync = self.daemon.get().sync_daemon(entry);
        spawn_detached(async move {
            let _ = sync.await;
        });
    }
    async fn read_memory(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: &Arc<dyn Machine>, sizes: &[WorkspaceSize]) -> bool {
        let offers: Vec<MachineSizeOffer> =
            sizes.iter().map(|s| MachineSizeOffer { cpu: s.cpu, mem_mb: s.mem_mb, rate_usd_per_hour: JsNumber(0.0), name: None }).collect();
        let mut held = record.borrow().clone();
        let read = self.records.get().read_memory(&mut held, machine.clone(), &offers).await;
        if read {
            record.borrow_mut().size.mem_mb = held.size.mem_mb;
        }
        read
    }
    fn module_of(&self, kind: &str) -> Result<Rc<dyn wsp_core::types::internal::KindModule>, HostError> {
        let kind = kind_of_word(kind).ok_or_else(|| HostError::new(no_kind_line(kind)))?;
        self.kinds.get().module_of(kind)
    }
    async fn write_daemon_roots(&self, entry: &Entry) -> Result<(), HostError> {
        self.daemon.get().write_daemon_roots(entry, false).await
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.records.get().persist(record).await
    }
    async fn mark_used(&self, place_id: Option<&str>) -> Result<(), HostError> {
        match self.create_place_door() {
            Some(door) => door.mark_used(place_id).await,
            None => Ok(()),
        }
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        self.records.get().view(record).unwrap_or_else(|e| panic!("{}", e.message))
    }
    fn ref_of(&self, project: &ProjectView) -> ProjectRef {
        self.channels.get().ref_of(project)
    }
}

#[async_trait(?Send)]
impl SubSpawnCtx for Ctx {
    fn live(&self, id: &str) -> Option<Entry> {
        self.live.borrow().get(id).cloned()
    }
    fn remove_live(&self, id: &str) {
        self.live.borrow_mut().shift_remove(id);
    }
    fn is_absent_machine(&self, entry: &Entry) -> bool {
        is_absent_machine(&*entry.borrow().machine)
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.thread_records.borrow().iter().map(|(thread_id, held)| (thread_id.clone(), held.workspace_id.clone())).collect()
    }
    fn push_head(&self, thread_id: &str) {
        let _ = self.boot.get().push_head(thread_id);
    }
    async fn local_machine(&self) -> AreaResult<Arc<dyn Machine>> {
        KindsCtx::local_machine(self).await
    }
    fn provider_backend(&self, id: &str) -> Option<Arc<dyn MachineBackend>> {
        self.places.backend(id).or_else(|| self.create_place_door()?.backend_of(id))
    }
    async fn computer_rows(&self) -> AreaResult<Vec<ProviderRow>> {
        let rows: Vec<ComputerRow> = self.boot.get().computer_rows().await;
        Ok(rows
            .into_iter()
            .map(|row| ProviderRow {
                id: row.id,
                name: row.name,
                kind: serde_json::to_value(row.kind).ok().and_then(|word| word.as_str().map(str::to_string)).unwrap_or_default(),
            })
            .collect())
    }
    async fn landing_place(&self, row_id: &str) -> AreaResult<Option<String>> {
        Ok(self.boot.get().landing_place(row_id).await?)
    }
    fn place_settings(&self, place: &str) -> Option<Map<String, Value>> {
        match serde_json::to_value(self.create_place_door()?.settings_at(place)).ok()? {
            Value::Object(settings) => Some(settings),
            _ => None,
        }
    }
    fn delete_after_ms(&self, place: &str) -> Option<f64> {
        self.create_place_door()?.settings_at(place).delete_after_ms.map(|ms| ms.0 .0)
    }
    fn place_name(&self, computer: &str) -> String {
        KindsCtx::place_name(self, computer)
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().place_id_of(record)
    }
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.kinds.get().backend_for(record)
    }
    fn name_given(&self, name: &str) -> String {
        CreateArea::name_given(name)
    }
    fn spawn_guard(&self, origin: Option<&Caller>) -> Result<Box<dyn FnOnce()>, HostError> {
        let release = self.rules.get().spawn_guard(SpawnAct::Fork, origin)?;
        Ok(Box::new(move || release()))
    }
    fn project_held(&self, id: &str) -> Result<ProjectView, HostError> {
        self.channels.get().project_held(id)
    }
    async fn copy_blocked(&self, lead: &Entry) -> AreaResult<()> {
        Ok(self.channels.get().copy_blocked(lead).await?)
    }
    async fn daemon_frame(&self, entry: &Entry, frame: Value) -> AreaResult<Value> {
        let Value::Object(frame) = frame else {
            return Err(HostError::new("a daemon frame is an object").into());
        };
        let work: Work<Value> = Box::new(move |ask: Ask| async move { ask(frame).await.map(Value::Object) }.boxed_local());
        self.channels.get().with_daemon(entry, work).await.map_err(|e| AreaError::from(host_of(e)))
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        self.channels.get().checkout_of(record)
    }
    fn brokered(&self, kind: &str, place: Option<&str>) -> bool {
        self.create_brokered_at(kind, place)
    }
    fn brokered_start(&self) -> bool {
        branch_of_record::brokered_start(self.create_broker_ready(), true)
    }
    async fn repo_of_record(&self, remote: &str, project: &str, on: &str) -> AreaResult<Map<String, Value>> {
        let repo = self.branch_area().repo_of_record(remote, project, on).await?;
        let mut out = Map::new();
        out.insert("repoId".into(), json!(repo.repo_id));
        out.insert("owner".into(), json!(repo.owner));
        out.insert("name".into(), json!(repo.name));
        if repo.public {
            out.insert("public".into(), json!(true));
        }
        Ok(out)
    }
    fn free_branch_name(&self, wanted: &str, heads: Option<&str>) -> String {
        branch_of_record::free_branch_name(&self.create_live_records(), wanted, heads)
    }
    async fn copy_of(&self, place: &str, image: &str) -> AreaResult<Option<GoldenManifest>> {
        Ok(self.records.get().copy_of(place, image).await?)
    }
    async fn record_of(&self, image: &str) -> AreaResult<Option<ImageRecord>> {
        let Some(sealed) = self.records.get().record_of(image).await? else { return Ok(None) };
        let logins = sealed
            .logins
            .iter()
            .map(|login| {
                (
                    login.name.clone(),
                    serde_json::to_value(login.state).ok().and_then(|s| s.as_str().map(str::to_string)).unwrap_or_default(),
                )
            })
            .collect();
        let agents = sealed
            .recipe
            .map(|recipe| {
                recipe
                    .rows
                    .into_iter()
                    .filter(|row| {
                        serde_json::to_value(row.kind).ok().and_then(|k| k.as_str().map(str::to_string)).as_deref() == Some("agent")
                    })
                    .map(|row| (row.id, row.on))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Some(ImageRecord { logins: Some(logins), agents }))
    }
    async fn place_guard(&self, place: &str) -> AreaResult<()> {
        Ok(self.channels.get().place_guard(place).await?)
    }
    async fn default_agent_of(&self, lead: &Entry) -> AreaResult<String> {
        let held = self.state.with(|state| state.preferences_held.clone());
        let prefs = match held {
            Some(prefs) => prefs,
            None => self.preferences.get().get().await.map_err(|e| AreaError::from(host_of(e)))?,
        };
        Ok(self.agents.get().default_agent_of(&prefs, Some(lead)))
    }
    fn unbrokered_refusal(&self, harness: &str) -> Option<HostError> {
        brokered::unbrokered_refusal(harness)
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        self.create.get().name_refusal(name)
    }
    fn tree_of(&self, scope: &ThreadScope) -> IndexMap<String, Value> {
        tree_map(Some(scope), &self.rules.get()).into_iter().collect()
    }
    fn forking_add(&self, name: &str) {
        self.create.get().forking.borrow_mut().insert(name.to_string());
    }
    fn forking_remove(&self, name: &str) {
        self.create.get().forking.borrow_mut().shift_remove(name);
    }
    fn supersede_failed(&self, name: &str) {
        self.create.get().supersede_failed(name);
    }
    async fn enqueue(&self, record: &Rc<RefCell<WorkspaceRecord>>, wait: Option<ThreadCapWait>) -> AreaResult<()> {
        self.sub_queue.get().enqueue(record, wait, false).await
    }
    fn queue_wait(&self, place: &str, limits: Option<&BoxLimits>) -> Option<ThreadCapWait> {
        self.sub_queue.get().queue_wait(place, limits)
    }
    fn view(&self, record: &WorkspaceRecord) -> Value {
        let view = self.records.get().view(record).unwrap_or_else(|e| panic!("{}", e.message));
        serde_json::to_value(view).expect("a view serializes as JSON")
    }
    fn stage_reporter(&self, id: &str, name: &str, started_at: f64, asked_by: Option<Value>) -> StageReport {
        let scope = asked_by.and_then(|asked| {
            let word = |key: &str| asked.get(key).and_then(Value::as_str).map(str::to_string);
            serde_json::from_value::<ThreadScope>(json!({
                "kind": "thread",
                "threadId": word("threadId")?,
                "rootThreadId": word("rootThreadId")?,
                "workspaceId": word("workspaceId").unwrap_or_default(),
            }))
            .ok()
        });
        self.create.get().stage_reporter(id, name, started_at as i64, scope.as_ref())
    }
    async fn fork(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        on_machine: Box<dyn Fn(Arc<dyn Machine>) -> Result<(), HostError>>,
        report: StageReport,
    ) -> AreaResult<()> {
        let bind = move |machine: &Arc<dyn Machine>, _: &WorkspaceRecord| on_machine(machine.clone());
        self.machines.get().fork(record, &bind, None, Some(report), false).await?;
        Ok(())
    }
    fn attach(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>) -> Result<Entry, HostError> {
        self.machines.get().attach(record, machine)
    }
    fn attach_dead(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Result<Entry, HostError> {
        self.machines.get().attach(record, dead_machine(""))
    }
    async fn ping_daemon(&self, entry: &Entry) -> Option<String> {
        self.reach.get().ping_daemon(entry).await
    }
    fn sync_daemon(&self, entry: &Entry) {
        let sync = self.daemon.get().sync_daemon(entry);
        spawn_detached(async move {
            let _ = sync.await;
        });
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()> {
        Ok(self.records.get().persist(record).await?)
    }
    async fn emit_status_reachable(&self, entry: &Entry) -> AreaResult<()> {
        self.reach.get().emit_status(entry, ReachState::Reachable, None).await
    }
    async fn unfork(&self, entry: &Entry) -> AreaResult<()> {
        Ok(self.machines.get().unfork(entry).await?)
    }
    fn broker_lift(&self, workspace_id: &str) {
        if let Some(broker) = &self.opts.broker {
            broker.lift(workspace_id);
        }
    }
    async fn rows_sent(&self) {
        self.boot.get().rows_sent().await;
    }
    fn session_file(&self, entry: &Entry, agent: &str, session_id: &str) -> Option<SessionFile> {
        let ask = AdapterAsk { named: Some(agent.to_string()), ..AdapterAsk::default() };
        let built = self.agents.get().adapter_for(entry, ask).ok()?;
        built.adapter.session_file(session_id).map(|file| SessionFile { folder: file.folder, name: file.name, depth: file.depth })
    }
    async fn get_seed_choice(&self, folder: &str) -> AreaResult<Option<SeedChoice>> {
        let held = self.store.get(SEED_CHOICES, folder).await?;
        Ok(held.and_then(|value| serde_json::from_value(value).ok()))
    }
    async fn get_blob(&self, collection: &str, key: &str) -> AreaResult<Option<Vec<u8>>> {
        Ok(self.store.get_blob(collection, key).await?)
    }
    async fn delete_create(&self, key: &str) -> AreaResult<()> {
        Ok(self.store.delete(CREATES, key).await?)
    }
    fn seeds(&self) -> bool {
        self.opts.seed.is_some()
    }
    async fn homes_here(&self) -> AreaResult<Value> {
        let homes = self.agents.get().homes_here().await?;
        Ok(Value::Object(homes.into_iter().map(|(agent, home)| (agent, Value::String(home))).collect()))
    }
    async fn seed_plan(&self, folder: &str, homes: &Value) -> AreaResult<SeedPlan> {
        let seed = self.opts.seed.clone().ok_or_else(|| HostError::new("Cannot read properties of undefined (reading 'plan')"))?;
        Ok(seed.plan(folder, &homes_of(homes)).await?)
    }
    async fn seed_pack(&self, plan: &SeedPlan, files: &[String], homes: &Value) -> AreaResult<Packed> {
        let seed = self.opts.seed.clone().ok_or_else(|| HostError::new("Cannot read properties of undefined (reading 'pack')"))?;
        let choice = SeedChoice { files: files.to_vec(), memory: false, commits: false, remember: None };
        let packed = seed.pack(plan.clone(), choice, &homes_of(homes)).await?;
        Ok(Packed { tar: packed.tar, files: packed.files, bytes: packed.bytes, left: packed.left })
    }
    fn folder_named(&self, path: &str) -> String {
        let home = std::env::var("HOME").unwrap_or_default();
        let cwd = std::env::current_dir().map(|dir| dir.to_string_lossy().to_string()).unwrap_or_default();
        wsp_core::types::wiring::folder_named(path, &home, &cwd)
    }
    fn emit(&self, event: Value) {
        match serde_json::from_value::<EventUnion>(event) {
            Ok(event) => {
                let _ = self.bus.emit(event);
            }
            Err(e) => wsp_core::console::warn(&format!("an event of a sub-thread's start was not sent: {e}")),
        }
    }
    fn now(&self) -> f64 {
        self.clock.now() as f64
    }
    fn random_hex(&self, bytes: usize) -> String {
        self.ids.bytes(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
}

/// The agents' homes as the seed wiring takes them: a JSON object of names and paths.
fn homes_of(homes: &Value) -> IndexMap<String, String> {
    homes
        .as_object()
        .map(|object| object.iter().filter_map(|(agent, home)| Some((agent.clone(), home.as_str()?.to_string()))).collect())
        .unwrap_or_default()
}
