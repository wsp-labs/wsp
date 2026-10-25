// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/place-machine.test.ts. Spec: spec/place_machine.md.
//!
//! The computer itself as a machine: the frames it sends over the link its daemon holds, and the calls that belong
//! to a workspace and are refused here.
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Map, Value};

mod roads_common;
use roads_common::*;

use wsp_engine::errors::Result;
use wsp_engine::exec_detached::{exec_fits, put_files, GuestWrite, PutFilesOptions, INLINE_EXEC_MS};
use wsp_engine::link_backend::{LinkAsk, LINK_MARGIN_MS};
use wsp_engine::machine::{BytesLanded, ExecOptions, ExecResult, Machine, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::place_machine::{place_part_bound_ms, PlaceFolderMachine, PlaceMachine, NOT_A_WORKSPACE, PLACE_PART_BYTES};
use wsp_wire::wire::daemon::EXEC_TIMEOUT_MAX_MS;
use wsp_wire::wire::limits::EXEC_BODY_MAX;
use wsp_words::agents_report::login_path_refusal;
use wsp_words::project_path::{place_daemon_paths, place_provision_paths};
use wsp_words::thread_cgroup::thread_cgroup;

const HOME: &str = "/root";
const MAX_MS: u64 = EXEC_TIMEOUT_MAX_MS as u64;

/// The link a computer holds, answering the exec frame the way its daemon does: the launch and the polls of a
/// detached run, and exit 0 for everything else.
fn link<F>(answer: F) -> Arc<FakeLink>
where
    F: Fn(&str) -> Option<ExecResult> + 'static,
{
    FakeLink::new(move |sent| {
        let cmd = sent.params.get("cmd").and_then(Value::as_str).unwrap_or("");
        let said = match answer(cmd) {
            Some(said) => said,
            None if cmd.contains("echo WSP_LAUNCHED") => res(0, "WSP_LAUNCHED\n", ""),
            None if cmd.contains("echo WSP_POLL") => res(0, "WSP_POLL\n0\n\n\ndown\nWSP_POLL_END\n", ""),
            None => res(0, "", ""),
        };
        Ok(json!({ "exitCode": said.exit_code, "stdout": said.stdout, "stderr": said.stderr, "truncated": false }))
    })
}

fn plain_link() -> Arc<FakeLink> {
    link(|_| None)
}

fn machine_on(l: &Arc<FakeLink>) -> PlaceMachine {
    PlaceMachine::new(l.clone(), "spoo", HOME)
}

fn cmd_of(s: &Sent) -> String {
    s.params.get("cmd").and_then(Value::as_str).unwrap_or("").to_string()
}

fn as_value(params: &Map<String, Value>) -> Value {
    Value::Object(params.clone())
}

fn ask(timeout_ms: u64, key: Option<&str>) -> LinkAsk {
    LinkAsk { timeout_ms: Some(timeout_ms), idempotency_key: key.map(str::to_string) }
}

fn err_text<T>(r: Result<T>) -> String {
    match r {
        Ok(_) => panic!("expected a failure, got a success"),
        Err(e) => e.to_string(),
    }
}

struct Fed {
    status: Option<i32>,
    stderr: String,
    timed_out: bool,
}

/// A command run with bytes on its stdin, bounded at 15 s as the TS's spawnSyncFed is.
fn run_fed(cmd: &str, stdin: Vec<u8>) -> Fed {
    let mut child =
        Command::new("bash").args(["-c", cmd]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut in_pipe = child.stdin.take().unwrap();
    let feeder = std::thread::spawn(move || {
        let _ = in_pipe.write_all(&stdin);
    });
    let mut out_pipe = child.stdout.take().unwrap();
    let mut err_pipe = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out_pipe.read_to_end(&mut b);
    });
    let err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err_pipe.read_to_end(&mut b);
        b
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status.code();
        }
        if Instant::now() > deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().unwrap().code();
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let _ = feeder.join();
    let _ = out.join();
    let stderr = String::from_utf8_lossy(&err.join().unwrap()).into_owned();
    Fed { status, stderr, timed_out }
}

