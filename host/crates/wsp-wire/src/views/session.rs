// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/session.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::types::usage::TurnLimit;
use crate::types::usage::UsageTokens;
use crate::views::sub_row::SubRow;
use crate::wire::helpers::PermissionOption;

/// TS: packages/protocol/src/views/session.ts:10 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionStatus {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "interrupted")]
    Interrupted,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for SessionStatus {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "completed", "interrupted", "failed"]))
    }
}

/// TS: packages/protocol/src/views/session.ts:15 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionOrigin {
    #[serde(rename = "person")]
    Person,
    #[serde(rename = "cli")]
    Cli,
    #[serde(rename = "agent")]
    Agent,
}

impl Check for SessionOrigin {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["person", "cli", "agent"]))
    }
}

/// TS: packages/protocol/src/views/session.ts:22 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TitleSource {
    #[serde(rename = "seed")]
    Seed,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "person")]
    Person,
}

impl Check for TitleSource {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["seed", "auto", "person"]))
    }
}

/// TS: packages/protocol/src/views/session.ts:28 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TurnRefusal {
    #[serde(rename = "sign-in")]
    SignIn,
}

impl Check for TurnRefusal {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["sign-in"]))
    }
}

/// TS: packages/protocol/src/views/session.ts:34 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionOutcome {
    #[serde(rename = "allowed")]
    Allowed,
    #[serde(rename = "denied")]
    Denied,
    #[serde(rename = "unanswered")]
    Unanswered,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl Check for PermissionOutcome {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["allowed", "denied", "unanswered", "cancelled"]))
    }
}

/// TS: packages/protocol/src/views/session.ts:37 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPrompt {
    pub ask_id: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_tool_use_id: Option<String>,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub detail: Option<String>,
    pub options: Vec<PermissionOption>,
}

impl Check for ThreadPrompt {
    /// Nested checks: options.
    fn check(&self) -> Result<(), SchemaError> {
        self.options.check().map_err(|e| e.under("options"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("askId") { None => out.push(crate::types::check::missing::<Self>("askId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("askId"))), } } }
        match obj.get("toolName") { None => out.push(crate::types::check::missing::<Self>("toolName")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("toolName"))), } } }
        if let Some(v) = obj.get("toolUseId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("toolUseId"))), } }
        if let Some(v) = obj.get("parentToolUseId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentToolUseId"))), } }
        match obj.get("input") { None => out.push(crate::types::check::missing::<Self>("input")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("input"))), } } }
        if let Some(v) = obj.get("detail") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("detail"))), } }
        match obj.get("options") { None => out.push(crate::types::check::missing::<Self>("options")), Some(v) => { match crate::types::check::read::<Vec<PermissionOption>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("options"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "askId" => <String as Check>::describe(rest),
                "toolName" => <String as Check>::describe(rest),
                "toolUseId" => <Option<String> as Check>::describe(rest),
                "parentToolUseId" => <Option<String> as Check>::describe(rest),
                "input" => <String as Check>::describe(rest),
                "detail" => <Option<String> as Check>::describe(rest),
                "options" => <Vec<PermissionOption> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session.ts:44 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadWaitingOn {
    pub thread_id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub title: String,
    pub prompt: ThreadPrompt,
}

impl Check for ThreadWaitingOn {
    /// Nested checks: prompt.
    fn check(&self) -> Result<(), SchemaError> {
        self.prompt.check().map_err(|e| e.under("prompt"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("sessionId") { None => out.push(crate::types::check::missing::<Self>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
        match obj.get("title") { None => out.push(crate::types::check::missing::<Self>("title")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("title"))), } } }
        match obj.get("prompt") { None => out.push(crate::types::check::missing::<Self>("prompt")), Some(v) => { match crate::types::check::read::<ThreadPrompt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("prompt"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threadId" => <String as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "sessionId" => <String as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "prompt" => <ThreadPrompt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreadCapWaitWhy {
    #[serde(rename = "slot")]
    Slot,
    #[serde(rename = "minute")]
    Minute,
}

impl Check for ThreadCapWaitWhy {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["slot", "minute"]))
    }
}

/// TS: packages/protocol/src/views/session.ts:58 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCapWait {
    pub place_id: String,
    pub place: String,
    pub running: JsInt,
    pub at_once: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub why: Option<ThreadCapWaitWhy>,
}

impl Check for ThreadCapWait {
    fn check(&self) -> Result<(), SchemaError> {
        self.why.check().map_err(|e| e.under("why"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("placeId") { None => out.push(crate::types::check::missing::<Self>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
        match obj.get("place") { None => out.push(crate::types::check::missing::<Self>("place")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } } }
        match obj.get("running") { None => out.push(crate::types::check::missing::<Self>("running")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("running"))), } } }
        match obj.get("atOnce") { None => out.push(crate::types::check::missing::<Self>("atOnce")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("atOnce"))), } } }
        if let Some(v) = obj.get("why") { match crate::types::check::read_present::<ThreadCapWaitWhy>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("why"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "place" => <String as Check>::describe(rest),
                "running" => <i64 as Check>::describe(rest),
                "atOnce" => <i64 as Check>::describe(rest),
                "why" => <Option<ThreadCapWaitWhy> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session.ts:71 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreadSection {
    #[serde(rename = "needs-you")]
    NeedsYou,
    #[serde(rename = "working")]
    Working,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "idle")]
    Idle,
}

impl Check for ThreadSection {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["needs-you", "working", "done", "idle"]))
    }
}

