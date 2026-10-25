// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/session-events.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::literal;
use crate::types::attachments::AttachmentRecord;
use crate::types::js_number::JsNumber;
use crate::types::pending_wire::PermissionOption;
use crate::types::pending_wire::SessionSlateEvent;
use crate::types::usage::TurnLimit;
use crate::types::usage::UsageTokens;
use crate::views::session::PermissionOutcome;
use crate::views::session::SessionOrigin;
use crate::views::session::SessionView;
use crate::views::session::SubagentState;
use crate::views::session::SubagentUsage;
use crate::views::session::TurnRefusal;
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
pub const SESSION_EVENT_TYPES: &[&str] = &["session.start", "session.delta", "session.done", "session.end", "session.steer", "session.notify", "session.permission", "session.permission.closed", "session.checkpoint", "session.changes", "session.plan", "session.compacted", "session.retry", "session.context", "session.run", "session.moved", "session.behind", "session.notice", "session.earlier", "session.capped", "session.uncapped", "session.subagent", "session.slate", "thread.waited"];

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

impl Check for DeltaKind {}

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

impl Check for TurnStatus {}

/// TS: packages/protocol/src/views/session-events.ts:25 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnTokens {
    pub input: JsNumber,
    pub output: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<JsNumber>,
}

impl Check for TurnTokens {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnResultModelsItem {
    pub model: String,
    pub tokens: UsageTokens,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<JsNumber>,
}

impl Check for TurnResultModelsItem {
    /// Nested checks: tokens.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:36 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnResult {
    pub status: TurnStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waited_ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TurnTokens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<TurnResultModelsItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unreached: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<TurnRefusal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<TurnLimit>,
}

impl Check for TurnResult {
    /// Nested checks: status, tokens, models, refusal, limit.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:86 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHarness {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slash_commands: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<String>>,
}

impl Check for SessionHarness {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:93 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<AttachmentRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<SlateLit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_cut: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_limit: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opens_thread: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_by: Option<SessionOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:136 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchHunk {
    pub old_start: i64,
    pub old_lines: i64,
    pub new_start: i64,
    pub new_lines: i64,
    pub lines: Vec<String>,
}

impl Check for PatchHunk {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:147 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePatch {
    pub path: String,
    pub hunks: Vec<PatchHunk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_to: Option<String>,
}

impl Check for FilePatch {
    /// Nested checks: hunks.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:154 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeltaEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub kind: DeltaKind,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<Vec<FilePatch>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:199 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDoneEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:205 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEndEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub exit_code: Option<JsNumber>,
    pub saw_result: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unstarted: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promptless: Option<True>,
}

impl SessionEndEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.end";
}

impl Check for SessionEndEvent {
    /// Rules: pos: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:224 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSteerEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<AttachmentRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<SlateLit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        todo!()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waits_for: Option<String>,
}

impl SessionQueuedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.queued";
}

impl Check for SessionQueuedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:260 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHeldEvent {
    pub workspace_id: String,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl SessionHeldEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.held";
}

impl Check for SessionHeldEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:275 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRowEvent {
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<SessionView>,
}

impl SessionRowEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.row";
}

impl Check for SessionRowEvent {
    /// Nested checks: row.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:298 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartingEvent {
    pub workspace_id: String,
    pub thread_id: String,
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installs: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl SessionStartingEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.starting";
}

impl Check for SessionStartingEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:374 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNotifyEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        todo!()
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
        todo!()
    }
}

/// TS: packages/protocol/src/views/session-events.ts:395 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionChangesEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub from: String,
    pub to: String,
    pub files: Vec<TurnChangedFile>,
    pub moved: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        todo!()
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

impl Check for PlanStepState {}

/// TS: packages/protocol/src/views/session-events.ts:408 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub text: String,
    pub state: PlanStepState,
}

impl Check for PlanStep {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
