// SPDX-License-Identifier: AGPL-3.0-only
//! The `ExecStreamFactory` of a turn on this computer: a detached bash writing the turn's output to a log file and
//! its code to an exit file under the state folder's `runs/`, a reader that polls the log from its first poll, the
//! attach by handle and the sweep. Port of packages/runtime/src/local-exec.ts:437-794 at 65f6b89c7. Spec:
//! spec/local_exec_stream.md.
//!
//! Private state sits behind a `std::sync::Mutex` that is never held across an `.await` or a call out (rule 3.3).
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, FileExt, OpenOptionsExt};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use futures::future::{LocalBoxFuture, Shared};
use futures::stream::LocalBoxStream;
use futures::FutureExt;
use tokio::sync::oneshot;
use tokio::task::spawn_local;
use wsp_adapters::port::{
    AttachRunOptions, Attached, ExecFailure, ExecFailureKind, ExecOptions, ExecStream, ExecStreamFactory, Settle, SharedFuture,
    WriteOutcome, AGENT_PID_FILE_ENV,
};
use wsp_wire::types::place_state::TURN_WALL_MS;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::wire::limits::{EXEC_CHUNK_BYTES, RUN_STOP_MS, TURN_IDLE_MS};

use super::reap::{
    alive, append, end_marker, group_rows, leftovers, lock, node_error, pid_in, pump_of, read_file, reap_run, signal_group, signal_pid,
    tree_work, try_release_held, try_release_late, Lines, TreeWorkReader,
};
use crate::clock::{Clock, RealClock};
use crate::ids::{Ids, OsIds};
use crate::machine_exec::{reads_work, turn_activity, turn_cut, NowFn, ReadingSet, TurnActivity, TurnLimits, TurnWaiting};

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
    pub ids: Option<Arc<dyn Ids>>,
}

impl LocalExecOptions {
    /// Every option at its default but the two folders.
    pub fn new(root: impl Into<String>, run_dir: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            run_dir: run_dir.into(),
            idle_ms: None,
            deadline_ms: None,
            now: None,
            poll_ms: None,
            read_work: None,
            reading: None,
            ids: None,
        }
    }
}

/// The options and the waiting question a factory's streams and sweeps share.
struct Cfg {
    opts: LocalExecOptions,
    waiting: TurnWaiting,
}

pub struct LocalExecFactory {
    cfg: Arc<Cfg>,
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
    let waiting = waiting.unwrap_or_else(|| Arc::new(|| false));
    Arc::new(LocalExecFactory { cfg: Arc::new(Cfg { opts, waiting }) })
}

/// A launch's refusal, as the TS throws it.
fn fail(message: impl Into<String>) -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Other, message: message.into() }
}

fn node_fail(e: io::Error, syscall: &str, path: &str) -> ExecFailure {
    fail(node_error(&e, syscall, Some(path)))
}

/// Milliseconds since a reader's start, as `turn_cut` reads them: a NaN start never reaches the wall (JS compares false
/// on NaN), and an infinite one reaches it or never as the JS difference does.
fn elapsed_ms(at: i64, started_at: f64) -> i64 {
    let elapsed = at as f64 - started_at;
    if elapsed.is_nan() {
        i64::MIN
    } else {
        elapsed as i64
    }
}

/// A file the owner alone reads, created exclusively (TS `writeOwned`, flag `wx`).
fn write_owned(path: &str, text: impl AsRef<[u8]>) -> io::Result<()> {
    OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?.write_all(text.as_ref())
}

/// What one reader of a run needs, read off the factory's options once per stream.
#[derive(Default)]
struct Open {
    started_at: Option<f64>,
    from: Option<u64>,
    taken: Option<Vec<String>>,
}

