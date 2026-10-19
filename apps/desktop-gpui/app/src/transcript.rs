// SPDX-License-Identifier: AGPL-3.0-only
//! A thread's events folded into the transcript's rows as apps/web/src/adapt/session.ts and adapt/timeline-rows.ts fold them.
//! No GPUI here, so the fold is tested on its own.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

use crate::screens::app_shell::title::{cut_line, title_line};
use crate::screens::sidebar::model as side;
use crate::words::{
    compacted_line, fmt_duration, server_tool, server_tool_words, tool_activity_line, tool_done_line, tool_field, tool_input, tool_row,
    waited_line, ToolItem as Item, ToolRequest as Request,
};
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

/// One call inside an expanded tool group (ExpandedWorkGroupEntries in the web app's timeline/workGroup.tsx).
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
    /// Some for a shell call: drawn as a command block (CommandCallRow in timeline/callBlock.tsx).
    pub command: Option<String>,
    pub running: bool,
    pub failed: bool,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<f64>,
    /// What opening the row shows: a command's output, a tool's detail, a patch.
    pub body: Option<String>,
    /// A unified patch for an edit, drawn as a diff (EditCallRow in timeline/callBlock.tsx).
    pub patch: Option<String>,
    /// The edit's files, each drawn under its own head when there are several.
    pub files: Vec<FilePatch>,
    /// The host kept only part of the edit's patch (`patchCut`).
    pub cut: bool,
    /// The whole output's size in bytes when the host kept only its first part.
    pub bytes: Option<u64>,
    /// When a running command started, for its timer.
    pub since: Option<f64>,
    pub open: bool,
}

/// One file of an edit: its path from the thread's folder, its own unified patch and counts, and the line Open diff opens at.
#[derive(Clone, Debug, PartialEq)]
pub struct FilePatch {
    pub path: String,
    pub text: String,
    pub added: usize,
    pub removed: usize,
    /// Written whole, so its head counts its lines (`isNewFile`).
    pub created: bool,
    pub line: u64,
}

/// One choice on a prompt: a consent's option, or one answer to a question with the sentence under it.
#[derive(Clone, Debug, PartialEq)]
pub struct PromptOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    /// allow, deny, mode or answer.
    pub effect: String,
    /// The access mode a mode option moves the turn to.
    pub mode: Option<String>,
}

/// One question a call put to the person (AskedQuestion in packages/protocol/src/words/tools.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub header: Option<String>,
    pub question: String,
    pub options: Vec<PromptOption>,
    pub multiple: bool,
}

/// One line in a subagent's fold: prose, or a mono call with its answer in `detail`; `text` is empty for an answer with no call.
#[derive(Clone, Debug, PartialEq)]
pub struct SubLine {
    pub text: String,
    pub mono: bool,
    pub detail: Option<String>,
}

/// A child a call started: a thread by its id, or one of the agent's subagents.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub thread: Option<String>,
    pub title: String,
    pub note: Option<String>,
    /// A subagent's state: running, done, failed or stopped; None for a thread, whose state the listing holds.
    pub status: Option<String>,
    pub subagent: bool,
    pub settled: bool,
    /// A subagent's launching call, which its page opens by.
    pub call: Option<String>,
    /// A thread's row in the listing, once it holds one: its agent, its state and where it runs.
    pub listed: Option<Listed>,
    /// The lead's workspace, which a subagent's page opens in.
    pub workspace: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChangedFile {
    pub path: String,
    pub added: u64,
    pub removed: u64,
}

/// A reply's shell block run (`session.run`): the block's index among the reply's fenced blocks.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub run_id: String,
    pub block: usize,
    pub command: String,
    /// running, exited, moved or lost.
    pub state: String,
    pub output: Option<String>,
    pub exit_code: Option<i64>,
    pub signal: Option<String>,
    /// The pty it runs or ran in, on the workspace's daemon.
    pub pty_id: Option<String>,
}

impl Run {
    /// runEndedWords in packages/protocol/src/run-block.ts.
    pub fn words(&self) -> String {
        match (self.state.as_str(), &self.signal) {
            ("moved", _) => "Moved to a terminal tab".to_owned(),
            ("lost", _) => "Ended unseen: its terminal went away first".to_owned(),
            ("running", _) => "Running".to_owned(),
            (_, Some(signal)) => format!("Stopped by signal {signal}"),
            _ => format!("Exited {}", self.exit_code.unwrap_or(0)),
        }
    }
}

/// What the thread's agent can do, off its HarnessCatalog: Fork from here and the conversation half of Rewind to here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Can {
    pub forks: bool,
    pub forks_by_count: bool,
    pub rewinds_conversation: bool,
    pub rewinds_by_count: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterWord {
    Interrupted,
    Failed,
    LimitReached,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    /// What the person sent (row.text, newlines as breaks); `fork` is the turn Fork from here forks at, empty on the first message;
    /// `full`: the person asked for the whole of a long message, toggled by "full:" + key; `steered`: it was sent
    /// while the agent worked, which its bubble's hover says.
    Person { at: Option<f64>, files: Vec<FileTile>, fork: Option<String>, full: bool, steered: bool },
    /// An agent message; `meta`: the settled turn's final reply, with a 16 px foot and a hover-only meta line.
    Agent { at: Option<f64>, meta: bool, streaming: bool, rewind: Option<String>, fork: Option<String>, runs: Vec<Run> },
    /// "Worked for 12s" (row.text) over a settled turn's hidden work; toggles `turn`.
    Fold { open: bool },
    /// "You stopped after 4.2s" (row.text) after an interrupted turn's last row.
    Stopped,
    /// A collapsed or expanded tool group's summary line (row.text); toggles `group`.
    Group { group: String, glyph: Glyph, open: bool },
    /// The running turn's trailing calls as one line (LiveWorkEntryTimelineRow in timeline/workGroup.tsx); `active` shimmers.
    Live { group: String, glyph: Glyph, verb: Option<String>, mono: bool, since: Option<f64>, open: bool, active: bool },
    /// The calls of the open group right above it.
    Calls { group: String, calls: Vec<Call> },
    /// A standalone work row (notice, compaction, a lone non-tool entry): row.text with its tone's glyph; `body` is what
    /// opening it shows, toggled by the row's key.
    Work { tone: Tone, glyph: Glyph, body: Option<String>, open: bool },
    /// A failed turn's error (row.text), in the destructive ink, never folded.
    Error,
    /// "Working for 12s" over the running turn; `waiting`: a prompt waits on the person, so it reads "Waiting for you";
    /// `retry`: the agent is making its model call again, which the line says in its place (`retryLine`); `wait`: the
    /// workspace cannot run the turn, which the line says in its place in the rule grammar (`machineWait`).
    Working { since: Option<f64>, waiting: bool, retry: Option<String>, wait: Option<Wait> },
    /// The running turn's last row while no call is live.
    Thinking,
    /// The latest turn ended without a reply: one mono word under the transcript; `why`: a stop that never reached the
    /// computer, which says so under the word.
    Footer { word: FooterWord, why: Option<String> },
    /// A permission prompt (row.text is `lead` and `code` on one line); sessions.answer takes `session` and `ask`.
    /// `fields` are the inputs the lead leaves out; `file` is a write's body, folded away; `outcome` is a closed prompt's line.
    Permission {
        ask: String,
        session: String,
        asker: Option<String>,
        glyph: Glyph,
        lead: String,
        code: Option<String>,
        fields: Vec<(String, String)>,
        file: Option<String>,
        options: Vec<PromptOption>,
        outcome: Option<String>,
        open: bool,
    },
    /// A call that asks the person something (row.text is its first question); `outcome` once answered.
    Question { ask: String, session: String, asker: Option<String>, questions: Vec<Asked>, outcome: Option<String> },
    /// A proposed plan: row.text is the plan's markdown, whole (`plan_folds`, `plan_shown`); toggle id "plan:" + key.
    Plan { title: String, open: bool },
    /// A subagent's fold, toggled by `id`; its prompts follow it as Permission and Question rows while it is open.
    Subagent { id: String, title: String, state: Option<String>, open: bool, lines: Vec<SubLine> },
    /// The children a run of calls started, as their rows.
    Spawn { children: Vec<Child> },
    /// What a settled turn changed, under its final reply.
    /// `range` is the turn's snapshots at its launch and its end, which Open diff opens the Changes pane on.
    ChangedFiles { label: String, files: Vec<ChangedFile>, others: Vec<ChangedFile>, range: (String, String) },
    /// "Forked from {source}" (row.text) where a fork's own history starts; `open` while the listing holds that thread.
    Fork { source: String, open: Option<ThreadAt> },
    /// A sub-thread's machine start that failed, under the message it was for, with Try again.
    StartFailed { failed: StartFailed },
    /// The Threads section under the transcript: the threads this one's agent opened.
    Threads { open: bool, children: Vec<Listed> },
    /// One quiet line about the workspace in the rule grammar (row.text): the machine line.
    Rule,
    /// "Updated the slate" (row.text) under the last reply of a turn the agent wrote the slate in; a press opens it.
    Slate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileTile {
    pub name: String,
    pub bytes: Option<u64>,
    pub image: bool,
    /// What sessions.attachment fetches: the send's request id, and the file's place among its attachments.
    pub id: Option<String>,
    pub index: usize,
    pub mime: Option<String>,
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
                self.around.dress(rows, split, true, false).0
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
            self.around.dress(rows, split, from == 0, shut)
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
        E::Unknown => (None, None, None, ""),
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
            working: false,
            retry: self.retry.clone(),
        });
    }

    fn working(&self) -> bool {
        self.turns.last().is_some_and(|t| t.state == State::Running)
    }

    /// An event that changes an entry or a turn summary the latest turn's opening left behind it.
    fn reach(&mut self, entry: Option<usize>, turn: Option<usize>) {
        let Some(mark) = self.latest.as_ref().filter(|_| self.folding >= self.fresh) else { return };
        if entry.is_some_and(|ix| ix < mark.entries) || turn.is_some_and(|ix| ix < mark.turns) {
            self.tangled = true;
        }
    }

    /// An event of a turn other than the open one.
    fn elsewhere(&mut self, turn_id: Option<&str>) {
        let other = turn_id.is_none() || turn_id != self.turn.as_ref().map(|t| t.id.as_str());
        if self.folding >= self.fresh && other {
            self.tangled = true;
        }
        if other {
            let at = turn_id.and_then(|id| self.turns.iter().rposition(|t| t.id == id));
            self.far(at.map_or(Far::Unheld, Far::Turn));
        }
    }

    /// Notes an event that reaches back past the open turn's opening, which a page laid under it has to fold again.
    fn far(&mut self, back: Far) {
        let before = match (&self.latest, back) {
            (Some(m), Far::Entry(ix)) => ix < m.entries,
            (Some(m), Far::Turn(ix)) => ix < m.turns,
            (Some(m), Far::Event(ix)) => ix < m.event,
            _ => true,
        };
        if before {
            self.far.push((self.folding, back));
        }
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
        self.retry = mark.retry.clone();
        self.prompts.retain(|_, ix| *ix < mark.entries);
        self.runs.retain(|_, run| run.event < mark.event);
        self.kids.retain(|_, kid| kid.event < mark.event);
        self.far.retain(|(event, _)| *event < mark.event);
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

    fn open(&mut self, id: String, at: Option<f64>, copied: bool) {
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
            unreached: false,
            copied,
            kept: None,
            changes: None,
            tokens: None,
            held: None,
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
            held: HashSet::new(),
            subagents: HashMap::new(),
            child_calls: HashMap::new(),
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
        self.open(turn_id.map_or_else(|| format!("{session}#0"), str::to_owned), at, false);
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
        let folds: Vec<usize> = t.subagents.values().copied().collect();
        for ix in folds {
            if let Body::Subagent(s) = &mut self.entries[ix].body {
                if s.state == SubState::Running {
                    s.state = SubState::Stopped;
                    s.ended = at;
                }
            }
        }
        let t = self.turn.as_ref().expect("a result lands in a turn");
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
        summary.unreached = outcome.unreached;
        summary.tokens = outcome.tokens;
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
        self.folding = ix;
        if let SessionEvent::SessionRetry { attempt, of, why, .. } = event {
            let line = retry_line(&attempt.to_string(), of.as_ref().map(ToString::to_string).as_deref(), why.as_deref());
            self.retry = Some(Retry { turn: turn_id.map(str::to_owned), line });
        } else if turn_id.is_some() && self.retry.as_ref().is_some_and(|r| r.turn.as_deref() == turn_id) && agent_wrote(event) {
            self.retry = None;
        }
        match event {
            SessionEvent::SessionStart { prompt, after_cut, after_limit, attachments, cwd, copied_from, request_id, agent, .. } => {
                if let Some(cwd) = cwd {
                    self.cwd = Some(cwd.clone());
                }
                if agent.is_some() {
                    self.agent.clone_from(agent);
                }
                self.from_start |= ix == 0;
                self.end_running(at);
                self.checkpoint(ix);
                let count = self.starts.get(session).map_or(1, |n| n + 1);
                self.starts.insert(session.to_owned(), count);
                let id = turn_id.map_or_else(|| format!("{session}#{count}"), str::to_owned);
                self.open(id.clone(), at, copied_from.is_some());
                if self.source.is_none() {
                    self.source.clone_from(copied_from);
                }
                if let (Some(prompt), None) = (prompt, after_limit) {
                    let files = tiles(attachments, request_id.as_deref());
                    self.add_message(Body::Person { text: prompt.clone(), files, turn: Some(id), steered: false }, at);
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
            SessionEvent::SessionSteer { prompt, attachments, request_id, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                let (files, turn) = (tiles(attachments, request_id.as_deref()), self.turn.as_ref().map(|t| t.id.clone()));
                self.add_message(Body::Person { text: prompt.clone(), files, turn, steered: true }, at);
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
            SessionEvent::SessionPermission { ask_id, tool_name, tool_use_id, parent_tool_use_id, input, detail, options, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                match (tool_use_id, parent_tool_use_id) {
                    (Some(call), None) => _ = t.held.insert(call.clone()),
                    (Some(_), Some(_)) => {}
                    (None, _) => t.held.extend(t.tools.keys().cloned()),
                }
                let turn = Some(t.id.clone());
                let prompt = Prompt {
                    ask: ask_id.clone(),
                    session: session.to_owned(),
                    tool: tool_name.clone(),
                    input: input.clone(),
                    detail: detail.clone(),
                    options: options.iter().map(|o| (o.id.clone(), o.label.clone(), o.effect.to_string(), o.mode.clone())).collect(),
                    outcome: None,
                    picked: None,
                };
                if let Some(parent) = parent_tool_use_id {
                    let ix = self.fold_for(parent, at);
                    if let Body::Subagent(s) = &mut self.entries[ix].body {
                        s.prompts.push(prompt);
                    }
                    self.prompts.insert(ask_id.clone(), ix);
                    return;
                }
                self.prompts.insert(ask_id.clone(), self.entries.len());
                let body = Body::Prompt(Box::new(prompt));
                self.entries.push(Entry { key: format!("permission:{ask_id}"), turn, at, earlier: false, body });
            }
            SessionEvent::SessionPermissionClosed { ask_id, outcome, option_id, .. } => {
                let Some(&ix) = self.prompts.get(ask_id) else { return self.far(Far::Unheld) };
                self.reach(Some(ix), None);
                self.far(Far::Entry(ix));
                let close = |p: &mut Prompt| {
                    p.outcome = Some(outcome.to_string());
                    p.picked = option_id.clone();
                };
                match self.entries.get_mut(ix).map(|e| &mut e.body) {
                    Some(Body::Prompt(p)) => close(p),
                    Some(Body::Subagent(s)) => s.prompts.iter_mut().filter(|p| p.ask == *ask_id).for_each(close),
                    _ => {}
                }
            }
            SessionEvent::SessionChanges { files, others, shared, from, to, .. } => {
                let Some(at) = self.turns.iter().position(|t| Some(t.id.as_str()) == turn_id) else { return self.far(Far::Unheld) };
                self.reach(None, Some(at));
                self.far(Far::Turn(at));
                let files = files.iter().map(|f| changed_file(&f.path, f.additions, f.deletions)).collect();
                let others = (!others.is_empty()).then(|| others.iter().map(|f| changed_file(&f.path, f.additions, f.deletions)).collect());
                self.turns[at].changes = Some(Changes { files, others, shared: *shared == Some(true), from: from.clone(), to: to.clone() });
            }
            SessionEvent::SessionCheckpoint { ref_, anchor, kept, .. } => {
                let Some(at) = self.turns.iter().position(|t| Some(t.id.as_str()) == turn_id) else { return self.far(Far::Unheld) };
                self.reach(None, Some(at));
                self.far(Far::Turn(at));
                self.turns[at].kept = Some(Kept { files: ref_.is_some(), anchor: anchor.is_some(), kept: kept.is_some() });
                if kept.is_some() && self.folding >= self.fresh {
                    self.tangled = true;
                }
            }
            SessionEvent::SessionRun { block, run_id, command, state, exit_code, signal, output, pty_id, .. } => {
                self.elsewhere(turn_id);
                let ran = Ran {
                    run_id: run_id.clone(),
                    command: command.clone(),
                    state: state.to_string(),
                    output: output.clone(),
                    exit_code: *exit_code,
                    signal: signal.map(|s| s.to_string()),
                    pty_id: pty_id.clone(),
                    event: ix,
                };
                self.runs.insert(block.clone(), ran);
            }
            SessionEvent::SessionSubagent { task, state, parent_tool_use_id, title, summary, .. } => {
                let Some(parent) = parent_tool_use_id else { return };
                self.elsewhere(turn_id);
                let state = sub_state(&state.to_string());
                match self.kids.get(parent) {
                    Some(kid) => self.far(Far::Event(kid.event)),
                    // An update to a subagent no row named yet may name one an older page holds, unless this turn launched it.
                    None if state != SubState::Running
                        && !self.turn.as_ref().is_some_and(|t| t.tools.contains_key(parent) || t.subagents.contains_key(parent)) =>
                    {
                        self.far(Far::Unheld)
                    }
                    None => {}
                }
                if state == SubState::Running {
                    let held = self.kids.get(parent);
                    let kid = Kid {
                        title: title.clone().or_else(|| held.map(|k| k.title.clone())).unwrap_or_else(|| task.clone()),
                        state,
                        summary: None,
                        turn: turn_id.map(str::to_owned).or_else(|| held.and_then(|k| k.turn.clone())),
                        event: held.map_or(ix, |k| k.event),
                    };
                    self.kids.insert(parent.clone(), kid);
                } else if let Some(kid) = self.kids.get_mut(parent) {
                    kid.state = state;
                    if summary.is_some() {
                        kid.summary = summary.clone();
                    }
                }
            }
            SessionEvent::SessionContext { context, window, .. } => {
                self.turn_for(ix, turn_id, session, at);
                let t = self.turn.as_ref().expect("turn_for opened a turn");
                self.turns[t.index].held = Some((*context, *window));
            }
            SessionEvent::SessionCompacted { before, after, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                self.add_work(Work::new(compacted_line(*before, *after), Tone::Compaction, Source::Other), at);
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
                self.notice(said.clone().unwrap_or_else(|| waited_line(*ms as f64)), None, Source::Notice, at);
            }
            SessionEvent::SessionEarlier { who, text, .. } => {
                if !self.turns.is_empty() && self.folding >= self.fresh {
                    self.tangled = true;
                }
                self.earlier += 1;
                let key = format!("earlier:{session}:{}", pos.map_or(format!("n{}", self.earlier), |p| p.to_string()));
                let body = match who {
                    SessionEventWho::Person => Body::Person { text: text.clone(), files: Vec::new(), turn: None, steered: false },
                    SessionEventWho::Agent => Body::Agent { text: text.clone(), streaming: false, updated: at },
                    SessionEventWho::Note | SessionEventWho::Unknown => {
                        Body::Work(Box::new(Work::new(text.clone(), Tone::Notice, Source::Other)))
                    }
                    SessionEventWho::Tool => Body::Work(Box::new(Work::new(text.clone(), Tone::Tool, Source::Other))),
                };
                self.entries.push(Entry { key, turn: None, at, earlier: true, body });
            }
            _ => {}
        }
    }

    /// foldFor: the fold one subagent's lines gather under, which takes the place of the call that launched it.
    fn fold_for(&mut self, parent: &str, at: Option<f64>) -> usize {
        let t = self.turn.as_ref().expect("a subagent's line lands in a turn");
        if let Some(&ix) = t.subagents.get(parent) {
            return ix;
        }
        let call = t.tools.get(parent);
        let launched = call.and_then(|c| work_of(&self.entries[c.entry]).map(|w| (c.entry, w)));
        let title = call
            .and_then(|c| task_line(&c.input))
            .or_else(|| launched.and_then(|(_, w)| w.detail.clone()))
            .or_else(|| launched.map(|(_, w)| w.label.clone()))
            .unwrap_or_else(|| "Subagent".to_owned());
        let answered = call.and_then(|c| c.answered);
        let state = match answered {
            None => SubState::Running,
            Some((_, true)) => SubState::Failed,
            Some(_) => SubState::Done,
        };
        let opened = launched.map_or(at, |(ix, _)| self.entries[ix].at);
        let sub = Sub {
            parent: parent.to_owned(),
            title,
            lines: Vec::new(),
            prompts: Vec::new(),
            state,
            started: opened,
            ended: answered.and_then(|(at, _)| at),
        };
        let entry = Entry {
            key: format!("subagent:{parent}"),
            turn: Some(t.id.clone()),
            at: opened,
            earlier: false,
            body: Body::Subagent(Box::new(sub)),
        };
        let ix = match launched {
            Some((ix, _)) => {
                self.entries[ix] = entry;
                ix
            }
            None => {
                self.entries.push(entry);
                self.entries.len() - 1
            }
        };
        if let Some(t) = &mut self.turn {
            t.subagents.insert(parent.to_owned(), ix);
        }
        ix
    }

    /// addFoldLine: one line inside a fold, or the change a call's result makes to the line its input opened.
    fn fold_line(&mut self, parent: &str, at: Option<f64>, mono: bool, label: Option<String>, detail: Option<String>, key: Option<String>) {
        let ix = self.fold_for(parent, at);
        let Body::Subagent(s) = &mut self.entries[ix].body else { return };
        if let Some(line) = key.as_ref().and_then(|key| s.lines.iter_mut().find(|l| l.id == *key)) {
            line.mono = mono;
            if let Some(label) = label {
                line.label = label;
            }
            if detail.is_some() {
                line.detail = detail;
            }
            return;
        }
        let id = key.unwrap_or_else(|| format!("{}:l{}", s.parent, s.lines.len()));
        s.lines.push(FoldLine { id, mono, label: label.unwrap_or_default(), detail });
    }

    /// applyChildDelta: every line a subagent wrote, inside its own fold.
    #[allow(clippy::too_many_arguments)]
    fn child_delta(
        &mut self,
        parent: &str,
        kind: SessionEventKind,
        text: &str,
        tool: Option<&str>,
        call: Option<&str>,
        failed: bool,
        at: Option<f64>,
    ) {
        let key = call.map(|id| format!("{parent}:t{id}"));
        match kind {
            SessionEventKind::Text | SessionEventKind::Note => self.fold_line(parent, at, false, Some(text.to_owned()), None, None),
            SessionEventKind::Thinking => self.fold_line(parent, at, true, Some(text.to_owned()), None, None),
            SessionEventKind::ToolUse => {
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let open = key.as_ref().and_then(|key| t.child_calls.get(key));
                let name = tool.map(str::to_owned).or_else(|| open.map(|(name, _)| name.clone())).unwrap_or_else(|| "tool".to_owned());
                let input = open.map_or(String::new(), |(_, input)| input.clone()) + text;
                let line = tool_activity_line(Some(&name), &input);
                if let Some(key) = &key {
                    t.child_calls.insert(key.clone(), (name, input));
                }
                self.fold_line(parent, at, true, Some(line), None, key);
            }
            SessionEventKind::ToolResult => {
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let open = key.as_ref().and_then(|key| t.child_calls.remove(key));
                let did = open.filter(|_| !failed).and_then(|(name, input)| tool_done_line(Some(&name), &input));
                self.fold_line(parent, at, true, did, result_line(text, failed), key);
            }
            SessionEventKind::Unknown => {}
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
            patch_cut,
            bytes,
            ..
        } = event
        else {
            return;
        };
        if let Some(parent) = parent_tool_use_id {
            self.close();
            self.child_delta(parent, *kind, text, tool_name.as_deref(), tool_use_id.as_deref(), *is_error == Some(true), at);
            return;
        }
        let launched = tool_use_id.as_ref().filter(|id| self.turn.as_ref().is_some_and(|t| t.subagents.contains_key(*id)));
        if let (SessionEventKind::ToolResult, Some(id)) = (kind, launched) {
            if text.contains(INTERNAL_RESULT) {
                return;
            }
            let failed = *is_error == Some(true);
            if let Some(answer) = result_line(text, failed) {
                self.fold_line(id, at, false, Some(answer), None, Some(format!("answer:{id}")));
            }
            let ix = self.fold_for(id, at);
            if let Body::Subagent(s) = &mut self.entries[ix].body {
                s.state = if failed { SubState::Failed } else { SubState::Done };
                s.ended = at;
            }
            if let Some(t) = &mut self.turn {
                t.tools.remove(id);
            }
            return;
        }
        match kind {
            SessionEventKind::Unknown => {}
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
                t.tools.insert(key, Tool { entry: ix, name: tool_name.clone(), input: text.clone(), answered: None });
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
                let Some(call) = key.as_ref().and_then(|key| t.tools.get_mut(key)) else {
                    let name = tool_name.clone().unwrap_or_else(|| "tool".to_owned());
                    let mut work = Work::new(name.clone(), Tone::Tool, Source::Other);
                    work.tool = Some(name);
                    work.call = tool_use_id.clone();
                    work.status = Some(if failed_now { Status::Failed } else { Status::Completed });
                    work.detail = output;
                    self.add_work(work, at);
                    return;
                };
                let (entry_ix, name) = (call.entry, call.name.clone());
                let failed = failed_now || matches!(&self.entries[entry_ix].body, Body::Work(w) if w.status == Some(Status::Failed));
                if !note {
                    call.answered = Some((at, failed));
                }
                let held = key.as_ref().is_some_and(|key| t.held.contains(key));
                let refused = failed_now && name.as_deref().is_some_and(spawns_thread);
                let entry = &mut self.entries[entry_ix];
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
                w.status = Some(if failed { Status::Failed } else { Status::Completed });
                if !failed {
                    let title = display_label(w).text;
                    w.spawned = spawned_thread(name.as_deref(), w.command.as_deref(), text).map(|thread| (thread, title));
                }
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
                w.cut = *patch_cut == Some(true);
                w.bytes = *bytes;
            }
        }
    }
}

/// `retryLine` in packages/protocol/src/words/turn.ts.
fn retry_line(attempt: &str, of: Option<&str>, why: Option<&str>) -> String {
    let which = of.map_or_else(|| attempt.to_owned(), |of| format!("{attempt} of {of}"));
    let Some(why) = why else { return format!("Trying again ({which})") };
    let mut chars = why.chars();
    let first = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    format!("{first}{}, trying again ({which})", chars.as_str())
}

/// AGENT_WROTE in adapt/session.ts: what only an agent whose model call got through writes into its turn.
fn agent_wrote(event: &SessionEvent) -> bool {
    use SessionEvent as E;
    matches!(
        event,
        E::SessionDelta { .. }
            | E::SessionContext { .. }
            | E::SessionCompacted { .. }
            | E::SessionPlan { .. }
            | E::SessionSubagent { .. }
            | E::SessionPermission { .. }
            | E::SessionDone { .. }
            | E::SessionEnd { .. }
    )
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

fn tiles(attachments: &[SessionEventAttachmentsItem], request: Option<&str>) -> Vec<FileTile> {
    attachments
        .iter()
        .enumerate()
        .map(|(index, a)| {
            let image = matches!(a.media_type.as_str(), "image/png" | "image/jpeg" | "image/gif" | "image/webp");
            let name = a.name.clone().unwrap_or_else(|| if image { "image" } else { "file" }.to_owned());
            let (id, mime) = (request.map(str::to_owned), Some(a.media_type.clone()));
            FileTile { name, bytes: Some(a.bytes), image, id, index, mime }
        })
        .collect()
}

fn changed_file(path: &str, added: f64, removed: f64) -> ChangedFile {
    ChangedFile { path: path.to_owned(), added: added.max(0.0) as u64, removed: removed.max(0.0) as u64 }
}

fn sub_state(state: &str) -> SubState {
    match state {
        "done" => SubState::Done,
        "failed" => SubState::Failed,
        "stopped" => SubState::Stopped,
        _ => SubState::Running,
    }
}

fn sub_word(state: SubState) -> &'static str {
    match state {
        SubState::Running => "running",
        SubState::Done => "done",
        SubState::Failed => "failed",
        SubState::Stopped => "stopped",
    }
}

/// lastLine in packages/protocol/src/words/base.ts.
fn last_line(text: &str) -> Option<String> {
    text.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
}

const LISTED_LINE_CHARS: usize = 200;

/// listedLastLine in packages/protocol/src/words/turn.ts: a list marker leading the line goes.
fn listed_last_line(text: &str) -> Option<String> {
    let last = last_line(text)?;
    let marker = ["- ", "* ", "+ "].iter().find_map(|m| last.strip_prefix(m)).or_else(|| {
        let digits = last.chars().take_while(char::is_ascii_digit).count();
        let rest = &last[digits..];
        (1..=9).contains(&digits).then(|| rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))).flatten()
    });
    let line = marker.map(str::trim_start).filter(|m| !m.is_empty()).unwrap_or(&last);
    Some(cut_line(line, LISTED_LINE_CHARS))
}

