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

mod common;

use common::{run, writer, Home};
use serde_json::Value;
use wsp_store::sqlite_store::sqlite_store;
use wsp_store::store::Store;

fn nested(depth: usize) -> Value {
    let mut v = Value::Null;
    for _ in 0..depth {
        v = Value::Array(vec![v]);
    }
    v
}

fn put_body(depth: usize) {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    let value = nested(depth);
    let _ = run(store.put("things", "a", value));
}

// Dies at 1543. sqlite_store.rs:443 put writes the record with wsp_wire::text::json_stringify (text.rs:187 write_value), which
// recurses once per level of the value it is given, and the store takes any Value. Fed by every record that holds a
// free-form value (github_cache.rs body, a provider's answer; place_file's joinedAt; the slates' documents). Node's
// JSON.stringify of the same record throws a RangeError (from about 3,650 levels) that the caller of the put receives.
#[test]
fn sqlite_put_of_a_deep_record() {
    in_child("sqlite_put_of_a_deep_record", 100_000, put_body);
}

fn put_answers_body(depth: usize) {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    run(store.put("things", "a", nested(depth))).expect("the record is stored");
    assert_eq!(run(store.keys("things")).expect("the keys"), vec!["a".to_string()]);
}

// `put` answers Ok for a record 100,000 levels deep (node's JSON.stringify throws a RangeError from about 3,650 levels:
// ruling 7 leaves the answer to the Rust), and drops the record it was given without a level of the stack per level.
#[test]
fn sqlite_put_of_a_deep_record_answers() {
    in_child("sqlite_put_of_a_deep_record_answers", 100_000, put_answers_body);
}
