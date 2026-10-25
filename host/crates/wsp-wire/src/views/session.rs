// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/session.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
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

impl Check for SessionStatus {}

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

impl Check for SessionOrigin {}

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

impl Check for TitleSource {}

/// TS: packages/protocol/src/views/session.ts:28 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TurnRefusal {
    #[serde(rename = "sign-in")]
    SignIn,
}

impl Check for TurnRefusal {}

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

impl Check for PermissionOutcome {}

/// TS: packages/protocol/src/views/session.ts:37 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPrompt {
    pub ask_id: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub options: Vec<PermissionOption>,
}

impl Check for ThreadPrompt {
    /// Nested checks: options.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreadCapWaitWhy {
    #[serde(rename = "slot")]
    Slot,
    #[serde(rename = "minute")]
    Minute,
}

impl Check for ThreadCapWaitWhy {}

/// TS: packages/protocol/src/views/session.ts:58 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCapWait {
    pub place_id: String,
    pub place: String,
    pub running: i64,
    pub at_once: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<ThreadCapWaitWhy>,
}

impl Check for ThreadCapWait {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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

impl Check for ThreadSection {}

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
        todo!()
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
        todo!()
    }
}

/// TS: packages/protocol/src/views/session.ts:87 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThreadMarks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<ThreadMarksPinned>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub order: Option<Option<JsNumber>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub snoozed_until: Option<Option<JsNumber>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub section: Option<Option<ThreadPlacement>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_at_reset: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folded: Option<bool>,
}

impl Check for ThreadMarks {
    /// Rules: order: zod number check finite .
    /// Nested checks: pinned, section.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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

impl Check for SubagentState {}

/// TS: packages/protocol/src/views/session.ts:110 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<UsageTokens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_uses: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<JsNumber>,
}

impl Check for SubagentUsage {
    /// Rules: toolUses: >= 0; durationMs: >= 0.
    /// Nested checks: tokens.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session.ts:116 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentView {
    pub id: String,
    pub title: String,
    pub state: SubagentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked: Option<String>,
    pub started_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<SubagentUsage>,
}

impl Check for SubagentView {
    /// Rules: depth: > 0.
    /// Nested checks: state, usage.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session.ts:138 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkSource {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
}

impl Check for ForkSource {
    /// Rules: at: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session.ts:143 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkedFrom {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub title: String,
}

impl Check for ForkedFrom {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
        todo!()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_by: Option<SessionOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_source: Option<TitleSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<TurnRefusal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<TurnLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<ThreadWaitingOn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<SessionViewProject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<SubRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_kept_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capped: Option<ThreadCapWait>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waits_for: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagents: Option<Vec<SubagentView>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folded_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub woke_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<ThreadPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewound_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<ForkedFrom>,
}

impl Check for SessionView {
    /// Nested checks: status, startedBy, titleSource, refusal, limit, waitingOn, project, sub, capped, subagents, section, forkedFrom.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/session.ts:294 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadView {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub workspace_id: String,
    pub harness: String,
    pub started_by: SessionOrigin,
    pub status: SessionStatus,
    pub title: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub turns: i64,
    pub ran: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<ThreadWaitingOn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<SubRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_kept_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capped: Option<ThreadCapWait>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<TurnLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_pid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folded_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub woke_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<ThreadPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewound_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<ForkedFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagents: Option<Vec<SubagentView>>,
}

impl Check for ThreadView {
    /// Rules: turns: > 0.
    /// Nested checks: startedBy, status, waitingOn, sub, capped, limit, section, forkedFrom, subagents.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
