// SPDX-License-Identifier: AGPL-3.0-only
//! The `ExecStreamFactory` of a turn on this computer: a detached bash writing the turn's output to a log file and
//! its code to an exit file under the state folder's `runs/`, a reader that polls the log from its first poll, the
//! attach by handle and the sweep. Port of packages/runtime/src/local-exec.ts:437-794 at 65f6b89c7. Spec:
//! spec/local_exec_stream.md.
//!
//! Private state sits behind a `std::sync::Mutex` that is never held across an `.await` or a call out (rule 3.3).
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::stream::BoxStream;
use wsp_adapters::port::{Attached, AttachRunOptions, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory, Settle, SharedFuture, WriteOutcome};

use super::reap::TreeWorkReader;
use crate::ids::Ids;
use crate::machine_exec::{NowFn, ReadingSet, TurnWaiting};

/// How often the log is read and the limits are read against the clock.
pub const POLL_MS: u64 = 100;
/// How often a process kept between turns is read while no turn is open.
pub const RESTING_POLL_MS: u64 = 1_000;

// TS local-exec.ts:50-64
// export interface LocalExecOptions extends Pick<MachineExecOptions, "idleMs" | "deadlineMs" | "now" | "pollMs"> {
//   /** The folder the child starts in; the command may cd elsewhere, as a harness turn's does. */
//   root: string;
//   /** The folder a run's script, log, pid and exit code live in. One folder per state file, so two hosts on this
//    * computer never sweep each other's turns. */
//   runDir: string;
//   /** How the turn's tree is read; ps is the road on this computer. A test hands in a reader that answers off the
//    * clock the rule measures against, since what a real tree is given on a loaded box is not what the rule is. */
//   readWork?: TreeWorkReader;
//   /** Every run this process is reading, each as the call that stops reading it. A poll of a run holds the event
//    * loop until that run ends, so a process that is done with its work has to let go of the ones still going: the
//    * turns themselves lead their own process groups and go on, and whatever opens them next reads their logs from
//    * the first byte. The wiring that made this factory owns the set and empties it when it closes. */
//   reading?: Set<() => void>;
// }
#[derive(Clone)]
pub struct LocalExecOptions {
    /// The folder the child starts in.
    pub root: String,
    /// The folder a run's script, log, pid and exit code live in; one per state file, given normalized (no trailing
    /// slash), since a run's handle is `run_dir + "/" + <12 hex>`.
    pub run_dir: String,
    /// `TURN_IDLE_MS` when `None`; `f64::INFINITY` for none.
    pub idle_ms: Option<f64>,
    /// `TURN_WALL_MS` when `None`; `f64::INFINITY` for none.
    pub deadline_ms: Option<f64>,
    pub now: Option<NowFn>,
    /// 100 when `None`.
    pub poll_ms: Option<u64>,
    /// `tree_work()`, one per open, when `None`.
    pub read_work: Option<TreeWorkReader>,
    pub reading: Option<Arc<ReadingSet>>,
    /// Where a run's name and late-line keys are drawn from (rule 3.16); `OsIds` when `None`.
    pub ids: Option<Arc<dyn Ids + Send + Sync>>,
}

impl LocalExecOptions {
    /// Every option at its default but the two folders.
    pub fn new(root: impl Into<String>, run_dir: impl Into<String>) -> Self {
        Self { root: root.into(), run_dir: run_dir.into(), idle_ms: None, deadline_ms: None, now: None, poll_ms: None, read_work: None, reading: None, ids: None }
    }
}

pub struct LocalExecFactory {
    opts: LocalExecOptions,
    waiting: TurnWaiting,
}

/// The factory for this computer; `waiting` is `false` always when `None`.
// TS local-exec.ts:437-446
// export function localExecStream(opts: LocalExecOptions, isWaiting?: TurnWaiting): ExecStreamFactory {
//   const limits = { idleMs: opts.idleMs ?? TURN_IDLE_MS, deadlineMs: opts.deadlineMs ?? TURN_WALL_MS };
//   const waiting = isWaiting ?? (() => false);
//   const now = opts.now ?? Date.now;
//   const pollMs = opts.pollMs ?? POLL_MS;
//   const runDir = opts.runDir;
//   /** A handle this factory could have minted, and nothing else: the shape is read here, by the one predicate both
//    * the attach road and the sweep read, so nothing that turned up in the run folder reaches a signal on the
//    * strength of being there. */
//   const minted = (run: string): boolean => run.startsWith(`${runDir}/`) && RUN_ID.test(run.slice(runDir.length + 1));
pub fn local_exec_stream(opts: LocalExecOptions, waiting: Option<TurnWaiting>) -> Arc<LocalExecFactory> {
    todo!()
}

