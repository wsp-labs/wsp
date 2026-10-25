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
    todo!()
}

impl wsp_core::context::WorkspacesArea for Workspaces {}

#[async_trait(?Send)]
impl WorkspacesApi for Workspaces {
    async fn create(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError> {
        todo!()
    }
    async fn landing(&self, project: &str, sized: bool, origin: Option<Caller>) -> Result<Landing, HostError> {
        todo!()
    }
    async fn get(&self, id: &str, origin: Option<Caller>, thread_id: Option<&str>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn held_or_creating(&self, id: &str) -> Result<Option<WorkspaceView>, HostError> {
        todo!()
    }
    fn thread_place(&self, scope: &ThreadScope) -> Option<String> {
        todo!()
    }
    async fn list(&self, origin: Option<Caller>) -> Result<Vec<WorkspaceView>, HostError> {
        todo!()
    }
    async fn resolve(&self, reference: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn nap(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn wake(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn resize(&self, id: &str, size: SizeAsk, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn stop_wake(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn upgrade(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn rebuild(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn restart_daemon(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        todo!()
    }
    async fn rename(&self, id: &str, name: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn look(&self, id: &str, look: WorkspaceLook, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn update_daemon(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        todo!()
    }
    async fn delete(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        todo!()
    }
    async fn forget(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        todo!()
    }
    async fn touch(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        todo!()
    }
    async fn exec(&self, id: &str, cmd: &str, timeout_ms: Option<u64>, origin: Option<Caller>) -> Result<ExecResult, HostError> {
        todo!()
    }
    async fn exec_stream(&self, id: &str, argv: Vec<String>, cwd: Option<String>, origin: Option<Caller>) -> Result<RunningExec, HostError> {
        todo!()
    }
    async fn bring_back(&self, workspace_id: &str, title: Option<String>, body: Option<String>, origin: Option<Caller>) -> Result<BringBackResult, HostError> {
        todo!()
    }
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError> {
        todo!()
    }
    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        todo!()
    }
    async fn folder(&self, project: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        todo!()
    }
    async fn worktree(&self, project: &str, branch: &str, origin: Option<Caller>) -> Result<WorktreeMade, HostError> {
        todo!()
    }
    async fn worktree_remove(&self, ask: WorktreeRemoveAsk, origin: Option<Caller>) -> Result<(), HostError> {
        todo!()
    }
    async fn checkout(&self, id: &str, origin: Option<Caller>, fresh: Option<bool>) -> Result<CheckoutReply, HostError> {
        todo!()
    }
    async fn discard(&self, ask: DiscardAsk, origin: Option<Caller>) -> Result<GitDiscardReply, HostError> {
        todo!()
    }
    async fn commit(&self, ask: CommitAsk, origin: Option<Caller>) -> Result<GitCommitReply, HostError> {
        todo!()
    }
    async fn commit_draft(&self, ask: CommitDraftAsk, origin: Option<Caller>) -> Result<CommitDraft, HostError> {
        todo!()
    }
    async fn viewed(&self, workspace_id: &str, mark: Option<ViewedMark>, origin: Option<Caller>) -> Result<ViewedMarks, HostError> {
        todo!()
    }
    async fn pull_request_view(&self, workspace_id: &str, fresh: Option<bool>, origin: Option<Caller>) -> Result<PullRequestPage, HostError> {
        todo!()
    }
    async fn pull_request_diff(&self, workspace_id: &str, origin: Option<Caller>) -> Result<GitPrDiffReply, HostError> {
        todo!()
    }
    async fn pull_request_send(&self, workspace_id: &str, items: Vec<PullRequestItem>, thread_id: Option<String>, origin: Option<Caller>) -> Result<PullRequestSendResult, HostError> {
        todo!()
    }
    async fn pull_request_reply(&self, ask: PullRequestReplyAsk, origin: Option<Caller>) -> Result<GitPrReplyReply, HostError> {
        todo!()
    }
    async fn pull_request_resolve(&self, workspace_id: &str, thread_id: &str, resolved: bool, origin: Option<Caller>) -> Result<GitPrResolveReply, HostError> {
        todo!()
    }
    async fn pull_request_react(&self, workspace_id: &str, subject: &str, content: ReactionContent, on: bool, origin: Option<Caller>) -> Result<GitPrReactReply, HostError> {
        todo!()
    }
    async fn fix(&self, ask: FixAsk, origin: Option<Caller>) -> Result<FixResult, HostError> {
        todo!()
    }
    async fn merge(&self, ask: MergeAsk, origin: Option<Caller>) -> Result<MergeResult, HostError> {
        todo!()
    }
    async fn update(&self, workspace_id: &str, thread_id: Option<String>, origin: Option<Caller>) -> Result<GitUpdateReply, HostError> {
        todo!()
    }
    async fn merge_in(&self, ask: MergeInAsk, origin: Option<Caller>) -> Result<MergeInResult, HostError> {
        todo!()
    }
    async fn start(&self, ask: StartFromLinkAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
        todo!()
    }
    async fn review(&self, ask: ReviewAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
        todo!()
    }
    async fn review_draft(&self, ask: ReviewDraftAsk, origin: Option<Caller>) -> Result<Option<ReviewDraft>, HostError> {
        todo!()
    }
    async fn review_post(&self, workspace_id: &str, thread_id: Option<String>, origin: Option<Caller>) -> Result<ReviewPostResult, HostError> {
        todo!()
    }
    async fn daemon_reach(&self, id: &str, origin: Option<Caller>) -> Result<DaemonReachView, HostError> {
        todo!()
    }
    async fn token_refused(&self, id: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn daemon_channel(&self, id: &str, on_event: DaemonEventListener, origin: Option<Caller>) -> Result<Rc<dyn DaemonChannel>, HostError> {
        todo!()
    }
    async fn guest_channel(&self, id: &str, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        todo!()
    }
    async fn served_by_its_computer(&self, id: &str, origin: Option<Caller>) -> Result<bool, HostError> {
        todo!()
    }
    async fn watch_sys(&self, id: &str, listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError> {
        todo!()
    }
    async fn port_reach(&self, id: &str, port: u16, origin: Option<Caller>) -> Result<PortReachView, HostError> {
        todo!()
    }
    async fn port_probe(&self, id: &str, port: u16, origin: Option<Caller>) -> Result<PortProbeView, HostError> {
        todo!()
    }
    async fn origin_refusal(&self, id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        todo!()
    }
    fn seen_by(&self, event: &serde_json::Value, origin: Option<&Caller>) -> bool {
        todo!()
    }
    fn shown_to(&self, event: serde_json::Value, origin: Option<&Caller>) -> serde_json::Value {
        todo!()
    }
    fn creating(&self) -> Vec<WorkspaceCreatingEvent> {
        todo!()
    }
}
