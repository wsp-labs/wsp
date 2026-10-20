// SPDX-License-Identifier: AGPL-3.0-only
//! A thread's events folded into the transcript's rows as apps/web/src/adapt/session.ts and adapt/timeline-rows.ts fold them.
//! No GPUI here, so the fold is tested on its own.

mod around;
mod derive;
mod facts;
mod fences;
mod groups;
mod listing;
mod model;
mod page;
mod plan;
mod prompt;
mod rows;
#[cfg(test)]
mod tests;
mod turns;

use std::collections::{HashMap, HashSet};

use crate::words::{ToolItem as Item, ToolRequest as Request};
use wsp_host_client::protocol::{SessionEvent, SessionEventResult, SessionEventResultStatus};

use around::Around;
use derive::{derive, owner, Opened};
use facts::Facts;
use page::slate_lines;

pub use around::forked_from_line;
pub(crate) use fences::{block_at, fence_openings};
pub use listing::{
    agent_starting_line, push_again_prompt, waiting_asker_line, ChildPart, ForkSource, Listed, ListedSubagent, Listing, SpawnStep,
    StartFailed, StepState, Strip, StripAct, ThreadAt, TreeCounts, Wait, PAUSED_KEY,
};
pub use page::{Page, SLATE_UPDATED};
pub use plan::{plan_folds, plan_shown};
pub(crate) use prompt::asking_line;
pub use rows::{
    Asked, Call, Can, ChangedFile, Child, FilePatch, FileTile, FooterWord, Glyph, PromptOption, Row, RowKind, Run, Splice, SubLine, Tone,
};
pub use turns::{Docked, RewindFacts, TurnContext};

