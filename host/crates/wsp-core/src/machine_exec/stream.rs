// SPDX-License-Identifier: AGPL-3.0-only
//! The one reader both machine roads share: polls a run's log over exec, yields its lines, ends and reaps the run
//! however it ends, cuts it at the turn's limits, and carries its input channel. Port of
//! packages/runtime/src/machine-exec.ts:217-264 and 346-736 at 65f6b89c7. Spec: spec/machine_exec_stream.md.
//!
//! Private state sits behind a `std::sync::Mutex` that is never held across an `.await` or a call out (rule 3.3).
use std::collections::VecDeque;
use std::future::pending;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use base64::engine::{general_purpose::NO_PAD, GeneralPurpose};
use base64::{alphabet, Engine as _};
use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture};
use futures::stream::LocalBoxStream;
use futures::StreamExt;
use wsp_adapters::port::{ExecFailure, ExecFailureKind, ExecStream, Settle, SharedFuture, WriteOutcome};
use wsp_engine::errors::until_reached;
use wsp_engine::exec_detached::put::{machine_answer, put_files, GuestWrite, PutFilesOptions, UploadResult, HANDSHAKE};
use wsp_engine::machine::ExecResult;
use tokio::task::spawn_local;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::shell_quote::shell_quote as q;
use wsp_wire::wire::limits::{EXEC_CHUNK_BYTES, LINK_RETRY_WINDOW_MS};
use wsp_words::words::thread::machine_unreached_line;

use super::factory::{claim, exec_failure_of, grace_checks, holds_run_probe, reap_script, release_held_sh, release_late_sh, Core, Launched, Posting};
use super::rules::{reads_work, turn_activity, turn_cut, LineCut, TurnActivity, TurnLimits, LEFTOVERS_AWK, TREE_WORK_AWK};

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
    state: Arc<RunState>,
}

/// What one run's reader, its writes and its signals share. Lock order: the state lock is taken alone, never across an
/// await or a call into the machine.
struct RunState {
    core: Arc<Core>,
    base: String,
    has_input: bool,
    sentinel: String,
    reap_mark: String,
    opened: Launched,
    posting: Option<Posting>,
    nap: Arc<PollNap>,
    exited: SharedFuture<Option<i32>>,
    chain: futures::lock::Mutex<()>,
    state: Mutex<RunMutable>,
}

struct RunMutable {
    killed: bool,
    signalled: bool,
    input_closed: bool,
    dropped: bool,
    resting: bool,
    lines_taken: bool,
    finished: bool,
    exit_tx: Option<oneshot::Sender<Option<i32>>>,
    drop_key: Option<u64>,
    parked: Option<(u64, SharedFuture<()>)>,
    parked_seq: u64,
    hurried_until: i64,
    started_at: i64,
    activity: TurnActivity,
    offset: u64,
    taken: Option<Vec<String>>,
}

/// How the run question after an unanswered launch came out.
enum Holds {
    Yes,
    No,
    Silent,
    Stopped,
}

/// One chunk of a reap: the bytes past the offset and the log's size the reap read up to.
struct Read {
    chunk: Vec<u8>,
    end: u64,
}

