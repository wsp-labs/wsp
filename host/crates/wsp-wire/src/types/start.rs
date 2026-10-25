// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/start.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::literal;
use crate::types::js_number::JsNumber;

literal!(GithubComLit, "github.com");

/// TS: packages/protocol/src/start.ts:104 at a3b870a6e.
pub const REVIEW_DIFF_MAX_BYTES: u64 = 98_304;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GithubLinkKind {
    #[serde(rename = "issue")]
    Issue,
    #[serde(rename = "pull_request")]
    PullRequest,
}

impl Check for GithubLinkKind {}

/// TS: packages/protocol/src/start.ts:12 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubLink {
    pub host: GithubComLit,
    pub repo: String,
    pub kind: GithubLinkKind,
    pub number: i64,
    pub url: String,
}

impl Check for GithubLink {
    /// Rules: number: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceFromKind {
    #[serde(rename = "issue")]
    Issue,
    #[serde(rename = "pull_request")]
    PullRequest,
    #[serde(rename = "review")]
    Review,
}

impl Check for WorkspaceFromKind {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFromHeadFork {
    pub owner: String,
    pub pushable: bool,
}

impl Check for WorkspaceFromHeadFork {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFromHead {
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork: Option<WorkspaceFromHeadFork>,
}

impl Check for WorkspaceFromHead {
    /// Nested checks: fork.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/start.ts:64 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFrom {
    pub kind: WorkspaceFromKind,
    pub repo: String,
    pub number: i64,
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<WorkspaceFromHead>,
}

impl Check for WorkspaceFrom {
    /// Rules: number: >= 0.
    /// Nested checks: head.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueReadCommentsItem {
    pub author: String,
    pub body: String,
    pub at: String,
}

impl Check for IssueReadCommentsItem {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/start.ts:76 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::IssueRead;

impl Check for IssueRead {
    /// Rules: number: >= 0.
    /// Nested checks: comments.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/start.ts:87 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitIssueReadReply;

impl Check for GitIssueReadReply {
    /// Nested checks: issue.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/start.ts:92 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrDiffReply;

impl Check for GitPrDiffReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/start.ts:96 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrCheckoutReply;

impl Check for GitPrCheckoutReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/start.ts:100 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrReviewReply;

impl Check for GitPrReviewReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewBlockVerdict {
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "approve")]
    Approve,
    #[serde(rename = "request_changes")]
    RequestChanges,
}

impl Check for ReviewBlockVerdict {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewBlockCommentsItemSide {
    #[serde(rename = "LEFT")]
    LEFT,
    #[serde(rename = "RIGHT")]
    RIGHT,
}

impl Check for ReviewBlockCommentsItemSide {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBlockCommentsItem {
    pub path: String,
    pub line: i64,
    pub side: ReviewBlockCommentsItemSide,
    pub body: String,
}

impl Check for ReviewBlockCommentsItem {
    /// Rules: path: at least 1 UTF-16 units; line: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/start.ts:195 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBlock {
    pub verdict: ReviewBlockVerdict,
    pub summary: String,
    pub comments: Vec<ReviewBlockCommentsItem>,
}

impl Check for ReviewBlock {
    /// Rules: verdict: a zod preprocess (spec/shapes.md rule 7).
    /// Nested checks: comments.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewDraftCommentSide {
    #[serde(rename = "LEFT")]
    LEFT,
    #[serde(rename = "RIGHT")]
    RIGHT,
}

impl Check for ReviewDraftCommentSide {}

/// TS: packages/protocol/src/start.ts:253 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDraftComment {
    pub id: String,
    pub path: String,
    pub line: i64,
    pub side: ReviewDraftCommentSide,
    pub body: String,
    pub on: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_summary: Option<bool>,
}

impl Check for ReviewDraftComment {
    /// Rules: line: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewDraftV0Verdict {
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "approve")]
    Approve,
    #[serde(rename = "request_changes")]
    RequestChanges,
}

impl Check for ReviewDraftV0Verdict {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDraftV0Posted {
    pub url: String,
    pub at: JsNumber,
    pub folded: Vec<String>,
}

impl Check for ReviewDraftV0Posted {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDraftV0 {
    pub verdict: ReviewDraftV0Verdict,
    pub summary: String,
    pub comments: Vec<ReviewDraftComment>,
    pub head_oid: String,
    pub thread_id: String,
    pub at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posted: Option<ReviewDraftV0Posted>,
}

impl Check for ReviewDraftV0 {
    /// Nested checks: comments, posted.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDraftV1 {
    pub note: String,
    pub at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasked: Option<bool>,
}

impl Check for ReviewDraftV1 {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/start.ts:266 at a3b870a6e.
/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ReviewDraft {
    V0(ReviewDraftV0),
    V1(ReviewDraftV1),
}

impl Check for ReviewDraft {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
