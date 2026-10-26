// SPDX-License-Identifier: AGPL-3.0-only
//! The process side of a turn on this computer: the tree's CPU through ps, what a kept turn left in its group,
//! whether a recorded pid still leads its run's group, the reap of a run's group and files, the held seed and late
//! line hand-overs, and the queue a local stream's lines wait in. Port of packages/runtime/src/local-exec.ts:38-435
//! at 65f6b89c7. Spec: spec/local_exec_reap.md.
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, UNIX_EPOCH};

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::Stream;
use indexmap::IndexSet;
use wsp_adapters::port::ExecFailure;
use wsp_words::js::{js_round, to_number};
use wsp_words::ps_time::ps_cpu_seconds;

use crate::clock::{Clock, RealClock};
use crate::machine_exec::LineCut;

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// The state behind a lock another thread can hold only briefly; a poisoned one is read as it was left.
pub(super) fn lock<T>(state: &Mutex<T>) -> MutexGuard<'_, T> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A file's text, or `None` where it cannot be read (TS `readFile`).
pub(super) fn read_file(path: &str) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// `kill(target, 0)` succeeds, or fails with EPERM: the target exists.
fn probe(target: i32) -> bool {
    // SAFETY: signal 0 sends nothing; it asks the kernel whether the target exists.
    if unsafe { libc::kill(target, 0) } == 0 {
        return true;
    }
    io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

pub(super) fn append(path: &str, text: &[u8]) -> io::Result<()> {
    OpenOptions::new().append(true).create(true).open(path)?.write_all(text)
}

/// One reading of the work a turn's tree has done, in the ticks the activity clock counts, for the leader given;
/// `None` where this computer cannot answer for the tree.
// TS local-exec.ts:46-48
// /** One reading of the work a turn's process tree has done, in the ticks the activity clock counts, or undefined
//  * when this computer cannot answer for the tree. */
// export type TreeWorkReader = (leader: number, then: (ticks: number | undefined) => void) => void;
pub type TreeWorkReader = Arc<dyn Fn(i32) -> LocalBoxFuture<'static, Option<i64>>>;

/// How often the reap looks at the group it asked to go.
pub const GRACE_POLL_MS: u64 = 200;
/// How far a leader's age may sit from its claim's before it is read as somebody else's process.
pub const LEADER_WINDOW_MS: i64 = 5_000;
/// The files `reap_run` deletes, in its order, before the claim folder.
pub const REAPED_SUFFIXES: [&str; 11] = ["sh", "log", "pid", "agent", "exit", "in", "held", "late", "fifo", "tail", "pump"];

/// One turn's whole group: a signal to `-pid`, every error swallowed (ESRCH and EPERM alike).
// TS local-exec.ts:85-94
// /** One turn's whole group, by the pid its launch recorded: the child leads the group, so a negative pid is every
//  * process the turn started. A group whose last member is already gone answers ESRCH, which is the same nothing to
//  * do as no group at all. */
// function signalGroup(pid: number, sig: NodeJS.Signals): void {
//   try {
//     process.kill(-pid, sig);
//   } catch {
//     return;
//   }
// }
pub fn signal_group(pid: i32, sig: i32) {
    // SAFETY: a negative pid names the group of a process this host started; the error is swallowed as in the TS.
    unsafe {
        libc::kill(-pid, sig);
    }
}

/// `kill(pid, 0)` succeeds or fails with EPERM.
// TS local-exec.ts:96-103
// function alive(pid: number): boolean {
//   try {
//     process.kill(pid, 0);
//     return true;
//   } catch (e) {
//     return (e as { code?: string }).code === "EPERM";
//   }
// }
pub fn alive(pid: i32) -> bool {
    probe(pid)
}

/// A signal to the pid alone, every error swallowed.
// TS local-exec.ts:228-234
// function signalPid(pid: number, sig: NodeJS.Signals): void {
//   try {
//     process.kill(pid, sig);
//   } catch {
//     return;
//   }
// }
pub fn signal_pid(pid: i32, sig: i32) {
    // SAFETY: the pid is one this host recorded for a process of a turn; the error is swallowed as in the TS.
    unsafe {
        libc::kill(pid, sig);
    }
}

/// Whether a process group with this id exists: `kill(-pid, 0)` succeeds or fails with EPERM.
// TS local-exec.ts:256-265
// /** Whether a process group with this id exists at all: signal 0 asks the kernel and sends nothing, and a group
//  * whose last member is gone answers ESRCH. EPERM is a group under another login, which exists. */
// export function groupExists(pid: number): boolean {
//   try {
//     process.kill(-pid, 0);
//     return true;
//   } catch (e) {
//     return (e as { code?: string }).code === "EPERM";
//   }
// }
pub fn group_exists(pid: i32) -> bool {
    probe(-pid)
}

/// ps's cumulative CPU column in hundredths, `js_round`ed; 0 when not a safe integer.
// TS local-exec.ts:113-118
// /** Cumulative CPU as ps prints it, in the ticks the activity clock counts. The column is parsed in one place, which
//  * the daemon's processes module reads too. */
// function cpuTicks(time: string): number {
//   const ticks = Math.round(psCpuSeconds(time) * 100);
//   return Number.isSafeInteger(ticks) ? ticks : 0;
// }
pub fn cpu_ticks(time: &str) -> i64 {
    let ticks = js_round(ps_cpu_seconds(time) * 100.0);
    if ticks.is_finite() && ticks.fract() == 0.0 && ticks.abs() <= MAX_SAFE_INTEGER as f64 {
        ticks as i64
    } else {
        0
    }
}

/// What one tree reader remembers between readings: each process's ticks at the last reading, and the total so far.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TreeTally {
    seen: HashMap<i64, i64>,
    total: i64,
}

