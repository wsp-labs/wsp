// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/session-events.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::literal;
use crate::types::attachments::AttachmentRecord;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::types::slate_wire::SessionSlateEvent;
use crate::types::usage::TurnLimit;
use crate::types::usage::UsageTokens;
use crate::views::session::PermissionOutcome;
use crate::views::session::SessionOrigin;
use crate::views::session::SessionView;
use crate::views::session::SubagentState;
use crate::views::session::SubagentUsage;
use crate::views::session::TurnRefusal;
use crate::wire::helpers::PermissionOption;
use wsp_frames::True;

literal!(SlateLit, "slate");

/// TS: packages/protocol/src/views/session-events.ts:312 at a3b870a6e.
pub const AGENT_STARTING_MS: u64 = 4_000;

/// TS: packages/protocol/src/views/session-events.ts:315 at a3b870a6e.
pub const FIRST_RUN_WORD: &str = "installs on first run";

/// TS: packages/protocol/src/views/session-events.ts:318 at a3b870a6e.
pub const VERSION_UNREAD_WORD: &str = "version unreadable";

/// TS: packages/protocol/src/views/session-events.ts:325 at a3b870a6e.
pub const NOTIFY_ME: &str = "me";

/// TS: packages/protocol/src/views/session-events.ts:346 at a3b870a6e.
pub const GUEST_DAEMON_DIR: &str = "/root/wsp-daemon";

/// TS: packages/protocol/src/views/session-events.ts:349 at a3b870a6e.
pub const DAEMON_UNIT: &str = "wsp-daemon.service";

/// TS: packages/protocol/src/views/session-events.ts:357 at a3b870a6e.
pub const GUEST_WSP_BIN: &str = "/root/wsp-daemon/wsp/dist/bin.js";

/// TS: packages/protocol/src/views/session-events.ts:362 at a3b870a6e.
pub const GUEST_WSP_PATH: &str = "/usr/local/bin/wsp";

/// TS: packages/protocol/src/views/session-events.ts:632 at a3b870a6e.
pub const RUN_PERSONS_LINE: &str = "a reply's block runs on the person's click, never on a thread's token";

/// TS: packages/protocol/src/views/session-events.ts:669 at a3b870a6e.
/// A Set in the TS, in insertion order.
pub const SESSION_EVENT_TYPES: &[&str] = &[
    "session.start",
    "session.delta",
    "session.done",
    "session.end",
    "session.steer",
    "session.notify",
    "session.permission",
    "session.permission.closed",
    "session.checkpoint",
    "session.changes",
    "session.plan",
    "session.compacted",
    "session.retry",
    "session.context",
    "session.run",
    "session.moved",
    "session.behind",
    "session.notice",
    "session.earlier",
    "session.capped",
    "session.uncapped",
    "session.subagent",
    "session.slate",
    "thread.waited",
];

/// TS: packages/protocol/src/views/session-events.ts:16 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeltaKind {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(rename = "note")]
    Note,
    #[serde(rename = "tool_use")]
    ToolUse,
    #[serde(rename = "tool_result")]
    ToolResult,
}

impl Check for DeltaKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["text", "thinking", "note", "tool_use", "tool_result"]))
    }
}

/// TS: packages/protocol/src/views/session-events.ts:19 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TurnStatus {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "interrupted")]
    Interrupted,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for TurnStatus {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["completed", "interrupted", "failed"]))
    }
}

/// TS: packages/protocol/src/views/session-events.ts:25 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnTokens {
    pub input: JsNumber,
    pub output: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cached: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cache_write: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reasoning: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub context: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub window: Option<JsNumber>,
}

