// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/places/records.ts` at 65f6b89c7: the place records as the store keeps them, the
//! providers beside them, and the turns every write of one takes. Spec: `spec/places_records.md` (rules R1-R34).
//!
//! One thread: state is `RefCell`, no borrow is held across an `.await` or a call out (spec 03 rule 3.3), and every
//! map is an `IndexMap` with `shift_remove` (rule 3.11).

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use wsp_engine::machine::MachineBackend;
use wsp_store::keys::PlaceKeyPair;
use wsp_wire::views::place::{PlaceAddJob, PlaceBack};
use wsp_wire::views::place_more::{CloudLimits, PlaceSettings, PlaceView};
use wsp_wire::wire::capabilities::MachineSizeOffer;
use wsp_wire::wire::place_link::PlaceAuthRefusal;

use super::context::PlaceDoorContext;
use super::types::{PlaceChallenge, PlaceLogin, PlaceRecord, PlaceResult, RowId, SudoVerb};

/// A boxed local future: what a chain runs.
pub type LocalFut<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// One install waiting on its computer to dial in (records.ts:163), keyed by the code it handed that computer. `join`
/// (views V17) writes `place_id`; `add` (manage A9) writes `login`, `back` and `woken`.
#[derive(Default)]
pub struct AwaitingInstall {
    pub place_id: Option<String>,
    pub login: Option<PlaceLogin>,
    pub back: Option<PlaceBack>,
    pub woken: Option<Box<dyn FnOnce(String)>>,
}

/// What `reached_over` read (records.ts:223, R22).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reached {
    Same,
    Other { other: Option<String> },
    Unread { unread: String, refused: bool },
}

/// A FIFO chain of the first form (spec 03 table, "chain"): each run starts after the one before it settles, its
/// caller gets its own result, and the tail swallows a failure so the next run still goes (records.ts:282-287).
pub struct Chain {
    tail: RefCell<Option<Rc<dyn std::any::Any>>>,
}

impl Chain {
    pub fn new() -> Self {
        todo!()
    }

    /// Runs `write` in its turn and answers what it answered.
    pub async fn run<T: 'static>(&self, write: LocalFut<'_, T>) -> T {
        todo!()
    }
}

impl Default for Chain {
    fn default() -> Self {
        Self::new()
    }
}

/// One `Chain` per key, each dropped once its last run settles and no later run joined (records.ts:336-345).
pub struct KeyedChain {
    turns: RefCell<IndexMap<String, Rc<Chain>>>,
}

impl KeyedChain {
    pub fn new() -> Self {
        todo!()
    }

    pub async fn run<T: 'static>(&self, key: &str, write: LocalFut<'_, T>) -> T {
        todo!()
    }

    /// The keys with a run in flight or queued, for the tests that pin R25's cleanup.
    pub fn keys(&self) -> Vec<String> {
        todo!()
    }
}

impl Default for KeyedChain {
    fn default() -> Self {
        Self::new()
    }
}

/// The records area (records.ts:39-373).
pub struct PlaceRecordsArea {
    pub ctx: Rc<PlaceDoorContext>,
    /// R3: what `settingsAt` answers; `load`, `set` and `forget` write it.
    pub settings_held: RefCell<IndexMap<String, PlaceSettings>>,
    /// R12: the last limits read per cloud, and the clouds with a read out.
    pub limits_kept: RefCell<IndexMap<String, CloudLimits>>,
    pub limits_asking: RefCell<IndexSet<String>>,
    /// R15.
    pub awaiting: RefCell<IndexMap<String, Rc<RefCell<AwaitingInstall>>>>,
    /// R16: the adds over ssh, oldest first.
    pub adds: RefCell<IndexMap<String, PlaceAddJob>>,
    /// R30: the mark's chain, which `set` and `forget` also take.
    pub marking: Chain,
    /// R25: the record turns.
    pub record_turns: KeyedChain,
}

impl PlaceRecordsArea {
    pub fn new(ctx: Rc<PlaceDoorContext>) -> Rc<Self> {
        todo!()
    }

    /// R1 (:42).
    pub async fn records(&self) -> PlaceResult<Vec<PlaceRecord>> {
        todo!()
    }

    /// R4 (:46).
    pub fn wired_provider(&self) -> Option<String> {
        todo!()
    }

    /// R5 (:50-56).
    pub fn provider_ids(&self) -> Vec<String> {
        todo!()
    }

    /// R6 (:58).
    pub fn provider_backend(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        todo!()
    }

    /// R7 (:61-65).
    pub fn provider_rate(&self, place_id: &str) -> Option<f64> {
        todo!()
    }

    /// R8 (:69).
    pub fn provider_sizes(&self, place_id: &str) -> Vec<MachineSizeOffer> {
        todo!()
    }

