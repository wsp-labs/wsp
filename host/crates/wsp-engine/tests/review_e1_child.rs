// SPDX-License-Identifier: AGPL-3.0-only
//! Review of packet e1: child_exec.rs, spawn_fed.rs and config_here.rs against the TS (node 22 on aws-mumbai).
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wsp_engine::child_exec::{run_child, ChildOptions};
use wsp_engine::config_here::{config_landed, write_config_here};
use wsp_engine::machine::{ExecResult, OnLine};
use wsp_engine::spawn_fed::{spawn_sync_fed, FedOptions};

fn sh(script: &str) -> (String, Vec<String>) {
    ("/bin/sh".to_string(), vec!["-c".to_string(), script.to_string()])
}

fn alive(pid: i32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => !s.contains(") Z "),
        Err(_) => false,
    }
}

fn fffd(s: &str) -> usize {
    s.chars().filter(|c| *c == '\u{fffd}').count()
}

fn read_pid(p: &Path) -> i32 {
    fs::read_to_string(p).unwrap().trim().parse().unwrap()
}

fn j(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

fn sink() -> (Arc<Mutex<Vec<String>>>, OnLine) {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let into = lines.clone();
    (lines, Arc::new(move |l: &str| into.lock().unwrap().push(l.to_string())))
}

/// Kills the process and its group whose pid a script wrote in a file, whether the test passed or not.
struct Reaper(PathBuf);
impl Drop for Reaper {
    fn drop(&mut self) {
        if let Ok(text) = fs::read_to_string(&self.0) {
            if let Ok(pid) = text.trim().parse::<i32>() {
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
        }
    }
}

// child-exec.ts:87 decodes each chunk node hands over, and node reads up to 64 KiB at a time; child_exec.rs:139 reads
// 8192 bytes at a time, so a character that straddles byte 8192 becomes U+FFFD only here.
#[tokio::test]
async fn finding_a_three_byte_character_is_whole_in_output_node_reads_in_one_chunk() {
    let dir = tempfile::tempdir().unwrap();
    for n in [3_334usize, 10_000, 20_000] {
        let f = dir.path().join(format!("eur{n}"));
        fs::write(&f, "€".repeat(n)).unwrap();
        let (lines, on_line) = sink();
        let res =
            run_child("/bin/cat", &[f.to_str().unwrap().to_string()], ChildOptions { on_line: Some(on_line), ..Default::default() }).await;
        let got = lines.lock().unwrap().clone();
        assert_eq!(
            (fffd(&res.stdout), res.stdout.len(), got.iter().map(|l| fffd(l)).sum::<usize>()),
            (0, 3 * n, 0),
            "n={n}: stdout U+FFFD count, bytes, on_line U+FFFD count"
        );
    }
}

// child-exec.ts:128-129 destroys stdout and stderr when the answer goes, so a child still in the pipe's other end
// gets SIGPIPE at its next write; child_exec.rs:138 and :153 keep their reader tasks reading until the pipes close.
#[tokio::test]
async fn finding_a_grandchild_that_keeps_writing_is_cut_off_once_the_answer_has_gone() {
    let dir = tempfile::tempdir().unwrap();
    let pf = dir.path().join("gc.pid");
    let _reap = Reaper(pf.clone());
    let (file, args) = sh(&format!("sh -c 'echo $$ > {}; while :; do echo x; sleep 0.05; done' & exit 0", pf.display()));
    let res = run_child(&file, &args, ChildOptions::default()).await;
    assert_eq!(res.exit_code, 0.into());
    let pid = read_pid(&pf);
    tokio::time::sleep(Duration::from_millis(1_000)).await;
    assert!(!alive(pid), "grandchild {pid} still runs one second after the answer: its pipe is still being read");
}

// Same rule, seen from the pipe: after the answer nothing of this process still holds the read end of a pipe a
// silent grandchild holds the write end of (node closed it with destroy()).
#[tokio::test]
async fn finding_the_read_ends_of_the_pipes_are_closed_once_the_answer_has_gone() {
    let dir = tempfile::tempdir().unwrap();
    let pf = dir.path().join("sleep.pid");
    let _reap = Reaper(pf.clone());
    let (file, args) = sh(&format!("sleep 30 & echo $! > {}; exit 0", pf.display()));
    let res = run_child(&file, &args, ChildOptions::default()).await;
    assert_eq!(res.exit_code, 0.into());
    let pid = read_pid(&pf);
    let mut held = Vec::new();
    for fd in [1, 2] {
        let pipe = fs::read_link(format!("/proc/{pid}/fd/{fd}")).unwrap();
        for e in fs::read_dir("/proc/self/fd").unwrap().flatten() {
            if fs::read_link(e.path()).map(|t| t == pipe).unwrap_or(false) {
                held.push((fd, pipe.clone(), e.file_name()));
            }
        }
    }
    assert_eq!(held, vec![], "this process still holds the read ends of the pipes the sleep holds the write ends of");
}

// spawn-fed.ts:25 is spawnSync, which closes the pipes when its timeout kills the child: it answers at the deadline
// with `error` ETIMEDOUT though a child of the shell still holds stdout. spawn_fed.rs:68 joins the readers, which wait
// for every holder of the pipes.
#[test]
fn finding_a_fed_spawn_past_its_timeout_answers_at_the_deadline_though_a_child_of_the_shell_holds_the_pipes() {
    let started = Instant::now();
    let ran = spawn_sync_fed(
        "/bin/sh",
        &["-c".to_string(), "echo hi; sleep 4; echo late".to_string()],
        b"",
        &FedOptions { timeout: Some(Duration::from_millis(300)), ..Default::default() },
    );
    let took = started.elapsed();
    assert_eq!(ran.error.as_deref(), Some("spawnSync /bin/sh ETIMEDOUT"));
    assert_eq!(ran.status, None);
    assert!(took < Duration::from_millis(1_500), "answered after {took:?}, the deadline was 300 ms (the TS: ~300 ms)");
}

// config_here.rs:52 `std::env::vars()` panics on any variable that is not valid Unicode; node passes it through.
// Run in a child of this test binary so the variable never reaches another test.
#[test]
#[ignore]
fn review_body_write_config_here_with_the_environment_it_was_given() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();
    let file = format!("{base}/f.json");
    write_config_here(&file, &base, None, "text\n", None).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "text\n");
}

