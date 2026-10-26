// SPDX-License-Identifier: AGPL-3.0-only
//! The workspaces door: making and finding workspaces, their machines' moves, everything that runs in or reads a
//! checkout, and the start and review doors. Port of runtime/src/projects/workspaces.ts at 65f6b89c7;
//! spec/workspaces_create.md, workspaces_phases.md, workspaces_git.md, workspaces_review.md.
pub mod create;
pub mod git;
pub mod phases;
pub mod review;
pub mod seam;

use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::api::{
    CommitAsk, CommitDraftAsk, DaemonEventListener, DiscardAsk, FixAsk, FolderFor, FolderForAsk, Landing, MergeAsk, MergeInAsk,
    PullRequestReplyAsk, ReviewAsk, ReviewDraftAsk, StartFromLinkAsk, ViewedMark, WorkspacesApi, WorktreeRemoveAsk,
};
use wsp_core::types::pending::DaemonChannel;
use wsp_core::types::wiring::{CreateWorkspaceOptions, CreatedWorkspace, Detach, RunningExec, SizeAsk, SysListener};
use wsp_frames::{GitCommitReply, GitDiscardReply};
use wsp_wire::types::bring_back::BringBackResult;
use wsp_wire::types::changes::{CheckoutReply, CommitDraft, ViewedMarks};
use wsp_wire::types::pull_request::{
    FixResult, GitPrReactReply, GitPrReplyReply, GitPrResolveReply, GitUpdateReply, MergeResult, PullRequestItem, PullRequestPage,
    PullRequestSendResult, ReactionContent,
};
use wsp_wire::types::start::{GitPrDiffReply, ReviewDraft};
use wsp_wire::types::tree::MergeInResult;
use wsp_wire::types::workspace_look::WorkspaceLook;
use wsp_wire::views::harness::{ReviewPostResult, StartResult};
use wsp_wire::views::workspace::{Caller, DaemonReachView, PortProbeView, PortReachView};
use wsp_wire::views::workspace_events::WorkspaceCreatingEvent;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceView, WorktreeMade};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

/// The whole context the workspaces door reads: each unit's trait. The wsp-core join implements all four.
pub trait WorkspacesCtx: create::CreateCtx + phases::PhasesCtx + git::GitCtx + review::ReviewCtx {}

/// The workspaces door. Each `WorkspacesApi` method is one line forwarding to its unit's function.
pub struct Workspaces {
    pub(crate) create: Rc<dyn create::CreateCtx>,
    pub(crate) phases: Rc<dyn phases::PhasesCtx>,
    pub(crate) git: Rc<dyn git::GitCtx>,
    pub(crate) review: Rc<dyn review::ReviewCtx>,
}

// export function workspacesArea(ctx: RuntimeContext): WorkspacesArea {
pub fn workspaces_area<C: WorkspacesCtx + 'static>(ctx: Rc<C>) -> Rc<Workspaces> {
    let create: Rc<dyn create::CreateCtx> = ctx.clone();
    let phases: Rc<dyn phases::PhasesCtx> = ctx.clone();
    let git: Rc<dyn git::GitCtx> = ctx.clone();
    let review: Rc<dyn review::ReviewCtx> = ctx;
    Rc::new(Workspaces { create, phases, git, review })
}

impl wsp_core::context::WorkspacesArea for Workspaces {}

