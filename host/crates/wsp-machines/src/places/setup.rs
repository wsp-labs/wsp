// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/setup.ts` at 65f6b89c7: the links' waits, the setup and the sync of a computer to its
//! recipe, and the link a machine there is driven over. Specs: `spec/places_setup_area.md` (this file),
//! `places_setup_run.md` (`setup/run.rs`), `places_setup_sign_ins.md` (`setup/sign_ins.rs`),
//! `places_setup_folders.md` (`setup/folders.rs`).

pub mod folders;
pub mod run;
pub mod sign_ins;

use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use super::helpers::Cancel;
use wsp_core::recipe_sync::RecipeChange;
use wsp_engine::link_backend::MachineLink;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PlaceSetup, PlaceSetupEvent, PlaceSetupStep, PlaceSync, PlaceSyncEvent};
use wsp_words::exit::HostError;

use super::context::PlaceDoorContext;
use super::helpers::LandedRow;
use super::types::{HeldApplied, PlaceRecord, PlaceResult, Resolved};

/// What the setup area reads off the records area (setup.ts:115). Slice b's `PlaceRecordsArea` implements it; the
/// tests drive a fake.
#[async_trait(?Send)]
pub trait SetupRecords {
    async fn record_of(&self, place_id: &str) -> PlaceResult<Option<PlaceRecord>>;
    /// One change of a computer's record as it stands, in its turn (records.ts:349): `None` where `mv` answered `None`
    /// or the record is gone.
    async fn change(&self, place_id: &str, mv: RecordMove) -> PlaceResult<Option<PlaceRecord>>;
    /// A login now standing on that computer, put on its daemon's last list (records.ts:359).
    async fn login_listed(&self, place_id: &str, agent: &str) -> PlaceResult<Option<PlaceRecord>>;
}

/// A move of a record from how it stands to what is written; `None` writes nothing.
pub type RecordMove = Box<dyn for<'a> FnOnce(&'a PlaceRecord) -> Option<PlaceRecord>>;

/// One row of a setup that ended, run again: the step it is under, and the row (setup.ts:84).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnlyRow {
    pub step: PlaceSetupStep,
    pub row: String,
}

/// The steps a run of one row takes with its own (setup.ts:92).
pub fn row_follows(step: PlaceSetupStep) -> &'static [PlaceSetupStep] {
    let _ = step;
    todo!()
}

/// The step a row of a setup is under (setup.ts:95).
pub fn row_step(record: &PlaceRecord, row: &str) -> Option<PlaceSetupStep> {
    let _ = (record, row);
    todo!()
}

/// Whether a run signs an agent in again (setup.ts:105).
pub fn signs_in_again(only: Option<&OnlyRow>, agent: &str, rows: &[wsp_wire::views::place::PlaceProvisionRow]) -> bool {
    let _ = (only, agent, rows);
    todo!()
}

/// A patch of the record's setup fields, written over the record as it stands (setup.ts:193). `sync: Some(None)`
/// removes the sync.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetupPatch {
    pub setup: Option<PlaceSetup>,
    pub applied: Option<HeldApplied>,
    pub picks: Option<RecipeFile>,
    pub recipe: Option<String>,
    pub sync: Option<Option<PlaceSync>>,
}

/// How a start stands the moment it is under way, or the sentence it said instead (setup.ts:927).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Started {
    pub setup: Option<PlaceSetup>,
    pub said: Option<String>,
}

/// Picks given to a start, with the recipe they came from.
#[derive(Clone, Debug, PartialEq)]
pub struct GivenPicks {
    pub picks: RecipeFile,
    pub recipe: Option<String>,
}

/// What moved between what a computer last applied and the recipe it follows (setup.ts:974).
#[derive(Clone, Debug, PartialEq)]
pub struct SyncRead {
    pub resolved: Resolved,
    pub changes: Vec<RecipeChange>,
}

/// A wait for one computer to open a socket again: woken with true for the socket it opened.
pub type Wake = Rc<dyn Fn(bool)>;
/// A reader of one computer's daemon events over a channel.
pub type ChannelReader = Rc<dyn Fn(&serde_json::Map<String, serde_json::Value>)>;
/// A skip of a row a running setup waits on: lands it skipped and stops what waited.
pub type Skipper = Rc<dyn Fn() -> LocalBoxFuture<'static, ()>>;
/// Lands a row from outside the run on the run still writing that computer's rows; true where it landed.
pub type Lander = Rc<dyn Fn(LandedRow) -> LocalBoxFuture<'static, bool>>;

/// A sign-in a setup is following on a computer, by `<place id>/<agent>`.
pub struct SigningIn {
    pub mute: Rc<dyn Fn()>,
    pub leave: Rc<dyn Fn()>,
}

