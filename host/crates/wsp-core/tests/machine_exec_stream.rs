// SPDX-License-Identifier: AGPL-3.0-only
//! machine_exec_stream: the reader both machine roads share, from the reader cases of
//! packages/runtime/test/machine-exec.test.ts. Spec: spec/machine_exec_stream.md.
mod support;

use std::collections::VecDeque;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::{FutureExt, StreamExt};
use regex::Regex;
use tokio::sync::oneshot;
use wsp_adapters::claude::{create_claude_adapter, AdapterDeps};
use wsp_adapters::port::{
    end_run, AdapterEvent, AttachRunOptions, Attached, ExecFailure, ExecOptions as Launch, ExecStream, ExecStreamFactory,
    HarnessStartOptions, Settle, WriteOutcome,
};
use wsp_core::machine_exec::{machine_exec_stream, KilledWhy, MachineExecOptions, NowFn, ReadingSet, SleepFn, TurnWaiting, TREE_WORK_AWK};
use wsp_engine::errors::{ErrorKind, Result as EngineResult};
use wsp_engine::machine::{ExecOptions as MachineOptions, ExecResult};
use wsp_engine::Machine;

use support::guest::{errors, Hook, Inner, LocalGuest, ScriptGuest, Step, Wrapped};
use support::strays;

/// A minute of one core, in the ticks the guest's read counts: what a test batch or a packager does per minute.
const CORE_MINUTE: i64 = 6_000;
const EXEC_CHUNK_BYTES: usize = 262_144;
const WALL_WORDS: &str = "send to continue where it stopped, or change the limit in Settings > Computers";

type Lines = LocalBoxStream<'static, Result<String, ExecFailure>>;
type Next = tokio::task::JoinHandle<(Option<Result<String, ExecFailure>>, Lines)>;

/// One `lines.next()` started now and awaited later (the TS's un-awaited `it.next()`), handing the iterator back.
fn next_of(mut lines: Lines) -> Next {
    tokio::task::spawn_local(async move {
        let item = lines.next().await;
        (item, lines)
    })
}

fn line_of(item: Option<Result<String, ExecFailure>>) -> String {
    item.expect("a line, not the end").expect("a line, not a failure")
}

fn opts(poll_ms: u64) -> MachineExecOptions {
    MachineExecOptions { poll_ms: Some(poll_ms), ..Default::default() }
}

fn local_opts(poll_ms: u64, run_dir: &str) -> MachineExecOptions {
    MachineExecOptions { poll_ms: Some(poll_ms), run_dir: Some(run_dir.to_string()), ..Default::default() }
}

fn clock() -> Arc<AtomicI64> {
    Arc::new(AtomicI64::new(0))
}

fn now_of(c: &Arc<AtomicI64>) -> NowFn {
    let c = c.clone();
    Arc::new(move || c.load(SeqCst))
}

fn sleep_fn(f: impl Fn(u64) -> LocalBoxFuture<'static, ()> + 'static) -> SleepFn {
    Arc::new(f)
}

/// A hook that answers each exec from its text and the guest's own answer (the TS `backend.execImpl = async ...`).
fn hook<F, Fut>(f: F) -> Hook
where
    F: Fn(String, Inner) -> Fut + 'static,
    Fut: Future<Output = EngineResult<ExecResult>> + 'static,
{
    Arc::new(move |cmd: String, _stdin: Option<Vec<u8>>, inner: Inner| -> LocalBoxFuture<'static, EngineResult<ExecResult>> {
        Box::pin(f(cmd, inner))
    })
}

fn with_input(lines: &[&str]) -> Launch {
    Launch { input: Some(lines.iter().map(|l| l.to_string()).collect()), ..Default::default() }
}

fn ready() -> Settle {
    Box::pin(async { Ok::<(), String>(()) })
}

fn never() -> Settle {
    Box::pin(futures::future::pending())
}

fn after_ms(ms: u64) -> Settle {
    Box::pin(async move {
        support::sleep_ms(ms).await;
        Ok::<(), String>(())
    })
}

/// A promise settled by hand: `send(Ok(()))` resolves it, `send(Err(words))` rejects it.
fn settles() -> (Settle, oneshot::Sender<Result<(), String>>) {
    let (tx, rx) = oneshot::channel::<Result<(), String>>();
    (Box::pin(async move { rx.await.unwrap_or_else(|_| Err("dropped".to_string())) }), tx)
}

fn matches(re: &str, text: &str) -> bool {
    Regex::new(re).unwrap().is_match(text)
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64
}

/// A path beside the run folder (the TS `join(runDir, "..", name)`).
fn beside(run_dir: &str, name: &str) -> String {
    std::path::Path::new(run_dir).parent().unwrap().join(name).to_string_lossy().into_owned()
}

/// This computer's bash as the guest, and the run folder under its temporary folder (kept while the guard lives).
fn local_with(stdin: bool, shell: &str) -> (Arc<dyn Machine>, String, tempfile::TempDir) {
    let (guest, run_dir, dir) = LocalGuest::new(stdin, shell);
    let machine: Arc<dyn Machine> = guest;
    (machine, run_dir, dir)
}

fn local() -> (Arc<dyn Machine>, String, tempfile::TempDir) {
    local_with(false, "")
}

fn file_with(run_dir: &str, suffix: &str) -> Option<String> {
    support::names(run_dir).into_iter().find(|f| f.ends_with(suffix))
}

fn text_of(run_dir: &str, suffix: &str) -> String {
    file_with(run_dir, suffix).map(|f| support::read(&format!("{run_dir}/{f}"))).unwrap_or_default()
}

/// Ends every pid and group the case recorded, as the TS `afterEach` does, even when the case failed.
struct Sweep;
impl Drop for Sweep {
    fn drop(&mut self) {
        strays::sweep();
    }
}

/// A reader of a stream's lines on a task of its own (the TS `read` helper): what it has seen, and how it ended.
struct Reader {
    seen: Arc<Mutex<Vec<String>>>,
    done: tokio::task::JoinHandle<Option<ExecFailure>>,
}

fn read(stream: &Arc<dyn ExecStream>) -> Reader {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let mut lines = stream.lines();
    let done = tokio::task::spawn_local(async move {
        while let Some(next) = lines.next().await {
            match next {
                Ok(line) => sink.lock().unwrap().push(line),
                Err(failure) => return Some(failure),
            }
        }
        None
    });
    Reader { seen, done }
}

impl Reader {
    fn lines(&self) -> Vec<String> {
        self.seen.lock().unwrap().clone()
    }
    /// Awaits the end of the reading, which must not be a failure.
    async fn done(self) {
        let failure = self.done.await.expect("the reader's task");
        assert!(failure.is_none(), "the reader failed: {failure:?}");
    }
    /// Awaits the end of the reading, whatever it was.
    async fn ended(self) -> Option<ExecFailure> {
        self.done.await.expect("the reader's task")
    }
}

/// The group of a pid is there (signal 0 to its negative).
fn group_alive(pid: i32) -> bool {
    // SAFETY: signal 0 sends nothing.
    unsafe { libc::kill(-pid, 0) == 0 }
}

/// What a case that records the polls' naps reads: every nap's length, in order.
async fn next_naps(naps: &Arc<Mutex<Vec<u64>>>, count: usize) -> Vec<u64> {
    let from = naps.lock().unwrap().len();
    support::wait_for(&format!("{count} more naps"), 5_000, || naps.lock().unwrap().len() >= from + count).await;
    naps.lock().unwrap()[from..from + count].to_vec()
}

/// A guest whose clock moves one minute per poll and whose command prints one line a minute until `quiet_from_ms`,
/// its process group doing `ticks` of work every minute however quiet the log is. The hook is returned, not set, for
/// a case that puts its own in front.
fn minute_parts(quiet_from_ms: i64, exit_at_ms: i64, ticks: i64) -> (Arc<ScriptGuest>, Arc<dyn Machine>, Arc<AtomicI64>, Hook) {
    let (guest, machine) = ScriptGuest::new(vec![]);
    let clock = clock();
    let (g, c) = (guest.clone(), clock.clone());
    let minute = hook(move |cmd, inner| {
        if cmd.contains("__WSP_EOF_") {
            let now = c.fetch_add(60_000, SeqCst) + 60_000;
            let step = if now < quiet_from_ms {
                Step::append(format!("tick {}\n", now / 60_000)).work(ticks)
            } else if now >= exit_at_ms {
                Step::none().exit(0).work(ticks)
            } else {
                Step::none().work(ticks)
            };
            g.push_step(step);
        }
        let res = inner(&cmd);
        async move { res }
    });
    (guest, machine, clock, minute)
}

