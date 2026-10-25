// SPDX-License-Identifier: AGPL-3.0-only
//! local_exec_stream: the launch, the reader, the limits, the re-open, the input channel and the sweep of a turn on
//! this computer, ported from packages/runtime/test/local-exec.test.ts. Spec: spec/local_exec_stream.md.
#![allow(dead_code)]
mod support;

use std::cell::Cell;
use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::StreamExt;
use tokio::sync::oneshot;
use wsp_adapters::port::{end_run, AttachRunOptions, Attached, Env, ExecFailure, ExecOptions as Launch, ExecStream, ExecStreamFactory, Settle, WriteOutcome};
use wsp_core::local_exec::{group_exists, local_exec_stream, LocalExecFactory, LocalExecOptions, TreeWorkReader};
use wsp_core::machine_exec::{turn_cut, ReadingSet, TurnLimits};

use support::strays::{alive, gone, grandchild};
use support::{gate, gate_loop, sleep_ms, sq, wait_for, within};

/// `EXEC_CHUNK_BYTES` of packages/protocol/src/wire/limits.ts.
const EXEC_CHUNK_BYTES: usize = 262_144;
const WALL_WORDS: &str = "send to continue where it stopped, or change the limit in Settings > Computers";

type Lines = LocalBoxStream<'static, Result<String, ExecFailure>>;

/// One case's temp root, with the run folder under it. The guard ends the streams the case asked to end and every pid
/// it recorded, then the root goes with the guard.
struct Case {
    _dir: tempfile::TempDir,
    root: String,
    run_dir: String,
    ending: Mutex<Vec<Arc<dyn ExecStream>>>,
}

impl Case {
    fn new(prefix: &str) -> Self {
        let dir = tempfile::Builder::new().prefix(prefix).tempdir().expect("a temp root");
        let root = dir.path().to_str().expect("a utf-8 path").to_string();
        let run_dir = format!("{root}/runs");
        Case { _dir: dir, root, run_dir, ending: Mutex::new(Vec::new()) }
    }

    fn opts(&self) -> LocalExecOptions {
        LocalExecOptions::new(self.root.clone(), self.run_dir.clone())
    }

    fn factory(&self) -> Arc<LocalExecFactory> {
        local_exec_stream(self.opts(), None)
    }

    fn path(&self, name: &str) -> String {
        format!("{}/{name}", self.root)
    }

    /// Runs a case waits on for an end it may never reach when it fails: killed after it, so a red run leaves no child.
    fn ended(&self, stream: Arc<dyn ExecStream>) -> Arc<dyn ExecStream> {
        self.ending.lock().unwrap().push(stream.clone());
        stream
    }

    /// The TS `afterEach`'s sweep of the run folder with a fresh factory.
    async fn sweep(&self) {
        self.factory().sweep(&[]).expect("a sweep").await.expect("a sweep that answers");
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        for stream in self.ending.lock().unwrap().drain(..) {
            stream.kill();
        }
        support::strays::sweep();
    }
}

fn go(factory: &LocalExecFactory, command: &str) -> Arc<dyn ExecStream> {
    factory.launch(command, Launch::default())
}

fn with_input(lines: &[&str]) -> Launch {
    Launch { input: Some(lines.iter().map(|s| s.to_string()).collect()), ..Launch::default() }
}

fn env_of(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("the clock").as_millis() as i64
}

fn attach_opts(input: bool) -> AttachRunOptions {
    AttachRunOptions { input, started_at: now_ms() as f64, from: None }
}

/// What an attach answered, the failure as an `Err`.
async fn attach(factory: &LocalExecFactory, run: &str, options: AttachRunOptions) -> Result<Attached, ExecFailure> {
    factory.attach(run, options).expect("an attach").await
}

fn is_gone(attached: Result<Attached, ExecFailure>) -> bool {
    match attached {
        Ok(Attached::Gone) => true,
        Ok(Attached::Stream(_)) => false,
        Err(e) => panic!("the attach failed: {e}"),
    }
}

/// The stream an attach re-opened; the run being gone fails the case.
async fn attached_stream(factory: &LocalExecFactory, run: &str, options: AttachRunOptions) -> Arc<dyn ExecStream> {
    match attach(factory, run, options).await {
        Ok(Attached::Stream(s)) => s,
        Ok(Attached::Gone) => panic!("the run {run} was gone"),
        Err(e) => panic!("the attach failed: {e}"),
    }
}

/// A held promise that never settles.
fn pending() -> Settle {
    Box::pin(futures::future::pending::<Result<(), String>>())
}

/// A promise settled by `tx.send(())`.
fn settle_on(rx: oneshot::Receiver<()>) -> Settle {
    Box::pin(async move { rx.await.map_err(|_| "dropped".to_string()) })
}

fn resolved() -> Settle {
    Box::pin(async { Ok(()) })
}

fn rejected(words: &str) -> Settle {
    let words = words.to_string();
    Box::pin(async move { Err(words) })
}

/// The next line of a stream: `None` when it ends, a failure fails the case.
async fn next_line(it: &mut Lines) -> Option<String> {
    match it.next().await {
        Some(Ok(line)) => Some(line),
        Some(Err(e)) => panic!("the stream failed: {e}"),
        None => None,
    }
}

/// What is left of a stream already begun: its lines and the failure it ended on.
async fn drain(it: &mut Lines) -> (Vec<String>, Option<ExecFailure>) {
    let mut out = Vec::new();
    while let Some(next) = it.next().await {
        match next {
            Ok(line) => out.push(line),
            Err(e) => return (out, Some(e)),
        }
    }
    (out, None)
}

/// `Promise.all([collect(stream.lines), stream.exited])`.
async fn lines_and_code(stream: &Arc<dyn ExecStream>) -> (Vec<String>, Option<i32>) {
    tokio::join!(support::lines_ok(&**stream), stream.exited())
}

fn strs(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|s| s.to_string()).collect()
}

/// A TS promise runs from the call: this polls the work once, then lets a task carry it on.
async fn started<T: 'static>(mut work: LocalBoxFuture<'static, T>) -> LocalBoxFuture<'static, T> {
    match futures::poll!(&mut work) {
        std::task::Poll::Ready(v) => Box::pin(std::future::ready(v)),
        std::task::Poll::Pending => {
            let task = tokio::task::spawn_local(work);
            Box::pin(async move { task.await.expect("the started work") })
        }
    }
}

/// A tree burning one core, counted on the clock the idle rule compares its readings against. A real burner is no
/// use here: ps answers whole seconds of CPU on Linux, so a tree given a fraction of a loaded box shows nothing at
/// all until a whole second of its own has passed, and no idle limit is long enough to outrun that ratio.
fn burning_tree() -> TreeWorkReader {
    let from = now_ms();
    Arc::new(move |_pgid: i32| -> LocalBoxFuture<'static, Option<i64>> {
        let ticks = (now_ms() - from) / 10;
        Box::pin(async move { Some(ticks) })
    })
}

