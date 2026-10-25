// SPDX-License-Identifier: AGPL-3.0-only
//! The host a test starts stops with the test, including a test process that dies without unwinding, where no drop
//! runs: an abort, a stack overflow, a SIGKILL.
#![cfg(target_os = "linux")]

mod common;

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// Names the file the child below writes its host's folder and the processes it started to; unset, the child is a test
/// that does nothing.
const CHILD: &str = "WSP_TEST_ABORTS_WITH_A_HOST";

/// Every process under `pid`, read off each one's parent in /proc.
fn descendants(pid: u32) -> Vec<u32> {
    let mut parents: HashMap<u32, Vec<u32>> = HashMap::new();
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(child) = entry.file_name().to_string_lossy().parse::<u32>() else { continue };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else { continue };
        // The name in parentheses may hold spaces; the parent is the second field after it.
        let Some(ppid) = stat.rsplit_once(')').and_then(|(_, rest)| rest.split_whitespace().nth(1)?.parse().ok()) else { continue };
        parents.entry(ppid).or_default().push(child);
    }
    let (mut found, mut next) = (Vec::new(), vec![pid]);
    while let Some(at) = next.pop() {
        for &child in parents.get(&at).into_iter().flatten() {
            found.push(child);
            next.push(child);
        }
    }
    found
}

fn alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
        && !std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .unwrap_or_default()
            .rsplit_once(')')
            .is_some_and(|(_, s)| s.trim_start().starts_with('Z'))
}

#[test]
fn a_child_that_aborts_with_a_host_up() {
    let Some(out) = std::env::var_os(CHILD) else { return };
    let host = common::Throwaway::start_with(common::STAND_IN, true);
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let started: Vec<String> = descendants(std::process::id()).iter().map(u32::to_string).collect();
    std::fs::write(out, format!("{}\n{}", host.dir.path().display(), started.join("\n"))).unwrap();
    std::process::abort();
}

#[test]
fn a_test_that_aborts_leaves_no_host_running() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("started");
    let ran = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "a_child_that_aborts_with_a_host_up", "--nocapture", "--test-threads", "1"])
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env(CHILD, &out)
        .status()
        .unwrap();
    assert!(!ran.success(), "the child aborted");
    let written = std::fs::read_to_string(&out).unwrap();
    let (folder, started) = written.split_once('\n').unwrap();
    let started: Vec<u32> = started.lines().filter_map(|l| l.parse().ok()).collect();
    assert!(started.len() >= 2, "the child started the shell and the host at least: {started:?}");
    println!("started under the child: {started:?}");
    let until = Instant::now() + Duration::from_secs(30);
    while started.iter().any(|&pid| alive(pid)) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(200));
    }
    let left: Vec<u32> = started.into_iter().filter(|&pid| alive(pid)).collect();
    assert!(left.is_empty(), "still running 30 s after the child aborted: {left:?}");
    std::fs::remove_dir_all(folder).unwrap();
}
