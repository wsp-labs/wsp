// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/places/manage.ts` at 65f6b89c7: the door's half a person drives. Three units, one
//! file each: `add` (add, dial, road, exec, adds, reportOf, homeOf; spec `places_manage_add.md`), `remove` (holds,
//! remove, update; `places_manage_remove.md`) and `settings` (list, rows, set, find, pending, choose, setUp, the
//! recipe roads, skip, estimate, setupLog, on, close; `places_manage_settings.md`).
//!
//! What manage reads off places-a's setup area comes through `ManageSetup` (manage.ts:113-116). Where the TS calls
//! `ctx.door.exec`, the port calls `ManageDoor::exec`, which is the door's.

use std::rc::Rc;

use async_trait::async_trait;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PlaceSetup, PlaceSetupEvent, PlaceSync, PlaceSyncEvent};

use super::context::PlaceDoorContext;
use super::records::{LocalFut, PlaceRecordsArea};
use super::types::{PlaceRecord, PlaceResult, Resolved};
use super::views::{LinkReads, PlaceViewsArea, ViewsSetup};

pub mod add;
pub mod remove;
pub mod settings;

/// The picks a setup is started with (manage.ts:310,753).
#[derive(Clone, Debug, PartialEq)]
pub struct SetupPicks {
    pub picks: RecipeFile,
    pub recipe: Option<String>,
}

/// What a started setup answers (setup.ts `startSetup`, `startedOrSaid`): the setup as it stands, or why none started.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetupStarted {
    pub setup: Option<PlaceSetup>,
    pub said: Option<String>,
}

/// What a computer's sync read against its recipe found (setup.ts `syncOf`): the keys of what moved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncRead {
    pub change_keys: Vec<String>,
}

/// What manage reads off places-a's setup area (manage.ts:113-116).
#[async_trait(?Send)]
pub trait ManageSetup: ViewsSetup {
    /// The ids `waiting` holds a wait for (setup.ts:123), in insertion order.
    fn waiting_ids(&self) -> Vec<String>;
    /// `setting.has(id)` (setup.ts:161).
    fn is_setting(&self, place_id: &str) -> bool;
    /// `syncing.has(id)` (setup.ts:164).
    fn is_syncing(&self, place_id: &str) -> bool;
    /// `skippers.get("<id>/<row>")` (setup.ts:171): a live sign-in's skip.
    fn skipper(&self, key: &str) -> Option<Rc<dyn Fn() -> LocalFut<'static, PlaceResult<()>>>>;
    /// `settingNow(record)` (setup.ts:185), the one home for "a computer busy with a setup".
    fn setting_now(&self, record: &PlaceRecord) -> Option<String>;
    /// `recipesMoved(slug)` (setup.ts:202).
    fn recipes_moved(&self, slug: &str);
    /// `syncFrame(event)` (setup.ts:205).
    fn sync_frame(&self, event: PlaceSyncEvent);
    /// `setupFrame(event)` (setup.ts:208).
    fn setup_frame(&self, event: PlaceSetupEvent);
    /// `startSetup(placeId, addId, picks, row)`.
    async fn start_setup(&self, place_id: &str, add_id: &str, picks: Option<SetupPicks>, row: Option<String>) -> PlaceResult<SetupStarted>;
    /// `startedOrSaid(placeId, addId, picks)`.
    async fn started_or_said(&self, place_id: &str, add_id: &str, picks: SetupPicks) -> PlaceResult<SetupStarted>;
    /// Cancels and clears every `syncTimers` entry.
    fn clear_sync_timers(&self);
    /// `syncOf(record, resolved)`.
    async fn sync_of(&self, record: &PlaceRecord, resolved: Option<&Resolved>) -> PlaceResult<Option<SyncRead>>;
    /// `markSync(placeId, sync)`.
    async fn mark_sync(&self, place_id: &str, sync: PlaceSync) -> PlaceResult<()>;
    /// `syncSoon(placeId, afterMs)`.
    fn sync_soon(&self, place_id: &str, after_ms: u64);
}

/// The door's half a person drives (manage.ts:107-872).
pub struct ManageDoor {
    pub ctx: Rc<PlaceDoorContext>,
    pub records: Rc<PlaceRecordsArea>,
    pub setup: Rc<dyn ManageSetup>,
    pub views: Rc<PlaceViewsArea>,
    pub links: Rc<dyn LinkReads>,
}

impl ManageDoor {
    pub fn new(ctx: Rc<PlaceDoorContext>, records: Rc<PlaceRecordsArea>, setup: Rc<dyn ManageSetup>, views: Rc<PlaceViewsArea>, links: Rc<dyn LinkReads>) -> Rc<Self> {
        todo!()
    }
}
