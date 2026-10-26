// SPDX-License-Identifier: AGPL-3.0-only
// The state.db of the TS host and of this one are one file (owner's ruling on Proposal 3, #2442; rules 2.19 and 2.29).
// Each test makes a state.db with one host, reads it with the other, writes through the other and has the first read
// those writes back, comparing every record through the store's own reads and every row as the file holds it. The TS
// host is the repository's own store (packages/runtime/src/sqlite-store.ts), run by oracle/cross-host.step.ts under
// vitest from the repository root; the ops both hosts apply are tests/fixtures/cross_host/ops.json.
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use rusqlite::{params, Connection};
use serde_json::{json, Map, Value};
use wsp_store::sqlite_store::{sqlite_store, SqliteStore};
use wsp_store::store::Store;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn ops() -> Value {
    fixture_at("cross_host/ops")
}

fn fixture_at(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// The TS host on `dir`: opens it, dumps, applies the op sets, dumps again. `{before, after}`, or `{error}` where the
/// TS host's open or a call refused.
fn ts_host(dir: &Path, apply: &[&str], fresh: bool) -> Value {
    let vitest = root().join("node_modules/.bin/vitest");
    assert!(vitest.exists(), "{} is missing: run pnpm install --offline at the repository root", vitest.display());
    let out = Command::new(&vitest)
        .current_dir(root())
        .args(["run", "--config", "host/crates/wsp-store/oracle/cross-host.config.ts", "--minWorkers=1", "--maxWorkers=1"])
        .env("CROSS_HOST_DIR", dir)
        .env("CROSS_HOST_APPLY", apply.join(","))
        .env("CROSS_HOST_FRESH", if fresh { "1" } else { "0" })
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the TS host step failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let text = std::fs::read_to_string(dir.join("ts-dump.json")).expect("the TS host step wrote its dump");
    std::fs::remove_file(dir.join("ts-dump.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn bytes_of(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn strings(v: &Value) -> Vec<&str> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect()
}

fn store_at(home: &Home) -> SqliteStore {
    sqlite_store(&home.state(), writer())
}

/// The op sets of the fixture applied through this host's store.
fn rust_applies(home: &Home, sets: &[&str]) {
    let ops = ops();
    let store = store_at(home);
    let rows = store.transcripts().unwrap();
    for set in sets {
        for op in ops[*set].as_array().unwrap() {
            let (name, args) = op.as_object().unwrap().iter().next().unwrap();
            let a = args.as_array().unwrap();
            let s = |i: usize| a[i].as_str().unwrap();
            match name.as_str() {
                "put" => run(store.put(s(0), s(1), a[2].clone())).unwrap(),
                "delete" => run(store.delete(s(0), s(1))).unwrap(),
                "putBlob" => run(store.put_blob(s(0), s(1), bytes_of(s(2)))).unwrap(),
                "deleteBlob" => run(store.delete_blob(s(0), s(1))).unwrap(),
                "append" => rows.append(s(0), &a[1].as_array().unwrap().iter().map(|e| event(e.clone())).collect::<Vec<_>>()).unwrap(),
                "remove" => rows.remove(s(0), &a[1].as_array().unwrap().iter().map(|p| p.as_i64().unwrap()).collect::<Vec<_>>()).unwrap(),
                "putIndex" => rows.put_index(s(0), s(1)).unwrap(),
                "clear" => rows.clear(s(0)).unwrap(),
                other => panic!("unknown op {other}"),
            }
        }
    }
}

fn events_json(events: &[wsp_wire::views::session_events_more::SessionEvent]) -> Value {
    Value::Array(events.iter().map(value).collect())
}

fn sized(events: &[wsp_wire::views::session_events_more::SessionEvent], size: u64) -> Value {
    json!({"events": events_json(events), "size": size})
}

fn sizes_json(sizes: Vec<wsp_store::sqlite_transcripts::EventSize>) -> Value {
    Value::Array(
        sizes
            .into_iter()
            .map(|s| {
                let mut o = Map::new();
                o.insert("pos".into(), json!(s.pos));
                if let Some(t) = s.thread_id {
                    o.insert("threadId".into(), json!(t));
                }
                o.insert("type".into(), json!(s.kind));
                o.insert("size".into(), json!(s.size));
                Value::Object(o)
            })
            .collect(),
    )
}

fn total_json(t: wsp_store::sqlite_transcripts::Total) -> Value {
    json!({"count": t.count, "size": t.size})
}

/// What the TS step dumps, read through this host's store and straight off the file's rows.
fn rust_dump(home: &Home) -> Value {
    let ops = ops();
    let d = &ops["dump"];
    let store = store_at(home);
    let rows = store.transcripts().unwrap();
    let mut collections = Map::new();
    for c in strings(&d["collections"]) {
        let mut get = Map::new();
        for id in d["ids"].get(c).map(strings).unwrap_or_default() {
            get.insert(id.into(), run(store.get(c, id)).unwrap().unwrap_or(Value::Null));
        }
        collections.insert(c.into(), json!({"keys": run(store.keys(c)).unwrap(), "list": run(store.list(c)).unwrap(), "get": get}));
    }
    let mut blobs = Map::new();
    for pair in d["blobs"].as_array().unwrap() {
        let (c, id) = (pair[0].as_str().unwrap(), pair[1].as_str().unwrap());
        let bytes = run(store.get_blob(c, id)).unwrap();
        let mark = run(store.stat_blob(c, id)).unwrap();
        blobs.insert(
            format!("{c}/{id}"),
            json!({"hex": bytes.map(|b| hex_of(&b)), "mark": mark.map(|m| json!({"bytes": m.bytes.0, "at": m.at.0}))}),
        );
    }
    let mut events = Map::new();
    for ws in strings(&d["workspaces"]) {
        let mut threads = Map::new();
        for t in strings(&d["threads"]) {
            let thread = rows.thread(ws, t).unwrap();
            let mut newest = Vec::new();
            rows.newest(ws, t, Some(5), &mut |e| {
                newest.push(e);
                std::ops::ControlFlow::Continue(())
            })
            .unwrap();
            threads.insert(
                t.into(),
                json!({
                    "thread": sized(&thread.events, thread.size),
                    "count": rows.count(ws, t).unwrap(),
                    "total": total_json(rows.total(ws, Some(t)).unwrap()),
                    "sizes": sizes_json(rows.sizes(ws, Some(t)).unwrap()),
                    "words": events_json(&rows.word_events(ws, t).unwrap()),
                    "newest": events_json(&newest),
                }),
            );
        }
        let all = rows.all(ws).unwrap();
        let positions: Vec<i64> = d["positions"].as_array().unwrap().iter().map(|p| p.as_i64().unwrap()).collect();
        events.insert(
            ws.into(),
            json!({
                "all": sized(&all.events, all.size),
                "newestPos": rows.newest_pos(ws).unwrap(),
                "total": total_json(rows.total(ws, None).unwrap()),
                "sizes": sizes_json(rows.sizes(ws, None).unwrap()),
                "threads": threads,
                "at": events_json(&rows.at(ws, &positions).unwrap()),
                "index": rows.index(ws).unwrap(),
            }),
        );
    }
    let c = Connection::open(home.db()).unwrap();
    let all = |sql: &str, n: usize| -> Vec<Value> {
        let mut stmt = c.prepare(sql).unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok(Value::Array(
                    (0..n)
                        .map(|i| match r.get_ref(i).unwrap() {
                            rusqlite::types::ValueRef::Integer(n) => json!(n),
                            rusqlite::types::ValueRef::Real(n) => json!(n),
                            rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                            other => panic!("{other:?}"),
                        })
                        .collect(),
                ))
            })
            .unwrap();
        rows.map(Result::unwrap).collect()
    };
    let shape: String = c.query_row("select json from shape where one = 1", [], |r| r.get(0)).unwrap();
    let shape: Map<String, Value> = serde_json::from_str(&shape).unwrap();
    let raw = json!({
        "collections": all("select collection, id, json from collections order by rowid", 3),
        "blobs": all("select collection, id, hex(bytes), at from blobs order by rowid", 4),
        "events": all("select workspace, pos, thread, type, size, json from events order by workspace, pos", 6),
        "index": all("select workspace, json from transcript_index order by workspace", 2),
        "shape": {"shape": shape["shape"], "keys": shape.keys().collect::<Vec<_>>()},
    });
    json!({"collections": collections, "blobs": blobs, "events": events, "raw": raw})
}

/// Every number as a double, since the two dumps print `5` and `5.0` for one value.
fn canon(v: Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64().unwrap()),
        Value::Array(a) => Value::Array(a.into_iter().map(canon).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, canon(v))).collect()),
        other => other,
    }
}

