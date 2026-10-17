// SPDX-License-Identifier: AGPL-3.0-only
//! A thread's events folded into the rows the transcript shows, the way the web app's timeline shows them by default
//! (apps/web/src/adapt/session.ts into adapt/timeline-rows.ts): a settled turn keeps the person's message, one
//! "Worked for" line and its last agent message, consecutive tool calls read as one summary line, and the running turn
//! reads as a working line, its replies and one live line. No GPUI here, so the fold is tested on its own.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

use wsp_host_client::protocol::{
    SessionEvent, SessionEventAttachmentsItem, SessionEventKind, SessionEventPatchItem, SessionEventResult, SessionEventResultStatus,
    SessionEventWho,
};

/// Which lucide glyph a tool line draws (TOOL_GROUP_GLYPHS and WORK_TONES in the web app's timeline/workEntry.tsx).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Glyph {
    Eye,
    SquarePen,
    FilePlus,
    Terminal,
    Globe,
    Search,
    Wrench,
    Hammer,
    Bot,
    Zap,
    Brain,
    Info,
    CircleAlert,
    Minimize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Thinking,
    Tool,
    Notice,
    Error,
    Compaction,
}

/// One call inside an expanded tool group (3.7 to 3.10 of the spec).
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub key: String,
    pub glyph: Glyph,
    pub tone: Tone,
    /// A verb drawn in the label ink before `label`, which then draws muted ("Edited" + "src/a.ts"); None draws `label` alone.
    pub verb: Option<String>,
    pub label: String,
    /// The label is a command or a path, drawn in the mono face.
    pub mono: bool,
    /// Some for a shell call: drawn as a command block (3.9).
    pub command: Option<String>,
    pub running: bool,
    pub failed: bool,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<f64>,
    /// What opening the row shows: a command's output, a tool's detail, a patch.
    pub body: Option<String>,
    /// A unified patch for an edit (3.10), drawn as a diff.
    pub patch: Option<String>,
    pub open: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterWord {
    Interrupted,
    Failed,
    LimitReached,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    /// What the person sent (row.text, markdown with single newlines as breaks); `at` is ms since the epoch.
    Person { at: Option<f64>, files: Vec<FileTile> },
    /// One agent message (row.text, markdown). `meta`: the settled turn's final reply, which keeps a 16 px foot and
    /// a hover-only meta line; `streaming`: the reply is still arriving.
    Agent { at: Option<f64>, meta: bool, streaming: bool },
    /// "Worked for 12s" (row.text) over a settled turn's hidden work; toggles `turn`.
    Fold { open: bool },
    /// "You stopped after 4.2s" (row.text) after an interrupted turn's last row.
    Stopped,
    /// A collapsed or expanded tool group's summary line (row.text); toggles `group`.
    Group { group: String, glyph: Glyph, open: bool },
    /// The running turn's trailing calls as the latest one's line (3.6): `verb` then row.text; `since` starts its timer.
    Live { group: String, glyph: Glyph, verb: Option<String>, mono: bool, since: Option<f64>, open: bool },
    /// The calls of the open group right above it.
    Calls { group: String, calls: Vec<Call> },
    /// A standalone work row (notice, compaction, a lone non-tool entry): row.text with its tone's glyph.
    Work { tone: Tone, glyph: Glyph },
    /// A failed turn's error (row.text), in the destructive ink, never folded.
    Error,
    /// "Working for 12s" at the start of the running turn; `since` is the turn's start in ms since the epoch.
    Working { since: Option<f64> },
    /// The running turn's last row while no call is live.
    Thinking,
    /// The latest turn ended without a reply: one mono word under the transcript.
    Footer { word: FooterWord },
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileTile {
    pub name: String,
    pub bytes: Option<u64>,
    pub image: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Stays the same for the same row across folds and live events, so the view keeps what it built for it.
    pub key: String,
    pub kind: RowKind,
    pub text: String,
    pub turn: Option<String>,
}

/// The rows changed by one fold step: rows[start..start+removed] were replaced by rows[start..start+inserted].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Splice {
    pub start: usize,
    pub removed: usize,
    pub inserted: usize,
}

/// A thread's events and the rows they fold into, folded again only from the running turn on when an event arrives.
#[derive(Default)]
pub struct Transcript {
    events: Vec<SessionEvent>,
    opened: HashSet<String>,
    rows: Vec<Row>,
    model: Model,
    /// Where the latest turn opened: a live event refolds from here; None folds the whole thread again.
    mark: Option<Mark>,
    /// Events folded plus entries drawn, so a test can tell a live event's cost from a load's.
    #[cfg(test)]
    cost: usize,
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, events: Vec<SessionEvent>) {
        self.events = events;
        self.full();
    }

    pub fn push(&mut self, event: SessionEvent) -> Option<Splice> {
        self.events.push(event);
        match self.mark.take() {
            Some(mark) => self.refold(mark),
            None => self.refold_all(),
        }
    }

    pub fn toggle(&mut self, id: &str) -> Option<Splice> {
        if !self.opened.remove(id) {
            self.opened.insert(id.to_owned());
        }
        self.refold_all()
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn events(&self) -> usize {
        self.events.len()
    }

    fn refold_all(&mut self) -> Option<Splice> {
        let old = std::mem::take(&mut self.rows);
        self.full();
        splice(&old, &self.rows)
    }

    fn full(&mut self) {
        self.model = Model::default();
        self.fold_from(0);
        let (rows, split) = self.derive(0, 0).expect("a fold from the first entry places every row");
        self.rows = rows;
        self.mark = self.model.latest.take().zip(split).map(|(mark, rows)| Mark { rows, ..mark });
    }

    /// Folds the events from the latest turn's opening on again, keeping every row drawn before it.
    fn refold(&mut self, mark: Mark) -> Option<Splice> {
        self.model.restore(&mark);
        self.fold_from(mark.event);
        if self.model.tangled {
            return self.refold_all();
        }
        let Some((tail, split)) = self.derive(mark.entries, mark.turns) else { return self.refold_all() };
        let at = mark.rows;
        let changed = splice(&self.rows[at..], &tail).map(|s| Splice { start: s.start + at, ..s });
        self.rows.truncate(at);
        self.rows.extend(tail);
        self.mark = self.model.latest.take().zip(split).map(|(mark, rows)| Mark { rows: at + rows, ..mark });
        changed
    }

    fn fold_from(&mut self, start: usize) {
        for (ix, event) in self.events.iter().enumerate().skip(start) {
            self.model.add(ix, event);
        }
        #[cfg(test)]
        {
            self.cost += self.events.len() - start;
        }
    }

    fn derive(&mut self, from: usize, turns_from: usize) -> Option<(Vec<Row>, Option<usize>)> {
        #[cfg(test)]
        {
            self.cost += self.model.entries.len() - from;
        }
        derive(&self.model, from, turns_from, &self.opened)
    }
}

/// The smallest replacement that turns `old` into `new`.
fn splice(old: &[Row], new: &[Row]) -> Option<Splice> {
    let start = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let room = old.len().min(new.len()) - start;
    let end = old.iter().rev().zip(new.iter().rev()).take(room).take_while(|(a, b)| a == b).count();
    let (removed, inserted) = (old.len() - start - end, new.len() - start - end);
    (removed > 0 || inserted > 0).then_some(Splice { start, removed, inserted })
}

const AFTER_CUT: &str = "previous turn was cut; resuming";
const RESUMED: &str = "resumed at the reset";
const INTERNAL_RESULT: &str = "This tool result is internal metadata";
const EARLIER: &str = "earlier";

/// The fold's state just before a turn opened, which a live event of that turn folds again from.
#[derive(Default)]
struct Mark {
    event: usize,
    entries: usize,
    turns: usize,
    seen: usize,
    starts: HashMap<String, usize>,
    earlier: usize,
    cwd: Option<String>,
    /// How many rows the entries before `entries` draw.
    rows: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Running,
    Completed,
    Interrupted,
    Failed,
}

impl From<SessionEventResultStatus> for State {
    fn from(status: SessionEventResultStatus) -> Self {
        match status {
            SessionEventResultStatus::Completed => State::Completed,
            SessionEventResultStatus::Interrupted => State::Interrupted,
            SessionEventResultStatus::Failed => State::Failed,
        }
    }
}

/// TurnSummary in the web app's adapt/view-model.ts, cut to what the rows read.
struct Summary {
    id: String,
    state: State,
    started: Option<f64>,
    completed: Option<f64>,
    duration_ms: Option<f64>,
    waited_ms: Option<f64>,
    error: Option<String>,
    limit: bool,
}

impl Summary {
    /// turnWorkedMs in packages/protocol/src/words/turn.ts.
    fn worked(&self) -> Option<f64> {
        let duration = self.duration_ms?;
        let waited = self.waited_ms.filter(|w| *w > 0.0).unwrap_or(0.0).min(duration);
        Some(duration - waited)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Running,
    Completed,
    Failed,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    /// runtime.notice: the host's line about the machine, which stands alone.
    Notice,
    /// runtime.error: a failed turn's words, drawn in the destructive ink.
    Error,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Command,
    FileChange,
    WebSearch,
    Agent,
    Mcp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    Command,
    FileRead,
    FileChange,
}

#[derive(Clone, Debug)]
struct Patch {
    verb: &'static str,
    what: String,
    one: bool,
    created: bool,
    text: String,
}

/// WorkLogEntry in the web app's adapt/view-model.ts, cut to what the rows read.
#[derive(Clone, Debug)]
struct Work {
    label: String,
    tool: Option<String>,
    tone: Tone,
    source: Source,
    status: Option<Status>,
    call: Option<String>,
    detail: Option<String>,
    preview: Option<String>,
    command: Option<String>,
    description: Option<String>,
    changed: Vec<String>,
    item: Option<Item>,
    request: Option<Request>,
    output: Option<String>,
    exit_code: Option<i64>,
    duration_ms: Option<f64>,
    patch: Option<Patch>,
}

impl Work {
    fn new(label: impl Into<String>, tone: Tone, source: Source) -> Self {
        Work {
            label: label.into(),
            tool: None,
            tone,
            source,
            status: None,
            call: None,
            detail: None,
            preview: None,
            command: None,
            description: None,
            changed: Vec::new(),
            item: None,
            request: None,
            output: None,
            exit_code: None,
            duration_ms: None,
            patch: None,
        }
    }

