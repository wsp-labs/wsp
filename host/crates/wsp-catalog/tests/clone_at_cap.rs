// SPDX-License-Identifier: AGPL-3.0-only
//! Ruling 3 of round 2 of #2448: a tree read from outside is at most 500 levels deep, and none of the catalog's readers
//! clones, converts or drops one that deep onto a stack it cannot hold. Each case reads a tree of nested objects (the
//! kind that clones deepest: 628 levels on a 1 MiB debug stack) at the cap, clones it and drops it, in a child copy of
//! this binary on a 1 MiB thread.
use wsp_catalog::json_edit::edit_json;
use wsp_catalog::json_parse::parse_json;
use wsp_catalog::jsonc_tree::{parse_tree, PathSeg};
use wsp_catalog::mcp::McpFormat;
use wsp_catalog::smol_toml::parse_toml;

const CHILD: &str = "CLONE_AT_CAP_CHILD";
const CAP: usize = 500;

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
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the whole process died ({}): {}",
        out.status,
        stderr.lines().rev().take(2).collect::<Vec<_>>().join(" | ")
    );
}

/// `{"a":{"a":...1}}`, `n` objects deep.
fn objects(n: usize) -> String {
    format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n))
}

fn server_file(extra: &str) -> String {
    format!(r#"{{"mcpServers":{{"x":{{"command":"c","args":["a"],"extra":{extra}}}}}}}"#)
}

fn json_tree_body() {
    let v = parse_json(&objects(CAP)).expect("the cap reads it");
    let copy = v.clone();
    assert_eq!(v, copy);
    drop(copy);
    drop(v);
}

#[test]
fn a_json_tree_of_500_object_levels_clones_and_drops() {
    in_child("a_json_tree_of_500_object_levels_clones_and_drops", json_tree_body);
}

fn toml_tree_body() {
    let text = format!("[{}]\nk = 1\n", vec!["a"; CAP].join("."));
    let t = parse_toml(&text).expect("the cap reads it");
    let copy = t.clone();
    assert_eq!(t, copy);
    drop(copy);
    drop(t);
}

#[test]
fn a_toml_table_nested_500_levels_clones_and_drops() {
    in_child("a_toml_table_nested_500_levels_clones_and_drops", toml_tree_body);
}

fn node_tree_body() {
    drop(parse_tree(&objects(CAP)).expect("the cap reads it"));
}

#[test]
fn a_comment_tree_of_500_object_levels_drops() {
    in_child("a_comment_tree_of_500_object_levels_drops", node_tree_body);
}

fn only_and_entry_body() {
    let text = server_file(&objects(CAP - 3));
    let names = vec!["x".to_string()];
    let only = McpFormat::McpServersJson.only(&text, &names, None, None).expect("only reads it");
    assert!(only.contains("extra"));
    assert!(McpFormat::McpServersJson.entry_of(&text, "x", None).is_some());
}

#[test]
fn only_and_entry_of_answer_a_server_holding_object_levels_to_the_cap() {
    in_child("only_and_entry_of_answer_a_server_holding_object_levels_to_the_cap", only_and_entry_body);
}

fn edit_json_body() {
    let text = server_file(&objects(CAP - 3));
    let edit = vec![(vec![PathSeg::Key("mcpServers".into()), PathSeg::Key("y".into())], Some(serde_json::json!({"command": "d"})))];
    assert!(edit_json(&text, &edit).expect("an edit beside a value at the cap").contains("\"y\""));
    let drop_x = vec![(vec![PathSeg::Key("mcpServers".into()), PathSeg::Key("x".into())], None)];
    assert!(!edit_json(&text, &drop_x).expect("a removal of a value at the cap").contains("extra"));
}

#[test]
fn edit_json_beside_and_over_a_value_at_the_cap() {
    in_child("edit_json_beside_and_over_a_value_at_the_cap", edit_json_body);
}

// A message that wraps a value kept to the cap (a reply, a stored row) may add `WRAPPER_LEVELS` levels of its own.
fn framed_body() {
    use wsp_catalog::json_parse::{parse_json_framed, WRAPPER_LEVELS};
    let at = format!("{}1{}", "{\"a\":".repeat(CAP + WRAPPER_LEVELS), "}".repeat(CAP + WRAPPER_LEVELS));
    let v = parse_json_framed(&at).expect("the cap and the wrapper's levels are read");
    drop(v.clone());
    let past = format!("{}1{}", "{\"a\":".repeat(CAP + WRAPPER_LEVELS + 1), "}".repeat(CAP + WRAPPER_LEVELS + 1));
    assert_eq!(parse_json_framed(&past).expect_err("one more is refused").message, "Maximum call stack size exceeded");
    assert!(parse_json(&at).is_err(), "the plain read keeps the cap");
}

#[test]
fn a_framed_read_takes_the_cap_and_the_wrapper_levels() {
    in_child("a_framed_read_takes_the_cap_and_the_wrapper_levels", framed_body);
}
