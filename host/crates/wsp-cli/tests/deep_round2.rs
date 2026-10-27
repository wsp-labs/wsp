// SPDX-License-Identifier: AGPL-3.0-only
//! Round 2 of wsp-labs/wsp-map#2448 (wsp-cli): the activity table over a process list that holds a pid twice, a chain of agents
//! folded in linear time, and the trees the readers keep cloned and dropped on a 1 MiB stack.
use std::time::{Duration, Instant};

use wsp_cli::json_whole::{parse_document, parse_frame, parse_value};
use wsp_cli::verbs::activity_table::{try_activity_table, TableOpts};
use wsp_wire::views::activity::ActivityView;

/// Runs `name` again in a child copy of this binary on a 1 MiB thread, under a 2 GB memory cap and a 30 s limit, and fails
/// when the child dies or does not end.
fn in_child(name: &str, body: fn()) {
    if std::env::var("ROUND2_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let mut child = std::process::Command::new("sh")
        .args(["-c", "ulimit -v 2000000; exec \"$0\" \"$@\""])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("ROUND2_2448_CHILD", name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut why = String::new();
            std::io::Read::read_to_string(&mut child.stderr.take().unwrap(), &mut why).unwrap();
            let why: Vec<&str> =
                why.lines().filter(|l| l.contains("linear") || l.contains("overflow") || l.contains("left:")).take(2).collect();
            assert!(status.success(), "the child failed ({status}) after {:?}: {}", start.elapsed(), why.join(" | "));
            return;
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the case did not end in 30 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn view_of(processes: Vec<String>) -> ActivityView {
    let view = format!(
        r#"{{"computer":"here","at":1760000010000,"wsp":[],"unowned":[],"threads":[{{"threadId":"thread-7f3a9c21e4b5","title":"t","workspaceId":"ws-1","memory":2048,"cpu":3,"processes":[{}]}}]}}"#,
        processes.join(",")
    );
    serde_json::from_str(&view).expect("an activity view")
}

fn proc(pid: u32, ppid: u32, kind: &str) -> String {
    format!(
        r#"{{"pid":{pid},"ppid":{ppid},"depth":0,"startedAt":1700000000000,"kind":"{kind}","name":"x","command":"claude --resume x","memory":1024,"cpu":0.5,"ports":[]}}"#
    )
}

fn table(view: &ActivityView) -> Result<String, String> {
    try_activity_table(view, TableOpts { width: None, wide: false }).map_err(|e| e.message)
}

// activity_table.rs `tree_of` builds its tree with wsp-wire's `fold_tree`, which never ended on a list that holds a pid twice
// inside a parent cycle: pid 1 under 99, 2 under 1, and 1 again under 2. Ruling 4 of the round 2 brief: the cycle is cut and
// the rest of the table is answered (a row for 1, 2 and 5).
fn cycle_body() {
    let view = view_of(vec![proc(1, 99, "tool"), proc(2, 1, "tool"), proc(1, 2, "tool"), proc(5, 1, "tool")]);
    let table = table(&view).expect("a table");
    assert_eq!(table.split('\n').count(), 3 + 4, "a row for each of 1, 2 and 5, and four more: {table}");
}

#[test]
fn a_table_over_a_pid_listed_twice_in_a_parent_cycle_ends() {
    in_child("a_table_over_a_pid_listed_twice_in_a_parent_cycle_ends", cycle_body);
}

// Two agents that name each other as parent are what node's recursion overflows on; the memo `agent_below` keeps does not
// change that.
#[test]
fn two_agents_naming_each_other_as_parent_fail_the_line_as_node_does() {
    let view = view_of(vec![proc(1, 2, "agent"), proc(2, 1, "agent")]);
    assert_eq!(table(&view), Err("Maximum call stack size exceeded".to_string()));
}

// activity_table.rs `agent_below`: every agent of a chain of agents walked the chain below it, so the table cost the square of
// the chain (the TS does the same). Doubling the chain must not quadruple the time. m3 of the review.
fn agent_chain(n: u32) -> ActivityView {
    view_of((0..n).map(|i| proc(i + 100, i + 99, "agent")).collect())
}

fn table_in(n: u32) -> Duration {
    let view = agent_chain(n);
    let at = Instant::now();
    let table = table(&view).expect("a table");
    let took = at.elapsed();
    assert!(table.lines().count() >= 4);
    took
}

fn agent_chain_body() {
    let best = |n: u32| (0..3).map(|_| table_in(n)).min().unwrap();
    let n: u32 = std::env::var("ROUND2_2448_AGENTS").ok().and_then(|n| n.parse().ok()).unwrap_or(5_000);
    let small = best(n).max(Duration::from_millis(20));
    let large = best(2 * n);
    eprintln!("{n} agents took {small:?}, {} took {large:?}", 2 * n);
    assert!(large < small * 3, "{} agents took {large:?} where {n} took {small:?}: not linear", 2 * n);
}

#[test]
fn a_table_over_a_chain_of_agents_costs_what_the_chain_costs() {
    in_child("a_table_over_a_chain_of_agents_costs_what_the_chain_costs", agent_chain_body);
}

// ruling 3 of the round 2 brief: each reader of json_whole reads a tree of 500 levels of objects, and the clones the command line makes
// of what it reads (a pushed frame is held with `frame.clone()` in client/socket.rs, setup_follow.rs copies a frame as
// `Value::Object(frame.clone())`) hold on a 1 MiB stack, where a Value of objects dies in clone at 628 levels.
fn objects(levels: usize) -> String {
    format!("{}1{}", "{\"a\":".repeat(levels), "}".repeat(levels))
}

fn frame_clone_body() {
    let text = format!(r#"{{"id":1,"values":{}}}"#, objects(499));
    let serde_json::Value::Object(frame) = parse_frame(&text).expect("a frame of 500 levels is read") else { panic!("an object") };
    let held = frame.clone();
    assert_eq!(held, frame);
    drop(held);
    drop(frame);
}

#[test]
fn a_frame_of_500_object_levels_is_cloned_and_dropped() {
    in_child("a_frame_of_500_object_levels_is_cloned_and_dropped", frame_clone_body);
}

fn document_clone_body() {
    let read = parse_document(&objects(500)).expect("a document of 500 levels is read");
    let held = read.clone();
    assert_eq!(held, read);
    drop(held);
    drop(read);
}

#[test]
fn a_document_of_500_object_levels_is_cloned_and_dropped() {
    in_child("a_document_of_500_object_levels_is_cloned_and_dropped", document_clone_body);
}

fn word_clone_body() {
    let read = parse_value(&objects(500)).expect("a word of 500 levels is read");
    let held = read.clone();
    assert_eq!(held, read);
    drop(held);
    drop(read);
}

#[test]
fn a_state_word_of_500_object_levels_is_cloned_and_dropped() {
    in_child("a_state_word_of_500_object_levels_is_cloned_and_dropped", word_clone_body);
}
