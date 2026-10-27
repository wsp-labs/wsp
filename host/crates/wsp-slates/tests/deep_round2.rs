// SPDX-License-Identifier: AGPL-3.0-only
//! Round 2 of wsp-labs/wsp-map#2448 (wsp-slates): a slate's own folder cleared without a descriptor per level (m4), and each way
//! a tree reaches a slate from outside, read at the 500-level cap, written, read back, cloned and dropped on a 1 MiB stack.
mod support;

use std::os::unix::process::ExitStatusExt;

use indexmap::IndexMap;
use serde_json::{json, Value};
use support::{as_thread, temp_root, FakeHost, MemStore, THREAD};
use wsp_slates::slates::{ReadParams, SlateTarget, Slates, StateParams, WriteParams};
use wsp_wire::views::workspace::Caller;

const CHILD: &str = "ROUND2_2448_CHILD";

/// Runs `name` again in a child copy of this binary on a 1 MiB thread (with `ulimit -n 200` where `files` is set) and fails
/// when the child dies or the case fails.
fn in_child(name: &str, files: bool, body: fn()) {
    if std::env::var(CHILD).as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let mut command = std::process::Command::new("sh");
    command.args(["-c", if files { "ulimit -n 200; exec \"$0\" \"$@\"" } else { "exec \"$0\" \"$@\"" }]);
    let out = command
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let why: Vec<&str> = stderr
        .lines()
        .filter(|l| l.contains("overflowed") || l.contains("left:") || l.contains("HostError") || l.contains("panicked"))
        .take(2)
        .collect();
    let how = if out.status.signal().is_some() { "the whole process died" } else { "the case failed" };
    assert!(out.status.success(), "{how} ({}): {}", out.status, why.join(" | "));
}

fn agent() -> Caller {
    as_thread(THREAD, THREAD)
}

fn target() -> SlateTarget {
    SlateTarget { thread_id: Some(THREAD.into()), turn_token: None }
}

fn records(n: usize, leaf: Value) -> Value {
    let mut v = leaf;
    for _ in 0..n {
        v = json!({ "a": v });
    }
    v
}

fn lists(n: usize, leaf: Value) -> Value {
    let mut v = leaf;
    for _ in 0..n {
        v = Value::Array(vec![v]);
    }
    v
}

const SMALL: &str = r#"<slate><value name="v" start={null} /><column><text>{len($v)}</text></column></slate>"#;

fn read_all() -> ReadParams {
    ReadParams { target: target(), values: Some(vec!["*".into()]), text: Some(true), sketch: Some(true), document: Some(true) }
}

// m4 of the review: the folder a slate's run writes under is cleared with std's `remove_dir_all`, which holds one descriptor per
// level (and recurses). A tree 300 deep with files beside each folder, under `ulimit -n 200`, is cleared by `forget` and by a
// rewrite of the slate's files, both through wsp-core's `remove_tree`.
fn deep_folder(under: &std::path::Path) {
    let mut at = under.join("tree");
    for _ in 0..300 {
        std::fs::create_dir_all(&at).unwrap();
        for f in 0..3 {
            std::fs::write(at.join(format!("f{f}")), "x").unwrap();
        }
        at = at.join("d");
    }
}

fn forget_body() {
    support::local(Box::pin(async move {
        support::own_group();
        let root = temp_root("r2-forget-");
        let host = FakeHost::new(root.path(), MemStore::new());
        let slates = host.slates();
        slates.write(WriteParams { text: Some(SMALL.into()), ..Default::default() }, Some(&agent())).await.expect("a write");
        slates.settled().await;
        let folder = host.slates_dir().join(THREAD);
        std::fs::create_dir_all(&folder).unwrap();
        deep_folder(&folder);
        slates.forget(THREAD, false).await.expect("forgotten");
        assert!(!folder.exists(), "the slate's folder is cleared");
    }));
}

#[test]
fn forgetting_a_slate_clears_a_folder_300_deep_under_200_descriptors() {
    in_child("forgetting_a_slate_clears_a_folder_300_deep_under_200_descriptors", true, forget_body);
}

