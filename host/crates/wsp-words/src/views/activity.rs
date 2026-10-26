// SPDX-License-Identifier: AGPL-3.0-only
//! The End's plan for a thread's processes, the lines and refusal of processes left running, and the lines an ended agent's thread says. Port of the functions of packages/protocol/src/views/activity.ts that wsp-wire left to this crate, at
//! the cut (383708c78); that file's shapes are wsp-wire's. See spec/views_activity.md.

use indexmap::IndexSet;
use serde_json::Value;

use wsp_frames::{ProcEntry, ProcKind, ProcOwnedBy};
use wsp_wire::views::activity::{
    end_gone_line, end_reused_line, not_started_line, not_threads_line, processes_left_fix, with_read_placement, wsp_own_line,
    ActivityProc, ActivityRead, ActivityScope, EndPlan, LeftThread, LeftVerb as WireLeftVerb, Placement, Whose, END_AGAIN_FIX,
    NOT_STARTED_FIX, NOT_THREADS_FIX, SAME_START_MS, WSP_OWN_FIX,
};

use crate::exit::{refusal, HostError, Kind};
use crate::js::number_string;
use crate::words::computer::thread_word;

// activity.ts:456
// export const cloudActivityLine = (threadId: string): string => `thread ${threadWord(threadId)} runs on a cloud machine, and wsp activity reads this computer's processes and a box's:`;
pub fn cloud_activity_line(thread_id: &str) -> String {
    format!("thread {} runs on a cloud machine, and wsp activity reads this computer's processes and a box's:", thread_word(thread_id))
}

// activity.ts:458
// export const notJoinedActivityLine = (threadId: string, name: string): string => `thread ${threadWord(threadId)} runs on ${name}, which this host has not joined:`;
pub fn not_joined_activity_line(thread_id: &str, name: &str) -> String {
    format!("thread {} runs on {name}, which this host has not joined:", thread_word(thread_id))
}

// activity.ts:471
// export const agentEndLine = (pid: number, scope: ActivityScope): string => `${pid} is the agent of thread ${threadWord(scope.threadId)} "${scope.title}", which only its thread's stop ends:`;
pub fn agent_end_line(pid: f64, scope: &ActivityScope) -> String {
    format!(
        "{} is the agent of thread {} \"{}\", which only its thread's stop ends:",
        number_string(pid),
        thread_word(&scope.thread_id),
        scope.title
    )
}

// activity.ts:472
// export const agentUnderLine = (pid: number, scope: ActivityScope): string => `${pid} has the agent of thread ${threadWord(scope.threadId)} "${scope.title}" under it, which only its thread's stop ends:`;
pub fn agent_under_line(pid: f64, scope: &ActivityScope) -> String {
    format!(
        "{} has the agent of thread {} \"{}\" under it, which only its thread's stop ends:",
        number_string(pid),
        thread_word(&scope.thread_id),
        scope.title
    )
}

// activity.ts:473
// export const agentEndFix = (scope: ActivityScope): string => `stop its turn with wsp stop ${threadWord(scope.threadId)}, or end a process under the agent.`;
pub fn agent_end_fix(scope: &ActivityScope) -> String {
    format!("stop its turn with wsp stop {}, or end a process under the agent.", thread_word(&scope.thread_id))
}