/// listedFailure in packages/protocol/src/words/turn.ts.
fn listed_failure(why: &str) -> String {
    let first = why.lines().find(|l| !l.trim().is_empty()).map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "));
    cut_line(first.as_deref().unwrap_or("turn ended without a result"), LISTED_LINE_CHARS)
}

/// toolResultLine in packages/protocol/src/words/tools.ts.
fn result_line(text: &str, failed: bool) -> Option<String> {
    if text.contains(INTERNAL_RESULT) {
        return None;
    }
    let first = title_line(text);
    match (failed, first.is_empty()) {
        (false, true) => None,
        (false, false) => Some(first),
        (true, true) => Some("failed".to_owned()),
        (true, false) => Some(format!("failed: {first}")),
    }
}

/// subagentTaskLine in packages/protocol/src/words/tools.ts.
fn task_line(input: &str) -> Option<String> {
    tool_input(input).and_then(|fields| tool_field(&fields, "description").map(title_line))
}

fn change_paths(fields: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let changes = fields.get("changes").and_then(serde_json::Value::as_array);
    changes.into_iter().flatten().filter_map(|c| c.get("path")?.as_str().filter(|p| !p.is_empty()).map(str::to_owned)).collect()
}

/// spawnedThreadOf in the web app's adapt/spawned.ts: the thread a finished call that starts one names in its answer.
fn spawned_thread(tool: Option<&str>, command: Option<&str>, answer: &str) -> Option<String> {
    (tool.is_some_and(spawns_thread) || command.is_some_and(starts_thread)).then(|| named_thread(answer)).flatten()
}

/// wsp's run or fork at any path, as a command of its own in a chain, inside a `sh -c` script where Codex wraps one.
fn starts_thread(command: &str) -> bool {
    let trimmed = command.trim();
    let script = trimmed.split_once(char::is_whitespace).and_then(|(shell, rest)| {
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("-lc").or_else(|| rest.strip_prefix("-c"))?.trim_start();
        let quote = rest.chars().next().filter(|q| *q == '\'' || *q == '"')?;
        (shell.ends_with("sh") && rest.len() >= 2 && rest.ends_with(quote)).then(|| &rest[1..rest.len() - 1])
    });
    let mut bare = String::new();
    let mut quote: Option<char> = None;
    for c in script.unwrap_or(trimmed).chars() {
        match quote {
            Some(q) if c == q => {
                quote = None;
                bare.push_str("''");
            }
            Some(_) => {}
            None if c == '\'' || c == '"' => quote = Some(c),
            None => bare.push(c),
        }
    }
    let words: Vec<&str> =
        bare.split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')')).filter(|w| !w.is_empty()).collect();
    words.windows(2).any(|pair| (pair[0] == "wsp" || pair[0].ends_with("/wsp")) && matches!(pair[1], "run" | "fork"))
}

fn named_thread(answer: &str) -> Option<String> {
    let thread_in = |text: &str, parsed: Option<serde_json::Value>| {
        let parsed = parsed.filter(serde_json::Value::is_object);
        let id =
            parsed.as_ref().and_then(|v| v.get("threadId").and_then(|t| t.as_str()).or_else(|| v.get("turn")?.get("threadId")?.as_str()));
        id.map(str::to_owned).or_else(|| {
            text.lines().find_map(|l| l.strip_prefix("thread ")).and_then(|rest| rest.split_whitespace().next()).map(str::to_owned)
        })
    };
    let parse = |text: &str| serde_json::from_str::<serde_json::Value>(text).ok();
    match parse(answer) {
        Some(serde_json::Value::Array(blocks)) => blocks.iter().find_map(|b| b.get("text")?.as_str().and_then(|t| thread_in(t, parse(t)))),
        whole => {
            let parsed = whole.or_else(|| parse(answer.trim().lines().last().unwrap_or("")));
            thread_in(answer, parsed)
        }
    }
}

/// One question of a call to the question tool (questionsIn in packages/protocol/src/words/tools.ts).
fn questions_in(fields: &serde_json::Map<String, serde_json::Value>) -> Option<Vec<Asked>> {
    let raw = fields.get("questions")?.as_array()?;
    let mut questions = Vec::new();
    for (at, entry) in raw.iter().enumerate() {
        let Some(q) = entry.as_object() else { continue };
        let Some(text) = tool_field(q, "question") else { continue };
        let choices = q.get("options").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(|o| o.as_object());
        let mut options: Vec<PromptOption> = Vec::new();
        for o in choices {
            let Some(label) = tool_field(o, "label") else { continue };
            let description = tool_field(o, "description").map(str::to_owned);
            let id = format!("q{at}:o{}", options.len());
            options.push(PromptOption { id, label: label.to_owned(), description, effect: "answer".to_owned(), mode: None });
        }
        let header = tool_field(q, "header").map(str::to_owned);
        let multiple = q.get("multiSelect").and_then(serde_json::Value::as_bool) == Some(true);
        questions.push(Asked { header, question: text.to_owned(), options, multiple });
    }
    (!questions.is_empty()).then_some(questions)
}

/// folderName and parentFolderName in packages/protocol/src/project-path.ts.
fn file_in_folder(path: &str) -> String {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    let name = parts.last().copied().unwrap_or(path);
    match parts.len() {
        0 | 1 => name.to_owned(),
        n if parts[n - 2].is_empty() => name.to_owned(),
        n => format!("{name} in {}", parts[n - 2]),
    }
}

/// permissionPromptWords in packages/protocol/src/words/tools.ts: the lead, the unsaid inputs, a write's body, the questions.
struct Words {
    says: String,
    code: Option<String>,
    rest: Vec<(String, String)>,
    body: Option<String>,
    questions: Option<Vec<Asked>>,
}

/// A prompt's lead: its words, and the call's own text where it has some.
type Lead = (String, Option<String>);

fn prompt_words(tool: &str, input: &str, detail: Option<&str>) -> Words {
    let fields = tool_input(input);
    let field = |name: &str| fields.as_ref().and_then(|f| tool_field(f, name)).map(str::to_owned);
    let server = server_tool_words(tool);
    let (lead, named, body): (Option<Lead>, &[&str], Option<&str>) = match tool {
        "Write" => {
            let size =
                fields.as_ref().and_then(|f| f.get("content")?.as_str()).map(|c| format!(" ({})", crate::ui::fmt_bytes(c.len() as u64)));
            (
                field("file_path").map(|p| (format!("Write {}{}", file_in_folder(&p), size.unwrap_or_default()), None)),
                &["file_path", "content"],
                Some("content"),
            )
        }
        "Edit" | "MultiEdit" => {
            let edits = fields.as_ref().and_then(|f| f.get("edits")?.as_array().map(Vec::len)).filter(|n| *n > 1);
            let count = edits.map_or(String::new(), |n| format!(" ({n} places)"));
            let lead = field("file_path").map(|p| (format!("Edit {}{count}", file_in_folder(&p)), None));
            (lead, &["file_path", "old_string", "new_string", "replace_all", "edits"], None)
        }
        "Bash" => (field("command").map(|c| ("Run a command:".to_owned(), Some(c))), &["command", "description"], None),
        "command_execution" => (field("command").map(|c| ("Run a command:".to_owned(), Some(c))), &["command", "cwd"], None),
        "WebFetch" => (field("url").map(|u| ("Fetch a page:".to_owned(), Some(u))), &["url", "prompt"], None),
        "Skill" => (field("skill").map(|k| (format!("Run the skill {k}"), None)), &["skill"], None),
        "AskUserQuestion" => {
            let first = fields.as_ref().and_then(questions_in).and_then(|q| q.into_iter().next());
            (first.map(|q| (q.question, None)), &["questions"], None)
        }
        "file_change" => {
            let paths: Vec<String> = fields.as_ref().map(change_paths).unwrap_or_default();
            let says = match paths.as_slice() {
                [] => None,
                [one] => Some(format!("Change {}", file_in_folder(one))),
                many => {
                    let names: Vec<&str> = many.iter().map(|p| p.trim_end_matches('/').rsplit('/').next().unwrap_or(p)).collect();
                    Some(format!("Change {} files: {}", many.len(), names.join(", ")))
                }
            };
            (says.map(|s| (s, None)), &["changes"], None)
        }
        _ => (server.clone().filter(|_| fields.is_some()).map(|s| (s, None)), &[], None),
    };
    let (says, code) = lead.unwrap_or_else(|| match detail.filter(|d| !d.is_empty()) {
        Some(detail) => (format!("Permission for {tool}: {detail}"), None),
        None => (format!("Permission for {tool}"), None),
    });
    let questions = (tool == "AskUserQuestion").then(|| fields.as_ref().and_then(questions_in)).flatten();
    let Some(fields) = fields.filter(|_| questions.is_none()) else {
        let rest = if questions.is_some() || input.is_empty() { Vec::new() } else { vec![(String::new(), input.to_owned())] };
        return Words { says, code, rest, body: None, questions };
    };
    let rest =
        fields.iter().filter(|(key, _)| !named.contains(&key.as_str())).map(|(key, value)| (key.clone(), rest_value(value))).collect();
    let body = body.and_then(|name| tool_field(&fields, name)).map(str::to_owned);
    Words { says, code, rest, body, questions: None }
}

/// restValue: one input value as the row reads it, its whitespace collapsed.
fn rest_value(value: &serde_json::Value) -> String {
    let text = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) if items.iter().all(|v| v.is_string() || v.is_number()) => {
            items.iter().map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned)).collect::<Vec<_>>().join(", ")
        }
        other => other.to_string(),
    };
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The glyph a prompt's record wears (recordGlyph in PermissionPromptRow.tsx).
fn prompt_glyph(tool: &str, input: &str) -> Glyph {
    let found = facts(tool, input, None);
    if found.request == Some(Request::Command) || found.item == Some(Item::Command) {
        Glyph::Terminal
    } else if found.request == Some(Request::FileChange) || found.item == Some(Item::FileChange) {
        Glyph::SquarePen
    } else if found.item == Some(Item::WebSearch) {
        Glyph::Globe
    } else {
        Glyph::Wrench
    }
}

/// permissionOutcomeLine and pickedOptions in packages/protocol/src/words/tools.ts.
fn outcome_line(p: &Prompt) -> Option<String> {
    let outcome = p.outcome.as_deref()?;
    let asks = p.options.iter().all(|(_, _, effect, _)| effect == "answer");
    let picked: Option<Vec<(String, String)>> = p
        .picked
        .as_deref()
        .unwrap_or("")
        .split('|')
        .map(|id| {
            p.options.iter().find(|(o, _, _, _)| o == id).map(|(_, label, effect, _)| (label.clone(), effect.clone())).or_else(|| {
                let typed = id.strip_prefix('q')?.split_once(":other:")?;
                (asks && typed.0.parse::<u32>().is_ok()).then(|| unescape(typed.1)).flatten().map(|words| (words, "answer".to_owned()))
            })
        })
        .collect();
    let named =
        picked.filter(|p| !p.is_empty()).map(|p| (p.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join(", "), p[0].1.clone()));
    let mode = named.as_ref().filter(|(_, effect)| effect == "mode").map_or(String::new(), |(label, _)| format!(": {label}"));
    Some(match outcome {
        "allowed" => match &named {
            Some((label, effect)) if effect == "answer" => format!("You answered: {label}"),
            _ => format!("Allowed{mode}"),
        },
        "denied" => format!("Denied{mode}"),
        "unanswered" => "Nobody answered; denied".to_owned(),
        "cancelled" => "Cancelled with the turn".to_owned(),
        _ => String::new(),
    })
}

