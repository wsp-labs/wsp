// SPDX-License-Identifier: AGPL-3.0-only
//! What Activity draws from one computer's process frame, after `activity/model.ts`: wsp's own parts, each thread
//! with what it started, what wsp started with no thread known, and the rest of the computer. Sort, filter, the Kinds
//! and Ports views and the tints are pure functions of that, so the page draws and the tests read one rule.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use super::place::{activity_name, agent_bin, agent_by_bin, agent_name, app_name, place_procs, PlaceInput, PlaceThread, Placement, Whose};
use super::wire::{AppKind, AppPart, Container, Kind, KnownPort, Pid, PlacedBy, ProcEntry, Snapshot};

/// The page's words (`ACTIVITY_WORDS` and `ACTIVITY_GROUP_WORDS`).
pub mod words {
    pub const FILTER: &str = "Filter by name, port or thread";
    pub const EVERY: &str = "Every process";
    pub const MEMORY: &str = "Memory";
    pub const CPU: &str = "CPU";
    pub const ENERGY: &str = "Power";
    pub const PID: &str = "PID";
    pub const THREAD: &str = "Thread";
    pub const PORT: &str = "Port";
    pub const PROCESS: &str = "Process";
    pub const REACH: &str = "Who can connect";
    pub const PROCESSES: &str = "Processes";
    pub const WSP: &str = "wsp";
    pub const WSP_NOTE: &str = "The app and what runs it";
    pub const WSP_NOTE_BOX: &str = "What runs wsp here";
    pub const LOOSE: &str = "No thread";
    pub const LOOSE_NOTE: &str = "Started with no thread wsp knows of";
    pub const SAMPLER: &str = "Task manager";
    pub const SAMPLER_NOTE: &str = "What reading this page costs";
    pub const END: &str = "End";
    pub const END_ALL: &str = "End all";
    pub const COPY_COMMAND: &str = "Copy command";
    pub const COPY_PID: &str = "Copy PID";
    pub const ENDING: &str = "Ending";
    pub const CONTAINER: &str = "Container";
    pub const END_CONTAINER: &str = "Stop the container, its data kept";
    pub const COPY_NAME: &str = "Copy name";
    pub const COPY_CONTAINER_ID: &str = "Copy container ID";
    pub const NOTE_LABEL: &str = "Its thread's docker started it";
    pub const NOTE_COMPOSE: &str = "Its compose project was last started from this thread";
    pub const NOTE_LOOSE: &str = "No thread started it, so wsp leaves it to whoever did";
    pub const NOTE_GONE: &str = "Its label names a thread wsp no longer knows";
    pub const NOTE_GONE_COMPOSE: &str = "Its compose project was last started from a thread wsp no longer knows";
    pub const SUBAGENT: &str = "Subagent";
    pub const SUBAGENT_NOTE: &str = "Runs inside its agent, whose row holds its memory and CPU";
    pub const NEEDED: &str = "wsp needs this to run";
    pub const SHARED: &str = "Shared by the app and its tabs";
    pub const LOOPBACK: &str = "Local only";
    pub const EVERY_ADDRESS: &str = "Open to the network";
    pub const ONE_CORE: &str = "100% is one core";
    pub const ENERGY_NOTE: &str = "CPU power, last 2 s";
    pub const NOTHING: &str = "No thread is running anything.";
    pub const NOTHING_MATCHES: &str = "Nothing matches.";
    pub const PENDING: &str = "Reading this computer's processes.";
    pub const ALL_KINDS: &str = "All kinds";
    pub const ALL_THREADS: &str = "All threads";
    pub const STOP_TURN: &str = "Stop turn";
    pub const END_ALL_PRESS: &str = "End all";
    pub const CANCEL: &str = "Cancel";

    pub fn views(view: super::View) -> &'static str {
        match view {
            super::View::Threads => "Threads",
            super::View::Processes => "Processes",
            super::View::Kinds => "Kinds",
            super::View::Ports => "Ports",
        }
    }

    pub fn slate(name: &str) -> String {
        if name.is_empty() {
            "Slate".into()
        } else {
            format!("Slate: {name}")
        }
    }

    pub fn up(time: &str) -> String {
        format!("Up {time}")
    }

    pub fn tokens(n: &str) -> String {
        format!("{n} tokens")
    }

    pub fn of(used: &str) -> String {
        format!("of {used} in use")
    }

    pub fn threads(n: usize) -> String {
        if n == 1 {
            "1 thread".into()
        } else {
            format!("{n} threads")
        }
    }

    pub fn process_count(n: usize) -> String {
        if n == 1 {
            "1 process".into()
        } else {
            format!("{n} processes")
        }
    }

    pub fn stop_title(title: &str) -> String {
        format!("Stop the turn in {title}?")
    }

    /// On a box a stop ends the thread's cgroup whole; here it ends the agent alone.
    pub fn stop_sentence(agent: &str, ends_all: bool) -> String {
        let rest = if ends_all {
            "Everything else the thread started on that computer ends with it, its dev and tool servers included."
        } else {
            "The dev server and tool servers it started keep running."
        };
        format!("{agent} stops where it is and the turn reads Interrupted. {rest}")
    }

    pub fn end_all_title(slate: &str) -> String {
        format!("End everything {} runs?", if slate.is_empty() { "the slate" } else { slate })
    }

    pub fn end_all_sentence(procs: usize, memory: &str) -> String {
        format!("{} holding {memory} end, and what each started.", process_count(procs))
    }

    pub fn subagent_state(state: super::SubState) -> &'static str {
        match state {
            super::SubState::Running => "Running",
            super::SubState::Done => "Done",
            super::SubState::Failed => "Failed",
            super::SubState::Stopped => "Stopped",
        }
    }
}