impl TreeTally {
    /// One `ps -eo pid=,ppid=,pgid=,time=` output read for `leader`: answers the new total, which never falls.
    // TS local-exec.ts:132-168
    //   let seen = new Map<number, number>();
    //   let total = 0;
    //   const add = (stdout: string, leader: number): number => {
    //     const rows = new Map<number, { ppid: number; pgid: number; ticks: number }>();
    //     for (const row of stdout.split("\n")) {
    //       const [pid = "", ppid = "", pgid = "", time = ""] = row.trim().split(/\s+/);
    //       if (pid !== "") rows.set(Number(pid), { ppid: Number(ppid), pgid: Number(pgid), ticks: cpuTicks(time) });
    //     }
    //     const inTree = new Map<number, boolean>();
    //     const reaches = (pid: number): boolean => {
    //       const path = new Set<number>();
    //       let at = pid;
    //       let found = inTree.get(at);
    //       while (found === undefined) {
    //         const row = rows.get(at);
    //         if (at === leader || row?.pgid === leader) found = true;
    //         // A parent loop ends the walk: procps reads /proc pid by pid, so a pid reused mid-read can print one.
    //         else if (row === undefined || at <= 1 || path.has(at)) found = false;
    //         else {
    //           path.add(at);
    //           at = row.ppid;
    //           found = inTree.get(at);
    //         }
    //       }
    //       for (const p of path) inTree.set(p, found);
    //       return found;
    //     };
    //     const now = new Map<number, number>();
    //     for (const [pid, { ticks }] of rows) {
    //       if (!reaches(pid)) continue;
    //       const before = seen.get(pid);
    //       total += before === undefined || ticks < before ? ticks : ticks - before;
    //       now.set(pid, ticks);
    //     }
    //     seen = now;
    //     return total;
    //   };
    pub fn add(&mut self, stdout: &str, leader: i64) -> i64 {
        let mut rows: HashMap<i64, TreeRow> = HashMap::new();
        for row in stdout.split('\n') {
            let mut words = row.split_whitespace();
            let Some(pid) = words.next().and_then(|w| w.parse::<i64>().ok()) else { continue };
            let mut number = || words.next().and_then(|w| w.parse::<i64>().ok()).unwrap_or(0);
            let ppid = number();
            let pgid = number();
            let ticks = cpu_ticks(words.next().unwrap_or(""));
            rows.insert(pid, TreeRow { ppid, pgid, ticks });
        }
        let mut in_tree: HashMap<i64, bool> = HashMap::new();
        let mut now: HashMap<i64, i64> = HashMap::new();
        for (&pid, row) in &rows {
            if !reaches(&rows, leader, &mut in_tree, pid) {
                continue;
            }
            let before = self.seen.get(&pid).copied();
            self.total += match before {
                Some(before) if row.ticks >= before => row.ticks - before,
                _ => row.ticks,
            };
            now.insert(pid, row.ticks);
        }
        self.seen = now;
        self.total
    }
}

#[derive(Clone, Copy)]
struct TreeRow {
    ppid: i64,
    pgid: i64,
    ticks: i64,
}

/// Whether a pid's parent chain reaches the leader or the leader's group, memoised per reading.
// TS local-exec.ts:138-154
fn reaches(rows: &HashMap<i64, TreeRow>, leader: i64, in_tree: &mut HashMap<i64, bool>, pid: i64) -> bool {
    let mut path: HashSet<i64> = HashSet::new();
    let mut at = pid;
    let mut found = in_tree.get(&at).copied();
    while found.is_none() {
        let row = rows.get(&at);
        if at == leader || row.is_some_and(|r| r.pgid == leader) {
            found = Some(true);
        // A parent loop ends the walk: procps reads /proc pid by pid, so a pid reused mid-read can print one.
        } else if row.is_none() || at <= 1 || path.contains(&at) {
            found = Some(false);
        } else {
            path.insert(at);
            at = row.map_or(at, |r| r.ppid);
            found = in_tree.get(&at).copied();
        }
    }
    let found = found.unwrap_or(false);
    for p in path {
        in_tree.insert(p, found);
    }
    found
}

