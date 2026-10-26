// SPDX-License-Identifier: AGPL-3.0-only
// The SQLite store against what sqlite-store.ts does (spec/sqlite_store.md): values, order, the open, the shape and
// its refusals, the schema, and the build. Ports store.test.ts's sqliteStore cases; the rest pin rules of chapter 02.
mod common;

use std::fs;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::*;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use wsp_store::sqlite_store::{sqlite_store, state_db_path, SCHEMA};
use wsp_store::store::{StateWriter, Store};
use wsp_wire::types::js_number::JsInt;

const NEWER_BIN: &str = "/Applications/wsp.app/Contents/Resources/bin.js";

fn raw(home: &Home) -> Connection {
    Connection::open(home.db()).unwrap()
}

/// A database laid down by hand: the fresh schema, one record, and the shape row as given (`None` for no row). The
/// row text is written as it stands, so a test can store text that is not JSON.
fn seed(home: &Home, shape_row: Option<&str>) {
    let c = raw(home);
    for s in SCHEMA {
        c.execute_batch(s).unwrap();
    }
    c.execute("insert into collections values ('workspaces', 'a', '{\"id\":\"a\"}')", []).unwrap();
    if let Some(row) = shape_row {
        c.execute("insert into shape values (1, ?)", params![row]).unwrap();
    }
}

fn shape_row(home: &Home) -> Value {
    let c = raw(home);
    let text: String = c.query_row("select json from shape", [], |r| r.get(0)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn db_text(home: &Home) -> String {
    home.db().display().to_string()
}

fn message<T: std::fmt::Debug>(r: Result<T, wsp_words::exit::HostError>) -> String {
    match r {
        Ok(v) => panic!("expected a refusal, got {v:?}"),
        Err(e) => e.message,
    }
}

/// Every read and write the store has, each refused in the one sentence (store.test.ts:230-238).
fn refuses_all(store: &dyn Store, refusal: &str) {
    assert_eq!(message(run(store.get("workspaces", "a"))), refusal, "get");
    assert_eq!(message(run(store.list("workspaces"))), refusal, "list");
    assert_eq!(message(run(store.keys("workspaces"))), refusal, "keys");
    assert_eq!(message(run(store.put("workspaces", "b", json!({"id": "b"})))), refusal, "put");
    assert_eq!(message(run(store.delete("workspaces", "a"))), refusal, "delete");
    assert_eq!(message(run(store.put_blob("vaults", "ws_1", b"x".to_vec()))), refusal, "put_blob");
    assert_eq!(message(run(store.get_blob("vaults", "ws_1"))), refusal, "get_blob");
    assert_eq!(message(run(store.stat_blob("vaults", "ws_1"))), refusal, "stat_blob");
    assert_eq!(message(run(store.delete_blob("vaults", "ws_1"))), refusal, "delete_blob");
}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() * 1000.0
}

#[test]
fn state_db_sits_beside_the_state_file() {
    assert_eq!(state_db_path("/h/wsp/state.json".as_ref()), std::path::PathBuf::from("/h/wsp/state.db"));
}

#[test]
fn get_put_list_delete_round_trips() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    assert_eq!(run(store.get("workspaces", "a")).unwrap(), None);
    run(store.put("workspaces", "a", json!({"id": "a", "n": 1}))).unwrap();
    run(store.put("workspaces", "b", json!({"id": "b", "n": 2}))).unwrap();
    assert_eq!(run(store.get("workspaces", "a")).unwrap(), Some(json!({"id": "a", "n": 1})));
    assert_eq!(run(store.list("workspaces")).unwrap(), vec![json!({"id": "a", "n": 1}), json!({"id": "b", "n": 2})]);
    run(store.delete("workspaces", "a")).unwrap();
    assert_eq!(run(store.get("workspaces", "a")).unwrap(), None);
    assert_eq!(run(store.list("goldens")).unwrap(), Vec::<Value>::new());
}