/// A thread's events and the rows they fold into, folded again only from the running turn on when an event arrives.
#[derive(Default)]
pub struct Transcript {
    events: Vec<SessionEvent>,
    opened: HashSet<String>,
    /// The waiting questions whose subagent folds the person shut; the next question opens its fold again.
    shut: HashSet<String>,
    can: Can,
    /// The title of the thread this one was forked from, off the listing; the rule names its id until one is set.
    source: Option<String>,
    rows: Vec<Row>,
    model: Model,
    /// Where the latest turn opened: a live event refolds from here; None folds the whole thread again.
    mark: Option<Mark>,
    /// Events folded plus entries drawn, so a test can tell a live event's cost from a load's.
    #[cfg(test)]
    cost: usize,
    /// Pages laid under the held events without folding the turns after them again.
    #[cfg(test)]
    grafts: usize,
    /// Entries read looking for the prompt the composer's dock answers.
    #[cfg(test)]
    looked: std::cell::Cell<usize>,
    around: Around,
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, events: Vec<SessionEvent>) {
        self.events = events;
        self.full();
    }

    /// Lays `page`, events older than every event held, under them. Only the page and the turn at its boundary fold;
    /// every later turn keeps its fold and its rows, unless one of its events reaches back past that turn.
    pub fn prepend(&mut self, page: Vec<SessionEvent>) -> Option<Splice> {
        let n = page.len();
        if n == 0 {
            return None;
        }
        let held = std::mem::replace(&mut self.events, page);
        self.events.extend(held);
        match self.graft(n) {
            Some(splice) => splice,
            None => self.refold_all(),
        }
    }

    /// Folds the page's `n` events and the held ones up to a turn opening at which the held fold can be kept, trying
    /// a few openings; None where none fits, and the caller folds the whole thread.
    fn graft(&mut self, n: usize) -> Option<Option<Splice>> {
        if self.around.page.is_some() || self.around.listing.as_ref().is_some_and(|l| l.start_failed.is_some()) {
            return None;
        }
        let (mut head, mut was) = (Model::default(), Model::default());
        for (ix, event) in self.events[..n].iter().enumerate() {
            head.add(ix, event);
        }
        #[cfg(test)]
        {
            self.cost += n;
        }
        let mut tries = 0;
        for c in 0..self.events.len() - n {
            let event = &self.events[n + c];
            let opens =
                c > 0 && !was.turns.is_empty() && matches!(event, SessionEvent::SessionStart { prompt: Some(_), after_limit: None, .. });
            head.add(n + c, event);
            was.add(c, event);
            #[cfg(test)]
            {
                self.cost += 2;
            }
            if !opens {
                continue;
            }
            if self.fits(&head, &was, n, c) {
                if let Some(splice) = self.join(head, was, n, c) {
                    return Some(splice);
                }
                return None;
            }
            tries += 1;
            if tries == GRAFT_TRIES {
                return None;
            }
        }
        None
    }

    /// Whether the held fold's turns from the opening at held event `c` on read nothing before it, so `head` (the page
    /// and the held events through `c`) can stand in for `was` (the held events through `c`) under them.
    fn fits(&self, head: &Model, was: &Model, n: usize, c: usize) -> bool {
        let (Some(hm), Some(wm)) = (&head.latest, &was.latest) else { return false };
        if hm.event != n + c || wm.event != c || wm.turns == 0 || hm.turns == 0 {
            return false;
        }
        let held = &self.model;
        let reads_back = held.far.iter().filter(|(event, _)| *event > c).any(|(_, back)| match *back {
            Far::Entry(ix) => ix < wm.entries,
            Far::Turn(ix) => ix < wm.turns,
            Far::Event(ix) => ix < c,
            Far::Unheld => true,
        });
        let later: &[String] = &held.order[was.order.len()..];
        let moved: HashSet<&str> = head.starts.iter().filter(|(s, k)| was.starts.get(*s) != Some(k)).map(|(s, _)| s.as_str()).collect();
        let counted = |id: &str| id.rsplit_once('#').is_some_and(|(s, _)| moved.contains(s));
        let kept = |t: &Summary| t.kept.as_ref().is_some_and(|k| k.kept);
        let anchored = |t: &Summary| t.kept.as_ref().is_some_and(|k| k.anchor);
        let kept_now = head.turns[..hm.turns].iter().any(kept) || held.turns[wm.turns..].iter().any(kept);
        !reads_back
            && held.earlier == was.earlier
            && head.cwd == was.cwd
            && head.retry == was.retry
            && head.agent == was.agent
            && !later.iter().any(|id| head.seen.contains(id))
            && !held.turns[wm.turns..].iter().any(|t| counted(&t.id))
            && held.turns.iter().any(kept) == kept_now
            && anchored(&head.turns[hm.turns - 1]) == anchored(&held.turns[wm.turns - 1])
            && self.rule_fits(head, hm.entries, wm.entries)
    }

    /// Whether the fork's rule stands where it stood: in the folded head where the head draws it, or else in the kept
    /// turns as the held fold drew it.
    fn rule_fits(&self, head: &Model, head_cut: usize, held_cut: usize) -> bool {
        let held = &self.model;
        if head.source.is_none() {
            return held.source.is_none();
        }
        let copied = |m: &Model| m.turns.iter().filter(|t| t.copied).map(|t| t.id.clone()).collect::<HashSet<String>>();
        let (in_head, in_held) = (copied(head), copied(held));
        let head_rule = head.entries[..head_cut].iter().any(|e| owner(e).is_none_or(|t| !in_head.contains(t)));
        let held_rule_later =
            held.source.is_some() && held.entries[..held_cut].iter().all(|e| owner(e).is_some_and(|t| in_held.contains(t)));
        if head_rule {
            !held_rule_later
        } else {
            held_rule_later && head.source == held.source
        }
    }

    /// Stands the head's fold and rows before the turn opening at held event `c`, and the held fold and rows from it on.
    fn join(&mut self, head: Model, was: Model, n: usize, c: usize) -> Option<Option<Splice>> {
        let held = std::mem::replace(&mut self.model, head);
        let derived = self.derive(0, 0);
        let mut head = std::mem::replace(&mut self.model, held);
        let (mut rows, Some(split)) = derived? else { return None };
        let seam = &rows.get(split)?.key;
        let held_split = self.rows.iter().position(|r| r.key == *seam)?;
        let (hm, wm) = (head.latest.take()?, was.latest.as_ref()?);
        let entry = |ix: usize| ix - wm.entries + hm.entries;
        let turn = |ix: usize| ix - wm.turns + hm.turns;
        let mut held = std::mem::take(&mut self.model);
        let starts = |counts: &HashMap<String, usize>| {
            let mut merged = hm.starts.clone();
            for (s, k) in counts {
                *merged.entry(s.clone()).or_insert(0) += k - wm.starts.get(s).copied().unwrap_or(0);
            }
            merged
        };
        let merged_starts = starts(&held.starts);
        if let Some(mark) = &mut self.mark {
            mark.event += n;
            mark.entries = entry(mark.entries);
            mark.turns = turn(mark.turns);
            mark.seen = mark.seen + hm.seen - wm.seen;
            mark.starts = starts(&mark.starts);
            mark.earlier = mark.earlier + hm.earlier - wm.earlier;
            mark.rows = mark.rows + split - held_split;
        }
        head.entries.truncate(hm.entries);
        head.entries.extend(held.entries.drain(wm.entries..));
        head.turns.truncate(hm.turns);
        head.turns.extend(held.turns.drain(wm.turns..));
        for id in held.order.drain(was.order.len()..) {
            head.seen.insert(id.clone());
            head.order.push(id);
        }
        head.starts = merged_starts;
        head.cwd = held.cwd;
        head.retry = held.retry;
        head.agent = held.agent;
        // Every later fold starts afresh or from the mark, which drops the open turn's builder and sets the event being
        // folded, so neither fold's is carried.
        head.turn = None;
        head.prompts.retain(|_, ix| *ix < hm.entries);
        head.prompts.extend(held.prompts.into_iter().filter(|(_, ix)| *ix >= wm.entries).map(|(ask, ix)| (ask, entry(ix))));
        head.runs.extend(held.runs.into_iter().filter(|(_, r)| r.event > c).map(|(block, r)| (block, Ran { event: r.event + n, ..r })));
        head.kids.extend(held.kids.into_iter().filter(|(_, k)| k.event > c).map(|(call, k)| (call, Kid { event: k.event + n, ..k })));
        head.far.extend(held.far.into_iter().filter(|(event, _)| *event > c).map(|(event, back)| {
            let back = match back {
                Far::Entry(ix) => Far::Entry(entry(ix)),
                Far::Turn(ix) => Far::Turn(turn(ix)),
                Far::Event(ix) => Far::Event(ix + n),
                Far::Unheld => Far::Unheld,
            };
            (event + n, back)
        }));
        self.model = head;
        let tail = self.rows.split_off(held_split);
        rows.truncate(split);
        let changed = splice(&self.rows, &rows);
        rows.extend(tail);
        self.rows = rows;
        #[cfg(test)]
        {
            self.grafts += 1;
        }
        Some(changed)
    }

    /// The fold work done since last asked: events folded plus entries drawn.
    #[cfg(test)]
    pub(crate) fn take_cost(&mut self) -> usize {
        std::mem::take(&mut self.cost)
    }

    pub fn push(&mut self, event: SessionEvent) -> Option<Splice> {
        self.events.push(event);
        match self.mark.take() {
            Some(mark) => self.refold(mark),
            None => self.refold_all(),
        }
    }

    pub fn toggle(&mut self, id: &str) -> Option<Splice> {
        let waiting: Vec<String> = match self.model.entries.iter().find(|e| e.key == id).map(|e| &e.body) {
            Some(Body::Subagent(s)) => s.prompts.iter().filter(|p| p.outcome.is_none()).map(|p| p.ask.clone()).collect(),
            _ => Vec::new(),
        };
        let open = if waiting.is_empty() { self.opened.contains(id) } else { waiting.iter().any(|a| !self.shut.contains(a)) };
        if open {
            self.opened.remove(id);
            self.shut.extend(waiting);
        } else {
            self.opened.insert(id.to_owned());
            waiting.iter().for_each(|a| _ = self.shut.remove(a));
        }
        self.refold_all()
    }

    pub fn can(&mut self, can: Can) -> Option<Splice> {
        if self.can == can {
            return None;
        }
        self.can = can;
        self.refold_all()
    }

    /// What the thread's agent can do, as its catalog said or the defaults until it has.
    pub fn agent_can(&self) -> Can {
        self.can
    }

    pub fn forked_from(&mut self, title: Option<String>) -> Option<Splice> {
        if self.source == title {
            return None;
        }
        self.source = title;
        self.refold_all()
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Whether the latest turn runs: Rewind to here stands on no reply while it does, which the views ask as they draw.
    pub fn working(&self) -> bool {
        self.model.working()
    }

    pub fn events(&self) -> usize {
        self.events.len()
    }

    /// Whether the fold holds a message, the person's or the agent's.
    pub fn messaged(&self) -> bool {
        self.model.entries.iter().any(|e| matches!(e.body, Body::Person { .. } | Body::Agent { .. }))
    }

    /// The folder the thread's agent works in, off its latest start, which a path a message names is read against.
    pub fn cwd(&self) -> Option<&str> {
        self.model.cwd.as_deref()
    }

    /// The rows with every turn fold, tool group, plan and subagent open, keyed as rows() would key them; changes nothing.
    pub fn unfolded(&self) -> Vec<Row> {
        if let Some(call) = &self.around.page {
            return self.page_rows(call);
        }
        let slated = self.slated();
        derive(&self.model, 0, 0, self.opened(true))
            .map(|(rows, _)| {
                let (rows, split) = slate_lines(rows, None, &slated);
                self.around.dress(rows, split, true, false, &self.model.entries).0
            })
            .unwrap_or_default()
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
        let working = self.model.working();
        self.mark = self.model.latest.take().zip(split).map(|(mark, rows)| Mark { rows, working, ..mark });
    }

    /// Folds the events from the latest turn's opening on again, keeping every row drawn before it.
    fn refold(&mut self, mark: Mark) -> Option<Splice> {
        self.model.restore(&mark);
        self.model.fresh = self.events.len() - 1;
        self.fold_from(mark.event);
        if self.model.tangled {
            return self.refold_all();
        }
        let by_count = self.can.rewinds_conversation && self.can.rewinds_by_count;
        if self.model.working() != mark.working && self.model.turns.iter().rev().skip(1).any(|t| by_count || t.kept.is_some()) {
            return self.refold_all();
        }
        let Some((tail, split)) = self.derive(mark.entries, mark.turns) else { return self.refold_all() };
        let at = mark.rows;
        let changed = splice(&self.rows[at..], &tail).map(|s| Splice { start: s.start + at, ..s });
        self.rows.truncate(at);
        self.rows.extend(tail);
        let working = self.model.working();
        self.mark = self.model.latest.take().zip(split).map(|(mark, rows)| Mark { rows: at + rows, working, ..mark });
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
        if let Some(call) = self.around.page.clone() {
            return Some((self.page_rows(&call), None));
        }
        let shut = self.around.listing.as_ref().is_some_and(|l| l.shut);
        let slated = self.slated();
        derive(&self.model, from, turns_from, self.opened(false)).map(|(rows, split)| {
            let (rows, split) = slate_lines(rows, split, &slated);
            self.around.dress(rows, split, from == 0, shut, &self.model.entries)
        })
    }

    fn opened(&self, all: bool) -> Opened<'_> {
        let listing = self.around.listing.as_ref();
        Opened { ids: &self.opened, shut: &self.shut, all, can: self.can, source: self.source.as_deref(), listing }
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

/// How many turn openings a page landing tries to fold up to before it folds the whole thread.
const GRAFT_TRIES: usize = 4;

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
    /// Whether the thread was working when these rows were drawn: a turn starting makes the reply before it an earlier one.
    working: bool,
    retry: Option<Retry>,
}

