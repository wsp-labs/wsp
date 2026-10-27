// SPDX-License-Identifier: AGPL-3.0-only
//! Re-review of round 2 of wsp-labs/wsp-map#2448: the host's own intake of a frame against ruling 1's one cap of 500.
mod support;

use serde_json::{json, Value};
use support::*;

fn lists(n: usize) -> Value {
    let mut v = json!("x");
    for _ in 0..n {
        v = Value::Array(vec![v]);
    }
    v
}

// door.rs:530 reads every frame with wsp-words' V8 reader, which keeps 128 levels and reads a container deeper than that
// as an empty one (v8_json.rs:10). So a `slates.state` value of 200 levels, under the slates' 500 and ruling 1's one cap,
// reaches the slates as a different tree, with no error: `[[...["x"]...]]` arrives as `[[...[]...]]`. serve.ts reads the
// frame with JSON.parse, which keeps it whole, and the slate holds what the thread sent.
#[test]
fn a_slate_value_200_deep_reaches_the_slates_as_it_was_sent() {
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (host, sink) = host_socket(&served).await;
        let sent = lists(200);
        let _ = request(&host, &sink, 1, "slates.state", json!({ "threadId": "t1", "values": { "$v": sent.clone() } })).await;
        let calls = rt.script.calls_to("slates.state");
        let got = calls.first().map(|c| c.args["params"]["values"]["$v"].to_string()).unwrap_or_default();
        assert_eq!((got.matches('[').count(), got.len()), (200, sent.to_string().len()), "levels and bytes the slates were handed");
    })
}
