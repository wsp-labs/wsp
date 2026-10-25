// SPDX-License-Identifier: AGPL-3.0-only
//! The one reader both machine roads share: polls a run's log over exec, yields its lines, ends and reaps the run
//! however it ends, cuts it at the turn's limits, and carries its input channel. Port of
//! packages/runtime/src/machine-exec.ts:217-264 and 346-736 at 65f6b89c7. Spec: spec/machine_exec_stream.md.
//!
//! Private state sits behind a `std::sync::Mutex` that is never held across an `.await` or a call out (rule 3.3).
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::stream::BoxStream;
use wsp_adapters::port::{ExecFailure, ExecStream, Settle, SharedFuture, WriteOutcome};

use super::factory::{Core, Launched, Posting};

/// The poll after a write, a hand-over, or a poll that took bytes in a launch's opening, and for how long.
pub const HURRIED_POLL_MS: u64 = 200;
pub const HURRIED_FOR_MS: i64 = 3_000;
/// How long from its launch a run is read at the hurried pace while its lines keep coming.
pub const OPENING_HURRY_MS: i64 = 30_000;
/// The poll of a process kept between turns while no turn is open.
pub const RESTING_POLL_MS: u64 = 10_000;
/// What bash writes as the exit of a command SIGKILL ended.
pub const SIGKILL_EXIT: i32 = 137;
/// How often the reap looks at the group it asked to go.
pub const GRACE_POLL_MS: u64 = 200;

/// Everything `open` is handed: a launch that has just posted its script, or an attach to a run an earlier host left.
pub struct OpenArgs {
    pub base: String,
    pub has_input: bool,
    /// The launch's outcome; already `Ok` on an attach.
    pub launched: Launched,
    /// The turn's own start, which an attach is handed; `now()` when `None`.
    pub turn_started_at: Option<i64>,
    /// On an attach with input, every line the channel took.
    pub taken: Option<Vec<String>>,
    /// The launch's posts; `None` on an attach.
    pub posting: Option<Posting>,
    /// The log byte the reader starts at; 0 when `None`.
    pub from: Option<u64>,
    /// A held seed's due date; the seed is handed over once it settles `Ok`, and the stream is killed on `Err`.
    pub seed_due: Option<Settle>,
}

/// One machine run as the turn sees it.
// TS machine-exec.ts:342-346
//   /** The one reader both roads share: a launch that has just posted its script, and an attach to a run an earlier
//    * host process left behind. `opened` settles once the run is known to be on the machine and rejects with the words
//    * the turn fails on when it is not. The log is read from its first byte either way, so a run that printed while no
//    * host was listening is replayed to whoever attaches. */
//   const open = (base: string, hasInput: boolean, launched: Promise<void>, turnStartedAt?: number, taken?: readonly string[], posting?: Posting, from?: number, seedDue?: Promise<void>): ExecStream => {
pub struct MachineStream {
    state: std::sync::Mutex<()>,
}