/// The page's four views, the top bar's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Threads,
    Processes,
    Kinds,
    Ports,
}

impl View {
    pub const ALL: [View; 4] = [View::Threads, View::Processes, View::Kinds, View::Ports];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubState {
    Running,
    Done,
    Failed,
    Stopped,
}

/// A subagent as a thread's listing carries it (`SubagentView`).
#[derive(Clone, Debug, PartialEq)]
pub struct SubagentIn {
    pub id: String,
    pub title: String,
    pub state: SubState,
    pub started_at: f64,
    pub ended_at: Option<f64>,
    /// Its own calls' input and output tokens, where its agent named them.
    pub tokens: Option<f64>,
    pub duration_ms: Option<f64>,
    pub model: Option<String>,
}

/// A thread on the computer the page reads (`ActivityThread`).
#[derive(Clone, Debug, PartialEq)]
pub struct Thread {
    pub id: String,
    pub title: String,
    pub harness: String,
    pub project: String,
    pub workspace_id: String,
    pub session_id: String,
    /// A turn runs on it now: its agent's act is the thread's stop.
    pub working: bool,
    pub pid: Option<Pid>,
    pub agent_pid: Option<Pid>,
    pub cgroup: Option<String>,
    pub parent: Option<String>,
    pub slate: Option<String>,
    pub subagents: Vec<SubagentIn>,
}

/// One of a thread's subagents as drawn (`ActivitySubagent`).
#[derive(Clone, Debug, PartialEq)]
pub struct Subagent {
    pub id: String,
    pub title: String,
    pub state: SubState,
    pub ms: f64,
    pub tokens: Option<f64>,
    pub model: Option<String>,
}

/// How long a subagent that ended stays drawn.
pub const SUBAGENT_SHOWN_MS: f64 = 60_000.;

/// `subagentsOf`: those running and those that ended in the last minute.
pub fn subagents_of(subagents: &[SubagentIn], now: f64) -> Vec<Subagent> {
    subagents
        .iter()
        .filter_map(|s| {
            let end = if s.state == SubState::Running { Some(now) } else { s.ended_at };
            let end = end.filter(|end| now - end <= SUBAGENT_SHOWN_MS)?;
            let ms = if s.state == SubState::Running { end - s.started_at } else { s.duration_ms.unwrap_or(end - s.started_at) };
            Some(Subagent {
                id: s.id.clone(),
                title: s.title.clone(),
                state: s.state,
                ms: ms.max(0.),
                tokens: s.tokens,
                model: s.model.clone(),
            })
        })
        .collect()
}

/// One port a process listens on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Port {
    pub port: i64,
    pub loopback: Option<bool>,
}

/// One process row (`ActivityProc`).
#[derive(Clone, Debug, PartialEq)]
pub struct Proc {
    pub pid: Pid,
    pub name: String,
    pub kind: Kind,
    pub mem: f64,
    pub cpu: f64,
    /// Milliwatts over the interval; none where the computer keeps no energy counter.
    pub energy: Option<f64>,
    pub ports: Vec<Port>,
    pub command: String,
    pub started_at: Option<f64>,
    pub id: Option<String>,
    pub harness: Option<String>,
    pub needed: bool,
    pub endable: bool,
    /// A heading over a slate's runs, no process itself.
    pub heading: bool,
    pub note: Option<&'static str>,
    pub children: Vec<Proc>,
}

/// One running container under its group (`ActivityContainer`).
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerRow {
    pub id: String,
    pub name: String,
    pub image: String,
    pub up_ms: Option<f64>,
    pub mem: Option<f64>,
    pub cpu: Option<f64>,
    pub folder: Option<String>,
    pub placed_by: Option<PlacedBy>,
    pub owned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    Wsp,
    Thread,
    Loose,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub id: String,
    pub kind: GroupKind,
    pub thread: Option<Thread>,
    pub procs: Vec<Proc>,
    pub subagents: Vec<Subagent>,
    pub containers: Vec<ContainerRow>,
    /// A box thread's processes' memory as its cgroup counts it.
    pub memory: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Scope {
    pub groups: Vec<Group>,
    pub rest: Vec<Proc>,
    pub energy: bool,
}

pub struct ScopeInput<'a> {
    pub snapshot: &'a Snapshot,
    pub threads: &'a [Thread],
    pub ports: &'a HashMap<Pid, Vec<Port>>,
    pub host: Option<Pid>,
    pub app: &'a HashMap<Pid, AppPart>,
    pub now: f64,
}

/// Listening ports by the pid holding them, off what the daemon's ports watch knows.
pub fn ports_by_pid(known: &[KnownPort]) -> HashMap<Pid, Vec<Port>> {
    let mut by_pid: HashMap<Pid, Vec<Port>> = HashMap::new();
    for k in known {
        if let Some(pid) = k.pid {
            by_pid.entry(pid).or_default().push(Port { port: k.port, loopback: k.loopback });
        }
    }
    by_pid
}

/// `foldTree`'s node: a row, the rows it holds, and its children.
struct Folded<'a> {
    p: &'a ProcEntry,
    held: Vec<&'a ProcEntry>,
    kids: Vec<Folded<'a>>,
}

