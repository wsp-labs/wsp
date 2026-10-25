// SPDX-License-Identifier: AGPL-3.0-only
//! machine_exec_factory: the launch, attach and sweep cases of packages/runtime/test/machine-exec.test.ts. Spec:
//! spec/machine_exec_factory.md.
mod support;

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

use futures::future::LocalBoxFuture;
use futures::StreamExt;
use regex::Regex;
use support::guest::{errors, Hook, Inner, LocalGuest, ScriptGuest, Step, Wrapped};
use wsp_adapters::port::{
    Attached, AttachRunOptions, Env, ExecFailure, ExecFailureKind, ExecOptions as Launch, ExecSecret, ExecStream, ExecStreamFactory, WriteOutcome,
    AGENT_PID_FILE_ENV,
};
use wsp_core::machine_exec::{machine_exec_stream, MachineExecFactory, MachineExecOptions, NowFn, SleepFn};
use wsp_engine::errors::{EngineError, ErrorKind, Result as EngineResult};
use wsp_engine::golden_seam::EXEC_ENV;
use wsp_engine::machine::{ExecOptions as MachineExec, ExecResult};
use wsp_engine::Machine;
use wsp_wire::wire::limits::{EXEC_BODY_MAX, EXEC_CHUNK_BYTES, LINK_RETRY_WINDOW_MS};

const WORK_SCORE_LINE: &str = "{ echo 500 > /proc/self/oom_score_adj; } 2>/dev/null; renice 0 $$ >/dev/null 2>&1";
const CUT_6H: &str = "at the 6h turn limit; send to continue where it stopped, or change the limit in Settings > Computers";
const UNREACHABLE_LINE: &str = "the provider cannot reach the machine (Sandbox is not reachable); pause and wake the workspace, or delete it";
const EXEC_FAILED_LINE: &str =
    "the provider cannot run commands on the machine (exec failed) while the machine and its threads may still be running; wait, or pause and wake the workspace, or delete it";
const ABSENT_SENTENCE: &str = "old-laptop is not answering; it connects on its own when it is on";

/// The bytes the provider counts: its request with the backend's wrapper around the command.
fn solari_body(cmd: &str) -> i64 {
    serde_json::json!({ "cmd": "bash", "args": ["-c", format!("{EXEC_ENV}\n{cmd}")], "timeoutMs": 20_000 }).to_string().len() as i64
}

/// Every exec before the first poll: the launch, with the pieces ahead of it when the script needs them.
fn launch_calls(calls: &[String]) -> Vec<String> {
    let at = calls.iter().position(|c| c.contains("__WSP_EOF_")).unwrap_or(calls.len().saturating_sub(1));
    calls[..at].to_vec()
}

/// Forty kilobytes of command, as a turn whose prompt rides the command line would be.
fn big_command() -> String {
    let text = (0..520).map(|i| format!("line {i} it's ünïcödé {}", "y".repeat(50))).collect::<Vec<_>>().join("\n");
    format!("claude -p {} </dev/null", support::sq(&text))
}

