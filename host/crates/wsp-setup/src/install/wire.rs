// SPDX-License-Identifier: AGPL-3.0-only
//! Packet C3's roads: the nine roads of `placeWiring` (`places.ts:31-64`) that add, dial, log, update, leave, sudo, run
//! over, undo and fetch a token for a computer over ssh, and the reverse forward `back`.
use std::rc::Rc;

use wsp_machines::places::types::{
    PlaceBackHolder, PlaceDialler, PlaceGithubToken, PlaceInstaller, PlaceLeaver, PlaceLogReader, PlaceRunOver, PlaceSudo, PlaceUndoer,
    PlaceUpdater,
};

use crate::context::HostWiring;

/// `placeInstaller` (`places/install.ts:30`).
pub fn install(_wiring: &HostWiring) -> Option<Rc<dyn PlaceInstaller>> {
    None
}

/// The dial `placeWiring` hands the runtime (`places.ts`).
pub fn dial(_wiring: &HostWiring) -> Option<Rc<dyn PlaceDialler>> {
    None
}

/// `placeLogReader` (`places/install.ts:571`).
pub fn log(_wiring: &HostWiring) -> Option<Rc<dyn PlaceLogReader>> {
    None
}

/// `placeUpdater` (`places/install.ts:343`).
pub fn update(_wiring: &HostWiring) -> Option<Rc<dyn PlaceUpdater>> {
    None
}

/// `placeLeaver` (`places/install.ts:452`).
pub fn leave(_wiring: &HostWiring) -> Option<Rc<dyn PlaceLeaver>> {
    None
}

/// `placeSudoReader` (`places/install.ts:490`).
pub fn sudo_over(_wiring: &HostWiring) -> Option<Rc<dyn PlaceSudo>> {
    None
}

/// `placeRunner` (`places/install.ts:471`).
pub fn run_over(_wiring: &HostWiring) -> Option<Rc<dyn PlaceRunOver>> {
    None
}

/// `placeUndoer` (`places/install.ts:537`).
pub fn undo(_wiring: &HostWiring) -> Option<Rc<dyn PlaceUndoer>> {
    None
}

/// `vaultGitHubToken` (`places/install.ts`).
pub fn github_token(_wiring: &HostWiring) -> Option<Rc<dyn PlaceGithubToken>> {
    None
}

/// `placeBackHolder` (`place-back.ts:148`): `PlaceWiring.back`.
pub fn back(_wiring: &HostWiring) -> Option<Rc<dyn PlaceBackHolder>> {
    None
}