fn stdin_of(s: &Sent) -> Vec<u8> {
    match s.params.get("stdin").and_then(Value::as_str) {
        Some(text) => BASE64.decode(text).unwrap(),
        None => Vec::new(),
    }
}

#[tokio::test]
async fn rides_the_daemons_own_exec_frame() {
    let l = link(|cmd| (cmd == "echo hi").then(|| res(3, "hi\n", "oops\n")));
    let got = machine_on(&l).exec("echo hi", ExecOptions::default()).await.unwrap();
    assert_eq!(got, res(3, "hi\n", "oops\n"));
    let sent = l.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].op, "exec");
    assert_eq!(as_value(&sent[0].params), json!({ "cmd": "echo hi", "timeoutMs": INLINE_EXEC_MS }));
    assert_eq!(sent[0].ask, ask(INLINE_EXEC_MS + LINK_MARGIN_MS, None));
}

#[tokio::test]
async fn holds_the_wait_under_the_cap_the_daemon_holds_its_own_timer_to() {
    let l = plain_link();
    let opts = ExecOptions { timeout_ms: Some(MAX_MS * 2), idempotency_key: Some("upload/1".to_string()), stdin: None };
    machine_on(&l).exec("sleep 1", opts).await.unwrap();
    let sent = l.sent();
    assert_eq!(sent[0].params["timeoutMs"], json!(MAX_MS));
    assert_eq!(sent[0].ask, ask(MAX_MS + LINK_MARGIN_MS, Some("upload/1")));
}

#[tokio::test]
async fn carries_the_bytes_for_the_commands_own_stdin_on_the_frame() {
    let l = plain_link();
    let stdin = vec![0u8, 1, 2, 250, 251];
    let opts = ExecOptions { stdin: Some(stdin.clone()), ..Default::default() };
    machine_on(&l).exec("cat > /root/x", opts).await.unwrap();
    let sent = l.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].op, "exec");
    assert_eq!(as_value(&sent[0].params), json!({ "cmd": "cat > /root/x", "timeoutMs": INLINE_EXEC_MS, "stdin": BASE64.encode(&stdin) }));
    assert_eq!(sent[0].ask, ask(INLINE_EXEC_MS + LINK_MARGIN_MS, None));
}

#[tokio::test]
async fn is_launched_under_wsps_own_folder_on_that_computer() {
    let l = plain_link();
    let run = RunOptions { deadline_ms: 60_000, poll_ms: Some(1), ..Default::default() };
    let got = machine_on(&l).run("apt-get install -y -qq jq", run).await.unwrap();
    assert_eq!(got.exit_code, 0.into());
    let sent = l.sent();
    let launch = sent.iter().find(|s| cmd_of(s).contains("echo WSP_LAUNCHED")).unwrap();
    assert!(cmd_of(launch).contains(&format!("b={}/", place_provision_paths(HOME).run_dir)));
    assert_eq!(place_provision_paths(HOME).run_dir, "/root/.wsp/provision/run");
}

fn parts_of(l: &FakeLink) -> Vec<Sent> {
    l.sent().into_iter().filter(|s| s.params.get("stdin").is_some()).collect()
}

#[tokio::test]
async fn cross_in_parts_each_one_frame() {
    let l = plain_link();
    let file = vec![7u8; 3 * PLACE_PART_BYTES];
    let landed = machine_on(&l).put_bytes("/root/.wsp/provision/in.tgz", &file, None).await.unwrap();
    let parts = parts_of(&l);
    assert_eq!(parts.len(), 3);
    assert_eq!(landed, Some(BytesLanded { pieces: parts.len() as u32 }));
    for f in &parts {
        assert!(cmd_of(f).len() < 200);
        assert!(exec_fits(&cmd_of(f)));
    }
    let sent = l.sent();
    assert_eq!(sent.len(), parts.len() + 1);
    assert!(cmd_of(sent.last().unwrap()).contains("cat '/root/.wsp/provision/in.tgz'.part{0..2} > '/root/.wsp/provision/in.tgz'"));
}

