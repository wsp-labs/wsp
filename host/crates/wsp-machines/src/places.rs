// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places.ts` (the door's assembly, below the module list) and `places/`.

pub mod context;
pub mod helpers;
pub mod link;
pub mod manage;
pub mod pane_ports;
pub mod provider_remove;
pub mod readings;
pub mod records;
pub mod setup;
pub mod types;
pub mod views;

use std::rc::Rc;

use types::{PlaceDoor, PlaceDoorOptions};

/// `makePlaceDoor` (places.ts:38): the context, then the areas in the TS order (records, setup, views, then the link
/// and manage halves), then the door set on the context. Neither slice's list names places.ts; this is its signature
/// alone, for whichever writer takes it.
pub fn make_place_door(opts: PlaceDoorOptions) -> Rc<dyn PlaceDoor> {
    let _ = opts;
    todo!()
}
