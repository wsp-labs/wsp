// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-processes.ts at 65f6b89c7: the ops that read or end what a thread runs,
//! Activity's own, and settle and delete, which meet the processes a thread still runs before they take it. Spec:
//! spec/serve_helpers.md, H25.
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve::served::{DynRuntime, Payload};

/// `activity.read`, `activity.end`, `sessions.settle` or `sessions.delete`. serve-processes.ts:9.
pub fn is_processes_request(msg: &RuntimeRequest) -> bool {
    todo!()
}

/// `answerProcesses(rt, msg, origin)`: activity off `activity()`, settle and delete off `sessions()`.
/// serve-processes.ts:11-24.
pub async fn answer_processes(rt: &DynRuntime, msg: &RuntimeRequest, origin: Option<Caller>) -> Result<Payload, HostError> {
    todo!()
}
