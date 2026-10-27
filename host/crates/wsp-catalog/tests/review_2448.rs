// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448): cases it leaves red. Each runs in a child copy of this binary
//! on a 1 MiB thread, as deep_input.rs does, and fails when the child dies.
use std::time::{Duration, Instant};

use indexmap::IndexSet;
use serde_json::json;
use wsp_catalog::json_edit::edit_json;
use wsp_catalog::jsonc_tree::PathSeg;
use wsp_catalog::mcp::{McpEditScope, McpFormat, McpMergeScope};
use wsp_catalog::mcp_launch::{claude_launch_servers, LaunchConfigs, TurnFile};
use wsp_catalog::mcp_refs::places;

const CHILD: &str = "REVIEW_2448_CHILD";

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
    format!(r#"{{"mcpServers":{{"x":{{"command":"c","args":["a","${{T}}"],"extra":{extra}}}}}}}"#)
}

fn same(s: &str, _command: bool) -> String {
    s.to_string()
}

// Ruling 1 keeps a tree to 500 levels, under the 627 a tree of objects clones to on the 1 MiB debug stack. These readers clone
// the servers table: mcp_json.rs edit, merge and only, mcp_json_refer.rs refer, mcp_launch.rs folder_entry (every turn's
// launch). The file is 500 levels deep, the deepest the cap reads: 3 levels of the file around 497 of the server's own.
fn mcp_edit_cap_objects_body() {
    let keep = McpEditScope { keep: vec!["x".into()], ..Default::default() };
    McpFormat::McpServersJson.edit(&same, &keep, &server_file(&objects(497))).expect("the cap lets it edit");
}

#[test]
fn an_mcp_json_server_holding_object_levels_to_the_cap_edits() {
    in_child("an_mcp_json_server_holding_object_levels_to_the_cap_edits", mcp_edit_cap_objects_body);
}

fn mcp_merge_cap_objects_body() {
    let keep = McpMergeScope { keep: vec!["x".into()], ..Default::default() };
    McpFormat::McpServersJson
        .merge(&same, &keep, Some(r#"{"mcpServers":{}}"#), &server_file(&objects(497)))
        .expect("the cap lets it merge");
}

#[test]
fn an_mcp_json_server_holding_object_levels_to_the_cap_merges() {
    in_child("an_mcp_json_server_holding_object_levels_to_the_cap_merges", mcp_merge_cap_objects_body);
}

fn mcp_refer_cap_objects_body() {
    McpFormat::McpServersJson.refer(&server_file(&objects(497)), None, &[], &IndexSet::new()).expect("node refers it");
}

#[test]
fn an_mcp_json_server_holding_object_levels_to_the_cap_refers() {
    in_child("an_mcp_json_server_holding_object_levels_to_the_cap_refers", mcp_refer_cap_objects_body);
}

fn launch_cap_objects_body() {
    let text = format!(r#"{{"projects":{{"/w":{{"extra":{},"mcpServers":{{}}}}}}}}"#, objects(497));
    let configs =
        LaunchConfigs { user: Some(TurnFile { path: "/h/.claude.json".into(), text }), projects: None, folder: "/w".into(), key: None };
    let values = indexmap::IndexMap::from([("T".to_string(), "s".to_string())]);
    let _ = claude_launch_servers(&configs, &values);
}

#[test]
fn a_turn_launches_with_object_levels_to_the_cap_in_the_folders_entry() {
    in_child("a_turn_launches_with_object_levels_to_the_cap_in_the_folders_entry", launch_cap_objects_body);
}

// mcp_toml_refer.rs:318: when the line editor does not give back the tree refer wants, the tree is written whole by
// `toml::to_string`, which recurses once per table and dies near 300 tables (debug, 1 MiB; 500 to 600 in release). A
// server env written as a dotted key takes that road, and the person's Codex config.toml can nest a table 400 deep under
// the 1,000 cap. node (mcp.ts:1661, smol-toml's stringify) answers at 400 with `env_vars = [ "TOKEN" ]` and the table
// kept, and refuses at 998 with "Could not stringify the object: maximum object depth exceeded".
fn refer_codex_dotted_env_400_body() {
    let text =
        format!("[mcp_servers.x]\ncommand = \"c\"\nenv.TOKEN = \"secret-value\"\n[mcp_servers.x.{}]\nk = 1\n", vec!["d"; 400].join("."));
    let referred = McpFormat::CodexToml.refer(&text, None, &[], &IndexSet::new()).expect("node refers it");
    assert!(referred.text.contains("env_vars") && !referred.text.contains("secret-value"));
}

#[test]
fn refer_codex_with_a_dotted_env_and_a_table_400_deep_answers() {
    in_child("refer_codex_with_a_dotted_env_and_a_table_400_deep_answers", refer_codex_dotted_env_400_body);
}

// Ruling 6: push_places linear. A query chain `https://h/?a=https://h/?a=...` still decodes and scans the rest of the
// string at every level (mcp_refs.rs:207 query_value, :258 the scan for '#'): four times the levels cost sixteen times.
fn query_chain(n: usize) -> Duration {
    let s = format!("{}x", "https://h/?a=".repeat(n));
    let at = Instant::now();
    assert_eq!(places(&s, "the file", None).len(), n + 1);
    at.elapsed()
}

#[test]
fn places_of_a_query_chain_costs_what_its_length_costs() {
    let (small, big) = (query_chain(2_000), query_chain(8_000));
    assert!(big < small * 8, "2,000 levels took {small:?} and 8,000 took {big:?}");
}

// node (mcp.ts:724 editJson) reads a 501-level array and refuses it with "the file is not a JSON object"; the port
// refuses the read (ruling 2) and edit_json (json_edit.rs:27) answers "could not be changed in place" for any too-deep
// text, though the first byte says the root is no object.
#[test]
fn an_array_root_past_the_cap_is_refused_as_not_an_object() {
    let text = format!("{}{}", "[".repeat(501), "]".repeat(501));
    let edit = vec![(vec![PathSeg::Key("mcpServers".into()), PathSeg::Key("x".into())], Some(json!({"command": "c"})))];
    assert_eq!(edit_json(&text, &edit).expect_err("not an object").message, "the file is not a JSON object");
}
