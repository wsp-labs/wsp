// SPDX-License-Identifier: AGPL-3.0-only
//! The pure data half of wsp's web sidebar, ported for the GPUI desktop app: session rows folded into threads,
//! each thread's status slot, the sections and trees the sidebar draws, the flat list of rows a virtual list paints,
//! and the marks a move writes. Every rule here is read off the web source named beside it.

use serde_json::{json, Map, Value};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::screens::app_shell::title::thread_title;

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

/// The registry of status kinds, in the order a thread is read against them; the first that matches wins.
/// `Finished` is never read off a thread: a lead's Finished fold gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    NeedsYou,
    Waiting,
    Starting,
    Working,
    Limited,
    Resuming,
    Failed,
    Released,
    Expired,
    Stopped,
    NotPushed,
    Done,
    Resting,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Input,
    Working,
    Failed,
    Done,
    /// The web's `limited` token, inked in warning-foreground.
    Warning,
}

/// What the one status slot draws. A tile draws Working's crab at its title's end rather than in the slot
/// (ThreadTile.tsx:427,452); Starting's crab stands in the slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub glyph: Option<&'static str>,
    pub crab: bool,
    pub tone: Option<Tone>,
    pub text: Option<String>,
}

pub fn kind_of(t: &Thread, now_ms: f64) -> Kind {
    let sub = t.sub.as_ref().map(|s| s.state);
    let failed = t.status == Status::Failed;
    if t.asks {
        Kind::NeedsYou
    } else if t.running() && (t.capped || sub == Some(SubState::Waiting)) {
        Kind::Waiting
    } else if sub == Some(SubState::Starting) && !failed {
        Kind::Starting
    } else if t.running() {
        Kind::Working
    } else if failed && t.limit && t.resume_at.is_none() {
        Kind::Limited
    } else if failed && t.limit {
        Kind::Resuming
    } else if failed {
        Kind::Failed
    } else if sub == Some(SubState::Released) && matches!(t.status, Status::Completed | Status::Interrupted) && !t.unread {
        Kind::Released
    } else if t.disk_kept_until.is_some_and(|keep| keep <= now_ms) && !t.unread {
        Kind::Expired
    } else if t.status == Status::Interrupted {
        Kind::Stopped
    } else if t.held() {
        Kind::NotPushed
    } else if t.unread {
        Kind::Done
    } else {
        Kind::Resting
    }
}

/// A subagent's kind, read as a thread's would be off its own state; a settled one keeps its age alone.
pub fn subagent_kind(s: &Subagent, part: Part) -> Kind {
    match (part, s.state) {
        (Part::Settled, _) => Kind::Resting,
        (_, SubagentState::Running) => Kind::Working,
        (_, SubagentState::Failed) => Kind::Failed,
        (_, SubagentState::Stopped) => Kind::Stopped,
        (Part::Finished, SubagentState::Done) => Kind::Finished,
        (Part::Live, SubagentState::Done) => Kind::Resting,
    }
}

/// The kind a drawn tile's slot shows: a settled row or a snoozed root rests, a finished row read already ends in
/// Finished's muted check.
pub fn tile_kind(t: &Thread, tile: &Tile, now_ms: f64) -> Kind {
    match tile.shape {
        Shape::Settled => Kind::Resting,
        _ if tile.snoozed_working.is_some() => Kind::Resting,
        Shape::Finished => match kind_of(t, now_ms) {
            Kind::Resting if t.status == Status::Completed => Kind::Finished,
            kind => kind,
        },
        Shape::Full => kind_of(t, now_ms),
    }
}

/// Whether a full tile's title takes the muted ink for this kind, unless it is the one open (ThreadTile.tsx:361).
/// Slim rows always draw it muted.
pub fn recedes(kind: Kind) -> bool {
    matches!(kind, Kind::Resting | Kind::Working | Kind::Resuming | Kind::Stopped | Kind::Released | Kind::Expired)
}

/// The slot for a kind. `age_at` is the moment a resting row's age counts from, as `Tile::age_at` gives it.
pub fn slot(t: &Thread, kind: Kind, now_ms: f64, age_at: Option<f64>) -> Slot {
    let (glyph, tone) = match kind {
        Kind::NeedsYou => (Some("message-circle-question"), Some(Tone::Input)),
        Kind::Waiting => (Some("hourglass"), None),
        Kind::Starting | Kind::Resting => (None, (kind == Kind::Starting).then_some(Tone::Working)),
        Kind::Working => (None, Some(Tone::Working)),
        Kind::Limited => (Some("gauge"), Some(Tone::Warning)),
        Kind::Resuming => (Some("hourglass"), None),
        Kind::Failed => (Some("circle-alert"), Some(Tone::Failed)),
        Kind::Released => (Some("cloud-off"), None),
        Kind::Expired => (Some("calendar-x-2"), None),
        Kind::Stopped => (Some("circle-stop"), None),
        Kind::NotPushed => (Some("upload"), Some(Tone::Input)),
        Kind::Done => (Some("circle-check"), Some(Tone::Done)),
        Kind::Finished => (Some("circle-check"), None),
    };
    let text = match kind {
        Kind::Resting => age_at.map(|at| resting_age(at, now_ms)),
        Kind::Working => Some(running_time(now_ms - t.started_at.unwrap_or(now_ms))),
        _ => None,
    };
    Slot { glyph, crab: matches!(kind, Kind::Starting | Kind::Working), tone, text }
}

/// A drawn tile's slot: a snoozed root's count of working threads, else "Refused" on a full tile at rest or failed
/// whose agent cannot start there (its reason is the slot's tooltip), else its kind's.
pub fn tile_slot(t: &Thread, tile: &Tile, now_ms: f64) -> Slot {
    let kind = tile_kind(t, tile, now_ms);
    if let Some(working) = tile.snoozed_working {
        return Slot { glyph: Some("alarm-clock"), crab: false, tone: None, text: Some(format!("{working} working")) };
    }
    if tile.shape == Shape::Full && t.setup_refusal.is_some() && matches!(kind, Kind::Resting | Kind::Failed) {
        return Slot { glyph: None, crab: false, tone: None, text: Some("Refused".to_owned()) };
    }
    slot(t, kind, now_ms, tile.age_at)
}

