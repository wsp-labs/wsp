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
        let out = try_activity_table(&view, TableOpts { width: None, wide });
        assert!(out.is_err(), "wide={wide}: node's line fails with RangeError at this depth");
    }
}

fn reply_body(depth: usize) {
    let text = format!(r#"{{"id":1,"ok":true,"values":{}}}"#, nested(depth));
    let read = parse_frame(&text).expect("node's JSON.parse reads it");
    if depth <= 1_000 {
        assert_eq!(stringify(&read), text, "node reads and writes it whole");
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
// fails with exit 1 and `wsp activity: Maximum call stack size exceeded`.
#[test]
fn a_process_chain_20000_deep_fails_the_line_not_the_process() {
    in_child("a_process_chain_20000_deep_fails_the_line_not_the_process", 20_000, table_chain_past_node_body);
}

// json_whole.rs:28 `parse_frame` falls to wsp-catalog's JSON.parse port (json_parse.rs, MAX_DEPTH 1000) for any reply
// past serde_json's 128, and the port dies at 876, under its own cap. Fed by every reply the host sends (a slate value an
// agent stored up to the slates' 500 bound reads; one past 876 cannot reach here today). node's JSON.parse has no depth
// limit and reads the frame at any depth.
#[test]
fn a_reply_1000_deep_reads_whole() {
    in_child("a_reply_1000_deep_reads_whole", 1_000, reply_body);
}

#[test]
fn a_reply_200000_deep_is_read_not_an_abort() {
    in_child("a_reply_200000_deep_is_read_not_an_abort", 200_000, reply_body);
}

// verbs/slate.rs:114 `js_value` recurses once per level over a `wsp slate state $x=<json>` word (the port reads it whole
// to 1,000). Dies at 496. node reads and sends the word whole to 3,610 levels, where JSON.stringify throws.
#[test]
fn a_state_word_1000_deep_is_sent_whole() {
    in_child("a_state_word_1000_deep_is_sent_whole", 1_000, state_word_body);
}
