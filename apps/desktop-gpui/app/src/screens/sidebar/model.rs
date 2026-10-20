// SPDX-License-Identifier: AGPL-3.0-only
//! The pure data half of wsp's web sidebar, ported for the GPUI desktop app: session rows folded into threads,
//! each thread's status slot, the sections and trees the sidebar draws, the flat list of rows a virtual list paints,
//! and the marks a move writes. Every rule here is read off the web source named beside it.

mod kinds;
mod list;

use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::screens::app_shell::title::thread_title;

pub use kinds::{
    kind_of, kind_slot, kind_word, recedes, resets_word, resting_age, running_time, slot, subagent_kind, tile_kind, tile_slot, word, Kind,
    Slot, Tone,
};
pub use list::{key_at, Folds, Item, Line, Move, MoveMark, RootPlace, Shape, Tile, Under};

/// The quiet a read thread waits out before it settles, at the record's default `settleAfter: "2h"`.
pub const SETTLE_MS_DEFAULT: f64 = 2.0 * 3_600_000.0;
/// How many tiles the open Settled fold draws at first, and each press of its "n more" row adds.
pub const SETTLED_PAGE: usize = 25;
/// How many rows an open Finished fold under a lead draws at a time.
pub const FINISHED_PAGE: usize = 20;
/// A drop above the top tree takes its key plus this, and one below the last its key less the other.
pub const ABOVE_TOP_MS: f64 = 1.0;
pub const BELOW_LAST_MS: f64 = 60_000.0;
const REKEY_GAP_MS: f64 = 1.0;

/// `SETUP_ROWS` in `settings/add/setup.ts`: the steps a setup counts, in their order, each by the name its row goes by.
pub(crate) const SETUP_ROWS: [(&str, &str); 9] = [
    ("floor", "Base packages"),
    ("agents", "Agents"),
    ("mcp", "MCP servers"),
    ("clis", "CLIs"),
    ("skills", "Skills"),
    ("plugins", "Plugins"),
    ("github", "GitHub"),
    ("folders", "Projects"),
    ("configs", "Other config"),
];

/// `SETUP_STEP_WORDS` in `views/place.ts`: each setup step as a line names it.
const SETUP_STEP_WORDS: [(&str, &str); 12] = [
    ("floor", "the base tools"),
    ("agents", "the agents"),
    ("signins", "the agents' sign-ins"),
    ("clis", "the CLIs"),
    ("skills", "the skills"),
    ("mcp", "the agents' own files and MCP servers"),
    ("plugins", "the plugins"),
    ("configs", "git and the shell"),
    ("folders", "the folders, as projects"),
    ("folderServers", "the projects' own MCP servers"),
    ("github", "the GitHub sign-in"),
    ("context", "what the agents read about this computer"),
];

/// A setup step's words; none for a step the protocol does not name.
pub(crate) fn setup_step_words(step: &str) -> Option<&'static str> {
    SETUP_STEP_WORDS.iter().find(|(s, _)| *s == step).map(|(_, w)| *w)
}

pub(crate) fn capitalised(word: &str) -> String {
    let mut c = word.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Running,
    Completed,
    Interrupted,
    Failed,
    Other(String),
}

