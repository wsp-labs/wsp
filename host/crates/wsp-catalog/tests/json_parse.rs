// SPDX-License-Identifier: AGPL-3.0-only
// parse_json reads what node's JSON.parse reads and fails in its words (oracle/json_parse.oracle.mjs).
use wsp_catalog::json_parse::parse_json;
use wsp_catalog::jsonc::{js_stringify, read_jsonc};

mod oracle_common;
use oracle_common::{fixture, s};

#[test]
fn every_text_reads_or_fails_as_node_reads_it() {
    let cases = fixture("json_parse.json");
    let cases = cases.as_array().unwrap();
    assert!(cases.len() > 3000);
    let mut wrong = vec![];
    for case in cases {
        let text = s(&case["text"]);
        let got = parse_json(text);
        match (got, case.get("ok"), case.get("error")) {
            (Ok(value), Some(ok), _) if js_stringify(&value) == s(ok) => {}
            (Err(e), _, Some(error)) if e.message == s(error) => {}
            (got, ..) => wrong.push(format!("{text:?}\n   got {:?}\n  want {case}", got.map(|v| js_stringify(&v)).map_err(|e| e.message))),
        }
    }
    assert!(wrong.is_empty(), "{} of {} differ, the first ten:\n{}", wrong.len(), cases.len(), wrong.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
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
    assert!(parse_json(&format!("{}{}", "[".repeat(900), "]".repeat(900))).is_ok());
    let runaway = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert_eq!(parse_json(&runaway).unwrap_err().message, "Maximum call stack size exceeded");
}
