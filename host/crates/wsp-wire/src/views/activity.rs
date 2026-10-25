// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/activity.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use wsp_frames::ProcKind;

/// TS: packages/protocol/src/views/activity.ts:77 at a3b870a6e.
pub const PROCESSES_CHOICES: &[&str] = &["stop", "keep"];

/// TS: packages/protocol/src/views/activity.ts:84 at a3b870a6e.
pub const END_GRACE_MS: u64 = 2_000;

/// TS: packages/protocol/src/views/activity.ts:86 at a3b870a6e.
pub const END_GRACE_WORD: &str = "2 s";

/// TS: packages/protocol/src/views/activity.ts:89 at a3b870a6e.
pub const SAME_START_MS: u64 = 2_000;

/// TS: packages/protocol/src/views/activity.ts:124 at a3b870a6e.
/// A Set in the TS, in insertion order.
pub const SCRIPT_RUNTIMES: &[&str] = &["node", "python", "python3", "bun", "deno", "ruby"];

/// TS: packages/protocol/src/views/activity.ts:164 at a3b870a6e.
/// A JavaScript RegExp, flags "": the pattern source as the TS writes it.
pub const RUN_SCRIPT: &str = "^bash \\S*\\/[0-9a-f]{12}\\.sh$";

/// TS: packages/protocol/src/views/activity.ts:174 at a3b870a6e.
/// A JavaScript RegExp, flags "": the pattern source as the TS writes it.
pub const SLATE_FOLDER: &str = "(^|\\/)(wsp-)?slates\\/[^/\\s]+\\/";

/// TS: packages/protocol/src/views/activity.ts:455 at a3b870a6e.
pub const BOX_END_FIX: &str = "end it from that computer's Processes pane in the app, or settle its thread with --processes stop.";

/// TS: packages/protocol/src/views/activity.ts:457 at a3b870a6e.
pub const CLOUD_ACTIVITY_FIX: &str = "open that thread's Processes pane in the app for its processes.";

/// TS: packages/protocol/src/views/activity.ts:462 at a3b870a6e.
pub const BOX_BEHIND_ACTIVITY_FIX: &str = "update it, then read wsp activity again.";

/// TS: packages/protocol/src/views/activity.ts:464 at a3b870a6e.
pub const NOT_READ_FIX: &str = "name this computer or a box wsp computers lists.";

/// TS: packages/protocol/src/views/activity.ts:466 at a3b870a6e.
pub const END_AGAIN_FIX: &str = "read wsp activity again and end it by the pid and start it lists now.";

/// TS: packages/protocol/src/views/activity.ts:478 at a3b870a6e.
pub const WSP_OWN_FIX: &str = "end a thread's process instead.";

/// TS: packages/protocol/src/views/activity.ts:481 at a3b870a6e.
pub const NOT_STARTED_FIX: &str = "end it where it was started; wsp activity lists every process wsp can end.";

/// TS: packages/protocol/src/views/activity.ts:482 at a3b870a6e.
pub const NOT_THREADS_FIX: &str = "a thread ends only what it and the threads under it started; ask the person for anything else.";

/// TS: packages/protocol/src/views/activity.ts:24 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityProc {
    pub pid: i64,
    pub ppid: i64,
    pub depth: i64,
    pub started_at: JsNumber,
    pub kind: ProcKind,
    pub name: String,
    pub command: String,
    pub memory: JsNumber,
    pub cpu: JsNumber,
    pub ports: Vec<i64>,
}

