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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

use crate::types::thread_cgroup::{cgroup_memory, in_cgroup};
use crate::views::activity_fold::PidRow;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use wsp_frames::{CgroupMemory, ProcEntry, ProcOwnedBy};

const SHELLS: &[&str] = &["sh", "bash", "zsh", "fish", "dash"];
const TEST_PHRASES: &[&str] = &["vitest", "jest", "mocha", "pytest", "playwright test", "cargo test", "cargo nextest", "go test", "bun test", "node --test"];
const BUILD_PHRASES: &[&str] = &["rustc", "cargo build", "cargo check", "cc1", "cc1plus", "clang", "ld", "tsc", "go build", "webpack", "swiftc"];
const DEV_PHRASES: &[&str] = &["vite", "next dev", "next-server", "webpack serve", "astro dev", "nuxt dev", "remix dev", "wrangler dev", "electron-vite", "http.server", "flask run", "uvicorn", "rails server"];

/// JavaScript's `\s` (what `\S` and `[^\s]` read), which Rust's `is_whitespace` differs from at NEL and the BOM.
fn js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// A phrase the command line holds at a word boundary on both ends, and whose following text `after_ok` accepts.
fn word_match(s: &str, phrase: &str, after_ok: impl Fn(&str) -> bool) -> bool {
    let bytes = s.as_bytes();
    s.match_indices(phrase).any(|(i, _)| {
        let end = i + phrase.len();
        (i == 0 || !is_word_byte(bytes[i - 1])) && (end == bytes.len() || !is_word_byte(bytes[end])) && after_ok(&s[end..])
    })
}

/// `TEST_LINE`: a test runner's name as a word.
fn test_line(s: &str) -> bool {
    TEST_PHRASES.iter().any(|p| word_match(s, p, |_| true))
}

/// `BUILD_LINE`: a compiler or build tool as a word, where `webpack` is not `webpack serve`.
fn build_line(s: &str) -> bool {
    BUILD_PHRASES.iter().any(|p| word_match(s, p, |rest| *p != "webpack" || !rest.starts_with(" serve")))
}

/// `DEV_LINE`: a dev server as a word.
fn dev_line(s: &str) -> bool {
    DEV_PHRASES.iter().any(|p| word_match(s, p, |_| true))
}

/// `SLATE_FOLDER`: a `slates/<id>/` or `wsp-slates/<id>/` folder at the start of a path segment.
fn slate_folder(s: &str) -> bool {
    s.match_indices("slates/").any(|(k, _)| {
        let start = if s[..k].ends_with("wsp-") { k - 4 } else { k };
        let boundary = start == 0 || s.as_bytes()[start - 1] == b'/';
        let run: usize = s[k + 7..].chars().take_while(|c| *c != '/' && !js_space(*c)).map(char::len_utf8).sum();
        boundary && run > 0 && s[k + 7 + run..].starts_with('/')
    })
}

/// Whether `rest` is `<any non-space text>/<12 lower-case hex><ext>`, which a turn's run script or pump names.
fn plumbing_path(rest: &str, ext: &str) -> bool {
    !rest.chars().any(js_space)
        && rest.strip_suffix(ext).is_some_and(|stem| {
            let b = stem.as_bytes();
            b.len() >= 13 && b[b.len() - 13] == b'/' && b[b.len() - 12..].iter().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
        })
}

fn dedupe(items: impl IntoIterator<Item = u32>) -> Vec<u32> {
    let mut seen = HashSet::new();
    items.into_iter().filter(|x| seen.insert(*x)).collect()
}

/// `byProcColumn("cpu")`: busiest first, ties by pid. wire/daemon.rs's `by_proc_column` is the same rule; it is a stub
/// until that unit lands, so this copy stands in and must be replaced by that call.
fn by_cpu_then_pid(a: &ProcEntry, b: &ProcEntry) -> Ordering {
    b.cpu.partial_cmp(&a.cpu).unwrap_or(Ordering::Equal).then(a.pid.cmp(&b.pid))
}

fn thread_id_of(w: Option<&Whose>) -> Option<&str> {
    match w {
        Some(Whose::Thread(t)) => Some(t),
        _ => None,
    }
}