    fn facts(&mut self, facts: Facts) {
        self.item = facts.item.or(self.item);
        self.request = facts.request.or(self.request);
        self.detail = facts.detail.or(self.detail.take());
        self.command = facts.command.or(self.command.take());
        self.description = facts.description.or(self.description.take());
        if !facts.changed.is_empty() {
            self.changed = facts.changed;
        }
    }
}

enum Body {
    Person {
        text: String,
        files: Vec<FileTile>,
    },
    Agent {
        text: String,
        streaming: bool,
        updated: Option<f64>,
    },
    /// A proposed plan, drawn as an agent message: the contract has no plan card.
    Plan(String),
    /// A permission prompt the agent raised, drawn elsewhere; it still splits the calls around it.
    Prompt {
        open: bool,
    },
    Work(Box<Work>),
}

struct Entry {
    key: String,
    /// The turn the entry belongs to; None for what the person sent and for a conversation from outside wsp.
    turn: Option<String>,
    at: Option<f64>,
    earlier: bool,
    body: Body,
}

struct Tool {
    entry: usize,
    name: Option<String>,
    input: String,
}

/// TurnBuild in the web app's adapt/session.ts: what the open turn keeps while its events arrive.
struct Build {
    id: String,
    index: usize,
    ordinal: usize,
    open: Option<usize>,
    open_id: Option<String>,
    saw_text: bool,
    tools: HashMap<String, Tool>,
    anon: Option<usize>,
    reply: Option<SessionEventResultStatus>,
    prompts: HashMap<String, usize>,
    held: HashSet<String>,
    plan: Option<usize>,
    keys: HashMap<String, usize>,
}

struct Outcome {
    status: SessionEventResultStatus,
    duration_ms: Option<f64>,
    waited_ms: Option<f64>,
    error: Option<String>,
    limit: bool,
    text: Option<String>,
}

impl Outcome {
    fn of(result: &SessionEventResult) -> Self {
        Outcome {
            status: result.status,
            duration_ms: result.duration_ms,
            waited_ms: result.waited_ms,
            error: result.error.clone(),
            limit: result.limit.is_some(),
            text: result.text.clone(),
        }
    }

    fn failed(error: String) -> Self {
        Outcome {
            status: SessionEventResultStatus::Failed,
            duration_ms: None,
            waited_ms: None,
            error: Some(error),
            limit: false,
            text: None,
        }
    }
}

/// createSessionFold in the web app's adapt/session.ts: events in, timeline entries and turn summaries out.
#[derive(Default)]
struct Model {
    entries: Vec<Entry>,
    turns: Vec<Summary>,
    /// Every turn id a turn or a refusal named, and the order they came in, so a restore can forget the later ones.
    seen: HashSet<String>,
    order: Vec<String>,
    starts: HashMap<String, usize>,
    earlier: usize,
    cwd: Option<String>,
    place: Option<u64>,
    turn: Option<Build>,
    /// Something since the latest turn opened ties its rows to rows before it, so only a whole fold is right.
    tangled: bool,
    latest: Option<Mark>,
}

/// The fields every event carries: its position, its stamp, its turn and its session.
fn head(event: &SessionEvent) -> (Option<u64>, Option<f64>, Option<&str>, &str) {
    use SessionEvent as E;
    match event {
        E::SessionStart { pos, at, turn_id, session_id, .. }
        | E::SessionDelta { pos, at, turn_id, session_id, .. }
        | E::SessionDone { pos, at, turn_id, session_id, .. }
        | E::SessionEnd { pos, at, turn_id, session_id, .. }
        | E::SessionSteer { pos, at, turn_id, session_id, .. }
        | E::SessionNotify { pos, at, turn_id, session_id, .. }
        | E::SessionPermission { pos, at, turn_id, session_id, .. }
        | E::SessionPermissionClosed { pos, at, turn_id, session_id, .. }
        | E::SessionCheckpoint { pos, at, turn_id, session_id, .. }
        | E::SessionChanges { pos, at, turn_id, session_id, .. }
        | E::SessionPlan { pos, at, turn_id, session_id, .. }
        | E::SessionCompacted { pos, at, turn_id, session_id, .. }
        | E::SessionRetry { pos, at, turn_id, session_id, .. }
        | E::SessionContext { pos, at, turn_id, session_id, .. }
        | E::SessionRun { pos, at, turn_id, session_id, .. }
        | E::SessionMoved { pos, at, turn_id, session_id, .. }
        | E::SessionBehind { pos, at, turn_id, session_id, .. }
        | E::SessionNotice { pos, at, turn_id, session_id, .. }
        | E::SessionEarlier { pos, at, turn_id, session_id, .. }
        | E::SessionCapped { pos, at, turn_id, session_id, .. }
        | E::SessionUncapped { pos, at, turn_id, session_id, .. }
        | E::SessionSubagent { pos, at, turn_id, session_id, .. }
        | E::SessionSlate { pos, at, turn_id, session_id, .. }
        | E::ThreadWaited { pos, at, turn_id, session_id, .. } => (pos.map(NonZeroU64::get), *at, turn_id.as_deref(), session_id),
    }
}

impl Model {
    fn checkpoint(&mut self, event: usize) {
        self.tangled = false;
        self.latest = Some(Mark {
            event,
            entries: self.entries.len(),
            turns: self.turns.len(),
            seen: self.order.len(),
            starts: self.starts.clone(),
            earlier: self.earlier,
            cwd: self.cwd.clone(),
            rows: 0,
        });
    }

    fn restore(&mut self, mark: &Mark) {
        self.entries.truncate(mark.entries);
        self.turns.truncate(mark.turns);
        for id in self.order.drain(mark.seen..) {
            self.seen.remove(&id);
        }
        self.starts = mark.starts.clone();
        self.earlier = mark.earlier;
        self.cwd = mark.cwd.clone();
        self.turn = None;
        self.tangled = false;
        self.latest = None;
    }

    fn note_id(&mut self, id: &str) {
        if self.seen.insert(id.to_owned()) {
            self.order.push(id.to_owned());
        } else {
            self.tangled = true;
        }
    }

    fn open(&mut self, id: String, at: Option<f64>) {
        self.note_id(&id);
        self.turns.push(Summary {
            id: id.clone(),
            state: State::Running,
            started: at,
            completed: None,
            duration_ms: None,
            waited_ms: None,
            error: None,
            limit: false,
        });
        self.turn = Some(Build {
            id,
            index: self.turns.len() - 1,
            ordinal: 0,
            open: None,
            open_id: None,
            saw_text: false,
            tools: HashMap::new(),
            anon: None,
            reply: None,
            prompts: HashMap::new(),
            held: HashSet::new(),
            plan: None,
            keys: HashMap::new(),
        });
    }

    /// turnFor: the turn an event lands in, opened here when history starts in the middle of one.
    fn turn_for(&mut self, ix: usize, turn_id: Option<&str>, session: &str, at: Option<f64>) {
        if let Some(t) = &self.turn {
            if turn_id.is_none_or(|id| id == t.id) {
                return;
            }
        }
        self.end_running(at);
        self.checkpoint(ix);
        self.open(turn_id.map_or_else(|| format!("{session}#0"), str::to_owned), at);
    }

    fn running(&self) -> bool {
        self.turn.as_ref().is_some_and(|t| self.turns[t.index].state == State::Running)
    }

    /// endRunningTurn: a running turn a new one supersedes keeps its reply's status, or failed if it never replied.
    fn end_running(&mut self, at: Option<f64>) {
        if !self.running() {
            return;
        }
        match self.turn.as_ref().and_then(|t| t.reply) {
            Some(status) => self.set_state(status.into()),
            None => self.finish(&Outcome::failed("session restarted before it finished".to_owned()), at),
        }
    }

    fn set_state(&mut self, state: State) {
        if let Some(t) = &self.turn {
            self.turns[t.index].state = state;
        }
    }

    fn finish(&mut self, outcome: &Outcome, at: Option<f64>) {
        self.apply_result(outcome, at);
        self.set_state(outcome.status.into());
    }

    fn apply_result(&mut self, outcome: &Outcome, at: Option<f64>) {
        self.close();
        let t = self.turn.as_ref().expect("a result lands in a turn");
        for tool in t.tools.values() {
            if let Body::Work(w) = &mut self.entries[tool.entry].body {
                if w.status == Some(Status::Running) {
                    w.status = Some(Status::Stopped);
                }
            }
        }
        let (saw_text, index) = (t.saw_text, t.index);
        if outcome.status == SessionEventResultStatus::Completed && !saw_text {
            if let Some(text) = outcome.text.as_ref().filter(|t| !t.is_empty()) {
                self.add_message(Body::Agent { text: text.clone(), streaming: false, updated: at }, at);
            }
        }
        if outcome.status == SessionEventResultStatus::Failed && !outcome.limit {
            if let Some(error) = outcome.error.as_ref().filter(|e| !killed_for_memory(e)) {
                self.add_work(Work::new(error.clone(), Tone::Error, Source::Error), at);
            }
        }
        let summary = &mut self.turns[index];
        summary.duration_ms = outcome.duration_ms;
        summary.waited_ms = outcome.waited_ms;
        summary.error = outcome.error.clone();
        summary.limit = outcome.limit;
        summary.completed = at;
    }

    fn close(&mut self) {
        let Some(t) = &mut self.turn else { return };
        if let Some(ix) = t.open.take() {
            if let Body::Agent { streaming, .. } = &mut self.entries[ix].body {
                *streaming = false;
            }
        }
        t.open_id = None;
    }

