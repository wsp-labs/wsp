// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/tree.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ahead_by: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub behind_by: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub status: Option<String>,
}

impl Check for GitBranchCompareReply {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.ahead_by { crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("aheadBy"))?; }
        if let Some(v) = self.behind_by { crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("behindBy"))?; }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("pushed") { None => out.push(crate::types::check::missing::<Self>("pushed")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pushed"))), } } }
        if let Some(v) = obj.get("aheadBy") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_ahead_by(&x) { out.push(e.under("aheadBy")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("aheadBy"))), } }
        if let Some(v) = obj.get("behindBy") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_behind_by(&x) { out.push(e.under("behindBy")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("behindBy"))), } }
        if let Some(v) = obj.get("status") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } }
        Some(out)
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
impl GitBranchCompareReply {
    fn rule_ahead_by(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) { crate::types::check::num_at_least(v.0.0, 0.0)?; }
        Ok(())
    }
    fn rule_behind_by(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) { crate::types::check::num_at_least(v.0.0, 0.0)?; }
        Ok(())
    }
}

/// TS: packages/protocol/src/tree.ts:22 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitMergeInReply {
    pub branch: String,
    pub merged: bool,
    pub commits: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub oid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub head: Option<String>,
    pub conflicts: Vec<String>,
}

impl Check for GitMergeInReply {
    /// Rules: commits: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.commits.0.0, 0.0).map_err(|e| e.under("commits"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("merged") { None => out.push(crate::types::check::missing::<Self>("merged")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("merged"))), } } }
        match obj.get("commits") { None => out.push(crate::types::check::missing::<Self>("commits")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_commits(&x) { out.push(e.under("commits")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("commits"))), } } }
        if let Some(v) = obj.get("oid") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("oid"))), } }
        if let Some(v) = obj.get("head") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("head"))), } }
        match obj.get("conflicts") { None => out.push(crate::types::check::missing::<Self>("conflicts")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("conflicts"))), } } }
        Some(out)
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
impl GitMergeInReply {
    fn rule_commits(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeRecordMerged {
    pub oid: String,
    pub at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub head: Option<String>,
}

impl Check for TreeRecordMerged {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("oid") { None => out.push(crate::types::check::missing::<Self>("oid")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("oid"))), } } }
        match obj.get("at") { None => out.push(crate::types::check::missing::<Self>("at")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
        if let Some(v) = obj.get("head") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("head"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged: Option<TreeRecordMerged>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub conflicts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub push_refused: Option<String>,
}

impl Check for TreeRecord {
    /// Nested checks: merged.
    fn check(&self) -> Result<(), SchemaError> {
        self.merged.check().map_err(|e| e.under("merged"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("merged") { match crate::types::check::read_present::<TreeRecordMerged>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("merged"))), } }
        if let Some(v) = obj.get("conflicts") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("conflicts"))), } }
        if let Some(v) = obj.get("pushRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pushRefused"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged: Option<TreeRecordMerged>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub conflicts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub push_refused: Option<String>,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pushed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ahead_of_lead: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub behind_lead: Option<JsInt>,
}

impl Check for TreeChild {
    /// Rules: aheadOfLead: >= 0; behindLead: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        self.merged.check().map_err(|e| e.under("merged"))?;
        if let Some(v) = self.ahead_of_lead {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("aheadOfLead"))?;
        }
        if let Some(v) = self.behind_lead {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("behindLead"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("merged") { match crate::types::check::read_present::<TreeRecordMerged>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("merged"))), } }
        if let Some(v) = obj.get("conflicts") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("conflicts"))), } }
        if let Some(v) = obj.get("pushRefused") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pushRefused"))), } }
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        if let Some(v) = obj.get("pushed") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pushed"))), } }
        if let Some(v) = obj.get("aheadOfLead") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_ahead_of_lead(&x) { out.push(e.under("aheadOfLead")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("aheadOfLead"))), } }
        if let Some(v) = obj.get("behindLead") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_behind_lead(&x) { out.push(e.under("behindLead")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("behindLead"))), } }
        Some(out)
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
impl TreeChild {
    fn rule_ahead_of_lead(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_behind_lead(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("leadBranch") { None => out.push(crate::types::check::missing::<Self>("leadBranch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("leadBranch"))), } } }
        match obj.get("children") { None => out.push(crate::types::check::missing::<Self>("children")), Some(v) => { match crate::types::check::read::<Vec<TreeChild>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("children"))), } } }
        match obj.get("readAt") { None => out.push(crate::types::check::missing::<Self>("readAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } } }
        Some(out)
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
    pub commits: JsInt,
    pub conflicts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub unpushed: Option<JsInt>,
}

impl Check for MergeInResult {
    /// Rules: commits: >= 0; unpushed: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.commits.0.0, 0.0).map_err(|e| e.under("commits"))?;
        if let Some(v) = self.unpushed {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("unpushed"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("lead") { None => out.push(crate::types::check::missing::<Self>("lead")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lead"))), } } }
        match obj.get("child") { None => out.push(crate::types::check::missing::<Self>("child")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("child"))), } } }
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("merged") { None => out.push(crate::types::check::missing::<Self>("merged")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("merged"))), } } }
        match obj.get("commits") { None => out.push(crate::types::check::missing::<Self>("commits")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_commits(&x) { out.push(e.under("commits")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("commits"))), } } }
        match obj.get("conflicts") { None => out.push(crate::types::check::missing::<Self>("conflicts")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("conflicts"))), } } }
        if let Some(v) = obj.get("unpushed") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_unpushed(&x) { out.push(e.under("unpushed")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("unpushed"))), } }
        Some(out)
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
impl MergeInResult {
    fn rule_commits(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
    fn rule_unpushed(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
}