impl Status {
    pub fn parse(word: &str) -> Self {
        match word {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "interrupted" => Self::Interrupted,
            "failed" => Self::Failed,
            other => Self::Other(other.to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubState {
    Waiting,
    Starting,
    Running,
    Napping,
    Released,
    Other,
}

impl SubState {
    pub fn parse(word: &str) -> Self {
        match word {
            "waiting" => Self::Waiting,
            "starting" => Self::Starting,
            "running" => Self::Running,
            "napping" => Self::Napping,
            "released" => Self::Released,
            _ => Self::Other,
        }
    }
}

/// A sub-thread's machine as the sidebar reads it: its state, and whether work not pushed holds it (`sub.held`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sub {
    pub state: SubState,
    pub held: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubagentState {
    Running,
    Done,
    Failed,
    Stopped,
}

/// One of an agent's own subagents, as the latest row of its thread carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct Subagent {
    pub id: String,
    pub title: String,
    pub state: SubagentState,
    pub started_at: f64,
    pub ended_at: Option<f64>,
    /// The call that launched it, which its page opens by.
    pub call: Option<String>,
    pub model: Option<String>,
    /// What its agent was asked.
    pub asked: Option<String>,
    pub failure: Option<String>,
    pub last_line: Option<String>,
}

/// The sections a thread's own state files it under; a placement names one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThreadSection {
    NeedsYou,
    Working,
    Done,
    Idle,
}

impl ThreadSection {
    pub fn id(self) -> &'static str {
        match self {
            Self::NeedsYou => "needs-you",
            Self::Working => "working",
            Self::Done => "done",
            Self::Idle => "idle",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        [Self::NeedsYou, Self::Working, Self::Done, Self::Idle].into_iter().find(|s| s.id() == word)
    }
}

/// Where the person dragged a thread, held while it is still in the state it was then.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub name: ThreadSection,
    pub while_state: String,
}

/// One row of the host's `sessions.list`, holding only what the sidebar reads.
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub id: String,
    pub workspace_id: String,
    pub harness: String,
    pub status: Status,
    pub claude_session_id: Option<String>,
    /// The agent refused the turn (`refusal`), so it did none of the work asked of it.
    pub refused: bool,
    pub thread_id: Option<String>,
    pub parent_thread_id: Option<String>,
    pub root_thread_id: Option<String>,
    pub attempt: Option<String>,
    pub prompt: Option<String>,
    pub harness_title: Option<String>,
    pub started_at: Option<f64>,
    pub ended_at: Option<f64>,
    /// `asking` is present.
    pub asking: bool,
    /// `waitingOn` is present.
    pub waiting_on: bool,
    /// `capped` is present.
    pub capped: bool,
    pub setup_refusal: Option<String>,
    pub sub: Option<Sub>,
    pub disk_kept_until: Option<f64>,
    /// `limit` is present.
    pub limit: bool,
    pub resume_at: Option<f64>,
    pub cost_usd: Option<f64>,
    pub last_line: Option<String>,
    pub failure: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub computer_name: Option<String>,
    pub subagents: Vec<Subagent>,
    pub read_at: Option<f64>,
    pub settled_at: Option<f64>,
    pub pinned_at: Option<f64>,
    pub order: Option<f64>,
    pub folded_at: Option<f64>,
    pub snoozed_until: Option<f64>,
    pub woke_at: Option<f64>,
    pub section: Option<Placement>,
    pub replaces: Option<String>,
    pub replaced_by: Option<String>,
    pub cwd: Option<String>,
    pub rewound_at: Option<f64>,
    /// The title of the thread this one was forked from.
    pub forked_from: Option<String>,
    /// What the agent asks, where it asks.
    pub asking_text: Option<String>,
    pub model: Option<String>,
    /// The thread it was forked from, by its id.
    pub forked_from_thread: Option<String>,
    /// The records the thread view reads whole, kept only where the host sent one: the open question this thread waits
    /// behind (`waitingOn`), the threads at once that hold it (`capped`), and a sub-thread's machine (`sub`).
    pub waiting_on_record: Option<Value>,
    pub capped_record: Option<Value>,
    pub sub_record: Option<Value>,
}

impl Row {
    /// A row read off the host's record, the few records the thread view reads whole moved out of it.
    pub fn owned(mut v: Value) -> Option<Row> {
        let row = Row::of(&v)?;
        let mut take = |k: &str| v.get_mut(k).map(Value::take).filter(|x| !x.is_null());
        Some(Row { waiting_on_record: take("waitingOn"), capped_record: take("capped"), sub_record: take("sub"), ..row })
    }

    /// The row as the host would send it, for the fields the thread header reads.
    pub fn as_sent(&self) -> Value {
        let mut v = json!({
            "id": self.id, "workspaceId": self.workspace_id, "harness": self.harness, "threadId": self.thread_id,
            "harnessTitle": self.harness_title, "prompt": self.prompt, "asking": self.asking_text, "waitingOn": self.waiting_on_record,
            "startedAt": self.started_at, "endedAt": self.ended_at, "computerName": self.computer_name,
        });
        if self.project_id.is_some() || self.project_name.is_some() {
            v["project"] = json!({ "id": self.project_id, "name": self.project_name });
        }
        v
    }

