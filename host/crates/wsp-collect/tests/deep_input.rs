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

use wsp_collect::detect::logins::api_key_helper_of;
use wsp_collect::history::reader::try_json;
use wsp_collect::project::reader::ProjectFile;
use wsp_collect::project::readers::package_json;

fn nested(depth: usize) -> String {
    format!(r#"{{"x":{}{}}}"#, "[".repeat(depth), "]".repeat(depth))
}

fn package_json_body(depth: usize) {
    let file = ProjectFile { path: "package.json".into(), text: nested(depth) };
    let found = package_json::read(&file);
    assert!(found.is_empty(), "node reads it and finds nothing in it");
}

// Dies at 875. Every feeder of wsp_catalog::json_parse::parse_json (the case in wsp-catalog/tests/deep_input.rs holds the recursion's
// number: dies at 878 levels). This one is the project scan: a package.json in any folder a project holds, which a
// repo's own authors write. Node's JSON.parse reads 1,000,000 levels, so the scan finds nothing in it and goes on.
// The text is 10,000 levels, 20 KB.
#[test]
fn package_json_scan_of_a_deep_file() {
    in_child("package_json_scan_of_a_deep_file", 10_000, package_json_body);
}

fn api_key_helper_body(depth: usize) {
    assert_eq!(api_key_helper_of(Some(&nested(depth))), None);
}

// Dies at 876. The same parse for the sign-in scan's read of ~/.claude/settings.json (detect/logins.rs:134, and :167 for the other
// sign-in files).
#[test]
fn sign_in_scan_of_a_deep_settings_file() {
    in_child("sign_in_scan_of_a_deep_settings_file", 10_000, api_key_helper_body);
}

fn try_json_body(depth: usize) {
    assert!(try_json(&nested(depth)).is_none(), "past the cap, the one line is not read");
}

// Dies at 877. history/reader.rs:81 try_json, which every history reader (claude, codex, hermes, the cache file) reads each line of a
// harness's session file through: a tool call an agent made, with an input nested this deep, is one such line.
#[test]
fn history_line_of_a_deep_tool_input() {
    in_child("history_line_of_a_deep_tool_input", 10_000, try_json_body);
}

fn try_json_cap_body(depth: usize) {
    assert!(try_json(&nested(depth - 1)).is_some(), "{depth} levels read");
    assert!(try_json(&nested(depth)).is_none(), "{} levels do not", depth + 1);
}

// The cap of the catalog's JSON.parse port as the history reader meets it, on the 1 MiB stack: 1,000 levels read and 1,001 do
// not (ruling 2).
#[test]
fn try_json_reads_1000_levels_and_refuses_1001() {
    in_child("try_json_reads_1000_levels_and_refuses_1001", 1000, try_json_cap_body);
}
