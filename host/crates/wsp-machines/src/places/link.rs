// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/link.ts` at 65f6b89c7: the door's half that joins, proves and holds each computer's
//! link, and answers what a link and a place say. Specs: `spec/places_link_door.md` (`answer_challenge` to `load`)
//! and `spec/places_link_reads.md` (`name_of` to the end, and `land_sign_in`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};
use wsp_core::types::pending::DaemonReach;
use wsp_engine::machine::MachineBackend;
use wsp_engine::provision::ProvisionStage;
use wsp_store::keys::Seal;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::views::place::PlaceProvisionRow;
use wsp_wire::views::place_more::{PlaceSettings, PlaceView};
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::machine_link::{BackendFacts, DaemonResponse};
use wsp_wire::wire::place_link::{PlaceAuthRefusal, PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, PlaceReport};

use super::context::PlaceDoorContext;
use super::helpers::{LandedRow, LinkSocket, PendingRecord};
use super::pane_ports::PanePorts;
use super::readings::{PlaceReadings, Reader, Release};
use super::setup::{PlaceSetupArea, SetupRecords};
use super::types::{AuthAnswer, FolderComputer, ForwardAsk, JoinAnswer, PlaceChallenge, PlaceFor, PlaceRecord, PlaceResult, Proved, RowId};

/// A join answered and waiting for its prove (views.ts:319).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joining {
    pub public_key: String,
    pub at: u64,
}

/// What the link door reads off the records area (link.ts:49-52). Slice b's `PlaceRecordsArea` implements it.
#[async_trait(?Send)]
pub trait LinkRecords: SetupRecords {
    /// Every record, sorted by `joinedAt`.
    async fn records(&self) -> PlaceResult<Vec<PlaceRecord>>;
    fn wired_provider(&self) -> Option<String>;
    fn provider_ids(&self) -> Vec<String>;
    fn provider_backend(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>>;
    async fn settings_of(&self, place_id: &str) -> PlaceResult<PlaceSettings>;
    /// The settings held by place id, which `load` fills and the reads answer from.
    fn settings_held(&self) -> Rc<RefCell<IndexMap<String, PlaceSettings>>>;
    /// The `woken` call of every wait in `awaiting` for this place, in order (none taken off the list).
    fn awaiting_for(&self, place_id: &str) -> Vec<Rc<dyn Fn(&str)>>;
    fn hold_back(&self, record: &PlaceRecord);
    async fn default_id(&self) -> PlaceResult<Option<String>>;
    /// Runs `write` in the records' one turn for the default mark (records.ts:283).
    async fn in_turn(&self, write: Box<dyn FnOnce() -> futures::future::LocalBoxFuture<'static, PlaceResult<()>>>) -> PlaceResult<()>;
    async fn mark_default(&self, place_id: &str) -> PlaceResult<()>;
    async fn mark_held(&self) -> PlaceResult<Option<String>>;
    fn challenge(&self, subject: &str, their_nonce: &str, their_ephemeral: Option<&str>) -> Option<PlaceChallenge>;
    fn signed_refusal(&self, place_id: &str, place_nonce: &str, sentence: &str) -> PlaceAuthRefusal;
    async fn write_seen(&self, place_id: &str, at: u64) -> PlaceResult<()>;
    async fn with_cap(&self, row: PlaceView, ids: &[RowId]) -> PlaceResult<PlaceView>;
    async fn row_ids(&self) -> PlaceResult<Vec<RowId>>;
}

/// What the link door reads off the views area (link.ts:55). Slice b's `PlaceViewsArea` implements it.
#[async_trait(?Send)]
pub trait LinkViews {
    async fn pending_records(&self) -> PlaceResult<Vec<PendingRecord>>;
    async fn put_pending(&self, p: PendingRecord) -> PlaceResult<()>;
    /// The pending adds whose floor is running.
    fn flooring(&self) -> Rc<RefCell<IndexSet<String>>>;
    async fn floor_once(&self, pending: PendingRecord, place_id: &str);
    async fn taken_back(&self, pending: PendingRecord);
    fn backend_from(&self, place_id: &str, facts: BackendFacts) -> Arc<dyn MachineBackend>;
    /// Whether the event was a tunnel's bytes this forward owns, taken.
    fn tunnelled(&self, place_id: &str, e: &DaemonEvent) -> bool;
    fn cut(&self, place_id: &str, reason: &str);
    fn joining(&self) -> Rc<RefCell<IndexMap<String, Joining>>>;
    async fn joined(&self, place_id: &str, pending: Joining, req: PlaceProveRequest, expect: &[u8], from: &str, at: u64) -> PlaceResult<Proved>;
    async fn joined_row(&self, record: &PlaceRecord, marked: &str) -> PlaceResult<PlaceView>;
}

/// A channel to one computer's daemon over the link it holds (link.ts:232).
pub struct PlaceChannel {
    state: RefCell<ChannelState>,
}

#[derive(Default)]
struct ChannelState {
    _private: (),
}

/// How a channel ended: the socket's code and reason, or `1000, "closed here"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelGone {
    pub code: u16,
    pub reason: String,
}

impl PlaceChannel {
    /// One frame up the link (`op` and the rest its params), `sys.watch` and `sys.unwatch` held here.
    pub async fn send(self: &Rc<Self>, frame: Map<String, Value>) -> PlaceResult<DaemonResponse> {
        let _ = (frame, &self.state);
        todo!()
    }

