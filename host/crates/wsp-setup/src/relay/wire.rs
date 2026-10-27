// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 7's roads: the callback relay, handed to the `forwards` slot of `serveRuntime` and to the agents' sign-in and
//! forward ports (`startCallbackRelay`, `server.ts:399-407`).
use std::rc::Rc;

use wsp_machines::agents_read::{CallbackForwards, SignInForward};
use wsp_serve::serve::ForwardsSource;

use crate::context::HostWiring;

/// `ServeOptions.forwards` (`server.ts:575`).
pub fn forwards_slot(_wiring: &HostWiring) -> Option<Rc<dyn ForwardsSource>> {
    None
}

/// `openSignIn` (`relay.ts:1080-1099`): the sign-in's forward.
pub fn sign_in_forward(_wiring: &HostWiring) -> Option<Rc<dyn SignInForward>> {
    None
}

/// `rt.agents.forwards` (`relay.ts:1178`).
pub fn callback_forwards(_wiring: &HostWiring) -> Option<Rc<dyn CallbackForwards>> {
    None
}
