// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 6's road: the provisioner a computer's setup runs its steps through.
use std::rc::Rc;

use wsp_machines::places::types::PlaceProvisioner;

use crate::context::HostWiring;

/// `placeProvisioner` (`place-provision.ts:357`): `PlaceWiring.provision`.
pub fn provision(_wiring: &HostWiring) -> Option<Rc<dyn PlaceProvisioner>> {
    None
}