fn env_of(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn launch_env(pairs: &[(&str, &str)]) -> Launch {
    Launch { env: env_of(pairs), ..Default::default() }
}

fn launch_input(pairs: &[(&str, &str)], input: &[&str]) -> Launch {
    Launch { env: env_of(pairs), input: Some(input.iter().map(|l| l.to_string()).collect()), ..Default::default() }
}

fn poll(poll_ms: u64) -> MachineExecOptions {
    MachineExecOptions { poll_ms: Some(poll_ms), ..Default::default() }
}

fn poll_in(poll_ms: u64, run_dir: &str) -> MachineExecOptions {
    MachineExecOptions { poll_ms: Some(poll_ms), run_dir: Some(run_dir.to_string()), ..Default::default() }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("the clock").as_millis() as i64
}

fn hook<F, Fut>(f: F) -> Hook
where
    F: Fn(String, Option<Vec<u8>>, Inner) -> Fut + 'static,
    Fut: Future<Output = EngineResult<ExecResult>> + 'static,
{
    Arc::new(move |cmd: String, stdin: Option<Vec<u8>>, inner: Inner| -> LocalBoxFuture<'static, EngineResult<ExecResult>> { Box::pin(f(cmd, stdin, inner)) })
}

fn wrapped<F, Fut>(inner: Arc<dyn Machine>, f: F) -> Arc<dyn Machine>
where
    F: Fn(Arc<dyn Machine>, String, MachineExec) -> Fut + 'static,
    Fut: Future<Output = EngineResult<ExecResult>> + 'static,
{
    let f = Arc::new(move |m: Arc<dyn Machine>, c: String, o: MachineExec| -> LocalBoxFuture<'static, EngineResult<ExecResult>> { Box::pin(f(m, c, o)) });
    Arc::new(Wrapped { inner, f })
}

fn clock_now(clock: &Arc<AtomicI64>) -> NowFn {
    let c = clock.clone();
    Arc::new(move || c.load(SeqCst))
}

fn sleeper(f: impl Fn(u64) + 'static) -> SleepFn {
    Arc::new(move |ms: u64| -> LocalBoxFuture<'static, ()> {
        f(ms);
        Box::pin(async { tokio::task::yield_now().await })
    })
}

fn answered(exit_code: i64, stdout: &str, stderr: &str) -> EngineResult<ExecResult> {
    Ok(ExecResult { exit_code: support::code(exit_code), stdout: stdout.to_string(), stderr: stderr.to_string() })
}

/// What a stream says when its lines end in a failure, or "streamed it" when they do not.
async fn failure_message(stream: &Arc<dyn ExecStream>) -> String {
    let (_, failure) = support::collect(&**stream).await;
    failure.map(|f| f.message).unwrap_or_else(|| "streamed it".to_string())
}

/// The failure the lines of a stream must end on.
async fn failure_of(stream: &Arc<dyn ExecStream>) -> ExecFailure {
    support::lines_err(&**stream).await.1
}

/// Reads a stream to its end, whatever it ends on.
async fn drain(stream: &Arc<dyn ExecStream>) {
    let _ = support::collect(&**stream).await;
}

fn ran(guest_calls: &[String], word: &str) -> usize {
    guest_calls.iter().filter(|c| c.contains(word)).count()
}

/// The rule's waits, each with the jitter it draws on top of its own base.
fn expect_backoffs(waits: &[u64], bases: &[u64]) {
    assert_eq!(waits.len(), bases.len(), "{waits:?} against {bases:?}");
    for (wait, base) in waits.iter().zip(bases) {
        assert!(wait >= base, "{wait} is under its base {base}");
        assert!(*wait < base + 250, "{wait} is past its base {base} plus the jitter");
    }
}

async fn attach(factory: &MachineExecFactory, run: &str, input: bool, started_at: f64) -> Result<Attached, ExecFailure> {
    factory.attach(run, AttachRunOptions { input, started_at, from: None }).expect("this factory attaches").await
}

async fn attach_stream(factory: &MachineExecFactory, run: &str, input: bool, started_at: f64) -> Arc<dyn ExecStream> {
    match attach(factory, run, input, started_at).await {
        Ok(Attached::Stream(stream)) => stream,
        Ok(Attached::Gone) => panic!("the attach answered gone"),
        Err(e) => panic!("the attach failed: {e:?}"),
    }
}

async fn attach_failure(factory: &MachineExecFactory, run: &str, input: bool, started_at: f64) -> ExecFailure {
    match attach(factory, run, input, started_at).await {
        Ok(_) => panic!("the probe answered where it should have failed"),
        Err(e) => e,
    }
}

fn parent_of(run_dir: &str) -> String {
    std::path::Path::new(run_dir).parent().expect("a parent").to_string_lossy().into_owned()
}

fn hex8() -> String {
    format!("{:016x}", rand::random::<u64>())
}

mod machine_exec_stream {
    use super::*;

    #[test]
    fn launches_detached_streams_log_lines_across_polls_and_resolves_the_exit_code() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![
                Step::append("{\"type\":\"system\",\"subtype\":\"init\"}\n{\"type\":\"assist"),
                Step::append("ant\"}\n"),
                Step::append("{\"type\":\"result\"}\n").exit(0),
                Step::none(), // drain poll: exit visible, no new bytes
            ]);
            let factory = machine_exec_stream(machine, poll(5), None);
            let stream = factory.launch("claude -p 'hi' </dev/null", launch_env(&[("CLAUDE_CONFIG_DIR", "/root/.claude-cfg")]));
            let lines = support::lines_ok(&*stream).await;
            assert_eq!(lines, vec!["{\"type\":\"system\",\"subtype\":\"init\"}", "{\"type\":\"assistant\"}", "{\"type\":\"result\"}"]);
            assert_eq!(stream.exited().await, Some(0));
            // The environment rides the launch, ahead of the setsid that starts the script under it, and never the script.
            assert!(guest.get_launch().contains("export CLAUDE_CONFIG_DIR='/root/.claude-cfg'\nsetsid bash "));
            assert!(!guest.get_script().contains("CLAUDE_CONFIG_DIR"));
            assert!(guest.get_script().contains("claude -p 'hi' </dev/null"));
        });
    }

    #[test]
    fn a_run_folder_the_machine_will_not_make_fails_the_launch_rather_than_reading_as_a() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![]);
            // What a machine answers when the run folder belongs to another login on it: the mkdir fails and the folder is
            // not there to claim. Read as a replay, the launch would say it went and leave the reader on a log nobody writes.
            guest.set_hook(hook(|cmd, _stdin, _inner| async move {
                if cmd.contains("WSP_LAUNCHED") {
                    answered(1, "", "mkdir: cannot create directory '/tmp/wsp-run/ab12cd34ef56.d': Permission denied\n")
                } else {
                    answered(0, "", "")
                }
            }));
            let stream = machine_exec_stream(machine, poll(5), None).launch("true", launch_env(&[]));
            assert!(failure_message(&stream).await.contains("remote launch failed"));
        });
    }

    #[test]
    fn a_launch_the_machine_answered_nothing_to_names_the_handshake_that_never_came_and() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![]);
            // The exec landed and the guest printed neither the handshake nor a reason. Exit 0 is success on every other
            // road, so the code on its own reads as a run that started and left the reader polling a log nobody writes.
            guest.set_hook(hook(|_cmd, _stdin, _inner| async move { answered(0, "", "") }));
            let stream = machine_exec_stream(machine.clone(), poll(5), None).launch("true", launch_env(&[]));
            let message = failure_message(&stream).await;
            assert_eq!(
                message,
                format!(
                    "remote launch failed on {}: nothing came back saying WSP_LAUNCHED, the word the guest prints once the run is up; it printed nothing and exited 0",
                    machine.id()
                )
            );
            assert!(!Regex::new(r"exit 0:\s*$").unwrap().is_match(&message));
        });
    }

    #[test]
    fn a_launch_the_machine_refused_carries_what_the_machine_said_beside_the_code() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![]);
            guest.set_hook(hook(|_cmd, _stdin, _inner| async move { answered(1, "", "no run folder on this machine: /tmp/wsp-run/ab12cd34ef56.d\n") }));
            let stream = machine_exec_stream(machine.clone(), poll(5), None).launch("true", launch_env(&[]));
            let message = failure_message(&stream).await;
            assert_eq!(
                message,
                format!(
                    "remote launch failed on {}: nothing came back saying WSP_LAUNCHED, the word the guest prints once the run is up; it exited 1 and said: no run folder on this machine: /tmp/wsp-run/ab12cd34ef56.d",
                    machine.id()
                )
            );
        });
    }

    #[test]
    fn a_command_that_fits_one_exec_body_launches_in_one_exec_under_the_cap() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("hi\n").exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, poll(5), None).launch("claude -p 'hi' </dev/null", launch_env(&[("CLAUDE_CONFIG_DIR", "/root/.claude-cfg")]));
            drain(&stream).await;
            assert_eq!(stream.exited().await, Some(0));
            let launch = launch_calls(&guest.calls());
            assert_eq!(launch.len(), 1);
            let shape = Regex::new(concat!(
                r"^mkdir -p '/tmp/wsp-run'\numask 077\nmkdir '/tmp/wsp-run/[0-9a-f]{12}\.d' 2>/dev/null \|\| \{ \[ -d '/tmp/wsp-run/[0-9a-f]{12}\.d' \] && \{ echo WSP_LAUNCHED; exit 0; \}; echo 'no run folder on this machine: /tmp/wsp-run/[0-9a-f]{12}\.d' >&2; exit 1; \}",
                r"\nset -o pipefail\nprintf %s '[A-Za-z0-9+/=]+' \| base64 -d > '/tmp/wsp-run/[0-9a-f]{12}\.sh' \|\| exit 1\nexport WSP_AGENT_PID_FILE='/tmp/wsp-run/[0-9a-f]{12}\.agent'\nexport CLAUDE_CONFIG_DIR='/root/\.claude-cfg'\nsetsid bash "
            ))
            .unwrap();
            assert!(shape.is_match(&launch[0]), "{}", launch[0]);
            assert!(solari_body(&launch[0]) <= EXEC_BODY_MAX);
        });
    }

    #[test]
    fn a_40_kb_command_goes_up_in_pieces_every_exec_body_under_the_cap_and_the_script_d() {
        support::run_local(async {
            let command = big_command();
            assert!(command.len() > 40_000);
            let (guest, machine) = ScriptGuest::new(vec![Step::append("hi\n").exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, poll(5), None).launch(&command, launch_env(&[("CLAUDE_CONFIG_DIR", "/root/.claude-cfg")]));
            let lines = support::lines_ok(&*stream).await;
            assert_eq!(lines, vec!["hi"]);
            assert_eq!(stream.exited().await, Some(0));
            for c in guest.calls() {
                assert!(solari_body(&c) <= EXEC_BODY_MAX, "{}", c.chars().take(80).collect::<String>());
            }
            let launch = launch_calls(&guest.calls());
            assert_eq!(launch.iter().filter(|c| c.ends_with("echo WSP_PIECE")).count(), 4);
            assert_eq!(launch.len(), 5);
            // Every piece is written under the mask too: it holds the same bytes as the script it is cut from, and it waits
            // on disk until the last exec joins the pieces and removes them.
            for piece in launch.iter().filter(|c| c.ends_with("echo WSP_PIECE")) {
                let lines: Vec<&str> = piece.split('\n').collect();
                assert_eq!(lines[1], "umask 077");
                assert!(lines[2].starts_with("printf %s "));
            }
            // The exit file is written under the same base as the script, and its path is quoted like every other; the agent's
            // pid file rides the launch with the rest of the environment, ahead of setsid.
            let last = launch.last().unwrap();
            let base = Regex::new(r"> '([^']*)\.sh'").unwrap().captures(last).expect("the script's path")[1].to_string();
            assert_eq!(guest.get_script(), format!("{WORK_SCORE_LINE}\n{command}\necho $? > '{base}'.exit\n"));
            assert!(last.contains(&format!("export {AGENT_PID_FILE_ENV}='{base}.agent'\n")));
        });
    }

    #[test]
    fn makes_every_file_of_a_run_under_the_login_s_own_mask_from_the_claim_folder_down() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("hi\n").exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, poll(5), None).launch("claude -p 'hi'", launch_input(&[("ANTHROPIC_API_KEY", "sk-ant-x")], &["one"]));
            drain(&stream).await;
            let launch = launch_calls(&guest.calls());
            let lines: Vec<String> = launch.last().unwrap().split('\n').map(str::to_string).collect();
            // Ahead of the claim's mkdir, so the folder the script, the input, the fifo, the log and the pid file land in
            // is the login's own; another account on a machine the person owns must not read the key in those exports.
            assert_eq!(lines[1], "umask 077");
            assert!(lines[2].starts_with("mkdir '/tmp/wsp-run/"));
            let umask = lines.iter().position(|l| l == "umask 077").unwrap();
            let setsid = lines.iter().position(|l| l.starts_with("setsid bash ")).expect("the setsid line");
            assert!(umask < setsid);
        });
    }

    #[test]
    fn the_run_script_puts_the_turn_s_processes_at_the_work_score_before_anything_else() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("hi\n").exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, poll(5), None).launch("claude -p 'hi'", launch_env(&[("PATH", "/root/.local/bin:/usr/bin")]));
            drain(&stream).await;
            let script = guest.get_script();
            let head: Vec<&str> = script.split('\n').take(2).collect();
            assert_eq!(head, vec![WORK_SCORE_LINE, "claude -p 'hi'"]);
        });
    }

    #[test]
    fn without_an_input_channel_the_command_runs_as_before_no_input_file_no_tail_and_wr() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("hi\n").exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, poll(5), None).launch("echo hi", launch_env(&[]));
            let refused = stream.write("x".to_string()).await.expect_err("a write with no channel rejects");
            assert!(refused.message.contains("channel"), "{}", refused.message);
            drain(&stream).await;
            assert!(!guest.get_script().contains("tail"));
            assert!(!guest.get_launch().contains(".in"));
            assert!(guest.writes().is_empty());
        });
    }

    #[test]
    fn with_an_input_channel_the_launch_seeds_the_file_and_the_script_feeds_it_to_the_c() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("{\"type\":\"result\"}\n").exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, poll_in(5, "/tmp/r"), None)
                .launch("cd ~ && claude -p --input-format stream-json", launch_input(&[], &["{\"type\":\"user\",\"text\":\"go\"}"]));
            drain(&stream).await;
            assert_eq!(stream.exited().await, Some(0));
            assert_eq!(guest.get_input(), "{\"type\":\"user\",\"text\":\"go\"}\n");
            // Every path in the script is one quoted word, so a run folder with a space in it feeds the same command.
            let base = Regex::new(r"mkfifo '(/tmp/r/[a-f0-9]+)'\.fifo").unwrap().captures(&guest.get_launch()).expect("the fifo's path")[1].to_string();
            let script = guest.get_script();
            assert!(script.contains(&format!("( tail -n +1 -f '{base}'.in > '{base}'.fifo & echo $! > '{base}'.tail )")));
            assert!(script.contains(&format!("}} < '{base}'.fifo")));
            assert!(script.contains("cd ~ && claude -p --input-format stream-json"));
            let exit_at = script.find(&format!("echo $? > '{base}'.exit")).expect("the exit write");
            let kill_at = script.find(&format!("kill $(cat '{base}'.tail)")).expect("the tail's kill");
            assert!(exit_at < kill_at);
        });
    }

    #[test]
    fn a_seeded_prompt_over_the_cap_goes_up_in_pieces_ahead_of_the_launch_every_exec_bo() {
        support::run_local(async {
            let text = (0..520).map(|i| format!("line {i} it's ünïcödé {}", "y".repeat(50))).collect::<Vec<_>>().join("\n");
            let prompt = format!("{{\"type\":\"user\",\"text\":{}}}", serde_json::to_string(&text).unwrap());
            assert!(prompt.len() > 40_000);
            let (guest, machine) = ScriptGuest::new(vec![Step::append("{\"type\":\"result\"}\n").exit(0), Step::none()]);
            let stream = machine_exec_stream(machine, poll(5), None).launch("claude -p --input-format stream-json", launch_input(&[], &[prompt.as_str()]));
            drain(&stream).await;
            assert_eq!(stream.exited().await, Some(0));
            for c in guest.calls() {
                assert!(solari_body(&c) <= EXEC_BODY_MAX, "{}", c.chars().take(80).collect::<String>());
            }
            let launch = launch_calls(&guest.calls());
            assert_eq!(launch.iter().filter(|c| c.ends_with("echo WSP_PIECE")).count(), 4);
            assert_eq!(launch.len(), 5);
            let shape = Regex::new(concat!(
                r"(?s)mkdir '/tmp/wsp-run/[a-f0-9]+\.d' 2>/dev/null \|\| \{ \[ -d '/tmp/wsp-run/[a-f0-9]+\.d' \] && \{ echo WSP_LAUNCHED; exit 0; \}; echo 'no run folder on this machine: /tmp/wsp-run/[a-f0-9]+\.d' >&2; exit 1; \}",
                r"\nset -o pipefail\n.*\nmkfifo '/tmp/wsp-run/[a-f0-9]+'\.fifo\nsetsid bash "
            ))
            .unwrap();
            assert!(shape.is_match(launch.last().unwrap()));
            assert_eq!(guest.get_input(), format!("{prompt}\n"));
            assert!(guest.get_script().contains("claude -p --input-format stream-json"));
        });
    }

    /// A guest whose launch fails `failures` times the way `make` says, each failed post costing five seconds of the
    /// clock as a DNS lookup that times out does; the stream's sleeps move the same clock.
    struct Unreached {
        guest: Arc<ScriptGuest>,
        machine: Arc<dyn Machine>,
        opts: MachineExecOptions,
        launches: Arc<AtomicU32>,
        waits: Arc<Mutex<Vec<u64>>>,
    }

    impl Unreached {
        fn launches(&self) -> u32 {
            self.launches.load(SeqCst)
        }
        fn backoffs(&self) -> Vec<u64> {
            self.waits.lock().unwrap().iter().copied().filter(|w| *w != 5).collect()
        }
    }

    fn unreached_guest(failures: u32, make: fn() -> EngineError) -> Unreached {
        let (guest, machine) = ScriptGuest::new(vec![Step::append("ran\n").exit(0), Step::none()]);
        let clock = Arc::new(AtomicI64::new(0));
        let waits = Arc::new(Mutex::new(Vec::<u64>::new()));
        let launches = Arc::new(AtomicU32::new(0));
        let (c, l) = (clock.clone(), launches.clone());
        guest.set_hook(hook(move |cmd, _stdin, inner| {
            let (c, l) = (c.clone(), l.clone());
            async move {
                if cmd.contains("WSP_LAUNCHED") {
                    let n = l.fetch_add(1, SeqCst) + 1;
                    if n <= failures {
                        c.fetch_add(5_000, SeqCst);
                        return Err(make());
                    }
                }
                inner(&cmd)
            }
        }));
        let (c, w) = (clock.clone(), waits.clone());
        let opts = MachineExecOptions {
            poll_ms: Some(5),
            now: Some(clock_now(&clock)),
            sleep: Some(sleeper(move |ms| {
                w.lock().unwrap().push(ms);
                c.fetch_add(ms as i64, SeqCst);
            })),
            ..Default::default()
        };
        Unreached { guest, machine, opts, launches, waits }
    }

    fn fetch_failed() -> EngineError {
        errors::fetch_failed("EAI_AGAIN")
    }

    fn missing_gone() -> EngineError {
        errors::refusal(ErrorKind::Missing, 404, "gone")
    }

    #[test]
    fn a_launch_nothing_answered_twice_is_posted_again_at_the_engine_s_backoff_and_the() {
        support::run_local(async {
            let g = unreached_guest(2, fetch_failed);
            let stream = machine_exec_stream(g.machine.clone(), g.opts.clone(), None).launch("claude -p hi", launch_env(&[]));
            let lines = support::lines_ok(&*stream).await;
            assert_eq!(lines, vec!["ran"]);
            assert_eq!(stream.exited().await, Some(0));
            assert_eq!(g.launches(), 3);
            expect_backoffs(&g.backoffs(), &[500, 1_000]);
        });
    }

    #[test]
    fn a_launch_nothing_answers_for_the_whole_link_window_fails_the_turn_in_the_protoco() {
        support::run_local(async {
            let g = unreached_guest(99, fetch_failed);
            let stream = machine_exec_stream(g.machine.clone(), g.opts.clone(), None).launch("claude -p hi", launch_env(&[]));
            let failure = failure_of(&stream).await;
            assert_eq!(failure.kind, ExecFailureKind::Unreached);
            // Seven five-second posts and the waits between them run past the minute a name that will not resolve is given.
            assert!(Regex::new(r"^the machine could not be reached from this computer after 7 attempts over 1m \d+s$").unwrap().is_match(&failure.message), "{}", failure.message);
            assert!(!failure.message.contains("fetch failed"));
            assert!(!failure.message.contains(g.machine.id()));
            assert_eq!(g.launches(), 7);
            expect_backoffs(&g.backoffs(), &[500, 1_000, 2_000, 4_000, 8_000, 10_000]);
            assert_eq!(stream.exited().await, None);
            assert_eq!(g.guest.kills().len(), 1);
            assert!(g.guest.files().is_empty());
        });
    }

    #[test]
    fn a_launch_whose_answer_was_lost_after_it_ran_is_read_as_that_run_once_the_machine() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("ran\n").exit(0), Step::none()]);
            let clock = Arc::new(AtomicI64::new(0));
            let landed = Arc::new(AtomicU32::new(0));
            let dark = Arc::new(AtomicBool::new(true));
            // The first post lands and its answer is lost; nothing answers after it until past the launch's own link window,
            // inside the one the question about its run is given.
            let (c, la, d) = (clock.clone(), landed.clone(), dark.clone());
            guest.set_hook(hook(move |cmd, _stdin, inner| {
                let (c, la, d) = (c.clone(), la.clone(), d.clone());
                async move {
                    if !d.load(SeqCst) {
                        return inner(&cmd);
                    }
                    if cmd.contains("WSP_LAUNCHED") && la.load(SeqCst) == 0 {
                        la.fetch_add(1, SeqCst);
                        if let Err(e) = inner(&cmd) {
                            return Err(e);
                        }
                    }
                    c.fetch_add(5_000, SeqCst);
                    Err(errors::reset())
                }
            }));
            let (c, d) = (clock.clone(), dark.clone());
            let opts = MachineExecOptions {
                poll_ms: Some(5),
                now: Some(clock_now(&clock)),
                sleep: Some(sleeper(move |ms| {
                    let now = c.fetch_add(ms as i64, SeqCst) + ms as i64;
                    if now as f64 > 1.5 * LINK_RETRY_WINDOW_MS as f64 {
                        d.store(false, SeqCst);
                    }
                })),
                ..Default::default()
            };
            let stream = machine_exec_stream(machine, opts, None).launch("claude -p hi", launch_env(&[]));
            let lines = support::lines_ok(&*stream).await;
            assert_eq!(lines, vec!["ran"]);
            assert_eq!(stream.exited().await, Some(0));
            assert_eq!(landed.load(SeqCst), 1);
            assert_eq!(ran(&guest.calls(), "WSP_LAUNCHED"), 1);
        });
    }

    struct LandedThenDark {
        lines: Vec<String>,
        failure: Option<ExecFailure>,
        at: i64,
        launches: usize,
    }

    /// The first launch post lands on the machine and its answer is lost with `lost`; every exec after it fails with
    /// `meanwhile` until `dark_ms` of the stream's clock, then the machine answers again and holds the run.
    async fn landed_then_dark(lost: fn() -> EngineError, meanwhile: fn() -> EngineError, dark_ms: i64, asks_until_stopped: bool, stop_at: Option<i64>) -> LandedThenDark {
        let (guest, machine) = ScriptGuest::new(vec![Step::append("ran\n").exit(0), Step::none()]);
        let clock = Arc::new(AtomicI64::new(0));
        let landed = Arc::new(AtomicU32::new(0));
        let cell: Arc<Mutex<Option<Arc<dyn ExecStream>>>> = Arc::new(Mutex::new(None));
        let (c, la, cl) = (clock.clone(), landed.clone(), cell.clone());
        guest.set_hook(hook(move |cmd, _stdin, inner| {
            let (c, la, cl) = (c.clone(), la.clone(), cl.clone());
            async move {
                if c.load(SeqCst) >= dark_ms {
                    return inner(&cmd);
                }
                let now = c.fetch_add(1_000, SeqCst) + 1_000;
                if let Some(at) = stop_at {
                    if now >= at {
                        let stream = cl.lock().unwrap().clone();
                        if let Some(stream) = stream {
                            stream.kill();
                        }
                    }
                }
                if cmd.contains("WSP_LAUNCHED") && la.load(SeqCst) == 0 {
                    la.fetch_add(1, SeqCst);
                    if let Err(e) = inner(&cmd) {
                        return Err(e);
                    }
                    return Err(lost());
                }
                Err(meanwhile())
            }
        }));
        let c = clock.clone();
        let opts = MachineExecOptions {
            poll_ms: Some(5),
            now: Some(clock_now(&clock)),
            sleep: Some(sleeper(move |ms| {
                c.fetch_add(ms as i64, SeqCst);
            })),
            asks_until_stopped,
            ..Default::default()
        };
        let stream = machine_exec_stream(machine, opts, None).launch("claude -p hi", launch_env(&[]));
        *cell.lock().unwrap() = Some(stream.clone());
        let (lines, failure) = support::collect(&*stream).await;
        LandedThenDark { lines, failure, at: clock.load(SeqCst), launches: ran(&guest.calls(), "WSP_LAUNCHED") }
    }

    #[test]
    fn a_launch_that_ran_and_lost_its_answer_to_a_link_drop_is_read_as_that_run_while_t() {
        support::run_local(async {
            let r = landed_then_dark(|| errors::plain("connection lost"), || errors::absent("lab-box is not connected"), 20_000, false, None).await;
            assert!(r.failure.is_none(), "{:?}", r.failure);
            assert_eq!(r.lines, vec!["ran"]);
            assert_eq!(r.launches, 1);
        });
    }

    #[test]
    fn a_launch_that_ran_and_lost_its_answer_is_read_as_that_run_while_the_provider_say() {
        support::run_local(async {
            let r = landed_then_dark(errors::reset, || errors::unreachable("m1", "Sandbox is not reachable", 502, "the computer is not reachable"), 20_000, false, None).await;
            assert!(r.failure.is_none(), "{:?}", r.failure);
            assert_eq!(r.lines, vec!["ran"]);
            assert_eq!(r.launches, 1);
        });
    }

    #[test]
    fn a_line_s_try_whose_launch_ran_and_lost_its_answer_keeps_asking_after_its_run_pas() {
        support::run_local(async {
            let r = landed_then_dark(errors::reset, errors::reset, 150_000, true, None).await;
            assert!(r.failure.is_none(), "{:?}", r.failure);
            assert_eq!(r.lines, vec!["ran"]);
            assert_eq!(r.launches, 1);
        });
    }

    #[test]
    fn a_line_s_try_asking_after_its_run_ends_at_the_stop_and_not_before_in_the_launch() {
        support::run_local(async {
            let r = landed_then_dark(errors::reset, errors::reset, 600_000, true, Some(300_000)).await;
            assert_eq!(r.failure.expect("the stream failed").kind, ExecFailureKind::Unreached);
            assert!(r.at >= 5 * LINK_RETRY_WINDOW_MS, "{}", r.at);
            assert!(r.at < 6 * LINK_RETRY_WINDOW_MS, "{}", r.at);
            assert!(r.lines.is_empty(), "{:?}", r.lines);
        });
    }

    struct Dark {
        machine: Arc<dyn Machine>,
        failure: ExecFailure,
        at: i64,
        calls: Vec<String>,
    }

    /// A machine every exec of which fails the way `fail` says, each costing a second of the stream's clock.
    async fn dark_machine(fail: fn() -> EngineError) -> Dark {
        let (guest, machine) = ScriptGuest::new(vec![Step::append("ran\n").exit(0), Step::none()]);
        let clock = Arc::new(AtomicI64::new(0));
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let (c, k) = (clock.clone(), calls.clone());
        guest.set_hook(hook(move |cmd, _stdin, _inner| {
            let (c, k) = (c.clone(), k.clone());
            async move {
                k.lock().unwrap().push(cmd);
                c.fetch_add(1_000, SeqCst);
                Err(fail())
            }
        }));
        let c = clock.clone();
        let opts = MachineExecOptions {
            now: Some(clock_now(&clock)),
            sleep: Some(sleeper(move |ms| {
                c.fetch_add(ms as i64, SeqCst);
            })),
            ..Default::default()
        };
        let stream = machine_exec_stream(machine.clone(), opts, None).launch("claude -p hi", launch_env(&[]));
        let failure = failure_of(&stream).await;
        let seen = calls.lock().unwrap().clone();
        Dark { machine, failure, at: clock.load(SeqCst), calls: seen }
    }

    #[test]
    fn a_launch_the_provider_refuses_fails_at_once_in_its_words_and_never_asks_after_a() {
        support::run_local(async {
            let r = dark_machine(missing_gone).await;
            assert_eq!(r.failure.message, format!("remote launch failed on {}: gone", r.machine.id()));
            assert_eq!(ran(&r.calls, "WSP_LAUNCHED"), 1);
            assert!(r.calls.iter().filter(|c| c.contains("WSP_RUN")).next().is_none(), "{:?}", r.calls);
            assert!(r.at < 10_000, "{}", r.at);
        });
    }

    #[test]
    fn a_launch_whose_posts_went_out_to_a_computer_that_never_answers_again_fails_as_un() {
        support::run_local(async {
            let r = dark_machine(errors::reset).await;
            assert_eq!(r.failure.kind, ExecFailureKind::Unreached);
            assert!(r.calls.iter().any(|c| c.contains("WSP_RUN")));
            assert!(r.at > LINK_RETRY_WINDOW_MS, "{}", r.at);
            assert!(r.at < 3 * LINK_RETRY_WINDOW_MS, "{}", r.at);
        });
    }

    #[test]
    fn a_launch_none_of_whose_posts_left_this_computer_fails_without_asking_the_machine() {
        support::run_local(async {
            let g = unreached_guest(99, fetch_failed);
            let stream = machine_exec_stream(g.machine.clone(), g.opts.clone(), None).launch("claude -p hi", launch_env(&[]));
            assert_eq!(failure_of(&stream).await.kind, ExecFailureKind::Unreached);
            assert!(g.guest.calls().iter().filter(|c| c.contains("WSP_RUN")).next().is_none());
        });
    }

    #[test]
    fn a_launch_that_failed_for_any_reason_but_the_network_fails_at_once_in_the_machine() {
        support::run_local(async {
            let g = unreached_guest(99, missing_gone);
            let stream = machine_exec_stream(g.machine.clone(), g.opts.clone(), None).launch("claude -p hi", launch_env(&[]));
            let failure = failure_of(&stream).await;
            assert!(failure.message.contains(&format!("remote launch failed on {}: gone", g.machine.id())), "{}", failure.message);
            assert_eq!(g.launches(), 1);
            assert!(g.backoffs().is_empty());
            assert_eq!(stream.exited().await, None);
        });
    }

    #[test]
    fn a_launch_on_a_computer_that_is_not_connected_keeps_that_computer_s_own_sentence() {
        support::run_local(async {
            let g = unreached_guest(99, || errors::absent(ABSENT_SENTENCE));
            let stream = machine_exec_stream(g.machine.clone(), g.opts.clone(), None).launch("claude -p hi", launch_env(&[]));
            let (_, thrown) = support::collect(&*stream).await;
            let thrown = thrown.expect("the stream failed");
            // The sentence the app holds the send with, word for word: a wrapper naming the machine put place:p_oldlaptop
            // in front of the one line that says what to do.
            assert_eq!(thrown.message, ABSENT_SENTENCE);
            assert!(!thrown.message.contains(g.machine.id()));
            assert!(!thrown.message.contains("remote launch failed"));
        });
    }

    #[test]
    fn a_launch_the_provider_refuses_because_it_cannot_reach_the_machine_fails_once_in() {
        support::run_local(async {
            let g = unreached_guest(99, || errors::unreachable("m1", "Sandbox is not reachable", 502, UNREACHABLE_LINE));
            let stream = machine_exec_stream(g.machine.clone(), g.opts.clone(), None).launch("claude -p hi", launch_env(&[]));
            let (_, thrown) = support::collect(&*stream).await;
            let thrown = thrown.expect("the stream failed");
            assert_eq!(thrown.message, UNREACHABLE_LINE);
            assert!(!thrown.message.contains(g.machine.id()));
            assert!(!thrown.message.contains("remote launch failed"));
            assert_eq!(g.launches(), 1);
            assert!(g.backoffs().is_empty());
        });
    }

    #[test]
    fn a_launch_the_provider_refuses_because_it_cannot_run_commands_on_the_machine_fail() {
        support::run_local(async {
            let g = unreached_guest(99, || errors::exec_failed("m1", "exec failed", 502, EXEC_FAILED_LINE));
            let stream = machine_exec_stream(g.machine.clone(), g.opts.clone(), None).launch("claude -p hi", launch_env(&[]));
            let (_, thrown) = support::collect(&*stream).await;
            let thrown = thrown.expect("the stream failed");
            assert_eq!(thrown.message, EXEC_FAILED_LINE);
            assert!(!thrown.message.contains(g.machine.id()));
            assert!(!thrown.message.contains("remote launch failed"));
            assert_eq!(g.launches(), 1);
            assert!(g.backoffs().is_empty());
        });
    }

    #[test]
    fn a_gateway_status_the_edge_answered_is_not_retried_by_the_launch_the_backend_alre() {
        support::run_local(async {
            let g = unreached_guest(99, || errors::refusal(ErrorKind::Transient, 503, "Service Unavailable"));
            let stream = machine_exec_stream(g.machine.clone(), g.opts.clone(), None).launch("claude -p hi", launch_env(&[]));
            let failure = failure_of(&stream).await;
            assert!(failure.message.contains(&format!("remote launch failed on {}: Service Unavailable", g.machine.id())), "{}", failure.message);
            assert_eq!(g.launches(), 1);
            assert!(g.backoffs().is_empty());
        });
    }
}