/// Starts a process as the leader of a group of its own with nothing of this process's attached, and reaps it as
/// node does, so a killed one is gone and not a zombie.
fn spawn_group(cmd: &mut Command) -> i32 {
    let child = cmd.process_group(0).spawn().expect("a spawn");
    let pid = child.id() as i32;
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
    pid
}

fn kill_group_hard(pid: i32) {
    assert!(pid > 1);
    // SAFETY: a negative pid names a group this test started.
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
}

fn owner_dir(path: &str) {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(path).expect(path);
}

fn is_idle_cut(message: &str) -> bool {
    regex::Regex::new(r"^stopped after \d+m \d\ds with no output for 0m$").unwrap().is_match(message)
}

mod local_exec_stream {
    use super::*;

    own_group_test!(streams_stdout_and_stderr_by_line_and_answers_with_the_exit_code, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = c.factory();
        let stream = go(&factory, "printf 'a\\nb\\n'; printf 'e\\n' 1>&2; exit 5");
        let (mut lines, code) = lines_and_code(&stream).await;
        lines.sort();
        assert_eq!(lines, strs(&["a", "b", "e"]));
        assert_eq!(code, Some(5));
        c.sweep().await;
    });

    own_group_test!(names_the_process_it_leads_which_is_the_group_every_process_of_the_turn_is_in, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = c.factory();
        // The shell prints its own pid and its group; both are the leader's, and the pid the stream reports is that one.
        let stream = go(&factory, "echo $$; ps -o pgid= -p $$");
        let (lines, _) = lines_and_code(&stream).await;
        let pid = stream.pid().expect("a pid");
        assert_eq!(lines[0].trim().parse::<u32>().expect("a pid line"), pid);
        assert_eq!(lines[1].trim().parse::<u32>().expect("a group line"), pid);
        c.sweep().await;
    });

    own_group_test!(runs_in_the_workspace_folder, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = c.factory();
        let stream = go(&factory, "pwd");
        let lines = support::lines_ok(&*stream).await;
        assert!(lines.join("").contains(c.root.trim_start_matches("/private")), "{lines:?} does not hold {}", c.root);
        assert_eq!(stream.exited().await, Some(0));
        c.sweep().await;
    });

    own_group_test!(an_input_channel_is_the_child_s_stdin_the_seed_lands_write_appends_close_input_e, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = c.factory();
        let stream = factory.launch(&format!("cat > {}", c.path("in.txt")), with_input(&["first"]));
        assert_eq!(stream.write("second".into()).await.unwrap(), WriteOutcome::Written);
        stream.close_input();
        assert_eq!(stream.exited().await, Some(0));
        assert_eq!(support::read(&c.path("in.txt")), "first\nsecond\n");
        c.sweep().await;
    });

    // A shell reader stands for the node child: the fact is that the end of the input channel is the end of the
    // child's stdin, and node's own reading of a pipe is not a thing this crate can show.
    own_group_test!(close_input_reaches_a_node_child_as_the_end_of_its_stdin_so_a_harness_exits_on_i, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = local_exec_stream(LocalExecOptions { poll_ms: Some(10), ..c.opts() }, None);
        let stream = factory.launch("perl -e '1 while <STDIN>; exit 7'", with_input(&["first"]));
        stream.close_input();
        let code = within(5_000, stream.exited()).await;
        if code.is_none() {
            stream.kill();
        }
        assert_eq!(code, Some(Some(7)));
        c.sweep().await;
    });

    own_group_test!(a_stream_started_without_an_input_channel_refuses_a_write, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = c.factory();
        let stream = go(&factory, "true");
        let refused = stream.write("x".into()).await.expect_err("a refused write");
        assert!(refused.message.contains("no input channel"), "{}", refused.message);
        stream.exited().await;
        c.sweep().await;
    });

    own_group_test!(a_child_that_never_writes_is_cut_at_the_idle_limit_with_the_same_line_a_cloud_tu, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = local_exec_stream(LocalExecOptions { idle_ms: Some(120.0), deadline_ms: Some(60_000.0), poll_ms: Some(10), ..c.opts() }, None);
        let stream = go(&factory, "sleep 30");
        let started = Instant::now();
        let (_, failure) = support::lines_err(&*stream).await;
        assert!(is_idle_cut(&failure.message), "{}", failure.message);
        assert!(started.elapsed() < Duration::from_millis(5_000));
        assert_eq!(stream.exited().await, None);
        let cut = turn_cut(TurnLimits { idle_ms: 120.0, deadline_ms: 60_000.0 }, 130, 120).expect("an idle cut");
        assert!(cut.message.ends_with("with no output for 0m"), "{}", cut.message);
        c.sweep().await;
    });

    own_group_test!(reads_an_exit_file_that_exists_and_is_empty_as_a_run_still_writing_and_answers_t, 15_000, {
        let c = Case::new("wsp-localexec-");
        // A group of this test's own, standing for the shell that is between the truncate of its exit file and the
        // write of the code; its pid is one this test started and wrote down, never one found by name.
        let pid = spawn_group(Command::new("bash").args(["-c", "sleep 300"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()));
        assert!(pid > 0);
        support::strays::record(pid);
        owner_dir(&c.run_dir);
        let base = format!("{}/bbbbbbbbbbbb", c.run_dir);
        owner_dir(&format!("{base}.d"));
        std::fs::write(format!("{base}.pid"), format!("{pid}\n")).unwrap();
        std::fs::write(format!("{base}.log"), "").unwrap();
        std::fs::write(format!("{base}.exit"), "").unwrap();
        let factory = local_exec_stream(LocalExecOptions { poll_ms: Some(10), ..c.opts() }, None);
        let stream = attached_stream(&factory, &base, attach_opts(false)).await;
        // Longer than the reap's 200 ms group poll, so a poll that settled on the empty file would be seen here.
        assert!(within(500, stream.exited()).await.is_none(), "the run settled on an empty exit file");
        std::fs::write(format!("{base}.exit"), "7\n").unwrap();
        assert_eq!(stream.exited().await, Some(7));
        // The reap the settle runs takes the group where the run settled; a red run leaves it to this.
        if alive(pid) {
            kill_group_hard(pid);
        }
        gone(pid).await;
    });

    own_group_test!(a_child_that_prints_nothing_while_the_tree_it_started_burns_a_core_is_not_cut_at, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = local_exec_stream(LocalExecOptions { idle_ms: Some(400.0), deadline_ms: Some(30_000.0), poll_ms: Some(20), read_work: Some(burning_tree()), ..c.opts() }, None);
        let stream = go(&factory, "sleep 2; echo still working; sleep 20");
        let mut it = stream.lines();
        // The line lands five idle limits into a silent turn, so only the tree's work can have held the turn open.
        let first = within(10_000, next_line(&mut it)).await;
        assert_eq!(first, Some(Some("still working".to_string())), "the turn printed nothing and was not cut");
        stream.kill();
        assert_ne!(stream.exited().await, Some(0));
        c.sweep().await;
    });

    own_group_test!(a_child_that_prints_nothing_while_the_tree_it_started_sleeps_is_cut_at_the_idle, 20_000, {
        let c = Case::new("wsp-localexec-");
        let marker = c.path("idle.pid");
        let factory = local_exec_stream(LocalExecOptions { idle_ms: Some(400.0), deadline_ms: Some(30_000.0), poll_ms: Some(20), ..c.opts() }, None);
        let stream = go(&factory, &format!("sleep 20 & echo $! > {marker}; sleep 20"));
        let sleeping = grandchild(&marker).await;
        let reader = stream.clone();
        let reading = tokio::task::spawn_local(async move { support::collect(&*reader).await });
        // The cut takes the group, so the grandchild is gone before the cut's own words reach the reader.
        gone(sleeping).await;
        let (_, failure) = reading.await.expect("the reader");
        assert!(is_idle_cut(&failure.expect("a cut").message));
        assert_eq!(stream.exited().await, None);
        c.sweep().await;
    });

    own_group_test!(a_child_stopped_on_a_question_only_a_person_can_answer_is_not_cut_at_the_idle_li, 10_000, {
        let c = Case::new("wsp-localexec-");
        let waiting = Arc::new(AtomicBool::new(true));
        let question = waiting.clone();
        let factory = local_exec_stream(
            LocalExecOptions { idle_ms: Some(120.0), deadline_ms: Some(60_000.0), poll_ms: Some(10), ..c.opts() },
            Some(Arc::new(move || question.load(Ordering::SeqCst))),
        );
        let stream = go(&factory, "sleep 1.2; echo answered; sleep 20");
        let mut it = stream.lines();
        // The silence runs many idle limits long; only the question held the turn open.
        let first = within(4_000, next_line(&mut it)).await;
        assert_eq!(first, Some(Some("answered".to_string())), "the turn was cut while it waited");
        waiting.store(false, Ordering::SeqCst);
        let (_, failure) = drain(&mut it).await;
        assert!(failure.expect("a cut").message.ends_with("with no output for 0m"));
        assert_eq!(stream.exited().await, None);
        c.sweep().await;
    });

    own_group_test!(a_child_that_keeps_writing_past_the_wall_is_cut_at_the_cap, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = local_exec_stream(LocalExecOptions { idle_ms: Some(60_000.0), deadline_ms: Some(150.0), poll_ms: Some(10), ..c.opts() }, None);
        let stream = go(&factory, &gate_loop(&c.path("stop"), "echo tick", 30));
        let (_, failure) = support::lines_err(&*stream).await;
        assert!(failure.message.ends_with(&format!("at the 0m turn limit; {WALL_WORDS}")), "{}", failure.message);
        assert_eq!(stream.exited().await, None);
        c.sweep().await;
    });

    own_group_test!(a_turn_re_opened_after_a_restart_is_cut_at_its_first_start_plus_the_wall_not_at, 15_000, {
        let c = Case::new("wsp-localexec-");
        const WALL_MS: u64 = 3_000;
        let reading = ReadingSet::new();
        let started_at = now_ms();
        let launched = local_exec_stream(LocalExecOptions { idle_ms: Some(60_000.0), deadline_ms: Some(WALL_MS as f64), poll_ms: Some(20), reading: Some(reading.clone()), ..c.opts() }, None)
            .launch(&gate_loop(&c.path("stop"), "echo tick", 30), Launch::default());
        sleep_ms(WALL_MS / 2).await;
        // The host goes: its reader lets go of the run, which keeps running, and the next host re-opens it by handle.
        reading.let_go_all();
        let attached_at = Instant::now();
        let next = local_exec_stream(LocalExecOptions { idle_ms: Some(60_000.0), deadline_ms: Some(WALL_MS as f64), poll_ms: Some(20), ..c.opts() }, None);
        let attached = attached_stream(&next, &launched.run().expect("a run"), AttachRunOptions { input: false, started_at: started_at as f64, from: None }).await;
        let (_, failure) = support::lines_err(&*attached).await;
        assert!(failure.message.ends_with(&format!("turn limit; {WALL_WORDS}")), "{}", failure.message);
        // Half the wall was left when it was re-opened, and that half is all it got.
        assert!(attached_at.elapsed() < Duration::from_millis((WALL_MS as f64 * 0.9) as u64), "the re-opened turn ran {:?}", attached_at.elapsed());
        assert_eq!(attached.exited().await, None);
        c.sweep().await;
    });

    own_group_test!(a_run_that_finished_while_no_host_read_it_keeps_its_reply_when_it_is_re_opened_p, 15_000, {
        let c = Case::new("wsp-localexec-");
        const WALL_MS: u64 = 1_000;
        let reading = ReadingSet::new();
        let started_at = now_ms();
        let launched = local_exec_stream(LocalExecOptions { idle_ms: Some(60_000.0), deadline_ms: Some(WALL_MS as f64), poll_ms: Some(20), reading: Some(reading.clone()), ..c.opts() }, None)
            .launch("sleep 0.3; echo the-reply", Launch::default());
        reading.let_go_all();
        sleep_ms(WALL_MS * 12 / 10).await;
        let next = local_exec_stream(LocalExecOptions { idle_ms: Some(60_000.0), deadline_ms: Some(WALL_MS as f64), poll_ms: Some(20), ..c.opts() }, None);
        let attached = attached_stream(&next, &launched.run().expect("a run"), AttachRunOptions { input: false, started_at: started_at as f64, from: None }).await;
        assert_eq!(support::lines_ok(&*attached).await, strs(&["the-reply"]));
        assert_eq!(attached.exited().await, Some(0));
        c.sweep().await;
    });

    own_group_test!(a_run_still_going_when_it_is_re_opened_past_its_wall_hands_over_what_it_printed, 15_000, {
        let c = Case::new("wsp-localexec-");
        const WALL_MS: u64 = 1_000;
        let reading = ReadingSet::new();
        let started_at = now_ms();
        let launched = local_exec_stream(LocalExecOptions { idle_ms: Some(60_000.0), deadline_ms: Some(WALL_MS as f64), poll_ms: Some(20), reading: Some(reading.clone()), ..c.opts() }, None)
            .launch("echo while-away; sleep 30", Launch::default());
        reading.let_go_all();
        sleep_ms(WALL_MS * 12 / 10).await;
        let next = local_exec_stream(LocalExecOptions { idle_ms: Some(60_000.0), deadline_ms: Some(WALL_MS as f64), poll_ms: Some(20), ..c.opts() }, None);
        let attached = attached_stream(&next, &launched.run().expect("a run"), AttachRunOptions { input: false, started_at: started_at as f64, from: None }).await;
        let (seen, failure) = support::lines_err(&*attached).await;
        assert!(failure.message.ends_with(&format!("turn limit; {WALL_WORDS}")), "{}", failure.message);
        assert_eq!(seen, strs(&["while-away"]));
        assert_eq!(attached.exited().await, None);
        c.sweep().await;
    });

    own_group_test!(the_defaults_are_the_turn_s_own_limits_so_a_quick_command_is_never_cut, 15_000, {
        let c = Case::new("wsp-localexec-");
        let stream = go(&c.factory(), "printf ok");
        assert_eq!(support::lines_ok(&*stream).await, strs(&["ok"]));
        assert_eq!(stream.exited().await, Some(0));
        c.sweep().await;
    });

    own_group_test!(a_run_that_ignores_term_keeps_what_it_printed_after_the_last_poll_past_one_chunk, 15_000, {
        let c = Case::new("wsp-localexec-");
        // A poll slower than the grace, so no read falls between the TERM and the KILL: the lines are read at the end or never.
        let n = EXEC_CHUNK_BYTES + 1000;
        let command = format!(r#"trap 'head -c {n} /dev/zero | tr "\0" x; echo; echo last words' TERM; echo ready; {}"#, gate(&c.path("stop")));
        let stream = go(&local_exec_stream(LocalExecOptions { poll_ms: Some(1_500), ..c.opts() }, None), &command);
        let mut it = stream.lines();
        assert_eq!(next_line(&mut it).await.as_deref(), Some("ready"));
        sleep_ms(100).await;
        end_run(&*stream, 300).await;
        let (rest, failure) = drain(&mut it).await;
        assert!(failure.is_none(), "{failure:?}");
        assert!(rest.contains(&"x".repeat(n)), "the line past one chunk is not among {} lines", rest.len());
        assert_eq!(rest.last().map(String::as_str), Some("last words"));
        c.sweep().await;
    });

    own_group_test!(kill_ends_the_child, 15_000, {
        let c = Case::new("wsp-localexec-");
        let factory = c.factory();
        let stream = go(&factory, "sleep 10");
        stream.kill();
        assert_ne!(stream.exited().await, Some(0));
        c.sweep().await;
    });
}