fn rewrite_body() {
    let root = temp_root("r2-files-");
    let dir = root.path().join("slate");
    std::fs::create_dir_all(&dir).unwrap();
    deep_folder(&dir);
    wsp_slates::slate_files::write_slate_files(&dir, &IndexMap::new()).expect("the folder holds the document's files and nothing else");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
}

#[test]
fn rewriting_a_slates_files_clears_a_folder_300_deep_under_200_descriptors() {
    in_child("rewriting_a_slates_files_clears_a_folder_300_deep_under_200_descriptors", true, rewrite_body);
}

// ---- readers: a tree of 500 levels of objects reaches a slate by state, by a document, by text and by a derived value ----

fn written_and_read(text: String) {
    support::local(Box::pin(async move {
        support::own_group();
        let root = temp_root("r2-");
        let host = FakeHost::new(root.path(), MemStore::new());
        let slates = host.slates();
        slates.write(WriteParams { text: Some(text), ..Default::default() }, Some(&agent())).await.expect("a write");
        slates.settled().await;
        let read = slates.read(read_all(), Some(&agent())).await.expect("a read");
        let _ = serde_json::to_string(&read.to_json());
    }));
}

fn state_body() {
    support::local(Box::pin(async move {
        support::own_group();
        let root = temp_root("r2-state-");
        let host = FakeHost::new(root.path(), MemStore::new());
        let slates = host.slates();
        slates.write(WriteParams { text: Some(SMALL.into()), ..Default::default() }, Some(&agent())).await.expect("a write");
        slates.settled().await;
        for deep in [records(500, json!("x")), lists(500, json!("x"))] {
            let mut values = IndexMap::new();
            values.insert("$v".to_string(), deep);
            slates
                .state(StateParams { target: target(), values: Some(values), ..Default::default() }, Some(&agent()))
                .await
                .expect("a value of 500 levels");
            slates.settled().await;
            let read = slates.read(read_all(), Some(&agent())).await.expect("a read");
            let copy = read.to_json();
            drop(copy.clone());
            std::mem::forget(copy);
            let past = IndexMap::from([("$v".to_string(), records(501, json!("x")))]);
            let refused = slates.state(StateParams { target: target(), values: Some(past), ..Default::default() }, Some(&agent())).await;
            let said = format!("{:?}", refused.map(|answer| (answer.text, answer.problems)));
            assert!(said.contains("500"), "501 levels are refused with the cap in the line: {said}");
        }
    }));
}

// A `$v` value of 500 levels by `slate_state`: set, read, cloned and dropped; 501 is refused with the cap in the line.
#[test]
fn a_state_value_of_500_levels_is_set_read_cloned_and_dropped() {
    in_child("a_state_value_of_500_levels_is_set_read_cloned_and_dropped", false, state_body);
}

fn document_body() {
    support::local(Box::pin(async move {
        support::own_group();
        let root = temp_root("r2-doc-");
        let host = FakeHost::new(root.path(), MemStore::new());
        let slates = host.slates();
        slates.write(WriteParams { text: Some(SMALL.into()), ..Default::default() }, Some(&agent())).await.expect("a write");
        slates.settled().await;
        let read = slates.read(read_all(), Some(&agent())).await.expect("a read");
        let mut document = read.to_json()["document"].clone();
        assert!(document.is_object(), "the read answers the document: {document}");
        document["values"]["v"]["start"] = records(500, json!("x"));
        slates
            .write(WriteParams { document: Some(document), ..Default::default() }, Some(&agent()))
            .await
            .expect("a document with a start value of 500 levels");
        slates.settled().await;
        let read = slates.read(read_all(), Some(&agent())).await.expect("a read");
        let _ = serde_json::to_string(&read.to_json());
    }));
}

// A document whose start value nests 500 levels of records, written as stored JSON: validated, kept, read back.
#[test]
fn a_document_with_a_start_value_of_500_levels_is_written_and_read() {
    in_child("a_document_with_a_start_value_of_500_levels_is_written_and_read", false, document_body);
}