fn app_part(input: &PlaceInput<'_>, pid: u32) -> Option<AppPart> {
    input.app.and_then(|app| app.get(&pid).copied())
}

fn wsp_part(input: &PlaceInput<'_>, p: &ProcEntry) -> bool {
    p.pid == input.daemon || Some(p.pid) == input.host || app_part(input, p.pid).is_some() || matches!(p.kind, Some(ProcKind::App | ProcKind::Wsp))
}

/// `STARTED_BY`: the rungs that name a process a thread started; the folder and coalition rungs do not.
fn started_by_wsp(p: &ProcEntry) -> bool {
    p.owner.as_ref().is_some_and(|o| matches!(o.by, ProcOwnedBy::Root | ProcOwnedBy::Fork | ProcOwnedBy::Group | ProcOwnedBy::Env | ProcOwnedBy::Cgroup))
}

fn thread_known(input: &PlaceInput<'_>, id: &str) -> bool {
    input.threads.iter().any(|t| t.id == id)
}

/// JavaScript's `Date.prototype.toISOString` for epoch milliseconds.
pub fn iso_time(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rest = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let year = if (0..=9999).contains(&y) {
        format!("{y:04}")
    } else if y < 0 {
        format!("-{:06}", -y)
    } else {
        format!("+{y:06}")
    };
    let (h, mi, s, milli) = (rest / 3_600_000, rest / 60_000 % 60, rest / 1000 % 60, rest % 1000);
    format!("{year}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{milli:03}Z")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

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
    let argv: Vec<&str> = p.cmdline.split(' ').filter(|w| !w.is_empty()).collect();
    let base = |w: &str| w.rsplit('/').next().unwrap_or("").to_string();
    let first = argv.first().map(|w| base(w));
    let script = match &first {
        Some(f) if SCRIPT_RUNTIMES.contains(&f.as_str()) => argv[1..].iter().find(|w| !w.starts_with('-')).map(|w| base(w)),
        _ => None,
    };
    [Some(p.comm.clone()), first, script].into_iter().flatten().filter(|b| !b.is_empty()).collect()
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
    let mut via: HashMap<u32, u32> = HashMap::new();
    let mut queue = VecDeque::from([root]);
    while let Some(pid) = queue.pop_front() {
        if by_pid.get(&pid).is_some_and(is_agent) {
            let mut path = vec![pid];
            let mut at = via.get(&pid).copied();
            while let Some(a) = at {
                path.insert(0, a);
                at = via.get(&a).copied();
            }
            let mut kids = kids_of(pid);
            while kids.len() == 1 && is_agent(&kids[0]) && !path.contains(&kids[0].pid) {
                path.push(kids[0].pid);
                kids = kids_of(kids[0].pid);
            }
            return Some(path);
        }
        for kid in kids_of(pid) {
            if via.contains_key(&kid.pid) || kid.pid == root {
                continue;
            }
            via.insert(kid.pid, pid);
            queue.push_back(kid.pid);
        }
    }
    None
}