#[async_trait(?Send)]
impl WorkspacesApi for Workspaces {
    async fn create(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError> {
        create::create(&self.create, opts, origin).await
    }
    async fn landing(&self, project: &str, sized: bool, origin: Option<Caller>) -> Result<Landing, HostError> {
        create::landing(&self.create, project, sized, origin).await
    }
    async fn get(&self, id: &str, origin: Option<Caller>, thread_id: Option<&str>) -> Result<WorkspaceView, HostError> {
        create::get(&self.create, id, origin, thread_id).await
    }
    async fn held_or_creating(&self, id: &str) -> Result<Option<WorkspaceView>, HostError> {
        create::held_or_creating(&self.create, id).await
    }
    fn thread_place(&self, scope: &ThreadScope) -> Result<Option<String>, HostError> {
        self.create.place_of_thread(scope)
    }
    async fn list(&self, origin: Option<Caller>) -> Result<Vec<WorkspaceView>, HostError> {
        create::list(&self.create, origin).await
    }
    async fn resolve(&self, reference: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        create::resolve(&self.create, reference, origin).await
    }
    async fn nap(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        phases::nap(&self.phases, id, origin).await
    }
    async fn wake(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        phases::wake(&self.phases, id, origin).await
    }
    async fn resize(&self, id: &str, size: SizeAsk, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        phases::resize(&self.phases, id, size, origin).await
    }
    async fn stop_wake(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        phases::stop_wake(&self.phases, id, origin).await
    }
    async fn upgrade(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        phases::upgrade(&self.phases, id, origin).await
    }
    async fn rebuild(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        phases::rebuild(&self.phases, id, origin).await
    }
    async fn restart_daemon(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        phases::restart_daemon(&self.phases, id, origin).await
    }
    async fn rename(&self, id: &str, name: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        phases::rename(&self.phases, id, name, origin).await
    }
    async fn look(&self, id: &str, look: WorkspaceLook, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        phases::look(&self.phases, id, look, origin).await
    }
    async fn update_daemon(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        phases::update_daemon(&self.phases, id, origin).await
    }
    async fn delete(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        phases::delete(&self.phases, id, origin).await
    }
    async fn forget(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        phases::forget(&self.phases, id, origin).await
    }
    async fn touch(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        phases::touch(&self.phases, id, origin).await
    }
    async fn exec(&self, id: &str, cmd: &str, timeout_ms: Option<u64>, origin: Option<Caller>) -> Result<ExecResult, HostError> {
        git::exec(&self.git, id, cmd, timeout_ms, origin).await
    }
    async fn exec_stream(
        &self,
        id: &str,
        argv: Vec<String>,
        cwd: Option<String>,
        origin: Option<Caller>,
    ) -> Result<RunningExec, HostError> {
        git::exec_stream(&self.git, id, argv, cwd, origin).await
    }
    async fn bring_back(
        &self,
        workspace_id: &str,
        title: Option<String>,
        body: Option<String>,
        origin: Option<Caller>,
    ) -> Result<BringBackResult, HostError> {
        git::bring_back(&self.git, workspace_id, title, body, origin).await
    }
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError> {
        git::folder_for(&self.git, ask, origin).await
    }
    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        git::take_setup_line(&self.git, workspace_id, origin).await
    }
    async fn folder(&self, project: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        git::folder(&self.git, project, origin).await
    }
    async fn worktree(&self, project: &str, branch: &str, origin: Option<Caller>) -> Result<WorktreeMade, HostError> {
        git::worktree(&self.git, project, branch, origin).await
    }
    async fn worktree_remove(&self, ask: WorktreeRemoveAsk, origin: Option<Caller>) -> Result<(), HostError> {
        git::worktree_remove(&self.git, ask, origin).await
    }
    async fn checkout(&self, id: &str, origin: Option<Caller>, fresh: Option<bool>) -> Result<CheckoutReply, HostError> {
        git::checkout(&self.git, id, origin, fresh.unwrap_or(false)).await
    }
    async fn discard(&self, ask: DiscardAsk, origin: Option<Caller>) -> Result<GitDiscardReply, HostError> {
        git::discard(&self.git, ask, origin).await
    }
    async fn commit(&self, ask: CommitAsk, origin: Option<Caller>) -> Result<GitCommitReply, HostError> {
        git::commit(&self.git, ask, origin).await
    }
    async fn commit_draft(&self, ask: CommitDraftAsk, origin: Option<Caller>) -> Result<CommitDraft, HostError> {
        git::commit_draft(&self.git, ask, origin).await
    }
    async fn viewed(&self, workspace_id: &str, mark: Option<ViewedMark>, origin: Option<Caller>) -> Result<ViewedMarks, HostError> {
        git::viewed(&self.git, workspace_id, mark, origin).await
    }
    async fn pull_request_view(
        &self,
        workspace_id: &str,
        fresh: Option<bool>,
        origin: Option<Caller>,
    ) -> Result<PullRequestPage, HostError> {
        git::pull_request_view(&self.git, workspace_id, fresh.unwrap_or(false), origin).await
    }
    async fn pull_request_diff(&self, workspace_id: &str, origin: Option<Caller>) -> Result<GitPrDiffReply, HostError> {
        git::pull_request_diff(&self.git, workspace_id, origin).await
    }
    async fn pull_request_send(
        &self,
        workspace_id: &str,
        items: Vec<PullRequestItem>,
        thread_id: Option<String>,
        origin: Option<Caller>,
    ) -> Result<PullRequestSendResult, HostError> {
        git::pull_request_send(&self.git, workspace_id, items, thread_id, origin).await
    }
    async fn pull_request_reply(&self, ask: PullRequestReplyAsk, origin: Option<Caller>) -> Result<GitPrReplyReply, HostError> {
        git::pull_request_reply(&self.git, ask, origin).await
    }
    async fn pull_request_resolve(
        &self,
        workspace_id: &str,
        thread_id: &str,
        resolved: bool,
        origin: Option<Caller>,
    ) -> Result<GitPrResolveReply, HostError> {
        git::pull_request_resolve(&self.git, workspace_id, thread_id, resolved, origin).await
    }
    async fn pull_request_react(
        &self,
        workspace_id: &str,
        subject: &str,
        content: ReactionContent,
        on: bool,
        origin: Option<Caller>,
    ) -> Result<GitPrReactReply, HostError> {
        git::pull_request_react(&self.git, workspace_id, subject, content, on, origin).await
    }
    async fn fix(&self, ask: FixAsk, origin: Option<Caller>) -> Result<FixResult, HostError> {
        git::fix(&self.git, ask, origin).await
    }
    async fn merge(&self, ask: MergeAsk, origin: Option<Caller>) -> Result<MergeResult, HostError> {
        git::merge(&self.git, ask, origin).await
    }
    async fn update(&self, workspace_id: &str, thread_id: Option<String>, origin: Option<Caller>) -> Result<GitUpdateReply, HostError> {
        git::update(&self.git, workspace_id, thread_id, origin).await
    }
    async fn merge_in(&self, ask: MergeInAsk, origin: Option<Caller>) -> Result<MergeInResult, HostError> {
        git::merge_in(&self.git, ask, origin).await
    }
    async fn start(&self, ask: StartFromLinkAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
        review::start(&self.review, ask, origin).await
    }
    async fn review(&self, ask: ReviewAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
        review::review(&self.review, ask, origin).await
    }
    async fn review_draft(&self, ask: ReviewDraftAsk, origin: Option<Caller>) -> Result<Option<ReviewDraft>, HostError> {
        review::review_draft(&self.review, ask, origin).await
    }
    async fn review_post(
        &self,
        workspace_id: &str,
        thread_id: Option<String>,
        origin: Option<Caller>,
    ) -> Result<ReviewPostResult, HostError> {
        review::review_post(&self.review, workspace_id, thread_id, origin).await
    }
    async fn daemon_reach(&self, id: &str, origin: Option<Caller>) -> Result<DaemonReachView, HostError> {
        review::daemon_reach(&self.review, id, origin).await
    }
    async fn token_refused(&self, id: &str) -> Result<(), HostError> {
        review::token_refused(&self.review, id).await
    }
    async fn daemon_channel(
        &self,
        id: &str,
        on_event: DaemonEventListener,
        origin: Option<Caller>,
    ) -> Result<Rc<dyn DaemonChannel>, HostError> {
        review::daemon_channel(&self.review, id, on_event, origin).await
    }
    async fn guest_channel(&self, id: &str, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        review::guest_channel(&self.review, id, on_event).await
    }
    async fn served_by_its_computer(&self, id: &str, origin: Option<Caller>) -> Result<bool, HostError> {
        review::served_by_its_computer(&self.review, id, origin).await
    }
    async fn watch_sys(&self, id: &str, listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError> {
        review::watch_sys(&self.review, id, listener, origin).await
    }
    async fn port_reach(&self, id: &str, port: u16, origin: Option<Caller>) -> Result<PortReachView, HostError> {
        review::port_reach(&self.review, id, port, origin).await
    }
    async fn port_probe(&self, id: &str, port: u16, origin: Option<Caller>) -> Result<PortProbeView, HostError> {
        review::port_probe(&self.review, id, port, origin).await
    }
    async fn origin_refusal(&self, id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        review::origin_refusal(&self.review, id, origin).await
    }
    fn seen_by(&self, event: &serde_json::Value, origin: Option<&Caller>) -> Result<bool, HostError> {
        review::seen_by(&self.review, event, origin)
    }
    fn shown_to(&self, event: serde_json::Value, origin: Option<&Caller>) -> Result<serde_json::Value, HostError> {
        review::shown_to(&self.review, event, origin)
    }
    fn creating(&self) -> Vec<WorkspaceCreatingEvent> {
        create::creating(&self.create)
    }
}