/// The End of one process, by the pid and the start Activity listed it with: only one Activity lists, so a process
/// wsp did not start is refused. `may` is whether the caller may end a process of that thread, or of no thread when
/// it is undefined.
// activity.ts:487
// export function endPlan(read: ActivityRead, pid: number, startedAt: number, may: (threadId: string | undefined) => boolean): EndPlan {
//   const placed = readPlaced(read);
//   const p = placed.byPid.get(pid);
//   if (p === undefined) throw refusal(endGoneLine(pid), END_AGAIN_FIX, "not-found");
//   const scopeOf = (threadId: string | undefined): ActivityScope | undefined => read.scopes.find(s => s.threadId === threadId);
//   // Reach before the start, so a caller learns nothing of a process it may not end, not even its name.
//   const owner = threadIdOf(placed.whose.get(pid));
//   if (!may(placed.reachOf(pid))) throw refusal(notThreadsLine(pid), NOT_THREADS_FIX, "usage");
//   if (pid === read.daemon || pid === read.host) throw refusal(wspOwnLine(pid), WSP_OWN_FIX, "usage");
//   if (owner === undefined && !startedByWsp(p)) throw refusal(notStartedLine(pid), NOT_STARTED_FIX, "usage");
//   if (Math.abs(p.startedAt - startedAt) > SAME_START_MS) throw refusal(endReusedLine(pid, p, startedAt), END_AGAIN_FIX, "not-found");
//   const agentOf = (q: number): ActivityScope | undefined => scopeOf(placed.agent.get(q) ?? placed.agent.get(placed.foldsInto.get(q) ?? -1));
//   const agent = agentOf(pid);
//   if (agent !== undefined) {
//     if (agent.kept !== undefined && placed.rootOf.get(pid) === agent.kept) return { kept: agent };
//     throw refusal(agentEndLine(pid, agent), agentEndFix(agent), "usage");
//   }
//   if (p.kind === "agent") {
//     const scope = scopeOf(owner);
//     if (scope !== undefined) throw refusal(agentEndLine(pid, scope), agentEndFix(scope), "usage");
//     throw refusal(`${pid} is an agent, which only its thread's stop ends:`, "stop its thread's turn with wsp stop.", "usage");
//   }
//   if (!placed.endable(pid)) throw refusal(notStartedLine(pid), NOT_STARTED_FIX, "usage");
//   const targets: ProcEntry[] = [];
//   const take = (q: ProcEntry): void => {
//     const under = agentOf(q.pid);
//     if (under !== undefined) throw refusal(agentUnderLine(pid, under), agentEndFix(under), "usage");
//     if (q.pid === read.daemon || q.pid === read.host) throw refusal(wspOwnLine(q.pid), WSP_OWN_FIX, "usage");
//     targets.push(q);
//     for (const kid of placed.kidsOf(q.pid)) take(kid);
//   };
//   take(p);
//   return { targets };
// }
/// The TS throws (a refusal) are the Err.
pub fn end_plan(
    read: &ActivityRead,
    pid: f64,
    started_at: f64,
    may: &dyn Fn(Option<&str>) -> bool,
) -> Result<EndPlan, crate::exit::HostError> {
    with_read_placement(read, |placed| {
        let whole = pid as i64;
        let Some((k, p)) = u32::try_from(whole).ok().and_then(|k| placed.by_pid.get(&k).map(|p| (k, p))) else {
            return Err(refusal(&end_gone_line(whole), END_AGAIN_FIX, Some(Kind::NotFound)));
        };
        let owner = thread_id_of(placed.whose.get(&k));
        if !may(placed.reach_of(k).as_deref()) {
            return Err(refusal(&not_threads_line(whole), NOT_THREADS_FIX, Some(Kind::Usage)));
        }
        if k == read.daemon || k == read.host {
            return Err(refusal(&wsp_own_line(whole), WSP_OWN_FIX, Some(Kind::Usage)));
        }
        let started_by_wsp = p.owner.as_ref().is_some_and(|o| {
            matches!(o.by, ProcOwnedBy::Root | ProcOwnedBy::Fork | ProcOwnedBy::Group | ProcOwnedBy::Env | ProcOwnedBy::Cgroup)
        });
        if owner.is_none() && !started_by_wsp {
            return Err(refusal(&not_started_line(whole), NOT_STARTED_FIX, Some(Kind::Usage)));
        }
        if (p.started_at as f64 - started_at).abs() > SAME_START_MS as f64 {
            if crate::js_date::time_value(p.started_at as f64).is_none() || crate::js_date::time_value(started_at).is_none() {
                return Err(HostError::new("Invalid time value"));
            }
            return Err(refusal(&end_reused_line(whole, p, started_at as i64), END_AGAIN_FIX, Some(Kind::NotFound)));
        }
        if let Some(agent) = agent_scope_of(placed, read, k) {
            if agent.kept.is_some() && placed.root_of.get(&k).copied() == agent.kept {
                return Ok(EndPlan::Kept(agent.clone()));
            }
            return Err(refusal(&agent_end_line(pid, agent), &agent_end_fix(agent), Some(Kind::Usage)));
        }
        if p.kind == Some(ProcKind::Agent) {
            if let Some(scope) = scope_of(read, owner) {
                return Err(refusal(&agent_end_line(pid, scope), &agent_end_fix(scope), Some(Kind::Usage)));
            }
            return Err(refusal(
                &format!("{} is an agent, which only its thread's stop ends:", number_string(pid)),
                "stop its thread's turn with wsp stop.",
                Some(Kind::Usage),
            ));
        }
        if !placed.endable(k) {
            return Err(refusal(&not_started_line(whole), NOT_STARTED_FIX, Some(Kind::Usage)));
        }
        let mut targets = Vec::new();
        let mut stack = vec![Take::In(p.clone())];
        let mut above = std::collections::HashSet::new();
        while let Some(step) = stack.pop() {
            let q = match step {
                Take::In(q) => q,
                Take::Out(pid) => {
                    above.remove(&pid);
                    continue;
                }
            };
            if !above.insert(q.pid) {
                return Err(HostError::new("Maximum call stack size exceeded"));
            }
            if let Some(under) = agent_scope_of(placed, read, q.pid) {
                return Err(refusal(&agent_under_line(pid, under), &agent_end_fix(under), Some(Kind::Usage)));
            }
            if q.pid == read.daemon || q.pid == read.host {
                return Err(refusal(&wsp_own_line(i64::from(q.pid)), WSP_OWN_FIX, Some(Kind::Usage)));
            }
            stack.push(Take::Out(q.pid));
            stack.extend(placed.kids_of(q.pid).into_iter().rev().map(Take::In));
            targets.push(q);
        }
        Ok(EndPlan::Targets(targets))
    })
}