// views/activity.ts:167 at a3b870a6e
// export const isTurnPlumbing = (command: string): boolean => RUN_SCRIPT.test(command) || RUN_PUMP.test(command);
/// Whether a command line is a turn's run script (`RUN_SCRIPT`) or its input pump (`tail -n +1 -f <dir>/<12 hex>.in`).
pub fn is_turn_plumbing(command: &str) -> bool {
    command.strip_prefix("bash ").is_some_and(|rest| plumbing_path(rest, ".sh")) || command.strip_prefix("tail -n +1 -f ").is_some_and(|rest| plumbing_path(rest, ".in"))
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
        self.children.get(&pid).cloned().unwrap_or_default()
    }

    /// What a process is, worked out once per pid and kept: the first read of a pid answers every later one, and a
    /// pid read while its own kind is being worked out (a parent loop) reads `other`.
    pub fn kind_of(&self, pid: u32) -> ProcKind {
        if let Some(held) = self.kinds.borrow().get(&pid).copied() {
            return held;
        }
        let Some(p) = self.by_pid.get(&pid) else {
            return ProcKind::Other;
        };
        self.kinds.borrow_mut().insert(pid, ProcKind::Other);
        let input = self.input;
        let parent = if p.ppid != p.pid { self.by_pid.get(&p.ppid) } else { None };
        let above = parent.map(|q| self.kind_of(q.pid));
        let kind = if let Some(part) = app_part(input, pid) {
            match part {
                AppPart::Tab => ProcKind::Tab,
                AppPart::App => ProcKind::App,
            }
        } else if self.agent.contains_key(&pid) || self.folds_into.contains_key(&pid) {
            ProcKind::Agent
        } else if (pid == input.daemon || Some(pid) == input.host) && p.kind.is_none() {
            ProcKind::Wsp
        } else if let Some(kind) = p.kind.filter(|k| *k != ProcKind::Terminal || p.pty.is_some()) {
            kind
        } else if p.pty.is_some() {
            ProcKind::Terminal
        } else if !matches!(self.whose.get(&pid), Some(Whose::Thread(_))) && proc_bins(p).iter().any(|b| input.agent_bins.contains(b)) {
            ProcKind::Agent
        } else if slate_folder(&p.cmdline) {
            ProcKind::Slate
        } else if test_line(&p.cmdline) {
            ProcKind::Test
        } else if build_line(&p.cmdline) {
            ProcKind::Build
        } else if (input.listens)(pid) || dev_line(&p.cmdline) {
            ProcKind::Dev
        } else if let Some(kind @ (ProcKind::Dev | ProcKind::Test | ProcKind::Build)) = above {
            kind
        } else if let (Some(ProcKind::Agent), Some(parent)) = (above, parent) {
            if !self.folds_into.contains_key(&parent.pid) && !SHELLS.contains(&p.comm.as_str()) {
                ProcKind::Tool
            } else {
                ProcKind::Other
            }
        } else {
            ProcKind::Other
        };
        self.kinds.borrow_mut().insert(pid, kind);
        kind
    }

    /// wsp's own: the daemon, the host, and the app's parts other than its tabs.
    pub fn needed(&self, pid: u32) -> bool {
        self.by_pid.get(&pid).is_some_and(|p| wsp_part(self.input, p) && app_part(self.input, pid) != Some(AppPart::Tab))
    }

    /// Whether End reaches it: a process a thread or wsp started, never a thread's agent or what wsp needs.
    pub fn endable(&self, pid: u32) -> bool {
        let (Some(p), Some(w)) = (self.by_pid.get(&pid), self.whose.get(&pid)) else {
            return false;
        };
        !self.thread_agent(pid) && !self.needed(pid) && app_part(self.input, pid).is_none() && (matches!(w, Whose::Thread(_)) || started_by_wsp(p))
    }

    /// The stretch a thread of this host runs as its agent, or a root the daemon stamped as one.
    fn thread_agent(&self, pid: u32) -> bool {
        self.agent.contains_key(&pid) || self.folds_into.contains_key(&pid) || self.by_pid.get(&pid).is_some_and(|p| p.kind == Some(ProcKind::Agent))
    }

    /// The thread whose End may reach it: its thread, but never by the WSP_THREAD mark (`owner.by == env`) alone.
    pub fn reach_of(&self, pid: u32) -> Option<String> {
        self.reach(pid, &HashSet::new())
    }

    fn reach(&self, pid: u32, seen: &HashSet<u32>) -> Option<String> {
        let p = self.by_pid.get(&pid)?;
        let Some(Whose::Thread(thread)) = self.whose.get(&pid) else {
            return None;
        };
        if !p.owner.as_ref().is_some_and(|o| o.by == ProcOwnedBy::Env) {
            return Some(thread.clone());
        }
        if let Some(stretch) = self.agent.get(&pid).or_else(|| self.folds_into.get(&pid).and_then(|at| self.agent.get(at))) {
            return Some(stretch.clone());
        }
        if let Some(cg) = self.input.threads.iter().find(|t| t.cgroup.as_deref().is_some_and(|c| in_cgroup(p.cgroup.as_deref(), c))) {
            return Some(cg.id.clone());
        }
        let parent = if p.ppid != p.pid { self.by_pid.get(&p.ppid) } else { None };
        match parent {
            Some(parent) if !seen.contains(&parent.pid) => {
                let mut seen = seen.clone();
                seen.insert(pid);
                self.reach(parent.pid, &seen)
            }
            _ => None,
        }
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
    let procs: Vec<ProcEntry> = input.procs.iter().filter(|p| !p.state.starts_with('Z')).cloned().collect();
    let by_pid: HashMap<u32, ProcEntry> = procs.iter().map(|p| (p.pid, p.clone())).collect();
    let mut children: HashMap<u32, Vec<ProcEntry>> = HashMap::new();
    for p in &procs {
        if p.ppid != p.pid && by_pid.contains_key(&p.ppid) {
            children.entry(p.ppid).or_default().push(p.clone());
        }
    }
    let kids = |pid: u32| children.get(&pid).cloned().unwrap_or_default();

    let mut tabbed: HashMap<u32, String> = HashMap::new();
    for t in input.threads {
        for pid in t.tabs.iter().flatten() {
            if app_part(input, *pid) == Some(AppPart::Tab) && !tabbed.contains_key(pid) {
                tabbed.insert(*pid, t.id.clone());
            }
        }
    }

    let mut agent: IndexMap<u32, String> = IndexMap::new();
    let mut folds_into: IndexMap<u32, u32> = IndexMap::new();
    let mut root_of: IndexMap<u32, u32> = IndexMap::new();
    let mut stand_ins: Vec<(u32, &PlaceThread)> = Vec::new();
    for t in input.threads {
        let recorded: HashSet<u32> = t.agents.iter().flatten().copied().collect();
        let by_name = |p: &ProcEntry| {
            proc_bins(p).iter().any(|b| match &t.bin {
                Some(bin) => b == bin,
                None => input.agent_bins.contains(b),
            })
        };
        let stamped: Vec<u32> = procs
            .iter()
            .filter(|p| p.kind == Some(ProcKind::Agent) && p.owner.as_ref().and_then(|o| o.thread.as_deref()) == Some(t.id.as_str()))
            .map(|p| p.pid)
            .collect();
        let named: Vec<u32> = dedupe(t.roots.iter().copied().chain(stamped)).into_iter().filter(|r| by_pid.contains_key(r)).collect();
        stand_ins.extend(named.iter().map(|r| (*r, t)));
        let mut heads: Vec<u32> = Vec::new();
        if let (true, Some(cgroup)) = (t.roots.is_empty(), t.cgroup.as_deref()) {
            let inside = dedupe(procs.iter().filter(|p| in_cgroup(p.cgroup.as_deref(), cgroup)).map(|p| p.pid));
            let inside_set: HashSet<u32> = inside.iter().copied().collect();
            heads = inside.into_iter().filter(|pid| !inside_set.contains(&by_pid[pid].ppid)).collect();
        }
        for root in dedupe(named.iter().copied().chain(heads)) {
            let path = match agent_path(root, &kids, &|p: &ProcEntry| recorded.contains(&p.pid), &by_pid) {
                Some(mut path) => {
                    let Some(&last) = path.last() else { continue };
                    let mut more = kids(last);
                    while more.len() == 1 && by_name(&more[0]) && !path.contains(&more[0].pid) {
                        path.push(more[0].pid);
                        more = kids(more[0].pid);
                    }
                    Some(path)
                }
                None => agent_path(root, &kids, &by_name, &by_pid),
            };
            let Some(path) = path else { continue };
            let Some(&at) = path.last() else { continue };
            agent.insert(at, t.id.clone());
            folds_into.shift_remove(&at);
            root_of.insert(at, root);
            for pid in &path[..path.len() - 1] {
                if agent.contains_key(pid) {
                    continue;
                }
                folds_into.insert(*pid, at);
                root_of.insert(*pid, root);
            }
        }
    }
    for (root, t) in stand_ins {
        if agent.contains_key(&root) || folds_into.contains_key(&root) {
            continue;
        }
        agent.insert(root, t.id.clone());
        root_of.insert(root, root);
    }

    let mut group_agent: HashMap<u32, u32> = HashMap::new();
    let keys: Vec<u32> = agent.keys().chain(folds_into.keys()).copied().collect();
    for pid in keys {
        let at = if agent.contains_key(&pid) { pid } else { folds_into[&pid] };
        if let Some(pgid) = by_pid.get(&pid).and_then(|p| p.pgid) {
            group_agent.entry(pgid).or_insert(at);
        }
        if let Some(&root) = root_of.get(&pid) {
            group_agent.entry(root).or_insert(at);
        }
    }
    for p in &procs {
        let Some(pgid) = p.pgid else { continue };
        if agent.contains_key(&p.pid) || folds_into.contains_key(&p.pid) || !is_turn_plumbing(&p.cmdline) {
            continue;
        }
        let Some(&at) = group_agent.get(&pgid) else { continue };
        let stamped = p.owner.as_ref().and_then(|o| o.thread.as_deref());
        if stamped.is_some_and(|s| thread_known(input, s) && agent.get(&at).map(String::as_str) != Some(s)) {
            continue;
        }
        folds_into.insert(p.pid, at);
        if let Some(&root) = root_of.get(&at) {
            root_of.insert(p.pid, root);
        }
    }

    let mut placed = Placement {
        procs,
        by_pid,
        whose: IndexMap::new(),
        agent,
        folds_into,
        root_of,
        input,
        children,
        kinds: RefCell::new(HashMap::new()),
    };
    let mut whose: IndexMap<u32, Whose> = IndexMap::new();
    let mut nobody: HashSet<u32> = HashSet::new();
    for p in &placed.procs {
        place_row(&placed, &tabbed, p, &HashSet::new(), &mut whose, &mut nobody);
    }
    placed.whose = whose;
    placed
}

