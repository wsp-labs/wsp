// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/pull-request.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::literal;
use crate::types::js_number::JsNumber;
use crate::views::session::ThreadCapWait;

literal!(UpdatedLit, "updated");

/// TS: packages/protocol/src/pull-request.ts:118 at a3b870a6e.
pub const PR_POLL_MS: u64 = 180_000;

/// TS: packages/protocol/src/pull-request.ts:121 at a3b870a6e.
pub const PR_POLL_IDLE_MS: u64 = 900_000;

/// TS: packages/protocol/src/pull-request.ts:138 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const CHECK_STATE_WORDS: &[(&str, &str)] = &[
    ("pass", "Passed"),
    ("fail", "Failed"),
    ("pending", "Running"),
    ("skipped", "Skipped"),
    ("cancelled", "Cancelled"),
];

/// TS: packages/protocol/src/pull-request.ts:188 at a3b870a6e.
pub const CHECK_LOG_BYTES: u64 = 49_152;

/// TS: packages/protocol/src/pull-request.ts:240 at a3b870a6e.
pub const SENT_HUNK_LINES: u64 = 6;

/// TS: packages/protocol/src/pull-request.ts:376 at a3b870a6e.
pub const AUTO_MERGE_OFF_LINE: &str = "the repository does not merge a pull request by itself once its checks pass; turn on auto-merge in its settings, or merge it when they pass";

/// TS: packages/protocol/src/pull-request.ts:573 at a3b870a6e.
pub const REPLY_EMPTY_LINE: &str = "a reply needs words";

/// TS: packages/protocol/src/pull-request.ts:576 at a3b870a6e.
pub const PR_REPLY_BODY_MAX: u64 = 65_536;

/// TS: packages/protocol/src/pull-request.ts:10 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::PullRequestState;

impl Check for PullRequestState {}

/// TS: packages/protocol/src/pull-request.ts:13 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::Mergeable;

impl Check for Mergeable {}

/// TS: packages/protocol/src/pull-request.ts:16 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::ReviewState;

impl Check for ReviewState {}

/// TS: packages/protocol/src/pull-request.ts:19 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::CheckState;

impl Check for CheckState {}

/// TS: packages/protocol/src/pull-request.ts:22 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::MergeMethod;

