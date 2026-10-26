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
// wsp-cli is the command line; nothing in the host links it, so a death here ends one `wsp` line (or a script driving
// it), not the host. Node runs the same line on its default stack (about 984 KB).

use wsp_cli::json_whole::{parse_frame, stringify};
use wsp_cli::verbs::activity_table::{try_activity_table, TableOpts};
use wsp_cli::verbs::slate::state_value;
use wsp_wire::views::activity::ActivityView;

fn nested(depth: usize) -> String {
    format!("{}1{}", "[".repeat(depth), "]".repeat(depth))
}

/// One thread whose processes are a chain `depth` long, each the parent of the next.
fn chain_view(depth: usize) -> ActivityView {
    let procs: Vec<String> = (0..depth)
        .map(|i| {
            format!(
                r#"{{"pid":{},"ppid":{},"depth":0,"startedAt":1700000000000,"kind":"tool","name":"sh","command":"sh -c x","memory":1024,"cpu":0.5,"ports":[]}}"#,
                i + 100,
                i + 99
            )
        })
        .collect();
    let view = format!(
        r#"{{"computer":"here","at":1760000010000,"wsp":[],"unowned":[],"threads":[{{"threadId":"thread-7f3a9c21e4b5","title":"t","workspaceId":"ws-1","memory":2048,"cpu":3,"processes":[{}]}}]}}"#,
        procs.join(",")
    );
    serde_json::from_str(&view).expect("an activity view")
}

fn table_chain_body(depth: usize) {
    let view = chain_view(depth);
    for wide in [false, true] {
        let table = try_activity_table(&view, TableOpts { width: None, wide }).expect("node prints the table at this depth");
        assert_eq!(table.lines().next(), Some(format!("here   wsp 2 KB   CPU 3%   {depth} processes   1 thread").as_str()));
        assert_eq!(table.split('\n').count(), depth + 4, "wide={wide}: node's table has a row per process and four more");
    }
}

fn table_chain_past_node_body(depth: usize) {
    let view = chain_view(depth);
    for wide in [false, true] {
        let table = try_activity_table(&view, TableOpts { width: None, wide }).expect("a chain has no cap, so the table is printed");
        assert_eq!(table.split('\n').count(), depth + 4, "wide={wide}: a row per process and four more");
    }
}