/// One row's whose: the first rung that names it, read once and kept. A row whose parent names it takes the parent's
/// answer, and no rung at all leaves it with nobody.
fn place_row(placed: &Placement<'_>, tabbed: &HashMap<u32, String>, p: &ProcEntry, seen: &HashSet<u32>, whose: &mut IndexMap<u32, Whose>, nobody: &mut HashSet<u32>) -> Option<Whose> {
    if let Some(held) = whose.get(&p.pid) {
        return Some(held.clone());
    }
    if nobody.contains(&p.pid) {
        return None;
    }
    let input = placed.input;
    let stamped = p.owner.as_ref().and_then(|o| o.thread.as_deref()).filter(|s| thread_known(input, s));
    let stretch = placed.agent.get(&p.pid).or_else(|| placed.folds_into.get(&p.pid).and_then(|at| placed.agent.get(at)));
    let said = if let Some(s) = stamped {
        Some(Whose::Thread(s.to_string()))
    } else if let Some(t) = stretch {
        Some(Whose::Thread(t.clone()))
    } else if let Some(t) = tabbed.get(&p.pid) {
        Some(Whose::Thread(t.clone()))
    } else if wsp_part(input, p) {
        Some(Whose::Wsp)
    } else if started_by_wsp(p) {
        Some(Whose::Loose)
    } else if let Some(cg) = input.threads.iter().find(|t| t.cgroup.as_deref().is_some_and(|c| in_cgroup(p.cgroup.as_deref(), c))) {
        Some(Whose::Thread(cg.id.clone()))
    } else if slate_folder(&p.cmdline) {
        let slate = input.threads.iter().find(|t| p.cmdline.contains(&format!("slates/{}/", t.id)));
        Some(slate.map_or(Whose::Loose, |t| Whose::Thread(t.id.clone())))
    } else if p.pty.is_some() {
        Some(Whose::Loose)
    } else {
        let parent = if p.ppid != p.pid { placed.by_pid.get(&p.ppid) } else { None };
        let mut said = match parent {
            Some(parent) if !seen.contains(&parent.pid) => {
                let mut seen = seen.clone();
                seen.insert(p.pid);
                place_row(placed, tabbed, parent, &seen, whose, nobody)
            }
            _ => None,
        };
        if said.is_none() && p.owner.as_ref().is_some_and(|o| o.by == ProcOwnedBy::Coalition) {
            said = Some(Whose::Loose);
        }
        said
    };
    match &said {
        Some(s) => {
            whose.insert(p.pid, s.clone());
        }
        None => {
            nobody.insert(p.pid);
        }
    }
    said
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
    with_read_placement(read, |placed| {
        let mut threads = Vec::new();
        for scope in &read.scopes {
            let members: Vec<ProcEntry> = placed.procs.iter().filter(|p| thread_id_of(placed.whose.get(&p.pid)) == Some(scope.thread_id.as_str())).cloned().collect();
            if members.is_empty() {
                continue;
            }
            let mut memory = 0.0;
            let mut cpu = 0.0;
            for p in &members {
                memory += p.rss as f64;
                cpu += p.cpu;
            }
            let memory = cgroup_memory(read.cgroups.as_deref(), scope.cgroup.as_deref()).map_or(memory, |m| m as f64);
            threads.push(ActivityThread {
                thread_id: scope.thread_id.clone(),
                title: scope.title.clone(),
                workspace_id: scope.workspace_id.clone(),
                memory: JsNumber::from(memory),
                cpu: JsNumber::from(cpu),
                processes: tree_rows(placed, &members, read.ports.as_ref()),
            });
        }
        let of = |w: Whose| -> Vec<ProcEntry> { placed.procs.iter().filter(|p| placed.whose.get(&p.pid) == Some(&w)).cloned().collect() };
        ActivityView {
            computer: read.computer.clone(),
            at: read.at.clone(),
            threads,
            wsp: tree_rows(placed, &of(Whose::Wsp), read.ports.as_ref()),
            unowned: tree_rows(placed, &of(Whose::Loose), read.ports.as_ref()),
        }
    })
}

