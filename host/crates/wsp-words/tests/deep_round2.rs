// SPDX-License-Identifier: AGPL-3.0-only
//! Round 2 of wsp-labs/wsp-map#2448 (wsp-words): an End over a list that holds a pid twice.
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

// P2 of the review: activity.ts:511 `take` walks `kidsOf` with no set, so a pid listed twice under one parent is taken twice,
// each time with its kids. node answers [1, 2, 3, 2, 3]; end_plan answered "Maximum call stack size exceeded" for it, a
// refusal that belongs to a cycle.
#[test]
fn an_end_over_a_pid_listed_twice_with_no_cycle_takes_it_twice_as_node_does() {
    let rows = vec![row(1, 0), row(2, 1), row(2, 1), row(3, 2)];
    assert_eq!(ending(rows, 1), Ok(vec![1, 2, 3, 2, 3]));
}

// A parent cycle is what node's recursion fails on: RangeError, which the port keeps as its message.
#[test]
fn an_end_over_a_parent_cycle_fails_as_node_does() {
    let rows = vec![row(1, 99), row(2, 1), row(1, 2)];
    assert_eq!(ending(rows, 1), Err("Maximum call stack size exceeded".to_string()));
}
