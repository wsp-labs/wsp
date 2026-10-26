// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript index of packages/runtime/src/types/internal.ts: what the host keeps of a transcript it does not hold,
//! its fold and its bytes. Re-exported by `internal`. Spec: spec/internal-index.md.

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wsp_wire::types::js_number::JsNumber;

use wsp_wire::views::session::{SessionView, SubagentState, SubagentView};
use wsp_wire::views::session_events::{copied_from_of, SessionEvent, TurnStatus};
use wsp_words::js::{js_collapse_whitespace, js_trim, utf16};
use wsp_words::words::turn::{listed_failure, listed_last_line};

use super::pending::BlobMark;

// --- the transcript index ------------------------------------------------------------------------------------------

/// One thread's words as search reads them. Every length here (`size`, `room`, a line's) counts UTF-16 code units,
/// as JavaScript's `length` does. Keys serialize in this order; `open` is written while it names a message, and left
/// out where it is none, as `JSON.stringify` drops `undefined`. internal.ts:236-251.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ThreadWords {
    pub lines: Vec<String>,
    /// `<turnId or sessionId>:<messageId or "">` of the reply still being written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<String>,
    pub last: JsNumber,
    /// Absent on an index written before WORDS_HELD; `index_read` fills it.
    #[serde(default)]
    pub size: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened: Option<JsNumber>,
    /// `true` where present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<usize>,
}

/// The first line a thread keeps when its lines go: its first 200 UTF-16 units. internal.ts:253.
pub const FIRST_KEPT: usize = 200;

/// A line cut to FIRST_KEPT UTF-16 units, an owned copy. internal.ts:256.
pub fn first_of(line: &str) -> String {
    utf16::cut(line, FIRST_KEPT)
}

/// An event's keys read by name, as the TS reads `e.threadId` and the like.
struct Fields(Value);

impl Fields {
    fn of(e: &SessionEvent) -> Fields {
        Fields(serde_json::to_value(e).unwrap_or(Value::Null))
    }
    fn str(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.as_str()
    }
    fn num(&self, key: &str) -> Option<f64> {
        self.0.get(key)?.as_f64()
    }
    fn flag(&self, key: &str) -> Option<bool> {
        self.0.get(key)?.as_bool()
    }
    fn has(&self, key: &str) -> bool {
        self.0.get(key).is_some()
    }
}

fn push_line(words: &mut ThreadWords, line: &str) {
    words.lines.push(line.to_string());
    words.size += utf16::len(line);
    if words.first.is_none() {
        words.first = Some(first_of(line));
    }
}

/// A subagent as the index holds it: what a listing answers, the turn whose end stops it, and its newest start row's
/// `at`. internal.ts:283.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub view: SubagentView,
    pub turn_id: Option<String>,
    pub start_row: Option<JsNumber>,
    /// Whether `usage` was first set by the row that ended the child, which writes it after the end's own keys; it takes
    /// no part in equality.
    pub usage_late: LateUsage,
}

/// Whether a child's `usage` key came after its end's keys.
#[derive(Clone, Copy, Debug, Default)]
pub struct LateUsage(pub bool);

