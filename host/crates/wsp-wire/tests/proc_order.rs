// SPDX-License-Identifier: AGPL-3.0-only
//! `by_proc_column` on a difference JavaScript calls NaN, and on the two process rows that share the rule.
use std::cmp::Ordering;

use serde_json::json;
use wsp_wire::types::JsNumber;
use wsp_wire::wire::daemon::{by_proc_column, ProcEntry, ProcSort};

fn row(pid: i64, cpu: f64) -> ProcEntry {
    let mut row: ProcEntry = serde_json::from_value(
        json!({ "pid": pid, "ppid": 1, "user": "u", "state": "S", "comm": "c", "cmdline": "x", "cpu": 0, "rss": 1, "startedAt": 1 }),
    )
    .unwrap();
    row.cpu = JsNumber(cpu);
    row
}

// daemon.ts:414: `const d = b.cpu - a.cpu; return d !== 0 ? d : a.pid - b.pid`. Infinity minus infinity is NaN, which is not 0,
// so the comparator answers NaN and `Array.sort` reads that as a tie.
#[test]
fn a_nan_difference_ties_the_rows_and_the_pid_does_not_break_it() {
    let (low, high) = (row(1, f64::INFINITY), row(2, f64::INFINITY));
    assert_eq!(by_proc_column(ProcSort::Cpu, &low, &high), Ordering::Equal);
    assert_eq!(by_proc_column(ProcSort::Cpu, &high, &low), Ordering::Equal);
    // A finite tie still breaks by pid.
    assert_eq!(by_proc_column(ProcSort::Cpu, &row(1, 5.0), &row(2, 5.0)), Ordering::Less);
}

#[test]
fn the_daemon_s_own_row_takes_the_same_rule() {
    let frames = |pid: u32, cpu: f64| wsp_frames::ProcEntry {
        pid,
        ppid: 1,
        user: "u".into(),
        state: "S".into(),
        comm: "c".into(),
        cmdline: "x".into(),
        cpu,
        rss: 1,
        started_at: 1,
        pty: None,
        cgroup: None,
        energy_mw: None,
        pgid: None,
        owner: None,
        kind: None,
        id: None,
    };
    assert_eq!(by_proc_column(ProcSort::Cpu, &frames(1, f64::INFINITY), &frames(2, f64::INFINITY)), Ordering::Equal);
    assert_eq!(by_proc_column(ProcSort::Cpu, &frames(2, 9.0), &frames(1, 5.0)), Ordering::Less);
    assert_eq!(by_proc_column(ProcSort::Cpu, &frames(2, 5.0), &frames(1, 5.0)), Ordering::Greater);
}
