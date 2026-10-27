// SPDX-License-Identifier: AGPL-3.0-only
//! Packet H4b's door: the init door the app's first-run screen reads.
use std::rc::Rc;

use wsp_core::types::wiring::InitDoor;

use crate::wiring::Wiring;

/// `hostCloudSetup` (`cli/serve.ts:37, 260, 285`) over `cloud-setup.ts:36-62`: `ServeOptions.init` (`server.ts:587`).
pub fn init_door(_wiring: &Wiring) -> Option<Rc<dyn InitDoor>> {
    None
}