mod what_a_run_leaves_on_this_computer {
    use super::*;

    own_group_test!(every_file_a_turn_s_launch_writes_is_owner_only_from_its_first_byte_and_so_is_th, 20_000, {
        let c = Case::new("wsp-localexec-files-");
        let factory = c.factory();
        let stream = factory.launch(
            &format!("sleep 300 & echo $! > {}; sleep 300", c.path("child")),
            Launch { env: env_of(&[("ANTHROPIC_API_KEY", "sk-ant-x-not-a-key")]), input: Some(strs(&["hello"])), ..Launch::default() },
        );
        let child = grandchild(&c.path("child")).await;
        let base = stream.run().expect("a run");

        assert_eq!(support::mode_of(&c.run_dir), "700");
        assert_eq!(support::mode_of(&format!("{base}.d")), "700");
        // The script carries the command; the input channel carries what a person sends into the turn; the log carries
        // everything the agent prints.
        for suffix in ["sh", "in", "log"] {
            assert_eq!((suffix, support::mode_of(&format!("{base}.{suffix}"))), (suffix, "600".to_string()));
        }

        // No file holding anything of the person's is open to another login: the run's own bookkeeping (a pid, a tail's
        // pid, an exit code) is written by the shell at its own umask, and the folder at 700 is what keeps that out of
        // anyone else's reach.
        let carries = ["sh", "in", "log"];
        let readable: Vec<String> = support::names(&c.run_dir)
            .into_iter()
            .filter(|name| carries.contains(&name.rsplit('.').next().unwrap()) && (std::fs::metadata(format!("{}/{name}", c.run_dir)).unwrap().permissions().mode() & 0o077) != 0)
            .collect();
        assert_eq!(readable, Vec::<String>::new());

        stream.kill();
        stream.exited().await;
        gone(child).await;
        c.sweep().await;
    });