impl LocalExecFactory {
    /// The launch, with the TS's synchronous throws (a secret, a failed mkdir or exclusive create) as `Err`.
    // TS local-exec.ts:702-755
    //   const factory: ExecStreamFactory = (command, { env, input, inputAfter, secret }) => {
    //     // This computer's agents read their servers' values off their own files, so no launch here is handed one.
    //     if (secret !== undefined) throw new Error("a launch on this computer takes no text holding a value; nothing was started");
    //     const base = join(runDir, randomBytes(6).toString("hex"));
    //     mkdirSync(runDir, { recursive: true, mode: OWNER_DIR });
    //     // The claim is the one path that says a run is on this computer, and mkdir is what makes it exist at once.
    //     mkdirSync(`${base}.d`, { mode: OWNER_DIR });
    //     // The command runs in a subshell, so a turn whose own line ends in `exit` leaves the script standing and its
    //     // code still reaches the exit file: a run whose code nobody wrote reads as a run that answered nothing.
    //     // Stdin is a pipe only the backgrounded pump holds: node on macOS never reads the end of a named fifo (2026-10-03).
    //     // The pump is an orphan in the run's group, as a command a turn leaves is: each of its two processes writes its own
    //     // pid before it carries a message, since a read of the group can come before the launcher has forked either.
    //     const body =
    //       input === undefined
    //         ? `( ${command}\n)\necho $? > ${shellQuote(`${base}.exit`)}\n`
    //         : `( ${command}\n) < <(sh -c ${shellQuote(`echo $$ >> ${shellQuote(`${base}.pump`)}; exec tail -n +1 -f ${shellQuote(`${base}.in`)}`)} | { sh -c ${shellQuote(`echo $PPID >> ${shellQuote(`${base}.pump`)}`)}; while IFS= read -r l; do [ "$l" = ${shellQuote(endMarker(base))} ] && break; printf '%s\\n' "$l"; done; } &)\n` +
    //           `echo $? > ${shellQuote(`${base}.exit`)}\n`;
    //     // The three files that carry what a turn is: the script holds the command, the input channel holds every
    //     // message a person sends and the log holds everything the agent prints. The mode goes on at the open, never by
    //     // a chmod after it: a file that is readable for one moment has been read. The environment, the provider key and
    //     // the turn's own token among it, reaches the process at the spawn alone and is never written here.
    //     writeOwned(`${base}.sh`, body);
    //     const seed = input?.map(line => `${line}\n`).join("");
    //     if (seed !== undefined) writeOwned(`${base}.in`, inputAfter === undefined ? seed : "");
    //     // On disk rather than in this process, so a host that goes before the seed is due leaves it for the next one.
    //     if (seed !== undefined && inputAfter !== undefined) writeOwned(`${base}.held`, seed);
    //     writeOwned(`${base}.log`, "");
    //     const log = openSync(`${base}.log`, "a");
    //     /** A launch node itself could not make: no bash on the PATH the turn runs under. Nothing wrote a log or an exit
    //      * code, so the reader has to be told rather than left polling for one. */
    //     const launch: { failed?: Error } = {};
    //     try {
    //       const child = spawn("bash", [`${base}.sh`], {
    //         cwd: opts.root,
    //         env: { ...env, [AGENT_PID_FILE_ENV]: `${base}.agent` },
    //         // No pipe of this process's is handed to the run: its output is the log, and its stdin the pump the script
    //         // starts, so nothing it holds dies with the host that launched it.
    //         stdio: ["ignore", log, log],
    //         // Its own process group, so a signal can reach what the turn started without reaching this host, and the
    //         // terminal's Ctrl-C reaches the host alone.
    //         detached: true,
    //       });
    //       child.on("error", (e: Error) => {
    //         launch.failed = e;
    //       });
    //       child.unref();
    //       if (child.pid !== undefined) writeFileSync(`${base}.pid`, `${child.pid}\n`);
    //     } finally {
    //       closeSync(log);
    //     }
    //     const stream = open(base, input !== undefined, { launch });
    //     if (seed !== undefined && inputAfter !== undefined) void inputAfter.then(() => releaseHeld(base)).catch(() => stream.kill());
    //     return stream;
    //   };
    pub fn try_launch(&self, command: &str, options: ExecOptions) -> Result<Arc<dyn ExecStream>, ExecFailure> {
        todo!()
    }
}

