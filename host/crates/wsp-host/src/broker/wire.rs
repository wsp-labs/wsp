// SPDX-License-Identifier: AGPL-3.0-only
//! Packet H1's port and door: the host's broker and the broker door of `serveRuntime`. A host with no broker keeps every
//! turn on today's road, and its door refuses the broker ops with the unserved line.
use std::rc::Rc;

use wsp_core::types::wiring::Broker;
use wsp_serve::serve::BrokerDoor;

use crate::wiring::Wiring;

/// `hostBroker({ statePath, vault }).port` (`cli/wiring.ts:312, 373`): `RuntimeOptions.broker`.
pub fn broker(_wiring: &Wiring) -> Option<Rc<dyn Broker>> {
    None
}

/// `ServeOptions.broker` (`server.ts:589-593`): `hostBroker(...).port.status`, `checkProviderKeyLive` and `cloudSignIn`.
pub fn broker_door(_wiring: &Wiring) -> Option<Rc<dyn BrokerDoor>> {
    None
}