    own_group_test!(a_turn_and_an_exec_launched_with_a_key_reach_it_in_their_environment_and_no_file, 20_000, {
        let c = Case::new("wsp-localexec-files-");
        let key = format!("sk-ant-x-{}", hex::encode(rand::random::<[u8; 8]>()));
        let gate_file = c.path("gate");
        let factory = local_exec_stream(LocalExecOptions { poll_ms: Some(10), ..c.opts() }, None);
        let seen = r#"case "$ANTHROPIC_API_KEY" in sk-ant-x-*) echo "seen ${#ANTHROPIC_API_KEY}";; esac"#;
        // A turn as an agent's launch makes one, its prompt on an input channel; an exec as the exec verb makes one.
        let turn = factory.launch(
            &format!(r#"{seen}; read -r line; echo "got $line""#),
            Launch { env: env_of(&[("ANTHROPIC_API_KEY", key.as_str())]), input: Some(strs(&["hello"])), input_after: Some(pending()), ..Launch::default() },
        );
        let exec = factory.launch(&format!("{seen}; {}", gate(&gate_file)), Launch { env: env_of(&[("ANTHROPIC_API_KEY", key.as_str())]), ..Launch::default() });
        let mut turn_lines = turn.lines();
        let mut exec_lines = exec.lines();
        assert_eq!(next_line(&mut turn_lines).await, Some(format!("seen {}", key.len())));
        assert_eq!(next_line(&mut exec_lines).await, Some(format!("seen {}", key.len())));

        // Both are running, each held on its own gate, with every file of theirs on disk.
        assert_eq!(support::holding(&c.run_dir, &key), Vec::<String>::new());

        std::fs::write(&gate_file, "").unwrap();
        assert_eq!(next_line(&mut exec_lines).await, None);
        turn.kill();
        assert_eq!((exec.exited().await, turn.exited().await), (Some(0), None));
        assert_eq!(support::names(&c.run_dir), Vec::<String>::new());
        turn.kill();
        exec.kill();
        tokio::join!(turn.exited(), exec.exited());
        c.sweep().await;
    });

    own_group_test!(a_script_an_older_host_wrote_with_the_key_in_it_is_gone_once_its_run_ends_read_o, 20_000, {
        let c = Case::new("wsp-localexec-files-");
        let key = format!("sk-ant-x-{}", hex::encode(rand::random::<[u8; 8]>()));
        let gate_file = c.path("gate");
        owner_dir(&c.run_dir);
        // A run as a host before this one launched it: the environment as export lines at the top of its script.
        let mut started: Vec<i32> = Vec::new();
        let mut older = |id: &str, command: &str| -> String {
            let base = format!("{}/{id}", c.run_dir);
            owner_dir(&format!("{base}.d"));
            let mut script = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(format!("{base}.sh")).unwrap();
            write!(script, "export ANTHROPIC_API_KEY={}\n( {command}\n)\necho $? > {}\n", sq(&key), sq(&format!("{base}.exit"))).unwrap();
            drop(script);
            let log = std::fs::OpenOptions::new().append(true).create(true).mode(0o600).open(format!("{base}.log")).unwrap();
            let err = log.try_clone().unwrap();
            let pid = spawn_group(Command::new("bash").arg(format!("{base}.sh")).current_dir(&c.root).stdin(Stdio::null()).stdout(log).stderr(err));
            started.push(pid);
            std::fs::write(format!("{base}.pid"), format!("{pid}\n")).unwrap();
            base
        };
        let read = older("aaaaaaaaaaaa", &format!("{}; echo done", gate(&gate_file)));
        let ended = older("bbbbbbbbbbbb", "echo done");
        wait_for("the finished run's code", 5_000, || support::read(&format!("{ended}.exit")).trim() == "0").await;
        assert_eq!(support::holding(&c.run_dir, &key), vec![format!("{read}.sh"), format!("{ended}.sh")]);

        // The host that comes next re-opens the run its rows hold and sweeps the one they do not.
        let next = local_exec_stream(LocalExecOptions { poll_ms: Some(10), ..c.opts() }, None);
        let attached = attached_stream(&next, &read, attach_opts(false)).await;
        assert_eq!(next.sweep(&[read.clone()]).expect("a sweep").await.unwrap(), vec![ended.clone()]);
        std::fs::write(&gate_file, "").unwrap();
        assert_eq!(lines_and_code(&attached).await, (strs(&["done"]), Some(0)));
        assert_eq!(support::names(&c.run_dir), Vec::<String>::new());
        for pid in &started {
            if group_exists(*pid) {
                kill_group_hard(*pid);
            }
        }
        c.sweep().await;
    });
}

mod a_run_kept_between_turns {
    use super::*;