    /// A `SessionView` as the host sent it, read by the protocol's own names.
    pub fn of(v: &Value) -> Option<Row> {
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let n = |v: &Value, k: &str| v.get(k).and_then(Value::as_f64);
        let present = |k: &str| v.get(k).is_some_and(|x| !x.is_null());
        Some(Row {
            id: s(v, "id")?,
            workspace_id: s(v, "workspaceId")?,
            harness: s(v, "harness").unwrap_or_default(),
            status: Status::parse(v.get("status").and_then(Value::as_str).unwrap_or("completed")),
            claude_session_id: s(v, "claudeSessionId"),
            refused: present("refusal"),
            thread_id: s(v, "threadId"),
            parent_thread_id: s(v, "parentThreadId"),
            root_thread_id: s(v, "rootThreadId"),
            attempt: s(v, "attempt"),
            prompt: s(v, "prompt"),
            harness_title: s(v, "harnessTitle"),
            started_at: n(v, "startedAt"),
            ended_at: n(v, "endedAt"),
            asking: present("asking"),
            waiting_on: present("waitingOn"),
            capped: present("capped"),
            setup_refusal: s(v, "setupRefusal"),
            sub: v.get("sub").filter(|x| !x.is_null()).map(|sub| Sub {
                state: SubState::parse(sub.get("state").and_then(Value::as_str).unwrap_or_default()),
                held: sub.get("held").is_some_and(|h| !h.is_null()),
            }),
            disk_kept_until: n(v, "diskKeptUntil"),
            limit: present("limit"),
            resume_at: n(v, "resumeAt"),
            cost_usd: n(v, "costUsd"),
            last_line: s(v, "lastLine"),
            failure: s(v, "failure"),
            project_id: v.get("project").and_then(|p| s(p, "id")),
            project_name: v.get("project").and_then(|p| s(p, "name")),
            computer_name: s(v, "computerName"),
            subagents: v.get("subagents").and_then(Value::as_array).map_or_else(Vec::new, |subs| {
                subs.iter()
                    .filter_map(|x| {
                        Some(Subagent {
                            id: s(x, "id")?,
                            title: s(x, "title").unwrap_or_default(),
                            state: match x.get("state").and_then(Value::as_str) {
                                Some("running") => SubagentState::Running,
                                Some("done") => SubagentState::Done,
                                Some("failed") => SubagentState::Failed,
                                _ => SubagentState::Stopped,
                            },
                            started_at: n(x, "startedAt").unwrap_or(0.0),
                            ended_at: n(x, "endedAt"),
                            call: s(x, "parentToolUseId"),
                            model: s(x, "model"),
                            asked: s(x, "asked"),
                            failure: s(x, "failure"),
                            last_line: s(x, "lastLine"),
                        })
                    })
                    .collect()
            }),
            read_at: n(v, "readAt"),
            settled_at: n(v, "settledAt"),
            pinned_at: n(v, "pinnedAt"),
            order: n(v, "order"),
            folded_at: n(v, "foldedAt"),
            snoozed_until: n(v, "snoozedUntil"),
            woke_at: n(v, "wokeAt"),
            section: v.get("section").filter(|x| !x.is_null()).and_then(|p| {
                Some(Placement {
                    name: ThreadSection::parse(p.get("name").and_then(Value::as_str)?)?,
                    while_state: s(p, "whileState").unwrap_or_default(),
                })
            }),
            replaces: s(v, "replaces"),
            replaced_by: s(v, "replacedBy"),
            cwd: s(v, "cwd"),
            rewound_at: n(v, "rewoundAt"),
            forked_from: v.get("forkedFrom").and_then(|f| s(f, "title")),
            asking_text: s(v, "asking"),
            model: s(v, "model"),
            forked_from_thread: v.get("forkedFrom").and_then(|f| s(f, "threadId")),
            waiting_on_record: None,
            capped_record: None,
            sub_record: None,
        })
    }
}

/// One sidebar thread: the rows sharing a thread id, in the state and stamps of the latest and with the opening
/// row's provenance.
#[derive(Clone, Debug)]
pub struct Thread {
    /// The fold key: the runtime's thread id, else the lone row's id. What every mark names.
    pub key: String,
    pub thread_id: Option<String>,
    /// The latest row's id, what a stop interrupts.
    pub session_id: String,
    pub workspace_id: String,
    pub harness: String,
    pub title: String,
    pub status: Status,
    /// A turn of it did work (session.ts threadRan), which a forget refuses.
    pub ran: bool,
    pub parent: Option<String>,
    pub root: Option<String>,
    pub attempt: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub computer: Option<String>,
    pub started_at: Option<f64>,
    pub ended_at: Option<f64>,
    /// It asks, or is stopped behind a thread that asks: the web's `asking !== null`.
    pub asks: bool,
    pub capped: bool,
    pub setup_refusal: Option<String>,
    pub sub: Option<Sub>,
    pub disk_kept_until: Option<f64>,
    pub limit: bool,
    pub resume_at: Option<f64>,
    pub cost_usd: Option<f64>,
    pub last_line: Option<String>,
    pub failure: Option<String>,
    pub read_at: Option<f64>,
    pub settled_at: Option<f64>,
    pub pinned_at: Option<f64>,
    pub order: Option<f64>,
    pub folded_at: Option<f64>,
    pub snoozed_until: Option<f64>,
    pub woke_at: Option<f64>,
    pub section: Option<Placement>,
    pub replaces: Option<String>,
    pub replaced_by: Option<String>,
    pub subagents: Vec<Subagent>,
    pub unread: bool,
    pub needs_you: bool,
    /// The latest turn's folder and the harness's own session id, which the terminal resumes.
    pub cwd: Option<String>,
    pub claude_session_id: Option<String>,
    pub rewound_at: Option<f64>,
    pub forked_from: Option<String>,
    pub asking_text: Option<String>,
    pub model: Option<String>,
}

impl Thread {
    fn running(&self) -> bool {
        self.status == Status::Running
    }

    fn held(&self) -> bool {
        self.sub.as_ref().is_some_and(|s| s.held)
    }
}