/// TS: packages/protocol/src/views/session.ts:77 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPlacement {
    pub name: ThreadSection,
    pub while_state: String,
}

impl Check for ThreadPlacement {
    /// Nested checks: name.
    fn check(&self) -> Result<(), SchemaError> {
        self.name.check().map_err(|e| e.under("name"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<ThreadSection>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("whileState") { None => out.push(crate::types::check::missing::<Self>("whileState")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("whileState"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <ThreadSection as Check>::describe(rest),
                "whileState" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThreadMarksPinned {
    V0(bool),
    V1(JsNumber),
}

impl Check for ThreadMarksPinned {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            ThreadMarksPinned::V0(v) => v.check(),
            ThreadMarksPinned::V1(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![crate::types::check::issues_of::<bool>(value), crate::types::check::issues_of::<JsNumber>(value)]))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() { return Some(crate::types::check::Zod::Union); }
        <bool as Check>::describe(path).or_else(|| <JsNumber as Check>::describe(path))
    }
}

/// TS: packages/protocol/src/views/session.ts:87 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThreadMarks {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pinned: Option<ThreadMarksPinned>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub order: Option<Option<JsNumber>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub snoozed_until: Option<Option<JsNumber>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub section: Option<Option<ThreadPlacement>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resume_at_reset: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folded: Option<bool>,
}

impl Check for ThreadMarks {
    /// Rules: order: zod number check finite .
    /// Nested checks: pinned, section.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        self.pinned.check().map_err(|e| e.under("pinned"))?;
        self.section.check().map_err(|e| e.under("section"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("pinned") { match crate::types::check::read_present::<ThreadMarksPinned>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pinned"))), } }
        if let Some(v) = obj.get("order") { match crate::types::check::read_double::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("order"))), } }
        if let Some(v) = obj.get("snoozedUntil") { match crate::types::check::read_double::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snoozedUntil"))), } }
        if let Some(v) = obj.get("section") { match crate::types::check::read_double::<ThreadPlacement>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("section"))), } }
        if let Some(v) = obj.get("resumeAtReset") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("resumeAtReset"))), } }
        if let Some(v) = obj.get("folded") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folded"))), } }
        let known: &[&str] = &["pinned", "order", "snoozedUntil", "section", "resumeAtReset", "folded"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() { out.push(crate::types::check::unrecognized(Vec::new(), extra)); }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pinned" => <Option<ThreadMarksPinned> as Check>::describe(rest),
                "order" => <Option<Option<JsNumber>> as Check>::describe(rest),
                "snoozedUntil" => <Option<Option<JsNumber>> as Check>::describe(rest),
                "section" => <Option<Option<ThreadPlacement>> as Check>::describe(rest),
                "resumeAtReset" => <Option<bool> as Check>::describe(rest),
                "folded" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session.ts:102 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubagentState {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "stopped")]
    Stopped,
}

impl Check for SubagentState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["running", "done", "failed", "stopped"]))
    }
}

/// TS: packages/protocol/src/views/session.ts:110 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentUsage {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tokens: Option<UsageTokens>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tool_uses: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub duration_ms: Option<JsNumber>,
}

