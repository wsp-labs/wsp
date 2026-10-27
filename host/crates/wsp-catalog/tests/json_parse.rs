// SPDX-License-Identifier: AGPL-3.0-only
// parse_json reads what node's JSON.parse reads and fails in its words (oracle/json_parse.oracle.mjs).
use wsp_catalog::json_parse::parse_json;
use wsp_catalog::jsonc::{js_stringify, read_jsonc};

mod oracle_common;
use oracle_common::{fixture, s};

/// The deepest nesting of brackets in a text, strings and their escapes left out.
fn depth_of(text: &str) -> usize {
    let (mut depth, mut deepest, mut in_string, mut escaped) = (0usize, 0usize, false, false);
    for c in text.chars() {
        match (in_string, c) {
            (true, _) if escaped => escaped = false,
            (true, '\\') => escaped = true,
            (true, '"') => in_string = false,
            (true, _) => {}
            (false, '"') => in_string = true,
            (false, '[' | '{') => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            (false, ']' | '}') => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}

// Node reads any depth. Ruling 1 of the round 2 brief on wsp-map#2448 keeps the host to 500 levels, so a text nested past
// that is refused with V8's words for a parser out of stack, where node's own output (in the fixture) is the value.
#[test]
fn every_text_reads_or_fails_as_node_reads_it() {
    let cases = fixture("json_parse.json");
    let cases = cases.as_array().unwrap();
    assert!(cases.len() > 3000);
    let mut wrong = vec![];
    for case in cases {
        let text = s(&case["text"]);
        let got = parse_json(text);
        if depth_of(text) > 500 {
            assert_eq!(got.expect_err("past the cap").message, "Maximum call stack size exceeded", "{} levels", depth_of(text));
            continue;
        }
        match (got, case.get("ok"), case.get("error")) {
            (Ok(value), Some(ok), _) if js_stringify(&value) == s(ok) => {}
            (Err(e), _, Some(error)) if e.message == s(error) => {}
            (got, ..) => wrong.push(format!("{text:?}\n   got {:?}\n  want {case}", got.map(|v| js_stringify(&v)).map_err(|e| e.message))),
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} differ, the first ten:\n{}",
        wrong.len(),
        cases.len(),
        wrong.iter().take(10).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn a_lone_surrogate_escape_is_read_and_written_as_it_was() {
    let text = r#"{"a":"cut \ud83d","b":"\ude00 and \ud83d\ude00","c":"\ud800\ud800\udc00"}"#;
    let value = parse_json(text).unwrap();
    let written = r#"{"a":"cut \ud83d","b":"\ude00 and 😀","c":"\ud800𐀀"}"#;
    assert_eq!(js_stringify(&value), written);
    assert_eq!(read_jsonc(text).unwrap(), value);
}

#[test]
fn nesting_past_128_reads_and_a_runaway_one_is_refused() {
    assert!(parse_json(&format!("{}{}", "[".repeat(500), "]".repeat(500))).is_ok());
    assert!(parse_json(&format!("{}{}", "[".repeat(501), "]".repeat(501))).is_err());
    let runaway = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert_eq!(parse_json(&runaway).unwrap_err().message, "Maximum call stack size exceeded");
}

mod writing_back_a_file_that_holds_a_lone_surrogate {
    use super::*;
    use wsp_catalog::mcp::{McpMergeScope, MCP_SERVERS_JSON};

    fn same(s: &str, _command: bool) -> String {
        s.to_string()
    }

    #[test]
    fn a_merge_keeps_the_escape_where_it_stands_and_writes_one_that_travelled_as_an_escape() {
        let own = "{\n  \"history\": [{\"display\": \"cut \\ud83d\"}],\n  \"mcpServers\": {\n    \"a\": {\"command\": \"x\"}\n  }\n}\n";
        let travelled = r#"{"mcpServers":{"b":{"command":"y","env":{"K":"half \ude00"}}}}"#;
        let scope = McpMergeScope { keep: vec!["b".into()], drop: vec![], project: None, replace: vec![] };
        let merged = MCP_SERVERS_JSON.merge(&same, &scope, Some(own), travelled).unwrap();
        assert!(merged.text.starts_with("{\n  \"history\": [{\"display\": \"cut \\ud83d\"}],\n"), "{}", merged.text);
        assert!(merged.text.contains(r#""K": "half \ude00""#), "{}", merged.text);
        assert_eq!(parse_json(&merged.text).unwrap()["mcpServers"]["b"]["command"], "y");
    }

    #[test]
    fn an_edit_leaves_every_byte_but_the_one_it_changes() {
        let own = "{\"history\": [\"\\ud800\"], \"mcpServers\": {\"a\": {\"command\": \"x\"}, \"b\": {\"command\": \"y\"}}}";
        let scope = wsp_catalog::mcp::McpEditScope { keep: vec!["b".into()], drop: vec!["a".into()], project: None };
        let edited = MCP_SERVERS_JSON.edit(&same, &scope, own).unwrap();
        assert_eq!(edited.text, "{\"history\": [\"\\ud800\"], \"mcpServers\": {\"b\": {\"command\": \"y\"}}}");
    }
}
