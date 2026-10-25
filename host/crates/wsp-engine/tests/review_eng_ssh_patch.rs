// SPDX-License-Identifier: AGPL-3.0-only
//! Ticket 2446, review of the ssh backend, the link backend and the place machine: the tests that fail on the current
//! code, one per finding (findings 1 to 13). Each asserts what the TS answered, written inline with the TS file:line
//! and the probe that printed it. Nothing outside the crate is read or written: homes, control folders and the stand-in
//! `ssh` live in temporary folders the tests make.
#![allow(dead_code)]

use std::future::Future;
use std::os::unix::fs::PermissionsExt;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::FutureExt;
use serde_json::{json, Map, Value};
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::EngineError;
use wsp_engine::link_backend::{LinkAsk, LinkBackend, LinkMachine, MachineLink};
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend};
use wsp_engine::ssh_backend::{
    hold_back_forward, local_run, make_ssh_control_dir, read_ssh_machine, spawn_ssh, ssh_client, ssh_control_dir, ssh_control_path,
    ssh_machine_id, SshCarried, SshReach, SshSpawn, SshTransportOptions,
};
use wsp_wire::types::check::parse as wire_parse;
use wsp_wire::wire::machine_link::{BackendFacts, MachineHandle, MachineSpec};

/// Tests that set the environment, the home, the PATH or the descriptor limit hold this, one at a time.
static ENV: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    ENV.lock().unwrap_or_else(|p| p.into_inner())
}

fn carried() -> SshCarried {
    SshCarried { reach: SshReach { user: "root".into(), host: "box".into(), port: 22, key_path: None }, options: vec![] }
}

fn reach() -> SshReach {
    SshReach { user: "root".into(), host: "box".into(), port: 22, key_path: None }
}

async fn on_local_set<T>(body: impl Future<Output = T>) -> T {
    tokio::task::LocalSet::new().run_until(body).await
}

/// The real `spawn_ssh` (tokio pipes, the real exit reading), running `sh -c script` where the holder asks for ssh.
fn real_spawn(script: String) -> SshSpawn {
    let real = spawn_ssh();
    Arc::new(move |_file: &str, _args: &[String]| real("sh", &["-c".to_string(), script.clone()]))
}

async fn settles<T>(ms: u64, fut: impl Future<Output = T>) -> Option<T> {
    tokio::time::timeout(Duration::from_millis(ms), fut).await.ok()
}

fn open_fds() -> usize {
    std::fs::read_dir("/proc/self/fd").map(|d| d.count()).unwrap_or(0)
}

/// A stand-in `ssh` that runs the remote command as sshd does: the words after `user@host` joined by spaces, under `sh -c`.
fn write_stand_in_ssh(dir: &std::path::Path) {
    let script = "#!/bin/sh\nwhile [ $# -gt 0 ]; do\n  case \"$1\" in\n    -n|-T) shift;;\n    -o|-p|-i|-F|-J|-W|-R) shift 2;;\n    -*) shift;;\n    *) break;;\n  esac\ndone\nshift\nexec sh -c \"$*\"\n";
    let path = dir.join("ssh");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Sets `name` for the length of one test and gives it back as it was.
struct EnvVar {
    name: &'static str,
    was: Option<std::ffi::OsString>,
}

impl EnvVar {
    fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let was = std::env::var_os(name);
        std::env::set_var(name, value);
        EnvVar { name, was }
    }

    fn unset(name: &'static str) -> Self {
        let was = std::env::var_os(name);
        std::env::remove_var(name);
        EnvVar { name, was }
    }
}

