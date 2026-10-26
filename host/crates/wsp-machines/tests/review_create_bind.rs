// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-machines at 2c4f6cb46: the create's `bind` attaches a copy of the record, so what the fork writes after
//! it never reaches the live entry or the stored record.

mod common;
#[path = "support/create.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::Value;
use support::{FakeRuntime, World};
use wsp_core::types::internal::KindModule;
use wsp_core::types::pending::Clock;
use wsp_core::types::wiring::{StageReport, WorkspaceRecord};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::machines::create::{
    CreateArea, CreateRuntime, KeptBuilder, LandingRoad, RecordBranchAsk,
};
use wsp_machines::machines::machines::{Entry, Ids};
use wsp_machines::places::types::PlaceDoor;
use wsp_wire::views::workspace::{BranchOfRecord, ProjectView};
use wsp_wire::views::workspace_more::{ProjectRef, ThreadScope, WorkspaceAgents, WorkspaceView};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

/// The create scenes' runtime, whose `fork` then does what `MachinesArea::fork` does after `bind` (machines.ts:331-338,
/// create.rs `fork_claimed`): it writes `npmBin` and `screen` onto the record it was handed.
struct AfterBind(Rc<FakeRuntime>);

#[async_trait(?Send)]
impl CreateRuntime for AfterBind {
    fn clock(&self) -> Rc<dyn Clock> {
        self.0.clock()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.0.ids()
    }
    fn emit(&self, event: EventUnion) -> Result<(), HostError> {
        self.0.emit(event)
    }
    fn warn(&self, line: &str) {
        self.0.warn(line)
    }
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>> {
        self.0.live()
    }
    fn wired(&self) -> String {
        self.0.wired()
    }
    fn place_door(&self) -> Option<Rc<dyn PlaceDoor>> {
        self.0.place_door()
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        self.0.landing_place(computer).await
    }
    async fn place_refuses(&self, place_id: Option<&str>) -> Result<(), HostError> {
        self.0.place_refuses(place_id).await
    }
    async fn landing_backend(
        &self,
        place_id: Option<&str>,
    ) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.0.landing_backend(place_id).await
    }
    fn landing_kind(&self, computer: &str, at: &Arc<dyn MachineBackend>) -> String {
        self.0.landing_kind(computer, at)
    }
    async fn landing_deps(&self, computer: &str) -> Result<Value, HostError> {
        self.0.landing_deps(computer).await
    }
    fn project_landing(&self, kind: &str) -> Result<Rc<dyn LandingRoad>, HostError> {
        self.0.project_landing(kind)
    }
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> {
        self.0.remember_project(project).await
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        self.0.project_held(project_id)
    }
    fn keeps_images(&self, at: &Arc<dyn MachineBackend>) -> bool {
        self.0.keeps_images(at)
    }
    async fn copy_for_fork(
        &self,
        golden: &str,
        place_id: Option<&str>,
        building: &dyn Fn(&str, f64),
    ) -> Result<String, HostError> {
        self.0.copy_for_fork(golden, place_id, building).await
    }
    async fn image_head(&self) -> Result<String, HostError> {
        self.0.image_head().await
    }
    async fn image_size(&self, golden: &str) -> Result<Option<WorkspaceSize>, HostError> {
        self.0.image_size(golden).await
    }
    fn brokered_start(&self, provider: bool) -> bool {
        self.0.brokered_start(provider)
    }
    async fn named_of_record(&self, ask: RecordBranchAsk) -> Result<BranchOfRecord, HostError> {
        self.0.named_of_record(ask).await
    }
    fn place_name(&self, place: &str) -> String {
        self.0.place_name(place)
    }
    fn place_door_name(&self, place: &str) -> String {
        self.0.place_door_name(place)
    }
    fn spawn_at(&self, place: &str) -> WorkspaceAgents {
        self.0.spawn_at(place)
    }
    fn tree_of(&self, spawned: Option<&ThreadScope>) -> serde_json::Map<String, Value> {
        self.0.tree_of(spawned)
    }
    fn attach(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>) -> Entry {
        self.0.attach(record, machine)
    }
    async fn fork(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        bind: &dyn for<'m, 'r> Fn(&'m Arc<dyn Machine>, &'r WorkspaceRecord),
        report: StageReport,
    ) -> Result<Arc<dyn Machine>, HostError> {
        let machine = self.0.fork(record, bind, report).await?;
        record.borrow_mut().npm_bin = Some("/opt/npm/bin".into());
        record.borrow_mut().screen = serde_json::from_value(
            serde_json::json!({ "streamUrl": "https://screen.example/m-new" }),
        )
        .ok();
        Ok(machine)
    }
    async fn refresh_builders(&self) -> Result<(), HostError> {
        self.0.refresh_builders().await
    }
    fn builders(&self) -> Vec<KeptBuilder> {
        self.0.builders()
    }
    fn cancel_grace(&self, builder_id: &str) {
        self.0.cancel_grace(builder_id)
    }
    async fn kill_builder(&self, builder_id: &str) -> Result<(), HostError> {
        self.0.kill_builder(builder_id).await
    }
    async fn forget_builder(&self, builder_id: &str) -> Result<(), HostError> {
        self.0.forget_builder(builder_id).await
    }
    fn holds_slot(&self, record: &WorkspaceRecord) -> bool {
        self.0.holds_slot(record)
    }
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String> {
        self.0.served_by_its_computer(entry)
    }
    fn daemon_answers_ms(&self, entry: &Entry) -> Result<u64, HostError> {
        self.0.daemon_answers_ms(entry)
    }
    async fn daemon_reach(&self, entry: &Entry) -> Result<(), HostError> {
        self.0.daemon_reach(entry).await
    }
    async fn ping_daemon(&self, entry: &Entry) -> Option<String> {
        self.0.ping_daemon(entry).await
    }
    fn sync_daemon(&self, entry: &Entry) {
        self.0.sync_daemon(entry)
    }
    async fn read_memory(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        machine: &Arc<dyn Machine>,
        sizes: &[WorkspaceSize],
    ) -> bool {
        self.0.read_memory(record, machine, sizes).await
    }
    fn module_of(&self, kind: &str) -> Result<Rc<dyn KindModule>, HostError> {
        self.0.module_of(kind)
    }
    async fn write_daemon_roots(&self, entry: &Entry) -> Result<(), HostError> {
        self.0.write_daemon_roots(entry).await
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.0.persist(record).await
    }
    async fn mark_used(&self, place_id: Option<&str>) -> Result<(), HostError> {
        self.0.mark_used(place_id).await
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        self.0.view(record)
    }
    fn ref_of(&self, project: &ProjectView) -> ProjectRef {
        self.0.ref_of(project)
    }
}