/// Folds the session rows into threads, in the order each thread's first row appears (session.ts foldThreads).
pub fn fold_threads(rows: &[Row]) -> Vec<Thread> {
    let mut at: HashMap<&str, usize> = HashMap::with_capacity(rows.len());
    let mut groups: Vec<(usize, usize, Option<f64>)> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let key = row.thread_id.as_deref().unwrap_or(&row.id);
        if let Some(&g) = at.get(key) {
            let group = &mut groups[g];
            group.1 = i;
            if let Some(cost) = row.cost_usd {
                group.2 = Some(group.2.unwrap_or(0.0) + cost);
            }
        } else {
            at.insert(key, groups.len());
            groups.push((i, i, row.cost_usd));
        }
    }
    let ran = |row: &Row| row.status == Status::Running || (row.claude_session_id.is_some() && !row.refused);
    let ran_in: HashMap<&str, bool> = rows.iter().fold(HashMap::new(), |mut m, row| {
        *m.entry(row.thread_id.as_deref().unwrap_or(&row.id)).or_default() |= ran(row);
        m
    });
    groups
        .into_iter()
        .map(|(first, latest, cost)| {
            let mut thread = thread_of(&rows[first], &rows[latest], cost);
            thread.ran = ran_in.get(thread.key.as_str()).copied().unwrap_or(false);
            thread
        })
        .collect()
}

fn thread_of(first: &Row, latest: &Row, cost_usd: Option<f64>) -> Thread {
    let title = thread_title(latest.harness_title.as_deref(), first.prompt.as_deref())
        .unwrap_or_else(|| first.claude_session_id.clone().unwrap_or_else(|| first.id.clone()));
    let mut thread = Thread {
        key: first.thread_id.clone().unwrap_or_else(|| first.id.clone()),
        thread_id: first.thread_id.clone(),
        session_id: latest.id.clone(),
        workspace_id: first.workspace_id.clone(),
        harness: first.harness.clone(),
        title,
        status: latest.status.clone(),
        ran: false,
        parent: first.parent_thread_id.clone(),
        root: first.root_thread_id.clone(),
        attempt: first.attempt.clone(),
        project_id: first.project_id.clone(),
        project_name: first.project_name.clone(),
        computer: latest.computer_name.clone().or_else(|| first.computer_name.clone()),
        started_at: latest.started_at,
        ended_at: latest.ended_at,
        asks: latest.asking || latest.waiting_on,
        capped: latest.capped,
        setup_refusal: latest.setup_refusal.clone(),
        sub: latest.sub.clone(),
        disk_kept_until: latest.disk_kept_until,
        limit: latest.limit,
        resume_at: latest.resume_at,
        cost_usd,
        last_line: latest.last_line.clone(),
        failure: latest.failure.clone(),
        read_at: latest.read_at,
        settled_at: latest.settled_at,
        pinned_at: latest.pinned_at,
        order: latest.order,
        folded_at: latest.folded_at,
        snoozed_until: latest.snoozed_until,
        woke_at: latest.woke_at,
        section: latest.section.clone(),
        replaces: latest.replaces.clone(),
        replaced_by: latest.replaced_by.clone(),
        subagents: latest.subagents.clone(),
        unread: false,
        needs_you: false,
        cwd: latest.cwd.clone(),
        claude_session_id: latest.claude_session_id.clone(),
        rewound_at: latest.rewound_at,
        forked_from: first.forked_from.clone(),
        asking_text: latest.asking_text.clone(),
        model: latest.model.clone(),
    };
    thread.unread = thread_unread(&thread);
    thread.needs_you = thread_needs_you(&thread);
    thread
}

/// The latest of its end and its snooze's end no window has shown since; nothing while it runs.
fn unseen_at(t: &Thread, ended: Option<f64>) -> Option<f64> {
    if t.running() {
        return None;
    }
    [ended, t.woke_at].into_iter().flatten().filter(|&at| t.read_at.is_none_or(|read| read < at)).reduce(f64::max)
}

/// `startFailed`: gone with no machine ever named, a create refused before any machine stood.
pub fn start_failed(w: &Value) -> bool {
    w.get("phase").and_then(Value::as_str) == Some("gone") && w.get("machineId").and_then(Value::as_str) == Some("")
}

/// The latest turn finished or was stopped, or a snooze ended, and no window has shown it since. A failure is no finish.
pub fn thread_unread(t: &Thread) -> bool {
    matches!(t.status, Status::Completed | Status::Interrupted) && unseen_at(t, t.ended_at).is_some()
}

/// It asks, holds work not pushed, or an end nobody has seen sits there; a stop's own end asks nothing.
pub fn thread_needs_you(t: &Thread) -> bool {
    if t.asks || (t.held() && !t.running()) {
        return true;
    }
    unseen_at(t, if t.status == Status::Interrupted { None } else { t.ended_at }).is_some()
}

/// The section a thread's own state files it under (Sidebar.logic.ts threadSection).
pub fn thread_section(t: &Thread) -> ThreadSection {
    if t.asks || (t.status == Status::Failed && t.resume_at.is_none()) || (t.held() && !t.running()) {
        ThreadSection::NeedsYou
    } else if t.running() {
        ThreadSection::Working
    } else if t.unread {
        ThreadSection::Done
    } else {
        ThreadSection::Idle
    }
}

