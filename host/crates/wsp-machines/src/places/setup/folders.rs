// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/setup.ts:633-744, 767-806` at 65f6b89c7: the folders, their servers, and a sync's
//! removals. Spec: `spec/places_setup_folders.md`.

use std::rc::Rc;

use indexmap::IndexMap;
use wsp_wire::types::recipe_file::RecipeFileFoldersValue;
use wsp_wire::views::place::PlaceProvisionRow;
use wsp_words::exit::HostError;

use super::super::types::{FolderMove, HeldRow};
use super::run::SetupRun;

impl SetupRun {
    /// Whether a folder's repository clones there only with GitHub signed in (setup.ts:635).
    pub async fn needs_github(self: &Rc<Self>, folder: &RecipeFileFoldersValue) -> bool {
        let _ = folder;
        todo!()
    }

    /// One folder made a project there by the add's own road (setup.ts:645).
    pub async fn add_folder(self: &Rc<Self>, key: &str, folder: &RecipeFileFoldersValue, mv: Option<&FolderMove>) -> HeldRow {
        let _ = (key, folder, mv);
        todo!()
    }

    /// One landed folder's own servers carried into its project there (setup.ts:674).
    pub async fn carry_servers(self: &Rc<Self>, key: &str, row: &HeldRow) -> Vec<PlaceProvisionRow> {
        let _ = (key, row);
        todo!()
    }

    /// The folderServers step (setup.ts:689).
    pub async fn folder_servers(self: &Rc<Self>) -> Vec<PlaceProvisionRow> {
        todo!()
    }

    /// The folders step (setup.ts:698).
    pub async fn folders(self: &Rc<Self>) -> Result<Vec<HeldRow>, HostError> {
        todo!()
    }

    /// The folder each agent's threads there are pointed at that the login reaches (setup.ts:769).
    pub async fn stores_here(self: &Rc<Self>) -> Option<IndexMap<String, String>> {
        todo!()
    }

    /// A sync's removals, before anything is planned (setup.ts:777).
    pub async fn undo(self: &Rc<Self>) -> Result<(), HostError> {
        todo!()
    }
}