/// The two dumps equal, said part by part so a difference names its part.
fn same(what: &str, mine: &Value, theirs: &Value) {
    let (mine, theirs) = (canon(mine.clone()), canon(theirs.clone()));
    for part in ["collections", "blobs", "events", "raw"] {
        assert_eq!(mine[part], theirs[part], "{what}: {part}");
    }
}

fn migrations(home: &Home) -> Vec<(String, i64)> {
    let c = Connection::open(home.db()).unwrap();
    let mut stmt = c.prepare("select module, version from migrations order by module").unwrap();
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    rows.map(Result::unwrap).collect()
}

fn ts_migrations() -> Vec<(String, i64)> {
    vec![("store".to_string(), 1), ("transcripts".to_string(), 1)]
}

fn shape_number(dump: &Value) -> Value {
    dump["raw"]["shape"]["shape"].clone()
}

fn ts_error(dump: &Value) -> String {
    dump["error"].as_str().unwrap_or_else(|| panic!("the TS host did not refuse: {dump}")).to_string()
}

#[test]
fn a_state_db_the_ts_host_made_is_read_and_written_by_this_host_and_read_back_by_the_ts_host() {
    let home = Home::new();
    let made = ts_host(home.dir.path(), &["a"], true);
    assert_eq!(migrations(&home), ts_migrations(), "the TS host made the file with its migrations table");

    let ts_a = &made["after"];
    assert!(ts_a["raw"]["events"].as_array().unwrap().len() >= 6 && ts_a["raw"]["collections"].as_array().unwrap().len() >= 10);
    assert_eq!(shape_number(ts_a), json!(2));
    let mine = rust_dump(&home);
    same("this host reads what the TS host wrote", &mine, ts_a);
    assert_eq!(migrations(&home), ts_migrations(), "reading left the migrations table and its rows as they were");

    rust_applies(&home, &["b"]);
    let wrote = rust_dump(&home);
    assert_ne!(wrote["raw"], mine["raw"], "the second op set changed the file");
    assert_eq!(migrations(&home), ts_migrations(), "writing left the migrations table and its rows as they were");
    assert_eq!(shape_number(&wrote), json!(2));

    let read = ts_host(home.dir.path(), &[], false);
    same("the TS host before", &read["before"], &wrote);
    same("the TS host after", &read["after"], &wrote);
    assert_eq!(migrations(&home), ts_migrations());
}

