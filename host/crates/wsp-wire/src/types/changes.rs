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
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("stashes"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("ahead") { None => out.push(crate::types::check::missing::<Self>("ahead")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ahead"))), } } }
        match obj.get("behind") { None => out.push(crate::types::check::missing::<Self>("behind")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("behind"))), } } }
        match obj.get("changed") { None => out.push(crate::types::check::missing::<Self>("changed")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("changed"))), } } }
        if let Some(v) = obj.get("editsUnread") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("editsUnread"))), } }
        if let Some(v) = obj.get("countsUnknown") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("countsUnknown"))), } }
        if let Some(v) = obj.get("stashes") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_stashes(&x) { out.push(e.under("stashes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("stashes"))), } }
        if let Some(v) = obj.get("head") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("head"))), } }
        match obj.get("readAt") { None => out.push(crate::types::check::missing::<Self>("readAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } } }
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
            crate::types::check::num_above(v.0.0, 0.0)?;
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
        if let Some(v) = obj.get("checkout") { match crate::types::check::read_present::<Checkout>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("checkout"))), } }
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
        match obj.get("message") { None => out.push(crate::types::check::missing::<Self>("message")), Some(v) => { match crate::types::check::read::<Option<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("message"))), } } }
        if let Some(v) = obj.get("note") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("note"))), } }
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
        match obj.get("viewed") { None => out.push(crate::types::check::missing::<Self>("viewed")), Some(v) => { match crate::types::check::read::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("viewed"))), } } }
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
