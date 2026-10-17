// SPDX-License-Identifier: AGPL-3.0-only
//! A thread's events folded into the rows the transcript shows, the way the web app's timeline shows them by default:
//! a settled turn keeps the person's message, one "Worked for" line and its last agent message, and consecutive tool
//! calls read as one summary line. No GPUI here, so the fold is tested on its own.

use std::collections::{HashMap, HashSet};

use wsp_host_client::protocol::{SessionEvent, SessionEventKind, SessionEventResultStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// What the person sent, as markdown.
    Person,
    /// One message of the agent's, as markdown.
    Agent,
    /// The hidden work of a settled turn in one line; opens the turn when clicked.
    Fold,
    /// Consecutive tool calls in one line, such as "Ran 3 commands and read 2 files".
    Calls,
    Notice,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Stays the same for the same row across folds, so the view keeps what it built for it.
    pub key: String,
    pub kind: RowKind,
    pub text: String,
    pub turn: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Action {
    Read,
    Edit,
    Command,
    Search,
    CodeSearch,
    Other,
}

enum Work {
    Tool { action: Action, file: Option<String> },
    Thinking,
    Notice { text: String, stands_alone: bool },
    Error(String),
}

enum Body {
    Person(String),
    Agent(String),
    Work(Work),
}

struct Entry {
    key: String,
    turn: Option<String>,
    body: Body,
}

#[derive(Default)]
struct Turn {
    started: Option<f64>,
    last: Option<f64>,
    status: Option<SessionEventResultStatus>,
    worked_ms: Option<f64>,
    ended: bool,
    terminal: Option<String>,
}

impl Turn {
    fn settled(&self) -> bool {
        self.status.is_some() || self.ended
    }

    fn duration(&self) -> Option<String> {
        let ms = self.worked_ms.or_else(|| Some(self.last? - self.started?))?;
        Some(duration(ms))
    }
}

/// Every row the transcript shows for `events`, oldest first. A turn whose id is in `opened` shows all its rows.
pub fn rows(events: &[SessionEvent], opened: &HashSet<String>) -> Vec<Row> {
    let (entries, turns) = entries(events);
    let mut rows = Vec::new();
    let mut folded: HashSet<&str> = HashSet::new();
    let mut calls: Vec<&Entry> = Vec::new();
    let mut last_of_turn: HashMap<&str, usize> = HashMap::new();
    for (ix, entry) in entries.iter().enumerate() {
        if let Some(turn) = &entry.turn {
            last_of_turn.insert(turn, ix);
        }
    }
    for (ix, entry) in entries.iter().enumerate() {
        let turn = entry.turn.as_deref().and_then(|id| turns.get(id).map(|t| (id, t)));
        let hides =
            turn.is_some_and(|(id, t)| t.settled() && t.status != Some(SessionEventResultStatus::Interrupted) && !opened.contains(id));
        let kept = match &entry.body {
            Body::Person(_) => true,
            Body::Agent(_) => turn.is_none_or(|(_, t)| t.terminal.as_deref() == Some(entry.key.as_str())),
            Body::Work(Work::Error(_)) | Body::Work(Work::Notice { stands_alone: true, .. }) => true,
            Body::Work(_) => false,
        };
        let grouped = matches!(entry.body, Body::Work(Work::Tool { .. } | Work::Thinking));
        if !(grouped && !hides) {
            flush(&mut calls, &mut rows);
        }
        if hides && !kept {
            let (id, t) = turn.expect("a hidden row has a turn");
            if folded.insert(id) {
                let label = t.duration().map_or("Worked".to_owned(), |d| format!("Worked for {d}"));
                rows.push(Row { key: format!("fold:{id}"), kind: RowKind::Fold, text: label, turn: Some(id.to_owned()) });
            }
        } else if grouped {
            if calls.last().is_some_and(|last| last.turn != entry.turn) {
                flush(&mut calls, &mut rows);
            }
            calls.push(entry);
        } else {
            let (kind, text) = match &entry.body {
                Body::Person(text) => (RowKind::Person, text.clone()),
                Body::Agent(text) => (RowKind::Agent, text.clone()),
                Body::Work(Work::Notice { text, .. }) => (RowKind::Notice, text.clone()),
                Body::Work(Work::Error(text)) => (RowKind::Error, text.clone()),
                Body::Work(_) => unreachable!("tool calls and thinking are grouped"),
            };
            rows.push(Row { key: entry.key.clone(), kind, text, turn: entry.turn.clone() });
        }
        if let Some((id, t)) = turn {
            if t.status == Some(SessionEventResultStatus::Interrupted) && last_of_turn.get(id) == Some(&ix) {
                flush(&mut calls, &mut rows);
                let text = t.duration().map_or("You stopped this response".to_owned(), |d| format!("You stopped after {d}"));
                rows.push(Row { key: format!("stop:{id}"), kind: RowKind::Notice, text, turn: Some(id.to_owned()) });
            }
        }
    }
    flush(&mut calls, &mut rows);
    rows
}

fn flush(calls: &mut Vec<&Entry>, rows: &mut Vec<Row>) {
    let Some(first) = calls.first() else { return };
    rows.push(Row { key: format!("calls:{}", first.key), kind: RowKind::Calls, text: summary(calls), turn: first.turn.clone() });
    calls.clear();
}

/// summarizeToolGroup in apps/web/src/components/chat/timeline/workGroup.tsx.
fn summary(calls: &[&Entry]) -> String {
    let mut order: Vec<Action> = Vec::new();
    let mut counts: HashMap<Action, (usize, HashSet<&str>)> = HashMap::new();
    for entry in calls {
        let Body::Work(Work::Tool { action, file }) = &entry.body else { continue };
        if !order.contains(action) {
            order.push(*action);
        }
        let (bare, files) = counts.entry(*action).or_default();
        match file {
            Some(file) if *action == Action::Edit => _ = files.insert(file),
            _ => *bare += 1,
        }
    }
    if order.is_empty() {
        return "Thinking".to_owned();
    }
    let labels: Vec<String> = order
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let (bare, files) = &counts[action];
            let label = label(*action, bare + files.len());
            if i == 0 {
                label
            } else {
                label[..1].to_lowercase() + &label[1..]
            }
        })
        .collect();
    match labels.as_slice() {
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
        [] => unreachable!(),
    }
}

