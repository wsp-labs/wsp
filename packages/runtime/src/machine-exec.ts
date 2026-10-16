// ExecStreamFactory over a Machine's one-shot REST exec, for running harnesses
// on remote workspaces. The command is launched detached inside the guest
// (setsid + log file), because REST-launched processes survive machine
// pause/resume while control-channel children get reaped (PoC P10/P4b). The
// stream then polls the log, so a mid-turn nap only stalls polling: polls fail
// while the machine is paused, recover after wake, and the turn's own output
// picks up where it left off. The script goes up through the engine's
// putFiles, so it lands under the exec body cap however long it is; the
// engine's execDetached polls the same way for a command that ends on its
// own, while this one streams and can be signalled while it runs, which is
// what a harness turn needs. A stream started with an input channel gets a
// file the launch seeds and every write() appends to through putFiles. A seed
// not due yet waits in a held file beside it, and a line written ahead of when
// it is due in a late file; each goes in once due, the seed also at the next
// write, and both at a later host's attach. A tail feeds the channel to the
// command's stdin through a fifo, so a message reaches a running process the
// runtime holds no pipe to. The tail's pid is recorded:
// closeInput() kills it so the command reads EOF, and the script kills it once
// the command ended. When the stream ends, however it ends, the recorded
// process group gets TERM then KILL and the run's files go: the CLI's own
// children (MCP servers under npx) stay in the setsid group after it exits,
// and were seen holding 90 MB each for the machine's life. A launch nothing
// answered (this computer's DNS gone, a reset connection, a gateway error the
// backend gave up on) is posted again at the engine's backoff for its reach
// window before the turn fails; the claim makes one that did land a no-op.
// The run outlives the process that launched it, so the factory also attaches
// to one by the handle its stream reported: the log on the guest is the whole
// turn, and a reader that comes later reads it from its first byte. The claim
// directory is what says the run is still there; the reap takes it with the
// rest, so an attach to a swept run answers gone instead of hanging. That
// question travels the launch road's reach window, and only the machine's own
// answer that the claim is gone may end a run: a probe nothing answered says
// nothing about the run it was sent to find, and killing that process group
// would end the very turn the attach exists to save. The same fact cuts the
// other way once a host has finished connecting: a run no row of that host
// holds is a harness process nobody will ever read again, so the sweep ends
// every claim on the machine that the caller did not name. A process kept
// for a thread's next turn rests between turns: no limit reads its quiet, its
// log is polled slowly until the next turn's message wakes the reader, and
// what the turn left running beside the agent is ended.

import { randomBytes } from "node:crypto";
import { ENV_FROM_INPUT, HANDSHAKE, INLINE_EXEC_MS, envInput, farEndAnswered, MachineUnreachableError, MachineUnreached, RUN_DIR, execFits, isPlaceAbsent, machineAnswer, neverSent, putFiles, realRetryClock, untilReached, type ExecResult, type GuestWrite, type Machine } from "@wsp/engine";
import { AGENT_PID_FILE_ENV, EXEC_CHUNK_BYTES, LINK_RETRY_WINDOW_MS, RUN_STOP_MS, TURN_IDLE_MS, TURN_WALL_MS, TURN_WORK_TICKS_PER_S, shellQuote, turnCutLine, workScoreLine } from "@wsp/protocol";
import type { ExecStream, ExecStreamFactory, TurnCutRule } from "@wsp/protocol";

export interface MachineExecOptions {
  /** Delay between log polls. */
  pollMs?: number;
  /** How long the log may stay quiet, write() and the work of the run's tree included, before the stream ends with
   * exit null and the idle line. */
  idleMs?: number;
  /** The cap on one stream however much it prints; the stream ends with exit null and the wall line past it. */
  deadlineMs?: number;
  /** Per-poll REST exec timeout. */
  execTimeoutMs?: number;
  /** Directory inside the guest for script/log/pid/exit files. */
  runDir?: string;
  /** The clock both limits read. */
  now?: () => number;
  /** What every wait runs on, the poll's and the launch retry's; tests hand in one that moves the clock. */
  sleep?: (ms: number) => Promise<void>;
  /** Every run this process is reading, each as the call that stops reading it. A poll of a run holds the event
   * loop until that run ends, so a process that is done with its work has to let go of the ones still going: the
   * turns themselves go on running on their machines, and whatever opens them next reads their logs from the
   * first byte. The wiring that made this factory owns the set and empties it when it closes. */
  reading?: Set<() => void>;
  /** The machine a run is launched through where it is not the one its polls, signals and reaps go through: a
   * computer the person joined launches a thread's turn from a shell standing in that thread's cgroup. */
  launchOn?: Machine;
  /** The thread a turn's run is for, which a kind that groups a thread's processes reads to pick `launchOn`. */
  thread?: string;
  /** A launch whose posts got no answer asks after its run until the turn is stopped, not for one link window: a
   * line's try, which the line's own hour stops, and which a second launch beside a run that is there would deliver
   * twice. */
  asksUntilStopped?: true;
  /** The words a run the kernel may have killed fails with instead of its exit code, where the kind has any: asked
   * once, with when the turn started, when the run's exit file says 137 or its group went with none, and never once
   * this side signalled it. An answer of nothing leaves the exit code to say how it ended. */
  killedWhy?: (startedAt: number) => Promise<string | undefined>;
}

/** A launch whose posts went out with no answer, and whose run the machine then said it does not hold: nothing ran
 * there and the computer never answered the launch, so the launch may go again. */
export class LaunchUnanswered extends Error {}

/** A launch's posts, as the reader that follows them asks after them: when the first went, how many went, and whether
 * one went out and got no answer, which may have started the run. */
interface Posting {
  since: number;
  posts: () => number;
  mayHaveRun: () => boolean;
}

/** The two limits a turn runs under on every kind of machine, and what ends one: the wall since it started, else the
 * idle stretch since it last did anything, which is its last byte, the person's last message, or work in the
 * process tree it started. */
export interface TurnLimits {
  idleMs: number;
  deadlineMs: number;
}

