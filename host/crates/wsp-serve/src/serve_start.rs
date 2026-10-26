// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-start.ts at 65f6b89c7: a sessions.start off the socket: a fork in its
//! source's folder, else a conversation kept outside wsp where it ran, else the folder of the record, project,
//! branch or folder it names, found or made first. Spec: spec/serve_helpers.md, H10-H15.
use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::api::{FolderFor, FolderForAsk, ForkOptions, OutsideOpening, PlacedStart, StartOptions, WorkspacesApi};
use wsp_threads::threads::answered_start::{answered_start, start_at, OnHeld, StartAtMsg, StartAtWorkspaces, StartedAt};
use wsp_wire::types::projects::BRANCH_ON_A_THREAD_LINE;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::SessionStartResult;
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::{usage_refusal, HostError, Kind};
use wsp_words::words::thread::{FORK_BESIDE_FIX, FORK_BESIDE_LINE, FORK_RESUME_FIX, FORK_RESUME_LINE};

use crate::serve::served::DynRuntime;

/// Each answer of a start, sent under the frame's id.
pub type AnswerFn = Rc<dyn Fn(SessionStartResult)>;

/// `startOptionsOf(msg, at, outside, held)`: each field the client named, copied by hand in the TS's order
/// (spec H15). `msg` is a `sessions.start` request.
pub fn start_options_of(msg: &RuntimeRequest, at: &StartedAt, outside: Option<OutsideOpening>, on_held: Option<OnHeld>) -> StartOptions {
    let RuntimeOp::SessionsStart {
        prompt,
        harness,
        thread,
        model,
        effort,
        permission_mode,
        access,
        context_window,
        fast,
        started_by,
        request_id,
        followed,
        attempt,
        notify,
        turn_token,
        title,
        replaces,
        attachments,
        ..
    } = &msg.op
    else {
        unreachable!("start_options_of is handed a sessions.start, which serve_start checks")
    };
    StartOptions {
        prompt: prompt.clone(),
        on_held,
        followed: followed.filter(|followed| *followed),
        harness: harness.clone(),
        thread: thread.clone(),
        cwd: at.cwd.clone(),
        setup: at.setup.clone(),
        model: model.clone(),
        effort: effort.clone(),
        permission_mode: permission_mode.clone(),
        access: *access,
        context_window: context_window.clone(),
        fast: *fast,
        started_by: *started_by,
        request_id: request_id.clone(),
        attempt: attempt.clone(),
        notify: notify.clone(),
        turn_token: turn_token.clone(),
        title: title.clone(),
        replaces: replaces.clone(),
        attachments: attachments.clone(),
        outside,
        ..Default::default()
    }
}

/// `serveStart(rt, msg, origin, reply)`. serve-start.ts:38-52. The refusals come first (spec H10-H11), then the
/// fork, else the resume, else the folder; the folder is found by threads' `start_at` and the answers go through its
/// `answered_start`.
pub async fn serve_start(rt: &DynRuntime, msg: &RuntimeRequest, origin: Option<Caller>, reply: AnswerFn) -> Result<(), HostError> {
    let RuntimeOp::SessionsStart {
        workspace_id,
        project,
        branch,
        harness,
        thread,
        cwd,
        model,
        effort,
        permission_mode,
        access,
        fast,
        answer_held,
        replaces,
        fork,
        resume,
        ..
    } = &msg.op
    else {
        return Err(HostError::new(format!("{} is not answered here", msg.op.name())));
    };
    if workspace_id.is_some() && branch.is_some() {
        return Err(HostError::with_kind(BRANCH_ON_A_THREAD_LINE, Kind::Usage));
    }
    if fork.is_some() && (workspace_id.is_some() || project.is_some() || cwd.is_some() || thread.is_some() || replaces.is_some()) {
        return Err(usage_refusal(FORK_BESIDE_LINE, FORK_BESIDE_FIX));
    }
    if fork.is_some() && resume.is_some() {
        return Err(usage_refusal(FORK_RESUME_LINE, FORK_RESUME_FIX));
    }
    let answer_held = *answer_held == Some(true);
    if let Some(fork) = fork {
        return answered_start(
            answer_held,
            |on_held| async move {
                let start = start_options_of(msg, &StartedAt::default(), None, on_held);
                let options = ForkOptions { start, fork: fork.clone(), branch: branch.clone() };
                rt.sessions().fork(options, origin).await
            },
            reply,
        )
        .await;
    }
    let placed = match resume {
        Some(resume) => {
            let start = PlacedStart {
                workspace_id: workspace_id.clone(),
                project: project.clone(),
                harness: harness.clone(),
                thread: thread.clone(),
                branch: branch.clone(),
                cwd: cwd.clone(),
            };
            Some(rt.conversations().placed(start, resume.clone(), origin.clone()).await?)
        }
        None => None,
    };
    let (at, outside) = match placed {
        Some(placed) => (StartedAt { workspace_id: placed.workspace_id, cwd: placed.cwd, setup: None }, Some(placed.outside)),
        None => {
            let picks = StartAtMsg {
                workspace_id: workspace_id.clone(),
                project: project.clone(),
                branch: branch.clone(),
                cwd: cwd.clone(),
                harness: harness.clone(),
                model: model.clone(),
                effort: effort.clone(),
                access: *access,
                permission_mode: permission_mode.clone(),
                fast: *fast,
            };
            (start_at(&Folders(rt.workspaces()), picks, origin.clone(), thread.is_none()).await?, None)
        }
    };
    answered_start(
        answer_held,
        |on_held| async move {
            let start = start_options_of(msg, &at, outside, on_held);
            rt.sessions().start(&at.workspace_id, start, origin).await
        },
        reply,
    )
    .await
}

/// `rt.workspaces` as threads' `start_at` reads it: `start_at` takes a `&dyn`, and the runtime hands out a `&dyn`
/// it cannot coerce to one, so this wraps it.
struct Folders<'a>(&'a dyn WorkspacesApi);

#[async_trait(?Send)]
impl StartAtWorkspaces for Folders<'_> {
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError> {
        WorkspacesApi::folder_for(self.0, ask, origin).await
    }

    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        WorkspacesApi::take_setup_line(self.0, workspace_id, origin).await
    }
}
