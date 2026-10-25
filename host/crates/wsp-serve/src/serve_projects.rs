// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-projects.ts at 65f6b89c7: the project ops a socket asks, answered off the
//! runtime's projects, and the computers a thread's own token is answered. Spec: spec/serve_helpers.md, H20-H21.
use wsp_core::types::api::ProjectsApi;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve::served::{Payload, ServedRuntime};

/// `msg.op.startsWith("projects.")`.
pub fn is_project_request(msg: &RuntimeRequest) -> bool {
    todo!()
}

/// `answerProject(projects, msg, origin)`. serve-projects.ts:10-33.
pub async fn answer_project(projects: &dyn ProjectsApi, msg: &RuntimeRequest, origin: Option<Caller>) -> Result<Payload, HostError> {
    todo!()
}

/// `threadPlaces(rt, origin, now)`: every cloud, and for a thread in a folder each computer holding a project it may
/// start work on; each row less its road, a provider's labelled. serve-projects.ts:35-44, spec H20.
pub async fn thread_places(rt: &dyn ServedRuntime, origin: Option<Caller>, now: i64) -> Result<Vec<PlaceView>, HostError> {
    todo!()
}
