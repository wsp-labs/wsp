// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of the child process, the fed spawn, the config write here and the env road. spec/child_exec.md.
#[path = "rest_common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde_json::Value;

use wsp_engine::child_exec::{run_child, ChildOptions, LineFeed, CHILD_TIMED_OUT, CHILD_UNSTARTABLE};
use wsp_engine::config_here::config_landed;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::ExecResult;
use wsp_engine::run_env::{env_exec, env_input, harness_exec, with_env_from_input, ENV_FROM_INPUT};
use wsp_engine::spawn_fed::{spawn_sync_fed, FedOptions};

fn oracle(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle").join(name);
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

fn sh(script: &str) -> (String, Vec<String>) {
    ("/bin/sh".to_string(), vec!["-c".to_string(), script.to_string()])
}

fn alive(pid: i32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists() && !fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default().contains(") Z ")
}

// spec/child_exec.md H7
#[test]
fn line_feed_matches_the_ts_oracle() {
    let want: Vec<String> = oracle("small.json")["lineFeed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let mut lf = LineFeed::new(Some(4));
    let mut got = Vec::new();
    got.extend(lf.feed("ab\ncdéfgh"));
    got.extend(lf.feed("ij\r\nk"));
    got.extend(lf.feed("🎉🎉🎉"));
    got.extend(lf.flush());
    assert_eq!(lf.flush(), None, "a second flush holds nothing");
    assert_eq!(got, want);
    // A cut inside an emoji keeps U+FFFD for the half it lands on (the ruling on #2446), never a dropped character.
    let mut odd = LineFeed::new(Some(3));
    assert_eq!(odd.feed("k\u{1f389}\u{1f389}\u{1f389}"), Vec::<String>::new());
    assert_eq!(odd.flush(), Some("\u{fffd}\u{1f389}".to_string()));
    let mut all = LineFeed::new(None);
    assert_eq!(all.feed("x".repeat(100_000).as_str()), Vec::<String>::new());
    assert_eq!(all.flush().map(|l| l.len()), Some(100_000), "no keep holds the whole line");
}

// spec/child_exec.md H3
#[tokio::test]
async fn on_line_gets_each_line_then_the_partial_one_once() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = lines.clone();
    let (file, args) = sh("printf 'a\\nb\\r\\n'; sleep 0.05; printf 'c'; printf 'err\\n' >&2");
    let res = run_child(
        &file,
        &args,
        ChildOptions { on_line: Some(Arc::new(move |l: &str| sink.lock().unwrap().push(l.to_string()))), ..Default::default() },
    )
    .await;
    assert_eq!(res, ExecResult { exit_code: 0.into(), stdout: "a\nb\r\nc".into(), stderr: "err\n".into() });
    assert_eq!(*lines.lock().unwrap(), vec!["a".to_string(), "b\r".to_string(), "c".to_string()]);
}

// spec/child_exec.md H6
#[tokio::test]
async fn an_unstartable_child_answers_127_with_the_error() {
    let res = run_child("/nonexistent/wsp-no-such-binary", &[], ChildOptions::default()).await;
    assert_eq!(res.exit_code, CHILD_UNSTARTABLE.into());
    assert_eq!(res.stdout, "");
    assert!(res.stderr.ends_with('\n') && res.stderr.len() > 1, "{:?}", res.stderr);
}

// spec/child_exec.md H1, H4: the group dies, not the shell alone, and the answer is 124 with what was read.
#[tokio::test]
async fn a_deadline_kills_the_whole_group_and_answers_124() {
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("pid");
    let (file, args) = sh(&format!("sleep 30 & echo $! > {}; echo started; wait", pidfile.display()));
    let started = Instant::now();
    let res = run_child(&file, &args, ChildOptions { timeout_ms: Some(300), ..Default::default() }).await;
    assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    assert_eq!(res.exit_code, CHILD_TIMED_OUT.into());
    assert_eq!(res.stdout, "started\n");
    let pid: i32 = fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(!alive(pid), "the backgrounded sleep in the group must be killed too");
}

// spec/child_exec.md H5: a grandchild in a session of its own keeps the pipes, and the answer still comes within the cap.
#[tokio::test]
async fn an_exit_answers_within_the_drain_cap_while_a_grandchild_holds_the_pipes() {
    let (file, args) = sh("setsid sh -c 'while :; do echo tick; sleep 0.05; done' & echo done; exit 3");
    let started = Instant::now();
    let res = run_child(&file, &args, ChildOptions::default()).await;
    let took = started.elapsed();
    assert_eq!(res.exit_code, 3.into());
    assert!(res.stdout.starts_with("done\n"), "{:?}", res.stdout);
    assert!(took >= Duration::from_millis(900) && took < Duration::from_millis(2500), "{took:?}");
    let _ = std::process::Command::new("pkill").args(["-f", "while :; do echo tick"]).status();
}

// spec/child_exec.md H2, H1: stdin is written and closed; a given environment is the whole environment.
#[tokio::test]
async fn stdin_is_fed_and_closed_and_a_given_env_is_all_there_is() {
    let (file, args) = sh("cat; echo \"[$FOO][$HOME]\"");
    let env: IndexMap<String, String> =
        [("FOO".to_string(), "bar".to_string()), ("PATH".to_string(), "/usr/bin:/bin".to_string())].into_iter().collect();
    let res = run_child(
        &file,
        &args,
        ChildOptions { stdin: Some(b"in\n".to_vec()), env: Some(env), cwd: Some("/".into()), ..Default::default() },
    )
    .await;
    assert_eq!(res, ExecResult { exit_code: 0.into(), stdout: "in\n[bar][]\n".into(), stderr: String::new() });
    // A child that never reads its stdin and exits is not failed by the broken pipe.
    let (file, args) = sh("exit 0");
    let res = run_child(&file, &args, ChildOptions { stdin: Some(vec![b'x'; 1 << 20]), ..Default::default() }).await;
    assert_eq!(res.exit_code, 0.into());
}

// spec/child_exec.md H8
#[test]
fn the_stdin_file_is_gone_before_the_child_reads_it() {
    let out =
        spawn_sync_fed("/bin/sh", &["-c".to_string(), "ls -l /proc/self/fd/0; cat".to_string()], b"secret bytes", &FedOptions::default());
    assert_eq!(out.status, Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("wsp-stdin-") && text.contains("(deleted)"), "{text}");
    assert!(text.ends_with("secret bytes"), "{text}");
    let leftover: Vec<_> = fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("wsp-stdin-"))
        .collect();
    assert!(leftover.is_empty(), "{leftover:?}");
    let missing = spawn_sync_fed("/nonexistent/wsp-x", &[], b"", &FedOptions::default());
    assert_eq!(missing.status, None);
    assert!(missing.error.is_some());
}

