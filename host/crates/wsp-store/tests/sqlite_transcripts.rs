// SPDX-License-Identifier: AGPL-3.0-only
// The transcripts module against sqlite-transcripts.ts (spec/sqlite_transcripts.md), through the SQLite store, which is
// the only road to the rows: the module's own four cases of sqlite-transcripts.test.ts, and the oracle's answers.
mod common;

use std::ops::ControlFlow;

use common::*;
use rusqlite::Connection;
use serde_json::{json, Value};
use wsp_store::sqlite_store::{sqlite_store, SqliteStore};
use wsp_store::sqlite_transcripts::{EventSize, Total, TranscriptRows};
use wsp_store::store::Store;
use wsp_store::transcript_reader::event_json;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

fn rows(store: &SqliteStore) -> &dyn TranscriptRows {
    store.transcripts().expect("the SQLite store keeps transcripts as rows")
}

fn poses(events: &[SessionEvent]) -> Vec<i64> {
    events.iter().map(|e| pos_of(e).unwrap()).collect()
}

fn newest(r: &dyn TranscriptRows, ws: &str, thread: &str, before: Option<i64>) -> Vec<i64> {
    let mut seen = Vec::new();
    r.newest(ws, thread, before, &mut |e| {
        seen.push(pos_of(&e).unwrap());
        ControlFlow::Continue(())
    })
    .unwrap();
    seen
}

/// The ten events of the oracle's `sqlite_transcripts` case, off its fixture.
fn oracle_events() -> Vec<SessionEvent> {
    fixture("sqlite_transcripts")["all"]["events"].as_array().unwrap()[..10].iter().cloned().map(event).collect()
}

fn fresh() -> (Home, SqliteStore) {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    (home, store)
}

#[test]
fn keeps_each_event_as_a_row_read_back_in_order_and_a_threads_newest_first_under_a_position() {
    let (_home, store) = fresh();
    let r = rows(&store);
    r.append("w", &[text(Some("a"), 1, "line 1"), text(Some("b"), 2, "line 2"), text(None, 3, "line 3"), text(Some("a"), 4, "line 4")])
        .unwrap();
    r.append("v", &[text(Some("a"), 1, "line 1")]).unwrap();
    let all = r.all("w").unwrap();
    assert_eq!(poses(&all.events), vec![1, 2, 3, 4]);
    assert_eq!(all.size, all.events.iter().map(|e| event_json(e).bytes).sum::<u64>());
    assert_eq!(newest(r, "w", "a", None), vec![4, 1]);
    assert_eq!(newest(r, "w", "a", Some(4)), vec![1]);
    assert_eq!(r.count("w", "a").unwrap(), 2);
    assert_eq!(r.total("w", None).unwrap(), Total { count: 4, size: all.size });
    let sizes: Vec<(i64, Option<String>, String)> = r.sizes("w", None).unwrap().into_iter().map(|s| (s.pos, s.thread_id, s.kind)).collect();
    assert_eq!(
        sizes,
        vec![
            (1, Some("a".into()), "session.delta".into()),
            (2, Some("b".into()), "session.delta".into()),
            (3, None, "session.delta".into()),
            (4, Some("a".into()), "session.delta".into()),
        ]
    );
    r.remove("w", &[1, 3]).unwrap();
    assert_eq!(poses(&r.all("w").unwrap().events), vec![2, 4]);
    assert_eq!(r.at("w", &[4]).unwrap(), vec![text(Some("a"), 4, "line 4")]);
    assert_eq!(poses(&r.all("v").unwrap().events), vec![1], "another workspace's rows are its own");
}