mod machine_exec_stream_attaching_to_a_run_its_process_did_not_launch {
    use super::*;

    struct Abandoned {
        factory: Arc<MachineExecFactory>,
        guest: Arc<ScriptGuest>,
        machine: Arc<dyn Machine>,
        run: String,
        _launched: Arc<dyn ExecStream>,
    }

    /// The run is claimed by the launch alone: nothing iterates the launched stream, as a host that went down under
    /// its own reader did not.
    async fn abandoned(steps: Vec<Step>) -> Abandoned {
        let (guest, machine) = ScriptGuest::new(steps);
        let factory = machine_exec_stream(machine.clone(), poll(5), None);
        let launched = factory.launch("claude -p hi", launch_input(&[], &["go"]));
        support::wait_for("the launch", 5_000, || !guest.get_launch().is_empty()).await;
        let run = launched.run().expect("a run handle");
        Abandoned { factory, guest, machine, run, _launched: launched }
    }

    const THREE: &str = "{\"type\":\"system\",\"subtype\":\"init\"}\n{\"type\":\"assistant\"}\n{\"type\":\"result\"}\n";

    fn three_lines() -> Vec<String> {
        vec!["{\"type\":\"system\",\"subtype\":\"init\"}".into(), "{\"type\":\"assistant\"}".into(), "{\"type\":\"result\"}".into()]
    }

