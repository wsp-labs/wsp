// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_table: slate-runs.ts:140-163, 210-242, 613-983 at 65f6b89c7. Spec: spec/runs_table.md.
//!
//! State is `RunsInner` behind one `Rc`, each field in a `RefCell`; no borrow is held across a call to `deps`, a
//! road, a kill or a release (rule 3.3): `on_record` re-enters the slate area, which may call `start` again.
//! The traits, `create_slate_runs` and the public types above `HeldFor` are fixed; `HeldFor`, `Live`, `Timer`,
//! `ThreadRuns`, `RunsInner` and its methods are the TS's closure state written out, and the implementer may reshape
//! them.
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::clock::{Cancel, Clock};

use super::parts::{CmdRunDecl, Reshape, RunAsk, RunRecord, RunResult, RunStart, RunStartAnswer};
use super::road::{RunRoad, Stream};
use super::secrets::{RunApprovals, Secrets, SlateSecrets};

// slate-runs.ts:140
// export interface SlateRunsDeps {
//   /** The person's login environment on this computer. Every WSP_ variable is dropped from it before a run sees it. */
//   env: () => Readonly<Record<string, string>>;
//   /** Every change of a run's record, scrubbed: the host writes it as the run's value in one batch. */
//   onRecord: (threadId: string, run: string, record: RunRecord) => void;
//   /** Each line a `stream` run prints, scrubbed, as it is printed. */
//   onLine?: (threadId: string, run: string, line: string, stream: "out" | "err") => void;
//   /** A timer wants its run started; the host starts it with `by: "timer"` through its batch. */
//   onTimer?: (threadId: string, run: string) => void;
//   approvals?: RunApprovals;
//   /** `slates.secrets.json` beside the state file, for secrets declared `keep`. Unset keeps nothing on disk. */
//   secretsFile?: string;
//   /** The slate's own folder with its files written, which every command and then reads as SLATE_DIR. */
//   dir?: (threadId: string) => string;
//   /** Where a run of this thread runs when that is not this computer: the thread's own machine, for a run `on` the
//    * thread whose thread is on a box. Unset, or answering nothing, runs it here. */
//   road?: (threadId: string, on: "thread" | "host") => RunRoad | undefined;
//   /** Names a command's process on this computer as the thread's slate run until the returned release. */
//   own?: (threadId: string, pid: number, kind: "slate") => () => void;
//   /** Why a run on="host" of this thread does not start, a stored one from a thread now off this computer: said as it
//    * starts, before anything asks the person. */
//   hostRefused?: (threadId: string, run: string) => string | undefined;
//   now?: () => number;
// }
pub trait SlateRunsDeps {
    /// The person's login environment on this computer.
    fn env(&self) -> IndexMap<String, String>;
    /// Every change of a run's record, scrubbed.
    fn on_record(&self, thread_id: &str, run: &str, record: RunRecord);
    /// Each line a `stream` run prints, scrubbed.
    fn on_line(&self, _thread_id: &str, _run: &str, _line: &str, _stream: Stream) {}
    /// A timer wants its run started.
    fn on_timer(&self, _thread_id: &str, _run: &str) {}
    /// None keeps approvals in memory (`DefaultApprovals`).
    fn approvals(&self) -> Option<Rc<dyn RunApprovals>> {
        None
    }
    /// `slates.secrets.json`; none keeps nothing on disk.
    fn secrets_file(&self) -> Option<PathBuf> {
        None
    }
    /// The slate's own folder with its files written: none when the host gives no folder; `Err` with the message
    /// when the files could not be written.
    fn dir(&self, _thread_id: &str) -> Option<Result<String, String>> {
        None
    }
    /// Where a run of this thread runs when that is not this computer.
    fn road(&self, _thread_id: &str, _on: &str) -> Option<Rc<dyn RunRoad>> {
        None
    }
    /// Names a command's process as the thread's slate run until the answer is called.
    fn own(&self, _thread_id: &str, _pid: u32, _kind: &str) -> Option<Rc<dyn Fn()>> {
        None
    }
    /// Why a run `on: host` of this thread does not start.
    fn host_refused(&self, _thread_id: &str, _run: &str) -> Option<String> {
        None
    }
    fn clock(&self) -> Rc<dyn Clock>;
}

