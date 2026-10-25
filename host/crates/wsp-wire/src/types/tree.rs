// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/tree.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;

/// TS: packages/protocol/src/tree.ts:149 at a3b870a6e.
pub const FIX_CHECK_OR_CHILD: &str = "name a failed check or a child to merge, not both";

/// TS: packages/protocol/src/tree.ts:12 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitStartOnReply;

impl Check for GitStartOnReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                "oid" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/tree.ts:17 at a3b870a6e.
/// How far a head is from a base on the git host: whether it is pushed, and the counts and the host's word when it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchCompareReply {
    pub pushed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ahead_by: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind_by: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl Check for GitBranchCompareReply {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.ahead_by { crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("aheadBy"))?; }
        if let Some(v) = self.behind_by { crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("behindBy"))?; }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pushed" => <bool as Check>::describe(rest),
                "aheadBy" => <Option<i64> as Check>::describe(rest),
                "behindBy" => <Option<i64> as Check>::describe(rest),
                "status" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/tree.ts:22 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitMergeInReply {
    pub branch: String,
    pub merged: bool,
    pub commits: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    pub conflicts: Vec<String>,
}

impl Check for GitMergeInReply {
    /// Rules: commits: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.commits as f64, 0.0).map_err(|e| e.under("commits"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                "merged" => <bool as Check>::describe(rest),
                "commits" => <i64 as Check>::describe(rest),
                "oid" => <Option<String> as Check>::describe(rest),
                "head" => <Option<String> as Check>::describe(rest),
                "conflicts" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeRecordMerged {
    pub oid: String,
    pub at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
}

impl Check for TreeRecordMerged {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "oid" => <String as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                "head" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/tree.ts:27 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged: Option<TreeRecordMerged>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_refused: Option<String>,
}

impl Check for TreeRecord {
    /// Nested checks: merged.
    fn check(&self) -> Result<(), SchemaError> {
        self.merged.check().map_err(|e| e.under("merged"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "merged" => <Option<TreeRecordMerged> as Check>::describe(rest),
                "conflicts" => <Option<Vec<String>> as Check>::describe(rest),
                "pushRefused" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/tree.ts:38 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeChild {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged: Option<TreeRecordMerged>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_refused: Option<String>,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ahead_of_lead: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind_lead: Option<i64>,
}

impl Check for TreeChild {
    /// Rules: aheadOfLead: >= 0; behindLead: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        self.merged.check().map_err(|e| e.under("merged"))?;
        if let Some(v) = self.ahead_of_lead {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("aheadOfLead"))?;
        }
        if let Some(v) = self.behind_lead {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("behindLead"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "merged" => <Option<TreeRecordMerged> as Check>::describe(rest),
                "conflicts" => <Option<Vec<String>> as Check>::describe(rest),
                "pushRefused" => <Option<String> as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "pushed" => <Option<bool> as Check>::describe(rest),
                "aheadOfLead" => <Option<i64> as Check>::describe(rest),
                "behindLead" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/tree.ts:51 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeFact {
    pub lead_branch: String,
    pub children: Vec<TreeChild>,
    pub read_at: JsNumber,
}

impl Check for TreeFact {
    /// Nested checks: children.
    fn check(&self) -> Result<(), SchemaError> {
        self.children.check().map_err(|e| e.under("children"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "leadBranch" => <String as Check>::describe(rest),
                "children" => <Vec<TreeChild> as Check>::describe(rest),
                "readAt" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/tree.ts:55 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeInResult {
    pub lead: String,
    pub child: String,
    pub branch: String,
    pub merged: bool,
    pub commits: i64,
    pub conflicts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unpushed: Option<i64>,
}

impl Check for MergeInResult {
    /// Rules: commits: >= 0; unpushed: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.commits as f64, 0.0).map_err(|e| e.under("commits"))?;
        if let Some(v) = self.unpushed {
            crate::types::check::num_at_least(v as f64, 0.0).map_err(|e| e.under("unpushed"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "lead" => <String as Check>::describe(rest),
                "child" => <String as Check>::describe(rest),
                "branch" => <String as Check>::describe(rest),
                "merged" => <bool as Check>::describe(rest),
                "commits" => <i64 as Check>::describe(rest),
                "conflicts" => <Vec<String> as Check>::describe(rest),
                "unpushed" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
