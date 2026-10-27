// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448). Not from the round: put takes any depth now, get does not.
mod common;

use common::{run, writer, Home};
use serde_json::Value;
use wsp_store::sqlite_store::sqlite_store;
use wsp_store::store::Store;

// sqlite_store.rs:520 parse_row reads with serde_json's 128-level limit, so a row past it fails get and fails list
// whole. node's JSON.parse reads the row.
#[test]
fn a_record_200_deep_reads_back() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    let mut v = Value::Null;
    for _ in 0..200 {
        v = Value::Array(vec![v]);
    }
    run(store.put("things", "a", v)).expect("stored");
    assert!(run(store.get("things", "a")).expect("reads back").is_some());
}