    #[test]
    fn reads_the_run_s_whole_log_from_its_first_byte_and_ends_on_the_exit_the_run_left() {
        support::run_local(async {
            let a = abandoned(vec![Step::append("{\"type\":\"system\",\"subtype\":\"init\"}\n{\"type\":\"assistant\"}\n"), Step::append("{\"type\":\"result\"}\n").exit(0)]).await;
            let stream = attach_stream(&a.factory, &a.run, true, now_ms() as f64).await;
            let lines = support::lines_ok(&*stream).await;
            assert_eq!(lines, three_lines());
            assert_eq!(stream.exited().await, Some(0));
        });
    }

    #[test]
    fn hands_over_every_line_the_run_s_channel_took_the_launch_s_own_and_each_written_s() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("{\"type\":\"system\",\"subtype\":\"init\"}\n"), Step::append("{\"type\":\"result\"}\n").exit(0)]);
            let launched = machine_exec_stream(machine.clone(), poll(5), None).launch("claude -p hi", launch_input(&[], &["go"]));
            support::wait_for("the launch", 5_000, || !guest.get_launch().is_empty()).await;
            assert_eq!(launched.write("{\"type\":\"user\",\"uuid\":\"u1\"}".to_string()).await.unwrap(), WriteOutcome::Written);
            let next = machine_exec_stream(machine.clone(), poll(5), None);
            let stream = attach_stream(&next, &launched.run().unwrap(), true, now_ms() as f64).await;
            assert_eq!(stream.taken(), Some(vec!["go".to_string(), "{\"type\":\"user\",\"uuid\":\"u1\"}".to_string()]));
            drain(&stream).await;
            // A reader that writes nothing is handed nothing.
            let a = abandoned(vec![Step::append("{\"type\":\"result\"}\n").exit(0)]).await;
            let reader = attach_stream(&a.factory, &a.run, false, now_ms() as f64).await;
            assert_eq!(reader.taken(), None);
            drain(&reader).await;
        });
    }

    #[test]
    fn keeps_the_reply_of_a_run_that_finished_while_no_host_read_it_when_it_is_re_opene() {
        support::run_local(async {
            // Everything the run printed, and its exit, were on the machine before anything re-opened it.
            let a = abandoned(vec![Step::append(THREE).exit(0)]).await;
            let stream = attach_stream(&a.factory, &a.run, true, (now_ms() - 7 * 3_600_000) as f64).await;
            let lines = support::lines_ok(&*stream).await;
            assert_eq!(lines, three_lines());
            assert_eq!(stream.exited().await, Some(0));
        });
    }

    #[test]
    fn keeps_the_reply_of_a_finished_run_re_opened_past_its_wall_when_the_poll_after_th() {
        support::run_local(async {
            let a = abandoned(vec![Step::append(THREE).exit(0)]).await;
            // The probe reached the machine; the first poll after it meets a gateway that falls over once.
            let failed = Arc::new(AtomicBool::new(false));
            let f = failed.clone();
            a.guest.set_hook(hook(move |cmd, _stdin, inner| {
                let f = f.clone();
                async move {
                    if cmd.contains("__WSP_EOF_") && !f.swap(true, SeqCst) {
                        return Err(errors::plain("gateway said 502"));
                    }
                    inner(&cmd)
                }
            }));
            let stream = attach_stream(&a.factory, &a.run, true, (now_ms() - 7 * 3_600_000) as f64).await;
            let lines = support::lines_ok(&*stream).await;
            assert!(failed.load(SeqCst));
            assert_eq!(lines, three_lines());
            assert_eq!(stream.exited().await, Some(0));
        });
    }

    #[test]
    fn cuts_a_turn_re_opened_past_its_wall_on_a_machine_that_stays_dark_once_the_reach() {
        support::run_local(async {
            let a = abandoned(vec![Step::append("while-away\n")]).await;
            let clock = Arc::new(AtomicI64::new(0));
            let c = clock.clone();
            a.guest.set_hook(hook(move |cmd, _stdin, inner| {
                let c = c.clone();
                async move {
                    if !cmd.contains("__WSP_EOF_") {
                        return inner(&cmd);
                    }
                    c.fetch_add(10_000, SeqCst);
                    Err(errors::plain("gateway said 502"))
                }
            }));
            let factory = machine_exec_stream(a.machine.clone(), MachineExecOptions { poll_ms: Some(1), now: Some(clock_now(&clock)), ..Default::default() }, None);
            let stream = attach_stream(&factory, &a.run, true, -7.0 * 3_600_000.0).await;
            let failure = failure_of(&stream).await;
            assert!(failure.message.ends_with(CUT_6H), "{}", failure.message);
            assert!(clock.load(SeqCst) >= LINK_RETRY_WINDOW_MS, "{}", clock.load(SeqCst));
            assert!(clock.load(SeqCst) < LINK_RETRY_WINDOW_MS + 20_000, "{}", clock.load(SeqCst));
            assert_eq!(stream.exited().await, None);
        });
    }

    #[test]
    fn hands_over_what_a_run_still_going_printed_before_it_is_cut_when_it_is_re_opened() {
        support::run_local(async {
            let a = abandoned(vec![Step::append("while-away\n")]).await;
            let stream = attach_stream(&a.factory, &a.run, true, (now_ms() - 7 * 3_600_000) as f64).await;
            let (lines, failure) = support::lines_err(&*stream).await;
            assert!(failure.message.ends_with(CUT_6H), "{}", failure.message);
            assert_eq!(lines, vec!["while-away"]);
            assert_eq!(stream.exited().await, None);
        });
    }

    #[test]
    fn takes_a_message_into_the_run_over_the_channel_the_launch_left_open() {
        support::run_local(async {
            let a = abandoned(vec![Step::append("one\n"), Step::append("two\n").exit(0)]).await;
            let stream = attach_stream(&a.factory, &a.run, true, now_ms() as f64).await;
            assert_eq!(stream.write("{\"type\":\"user\"}".to_string()).await.unwrap(), WriteOutcome::Written);
            assert_eq!(a.guest.get_input(), "go\n{\"type\":\"user\"}\n");
            drain(&stream).await;
            assert_eq!(stream.exited().await, Some(0));
        });
    }

    #[test]
    fn a_machine_that_answers_and_no_longer_holds_the_run_says_gone_with_no_reader_and() {
        support::run_local(async {
            let a = abandoned(vec![Step::append("never read\n").exit(0)]).await;
            a.guest.sweep();
            match attach(&a.factory, &a.run, true, now_ms() as f64).await {
                Ok(Attached::Gone) => {}
                Ok(Attached::Stream(_)) => panic!("the attach opened a stream where the run is gone"),
                Err(e) => panic!("the attach failed: {e:?}"),
            }
            assert!(a.guest.polls().is_empty());
        });
    }

    #[test]
    fn a_probe_nothing_answers_leaves_the_run_alone_no_kill_no_rm_and_the_reach_window() {
        support::run_local(async {
            let a = abandoned(vec![Step::append("still working\n")]).await;
            a.guest.refuse_probes(errors::plain("gateway said 502"));
            let failure = attach_failure(&a.factory, &a.run, true, now_ms() as f64).await;
            assert_eq!(failure.message, "gateway said 502");
            // The turn this attach was sent to save is still running, and its log is still there to be read next time.
            assert!(a.guest.kills().is_empty());
            assert!(a.guest.child_alive());
            assert!(a.guest.files().contains(&format!("{}.sh", a.run)), "{:?}", a.guest.files());
        });
    }

    #[test]
    fn a_probe_whose_road_is_out_is_asked_again_over_the_reach_window_before_it_gives_u() {
        support::run_local(async {
            let (guest, machine) = ScriptGuest::new(vec![Step::append("still working\n")]);
            let clock = Arc::new(AtomicI64::new(0));
            let c = clock.clone();
            let opts = MachineExecOptions {
                poll_ms: Some(5),
                now: Some(clock_now(&clock)),
                sleep: Some(sleeper(move |ms| {
                    c.fetch_add(ms as i64, SeqCst);
                })),
                ..Default::default()
            };
            let factory = machine_exec_stream(machine, opts, None);
            let launched = factory.launch("claude -p hi", launch_input(&[], &["go"]));
            support::wait_for("the launch", 5_000, || !guest.get_launch().is_empty()).await;
            guest.refuse_probes(errors::fetch_failed("ECONNRESET"));
            let failure = attach_failure(&factory, &launched.run().unwrap(), true, now_ms() as f64).await;
            assert_eq!(failure.kind, ExecFailureKind::Unreached);
            assert!(guest.probes() > 1, "{}", guest.probes());
            assert!(guest.kills().is_empty());
            assert!(guest.child_alive());
        });
    }

    #[test]
    fn a_machine_that_answers_neither_way_is_not_a_run_to_end() {
        support::run_local(async {
            let a = abandoned(vec![Step::append("still working\n")]).await;
            a.guest.garble_probes();
            let failure = attach_failure(&a.factory, &a.run, true, now_ms() as f64).await;
            assert!(failure.message.contains("did not answer whether it still holds"), "{}", failure.message);
            assert!(a.guest.kills().is_empty());
            assert!(a.guest.child_alive());
        });
    }

    #[test]
    fn a_handle_this_host_could_not_have_launched_is_refused_before_any_shell_text_goes() {
        support::run_local(async {
            let a = abandoned(vec![Step::append("still working\n")]).await;
            let before = a.guest.calls().len();
            for bad in ["/tmp/wsp-run/../../etc/x", "/tmp/wsp-run/$(id)", "/etc/wsp-run/aabbccddeeff", "/tmp/wsp-run/nothex000000", "/tmp/wsp-run/aabbccddeef"] {
                let failure = attach_failure(&a.factory, bad, true, now_ms() as f64).await;
                assert!(failure.message.contains("is not a run this host could have launched"), "{}", failure.message);
            }
            assert_eq!(a.guest.calls().len(), before);
        });
    }
}