/// The setup area (setup.ts:113). Every map is memory only.
pub struct PlaceSetupArea {
    pub ctx: Rc<PlaceDoorContext>,
    pub records: Rc<dyn SetupRecords>,
    /// The channels a road on this host opened over each computer's link, by place id, then by reader id.
    pub channels: RefCell<IndexMap<String, IndexMap<u64, ChannelReader>>>,
    /// What is waiting for each computer to open a socket again, by place id, then by wake id.
    pub waiting: RefCell<IndexMap<String, IndexMap<u64, Wake>>>,
    /// When each computer's last socket closed under this host, in clock ms.
    pub closed_at: RefCell<IndexMap<String, u64>>,
    pub setting: RefCell<IndexSet<String>>,
    pub syncing: RefCell<IndexSet<String>>,
    pub signing_in: RefCell<IndexMap<String, SigningIn>>,
    pub skippers: RefCell<IndexMap<String, Skipper>>,
    pub landers: RefCell<IndexMap<String, Lander>>,
    /// The computers a change reached while a setup or a sync ran there.
    pub again: RefCell<IndexSet<String>>,
    pub sync_timers: RefCell<IndexMap<String, Cancel>>,
    next_id: std::cell::Cell<u64>,
}

tokio::task_local! {
    /// The computer whose setup's own folder step the running call belongs to (setup.ts:190).
    pub static FOLDERS_OF: String;
}

/// The place the current call runs under as its setup's own folder add, if any.
pub fn current_folders_of() -> Option<String> {
    todo!()
}

/// Runs `work` as the folder add of `place_id`'s own setup.
pub async fn under_folders_of<T>(place_id: &str, work: impl Future<Output = T>) -> T {
    let _ = (place_id, work);
    todo!()
}

impl PlaceSetupArea {
    pub fn new(ctx: Rc<PlaceDoorContext>, records: Rc<dyn SetupRecords>) -> Rc<PlaceSetupArea> {
        let _ = (ctx, records);
        todo!()
    }

    /// Ends every wait on one computer (setup.ts:131).
    pub fn woken(&self, place_id: &str, back: bool) {
        let _ = (place_id, back, &self.next_id);
        todo!()
    }

    /// Waits for that computer to dial in again, up to `until` on the clock (setup.ts:140).
    pub async fn dials_back(self: &Rc<Self>, place_id: &str, until: u64) -> bool {
        let _ = (place_id, until);
        todo!()
    }

    /// The computer the host runs on, by the name the app reads it by (setup.ts:177).
    pub fn here(&self) -> String {
        todo!()
    }

    /// The one busy reading: the sentence a fork or a second setup there is refused with (setup.ts:185).
    pub fn setting_now(&self, record: &PlaceRecord) -> Option<String> {
        let _ = record;
        todo!()
    }

    /// One write of a setup's state onto the record as it stands (setup.ts:193).
    pub async fn write_setup(&self, place_id: &str, patch: SetupPatch) -> PlaceResult<()> {
        let _ = (place_id, patch);
        todo!()
    }

    /// `recipes.changed { slug }` on the stream (setup.ts:202).
    pub fn recipes_moved(&self, slug: &str) {
        let _ = slug;
        todo!()
    }

    /// One move of a computer's sync on the stream (setup.ts:205).
    pub fn sync_frame(&self, event: PlaceSyncEvent) {
        let _ = event;
        todo!()
    }

    /// One frame of a setup on the stream (setup.ts:208).
    pub fn setup_frame(&self, event: PlaceSetupEvent) {
        let _ = event;
        todo!()
    }

    /// Starts the setup on one computer from its picks (setup.ts:927).
    pub async fn start_setup(self: &Rc<Self>, place_id: &str, add_id: &str, given: Option<GivenPicks>, row: Option<&str>) -> PlaceResult<Started> {
        let _ = (place_id, add_id, given, row);
        todo!()
    }

    /// What moved between what a computer last applied and the recipe it follows (setup.ts:974).
    pub async fn sync_of(&self, record: &PlaceRecord, given: Option<Resolved>) -> Option<SyncRead> {
        let _ = (record, given);
        todo!()
    }

    /// A computer out of step reads Behind with what moved; one in step drops the word (setup.ts:985).
    pub async fn mark_sync(&self, place_id: &str, sync: Option<PlaceSync>) -> PlaceResult<()> {
        let _ = (place_id, sync);
        todo!()
    }

    /// One sync of a computer to the recipe it follows, one at a time per computer (setup.ts:996).
    pub async fn sync_place(self: &Rc<Self>, place_id: &str) -> PlaceResult<()> {
        let _ = place_id;
        todo!()
    }

    /// A sync of one computer after `after_ms`, the clock starting again at every ask (setup.ts:1030).
    pub fn sync_soon(self: &Rc<Self>, place_id: &str, after_ms: u64) {
        let _ = (place_id, after_ms);
        todo!()
    }

    /// The start with its refusal as a sentence (setup.ts:1042).
    pub async fn started_or_said(self: &Rc<Self>, place_id: &str, add_id: &str, given: Option<GivenPicks>) -> Started {
        let _ = (place_id, add_id, given);
        todo!()
    }

    /// The road the engine drives one place's machines over (setup.ts:1053). The engine's link is `Send + Sync` and
    /// the door is one thread's, so the link is held in a `send_wrapper::SendWrapper`: it is only ever touched on
    /// the host's own thread, and touching it anywhere else panics.
    pub fn link_to(self: &Rc<Self>, place_id: &str) -> Arc<dyn MachineLink> {
        let _ = place_id;
        todo!()
    }
}

/// What a sign-in's outcome waits on: a `Result` the run turns into a failed row.
pub type SetupResult<T> = Result<T, HostError>;