/// A group's rows as a tree: each process under the first of its parents the group holds, siblings busiest first.
fn tree_rows(placed: &Placement<'_>, members: &[ProcEntry], ports: Option<&HashMap<u32, Vec<i64>>>) -> Vec<ActivityProc> {
    let inside: HashSet<u32> = members.iter().map(|p| p.pid).collect();
    let heads: Vec<ProcEntry> = members.iter().filter(|p| p.ppid == p.pid || !inside.contains(&p.ppid)).cloned().collect();
    let mut out = Vec::new();
    tree_walk(placed, &inside, ports, &heads, 0, &mut out);
    out
}

fn tree_walk(placed: &Placement<'_>, inside: &HashSet<u32>, ports: Option<&HashMap<u32, Vec<i64>>>, list: &[ProcEntry], depth: i64, out: &mut Vec<ActivityProc>) {
    let mut sorted = list.to_vec();
    sorted.sort_by(by_cpu_then_pid);
    for p in &sorted {
        out.push(ActivityProc {
            pid: i64::from(p.pid),
            ppid: i64::from(p.ppid),
            depth,
            started_at: JsNumber::from(p.started_at),
            kind: placed.kind_of(p.pid),
            name: p.comm.clone(),
            command: if p.cmdline.is_empty() { p.comm.clone() } else { p.cmdline.clone() },
            memory: JsNumber::from(p.rss as f64),
            cpu: JsNumber::from(p.cpu),
            ports: ports.and_then(|m| m.get(&p.pid)).cloned().unwrap_or_default(),
        });
        let kids: Vec<ProcEntry> = placed.kids_of(p.pid).into_iter().filter(|k| inside.contains(&k.pid)).collect();
        tree_walk(placed, inside, ports, &kids, depth + 1, out);
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
/// The words of `wire::daemon_version::place_behind_line(name, "its daemon reads no processes in one read")` written
/// here until that stub lands; replace them with the call then.
pub fn box_behind_activity_line(name: &str) -> String {
    let behind = format!("{name} is behind: its daemon reads no processes in one read; wsp add {name} --update puts this wsp's daemon on it");
    format!("{behind}, so wsp activity reads nothing there:")
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
    format!("pid {pid} names another process now, {} started {}, not the one started {}:", now.comm, iso_time(i64::from(now.started_at)), iso_time(asked))
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
            let by_pid: HashMap<i64, &ActivityProc> = t.processes.iter().map(|p| (p.pid, p)).collect();
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