/// One declared timer.
#[derive(Clone, Debug, PartialEq)]
pub struct TimerDecl {
    pub run: String,
    /// Seconds.
    pub every: f64,
    pub key: String,
    pub always: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApproveScope {
    Once,
    Always,
}

// slate-runs.ts:210
// export interface SlateRuns {
//   /** The approval key: kind, text, env names and the expressions filling them, args and stdin expressions, on,
//    * cwd, stream, every, always, timeout, once, then, the slate files it reads and the hashes of the scripts it names.
//    * A changed value is the same key; a changed declaration is a new one. */
//   key(decl: CmdRunDecl): string;
//   start(req: RunStart): RunStartAnswer;
//   /** The person's answer to a held run: "Run once" or "Always in this thread" starts it now. */
//   approve(threadId: string, run: string, scope: "once" | "always"): RunStartAnswer | undefined;
//   /** "Not now": the held run ends cancelled, so its `done` reactions fire. */
//   deny(threadId: string, run: string): void;
//   revoke(threadId: string, key: string): void;
//   allowed(threadId: string): string[];
//   /** The held runs of a thread with what their sheets show, for a tab that opens on them. */
//   held(threadId: string): RunAsk[];
//   cancel(threadId: string, run: string): void;
//   /** Every run of the thread killed. `quiet` drops their completions and writes nothing, for a rewind, whose
//    * restored values say what the runs were. */
//   stopAll(threadId: string, opts?: { quiet?: boolean; why?: string }): void;
//   /** The declared timers of a slate; replaces what it had. A timer whose key, every and always are unchanged keeps
//    * its period; a new or changed one fires at once. */
//   timers(threadId: string, timers: { run: string; every: number; key: string; always?: boolean }[]): void;
//   /** Whether any window shows the slate. Shown-only timers tick only while it is; `always` ones regardless. */
//   shown(threadId: string, shown: boolean): void;
//   /** The person pressed, or the agent wrote: a run held by the start budget may start again. */
//   release(threadId: string): void;
//   /** A run's `then`: the literal command on this computer with the raw result, already scrubbed, on its stdin. */
//   reshape(threadId: string, cmd: string, input: string, folder: string, timeoutS?: number): Reshape;
//   secrets: SlateSecrets;
//   /** The thread is gone: its runs, timers, approvals in memory and secrets with it. */
//   /** Forgets the thread's runs; with `keep`, those running are left running and only its timers stop. */
//   drop(threadId: string, keep?: boolean): void;
//   close(): void;
// }
pub trait SlateRuns {
    fn key(&self, decl: &CmdRunDecl) -> String;
    fn start(&self, req: RunStart) -> RunStartAnswer;
    fn approve(&self, thread_id: &str, run: &str, scope: ApproveScope) -> Option<RunStartAnswer>;
    fn deny(&self, thread_id: &str, run: &str);
    fn revoke(&self, thread_id: &str, key: &str);
    fn allowed(&self, thread_id: &str) -> Vec<String>;
    fn held(&self, thread_id: &str) -> Vec<RunAsk>;
    fn cancel(&self, thread_id: &str, run: &str);
    /// `quiet` drops the completions and writes nothing; else each records `why` (or `cancelled`).
    fn stop_all(&self, thread_id: &str, quiet: bool, why: Option<&str>);
    fn timers(&self, thread_id: &str, list: Vec<TimerDecl>);
    fn shown(&self, thread_id: &str, shown: bool);
    fn release(&self, thread_id: &str);
    /// `timeout_s` none is 60.
    fn reshape(&self, thread_id: &str, cmd: &str, input: &str, folder: &str, timeout_s: Option<f64>) -> Reshape;
    fn secrets(&self) -> Rc<dyn SlateSecrets>;
    /// The TS `drop`.
    fn drop_thread(&self, thread_id: &str, keep: bool);
    fn close(&self);
}

// slate-runs.ts:415
// type HeldFor = "approval" | "busy" | "budget" | "pressed";
//
// interface Live {
//   record: RunRecord;
//   /** Bumped by every start and cancel, so a completion of an instance nobody is waiting for any more is dropped. */
//   gen: number;
//   /** The last result, kept through holds so the next start carries it. */
//   result?: RunResult;
//   /** Ends the running command, its whole group, on whichever computer it runs. */
//   kill?: () => void;
//   /** The run's `then`, while it reshapes the result. */
//   reshaping?: Reshape;
//   held?: { req: RunStart; for: HeldFor };
//   /** When reactions and timers started it, for the start budget. */
//   starts: number[];
//   /** When the person started it, for their own budget, which no press releases. */
//   pressed?: number[];
// }
/// Why a start is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeldFor {
    Approval,
    Busy,
    Budget,
    Pressed,
}

