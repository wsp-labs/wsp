// SPDX-License-Identifier: AGPL-3.0-only
//! Ruling 3 of round 2 of #2448: the history reader's parse keeps a tree of nested objects to 500 levels, and a clone and
//! a drop of one that deep hold on a 1 MiB debug stack (a clone of objects dies at 628 levels). The readers that clone a
//! part of one are in `review_2448.rs`; this reads the whole tree, clones and drops it, in a child copy of this binary.
use wsp_collect::history::reader::try_json;

const CHILD: &str = "CLONE_AT_CAP_CHILD";

fn in_child(name: &str, body: fn()) {
    if std::env::var(CHILD).as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the whole process died ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
    );
}

fn tree_body() {
    let text = format!("{}1{}", "{\"a\":".repeat(500), "}".repeat(500));
    let v = try_json(&text).expect("the cap reads it");
    let copy = v.clone();
    assert_eq!(v, copy);
    drop(copy);
    drop(v);
}

#[test]
fn a_history_line_of_500_object_levels_clones_and_drops() {
    in_child("a_history_line_of_500_object_levels_clones_and_drops", tree_body);
}