impl PartialEq for LateUsage {
    fn eq(&self, _: &LateUsage) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChildWire {
    #[serde(flatten)]
    view: SubagentView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_row: Option<JsNumber>,
}

/// The keys of a child as the TS object holds them: the running row's, then the end's, and `usage` after them when the
/// end was the first to carry it.
const CHILD_KEYS: [&str; 14] = [
    "id",
    "title",
    "state",
    "parentToolUseId",
    "depth",
    "model",
    "asked",
    "usage",
    "startedAt",
    "turnId",
    "startRow",
    "endedAt",
    "lastLine",
    "failure",
];

impl Serialize for Child {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let wire = ChildWire { view: self.view.clone(), turn_id: self.turn_id.clone(), start_row: self.start_row };
        let serde_json::Value::Object(mut held) = serde_json::to_value(&wire).map_err(S::Error::custom)? else {
            return Err(S::Error::custom("a child is an object"));
        };
        let mut out = serde_json::Map::new();
        for key in CHILD_KEYS.iter().filter(|k| **k != "usage" || !self.usage_late.0) {
            if let Some(value) = held.remove(*key) {
                out.insert((*key).to_string(), value);
            }
        }
        if let Some(usage) = held.remove("usage") {
            out.insert("usage".to_string(), usage);
        }
        out.extend(held);
        serde_json::Value::Object(out).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Child {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;
        let at = |key: &str| value.as_object().and_then(|o| o.keys().position(|k| k == key));
        let late = matches!((at("usage"), at("startedAt")), (Some(usage), Some(started)) if usage > started);
        let wire: ChildWire = serde_json::from_value(value).map_err(D::Error::custom)?;
        Ok(Child { view: wire.view, turn_id: wire.turn_id, start_row: wire.start_row, usage_late: LateUsage(late) })
    }
}

/// `"started" | "steered"`. internal.ts:289.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TakenOutcome {
    Started,
    Steered,
}

/// internal.ts:285-290.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Taken {
    pub session_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub outcome: TakenOutcome,
}

/// internal.ts:293.
pub fn transcript_unread_line(workspace_id: &str, why: &str) -> String {
    format!("the transcript of {workspace_id} does not read ({why})")
}

/// cwd, permissionMode, model, effort, in that order. internal.ts:296.
pub const SESSION_FACTS: [&str; 4] = ["cwd", "permissionMode", "model", "effort"];

/// What a resumed session's turns carry, read off its newest start that named each. Written in the order each key was
/// first set, as the TS object holds them. internal.ts:297.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionFacts {
    pub cwd: Option<String>,
    pub permission_mode: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    /// The keys in the order they were first set; it takes no part in equality.
    pub order: FactOrder,
}

