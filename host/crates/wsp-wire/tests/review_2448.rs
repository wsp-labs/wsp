// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448): a daemon list that holds one pid twice inside a parent cycle.
//! The walks have no visited set, so they never end; each case runs in a child under a 2 GB memory cap and a 30 s limit.
use std::time::{Duration, Instant};

use wsp_frames::ProcEntry;
use wsp_wire::views::activity::{activity_of, ActivityRead};
use wsp_wire::views::activity_fold::fold_tree;

/// Runs `body` in a child copy of this binary under `ulimit -v`, and fails when it dies or does not end in 30 s.
fn in_capped_child(name: &str, body: fn()) {
    if std::env::var("REVIEW_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let mut child = std::process::Command::new("sh")
        .args(["-c", "ulimit -v 2000000; exec \"$0\" \"$@\""])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("REVIEW_2448_CHILD", name)
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

/// pid 1 under 99, pid 2 under 1, and pid 1 again under 2: 1 -> 2 -> 1 -> 2 ...
fn twice_in_a_cycle() -> Vec<ProcEntry> {
    vec![row(1, 99), row(2, 1), row(1, 2)]
}

// rows.rs:26 tree_walk pushes each process's kids with no set of pids already walked (ruling 5: one visited set). node
// (views/activity.ts:415) fails that one call with RangeError; place_procs, kind_of, reach_of and end_plan all end on it.
fn tree_walk_body() {
    let read = ActivityRead {
        computer: "box".into(),
        at: wsp_wire::types::JsNumber(1.0),
        procs: twice_in_a_cycle(),
        daemon: 999_999_999,
        host: 999_999_998,
        scopes: vec![],
        agent_bins: vec!["claude".into()],
        ports: None,
        cgroups: None,
    };
    let _ = activity_of(&read);
}

#[test]
fn tree_walk_ends_on_a_pid_listed_twice_in_a_cycle() {
    in_capped_child("tree_walk_ends_on_a_pid_listed_twice_in_a_cycle", tree_walk_body);
}

// activity_fold.rs:146 build: the same list. node (views/activity-fold.ts:46) fails the call with RangeError.
fn fold_tree_body() {
    let folded = fold_tree(&twice_in_a_cycle(), |_| None);
    std::mem::forget(folded);
}

#[test]
fn fold_tree_ends_on_a_pid_listed_twice_in_a_cycle() {
    in_capped_child("fold_tree_ends_on_a_pid_listed_twice_in_a_cycle", fold_tree_body);
}
