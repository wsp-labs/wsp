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

use indexmap::IndexSet;
use wsp_catalog::json_edit::edit_json;
use wsp_catalog::json_parse::parse_json;
use wsp_catalog::jsonc::{js_stringify, js_stringify_pretty};
use wsp_catalog::jsonc_tree::{parse_tree, PathSeg};
use wsp_catalog::mcp::{McpEditScope, McpFormat, McpMergeScope};
use wsp_catalog::mcp_codex::{codex_trusts, trust_codex};
use wsp_catalog::mcp_refs::{places, still_stands};
use wsp_catalog::smol_toml::parse_toml;

fn arrays(depth: usize) -> String {
    format!("{}{}", "[".repeat(depth), "]".repeat(depth))
}

fn objects(depth: usize) -> String {
    format!("{}1{}", "{\"a\":".repeat(depth), "}".repeat(depth))
}

/// Nesting `parse_json` takes: its own cap is 1000 levels, counted from the root.
const JSON_FITS: usize = 990;

/// A Claude Code config whose server `x` holds `extra`, `extra_depth` levels deep in all, read as three levels more.
fn claude_file(extra: &str) -> String {
    format!(r#"{{"mcpServers":{{"x":{{"command":"c","args":["a","${{T}}"],"extra":{extra}}}}}}}"#)
}

fn no_rewrite(s: &str, _command: bool) -> String {
    s.to_string()
}

fn upper(s: &str, _command: bool) -> String {
    s.to_uppercase()
}

// ---- parse_json, the cap that holds the rest ----

fn parse_json_body(depth: usize) {
    for text in [arrays(depth), objects(depth)] {
        let err = parse_json(&text).expect_err("past the cap, the one read fails");
        assert_eq!(err.message, "Maximum call stack size exceeded");
    }
}

// json_parse.rs:138 value, :151 object and :186 array recurse once per level and refuse at MAX_DEPTH 1000, but the
// stack is gone first: a 1 MiB debug stack dies at 878 levels of `[[[...]]]`. Fed by every JSON the catalog reads: the
// the harnesses' config files (read_jsonc: settings.json, .claude.json, .mcp.json, opencode.json), plugins' json files
// (plugins.rs:211), a Codex app-server's reply lines (plugins_codex.rs:113), `codex mcp list --json` output
// (mcp_check.rs:80), `claude auth status` output (signin.rs:493), and the callers in wsp-collect and wsp-engine. Node's
// JSON.parse answers with the value at 1,000,000 levels (the refusal at 1000 is the port's own choice and also differs).
// The text is 2 bytes a level, so 100,000 levels is 200 KB.
#[test]
fn parse_json_of_deep_nesting() {
    in_child("parse_json_of_deep_nesting", 100_000, parse_json_body);
}

fn parse_json_cap_body(depth: usize) {
    for build in [arrays, objects] {
        assert!(parse_json(&build(depth)).is_ok(), "{depth} levels read");
        let err = parse_json(&build(depth + 1)).expect_err("one level past the cap refuses");
        assert_eq!(err.message, "Maximum call stack size exceeded");
    }
}

// The cap itself, on the 1 MiB stack: 1,000 levels read and 1,001 refuse (ruling 2 of #2448).
#[test]
fn parse_json_reads_1000_levels_and_refuses_1001() {
    in_child("parse_json_reads_1000_levels_and_refuses_1001", 1000, parse_json_cap_body);
}

// ---- trees and writers over a value the parse accepted (999 levels at most) ----

fn edit_json_body(depth: usize) {
    let text = format!(r#"{{"mcpServers":{}}}"#, objects(depth));
    let edit = (vec![PathSeg::Key("mcpServers".into()), PathSeg::Key("x".into())], Some(serde_json::json!(1)));
    let out = edit_json(&text, &[edit]).expect("node edits a file nested this deep in place");
    assert!(out.contains(r#""x": 1"#) || out.contains(r#""x":1"#));
}

// Dies at 433 levels (1 MiB, debug). jsonc_tree.rs:164 (value, object, property, value) recurses once per level with no cap of its own; edit_json reaches
// it with text read_jsonc took, so only the 1000 levels read_jsonc takes reach it. A config the person keeps
// (settings.json, .mcp.json, opencode.json) that the host edits in place. Node answers with the edited text up to 2,344
// levels (editJson of the same file) and fails the one call ("could not be changed in place") from there.
#[test]
fn edit_json_deep_object_in_a_kept_config() {
    in_child("edit_json_deep_object_in_a_kept_config", JSON_FITS, edit_json_body);
}

fn parse_tree_body(depth: usize) {
    let text = objects(depth);
    assert!(parse_tree(&text).is_some());
}

// Dies at 439. jsonc_tree.rs::parse_tree alone, the public entry: no cap, the recursion is the input's. Reached only through
// edit_json and edit_jsonc after read_jsonc, so 1000 levels at most today.
#[test]
fn parse_tree_deep_objects() {
    in_child("parse_tree_deep_objects", JSON_FITS, parse_tree_body);
}

fn tree_cap_body(depth: usize) {
    for build in [arrays, objects] {
        assert!(parse_tree(&build(depth)).is_some(), "{depth} levels read");
        assert!(parse_tree(&build(depth + 1)).is_none(), "one level past the cap is not read");
    }
}

// The cap of jsonc_tree.rs::parse_tree, on the 1 MiB stack: 1,000 levels read and 1,001 do not (ruling 2).
#[test]
fn parse_tree_reads_1000_levels_and_refuses_1001() {
    in_child("parse_tree_reads_1000_levels_and_refuses_1001", 1000, tree_cap_body);
}

fn edit_json_cap_body(depth: usize) {
    let edit = || (vec![PathSeg::Key("mcpServers".into()), PathSeg::Key("x".into())], Some(serde_json::json!(1)));
    let fits = format!(r#"{{"mcpServers":{}}}"#, objects(depth - 1));
    assert!(edit_json(&fits, &[edit()]).expect("1,000 levels edit in place").contains("\"x\""));
    let past = format!(r#"{{"mcpServers":{}}}"#, objects(depth));
    let err = edit_json(&past, &[edit()]).expect_err("1,001 levels do not");
    assert!(err.message.starts_with("the file could not be changed in place"), "{}", err.message);
}

// edit_json answers node's words at its own limit ("could not be changed in place") past the cap (ruling 2).
#[test]
fn edit_json_refuses_past_1000_levels_in_place() {
    in_child("edit_json_refuses_past_1000_levels_in_place", 1000, edit_json_cap_body);
}

fn stringify_body(depth: usize) {
    let value = parse_json(&objects(depth)).expect("under the cap");
    assert!(js_stringify(&value).len() > depth);
    assert!(js_stringify_pretty(&value).len() > depth);
}

// Dies at 554. jsonc.rs:130 write_value over a value parse_json read, 999 levels at most; node's JSON.stringify answers there.
#[test]
fn stringify_deep_value() {
    in_child("stringify_deep_value", JSON_FITS, stringify_body);
}

fn canonical_body(depth: usize) {
    let value = parse_json(&objects(depth)).expect("under the cap");
    assert!(wsp_catalog::json_edit::json_canonical(&value).len() > depth);
}

// Dies at 554. json_edit.rs:56 json_canonical over a value parse_json read.
#[test]
fn canonical_deep_value() {
    in_child("canonical_deep_value", JSON_FITS, canonical_body);
}

// ---- the formats over a config whose server holds a deep value ----

fn format_edit_body(depth: usize) {
    let text = claude_file(&arrays(depth));
    let scope = McpEditScope { keep: vec!["x".into()], ..Default::default() };
    let out = McpFormat::McpServersJson.edit(&upper, &scope, &text).expect("node edits it");
    assert!(out.text.contains("${T}") || out.text.contains("${t}") || out.text.contains("${T"));
}

// Dies at 634. mcp_json.rs:281 walk_strings and :291 string_edits recurse once per level of a kept server's definition; node's
// walk (mcp.ts rewriteStrings) is recursive too. From the person's agent config.
#[test]
fn format_edit_deep_definition() {
    in_child("format_edit_deep_definition", JSON_FITS - 3, format_edit_body);
}

fn format_merge_body(depth: usize) {
    let own = r#"{"mcpServers":{}}"#;
    let travelled = claude_file(&arrays(depth));
    let scope = McpMergeScope { keep: vec!["x".into()], ..Default::default() };
    let merged = McpFormat::McpServersJson.merge(&no_rewrite, &scope, Some(own), &travelled).expect("node merges it");
    assert!(merged.text.contains("mcpServers"));
}

// Dies at 634. The same walk on the merge, over the travelled config: a config copied from another computer.
#[test]
fn format_merge_deep_definition() {
    in_child("format_merge_deep_definition", JSON_FITS - 3, format_merge_body);
}

fn format_refer_body(depth: usize) {
    let text = claude_file(&arrays(depth));
    let referred = McpFormat::McpServersJson.refer(&text, None, &[], &IndexSet::new()).expect("node reads it");
    assert_eq!(referred.servers.len(), 1);
}

// Dies at 855 (parse_json itself dies at 878, so this adds no earlier fault of its own). mcp_refs.rs:533 strings_in, :556 oauth_secrets_at and :575 held_in recurse once per level of every definition the
// refer weighs (the copy of an agent's config to a machine).
#[test]
fn format_refer_deep_definition() {
    in_child("format_refer_deep_definition", JSON_FITS - 3, format_refer_body);
}

fn format_entry_of_body(depth: usize) {
    let text = claude_file(&arrays(depth));
    let entry = McpFormat::McpServersJson.entry_of(&text, "x", None).expect("an entry");
    assert!(entry.len() > depth);
}

// Dies at 873, the parse's own limit: no walk of its own shows here.
#[test]
fn format_entry_of_deep_definition() {
    in_child("format_entry_of_deep_definition", JSON_FITS - 3, format_entry_of_body);
}

fn launch_servers_body(depth: usize) {
    use wsp_catalog::mcp_launch::{claude_launch_servers, LaunchConfigs, TurnFile};
    let text = claude_file(&arrays(depth));
    let configs = LaunchConfigs {
        user: Some(TurnFile { path: "/home/u/.claude.json".into(), text }),
        projects: None,
        folder: "/work".into(),
        key: None,
    };
    let values = indexmap::IndexMap::from([("T".to_string(), "secret".to_string())]);
    let out = claude_launch_servers(&configs, &values);
    assert_eq!(out.len(), 1);
}

// Dies at 341. mcp_launch.rs:129 filled (and json_canonical on the way in) recurses once per level of the entry a turn's launch fills with the vault's values.
#[test]
fn launch_servers_deep_definition() {
    in_child("launch_servers_deep_definition", JSON_FITS - 3, launch_servers_body);
}

fn formats_at_the_cap_body(depth: usize) {
    use wsp_catalog::mcp_launch::{claude_launch_servers, LaunchConfigs, TurnFile};
    // the server's `extra` stands three levels down, so `depth` of it is `depth + 3` in the file
    let text = claude_file(&arrays(depth - 3));
    let scope = McpEditScope { keep: vec!["x".into()], ..Default::default() };
    McpFormat::McpServersJson.edit(&upper, &scope, &text).expect("edits at the cap");
    let mscope = McpMergeScope { keep: vec!["x".into()], ..Default::default() };
    McpFormat::McpServersJson.merge(&no_rewrite, &mscope, Some(r#"{"mcpServers":{}}"#), &text).expect("merges at the cap");
    let referred = McpFormat::McpServersJson.refer(&text, None, &[], &IndexSet::new()).expect("refers at the cap");
    assert_eq!(referred.servers.len(), 1);
    assert!(McpFormat::McpServersJson.entry_of(&text, "x", None).is_some());
    let configs = LaunchConfigs {
        user: Some(TurnFile { path: "/home/u/.claude.json".into(), text: text.clone() }),
        projects: None,
        folder: "/work".into(),
        key: None,
    };
    let values = indexmap::IndexMap::from([("T".to_string(), "secret".to_string())]);
    assert_eq!(claude_launch_servers(&configs, &values).len(), 1);
    let past = claude_file(&arrays(depth - 2));
    assert!(McpFormat::McpServersJson.edit(&upper, &scope, &past).is_err());
    assert!(McpFormat::McpServersJson.refer(&past, None, &[], &IndexSet::new()).is_err());
    assert!(claude_launch_servers(
        &LaunchConfigs {
            user: Some(TurnFile { path: "/home/u/.claude.json".into(), text: past }),
            projects: None,
            folder: "/work".into(),
            key: None
        },
        &values
    )
    .is_empty());
}

// Every walk over a server definition at the cap, on the 1 MiB stack: a file nested 1,000 levels is read and answered, and
// 1,001 is refused by the parse (ruling 2, ruling 4).
#[test]
fn formats_answer_at_1000_levels() {
    in_child("formats_answer_at_1000_levels", 1000, formats_at_the_cap_body);
}

// ---- strings that nest by their own text ----

fn places_body(depth: usize) {
    let s = format!("{}x", "bearer ".repeat(depth));
    let found = places(&s, "the file", None);
    assert!(found.len() > depth);
}

// Dies at 460 levels. mcp_refs.rs:81 push_places recurses once per leading scheme word (`bearer bearer bearer ...`), flag (`--a=--a=`) or
// header (`A: A: A:`) of a definition's string; 7 bytes a level. node's generator (mcp.ts places, yield* per level)
// answers up to 3,049 levels and throws a RangeError from 3,050 to the caller of stillStands/referring. The 20,000
// levels here are 140 KB of one string.
#[test]
fn places_repeated_scheme_word() {
    in_child("places_repeated_scheme_word", 20_000, places_body);
}

fn places_flag_body(depth: usize) {
    let s = format!("{}x", "--a=".repeat(depth));
    assert!(places(&s, "the file", None).len() > depth);
}

#[test]
fn places_repeated_flag() {
    in_child("places_repeated_flag", 20_000, places_flag_body);
}

fn places_header_body(depth: usize) {
    let s = format!("{}x", "A: ".repeat(depth));
    assert!(places(&s, "the file", None).len() > depth);
}

#[test]
fn places_repeated_header() {
    in_child("places_repeated_header", 20_000, places_header_body);
}

fn still_stands_body(depth: usize) {
    let entry = serde_json::json!({ "args": [format!("{}x", "bearer ".repeat(depth))] });
    let _ = still_stands(&[entry], &[]);
}

#[test]
fn still_stands_repeated_scheme_word() {
    in_child("still_stands_repeated_scheme_word", 20_000, still_stands_body);
}

// ---- TOML ----

const TOML_TOO_DEEP: &str = "Invalid TOML document: document contains excessively nested structures. aborting.";

fn toml_cap_body(depth: usize) {
    let header = |n: usize| format!("[{}]\nk = 1\n", vec!["a"; n].join("."));
    let dotted = |n: usize| format!("{} = 1\n", vec!["a"; n].join("."));
    let arrays = |n: usize| format!("a = {}{}\n", "[".repeat(n), "]".repeat(n));
    let inline = |n: usize| format!("a = {}1{}\n", "{b=".repeat(n), "}".repeat(n));
    let dotted_inline = |n: usize| format!("a = {{{} = 1}}\n", vec!["b"; n].join("."));
    let cases: [(&str, &dyn Fn(usize) -> String); 5] = [
        ("header", &header),
        ("dotted key", &dotted),
        ("arrays", &arrays),
        ("inline tables", &inline),
        ("dotted key in an inline table", &dotted_inline),
    ];
    for (name, build) in cases {
        let fits = match name {
            "dotted key" => depth + 1,
            "dotted key in an inline table" => depth,
            _ => depth,
        };
        assert!(parse_toml(&build(fits)).is_ok(), "{name}: {fits} levels read");
        let err = parse_toml(&build(fits + 1)).expect_err(name);
        assert!(err.starts_with(TOML_TOO_DEEP), "{name}: {err}");
    }
}

// The cap of smol_toml.rs, on the 1 MiB stack: 1,000 levels read and 1,001 refuse, for the tables a header or a dotted
// key nests (smol-toml has no cap on them, ruling 2) and for arrays and inline tables (smol-toml's maxDepth, ruling 3).
#[test]
fn toml_reads_1000_levels_and_refuses_1001() {
    in_child("toml_reads_1000_levels_and_refuses_1001", 1000, toml_cap_body);
}

fn toml_header_body(depth: usize) {
    let header = vec!["a"; depth].join(".");
    let text = format!("[{header}]\nk = 1\n");
    assert!(parse_toml(&text).expect_err("past the cap, the one read fails").starts_with(TOML_TOO_DEEP));
}

// Dies at 1751 parts. smol_toml.rs: the table a `[a.a.a...]` header names is nested once per part with no cap (the 256 cap counts arrays
// and inline tables only); the build is a loop, the drop and `value_of` (smol_toml.rs:975) recurse. Codex's
// config.toml, read by codex_trusts, trust_codex, refer_codex, unset_toml_keys.
#[test]
fn toml_deep_table_header() {
    in_child("toml_deep_table_header", 20_000, toml_header_body);
}

fn toml_dotted_body(depth: usize) {
    let key = vec!["a"; depth].join(".");
    let text = format!("{key} = 1\n");
    assert!(parse_toml(&text).expect_err("past the cap, the one read fails").starts_with(TOML_TOO_DEEP));
}

#[test]
fn toml_deep_dotted_key() {
    in_child("toml_deep_dotted_key", 20_000, toml_dotted_body);
}

fn codex_trusts_body(depth: usize) {
    let header = vec!["a"; depth].join(".");
    let text = format!("[{header}]\nk = 1\n");
    assert!(!codex_trusts(&text, "/work"));
    let err = trust_codex(Some(&text), "/work").expect_err("an unreadable file is not edited");
    assert!(err.message.starts_with(TOML_TOO_DEEP), "{}", err.message);
}

#[test]
fn codex_trusts_deep_header() {
    in_child("codex_trusts_deep_header", 20_000, codex_trusts_body);
}

fn refer_codex_cap_body(depth: usize) {
    let text = |parts: usize| {
        format!(
            "[mcp_servers.x]\ncommand = \"c\"\n[mcp_servers.x.env]\nTOKEN = \"secret-value\"\n[mcp_servers.x.{}]\nk = 1\n",
            vec!["a"; parts].join(".")
        )
    };
    let referred = McpFormat::CodexToml.refer(&text(depth - 2), None, &[], &IndexSet::new()).expect("1,000 levels refer");
    assert_eq!(referred.servers.len(), 1);
    assert_eq!(referred.entries.len(), 1);
    let err = McpFormat::CodexToml.refer(&text(depth - 1), None, &[], &IndexSet::new()).expect_err("1,001 levels do not");
    assert!(err.message.contains("excessively nested"), "{}", err.message);
}

// Codex's refer at the cap, on the 1 MiB stack: a table 1,000 levels down is read, converted and written back, and 1,001
// is refused as an unreadable file (ruling 2).
#[test]
fn refer_codex_reads_1000_levels_and_refuses_1001() {
    in_child("refer_codex_reads_1000_levels_and_refuses_1001", 1000, refer_codex_cap_body);
}

fn refer_codex_body(depth: usize) {
    let header = vec!["a"; depth].join(".");
    let text = format!("[mcp_servers.x]\ncommand = \"c\"\n[mcp_servers.x.{header}]\nk = 1\n");
    let err = McpFormat::CodexToml.refer(&text, None, &[], &IndexSet::new()).expect_err("an unreadable file is not referred");
    assert!(err.message.contains("excessively nested"), "{}", err.message);
}

// Dies at 616 parts. The refer of a Codex config.toml: serde_json::to_value (mcp_toml_refer.rs:300), table.clone() and Drop of the
// parsed toml::Table recurse once per header part.
#[test]
fn refer_codex_deep_table() {
    in_child("refer_codex_deep_table", 20_000, refer_codex_body);
}

// ---- each walk alone, over a value built by a loop (no parse in front, nothing dropped) ----

fn nested_json(depth: usize) -> serde_json::Value {
    let mut v = serde_json::Value::Null;
    for _ in 0..depth {
        v = serde_json::Value::Object(serde_json::Map::from_iter([("a".to_string(), v)]));
    }
    v
}

fn nested_array(depth: usize) -> serde_json::Value {
    let mut v = serde_json::Value::Null;
    for _ in 0..depth {
        v = serde_json::Value::Array(vec![v]);
    }
    v
}

fn alone_stringify_body(depth: usize) {
    let v = nested_json(depth);
    assert!(js_stringify(&v).len() > depth);
    std::mem::forget(v);
}

// Dies at 1548. jsonc.rs:130 write_value alone. Fed by a value parse_json read (999 levels at most) or one a TOML table became
// (toml_deep_table_header: no cap).
#[test]
fn alone_stringify() {
    in_child("alone_stringify", 100_000, alone_stringify_body);
}

fn alone_canonical_body(depth: usize) {
    let v = nested_json(depth);
    assert!(wsp_catalog::json_edit::json_canonical(&v).len() > depth);
    std::mem::forget(v);
}

// Dies at 730. json_edit.rs:56 json_canonical alone.
#[test]
fn alone_canonical() {
    in_child("alone_canonical", 100_000, alone_canonical_body);
}

fn alone_oauth_body(depth: usize) {
    let v = nested_json(depth);
    let _ = wsp_catalog::mcp_refs::oauth_secrets_in(&v, &|_| false);
    std::mem::forget(v);
}

// Dies at 638. mcp_refs.rs:556 oauth_secrets_at alone (it also copies the path at every level, so it is quadratic as well).
#[test]
fn alone_oauth_secrets() {
    in_child("alone_oauth_secrets", 100_000, alone_oauth_body);
}

fn alone_strings_in_body(depth: usize) {
    let v = nested_array(depth);
    let _ = still_stands(std::slice::from_ref(&v), &[]);
    std::mem::forget(v);
}

// Dies at 5908. mcp_refs.rs:533 strings_in alone, through still_stands; only a TOML table (no depth cap) reaches it that deep.
#[test]
fn alone_strings_in() {
    in_child("alone_strings_in", 100_000, alone_strings_in_body);
}
