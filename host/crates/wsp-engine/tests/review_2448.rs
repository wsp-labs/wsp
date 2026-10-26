// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448).
use base64::Engine;
use wsp_engine::machine_context::parse_facts;

// machine_context/probe.rs:136 `items` clones each element of a facts list before `from_value`. The FACTS line of the
// guest's machine-context.json reaches it through the 1,000-level port, and a thread's work on that machine can write it.
// A Value of objects clones to 627 levels on a 1 MiB debug stack. node (machine-context.ts:93) reads the arrays as given.
fn facts_700_objects_body() {
    let d = 700;
    let json = format!(r#"{{"tools":[{{"id":"a","label":"b","note":{}1{}}}],"agents":[],"files":[]}}"#, "{\"a\":".repeat(d), "}".repeat(d));
    assert!(parse_facts(&base64::engine::general_purpose::STANDARD.encode(json)).is_some());
}

#[test]
fn parse_facts_with_a_note_700_object_levels_deep() {
    let name = "parse_facts_with_a_note_700_object_levels_deep";
    if std::env::var("REVIEW_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(facts_700_objects_body).unwrap().join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("REVIEW_2448_CHILD", name)
        .output()
        .unwrap();
    assert!(out.status.success(), "the whole process died ({}): {}", out.status, String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or(""));
}
