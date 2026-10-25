// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of start.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::start::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_start.json");

/// 3 cases: what zod's parse of GitIssueReadReply wrote, or the first issue it raised.
#[test]
fn git_issue_read_reply_reads_and_writes_as_zod_does() {
    cases::<GitIssueReadReply>(FIXTURES, "GitIssueReadReply");
}

/// 3 cases: what zod's parse of GitPrCheckoutReply wrote, or the first issue it raised.
#[test]
fn git_pr_checkout_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrCheckoutReply>(FIXTURES, "GitPrCheckoutReply");
}

/// 4 cases: what zod's parse of GitPrDiffReply wrote, or the first issue it raised.
#[test]
fn git_pr_diff_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrDiffReply>(FIXTURES, "GitPrDiffReply");
}

/// 3 cases: what zod's parse of GitPrReviewReply wrote, or the first issue it raised.
#[test]
fn git_pr_review_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrReviewReply>(FIXTURES, "GitPrReviewReply");
}

/// 6 cases: what zod's parse of GithubLink wrote, or the first issue it raised.
#[test]
fn github_link_reads_and_writes_as_zod_does() {
    cases::<GithubLink>(FIXTURES, "GithubLink");
}

/// 5 cases: what zod's parse of IssueRead wrote, or the first issue it raised.
#[test]
fn issue_read_reads_and_writes_as_zod_does() {
    cases::<IssueRead>(FIXTURES, "IssueRead");
}

/// 3 cases: what zod's parse of ReviewBlock wrote, or the first issue it raised.
#[test]
fn review_block_reads_and_writes_as_zod_does() {
    cases::<ReviewBlock>(FIXTURES, "ReviewBlock");
}

/// 3 cases: what zod's parse of ReviewDraft wrote, or the first issue it raised.
#[test]
fn review_draft_reads_and_writes_as_zod_does() {
    cases::<ReviewDraft>(FIXTURES, "ReviewDraft");
}

/// 9 cases: what zod's parse of ReviewDraftComment wrote, or the first issue it raised.
#[test]
fn review_draft_comment_reads_and_writes_as_zod_does() {
    cases::<ReviewDraftComment>(FIXTURES, "ReviewDraftComment");
}

/// 7 cases: what zod's parse of WorkspaceFrom wrote, or the first issue it raised.
#[test]
fn workspace_from_reads_and_writes_as_zod_does() {
    cases::<WorkspaceFrom>(FIXTURES, "WorkspaceFrom");
}