/// Opens the reader. Eager: the run question after an unanswered launch, the seed's hand-over and signals; lazy: the
/// poll, which runs only while `lines` is iterated.
// TS machine-exec.ts:346-436
//   const open = (base: string, hasInput: boolean, launched: Promise<void>, turnStartedAt?: number, taken?: readonly string[], posting?: Posting, from?: number, seedDue?: Promise<void>): ExecStream => {
//     const sentinel = `__WSP_EOF_${randomBytes(6).toString("hex")}__`;
//     const reapMark = `__WSP_REAPED_${randomBytes(6).toString("hex")}__`;
//     /** Every shell that hands the channel a line runs after the last one ended: two hand-overs of one held seed that
//      * ran together would both read the channel as empty and append the seed twice. */
//     let handing: Promise<unknown> = Promise.resolve();
//     const oneAtATime = <T>(f: () => Promise<T>): Promise<T> => {
//       const next = handing.then(f, f);
//       handing = next.catch(() => undefined);
//       return next;
//     };
//     /** Settles once every line parked with writeAfter went in or was dropped; set only while one is parked. A line written
//      * meanwhile goes after it, and a second parked line parks once the first is out of the way. */
//     let parkedLine: Promise<void> | undefined;
//
//     let killed = false;
//     /** Set once this side sent the run a signal: a run it ended is not one the kernel killed. */
//     let signalled = false;
//     let inputClosed = false;
//     /** Set when this process lets go of the run: the poll ends where it stands, nothing is reaped and no exit is
//      * written, since ending the run here would take a turn its own owner is still waiting on. */
//     let dropped = false;
//     const { nap, wake, hurry } = pollNap(opts.sleep);
//     // The agent's reply to a write (a stop, a steer, an answer, or an adapter's own line) would otherwise wait out a
//     // whole poll: the polls run at HURRIED_POLL_MS for a while after any write. So do they in a launch's opening, until
//     // its first line and for a while after each line there.
//     let hurriedUntil = posting === undefined ? 0 : now() + OPENING_HURRY_MS;
//     const gap = (): number => (now() < hurriedUntil ? Math.min(pollMs, HURRIED_POLL_MS) : resting ? RESTING_POLL_MS : pollMs);
//     /** The polls run at HURRIED_POLL_MS from now for a while, the one under way woken. */
//     const hurryPolls = (): void => {
//       hurriedUntil = now() + HURRIED_FOR_MS;
//       hurry();
//     };
//     const drop = (): void => {
//       dropped = true;
//       opts.reading?.delete(drop);
//       wake();
//     };
//     opts.reading?.add(drop);
//     // The idle clock runs from this reader's first second, since nothing on the machine records when the run's last
//     // byte landed; the wall runs from the turn's own start, which an attach is handed.
//     let startedAt = turnStartedAt ?? now();
//     let activity = turnActivity(now());
//     /** Between two turns of a process kept for the next one: no limit reads the quiet, since no turn is open to be stuck. */
//     let resting = false;
//     /** How much of the log this reader has taken: a turn on a process that served earlier turns starts where its
//      * re-open was told to. */
//     let offset = from ?? 0;
//     let finishCode: number | null | undefined;
//     let resolveExit: (code: number | null) => void = () => {};
//     const exited = new Promise<number | null>(resolve => {
//       resolveExit = resolve;
//     });
//     const finish = (code: number | null): void => {
//       if (finishCode === undefined) {
//         finishCode = code;
//         opts.reading?.delete(drop);
//         resolveExit(code);
//       }
//     };
//
//     /** Whether the machine holds the run, asked for one link window, the wait the launch's own posts were given, or
//      * until a stop on a line's try: a launch whose posts went out and whose answer never came may have started it.
//      * Only the guest's own answer says it does not; a refusal of the question (a box not connected, a sandbox the
//      * provider cannot reach) says nothing about the run, and is asked again, as a machine silent for the whole window
//      * or a stop says nothing. Nothing is known while the process lets go of the run. */
//     const holdsRun = async (): Promise<{ held: "yes" | "no" | "silent" | "stopped"; asked: number }> => {
//       const from = now();
//       for (let asked = 0; ; asked++) {
//         if (dropped) await never();
//         if (killed) return { held: "stopped", asked };
//         if (opts.asksUntilStopped !== true && now() - from >= LINK_RETRY_WINDOW_MS) return { held: "silent", asked };
//         const res = await machine.exec(`if [ -d ${q(claim(base))} ]; then echo ${HANDSHAKE.run}; else echo ${HANDSHAKE.gone}; fi`, { timeoutMs: execTimeoutMs }).catch(() => undefined);
//         if (res?.stdout.includes(HANDSHAKE.run) === true) return { held: "yes", asked: asked + 1 };
//         if (res?.stdout.includes(HANDSHAKE.gone) === true) return { held: "no", asked: asked + 1 };
//         await nap(pollMs);
//       }
//     };
//     // A run the machine holds after such a launch is read as one that launched, so its agent's lines reach the turn
//     // and nothing launches it a second time. One it does not hold never ran, and one it stayed silent about through
//     // the window is a computer that did not answer: either may go again, and a line's own clock decides whether it
//     // does. A stop keeps the launch's own words.
//     const opened = launched.catch(async (e: unknown) => {
//       if (posting === undefined || !posting.mayHaveRun()) throw e;
//       const { held, asked } = await holdsRun();
//       if (held === "yes") return;
//       if (held === "silent") throw new MachineUnreached(posting.posts() + asked, now() - posting.since, e);
//       if (held === "no" && !(e instanceof MachineUnreached)) throw new LaunchUnanswered(e instanceof Error ? e.message : String(e), { cause: e });
//       throw e;
//     });
//
pub fn open(core: Arc<Core>, args: OpenArgs) -> Arc<MachineStream> {
    todo!()
}

