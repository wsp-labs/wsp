// SPDX-License-Identifier: AGPL-3.0-only
use serde_json::json;
use wsp_host_client::protocol::*;

use crate::common::{answered, ask, refused, shared};

const NO_PULL_REQUEST: &str = "pull_request-x has no pull request: push its branch and open one with gh pr create first";
const NO_DAEMON: &str = "wsp-daemon binary missing";

#[tokio::test]
async fn pull_request() {
    let host = &shared().host;
    let folder = host.repo("pull_request-x");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "pull_request-x", "changes pane", "--agent", "claude"]);
    let client = host.client().await;
    let row = client
        .threads()
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.prompt.as_deref() == Some("changes pane"))
        .expect("the thread wsp run opened");
    let ws = row.workspace_id.as_str();
    let thread = row.thread_id.expect("a thread id");
    std::fs::write(folder.join("kept.txt"), "kept\n").unwrap();
    std::fs::write(folder.join("dropped.txt"), "dropped\n").unwrap();

    // With no daemon staged git is never read, so the host answers with no checkout fact rather than a refusal.
    answered("workspaces.checkout", client.call(ask::<WorkspacesCheckoutRequest>(json!({ "workspaceId": ws, "fresh": true }))).await);

    let viewed = answered(
        "workspaces.viewed",
        client.call(ask::<WorkspacesViewedRequest>(json!({ "workspaceId": ws, "path": "kept.txt", "blob": "b1" }))).await,
    );
    assert_eq!(viewed.viewed.get("kept.txt").map(String::as_str), Some("b1"));
    let mut unmark = ask::<WorkspacesViewedRequest>(json!({ "workspaceId": ws, "path": "kept.txt" }));
    unmark.blob = Some(WorkspacesViewedRequestBlobOrNull(None));
    answered("workspaces.viewed", client.call(unmark).await);
    let viewed = answered("workspaces.viewed", client.call(ask::<WorkspacesViewedRequest>(json!({ "workspaceId": ws }))).await);
    assert!(!viewed.viewed.contains_key("kept.txt"), "{viewed:?}");

    refused(
        "workspaces.commitDraft",
        client.call(ask::<WorkspacesCommitDraftRequest>(json!({ "workspaceId": ws, "paths": ["kept.txt"], "threadId": thread }))).await,
        NO_DAEMON,
    );
    refused(
        "workspaces.discard",
        client.call(ask::<WorkspacesDiscardRequest>(json!({ "workspaceId": ws, "path": "dropped.txt" }))).await,
        NO_DAEMON,
    );
    refused(
        "workspaces.commit",
        client.call(ask::<WorkspacesCommitRequest>(json!({ "workspaceId": ws, "message": "add kept", "paths": ["kept.txt"] }))).await,
        NO_DAEMON,
    );
    refused("workspaces.update", client.call(ask::<WorkspacesUpdateRequest>(json!({ "workspaceId": ws }))).await, NO_DAEMON);
    refused("workspaces.fix", client.call(ask::<WorkspacesFixRequest>(json!({ "workspaceId": ws }))).await, NO_DAEMON);
    refused("workspaces.bringBack", client.call(ask::<WorkspacesBringBackRequest>(json!({ "workspaceId": ws }))).await, NO_DAEMON);
    assert!(folder.join("dropped.txt").exists());

    refused(
        "workspaces.pullRequestView",
        client.call(ask::<WorkspacesPullRequestViewRequest>(json!({ "workspaceId": ws }))).await,
        NO_PULL_REQUEST,
    );
    refused(
        "workspaces.pullRequestDiff",
        client.call(ask::<WorkspacesPullRequestDiffRequest>(json!({ "workspaceId": ws }))).await,
        NO_PULL_REQUEST,
    );
    refused(
        "workspaces.pullRequestSend",
        client.call(ask::<WorkspacesPullRequestSendRequest>(json!({ "workspaceId": ws, "items": [{ "kind": "comment", "id": 1 }] }))).await,
        NO_PULL_REQUEST,
    );
    refused(
        "workspaces.pullRequestReply",
        client.call(ask::<WorkspacesPullRequestReplyRequest>(json!({ "workspaceId": ws, "body": "thanks" }))).await,
        NO_PULL_REQUEST,
    );
    refused(
        "workspaces.pullRequestResolve",
        client.call(ask::<WorkspacesPullRequestResolveRequest>(json!({ "workspaceId": ws, "threadId": "T_1", "resolved": true }))).await,
        NO_PULL_REQUEST,
    );
    refused(
        "workspaces.pullRequestReact",
        client
            .call(ask::<WorkspacesPullRequestReactRequest>(json!({ "workspaceId": ws, "subject": "IC_1", "content": "+1", "on": true })))
            .await,
        NO_PULL_REQUEST,
    );
    refused("workspaces.merge", client.call(ask::<WorkspacesMergeRequest>(json!({ "workspaceId": ws }))).await, NO_PULL_REQUEST);

    let review = answered("workspaces.reviewDraft", client.call(ask::<WorkspacesReviewDraftRequest>(json!({ "workspaceId": ws }))).await);
    assert!(review.review.is_none());
    refused(
        "workspaces.reviewPost",
        client.call(ask::<WorkspacesReviewPostRequest>(json!({ "workspaceId": ws }))).await,
        "pull_request-x has no review to post yet",
    );
}