    own_group_test!(re_opened_mid_turn_spares_only_its_input_pump_at_that_turn_s_end_what_the_turn_l, 20_000, {
        let c = Case::new("wsp-localexec-kept-");
        let gate_file = c.path("gate");
        let child = c.path("child");
        let holder = c.path("holder");
        // "leave" starts a holder keeping a sleep under it until the gate opens: no orphan when the next host re-opens the
        // run, and one by the time the turn ends.
        let hold = format!("sleep 300 > /dev/null 2>&1 & echo $! > {}; echo $$ > {}; {}", sq(&child), sq(&holder), gate(&gate_file));
        let agent = format!(r#"while IFS= read -r l; do if [ "$l" = leave ]; then sh -c {} & echo left; else echo "said $l"; fi; done"#, sq(&hold));
        let reading = ReadingSet::new();
        let first = c.ended(local_exec_stream(LocalExecOptions { poll_ms: Some(20), reading: Some(reading.clone()), ..c.opts() }, None).launch(&agent, with_input(&["leave"])));
        assert_eq!(next_line(&mut first.lines()).await.as_deref(), Some("left"));
        let sleeper = grandchild(&child).await;
        let held = grandchild(&holder).await;
        reading.let_go_all();
        let run = first.run().expect("a run");
        let next = local_exec_stream(LocalExecOptions { poll_ms: Some(20), ..c.opts() }, None);
        let from = std::fs::metadata(format!("{run}.log")).unwrap().len();
        let reopened = c.ended(attached_stream(&next, &run, AttachRunOptions { input: true, started_at: now_ms() as f64, from: Some(from) }).await);
        std::fs::write(&gate_file, "go\n").unwrap();
        wait_for("the sleeper to leave the holder", 5_000, || support::strays::ppid_of(sleeper) != held).await;
        let mut lines = reopened.lines();
        assert_eq!(reopened.write("done".into()).await.unwrap(), WriteOutcome::Written);
        assert_eq!(next_line(&mut lines).await.as_deref(), Some("said done"));
        reopened.rest().expect("a rest");
        gone(sleeper).await;
        assert_eq!(reopened.write("again".into()).await.unwrap(), WriteOutcome::Written);
        assert_eq!(next_line(&mut lines).await.as_deref(), Some("said again"));
    });
}

mod a_real_turn_s_process_group {
    use super::*;

    /// The pid the command wrote for what it left running, once it has.
    async fn left_running(c: &Case) -> i32 {
        grandchild(&c.path("child")).await
    }

    own_group_test!(a_child_the_command_left_running_is_gone_when_the_turn_ends_and_the_stream_ends, 15_000, {
        let c = Case::new("wsp-localexec-group-");
        let factory = c.factory();
        let stream = go(&factory, &format!("sleep 300 & echo $! > {}; echo hi", c.path("child")));
        // Read before the assertion that hangs while the bug is there, so a red run still records what to clean up.
        let pid = left_running(&c).await;
        assert_eq!(support::lines_ok(&*stream).await, strs(&["hi"]));
        assert_eq!(stream.exited().await, Some(0));
        gone(pid).await;
    });

    own_group_test!(a_turn_the_idle_limit_cut_takes_its_whole_group_with_it_and_still_ends_on_the_cu, 15_000, {
        let c = Case::new("wsp-localexec-group-");
        let factory = local_exec_stream(LocalExecOptions { idle_ms: Some(120.0), deadline_ms: Some(60_000.0), poll_ms: Some(10), ..c.opts() }, None);
        let stream = go(&factory, &format!("sleep 300 & echo $! > {}; sleep 300", c.path("child")));
        let pid = left_running(&c).await;
        let (_, failure) = support::lines_err(&*stream).await;
        assert!(failure.message.ends_with("with no output for 0m"), "{}", failure.message);
        assert_eq!(stream.exited().await, None);
        gone(pid).await;
    });