impl ExecStream for MachineStream {
    // TS machine-exec.ts:494-615
    //     async function* lines(): AsyncGenerator<string> {
    //       let downs = 0;
    //       const split = new LineCut();
    //       /** Every line the log held past the last poll once the KILL went out, read by the reap, and the one it ended on
    //        * without a newline; the run's files go whatever the reads answered. */
    //       async function* reaped(): AsyncGenerator<string> {
    //         let end: number | undefined;
    //         for (let read = await reap(offset); read !== undefined; read = await reap(offset, end)) {
    //           end = read.end;
    //           offset += read.chunk.length;
    //           yield* split.take(read.chunk);
    //           if (read.chunk.length === 0 || offset >= end) break;
    //         }
    //         if (end === undefined || offset < end) await remove();
    //         const last = split.rest();
    //         if (last !== undefined) yield last;
    //       }
    //
    //       try {
    //         await opened;
    //       } catch (e) {
    //         yield* reaped();
    //         finish(null);
    //         throw e;
    //       }
    //
    //       /** Set once a poll took everything the log held at that moment, short of a chunk: until then a limit waits, and
    //        * it waits too while the last poll saw the exit, so a turn re-opened past its wall keeps what it printed while
    //        * no host read it, and the reply of a run that finished in that time. */
    //       let caughtUp = false;
    //       let exitSeen = false;
    //       /** When the polls last started coming back with nothing: before the reader has caught up, a cut waits out the
    //        * same reach window an attach's probe gets, so one gateway error after a re-open does not take the reply. */
    //       let darkSince: number | undefined;
    //       async function* cutHere(e: Error): AsyncGenerator<string, never> {
    //         yield* reaped();
    //         finish(null);
    //         throw e;
    //       }
    //       async function* cutInTheDark(cut: Error | undefined, at: number): AsyncGenerator<string> {
    //         darkSince ??= at;
    //         if (cut !== undefined && (caughtUp || now() - darkSince >= LINK_RETRY_WINDOW_MS)) yield* cutHere(cut);
    //       }
    //       while (true) {
    //         // This process is done reading this run: the poll ends here and the stream settles for nobody.
    //         if (dropped) await never();
    //         if (killed) {
    //           yield* reaped();
    //           finish(null);
    //           return;
    //         }
    //         const at = now();
    //         if (waiting()) activity.touch(at);
    //         const quietMs = activity.quietMs(at);
    //         const cut = resting ? undefined : turnCut({ idleMs, deadlineMs }, at - startedAt, quietMs);
    //         if (cut !== undefined && caughtUp && !exitSeen) yield* cutHere(cut);
    //
    //         let res: ExecResult;
    //         try {
    //           res = await machine.exec(pollCmd(offset, !resting && readsWork(idleMs, quietMs)), { timeoutMs: execTimeoutMs });
    //         } catch {
    //           // Machine likely napping; polls recover after wake (P10 semantics). A poll that never reached the machine
    //           // says nothing about the process it was sent to read, so the stretch the road was dark is no part of the
    //           // turn's silence: the idle clock holds here and goes on from the road's return.
    //           yield* cutInTheDark(cut, at);
    //           await nap(pollMs);
    //           activity.hold(now() - at);
    //           continue;
    //         }
    //
    //         const out = res.stdout.split("\n");
    //         const markIdx = out.findIndex(l => l.startsWith(sentinel));
    //         if (markIdx === -1) {
    //           yield* cutInTheDark(cut, at);
    //           await nap(pollMs);
    //           continue;
    //         }
    //         darkSince = undefined;
    //         const [, exitStr = "", live = "up", workStr = ""] = out[markIdx]!.split(" ");
    //         const work = Number.parseInt(workStr, 10);
    //         if (Number.isSafeInteger(work)) activity.read(work, now());
    //         const chunk = Buffer.from(out.slice(0, markIdx).join(""), "base64");
    //         offset += chunk.length;
    //
    //         if (chunk.length > 0) {
    //           activity.touch(now());
    //           if (posting !== undefined && now() - posting.since < OPENING_HURRY_MS) hurriedUntil = now() + HURRIED_FOR_MS;
    //           yield* split.take(chunk);
    //           if (chunk.length < EXEC_CHUNK_BYTES) caughtUp = true;
    //           exitSeen = exitStr !== "";
    //           continue; // there may be more than one chunk buffered up
    //         }
    //
    //         if (exitStr !== "") {
    //           yield* reaped();
    //           const code = Number.parseInt(exitStr, 10);
    //           finish(code);
    //           if (code === SIGKILL_EXIT) await killedHere();
    //           return;
    //         }
    //         caughtUp = true;
    //         if (cut !== undefined) yield* cutHere(cut);
    //         // The pid is checked after the exit file: a leader that finished in between shows as down with no exit yet.
    //         if (live === "down" && ++downs > 1) {
    //           yield* reaped();
    //           finish(null);
    //           await killedHere();
    //           return;
    //         }
    //         await nap(gap());
    //       }
    //     }
    //
    //     /** The lines as the stream hands them out: a reader its consumer let go of is done, since nothing reads the run's
    //      * end any more, and the set of runs this process reads would otherwise keep the stream with its last line. */
    //     async function* lettingGo(): AsyncGenerator<string> {
    //       try {
    //         yield* lines();
    //       } finally {
    //         finish(null);
    //       }
    //     }
    fn lines(&self) -> BoxStream<'static, Result<String, ExecFailure>> {
        todo!()
    }
    fn run(&self) -> Option<String> {
        todo!()
    }
    // TS machine-exec.ts:437-449
    //     const signal = (sig: "TERM" | "KILL"): void => {
    //       signalled = true;
    //       // A process resting between turns is read at RESTING_POLL_MS, and its end is waited on.
    //       if (resting) hurryPolls();
    //       void opened
    //         .catch(() => undefined)
    //         .then(() =>
    //           machine.exec(`P=$(cat ${q(base)}.pid 2>/dev/null); [ -n "$P" ] && kill -${sig} -- -$P 2>/dev/null; true`, {
    //             timeoutMs: execTimeoutMs,
    //           }),
    //         )
    //         .catch(() => undefined);
    //     };
    fn teardown(&self) {
        todo!()
    }
    fn kill(&self) {
        todo!()
    }
    // TS machine-exec.ts:632-667
    //     /** One line onto the channel, after a seed still held. */
    //     const append = async (line: string): Promise<"written" | "gone"> => {
    //       // The guest knows the command ended the moment its exit file exists, up to a poll before this side does,
    //       // and a reap in flight has taken the claim with the rest of the run.
    //       const res = await oneAtATime(() => putFiles(machine, [{ path: `${base}.in`, text: `${line}\n`, append: true }], {
    //         // Any line the channel takes goes after a seed still held, or the release would read the seed as handed over.
    //         before: [`{ [ -e ${q(base)}.exit ] || [ ! -d ${q(claim(base))} ]; } && { echo ${HANDSHAKE.gone}; exit 0; }`, releaseHeld(base)],
    //         after: [`echo ${HANDSHAKE.written}`],
    //         timeoutMs: execTimeoutMs,
    //       }));
    //       if (res.stdout.includes(HANDSHAKE.gone)) return "gone";
    //       if (res.exitCode !== 0 || !res.stdout.includes(HANDSHAKE.written)) throw new Error(`remote write failed on ${machine.id}: nothing came back saying ${HANDSHAKE.written}, the word the guest prints once the message landed; ${machineAnswer(res)}`);
    //       // The person just acted, so the turn gets its idle time over.
    //       activity.touch(now());
    //       hurryPolls();
    //       return "written";
    //     };
    //
    //     const stream: ExecStream = {
    //       lines: lettingGo(),
    //       run: base,
    //       teardown: () => signal("TERM"),
    //       kill: () => {
    //         killed = true;
    //         signal("KILL");
    //       },
    //       write: async line => {
    //         if (!hasInput) throw new Error("this stream has no input channel");
    //         if (finishCode !== undefined) throw new Error("the stream has ended");
    //         // Only what was parked before this write goes ahead of it: a kept Codex writes thread/resume and parks its
    //         // turn/start in the same tick, and a turn/start on a thread the server let go is never answered.
    //         const ahead = parkedLine;
    //         await opened;
    //         await ahead;
    //         return append(line);
    //       },
    fn write(&self, line: String) -> BoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        todo!()
    }
    // TS machine-exec.ts:668-679
    //       closeInput: () => {
    //         if (!hasInput || inputClosed || finishCode !== undefined) return;
    //         inputClosed = true;
    //         if (resting) hurryPolls();
    //         void opened
    //           .catch(() => undefined)
    //           .then(async () => {
    //             while (parkedLine !== undefined) await parkedLine;
    //           })
    //           .then(() => oneAtATime(() => machine.exec(`${releaseHeld(base)}; P=$(cat ${q(base)}.tail 2>/dev/null); [ -n "$P" ] && kill -TERM "$P" 2>/dev/null; true`, { timeoutMs: execTimeoutMs })))
    //           .catch(() => undefined);
    //       },
    fn close_input(&self) {
        todo!()
    }
    /// `None` on a stream with no input channel (the TS throws), so the caller's own write gets the refusal.
    // TS machine-exec.ts:680-715
    //       writeAfter: (line, after) => {
    //         if (!hasInput) throw new Error("this stream has no input channel");
    //         const before = parkedLine ?? Promise.resolve();
    //         let settled: () => void = () => {};
    //         const mine: Promise<void> = new Promise(resolve => (settled = resolve));
    //         parkedLine = mine;
    //         // Beside the run rather than in this process, so a host that goes before the line is due leaves it for the next.
    //         // Its key lands first, so a parked line always has one; both are the login's alone, as every file of the run is.
    //         const park = async (): Promise<void> => {
    //           await before;
    //           const parked = await opened
    //             .then(() => putFiles(machine, [{ path: `${base}.late.key`, text: randomBytes(6).toString("hex") }, { path: `${base}.late`, text: `${line}\n` }], { before: ["umask 077"], timeoutMs: execTimeoutMs }))
    //             .then(
    //               () => true,
    //               () => false,
    //             );
    //           const due = await after.then(
    //             () => true,
    //             () => false,
    //           );
    //           if (!due) {
    //             if (parked) await machine.exec(`rm -f ${q(base)}.late ${q(base)}.late.key`, { timeoutMs: execTimeoutMs });
    //             return;
    //           }
    //           if (!parked) return void (await append(line));
    //           await oneAtATime(() => untilReached(() => machine.exec(releaseLate(base), { timeoutMs: execTimeoutMs }), { now, sleep }));
    //           activity.touch(now());
    //           hurryPolls();
    //         };
    //         void park()
    //           .catch(() => undefined)
    //           .finally(() => {
    //             if (parkedLine === mine) parkedLine = undefined;
    //             settled();
    //           });
    //       },
    fn write_after(&self, line: String, after: Settle) -> Option<()> {
        todo!()
    }
    fn taken(&self) -> Option<Vec<String>> {
        todo!()
    }
    // TS machine-exec.ts:617-630
    //     /** Ends what the turn just over left running beside the agent, TERM then KILL once the stop grace passes: a command
    //      * it left going holds the turn's token, which would otherwise outlive the turn by the keep. */
    //     const endLeftovers = (): void => {
    //       const script =
    //         `P=$(cat ${q(base)}.pid 2>/dev/null); T=$(cat ${q(base)}.tail 2>/dev/null); [ -n "$P" ] || exit 0; ` +
    //         `L=$(for f in /proc/[0-9]*/stat; do read -r s < "$f" && printf '%s\\n' "$s"; done 2>/dev/null | awk -v p="$P" -v t="$T" ${shellQuote(LEFTOVERS_AWK)}); [ -n "$L" ] || exit 0; ` +
    //         `kill -TERM $L 2>/dev/null; ` +
    //         `for i in ${GRACE_CHECKS}; do S=; for x in $L; do kill -0 $x 2>/dev/null && S=1; done; [ -n "$S" ] || break; sleep ${GRACE_POLL_MS / 1000}; done; ` +
    //         `kill -KILL $L 2>/dev/null; true`;
    //       void opened
    //         .catch(() => undefined)
    //         .then(() => machine.exec(script, { timeoutMs: execTimeoutMs }))
    //         .catch(() => undefined);
    //     };
    // TS machine-exec.ts:716-719
    //       rest: () => {
    //         resting = true;
    //         endLeftovers();
    //       },
    fn rest(&self) -> Option<()> {
        todo!()
    }
    // TS machine-exec.ts:720-726
    //       nextTurn: () => {
    //         resting = false;
    //         startedAt = now();
    //         activity = turnActivity(now());
    //         hurry();
    //         return offset;
    //       },
    fn next_turn(&self) -> Option<u64> {
        todo!()
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        todo!()
    }
}