impl ExecStreamFactory for LocalExecFactory {
    /// `try_launch`, with a refusal answered as a stream whose `lines` yields it once and whose `exited` is `None`.
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        todo!()
    }

    // TS local-exec.ts:757-773
    //   factory.attach = async (run, { input, startedAt, from }) => {
    //     if (!minted(run)) throw new Error(`${run} is not a run this host could have launched`);
    //     // The claim is what says the run is still here, and this computer's own answer is the only one there is: a
    //     // folder that is gone is a run that is gone, and nothing else may end one.
    //     if (!existsSync(`${run}.d`)) return "gone";
    //     if (input) releaseHeld(run);
    //     // An empty channel is a launch whose host went before its prompt was written, and nothing will write it now.
    //     if (input && existsSync(`${run}.in`) && statSync(`${run}.in`).size === 0) {
    //       await reapRun(run);
    //       return "gone";
    //     }
    //     const at = { startedAt, ...(from !== undefined ? { from } : {}) };
    //     if (!input) return open(run, input, at);
    //     releaseLate(run);
    //     const taken = (readFile(`${run}.in`) ?? "").split("\n").filter(line => line !== "" && line !== endMarker(run));
    //     return open(run, input, { ...at, taken });
    //   };
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<BoxFuture<'static, Result<Attached, ExecFailure>>> {
        todo!()
    }

    // TS local-exec.ts:775-791
    //   factory.sweep = async keep => {
    //     const kept = new Set(keep);
    //     let held: string[];
    //     try {
    //       held = readdirSync(runDir);
    //     } catch {
    //       return [];
    //     }
    //     const stale = held
    //       .filter(name => name.endsWith(".d"))
    //       .map(name => join(runDir, name.slice(0, -".d".length)))
    //       .filter(base => minted(base) && !kept.has(base));
    //     // Each run's group is ended beside the others, not after them: every reap waits out its own stop grace, and a
    //     // computer holding a day of them would spend that grace once per run in a connect that has to end.
    //     await Promise.all(stale.map(base => reapRun(base)));
    //     return stale;
    //   };
    fn sweep(&self, keep: &[String]) -> Option<BoxFuture<'static, Result<Vec<String>, ExecFailure>>> {
        todo!()
    }
}

/// A handle this factory could have minted: `run_dir/` and 12 lowercase hex.
// TS local-exec.ts:446-446
//   const minted = (run: string): boolean => run.startsWith(`${runDir}/`) && RUN_ID.test(run.slice(runDir.length + 1));
pub fn minted(run_dir: &str, run: &str) -> bool {
    todo!()
}

/// The script `base.sh`: the command in a subshell and its code into `base.exit`, its stdin the pump of `base.in`
/// when it has an input channel.
// TS local-exec.ts:709-718
//     // The command runs in a subshell, so a turn whose own line ends in `exit` leaves the script standing and its
//     // code still reaches the exit file: a run whose code nobody wrote reads as a run that answered nothing.
//     // Stdin is a pipe only the backgrounded pump holds: node on macOS never reads the end of a named fifo (2026-10-03).
//     // The pump is an orphan in the run's group, as a command a turn leaves is: each of its two processes writes its own
//     // pid before it carries a message, since a read of the group can come before the launcher has forked either.
//     const body =
//       input === undefined
//         ? `( ${command}\n)\necho $? > ${shellQuote(`${base}.exit`)}\n`
//         : `( ${command}\n) < <(sh -c ${shellQuote(`echo $$ >> ${shellQuote(`${base}.pump`)}; exec tail -n +1 -f ${shellQuote(`${base}.in`)}`)} | { sh -c ${shellQuote(`echo $PPID >> ${shellQuote(`${base}.pump`)}`)}; while IFS= read -r l; do [ "$l" = ${shellQuote(endMarker(base))} ] && break; printf '%s\\n' "$l"; done; } &)\n` +
//           `echo $? > ${shellQuote(`${base}.exit`)}\n`;
pub fn launch_script(base: &str, command: &str, has_input: bool) -> String {
    todo!()
}