fn reply_body(depth: usize) {
    let text = format!(r#"{{"id":1,"ok":true,"values":{}}}"#, nested(depth));
    let read = parse_frame(&text);
    if depth < 500 {
        assert_eq!(stringify(&read.expect("a frame under the cap is read")), text, "node reads and writes it whole");
    } else {
        assert_eq!(read, None, "a frame past the cap is not read, and the process holds");
    }
}

fn state_word_body(depth: usize) {
    let (path, value) = state_value(&format!("$x={}", nested(depth))).expect("a path and a value");
    assert_eq!(path, "$x");
    assert_eq!(stringify(&value), nested(depth), "node sends the value whole");
}

// activity_table.rs:157 `node_of`, :177 `walk` and wsp-wire activity_fold.rs `fold_tree` (which dies first, at 650 alone)
// recurse once per level of the process tree, uncapped. Fed by the host's `activity.read` answer: the processes each
// computer's daemon reports. Dies at 457 on a 1 MiB stack. node prints the table to 1,484 levels.
#[test]
fn a_process_chain_1000_deep_prints_its_table() {
    in_child("a_process_chain_1000_deep_prints_its_table", 1_000, table_chain_body);
}

// The same past node's depth: node's treeOf/procRows throw RangeError from 1,485 (folded) and 1,550 (--wide), and the line
// fails with exit 1 and `wsp activity: Maximum call stack size exceeded`. Ruling 7: past node's own RangeError depth the
// loop may answer, and it prints the table; this test expected the failed line.
#[test]
fn a_process_chain_20000_deep_fails_the_line_not_the_process() {
    in_child("a_process_chain_20000_deep_fails_the_line_not_the_process", 20_000, table_chain_past_node_body);
}

// Ruling 1 of the round 2 brief: one cap of 500 levels for the whole host. The frame holds a level more than `depth` (its
// own object), so a reply 499 deep is 500 levels, the cap of the catalog's port, and is read whole.
// json_whole.rs `parse_frame` falls to wsp-catalog's JSON.parse port for any reply past serde_json's 128. Fed by every
// reply the host sends. node's JSON.parse has no depth limit and reads the frame at any depth.
#[test]
fn a_reply_499_deep_reads_whole() {
    in_child("a_reply_499_deep_reads_whole", 499, reply_body);
}

// Past the cap the read fails (m1 of the review: "past the cap the read must fail, no lax fallback tree"); it answers, and the
// process holds.
#[test]
fn a_reply_200000_deep_is_refused_not_an_abort() {
    in_child("a_reply_200000_deep_is_refused_not_an_abort", 200_000, reply_body);
}

// verbs/slate.rs:114 `js_value` recurses once per level over a `wsp slate state $x=<json>` word (the port reads it whole
// to 500, ruling 1 of the round 2 brief). Dies at 496. node reads and sends the word whole to 3,610 levels, where JSON.stringify
// throws.
#[test]
fn a_state_word_500_deep_is_sent_whole() {
    in_child("a_state_word_500_deep_is_sent_whole", 500, state_word_body);
}

fn reply_cap_body(depth: usize) {
    let frame = |levels: usize| format!(r#"{{"id":1,"values":{}}}"#, nested(levels - 1));
    let read = parse_frame(&frame(depth)).expect("a frame at the cap is read");
    assert_eq!(stringify(&read), frame(depth), "{depth} levels read whole");
    assert_eq!(parse_frame(&frame(depth + 1)), None, "{} levels are refused, not read as a shallower tree", depth + 1);
}

// The cap of the catalog's port as a reply meets it, on the 1 MiB stack: 500 levels read whole, and 501 are refused. The
// review's m1: "past the cap the read must fail, no lax fallback tree"; this test asserted the shallower tree (`assert_ne!`).
#[test]
fn a_reply_reads_500_levels_whole_and_not_501() {
    in_child("a_reply_reads_500_levels_whole_and_not_501", 500, reply_cap_body);
}

fn chain_time_body(depth: usize) {
    let time = |n: usize| {
        let view = chain_view(n);
        let start = std::time::Instant::now();
        let table = try_activity_table(&view, TableOpts { width: None, wide: false }).expect("the table is printed");
        assert_eq!(table.split('\n').count(), n + 4);
        start.elapsed()
    };
    let small = time(depth / 4).max(std::time::Duration::from_millis(50));
    let large = time(depth);
    assert!(large < small * 10, "{depth} processes took {large:?} where {} took {small:?}: not linear", depth / 4);
}

// A 100,000-process chain prints its table on the 1 MiB stack, and four times the processes take well under ten times as long
// (a table per level of the chain would be sixteen times). Ruling 5.
#[test]
fn a_process_chain_100000_deep_prints_its_table_in_linear_time() {
    in_child("a_process_chain_100000_deep_prints_its_table_in_linear_time", 100_000, chain_time_body);
}

fn thread_chain_body(depth: usize) {
    use wsp_cli::verbs::workspaces_help::reads::ThreadRow;
    use wsp_cli::verbs::workspaces_help::rows::thread_tree;
    let rows: Vec<ThreadRow> = (0..depth)
        .map(|i| {
            let mut row = serde_json::json!({
                "id": format!("t-{i}"), "workspaceId": "ws-1", "harness": "claude", "startedBy": "person", "status": "completed",
                "title": "t", "sessionId": format!("s-{i}"), "turns": 1, "ran": true, "projectName": "wsp", "folder": "/root/wsp",
                "branch": "main", "computerName": "attic"
            });
            if i > 0 {
                row["parentThreadId"] = serde_json::json!(format!("t-{}", i - 1));
            }
            serde_json::from_value(row).expect("a thread row")
        })
        .collect();
    let tree = thread_tree(&rows);
    assert_eq!(tree.len(), depth);
    assert!(tree.iter().enumerate().all(|(i, n)| n.depth == i && n.row.thread.id == format!("t-{i}")), "one row a level, in order");
}

// verbs/workspaces_help/rows.rs `thread_tree`: threads in a chain of parents, 20,000 deep, listed in a tree. node's threadTree
// recurses once per level, and reads about as deep as the table does.
#[test]
fn a_thread_chain_20000_deep_lists_as_a_tree() {
    in_child("a_thread_chain_20000_deep_lists_as_a_tree", 20_000, thread_chain_body);
}

fn agent_chain_body(depth: usize) {
    let procs: Vec<String> = (0..depth)
        .map(|i| {
            format!(
                r#"{{"pid":{},"ppid":{},"depth":0,"startedAt":1700000000000,"kind":"agent","name":"claude","command":"claude --resume x","memory":1024,"cpu":0.5,"ports":[]}}"#,
                i + 100,
                i + 99
            )
        })
        .collect();
    let view = format!(
        r#"{{"computer":"here","at":1760000010000,"wsp":[],"unowned":[],"threads":[{{"threadId":"thread-7f3a9c21e4b5","title":"t","workspaceId":"ws-1","memory":2048,"cpu":3,"processes":[{}]}}]}}"#,
        procs.join(",")
    );
    let view: ActivityView = serde_json::from_str(&view).expect("an activity view");
    let table = try_activity_table(&view, TableOpts { width: None, wide: false }).expect("a chain of agents is folded into one row");
    assert!(table.lines().count() >= 4);
}

// activity_table.rs `agent_below` recursed once per level of a chain of agents under agents, folding each into the one below.
#[test]
fn a_chain_of_2000_agents_folds_into_the_last() {
    in_child("a_chain_of_2000_agents_folds_into_the_last", 2_000, agent_chain_body);
}

fn state_cap_body(depth: usize) {
    let (_, value) = state_value(&format!("$x={}", nested(depth))).expect("a path and a value");
    assert_eq!(stringify(&value), nested(depth), "{depth} levels are sent whole");
    let (_, value) = state_value(&format!("$x={}", "[".repeat(depth + 1) + "1" + &"]".repeat(depth + 1))).expect("a path and a value");
    assert_ne!(stringify(&value), nested(depth + 1), "{} levels are not read whole", depth + 1);
}

// verbs/slate.rs `state_value` at the cap of the port, on the 1 MiB stack (ruling 1 of the round 2 brief).
#[test]
fn a_state_word_reads_500_levels_whole_and_not_501() {
    in_child("a_state_word_reads_500_levels_whole_and_not_501", 500, state_cap_body);
}