    /// rowId: a row is named by its turn and the event that opened it, or by its call.
    fn key(&mut self, kind: &str, call: Option<&str>) -> String {
        let t = self.turn.as_mut().expect("a keyed entry lands in a turn");
        t.ordinal += 1;
        let Some(place) = self.place else { return format!("{}:{kind}{}", t.id, t.ordinal) };
        let base = match call {
            Some(call) => format!("{}:call:{call}", t.id),
            None => format!("{}:{kind}@{place}", t.id),
        };
        let n = t.keys.entry(base.clone()).or_insert(0);
        let key = if *n == 0 { base } else { format!("{base}.{n}") };
        *n += 1;
        key
    }

    fn add_message(&mut self, body: Body, at: Option<f64>) -> usize {
        let key = self.key("m", None);
        let turn = matches!(body, Body::Agent { .. }).then(|| self.turn.as_ref().map(|t| t.id.clone())).flatten();
        self.entries.push(Entry { key, turn, at, earlier: false, body });
        self.entries.len() - 1
    }

    fn add_work(&mut self, work: Work, at: Option<f64>) -> usize {
        let key = self.key("w", work.call.as_deref());
        let turn = self.turn.as_ref().map(|t| t.id.clone());
        self.entries.push(Entry { key, turn, at, earlier: false, body: Body::Work(Box::new(work)) });
        self.entries.len() - 1
    }

    fn notice(&mut self, label: String, detail: Option<String>, source: Source, at: Option<f64>) {
        let mut work = Work::new(label, Tone::Notice, source);
        work.detail = detail;
        self.add_work(work, at);
    }

    fn add(&mut self, ix: usize, event: &SessionEvent) {
        let (pos, at, turn_id, session) = head(event);
        self.place = pos;
        match event {
            SessionEvent::SessionStart { prompt, after_cut, after_limit, attachments, cwd, .. } => {
                if let Some(cwd) = cwd {
                    self.cwd = Some(cwd.clone());
                }
                self.end_running(at);
                self.checkpoint(ix);
                let count = self.starts.get(session).map_or(1, |n| n + 1);
                self.starts.insert(session.to_owned(), count);
                self.open(turn_id.map_or_else(|| format!("{session}#{count}"), str::to_owned), at);
                if let (Some(prompt), None) = (prompt, after_limit) {
                    self.add_message(Body::Person { text: prompt.clone(), files: tiles(attachments) }, at);
                }
                if *after_cut == Some(true) {
                    self.notice(AFTER_CUT.to_owned(), None, Source::Other, at);
                }
                if after_limit.is_some() {
                    self.notice(RESUMED.to_owned(), None, Source::Other, at);
                }
            }
            SessionEvent::SessionDelta { .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.delta(event, at);
            }
            SessionEvent::SessionSteer { prompt, attachments, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                self.add_message(Body::Person { text: prompt.clone(), files: tiles(attachments) }, at);
            }
            SessionEvent::SessionNotify { notify, text, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                let label = if notify == "me" {
                    "told you".to_owned()
                } else {
                    format!("told thread {}", notify.chars().take(8).collect::<String>())
                };
                self.notice(label, Some(text.clone()), Source::Other, at);
            }
            SessionEvent::SessionPermission { ask_id, tool_use_id, parent_tool_use_id, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                match (tool_use_id, parent_tool_use_id) {
                    (Some(call), None) => _ = t.held.insert(call.clone()),
                    (Some(_), Some(_)) => {}
                    (None, _) => t.held.extend(t.tools.keys().cloned()),
                }
                if parent_tool_use_id.is_some() {
                    return;
                }
                let turn = Some(t.id.clone());
                t.prompts.insert(ask_id.clone(), self.entries.len());
                self.entries.push(Entry {
                    key: format!("permission:{ask_id}"),
                    turn,
                    at,
                    earlier: false,
                    body: Body::Prompt { open: true },
                });
            }
            SessionEvent::SessionPermissionClosed { ask_id, .. } => {
                let Some(ix) = self.turn.as_ref().and_then(|t| t.prompts.get(ask_id).copied()) else { return };
                if let Body::Prompt { open } = &mut self.entries[ix].body {
                    *open = false;
                }
            }
            SessionEvent::SessionContext { .. } => self.turn_for(ix, turn_id, session, at),
            SessionEvent::SessionCompacted { before, after, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                self.add_work(Work::new(compacted(*before, *after), Tone::Compaction, Source::Other), at);
            }
            SessionEvent::SessionPlan { text, .. } => {
                self.turn_for(ix, turn_id, session, at);
                let Some(text) = text else { return };
                let t = self.turn.as_ref().expect("turn_for opened a turn");
                if let Some(ix) = t.plan {
                    self.entries[ix].body = Body::Plan(text.clone());
                    return;
                }
                let (key, turn) = (format!("plan:{}", t.id), Some(t.id.clone()));
                self.close();
                self.entries.push(Entry { key, turn, at, earlier: false, body: Body::Plan(text.clone()) });
                let ix = self.entries.len() - 1;
                if let Some(t) = &mut self.turn {
                    t.plan = Some(ix);
                }
            }
            SessionEvent::SessionDone { result, .. } => {
                self.turn_for(ix, turn_id, session, at);
                let outcome = Outcome::of(result);
                self.apply_result(&outcome, at);
                if let Some(t) = &mut self.turn {
                    t.reply = Some(outcome.status);
                }
            }
            SessionEvent::SessionEnd { reason, saw_result, exit_code, .. } => {
                let current = self.turn.as_ref().map(|t| t.id.as_str());
                if current.is_none() || turn_id.is_some_and(|id| Some(id) != current) {
                    self.end_running(at);
                    if let Some(reason) = reason {
                        if let Some(id) = turn_id {
                            self.note_id(id);
                        }
                        let key = format!("refusal:{}", turn_id.unwrap_or(session));
                        let body = Body::Work(Box::new(Work::new(reason.clone(), Tone::Error, Source::Error)));
                        self.entries.push(Entry { key, turn: turn_id.map(str::to_owned), at, earlier: false, body });
                    }
                    return;
                }
                if !self.running() {
                    return;
                }
                match self.turn.as_ref().and_then(|t| t.reply) {
                    Some(status) => self.set_state(status.into()),
                    None => {
                        let error = reason.clone().unwrap_or_else(|| match (saw_result, exit_code) {
                            (true, _) => "session exited without a result".to_owned(),
                            (false, Some(code)) => format!("session exited without a result (exit code {code})"),
                            (false, None) => "session exited without a result (exit code unknown)".to_owned(),
                        });
                        self.finish(&Outcome::failed(error), at);
                    }
                }
            }
            SessionEvent::SessionNotice { text, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                self.notice(text.clone(), None, Source::Notice, at);
            }
            SessionEvent::ThreadWaited { ms, said, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                self.notice(said.clone().unwrap_or_else(|| waited(*ms)), None, Source::Notice, at);
            }
            SessionEvent::SessionEarlier { who, text, .. } => {
                if !self.turns.is_empty() {
                    self.tangled = true;
                }
                self.earlier += 1;
                let key = format!("earlier:{session}:{}", pos.map_or(format!("n{}", self.earlier), |p| p.to_string()));
                let body = match who {
                    SessionEventWho::Person => Body::Person { text: text.clone(), files: Vec::new() },
                    SessionEventWho::Agent => Body::Agent { text: text.clone(), streaming: false, updated: at },
                    SessionEventWho::Note => Body::Work(Box::new(Work::new(text.clone(), Tone::Notice, Source::Other))),
                    SessionEventWho::Tool => Body::Work(Box::new(Work::new(text.clone(), Tone::Tool, Source::Other))),
                };
                self.entries.push(Entry { key, turn: None, at, earlier: true, body });
            }
            _ => {}
        }
    }

