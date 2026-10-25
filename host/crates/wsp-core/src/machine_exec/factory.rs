// SPDX-License-Identifier: AGPL-3.0-only
//! The `ExecStreamFactory` of a turn on a machine reached by exec: the launch, the attach by handle and the sweep,
//! and the shell text both they and the reader send. Port of packages/runtime/src/machine-exec.ts:1-93, 300-345 and
//! 738-861 at 65f6b89c7. Spec: spec/machine_exec_factory.md.
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;

use futures::future::{BoxFuture, Shared};
use wsp_adapters::port::{Attached, AttachRunOptions, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory};
use wsp_engine::errors::{EngineError, RetryClock};
use wsp_engine::Machine;

use super::rules::{ReadingSet, TurnWaiting};
use crate::ids::Ids;
use crate::owner_thread::OwnerThread;

/// The clock both limits read; ms epoch.
pub type NowFn = Arc<dyn Fn() -> i64 + Send + Sync>;
/// What every wait runs on, the poll's and the launch retry's; tests hand in one that moves the clock.
pub type SleepFn = Arc<dyn Fn(u64) -> BoxFuture<'static, ()> + Send + Sync>;
/// The words a run the kernel may have killed fails with, asked with when the turn started; `None` leaves the code.
pub type KilledWhy = Arc<dyn Fn(i64) -> BoxFuture<'static, Option<String>> + Send + Sync>;

// TS machine-exec.ts:47-81
// export interface MachineExecOptions {
//   /** Delay between log polls. */
//   pollMs?: number;
//   /** How long the log may stay quiet, write() and the work of the run's tree included, before the stream ends with
//    * exit null and the idle line. */
//   idleMs?: number;
//   /** The cap on one stream however much it prints; the stream ends with exit null and the wall line past it. */
//   deadlineMs?: number;
//   /** Per-poll REST exec timeout. */
//   execTimeoutMs?: number;
//   /** Directory inside the guest for script/log/pid/exit files. */
//   runDir?: string;
//   /** The clock both limits read. */
//   now?: () => number;
//   /** What every wait runs on, the poll's and the launch retry's; tests hand in one that moves the clock. */
//   sleep?: (ms: number) => Promise<void>;
//   /** Every run this process is reading, each as the call that stops reading it. A poll of a run holds the event
//    * loop until that run ends, so a process that is done with its work has to let go of the ones still going: the
//    * turns themselves go on running on their machines, and whatever opens them next reads their logs from the
//    * first byte. The wiring that made this factory owns the set and empties it when it closes. */
//   reading?: Set<() => void>;
//   /** The machine a run is launched through where it is not the one its polls, signals and reaps go through: a
//    * computer the person joined launches a thread's turn from a shell standing in that thread's cgroup. */
//   launchOn?: Machine;
//   /** The thread a turn's run is for, which a kind that groups a thread's processes reads to pick `launchOn`. */
//   thread?: string;
//   /** A launch whose posts got no answer asks after its run until the turn is stopped, not for one link window: a
//    * line's try, which the line's own hour stops, and which a second launch beside a run that is there would deliver
//    * twice. */
//   asksUntilStopped?: true;
//   /** The words a run the kernel may have killed fails with instead of its exit code, where the kind has any: asked
//    * once, with when the turn started, when the run's exit file says 137 or its group went with none, and never once
//    * this side signalled it. An answer of nothing leaves the exit code to say how it ended. */
//   killedWhy?: (startedAt: number) => Promise<string | undefined>;
// }
#[derive(Clone, Default)]
pub struct MachineExecOptions {
    /// Delay between log polls; 1500 when `None`.
    pub poll_ms: Option<u64>,
    /// The idle limit; `TURN_IDLE_MS` when `None`, `f64::INFINITY` for none.
    pub idle_ms: Option<f64>,
    /// The wall limit; `TURN_WALL_MS` when `None`, `f64::INFINITY` for none.
    pub deadline_ms: Option<f64>,
    /// Per-exec timeout; `INLINE_EXEC_MS` when `None`.
    pub exec_timeout_ms: Option<u64>,
    /// The guest folder a run's files live in; `RUN_DIR` when `None`.
    pub run_dir: Option<String>,
    pub now: Option<NowFn>,
    /// `None` naps on a real timer that a wake cancels; `Some` naps on this, for the retries too.
    pub sleep: Option<SleepFn>,
    pub reading: Option<Arc<ReadingSet>>,
    /// The machine a launch goes through where it is not the one polls, signals and reaps go through.
    pub launch_on: Option<Arc<dyn Machine>>,
    /// The thread a turn's run is for, which a kind that groups a thread's processes reads to pick `launch_on`.
    pub thread: Option<String>,
    /// A launch whose posts got no answer asks after its run until the turn is stopped, not for one link window.
    pub asks_until_stopped: bool,
    pub killed_why: Option<KilledWhy>,
    /// Where a run's name, sentinels and late-line keys are drawn from (rule 3.16); `OsIds` when `None`.
    pub ids: Option<Arc<dyn Ids + Send + Sync>>,
}

