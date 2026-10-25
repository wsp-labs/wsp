// SPDX-License-Identifier: AGPL-3.0-only
// parse_toml reads what smol-toml 1.9.0 reads and fails in its words, the code block under the first line too (oracle/toml_parse.oracle.mjs).
use serde_json::{json, Value};
use wsp_catalog::smol_toml::parse_toml;

mod oracle_common;
use oracle_common::{fixture, s};

fn encode(v: &toml::Value) -> Value {
    match v {
        toml::Value::String(x) => json!(x),
        toml::Value::Integer(i) => json!(i),
        toml::Value::Float(f) if f.is_finite() => json!(f),
        toml::Value::Float(f) => json!(if f.is_nan() { "NaN".to_string() } else if *f > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }),
        toml::Value::Boolean(b) => json!(b),
        toml::Value::Datetime(_) => json!({ "$date": true }),
        toml::Value::Array(a) => Value::Array(a.iter().map(encode).collect()),
        toml::Value::Table(t) => Value::Object(t.iter().map(|(k, v)| (k.clone(), encode(v))).collect()),
    }
}

#[test]
fn every_text_reads_or_fails_as_smol_toml_reads_it() {
    let cases = fixture("toml_parse.json");
    let cases = cases.as_array().unwrap();
    assert!(cases.len() > 5000);
    let mut wrong = vec![];
    for case in cases {
        let text = s(&case["text"]);
        let got = parse_toml(text);
        let same = match (&got, case.get("ok"), case.get("error")) {
            (Ok(tree), Some(ok), _) => {
                let want: Value = serde_json::from_str(s(ok)).unwrap();
                let have = encode(&toml::Value::Table(tree.clone()));
                have.to_string() == want.to_string()
            }
            (Err(e), _, Some(error)) => e == s(error),
            _ => false,
        };
        if !same {
            let shown = got.map(|t| encode(&toml::Value::Table(t)).to_string());
            wrong.push(format!("{text:?}\n   got {shown:?}\n  want {case}"));
        }
    }
    assert!(wrong.is_empty(), "{} of {} differ, the first ten:\n{}", wrong.len(), cases.len(), wrong.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn nesting_past_the_limit_is_refused_in_smol_tomls_words_and_never_overflows_the_stack() {
    let deep = format!("a = {}{}", "[".repeat(5000), "]".repeat(5000));
    let e = parse_toml(&deep).unwrap_err();
    assert!(e.starts_with("Invalid TOML document: document contains excessively nested structures. aborting.\n\n1:  a = ["), "{}", &e[..120]);
}
