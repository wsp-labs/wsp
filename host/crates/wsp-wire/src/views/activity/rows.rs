// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// A group's rows as a tree: each process under the first of its parents the group holds, siblings busiest first.
pub(super) fn tree_rows(placed: &Placement<'_>, members: &[ProcEntry], ports: Option<&HashMap<u32, Vec<JsInt>>>) -> Vec<ActivityProc> {
    let inside: HashSet<u32> = members.iter().map(|p| p.pid).collect();
    let heads: Vec<ProcEntry> = members.iter().filter(|p| p.ppid == p.pid || !inside.contains(&p.ppid)).cloned().collect();
    let mut out = Vec::new();
    tree_walk(placed, &inside, ports, heads, &mut out);
    out
}

/// Each process then its kids, a generation at a time: a stack of sibling lists in place of a call per generation.
fn tree_walk(
    placed: &Placement<'_>,
    inside: &HashSet<u32>,
    ports: Option<&HashMap<u32, Vec<JsInt>>>,
    heads: Vec<ProcEntry>,
    out: &mut Vec<ActivityProc>,
) {
    let sorted = |mut list: Vec<ProcEntry>| {
        list.sort_by(|a, b| by_proc_column(ProcSort::Cpu, a, b));
        list.into_iter()
    };
    let mut stack = vec![(sorted(heads), 0_i64)];
    while let Some((siblings, depth)) = stack.last_mut() {
        let depth = *depth;
        let Some(p) = siblings.next() else {
            stack.pop();
            continue;
        };
        out.push(ActivityProc {
            pid: JsInt::from(i64::from(p.pid)),
            ppid: JsInt::from(i64::from(p.ppid)),
            depth: JsInt::from(depth),
            started_at: JsNumber::from(p.started_at),
            kind: placed.kind_of(p.pid),
            name: p.comm.clone(),
            command: if p.cmdline.is_empty() { p.comm.clone() } else { p.cmdline.clone() },
            memory: JsNumber::from(p.rss as f64),
            cpu: JsNumber::from(p.cpu),
            ports: ports.and_then(|m| m.get(&p.pid)).cloned().unwrap_or_default(),
        });
        let kids: Vec<ProcEntry> = placed.kids_of(p.pid).into_iter().filter(|k| inside.contains(&k.pid)).collect();
        stack.push((sorted(kids), depth + 1));
    }
}

// views/activity.ts:449 at a3b870a6e
// export type EndPlan = { targets: ProcEntry[] } | { kept: ActivityScope };
/// What an End signals, read before the first signal (the words crate's `end_plan` makes it).
#[derive(Clone, Debug, PartialEq)]
pub enum EndPlan {
    Targets(Vec<ProcEntry>),
    Kept(ActivityScope),
}

