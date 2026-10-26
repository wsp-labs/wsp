// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/answered-start.ts at 65f6b89c7. Spec: spec/answered_start.md.
use std::cell::Cell;
use std::future::Future;
use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::api::{FolderFor, FolderForAsk, HeldStart, WorkspacesApi};
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::wiring::SessionHandle;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::{SessionStartOutcome, SessionStartResult};
use wsp_words::exit::HostError;

/// What a held start is answered through (TS `OnHeld`).
pub type OnHeld = Rc<dyn Fn(HeldStart)>;

/// answered-start.ts:11. Rules A1-A6.
pub async fn answered_start<S, F>(answer_held: bool, start: S, answer: Rc<dyn Fn(SessionStartResult)>) -> Result<(), HostError>
where
    S: FnOnce(Option<OnHeld>) -> F,
    F: Future<Output = Result<Rc<dyn SessionHandle>, HostError>>,
{
    let answered = Rc::new(Cell::new(false));
    let on_held: Option<OnHeld> = answer_held.then(|| {
        let (answered, answer) = (answered.clone(), answer.clone());
        Rc::new(move |held: HeldStart| {
            answered.set(true);
            answer(SessionStartResult { session: held.view, outcome: SessionStartOutcome::Held, turn_id: held.turn_id, notice: None });
        }) as OnHeld
    });
    let handle = match start(on_held).await {
        Ok(handle) => handle,
        Err(e) => return if answered.get() { Ok(()) } else { Err(e) },
    };
    if !answered.get() {
        let session = handle.view();
        let outcome = handle.outcome();
        let turn_id = handle.turn_id();
        let notice = handle.notice();
        answer(SessionStartResult { session, outcome, turn_id, notice });
    }
    Ok(())
}

/// What `start_at` reads of the workspaces area (TS `Pick<Runtime["workspaces"], "folderFor" | "takeSetupLine">`).
#[async_trait(?Send)]
pub trait StartAtWorkspaces {
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError>;
    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError>;
}

#[async_trait(?Send)]
impl<W: WorkspacesApi + ?Sized> StartAtWorkspaces for W {
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError> {
        WorkspacesApi::folder_for(self, ask, origin).await
    }
    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        WorkspacesApi::take_setup_line(self, workspace_id, origin).await
    }
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
    if let Some(workspace_id) = msg.workspace_id {
        let setup = if opens { workspaces.take_setup_line(&workspace_id, origin).await? } else { None };
        return Ok(StartedAt { workspace_id, cwd: msg.cwd, setup });
    }
    let picks = StartPicksAsked {
        harness: msg.harness,
        model: msg.model,
        effort: msg.effort,
        access: msg.access,
        permission_mode: msg.permission_mode,
        fast: (msg.fast == Some(true)).then_some(true),
    };
    let picks = (picks != StartPicksAsked::default()).then_some(picks);
    let found = workspaces.folder_for(FolderForAsk { project: msg.project, branch: msg.branch, cwd: msg.cwd, picks, opens }, origin).await?;
    Ok(StartedAt { workspace_id: found.workspace.id, cwd: found.cwd, setup: found.setup })
}