/// `foldTree`: the list's tree, in the list's order; a row folds into the one `folds_into` names.
fn fold_tree<'a>(list: &[&'a ProcEntry], folds_into: impl Fn(Pid) -> Option<Pid>) -> Vec<Folded<'a>> {
    let by_pid: HashMap<Pid, &ProcEntry> = list.iter().map(|p| (p.pid, *p)).collect();
    let into = |pid: Pid| folds_into(pid).filter(|at| *at != pid && by_pid.contains_key(at));
    let up = |p: &ProcEntry| (p.ppid != p.pid).then(|| by_pid.get(&p.ppid).copied()).flatten();
    let parent_of = |p: &ProcEntry| {
        let mut at = up(p);
        let mut n = 0;
        while let Some(q) = at.filter(|_| n <= list.len()) {
            match into(q.pid) {
                None => return Some(q.pid),
                Some(target) if target != p.pid => return Some(target),
                Some(_) => at = up(q),
            }
            n += 1;
        }
        None
    };
    let mut kids: HashMap<Pid, Vec<&ProcEntry>> = HashMap::new();
    let mut held: HashMap<Pid, Vec<&ProcEntry>> = HashMap::new();
    let mut heads: Vec<&ProcEntry> = Vec::new();
    for p in list {
        match into(p.pid) {
            Some(target) => held.entry(target).or_default().push(p),
            None => match parent_of(p) {
                None => heads.push(p),
                Some(parent) => kids.entry(parent).or_default().push(p),
            },
        }
    }
    fn build<'a>(
        p: &'a ProcEntry,
        kids: &HashMap<Pid, Vec<&'a ProcEntry>>,
        held: &HashMap<Pid, Vec<&'a ProcEntry>>,
        depth: usize,
    ) -> Folded<'a> {
        let under = if depth > 512 {
            Vec::new()
        } else {
            kids.get(&p.pid).map(|k| k.iter().map(|q| build(q, kids, held, depth + 1)).collect()).unwrap_or_default()
        };
        Folded { p, held: held.get(&p.pid).cloned().unwrap_or_default(), kids: under }
    }
    heads.into_iter().map(|p| build(p, &kids, &held, 0)).collect()
}

/// `scopeOf`: the computer's frame read into groups.
pub fn scope_of(input: ScopeInput) -> Scope {
    let ScopeInput { snapshot, threads, ports, host, app, now } = input;
    let thread_by_id: HashMap<&str, &Thread> = threads.iter().map(|t| (t.id.as_str(), t)).collect();
    let place_threads: Vec<PlaceThread> = threads
        .iter()
        .map(|t| PlaceThread {
            id: t.id.clone(),
            roots: t.pid.into_iter().collect(),
            cgroup: t.cgroup.clone(),
            bin: agent_bin(&t.harness).map(str::to_owned),
            agents: t.agent_pid.into_iter().collect(),
        })
        .collect();
    let listens = |pid: Pid| ports.get(&pid).is_some_and(|p| !p.is_empty());
    let placed = place_procs(PlaceInput {
        procs: &snapshot.procs,
        daemon: snapshot.daemon,
        host,
        threads: &place_threads,
        listens: &listens,
        app: app.iter().map(|(pid, part)| (*pid, matches!(part.kind, AppKind::Tab | AppKind::Frame))).collect(),
    });
    let key_of = |pid: Pid| match placed.whose.get(&pid) {
        None => String::new(),
        Some(Whose::Thread(t)) => format!("t:{t}"),
        Some(Whose::Wsp) => "wsp".into(),
        Some(Whose::Loose) => "loose".into(),
    };
    let mut members: HashMap<String, Vec<&ProcEntry>> = HashMap::new();
    let mut rest: Vec<&ProcEntry> = Vec::new();
    for p in &placed.procs {
        let key = key_of(p.pid);
        if key.is_empty() {
            rest.push(p);
        } else {
            members.entry(key).or_default().push(p);
        }
    }
    let energy = placed.procs.iter().any(|p| p.energy_mw.is_some());
    let reader = Reader { placed: &placed, thread_by_id: &thread_by_id, app, ports, daemon: snapshot.daemon, host, energy };

    let mut containers: HashMap<String, Vec<ContainerRow>> = HashMap::new();
    for c in snapshot.containers.iter().flatten() {
        let at = match &c.thread {
            Some(t) if thread_by_id.contains_key(t.as_str()) => format!("t:{t}"),
            _ => "loose".to_owned(),
        };
        let owned = at != "loose";
        containers.entry(at).or_default().push(container_row(c, now, owned));
    }

    let mut groups: Vec<Group> = Vec::new();
    if let Some(wsp) = members.get("wsp") {
        groups.push(Group {
            id: "wsp".into(),
            kind: GroupKind::Wsp,
            thread: None,
            procs: reader.forest(wsp),
            subagents: vec![],
            containers: vec![],
            memory: None,
        });
    }
    for t in threads {
        let mine = members.get(&format!("t:{}", t.id));
        let subagents = subagents_of(&t.subagents, now);
        let held = containers.remove(&format!("t:{}", t.id)).unwrap_or_default();
        let memory = t.cgroup.as_ref().and_then(|cg| snapshot.cgroups.as_ref()?.iter().find(|(c, _)| c == cg).map(|(_, m)| *m));
        if mine.is_some() || !subagents.is_empty() || !held.is_empty() {
            let procs = mine.map(|m| with_slate(reader.forest(m), t)).unwrap_or_default();
            groups.push(Group {
                id: t.id.clone(),
                kind: GroupKind::Thread,
                thread: Some(t.clone()),
                procs,
                subagents,
                containers: held,
                memory,
            });
        }
    }
    let loose = members.get("loose");
    let loose_containers = containers.remove("loose").unwrap_or_default();
    if loose.is_some() || !loose_containers.is_empty() {
        let procs = loose.map(|l| reader.forest(l)).unwrap_or_default();
        groups.push(Group {
            id: "loose".into(),
            kind: GroupKind::Loose,
            thread: None,
            procs,
            subagents: vec![],
            containers: loose_containers,
            memory: None,
        });
    }
    Scope { groups, rest: rest.iter().map(|p| reader.leaf(p, vec![])).collect(), energy }
}