/// The placement an End and Activity both read (the TS's private `readPlaced`): the read's scopes as threads
/// (`scope_threads`), no host-less read, no app parts, and a pid listening where `ports` holds a non-empty list for it.
/// The words crate's `end_plan` reads its placement through this.
pub fn with_read_placement<R>(read: &ActivityRead, f: impl FnOnce(&Placement<'_>) -> R) -> R {
    let threads = scope_threads(read);
    let listens = |pid: u32| read.ports.as_ref().and_then(|ports| ports.get(&pid)).is_some_and(|ports| !ports.is_empty());
    let input = PlaceInput {
        procs: &read.procs,
        daemon: read.daemon,
        host: Some(read.host),
        threads: &threads,
        agent_bins: &read.agent_bins,
        listens: &listens,
        app: None,
    };
    f(&place_procs(&input))
}

/// The read's scopes as the placement takes its threads: id, roots, and cgroup, bin and agents where the scope has them.
pub fn scope_threads(read: &ActivityRead) -> Vec<PlaceThread> {
    read.scopes
        .iter()
        .map(|s| PlaceThread {
            id: s.thread_id.clone(),
            roots: s.roots.clone(),
            cgroup: s.cgroup.clone(),
            bin: s.bin.clone(),
            agents: s.agents.clone(),
            tabs: None,
        })
        .collect()
}

// views/activity.ts:454 at a3b870a6e
// export const boxEndLine = (name: string): string => `${name} is a box, and wsp activity end ends processes on this computer:`;
pub fn box_end_line(name: &str) -> String {
    format!("{name} is a box, and wsp activity end ends processes on this computer:")
}

// views/activity.ts:459 at a3b870a6e
// export const notJoinedActivityFix = (name: string): string => `add ${name} again with wsp add, then read its processes.`;
pub fn not_joined_activity_fix(name: &str) -> String {
    format!("add {name} again with wsp add, then read its processes.")
}

// views/activity.ts:461 at a3b870a6e
// export const boxBehindActivityLine = (name: string): string => `${placeBehindLine(name, "its daemon reads no processes in one read")}, so wsp activity reads nothing there:`;
pub fn box_behind_activity_line(name: &str) -> String {
    format!("{}, so wsp activity reads nothing there:", place_behind_line(name, "its daemon reads no processes in one read"))
}

// views/activity.ts:463 at a3b870a6e
// export const notReadLine = (name: string): string => `${name} is no box this host joined, and wsp activity reads this computer's processes and a box's:`;
pub fn not_read_line(name: &str) -> String {
    format!("{name} is no box this host joined, and wsp activity reads this computer's processes and a box's:")
}

// views/activity.ts:469 at a3b870a6e
// export const endGoneSaid = (pid: number): string => `nothing was signalled: the process listed as ${pid} is gone`;
pub fn end_gone_said(pid: i64) -> String {
    format!("nothing was signalled: the process listed as {pid} is gone")
}

// views/activity.ts:474 at a3b870a6e
// export const endGoneLine = (pid: number): string => `no process ${pid} is running on this computer:`;
pub fn end_gone_line(pid: i64) -> String {
    format!("no process {pid} is running on this computer:")
}

// views/activity.ts:475 at a3b870a6e
// export const endReusedLine = (pid: number, now: ProcEntry, asked: number): string =>
//   `pid ${pid} names another process now, ${now.comm} started ${at(now.startedAt)}, not the one started ${at(asked)}:`;
/// Both starts are written as `Date.prototype.toISOString` writes epoch milliseconds: `2026-10-25T10:00:00.000Z`.
pub fn end_reused_line(pid: i64, now: &ProcEntry, asked: i64) -> String {
    format!(
        "pid {pid} names another process now, {} started {}, not the one started {}:",
        now.comm,
        iso_time(now.started_at),
        iso_time(asked)
    )
}

// views/activity.ts:477 at a3b870a6e
// export const wspOwnLine = (pid: number): string => `${pid} is wsp's own process on this computer, which wsp stop and the app end:`;
pub fn wsp_own_line(pid: i64) -> String {
    format!("{pid} is wsp's own process on this computer, which wsp stop and the app end:")
}

// views/activity.ts:479 at a3b870a6e
// export const notThreadsLine = (pid: number): string => `${pid} is not a process of this thread or a thread under it:`;
pub fn not_threads_line(pid: i64) -> String {
    format!("{pid} is not a process of this thread or a thread under it:")
}

// views/activity.ts:480 at a3b870a6e
// export const notStartedLine = (pid: number): string => `wsp did not start ${pid}, and it ends only what a thread or wsp started:`;
pub fn not_started_line(pid: i64) -> String {
    format!("wsp did not start {pid}, and it ends only what a thread or wsp started:")
}

// views/activity.ts:526 at a3b870a6e
// export function processesLeft(view: ActivityView, threadIds: readonly string[]): ActivityThread[] {
//   const asked = new Set(threadIds);
//   return view.threads.flatMap(t => {
//     if (!asked.has(t.threadId)) return [];
//     const byPid = new Map(t.processes.map(p => [p.pid, p]));
//     const underAgent = (p: ActivityProc): boolean => {
//       for (let at: ActivityProc | undefined = p, steps = 0; at !== undefined && steps <= t.processes.length; at = byPid.get(at.ppid), steps++) {
//         if (at.kind === "agent" || isTurnPlumbing(at.command)) return true;
//       }
//       return false;
//     };
//     const processes = t.processes.filter(p => !underAgent(p));
//     return processes.length === 0 ? [] : [{ ...t, processes }];
//   });
// }
/// Each asked thread's processes a settle, a delete or End all would leave running, in the view's thread order; a
/// thread with none left is dropped.
pub fn processes_left(view: &ActivityView, thread_ids: &[String]) -> Vec<ActivityThread> {
    view.threads
        .iter()
        .filter(|t| thread_ids.contains(&t.thread_id))
        .filter_map(|t| {
            let by_pid: HashMap<JsInt, &ActivityProc> = t.processes.iter().map(|p| (p.pid, p)).collect();
            let under_agent = |p: &ActivityProc| {
                let mut at = Some(p);
                let mut steps = 0;
                while let Some(a) = at.filter(|_| steps <= t.processes.len()) {
                    if a.kind == ProcKind::Agent || is_turn_plumbing(&a.command) {
                        return true;
                    }
                    at = by_pid.get(&a.ppid).copied();
                    steps += 1;
                }
                false
            };
            let processes: Vec<ActivityProc> = t.processes.iter().filter(|p| !under_agent(p)).cloned().collect();
            (!processes.is_empty()).then(|| ActivityThread { processes, ..t.clone() })
        })
        .collect()
}

// views/activity.ts:543 at a3b870a6e
// export const endHeads = <P extends { pid: number; ppid: number }>(processes: readonly P[]): P[] => {
//   const ending = new Set(processes.map(p => p.pid));
//   return processes.filter(p => !ending.has(p.ppid));
// };
/// The tops of a set of processes: those whose parent is not among them, in the order given.
pub fn end_heads<P: PidRow + Clone>(processes: &[P]) -> Vec<P> {
    let ending: HashSet<i64> = processes.iter().map(PidRow::pid).collect();
    processes.iter().filter(|p| !ending.contains(&p.ppid())).cloned().collect()
}

/// The verb a refusal over processes left running names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftVerb {
    Settle,
    Delete,
}