impl Cfg {
    fn limits(&self) -> TurnLimits {
        TurnLimits {
            idle_ms: self.opts.idle_ms.unwrap_or(TURN_IDLE_MS as f64),
            deadline_ms: self.opts.deadline_ms.unwrap_or(TURN_WALL_MS as f64),
        }
    }
    fn now_fn(&self) -> NowFn {
        match &self.opts.now {
            Some(now) => now.clone(),
            None => Arc::new(|| RealClock.now()),
        }
    }
    fn ids(&self) -> Arc<dyn Ids> {
        match &self.opts.ids {
            Some(ids) => ids.clone(),
            None => Arc::new(OsIds),
        }
    }
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
        let cfg = &*self.cfg;
        let ExecOptions { env, input, input_after, secret } = options;
        if secret.is_some() {
            return Err(fail("a launch on this computer takes no text holding a value; nothing was started"));
        }
        if let Some((key, value)) = env.iter().find(|(_, value)| value.contains('\0')) {
            let received = value.replace('\0', "\\x00");
            return Err(fail(format!("The property 'options.env['{key}']' must be a string without null bytes. Received '{received}'")));
        }
        let run_dir = &cfg.opts.run_dir;
        let base = format!("{run_dir}/{}", hex::encode(cfg.ids().bytes(6)));
        DirBuilder::new().recursive(true).mode(0o700).create(run_dir).map_err(|e| node_fail(e, "mkdir", run_dir))?;
        DirBuilder::new().mode(0o700).create(format!("{base}.d")).map_err(|e| node_fail(e, "mkdir", &format!("{base}.d")))?;
        let file = |suffix: &str| format!("{base}.{suffix}");
        write_owned(&file("sh"), launch_script(&base, command, input.is_some())).map_err(|e| node_fail(e, "open", &file("sh")))?;
        let seed: Option<String> = input.as_ref().map(|lines| lines.iter().map(|line| format!("{line}\n")).collect());
        if let Some(seed) = &seed {
            let channel = if input_after.is_some() { "" } else { seed.as_str() };
            write_owned(&file("in"), channel).map_err(|e| node_fail(e, "open", &file("in")))?;
            if input_after.is_some() {
                write_owned(&file("held"), seed).map_err(|e| node_fail(e, "open", &file("held")))?;
            }
        }
        write_owned(&file("log"), "").map_err(|e| node_fail(e, "open", &file("log")))?;
        let log = OpenOptions::new().append(true).open(file("log")).map_err(|e| node_fail(e, "open", &file("log")))?;
        let log_err = log.try_clone().map_err(|e| node_fail(e, "open", &file("log")))?;