fn minute_guest(quiet_from_ms: i64, exit_at_ms: i64, ticks: i64) -> (Arc<ScriptGuest>, Arc<dyn Machine>, Arc<AtomicI64>) {
    let (guest, machine, clock, minute) = minute_parts(quiet_from_ms, exit_at_ms, ticks);
    guest.set_hook(minute);
    (guest, machine, clock)
}

mod machine_exec_stream {
    use super::*;

    #[test]
    fn when_the_poll_sees_the_exit_code_the_run_s_process_group_gets_term_then_kill_aft() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("almost"), Step::append(" done\n").exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, opts(5), None).launch("claude -p 'hi' & sleep 300 &", Launch::default());
            let lines = support::lines_ok(&*stream).await;
            assert_eq!(stream.exited().await, Some(0));
            assert_eq!(lines, vec!["almost done"]);
            assert!(!guest.child_alive());
            assert_eq!(guest.files(), Vec::<String>::new());
            let kills = guest.kills();
            assert_eq!(kills.len(), 1);
            let reap = &kills[0];
            assert!(matches(r"^P=\$\(cat '/tmp/wsp-run/[a-f0-9]{12}'\.pid 2>/dev/null\); ", reap), "{reap}");
            let term = reap.find("kill -TERM -- -$P").expect("a TERM to the group");
            let kill = reap.find("kill -KILL -- -$P").expect("a KILL to the group");
            let remove = reap.find("rm -rf '/tmp/wsp-run/").expect("the files' removal");
            assert!(term < kill);
            assert!(kill < remove);
            assert!(matches(r"rm -rf '/tmp/wsp-run/[a-f0-9]{12}'\.\*; true$", reap), "{reap}");
            let calls = guest.calls();
            let polls = guest.polls();
            let at = |c: &str| calls.iter().position(|x| x == c).expect("the exec was made");
            assert!(at(reap) > at(polls.last().expect("a poll")));
        });
    }

    #[test]
    fn a_leader_that_died_without_an_exit_file_still_gets_its_group_reaped_and_its_file() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("partial output\n"), Step::dead(), Step::none()]);
            let stream = machine_exec_stream(machine, opts(5), None).launch("claude -p 'hi'", Launch::default());
            support::lines_ok(&*stream).await;
            assert_eq!(stream.exited().await, None);
            assert!(!guest.child_alive());
            assert_eq!(guest.files(), Vec::<String>::new());
        });
    }

    #[test]
    fn reassembles_multi_byte_characters_split_across_poll_boundaries() {
        support::run_local(async {
            let line = "café ☕ done\n".as_bytes();
            let (_guest, machine) = ScriptGuest::new(vec![
                Step::append(&line[..4]), // splits the é
                Step::append(&line[4..]).exit(0),
                Step::none(),
            ]);
            let stream = machine_exec_stream(machine, opts(5), None).launch("echo done", Launch::default());
            assert_eq!(support::lines_ok(&*stream).await, vec!["café ☕ done"]);
        });
    }

    #[test]
    fn ends_with_exit_null_when_the_process_dies_without_writing_an_exit_file() {
        support::run_local(async {
            let (_guest, machine) = ScriptGuest::new(vec![Step::append("partial output\n"), Step::dead(), Step::none()]);
            let stream = machine_exec_stream(machine, opts(5), None).launch("claude -p 'hi'", Launch::default());
            assert_eq!(support::lines_ok(&*stream).await, vec!["partial output"]);
            assert_eq!(stream.exited().await, None);
        });
    }

    #[test]
    fn waits_one_more_poll_when_the_leader_is_gone_before_its_exit_file_is_there() {
        support::run_local(async {
            let (_guest, machine) =
                ScriptGuest::new(vec![Step::append("almost\n"), Step::dead(), Step::append("done\n").exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, opts(5), None).launch("claude -p 'hi'", Launch::default());
            assert_eq!(support::lines_ok(&*stream).await, vec!["almost", "done"]);
            assert_eq!(stream.exited().await, Some(0));
        });
    }

    #[test]
    fn kill_sigkills_the_process_group_and_unblocks_exited() {
        support::run_local(async {
            // never exits on its own: every poll just says "up" with no new output
            let (guest, machine) = ScriptGuest::new(vec![]);
            let stream = machine_exec_stream(machine, opts(5), None).launch("sleep 9999", Launch::default());
            let killer = stream.clone();
            tokio::task::spawn_local(async move {
                support::sleep_ms(25).await;
                killer.kill();
            });
            support::lines_ok(&*stream).await;
            assert_eq!(stream.exited().await, None);
            assert!(guest.kills().iter().any(|k| k.contains("kill -KILL")));
            assert!(!guest.child_alive());
            assert_eq!(guest.files(), Vec::<String>::new());
        });
    }

    #[test]
    fn a_run_that_exits_137_is_asked_about_once_from_when_the_turn_started_and_one_this() {
        support::run_local(async {
            let asked = Arc::new(Mutex::new(Vec::<i64>::new()));
            let record = asked.clone();
            let killed_why: KilledWhy = {
                fn typed(f: impl Fn(i64) -> LocalBoxFuture<'static, Option<String>> + 'static) -> KilledWhy {
                    Arc::new(f)
                }
                typed(move |at| {
                    record.lock().unwrap().push(at);
                    async { Some("killed for memory".to_string()) }.boxed()
                })
            };
            let (_guest, machine) = ScriptGuest::new(vec![Step::append("working\n"), Step::only_exit(137), Step::none()]);
            let options = MachineExecOptions {
                poll_ms: Some(5),
                now: Some(Arc::new(|| 1_760_000_000_000_i64)),
                killed_why: Some(killed_why.clone()),
                ..Default::default()
            };
            let stream = machine_exec_stream(machine, options, None).launch("claude -p 'hi'", Launch::default());
            let (lines, failure) = support::lines_err(&*stream).await;
            assert!(failure.message.contains("killed for memory"), "{}", failure.message);
            assert_eq!(lines, vec!["working"]);
            assert_eq!(stream.exited().await, Some(137));
            assert_eq!(*asked.lock().unwrap(), vec![1_760_000_000_000]);

            let (_again, machine) = ScriptGuest::new(vec![]);
            let options = MachineExecOptions { poll_ms: Some(5), killed_why: Some(killed_why), ..Default::default() };
            let stopped = machine_exec_stream(machine, options, None).launch("sleep 9999", Launch::default());
            let killer = stopped.clone();
            tokio::task::spawn_local(async move {
                support::sleep_ms(25).await;
                killer.kill();
            });
            support::lines_ok(&*stopped).await;
            assert_eq!(stopped.exited().await, None);
            assert_eq!(asked.lock().unwrap().len(), 1);
        });
    }

    #[test]
    fn write_appends_one_base64_decoded_line_to_the_input_file_with_one_exec_a_write_af() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::none(), Step::none(), Step::none(), Step::only_exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, opts(5), None).launch("claude", with_input(&["first"]));
            assert_eq!(stream.write(r#"{"type":"user","text":"don't stop"}"#.to_string()).await.unwrap(), WriteOutcome::Written);
            assert_eq!(guest.writes().len(), 1);
            assert!(matches(r"\| base64 -d >> '/tmp/wsp-run/[a-f0-9]+\.in'", &guest.writes()[0]), "{}", guest.writes()[0]);
            assert_eq!(guest.get_input(), "first\n{\"type\":\"user\",\"text\":\"don't stop\"}\n");
            support::lines_ok(&*stream).await;
            let late = stream.write("late".to_string()).await.unwrap_err();
            assert!(late.message.contains("ended"), "{}", late.message);
            assert_eq!(guest.writes().len(), 1);
        });
    }

    #[test]
    fn a_write_after_the_command_exited_but_before_the_poll_saw_it_answers_gone_with_on() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::none(), Step::none(), Step::none(), Step::none()]);
            let now = clock();
            let options = MachineExecOptions { poll_ms: Some(200), idle_ms: Some(1000.0), now: Some(now_of(&now)), ..Default::default() };
            let stream = machine_exec_stream(machine, options, None).launch("claude", with_input(&["first"]));
            let first = next_of(stream.lines());
            support::sleep_ms(20).await;
            guest.exit(0);
            now.store(900, SeqCst);
            assert_eq!(stream.write("late".to_string()).await.unwrap(), WriteOutcome::Gone);
            assert_eq!(guest.writes().len(), 1);
            let pattern = r#"^mkdir -p '/tmp/wsp-run'\n\{ \[ -e '/tmp/wsp-run/[a-f0-9]+'\.exit \] \|\| \[ ! -d '/tmp/wsp-run/[a-f0-9]+\.d' \]; \} && \{ echo WSP_GONE; exit 0; \}\nif \[ -e '/tmp/wsp-run/[a-f0-9]+'\.held \]; then [^\n]+; fi\nset -o pipefail\n\[ "\$\(cat '/tmp/wsp-run/[a-f0-9]+\.in\.appended' 2>/dev/null\)" = '[0-9a-f]{12}\.0' \] \|\| \{ printf %s '[A-Za-z0-9+/=]+' \| base64 -d >> '/tmp/wsp-run/[a-f0-9]+\.in' && printf %s '[0-9a-f]{12}\.0' > '[^']+'; \} \|\| exit 1\necho WSP_OK$"#;
            assert!(matches(pattern, &guest.writes()[0]), "{}", guest.writes()[0]);
            assert_eq!(guest.get_input(), "first\n");
            now.store(1100, SeqCst);
            let (item, _lines) = first.await.unwrap();
            let failure = item.expect("the first read ended in a failure").unwrap_err();
            assert_eq!(failure.message, "stopped after 0m 01s with no output for 0m");
            assert_eq!(stream.exited().await, None);
        });
    }

    #[test]
    fn reads_the_agent_s_reply_to_a_write_at_hurried_polls_not_at_the_next_1500_ms_one() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![]);
            let clock = clock();
            let naps = Arc::new(Mutex::new(Vec::<u64>::new()));
            let reading = ReadingSet::new();
            // A full poll's wait never ends here, so only a write's hurry lets the reply through.
            let record = naps.clone();
            let sleep = sleep_fn(move |ms| {
                record.lock().unwrap().push(ms);
                if ms >= 1500 {
                    futures::future::pending::<()>().boxed()
                } else {
                    support::sleep_ms(1).boxed()
                }
            });
            let options = MachineExecOptions {
                poll_ms: Some(1500),
                now: Some(now_of(&clock)),
                sleep: Some(sleep),
                reading: Some(reading.clone()),
                ..Default::default()
            };
            let stream = machine_exec_stream(machine, options, None).launch("codex app-server", with_input(&["first"]));
            let mut lines = stream.lines();
            guest.append("started\n");
            assert_eq!(line_of(lines.next().await), "started");
            clock.store(3_001, SeqCst);
            let reply = next_of(lines);
            support::wait_for("a nap of 1500", 5_000, || naps.lock().unwrap().last() == Some(&1500)).await;
            assert_eq!(stream.write("interrupt".to_string()).await.unwrap(), WriteOutcome::Written);
            guest.append("interrupted\n");
            let (item, lines) = reply.await.unwrap();
            assert_eq!(line_of(item), "interrupted");
            assert_eq!(naps.lock().unwrap().last(), Some(&200));
            clock.store(6_002, SeqCst);
            let _more = next_of(lines);
            support::wait_for("a nap of 1500", 5_000, || naps.lock().unwrap().last() == Some(&1500)).await;
            reading.let_go_all();
        });
    }

    #[test]
    fn reads_a_launch_at_hurried_polls_until_its_first_line_for_30_s_at_most_and_for_a() {
        support::run_local(async {
            let clock = clock();
            let naps = Arc::new(Mutex::new(Vec::<u64>::new()));
            let record = naps.clone();
            let sleep = sleep_fn(move |ms| {
                record.lock().unwrap().push(ms);
                support::sleep_ms(1).boxed()
            });
            let reading = ReadingSet::new();
            let launch = || {
                let (guest, machine) = ScriptGuest::new(vec![]);
                let options = MachineExecOptions {
                    poll_ms: Some(1500),
                    now: Some(now_of(&clock)),
                    sleep: Some(sleep.clone()),
                    reading: Some(reading.clone()),
                    ..Default::default()
                };
                let stream = machine_exec_stream(machine, options, None).launch("claude -p", with_input(&["say hi"]));
                (guest, stream.lines())
            };
            let (guest, lines) = launch();
            let first = next_of(lines);
            assert_eq!(next_naps(&naps, 3).await, vec![200, 200, 200]);
            guest.append("init\n");
            let (item, lines) = first.await.unwrap();
            assert_eq!(line_of(item), "init");
            let second = next_of(lines);
            assert_eq!(next_naps(&naps, 2).await, vec![200, 200]);
            clock.fetch_add(3_001, SeqCst);
            assert_eq!(*next_naps(&naps, 2).await.last().unwrap(), 1500);
            guest.append("retry\n");
            let (item, lines) = second.await.unwrap();
            assert_eq!(line_of(item), "retry");
            let _third = next_of(lines);
            assert_eq!(next_naps(&naps, 2).await, vec![200, 200]);
            reading.let_go_all();

            let (_silent_guest, silent) = launch();
            let _silent = next_of(silent);
            assert_eq!(next_naps(&naps, 2).await, vec![200, 200]);
            clock.fetch_add(30_001, SeqCst);
            assert_eq!(*next_naps(&naps, 2).await.last().unwrap(), 1500);
            reading.let_go_all();

            let (late_guest, late_lines) = launch();
            let opening = next_of(late_lines);
            late_guest.append("init\n");
            let (item, late_lines) = opening.await.unwrap();
            assert_eq!(line_of(item), "init");
            clock.fetch_add(30_001, SeqCst);
            let after_opening = next_of(late_lines);
            late_guest.append("tool result\n");
            let (item, late_lines) = after_opening.await.unwrap();
            assert_eq!(line_of(item), "tool result");
            let _last = next_of(late_lines);
            assert_eq!(next_naps(&naps, 1).await, vec![1500]);
            reading.let_go_all();
        });
    }

    // The Claude adapter's own Rust API takes the factory and a start's `on_event`, so this one drives it.
    #[test]
    fn hands_claude_code_each_retry_of_a_failing_model_call_before_it_prints_the_next_f() {
        support::run_local(async {
            enum Printed {
                Line(String),
                Exit(i32),
            }
            let (guest, machine) = ScriptGuest::new(vec![]);
            let clock = clock();
            const SID: &str = "5a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
            let retry = |attempt: i64| {
                serde_json::json!({ "type": "system", "subtype": "api_retry", "attempt": attempt, "max_retries": 10, "retry_delay_ms": 500 * 2i64.pow((attempt - 1) as u32), "error_status": null, "error": "unknown", "session_id": SID }).to_string()
            };
            // Claude Code 2.1.301 against a refused route: init after its servers came up, the first retry 17 ms after it and
            // each next one at its backoff (0.56 s, 1.04 s, 2.36 s apart, measured).
            let printed: VecDeque<(i64, Printed)> = VecDeque::from(vec![
                (4_050, Printed::Line(serde_json::json!({ "type": "system", "subtype": "init", "session_id": SID, "cwd": "/root", "tools": [], "model": "claude-opus-5-5" }).to_string())),
                (4_067, Printed::Line(retry(1))),
                (4_627, Printed::Line(retry(2))),
                (5_667, Printed::Line(retry(3))),
                (8_031, Printed::Line(retry(4))),
                (
                    9_000,
                    Printed::Line(serde_json::json!({ "type": "result", "subtype": "success", "is_error": false, "result": "hi", "session_id": SID, "num_turns": 1, "usage": { "input_tokens": 1, "output_tokens": 1 } }).to_string()),
                ),
                (9_010, Printed::Exit(0)),
            ]);
            let printed = Arc::new(Mutex::new(printed));
            let due: Arc<dyn Fn()> = {
                let (printed, guest, clock) = (printed.clone(), guest.clone(), clock.clone());
                Arc::new(move || loop {
                    let next = {
                        let mut queue = printed.lock().unwrap();
                        match queue.front() {
                            Some((at, _)) if *at <= clock.load(SeqCst) => queue.pop_front(),
                            _ => None,
                        }
                    };
                    match next {
                        Some((_, Printed::Exit(code))) => guest.exit(code),
                        Some((_, Printed::Line(text))) => guest.append(&format!("{text}\n")),
                        None => break,
                    }
                })
            };
            // An exec on Boat answers in 0.24 to 0.38 s, and the guest's shell reads the log halfway through it.
            let c = clock.clone();
            guest.set_hook(hook(move |cmd, inner| {
                c.fetch_add(125, SeqCst);
                due();
                let res = inner(&cmd);
                c.fetch_add(125, SeqCst);
                async move { res }
            }));
            let c = clock.clone();
            let sleep = sleep_fn(move |ms| {
                c.fetch_add(ms as i64, SeqCst);
                async {}.boxed()
            });
            let exec = machine_exec_stream(
                machine,
                MachineExecOptions { poll_ms: Some(1500), now: Some(now_of(&clock)), sleep: Some(sleep), ..Default::default() },
                None,
            );
            let retries = Arc::new(Mutex::new(Vec::<(i64, i64)>::new()));
            let (seen, c) = (retries.clone(), clock.clone());
            let adapter = create_claude_adapter(AdapterDeps::new(exec, "/root/.claude-cfg")).expect("the adapter");
            let session = adapter
                .start(HarnessStartOptions {
                    prompt: "say hi".to_string(),
                    on_event: Some(Arc::new(move |e: AdapterEvent| {
                        if let AdapterEvent::TurnRetry { attempt, .. } = e {
                            seen.lock().unwrap().push((attempt as i64, c.load(SeqCst)));
                        }
                    })),
                    ..Default::default()
                })
                .expect("a started turn");
            session.finished().await;
            let retries = retries.lock().unwrap().clone();
            assert_eq!(retries.iter().map(|r| r.0).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
            let next_printed = [4_627, 5_667, 8_031, 9_000];
            for (i, (attempt, at)) in retries.iter().enumerate() {
                assert!(*at < next_printed[i], "retry {attempt} reached the turn at {at} ms");
            }
        });
    }

    #[test]
    fn closeinput_kills_the_recorded_tail_pid_and_nothing_else_once() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::none(), Step::none(), Step::only_exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, opts(5), None).launch("claude", with_input(&["first"]));
            stream.close_input();
            stream.close_input();
            support::lines_ok(&*stream).await;
            let signals = |g: &ScriptGuest| g.kills().into_iter().filter(|k| !k.contains("rm -rf")).collect::<Vec<_>>();
            let before = signals(&guest);
            assert_eq!(before.len(), 1);
            assert!(
                matches(r#"P=\$\(cat '/tmp/wsp-run/[a-f0-9]+'\.tail 2>/dev/null\); \[ -n "\$P" \] && kill -TERM "\$P""#, &before[0]),
                "{}",
                before[0]
            );
            assert!(!before[0].contains("-- -"));
            stream.close_input();
            assert_eq!(signals(&guest).len(), 1);
        });
    }

    #[test]
    fn a_write_restarts_the_idle_clock_past_it_the_stream_ends_with_the_idle_line_and_t() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![]);
            let now = clock();
            let options = MachineExecOptions { poll_ms: Some(1), idle_ms: Some(1000.0), now: Some(now_of(&now)), ..Default::default() };
            let stream = machine_exec_stream(machine, options, None).launch("claude", with_input(&["first"]));
            let first = next_of(stream.lines());
            now.store(900, SeqCst);
            stream.write("more".to_string()).await.unwrap();
            now.store(1800, SeqCst);
            support::sleep_ms(20).await;
            assert_eq!(guest.kills(), Vec::<String>::new());
            now.store(2000, SeqCst);
            let (item, _lines) = first.await.unwrap();
            assert_eq!(item.expect("a failure").unwrap_err().message, "stopped after 0m 02s with no output for 0m");
            assert_eq!(stream.exited().await, None);
            assert!(guest.kills().iter().any(|k| k.contains("kill -KILL -- -$P")));
            assert!(!guest.child_alive());
            assert_eq!(guest.files(), Vec::<String>::new());
        });
    }

    #[test]
    fn a_turn_stopped_on_a_question_only_a_person_can_answer_is_not_cut_at_the_idle_lim() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![]);
            let now = clock();
            let waiting = Arc::new(AtomicBool::new(true));
            let asks: TurnWaiting = {
                let waiting = waiting.clone();
                Arc::new(move || waiting.load(SeqCst))
            };
            let options = MachineExecOptions { poll_ms: Some(1), idle_ms: Some(1000.0), now: Some(now_of(&now)), ..Default::default() };
            let stream = machine_exec_stream(machine, options, Some(asks)).launch("claude", with_input(&["first"]));
            let first = next_of(stream.lines());
            now.store(9000, SeqCst);
            support::sleep_ms(20).await;
            assert_eq!(guest.kills(), Vec::<String>::new());
            // The answer lands and the clock runs again from there.
            waiting.store(false, SeqCst);
            now.store(9500, SeqCst);
            support::sleep_ms(20).await;
            assert_eq!(guest.kills(), Vec::<String>::new());
            now.store(10_600, SeqCst);
            let (item, _lines) = first.await.unwrap();
            let failure = item.expect("a failure").unwrap_err();
            assert!(failure.message.ends_with("with no output for 0m"), "{}", failure.message);
            assert_eq!(stream.exited().await, None);
        });
    }

    #[test]
    fn a_stream_that_prints_a_line_every_minute_runs_past_the_old_15_minute_wall_clock() {
        support::run_local(async {
            let (guest, machine, clock) = minute_guest(21 * 60_000, 21 * 60_000, 0);
            let options = MachineExecOptions { poll_ms: Some(1), now: Some(now_of(&clock)), ..Default::default() };
            let stream = machine_exec_stream(machine, options, None).launch("claude", Launch::default());
            let seen = support::lines_ok(&*stream).await;
            assert_eq!(seen.len(), 20);
            assert_eq!(seen.last().unwrap(), "tick 20");
            assert!(clock.load(SeqCst) > 900_000);
            assert_eq!(stream.exited().await, Some(0));
            assert_eq!(guest.files(), Vec::<String>::new());
        });
    }

    #[test]
    fn a_stream_that_goes_silent_is_cut_once_no_byte_came_for_the_idle_limit_and_the_er() {
        support::run_local(async {
            let (guest, machine, clock) = minute_guest(6 * 60_000, i64::MAX, 0);
            let options = MachineExecOptions { poll_ms: Some(1), now: Some(now_of(&clock)), ..Default::default() };
            let stream = machine_exec_stream(machine, options, None).launch("claude", Launch::default());
            let (seen, failure) = support::lines_err(&*stream).await;
            assert_eq!(failure.message, "stopped after 15m 00s with no output for 10m");
            assert_eq!(seen.len(), 5);
            // Five minutes of lines, then the ten minute idle limit (TURN_IDLE_MS = 600_000).
            assert_eq!(clock.load(SeqCst), 5 * 60_000 + 600_000);
            assert_eq!(stream.exited().await, None);
            assert!(guest.kills().iter().any(|k| k.contains("kill -KILL -- -$P")));
            assert!(!guest.child_alive());
            assert_eq!(guest.files(), Vec::<String>::new());
        });
    }

    #[test]
    fn a_run_re_opened_after_a_restart_is_cut_at_its_first_start_plus_the_wall_not_at_t() {
        support::run_local(async {
            let (guest, machine, clock) = minute_guest(i64::MAX, i64::MAX, 0);
            let reading = ReadingSet::new();
            let options = MachineExecOptions {
                poll_ms: Some(1),
                deadline_ms: Some(30.0 * 60_000.0),
                now: Some(now_of(&clock)),
                reading: Some(reading.clone()),
                ..Default::default()
            };
            let launched = machine_exec_stream(machine.clone(), options, None).launch("claude", Launch::default());
            support::wait_for("the launch to land", 5_000, || !guest.get_launch().is_empty()).await;
            // The host goes and comes back twenty minutes into the turn; the next one re-opens the run by handle.
            reading.let_go_all();
            clock.store(20 * 60_000, SeqCst);
            let options = MachineExecOptions {
                poll_ms: Some(1),
                deadline_ms: Some(30.0 * 60_000.0),
                now: Some(now_of(&clock)),
                ..Default::default()
            };
            let attached = machine_exec_stream(machine, options, None)
                .attach(&launched.run().expect("a handle"), AttachRunOptions { input: false, started_at: 0.0, from: None })
                .expect("an attach")
                .await;
            let Attached::Stream(attached) = attached.expect("an attach that answers") else { panic!("the run was gone") };
            let (_lines, failure) = support::lines_err(&*attached).await;
            assert_eq!(failure.message, format!("stopped after 30m 00s at the 30m turn limit; {WALL_WORDS}"));
            assert_eq!(clock.load(SeqCst), 30 * 60_000);
        });
    }

    #[test]
    fn a_run_that_prints_nothing_while_its_process_group_works_runs_past_the_idle_limit() {
        support::run_local(async {
            let (guest, machine, clock) = minute_guest(0, i64::MAX, CORE_MINUTE);
            let options = MachineExecOptions {
                poll_ms: Some(1),
                deadline_ms: Some(30.0 * 60_000.0),
                now: Some(now_of(&clock)),
                ..Default::default()
            };
            let stream = machine_exec_stream(machine, options, None).launch("claude -p 'build it'", Launch::default());
            let (seen, failure) = support::lines_err(&*stream).await;
            assert_eq!(failure.message, format!("stopped after 30m 00s at the 30m turn limit; {WALL_WORDS}"));
            assert_eq!(seen, Vec::<String>::new());
            assert_eq!(clock.load(SeqCst), 30 * 60_000);
            assert_eq!(stream.exited().await, None);
            assert!(!guest.child_alive());
        });
    }

    #[test]
    fn a_run_that_prints_nothing_while_its_group_does_less_than_the_work_floor_is_still() {
        support::run_local(async {
            let (guest, machine, clock) = minute_guest(0, i64::MAX, 100);
            let options = MachineExecOptions { poll_ms: Some(1), now: Some(now_of(&clock)), ..Default::default() };
            let stream = machine_exec_stream(machine, options, None).launch("claude -p 'hi'", Launch::default());
            let (_lines, failure) = support::lines_err(&*stream).await;
            assert_eq!(failure.message, "stopped after 10m 00s with no output for 10m");
            // TURN_IDLE_MS
            assert_eq!(clock.load(SeqCst), 600_000);
            assert_eq!(stream.exited().await, None);
            assert!(!guest.child_alive());
            assert_eq!(guest.files(), Vec::<String>::new());
        });
    }

    #[test]
    fn a_road_that_goes_dark_holds_the_turn_s_idle_clock_which_goes_on_from_the_road_s() {
        support::run_local(async {
            // Quiet from its first second, its exit written at the twenty fourth minute. The road is dark from the fifth
            // minute to the twentieth, which on its own outlasts the idle limit.
            let (guest, machine, clock, scripted) = minute_parts(0, 24 * 60_000, 0);
            let dark = Arc::new(AtomicI64::new(0));
            let (c, d) = (clock.clone(), dark.clone());
            guest.set_hook(Arc::new(
                move |cmd: String, stdin: Option<Vec<u8>>, inner: Inner| -> LocalBoxFuture<'static, EngineResult<ExecResult>> {
                    let now = c.load(SeqCst);
                    if cmd.contains("__WSP_EOF_") && (5 * 60_000..20 * 60_000).contains(&now) {
                        c.fetch_add(60_000, SeqCst);
                        d.fetch_add(1, SeqCst);
                        return async { Err(errors::fetch_failed("EAI_AGAIN")) }.boxed();
                    }
                    scripted(cmd, stdin, inner)
                },
            ));
            let options = MachineExecOptions { poll_ms: Some(1), now: Some(now_of(&clock)), ..Default::default() };
            let stream = machine_exec_stream(machine, options, None).launch("claude -p 'build it'", Launch::default());
            support::lines_ok(&*stream).await;
            // TURN_IDLE_MS / 60_000
            assert!(dark.load(SeqCst) > 10, "the road was dark for {} polls", dark.load(SeqCst));
            assert_eq!(stream.exited().await, Some(0));
            assert!(clock.load(SeqCst) >= 24 * 60_000);
            assert_eq!(guest.files(), Vec::<String>::new());
        });
    }

    // The part of the TS case that runs the awk over a fake /proc is in tests/machine_exec_rules.rs.
    #[test]
    fn the_poll_asks_for_the_tree_s_work_only_once_the_stream_has_been_quiet_for_half_t() {
        support::run_local(async {
            let (guest, machine, clock) = minute_guest(0, 8 * 60_000, CORE_MINUTE);
            let options = MachineExecOptions { poll_ms: Some(1), now: Some(now_of(&clock)), ..Default::default() };
            let stream = machine_exec_stream(machine, options, None).launch("claude -p 'hi'", Launch::default());
            support::lines_ok(&*stream).await;
            // One poll a minute on a silent stream, so the first five are inside the half of a ten minute limit.
            let polls = guest.polls();
            assert!(!polls[..5].iter().any(|c| c.contains("awk -v p=")));
            assert!(
                polls[5].contains(&format!("W=$(awk -v p=\"$P\" {} /proc/[0-9]*/stat 2>/dev/null)", support::sq(TREE_WORK_AWK))),
                "{}",
                polls[5]
            );

            // The exec verb hands both limits infinite on purpose, and no reading could cut such a stream.
            let (exec, machine) = ScriptGuest::new(vec![Step::none(), Step::none(), Step::append("built\n").exit(0), Step::none()]);
            let options = MachineExecOptions {
                poll_ms: Some(1),
                idle_ms: Some(f64::INFINITY),
                deadline_ms: Some(f64::INFINITY),
                ..Default::default()
            };
            let run = machine_exec_stream(machine, options, None).launch("pnpm build", Launch::default());
            support::lines_ok(&*run).await;
            assert!(exec.polls().len() > 2);
            assert!(!exec.calls().iter().any(|c| c.contains("awk -v p=")));
        });
    }

    #[test]
    fn a_stream_that_never_goes_quiet_is_still_cut_at_the_wall_cap_and_the_error_names() {
        support::run_local(async {
            let (guest, machine, clock) = minute_guest(i64::MAX, i64::MAX, 0);
            let options = MachineExecOptions {
                poll_ms: Some(1),
                deadline_ms: Some(2.0 * 3_600_000.0),
                now: Some(now_of(&clock)),
                ..Default::default()
            };
            let stream = machine_exec_stream(machine, options, None).launch("claude", Launch::default());
            let (seen, failure) = support::lines_err(&*stream).await;
            assert_eq!(failure.message, format!("stopped after 2h 00m 00s at the 2h turn limit; {WALL_WORDS}"));
            assert_eq!(seen.len(), 120);
            assert_eq!(stream.exited().await, None);
            assert!(!guest.child_alive());
        });
    }

    #[test]
    fn tolerates_exec_failures_mid_poll_a_napping_machine_and_finishes_after_recovery() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("before nap\n"), Step::append("after wake\n").exit(0), Step::none()]);
            let failures = Arc::new(AtomicI32::new(3));
            let f = failures.clone();
            guest.set_hook(hook(move |cmd, inner| {
                let failing = cmd.contains("__WSP_EOF_") && f.load(SeqCst) > 0;
                if failing {
                    f.fetch_sub(1, SeqCst);
                }
                let res = if failing { Err(errors::refusal(ErrorKind::Conflict, 409, "machine paused")) } else { inner(&cmd) };
                async move { res }
            }));
            let stream = machine_exec_stream(machine, opts(5), None).launch("long task", Launch::default());
            assert_eq!(support::lines_ok(&*stream).await, vec!["before nap", "after wake"]);
            assert_eq!(stream.exited().await, Some(0));
        });
    }
}