#[test]
fn reads_a_threads_window_off_the_index_on_workspace_thread_pos() {
    let (home, store) = fresh();
    rows(&store).append("w", &[text(Some("a"), 1, "x")]).unwrap();
    let c = Connection::open(home.db()).unwrap();
    let plan: Vec<String> = c
        .prepare("explain query plan select json from events where workspace = ? and thread = ? and pos < ? order by pos desc")
        .unwrap()
        .query_map(rusqlite::params!["w", "a", 9], |r| r.get::<_, String>(3))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let plan = plan.join("\n");
    assert!(plan.contains("USING INDEX events_thread (workspace=? AND thread=? AND pos<?)"), "{plan}");
    assert!(!plan.contains("TEMP B-TREE"), "{plan}");
}

#[test]
fn undoes_the_whole_of_a_transaction_that_fails() {
    let (_home, store) = fresh();
    let r = rows(&store);
    r.append("w", &[text(Some("a"), 1, "one")]).unwrap();
    let failed = r.atomically(&mut || {
        r.append("w", &[text(Some("a"), 2, "two")])?;
        r.put_index("w", "{}")?;
        Err(HostError {
            message: "stop".into(),
            fix: None,
            kind: None,
            errors: None,
            warnings: None,
            left: None,
            host_key: None,
            code: None,
            status: None,
            marks: vec![],
        })
    });
    assert_eq!(failed.unwrap_err().message, "stop");
    assert_eq!(poses(&r.all("w").unwrap().events), vec![1]);
    assert_eq!(r.index("w").unwrap(), None);
    // The depth went back down: the next call is a transaction of its own and commits.
    r.atomically(&mut || r.put_index("w", "after")).unwrap();
    assert_eq!(r.index("w").unwrap(), Some("after".into()));
}

#[test]
fn clears_one_workspaces_rows_and_index_and_leaves_every_other() {
    let (_home, store) = fresh();
    let r = rows(&store);
    r.append("w", &[text(Some("a"), 1, "w")]).unwrap();
    r.append("v", &[text(Some("a"), 1, "v")]).unwrap();
    r.put_index("w", "{}").unwrap();
    r.put_index("v", "{}").unwrap();
    r.clear("w").unwrap();
    assert_eq!(r.all("w").unwrap().events, Vec::<SessionEvent>::new());
    assert_eq!(r.index("w").unwrap(), None);
    assert_eq!(r.all("v").unwrap().events, vec![text(Some("a"), 1, "v")]);
    assert_eq!(r.index("v").unwrap(), Some("{}".into()));
}

#[test]
fn stores_each_event_with_its_thread_type_utf16_size_and_json_as_the_ts_does() {
    let f = fixture("sqlite_transcripts");
    let (home, store) = fresh();
    rows(&store).append("w", &oracle_events()).unwrap();
    let c = Connection::open(home.db()).unwrap();
    let got: Vec<(String, i64, String, String, i64, Value)> = c
        .prepare("select workspace, pos, thread, type, size, json from events order by pos")
        .unwrap()
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, serde_json::from_str(&r.get::<_, String>(5)?).unwrap()))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let want: Vec<(String, i64, String, String, i64, Value)> = f["raw"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["workspace"].as_str().unwrap().into(),
                r["pos"].as_i64().unwrap(),
                r["thread"].as_str().unwrap().into(),
                r["type"].as_str().unwrap().into(),
                r["size"].as_i64().unwrap(),
                serde_json::from_str(r["json"].as_str().unwrap()).unwrap(),
            )
        })
        .collect();
    assert_eq!(got, want);
}

