// SPDX-License-Identifier: AGPL-3.0-only
//! The access pick (sessions.ts:1044 and :1057) writes `{ ...record, permissionMode }`: the key lands after the keys the
//! record holds, as node writes it. Expected bytes: fixtures/thread_record_order/gen.mjs, road `access`.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_acts/world.rs");

const CASES: &str = include_str!("../../wsp-core/tests/fixtures/thread_record_order/cases.json");

fn cases(road: &str) -> Vec<Value> {
    let all: Vec<Value> = serde_json::from_str(CASES).expect("the cases are JSON");
    all.into_iter().filter(|c| c["road"] == road).collect()
}

fn held(case: &Value) -> ThreadRecord {
    ThreadRecord::from_map(case["before"].as_object().expect("a record").clone()).expect("the record literal")
}

fn wrote(host: &FakeHost, thread: &str) -> String {
    serde_json::to_string(&record_of(host, thread).expect("the record is held")).expect("serializes")
}

#[test]
fn access_on_a_thread_with_no_row_writes_the_bytes_node_writes() {
    let all = cases("access");
    assert_eq!(all.len(), 4);
    for case in all {
        run_local(async move {
            let (host, area) = host();
            host.add_record("t9", "w1", held(&case));
            adapters(&host, StubAdapter::default());
            let got = area.access_row("t9", "plan", None).await.expect("an answer");
            assert_eq!(json_of(&got), json!({ "outcome": "set" }));
            assert_eq!(wrote(&host, "t9"), case["after"].as_str().expect("the bytes"), "{}", case["name"]);
        });
    }
}

#[test]
fn access_on_a_thread_with_a_row_writes_the_bytes_node_writes() {
    for case in cases("access") {
        run_local(async move {
            let (host, area) = host();
            let no_record = case["name"] == "a thread with no record yet";
            if !no_record {
                host.add_record("t1", "w1", held(&case));
            }
            let log = Rc::new(RefCell::new(Vec::new()));
            put(&host, "turn-1", "w1", json!({ "threadId": "t1", "status": "completed" }), Some(StubHandle::new(&log)));
            adapters(&host, StubAdapter::default());
            let got = area.access_row("turn-1", "plan", None).await.expect("an answer");
            assert_eq!(json_of(&got), json!({ "outcome": "set" }));
            assert_eq!(wrote(&host, "t1"), case["after"].as_str().expect("the bytes"), "{}", case["name"]);
        });
    }
}