/// The poll's own wait and the calls that cut it short (`pollNap`). `wake` lets go for good: every nap after it
/// returns at once with no timer; `hurry` ends the nap in flight only.
// TS machine-exec.ts:229-264
// /** The poll's own wait, and the one call that cuts it short. The timer belongs to the reader, so a reader that
//  * lets go of a run frees it where it stands rather than at the end of the poll it was in; a caller that handed in
//  * a clock of its own waits on that instead, and the wake ends that wait the same way. */
// function pollNap(sleep?: (ms: number) => Promise<void>): { nap: (ms: number) => Promise<void>; wake: () => void; hurry: () => void } {
//   let woken: (() => void) | undefined;
//   /** Set once the reader has let go. Every nap after that returns where it stands rather than starting a timer:
//    * the wake often lands while the poll is inside its exec, and the nap it comes back to would otherwise hold
//    * this process for one more poll. */
//   let awake = false;
//   const nap = (ms: number): Promise<void> => {
//     if (awake) return Promise.resolve();
//     return new Promise<void>(resolve => {
//       let done = false;
//       const settle = (): void => {
//         if (done) return;
//         done = true;
//         woken = undefined;
//         resolve();
//       };
//       const timer = sleep === undefined ? setTimeout(settle, ms) : undefined;
//       if (sleep !== undefined) void sleep(ms).then(settle);
//       woken = () => {
//         if (timer !== undefined) clearTimeout(timer);
//         settle();
//       };
//     });
//   };
//   return {
//     nap,
//     wake: () => {
//       awake = true;
//       woken?.();
//     },
//     hurry: () => woken?.(),
//   };
// }
pub struct PollNap {
    state: std::sync::Mutex<()>,
}

