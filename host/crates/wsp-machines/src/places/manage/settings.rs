// SPDX-License-Identifier: AGPL-3.0-only
//! manage.ts:122-145,425-492,722-871 at 65f6b89c7: list, rows, set, find, pending, choose, setUp, the recipe roads,
//! skip, estimate, setupLog, picksOf, on and close. Spec: `spec/places_manage_settings.md` (rules L1-L3, S1-S10,
//! F1-F6, P1-P3, SK1-SK4, ES1, LG1, C1-C2).

use std::rc::Rc;

use indexmap::IndexMap;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PendingComputer, PlaceSetAlso, PlaceSetupStep};
use wsp_wire::views::place_more::{PlaceSettingWord, PlaceSettingsAsk, PlaceView};
use wsp_wire::views::place_setup_words::PlaceEstimate;

use super::ManageDoor;
use crate::places::types::{PlaceEvent, PlaceRecord, PlaceResult, PlaceSetUp, SetUpAsk};

impl ManageDoor {
    /// F1 (:124-135).
    pub async fn follow_to(&self, place_id: &str, recipe: &str) -> PlaceResult<Option<PlaceRecord>> {
        todo!()
    }

    /// S5 (:140-145).
    pub async fn same_computer_over(&self, record: &PlaceRecord, ssh: &str) -> PlaceResult<()> {
        todo!()
    }

    /// L1 (:425-435).
    pub async fn list(self: &Rc<Self>, now: i64) -> PlaceResult<Vec<PlaceView>> {
        todo!()
    }

    /// L2 (:437).
    pub async fn rows(&self) -> PlaceResult<Vec<PlaceView>> {
        todo!()
    }

    /// S1-S10 (:439-492).
    pub async fn set(self: &Rc<Self>, place_id: &str, set: PlaceSettingsAsk, reset: Vec<PlaceSettingWord>, also: PlaceSetAlso) -> PlaceResult<PlaceView> {
        todo!()
    }

    /// L3 (:722).
    pub async fn find(&self, reference: &str) -> PlaceResult<Vec<PlaceRecord>> {
        todo!()
    }

    /// P1 (:724).
    pub async fn pending(&self) -> PlaceResult<Vec<PendingComputer>> {
        todo!()
    }

    /// P2 (:726-733).
    pub async fn choose(&self, reference: &str, choices: RecipeFile, recipe: Option<String>) -> PlaceResult<PendingComputer> {
        todo!()
    }

    /// P3 (:735-755).
    pub async fn set_up(&self, reference: &str, o: SetUpAsk) -> PlaceResult<PlaceSetUp> {
        todo!()
    }

    /// F2 (:757-761).
    pub async fn follow(&self, place_id: &str, recipe: &str) -> PlaceResult<PlaceView> {
        todo!()
    }

    /// F3 (:763-781).
    pub async fn recipe_changed(&self, slug: &str, after_ms: u64) -> PlaceResult<Vec<String>> {
        todo!()
    }

    /// F4 (:783-787).
    pub async fn followers(&self) -> PlaceResult<IndexMap<String, Vec<String>>> {
        todo!()
    }

    /// SK1-SK4 (:789-813).
    pub async fn skip(&self, place_id: &str, row: &str) -> PlaceResult<PlaceView> {
        todo!()
    }

    /// ES1 (:815-824).
    pub async fn estimate(&self, reference: &str, choices: RecipeFile) -> PlaceResult<PlaceEstimate> {
        todo!()
    }

    /// LG1 (:826-837).
    pub async fn setup_log(&self, place_id: &str, step: Option<PlaceSetupStep>) -> PlaceResult<Vec<String>> {
        todo!()
    }

    /// F5 (:839-847).
    pub async fn unfollow(&self, slug: &str) -> PlaceResult<Vec<String>> {
        todo!()
    }

    /// F6 (:849).
    pub async fn picks_of(&self, place_id: &str) -> PlaceResult<Option<RecipeFile>> {
        todo!()
    }

    /// C1 (:851-854): answers the id `off` takes.
    pub fn on(&self, f: Rc<dyn Fn(&PlaceEvent)>) -> u64 {
        todo!()
    }

    /// C1.
    pub fn off(&self, id: u64) {
        todo!()
    }

    /// C2 (:856-870).
    pub async fn close(&self) -> PlaceResult<()> {
        todo!()
    }
}
