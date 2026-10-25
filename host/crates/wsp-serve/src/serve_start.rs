// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-start.ts at 65f6b89c7: a sessions.start off the socket: a fork in its
//! source's folder, else a conversation kept outside wsp where it ran, else the folder of the record, project,
//! branch or folder it names, found or made first. Spec: spec/serve_helpers.md, H10-H15.
use std::rc::Rc;

use wsp_core::types::api::{OutsideOpening, StartOptions};
use wsp_threads::threads::answered_start::{OnHeld, StartedAt};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::SessionStartResult;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve::served::DynRuntime;

/// Each answer of a start, sent under the frame's id.
pub type AnswerFn = Rc<dyn Fn(SessionStartResult)>;

/// `startOptionsOf(msg, at, outside, held)`: each field the client named, copied by hand in the TS's order
/// (spec H15). `msg` is a `sessions.start` request.
pub fn start_options_of(msg: &RuntimeRequest, at: &StartedAt, outside: Option<OutsideOpening>, on_held: Option<OnHeld>) -> StartOptions {
    todo!()
}

/// `serveStart(rt, msg, origin, reply)`. serve-start.ts:38-52. The refusals come first (spec H10-H11), then the
/// fork, else the resume, else the folder; the folder is found by threads' `start_at` and the answers go through its
/// `answered_start`.
pub async fn serve_start(rt: &DynRuntime, msg: &RuntimeRequest, origin: Option<Caller>, reply: AnswerFn) -> Result<(), HostError> {
    todo!()
}