impl RunState {
    fn lock(&self) -> MutexGuard<'_, RunMutable> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn now(&self) -> i64 {
        (self.core.now)()
    }
    fn offset(&self) -> u64 {
        self.lock().offset
    }
    fn add_offset(&self, n: u64) -> u64 {
        let mut m = self.lock();
        m.offset += n;
        m.offset
    }
    fn hurry_polls(&self) {
        let until = self.now() + HURRIED_FOR_MS;
        self.lock().hurried_until = until;
        self.nap.hurry();
    }
    fn gap(&self) -> u64 {
        let now = self.now();
        let (hurried, resting) = {
            let m = self.lock();
            (m.hurried_until, m.resting)
        };
        if now < hurried {
            self.core.poll_ms.min(HURRIED_POLL_MS)
        } else if resting {
            RESTING_POLL_MS
        } else {
            self.core.poll_ms
        }
    }
    fn finish(&self, code: Option<i32>) {
        let key = {
            let mut m = self.lock();
            if m.finished {
                return;
            }
            m.finished = true;
            if let Some(tx) = m.exit_tx.take() {
                let _ = tx.send(code);
            }
            m.drop_key.take()
        };
        self.forget(key);
    }
    fn forget(&self, key: Option<u64>) {
        if let (Some(key), Some(reading)) = (key, &self.core.reading) {
            reading.remove(key);
        }
    }
    fn let_go(&self) {
        let key = {
            let mut m = self.lock();
            m.dropped = true;
            m.drop_key.take()
        };
        self.forget(key);
        self.nap.wake();
    }
    /// The signal's own exec: the run's group gets it once the launch settled.
    fn signal(self: &Arc<Self>, sig: &str) {
        let resting = {
            let mut m = self.lock();
            m.signalled = true;
            m.resting
        };
        if resting {
            self.hurry_polls();
        }
        let me = self.clone();
        let cmd = signal_cmd(&self.base, sig);
        spawn_local(async move {
            let _ = me.opened.clone().await;
            let _ = me.core.machine.exec(&cmd, me.core.exec_opts()).await;
        });
    }
    /// A second kill sends nothing: the group already had its KILL, and a kill answering its own exec never lets the reader run.
    fn kill(self: &Arc<Self>) {
        let first = !std::mem::replace(&mut self.lock().killed, true);
        if first {
            self.signal("KILL");
        }
    }
    async fn killed_here(&self) -> Option<String> {
        let (signalled, started_at) = {
            let m = self.lock();
            (m.signalled, m.started_at)
        };
        match &self.core.killed_why {
            Some(killed_why) if !signalled => killed_why(started_at).await,
            _ => None,
        }
    }
    async fn reap(&self, offset: u64, end: Option<u64>) -> Option<Read> {
        let read = reap_read(&self.base, &self.reap_mark, offset, end);
        let cmd = format!("{}; true", reap_script(&self.base, &read, end.is_none()));
        let res = self.core.machine.exec(&cmd, self.core.exec_opts()).await.ok()?;
        let out: Vec<&str> = res.stdout.split('\n').collect();
        let prefix = format!("{} ", self.reap_mark);
        let at = out.iter().position(|line| line.starts_with(&prefix))?;
        let size = u64::try_from(js_parse_int(&out[at][prefix.len()..])?).ok()?;
        Some(Read { chunk: node_base64(&out[..at].concat()), end: size })
    }
    async fn remove(&self) {
        let cmd = format!("{}; true", reap_script(&self.base, "", false));
        let _ = self.core.machine.exec(&cmd, self.core.exec_opts()).await;
    }
    async fn holds_run(&self) -> (Holds, u32) {
        let from = self.now();
        let mut asked: u32 = 0;
        loop {
            let dropped = self.lock().dropped;
            if dropped {
                pending::<()>().await;
            }
            if self.lock().killed {
                return (Holds::Stopped, asked);
            }
            if !self.core.asks_until_stopped && self.now() - from >= LINK_RETRY_WINDOW_MS {
                return (Holds::Silent, asked);
            }
            let res = self.core.machine.exec(&holds_run_probe(&self.base), self.core.exec_opts()).await.ok();
            let said = res.as_ref().map_or("", |r| r.stdout.as_str());
            if said.contains(HANDSHAKE.run) {
                return (Holds::Yes, asked + 1);
            }
            if said.contains(HANDSHAKE.gone) {
                return (Holds::No, asked + 1);
            }
            self.nap.nap(self.core.poll_ms).await;
            asked += 1;
        }
    }
    /// One line onto the channel, after a seed still held: the one-at-a-time chain, then the guest's answer.
    async fn append(&self, line: &str) -> Result<WriteOutcome, ExecFailure> {
        let upload = {
            let _turn = self.chain.lock().await;
            let file = GuestWrite { path: format!("{}.in", self.base), text: format!("{line}\n"), append: true };
            put_files(
                &*self.core.machine,
                &[file],
                PutFilesOptions {
                    before: append_before(&self.base),
                    after: vec![format!("echo {}", HANDSHAKE.written)],
                    timeout_ms: Some(self.core.exec_timeout_ms),
                    stdin: None,
                },
            )
            .await
        };
        let res = upload.map_err(|e| exec_failure_of(&e))?;
        if res.stdout.contains(HANDSHAKE.gone) {
            return Ok(WriteOutcome::Gone);
        }
        if res.exit_code != 0 || !res.stdout.contains(HANDSHAKE.written) {
            return Err(ExecFailure {
                kind: ExecFailureKind::Other,
                message: format!(
                    "remote write failed on {}: nothing came back saying {}, the word the guest prints once the message landed; {}",
                    self.core.machine.id(),
                    HANDSHAKE.written,
                    upload_answer(&res)
                ),
            });
        }
        let now = self.now();
        self.lock().activity.touch(now);
        self.hurry_polls();
        tokio::task::yield_now().await;
        Ok(WriteOutcome::Written)
    }
    /// A line held beside the run until `after` settles: keyed and parked on the guest, then released or appended.
    async fn park(&self, line: String, after: Settle) {
        let b = q(&self.base);
        let parked = if self.opened.clone().await.is_ok() {
            let files = [
                GuestWrite { path: format!("{}.late.key", self.base), text: hex::encode(self.core.ids.bytes(6)), append: false },
                GuestWrite { path: format!("{}.late", self.base), text: format!("{line}\n"), append: false },
            ];
            let options = PutFilesOptions { before: vec!["umask 077".into()], timeout_ms: Some(self.core.exec_timeout_ms), ..Default::default() };
            put_files(&*self.core.machine, &files, options).await.is_ok()
        } else {
            false
        };
        if after.await.is_err() {
            if parked {
                let _ = self.core.machine.exec(&format!("rm -f {b}.late {b}.late.key"), self.core.exec_opts()).await;
            }
            return;
        }
        if !parked {
            let _ = self.append(&line).await;
            return;
        }
        let clock = self.core.retry_clock();
        let turn = self.chain.lock().await;
        let cmd = release_late_sh(&self.base);
        let released = until_reached(
            || {
                let (machine, cmd, opts) = (self.core.machine.clone(), cmd.clone(), self.core.exec_opts());
                async move { machine.exec(&cmd, opts).await }
            },
            &*clock,
        )
        .await;
        drop(turn);
        if released.is_err() {
            return;
        }
        let now = self.now();
        self.lock().activity.touch(now);
        self.hurry_polls();
    }
}

