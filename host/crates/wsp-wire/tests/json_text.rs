// SPDX-License-Identifier: AGPL-3.0-only
//! `parse_json` reads a text as node's `JSON.parse` does (node 22.23.4): every expectation below is what node answered.

use serde_json::{json, Value};
use wsp_wire::wire::json_text::{parse_json, restore_surrogates};

/// `JSON.parse("[1e400,-1e999,1e-400,5.0,-0]")` is `[Infinity,-Infinity,0,5,-0]`, and `JSON.stringify` writes the two
/// infinities as null.
#[test]
fn a_number_past_a_double_reads_as_infinity_which_writes_null() {
    let read = parse_json("[1e400,-1e999,1e-400]").unwrap();
    assert_eq!(read, json!([null, null, 0.0]));
}

/// Texts `JSON.parse` throws on.
#[test]
fn what_json_parse_refuses_is_refused() {
    for text in [
        "",
        " ",
        "01",
        "-",
        "1.",
        ".5",
        "+1",
        "NaN",
        "[1,]",
        "[,1]",
        "{\"a\":1,}",
        "{a:1}",
        "'a'",
        "\"\\x41\"",
        "\"a\nb\"",
        "\"\\u12\"",
        "\u{feff}1",
        "\u{a0}1",
        "1 2",
        "[1 2]",
        "{\"a\" 1}",
        "[",
        "{\"a\":1",
        "tru",
        "\"unterminated",
    ] {
        assert_eq!(parse_json(text), None, "node refuses {text:?}");
    }
}

/// `JSON.parse` takes arrays nested past serde_json's 128 levels, and an object holds a lone surrogate escape as a string
/// that `JSON.stringify` writes back as the same escape, in lower case.
#[test]
fn a_lone_surrogate_and_a_deep_text_are_read_and_a_surrogate_is_written_back() {
    let deep = format!("{}{}", "[".repeat(300), "]".repeat(300));
    assert!(parse_json(&deep).is_some());
    let read = parse_json("[\"\\ud800\",\"\\ude00\\ud83d\",\"\\ud83d\\ude00\",\"\\uDBFF\\uDFFF\",\"\u{10FFFF}\"]").unwrap();
    let written = restore_surrogates(&serde_json::to_string(&read).unwrap());
    assert_eq!(written, "[\"\\ud800\",\"\\ude00\\ud83d\",\"\u{1F600}\",\"\u{10FFFF}\",\"\u{10FFFF}\"]");
    let key = parse_json(r#"{"\ud800":"\udc00"}"#).unwrap();
    assert_eq!(restore_surrogates(&serde_json::to_string(&key).unwrap()), r#"{"\ud800":"\udc00"}"#);
    assert!(matches!(key, Value::Object(_)));
}