/// The slot's word, for its tooltip and its label; Resting has none. `utc_offset_ms` places a reset days away on
/// the reader's weekday.
pub fn word(t: &Thread, kind: Kind, now_ms: f64, utc_offset_ms: f64) -> Cow<'static, str> {
    Cow::Borrowed(match kind {
        Kind::NeedsYou => "Needs you",
        Kind::Waiting => "Waiting",
        Kind::Starting => "Starting",
        Kind::Working => "Working",
        Kind::Limited => "At limit",
        Kind::Resuming => return Cow::Owned(resets_word(t.resume_at.unwrap_or(0.0), now_ms, utc_offset_ms)),
        Kind::Failed => "Failed",
        Kind::Released => "Released",
        Kind::Expired => "Expired",
        Kind::Stopped => "Stopped",
        Kind::NotPushed => "Not pushed",
        Kind::Done | Kind::Finished => "Done",
        Kind::Resting => "",
    })
}

/// A turn's running time as the slot shows it: seconds in its first minute, then minutes: "42s", "17m", "1h 5m".
pub fn running_time(ms: f64) -> String {
    let ms = if ms.is_finite() && ms > 0.0 { ms } else { 0.0 };
    if ms < 60_000.0 {
        return format!("{}s", (ms / 1_000.0).floor());
    }
    let minutes = (ms / 60_000.0).floor() as u64;
    let hours = minutes / 60;
    if minutes < 60 {
        format!("{minutes}m")
    } else if hours < 24 {
        format!("{hours}h {}m", minutes % 60)
    } else {
        format!("{}d {}h", hours / 24, hours % 24)
    }
}

/// How long ago a moment was, as a resting row says it: "now", "5m", "3h", "2d".
pub fn resting_age(at_ms: f64, now_ms: f64) -> String {
    let seconds = ((now_ms - at_ms) / 1_000.0).floor();
    if seconds < 60.0 {
        return "now".to_owned();
    }
    let minutes = (seconds / 60.0).floor();
    let hours = (minutes / 60.0).floor();
    if minutes < 60.0 {
        format!("{minutes}m")
    } else if hours < 24.0 {
        format!("{hours}h")
    } else {
        format!("{}d", (hours / 24.0).floor())
    }
}

/// When a usage limit resets, as a slot words it (usage.ts resetsWord).
pub fn resets_word(resets_at: f64, now_ms: f64, utc_offset_ms: f64) -> String {
    let left = resets_at - now_ms;
    if left <= 0.0 {
        return "reset".to_owned();
    }
    if left < 3_600_000.0 {
        return format!("resets in {} min", (left / 60_000.0).round().max(1.0));
    }
    if left < 86_400_000.0 {
        return format!("resets in {} h", (left / 3_600_000.0).floor());
    }
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let day = ((resets_at + utc_offset_ms) / 86_400_000.0).floor().rem_euclid(7.0) as usize;
    format!("resets on {}", DAYS[day])
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
#[derive(Clone, Debug)]
pub struct Folds {
    /// Sections folded by their heads; `wsp:sidebar-folded`, default Settled alone. Threads has no head.
    pub sections: HashSet<Section>,
    /// How many tiles the open Settled fold draws.
    pub settled_shown: usize,
    /// The leads whose Finished fold is open, by fold key, each with how many rows it draws.
    pub finished_open: HashMap<String, usize>,
    /// Group heads shut by their chevron, by attempt id.
    pub groups_shut: HashSet<String>,
    /// The thread open in the centre: a settled tree holding it stays on the drawn page.
    pub open: Option<String>,
}

impl Default for Folds {
    fn default() -> Self {
        Self {
            sections: HashSet::from([Section::Settled]),
            settled_shown: SETTLED_PAGE,
            finished_open: HashMap::new(),
            groups_shut: HashSet::new(),
            open: None,
        }
    }
}

/// How a tile is drawn: the two-row tile, or a slim row in a Finished fold or a settled tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Full,
    Finished,
    Settled,
}

/// The tree under a tile: none drawn, drawn open, or folded with how many rows it would draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Under {
    None,
    Open,
    Folded(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub thread: usize,
    pub shape: Shape,
    pub under: Under,
    /// A section's root that a move or a drag takes.
    pub moves: bool,
    /// A tile of the Needs you inbox, and the threads from its tree's top down to the one that started it there.
    pub inbox: bool,
    pub started_by: Vec<usize>,
    pub snoozed_working: Option<usize>,
    /// The moment a resting slot's age counts from.
    pub age_at: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Head {
        section: Section,
        count: usize,
        folded: bool,
    },
    Tile(Tile),
    /// One send to several models: its title is `title_of`'s.
    Group {
        attempt: String,
        title_of: usize,
        copies: usize,
        open: bool,
        moves: bool,
    },
    Subagent {
        lead: usize,
        index: usize,
        part: Part,
    },
    /// A lead's Finished fold.
    Finished {
        lead: usize,
        count: usize,
        open: bool,
    },
    /// "n more", under a lead's open Finished fold, or under the Settled fold where `lead` is None.
    More {
        n: usize,
        lead: Option<usize>,
    },
}

/// One drawn row. `depth` is how many child lists in it stands; `last` that it ends its list, so its rail stops at
/// its elbow; bit d of `rails` that the rail of depth d runs past it, an item above it at that depth not being last.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub depth: u8,
    pub last: bool,
    pub rails: u32,
    pub item: Item,
    /// The row opens a section with a drawn section above it, or is the Settled head: the sections' 12 px gap
    /// stands over it (WorkspaceSidebar.tsx gapsAtRest).
    pub gap: bool,
    /// The section the row stands in.
    pub section: Section,
}

struct Entry {
    item: Item,
    kids: Vec<Entry>,
}

fn leaf(item: Item) -> Entry {
    Entry { item, kids: Vec::new() }
}

struct Draw<'v, 'a> {
    view: &'v View<'a>,
    folds: &'v Folds,
}