fn label(action: Action, n: usize) -> String {
    let (verb, one, many) = match action {
        Action::Read => ("Read", "file", "files"),
        Action::Edit => ("Changed", "file", "files"),
        Action::Command => ("Ran", "command", "commands"),
        Action::Search => ("Searched the web", "time", "times"),
        Action::CodeSearch => ("Searched code", "time", "times"),
        Action::Other => ("Used", "tool", "tools"),
    };
    format!("{verb} {n} {}", if n == 1 { one } else { many })
}

/// The kind of call a tool name is, after TOOL_ROWS in packages/protocol/src/words/tools.ts.
fn action(tool: &str) -> Action {
    match tool {
        "Bash" | "command_execution" => Action::Command,
        "Read" => Action::Read,
        "Write" | "Edit" | "MultiEdit" | "NotebookEdit" | "file_change" => Action::Edit,
        "Grep" | "Glob" => Action::CodeSearch,
        "WebSearch" | "web_search" | "WebFetch" => Action::Search,
        _ => Action::Other,
    }
}

fn entries(events: &[SessionEvent]) -> (Vec<Entry>, HashMap<String, Turn>) {
    let mut entries: Vec<Entry> = Vec::new();
    let mut turns: HashMap<String, Turn> = HashMap::new();
    let mut current: Option<String> = None;
    let mut starts = 0usize;
    // The agent message text deltas are appended to, and the harness message id it came under.
    let mut open: Option<(usize, Option<String>)> = None;
    let mut tools: HashMap<String, usize> = HashMap::new();
    for (ix, event) in events.iter().enumerate() {
        let key = |kind: &str| format!("{kind}:{}", wsp_host_client::pos(event).map_or(format!("i{ix}"), |p| p.to_string()));
        match event {
            SessionEvent::SessionStart { prompt, turn_id, session_id, at, after_limit, .. } => {
                starts += 1;
                let id = turn_id.clone().unwrap_or_else(|| format!("{session_id}#{starts}"));
                turns.insert(id.clone(), Turn { started: *at, last: *at, ..Turn::default() });
                current = Some(id);
                open = None;
                if let (Some(prompt), None) = (prompt, after_limit) {
                    entries.push(Entry { key: key("person"), turn: None, body: Body::Person(prompt.clone()) });
                }
            }
            SessionEvent::SessionSteer { prompt, .. } => {
                open = None;
                entries.push(Entry { key: key("person"), turn: None, body: Body::Person(prompt.clone()) });
            }
            SessionEvent::SessionDelta { kind, text, message_id, tool_name, tool_use_id, parent_tool_use_id, turn_id, at, .. } => {
                if parent_tool_use_id.is_some() {
                    continue;
                }
                let turn = turn_id.clone().or_else(|| current.clone());
                if let Some(t) = turn.as_ref().and_then(|id| turns.get_mut(id)) {
                    t.last = at.or(t.last);
                }
                match kind {
                    SessionEventKind::Text => {
                        if let Some((at, id)) = &open {
                            if message_id.is_none() || id.is_none() || message_id == id {
                                if let Body::Agent(said) = &mut entries[*at].body {
                                    said.push_str(text);
                                    continue;
                                }
                            }
                        }
                        open = Some((entries.len(), message_id.clone()));
                        if let Some(t) = turn.as_ref().and_then(|id| turns.get_mut(id)) {
                            t.terminal = Some(key("agent"));
                        }
                        entries.push(Entry { key: key("agent"), turn, body: Body::Agent(text.clone()) });
                    }
                    SessionEventKind::Note => {
                        entries.push(Entry {
                            key: key("note"),
                            turn,
                            body: Body::Work(Work::Notice { text: text.clone(), stands_alone: false }),
                        });
                    }
                    SessionEventKind::Thinking => {
                        open = None;
                        let continues = entries.last().is_some_and(|e| e.turn == turn && matches!(e.body, Body::Work(Work::Thinking)));
                        if !continues {
                            entries.push(Entry { key: key("thinking"), turn, body: Body::Work(Work::Thinking) });
                        }
                    }
                    SessionEventKind::ToolUse => {
                        open = None;
                        if tool_use_id.as_ref().is_some_and(|id| tools.contains_key(id)) {
                            continue;
                        }
                        let file = serde_json::from_str::<serde_json::Value>(text)
                            .ok()
                            .and_then(|input| input.get("file_path").or(input.get("notebook_path"))?.as_str().map(str::to_owned));
                        let action = action(tool_name.as_deref().unwrap_or("tool"));
                        if let Some(id) = tool_use_id {
                            tools.insert(id.clone(), entries.len());
                        }
                        entries.push(Entry { key: key("tool"), turn, body: Body::Work(Work::Tool { action, file }) });
                    }
                    SessionEventKind::ToolResult => {
                        open = None;
                        if tool_use_id.as_ref().is_some_and(|id| tools.contains_key(id)) {
                            continue;
                        }
                        let action = action(tool_name.as_deref().unwrap_or("tool"));
                        entries.push(Entry { key: key("tool"), turn, body: Body::Work(Work::Tool { action, file: None }) });
                    }
                }
            }
            SessionEvent::SessionDone { result, turn_id, at, .. } => {
                let turn = turn_id.clone().or_else(|| current.clone());
                if let Some(t) = turn.as_ref().and_then(|id| turns.get_mut(id)) {
                    t.status = Some(result.status);
                    t.worked_ms = result.duration_ms.map(|d| d - result.waited_ms.unwrap_or(0.0).clamp(0.0, d));
                    t.last = at.or(t.last);
                }
                if result.status == SessionEventResultStatus::Failed {
                    let text = result.error.clone().unwrap_or_else(|| "The turn failed".to_owned());
                    entries.push(Entry { key: key("error"), turn, body: Body::Work(Work::Error(text)) });
                }
                open = None;
            }
            SessionEvent::SessionEnd { turn_id, reason, saw_result, exit_code, at, .. } => {
                let turn = turn_id.clone().or_else(|| current.clone());
                let Some(t) = turn.as_ref().and_then(|id| turns.get_mut(id)) else {
                    if let Some(reason) = reason {
                        entries.push(Entry { key: key("error"), turn: None, body: Body::Work(Work::Error(reason.clone())) });
                    }
                    continue;
                };
                if t.settled() {
                    continue;
                }
                t.ended = true;
                t.last = at.or(t.last);
                let error = reason.clone().unwrap_or_else(|| match (saw_result, exit_code) {
                    (true, _) => "session exited without a result".to_owned(),
                    (_, Some(code)) => format!("session exited without a result (exit code {code})"),
                    _ => "session exited without a result (exit code unknown)".to_owned(),
                });
                entries.push(Entry { key: key("error"), turn, body: Body::Work(Work::Error(error)) });
                open = None;
            }
            SessionEvent::SessionNotice { text, turn_id, .. } => {
                let turn = turn_id.clone().or_else(|| current.clone());
                entries.push(Entry { key: key("notice"), turn, body: Body::Work(Work::Notice { text: text.clone(), stands_alone: true }) });
            }
            SessionEvent::SessionCompacted { turn_id, .. } => {
                let turn = turn_id.clone().or_else(|| current.clone());
                let text = "Compacted the conversation".to_owned();
                entries.push(Entry { key: key("compacted"), turn, body: Body::Work(Work::Notice { text, stands_alone: true }) });
            }
            _ => {}
        }
    }
    (entries, turns)
}