mod machine_exec_stream_over_this_machine_s_bash {
    use super::*;

    own_group_test!(a_launch_posted_twice_under_one_base_as_a_retried_exec_does_starts_the_script_on, 15_000, {
        let (guest, run_dir, _dir) = LocalGuest::new(false, "");
        let local: Arc<dyn Machine> = guest.clone();
        let marks = format!("{}/marks", parent_of(&run_dir));
        let retrying = wrapped(local, |inner, cmd, opts| async move {
            let first = inner.exec(&cmd, opts.clone()).await;
            if cmd.contains("echo WSP_LAUNCHED") {
                inner.exec(&cmd, opts).await
            } else {
                first
            }
        });
        let stream = machine_exec_stream(retrying, poll_in(50, &run_dir), None).launch(&format!("echo ran >> {marks}; echo hi"), launch_env(&[("WSP_MARK", "x")]));
        let lines = support::lines_ok(&*stream).await;
        assert_eq!(stream.exited().await, Some(0));
        assert_eq!(lines, vec!["hi"]);
        assert_eq!(support::read(&marks), "ran\n");
    });

    own_group_test!(an_attach_reads_back_only_the_last_exec_chunk_bytes_of_the_run_s_channel_whole_l, 15_000, {
        let (guest, run_dir, _dir) = LocalGuest::new(false, "");
        let machine: Arc<dyn Machine> = guest.clone();
        let gate = format!("{}/gate", parent_of(&run_dir));
        let factory = machine_exec_stream(machine, poll_in(20, &run_dir), None);
        let launched = factory.launch(&format!("{}; echo done", support::gate(&gate)), launch_input(&[], &["go"]));
        let run = launched.run().expect("a run handle");
        support::wait_for("the input file", 5_000, || std::path::Path::new(&format!("{run}.in")).exists()).await;
        let taken_now = || async {
            let stream = attach_stream(&factory, &run, true, now_ms() as f64).await;
            stream.close_input();
            stream.taken()
        };
        assert_eq!(taken_now().await, Some(vec!["go".to_string()]));
        // A line past the cap, the turn's opening with its images say, then two messages within it.
        {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new().append(true).open(format!("{run}.in")).expect("the input file");
            write!(file, "{}\n{{\"uuid\":\"u1\"}}\n{{\"uuid\":\"u2\"}}\n", "x".repeat(EXEC_CHUNK_BYTES as usize)).unwrap();
        }
        assert_eq!(taken_now().await, Some(vec!["{\"uuid\":\"u1\"}".to_string(), "{\"uuid\":\"u2\"}".to_string()]));
        std::fs::write(&gate, "").unwrap();
        drain(&launched).await;
    });
}