    fn delta(&mut self, event: &SessionEvent, at: Option<f64>) {
        let SessionEvent::SessionDelta {
            kind,
            text,
            message_id,
            tool_name,
            tool_use_id,
            is_error,
            parent_tool_use_id,
            exit_code,
            duration_ms,
            patch,
            ..
        } = event
        else {
            return;
        };
        if parent_tool_use_id.is_some() {
            self.close();
            return;
        }
        match kind {
            SessionEventKind::Text => {
                let t = self.turn.as_ref().expect("turn_for opened a turn");
                if t.open_id.is_some() && message_id.is_some() && *message_id != t.open_id {
                    self.close();
                }
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                t.saw_text = true;
                if let Some(Body::Agent { text: said, updated, .. }) = t.open.map(|ix| &mut self.entries[ix].body) {
                    said.push_str(text);
                    *updated = at.or(*updated);
                    return;
                }
                let ix = self.add_message(Body::Agent { text: text.clone(), streaming: true, updated: at }, at);
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                t.open = Some(ix);
                t.open_id = message_id.clone();
            }
            SessionEventKind::Note => self.notice(text.clone(), None, Source::Other, at),
            SessionEventKind::Thinking => {
                self.close();
                let turn = self.turn.as_ref().map(|t| t.id.as_str());
                if let Some(last) = self.entries.last_mut().filter(|e| e.turn.as_deref() == turn) {
                    if let Body::Work(w) = &mut last.body {
                        if w.tone == Tone::Thinking {
                            let detail = w.detail.take().unwrap_or_default() + text;
                            w.preview = preview(&detail).or(w.preview.take());
                            w.detail = Some(detail);
                            return;
                        }
                    }
                }
                let mut work = Work::new("Thinking", Tone::Thinking, Source::Other);
                work.preview = preview(text);
                work.detail = Some(text.clone());
                self.add_work(work, at);
            }
            SessionEventKind::ToolUse => {
                self.close();
                let cwd = self.cwd.clone();
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let key = tool_use_id.clone().or_else(|| t.anon.map(|ix| format!("anon:{ix}")));
                if let Some(tool) = key.and_then(|key| t.tools.get_mut(&key)) {
                    if let Body::Work(w) = &mut self.entries[tool.entry].body {
                        if w.status == Some(Status::Running) {
                            tool.input.push_str(text);
                            let arriving = tool_name.clone().unwrap_or_else(|| w.label.clone());
                            let grown = facts(&arriving, &tool.input, cwd.as_deref());
                            let named = grown.title.clone().unwrap_or(arriving);
                            w.label = named.clone();
                            w.tool = Some(named);
                            w.facts(grown);
                            return;
                        }
                    }
                }
                let name = tool_name.as_deref().unwrap_or("tool");
                let found = facts(name, text, cwd.as_deref());
                let named = found.title.clone().unwrap_or_else(|| name.to_owned());
                let mut work = Work::new(named.clone(), Tone::Tool, Source::Other);
                work.tool = Some(named);
                work.status = Some(Status::Running);
                work.call = tool_use_id.clone();
                work.facts(found);
                let ix = self.add_work(work, at);
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let key = tool_use_id.clone().unwrap_or_else(|| format!("anon:{ix}"));
                t.tools.insert(key, Tool { entry: ix, name: tool_name.clone(), input: text.clone() });
                if tool_use_id.is_none() {
                    t.anon = Some(ix);
                }
            }
            SessionEventKind::ToolResult => {
                self.close();
                let cwd = self.cwd.clone();
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let key = tool_use_id.clone().or_else(|| t.anon.map(|ix| format!("anon:{ix}")));
                let failed_now = *is_error == Some(true);
                let note = text.contains(INTERNAL_RESULT);
                let output = if note { None } else { preview(text) };
                let Some(call) = key.as_ref().and_then(|key| t.tools.get(key)) else {
                    let name = tool_name.clone().unwrap_or_else(|| "tool".to_owned());
                    let mut work = Work::new(name.clone(), Tone::Tool, Source::Other);
                    work.tool = Some(name);
                    work.call = tool_use_id.clone();
                    work.status = Some(if failed_now { Status::Failed } else { Status::Completed });
                    work.detail = output;
                    self.add_work(work, at);
                    return;
                };
                let held = key.as_ref().is_some_and(|key| t.held.contains(key));
                let refused = failed_now && call.name.as_deref().is_some_and(spawns_thread);
                let entry = &mut self.entries[call.entry];
                let Body::Work(w) = &mut entry.body else { return };
                if tool_use_id.is_none() {
                    t.anon = None;
                }
                if let Some(line) = refused.then(|| compact_lines(text).into_iter().next()).flatten() {
                    let mut row = Work::new(line, Tone::Error, Source::Other);
                    row.status = Some(Status::Failed);
                    row.call = w.call.take();
                    **w = row;
                    return;
                }
                let failed = failed_now || w.status == Some(Status::Failed);
                w.status = Some(if failed { Status::Failed } else { Status::Completed });
                if let Some(output) = output {
                    w.detail = Some(output);
                }
                if note || key.is_none() {
                    return;
                }
                let command = w.command.is_some();
                let took = entry.at.zip(at).map(|(from, to)| to - from).filter(|ms| *ms >= 0.0);
                if let Some(ms) = duration_ms.or(took.filter(|_| command && !held)) {
                    w.duration_ms = Some(ms);
                }
                let output = match exit_code {
                    Some(code) => strip_exit(text, *code),
                    None => text.as_str(),
                };
                if command && !output.is_empty() {
                    w.output = Some(output.to_owned());
                }
                if exit_code.is_some() {
                    w.exit_code = *exit_code;
                }
                if let Some(patch) = unified(patch, cwd.as_deref()) {
                    w.patch = Some(patch);
                }
            }
        }
    }
}

/// Claude Code opens a failed command's words with its code for the model, which the row says on its own.
fn strip_exit(text: &str, code: i64) -> &str {
    let lead = format!("Exit code {code}");
    match text.strip_prefix(&lead) {
        Some("") => "",
        Some(rest) => rest.strip_prefix('\n').unwrap_or(text),
        None => text,
    }
}

fn killed_for_memory(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.strip_prefix("the agent on ").and_then(|rest| rest.get(1..)).is_some_and(|rest| rest.contains(" was killed for memory: "))
}

/// spawnsThread in packages/protocol/src/wsp-tools.ts: the wsp tools a cap can refuse.
fn spawns_thread(tool: &str) -> bool {
    matches!(server_tool(tool), Some(("wsp", "run" | "fork")))
}

fn server_tool(tool: &str) -> Option<(&str, &str)> {
    let rest = tool.strip_prefix("mcp__")?;
    let (server, name) = rest.split_once("__")?;
    (!server.is_empty()).then_some((server, name))
}

fn tiles(attachments: &[SessionEventAttachmentsItem]) -> Vec<FileTile> {
    attachments
        .iter()
        .map(|a| {
            let image = matches!(a.media_type.as_str(), "image/png" | "image/jpeg" | "image/gif" | "image/webp");
            let name = a.name.clone().unwrap_or_else(|| if image { "image" } else { "file" }.to_owned());
            FileTile { name, bytes: Some(a.bytes), image }
        })
        .collect()
}

/// waitedLine in packages/protocol/src/workspace-state.ts.
fn waited(ms: u64) -> String {
    let seconds = (ms as f64 / 1_000.0).round();
    if seconds < 60.0 {
        format!("Waited {seconds} s for your computer")
    } else {
        format!("Waited {} min for your computer", (ms as f64 / 60_000.0).round())
    }
}

/// compactedLine in packages/protocol/src/words/turn.ts.
fn compacted(before: Option<f64>, after: Option<f64>) -> String {
    match (before, after) {
        (_, None) => "Compacted the context".to_owned(),
        (None, Some(after)) => format!("Compacted the context to {} tokens", tokens(after)),
        (Some(before), Some(after)) => format!("Compacted the context, {} to {} tokens", tokens(before), tokens(after)),
    }
}

/// fmtTokens in packages/protocol/src/words/units.ts.
fn tokens(n: f64) -> String {
    let count = if n.is_finite() && n > 0.0 { n.round() } else { 0.0 };
    if count < 1_000.0 {
        return format!("{count}");
    }
    let (value, unit) = if count < 999_500.0 {
        (count / 1_000.0, "k")
    } else if count < 999_500_000.0 {
        (count / 1_000_000.0, "M")
    } else {
        (count / 1_000_000_000.0, "B")
    };
    let places = if value < 10.0 {
        2
    } else if value < 100.0 {
        1
    } else {
        0
    };
    let fixed = format!("{value:.places$}");
    let trimmed = if fixed.contains('.') { fixed.trim_end_matches('0').trim_end_matches('.') } else { &fixed };
    format!("{trimmed}{unit}")
}

/// compactLines in adapt/session.ts: each non-blank line with its runs of space as one.
fn compact_lines(text: &str) -> Vec<String> {
    text.lines().map(|line| line.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|line| !line.is_empty()).collect()
}

/// summarizeOutput in adapt/session.ts: the first line worth reading, cut at 84 characters.
fn preview(text: &str) -> Option<String> {
    let lines = compact_lines(text);
    let Some(first) = lines.iter().find(|line| *line != "```") else {
        return (lines.len() > 1).then(|| format!("{} lines", lines.len()));
    };
    if first.chars().count() <= 84 {
        return Some(first.clone());
    }
    let cut: String = first.chars().take(83).collect();
    Some(format!("{}…", cut.trim_end()))
}

fn command_line(command: &str) -> String {
    compact_lines(command).into_iter().next().unwrap_or_else(|| command.trim().to_owned())
}

fn relative(path: &str, cwd: Option<&str>) -> String {
    let rest = cwd.and_then(|root| path.strip_prefix(root.trim_end_matches('/'))).and_then(|rest| rest.strip_prefix('/'));
    rest.unwrap_or(path).to_owned()
}

/// An edit's patch as one unified diff, with the heading its block draws (editHeading in timeline/callFacts.ts).
fn unified(files: &[SessionEventPatchItem], cwd: Option<&str>) -> Option<Patch> {
    let created = |f: &SessionEventPatchItem| matches!(f.hunks.as_slice(), [h] if h.old_start == 0 && h.old_lines == 0);
    let gone = |f: &SessionEventPatchItem| matches!(f.hunks.as_slice(), [h] if h.new_start == 0 && h.new_lines == 0);
    let (verb, what, new) = match files {
        [] => return None,
        [one] => match &one.moved_to {
            Some(to) => ("Moved", format!("{} to {}", relative(&one.path, cwd), relative(to, cwd)), false),
            None if created(one) => ("Wrote", relative(&one.path, cwd), true),
            None => (if gone(one) { "Deleted" } else { "Edited" }, relative(&one.path, cwd), false),
        },
        many => ("Edited", format!("{} files", many.len()), false),
    };
    let mut text = Vec::new();
    for file in files {
        let path = file.path.strip_prefix('/').unwrap_or(&file.path);
        let to = file.moved_to.as_deref().map_or(path, |to| to.strip_prefix('/').unwrap_or(to));
        text.push(format!("diff --git a/{path} b/{to}"));
        if created(file) {
            text.extend(["--- /dev/null".to_owned(), format!("+++ b/{to}")]);
        } else if gone(file) {
            text.extend([format!("--- a/{path}"), "+++ /dev/null".to_owned()]);
        } else {
            text.extend([format!("--- a/{path}"), format!("+++ b/{to}")]);
        }
        for h in &file.hunks {
            text.push(format!("@@ -{},{} +{},{} @@", h.old_start, h.old_lines, h.new_start, h.new_lines));
            text.extend(h.lines.iter().cloned());
        }
    }
    Some(Patch { verb, what, one: files.len() == 1, created: new, text: text.join("\n") })
}

/// ToolCallFacts in packages/protocol/src/words/tools.ts.
#[derive(Default)]
struct Facts {
    title: Option<String>,
    detail: Option<String>,
    command: Option<String>,
    description: Option<String>,
    changed: Vec<String>,
    item: Option<Item>,
    request: Option<Request>,
}

/// One row of TOOL_ROWS: the fields it shows, its title, its kinds and the input field its changed path is in.
struct ToolRow {
    shows: &'static [&'static str],
    title: Option<&'static str>,
    item: Option<Item>,
    request: Option<Request>,
    path: Option<&'static str>,
    code_search: bool,
}