/// When the settle rule folds the thread: the hand's stamp, or the end of the quiet it waits out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettleMoment {
    pub ms: f64,
    pub by_hand: bool,
}

/// thread-state.ts settleMoment: a hand settle with nothing since, else seen, not failed and not held, `settle_ms`
/// after the later of its last activity and that showing. `settle_ms` None is never.
pub fn settle_moment(t: &Thread, settle_ms: Option<f64>, held: bool) -> Option<SettleMoment> {
    if t.asks || t.running() {
        return None;
    }
    let last = match (t.started_at, t.ended_at) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    if let Some(at) = t.settled_at.filter(|&at| last.is_none_or(|l| at >= l)) {
        return Some(SettleMoment { ms: at, by_hand: true });
    }
    let seen = t.ended_at.is_none_or(|end| t.read_at.is_some_and(|read| read >= end));
    let (Some(quiet), Some(last)) = (settle_ms, last) else { return None };
    if held || t.status == Status::Failed || !seen {
        return None;
    }
    Some(SettleMoment { ms: last.max(t.read_at.unwrap_or(last)) + quiet, by_hand: false })
}

pub fn thread_settled(t: &Thread, now_ms: f64, settle_ms: Option<f64>, held: bool) -> bool {
    settle_moment(t, settle_ms, held).is_some_and(|m| m.by_hand || now_ms >= m.ms)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Pinned,
    NeedsYou,
    Threads,
    Settled,
}

impl Section {
    /// The id the web keeps in `wsp:sidebar-folded`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::NeedsYou => "needs-you",
            Self::Threads => "threads",
            Self::Settled => "settled",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    /// How long a read thread sits quiet before it settles; None is never.
    pub settle_ms: Option<f64>,
    /// The project the list is narrowed to: only its roots are listed, children kept wherever they run.
    pub project: Option<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { settle_ms: Some(SETTLE_MS_DEFAULT), project: None }
    }
}

/// One tile with the tiles its agents opened under it. A group head (one send to several models) holds no thread.
#[derive(Clone, Debug)]
pub struct Tree {
    /// The tile's id: the thread's fold key, or `attempt:<id>` for a group head.
    pub key: String,
    pub thread: Option<usize>,
    pub started_at: Option<f64>,
    pub children: Vec<Tree>,
    /// On a snoozed root while threads of it run: how many.
    pub snoozed_working: Option<usize>,
    /// On a tile of the Needs you inbox: the threads from its tree's top down to the one it hangs under.
    pub inbox: Option<Vec<usize>>,
}

impl Tree {
    fn leaf(key: String, thread: usize, started_at: Option<f64>) -> Self {
        Self { key, thread: Some(thread), started_at, children: Vec::new(), snoozed_working: None, inbox: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Live,
    Finished,
    Settled,
}

#[derive(Clone, Copy)]
pub(crate) enum Lead<'t> {
    Thread(&'t Tree),
    Sub(usize, usize),
}

/// The reading every tree rule shares: the threads, the clock sample and the settle quiet.
#[derive(Clone, Copy)]
pub(crate) struct Ctx<'a> {
    pub(crate) all: &'a [Thread],
    pub(crate) now: f64,
    pub(crate) settle: Option<f64>,
}

impl<'a> Ctx<'a> {
    fn settled(&self, t: &Thread, held: bool) -> bool {
        thread_settled(t, self.now, self.settle, held)
    }