// views/activity.ts:562 at a3b870a6e
// export const processesLeftFix = (verb: "settle" | "delete", theirs: ReadonlySet<number> = new Set()): string =>
//   theirs.size > 0
//     ? `a thread ends only what it and the threads under it started, so ask the person to end ${[...theirs].join(" ")}, or ${verb} it again with processes keep to leave them running (--processes on the command line).`
//     : `${verb} it again with processes stop to end them first, or processes keep to leave them running (--processes on the command line).`;
/// `theirs` in the TS is a Set: its pids are written in the order they were added, which is the slice's order here.
pub fn processes_left_fix(verb: LeftVerb, theirs: &[i64]) -> String {
    let verb = match verb {
        LeftVerb::Settle => "settle",
        LeftVerb::Delete => "delete",
    };
    let mut named: Vec<i64> = Vec::new();
    for pid in theirs {
        if !named.contains(pid) {
            named.push(*pid);
        }
    }
    if named.is_empty() {
        return format!("{verb} it again with processes stop to end them first, or processes keep to leave them running (--processes on the command line).");
    }
    let pids: Vec<String> = named.iter().map(i64::to_string).collect();
    format!(
        "a thread ends only what it and the threads under it started, so ask the person to end {}, or {verb} it again with processes keep to leave them running (--processes on the command line).",
        pids.join(" ")
    )
}

// views/activity.ts:578 at a3b870a6e
// export const standingAfterStop = (ended: readonly ActivityEnded[]): ActivityProc[] => ended.flatMap(e => e.ended.filter(p => e.left.includes(p.pid)));
/// The processes a stop signalled that were still there after it, each as it was listed, in the order of `ended`.
pub fn standing_after_stop(ended: &[ActivityEnded]) -> Vec<ActivityProc> {
    ended.iter().flat_map(|e| e.ended.iter().filter(|p| e.left.contains(&p.pid)).cloned()).collect()
}