impl Check for TurnTokens {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("input") { None => out.push(crate::types::check::missing::<Self>("input")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("input"))), } } }
        match obj.get("output") { None => out.push(crate::types::check::missing::<Self>("output")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("output"))), } } }
        if let Some(v) = obj.get("cached") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cached"))), } }
        if let Some(v) = obj.get("cacheWrite") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cacheWrite"))), } }
        if let Some(v) = obj.get("reasoning") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reasoning"))), } }
        if let Some(v) = obj.get("context") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("context"))), } }
        if let Some(v) = obj.get("window") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("window"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "input" => <JsNumber as Check>::describe(rest),
                "output" => <JsNumber as Check>::describe(rest),
                "cached" => <Option<JsNumber> as Check>::describe(rest),
                "cacheWrite" => <Option<JsNumber> as Check>::describe(rest),
                "reasoning" => <Option<JsNumber> as Check>::describe(rest),
                "context" => <Option<JsNumber> as Check>::describe(rest),
                "window" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnResultModelsItem {
    pub model: String,
    pub tokens: UsageTokens,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost_usd: Option<JsNumber>,
}

impl Check for TurnResultModelsItem {
    /// Nested checks: tokens.
    fn check(&self) -> Result<(), SchemaError> {
        self.tokens.check().map_err(|e| e.under("tokens"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("model") { None => out.push(crate::types::check::missing::<Self>("model")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } } }
        match obj.get("tokens") { None => out.push(crate::types::check::missing::<Self>("tokens")), Some(v) => { match crate::types::check::read::<UsageTokens>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tokens"))), } } }
        if let Some(v) = obj.get("costUsd") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("costUsd"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "model" => <String as Check>::describe(rest),
                "tokens" => <UsageTokens as Check>::describe(rest),
                "costUsd" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:36 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnResult {
    pub status: TurnStatus,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub duration_ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub waited_ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tokens: Option<TurnTokens>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub models: Option<Vec<TurnResultModelsItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub unreached: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub refusal: Option<TurnRefusal>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub limit: Option<TurnLimit>,
}

impl Check for TurnResult {
    /// Nested checks: status, tokens, models, refusal, limit.
    fn check(&self) -> Result<(), SchemaError> {
        self.status.check().map_err(|e| e.under("status"))?;
        self.tokens.check().map_err(|e| e.under("tokens"))?;
        self.models.check().map_err(|e| e.under("models"))?;
        self.refusal.check().map_err(|e| e.under("refusal"))?;
        self.limit.check().map_err(|e| e.under("limit"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("status") { None => out.push(crate::types::check::missing::<Self>("status")), Some(v) => { match crate::types::check::read::<TurnStatus>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } } }
        if let Some(v) = obj.get("durationMs") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("durationMs"))), } }
        if let Some(v) = obj.get("waitedMs") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("waitedMs"))), } }
        if let Some(v) = obj.get("costUsd") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("costUsd"))), } }
        if let Some(v) = obj.get("tokens") { match crate::types::check::read_present::<TurnTokens>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tokens"))), } }
        if let Some(v) = obj.get("model") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } }
        if let Some(v) = obj.get("models") { match crate::types::check::read_present::<Vec<TurnResultModelsItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("models"))), } }
        if let Some(v) = obj.get("text") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } }
        if let Some(v) = obj.get("error") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("error"))), } }
        if let Some(v) = obj.get("unreached") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("unreached"))), } }
        if let Some(v) = obj.get("refusal") { match crate::types::check::read_present::<TurnRefusal>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("refusal"))), } }
        if let Some(v) = obj.get("limit") { match crate::types::check::read_present::<TurnLimit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("limit"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "status" => <TurnStatus as Check>::describe(rest),
                "durationMs" => <Option<JsNumber> as Check>::describe(rest),
                "waitedMs" => <Option<JsNumber> as Check>::describe(rest),
                "costUsd" => <Option<JsNumber> as Check>::describe(rest),
                "tokens" => <Option<TurnTokens> as Check>::describe(rest),
                "model" => <Option<String> as Check>::describe(rest),
                "models" => <Option<Vec<TurnResultModelsItem>> as Check>::describe(rest),
                "text" => <Option<String> as Check>::describe(rest),
                "error" => <Option<String> as Check>::describe(rest),
                "unreached" => None,
                "refusal" => <Option<TurnRefusal> as Check>::describe(rest),
                "limit" => <Option<TurnLimit> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:86 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHarness {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub slash_commands: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<Vec<String>>,
}

impl Check for SessionHarness {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("slashCommands") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("slashCommands"))), } }
        if let Some(v) = obj.get("permissionMode") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("permissionMode"))), } }
        if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "slashCommands" => <Option<Vec<String>> as Check>::describe(rest),
                "permissionMode" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<Vec<String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:93 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub attachments: Option<Vec<AttachmentRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub via: Option<SlateLit>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub after_cut: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub after_limit: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub opens_thread: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_by: Option<SessionOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub harness: Option<SessionHarness>,
}

impl SessionStartEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.start";
}