    pub(crate) fn lead_nodes<'t>(&self, node: &'t Tree) -> Vec<Lead<'t>> {
        let subs = node.thread.map_or(0, |i| self.all[i].subagents.len());
        let mut out = Vec::with_capacity(node.children.len() + subs);
        out.extend(node.children.iter().map(Lead::Thread));
        out.extend(node.thread.into_iter().flat_map(|i| (0..subs).map(move |s| Lead::Sub(i, s))));
        out
    }

    /// leadTree.ts ownPart.
    pub(crate) fn own_part(&self, l: Lead) -> Part {
        match l {
            Lead::Sub(i, s) => {
                let (of, sub) = (&self.all[i], &self.all[i].subagents[s]);
                if sub.state == SubagentState::Running {
                    return Part::Live;
                }
                let in_turn = of.running() && sub.started_at >= of.started_at.unwrap_or(0.0);
                if in_turn && !self.settled(of, false) {
                    Part::Finished
                } else {
                    Part::Settled
                }
            }
            Lead::Thread(node) => {
                let Some(i) = node.thread else { return Part::Live };
                let t = &self.all[i];
                if self.settled(t, false) {
                    Part::Settled
                } else if t.asks || t.running() || t.status == Status::Failed || t.held() {
                    Part::Live
                } else {
                    Part::Finished
                }
            }
        }
    }

    /// How pressing a child is on its own: asks or a limit, failed, works, waits, then quiet.
    pub(crate) fn own_rank(&self, l: Lead, part: Part) -> u8 {
        if part != Part::Live {
            return 4;
        }
        let Lead::Thread(Tree { thread: Some(i), .. }) = l else { return 2 };
        let t = &self.all[*i];
        if t.asks || (t.status == Status::Failed && t.limit) {
            0
        } else if t.status == Status::Failed || t.held() {
            1
        } else if t.capped {
            3
        } else {
            2
        }
    }

    fn tree_rank(&self, l: Lead) -> u8 {
        let mut best = self.own_rank(l, self.own_part(l));
        if let Lead::Thread(node) = l {
            for kid in self.lead_nodes(node) {
                if best == 0 {
                    break;
                }
                best = best.min(self.tree_rank(kid));
            }
        }
        best
    }

    fn part_of(&self, l: Lead) -> Part {
        if self.tree_rank(l) < 4 {
            Part::Live
        } else {
            self.own_part(l)
        }
    }

    fn start_of(&self, l: Lead) -> f64 {
        match l {
            Lead::Sub(i, s) => self.all[i].subagents[s].started_at,
            Lead::Thread(n) => n.thread.and_then(|i| self.all[i].started_at).unwrap_or(0.0),
        }
    }

    /// leadTree.ts childParts: live by the most pressing rank under it then newest start, finished by newest end,
    /// settled by newest settle.
    pub(crate) fn child_parts<'t>(&self, nodes: Vec<Lead<'t>>) -> [Vec<Lead<'t>>; 3] {
        let mut live: Vec<(Lead, u8)> = Vec::new();
        let (mut finished, mut settled) = (Vec::new(), Vec::new());
        for l in nodes {
            let rank = self.tree_rank(l);
            match if rank < 4 { Part::Live } else { self.own_part(l) } {
                Part::Live => live.push((l, rank)),
                Part::Finished => finished.push(l),
                Part::Settled => settled.push(l),
            }
        }
        live.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| self.start_of(b.0).total_cmp(&self.start_of(a.0))));
        let end = |l: &Lead| match *l {
            Lead::Sub(i, s) => self.all[i].subagents[s].ended_at.unwrap_or(0.0),
            Lead::Thread(n) => n.thread.and_then(|i| self.all[i].ended_at).unwrap_or(0.0),
        };
        let settle = |l: &Lead| match *l {
            Lead::Sub(i, s) => self.all[i].subagents[s].ended_at.unwrap_or(0.0),
            Lead::Thread(n) => n.thread.and_then(|i| self.all[i].settled_at.or(self.all[i].ended_at)).unwrap_or(0.0),
        };
        finished.sort_by(|a, b| end(b).total_cmp(&end(a)));
        settled.sort_by(|a, b| settle(b).total_cmp(&settle(a)));
        [live.into_iter().map(|(l, _)| l).collect(), finished, settled]
    }

    /// A child thread, settled or not, or a subagent that is not settled.
    fn draws_under(&self, node: &Tree) -> bool {
        self.lead_nodes(node).into_iter().any(|l| matches!(l, Lead::Thread(_)) || self.part_of(l) != Part::Settled)
    }

    /// How many rows a folded tile would draw under it: its live threads and subagents and theirs, and each
    /// Finished fold as its one row (LeadTree.tsx rowsUnder).
    fn rows_under(&self, node: &Tree) -> usize {
        let [live, finished, settled] = self.child_parts(self.lead_nodes(node));
        let fold = usize::from(!finished.is_empty() || settled.iter().any(|l| matches!(l, Lead::Thread(_))));
        live.iter().map(|l| 1 + if let Lead::Thread(n) = l { self.rows_under(n) } else { 0 }).sum::<usize>() + fold
    }

    /// How many tiles a tree draws, its top among them, none under a folded tile (threadTree.ts drawnCount).
    fn drawn_count(&self, node: &Tree) -> usize {
        let Some(i) = node.thread else { return node.children.iter().map(|c| self.drawn_count(c)).sum() };
        if self.all[i].folded_at.is_some() {
            return 1;
        }
        1 + node.children.iter().filter(|c| self.part_of(Lead::Thread(c)) == Part::Live).map(|c| self.drawn_count(c)).sum::<usize>()
    }

    fn tree_settled_ms(&self, node: &Tree) -> f64 {
        let own = node.thread.and_then(|i| settle_moment(&self.all[i], self.settle, false)).map_or(0.0, |m| m.ms);
        node.children.iter().map(|c| self.tree_settled_ms(c)).fold(own, f64::max)
    }

    fn tree_section(&self, node: &Tree) -> ThreadSection {
        let own = node.thread.map_or(ThreadSection::Idle, |i| thread_section(&self.all[i]));
        node.children.iter().map(|c| self.tree_section(c)).fold(own, |a, b| if (b as u8) < (a as u8) { b } else { a })
    }

    /// The state a placement holds while: the tree's section and its root's latest turn.
    fn placement_key(&self, node: &Tree) -> String {
        let session = node.thread.map_or(node.key.as_str(), |i| self.all[i].session_id.as_str());
        format!("{}:{session}", self.tree_section(node).id())
    }

    fn section_of(&self, node: &Tree, own: ThreadSection) -> ThreadSection {
        match node.thread.and_then(|i| self.all[i].section.as_ref()) {
            Some(placed) if placed.while_state == self.placement_key(node) => placed.name,
            _ => own,
        }
    }

    fn root_threads<'t>(&self, node: &'t Tree) -> impl Iterator<Item = usize> + 't {
        let grouped = node.thread.is_none();
        node.thread.into_iter().chain(node.children.iter().filter(move |_| grouped).filter_map(|c| c.thread))
    }

    fn pin_key(&self, node: &Tree) -> Option<f64> {
        self.root_threads(node).filter_map(|i| self.all[i].pinned_at).reduce(f64::max)
    }

    fn list_key(&self, node: &Tree) -> f64 {
        let start = |at: Option<f64>| at.unwrap_or(0.0);
        self.root_threads(node)
            .map(|i| self.all[i].order.unwrap_or(start(self.all[i].started_at)))
            .reduce(f64::max)
            .unwrap_or(start(node.started_at))
    }

    fn key_in(&self, section: Section, node: &Tree) -> f64 {
        if section == Section::Pinned {
            self.pin_key(node).unwrap_or(0.0)
        } else {
            self.list_key(node)
        }
    }

    fn every_tile(&self, node: &Tree, test: &impl Fn(&Thread) -> bool) -> bool {
        node.thread.is_none_or(|i| test(&self.all[i])) && node.children.iter().all(|c| self.every_tile(c, test))
    }

    fn any_tile(&self, node: &Tree, test: &impl Fn(&Thread) -> bool) -> bool {
        node.thread.is_some_and(|i| test(&self.all[i])) || node.children.iter().any(|c| self.any_tile(c, test))
    }

    fn working_in(&self, node: &Tree) -> usize {
        usize::from(node.thread.is_some_and(|i| self.all[i].running())) + node.children.iter().map(|c| self.working_in(c)).sum::<usize>()
    }

    /// threadTree.ts inboxTiles: every thread that asks or failed and is not armed to resume, not settled, alone.
    fn inbox_tiles(&self, node: &Tree, path: &mut Vec<usize>, open: Option<&str>, out: &mut Vec<Tree>) {
        if let Some(i) = node.thread {
            let t = &self.all[i];
            let own = thread_section(t);
            let section = if path.is_empty() { self.section_of(node, own) } else { own };
            if section == ThreadSection::NeedsYou && !self.settled(t, open == Some(t.key.as_str())) {
                out.push(Tree { inbox: Some(path.clone()), ..Tree::leaf(node.key.clone(), i, node.started_at) });
            }
            path.push(i);
        }
        for child in &node.children {
            self.inbox_tiles(child, path, open, out);
        }
        if node.thread.is_some() {
            path.pop();
        }
    }
}

