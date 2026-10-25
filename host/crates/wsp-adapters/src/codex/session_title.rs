// SPDX-License-Identifier: AGPL-3.0-only
//! What Codex calls a thread, read from its thread index; a rename through the app server; a title or a commit
//! message asked of `codex exec`. TS adapter-codex/src/session-title.ts. Spec: spec/codex_session_title.md.

use super::command::CodexBroker;
use crate::port::{AgentLaunch, SessionRenameWrite};

/// Sets `$d` to the highest-versioned state db in CODEX_HOME. TS session-title.ts:418.
pub const NEWEST_STATE_DB: &str = "d=$(ls -1 state_*.sqlite 2>/dev/null | sort -t_ -k2,2n | tail -n 1)";

/// TS session-title.ts:415.
pub const PROMPT_END: &str = "WSP_PROMPT_END";

/// TS session-title.ts:428.
pub fn session_title_command(home: &str, thread_id: &str) -> Result<String, String> {
    todo!()
}

/// TS session-title.ts:439.
pub fn parse_session_title(stdout: &str) -> Option<String> {
    todo!()
}

/// TS session-title.ts:466.
pub fn title_for_command(prompt: &str, model: Option<&str>, launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>, now_ms: i64) -> Result<String, String> {
    todo!()
}

/// TS session-title.ts:484.
pub fn draft_for_command(prompt_file: &str, model: Option<&str>, launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>, now_ms: i64) -> Result<String, String> {
    todo!()
}

/// TS session-title.ts:489.
pub fn parse_draft_for(stdout: &str) -> Option<String> {
    todo!()
}

/// TS session-title.ts:513.
pub fn parse_title_for(stdout: &str) -> Option<String> {
    todo!()
}

/// TS session-title.ts:547.
pub fn rename_command(thread_id: &str, title: &str, launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>) -> Result<String, String> {
    todo!()
}

/// TS session-title.ts:555.
pub fn parse_rename(stdout: &str) -> SessionRenameWrite {
    todo!()
}