mod machine_exec_stream_reaping_a_real_turn_s_process_group {
    use super::*;

    own_group_test!(a_child_the_command_left_in_the_background_is_gone_once_the_turn_ended_and_the_r, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let child_file = beside(&run_dir, "child");
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None)
            .launch(&format!("sleep 300 & echo $! > {}; echo hi", support::sq(&child_file)), Launch::default());
        let lines = support::lines_ok(&*stream).await;
        assert_eq!(stream.exited().await, Some(0));
        assert_eq!(lines, vec!["hi"]);
        let child = strays::grandchild(&child_file).await;
        assert!(child > 0);
        strays::gone(child).await;
        assert_eq!(support::names(&run_dir), Vec::<String>::new());
    });
}

mod machine_exec_stream_ending_a_run_that_ignores_term {
    use super::*;

    own_group_test!(what_it_printed_after_the_last_poll_is_read_before_the_kill_s_reap_takes_the_log, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        // A poll slower than the grace, so no read falls between the TERM and the KILL: the lines are read at the end or never.
        let command = format!(
            "trap 'head -c {} /dev/zero | tr \"\\0\" x; echo; echo last words' TERM; echo ready; {}",
            EXEC_CHUNK_BYTES + 1000,
            support::gate(&format!("{run_dir}/stop"))
        );
        let stream = machine_exec_stream(machine, local_opts(1_500, &run_dir), None).launch(&command, Launch::default());
        let mut lines = stream.lines();
        assert_eq!(line_of(lines.next().await), "ready");
        support::sleep_ms(100).await;
        let ender = stream.clone();
        let ended = tokio::task::spawn_local(async move { end_run(&*ender, 300).await });
        let mut rest = Vec::new();
        while let Some(next) = lines.next().await {
            rest.push(next.expect("a line, not a failure"));
        }
        ended.await.unwrap();
        assert!(rest.contains(&"x".repeat(EXEC_CHUNK_BYTES + 1000)));
        assert_eq!(rest.last().unwrap(), "last words");
        assert_eq!(support::names(&run_dir), Vec::<String>::new());
    });
}