/// decodeURIComponent: None where an escape is not whole UTF-8.
fn unescape(text: &str) -> Option<String> {
    let mut out = Vec::new();
    let raw = text.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            out.push(u8::from_str_radix(text.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `askingLine`: a prompt's lead as one line, the call's own text after its words.
pub(crate) fn asking_line(tool: &str, input: &str, detail: Option<&str>) -> String {
    let words = prompt_words(tool, input, detail);
    match words.code {
        Some(code) => format!("{} {code}", words.says),
        None => words.says,
    }
}

/// One prompt as its row: a question the call puts to the person, or the consent it asks for.
fn prompt_row(key: &str, turn: Option<String>, p: &Prompt, asker: Option<String>) -> Row {
    let words = prompt_words(&p.tool, &p.input, p.detail.as_deref());
    let outcome = outcome_line(p);
    let (ask, session) = (p.ask.clone(), p.session.clone());
    if let Some(questions) = words.questions {
        let kind = RowKind::Question { ask, session, asker, questions, outcome };
        return Row { key: key.to_owned(), kind, text: words.says, turn };
    }
    let text = match &words.code {
        Some(code) => format!("{} {code}", words.says),
        None => words.says.clone(),
    };
    let options = p
        .options
        .iter()
        .map(|(id, label, effect, mode)| PromptOption {
            id: id.clone(),
            label: label.clone(),
            description: None,
            effect: effect.clone(),
            mode: mode.clone(),
        })
        .collect();
    let kind = RowKind::Permission {
        ask,
        session,
        asker,
        glyph: prompt_glyph(&p.tool, &p.input),
        lead: words.says,
        code: words.code,
        fields: words.rest,
        file: words.body,
        options,
        open: outcome.is_none(),
        outcome,
    };
    Row { key: key.to_owned(), kind, text, turn }
}

/// proposedPlanTitle in the web app's lib/proposedPlan.ts: the plan's first heading.
fn plan_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(heading).filter(|h| !h.is_empty())
}

/// A markdown heading's words, `^\s{0,3}#{1,6}\s+(.+)$`; Some("") for a heading with none.
fn heading(line: &str) -> Option<String> {
    let indent = line.len() - line.trim_start().len();
    let rest = &line[indent..];
    let hashes = rest.len() - rest.trim_start_matches('#').len();
    let text = &rest[hashes..];
    (indent <= 3 && (1..=6).contains(&hashes) && text.starts_with(char::is_whitespace)).then(|| text.trim().to_owned())
}

/// A plan the card shows collapsed until opened: over 900 characters or 20 lines.
pub fn plan_folds(markdown: &str) -> bool {
    markdown.encode_utf16().count() > 900 || markdown.split('\n').count() > 20
}

/// What the plan card draws: the plan less its title and a leading Summary heading, or while a long plan is
/// collapsed its first ten lines and "..." (stripDisplayedPlanMarkdown and buildCollapsedProposedPlanPreviewMarkdown).
pub fn plan_shown(markdown: &str, open: bool) -> String {
    let lines: Vec<&str> = markdown.trim_end().lines().collect();
    let mut source: Vec<&str> = match lines.first() {
        Some(first) if heading(first).is_some() => lines[1..].to_vec(),
        _ => lines.clone(),
    };
    let drop_blank = |source: &mut Vec<&str>| {
        while source.first().is_some_and(|l| l.trim().is_empty()) {
            source.remove(0);
        }
    };
    drop_blank(&mut source);
    if source.first().and_then(|l| heading(l)).is_some_and(|h| h.to_lowercase() == "summary") {
        source.remove(0);
        drop_blank(&mut source);
    }
    if open || !plan_folds(markdown) {
        return source.join("\n");
    }
    let mut preview: Vec<&str> = Vec::new();
    let (mut seen, mut more) = (0, false);
    for line in source.iter().map(|l| l.trim_end()) {
        let visible = !line.trim().is_empty();
        if visible && seen >= 10 {
            more = true;
            break;
        }
        preview.push(line);
        seen += usize::from(visible);
    }
    while preview.last().is_some_and(|l| l.trim().is_empty()) {
        preview.pop();
    }
    if preview.is_empty() {
        return plan_title(markdown).unwrap_or_else(|| "Plan preview unavailable.".to_owned());
    }
    if more {
        preview.extend(["", "..."]);
    }
    preview.join("\n")
}

/// The reply's fenced block opening on the line holding `offset`, in UTF-16 units (runBlockKey in protocol/src/run-block.ts).
pub(crate) fn block_at(text: &str, offset: usize) -> Option<usize> {
    fence_openings(text).iter().position(|opening| (opening.at..=opening.at + opening.width).contains(&offset))
}

/// A fenced block's opening line: its byte offset, and its offset and width in UTF-16 units.
pub(crate) struct FenceOpening {
    pub byte: usize,
    pub at: usize,
    pub width: usize,
}

/// Each fenced block's opening line in a reply, in order, by runBlockKey's rules.
pub(crate) fn fence_openings(text: &str) -> Vec<FenceOpening> {
    let (mut byte, mut at) = (0, 0);
    let mut open: Option<(char, usize)> = None;
    let mut openings = Vec::new();
    for line in text.split('\n') {
        let width = line.encode_utf16().count();
        let bare = line.trim_start_matches([' ', '>', '\t']);
        let fence = bare.chars().next().filter(|c| *c == '`' || *c == '~').map(|c| (c, bare.chars().take_while(|d| *d == c).count()));
        match (open, fence) {
            (Some((c, n)), Some((d, m))) if c == d && m >= n && bare.trim_end().chars().all(|e| e == c) => open = None,
            (Some(_), _) => {}
            (None, Some((c, n))) if n >= 3 && !(c == '`' && bare[n..].contains('`')) => {
                openings.push(FenceOpening { byte, at, width });
                open = Some((c, n));
            }
            (None, _) => {}
        }
        byte += line.len() + 1;
        at += width + 1;
    }
    openings
}

/// compactLines in adapt/session.ts: each non-blank line with its runs of space as one.
fn compact_lines(text: &str) -> Vec<String> {
    text.lines().map(|line| line.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|line| !line.is_empty()).collect()
}

/// summarizeOutput in adapt/session.ts: the first line worth reading, cut at 84 characters.
fn preview(text: &str) -> Option<String> {
    let mut fences = 0;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let Some(first) = words.next() else { continue };
        if first == "```" && words.clone().next().is_none() {
            fences += 1;
            continue;
        }
        // Stops at 85 characters: a thinking block's preview runs again on every delta.
        let mut said = String::new();
        let mut count = 0;
        for word in std::iter::once(first).chain(words) {
            if count > 0 {
                said.push(' ');
                count += 1;
            }
            for ch in word.chars().take(85 - count) {
                said.push(ch);
                count += 1;
            }
            if count > 84 {
                break;
            }
        }
        if count <= 84 {
            return Some(said);
        }
        let cut: String = said.chars().take(83).collect();
        return Some(format!("{}…", cut.trim_end()));
    }
    (fences > 1).then(|| format!("{fences} lines"))
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
    let mut each = Vec::new();
    for file in files {
        let from = text.len();
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
        let lines = file.hunks.iter().flat_map(|h| h.lines.iter());
        let (added, removed) = lines.fold((0, 0), |(a, r), l| match l.as_bytes().first() {
            Some(b'+') => (a + 1, r),
            Some(b'-') => (a, r + 1),
            _ => (a, r),
        });
        each.push(FilePatch {
            path: relative(file.moved_to.as_deref().unwrap_or(&file.path), cwd),
            text: text[from..].join("\n"),
            added,
            removed,
            created: created(file),
            line: file.hunks.first().map_or(1, |h| (h.new_start as u64).max(1)),
        });
    }
    Some(Patch { verb, what, one: files.len() == 1, created: new, text: text.join("\n"), files: each })
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

const SHOWN_FIELDS: [&str; 7] = ["file_path", "notebook_path", "pattern", "query", "url", "description", "prompt"];

fn facts(name: &str, input: &str, cwd: Option<&str>) -> Facts {
    let row = tool_row(name);
    let lent = if row.is_none() { server_tool_words(name) } else { None };
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

fn group_id(e: &Entry, w: &Work) -> String {
    match &w.call {
        Some(call) => format!("group:tool:{}:{call}", e.turn.as_deref().unwrap_or("no-turn")),
        None => format!("group:{}", e.key),
    }
}

/// A work entry standing on its own line: a severe error in the destructive ink, anything else in its tone, opening
/// to its body where it has one (PlainWorkEntryRow).
fn work_row(key: &str, e: &Entry, w: &Work, opened: &Opened) -> Row {
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

/// What derive treats as open: the ids the person opened, or with `all` every fold and group (a call still by its id).
#[derive(Clone, Copy)]
struct Opened<'a> {
    ids: &'a HashSet<String>,
    shut: &'a HashSet<String>,
    all: bool,
    can: Can,
    source: Option<&'a str>,
    /// What the listing holds: a call stands as its child's row only where the listing holds that child.
    listing: Option<&'a Listing>,
}

impl Opened<'_> {
    fn contains(&self, id: &str) -> bool {
        self.all || self.ids.contains(id)
    }
}

/// deriveMessagesTimelineRows from `from`: the rows and how many precede the latest turn; None if the working line lands earlier.
fn derive(model: &Model, from: usize, turns_from: usize, opened: Opened) -> Option<(Vec<Row>, Option<usize>)> {
    let entries = &model.entries;
    let len = entries.len();
    let by_id: HashMap<&str, &Summary> = model.turns[turns_from..].iter().map(|s| (s.id.as_str(), s)).collect();
    let latest = model.turns.last();
    let unsettled = latest.filter(|t| t.state == State::Running).map(|t| t.id.as_str());
    let working = unsettled.is_some();
    let boundary = model.latest.as_ref().map(|m| m.entries);
    let turn_of = |ix: usize| entries[ix].turn.as_deref();

    // A fork's rule stands before its first entry of no turn it was handed: its own first message, or the end.
    let copied: HashSet<&str> = model.turns.iter().filter(|t| t.copied).map(|t| t.id.as_str()).collect();
    let rule_at = (!copied.is_empty())
        .then(|| (0..len).find(|&ix| owner(&entries[ix]).is_none_or(|t| !copied.contains(t))).unwrap_or(len))
        .filter(|&at| at >= from);
    let source = opened.source.or(model.source.as_deref()).unwrap_or_default();

    // Rewind to here and Fork from here (rewindableReplies in RewindDialog.tsx, forkableMessages in forks.ts).
    let can = opened.can;
    let at_of: HashMap<&str, usize> = model.turns.iter().enumerate().skip(turns_from).map(|(i, t)| (t.id.as_str(), i)).collect();
    let kept_why = model.turns.iter().any(|t| t.kept.as_ref().is_some_and(|k| k.kept));
    let anchored = |t: &Summary| t.kept.as_ref().is_some_and(|k| k.anchor);
    let cuts = |t: &Summary| can.forks_by_count || anchored(t);
    let rewind_of = |i: usize| {
        let t = &model.turns[i];
        let files = t.kept.as_ref().is_some_and(|k| k.files);
        let conversation = !kept_why && can.rewinds_conversation && (anchored(t) || can.rewinds_by_count);
        let earlier = i + 1 < model.turns.len() && t.state != State::Running && !t.copied;
        (earlier && (files || conversation)).then(|| t.id.clone())
    };
    let reply_fork = |i: usize| {
        let t = &model.turns[i];
        (can.forks && t.state != State::Running && cuts(t)).then(|| t.id.clone())
    };
    let opener_fork = |i: usize| {
        if !can.forks || model.turns[i].state == State::Running {
            return None;
        }
        match i.checked_sub(1) {
            None => (model.from_start && model.earlier == 0).then(String::new),
            Some(before) => cuts(&model.turns[before]).then(|| model.turns[before].id.clone()),
        }
    };
    let mut last_reply: HashMap<&str, usize> = HashMap::new();
    let mut openers: HashSet<usize> = HashSet::new();
    let mut opened_turns: HashSet<&str> = HashSet::new();
    for (ix, e) in entries.iter().enumerate().skip(from) {
        match &e.body {
            Body::Agent { .. } => {
                if let Some(turn) = e.turn.as_deref() {
                    last_reply.insert(turn, ix);
                }
            }
            Body::Person { turn: Some(turn), steered: false, .. } if opened_turns.insert(turn.as_str()) => _ = openers.insert(ix),
            _ => {}
        }
    }
    let mut runs: HashMap<&str, Vec<(usize, &Ran)>> = HashMap::new();
    for (block, ran) in &model.runs {
        if let Some((message, offset)) = block.rsplit_once('@').and_then(|(m, o)| Some((m, o.parse::<usize>().ok()?))) {
            runs.entry(message).or_default().push((offset, ran));
        }
    }

    // The children calls started: spawnCallOf in adapt/spawned.ts, with the subagents session.subagent names.
    let child_of = |call: &String, kid: &Kid| {
        let settled = kid.state != SubState::Running && !(working && kid.turn.as_deref() == unsettled);
        let note = match (settled, kid.state, kid.summary.as_deref()) {
            (false, SubState::Failed, Some(why)) => Some(listed_failure(why)),
            (false, SubState::Done, Some(said)) => listed_last_line(said),
            _ => None,
        };
        let status = Some(sub_word(kid.state).to_owned());
        Child {
            thread: None,
            title: kid.title.clone(),
            note,
            status,
            subagent: true,
            settled,
            call: Some(call.clone()),
            listed: None,
            workspace: None,
        }
    };
    // spawnCallOf: a call is its child's row only where the listing holds that child, else it stands as itself.
    let holds_thread = |thread: &str| opened.listing.is_some_and(|l| l.child(thread).is_some());
    let holds_call = |call: &str| opened.listing.is_some_and(|l| l.subagent(call).is_some());
    let spawn_of = |ix: usize| match &entries[ix].body {
        Body::Subagent(s) if holds_call(&s.parent) => model.kids.get(&s.parent).map(|kid| child_of(&s.parent, kid)),
        Body::Subagent(_) => None,
        Body::Work(w) => match &w.spawned {
            Some((thread, title)) if holds_thread(thread) => Some(Child {
                thread: Some(thread.clone()),
                title: title.clone(),
                note: None,
                status: None,
                subagent: false,
                settled: false,
                call: None,
                listed: None,
                workspace: None,
            }),
            _ => w.call.as_ref().filter(|call| holds_call(call)).and_then(|call| model.kids.get(call).map(|kid| child_of(call, kid))),
        },
        _ => None,
    };

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
            let label = worked.map_or("You stopped this response".to_owned(), |ms| format!("You stopped after {}", fmt_duration(ms)));
            stops.push((last, *turn, label));
            continue;
        }
        let kept = |ix: &usize| {
            terminal.contains(ix) || matches!(&entries[*ix].body, Body::Plan(_)) || work_of(&entries[*ix]).is_some_and(stands_alone)
        };
        let hidden: Vec<usize> = members.iter().copied().filter(|ix| !kept(ix)).collect();
        let Some(&first_hidden) = hidden.first() else { continue };
        let label = worked.map_or("Worked".to_owned(), |ms| format!("Worked for {}", fmt_duration(ms)));
        folds.insert(first_hidden, (*turn, label));
        if !opened.contains(turn) {
            for ix in hidden {
                collapsed[ix - from] = true;
            }
        }
    }
    stops.sort_by_key(|(last, _, _)| *last);
    let tail_earlier = entries[from..].iter().filter(|e| e.earlier).count();
    let first_earlier = (model.earlier == tail_earlier).then(|| (from..len).find(|&ix| entries[ix].earlier)).flatten();
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
                Some(w) if turn_of(ix) == unsettled && !stands_alone(w) && spawn_of(ix).is_none() => trailing.push(ix),
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
        && entries[from..].iter().any(|e| e.turn.as_deref() == unsettled && matches!(&e.body, Body::Prompt(p) if p.outcome.is_none()));
    let retry = model.retry.as_ref().filter(|r| working && r.turn.as_deref() == unsettled).map(|r| r.line.clone());
    let working_row = || Row {
        key: "working".to_owned(),
        kind: RowKind::Working { since: latest.and_then(|t| t.started), waiting, retry: retry.clone(), wait: None },
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
        if rule_at == Some(ix) {
            let kind = RowKind::Fork { source: source.to_owned(), open: None };
            rows.push(Row { key: "fork-rule".to_owned(), kind, text: format!("Forked from {source}"), turn: None });
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
                rows.push(calls_row(&live.group, &live.calls, entries, opened.ids));
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
            let kind = RowKind::Fold { open: opened.contains(turn) };
            rows.push(Row { key: format!("fold:{turn}"), kind, text: label.clone(), turn: Some((*turn).to_owned()) });
        }
        if collapsed[ix - from] || taken.contains(&ix) {
            ix += 1;
            continue;
        }
        if let Some(first) = spawn_of(ix) {
            let mut children = vec![first];
            let mut end = ix + 1;
            while end < len && !collapsed[end - from] && !taken.contains(&end) && !folds.contains_key(&end) && entries[end].turn == e.turn {
                let Some(child) = spawn_of(end) else { break };
                children.push(child);
                end += 1;
            }
            rows.push(Row {
                key: format!("spawn:{}", e.key),
                kind: RowKind::Spawn { children },
                text: String::new(),
                turn: e.turn.clone(),
            });
            ix = end;
            continue;
        }
        match &e.body {
            Body::Work(w) if stands_alone(w) => rows.push(work_row(&e.key, e, w, &opened)),
            Body::Work(_) => {
                let mut end = ix + 1;
                while end < len
                    && work_of(&entries[end]).is_some_and(|w| !stands_alone(w))
                    && !taken.contains(&end)
                    && !collapsed[end - from]
                    && !folds.contains_key(&end)
                    && entries[end].turn == e.turn
                    && spawn_of(end).is_none()
                {
                    end += 1;
                }
                let members: Vec<usize> =
                    (ix..end).filter(|&k| work_of(&entries[k]).is_some_and(|w| visible_in_group(w, in_run(k, w)))).collect();
                let works: Vec<&Work> = members.iter().filter_map(|&k| work_of(&entries[k])).collect();
                match works.as_slice() {
                    [] => {}
                    [only] if !tool_like(only) => rows.push(work_row(&e.key, &entries[members[0]], only, &opened)),
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
                            rows.push(calls_row(&group, &members, entries, opened.ids));
                        }
                    }
                }
                ix = end;
                continue;
            }
            Body::Plan(text) => {
                let title = plan_title(text).unwrap_or_else(|| "Proposed plan".to_owned());
                let kind = RowKind::Plan { title, open: opened.contains(&format!("plan:{}", e.key)) };
                rows.push(Row { key: e.key.clone(), kind, text: text.clone(), turn: e.turn.clone() });
            }
            Body::Prompt(p) => rows.push(prompt_row(&e.key, e.turn.clone(), p, None)),
            Body::Subagent(s) => {
                let mut waiting = s.prompts.iter().filter(|p| p.outcome.is_none()).peekable();
                let open = match waiting.peek() {
                    Some(_) => opened.all || waiting.any(|p| !opened.shut.contains(&p.ask)),
                    None => opened.contains(&e.key),
                };
                let word = match s.state {
                    SubState::Running => None,
                    SubState::Done => Some("Done"),
                    SubState::Failed => Some("Failed"),
                    SubState::Stopped => Some("Stopped"),
                };
                let took = s.started.zip(s.ended).map(|(from, to)| to - from).filter(|ms| *ms > 0.0).map(fmt_duration);
                let state = match (word, took) {
                    (Some(word), Some(took)) => Some(format!("{word} {took}")),
                    (word, took) => word.map(str::to_owned).or(took),
                };
                let lines = s.lines.iter().map(|l| SubLine { text: l.label.clone(), mono: l.mono, detail: l.detail.clone() }).collect();
                let kind = RowKind::Subagent { id: e.key.clone(), title: s.title.clone(), state, open, lines };
                rows.push(Row { key: e.key.clone(), kind, text: s.title.clone(), turn: e.turn.clone() });
                if open {
                    for p in &s.prompts {
                        let asker = Some(format!("{} asks", s.title));
                        rows.push(prompt_row(&format!("permission:{}", p.ask), e.turn.clone(), p, asker));
                    }
                }
            }
            Body::Person { text, files, turn, steered } => {
                let fork = turn.as_deref().and_then(|t| at_of.get(t)).filter(|_| openers.contains(&ix)).and_then(|&i| opener_fork(i));
                rows.push(Row {
                    key: e.key.clone(),
                    kind: RowKind::Person {
                        at: e.at,
                        files: files.clone(),
                        fork,
                        full: opened.ids.contains(&format!("full:{}", e.key)),
                        steered: *steered,
                    },
                    text: text.clone(),
                    turn: None,
                });
            }
            Body::Agent { text, streaming, updated } => {
                let still = e.turn.is_some() && e.turn.as_deref() == unsettled;
                let last = e.turn.as_deref().filter(|t| last_reply.get(t) == Some(&ix));
                let at = last.and_then(|t| at_of.get(t).copied());
                let mut ran: Vec<Run> = runs
                    .get(e.key.as_str())
                    .into_iter()
                    .flatten()
                    .filter_map(|(offset, r)| {
                        Some(Run {
                            run_id: r.run_id.clone(),
                            block: block_at(text, *offset)?,
                            command: r.command.clone(),
                            state: r.state.clone(),
                            output: r.output.clone(),
                            exit_code: r.exit_code,
                            signal: r.signal.clone(),
                            pty_id: r.pty_id.clone(),
                        })
                    })
                    .collect();
                ran.sort_by(|a, b| a.block.cmp(&b.block).then_with(|| a.run_id.cmp(&b.run_id)));
                let kind = RowKind::Agent {
                    at: *updated,
                    meta: terminal.contains(&ix) && !still,
                    streaming: *streaming,
                    rewind: at.and_then(rewind_of),
                    fork: at.and_then(reply_fork),
                    runs: ran,
                };
                rows.push(Row { key: e.key.clone(), kind, text: text.clone(), turn: e.turn.clone() });
                if let Some(row) = last.and_then(|t| by_id.get(t)).and_then(|t| changes_row(t)) {
                    rows.push(row);
                }
            }
        }
        ix += 1;
    }
    if working && boundary.is_some_and(|b| header < b) {
        split = None;
    }
    if working && !waiting && (!activity || latest_failed) {
        // Nothing thinks while the model call is being made again: the row keeps its height and says nothing.
        rows.push(Row {
            key: "thinking".to_owned(),
            kind: RowKind::Thinking,
            text: if retry.is_some() { String::new() } else { "Thinking".to_owned() },
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
            // Any other stop is the person's own, and its word says it all whatever the agent wrote as its reason.
            let why = t.error.clone().filter(|_| t.state == State::Interrupted && t.unreached);
            rows.push(Row {
                key: "footer".to_owned(),
                kind: RowKind::Footer { word, why },
                text: text.to_owned(),
                turn: Some(t.id.clone()),
            });
        }
    }
    Some((rows, split))
}

/// The turn an entry belongs to, a person's message included.
fn owner(e: &Entry) -> Option<&str> {
    match &e.body {
        Body::Person { turn, .. } => turn.as_deref(),
        _ => e.turn.as_deref(),
    }
}