/** The one rule that cuts a turn, whatever launched it: the error carrying turnCutLine when a limit has passed, else
 * nothing. The cloud road and the local child both read it, so a hung agent ends with the same words on either. */
export function turnCut(limits: TurnLimits, elapsedMs: number, quietMs: number): Error | undefined {
  const rule: TurnCutRule | undefined = elapsedMs >= limits.deadlineMs ? "wall" : quietMs >= limits.idleMs ? "idle" : undefined;
  return rule === undefined ? undefined : new Error(turnCutLine(rule, elapsedMs, rule === "wall" ? limits.deadlineMs : limits.idleMs));
}

/** One reading of the process tree a turn started: its work in ticks, whatever the road can count of them, and the
 * clock the reading was taken on. Only the difference between two readings means anything. */
interface WorkReading {
  ticks: number;
  at: number;
}

/** The clock the idle limit is read against, one shape for every road: the harness's bytes and the person's messages
 * move it, and so does work in the process tree the turn started once that work clears TURN_WORK_TICKS_PER_S since
 * the clock last moved. A harness sitting in a long tool call prints nothing while its children burn a core, and
 * reading bytes alone ended two builds that were still working (43 and 38 minutes, 2026-09-08). */
export interface TurnActivity {
  /** How long the turn has done nothing: what the idle limit is compared with. */
  quietMs(nowMs: number): number;
  /** A byte from the harness, or a message the person sent into the turn. */
  touch(atMs: number): void;
  /** A stretch nothing could be read through: the road to the machine was dark, so it is no part of the turn's
   * quiet and the clock goes on from where it stood. */
  hold(ms: number): void;
  /** One reading of the turn's tree. The first, and any that undercuts the one before it, is only what the next is
   * measured from: a tree whose members exited carries less work than it did and is no baseline. */
  read(ticks: number, atMs: number): void;
}

export function turnActivity(startedAt: number): TurnActivity {
  let activeAt = startedAt;
  let base: WorkReading | undefined;
  return {
    quietMs: nowMs => nowMs - activeAt,
    touch: atMs => {
      activeAt = atMs;
    },
    hold: ms => {
      activeAt += ms;
    },
    read: (ticks, atMs) => {
      if (base === undefined || ticks < base.ticks) {
        base = { ticks, at: atMs };
        return;
      }
      if (ticks - base.ticks < Math.max(1, (TURN_WORK_TICKS_PER_S * (atMs - base.at)) / 1_000)) return;
      activeAt = atMs;
      base = { ticks, at: atMs };
    },
  };
}

/** Whether the turn's tree is worth reading at this moment: there is an idle limit for a reading to hold open, and
 * the stream has been quiet long enough that the cut is in sight. A turn that is printing is already alive on its
 * bytes, and a stream run with no idle limit (the exec verb hands both limits infinite on purpose) would pay for a
 * reading every second for the life of a build and could not be cut by any answer it got. */
export function readsWork(idleMs: number, quietMs: number): boolean {
  return Number.isFinite(idleMs) && quietMs >= idleMs / 2;
}

/** What the guest counts a run's work by, given the pid of the run's script in `p`, which also leads the run's
 * process group: every process whose parent chain reaches that pid or a member of that group, each counted once
 * for the CPU ticks it and the children it has reaped have burned and the megabytes it has read or written,
 * summed into one number the next poll is compared with. The chain matters because Claude Code starts every Bash
 * tool command, foreground or background, in a session and group of its own, so a build an agent runs is never in
 * the run's group; the group keeps the members whose parent went, the run's input pump among them. The reaped
 * children's ticks (cutime, cstime) keep the sum from falling each time a compiler exits, since a reading below the
 * last is only a new baseline; the kernel folds a reaped child's bytes into its parent's io already. /proc is the
 * only road to it, since the guests have no pgrep, and the comm field is stripped by its last bracket because a
 * process may be named `(sh (2))`. Each file is opened by getline rather than handed to awk as input: every awk
 * aborts on an input file it cannot open, so one process exiting between the shell's glob and the read would void
 * the whole reading. A guest with no /proc reads 0 on every poll, and one with no awk reads nothing; either leaves
 * the turn's idle clock on bytes alone. */
export const TREE_WORK_AWK =
  "BEGIN{for(k=1;k<ARGC;k++){if((getline s<ARGV[k])>0){n=s+0;sub(/^[0-9]+ \\(.*\\) /,\"\",s);split(s,f,\" \");up[n]=f[2];g[n]=f[3];c[n]=f[12]+f[13]+f[14]+f[15];at[n]=ARGV[k]}close(ARGV[k])}" +
  "for(n in up){q=n;for(d=0;d<64;d++){if(q==p||((q in g)&&g[q]==p)){t+=c[n];i=at[n];sub(/stat$/,\"io\",i);while((getline l<i)>0)if(l~/^[rw]char:/){split(l,h,\" \");t+=int(h[2]/1048576)}close(i);break}if(!(q in up))break;q=up[q]}}" +
  "print t+0}";

/** What a turn on a kept process left running in its run's group, off one /proc/[pid]/stat line per process on stdin,
 * given the group in `p` and the input pump's pid in `t`: every process whose parent is outside the group, the leader
 * and the pump aside, and everything under each. The agent and the servers it started have their parents in the
 * group, so they stay. The lines come from the shell, since gawk ends at a stat file whose process went after the glob. */
export const LEFTOVERS_AWK =
  '{s=$0;sub(/^[0-9]+ \\(.*\\) /,"",s);split(s,f," ");if(f[3]==p)up[$1]=f[2]}' +
  'END{for(x in up)if(!(up[x] in up)&&x!=p&&x!=t)l[x]=1;do{g=0;for(x in up)if((up[x] in l)&&!(x in l)){l[x]=1;g=1}}while(g);for(x in l)printf "%s ",x}';

const ENV_KEY = /^[A-Za-z_][A-Za-z0-9_]*$/;
/** What a launch mints a run's name from, so a handle read back off the sessions index is checked against the shape
 * this code writes before it reaches shell text: a value that has been to a file on disk is no longer this code's. */