    /// Settles once with how the channel ended.
    pub async fn closed(&self) -> ChannelGone {
        todo!()
    }

    pub fn close(self: &Rc<Self>) {
        todo!()
    }
}

/// The link door (link.ts:45). State beyond the context and the setup area is memory only.
pub struct LinkDoor {
    pub ctx: Rc<PlaceDoorContext>,
    pub records: Rc<dyn LinkRecords>,
    pub setup: Rc<PlaceSetupArea>,
    pub views: Rc<dyn LinkViews>,
    pub panes: Rc<PanePorts>,
    pub readings: Rc<PlaceReadings>,
    /// One folder machine per place while its home and PATH stand, keyed `"{home}\0{path}"`.
    folder_machines: RefCell<IndexMap<String, (String, Arc<wsp_engine::place_machine::PlaceFolderMachine>)>>,
    /// The tunnel counter, one per door, from 1.
    tunnel_seq: Cell<u64>,
}

impl LinkDoor {
    pub fn new(ctx: Rc<PlaceDoorContext>, records: Rc<dyn LinkRecords>, setup: Rc<PlaceSetupArea>, views: Rc<dyn LinkViews>) -> Rc<LinkDoor> {
        let _ = (ctx, records, setup, views);
        todo!()
    }

    /// A sign-in finished outside the setup lands the setup's own row for it (link.ts:61).
    pub async fn land_sign_in(self: &Rc<Self>, place_id: &str, row: LandedRow) -> PlaceResult<()> {
        let _ = (place_id, row, &self.folder_machines, &self.tunnel_seq);
        todo!()
    }

    pub fn answer_challenge(&self, subject: &str, nonce: &str, ephemeral: Option<&str>) -> Option<PlaceChallenge> {
        let _ = (subject, nonce, ephemeral);
        todo!()
    }

    /// link.ts:82. `id` draws 8 random bytes through the clock's ids seam.
    pub async fn join(self: &Rc<Self>, req: PlaceJoinRequest, from: &str, at: u64) -> PlaceResult<JoinAnswer> {
        let _ = (req, from, at);
        todo!()
    }

    /// link.ts:102.
    pub async fn auth(self: &Rc<Self>, req: PlaceAuthRequest) -> PlaceResult<AuthAnswer> {
        let _ = req;
        todo!()
    }

    /// link.ts:116.
    pub fn host_key(&self) -> String {
        todo!()
    }

    /// link.ts:120.
    pub async fn prove(self: &Rc<Self>, place_id: &str, req: PlaceProveRequest, expect: &[u8], from: &str, at: u64) -> PlaceResult<Proved> {
        let _ = (place_id, req, expect, from, at);
        todo!()
    }

    /// link.ts:137.
    pub async fn attach(self: &Rc<Self>, place_id: &str, socket: Rc<dyn LinkSocket>, report: PlaceReport, from: &str, at: u64, seal: Option<Seal>) -> PlaceResult<()> {
        let _ = (place_id, socket, report, from, at, seal);
        todo!()
    }