impl Check for SessionStartEvent {
    /// Rules: pos: > 0.
    /// Nested checks: attachments, startedBy, harness.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.attachments.check().map_err(|e| e.under("attachments"))?;
        self.via.check().map_err(|e| e.under("via"))?;
        self.started_by.check().map_err(|e| e.under("startedBy"))?;
        self.harness.check().map_err(|e| e.under("harness"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("sessionId") { None => out.push(crate::types::check::missing::<Self>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
        if let Some(v) = obj.get("at") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } }
        if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        if let Some(v) = obj.get("pos") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_pos(&x) { out.push(e.under("pos")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("pos"))), } }
        if let Some(v) = obj.get("copiedFrom") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copiedFrom"))), } }
        if let Some(v) = obj.get("prompt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("prompt"))), } }
        if let Some(v) = obj.get("attachments") { match crate::types::check::read_present::<Vec<AttachmentRecord>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("attachments"))), } }
        if let Some(v) = obj.get("requestId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("requestId"))), } }
        if let Some(v) = obj.get("via") { match crate::types::check::read_present::<SlateLit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("via"))), } }
        if let Some(v) = obj.get("afterCut") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("afterCut"))), } }
        if let Some(v) = obj.get("afterLimit") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("afterLimit"))), } }
        if let Some(v) = obj.get("opensThread") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("opensThread"))), } }
        if let Some(v) = obj.get("startedBy") { match crate::types::check::read_present::<SessionOrigin>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedBy"))), } }
        if let Some(v) = obj.get("model") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } }
        if let Some(v) = obj.get("cwd") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cwd"))), } }
        if let Some(v) = obj.get("permissionMode") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("permissionMode"))), } }
        if let Some(v) = obj.get("effort") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("effort"))), } }
        if let Some(v) = obj.get("agent") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } }
        if let Some(v) = obj.get("tools") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tools"))), } }
        if let Some(v) = obj.get("harness") { match crate::types::check::read_present::<SessionHarness>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harness"))), } }
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
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "prompt" => <Option<String> as Check>::describe(rest),
                "attachments" => <Option<Vec<AttachmentRecord>> as Check>::describe(rest),
                "requestId" => <Option<String> as Check>::describe(rest),
                "via" => <Option<SlateLit> as Check>::describe(rest),
                "afterCut" => None,
                "afterLimit" => <Option<JsNumber> as Check>::describe(rest),
                "opensThread" => None,
                "startedBy" => <Option<SessionOrigin> as Check>::describe(rest),
                "model" => <Option<String> as Check>::describe(rest),
                "cwd" => <Option<String> as Check>::describe(rest),
                "permissionMode" => <Option<String> as Check>::describe(rest),
                "effort" => <Option<String> as Check>::describe(rest),
                "agent" => <Option<String> as Check>::describe(rest),
                "tools" => <Option<Vec<String>> as Check>::describe(rest),
                "harness" => <Option<SessionHarness> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionStartEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:136 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchHunk {
    pub old_start: JsInt,
    pub old_lines: JsInt,
    pub new_start: JsInt,
    pub new_lines: JsInt,
    pub lines: Vec<String>,
}

impl Check for PatchHunk {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("oldStart") { None => out.push(crate::types::check::missing::<Self>("oldStart")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("oldStart"))), } } }
        match obj.get("oldLines") { None => out.push(crate::types::check::missing::<Self>("oldLines")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("oldLines"))), } } }
        match obj.get("newStart") { None => out.push(crate::types::check::missing::<Self>("newStart")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("newStart"))), } } }
        match obj.get("newLines") { None => out.push(crate::types::check::missing::<Self>("newLines")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("newLines"))), } } }
        match obj.get("lines") { None => out.push(crate::types::check::missing::<Self>("lines")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lines"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "oldStart" => <i64 as Check>::describe(rest),
                "oldLines" => <i64 as Check>::describe(rest),
                "newStart" => <i64 as Check>::describe(rest),
                "newLines" => <i64 as Check>::describe(rest),
                "lines" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:147 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePatch {
    pub path: String,
    pub hunks: Vec<PatchHunk>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub moved_to: Option<String>,
}

impl Check for FilePatch {
    /// Nested checks: hunks.
    fn check(&self) -> Result<(), SchemaError> {
        self.hunks.check().map_err(|e| e.under("hunks"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("hunks") { None => out.push(crate::types::check::missing::<Self>("hunks")), Some(v) => { match crate::types::check::read::<Vec<PatchHunk>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hunks"))), } } }
        if let Some(v) = obj.get("movedTo") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("movedTo"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "hunks" => <Vec<PatchHunk> as Check>::describe(rest),
                "movedTo" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:154 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeltaEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    pub kind: DeltaKind,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cut: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub bytes: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub exit_code: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub duration_ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub patch: Option<Vec<FilePatch>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub patch_cut: Option<True>,
}

impl SessionDeltaEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.delta";
}

impl Check for SessionDeltaEvent {
    /// Rules: pos: > 0; line: > 0; cut: > 0; bytes: >= 0; durationMs: >= 0.
    /// Nested checks: kind, patch.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.kind.check().map_err(|e| e.under("kind"))?;
        if let Some(v) = self.line {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("line"))?;
        }
        if let Some(v) = self.cut {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("cut"))?;
        }
        if let Some(v) = self.bytes {
            crate::types::check::num_at_least(v.0.0, 0.0).map_err(|e| e.under("bytes"))?;
        }
        if let Some(v) = self.duration_ms {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("durationMs"))?;
        }
        self.patch.check().map_err(|e| e.under("patch"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("sessionId") { None => out.push(crate::types::check::missing::<Self>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
        if let Some(v) = obj.get("at") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } }
        if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        if let Some(v) = obj.get("pos") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_pos(&x) { out.push(e.under("pos")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("pos"))), } }
        if let Some(v) = obj.get("copiedFrom") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copiedFrom"))), } }
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<DeltaKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("text") { None => out.push(crate::types::check::missing::<Self>("text")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } } }
        if let Some(v) = obj.get("messageId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("messageId"))), } }
        if let Some(v) = obj.get("toolName") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("toolName"))), } }
        if let Some(v) = obj.get("toolUseId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("toolUseId"))), } }
        if let Some(v) = obj.get("isError") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("isError"))), } }
        if let Some(v) = obj.get("parentToolUseId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parentToolUseId"))), } }
        if let Some(v) = obj.get("cwd") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cwd"))), } }
        if let Some(v) = obj.get("line") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_line(&x) { out.push(e.under("line")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("line"))), } }
        if let Some(v) = obj.get("cut") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_cut(&x) { out.push(e.under("cut")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("cut"))), } }
        if let Some(v) = obj.get("bytes") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_bytes(&x) { out.push(e.under("bytes")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } }
        if let Some(v) = obj.get("exitCode") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exitCode"))), } }
        if let Some(v) = obj.get("durationMs") { match crate::types::check::read_present::<JsNumber>(v) { Ok(x) => { if let Err(e) = Self::rule_duration_ms(&x) { out.push(e.under("durationMs")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("durationMs"))), } }
        if let Some(v) = obj.get("patch") { match crate::types::check::read_present::<Vec<FilePatch>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("patch"))), } }
        if let Some(v) = obj.get("patchCut") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("patchCut"))), } }
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
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "kind" => <DeltaKind as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                "messageId" => <Option<String> as Check>::describe(rest),
                "toolName" => <Option<String> as Check>::describe(rest),
                "toolUseId" => <Option<String> as Check>::describe(rest),
                "isError" => <Option<bool> as Check>::describe(rest),
                "parentToolUseId" => <Option<String> as Check>::describe(rest),
                "cwd" => <Option<String> as Check>::describe(rest),
                "line" => <Option<i64> as Check>::describe(rest),
                "cut" => <Option<i64> as Check>::describe(rest),
                "bytes" => <Option<i64> as Check>::describe(rest),
                "exitCode" => <Option<i64> as Check>::describe(rest),
                "durationMs" => <Option<JsNumber> as Check>::describe(rest),
                "patch" => <Option<Vec<FilePatch>> as Check>::describe(rest),
                "patchCut" => None,
                _ => None,
            },
        }
    }
}
impl SessionDeltaEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_line(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_cut(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
    fn rule_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
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

/// TS: packages/protocol/src/views/session-events.ts:199 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDoneEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    pub result: TurnResult,
}

impl SessionDoneEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.done";
}