const RUN_ID = /^[0-9a-f]{12}$/;
/** How often the reap looks at the group it asked to go, while it waits out the one stop grace both roads give. */
const GRACE_POLL_MS = 200;
/** The poll after a write to a run's input, and for how long: at 1500 ms polls a stop on a box took 2.6 s to end its
 * turn, nearly all of it waiting for the poll after the agent's reply had landed (240 ms at these figures). */
const HURRIED_POLL_MS = 200;
const HURRIED_FOR_MS = 3_000;
/** How often a process kept between turns is read while no turn is open: it prints nothing then, and its next turn
 * wakes the reader at once, so the poll is there only to see the process go on its own. Each poll is a shell on the
 * computer, one per kept process. */
const RESTING_POLL_MS = 10_000;
/** That grace as the shell's own counter, since a guest has no seq to lean on. */
const GRACE_CHECKS = Array.from({ length: Math.round(RUN_STOP_MS / GRACE_POLL_MS) }, (_, i) => String(i + 1)).join(" ");
/** What bash writes as the exit of a command SIGKILL ended, the kernel's memory killer included: 128 plus 9. */
const SIGKILL_EXIT = 137;

/** Whether the run is stopped on a question only a person can answer. A harness blocked on a permission prompt
 * prints nothing and burns nothing, so the idle limit would cut the very turn the question is waiting for; the idle
 * clock does not run while this answers true. It rides beside the options rather than in them because it is a fact
 * about one turn and not a limit: the registry hands the turn road no limits, so that road stays the one that runs
 * under the turn's own. */
export type TurnWaiting = () => boolean;

/** What a reader that has let go of a run waits on: nothing settles it, so the stream it handed out ends for
 * nobody and the run is left exactly as it is. Ending the stream here would settle a turn this process has merely
 * stopped reading, and its own owner would read it as over. */
const never = (): Promise<never> => new Promise<never>(() => {});

/** The poll's own wait, and the one call that cuts it short. The timer belongs to the reader, so a reader that
 * lets go of a run frees it where it stands rather than at the end of the poll it was in; a caller that handed in
 * a clock of its own waits on that instead, and the wake ends that wait the same way. */
function pollNap(sleep?: (ms: number) => Promise<void>): { nap: (ms: number) => Promise<void>; wake: () => void; hurry: () => void } {
  let woken: (() => void) | undefined;
  /** Set once the reader has let go. Every nap after that returns where it stands rather than starting a timer:
   * the wake often lands while the poll is inside its exec, and the nap it comes back to would otherwise hold
   * this process for one more poll. */
  let awake = false;
  const nap = (ms: number): Promise<void> => {
    if (awake) return Promise.resolve();
    return new Promise<void>(resolve => {
      let done = false;
      const settle = (): void => {
        if (done) return;
        done = true;
        woken = undefined;
        resolve();
      };
      const timer = sleep === undefined ? setTimeout(settle, ms) : undefined;
      if (sleep !== undefined) void sleep(ms).then(settle);
      woken = () => {
        if (timer !== undefined) clearTimeout(timer);
        settle();
      };
    });
  };
  return {
    nap,
    wake: () => {
      awake = true;
      woken?.();
    },
    hurry: () => woken?.(),
  };
}

