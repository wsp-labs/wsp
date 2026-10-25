// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/pull-request.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::literal;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::views::session::ThreadCapWait;

literal!(UpdatedLit, "updated");

/// TS: packages/protocol/src/pull-request.ts:118 at a3b870a6e.
pub const PR_POLL_MS: u64 = 180_000;

/// TS: packages/protocol/src/pull-request.ts:121 at a3b870a6e.
pub const PR_POLL_IDLE_MS: u64 = 900_000;

/// TS: packages/protocol/src/pull-request.ts:138 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const CHECK_STATE_WORDS: &[(&str, &str)] =
    &[("pass", "Passed"), ("fail", "Failed"), ("pending", "Running"), ("skipped", "Skipped"), ("cancelled", "Cancelled")];

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

impl Check for PullRequestState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["open", "merged", "closed"]))
    }
}

/// TS: packages/protocol/src/pull-request.ts:13 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::Mergeable;

impl Check for Mergeable {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["mergeable", "conflicting", "unknown"]))
    }
}

/// TS: packages/protocol/src/pull-request.ts:16 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::ReviewState;

impl Check for ReviewState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["none", "approved", "changes_asked", "required"]))
    }
}

/// TS: packages/protocol/src/pull-request.ts:19 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::CheckState;

impl Check for CheckState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["pass", "fail", "pending", "skipped", "cancelled"]))
    }
}

/// TS: packages/protocol/src/pull-request.ts:22 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::MergeMethod;