impl Draw<'_, '_> {
    /// WorkspaceSidebar.tsx tileItem: a tile and what it draws under it, nested two levels in and then at the
    /// second level's x, as siblings in the list it stands in.
    fn tile(&self, node: &Tree, depth: usize, settled: bool, part: Part, root: bool, out: &mut Vec<Entry>) {
        let cx = &self.view.cx;
        let Some(i) = node.thread else {
            let attempt = node.key.trim_start_matches("attempt:").to_owned();
            let open = !self.folds.groups_shut.contains(&attempt);
            let mut kids = Vec::new();
            for child in node.children.iter().filter(|_| open) {
                self.tile(child, depth + 1, settled, Part::Live, false, &mut kids);
            }
            let title_of = node.children.first().and_then(|c| c.thread).unwrap_or(0);
            out.push(Entry { item: Item::Group { attempt, title_of, copies: node.children.len(), open, moves: root && !settled }, kids });
            return;
        };
        let t = &cx.all[i];
        let draws = !settled && part == Part::Live && node.inbox.is_none() && node.snoozed_working.is_none() && cx.draws_under(node);
        let under = match (draws, t.folded_at.is_some()) {
            (false, _) => Under::None,
            (true, true) => Under::Folded(cx.rows_under(node)),
            (true, false) => Under::Open,
        };
        let tile = Tile {
            thread: i,
            shape: if settled {
                Shape::Settled
            } else if part == Part::Finished {
                Shape::Finished
            } else {
                Shape::Full
            },
            under,
            moves: root && !settled && node.snoozed_working.is_none() && node.inbox.as_ref().is_none_or(Vec::is_empty),
            inbox: node.inbox.is_some(),
            started_by: node.inbox.clone().unwrap_or_default(),
            snoozed_working: node.snoozed_working,
            age_at: if settled && part == Part::Live { Some(cx.tree_settled_ms(node)) } else { t.ended_at.or(t.started_at) },
        };
        let lead = if under == Under::Open { self.lead_tree(node, depth + 1) } else { Vec::new() };
        let mut kids = Vec::new();
        if settled {
            for child in &node.children {
                self.tile(child, depth + 1, true, Part::Live, false, &mut kids);
            }
        }
        let nests = depth < 2;
        if nests {
            kids.extend(lead);
            out.push(Entry { item: Item::Tile(tile), kids });
        } else {
            out.push(Entry { item: Item::Tile(tile), kids });
            out.extend(lead);
        }
    }

    /// LeadTree.tsx: the live children, then the Finished fold of finished and settled threads, paged.
    fn lead_tree(&self, node: &Tree, depth: usize) -> Vec<Entry> {
        let cx = &self.view.cx;
        let lead = node.thread.unwrap_or(0);
        let [live, finished, settled] = cx.child_parts(cx.lead_nodes(node));
        let mut out = Vec::new();
        for l in live {
            match l {
                Lead::Thread(child) => self.tile(child, depth, false, Part::Live, false, &mut out),
                Lead::Sub(of, index) => out.push(leaf(Item::Subagent { lead: of, index, part: Part::Live })),
            }
        }
        let folded: Vec<(Lead, Part)> = finished
            .into_iter()
            .map(|l| (l, Part::Finished))
            .chain(settled.into_iter().filter(|l| matches!(l, Lead::Thread(_))).map(|l| (l, Part::Settled)))
            .collect();
        if folded.is_empty() {
            return out;
        }
        let shown = self.folds.finished_open.get(&cx.all[lead].key).copied();
        out.push(leaf(Item::Finished { lead, count: folded.len(), open: shown.is_some() }));
        if let Some(shown) = shown {
            for &(l, part) in folded.iter().take(shown) {
                match l {
                    Lead::Thread(child) => self.tile(child, depth, part == Part::Settled, part, false, &mut out),
                    Lead::Sub(of, index) => out.push(leaf(Item::Subagent { lead: of, index, part })),
                }
            }
            if folded.len() > shown {
                out.push(leaf(Item::More { n: folded.len() - shown, lead: Some(lead) }));
            }
        }
        out
    }
}

fn tile_count(node: &Tree) -> usize {
    usize::from(node.thread.is_some()) + node.children.iter().map(tile_count).sum::<usize>()
}

fn flatten(list: Vec<Entry>, depth: usize, rails: u32, out: &mut Vec<Line>) {
    let n = list.len();
    for (k, entry) in list.into_iter().enumerate() {
        let last = k + 1 == n;
        out.push(Line { depth: depth.min(u8::MAX as usize) as u8, last, rails, item: entry.item, gap: false, section: Section::Threads });
        let below = if (1..32).contains(&depth) && !last { rails | 1 << depth } else { rails };
        flatten(entry.kids, depth + 1, below, out);
    }
}

/// Where a root stands among the trees of its section that move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootPlace {
    pub section: Section,
    pub index: usize,
    pub count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Top,
    /// A drop at a place in Pinned or the list, `at` counted among the section's trees with the moving one left out.
    To {
        section: Section,
        at: usize,
    },
}

/// One `sessions.mark` a move sends: the threads it names and the marks it writes on each.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveMark {
    pub thread_ids: Vec<String>,
    pub marks: Value,
}