/// The order a session's facts were first set in.
#[derive(Clone, Debug, Default)]
pub struct FactOrder(pub Vec<&'static str>);

impl PartialEq for FactOrder {
    fn eq(&self, _: &FactOrder) -> bool {
        true
    }
}

impl SessionFacts {
    fn set(&mut self, key: &'static str, value: &str) {
        let slot = match key {
            "cwd" => &mut self.cwd,
            "permissionMode" => &mut self.permission_mode,
            "model" => &mut self.model,
            _ => &mut self.effort,
        };
        *slot = Some(value.to_string());
        if !self.order.0.contains(&key) {
            self.order.0.push(key);
        }
    }

    fn value(&self, key: &str) -> Option<&String> {
        match key {
            "cwd" => self.cwd.as_ref(),
            "permissionMode" => self.permission_mode.as_ref(),
            "model" => self.model.as_ref(),
            _ => self.effort.as_ref(),
        }
    }
}

impl Serialize for SessionFacts {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let keys: Vec<&str> = self.order.0.iter().copied().chain(SESSION_FACTS).filter(|k| self.value(k).is_some()).collect();
        let mut seen: Vec<&str> = Vec::new();
        let mut map = serializer.serialize_map(None)?;
        for key in keys {
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            map.serialize_entry(key, &self.value(key))?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for SessionFacts {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let map = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
        let mut facts = SessionFacts::default();
        for (key, value) in &map {
            let Some(known) = SESSION_FACTS.into_iter().find(|k| k == key) else { continue };
            let text = value.as_str().ok_or_else(|| D::Error::custom(format!("{key} is not a string")))?;
            facts.set(known, text);
        }
        Ok(facts)
    }
}

/// What the host keeps of a transcript it does not hold. Every map iterates in insertion order (chapter 03, rule
/// 3.11): a delete is `shift_remove`. internal.ts:261-279.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TranscriptIndex {
    pub words: IndexMap<String, ThreadWords>,
    pub starts: IndexMap<String, String>,
    pub cut: IndexMap<String, bool>,
    pub facts: IndexMap<String, SessionFacts>,
    pub taken: IndexMap<String, Taken>,
    pub children: IndexMap<String, IndexMap<String, Child>>,
    pub trimmed: IndexSet<String>,
    /// The newest position issued; never moves back.
    pub pos: u64,
}

/// internal.ts:299.
pub fn empty_index() -> TranscriptIndex {
    TranscriptIndex::default()
}

/// One session.subagent row into a thread's children. internal.ts:305-332. The event must be a session.subagent;
/// any other is ignored.
pub fn fold_child(index: &mut TranscriptIndex, e: &SessionEvent) {
    let SessionEvent::SessionSubagentEvent(e) = e else { return };
    let Some(thread) = e.thread_id.as_deref() else { return };
    let child = index.children.get(thread).and_then(|held| held.get(&e.task)).cloned();
    if e.state == SubagentState::Running {
        let view = SubagentView {
            id: e.task.clone(),
            title: e.title.clone().or_else(|| child.as_ref().map(|c| c.view.title.clone())).unwrap_or_else(|| e.task.clone()),
            state: SubagentState::Running,
            parent_tool_use_id: e.parent_tool_use_id.clone().or_else(|| child.as_ref().and_then(|c| c.view.parent_tool_use_id.clone())),
            depth: e.depth.or_else(|| child.as_ref().and_then(|c| c.view.depth)),
            model: e.model.clone().or_else(|| child.as_ref().and_then(|c| c.view.model.clone())),
            asked: e.asked.clone().or_else(|| child.as_ref().and_then(|c| c.view.asked.clone())),
            started_at: child.as_ref().map(|c| c.view.started_at).or(e.at).unwrap_or(JsNumber(0.0)),
            ended_at: None,
            last_line: None,
            failure: None,
            usage: e.usage.clone().or_else(|| child.as_ref().and_then(|c| c.view.usage.clone())),
        };
        let row = Child { view, turn_id: e.turn_id.clone(), start_row: e.at, usage_late: LateUsage(false) };
        index.children.entry(thread.to_string()).or_default().insert(e.task.clone(), row);
    } else if let Some(child) = index.children.get_mut(thread).and_then(|held| held.get_mut(&e.task)) {
        child.view.state = e.state;
        child.view.ended_at = Some(e.at.unwrap_or(child.view.started_at));
        if e.state == SubagentState::Done {
            if let Some(last) = listed_last_line(e.summary.as_deref()) {
                child.view.last_line = Some(last);
            }
        }
        if e.state == SubagentState::Failed && e.summary.is_some() {
            child.view.failure = Some(listed_failure(e.summary.as_deref()));
        }
        if e.usage.is_some() {
            if child.view.usage.is_none() {
                child.usage_late = LateUsage(true);
            }
            child.view.usage = e.usage.clone();
        }
    }
}

/// The row with its turns' words left out: no lastLine, no failure, and each subagent without asked, lastLine and
/// failure. internal.ts:336-339.
pub fn named_only(view: &SessionView) -> SessionView {
    let mut out = view.clone();
    out.last_line = None;
    out.failure = None;
    if let Some(subagents) = out.subagents.as_mut() {
        for child in subagents.iter_mut() {
            child.asked = None;
            child.last_line = None;
            child.failure = None;
        }
    }
    out
}

/// What a turn's end left, onto its row: each line set where given and removed where not. internal.ts:342-348.
pub fn write_lines(view: &mut SessionView, last_line: Option<String>, failure: Option<String>) {
    view.last_line = last_line;
    view.failure = failure;
}

/// A turn's end stops every running child of that turn. internal.ts:351-362.
pub fn end_children(index: &mut TranscriptIndex, thread_id: &str, turn_id: Option<&str>, at: Option<f64>) {
    let Some(held) = index.children.get_mut(thread_id) else { return };
    for child in held.values_mut() {
        if child.view.state != SubagentState::Running || child.turn_id.as_deref() != turn_id {
            continue;
        }
        child.view.state = SubagentState::Stopped;
        child.view.ended_at = Some(at.map_or(child.view.started_at, JsNumber));
        if let Some(usage) = child.view.usage.as_mut() {
            usage.duration_ms = None;
        }
    }
}

/// A row the ring dropped: a child whose start it was leaves the index. internal.ts:365-371.
pub fn forget_child(index: &mut TranscriptIndex, e: &SessionEvent) {
    let SessionEvent::SessionSubagentEvent(e) = e else { return };
    if e.state != SubagentState::Running {
        return;
    }
    let Some(thread) = e.thread_id.as_deref() else { return };
    let Some(held) = index.children.get_mut(thread) else { return };
    if held.get(&e.task).and_then(|c| c.start_row) != e.at {
        return;
    }
    held.shift_remove(&e.task);
    if held.is_empty() {
        index.children.shift_remove(thread);
    }
}

/// One event into an index. internal.ts:376-403.
pub fn fold_event(index: &mut TranscriptIndex, e: &SessionEvent) {
    let f = Fields::of(e);
    if let Some(pos) = f.num("pos") {
        if pos > index.pos as f64 {
            index.pos = pos as u64;
        }
    }
    if copied_from_of(e).is_some() {
        match f.str("type") {
            Some("session.subagent") => fold_child(index, e),
            Some("session.end") => {
                if let Some(thread) = f.str("threadId") {
                    end_children(index, thread, f.str("turnId"), f.num("at"));
                }
            }
            _ => {}
        }
        fold_words(index, e);
        return;
    }
    let ty = f.str("type");
    let (thread, turn) = (f.str("threadId"), f.str("turnId"));
    if let (Some("session.start" | "session.steer"), Some(request), Some(thread), Some(turn)) = (ty, f.str("requestId"), thread, turn) {
        let outcome = if ty == Some("session.start") { TakenOutcome::Started } else { TakenOutcome::Steered };
        let taken = Taken {
            session_id: f.str("sessionId").unwrap_or_default().to_string(),
            thread_id: thread.to_string(),
            turn_id: turn.to_string(),
            outcome,
        };
        index.taken.insert(request.to_string(), taken);
    }
    if ty == Some("session.end") && f.flag("promptless") == Some(true) {
        if let Some(turn) = turn {
            index.taken.retain(|_, t| !(t.turn_id == turn && t.outcome == TakenOutcome::Started));
        }
    }
    match ty {
        Some("session.start") => {
            let session = f.str("sessionId").unwrap_or_default().to_string();
            if let Some(thread) = thread {
                index.starts.insert(thread.to_string(), session.clone());
            }
            let facts = index.facts.entry(session).or_default();
            if let Some(v) = f.str("cwd") {
                facts.set("cwd", v);
            }
            if let Some(v) = f.str("permissionMode") {
                facts.set("permissionMode", v);
            }
            if let Some(v) = f.str("model") {
                facts.set("model", v);
            }
            if let Some(v) = f.str("effort") {
                facts.set("effort", v);
            }
        }
        Some("session.end") if thread.is_some() && f.flag("unstarted") != Some(true) => {
            if let Some(thread) = thread {
                let cut = f.0["exitCode"].is_null() && !f.flag("sawResult").unwrap_or(false);
                index.cut.insert(thread.to_string(), cut);
                end_children(index, thread, turn, f.num("at"));
            }
        }
        Some("session.subagent") => fold_child(index, e),
        _ => {}
    }
    fold_words(index, e);
}

/// A thread's messages and its own agent's replies into the index's words. internal.ts:407-433.
pub fn fold_words(index: &mut TranscriptIndex, e: &SessionEvent) {
    let f = Fields::of(e);
    let Some(thread) = f.str("threadId") else { return };
    let held = index.words.entry(thread.to_string()).or_default();
    held.last = JsNumber(held.last.0.max(f.num("at").unwrap_or(0.0)));
    if held.cold == Some(true) {
        return;
    }
    match f.str("type") {
        Some("session.start") if f.has("prompt") => {
            push_line(held, f.str("prompt").unwrap_or_default());
            held.open = None;
        }
        Some("session.steer") => {
            push_line(held, f.str("prompt").unwrap_or_default());
            held.open = None;
        }
        Some("session.earlier") if matches!(f.str("who"), Some("person" | "agent")) => {
            push_line(held, f.str("text").unwrap_or_default());
            held.open = None;
        }
        Some("session.delta") if f.str("kind") == Some("text") && !f.has("parentToolUseId") => {
            let message =
                format!("{}:{}", f.str("turnId").or(f.str("sessionId")).unwrap_or_default(), f.str("messageId").unwrap_or_default());
            let text = f.str("text").unwrap_or_default();
            if held.open.as_deref() == Some(message.as_str()) {
                if let Some(last) = held.lines.last_mut() {
                    last.push_str(text);
                }
                held.size += utf16::len(text);
            } else {
                push_line(held, text);
            }
            held.open = Some(message);
        }
        _ => {}
    }
}

/// One thread's words read off its events. internal.ts:436-440.
pub fn thread_words_of(events: &[SessionEvent], thread_id: &str) -> ThreadWords {
    let mut index = empty_index();
    for e in events {
        if Fields::of(e).str("threadId") == Some(thread_id) {
            fold_words(&mut index, e);
        }
    }
    index.words.swap_remove(thread_id).unwrap_or_default()
}

/// A thread's words with its lines let go. internal.ts:443.
pub fn cold_words(w: &ThreadWords) -> ThreadWords {
    ThreadWords {
        lines: Vec::new(),
        open: None,
        last: w.last,
        size: w.size,
        first: w.first.clone(),
        opened: w.opened,
        cold: Some(true),
        room: None,
    }
}

/// A thread's words with its oldest lines let go, its newest kept while they fit in `room`. internal.ts:445-450.
pub fn newest_of(w: &ThreadWords, room: usize) -> ThreadWords {
    if w.cold != Some(true) && w.size <= room {
        return w.clone();
    }
    let mut from = w.lines.len();
    let mut kept = 0;
    while from > 0 && kept + utf16::len(&w.lines[from - 1]) <= room {
        from -= 1;
        kept += utf16::len(&w.lines[from]);
    }
    ThreadWords { lines: w.lines[from..].to_vec(), room: Some(room), ..cold_words(w) }
}

/// The UTF-16 units of words a thread holds now. internal.ts:452.
pub fn held_of(w: &ThreadWords) -> usize {
    if w.cold == Some(true) {
        w.lines.iter().map(|line| utf16::len(line)).sum()
    } else {
        w.size
    }
}

/// internal.ts:454-458.
pub fn index_of(events: &[SessionEvent]) -> TranscriptIndex {
    let mut index = empty_index();
    for e in events {
        fold_event(&mut index, e);
    }
    index
}

/// These threads' entries read again off their own events, in the order named; a session's facts stay. internal.ts:462-466.
pub fn reindex_threads<'a>(
    index: &mut TranscriptIndex,
    threads: impl IntoIterator<Item = &'a String> + Copy,
    events_of: &dyn Fn(&str) -> Vec<SessionEvent>,
) {
    for thread in threads {
        index.words.shift_remove(thread.as_str());
        index.starts.shift_remove(thread.as_str());
        index.cut.shift_remove(thread.as_str());
        index.children.shift_remove(thread.as_str());
    }
    index.taken.retain(|_, t| !threads.into_iter().any(|name| *name == t.thread_id));
    for thread in threads {
        for e in events_of(thread) {
            fold_event(index, &e);
        }
    }
}

