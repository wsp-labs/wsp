// SPDX-License-Identifier: AGPL-3.0-only
//! What Claude Code itself calls a session, and naming one. TS: packages/adapter-claude/src/session-title.ts at
//! a3b870a6e.

use serde_json::{json, Value};

use crate::claude::fields::{parse_line, string};
use crate::claude::landmines::KEY_ENV;
use crate::pending::{generated_title, shell_quote};
use crate::port::{program_word, AgentLaunch, SessionRenameWrite};
use wsp_wire::text::json_stringify;
use wsp_wire::types::shell_quote::ENV_FROM_INPUT;
use wsp_words::js::js_trim;

const AI_TITLE: &str = r#""type":"ai-title""#;
const CUSTOM_TITLE: &str = r#""type":"custom-title""#;
const WROTE: &str = "wrote";
const NO_SESSION: &str = "no-session";
const FAILED: &str = "failed";

/// session-title.ts:26.
pub fn session_title_command(config_dir: &str, session_id: &str) -> String {
    let file = format!("{}/*/{}", shell_quote(&format!("{config_dir}/projects")), shell_quote(&format!("{session_id}.jsonl")));
    format!("f=$(ls -1td {file} 2>/dev/null | head -n 1); [ -n \"$f\" ] || exit 0; grep -F {} \"$f\" | tail -n 1; grep -F {} \"$f\" | tail -n 1; true", shell_quote(CUSTOM_TITLE), shell_quote(AI_TITLE))
}

/// session-title.ts:36. The last custom-title beats the last ai-title wherever each sits; blank ones are skipped.
pub fn parse_session_title(stdout: &str) -> Option<String> {
    let mut renamed: Option<String> = None;
    let mut generated: Option<String> = None;
    for record in stdout.split('\n').filter_map(parse_line) {
        match string(record.get("type")) {
            Some("custom-title") => renamed = string(record.get("customTitle")).map(String::from).or(renamed),
            Some("ai-title") => generated = string(record.get("aiTitle")).map(String::from).or(generated),
            _ => {}
        }
    }
    [renamed, generated].into_iter().flatten().map(|title| js_trim(&title).to_string()).find(|title| !title.is_empty())
}

/// The print-mode question as the title asks it: the person's customizations off, no tool, and no session kept.
fn question_line(model: Option<&str>, launch: Option<&AgentLaunch>) -> String {
    let mut words = vec![format!("{} -p", program_word("claude", launch)), "--safe-mode".into(), "--output-format json".into(), "--tools ''".into(), "--no-session-persistence".into()];
    if let Some(model) = model {
        words.push(format!("--model {}", shell_quote(model)));
    }
    words.join(" ")
}

/// Inherited Claude Code marks and the inherited key dropped, then the question's own environment read off its input.
fn clean() -> String {
    format!("unset ${{!CLAUDE_CODE_@}} CLAUDECODE FORCE_CODE_TERMINAL {KEY_ENV}; {ENV_FROM_INPUT}")
}

/// session-title.ts:74.
pub fn title_for_command(prompt: &str, model: Option<&str>, launch: Option<&AgentLaunch>, sign_in: Option<&str>) -> String {
    format!("cd ~ && {}; {}printf '%s' {} | {}", clean(), sign_in.unwrap_or(""), shell_quote(prompt), question_line(model, launch))
}

/// session-title.ts:93.
pub fn draft_for_command(prompt_file: &str, model: Option<&str>, launch: Option<&AgentLaunch>, sign_in: Option<&str>) -> String {
    format!("cd ~ && {}; {}{} < {}", clean(), sign_in.unwrap_or(""), question_line(model, launch), shell_quote(prompt_file))
}

/// The result field of the first answer that is a result, not an error, and carries a string.
fn first_result(stdout: &str) -> Option<String> {
    let answer = stdout.split('\n').filter_map(parse_line).find(|answer| {
        string(answer.get("type")) == Some("result") && answer.get("is_error") != Some(&Value::Bool(true)) && string(answer.get("result")).is_some()
    })?;
    string(answer.get("result")).map(String::from)
}

/// session-title.ts:98. The first non-error result's text whole; `None` where it is blank.
pub fn parse_draft_for(stdout: &str) -> Option<String> {
    first_result(stdout).filter(|result| !js_trim(result).is_empty())
}

/// session-title.ts:118. generated_title of the first non-error string result.
pub fn parse_title_for(stdout: &str) -> Option<String> {
    first_result(stdout).and_then(|result| generated_title(&result))
}

/// session-title.ts:151.
pub fn rename_command(config_dir: &str, session_id: &str, title: &str) -> String {
    let file = format!("{}/*/{}", shell_quote(&format!("{config_dir}/projects")), shell_quote(&format!("{session_id}.jsonl")));
    let record = json_stringify(&json!({"type": "custom-title", "customTitle": title, "sessionId": session_id}));
    format!(
        "f=$(ls -1td {file} 2>/dev/null | head -n 1); [ -n \"$f\" ] || {{ echo {NO_SESSION}; exit 0; }}; [ -z \"$(tail -c 1 \"$f\")\" ] || printf '\n' >> \"$f\" 2>/dev/null; e=$({{ printf '%s\n' {} >> \"$f\"; }} 2>&1) && echo {WROTE} || printf '%s %s\n' {FAILED} \"$e\"",
        shell_quote(&record)
    )
}

/// session-title.ts:163.
pub fn parse_rename(stdout: &str) -> SessionRenameWrite {
    let lines: Vec<&str> = stdout.split('\n').map(js_trim).collect();
    if lines.contains(&WROTE) {
        return SessionRenameWrite::Written;
    }
    if lines.contains(&NO_SESSION) {
        return SessionRenameWrite::NoSession;
    }
    let said: Vec<&str> = lines.iter().copied().filter(|line| !line.is_empty()).collect();
    let error = match said.iter().find_map(|line| line.strip_prefix("failed ")) {
        Some(rest) => rest.to_string(),
        None if said.is_empty() => "the machine said nothing about the write".to_string(),
        None => said.join("; "),
    };
    SessionRenameWrite::Failed { error }
}