struct Reader<'a, 'b> {
    placed: &'b Placement<'a>,
    thread_by_id: &'b HashMap<&'b str, &'b Thread>,
    app: &'b HashMap<Pid, AppPart>,
    ports: &'b HashMap<Pid, Vec<Port>>,
    daemon: Pid,
    host: Option<Pid>,
    energy: bool,
}

impl Reader<'_, '_> {
    fn leaf(&self, p: &ProcEntry, children: Vec<Proc>) -> Proc {
        let kind = self.placed.kind_of(p.pid);
        let thread = self.placed.agent_of(p.pid).and_then(|t| self.thread_by_id.get(t));
        let part = self.app.get(&p.pid);
        let agent = match thread {
            Some(t) => Some(agent_name(&t.harness)),
            None if kind == Kind::Agent => agent_by_bin(p).map(agent_name),
            None => None,
        };
        let name = match part {
            Some(part) => app_name(part),
            None => activity_name(p, self.daemon, self.host, agent.as_deref()),
        };
        Proc {
            pid: p.pid,
            name,
            kind,
            mem: p.rss,
            cpu: p.cpu,
            energy: p.energy_mw.or(self.energy.then_some(0.)),
            ports: self.ports.get(&p.pid).cloned().unwrap_or_default(),
            command: if p.cmdline.is_empty() { format!("[{}]", p.comm) } else { p.cmdline.clone() },
            started_at: Some(p.started_at),
            id: p.id.clone(),
            harness: thread.map(|t| t.harness.clone()),
            needed: self.placed.needed(p.pid),
            endable: self.placed.endable(p.pid),
            heading: false,
            note: part.filter(|part| matches!(part.kind, AppKind::Gpu | AppKind::Network)).map(|_| words::SHARED),
            children,
        }
    }

    /// A group's trees; each process an agent's row holds is no row of its own, its figures join the agent's.
    fn forest(&self, list: &[&ProcEntry]) -> Vec<Proc> {
        fold_tree(list, |pid| self.placed.folds_into(pid)).into_iter().map(|f| self.build(f)).collect()
    }

    fn build(&self, f: Folded) -> Proc {
        let kids = f.kids.into_iter().map(|k| self.build(k)).collect();
        let mut row = self.leaf(f.p, kids);
        if f.held.is_empty() {
            return row;
        }
        let energies: Vec<f64> =
            std::iter::once(row.energy).chain(f.held.iter().map(|q| q.energy_mw.or(self.energy.then_some(0.)))).flatten().collect();
        row.mem += f.held.iter().map(|q| q.rss).sum::<f64>();
        row.cpu += f.held.iter().map(|q| q.cpu).sum::<f64>();
        row.energy = (!energies.is_empty()).then(|| energies.iter().sum());
        row
    }
}

/// A thread's slate runs and the tool servers it keeps under one heading beside its agent.
fn with_slate(heads: Vec<Proc>, t: &Thread) -> Vec<Proc> {
    let slated = |p: &Proc| matches!(p.kind, Kind::Slate | Kind::Tool);
    let (runs, mut rest): (Vec<Proc>, Vec<Proc>) = heads.into_iter().partition(slated);
    if runs.is_empty() {
        return rest;
    }
    let sum = spent(&runs);
    rest.push(Proc {
        pid: -1,
        name: t.slate.clone().unwrap_or_default(),
        kind: Kind::Slate,
        mem: sum.mem,
        cpu: sum.cpu,
        energy: sum.energy,
        ports: vec![],
        command: String::new(),
        started_at: None,
        id: None,
        harness: None,
        needed: false,
        endable: false,
        heading: true,
        note: None,
        children: runs,
    });
    rest
}

fn container_row(c: &Container, now: f64, owned: bool) -> ContainerRow {
    ContainerRow {
        id: c.id.clone(),
        name: c.name.clone(),
        image: c.image.clone(),
        up_ms: c.started_at.map(|s| (now - s).max(0.)),
        mem: c.memory,
        cpu: c.cpu,
        folder: c.folder.clone(),
        placed_by: c.placed_by,
        owned,
    }
}

/// `flat`: every process under a row, the row included; a heading is no process.
pub fn flat(p: &Proc) -> Vec<&Proc> {
    let mut out = if p.heading { vec![] } else { vec![p] };
    for c in &p.children {
        out.extend(flat(c));
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Spent {
    pub mem: f64,
    pub cpu: f64,
    pub energy: Option<f64>,
    pub count: usize,
}

fn sum_energy(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    values.flatten().fold(None, |acc, e| Some(acc.unwrap_or(0.) + e))
}

pub fn spent(procs: &[Proc]) -> Spent {
    let all: Vec<&Proc> = procs.iter().flat_map(flat).collect();
    Spent {
        mem: all.iter().map(|p| p.mem).sum(),
        cpu: all.iter().map(|p| p.cpu).sum(),
        energy: sum_energy(all.iter().map(|p| p.energy)),
        count: all.len(),
    }
}

/// `groupSpent`: a group's processes' and its containers' figures; a box thread's memory is its cgroup's.
pub fn group_spent(g: &Group) -> Spent {
    let p = spent(&g.procs);
    Spent {
        mem: g.containers.iter().map(|c| c.mem.unwrap_or(0.)).sum::<f64>() + g.memory.unwrap_or(p.mem),
        cpu: g.containers.iter().map(|c| c.cpu.unwrap_or(0.)).sum::<f64>() + p.cpu,
        ..p
    }
}

pub fn total_spent<'a>(groups: impl IntoIterator<Item = &'a Group>) -> Spent {
    let each: Vec<Spent> = groups.into_iter().map(group_spent).collect();
    Spent {
        mem: each.iter().map(|s| s.mem).sum(),
        cpu: each.iter().map(|s| s.cpu).sum(),
        energy: sum_energy(each.iter().map(|s| s.energy)),
        count: each.iter().map(|s| s.count).sum(),
    }
}

/// Which column the rows are heaviest first by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Mem,
    Cpu,
    Energy,
}