mod machine_exec_stream_keeping_a_key_off_the_machine_s_disk {
    use super::*;

    async fn next_line(it: &mut futures::stream::LocalBoxStream<'static, Result<String, ExecFailure>>) -> Option<String> {
        match it.next().await {
            Some(Ok(line)) => Some(line),
            Some(Err(e)) => panic!("the stream failed: {e:?}"),
            None => None,
        }
    }

    async fn key_reaches_the_environment_and_no_file_holds_it(stdin: bool) {
        let (guest, run_dir, _dir) = LocalGuest::new(stdin, "");
        let machine: Arc<dyn Machine> = guest.clone();
        let key = format!("sk-ant-x-{}", hex8());
        let gate = format!("{}/gate", parent_of(&run_dir));
        let factory = machine_exec_stream(machine, poll_in(20, &run_dir), None);
        let seen = r#"case "$ANTHROPIC_API_KEY" in sk-ant-x-*) echo "seen ${#ANTHROPIC_API_KEY}";; esac"#;
        // A turn as an agent's launch makes one, its prompt on an input channel; an exec as the exec verb makes one.
        let turn = factory.launch(&format!("{seen}; read -r line; echo \"got $line\""), launch_input(&[("ANTHROPIC_API_KEY", &key)], &["hello"]));
        let exec = factory.launch(&format!("{seen}; {}", support::gate(&gate)), launch_env(&[("ANTHROPIC_API_KEY", &key)]));
        let mut turn_lines = turn.lines();
        let mut exec_lines = exec.lines();
        assert_eq!(next_line(&mut turn_lines).await, Some(format!("seen {}", key.len())));
        assert_eq!(next_line(&mut turn_lines).await, Some("got hello".to_string()));
        assert_eq!(next_line(&mut exec_lines).await, Some(format!("seen {}", key.len())));

        // Both are running, the turn on its open channel and the exec on its gate, with every file of theirs on disk.
        assert_eq!(support::holding(&run_dir, &key), Vec::<String>::new());
        // On a computer you own the launch's text is a world-readable command line; a provider's single-login machine
        // keeps it there, and the image keeps it out of the logs.
        if stdin {
            assert_eq!(guest.calls().into_iter().filter(|cmd| cmd.contains(&key)).collect::<Vec<_>>(), Vec::<String>::new());
        }

        std::fs::write(&gate, "").unwrap();
        assert_eq!(next_line(&mut exec_lines).await, None);
        turn.close_input();
        assert_eq!(next_line(&mut turn_lines).await, None);
        assert_eq!((exec.exited().await, turn.exited().await), (Some(0), Some(0)));
        assert_eq!(support::names(&run_dir), Vec::<String>::new());

        // A stream on this road reaps its run only while something reads it.
        for stream in [&turn, &exec] {
            stream.kill();
        }
        while support::within(5_000, turn_lines.next()).await.flatten().is_some() {}
        while support::within(5_000, exec_lines.next()).await.flatten().is_some() {}
    }