    /// R9 (:72-78): sets `buildsImages`, `build` and `buildStopped` on `row` as the rule says, nothing else.
    pub fn image_facts(&self, row: &mut PlaceView, place_id: &str, backend: Option<&Arc<dyn MachineBackend>>) {
        todo!()
    }

    /// R2 (:79-82).
    pub async fn record_of(&self, place_id: &str) -> PlaceResult<Option<PlaceRecord>> {
        todo!()
    }

    /// R3 (:83-86).
    pub async fn settings_of(&self, place_id: &str) -> PlaceResult<PlaceSettings> {
        todo!()
    }

    /// R10 (:90-94).
    pub async fn row_ids(&self) -> PlaceResult<Vec<RowId>> {
        todo!()
    }

    /// R11 (:96-115).
    pub async fn with_cap(&self, row: PlaceView, ids: &[RowId]) -> PlaceResult<PlaceView> {
        todo!()
    }

    /// R12 (:122-138). Synchronous; the read it starts is a spawned local task.
    pub fn with_limits(self: &Rc<Self>, row: PlaceView) -> PlaceView {
        todo!()
    }

    /// R13 (:141-145): sets `size` and `sizeDefault` on `row`.
    pub fn size_facts(&self, row: &mut PlaceView, place_id: &str, settings: &PlaceSettings) {
        todo!()
    }

    /// R14 (:148-158).
    pub async fn until_daemon_version(&self, place_id: &str, from: u32, wait_ms: u64) -> PlaceResult<u32> {
        todo!()
    }

    /// R16 (:167-173).
    pub fn put_add(&self, add_id: &str, next: impl FnOnce(PlaceAddJob) -> PlaceAddJob) {
        todo!()
    }

    /// R17 (:177-189). `install` is the in-flight install's login and back, where one is.
    pub fn with_road(record: PlaceRecord, install: Option<(&Option<PlaceLogin>, &Option<PlaceBack>)>) -> PlaceRecord {
        todo!()
    }

    /// R19 (:194-197).
    pub fn login_of(record: &PlaceRecord) -> Option<PlaceLogin> {
        todo!()
    }

    /// R20 (:201-202).
    pub async fn root_over(&self, login: &PlaceLogin, sudo_password: Option<&str>, verb: SudoVerb, name: &str) -> PlaceResult<Option<String>> {
        todo!()
    }

    /// R21 (:209-217).
    pub async fn login_answers(&self, login: &PlaceLogin) -> bool {
        todo!()
    }

    /// R22 (:223-240).
    pub async fn reached_over(&self, record: &PlaceRecord, login: &PlaceLogin, sudo_password: Option<&str>) -> PlaceResult<Reached> {
        todo!()
    }

    /// R18 (:244-247): the login and back of the first install whose join became `place_id`.
    pub fn road_of_install(&self, place_id: &str) -> Option<(Option<PlaceLogin>, Option<PlaceBack>)> {
        todo!()
    }

    /// R23 (:251-265).
    pub fn hold_back(self: &Rc<Self>, record: &PlaceRecord) {
        todo!()
    }

    /// R24 (:271-276).
    pub async fn keep(&self, record: PlaceRecord) -> PlaceResult<PlaceRecord> {
        todo!()
    }

    /// R29 (:277-280).
    pub async fn default_id(&self) -> PlaceResult<Option<String>> {
        todo!()
    }

    /// R30 (:282-287).
    pub async fn in_turn<T: 'static>(&self, write: LocalFut<'_, T>) -> T {
        todo!()
    }

    /// R31 (:288).
    pub async fn mark_default(&self, place_id: &str) -> PlaceResult<()> {
        todo!()
    }

    /// R32 (:290-294).
    pub async fn mark_held(&self) -> PlaceResult<Option<String>> {
        todo!()
    }

    /// R33 (:303-321).
    pub fn challenge(host_key: &PlaceKeyPair, subject: &str, their_nonce: &str, their_ephemeral: Option<&str>) -> Option<PlaceChallenge> {
        todo!()
    }

    /// R34 (:325-328).
    pub fn signed_refusal(host_key: &PlaceKeyPair, place_id: &str, place_nonce: &str, sentence: &str) -> PlaceAuthRefusal {
        todo!()
    }

    /// R27 (:330-332).
    pub async fn write_seen(&self, place_id: &str, at: i64) -> PlaceResult<()> {
        todo!()
    }

    /// R25 (:336-345).
    pub async fn in_record_turn<T: 'static>(&self, place_id: &str, write: LocalFut<'_, T>) -> T {
        todo!()
    }

    /// R26 (:349-354), runtime invariant 6: the one road a record changes by.
    pub async fn change(&self, place_id: &str, mv: impl FnOnce(PlaceRecord) -> Option<PlaceRecord>) -> PlaceResult<Option<PlaceRecord>> {
        todo!()
    }

    /// R28 (:359-365).
    pub async fn login_listed(&self, place_id: &str, agent: &str) -> PlaceResult<Option<PlaceRecord>> {
        todo!()
    }
}