impl Check for SessionDoneEvent {
    /// Rules: pos: > 0.
    /// Nested checks: result.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.result.check().map_err(|e| e.under("result"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("sessionId") { None => out.push(crate::types::check::missing::<Self>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
        if let Some(v) = obj.get("at") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } }
        if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        if let Some(v) = obj.get("pos") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_pos(&x) { out.push(e.under("pos")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("pos"))), } }
        if let Some(v) = obj.get("copiedFrom") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copiedFrom"))), } }
        match obj.get("result") { None => out.push(crate::types::check::missing::<Self>("result")), Some(v) => { match crate::types::check::read::<TurnResult>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("result"))), } } }
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
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "result" => <TurnResult as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionDoneEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:205 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEndEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub exit_code: Option<JsNumber>,
    pub saw_result: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub unstarted: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub promptless: Option<True>,
}

impl SessionEndEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.end";
}

impl Check for SessionEndEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("pos"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("sessionId") { None => out.push(crate::types::check::missing::<Self>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
        if let Some(v) = obj.get("at") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } }
        if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        if let Some(v) = obj.get("pos") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_pos(&x) { out.push(e.under("pos")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("pos"))), } }
        if let Some(v) = obj.get("copiedFrom") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copiedFrom"))), } }
        match obj.get("exitCode") { None => out.push(crate::types::check::missing::<Self>("exitCode")), Some(v) => { match crate::types::check::read::<Option<JsNumber>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exitCode"))), } } }
        match obj.get("sawResult") { None => out.push(crate::types::check::missing::<Self>("sawResult")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sawResult"))), } } }
        if let Some(v) = obj.get("reason") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reason"))), } }
        if let Some(v) = obj.get("unstarted") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("unstarted"))), } }
        if let Some(v) = obj.get("prompt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("prompt"))), } }
        if let Some(v) = obj.get("promptless") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("promptless"))), } }
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
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "exitCode" => <Option<JsNumber> as Check>::describe(rest),
                "sawResult" => <bool as Check>::describe(rest),
                "reason" => <Option<String> as Check>::describe(rest),
                "unstarted" => None,
                "prompt" => <Option<String> as Check>::describe(rest),
                "promptless" => None,
                _ => None,
            },
        }
    }
}
impl SessionEndEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:224 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSteerEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub attachments: Option<Vec<AttachmentRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub via: Option<SlateLit>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub waiting: Option<bool>,
}