fn figure(sort: Sort, mem: f64, cpu: f64, energy: Option<f64>) -> f64 {
    match sort {
        Sort::Mem => mem,
        Sort::Cpu => cpu,
        Sort::Energy => energy.unwrap_or(0.),
    }
}

fn heavier(a: f64, b: f64) -> Ordering {
    b.partial_cmp(&a).unwrap_or(Ordering::Equal)
}

/// `sortProcs`: heaviest first on the column, ties by pid.
pub fn sort_procs(procs: &mut [Proc], sort: Sort) {
    procs.sort_by(|a, b| heavier(figure(sort, a.mem, a.cpu, a.energy), figure(sort, b.mem, b.cpu, b.energy)).then(a.pid.cmp(&b.pid)));
    for p in procs {
        sort_procs(&mut p.children, sort);
    }
}

fn sort_containers(containers: &mut [ContainerRow], sort: Sort) {
    let of = |c: &ContainerRow| match sort {
        Sort::Mem => c.mem.unwrap_or(0.),
        Sort::Cpu => c.cpu.unwrap_or(0.),
        Sort::Energy => 0.,
    };
    containers.sort_by(|a, b| heavier(of(a), of(b)).then_with(|| a.name.cmp(&b.name)));
}

/// `nestGroups`: each thread group under the lead that started it, where the lead has a group too.
pub fn nest_groups(groups: &[Group]) -> (Vec<&Group>, HashMap<String, Vec<&Group>>) {
    let shown: HashSet<&str> = groups.iter().map(|g| g.id.as_str()).collect();
    let lead_of =
        |g: &Group| g.thread.as_ref().and_then(|t| t.parent.clone()).filter(|lead| shown.contains(lead.as_str()) && *lead != g.id);
    let mut tops = Vec::new();
    let mut kids: HashMap<String, Vec<&Group>> = HashMap::new();
    for g in groups {
        let lead = lead_of(g);
        let mut seen: HashSet<String> = HashSet::from([g.id.clone()]);
        let mut up = lead.clone();
        while let Some(at) = up.clone().filter(|at| !seen.contains(at)) {
            seen.insert(at.clone());
            up = groups.iter().find(|x| x.id == at).and_then(lead_of);
        }
        match lead {
            Some(lead) if up.is_none() => kids.entry(lead).or_default().push(g),
            _ => tops.push(g),
        }
    }
    (tops, kids)
}

/// `groupsUnder`: every group folded under a group, at every depth.
pub fn groups_under<'a>(kids: &HashMap<String, Vec<&'a Group>>, id: &str) -> Vec<&'a Group> {
    kids.get(id).map(|k| k.iter().flat_map(|g| std::iter::once(*g).chain(groups_under(kids, &g.id))).collect()).unwrap_or_default()
}

/// `sortGroups`: wsp's parts first and No thread last; the threads between heaviest first by their heads' totals.
pub fn sort_groups(groups: Vec<Group>, sort: Sort) -> Vec<Group> {
    let rank = |g: &Group| match g.kind {
        GroupKind::Wsp => 0,
        GroupKind::Thread => 1,
        GroupKind::Loose => 2,
    };
    let totals: Vec<f64> = {
        let (_, kids) = nest_groups(&groups);
        groups
            .iter()
            .map(|g| {
                let t = total_spent(std::iter::once(g).chain(groups_under(&kids, &g.id)));
                figure(sort, t.mem, t.cpu, t.energy)
            })
            .collect()
    };
    let mut ranked: Vec<(f64, Group)> = totals.into_iter().zip(groups).collect();
    for (_, g) in &mut ranked {
        sort_procs(&mut g.procs, sort);
        sort_containers(&mut g.containers, sort);
    }
    ranked.sort_by(|(ta, a), (tb, b)| rank(a).cmp(&rank(b)).then(heavier(*ta, *tb)).then_with(|| a.id.cmp(&b.id)));
    ranked.into_iter().map(|(_, g)| g).collect()
}

fn container_hit(c: &ContainerRow, needle: &str) -> bool {
    [c.name.as_str(), c.image.as_str(), c.folder.as_deref().unwrap_or(""), words::CONTAINER]
        .iter()
        .any(|s| s.to_lowercase().contains(needle))
}

fn proc_hit(p: &Proc, needle: &str) -> bool {
    [p.name.as_str(), p.command.as_str(), p.kind.word()].iter().any(|s| s.to_lowercase().contains(needle))
        || p.pid.to_string().contains(needle)
        || p.ports.iter().any(|x| x.port.to_string().contains(needle))
}

/// A tree cut to what matches and the parents above each match.
fn prune(procs: &[Proc], keep: &dyn Fn(&Proc) -> bool) -> Vec<Proc> {
    procs
        .iter()
        .filter_map(|p| {
            let children = prune(&p.children, keep);
            (keep(p) || !children.is_empty()).then(|| Proc { children, ..p.clone() })
        })
        .collect()
}

