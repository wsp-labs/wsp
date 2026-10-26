// SPDX-License-Identifier: AGPL-3.0-only
//! Events, status, the workspace verbs, daemon channels, readings and exec. serve.ts:369-373, 1170-1473, 1634-1678.
//! Spec: spec/serve_workspaces.md.
use std::rc::Rc;

use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use super::answer::Handled;
use super::socket::Socket;

/// The length in bytes of a daemon channel's id and an exec's id before they are written in lower hex. serve.ts:1408, 1658.
pub const CHANNEL_ID_BYTES: usize = 6;
/// What `daemon.send` and `daemon.close` fail with for an id this socket did not open. serve.ts:1455, 1462.
pub const NO_SUCH_CHANNEL: &str = "no such daemon channel on this socket";

/// Answers the ops of this unit and sends their replies and pushes; `NotMine` for any other op.
pub async fn answer_workspaces(
    socket: &Rc<Socket>,
    frame: &serde_json::Value,
    msg: &RuntimeRequest,
    origin: Option<Caller>,
) -> Result<Handled, HostError> {
    todo!()
}