        let mut cmd = tokio::process::Command::new("bash");
        cmd.arg(format!("{base}.sh"))
            .current_dir(&cfg.opts.root)
            .env_clear()
            .envs(env.iter())
            .env(AGENT_PID_FILE_ENV, format!("{base}.agent"))
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(log_err);
        // SAFETY: between fork and exec this resets every signal to its default, as libuv's child init does, then calls
        // only setsid, as the TS detached spawn does.
        unsafe {
            cmd.pre_exec(|| {
                for sig in 1..32 {
                    libc::signal(sig, libc::SIG_DFL);
                }
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let launch_failed = match cmd.spawn() {
            Ok(mut child) => {
                if let Some(pid) = child.id() {
                    let _ = fs::write(format!("{base}.pid"), format!("{pid}\n"));
                }
                spawn_local(async move {
                    let _ = child.wait().await;
                });
                false
            }
            Err(_) => true,
        };

        let stream = open(cfg, base.clone(), input.is_some(), launch_failed, Open::default());
        if let (Some(_), Some(after)) = (seed, input_after) {
            let killer = stream.clone();
            spawn_local(async move {
                match after.await {
                    Ok(()) => {
                        if try_release_held(&base).is_err() {
                            killer.kill();
                        }
                    }
                    Err(_) => killer.kill(),
                }
            });
        }
        Ok(stream)
    }
}

impl ExecStreamFactory for LocalExecFactory {
    /// `try_launch`, with a refusal answered as a stream whose `lines` yields it once and whose `exited` is `None`.
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        match self.try_launch(command, options) {
            Ok(stream) => stream,
            Err(failure) => {
                let lines = Lines::new();
                lines.fail(failure);
                let (tx, rx) = oneshot::channel::<Option<i32>>();
                let _ = tx.send(None);
                Arc::new(RefusedStream { lines, exited: async move { rx.await.ok().flatten() }.boxed_local().shared() })
            }
        }
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
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<LocalBoxFuture<'static, Result<Attached, ExecFailure>>> {
        let cfg = self.cfg.clone();
        let run = run.to_string();
        Some(Box::pin(async move { attach_run(&cfg, &run, options).await }))
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
    fn sweep(&self, keep: &[String]) -> Option<LocalBoxFuture<'static, Result<Vec<String>, ExecFailure>>> {
        let cfg = self.cfg.clone();
        let keep: Vec<String> = keep.to_vec();
        Some(Box::pin(async move {
            let run_dir = cfg.opts.run_dir.clone();
            let Ok(entries) = fs::read_dir(&run_dir) else { return Ok(Vec::new()) };
            let mut names: Vec<String> = entries.flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
            names.sort();
            let stale: Vec<String> = names
                .iter()
                .filter_map(|name| name.strip_suffix(".d").map(|id| format!("{run_dir}/{id}")))
                .filter(|base| minted(&run_dir, base) && !keep.contains(base))
                .collect();
            let reaped = futures::future::join_all(stale.iter().map(|base| reap_run(base, None, RUN_STOP_MS as u64))).await;
            reaped.into_iter().try_for_each(|r| r.map_err(fail))?;
            Ok(stale)
        }))
    }
}

/// A handle this factory could have minted: `run_dir/` and 12 lowercase hex.
// TS local-exec.ts:446-446
//   const minted = (run: string): boolean => run.startsWith(`${runDir}/`) && RUN_ID.test(run.slice(runDir.length + 1));
pub fn minted(run_dir: &str, run: &str) -> bool {
    run.strip_prefix(run_dir)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|id| id.len() == 12 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

/// The attach road: a run this host could have launched, re-opened by its handle.
async fn attach_run(cfg: &Cfg, run: &str, options: AttachRunOptions) -> Result<Attached, ExecFailure> {
    let AttachRunOptions { input, started_at, from } = options;
    if !minted(&cfg.opts.run_dir, run) {
        return Err(fail(format!("{run} is not a run this host could have launched")));
    }
    if !Path::new(&format!("{run}.d")).exists() {
        return Ok(Attached::Gone);
    }
    if input {
        try_release_held(run).map_err(|e| fail(node_error(&e, "write", None)))?;
    }
    let channel = format!("{run}.in");
    if input && fs::metadata(&channel).is_ok_and(|m| m.len() == 0) {
        reap_run(run, None, RUN_STOP_MS as u64).await.map_err(fail)?;
        return Ok(Attached::Gone);
    }
    let at = Open { started_at: Some(started_at), from, taken: None };
    if !input {
        return Ok(Attached::Stream(open(cfg, run.to_string(), input, false, at)));
    }
    try_release_late(run).map_err(fail)?;
    let marker = end_marker(run);
    let taken: Vec<String> =
        read_file(&channel).unwrap_or_default().split('\n').filter(|line| !line.is_empty() && *line != marker).map(String::from).collect();
    Ok(Attached::Stream(open(cfg, run.to_string(), input, false, Open { taken: Some(taken), ..at })))
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
    let exit = shell_quote(&format!("{base}.exit"));
    if !has_input {
        return format!("( {command}\n)\necho $? > {exit}\n");
    }
    let pump = shell_quote(&format!("{base}.pump"));
    let feed = shell_quote(&format!("echo $$ >> {pump}; exec tail -n +1 -f {}", shell_quote(&format!("{base}.in"))));
    let ppid = shell_quote(&format!("echo $PPID >> {pump}"));
    let end = shell_quote(&end_marker(base));
    format!(
        "( {command}\n) < <(sh -c {feed} | {{ sh -c {ppid}; while IFS= read -r l; do [ \"$l\" = {end} ] && break; printf '%s\\n' \"$l\"; done; }} &)\necho $? > {exit}\n"
    )
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
    inner: Arc<Inner>,
    pid: Option<u32>,
    taken: Option<Vec<String>>,
    exited: SharedFuture<Option<i32>>,
}

/// One run's reader state; its lock is never held across an await.
struct Inner {
    base: String,
    has_input: bool,
    limits: TurnLimits,
    poll_ms: u64,
    now: NowFn,
    ids: Arc<dyn Ids>,
    waiting: TurnWaiting,
    read_work: TreeWorkReader,
    reading_set: Option<Arc<ReadingSet>>,
    launch_failed: bool,
    lines: Lines,
    state: Mutex<State>,
}

/// A line parked with `write_after`, settled once it went in or was dropped.
struct Parked {
    seq: u64,
    done: Shared<LocalBoxFuture<'static, ()>>,
}

struct State {
    started_at: f64,
    activity: TurnActivity,
    resting: bool,
    killed: bool,
    input_closed: bool,
    parked: Option<Parked>,
    seq: u64,
    finished: bool,
    exit_tx: Option<oneshot::Sender<Option<i32>>>,
    offset: u64,
    downs: u32,
    reading: bool,
    caught_up: bool,
    exit_seen: bool,
    dropped: bool,
    wake: Option<oneshot::Sender<()>>,
    reading_key: Option<u64>,
}

impl Inner {
    fn now(&self) -> i64 {
        (self.now)()
    }

    /// The leader's pid as the launch recorded it, while the file reads as a positive number.
    fn leader(&self) -> Option<i32> {
        pid_in(&format!("{}.pid", self.base))
    }

    fn signal(&self, sig: i32) {
        let Some(pid) = self.leader() else { return };
        if lock(&self.state).finished {
            return;
        }
        signal_group(pid, sig);
    }

    fn kill(&self) {
        lock(&self.state).killed = true;
        self.signal(libc::SIGKILL);
    }

    fn wake(&self) {
        let wake = lock(&self.state).wake.take();
        if let Some(wake) = wake {
            let _ = wake.send(());
        }
    }

    fn let_go(&self) {
        let (wake, key) = {
            let mut st = lock(&self.state);
            st.dropped = true;
            (st.wake.take(), st.reading_key)
        };
        if let Some(wake) = wake {
            let _ = wake.send(());
        }
        if let (Some(set), Some(key)) = (&self.reading_set, key) {
            set.remove(key);
        }
    }

    fn read_exit(&self) -> Option<String> {
        read_file(&format!("{}.exit", self.base)).map(|text| text.trim().to_string())
    }

    /// What the log holds past what was read, up to `end`, at most one chunk; empty where the log cannot be read.
    fn read_log(&self, end: u64) -> Vec<u8> {
        let log = format!("{}.log", self.base);
        let Ok(meta) = fs::metadata(&log) else { return Vec::new() };
        let offset = lock(&self.state).offset;
        let size = meta.len().min(end);
        if size <= offset {
            return Vec::new();
        }
        let Ok(file) = File::open(&log) else { return Vec::new() };
        let mut buf = vec![0u8; ((size - offset) as usize).min(EXEC_CHUNK_BYTES as usize)];
        let Ok(read) = file.read_at(&mut buf, offset) else { return Vec::new() };
        buf.truncate(read);
        lock(&self.state).offset = offset + read as u64;
        buf
    }

    fn finish(&self, code: Option<i32>) {
        let mut st = lock(&self.state);
        if st.finished {
            return;
        }
        st.finished = true;
        if let Some(tx) = st.exit_tx.take() {
            let _ = tx.send(code);
        }
    }

    async fn nap(&self, ms: u64) {
        let (tx, rx) = oneshot::channel();
        lock(&self.state).wake = Some(tx);
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(ms)) => {}
            _ = rx => {}
        }
        lock(&self.state).wake = None;
    }

