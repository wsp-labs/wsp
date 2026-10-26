// SPDX-License-Identifier: AGPL-3.0-only
//! What Codex calls a thread, read from its thread index; a rename through the app server; a title or a commit
//! message asked of `codex exec`. TS adapter-codex/src/session-title.ts. Spec: spec/codex_session_title.md.

use serde_json::{json, Value};

use super::command::{codex_launch, codex_word, slug, CodexBroker};
use crate::pending::{
    answers_of, app_server_initialize, app_server_notification, app_server_request, app_server_script, generated_title, ServerAnswer,
    ServerStage,
};
use crate::port::{AgentLaunch, SessionRenameWrite};
use wsp_wire::types::shell_quote::{in_folder, shell_quote, ENV_FROM_INPUT};
use wsp_words::js::js_trim;
use wsp_words::v8_json::parse_json_lax;

/// Sets `$d` to the highest-versioned state db in CODEX_HOME. TS session-title.ts:418.
pub const NEWEST_STATE_DB: &str = "d=$(ls -1 state_*.sqlite 2>/dev/null | sort -t_ -k2,2n | tail -n 1)";

/// TS session-title.ts:415.
pub const PROMPT_END: &str = "WSP_PROMPT_END";

const INIT: i64 = 1;
const NAME: i64 = 2;
const NO_ROLLOUT: &str = "no rollout found";

/// The JSON object each line of the output parses to, when it is a line that starts with `{` and holds an object.
fn object_lines(stdout: &str) -> impl Iterator<Item = serde_json::Map<String, Value>> + '_ {
    stdout.split('\n').filter_map(|raw| {
        let line = js_trim(raw);
        if !line.starts_with('{') {
            return None;
        }
        match parse_json_lax(line)? {
            Value::Object(row) => Some(row),
            _ => None,
        }
    })
}

/// The text of the last agent message the turn's events carry, a string even when empty. TS session-title.ts:489.
fn last_agent_message(stdout: &str) -> Option<String> {
    let mut text = None;
    for event in object_lines(stdout) {
        if event.get("type").and_then(Value::as_str) != Some("item.completed") {
            continue;
        }
        let Some(item) = event.get("item").and_then(Value::as_object) else {
            continue;
        };
        if item.get("type").and_then(Value::as_str) == Some("agent_message") {
            if let Some(message) = item.get("text").and_then(Value::as_str) {
                text = Some(message.to_string());
            }
        }
    }
    text
}

/// TS session-title.ts:428.
pub fn session_title_command(home: &str, thread_id: &str) -> Result<String, String> {
    let query =
        format!("select json_object('name', name, 'title', title) from threads where id = '{}' limit 1;", slug("threadId", thread_id)?);
    Ok(format!(
        "command -v sqlite3 > /dev/null 2>&1 || exit 0; cd {} 2>/dev/null || exit 0; {NEWEST_STATE_DB}; [ -n \"$d\" ] || exit 0; sqlite3 -readonly \"$d\" {} 2>/dev/null; true",
        shell_quote(home),
        shell_quote(&query)
    ))
}

/// TS session-title.ts:439.
pub fn parse_session_title(stdout: &str) -> Option<String> {
    let row = object_lines(stdout).next()?;
    ["name", "title"]
        .iter()
        .map(|column| js_trim(row.get(*column).and_then(Value::as_str).unwrap_or("")))
        .find(|title| !title.is_empty())
        .map(String::from)
}

/// The question line a title and a draft both ask: read-only, no approval asked. TS session-title.ts:74.
fn question_line(model: Option<&str>, launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>, now_ms: i64) -> Result<String, String> {
    let codex = codex_launch(launch, broker, now_ms)?;
    let model_flag = match model {
        None => String::new(),
        Some(model) => format!(" -m {}", slug("model", model)?),
    };
    let words: Vec<&str> = [codex.word.as_str(), "exec"].into_iter().chain(codex.flags.iter().map(String::as_str)).collect();
    Ok(format!(
        "{} --json --skip-git-repo-check -c sandbox_mode='\"read-only\"' -c approval_policy='\"never\"'{model_flag} -",
        words.join(" ")
    ))
}

/// TS session-title.ts:466.
pub fn title_for_command(
    prompt: &str,
    model: Option<&str>,
    launch: Option<&AgentLaunch>,
    broker: Option<&CodexBroker>,
    now_ms: i64,
) -> Result<String, String> {
    if prompt.split('\n').any(|line| line == PROMPT_END) {
        return Err(format!("the prompt has a line that reads {PROMPT_END}, which ends the prompt"));
    }
    let question = question_line(model, launch, broker, now_ms)?;
    Ok(format!("{ENV_FROM_INPUT}; {}", in_folder(None, &format!("{question} <<'{PROMPT_END}'\n{prompt}\n{PROMPT_END}"))))
}

/// TS session-title.ts:484.
pub fn draft_for_command(
    prompt_file: &str,
    model: Option<&str>,
    launch: Option<&AgentLaunch>,
    broker: Option<&CodexBroker>,
    now_ms: i64,
) -> Result<String, String> {
    let question = question_line(model, launch, broker, now_ms)?;
    Ok(format!("{ENV_FROM_INPUT}; {}", in_folder(None, &format!("{question} < {}", shell_quote(prompt_file)))))
}

/// TS session-title.ts:489.
pub fn parse_draft_for(stdout: &str) -> Option<String> {
    last_agent_message(stdout).filter(|text| !js_trim(text).is_empty())
}

/// TS session-title.ts:513.
pub fn parse_title_for(stdout: &str) -> Option<String> {
    last_agent_message(stdout).and_then(|text| generated_title(&text))
}

/// TS session-title.ts:547.
pub fn rename_command(thread_id: &str, title: &str, launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>) -> Result<String, String> {
    let codex = codex_word(launch, broker)?;
    let params = json!({"threadId": slug("threadId", thread_id)?, "name": title});
    let lines = vec![
        app_server_initialize(INIT),
        app_server_notification("initialized"),
        app_server_request(NAME, "thread/name/set", params.as_object().cloned().unwrap_or_default()),
    ];
    Ok(format!("cd ~ && {ENV_FROM_INPUT}\n{}", app_server_script(&codex, &[ServerStage { lines, answers: 2 }])))
}

/// TS session-title.ts:555.
pub fn parse_rename(stdout: &str) -> SessionRenameWrite {
    let answers = answers_of(stdout);
    match answers.get(&NAME) {
        None => SessionRenameWrite::Failed { error: "codex's app server did not answer the rename".into() },
        Some(ServerAnswer::Result(_)) => SessionRenameWrite::Written,
        Some(ServerAnswer::Error { message, .. }) if message.starts_with(NO_ROLLOUT) => SessionRenameWrite::NoSession,
        Some(ServerAnswer::Error { message, .. }) => SessionRenameWrite::Failed { error: message.clone() },
    }
}