mod machine_exec_stream_ending_a_run_something_outside_its_group_still_writes_to {
    use super::*;

    /// The guest at the far end of a link that takes `ms` each way round, as a cloud machine's exec does.
    fn slow_guest(ms: u64) -> (Arc<dyn Machine>, String, tempfile::TempDir) {
        let (inner, run_dir, dir) = local();
        let machine: Arc<dyn Machine> = Arc::new(Wrapped {
            inner,
            f: Arc::new(move |m: Arc<dyn Machine>, cmd: String, o: MachineOptions| -> LocalBoxFuture<'static, EngineResult<ExecResult>> {
                Box::pin(async move {
                    support::sleep_ms(ms).await;
                    m.exec(&cmd, o).await
                })
            }),
        });
        (machine, run_dir, dir)
    }

    /// A command that starts a writer in a session of its own, holding the log at about 2 MB/s for `s` seconds, and
    /// writes the writer's group to `marker`.
    fn leaves_a_writer(marker: &str, s: u64) -> String {
        format!(
            "setsid sh -c 'end=$(($(date +%s) + {s})); while [ $(date +%s) -lt $end ]; do head -c 65536 /dev/zero | tr \"\\0\" y; echo; sleep 0.03; done' & echo $! > {}; echo ready; sleep 30",
            support::sq(marker)
        )
    }

    own_group_test!(a_stop_reads_the_log_to_the_size_it_had_at_the_kill_and_ends_while_the_writer_go, 30_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = slow_guest(200);
        let marker = beside(&run_dir, "writer");
        let stream = machine_exec_stream(machine, local_opts(100, &run_dir), None).launch(&leaves_a_writer(&marker, 60), Launch::default());
        let reading = read(&stream);
        let writer = strays::grandchild(&marker).await;
        support::sleep_ms(1_000).await;
        stream.kill();
        reading.done().await;
        stream.exited().await;
        // The stop ended while the writer still prints: on a link slower than the writer, a read to the end of the log
        // ends only once the writer does.
        assert!(group_alive(writer));
        assert_eq!(support::names(&run_dir), Vec::<String>::new());
    });

    own_group_test!(a_launch_read_as_failed_takes_the_run_s_files_and_its_claim_however_much_its_log, 15_000, {
        let _sweep = Sweep;
        let (guest, run_dir, _dir) = local();
        fn logged(dir: &str) -> u64 {
            support::names(dir)
                .iter()
                .filter(|name| name.ends_with(".log"))
                .map(|name| std::fs::metadata(format!("{dir}/{name}")).map(|m| m.len()).unwrap_or(0))
                .sum()
        }
        // The run starts, and the launch's answer comes back without the word, once the log holds more than a chunk.
        let dir = run_dir.clone();
        let machine: Arc<dyn Machine> = Arc::new(Wrapped {
            inner: guest,
            f: Arc::new(move |m: Arc<dyn Machine>, cmd: String, o: MachineOptions| -> LocalBoxFuture<'static, EngineResult<ExecResult>> {
                let dir = dir.clone();
                Box::pin(async move {
                    let res = m.exec(&cmd, o).await?;
                    if !cmd.contains("WSP_LAUNCHED") {
                        return Ok(res);
                    }
                    support::wait_for("the log to hold more than a chunk", 5_000, || logged(&dir) > EXEC_CHUNK_BYTES as u64 + 1000).await;
                    Ok(ExecResult { stdout: String::new(), ..res })
                })
            }),
        });
        let command = format!("head -c {} /dev/zero | tr '\\0' x; echo; sleep 30", EXEC_CHUNK_BYTES + 2000);
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None).launch(&command, Launch::default());
        let (_lines, failure) = support::lines_err(&*stream).await;
        assert!(failure.message.contains("remote launch failed"), "{}", failure.message);
        assert_eq!(stream.exited().await, None);
        assert_eq!(support::names(&run_dir), Vec::<String>::new());
    });
}

