// SPDX-License-Identifier: AGPL-3.0-only
//! The shell line that runs one OpenCode turn. TS adapter-opencode/src/command.ts. Spec: spec/opencode.md rules 1-4.

use indexmap::IndexMap;

use crate::port::{AgentLaunch, Env};

/// TS command.ts:9.
pub const PROMPT_END: &str = "WSP_PROMPT_END";

/// OpenCode's one access mode here. TS command.ts:16.
pub const AUTO_MODE: &str = "auto";

/// TS command.ts:18.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenCodeEnvOptions {
    pub base: Option<IndexMap<String, Option<String>>>,
    pub api_key: Option<String>,
    pub key_env: Option<String>,
}

/// TS command.ts:25.
pub fn build_env(options: &OpenCodeEnvOptions) -> Env {
    todo!()
}

/// TS command.ts:32.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildCommandOptions {
    pub prompt: String,
    pub resume: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub title: Option<String>,
    /// Absolute paths of images already on the machine.
    pub images: Vec<String>,
    pub launch: Option<AgentLaunch>,
}

/// TS command.ts:62.
pub fn build_command(options: &BuildCommandOptions) -> Result<String, String> {
    todo!()
}
