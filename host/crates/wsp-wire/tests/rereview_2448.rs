// SPDX-License-Identifier: AGPL-3.0-only
//! Re-review of round 2 of wsp-labs/wsp-map#2448: process lists the round's tests do not try. Each case runs in a child
//! copy of this binary under a 2 GB memory cap and a 30 s limit.
use std::time::{Duration, Instant};

use wsp_frames::ProcEntry;
use wsp_wire::views::activity::{activity_of, ActivityRead};
use wsp_wire::views::activity_fold::fold_tree;

fn in_capped_child(name: &str, body: fn()) {
    if std::env::var("REREVIEW_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let mut child = std::process::Command::new("sh")
        .args(["-c", "ulimit -v 2000000; exec \"$0\" \"$@\""])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("REREVIEW_2448_CHILD", name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "the child died ({status}) after {:?}", start.elapsed());
            return;
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the walk did not end in 30 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn row(pid: u32, ppid: u32) -> ProcEntry {
    ProcEntry {
        pid,
        ppid,
        user: "ada".into(),
        state: "S".into(),
        comm: "sleep".into(),
        cmdline: "sleep 1000".into(),
        cpu: 0.0,
        rss: 1000,
        started_at: 1_760_000_000_000 + pid as i64,
        pty: Some("pts/0".into()),
        cgroup: None,
        energy_mw: None,
        pgid: None,
        owner: None,
        kind: None,
        id: None,
    }
}

fn read_of(procs: Vec<ProcEntry>) -> ActivityRead {
    ActivityRead {
        computer: "box".into(),
        at: wsp_wire::types::JsNumber(1.0),
        procs,
        daemon: 999_999_999,
        host: 999_999_998,
        scopes: vec![],
        agent_bins: vec!["claude".into()],
        ports: None,
        cgroups: None,
    }
}

/// pid 1 three times: under 99, and twice under 2, which is under 1.
fn three_times() -> Vec<ProcEntry> {
    vec![row(1, 99), row(2, 1), row(1, 2), row(1, 2), row(5, 1)]
}

fn three_times_body() {
    let pids: Vec<i64> = activity_of(&read_of(three_times())).unowned.iter().map(|p| p.pid.get()).collect();
    assert_eq!(pids, vec![1, 2, 5]);
    std::mem::forget(fold_tree(&three_times(), |_| None));
}

#[test]
fn a_cycle_through_a_pid_listed_three_times_is_cut() {
    in_capped_child("a_cycle_through_a_pid_listed_three_times_is_cut", three_times_body);
}

/// A ring of 40 pids, each listed twice under the one before it, closed back to pid 1, the one head. The walks keep only
/// the pids above the row they are on (rows.rs:26, activity_fold.rs:146), so they walk every one of the 2^39 paths
/// before the ring closes. node (views/activity.ts:415, views/activity-fold.ts:46) goes down the first kid each time,
/// meets the ring after 40 levels and again after 80, and fails the call with RangeError in milliseconds. Ruling 4 of
/// round 2: one visited set, and the walk ends.
fn doubled_ring() -> Vec<ProcEntry> {
    let mut rows = vec![row(1, 99)];
    for pid in 2..=40 {
        rows.push(row(pid, pid - 1));
        rows.push(row(pid, pid - 1));
    }
    rows.push(row(1, 40));
    rows
}

fn tree_walk_ring_body() {
    let _ = activity_of(&read_of(doubled_ring()));
}

#[test]
fn tree_walk_ends_on_a_ring_with_every_pid_listed_twice() {
    in_capped_child("tree_walk_ends_on_a_ring_with_every_pid_listed_twice", tree_walk_ring_body);
}

fn fold_tree_ring_body() {
    std::mem::forget(fold_tree(&doubled_ring(), |_| None));
}

#[test]
fn fold_tree_ends_on_a_ring_with_every_pid_listed_twice() {
    in_capped_child("fold_tree_ends_on_a_ring_with_every_pid_listed_twice", fold_tree_ring_body);
}
