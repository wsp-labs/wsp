// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/engine/test/child-exec.test.ts, spawn-fed.test.ts and run-env.test.ts.
#[path = "rest_common/mod.rs"]
mod common;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use indexmap::IndexMap;

use common::FakeMachine;
use wsp_engine::child_exec::{run_child, ChildOptions, CHILD_TIMED_OUT};
use wsp_engine::machine::{ExecResult, Machine};
use wsp_engine::run_env::{env_exec, harness_exec, ENV_FROM_INPUT};
use wsp_engine::spawn_fed::{spawn_sync_fed, FedOptions};

// ---- child-exec

/// A case's folder; the sleep it started, by the pid it wrote down, is killed whether the case passed or not.
struct Holder {
    dir: tempfile::TempDir,
    at: PathBuf,
}

impl Holder {
    fn pid_file(&self) -> PathBuf {
        self.dir.path().join("sleep.pid")
    }
    fn pid(&self) -> u32 {
        std::fs::read_to_string(self.pid_file()).unwrap().trim().parse().unwrap()
    }
    fn at(&self) -> &str {
        self.at.to_str().unwrap()
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        if let Ok(text) = std::fs::read_to_string(self.pid_file()) {
            if let Ok(pid) = text.trim().parse::<i32>() {
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
        }
    }
}

/// A live process that is not a zombie waiting for a parent to reap it.
fn running(pid: u32) -> bool {
    let out = Command::new("ps").args(["-o", "stat=", "-p", &pid.to_string()]).output().unwrap();
    out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().starts_with('Z')
}

/// A command that starts `child` in the background holding its stdout, writes that child's pid where the case and the
/// cleanup read it, says started, then runs `then`.
fn command(child: &str, then: &str) -> Holder {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("sleep.pid");
    let at = dir.path().join("holder");
    let script = format!("#!/bin/sh\n{child} &\necho $! > {:?}\necho started\n{then}\n", pid_file.to_str().unwrap());
    std::fs::write(&at, script).unwrap();
    std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).unwrap();
    Holder { dir, at }
}

/// A child that writes a line every 50 ms for 20 s, from one process.
const WRITER: &str = r#"perl -e '$| = 1; for (1 .. 400) { print "tick\n"; select(undef, undef, undef, 0.05) }'"#;

/// A command that starts a sleep holding its stdout, then waits on it or exits.
fn holder(wait: bool) -> Holder {
    command("sleep 30", if wait { "wait" } else { "exit 0" })
}

// TS: packages/engine/test/child-exec.test.ts:50
#[tokio::test]
async fn returns_at_its_deadline_with_the_commands_own_children_gone_the_output_so_far_kept() {
    let h = holder(true);
    let result = run_child(h.at(), &[], ChildOptions { timeout_ms: Some(500), ..Default::default() }).await;
    assert_eq!(result.exit_code, CHILD_TIMED_OUT);
    assert_eq!(result.stdout, "started\n");
    let sleep = h.pid();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!running(sleep), "sleep {sleep} still runs");
}

// TS: packages/engine/test/child-exec.test.ts:60
#[tokio::test]
async fn answers_when_the_command_exits_though_a_child_it_left_behind_still_holds_its_output() {
    let h = holder(false);
    let result = run_child(h.at(), &[], ChildOptions::default()).await;
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "started\n");
    assert!(running(h.pid()));
}

// TS: packages/engine/test/child-exec.test.ts:67
#[tokio::test]
async fn answers_a_command_that_exited_0_before_its_deadline_with_0_though_what_it_left_outlives_the_deadline() {
    let h = command(WRITER, "exit 0");
    let result = run_child(h.at(), &[], ChildOptions { timeout_ms: Some(2_000), ..Default::default() }).await;
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("started\n"), "stdout: {:?}", result.stdout);
}

// TS: packages/engine/test/child-exec.test.ts:74
#[tokio::test]
async fn answers_a_command_whose_child_keeps_writing_within_a_second_of_the_commands_exit() {
    let h = command(WRITER, "exit 0");
    let result = run_child(h.at(), &[], ChildOptions::default()).await;
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("started\n"), "stdout: {:?}", result.stdout);
    // The writer prints 400 ticks over 20 s; an answer that waited for it to end holds them all.
    let ticks = result.stdout.lines().filter(|l| *l == "tick").count();
    assert!(ticks < 400, "the answer waited for the writer to end");
}