impl SessionSteerEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.steer";
}

impl Check for SessionSteerEvent {
    /// Rules: pos: > 0.
    /// Nested checks: attachments.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.attachments.check().map_err(|e| e.under("attachments"))?;
        self.via.check().map_err(|e| e.under("via"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("sessionId") { None => out.push(crate::types::check::missing::<Self>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
        if let Some(v) = obj.get("at") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } }
        if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        if let Some(v) = obj.get("pos") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_pos(&x) { out.push(e.under("pos")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("pos"))), } }
        if let Some(v) = obj.get("copiedFrom") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copiedFrom"))), } }
        match obj.get("prompt") { None => out.push(crate::types::check::missing::<Self>("prompt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("prompt"))), } } }
        if let Some(v) = obj.get("attachments") { match crate::types::check::read_present::<Vec<AttachmentRecord>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("attachments"))), } }
        if let Some(v) = obj.get("requestId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("requestId"))), } }
        if let Some(v) = obj.get("via") { match crate::types::check::read_present::<SlateLit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("via"))), } }
        if let Some(v) = obj.get("waiting") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("waiting"))), } }
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
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "prompt" => <String as Check>::describe(rest),
                "attachments" => <Option<Vec<AttachmentRecord>> as Check>::describe(rest),
                "requestId" => <Option<String> as Check>::describe(rest),
                "via" => <Option<SlateLit> as Check>::describe(rest),
                "waiting" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionSteerEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:243 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionQueuedEvent {
    pub workspace_id: String,
    pub thread_id: String,
    pub harness: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub waits_for: Option<String>,
}

impl SessionQueuedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.queued";
}