/// The reader's own state: which step comes next, and what the run's end is still owed.
enum Stage {
    Opening,
    Looping,
    Reaping(Reaping),
    Done,
}

/// The reap a run's end runs, and what it settles the run with once the lines are out.
#[derive(Default)]
struct Reaping {
    ending: Ending,
    end: Option<u64>,
}

/// How a run ends after its reap: the exit code it settles, whether the kernel's kill words are asked for, and the
/// failure the stream ends on.
#[derive(Default)]
struct Ending {
    code: Option<i32>,
    check_killed: bool,
    fail: Option<ExecFailure>,
}

/// One reader of one run: a stream's `lines` iterates it. Dropping it before the end lets go of the run's exit.
struct Reader {
    s: Arc<RunState>,
    split: LineCut,
    queue: VecDeque<Result<String, ExecFailure>>,
    stage: Stage,
    downs: u32,
    caught_up: bool,
    exit_seen: bool,
    dark_since: Option<i64>,
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.s.finish(None);
    }
}

impl Reader {
    fn new(s: Arc<RunState>) -> Self {
        Reader { s, split: LineCut::new(), queue: VecDeque::new(), stage: Stage::Opening, downs: 0, caught_up: false, exit_seen: false, dark_since: None }
    }
    fn push_chunk(&mut self, chunk: &[u8]) {
        let lines = self.split.take(chunk);
        self.queue.extend(lines.into_iter().map(Ok));
    }
    fn cut_in_the_dark(&mut self, cut: Option<ExecFailure>, at: i64) -> bool {
        let since = *self.dark_since.get_or_insert(at);
        if let Some(cut) = cut {
            if self.caught_up || self.s.now() - since >= LINK_RETRY_WINDOW_MS {
                self.stage = Stage::Reaping(Reaping { ending: Ending { fail: Some(cut), ..Ending::default() }, end: None });
                return true;
            }
        }
        false
    }
    /// One step of the run's reader: the opening, one poll, one reap's chunk, or the end. Lines it makes are queued.
    async fn advance(&mut self) {
        match std::mem::replace(&mut self.stage, Stage::Done) {
            Stage::Done => {}
            Stage::Opening => {
                self.stage = match self.s.opened.clone().await {
                    Ok(()) => Stage::Looping,
                    Err(fail) => Stage::Reaping(Reaping { ending: Ending { fail: Some(fail), ..Ending::default() }, end: None }),
                };
            }
            Stage::Looping => {
                self.stage = Stage::Looping;
                self.looping().await;
            }
            Stage::Reaping(r) => self.reaping(r).await,
        }
    }
    async fn looping(&mut self) {
        let s = self.s.clone();
        let dropped = s.lock().dropped;
        if dropped {
            pending::<()>().await;
        }
        if s.lock().killed {
            self.stage = Stage::Reaping(Reaping::default());
            return;
        }
        let at = s.now();
        if (s.core.waiting)() {
            s.lock().activity.touch(at);
        }
        let (quiet, started_at, resting) = {
            let m = s.lock();
            (m.activity.quiet_ms(at), m.started_at, m.resting)
        };
        let limits = TurnLimits { idle_ms: s.core.idle_ms, deadline_ms: s.core.deadline_ms };
        let cut = if resting { None } else { turn_cut(limits, at - started_at, quiet) };
        if cut.is_some() && self.caught_up && !self.exit_seen {
            self.stage = Stage::Reaping(Reaping { ending: Ending { fail: cut, ..Ending::default() }, end: None });
            return;
        }
        let work = !resting && reads_work(s.core.idle_ms, quiet);
        let cmd = poll_cmd(&s.base, &s.sentinel, s.offset(), work);
        let res = match s.core.machine.exec(&cmd, s.core.exec_opts()).await {
            Ok(res) => res,
            Err(_) => {
                if self.cut_in_the_dark(cut, at) {
                    return;
                }
                s.nap.nap(s.core.poll_ms).await;
                let now = s.now();
                s.lock().activity.hold(now - at);
                return;
            }
        };
        let out: Vec<&str> = res.stdout.split('\n').collect();
        let Some(mark) = out.iter().position(|line| line.starts_with(s.sentinel.as_str())) else {
            if self.cut_in_the_dark(cut, at) {
                return;
            }
            s.nap.nap(s.core.poll_ms).await;
            return;
        };
        self.dark_since = None;
        let parts: Vec<&str> = out[mark].split(' ').collect();
        let exit_str = parts.get(1).copied().unwrap_or("");
        let live = parts.get(2).copied().unwrap_or("up");
        let work_str = parts.get(3).copied().unwrap_or("");
        if let Some(work) = js_parse_int(work_str) {
            let now = s.now();
            s.lock().activity.read(work, now);
        }
        let chunk = node_base64(&out[..mark].concat());
        s.add_offset(chunk.len() as u64);
        if !chunk.is_empty() {
            let now = s.now();
            s.lock().activity.touch(now);
            if let Some(posting) = &s.posting {
                if s.now() - posting.since < OPENING_HURRY_MS {
                    let until = s.now() + HURRIED_FOR_MS;
                    s.lock().hurried_until = until;
                }
            }
            self.push_chunk(&chunk);
            if (chunk.len() as i64) < EXEC_CHUNK_BYTES {
                self.caught_up = true;
            }
            self.exit_seen = !exit_str.is_empty();
            return;
        }
        if !exit_str.is_empty() {
            let code = js_parse_int(exit_str).and_then(|c| i32::try_from(c).ok());
            self.stage = Stage::Reaping(Reaping { ending: Ending { code, check_killed: code == Some(SIGKILL_EXIT), fail: None }, end: None });
            return;
        }
        self.caught_up = true;
        if cut.is_some() {
            self.stage = Stage::Reaping(Reaping { ending: Ending { fail: cut, ..Ending::default() }, end: None });
            return;
        }
        if live == "down" {
            self.downs += 1;
            if self.downs > 1 {
                self.stage = Stage::Reaping(Reaping { ending: Ending { check_killed: true, ..Ending::default() }, end: None });
                return;
            }
        }
        let gap = s.gap();
        s.nap.nap(gap).await;
    }
    /// The run's reap, one chunk at a time: each chunk's lines are handed out before the next reap goes.
    async fn reaping(&mut self, mut r: Reaping) {
        let read = self.s.reap(self.s.offset(), r.end).await;
        let finishing = match read {
            Some(read) => {
                r.end = Some(read.end);
                let offset = self.s.add_offset(read.chunk.len() as u64);
                self.push_chunk(&read.chunk);
                read.chunk.is_empty() || offset >= read.end
            }
            None => true,
        };
        if !finishing {
            self.stage = Stage::Reaping(r);
            return;
        }
        if r.end.map_or(true, |end| self.s.offset() < end) {
            self.s.remove().await;
        }
        if let Some(last) = self.split.rest() {
            self.queue.push_back(Ok(last));
        }
        self.settle(r.ending).await;
    }
    async fn settle(&mut self, ending: Ending) {
        self.stage = Stage::Done;
        self.s.finish(ending.code);
        if let Some(fail) = ending.fail {
            self.queue.push_back(Err(fail));
            return;
        }
        if ending.check_killed {
            if let Some(why) = self.s.killed_here().await {
                self.queue.push_back(Err(ExecFailure { kind: ExecFailureKind::Other, message: why }));
            }
        }
    }
}

