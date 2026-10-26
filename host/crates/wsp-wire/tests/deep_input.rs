// SPDX-License-Identifier: AGPL-3.0-only
//! Deep input audit (wsp-labs/wsp-map#2448): input nested past what a walk can hold must be answered, never abort the host.
//! A stack overflow kills the whole process, so each case runs its body in a child copy of this test binary on a thread
//! with a 1 MiB stack, the size the host's threads would run it on, and fails when the child dies.
//! `DEEP_AUDIT_DEPTH` overrides a case's depth, which is how the depth each one dies at was found.

const CHILD: &str = "DEEP_AUDIT_CHILD";
const DEPTH: &str = "DEEP_AUDIT_DEPTH";
const HOST_THREAD_STACK: usize = 1024 * 1024;

/// Runs `body(depth)` in a child copy of this binary and fails when the child dies, as a stack overflow kills a host.
fn in_child(name: &str, depth: usize, body: fn(usize)) {
    let depth = std::env::var(DEPTH).ok().and_then(|d| d.parse().ok()).unwrap_or(depth);
    if std::env::var(CHILD).as_deref() == Ok(name) {
        std::thread::Builder::new()
            .stack_size(HOST_THREAD_STACK)
            .spawn(move || body(depth))
            .expect("a thread")
            .join()
            .expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().expect("this binary"))
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .env(DEPTH, depth.to_string())
        .output()
        .expect("the child starts");
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).rev().take(3).collect();
        panic!(
            "{depth} deep: the whole process died ({}) instead of answering: {}",
            out.status,
            tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        );
    }
}

// ---- cases ----
//
// Each case: a `fn <case>_body(depth: usize)` that builds the input `depth` deep, calls the code under test and asserts
// the answer node gives for the same input (an answer, a refusal, or a failed call, which in Rust is an Err), then a
// #[test] that calls in_child with the test's exact name and a depth past where the code dies. The comment above the
// #[test] names the recursion (file:line), what reaches it from outside the host, the depth it dies at on a 1 MiB stack
// in a debug build, and what node answers at the test's depth.

use wsp_frames::{ProcEntry, ProcOwnedBy, ProcOwner};
use wsp_wire::views::activity::{activity_of, place_procs, ActivityRead, ActivityScope, PlaceInput, PlaceThread};
use wsp_wire::views::activity_fold::fold_tree;

fn proc_row(pid: u32, ppid: u32) -> ProcEntry {
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
        owner: None,
        kind: None,
        id: None,
    }
}

/// Each process the parent of the next: pid 1 first (its parent is not in the list), pid `depth` last. `deepest_first`
/// lists them the other way round.
fn chain(depth: usize, deepest_first: bool, tweak: impl Fn(&mut ProcEntry)) -> Vec<ProcEntry> {
    let mut rows: Vec<ProcEntry> = (1..=depth as u32)
        .map(|pid| {
            let mut p = proc_row(pid, pid - 1);
            tweak(&mut p);
            p
        })
        .collect();
    if deepest_first {
        rows.reverse();
    }
    rows
}

fn place_input<'a>(
    procs: &'a [ProcEntry],
    threads: &'a [PlaceThread],
    bins: &'a [String],
    listens: &'a dyn Fn(u32) -> bool,
) -> PlaceInput<'a> {
    PlaceInput { procs, daemon: 999_999_999, host: None, threads, agent_bins: bins, listens, app: None }
}

fn read_of(procs: Vec<ProcEntry>, scopes: Vec<ActivityScope>) -> ActivityRead {
    ActivityRead {
        computer: "box".into(),
        at: wsp_wire::types::JsNumber(1.0),
        procs,
        daemon: 999_999_999,
        host: 999_999_998,
        scopes,
        agent_bins: vec!["claude".into()],
        ports: None,
        cgroups: None,
    }
}

fn place_row_body(depth: usize) {
    let threads = [PlaceThread { id: "t1".into(), cgroup: Some("/wsp/t1".into()), ..Default::default() }];
    let procs = chain(depth, true, |p| {
        if p.pid == 1 {
            p.cgroup = Some("/wsp/t1".into());
        }
    });
    let bins = vec![];
    let listens = |_: u32| false;
    let input = place_input(&procs, &threads, &bins, &listens);
    let placed = place_procs(&input);
    assert_eq!(placed.whose.len(), depth, "every process in the chain is the thread's, through its parents");
}