/// A launch's posts as the reader that follows them asks after them (machine-exec.ts:89-93).
#[derive(Clone)]
pub struct Posting {
    /// When the first post went, on the factory's clock.
    pub since: i64,
    pub posts: Arc<AtomicU32>,
    /// A post went out and got no answer, which may have started the run.
    pub may_have_run: Arc<AtomicBool>,
}

/// The launch's outcome as the reader awaits it: `Ok` once the run is known to be up, else the failure the turn ends
/// on, already in the words and kind of machine_exec_factory rule 8.
pub type Launched = Shared<BoxFuture<'static, Result<(), ExecFailure>>>;

/// What the factory and every stream it opens read: the machine and the resolved options.
pub struct Core {
    /// Behind `OwnerThread`: the engine's `Machine` is not `Send`, and the factory must be (wsp-adapters' port).
    pub machine: OwnerThread<Arc<dyn Machine>>,
    pub launch_on: Option<OwnerThread<Arc<dyn Machine>>>,
    pub poll_ms: u64,
    pub idle_ms: f64,
    pub deadline_ms: f64,
    pub exec_timeout_ms: u64,
    pub run_dir: String,
    pub now: NowFn,
    /// The option as handed in: the nap reads whether there is one.
    pub sleep: Option<SleepFn>,
    /// `sleep`, or a real timer: what the retries wait on.
    pub retry_sleep: SleepFn,
    pub reading: Option<Arc<ReadingSet>>,
    pub asks_until_stopped: bool,
    pub killed_why: Option<KilledWhy>,
    pub waiting: TurnWaiting,
    pub ids: Arc<dyn Ids + Send + Sync>,
}

impl Core {
    /// The engine's retry clock over `now` and `retry_sleep`, with the real random draw, for `until_reached`.
    pub fn retry_clock(&self) -> Arc<dyn RetryClock> {
        todo!()
    }
}

// TS machine-exec.ts:300-308
// export function machineExecStream(machine: Machine, opts: MachineExecOptions = {}, isWaiting?: TurnWaiting): ExecStreamFactory {
//   const pollMs = opts.pollMs ?? 1500;
//   const idleMs = opts.idleMs ?? TURN_IDLE_MS;
//   const waiting = isWaiting ?? (() => false);
//   const deadlineMs = opts.deadlineMs ?? TURN_WALL_MS;
//   const execTimeoutMs = opts.execTimeoutMs ?? INLINE_EXEC_MS;
//   const runDir = opts.runDir ?? RUN_DIR;
//   const now = opts.now ?? Date.now;
//   const sleep = opts.sleep ?? realRetryClock.sleep;
pub struct MachineExecFactory {
    core: Arc<Core>,
}

/// The factory for one machine; `waiting` is `false` always when `None`.
pub fn machine_exec_stream(machine: Arc<dyn Machine>, opts: MachineExecOptions, waiting: Option<TurnWaiting>) -> Arc<MachineExecFactory> {
    todo!()
}