/// The sidebar's sections and the Settled fold, each a list of root trees in the order drawn.
pub struct View<'a> {
    cx: Ctx<'a>,
    pub pinned: Vec<Tree>,
    pub needs_you: Vec<Tree>,
    pub threads: Vec<Tree>,
    pub settled: Vec<Tree>,
}

/// threadTree.ts sidebarTiles. `held` is the thread open in the centre that the time fold leaves alone: the web
/// passes it only while that thread was opened from the live list.
pub fn sidebar<'a>(all: &'a [Thread], prefs: &Prefs, now_ms: f64, held: Option<&str>) -> View<'a> {
    let cx = Ctx { all, now: now_ms, settle: prefs.settle_ms };
    let mut order: Vec<usize> = (0..all.len()).collect();
    let start = |i: usize| all[i].started_at.unwrap_or(0.0);
    order.sort_by(|&a, &b| start(b).total_cmp(&start(a)).then_with(|| all[a].key.cmp(&all[b].key)));
    let mut roots = forest(all, &order);
    if let Some(project) = &prefs.project {
        roots.retain(|n| n.thread.is_some_and(|i| all[i].project_id.as_deref() == Some(project)));
    }
    let (mut pinned, mut listed, mut settled, mut snoozed) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for node in attempt_groups(all, roots) {
        let snoozed_root = node.thread.is_some_and(|i| all[i].snoozed_until.is_some());
        if snoozed_root && !cx.any_tile(&node, &|t| t.needs_you) {
            let working = cx.working_in(&node);
            if working > 0 {
                snoozed.push(Tree { snoozed_working: Some(working), children: Vec::new(), ..node });
            }
            continue;
        }
        let pins = cx.pin_key(&node).is_some();
        if cx.every_tile(&node, &|t| cx.settled(t, pins || held == Some(t.key.as_str()))) {
            settled.push(node);
        } else if pins {
            pinned.push(node);
        } else {
            listed.push(node);
        }
    }
    let by_key = |section: Section| {
        move |a: &Tree, b: &Tree| cx.key_in(section, b).total_cmp(&cx.key_in(section, a)).then_with(|| a.key.cmp(&b.key))
    };
    pinned.sort_by(by_key(Section::Pinned));
    listed.sort_by(by_key(Section::Threads));

    let mut inbox = Vec::new();
    for node in pinned.iter().chain(&listed) {
        cx.inbox_tiles(node, &mut Vec::new(), held, &mut inbox);
    }
    let tops: HashSet<&str> = inbox.iter().filter(|t| t.inbox.as_ref().is_some_and(Vec::is_empty)).map(|t| t.key.as_str()).collect();
    let alone: Vec<bool> = listed.iter().map(|n| tops.contains(n.key.as_str()) && !cx.draws_under(n)).collect();
    let mut inbox_only: HashMap<String, Tree> = HashMap::new();
    let mut threads = Vec::with_capacity(listed.len() + snoozed.len());
    for (node, alone) in listed.into_iter().zip(alone) {
        if alone {
            inbox_only.insert(node.key.clone(), node);
        } else {
            threads.push(node);
        }
    }
    threads.extend(snoozed);
    // A root with nothing drawn under it stands nowhere else, so its inbox tile is its own tile.
    let needs_you = inbox.into_iter().map(|tile| inbox_only.remove(&tile.key).unwrap_or(tile)).collect();

    let settled_at = |n: &Tree| n.thread.and_then(|i| all[i].ended_at).or(n.started_at).unwrap_or(0.0);
    settled.sort_by(|a, b| settled_at(b).total_cmp(&settled_at(a)).then_with(|| a.key.cmp(&b.key)));
    let mut keyed: Vec<(f64, Tree)> = settled.into_iter().map(|n| (cx.tree_settled_ms(&n), n)).collect();
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
    View { cx, pinned, needs_you, threads, settled: keyed.into_iter().map(|(_, n)| n).collect() }
}