/// A reader of the turn's tree through this computer's ps, with its own memory; a ps that cannot be spawned or fails
/// answers `None`.
// TS local-exec.ts:120-176
// /** The work the turn's process tree has done, as this computer's ps reports it: the cumulative CPU of every process
//  * whose parent chain reaches the turn's leader, and of the leader's group, which holds the orphans the turn left. The
//  * group alone is not the tree: Claude Code starts every Bash call in a session of its own, so a build a tool runs is in
//  * no group the turn leads. ps counts only the processes still running, and no column of a Mac's ps carries the CPU of a
//  * child its parent has reaped, so a plain sum falls each time a compiler exits. Each reader keeps what every process of
//  * the tree was last read at and adds what each has done since, so the total never falls when one exits; a process born
//  * and reaped between two reads is not seen. A pid read lower than before is a new process on that number. Neither ps
//  * reports a process's I/O, so a local turn's reading is CPU alone where a guest's counts bytes moved too. Nothing here
//  * may reach the timer that calls it: node hands only five errnos to a spawn's async error path and throws the rest
//  * (EPERM where ps is out of reach, ENOMEM on a full box) straight out of execFile, so both roads answer with no
//  * reading, and a turn whose tree cannot be read is left on its stream alone. */
// export function treeWork(): TreeWorkReader {
//   let seen = new Map<number, number>();
//   let total = 0;
//   const add = (stdout: string, leader: number): number => {
//     const rows = new Map<number, { ppid: number; pgid: number; ticks: number }>();
//     for (const row of stdout.split("\n")) {
//       const [pid = "", ppid = "", pgid = "", time = ""] = row.trim().split(/\s+/);
//       if (pid !== "") rows.set(Number(pid), { ppid: Number(ppid), pgid: Number(pgid), ticks: cpuTicks(time) });
//     }
//     const inTree = new Map<number, boolean>();
//     const reaches = (pid: number): boolean => {
//       const path = new Set<number>();
//       let at = pid;
//       let found = inTree.get(at);
//       while (found === undefined) {
//         const row = rows.get(at);
//         if (at === leader || row?.pgid === leader) found = true;
//         // A parent loop ends the walk: procps reads /proc pid by pid, so a pid reused mid-read can print one.
//         else if (row === undefined || at <= 1 || path.has(at)) found = false;
//         else {
//           path.add(at);
//           at = row.ppid;
//           found = inTree.get(at);
//         }
//       }
//       for (const p of path) inTree.set(p, found);
//       return found;
//     };
//     const now = new Map<number, number>();
//     for (const [pid, { ticks }] of rows) {
//       if (!reaches(pid)) continue;
//       const before = seen.get(pid);
//       total += before === undefined || ticks < before ? ticks : ticks - before;
//       now.set(pid, ticks);
//     }
//     seen = now;
//     return total;
//   };
//   return (leader, then) => {
//     try {
//       execFile("ps", ["-eo", "pid=,ppid=,pgid=,time="], (err, stdout) => then(err === null ? add(stdout, leader) : undefined));
//     } catch {
//       then(undefined);
//     }
//   };
// }
pub fn tree_work() -> TreeWorkReader {
    let tally = Arc::new(Mutex::new(TreeTally::default()));
    Arc::new(move |leader: i32| -> LocalBoxFuture<'static, Option<i64>> {
        let tally = tally.clone();
        Box::pin(async move {
            let out = tokio::process::Command::new("ps").args(["-eo", "pid=,ppid=,pgid=,time="]).output().await.ok()?;
            if !out.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&out.stdout);
            Some(lock(&tally).add(&text, i64::from(leader)))
        })
    })
}

/// One process of a run's group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupRow {
    pub pid: i64,
    pub ppid: i64,
}

/// Every process of one group as `ps -axo pid=,ppid=,pgid=` reports it; `[]` where ps will not answer.
// TS local-exec.ts:186-203
// /** Every process of one group as ps reports it; nothing where ps will not answer, which ends nothing. */
// function groupRows(pgid: number): Promise<GroupRow[]> {
//   return new Promise(resolve => {
//     try {
//       execFile("ps", ["-axo", "pid=,ppid=,pgid="], (err, stdout) => {
//         if (err !== null) return resolve([]);
//         resolve(
//           stdout.split("\n").flatMap(line => {
//             const [pid, ppid, group] = line.trim().split(/\s+/).map(Number);
//             return group === pgid && Number.isSafeInteger(pid) && Number.isSafeInteger(ppid) ? [{ pid: pid!, ppid: ppid! }] : [];
//           }),
//         );
//       });
//     } catch {
//       resolve([]);
//     }
//   });
// }
pub async fn group_rows(pgid: i32) -> Vec<GroupRow> {
    let Ok(out) = tokio::process::Command::new("ps").args(["-axo", "pid=,ppid=,pgid="]).output().await else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let (Some(pid), Some(ppid), Some(group)) = (words.next(), words.next(), words.next()) else { return None };
            let (Ok(pid), Ok(ppid), Ok(group)) = (pid.parse::<i64>(), ppid.parse::<i64>(), group.parse::<i64>()) else { return None };
            (group == i64::from(pgid)).then_some(GroupRow { pid, ppid })
        })
        .collect()
}

