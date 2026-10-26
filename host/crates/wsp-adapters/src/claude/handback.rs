// SPDX-License-Identifier: AGPL-3.0-only
//! How a subagent's end reaches the fold on Claude Code 2.1.295. TS: packages/adapter-claude/src/handback.ts at
//! a3b870a6e.

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use crate::claude::fields::{rec, string, Obj};
use crate::pending::NO_REPORT_LINE;
use crate::port::{DeltaKind, SubagentState, TurnDelta};
use wsp_words::js::js_trim;

const HANDBACK_TOOL: &str = "SubagentHandback";
/// The line the CLI opens a report it frames with; the report follows, each line indented by two spaces.
const REPORT_FRAME: &str = "[Subagent hand-back] ";

/// handback.ts:21.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HandbackBook {
    pub subagents: IndexSet<String>,
    /// Each task a subagent started, until its notification, with that subagent's handle.
    pub owned: IndexMap<String, String>,
    pub notified: IndexSet<String>,
    /// The launching calls whose subagents run in the background.
    pub backgrounded: IndexSet<String>,
    /// Each hand-back call until its result.
    pub held: IndexMap<String, TurnDelta>,
    /// The launching calls a notification answered, with that answer.
    pub answered: IndexMap<String, String>,
    /// The last words each subagent wrote, by its launching call.
    pub last_text: IndexMap<String, String>,
}

/// handback.ts:42.
pub fn note_line(book: &mut HandbackBook, event: &Obj) {
    let task = string(event.get("task_id"));
    let call = string(event.get("tool_use_id"));
    if string(event.get("type")) == Some("system") && string(event.get("subtype")) == Some("task_started") {
        if let Some(task) = task {
            let local = string(event.get("task_type")) == Some("local_agent");
            if let Some(owner) = string(event.get("parent_task_id")) {
                book.owned.insert(task.to_string(), owner.to_string());
            }
            if local {
                book.subagents.insert(task.to_string());
                if let (Some(Value::Bool(true)), Some(call)) = (event.get("is_backgrounded"), call) {
                    book.backgrounded.insert(call.to_string());
                }
            }
        }
    }
    if is_notification(event) {
        if let Some(task) = task {
            book.owned.shift_remove(task);
        }
    }
    let Some(parent) = string(event.get("parent_tool_use_id")) else {
        return;
    };
    let blocks = rec(event.get("message")).and_then(|m| m.get("content")).and_then(Value::as_array);
    if string(event.get("type")) != Some("assistant") {
        return;
    }
    for block in blocks.into_iter().flatten() {
        if string(rec(Some(block)).and_then(|b| b.get("type"))) == Some("text") {
            let text = string(rec(Some(block)).and_then(|b| b.get("text"))).unwrap_or("");
            book.last_text.insert(parent.to_string(), text.to_string());
        }
    }
}

/// handback.ts:60. Holds a SubagentHandback call made inside a subagent; false for any other call.
pub fn held_call(book: &mut HandbackBook, call: &TurnDelta) -> bool {
    let (Some(_), Some(tool_use_id)) = (&call.parent_tool_use_id, &call.tool_use_id) else {
        return false;
    };
    if call.tool_name.as_deref() != Some(HANDBACK_TOOL) {
        return false;
    }
    book.held.insert(tool_use_id.clone(), call.clone());
    true
}

fn parsed(text: &str) -> Option<Obj> {
    match wsp_words::v8_json::parse_json_lax(text) {
        Some(Value::Object(object)) => Some(object),
        _ => None,
    }
}

/// The report inside the CLI's frame, or nothing for an answer it did not frame.
fn unframed(text: &str) -> Option<String> {
    if !text.starts_with(REPORT_FRAME) {
        return None;
    }
    let lines: Vec<&str> = text.split('\n').skip(1).take_while(|line| line.starts_with("  ")).map(|line| &line[2..]).collect();
    Some(js_trim(&lines.join("\n")).to_string())
}

/// handback.ts:88. The deltas one tool_result draws, in order.
pub fn read_answer(book: &mut HandbackBook, answer: TurnDelta) -> Vec<TurnDelta> {
    let Some(call) = answer.tool_use_id.clone() else {
        return vec![answer];
    };
    if let Some(handback) = book.held.shift_remove(&call) {
        let said = parsed(&answer.text);
        if said.as_ref().and_then(|s| s.get("success")) == Some(&Value::Bool(true)) {
            return Vec::new();
        }
        let why = string(said.as_ref().and_then(|s| s.get("message"))).map(String::from);
        let refusal = match why {
            Some(why) => TurnDelta { text: why, is_error: Some(true), ..answer },
            None => answer,
        };
        return vec![handback, refusal];
    }
    if let Some(given) = book.answered.shift_remove(&call) {
        return if answer.parent_tool_use_id.is_none() { Vec::new() } else { vec![TurnDelta { text: given, ..answer }] };
    }
    match unframed(&answer.text) {
        Some(report) => vec![TurnDelta { text: report, ..answer }],
        None => vec![answer],
    }
}

