// SPDX-License-Identifier: AGPL-3.0-only
//! The identity re-asks, the person's computers, recipes, the account, devices and tickets. serve.ts:396-418,
//! 863-868, 880-1169. Spec: spec/serve_places.md.
use std::rc::Rc;

use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use super::answer::Handled;
use super::socket::Socket;

/// Answers the ops of this unit and sends their replies; `NotMine` for any other op. An error is the switch's to
/// send (spec O2).
pub async fn answer_places(socket: &Rc<Socket>, frame: &serde_json::Value, msg: &RuntimeRequest, origin: Option<Caller>) -> Result<Handled, HostError> {
    todo!()
}

/// `refusedOffOwnRoad()`: a socket off the person's own road is sent the ticket refusal and the op runs nothing
/// (serve.ts:864-868). Answers whether it refused.
pub fn refused_off_own_road(socket: &Rc<Socket>, frame: &serde_json::Value, msg: &RuntimeRequest) -> bool {
    todo!()
}
