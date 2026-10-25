// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/setup.ts:246-372, 746-765, 808-922` at 65f6b89c7: one run of a computer's setup or
//! sync. Spec: `spec/places_setup_run.md`. The sign-ins and the folders are this run's other two files.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use indexmap::IndexSet;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PlaceProvisionRow, PlaceSetup, PlaceSetupLine, PlaceSetupStep, PlaceWait};

use super::super::helpers::{LandedRow, SyncJob};
use super::super::types::{HeldApplied, HeldRow, PlaceResult};
use super::{OnlyRow, PlaceSetupArea};

/// What one run is handed (setup.ts:254).
pub struct RunAsk {
    pub place_id: String,
    pub add_id: String,
    pub picks: RecipeFile,
    pub done: IndexSet<PlaceSetupStep>,
    pub started: PlaceSetup,
    pub rerun: Vec<PlaceWait>,
    pub sync: Option<SyncJob>,
    pub only: Option<OnlyRow>,
}

/// One run's state (the closures of `runSetup`). Memory only; built per run.
pub struct SetupRun {
    pub area: Rc<PlaceSetupArea>,
    pub ask: RunAsk,
    /// The rows this run writes, in order; a row is replaced by id.
    pub rows: RefCell<Vec<HeldRow>>,
    pub held: RefCell<PlaceSetup>,
    pub ended: Cell<bool>,
    pub stepped: Cell<bool>,
    pub running: RefCell<IndexSet<PlaceSetupStep>>,
    pub folders_waiting: Cell<u32>,
    pub agent_files_there: Cell<bool>,
    pub github_known: Cell<Option<bool>>,
}

/// Runs the setup or the sync behind whoever asked for it (setup.ts:254). Never fails: a failure ends the run
/// `failed`, a computer that went away leaves it running, a sync that went nowhere reads behind.
pub async fn run_setup(area: Rc<PlaceSetupArea>, ask: RunAsk) {
    let _ = (area, ask);
    todo!()
}

impl SetupRun {
    /// The applied rows as the record keeps them (setup.ts:284).
    pub fn applied(&self) -> HeldApplied {
        todo!()
    }

    /// The setup as it stands, written after every earlier write (setup.ts:297).
    pub fn push(self: &Rc<Self>, next: PlaceSetup) {
        let _ = next;
        todo!()
    }

    /// One line of a step (setup.ts:310).
    pub fn line(self: &Rc<Self>, l: PlaceSetupLine) {
        let _ = l;
        todo!()
    }

    /// One step: marked running, run, its rows kept, marked done or failed (setup.ts:330). `None` where it does not
    /// run this time; `Err` only for a computer that went away.
    pub async fn step(self: &Rc<Self>, name: PlaceSetupStep, work: impl std::future::Future<Output = PlaceResult<Vec<PlaceProvisionRow>>>) -> PlaceResult<Option<(Vec<PlaceProvisionRow>, bool)>> {
        let _ = (name, work);
        todo!()
    }

    /// A row that lands after its step ended (setup.ts:352).
    pub fn land_row(self: &Rc<Self>, r: HeldRow) {
        let _ = r;
        todo!()
    }

    /// Lands a row from outside the run; true once its write is on the record (setup.ts:361).
    pub async fn lander(self: &Rc<Self>, r: LandedRow) -> bool {
        let _ = r;
        todo!()
    }

    /// This run writes no more rows (setup.ts:370).
    pub fn let_go(&self) {
        todo!()
    }

    /// The end of the run (setup.ts:746).
    pub async fn end(self: &Rc<Self>, failed: Option<String>) {
        let _ = failed;
        todo!()
    }
}
