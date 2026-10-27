// SPDX-License-Identifier: AGPL-3.0-only
//! Round 2 of wsp-labs/wsp-map#2448 (wsp-words): an End over a list that holds a pid twice, and a tree the words reader keeps
//! cloned and dropped on a 1 MiB stack.
use wsp_frames::{ProcEntry, ProcOwnedBy, ProcOwner};
use wsp_wire::views::activity::{ActivityRead, EndPlan};
use wsp_words::views::activity::end_plan;

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
        pty: None,
        cgroup: None,
        energy_mw: None,
        pgid: None,
        owner: Some(ProcOwner { by: ProcOwnedBy::Root, thread: None, workspace: None, warm: false }),
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

fn ending(procs: Vec<ProcEntry>, pid: u32) -> Result<Vec<u32>, String> {
    let started = 1_760_000_000_000.0 + f64::from(pid);
    match end_plan(&read_of(procs), f64::from(pid), started, &|_| true) {
        Ok(EndPlan::Targets(targets)) => Ok(targets.iter().map(|p| p.pid).collect()),
        Ok(EndPlan::Kept(_)) => Err("kept".into()),
        Err(e) => Err(e.message),
    }
}

// Ruling 1 of fix round 3: one visited set, "a pid walked once is never walked again". activity.ts:511 `take` walks
// `kidsOf` with no set and answers [1, 2, 3, 2, 3] here; the port takes each pid once, so a list cannot make an End walk
// every path. A cycle is still the refusal below.
#[test]
fn an_end_over_a_pid_listed_twice_with_no_cycle_takes_it_once() {
    let rows = vec![row(1, 0), row(2, 1), row(2, 1), row(3, 2)];
    assert_eq!(ending(rows, 1), Ok(vec![1, 2, 3]));
}

// A parent cycle is what node's recursion fails on: RangeError, which the port keeps as its message.
#[test]
fn an_end_over_a_parent_cycle_fails_as_node_does() {
    let rows = vec![row(1, 99), row(2, 1), row(1, 2)];
    assert_eq!(ending(rows, 1), Err("Maximum call stack size exceeded".to_string()));
}

// ruling 3 of the round 2 brief: v8_json::parse_json reads 500 levels of objects, keeps 128 and reads empty containers below; the
// tree it hands back is cloned and dropped on a 1 MiB stack.
#[test]
fn a_tree_of_500_object_levels_read_by_v8_json_is_cloned_and_dropped() {
    let name = "a_tree_of_500_object_levels_read_by_v8_json_is_cloned_and_dropped";
    if std::env::var("ROUND2_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let text = format!("{}1{}", "{\"a\":".repeat(500), "}".repeat(500));
                let read = wsp_words::v8_json::parse_json(&text).expect("a text at 500 levels is read");
                let copy = read.clone();
                assert_eq!(copy, read);
                drop(copy);
                drop(read);
            })
            .unwrap()
            .join()
            .expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("ROUND2_2448_CHILD", name)
        .output()
        .unwrap();
    assert!(out.status.success(), "the child died ({}): {}", out.status, String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or(""));
}

// Ruling 1 of fix round 3: a list with every pid twice under the one before, no ring, is 2^39 paths in node and one walk here.
#[test]
fn an_end_over_a_chain_of_doubled_pids_takes_each_once() {
    let mut rows = vec![row(1, 0)];
    for pid in 2..=40 {
        rows.push(row(pid, pid - 1));
        rows.push(row(pid, pid - 1));
    }
    assert_eq!(ending(rows, 1), Ok((1..=40).collect::<Vec<u32>>()));
}