#[tokio::test]
async fn cross_as_base64_once_not_as_base64_of_base64() {
    let l = plain_link();
    let mut file = vec![1u8; PLACE_PART_BYTES];
    file.extend(vec![2u8; PLACE_PART_BYTES]);
    file.extend(b"tail");
    machine_on(&l).put_bytes("/root/.wsp/provision/in.tgz", &file, None).await.unwrap();
    let crossed: Vec<u8> = parts_of(&l).iter().flat_map(stdin_of).collect();
    assert_eq!(crossed, file);
}

#[tokio::test]
async fn bound_each_frame_by_the_bytes_it_carries() {
    assert_eq!(place_part_bound_ms(PLACE_PART_BYTES, None), INLINE_EXEC_MS);
    assert_eq!(place_part_bound_ms(4 * 1024 * 1024, None), 52_512);
    assert_eq!(place_part_bound_ms(64 * 1024 * 1024, None), MAX_MS);
    let l = plain_link();
    machine_on(&l).put_bytes("/root/.wsp/provision/in.tgz", &vec![7u8; PLACE_PART_BYTES + 1], None).await.unwrap();
    let parts = parts_of(&l);
    let bounds: Vec<Value> = parts.iter().map(|f| f.params["timeoutMs"].clone()).collect();
    assert_eq!(bounds, vec![json!(place_part_bound_ms(PLACE_PART_BYTES, None)), json!(place_part_bound_ms(1, None))]);
    let asks: Vec<LinkAsk> = parts.iter().map(|f| LinkAsk { timeout_ms: f.ask.timeout_ms, idempotency_key: None }).collect();
    assert_eq!(asks, vec![ask(INLINE_EXEC_MS + LINK_MARGIN_MS, None), ask(INLINE_EXEC_MS + LINK_MARGIN_MS, None)]);
}

#[tokio::test]
async fn leave_nothing_of_the_file_on_the_computer_where_a_frame_failed() {
    let l = link(|cmd| cmd.ends_with(".part1").then(|| res(1, "", "no space left on device")));
    let put = machine_on(&l).put_bytes("/root/.wsp/provision/in.tgz", &vec![7u8; 3 * PLACE_PART_BYTES], None).await;
    assert!(err_text(put)
        .contains("part 2 of 3 did not land at /root/.wsp/provision/in.tgz on spoo: it exited 1 and said: no space left on device"));
    let sent = l.sent();
    assert_eq!(cmd_of(sent.last().unwrap()), "rm -f '/root/.wsp/provision/in.tgz'.part{0..2}");
}

#[tokio::test(flavor = "multi_thread")]
async fn land_the_same_bytes_once_where_the_link_lost_an_answer() {
    let l = plain_link();
    let at = tempfile::tempdir().unwrap();
    let path = at.path().join("deep").join("in.tgz");
    let mut file = vec![3u8; PLACE_PART_BYTES];
    file.extend(b"tail");
    machine_on(&l).put_bytes(path.to_str().unwrap(), &file, None).await.unwrap();
    for f in l.sent() {
        for send in [1, 2] {
            let ran = run_fed(&cmd_of(&f), stdin_of(&f));
            assert!(!ran.timed_out, "send {send} of {} did not finish", cmd_of(&f));
            assert_eq!(ran.status, Some(0), "{}", ran.stderr);
        }
    }
    assert_eq!(std::fs::read(&path).unwrap(), file);
    let left: Vec<String> =
        std::fs::read_dir(at.path().join("deep")).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    assert_eq!(left, ["in.tgz"]);
}

fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[tokio::test(flavor = "multi_thread")]
async fn are_its_logins_alone_in_every_part_and_in_the_file() {
    let l = plain_link();
    let at = tempfile::tempdir().unwrap();
    let path = at.path().join(".wsp-config-tmp.0123456789ab");
    let mut file = vec![3u8; PLACE_PART_BYTES];
    file.extend(b"tail");
    machine_on(&l).put_bytes(path.to_str().unwrap(), &file, None).await.unwrap();
    let mut modes: Vec<String> = Vec::new();
    for f in l.sent() {
        let ran = run_fed(&format!("umask 022\n{}", cmd_of(&f)), stdin_of(&f));
        assert_eq!(ran.status, Some(0), "{}", ran.stderr);
        for entry in std::fs::read_dir(at.path()).unwrap() {
            let entry = entry.unwrap();
            modes.push(format!("{} {:o}", entry.file_name().to_str().unwrap(), mode_of(&entry.path())));
        }
    }
    modes.sort();
    modes.dedup();
    assert_eq!(
        modes,
        [".wsp-config-tmp.0123456789ab 600", ".wsp-config-tmp.0123456789ab.part0 600", ".wsp-config-tmp.0123456789ab.part1 600"]
    );
}

#[tokio::test]
async fn are_refused_in_one_sentence_rather_than_sent_as_a_frame() {
    let l = plain_link();
    let machine = machine_on(&l);
    let life = MachineLife { first_life: true };
    let refused = [
        err_text(machine.snapshot("x", life, SnapshotOptions::default()).await),
        err_text(machine.pause(None).await),
        err_text(machine.resume(None).await),
        err_text(machine.kill().await),
        err_text(machine.download_url("/x").await),
        err_text(machine.upload_url("/x").await),
    ];
    for said in refused {
        assert!(said.contains(NOT_A_WORKSPACE), "{said}");
    }
    assert!(l.sent().is_empty());
    assert_eq!(machine.state().await.unwrap(), MachineState::Running);
    assert!(l.sent().is_empty());
}

/// A login PATH of `chars` characters, which every line on the computer carries in front of it.
fn path_of(chars: usize) -> String {
    format!("/opt/{}:/usr/bin:/bin", "a".repeat(chars - "/opt/:/usr/bin:/bin".len()))
}

/// The computer as the daemon's root login reaches it, with that PATH, every upload frame answered as landed.
fn folder_on(chars: usize) -> (Arc<FakeLink>, Arc<PlaceFolderMachine>) {
    let path = path_of(chars);
    let reply = path.clone();
    let l = link(move |cmd| {
        if cmd.starts_with("uname -s") {
            return Some(res(0, &format!("Linux\n0\nroot\nroot\n1\n{HOME}\n{reply}\n"), ""));
        }
        if cmd.contains("-ilc ") {
            return Some(res(1, "", ""));
        }
        cmd.contains("echo WSP_PIECE").then(|| res(0, "WSP_PIECE\n", ""))
    });
    let machine = PlaceFolderMachine::new(l.clone(), "spoo", HOME, Some(&path));
    (l, machine)
}

fn uploads(l: &FakeLink) -> Vec<String> {
    l.sent().iter().map(cmd_of).filter(|cmd| ["WSP_PIECE", "WSP_LAUNCHED", "WSP_OK"].iter().any(|word| cmd.contains(word))).collect()
}

fn text() -> String {
    "x".repeat(20_000)
}

#[derive(Clone, Copy)]
enum Road {
    AMessageWrittenIntoARunningTurn,
    AStepRunForMinutes,
    ATurnsLaunch,
}