    /// Waits for every line parked with `write_after` to go in or be dropped.
    async fn parked_out(&self) {
        loop {
            let done = lock(&self.state).parked.as_ref().map(|parked| parked.done.clone());
            match done {
                Some(done) => done.await,
                None => break,
            }
        }
    }

    fn close_channel(&self) {
        let _ = try_release_held(&self.base)
            .and_then(|_| append(&format!("{}.in", self.base), format!("{}\n", end_marker(&self.base)).as_bytes()));
    }

    /// Writes a parked line's key and the line itself, both owner-only; false where either cannot be made.
    fn park_files(&self, line: &str) -> bool {
        let key = hex::encode(self.ids.bytes(6));
        write_owned(&format!("{}.late.key", self.base), key).is_ok()
            && write_owned(&format!("{}.late", self.base), format!("{line}\n")).is_ok()
    }
}

/// The poll of one run: the limits, the exit file and the log, until the run settles or this process lets go.
async fn poll(inner: Arc<Inner>) {
    loop {
        {
            let st = lock(&inner.state);
            if st.finished || st.dropped {
                return;
            }
        }
        if lock(&inner.state).killed || inner.launch_failed {
            return settle(&inner, None, None).await;
        }
        let at = inner.now();
        if (inner.waiting)() {
            lock(&inner.state).activity.touch(at);
        }
        let quiet = lock(&inner.state).activity.quiet_ms(at);
        let pid = inner.leader();
        if let Some(pid) = pid {
            let start = {
                let mut st = lock(&inner.state);
                let start = !st.reading && !st.resting && reads_work(inner.limits.idle_ms, quiet);
                st.reading = st.reading || start;
                start
            };
            if start {
                spawn_local(read_tree(inner.clone(), pid, at));
            }
        }
        let (resting, started_at) = {
            let st = lock(&inner.state);
            (st.resting, st.started_at)
        };
        let cut = if resting { None } else { turn_cut(inner.limits, elapsed_ms(at, started_at), quiet) };
        let cut_now = {
            let st = lock(&inner.state);
            cut.is_some() && st.caught_up && !st.exit_seen
        };
        if cut_now {
            return settle(&inner, None, cut).await;
        }

        // The exit file is read before the log, so a poll that sees an exit code reads a log that is complete.
        let ended = inner.read_exit();
        let chunk = inner.read_log(u64::MAX);
        if !chunk.is_empty() {
            let now = inner.now();
            {
                let mut st = lock(&inner.state);
                st.activity.touch(now);
                if chunk.len() < EXEC_CHUNK_BYTES as usize {
                    st.caught_up = true;
                }
                st.exit_seen = ended.as_deref().is_some_and(|code| !code.is_empty());
            }
            inner.lines.feed(&chunk);
            tokio::task::yield_now().await;
            continue;
        }
        if let Some(code) = ended.as_deref().filter(|code| !code.is_empty()) {
            return settle(&inner, parse_int(code), None).await;
        }
        lock(&inner.state).caught_up = true;
        if cut.is_some() {
            return settle(&inner, None, cut).await;
        }
        if pid.is_none_or(|pid| !alive(pid)) {
            let downs = {
                let mut st = lock(&inner.state);
                st.downs += 1;
                st.downs
            };
            if downs > 1 {
                return settle(&inner, None, None).await;
            }
        }
        let ms = if resting { RESTING_POLL_MS } else { inner.poll_ms };
        inner.nap(ms).await;
    }
}