mod machine_exec_stream_feeding_a_real_process_over_the_input_channel {
    use super::*;

    own_group_test!(the_seeded_line_and_a_later_write_reach_the_command_s_stdin_in_order_closeinput, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None).launch("cat", with_input(&["hello"]));
        let reading = read(&stream);
        support::wait_for("a tail pid file", 5_000, || file_with(&run_dir, ".tail").is_some()).await;
        let tail = strays::grandchild(&format!("{run_dir}/{}", file_with(&run_dir, ".tail").unwrap())).await;
        assert!(tail > 0);
        assert_eq!(stream.write("world".to_string()).await.unwrap(), WriteOutcome::Written);
        support::wait_for("both lines", 5_000, || reading.lines() == vec!["hello", "world"]).await;
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["hello", "world"]);
        assert_eq!(stream.exited().await, Some(0));
        strays::gone(tail).await;
    });

    own_group_test!(a_write_after_the_command_exited_answers_gone_and_the_input_file_keeps_only_what, 15_000, {
        let _sweep = Sweep;
        let (inner, run_dir, _dir) = local();
        let hold_polls = Arc::new(AtomicBool::new(false));
        let held = Arc::new(Mutex::new(Vec::<oneshot::Sender<()>>::new()));
        let (hold, parked) = (hold_polls.clone(), held.clone());
        let gated: Arc<dyn Machine> = Arc::new(Wrapped {
            inner,
            f: Arc::new(move |m: Arc<dyn Machine>, cmd: String, o: MachineOptions| -> LocalBoxFuture<'static, EngineResult<ExecResult>> {
                let (hold, parked) = (hold.clone(), parked.clone());
                Box::pin(async move {
                    if hold.load(SeqCst) && cmd.contains("__WSP_EOF_") {
                        let (tx, rx) = oneshot::channel::<()>();
                        parked.lock().unwrap().push(tx);
                        let _ = rx.await;
                    }
                    m.exec(&cmd, o).await
                })
            }),
        });
        let stream = machine_exec_stream(gated, local_opts(20, &run_dir), None).launch("cat", with_input(&["hello"]));
        let reading = read(&stream);
        support::wait_for("a tail pid file", 5_000, || !text_of(&run_dir, ".tail").trim().is_empty()).await;
        strays::grandchild(&format!("{run_dir}/{}", file_with(&run_dir, ".tail").unwrap())).await;
        hold_polls.store(true, SeqCst);
        stream.close_input();
        support::wait_for("an exit file", 5_000, || file_with(&run_dir, ".exit").is_some()).await;
        assert_eq!(stream.write("late".to_string()).await.unwrap(), WriteOutcome::Gone);
        assert_eq!(text_of(&run_dir, ".in"), "hello\n");
        hold_polls.store(false, SeqCst);
        let releases: Vec<_> = held.lock().unwrap().drain(..).collect();
        for release in releases {
            let _ = release.send(());
        }
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["hello"]);
        assert_eq!(stream.exited().await, Some(0));
        assert_eq!(support::names(&run_dir), Vec::<String>::new());
    });
}