fn tool_row(name: &str) -> Option<ToolRow> {
    let row = |shows: &'static [&'static str], item: Option<Item>, request: Option<Request>| ToolRow {
        shows,
        title: None,
        item,
        request,
        path: None,
        code_search: false,
    };
    Some(match name {
        "Bash" | "command_execution" => row(&["description"], Some(Item::Command), Some(Request::Command)),
        "Read" => row(&["file_path"], None, Some(Request::FileRead)),
        "Write" | "Edit" | "MultiEdit" => {
            ToolRow { path: Some("file_path"), ..row(&["file_path"], Some(Item::FileChange), Some(Request::FileChange)) }
        }
        "NotebookEdit" => {
            ToolRow { path: Some("notebook_path"), ..row(&["notebook_path"], Some(Item::FileChange), Some(Request::FileChange)) }
        }
        "file_change" => ToolRow { path: Some("changes"), ..row(&[], Some(Item::FileChange), Some(Request::FileChange)) },
        "Grep" | "Glob" => ToolRow { code_search: true, ..row(&["pattern"], None, None) },
        "WebSearch" | "web_search" => row(&["query"], Some(Item::WebSearch), None),
        "WebFetch" => row(&["url"], Some(Item::WebSearch), None),
        "Task" | "Agent" => row(&["description"], Some(Item::Agent), None),
        "spawn_agent" => row(&["prompt"], Some(Item::Agent), None),
        "AskUserQuestion" => ToolRow { title: Some("Asked you"), ..row(&[], None, None) },
        _ => return None,
    })
}

const SHOWN_FIELDS: [&str; 7] = ["file_path", "notebook_path", "pattern", "query", "url", "description", "prompt"];

fn facts(name: &str, input: &str, cwd: Option<&str>) -> Facts {
    let row = tool_row(name);
    let lent =
        if row.is_none() { server_tool(name).map(|(server, tool)| format!("Use {server}'s {}", tool.replace('_', " "))) } else { None };
    let item = row.as_ref().and_then(|r| r.item).or(lent.as_ref().map(|_| Item::Mcp));
    let request = row.as_ref().and_then(|r| r.request);
    let title = row.as_ref().and_then(|r| r.title.map(str::to_owned)).or(lent.clone());
    let Some(fields) = serde_json::from_str::<serde_json::Value>(input).ok().filter(serde_json::Value::is_object) else {
        return Facts { title, item, request, detail: (!input.is_empty()).then(|| input.to_owned()), ..Facts::default() };
    };
    let field = |name: &str| fields.get(name).and_then(serde_json::Value::as_str).filter(|v| !v.trim().is_empty()).map(str::to_owned);
    let server: &[&str] = if lent.is_some() { &["title", "message"] } else { &[] };
    let shows = row.as_ref().map_or(&[][..], |r| r.shows);
    let detail = shows.iter().chain(SHOWN_FIELDS.iter()).chain(server.iter()).find_map(|name| field(name));
    let shell = request == Some(Request::Command);
    let changed = match row.as_ref().and_then(|r| r.path) {
        Some("changes") => fields
            .get("changes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|change| change.get("path")?.as_str().filter(|p| !p.is_empty()).map(|p| relative(p, cwd)))
            .collect(),
        Some(path) => field(path).map(|p| relative(&p, cwd)).into_iter().collect(),
        None => Vec::new(),
    };
    Facts {
        title,
        detail,
        command: if shell { field("command") } else { None },
        description: if shell { field("description") } else { None },
        changed,
        item,
        request,
    }
}

// What a tool line reads and draws: the helpers of adapt/timeline-rows.ts, MessagesTimeline.logic.ts and timeline/workEntry.tsx.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Action {
    Read,
    Edit,
    Command,
    Search,
    CodeSearch,
    Other,
    Update,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Action(Action),
    Agent,
    ToneTool,
    Mixed,
}