impl PollNap {
    pub fn new(sleep: Option<super::factory::SleepFn>) -> Arc<Self> {
        todo!()
    }
    pub fn nap(&self, ms: u64) -> BoxFuture<'static, ()> {
        todo!()
    }
    pub fn wake(&self) {
        todo!()
    }
    pub fn hurry(&self) {
        todo!()
    }
}

/// One poll's exec: the exit file, a chunk of the log past `offset` as base64, the tree's work when `work`, then the
/// sentinel line with the exit, `up` or `down`, and the work.
// TS machine-exec.ts:475-485
//     // The exit file is read before the log, so a poll that sees an exit code reads a log that is complete. The
//     // tree's work rides the same poll when it is worth reading, so a turn working in silence costs no exec of its
//     // own and a stream nothing could cut asks the guest for nothing; unasked, $W is empty and the reader sees no
//     // reading.
//     const pollCmd = (offset: number, work: boolean): string =>
//       `E=$(cat ${q(base)}.exit 2>/dev/null); ` +
//       `tail -c +${offset + 1} ${q(base)}.log 2>/dev/null | head -c ${EXEC_CHUNK_BYTES} | base64 -w0; ` +
//       `P=$(cat ${q(base)}.pid 2>/dev/null); ` +
//       (work ? `W=$(awk -v p="$P" ${shellQuote(TREE_WORK_AWK)} /proc/[0-9]*/stat 2>/dev/null); ` : "") +
//       `printf '\\n${sentinel} %s %s %s\\n' "$E" ` +
//       `"$([ -n "$P" ] && kill -0 "$P" 2>/dev/null && echo up || echo down)" "$W"`;
pub fn poll_cmd(base: &str, sentinel: &str, offset: u64, work: bool) -> String {
    todo!()
}