/// One tree reading, taken off the poll's clock; its answer moves the idle clock only when the tree is busy.
async fn read_tree(inner: Arc<Inner>, pid: i32, at: i64) {
    let ticks = (inner.read_work)(pid).await;
    let mut st = lock(&inner.state);
    st.reading = false;
    if let Some(ticks) = ticks {
        st.activity.read(ticks, at);
    }
}

/// Reaps the run, hands over the rest of its log, then settles its lines and exit code.
async fn settle(inner: &Arc<Inner>, code: Option<i32>, cut: Option<ExecFailure>) {
    let key = lock(&inner.state).reading_key;
    if let (Some(set), Some(key)) = (&inner.reading_set, key) {
        set.remove(key);
    }
    let reader = inner.clone();
    let last_read: Box<dyn FnOnce(u64) -> LocalBoxFuture<'static, ()>> = Box::new(move |end: u64| {
        let reader = reader.clone();
        Box::pin(async move {
            loop {
                let chunk = reader.read_log(end);
                if chunk.is_empty() {
                    break;
                }
                reader.lines.feed(&chunk);
                tokio::task::yield_now().await;
            }
        })
    });
    if let Err(message) = reap_run(&inner.base, Some(last_read), RUN_STOP_MS as u64).await {
        inner.lines.fail(fail(message));
        inner.finish(None);
        return;
    }
    match cut {
        Some(cut) => inner.lines.fail(cut),
        None => inner.lines.end(),
    }
    inner.finish(code);
}

/// `parseInt(text, 10)` as an exit code: the sign and digits that lead the text, `None` where no digit leads.
fn parse_int(text: &str) -> Option<i32> {
    let text = text.trim_start();
    let (sign, rest) = match text.as_bytes().first() {
        Some(b'-') => (-1i64, &text[1..]),
        Some(b'+') => (1, &text[1..]),
        _ => (1, text),
    };
    let digits = &rest[..rest.bytes().take_while(u8::is_ascii_digit).count()];
    if digits.is_empty() {
        return None;
    }
    i32::try_from(sign * digits.parse::<i64>().ok()?).ok()
}

