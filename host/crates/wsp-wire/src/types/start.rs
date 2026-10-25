// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/start.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::literal;
use crate::types::check::{Check, SchemaError};
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

impl Check for GithubLinkKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["issue", "pull_request"]))
    }
}

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
        self.host.check().map_err(|e| e.under("host"))?;
        self.kind.check().map_err(|e| e.under("kind"))?;
        crate::types::check::num_at_least(self.number as f64, 0.0).map_err(|e| e.under("number"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "host" => <GithubComLit as Check>::describe(rest),
                "repo" => <String as Check>::describe(rest),
                "kind" => <GithubLinkKind as Check>::describe(rest),
                "number" => <i64 as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                _ => None,
            },
        }
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

impl Check for WorkspaceFromKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["issue", "pull_request", "review"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFromHeadFork {
    pub owner: String,
    pub pushable: bool,
}

impl Check for WorkspaceFromHeadFork {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "owner" => <String as Check>::describe(rest),
                "pushable" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFromHead {
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub oid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fork: Option<WorkspaceFromHeadFork>,
}

impl Check for WorkspaceFromHead {
    /// Nested checks: fork.
    fn check(&self) -> Result<(), SchemaError> {
        self.fork.check().map_err(|e| e.under("fork"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                "oid" => <Option<String> as Check>::describe(rest),
                "fork" => <Option<WorkspaceFromHeadFork> as Check>::describe(rest),
                _ => None,
            },
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub head: Option<WorkspaceFromHead>,
}

impl Check for WorkspaceFrom {
    /// Rules: number: >= 0.
    /// Nested checks: head.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        crate::types::check::num_at_least(self.number as f64, 0.0).map_err(|e| e.under("number"))?;
        self.head.check().map_err(|e| e.under("head"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <WorkspaceFromKind as Check>::describe(rest),
                "repo" => <String as Check>::describe(rest),
                "number" => <i64 as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "base" => <Option<String> as Check>::describe(rest),
                "head" => <Option<WorkspaceFromHead> as Check>::describe(rest),
                _ => None,
            },
        }
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
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "author" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/start.ts:76 at a3b870a6e.
/// An issue as a start's task reads it: its number, link, title, body, state and comments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueRead {
    pub number: i64,
    pub url: String,
    pub title: String,
    pub body: String,
    pub state: String,
    pub comments: Vec<IssueReadCommentsItem>,
}

impl Check for IssueRead {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.number as f64, 0.0).map_err(|e| e.under("number"))?;
        self.comments.check().map_err(|e| e.under("comments"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "number" => <i64 as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "state" => <String as Check>::describe(rest),
                "comments" => <Vec<IssueReadCommentsItem> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/start.ts:87 at a3b870a6e.
/// A git.issueRead's answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitIssueReadReply {
    pub issue: IssueRead,
}

impl Check for GitIssueReadReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.issue.check().map_err(|e| e.under("issue"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "issue" => <IssueRead as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/start.ts:92 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrDiffReply;

impl Check for GitPrDiffReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "diff" => <String as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                "left" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/start.ts:96 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrCheckoutReply;

impl Check for GitPrCheckoutReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/start.ts:100 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrReviewReply;

impl Check for GitPrReviewReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "url" => <String as Check>::describe(rest),
                "folded" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
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

impl Check for ReviewBlockVerdict {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["comment", "approve", "request_changes"]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewBlockCommentsItemSide {
    #[serde(rename = "LEFT")]
    LEFT,
    #[serde(rename = "RIGHT")]
    RIGHT,
}

impl Check for ReviewBlockCommentsItemSide {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["LEFT", "RIGHT"]))
    }
}

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
        crate::types::check::text_at_least(&self.path, 1).map_err(|e| e.under("path"))?;
        crate::types::check::num_above(self.line as f64, 0.0).map_err(|e| e.under("line"))?;
        self.side.check().map_err(|e| e.under("side"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "line" => <i64 as Check>::describe(rest),
                "side" => <ReviewBlockCommentsItemSide as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/start.ts:195 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBlock {
    #[serde(deserialize_with = "review_verdict")]
    pub verdict: ReviewBlockVerdict,
    pub summary: String,
    pub comments: Vec<ReviewBlockCommentsItem>,
}

impl Check for ReviewBlock {
    /// Rules: verdict: a zod preprocess (spec/shapes.md rule 7).
    /// Nested checks: comments.
    fn check(&self) -> Result<(), SchemaError> {
        self.verdict.check().map_err(|e| e.under("verdict"))?;
        self.comments.check().map_err(|e| e.under("comments"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "verdict" => <ReviewBlockVerdict as Check>::describe(rest),
                "summary" => <String as Check>::describe(rest),
                "comments" => <Vec<ReviewBlockCommentsItem> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewDraftCommentSide {
    #[serde(rename = "LEFT")]
    LEFT,
    #[serde(rename = "RIGHT")]
    RIGHT,
}

impl Check for ReviewDraftCommentSide {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["LEFT", "RIGHT"]))
    }
}

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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub in_summary: Option<bool>,
}

impl Check for ReviewDraftComment {
    /// Rules: line: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_above(self.line as f64, 0.0).map_err(|e| e.under("line"))?;
        self.side.check().map_err(|e| e.under("side"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "line" => <i64 as Check>::describe(rest),
                "side" => <ReviewDraftCommentSide as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "on" => <bool as Check>::describe(rest),
                "inSummary" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
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

impl Check for ReviewDraftV0Verdict {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["comment", "approve", "request_changes"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDraftV0Posted {
    pub url: String,
    pub at: JsNumber,
    pub folded: Vec<String>,
}

impl Check for ReviewDraftV0Posted {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "url" => <String as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                "folded" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub posted: Option<ReviewDraftV0Posted>,
}

impl Check for ReviewDraftV0 {
    /// Nested checks: comments, posted.
    fn check(&self) -> Result<(), SchemaError> {
        self.verdict.check().map_err(|e| e.under("verdict"))?;
        self.comments.check().map_err(|e| e.under("comments"))?;
        self.posted.check().map_err(|e| e.under("posted"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "verdict" => <ReviewDraftV0Verdict as Check>::describe(rest),
                "summary" => <String as Check>::describe(rest),
                "comments" => <Vec<ReviewDraftComment> as Check>::describe(rest),
                "headOid" => <String as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                "posted" => <Option<ReviewDraftV0Posted> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDraftV1 {
    pub note: String,
    pub at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reasked: Option<bool>,
}

impl Check for ReviewDraftV1 {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "note" => <String as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                "reasked" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
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
        match self {
            ReviewDraft::V0(v) => v.check(),
            ReviewDraft::V1(v) => v.check(),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Union); }
        <ReviewDraftV0 as Check>::describe(path).or_else(|| <ReviewDraftV1 as Check>::describe(path))
    }
}

// ---- Hand-written below: the generated part above is rewritten by the generator, this part is kept.

/// zod's `preprocess` on a review's verdict (start.ts:196): a string is lower-cased and each white space or dash
/// becomes `_` before the word is read.
pub fn review_verdict<'de, D: serde::Deserializer<'de>>(d: D) -> Result<ReviewBlockVerdict, D::Error> {
    use serde::de::Error;
    let v = serde_json::Value::deserialize(d)?;
    let v = match v {
        serde_json::Value::String(s) => {
            serde_json::Value::String(s.to_lowercase().chars().map(|c| if crate::types::check::js_space(c) || c == '-' { '_' } else { c }).collect())
        }
        other => other,
    };
    ReviewBlockVerdict::deserialize(v).map_err(D::Error::custom)
}
