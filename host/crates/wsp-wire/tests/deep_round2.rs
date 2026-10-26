// SPDX-License-Identifier: AGPL-3.0-only
//! Round 2 of wsp-labs/wsp-map#2448 (wsp-wire): a process list that holds a pid twice walks as node answers where node
//! answers and ends where a parent cycle would not.
use wsp_frames::ProcEntry;
use wsp_wire::views::activity::{activity_of, ActivityRead};
use wsp_wire::views::activity_fold::fold_tree;

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

fn pids_and_depths(procs: Vec<ProcEntry>) -> Vec<(i64, i64)> {
    activity_of(&read_of(procs)).unowned.iter().map(|p| (p.pid.get(), p.depth.get())).collect()
}

// activity.ts:411 treeRows walks `kidsOf` with no set at all, so a pid listed twice under one parent is walked twice, each
// time with the kids of the pid: node prints 1, 2, 3, 2, 3. The walk keeps the pids above the row it is on, so this holds.
#[test]
fn a_pid_listed_twice_with_no_cycle_is_walked_twice_as_node_walks_it() {
    let rows = vec![row(1, 0), row(2, 1), row(2, 1), row(3, 2)];
    assert_eq!(pids_and_depths(rows), vec![(1, 0), (2, 1), (3, 2), (2, 1), (3, 2)]);
}

// Ruling 4 of the round 2 brief: a parent cycle is broken and the rest of the tree is answered.
#[test]
fn a_parent_cycle_through_a_pid_listed_twice_is_cut_and_the_rest_is_answered() {
    let rows = vec![row(1, 99), row(2, 1), row(1, 2), row(5, 1)];
    assert_eq!(pids_and_depths(rows), vec![(1, 0), (2, 1), (5, 1)]);
}

#[test]
fn fold_tree_cuts_a_parent_cycle_and_answers_the_rest() {
    let list = vec![row(1, 99), row(2, 1), row(1, 2), row(5, 1)];
    let folded = fold_tree(&list, |_| None);
    assert_eq!(folded.len(), 1);
    assert_eq!(folded[0].p.pid, 1);
    assert_eq!(folded[0].kids.iter().map(|k| k.p.pid).collect::<Vec<_>>(), vec![2, 5]);
    assert!(folded[0].kids[0].kids.is_empty(), "pid 1 under pid 2 is the cycle");
}

#[test]
fn fold_tree_keeps_a_pid_listed_twice_with_no_cycle_as_node_does() {
    let list = vec![row(1, 0), row(2, 1), row(2, 1), row(3, 2)];
    let folded = fold_tree(&list, |_| None);
    assert_eq!(folded.len(), 1);
    assert_eq!(folded[0].kids.len(), 2);
    assert!(folded[0].kids.iter().all(|k| k.p.pid == 2 && k.kids.len() == 1 && k.kids[0].p.pid == 3));
}