#[test]
fn reads_threads_counts_totals_and_sizes_as_the_ts_does() {
    let f = fixture("sqlite_transcripts");
    let (_home, store) = fresh();
    let r = rows(&store);
    r.append("w", &oracle_events()).unwrap();
    r.append("w", &[text(Some("a"), 20, "inner")]).unwrap();
    let all = r.all("w").unwrap();
    assert_eq!(all.events.iter().map(value).collect::<Vec<_>>(), f["all"]["events"].as_array().unwrap().clone());
    assert_eq!(all.size, f["all"]["size"].as_u64().unwrap());
    let thread = r.thread("w", "a").unwrap();
    assert_eq!(thread.events.iter().map(value).collect::<Vec<_>>(), f["thread"]["events"].as_array().unwrap().clone());
    assert_eq!(thread.size, f["thread"]["size"].as_u64().unwrap());
    let empty = r.thread("w", "").unwrap();
    assert_eq!(empty.events.iter().map(value).collect::<Vec<_>>(), f["threadEmpty"]["events"].as_array().unwrap().clone());
    assert_eq!(empty.size, f["threadEmpty"]["size"].as_u64().unwrap());
    let words: Vec<i64> = serde_json::from_value(f["wordEvents"].clone()).unwrap();
    assert_eq!(poses(&r.word_events("w", "a").unwrap()), words);
    for (key, before) in [("newest", None), ("newestBelow7", Some(7)), ("newestBelow0", Some(0))] {
        let want: Vec<i64> = serde_json::from_value(f[key].clone()).unwrap();
        assert_eq!(newest(r, "w", "a", before), want, "{key}");
    }
    assert_eq!([r.count("w", "a").unwrap(), r.count("w", "").unwrap(), r.count("w", "zzz").unwrap()], [8, 2, 0]);
    let total = |v: &Value| Total { count: v["count"].as_u64().unwrap(), size: v["size"].as_u64().unwrap() };
    assert_eq!(r.total("w", None).unwrap(), total(&f["total"]["all"]));
    assert_eq!(r.total("w", Some("a")).unwrap(), total(&f["total"]["a"]));
    assert_eq!(r.total("w", Some("")).unwrap(), total(&f["total"]["empty"]));
    assert_eq!(r.total("nope", None).unwrap(), Total { count: 0, size: 0 });
    assert_eq!((r.newest_pos("w").unwrap(), r.newest_pos("nope").unwrap()), (20, 0));
    let size = |v: &Value| EventSize {
        pos: v["pos"].as_i64().unwrap(),
        thread_id: v.get("threadId").map(|t| t.as_str().unwrap().to_string()),
        kind: v["type"].as_str().unwrap().into(),
        size: v["size"].as_u64().unwrap(),
    };
    let want: Vec<EventSize> = f["sizes"].as_array().unwrap().iter().map(size).collect();
    assert_eq!(r.sizes("w", None).unwrap(), want);
    assert_eq!(r.sizes("w", Some("")).unwrap(), f["sizesEmpty"].as_array().unwrap().iter().map(size).collect::<Vec<_>>());
    assert_eq!(poses(&r.at("w", &[4, 4, 99, 1]).unwrap()), vec![4, 4, 1]);
}

#[test]
fn an_event_with_no_thread_and_one_with_an_empty_thread_are_both_thread_empty_and_read_back_apart() {
    let (_home, store) = fresh();
    let r = rows(&store);
    r.append("w", &[text(None, 1, "none"), text(Some(""), 2, "empty")]).unwrap();
    let back = r.at("w", &[1, 2]).unwrap();
    assert_eq!(value(&back[0]).get("threadId"), None);
    assert_eq!(value(&back[1]).get("threadId"), Some(&json!("")));
    assert_eq!(r.sizes("w", Some("")).unwrap().iter().map(|s| s.thread_id.clone()).collect::<Vec<_>>(), vec![None, None]);
}