fn is_notification(event: &Obj) -> bool {
    string(event.get("type")) == Some("system") && string(event.get("subtype")) == Some("task_notification")
}

/// handback.ts:114.
pub fn interim_end(book: &HandbackBook, event: &Obj) -> bool {
    let Some(task) = string(event.get("task_id")) else {
        return false;
    };
    if !is_notification(event) || !book.subagents.contains(task) || event.get("handback").is_some() {
        return false;
    }
    book.owned.values().any(|owner| owner == task)
}

/// handback.ts:121.
pub fn later_end(book: &HandbackBook, event: &Obj) -> bool {
    is_notification(event) && book.notified.contains(string(event.get("task_id")).unwrap_or(""))
}

/// A task the CLI says is over.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskEnd {
    pub task: String,
    pub state: SubagentState,
    /// Never `Some("")`.
    pub summary: Option<String>,
}

/// The report an end delivered: its text, and its warning where it names one (the text then carries the warning above it).
fn report_of(event: &Obj) -> Option<(String, Option<String>)> {
    let report = rec(event.get("handback_report"));
    let text = string(report.and_then(|r| r.get("text")))?;
    Some(match string(report.and_then(|r| r.get("warning"))) {
        None => (text.to_string(), None),
        Some(warning) => (format!("{warning}\n\n{text}"), Some(warning.to_string())),
    })
}

fn end_summary(event: &Obj) -> Option<String> {
    if event.get("handback").is_none() {
        return string(event.get("summary")).map(String::from);
    }
    match report_of(event) {
        None => Some(NO_REPORT_LINE.to_string()),
        Some((said, warning)) => Some(warning.unwrap_or(said)),
    }
}

/// handback.ts:137. completed→done, failed→failed, stopped and killed→stopped, any other word→failed.
pub fn task_ended(event: &Obj) -> Option<TaskEnd> {
    if string(event.get("type")) != Some("system") {
        return None;
    }
    let subtype = string(event.get("subtype"));
    let task = string(event.get("task_id"))?;
    let status = match subtype {
        Some("task_notification") => string(event.get("status")),
        Some("task_updated") => string(rec(event.get("patch")).and_then(|p| p.get("status"))),
        _ => None,
    }?;
    if subtype == Some("task_updated") && status != "killed" {
        return None;
    }
    let summary = if subtype == Some("task_notification") { end_summary(event) } else { None };
    let state = match status {
        "completed" => SubagentState::Done,
        "stopped" | "killed" => SubagentState::Stopped,
        _ => SubagentState::Failed,
    };
    Some(TaskEnd { task: task.to_string(), state, summary: summary.filter(|s| !s.is_empty()) })
}

/// handback.ts:159.
pub fn end_answer(book: &mut HandbackBook, event: &Obj, session_id: &str) -> Vec<TurnDelta> {
    let (Some(call), Some(task)) = (string(event.get("tool_use_id")), string(event.get("task_id"))) else {
        return Vec::new();
    };
    if !is_notification(event) || !book.subagents.contains(task) {
        return Vec::new();
    }
    book.notified.insert(task.to_string());
    let has_handback = event.get("handback").is_some();
    let report = if has_handback { report_of(event) } else { None };
    let status = string(event.get("status"));
    let failed = status == Some("failed");
    let answer = if has_handback {
        report.as_ref().map_or_else(|| NO_REPORT_LINE.to_string(), |(said, _)| said.clone())
    } else if book.backgrounded.contains(call) && (status == Some("completed") || failed) {
        string(event.get("summary")).unwrap_or("").to_string()
    } else {
        return Vec::new();
    };
    let words = match &report {
        None if has_handback => {
            vec![TurnDelta { parent_tool_use_id: Some(call.to_string()), ..TurnDelta::new(session_id, DeltaKind::Note, NO_REPORT_LINE) }]
        }
        None => Vec::new(),
        Some((said, _)) if book.last_text.get(call).map(|t| js_trim(t)) == Some(js_trim(said)) => Vec::new(),
        Some((said, _)) => {
            vec![TurnDelta { parent_tool_use_id: Some(call.to_string()), ..TurnDelta::new(session_id, DeltaKind::Text, said.as_str()) }]
        }
    };
    book.answered.insert(call.to_string(), answer.clone());
    let result = TurnDelta {
        tool_use_id: Some(call.to_string()),
        is_error: Some(failed),
        ..TurnDelta::new(session_id, DeltaKind::ToolResult, answer)
    };
    words.into_iter().chain([result]).collect()
}