fn text_records_body() {
    let level = "{a: ".repeat(500);
    written_and_read(format!(
        r#"<slate><value name="v" start={{{level}1{}}} /><column><text>{{len($v)}}</text></column></slate>"#,
        "}".repeat(500)
    ));
}

#[test]
fn a_start_literal_of_500_record_levels_is_written_and_read() {
    in_child("a_start_literal_of_500_record_levels_is_written_and_read", false, text_records_body);
}

fn text_lists_body() {
    written_and_read(format!(
        r#"<slate><value name="v" start={{{}1{}}} /><column><text>{{len($v)}}</text></column></slate>"#,
        "[".repeat(500),
        "]".repeat(500)
    ));
}

#[test]
fn a_start_literal_of_500_list_levels_is_written_and_read() {
    in_child("a_start_literal_of_500_list_levels_is_written_and_read", false, text_lists_body);
}

fn chain(steps: usize, per: usize, records: bool) -> String {
    let mut out = String::from(r#"<slate><value name="v" start={[1]} />"#);
    for i in 0..steps {
        let prev = if i == 0 { "$v".to_string() } else { format!("$d{}", i - 1) };
        let (open, close) = if records { ("{a: ", "}") } else { ("[", "]") };
        out.push_str(&format!(r#"<derived name="d{i}" value={{{}{prev}{}}} />"#, open.repeat(per), close.repeat(per)));
    }
    out.push_str(&format!(r#"<column><text>{{len($d{})}}</text></column></slate>"#, steps - 1));
    out
}

// 32 chained derived values may each hold 15 levels: 480 levels of lists, and of records, by the last.
fn chain_lists_body() {
    written_and_read(chain(32, 15, false));
}

#[test]
fn thirty_two_derived_values_of_fifteen_nested_lists_each_are_written_and_read() {
    in_child("thirty_two_derived_values_of_fifteen_nested_lists_each_are_written_and_read", false, chain_lists_body);
}

fn chain_records_body() {
    written_and_read(chain(32, 15, true));
}

#[test]
fn thirty_two_derived_values_of_fifteen_nested_records_each_are_written_and_read() {
    in_child("thirty_two_derived_values_of_fifteen_nested_records_each_are_written_and_read", false, chain_records_body);
}

// The settle of derived values answers what the nested read answered: a derived value reads the ones it names, in any order,
// and a name no derived value owns reads the value of that name.
fn derived_answers_body() {
    support::local(Box::pin(async move {
        support::own_group();
        let root = temp_root("r2-derived-");
        let host = FakeHost::new(root.path(), MemStore::new());
        let slates = host.slates();
        let text = r#"<slate><value name="n" start={2} /><derived name="c" value={$b + $n} /><derived name="b" value={$a * 10} /><derived name="a" value={$n + 1} /><column><text>{$c}</text></column></slate>"#;
        slates.write(WriteParams { text: Some(text.into()), ..Default::default() }, Some(&agent())).await.expect("a write");
        slates.settled().await;
        let read = slates.read(read_all(), Some(&agent())).await.expect("a read");
        let json = read.to_json();
        assert!(json.to_string().contains("32"), "$c is (2 + 1) * 10 + 2: {json}");
    }));
}

#[test]
fn derived_values_read_each_other_in_any_order() {
    in_child("derived_values_read_each_other_in_any_order", false, derived_answers_body);
}

// ruling 3 of the round 2 brief: the JSON a run prints is read to 490 levels (road.rs JSON_DEPTH_MAX, the record wrapping the
// value in the 500) by a loop, and the value is cloned into the run's record and dropped.
fn run_output_body() {
    use wsp_slates::slate_runs::road::{parse_json, JSON_DEPTH_MAX};
    let text = format!("{}1{}", "{\"a\":".repeat(JSON_DEPTH_MAX), "}".repeat(JSON_DEPTH_MAX));
    let read = parse_json(&text).expect("a printed JSON at the cap is read");
    let copy = read.clone();
    assert_eq!(copy, read);
    drop(copy);
    drop(read);
}

#[test]
fn the_json_a_run_prints_at_its_cap_is_cloned_and_dropped() {
    in_child("the_json_a_run_prints_at_its_cap_is_cloned_and_dropped", false, run_output_body);
}