/// One local run as the turn sees it.
// TS local-exec.ts:448-700
//   /** The one reader both roads share: a launch that has just started its child, and an attach to a run an earlier
//    * host process left behind. The log is read from its first byte either way, so a run that printed while no host
//    * was listening is replayed to whoever attaches. */
//   const open = (base: string, hasInput: boolean, o: { launch?: { failed?: Error }; startedAt?: number; from?: number; taken?: readonly string[] } = {}): ExecStream => {
//     const { launch } = o;
//     // The idle clock runs from this reader's first second, since nothing on disk records when the run's last byte
//     // landed; the wall runs from the turn's own start, which an attach is handed.
//     let startedAt = o.startedAt ?? now();
//     let activity = turnActivity(now());
//     const readWork = opts.readWork ?? treeWork();
//     /** Between two turns of a process kept for the next one: no limit reads the quiet, since no turn is open to be stuck. */
//     let resting = false;
//     /** A turn's end on a kept process ends what the turn left running in the group, as the reap of a turn's whole group
//      * did: a command the agent left going holds the turn's token, which otherwise would outlive the turn by the keep.
//      * The agent, the servers it started and its input stay. */
//     const endLeftovers = async (): Promise<void> => {
//       const pid = leader();
//       if (pid === undefined) return;
//       const left = leftovers(await groupRows(pid), pid, pumpOf(base));
//       for (const stray of left) signalPid(stray, "SIGTERM");
//       if (left.length > 0) setTimeout(() => left.filter(alive).forEach(stray => signalPid(stray, "SIGKILL")), RUN_STOP_MS).unref();
//     };
//     let killed = false;
//     let inputClosed = false;
//     /** Settles once every line parked with writeAfter went in or was dropped; set only while one is parked. A line written
//      * meanwhile goes after it, and a second parked line parks once the first is out of the way. */
//     let parkedLine: Promise<void> | undefined;
//     const parkedOut = async (): Promise<void> => {
//       while (parkedLine !== undefined) await parkedLine;
//     };
//     let finishCode: number | null | undefined;
//     let resolveExit: (code: number | null) => void = () => {};
//     const exited = new Promise<number | null>(resolve => {
//       resolveExit = resolve;
//     });
//     const finish = (code: number | null): void => {
//       if (finishCode !== undefined) return;
//       finishCode = code;
//       resolveExit(code);
//     };
//     const leader = (): number | undefined => {
//       const pid = Number(readFile(`${base}.pid`)?.trim() ?? "");
//       return Number.isSafeInteger(pid) && pid > 0 ? pid : undefined;
//     };
//     const signal = (sig: NodeJS.Signals): void => {
//       const pid = leader();
//       if (pid === undefined || finishCode !== undefined) return;
//       signalGroup(pid, sig);
//     };
//
//     /** The poll runs whether or not anybody is reading yet: the run is on this computer either way, so its limits,
//      * its ending and its reap cannot wait on a consumer. What it reads is queued for whoever iterates. */
//     const lines = new Lines();
//     let offset = o.from ?? 0;
//     let downs = 0;
//     let reading = false;
//     /** Set once a read took everything the log held at that moment, short of a chunk: until then a limit waits, and
//      * it waits too while the last read saw the exit, so a turn re-opened past its wall keeps what it printed while no
//      * host read it, and the reply of a run that finished in that time. */
//     let caughtUp = false;
//     let exitSeen = false;
//     /** Set when this process lets go of the run: the poll ends where it stands, the stream never settles and the
//      * run is left exactly as it is, since ending it here would write the turn off for whoever owns it. */
//     let dropped = false;
//     let wake: (() => void) | undefined;
//     const nap = (ms: number): Promise<void> =>
//       new Promise<void>(resolve => {
//         const timer = setTimeout(() => {
//           wake = undefined;
//           resolve();
//         }, ms);
//         wake = () => {
//           clearTimeout(timer);
//           wake = undefined;
//           resolve();
//         };
//       });
//     const drop = (): void => {
//       dropped = true;
//       opts.reading?.delete(drop);
//       wake?.();
//     };
//     opts.reading?.add(drop);
//     /** What the log holds past what has been read, up to `end`, a chunk at a time so one poll of a run that printed
//      * megabytes while nobody watched cannot hold the loop. */
//     const readLog = (end = Infinity): Buffer => {
//       let fd: number | undefined;
//       try {
//         const size = Math.min(statSync(`${base}.log`).size, end);
//         if (size <= offset) return Buffer.alloc(0);
//         fd = openSync(`${base}.log`, "r");
//         const buf = Buffer.alloc(Math.min(size - offset, EXEC_CHUNK_BYTES));
//         const read = readSync(fd, buf, 0, buf.length, offset);
//         offset += read;
//         return buf.subarray(0, read);
//       } catch {
//         return Buffer.alloc(0);
//       } finally {
//         if (fd !== undefined) closeSync(fd);
//       }
//     };
//     const settle = async (code: number | null, cut?: Error): Promise<void> => {
//       if (finishCode !== undefined) return;
//       opts.reading?.delete(drop);
//       await reapRun(base, async end => {
//         for (let chunk = readLog(end); chunk.length > 0; chunk = readLog(end)) {
//           lines.feed(chunk);
//           await new Promise(resolve => setImmediate(resolve));
//         }
//       });
//       if (cut === undefined) lines.end();
//       else lines.fail(cut);
//       finish(code);
//     };
//
//     const poll = async (): Promise<void> => {
//       while (finishCode === undefined && !dropped) {
//         if (killed) return settle(null);
//         // Nothing was launched, so no process will write a log or an exit code: the stream ends where a run that
//         // was killed before it answered ends, rather than polling a log nobody writes.
//         if (launch?.failed !== undefined) return settle(null);
//         const at = now();
//         if (waiting()) activity.touch(at);
//         const quietMs = activity.quietMs(at);
//         const pid = leader();
//         if (pid !== undefined && !reading && !resting && readsWork(limits.idleMs, quietMs)) {
//           reading = true;
//           readWork(pid, ticks => {
//             reading = false;
//             if (ticks !== undefined) activity.read(ticks, at);
//           });
//         }
//         const cut = resting ? undefined : turnCut(limits, at - startedAt, quietMs);
//         if (cut !== undefined && caughtUp && !exitSeen) return settle(null, cut);
//
//         // The exit file is read before the log, so a poll that sees an exit code reads a log that is complete.
//         // An empty one is a run still writing: the shell's write truncates before it writes, as the cloud road reads it too.
//         const ended = readFile(`${base}.exit`)?.trim();
//         const chunk = readLog();
//         if (chunk.length > 0) {
//           activity.touch(now());
//           lines.feed(chunk);
//           if (chunk.length < EXEC_CHUNK_BYTES) caughtUp = true;
//           exitSeen = ended !== undefined && ended !== "";
//           // There may be more than one chunk waiting, and a log read from its first byte can be hundreds of them: the
//           // loop turns between two, or a host re-opening a long turn answers nothing until it has read all of it.
//           await new Promise(resolve => setImmediate(resolve));
//           continue;
//         }
//         if (ended !== undefined && ended !== "") return settle(Number.parseInt(ended, 10));
//         caughtUp = true;
//         if (cut !== undefined) return settle(null, cut);
//         // The leader is checked after the exit file: one that finished in between shows as down with no exit yet.
//         if ((pid === undefined || !alive(pid)) && ++downs > 1) return settle(null);
//         await nap(resting ? RESTING_POLL_MS : pollMs);
//       }
//     };
//     // Nothing the poll does may reach this process's own error road: a reader that threw would take the host and
//     // every other turn on it with it, so the throw ends this stream alone and its reader sees it.
//     void poll().catch((e: unknown) => {
//       lines.fail(e instanceof Error ? e : new Error(String(e)));
//       finish(null);
//     });
//
//     return {
//       lines: lines.iterate(),
//       run: base,
//       // The leader of the turn's own group, the root of the turn's tree (every process under it, and its group): what
//       // names this turn's tree in the pane that lists this computer's processes.
//       ...((): { pid?: number } => {
//         const pid = leader();
//         return pid !== undefined ? { pid } : {};
//       })(),
//       teardown: () => signal("SIGTERM"),
//       kill: () => {
//         killed = true;
//         signal("SIGKILL");
//       },
//       write: async line => {
//         if (!hasInput) throw new Error("this stream has no input channel");
//         if (parkedLine !== undefined) await parkedOut();
//         if (finishCode !== undefined || inputClosed) return "gone";
//         // The run knows the command ended the moment its exit file exists, up to a poll before this side does, and
//         // a reap in flight has taken the claim with the rest of the run.
//         if (existsSync(`${base}.exit`) || !existsSync(`${base}.d`)) return "gone";
//         try {
//           // Any line the channel takes goes after a seed still held, or the release would read the seed as handed over.
//           releaseHeld(base);
//           appendFileSync(`${base}.in`, `${line}\n`);
//         } catch {
//           return "gone";
//         }
//         // The person just acted, so the turn gets its idle time over, as on a cloud turn.
//         activity.touch(now());
//         return "written";
//       },
//       closeInput: () => {
//         if (!hasInput || inputClosed || finishCode !== undefined) return;
//         inputClosed = true;
//         const close = (): void => {
//           try {
//             releaseHeld(base);
//             appendFileSync(`${base}.in`, `${endMarker(base)}\n`);
//           } catch {
//             return;
//           }
//         };
//         if (parkedLine === undefined) close();
//         else void parkedOut().then(close);
//       },
//       writeAfter: (line, after) => {
//         if (!hasInput) throw new Error("this stream has no input channel");
//         const before = parkedLine;
//         let settled: () => void = () => {};
//         const mine: Promise<void> = new Promise(resolve => (settled = resolve));
//         parkedLine = mine;
//         const park = (): Promise<void> => {
//           writeOwned(`${base}.late.key`, randomBytes(6).toString("hex"));
//           writeOwned(`${base}.late`, `${line}\n`);
//           return after.then(
//             () => releaseLate(base),
//             () => {
//               rmSync(`${base}.late`, { force: true });
//               rmSync(`${base}.late.key`, { force: true });
//             },
//           );
//         };
//         void (before === undefined ? park() : before.then(park))
//           .catch(() => undefined)
//           .finally(() => {
//             if (parkedLine === mine) parkedLine = undefined;
//             settled();
//           });
//       },
//       rest: () => {
//         resting = true;
//         void endLeftovers();
//       },
//       nextTurn: () => {
//         resting = false;
//         startedAt = now();
//         activity = turnActivity(now());
//         wake?.();
//         try {
//           return statSync(`${base}.log`).size;
//         } catch {
//           return offset;
//         }
//       },
//       ...(o.taken !== undefined ? { taken: o.taken } : {}),
//       exited,
//     } satisfies ExecStream;
//   };
pub struct LocalStream {
    state: std::sync::Mutex<()>,
}

impl ExecStream for LocalStream {
    fn lines(&self) -> BoxStream<'static, Result<String, ExecFailure>> {
        todo!()
    }
    fn run(&self) -> Option<String> {
        todo!()
    }
    /// The leader read once at open.
    fn pid(&self) -> Option<u32> {
        todo!()
    }
    fn teardown(&self) {
        todo!()
    }
    fn kill(&self) {
        todo!()
    }
    fn write(&self, line: String) -> BoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        todo!()
    }
    fn close_input(&self) {
        todo!()
    }
    /// `None` on a stream with no input channel (the TS throws), so the caller's own write gets the refusal.
    fn write_after(&self, line: String, after: Settle) -> Option<()> {
        todo!()
    }
    fn taken(&self) -> Option<Vec<String>> {
        todo!()
    }
    fn rest(&self) -> Option<()> {
        todo!()
    }
    /// The log's size, or the offset where the log cannot be read.
    fn next_turn(&self) -> Option<u64> {
        todo!()
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        todo!()
    }
}
