// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-start.ts at 65f6b89c7: a sessions.start off the socket: a fork in its
//! source's folder, else a conversation kept outside wsp where it ran, else the folder of the record, project,
//! branch or folder it names, found or made first. Spec: spec/serve_helpers.md, H10-H15.
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use wsp_core::types::api::{HeldStart, OutsideOpening, StartOptions, WorkspacesApi};
use wsp_core::types::wiring::SessionHandle;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::SessionStartResult;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve::served::ServedRuntime;

/// Where a start or a warm runs: the record, and the folder and setup line the folder road read. answered-start.ts:24-30.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartFolder {
    pub workspace_id: String,
    pub cwd: Option<String>,
    pub setup: Option<String>,
}

/// The start a held answer hands its hold to. answered-start.ts:6.
pub type OnHeld = Rc<dyn Fn(HeldStart)>;
/// The start `answered_start` runs, handed the hold callback where the frame asked to hear of a hold.
pub type StartFn = Box<dyn FnOnce(Option<OnHeld>) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>>>;
/// Each answer of a start, sent under the frame's id.
pub type AnswerFn = Rc<dyn Fn(SessionStartResult)>;

/// threads/answered-start.ts, which wsp-threads ports (threads a's slice). Reached through this trait until that
/// crate builds; the join calls its functions directly.
#[async_trait(?Send)]
pub trait StartRoads {
    /// `startAt(workspaces, msg, origin, { opens })`: the record named, else the folder of the project, branch or cwd.
    async fn start_at(&self, workspaces: &dyn WorkspacesApi, msg: &RuntimeRequest, origin: Option<Caller>, opens: bool) -> Result<StartFolder, HostError>;
    /// `answeredStart(answerHeld, start, answer)`: answers once at the hold, else once at the start.
    async fn answered_start(&self, answer_held: bool, start: StartFn, answer: AnswerFn) -> Result<(), HostError>;
}

/// `startOptionsOf(msg, at, outside, held)`: each field the client named, copied by hand in the TS's order
/// (spec H15). `msg` is a `sessions.start` request.
pub fn start_options_of(msg: &RuntimeRequest, at: &StartFolder, outside: Option<OutsideOpening>, on_held: Option<OnHeld>) -> StartOptions {
    todo!()
}

/// `serveStart(rt, msg, origin, reply)`. serve-start.ts:38-52. The refusals come first (spec H10-H11), then the
/// fork, else the resume, else the folder.
pub async fn serve_start(rt: &dyn ServedRuntime, roads: &dyn StartRoads, msg: &RuntimeRequest, origin: Option<Caller>, reply: AnswerFn) -> Result<(), HostError> {
    todo!()
}
