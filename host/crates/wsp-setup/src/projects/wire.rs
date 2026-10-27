// SPDX-License-Identifier: AGPL-3.0-only
//! Packet H5's port: the seed half of an add on this computer.
use std::rc::Rc;

use wsp_core::types::wiring::SeedWiring;

use crate::context::HostWiring;

/// `hostSeed()` (`cli/wiring.ts:368-387`): `RuntimeOptions.seed`.
pub fn seed(_wiring: &HostWiring) -> Option<Rc<dyn SeedWiring>> {
    None
}