/// The two selects' picks: a thread's id and a kind, or every one.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Picks {
    pub thread: Option<String>,
    pub kind: Option<Kind>,
}

/// `filterGroups`: the groups a filter and the two selects leave.
pub fn filter_groups(groups: &[Group], text: &str, picks: &Picks) -> Vec<Group> {
    let needle = text.trim().to_lowercase();
    groups
        .iter()
        .filter(|g| picks.thread.as_ref().is_none_or(|t| *t == g.id))
        .filter_map(|g| {
            let mut procs = g.procs.clone();
            let mut subagents = if picks.kind.is_some() { vec![] } else { g.subagents.clone() };
            let mut containers = if picks.kind.is_some() { vec![] } else { g.containers.clone() };
            if let Some(kind) = picks.kind {
                procs = prune(&procs, &|p| p.kind == kind && !p.heading);
            }
            let titled = !needle.is_empty() && g.thread.as_ref().is_some_and(|t| t.title.to_lowercase().contains(&needle));
            if !needle.is_empty() && !titled {
                procs = prune(&procs, &|p| proc_hit(p, &needle));
                subagents.retain(|s| s.title.to_lowercase().contains(&needle));
                containers.retain(|c| container_hit(c, &needle));
            }
            (!procs.is_empty() || !subagents.is_empty() || !containers.is_empty() || (titled && picks.kind.is_none())).then(|| Group {
                procs,
                subagents,
                containers,
                ..g.clone()
            })
        })
        .collect()
}

/// `pickChoices`: each kind wsp's processes here run as, and each thread with a group.
pub fn pick_choices(groups: &[Group], picks: &Picks) -> (Vec<Kind>, Vec<(String, String)>) {
    let mut running: HashSet<Kind> = groups.iter().flat_map(|g| g.procs.iter().flat_map(flat)).map(|p| p.kind).collect();
    running.extend(picks.kind);
    let threads = groups.iter().filter_map(|g| g.thread.as_ref().map(|t| (g.id.clone(), t.title.clone()))).collect();
    (Kind::ALL.into_iter().filter(|k| running.contains(k)).collect(), threads)
}

/// One row of the flat views, with the group it came from (`PlacedProc`).
#[derive(Clone, Debug)]
pub struct Placed<'a> {
    pub p: &'a Proc,
    pub group: Option<&'a Group>,
}

/// `flatRows`: every owned process flat, and with `every` the rest of the computer after them, heaviest first.
pub fn flat_rows<'a>(groups: &'a [Group], rest: &'a [Proc], every: bool, sort: Sort, text: &str) -> Vec<Placed<'a>> {
    let needle = text.trim().to_lowercase();
    let mut rows: Vec<Placed> =
        groups.iter().flat_map(|g| g.procs.iter().flat_map(flat).map(move |p| Placed { p, group: Some(g) })).collect();
    if every {
        rows.extend(rest.iter().filter(|p| needle.is_empty() || proc_hit(p, &needle)).map(|p| Placed { p, group: None }));
    }
    rows.sort_by(|a, b| {
        heavier(figure(sort, a.p.mem, a.p.cpu, a.p.energy), figure(sort, b.p.mem, b.p.cpu, b.p.energy)).then(a.p.pid.cmp(&b.p.pid))
    });
    rows
}

/// `kindGroups`: one group per kind, each holding its processes flat with the group they came from.
pub fn kind_groups(groups: &[Group], sort: Sort) -> Vec<(Kind, Vec<Placed<'_>>)> {
    let rows: Vec<Placed> = groups.iter().flat_map(|g| g.procs.iter().flat_map(flat).map(move |p| Placed { p, group: Some(g) })).collect();
    Kind::ALL
        .into_iter()
        .filter_map(|kind| {
            let mut mine: Vec<Placed> = rows.iter().filter(|r| r.p.kind == kind).cloned().collect();
            mine.sort_by(|a, b| {
                heavier(figure(sort, a.p.mem, a.p.cpu, a.p.energy), figure(sort, b.p.mem, b.p.cpu, b.p.energy)).then(a.p.pid.cmp(&b.p.pid))
            });
            (!mine.is_empty()).then_some((kind, mine))
        })
        .collect()
}

/// `portRows`: every port an owned process listens on, lowest first.
pub fn port_rows(groups: &[Group]) -> Vec<(Port, &Proc, &Group)> {
    let mut rows: Vec<(Port, &Proc, &Group)> = groups
        .iter()
        .flat_map(|g| g.procs.iter().flat_map(flat).flat_map(move |p| p.ports.iter().map(move |port| (*port, p, g))))
        .collect();
    rows.sort_by(|a, b| a.0.port.cmp(&b.0.port).then(a.1.pid.cmp(&b.1.pid)));
    rows
}

/// A figure is inked only when it means something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Warn,
    Danger,
}

const GIB: f64 = 1024. * 1024. * 1024.;
pub const MEMORY_HIGH: f64 = 2. * GIB;
pub const MEMORY_VERY_HIGH: f64 = 8. * GIB;
pub const MEMORY_SHARE_HIGH: f64 = 0.5;
pub const MEMORY_SHARE_VERY_HIGH: f64 = 0.75;
pub const CPU_HIGH: f64 = 100.;
pub const CPU_VERY_HIGH: f64 = 400.;
pub const ENERGY_HIGH_MW: f64 = 1000.;
pub const ENERGY_VERY_HIGH_MW: f64 = 3000.;

fn past(value: f64, high: f64, very_high: f64) -> Option<Tint> {
    if value > very_high {
        Some(Tint::Danger)
    } else if value > high {
        Some(Tint::Warn)
    } else {
        None
    }
}