/// The next line of a reader, running its steps until one yields a line or the reader is done.
async fn next_line(mut reader: Reader) -> Option<(Result<String, ExecFailure>, Reader)> {
    loop {
        if let Some(item) = reader.queue.pop_front() {
            return Some((item, reader));
        }
        if matches!(reader.stage, Stage::Done) {
            return None;
        }
        reader.advance().await;
    }
}

/// The run question an unanswered launch gets: its own sentence, or a question the run answers.
async fn opened_future(weak: Weak<RunState>, launched: Launched, posting: Option<Posting>) -> Result<(), ExecFailure> {
    let e = match launched.await {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };
    let Some(posting) = posting else { return Err(e) };
    if !posting.may_have_run.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(e);
    }
    let Some(s) = weak.upgrade() else { return pending().await };
    let (holds, asked) = s.holds_run().await;
    match holds {
        Holds::Yes => Ok(()),
        Holds::Silent => {
            let attempts = posting.posts.load(std::sync::atomic::Ordering::SeqCst) + asked;
            let elapsed = s.now() - posting.since;
            Err(ExecFailure { kind: ExecFailureKind::Unreached, message: machine_unreached_line(f64::from(attempts), elapsed as f64) })
        }
        Holds::No if e.kind != ExecFailureKind::Unreached => {
            Err(ExecFailure { kind: ExecFailureKind::LaunchUnanswered, message: e.message })
        }
        Holds::No | Holds::Stopped => Err(e),
    }
}