/// A step of the walk down from the process an End names: a process to take, or the leaving of one whose kids are done.
enum Take {
    In(ProcEntry),
    Out(u32),
}

fn scope_of<'a>(read: &'a ActivityRead, thread: Option<&str>) -> Option<&'a ActivityScope> {
    thread.and_then(|id| read.scopes.iter().find(|s| s.thread_id == id))
}

/// The scope of the thread whose agent a process is, or the root that stands for it, or the one its row folds into.
fn agent_scope_of<'a>(placed: &Placement<'_>, read: &'a ActivityRead, pid: u32) -> Option<&'a ActivityScope> {
    let thread = placed.agent.get(&pid).or_else(|| placed.folds_into.get(&pid).and_then(|root| placed.agent.get(root)));
    scope_of(read, thread.map(String::as_str))
}

fn thread_id_of(whose: Option<&Whose>) -> Option<&str> {
    match whose {
        Some(Whose::Thread(thread)) => Some(thread.as_str()),
        _ => None,
    }
}

/// The most processes a refusal names before it counts the rest.
const LEFT_NAMED: usize = 8;

/// What a settle or a delete would leave running, each process named; `theirs` holds the pids the caller may not end,
/// which a thread hears as the person's to end before it asks for a stop that would refuse them.
// activity.ts:553
// export const processesLeftLine = (verb: "settle" | "delete", left: readonly LeftThread[], theirs: ReadonlySet<number> = new Set()): string => {
//   const rows = left.flatMap(t => t.processes.map(p => ({ t, p })));
//   const named = rows
//     .slice(0, LEFT_NAMED)
//     .map(({ t, p }) => `${p.pid} ${p.name} (${[p.kind, ...(p.ports.length > 0 ? [`:${p.ports.join(" :")}`] : []), ...(theirs.has(p.pid) ? ["the person's to end"] : [])].join(", ")}) of ${threadWord(t.threadId)}${t.on !== undefined ? ` on ${t.on}` : ""}`);
//   const more = rows.length > LEFT_NAMED ? ` and ${rows.length - LEFT_NAMED} more` : "";
//   return `${verb === "settle" ? "a thread to settle" : "the thread to delete"} still runs ${rows.length === 1 ? "a process" : `${rows.length} processes`}: ${named.join(", ")}${more}:`;
// };
/// The two verbs that can leave processes running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LeftVerb {
    Settle,
    Delete,
}