#[test]
fn newest_stops_where_its_reader_stops() {
    let (_home, store) = fresh();
    let r = rows(&store);
    r.append("w", &(1..=5).map(|p| text(Some("a"), p, "x")).collect::<Vec<_>>()).unwrap();
    let mut seen = Vec::new();
    r.newest("w", "a", None, &mut |e| {
        seen.push(pos_of(&e).unwrap());
        if seen.len() == 2 {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })
    .unwrap();
    assert_eq!(seen, vec![5, 4]);
}

#[test]
fn invariant_2_append_refuses_a_position_already_kept_and_lands_nothing_of_it() {
    let (_home, store) = fresh();
    let r = rows(&store);
    r.append("w", &oracle_events()).unwrap();
    let twice = r.append("w", &[text(Some("a"), 12, "new"), text(Some("a"), 1, "again")]);
    assert_eq!(twice.unwrap_err().message, fixture("sqlite_transcripts")["twice"]["err"].as_str().unwrap());
    assert_eq!(poses(&r.all("w").unwrap().events), (1..=10).collect::<Vec<_>>());
    assert_eq!(r.at("w", &[1]).unwrap(), vec![oracle_events()[0].clone()], "the kept row is the first write's");
}

#[test]
fn an_event_with_no_position_is_refused_and_nothing_of_its_append_lands() {
    let (_home, store) = fresh();
    let r = rows(&store);
    r.append("w", &[text(Some("a"), 1, "kept")]).unwrap();
    assert!(r.append("w", &[delta(Some("a"), None, "no pos", "text"), text(Some("a"), 11, "after")]).is_err());
    assert_eq!(poses(&r.all("w").unwrap().events), vec![1]);
}

#[test]
fn an_inner_transaction_joins_the_outer_so_a_caught_inner_failure_keeps_its_writes() {
    let f = fixture("sqlite_transcripts");
    let (_home, store) = fresh();
    let r = rows(&store);
    r.append("w", &oracle_events()).unwrap();
    let mut caught = String::new();
    r.atomically(&mut || {
        r.put_index("w", "outer")?;
        let inner = r.atomically(&mut || {
            r.append("w", &[text(Some("a"), 20, "inner")])?;
            Err(HostError {
                message: "inner stop".into(),
                fix: None,
                kind: None,
                errors: None,
                warnings: None,
                left: None,
                host_key: None,
                code: None,
                status: None,
                marks: vec![],
            })
        });
        caught = inner.unwrap_err().message;
        Ok(())
    })
    .unwrap();
    assert_eq!(caught, f["nested"]["caught"].as_str().unwrap());
    assert_eq!(r.index("w").unwrap().as_deref(), f["nested"]["index"].as_str());
    let want: Vec<i64> = serde_json::from_value(f["nested"]["pos"].clone()).unwrap();
    assert_eq!(poses(&r.all("w").unwrap().events), want);
}

#[test]
fn invariant_5_an_index_that_does_not_parse_is_kept_verbatim_and_writes_go_on() {
    let (home, store) = fresh();
    let r = rows(&store);
    r.put_index("w", "{\"taken\": [not json").unwrap();
    assert_eq!(r.index("w").unwrap().as_deref(), Some("{\"taken\": [not json"));
    r.append("w", &[text(Some("a"), 1, "a turn after")]).unwrap();
    r.put_index("w", "{\"pos\":1}").unwrap();
    assert_eq!(r.index("w").unwrap().as_deref(), Some("{\"pos\":1}"));
    drop(store);
    // A new store over the same state reads both back: rows and index survive a restart.
    let again = sqlite_store(&home.state(), writer());
    let r = rows(&again);
    assert_eq!(r.all("w").unwrap().events, vec![text(Some("a"), 1, "a turn after")]);
    assert_eq!(r.index("w").unwrap().as_deref(), Some("{\"pos\":1}"));
}

#[test]
fn a_row_that_does_not_parse_as_an_event_fails_the_read() {
    let (home, store) = fresh();
    rows(&store).append("w", &[text(Some("a"), 1, "fine")]).unwrap();
    Connection::open(home.db()).unwrap().execute("insert into events values ('w', 2, 'a', 'session.delta', 3, 'not json')", []).unwrap();
    assert!(rows(&store).all("w").is_err());
    assert_eq!(poses(&rows(&store).at("w", &[1]).unwrap()), vec![1], "a read that skips the row still answers");
}

#[test]
fn a_transcript_call_opens_the_database_as_any_store_call_does() {
    let (home, store) = fresh();
    assert_eq!(rows(&store).newest_pos("w").unwrap(), 0);
    assert!(home.db().exists());
    assert_eq!(run(store.keys("workspaces")).unwrap(), Vec::<String>::new());
}