export function machineExecStream(machine: Machine, opts: MachineExecOptions = {}, isWaiting?: TurnWaiting): ExecStreamFactory {
  const pollMs = opts.pollMs ?? 1500;
  const idleMs = opts.idleMs ?? TURN_IDLE_MS;
  const waiting = isWaiting ?? (() => false);
  const deadlineMs = opts.deadlineMs ?? TURN_WALL_MS;
  const execTimeoutMs = opts.execTimeoutMs ?? INLINE_EXEC_MS;
  const runDir = opts.runDir ?? RUN_DIR;
  const now = opts.now ?? Date.now;
  const sleep = opts.sleep ?? realRetryClock.sleep;

  /** Every path this file puts in shell text goes through here first. The run folder is the machine's own answer on
   * a kind that has one (a home read over ssh), so a word in it must not be read as shell; a glob or a brace that
   * the shell has to expand keeps its own place outside the quotes. */
  const q = (path: string): string => shellQuote(path);
  /** The one path that says a run is on the machine: the launch makes it, the reap takes it with the rest of the
   * run's files, and every road that asks whether a run is still there asks about this one. */
  const claim = (base: string): string => `${base}.d`;
  /** Hands a run the seed its launch held back: into the channel where nothing reached it yet, then dropped. The
   * channel's size is the check, so a host that went between the two steps leaves no road to a second copy. */
  const releaseHeld = (b: string): string =>
    `if [ -e ${q(b)}.held ]; then if [ -d ${q(claim(b))} ] && [ ! -s ${q(b)}.in ]; then cat ${q(b)}.held >> ${q(b)}.in; fi; rm -f ${q(b)}.held; fi`;
  /** A line written after the seed and held until it is due goes in once, after the seed, onto a run that has not
   * ended. Its key says whether it went, never its words: a thread may send one agent the same words twice, and a host
   * that went between the append and the drop left the key it appended under beside the channel. */
  const releaseLate = (b: string): string =>
    `${releaseHeld(b)}; if [ -e ${q(b)}.late ]; then if [ -d ${q(claim(b))} ] && [ ! -e ${q(b)}.exit ] && [ "$(cat ${q(b)}.late.key 2>/dev/null)" != "$(cat ${q(b)}.late.went 2>/dev/null)" ]; then cat ${q(b)}.late >> ${q(b)}.in && cp ${q(b)}.late.key ${q(b)}.late.went; fi; rm -f ${q(b)}.late ${q(b)}.late.key; fi`;
  /** What ending one run comes to on the guest, in the shell both the reader's own reap and the connect sweep run:
   * the recorded process group gets TERM, then KILL once the stop grace passes, and the run's files go. `read` runs
   * between the two, once nothing of the group is left to write. Without `signal` the group is not sent anything:
   * a group the reader's first reap ended is signalled once, never again. */
  const reapScript = (b: string, read = "", signal = true): string =>
    (signal
      ? `P=$(cat ${q(b)}.pid 2>/dev/null); ` +
        `if [ -n "$P" ]; then kill -TERM -- -$P 2>/dev/null; ` +
        `for i in ${GRACE_CHECKS}; do kill -0 -- -$P 2>/dev/null || break; sleep ${GRACE_POLL_MS / 1000}; done; ` +
        `kill -KILL -- -$P 2>/dev/null; fi; `
      : "") +
    read +
    `rm -rf ${q(b)}.*`;
  /** A handle this factory could have minted: the run directory it launches into and a name of its own shape. */
  const minted = (run: string): boolean => run.startsWith(`${runDir}/`) && RUN_ID.test(run.slice(runDir.length + 1));

  /** The one reader both roads share: a launch that has just posted its script, and an attach to a run an earlier
   * host process left behind. `opened` settles once the run is known to be on the machine and rejects with the words
   * the turn fails on when it is not. The log is read from its first byte either way, so a run that printed while no
   * host was listening is replayed to whoever attaches. */
  const open = (base: string, hasInput: boolean, launched: Promise<void>, turnStartedAt?: number, taken?: readonly string[], posting?: Posting, from?: number, seedDue?: Promise<void>): ExecStream => {
    const sentinel = `__WSP_EOF_${randomBytes(6).toString("hex")}__`;
    const reapMark = `__WSP_REAPED_${randomBytes(6).toString("hex")}__`;
    /** Every shell that hands the channel a line runs after the last one ended: two hand-overs of one held seed that
     * ran together would both read the channel as empty and append the seed twice. */
    let handing: Promise<unknown> = Promise.resolve();
    const oneAtATime = <T>(f: () => Promise<T>): Promise<T> => {
      const next = handing.then(f, f);
      handing = next.catch(() => undefined);
      return next;
    };
    /** Settles once every line parked with writeAfter went in or was dropped; set only while one is parked. A line written
     * meanwhile goes after it, and a second parked line parks once the first is out of the way. */
    let parkedLine: Promise<void> | undefined;

    let killed = false;
    /** Set once this side sent the run a signal: a run it ended is not one the kernel killed. */
    let signalled = false;
    let inputClosed = false;
    /** Set when this process lets go of the run: the poll ends where it stands, nothing is reaped and no exit is
     * written, since ending the run here would take a turn its own owner is still waiting on. */
    let dropped = false;
    const { nap, wake, hurry } = pollNap(opts.sleep);
    // The agent's reply to a write (a stop, a steer, an answer, or an adapter's own line) would otherwise wait out a
    // whole poll: the polls run at HURRIED_POLL_MS for a while after any write.
    let hurriedUntil = 0;
    const gap = (): number => (now() < hurriedUntil ? Math.min(pollMs, HURRIED_POLL_MS) : resting ? RESTING_POLL_MS : pollMs);
    /** The polls run at HURRIED_POLL_MS from now for a while, the one under way woken. */
    const hurryPolls = (): void => {
      hurriedUntil = now() + HURRIED_FOR_MS;
      hurry();
    };
    const drop = (): void => {
      dropped = true;
      opts.reading?.delete(drop);
      wake();
    };
    opts.reading?.add(drop);
    // The idle clock runs from this reader's first second, since nothing on the machine records when the run's last
    // byte landed; the wall runs from the turn's own start, which an attach is handed.
    let startedAt = turnStartedAt ?? now();
    let activity = turnActivity(now());
    /** Between two turns of a process kept for the next one: no limit reads the quiet, since no turn is open to be stuck. */
    let resting = false;
    /** How much of the log this reader has taken: a turn on a process that served earlier turns starts where its
     * re-open was told to. */
    let offset = from ?? 0;
    let finishCode: number | null | undefined;
    let resolveExit: (code: number | null) => void = () => {};
    const exited = new Promise<number | null>(resolve => {
      resolveExit = resolve;
    });
    const finish = (code: number | null): void => {
      if (finishCode === undefined) {
        finishCode = code;
        opts.reading?.delete(drop);
        resolveExit(code);
      }
    };

    /** Whether the machine holds the run, asked for one link window, the wait the launch's own posts were given, or
     * until a stop on a line's try: a launch whose posts went out and whose answer never came may have started it.
     * Only the guest's own answer says it does not; a refusal of the question (a box not connected, a sandbox the
     * provider cannot reach) says nothing about the run, and is asked again, as a machine silent for the whole window
     * or a stop says nothing. Nothing is known while the process lets go of the run. */
    const holdsRun = async (): Promise<{ held: "yes" | "no" | "silent" | "stopped"; asked: number }> => {
      const from = now();
      for (let asked = 0; ; asked++) {
        if (dropped) await never();
        if (killed) return { held: "stopped", asked };
        if (opts.asksUntilStopped !== true && now() - from >= LINK_RETRY_WINDOW_MS) return { held: "silent", asked };
        const res = await machine.exec(`if [ -d ${q(claim(base))} ]; then echo ${HANDSHAKE.run}; else echo ${HANDSHAKE.gone}; fi`, { timeoutMs: execTimeoutMs }).catch(() => undefined);
        if (res?.stdout.includes(HANDSHAKE.run) === true) return { held: "yes", asked: asked + 1 };
        if (res?.stdout.includes(HANDSHAKE.gone) === true) return { held: "no", asked: asked + 1 };
        await nap(pollMs);
      }
    };
    // A run the machine holds after such a launch is read as one that launched, so its agent's lines reach the turn
    // and nothing launches it a second time. One it does not hold never ran, and one it stayed silent about through
    // the window is a computer that did not answer: either may go again, and a line's own clock decides whether it
    // does. A stop keeps the launch's own words.
    const opened = launched.catch(async (e: unknown) => {
      if (posting === undefined || !posting.mayHaveRun()) throw e;
      const { held, asked } = await holdsRun();
      if (held === "yes") return;
      if (held === "silent") throw new MachineUnreached(posting.posts() + asked, now() - posting.since, e);
      if (held === "no" && !(e instanceof MachineUnreached)) throw new LaunchUnanswered(e instanceof Error ? e.message : String(e), { cause: e });
      throw e;
    });

    const signal = (sig: "TERM" | "KILL"): void => {
      signalled = true;
      // A process resting between turns is read at RESTING_POLL_MS, and its end is waited on.
      if (resting) hurryPolls();
      void opened
        .catch(() => undefined)
        .then(() =>
          machine.exec(`P=$(cat ${q(base)}.pid 2>/dev/null); [ -n "$P" ] && kill -${sig} -- -$P 2>/dev/null; true`, {
            timeoutMs: execTimeoutMs,
          }),
        )
        .catch(() => undefined);
    };

    /** Ends the run on the first call and answers a chunk of its log past `offset`, read once the group is gone: a
     * run that outlasted its stop grace printed into a window no poll read. The read stops at `end`, the size the first
     * reap found the log at once the KILL went out, since a process that left the group and still holds the log would
     * otherwise keep the stop going; the files stay while more than the chunk is left, for the next reap to read on
     * from. Answers undefined where nothing came back. */
    const reap = (offset: number, end?: number): Promise<{ chunk: Buffer; end: number } | undefined> => {
      const log = `${q(base)}.log`;
      const read =
        `S=${end ?? `$({ wc -c < ${log}; } 2>/dev/null || echo 0)`}; N=$((S - ${offset})); [ $N -gt ${EXEC_CHUNK_BYTES} ] && N=${EXEC_CHUNK_BYTES}; ` +
        `[ $N -gt 0 ] && tail -c +${offset + 1} ${log} 2>/dev/null | head -c $N | base64 -w0; printf '\\n%s %s\\n' ${reapMark} "$((S))"; ` +
        `[ $((S - ${offset})) -gt ${EXEC_CHUNK_BYTES} ] && exit 0; `;
      return machine.exec(`${reapScript(base, read, end === undefined)}; true`, { timeoutMs: execTimeoutMs }).then(
        res => {
          const out = res.stdout.split("\n");
          const markIdx = out.findIndex(l => l.startsWith(`${reapMark} `));
          const size = markIdx === -1 ? Number.NaN : Number.parseInt(out[markIdx]!.slice(reapMark.length + 1), 10);
          return Number.isSafeInteger(size) ? { chunk: Buffer.from(out.slice(0, markIdx).join(""), "base64"), end: size } : undefined;
        },
        () => undefined,
      );
    };
    const remove = (): Promise<void> =>
      machine.exec(`${reapScript(base, "", false)}; true`, { timeoutMs: execTimeoutMs }).then(() => undefined, () => undefined);

    // The exit file is read before the log, so a poll that sees an exit code reads a log that is complete. The
    // tree's work rides the same poll when it is worth reading, so a turn working in silence costs no exec of its
    // own and a stream nothing could cut asks the guest for nothing; unasked, $W is empty and the reader sees no
    // reading.
    const pollCmd = (offset: number, work: boolean): string =>
      `E=$(cat ${q(base)}.exit 2>/dev/null); ` +
      `tail -c +${offset + 1} ${q(base)}.log 2>/dev/null | head -c ${EXEC_CHUNK_BYTES} | base64 -w0; ` +
      `P=$(cat ${q(base)}.pid 2>/dev/null); ` +
      (work ? `W=$(awk -v p="$P" ${shellQuote(TREE_WORK_AWK)} /proc/[0-9]*/stat 2>/dev/null); ` : "") +
      `printf '\\n${sentinel} %s %s %s\\n' "$E" ` +
      `"$([ -n "$P" ] && kill -0 "$P" 2>/dev/null && echo up || echo down)" "$W"`;

    /** Throws the kind's words for a run that ended by a signal nobody here sent, where it has any. */
    const killedHere = async (): Promise<void> => {
      if (opts.killedWhy === undefined || signalled) return;
      const why = await opts.killedWhy(startedAt);
      if (why !== undefined) throw new Error(why);
    };

    async function* lines(): AsyncGenerator<string> {
      let downs = 0;
      let pending = Buffer.alloc(0);
      /** Every line the log held past the last poll once the KILL went out, read by the reap, and the one it ended on
       * without a newline; the run's files go whatever the reads answered. */
      async function* reaped(): AsyncGenerator<string> {
        let end: number | undefined;
        for (let read = await reap(offset); read !== undefined; read = await reap(offset, end)) {
          end = read.end;
          offset += read.chunk.length;
          pending = Buffer.concat([pending, read.chunk]);
          let nl: number;
          while ((nl = pending.indexOf(0x0a)) !== -1) {
            yield pending.subarray(0, nl).toString("utf8");
            pending = pending.subarray(nl + 1);
          }
          if (read.chunk.length === 0 || offset >= end) break;
        }
        if (end === undefined || offset < end) await remove();
        if (pending.length > 0) yield pending.toString("utf8");
      }

      try {
        await opened;
      } catch (e) {
        yield* reaped();
        finish(null);
        throw e;
      }

      /** Set once a poll took everything the log held at that moment, short of a chunk: until then a limit waits, and
       * it waits too while the last poll saw the exit, so a turn re-opened past its wall keeps what it printed while
       * no host read it, and the reply of a run that finished in that time. */
      let caughtUp = false;
      let exitSeen = false;
      /** When the polls last started coming back with nothing: before the reader has caught up, a cut waits out the
       * same reach window an attach's probe gets, so one gateway error after a re-open does not take the reply. */
      let darkSince: number | undefined;
      async function* cutHere(e: Error): AsyncGenerator<string, never> {
        yield* reaped();
        finish(null);
        throw e;
      }
      async function* cutInTheDark(cut: Error | undefined, at: number): AsyncGenerator<string> {
        darkSince ??= at;
        if (cut !== undefined && (caughtUp || now() - darkSince >= LINK_RETRY_WINDOW_MS)) yield* cutHere(cut);
      }
      while (true) {
        // This process is done reading this run: the poll ends here and the stream settles for nobody.
        if (dropped) await never();
        if (killed) {
          yield* reaped();
          finish(null);
          return;
        }
        const at = now();
        if (waiting()) activity.touch(at);
        const quietMs = activity.quietMs(at);
        const cut = resting ? undefined : turnCut({ idleMs, deadlineMs }, at - startedAt, quietMs);
        if (cut !== undefined && caughtUp && !exitSeen) yield* cutHere(cut);

        let res: ExecResult;
        try {
          res = await machine.exec(pollCmd(offset, !resting && readsWork(idleMs, quietMs)), { timeoutMs: execTimeoutMs });
        } catch {
          // Machine likely napping; polls recover after wake (P10 semantics). A poll that never reached the machine
          // says nothing about the process it was sent to read, so the stretch the road was dark is no part of the
          // turn's silence: the idle clock holds here and goes on from the road's return.
          yield* cutInTheDark(cut, at);
          await nap(pollMs);
          activity.hold(now() - at);
          continue;
        }

        const out = res.stdout.split("\n");
        const markIdx = out.findIndex(l => l.startsWith(sentinel));
        if (markIdx === -1) {
          yield* cutInTheDark(cut, at);
          await nap(pollMs);
          continue;
        }
        darkSince = undefined;
        const [, exitStr = "", live = "up", workStr = ""] = out[markIdx]!.split(" ");
        const work = Number.parseInt(workStr, 10);
        if (Number.isSafeInteger(work)) activity.read(work, now());
        const chunk = Buffer.from(out.slice(0, markIdx).join(""), "base64");
        offset += chunk.length;

        if (chunk.length > 0) {
          activity.touch(now());
          pending = Buffer.concat([pending, chunk]);
          let nl: number;
          while ((nl = pending.indexOf(0x0a)) !== -1) {
            yield pending.subarray(0, nl).toString("utf8");
            pending = pending.subarray(nl + 1);
          }
          if (chunk.length < EXEC_CHUNK_BYTES) caughtUp = true;
          exitSeen = exitStr !== "";
          continue; // there may be more than one chunk buffered up
        }

        if (exitStr !== "") {
          yield* reaped();
          const code = Number.parseInt(exitStr, 10);
          finish(code);
          if (code === SIGKILL_EXIT) await killedHere();
          return;
        }
        caughtUp = true;
        if (cut !== undefined) yield* cutHere(cut);
        // The pid is checked after the exit file: a leader that finished in between shows as down with no exit yet.
        if (live === "down" && ++downs > 1) {
          yield* reaped();
          finish(null);
          await killedHere();
          return;
        }
        await nap(gap());
      }
    }

    /** Ends what the turn just over left running beside the agent, TERM then KILL once the stop grace passes: a command
     * it left going holds the turn's token, which would otherwise outlive the turn by the keep. */
    const endLeftovers = (): void => {
      const script =
        `P=$(cat ${q(base)}.pid 2>/dev/null); T=$(cat ${q(base)}.tail 2>/dev/null); [ -n "$P" ] || exit 0; ` +
        `L=$(for f in /proc/[0-9]*/stat; do read -r s < "$f" && printf '%s\\n' "$s"; done 2>/dev/null | awk -v p="$P" -v t="$T" ${shellQuote(LEFTOVERS_AWK)}); [ -n "$L" ] || exit 0; ` +
        `kill -TERM $L 2>/dev/null; ` +
        `for i in ${GRACE_CHECKS}; do S=; for x in $L; do kill -0 $x 2>/dev/null && S=1; done; [ -n "$S" ] || break; sleep ${GRACE_POLL_MS / 1000}; done; ` +
        `kill -KILL $L 2>/dev/null; true`;
      void opened
        .catch(() => undefined)
        .then(() => machine.exec(script, { timeoutMs: execTimeoutMs }))
        .catch(() => undefined);
    };

    /** One line onto the channel, after a seed still held. */
    const append = async (line: string): Promise<"written" | "gone"> => {
      // The guest knows the command ended the moment its exit file exists, up to a poll before this side does,
      // and a reap in flight has taken the claim with the rest of the run.
      const res = await oneAtATime(() => putFiles(machine, [{ path: `${base}.in`, text: `${line}\n`, append: true }], {
        // Any line the channel takes goes after a seed still held, or the release would read the seed as handed over.
        before: [`{ [ -e ${q(base)}.exit ] || [ ! -d ${q(claim(base))} ]; } && { echo ${HANDSHAKE.gone}; exit 0; }`, releaseHeld(base)],
        after: [`echo ${HANDSHAKE.written}`],
        timeoutMs: execTimeoutMs,
      }));
      if (res.stdout.includes(HANDSHAKE.gone)) return "gone";
      if (res.exitCode !== 0 || !res.stdout.includes(HANDSHAKE.written)) throw new Error(`remote write failed on ${machine.id}: nothing came back saying ${HANDSHAKE.written}, the word the guest prints once the message landed; ${machineAnswer(res)}`);
      // The person just acted, so the turn gets its idle time over.
      activity.touch(now());
      hurryPolls();
      return "written";
    };

    const stream: ExecStream = {
      lines: lines(),
      run: base,
      teardown: () => signal("TERM"),
      kill: () => {
        killed = true;
        signal("KILL");
      },
      write: async line => {
        if (!hasInput) throw new Error("this stream has no input channel");
        if (finishCode !== undefined) throw new Error("the stream has ended");
        // Only what was parked before this write goes ahead of it: a kept Codex writes thread/resume and parks its
        // turn/start in the same tick, and a turn/start on a thread the server let go is never answered.
        const ahead = parkedLine;
        await opened;
        await ahead;
        return append(line);
      },
      closeInput: () => {
        if (!hasInput || inputClosed || finishCode !== undefined) return;
        inputClosed = true;
        if (resting) hurryPolls();
        void opened
          .catch(() => undefined)
          .then(async () => {
            while (parkedLine !== undefined) await parkedLine;
          })
          .then(() => oneAtATime(() => machine.exec(`${releaseHeld(base)}; P=$(cat ${q(base)}.tail 2>/dev/null); [ -n "$P" ] && kill -TERM "$P" 2>/dev/null; true`, { timeoutMs: execTimeoutMs })))
          .catch(() => undefined);
      },
      writeAfter: (line, after) => {
        if (!hasInput) throw new Error("this stream has no input channel");
        const before = parkedLine ?? Promise.resolve();
        let settled: () => void = () => {};
        const mine: Promise<void> = new Promise(resolve => (settled = resolve));
        parkedLine = mine;
        // Beside the run rather than in this process, so a host that goes before the line is due leaves it for the next.
        // Its key lands first, so a parked line always has one; both are the login's alone, as every file of the run is.
        const park = async (): Promise<void> => {
          await before;
          const parked = await opened
            .then(() => putFiles(machine, [{ path: `${base}.late.key`, text: randomBytes(6).toString("hex") }, { path: `${base}.late`, text: `${line}\n` }], { before: ["umask 077"], timeoutMs: execTimeoutMs }))
            .then(
              () => true,
              () => false,
            );
          const due = await after.then(
            () => true,
            () => false,
          );
          if (!due) {
            if (parked) await machine.exec(`rm -f ${q(base)}.late ${q(base)}.late.key`, { timeoutMs: execTimeoutMs });
            return;
          }
          if (!parked) return void (await append(line));
          await oneAtATime(() => untilReached(() => machine.exec(releaseLate(base), { timeoutMs: execTimeoutMs }), { now, sleep }));
          activity.touch(now());
          hurryPolls();
        };
        void park()
          .catch(() => undefined)
          .finally(() => {
            if (parkedLine === mine) parkedLine = undefined;
            settled();
          });
      },
      rest: () => {
        resting = true;
        endLeftovers();
      },
      nextTurn: () => {
        resting = false;
        startedAt = now();
        activity = turnActivity(now());
        hurry();
        return offset;
      },
      exited,
      ...(taken !== undefined ? { taken } : {}),
    };
    // A launch that failed fails the stream on its own; a seed that never comes due, or never reaches the run, ends it.
    if (seedDue !== undefined)
      void seedDue
        .then(() => opened.then(() => oneAtATime(() => untilReached(() => machine.exec(releaseHeld(base), { timeoutMs: execTimeoutMs }), { now, sleep })), () => undefined))
        .catch(() => stream.kill());
    return stream;
  };

  const factory: ExecStreamFactory = (command, { env, input, inputAfter, secret }) => {
    const base = `${runDir}/${randomBytes(6).toString("hex")}`;
    // A seed that is not due yet waits on the guest rather than in this process, so a host that goes before it is due
    // leaves it for the next one, and the command starts meanwhile.
    const holdsSeed = input !== undefined && inputAfter !== undefined;
    const seedPath = `${base}.${holdsSeed ? "held" : "in"}`;
    if (secret !== undefined && machine.takesStdin !== true) throw new Error(`${machine.id} takes no input on its launch, so text holding a value would ride its command; nothing was started`);
    // Text that holds values rides the launch's input under names of its own, is written to the run's files under
    // its mask and leaves the shell before the run starts, so the run's environment never carries it.
    const held = [
      ...Object.entries(secret?.files ?? {}).map(([variable, text], i) => ({ name: `WSP_LAND_${i}`, path: `${base}.f${i}`, text, variable })),
      ...(secret?.input === true && input !== undefined ? [{ name: "WSP_LAND_IN", path: seedPath, text: input.map(line => `${line}\n`).join(""), variable: undefined }] : []),
    ];

    // The environment reaches the run as the one setsid starts it under and never sits in its script. A machine
    // that takes stdin reads it off the launch exec's input, since its command line is world-readable there; a
    // provider's exec drops stdin, so on those single-login machines it rides the launch's text, which the images
    // wsp builds keep sudo and the box login's systemd manager from logging (QUIET_LOGS).
    const named = Object.fromEntries(Object.entries(env).filter(([k]) => ENV_KEY.test(k)));
    const envLines = machine.takesStdin === true ? [ENV_FROM_INPUT] : Object.entries(named).map(([k, v]) => `export ${k}=${shellQuote(v)}`);
    const heldLines = held.flatMap(h => [`printf %s "$${h.name}" > ${q(h.path)} || exit 1`, `unset ${h.name}`, ...(h.variable !== undefined ? [`export ${h.variable}=${q(h.path)}`] : [])]);
    // The tail starts in a subshell so bash's job notice for its kill never lands in the log; the command's exit code
    // is written before the tail is killed, so a poll that sees it reads a finished log.
    const run =
      input === undefined
        ? `${command}\necho $? > ${q(base)}.exit\n`
        : `( tail -n +1 -f ${q(base)}.in > ${q(base)}.fifo & echo $! > ${q(base)}.tail )\n{ ${command}\n} < ${q(base)}.fifo\necho $? > ${q(base)}.exit\nkill $(cat ${q(base)}.tail) 2>/dev/null\n`;
    // The turn's processes are what the kernel takes first when memory runs out: the work outgrew the machine, and
    // the daemon and the guest agent are how anyone hears of it.
    const files: GuestWrite[] = [{ path: `${base}.sh`, text: `${workScoreLine()}\n${run}` }];
    if (input !== undefined && !held.some(h => h.variable === undefined)) files.push({ path: seedPath, text: input.map(line => `${line}\n`).join("") });

    // Whether a post of the launch went out and got no answer: one that failed before it left this computer started
    // nothing there, and one the far end answered, a refusal included, said what came of it.
    let mayHaveRun = false;
    let posts = 0;
    const since = now();
    // Spawn eagerly, like a local child process would: a held seed's one more exec trip goes while the command starts.
    const posted: Promise<ExecResult> = untilReached(
      () => {
        posts++;
        return putFiles(opts.launchOn ?? machine, files, {
          // exec honours no idempotency key and a launch whose answer was lost is retried; the claim makes the second
          // a no-op. A mkdir that fails for any other reason (a run folder another login on the machine owns) fails
          // the launch: read as a replay it would answer launched and leave the reader polling a log nobody writes.
          // Every file of the run is the login's alone from the moment it is made, not chmodded after: the input
          // and the log carry the person's messages and the agent's output, and a machine somebody owns may carry
          // other logins that can read a folder wsp did not make.
          before: ["umask 077", `mkdir ${q(claim(base))} 2>/dev/null || { [ -d ${q(claim(base))} ] && { echo ${HANDSHAKE.launched}; exit 0; }; echo ${q(`no run folder on this machine: ${claim(base)}`)} >&2; exit 1; }`],
          // The agent writes its pid beside the run, where that computer's daemon reads it before any kill, since this
          // host knows no pid there.
          after: [`export ${AGENT_PID_FILE_ENV}=${q(`${base}.agent`)}`, ...(holdsSeed ? [`: > ${q(base)}.in`] : []), ...(input === undefined ? [] : [`mkfifo ${q(base)}.fifo`]), ...envLines, ...heldLines, `setsid bash ${q(base)}.sh > ${q(base)}.log 2>&1 & echo $! > ${q(base)}.pid; echo ${HANDSHAKE.launched}`],
          timeoutMs: execTimeoutMs,
          ...(machine.takesStdin === true ? { stdin: envInput({ ...named, ...Object.fromEntries(held.map(h => [h.name, h.text])) }) } : {}),
        }).catch((e: unknown) => {
          if (!neverSent(e) && !farEndAnswered(e)) mayHaveRun = true;
          throw e;
        });
      },
      { now, sleep },
    );
    const opened = posted.then(
      res => {
        if (res.exitCode !== 0 || !res.stdout.includes(HANDSHAKE.launched)) throw new Error(`remote launch failed on ${machine.id}: nothing came back saying ${HANDSHAKE.launched}, the word the guest prints once the run is up; ${machineAnswer(res)}`);
      },
      (e: unknown) => {
        // Each carries the one sentence every surface says about it, and a machine id wrapped round it is nothing a person reads.
        if (e instanceof MachineUnreached || e instanceof MachineUnreachableError || isPlaceAbsent(e)) throw e;
        throw new Error(`remote launch failed on ${machine.id}: ${e instanceof Error ? e.message : String(e)}`);
      },
    );
    return open(base, input !== undefined, opened, undefined, undefined, { since, posts: () => posts, mayHaveRun: () => mayHaveRun }, undefined, holdsSeed ? inputAfter : undefined);
  };

  factory.attach = async (run, { input, startedAt, from }) => {
    if (!minted(run)) throw new Error(`${run} is not a run this host could have launched`);
    // With the run's channel read back under the answer: the lines it took, which a re-opened turn reads for what its
    // harness was handed before this host came up. The agent can write that file, so only its last EXEC_CHUNK_BYTES
    // come back, whole lines alone: a line before them, most often a turn's opening with its images, is not read, and
    // a message steered before them is neither waited on nor sent again.
    const channel = input
      ? ` n=$({ wc -c < ${q(run)}.in; } 2>/dev/null || echo 0); if [ "$n" -gt ${EXEC_CHUNK_BYTES} ]; then tail -c ${EXEC_CHUNK_BYTES + 1} ${q(run)}.in | sed 1d; else cat ${q(run)}.in 2>/dev/null; fi;`
      : "";
    const res = await untilReached(() => machine.exec(`if [ -d ${q(claim(run))} ]; then echo ${HANDSHAKE.run};${input ? ` ${releaseLate(run)};` : ""}${channel} else echo ${HANDSHAKE.gone}; fi`, { timeoutMs: execTimeoutMs }), { now, sleep });
    // Only these two answers say anything about the run. Anything else is the machine failing to answer the
    // question, which is the unreached road, not a run to end: the reader is built and the run swept on WSP_GONE
    // alone, so nothing here can take a live turn's process group with it.
    const said = res.stdout.split("\n");
    const at = said.findIndex(line => line.includes(HANDSHAKE.run));
    if (at >= 0) return open(run, input, Promise.resolve(), startedAt, input ? said.slice(at + 1).filter(line => line !== "") : undefined, undefined, from);
    if (res.stdout.includes(HANDSHAKE.gone)) return "gone";
    throw new Error(`the machine did not answer whether it still holds ${run}; ${machineAnswer(res)}`);
  };

  factory.sweep = async keep => {
    // The claims are what the machine holds, so the machine is asked what is there rather than told; the shape a
    // handle must have to be one of this factory's is read here, by the same predicate the attach road reads, so
    // nothing the guest wrote into the run directory reaches shell text on the strength of being there.
    const listed = await machine.exec(`for d in ${q(runDir)}/*.d; do [ -d "$d" ] && printf '%s\\n' "$d"; done`, { timeoutMs: execTimeoutMs });
    const kept = new Set(keep);
    const stale = listed.stdout
      .split("\n")
      .map(line => line.trim())
      .filter(line => line.endsWith(".d"))
      .map(line => line.slice(0, -".d".length))
      .filter(base => minted(base) && !kept.has(base));
    if (stale.length === 0) return [];
    // Each run's group is ended beside the others, not after them: every reap waits out its own stop grace, and a
    // machine holding a day of them would spend that grace once per run in a connect that has to end.
    const page = (bases: readonly string[]): string => `${bases.map(base => `{ ${reapScript(base)}; } &`).join(" ")} wait; true`;
    // The machine holding the most stale runs is the one this exists for, and it is the one whose command would pass
    // the exec body cap and be refused whole, so the reaps go a page at a time under the same rule every upload reads.
    const pages: string[][] = [[]];
    for (const base of stale) {
      const last = pages.at(-1)!;
      if (last.length > 0 && !execFits(page([...last, base]))) pages.push([base]);
      else last.push(base);
    }
    for (const bases of pages) await machine.exec(page(bases), { timeoutMs: execTimeoutMs });
    return stale;
  };

  return factory;
}