/// create.ts:104-109 and machines.ts:321-338: `bind` attaches the record the fork was handed, and the fork then writes
/// `npmBin`, `shape`, `size` and `screen` on that same object, so the live entry and the record `persist` stores carry
/// what the provider built. The Rust `bind` (create.rs, `let mut copy = r.clone()`) attaches a clone taken at bind
/// time, and the create persists the clone.
#[test]
fn what_the_fork_writes_after_bind_reaches_the_live_entry_and_the_stored_record() {
    common::run_local(async {
        let fixture = common::fixture("create");
        let sc = common::scene(&fixture, "a provider fork lands and is ready");
        let w = World::new(&sc["scene"]);
        let area = CreateArea::new(Rc::new(AfterBind(w.rt.clone())) as Rc<dyn CreateRuntime>);
        *w.rt.area.borrow_mut() = Rc::downgrade(&area);
        let o = support::options_of(sc["scene"].get("o").unwrap_or(&serde_json::json!({})));
        area.create_staged(
            &o,
            &w.rt.project,
            "ws_1",
            support::logging_report(&w.log),
            None,
            None,
            None,
        )
        .await
        .expect("the scene's create lands");
        let persisted = w
            .log
            .take()
            .into_iter()
            .rev()
            .find(|e| e[0] == "persist")
            .expect("the create persisted its record");
        assert_eq!(
            persisted[1]["npmBin"], "/opt/npm/bin",
            "the stored record keeps what the fork wrote: {}",
            persisted[1]
        );
        assert_eq!(
            persisted[1]["screen"]["streamUrl"],
            "https://screen.example/m-new"
        );
        let entry = w.entry("ws_1").expect("the create's entry is live");
        let live = entry.borrow().record.borrow().clone();
        assert_eq!(
            live.npm_bin.as_deref(),
            Some("/opt/npm/bin"),
            "the live entry's record is the one the fork wrote"
        );
    })
}
