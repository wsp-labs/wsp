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
        todo!()
    }
}

/// TS: packages/protocol/src/tree.ts:17 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitBranchCompareReply;

impl Check for GitBranchCompareReply {
    /// Rules: aheadBy: >= 0; behindBy: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
    }
}