/// One run's live entry.
pub(crate) struct Live {
    pub record: RunRecord,
    pub gen: u64,
    pub result: Option<RunResult>,
    pub kill: Option<Rc<dyn Fn()>>,
    pub reshaping: Option<Rc<dyn Fn()>>,
    pub held: Option<(RunStart, HeldFor)>,
    pub starts: Vec<i64>,
    pub pressed: Option<Vec<i64>>,
}

// slate-runs.ts:434
// interface Timer {
//   every: number;
//   key: string;
//   always: boolean;
//   handle?: ReturnType<typeof setInterval>;
//   /** Failed starts in a row, and when the last one ended. */
//   failures: number;
//   failedAt?: number;
// }
pub(crate) struct Timer {
    pub every: f64,
    pub key: String,
    pub always: bool,
    /// The armed interval's cancel.
    pub handle: Option<Cancel>,
    pub failures: i64,
    pub failed_at: Option<i64>,
}

// slate-runs.ts:444
// interface ThreadRuns {
//   runs: Map<string, Live>;
//   /** Runs held because four were running, in the order they asked. */
//   queue: string[];
//   timers: Map<string, Timer>;
//   shown: boolean;
// }
#[derive(Default)]
pub(crate) struct ThreadRuns {
    pub runs: IndexMap<String, Live>,
    pub queue: Vec<String>,
    pub timers: IndexMap<String, Timer>,
    pub shown: bool,
}

/// The table's state.
pub(crate) struct RunsInner {
    pub deps: Rc<dyn SlateRunsDeps>,
    pub approvals: Rc<dyn RunApprovals>,
    pub secrets: Rc<Secrets>,
    pub threads: RefCell<IndexMap<String, ThreadRuns>>,
    pub closed: Cell<bool>,
}

/// `createSlateRuns`' answer.
pub struct SlateRunsTable {
    pub(crate) inner: Rc<RunsInner>,
}

// slate-runs.ts:613
// export function createSlateRuns(deps: SlateRunsDeps): SlateRuns {
//   const now = deps.now ?? Date.now;
//   const approvals = deps.approvals ?? new DefaultApprovals();
//   const secrets = createSecrets(deps.secretsFile, now);
//   const threads = new Map<string, ThreadRuns>();
//
//   const thread = (threadId: string): ThreadRuns => {
//     let t = threads.get(threadId);
//     if (t === undefined) {
//       t = { runs: new Map(), queue: [], timers: new Map(), shown: false };
//       threads.set(threadId, t);
//     }
//     return t;
//   };
//   const live = (threadId: string, run: string, last?: RunRecord): Live => {
//     const t = thread(threadId);
//     let l = t.runs.get(run);
//     if (l === undefined) {
//       const result = last === undefined ? undefined : lastResult(last);
//       l = { record: { state: "idle", runs: last?.runs ?? 0 }, gen: 0, starts: [], ...(result !== undefined ? { result } : {}) };
//       t.runs.set(run, l);
//     }
//     return l;
//   };
//   const write = (threadId: string, run: string, l: Live, record: RunRecord): RunRecord => {
//     l.record = record;
//     const result = record.state === "running" ? undefined : lastResult(record);
//     if (result !== undefined) l.result = result;
//     deps.onRecord(threadId, run, record);
//     return record;
//   };
//   /** Throws where the slate's files could not be written. */
//   const loginEnv = (threadId: string): Record<string, string> => {
//     const env: Record<string, string> = {};
//     for (const [name, value] of Object.entries(deps.env())) if (!name.startsWith("WSP_")) env[name] = value;
//     if (deps.dir !== undefined) env["SLATE_DIR"] = deps.dir(threadId);
//     return env;
//   };
//   const reshape = (threadId: string, cmd: string, input: string, folder: string, timeoutS = DEFAULT_TIMEOUT_S): Reshape => {
//     let env: Record<string, string>;
//     try {
//       env = loginEnv(threadId);
//     } catch (e) {
//       return { done: Promise.resolve({ why: filesUnwritten(e), exit: null, err: "" }), kill: () => {} };
//     }
//     return reshapeResult({ cmd, input, cwd: existsSync(folder) ? folder : process.cwd(), env, timeoutS, scrub: text => secrets.scrub(threadId, text) });
//   };
//   const running = (t: ThreadRuns): number => [...t.runs.values()].filter(l => l.record.state === "running").length;
pub fn create_slate_runs(deps: Rc<dyn SlateRunsDeps>) -> Rc<dyn SlateRuns> {
    todo!()
}