impl MachineExecFactory {
    pub fn core(&self) -> &Arc<Core> {
        todo!()
    }

    /// The launch, with the TS's synchronous refusal (a secret on a machine whose launch takes no input) as `Err` and
    /// nothing sent. The upload starts before this returns.
    // TS machine-exec.ts:738-810
    //   const factory: ExecStreamFactory = (command, { env, input, inputAfter, secret }) => {
    //     const base = `${runDir}/${randomBytes(6).toString("hex")}`;
    //     // A seed that is not due yet waits on the guest rather than in this process, so a host that goes before it is due
    //     // leaves it for the next one, and the command starts meanwhile.
    //     const holdsSeed = input !== undefined && inputAfter !== undefined;
    //     const seedPath = `${base}.${holdsSeed ? "held" : "in"}`;
    //     if (secret !== undefined && machine.takesStdin !== true) throw new Error(`${machine.id} takes no input on its launch, so text holding a value would ride its command; nothing was started`);
    //     // Text that holds values rides the launch's input under names of its own, is written to the run's files under
    //     // its mask and leaves the shell before the run starts, so the run's environment never carries it.
    //     const held = [
    //       ...Object.entries(secret?.files ?? {}).map(([variable, text], i) => ({ name: `WSP_LAND_${i}`, path: `${base}.f${i}`, text, variable })),
    //       ...(secret?.input === true && input !== undefined ? [{ name: "WSP_LAND_IN", path: seedPath, text: input.map(line => `${line}\n`).join(""), variable: undefined }] : []),
    //     ];
    //
    //     // The environment reaches the run as the one setsid starts it under and never sits in its script. A machine
    //     // that takes stdin reads it off the launch exec's input, since its command line is world-readable there; a
    //     // provider's exec drops stdin, so on those single-login machines it rides the launch's text, which the images
    //     // wsp builds keep sudo and the box login's systemd manager from logging (QUIET_LOGS).
    //     const named = Object.fromEntries(Object.entries(env).filter(([k]) => ENV_KEY.test(k)));
    //     const envLines = machine.takesStdin === true ? [ENV_FROM_INPUT] : Object.entries(named).map(([k, v]) => `export ${k}=${shellQuote(v)}`);
    //     const heldLines = held.flatMap(h => [`printf %s "$${h.name}" > ${q(h.path)} || exit 1`, `unset ${h.name}`, ...(h.variable !== undefined ? [`export ${h.variable}=${q(h.path)}`] : [])]);
    //     // The tail starts in a subshell so bash's job notice for its kill never lands in the log; the command's exit code
    //     // is written before the tail is killed, so a poll that sees it reads a finished log.
    //     const run =
    //       input === undefined
    //         ? `${command}\necho $? > ${q(base)}.exit\n`
    //         : `( tail -n +1 -f ${q(base)}.in > ${q(base)}.fifo & echo $! > ${q(base)}.tail )\n{ ${command}\n} < ${q(base)}.fifo\necho $? > ${q(base)}.exit\nkill $(cat ${q(base)}.tail) 2>/dev/null\n`;
    //     // The turn's processes are what the kernel takes first when memory runs out: the work outgrew the machine, and
    //     // the daemon and the guest agent are how anyone hears of it.
    //     const files: GuestWrite[] = [{ path: `${base}.sh`, text: `${workScoreLine()}\n${run}` }];
    //     if (input !== undefined && !held.some(h => h.variable === undefined)) files.push({ path: seedPath, text: input.map(line => `${line}\n`).join("") });
    //
    //     // Whether a post of the launch went out and got no answer: one that failed before it left this computer started
    //     // nothing there, and one the far end answered, a refusal included, said what came of it.
    //     let mayHaveRun = false;
    //     let posts = 0;
    //     const since = now();
    //     // Spawn eagerly, like a local child process would: a held seed's one more exec trip goes while the command starts.
    //     const posted: Promise<ExecResult> = untilReached(
    //       () => {
    //         posts++;
    //         return putFiles(opts.launchOn ?? machine, files, {
    //           // exec honours no idempotency key and a launch whose answer was lost is retried; the claim makes the second
    //           // a no-op. A mkdir that fails for any other reason (a run folder another login on the machine owns) fails
    //           // the launch: read as a replay it would answer launched and leave the reader polling a log nobody writes.
    //           // Every file of the run is the login's alone from the moment it is made, not chmodded after: the input
    //           // and the log carry the person's messages and the agent's output, and a machine somebody owns may carry
    //           // other logins that can read a folder wsp did not make.
    //           before: ["umask 077", `mkdir ${q(claim(base))} 2>/dev/null || { [ -d ${q(claim(base))} ] && { echo ${HANDSHAKE.launched}; exit 0; }; echo ${q(`no run folder on this machine: ${claim(base)}`)} >&2; exit 1; }`],
    //           // The agent writes its pid beside the run, where that computer's daemon reads it before any kill, since this
    //           // host knows no pid there.
    //           after: [`export ${AGENT_PID_FILE_ENV}=${q(`${base}.agent`)}`, ...(holdsSeed ? [`: > ${q(base)}.in`] : []), ...(input === undefined ? [] : [`mkfifo ${q(base)}.fifo`]), ...envLines, ...heldLines, `setsid bash ${q(base)}.sh > ${q(base)}.log 2>&1 & echo $! > ${q(base)}.pid; echo ${HANDSHAKE.launched}`],
    //           timeoutMs: execTimeoutMs,
    //           ...(machine.takesStdin === true ? { stdin: envInput({ ...named, ...Object.fromEntries(held.map(h => [h.name, h.text])) }) } : {}),
    //         }).catch((e: unknown) => {
    //           if (!neverSent(e) && !farEndAnswered(e)) mayHaveRun = true;
    //           throw e;
    //         });
    //       },
    //       { now, sleep },
    //     );
    //     const opened = posted.then(
    //       res => {
    //         if (res.exitCode !== 0 || !res.stdout.includes(HANDSHAKE.launched)) throw new Error(`remote launch failed on ${machine.id}: nothing came back saying ${HANDSHAKE.launched}, the word the guest prints once the run is up; ${machineAnswer(res)}`);
    //       },
    //       (e: unknown) => {
    //         // Each carries the one sentence every surface says about it, and a machine id wrapped round it is nothing a person reads.
    //         if (e instanceof MachineUnreached || e instanceof MachineUnreachableError || isPlaceAbsent(e)) throw e;
    //         throw new Error(`remote launch failed on ${machine.id}: ${e instanceof Error ? e.message : String(e)}`);
    //       },
    //     );
    //     return open(base, input !== undefined, opened, undefined, undefined, { since, posts: () => posts, mayHaveRun: () => mayHaveRun }, undefined, holdsSeed ? inputAfter : undefined);
    //   };
    pub fn try_launch(&self, command: &str, options: ExecOptions) -> Result<Arc<dyn ExecStream>, ExecFailure> {
        todo!()
    }
}

