// SPDX-License-Identifier: AGPL-3.0-only
//! Port of the shapes of packages/protocol/src/slate/wire.ts at a3b870a6e that the wire names: the values a slate op
//! carries and the three slate events on the bus. The op params themselves are fields of RuntimeOp's slates.* variants.
//! The slate's parser, expressions and validator stay with wsp-slate; these shapes sit here because wsp-wire's op
//! union and event union name them and wsp-slate depends on wsp-wire.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;

/// slate/wire.ts:21: a record of any JSON by path, kept in the order sent.
pub type SlateWireValues = IndexMap<String, Value>;

/// slate/wire.ts:24 and :28, in order.
pub const SLATE_CAUSES: &[&str] = &["write", "state", "clear", "undo", "restore", "comment", "run"];
pub const SLATE_BY: &[&str] = &["agent", "person", "host", "reaction", "timer"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSlateEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    pub cause: SlateCause,
    pub version: JsInt,
    pub by: SlateBy,
    pub pieces: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub run: Option<String>,
}

impl SessionSlateEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.slate";
}
impl Check for SessionSlateEvent {
    /// Rules: pos: > 0; pieces: at most 20 items.
    /// Nested checks: cause, by.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.cause.check().map_err(|e| e.under("cause"))?;
        self.by.check().map_err(|e| e.under("by"))?;
        crate::types::check::items_at_most(self.pieces.len(), 20).map_err(|e| e.under("pieces"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "at", &mut out);
        crate::types::keys::opt_key::<String>(obj, "turnId", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "pos", &mut out, Self::rule_pos);
        crate::types::keys::req_key::<SlateCause>(obj, "cause", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "version", &mut out);
        crate::types::keys::req_key::<SlateBy>(obj, "by", &mut out);
        crate::types::keys::req_key_with::<Vec<String>>(obj, "pieces", &mut out, Self::rule_pieces);
        crate::types::keys::opt_key::<String>(obj, "run", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "sessionId" => <String as Check>::describe(rest),
                "at" => <Option<JsNumber> as Check>::describe(rest),
                "turnId" => <Option<String> as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "cause" => <SlateCause as Check>::describe(rest),
                "version" => <i64 as Check>::describe(rest),
                "by" => <SlateBy as Check>::describe(rest),
                "pieces" => <Vec<String> as Check>::describe(rest),
                "run" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionSlateEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_pieces(x: &Vec<String>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_at_most((*x).len(), 20)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlateCause {
    #[serde(rename = "write")]
    Write,
    #[serde(rename = "state")]
    State,
    #[serde(rename = "clear")]
    Clear,
    #[serde(rename = "undo")]
    Undo,
    #[serde(rename = "restore")]
    Restore,
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "run")]
    Run,
}

impl Check for SlateCause {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&["write", "state", "clear", "undo", "restore", "comment", "run"]),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlateBy {
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "person")]
    Person,
    #[serde(rename = "host")]
    Host,
    #[serde(rename = "reaction")]
    Reaction,
    #[serde(rename = "timer")]
    Timer,
}

impl Check for SlateBy {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["agent", "person", "host", "reaction", "timer"]))
    }
}

/// slate/wire.ts:296: values that moved in a batch, pushed and never recorded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlateValuesEvent {
    pub workspace_id: String,
    pub thread_id: String,
    pub version: JsInt,
    pub revision: JsInt,
    pub values: SlateWireValues,
}

impl SlateValuesEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "slate.values";
}

impl Check for SlateValuesEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "version", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "revision", &mut out);
        crate::types::keys::req_key::<SlateWireValues>(obj, "values", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "version" => <i64 as Check>::describe(rest),
                "revision" => <i64 as Check>::describe(rest),
                "values" => <SlateWireValues as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// slate/wire.ts:300: new lines of a streaming run, scrubbed, pushed and never recorded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlateRunEvent {
    pub workspace_id: String,
    pub thread_id: String,
    pub run: String,
    pub lines: Vec<String>,
}

impl SlateRunEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "slate.run";
}

impl Check for SlateRunEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "run", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "lines", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "run" => <String as Check>::describe(rest),
                "lines" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