/// fmtDuration's short style in packages/protocol/src/words/units.ts.
fn duration(ms: f64) -> String {
    if !ms.is_finite() || ms < 0.0 {
        return "0ms".to_owned();
    }
    if ms < 1_000.0 {
        return format!("{}ms", ms.round().max(1.0));
    }
    if ms < 10_000.0 {
        let tenths = (ms / 100.0).round() / 10.0;
        return if tenths >= 10.0 { "10s".to_owned() } else { format!("{tenths:.1}s") };
    }
    if ms < 60_000.0 {
        return format!("{}s", (ms / 1_000.0).round());
    }
    let total = (ms / 1_000.0).round() as u64;
    let (minutes, seconds) = (total / 60, total % 60);
    if seconds == 0 {
        format!("{minutes}m")
    } else {
        format!("{minutes}m {seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn shown(rows: &[Row]) -> Vec<(RowKind, &str)> {
        rows.iter().map(|r| (r.kind, r.text.as_str())).collect()
    }

    #[test]
    fn a_settled_turn_keeps_the_message_one_fold_and_the_last_reply() {
        let rows = rows(&turn(true), &HashSet::new());
        assert_eq!(shown(&rows), [(RowKind::Person, "fix it"), (RowKind::Fold, "Worked for 12s"), (RowKind::Agent, "Done here.")]);
    }

    #[test]
    fn an_opened_turn_shows_its_calls_as_one_summary_line() {
        let rows = rows(&turn(true), &HashSet::from(["u1".to_owned()]));
        assert_eq!(
            shown(&rows),
            [
                (RowKind::Person, "fix it"),
                (RowKind::Agent, "Looking."),
                (RowKind::Calls, "Ran 2 commands and read 1 file"),
                (RowKind::Agent, "Done here."),
            ]
        );
    }

    #[test]
    fn a_running_turn_shows_everything_so_far() {
        let rows = rows(&turn(false), &HashSet::new());
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[2].kind, RowKind::Calls);
    }

    #[test]
    fn keys_hold_across_a_new_event() {
        let before = rows(&turn(false), &HashSet::new());
        let mut events = turn(false);
        events.push(base("session.delta", 9, json!({"kind": "text", "text": " More.", "messageId": "m2"})));
        let after = rows(&events, &HashSet::new());
        assert_eq!(before.iter().map(|r| &r.key).collect::<Vec<_>>(), after.iter().map(|r| &r.key).collect::<Vec<_>>());
        assert_eq!(after[3].text, "Done here. More.");
    }

    #[test]
    fn durations_read_as_the_web_app_writes_them() {
        assert_eq!(duration(400.0), "400ms");
        assert_eq!(duration(4_250.0), "4.3s");
        assert_eq!(duration(12_400.0), "12s");
        assert_eq!(duration(125_000.0), "2m 5s");
    }
}