impl ExecStreamFactory for MachineExecFactory {
    /// `try_launch`, with a refusal answered as a stream whose `lines` yields it once and whose `exited` is `None`.
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        todo!()
    }

    // TS machine-exec.ts:812-830
    //   factory.attach = async (run, { input, startedAt, from }) => {
    //     if (!minted(run)) throw new Error(`${run} is not a run this host could have launched`);
    //     // With the run's channel read back under the answer: the lines it took, which a re-opened turn reads for what its
    //     // harness was handed before this host came up. The agent can write that file, so only its last EXEC_CHUNK_BYTES
    //     // come back, whole lines alone: a line before them, most often a turn's opening with its images, is not read, and
    //     // a message steered before them is neither waited on nor sent again.
    //     const channel = input
    //       ? ` n=$({ wc -c < ${q(run)}.in; } 2>/dev/null || echo 0); if [ "$n" -gt ${EXEC_CHUNK_BYTES} ]; then tail -c ${EXEC_CHUNK_BYTES + 1} ${q(run)}.in | sed 1d; else cat ${q(run)}.in 2>/dev/null; fi;`
    //       : "";
    //     const res = await untilReached(() => machine.exec(`if [ -d ${q(claim(run))} ]; then echo ${HANDSHAKE.run};${input ? ` ${releaseLate(run)};` : ""}${channel} else echo ${HANDSHAKE.gone}; fi`, { timeoutMs: execTimeoutMs }), { now, sleep });
    //     // Only these two answers say anything about the run. Anything else is the machine failing to answer the
    //     // question, which is the unreached road, not a run to end: the reader is built and the run swept on WSP_GONE
    //     // alone, so nothing here can take a live turn's process group with it.
    //     const said = res.stdout.split("\n");
    //     const at = said.findIndex(line => line.includes(HANDSHAKE.run));
    //     if (at >= 0) return open(run, input, Promise.resolve(), startedAt, input ? said.slice(at + 1).filter(line => line !== "") : undefined, undefined, from);
    //     if (res.stdout.includes(HANDSHAKE.gone)) return "gone";
    //     throw new Error(`the machine did not answer whether it still holds ${run}; ${machineAnswer(res)}`);
    //   };
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<BoxFuture<'static, Result<Attached, ExecFailure>>> {
        todo!()
    }

    // TS machine-exec.ts:832-858
    //   factory.sweep = async keep => {
    //     // The claims are what the machine holds, so the machine is asked what is there rather than told; the shape a
    //     // handle must have to be one of this factory's is read here, by the same predicate the attach road reads, so
    //     // nothing the guest wrote into the run directory reaches shell text on the strength of being there.
    //     const listed = await machine.exec(`for d in ${q(runDir)}/*.d; do [ -d "$d" ] && printf '%s\\n' "$d"; done`, { timeoutMs: execTimeoutMs });
    //     const kept = new Set(keep);
    //     const stale = listed.stdout
    //       .split("\n")
    //       .map(line => line.trim())
    //       .filter(line => line.endsWith(".d"))
    //       .map(line => line.slice(0, -".d".length))
    //       .filter(base => minted(base) && !kept.has(base));
    //     if (stale.length === 0) return [];
    //     // Each run's group is ended beside the others, not after them: every reap waits out its own stop grace, and a
    //     // machine holding a day of them would spend that grace once per run in a connect that has to end.
    //     const page = (bases: readonly string[]): string => `${bases.map(base => `{ ${reapScript(base)}; } &`).join(" ")} wait; true`;
    //     // The machine holding the most stale runs is the one this exists for, and it is the one whose command would pass
    //     // the exec body cap and be refused whole, so the reaps go a page at a time under the same rule every upload reads.
    //     const pages: string[][] = [[]];
    //     for (const base of stale) {
    //       const last = pages.at(-1)!;
    //       if (last.length > 0 && !execFits(page([...last, base]))) pages.push([base]);
    //       else last.push(base);
    //     }
    //     for (const bases of pages) await machine.exec(page(bases), { timeoutMs: execTimeoutMs });
    //     return stale;
    //   };
    fn sweep(&self, keep: &[String]) -> Option<BoxFuture<'static, Result<Vec<String>, ExecFailure>>> {
        todo!()
    }
}