/// A held seed's due date: the hand-over once the launch opened, and a kill when the seed never came due or never
/// reached the run.
async fn hand_over_seed(weak: Weak<RunState>, seed_due: Settle) {
    let Some(s) = weak.upgrade() else { return };
    if seed_due.await.is_err() {
        s.kill();
        return;
    }
    if s.opened.clone().await.is_err() {
        return;
    }
    let clock = s.core.retry_clock();
    let handed = {
        let _turn = s.chain.lock().await;
        let (machine, cmd, opts) = (s.core.machine.clone(), release_held_sh(&s.base), s.core.exec_opts());
        until_reached(
            || {
                let (machine, cmd, opts) = (machine.clone(), cmd.clone(), opts.clone());
                async move { machine.exec(&cmd, opts).await }
            },
            &*clock,
        )
        .await
    };
    if handed.is_err() {
        s.kill();
    }
}

/// Eagerly runs `fut` on this thread; its answer is one shared future every reader awaits.
pub(super) fn eager<T: Clone + 'static>(fut: LocalBoxFuture<'static, T>) -> SharedFuture<T> {
    let (tx, rx) = oneshot::channel();
    spawn_local(async move {
        let _ = tx.send(fut.await);
    });
    settled(rx)
}

/// A shared future that settles with what `rx` got, and never settles if the sender went without answering.
fn settled<T: Clone + 'static>(rx: oneshot::Receiver<T>) -> SharedFuture<T> {
    async move {
        match rx.await {
            Ok(value) => value,
            Err(_) => pending().await,
        }
    }
    .boxed_local()
    .shared()
}

