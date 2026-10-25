// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of pull-request.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::types::pull_request::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_pull_request.json");

/// 6 cases: what zod's parse of CheckState wrote, or the first issue it raised.
#[test]
fn check_state_reads_and_writes_as_zod_does() {
    cases::<CheckState>(FIXTURES, "CheckState");
}

/// 6 cases: what zod's parse of FixOutcome wrote, or the first issue it raised.
#[test]
fn fix_outcome_reads_and_writes_as_zod_does() {
    cases::<FixOutcome>(FIXTURES, "FixOutcome");
}

/// 3 cases: what zod's parse of FixResult wrote, or the first issue it raised.
#[test]
fn fix_result_reads_and_writes_as_zod_does() {
    cases::<FixResult>(FIXTURES, "FixResult");
}

/// 5 cases: what zod's parse of GitPrMergeReply wrote, or the first issue it raised.
#[test]
fn git_pr_merge_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrMergeReply>(FIXTURES, "GitPrMergeReply");
}

/// 3 cases: what zod's parse of GitPrReactReply wrote, or the first issue it raised.
#[test]
fn git_pr_react_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrReactReply>(FIXTURES, "GitPrReactReply");
}

/// 4 cases: what zod's parse of GitPrReadReply wrote, or the first issue it raised.
#[test]
fn git_pr_read_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrReadReply>(FIXTURES, "GitPrReadReply");
}

/// 3 cases: what zod's parse of GitPrReplyReply wrote, or the first issue it raised.
#[test]
fn git_pr_reply_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrReplyReply>(FIXTURES, "GitPrReplyReply");
}

/// 4 cases: what zod's parse of GitPrResolveReply wrote, or the first issue it raised.
#[test]
fn git_pr_resolve_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrResolveReply>(FIXTURES, "GitPrResolveReply");
}

/// 4 cases: what zod's parse of GitPrViewReply wrote, or the first issue it raised.
#[test]
fn git_pr_view_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrViewReply>(FIXTURES, "GitPrViewReply");
}

/// 5 cases: what zod's parse of GitRepoReadReply wrote, or the first issue it raised.
#[test]
fn git_repo_read_reply_reads_and_writes_as_zod_does() {
    cases::<GitRepoReadReply>(FIXTURES, "GitRepoReadReply");
}

/// 4 cases: what zod's parse of GitRunLogReply wrote, or the first issue it raised.
#[test]
fn git_run_log_reply_reads_and_writes_as_zod_does() {
    cases::<GitRunLogReply>(FIXTURES, "GitRunLogReply");
}

/// 6 cases: what zod's parse of GitUpdateReply wrote, or the first issue it raised.
#[test]
fn git_update_reply_reads_and_writes_as_zod_does() {
    cases::<GitUpdateReply>(FIXTURES, "GitUpdateReply");
}

/// 4 cases: what zod's parse of MergeMethod wrote, or the first issue it raised.
#[test]
fn merge_method_reads_and_writes_as_zod_does() {
    cases::<MergeMethod>(FIXTURES, "MergeMethod");
}

/// 8 cases: what zod's parse of MergeResult wrote, or the first issue it raised.
#[test]
fn merge_result_reads_and_writes_as_zod_does() {
    cases::<MergeResult>(FIXTURES, "MergeResult");
}

/// 4 cases: what zod's parse of Mergeable wrote, or the first issue it raised.
#[test]
fn mergeable_reads_and_writes_as_zod_does() {
    cases::<Mergeable>(FIXTURES, "Mergeable");
}

/// 20 cases: what zod's parse of PullRequest wrote, or the first issue it raised.
#[test]
fn pull_request_reads_and_writes_as_zod_does() {
    cases::<PullRequest>(FIXTURES, "PullRequest");
}

/// 5 cases: what zod's parse of PullRequestCheck wrote, or the first issue it raised.
#[test]
fn pull_request_check_reads_and_writes_as_zod_does() {
    cases::<PullRequestCheck>(FIXTURES, "PullRequestCheck");
}

/// 7 cases: what zod's parse of PullRequestComment wrote, or the first issue it raised.
#[test]
fn pull_request_comment_reads_and_writes_as_zod_does() {
    cases::<PullRequestComment>(FIXTURES, "PullRequestComment");
}

/// 11 cases: what zod's parse of PullRequestCommit wrote, or the first issue it raised.
#[test]
fn pull_request_commit_reads_and_writes_as_zod_does() {
    cases::<PullRequestCommit>(FIXTURES, "PullRequestCommit");
}

