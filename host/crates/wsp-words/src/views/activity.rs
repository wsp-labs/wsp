// SPDX-License-Identifier: AGPL-3.0-only
//! The End's plan for a thread's processes, the lines and refusal of processes left running, and the lines an ended agent's thread says. Port of the functions of packages/protocol/src/views/activity.ts that wsp-wire left to this crate, at
//! a3b870a6e; that file's shapes are wsp-wire's. See spec/views_activity.md.

use wsp_wire::views::activity::ActivityRead;
use wsp_wire::views::activity::ActivityScope;
use wsp_wire::views::activity::EndPlan;
use wsp_wire::views::activity::LeftThread;

// activity.ts:456
// export const cloudActivityLine = (threadId: string): string => `thread ${threadWord(threadId)} runs on a cloud machine, and wsp activity reads this computer's processes and a box's:`;
pub fn cloud_activity_line(thread_id: &str) -> String {
    todo!()
}

// activity.ts:458
// export const notJoinedActivityLine = (threadId: string, name: string): string => `thread ${threadWord(threadId)} runs on ${name}, which this host has not joined:`;
pub fn not_joined_activity_line(thread_id: &str, name: &str) -> String {
    todo!()
}

// activity.ts:471
// export const agentEndLine = (pid: number, scope: ActivityScope): string => `${pid} is the agent of thread ${threadWord(scope.threadId)} "${scope.title}", which only its thread's stop ends:`;
pub fn agent_end_line(pid: f64, scope: &ActivityScope) -> String {
    todo!()
}

// activity.ts:472
// export const agentUnderLine = (pid: number, scope: ActivityScope): string => `${pid} has the agent of thread ${threadWord(scope.threadId)} "${scope.title}" under it, which only its thread's stop ends:`;
pub fn agent_under_line(pid: f64, scope: &ActivityScope) -> String {
    todo!()
}

// activity.ts:473
// export const agentEndFix = (scope: ActivityScope): string => `stop its turn with wsp stop ${threadWord(scope.threadId)}, or end a process under the agent.`;
pub fn agent_end_fix(scope: &ActivityScope) -> String {
    todo!()
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
pub fn end_plan(read: &ActivityRead, pid: f64, started_at: f64, may: &dyn Fn(Option<&str>) -> bool) -> EndPlan {
    todo!()
}

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

pub fn processes_left_line(verb: LeftVerb, left: &[LeftThread], theirs: Option<&indexmap::IndexSet<u32>>) -> String {
    todo!()
}

// activity.ts:567
// export const processesLeftRefusal = (verb: "settle" | "delete", left: readonly LeftThread[], theirs?: ReadonlySet<number>): Error =>
//   Object.assign(refusal(processesLeftLine(verb, left, theirs), processesLeftFix(verb, theirs), PROCESSES_LEFT_KIND), { left });
pub fn processes_left_refusal(verb: LeftVerb, left: &[LeftThread], theirs: Option<&indexmap::IndexSet<u32>>) -> crate::exit::HostError {
    todo!()
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
    todo!()
}
