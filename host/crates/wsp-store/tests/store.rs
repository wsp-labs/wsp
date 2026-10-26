// SPDX-License-Identifier: AGPL-3.0-only
// The store unit against the TS oracle (tests/fixtures/store/store.json, made by oracle/store.oracle.ts) and the
// memory store cases of packages/runtime/test/store.test.ts. The Rust host reads shape 3 where the TS fixture's lines
// say 2, so the refusal lines are written out here with 3.
mod common;

use std::time::{SystemTime, UNIX_EPOCH};

use common::*;
use serde_json::{json, Map, Value};
use wsp_store::store::*;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::wire::daemon_version::StateShape;

const BIN: &str = "/Applications/wsp.app/Contents/Resources/bin.js";
const AT: &str = "2026-09-18T15:15:00.000Z";

fn shape_doc(shape: i64) -> StateShape {
    StateShape { shape: JsInt::from(shape), wsp: "0.3.0".into(), daemon: JsInt::from(160), bin: BIN.into(), at: AT.into() }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

fn mark(bytes: f64, at: f64) -> BlobMark {
    BlobMark { bytes: JsNumber(bytes), at: JsNumber(at) }
}

fn ids_put() -> Vec<&'static str> {
    vec!["b", "2", "a", "1", "01", "4294967295", "4294967294"]
}

fn filled_with_ids() -> MemoryStore {
    let m = memory_store();
    for id in ids_put() {
        run(m.put("c", id, json!({ "id": id }))).unwrap();
    }
    m
}

#[test]
fn a_missing_record_reads_as_none() {
    let m = memory_store();
    assert_eq!(run(m.get("workspaces", "a")), Ok(None));
}

#[test]
fn records_put_get_list_and_delete_round_trip() {
    let m = memory_store();
    assert_eq!(run(m.get("workspaces", "a")), Ok(None));
    run(m.put("workspaces", "a", json!({"id": "a", "n": 1}))).unwrap();
    run(m.put("workspaces", "b", json!({"id": "b", "n": 2}))).unwrap();
    assert_eq!(run(m.get("workspaces", "a")), Ok(Some(json!({"id": "a", "n": 1}))));
    assert_eq!(run(m.list("workspaces")), Ok(vec![json!({"id": "a", "n": 1}), json!({"id": "b", "n": 2})]));
    assert_eq!(run(m.keys("workspaces")), Ok(vec!["a".to_string(), "b".to_string()]));
    run(m.delete("workspaces", "a")).unwrap();
    assert_eq!(run(m.get("workspaces", "a")), Ok(None));
    assert_eq!(run(m.list("workspaces")), Ok(vec![json!({"id": "b", "n": 2})]));
    assert_eq!(run(m.list("goldens")), Ok(vec![]));
}

#[test]
fn blobs_round_trip_as_bytes_the_latest_write_wins_and_delete_forgets() {
    let m = memory_store();
    assert_eq!(run(m.get_blob("vaults", "ws_1")), Ok(None));
    run(m.put_blob("vaults", "ws_1", b"v1".to_vec())).unwrap();
    run(m.put_blob("vaults", "ws_1", vec![0, 255, 7])).unwrap();
    assert_eq!(run(m.get_blob("vaults", "ws_1")), Ok(Some(vec![0, 255, 7])));
    run(m.delete_blob("vaults", "ws_1")).unwrap();
    assert_eq!(run(m.get_blob("vaults", "ws_1")), Ok(None));
}

#[test]
fn keys_follow_javascript_object_key_order() {
    let m = filled_with_ids();
    let f = fixture("store");
    assert_eq!(run(m.keys("c")), Ok(strings(&f["memKeys"])));
}

#[test]
fn list_follows_javascript_object_key_order() {
    let m = filled_with_ids();
    let f = fixture("store");
    assert_eq!(run(m.list("c")), Ok(f["memList"].as_array().unwrap().clone()));
}

#[test]
fn a_put_again_a_delete_and_a_put_move_ids_as_an_object_key_would_move() {
    let m = filled_with_ids();
    run(m.put("c", "b", json!({"again": true}))).unwrap();
    run(m.delete("c", "a")).unwrap();
    run(m.put("c", "a", json!({"back": true}))).unwrap();
    let f = fixture("store");
    let keys = strings(&f["afterReputDeleteAndPut"]);
    let expect: Vec<Value> = keys
        .iter()
        .map(|k| match k.as_str() {
            "b" => json!({"again": true}),
            "a" => json!({"back": true}),
            id => json!({ "id": id }),
        })
        .collect();
    assert_eq!(run(m.keys("c")), Ok(keys));
    assert_eq!(run(m.list("c")), Ok(expect));
}