// spec/child_exec.md H9
#[test]
fn config_landed_reads_refusals_and_first_lines() {
    let ok = ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() };
    assert_eq!(config_landed(&ok, "/h/f", None), Ok(()));
    let failed = ExecResult { exit_code: 2.into(), stdout: "out line\n".into(), stderr: "  \nfirst err\nsecond\n".into() };
    let shown = |p: &str| format!("~{p}");
    assert_eq!(
        config_landed(&failed, "/h/f", Some(&shown)),
        Err(EngineError::Other { message: "~/h/f stands as it was: the write did not finish (first err).".into(), code: None })
    );
    let quiet = ExecResult { exit_code: 9.into(), stdout: String::new(), stderr: String::new() };
    assert_eq!(
        config_landed(&quiet, "/h/f", None),
        Err(EngineError::Other { message: "/h/f stands as it was: the write did not finish (exit 9).".into(), code: None })
    );
    let stdout_only = ExecResult { exit_code: 1.into(), stdout: "said on stdout\n".into(), stderr: String::new() };
    assert_eq!(
        config_landed(&stdout_only, "/f", None).unwrap_err().to_string(),
        "/f stands as it was: the write did not finish (said on stdout)."
    );
}

// spec/child_exec.md H11, H12
#[tokio::test]
async fn env_exec_writes_the_ts_lines() {
    let o = oracle("commands.json");
    let vars: IndexMap<String, String> = [("A", "1"), ("B", "x'y\nz")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let pipe = common::FakeMachine::new("m1");
    env_exec(&pipe, "echo $A", &vars, None).await.unwrap();
    env_exec(&pipe, "true", &IndexMap::new(), Some(5)).await.unwrap();
    let ran = pipe.ran();
    assert_eq!(
        ran.iter().map(|r| r.cmd.clone()).collect::<Vec<_>>(),
        o["envExec"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>()
    );
    assert!(ran.iter().all(|r| r.opts.stdin.is_none()));
    assert_eq!(ran[1].opts.timeout_ms, Some(5));
    let mut stdin = common::FakeMachine::new("m2");
    stdin.takes_stdin = true;
    env_exec(&stdin, "run", &vars, Some(7)).await.unwrap();
    assert_eq!(stdin.ran()[0].cmd, "run");
    assert_eq!(stdin.ran()[0].opts.stdin, Some(env_input(&vars)));
    assert_eq!(stdin.ran()[0].opts.timeout_ms, Some(7));
    use base64::Engine;
    let with_empty: IndexMap<String, String> =
        [("A", "1"), ("B", "x'y\nz"), ("", "")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    assert_eq!(env_input(&with_empty), base64::engine::general_purpose::STANDARD.decode(o["envInput"].as_str().unwrap()).unwrap());
    assert_eq!(env_input(&IndexMap::new()), Vec::<u8>::new());
    assert_eq!(with_env_from_input("x"), format!("{ENV_FROM_INPUT}\nx"));
}

// spec/child_exec.md H13
#[tokio::test]
async fn harness_exec_answers_stdout_whatever_the_exit() {
    let m = Arc::new(common::FakeMachine::exec("m1", |_, _| {
        Ok(ExecResult { exit_code: 1.into(), stdout: "out".into(), stderr: "err".into() })
    }));
    let exec = harness_exec(m.clone(), 9_000);
    assert_eq!(exec("ls".into(), None).await.unwrap(), "out");
    let vars: IndexMap<String, String> = [("K".to_string(), "v".to_string())].into_iter().collect();
    assert_eq!(exec("ls".into(), Some(vars)).await.unwrap(), "out");
    let ran = m.ran();
    assert_eq!(ran[0].cmd, "ls");
    assert_eq!(ran[0].opts.timeout_ms, Some(9_000));
    assert_eq!(ran[1].cmd, "printf '%s\\0' 'K=v' | {\nls\n}");
    assert_eq!(ran[1].opts.timeout_ms, Some(9_000));
}

// spec/child_exec.md H14: a child killed by a signal answers -1, never 128 + the signal, with what it wrote kept.
#[tokio::test]
async fn a_child_killed_by_a_signal_answers_minus_one_with_its_output() {
    let want = &oracle("strengthen_child.json")["signalled"];
    let (file, args) = sh("echo hi; kill -9 $$");
    let res = run_child(&file, &args, ChildOptions::default()).await;
    assert_eq!(res.exit_code, want["exitCode"].as_i64().unwrap().into());
    assert_eq!(res, ExecResult { exit_code: (-1).into(), stdout: "hi\n".into(), stderr: String::new() });
}

// spec/child_exec.md H15: after the exit, a chunk on either pipe restarts the 200 ms quiet, so what something it left
// running writes within those windows is in the answer, stdout and stderr alike, and on_line hears it before the flush.
#[tokio::test]
async fn a_write_on_either_pipe_after_the_exit_restarts_the_quiet_and_is_in_the_answer() {
    let want = &oracle("strengthen_child.json")["drained"];
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = lines.clone();
    let (file, args) = sh("(sleep 0.1; echo a; sleep 0.15; echo b >&2; sleep 0.15; echo c; sleep 3) & exit 4");
    let started = Instant::now();
    let res = run_child(
        &file,
        &args,
        ChildOptions { on_line: Some(Arc::new(move |l: &str| sink.lock().unwrap().push(l.to_string()))), ..Default::default() },
    )
    .await;
    assert!(started.elapsed() >= Duration::from_millis(500), "{:?}", started.elapsed());
    assert_eq!(res.exit_code, want["exitCode"].as_i64().unwrap().into());
    assert_eq!(res.stdout, want["stdout"].as_str().unwrap());
    assert_eq!(res.stderr, want["stderr"].as_str().unwrap());
    assert_eq!(serde_json::json!(*lines.lock().unwrap()), want["lines"]);
}

// spec/child_exec.md H16: a child whose pipes close with its exit answers then, never after a quiet window of its own:
// ten such children take well under the 2 s ten waits of DRAIN_MS would (the TS took 26 ms).
#[tokio::test]
async fn an_exit_whose_pipes_close_answers_at_once() {
    let started = Instant::now();
    for _ in 0..10 {
        let (file, args) = sh("exit 5");
        assert_eq!(run_child(&file, &args, ChildOptions::default()).await.exit_code, 5.into());
    }
    assert!(started.elapsed() < Duration::from_millis(1500), "{:?}", started.elapsed());
}