fn kind_glyph(kind: Kind) -> Glyph {
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
fn stands_alone(w: &Work) -> bool {
    w.tone == Tone::Error || w.tone == Tone::Compaction || w.source == Source::Notice
}

fn tool_like(w: &Work) -> bool {
    matches!(w.tone, Tone::Tool | Tone::Thinking | Tone::Error)
        || non_blank(&w.command).is_some()
        || w.request.is_some()
        || w.item.is_some()
}

fn failure(w: &Work) -> bool {
    w.tone == Tone::Error || w.status == Some(Status::Failed)
}

fn success(w: &Work) -> bool {
    tool_like(w) && !failure(w) && w.tone != Tone::Thinking && !matches!(w.status, Some(Status::Running | Status::Stopped))
}

fn neutral(w: &Work) -> bool {
    if w.tone == Tone::Thinking && non_blank(&w.detail).is_some() {
        return false;
    }
    tool_like(w) && !failure(w) && !success(w)
}

fn visible_in_group(w: &Work, live: bool) -> bool {
    (live && w.status == Some(Status::Running)) || !neutral(w)
}

fn active_activity(w: &Work) -> bool {
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
fn group_kind(group: &[&Work]) -> Kind {
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
fn summary(group: &[&Work]) -> String {
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

struct Label {
    verb: Option<String>,
    text: String,
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
fn display_label(w: &Work) -> Label {
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
    } else {
        let label = display_label(w);
        call.body = work_body(w, &label.line());
        call.verb = label.verb;
        call.label = label.text;
        call.mono = label.mono;
    }
    call
}

fn group_id(e: &Entry, w: &Work) -> String {
    match &w.call {
        Some(call) => format!("group:tool:{}:{call}", e.turn.as_deref().unwrap_or("no-turn")),
        None => format!("group:{}", e.key),
    }
}

/// A work entry standing on its own line: a severe error in the destructive ink, anything else in its tone.
fn work_row(key: &str, e: &Entry, w: &Work) -> Row {
    let failed = failure(w);
    let text = display_label(w).line();
    let kind = if failed && (w.source == Source::Error || !tool_like(w)) {
        RowKind::Error
    } else {
        RowKind::Work { tone: w.tone, glyph: if failed { Glyph::CircleAlert } else { entry_glyph(w) } }
    };
    Row { key: key.to_owned(), kind, text, turn: e.turn.clone() }
}

struct Live<'a> {
    key: String,
    group: String,
    entry: &'a Entry,
    work: &'a Work,
    calls: Vec<usize>,
    active: bool,
    open: bool,
}

impl Live<'_> {
    fn row(&self) -> Row {
        let label = live_label(self.work, self.active);
        let glyph = if failure(self.work) { Glyph::CircleAlert } else { entry_glyph(self.work) };
        let since = (self.work.command.is_some() && self.work.status == Some(Status::Running)).then_some(self.entry.at).flatten();
        Row {
            key: self.key.clone(),
            kind: RowKind::Live { group: self.group.clone(), glyph, verb: label.verb, mono: label.mono, since, open: self.open },
            text: label.text,
            turn: self.entry.turn.clone(),
        }
    }
}

fn work_of(e: &Entry) -> Option<&Work> {
    match &e.body {
        Body::Work(w) => Some(w.as_ref()),
        _ => None,
    }
}

fn calls_row(group: &str, members: &[usize], entries: &[Entry], opened: &HashSet<String>) -> Row {
    let calls = members.iter().filter_map(|&ix| work_of(&entries[ix]).map(|w| call(&entries[ix], w, opened))).collect();
    let turn = members.first().and_then(|&ix| entries[ix].turn.clone());
    Row { key: format!("{group}:calls"), kind: RowKind::Calls { group: group.to_owned(), calls }, text: String::new(), turn }
}

fn elapsed(start: Option<f64>, end: Option<f64>) -> Option<f64> {
    Some((end? - start?).max(0.0))
}

/// deriveMessagesTimelineRows for the entries from `from` on, with the turns from `turns_from` on: the rows, and how
/// many of them come before the latest turn's first entry. None when the working line would land before `from`.
fn derive(model: &Model, from: usize, turns_from: usize, opened: &HashSet<String>) -> Option<(Vec<Row>, Option<usize>)> {
    let entries = &model.entries;
    let len = entries.len();
    let by_id: HashMap<&str, &Summary> = model.turns[turns_from..].iter().map(|s| (s.id.as_str(), s)).collect();
    let latest = model.turns.last();
    let unsettled = latest.filter(|t| t.state == State::Running).map(|t| t.id.as_str());
    let working = unsettled.is_some();
    let boundary = model.latest.as_ref().map(|m| m.entries);
    let turn_of = |ix: usize| entries[ix].turn.as_deref();

    let mut replies: HashMap<(Option<&str>, usize), usize> = HashMap::new();
    let mut users = 0;
    for (ix, e) in entries.iter().enumerate().skip(from) {
        match &e.body {
            Body::Person { .. } => users += 1,
            Body::Agent { .. } => _ = replies.insert((e.turn.as_deref(), if e.turn.is_some() { 0 } else { users }), ix),
            _ => {}
        }
    }
    let terminal: HashSet<usize> = replies.into_values().collect();

    let mut groups: Vec<(&str, Vec<usize>, bool)> = Vec::new();
    let mut group_at: HashMap<&str, usize> = HashMap::new();
    for (ix, e) in entries.iter().enumerate().skip(from) {
        let (Some(turn), false) = (e.turn.as_deref(), matches!(e.body, Body::Person { .. })) else { continue };
        let at = *group_at.entry(turn).or_insert_with(|| {
            groups.push((turn, Vec::new(), false));
            groups.len() - 1
        });
        groups[at].1.push(ix);
        groups[at].2 |= matches!(e.body, Body::Agent { streaming: true, .. });
    }
    let mut folds: HashMap<usize, (&str, String)> = HashMap::new();
    let mut stops: Vec<(usize, &str, String)> = Vec::new();
    let mut collapsed = vec![false; len - from];
    for (turn, members, streaming) in &groups {
        if Some(*turn) == unsettled || *streaming {
            continue;
        }
        let summary = by_id.get(turn);
        let (first, last) = (members[0], members[members.len() - 1]);
        let worked = summary.and_then(|s| s.worked()).or_else(|| {
            elapsed(summary.and_then(|s| s.started).or(entries[first].at), summary.and_then(|s| s.completed).or(entries[last].at))
        });
        if summary.is_some_and(|s| s.state == State::Interrupted) {
            let label = worked.map_or("You stopped this response".to_owned(), |ms| format!("You stopped after {}", duration(ms)));
            stops.push((last, *turn, label));
            continue;
        }
        let kept = |ix: &usize| {
            terminal.contains(ix) || matches!(&entries[*ix].body, Body::Plan(_)) || work_of(&entries[*ix]).is_some_and(stands_alone)
        };
        let hidden: Vec<usize> = members.iter().copied().filter(|ix| !kept(ix)).collect();
        let Some(&first_hidden) = hidden.first() else { continue };
        let label = worked.map_or("Worked".to_owned(), |ms| format!("Worked for {}", duration(ms)));
        folds.insert(first_hidden, (*turn, label));
        if !opened.contains(*turn) {
            for ix in hidden {
                collapsed[ix - from] = true;
            }
        }
    }
    stops.sort_by_key(|(last, _, _)| *last);
    let first_earlier = (from..len).find(|&ix| entries[ix].earlier);
    if !opened.contains(EARLIER) {
        for ix in from..len {
            collapsed[ix - from] |= entries[ix].earlier;
        }
    }

    let header = if working {
        let last_user = (from..len).rev().find(|&ix| matches!(entries[ix].body, Body::Person { .. }));
        let owned = (last_user.map_or(from, |u| u + 1)..len).find(|&ix| turn_of(ix) == unsettled);
        match (owned, last_user) {
            (Some(ix), _) => ix,
            (None, Some(u)) => u + 1,
            (None, None) if from == 0 => 0,
            (None, None) => return None,
        }
    } else {
        len
    };
    let in_run = |ix: usize, w: &Work| w.status == Some(Status::Running) && working && turn_of(ix) == unsettled;

    let mut trailing: Vec<usize> = Vec::new();
    if working {
        for ix in (header..len).rev() {
            match work_of(&entries[ix]) {
                Some(w) if turn_of(ix) == unsettled && !stands_alone(w) => trailing.push(ix),
                _ => break,
            }
        }
        trailing.reverse();
    }
    let shown: Vec<usize> =
        trailing.iter().copied().filter(|&ix| work_of(&entries[ix]).is_some_and(|w| visible_in_group(w, true))).collect();
    let latest_shown = shown.last().copied();
    let latest_running = shown.iter().rev().copied().find(|&ix| work_of(&entries[ix]).is_some_and(active_activity));
    let latest_failed = latest_running.is_none() && latest_shown.and_then(|ix| work_of(&entries[ix])).is_some_and(failure);
    let keeps_live = latest_running.is_some() || latest_shown.and_then(|ix| work_of(&entries[ix])).is_some_and(success);
    let live = match (trailing.first(), latest_running.or(latest_shown)) {
        (Some(&anchor), Some(ix)) if !latest_failed => {
            let anchor_work = work_of(&entries[anchor])?;
            let group = group_id(&entries[anchor], anchor_work);
            Some(Live {
                key: if keeps_live { "live".to_owned() } else { group.clone() },
                open: opened.contains(&group),
                group,
                entry: &entries[ix],
                work: work_of(&entries[ix])?,
                calls: shown.clone(),
                active: keeps_live,
            })
        }
        _ => None,
    };
    let taken: HashSet<usize> = if live.is_some() || latest_failed { trailing.iter().copied().collect() } else { HashSet::new() };
    let waiting = unsettled.is_some()
        && entries[from..].iter().any(|e| e.turn.as_deref() == unsettled && matches!(e.body, Body::Prompt { open: true }));
    let working_row = || Row {
        key: "working".to_owned(),
        kind: RowKind::Working { since: latest.and_then(|t| t.started) },
        text: if waiting { "Waiting for you" } else { "Working" }.to_owned(),
        turn: unsettled.map(str::to_owned),
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut split = None;
    let mut activity = false;
    let mut unsaid = stops.into_iter().peekable();
    let mut ix = from;
    loop {
        while let Some((_, turn, label)) = unsaid.next_if(|(last, _, _)| *last < ix) {
            rows.push(Row { key: format!("stop:{turn}"), kind: RowKind::Stopped, text: label, turn: Some(turn.to_owned()) });
        }
        if boundary == Some(ix) {
            split = Some(rows.len());
        }
        if working && header == ix {
            rows.push(working_row());
        }
        if ix == len {
            break;
        }
        let e = &entries[ix];
        if let Some(live) = live.as_ref().filter(|_| latest_shown == Some(ix)) {
            rows.push(live.row());
            activity |= live.active;
            if live.open {
                rows.push(calls_row(&live.group, &live.calls, entries, opened));
            }
        }
        if first_earlier == Some(ix) {
            let text = "Earlier, outside wsp".to_owned();
            rows.push(Row {
                key: format!("fold:{EARLIER}"),
                kind: RowKind::Fold { open: opened.contains(EARLIER) },
                text,
                turn: Some(EARLIER.to_owned()),
            });
        }
        if let Some((turn, label)) = folds.get(&ix) {
            let kind = RowKind::Fold { open: opened.contains(*turn) };
            rows.push(Row { key: format!("fold:{turn}"), kind, text: label.clone(), turn: Some((*turn).to_owned()) });
        }
        if collapsed[ix - from] || taken.contains(&ix) {
            ix += 1;
            continue;
        }
        match &e.body {
            Body::Work(w) if stands_alone(w) => rows.push(work_row(&e.key, e, w)),
            Body::Work(_) => {
                let mut end = ix + 1;
                while end < len
                    && work_of(&entries[end]).is_some_and(|w| !stands_alone(w))
                    && !taken.contains(&end)
                    && !collapsed[end - from]
                    && !folds.contains_key(&end)
                    && entries[end].turn == e.turn
                {
                    end += 1;
                }
                let members: Vec<usize> =
                    (ix..end).filter(|&k| work_of(&entries[k]).is_some_and(|w| visible_in_group(w, in_run(k, w)))).collect();
                let works: Vec<&Work> = members.iter().filter_map(|&k| work_of(&entries[k])).collect();
                match works.as_slice() {
                    [] => {}
                    [only] if !tool_like(only) => rows.push(work_row(&e.key, &entries[members[0]], only)),
                    _ => {
                        let group = work_of(e).map(|w| group_id(e, w)).unwrap_or_default();
                        let open = opened.contains(&group);
                        let running: Vec<usize> =
                            members.iter().copied().filter(|&k| work_of(&entries[k]).is_some_and(|w| in_run(k, w))).collect();
                        if let Some(&last) = running.last() {
                            let live = Live {
                                key: group.clone(),
                                group: group.clone(),
                                entry: &entries[last],
                                work: work_of(&entries[last])?,
                                calls: members.clone(),
                                active: true,
                                open,
                            };
                            rows.push(live.row());
                            activity = true;
                        } else {
                            let kind = RowKind::Group { group: group.clone(), glyph: kind_glyph(group_kind(&works)), open };
                            rows.push(Row { key: group.clone(), kind, text: summary(&works), turn: e.turn.clone() });
                        }
                        if open {
                            rows.push(calls_row(&group, &members, entries, opened));
                        }
                    }
                }
                ix = end;
                continue;
            }
            Body::Plan(text) => rows.push(Row {
                key: e.key.clone(),
                kind: RowKind::Agent { at: e.at, meta: false, streaming: false },
                text: text.clone(),
                turn: e.turn.clone(),
            }),
            Body::Prompt { .. } => {}
            Body::Person { text, files } => rows.push(Row {
                key: e.key.clone(),
                kind: RowKind::Person { at: e.at, files: files.clone() },
                text: text.clone(),
                turn: None,
            }),
            Body::Agent { text, streaming, updated } => {
                let still = e.turn.is_some() && e.turn.as_deref() == unsettled;
                let kind = RowKind::Agent { at: *updated, meta: terminal.contains(&ix) && !still, streaming: *streaming };
                rows.push(Row { key: e.key.clone(), kind, text: text.clone(), turn: e.turn.clone() });
            }
        }
        ix += 1;
    }
    if working && boundary.is_some_and(|b| header < b) {
        split = None;
    }
    if working && !waiting && (!activity || latest_failed) {
        rows.push(Row {
            key: "thinking".to_owned(),
            kind: RowKind::Thinking,
            text: "Thinking".to_owned(),
            turn: unsettled.map(str::to_owned),
        });
    }
    if let Some(t) = latest.filter(|t| t.state != State::Running) {
        let on_reply = t.state == State::Completed || (t.state == State::Failed && t.error.is_some() && !t.limit);
        if !on_reply {
            let (word, text) = match t.state {
                _ if t.limit => (FooterWord::LimitReached, "limit reached"),
                State::Interrupted => (FooterWord::Interrupted, "interrupted"),
                _ => (FooterWord::Failed, "failed"),
            };
            rows.push(Row { key: "footer".to_owned(), kind: RowKind::Footer { word }, text: text.to_owned(), turn: Some(t.id.clone()) });
        }
    }
    Some((rows, split))
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
        }
    }

    fn shown(rows: &[Row]) -> Vec<(&'static str, &str)> {
        rows.iter().map(|r| (name(&r.kind), r.text.as_str())).collect()
    }

    fn opened(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn a_settled_turn_keeps_the_message_one_fold_and_the_last_reply() {
        let rows = rows(&turn(true), &HashSet::new());
        assert_eq!(shown(&rows), [("person", "fix it"), ("fold", "Worked for 12s"), ("agent", "Done here.")]);
        assert_eq!(rows[1].kind, RowKind::Fold { open: false });
        assert_eq!(rows[1].turn.as_deref(), Some("u1"));
        assert_eq!(rows[2].kind, RowKind::Agent { at: None, meta: true, streaming: false });
    }

    #[test]
    fn an_opened_turn_draws_its_hidden_rows_in_place_under_the_fold() {
        let rows = rows(&turn(true), &opened(&["u1"]));
        assert_eq!(
            shown(&rows),
            [("person", "fix it"), ("fold", "Worked for 12s"), ("agent", "Looking."), ("group", "Ran 1 command"), ("agent", "Done here."),]
        );
        assert_eq!(rows[1].kind, RowKind::Fold { open: true });
        assert_eq!(rows[2].kind, RowKind::Agent { at: None, meta: false, streaming: false });
    }

    #[test]
    fn a_turn_without_a_clock_reads_worked() {
        let events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "a", "text": "{\"file_path\":\"a\"}"})),
            base("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "ok"})),
            base("session.delta", 4, json!({"kind": "text", "text": "Done."})),
            base("session.done", 5, json!({"result": {"status": "completed"}})),
            base("session.end", 6, json!({"exitCode": 0, "sawResult": true})),
        ];
        assert_eq!(shown(&rows(&events, &HashSet::new())), [("person", "go"), ("fold", "Worked"), ("agent", "Done.")]);
    }

    #[test]
    fn the_person_row_carries_its_stamp_and_files() {
        let events = vec![
            base(
                "session.start",
                1,
                json!({"prompt": "look", "at": 5_000.0, "attachments": [
                    {"mediaType": "image/png", "bytes": 1200, "name": "shot.png"},
                    {"mediaType": "text/plain", "bytes": 30}
                ]}),
            ),
            base(
                "session.steer",
                2,
                json!({"prompt": "and this", "at": 6_000.0, "attachments": [{"mediaType": "image/jpeg", "bytes": 9}]}),
            ),
        ];
        let rows = rows(&events, &HashSet::new());
        assert_eq!(
            rows[0].kind,
            RowKind::Person {
                at: Some(5_000.0),
                files: vec![
                    FileTile { name: "shot.png".into(), bytes: Some(1200), image: true },
                    FileTile { name: "file".into(), bytes: Some(30), image: false },
                ],
            }
        );
        assert_eq!(
            rows[1].kind,
            RowKind::Person { at: Some(6_000.0), files: vec![FileTile { name: "image".into(), bytes: Some(9), image: true }] }
        );
        assert_eq!(rows[1].text, "and this");
        assert_eq!(name(&rows[2].kind), "working");
    }

    #[test]
    fn a_running_turn_reads_working_its_replies_and_one_live_line() {
        let mut events = turn(false);
        events[5] = base(
            "session.delta",
            6,
            json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "c", "text": "{\"command\":\"cargo test\"}", "at": 4_000.0}),
        );
        events.truncate(6);
        let rows = rows(&events, &HashSet::new());
        assert_eq!(shown(&rows), [("person", "fix it"), ("working", "Working"), ("agent", "Looking."), ("live", "cargo test")]);
        assert_eq!(rows[1].kind, RowKind::Working { since: Some(1_000.0) });
        assert_eq!(rows[2].kind, RowKind::Agent { at: None, meta: false, streaming: false });
        let group = "group:tool:u1:a".to_owned();
        assert_eq!(
            rows[3].kind,
            RowKind::Live { group, glyph: Glyph::Terminal, verb: Some("Running".into()), mono: true, since: Some(4_000.0), open: false }
        );
        assert_eq!(rows[3].key, "live");
    }

    #[test]
    fn a_streaming_reply_is_marked_and_thinking_closes_the_running_turn() {
        let events =
            vec![base("session.start", 1, json!({"prompt": "go"})), base("session.delta", 2, json!({"kind": "text", "text": "Hel"}))];
        let rows = rows(&events, &HashSet::new());
        assert_eq!(shown(&rows), [("person", "go"), ("working", "Working"), ("agent", "Hel"), ("thinking", "Thinking")]);
        assert_eq!(rows[2].kind, RowKind::Agent { at: None, meta: false, streaming: true });
    }

    #[test]
    fn a_finished_trailing_call_reads_ran_and_a_failed_one_leaves_thinking() {
        let mut events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"ls -la\\nmore\"}"}),
            ),
            base("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "out", "exitCode": 0})),
        ];
        let live = rows(&events, &HashSet::new());
        assert_eq!(shown(&live), [("person", "go"), ("working", "Working"), ("live", "ls -la")]);
        assert!(matches!(&live[2].kind, RowKind::Live { verb: Some(v), since: None, .. } if v == "Ran"));
        events.push(base(
            "session.delta",
            4,
            json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "b", "text": "{\"command\":\"false\"}"}),
        ));
        events.push(base(
            "session.delta",
            5,
            json!({"kind": "tool_result", "toolUseId": "b", "text": "Exit code 1\nboom", "isError": true, "exitCode": 1}),
        ));
        assert_eq!(shown(&rows(&events, &HashSet::new())), [("person", "go"), ("working", "Working"), ("thinking", "Thinking")]);
    }

    #[test]
    fn an_open_group_lists_its_calls_with_their_results() {
        let events = vec![
            base("session.start", 1, json!({"prompt": "go", "cwd": "/w"})),
            base(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"false\"}", "at": 1_000.0}),
            ),
            base(
                "session.delta",
                3,
                json!({"kind": "tool_result", "toolUseId": "a", "text": "Exit code 1\n\nboom\n", "isError": true, "exitCode": 1, "at": 1_850.0}),
            ),
            base(
                "session.delta",
                4,
                json!({"kind": "tool_use", "toolName": "Edit", "toolUseId": "b", "text": "{\"file_path\":\"/w/src/a.rs\"}"}),
            ),
            base(
                "session.delta",
                5,
                json!({"kind": "tool_result", "toolUseId": "b", "text": "updated", "patch": [{"path": "/w/src/a.rs", "hunks": [
                    {"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1, "lines": ["-a", "+b"]}
                ]}]}),
            ),
            base(
                "session.delta",
                6,
                json!({"kind": "tool_use", "toolName": "Grep", "toolUseId": "c", "text": "{\"pattern\":\"fn main\"}"}),
            ),
            base("session.delta", 7, json!({"kind": "tool_result", "toolUseId": "c", "text": "src/main.rs:1\nsrc/lib.rs:4"})),
            base("session.delta", 8, json!({"kind": "text", "text": "Done."})),
        ];
        let group = "group:tool:u1:a";
        let rows = rows(&events, &opened(&[group, "u1:call:c"]));
        assert_eq!(rows[2].kind, RowKind::Group { group: group.into(), glyph: Glyph::Hammer, open: true });
        assert_eq!(rows[2].text, "Ran 1 command, changed 1 file, and searched code 1 time");
        let RowKind::Calls { calls, .. } = &rows[3].kind else { panic!("no calls row: {rows:?}") };
        assert_eq!(rows[3].key, format!("{group}:calls"));
        let [bash, edit, grep] = calls.as_slice() else { panic!("{calls:?}") };
        assert_eq!((bash.label.as_str(), bash.command.as_deref(), bash.mono), ("false", Some("false"), true));
        assert_eq!((bash.failed, bash.exit_code, bash.duration_ms, bash.glyph), (true, Some(1), Some(850.0), Glyph::Terminal));
        assert_eq!(bash.body.as_deref(), Some("boom"));
        assert_eq!(
            (edit.verb.as_deref(), edit.label.as_str(), edit.glyph, edit.mono),
            (Some("Edited"), "src/a.rs", Glyph::SquarePen, true)
        );
        assert_eq!(
            edit.patch.as_deref(),
            Some("diff --git a/w/src/a.rs b/w/src/a.rs\n--- a/w/src/a.rs\n+++ b/w/src/a.rs\n@@ -1,1 +1,1 @@\n-a\n+b")
        );
        assert_eq!((grep.label.as_str(), grep.glyph, grep.open, grep.body.as_deref()), ("src/main.rs:1", Glyph::Search, true, None));
        assert!(!bash.open);
    }

    #[test]
    fn a_group_reads_its_one_kind_and_thinking_alone_reads_thinking() {
        let events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base("session.delta", 2, json!({"kind": "thinking", "text": "Let me see"})),
            base("session.delta", 3, json!({"kind": "text", "text": "Hm."})),
            base("session.delta", 4, json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "a", "text": "{\"file_path\":\"a\"}"})),
            base("session.delta", 5, json!({"kind": "tool_result", "toolUseId": "a", "text": "x"})),
            base("session.delta", 6, json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "b", "text": "{\"file_path\":\"b\"}"})),
            base("session.delta", 7, json!({"kind": "tool_result", "toolUseId": "b", "text": "y"})),
            base("session.delta", 8, json!({"kind": "text", "text": "Done."})),
            base("session.done", 9, json!({"result": {"status": "completed", "durationMs": 4_250}})),
            base("session.end", 10, json!({"exitCode": 0, "sawResult": true})),
        ];
        let rows = rows(&events, &opened(&["u1"]));
        assert_eq!(
            shown(&rows),
            [
                ("person", "go"),
                ("fold", "Worked for 4.3s"),
                ("group", "Thinking"),
                ("agent", "Hm."),
                ("group", "Read 2 files"),
                ("agent", "Done.")
            ]
        );
        assert!(matches!(rows[2].kind, RowKind::Group { glyph: Glyph::Bot, .. }));
        assert!(matches!(rows[4].kind, RowKind::Group { glyph: Glyph::Eye, .. }));
        assert_eq!(rows[2].key, "group:u1:w@2");
    }

    #[test]
    fn an_interrupted_turn_never_folds_and_says_you_stopped() {
        let mut events = turn(false);
        events.push(base("session.done", 9, json!({"result": {"status": "interrupted", "durationMs": 4_200}})));
        events.push(base("session.end", 10, json!({"exitCode": 0, "sawResult": true})));
        let rows = rows(&events, &HashSet::new());
        assert_eq!(
            shown(&rows),
            [
                ("person", "fix it"),
                ("agent", "Looking."),
                ("group", "Ran 1 command"),
                ("agent", "Done here."),
                ("stopped", "You stopped after 4.2s"),
                ("footer", "interrupted"),
            ]
        );
        assert_eq!(rows[5].kind, RowKind::Footer { word: FooterWord::Interrupted });
        assert_eq!(rows[4].key, "stop:u1");
    }

    #[test]
    fn a_stop_with_no_clock_says_you_stopped_this_response() {
        let events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base("session.delta", 2, json!({"kind": "text", "text": "Hi"})),
            base("session.done", 3, json!({"result": {"status": "interrupted"}})),
            base("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
        ];
        assert_eq!(shown(&rows(&events, &HashSet::new()))[2], ("stopped", "You stopped this response"));
    }

    #[test]
    fn a_failed_turn_keeps_its_error_through_the_fold() {
        let mut events = turn(false);
        events.push(base("session.done", 9, json!({"result": {"status": "failed", "error": "API down", "durationMs": 2_000}})));
        events.push(base("session.end", 10, json!({"exitCode": 1, "sawResult": true})));
        let rows = rows(&events, &HashSet::new());
        assert_eq!(shown(&rows), [("person", "fix it"), ("fold", "Worked for 2.0s"), ("agent", "Done here."), ("error", "API down")]);
    }

    #[test]
    fn a_failure_without_words_says_failed_under_the_transcript() {
        let mut events = turn(false);
        events.push(base("session.done", 9, json!({"result": {"status": "failed"}})));
        events.push(base("session.end", 10, json!({"exitCode": 1, "sawResult": true})));
        let rows = rows(&events, &HashSet::new());
        assert_eq!(rows.last().unwrap().kind, RowKind::Footer { word: FooterWord::Failed });
        assert!(!rows.iter().any(|r| r.kind == RowKind::Error));
    }

    #[test]
    fn a_session_that_exits_without_a_result_fails_its_turn() {
        let mut events = turn(false);
        events.push(base("session.end", 9, json!({"exitCode": 137, "sawResult": false})));
        let rows = rows(&events, &HashSet::new());
        assert_eq!(
            rows.last().map(|r| (name(&r.kind), r.text.as_str())),
            Some(("error", "Session exited without a result (exit code 137)"))
        );
    }

    #[test]
    fn a_usage_limit_says_limit_reached() {
        let events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base("session.done", 2, json!({"result": {"status": "failed", "error": "limit", "limit": {}}})),
            base("session.end", 3, json!({"exitCode": 0, "sawResult": true})),
        ];
        assert_eq!(shown(&rows(&events, &HashSet::new())), [("person", "go"), ("footer", "limit reached")]);
    }

    #[test]
    fn notices_and_compactions_stand_alone_through_the_fold() {
        let mut events = turn(false);
        events.insert(2, base("session.notice", 20, json!({"text": "machine woke up"})));
        events.insert(3, base("session.compacted", 21, json!({"before": 180_000, "after": 42_100})));
        events.push(base("session.done", 9, json!({"result": {"status": "completed", "durationMs": 1_000}})));
        events.push(base("session.end", 10, json!({"exitCode": 0, "sawResult": true})));
        let rows = rows(&events, &HashSet::new());
        assert_eq!(
            shown(&rows),
            [
                ("person", "fix it"),
                ("fold", "Worked for 1.0s"),
                ("work", "Machine woke up"),
                ("work", "Compacted the context, 180k to 42.1k tokens"),
                ("agent", "Done here."),
            ]
        );
        assert_eq!(rows[2].kind, RowKind::Work { tone: Tone::Notice, glyph: Glyph::Info });
        assert_eq!(rows[3].kind, RowKind::Work { tone: Tone::Compaction, glyph: Glyph::Minimize });
    }

    #[test]
    fn keys_hold_across_a_new_event() {
        let before = rows(&turn(false), &HashSet::new());
        let mut events = turn(false);
        events.push(base("session.delta", 9, json!({"kind": "text", "text": " More.", "messageId": "m2"})));
        let after = rows(&events, &HashSet::new());
        let keys = |rows: &[Row]| rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>();
        assert_eq!(keys(&before), keys(&after));
        assert_eq!(after.iter().find(|r| r.key == "u1:m@7").map(|r| r.text.as_str()), Some("Done here. More."));
    }

    #[test]
    fn a_turn_whose_harness_reports_a_negative_duration_still_folds() {
        let mut events = turn(false);
        events.push(base("session.done", 9, json!({"result": {"status": "completed", "durationMs": -5.0, "waitedMs": 2.0}})));
        events.push(base("session.end", 10, json!({"exitCode": 0, "sawResult": true})));
        assert_eq!(name(&rows(&events, &HashSet::new())[1].kind), "fold");
    }

    #[test]
    fn durations_read_as_the_web_app_writes_them() {
        assert_eq!(duration(400.0), "400ms");
        assert_eq!(duration(4_250.0), "4.3s");
        assert_eq!(duration(12_400.0), "12s");
        assert_eq!(duration(125_000.0), "2m 5s");
        assert_eq!(tokens(42_100.0), "42.1k");
        assert_eq!(tokens(4_270.0), "4.27k");
        assert_eq!(tokens(180_000.0), "180k");
    }

    /// A thread of `turns` turns with tools, thinking, streamed replies, a failure, an interruption, a notice, a steer
    /// and a turn the next start cuts off, stamped so every clock shows.
    fn thread(turns: usize) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        let mut pos = 0u64;
        let mut at = 1_000.0;
        let mut push = |kind: &str, turn: usize, extra: serde_json::Value| {
            pos += 1;
            at += 250.0;
            let mut v = json!({"type": kind, "workspaceId": "w", "sessionId": "s", "turnId": format!("u{turn}"), "pos": pos, "at": at});
            v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            events.push(event(v));
        };
        for n in 0..turns {
            push("session.start", n, json!({"prompt": format!("ask {n}")}));
            push("session.delta", n, json!({"kind": "thinking", "text": "First "}));
            push("session.delta", n, json!({"kind": "thinking", "text": "thought"}));
            for word in ["Let ", "me ", "look."] {
                push("session.delta", n, json!({"kind": "text", "text": word, "messageId": format!("a{n}")}));
            }
            push(
                "session.delta",
                n,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": format!("b{n}"), "text": "{\"command\":\"cargo test\"}"}),
            );
            if n % 5 == 2 {
                push("session.steer", n, json!({"prompt": "also check docs"}));
                push("session.permission", n, json!({"askId": format!("k{n}"), "toolName": "Bash", "input": "{}", "options": []}));
                push("session.permission.closed", n, json!({"askId": format!("k{n}"), "outcome": "allowed"}));
            }
            push("session.delta", n, json!({"kind": "tool_result", "toolUseId": format!("b{n}"), "text": "ok\n", "exitCode": 0}));
            push(
                "session.delta",
                n,
                json!({"kind": "tool_use", "toolName": "Read", "toolUseId": format!("r{n}"), "text": "{\"file_path\":\"src/a.rs\"}"}),
            );
            push("session.delta", n, json!({"kind": "tool_result", "toolUseId": format!("r{n}"), "text": "fn a() {}"}));
            if n % 4 == 1 {
                push("session.notice", n, json!({"text": "the machine slept"}));
            }
            push(
                "session.delta",
                n,
                json!({"kind": "tool_use", "toolName": "Edit", "toolUseId": format!("e{n}"), "text": "{\"file_path\":\"src/a.rs\"}"}),
            );
            push(
                "session.delta",
                n,
                json!({"kind": "tool_result", "toolUseId": format!("e{n}"), "text": "done", "patch": [{"path": "src/a.rs", "hunks": [
                    {"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1, "lines": ["-a", "+b"]}
                ]}]}),
            );
            for word in ["All ", "done ", "now."] {
                push("session.delta", n, json!({"kind": "text", "text": word, "messageId": format!("z{n}")}));
            }
            if n % 6 == 4 {
                continue;
            }
            let result = match n % 7 {
                3 => json!({"status": "failed", "error": "API down", "durationMs": 3_000}),
                5 => json!({"status": "interrupted", "durationMs": 2_000}),
                6 => json!({"status": "failed"}),
                _ => json!({"status": "completed", "durationMs": 9_000, "waitedMs": 1_000}),
            };
            push("session.done", n, json!({"result": result}));
            if n % 3 != 1 {
                push("session.end", n, json!({"exitCode": 0, "sawResult": true}));
            }
        }
        events
    }

    #[test]
    fn a_live_event_folds_to_what_a_load_of_the_same_events_draws() {
        let events = thread(12);
        assert!(events.len() >= 200, "{} events", events.len());
        let ids = (0..12).map(|n| format!("u{n}")).chain(["group:tool:u7:b7".to_owned(), "u8:call:b8".to_owned()]);
        for opened in [HashSet::new(), ids.collect::<HashSet<String>>()] {
            let mut live = Transcript { opened: opened.clone(), ..Transcript::default() };
            for (n, e) in events.iter().enumerate() {
                let before = live.rows.clone();
                let changed = live.push(e.clone());
                let full = rows(&events[..=n], &opened);
                assert_eq!(live.rows, full, "after event {n}");
                assert_eq!(changed, splice(&before, &full), "splice after event {n}");
            }
            assert!(live.rows.iter().any(|r| r.kind == RowKind::Stopped));
            assert!(live.rows.iter().any(|r| r.kind == RowKind::Error && r.text == "Session restarted before it finished"));
        }
    }

    #[test]
    fn a_live_event_folds_only_the_latest_turn() {
        let mut events = thread(900);
        let last = events.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
        events.truncate(last + 6);
        let mut t = Transcript::new();
        t.load(events.clone());
        let load = std::mem::take(&mut t.cost);
        let delta = base("session.delta", 999_999, json!({"kind": "text", "text": " more", "messageId": "a899", "turnId": "u899"}));
        t.push(delta.clone());
        assert!(t.cost * 100 < load, "a live event cost {} against a load's {load}", t.cost);
        events.push(delta);
        assert_eq!(t.rows, rows(&events, &HashSet::new()));
    }

    #[test]
    fn a_streamed_delta_changes_only_its_reply_row() {
        let mut t = Transcript::new();
        t.load(thread(3));
        t.push(base("session.start", 900, json!({"prompt": "next", "turnId": "u9"})));
        t.push(base("session.delta", 901, json!({"kind": "text", "text": "Hel", "turnId": "u9"})));
        let reply = t.rows.iter().position(|r| r.key == "u9:m@901").unwrap();
        let changed = t.push(base("session.delta", 902, json!({"kind": "text", "text": "lo", "turnId": "u9"})));
        assert_eq!(changed, Some(Splice { start: reply, removed: 1, inserted: 1 }));
        assert_eq!(t.rows[reply].text, "Hello");
    }
}