/// A handle this factory could have minted: `run_dir/` and 12 lowercase hex.
// TS machine-exec.ts:191-194
// const ENV_KEY = /^[A-Za-z_][A-Za-z0-9_]*$/;
// /** What a launch mints a run's name from, so a handle read back off the sessions index is checked against the shape
//  * this code writes before it reaches shell text: a value that has been to a file on disk is no longer this code's. */
// const RUN_ID = /^[0-9a-f]{12}$/;
// TS machine-exec.ts:339-340
//   /** A handle this factory could have minted: the run directory it launches into and a name of its own shape. */
//   const minted = (run: string): boolean => run.startsWith(`${runDir}/`) && RUN_ID.test(run.slice(runDir.length + 1));
pub fn minted(run_dir: &str, run: &str) -> bool {
    todo!()
}

/// The claim folder: the one path that says a run is on the machine.
pub fn claim(base: &str) -> String {
    todo!()
}

/// `RUN_STOP_MS / 200` checks counted from 1, joined by spaces: `1 2 3 4 5 6 7 8 9 10`.
// TS machine-exec.ts:212-213
// /** That grace as the shell's own counter, since a guest has no seq to lean on. */
// const GRACE_CHECKS = Array.from({ length: Math.round(RUN_STOP_MS / GRACE_POLL_MS) }, (_, i) => String(i + 1)).join(" ");
pub fn grace_checks() -> String {
    todo!()
}