// Dies at 1265 processes (1 MiB, debug). processes_choices.rs:1119 place_row recurses once per parent up a chain of processes none of which names a thread
// itself (each takes its parent's answer), cloning the seen set at every level. Fed by a daemon's process list
// (proc.list of a computer or box, read for `wsp activity`, End, settle and thread delete), so a chain of processes
// each the parent of the next on that computer. Node's placeProcs/activityOf walks it as a recursive function that
// fails with a RangeError from 4,556 chain levels (the call fails, the host stays up); at 10,000 it throws.
#[test]
fn place_row_over_a_process_chain() {
    in_child("place_row_over_a_process_chain", 10_000, place_row_body);
}

fn kind_of_body(depth: usize) {
    let threads: [PlaceThread; 0] = [];
    let procs = chain(depth, false, |p| p.pty = Some("pts/0".into()));
    let bins = vec![];
    let listens = |_: u32| false;
    let input = place_input(&procs, &threads, &bins, &listens);
    let placed = place_procs(&input);
    let _ = placed.kind_of(depth as u32);
}

// Dies at 2601. processes_choices.rs:736 kind_of reads its parent's kind first, so one read of the last process of a chain recurses
// once per process above it. Reached by tree_rows for every row of a view, and by End's plan.
#[test]
fn kind_of_over_a_process_chain() {
    in_child("kind_of_over_a_process_chain", 10_000, kind_of_body);
}

fn tree_walk_body(depth: usize) {
    let procs = chain(depth, false, |p| p.pty = Some("pts/0".into()));
    let view = activity_of(&read_of(procs, vec![]));
    assert_eq!(view.unowned.len(), depth);
}

// Dies at 1581. rows.rs:13 tree_walk recurses once per generation of a group's tree (and clones the list of kids at each). Fed by
// the same process list; activity_of draws the unowned group's, the wsp group's and each thread's tree.
#[test]
fn tree_walk_over_a_process_chain() {
    in_child("tree_walk_over_a_process_chain", 10_000, tree_walk_body);
}

fn activity_of_body(depth: usize) {
    let procs = chain(depth, true, |p| p.pty = Some("pts/0".into()));
    let view = activity_of(&read_of(procs, vec![]));
    assert_eq!(view.unowned.len(), depth);
}

// Dies at 1581. activity_of as the host calls it (proc.list order is the daemon's, here deepest first), through every walk at once.
// Node's activityOf of the same list throws a RangeError from 3,712 levels.
#[test]
fn activity_of_over_a_process_chain() {
    in_child("activity_of_over_a_process_chain", 10_000, activity_of_body);
}

fn reach_body(depth: usize) {
    let threads = [PlaceThread { id: "t1".into(), ..Default::default() }];
    let procs = chain(depth, false, |p| {
        p.owner = Some(ProcOwner { by: ProcOwnedBy::Env, thread: Some("t1".into()), workspace: None, warm: false });
    });
    let bins = vec![];
    let listens = |_: u32| false;
    let input = place_input(&procs, &threads, &bins, &listens);
    let placed = place_procs(&input);
    assert_eq!(placed.reach_of(depth as u32), None);
}

// Dies at 3419. processes_choices.rs:826 reach recurses once per parent while each process carries only the WSP_THREAD mark (owner
// by env), cloning the visited set at each level. Reached by End's plan (wsp-words end_plan) for a pid the person or
// an agent names.
#[test]
fn reach_of_over_a_marked_process_chain() {
    in_child("reach_of_over_a_marked_process_chain", 10_000, reach_body);
}

fn fold_body(depth: usize) {
    let rows = chain(depth, false, |_| {});
    let folded = fold_tree(&rows, |_| None);
    assert_eq!(folded.len(), 1);
    std::mem::forget(folded);
}

