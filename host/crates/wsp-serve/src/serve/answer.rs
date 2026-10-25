// SPDX-License-Identifier: AGPL-3.0-only
//! The switch every frame of a socket that is let in goes through (serve.ts:850-878, 1884-1895): the prefix ops,
//! then the op units, then the catch. Spec: spec/serve_ops.md, O1-O3.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use wsp_wire::wire::runtime_ops::RuntimeRequest;

use super::socket::Socket;

/// Whether an op unit answered the frame, or it was not that unit's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handled {
    Answered,
    NotMine,
}

/// Answers one parsed frame of a socket that is let in. `frame` is the frame as it parsed, for the refusal log.
/// Every reply goes through `Socket::answer`; an error any op raises becomes the refusal of spec O2 and tells
/// `opts.failed`.
pub fn answer_frame(socket: &Rc<Socket>, frame: serde_json::Value, msg: RuntimeRequest) -> LocalBoxFuture<'static, ()> {
    todo!()
}