/// How much of itself a turn has written. internal.ts:469-482.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TurnWritten {
    pub lines: u64,
    pub subagents: u64,
    pub compactions: u64,
    pub retries: u64,
    pub reply: Option<TurnStatus>,
    pub started: bool,
    pub changes: bool,
}

/// internal.ts:483-503.
pub fn turn_written(events: &[SessionEvent], turn_id: &str) -> TurnWritten {
    let fields: Vec<Fields> = events.iter().map(Fields::of).collect();
    let last_of = |ty: &str, lined: bool| {
        fields.iter().rev().find(|f| f.str("type") == Some(ty) && f.str("turnId") == Some(turn_id) && (!lined || f.has("line")))
    };
    let count = |ty: &str, lined: bool| last_of(ty, lined).and_then(|f| f.num("line")).unwrap_or(0.0) as u64;
    let lines = count("session.delta", false);
    let subagents = count("session.subagent", false);
    let compactions = count("session.compacted", false);
    let retries = count("session.retry", true);
    let reply = events.iter().rev().find_map(|e| match e {
        SessionEvent::SessionDoneEvent(d) if d.turn_id.as_deref() == Some(turn_id) => Some(d.result.status),
        _ => None,
    });
    let started =
        lines > 0 || subagents > 0 || compactions > 0 || retries > 0 || reply.is_some() || last_of("session.start", false).is_some();
    let changes = last_of("session.changes", false).is_some();
    TurnWritten { lines, subagents, compactions, retries, reply, started, changes }
}