/// The files a turn changed, under its last reply (ChangedFilesCard in ChangedFilesTree.tsx).
fn changes_row(t: &Summary) -> Option<Row> {
    let changes = t.changes.as_ref()?;
    let others = changes.others.clone().unwrap_or_default();
    if changes.files.is_empty() && others.is_empty() {
        return None;
    }
    let n = changes.files.len();
    let count = format!("{n} changed file{}", if n == 1 { "" } else { "s" });
    let label = if n == 0 {
        "No files from this thread's edits".to_owned()
    } else if changes.others.is_none() && changes.shared {
        format!("{count} in this folder")
    } else {
        count
    };
    let range = (changes.from.clone(), changes.to.clone());
    let kind = RowKind::ChangedFiles { label: label.clone(), files: changes.files.clone(), others, range };
    Some(Row { key: format!("changes:{}", t.id), kind, text: label, turn: Some(t.id.clone()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::words::fmt_tokens;
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

    #[test]
    fn a_settled_turn_keeps_the_message_one_fold_and_the_last_reply() {
        let rows = rows(&turn(true), &HashSet::new());
        assert_eq!(shown(&rows), [("person", "fix it"), ("fold", "Worked for 12s"), ("agent", "Done here.")]);
        assert_eq!(rows[1].kind, RowKind::Fold { open: false });
        assert_eq!(rows[1].turn.as_deref(), Some("u1"));
        assert_eq!(rows[2].kind, agent(None, true, false));
    }

    #[test]
    fn an_opened_turn_draws_its_hidden_rows_in_place_under_the_fold() {
        let rows = rows(&turn(true), &opened(&["u1"]));
        assert_eq!(
            shown(&rows),
            [("person", "fix it"), ("fold", "Worked for 12s"), ("agent", "Looking."), ("group", "Ran 1 command"), ("agent", "Done here."),]
        );
        assert_eq!(rows[1].kind, RowKind::Fold { open: true });
        assert_eq!(rows[2].kind, agent(None, false, false));
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
                json!({"prompt": "look", "at": 5_000.0, "requestId": "q1", "attachments": [
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
                fork: None,
                full: false,
                steered: false,
                at: Some(5_000.0),
                files: vec![
                    FileTile { id: Some("q1".into()), ..tile("shot.png", 1200, true, 0, "image/png") },
                    FileTile { id: Some("q1".into()), ..tile("file", 30, false, 1, "text/plain") },
                ],
            }
        );
        assert_eq!(
            rows[1].kind,
            RowKind::Person {
                at: Some(6_000.0),
                files: vec![tile("image", 9, true, 0, "image/jpeg")],
                fork: None,
                full: false,
                steered: true
            },
            "a message sent while the agent worked is not marked so"
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
        assert_eq!(rows[1].kind, RowKind::Working { since: Some(1_000.0), waiting: false, retry: None, wait: None });
        assert_eq!(rows[2].kind, agent(None, false, false));
        let group = "group:tool:u1:a".to_owned();
        assert_eq!(
            rows[3].kind,
            RowKind::Live {
                group,
                glyph: Glyph::Terminal,
                verb: Some("Running".into()),
                mono: true,
                since: Some(4_000.0),
                open: false,
                active: true
            }
        );
        assert_eq!(rows[3].key, "live");
    }

    #[test]
    fn a_streaming_reply_is_marked_and_thinking_closes_the_running_turn() {
        let events =
            vec![base("session.start", 1, json!({"prompt": "go"})), base("session.delta", 2, json!({"kind": "text", "text": "Hel"}))];
        let rows = rows(&events, &HashSet::new());
        assert_eq!(shown(&rows), [("person", "go"), ("working", "Working"), ("agent", "Hel"), ("thinking", "Thinking")]);
        assert_eq!(rows[2].kind, agent(None, false, true));
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
        assert_eq!(rows[5].kind, RowKind::Footer { word: FooterWord::Interrupted, why: None });
        assert_eq!(rows[4].key, "stop:u1");
    }

    #[test]
    fn a_stop_that_never_reached_the_computer_says_why_under_its_word() {
        let said = "The stop did not reach box: its computer did not answer";
        let ended = |unreached: bool| {
            let mut result = json!({"status": "interrupted", "error": said});
            if unreached {
                result["unreached"] = json!(true);
            }
            let events = vec![
                base("session.start", 1, json!({"prompt": "fix it"})),
                base("session.done", 2, json!({"result": result})),
                base("session.end", 3, json!({"exitCode": 0, "sawResult": true})),
            ];
            rows(&events, &HashSet::new()).last().map(|r| r.kind.clone())
        };
        assert_eq!(ended(true), Some(RowKind::Footer { word: FooterWord::Interrupted, why: Some(said.to_owned()) }));
        assert_eq!(ended(false), Some(RowKind::Footer { word: FooterWord::Interrupted, why: None }), "a stop the agent heard says why");
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
        assert_eq!(rows.last().unwrap().kind, RowKind::Footer { word: FooterWord::Failed, why: None });
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
        assert_eq!(rows[2].kind, RowKind::Work { tone: Tone::Notice, glyph: Glyph::Info, body: None, open: false });
        assert_eq!(rows[3].kind, RowKind::Work { tone: Tone::Compaction, glyph: Glyph::Minimize, body: None, open: false });
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
        assert_eq!(fmt_duration(400.0), "400ms");
        assert_eq!(fmt_duration(4_250.0), "4.3s");
        assert_eq!(fmt_duration(12_400.0), "12s");
        assert_eq!(fmt_duration(125_000.0), "2m 5s");
        assert_eq!(fmt_tokens(42_100.0), "42.1k");
        assert_eq!(fmt_tokens(4_270.0), "4.27k");
        assert_eq!(fmt_tokens(180_000.0), "180k");
    }

    fn find<'a>(rows: &'a [Row], kind: &str) -> &'a Row {
        rows.iter().find(|r| name(&r.kind) == kind).unwrap_or_else(|| panic!("no {kind} row in {:?}", shown(rows)))
    }

    fn prompt(p: &[(&str, &str, &str)], outcome: Option<&str>, picked: Option<&str>) -> Prompt {
        Prompt {
            ask: "k".into(),
            session: "s".into(),
            tool: "Bash".into(),
            input: "{}".into(),
            detail: None,
            options: p.iter().map(|(a, b, c)| ((*a).into(), (*b).into(), (*c).into(), None)).collect(),
            outcome: outcome.map(str::to_owned),
            picked: picked.map(str::to_owned),
        }
    }

    #[test]
    fn a_permission_reads_its_lead_and_waits_until_answered() {
        let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}, {"id": "n", "label": "Deny", "effect": "deny"}]);
        let mut events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"cargo test --all\",\"timeout\":5}"}),
            ),
            base(
                "session.permission",
                3,
                json!({"askId": "k", "toolName": "Bash", "toolUseId": "a", "input": "{\"command\":\"cargo test --all\",\"timeout\":5}", "options": options}),
            ),
        ];
        let waiting = rows(&events, &HashSet::new());
        assert_eq!(find(&waiting, "working").kind, RowKind::Working { since: None, waiting: true, retry: None, wait: None });
        assert_eq!(find(&waiting, "working").text, "Waiting for you");
        assert!(!waiting.iter().any(|r| r.kind == RowKind::Thinking));
        let row = find(&waiting, "permission");
        assert_eq!((row.key.as_str(), row.text.as_str()), ("permission:k", "Run a command: cargo test --all"));
        assert_eq!(
            row.kind,
            RowKind::Permission {
                ask: "k".into(),
                session: "s".into(),
                asker: None,
                glyph: Glyph::Terminal,
                lead: "Run a command:".into(),
                code: Some("cargo test --all".into()),
                fields: vec![("timeout".into(), "5".into())],
                file: None,
                options: vec![
                    PromptOption { id: "y".into(), label: "Allow".into(), description: None, effect: "allow".into(), mode: None },
                    PromptOption { id: "n".into(), label: "Deny".into(), description: None, effect: "deny".into(), mode: None },
                ],
                outcome: None,
                open: true,
            }
        );
        events.push(base("session.permission.closed", 4, json!({"askId": "k", "outcome": "allowed", "optionId": "y"})));
        let answered = rows(&events, &HashSet::new());
        assert!(matches!(&find(&answered, "permission").kind, RowKind::Permission { outcome: Some(o), open: false, .. } if o == "Allowed"));
        assert_eq!(find(&answered, "working").kind, RowKind::Working { since: None, waiting: false, retry: None, wait: None });
    }

    #[test]
    fn outcome_words_read_as_the_web_app_writes_them() {
        let consent = [("y", "Allow", "allow"), ("m", "Allow, then auto", "mode"), ("n", "Deny", "deny")];
        let line = |outcome, picked| outcome_line(&prompt(&consent, Some(outcome), picked));
        assert_eq!(line("allowed", Some("y")).as_deref(), Some("Allowed"));
        assert_eq!(line("allowed", Some("m")).as_deref(), Some("Allowed: Allow, then auto"));
        assert_eq!(line("denied", Some("n")).as_deref(), Some("Denied"));
        assert_eq!(line("unanswered", None).as_deref(), Some("Nobody answered; denied"));
        assert_eq!(line("cancelled", None).as_deref(), Some("Cancelled with the turn"));
        assert_eq!(outcome_line(&prompt(&consent, None, None)), None);
        let answers = [("q0:o0", "Red", "answer"), ("q1:o0", "Big", "answer")];
        let answered = |picked| outcome_line(&prompt(&answers, Some("allowed"), Some(picked)));
        assert_eq!(answered("q0:o0|q1:o0").as_deref(), Some("You answered: Red, Big"));
        assert_eq!(answered("q0:other:Light%20blue").as_deref(), Some("You answered: Light blue"));
    }

    #[test]
    fn a_prompt_says_its_kind_of_call_in_words() {
        let write = prompt_words("Write", "{\"file_path\":\"src/app.ts\",\"content\":\"hello\"}", None);
        assert_eq!((write.says.as_str(), write.body.as_deref(), write.rest.len()), ("Write app.ts in src (5 B)", Some("hello"), 0));
        let edit = prompt_words("Edit", "{\"file_path\":\"/w/src/app.ts\",\"edits\":[{},{}]}", None);
        assert_eq!(edit.says, "Edit app.ts in src (2 places)");
        assert_eq!(prompt_words("WebFetch", "{\"url\":\"https://a.b\"}", None).code.as_deref(), Some("https://a.b"));
        assert_eq!(prompt_words("Frob", "{\"x\":\"a  b\"}", Some("why")).says, "Permission for Frob: why");
        assert_eq!(prompt_words("Frob", "{\"x\":\"a  b\"}", None).rest, vec![("x".to_owned(), "a b".to_owned())]);
        assert_eq!(prompt_words("mcp__wsp__run", "{}", None).says, "Use wsp's run");
        assert_eq!(prompt_glyph("Edit", "{}"), Glyph::SquarePen);
        assert_eq!(prompt_glyph("WebSearch", "{}"), Glyph::Globe);
        assert_eq!(prompt_glyph("Frob", "{}"), Glyph::Wrench);
    }

    #[test]
    fn a_question_lists_its_choices_and_the_answer() {
        let input = json!({"questions": [{"question": "Which colour?", "header": "Colour", "multiSelect": true, "options": [
            {"label": "Red", "description": "warm"}, {"label": "Blue"}
        ]}]});
        let options = json!([{"id": "q0:o0", "label": "Red", "effect": "answer"}, {"id": "q0:o1", "label": "Blue", "effect": "answer"}]);
        let mut events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base(
                "session.permission",
                2,
                json!({"askId": "w", "toolName": "AskUserQuestion", "input": input.to_string(), "options": options}),
            ),
        ];
        let open = rows(&events, &HashSet::new());
        let row = find(&open, "question");
        assert_eq!(row.text, "Which colour?");
        let asked = Asked {
            header: Some("Colour".into()),
            question: "Which colour?".into(),
            options: vec![
                PromptOption {
                    id: "q0:o0".into(),
                    label: "Red".into(),
                    description: Some("warm".into()),
                    effect: "answer".into(),
                    mode: None,
                },
                PromptOption { id: "q0:o1".into(), label: "Blue".into(), description: None, effect: "answer".into(), mode: None },
            ],
            multiple: true,
        };
        assert_eq!(
            row.kind,
            RowKind::Question { ask: "w".into(), session: "s".into(), asker: None, questions: vec![asked], outcome: None }
        );
        assert!(matches!(find(&open, "working").kind, RowKind::Working { waiting: true, .. }));
        events.push(base("session.permission.closed", 3, json!({"askId": "w", "outcome": "allowed", "optionId": "q0:o1"})));
        let closed = rows(&events, &HashSet::new());
        assert!(matches!(&find(&closed, "question").kind, RowKind::Question { outcome: Some(o), .. } if o == "You answered: Blue"));
    }

    #[test]
    fn a_plan_stays_through_the_fold_and_opens_when_long() {
        let plan = "# Fix the build\n\n## Summary\n\nFirst.\n\n- one\n- two";
        let events = vec![
            base("session.start", 1, json!({"prompt": "plan it"})),
            base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "a", "text": "{\"file_path\":\"a\"}"})),
            base("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "x"})),
            base("session.plan", 4, json!({"text": plan})),
            base("session.delta", 5, json!({"kind": "text", "text": "Here it is."})),
            base("session.done", 6, json!({"result": {"status": "completed", "durationMs": 2_000}})),
            base("session.end", 7, json!({"exitCode": 0, "sawResult": true})),
        ];
        let mut t = Transcript::new();
        t.load(events);
        assert_eq!(shown(&t.rows), [("person", "plan it"), ("fold", "Worked for 2.0s"), ("plan", plan), ("agent", "Here it is.")]);
        assert_eq!(t.rows[2].kind, RowKind::Plan { title: "Fix the build".into(), open: false });
        t.toggle("plan:plan:u1");
        assert_eq!(t.rows[2].kind, RowKind::Plan { title: "Fix the build".into(), open: true });
        assert_eq!(plan_shown(plan, false), "First.\n\n- one\n- two");
        assert!(!plan_folds(plan));
        let long: String = (0..25).map(|n| format!("- step {n}\n")).collect();
        assert!(plan_folds(&long));
        let preview = plan_shown(&long, false);
        assert!(preview.starts_with("- step 0\n") && preview.ends_with("- step 9\n\n..."), "{preview}");
        assert_eq!(plan_shown(&long, true), long.trim_end());
        assert_eq!(plan_title("no heading"), None);
    }

    fn subagent(result: Option<serde_json::Value>) -> Vec<SessionEvent> {
        let task = "{\"description\":\"Survey the code\\nmore\",\"prompt\":\"look\"}";
        let mut events = vec![
            base("session.start", 1, json!({"prompt": "go", "at": 1_000.0})),
            base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": task, "at": 2_000.0})),
            base("session.delta", 3, json!({"kind": "text", "text": "Reading.", "parentToolUseId": "t"})),
            base(
                "session.delta",
                4,
                json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "c", "parentToolUseId": "t", "text": "{\"file_path\":\"src/b.rs\"}"}),
            ),
            base("session.delta", 5, json!({"kind": "tool_result", "toolUseId": "c", "parentToolUseId": "t", "text": "fn b() {}\nmore"})),
        ];
        events.extend(result.map(|r| base("session.delta", 6, r)));
        events
    }

    #[test]
    fn a_subagents_command_skips_a_first_line_the_protocol_reads_as_blank() {
        let mut events = subagent(None);
        let call = json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "d", "parentToolUseId": "t", "text": "{\"command\":\"\\ufeff\\nls -la\"}"});
        events.push(base("session.delta", 6, call));
        let rows = rows(&events, &opened(&["subagent:t"]));
        let RowKind::Subagent { lines, .. } = &find(&rows, "subagent").kind else { panic!("no subagent row: {:?}", shown(&rows)) };
        assert_eq!(lines.last().map(|l| l.text.as_str()), Some("$ ls -la"));
    }

    #[test]
    fn a_subagent_folds_its_lines_under_its_title() {
        let done = json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it.", "at": 67_000.0});
        let rows = rows(&subagent(Some(done)), &opened(&["subagent:t"]));
        let row = find(&rows, "subagent");
        let line = |text: &str, mono, detail: Option<&str>| SubLine { text: text.into(), mono, detail: detail.map(str::to_owned) };
        assert_eq!(
            row.kind,
            RowKind::Subagent {
                id: "subagent:t".into(),
                title: "Survey the code".into(),
                state: Some("Done 1m 5s".into()),
                open: true,
                lines: vec![line("Reading.", false, None), line("read src/b.rs", true, Some("fn b() {}")), line("Found it.", false, None)],
            }
        );
        assert_eq!(row.key, "subagent:t");
        assert!(!rows.iter().any(|r| name(&r.kind) == "group"), "{:?}", shown(&rows));
        let failed = json!({"kind": "tool_result", "toolUseId": "t", "text": "boom", "isError": true, "at": 14_000.0});
        let failed = super::tests::rows(&subagent(Some(failed)), &HashSet::new());
        assert!(matches!(&find(&failed, "subagent").kind, RowKind::Subagent { state: Some(s), open: false, .. } if s == "Failed 12s"));
        let mut cut = subagent(None);
        cut.push(base("session.done", 7, json!({"result": {"status": "interrupted"}, "at": 12_000.0})));
        let stopped = super::tests::rows(&cut, &HashSet::new());
        assert!(matches!(&find(&stopped, "subagent").kind, RowKind::Subagent { state: Some(s), .. } if s == "Stopped 10s"));
        let running = super::tests::rows(&subagent(None), &HashSet::new());
        assert!(matches!(&find(&running, "subagent").kind, RowKind::Subagent { state: None, .. }));
    }

    #[test]
    fn a_question_opens_its_subagents_fold_and_the_persons_shut_stands_until_the_next() {
        let fold = |t: &Transcript| matches!(find(&t.rows, "subagent").kind, RowKind::Subagent { open: true, .. });
        let ask = |id: &str, pos| {
            let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
            base(
                "session.permission",
                pos,
                json!({"askId": id, "toolName": "Bash", "parentToolUseId": "t", "input": "{}", "options": options}),
            )
        };
        let mut t = Transcript::new();
        t.load(subagent(None));
        assert!(!fold(&t));
        t.push(ask("s1", 6));
        assert!(fold(&t), "a question opens the fold");
        t.toggle("subagent:t");
        assert!(!fold(&t), "the person's shut stands while the question waits");
        t.push(ask("s2", 7));
        assert!(fold(&t), "the next question opens it again");
        t.toggle("subagent:t");
        t.toggle("subagent:t");
        assert!(fold(&t), "a second press opens it again");
    }

    #[test]
    fn a_subagent_prompt_opens_its_fold_under_its_name() {
        let mut events = subagent(None);
        let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
        let asked = json!({"askId": "s1", "toolName": "Bash", "parentToolUseId": "t", "toolUseId": "x", "input": "{\"command\":\"rm x\"}", "options": options});
        events.push(base("session.permission", 6, asked));
        let open = rows(&events, &HashSet::new());
        let at = open.iter().position(|r| name(&r.kind) == "subagent").unwrap();
        assert!(matches!(open[at].kind, RowKind::Subagent { open: true, .. }));
        assert!(matches!(&open[at + 1].kind, RowKind::Permission { asker: Some(a), open: true, .. } if a == "Survey the code asks"));
        assert_eq!(open[at + 1].key, "permission:s1");
        assert!(!matches!(find(&open, "working").kind, RowKind::Working { waiting: true, .. }));
        events.push(base("session.permission.closed", 7, json!({"askId": "s1", "outcome": "denied"})));
        let shut = rows(&events, &HashSet::new());
        assert!(matches!(find(&shut, "subagent").kind, RowKind::Subagent { open: false, .. }));
        assert!(!shut.iter().any(|r| name(&r.kind) == "permission"));
    }

    #[test]
    fn calls_that_started_children_draw_as_one_spawn_row() {
        let events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": "{\"description\":\"Survey\"}"}),
            ),
            base("session.subagent", 3, json!({"task": "k1", "state": "running", "parentToolUseId": "t", "title": "Survey it"})),
            base("session.delta", 4, json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it."})),
            base("session.subagent", 5, json!({"task": "k1", "state": "failed", "parentToolUseId": "t", "summary": "\nout of room\nmore"})),
            base(
                "session.delta",
                6,
                json!({"kind": "tool_use", "toolName": "mcp__wsp__run", "toolUseId": "r", "text": "{\"title\":\"Fix docs\"}"}),
            ),
            base("session.delta", 7, json!({"kind": "tool_result", "toolUseId": "r", "text": "{\"threadId\":\"th1\"}"})),
            base("session.delta", 8, json!({"kind": "text", "text": "Started."})),
        ];
        assert!(!shown(&rows(&events, &HashSet::new())).iter().any(|(kind, _)| *kind == "spawn"), "a child the listing does not hold");
        let listing = Listing::holding(&events);
        let mut t = Transcript::new();
        t.listing(listing.clone());
        t.load(events.clone());
        let rows = t.rows;
        assert_eq!(
            shown(&rows),
            [
                ("person", "go"),
                ("working", "Working"),
                ("spawn", ""),
                ("agent", "Started."),
                ("thinking", "Thinking"),
                ("threads", "Threads")
            ]
        );
        let RowKind::Spawn { children } = &rows[2].kind else { unreachable!() };
        let survey = Child {
            thread: None,
            title: "Survey it".into(),
            note: Some("out of room".into()),
            status: Some("failed".into()),
            subagent: true,
            settled: false,
            call: Some("t".into()),
            listed: None,
            workspace: None,
        };
        let docs = Child {
            thread: Some("th1".into()),
            title: "Fix docs".into(),
            note: None,
            status: None,
            subagent: false,
            settled: false,
            call: None,
            listed: listing.child("th1").cloned(),
            workspace: None,
        };
        assert_eq!(children, &vec![survey, docs]);
        assert_eq!(rows[2].key, "spawn:u1:call:t");
        let mut named = subagent(None);
        named.push(base("session.subagent", 6, json!({"task": "k1", "state": "running", "parentToolUseId": "t", "title": "Survey"})));
        let mut held = Transcript::new();
        held.listing(Listing::holding(&named));
        held.load(named);
        let named = held.rows;
        assert!(named.iter().any(|r| r.key == "spawn:subagent:t"), "{:?}", shown(&named));
        assert!(!named.iter().any(|r| name(&r.kind) == "subagent"));
        assert_eq!(named_thread("thread th2 on wsp in /w"), Some("th2".into()));
        assert_eq!(named_thread("[{\"type\":\"text\",\"text\":\"{\\\"turn\\\":{\\\"threadId\\\":\\\"th3\\\"}}\"}]"), Some("th3".into()));
        assert!(starts_thread("cd /w && ~/bin/wsp run --project x 'go'"));
        assert!(starts_thread("bash -lc 'wsp fork t1'"));
        assert!(!starts_thread("echo 'wsp run'"));
    }

    fn changes(extra: serde_json::Value) -> Vec<Row> {
        let mut v = json!({"from": "a", "to": "b", "moved": [], "files": [
            {"path": "src/a.rs", "kind": "modified", "additions": 3, "deletions": 1},
            {"path": "b.rs", "kind": "added", "additions": 9, "deletions": 0}
        ]});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let mut events = turn(true);
        events.push(base("session.changes", 11, v));
        rows(&events, &HashSet::new())
    }

    #[test]
    fn a_turns_changes_hang_under_its_last_reply() {
        let rows = changes(json!({}));
        assert_eq!(
            shown(&rows),
            [("person", "fix it"), ("fold", "Worked for 12s"), ("agent", "Done here."), ("changes", "2 changed files")]
        );
        let files =
            vec![ChangedFile { path: "src/a.rs".into(), added: 3, removed: 1 }, ChangedFile { path: "b.rs".into(), added: 9, removed: 0 }];
        let range = ("a".into(), "b".into());
        assert_eq!(rows[3].kind, RowKind::ChangedFiles { label: "2 changed files".into(), files, others: Vec::new(), range });
        assert_eq!((rows[3].key.as_str(), rows[3].turn.as_deref()), ("changes:u1", Some("u1")));
        assert_eq!(changes(json!({"shared": true}))[3].text, "2 changed files in this folder");
        let others = json!({"files": [], "others": [{"path": "c.rs", "kind": "modified", "additions": 1, "deletions": 1}]});
        let lone = changes(others);
        assert_eq!(lone[3].text, "No files from this thread's edits");
        assert!(matches!(&lone[3].kind, RowKind::ChangedFiles { others, .. } if others.len() == 1));
        let one = json!({"files": [{"path": "a", "kind": "modified", "additions": 1, "deletions": 0}]});
        assert_eq!(changes(one)[3].text, "1 changed file");
        assert_eq!(changes(json!({"files": []})).len(), 3);
    }

    fn forked() -> Vec<SessionEvent> {
        let mut events = turn(true);
        for e in &mut events {
            let mut v = serde_json::to_value(&*e).unwrap();
            v["copiedFrom"] = json!("th-source");
            *e = event(v);
        }
        events.push(base("session.start", 20, json!({"prompt": "and now", "turnId": "u2"})));
        events
    }

    #[test]
    fn a_fork_says_where_it_came_from_before_its_own_first_message() {
        let mut t = Transcript::new();
        t.load(forked()[..10].to_vec());
        assert!(!shown(&t.rows).iter().any(|(kind, _)| *kind == "fork"), "the rule stands before the listing names its thread");
        let source = ForkSource { title: "Fix the build".into(), at: None };
        t.listing(Listing { source: Some(source), ..Listing::default() });
        assert_eq!(shown(&t.rows).last(), Some(&("fork", "Forked from Fix the build")));
        t.push(forked()[10].clone());
        let at = t.rows.iter().position(|r| name(&r.kind) == "fork").unwrap();
        assert_eq!(shown(&t.rows[at..at + 2]), [("fork", "Forked from Fix the build"), ("person", "and now")]);
        assert_eq!(t.rows[at].kind, RowKind::Fork { source: "Fix the build".into(), open: None });
        t.forked_from(Some("Fix the build".into()));
        assert_eq!(t.rows[at].text, "Forked from Fix the build");
        assert!(!rows(&turn(true), &HashSet::new()).iter().any(|r| name(&r.kind) == "fork"));
    }

    fn two_turns(running: bool) -> Vec<SessionEvent> {
        let mut events = turn(true);
        events.push(base("session.checkpoint", 11, json!({"ref": "c1", "anchor": "x"})));
        events.push(base("session.start", 12, json!({"prompt": "more", "turnId": "u2"})));
        events.push(base("session.delta", 13, json!({"kind": "text", "text": "Ok.", "turnId": "u2"})));
        if !running {
            events.push(base("session.done", 14, json!({"result": {"status": "completed"}, "turnId": "u2"})));
            events.push(base("session.end", 15, json!({"exitCode": 0, "sawResult": true, "turnId": "u2"})));
        }
        events
    }

    #[test]
    fn rewind_and_fork_stand_on_earlier_replies() {
        let can = Can { forks: true, ..Can::default() };
        let load = |events: Vec<SessionEvent>, can| {
            let mut t = Transcript { can, ..Transcript::default() };
            t.load(events);
            t.rows
        };
        let agent_of = |rows: &[Row], text: &str| rows.iter().find(|r| r.text == text).map(|r| r.kind.clone()).unwrap();
        let settled = load(two_turns(false), can);
        assert!(
            matches!(agent_of(&settled, "Done here."), RowKind::Agent { rewind: Some(r), fork: Some(f), .. } if r == "u1" && f == "u1")
        );
        assert!(matches!(agent_of(&settled, "Ok."), RowKind::Agent { rewind: None, fork: None, .. }));
        assert!(matches!(agent_of(&settled, "fix it"), RowKind::Person { fork: Some(f), .. } if f.is_empty()));
        assert!(matches!(agent_of(&settled, "more"), RowKind::Person { fork: Some(f), .. } if f == "u1"));
        let working = load(two_turns(true), can);
        assert!(matches!(agent_of(&working, "Done here."), RowKind::Agent { rewind: Some(_), fork: Some(_), .. }));
        assert!(matches!(agent_of(&working, "more"), RowKind::Person { fork: None, .. }));
        let cannot = load(two_turns(false), Can::default());
        assert!(matches!(agent_of(&cannot, "Done here."), RowKind::Agent { rewind: Some(_), fork: None, .. }));
        let mut live = Transcript { can, ..Transcript::default() };
        for e in two_turns(false) {
            live.push(e);
        }
        assert_eq!(live.rows, settled);
    }

    #[test]
    fn a_reply_carries_its_block_runs() {
        let text = "Try:\n\n```sh\nls\n```\n\nthen\n\n~~~bash\npwd\n~~~\n";
        let events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base("session.delta", 2, json!({"kind": "text", "text": text})),
            base("session.done", 3, json!({"result": {"status": "completed"}})),
            base("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
            base("session.run", 5, json!({"runId": "r1", "block": "u1:m@2@6", "command": "ls", "state": "running"})),
            base(
                "session.run",
                6,
                json!({"runId": "r2", "block": "u1:m@2@28", "command": "pwd", "state": "exited", "exitCode": 1, "output": "/w"}),
            ),
            base("session.run", 7, json!({"runId": "r3", "block": "u1:m@2@6", "command": "ls", "state": "exited", "signal": 9})),
        ];
        let rows = rows(&events, &HashSet::new());
        let RowKind::Agent { runs, .. } = &find(&rows, "agent").kind else { unreachable!() };
        let run = |id: &str, block, command: &str, state: &str, output: Option<&str>, exit_code, signal: Option<&str>| Run {
            run_id: id.into(),
            block,
            command: command.into(),
            state: state.into(),
            output: output.map(str::to_owned),
            exit_code,
            signal: signal.map(str::to_owned),
            pty_id: None,
        };
        assert_eq!(
            runs,
            &vec![run("r3", 0, "ls", "exited", None, None, Some("9")), run("r2", 1, "pwd", "exited", Some("/w"), Some(1), None)]
        );
        assert_eq!((runs[0].words(), runs[1].words()), ("Stopped by signal 9".into(), "Exited 1".into()));
        assert_eq!(block_at("é\n```\nx\n```", 2), Some(0));
        assert_eq!(block_at("text", 0), None);
    }

    /// A thread of `turns` turns using every event the fold reads, some landing on a turn after the next one opened.
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
            pos
        };
        let mut replies = Vec::new();
        let files = |n: usize| json!([{"path": format!("src/f{n}.rs"), "kind": "modified", "additions": 3, "deletions": 1}]);
        for n in 0..turns {
            let mut start = json!({"prompt": format!("ask {n}")});
            if n < 2 {
                start["copiedFrom"] = json!("source-thread");
            }
            if n % 4 == 0 {
                start["requestId"] = json!(format!("q{n}"));
                start["attachments"] = json!([{"mediaType": "image/png", "bytes": 10, "name": "a.png"}]);
            }
            push("session.start", n, start);
            if n % 4 == 3 {
                push("session.changes", n - 1, json!({"from": "a", "to": "b", "files": files(n), "moved": []}));
            }
            if n % 5 == 1 {
                push("session.checkpoint", n - 1, json!({"ref": "c", "anchor": "x"}));
            }
            if n % 6 == 2 && n >= 3 {
                let block: &String = &replies[n - 2];
                push("session.run", n - 2, json!({"runId": format!("run{n}"), "block": block, "command": "ls", "state": "running"}));
            }
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
                let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}, {"id": "n", "label": "Deny", "effect": "deny"}]);
                let input = "{\"command\":\"cargo test\"}";
                push("session.permission", n, json!({"askId": format!("k{n}"), "toolName": "Bash", "input": input, "options": options}));
                push("session.permission.closed", n, json!({"askId": format!("k{n}"), "outcome": "allowed", "optionId": "y"}));
            }
            push("session.delta", n, json!({"kind": "tool_result", "toolUseId": format!("b{n}"), "text": "ok\n", "exitCode": 0}));
            push(
                "session.delta",
                n,
                json!({"kind": "tool_use", "toolName": "Read", "toolUseId": format!("r{n}"), "text": "{\"file_path\":\"src/a.rs\"}"}),
            );
            push("session.delta", n, json!({"kind": "tool_result", "toolUseId": format!("r{n}"), "text": "fn a() {}"}));
            if n % 3 == 0 {
                let task = format!("t{n}");
                let input = "{\"description\":\"Survey the code\",\"prompt\":\"look around\"}";
                push("session.delta", n, json!({"kind": "tool_use", "toolName": "Task", "toolUseId": task, "text": input}));
                if n % 6 == 0 {
                    push(
                        "session.subagent",
                        n,
                        json!({"task": format!("k-{task}"), "state": "running", "parentToolUseId": task, "title": "Survey"}),
                    );
                }
                push("session.delta", n, json!({"kind": "text", "text": "Reading.", "parentToolUseId": task}));
                let read = json!({"kind": "tool_use", "toolName": "Read", "toolUseId": format!("c{n}"), "parentToolUseId": task, "text": "{\"file_path\":\"src/b.rs\"}"});
                push("session.delta", n, read);
                push(
                    "session.delta",
                    n,
                    json!({"kind": "tool_result", "toolUseId": format!("c{n}"), "parentToolUseId": task, "text": "fn b() {}"}),
                );
                if n % 9 == 0 {
                    let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
                    let asked = json!({"askId": format!("s{n}"), "toolName": "Bash", "parentToolUseId": task, "input": "{\"command\":\"rm x\"}", "options": options});
                    push("session.permission", n, asked);
                    push("session.permission.closed", n, json!({"askId": format!("s{n}"), "outcome": "denied"}));
                }
                if n % 12 != 3 {
                    push("session.delta", n, json!({"kind": "tool_result", "toolUseId": task, "text": "Found it."}));
                }
                if n % 6 == 0 {
                    push(
                        "session.subagent",
                        n,
                        json!({"task": format!("k-{task}"), "state": "done", "parentToolUseId": task, "summary": "Found it."}),
                    );
                }
            }
            if n % 5 == 3 {
                let input = json!({"questions": [{"question": "Which colour?", "header": "Colour", "options": [{"label": "Red"}, {"label": "Blue"}]}]});
                let options =
                    json!([{"id": "q0:o0", "label": "Red", "effect": "answer"}, {"id": "q0:o1", "label": "Blue", "effect": "answer"}]);
                push(
                    "session.permission",
                    n,
                    json!({"askId": format!("w{n}"), "toolName": "AskUserQuestion", "input": input.to_string(), "options": options}),
                );
                push("session.permission.closed", n, json!({"askId": format!("w{n}"), "outcome": "allowed", "optionId": "q0:o1"}));
            }
            if n % 4 == 1 {
                push("session.notice", n, json!({"text": "the machine slept"}));
            }
            if n % 4 == 2 {
                push("session.plan", n, json!({"text": "# Fix plan\n\n- step one"}));
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
            for (i, word) in ["All ", "done:\n```sh\nls\n```\n", "now."].into_iter().enumerate() {
                let at = push("session.delta", n, json!({"kind": "text", "text": word, "messageId": format!("z{n}")}));
                if i == 0 {
                    replies.push(format!("u{n}:m@{at}@10"));
                }
            }
            if n % 6 == 4 {
                let input = "{\"file_path\":\"src/app.ts\",\"old_string\":\"a\",\"new_string\":\"b\"}";
                let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
                push("session.permission", n, json!({"askId": format!("o{n}"), "toolName": "Edit", "input": input, "options": options}));
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
            if n % 2 == 0 {
                let others = if n % 4 == 0 { json!({"shared": true}) } else { json!({"others": files(n + 100)}) };
                let mut changes = json!({"from": "a", "to": "b", "files": files(n), "moved": []});
                changes.as_object_mut().unwrap().extend(others.as_object().unwrap().clone());
                push("session.changes", n, changes);
            }
            if n % 3 != 2 {
                push("session.checkpoint", n, json!({"ref": "c", "anchor": "x"}));
            }
            if n % 4 == 1 {
                let block = replies[n].clone();
                push(
                    "session.run",
                    n,
                    json!({"runId": format!("run{n}"), "block": block, "command": "ls", "state": "exited", "exitCode": 0, "output": "a\n"}),
                );
            }
        }
        events
    }

    #[test]
    fn a_live_event_folds_to_what_a_load_of_the_same_events_draws() {
        let events = thread(12);
        assert!(events.len() >= 200, "{} events", events.len());
        let ids =
            (0..12).map(|n| format!("u{n}")).chain(["group:tool:u7:b7", "u8:call:b8", "plan:plan:u6", "subagent:t3"].map(str::to_owned));
        let can = Can { forks: true, forks_by_count: false, rewinds_conversation: true, rewinds_by_count: false };
        for (opened, can) in [(HashSet::new(), Can::default()), (ids.collect::<HashSet<String>>(), can)] {
            let listing = Listing::holding(&events);
            let mut live = Transcript { opened: opened.clone(), can, ..Transcript::default() };
            live.listing(listing.clone());
            let mut kinds = HashSet::new();
            for (n, e) in events.iter().enumerate() {
                let before = live.rows.clone();
                let changed = live.push(e.clone());
                let mut load = Transcript { opened: opened.clone(), can, ..Transcript::default() };
                load.listing(listing.clone());
                load.load(events[..=n].to_vec());
                assert_eq!(live.rows, load.rows, "after event {n}");
                assert_eq!(changed, splice(&before, &load.rows), "splice after event {n}");
                kinds.extend(live.rows.iter().map(|r| name(&r.kind)));
                kinds.extend(live.rows.iter().filter(|r| matches!(r.kind, RowKind::Working { waiting: true, .. })).map(|_| "waiting"));
            }
            assert!(live.rows.iter().any(|r| r.kind == RowKind::Stopped));
            assert!(live.rows.iter().any(|r| r.kind == RowKind::Error && r.text == "Session restarted before it finished"));
            for kind in ["permission", "question", "plan", "subagent", "spawn", "changes", "fork", "waiting"] {
                assert!(kinds.contains(kind), "no {kind} row was drawn");
            }
            if can.forks {
                let agents = live.rows.iter().filter_map(|r| match &r.kind {
                    RowKind::Agent { rewind, fork, runs, .. } => Some((rewind.is_some(), fork.is_some(), !runs.is_empty())),
                    _ => None,
                });
                let (rewinds, forks, runs) = agents.fold((false, false, false), |a, b| (a.0 | b.0, a.1 | b.1, a.2 | b.2));
                assert!(rewinds && forks && runs, "rewind {rewinds}, fork {forks}, runs {runs}");
                assert!(live.rows.iter().any(|r| matches!(&r.kind, RowKind::Person { fork: Some(_), .. })));
            }
        }
    }

    #[test]
    fn a_live_event_folds_only_the_latest_turn() {
        let mut events = thread(900);
        let last = events.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
        events.truncate(last + 6);
        let delta = base("session.delta", 999_999, json!({"kind": "text", "text": " more", "messageId": "a899", "turnId": "u899"}));
        cheap_push(events, delta);
    }

    #[test]
    fn unfolded_rows_are_the_rows_with_every_fold_and_group_open() {
        let mut t = Transcript::new();
        t.load(thread(8));
        let shut = t.rows.clone();
        let unfolded = t.unfolded();
        assert_eq!(t.rows, shut);
        assert!(t.opened.is_empty());
        let ids: HashSet<String> = unfolded
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::Fold { .. } => r.turn.clone(),
                RowKind::Group { group, .. } | RowKind::Live { group, .. } => Some(group.clone()),
                RowKind::Plan { .. } => Some(format!("plan:{}", r.key)),
                RowKind::Subagent { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert!(unfolded.len() > shut.len());
        assert_eq!(unfolded, rows(&thread(8), &ids));
    }

    /// Folds `events`, pushes `delta`, and holds the push under 1% of the load and its rows to a whole fold's.
    fn cheap_push(mut events: Vec<SessionEvent>, delta: SessionEvent) {
        let mut t = Transcript::new();
        t.load(events.clone());
        let load = std::mem::take(&mut t.cost);
        t.push(delta.clone());
        assert!(t.cost * 100 < load, "a live event cost {} against a load's {load}", t.cost);
        events.push(delta);
        assert_eq!(t.rows, rows(&events, &HashSet::new()));
    }

    /// Cuts `events` at every event, reads the newer part, lays the older part under it as a page lands, and holds the
    /// rows, the unfolded rows, the turns and a live event after it to a whole read of the thread; most cuts graft.
    fn lands_as_a_whole_read(events: &[SessionEvent], can: Can, open: &[&str], live: SessionEvent) {
        let mut grafted = 0;
        for k in 1..events.len() {
            let whole = |events: &[SessionEvent]| {
                let mut t = Transcript { opened: opened(open), ..Transcript::default() };
                _ = t.can(can);
                t.load(events.to_vec());
                t
            };
            let mut all = whole(events);
            let mut t = whole(&events[k..]);
            t.prepend(events[..k].to_vec());
            grafted += t.grafts;
            assert_eq!(t.rows, all.rows, "the rows of a page cut at event {k}");
            assert_eq!(t.unfolded(), all.unfolded(), "the unfolded rows of a page cut at event {k}");
            assert_eq!(t.turn_contexts(), all.turn_contexts(), "the turns of a page cut at event {k}");
            assert_eq!(t.prepend(Vec::new()), None);
            let (a, b) = (t.push(live.clone()), all.push(live.clone()));
            assert_eq!((t.rows.clone(), a), (all.rows.clone(), b), "a live event after a page cut at event {k}");
        }
        assert!(grafted * 2 > events.len(), "only {grafted} of {} cuts kept the held fold", events.len() - 1);
    }

    #[test]
    fn a_page_landing_draws_the_thread_as_a_whole_read_does() {
        let events = thread(30);
        let next = base("session.start", 999_000, json!({"prompt": "again", "turnId": "u30", "at": 99_000_000.0}));
        lands_as_a_whole_read(&events, Can::default(), &[], next.clone());
        let all = Can { forks: true, forks_by_count: true, rewinds_conversation: true, rewinds_by_count: true };
        lands_as_a_whole_read(&events, all, &["u3", "u12", EARLIER, "group:tool:u20:b20", "subagent:t9"], next);
        let mut running = thread(30);
        let last = running.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
        running.truncate(last + 9);
        let delta = base("session.delta", 999_001, json!({"kind": "text", "text": " more", "messageId": "a29", "turnId": "u29"}));
        lands_as_a_whole_read(&running, Can::default(), &[], delta);
    }

    #[test]
    fn the_docked_prompt_is_read_off_the_latest_turn_alone() {
        let mut events = thread(120);
        let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
        let ask = json!({"askId": "late", "toolName": "Bash", "input": "{\"command\":\"ls\"}", "options": options, "turnId": "u120"});
        events.push(base("session.start", 900_000, json!({"prompt": "again", "turnId": "u120"})));
        events.push(base("session.permission", 900_001, ask));
        let mut t = Transcript::new();
        t.load(events);
        assert_eq!(t.docked_key(), Some("permission:late"));
        assert_eq!(t.docked().map(|d| d.row.key), Some("permission:late".to_owned()));
        let looked = t.looked.get();
        assert!(looked < 10, "two reads of the docked prompt looked at {looked} of {} entries", t.model.entries.len());
    }

    #[test]
    fn a_page_landing_folds_the_page_and_the_turn_it_meets() {
        let events = thread(200);
        let k = events.iter().position(|e| matches!(e, SessionEvent::SessionStart { turn_id: Some(t), .. } if t == "u40")).unwrap() - 7;
        let mut t = Transcript::new();
        t.load(events[k..].to_vec());
        let read = t.take_cost();
        t.prepend(events[..k].to_vec());
        let landed = t.take_cost();
        assert!(landed * 3 < read, "a page of {k} events cost {landed} to land under a thread that cost {read} to read");
    }

    #[test]
    fn a_live_event_stays_cheap_for_an_agent_that_rewinds_by_count() {
        let mut events = thread(900);
        let last = events.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
        events.truncate(last + 6);
        let mut t = Transcript::new();
        t.can(Can { forks: true, forks_by_count: true, rewinds_conversation: true, rewinds_by_count: true });
        t.load(events);
        let load = std::mem::take(&mut t.cost);
        t.push(base("session.delta", 999_999, json!({"kind": "text", "text": " more", "messageId": "a899", "turnId": "u899"})));
        assert!(t.cost * 100 < load, "a live event cost {} against a load's {load}", t.cost);
    }

    #[test]
    fn a_live_event_stays_cheap_after_an_earlier_turns_run_lands_in_the_latest_turn() {
        let mut events = thread(897);
        let last = events.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
        events.truncate(last + 6);
        let late = |e: &SessionEvent| matches!(e, SessionEvent::SessionRun { turn_id, .. } if turn_id.as_deref() == Some("u894"));
        assert!(events[last..].iter().any(late));
        let delta = base("session.delta", 999_999, json!({"kind": "text", "text": " more", "messageId": "a896", "turnId": "u896"}));
        cheap_push(events, delta);
    }

    #[test]
    fn a_live_event_stays_cheap_after_an_earlier_line_lands_in_the_latest_turn() {
        let earlier = |pos: u64, text: &str| {
            event(json!({"type": "session.earlier", "workspaceId": "w", "sessionId": "s", "pos": pos, "who": "person", "text": text}))
        };
        let mut events = vec![earlier(100_000, "from before")];
        events.extend(thread(300));
        let last = events.iter().rposition(|e| matches!(e, SessionEvent::SessionStart { .. })).unwrap();
        events.truncate(last + 6);
        events.insert(last + 1, earlier(100_001, "also from before"));
        let delta = base("session.delta", 999_999, json!({"kind": "text", "text": " more", "messageId": "a299", "turnId": "u299"}));
        cheap_push(events, delta);
    }

    #[test]
    fn each_turn_keeps_its_context_figures_and_the_latest_start_names_the_harness() {
        let events = vec![
            base("session.start", 1, json!({"prompt": "a", "agent": "codex"})),
            base("session.context", 2, json!({"context": 40.0, "window": 100.0})),
            base("session.done", 3, json!({"result": {"status": "completed", "tokens": {"input": 1, "output": 2, "context": 50.0}}})),
            base("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
            base("session.start", 5, json!({"prompt": "b", "turnId": "u2"})),
            base("session.context", 6, json!({"context": 70.0, "turnId": "u2"})),
        ];
        let mut t = Transcript::new();
        t.load(events);
        let first = TurnContext {
            counted: true,
            tokens_context: Some(50.0),
            held_context: Some(40.0),
            held_window: Some(100.0),
            ..TurnContext::default()
        };
        let second = TurnContext { running: true, held_context: Some(70.0), ..TurnContext::default() };
        assert_eq!(t.turn_contexts(), vec![first, second]);
        assert_eq!(t.harness().as_deref(), Some("codex"));
    }

    #[test]
    fn a_preview_is_the_first_line_worth_reading_cut_at_84_characters() {
        assert_eq!(preview("\n  \n```\n  first   line  \nsecond").as_deref(), Some("first line"));
        assert_eq!(preview("```\n\n```").as_deref(), Some("2 lines"));
        assert_eq!(preview("```"), None);
        assert_eq!(preview(""), None);
        let exact = "x".repeat(84);
        assert_eq!(preview(&exact), Some(exact.clone()));
        let words = format!("{} tail of the line", "y".repeat(82));
        assert_eq!(preview(&words), Some(format!("{}…", "y".repeat(82))));
        let long = "z".repeat(100_000);
        assert_eq!(preview(&long), Some(format!("{}…", "z".repeat(83))));
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
    #[test]
    fn a_turn_the_person_opened_stays_open_when_the_next_starts() {
        let mut t = Transcript::new();
        t.load(turn(true));
        t.toggle("u1");
        assert!(t.rows().iter().any(|r| matches!(r.kind, RowKind::Fold { open: true })), "the fold did not open");
        t.push(base("session.start", 12, json!({"prompt": "more", "turnId": "u2"})));
        t.push(base("session.delta", 13, json!({"kind": "text", "text": "Ok.", "turnId": "u2"})));
        assert!(t.rows().iter().any(|r| matches!(r.kind, RowKind::Fold { open: true })), "the next turn shut the opened fold");
        t.push(base("session.done", 14, json!({"result": {"status": "completed"}, "turnId": "u2"})));
        let folds: Vec<bool> = t
            .rows()
            .iter()
            .filter_map(|r| match r.kind {
                RowKind::Fold { open } => Some(open),
                _ => None,
            })
            .collect();
        assert_eq!(folds.first(), Some(&true), "the next turn's settling shut the opened fold: {folds:?}");
    }

    #[test]
    fn a_retry_stands_until_the_agent_writes_into_its_turn() {
        let retrying = |events: &[SessionEvent]| {
            rows(events, &HashSet::new()).iter().find_map(|r| match &r.kind {
                RowKind::Working { retry, .. } => Some(retry.clone()),
                _ => None,
            })
        };
        let mut events = vec![
            base("session.start", 1, json!({"prompt": "go"})),
            base("session.retry", 2, json!({"attempt": 2, "of": 10, "why": "overloaded"})),
        ];
        assert_eq!(retrying(&events), Some(Some("Overloaded, trying again (2 of 10)".to_owned())));
        events.push(base("session.retry", 3, json!({"attempt": 3})));
        assert_eq!(retrying(&events), Some(Some("Trying again (3)".to_owned())));
        events.push(base("session.notice", 4, json!({"text": "Machine woke up"})));
        assert_eq!(retrying(&events), Some(Some("Trying again (3)".to_owned())), "a notice is no sign the call got through");
        events.push(base("session.delta", 5, json!({"kind": "text", "text": "Here"})));
        assert_eq!(retrying(&events), Some(None), "the reply left the retry standing");
        let mut t = Transcript::new();
        t.load(events[..2].to_vec());
        t.push(events[2].clone());
        assert!(t.rows().iter().any(|r| matches!(&r.kind, RowKind::Working { retry: Some(line), .. } if line == "Trying again (3)")));
        t.push(events[4].clone());
        assert!(t.rows().iter().any(|r| matches!(r.kind, RowKind::Working { retry: None, .. })), "a live reply left the retry standing");
    }
}

/// What Rewind to here's dialog says about one earlier turn (`RewindRequest` in shellRequests.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct RewindFacts {
    pub turns_after: usize,
    pub files: bool,
    pub cuts_conversation: bool,
    pub kept: Option<String>,
    pub harness: Option<String>,
}

impl Transcript {
    /// The facts of the turn `turn`, while the transcript holds it.
    pub(crate) fn rewind_facts(&self, turn: &str) -> Option<RewindFacts> {
        let turns = &self.model.turns;
        let at = turns.iter().position(|t| t.id == turn)?;
        let checkpoint = |id: &str| {
            self.events.iter().rev().find_map(|e| match e {
                SessionEvent::SessionCheckpoint { turn_id, kept, .. } if turn_id.as_deref() == Some(id) => Some(kept.clone()),
                _ => None,
            })
        };
        let kept = turns.iter().filter(|t| t.kept.as_ref().is_some_and(|k| k.kept)).find_map(|t| checkpoint(&t.id).flatten());
        Some(RewindFacts {
            turns_after: turns.len() - 1 - at,
            files: turns[at].kept.as_ref().is_some_and(|k| k.files),
            cuts_conversation: self.can.rewinds_conversation && kept.is_none(),
            kept,
            harness: self.harness(),
        })
    }
}

/// One turn's context figures as TurnSummary's `tokens` and `held` carry them; `counted` where its reply carried tokens.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TurnContext {
    pub running: bool,
    pub failed: bool,
    pub counted: bool,
    pub tokens_context: Option<f64>,
    pub tokens_window: Option<f64>,
    pub held_context: Option<f64>,
    pub held_window: Option<f64>,
}

