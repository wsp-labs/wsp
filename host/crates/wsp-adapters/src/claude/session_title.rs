// SPDX-License-Identifier: AGPL-3.0-only
//! What Claude Code itself calls a session, and naming one. TS: packages/adapter-claude/src/session-title.ts at
//! a3b870a6e.

use crate::port::{AgentLaunch, SessionRenameWrite};

/// session-title.ts:26.
pub fn session_title_command(config_dir: &str, session_id: &str) -> String {
    todo!()
}

/// session-title.ts:36. The last custom-title beats the last ai-title wherever each sits; blank ones are skipped.
pub fn parse_session_title(stdout: &str) -> Option<String> {
    todo!()
}

/// session-title.ts:74.
pub fn title_for_command(prompt: &str, model: Option<&str>, launch: Option<&AgentLaunch>, sign_in: Option<&str>) -> String {
    todo!()
}

/// session-title.ts:93.
pub fn draft_for_command(prompt_file: &str, model: Option<&str>, launch: Option<&AgentLaunch>, sign_in: Option<&str>) -> String {
    todo!()
}

/// session-title.ts:98. The first non-error result's text whole; `None` where it is blank.
pub fn parse_draft_for(stdout: &str) -> Option<String> {
    todo!()
}

/// session-title.ts:118. generated_title of the first non-error string result.
pub fn parse_title_for(stdout: &str) -> Option<String> {
    todo!()
}

/// session-title.ts:151.
pub fn rename_command(config_dir: &str, session_id: &str, title: &str) -> String {
    todo!()
}

/// session-title.ts:163.
pub fn parse_rename(stdout: &str) -> SessionRenameWrite {
    todo!()
}