/// The index as it is kept beside a transcript, the JSON of `{of, words, starts, cut, facts, taken, children,
/// trimmed, pos}` with each map as an array of `[key, value]` pairs; with `lines` false each thread's words are
/// `cold_words`. `of` is left out where None. internal.ts:510-511.
pub fn index_bytes(index: &TranscriptIndex, of: Option<BlobMark>, lines: bool) -> Vec<u8> {
    let mut out = serde_json::Map::new();
    if let Some(of) = of {
        out.insert("of".into(), serde_json::to_value(of).unwrap_or(Value::Null));
    }
    let words: Vec<Value> = index
        .words
        .iter()
        .map(|(thread, w)| {
            let w = if lines { w.clone() } else { cold_words(w) };
            json!([thread, w])
        })
        .collect();
    out.insert("words".into(), Value::Array(words));
    out.insert("starts".into(), json!(index.starts.iter().collect::<Vec<_>>()));
    out.insert("cut".into(), json!(index.cut.iter().collect::<Vec<_>>()));
    out.insert("facts".into(), json!(index.facts.iter().collect::<Vec<_>>()));
    out.insert("taken".into(), json!(index.taken.iter().collect::<Vec<_>>()));
    let children: Vec<Value> = index.children.iter().map(|(thread, held)| json!([thread, held.iter().collect::<Vec<_>>()])).collect();
    out.insert("children".into(), Value::Array(children));
    out.insert("trimmed".into(), json!(index.trimmed.iter().collect::<Vec<_>>()));
    out.insert("pos".into(), json!(index.pos));
    serde_json::to_vec(&Value::Object(out)).unwrap_or_default()
}