mod machine_exec_stream_holding_a_seed_until_it_is_due {
    use super::*;

    own_group_test!(starts_the_command_at_once_hands_it_the_seed_once_due_and_not_before_and_leaves, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let (input_after, due) = settles();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None)
            .launch("cat", Launch { input: Some(vec!["hello".to_string()]), input_after: Some(input_after), ..Default::default() });
        let reading = read(&stream);
        support::wait_for("a tail pid", 5_000, || !text_of(&run_dir, ".tail").trim().is_empty()).await;
        assert_eq!(text_of(&run_dir, ".in"), "");
        support::sleep_ms(100).await;
        assert_eq!(reading.lines(), Vec::<String>::new());
        due.send(Ok(())).unwrap();
        support::wait_for("the seed's line", 5_000, || reading.lines() == vec!["hello"]).await;
        assert_eq!(file_with(&run_dir, ".held"), None);
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["hello"]);
        assert_eq!(stream.exited().await, Some(0));
    });

    own_group_test!(puts_a_line_written_before_the_seed_is_due_after_the_seed_and_hands_the_seed_ove, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let (input_after, due) = settles();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None)
            .launch("cat", Launch { input: Some(vec!["hello".to_string()]), input_after: Some(input_after), ..Default::default() });
        let reading = read(&stream);
        assert_eq!(stream.write("world".to_string()).await.unwrap(), WriteOutcome::Written);
        support::wait_for("both lines", 5_000, || reading.lines() == vec!["hello", "world"]).await;
        due.send(Ok(())).unwrap();
        support::sleep_ms(200).await;
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["hello", "world"]);
    });

    own_group_test!(a_host_that_attaches_to_a_run_whose_seed_is_still_held_hands_the_seed_over_and_r, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let factory = machine_exec_stream(machine.clone(), local_opts(20, &run_dir), None);
        let launched =
            factory.launch("cat", Launch { input: Some(vec!["hello".to_string()]), input_after: Some(never()), ..Default::default() });
        support::wait_for("a held seed", 5_000, || file_with(&run_dir, ".held").is_some()).await;
        let run = launched.run().expect("a handle");
        let attached = machine_exec_stream(machine, local_opts(20, &run_dir), None)
            .attach(&run, AttachRunOptions { input: true, started_at: now_ms(), from: None })
            .expect("an attach")
            .await;
        let Attached::Stream(stream) = attached.expect("an attach that answers") else { panic!("the run was gone") };
        assert_eq!(stream.taken(), Some(vec!["hello".to_string()]));
        let reading = read(&stream);
        support::wait_for("the seed's line", 5_000, || reading.lines() == vec!["hello"]).await;
        assert_eq!(file_with(&run_dir, ".held"), None);
        stream.close_input();
        reading.done().await;
        assert_eq!(stream.exited().await, Some(0));
        launched.kill();
    });
}