impl Check for MergeMethod {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestCheckRun {
    pub run_id: i64,
    pub job_id: i64,
}

impl Check for PullRequestCheckRun {
    /// Rules: runId: >= 0; jobId: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:28 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestCheck {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    pub state: CheckState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<PullRequestCheckRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

impl Check for PullRequestCheck {
    /// Nested checks: state, run.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestFork {
    pub owner: String,
    pub pushable: bool,
}

impl Check for PullRequestFork {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestAutoMerge {
    pub method: MergeMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

impl Check for PullRequestAutoMerge {
    /// Nested checks: method.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:43 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: i64,
    pub url: String,
    pub state: PullRequestState,
    pub host: String,
    pub draft: bool,
    pub base: String,
    pub branch: String,
    pub head_oid: String,
    pub head_subject: String,
    pub mergeable: Mergeable,
    pub merge_state: String,
    pub review: ReviewState,
    pub checks: Vec<PullRequestCheck>,
    pub additions: i64,
    pub deletions: i64,
    pub changed_files: i64,
    pub commits: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind_base: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork: Option<PullRequestFork>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_merge: Option<PullRequestAutoMerge>,
}

impl Check for PullRequest {
    /// Rules: number: >= 0; additions: >= 0; deletions: >= 0; changedFiles: >= 0; commits: >= 0; behindBase: >= 0.
    /// Nested checks: state, mergeable, review, checks, fork, autoMerge.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:74 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestFact {
    pub number: i64,
    pub url: String,
    pub state: PullRequestState,
    pub host: String,
    pub draft: bool,
    pub base: String,
    pub branch: String,
    pub head_oid: String,
    pub head_subject: String,
    pub mergeable: Mergeable,
    pub merge_state: String,
    pub review: ReviewState,
    pub checks: Vec<PullRequestCheck>,
    pub additions: i64,
    pub deletions: i64,
    pub changed_files: i64,
    pub commits: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind_base: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork: Option<PullRequestFork>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_merge: Option<PullRequestAutoMerge>,
    pub read_at: JsNumber,
}

impl Check for PullRequestFact {
    /// Rules: number: >= 0; additions: >= 0; deletions: >= 0; changedFiles: >= 0; commits: >= 0; behindBase: >= 0.
    /// Nested checks: state, mergeable, review, checks.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:78 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestUnread {
    pub why: String,
    pub read_at: JsNumber,
}

impl Check for PullRequestUnread {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:84 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestRecord {
    pub number: i64,
    pub url: String,
    pub state: PullRequestState,
    pub base: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<PullRequestCheck>>,
}

impl Check for PullRequestRecord {
    /// Rules: number: >= 0.
    /// Nested checks: state, checks.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:97 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestKept {
    pub number: i64,
    pub url: String,
    pub state: PullRequestState,
    pub base: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<PullRequestCheck>>,
    pub read_at: JsNumber,
}

impl Check for PullRequestKept {
    /// Rules: number: >= 0.
    /// Nested checks: state, checks.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:102 at a3b870a6e.
/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PullRequestSeen {
    PullRequestFact(PullRequestFact),
    PullRequestKept(PullRequestKept),
    PullRequestUnread(PullRequestUnread),
}

impl Check for PullRequestSeen {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:306 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FixOutcome {
    #[serde(rename = "steered")]
    Steered,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "held")]
    Held,
    #[serde(rename = "updated")]
    Updated,
}

impl Check for FixOutcome {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FixResultV0Outcome {
    #[serde(rename = "steered")]
    Steered,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "held")]
    Held,
}

impl Check for FixResultV0Outcome {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixResultV0 {
    pub outcome: FixResultV0Outcome,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<String>,
    pub base: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capped: Option<ThreadCapWait>,
}

impl Check for FixResultV0 {
    /// Nested checks: capped.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixResultV1 {
    pub outcome: UpdatedLit,
    pub base: String,
}

impl Check for FixResultV1 {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:309 at a3b870a6e.
/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FixResult {
    V0(FixResultV0),
    V1(FixResultV1),
}

impl Check for FixResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:315 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeResult {
    pub number: i64,
    pub method: MergeMethod,
    pub merged: bool,
    pub auto_armed: bool,
}

impl Check for MergeResult {
    /// Rules: number: >= 0.
    /// Nested checks: method.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:386 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::PullRequestLabel;

impl Check for PullRequestLabel {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:390 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::PullRequestReviewRequest;

impl Check for PullRequestReviewRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:394 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::ReactionContent;

impl Check for ReactionContent {}

/// TS: packages/protocol/src/pull-request.ts:399 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::PullRequestReaction;

impl Check for PullRequestReaction {
    /// Rules: count: >= 0.
    /// Nested checks: content.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:405 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::PullRequestCommit;

impl Check for PullRequestCommit {
    /// Rules: parents: >= 0; additions: >= 0; deletions: >= 0.
    /// Nested checks: check.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:420 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReview {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub association: Option<String>,
    pub state: String,
    pub body: String,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Vec<PullRequestReaction>>,
}

impl Check for PullRequestReview {
    /// Rules: id: >= 0.
    /// Nested checks: reactions.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:435 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestComment {
    pub id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub association: Option<String>,
    pub bot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    pub body: String,
    pub url: String,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Vec<PullRequestReaction>>,
}

impl Check for PullRequestComment {
    /// Rules: id: >= 0.
    /// Nested checks: reactions.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:453 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReviewComment {
    pub id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub association: Option<String>,
    pub bot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    pub body: String,
    pub url: String,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hunk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Vec<PullRequestReaction>>,
}

impl Check for PullRequestReviewComment {
    /// Rules: id: >= 0; line: >= 0; replyTo: >= 0; reviewId: >= 0.
    /// Nested checks: reactions.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrViewReplyLatestReviewsItem {
    pub author: String,
    pub state: String,
    pub at: String,
}

impl Check for GitPrViewReplyLatestReviewsItem {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrViewReplyFilesItem {
    pub path: String,
    pub additions: i64,
    pub deletions: i64,
}

impl Check for GitPrViewReplyFilesItem {
    /// Rules: additions: >= 0; deletions: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrViewReplyCut {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commits: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviews: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comments: Option<bool>,
}

impl Check for GitPrViewReplyCut {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:479 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrViewReply {
    pub title: String,
    pub body: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_commit: Option<String>,
    pub labels: Vec<PullRequestLabel>,
    pub review_requests: Vec<PullRequestReviewRequest>,
    pub latest_reviews: Vec<GitPrViewReplyLatestReviewsItem>,
    pub assignees: Vec<String>,
    pub commits: Vec<PullRequestCommit>,
    pub reviews: Vec<PullRequestReview>,
    pub comments: Vec<PullRequestComment>,
    pub review_comments: Vec<PullRequestReviewComment>,
    pub files: Vec<GitPrViewReplyFilesItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut: Option<GitPrViewReplyCut>,
}

impl Check for GitPrViewReply {
    /// Nested checks: labels, reviewRequests, latestReviews, commits, reviews, comments, reviewComments, files, cut.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:511 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrReadReply;

impl Check for GitPrReadReply {
    /// Nested checks: pr.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:514 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitRunLogReply;

impl Check for GitRunLogReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:517 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrMergeReply;

impl Check for GitPrMergeReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:520 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitRepoReadReply;

impl Check for GitRepoReadReply {
    /// Nested checks: methods, defaultMethod.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:523 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitUpdateReply {
    pub base: String,
    pub merged: bool,
    pub commits: i64,
    pub conflicts: Vec<String>,
}

impl Check for GitUpdateReply {
    /// Rules: commits: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PullRequestItemKind {
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "reviewComment")]
    ReviewComment,
}

impl Check for PullRequestItemKind {}

/// TS: packages/protocol/src/pull-request.ts:528 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestItem {
    pub kind: PullRequestItemKind,
    pub id: i64,
}

impl Check for PullRequestItem {
    /// Rules: id: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:532 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSent {
    pub kind: PullRequestItemKind,
    pub id: i64,
    pub at: JsNumber,
}

impl Check for PullRequestSent {
    /// Rules: id: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:538 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestPage {
    pub title: String,
    pub body: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_commit: Option<String>,
    pub labels: Vec<PullRequestLabel>,
    pub review_requests: Vec<PullRequestReviewRequest>,
    pub latest_reviews: Vec<GitPrViewReplyLatestReviewsItem>,
    pub assignees: Vec<String>,
    pub commits: Vec<PullRequestCommit>,
    pub reviews: Vec<PullRequestReview>,
    pub comments: Vec<PullRequestComment>,
    pub review_comments: Vec<PullRequestReviewComment>,
    pub files: Vec<GitPrViewReplyFilesItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut: Option<GitPrViewReplyCut>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<GitRepoReadReply>,
    pub sent: Vec<PullRequestSent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posts_as_you: Option<bool>,
}

impl Check for PullRequestPage {
    /// Nested checks: labels, reviewRequests, commits, reviews, comments, reviewComments, merge, sent.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PullRequestSendResultOutcome {
    #[serde(rename = "steered")]
    Steered,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "held")]
    Held,
}

impl Check for PullRequestSendResultOutcome {}

/// TS: packages/protocol/src/pull-request.ts:550 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSendResult {
    pub outcome: PullRequestSendResultOutcome,
    pub thread_id: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capped: Option<ThreadCapWait>,
    pub sent: Vec<PullRequestSent>,
}

impl Check for PullRequestSendResult {
    /// Nested checks: capped, sent.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:561 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrReplyReply;

impl Check for GitPrReplyReply {
    /// Nested checks: comment, reviewComment.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:565 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrResolveReply;

impl Check for GitPrResolveReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/pull-request.ts:569 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrReactReply;

impl Check for GitPrReactReply {
    /// Nested checks: reactions.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
