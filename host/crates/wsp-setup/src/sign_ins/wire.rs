// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 8's ports: the agents reader, its acts, and the cloud sign-in the broker door answers `agents.cloudSignIn` with.
use std::rc::Rc;

use wsp_core::sign_in_socket::CloudSignIn;
use wsp_machines::agents_read::{AgentsActs, AgentsReader};

use crate::context::HostWiring;

/// `agentsReader({ vault, loginEnv, latest, signInStores })` (`cli/wiring.ts:352`).
pub fn agents_reader(_wiring: &HostWiring) -> Option<Rc<dyn AgentsReader>> {
    None
}

/// `hostActs({ vaultFile, home, wspServer })` (`agents-signin.ts:362-388`, `cli/wiring.ts:355`).
pub fn agents_acts(_wiring: &HostWiring) -> Option<Rc<dyn AgentsActs>> {
    None
}

/// `cloudSignIn` (`agents-signin.ts:354`): the broker door's `cloud_sign_in`.
pub fn cloud_sign_in(_wiring: &HostWiring) -> Option<CloudSignIn> {
    None
}