impl Check for ActivityProc {
    /// Rules: depth: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.depth as f64, 0.0).map_err(|e| e.under("depth"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pid" => <i64 as Check>::describe(rest),
                "ppid" => <i64 as Check>::describe(rest),
                "depth" => <i64 as Check>::describe(rest),
                "startedAt" => <JsNumber as Check>::describe(rest),
                "kind" => None,
                "name" => <String as Check>::describe(rest),
                "command" => <String as Check>::describe(rest),
                "memory" => <JsNumber as Check>::describe(rest),
                "cpu" => <JsNumber as Check>::describe(rest),
                "ports" => <Vec<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/activity.ts:38 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityThread {
    pub thread_id: String,
    pub title: String,
    pub workspace_id: String,
    pub memory: JsNumber,
    pub cpu: JsNumber,
    pub processes: Vec<ActivityProc>,
}

impl Check for ActivityThread {
    /// Nested checks: processes.
    fn check(&self) -> Result<(), SchemaError> {
        self.processes.check().map_err(|e| e.under("processes"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threadId" => <String as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "memory" => <JsNumber as Check>::describe(rest),
                "cpu" => <JsNumber as Check>::describe(rest),
                "processes" => <Vec<ActivityProc> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/activity.ts:51 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityView {
    pub computer: String,
    pub at: JsNumber,
    pub threads: Vec<ActivityThread>,
    pub wsp: Vec<ActivityProc>,
    pub unowned: Vec<ActivityProc>,
}

impl Check for ActivityView {
    /// Nested checks: threads, wsp, unowned.
    fn check(&self) -> Result<(), SchemaError> {
        self.threads.check().map_err(|e| e.under("threads"))?;
        self.wsp.check().map_err(|e| e.under("wsp"))?;
        self.unowned.check().map_err(|e| e.under("unowned"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "computer" => <String as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                "threads" => <Vec<ActivityThread> as Check>::describe(rest),
                "wsp" => <Vec<ActivityProc> as Check>::describe(rest),
                "unowned" => <Vec<ActivityProc> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/activity.ts:62 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftThread {
    pub thread_id: String,
    pub title: String,
    pub workspace_id: String,
    pub memory: JsNumber,
    pub cpu: JsNumber,
    pub processes: Vec<ActivityProc>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub on: Option<String>,
}

impl Check for LeftThread {
    /// Nested checks: processes.
    fn check(&self) -> Result<(), SchemaError> {
        self.processes.check().map_err(|e| e.under("processes"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "threadId" => <String as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "memory" => <JsNumber as Check>::describe(rest),
                "cpu" => <JsNumber as Check>::describe(rest),
                "processes" => <Vec<ActivityProc> as Check>::describe(rest),
                "on" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/activity.ts:67 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEnded {
    pub pid: i64,
    pub ended: Vec<ActivityProc>,
    pub forced: Vec<i64>,
    pub left: Vec<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub freed: Option<String>,
}

impl Check for ActivityEnded {
    /// Nested checks: ended.
    fn check(&self) -> Result<(), SchemaError> {
        self.ended.check().map_err(|e| e.under("ended"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pid" => <i64 as Check>::describe(rest),
                "ended" => <Vec<ActivityProc> as Check>::describe(rest),
                "forced" => <Vec<i64> as Check>::describe(rest),
                "left" => <Vec<i64> as Check>::describe(rest),
                "freed" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/activity.ts:80 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcessesAsk {
    #[serde(rename = "ask")]
    Ask,
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "keep")]
    Keep,
}

impl Check for ProcessesAsk {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["ask", "stop", "keep"]))
    }
}

// ---- Hand-written below: the functions of the unit.

use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::HashMap;
use wsp_frames::{CgroupMemory, ProcEntry};

// views/activity.ts:93 at a3b870a6e
// export interface ActivityScope {
//   threadId: string;
//   title: string;
//   workspaceId: string;
//   roots: readonly number[];
//   /** The root of the agent the thread keeps for its next turn, which an End frees through the thread. */
//   kept?: number;
//   /** On a box, the cgroup every process the thread started stands in. */
//   cgroup?: string;
//   /** The command its agent puts on PATH, where the read knows the thread's agent. */
//   bin?: string;
//   /** The agent each running turn's run recorded as it started it. */
//   agents?: readonly number[];
// }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActivityScope {
    pub thread_id: String,
    pub title: String,
    pub workspace_id: String,
    pub roots: Vec<u32>,
    /// The root of the agent the thread keeps for its next turn, which an End frees through the thread.
    pub kept: Option<u32>,
    /// On a box, the cgroup every process the thread started stands in.
    pub cgroup: Option<String>,
    /// The command its agent puts on PATH, where the read knows the thread's agent.
    pub bin: Option<String>,
    /// The agent each running turn's run recorded as it started it.
    pub agents: Option<Vec<u32>>,
}

// views/activity.ts:108 at a3b870a6e
// export interface ActivityRead {
//   computer: string;
//   at: number;
//   procs: readonly ProcEntry[];
//   /** The daemon's pid, as its snapshot names it, and this host's. */
//   daemon: number;
//   host: number;
//   scopes: readonly ActivityScope[];
//   /** The command each agent puts on PATH, which ends the stretch from a root down to the agent. */
//   agentBins: readonly string[];
//   ports?: ReadonlyMap<number, readonly number[]>;
//   /** On a box, each thread cgroup's memory as the daemon read it, which a box thread's total is. */
//   cgroups?: readonly CgroupMemory[];
// }
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityRead {
    pub computer: String,
    pub at: JsNumber,
    pub procs: Vec<ProcEntry>,
    /// The daemon's pid, as its snapshot names it, and this host's.
    pub daemon: u32,
    pub host: u32,
    pub scopes: Vec<ActivityScope>,
    /// The command each agent puts on PATH, which ends the stretch from a root down to the agent.
    pub agent_bins: Vec<String>,
    /// The ports each pid listens on, in the order the reader found them.
    pub ports: Option<HashMap<u32, Vec<i64>>>,
    /// On a box, each thread cgroup's memory as the daemon read it, which a box thread's total is.
    pub cgroups: Option<Vec<CgroupMemory>>,
}

// views/activity.ts:129 at a3b870a6e
// export function procBins(p: Pick<ProcEntry, "comm" | "cmdline">): string[] {
//   const argv = p.cmdline.split(" ").filter(w => w !== "");
//   const base = (w: string | undefined): string | undefined => w?.split("/").at(-1);
//   const first = base(argv[0]);
//   const script = first !== undefined && SCRIPT_RUNTIMES.has(first) ? base(argv.slice(1).find(w => !w.startsWith("-"))) : undefined;
//   return [p.comm, first, script].filter((b): b is string => b !== undefined && b !== "");
// }
/// The names a process runs by, in this order and with empty ones left out: its comm, argv[0]'s base name, and the
/// base name of the first non-flag word after argv[0] where argv[0]'s base name is one of `SCRIPT_RUNTIMES`.
pub fn proc_bins(p: &ProcEntry) -> Vec<String> {
    todo!()
}

// views/activity.ts:140 at a3b870a6e
// export function agentPath(root: number, kidsOf: (pid: number) => readonly ProcEntry[], isAgent: (p: ProcEntry) => boolean, byPid: ReadonlyMap<number, ProcEntry>): number[] | undefined {
//   const via = new Map<number, number>();
//   const queue = [root];
//   while (queue.length > 0) {
//     const pid = queue.shift()!;
//     const p = byPid.get(pid);
//     if (p !== undefined && isAgent(p)) {
//       const path = [pid];
//       for (let at = via.get(pid); at !== undefined; at = via.get(at)) path.unshift(at);
//       for (let kids = kidsOf(pid); kids.length === 1 && isAgent(kids[0]!) && !path.includes(kids[0]!.pid); kids = kidsOf(kids[0]!.pid)) path.push(kids[0]!.pid);
//       return path;
//     }
//     for (const kid of kidsOf(pid)) {
//       if (via.has(kid.pid) || kid.pid === root) continue;
//       via.set(kid.pid, pid);
//       queue.push(kid.pid);
//     }
//   }
//   return undefined;
// }
/// From a root down to the shallowest process under it that is the agent (breadth first, children in the order
/// `kids_of` gives), then on through an only child that is the agent too, root first; None where no process is.
pub fn agent_path(
    root: u32,
    kids_of: &dyn Fn(u32) -> Vec<ProcEntry>,
    is_agent: &dyn Fn(&ProcEntry) -> bool,
    by_pid: &HashMap<u32, ProcEntry>,
) -> Option<Vec<u32>> {
    todo!()
}

// views/activity.ts:167 at a3b870a6e
// export const isTurnPlumbing = (command: string): boolean => RUN_SCRIPT.test(command) || RUN_PUMP.test(command);
/// Whether a command line is a turn's run script (`RUN_SCRIPT`) or its input pump (`tail -n +1 -f <dir>/<12 hex>.in`).
pub fn is_turn_plumbing(command: &str) -> bool {
    todo!()
}

// views/activity.ts:183 at a3b870a6e
// export interface PlaceThread {
//   id: string;
//   /** Its turn's process, the groups its turns led and the agent it keeps: each the top of a stretch down to its agent. */
//   roots: readonly number[];
//   /** On a box, the cgroup everything it started stands in; where it names no root, the heads there are its roots. */
//   cgroup?: string;
//   /** The command its agent puts on PATH; any agent's where left out. */
//   bin?: string;
//   /** The agent's pids its turns' runs recorded as they started it, the agent whatever it runs as. */
//   agents?: readonly number[];
//   /** The renderers of the browser tabs its pane holds, as the desktop app names them on the computer it runs on. */
//   tabs?: readonly number[];
// }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaceThread {
    pub id: String,
    /// Its turn's process, the groups its turns led and the agent it keeps: each the top of a stretch down to its agent.
    pub roots: Vec<u32>,
    /// On a box, the cgroup everything it started stands in; where it names no root, the heads there are its roots.
    pub cgroup: Option<String>,
    /// The command its agent puts on PATH; any agent's where left out.
    pub bin: Option<String>,
    /// The agent's pids its turns' runs recorded as they started it, the agent whatever it runs as.
    pub agents: Option<Vec<u32>>,
    /// The renderers of the browser tabs its pane holds, as the desktop app names them.
    pub tabs: Option<Vec<u32>>,
}

/// What the desktop app says one of its own processes is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppPart {
    Tab,
    App,
}

// views/activity.ts:197 at a3b870a6e
// export interface PlaceInput {
//   procs: readonly ProcEntry[];
//   daemon: number;
//   /** The host's pid, on the computer it runs on. */
//   host?: number;
//   threads: readonly PlaceThread[];
//   agentBins: readonly string[];
//   /** Whether a process listens on a port. */
//   listens: (pid: number) => boolean;
//   /** The desktop app's own processes, each a browser tab or frame, or another part of the app. */
//   app?: ReadonlyMap<number, "tab" | "app">;
// }
pub struct PlaceInput<'a> {
    pub procs: &'a [ProcEntry],
    pub daemon: u32,
    /// The host's pid, on the computer it runs on.
    pub host: Option<u32>,
    pub threads: &'a [PlaceThread],
    pub agent_bins: &'a [String],
    /// Whether a process listens on a port.
    pub listens: &'a dyn Fn(u32) -> bool,
    /// The desktop app's own processes, each a browser tab or another part of the app.
    pub app: Option<&'a HashMap<u32, AppPart>>,
}

// views/activity.ts:211 at a3b870a6e
// export type Whose = { thread: string } | "wsp" | "loose";
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Whose {
    Thread(String),
    Wsp,
    Loose,
}

// views/activity.ts:214 at a3b870a6e
// export interface Placement {
//   /** Every live process: a zombie holds no memory and takes no signal. */
//   procs: readonly ProcEntry[];
//   byPid: ReadonlyMap<number, ProcEntry>;
//   kidsOf: (pid: number) => readonly ProcEntry[];
//   /** Absent for a process that is not wsp's. */
//   whose: ReadonlyMap<number, Whose>;
//   /** The process each thread's agent runs as, or the root that stands for it until it shows, by its thread. */
//   agent: ReadonlyMap<number, string>;
//   /** Every other process the agent's row holds, by that agent's pid: the run script and any shell from a root down to
//    * it, and the input pump and the script's shells left in its group. */
//   foldsInto: ReadonlyMap<number, number>;
//   /** The root each of those stands under. */
//   rootOf: ReadonlyMap<number, number>;
//   kindOf: (pid: number) => ProcKind;
//   /** wsp's own: the daemon, the host, and the app's parts other than its tabs. */
//   needed: (pid: number) => boolean;
//   /** Whether End reaches it: a process a thread or wsp started, never a thread's agent or what wsp needs. */
//   endable: (pid: number) => boolean;
//   /** The thread whose End may reach it: its thread, but never by the WSP_THREAD mark alone, which any process can carry
//    * with any id; there its agent's stretch, its thread's cgroup or its parent's reach, or none. */
//   reachOf: (pid: number) => string | undefined;
// }
/// One computer's processes placed. The maps keep the order the TS `Map`s do: insertion order, a deleted key leaving
/// the others where they stand (`shift_remove`) and a key set again going to the end.
pub struct Placement<'a> {
    /// Every live process (a state starting with `Z` is a zombie and left out), in the read's order.
    pub procs: Vec<ProcEntry>,
    pub by_pid: HashMap<u32, ProcEntry>,
    /// Absent for a process that is not wsp's.
    pub whose: IndexMap<u32, Whose>,
    /// The process each thread's agent runs as, or the root that stands for it until it shows, by its thread.
    pub agent: IndexMap<u32, String>,
    /// Every other process the agent's row holds, by that agent's pid.
    pub folds_into: IndexMap<u32, u32>,
    /// The root each of those stands under.
    pub root_of: IndexMap<u32, u32>,
    input: &'a PlaceInput<'a>,
    children: HashMap<u32, Vec<ProcEntry>>,
    kinds: RefCell<HashMap<u32, ProcKind>>,
}

impl<'a> Placement<'a> {
    /// The children of a pid among the live processes, in the read's order; a self-parented row is no one's child.
    pub fn kids_of(&self, pid: u32) -> Vec<ProcEntry> {
        todo!()
    }

    /// What a process is, worked out once per pid and kept: the first read of a pid answers every later one, and a
    /// pid read while its own kind is being worked out (a parent loop) reads `other`.
    pub fn kind_of(&self, pid: u32) -> ProcKind {
        todo!()
    }

    /// wsp's own: the daemon, the host, and the app's parts other than its tabs.
    pub fn needed(&self, pid: u32) -> bool {
        todo!()
    }

    /// Whether End reaches it: a process a thread or wsp started, never a thread's agent or what wsp needs.
    pub fn endable(&self, pid: u32) -> bool {
        todo!()
    }

    /// The thread whose End may reach it: its thread, but never by the WSP_THREAD mark (`owner.by == env`) alone.
    pub fn reach_of(&self, pid: u32) -> Option<String> {
        todo!()
    }
}

// views/activity.ts:238 at a3b870a6e
// export function placeProcs(input: PlaceInput): Placement {
//   const procs = input.procs.filter(p => !p.state.startsWith("Z"));
//   const byPid = new Map(procs.map(p => [p.pid, p]));
//   const children = new Map<number, ProcEntry[]>();
//   for (const p of procs) if (p.ppid !== p.pid && byPid.has(p.ppid)) (children.get(p.ppid) ?? children.set(p.ppid, []).get(p.ppid)!).push(p);
//   const kidsOf = (pid: number): readonly ProcEntry[] => children.get(pid) ?? [];
//   const threadById = new Map(input.threads.map(t => [t.id, t]));
//   const app = input.app ?? new Map<number, "tab" | "app">();
//   const bins = new Set(input.agentBins);
//   const tabbed = new Map<number, string>();
//   for (const t of input.threads) for (const pid of t.tabs ?? []) if (app.get(pid) === "tab" && !tabbed.has(pid)) tabbed.set(pid, t.id);
//
//   const agent = new Map<number, string>();
//   const foldsInto = new Map<number, number>();
//   const rootOf = new Map<number, number>();
//   const standIns: [number, PlaceThread][] = [];
//   for (const t of input.threads) {
//     const recorded = new Set(t.agents ?? []);
//     const byName = (p: ProcEntry): boolean => procBins(p).some(b => (t.bin !== undefined ? b === t.bin : bins.has(b)));
//     // The daemon stamps kind agent on the root the host named, the run script, never on the agent under it.
//     const stamped = procs.filter(p => p.kind === "agent" && p.owner?.thread === t.id).map(p => p.pid);
//     const named = [...new Set([...t.roots, ...stamped])].filter(r => byPid.has(r));
//     standIns.push(...named.map((r): [number, PlaceThread] => [r, t]));
//     const cgroup = t.cgroup;
//     let heads: number[] = [];
//     if (t.roots.length === 0 && cgroup !== undefined) {
//       const inside = new Set(procs.filter(p => inCgroup(p.cgroup, cgroup)).map(p => p.pid));
//       heads = [...inside].filter(pid => !inside.has(byPid.get(pid)!.ppid));
//     }
//     for (const root of new Set([...named, ...heads])) {
//       // The pid the run recorded is the agent whatever it runs as; its name is read only where none was recorded.
//       let path = agentPath(root, kidsOf, p => recorded.has(p.pid), byPid);
//       if (path === undefined) path = agentPath(root, kidsOf, byName, byPid);
//       else for (let kids = kidsOf(path.at(-1)!); kids.length === 1 && byName(kids[0]!) && !path.includes(kids[0]!.pid); kids = kidsOf(kids[0]!.pid)) path.push(kids[0]!.pid);
//       if (path === undefined) continue;
//       const at = path.at(-1)!;
//       agent.set(at, t.id);
//       foldsInto.delete(at);
//       rootOf.set(at, root);
//       for (const pid of path.slice(0, -1)) {
//         if (agent.has(pid)) continue;
//         foldsInto.set(pid, at);
//         rootOf.set(pid, root);
//       }
//     }
//   }
//   for (const [root, t] of standIns) {
//     if (agent.has(root) || foldsInto.has(root)) continue;
//     agent.set(root, t.id);
//     rootOf.set(root, root);
//   }
//   // The pump and the script's shells stand in the group of the agent's run, under pid 1 once the script's shell is gone.
//   const groupAgent = new Map<number, number>();
//   for (const pid of [...agent.keys(), ...foldsInto.keys()]) {
//     const at = agent.has(pid) ? pid : foldsInto.get(pid)!;
//     const pgid = byPid.get(pid)?.pgid;
//     if (pgid !== undefined && !groupAgent.has(pgid)) groupAgent.set(pgid, at);
//     const root = rootOf.get(pid);
//     if (root !== undefined && !groupAgent.has(root)) groupAgent.set(root, at);
//   }
//   for (const p of procs) {
//     if (p.pgid === undefined || agent.has(p.pid) || foldsInto.has(p.pid) || !isTurnPlumbing(p.cmdline)) continue;
//     const at = groupAgent.get(p.pgid);
//     const stamped = p.owner?.thread;
//     if (at === undefined || (stamped !== undefined && threadById.has(stamped) && stamped !== agent.get(at))) continue;
//     foldsInto.set(p.pid, at);
//     rootOf.set(p.pid, rootOf.get(at)!);
//   }
//
//   const wspPart = (p: ProcEntry): boolean => p.pid === input.daemon || p.pid === input.host || app.has(p.pid) || p.kind === "app" || p.kind === "wsp";
//   const whose = new Map<number, Whose>();
//   const nobody = new Set<number>();
//   const place = (p: ProcEntry, seen: Set<number>): Whose | undefined => {
//     const held = whose.get(p.pid);
//     if (held !== undefined || nobody.has(p.pid)) return held;
//     let said: Whose | undefined;
//     const stamped = p.owner?.thread;
//     const stretch = agent.get(p.pid) ?? agent.get(foldsInto.get(p.pid) ?? -1);
//     if (stamped !== undefined && threadById.has(stamped)) said = { thread: stamped };
//     else if (stretch !== undefined) said = { thread: stretch };
//     else if (tabbed.has(p.pid)) said = { thread: tabbed.get(p.pid)! };
//     else if (wspPart(p)) said = "wsp";
//     else if (startedByWsp(p)) said = "loose";
//     else {
//       const cg = input.threads.find(t => t.cgroup !== undefined && inCgroup(p.cgroup, t.cgroup));
//       const slate = SLATE_FOLDER.test(p.cmdline) ? (input.threads.find(t => p.cmdline.includes(`slates/${t.id}/`)) ?? null) : undefined;
//       if (cg !== undefined) said = { thread: cg.id };
//       else if (slate !== undefined) said = slate === null ? "loose" : { thread: slate.id };
//       else if (p.pty !== undefined) said = "loose";
//       else {
//         const parent = p.ppid !== p.pid ? byPid.get(p.ppid) : undefined;
//         // What the host and the app start outside a thread, the guest tool server among them, is wsp's own.
//         if (parent !== undefined && !seen.has(parent.pid)) said = place(parent, seen.add(p.pid));
//         if (said === undefined && p.owner?.by === "coalition") said = "loose";
//       }
//     }
//     if (said === undefined) nobody.add(p.pid);
//     else whose.set(p.pid, said);
//     return said;
//   };
//   for (const p of procs) place(p, new Set());
//
//   const kinds = new Map<number, ProcKind>();
//   const kindOf = (pid: number): ProcKind => {
//     const held = kinds.get(pid);
//     if (held !== undefined) return held;
//     const p = byPid.get(pid);
//     if (p === undefined) return "other";
//     kinds.set(pid, "other");
//     const part = app.get(pid);
//     const parent = p.ppid !== p.pid ? byPid.get(p.ppid) : undefined;
//     const above = parent === undefined ? null : kindOf(parent.pid);
//     let kind: ProcKind;
//     if (part !== undefined) kind = part;
//     else if (agent.has(pid) || foldsInto.has(pid)) kind = "agent";
//     else if ((pid === input.daemon || pid === input.host) && p.kind === undefined) kind = "wsp";
//     // A shell the pane opened is named terminal to the daemon, which passes the kind down; what runs in it keeps its own.
//     else if (p.kind !== undefined && (p.kind !== "terminal" || p.pty !== undefined)) kind = p.kind;
//     else if (p.pty !== undefined) kind = "terminal";
//     // A thread's agent is found off its roots above; outside this host's threads an agent's binary still names it.
//     else if (typeof whose.get(pid) !== "object" && procBins(p).some(b => bins.has(b))) kind = "agent";
//     else if (SLATE_FOLDER.test(p.cmdline)) kind = "slate";
//     else if (TEST_LINE.test(p.cmdline)) kind = "test";
//     else if (BUILD_LINE.test(p.cmdline)) kind = "build";
//     else if (input.listens(pid) || DEV_LINE.test(p.cmdline)) kind = "dev";
//     else if (above === "dev" || above === "test" || above === "build") kind = above;
//     else if (above === "agent" && !foldsInto.has(parent!.pid) && !SHELLS.has(p.comm)) kind = "tool";
//     else kind = "other";
//     kinds.set(pid, kind);
//     return kind;
//   };
//   /** The stretch a thread of this host runs as its agent, or a root the daemon stamped as one: only its thread ends it. */
//   const threadAgent = (pid: number): boolean => agent.has(pid) || foldsInto.has(pid) || byPid.get(pid)?.kind === "agent";
//   const needed = (pid: number): boolean => {
//     const p = byPid.get(pid);
//     return p !== undefined && wspPart(p) && app.get(pid) !== "tab";
//   };
//   /** A tab's renderer is the app's to end with its tab, and the daemon refuses a signal to anything under the app. An
//    * agent no thread of this host holds is ended like any process wsp started: no thread's stop reaches it. */
//   const endable = (pid: number): boolean => {
//     const p = byPid.get(pid);
//     const w = whose.get(pid);
//     return p !== undefined && w !== undefined && !threadAgent(pid) && !needed(pid) && !app.has(pid) && (typeof w === "object" || startedByWsp(p));
//   };
//   const reachOf = (pid: number, seen = new Set<number>()): string | undefined => {
//     const w = whose.get(pid);
//     const p = byPid.get(pid);
//     if (typeof w !== "object" || p === undefined) return undefined;
//     if (p.owner?.by !== "env") return w.thread;
//     const stretch = agent.get(pid) ?? agent.get(foldsInto.get(pid) ?? -1);
//     if (stretch !== undefined) return stretch;
//     const cg = input.threads.find(t => t.cgroup !== undefined && inCgroup(p.cgroup, t.cgroup));
//     if (cg !== undefined) return cg.id;
//     const parent = p.ppid !== p.pid ? byPid.get(p.ppid) : undefined;
//     return parent === undefined || seen.has(parent.pid) ? undefined : reachOf(parent.pid, seen.add(pid));
//   };
//   return { procs, byPid, kidsOf, whose, agent, foldsInto, rootOf, kindOf, needed, endable, reachOf: pid => reachOf(pid) };
// }
/// One computer's processes placed: the one rule Activity, `wsp activity`, settle and delete read. spec/activity.md
/// gives it step by step.
pub fn place_procs<'a>(input: &'a PlaceInput<'a>) -> Placement<'a> {
    todo!()
}

// views/activity.ts:426 at a3b870a6e
// export function activityOf(read: ActivityRead): ActivityView {
//   const placed = readPlaced(read);
//   const threads: ActivityThread[] = [];
//   for (const scope of read.scopes) {
//     const members = placed.procs.filter(p => threadIdOf(placed.whose.get(p.pid)) === scope.threadId);
//     if (members.length === 0) continue;
//     let memory = 0;
//     let cpu = 0;
//     for (const p of members) {
//       memory += p.rss;
//       cpu += p.cpu;
//     }
//     // A box thread's memory is its cgroup's, as Settings > Activity totals it (`cgroupMemory`); its rows' resident sum
//     // stands where the daemon read none.
//     memory = cgroupMemory(read.cgroups, scope.cgroup) ?? memory;
//     threads.push({ threadId: scope.threadId, title: scope.title, workspaceId: scope.workspaceId, memory, cpu, processes: treeRows(placed, members, read.ports) });
//   }
//   const of = (w: "wsp" | "loose"): ProcEntry[] => placed.procs.filter(p => placed.whose.get(p.pid) === w);
//   return { computer: read.computer, at: read.at, threads, wsp: treeRows(placed, of("wsp"), read.ports), unowned: treeRows(placed, of("loose"), read.ports) };
// }
/// The computer's processes by thread, each thread in the order the read's scopes name it and only where it has any.
pub fn activity_of(read: &ActivityRead) -> ActivityView {
    todo!()
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
    todo!()
}

/// The read's scopes as the placement takes its threads: id, roots, and cgroup, bin and agents where the scope has them.
pub fn scope_threads(read: &ActivityRead) -> Vec<PlaceThread> {
    todo!()
}

// views/activity.ts:454 at a3b870a6e
// export const boxEndLine = (name: string): string => `${name} is a box, and wsp activity end ends processes on this computer:`;
pub fn box_end_line(name: &str) -> String {
    todo!()
}

// views/activity.ts:459 at a3b870a6e
// export const notJoinedActivityFix = (name: string): string => `add ${name} again with wsp add, then read its processes.`;
pub fn not_joined_activity_fix(name: &str) -> String {
    todo!()
}

// views/activity.ts:461 at a3b870a6e
// export const boxBehindActivityLine = (name: string): string => `${placeBehindLine(name, "its daemon reads no processes in one read")}, so wsp activity reads nothing there:`;
/// Calls `crate::wire::daemon_version::place_behind_line(name, "its daemon reads no processes in one read")`.
pub fn box_behind_activity_line(name: &str) -> String {
    todo!()
}

// views/activity.ts:463 at a3b870a6e
// export const notReadLine = (name: string): string => `${name} is no box this host joined, and wsp activity reads this computer's processes and a box's:`;
pub fn not_read_line(name: &str) -> String {
    todo!()
}

// views/activity.ts:469 at a3b870a6e
// export const endGoneSaid = (pid: number): string => `nothing was signalled: the process listed as ${pid} is gone`;
pub fn end_gone_said(pid: i64) -> String {
    todo!()
}

// views/activity.ts:474 at a3b870a6e
// export const endGoneLine = (pid: number): string => `no process ${pid} is running on this computer:`;
pub fn end_gone_line(pid: i64) -> String {
    todo!()
}

// views/activity.ts:475 at a3b870a6e
// export const endReusedLine = (pid: number, now: ProcEntry, asked: number): string =>
//   `pid ${pid} names another process now, ${now.comm} started ${at(now.startedAt)}, not the one started ${at(asked)}:`;
/// Both starts are written as `Date.prototype.toISOString` writes epoch milliseconds: `2026-10-25T10:00:00.000Z`.
pub fn end_reused_line(pid: i64, now: &ProcEntry, asked: i64) -> String {
    todo!()
}

// views/activity.ts:477 at a3b870a6e
// export const wspOwnLine = (pid: number): string => `${pid} is wsp's own process on this computer, which wsp stop and the app end:`;
pub fn wsp_own_line(pid: i64) -> String {
    todo!()
}

// views/activity.ts:479 at a3b870a6e
// export const notThreadsLine = (pid: number): string => `${pid} is not a process of this thread or a thread under it:`;
pub fn not_threads_line(pid: i64) -> String {
    todo!()
}

// views/activity.ts:480 at a3b870a6e
// export const notStartedLine = (pid: number): string => `wsp did not start ${pid}, and it ends only what a thread or wsp started:`;
pub fn not_started_line(pid: i64) -> String {
    todo!()
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
    todo!()
}

// views/activity.ts:543 at a3b870a6e
// export const endHeads = <P extends { pid: number; ppid: number }>(processes: readonly P[]): P[] => {
//   const ending = new Set(processes.map(p => p.pid));
//   return processes.filter(p => !ending.has(p.ppid));
// };
/// The tops of a set of processes: those whose parent is not among them, in the order given.
pub fn end_heads<P: crate::views::activity_fold::PidRow + Clone>(processes: &[P]) -> Vec<P> {
    todo!()
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
    todo!()
}

// views/activity.ts:578 at a3b870a6e
// export const standingAfterStop = (ended: readonly ActivityEnded[]): ActivityProc[] => ended.flatMap(e => e.ended.filter(p => e.left.includes(p.pid)));
/// The processes a stop signalled that were still there after it, each as it was listed, in the order of `ended`.
pub fn standing_after_stop(ended: &[ActivityEnded]) -> Vec<ActivityProc> {
    todo!()
}