async fn go(road: Road, m: &Arc<PlaceFolderMachine>) -> Result<()> {
    let at = place_daemon_paths(HOME);
    match road {
        Road::AMessageWrittenIntoARunningTurn => {
            let files = [GuestWrite { path: format!("{}/0123456789ab.in", at.run_dir), text: text(), append: true }];
            let opts = PutFilesOptions { after: vec!["echo WSP_OK".to_string()], ..Default::default() };
            put_files(m.as_ref(), &files, opts).await.map(|_| ())
        }
        Road::AStepRunForMinutes => {
            let run = RunOptions { deadline_ms: 60_000, poll_ms: Some(1), ..Default::default() };
            m.run(&text(), run).await.map(|_| ())
        }
        Road::ATurnsLaunch => {
            let grouped = m.in_cgroup(&thread_cgroup("thread-1"), Some(&at.guest_bin));
            let files = [GuestWrite { path: format!("{}/0123456789ab.sh", at.run_dir), text: text(), append: false }];
            let opts = PutFilesOptions { after: vec!["echo WSP_LAUNCHED".to_string()], ..Default::default() };
            put_files(grouped.as_ref(), &files, opts).await.map(|_| ())
        }
    }
}

/// The longest command that fits one exec body, read off the rule every road pages against.
fn longest() -> i64 {
    let mut n = EXEC_BODY_MAX as usize;
    while !exec_fits(&"x".repeat(n)) {
        n -= 1;
    }
    n as i64
}

/// The PATH at which a piece of the road's upload has no character left over: read off the first piece it sends with
/// a PATH of 10,000, less its part, the length of the piece number it is measured with standing in for the first's.
async fn zero_room_of(road: Road) -> i64 {
    let (l, machine) = folder_on(10_000);
    go(road, &machine).await.unwrap();
    let first = uploads(&l).into_iter().find(|cmd| cmd.contains("WSP_PIECE")).unwrap();
    let from = first.find("printf %s '").unwrap() + "printf %s '".len();
    let part_len = first[from..].chars().take_while(|c| c.is_ascii_alphanumeric() || "+/=".contains(*c)).count();
    let encoded_len = BASE64.encode(text()).len();
    let measured = first.len() as i64 - part_len as i64 - "0".len() as i64 + encoded_len.to_string().len() as i64;
    10_000 + longest() - measured
}

async fn refuses(road: Road, off: i64) {
    let chars = zero_room_of(road).await + off;
    let (l, machine) = folder_on(chars as usize);
    let said = err_text(go(road, &machine).await);
    assert!(said.contains(&login_path_refusal(chars as f64)), "{said}");
    assert_eq!(uploads(&l), Vec::<String>::new());
}

macro_rules! refusal_at {
    ($name:ident, $road:expr, $off:expr) => {
        #[tokio::test]
        async fn $name() {
            refuses($road, $off).await;
        }
    };
}

refusal_at!(refuses_a_message_written_into_a_running_turn_at_one_under, Road::AMessageWrittenIntoARunningTurn, -1);
refusal_at!(refuses_a_message_written_into_a_running_turn_at_zero_room, Road::AMessageWrittenIntoARunningTurn, 0);
refusal_at!(refuses_a_message_written_into_a_running_turn_at_one_over, Road::AMessageWrittenIntoARunningTurn, 1);
refusal_at!(refuses_a_step_run_for_minutes_at_one_under, Road::AStepRunForMinutes, -1);
refusal_at!(refuses_a_step_run_for_minutes_at_zero_room, Road::AStepRunForMinutes, 0);
refusal_at!(refuses_a_step_run_for_minutes_at_one_over, Road::AStepRunForMinutes, 1);
refusal_at!(refuses_a_turns_launch_at_one_under, Road::ATurnsLaunch, -1);
refusal_at!(refuses_a_turns_launch_at_zero_room, Road::ATurnsLaunch, 0);
refusal_at!(refuses_a_turns_launch_at_one_over, Road::ATurnsLaunch, 1);

#[tokio::test]
async fn names_the_path_where_it_is_merely_too_long() {
    let (l, machine) = folder_on(16_000);
    let at = place_daemon_paths(HOME);
    let files = [GuestWrite { path: format!("{}/0123456789ab.in", at.run_dir), text: "x".to_string(), append: false }];
    let said = err_text(put_files(machine.as_ref(), &files, PutFilesOptions::default()).await);
    assert!(said.contains(&login_path_refusal(16_000.0)), "{said}");
    assert_eq!(uploads(&l), Vec::<String>::new());
}