// slate-runs.ts:670
//   const shown = (threadId: string, input: RunInput): string =>
//     "secret" in input ? (secrets.handle(threadId, input.secret).set ? SECRET_DOTS : `${SECRET_DOTS} (not filled)`) : input.value === null ? "(unset)" : asText(input.value);
//
//   const ask = (req: RunStart): RunAsk => {
//     const inputs = req.inputs();
//     const stdin = inputs.stdin === undefined ? undefined : shown(req.threadId, inputs.stdin).split("\n")[0];
//     return {
//       run: req.run,
//       key: key(req.decl),
//       cmd: req.decl.cmd,
//       env: Object.fromEntries(Object.entries(inputs.env ?? {}).map(([name, input]) => [name, shown(req.threadId, input)])),
//       args: (inputs.args ?? []).map(input => shown(req.threadId, input)),
//       ...(stdin !== undefined ? { stdin } : {}),
//       folder: req.decl.cwd === undefined ? req.folder : resolve(req.folder, req.decl.cwd),
//       on: req.decl.on ?? "thread",
//       timeout: timeoutOf(req.decl),
//       ...(req.decl.confirm !== undefined ? { confirm: req.decl.confirm } : {}),
//       ...(req.decl.then !== undefined ? { then: req.decl.then } : {}),
//       ...(req.decl.files !== undefined ? { files: req.decl.files } : {}),
//     };
//   };
//
//   /** A run's end as its timer counts it: a success clears the streak, a failure adds to it and, past the fifth on an
//    * `always` timer, says the timer backed off. */
//   const streak = (threadId: string, run: string, state: RunRecord["state"], why: string | undefined): string | undefined => {
//     const timer = thread(threadId).timers.get(run);
//     if (timer === undefined) return why;
//     if (state === "done") timer.failures = 0;
//     if (state !== "failed") return why;
//     timer.failures += 1;
//     timer.failedAt = now();
//     return timer.always && timer.failures >= BACKOFF_AFTER ? `${why ?? "it failed"}; ${BACKED_OFF}` : why;
//   };
//
//   const hold = (req: RunStart, l: Live, why: HeldFor, beside = false): RunStartAnswer => {
//     l.held = { req, for: why };
//     const t = thread(req.threadId);
//     if (why === "busy" && !t.queue.includes(req.run)) t.queue.push(req.run);
//     // A start held beside an instance still running leaves that instance and its record be: it stops only once the new
//     // start launches.
//     if (beside) return why === "approval" ? { outcome: "held", record: l.record, ask: ask(req) } : { outcome: "held", record: l.record };
//     const record = write(req.threadId, req.run, l, { state: "held", why: why === "approval" ? HELD_APPROVAL : why === "busy" ? HELD_BUSY : why === "pressed" ? HELD_PRESSED : HELD_BUDGET, runs: l.record.runs });
//     return why === "approval" ? { outcome: "held", record, ask: ask(req) } : { outcome: "held", record };
//   };
//
//   const fail = (req: RunStart, l: Live, why: string): RunStartAnswer => {
//     const at = now();
//     const record = write(req.threadId, req.run, l, { state: "failed", why: streak(req.threadId, req.run, "failed", why)!, exit: null, runs: l.record.runs, endedAt: at });
//     return { outcome: "failed", record };
//   };
//
//   const next = (threadId: string): void => {
//     const t = thread(threadId);
//     while (t.queue.length > 0 && running(t) < RUNNING_MAX) {
//       const run = t.queue.shift()!;
//       const l = t.runs.get(run);
//       if (l?.held?.for !== "busy") continue;
//       const req = l.held.req;
//       delete l.held;
//       launch(req, l);
//     }
//   };
//
//   const launch = (req: RunStart, l: Live): RunStartAnswer => {
//     const { threadId, run, decl } = req;
//     const t = thread(threadId);
//     // Starting again stops the instance still running, now that this one goes.
//     if (l.record.state === "running") stop(threadId, run, l, undefined);
//     if (running(t) >= RUNNING_MAX) return hold(req, l, "busy");
//     const inputs = req.inputs();
//     if ((inputs.args ?? []).some(input => "secret" in input)) return fail(req, l, SLATE_SECRET_IN_ARGS);
//     // On the thread's own machine the command takes that machine's environment, SLATE_DIR there and the thread's mark,
//     // which names the run's processes as the thread's to that machine's daemon.
//     const road = deps.road?.(threadId, decl.on ?? "thread");
//     let env: Record<string, string>;
//     try {
//       env = road === undefined ? loginEnv(threadId) : { SLATE_DIR: road.slateDir, [THREAD_ENV]: threadId };
//     } catch (e) {
//       return fail(req, l, filesUnwritten(e));
//     }
//     for (const [name, input] of Object.entries(inputs.env ?? {})) {
//       if (!ENV_NAME.test(name)) return fail(req, l, `${name} is not a name an environment variable can have`);
//       const text = "secret" in input ? secrets.plaintext(threadId, input.secret) : input.value === null ? undefined : asText(input.value);
//       if (text === undefined) delete env[name];
//       else env[name] = text;
//     }
//     const args = (inputs.args ?? []).map(input => ("secret" in input ? "" : asText(input.value)));
//     const stdin = inputs.stdin === undefined ? undefined : "secret" in inputs.stdin ? (secrets.plaintext(threadId, inputs.stdin.secret) ?? "") : asText(inputs.stdin.value);
//     const cwd = decl.cwd === undefined ? req.folder : road === undefined ? resolve(req.folder, decl.cwd) : posix.resolve(req.folder, decl.cwd);
//     // A folder on the thread's machine is that machine's to find; its launcher says so when it is not there.
//     if (road === undefined && (!existsSync(cwd) || !statSync(cwd).isDirectory())) return fail(req, l, `the folder ${cwd} does not exist`);
//     const timeout = timeoutOf(decl);
//
//     const gen = ++l.gen;
//     const startedAt = now();
//     const runs = l.record.runs + 1;
//     const record = write(threadId, run, l, runningRecord(l.result, runs, startedAt));
//
//     const lines: string[] = [];
//     const line = (stream: "out" | "err", text: string): void => {
//       const clean = secrets.scrub(threadId, text);
//       lines.push(clean);
//       if (lines.length > LINES_KEPT) lines.shift();
//       if (l.gen === gen) deps.onLine?.(threadId, run, clean, stream);
//     };
//     const end = (o: RoadEnd): void => {
//       if (l.gen !== gen) return;
//       delete l.kill;
//       const raw = secrets.scrub(threadId, o.out);
//       const out = runOutputTail(raw);
//       const err = runOutputTail(secrets.scrub(threadId, o.err));
//       let json: SlateJson | undefined;
//       if (out.trim() !== "") {
//         try {
//           json = JSON.parse(out) as SlateJson;
//         } catch {
//           json = undefined;
//         }
//       }
//       const exit = o.timedOut ? EXEC_DEADLINE_EXIT : o.code;
//       const why = o.error !== undefined ? secrets.scrub(threadId, o.error.message) : o.timedOut ? `timed out after ${timeout} s` : exit === 0 ? undefined : exit === null ? `ended by ${o.signal ?? "a signal"}` : `exited with ${exit}`;
//       const finish = (fields: Pick<RunRecord, "state" | "why" | "exit" | "err" | "json">): void => {
//         const endedAt = now();
//         const said = streak(threadId, run, fields.state, fields.why);
//         write(threadId, run, l, {
//           ...fields,
//           ...(said !== undefined ? { why: said } : {}),
//           out,
//           ...(decl.stream === true ? { lines: [...lines] } : {}),
//           startedAt,
//           endedAt,
//           ms: endedAt - startedAt,
//           runs,
//           ...(o.cut ? { cut: true as const } : {}),
//         });
//         next(threadId);
//       };
//       const ok = exit === 0 && o.error === undefined;
//       if (!ok || decl.then === undefined) {
//         finish({ state: ok ? "done" : "failed", ...(why !== undefined ? { why } : {}), exit, err, ...(json !== undefined ? { json } : {}) });
//         return;
//       }
//       // A then gets the raw result and nothing the command was given (12): on a box, its login, SLATE_DIR and the mark. It
//       // has 60 s whatever the command's own timeout, as a tool run's then has.
//       const shaping = road === undefined ? reshape(threadId, decl.then, raw, cwd) : road.reshape({ cmd: decl.then, input: raw, cwd, env: { SLATE_DIR: road.slateDir, [THREAD_ENV]: threadId }, timeoutS: DEFAULT_TIMEOUT_S, scrub: text => secrets.scrub(threadId, text) });
//       l.reshaping = shaping;
//       void shaping.done.then(answer => {
//         if (l.gen !== gen) return;
//         delete l.reshaping;
//         finish("json" in answer ? { state: "done", exit, err, json: answer.json } : { state: "failed", why: answer.why, exit: answer.exit, err: answer.err });
//       });
//     };
//     let release: (() => void) | undefined;
//     let over = false;
//     const ended = (o: RoadEnd): void => {
//       over = true;
//       release?.();
//       end(o);
//     };
//     const started = (road ?? localRoad).start({ cmd: decl.cmd, args, cwd, env, ...(stdin !== undefined ? { stdin } : {}), timeoutS: timeout, stream: decl.stream === true }, { line, end: ended });
//     if (!over && started.pid !== undefined) release = deps.own?.(threadId, started.pid, "slate");
//     // A road may end before it answers, as a launch refused at once does; nothing is left to kill then.
//     if (l.gen === gen && l.record.state === "running") l.kill = started.kill;
//     return { outcome: "running", record };
//   };
//
//   const stop = (threadId: string, run: string, l: Live, why: string | undefined): void => {
//     const was = l.record.state;
//     l.gen += 1;
//     l.kill?.();
//     delete l.kill;
//     l.reshaping?.kill();
//     delete l.reshaping;
//     delete l.held;
//     const t = thread(threadId);
//     t.queue = t.queue.filter(r => r !== run);
//     if (why === undefined || (was !== "running" && was !== "held")) return;
//     const endedAt = now();
//     write(threadId, run, l, {
//       state: "cancelled",
//       why,
//       exit: null,
//       runs: l.record.runs,
//       ...(l.record.startedAt !== undefined && was === "running" ? { startedAt: l.record.startedAt, ms: endedAt - l.record.startedAt } : {}),
//       endedAt,
//     });
//   };
//
//   const tick = (threadId: string): void => {
//     const t = thread(threadId);
//     for (const [run, timer] of t.timers) {
impl RunsInner {
    pub(crate) fn live(&self, thread_id: &str, run: &str, last: Option<&RunRecord>) {
        todo!()
    }
    pub(crate) fn write(&self, thread_id: &str, run: &str, record: RunRecord) -> RunRecord {
        todo!()
    }
    pub(crate) fn login_env(&self, thread_id: &str) -> Result<IndexMap<String, String>, String> {
        todo!()
    }
    pub(crate) fn running(&self, thread_id: &str) -> usize {
        todo!()
    }
    pub(crate) fn ask(&self, req: &RunStart) -> RunAsk {
        todo!()
    }
    pub(crate) fn streak(&self, thread_id: &str, run: &str, state: super::parts::RunState, why: Option<&str>) -> Option<String> {
        todo!()
    }
    pub(crate) fn hold(&self, req: RunStart, why: HeldFor, beside: bool) -> RunStartAnswer {
        todo!()
    }
    pub(crate) fn fail(&self, req: &RunStart, why: &str) -> RunStartAnswer {
        todo!()
    }
    pub(crate) fn next(self: &Rc<Self>, thread_id: &str) {
        todo!()
    }
    pub(crate) fn launch(self: &Rc<Self>, req: RunStart) -> RunStartAnswer {
        todo!()
    }
    /// `why` none writes nothing.
    pub(crate) fn stop(&self, thread_id: &str, run: &str, why: Option<&str>) {
        todo!()
    }
    // slate-runs.ts:858
    //   const tick = (threadId: string): void => {
    //     const t = thread(threadId);
    //     for (const [run, timer] of t.timers) {
    //       const on = timer.always || t.shown;
    //       if (on && timer.handle === undefined) {
    //         const fire = (): void => {
    //           if (timer.always && timer.failures >= BACKOFF_AFTER && timer.failedAt !== undefined && now() - timer.failedAt < BACKOFF_MS) return;
    //           deps.onTimer?.(threadId, run);
    //         };
    //         fire();
    //         timer.handle = setInterval(fire, Math.max(timer.every, TIMER_FLOOR_S) * 1_000);
    //         timer.handle.unref();
    //       } else if (!on && timer.handle !== undefined) {
    //         clearInterval(timer.handle);
    //         delete timer.handle;
    //       }
    //     }
    //   };
    //   const clearTimers = (t: ThreadRuns): void => {
    //     for (const timer of t.timers.values()) if (timer.handle !== undefined) clearInterval(timer.handle);
    //     t.timers.clear();
    //   };
    pub(crate) fn tick(self: &Rc<Self>, thread_id: &str) {
        todo!()
    }
}