    own_group_test!(a_turn_and_an_exec_launched_with_a_key_reach_it_in_their_environment_and_no_file_stdin_machine, 20_000, {
        key_reaches_the_environment_and_no_file_holds_it(true).await;
    });

    own_group_test!(a_turn_and_an_exec_launched_with_a_key_reach_it_in_their_environment_and_no_file_provider_machine, 20_000, {
        key_reaches_the_environment_and_no_file_holds_it(false).await;
    });

    /// A run as a host before this one launched it: the environment as export lines at the top of its script.
    async fn older(machine: &Arc<dyn Machine>, run_dir: &str, key: &str, id: &str, command: &str) -> String {
        let b = support::sq(&format!("{run_dir}/{id}"));
        let script = format!("export ANTHROPIC_API_KEY={}\n{command}\necho $? > {b}.exit\n", support::sq(key));
        let res = machine
            .exec(
                &format!("umask 077; mkdir -p {} {b}.d && printf %s {} > {b}.sh && {{ setsid bash {b}.sh > {b}.log 2>&1 & echo $! > {b}.pid; }}", support::sq(run_dir), support::sq(&script)),
                MachineExec::default(),
            )
            .await
            .expect("the older launch");
        assert_eq!(res.exit_code.0 .0 as i64, 0);
        support::strays::grandchild(&format!("{run_dir}/{id}.pid")).await;
        format!("{run_dir}/{id}")
    }

    own_group_test!(a_script_an_older_host_wrote_with_the_key_in_it_is_gone_once_its_run_ends_read_o, 20_000, {
        let (guest, run_dir, _dir) = LocalGuest::new(false, "");
        let machine: Arc<dyn Machine> = guest.clone();
        let key = format!("sk-ant-x-{}", hex8());
        let gate = format!("{}/gate", parent_of(&run_dir));
        let read = older(&machine, &run_dir, &key, "aaaaaaaaaaaa", &format!("{}; echo done", support::gate(&gate))).await;
        let ended = older(&machine, &run_dir, &key, "bbbbbbbbbbbb", "echo done").await;
        support::wait_for("the ended run's exit file", 5_000, || support::read(&format!("{ended}.exit")).trim() == "0").await;
        let mut holders = support::holding(&run_dir, &key);
        holders.sort();
        assert_eq!(holders, vec![format!("{read}.sh"), format!("{ended}.sh")]);

        // The host that comes next re-opens the run its rows hold and sweeps the one they do not.
        let next = machine_exec_stream(machine.clone(), poll_in(20, &run_dir), None);
        let attached = attach_stream(&next, &read, false, now_ms() as f64).await;
        assert_eq!(next.sweep(&[read.clone()]).expect("this factory sweeps").await.unwrap(), vec![ended.clone()]);
        std::fs::write(&gate, "").unwrap();
        let lines = support::lines_ok(&*attached).await;
        assert_eq!((lines, attached.exited().await), (vec!["done".to_string()], Some(0)));
        assert_eq!(support::names(&run_dir), Vec::<String>::new());
    });
}