impl<'a> View<'a> {
    pub fn all(&self) -> &'a [Thread] {
        self.cx.all
    }

    pub fn section(&self, section: Section) -> &[Tree] {
        match section {
            Section::Pinned => &self.pinned,
            Section::NeedsYou => &self.needs_you,
            Section::Threads => &self.threads,
            Section::Settled => &self.settled,
        }
    }

    /// The rows the sidebar draws top to bottom: Pinned and Needs you under their heads, the list with none, then
    /// the Settled head and its page of trees.
    pub fn lines(&self, folds: &Folds) -> Vec<Line> {
        let draw = Draw { view: self, folds };
        let mut top = Vec::new();
        let mut gaps = Vec::new();
        let mut sections: Vec<(usize, Section)> = Vec::new();
        for section in [Section::Pinned, Section::NeedsYou, Section::Threads] {
            let roots = self.section(section);
            if roots.is_empty() {
                continue;
            }
            if !top.is_empty() {
                gaps.push(top.len());
            }
            sections.push((top.len(), section));
            let folded = section != Section::Threads && folds.sections.contains(&section);
            if section != Section::Threads {
                let count = roots.iter().map(|n| self.cx.drawn_count(n)).sum();
                top.push(leaf(Item::Head { section, count, folded }));
            }
            for node in roots.iter().filter(|_| !folded) {
                draw.tile(node, 0, false, Part::Live, true, &mut top);
            }
        }
        if !self.settled.is_empty() {
            gaps.push(top.len());
            sections.push((top.len(), Section::Settled));
            let count: usize = self.settled.iter().map(tile_count).sum();
            let folded = folds.sections.contains(&Section::Settled);
            top.push(leaf(Item::Head { section: Section::Settled, count, folded }));
            if !folded {
                let holding = |n: &Tree| folds.open.as_deref().is_some_and(|open| holds(n, self.cx.all, open));
                let through_open = self.settled.iter().position(holding).map_or(0, |at| self.settled[..=at].iter().map(tile_count).sum());
                let shown = folds.settled_shown.max(through_open);
                let mut tiles = 0;
                let mut drawn = 0;
                for (at, node) in self.settled.iter().enumerate() {
                    tiles += tile_count(node);
                    if tiles > shown && at > 0 {
                        break;
                    }
                    drawn = tiles;
                    draw.tile(node, 0, true, Part::Live, false, &mut top);
                }
                if count > drawn {
                    top.push(leaf(Item::More { n: count - drawn, lead: None }));
                }
            }
        }
        fn size(e: &Entry) -> usize {
            1 + e.kids.iter().map(size).sum::<usize>()
        }
        let mut starts = Vec::with_capacity(top.len());
        let mut at = 0;
        for e in &top {
            starts.push(at);
            at += size(e);
        }
        let mut out = Vec::with_capacity(self.cx.all.len() + 8);
        flatten(top, 0, 0, &mut out);
        for g in gaps {
            if let Some(&line) = starts.get(g) {
                out[line].gap = true;
            }
        }
        for (k, &(entry, section)) in sections.iter().enumerate() {
            let from = starts.get(entry).copied().unwrap_or(out.len());
            let to = sections.get(k + 1).and_then(|&(e, _)| starts.get(e).copied()).unwrap_or(out.len());
            out[from..to].iter_mut().for_each(|l| l.section = section);
        }
        out
    }

    fn root(&self, key: &str) -> Option<&Tree> {
        let top = |n: &&Tree| n.key == key && n.inbox.as_ref().is_none_or(Vec::is_empty);
        self.pinned.iter().chain(&self.threads).chain(&self.needs_you).find(top)
    }

    /// Every finished thread under a tree's tiles and every thread under those, which "Settle n finished" takes
    /// (leadTree.ts finishedTake); none where the key stands in no tree drawn.
    pub fn finished_under(&self, key: &str) -> Vec<String> {
        fn find<'t>(nodes: &'t [Tree], key: &str) -> Option<&'t Tree> {
            nodes.iter().find_map(|n| if n.key == key { Some(n) } else { find(&n.children, key) })
        }
        fn all_under(node: &Tree, all: &[Thread], out: &mut Vec<String>) {
            if let Some(i) = node.thread {
                out.push(all[i].key.clone());
            }
            node.children.iter().for_each(|c| all_under(c, all, out));
        }
        fn walk(cx: &Ctx, node: &Tree, out: &mut Vec<String>) {
            let [live, finished, settled] = cx.child_parts(cx.lead_nodes(node));
            for l in finished.iter().chain(&settled) {
                if let Lead::Thread(n) = *l {
                    if n.thread.is_some_and(|i| cx.all[i].settled_at.is_none()) {
                        all_under(n, cx.all, out);
                    }
                }
            }
            for l in live {
                if let Lead::Thread(n) = l {
                    walk(cx, n, out);
                }
            }
        }
        let tops = [&self.pinned, &self.threads, &self.needs_you];
        let Some(node) = tops.into_iter().find_map(|t| find(t, key)) else { return Vec::new() };
        let mut out = Vec::new();
        walk(&self.cx, node, &mut out);
        out
    }

    pub fn root_place(&self, key: &str) -> Option<RootPlace> {
        [Section::Pinned, Section::Threads].into_iter().find_map(|section| {
            let roots: Vec<&Tree> = self.section(section).iter().filter(|r| r.snoozed_working.is_none()).collect();
            roots.iter().position(|r| r.key == key).map(|index| RootPlace { section, index, count: roots.len() })
        })
    }

    /// What dropping a root on a section's head writes, on every thread it stands for; None for a drop that changes
    /// nothing. A group goes as one: pinned, or unpinned where a member is (WorkspaceSidebar.tsx dropOnHead). A
    /// drop on Settled is a settle, not a mark.
    pub fn drop_marks(&self, key: &str, section: Section) -> Option<MoveMark> {
        let node = self.root(key)?;
        let thread_ids: Vec<String> = self.cx.root_threads(node).map(|i| self.cx.all[i].key.clone()).collect();
        let marks = if node.thread.is_some() {
            Value::Object(self.drop_map(node, section)?)
        } else if section == Section::Pinned {
            json!({ "pinned": true })
        } else if section != Section::Settled && self.cx.pin_key(node).is_some() {
            json!({ "pinned": false })
        } else {
            return None;
        };
        Some(MoveMark { thread_ids, marks })
    }

    fn drop_map(&self, node: &Tree, section: Section) -> Option<Map<String, Value>> {
        let t = &self.cx.all[node.thread?];
        let pinned = t.pinned_at.is_some();
        let mut marks = Map::new();
        match section {
            Section::Settled => return None,
            Section::Pinned if pinned => return None,
            Section::Pinned => {
                marks.insert("pinned".into(), Value::Bool(true));
                return Some(marks);
            }
            _ => {}
        }
        let own_list = if thread_section(t) == ThreadSection::NeedsYou { Section::NeedsYou } else { Section::Threads };
        let own = own_list == section;
        if own && !pinned && t.section.is_none() {
            return None;
        }
        if pinned {
            marks.insert("pinned".into(), Value::Bool(false));
        }
        let placed = if section == Section::NeedsYou { ThreadSection::NeedsYou } else { ThreadSection::Idle };
        let place = if own { Value::Null } else { json!({ "name": placed.id(), "whileState": self.cx.placement_key(node) }) };
        marks.insert("section".into(), place);
        Some(marks)
    }

    /// Move up, Move down, Move to top, or a drop at a place: the marks to send, None where nothing moves
    /// (treeMoves.ts moveRoot). A section with no key to land beside keys off the view's clock sample.
    pub fn move_root(&self, key: &str, to: Move) -> Option<Vec<MoveMark>> {
        let node = self.root(key)?;
        let from = self.root_place(key);
        let section = match to {
            Move::To { section, .. } => section,
            _ => from?.section,
        };
        if !matches!(section, Section::Pinned | Section::Threads) {
            return None;
        }
        let at = match (to, from) {
            (Move::To { at, .. }, _) => at as isize,
            (Move::Top, _) => 0,
            (Move::Up, Some(f)) => f.index as isize - 1,
            (Move::Down, Some(f)) => f.index as isize + 1,
            _ => return None,
        };
        let others: Vec<&Tree> = self.section(section).iter().filter(|r| r.snoozed_working.is_none() && r.key != key).collect();
        if at < 0 || at as usize > others.len() || from.is_some_and(|f| f.section == section && f.index as isize == at) {
            return None;
        }
        let at = at as usize;
        let keys: Vec<f64> = others.iter().map(|r| self.cx.key_in(section, r)).collect();
        let (new_key, rekeyed) = key_at(&keys, at, self.cx.now);
        let ids = |n: &Tree| self.cx.root_threads(n).map(|i| self.cx.all[i].key.clone()).collect::<Vec<_>>();
        let field = if section == Section::Pinned { "pinned" } else { "order" };
        let own = if section == Section::Pinned {
            json!({ "pinned": new_key })
        } else {
            let mut marks = self.leaving(node).unwrap_or_default();
            if self.cx.pin_key(node).is_some() {
                marks.insert("pinned".into(), Value::Bool(false));
            }
            marks.insert("order".into(), json!(new_key));
            Value::Object(marks)
        };
        let mut out = vec![MoveMark { thread_ids: ids(node), marks: own }];
        out.extend(rekeyed.into_iter().map(|(index, k)| MoveMark { thread_ids: ids(others[index]), marks: json!({ field: k }) }));
        Some(out)
    }

    /// What a tree carries into the list beside its key: a placement where its state files it in Needs you, and a
    /// placement taken off where it has one.
    fn leaving(&self, node: &Tree) -> Option<Map<String, Value>> {
        let mut marks = self.drop_map(node, Section::Threads)?;
        let has_placement = node.thread.is_some_and(|i| self.cx.all[i].section.is_some());
        if marks.get("section").is_some_and(Value::is_null) && !has_placement {
            marks.remove("section");
        }
        Some(marks)
    }
}