/// Ends a parked line: its file goes in once `after` resolves, and its marker is cleared when it is the last parked.
async fn park(
    inner: Arc<Inner>,
    line: String,
    made: Option<bool>,
    due: oneshot::Receiver<bool>,
    seq: u64,
    done: oneshot::Sender<()>,
    before: Option<Shared<LocalBoxFuture<'static, ()>>>,
) {
    if let Some(before) = before {
        before.await;
    }
    let made = made.unwrap_or_else(|| inner.park_files(&line));
    if made {
        if due.await.unwrap_or(false) {
            drop(try_release_late(&inner.base));
        } else {
            let _ = fs::remove_file(format!("{}.late", inner.base));
            let _ = fs::remove_file(format!("{}.late.key", inner.base));
        }
    }
    {
        let mut st = lock(&inner.state);
        if st.parked.as_ref().is_some_and(|parked| parked.seq == seq) {
            st.parked = None;
        }
    }
    let _ = done.send(());
}

/// Writes one line to the run's input, after a seed still held and every line parked before it.
async fn write_line(inner: Arc<Inner>, line: String) -> Result<WriteOutcome, ExecFailure> {
    if !inner.has_input {
        return Err(fail("this stream has no input channel"));
    }
    inner.parked_out().await;
    {
        let st = lock(&inner.state);
        if st.finished || st.input_closed {
            return Ok(WriteOutcome::Gone);
        }
    }
    let base = &inner.base;
    if Path::new(&format!("{base}.exit")).exists() || !Path::new(&format!("{base}.d")).exists() {
        return Ok(WriteOutcome::Gone);
    }
    if try_release_held(base).and_then(|_| append(&format!("{base}.in"), format!("{line}\n").as_bytes())).is_err() {
        return Ok(WriteOutcome::Gone);
    }
    let now = inner.now();
    lock(&inner.state).activity.touch(now);
    Ok(WriteOutcome::Written)
}

/// Ends the leftovers of a run kept between turns: TERM now, KILL after the stop grace for any still alive.
async fn end_leftovers(inner: Arc<Inner>) {
    let Some(pid) = inner.leader() else { return };
    let rows = group_rows(pid).await;
    let left = leftovers(&rows, i64::from(pid), &pump_of(&inner.base));
    let pids: Vec<i32> = left.iter().filter_map(|&stray| i32::try_from(stray).ok()).collect();
    for &stray in &pids {
        signal_pid(stray, libc::SIGTERM);
    }
    if !pids.is_empty() {
        spawn_local(async move {
            tokio::time::sleep(Duration::from_millis(RUN_STOP_MS as u64)).await;
            for stray in pids.into_iter().filter(|&stray| alive(stray)) {
                signal_pid(stray, libc::SIGKILL);
            }
        });
    }
}

/// A stream for a run that never started: its lines fail with the refusal and its code never comes.
struct RefusedStream {
    lines: Lines,
    exited: SharedFuture<Option<i32>>,
}

impl ExecStream for RefusedStream {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        self.lines.iterate()
    }
    fn teardown(&self) {}
    fn kill(&self) {}
    fn write(&self, _line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        Box::pin(async { Ok(WriteOutcome::Gone) })
    }
    fn close_input(&self) {}
    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.exited.clone()
    }
}