/// What `index_read` answers.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexRead {
    pub index: TranscriptIndex,
    pub of: Option<BlobMark>,
}

/// Drops the lone surrogate escapes a TS text holds, as `first_of` drops a split pair's half: a Rust string cannot hold one.
fn drop_lone_surrogates(b: &[u8]) -> Vec<u8> {
    let hex_at = |at: usize| b.get(at..at + 4).and_then(|h| u16::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
    let u_at = |at: usize| b.get(at..at + 2) == Some(&b"\\u"[..]);
    let mut out = Vec::with_capacity(b.len());
    let mut in_string = false;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' {
            in_string = !in_string;
        }
        if c != b'\\' || !in_string {
            out.push(c);
            i += 1;
        } else if !u_at(i) {
            let end = (i + 2).min(b.len());
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if matches!(hex_at(i + 2), Some(0xD800..=0xDBFF)) && u_at(i + 6) && matches!(hex_at(i + 8), Some(0xDC00..=0xDFFF)) {
            out.extend_from_slice(&b[i..i + 12]);
            i += 12;
        } else {
            if !matches!(hex_at(i + 2), Some(0xD800..=0xDFFF)) {
                out.extend_from_slice(&b[i..(i + 6).min(b.len())]);
            }
            i += 6;
        }
    }
    out
}

