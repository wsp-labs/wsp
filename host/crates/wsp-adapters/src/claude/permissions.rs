// SPDX-License-Identifier: AGPL-3.0-only
//! Claude Code's side of the permission prompt: the control channel on the stream-json pair. TS:
//! packages/adapter-claude/src/permissions.ts at a3b870a6e. Every `*_line` is one JSON line, keys in the TS literal's
//! order, byte for byte what `JSON.stringify` writes.

use serde_json::{json, Map, Value};

use crate::claude::fields::{rec, string, Obj};
use crate::pending::{asked_questions, picked_options, question_answer_input, question_options};
use crate::port::{AdapterError, PermissionAsk, PERMISSION_ALLOW, PERMISSION_DENY};
use wsp_wire::text::json_stringify;
use wsp_wire::wire::helpers::{PermissionEffect, PermissionOption};

/// permissions.ts:22.
pub const PERMISSION_PROMPT_TOOL: &str = "stdio";
/// permissions.ts:28.
pub const SKIP_PROMPTS_MODE: &str = "bypassPermissions";

/// What one line is to the control channel. permissions.ts:42.
#[derive(Clone, Debug, PartialEq)]
pub enum ControlLine {
    Ask { ask: PermissionAsk, agent_id: Option<String> },
    Cancel { request_id: String },
    Unknown { request_id: String, subtype: String },
    Answer { request_id: String, error: Option<String> },
}

/// permissions.ts:59. `None` for an ordinary event.
pub fn control_line(event: &Obj) -> Option<ControlLine> {
    match string(event.get("type")) {
        Some("control_cancel_request") => string(event.get("request_id")).map(|id| ControlLine::Cancel { request_id: id.to_string() }),
        Some("control_response") => {
            let response = rec(event.get("response"));
            let request_id = string(response.and_then(|r| r.get("request_id")))?.to_string();
            let error = (string(response.and_then(|r| r.get("subtype"))) == Some("error"))
                .then(|| string(response.and_then(|r| r.get("error"))).unwrap_or("the CLI refused it without saying why").to_string());
            Some(ControlLine::Answer { request_id, error })
        }
        Some("control_request") => control_request(event),
        _ => None,
    }
}

fn control_request(event: &Obj) -> Option<ControlLine> {
    let request_id = string(event.get("request_id"))?;
    let request = rec(event.get("request"))?;
    let subtype = string(request.get("subtype")).unwrap_or("");
    let unknown = || ControlLine::Unknown { request_id: request_id.to_string(), subtype: subtype.to_string() };
    if subtype != "can_use_tool" {
        return Some(unknown());
    }
    let Some(tool_name) = string(request.get("tool_name")) else {
        return Some(unknown());
    };
    let mut modes: Vec<PermissionOption> = Vec::new();
    for suggestion in request.get("permission_suggestions").and_then(Value::as_array).into_iter().flatten() {
        let Some(suggestion) = rec(Some(suggestion)) else {
            continue;
        };
        let Some(mode) = string(suggestion.get("mode")) else {
            continue;
        };
        if string(suggestion.get("type")) != Some("setMode") || modes.iter().any(|o| o.mode.as_deref() == Some(mode)) {
            continue;
        }
        modes.push(PermissionOption {
            id: format!("mode:{mode}"),
            label: mode.to_string(),
            effect: PermissionEffect::Mode,
            mode: Some(mode.to_string()),
        });
    }
    let null = Value::Null;
    let input = json_stringify(request.get("input").unwrap_or(&null));
    let options = if asked_questions(tool_name, &input).is_some() {
        question_options(tool_name, &input)
    } else {
        let allow = PermissionOption { id: PERMISSION_ALLOW.into(), label: "Allow".into(), effect: PermissionEffect::Allow, mode: None };
        let deny = PermissionOption { id: PERMISSION_DENY.into(), label: "Deny".into(), effect: PermissionEffect::Deny, mode: None };
        [allow, deny].into_iter().chain(modes).collect()
    };
    let ask = PermissionAsk {
        ask_id: request_id.to_string(),
        tool_name: tool_name.to_string(),
        tool_use_id: string(request.get("tool_use_id")).map(String::from),
        parent_tool_use_id: None,
        input,
        detail: string(request.get("description")).filter(|d| !d.is_empty()).map(String::from),
        options,
    };
    Some(ControlLine::Ask { ask, agent_id: string(request.get("agent_id")).map(String::from) })
}