impl Transcript {
    /// Each turn's context figures, oldest first, for the header's context ring.
    pub fn turn_contexts(&self) -> Vec<TurnContext> {
        self.model
            .turns
            .iter()
            .map(|t| TurnContext {
                running: t.state == State::Running,
                failed: t.state == State::Failed,
                counted: t.tokens.is_some(),
                tokens_context: t.tokens.and_then(|(context, _)| context),
                tokens_window: t.tokens.and_then(|(_, window)| window),
                held_context: t.held.map(|(context, _)| context),
                held_window: t.held.and_then(|(_, window)| window),
            })
            .collect()
    }

    /// The harness the thread's latest turn ran on, where its start named one.
    pub fn harness(&self) -> Option<String> {
        self.model.agent.clone()
    }

    pub fn event_list(&self) -> &[SessionEvent] {
        &self.events
    }
    /// The prompt the latest turn waits on where the thread's own agent raised it and nobody has answered
    /// (`openPrompt` in WorkspaceThread.tsx): the dock's, whose row keeps only the waiting record.
    pub fn docked(&self) -> Option<Docked<'_>> {
        let (e, p) = self.waiting()?;
        Some(Docked {
            row: prompt_row(&e.key, e.turn.clone(), p, None),
            tool: &p.tool,
            input: &p.input,
            detail: p.detail.as_deref(),
            cwd: self.model.cwd.as_deref(),
        })
    }

    /// The key of the row [`Transcript::docked`] answers, without drawing it.
    pub fn docked_key(&self) -> Option<&str> {
        self.waiting().map(|(e, _)| e.key.as_str())
    }

    /// The latest turn's first unanswered prompt, read off the entries from that turn's opening on where the fold
    /// marked it.
    fn waiting(&self) -> Option<(&Entry, &Prompt)> {
        let latest = self.model.turns.last()?.id.as_str();
        let from = self.mark.as_ref().map_or(0, |m| m.entries.min(self.model.entries.len()));
        #[cfg(test)]
        self.looked.set(self.looked.get() + self.model.entries.len() - from);
        self.model.entries[from..].iter().find_map(|e| match &e.body {
            Body::Prompt(p) if p.outcome.is_none() && e.turn.as_deref() == Some(latest) => Some((e, &**p)),
            _ => None,
        })
    }
}