pub fn processes_left_line(verb: LeftVerb, left: &[LeftThread], theirs: Option<&IndexSet<u32>>) -> String {
    let rows: Vec<(&LeftThread, &ActivityProc)> = left.iter().flat_map(|t| t.processes.iter().map(move |p| (t, p))).collect();
    let named: Vec<String> = rows
        .iter()
        .take(LEFT_NAMED)
        .map(|(t, p)| {
            let kind = serde_json::to_value(p.kind).ok().and_then(|word| word.as_str().map(str::to_string)).unwrap_or_default();
            let mut tags = vec![kind];
            if !p.ports.is_empty() {
                tags.push(format!(":{}", p.ports.iter().map(ToString::to_string).collect::<Vec<_>>().join(" :")));
            }
            if theirs.is_some_and(|pids| pids.contains(&(p.pid.get() as u32))) {
                tags.push("the person's to end".to_string());
            }
            let on = t.on.as_deref().map(|on| format!(" on {on}")).unwrap_or_default();
            format!("{} {} ({}) of {}{on}", p.pid, p.name, tags.join(", "), thread_word(&t.thread_id))
        })
        .collect();
    let more = if rows.len() > LEFT_NAMED { format!(" and {} more", rows.len() - LEFT_NAMED) } else { String::new() };
    let subject = match verb {
        LeftVerb::Settle => "a thread to settle",
        LeftVerb::Delete => "the thread to delete",
    };
    let count = if rows.len() == 1 { "a process".to_string() } else { format!("{} processes", rows.len()) };
    format!("{subject} still runs {count}: {}{more}:", named.join(", "))
}

fn wire_verb(verb: LeftVerb) -> WireLeftVerb {
    match verb {
        LeftVerb::Settle => WireLeftVerb::Settle,
        LeftVerb::Delete => WireLeftVerb::Delete,
    }
}

// activity.ts:567
// export const processesLeftRefusal = (verb: "settle" | "delete", left: readonly LeftThread[], theirs?: ReadonlySet<number>): Error =>
//   Object.assign(refusal(processesLeftLine(verb, left, theirs), processesLeftFix(verb, theirs), PROCESSES_LEFT_KIND), { left });
pub fn processes_left_refusal(verb: LeftVerb, left: &[LeftThread], theirs: Option<&IndexSet<u32>>) -> crate::exit::HostError {
    let theirs_pids: Vec<i64> = theirs.map(|pids| pids.iter().map(|pid| i64::from(*pid)).collect()).unwrap_or_default();
    let fix = processes_left_fix(wire_verb(verb), &theirs_pids);
    let left_values = left.iter().map(|t| serde_json::to_value(t).expect("a thread serializes")).collect();
    HostError { left: Some(left_values), ..refusal(&processes_left_line(verb, left, theirs), &fix, Some(Kind::ProcessesLeft)) }
}

/// The threads a refusal over processes left running carries, as a reply puts them beside its error.
// activity.ts:571
// export function processesLeftOf(e: unknown): { left?: LeftThread[] } {
//   if (typeof e !== "object" || e === null || (e as { kind?: unknown }).kind !== PROCESSES_LEFT_KIND) return {};
//   const read = z.array(LeftThread).safeParse((e as { left?: unknown }).left);
//   return read.success ? { left: read.data } : {};
// }
/// The TS `{ left? }`: None where the error is not a processes-left refusal or its `left` does not read as threads.
pub fn processes_left_of(e: &crate::exit::HostError) -> Option<Vec<LeftThread>> {
    if e.kind != Some(Kind::ProcessesLeft) {
        return None;
    }
    let left = e.left.clone()?;
    wsp_wire::types::check::read::<Vec<LeftThread>>(&Value::Array(left)).ok()
}