fn holds(node: &Tree, all: &[Thread], key: &str) -> bool {
    node.thread.is_some_and(|i| all[i].key == key) || node.children.iter().any(|c| holds(c, all, key))
}

/// treeOrder.ts keyAt: a tree's key where it lands among a section's keys in drawn order, and the trees a gap that
/// ran out re-keys, by index. Between two trees it is their midpoint.
pub fn key_at(keys: &[f64], at: usize, now_ms: f64) -> (f64, Vec<(usize, f64)>) {
    let (Some(&first), Some(&last)) = (keys.first(), keys.last()) else { return (now_ms, Vec::new()) };
    if at == 0 {
        return (first + ABOVE_TOP_MS, Vec::new());
    }
    if at >= keys.len() {
        return (last - BELOW_LAST_MS, Vec::new());
    }
    let (above, below) = (keys[at - 1], keys[at]);
    let mid = above / 2.0 + below / 2.0;
    if mid < above && mid > below {
        return (mid, Vec::new());
    }
    let mut end = at;
    while end + 1 < keys.len() && keys[end] - keys[end + 1] < REKEY_GAP_MS {
        end += 1;
    }
    let floor = if end + 1 < keys.len() { keys[end + 1] } else { keys[end] - BELOW_LAST_MS };
    let step = (above - floor) / (end - at + 3) as f64;
    (above - step, (0..=end - at).map(|i| (at + i, above - step * (i + 2) as f64)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: f64 = 1_800_000_000_000.0;
    const MIN: f64 = 60_000.0;
    const HOUR: f64 = 3_600_000.0;

    fn row(id: &str, status: Status, started: f64) -> Row {
        Row { id: id.into(), workspace_id: "w".into(), harness: "claude".into(), status, started_at: Some(started), ..Row::default() }
    }

    fn done(id: &str, started: f64) -> Row {
        Row { ended_at: Some(started + MIN), read_at: Some(started + MIN), ..row(id, Status::Completed, started) }
    }

    fn child(mut r: Row, parent: &str) -> Row {
        r.parent_thread_id = Some(parent.into());
        r
    }

    fn view(rows: &[Row]) -> Vec<Thread> {
        fold_threads(rows)
    }

    fn keys(all: &[Thread], trees: &[Tree]) -> Vec<String> {
        trees.iter().map(|t| t.thread.map_or_else(|| t.key.clone(), |i| all[i].key.clone())).collect()
    }

    fn tiles(lines: &[Line], all: &[Thread]) -> Vec<(String, u8)> {
        lines.iter().filter_map(|l| if let Item::Tile(t) = &l.item { Some((all[t.thread].key.clone(), l.depth)) } else { None }).collect()
    }

    #[test]
    fn fold_takes_latest_state_and_opening_provenance() {
        let mut first = row("a1", Status::Completed, NOW - HOUR);
        first.thread_id = Some("t".into());
        first.prompt = Some("Fix the login page. Then write tests for it.".into());
        first.parent_thread_id = Some("lead".into());
        first.cost_usd = Some(0.5);
        let mut second = row("a2", Status::Running, NOW);
        second.thread_id = Some("t".into());
        second.cost_usd = Some(0.25);
        let all = fold_threads(&[first.clone(), second.clone(), row("solo", Status::Completed, NOW)]);
        assert_eq!(all.len(), 2);
        assert_eq!((all[0].key.as_str(), all[0].session_id.as_str()), ("t", "a2"));
        assert_eq!(all[0].status, Status::Running);
        assert_eq!(all[0].title, "Fix the login page.");
        assert_eq!(all[0].parent.as_deref(), Some("lead"));
        assert_eq!(all[0].cost_usd, Some(0.75));
        assert_eq!(all[1].cost_usd, None);
        second.harness_title = Some("  Login\n fix ".into());
        assert_eq!(fold_threads(&[first, second])[0].title, "Login");
        assert_eq!(fold_threads(&[row("bare", Status::Completed, NOW)])[0].title, "bare");
    }

    #[test]
    fn needs_you_skips_a_stops_end_but_not_a_finish_or_held_work() {
        let all = view(&[
            Row { ended_at: Some(NOW), ..row("fin", Status::Completed, NOW - MIN) },
            Row { ended_at: Some(NOW), ..row("stop", Status::Interrupted, NOW - MIN) },
            Row { sub: Some(Sub { state: SubState::Napping, held: true }), ..done("held", NOW - HOUR) },
            Row { woke_at: Some(NOW), ..done("woke", NOW - HOUR) },
        ]);
        let needs: Vec<bool> = all.iter().map(|t| t.needs_you).collect();
        assert_eq!(needs, [true, false, true, true]);
        assert!(all[1].unread, "a stop nobody saw is still unread");
        assert!(!all[2].unread);
    }

    #[test]
    fn settle_by_hand_by_quiet_and_never_for_failed_or_unseen() {
        let settle = Some(SETTLE_MS_DEFAULT);
        let read = &view(&[done("r", NOW - 5.0 * HOUR)])[0];
        assert!(thread_settled(read, NOW, settle, false));
        assert!(!thread_settled(read, NOW, settle, true), "held never folds by time");
        assert!(!thread_settled(read, NOW, None, false), "never means only a hand settles");
        let recent = &view(&[done("r", NOW - HOUR)])[0];
        assert!(!thread_settled(recent, NOW, settle, false));
        let unseen = &view(&[Row { read_at: None, ..done("u", NOW - 5.0 * HOUR) }])[0];
        assert!(!thread_settled(unseen, NOW, settle, false));
        let failed = &view(&[Row { status: Status::Failed, ..done("f", NOW - 5.0 * HOUR) }])[0];
        assert!(!thread_settled(failed, NOW, settle, false));
        let hand = &view(&[Row { settled_at: Some(NOW - HOUR + 2.0 * MIN), ..done("h", NOW - HOUR) }])[0];
        assert_eq!(settle_moment(hand, settle, true), Some(SettleMoment { ms: NOW - HOUR + 2.0 * MIN, by_hand: true }));
        let since = &view(&[Row { settled_at: Some(NOW - 3.0 * HOUR), ..done("s", NOW - HOUR) }])[0];
        assert!(!thread_settled(since, NOW, settle, false), "activity after a hand settle brings it back");
    }

    #[test]
    fn kinds_read_in_registry_order() {
        let k = |r: Row| kind_of(&view(&[r])[0], NOW);
        let running = row("r", Status::Running, NOW);
        assert_eq!(k(Row { asking: true, ..running.clone() }), Kind::NeedsYou);
        assert_eq!(k(Row { waiting_on: true, ..running.clone() }), Kind::NeedsYou);
        assert_eq!(k(Row { capped: true, ..running.clone() }), Kind::Waiting);
        assert_eq!(k(Row { sub: Some(Sub { state: SubState::Starting, held: false }), ..running.clone() }), Kind::Starting);
        assert_eq!(k(running.clone()), Kind::Working);
        let failed = Row { ended_at: Some(NOW), ..row("f", Status::Failed, NOW - MIN) };
        assert_eq!(k(Row { limit: true, ..failed.clone() }), Kind::Limited);
        assert_eq!(k(Row { limit: true, resume_at: Some(NOW + HOUR), ..failed.clone() }), Kind::Resuming);
        assert_eq!(k(failed), Kind::Failed);
        let released = Some(Sub { state: SubState::Released, held: false });
        assert_eq!(k(Row { sub: released.clone(), ..done("x", NOW - HOUR) }), Kind::Released);
        assert_eq!(k(Row { sub: released, read_at: None, ..done("x", NOW - HOUR) }), Kind::Done, "unread reads Done first");
        assert_eq!(k(Row { disk_kept_until: Some(NOW - 1.0), ..done("x", NOW - HOUR) }), Kind::Expired);
        assert_eq!(k(Row { ended_at: Some(NOW), ..row("s", Status::Interrupted, NOW - MIN) }), Kind::Stopped, "a stop outranks Done");
        assert_eq!(k(Row { sub: Some(Sub { state: SubState::Napping, held: true }), ..done("x", NOW - HOUR) }), Kind::NotPushed);
        assert_eq!(k(done("x", NOW - HOUR)), Kind::Resting);
    }

    #[test]
    fn slots_and_receding_titles() {
        let working = &view(&[row("w", Status::Running, NOW - 42_000.0)])[0];
        let s = slot(working, Kind::Working, NOW, None);
        assert_eq!((s.text.as_deref(), s.tone, s.glyph, s.crab), (Some("42s"), Some(Tone::Working), None, true));
        let resting = &view(&[done("r", NOW - 3.0 * HOUR)])[0];
        assert_eq!(slot(resting, Kind::Resting, NOW, Some(NOW - 5.0 * MIN)).text.as_deref(), Some("5m"));
        assert_eq!(
            slot(resting, Kind::Failed, NOW, None),
            Slot { glyph: Some("circle-alert"), crab: false, tone: Some(Tone::Failed), text: None }
        );
        assert_eq!(word(resting, Kind::Limited, NOW, 0.0), "At limit");
        let refused = &view(&[Row { setup_refusal: Some("config folder outside home".into()), ..done("x", NOW - HOUR) }])[0];
        let tile = Tile {
            thread: 0,
            shape: Shape::Full,
            under: Under::None,
            moves: true,
            inbox: false,
            started_by: Vec::new(),
            snoozed_working: None,
            age_at: None,
        };
        assert_eq!(tile_slot(refused, &tile, NOW).text.as_deref(), Some("Refused"));
        assert_eq!(tile_slot(refused, &Tile { shape: Shape::Settled, ..tile }, NOW).text, None);
        assert!(recedes(Kind::Working) && recedes(Kind::Resting) && recedes(Kind::Stopped));
        assert!(!recedes(Kind::NeedsYou) && !recedes(Kind::Done) && !recedes(Kind::Failed) && !recedes(Kind::NotPushed));
    }

    #[test]
    fn running_time_and_resting_age_formats() {
        assert_eq!(running_time(42_000.0), "42s");
        assert_eq!(running_time(17.0 * MIN + 5_000.0), "17m");
        assert_eq!(running_time(65.0 * MIN), "1h 5m");
        assert_eq!(running_time(26.0 * HOUR), "1d 2h");
        assert_eq!(running_time(-5.0), "0s");
        assert_eq!(resting_age(NOW - 30_000.0, NOW), "now");
        assert_eq!(resting_age(NOW + 5_000.0, NOW), "now");
        assert_eq!(resting_age(NOW - 5.0 * MIN, NOW), "5m");
        assert_eq!(resting_age(NOW - 3.0 * HOUR, NOW), "3h");
        assert_eq!(resting_age(NOW - 50.0 * HOUR, NOW), "2d");
        assert_eq!(resets_word(NOW + 14.0 * MIN, NOW, 0.0), "resets in 14 min");
        assert_eq!(resets_word(NOW + 5.0 * HOUR, NOW, 0.0), "resets in 5 h");
        assert_eq!(resets_word(0.0 + 3.0 * 24.0 * HOUR, 0.0, 0.0), "resets on Sun");
    }

    #[test]
    fn sections_split_pinned_inbox_list_and_settled_newest_first() {
        let all = view(&[
            row("old", Status::Running, NOW - 3.0 * HOUR),
            row("new", Status::Running, NOW - HOUR),
            Row { pinned_at: Some(NOW), ..row("pin", Status::Running, NOW - 4.0 * HOUR) },
            Row { asking: true, ..row("ask", Status::Running, NOW - 2.0 * HOUR) },
            done("quiet", NOW - 9.0 * HOUR),
        ]);
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        assert_eq!(keys(&all, &v.pinned), ["pin"]);
        assert_eq!(keys(&all, &v.needs_you), ["ask"], "a lone root that asks stands in the inbox alone");
        assert_eq!(keys(&all, &v.threads), ["new", "old"]);
        assert_eq!(keys(&all, &v.settled), ["quiet"]);
    }

    #[test]
    fn a_moved_key_reorders_the_list() {
        let all =
            view(&[row("a", Status::Running, NOW - 3.0 * HOUR), Row { order: Some(NOW), ..row("b", Status::Running, NOW - 5.0 * HOUR) }]);
        assert_eq!(keys(&all, &sidebar(&all, &Prefs::default(), NOW, None).threads), ["b", "a"]);
    }

    #[test]
    fn a_tree_with_an_asking_child_shows_in_both_inbox_and_list() {
        let all =
            view(&[row("lead", Status::Running, NOW - HOUR), Row { asking: true, ..child(row("kid", Status::Running, NOW), "lead") }]);
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        assert_eq!(keys(&all, &v.threads), ["lead"]);
        assert_eq!(keys(&all, &v.needs_you), ["kid"]);
        assert_eq!(v.needs_you[0].inbox.as_deref(), Some(&[0usize][..]));
        let lines = v.lines(&Folds::default());
        let inbox = lines.iter().find_map(|l| if let Item::Tile(t) = &l.item { t.inbox.then_some(t) } else { None }).unwrap();
        assert_eq!((inbox.started_by.as_slice(), inbox.moves, inbox.under), (&[0usize][..], false, Under::None));
    }

    #[test]
    fn placement_holds_while_the_state_holds() {
        let mut ask = Row { asking: true, ..row("ask", Status::Running, NOW - 2.0 * HOUR) };
        let all = view(std::slice::from_ref(&ask));
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        let marks = v.drop_marks("ask", Section::Threads).unwrap().marks;
        assert_eq!(marks, json!({ "section": { "name": "idle", "whileState": "needs-you:ask" } }));
        ask.section = Some(Placement { name: ThreadSection::Idle, while_state: "needs-you:ask".into() });
        let all = view(std::slice::from_ref(&ask));
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        assert!(v.needs_you.is_empty());
        assert_eq!(keys(&all, &v.threads), ["ask"]);
        ask.section.as_mut().unwrap().while_state = "needs-you:older-turn".into();
        let all = view(&[ask]);
        assert_eq!(keys(&all, &sidebar(&all, &Prefs::default(), NOW, None).needs_you), ["ask"], "a new turn lapses it");
        assert_eq!(
            sidebar(&all, &Prefs::default(), NOW, None).drop_marks("ask", Section::Pinned).map(|m| m.marks),
            Some(json!({ "pinned": true }))
        );
    }

    #[test]
    fn attempts_group_under_one_head_oldest_first() {
        let a = |id: &str, at: f64| Row { attempt: Some("x".into()), prompt: Some(format!("{id} task")), ..row(id, Status::Running, at) };
        let all = view(&[a("m2", NOW - MIN), a("m1", NOW - 2.0 * MIN), row("solo", Status::Running, NOW)]);
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        assert_eq!(keys(&all, &v.threads), ["solo", "attempt:x"]);
        assert_eq!(keys(&all, &v.threads[1].children), ["m1", "m2"]);
        let pin = v.drop_marks("attempt:x", Section::Pinned).unwrap();
        assert_eq!((pin.thread_ids, pin.marks), (vec!["m1".to_owned(), "m2".to_owned()], json!({ "pinned": true })), "a group pins as one");
        assert_eq!(v.drop_marks("attempt:x", Section::NeedsYou), None);
        let lines = v.lines(&Folds::default());
        let group =
            lines.iter().find_map(|l| if let Item::Group { title_of, copies, .. } = l.item { Some((title_of, copies)) } else { None });
        assert_eq!(group.map(|(t, c)| (all[t].title.as_str(), c)), Some(("m1 task", 2)));
    }

    #[test]
    fn a_snoozed_tree_hides_unless_it_works() {
        let snooze = |r: Row| Row { snoozed_until: Some(NOW + HOUR), ..r };
        let all = view(&[
            snooze(row("busy", Status::Running, NOW - HOUR)),
            snooze(done("idle", NOW - HOUR)),
            row("plain", Status::Running, NOW - 2.0 * HOUR),
        ]);
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        assert_eq!(keys(&all, &v.threads), ["plain", "busy"]);
        assert_eq!(v.threads[1].snoozed_working, Some(1));
        assert!(v.settled.is_empty());
        assert_eq!(v.root_place("busy"), None, "a snoozed tree at the foot does not move");
        let lines = v.lines(&Folds::default());
        let Item::Tile(busy) = &lines[1].item else { panic!("busy is the second row") };
        assert_eq!(tile_slot(&all[0], busy, NOW).text.as_deref(), Some("1 working"));
    }

    #[test]
    fn lines_head_counts_and_settled_paging() {
        let mut rows: Vec<Row> = (0..30).map(|i| done(&format!("s{i:02}"), NOW - 10.0 * HOUR - f64::from(i) * MIN)).collect();
        rows.push(Row { pinned_at: Some(NOW), ..row("pin", Status::Running, NOW) });
        let all = view(&rows);
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        let lines = v.lines(&Folds::default());
        let heads: Vec<_> = lines
            .iter()
            .filter_map(|l| if let Item::Head { section, count, folded } = l.item { Some((section, count, folded)) } else { None })
            .collect();
        assert_eq!(heads, [(Section::Pinned, 1, false), (Section::Settled, 30, true)]);
        assert_eq!(lines.len(), 3, "Settled starts folded");
        let open = Folds { sections: HashSet::new(), ..Folds::default() };
        let lines = v.lines(&open);
        assert_eq!(tiles(&lines, &all).len(), 26);
        assert_eq!(lines.last().map(|l| &l.item), Some(&Item::More { n: 5, lead: None }));
        let to_open = Folds { open: Some("s29".into()), ..open.clone() };
        assert_eq!(tiles(&v.lines(&to_open), &all).len(), 31, "the open thread's settled tree stays on the page");
        let folded_pin = Folds { sections: HashSet::from([Section::Pinned, Section::Settled]), ..Folds::default() };
        assert_eq!(v.lines(&folded_pin).len(), 2);
    }

    #[test]
    fn lines_nest_two_levels_then_stand_flat_with_rails() {
        let all = view(&[
            row("a", Status::Running, NOW - 5.0 * HOUR),
            child(row("b", Status::Running, NOW - 4.0 * HOUR), "a"),
            child(row("c", Status::Running, NOW - 3.0 * HOUR), "b"),
            child(row("d", Status::Running, NOW - 2.0 * HOUR), "c"),
            child(row("e", Status::Running, NOW - HOUR), "a"),
        ]);
        let lines = sidebar(&all, &Prefs::default(), NOW, None).lines(&Folds::default());
        let drawn = tiles(&lines, &all);
        let expect: Vec<(String, u8)> = [("a", 0), ("e", 1), ("b", 1), ("c", 2), ("d", 2)].iter().map(|&(k, d)| (k.into(), d)).collect();
        assert_eq!(drawn, expect, "live children newest first, then flat at the second level");
        let at = |k: &str| &lines[drawn.iter().position(|(key, _)| key == k).unwrap()];
        assert!(!at("e").last && at("b").last);
        assert!(!at("c").last && at("d").last, "c's own children stand after it in its list");
        assert_eq!(at("c").rails, 0, "b is last, so no rail runs at depth 1 past its tree");
    }

    #[test]
    fn a_folded_tile_counts_its_rows_and_finished_children_fold() {
        let lead = Row { folded_at: Some(NOW), ..row("lead", Status::Running, NOW - 3.0 * HOUR) };
        let fin = child(Row { read_at: None, ..done("fin", NOW - HOUR) }, "lead");
        let live = child(row("live", Status::Running, NOW - HOUR), "lead");
        let all = view(&[lead.clone(), fin.clone(), live.clone()]);
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        let first = v.lines(&Folds::default()).remove(0);
        assert!(matches!(first.item, Item::Tile(Tile { under: Under::Folded(2), .. })), "one live child and the Finished row");
        let all = view(&[Row { folded_at: None, ..lead }, fin, live]);
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        let shut = v.lines(&Folds::default());
        assert_eq!(tiles(&shut, &all).len(), 2);
        assert!(shut.iter().any(|l| l.item == Item::Finished { lead: 0, count: 1, open: false } && l.depth == 1 && l.last));
        let open = Folds { finished_open: HashMap::from([("lead".into(), FINISHED_PAGE)]), ..Folds::default() };
        let lines = v.lines(&open);
        let fin_line =
            lines.iter().find_map(|l| if let Item::Tile(t) = &l.item { (all[t.thread].key == "fin").then_some(t) } else { None });
        assert_eq!(fin_line.map(|t| t.shape), Some(Shape::Finished));
        assert_eq!(tile_kind(&all[1], fin_line.unwrap(), NOW), Kind::Done);
    }

    #[test]
    fn key_at_lands_above_below_between_and_rekeys() {
        assert_eq!(key_at(&[], 0, NOW).0, NOW);
        assert_eq!(key_at(&[100.0, 50.0], 0, NOW).0, 101.0);
        assert_eq!(key_at(&[100.0, 50.0], 2, NOW).0, 50.0 - BELOW_LAST_MS);
        assert_eq!(key_at(&[100.0, 50.0], 1, NOW), (75.0, vec![]));
        // The figures treeOrder.ts keyAt answers for the same keys.
        assert_eq!(key_at(&[NOW, NOW, NOW - 10.0], 1, 0.0), (1799999999996.6667, vec![(1, 1799999999993.3333)]));
        assert_eq!(key_at(&[NOW; 4], 2, 0.0), (1799999985000.0, vec![(2, 1799999970000.0), (3, 1799999955000.0)]));
        let close = [NOW + 0.0001, NOW, NOW - 0.0002, NOW - 5.0];
        assert_eq!(key_at(&close, 1, 0.0), (1799999999998.75, vec![(1, 1799999999997.5), (2, 1799999999996.25)]));
    }

    #[test]
    fn moves_write_order_keys_and_cross_out_of_pinned() {
        let all = view(&[
            row("a", Status::Running, NOW - HOUR),
            row("b", Status::Running, NOW - 2.0 * HOUR),
            row("c", Status::Running, NOW - 3.0 * HOUR),
            Row { pinned_at: Some(NOW), ..row("p", Status::Running, NOW - 4.0 * HOUR) },
        ]);
        let v = sidebar(&all, &Prefs::default(), NOW, None);
        assert_eq!(v.root_place("c"), Some(RootPlace { section: Section::Threads, index: 2, count: 3 }));
        let top = v.move_root("c", Move::Top).unwrap();
        assert_eq!(top, [MoveMark { thread_ids: vec!["c".into()], marks: json!({ "order": NOW - HOUR + ABOVE_TOP_MS }) }]);
        let up = v.move_root("c", Move::Up).unwrap();
        assert_eq!(up[0].marks, json!({ "order": NOW - 1.5 * HOUR }));
        assert_eq!(v.move_root("a", Move::Up), None, "the top moves up nowhere");
        assert_eq!(v.move_root("c", Move::Down), None);
        let out = v.move_root("p", Move::To { section: Section::Threads, at: 3 }).unwrap();
        assert_eq!(out[0].marks, json!({ "pinned": false, "order": NOW - 3.0 * HOUR - BELOW_LAST_MS }));
        let pin = v.move_root("a", Move::To { section: Section::Pinned, at: 0 }).unwrap();
        assert_eq!(pin[0].marks, json!({ "pinned": NOW + ABOVE_TOP_MS }));
    }

    #[test]
    fn subagents_stand_under_their_lead_and_count() {
        let sub = |id: &str, state| Subagent {
            id: id.into(),
            title: id.into(),
            state,
            started_at: NOW - MIN,
            ended_at: None,
            call: None,
            model: None,
            asked: None,
            failure: None,
            last_line: None,
        };
        let lead = Row {
            subagents: vec![sub("s1", SubagentState::Running), sub("s2", SubagentState::Done)],
            ..row("lead", Status::Running, NOW - HOUR)
        };
        let all = view(&[lead]);
        let lines = sidebar(&all, &Prefs::default(), NOW, None).lines(&Folds::default());
        assert!(matches!(lines[0].item, Item::Tile(Tile { under: Under::Open, .. })));
        assert_eq!(lines[1].item, Item::Subagent { lead: 0, index: 0, part: Part::Live });
        assert_eq!(lines[2].item, Item::Finished { lead: 0, count: 1, open: false });
        assert_eq!(subagent_kind(&all[0].subagents[1], Part::Finished), Kind::Finished);
    }

    #[test]
    fn project_pick_keeps_children_wherever_they_run() {
        let mine = |r: Row, p: &str| Row { project_id: Some(p.into()), ..r };
        let all = view(&[
            mine(row("a", Status::Running, NOW - HOUR), "p1"),
            mine(child(row("k", Status::Running, NOW), "a"), "p2"),
            mine(row("b", Status::Running, NOW), "p2"),
        ]);
        let v = sidebar(&all, &Prefs { project: Some("p1".into()), ..Prefs::default() }, NOW, None);
        assert_eq!(keys(&all, &v.threads), ["a"]);
        assert_eq!(keys(&all, &v.threads[0].children), ["k"]);
    }
}
