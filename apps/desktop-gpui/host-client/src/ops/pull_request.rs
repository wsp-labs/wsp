// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::protocol;

ops! {
    "workspaces.checkout" => WorkspacesCheckoutRequest => protocol::CheckoutReply,
    "workspaces.commit" => WorkspacesCommitRequest => protocol::GitCommitReply,
    "workspaces.commitDraft" => WorkspacesCommitDraftRequest => protocol::CommitDraft,
    "workspaces.discard" => WorkspacesDiscardRequest => protocol::GitDiscardReply,
    "workspaces.viewed" => WorkspacesViewedRequest => protocol::ViewedMarks,
    "workspaces.pullRequestView" => WorkspacesPullRequestViewRequest => protocol::PullRequestPage,
    "workspaces.pullRequestDiff" => WorkspacesPullRequestDiffRequest => protocol::GitPrDiffReply,
    "workspaces.pullRequestSend" => WorkspacesPullRequestSendRequest => protocol::PullRequestSendResult,
    "workspaces.pullRequestReply" => WorkspacesPullRequestReplyRequest => protocol::GitPrReplyReply,
    "workspaces.pullRequestResolve" => WorkspacesPullRequestResolveRequest => protocol::GitPrResolveReply,
    "workspaces.pullRequestReact" => WorkspacesPullRequestReactRequest => protocol::GitPrReactReply,
    "workspaces.fix" => WorkspacesFixRequest => protocol::FixResult,
    "workspaces.merge" => WorkspacesMergeRequest => protocol::MergeResult,
    "workspaces.update" => WorkspacesUpdateRequest => protocol::GitUpdateReply,
    "workspaces.bringBack" => WorkspacesBringBackRequest => protocol::BringBackResult,
    "workspaces.reviewDraft" => WorkspacesReviewDraftRequest => ReviewDraftReply,
    "workspaces.reviewPost" => WorkspacesReviewPostRequest => protocol::ReviewPostResult,
}

/// A review workspace's draft, absent where the workspace holds none.
#[derive(Debug, Deserialize)]
pub struct ReviewDraftReply {
    pub review: Option<protocol::ReviewDraft>,
}