mod machine_exec_stream_sweeping_the_runs_a_connecting_host_does_not_hold {
    use super::*;

    own_group_test!(ends_every_claimed_run_but_the_ones_it_is_named_takes_their_files_and_leaves_a_n, 30_000, {
        let (guest, run_dir, _dir) = LocalGuest::new(false, "");
        let machine: Arc<dyn Machine> = guest.clone();
        let factory = machine_exec_stream(machine, poll_in(20, &run_dir), None);
        // Nothing iterates either stream, as a host that went down under its own reader did not.
        let held = factory.launch("sleep 300", launch_env(&[]));
        let orphan = factory.launch("sleep 300", launch_env(&[]));
        let held_run = held.run().expect("a run handle");
        let orphan_run = orphan.run().expect("a run handle");
        let held_pid = support::strays::grandchild(&format!("{held_run}.pid")).await;
        let orphan_pid = support::strays::grandchild(&format!("{orphan_run}.pid")).await;
        assert!(held_pid > 0 && orphan_pid > 0);
        let files_of = |base: &str| -> Vec<String> {
            let stem = std::path::Path::new(base).file_name().unwrap().to_string_lossy().into_owned();
            support::names(&run_dir).into_iter().filter(|name| name.starts_with(&stem)).collect()
        };
        // A claim of a shape this factory could not have minted is the guest's, not a run of ours to end.
        std::fs::create_dir(format!("{run_dir}/notarun.d")).unwrap();

        assert_eq!(factory.sweep(&[held_run.clone()]).expect("this factory sweeps").await.unwrap(), vec![orphan_run.clone()]);

        support::strays::gone(orphan_pid).await;
        assert_eq!(files_of(&orphan_run), Vec::<String>::new());
        assert!(support::strays::alive(held_pid));
        assert!(!files_of(&held_run).is_empty());

        // The same host, holding nothing: the run it was reading is a run nobody reads.
        assert_eq!(factory.sweep(&[]).expect("this factory sweeps").await.unwrap(), vec![held_run.clone()]);
        support::strays::gone(held_pid).await;
        assert_eq!(support::names(&run_dir), vec!["notarun.d".to_string()]);
    });

    #[test]
    fn a_machine_holding_two_hundred_stale_runs_is_swept_a_page_at_a_time_every_page_un() {
        support::run_local(async {
            let stale: Vec<String> = (0..200).map(|i| format!("/tmp/wsp-run/{i:012x}")).collect();
            let calls = Arc::new(Mutex::new(Vec::<String>::new()));
            let (k, listing) = (calls.clone(), format!("{}\n", stale.iter().map(|base| format!("{base}.d")).collect::<Vec<_>>().join("\n")));
            let (_guest, inner) = ScriptGuest::new(vec![]);
            let machine = wrapped(inner, move |_inner, cmd, _opts| {
                let (k, listing) = (k.clone(), listing.clone());
                async move {
                    let out = if cmd.starts_with("for d in ") { listing } else { String::new() };
                    k.lock().unwrap().push(cmd);
                    answered(0, &out, "")
                }
            });

            let swept = machine_exec_stream(machine, MachineExecOptions::default(), None).sweep(&[]).expect("this factory sweeps").await.unwrap();
            assert_eq!(swept, stale);

            let calls = calls.lock().unwrap().clone();
            let reaps = &calls[1..];
            // Measured as the wire body the backend sends, the way every other call this file holds under the cap is.
            for cmd in &calls {
                assert!(solari_body(cmd) <= EXEC_BODY_MAX, "{}", cmd.chars().take(80).collect::<String>());
            }
            assert!(reaps.len() > 1);
            // Every run is reaped once across the pages, and no page carries a run twice.
            let reaped: Vec<String> = stale.iter().filter(|base| reaps.iter().filter(|cmd| cmd.contains(&format!("rm -rf '{base}'.*"))).count() == 1).cloned().collect();
            assert_eq!(reaped, stale);
        });
    }

    own_group_test!(a_machine_holding_no_run_of_this_factory_s_is_swept_without_a_second_word_going, 15_000, {
        let (guest, run_dir, _dir) = LocalGuest::new(false, "");
        let local: Arc<dyn Machine> = guest.clone();
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let k = calls.clone();
        let counted = wrapped(local, move |inner, cmd, opts| {
            let k = k.clone();
            async move {
                k.lock().unwrap().push(cmd.clone());
                inner.exec(&cmd, opts).await
            }
        });
        let swept = machine_exec_stream(counted, MachineExecOptions { run_dir: Some(run_dir.clone()), ..Default::default() }, None).sweep(&[]).expect("this factory sweeps").await.unwrap();
        assert_eq!(swept, Vec::<String>::new());
        assert_eq!(calls.lock().unwrap().len(), 1);
    });
}

mod text_a_run_reads_that_holds_a_value {
    use super::*;

    own_group_test!(lands_in_files_of_the_run_s_own_off_the_launch_s_input_the_run_handed_the_path_a, 15_000, {
        let (guest, run_dir, _dir) = LocalGuest::new(true, "");
        let machine: Arc<dyn Machine> = guest.clone();
        let value = format!("lin_{}", hex8());
        let factory = machine_exec_stream(machine, poll_in(20, &run_dir), None);
        // The mode through ls, since macOS's stat takes no -c.
        let script = r#"ls -l "$WSP_MCP_VALUES" | cut -c1-10; cat "$WSP_MCP_VALUES"; echo; read -r line; echo "got $line"; env | grep -c "^WSP_LAND" || true"#;
        let stream = factory.launch(
            script,
            Launch {
                env: env_of(&[]),
                input: Some(vec![format!("seed {value}")]),
                secret: Some(ExecSecret { files: Some(env_of(&[("WSP_MCP_VALUES", &format!("{{\"token\":\"{value}\"}}"))])), input: true }),
                ..Default::default()
            },
        );
        let mut lines = Vec::new();
        let mut it = stream.lines();
        while let Some(next) = it.next().await {
            let line = next.expect("the stream failed");
            if line == "0" {
                stream.close_input();
            }
            lines.push(line);
        }
        assert_eq!(lines, vec!["-rw-------".to_string(), format!("{{\"token\":\"{value}\"}}"), format!("got seed {value}"), "0".to_string()]);
        assert_eq!(stream.exited().await, Some(0));
        assert_eq!(guest.calls().into_iter().filter(|cmd| cmd.contains(&value)).collect::<Vec<_>>(), Vec::<String>::new());
        assert_eq!(support::names(&run_dir), Vec::<String>::new());
    });

    own_group_test!(is_refused_on_a_machine_whose_launch_takes_no_input_before_anything_is_started, 15_000, {
        let (guest, run_dir, _dir) = LocalGuest::new(false, "");
        let machine: Arc<dyn Machine> = guest.clone();
        let factory = machine_exec_stream(machine, poll_in(20, &run_dir), None);
        let held = || Launch {
            env: env_of(&[]),
            secret: Some(ExecSecret { files: Some(env_of(&[("WSP_MCP_VALUES", "{}")])), input: false }),
            ..Default::default()
        };
        match factory.try_launch("true", held()) {
            Err(failure) => assert!(failure.message.contains("takes no input on its launch"), "{}", failure.message),
            Ok(_) => panic!("the launch was not refused"),
        }
        // The trait's launch cannot fail, so it answers a stream that fails with the same words.
        let stream = factory.launch("true", held());
        let (lines, failure) = support::lines_err(&*stream).await;
        assert!(lines.is_empty());
        assert!(failure.message.contains("takes no input on its launch"), "{}", failure.message);
        assert_eq!(stream.exited().await, None);
        assert_eq!(guest.calls(), Vec::<String>::new());
    });
}