#[test]
fn a_state_db_this_host_made_is_read_and_written_by_the_ts_host_and_read_back_by_this_host() {
    let home = Home::new();
    rust_applies(&home, &["a"]);
    let mine = rust_dump(&home);
    assert!(mine["raw"]["events"].as_array().unwrap().len() >= 6 && mine["raw"]["collections"].as_array().unwrap().len() >= 10);
    assert_eq!(shape_number(&mine), json!(2));

    let ts = ts_host(home.dir.path(), &["b"], false);
    assert!(ts.get("error").is_none(), "the TS host refused the file this host made: {}", ts["error"]);
    same("the TS host reads what this host wrote", &ts["before"], &mine);

    let back = rust_dump(&home);
    same("this host reads what the TS host wrote", &back, &ts["after"]);
    assert_ne!(back["raw"], mine["raw"], "the second op set changed the file");
    assert_eq!(shape_number(&back), json!(2));
}

#[test]
fn a_state_db_in_any_other_shape_is_refused_in_the_ts_hosts_own_words() {
    let home = Home::new();
    ts_host(home.dir.path(), &["a"], true);
    let older = r#"{"shape":1,"wsp":"0.1.0","daemon":159,"bin":"/Users/z/.local/bin/wsp","at":"2026-09-10T08:00:00.000Z"}"#;
    let newer = r#"{"shape":3,"wsp":"0.9.0","daemon":300,"bin":"/Users/z/.local/bin/wsp","at":"2026-12-10T08:00:00.000Z"}"#;
    for (what, row) in
        [("older", Some(older)), ("newer", Some(newer)), ("no shape row", None), ("not a shape document", Some(r#"{"shape":2}"#))]
    {
        let c = Connection::open(home.db()).unwrap();
        c.execute("delete from shape", []).unwrap();
        if let Some(row) = row {
            c.execute("insert into shape values (1, ?)", params![row]).unwrap();
        }
        drop(c);
        let before = std::fs::read(home.db()).unwrap();
        let theirs = ts_error(&ts_host(home.dir.path(), &[], false));
        let store = store_at(&home);
        let mine = match run(store.keys("workspaces")) {
            Ok(keys) => panic!("{what}: this host read {keys:?}"),
            Err(e) => e.message,
        };
        assert_eq!(mine, theirs, "{what}");
        assert_eq!(std::fs::read(home.db()).unwrap(), before, "{what}: a refused file is left as it was found");
    }
}
