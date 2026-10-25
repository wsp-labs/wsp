// SPDX-License-Identifier: AGPL-3.0-only
//! The process side of a turn on this computer: the tree's CPU through ps, what a kept turn left in its group,
//! whether a recorded pid still leads its run's group, the reap of a run's group and files, the held seed and late
//! line hand-overs, and the queue a local stream's lines wait in. Port of packages/runtime/src/local-exec.ts:38-435
//! at 65f6b89c7. Spec: spec/local_exec_reap.md.
use std::collections::HashMap;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use indexmap::IndexSet;
use wsp_adapters::port::ExecFailure;

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
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
        todo!()
    }
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
    todo!()
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
    todo!()
}

/// The group's members whose parent is not in it, the leader aside, in row order.
// TS local-exec.ts:205-206
// /** The group's processes whose parent is not in it, the leader aside: one whose parent went is the system's now. */
// const orphansOf = (rows: readonly GroupRow[], leader: number): number[] => rows.filter(r => r.pid !== leader && !rows.some(o => o.pid === r.ppid)).map(r => r.pid);
pub fn orphans_of(rows: &[GroupRow], leader: i64) -> Vec<i64> {
    todo!()
}

/// The input pump's two pids, as the pump wrote them into `base.pump`: positive safe integers, one a line.
// TS local-exec.ts:208-211
// /** The input pump's two processes, tail and the read loop, as the pump wrote them before it carried a message. */
// function pumpOf(base: string): Set<number> {
//   return new Set((readFile(`${base}.pump`) ?? "").split("\n").map(Number).filter(pid => Number.isSafeInteger(pid) && pid > 0));
// }
pub fn pump_of(base: &str) -> IndexSet<i64> {
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
}

/// What a reader takes once the group is gone, up to the log's size at that moment.
pub type LastRead = Box<dyn FnOnce(u64) -> LocalBoxFuture<'static, ()>>;

/// Ends a run's group (TERM, the grace in 200 ms steps while the group exists, KILL) when its recorded pid leads it,
/// runs `last_read`, then deletes its files and its claim. `grace_ms` is `RUN_STOP_MS` at every TS call site.
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
pub async fn reap_run(base: &str, last_read: Option<LastRead>, grace_ms: u64) {
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
}

/// The line that ends one run's input channel, from the run's own name: `__WSP_EOF_<last 12 of base>__`.
// TS local-exec.ts:79-83
// /** The line that ends one run's input channel. The pump forwards every line written before it and then goes, so the
//  * command reads EOF after the last message and never in front of one; ending the pump by a signal instead took the
//  * message a person had just sent with it. Derived from the run's own name, so a host that attaches to a run an
//  * earlier one launched closes the same channel without being told anything. */
// const endMarker = (base: string): string => `__WSP_EOF_${base.slice(-12)}__`;
pub fn end_marker(base: &str) -> String {
    todo!()
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
    inner: Arc<std::sync::Mutex<()>>,
}

impl Lines {
    pub fn new() -> Self {
        todo!()
    }
    pub fn feed(&self, chunk: &[u8]) {
        todo!()
    }
    /// Ends the stream with `error` once everything already queued is out.
    pub fn fail(&self, error: ExecFailure) {
        todo!()
    }
    pub fn end(&self) {
        todo!()
    }
    /// The stream's one reader; dropping it lets go (the queue empties and later feeds are dropped).
    pub fn iterate(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        todo!()
    }
}

impl Default for Lines {
    fn default() -> Self {
        Self::new()
    }
}