/// The group's members whose parent is not in it, the leader aside, in row order.
// TS local-exec.ts:205-206
// /** The group's processes whose parent is not in it, the leader aside: one whose parent went is the system's now. */
// const orphansOf = (rows: readonly GroupRow[], leader: number): number[] => rows.filter(r => r.pid !== leader && !rows.some(o => o.pid === r.ppid)).map(r => r.pid);
pub fn orphans_of(rows: &[GroupRow], leader: i64) -> Vec<i64> {
    rows.iter().filter(|r| r.pid != leader && !rows.iter().any(|o| o.pid == r.ppid)).map(|r| r.pid).collect()
}

/// The input pump's two pids, as the pump wrote them into `base.pump`: positive safe integers, one a line.
// TS local-exec.ts:208-211
// /** The input pump's two processes, tail and the read loop, as the pump wrote them before it carried a message. */
// function pumpOf(base: string): Set<number> {
//   return new Set((readFile(`${base}.pump`) ?? "").split("\n").map(Number).filter(pid => Number.isSafeInteger(pid) && pid > 0));
// }
pub fn pump_of(base: &str) -> IndexSet<i64> {
    read_file(&format!("{base}.pump"))
        .unwrap_or_default()
        .split('\n')
        .filter_map(|line| line.trim().parse::<i64>().ok())
        .filter(|&pid| pid > 0 && pid <= MAX_SAFE_INTEGER)
        .collect()
}

/// What a turn left running in its group: every orphan that is not the pump, and every process under one.
// TS local-exec.ts:213-226
// /** What a turn left running in its run's group: every orphan that is not the input pump, and every process under one. */
// function leftovers(rows: readonly GroupRow[], leader: number, pump: ReadonlySet<number>): number[] {
//   const left = new Set(orphansOf(rows, leader).filter(pid => !pump.has(pid)));
//   for (let grew = true; grew; ) {
//     grew = false;
//     for (const r of rows) {
//       if (left.has(r.ppid) && !left.has(r.pid)) {
//         left.add(r.pid);
//         grew = true;
//       }
//     }
//   }
//   return [...left];
// }
pub fn leftovers(rows: &[GroupRow], leader: i64, pump: &IndexSet<i64>) -> Vec<i64> {
    let mut left: IndexSet<i64> = orphans_of(rows, leader).into_iter().filter(|pid| !pump.contains(pid)).collect();
    let mut grew = true;
    while grew {
        grew = false;
        for r in rows {
            if left.contains(&r.ppid) && !left.contains(&r.pid) {
                left.insert(r.pid);
                grew = true;
            }
        }
    }
    left.into_iter().collect()
}

/// The pid the run's agent wrote into `run.agent` as it started, while it lives.
// TS local-exec.ts:248-254
// /** The pid the run's agent wrote as it started (agentWord in @wsp/protocol), while it lives; none before it starts or
//  * on a run whose command starts no agent. Read for a running turn alone, and taken by the pane only under the turn's
//  * own root, so a number the kernel handed on after the agent went names nothing. */
// export function agentPidOf(run: string): number | undefined {
//   const pid = Number(readFile(`${run}.agent`)?.trim() ?? "");
//   return Number.isSafeInteger(pid) && pid > 0 && alive(pid) ? pid : undefined;
// }
pub fn agent_pid_of(run: &str) -> Option<i32> {
    pid_in(&format!("{run}.agent")).filter(|&pid| alive(pid))
}

/// A pid a run's file names, read as node reads it (`Number(text.trim())`): a positive safe integer, else none.
pub(super) fn pid_in(path: &str) -> Option<i32> {
    let text = read_file(path)?;
    let pid = to_number(text.trim());
    if pid.fract() != 0.0 || pid <= 0.0 || pid > MAX_SAFE_INTEGER as f64 {
        return None;
    }
    i32::try_from(pid as i64).ok()
}

/// node's words for a failed call: `<CODE>: <what>, <syscall> '<path>'`, or with no path for a write, as node words it.
pub(super) fn node_error(e: &io::Error, syscall: &str, path: Option<&str>) -> String {
    let (code, what) = match e.raw_os_error() {
        Some(libc::EEXIST) => ("EEXIST", "file already exists"),
        Some(libc::ENOENT) => ("ENOENT", "no such file or directory"),
        Some(libc::EACCES) => ("EACCES", "permission denied"),
        Some(libc::EPERM) => ("EPERM", "operation not permitted"),
        Some(libc::ENOTDIR) => ("ENOTDIR", "not a directory"),
        Some(libc::EISDIR) => ("EISDIR", "illegal operation on a directory"),
        Some(libc::ENOTEMPTY) => ("ENOTEMPTY", "directory not empty"),
        Some(libc::ENOSPC) => ("ENOSPC", "no space left on device"),
        Some(libc::EROFS) => ("EROFS", "read-only file system"),
        Some(libc::EXDEV) => ("EXDEV", "cross-device link not permitted"),
        Some(libc::ELOOP) => ("ELOOP", "too many symbolic links encountered"),
        Some(libc::EIO) => ("EIO", "i/o error"),
        _ => {
            let text = e.to_string();
            let text = text.split(" (os error").next().unwrap_or_default();
            return match path {
                Some(path) => format!("{text}, {syscall} '{path}'"),
                None => format!("{text}, {syscall}"),
            };
        }
    };
    let at = path.map_or(String::new(), |path| format!(" '{path}'"));
    format!("{code}: {what}, {syscall}{at}")
}