impl Drop for EnvVar {
    fn drop(&mut self) {
        match &self.was {
            Some(v) => std::env::set_var(self.name, v),
            None => std::env::remove_var(self.name),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 1. The held forward settles on the child's exit, with a helper still holding its pipes.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:346 (`child.once("exit", ...)`), printed by held.probe.ts:
///   HELD grandchild keeps stderr open, parent exits 255: up=rejected: boom ended="boom" after=6ms
#[tokio::test]
async fn f1a_ended_answers_when_the_child_exits_though_a_helper_it_left_still_holds_its_stderr() {
    let _g = lock();
    on_local_set(async {
        let held = hold_back_forward(&carried(), 4640, 4640, real_spawn("sleep 4 & echo boom >&2; exit 255".to_string()));
        let up = settles(1500, held.up()).await;
        assert!(up == Some(Err("boom".to_string())), "up after the child exited: TS Some(Err(\"boom\")), Rust {up:?}");
        let ended = settles(1500, held.ended()).await;
        assert!(ended == Some("boom".to_string()), "ended after the child exited: TS Some(\"boom\"), Rust {ended:?}");
    })
    .await;
}

/// The same with the helper holding stdout. TS ssh-backend.ts:346, held.probe.ts: a child that wrote WSP_BACK_UP and
/// exited 255 reads up=up ended="ssh exited with 255" at once, whoever holds a pipe.
#[tokio::test]
async fn f1b_ended_answers_when_the_child_exits_though_a_helper_holds_its_stdout() {
    let _g = lock();
    on_local_set(async {
        let held = hold_back_forward(
            &carried(),
            4640,
            4640,
            real_spawn("(sleep 4 2>/dev/null) 2>&- & echo WSP_BACK_UP; exit 255".to_string()),
        );
        assert_eq!(settles(1500, held.up()).await, Some(Ok(())));
        let ended = settles(1500, held.ended()).await;
        assert!(ended == Some("ssh exited with 255".to_string()), "ended after the child exited: TS Some(\"ssh exited with 255\"), Rust {ended:?}");
    })
    .await;
}

// ---------------------------------------------------------------------------------------------------------------
// 2. A cancelled resume leaves the link's request unfinished.
// ---------------------------------------------------------------------------------------------------------------

/// A link whose request takes one scheduling turn before it sends, then behaves as asked; the log says how far each
/// request got.
struct TimedLink {
    behaviour: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

#[async_trait(?Send)]
impl MachineLink for TimedLink {
    async fn request(&self, op: &str, _params: Map<String, Value>, _ask: LinkAsk) -> Result<Map<String, Value>, EngineError> {
        self.log.lock().unwrap().push(format!("req:{op}"));
        tokio::task::yield_now().await;
        self.log.lock().unwrap().push("sent".to_string());
        if self.behaviour == "never" {
            futures::future::pending::<()>().await;
        }
        if self.behaviour == "instant" {
            self.log.lock().unwrap().push("answered".to_string());
            return Ok(Map::new());
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
        if self.behaviour == "fails" {
            self.log.lock().unwrap().push("failed".to_string());
            return Err(EngineError::Other { message: "link dropped".into(), code: None });
        }
        self.log.lock().unwrap().push("answered".to_string());
        Ok(Map::new())
    }

    async fn forward(&self, _place_port: u16) -> Result<u16, EngineError> {
        Ok(1)
    }
}

fn handle_c1() -> MachineHandle {
    wire_parse::<MachineHandle>(&json!({ "id": "c1", "kind": "sandbox",
        "roads": { "previewUrl": true, "daemonAnswers": true, "putBytes": true, "describe": true, "facts": true, "metrics": true } }))
    .unwrap()
}

/// TS link-backend.ts:181-194 (the request keeps running after the abort wins the race), printed by frames.probe.ts:
///   RESUME slow during_none => err: This operation was aborted ["req:machine.resume","sent","answered"]
/// Each row: link behaviour, when and how the cancel came, the words resume answered (Rust matches these), and the
/// log of what the link had done 100 ms later in the TS (Rust stops short of it).
#[tokio::test]
async fn f2_a_cancelled_resume_still_lets_the_link_finish_the_frame() {
    let table: [(&'static str, &str, &str, &[&str]); 6] = [
        ("instant", "before_none", "err: This operation was aborted", &["req:machine.resume", "sent", "answered"]),
        ("slow", "before_error", "err: stopped by the caller", &["req:machine.resume", "sent", "answered"]),
        ("slow", "during_string", "err: plain words", &["req:machine.resume", "sent", "answered"]),
        ("fails", "before_string", "err: plain words", &["req:machine.resume", "sent", "failed"]),
        ("fails", "during_none", "err: This operation was aborted", &["req:machine.resume", "sent", "failed"]),
        ("never", "before_none", "err: This operation was aborted", &["req:machine.resume", "sent"]),
    ];
    let mut wrong = Vec::new();
    for (behaviour, abort, want_said, want_log) in table {
        let log = Arc::new(Mutex::new(Vec::new()));
        let link: Arc<dyn MachineLink> = Arc::new(TimedLink { behaviour, log: log.clone() });
        let machine = LinkMachine::new(link, handle_c1());
        let cancel = Cancel::new();
        let reason = if abort.ends_with("none") { None } else if abort.ends_with("error") { Some("stopped by the caller") } else { Some("plain words") };
        if abort.starts_with("before") {
            cancel.cancel(reason);
        }
        let c2 = cancel.clone();
        let canceller = async move {
            if abort.starts_with("during") {
                tokio::time::sleep(Duration::from_millis(5)).await;
                c2.cancel(reason);
            }
            futures::future::pending::<()>().await;
        };
        let said = tokio::select! {
            r = machine.resume(Some(&cancel)) => match r { Ok(()) => "ok".to_string(), Err(e) => format!("err: {e}") },
            () = canceller => unreachable!(),
            () = tokio::time::sleep(Duration::from_millis(150)) => "pending".to_string(),
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
        let got = log.lock().unwrap().clone();
        if said != want_said || got != want_log {
            wrong.push(format!("link {behaviour}, cancel {abort}: TS {want_log:?}, Rust {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{} of {} scenes leave the link's request unfinished; first: {}", wrong.len(), table.len(), wrong[0]);
}

// ---------------------------------------------------------------------------------------------------------------
// 3. A process environment value that is not UTF-8 panics `env::vars()`.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:369-373 (`env: process.env`), printed by badenv.js on Node 22: `X is "caf\u{fffd}"`, the child
/// sees the bytes 357 277 275, and sshClient answered {"exitCode":0,"stdout":"x\n","stderr":""}. dial.rs:782 panics.
#[tokio::test]
async fn f3a_an_environment_value_that_is_not_utf8_does_not_stop_an_ssh_command() {
    use std::os::unix::ffi::OsStringExt;
    let _g = lock();
    let dir = tempfile::tempdir().unwrap();
    write_stand_in_ssh(dir.path());
    let home = tempfile::tempdir().unwrap();
    let path = std::env::var("PATH").unwrap();
    let _path = EnvVar::set("PATH", format!("{}:{path}", dir.path().display()));
    let _home = EnvVar::set("HOME", home.path());
    let _bad = EnvVar::set("WSP_REVIEW_BAD", std::ffi::OsString::from_vec(b"caf\xe9".to_vec()));
    let out = AssertUnwindSafe(ssh_client()(reach(), "echo x".to_string(), SshTransportOptions::default())).catch_unwind().await;
    let out = out.unwrap_or_else(|_| panic!("ssh_client panicked on an environment value that is not UTF-8; the TS answered exit 0, stdout \"x\\n\""));
    let out = out.expect("the TS client carried the command");
    assert_eq!((out.exit_code.get(), out.stdout.as_str()), (0, "x\n"));
}

/// TS ssh-backend.ts:386 (`runChild(file, args, { env: process.env, timeoutMs })`), same Node behaviour as above.
/// keys.rs:33 panics.
#[tokio::test]
async fn f3b_a_local_read_runs_with_an_environment_value_that_is_not_utf8() {
    use std::os::unix::ffi::OsStringExt;
    let _g = lock();
    let _bad = EnvVar::set("WSP_REVIEW_BAD", std::ffi::OsString::from_vec(b"caf\xe9".to_vec()));
    let out = AssertUnwindSafe(local_run()("true".to_string(), Vec::new(), 2000)).catch_unwind().await;
    let out = out.unwrap_or_else(|_| panic!("local_run panicked on an environment value that is not UTF-8; the TS ran the command"));
    assert_eq!(out.exit_code.get(), 0);
}

// ---------------------------------------------------------------------------------------------------------------
// 4. The holder needs a LocalSet.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:323 (`holdBackForward` has no context requirement): called from a plain runtime it holds the
/// child and `up` answers. dial.rs:745 calls `tokio::task::spawn_local`, which panics outside a LocalSet.
#[tokio::test]
async fn f4_the_holder_starts_from_a_plain_runtime_as_the_ts_holder_starts_from_anywhere() {
    let _g = lock();
    let held = hold_back_forward(&carried(), 4640, 4640, real_spawn("echo WSP_BACK_UP; exec cat".to_string()));
    assert_eq!(settles(3000, held.up()).await, Some(Ok(())));
    held.release();
}

// ---------------------------------------------------------------------------------------------------------------
// 5. Signal names.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:346 (`ssh ended on ${signal}`, Node's signal name), printed by held.probe.ts and sig.probe.ts:
///   HELD self-kill SIGUSR1: up=rejected: ssh ended on SIGUSR1 ended="ssh ended on SIGUSR1"
/// The table is the signals where dial.rs:517 `signal_name` answers `SIG<n>` instead.
#[tokio::test]
async fn f5_a_child_ended_by_a_signal_is_named_as_node_names_it() {
    let _g = lock();
    let table: [(i32, &str); 15] = [
        (4, "SIGILL"),
        (5, "SIGTRAP"),
        (7, "SIGBUS"),
        (8, "SIGFPE"),
        (10, "SIGUSR1"),
        (12, "SIGUSR2"),
        (14, "SIGALRM"),
        (16, "SIGSTKFLT"),
        (24, "SIGXCPU"),
        (25, "SIGXFSZ"),
        (26, "SIGVTALRM"),
        (27, "SIGPROF"),
        (29, "SIGIO"),
        (30, "SIGPWR"),
        (31, "SIGSYS"),
    ];
    let mut wrong = Vec::new();
    for (n, name) in table {
        let said = on_local_set(async {
            let held = hold_back_forward(&carried(), 4640, 4640, real_spawn(format!("kill -{n} $$")));
            settles(3000, held.ended()).await
        })
        .await;
        let want = format!("ssh ended on {name}");
        if said.as_deref() != Some(want.as_str()) {
            wrong.push(format!("signal {n}: TS says {want:?}, Rust says {said:?}"));
        }
    }
    assert!(wrong.is_empty(), "{} of {} signals are named differently; first: {}", wrong.len(), table.len(), wrong[0]);
}

// ---------------------------------------------------------------------------------------------------------------
// 6. A control folder that cannot be made.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:115-119, 369 (`mkdirSync` throws out of `sshClient`), printed by client.probe.ts:
///   ENOTDIR: not a directory, mkdir '<home>/.wsp/ssh'
/// for a home that is a regular file. dial.rs:765 answers exit 127 and Rust's words, which readers wrap as a failure of the box.
#[tokio::test]
async fn f6_a_control_folder_that_cannot_be_made_throws_node_s_error() {
    let _g = lock();
    let dir = tempfile::tempdir().unwrap();
    let file_home = dir.path().join("home-is-a-file");
    std::fs::write(&file_home, "x").unwrap();
    let _home = EnvVar::set("HOME", &file_home);
    let out = read_ssh_machine(&reach(), &ssh_client()).await;
    let err = out.err().map(|e| e.to_string());
    let want = format!("ENOTDIR: not a directory, mkdir '{}/.wsp/ssh'", file_home.display());
    assert!(err.as_deref() == Some(want.as_str()), "TS threw {want:?}, Rust read answered {err:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// 7. An empty $HOME.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:106 (`home = homedir()`), printed by dir.probe.ts with HOME empty:
///   homes.empty ".wsp/ssh", empty_path ".wsp/ssh/wsp-ssh-27c1671024c60a66.sock" (Node 22.23: `HOME= node -e` prints "")
/// dial.rs:164 sends an empty $HOME to the passwd entry instead.
#[test]
fn f7_an_empty_home_variable_is_the_home_it_says_as_node_reads_it() {
    let _g = lock();
    let _home = EnvVar::set("HOME", "");
    let dir = ssh_control_dir(None).to_string_lossy().into_owned();
    let path = ssh_control_path(&SshReach { user: "u".into(), host: "h".into(), port: 22, key_path: None }, None);
    assert!(
        dir == ".wsp/ssh" && path == ".wsp/ssh/wsp-ssh-27c1671024c60a66.sock",
        "TS \".wsp/ssh\" and \".wsp/ssh/wsp-ssh-27c1671024c60a66.sock\", Rust {dir:?} and {path:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 8. The control folder is `join`ed and normalized.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:106 (`join(home, ".wsp", "ssh")` normalizes), printed by dir2.probe.ts. The table is the homes
/// where dial.rs:173 (`PathBuf::join`, which keeps `..`, `.` and `//`) answers differently.
#[test]
fn f8_the_control_folder_is_the_joined_and_normalized_path_the_ts_names() {
    let table = [
        ("/a/../b", "/b/.wsp/ssh"),
        ("a/./b", "a/b/.wsp/ssh"),
        ("./rel", "rel/.wsp/ssh"),
        ("//x//y", "/x/y/.wsp/ssh"),
        ("/..", "/.wsp/ssh"),
        ("/a/b/../../..", "/.wsp/ssh"),
    ];
    let wrong: Vec<String> = table
        .iter()
        .filter_map(|(home, want)| {
            let got = ssh_control_dir(Some(home)).to_string_lossy().into_owned();
            (got != *want).then(|| format!("home {home:?}: TS {want:?}, Rust {got:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{} of {} homes differ; first: {}", wrong.len(), table.len(), wrong.first().cloned().unwrap_or_default());
}

/// The same finding's other face: `make_ssh_control_dir` creates `x` for `H/x/../y`, where the TS makes only `H/y/.wsp/ssh`
/// (dir.probe.ts printed `dotdot_home ... "ok H/y/.wsp/ssh"` with no `x` made).
#[test]
fn f8b_a_home_with_dot_dot_makes_only_the_normalized_folder() {
    let base = tempfile::tempdir().unwrap();
    let home = format!("{}/x/../y", base.path().display());
    let made = make_ssh_control_dir(Some(&home)).map(|p| p.to_string_lossy().into_owned());
    let want = format!("{}/y/.wsp/ssh", base.path().display());
    let stray = base.path().join("x").exists();
    assert!(made.as_deref().ok() == Some(want.as_str()) && !stray, "TS made {want:?} and no `x`, Rust made {made:?} and `x` exists: {stray}");
}

// ---------------------------------------------------------------------------------------------------------------
// 9. A machine id for a host with a query mark and a key.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:43-46 (`url.searchParams.set("key", ...)` re-reads the host's own `?b:22` as a form pair), printed
/// by pure.probe.ts for host "a?b", port 22, key "/k":  ssh://root@a?b%3A22=&key=%2Fk
#[test]
fn f9_a_machine_id_whose_host_carries_a_query_is_written_as_the_ts_writes_it() {
    let r = SshReach { user: "root".into(), host: "a?b".into(), port: 22, key_path: Some("/k".into()) };
    let got = ssh_machine_id(&r).unwrap();
    assert!(got == "ssh://root@a?b%3A22=&key=%2Fk", "TS \"ssh://root@a?b%3A22=&key=%2Fk\", Rust {got:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// 10. A dropped holder.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:349-358 (only `release()` acts; dropping the object does nothing), printed by drop.probe.ts:
///   DROP pid 1002273 alive 500ms after the holder was dropped: true
/// dial.rs drops the child's stdin with the holder's state, which ends the forward's `cat`.
#[tokio::test]
async fn f10_a_dropped_holder_leaves_the_child_running_as_the_ts_does() {
    let _g = lock();
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("pid");
    let script = format!("echo $$ > {}; echo WSP_BACK_UP; exec cat", pidfile.display());
    let pid: i32 = on_local_set(async {
        let held = hold_back_forward(&carried(), 4640, 4640, real_spawn(script));
        assert_eq!(settles(3000, held.up()).await, Some(Ok(())));
        let pid: i32 = std::fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
        drop(held);
        tokio::time::sleep(Duration::from_millis(500)).await;
        pid
    })
    .await;
    let state = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|t| t.rsplit(')').next().map(|r| r.trim_start().chars().next().unwrap_or('?')));
    let alive = matches!(state, Some(c) if c != 'Z');
    if alive {
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    assert!(alive, "the child (pid {pid}) is gone half a second after the holder was dropped; the TS child runs on");
}

// ---------------------------------------------------------------------------------------------------------------
// 11. Spawn errors Node names by code.
// ---------------------------------------------------------------------------------------------------------------

/// TS ssh-backend.ts:347 (`child.once("error", error => end(error.message))`): Node's message is `spawn <file> <CODE>`
/// for EACCES, EAGAIN, EMFILE, ENFILE and ENOENT. Printed with `ulimit -n 24` on Node 22: `ERR 24 21 spawn /bin/sh EMFILE`.
/// dial.rs:456 names ENOENT and EACCES (those match) and writes Rust's text for the others. The table is the case that differs.
#[tokio::test]
async fn f11_a_child_that_cannot_be_started_says_node_s_spawn_line() {
    let _g = lock();
    let real = spawn_ssh();
    let table = [("/bin/sh", "spawn /bin/sh EMFILE")];
    let mut wrong = Vec::new();
    for (file, want) in table {
        let said = on_local_set(async {
            let real = real.clone();
            let spawn: SshSpawn = Arc::new(move |_: &str, args: &[String]| real(file, args));
            let mut old = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
            unsafe {
                libc::getrlimit(libc::RLIMIT_NOFILE, &mut old);
                let n = open_fds() as u64 + 3;
                libc::setrlimit(libc::RLIMIT_NOFILE, &libc::rlimit { rlim_cur: n.min(old.rlim_max), rlim_max: old.rlim_max });
            }
            let held = hold_back_forward(&carried(), 4640, 4640, spawn);
            unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &old) };
            settles(2000, held.ended()).await
        })
        .await;
        if said.as_deref() != Some(want) {
            wrong.push(format!("{file}: TS {want:?}, Rust {said:?}"));
        }
    }
    assert!(wrong.is_empty(), "{} of {} spawn errors differ; first: {}", wrong.len(), table.len(), wrong[0]);
}

// ---------------------------------------------------------------------------------------------------------------
// 12. A create whose numbers pass what a u64 holds.
// ---------------------------------------------------------------------------------------------------------------

/// Answers every frame with one fixed reply and records the ask.
struct Recorder {
    reply: Value,
    asks: Mutex<Vec<LinkAsk>>,
}

#[async_trait(?Send)]
impl MachineLink for Recorder {
    async fn request(&self, _op: &str, _params: Map<String, Value>, ask: LinkAsk) -> Result<Map<String, Value>, EngineError> {
        self.asks.lock().unwrap().push(ask);
        Ok(self.reply.as_object().cloned().unwrap_or_default())
    }

    async fn forward(&self, _place_port: u16) -> Result<u16, EngineError> {
        Ok(1)
    }
}

/// TS link-backend.ts:355-359 (`{ spec: asked }` is sent whatever the numbers), printed by create.probe.ts: `res "ok"` for
/// `{ kind: "sandbox", cpu: 1e20 }` and for `{ cpu: 1e21, memMb: 1e19 }`. link_backend.rs:454 `serde_json::to_value(&spec)`
/// fails on a JsNumber from 1.8e19 up (wsp-wire js_number.rs serializes it as u128). The table is the specs that differ.
#[tokio::test]
async fn f12_a_create_sends_the_spec_whatever_its_numbers() {
    let table = [json!({ "kind": "sandbox", "cpu": 1e20 }), json!({ "kind": "sandbox", "cpu": 1e21, "memMb": 1e19 })];
    let facts = wire_parse::<BackendFacts>(&json!({ "offer": "docker", "capabilities": { "liveCloneForks": false, "replacesMachine": true,
        "previewUrls": true, "signedUrls": false, "callbackRelay": true, "diskSnapshots": true, "images": true, "snapshotsAnyLife": false,
        "snapshotListing": true, "templates": true, "kept": false, "copies": true, "ownNetwork": true, "sizes": [] },
        "pricing": { "defaultSize": { "cpu": 2, "memMb": 4096 }, "snapshotStorage": { "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" } } }))
    .unwrap();
    let mut wrong = Vec::new();
    for spec_json in &table {
        let link = Arc::new(Recorder {
            reply: json!({ "machine": { "id": "c1", "kind": "sandbox",
                "roads": { "previewUrl": true, "daemonAnswers": true, "putBytes": true, "describe": true, "facts": true, "metrics": true } } }),
            asks: Mutex::new(Vec::new()),
        });
        let backend = LinkBackend::of(link.clone(), facts.clone());
        let spec = wire_parse::<MachineSpec>(spec_json).unwrap();
        match backend.create(spec, None, None).await {
            Ok(_) => {}
            Err(e) => wrong.push(format!("create of {spec_json}: TS sent the frame and answered ok, Rust answered err: {e}")),
        }
    }
    assert!(wrong.is_empty(), "{} of {} creates differ; first: {}", wrong.len(), table.len(), wrong[0]);
}

// ---------------------------------------------------------------------------------------------------------------
// 13. A very long wait plus the link margin.
// ---------------------------------------------------------------------------------------------------------------

/// TS link-backend.ts:152-159 (`timeoutMs + LINK_MARGIN_MS`, a float sum) rides any wait the caller names as that wait
/// plus 5,000 ms. link_backend.rs:238 adds in u64: a debug build panics, a release build wraps to 4,999.
#[tokio::test]
async fn f13_a_very_long_wait_rides_the_ask_as_the_wait_plus_the_margin() {
    let link = Arc::new(Recorder { reply: json!({ "result": { "exitCode": 0, "stdout": "", "stderr": "" } }), asks: Mutex::new(Vec::new()) });
    let machine = LinkMachine::new(link.clone(), handle_c1());
    let long = u64::MAX - 1_000;
    let done = AssertUnwindSafe(machine.exec("x", ExecOptions { timeout_ms: Some(long), ..Default::default() })).catch_unwind().await;
    assert!(done.is_ok(), "exec panicked adding the link margin to a wait of {long} ms; the TS sends the wait plus 5000");
    let ask = link.asks.lock().unwrap()[0].timeout_ms.unwrap();
    assert!(ask >= long, "the ask waits {ask} ms for a command given {long} ms");
}
