// SPDX-License-Identifier: AGPL-3.0-only
//! What the thread listing says about the open thread and the threads around it.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::screens::sidebar::model as side;
#[cfg(test)]
use wsp_host_client::protocol::SessionEvent;

#[cfg(test)]
use super::{model::sub_word, Body, Transcript};

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

/// A send in flight as its bubble draws it: its words and its files (`pendingPrompt` with its attachments' records).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sent {
    pub text: String,
    pub files: Vec<super::FileTile>,
    /// The send's request id where it is known, whose turn's start in the events takes the bubble's place.
    pub request: Option<String>,
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
    /// A send in flight, the person's message at once until its turn starts (`pendingPrompt`).
    pub sent: Option<Sent>,
    /// The thread's own row while it runs (`drawnOffRow`): when it began, which the running turn's Working line counts
    /// from where it is earlier (`turnFromRow`), and the head of its opening message, the person's message until the
    /// thread has an event (`runningOffRow`).
    pub running_since: Option<f64>,
    pub opening: Option<String>,
    /// Its machine is being made or waits for one (`spawning` in ChatView.tsx): the thread is not at work.
    pub spawning: bool,
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

/// The key of a send's bubble while its turn has not started, whose images the view holds off the composer's bytes.
pub const SENT_KEY: &str = "sent";

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
        rows: &[Arc<side::Row>],
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
        let rows: Vec<Arc<side::Row>> = rows.iter().filter(|r| tree.contains(r.thread_id.as_deref().unwrap_or(&r.id))).cloned().collect();
        let rows = rows.as_slice();
        let threads = side::fold_threads(rows);
        let mut order: Vec<usize> = (0..threads.len()).collect();
        let start = |i: usize| threads[i].started_at.unwrap_or(0.);
        order.sort_by(|&a, &b| start(b).total_cmp(&start(a)).then_with(|| threads[a].key.cmp(&threads[b].key)));
        let forest = side::forest(&threads, &order);
        let by_id: HashMap<&str, &side::Row> = rows.iter().map(|r| (r.id.as_str(), &**r)).collect();
        let tree = Reading { cx: side::Ctx { all: &threads, now: now_ms, settle: settle_ms }, rows: by_id, lead_computer: computer };
        let children = find_node(&forest, thread).map(|node| tree.children(node)).unwrap_or_default();
        let (strip, start_failed) = own.map(|own| sub_of(own, here, now_ms)).unwrap_or_default();
        let running = own.filter(|r| r.status == side::Status::Running);
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
            running_since: running.and_then(|r| r.started_at),
            opening: running.and_then(|r| r.prompt.clone()),
            spawning: own
                .and_then(|r| r.sub.as_ref())
                .is_some_and(|s| matches!(s.state, side::SubState::Starting | side::SubState::Waiting)),
            wait: None,
            paused: None,
        }
    }

    /// The same off the rows as JSON, at the record's default settle and with nothing shut, for a test.
    #[cfg(test)]
    pub fn of(rows: &[serde_json::Value], thread: &str, now_ms: f64) -> Listing {
        let rows: Vec<Arc<side::Row>> = rows.iter().cloned().filter_map(side::Row::owned).map(Arc::new).collect();
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