/// The guest's answer to an upload, in the words a sentence about it reads.
pub(super) fn upload_answer(res: &UploadResult) -> String {
    machine_answer(&ExecResult { exit_code: JsInt(JsNumber::from(res.exit_code)), stdout: res.stdout.clone(), stderr: res.stderr.clone() })
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
    let OpenArgs { base, has_input, launched, turn_started_at, taken, posting, from, seed_due } = args;
    let sentinel = format!("__WSP_EOF_{}__", hex::encode(core.ids.bytes(6)));
    let reap_mark = format!("__WSP_REAPED_{}__", hex::encode(core.ids.bytes(6)));
    let now = (core.now)();
    let nap = PollNap::new(core.sleep.clone());
    let (exit_tx, exit_rx) = oneshot::channel::<Option<i32>>();
    let exited = settled(exit_rx);
    let hurried_until = if posting.is_none() { 0 } else { now + OPENING_HURRY_MS };
    let state = RunMutable {
        killed: false,
        signalled: false,
        input_closed: false,
        dropped: false,
        resting: false,
        lines_taken: false,
        finished: false,
        exit_tx: Some(exit_tx),
        drop_key: None,
        parked: None,
        parked_seq: 0,
        hurried_until,
        started_at: turn_started_at.unwrap_or(now),
        activity: turn_activity(now),
        offset: from.unwrap_or(0),
        taken,
    };
    let posting_for_opened = posting.clone();
    let shared = Arc::new_cyclic(|weak: &Weak<RunState>| RunState {
        core: core.clone(),
        base,
        has_input,
        sentinel,
        reap_mark,
        opened: eager(opened_future(weak.clone(), launched, posting_for_opened).boxed_local()),
        posting,
        nap,
        exited,
        chain: futures::lock::Mutex::new(()),
        state: Mutex::new(state),
    });
    if let Some(reading) = &core.reading {
        let weak = Arc::downgrade(&shared);
        let key = reading.add(Arc::new(move || {
            if let Some(s) = weak.upgrade() {
                s.let_go();
            }
        }));
        shared.lock().drop_key = Some(key);
    }
    if let Some(seed_due) = seed_due {
        spawn_local(hand_over_seed(Arc::downgrade(&shared), seed_due));
    }
    Arc::new(MachineStream { state: shared })
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
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        let taken = std::mem::replace(&mut self.state.lock().lines_taken, true);
        if taken {
            return futures::stream::empty().boxed_local();
        }
        futures::stream::unfold(Reader::new(self.state.clone()), next_line).fuse().boxed_local()
    }
    fn run(&self) -> Option<String> {
        Some(self.state.base.clone())
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
        self.state.signal("TERM");
    }
    fn kill(&self) {
        self.state.kill();
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
    fn write(&self, line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        let s = self.state.clone();
        if !s.has_input {
            return Box::pin(std::future::ready(Err(refusal("this stream has no input channel"))));
        }
        let (finished, ahead) = {
            let m = s.lock();
            (m.finished, m.parked.clone())
        };
        if finished {
            return Box::pin(std::future::ready(Err(refusal("the stream has ended"))));
        }
        Box::pin(async move {
            s.opened.clone().await?;
            if let Some((_, ahead)) = ahead {
                let _ = ahead.await;
            }
            s.append(&line).await
        })
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
        let s = self.state.clone();
        let resting = {
            let mut m = s.lock();
            if !s.has_input || m.input_closed || m.finished {
                return;
            }
            m.input_closed = true;
            m.resting
        };
        if resting {
            s.hurry_polls();
        }
        spawn_local(async move {
            let _ = s.opened.clone().await;
            loop {
                let parked = s.lock().parked.clone();
                match parked {
                    Some((_, parked)) => {
                        let _ = parked.await;
                    }
                    None => break,
                }
            }
            let _turn = s.chain.lock().await;
            let _ = s.core.machine.exec(&close_input_cmd(&s.base), s.core.exec_opts()).await;
        });
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
        let s = self.state.clone();
        if !s.has_input {
            return None;
        }
        let (tx, rx) = oneshot::channel::<()>();
        let mine: SharedFuture<()> = async move {
            let _ = rx.await;
        }
        .boxed_local()
        .shared();
        let (before, seq) = {
            let mut m = s.lock();
            m.parked_seq += 1;
            let seq = m.parked_seq;
            (m.parked.replace((seq, mine)), seq)
        };
        spawn_local(async move {
            if let Some((_, before)) = before {
                let _ = before.await;
            }
            s.park(line, after).await;
            {
                let mut m = s.lock();
                if matches!(m.parked, Some((id, _)) if id == seq) {
                    m.parked = None;
                }
            }
            let _ = tx.send(());
        });
        Some(())
    }
    fn taken(&self) -> Option<Vec<String>> {
        self.state.lock().taken.clone()
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
        self.state.lock().resting = true;
        let s = self.state.clone();
        spawn_local(async move {
            let _ = s.opened.clone().await;
            let _ = s.core.machine.exec(&end_leftovers_cmd(&s.base), s.core.exec_opts()).await;
        });
        Some(())
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
        let now = self.state.now();
        let offset = {
            let mut m = self.state.lock();
            m.resting = false;
            m.started_at = now;
            m.activity = turn_activity(now);
            m.offset
        };
        self.state.nap.hurry();
        Some(offset)
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.state.exited.clone()
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
    sleep: Option<super::factory::SleepFn>,
    state: std::sync::Mutex<NapState>,
}

#[derive(Default)]
struct NapState {
    awake: bool,
    woken: Option<oneshot::Sender<()>>,
}

impl PollNap {
    pub fn new(sleep: Option<super::factory::SleepFn>) -> Arc<Self> {
        Arc::new(PollNap { sleep, state: std::sync::Mutex::new(NapState::default()) })
    }
    fn locked(&self) -> MutexGuard<'_, NapState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    pub fn nap(&self, ms: u64) -> LocalBoxFuture<'static, ()> {
        if self.locked().awake {
            return Box::pin(std::future::ready(()));
        }
        let timer: LocalBoxFuture<'static, ()> = match &self.sleep {
            Some(sleep) => sleep(ms),
            None => Box::pin(tokio::time::sleep(Duration::from_millis(ms))),
        };
        let (tx, rx) = oneshot::channel::<()>();
        self.locked().woken = Some(tx);
        Box::pin(async move {
            futures::future::select(timer, rx).await;
        })
    }
    pub fn wake(&self) {
        let woken = {
            let mut s = self.locked();
            s.awake = true;
            s.woken.take()
        };
        if let Some(tx) = woken {
            let _ = tx.send(());
        }
    }
    pub fn hurry(&self) {
        let woken = self.locked().woken.take();
        if let Some(tx) = woken {
            let _ = tx.send(());
        }
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
    let b = q(base);
    let tree = if work { format!("W=$(awk -v p=\"$P\" {} /proc/[0-9]*/stat 2>/dev/null); ", q(TREE_WORK_AWK)) } else { String::new() };
    format!(
        "E=$(cat {b}.exit 2>/dev/null); tail -c +{} {b}.log 2>/dev/null | head -c {EXEC_CHUNK_BYTES} | base64 -w0; P=$(cat {b}.pid 2>/dev/null); {tree}printf '\\n{sentinel} %s %s %s\\n' \"$E\" \"$([ -n \"$P\" ] && kill -0 \"$P\" 2>/dev/null && echo up || echo down)\" \"$W\"",
        offset + 1
    )
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
    let log = format!("{}.log", q(base));
    let size = match end {
        Some(end) => end.to_string(),
        None => format!("$({{ wc -c < {log}; }} 2>/dev/null || echo 0)"),
    };
    format!(
        "S={size}; N=$((S - {offset})); [ $N -gt {EXEC_CHUNK_BYTES} ] && N={EXEC_CHUNK_BYTES}; [ $N -gt 0 ] && tail -c +{} {log} 2>/dev/null | head -c $N | base64 -w0; printf '\\n%s %s\\n' {mark} \"$((S))\"; [ $((S - {offset})) -gt {EXEC_CHUNK_BYTES} ] && exit 0; ",
        offset + 1
    )
}

/// `P=$(cat '<b>'.pid 2>/dev/null); [ -n "$P" ] && kill -<sig> -- -$P 2>/dev/null; true`.
// TS machine-exec.ts:444-444
//           machine.exec(`P=$(cat ${q(base)}.pid 2>/dev/null); [ -n "$P" ] && kill -${sig} -- -$P 2>/dev/null; true`, {
pub fn signal_cmd(base: &str, sig: &str) -> String {
    format!("P=$(cat {}.pid 2>/dev/null); [ -n \"$P\" ] && kill -{sig} -- -$P 2>/dev/null; true", q(base))
}

/// The close of the channel: the held seed first, then TERM to the recorded tail alone.
// TS machine-exec.ts:677-677
//           .then(() => oneAtATime(() => machine.exec(`${releaseHeld(base)}; P=$(cat ${q(base)}.tail 2>/dev/null); [ -n "$P" ] && kill -TERM "$P" 2>/dev/null; true`, { timeoutMs: execTimeoutMs })))
pub fn close_input_cmd(base: &str) -> String {
    format!("{}; P=$(cat {}.tail 2>/dev/null); [ -n \"$P\" ] && kill -TERM \"$P\" 2>/dev/null; true", release_held_sh(base), q(base))
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
    let b = q(base);
    let checks = grace_checks();
    let poll = GRACE_POLL_MS as f64 / 1000.0;
    format!(
        "P=$(cat {b}.pid 2>/dev/null); T=$(cat {b}.tail 2>/dev/null); [ -n \"$P\" ] || exit 0; L=$(for f in /proc/[0-9]*/stat; do read -r s < \"$f\" && printf '%s\\n' \"$s\"; done 2>/dev/null | awk -v p=\"$P\" -v t=\"$T\" {}); [ -n \"$L\" ] || exit 0; kill -TERM $L 2>/dev/null; for i in {checks}; do S=; for x in $L; do kill -0 $x 2>/dev/null && S=1; done; [ -n \"$S\" ] || break; sleep {poll}; done; kill -KILL $L 2>/dev/null; true",
        q(LEFTOVERS_AWK)
    )
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
    vec![
        format!("{{ [ -e {}.exit ] || [ ! -d {} ]; }} && {{ echo {}; exit 0; }}", q(base), q(&claim(base)), HANDSHAKE.gone),
        release_held_sh(base),
    ]
}

/// JS `parseInt(text, 10)`: leading whitespace, an optional sign, then digits; `None` where that reads NaN or the
/// value is not a safe integer.
pub fn js_parse_int(text: &str) -> Option<i64> {
    let text = text.trim_start();
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let count = digits.bytes().take_while(u8::is_ascii_digit).count();
    if count == 0 {
        return None;
    }
    let magnitude: f64 = digits[..count].parse().ok()?;
    let value = if negative { -magnitude } else { magnitude };
    (value.abs() <= 9_007_199_254_740_991.0).then_some(value as i64)
}

/// Node's `Buffer.from(text, "base64")`: characters outside the alphabet are skipped and a missing padding is fine.
fn node_base64(text: &str) -> Vec<u8> {
    let mut clean: String = text.chars().take_while(|&c| c != '=').filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/').collect();
    if clean.len() % 4 == 1 {
        clean.pop();
    }
    LENIENT_BASE64.decode(clean).unwrap_or_default()
}

const LENIENT_BASE64: GeneralPurpose = GeneralPurpose::new(&alphabet::STANDARD, NO_PAD.with_decode_allow_trailing_bits(true));

fn refusal(message: &str) -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Other, message: message.to_string() }
}