// Dies at 501 (ProcEntry rows; the lead measured 650 with ActivityProc). Node's foldTree throws a RangeError from 2,930.
// activity_fold.rs:112 build recurses once per generation (the result is cloned and dropped recursively too).
// Called from wsp-cli's activity table alone (the client printing a view the host already built).
#[test]
fn fold_tree_over_a_process_chain() {
    in_child("fold_tree_over_a_process_chain", 10_000, fold_body);
}

fn json_stringify_alone_body(depth: usize) {
    let mut v = serde_json::Value::Null;
    for _ in 0..depth {
        v = serde_json::Value::Array(vec![v]);
    }
    assert!(wsp_wire::text::json_stringify(&v).len() > depth);
    std::mem::forget(v);
}

// Dies at 1548. text.rs:187 write_value (json_stringify) alone, over a value built by a loop. The store's put, the zod issue text
// and every place the wire writes a free-form value reach it. Node's JSON.stringify throws a RangeError from about
// 3,650 levels.
#[test]
fn json_stringify_of_a_deep_value() {
    in_child("json_stringify_of_a_deep_value", 100_000, json_stringify_alone_body);
}

// ---- ruling 5 of the fix round: a process chain has no cap, and each walk over it is linear ----

const CHAIN: usize = 100_000;

fn timed<R>(what: &str, f: impl FnOnce() -> R) -> R {
    let at = std::time::Instant::now();
    let out = f();
    eprintln!("{what} over {CHAIN} processes: {:?}", at.elapsed());
    out
}

fn linear_activity_body(depth: usize) {
    let procs = chain(depth, true, |p| p.pty = Some("pts/0".into()));
    let view = timed("activity_of, deepest process first", || activity_of(&read_of(procs, vec![])));
    assert_eq!(view.unowned.len(), depth);
    assert_eq!(view.unowned[depth - 1].depth.get(), depth as i64 - 1);
}

#[test]
fn activity_of_a_100000_process_chain_is_linear() {
    in_child("activity_of_a_100000_process_chain_is_linear", CHAIN, linear_activity_body);
}

fn linear_threads_body(depth: usize) {
    let threads = [PlaceThread { id: "t1".into(), cgroup: Some("/wsp/t1".into()), ..Default::default() }];
    let procs = chain(depth, true, |p| {
        if p.pid == 1 {
            p.cgroup = Some("/wsp/t1".into());
        }
    });
    let bins = vec![];
    let listens = |_: u32| false;
    let input = place_input(&procs, &threads, &bins, &listens);
    let placed = timed("place_procs", || place_procs(&input));
    assert_eq!(placed.whose.len(), depth);
    let _ = timed("kind_of the last process", || placed.kind_of(depth as u32));
}

#[test]
fn place_procs_of_a_100000_process_chain_is_linear() {
    in_child("place_procs_of_a_100000_process_chain_is_linear", CHAIN, linear_threads_body);
}

fn linear_reach_body(depth: usize) {
    let threads = [PlaceThread { id: "t1".into(), ..Default::default() }];
    let procs = chain(depth, false, |p| {
        p.owner = Some(ProcOwner { by: ProcOwnedBy::Env, thread: Some("t1".into()), workspace: None, warm: false });
    });
    let bins = vec![];
    let listens = |_: u32| false;
    let input = place_input(&procs, &threads, &bins, &listens);
    let placed = place_procs(&input);
    assert_eq!(timed("reach_of", || placed.reach_of(depth as u32)), None);
}

#[test]
fn reach_of_a_100000_marked_process_chain_is_linear() {
    in_child("reach_of_a_100000_marked_process_chain_is_linear", CHAIN, linear_reach_body);
}

fn linear_fold_body(depth: usize) {
    let rows = chain(depth, false, |_| {});
    let folded = timed("fold_tree", || fold_tree(&rows, |_| None));
    assert_eq!(folded.len(), 1);
    let mut at = &folded[0];
    for _ in 1..depth {
        at = &at.kids[0];
    }
    assert_eq!(at.p.pid as usize, depth);
    timed("dropping the folded tree", move || drop(folded));
}

#[test]
fn fold_tree_of_a_100000_process_chain_is_linear_and_drops() {
    in_child("fold_tree_of_a_100000_process_chain_is_linear_and_drops", CHAIN, linear_fold_body);
}