/// The prompt the composer's dock answers: its row, the call it asks about, and the thread's folder.
pub struct Docked<'a> {
    pub row: Row,
    pub tool: &'a str,
    pub input: &'a str,
    pub detail: Option<&'a str>,
    pub cwd: Option<&'a str>,
}

/// A thread the transcript names: where a press on its row or its Open goes.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadAt {
    pub workspace: String,
    pub thread: String,
}

/// Which part of a lead's tree a child stands in (`ChildPart` in leadTree.ts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildPart {
    Live,
    Finished,
    Settled,
}

/// A child in the open thread's tree, as its row draws it (`ThreadRow` and `SubagentRow` in ThreadRows.tsx, `LeadNode`
/// in leadTree.ts): a thread, or a subagent of the thread `at` names, with what stands under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub at: ThreadAt,
    pub title: String,
    pub harness: String,
    /// The status slot's kind as the tree reads it, and the slot's time or age.
    pub kind: crate::screens::sidebar::model::Kind,
    pub slot: Option<String>,
    /// The part it stands in, read off its whole subtree, and its own part and rank alone, which the counts read.
    pub part: ChildPart,
    pub own: ChildPart,
    pub rank: u8,
    pub started: f64,
    /// The computer it runs on, where that is not the lead's.
    pub computer: Option<String>,
    pub note: Option<String>,
    /// A subagent's call, which its page opens by; its id where no call names it.
    pub subagent: Option<String>,
    /// What a subagent was asked and the model it runs, which its card says, and its own id, which Stop subagent
    /// names.
    pub asked: Option<String>,
    pub model: Option<String>,
    pub task: Option<String>,
    /// What its acts read (`ChildTarget`): the session Stop interrupts, whether its turn runs, whether anything in its
    /// subtree works, whether threads stand under it, and how many threads a settle of it takes.
    pub session: String,
    pub running: bool,
    pub working: bool,
    pub under: bool,
    pub threads: usize,
    /// Live, then finished, then settled, each in its order; none under a settled thread or a subagent.
    pub children: Vec<Listed>,
}

impl Listed {
    /// A thread with nothing under it, in `part` alone, as a test or a host's row would name it.
    pub fn thread(at: ThreadAt, title: &str, kind: crate::screens::sidebar::model::Kind, part: ChildPart) -> Listed {
        Listed {
            session: at.thread.clone(),
            at,
            title: title.to_owned(),
            harness: "claude".to_owned(),
            kind,
            slot: None,
            part,
            own: part,
            rank: if part == ChildPart::Live { 2 } else { 4 },
            started: 0.,
            computer: None,
            note: None,
            subagent: None,
            asked: None,
            model: None,
            task: None,
            running: kind == crate::screens::sidebar::model::Kind::Working,
            working: kind == crate::screens::sidebar::model::Kind::Working,
            under: false,
            threads: 1,
            children: Vec::new(),
        }
    }
}

/// One of the thread's agent's subagents as the listing carries it (`SubagentView`).
#[derive(Clone, Debug, PartialEq)]
pub struct ListedSubagent {
    /// The call that launched it, which its page is opened by.
    pub call: String,
    pub title: String,
    /// running, done, failed or stopped.
    pub state: String,
    pub model: Option<String>,
    pub asked: Option<String>,
    pub started: f64,
    pub ended: Option<f64>,
    pub failure: Option<String>,
}

/// The thread this one forked from, as the listing names it; `at` while that thread is listed, which Open goes to.
#[derive(Clone, Debug, PartialEq)]
pub struct ForkSource {
    pub title: String,
    pub at: Option<ThreadAt>,
}

/// A sub-thread's latest machine start, as the line under its message says it (`SpawnLine.tsx`, `spawnLog.ts`): under
/// way, done, or failed on a step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StartFailed {
    /// The cloud by the name a person reads.
    pub on: String,
    /// The step it stopped on, as its line names it (`SUB_SPAWN_WORDS`), and what the host said.
    pub step: String,
    pub said: String,
    pub fix: Option<String>,
    /// The message the start was for, which Try again sends again.
    pub prompt: Option<String>,
    pub log: Vec<String>,
    /// Whether it failed, when it began (ms epoch), when the machine stood ready, and how long it took to the agent's
    /// first answer once one came.
    pub failed: bool,
    pub since: f64,
    pub ready: Option<f64>,
    pub took: Option<f64>,
    /// Each step with its mark, words, facts and time, in the order the host takes them.
    pub steps: Vec<SpawnStep>,
}

/// One step of a machine start (`StepLine`).
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnStep {
    pub id: String,
    pub name: String,
    pub note: Option<String>,
    pub state: StepState,
    /// How long a done step took, or when a step under way began.
    pub ms: Option<f64>,
    pub since: Option<f64>,
    pub said: Option<String>,
    pub fix: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Working,
    Done,
    Warned,
    Failed,
    Waiting,
}

/// What a sub-thread's strip over the composer says (`SubThreadStrip` in SubStrip.tsx): its key, glyph, title, why,
/// and acts by their words.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub key: &'static str,
    pub title: String,
    pub note: String,
    pub acts: Vec<StripAct>,
}

/// One act of a sub-thread's strip, by what it does (`SubStripAct` in SubStrip.tsx).
#[derive(Clone, Debug, PartialEq)]
pub enum StripAct {
    /// Raise machines at once, in the settings of the computer the cap names, by its place id.
    Raise(Option<String>),
    /// Copy to the computer the host runs on, by its name: the sub-thread's commits copied home.
    CopyHome(String),
    /// Push again: asks the thread's agent to push its branch again.
    PushAgain(String),
    /// Add the cloud again, by the name a person reads it by: Settings' Computers, with Add a cloud open.
    AddAgain(String),
    /// Start again at the next size the cloud sells: asks the lead, on its agent, to start the work again there.
    StartAgain { size: String, lead: ThreadAt, harness: String, prompt: String },
}

impl StripAct {
    /// Its words, `SUB_STRIP` in packages/protocol/src/sub-thread.ts.
    pub fn word(&self) -> String {
        match self {
            StripAct::Raise(_) => "Raise machines at once".to_owned(),
            StripAct::CopyHome(here) => format!("Copy to {here}"),
            StripAct::PushAgain(_) => "Push again".to_owned(),
            StripAct::AddAgain(on) => format!("Add {on} again"),
            StripAct::StartAgain { size, .. } => format!("Start again at {size}"),
        }
    }
}

/// `subPushAgainPrompt`: what Push again sends the thread.
pub fn push_again_prompt(branch: &str) -> String {
    format!("Push {branch} again, and say what the remote answers.")
}

/// What the thread listing says about the open thread and the threads around it, read once each time the view follows
/// the thread: the rows the transcript draws off it are the fork rule's title and Open, Rewind held on a capped turn,
/// the open question this thread waits behind, the children's rows and the Threads section.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listing {
    pub thread: String,
    /// The thread's own title as the sidebar reads it.
    pub title: Option<String>,
    pub workspace: Option<String>,
    pub harness: Option<String>,
    /// The latest turn waits for its computer's threads at once (`capped`).
    pub capped: bool,
    pub source: Option<ForkSource>,
    /// The thread this one's running call waits behind, its title and its open question.
    pub waiting_on: Option<(String, serde_json::Value)>,
    /// The children's tree: the threads it opened, then the subagents its agent runs, at every depth.
    pub children: Vec<Listed>,
    /// The person shut the Threads section under this thread (`threadsShut` in the preferences).
    pub shut: bool,
    pub subagents: Vec<ListedSubagent>,
    pub strip: Option<Strip>,
    pub start_failed: Option<StartFailed>,
    /// The machine line under the transcript (`machineLine` in notices/workspaceLines.ts): what the machine lacks for
    /// wsp, or its memory near full; None where the person has nothing to do. The workspace's watcher sets it.
    pub machine: Option<String>,
    /// The notice under a send in flight while its agent starts (`agentStartingLine`), until its session starts; the
    /// send's link sets it off `session.starting`.
    pub starting: Option<String>,
    /// The words of a send in flight, the person's message at once until its turn starts (`pendingPrompt`).
    pub sent: Option<String>,
    /// What the working line says in its place while the thread's workspace cannot run the turn (`turnWait`).
    pub wait: Option<Wait>,
    /// The workspace's one word while it is paused, said under a quiet thread with Wake beside it; none on a
    /// sub-thread, whose nap is its delete timer's.
    pub paused: Option<String>,
}

/// `turnWait` in adapt/workspaces.ts: the words, whether Wake stands beside them, and whether the turn's elapsed does.
#[derive(Clone, Debug, PartialEq)]
pub struct Wait {
    pub label: String,
    pub wake: bool,
    pub elapsed: bool,
}

/// The key of the paused line under the transcript, which the rule draws with Wake.
pub const PAUSED_KEY: &str = "paused";

/// `waitingAskerLine` in packages/protocol/src/words/tools.ts.
pub fn waiting_asker_line(title: &str) -> String {
    format!("{title} asks; this thread waits on the answer")
}

/// `agentStartingLine` in packages/protocol/src/views/session-events.ts.
pub fn agent_starting_line(agent: &str, installs: bool) -> String {
    if installs {
        format!("{agent} installs on first run")
    } else {
        format!("Starting {agent}")
    }
}

/// `cloudSaid`: a cloud by the name its key rows give it (`PROVIDER_KEY_WORDS`), any other word as itself.
fn cloud_said(on: &str) -> String {
    crate::screens::sidebar::looks::PROVIDER_NAMES
        .iter()
        .find(|(id, name)| on.eq_ignore_ascii_case(id) || on.eq_ignore_ascii_case(name))
        .map_or_else(|| on.to_owned(), |(_, name)| (*name).to_owned())
}