pub fn memory_tint(bytes: f64) -> Option<Tint> {
    past(bytes, MEMORY_HIGH, MEMORY_VERY_HIGH)
}

pub fn memory_share_tint(bytes: f64, total: Option<f64>) -> Option<Tint> {
    total.filter(|t| *t > 0.).and_then(|t| past(bytes / t, MEMORY_SHARE_HIGH, MEMORY_SHARE_VERY_HIGH))
}

pub fn cpu_tint(percent: f64) -> Option<Tint> {
    past(percent, CPU_HIGH, CPU_VERY_HIGH)
}

pub fn energy_tint(mw: Option<f64>) -> Option<Tint> {
    mw.and_then(|mw| past(mw, ENERGY_HIGH_MW, ENERGY_VERY_HIGH_MW))
}

/// Five minutes of frames at the sampler's two seconds.
pub const HISTORY_POINTS: usize = 150;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HistoryPoint {
    pub at: f64,
    pub mem: f64,
    pub cpu: f64,
    pub energy: Option<f64>,
}

/// `pushHistory`: a frame's totals added, the oldest dropped past the cap; a frame already held changes nothing.
pub fn push_history(history: &mut Vec<HistoryPoint>, point: HistoryPoint) {
    if history.last().is_some_and(|h| h.at == point.at) {
        return;
    }
    history.push(point);
    if history.len() > HISTORY_POINTS {
        history.drain(..history.len() - HISTORY_POINTS);
    }
}

/// Percent of one core, one decimal.
pub fn cpu_word(cpu: f64) -> String {
    format!("{cpu:.1}%")
}

/// Milliwatts, watts with one decimal from 1,000.
pub fn energy_word(mw: Option<f64>) -> String {
    match mw {
        None => String::new(),
        Some(mw) if mw >= 1000. => format!("{:.1} W", mw / 1000.),
        Some(mw) if mw > 0. && mw < 1. => format!("{mw:.1} mW"),
        Some(mw) => format!("{} mW", mw.round()),
    }
}

pub fn ports_word(ports: &[Port]) -> String {
    match ports {
        [] => String::new(),
        [one] => format!(":{}", one.port),
        [first, rest @ ..] => format!(":{} +{}", first.port, rest.len()),
    }
}

/// `fmtBytes`: binary units, whole under a gigabyte, GB with one decimal unless whole.
pub fn fmt_bytes(n: f64) -> String {
    const K: f64 = 1024.;
    if n < K {
        return format!("{} B", n.round());
    }
    if n < K * K {
        return format!("{} KB", (n / K).round());
    }
    if n < K * K * K {
        return format!("{} MB", (n / K / K).round());
    }
    let gb = n / K / K / K;
    let tenth = format!("{gb:.1}");
    if tenth.ends_with(".0") {
        format!("{} GB", gb.round())
    } else {
        format!("{tenth} GB")
    }
}

/// `fmtDuration`'s short style.
pub fn fmt_duration(ms: f64) -> String {
    if !ms.is_finite() || ms < 60_000. {
        if !ms.is_finite() || ms < 0. {
            return "0ms".into();
        }
        if ms < 1_000. {
            return format!("{}ms", ms.round().max(1.));
        }
        if ms < 10_000. {
            let tenths = (ms / 100.).round() / 10.;
            return if tenths >= 10. { "10s".into() } else { format!("{tenths:.1}s") };
        }
        return format!("{}s", (ms / 1_000.).round());
    }
    let total = (ms / 1_000.).round() as i64;
    let (minutes, seconds) = (total / 60, total % 60);
    if seconds == 0 {
        format!("{minutes}m")
    } else {
        format!("{minutes}m {seconds}s")
    }
}

/// `fmtTokens`: whole under a thousand, then thousands and millions to three figures.
pub fn fmt_tokens(n: f64) -> String {
    let count = if n.is_finite() && n > 0. { n.round() } else { 0. };
    if count < 1_000. {
        return format!("{count}");
    }
    let (value, unit) = if count < 999_500. {
        (count / 1_000., "k")
    } else if count < 999_500_000. {
        (count / 1_000_000., "M")
    } else {
        (count / 1_000_000_000., "B")
    };
    let places = if value < 10. {
        2
    } else if value < 100. {
        1
    } else {
        0
    };
    let fixed = format!("{value:.places$}");
    let trimmed = if fixed.contains('.') { fixed.trim_end_matches('0').trim_end_matches('.') } else { &fixed };
    format!("{trimmed}{unit}")
}

/// `fmtUptime`: minutes under an hour, hours and minutes under a day, then days and hours.
pub fn fmt_uptime(ms: f64) -> String {
    let minutes = if ms.is_finite() && ms > 0. { (ms / 60_000.).floor() as i64 } else { 0 };
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h {}m", minutes % 60);
    }
    format!("{}d {}h", hours / 24, hours % 24)
}

/// `fmtElapsed`: whole seconds, then the short style's minutes and seconds.
pub fn fmt_elapsed(ms: f64) -> String {
    let seconds = if ms.is_finite() && ms > 0. { (ms / 1_000.).floor() } else { 0. };
    if seconds < 60. {
        format!("{seconds}s")
    } else {
        fmt_duration(seconds * 1_000.)
    }
}