    own_group_test!(a_factory_that_never_launched_a_run_attaches_to_it_by_handle_and_reads_its_whole, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let gate_file = c.path("gate");
        let factory = c.factory();
        let launched = go(&factory, &format!("echo first; {}; echo second; sleep 30", gate(&gate_file)));
        let run = launched.run().expect("a run");
        assert!(run.starts_with(&format!("{}/", c.run_dir)));
        // The run has printed its first line to the log on disk before anything attaches to it.
        assert_eq!(next_line(&mut launched.lines()).await.as_deref(), Some("first"));

        // A second factory, standing for the host that comes next: it never launched this run and re-opens it by handle.
        let next = c.factory();
        let attached = attached_stream(&next, &run, attach_opts(false)).await;
        let mut reader = attached.lines();
        // The line printed before this reader existed reaches it, which is what reading the log from byte zero is for.
        assert_eq!(next_line(&mut reader).await.as_deref(), Some("first"));
        std::fs::write(&gate_file, "go\n").unwrap();
        assert_eq!(next_line(&mut reader).await.as_deref(), Some("second"));
        launched.kill();
        launched.exited().await;
    });

    own_group_test!(an_attach_to_a_run_whose_input_channel_was_never_written_ends_it_and_answers_gon, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let reading = ReadingSet::new();
        let factory = local_exec_stream(LocalExecOptions { reading: Some(reading.clone()), ..c.opts() }, None);
        let unprompted = factory.launch(&format!("sleep 300 & echo $! > {}; cat > /dev/null", c.path("child")), with_input(&[]));
        let pid = grandchild(&c.path("child")).await;
        let prompted = factory.launch("cat > /dev/null", with_input(&["hello"]));
        // The host that launched them goes: a reader of its own still polling would reap the run under the attach.
        reading.let_go_all();
        let next = local_exec_stream(LocalExecOptions { poll_ms: Some(500), ..c.opts() }, None);
        assert!(is_gone(attach(&next, &unprompted.run().unwrap(), attach_opts(true)).await));
        gone(pid).await;
        assert!(!std::path::Path::new(&format!("{}.d", unprompted.run().unwrap())).exists());
        let attached = attached_stream(&next, &prompted.run().unwrap(), attach_opts(true)).await;
        attached.close_input();
        assert_eq!(attached.exited().await, Some(0));
    });