impl Listing {
    /// The listing off the rows the sidebar holds, read for `thread` at `now_ms`: a child settles after `settle_ms` of
    /// quiet, the person's `settleAfter`, and the section is shut where the preferences say so.
    pub fn listed(
        rows: &[side::Row],
        thread: &str,
        now_ms: f64,
        settle_ms: Option<f64>,
        preferences: &serde_json::Value,
        here: &str,
    ) -> Listing {
        let key = |r: &side::Row| r.thread_id.clone().unwrap_or_else(|| r.id.clone());
        let latest = |id: &str| {
            rows.iter().filter(|r| key(r) == id).max_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.)))
        };
        let own = latest(thread);
        let text = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
        let workspace = own.map(|r| r.workspace_id.clone());
        let computer = own.and_then(|r| r.computer_name.clone());
        let source = own.and_then(|r| {
            let (id, title) = (r.forked_from_thread.clone()?, r.forked_from.clone()?);
            let at = latest(&id).map(|r| ThreadAt { workspace: r.workspace_id.clone(), thread: id.clone() });
            Some(ForkSource { title, at })
        });
        let waiting_on = own.and_then(|r| r.waiting_on_record.as_ref()).and_then(|w| Some((text(w, "title")?, w.clone())));
        let subagents = own
            .map(|r| {
                r.subagents
                    .iter()
                    .filter_map(|s| {
                        Some(ListedSubagent {
                            call: s.call.clone()?,
                            title: s.title.clone(),
                            state: match s.state {
                                side::SubagentState::Running => "running",
                                side::SubagentState::Done => "done",
                                side::SubagentState::Failed => "failed",
                                side::SubagentState::Stopped => "stopped",
                            }
                            .to_owned(),
                            model: s.model.clone(),
                            asked: s.asked.clone(),
                            started: s.started_at,
                            ended: s.ended_at,
                            failure: s.failure.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Only the thread's own tree is folded: a host's rows on every computer would cost a fold on every push.
        let mut under: HashMap<&str, Vec<&str>> = HashMap::new();
        for r in rows {
            if let Some(parent) = r.parent_thread_id.as_deref() {
                under.entry(parent).or_default().push(r.thread_id.as_deref().unwrap_or(&r.id));
            }
        }
        let mut tree: HashSet<&str> = HashSet::from([thread]);
        let mut walk = vec![thread];
        while let Some(at) = walk.pop() {
            for &kid in under.get(at).into_iter().flatten() {
                if tree.insert(kid) {
                    walk.push(kid);
                }
            }
        }
        let rows: Vec<side::Row> = rows.iter().filter(|r| tree.contains(r.thread_id.as_deref().unwrap_or(&r.id))).cloned().collect();
        let rows = rows.as_slice();
        let threads = side::fold_threads(rows);
        let mut order: Vec<usize> = (0..threads.len()).collect();
        let start = |i: usize| threads[i].started_at.unwrap_or(0.);
        order.sort_by(|&a, &b| start(b).total_cmp(&start(a)).then_with(|| threads[a].key.cmp(&threads[b].key)));
        let forest = side::forest(&threads, &order);
        let by_id: HashMap<&str, &side::Row> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
        let tree = Reading { cx: side::Ctx { all: &threads, now: now_ms, settle: settle_ms }, rows: by_id, lead_computer: computer };
        let children = find_node(&forest, thread).map(|node| tree.children(node)).unwrap_or_default();
        let (strip, start_failed) = own.map(|own| sub_of(own, here, now_ms)).unwrap_or_default();
        Listing {
            thread: thread.to_owned(),
            title: threads.iter().find(|t| t.thread_id.as_deref() == Some(thread)).map(|t| t.title.clone()),
            workspace,
            harness: own.map(|r| r.harness.clone()).filter(|h| !h.is_empty()),
            capped: own.is_some_and(|r| r.capped),
            source,
            waiting_on,
            children,
            shut: preferences.pointer(&format!("/threadsShut/{}", thread.replace('~', "~0").replace('/', "~1")))
                == Some(&serde_json::Value::Bool(true)),
            subagents,
            strip,
            start_failed,
            machine: None,
            starting: None,
            sent: None,
            wait: None,
            paused: None,
        }
    }

    /// The same off the rows as JSON, at the record's default settle and with nothing shut, for a test.
    #[cfg(test)]
    pub fn of(rows: &[serde_json::Value], thread: &str, now_ms: f64) -> Listing {
        let rows: Vec<side::Row> = rows.iter().cloned().filter_map(side::Row::owned).collect();
        Listing::listed(&rows, thread, now_ms, Some(side::SETTLE_MS_DEFAULT), &serde_json::Value::Null, "")
    }

    /// The thread the listing names `thread` as a child of the open one: a thread, never a subagent.
    pub fn child(&self, thread: &str) -> Option<&Listed> {
        self.children.iter().find(|c| c.subagent.is_none() && c.at.thread == thread)
    }

    pub fn subagent(&self, call: &str) -> Option<&ListedSubagent> {
        self.subagents.iter().find(|s| s.call == call)
    }

    /// The tree's figures as the shut head says them (`countTree`).
    pub fn counts(&self) -> TreeCounts {
        let mut c = TreeCounts::default();
        fn walk(node: &Listed, c: &mut TreeCounts) {
            if node.own == ChildPart::Live {
                match node.rank {
                    0 => c.needs_you += 1,
                    1 => c.failed += 1,
                    3 => c.waiting += 1,
                    _ if node.subagent.is_some() => c.working_subagents += 1,
                    _ => c.working += 1,
                }
            }
            node.children.iter().for_each(|kid| walk(kid, c));
        }
        self.children.iter().for_each(|node| walk(node, &mut c));
        c.working += c.working_subagents;
        c.finished = self.finished().1;
        c
    }

    /// The one live thread or subagent anywhere in the tree a shut section shows: the most pressing by its own rank,
    /// newest first within a rank (`mostPressing`).
    pub fn most_pressing(&self) -> Option<&Listed> {
        fn walk<'a>(node: &'a Listed, best: &mut Option<&'a Listed>) {
            if node.own == ChildPart::Live && best.is_none_or(|b| node.rank < b.rank || node.rank == b.rank && node.started > b.started) {
                *best = Some(node);
            }
            node.children.iter().for_each(|kid| walk(kid, best));
        }
        let mut best = None;
        self.children.iter().for_each(|node| walk(node, &mut best));
        best
    }

    /// Every finished thread anywhere in the tree, each once, and how many threads they hold: what Settle N finished
    /// sends and counts (`finishedTake`).
    pub fn finished(&self) -> (Vec<String>, usize) {
        fn take(nodes: &[Listed], out: &mut (Vec<String>, usize)) {
            for node in nodes.iter().filter(|n| n.subagent.is_none()) {
                if node.part == ChildPart::Finished {
                    out.0.push(node.at.thread.clone());
                    out.1 += node.threads;
                } else {
                    take(&node.children, out);
                }
            }
        }
        let mut out = (Vec::new(), 0);
        take(&self.children, &mut out);
        out
    }
}

/// A lead's tree in figures (`TreeCounts` in leadTree.ts): every live thread and subagent at any depth by what it does,
/// and the threads Settle N finished takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeCounts {
    pub needs_you: usize,
    pub failed: usize,
    pub working: usize,
    pub working_subagents: usize,
    pub waiting: usize,
    pub finished: usize,
}

fn find_node<'t>(nodes: &'t [side::Tree], key: &str) -> Option<&'t side::Tree> {
    nodes.iter().find_map(|n| if n.key == key { Some(n) } else { find_node(&n.children, key) })
}

/// The sidebar's reading of a lead's tree, with the rows as the host sent them for what that reading leaves out.
struct Reading<'a> {
    cx: side::Ctx<'a>,
    rows: HashMap<&'a str, &'a side::Row>,
    lead_computer: Option<String>,
}

impl Reading<'_> {
    /// A thread's children, live then finished then settled (`childParts`), each with its own under it.
    fn children(&self, node: &side::Tree) -> Vec<Listed> {
        let [live, finished, settled] = self.cx.child_parts(self.cx.lead_nodes(node));
        let parts = [(live, ChildPart::Live), (finished, ChildPart::Finished), (settled, ChildPart::Settled)];
        parts
            .into_iter()
            .flat_map(|(nodes, part)| nodes.into_iter().map(move |l| (l, part)))
            .filter_map(|(l, part)| self.node(l, part))
            .collect()
    }

    fn works(&self, l: side::Lead) -> bool {
        match l {
            side::Lead::Sub(i, s) => self.cx.all[i].subagents[s].state == side::SubagentState::Running,
            side::Lead::Thread(n) => {
                n.thread.is_some_and(|i| self.cx.all[i].status == side::Status::Running)
                    || self.cx.lead_nodes(n).into_iter().any(|kid| self.works(kid))
            }
        }
    }

    fn node(&self, l: side::Lead, part: ChildPart) -> Option<Listed> {
        let (now, all) = (self.cx.now, self.cx.all);
        let own = match self.cx.own_part(l) {
            side::Part::Live => ChildPart::Live,
            side::Part::Finished => ChildPart::Finished,
            side::Part::Settled => ChildPart::Settled,
        };
        let side_part = match part {
            ChildPart::Live => side::Part::Live,
            ChildPart::Finished => side::Part::Finished,
            ChildPart::Settled => side::Part::Settled,
        };
        let rank = self.cx.own_rank(l, self.cx.own_part(l));
        let text = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
        match l {
            side::Lead::Sub(i, s) => {
                let (of, sub) = (&all[i], &all[i].subagents[s]);
                let kind = side::subagent_kind(sub, side_part);
                // noteOf for a subagent: why it failed, else once ended its last line.
                let note = match (part, sub.state) {
                    (ChildPart::Settled, _) | (_, side::SubagentState::Running) => None,
                    (_, side::SubagentState::Failed) => sub.failure.clone(),
                    _ => sub.last_line.clone(),
                };
                Some(Listed {
                    at: ThreadAt { workspace: of.workspace_id.clone(), thread: of.key.clone() },
                    title: sub.title.clone(),
                    harness: of.harness.clone(),
                    kind,
                    slot: match kind {
                        side::Kind::Working => Some(side::running_time(now - sub.started_at)),
                        side::Kind::Resting => Some(side::resting_age(sub.ended_at.unwrap_or(sub.started_at), now)),
                        _ => None,
                    },
                    part,
                    own,
                    rank,
                    started: sub.started_at,
                    computer: None,
                    note,
                    subagent: Some(sub.call.clone().unwrap_or_else(|| sub.id.clone())),
                    asked: sub.asked.clone(),
                    model: sub.model.clone(),
                    task: Some(sub.id.clone()),
                    session: of.session_id.clone(),
                    running: sub.state == side::SubagentState::Running,
                    working: sub.state == side::SubagentState::Running,
                    under: false,
                    threads: 0,
                    children: Vec::new(),
                })
            }
            side::Lead::Thread(n) => {
                let t = &all[n.thread?];
                let kind = match (part, side::kind_of(t, now)) {
                    (ChildPart::Settled, _) => side::Kind::Resting,
                    (ChildPart::Finished, side::Kind::Resting) if t.status == side::Status::Completed => side::Kind::Finished,
                    (_, kind) => kind,
                };
                let raw = self.rows.get(t.session_id.as_str()).and_then(|r| r.capped_record.as_ref());
                // noteOf for a thread: what it asks, why it failed, the threads at once that hold it, a finished one's last line.
                let capped = raw.and_then(|c| {
                    let (place, running, at_once) = (text(c, "place")?, c.get("running")?.as_u64()?, c.get("atOnce")?.as_u64()?);
                    Some(match text(c, "why").as_deref() {
                        Some("minute") => format!("{place}'s starts this minute are used: {running} of {at_once}"),
                        why => {
                            let word = if why == Some("slot") { "machine" } else { "thread" };
                            format!("{place} is running {running} of {at_once} {word}{}", if at_once == 1 { "" } else { "s" })
                        }
                    })
                });
                let note = match part {
                    ChildPart::Settled => None,
                    _ if t.asking_text.is_some() => t.asking_text.clone(),
                    _ if t.status == side::Status::Failed => t.failure.clone(),
                    _ if capped.is_some() => capped,
                    ChildPart::Finished => t.last_line.clone(),
                    ChildPart::Live => None,
                };
                let children = if part == ChildPart::Settled { Vec::new() } else { self.children(n) };
                // What works under it is read off the children just built; a settled thread's are not built.
                let working = t.status == side::Status::Running
                    || if part == ChildPart::Settled { self.works(l) } else { children.iter().any(|c| c.working) };
                fn count(n: &side::Tree) -> usize {
                    usize::from(n.thread.is_some()) + n.children.iter().map(count).sum::<usize>()
                }
                Some(Listed {
                    at: ThreadAt { workspace: t.workspace_id.clone(), thread: t.thread_id.clone().unwrap_or_else(|| t.key.clone()) },
                    title: t.title.clone(),
                    harness: t.harness.clone(),
                    kind,
                    slot: match kind {
                        side::Kind::Working => t.started_at.map(|s| side::running_time(now - s)),
                        side::Kind::Resting => t.ended_at.or(t.started_at).map(|e| side::resting_age(e, now)),
                        _ => None,
                    },
                    part,
                    own,
                    rank,
                    started: t.started_at.unwrap_or(0.),
                    computer: t.computer.clone().filter(|c| Some(c) != self.lead_computer.as_ref()),
                    note,
                    subagent: None,
                    asked: None,
                    model: None,
                    task: None,
                    session: t.session_id.clone(),
                    running: t.status == side::Status::Running,
                    working,
                    under: !n.children.is_empty(),
                    threads: count(n),
                    children,
                })
            }
        }
    }
}

/// `SubThreadStrip` and the failed start's line off a sub-thread's latest row: the strip it calls for, if any, and
/// the start that failed, if one did.
fn sub_of(row: &side::Row, here: &str, now_ms: f64) -> (Option<Strip>, Option<StartFailed>) {
    let text = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    let Some(sub) = row.sub_record.as_ref() else { return (None, None) };
    let on = cloud_said(&text(sub, "on").unwrap_or_default());
    let status = match row.status {
        side::Status::Running => "running",
        side::Status::Failed => "failed",
        _ => "",
    };
    let spawn = sub.get("spawn").filter(|s| !s.is_null());
    let failed = spawn.and_then(|s| spawn_of(s, &on, row));
    let strip = |key: &'static str, title: String, note: String, acts: Vec<StripAct>| Some(Strip { key, title, note, acts });
    let state = text(sub, "state").unwrap_or_default();
    let capped = row.capped_record.as_ref();
    let failure = row.failure.clone().filter(|f| status == "failed" && is_sub_oom_line(f));
    let held = sub.get("held").filter(|h| !h.is_null());
    let branch = text(sub, "branch").unwrap_or_default();
    // SUB_STRIP in packages/protocol/src/sub-thread.ts.
    let shown = if status == "running" {
        capped.and_then(|c| {
            let (running, at_once) = (c.get("running")?.as_u64()?, c.get("atOnce")?.as_u64()?);
            if text(c, "why").as_deref() == Some("minute") {
                strip(
                    "waiting-minute",
                    format!("Waiting for {on}'s next minute of starts"),
                    format!("{running} of {at_once} starts this minute are used. It starts on its own within the minute."),
                    Vec::new(),
                )
            } else {
                strip(
                    "waiting-machines",
                    format!("Waiting for a machine on {on}"),
                    format!("{running} of {at_once} machines are running. It starts on its own when one frees."),
                    vec![StripAct::Raise(text(c, "placeId"))],
                )
            }
        })
    } else if sub.get("cloudGone").and_then(|g| g.as_bool()) == Some(true) {
        strip(
            "forgotten",
            format!("{on} was removed from {here}"),
            format!("This thread has no machine to start until {on} is added again. Its transcript stays."),
            vec![StripAct::AddAgain(on.clone())],
        )
    } else if let Some(line) = failure {
        strip("oom", "Killed for memory".to_owned(), capitalised(&line), Vec::new())
    } else if state == "released" && failed.as_ref().is_some_and(|f| f.failed) {
        None
    } else if state == "released" {
        strip(
            "released",
            "This thread's machine was deleted".to_owned(),
            format!("A message starts a new one on {on}, and the conversation goes on."),
            Vec::new(),
        )
    } else if let Some(held) = held {
        let count = |k: &str| held.get(k).and_then(|n| n.as_u64()).unwrap_or(0);
        let (files, commits) = (count("files"), count("commits"));
        let parts: Vec<String> = [
            (commits > 0).then(|| format!("{commits} {}", if commits == 1 { "commit" } else { "commits" })),
            (files > 0).then(|| format!("{files} changed {}", if files == 1 { "file" } else { "files" })),
        ]
        .into_iter()
        .flatten()
        .collect();
        let held_words = if parts.is_empty() { "work".to_owned() } else { parts.join(" and ") };
        let note = match sub.pointer("/result/pushRefused").and_then(|r| r.as_str()) {
            None => format!("They are on {branch} on this machine."),
            Some(refused) => format!("The push to {branch} was refused: {refused}."),
        };
        // `subKept`: the day the cloud may delete the napping machine's disk from.
        let kept = row.disk_kept_until.map(|until| format!(", but {}", crate::screens::computer::folds::disk_may_go_line(&on, until)));
        strip(
            "not-pushed",
            format!("{} not pushed", capitalised(&held_words)),
            format!("{note} This machine stays until the work is pushed or copied to your computer{}.", kept.unwrap_or_default()),
            vec![StripAct::CopyHome(here.to_owned()), StripAct::PushAgain(branch.clone())],
        )
    } else if let Some(at) = sub.get("deleteAt").and_then(|d| d.as_f64()) {
        strip(
            "deleting",
            format!("Its machine is deleted in {}", crate::screens::computer::folds::minutes_word(at - now_ms)),
            format!("Everything is pushed to {branch}. The transcript stays, and a message after the delete starts a new machine."),
            Vec::new(),
        )
    } else {
        None
    };
    (shown, failed)
}

/// `spawnLogOf`: a sub-thread's latest machine start off its row's `sub.spawn`, every step after a failed one not
/// started; the agent's own step is done once the thread's first answer is laid over it (`Around::dress`).
fn spawn_of(spawn: &serde_json::Value, on: &str, row: &side::Row) -> Option<StartFailed> {
    let text = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    let num = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_f64());
    let since = num(spawn, "startedAt")?;
    let failed = spawn.get("failed").filter(|f| !f.is_null());
    let failed_on = failed.and_then(|f| text(f, "step"));
    let mut steps: Vec<SpawnStep> = Vec::new();
    let mut stopped = false;
    let mut add = |step: SpawnStep, stopped: bool| {
        if !stopped || step.state == StepState::Warned {
            steps.push(step);
        } else {
            steps.push(SpawnStep { state: StepState::Waiting, ms: None, since: None, said: None, fix: None, ..step });
        }
    };
    let step = |id: &str, name: String, note: Option<String>, state: StepState| SpawnStep {
        id: id.to_owned(),
        name,
        note,
        state,
        ms: None,
        since: None,
        said: None,
        fix: None,
    };
    let timed = |mut s: SpawnStep, at: &serde_json::Value| {
        match s.state {
            StepState::Done => s.ms = num(at, "ms"),
            StepState::Working => s.since = num(at, "since"),
            StepState::Failed => {
                s.said = failed.and_then(|f| text(f, "said"));
                s.fix = failed.and_then(|f| text(f, "fix"));
            }
            _ => {}
        }
        s
    };
    let state_of = |at: &serde_json::Value, id: &str| match () {
        _ if failed_on.as_deref() == Some(id) => StepState::Failed,
        _ if num(at, "ms").is_some() => StepState::Done,
        _ => StepState::Working,
    };
    let branch = spawn.get("branch").filter(|b| !b.is_null());
    if let Some(b) = branch {
        let fact = format!(
            "{} is pushed at {}",
            text(b, "branch").unwrap_or_default(),
            text(b, "sha").unwrap_or_default().chars().take(7).collect::<String>()
        );
        add(SpawnStep { ms: num(b, "ms"), ..step("branch", "Checked the branch".to_owned(), Some(fact), StepState::Done) }, stopped);
    }
    let machine = spawn.get("machine").filter(|m| !m.is_null()).cloned();
    if machine.is_some() || failed_on.as_deref() == Some("machine") {
        let machine = machine.unwrap_or_else(|| serde_json::json!({ "since": since, "states": [] }));
        let state = state_of(&machine, "machine");
        let name = if state == StepState::Done { format!("Asked {on} for a machine") } else { format!("Asking {on} for a machine") };
        let states: Vec<String> = machine
            .get("states")
            .and_then(|s| s.as_array())
            .map(|s| s.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let note = text(&machine, "id").map(|id| std::iter::once(id).chain(states).collect::<Vec<_>>().join(", "));
        add(timed(step("machine", name, note, state), &machine), stopped);
        stopped |= state == StepState::Failed;
    }
    for line in spawn.get("replaced").and_then(|r| r.as_array()).into_iter().flatten().filter_map(|l| l.as_str()) {
        add(step(&format!("replaced:{line}"), line.to_owned(), None, StepState::Warned), stopped);
    }
    if let (Some(n), Some(b)) = (spawn.get("uncommitted").and_then(|n| n.as_u64()), branch) {
        let (name, sha) = (text(b, "branch").unwrap_or_default(), text(b, "sha").unwrap_or_default());
        let folder = row.project_name.clone().unwrap_or_default();
        let line = format!(
            "{n} uncommitted {} in {folder} did not travel; the sub-thread starts at {name} as pushed ({}).",
            if n == 1 { "change" } else { "changes" },
            sha.chars().take(7).collect::<String>()
        );
        add(step("uncommitted", line, None, StepState::Warned), stopped);
    }
    if let Some(why) = text(spawn, "notSent") {
        add(step("not-sent", format!("No git-ignored files were sent: {why}."), None, StepState::Warned), stopped);
    }
    let files = spawn.get("files").filter(|f| !f.is_null());
    if let Some(f) = files {
        let state = state_of(f, "files");
        let paths: Vec<String> = f
            .get("paths")
            .and_then(|p| p.as_array())
            .map(|p| p.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let n = paths.len();
        let name =
            format!("{} {n} picked {}", if state == StepState::Done { "Copied" } else { "Copying" }, if n == 1 { "file" } else { "files" });
        let note = format!("{}, {}", paths.join(", "), crate::ui::fmt_bytes(num(f, "bytes").unwrap_or(0.) as u64));
        add(timed(step("files", name, Some(note), state), f), stopped);
        stopped |= state == StepState::Failed;
    }
    let ready = num(spawn, "readyAt");
    let agent = crate::ui::catalog::name(&row.harness);
    let agent_note = ready.and(row.model.clone());
    add(
        SpawnStep {
            since: ready,
            ..step("agent", format!("Starting {agent}"), agent_note, if ready.is_some() { StepState::Working } else { StepState::Waiting })
        },
        stopped,
    );
    let failed_step = steps.iter().find(|s| s.state == StepState::Failed).map(|s| s.name.clone());
    Some(StartFailed {
        on: on.to_owned(),
        step: failed_step.unwrap_or_default(),
        said: failed.and_then(|f| text(f, "said")).unwrap_or_default(),
        fix: failed.and_then(|f| text(f, "fix")),
        prompt: text(spawn, "asked").or_else(|| row.prompt.clone()),
        log: spawn
            .get("log")
            .and_then(|l| l.as_array())
            .map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
        failed: failed.is_some(),
        since,
        ready,
        took: None,
        steps,
    })
}

/// `isSubOomLine`: a turn's failure the strip over the composer says in its place.
fn is_sub_oom_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.strip_prefix("the agent on ").and_then(|rest| rest.find(" was killed for memory: ")).is_some_and(|at| at > 0)
}

impl Listing {
    /// What the thread's workspace says in and under the transcript, off its record and live status as the sidebar
    /// holds them (`turnWait`, `workspaceWord` and `machineLine` in ChatView), with the newest memory reading of its
    /// computer, if one stands near full.
    pub fn with_workspace(self, s: &crate::sidebar::Sidebar, memory: Option<crate::screens::terminal::words::Memory>) -> Listing {
        use crate::screens::sidebar::{looks, menu};
        let Some(id) = self.workspace.clone() else { return self };
        let (Some(w), status) = s.workspace(&id) else { return self };
        let text = |v: Option<&serde_json::Value>, k: &str| v.and_then(|v| v.pointer(k)?.as_str()).map(str::to_owned);
        let state = menu::workspace_state(w, status);
        let name = text(Some(w), "/name").unwrap_or_else(|| id.clone());
        let kind = text(Some(w), "/kind");
        let local = kind.as_deref() == Some("local");
        let at = s
            .place_of(&id)
            .and_then(|place| s.around().places.iter().find(|p| p.get("id").and_then(|i| i.as_str()) == Some(place.as_str())).cloned());
        let where_ = at
            .as_ref()
            .map(looks::place_name)
            .filter(|n| !n.is_empty())
            .or_else(|| local.then(|| s.here_name()).filter(|n| !n.is_empty()))
            .or_else(|| text(Some(w), "/provider").map(|p| cloud_said(&p)))
            .or_else(|| text(Some(w), "/machineId"))
            .unwrap_or_else(|| name.clone());
        let wait = match state {
            "pausing" | "paused" => Some(Wait { label: format!("waiting for {name} to wake"), wake: true, elapsed: false }),
            "waking" => Some(Wait { label: format!("waking {name} on {where_}"), wake: false, elapsed: true }),
            "unreachable" => Some(Wait { label: format!("waiting for {name} to answer"), wake: false, elapsed: false }),
            "gone" => Some(Wait { label: format!("{name} is gone"), wake: false, elapsed: false }),
            "released" => Some(Wait { label: format!("{name}'s computer was deleted"), wake: false, elapsed: false }),
            _ => None,
        };
        let pause_mode = text(status, "/pauseMode").or_else(|| text(Some(w), "/pauseMode"));
        let sub = s.rows().iter().any(|r| r.thread_id.as_deref() == Some(self.thread.as_str()) && r.sub_record.is_some());
        let paused = (state == "paused" && !sub).then(|| menu::workspace_word(state, pause_mode.as_deref()).to_owned());
        let reach = text(status, "/reach/state");
        let lacks = text(status.or(Some(w)), "/daemonRefusedAt/why");
        let joined = local || kind.as_deref() == Some("place");
        let running = text(status, "/phase").or_else(|| text(Some(w), "/phase")).is_none_or(|p| p == "running");
        let machine = match lacks {
            _ if matches!(state, "pausing" | "paused") => None,
            Some(lacks) if joined && reach.as_deref() == Some("unsupported") => {
                Some(capitalised(lacks.split(',').next().unwrap_or_default().trim()))
            }
            _ => memory
                .filter(|m| running && m.near_full())
                .map(|m| format!("Out of memory, {}", crate::screens::thread_view::rows::bytes_of(m.used as u64, m.total as u64))),
        };
        let strip = self.strip.clone().map(|mut strip| {
            if strip.key == "oom" {
                strip.acts = start_again(s, &self.thread).into_iter().collect();
            }
            strip
        });
        Listing { wait, paused, machine, strip, ..self }
    }
}