mod machine_exec_stream_holding_a_line_written_ahead_of_when_it_is_due {
    use super::*;

    own_group_test!(parks_the_line_beside_the_run_and_appends_it_once_due_after_what_the_channel_too, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None).launch("cat", with_input(&["one"]));
        let reading = read(&stream);
        support::wait_for("the first line", 5_000, || reading.lines() == vec!["one"]).await;
        let (after, due) = settles();
        assert!(stream.write_after("two".to_string(), after).is_some());
        support::wait_for("a parked line", 5_000, || file_with(&run_dir, ".late").is_some()).await;
        support::sleep_ms(100).await;
        assert_eq!(reading.lines(), vec!["one"]);
        due.send(Ok(())).unwrap();
        support::wait_for("both lines", 5_000, || reading.lines() == vec!["one", "two"]).await;
        assert_eq!(file_with(&run_dir, ".late"), None);
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["one", "two"]);
    });

    // machine-exec.test.ts:1752-1769 (62fededd7): a wait refused before its line is handed over drops the line. The TS
    // takes the wait's outcome when writeAfter is called, which a Rust future shows as being polled at once.
    own_group_test!(a_wait_already_refused_while_an_earlier_line_is_still_parked_is_taken_at_once_and_drops_its_line, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None).launch("cat", with_input(&["one"]));
        let reading = read(&stream);
        support::wait_for("the first line", 5_000, || reading.lines() == vec!["one"]).await;
        assert!(stream.write_after("first".to_string(), after_ms(400)).is_some());
        let polled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let noted = polled.clone();
        let refused: Settle = Box::pin(async move {
            noted.store(true, std::sync::atomic::Ordering::SeqCst);
            Err("stopped".to_string())
        });
        assert!(stream.write_after("never".to_string(), refused).is_some());
        support::sleep_ms(100).await;
        assert!(
            polled.load(std::sync::atomic::Ordering::SeqCst),
            "the refused wait was not taken while an earlier line still held the queue"
        );
        support::wait_for("the first parked line", 5_000, || reading.lines() == vec!["one", "first"]).await;
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["one", "first"]);
    });

    own_group_test!(drops_the_line_when_what_it_waits_on_is_refused, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None).launch("cat", with_input(&["one"]));
        let reading = read(&stream);
        let (after, refuse) = settles();
        assert!(stream.write_after("two".to_string(), after).is_some());
        support::wait_for("a parked line", 5_000, || file_with(&run_dir, ".late").is_some()).await;
        refuse.send(Err("stopped".to_string())).unwrap();
        support::wait_for("the parked line gone", 5_000, || file_with(&run_dir, ".late").is_none()).await;
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["one"]);
    });

    own_group_test!(hands_a_held_seed_over_once_when_a_write_lands_as_it_comes_due_and_a_prompt_with, 20_000, {
        let _sweep = Sweep;
        // The guest's cat reads a held seed at once and prints it a second later, so two hand-overs that ran at once would
        // both have found the channel empty and both append it.
        let shell = format!(
            "{}\n",
            r#"cat() { case "$1" in *.held) local t; t=$(command cat "$1"); sleep 1; printf '%s\n' "$t"; return ;; esac; command cat "$@"; }"#
        );
        let (machine, run_dir, _dir) = local_with(false, &shell);
        let (input_after, due) = settles();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None)
            .launch("cat", Launch { input: Some(vec!["seed".to_string()]), input_after: Some(input_after), ..Default::default() });
        let reading = read(&stream);
        support::wait_for("a held seed", 5_000, || file_with(&run_dir, ".held").is_some()).await;
        due.send(Ok(())).unwrap();
        assert_eq!(stream.write("steer".to_string()).await.unwrap(), WriteOutcome::Written);
        support::wait_for("seed and steer", 5_000, || reading.lines() == vec!["seed", "steer"]).await;
        let yes = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"yes"}]},"parent_tool_use_id":null,"session_id":"s"}"#;
        assert!(stream.write_after(yes.to_string(), ready()).is_some());
        support::wait_for("the first yes", 5_000, || reading.lines() == vec!["seed", "steer", yes]).await;
        assert!(stream.write_after(yes.to_string(), ready()).is_some());
        support::wait_for("the second yes", 5_000, || reading.lines() == vec!["seed", "steer", yes, yes]).await;
        support::sleep_ms(300).await;
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["seed", "steer", yes, yes]);
    });

    own_group_test!(parks_the_line_as_the_login_s_alone_as_every_other_file_of_the_run, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None)
            .launch("cat", Launch { input: Some(vec!["one".to_string()]), input_after: Some(never()), ..Default::default() });
        let reading = read(&stream);
        support::wait_for("a held seed", 5_000, || file_with(&run_dir, ".held").is_some()).await;
        assert!(stream.write_after("two".to_string(), never()).is_some());
        support::wait_for("a parked line", 5_000, || file_with(&run_dir, ".late").is_some()).await;
        let mode = |suffix: &str| support::mode_of(&format!("{run_dir}/{}", file_with(&run_dir, suffix).unwrap()));
        assert_eq!([mode(".in"), mode(".held"), mode(".late")], ["600", "600", "600"]);
        stream.kill();
        let _ = reading.ended().await;
    });

    own_group_test!(puts_a_line_written_while_another_is_parked_after_the_parked_one, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None).launch("cat", with_input(&["one"]));
        let reading = read(&stream);
        support::wait_for("the first line", 5_000, || reading.lines() == vec!["one"]).await;
        let (after, due) = settles();
        assert!(stream.write_after("prompt".to_string(), after).is_some());
        support::wait_for("a parked line", 5_000, || file_with(&run_dir, ".late").is_some()).await;
        let steered = tokio::task::spawn_local(stream.write("steer".to_string()));
        support::sleep_ms(100).await;
        due.send(Ok(())).unwrap();
        assert_eq!(steered.await.unwrap().unwrap(), WriteOutcome::Written);
        support::wait_for("all three lines", 5_000, || reading.lines() == vec!["one", "prompt", "steer"]).await;
        stream.close_input();
        reading.done().await;
    });

    // `write` is called, `write_after` called straight after, and only then is the write awaited: the write is
    // ahead of the parked line only if it took its place in the queue when it was called, as the TS async body does.
    own_group_test!(puts_a_line_written_in_the_tick_before_another_is_parked_ahead_of_it_as_a_kept_c, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None).launch("cat", with_input(&["one"]));
        let reading = read(&stream);
        support::wait_for("the first line", 5_000, || reading.lines() == vec!["one"]).await;
        let resumed = stream.write("resume".to_string());
        assert!(stream.write_after("turn".to_string(), after_ms(200)).is_some());
        assert_eq!(resumed.await.unwrap(), WriteOutcome::Written);
        support::wait_for("all three lines", 5_000, || reading.lines() == vec!["one", "resume", "turn"]).await;
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["one", "resume", "turn"]);
    });

    own_group_test!(hands_two_lines_parked_one_after_the_other_over_in_order_neither_lost, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None).launch("cat", with_input(&["one"]));
        let reading = read(&stream);
        support::wait_for("the first line", 5_000, || reading.lines() == vec!["one"]).await;
        let (first_after, first) = settles();
        let (second_after, second) = settles();
        assert!(stream.write_after("two".to_string(), first_after).is_some());
        support::wait_for("a parked line", 5_000, || file_with(&run_dir, ".late").is_some()).await;
        assert!(stream.write_after("three".to_string(), second_after).is_some());
        second.send(Ok(())).unwrap();
        support::sleep_ms(200).await;
        assert_eq!(reading.lines(), vec!["one"]);
        first.send(Ok(())).unwrap();
        support::wait_for("all three lines", 5_000, || reading.lines() == vec!["one", "two", "three"]).await;
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["one", "two", "three"]);
    });

    own_group_test!(a_line_parked_while_the_seed_is_still_held_goes_after_the_seed_which_is_not_lost, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let (input_after, due) = settles();
        let stream = machine_exec_stream(machine, local_opts(20, &run_dir), None)
            .launch("cat", Launch { input: Some(vec!["seed".to_string()]), input_after: Some(input_after), ..Default::default() });
        let reading = read(&stream);
        assert!(stream.write_after("late".to_string(), ready()).is_some());
        support::wait_for("seed and late", 5_000, || reading.lines() == vec!["seed", "late"]).await;
        due.send(Ok(())).unwrap();
        support::sleep_ms(300).await;
        stream.close_input();
        let lines = reading.seen.clone();
        reading.done().await;
        assert_eq!(*lines.lock().unwrap(), vec!["seed", "late"]);
    });

    own_group_test!(a_host_that_attaches_to_the_run_hands_a_parked_line_over_once_after_the_channel, 15_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let launched = machine_exec_stream(machine.clone(), local_opts(20, &run_dir), None).launch("cat", with_input(&["one"]));
        let first = read(&launched);
        support::wait_for("the first line", 5_000, || first.lines() == vec!["one"]).await;
        assert!(launched.write_after("two".to_string(), never()).is_some());
        support::wait_for("a parked line", 5_000, || file_with(&run_dir, ".late").is_some()).await;
        let attached = machine_exec_stream(machine, local_opts(20, &run_dir), None)
            .attach(&launched.run().expect("a handle"), AttachRunOptions { input: true, started_at: now_ms(), from: None })
            .expect("an attach")
            .await;
        let Attached::Stream(stream) = attached.expect("an attach that answers") else { panic!("the run was gone") };
        assert_eq!(stream.taken(), Some(vec!["one".to_string(), "two".to_string()]));
        let reading = read(&stream);
        support::wait_for("both lines", 5_000, || reading.lines() == vec!["one", "two"]).await;
        assert_eq!(file_with(&run_dir, ".late"), None);
        stream.close_input();
        reading.done().await;
        launched.kill();
    });
}