#[test]
fn finding_a_config_write_does_not_panic_on_an_environment_variable_that_is_not_unicode() {
    let exe = std::env::current_exe().unwrap();
    let out = Command::new(exe)
        .args(["--ignored", "--exact", "review_body_write_config_here_with_the_environment_it_was_given", "--test-threads=1"])
        .env("WSP_REVIEW_NOT_UNICODE", std::ffi::OsStr::from_bytes(b"caf\xe9"))
        .output()
        .unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the write panicked on the environment:\n{text}");
}

// spawn-fed.ts:17 mkdtempSync throws node's own fs error when the temp folder cannot be made; writeConfigHere lets
// it out as it is. config_here.rs turns it into the "stands as it was" sentence with Rust's wording.
#[test]
#[ignore]
fn review_body_write_config_here_with_no_temp_folder() {
    let base = std::env::var("WSP_REVIEW_BASE").unwrap();
    let err = write_config_here(&format!("{base}/f.json"), &base, None, "text\n", None).unwrap_err();
    println!("MESSAGE {}", j(&err.to_string()));
    assert_eq!(err.to_string(), "ENOENT: no such file or directory, mkdtemp '/nonexistent-review-tmp/wsp-stdin-XXXXXX'");
}

#[test]
fn finding_a_config_write_with_no_temp_folder_says_what_node_says() {
    let base = tempfile::tempdir().unwrap();
    let exe = std::env::current_exe().unwrap();
    let out = Command::new(exe)
        .args(["--ignored", "--exact", "review_body_write_config_here_with_no_temp_folder", "--test-threads=1", "--nocapture"])
        .env("TMPDIR", "/nonexistent-review-tmp")
        .env("WSP_REVIEW_BASE", base.path())
        .output()
        .unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{text}");
}

// spawn-fed.ts:18 node names a signal that killed the child by its own name, all of them.
#[test]
fn finding_a_fed_spawn_names_every_signal_the_way_node_does() {
    let mut got = Vec::new();
    for sig in ["BUS", "FPE", "ILL", "TRAP", "SYS", "XCPU", "XFSZ", "VTALRM", "PROF", "IO", "PWR"] {
        let ran = spawn_sync_fed("/bin/sh", &["-c".to_string(), format!("kill -{sig} $$")], b"", &FedOptions::default());
        got.push((sig, ran.signal.unwrap_or_default()));
    }
    let want: Vec<(&str, String)> = ["BUS", "FPE", "ILL", "TRAP", "SYS", "XCPU", "XFSZ", "VTALRM", "PROF", "IO", "PWR"]
        .iter()
        .map(|s| (*s, format!("SIG{s}")))
        .collect();
    assert_eq!(got, want);
}

