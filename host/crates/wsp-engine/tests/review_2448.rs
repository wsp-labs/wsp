// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448).
use base64::Engine;
use wsp_engine::machine_context::parse_facts;

// machine_context/probe.rs:136 `items` clones each element of a facts list before `from_value`. The FACTS line of the
// guest's machine-context.json reaches it through the 500-level port, and a thread's work on that machine can write it.
// A Value of objects clones to 627 levels on a 1 MiB debug stack. node (machine-context.ts:93) reads the arrays as given.
fn facts_cap_objects_body() {
    let d = 497;
    let json = format!(r#"{{"tools":[{{"id":"a","label":"b","note":{}1{}}}],"agents":[],"files":[]}}"#, "{\"a\":".repeat(d), "}".repeat(d));
    assert!(parse_facts(&base64::engine::general_purpose::STANDARD.encode(json)).is_some());
}

#[test]
fn parse_facts_with_a_note_object_levels_to_the_cap_deep() {
    let name = "parse_facts_with_a_note_object_levels_to_the_cap_deep";
    if std::env::var("REVIEW_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(facts_cap_objects_body).unwrap().join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("REVIEW_2448_CHILD", name)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the whole process died ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
    );
}

// m2 of the review, for the engine's `walk_files`: std's `DirEntry` keeps its folder open while it lives, so a walk that
// held each level's entries held a descriptor per level. node's `readdirSync` holds none. A tree 300 deep with files beside
// each folder is walked under `ulimit -n 200`.
#[test]
fn a_tree_300_deep_with_files_beside_each_folder_is_walked_under_200_descriptors() {
    let name = "a_tree_300_deep_with_files_beside_each_folder_is_walked_under_200_descriptors";
    if std::env::var("REVIEW_2448_CHILD").is_err() {
        let out = std::process::Command::new("sh")
            .args(["-c", "ulimit -n 200; exec \"$0\" \"$@\""])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env("REVIEW_2448_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr).lines().find(|l| l.contains("Err") || l.contains("panicked")).unwrap_or("")
        );
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let mut at = root.path().join("r");
    for _ in 0..300 {
        std::fs::create_dir_all(&at).unwrap();
        for f in 0..3 {
            std::fs::write(at.join(format!("f{f}.jsonl")), "{}\n").unwrap();
        }
        at = at.join("d");
    }
    let found = wsp_engine::project_state::files_under(root.path().to_str().unwrap(), ".jsonl").expect("walked");
    assert_eq!(found.len(), 900);
}

// n2 of the review: node's `readdirSync(dir, { recursive: true })` follows a link that points at its own folder and stops
// quietly where the kernel gives ELOOP, so a folder with one file and such a link holds 41 files there (node 22.23.4).
#[test]
fn a_folder_holding_a_link_to_itself_is_walked_as_node_walks_it() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("r");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("f.jsonl"), "{}\n").unwrap();
    std::os::unix::fs::symlink(".", dir.join("l")).unwrap();
    let found = wsp_engine::project_state::files_under(dir.to_str().unwrap(), ".jsonl").expect("walked");
    assert_eq!(found.len(), 41);
}
