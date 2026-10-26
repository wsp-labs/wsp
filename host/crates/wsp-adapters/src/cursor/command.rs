// SPDX-License-Identifier: AGPL-3.0-only
//! The shell line that runs one Cursor turn. TS adapter-cursor/src/command.ts. Spec: spec/cursor.md rules 1-3.

use indexmap::IndexMap;

use crate::pending::{in_folder, shell_quote};
use crate::port::{agent_word, launch_words, AgentLaunch, Env};

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
    let mut env = Env::new();
    for (key, value) in options.base.iter().flatten() {
        if let Some(value) = value {
            env.insert(key.clone(), value.clone());
        }
    }
    if let (Some(api_key), Some(key_env)) = (&options.api_key, &options.key_env) {
        env.insert(key_env.clone(), api_key.clone());
    }
    env
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

/// `^[A-Za-z0-9][A-Za-z0-9-]*$`.
fn is_chat_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric()) && chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// TS command.ts:37.
pub fn build_command(options: &BuildCommandOptions) -> Result<String, String> {
    let BuildCommandOptions { prompt, resume, cwd, model, permission_mode, launch } = options;
    if prompt.split('\n').any(|line| line == PROMPT_END) {
        return Err(format!("the prompt has a line that reads {PROMPT_END}, which ends the prompt"));
    }
    if let Some(mode) = permission_mode {
        if mode != DEFAULT_MODE && mode != FORCE_MODE {
            return Err(format!("permissionMode must be one of {DEFAULT_MODE}, {FORCE_MODE}, got \"{mode}\""));
        }
    }
    if let Some(model) = model {
        if model.starts_with('-') || model.contains('\n') {
            return Err(format!("model must be one model name, got \"{model}\""));
        }
    }
    if let Some(resume) = resume {
        if !is_chat_id(resume) {
            return Err(format!("resume must be a chat id, got \"{resume}\""));
        }
    }
    let mut words = vec![format!("{} -p", agent_word("cursor-agent", launch.as_ref()))];
    words.extend(launch_words(launch.as_ref()));
    words.push("--output-format stream-json --stream-partial-output --trust".into());
    if let Some(cwd) = cwd {
        words.push(format!("--workspace {}", shell_quote(cwd)));
    }
    if let Some(model) = model {
        words.push(format!("--model {}", shell_quote(model)));
    }
    if let Some(resume) = resume {
        words.push(format!("--resume {resume}"));
    }
    if permission_mode.as_deref() == Some(FORCE_MODE) {
        words.push("--force".into());
    }
    let line = words.join(" ");
    Ok(in_folder(cwd.as_deref(), &format!("{line} <<'{PROMPT_END}'\n{prompt}\n{PROMPT_END}")))
}
