// SPDX-License-Identifier: AGPL-3.0-only
//! The shell line that runs one OpenCode turn. TS adapter-opencode/src/command.ts. Spec: spec/opencode.md rules 1-4.

use indexmap::IndexMap;

use crate::pending::{in_folder, shell_quote};
use crate::port::{agent_word, launch_words, AgentLaunch, Env};

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

/// `^[A-Za-z0-9][A-Za-z0-9._:-]*$`.
fn is_slug(value: &str) -> bool {
    starts_alnum_then(value, "._:-")
}

/// `^[A-Za-z0-9]` then the characters of `extra` or ASCII alphanumerics, to the end.
fn starts_alnum_then(value: &str, extra: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric()) && chars.all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
}

/// `^[A-Za-z0-9][A-Za-z0-9._-]*\/[A-Za-z0-9][A-Za-z0-9._:@/-]*$`: the provider ends at the first slash.
fn is_provider_model(value: &str) -> bool {
    match value.split_once('/') {
        Some((provider, model)) => starts_alnum_then(provider, "._-") && starts_alnum_then(model, "._:@/-"),
        None => false,
    }
}

fn slug(name: &str, value: &str) -> Result<String, String> {
    if is_slug(value) {
        Ok(value.to_string())
    } else {
        Err(format!("{name} must be a plain slug, got \"{value}\""))
    }
}

fn provider_model(value: &str) -> Result<String, String> {
    if is_provider_model(value) {
        Ok(value.to_string())
    } else {
        Err(format!("model must be provider/model, got \"{value}\""))
    }
}

fn image_path(path: &str) -> Result<String, String> {
    if !path.starts_with('/') || path.contains('\n') {
        return Err(format!("an image path must be one absolute path on the machine, got \"{path}\""));
    }
    Ok(shell_quote(path))
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
    let BuildCommandOptions { prompt, resume, cwd, model, effort, permission_mode, title, images, launch } = options;
    if prompt.split('\n').any(|line| line == PROMPT_END) {
        return Err(format!("the prompt has a line that reads {PROMPT_END}, which ends the prompt"));
    }
    if let Some(mode) = permission_mode {
        if mode != AUTO_MODE {
            return Err(format!("permissionMode must be {AUTO_MODE}, got \"{mode}\""));
        }
    }
    let mut words = vec![format!("{} run", agent_word("opencode", launch.as_ref()))];
    words.extend(launch_words(launch.as_ref()));
    words.push("--format json".into());
    words.push("--print-logs --log-level ERROR".into());
    if let Some(cwd) = cwd {
        words.push(format!("--dir {}", shell_quote(cwd)));
    }
    if let Some(resume) = resume {
        words.push(format!("-s {}", slug("resume", resume)?));
    }
    if let Some(model) = model {
        words.push(format!("-m {}", provider_model(model)?));
    }
    if let Some(effort) = effort {
        words.push(format!("--variant {}", slug("effort", effort)?));
    }
    if let Some(title) = title {
        words.push(format!("--title={}", shell_quote(title)));
    }
    if permission_mode.as_deref() == Some(AUTO_MODE) {
        words.push("--auto".into());
    }
    for path in images {
        words.push(format!("-f {}", image_path(path)?));
    }
    let line = words.join(" ");
    Ok(in_folder(cwd.as_deref(), &format!("{line} <<'{PROMPT_END}'\n{prompt}\n{PROMPT_END}")))
}
