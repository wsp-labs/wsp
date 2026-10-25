// SPDX-License-Identifier: AGPL-3.0-only
//! Claude Code's side of the permission prompt: the control channel on the stream-json pair. TS:
//! packages/adapter-claude/src/permissions.ts at a3b870a6e. Every `*_line` is one JSON line, keys in the TS literal's
//! order, byte for byte what `JSON.stringify` writes.

use crate::claude::fields::Obj;
use crate::port::{AdapterError, PermissionAsk};

/// permissions.ts:22.
pub const PERMISSION_PROMPT_TOOL: &str = "stdio";
/// permissions.ts:28.
pub const SKIP_PROMPTS_MODE: &str = "bypassPermissions";

/// What one line is to the control channel. permissions.ts:42.
#[derive(Clone, Debug, PartialEq)]
pub enum ControlLine {
    Ask { ask: PermissionAsk, agent_id: Option<String> },
    Cancel { request_id: String },
    Unknown { request_id: String, subtype: String },
    Answer { request_id: String, error: Option<String> },
}

/// permissions.ts:59. `None` for an ordinary event.
pub fn control_line(event: &Obj) -> Option<ControlLine> {
    todo!()
}

/// permissions.ts:122. `Err` with "<optionId> is not an option on this permission prompt" for an id the ask never
/// offered.
pub fn control_answer_line(ask: &PermissionAsk, option_id: &str, deny_message: &str) -> Result<String, AdapterError> {
    todo!()
}

/// permissions.ts:139.
pub fn control_allow_line(ask: &PermissionAsk) -> Result<String, AdapterError> {
    todo!()
}

/// permissions.ts:146.
pub fn mode_option_on(ask: &PermissionAsk, mode: &str) -> Option<String> {
    todo!()
}

/// permissions.ts:156.
pub fn set_mode_line(request_id: &str, mode: &str) -> String {
    todo!()
}

/// permissions.ts:164.
pub fn stop_task_line(request_id: &str, task: &str) -> String {
    todo!()
}

/// permissions.ts:171. `cancel_queued: true` rides only when asked.
pub fn interrupt_line(request_id: &str, cancel_queued: bool) -> String {
    todo!()
}

/// permissions.ts:178.
pub fn control_error_line(request_id: &str, subtype: &str) -> String {
    todo!()
}