/// Sidebar.logic.ts threadForest over the sorted threads: each thread under its opener where the opener is listed,
/// rows whose parents lead round in a circle kept at the end in their own order.
pub(crate) fn forest(all: &[Thread], sorted: &[usize]) -> Vec<Tree> {
    let index: HashMap<&str, usize> = all.iter().enumerate().map(|(i, t)| (t.key.as_str(), i)).collect();
    let mut spawned: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut under = vec![false; all.len()];
    for &i in sorted {
        if let Some(&p) = all[i].parent.as_deref().and_then(|p| index.get(p)) {
            if p != i {
                spawned.entry(p).or_default().push(i);
                under[i] = true;
            }
        }
    }
    fn node(i: usize, all: &[Thread], spawned: &HashMap<usize, Vec<usize>>, drawn: &mut [bool]) -> Option<Tree> {
        if std::mem::replace(&mut drawn[i], true) {
            return None;
        }
        let children = spawned.get(&i).map_or(&[][..], Vec::as_slice).iter().filter_map(|&c| node(c, all, spawned, drawn)).collect();
        Some(Tree { children, ..Tree::leaf(all[i].key.clone(), i, all[i].started_at) })
    }
    let mut drawn = vec![false; all.len()];
    let mut out: Vec<Tree> = sorted.iter().filter(|&&i| !under[i]).filter_map(|&i| node(i, all, &spawned, &mut drawn)).collect();
    out.extend(sorted.iter().filter_map(|&i| node(i, all, &spawned, &mut drawn)));
    out
}

/// threadTree.ts attemptGroups: the roots one send to several models opened, as one head where the first of them
/// stands, its members in the order they were opened.
fn attempt_groups(all: &[Thread], roots: Vec<Tree>) -> Vec<Tree> {
    fn attempt<'a>(all: &'a [Thread], n: &Tree) -> Option<&'a str> {
        n.thread.and_then(|i| all[i].attempt.as_deref())
    }
    let mut members: HashMap<&str, Vec<usize>> = HashMap::new();
    for (at, node) in roots.iter().enumerate() {
        if let Some(a) = attempt(all, node) {
            members.entry(a).or_default().push(at);
        }
    }
    let grouped: Vec<Option<&str>> = roots.iter().map(|n| attempt(all, n).filter(|a| members[a].len() >= 2)).collect();
    let mut slots: Vec<Option<Tree>> = roots.into_iter().map(Some).collect();
    let mut out = Vec::with_capacity(slots.len());
    for at in 0..slots.len() {
        match grouped[at] {
            None => out.extend(slots[at].take()),
            Some(a) if slots[at].is_some() => {
                let mut opened: Vec<Tree> = members[a].iter().filter_map(|&m| slots[m].take()).collect();
                opened.sort_by(|x, y| x.started_at.partial_cmp(&y.started_at).unwrap_or(Ordering::Equal));
                let started_at = opened[0].started_at;
                out.push(Tree {
                    key: format!("attempt:{a}"),
                    thread: None,
                    started_at,
                    children: opened,
                    snoozed_working: None,
                    inbox: None,
                });
            }
            Some(_) => {}
        }
    }
    out
}

/// What the person has folded and paged, the state the web keeps in local storage and in its components.
#[cfg(test)]
mod tests;
