// SPDX-License-Identifier: AGPL-3.0-only
//! The shell line that runs one Cursor turn. TS adapter-cursor/src/command.ts. Spec: spec/cursor.md rules 1-3.

use indexmap::IndexMap;

use crate::port::{AgentLaunch, Env};

/// TS command.ts:3.
pub const PROMPT_END: &str = "WSP_PROMPT_END";
/// Proposes changes and applies none. TS command.ts:7.
pub const DEFAULT_MODE: &str = "default";
/// Edits and runs commands without asking. TS command.ts:9.
pub const FORCE_MODE: &str = "force";

/// TS command.ts:11.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CursorEnvOptions {
    pub base: Option<IndexMap<String, Option<String>>>,
    pub api_key: Option<String>,
    pub key_env: Option<String>,
}

/// TS command.ts:19.
pub fn build_env(options: &CursorEnvOptions) -> Env {
    todo!()
}

/// TS command.ts:26.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildCommandOptions {
    pub prompt: String,
    pub resume: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub launch: Option<AgentLaunch>,
}

/// TS command.ts:37.
pub fn build_command(options: &BuildCommandOptions) -> Result<String, String> {
    todo!()
}
