// SPDX-License-Identifier: AGPL-3.0-only
//! A side question on Claude Code. TS: packages/adapter-claude/src/aside.ts at a3b870a6e.

use indexmap::IndexMap;

use crate::claude::fields::Obj;
use crate::pending::McpServerSpec;
use crate::port::{AdapterError, AgentLaunch, AsideAnswer};

/// aside.ts:13.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AsideCommandOptions {
    pub fork: String,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub server_values: bool,
    pub memory_dir: Option<String>,
    pub launch: Option<AgentLaunch>,
}

/// aside.ts:39.
pub fn aside_command(options: &AsideCommandOptions) -> Result<String, AdapterError> {
    todo!()
}

/// aside.ts:58.
pub const ASIDE_HOOKS_ID: &str = "wsp-aside-hooks";

/// aside.ts:65.
pub fn aside_hooks_line() -> String {
    todo!()
}

/// aside.ts:70.
pub fn hook_deny_line(request_id: &str) -> String {
    todo!()
}

/// aside.ts:75.
pub fn prompt_deny_line(request_id: &str) -> String {
    todo!()
}

/// aside.ts:90, byte for byte.
pub fn aside_prompt(question: &str, running: &[String]) -> String {
    todo!()
}

/// aside.ts:106.
pub fn aside_text_of(event: &Obj) -> Option<String> {
    todo!()
}

/// aside.ts:117. `Err(words)` for the CLI's own reason it gave no answer.
pub fn aside_answer(event: &Obj) -> Result<AsideAnswer, String> {
    todo!()
}