// slate-runs.ts:881
//   return {
//     key,
//     start(req) {
//       const l = live(req.threadId, req.run, req.last);
//       // A tick on a napping box wakes nothing: the run waits with its last result in view, and no done fires.
//       if (req.asleep === true && req.by === "timer") {
//         if (l.record.state === "running") return { outcome: "noop", record: l.record };
//         delete l.held;
//         return { outcome: "held", record: write(req.threadId, req.run, l, { ...(l.result ?? {}), state: "held", why: HELD_ASLEEP, runs: l.record.runs }) };
//       }
//       const refused = req.decl.on === "host" ? deps.hostRefused?.(req.threadId, req.run) : undefined;
//       if (refused !== undefined) return fail(req, l, refused);
//       const beside = l.record.state === "running";
//       if (beside && req.decl.once === true) return { outcome: "noop", record: l.record };
//       delete l.held;
//       // The person's starts have a budget of their own: a scripted window pressing fast restarts nothing past it.
//       const person = req.by === "person";
//       const times = spent(person ? l.pressed : l.starts, now());
//       if (times === undefined) return hold(req, l, person ? "pressed" : "budget", beside);
//       if (person) l.pressed = times;
//       else l.starts = times;
//       if (req.decl.confirm !== undefined || !approvals.has(req.threadId, key(req.decl))) return hold(req, l, "approval", beside);
//       return launch(req, l);
//     },
//     approve(threadId, run, scope) {
//       const l = threads.get(threadId)?.runs.get(run);
//       if (l?.held?.for !== "approval") return undefined;
//       const req = l.held.req;
//       delete l.held;
//       if (scope === "always") approvals.allow(threadId, key(req.decl));
//       return launch(req, l);
//     },
//     deny(threadId, run) {
//       const l = threads.get(threadId)?.runs.get(run);
//       if (l?.held === undefined) return;
//       // Turning down a new start leaves the instance still running to finish.
//       if (l.kill !== undefined || l.reshaping !== undefined) {
//         delete l.held;
//         thread(threadId).queue = thread(threadId).queue.filter(r => r !== run);
//         return;
//       }
//       stop(threadId, run, l, "you said not to run it");
//     },
//     revoke: (threadId, k) => approvals.revoke(threadId, k),
//     allowed: threadId => approvals.list(threadId),
//     held(threadId) {
//       return [...(threads.get(threadId)?.runs.values() ?? [])].flatMap(l => (l.held?.for === "approval" ? [ask(l.held.req)] : []));
//     },
//     cancel(threadId, run) {
//       const l = threads.get(threadId)?.runs.get(run);
//       if (l === undefined) return;
//       stop(threadId, run, l, "cancelled");
//       next(threadId);
//     },
//     stopAll(threadId, opts) {
//       const t = threads.get(threadId);
//       if (t === undefined) return;
//       for (const [run, l] of t.runs) stop(threadId, run, l, opts?.quiet === true ? undefined : (opts?.why ?? "cancelled"));
//     },
//     timers(threadId, list) {
//       const t = thread(threadId);
//       const kept = new Map([...t.timers].filter(([run, was]) => list.some(n => n.run === run && n.key === was.key && n.every === was.every && (n.always === true) === was.always)));
//       for (const [run, timer] of t.timers) if (!kept.has(run) && timer.handle !== undefined) clearInterval(timer.handle);
//       t.timers = kept;
//       for (const timer of list) if (!kept.has(timer.run)) t.timers.set(timer.run, { every: timer.every, key: timer.key, always: timer.always === true, failures: 0 });
//       tick(threadId);
//     },
//     shown(threadId, on) {
//       const t = thread(threadId);
//       t.shown = on;
//       tick(threadId);
//     },
//     reshape,
//     release(threadId) {
//       for (const l of threads.get(threadId)?.runs.values() ?? []) l.starts = [];
//     },
//     secrets: {
//       set: secrets.set,
//       clear: secrets.clear,
//       handle: secrets.handle,
//       forget: secrets.forget,
//       scrub: secrets.scrub,
//       plaintext: secrets.plaintext,
//     },
//     drop(threadId, keep = false) {
//       const t = threads.get(threadId);
//       if (t !== undefined) {
//         if (!keep) for (const [run, l] of t.runs) stop(threadId, run, l, undefined);
//         clearTimers(t);
//         threads.delete(threadId);
//       }
//       for (const k of approvals.list(threadId)) approvals.revoke(threadId, k);
//       secrets.forget(threadId);
//     },
//     close() {
//       for (const [threadId, t] of threads) {
//         for (const [run, l] of t.runs) stop(threadId, run, l, undefined);
//         clearTimers(t);
//       }
//       threads.clear();
//     },
//   };
impl SlateRuns for SlateRunsTable {
    fn key(&self, decl: &CmdRunDecl) -> String {
        todo!()
    }
    fn start(&self, req: RunStart) -> RunStartAnswer {
        todo!()
    }
    fn approve(&self, thread_id: &str, run: &str, scope: ApproveScope) -> Option<RunStartAnswer> {
        todo!()
    }
    fn deny(&self, thread_id: &str, run: &str) {
        todo!()
    }
    fn revoke(&self, thread_id: &str, key: &str) {
        todo!()
    }
    fn allowed(&self, thread_id: &str) -> Vec<String> {
        todo!()
    }
    fn held(&self, thread_id: &str) -> Vec<RunAsk> {
        todo!()
    }
    fn cancel(&self, thread_id: &str, run: &str) {
        todo!()
    }
    fn stop_all(&self, thread_id: &str, quiet: bool, why: Option<&str>) {
        todo!()
    }
    fn timers(&self, thread_id: &str, list: Vec<TimerDecl>) {
        todo!()
    }
    fn shown(&self, thread_id: &str, shown: bool) {
        todo!()
    }
    fn release(&self, thread_id: &str) {
        todo!()
    }
    fn reshape(&self, thread_id: &str, cmd: &str, input: &str, folder: &str, timeout_s: Option<f64>) -> Reshape {
        todo!()
    }
    fn secrets(&self) -> Rc<dyn SlateSecrets> {
        todo!()
    }
    fn drop_thread(&self, thread_id: &str, keep: bool) {
        todo!()
    }
    fn close(&self) {
        todo!()
    }
}