#[test]
fn blobs_round_trip_as_bytes_latest_write_wins_delete_forgets() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    assert_eq!(run(store.get_blob("vaults", "ws_1")).unwrap(), None);
    run(store.put_blob("vaults", "ws_1", b"v1".to_vec())).unwrap();
    run(store.put_blob("vaults", "ws_1", vec![0, 255, 7])).unwrap();
    assert_eq!(run(store.get_blob("vaults", "ws_1")).unwrap(), Some(vec![0, 255, 7]));
    run(store.delete_blob("vaults", "ws_1")).unwrap();
    assert_eq!(run(store.get_blob("vaults", "ws_1")).unwrap(), None);
    assert_eq!(run(store.stat_blob("vaults", "ws_1")).unwrap(), None);
}

#[test]
fn survives_a_new_instance_over_the_same_state_in_first_write_order() {
    let home = Home::new();
    let s1 = sqlite_store(&home.state(), writer());
    run(s1.put("goldens", "default", json!({"head": 1}))).unwrap();
    run(s1.put("goldens", "b", json!({"head": 2}))).unwrap();
    run(s1.put("goldens", "a", json!({"head": 3}))).unwrap();
    run(s1.put("goldens", "default", json!({"head": 4}))).unwrap();
    run(s1.put_blob("vaults", "ws_1", b"tgz".to_vec())).unwrap();
    let s2 = sqlite_store(&home.state(), writer());
    assert_eq!(run(s2.get("goldens", "default")).unwrap(), Some(json!({"head": 4})));
    assert_eq!(run(s2.keys("goldens")).unwrap(), vec!["default", "b", "a"]);
    assert_eq!(run(s2.list("goldens")).unwrap(), vec![json!({"head": 4}), json!({"head": 2}), json!({"head": 3})]);
    assert_eq!(run(s2.get_blob("vaults", "ws_1")).unwrap(), Some(b"tgz".to_vec()));
}

#[test]
fn marks_each_write_of_a_blob_apart_in_wall_clock_ms_and_none_for_a_missing_blob() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    assert_eq!(run(store.stat_blob("transcripts", "w")).unwrap(), None);
    let before = now_ms();
    run(store.put_blob("transcripts", "w", b"one".to_vec())).unwrap();
    let first = run(store.stat_blob("transcripts", "w")).unwrap().unwrap();
    run(store.put_blob("transcripts", "w", b"two".to_vec())).unwrap();
    let second = run(store.stat_blob("transcripts", "w")).unwrap().unwrap();
    let after = now_ms();
    assert_eq!((first.bytes.0, second.bytes.0), (3.0, 3.0));
    assert!(second.at.0 - first.at.0 >= 0.001 - 1e-6, "{} then {}", first.at.0, second.at.0);
    // Wall-clock milliseconds (rule 2.9), not a counter.
    assert!(first.at.0 >= before - 5.0 && second.at.0 <= after + 5.0, "{before} <= {} <= {} <= {after}", first.at.0, second.at.0);
    run(store.delete_blob("transcripts", "w")).unwrap();
    assert_eq!(run(store.stat_blob("transcripts", "w")).unwrap(), None);
}

#[test]
fn a_write_another_instance_made_keeps_its_records_beside_this_ones() {
    let home = Home::new();
    let mine = sqlite_store(&home.state(), writer());
    let theirs = sqlite_store(&home.state(), writer());
    run(mine.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    run(theirs.get("workspaces", "a")).unwrap();
    run(mine.put("workspaces", "p", json!({"id": "p"}))).unwrap();
    run(theirs.put("workspaces", "q", json!({"id": "q"}))).unwrap();
    assert_eq!(run(sqlite_store(&home.state(), writer()).keys("workspaces")).unwrap(), vec!["a", "p", "q"]);
    assert_eq!(run(mine.keys("workspaces")).unwrap(), vec!["a", "p", "q"]);
}

#[test]
fn hands_out_copies_so_a_caller_changing_what_it_read_changes_nothing_kept() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    run(store.put("workspaces", "a", json!({"id": "a", "phase": "running"}))).unwrap();
    let mut got = run(store.get("workspaces", "a")).unwrap().unwrap();
    got["phase"] = json!("paused");
    let mut listed = run(store.list("workspaces")).unwrap();
    listed[0]["phase"] = json!("gone");
    run(store.put("workspaces", "b", json!({"id": "b"}))).unwrap();
    assert_eq!(run(sqlite_store(&home.state(), writer()).get("workspaces", "a")).unwrap(), Some(json!({"id": "a", "phase": "running"})));
}