/// ps's elapsed time `[[dd-]hh:]mm:ss` as the start it implies, `now_ms` less the span; `None` for anything else.
// TS local-exec.ts:272-279
//     const parse = (out: string): number | undefined => {
//       const said = out.trim();
//       const [span = "", days = "0"] = said.split("-").reverse();
//       const parts = span.split(":").map(Number);
//       if (parts.length < 2 || parts.some(n => !Number.isFinite(n))) return undefined;
//       const [hours = 0, minutes = 0, seconds = 0] = parts.length === 3 ? parts : [0, ...parts];
//       return Date.now() - (((Number(days) * 24 + hours) * 60 + minutes) * 60 + seconds) * 1_000;
//     };
pub fn parse_etime(out: &str, now_ms: i64) -> Option<i64> {
    let said = out.trim();
    let pieces: Vec<&str> = said.split('-').collect();
    let span = pieces.last().copied().unwrap_or("");
    let days = if pieces.len() >= 2 { pieces[pieces.len() - 2] } else { "0" };
    let parts: Vec<f64> = span.split(':').map(to_number).collect();
    if parts.len() < 2 || parts.iter().any(|n| !n.is_finite()) {
        return None;
    }
    let (hours, minutes, seconds) = if parts.len() == 3 { (parts[0], parts[1], parts[2]) } else { (0.0, parts[0], parts[1]) };
    let span_ms = (((to_number(days) * 24.0 + hours) * 60.0 + minutes) * 60.0 + seconds) * 1_000.0;
    let at = now_ms as f64 - span_ms;
    (!at.is_nan()).then_some(at as i64)
}

/// When a process started, by `ps -o etime= -p <pid>`; `None` where ps will not answer.
// TS local-exec.ts:267-286
// /** When a process started, in milliseconds on this computer's clock, off the elapsed time ps prints: `[[dd-]hh:]mm:ss`
//  * on both computers wsp runs a local turn on. Nothing here may reach the caller's error road, so a ps that will not
//  * answer is no reading rather than a throw. */
// function startedAt(pid: number): Promise<number | undefined> {
//   return new Promise(resolve => {
//     const parse = (out: string): number | undefined => {
//       const said = out.trim();
//       const [span = "", days = "0"] = said.split("-").reverse();
//       const parts = span.split(":").map(Number);
//       if (parts.length < 2 || parts.some(n => !Number.isFinite(n))) return undefined;
//       const [hours = 0, minutes = 0, seconds = 0] = parts.length === 3 ? parts : [0, ...parts];
//       return Date.now() - (((Number(days) * 24 + hours) * 60 + minutes) * 60 + seconds) * 1_000;
//     };
//     try {
//       execFile("ps", ["-o", "etime=", "-p", String(pid)], (err, stdout) => resolve(err === null ? parse(stdout) : undefined));
//     } catch {
//       resolve(undefined);
//     }
//   });
// }
pub async fn started_at(pid: i32) -> Option<i64> {
    let out = tokio::process::Command::new("ps").args(["-o", "etime=", "-p", &pid.to_string()]).output().await.ok()?;
    if !out.status.success() {
        return None;
    }
    parse_etime(&String::from_utf8_lossy(&out.stdout), RealClock.now())
}

/// Whether the pid a run recorded still leads that run's own group.
// TS local-exec.ts:288-313
// /** How far a leader's own age may sit from the claim's before it is read as somebody else's process. The launch
//  * makes the claim and spawns within the same moment, so this is slack for a busy computer and nothing more; a pid
//  * this computer handed out again is younger than its claim by however long the run had been going. */
// const LEADER_WINDOW_MS = 5_000;
//
// /** Whether the pid a run recorded still leads that run's own group, which is what a reap must know before it
//  * signals anything. A pid is free again only once its group has no members left, so a group that still answers is
//  * the one this run made and its stragglers are the turn's. The one hole that leaves is a pid handed out again
//  * after the whole group went, to a process that then led a group of its own; the leader's own age closes it, since
//  * the launch claims and spawns in the same moment. A ps that will not answer leaves the group's own reading as the
//  * whole of it, which is what this had before the age was read at all. */
// async function leadsThisRun(base: string, pid: number): Promise<boolean> {
//   if (!groupExists(pid)) return false;
//   // The leader is gone and its children hold the group open: the kernel keeps that number while they do.
//   if (!alive(pid)) return true;
//   const started = await startedAt(pid);
//   if (started === undefined) return true;
//   const claimed = ((): number | undefined => {
//     try {
//       return statSync(`${base}.d`).mtimeMs;
//     } catch {
//       return undefined;
//     }
//   })();
//   return claimed === undefined || Math.abs(started - claimed) <= LEADER_WINDOW_MS;
// }
pub async fn leads_this_run(base: &str, pid: i32) -> bool {
    if !group_exists(pid) {
        return false;
    }
    if !alive(pid) {
        return true;
    }
    let Some(started) = started_at(pid).await else { return true };
    match claimed_at(base) {
        None => true,
        Some(claimed) => (started as f64 - claimed as f64).abs() <= LEADER_WINDOW_MS as f64,
    }
}