/// Opens the reader of a run: the poll starts now, and the lines and the exit code are queued for whoever reads them.
fn open(cfg: &Cfg, base: String, has_input: bool, launch_failed: bool, o: Open) -> Arc<dyn ExecStream> {
    let now = cfg.now_fn();
    let now_at = now();
    let started_at = o.started_at.unwrap_or(now_at as f64);
    let read_work = cfg.opts.read_work.clone().unwrap_or_else(tree_work);
    let pid = pid_in(&format!("{base}.pid")).map(|pid| pid as u32);
    let (exit_tx, exit_rx) = oneshot::channel::<Option<i32>>();
    let exited: SharedFuture<Option<i32>> = async move { exit_rx.await.ok().flatten() }.boxed_local().shared();
    let inner = Arc::new(Inner {
        base,
        has_input,
        limits: cfg.limits(),
        poll_ms: cfg.opts.poll_ms.unwrap_or(POLL_MS),
        now: cfg.now_fn(),
        ids: cfg.ids(),
        waiting: cfg.waiting.clone(),
        read_work,
        reading_set: cfg.opts.reading.clone(),
        launch_failed,
        lines: Lines::new(),
        state: Mutex::new(State {
            started_at,
            activity: turn_activity(now_at),
            resting: false,
            killed: false,
            input_closed: false,
            parked: None,
            seq: 0,
            finished: false,
            exit_tx: Some(exit_tx),
            offset: o.from.unwrap_or(0),
            downs: 0,
            reading: false,
            caught_up: false,
            exit_seen: false,
            dropped: false,
            wake: None,
            reading_key: None,
        }),
    });
    if let Some(set) = &inner.reading_set {
        let weak: Weak<Inner> = Arc::downgrade(&inner);
        let key = set.add(Arc::new(move || {
            if let Some(inner) = weak.upgrade() {
                inner.let_go();
            }
        }));
        lock(&inner.state).reading_key = Some(key);
    }
    spawn_local(poll(inner.clone()));
    Arc::new(LocalStream { inner, pid, taken: o.taken, exited })
}

impl ExecStream for LocalStream {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        self.inner.lines.iterate()
    }
    fn run(&self) -> Option<String> {
        Some(self.inner.base.clone())
    }
    /// The leader read once at open.
    fn pid(&self) -> Option<u32> {
        self.pid
    }
    fn teardown(&self) {
        self.inner.signal(libc::SIGTERM);
    }
    fn kill(&self) {
        self.inner.kill();
    }
    fn write(&self, line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        Box::pin(write_line(self.inner.clone(), line))
    }
    fn close_input(&self) {
        let inner = self.inner.clone();
        {
            let mut st = lock(&inner.state);
            if !inner.has_input || st.input_closed || st.finished {
                return;
            }
            st.input_closed = true;
        }
        if lock(&inner.state).parked.is_none() {
            inner.close_channel();
        } else {
            spawn_local(async move {
                inner.parked_out().await;
                inner.close_channel();
            });
        }
    }
    /// `None` on a stream with no input channel (the TS throws), so the caller's own write gets the refusal.
    fn write_after(&self, line: String, after: Settle) -> Option<()> {
        let inner = self.inner.clone();
        if !inner.has_input {
            return None;
        }
        // Taken at once, so a refusal that comes while an earlier line still holds the queue is handled then.
        let (due_tx, due) = oneshot::channel::<bool>();
        spawn_local(async move {
            let _ = due_tx.send(after.await.is_ok());
        });
        let (done_tx, done_rx) = oneshot::channel::<()>();
        let done: Shared<LocalBoxFuture<'static, ()>> = async move {
            let _ = done_rx.await;
        }
        .boxed_local()
        .shared();
        let (seq, before) = {
            let mut st = lock(&inner.state);
            st.seq += 1;
            let seq = st.seq;
            let before = st.parked.replace(Parked { seq, done: done.clone() });
            (seq, before.map(|parked| parked.done))
        };
        // Without a line parked ahead, the files are written here, so they exist when this returns, as the TS's are.
        let made = before.is_none().then(|| inner.park_files(&line));
        spawn_local(park(inner, line, made, due, seq, done_tx, before));
        Some(())
    }
    fn taken(&self) -> Option<Vec<String>> {
        self.taken.clone()
    }
    fn rest(&self) -> Option<()> {
        lock(&self.inner.state).resting = true;
        spawn_local(end_leftovers(self.inner.clone()));
        Some(())
    }
    /// The log's size, or the offset where the log cannot be read.
    fn next_turn(&self) -> Option<u64> {
        let now = self.inner.now();
        {
            let mut st = lock(&self.inner.state);
            st.resting = false;
            st.started_at = now as f64;
            st.activity = turn_activity(now);
        }
        self.inner.wake();
        let size = fs::metadata(format!("{}.log", self.inner.base)).map(|m| m.len());
        Some(size.unwrap_or_else(|_| lock(&self.inner.state).offset))
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.exited.clone()
    }
}
