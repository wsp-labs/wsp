// SPDX-License-Identifier: AGPL-3.0-only
// What a tool line reads and draws: the helpers of adapt/timeline-rows.ts, MessagesTimeline.logic.ts and timeline/workEntry.tsx.

use std::collections::HashSet;

use crate::words::{tool_row, ToolItem as Item, ToolRequest as Request};

use super::{derive::Opened, facts::command_line, Body, Call, Entry, Glyph, Row, RowKind, Source, Status, Tone, Work};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Action {
    Read,
    Edit,
    Command,
    Search,
    CodeSearch,
    Other,
    Update,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Kind {
    Action(Action),
    Agent,
    ToneTool,
    Mixed,
}

pub(super) fn kind_glyph(kind: Kind) -> Glyph {
    match kind {
        Kind::Action(Action::Read) => Glyph::Eye,
        Kind::Action(Action::Edit) => Glyph::SquarePen,
        Kind::Action(Action::Command) => Glyph::Terminal,
        Kind::Action(Action::Search) => Glyph::Globe,
        Kind::Action(Action::CodeSearch) => Glyph::Search,
        Kind::Action(Action::Other) => Glyph::Wrench,
        Kind::Action(Action::Update) | Kind::Mixed => Glyph::Hammer,
        Kind::Agent => Glyph::Bot,
        Kind::ToneTool => Glyph::Zap,
    }
}

fn tone_glyph(tone: Tone) -> Glyph {
    match tone {
        Tone::Thinking => Glyph::Brain,
        Tone::Tool => Glyph::Zap,
        Tone::Notice => Glyph::Info,
        Tone::Error => Glyph::CircleAlert,
        Tone::Compaction => Glyph::Minimize,
    }
}

fn non_blank(text: &Option<String>) -> Option<&str> {
    text.as_deref().filter(|t| !t.trim().is_empty())
}

/// An error, a compaction and the host's notice stand alone: no group folds them and no settled turn hides them.
pub(super) fn stands_alone(w: &Work) -> bool {
    w.tone == Tone::Error || w.tone == Tone::Compaction || w.source == Source::Notice
}

pub(super) fn tool_like(w: &Work) -> bool {
    matches!(w.tone, Tone::Tool | Tone::Thinking | Tone::Error)
        || non_blank(&w.command).is_some()
        || w.request.is_some()
        || w.item.is_some()
}

pub(super) fn failure(w: &Work) -> bool {
    w.tone == Tone::Error || w.status == Some(Status::Failed)
}

pub(super) fn success(w: &Work) -> bool {
    tool_like(w) && !failure(w) && w.tone != Tone::Thinking && !matches!(w.status, Some(Status::Running | Status::Stopped))
}

fn neutral(w: &Work) -> bool {
    if w.tone == Tone::Thinking && non_blank(&w.detail).is_some() {
        return false;
    }
    tool_like(w) && !failure(w) && !success(w)
}

pub(super) fn visible_in_group(w: &Work, live: bool) -> bool {
    (live && w.status == Some(Status::Running)) || !neutral(w)
}

pub(super) fn active_activity(w: &Work) -> bool {
    w.status == Some(Status::Running) || (w.status.is_none() && tool_like(w))
}

fn code_search(tool: Option<&str>) -> bool {
    tool.and_then(tool_row).is_some_and(|row| row.code_search)
}

fn action(w: &Work) -> Action {
    if w.request == Some(Request::FileRead) {
        Action::Read
    } else if w.request == Some(Request::FileChange) || w.item == Some(Item::FileChange) || !w.changed.is_empty() {
        Action::Edit
    } else if w.request == Some(Request::Command) || w.item == Some(Item::Command) || w.command.as_deref().is_some_and(|c| !c.is_empty()) {
        Action::Command
    } else if code_search(w.tool.as_deref()) {
        Action::CodeSearch
    } else if w.item == Some(Item::WebSearch) {
        Action::Search
    } else if tool_like(w) {
        Action::Other
    } else {
        Action::Update
    }
}

fn work_kind(w: &Work) -> Option<Kind> {
    match (action(w), w.item) {
        (Action::Other, Some(Item::Mcp)) => Some(Kind::Action(Action::Other)),
        (Action::Other, Some(Item::Agent)) => Some(Kind::Agent),
        (Action::Other, _) => None,
        (action, _) => Some(Kind::Action(action)),
    }
}

fn entry_glyph(w: &Work) -> Glyph {
    tool_like(w).then(|| work_kind(w)).flatten().map_or(tone_glyph(w.tone), kind_glyph)
}

/// toolGroupSummaryKind: the one kind a group's calls share, else Mixed.
pub(super) fn group_kind(group: &[&Work]) -> Kind {
    let tools: Vec<&Work> = group.iter().copied().filter(|w| w.tone != Tone::Thinking).collect();
    let counted = if tools.is_empty() { group } else { &tools[..] };
    let actions: HashSet<Action> = counted.iter().map(|w| action(w)).collect();
    match actions.into_iter().collect::<Vec<_>>().as_slice() {
        [Action::Other] => {}
        [one] => return Kind::Action(*one),
        _ => return Kind::Mixed,
    }
    let tone_kind = |tone| match tone {
        Tone::Thinking => Kind::Agent,
        Tone::Tool => Kind::ToneTool,
        _ => Kind::Action(Action::Other),
    };
    let kinds: HashSet<Kind> = group.iter().map(|w| work_kind(w).unwrap_or(tone_kind(w.tone))).collect();
    if kinds.len() == 1 {
        kinds.into_iter().next().unwrap_or(Kind::Mixed)
    } else {
        Kind::Mixed
    }
}

/// summarizeToolGroup: each kind of call counted, in the order first seen.
pub(super) fn summary(group: &[&Work]) -> String {
    let tools: Vec<&Work> = group.iter().copied().filter(|w| w.tone != Tone::Thinking).collect();
    if tools.is_empty() {
        return "Thinking".to_owned();
    }
    let mut order: Vec<(Action, Vec<&Work>)> = Vec::new();
    for w in tools {
        let action = action(w);
        match order.iter_mut().find(|(a, _)| *a == action) {
            Some((_, calls)) => calls.push(w),
            None => order.push((action, vec![w])),
        }
    }
    let labels: Vec<String> = order
        .iter()
        .enumerate()
        .map(|(i, (action, calls))| {
            let n = if *action == Action::Edit {
                let files: HashSet<&str> = calls.iter().flat_map(|w| w.changed.iter().map(String::as_str)).collect();
                files.len() + calls.iter().filter(|w| w.changed.is_empty()).count()
            } else {
                calls.len()
            };
            let label = action_label(*action, n);
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
        [] => String::new(),
    }
}

fn action_label(action: Action, n: usize) -> String {
    let (verb, one, many) = match action {
        Action::Read => ("Read", "file", "files"),
        Action::Edit => ("Changed", "file", "files"),
        Action::Command => ("Ran", "command", "commands"),
        Action::Search => ("Searched the web", "time", "times"),
        Action::CodeSearch => ("Searched code", "time", "times"),
        Action::Other => ("Used", "tool", "tools"),
        Action::Update => ("Received", "update", "updates"),
    };
    format!("{verb} {n} {}", if n == 1 { one } else { many })
}

pub(super) struct Label {
    verb: Option<String>,
    pub(super) text: String,
    mono: bool,
}

impl Label {
    fn prose(text: impl Into<String>) -> Self {
        Label { verb: None, text: text.into(), mono: false }
    }

    fn line(&self) -> String {
        match &self.verb {
            Some(verb) => format!("{verb} {}", self.text),
            None => self.text.clone(),
        }
    }
}

/// workEntryDisplayLabel.
pub(super) fn display_label(w: &Work) -> Label {
    if let Some(preview) = w.preview.as_deref().filter(|p| !p.is_empty()) {
        return Label::prose(preview);
    }
    if let Some(description) = w.description.as_deref().filter(|d| !d.is_empty()) {
        return Label::prose(description);
    }
    if let Some(command) = non_blank(&w.command) {
        return Label { verb: None, text: command_line(command), mono: true };
    }
    if let Some(detail) = w.detail.as_deref().filter(|d| !d.is_empty()) {
        return match w.tool.as_deref().filter(|t| w.item == Some(Item::Mcp) && !t.is_empty()) {
            Some(tool) => Label { verb: Some(tool.to_owned()), text: detail.to_owned(), mono: false },
            None => Label::prose(detail),
        };
    }
    if let Some(first) = w.changed.first() {
        return Label::prose(if w.changed.len() == 1 { first.clone() } else { format!("{first} +{} more", w.changed.len() - 1) });
    }
    let named = w.tool.as_deref().filter(|t| !t.is_empty()).unwrap_or(&w.label);
    let heading = strip_complete(named);
    let mut chars = heading.chars();
    Label::prose(chars.next().map_or(String::new(), |c| c.to_uppercase().collect::<String>() + chars.as_str()))
}

/// normalizeCompactToolLabel: a trailing "complete" or "completed" says nothing.
fn strip_complete(label: &str) -> &str {
    let trimmed = label.trim_end();
    for word in ["completed", "complete"] {
        let Some(cut) = trimmed.len().checked_sub(word.len()) else { continue };
        if trimmed.is_char_boundary(cut) && trimmed[cut..].eq_ignore_ascii_case(word) && trimmed[..cut].ends_with(char::is_whitespace) {
            return trimmed[..cut].trim();
        }
    }
    label.trim()
}

/// liveWorkEntryLabel: a command reads its state ahead of its first line.
fn live_label(w: &Work, active: bool) -> Label {
    let Some(command) = non_blank(&w.command) else { return display_label(w) };
    if let Some(description) = w.description.as_deref().filter(|d| !d.is_empty()) {
        return Label::prose(description);
    }
    let verb = match w.status.unwrap_or(if active { Status::Running } else { Status::Completed }) {
        Status::Running => "Running",
        Status::Failed => "Failed",
        Status::Stopped => "Stopped",
        Status::Completed => "Ran",
    };
    Label { verb: Some(verb.to_owned()), text: command_line(command), mono: true }
}

/// workEntryBody, shown only where workEntryCanExpand says the row opens.
fn work_body(w: &Work, label: &str) -> Option<String> {
    let detail = w.detail.as_deref().map(str::trim).filter(|d| !d.is_empty());
    if detail.is_none_or(|d| d == label) && w.changed.is_empty() {
        return None;
    }
    let mut blocks: Vec<String> = detail.map(str::to_owned).into_iter().collect();
    if !w.changed.is_empty() {
        blocks.push(w.changed.join("\n"));
    }
    Some(blocks.join("\n\n"))
}

/// outputText in timeline/callFacts.ts: leading blank lines and trailing space go.
fn output_text(output: &str) -> String {
    let lead = output.len() - output.trim_start().len();
    let start = output[..lead].rfind('\n').map_or(0, |at| at + 1);
    output[start..].trim_end().to_owned()
}

fn call(e: &Entry, w: &Work, opened: &HashSet<String>) -> Call {
    let failed = failure(w);
    let mut call = Call {
        key: e.key.clone(),
        glyph: if failed { Glyph::CircleAlert } else { entry_glyph(w) },
        tone: w.tone,
        verb: None,
        label: String::new(),
        mono: false,
        command: None,
        running: w.status == Some(Status::Running),
        failed,
        exit_code: w.exit_code,
        duration_ms: w.duration_ms,
        body: None,
        patch: None,
        files: Vec::new(),
        cut: w.cut,
        bytes: w.bytes,
        since: (w.status == Some(Status::Running)).then_some(e.at).flatten(),
        open: opened.contains(&e.key),
    };
    if let Some(command) = non_blank(&w.command) {
        let exited = w.exit_code.is_some_and(|code| code != 0);
        call.glyph = if !exited && failed { Glyph::CircleAlert } else { Glyph::Terminal };
        call.label = command_line(command);
        call.mono = true;
        call.command = Some(command.to_owned());
        call.body = w.output.as_deref().map(output_text);
    } else if let Some(patch) = &w.patch {
        call.glyph = if patch.created { Glyph::FilePlus } else { Glyph::SquarePen };
        call.verb = Some(patch.verb.to_owned());
        call.label = patch.what.clone();
        call.mono = patch.one;
        call.patch = Some(patch.text.clone());
        call.files = patch.files.clone();
    } else {
        let label = display_label(w);
        call.body = work_body(w, &label.line());
        call.verb = label.verb;
        call.label = label.text;
        call.mono = label.mono;
    }
    call
}

pub(super) fn group_id(e: &Entry, w: &Work) -> String {
    match &w.call {
        Some(call) => format!("group:tool:{}:{call}", e.turn.as_deref().unwrap_or("no-turn")),
        None => format!("group:{}", e.key),
    }
}

/// A work entry standing on its own line: a severe error in the destructive ink, anything else in its tone, opening
/// to its body where it has one (PlainWorkEntryRow).
pub(super) fn work_row(key: &str, e: &Entry, w: &Work, opened: &Opened) -> Row {
    let failed = failure(w);
    let text = display_label(w).line();
    let kind = if failed && (w.source == Source::Error || !tool_like(w)) {
        RowKind::Error
    } else {
        let body = work_body(w, &text);
        let open = body.is_some() && opened.ids.contains(key);
        RowKind::Work { tone: w.tone, glyph: if failed { Glyph::CircleAlert } else { entry_glyph(w) }, body, open }
    };
    Row { key: key.to_owned(), kind, text, turn: e.turn.clone() }
}

pub(super) struct Live<'a> {
    pub(super) key: String,
    pub(super) group: String,
    pub(super) entry: &'a Entry,
    pub(super) work: &'a Work,
    pub(super) calls: Vec<usize>,
    pub(super) active: bool,
    pub(super) open: bool,
}

impl Live<'_> {
    pub(super) fn row(&self) -> Row {
        let label = live_label(self.work, self.active);
        let glyph = if failure(self.work) { Glyph::CircleAlert } else { entry_glyph(self.work) };
        let since = (self.work.command.is_some() && self.work.status == Some(Status::Running)).then_some(self.entry.at).flatten();
        Row {
            key: self.key.clone(),
            kind: RowKind::Live {
                group: self.group.clone(),
                glyph,
                verb: label.verb,
                mono: label.mono,
                since,
                open: self.open,
                active: self.active,
            },
            text: label.text,
            turn: self.entry.turn.clone(),
        }
    }
}

pub(super) fn work_of(e: &Entry) -> Option<&Work> {
    match &e.body {
        Body::Work(w) => Some(w.as_ref()),
        _ => None,
    }
}

pub(super) fn calls_row(group: &str, members: &[usize], entries: &[Entry], opened: &HashSet<String>) -> Row {
    let calls = members.iter().filter_map(|&ix| work_of(&entries[ix]).map(|w| call(&entries[ix], w, opened))).collect();
    let turn = members.first().and_then(|&ix| entries[ix].turn.clone());
    Row { key: format!("{group}:calls"), kind: RowKind::Calls { group: group.to_owned(), calls }, text: String::new(), turn }
}

pub(super) fn elapsed(start: Option<f64>, end: Option<f64>) -> Option<f64> {
    Some((end? - start?).max(0.0))
}