/// The retry the running turn's agent is making of a failed model call: its turn and its line.
#[derive(Clone, Default, PartialEq)]
struct Retry {
    turn: Option<String>,
    line: String,
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
            SessionEventResultStatus::Completed | SessionEventResultStatus::Unknown => State::Completed,
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
    /// A stop ended it because its computer could not be reached, `error` saying so.
    unreached: bool,
    /// The turn came with a fork's history (copiedFrom on its start).
    copied: bool,
    kept: Option<Kept>,
    changes: Option<Changes>,
    /// The reply's token count: its context and window figures.
    tokens: Option<(Option<f64>, Option<f64>)>,
    /// The latest session.context reading: the context held and the window.
    held: Option<(f64, Option<f64>)>,
}

/// A turn's checkpoint (session.checkpoint): whether it kept the files, named an anchor, and said why it kept none.
struct Kept {
    files: bool,
    anchor: bool,
    kept: bool,
}

/// What a turn changed (session.changes); `shared` with no `others` means the files are the folder's.
struct Changes {
    files: Vec<ChangedFile>,
    others: Option<Vec<ChangedFile>>,
    shared: bool,
    from: String,
    to: String,
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

#[derive(Clone, Debug)]
struct Patch {
    verb: &'static str,
    what: String,
    one: bool,
    created: bool,
    text: String,
    files: Vec<FilePatch>,
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
    /// The host kept part of the patch (`patchCut`), and the whole output's bytes when it kept part of that.
    cut: bool,
    bytes: Option<u64>,
    /// The thread the call started, off its answer, and what the call called it.
    spawned: Option<(String, String)>,
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
            cut: false,
            bytes: None,
            spawned: None,
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
    /// `turn` is the turn the message opened or was steered into, which Entry.turn leaves out.
    Person {
        text: String,
        files: Vec<FileTile>,
        turn: Option<String>,
        steered: bool,
    },
    Agent {
        text: String,
        streaming: bool,
        updated: Option<f64>,
    },
    Plan(String),
    Prompt(Box<Prompt>),
    Subagent(Box<Sub>),
    Work(Box<Work>),
}

/// PermissionPrompt in the web app's adapt/view-model.ts.
#[derive(Clone)]
struct Prompt {
    ask: String,
    session: String,
    tool: String,
    input: String,
    detail: Option<String>,
    /// Each option's id, label, effect (allow, deny, mode or answer) and the mode a mode option moves to.
    options: Vec<(String, String, String, Option<String>)>,
    outcome: Option<String>,
    picked: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubState {
    Running,
    Done,
    Failed,
    Stopped,
}

/// SubagentRun in the web app's adapt/view-model.ts: one subagent's fold, by the call that launched it.
struct Sub {
    parent: String,
    title: String,
    lines: Vec<FoldLine>,
    prompts: Vec<Prompt>,
    state: SubState,
    started: Option<f64>,
    ended: Option<f64>,
}

struct FoldLine {
    id: String,
    mono: bool,
    label: String,
    detail: Option<String>,
}

/// One of the thread's subagents as its session.subagent rows leave it, by the call that launched it.
struct Kid {
    title: String,
    state: SubState,
    summary: Option<String>,
    turn: Option<String>,
    /// The event that first named it, so a refold from a later turn forgets it.
    event: usize,
}

/// A session.run as the reply's block keeps it, and the event it came in.
struct Ran {
    run_id: String,
    command: String,
    state: String,
    output: Option<String>,
    exit_code: Option<i64>,
    signal: Option<String>,
    pty_id: Option<String>,
    event: usize,
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
    /// When the call itself answered and whether it failed; the harness's own note to the agent is no answer.
    answered: Option<(Option<f64>, bool)>,
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
    held: HashSet<String>,
    /// Where each subagent's fold sits, by the call that launched it, and each of its open calls by its line's key.
    subagents: HashMap<String, usize>,
    child_calls: HashMap<String, (String, String)>,
    plan: Option<usize>,
    keys: HashMap<String, usize>,
}

struct Outcome {
    status: SessionEventResultStatus,
    duration_ms: Option<f64>,
    waited_ms: Option<f64>,
    error: Option<String>,
    limit: bool,
    unreached: bool,
    text: Option<String>,
    tokens: Option<(Option<f64>, Option<f64>)>,
}

impl Outcome {
    fn of(result: &SessionEventResult) -> Self {
        Outcome {
            status: result.status,
            duration_ms: result.duration_ms,
            waited_ms: result.waited_ms,
            error: result.error.clone(),
            limit: result.limit.is_some(),
            unreached: result.unreached == Some(true),
            text: result.text.clone(),
            tokens: result.tokens.as_ref().map(|t| (t.context, t.window)),
        }
    }