    /// link.ts:228.
    pub fn link(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:230.
    pub async fn watch_sys(self: &Rc<Self>, place_id: &str, f: Reader) -> PlaceResult<Release> {
        let _ = (place_id, f);
        todo!()
    }

    /// link.ts:232.
    pub fn channel(self: &Rc<Self>, place_id: &str, on_event: Rc<dyn Fn(&Map<String, Value>)>) -> Option<Rc<PlaceChannel>> {
        let _ = (place_id, on_event);
        todo!()
    }

    /// link.ts:291.
    pub async fn load(self: &Rc<Self>) -> PlaceResult<()> {
        todo!()
    }

    /// link.ts:317.
    pub fn name_of(&self, place_id: &str) -> String {
        let _ = place_id;
        todo!()
    }

    /// link.ts:319.
    pub fn settings_at(&self, place_id: &str) -> PlaceSettings {
        let _ = place_id;
        todo!()
    }

    /// link.ts:321. `None` no row; `Some(None)` no limit.
    pub fn turn_limit_at(&self, place_id: &str) -> Option<Option<u64>> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:326.
    pub fn threads_at(&self, place_id: &str) -> Option<u64> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:333.
    pub fn machines_at(&self, place_id: &str) -> Option<u64> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:339.
    pub fn sign_ins_at(&self, place_id: &str, agents: Option<&[String]>) -> Option<IndexMap<String, AgentSignInState>> {
        let _ = (place_id, agents);
        todo!()
    }

    /// link.ts:341.
    pub fn vault_at(&self, place_id: &str, vault: &IndexMap<String, String>) -> IndexMap<String, String> {
        let _ = (place_id, vault);
        todo!()
    }

    /// link.ts:345.
    pub fn github_from_vault(&self, place_id: &str) -> bool {
        let _ = place_id;
        todo!()
    }

    /// link.ts:351.
    pub async fn login_landed(self: &Rc<Self>, place_id: &str, agent: &str) -> PlaceResult<()> {
        let _ = (place_id, agent);
        todo!()
    }

    /// link.ts:356.
    pub async fn logins_again(self: &Rc<Self>, place_id: &str) -> PlaceResult<()> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:368.
    pub async fn key_landed(self: &Rc<Self>, agent: &str) -> PlaceResult<()> {
        let _ = agent;
        todo!()
    }

    /// link.ts:373.
    pub async fn gh_there(self: &Rc<Self>, place_id: &str, stage: ProvisionStage) -> PlaceResult<Vec<PlaceProvisionRow>> {
        let _ = (place_id, stage);
        todo!()
    }

    /// link.ts:388.
    pub fn offer_of(&self, place_id: &str) -> Option<String> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:390.
    pub fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:400.
    pub fn joined(&self, place_id: &str) -> bool {
        let _ = place_id;
        todo!()
    }

    /// link.ts:402.
    pub fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:415.
    pub async fn forking_backend(self: &Rc<Self>, place_id: &str) -> PlaceResult<Arc<dyn MachineBackend>> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:452.
    pub async fn forward(self: &Rc<Self>, place_id: &str, place_port: u16, o: ForwardAsk) -> PlaceResult<u16> {
        let _ = (place_id, place_port, o);
        todo!()
    }

    /// link.ts:502.
    pub fn pane_forwards(&self) -> Rc<PanePorts> {
        todo!()
    }

    /// link.ts:504.
    pub async fn place_for(self: &Rc<Self>, word: &str) -> PlaceResult<PlaceFor> {
        let _ = word;
        todo!()
    }

    /// link.ts:522.
    pub async fn default_place(self: &Rc<Self>) -> PlaceResult<PlaceFor> {
        todo!()
    }

    /// link.ts:531.
    pub async fn mark_used(self: &Rc<Self>, place_id: Option<&str>) -> PlaceResult<()> {
        let _ = place_id;
        todo!()
    }

    /// link.ts:535.
    pub async fn mark_default_if_none(self: &Rc<Self>, place_id: &str) -> PlaceResult<()> {
        let _ = place_id;
        todo!()
    }
}