/// Hands a run the seed its launch held back, into a channel nothing reached yet, then drops it.
// TS machine-exec.ts:317-320
//   /** Hands a run the seed its launch held back: into the channel where nothing reached it yet, then dropped. The
//    * channel's size is the check, so a host that went between the two steps leaves no road to a second copy. */
//   const releaseHeld = (b: string): string =>
//     `if [ -e ${q(b)}.held ]; then if [ -d ${q(claim(b))} ] && [ ! -s ${q(b)}.in ]; then cat ${q(b)}.held >> ${q(b)}.in; fi; rm -f ${q(b)}.held; fi`;
pub fn release_held_sh(base: &str) -> String {
    todo!()
}

/// A late line goes in once, by its key, after the seed, onto a run that has not ended.
// TS machine-exec.ts:321-325
//   /** A line written after the seed and held until it is due goes in once, after the seed, onto a run that has not
//    * ended. Its key says whether it went, never its words: a thread may send one agent the same words twice, and a host
//    * that went between the append and the drop left the key it appended under beside the channel. */
//   const releaseLate = (b: string): string =>
//     `${releaseHeld(b)}; if [ -e ${q(b)}.late ]; then if [ -d ${q(claim(b))} ] && [ ! -e ${q(b)}.exit ] && [ "$(cat ${q(b)}.late.key 2>/dev/null)" != "$(cat ${q(b)}.late.went 2>/dev/null)" ]; then cat ${q(b)}.late >> ${q(b)}.in && cp ${q(b)}.late.key ${q(b)}.late.went; fi; rm -f ${q(b)}.late ${q(b)}.late.key; fi`;
pub fn release_late_sh(base: &str) -> String {
    todo!()
}

/// Ending one run on the guest: TERM to the recorded group, the stop grace, KILL, then `read`, then the files go.
/// Without `signal` nothing is sent and only `read` and the removal run.
// TS machine-exec.ts:326-338
//   /** What ending one run comes to on the guest, in the shell both the reader's own reap and the connect sweep run:
//    * the recorded process group gets TERM, then KILL once the stop grace passes, and the run's files go. `read` runs
//    * between the two, once nothing of the group is left to write. Without `signal` the group is not sent anything:
//    * a group the reader's first reap ended is signalled once, never again. */
//   const reapScript = (b: string, read = "", signal = true): string =>
//     (signal
//       ? `P=$(cat ${q(b)}.pid 2>/dev/null); ` +
//         `if [ -n "$P" ]; then kill -TERM -- -$P 2>/dev/null; ` +
//         `for i in ${GRACE_CHECKS}; do kill -0 -- -$P 2>/dev/null || break; sleep ${GRACE_POLL_MS / 1000}; done; ` +
//         `kill -KILL -- -$P 2>/dev/null; fi; `
//       : "") +
//     read +
//     `rm -rf ${q(b)}.*`;
pub fn reap_script(base: &str, read: &str, signal: bool) -> String {
    todo!()
}

/// The guest's probe for the claim: `if [ -d '<claim>' ]; then echo WSP_RUN; else echo WSP_GONE; fi`.
pub fn holds_run_probe(base: &str) -> String {
    todo!()
}