/// `sizeWord`: cpus by memory in GB to a tenth, as a plan names a size.
fn size_word(size: &serde_json::Value) -> Option<String> {
    let (cpu, mem) = (size.get("cpu")?.as_f64()?, size.get("memMb")?.as_f64()?);
    Some(format!("{cpu}x{}", (mem / 1024. * 10.).round() / 10.))
}

/// Start again on a sub-thread its machine's memory killed (`SubThreadStrip`'s oom): the next size its cloud sells
/// (`subNextSize`, `subSizesSold`), asked of the lead that starts sub-threads; none where either is unknown.
fn start_again(s: &crate::sidebar::Sidebar, thread: &str) -> Option<StripAct> {
    let latest = |id: &str| {
        s.rows()
            .iter()
            .filter(|r| r.thread_id.as_deref() == Some(id))
            .max_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.)))
    };
    let own = latest(thread)?;
    let sub = own.sub_record.as_ref()?;
    let raw = sub.get("on")?.as_str()?;
    let on = cloud_said(raw);
    let text = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    // `namesPlace`: by its id or name, a provider also by its key's name in any case.
    let cloud = s.around().places.iter().find(|p| {
        let (id, name) = (text(p, "id").unwrap_or_default(), text(p, "name").unwrap_or_default());
        raw == id
            || raw == name
            || (text(p, "kind").as_deref() == Some("provider")
                && [id.clone(), name.clone(), cloud_said(&name)].iter().any(|n| n.eq_ignore_ascii_case(raw)))
    })?;
    let plan: Option<Vec<String>> =
        cloud.pointer("/plan/sizes").and_then(|p| p.as_array()).map(|p| p.iter().filter_map(|w| w.as_str().map(str::to_owned)).collect());
    let mem = sub.pointer("/size/memMb")?.as_f64()?;
    let next = cloud
        .get("sizes")?
        .as_array()?
        .iter()
        .filter(|o| plan.as_ref().is_none_or(|plan| size_word(o).is_some_and(|w| plan.contains(&w))))
        .filter(|o| o.get("memMb").and_then(|m| m.as_f64()).is_some_and(|m| m > mem))
        .min_by(|a, b| a["memMb"].as_f64().unwrap_or(0.).total_cmp(&b["memMb"].as_f64().unwrap_or(0.)))?;
    let size = size_word(next)?;
    let lead = latest(own.parent_thread_id.as_deref()?)?;
    let title = own.harness_title.clone().or_else(|| text(sub, "machine")).unwrap_or_default();
    Some(StripAct::StartAgain {
        prompt: format!("Start the work of the sub-thread \"{title}\" again on {on} at size {size}: its agent was killed for memory."),
        size,
        lead: ThreadAt { workspace: lead.workspace_id.clone(), thread: lead.thread_id.clone()? },
        harness: lead.harness.clone(),
    })
}

fn capitalised(line: &str) -> String {
    let mut chars = line.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

impl Transcript {
    /// Takes what the thread listing says; the rows read off it are folded again.
    pub fn listing(&mut self, listing: Listing) -> Option<Splice> {
        if self.around.listing.as_ref() == Some(&listing) {
            return None;
        }
        self.around.listing = Some(listing);
        self.refold_all()
    }

    /// The listing the transcript reads, once one has arrived.
    pub fn listed(&self) -> Option<&Listing> {
        self.around.listing.as_ref()
    }
}

/// What the transcript keeps from outside its own events.
#[derive(Clone, Debug, Default)]
struct Around {
    listing: Option<Listing>,
    /// The subagent whose page the rows are, by the call that launched it.
    page: Option<String>,
}

/// The rule line's own words: `forkedFromLine` in packages/protocol/src/words/thread.ts.
pub fn forked_from_line(title: &str) -> String {
    format!("Forked from {title}")
}

impl Around {
    /// Lays what the listing and the window know over the rows the events folded into: the fork rule's title and Open,
    /// no Rewind on a capped turn, each child's state, the borrowed question, the start that failed, and the foot.
    /// `split` counts the rows before the latest turn and moves with any row taken out before it.
    fn dress(&self, mut rows: Vec<Row>, mut split: Option<usize>, whole: bool, shut: bool) -> (Vec<Row>, Option<usize>) {
        let listing = self.listing.as_ref();
        let mut ix = 0;
        while ix < rows.len() {
            match &mut rows[ix].kind {
                RowKind::Fork { source, open } => match listing.map(|l| l.source.as_ref()) {
                    Some(Some(from)) => {
                        *source = from.title.clone();
                        *open = from.at.clone();
                        rows[ix].text = forked_from_line(&from.title);
                    }
                    // As today, no rule stands until the listing names the thread the fork came from.
                    _ => {
                        rows.remove(ix);
                        split = split.map(|s| if s > ix { s - 1 } else { s });
                        continue;
                    }
                },
                RowKind::Agent { rewind, .. } if listing.is_some_and(|l| l.capped) => *rewind = None,
                RowKind::Spawn { children } => {
                    for child in children.iter_mut() {
                        child.listed = child.thread.as_deref().and_then(|t| listing?.child(t)).cloned();
                        child.workspace = listing.and_then(|l| l.workspace.clone());
                    }
                }
                _ => {}
            }
            ix += 1;
        }
        let Some(listing) = listing else { return self.foot(rows, split, shut) };
        if let Some(start) = &listing.start_failed {
            // `machineStartAt`: after the person's first message at or after the start began, with some slack; a start
            // still under way or failed with no such message stands last, a finished one is not drawn.
            let person =
                rows.iter().position(|r| matches!(r.kind, RowKind::Person { at, .. } if at.is_none_or(|at| at >= start.since - 10_000.)));
            let answered = person.and_then(|p| {
                rows[p + 1..].iter().find_map(|r| match r.kind {
                    RowKind::Agent { at, .. } => at,
                    _ => None,
                })
            });
            let mut start = start.clone();
            if let (Some(ready), Some(answered)) = (start.ready, answered) {
                start.took = Some((answered - start.since).max(0.));
                if let Some(agent) = start.steps.iter_mut().find(|s| s.id == "agent") {
                    (agent.state, agent.ms, agent.since) = (StepState::Done, Some((answered - ready).max(0.)), None);
                }
            }
            let at = person.map_or(rows.len(), |p| p + 1);
            let stands = person.is_some() || start.failed || start.ready.is_none();
            if stands && (whole || at >= rows.len()) {
                let text = match start.took {
                    _ if start.failed => format!("Could not start a machine on {}", start.on),
                    None => format!("Starting a machine on {}", start.on),
                    Some(took) => format!("Started on {} in {}", start.on, crate::words::fmt_duration(took)),
                };
                rows.insert(
                    at.min(rows.len()),
                    Row { key: "start-failed".to_owned(), kind: RowKind::StartFailed { failed: start }, text, turn: None },
                );
                split = split.map(|s| if s >= at { s + 1 } else { s });
            }
        }
        if let Some((title, prompt)) = &listing.waiting_on {
            let working = rows.iter().any(|r| matches!(r.kind, RowKind::Working { .. }));
            if working {
                rows.retain(|r| !matches!(r.kind, RowKind::Thinking));
                for row in rows.iter_mut() {
                    if let RowKind::Working { waiting, .. } = &mut row.kind {
                        *waiting = true;
                        row.text = "Waiting for you".to_owned();
                    }
                }
                let text = |k: &str| prompt.pointer(k).and_then(|x| x.as_str()).map(str::to_owned);
                let options = prompt
                    .pointer("/prompt/options")
                    .and_then(|o| o.as_array())
                    .map(|o| {
                        o.iter()
                            .filter_map(|o| {
                                let s = |k: &str| o.get(k).and_then(|x| x.as_str()).map(str::to_owned);
                                Some((s("id")?, s("label")?, s("effect")?, s("mode")))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if let (Some(ask), Some(session), Some(tool), Some(thread)) =
                    (text("/prompt/askId"), text("/sessionId"), text("/prompt/toolName"), text("/threadId"))
                {
                    let borrowed = Prompt {
                        ask: ask.clone(),
                        session,
                        tool,
                        input: text("/prompt/input").unwrap_or_default(),
                        detail: text("/prompt/detail"),
                        options,
                        outcome: None,
                        picked: None,
                    };
                    rows.push(prompt_row(&format!("waiting-on:{thread}:{ask}"), None, &borrowed, Some(waiting_asker_line(title))));
                }
            }
        }
        if let Some(wait) = &listing.wait {
            for row in rows.iter_mut() {
                match &mut row.kind {
                    RowKind::Working { wait: at, .. } => *at = Some(wait.clone()),
                    // Nothing thinks on a machine that is not running; the row keeps its room.
                    RowKind::Thinking => row.text.clear(),
                    _ => {}
                }
            }
        }
        self.foot(rows, split, shut)
    }

    /// The rows under the transcript: the send's starting notice, the Threads section and the machine line.
    fn foot(&self, mut rows: Vec<Row>, split: Option<usize>, shut: bool) -> (Vec<Row>, Option<usize>) {
        // The settled turn's word stands under the Threads section, as ChatView's foot orders them.
        let word = rows.last().filter(|r| matches!(r.kind, RowKind::Footer { .. })).is_some().then(|| rows.pop()).flatten();
        if let Some(text) = self.listing.as_ref().and_then(|l| l.sent.as_ref()) {
            let kind = RowKind::Person { at: None, files: Vec::new(), fork: None, full: false, steered: false };
            rows.push(Row { key: "sent".to_owned(), kind, text: text.clone(), turn: None });
        }
        if let Some(line) = self.listing.as_ref().and_then(|l| l.starting.as_ref()) {
            rows.push(Row {
                key: "starting".to_owned(),
                kind: RowKind::Work { tone: Tone::Notice, glyph: Glyph::Info, body: None, open: false },
                text: line.clone(),
                turn: None,
            });
        }
        let children = self.listing.as_ref().map(|l| l.children.clone()).unwrap_or_default();
        // The section stands for the threads this one opened; its agent's subagents stand among them, never alone.
        if children.iter().any(|c| c.subagent.is_none()) {
            rows.push(Row {
                key: "threads".to_owned(),
                kind: RowKind::Threads { open: !shut, children },
                text: "Threads".to_owned(),
                turn: None,
            });
        }
        rows.extend(word);
        let working = rows.iter().any(|r| matches!(r.kind, RowKind::Working { .. }));
        if let Some(word) = self.listing.as_ref().and_then(|l| l.paused.as_ref()).filter(|_| !working) {
            rows.push(Row { key: PAUSED_KEY.to_owned(), kind: RowKind::Rule, text: word.clone(), turn: None });
        }
        if let Some(line) = self.listing.as_ref().and_then(|l| l.machine.as_ref()) {
            rows.push(Row { key: "machine".to_owned(), kind: RowKind::Rule, text: line.clone(), turn: None });
        }
        (rows, split)
    }
}

/// One subagent's page off the lead's transcript (`subagentEntries` in adapt/session.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub call: String,
    pub title: String,
    /// running, done, failed or stopped: the listing's word where it holds one, else the fold's.
    pub state: String,
    pub started: Option<f64>,
    pub ended: Option<f64>,
    pub failure: Option<String>,
    pub model: Option<String>,
    pub harness: Option<String>,
}

impl Transcript {
    /// The turns the agent wrote the slate in: a person's write and the host's draw no line (adapt/session.ts).
    fn slated(&self) -> HashSet<String> {
        self.events
            .iter()
            .filter_map(|e| match e {
                SessionEvent::SessionSlate { turn_id: Some(turn), by, .. } => {
                    (serde_json::to_value(by).ok()?.as_str() == Some("agent")).then(|| turn.clone())
                }
                _ => None,
            })
            .collect()
    }

    /// Shows one subagent's page in place of the thread's own rows, by the call that launched it; None shows the thread.
    pub fn page(&mut self, call: Option<String>) -> Option<Splice> {
        if self.around.page == call {
            return None;
        }
        self.around.page = call;
        self.refold_all()
    }

    /// What the page's bar and crumbs say: the subagent's title and state, its model and its agent; None where neither
    /// the transcript nor the listing knows the call (ChatView then opens the lead). The fold's own state and times
    /// come first, as they follow the run event by event where the listing is read only now and then.
    pub fn page_of(&self, call: &str) -> Option<Page> {
        let listed = self.around.listing.as_ref().and_then(|l| l.subagent(call));
        let sub = self.sub_of(call);
        if listed.is_none() && sub.is_none() {
            return None;
        }
        let fold_word = sub.map(|s| sub_word(s.state).to_owned());
        Some(Page {
            call: call.to_owned(),
            title: listed.map(|l| l.title.clone()).or_else(|| sub.map(|s| s.title.clone())).unwrap_or_default(),
            state: fold_word.or_else(|| listed.map(|l| l.state.clone())).unwrap_or_else(|| "running".to_owned()),
            started: sub.and_then(|s| s.started).or_else(|| listed.map(|l| l.started)),
            ended: sub.and_then(|s| s.ended).or_else(|| listed.and_then(|l| l.ended)),
            failure: listed.and_then(|l| l.failure.clone()),
            model: listed.and_then(|l| l.model.clone()),
            harness: self.around.listing.as_ref().and_then(|l| l.harness.clone()).or_else(|| self.harness()),
        })
    }

    fn sub_of(&self, call: &str) -> Option<&Sub> {
        self.model.entries.iter().find_map(|e| match &e.body {
            Body::Subagent(s) if s.parent == call => Some(&**s),
            _ => None,
        })
    }

    /// A subagent's own lines folded as a thread's are, so its calls group and open as the thread's own do: its
    /// deltas, out from under the call that launched it, in one turn of its own that is settled and open, then cut to
    /// its replies, notes and calls, with no turn fold, working row or stop line of its own. A run that said nothing
    /// itself, or was stopped, keeps the launching call's answer as its last line (`subagentEntries`).
    fn page_lines(&self, call: &str, sub: &Sub) -> Vec<Row> {
        let turn = format!("page:{call}");
        let mut events: Vec<SessionEvent> = Vec::new();
        let ids = self.events.first().and_then(|e| serde_json::to_value(e).ok()).unwrap_or_default();
        let id = |k: &str| ids.get(k).cloned().unwrap_or_else(|| serde_json::json!(""));
        let frame = |kind: &str, extra: serde_json::Value| {
            let mut v = serde_json::json!({
                "type": kind, "workspaceId": id("workspaceId"), "sessionId": id("sessionId"), "threadId": id("threadId"), "turnId": turn, "pos": 1,
            });
            if let (Some(o), Some(extra)) = (v.as_object_mut(), extra.as_object()) {
                o.extend(extra.clone());
            }
            serde_json::from_value::<SessionEvent>(v).ok()
        };
        events.extend(frame("session.start", serde_json::json!({})));
        for event in &self.events {
            let SessionEvent::SessionDelta { parent_tool_use_id: Some(parent), .. } = event else { continue };
            if parent != call {
                continue;
            }
            let Ok(mut v) = serde_json::to_value(event) else { continue };
            if let Some(o) = v.as_object_mut() {
                o.remove("parentToolUseId");
                o.insert("turnId".to_owned(), serde_json::json!(turn));
            }
            events.extend(serde_json::from_value::<SessionEvent>(v).ok());
        }
        let spoke = sub.lines.iter().any(|l| !l.mono && !l.label.is_empty() && !l.id.starts_with("answer:"));
        let answer =
            (sub.state == SubState::Stopped || !spoke).then(|| sub.lines.iter().rev().find(|l| l.id.starts_with("answer:"))).flatten();
        if let Some(line) = answer {
            events.extend(frame("session.delta", serde_json::json!({ "kind": "text", "text": line.label })));
        }
        events.extend(frame("session.done", serde_json::json!({ "result": { "status": "completed" } })));
        events.extend(frame("session.end", serde_json::json!({ "exitCode": 0, "sawResult": true })));
        let mut model = Model::default();
        for (ix, event) in events.iter().enumerate() {
            model.add(ix, event);
        }
        let mut ids = self.opened.clone();
        ids.insert(turn.clone());
        let opened = Opened { ids: &ids, shut: &self.shut, all: false, can: Can::default(), source: None, listing: None };
        let rows = derive(&model, 0, 0, opened).map(|(rows, _)| rows).unwrap_or_default();
        rows.into_iter()
            .filter(|r| {
                matches!(
                    r.kind,
                    RowKind::Agent { .. } | RowKind::Group { .. } | RowKind::Calls { .. } | RowKind::Work { .. } | RowKind::Live { .. }
                )
            })
            .map(|mut r| {
                if let RowKind::Agent { rewind, fork, .. } = &mut r.kind {
                    (*rewind, *fork) = (None, None);
                }
                r.turn = None;
                r
            })
            .collect()
    }

    /// The page's rows: what it was asked as the person's message, its text as replies, its calls as rows, and its
    /// prompts last; no turn of its own to fold, count or close.
    fn page_rows(&self, call: &str) -> Vec<Row> {
        let asked = self
            .events
            .iter()
            .find_map(|e| match e {
                SessionEvent::SessionDelta { tool_use_id: Some(id), text, .. } if id == call => {
                    tool_input(text).and_then(|f| tool_field(&f, "prompt").map(str::to_owned))
                }
                _ => None,
            })
            .or_else(|| self.around.listing.as_ref().and_then(|l| l.subagent(call)).and_then(|s| s.asked.clone()));
        let mut rows = Vec::new();
        if let Some(asked) = asked {
            let kind = RowKind::Person { at: None, files: Vec::new(), fork: None, full: false, steered: false };
            rows.push(Row { key: format!("{call}:asked"), kind, text: asked, turn: None });
        }
        let Some(sub) = self.sub_of(call) else { return rows };
        rows.extend(self.page_lines(call, sub));
        rows.extend(sub.prompts.iter().map(|p| prompt_row(&format!("permission:{}", p.ask), None, p, None)));
        rows
    }
}

#[cfg(test)]
impl Listing {
    /// A listing that holds every child `events` started and names a fork's source by its id, as a test's host would.
    pub(crate) fn holding(events: &[SessionEvent]) -> Listing {
        let mut t = Transcript::new();
        t.load(events.to_vec());
        let children = t
            .model
            .entries
            .iter()
            .filter_map(|e| match &e.body {
                Body::Work(w) => w.spawned.clone(),
                _ => None,
            })
            .map(|(thread, title)| Listed::thread(ThreadAt { workspace: "w".into(), thread }, &title, side::Kind::Working, ChildPart::Live))
            .collect();
        let subagents = t
            .model
            .kids
            .iter()
            .map(|(call, kid)| ListedSubagent {
                call: call.clone(),
                title: kid.title.clone(),
                state: sub_word(kid.state).to_owned(),
                model: None,
                asked: None,
                started: 0.,
                ended: None,
                failure: None,
            })
            .collect();
        let source = t.model.source.clone().map(|title| ForkSource { title, at: None });
        Listing { thread: "t".into(), source, children, subagents, ..Listing::default() }
    }
}

/// "Updated the slate" under the last reply of each turn the agent wrote the slate in, once however many writes it
/// made, after the files that turn changed (`slatedMessageIds` in ChatView.tsx, `SlateUpdatedLine`).
fn slate_lines(mut rows: Vec<Row>, mut split: Option<usize>, slated: &HashSet<String>) -> (Vec<Row>, Option<usize>) {
    if slated.is_empty() {
        return (rows, split);
    }
    let mut last: HashMap<&str, usize> = HashMap::new();
    for (ix, row) in rows.iter().enumerate() {
        if let (RowKind::Agent { .. }, Some(turn)) = (&row.kind, row.turn.as_deref()) {
            if slated.contains(turn) {
                last.insert(turn, ix);
            }
        }
    }
    let mut at: Vec<(usize, String)> = last
        .into_iter()
        .map(|(turn, ix)| {
            let after = match rows.get(ix + 1) {
                Some(Row { kind: RowKind::ChangedFiles { .. }, turn: t, .. }) if t.as_deref() == Some(turn) => ix + 2,
                _ => ix + 1,
            };
            (after, turn.to_owned())
        })
        .collect();
    at.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (ix, turn) in at {
        let row = Row { key: format!("slate:{turn}"), kind: RowKind::Slate, text: SLATE_UPDATED.to_owned(), turn: Some(turn) };
        rows.insert(ix, row);
        split = split.map(|s| if s >= ix { s + 1 } else { s });
    }
    (rows, split)
}

/// `SlateUpdatedLine`'s words.
pub const SLATE_UPDATED: &str = "Updated the slate";