// ---- spawn-fed

// TS: packages/engine/test/spawn-fed.test.ts:6
#[test]
fn reads_every_byte_and_their_end_from_a_file_never_from_a_pipe_whose_end_can_be_lost() {
    let bytes = vec![5u8; 1024 * 1024];
    let ran = spawn_sync_fed(
        "sh",
        &["-c".to_string(), "[ -f /dev/stdin ] && echo file; wc -c | tr -d ' '".to_string()],
        &bytes,
        &FedOptions { timeout: Some(Duration::from_millis(15_000)), ..Default::default() },
    );
    assert_eq!(ran.error, None);
    assert_eq!(String::from_utf8(ran.stdout).unwrap(), format!("file\n{}\n", bytes.len()));
}

// TS: packages/engine/test/spawn-fed.test.ts:13
#[test]
fn gives_the_child_nothing_to_read_where_the_bytes_are_empty() {
    let ran = spawn_sync_fed("cat", &[], &[], &FedOptions { timeout: Some(Duration::from_millis(15_000)), ..Default::default() });
    assert_eq!(ran.status, Some(0));
    assert_eq!(ran.stdout, Vec::<u8>::new());
}

// ---- run-env

/// This computer's bash as a machine's exec, every command's text kept; `stdin` hands it the input, as a computer you
/// own does, and without it the input is dropped, as a provider's exec drops it.
fn bash_machine(stdin: bool) -> FakeMachine {
    let mut m = FakeMachine::exec("m1", move |cmd, opts| {
        let mut child = Command::new("bash")
            .args(["-c", cmd])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("CLAUDE_CODE_ENTRYPOINT", "cli")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        if stdin {
            if let Some(bytes) = &opts.stdin {
                let _ = input.write_all(bytes);
            }
        }
        drop(input);
        let out = child.wait_with_output().unwrap();
        Ok(ExecResult {
            exit_code: if out.status.success() { 0 } else { 1 },
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    });
    m.takes_stdin = stdin;
    m
}

/// Inherited marks are dropped first and the command's own read after, so a variable of that family it was handed
/// stands, which a read ahead of the command would lose to the command's own unset.
fn read_command() -> String {
    let read = r#"printf '%s|%s|%s\n' "$GATEWAY_TOKEN" "$CLAUDE_CODE_AUTO_CONNECT_IDE" "${CLAUDE_CODE_ENTRYPOINT-gone}""#;
    format!("unset ${{!CLAUDE_CODE_@}}; {ENV_FROM_INPUT}; {read}")
}

fn token() -> String {
    format!("tok-{}", hex::encode(rand::random::<[u8; 6]>()))
}

async fn gets_them(stdin: bool) {
    let key = token();
    let value = "it's a \"value\"\nover two lines";
    let machine = bash_machine(stdin);
    let vars: IndexMap<String, String> = [
        ("GATEWAY_TOKEN".to_string(), key.clone()),
        ("CLAUDE_CODE_AUTO_CONNECT_IDE".to_string(), "0".to_string()),
        ("ODD".to_string(), value.to_string()),
    ]
    .into_iter()
    .collect();
    let res = env_exec(&machine, &read_command(), &vars, None).await.unwrap();
    assert_eq!(res.stdout, format!("{key}|0|gone\n"));
    if stdin {
        let holding: Vec<String> = machine.cmds().into_iter().filter(|t| t.contains(&key)).collect();
        assert_eq!(holding, Vec::<String>::new());
    }
}

// TS: packages/engine/test/run-env.test.ts:30
#[tokio::test]
async fn gets_them_on_the_execs_input_where_the_machine_takes_it_and_no_exec_text_holds_the_value() {
    gets_them(true).await;
}

// TS: packages/engine/test/run-env.test.ts:30
#[tokio::test]
async fn gets_them_piped_in_from_the_execs_text_on_a_machine_whose_exec_drops_its_input() {
    gets_them(false).await;
}

// TS: packages/engine/test/run-env.test.ts:42
#[tokio::test]
async fn reads_nothing_and_waits_on_nothing_when_it_is_handed_none_on_either_road() {
    for stdin in [true, false] {
        let machine: Arc<dyn Machine> = Arc::new(bash_machine(stdin));
        let out = harness_exec(machine, 5_000)(read_command(), Some(IndexMap::new())).await.unwrap();
        assert_eq!(out.trim(), "||gone");
    }
}