/// The claim folder's modification time in epoch ms, `None` where it cannot be read.
fn claimed_at(base: &str) -> Option<i64> {
    let modified = fs::metadata(format!("{base}.d")).ok()?.modified().ok()?;
    modified.duration_since(UNIX_EPOCH).ok().map(|d| d.as_millis() as i64)
}

/// What a reader takes once the group is gone, up to the log's size at that moment.
pub type LastRead = Box<dyn FnOnce(u64) -> LocalBoxFuture<'static, ()>>;

/// Ends a run's group (TERM, the grace in 200 ms steps while the group exists, KILL) when its recorded pid leads it,
/// runs `last_read`, then deletes its files and its claim. `grace_ms` is `RUN_STOP_MS` at every TS call site. A removal
/// node's `rmSync` would throw comes back as node's words, and the files after it are left as they are.
// TS local-exec.ts:315-333
// /** Everything one run left on this computer, ended and taken away: the run's own process group gets TERM, then
//  * KILL once the stop grace passes, and the run's files go. The group and never the leader alone, at every ending
//  * including the leader's own exit, since the children a harness leaves behind are what hold a machine's memory for
//  * its life. A group that is not this run's is left alone and only its files go. `lastRead` runs once the group is
//  * gone and before the log goes, so a reader takes what was printed after its last poll, up to the size the log had at
//  * the KILL: a process that left the group and still holds the log would otherwise keep the read, and the stop, going. */
// async function reapRun(base: string, lastRead: (end: number) => Promise<void> = async () => {}, graceMs = RUN_STOP_MS): Promise<void> {
//   const pid = Number(readFile(`${base}.pid`)?.trim() ?? "");
//   if (Number.isSafeInteger(pid) && pid > 0 && (await leadsThisRun(base, pid))) {
//     signalGroup(pid, "SIGTERM");
//     // The grace is the group's, not the leader's: a leader that goes at once on the TERM leaves the children it
//     // started to take the whole of it, which is the stop every road here promises them.
//     for (let waited = 0; waited < graceMs && groupExists(pid); waited += GRACE_POLL_MS) await sleep(Math.min(GRACE_POLL_MS, graceMs - waited));
//     signalGroup(pid, "SIGKILL");
//   }
//   await lastRead(logSize(base));
//   for (const suffix of ["sh", "log", "pid", "agent", "exit", "in", "held", "late", "fifo", "tail", "pump"]) rmSync(`${base}.${suffix}`, { force: true });
//   rmSync(`${base}.d`, { recursive: true, force: true });
// }
pub async fn reap_run(base: &str, last_read: Option<LastRead>, grace_ms: u64) -> Result<(), String> {
    if let Some(pid) = pid_in(&format!("{base}.pid")) {
        if leads_this_run(base, pid).await {
            signal_group(pid, libc::SIGTERM);
            let mut waited = 0;
            while waited < grace_ms && group_exists(pid) {
                tokio::time::sleep(Duration::from_millis(GRACE_POLL_MS.min(grace_ms - waited))).await;
                waited += GRACE_POLL_MS;
            }
            signal_group(pid, libc::SIGKILL);
        }
    }
    if let Some(last_read) = last_read {
        last_read(log_size(base)).await;
    }
    for suffix in REAPED_SUFFIXES {
        remove_forced(&format!("{base}.{suffix}"))?;
    }
    remove_tree(&format!("{base}.d"))
}

/// `rmSync(path, { force: true })`: an absent file is no error, any other failure is node's words.
fn remove_forced(path: &str) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(node_error(&e, "unlink", Some(path))),
    }
}

/// `rmSync(path, { recursive: true, force: true })`: a folder is removed as node does, empty first and then its children.
/// One stack of folders of its own, each holding the names `readdirSync` would answer, so no depth of folders is a depth of
/// the thread's stack or a descriptor held open.
fn remove_tree(path: &str) -> Result<(), String> {
    let mut open: Vec<(String, std::vec::IntoIter<std::fs::DirEntry>)> = Vec::new();
    let mut next = Some(path.to_string());
    loop {
        if let Some(path) = next.take() {
            if let Some(entries) = enter(&path)? {
                open.push((path, entries.into_iter()));
            }
        }
        let Some((here, entries)) = open.last_mut() else { return Ok(()) };
        match entries.next() {
            Some(entry) => next = Some(entry.path().to_string_lossy().into_owned()),
            None => {
                fs::remove_dir(&*here).map_err(|e| node_error(&e, "rmdir", Some(here)))?;
                open.pop();
            }
        }
    }
}

