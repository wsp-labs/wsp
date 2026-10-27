// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 4's roads: the recipe shelf the runtime lists saved recipes from and the `recipes` slot of `serveRuntime`, one
//! shelf for both (`recipeShelf`, `cli/wiring.ts:342`).
use std::rc::Rc;

use wsp_core::types::wiring::RecipeShelf;

use crate::context::HostWiring;

/// `RuntimeOptions.recipes`.
pub fn recipes(_wiring: &HostWiring) -> Option<Rc<dyn RecipeShelf>> {
    None
}

/// `ServeOptions.recipes` (`server.ts:581`).
pub fn recipes_slot(_wiring: &HostWiring) -> Option<Rc<dyn RecipeShelf>> {
    None
}