    own_group_test!(a_held_seed_reaches_the_child_only_once_it_is_due_and_once, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let (due, rx) = oneshot::channel::<()>();
        let factory = c.factory();
        let stream = c.ended(factory.launch("cat", Launch { input: Some(strs(&["hello"])), input_after: Some(settle_on(rx)), ..Launch::default() }));
        let mut reader = stream.lines();
        sleep_ms(100).await;
        assert_eq!(support::read(&format!("{}.in", stream.run().unwrap())), "");
        due.send(()).unwrap();
        assert_eq!(next_line(&mut reader).await.as_deref(), Some("hello"));
        stream.close_input();
        assert_eq!(stream.exited().await, Some(0));
    });

    own_group_test!(a_line_written_or_an_end_asked_for_before_a_held_seed_is_due_goes_after_the_seed, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let factory = c.factory();
        let held = || Launch { input: Some(strs(&["first"])), input_after: Some(pending()), ..Launch::default() };
        let written = c.ended(factory.launch("cat", held()));
        assert_eq!(written.write("second".into()).await.unwrap(), WriteOutcome::Written);
        written.close_input();
        let closed = c.ended(factory.launch("cat", held()));
        closed.close_input();
        for (stream, lines) in [(written, strs(&["first", "second"])), (closed, strs(&["first"]))] {
            assert_eq!(lines_and_code(&stream).await, (lines, Some(0)));
        }
    });

    own_group_test!(a_line_held_after_the_seed_reaches_the_child_once_it_is_due_and_once_and_one_who, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let factory = c.factory();
        let (due, rx) = oneshot::channel::<()>();
        let kept = c.ended(factory.launch("cat", with_input(&["seed"])));
        kept.write_after("late".into(), settle_on(rx)).expect("a held line");
        let dropped = c.ended(factory.launch("cat", with_input(&["seed"])));
        dropped.write_after("late".into(), rejected("stopped")).expect("a held line");
        sleep_ms(100).await;
        let kept_in = format!("{}.in", kept.run().unwrap());
        assert_eq!(support::read(&kept_in), "seed\n");
        due.send(()).unwrap();
        wait_for("the late line in the channel", 5_000, || support::read(&kept_in) == "seed\nlate\n").await;
        assert!(!std::path::Path::new(&format!("{}.late", dropped.run().unwrap())).exists());
        kept.close_input();
        dropped.close_input();
        let both = tokio::join!(support::lines_ok(&*kept), support::lines_ok(&*dropped));
        assert_eq!(both, (strs(&["seed", "late"]), strs(&["seed"])));
    });

    own_group_test!(a_line_held_after_the_seed_with_the_same_words_as_one_the_child_already_read_rea, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let stream = c.ended(go_input(&c, "cat", &["seed"]));
        let input = format!("{}.in", stream.run().unwrap());
        stream.write_after("yes".into(), resolved()).expect("a held line");
        wait_for("the first yes in the channel", 5_000, || support::read(&input) == "seed\nyes\n").await;
        let (due, due_rx) = oneshot::channel::<()>();
        stream.write_after("yes".into(), settle_on(due_rx)).expect("a held line");
        let steered = started(stream.write("steer".into())).await;
        let (second, second_rx) = oneshot::channel::<()>();
        stream.write_after("again".into(), settle_on(second_rx)).expect("a held line");
        second.send(()).unwrap();
        sleep_ms(100).await;
        assert_eq!(support::read(&input), "seed\nyes\n");
        due.send(()).unwrap();
        assert_eq!(steered.await.unwrap(), WriteOutcome::Written);
        wait_for("six pieces in the channel", 5_000, || support::read(&input).split('\n').count() == 6).await;
        stream.close_input();
        assert_eq!(support::lines_ok(&*stream).await, strs(&["seed", "yes", "yes", "again", "steer"]));
    });

    own_group_test!(an_attach_hands_over_a_late_line_held_beside_the_run_and_one_the_channel_already, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let reading = ReadingSet::new();
        let factory = local_exec_stream(LocalExecOptions { reading: Some(reading.clone()), ..c.opts() }, None);
        let held = c.ended(factory.launch("cat", with_input(&["seed"])));
        held.write_after("late".into(), pending()).expect("a held line");
        let reached = c.ended(factory.launch("cat", with_input(&["seed"])));
        reached.write_after("late".into(), resolved()).expect("a held line");
        let reached_run = reached.run().unwrap();
        wait_for("the late line in the channel", 5_000, || support::read(&format!("{reached_run}.in")) == "seed\nlate\n").await;
        // A host that went between handing the line over and dropping it: the line is in the channel and still held.
        std::fs::write(format!("{reached_run}.late.key"), support::read(&format!("{reached_run}.late.went"))).unwrap();
        std::fs::write(format!("{reached_run}.late"), "late\n").unwrap();
        reading.let_go_all();
        let next = local_exec_stream(LocalExecOptions { poll_ms: Some(50), ..c.opts() }, None);
        for run in [held.run().unwrap(), reached_run.clone()] {
            let attached = attached_stream(&next, &run, attach_opts(true)).await;
            assert_eq!(attached.taken(), Some(strs(&["seed", "late"])));
            c.ended(attached.clone()).close_input();
            assert_eq!(lines_and_code(&attached).await, (strs(&["seed", "late"]), Some(0)));
        }
    });

    own_group_test!(a_held_seed_whose_time_never_comes_kills_the_run_and_the_child_never_reads_it, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let stream = c.ended(
            local_exec_stream(c.opts(), None)
                .launch(&format!("cat > {}", c.path("read")), Launch { input: Some(strs(&["hello"])), input_after: Some(rejected("no snapshot")), ..Launch::default() }),
        );
        assert_eq!(stream.exited().await, None);
        assert!(!std::path::Path::new(&format!("{}.d", stream.run().unwrap())).exists());
        assert_eq!(support::read(&c.path("read")), "");
    });

    own_group_test!(an_attach_to_a_run_whose_held_seed_never_came_due_hands_the_seed_over_and_one_wh, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let reading = ReadingSet::new();
        let factory = local_exec_stream(LocalExecOptions { reading: Some(reading.clone()), ..c.opts() }, None);
        let held = c.ended(factory.launch("cat", Launch { input: Some(strs(&["hello"])), input_after: Some(pending()), ..Launch::default() }));
        let reached = c.ended(factory.launch("cat", Launch { input: Some(strs(&["hello"])), input_after: Some(resolved()), ..Launch::default() }));
        let reached_run = reached.run().unwrap();
        wait_for("the seed in the channel", 5_000, || !support::read(&format!("{reached_run}.in")).is_empty()).await;
        // A host that went between handing the seed over and dropping it: the seed is in the channel and still held.
        std::fs::write(format!("{reached_run}.held"), "hello\n").unwrap();
        reading.let_go_all();
        assert_eq!(support::read(&format!("{}.in", held.run().unwrap())), "");
        let next = local_exec_stream(LocalExecOptions { poll_ms: Some(50), ..c.opts() }, None);
        for run in [held.run().unwrap(), reached_run.clone()] {
            let attached = attached_stream(&next, &run, attach_opts(true)).await;
            c.ended(attached.clone()).close_input();
            let (lines, code) = lines_and_code(&attached).await;
            assert_eq!((lines, code), (strs(&["hello"]), Some(0)));
        }
    });

    own_group_test!(an_attach_reading_a_long_log_from_the_first_byte_lets_the_loop_turn_between_its, 90_000, {
        // A host re-opening a turn reads its whole log, and read in one go 200 MB held the loop 3.7 s (measured
        // 2026-09-28), which is a host answering nothing to every window and every line at a terminal.
        let c = Case::new("wsp-localexec-group-");
        let bytes: u64 = 200_000_000;
        let factory = c.factory();
        let launched = go(&factory, &format!("head -c {bytes} /dev/zero | tr '\\0' x | fold -w 999; sleep 30"));
        let run = launched.run().expect("a run");
        let log = format!("{run}.log");
        wait_for("the whole log", 60_000, || std::fs::metadata(&log).map(|m| m.len() > bytes).unwrap_or(false)).await;
        let next = c.factory();
        let attached = attached_stream(&next, &run, attach_opts(false)).await;
        let mut it = attached.lines();
        let mut last = Instant::now();
        let mut held = Duration::ZERO;
        let mut read = 0;
        while let Some(line) = it.next().await {
            line.expect("a line");
            held = held.max(last.elapsed());
            last = Instant::now();
            read += 1;
            if read == 200_000 {
                break;
            }
        }
        held = held.max(last.elapsed());
        assert_eq!(read, 200_000);
        assert!(held < Duration::from_millis(500), "the longest gap between two lines was {held:?}");
        launched.kill();
        launched.exited().await;
        c.sweep().await;
    });

    own_group_test!(a_stop_that_lands_while_an_attach_replays_a_long_log_lets_the_loop_turn_between, 90_000, {
        let c = Case::new("wsp-localexec-group-");
        let bytes: u64 = 200_000_000;
        let reading = ReadingSet::new();
        let launched = local_exec_stream(LocalExecOptions { reading: Some(reading.clone()), ..c.opts() }, None)
            .launch(&format!("head -c {bytes} /dev/zero | tr '\\0' x | fold -w 999; sleep 30"), Launch::default());
        reading.let_go_all();
        let run = launched.run().expect("a run");
        // The whole log, fold's newlines included: the kill ends the writer, so a log still growing is read short.
        let whole = bytes + bytes / 999;
        let log = format!("{run}.log");
        wait_for("the whole log", 60_000, || std::fs::metadata(&log).map(|m| m.len() == whole).unwrap_or(false)).await;
        let attached = attached_stream(&c.factory(), &run, attach_opts(false)).await;
        // The poll has read its first chunk; the other 199 MB are the stop's to read.
        attached.kill();
        // Lines handed over between two turns of the loop: a counter rather than a clock, so a loaded computer moves
        // nothing. The ticker and the reader are polled by one task, so a turn of the loop is a poll of that task: a
        // read that held it would hand over every line in one poll.
        let since_turn = Cell::new(0usize);
        let turning = Cell::new(true);
        let ticker = async {
            while turning.get() {
                since_turn.set(0);
                tokio::task::yield_now().await;
            }
        };
        let mut it = attached.lines();
        let reader = async {
            let mut most_in_one_turn = 0usize;
            let mut read = 0usize;
            while let Some(line) = it.next().await {
                if line.expect("a line").starts_with('x') {
                    read += 1;
                }
                since_turn.set(since_turn.get() + 1);
                most_in_one_turn = most_in_one_turn.max(since_turn.get());
            }
            turning.set(false);
            (read, most_in_one_turn)
        };
        let ((), (read, most_in_one_turn)) = futures::join!(ticker, reader);
        assert_eq!(read as u64, (bytes + 998) / 999);
        // A chunk's lines and what the turn before it left: a read that held the loop hands over every line in one turn.
        assert!(most_in_one_turn <= (3 * EXEC_CHUNK_BYTES + 999) / 1000, "{most_in_one_turn} lines in one turn");
        assert_eq!(attached.exited().await, None);
        c.sweep().await;
    });

    own_group_test!(a_character_split_across_two_chunks_of_the_log_comes_back_whole, 15_000, {
        let c = Case::new("wsp-localexec-group-");
        let reading = ReadingSet::new();
        let launched = local_exec_stream(LocalExecOptions { reading: Some(reading.clone()), ..c.opts() }, None)
            .launch(&format!("head -c {} /dev/zero | tr '\\0' x; printf '\\303\\251\\n'", EXEC_CHUNK_BYTES - 1), Launch::default());
        reading.let_go_all();
        let run = launched.run().expect("a run");
        // Read from its first byte once it is whole, so the first chunk ends inside the é.
        wait_for("the exit code", 10_000, || support::read(&format!("{run}.exit")).trim() == "0").await;
        let attached = attached_stream(&c.factory(), &run, attach_opts(false)).await;
        let lines = support::lines_ok(&*attached).await;
        assert_eq!(lines, vec![format!("{}é", "x".repeat(EXEC_CHUNK_BYTES - 1))]);
        c.sweep().await;
    });

    own_group_test!(a_stop_reads_the_log_to_the_size_it_had_at_the_kill_and_ends_while_a_writer_outs, 30_000, {
        let c = Case::new("wsp-localexec-group-");
        let marker = c.path("writer");
        let command = format!(
            "{}setsid sh -c 'end=$(($(date +%s) + 60)); while [ $(date +%s) -lt $end ]; do head -c 65536 /dev/zero | tr \"\\0\" y; echo; sleep 0.03; done' & echo $! > {}; echo ready; sleep 30",
            support::linux_shell_prelude(),
            sq(&marker),
        );
        let stream = go(&c.factory(), &command);
        let reader = stream.clone();
        let reading = tokio::task::spawn_local(async move { support::collect(&*reader).await });
        let writer = grandchild(&marker).await;
        sleep_ms(1_000).await;
        stream.kill();
        reading.await.expect("the reader");
        stream.exited().await;
        // The stop ended while the writer still prints: a read to the end of the log ends only once the writer does.
        let still_there = group_exists(writer);
        if still_there {
            kill_group_hard(writer);
        }
        assert!(still_there);
    });

    own_group_test!(an_attach_answers_gone_for_a_run_this_computer_no_longer_holds_and_refuses_a_han, 15_000, {
        let c = Case::new("wsp-localexec-group-");
        let factory = c.factory();
        let stream = go(&factory, "echo hi");
        let run = stream.run().expect("a run");
        assert_eq!(support::lines_ok(&*stream).await, strs(&["hi"]));
        assert_eq!(stream.exited().await, Some(0));
        // The reap took the claim with the rest of the run when the stream ended.
        assert!(is_gone(attach(&factory, &run, attach_opts(false)).await));
        // node's join folds the ../ away: the handle is one folder up from the run folder.
        for handle in [format!("{}/elsewhere", c.root), format!("{}/not-a-run-id", c.run_dir)] {
            match attach(&factory, &handle, attach_opts(false)).await {
                Err(e) => assert!(e.message.contains("not a run this host could have launched"), "{}", e.message),
                Ok(_) => panic!("{handle} attached"),
            }
        }
    });

    own_group_test!(the_sweep_ends_every_run_this_computer_holds_that_the_caller_did_not_name_and_le, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let factory = c.factory();
        let kept = go(&factory, &format!("sleep 300 & echo $! > {}; sleep 300", c.path("child")));
        let kept_pid = left_running(&c).await;
        let swept = go(&factory, &format!("sleep 300 & echo $! > {}; sleep 300", c.path("orphan")));
        let swept_pid = grandchild(&c.path("orphan")).await;

        let ended = factory.sweep(&[kept.run().unwrap()]).expect("a sweep").await.unwrap();

        assert_eq!(ended, vec![swept.run().unwrap()]);
        gone(swept_pid).await;
        assert!(alive(kept_pid));
        // The run that was named is still there to attach to; the one the sweep took is gone.
        assert!(is_gone(attach(&factory, &swept.run().unwrap(), attach_opts(false)).await));
        assert!(!is_gone(attach(&factory, &kept.run().unwrap(), attach_opts(false)).await));
        kept.kill();
        kept.exited().await;
        gone(kept_pid).await;
    });

    own_group_test!(lets_go_of_a_run_it_is_reading_when_the_wiring_that_made_it_closes_the_poll_stop, 20_000, {
        let c = Case::new("wsp-localexec-group-");
        let gate_file = c.path("gate");
        let reading = ReadingSet::new();
        let factory = local_exec_stream(LocalExecOptions { reading: Some(reading.clone()), ..c.opts() }, None);
        let launched = go(&factory, &format!("sleep 300 & echo $! > {}; echo first; {}; echo second; sleep 300", c.path("child"), gate(&gate_file)));
        let pid = left_running(&c).await;
        let mut reader = launched.lines();
        assert_eq!(next_line(&mut reader).await.as_deref(), Some("first"));
        // The one run this factory is reading, as the call that lets go of it.
        assert_eq!(reading.len(), 1);

        reading.let_go_all();
        assert_eq!(reading.len(), 0);

        std::fs::write(&gate_file, "go\n").unwrap();
        // The line landed in the log on disk and reaches nobody here: this process stopped reading, and the timer that
        // read it is what would have held the loop open after the last line of whatever asked.
        assert!(within(500, next_line(&mut reader)).await.is_none(), "a line reached a reader that was let go");
        assert!(support::read(&format!("{}.log", launched.run().unwrap())).contains("second"));
        // The stream never settles and the run is left exactly as it stands: ending it here would write the turn off
        // for the process that owns it.
        assert!(within(500, launched.exited()).await.is_none(), "the stream settled after it was let go");
        assert!(!std::path::Path::new(&format!("{}.exit", launched.run().unwrap())).exists());
        assert!(std::path::Path::new(&format!("{}.d", launched.run().unwrap())).exists());
        assert!(alive(pid));

        launched.kill();
        gone(pid).await;
    });

    own_group_test!(a_run_that_ended_on_its_own_is_no_longer_one_to_let_go_of, 15_000, {
        let c = Case::new("wsp-localexec-group-");
        let reading = ReadingSet::new();
        let factory = local_exec_stream(LocalExecOptions { reading: Some(reading.clone()), ..c.opts() }, None);
        let stream = go(&factory, "echo hi");
        assert_eq!(support::lines_ok(&*stream).await, strs(&["hi"]));
        assert_eq!(stream.exited().await, Some(0));
        assert_eq!(reading.len(), 0);
    });

    own_group_test!(teardown_reaches_what_the_turn_started_not_the_shell_alone, 15_000, {
        let c = Case::new("wsp-localexec-group-");
        let factory = c.factory();
        let stream = go(&factory, &format!("sleep 300 & echo $! > {}; sleep 300", c.path("child")));
        let pid = left_running(&c).await;
        stream.teardown();
        assert_ne!(stream.exited().await, Some(0));
        gone(pid).await;
    });

    fn go_input(c: &Case, command: &str, input: &[&str]) -> Arc<dyn ExecStream> {
        c.factory().launch(command, with_input(input))
    }
}

// pinned cases follow