mod machine_exec_stream_polling_a_script_that_ends_at_once {
    use super::*;

    // Twenty rounds of three to five real bash execs each: 3 s on an idle Mac, 24 s seen at load average 330.
    own_group_test!(keeps_the_last_line_and_the_exit_code_when_the_poll_lands_as_the_script_ends, 60_000, {
        let _sweep = Sweep;
        let (machine, run_dir, _dir) = local();
        let mut results: Vec<(Vec<String>, Option<i32>)> = Vec::new();
        for _ in 0..20 {
            let stream = machine_exec_stream(machine.clone(), local_opts(1, &run_dir), None).launch("echo hi", Launch::default());
            let lines = support::lines_ok(&*stream).await;
            results.push((lines, stream.exited().await));
        }
        let expected: Vec<(Vec<String>, Option<i32>)> = (0..20).map(|_| (vec!["hi".to_string()], Some(0))).collect();
        assert_eq!(results, expected);
    });
}

mod a_reader_that_lets_go_of_a_run_still_going {
    use super::*;

    #[test]
    fn ends_its_poll_where_it_stands_settles_nothing_reaps_nothing_and_the_run_is_left() {
        support::run_local(async {
            // A run that prints nothing and never ends, which is what a turn left running on a machine is.
            let (guest, machine) = ScriptGuest::new(vec![]);
            let reading = ReadingSet::new();
            let factory = machine_exec_stream(
                machine,
                MachineExecOptions { poll_ms: Some(10), reading: Some(reading.clone()), ..Default::default() },
                None,
            );
            let stream = factory.launch("claude -p hi", Launch::default());
            // Nothing awaits this: the reader is the poll, and letting go is what this case is about.
            let reader = stream.clone();
            tokio::task::spawn_local(async move {
                let _ = support::collect(&*reader).await;
            });
            support::wait_for("more than one poll", 1_000, || guest.polls().len() > 1).await;
            // The reader registered itself with the wiring that made the factory, as the call that stops it.
            assert_eq!(reading.len(), 1);

            reading.let_go_all();
            assert_eq!(reading.len(), 0);
            let after = guest.polls().len();
            support::sleep_ms(100).await;
            assert_eq!(guest.polls().len(), after);
            // The turn is not written off: nothing was signalled, nothing reaped, and the stream settles for nobody, so
            // whoever owns that turn reads its log from the first byte when it opens it again.
            assert_eq!(guest.kills(), Vec::<String>::new());
            assert!(guest.child_alive());
            assert!(guest.files().contains(&format!("{}.sh", stream.run().expect("a handle"))), "{:?}", guest.files());
            let settled = support::within(50, stream.exited()).await;
            assert_eq!(settled, None, "the stream settled");
        });
    }

    #[test]
    fn is_dropped_from_the_set_the_moment_its_run_ends_so_a_close_after_that_stops_noth() {
        support::run_local(async {
            let (_guest, machine) = ScriptGuest::new(vec![Step::append("done\n").exit(0), Step::none()]);
            let reading = ReadingSet::new();
            let stream = machine_exec_stream(
                machine,
                MachineExecOptions { poll_ms: Some(5), reading: Some(reading.clone()), ..Default::default() },
                None,
            )
            .launch("claude -p hi", Launch::default());
            support::lines_ok(&*stream).await;
            assert_eq!(stream.exited().await, Some(0));
            assert_eq!(reading.len(), 0);
        });
    }
}

mod a_reader_let_go_of_while_its_poll_is_in_flight {
    use super::*;

    // The TS counts node's active timers, which Rust has no count of. The state it protects is that the poll the
    // reader comes back to starts no nap and no exec: the nap is counted through the factory's own `sleep` (a let-go
    // reader asks it for none), and the polls are counted on the guest over 100 ms after the held one is answered.
    #[test]
    fn starts_no_timer_for_the_poll_it_comes_back_to_so_nothing_it_held_outlives_the_cl() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![]);
            let in_flight = Arc::new(AtomicBool::new(false));
            let (answer, holding) = oneshot::channel::<()>();
            let holding = Arc::new(Mutex::new(Some(holding)));
            let (flying, parked) = (in_flight.clone(), holding.clone());
            guest.set_hook(hook(move |cmd, inner| {
                let waiting = if cmd.contains("__WSP_EOF_") { parked.lock().unwrap().take() } else { None };
                let flying = flying.clone();
                async move {
                    if let Some(rx) = waiting {
                        flying.store(true, SeqCst);
                        let _ = rx.await;
                    }
                    inner(&cmd)
                }
            }));
            let naps = Arc::new(AtomicUsize::new(0));
            let counted = naps.clone();
            let sleep = sleep_fn(move |ms| {
                counted.fetch_add(1, SeqCst);
                support::sleep_ms(ms).boxed()
            });
            let reading = ReadingSet::new();
            let stream = machine_exec_stream(
                machine,
                MachineExecOptions { poll_ms: Some(5_000), sleep: Some(sleep), reading: Some(reading.clone()), ..Default::default() },
                None,
            )
            .launch("claude -p hi", Launch::default());
            let reader = stream.clone();
            tokio::task::spawn_local(async move {
                let _ = support::collect(&*reader).await;
            });
            support::wait_for("a poll in flight", 1_000, || in_flight.load(SeqCst)).await;

            let before = guest.polls().len();
            reading.let_go_all();
            answer.send(()).unwrap();
            support::wait_for("the held poll to land", 1_000, || guest.polls().len() == before + 1).await;
            let landed = guest.polls().len();
            support::sleep_ms(100).await;
            assert_eq!(guest.polls().len(), landed, "a poll went out after the let-go");
            assert_eq!(naps.load(SeqCst), 0, "the poll it came back to started a nap");
            // And the run is left as it was, as a dropped reader always leaves it.
            assert_eq!(guest.kills(), Vec::<String>::new());
            assert!(guest.child_alive());
        });
    }
}