/// The reap's read: up to one chunk past `offset` and the log's size after the mark, stopping at `end` when set, and
/// leaving the files when more than a chunk remains.
// TS machine-exec.ts:451-461
//     /** Ends the run on the first call and answers a chunk of its log past `offset`, read once the group is gone: a
//      * run that outlasted its stop grace printed into a window no poll read. The read stops at `end`, the size the first
//      * reap found the log at once the KILL went out, since a process that left the group and still holds the log would
//      * otherwise keep the stop going; the files stay while more than the chunk is left, for the next reap to read on
//      * from. Answers undefined where nothing came back. */
//     const reap = (offset: number, end?: number): Promise<{ chunk: Buffer; end: number } | undefined> => {
//       const log = `${q(base)}.log`;
//       const read =
//         `S=${end ?? `$({ wc -c < ${log}; } 2>/dev/null || echo 0)`}; N=$((S - ${offset})); [ $N -gt ${EXEC_CHUNK_BYTES} ] && N=${EXEC_CHUNK_BYTES}; ` +
//         `[ $N -gt 0 ] && tail -c +${offset + 1} ${log} 2>/dev/null | head -c $N | base64 -w0; printf '\\n%s %s\\n' ${reapMark} "$((S))"; ` +
//         `[ $((S - ${offset})) -gt ${EXEC_CHUNK_BYTES} ] && exit 0; `;
pub fn reap_read(base: &str, mark: &str, offset: u64, end: Option<u64>) -> String {
    todo!()
}