// child-exec.ts:111-125 reads each chunk as node delivers it, so nothing node has read is ever cut off by the drain
// timers: they only start at the exit and stop at the pipes' close, and node cannot have read more than the kernel
// holds. child_exec.rs reads into an unbounded channel on other tasks, so at the exit the whole output can be queued
// unprocessed, and DRAIN_CAP_MS then ends the answer while it is still being worked through: exit 0, output cut.
// (The TS: 10 MB of short lines with a line callback in about 0.2 s; 30 MB in 0.3 s.) On a current_thread runtime the
// reader and the answer take turns and nothing queues; the host's runtime is multi-threaded (Cargo.toml: rt-multi-thread).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn finding_ten_million_bytes_of_short_lines_are_answered_whole_with_a_line_callback() {
    let (file, args) = sh("yes x | head -c 10000000");
    let count = Arc::new(Mutex::new(0usize));
    let into = count.clone();
    let on_line: OnLine = Arc::new(move |_l: &str| *into.lock().unwrap() += 1);
    let started = Instant::now();
    let res = run_child(&file, &args, ChildOptions { on_line: Some(on_line), ..Default::default() }).await;
    let got = (res.exit_code.get(), res.stdout.len(), *count.lock().unwrap());
    assert_eq!(got, (0, 10_000_000, 5_000_000), "exit code, stdout bytes, lines heard, after {:?}", started.elapsed());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn finding_twenty_million_bytes_with_no_newline_are_answered_whole_with_a_line_callback() {
    let (file, args) = sh("head -c 20000000 /dev/zero | tr '\\0' 'a'");
    let heard = Arc::new(Mutex::new(0usize));
    let into = heard.clone();
    let on_line: OnLine = Arc::new(move |l: &str| *into.lock().unwrap() += l.len());
    let started = Instant::now();
    let res = run_child(&file, &args, ChildOptions { on_line: Some(on_line), ..Default::default() }).await;
    let got = (res.exit_code.get(), res.stdout.len(), *heard.lock().unwrap());
    assert_eq!(got, (0, 20_000_000, 20_000_000), "exit code, stdout bytes, bytes in the last partial line, after {:?}", started.elapsed());
}

// child-exec.ts:92 node's spawn is execvp, which hands an executable file with no #! line to /bin/sh; child_exec.rs
// spawns by posix_spawn and answers 127 "Exec format error".
#[tokio::test]
async fn finding_an_executable_file_with_no_shebang_line_is_run_by_the_shell_as_node_does() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path().join("noshebang");
    fs::write(&at, "echo noshebang\n").unwrap();
    fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).unwrap();
    let res = run_child(at.to_str().unwrap(), &[], ChildOptions::default()).await;
    assert_eq!((res.exit_code, res.stdout.as_str(), res.stderr.as_str()), (0.into(), "noshebang\n", ""));
}

// spawn-fed.ts:25 spawnSync names a start failure by its errno word in `error.message`, whatever it is.
#[test]
fn finding_a_fed_spawn_start_failure_is_named_by_its_errno_word() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain");
    fs::write(&plain, "x").unwrap();
    let a = spawn_sync_fed("/bin/sh", &["-c".to_string(), "pwd".to_string()], b"", &FedOptions { cwd: Some(plain), ..Default::default() });
    let b = spawn_sync_fed("/bin/sh", &["-c".to_string(), "x".repeat(300_000)], b"", &FedOptions::default());
    assert_eq!((a.error.as_deref(), b.error.as_deref()), (Some("spawnSync /bin/sh ENOTDIR"), Some("spawnSync /bin/sh E2BIG")));
}

// config-write.ts's configRefusal compares the exit code to 4, 5 and 6 as a JS number; config_here.rs:25 narrows it
// to an i32 first, so an exit code of 2^32 + 4 (a machine can answer any number) reads as the link refusal.
#[test]
fn finding_an_exit_code_that_is_not_4_is_not_read_as_4() {
    let res = ExecResult { exit_code: ((1i64 << 32) + 4).into(), stdout: String::new(), stderr: String::new() };
    let err = config_landed(&res, "/h/f", None).unwrap_err();
    assert_eq!(err.to_string(), "/h/f stands as it was: the write did not finish (exit 4294967300).");
}
