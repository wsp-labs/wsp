// SPDX-License-Identifier: AGPL-3.0-only
// spec/slate.md's rules on the words a slate line takes apart, outside a whole line.

use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_cli::verbs::client::Frame;
use wsp_cli::verbs::slate::{slate_ask, state_value};

use crate::common::ScriptedHost;

fn pair(word: &str) -> Option<(String, String)> {
    state_value(word).map(|(k, v)| (k, serde_json::to_string(&v).unwrap()))
}

#[test]
fn state_value_splits_at_the_first_equals_after_the_first_character() {
    assert_eq!(pair("$a=1"), Some(("$a".into(), "1".into())));
    assert_eq!(pair("a=b=c"), Some(("a".into(), "\"b=c\"".into())));
    assert_eq!(pair("=1"), None, "an = at index 0 names no path");
    assert_eq!(pair("nope"), None);
    assert_eq!(pair("i="), Some(("i".into(), "\"\"".into())), "an empty value is the empty text");
}

#[test]
fn state_value_parses_json_as_v8_does_and_keeps_text_that_does_not_parse() {
    assert_eq!(pair("$s=\"q\""), Some(("$s".into(), "\"q\"".into())));
    assert_eq!(pair("$t=two"), Some(("$t".into(), "\"two\"".into())));
    assert_eq!(pair("$lead=01"), Some(("$lead".into(), "\"01\"".into())), "a leading zero is not JSON");
    assert_eq!(pair("$pad= 1 "), Some(("$pad".into(), "1".into())), "JSON's own whitespace is read past");
    assert_eq!(pair("$exp=0.1e1"), Some(("$exp".into(), "1".into())));
    assert_eq!(pair("$neg=-0"), Some(("$neg".into(), "0".into())), "negative zero is sent as 0");
    assert_eq!(pair("$big=12345678901234567890"), Some(("$big".into(), "12345678901234567000".into())), "a number past 2^53 is the double");
    assert_eq!(pair("$huge=1e400"), Some(("$huge".into(), "null".into())), "Infinity is sent as null");
    assert_eq!(
        pair("$o={\"b\":1,\"a\":[2],\"b\":3}"),
        Some(("$o".into(), "{\"b\":3,\"a\":[2]}".into())),
        "a repeated key keeps its first place"
    );
}

#[test]
fn slate_ask_keeps_every_other_key_in_order() {
    let mut replies = IndexMap::new();
    replies
        .insert("slates.read".to_string(), r#"{"version":4,"id":1,"text":"s","ok":true,"values":{"b":1,"a":2},"problems":[]}"#.to_string());
    let host = ScriptedHost::new(replies, IndexMap::new());
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let mut params = Frame::new();
    params.insert("threadId".into(), json!("t-1"));
    let answer = rt.block_on(slate_ask(host.as_ref(), "slates.read", params)).unwrap();
    assert_eq!(serde_json::to_string(&Value::Object(answer)).unwrap(), r#"{"version":4,"text":"s","values":{"b":1,"a":2},"problems":[]}"#);
    assert_eq!(host.sent(), [r#"{"threadId":"t-1","id":2,"op":"slates.read"}"#]);
}