/// `P=$(cat '<b>'.pid 2>/dev/null); [ -n "$P" ] && kill -<sig> -- -$P 2>/dev/null; true`.
// TS machine-exec.ts:444-444
//           machine.exec(`P=$(cat ${q(base)}.pid 2>/dev/null); [ -n "$P" ] && kill -${sig} -- -$P 2>/dev/null; true`, {
pub fn signal_cmd(base: &str, sig: &str) -> String {
    todo!()
}

/// The close of the channel: the held seed first, then TERM to the recorded tail alone.
// TS machine-exec.ts:677-677
//           .then(() => oneAtATime(() => machine.exec(`${releaseHeld(base)}; P=$(cat ${q(base)}.tail 2>/dev/null); [ -n "$P" ] && kill -TERM "$P" 2>/dev/null; true`, { timeoutMs: execTimeoutMs })))
pub fn close_input_cmd(base: &str) -> String {
    todo!()
}

/// Ends what a kept turn left beside the agent: the leftovers off `/proc`, TERM, ten checks, KILL.
// TS machine-exec.ts:619-625
//     const endLeftovers = (): void => {
//       const script =
//         `P=$(cat ${q(base)}.pid 2>/dev/null); T=$(cat ${q(base)}.tail 2>/dev/null); [ -n "$P" ] || exit 0; ` +
//         `L=$(for f in /proc/[0-9]*/stat; do read -r s < "$f" && printf '%s\\n' "$s"; done 2>/dev/null | awk -v p="$P" -v t="$T" ${shellQuote(LEFTOVERS_AWK)}); [ -n "$L" ] || exit 0; ` +
//         `kill -TERM $L 2>/dev/null; ` +
//         `for i in ${GRACE_CHECKS}; do S=; for x in $L; do kill -0 $x 2>/dev/null && S=1; done; [ -n "$S" ] || break; sleep ${GRACE_POLL_MS / 1000}; done; ` +
//         `kill -KILL $L 2>/dev/null; true`;
pub fn end_leftovers_cmd(base: &str) -> String {
    todo!()
}

/// The `before` lines of one append into the channel: gone when the run ended or its claim went, then the held seed.
// TS machine-exec.ts:636-641
//       const res = await oneAtATime(() => putFiles(machine, [{ path: `${base}.in`, text: `${line}\n`, append: true }], {
//         // Any line the channel takes goes after a seed still held, or the release would read the seed as handed over.
//         before: [`{ [ -e ${q(base)}.exit ] || [ ! -d ${q(claim(base))} ]; } && { echo ${HANDSHAKE.gone}; exit 0; }`, releaseHeld(base)],
//         after: [`echo ${HANDSHAKE.written}`],
//         timeoutMs: execTimeoutMs,
//       }));
pub fn append_before(base: &str) -> Vec<String> {
    todo!()
}

/// JS `parseInt(text, 10)`: leading whitespace, an optional sign, then digits; `None` where that reads NaN or the
/// value is not a safe integer.
pub fn js_parse_int(text: &str) -> Option<i64> {
    todo!()
}