impl Check for MergeMethod {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["merge", "squash", "rebase"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestCheckRun {
    pub run_id: i64,
    pub job_id: i64,
}

impl Check for PullRequestCheckRun {
    /// Rules: runId: >= 0; jobId: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.run_id as f64, 0.0).map_err(|e| e.under("runId"))?;
        crate::types::check::num_at_least(self.job_id as f64, 0.0).map_err(|e| e.under("jobId"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "runId" => <i64 as Check>::describe(rest),
                "jobId" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:28 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestCheck {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub workflow: Option<String>,
    pub state: CheckState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub run: Option<PullRequestCheckRun>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub completed_at: Option<String>,
}

impl Check for PullRequestCheck {
    /// Nested checks: state, run.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        self.run.check().map_err(|e| e.under("run"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "workflow" => <Option<String> as Check>::describe(rest),
                "state" => <CheckState as Check>::describe(rest),
                "run" => <Option<PullRequestCheckRun> as Check>::describe(rest),
                "link" => <Option<String> as Check>::describe(rest),
                "description" => <Option<String> as Check>::describe(rest),
                "startedAt" => <Option<String> as Check>::describe(rest),
                "completedAt" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
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
pub struct PullRequestAutoMerge {
    pub method: MergeMethod,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub by: Option<String>,
}

impl Check for PullRequestAutoMerge {
    /// Nested checks: method.
    fn check(&self) -> Result<(), SchemaError> {
        self.method.check().map_err(|e| e.under("method"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "method" => <MergeMethod as Check>::describe(rest),
                "by" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub behind_base: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fork: Option<PullRequestFork>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub auto_merge: Option<PullRequestAutoMerge>,
}

impl Check for PullRequest {
    /// Rules: number: >= 0; additions: >= 0; deletions: >= 0; changedFiles: >= 0; commits: >= 0; behindBase: >= 0.
    /// Nested checks: state, mergeable, review, checks, fork, autoMerge.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.number as f64, 0.0).map_err(|e| e.under("number"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        self.mergeable.check().map_err(|e| e.under("mergeable"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.checks.check().map_err(|e| e.under("checks"))?;
        crate::types::check::num_at_least(self.additions as f64, 0.0).map_err(|e| e.under("additions"))?;
        crate::types::check::num_at_least(self.deletions as f64, 0.0).map_err(|e| e.under("deletions"))?;
        crate::types::check::num_at_least(self.changed_files as f64, 0.0).map_err(|e| e.under("changedFiles"))?;
        crate::types::check::num_at_least(self.commits as f64, 0.0).map_err(|e| e.under("commits"))?;
        if let Some(v) = self.behind_base {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("behindBase"))?;
        }
        self.fork.check().map_err(|e| e.under("fork"))?;
        self.auto_merge.check().map_err(|e| e.under("autoMerge"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "number" => <i64 as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "state" => <PullRequestState as Check>::describe(rest),
                "host" => <String as Check>::describe(rest),
                "draft" => <bool as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "headOid" => <String as Check>::describe(rest),
                "headSubject" => <String as Check>::describe(rest),
                "mergeable" => <Mergeable as Check>::describe(rest),
                "mergeState" => <String as Check>::describe(rest),
                "review" => <ReviewState as Check>::describe(rest),
                "checks" => <Vec<PullRequestCheck> as Check>::describe(rest),
                "additions" => <i64 as Check>::describe(rest),
                "deletions" => <i64 as Check>::describe(rest),
                "changedFiles" => <i64 as Check>::describe(rest),
                "commits" => <i64 as Check>::describe(rest),
                "behindBase" => <Option<i64> as Check>::describe(rest),
                "author" => <Option<String> as Check>::describe(rest),
                "fork" => <Option<PullRequestFork> as Check>::describe(rest),
                "autoMerge" => <Option<PullRequestAutoMerge> as Check>::describe(rest),
                _ => None,
            },
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub behind_base: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fork: Option<PullRequestFork>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub auto_merge: Option<PullRequestAutoMerge>,
    pub read_at: JsNumber,
}

impl Check for PullRequestFact {
    /// Rules: number: >= 0; additions: >= 0; deletions: >= 0; changedFiles: >= 0; commits: >= 0; behindBase: >= 0.
    /// Nested checks: state, mergeable, review, checks.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.number as f64, 0.0).map_err(|e| e.under("number"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        self.mergeable.check().map_err(|e| e.under("mergeable"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.checks.check().map_err(|e| e.under("checks"))?;
        crate::types::check::num_at_least(self.additions as f64, 0.0).map_err(|e| e.under("additions"))?;
        crate::types::check::num_at_least(self.deletions as f64, 0.0).map_err(|e| e.under("deletions"))?;
        crate::types::check::num_at_least(self.changed_files as f64, 0.0).map_err(|e| e.under("changedFiles"))?;
        crate::types::check::num_at_least(self.commits as f64, 0.0).map_err(|e| e.under("commits"))?;
        if let Some(v) = self.behind_base {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("behindBase"))?;
        }
        self.fork.check().map_err(|e| e.under("fork"))?;
        self.auto_merge.check().map_err(|e| e.under("autoMerge"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "number" => <i64 as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "state" => <PullRequestState as Check>::describe(rest),
                "host" => <String as Check>::describe(rest),
                "draft" => <bool as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "headOid" => <String as Check>::describe(rest),
                "headSubject" => <String as Check>::describe(rest),
                "mergeable" => <Mergeable as Check>::describe(rest),
                "mergeState" => <String as Check>::describe(rest),
                "review" => <ReviewState as Check>::describe(rest),
                "checks" => <Vec<PullRequestCheck> as Check>::describe(rest),
                "additions" => <i64 as Check>::describe(rest),
                "deletions" => <i64 as Check>::describe(rest),
                "changedFiles" => <i64 as Check>::describe(rest),
                "commits" => <i64 as Check>::describe(rest),
                "behindBase" => <Option<i64> as Check>::describe(rest),
                "author" => <Option<String> as Check>::describe(rest),
                "fork" => <Option<PullRequestFork> as Check>::describe(rest),
                "autoMerge" => <Option<PullRequestAutoMerge> as Check>::describe(rest),
                "readAt" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
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
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "why" => <String as Check>::describe(rest),
                "readAt" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub closed_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub checks: Option<Vec<PullRequestCheck>>,
}

impl Check for PullRequestRecord {
    /// Rules: number: >= 0.
    /// Nested checks: state, checks.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.number as f64, 0.0).map_err(|e| e.under("number"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        self.checks.check().map_err(|e| e.under("checks"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "number" => <i64 as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "state" => <PullRequestState as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "mergedAt" => <Option<JsNumber> as Check>::describe(rest),
                "closedAt" => <Option<JsNumber> as Check>::describe(rest),
                "checks" => <Option<Vec<PullRequestCheck>> as Check>::describe(rest),
                _ => None,
            },
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub closed_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub checks: Option<Vec<PullRequestCheck>>,
    pub read_at: JsNumber,
}

impl Check for PullRequestKept {
    /// Rules: number: >= 0.
    /// Nested checks: state, checks.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.number as f64, 0.0).map_err(|e| e.under("number"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        self.checks.check().map_err(|e| e.under("checks"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "number" => <i64 as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "state" => <PullRequestState as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "mergedAt" => <Option<JsNumber> as Check>::describe(rest),
                "closedAt" => <Option<JsNumber> as Check>::describe(rest),
                "checks" => <Option<Vec<PullRequestCheck>> as Check>::describe(rest),
                "readAt" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
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
        match self {
            PullRequestSeen::PullRequestFact(v) => v.check(),
            PullRequestSeen::PullRequestKept(v) => v.check(),
            PullRequestSeen::PullRequestUnread(v) => v.check(),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Union); }
        <PullRequestFact as Check>::describe(path).or_else(|| <PullRequestKept as Check>::describe(path)).or_else(|| <PullRequestUnread as Check>::describe(path))
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

impl Check for FixOutcome {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["steered", "queued", "started", "held", "updated"]))
    }
}

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

impl Check for FixResultV0Outcome {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["steered", "queued", "started", "held"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixResultV0 {
    pub outcome: FixResultV0Outcome,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub check: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub child: Option<String>,
    pub base: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub capped: Option<ThreadCapWait>,
}

impl Check for FixResultV0 {
    /// Nested checks: capped.
    fn check(&self) -> Result<(), SchemaError> {
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        self.capped.check().map_err(|e| e.under("capped"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "outcome" => <FixResultV0Outcome as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "check" => <Option<String> as Check>::describe(rest),
                "child" => <Option<String> as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "agent" => <String as Check>::describe(rest),
                "capped" => <Option<ThreadCapWait> as Check>::describe(rest),
                _ => None,
            },
        }
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
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "outcome" => <UpdatedLit as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                _ => None,
            },
        }
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
        match self {
            FixResult::V0(v) => v.check(),
            FixResult::V1(v) => v.check(),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Union); }
        <FixResultV0 as Check>::describe(path).or_else(|| <FixResultV1 as Check>::describe(path))
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
        crate::types::check::num_at_least(self.number as f64, 0.0).map_err(|e| e.under("number"))?;
        self.method.check().map_err(|e| e.under("method"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "number" => <i64 as Check>::describe(rest),
                "method" => <MergeMethod as Check>::describe(rest),
                "merged" => <bool as Check>::describe(rest),
                "autoArmed" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:386 at a3b870a6e.
/// A label on a pull request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestLabel {
    pub name: String,
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub description: Option<String>,
}

impl Check for PullRequestLabel {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "color" => <String as Check>::describe(rest),
                "description" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:390 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::PullRequestReviewRequest;

impl Check for PullRequestReviewRequest {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "team" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:394 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::ReactionContent;

impl Check for ReactionContent {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["+1", "-1", "laugh", "hooray", "confused", "heart", "rocket", "eyes"]))
    }
}

/// TS: packages/protocol/src/pull-request.ts:399 at a3b870a6e.
/// One reaction left on an item: which, how many left it, and whether the person is one of them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReaction {
    pub content: ReactionContent,
    pub count: i64,
    pub mine: bool,
}

impl Check for PullRequestReaction {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.count as f64, 0.0).map_err(|e| e.under("count"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "content" => <ReactionContent as Check>::describe(rest),
                "count" => <i64 as Check>::describe(rest),
                "mine" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:405 at a3b870a6e.
/// One commit of a pull request, with its counts where the host answered for them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestCommit {
    pub oid: String,
    pub subject: String,
    pub body: String,
    pub at: String,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub additions: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub deletions: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub check: Option<CheckState>,
}

impl Check for PullRequestCommit {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.parents { crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("parents"))?; }
        if let Some(v) = self.additions { crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("additions"))?; }
        if let Some(v) = self.deletions { crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("deletions"))?; }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "oid" => <String as Check>::describe(rest),
                "subject" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "parents" => <Option<i64> as Check>::describe(rest),
                "additions" => <Option<i64> as Check>::describe(rest),
                "deletions" => <Option<i64> as Check>::describe(rest),
                "check" => <Option<CheckState> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:420 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReview {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub node_id: Option<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub association: Option<String>,
    pub state: String,
    pub body: String,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reactions: Option<Vec<PullRequestReaction>>,
}

impl Check for PullRequestReview {
    /// Rules: id: >= 0.
    /// Nested checks: reactions.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.id {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("id"))?;
        }
        self.reactions.check().map_err(|e| e.under("reactions"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <Option<i64> as Check>::describe(rest),
                "nodeId" => <Option<String> as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "association" => <Option<String> as Check>::describe(rest),
                "state" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "reactions" => <Option<Vec<PullRequestReaction>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:435 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestComment {
    pub id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub node_id: Option<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub association: Option<String>,
    pub bot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub avatar: Option<String>,
    pub body: String,
    pub url: String,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reactions: Option<Vec<PullRequestReaction>>,
}

impl Check for PullRequestComment {
    /// Rules: id: >= 0.
    /// Nested checks: reactions.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.id as f64, 0.0).map_err(|e| e.under("id"))?;
        self.reactions.check().map_err(|e| e.under("reactions"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <i64 as Check>::describe(rest),
                "nodeId" => <Option<String> as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "association" => <Option<String> as Check>::describe(rest),
                "bot" => <bool as Check>::describe(rest),
                "avatar" => <Option<String> as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "reactions" => <Option<Vec<PullRequestReaction>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:453 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReviewComment {
    pub id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub node_id: Option<String>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub side: Option<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub association: Option<String>,
    pub bot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub avatar: Option<String>,
    pub body: String,
    pub url: String,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub hunk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reply_to: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resolved: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reactions: Option<Vec<PullRequestReaction>>,
}

impl Check for PullRequestReviewComment {
    /// Rules: id: >= 0; line: >= 0; replyTo: >= 0; reviewId: >= 0.
    /// Nested checks: reactions.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.id as f64, 0.0).map_err(|e| e.under("id"))?;
        if let Some(v) = self.line {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("line"))?;
        }
        if let Some(v) = self.reply_to {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("replyTo"))?;
        }
        if let Some(v) = self.review_id {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("reviewId"))?;
        }
        self.reactions.check().map_err(|e| e.under("reactions"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <i64 as Check>::describe(rest),
                "nodeId" => <Option<String> as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "line" => <Option<i64> as Check>::describe(rest),
                "side" => <Option<String> as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "association" => <Option<String> as Check>::describe(rest),
                "bot" => <bool as Check>::describe(rest),
                "avatar" => <Option<String> as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "hunk" => <Option<String> as Check>::describe(rest),
                "replyTo" => <Option<i64> as Check>::describe(rest),
                "reviewId" => <Option<i64> as Check>::describe(rest),
                "resolved" => <Option<bool> as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "reactions" => <Option<Vec<PullRequestReaction>> as Check>::describe(rest),
                _ => None,
            },
        }
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
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "author" => <String as Check>::describe(rest),
                "state" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                _ => None,
            },
        }
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
        crate::types::check::num_at_least(self.additions as f64, 0.0).map_err(|e| e.under("additions"))?;
        crate::types::check::num_at_least(self.deletions as f64, 0.0).map_err(|e| e.under("deletions"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "additions" => <i64 as Check>::describe(rest),
                "deletions" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrViewReplyCut {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub commits: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reviews: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub threads: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub comments: Option<bool>,
}

impl Check for GitPrViewReplyCut {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "commits" => <Option<bool> as Check>::describe(rest),
                "reviews" => <Option<bool> as Check>::describe(rest),
                "threads" => <Option<bool> as Check>::describe(rest),
                "comments" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub closed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cut: Option<GitPrViewReplyCut>,
}

impl Check for GitPrViewReply {
    /// Nested checks: labels, reviewRequests, latestReviews, commits, reviews, comments, reviewComments, files, cut.
    fn check(&self) -> Result<(), SchemaError> {
        self.labels.check().map_err(|e| e.under("labels"))?;
        self.review_requests.check().map_err(|e| e.under("reviewRequests"))?;
        self.latest_reviews.check().map_err(|e| e.under("latestReviews"))?;
        self.commits.check().map_err(|e| e.under("commits"))?;
        self.reviews.check().map_err(|e| e.under("reviews"))?;
        self.comments.check().map_err(|e| e.under("comments"))?;
        self.review_comments.check().map_err(|e| e.under("reviewComments"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        self.cut.check().map_err(|e| e.under("cut"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "title" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "updatedAt" => <String as Check>::describe(rest),
                "closedAt" => <Option<String> as Check>::describe(rest),
                "mergedAt" => <Option<String> as Check>::describe(rest),
                "mergedBy" => <Option<String> as Check>::describe(rest),
                "mergeCommit" => <Option<String> as Check>::describe(rest),
                "labels" => <Vec<PullRequestLabel> as Check>::describe(rest),
                "reviewRequests" => <Vec<PullRequestReviewRequest> as Check>::describe(rest),
                "latestReviews" => <Vec<GitPrViewReplyLatestReviewsItem> as Check>::describe(rest),
                "assignees" => <Vec<String> as Check>::describe(rest),
                "commits" => <Vec<PullRequestCommit> as Check>::describe(rest),
                "reviews" => <Vec<PullRequestReview> as Check>::describe(rest),
                "comments" => <Vec<PullRequestComment> as Check>::describe(rest),
                "reviewComments" => <Vec<PullRequestReviewComment> as Check>::describe(rest),
                "files" => <Vec<GitPrViewReplyFilesItem> as Check>::describe(rest),
                "cut" => <Option<GitPrViewReplyCut> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:511 at a3b870a6e.
/// A pull request read: the pull request, the step it last saw, and whether nothing changed since.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrReadReply {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pr: Option<PullRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub seen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub unchanged: Option<bool>,
}

impl Check for GitPrReadReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.pr.check().map_err(|e| e.under("pr"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pr" => <Option<PullRequest> as Check>::describe(rest),
                "seen" => <Option<String> as Check>::describe(rest),
                "unchanged" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:514 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitRunLogReply;

impl Check for GitRunLogReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "lines" => <Vec<String> as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:517 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrMergeReply;

impl Check for GitPrMergeReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "merged" => <bool as Check>::describe(rest),
                "autoArmed" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:520 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitRepoReadReply;

impl Check for GitRepoReadReply {
    /// Nested checks: methods, defaultMethod.
    fn check(&self) -> Result<(), SchemaError> {
        self.methods.check().map_err(|e| e.under("methods"))?;
        self.default_method.check().map_err(|e| e.under("defaultMethod"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "methods" => <Vec<MergeMethod> as Check>::describe(rest),
                "defaultMethod" => <MergeMethod as Check>::describe(rest),
                "autoMerge" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
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
        crate::types::check::num_at_least(self.commits as f64, 0.0).map_err(|e| e.under("commits"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "base" => <String as Check>::describe(rest),
                "merged" => <bool as Check>::describe(rest),
                "commits" => <i64 as Check>::describe(rest),
                "conflicts" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
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

impl Check for PullRequestItemKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["comment", "review", "reviewComment"]))
    }
}

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
        self.kind.check().map_err(|e| e.under("kind"))?;
        crate::types::check::num_at_least(self.id as f64, 0.0).map_err(|e| e.under("id"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <PullRequestItemKind as Check>::describe(rest),
                "id" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
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
        self.kind.check().map_err(|e| e.under("kind"))?;
        crate::types::check::num_at_least(self.id as f64, 0.0).map_err(|e| e.under("id"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <PullRequestItemKind as Check>::describe(rest),
                "id" => <i64 as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub closed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cut: Option<GitPrViewReplyCut>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merge: Option<GitRepoReadReply>,
    pub sent: Vec<PullRequestSent>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub posts_as_you: Option<bool>,
}

impl Check for PullRequestPage {
    /// Nested checks: labels, reviewRequests, commits, reviews, comments, reviewComments, merge, sent.
    fn check(&self) -> Result<(), SchemaError> {
        self.labels.check().map_err(|e| e.under("labels"))?;
        self.review_requests.check().map_err(|e| e.under("reviewRequests"))?;
        self.latest_reviews.check().map_err(|e| e.under("latestReviews"))?;
        self.commits.check().map_err(|e| e.under("commits"))?;
        self.reviews.check().map_err(|e| e.under("reviews"))?;
        self.comments.check().map_err(|e| e.under("comments"))?;
        self.review_comments.check().map_err(|e| e.under("reviewComments"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        self.cut.check().map_err(|e| e.under("cut"))?;
        self.merge.check().map_err(|e| e.under("merge"))?;
        self.sent.check().map_err(|e| e.under("sent"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "title" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "updatedAt" => <String as Check>::describe(rest),
                "closedAt" => <Option<String> as Check>::describe(rest),
                "mergedAt" => <Option<String> as Check>::describe(rest),
                "mergedBy" => <Option<String> as Check>::describe(rest),
                "mergeCommit" => <Option<String> as Check>::describe(rest),
                "labels" => <Vec<PullRequestLabel> as Check>::describe(rest),
                "reviewRequests" => <Vec<PullRequestReviewRequest> as Check>::describe(rest),
                "latestReviews" => <Vec<GitPrViewReplyLatestReviewsItem> as Check>::describe(rest),
                "assignees" => <Vec<String> as Check>::describe(rest),
                "commits" => <Vec<PullRequestCommit> as Check>::describe(rest),
                "reviews" => <Vec<PullRequestReview> as Check>::describe(rest),
                "comments" => <Vec<PullRequestComment> as Check>::describe(rest),
                "reviewComments" => <Vec<PullRequestReviewComment> as Check>::describe(rest),
                "files" => <Vec<GitPrViewReplyFilesItem> as Check>::describe(rest),
                "cut" => <Option<GitPrViewReplyCut> as Check>::describe(rest),
                "merge" => <Option<GitRepoReadReply> as Check>::describe(rest),
                "sent" => <Vec<PullRequestSent> as Check>::describe(rest),
                "postsAsYou" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
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

impl Check for PullRequestSendResultOutcome {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["steered", "queued", "started", "held"]))
    }
}

/// TS: packages/protocol/src/pull-request.ts:550 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSendResult {
    pub outcome: PullRequestSendResultOutcome,
    pub thread_id: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub capped: Option<ThreadCapWait>,
    pub sent: Vec<PullRequestSent>,
}

impl Check for PullRequestSendResult {
    /// Nested checks: capped, sent.
    fn check(&self) -> Result<(), SchemaError> {
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        self.capped.check().map_err(|e| e.under("capped"))?;
        self.sent.check().map_err(|e| e.under("sent"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "outcome" => <PullRequestSendResultOutcome as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "agent" => <String as Check>::describe(rest),
                "capped" => <Option<ThreadCapWait> as Check>::describe(rest),
                "sent" => <Vec<PullRequestSent> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:561 at a3b870a6e.
/// The comment a reply made, on the conversation or on a line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrReplyReply {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub comment: Option<PullRequestComment>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review_comment: Option<PullRequestReviewComment>,
}

impl Check for GitPrReplyReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.comment.check().map_err(|e| e.under("comment"))?;
        self.review_comment.check().map_err(|e| e.under("reviewComment"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "comment" => <Option<PullRequestComment> as Check>::describe(rest),
                "reviewComment" => <Option<PullRequestReviewComment> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:565 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrResolveReply;

impl Check for GitPrResolveReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threadId" => <String as Check>::describe(rest),
                "resolved" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:569 at a3b870a6e.
/// A reaction added or taken off as the person: the item's node id and every reaction on it now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrReactReply {
    pub subject: String,
    pub reactions: Vec<PullRequestReaction>,
}

impl Check for GitPrReactReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.reactions.check().map_err(|e| e.under("reactions"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "subject" => <String as Check>::describe(rest),
                "reactions" => <Vec<PullRequestReaction> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
