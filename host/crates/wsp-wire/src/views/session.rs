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
        crate::types::keys::req_key::<String>(obj, "askId", &mut out);
        crate::types::keys::req_key::<String>(obj, "toolName", &mut out);
        crate::types::keys::opt_key::<String>(obj, "toolUseId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentToolUseId", &mut out);
        crate::types::keys::req_key::<String>(obj, "input", &mut out);
        crate::types::keys::opt_key::<String>(obj, "detail", &mut out);
        crate::types::keys::req_key::<Vec<PermissionOption>>(obj, "options", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        crate::types::keys::req_key::<ThreadPrompt>(obj, "prompt", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::req_key::<String>(obj, "place", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "running", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "atOnce", &mut out);
        crate::types::keys::opt_key::<ThreadCapWaitWhy>(obj, "why", &mut out);
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
        crate::types::keys::req_key::<ThreadSection>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "whileState", &mut out);
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
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ThreadMarksPinned {
    V0(bool),
    V1(JsNumber),
}

crate::zod_union!(ThreadMarksPinned { V0(bool), V1(JsNumber) });

impl Check for ThreadMarksPinned {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            ThreadMarksPinned::V0(v) => v.check(),
            ThreadMarksPinned::V1(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![
            crate::types::check::issues_of::<bool>(value),
            crate::types::check::issues_of::<JsNumber>(value),
        ]))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Union);
        }
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
        crate::types::keys::opt_key::<ThreadMarksPinned>(obj, "pinned", &mut out);
        crate::types::keys::dbl_key::<JsNumber>(obj, "order", &mut out);
        crate::types::keys::dbl_key::<JsNumber>(obj, "snoozedUntil", &mut out);
        crate::types::keys::dbl_key::<ThreadPlacement>(obj, "section", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "resumeAtReset", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "folded", &mut out);
        let known: &[&str] = &["pinned", "order", "snoozedUntil", "section", "resumeAtReset", "folded"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("toolUses"))?;
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
        crate::types::keys::opt_key::<UsageTokens>(obj, "tokens", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "toolUses", &mut out, Self::rule_tool_uses);
        crate::types::keys::opt_key_with::<JsNumber>(obj, "durationMs", &mut out, Self::rule_duration_ms);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_duration_ms(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
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
            crate::types::check::num_above(v.0 .0, 0.0).map_err(|e| e.under("depth"))?;
        }
        self.usage.check().map_err(|e| e.under("usage"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        crate::types::keys::req_key::<SubagentState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentToolUseId", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "depth", &mut out, Self::rule_depth);
        crate::types::keys::opt_key::<String>(obj, "model", &mut out);
        crate::types::keys::opt_key::<String>(obj, "asked", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "startedAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "endedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "lastLine", &mut out);
        crate::types::keys::opt_key::<String>(obj, "failure", &mut out);
        crate::types::keys::opt_key::<SubagentUsage>(obj, "usage", &mut out);
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
        if let Some(v) = *x {
            crate::types::check::num_above(v.0 .0, 0.0)?;
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
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("at"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "turnId", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "at", &mut out, Self::rule_at);
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
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
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
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "turnId", &mut out);
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "harness", &mut out);
        crate::types::keys::req_key::<SessionStatus>(obj, "status", &mut out);
        crate::types::keys::opt_key::<SessionOrigin>(obj, "startedBy", &mut out);
        crate::types::keys::opt_key::<String>(obj, "claudeSessionId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "rootThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "attempt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "prompt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "harnessTitle", &mut out);
        crate::types::keys::opt_key::<TitleSource>(obj, "titleSource", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "startedAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "endedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
        crate::types::keys::opt_key::<TurnRefusal>(obj, "refusal", &mut out);
        crate::types::keys::opt_key::<TurnLimit>(obj, "limit", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "resumeAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "costUsd", &mut out);
        crate::types::keys::opt_key::<String>(obj, "lastLine", &mut out);
        crate::types::keys::opt_key::<String>(obj, "failure", &mut out);
        crate::types::keys::opt_key::<String>(obj, "model", &mut out);
        crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
        crate::types::keys::opt_key::<String>(obj, "permissionMode", &mut out);
        crate::types::keys::opt_key::<String>(obj, "contextWindow", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "fast", &mut out);
        crate::types::keys::opt_key::<String>(obj, "asking", &mut out);
        crate::types::keys::opt_key::<ThreadWaitingOn>(obj, "waitingOn", &mut out);
        crate::types::keys::opt_key::<String>(obj, "setupRefusal", &mut out);
        crate::types::keys::opt_key::<SessionViewProject>(obj, "project", &mut out);
        crate::types::keys::opt_key::<String>(obj, "computerName", &mut out);
        crate::types::keys::opt_key::<SubRow>(obj, "sub", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "diskKeptUntil", &mut out);
        crate::types::keys::opt_key::<String>(obj, "diskGone", &mut out);
        crate::types::keys::opt_key::<ThreadCapWait>(obj, "capped", &mut out);
        crate::types::keys::opt_key::<String>(obj, "waitsFor", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "pid", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "agentPid", &mut out);
        crate::types::keys::opt_key::<Vec<SubagentView>>(obj, "subagents", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "readAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "settledAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "pinnedAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "order", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "foldedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "replaces", &mut out);
        crate::types::keys::opt_key::<String>(obj, "replacedBy", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "snoozedUntil", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "wokeAt", &mut out);
        crate::types::keys::opt_key::<ThreadPlacement>(obj, "section", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "rewoundAt", &mut out);
        crate::types::keys::opt_key::<ForkedFrom>(obj, "forkedFrom", &mut out);
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
        crate::types::check::num_above(self.turns.0 .0, 0.0).map_err(|e| e.under("turns"))?;
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "harness", &mut out);
        crate::types::keys::req_key::<SessionOrigin>(obj, "startedBy", &mut out);
        crate::types::keys::req_key::<SessionStatus>(obj, "status", &mut out);
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "claudeSessionId", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "startedAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "endedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "turns", &mut out, Self::rule_turns);
        crate::types::keys::req_key::<bool>(obj, "ran", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "rootThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "attempt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "asking", &mut out);
        crate::types::keys::opt_key::<ThreadWaitingOn>(obj, "waitingOn", &mut out);
        crate::types::keys::opt_key::<String>(obj, "setupRefusal", &mut out);
        crate::types::keys::opt_key::<SubRow>(obj, "sub", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "diskKeptUntil", &mut out);
        crate::types::keys::opt_key::<String>(obj, "diskGone", &mut out);
        crate::types::keys::opt_key::<ThreadCapWait>(obj, "capped", &mut out);
        crate::types::keys::opt_key::<TurnLimit>(obj, "limit", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "resumeAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "costUsd", &mut out);
        crate::types::keys::opt_key::<String>(obj, "lastLine", &mut out);
        crate::types::keys::opt_key::<String>(obj, "failure", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "pid", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "agentPid", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "readAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "settledAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "pinnedAt", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "order", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "foldedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "replaces", &mut out);
        crate::types::keys::opt_key::<String>(obj, "replacedBy", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "snoozedUntil", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "wokeAt", &mut out);
        crate::types::keys::opt_key::<ThreadPlacement>(obj, "section", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "rewoundAt", &mut out);
        crate::types::keys::opt_key::<ForkedFrom>(obj, "forkedFrom", &mut out);
        crate::types::keys::opt_key::<String>(obj, "permissionMode", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "fast", &mut out);
        crate::types::keys::opt_key::<Vec<SubagentView>>(obj, "subagents", &mut out);
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
        crate::types::check::num_above(x.0 .0, 0.0)?;
        Ok(())
    }
}

// ---- Hand-written below: the functions of the unit.

// views/session.ts:368 at a3b870a6e
// export const threadKeyOf = (session: SessionView): string => session.threadId ?? session.id;
pub fn thread_key_of(session: &SessionView) -> String {
    session.thread_id.clone().unwrap_or_else(|| session.id.clone())
}

// views/session.ts:374 at a3b870a6e
// export function threadRan(turns: ReadonlyArray<Pick<SessionView, "claudeSessionId" | "status" | "refusal">>): boolean {
//   return turns.some(turn => turn.status === "running" || (turn.claudeSessionId !== undefined && turn.refusal === undefined));
// }
/// Whether any turn is running, or has a harness session id and no refusal.
pub fn thread_ran(turns: &[SessionView]) -> bool {
    turns.iter().any(|turn| turn.status == SessionStatus::Running || (turn.claude_session_id.is_some() && turn.refusal.is_none()))
}
