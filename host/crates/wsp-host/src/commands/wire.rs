// SPDX-License-Identifier: AGPL-3.0-only
//! Packet H4a's door: how the host restarts itself.
use std::rc::Rc;

use wsp_serve::serve::RestartDoor;

use crate::wiring::Wiring;

/// `restartRoads` (`restart.ts:54`, made at `cli/serve.ts:238-251`): `ServeOptions.restart` (`server.ts:598`). A host
/// with no door answers `host.restart` with the no-restart line.
pub fn restart_door(_wiring: &Wiring) -> Option<Rc<dyn RestartDoor>> {
    None
}
