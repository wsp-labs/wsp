// SPDX-License-Identifier: AGPL-3.0-only
//! Deep input audit (wsp-labs/wsp-map#2448): input nested past what a walk can hold must be answered, never abort the host.
//! A stack overflow kills the whole process, so each case runs its body in a child copy of this test binary on a thread
//! with a 1 MiB stack, the size the host's threads would run it on, and fails when the child dies.
//! The words crate writes a host reply whole, and a reply the command line reads can nest to the catalog port's 500 levels.
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

use serde_json::Value;
use wsp_words::exit::json_line;

fn nested(depth: usize, object: bool) -> Value {
    let mut v = Value::Null;
    for _ in 0..depth {
        v = if object { Value::Object([("a".to_string(), v)].into_iter().collect()) } else { Value::Array(vec![v]) };
    }
    v
}

fn json_line_body(depth: usize) {
    for object in [false, true] {
        let v = nested(depth, object);
        let (open, close, key) = if object { ("{", "}", "\"a\":") } else { ("[", "]", "") };
        let compact = format!("{}null{}", format!("{open}{key}").repeat(depth), close.repeat(depth));
        assert_eq!(json_line(&v, None), compact);
        if depth <= 2_000 {
            let pretty = json_line(&v, Some(2));
            let lines: Vec<&str> = pretty.lines().collect();
            let leaf = if object { "\"a\": null" } else { "null" };
            assert_eq!(lines.len(), 2 * depth + 1);
            assert_eq!(lines[depth], format!("{}{leaf}", "  ".repeat(depth)));
            assert_eq!(lines[depth + 1], format!("{}{close}", "  ".repeat(depth - 1)));
            assert_eq!(lines[2 * depth], close);
        }
        std::mem::forget(v);
    }
}

// exit.rs write_json recursed once per level, over a reply the command line kept whole to 500 levels (ruling 1 of the round 2
// brief: one cap of 500 levels for the whole host). JSON.stringify answers at any depth up to its own RangeError (about
// 3,650 levels).
#[test]
fn json_line_writes_a_reply_500_deep() {
    in_child("json_line_writes_a_reply_500_deep", 500, json_line_body);
}

#[test]
fn json_line_writes_a_reply_20000_deep() {
    in_child("json_line_writes_a_reply_20000_deep", 20_000, json_line_body);
}