    fn failed(error: String) -> Self {
        Outcome {
            status: SessionEventResultStatus::Failed,
            duration_ms: None,
            waited_ms: None,
            error: Some(error),
            limit: false,
            unreached: false,
            text: None,
            tokens: None,
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
    /// Where each prompt sits, its own row or the fold of the subagent that raised it, by its ask id.
    prompts: HashMap<String, usize>,
    /// Every reply block's latest run, by the block it was run from (`{message key}@{offset}`).
    runs: HashMap<String, Ran>,
    /// The thread's subagents its session.subagent rows name, by the call that launched each.
    kids: HashMap<String, Kid>,
    /// The first event opened a turn, so the thread's first message is in view.
    from_start: bool,
    /// The thread a fork's copied history came from.
    source: Option<String>,
    /// The harness the latest start that named one ran on.
    agent: Option<String>,
    /// Something since the latest turn opened ties its rows to rows before it, so only a whole fold is right.
    tangled: bool,
    latest: Option<Mark>,
    /// The retry the running turn's agent is making; gone once anything only a call that got through writes lands.
    retry: Option<Retry>,
    /// The event being folded, and the first one the rows lack: a refold's events before `fresh` are drawn already.
    folding: usize,
    fresh: usize,
    /// Each event that read or changed what an earlier turn left, by the event and how far back it reached.
    far: Vec<(usize, Far)>,
}

/// How far back an event reached: an entry, a turn, the event that named a subagent, or something no event held names.
#[derive(Clone, Copy, Debug)]
enum Far {
    Entry(usize),
    Turn(usize),
    Event(usize),
    Unheld,
}