#[test]
fn blob_marks_count_writes_in_this_store() {
    let m = memory_store();
    run(m.put_blob("v", "x", b"one".to_vec())).unwrap();
    run(m.put_blob("v", "y", b"two!".to_vec())).unwrap();
    run(m.put_blob("v", "x", b"three".to_vec())).unwrap();
    let f = fixture("store");
    let x: BlobMark = serde_json::from_value(f["marks"]["x"].clone()).unwrap();
    let y: BlobMark = serde_json::from_value(f["marks"]["y"].clone()).unwrap();
    assert_eq!((x, y), (mark(5.0, 3.0), mark(4.0, 2.0)));
    assert_eq!(run(m.stat_blob("v", "x")), Ok(Some(x)));
    assert_eq!(run(m.stat_blob("v", "y")), Ok(Some(y)));
}

#[test]
fn a_deleted_blob_has_no_mark() {
    let m = memory_store();
    run(m.put_blob("v", "x", b"one".to_vec())).unwrap();
    run(m.delete_blob("v", "x")).unwrap();
    assert_eq!(fixture("store")["gone"], json!("<undefined>"));
    assert_eq!(run(m.stat_blob("v", "x")), Ok(None));
}

#[test]
fn an_empty_blob_put_back_reads_as_empty_bytes_with_the_fourth_write_mark() {
    let m = memory_store();
    run(m.put_blob("v", "x", b"one".to_vec())).unwrap();
    run(m.put_blob("v", "y", b"two!".to_vec())).unwrap();
    run(m.put_blob("v", "x", b"three".to_vec())).unwrap();
    run(m.delete_blob("v", "x")).unwrap();
    run(m.put_blob("v", "x", vec![])).unwrap();
    let f = fixture("store");
    let want: BlobMark = serde_json::from_value(f["empty"]["mark"].clone()).unwrap();
    assert_eq!(want, mark(0.0, 4.0));
    assert_eq!(run(m.stat_blob("v", "x")), Ok(Some(want)));
    assert_eq!(f["empty"]["bytes"], json!([]));
    assert_eq!(run(m.get_blob("v", "x")), Ok(Some(vec![])));
}

#[test]
fn a_value_read_is_the_callers_own_copy() {
    let m = memory_store();
    run(m.put("c", "a", json!({"n": 1, "list": [1, 2]}))).unwrap();
    let mut got = run(m.get("c", "a")).unwrap().unwrap();
    got["n"] = json!(99);
    got["list"].as_array_mut().unwrap().push(json!(3));
    assert_eq!(run(m.get("c", "a")), Ok(Some(json!({"n": 1, "list": [1, 2]}))));
    let mut listed = run(m.list("c")).unwrap();
    listed[0]["n"] = json!(7);
    assert_eq!(run(m.list("c")), Ok(vec![json!({"n": 1, "list": [1, 2]})]));
}

#[test]
fn the_memory_store_keeps_no_transcript_rows() {
    let m = memory_store();
    assert!(m.transcripts().is_none());
}

#[test]
fn an_unknown_collection_lists_and_keys_as_empty() {
    let m = memory_store();
    run(m.put("c", "a", json!({"id": "a"}))).unwrap();
    assert_eq!(run(m.list("nothing")), Ok(vec![]));
    assert_eq!(run(m.keys("nothing")), Ok(Vec::<String>::new()));
}

#[test]
fn deleting_a_missing_id_is_ok() {
    let m = memory_store();
    assert_eq!(run(m.delete("c", "nothing")), Ok(()));
    run(m.put("c", "a", json!({"id": "a"}))).unwrap();
    assert_eq!(run(m.delete("c", "missing")), Ok(()));
    assert_eq!(run(m.list("c")), Ok(vec![json!({"id": "a"})]));
    assert_eq!(run(m.delete_blob("v", "missing")), Ok(()));
}

#[test]
fn the_newer_state_line_names_both_shapes_the_writer_and_the_fix() {
    assert_eq!(
        state_written_by_newer_line("/h/state.db", &shape_doc(4)),
        "/h/state.db was written by a newer wsp (state shape 4; this wsp reads 3; written 2026-09-18T15:15:00.000Z by /Applications/wsp.app/Contents/Resources/bin.js (wsp 0.3.0, daemon 160)): run that wsp, or move the file aside"
    );
}

