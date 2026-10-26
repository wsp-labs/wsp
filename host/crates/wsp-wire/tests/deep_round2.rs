// SPDX-License-Identifier: AGPL-3.0-only
//! Round 2 of wsp-labs/wsp-map#2448 (wsp-wire): a process list that holds a pid twice walks as node answers where node
//! answers and ends where a parent cycle would not, and a chain of agents is placed in linear time.
use std::time::{Duration, Instant};

use wsp_frames::ProcEntry;
use wsp_wire::views::activity::{activity_of, place_procs, ActivityRead, PlaceInput, PlaceThread};
use wsp_wire::views::activity_fold::fold_tree;

fn in_child(name: &str, body: fn()) {
    if std::env::var("ROUND2_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("ROUND2_2448_CHILD", name)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let why: Vec<&str> = stderr.lines().filter(|l| l.contains("linear") || l.contains("overflow") || l.contains("left:")).take(2).collect();
    assert!(out.status.success(), "the child failed ({}): {}", out.status, why.join(" | "));
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

// processes_choices.rs agent_path: `path.insert(0, ..)` per step of the way up and `path.contains` per step of the way
// down made a chain of agents quadratic (the TS `unshift` and `includes` are). Doubling the chain must not quadruple the time.
fn chain_of_agents(n: u32) -> Vec<ProcEntry> {
    (1..=n)
        .map(|pid| {
            let mut p = row(pid, pid - 1);
            p.comm = "claude".into();
            p.cmdline = "claude".into();
            p
        })
        .collect()
}

fn placed_in(n: u32, recorded: bool) -> Duration {
    let threads = [PlaceThread {
        id: "t1".into(),
        roots: vec![1],
        bin: Some("claude".into()),
        agents: recorded.then(|| vec![n]),
        ..Default::default()
    }];
    let procs = chain_of_agents(n);
    let bins = vec!["claude".to_string()];
    let listens = |_: u32| false;
    let input =
        PlaceInput { procs: &procs, daemon: 999_999_999, host: None, threads: &threads, agent_bins: &bins, listens: &listens, app: None };
    let at = Instant::now();
    let placed = place_procs(&input);
    let took = at.elapsed();
    assert_eq!(placed.agent.keys().copied().collect::<Vec<_>>(), vec![n], "the agent is the last of the chain");
    assert_eq!(placed.folds_into.len() as u32, n - 1, "every other process of the chain folds into it");
    took
}

fn best_of_three(n: u32, recorded: bool) -> Duration {
    (0..3).map(|_| placed_in(n, recorded)).min().unwrap()
}

fn agent_chain_body() {
    for recorded in [false, true] {
        let small = best_of_three(20_000, recorded).max(Duration::from_millis(20));
        let large = best_of_three(40_000, recorded);
        assert!(large < small * 3, "recorded={recorded}: 40,000 agents took {large:?} where 20,000 took {small:?}: not linear");
    }
}

#[test]
fn a_chain_of_agents_is_placed_in_linear_time() {
    in_child("a_chain_of_agents_is_placed_in_linear_time", agent_chain_body);
}