/// 20 cases: what zod's parse of PullRequestFact wrote, or the first issue it raised.
#[test]
fn pull_request_fact_reads_and_writes_as_zod_does() {
    cases::<PullRequestFact>(FIXTURES, "PullRequestFact");
}

/// 6 cases: what zod's parse of PullRequestItem wrote, or the first issue it raised.
#[test]
fn pull_request_item_reads_and_writes_as_zod_does() {
    cases::<PullRequestItem>(FIXTURES, "PullRequestItem");
}

/// 7 cases: what zod's parse of PullRequestKept wrote, or the first issue it raised.
#[test]
fn pull_request_kept_reads_and_writes_as_zod_does() {
    cases::<PullRequestKept>(FIXTURES, "PullRequestKept");
}

/// 4 cases: what zod's parse of PullRequestLabel wrote, or the first issue it raised.
#[test]
fn pull_request_label_reads_and_writes_as_zod_does() {
    cases::<PullRequestLabel>(FIXTURES, "PullRequestLabel");
}

/// 5 cases: what zod's parse of PullRequestPage wrote, or the first issue it raised.
#[test]
fn pull_request_page_reads_and_writes_as_zod_does() {
    cases::<PullRequestPage>(FIXTURES, "PullRequestPage");
}

/// 7 cases: what zod's parse of PullRequestReaction wrote, or the first issue it raised.
#[test]
fn pull_request_reaction_reads_and_writes_as_zod_does() {
    cases::<PullRequestReaction>(FIXTURES, "PullRequestReaction");
}

/// 7 cases: what zod's parse of PullRequestRecord wrote, or the first issue it raised.
#[test]
fn pull_request_record_reads_and_writes_as_zod_does() {
    cases::<PullRequestRecord>(FIXTURES, "PullRequestRecord");
}

/// 6 cases: what zod's parse of PullRequestReview wrote, or the first issue it raised.
#[test]
fn pull_request_review_reads_and_writes_as_zod_does() {
    cases::<PullRequestReview>(FIXTURES, "PullRequestReview");
}

/// 14 cases: what zod's parse of PullRequestReviewComment wrote, or the first issue it raised.
#[test]
fn pull_request_review_comment_reads_and_writes_as_zod_does() {
    cases::<PullRequestReviewComment>(FIXTURES, "PullRequestReviewComment");
}

/// 4 cases: what zod's parse of PullRequestReviewRequest wrote, or the first issue it raised.
#[test]
fn pull_request_review_request_reads_and_writes_as_zod_does() {
    cases::<PullRequestReviewRequest>(FIXTURES, "PullRequestReviewRequest");
}

/// 4 cases: what zod's parse of PullRequestSeen wrote, or the first issue it raised.
#[test]
fn pull_request_seen_reads_and_writes_as_zod_does() {
    cases::<PullRequestSeen>(FIXTURES, "PullRequestSeen");
}

/// 5 cases: what zod's parse of PullRequestSendResult wrote, or the first issue it raised.
#[test]
fn pull_request_send_result_reads_and_writes_as_zod_does() {
    cases::<PullRequestSendResult>(FIXTURES, "PullRequestSendResult");
}

/// 6 cases: what zod's parse of PullRequestSent wrote, or the first issue it raised.
#[test]
fn pull_request_sent_reads_and_writes_as_zod_does() {
    cases::<PullRequestSent>(FIXTURES, "PullRequestSent");
}

/// 4 cases: what zod's parse of PullRequestState wrote, or the first issue it raised.
#[test]
fn pull_request_state_reads_and_writes_as_zod_does() {
    cases::<PullRequestState>(FIXTURES, "PullRequestState");
}

/// 3 cases: what zod's parse of PullRequestUnread wrote, or the first issue it raised.
#[test]
fn pull_request_unread_reads_and_writes_as_zod_does() {
    cases::<PullRequestUnread>(FIXTURES, "PullRequestUnread");
}

/// 9 cases: what zod's parse of ReactionContent wrote, or the first issue it raised.
#[test]
fn reaction_content_reads_and_writes_as_zod_does() {
    cases::<ReactionContent>(FIXTURES, "ReactionContent");
}

/// 5 cases: what zod's parse of ReviewState wrote, or the first issue it raised.
#[test]
fn review_state_reads_and_writes_as_zod_does() {
    cases::<ReviewState>(FIXTURES, "ReviewState");
}