#[test]
fn stores_a_record_as_json_stringify_writes_it() {
    let f = fixture("sqlite_store");
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    let nums: Value = serde_json::from_str(
        r#"{"z":1,"a":2,"2":0,"n":1.5,"big":1e21,"small":1e-7,"neg0":-0.0,"whole":10.0,"nested":{"10":1,"b":[1,"x",null]}}"#,
    )
    .unwrap();
    run(store.put("r", "nums", nums)).unwrap();
    run(store.put("r", "null", Value::Null)).unwrap();
    run(store.put("r", "text", json!("héllo \u{2028} \u{1} / \"q\""))).unwrap();
    let rows: Vec<(String, String, String)> = raw(&home)
        .prepare("select collection, id, json from collections where collection = 'r' order by rowid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let want: Vec<(String, String, String)> = f["raw"].as_array().unwrap()[..3]
        .iter()
        .map(|r| (r["collection"].as_str().unwrap().into(), r["id"].as_str().unwrap().into(), r["json"].as_str().unwrap().into()))
        .collect();
    assert_eq!(rows, want);
    assert_eq!(run(store.get("r", "null")).unwrap(), Some(Value::Null));
    // A record reads back in the order its text holds, index keys first as JSON.stringify left them.
    let back = run(store.get("r", "nums")).unwrap().unwrap();
    assert_eq!(back.as_object().unwrap().keys().collect::<Vec<_>>(), vec!["2", "z", "a", "n", "big", "small", "neg0", "whole", "nested"]);
}

#[test]
fn keeps_first_write_order_a_reput_keeps_its_place_a_delete_and_put_goes_last() {
    let f = fixture("sqlite_store");
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    for id in ["a", "b", "c"] {
        run(store.put("o", id, json!({"id": id}))).unwrap();
    }
    run(store.put("o", "a", json!({"id": "a", "again": true}))).unwrap();
    run(store.delete("o", "b")).unwrap();
    run(store.put("o", "b", json!({"id": "b", "back": true}))).unwrap();
    run(store.delete("o", "missing")).unwrap();
    let keys: Vec<String> = serde_json::from_value(f["keys"].clone()).unwrap();
    assert_eq!(run(store.keys("o")).unwrap(), keys);
    assert_eq!(Value::Array(run(store.list("o")).unwrap()), f["list"]);
    assert_eq!(run(store.list("nothing")).unwrap(), Vec::<Value>::new());
    assert_eq!(run(store.keys("nothing")).unwrap(), Vec::<String>::new());
}

#[test]
fn an_empty_blob_reads_back_empty_and_marks_zero_bytes() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    run(store.put_blob("v", "e", Vec::new())).unwrap();
    assert_eq!(run(store.get_blob("v", "e")).unwrap(), Some(Vec::new()));
    assert_eq!(run(store.stat_blob("v", "e")).unwrap().unwrap().bytes.0, 0.0);
}

#[test]
fn keeps_its_records_in_state_db_in_wal_mode_and_writes_no_state_json() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    run(store.put_blob("transcripts", "a", b"events".to_vec())).unwrap();
    assert!(!home.state().exists());
    assert!(!home.dir.path().join("blobs").exists());
    assert_eq!(home.names(), vec!["state.db", "state.db-shm", "state.db-wal"]);
    let c = raw(&home);
    let tables: Vec<String> = c
        .prepare("select name from sqlite_master where type = 'table' order by name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(tables, vec!["blobs", "collections", "events", "migrations", "shape", "transcript_index"]);
    let mode: String = c.query_row("pragma journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode, "wal");
    let rows: Vec<(String, String, String)> = c
        .prepare("select collection, id, json from collections")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows, vec![("workspaces".to_string(), "a".to_string(), "{\"id\":\"a\"}".to_string())]);
}

