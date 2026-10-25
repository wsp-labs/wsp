// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/changes.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use indexmap::IndexMap;

/// TS: packages/protocol/src/changes.ts:53 at a3b870a6e.
pub const DRAFT_DIFF_MAX_BYTES: u64 = 49_152;

/// TS: packages/protocol/src/changes.ts:120 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const DRAFT_NOTES: &[(&str, &str)] = &[
    ("noAgent", "No agent here drafts commit messages; write it yourself."),
    ("noAnswer", "The agent did not draft a message; write it yourself."),
    ("nothing", "Nothing is ticked to draft a message from."),
];

/// TS: packages/protocol/src/changes.ts:11 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkout {
    pub branch: String,
    pub ahead: JsInt,
    pub behind: JsInt,
    pub changed: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub edits_unread: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub counts_unknown: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub stashes: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub head: Option<String>,
    pub read_at: JsNumber,
}

impl Check for Checkout {
    /// Rules: stashes: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.stashes {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("stashes"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "branch", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "ahead", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "behind", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "changed", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "editsUnread", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "countsUnknown", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "stashes", &mut out, Self::rule_stashes);
        crate::types::keys::opt_key::<String>(obj, "head", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "readAt", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                "ahead" => <i64 as Check>::describe(rest),
                "behind" => <i64 as Check>::describe(rest),
                "changed" => <i64 as Check>::describe(rest),
                "editsUnread" => <Option<bool> as Check>::describe(rest),
                "countsUnknown" => <Option<bool> as Check>::describe(rest),
                "stashes" => <Option<i64> as Check>::describe(rest),
                "head" => <Option<String> as Check>::describe(rest),
                "readAt" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl Checkout {
    fn rule_stashes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/changes.ts:41 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutReply {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub checkout: Option<Checkout>,
}

impl Check for CheckoutReply {
    /// Nested checks: checkout.
    fn check(&self) -> Result<(), SchemaError> {
        self.checkout.check().map_err(|e| e.under("checkout"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<Checkout>(obj, "checkout", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "checkout" => <Option<Checkout> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/changes.ts:45 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDraft {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
}

impl Check for CommitDraft {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Option<String>>(obj, "message", &mut out);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "message" => <Option<String> as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/changes.ts:49 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewedMarks {
    #[serde(serialize_with = "crate::types::check::js_record")]
    pub viewed: IndexMap<String, String>,
}

impl Check for ViewedMarks {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<IndexMap<String, String>>(obj, "viewed", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "viewed" => <IndexMap<String, String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