impl Check for SessionQueuedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        match obj.get("harness") { None => out.push(crate::types::check::missing::<Self>("harness")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harness"))), } } }
        match obj.get("prompt") { None => out.push(crate::types::check::missing::<Self>("prompt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("prompt"))), } } }
        if let Some(v) = obj.get("requestId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("requestId"))), } }
        if let Some(v) = obj.get("waitsFor") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("waitsFor"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "harness" => <String as Check>::describe(rest),
                "prompt" => <String as Check>::describe(rest),
                "requestId" => <Option<String> as Check>::describe(rest),
                "waitsFor" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:260 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHeldEvent {
    pub workspace_id: String,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub request_id: Option<String>,
}

impl SessionHeldEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.held";
}

impl Check for SessionHeldEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        if let Some(v) = obj.get("requestId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("requestId"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "requestId" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:275 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRowEvent {
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub row: Option<SessionView>,
}

impl SessionRowEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.row";
}

impl Check for SessionRowEvent {
    /// Nested checks: row.
    fn check(&self) -> Result<(), SchemaError> {
        self.row.check().map_err(|e| e.under("row"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        if let Some(v) = obj.get("row") { match crate::types::check::read_present::<SessionView>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("row"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "id" => <String as Check>::describe(rest),
                "row" => <Option<SessionView> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:298 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartingEvent {
    pub workspace_id: String,
    pub thread_id: String,
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub installs: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub request_id: Option<String>,
}

impl SessionStartingEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.starting";
}

impl Check for SessionStartingEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        match obj.get("harness") { None => out.push(crate::types::check::missing::<Self>("harness")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harness"))), } } }
        if let Some(v) = obj.get("installs") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("installs"))), } }
        if let Some(v) = obj.get("requestId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("requestId"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "harness" => <String as Check>::describe(rest),
                "installs" => None,
                "requestId" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:374 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNotifyEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    pub notify: String,
    pub text: String,
}

impl SessionNotifyEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.notify";
}

impl Check for SessionNotifyEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("pos"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("sessionId") { None => out.push(crate::types::check::missing::<Self>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
        if let Some(v) = obj.get("at") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } }
        if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        if let Some(v) = obj.get("pos") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_pos(&x) { out.push(e.under("pos")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("pos"))), } }
        if let Some(v) = obj.get("copiedFrom") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copiedFrom"))), } }
        match obj.get("notify") { None => out.push(crate::types::check::missing::<Self>("notify")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("notify"))), } } }
        match obj.get("text") { None => out.push(crate::types::check::missing::<Self>("text")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } } }
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
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "notify" => <String as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionNotifyEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/session-events.ts:384 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnChangedFile {
    pub path: String,
    pub kind: String,
    pub additions: JsNumber,
    pub deletions: JsNumber,
}

impl Check for TurnChangedFile {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("additions") { None => out.push(crate::types::check::missing::<Self>("additions")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("additions"))), } } }
        match obj.get("deletions") { None => out.push(crate::types::check::missing::<Self>("deletions")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("deletions"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "kind" => <String as Check>::describe(rest),
                "additions" => <JsNumber as Check>::describe(rest),
                "deletions" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/session-events.ts:395 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionChangesEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pos: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copied_from: Option<String>,
    pub from: String,
    pub to: String,
    pub files: Vec<TurnChangedFile>,
    pub moved: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub shared: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub others: Option<Vec<TurnChangedFile>>,
}

impl SessionChangesEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.changes";
}

impl Check for SessionChangesEvent {
    /// Rules: pos: > 0.
    /// Nested checks: files, others.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.pos {
            crate::types::check::num_above(v.0.0, 0.0).map_err(|e| e.under("pos"))?;
        }
        self.files.check().map_err(|e| e.under("files"))?;
        self.others.check().map_err(|e| e.under("others"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("sessionId") { None => out.push(crate::types::check::missing::<Self>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
        if let Some(v) = obj.get("at") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } }
        if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
        if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
        if let Some(v) = obj.get("pos") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_pos(&x) { out.push(e.under("pos")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("pos"))), } }
        if let Some(v) = obj.get("copiedFrom") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copiedFrom"))), } }
        match obj.get("from") { None => out.push(crate::types::check::missing::<Self>("from")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } } }
        match obj.get("to") { None => out.push(crate::types::check::missing::<Self>("to")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("to"))), } } }
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<Vec<TurnChangedFile>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("moved") { None => out.push(crate::types::check::missing::<Self>("moved")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("moved"))), } } }
        if let Some(v) = obj.get("shared") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("shared"))), } }
        if let Some(v) = obj.get("others") { match crate::types::check::read_present::<Vec<TurnChangedFile>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("others"))), } }
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
                "threadId" => <Option<String> as Check>::describe(rest),
                "pos" => <Option<i64> as Check>::describe(rest),
                "copiedFrom" => <Option<String> as Check>::describe(rest),
                "from" => <String as Check>::describe(rest),
                "to" => <String as Check>::describe(rest),
                "files" => <Vec<TurnChangedFile> as Check>::describe(rest),
                "moved" => <Vec<String> as Check>::describe(rest),
                "shared" => None,
                "others" => <Option<Vec<TurnChangedFile>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SessionChangesEvent {
    fn rule_pos(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            crate::types::check::num_above(v.0.0, 0.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanStepState {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "working")]
    Working,
    #[serde(rename = "done")]
    Done,
}

impl Check for PlanStepState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["pending", "working", "done"]))
    }
}