#[test]
fn a_fresh_database_holds_the_schema_the_ts_hosts_migrations_rows_and_the_shape_row_and_nothing_else() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    assert_eq!(run(store.keys("workspaces")).unwrap(), Vec::<String>::new());
    let c = raw(&home);
    let made: Vec<String> = c
        .prepare("select sql from sqlite_master where sql is not null order by rowid")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let kept = |s: &&str| s.replacen("create table", "CREATE TABLE", 1).replacen("create index", "CREATE INDEX", 1);
    assert_eq!(made, SCHEMA.iter().map(kept).collect::<Vec<_>>());
    for table in ["collections", "blobs", "events", "transcript_index"] {
        let n: i64 = c.query_row(&format!("select count(*) from {table}"), [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "{table}");
    }
    // The rows the TS host leaves, which it needs to open the file; this host never reads them.
    let migrated: Vec<(String, i64)> = c
        .prepare("select module, version from migrations order by rowid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(migrated, vec![("store".to_string(), 1), ("transcripts".to_string(), 1)]);
    let shape = shape_row(&home);
    assert_eq!(shape.as_object().unwrap().keys().collect::<Vec<_>>(), vec!["shape", "wsp", "daemon", "bin", "at"]);
    assert_eq!(
        (shape["shape"].clone(), shape["wsp"].clone(), shape["daemon"].clone(), shape["bin"].clone()),
        (json!(2), json!("test"), json!(1), json!("/usr/local/bin/wsp"))
    );
}

#[test]
fn the_shape_is_beside_the_collections_and_no_collection_read_sees_it() {
    let home = Home::new();
    let w = StateWriter { wsp: "0.2.0".into(), daemon: JsInt::from(160), bin: "/Users/z/.local/bin/wsp".into() };
    let store = sqlite_store(&home.state(), w);
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    let held = shape_row(&home);
    assert_eq!(
        (held["shape"].clone(), held["wsp"].clone(), held["daemon"].clone(), held["bin"].clone()),
        (json!(2), json!("0.2.0"), json!(160), json!("/Users/z/.local/bin/wsp"))
    );
    assert_eq!(run(store.list("$shape")).unwrap(), Vec::<Value>::new());
    assert_eq!(run(store.keys("$shape")).unwrap(), Vec::<String>::new());
    assert_eq!(run(store.get("$shape", "shape")).unwrap(), None);
    run(store.put("projects", "p", json!({"id": "p"}))).unwrap();
    assert_eq!(run(store.keys("workspaces")).unwrap(), vec!["a"]);
}

#[test]
fn reads_never_write_the_shape_and_each_stores_first_write_stamps_it_again() {
    let home = Home::new();
    let one = StateWriter { wsp: "0.1.0".into(), daemon: JsInt::from(1), bin: "/one".into() };
    let two = StateWriter { wsp: "0.2.0".into(), daemon: JsInt::from(2), bin: "/two".into() };
    run(sqlite_store(&home.state(), one).put("workspaces", "a", json!({"id": "a"}))).unwrap();
    let first = shape_row(&home);
    assert_eq!(first["bin"], json!("/one"));
    let second = sqlite_store(&home.state(), two);
    run(second.get("workspaces", "a")).unwrap();
    run(second.list("workspaces")).unwrap();
    run(second.stat_blob("v", "x")).unwrap();
    assert_eq!(shape_row(&home), first, "a read wrote the shape row");
    std::thread::sleep(Duration::from_millis(5));
    run(second.delete_blob("v", "x")).unwrap();
    let stamped = shape_row(&home);
    assert_eq!((stamped["wsp"].clone(), stamped["daemon"].clone(), stamped["bin"].clone()), (json!("0.2.0"), json!(2), json!("/two")));
    assert_ne!(stamped["at"], first["at"]);
}

#[test]
fn refuses_every_call_of_a_state_a_newer_wsp_wrote_and_leaves_its_bytes() {
    let home = Home::new();
    let wrote = json!({"shape": 3, "wsp": "0.3.0", "daemon": 161, "bin": NEWER_BIN, "at": "2026-09-18T15:15:00.000Z"});
    seed(&home, Some(&wrote.to_string()));
    let before = fs::read(home.db()).unwrap();
    let refusal = format!(
        "{} was written by a newer wsp (state shape 3; this wsp reads 2; written 2026-09-18T15:15:00.000Z by {NEWER_BIN} (wsp 0.3.0, daemon 161)): run that wsp, or move the file aside",
        db_text(&home)
    );
    refuses_all(&sqlite_store(&home.state(), writer()), &refusal);
    assert_eq!(fs::read(home.db()).unwrap(), before);
    assert_eq!(home.names(), vec!["state.db"]);
}

#[test]
fn refuses_every_call_of_a_state_whose_shape_document_does_not_parse() {
    // A copy had its shape set to the bare number 2 by hand: no build can be named off that.
    for (row, shown) in [("2", "2"), ("null", "null"), (r#"{"shape":2}"#, r#"{"shape":2}"#), ("not json {", r#""not json {""#)] {
        let home = Home::new();
        seed(&home, Some(row));
        let before = fs::read(home.db()).unwrap();
        let refusal = format!(
            "{} has a $shape that is not a shape document ({shown}), so no wsp can say which build wrote it or in which shape: move the file aside, or put back the document a wsp writes",
            db_text(&home)
        );
        refuses_all(&sqlite_store(&home.state(), writer()), &refusal);
        assert_eq!(fs::read(home.db()).unwrap(), before, "{row}");
    }
}

#[test]
fn refuses_every_call_of_a_state_in_an_older_shape_or_with_no_shape_row() {
    let older = json!({"shape": 1, "wsp": "0.1.0", "daemon": 159, "bin": "/Users/z/.local/bin/wsp", "at": "2026-09-10T08:00:00.000Z"});
    for (row, words) in [
        (
            Some(older.to_string()),
            "shape 1, written 2026-09-10T08:00:00.000Z by /Users/z/.local/bin/wsp (wsp 0.1.0, daemon 159)".to_string(),
        ),
        (None, "none recorded".to_string()),
    ] {
        let home = Home::new();
        seed(&home, row.as_deref());
        let before = fs::read(home.db()).unwrap();
        let refusal = format!(
            "{} holds state in an older shape ({words}; this wsp reads shape 2): move the file aside and this wsp starts a fresh one",
            db_text(&home)
        );
        refuses_all(&sqlite_store(&home.state(), writer()), &refusal);
        assert_eq!(fs::read(home.db()).unwrap(), before);
    }
}

#[test]
fn opens_a_ts_hosts_state_db_as_it_is_and_leaves_its_migrations_table_and_rows() {
    let home = Home::new();
    let c = raw(&home);
    // The TS host's seven statements (sqlite-store.ts:30-32,137; sqlite-transcripts.ts:15-17) and what it writes.
    c.execute_batch(
        "create table collections (collection text not null, id text not null, json text not null, primary key (collection, id));
         create table blobs (collection text not null, id text not null, bytes blob not null, at real not null, primary key (collection, id));
         create table shape (one integer primary key check (one = 1), json text not null);
         create table if not exists migrations (module text primary key, version integer not null);
         create table events (workspace text not null, pos integer not null, thread text not null, type text not null, size integer not null, json text not null, primary key (workspace, pos));
         create index events_thread on events (workspace, thread, pos);
         create table transcript_index (workspace text primary key, json text not null);
         insert into migrations values ('store', 1), ('transcripts', 1);
         insert into collections values ('owner', 'id', '{\"id\":\"h_0a993e4c\"}');
         insert into shape values (1, '{\"shape\":2,\"wsp\":\"0.3.8\",\"daemon\":167,\"bin\":\"/usr/local/lib/wsp/bin.js\",\"at\":\"2026-10-25T07:00:00.000Z\"}');",
    )
    .unwrap();
    drop(c);
    let schema_of = |c: &Connection| -> Vec<(String, String)> {
        c.prepare("select name, sql from sqlite_master where sql is not null order by rowid")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let schema = schema_of(&raw(&home));
    let store = sqlite_store(&home.state(), writer());
    assert_eq!(run(store.get("owner", "id")).unwrap(), Some(json!({"id": "h_0a993e4c"})));
    assert_eq!(shape_row(&home)["wsp"], json!("0.3.8"), "a read wrote the shape row");
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    assert_eq!(run(store.keys("workspaces")).unwrap(), vec!["a"]);
    let c = raw(&home);
    assert_eq!(shape_row(&home)["shape"], json!(2));
    assert_eq!(shape_row(&home)["wsp"], json!("test"), "the first write stamps the shape row with this writer");
    assert_eq!(schema_of(&c), schema, "the schema was left as the TS host made it");
    let migrations: Vec<(String, i64)> = c
        .prepare("select module, version from migrations order by module")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(migrations, vec![("store".to_string(), 1), ("transcripts".to_string(), 1)]);
}

#[test]
fn refuses_a_state_another_process_moved_to_a_newer_shape_while_held_open() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    let wrote = json!({"shape": 3, "wsp": "0.3.0", "daemon": 1, "bin": "/Users/z/.local/bin/wsp", "at": "2026-09-19T08:00:00.000Z"});
    raw(&home).execute("update shape set json = ?", params![wrote.to_string()]).unwrap();
    let refusal = format!(
        "{} was written by a newer wsp (state shape 3; this wsp reads 2; written 2026-09-19T08:00:00.000Z by /Users/z/.local/bin/wsp (wsp 0.3.0, daemon 1)): run that wsp, or move the file aside",
        db_text(&home)
    );
    assert_eq!(message(run(store.get("workspaces", "a"))), refusal);
    assert_eq!(message(store.transcripts().unwrap().all("w")), refusal, "the transcript rows reach the database through the same check");
}

#[test]
fn a_refused_open_is_tried_again_whole_at_the_next_call() {
    let home = Home::new();
    let newer = json!({"shape": 3, "wsp": "0.3.0", "daemon": 1, "bin": "/b", "at": "2026-09-19T08:00:00.000Z"});
    seed(&home, Some(&newer.to_string()));
    let store = sqlite_store(&home.state(), writer());
    assert!(message(run(store.keys("workspaces"))).contains("was written by a newer wsp"));
    let fixed = json!({"shape": 2, "wsp": "0.4.0", "daemon": 1, "bin": "/b", "at": "2026-09-19T08:00:00.000Z"});
    raw(&home).execute("update shape set json = ?", params![fixed.to_string()]).unwrap();
    assert_eq!(run(store.keys("workspaces")).unwrap(), vec!["a"]);
}

#[test]
fn the_transcript_rows_of_a_refused_state_refuse_in_the_same_sentence() {
    let home = Home::new();
    seed(&home, None);
    let store = sqlite_store(&home.state(), writer());
    let refusal = format!(
        "{} holds state in an older shape (none recorded; this wsp reads shape 2): move the file aside and this wsp starts a fresh one",
        db_text(&home)
    );
    let rows = store.transcripts().expect("the SQLite store keeps transcripts as rows");
    assert_eq!(message(rows.all("w")), refusal);
    assert_eq!(message(rows.append("w", &[text(Some("a"), 1, "x")])), refusal);
    assert_eq!(message(rows.index("w")), refusal);
}

#[test]
fn makes_the_states_folder_where_it_is_not_made_yet_the_database_the_owners_alone() {
    use std::os::unix::fs::PermissionsExt;
    let home = Home::new();
    let folder = home.dir.path().join("not/made/yet");
    let store = sqlite_store(&folder.join("state.json"), writer());
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    assert_eq!(run(store.keys("workspaces")).unwrap(), vec!["a"]);
    assert_eq!(fs::metadata(folder.join("state.db")).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(&folder).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
fn refuses_a_state_db_that_is_no_database_naming_it_and_leaves_its_bytes() {
    let home = Home::new();
    fs::write(home.db(), "not a database at all, just some bytes someone left here").unwrap();
    let before = fs::read(home.db()).unwrap();
    let store = sqlite_store(&home.state(), writer());
    let refusal = format!("{} does not read as a state file (file is not a database), so no wsp can read the records in it: move the file aside, or put back a copy a wsp wrote", db_text(&home));
    assert_eq!(message(run(store.keys("workspaces"))), refusal);
    assert_eq!(message(run(store.put("workspaces", "a", json!({"id": "a"})))), refusal);
    assert_eq!(fs::read(home.db()).unwrap(), before);
}

#[test]
fn a_zero_byte_state_db_opens_as_a_fresh_database() {
    let home = Home::new();
    fs::write(home.db(), b"").unwrap();
    let store = sqlite_store(&home.state(), writer());
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    assert_eq!(run(store.keys("workspaces")).unwrap(), vec!["a"]);
    assert_eq!(shape_row(&home)["shape"], json!(2));
}

#[test]
fn a_state_file_beside_the_database_is_never_read_nor_moved() {
    let home = Home::new();
    let ts_state = r#"{"workspaces":{"ws_1":{"id":"ws_1"}},"$shape":{"shape":2,"wsp":"0.3.5","daemon":150,"bin":"/b","at":"2026-09-01T00:00:00.000Z"}}"#;
    fs::write(home.state(), ts_state).unwrap();
    let store = sqlite_store(&home.state(), writer());
    assert_eq!(run(store.keys("workspaces")).unwrap(), Vec::<String>::new());
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    assert_eq!(fs::read_to_string(home.state()).unwrap(), ts_state);
    assert_eq!(home.names(), vec!["state.db", "state.db-shm", "state.db-wal", "state.json"]);
}

#[test]
fn a_database_with_tables_and_no_shape_table_fails_on_the_schema_and_keeps_its_rows() {
    let home = Home::new();
    let c = raw(&home);
    c.execute_batch(
        "create table collections (collection text not null, id text not null, json text not null, primary key (collection, id));
         insert into collections values ('workspaces', 'a', '{}');",
    )
    .unwrap();
    drop(c);
    let store = sqlite_store(&home.state(), writer());
    assert_eq!(message(run(store.keys("workspaces"))), "table collections already exists");
    let c = raw(&home);
    let n: i64 = c.query_row("select count(*) from collections", [], |r| r.get(0)).unwrap();
    let shape: i64 = c.query_row("select count(*) from sqlite_master where name = 'shape'", [], |r| r.get(0)).unwrap();
    assert_eq!((n, shape), (1, 0), "the schema's transaction rolled back whole");
}

#[test]
fn the_main_connection_runs_with_the_pragmas_of_chapter_02() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    assert_eq!(store.pragma("journal_mode").unwrap(), json!("wal"));
    assert_eq!(store.pragma("busy_timeout").unwrap(), json!(10000));
    assert_eq!(store.pragma("synchronous").unwrap(), json!(1));
    let page = store.pragma("page_size").unwrap().as_u64().unwrap();
    assert_eq!(store.pragma("wal_autocheckpoint").unwrap(), json!((64 * 1024 * 1024u64).div_ceil(page)));
    assert_eq!(store.pragma("journal_size_limit").unwrap(), json!(16 * 1024 * 1024));
    assert_eq!(store.pragma("foreign_keys").unwrap(), json!(1), "never set: node's binding and the bundled SQLite both default it on");
}

#[test]
fn a_write_waits_on_another_connections_lock_rather_than_failing() {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    let db = home.db();
    let (locked, wait) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let c = Connection::open(db).unwrap();
        c.execute_batch("begin immediate").unwrap();
        locked.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        c.execute_batch("commit").unwrap();
    });
    wait.recv().unwrap();
    let started = Instant::now();
    run(store.put("workspaces", "b", json!({"id": "b"}))).unwrap();
    assert!(started.elapsed() >= Duration::from_millis(300), "the put did not wait: {:?}", started.elapsed());
    holder.join().unwrap();
    assert_eq!(run(store.keys("workspaces")).unwrap(), vec!["a", "b"]);
}

#[test]
fn a_build_clears_dead_and_own_leftovers_and_keeps_a_live_processes() {
    let home = Home::new();
    let own = format!("state.db.importing-{}", std::process::id());
    // pid 1 is alive; no process has a pid this high (pid_max is at most 2^22).
    for name in [own.as_str(), "state.db.importing-1", "state.db.importing-2147483646", "state.db.importing-2147483646-wal"] {
        fs::write(home.dir.path().join(name), b"left").unwrap();
    }
    let store = sqlite_store(&home.state(), writer());
    run(store.keys("workspaces")).unwrap();
    assert_eq!(home.names(), vec!["state.db", "state.db-shm", "state.db-wal", "state.db.importing-1"]);
    assert_eq!(fs::read(home.dir.path().join("state.db.importing-1")).unwrap(), b"left");
}