#[test]
fn the_older_state_line_with_no_shape_recorded_says_none_recorded() {
    assert_eq!(
        state_written_by_older_line("/h/state.db", None),
        "/h/state.db holds state in an older shape (none recorded; this wsp reads shape 3): move the file aside and this wsp starts a fresh one"
    );
}

#[test]
fn the_older_state_line_with_a_shape_names_it_and_the_writer() {
    assert_eq!(
        state_written_by_older_line("/h/state.db", Some(&shape_doc(2))),
        "/h/state.db holds state in an older shape (shape 2, written 2026-09-18T15:15:00.000Z by /Applications/wsp.app/Contents/Resources/bin.js (wsp 0.3.0, daemon 160); this wsp reads shape 3): move the file aside and this wsp starts a fresh one"
    );
}

#[test]
fn the_unreadable_state_line_names_the_file_and_why() {
    let f = fixture("store");
    assert_eq!(state_unreadable_line("/h/state.db", "file is not a database"), f["lines"]["unreadable"].as_str().unwrap());
}

#[test]
fn the_shape_unreadable_line_writes_each_document_as_json_stringify_does() {
    let f = fixture("store");
    let want = strings(&f["lines"]["shapeUnreadable"]);
    let docs = [json!(3), Value::Null, json!({"shape": 2}), json!("not json {")];
    assert_eq!(want.len(), docs.len());
    for (doc, want) in docs.iter().zip(want) {
        assert_eq!(state_shape_unreadable_line("/h/state.db", doc), want, "document {doc}");
    }
}

#[test]
fn a_state_with_no_shape_recorded_is_refused_as_older() {
    let e = refuse_other_shape("/h/state.db", None).unwrap_err();
    assert_eq!(
        e.message,
        "/h/state.db holds state in an older shape (none recorded; this wsp reads shape 3): move the file aside and this wsp starts a fresh one"
    );
}

#[test]
fn a_state_in_shape_two_is_refused_as_older() {
    let e = refuse_other_shape("/h/state.db", Some(&shape_doc(2))).unwrap_err();
    assert_eq!(
        e.message,
        "/h/state.db holds state in an older shape (shape 2, written 2026-09-18T15:15:00.000Z by /Applications/wsp.app/Contents/Resources/bin.js (wsp 0.3.0, daemon 160); this wsp reads shape 3): move the file aside and this wsp starts a fresh one"
    );
}

#[test]
fn a_state_in_shape_four_is_refused_as_newer() {
    let e = refuse_other_shape("/h/state.db", Some(&shape_doc(4))).unwrap_err();
    assert_eq!(
        e.message,
        "/h/state.db was written by a newer wsp (state shape 4; this wsp reads 3; written 2026-09-18T15:15:00.000Z by /Applications/wsp.app/Contents/Resources/bin.js (wsp 0.3.0, daemon 160)): run that wsp, or move the file aside"
    );
}

#[test]
fn a_state_in_this_builds_shape_is_accepted() {
    assert_eq!(refuse_other_shape("/h/state.db", Some(&shape_doc(3))), Ok(()));
}

fn epoch_ms(at: &str) -> i64 {
    let b = at.as_bytes();
    let num = |from: usize, to: usize| at[from..to].parse::<i64>().unwrap();
    let (y, mo, d) = (num(0, 4), num(5, 7), num(8, 10));
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    assert_eq!(b.len(), 24);
    ((days * 24 + num(11, 13)) * 60 + num(14, 16)) * 60_000 + num(17, 19) * 1000 + num(20, 23)
}

fn is_iso_millis(at: &str) -> bool {
    let b = at.as_bytes();
    b.len() == 24
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'.',
            23 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

#[test]
fn shape_now_writes_shape_three_the_writers_build_and_the_time_in_that_key_order() {
    let wrote = shape_now(&writer());
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
    let mut doc: Map<String, Value> = match serde_json::to_value(&wrote).unwrap() {
        Value::Object(o) => o,
        other => panic!("not an object: {other}"),
    };
    assert_eq!(doc.keys().cloned().collect::<Vec<_>>(), ["shape", "wsp", "daemon", "bin", "at"]);
    let at = doc.remove("at").unwrap();
    let at = at.as_str().unwrap().to_string();
    assert_eq!(Value::Object(doc), json!({"shape": 3, "wsp": "test", "daemon": 1, "bin": "/usr/local/bin/wsp"}));
    assert!(is_iso_millis(&at), "at is {at}");
    assert!((epoch_ms(&at) - now).abs() <= 5000, "at {at} is not within 5 seconds of now");
}