/// `compactTimeLabel`: how long ago, `now` under a minute, then minutes, hours and days.
pub fn age_word(at: f64, now: f64) -> String {
    let seconds = ((now - at) / 1000.).floor();
    if seconds < 60. {
        return "now".into();
    }
    let minutes = (seconds / 60.).floor();
    if minutes < 60. {
        return format!("{minutes}m");
    }
    let hours = (minutes / 60.).floor();
    if hours < 24. {
        return format!("{hours}h");
    }
    format!("{}d", (hours / 24.).floor())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: Pid, mem: f64, cpu: f64) -> Proc {
        Proc {
            pid,
            name: format!("p{pid}"),
            kind: Kind::Other,
            mem,
            cpu,
            energy: None,
            ports: vec![],
            command: String::new(),
            started_at: None,
            id: None,
            harness: None,
            needed: false,
            endable: false,
            heading: false,
            note: None,
            children: vec![],
        }
    }

    fn group(id: &str, kind: GroupKind, procs: Vec<Proc>, parent: Option<&str>) -> Group {
        let thread = (kind == GroupKind::Thread).then(|| Thread {
            id: id.into(),
            title: format!("Title {id}"),
            harness: "claude".into(),
            project: "p".into(),
            workspace_id: "w".into(),
            session_id: "s".into(),
            working: false,
            pid: None,
            agent_pid: None,
            cgroup: None,
            parent: parent.map(Into::into),
            slate: None,
            subagents: vec![],
        });
        Group { id: id.into(), kind, thread, procs, subagents: vec![], containers: vec![], memory: None }
    }

    #[test]
    fn groups_keep_wsp_first_and_no_thread_last_threads_heaviest_between() {
        let groups = vec![
            group("loose", GroupKind::Loose, vec![proc(9, 900., 0.)], None),
            group("a", GroupKind::Thread, vec![proc(1, 10., 0.)], None),
            group("b", GroupKind::Thread, vec![proc(2, 50., 0.)], None),
            group("c", GroupKind::Thread, vec![proc(3, 45., 0.)], Some("a")),
            group("wsp", GroupKind::Wsp, vec![proc(4, 1., 0.)], None),
        ];
        let sorted = sort_groups(groups, Sort::Mem);
        let ids: Vec<&str> = sorted.iter().map(|g| g.id.as_str()).collect();
        // a holds c under it, 55 against b's 50.
        assert_eq!(ids, vec!["wsp", "a", "b", "c", "loose"]);
        let (tops, kids) = nest_groups(&sorted);
        assert_eq!(tops.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), vec!["wsp", "a", "b", "loose"]);
        assert_eq!(kids["a"][0].id, "c");
    }

    #[test]
    fn a_filter_keeps_a_match_under_its_parents() {
        let mut parent = proc(1, 1., 0.);
        let mut child = proc(2, 1., 0.);
        child.name = "vite".into();
        child.ports = vec![Port { port: 5173, loopback: Some(true) }];
        parent.children = vec![child];
        let groups = vec![group("a", GroupKind::Thread, vec![parent, proc(3, 1., 0.)], None)];
        let left = filter_groups(&groups, " 5173 ", &Picks::default());
        assert_eq!(left[0].procs.len(), 1);
        assert_eq!(left[0].procs[0].children[0].name, "vite");
        assert!(filter_groups(&groups, "title a", &Picks::default())[0].procs.len() == 2);
        assert!(filter_groups(&groups, "nothing", &Picks::default()).is_empty());
    }

    #[test]
    fn figures_read_as_the_web_writes_them() {
        assert_eq!(fmt_bytes(512.), "512 B");
        assert_eq!(fmt_bytes(300. * 1024. * 1024.), "300 MB");
        assert_eq!(fmt_bytes(2. * GIB), "2 GB");
        assert_eq!(fmt_bytes(1.55 * GIB), "1.6 GB");
        assert_eq!(cpu_word(12.345), "12.3%");
        assert_eq!(energy_word(Some(1500.)), "1.5 W");
        assert_eq!(energy_word(Some(0.4)), "0.4 mW");
        assert_eq!(energy_word(Some(42.4)), "42 mW");
        assert_eq!(ports_word(&[Port { port: 3000, loopback: None }, Port { port: 3001, loopback: None }]), ":3000 +1");
        assert_eq!(fmt_tokens(4_270.), "4.27k");
        assert_eq!(fmt_tokens(22_600_000.), "22.6M");
        assert_eq!(fmt_duration(8_200.), "8.2s");
        assert_eq!(fmt_duration(492_000.), "8m 12s");
        assert_eq!(fmt_uptime(3_900_000.), "1h 5m");
        assert_eq!(memory_tint(3. * GIB), Some(Tint::Warn));
        assert_eq!(cpu_tint(450.), Some(Tint::Danger));
    }

    #[test]
    fn history_is_bounded() {
        let mut h = Vec::new();
        for at in 0..400 {
            push_history(&mut h, HistoryPoint { at: at as f64, mem: 0., cpu: 0., energy: None });
        }
        push_history(&mut h, HistoryPoint { at: 399., mem: 1., cpu: 0., energy: None });
        assert_eq!(h.len(), HISTORY_POINTS);
        assert_eq!(h[0].at, 250.);
        assert_eq!(h.last().unwrap().mem, 0.);
    }

    #[test]
    fn a_subagent_that_ended_leaves_after_its_minute() {
        let s = |state, ended_at| SubagentIn {
            id: "s".into(),
            title: "t".into(),
            state,
            started_at: 0.,
            ended_at,
            tokens: None,
            duration_ms: Some(5_000.),
            model: None,
        };
        assert_eq!(subagents_of(&[s(SubState::Running, None)], 10_000.)[0].ms, 10_000.);
        assert_eq!(subagents_of(&[s(SubState::Done, Some(1_000.))], 30_000.)[0].ms, 5_000.);
        assert!(subagents_of(&[s(SubState::Done, Some(1_000.))], 70_000.).is_empty());
    }
}