/// permissions.ts:122. `Err` with "<optionId> is not an option on this permission prompt" for an id the ask never
/// offered.
pub fn control_answer_line(ask: &PermissionAsk, option_id: &str, deny_message: &str) -> Result<String, AdapterError> {
    let not_offered = || AdapterError(format!("{option_id} is not an option on this permission prompt"));
    let picked = picked_options(&ask.options, option_id).ok_or_else(not_offered)?;
    let option = picked.first().ok_or_else(not_offered)?;
    let picked_ids: Vec<String> = picked.iter().map(|o| o.id.clone()).collect();
    let updated_input = match option.effect {
        PermissionEffect::Answer => question_answer_input(&ask.tool_name, &ask.input, &picked_ids).map(Value::Object),
        _ => None,
    };
    let updated_input = match updated_input {
        Some(answered) => answered,
        None => match wsp_words::v8_json::parse_json(&ask.input).map_err(AdapterError)? {
            Value::Null => json!({}),
            parsed => parsed,
        },
    };
    let response = match option.effect {
        PermissionEffect::Deny => json!({"behavior": "deny", "message": deny_message}),
        PermissionEffect::Mode => {
            let mut update = Map::new();
            update.insert("type".into(), json!("setMode"));
            if let Some(mode) = &option.mode {
                update.insert("mode".into(), json!(mode));
            }
            update.insert("destination".into(), json!("session"));
            json!({"behavior": "allow", "updatedInput": updated_input, "updatedPermissions": [Value::Object(update)]})
        }
        _ => json!({"behavior": "allow", "updatedInput": updated_input}),
    };
    Ok(json_stringify(
        &json!({"type": "control_response", "response": {"subtype": "success", "request_id": ask.ask_id, "response": response}}),
    ))
}

/// permissions.ts:139.
pub fn control_allow_line(ask: &PermissionAsk) -> Result<String, AdapterError> {
    control_answer_line(ask, PERMISSION_ALLOW, "")
}

/// permissions.ts:146.
pub fn mode_option_on(ask: &PermissionAsk, mode: &str) -> Option<String> {
    ask.options.iter().find(|o| o.effect == PermissionEffect::Mode && o.mode.as_deref() == Some(mode)).map(|o| o.id.clone())
}

/// permissions.ts:156.
pub fn set_mode_line(request_id: &str, mode: &str) -> String {
    json_stringify(
        &json!({"type": "control_request", "request_id": request_id, "request": {"subtype": "set_permission_mode", "mode": mode}}),
    )
}

/// permissions.ts:164.
pub fn stop_task_line(request_id: &str, task: &str) -> String {
    json_stringify(&json!({"type": "control_request", "request_id": request_id, "request": {"subtype": "stop_task", "task_id": task}}))
}

/// permissions.ts:171. `cancel_queued: true` rides only when asked.
pub fn interrupt_line(request_id: &str, cancel_queued: bool) -> String {
    let mut request = json!({"subtype": "interrupt"});
    if cancel_queued {
        request["cancel_queued"] = json!(true);
    }
    json_stringify(&json!({"type": "control_request", "request_id": request_id, "request": request}))
}

/// permissions.ts:178.
pub fn control_error_line(request_id: &str, subtype: &str) -> String {
    json_stringify(
        &json!({"type": "control_response", "response": {"subtype": "error", "request_id": request_id, "error": format!("wsp answers no {subtype} control request")}}),
    )
}
