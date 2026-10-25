// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/node-exec.test.ts, plus the tests that pin live-host.ts behaviour the TS file
//! leaves out. These read the real disk and run real processes.
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::StreamExt;
use indexmap::IndexMap;
use tokio_util::sync::CancellationToken;
use wsp_collect::host::{HostExec, HostFs, Platform, RunOptions, StatKind};
use wsp_collect::live_host::{children_of, on_path, platform_of, spawn_run, NodeExec, NodeFs};

/// Every pid a test recorded is killed when the test ends, as the TS afterEach does.
#[derive(Default)]
struct Pids(RefCell<Vec<i32>>);

impl Drop for Pids {
    fn drop(&mut self) {
        for pid in self.0.borrow().iter() {
            unsafe {
                libc::kill(*pid, libc::SIGKILL);
            }
        }
    }
}

/// The TS test budget of 15 s.
async fn within<F: Future>(body: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(15), body).await.expect("the test ran past its 15 s budget")
}

fn alive(pid: i32) -> bool {
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// A job whose parent is gone is reaped by launchd, so it may read as alive for a moment after the kill.
async fn gone(pid: i32) -> bool {
    for _ in 0..100 {
        if !alive(pid) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    !alive(pid)
}

/// Polls the pid file a shell writes so the pid is known before it is judged; a loaded machine starts a child late.
async fn pid_in(pids: &Pids, file: &Path) -> i32 {
    for _ in 0..400 {
        if file.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let pid = read_pid(file);
    pids.0.borrow_mut().push(pid);
    pid
}

fn read_pid(file: &Path) -> i32 {
    std::fs::read_to_string(file).unwrap().trim().parse().unwrap()
}

fn sv(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

fn budget(timeout_ms: u64) -> RunOptions {
    RunOptions { timeout_ms: Some(timeout_ms), ..Default::default() }
}

async fn run(cmd: &str, args: &[&str]) -> Option<String> {
    NodeExec.run(cmd, &sv(args), RunOptions::default()).await
}

fn path_str(path: &Path) -> &str {
    path.to_str().unwrap()
}

async fn all(path: &Path, holding: Option<&str>) -> Vec<String> {
    NodeFs.lines(path_str(path), holding).map(|line| line.unwrap()).collect().await
}

fn mtime_ms(path: &Path) -> f64 {
    let meta = std::fs::metadata(path).unwrap();
    meta.mtime() as f64 * 1e3 + meta.mtime_nsec() as f64 / 1e6
}

fn write_mode(path: &Path, bytes: &[u8], mode: u32) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

mod node_fs_stat {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn reads_the_modification_time_off_the_disk_so_a_reader_can_tell_a_file_it_already() {
        within(async {
            let d = tempfile::tempdir().unwrap();
            let file = d.path().join("s1.jsonl");
            std::fs::write(&file, "one\n").unwrap();
            let before = NodeFs.stat(path_str(&file)).await.expect("the file has a stat");
            assert_eq!(before.kind, StatKind::File);
            assert_eq!(before.bytes, 4);
            assert_eq!(before.mtime_ms, mtime_ms(&file));
            let moved = std::fs::metadata(&file).unwrap().modified().unwrap() + Duration::from_secs(5);
            std::fs::OpenOptions::new().write(true).open(&file).unwrap().set_modified(moved).unwrap();
            let after = NodeFs.stat(path_str(&file)).await.expect("the file has a stat");
            assert!(after.mtime_ms > before.mtime_ms);
        })
        .await;
    }
}

mod node_fs_lines {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn gives_each_line_without_its_ending_a_blank_one_kept_the_last_one_read_without_a() {
        within(async {
            let d = tempfile::tempdir().unwrap();
            let file = d.path().join("a.jsonl");
            std::fs::write(&file, "one\r\ntwo\n\nthree").unwrap();
            assert_eq!(all(&file, None).await, ["one", "two", "", "three"]);
            let other = tempfile::tempdir().unwrap();
            assert_eq!(all(&other.path().join("gone.jsonl"), None).await, Vec::<String>::new());
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn keeps_a_line_longer_than_one_read_whole_its_multi_byte_characters_across_the_cut() {
        within(async {
            let d = tempfile::tempdir().unwrap();
            let file = d.path().join("b.jsonl");
            let long = format!("{{\"text\":\"{}\"}}", "é漢".repeat(700_000));
            std::fs::write(&file, format!("first\n{long}\nlast\n")).unwrap();
            assert_eq!(all(&file, None).await, ["first", long.as_str(), "last"]);
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gives_only_the_lines_holding_the_text_asked_for_the_long_one_included() {
        within(async {
            let d = tempfile::tempdir().unwrap();
            let file = d.path().join("c.jsonl");
            let long = format!("{{\"type\":\"tool_use\",\"pad\":\"{}\"}}", "x".repeat(3_000_000));
            std::fs::write(&file, format!("{{\"type\":\"user\"}}\n{long}\n{{\"type\":\"tool_use\"}}\n{{\"type\":\"result\"}}")).unwrap();
            assert_eq!(all(&file, Some("\"tool_use\"")).await, [long.as_str(), "{\"type\":\"tool_use\"}"]);
        })
        .await;
    }
}

mod node_exec_run {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn returns_stdout_on_exit_0_and_nothing_on_a_failure_or_a_missing_command() {
        within(async {
            assert_eq!(run("/bin/sh", &["-c", "printf hi"]).await, Some("hi".to_string()));
            assert_eq!(run("/bin/sh", &["-c", "echo out; exit 3"]).await, None);
            assert_eq!(run("/nonexistent/wsp-bin", &[]).await, None);
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gives_every_child_end_of_file_on_stdin_so_a_shell_that_reads_it_returns_at_once() {
        within(async {
            let out = NodeExec.run("/bin/sh", &sv(&["-c", "read x; echo done"]), budget(5000)).await;
            assert_eq!(out, Some("done\n".to_string()));
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn adds_the_given_variables_to_the_collector_s_own_environment() {
        within(async {
            let mut given = IndexMap::new();
            given.insert("WSP_COLLECT".to_string(), "1".to_string());
            let opts = RunOptions { env: Some(given), ..Default::default() };
            let out = NodeExec.run("/bin/sh", &sv(&["-c", "echo \"$WSP_COLLECT:${PATH:+path}\""]), opts).await;
            assert_eq!(out, Some("1:path\n".to_string()));
            assert_eq!(run("/bin/sh", &["-c", "echo \"[$WSP_COLLECT]\""]).await, Some("[]\n".to_string()));
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ends_a_child_at_the_budget() {
        let pids = Pids::default();
        within(async {
            let d = tempfile::tempdir().unwrap();
            let script = format!("echo $$ > {}; while :; do sleep 1; done", d.path().join("pid").display());
            let args = sv(&["-c", &script]);
            let out = NodeExec.run("/bin/sh", &args, budget(300));
            let (out, pid) = tokio::join!(out, async {
                let pid = pid_in(&pids, &d.path().join("pid")).await;
                assert!(alive(pid));
                pid
            });
            assert_eq!(out, None);
            assert!(gone(pid).await);
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ends_a_child_that_ignores_the_budget_s_sigterm_as_an_interactive_zsh_does_and_ev() {
        let pids = Pids::default();
        within(async {
            let d = tempfile::tempdir().unwrap();
            let script = format!(
                "trap \"\" TERM; echo $$ > {}; sleep 100 & echo $! > {}; while :; do sleep 1; done",
                d.path().join("pid").display(),
                d.path().join("job").display()
            );
            let args = sv(&["-c", &script]);
            let out = NodeExec.run("/bin/sh", &args, budget(300));
            let (out, (shell, job)) = tokio::join!(out, async {
                let shell = pid_in(&pids, &d.path().join("pid")).await;
                let job = pid_in(&pids, &d.path().join("job")).await;
                (shell, job)
            });
            assert_eq!(out, None);
            assert!(gone(shell).await);
            assert!(gone(job).await);
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn does_not_wait_on_a_grandchild_that_kept_stdout_after_the_child_exited_and_ends_i() {
        let pids = Pids::default();
        within(async {
            let d = tempfile::tempdir().unwrap();
            let script = format!("sleep 30 & echo $! > {}; echo listed", d.path().join("pid").display());
            let out = run("/bin/sh", &["-c", &script]).await;
            let pid = pid_in(&pids, &d.path().join("pid")).await;
            assert_eq!(out, Some("listed\n".to_string()));
            assert!(gone(pid).await);
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_abort_after_the_child_exited_signals_its_group_alone_never_the_pid_it_let_go() {
        let pids = Pids::default();
        within(async {
            let d = tempfile::tempdir().unwrap();
            let stop = CancellationToken::new();
            let script = format!("echo $$ > {}; sleep 30 & echo $! > {}", d.path().join("pid").display(), d.path().join("job").display());
            let args = sv(&["-c", &script]);
            let out = NodeExec.run("/bin/sh", &args, RunOptions { signal: Some(stop.clone()), ..Default::default() });
            // The TS spies on process.kill and asserts no signal went to the shell's pid, which the system may have
            // handed to another process once the child was reaped. libc::kill cannot be spied on, so what is
            // observable stays: the run answers None and the job the shell left behind is gone.
            let (out, job) = tokio::join!(out, async {
                let job = pid_in(&pids, &d.path().join("job")).await;
                let shell = read_pid(&d.path().join("pid"));
                assert!(gone(shell).await);
                stop.cancel();
                job
            });
            assert_eq!(out, None);
            assert!(gone(job).await);
        })
        .await;
    }

    // The shell is a fixture here, not the subject: what this proves is that the budget takes the group, whatever
    // started the job in it. It is bash, which every Mac and every Linux runner has, read through --rcfile so the
    // person's own rc file is never sourced; macOS's bash 3.2 honours the flag for an interactive shell as well.
    #[tokio::test(flavor = "current_thread")]
    async fn the_budget_ends_the_whole_process_group_a_job_a_real_rc_file_started_is_dead_whe() {
        let pids = Pids::default();
        within(async {
            let d = tempfile::tempdir().unwrap();
            let rc = d.path().join("rc");
            std::fs::write(
                &rc,
                format!("sleep 100 & echo $! > {}; echo $$ > {}; wait\n", d.path().join("job").display(), d.path().join("pid").display()),
            )
            .unwrap();
            let args = sv(&["--rcfile", path_str(&rc), "-ic", ":"]);
            let out = NodeExec.run("/bin/bash", &args, budget(500));
            let (out, (shell, job)) = tokio::join!(out, async {
                let shell = pid_in(&pids, &d.path().join("pid")).await;
                let job = pid_in(&pids, &d.path().join("job")).await;
                assert!(alive(shell));
                assert!(alive(job));
                (shell, job)
            });
            assert_eq!(out, None);
            assert!(!alive(shell));
            assert!(gone(job).await);
        })
        .await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_run_whose_stdout_passes_64_mib_answers_nothing() {
    within(async {
        assert_eq!(run("/bin/sh", &["-c", "head -c 67108865 /dev/zero"]).await, None);
        let text = run("/bin/sh", &["-c", "head -c 1048576 /dev/zero"]).await.expect("1 MiB of output is answered");
        assert_eq!(text.len(), 1048576);
        assert!(text.chars().all(|c| c == '\0'));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_run_asked_after_its_signal_was_cancelled_starts_nothing() {
    within(async {
        let d = tempfile::tempdir().unwrap();
        let stop = CancellationToken::new();
        stop.cancel();
        let script = format!("touch {}", d.path().join("ran").display());
        let out = NodeExec.run("/bin/sh", &sv(&["-c", &script]), RunOptions { signal: Some(stop), ..Default::default() }).await;
        assert_eq!(out, None);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!d.path().join("ran").exists());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_child_killed_by_a_signal_answers_nothing_even_with_output() {
    within(async {
        assert_eq!(run("/bin/sh", &["-c", "echo hi; kill -TERM $$"]).await, None);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_child_runs_under_exactly_the_environment_it_is_given() {
    within(async {
        let given = vec![("A".to_string(), "1".to_string())];
        let out = spawn_run("/bin/sh", &sv(&["-c", "echo \"[$A][$HOME]\""]), &given, RunOptions::default()).await;
        assert_eq!(out, Some("[1][]\n".to_string()));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn stderr_is_dropped_and_never_mixed_into_the_answer() {
    within(async {
        assert_eq!(run("/bin/sh", &["-c", "echo out; echo err >&2"]).await, Some("out\n".to_string()));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn lines_drop_one_carriage_return_and_give_a_lone_cr_line_as_empty() {
    within(async {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("cr");
        std::fs::write(&file, b"a\r\r\n\r\nb\r").unwrap();
        assert_eq!(all(&file, None).await, ["a\r", "", "b"]);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn lines_holding_text_skip_lines_without_it_and_keep_a_matching_last_line() {
    within(async {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("x");
        std::fs::write(&file, "x1\ny2\nx3").unwrap();
        assert_eq!(all(&file, Some("x")).await, ["x1", "x3"]);
        assert_eq!(all(&file, Some("zz")).await, Vec::<String>::new());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn lines_read_bad_utf8_as_replacement_characters() {
    within(async {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("bad");
        std::fs::write(&file, b"ok\n\xff\xfe\n").unwrap();
        assert_eq!(all(&file, None).await, ["ok", "\u{FFFD}\u{FFFD}"]);
    })
    .await;
}

/// a.txt (3 bytes), sub/b.txt (4), .git/c (100), node_modules/d (100) and a link to a.txt.
fn tree() -> (tempfile::TempDir, PathBuf) {
    let holder = tempfile::tempdir().unwrap();
    let dir = holder.path().join("tree");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::create_dir_all(dir.join("node_modules")).unwrap();
    std::fs::write(dir.join("a.txt"), "abc").unwrap();
    std::fs::write(dir.join("sub/b.txt"), "defg").unwrap();
    std::fs::write(dir.join(".git/c"), vec![b'x'; 100]).unwrap();
    std::fs::write(dir.join("node_modules/d"), vec![b'x'; 100]).unwrap();
    std::os::unix::fs::symlink(dir.join("a.txt"), dir.join("link")).unwrap();
    (holder, dir)
}

#[tokio::test(flavor = "current_thread")]
async fn stat_sizes_a_folder_by_every_file_under_it_but_not_git_node_modules_or_links() {
    within(async {
        let (_holder, dir) = tree();
        let stat = NodeFs.stat(path_str(&dir)).await.expect("a folder has a stat");
        assert_eq!(stat.kind, StatKind::Dir);
        assert_eq!(stat.bytes, 7);
        let fifo = dir.join("fifo");
        let name = std::ffi::CString::new(path_str(&fifo)).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert_eq!(NodeFs.stat(path_str(&fifo)).await, None);
        assert_eq!(NodeFs.stat(path_str(&dir.join("missing"))).await, None);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn walk_lists_every_regular_file_sorted_absolute_skipping_git_node_modules_and_links() {
    within(async {
        let (_holder, dir) = tree();
        let expected = vec![path_str(&dir.join("a.txt")).to_string(), path_str(&dir.join("sub/b.txt")).to_string()];
        assert_eq!(NodeFs.walk(path_str(&dir)).await, expected);
        assert_eq!(NodeFs.walk(path_str(&dir.join("missing"))).await, Vec::<String>::new());
        assert_eq!(NodeFs.list(path_str(&dir)).await, [".git", "a.txt", "link", "node_modules", "sub"]);
        assert_eq!(NodeFs.list(path_str(&dir.join("missing"))).await, Vec::<String>::new());
    })
    .await;
}

#[test]
fn children_of_reads_a_ps_listing_and_skips_lines_that_are_not_two_numbers() {
    let expected: HashMap<u32, Vec<u32>> = HashMap::from([(0, vec![1]), (1, vec![20, 21]), (20, vec![30])]);
    assert_eq!(children_of("  1     0\n 20 1\nx y\n 21 1\n\n 30 20\n"), expected);
}

#[test]
fn platform_of_knows_darwin_and_linux_only() {
    assert_eq!(platform_of("macos"), Some(Platform::Darwin));
    assert_eq!(platform_of("linux"), Some(Platform::Linux));
    assert_eq!(platform_of("windows"), None);
    assert_eq!(platform_of("freebsd"), None);
}

#[tokio::test(flavor = "current_thread")]
async fn on_path_finds_an_executable_file_and_skips_empty_entries() {
    within(async {
        let d = tempfile::tempdir().unwrap();
        write_mode(&d.path().join("wsp-x"), b"#!/bin/sh\n", 0o755);
        write_mode(&d.path().join("wsp-y"), b"#!/bin/sh\n", 0o644);
        let path = format!(":/nonexistent:{}", d.path().display());
        assert!(on_path("wsp-x", Some(&path)).await);
        assert!(!on_path("wsp-y", Some(&path)).await);
        assert!(!on_path("wsp-x", None).await);
        assert!(!on_path("wsp-x", Some("")).await);
    })
    .await;
}