/// The script `base.sh`: the work score line, then the command with its exit written, through the fifo when it has
/// an input channel.
// TS machine-exec.ts:759-767
//     // The tail starts in a subshell so bash's job notice for its kill never lands in the log; the command's exit code
//     // is written before the tail is killed, so a poll that sees it reads a finished log.
//     const run =
//       input === undefined
//         ? `${command}\necho $? > ${q(base)}.exit\n`
//         : `( tail -n +1 -f ${q(base)}.in > ${q(base)}.fifo & echo $! > ${q(base)}.tail )\n{ ${command}\n} < ${q(base)}.fifo\necho $? > ${q(base)}.exit\nkill $(cat ${q(base)}.tail) 2>/dev/null\n`;
//     // The turn's processes are what the kernel takes first when memory runs out: the work outgrew the machine, and
//     // the daemon and the guest agent are how anyone hears of it.
//     const files: GuestWrite[] = [{ path: `${base}.sh`, text: `${workScoreLine()}\n${run}` }];
pub fn run_script(base: &str, command: &str, has_input: bool) -> String {
    todo!()
}

/// The claim's mkdir, which makes a second post of one launch a no-op and fails on a folder it cannot make.
// TS machine-exec.ts:786-786
//           before: ["umask 077", `mkdir ${q(claim(base))} 2>/dev/null || { [ -d ${q(claim(base))} ] && { echo ${HANDSHAKE.launched}; exit 0; }; echo ${q(`no run folder on this machine: ${claim(base)}`)} >&2; exit 1; }`],
pub fn claim_mkdir(base: &str) -> String {
    todo!()
}

/// The attach's one exec, with the run's channel read back under the answer when `input`.
// TS machine-exec.ts:818-821
//     const channel = input
//       ? ` n=$({ wc -c < ${q(run)}.in; } 2>/dev/null || echo 0); if [ "$n" -gt ${EXEC_CHUNK_BYTES} ]; then tail -c ${EXEC_CHUNK_BYTES + 1} ${q(run)}.in | sed 1d; else cat ${q(run)}.in 2>/dev/null; fi;`
//       : "";
//     const res = await untilReached(() => machine.exec(`if [ -d ${q(claim(run))} ]; then echo ${HANDSHAKE.run};${input ? ` ${releaseLate(run)};` : ""}${channel} else echo ${HANDSHAKE.gone}; fi`, { timeoutMs: execTimeoutMs }), { now, sleep });
pub fn attach_probe(run: &str, input: bool) -> String {
    todo!()
}

/// One page of the sweep: each run's reap in the background, then `wait; true`.
// TS machine-exec.ts:847-847
//     const page = (bases: readonly string[]): string => `${bases.map(base => `{ ${reapScript(base)}; } &`).join(" ")} wait; true`;
pub fn sweep_page(bases: &[String]) -> String {
    todo!()
}

/// The sweep's listing of claims under the run folder.
// TS machine-exec.ts:836-836
//     const listed = await machine.exec(`for d in ${q(runDir)}/*.d; do [ -d "$d" ] && printf '%s\\n' "$d"; done`, { timeoutMs: execTimeoutMs });
pub fn sweep_listing(run_dir: &str) -> String {
    todo!()
}

/// How a failed launch reads (machine-exec.ts:799-808): an `Unreached`, a `MachineUnreachable` (exec-failed too) or a
/// `PlaceAbsent` keeps its sentence and its kind; anything else is `remote launch failed on <id>: <message>`, `Other`.
pub fn launch_failure(machine_id: &str, e: &EngineError) -> ExecFailure {
    todo!()
}

/// An engine failure as the stream hands it on unchanged: its message, and the kind of its class
/// (`Unreached`, `Unreachable`, `PlaceAbsent`, else `Other`).
pub fn exec_failure_of(e: &EngineError) -> ExecFailure {
    todo!()
}
