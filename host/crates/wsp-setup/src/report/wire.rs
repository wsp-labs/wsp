// SPDX-License-Identifier: AGPL-3.0-only
//! Packet C1's road: the row of this computer in the places list.
use std::rc::Rc;

use wsp_machines::places::types::HerePlace;

use crate::context::HostWiring;

/// `placeHere` (`place-report.ts`): `PlaceWiring.here`. `None` keeps the row `wsp-host` builds from the host's own name,
/// os and home.
pub fn here(_wiring: &HostWiring) -> Option<Rc<dyn Fn() -> HerePlace>> {
    None
}