/// TS: packages/protocol/src/views/session-events.ts:408 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub text: String,
    pub state: PlanStepState,
}

impl Check for PlanStep {
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("text") { None => out.push(crate::types::check::missing::<Self>("text")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } } }
        match obj.get("state") { None => out.push(crate::types::check::missing::<Self>("state")), Some(v) => { match crate::types::check::read::<PlanStepState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "text" => <String as Check>::describe(rest),
                "state" => <PlanStepState as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

pub use super::session_events_more::*;

// ---- Hand-written below: the functions of the unit.

use std::collections::HashMap;

// views/session-events.ts:321 at a3b870a6e
// export const agentStartingLine = (agent: string, installs: boolean): string => (installs ? `${agent} ${FIRST_RUN_WORD}` : `Starting ${agent}`);
pub fn agent_starting_line(agent: &str, installs: bool) -> String {
    todo!()
}

/// What `host_from_env` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostFromEnv {
    pub url: String,
    pub token: String,
    pub host_key: Option<String>,
}

// views/session-events.ts:334 at a3b870a6e
// export function hostFromEnv(env: Readonly<Record<string, string | undefined>>): { url: string; token: string; hostKey?: string } | undefined {
//   const url = env[HOST_URL_ENV]?.trim();
//   const token = env[HOST_TOKEN_ENV]?.trim();
//   const hostKey = env[HOST_KEY_ENV]?.trim();
//   if (url === undefined || url === "" || token === undefined || token === "") return undefined;
//   return { url, token, ...(hostKey === undefined || hostKey === "" ? {} : { hostKey }) };
// }
/// The host a process inside a turn reaches: HOST_URL_ENV, HOST_TOKEN_ENV and HOST_KEY_ENV, each trimmed as
/// JavaScript's `trim` does; None where the url or the token is missing or empty, and no key where it is.
pub fn host_from_env(env: &HashMap<String, String>) -> Option<HostFromEnv> {
    todo!()
}

// views/session-events.ts:355 at a3b870a6e
// export const wspPackageIn = (dir: string): string => `${dir}/wsp`;
pub fn wsp_package_in(dir: &str) -> String {
    todo!()
}

// views/session-events.ts:356 at a3b870a6e
// export const wspBinIn = (dir: string): string => `${wspPackageIn(dir)}/dist/bin.js`;
pub fn wsp_bin_in(dir: &str) -> String {
    todo!()
}

// views/session-events.ts:365 at a3b870a6e
// export function turnTokenOf(env: Readonly<Record<string, string | undefined>>): string | undefined {
//   const token = env[TURN_TOKEN_ENV];
//   return token === undefined || token === "" ? undefined : token;
// }
/// TURN_TOKEN_ENV as it is, untrimmed; None where it is missing or empty.
pub fn turn_token_of(env: &HashMap<String, String>) -> Option<String> {
    todo!()
}

// views/session-events.ts:665 at a3b870a6e
// export const copiedFromOf = (e: SessionEvent): string | undefined => ("copiedFrom" in e ? e.copiedFrom : undefined);
/// The `copiedFrom` of an event whose type carries that key, None for any other.
pub fn copied_from_of(e: &SessionEvent) -> Option<String> {
    todo!()
}

// views/session-events.ts:673 at a3b870a6e
// export const isSessionEvent = (e: { type: string }): e is SessionEvent => SESSION_EVENT_TYPES.has(e.type as SessionEvent["type"]);
/// Whether an event type is one of SESSION_EVENT_TYPES.
pub fn is_session_event(event_type: &str) -> bool {
    todo!()
}