impl Check for SubagentUsage {
    /// Rules: toolUses: >= 0; durationMs: >= 0.
    /// Nested checks: tokens.
    fn check(&self) -> Result<(), SchemaError> {
        self.tokens.check().map_err(|e| e.under("tokens"))?;
        if let Some(v) = self.tool_uses {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("toolUses"))?;
        }
        if let Some(v) = self.duration_ms {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("durationMs"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("tokens") { match crate::types::check::read_present::<UsageTokens>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tokens"))), } }
        if let Some(v) = obj.get("toolUses") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_tool_uses(&x) { out.push(e.under("toolUses")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("toolUses"))), } }
        if let Some(v) = obj.get("durationMs") { match crate::types::check::read_present::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_duration_ms(&x) { out.push(e.under("durationMs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("durationMs"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "tokens" => <Option<UsageTokens> as Check>::describe(rest),
                "toolUses" => <Option<i64> as Check>::describe(rest),
                "durationMs" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubagentUsage {
    fn rule_tool_uses(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_duration_ms(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session.ts:116 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentView {
    pub id: String,
    pub title: String,
    pub state: SubagentState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub depth: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub asked: Option<String>,
    pub started_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ended_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub last_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub usage: Option<SubagentUsage>,
}

impl Check for SubagentView {
    /// Rules: depth: > 0.
    /// Nested checks: state, usage.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        if let Some(v) = self.depth {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("depth"))?;
        }
        self.usage.check().map_err(|e| e.under("usage"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("title") { None => out.push(crate::types::check::missing::<Self>("title")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("title"))), } } }
        match obj.get("state") { None => out.push(crate::types::check::missing::<Self>("state")), Some(v) => { match crate::types::check::read::<SubagentState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
        if let Some(v) = obj.get("parentToolUseId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentToolUseId"))), } }
        if let Some(v) = obj.get("depth") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_depth(&x) { out.push(e.under("depth")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("depth"))), } }
        if let Some(v) = obj.get("model") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } }
        if let Some(v) = obj.get("asked") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("asked"))), } }
        match obj.get("startedAt") { None => out.push(crate::types::check::missing::<Self>("startedAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedAt"))), } } }
        if let Some(v) = obj.get("endedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("endedAt"))), } }
        if let Some(v) = obj.get("lastLine") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lastLine"))), } }
        if let Some(v) = obj.get("failure") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("failure"))), } }
        if let Some(v) = obj.get("usage") { match crate::types::check::read_present::<SubagentUsage>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("usage"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "state" => <SubagentState as Check>::describe(rest),
                "parentToolUseId" => <Option<String> as Check>::describe(rest),
                "depth" => <Option<i64> as Check>::describe(rest),
                "model" => <Option<String> as Check>::describe(rest),
                "asked" => <Option<String> as Check>::describe(rest),
                "startedAt" => <JsNumber as Check>::describe(rest),
                "endedAt" => <Option<JsNumber> as Check>::describe(rest),
                "lastLine" => <Option<String> as Check>::describe(rest),
                "failure" => <Option<String> as Check>::describe(rest),
                "usage" => <Option<SubagentUsage> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SubagentView {
    fn rule_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session.ts:138 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkSource {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsInt>,
}

impl Check for ForkSource {
    /// Rules: at: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.at {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("at"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
        if let Some(v) = obj.get("at") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_at(&x) { out.push(e.under("at")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threadId" => <String as Check>::describe(rest),
                "turnId" => <Option<String> as Check>::describe(rest),
                "at" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ForkSource {
    fn rule_at(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_at_least(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session.ts:143 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkedFrom {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    pub title: String,
}

impl Check for ForkedFrom {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
        match obj.get("title") { None => out.push(crate::types::check::missing::<Self>("title")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("title"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threadId" => <String as Check>::describe(rest),
                "turnId" => <Option<String> as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionViewProject {
    pub id: String,
    pub name: String,
}

impl Check for SessionViewProject {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session.ts:146 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub workspace_id: String,
    pub harness: String,
    pub status: SessionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_by: Option<SessionOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub attempt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub harness_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub title_source: Option<TitleSource>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ended_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub refusal: Option<TurnRefusal>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub limit: Option<TurnLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resume_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub last_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub context_window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fast: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub asking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub waiting_on: Option<ThreadWaitingOn>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub setup_refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub project: Option<SessionViewProject>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub computer_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sub: Option<SubRow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_kept_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub capped: Option<ThreadCapWait>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub waits_for: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pid: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent_pid: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub subagents: Option<Vec<SubagentView>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub read_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub settled_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pinned_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub order: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folded_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub replaces: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub replaced_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub snoozed_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub woke_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub section: Option<ThreadPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rewound_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub forked_from: Option<ForkedFrom>,
}

impl Check for SessionView {
    /// Nested checks: status, startedBy, titleSource, refusal, limit, waitingOn, project, sub, capped, subagents, section, forkedFrom.
    fn check(&self) -> Result<(), SchemaError> {
        self.status.check().map_err(|e| e.under("status"))?;
        self.started_by.check().map_err(|e| e.under("startedBy"))?;
        self.title_source.check().map_err(|e| e.under("titleSource"))?;
        self.refusal.check().map_err(|e| e.under("refusal"))?;
        self.limit.check().map_err(|e| e.under("limit"))?;
        self.waiting_on.check().map_err(|e| e.under("waitingOn"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.sub.check().map_err(|e| e.under("sub"))?;
        self.capped.check().map_err(|e| e.under("capped"))?;
        self.subagents.check().map_err(|e| e.under("subagents"))?;
        self.section.check().map_err(|e| e.under("section"))?;
        self.forked_from.check().map_err(|e| e.under("forkedFrom"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("harness") { None => out.push(crate::types::check::missing::<Self>("harness")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harness"))), } } }
        match obj.get("status") { None => out.push(crate::types::check::missing::<Self>("status")), Some(v) => { match crate::types::check::read::<SessionStatus>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } } }
        if let Some(v) = obj.get("startedBy") { match crate::types::check::read_present::<SessionOrigin>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedBy"))), } }
        if let Some(v) = obj.get("claudeSessionId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("claudeSessionId"))), } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        if let Some(v) = obj.get("parentThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentThreadId"))), } }
        if let Some(v) = obj.get("rootThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rootThreadId"))), } }
        if let Some(v) = obj.get("attempt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("attempt"))), } }
        if let Some(v) = obj.get("prompt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("prompt"))), } }
        if let Some(v) = obj.get("harnessTitle") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harnessTitle"))), } }
        if let Some(v) = obj.get("titleSource") { match crate::types::check::read_present::<TitleSource>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("titleSource"))), } }
        if let Some(v) = obj.get("startedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedAt"))), } }
        if let Some(v) = obj.get("endedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("endedAt"))), } }
        if let Some(v) = obj.get("cwd") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cwd"))), } }
        if let Some(v) = obj.get("refusal") { match crate::types::check::read_present::<TurnRefusal>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("refusal"))), } }
        if let Some(v) = obj.get("limit") { match crate::types::check::read_present::<TurnLimit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("limit"))), } }
        if let Some(v) = obj.get("resumeAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("resumeAt"))), } }
        if let Some(v) = obj.get("costUsd") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("costUsd"))), } }
        if let Some(v) = obj.get("lastLine") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lastLine"))), } }
        if let Some(v) = obj.get("failure") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("failure"))), } }
        if let Some(v) = obj.get("model") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } }
        if let Some(v) = obj.get("effort") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("effort"))), } }
        if let Some(v) = obj.get("permissionMode") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("permissionMode"))), } }
        if let Some(v) = obj.get("contextWindow") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("contextWindow"))), } }
        if let Some(v) = obj.get("fast") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fast"))), } }
        if let Some(v) = obj.get("asking") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("asking"))), } }
        if let Some(v) = obj.get("waitingOn") { match crate::types::check::read_present::<ThreadWaitingOn>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("waitingOn"))), } }
        if let Some(v) = obj.get("setupRefusal") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("setupRefusal"))), } }
        if let Some(v) = obj.get("project") { match crate::types::check::read_present::<SessionViewProject>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
        if let Some(v) = obj.get("computerName") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computerName"))), } }
        if let Some(v) = obj.get("sub") { match crate::types::check::read_present::<SubRow>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sub"))), } }
        if let Some(v) = obj.get("diskKeptUntil") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskKeptUntil"))), } }
        if let Some(v) = obj.get("diskGone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskGone"))), } }
        if let Some(v) = obj.get("capped") { match crate::types::check::read_present::<ThreadCapWait>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("capped"))), } }
        if let Some(v) = obj.get("waitsFor") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("waitsFor"))), } }
        if let Some(v) = obj.get("pid") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } }
        if let Some(v) = obj.get("agentPid") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agentPid"))), } }
        if let Some(v) = obj.get("subagents") { match crate::types::check::read_present::<Vec<SubagentView>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("subagents"))), } }
        if let Some(v) = obj.get("readAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } }
        if let Some(v) = obj.get("settledAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("settledAt"))), } }
        if let Some(v) = obj.get("pinnedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pinnedAt"))), } }
        if let Some(v) = obj.get("order") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("order"))), } }
        if let Some(v) = obj.get("foldedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("foldedAt"))), } }
        if let Some(v) = obj.get("replaces") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replaces"))), } }
        if let Some(v) = obj.get("replacedBy") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replacedBy"))), } }
        if let Some(v) = obj.get("snoozedUntil") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snoozedUntil"))), } }
        if let Some(v) = obj.get("wokeAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wokeAt"))), } }
        if let Some(v) = obj.get("section") { match crate::types::check::read_present::<ThreadPlacement>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("section"))), } }
        if let Some(v) = obj.get("rewoundAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rewoundAt"))), } }
        if let Some(v) = obj.get("forkedFrom") { match crate::types::check::read_present::<ForkedFrom>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("forkedFrom"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "harness" => <String as Check>::describe(rest),
                "status" => <SessionStatus as Check>::describe(rest),
                "startedBy" => <Option<SessionOrigin> as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "attempt" => <Option<String> as Check>::describe(rest),
                "prompt" => <Option<String> as Check>::describe(rest),
                "harnessTitle" => <Option<String> as Check>::describe(rest),
                "titleSource" => <Option<TitleSource> as Check>::describe(rest),
                "startedAt" => <Option<JsNumber> as Check>::describe(rest),
                "endedAt" => <Option<JsNumber> as Check>::describe(rest),
                "cwd" => <Option<String> as Check>::describe(rest),
                "refusal" => <Option<TurnRefusal> as Check>::describe(rest),
                "limit" => <Option<TurnLimit> as Check>::describe(rest),
                "resumeAt" => <Option<JsNumber> as Check>::describe(rest),
                "costUsd" => <Option<JsNumber> as Check>::describe(rest),
                "lastLine" => <Option<String> as Check>::describe(rest),
                "failure" => <Option<String> as Check>::describe(rest),
                "model" => <Option<String> as Check>::describe(rest),
                "effort" => <Option<String> as Check>::describe(rest),
                "permissionMode" => <Option<String> as Check>::describe(rest),
                "contextWindow" => <Option<String> as Check>::describe(rest),
                "fast" => <Option<bool> as Check>::describe(rest),
                "asking" => <Option<String> as Check>::describe(rest),
                "waitingOn" => <Option<ThreadWaitingOn> as Check>::describe(rest),
                "setupRefusal" => <Option<String> as Check>::describe(rest),
                "project" => <Option<SessionViewProject> as Check>::describe(rest),
                "computerName" => <Option<String> as Check>::describe(rest),
                "sub" => <Option<SubRow> as Check>::describe(rest),
                "diskKeptUntil" => <Option<JsNumber> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "capped" => <Option<ThreadCapWait> as Check>::describe(rest),
                "waitsFor" => <Option<String> as Check>::describe(rest),
                "pid" => <Option<i64> as Check>::describe(rest),
                "agentPid" => <Option<i64> as Check>::describe(rest),
                "subagents" => <Option<Vec<SubagentView>> as Check>::describe(rest),
                "readAt" => <Option<JsNumber> as Check>::describe(rest),
                "settledAt" => <Option<JsNumber> as Check>::describe(rest),
                "pinnedAt" => <Option<JsNumber> as Check>::describe(rest),
                "order" => <Option<JsNumber> as Check>::describe(rest),
                "foldedAt" => <Option<JsNumber> as Check>::describe(rest),
                "replaces" => <Option<String> as Check>::describe(rest),
                "replacedBy" => <Option<String> as Check>::describe(rest),
                "snoozedUntil" => <Option<JsNumber> as Check>::describe(rest),
                "wokeAt" => <Option<JsNumber> as Check>::describe(rest),
                "section" => <Option<ThreadPlacement> as Check>::describe(rest),
                "rewoundAt" => <Option<JsNumber> as Check>::describe(rest),
                "forkedFrom" => <Option<ForkedFrom> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session.ts:294 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadView {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    pub workspace_id: String,
    pub harness: String,
    pub started_by: SessionOrigin,
    pub status: SessionStatus,
    pub title: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ended_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cwd: Option<String>,
    pub turns: JsInt,
    pub ran: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub attempt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub asking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub waiting_on: Option<ThreadWaitingOn>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub setup_refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sub: Option<SubRow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_kept_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub capped: Option<ThreadCapWait>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub limit: Option<TurnLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resume_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub last_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pid: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent_pid: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub read_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub settled_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pinned_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub order: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folded_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub replaces: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub replaced_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub snoozed_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub woke_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub section: Option<ThreadPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rewound_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub forked_from: Option<ForkedFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fast: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub subagents: Option<Vec<SubagentView>>,
}

impl Check for ThreadView {
    /// Rules: turns: > 0.
    /// Nested checks: startedBy, status, waitingOn, sub, capped, limit, section, forkedFrom, subagents.
    fn check(&self) -> Result<(), SchemaError> {
        self.started_by.check().map_err(|e| e.under("startedBy"))?;
        self.status.check().map_err(|e| e.under("status"))?;
        crate::types::check::num_above(self.turns.0.0, 0.0).map_err(|e| e.under("turns"))?;
        self.waiting_on.check().map_err(|e| e.under("waitingOn"))?;
        self.sub.check().map_err(|e| e.under("sub"))?;
        self.capped.check().map_err(|e| e.under("capped"))?;
        self.limit.check().map_err(|e| e.under("limit"))?;
        self.section.check().map_err(|e| e.under("section"))?;
        self.forked_from.check().map_err(|e| e.under("forkedFrom"))?;
        self.subagents.check().map_err(|e| e.under("subagents"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("harness") { None => out.push(crate::types::check::missing::<Self>("harness")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harness"))), } } }
        match obj.get("startedBy") { None => out.push(crate::types::check::missing::<Self>("startedBy")), Some(v) => { match crate::types::check::read::<SessionOrigin>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedBy"))), } } }
        match obj.get("status") { None => out.push(crate::types::check::missing::<Self>("status")), Some(v) => { match crate::types::check::read::<SessionStatus>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } } }
        match obj.get("title") { None => out.push(crate::types::check::missing::<Self>("title")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("title"))), } } }
        match obj.get("sessionId") { None => out.push(crate::types::check::missing::<Self>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
        if let Some(v) = obj.get("claudeSessionId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("claudeSessionId"))), } }
        if let Some(v) = obj.get("startedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedAt"))), } }
        if let Some(v) = obj.get("endedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("endedAt"))), } }
        if let Some(v) = obj.get("cwd") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cwd"))), } }
        match obj.get("turns") { None => out.push(crate::types::check::missing::<Self>("turns")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_turns(&x) { out.push(e.under("turns")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("turns"))), } } }
        match obj.get("ran") { None => out.push(crate::types::check::missing::<Self>("ran")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ran"))), } } }
        if let Some(v) = obj.get("parentThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentThreadId"))), } }
        if let Some(v) = obj.get("rootThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rootThreadId"))), } }
        if let Some(v) = obj.get("attempt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("attempt"))), } }
        if let Some(v) = obj.get("asking") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("asking"))), } }
        if let Some(v) = obj.get("waitingOn") { match crate::types::check::read_present::<ThreadWaitingOn>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("waitingOn"))), } }
        if let Some(v) = obj.get("setupRefusal") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("setupRefusal"))), } }
        if let Some(v) = obj.get("sub") { match crate::types::check::read_present::<SubRow>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sub"))), } }
        if let Some(v) = obj.get("diskKeptUntil") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskKeptUntil"))), } }
        if let Some(v) = obj.get("diskGone") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskGone"))), } }
        if let Some(v) = obj.get("capped") { match crate::types::check::read_present::<ThreadCapWait>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("capped"))), } }
        if let Some(v) = obj.get("limit") { match crate::types::check::read_present::<TurnLimit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("limit"))), } }
        if let Some(v) = obj.get("resumeAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("resumeAt"))), } }
        if let Some(v) = obj.get("costUsd") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("costUsd"))), } }
        if let Some(v) = obj.get("lastLine") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lastLine"))), } }
        if let Some(v) = obj.get("failure") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("failure"))), } }
        if let Some(v) = obj.get("pid") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } }
        if let Some(v) = obj.get("agentPid") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agentPid"))), } }
        if let Some(v) = obj.get("readAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } }
        if let Some(v) = obj.get("settledAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("settledAt"))), } }
        if let Some(v) = obj.get("pinnedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pinnedAt"))), } }
        if let Some(v) = obj.get("order") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("order"))), } }
        if let Some(v) = obj.get("foldedAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("foldedAt"))), } }
        if let Some(v) = obj.get("replaces") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replaces"))), } }
        if let Some(v) = obj.get("replacedBy") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replacedBy"))), } }
        if let Some(v) = obj.get("snoozedUntil") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snoozedUntil"))), } }
        if let Some(v) = obj.get("wokeAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wokeAt"))), } }
        if let Some(v) = obj.get("section") { match crate::types::check::read_present::<ThreadPlacement>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("section"))), } }
        if let Some(v) = obj.get("rewoundAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rewoundAt"))), } }
        if let Some(v) = obj.get("forkedFrom") { match crate::types::check::read_present::<ForkedFrom>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("forkedFrom"))), } }
        if let Some(v) = obj.get("permissionMode") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("permissionMode"))), } }
        if let Some(v) = obj.get("fast") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fast"))), } }
        if let Some(v) = obj.get("subagents") { match crate::types::check::read_present::<Vec<SubagentView>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("subagents"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "harness" => <String as Check>::describe(rest),
                "startedBy" => <SessionOrigin as Check>::describe(rest),
                "status" => <SessionStatus as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "sessionId" => <String as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "startedAt" => <Option<JsNumber> as Check>::describe(rest),
                "endedAt" => <Option<JsNumber> as Check>::describe(rest),
                "cwd" => <Option<String> as Check>::describe(rest),
                "turns" => <i64 as Check>::describe(rest),
                "ran" => <bool as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "attempt" => <Option<String> as Check>::describe(rest),
                "asking" => <Option<String> as Check>::describe(rest),
                "waitingOn" => <Option<ThreadWaitingOn> as Check>::describe(rest),
                "setupRefusal" => <Option<String> as Check>::describe(rest),
                "sub" => <Option<SubRow> as Check>::describe(rest),
                "diskKeptUntil" => <Option<JsNumber> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "capped" => <Option<ThreadCapWait> as Check>::describe(rest),
                "limit" => <Option<TurnLimit> as Check>::describe(rest),
                "resumeAt" => <Option<JsNumber> as Check>::describe(rest),
                "costUsd" => <Option<JsNumber> as Check>::describe(rest),
                "lastLine" => <Option<String> as Check>::describe(rest),
                "failure" => <Option<String> as Check>::describe(rest),
                "pid" => <Option<i64> as Check>::describe(rest),
                "agentPid" => <Option<i64> as Check>::describe(rest),
                "readAt" => <Option<JsNumber> as Check>::describe(rest),
                "settledAt" => <Option<JsNumber> as Check>::describe(rest),
                "pinnedAt" => <Option<JsNumber> as Check>::describe(rest),
                "order" => <Option<JsNumber> as Check>::describe(rest),
                "foldedAt" => <Option<JsNumber> as Check>::describe(rest),
                "replaces" => <Option<String> as Check>::describe(rest),
                "replacedBy" => <Option<String> as Check>::describe(rest),
                "snoozedUntil" => <Option<JsNumber> as Check>::describe(rest),
                "wokeAt" => <Option<JsNumber> as Check>::describe(rest),
                "section" => <Option<ThreadPlacement> as Check>::describe(rest),
                "rewoundAt" => <Option<JsNumber> as Check>::describe(rest),
                "forkedFrom" => <Option<ForkedFrom> as Check>::describe(rest),
                "permissionMode" => <Option<String> as Check>::describe(rest),
                "fast" => <Option<bool> as Check>::describe(rest),
                "subagents" => <Option<Vec<SubagentView>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ThreadView {
    fn rule_turns(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_above((*x).0.0, 0.0)?;
        Ok(())
    }
}

// ---- Hand-written below: the functions of the unit.

// views/session.ts:368 at a3b870a6e
// export const threadKeyOf = (session: SessionView): string => session.threadId ?? session.id;
pub fn thread_key_of(session: &SessionView) -> String {
    todo!()
}

// views/session.ts:374 at a3b870a6e
// export function threadRan(turns: ReadonlyArray<Pick<SessionView, "claudeSessionId" | "status" | "refusal">>): boolean {
//   return turns.some(turn => turn.status === "running" || (turn.claudeSessionId !== undefined && turn.refusal === undefined));
// }
/// Whether any turn is running, or has a harness session id and no refusal.
pub fn thread_ran(turns: &[SessionView]) -> bool {
    todo!()
}
