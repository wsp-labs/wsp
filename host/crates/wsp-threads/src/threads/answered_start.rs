// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/answered-start.ts at 65f6b89c7. Spec: spec/answered_start.md.
use std::future::Future;
use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::api::{FolderFor, FolderForAsk, HeldStart};
use wsp_core::types::wiring::SessionHandle;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::SessionStartResult;
use wsp_words::exit::HostError;

/// What a held start is answered through (TS `OnHeld`).
pub type OnHeld = Rc<dyn Fn(HeldStart)>;

/// answered-start.ts:11. Rules A1-A6.
pub async fn answered_start<S, F>(answer_held: bool, start: S, answer: Rc<dyn Fn(SessionStartResult)>) -> Result<(), HostError>
where
    S: FnOnce(Option<OnHeld>) -> F,
    F: Future<Output = Result<Rc<dyn SessionHandle>, HostError>>,
{
    todo!()
}

/// What `start_at` reads of the workspaces area (TS `Pick<Runtime["workspaces"], "folderFor" | "takeSetupLine">`).
#[async_trait(?Send)]
pub trait StartAtWorkspaces {
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError>;
    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError>;
}

/// The message a `sessions.start` or `sessions.warm` names its place by (answered-start.ts:27).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StartAtMsg {
    pub workspace_id: Option<String>,
    pub project: Option<String>,
    pub branch: Option<String>,
    pub cwd: Option<String>,
    pub harness: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub access: Option<AccessChoice>,
    pub permission_mode: Option<String>,
    pub fast: Option<bool>,
}

/// answered-start.ts:30.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StartedAt {
    pub workspace_id: String,
    pub cwd: Option<String>,
    pub setup: Option<String>,
}

/// answered-start.ts:25. `opens` is TS `o.opens === true`. Rules S1-S5.
pub async fn start_at(workspaces: &dyn StartAtWorkspaces, msg: StartAtMsg, origin: Option<Caller>, opens: bool) -> Result<StartedAt, HostError> {
    todo!()
}
