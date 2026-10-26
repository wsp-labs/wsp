// SPDX-License-Identifier: AGPL-3.0-only
//! `wsframe::Receiver` against ws's own Receiver: tests/fixtures/ws_receiver/corpus.json holds byte sequences and what
//! ws (the version the corpus names) emitted for each, recorded by gen.mjs beside it. Every case is replayed chunk by
//! chunk and the events are compared in order: kind, payload bytes, close code, and the fault's code and sentence.
use serde_json::{json, Value};
use wsp_serve::serve::wsframe::{Fault, Inbound, Receiver};

const CORPUS: &str = include_str!("fixtures/ws_receiver/corpus.json");

fn event(item: Result<Inbound, Fault>) -> Value {
    match item {
        Ok(Inbound::Text(text)) => json!({ "t": "text", "hex": hex::encode(text) }),
        Ok(Inbound::Binary(bytes)) => json!({ "t": "binary", "hex": hex::encode(bytes) }),
        Ok(Inbound::Ping(bytes)) => json!({ "t": "ping", "hex": hex::encode(bytes) }),
        Ok(Inbound::Pong(bytes)) => json!({ "t": "pong", "hex": hex::encode(bytes) }),
        Ok(Inbound::Close(code, reason)) => json!({ "t": "close", "code": code, "hex": hex::encode(reason) }),
        Err(fault) => json!({ "t": "error", "code": fault.code, "sentence": fault.sentence }),
    }
}

#[test]
fn every_case_reads_as_ws_reads_it() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("the corpus is JSON");
    let cases = corpus["cases"].as_array().expect("cases");
    assert!(cases.len() > 3000, "the corpus is whole");
    let mut wrong = Vec::new();
    for case in cases {
        let mut receiver = Receiver::new();
        let mut got = Vec::new();
        for chunk in case["chunks"].as_array().expect("chunks") {
            let bytes = hex::decode(chunk.as_str().expect("hex")).expect("hex");
            got.extend(receiver.push(&bytes).into_iter().map(event));
        }
        if Value::Array(got.clone()) != case["events"] {
            let short = |v: &Value| v.to_string().chars().take(300).collect::<String>();
            wrong.push(format!("{}: ws {} but wsframe {}", case["name"], short(&case["events"]), short(&Value::Array(got))));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} cases differ from ws {}:\n{}",
        wrong.len(),
        cases.len(),
        corpus["ws"],
        wrong.iter().take(25).cloned().collect::<Vec<_>>().join("\n")
    );
}