/// An index read back; None where it does not parse or lacks taken, children or pos. internal.ts:512-526.
pub fn index_read(bytes: &[u8]) -> Option<IndexRead> {
    #[derive(Deserialize)]
    struct Held {
        of: Option<BlobMark>,
        words: Vec<(String, Value)>,
        starts: Vec<(String, String)>,
        cut: Vec<(String, bool)>,
        facts: Vec<(String, SessionFacts)>,
        taken: Option<Vec<(String, Taken)>>,
        children: Option<Vec<(String, Vec<(String, Child)>)>>,
        trimmed: Option<Vec<String>>,
        pos: Option<u64>,
    }
    let held: Held = serde_json::from_slice(&drop_lone_surrogates(bytes)).ok()?;
    let (taken, children, pos) = (held.taken?, held.children?, held.pos?);
    let words = held
        .words
        .into_iter()
        .map(|(thread, mut w)| {
            let obj = w.as_object_mut()?;
            let lines: Vec<String> = obj.get("lines").and_then(|l| serde_json::from_value(l.clone()).ok())?;
            if obj.get("size").and_then(Value::as_u64).is_none() {
                obj.insert("size".into(), json!(lines.iter().map(|l| utf16::len(l)).sum::<usize>()));
            }
            if obj.get("first").and_then(Value::as_str).is_none() {
                if let Some(first) = lines.first() {
                    obj.insert("first".into(), json!(first_of(first)));
                }
            }
            Some((thread, serde_json::from_value::<ThreadWords>(w).ok()?))
        })
        .collect::<Option<IndexMap<String, ThreadWords>>>()?;
    let index = TranscriptIndex {
        words,
        starts: held.starts.into_iter().collect(),
        cut: held.cut.into_iter().collect(),
        facts: held.facts.into_iter().collect(),
        taken: taken.into_iter().collect(),
        children: children.into_iter().map(|(thread, kids)| (thread, kids.into_iter().collect())).collect(),
        trimmed: held.trimmed.unwrap_or_default().into_iter().collect(),
        pos,
    };
    Some(IndexRead { index, of: held.of })
}

/// One search hit. internal.ts:528.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub workspace_id: String,
    pub thread_id: String,
    pub snippet: String,
}

/// internal.ts:528.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchedHit {
    pub hit: SearchHit,
    pub last: JsNumber,
}

/// One folder's hits of `words` (lowercased) into `found`; answers how many threads the caller drives it could not
/// search. internal.ts:532-542.
pub fn search_index(
    workspace_id: &str,
    index: &TranscriptIndex,
    words: &str,
    drives: &dyn Fn(&str) -> bool,
    found: &mut Vec<SearchedHit>,
) -> usize {
    let mut unsearched = 0;
    for (thread_id, w) in &index.words {
        if !drives(thread_id) {
            continue;
        }
        if w.cold == Some(true) && w.lines.is_empty() && w.size > 0 {
            unsearched += 1;
        }
        let Some(text) = w.lines.iter().find(|line| line.to_lowercase().contains(words)) else { continue };
        let lower = text.to_lowercase();
        let at = lower.find(words).map_or(0, |byte| utf16::len(&lower[..byte]));
        let snippet = snippet_around(text, at, utf16::len(words));
        found.push(SearchedHit {
            hit: SearchHit { workspace_id: workspace_id.to_string(), thread_id: thread_id.clone(), snippet },
            last: w.last,
        });
    }
    unsearched
}

/// The words around a hit: 40 UTF-16 units before, 80 after, whitespace runs as one space, trimmed, "…" where the text
/// goes on. internal.ts:544-548.
pub fn snippet_around(text: &str, at: usize, length: usize) -> String {
    let len = utf16::len(text);
    let from = at.saturating_sub(40);
    let to = len.min(at + length + 80);
    let kept = js_trim(&js_collapse_whitespace(&utf16::slice(text, from as i64, Some(to as i64)))).to_string();
    let head = if from > 0 { "\u{2026}" } else { "" };
    let tail = if to < len { "\u{2026}" } else { "" };
    format!("{head}{kept}{tail}")
}

/// internal.ts:551.
pub const SESSION_INDEX_CAP: usize = 200;
/// internal.ts:554.
pub const TRANSCRIPT_FLUSH_MS: u64 = 250;
