// SPDX-License-Identifier: AGPL-3.0-only

mod live;
mod prompts;
mod replies;
mod subagents;
mod turns;

use super::*;
use super::{
    facts::preview,
    model::{named_thread, starts_thread},
    plan::plan_title,
    prompt::{outcome_line, prompt_glyph, prompt_words},
};
use crate::words::{fmt_duration, fmt_tokens};
use serde_json::json;

fn event(value: serde_json::Value) -> SessionEvent {
    serde_json::from_value(value).unwrap()
}

fn base(kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
    let mut v = json!({"type": kind, "workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "u1", "pos": pos});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    event(v)
}

fn turn(done: bool) -> Vec<SessionEvent> {
    let mut events = vec![
        base("session.start", 1, json!({"prompt": "fix it", "at": 1_000.0})),
        base("session.delta", 2, json!({"kind": "text", "text": "Looking.", "messageId": "m1"})),
        base("session.delta", 3, json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"ls\"}"})),
        base("session.delta", 4, json!({"kind": "tool_result", "toolUseId": "a", "text": "x"})),
        base("session.delta", 5, json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "b", "text": "{\"file_path\":\"a.rs\"}"})),
        base("session.delta", 6, json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "c", "text": "{}"})),
        base("session.delta", 7, json!({"kind": "text", "text": "Done ", "messageId": "m2"})),
        base("session.delta", 8, json!({"kind": "text", "text": "here.", "messageId": "m2"})),
    ];
    if done {
        events.push(base("session.done", 9, json!({"result": {"status": "completed", "durationMs": 12_400}})));
        events.push(base("session.end", 10, json!({"exitCode": 0, "sawResult": true})));
    }
    events
}

fn agent(at: Option<f64>, meta: bool, streaming: bool) -> RowKind {
    RowKind::Agent { at, meta, streaming, rewind: None, fork: None, runs: Vec::new() }
}

fn tile(name: &str, bytes: u64, image: bool, index: usize, mime: &str) -> FileTile {
    FileTile { name: name.into(), bytes: Some(bytes), image, id: None, index, mime: Some(mime.into()) }
}

fn rows(events: &[SessionEvent], opened: &HashSet<String>) -> Vec<Row> {
    let mut t = Transcript { opened: opened.clone(), ..Transcript::default() };
    t.load(events.to_vec());
    t.rows
}

fn name(kind: &RowKind) -> &'static str {
    match kind {
        RowKind::Person { .. } => "person",
        RowKind::Agent { .. } => "agent",
        RowKind::Fold { .. } => "fold",
        RowKind::Stopped => "stopped",
        RowKind::Group { .. } => "group",
        RowKind::Live { .. } => "live",
        RowKind::Calls { .. } => "calls",
        RowKind::Work { .. } => "work",
        RowKind::Error => "error",
        RowKind::Working { .. } => "working",
        RowKind::Thinking => "thinking",
        RowKind::Footer { .. } => "footer",
        RowKind::Permission { .. } => "permission",
        RowKind::Question { .. } => "question",
        RowKind::Plan { .. } => "plan",
        RowKind::Subagent { .. } => "subagent",
        RowKind::Spawn { .. } => "spawn",
        RowKind::ChangedFiles { .. } => "changes",
        RowKind::Fork { .. } => "fork",
        RowKind::StartFailed { .. } => "start-failed",
        RowKind::Threads { .. } => "threads",
        RowKind::Rule => "rule",
        RowKind::Slate => "slate",
    }
}

fn shown(rows: &[Row]) -> Vec<(&'static str, &str)> {
    rows.iter().map(|r| (name(&r.kind), r.text.as_str())).collect()
}

fn opened(ids: &[&str]) -> HashSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

fn find<'a>(rows: &'a [Row], kind: &str) -> &'a Row {
    rows.iter().find(|r| name(&r.kind) == kind).unwrap_or_else(|| panic!("no {kind} row in {:?}", shown(rows)))
}