/// One step of `remove_tree` at `path`: a file or an empty folder is removed here; a folder with something in it answers
/// what is in it, for the caller to remove first.
fn enter(path: &str) -> Result<Option<Vec<std::fs::DirEntry>>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(node_error(&e, "lstat", Some(path))),
    };
    if !meta.is_dir() {
        return remove_forced(path).map(|()| None);
    }
    match fs::remove_dir(path) {
        Ok(()) => return Ok(None),
        Err(e) if matches!(e.raw_os_error(), Some(libc::ENOTEMPTY | libc::EEXIST)) => {}
        Err(e) => return Err(node_error(&e, "rmdir", Some(path))),
    }
    let entries = fs::read_dir(path).map_err(|e| node_error(&e, "scandir", Some(path)))?;
    let names = entries.collect::<io::Result<Vec<_>>>().map_err(|e| node_error(&e, "scandir", Some(path)))?;
    Ok(Some(names))
}

/// The log's size, 0 when it cannot be read.
// TS local-exec.ts:335-341
// function logSize(base: string): number {
//   try {
//     return statSync(`${base}.log`).size;
//   } catch {
//     return 0;
//   }
// }
pub fn log_size(base: &str) -> u64 {
    fs::metadata(format!("{base}.log")).map(|m| m.len()).unwrap_or(0)
}

/// Hands a run the seed its launch held back, into a channel nothing reached yet, then drops it.
// TS local-exec.ts:343-350
// /** Hands a run the seed its launch held back: into the channel where nothing reached it yet, then dropped. The
//  * channel's size is the check, so a host that went between the two steps leaves no road to a second copy. */
// function releaseHeld(base: string): void {
//   const held = readFile(`${base}.held`);
//   if (held === undefined) return;
//   if (existsSync(`${base}.d`) && existsSync(`${base}.in`) && statSync(`${base}.in`).size === 0) appendFileSync(`${base}.in`, held);
//   rmSync(`${base}.held`, { force: true });
// }
pub fn release_held(base: &str) {
    let _ = try_release_held(base);
}

/// `release_held` where a failed append is the caller's to read, with the seed left held: the write road answers `Gone`
/// on it, and an attach fails, as the TS does.
pub(super) fn try_release_held(base: &str) -> io::Result<()> {
    let Some(held) = read_file(&format!("{base}.held")) else { return Ok(()) };
    let input = format!("{base}.in");
    let empty = fs::metadata(&input).is_ok_and(|m| m.len() == 0);
    if Path::new(&format!("{base}.d")).exists() && empty {
        append(&input, held.as_bytes())?;
    }
    let _ = fs::remove_file(format!("{base}.held"));
    Ok(())
}

/// A late line goes in once, by its key, after the seed, onto a run that has not ended.
// TS local-exec.ts:352-366
// /** A line written after the seed and held until it is due goes in once, after the seed, onto a run that has not ended.
//  * Its key says whether it went, never its words: a thread may send one agent the same words twice, and a host that went
//  * between the append and the drop left the key it appended under beside the channel. */
// function releaseLate(base: string): void {
//   releaseHeld(base);
//   const late = readFile(`${base}.late`);
//   if (late === undefined) return;
//   const key = readFile(`${base}.late.key`);
//   if (existsSync(`${base}.d`) && !existsSync(`${base}.exit`) && key !== readFile(`${base}.late.went`)) {
//     appendFileSync(`${base}.in`, late);
//     writeFileSync(`${base}.late.went`, key ?? "", { mode: OWNER_FILE });
//   }
//   rmSync(`${base}.late`, { force: true });
//   rmSync(`${base}.late.key`, { force: true });
// }
pub fn release_late(base: &str) {
    let _ = try_release_late(base);
}

/// `release_late` where a failed append is the caller's to read: the line stays parked and is not marked as gone.
pub(super) fn try_release_late(base: &str) -> Result<(), String> {
    try_release_held(base).map_err(|e| node_error(&e, "write", None))?;
    let Some(late) = read_file(&format!("{base}.late")) else { return Ok(()) };
    let key = read_file(&format!("{base}.late.key"));
    let went = format!("{base}.late.went");
    if Path::new(&format!("{base}.d")).exists() && !Path::new(&format!("{base}.exit")).exists() && key != read_file(&went) {
        append(&format!("{base}.in"), late.as_bytes()).map_err(|e| node_error(&e, "write", None))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&went)
            .map_err(|e| node_error(&e, "open", Some(&went)))?;
        file.write_all(key.unwrap_or_default().as_bytes()).map_err(|e| node_error(&e, "write", None))?;
    }
    let _ = fs::remove_file(format!("{base}.late"));
    let _ = fs::remove_file(format!("{base}.late.key"));
    Ok(())
}

/// The line that ends one run's input channel, from the run's own name: `__WSP_EOF_<last 12 of base>__`.
// TS local-exec.ts:79-83
// /** The line that ends one run's input channel. The pump forwards every line written before it and then goes, so the
//  * command reads EOF after the last message and never in front of one; ending the pump by a signal instead took the
//  * message a person had just sent with it. Derived from the run's own name, so a host that attaches to a run an
//  * earlier one launched closes the same channel without being told anything. */
// const endMarker = (base: string): string => `__WSP_EOF_${base.slice(-12)}__`;
pub fn end_marker(base: &str) -> String {
    let chars: Vec<char> = base.chars().collect();
    let tail: String = chars[chars.len().saturating_sub(12)..].iter().collect();
    format!("__WSP_EOF_{tail}__")
}

