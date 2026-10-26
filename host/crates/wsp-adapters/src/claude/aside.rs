// SPDX-License-Identifier: AGPL-3.0-only
//! A side question on Claude Code. TS: packages/adapter-claude/src/aside.ts at a3b870a6e.

use indexmap::IndexMap;
use serde_json::{json, Value};

use crate::claude::chain::js_string;
use crate::claude::fields::{string, Obj};
use crate::claude::landmines::{build_command, is_uuid, BuildCommandOptions};
use crate::pending::{McpServerSpec, ASIDE_TOOL_LINE};
use crate::port::{AdapterError, AgentLaunch, AsideAnswer};
use wsp_wire::text::json_stringify;
use wsp_words::js::js_trim;

const NO_TOOLS_HOOK: &str = "wsp-aside-no-tools";
const NO_TOOLS_WORDS: &str = "No tool runs in a side question; answer from the conversation.";

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
    if !is_uuid(&options.fork) {
        return Err(AdapterError(format!("session identifier must be a UUID, got \"{}\"", options.fork)));
    }
    build_command(&BuildCommandOptions {
        resume: Some(options.fork.clone()),
        aside: true,
        cwd: options.cwd.clone(),
        model: options.model.clone(),
        effort: options.effort.clone(),
        context_window: options.context_window.clone(),
        fast: options.fast.filter(|fast| *fast),
        mcp_servers: options.mcp_servers.clone(),
        server_values: options.server_values,
        memory_dir: options.memory_dir.clone(),
        launch: options.launch.clone(),
        ..Default::default()
    })
}

/// aside.ts:58.
pub const ASIDE_HOOKS_ID: &str = "wsp-aside-hooks";

/// aside.ts:65.
pub fn aside_hooks_line() -> String {
    json_stringify(
        &json!({"type": "control_request", "request_id": ASIDE_HOOKS_ID, "request": {"subtype": "initialize", "hooks": {"PreToolUse": [{"matcher": null, "hookCallbackIds": [NO_TOOLS_HOOK]}]}}}),
    )
}

/// aside.ts:70.
pub fn hook_deny_line(request_id: &str) -> String {
    json_stringify(
        &json!({"type": "control_response", "response": {"subtype": "success", "request_id": request_id, "response": {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": NO_TOOLS_WORDS}}}}),
    )
}

/// aside.ts:75.
pub fn prompt_deny_line(request_id: &str) -> String {
    json_stringify(
        &json!({"type": "control_response", "response": {"subtype": "success", "request_id": request_id, "response": {"behavior": "deny", "message": NO_TOOLS_WORDS}}}),
    )
}

/// aside.ts:90, byte for byte.
pub fn aside_prompt(question: &str, running: &[String]) -> String {
    let mut lines = vec![
        "<system-reminder>This is a side question from the person, answered by a separate copy of this conversation.".to_string(),
        "The thread itself was not interrupted and is not waiting on this: its turn goes on working in the background.".to_string(),
        r#"A "Continue from where you left off." and a "No response requested." just before this are the copy being opened, not the person stopping you."#.to_string(),
        "So is every task-notification just before this, for a background command or a background agent, whatever its status says (stopped, failed or another), that says the task didn't finish before the previous session ended or was running when the previous Claude Code process exited, or that its state was lost: opening this copy wrote it. That task did not fail, stop or get cut off, and in the thread it goes on running; its result is not in this conversation yet.".to_string(),
    ];
    if !running.is_empty() {
        lines.push(format!(
            "This copy ends before the tool call the thread is running right now, whose result has not come back yet: {}.",
            running.join("; ")
        ));
    }
    lines.extend([
        "The person never saw these lines, and a question about the thread's work is not one about them: answer about the work, and do not mention these lines, a notification or this copy unless the person asks about them by name.".to_string(),
        "Answer in this one reply from what the conversation already holds. No tool runs here: call none, and do not offer to look something up or say what you will do next.".to_string(),
        "If the conversation does not hold the answer, say so.</system-reminder>".to_string(),
        String::new(),
        question.to_string(),
    ]);
    lines.join("\n")
}

/// aside.ts:106.
pub fn aside_text_of(event: &Obj) -> Option<String> {
    if string(event.get("type")) != Some("stream_event") {
        return None;
    }
    let inner = event.get("event")?.as_object()?;
    if string(inner.get("type")) != Some("content_block_delta") {
        return None;
    }
    let delta = inner.get("delta")?.as_object()?;
    let text = string(delta.get("text"))?;
    (string(delta.get("type")) == Some("text_delta") && !text.is_empty()).then(|| text.to_string())
}

/// aside.ts:117. `Err(words)` for the CLI's own reason it gave no answer.
pub fn aside_answer(event: &Obj) -> Result<AsideAnswer, String> {
    let text = string(event.get("result")).unwrap_or("");
    if string(event.get("subtype")) == Some("error_max_turns") {
        return Err(ASIDE_TOOL_LINE.to_string());
    }
    if event.get("is_error") == Some(&Value::Bool(true)) || string(event.get("subtype")) != Some("success") {
        let first_error = event
            .get("errors")
            .and_then(Value::as_array)
            .and_then(|errors| errors.iter().filter_map(Value::as_str).find(|error| !error.starts_with("[ede_diagnostic]")));
        let subtype = match event.get("subtype") {
            None | Some(Value::Null) => "no result".to_string(),
            other => js_string(other),
        };
        let words = match js_trim(text) {
            "" => {
                first_error.filter(|error| !error.is_empty()).map_or(format!("claude ended the side question with {subtype}"), String::from)
            }
            trimmed => trimmed.to_string(),
        };
        return Err(words);
    }
    let usage: Option<IndexMap<String, Value>> =
        event.get("usage").and_then(Value::as_object).map(|usage| usage.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
    Ok(AsideAnswer { text: text.to_string(), usage })
}