/// One complete line at a time out of a growing byte stream, queued whether or not anybody iterates yet; a reader
/// that lets go takes the queue with it. Clones share one queue.
// TS local-exec.ts:368-435
// /** One complete line at a time out of a growing byte stream: what precedes each newline is yielded, the tail waits
//  * for more, and the final tail with no newline is yielded when the stream closes. The queue fills whether or not
//  * anybody is iterating yet, so a turn's ending is the run's own and not its reader's; a consumer that lets go takes
//  * the queue with it, since nothing would read it again. */
// class Lines {
//   private queue: string[] = [];
//   private split = new LineCut();
//   private wake: (() => void) | undefined;
//   private done = false;
//   private letGo = false;
//   private error: Error | undefined;
//
//   feed(chunk: Buffer): void {
//     if (this.letGo) return;
//     for (const line of this.split.take(chunk)) this.queue.push(line);
//     this.wake?.();
//     this.wake = undefined;
//   }
//
//   /** Ends the stream with an error once everything already read is out: what a cut turn's reader sees last. */
//   fail(error: Error): void {
//     this.error = error;
//     this.end();
//   }
//
//   end(): void {
//     const last = this.split.rest();
//     if (last !== undefined && !this.letGo) this.queue.push(last);
//     this.done = true;
//     this.wake?.();
//     this.wake = undefined;
//   }
//
//   /** The stream's one reader. A return() before its first next() never starts a generator, so its finally never runs:
//    * the reader lets go on return() too. */
//   iterate(): AsyncGenerator<string> {
//     const read = this.read();
//     const close = read.return.bind(read);
//     read.return = value => {
//       this.release();
//       return close(value);
//     };
//     return read;
//   }
//
//   private release(): void {
//     this.letGo = true;
//     this.queue = [];
//     this.split = new LineCut();
//   }
//
//   private async *read(): AsyncGenerator<string> {
//     try {
//       while (true) {
//         while (this.queue.length > 0) yield this.queue.shift()!;
//         if (this.done) {
//           if (this.error !== undefined) throw this.error;
//           return;
//         }
//         await new Promise<void>(resolve => {
//           this.wake = resolve;
//         });
//       }
//     } finally {
//       this.release();
//     }
//   }
// }
#[derive(Clone)]
pub struct Lines {
    inner: Arc<Mutex<LinesState>>,
}

#[derive(Default)]
struct LinesState {
    queue: VecDeque<String>,
    split: LineCut,
    wake: Option<Waker>,
    done: bool,
    let_go: bool,
    taken: bool,
    error: Option<ExecFailure>,
}

impl Lines {
    pub fn new() -> Self {
        Lines { inner: Arc::new(Mutex::new(LinesState::default())) }
    }
    pub fn feed(&self, chunk: &[u8]) {
        let mut guard = lock(&self.inner);
        let st = &mut *guard;
        if st.let_go {
            return;
        }
        let lines = st.split.take(chunk);
        st.queue.extend(lines);
        if let Some(wake) = st.wake.take() {
            wake.wake();
        }
    }
    /// Ends the stream with `error` once everything already queued is out.
    pub fn fail(&self, error: ExecFailure) {
        lock(&self.inner).error = Some(error);
        self.end();
    }
    pub fn end(&self) {
        let mut guard = lock(&self.inner);
        let st = &mut *guard;
        let last = st.split.rest();
        if let Some(last) = last {
            if !st.let_go {
                st.queue.push_back(last);
            }
        }
        st.done = true;
        if let Some(wake) = st.wake.take() {
            wake.wake();
        }
    }
    /// The stream's one reader; dropping it lets go (the queue empties and later feeds are dropped). A second call
    /// answers an empty stream at once.
    pub fn iterate(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        let mut st = lock(&self.inner);
        if std::mem::replace(&mut st.taken, true) {
            return Box::pin(futures::stream::empty::<Result<String, ExecFailure>>());
        }
        drop(st);
        Box::pin(LinesReader { inner: self.inner.clone(), finished: false })
    }
}

struct LinesReader {
    inner: Arc<Mutex<LinesState>>,
    finished: bool,
}

impl Stream for LinesReader {
    type Item = Result<String, ExecFailure>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }
        let inner = self.inner.clone();
        let mut guard = lock(&inner);
        let st = &mut *guard;
        if let Some(line) = st.queue.pop_front() {
            return Poll::Ready(Some(Ok(line)));
        }
        if st.done {
            self.finished = true;
            return Poll::Ready(st.error.clone().map(Err));
        }
        st.wake = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl Drop for LinesReader {
    fn drop(&mut self) {
        let mut guard = lock(&self.inner);
        guard.let_go = true;
        guard.queue.clear();
        guard.split = LineCut::default();
    }
}

impl Default for Lines {
    fn default() -> Self {
        Self::new()
    }
}
