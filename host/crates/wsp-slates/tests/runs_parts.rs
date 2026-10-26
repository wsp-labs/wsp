// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_parts: spec/runs_parts.md, the record and key halves of slate-runs.test.ts, and the oracle's `key.*`,
//! `stableJson`, `asText`, `timeoutOf` and `spent` values. Constants are only text and cannot be red on their own, so
//! their test starts with a call into `is_env_name`, which is the unit's one function among them.
mod support;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_slates::slate_runs::{
    as_text, ended_record, files_unwritten, is_env_name, last_result, map_strings, restarted_record, rewound_record, run_key,
    running_record, spent, stable_json, timeout_of, CmdRunDecl, RunBy, RunRecord, RunState, BACKED_OFF, BACKOFF_AFTER, BACKOFF_MS,
    CLOSE_GRACE_MS, DEFAULT_TIMEOUT_S, HELD_APPROVAL, HELD_ASLEEP, HELD_BUDGET, HELD_BUSY, HELD_PRESSED, LINES_KEPT, NO_FOLDER,
    RESULT_FIELDS, RUNNING_MAX, SECRET_DOTS, STARTS_PER_MINUTE, TIMER_FLOOR_S,
};
use wsp_wire::types::js_number::JsNumber;

fn rec(v: Value) -> RunRecord {
    RunRecord(v.as_object().expect("an object").clone())
}

fn show(r: &RunRecord) -> String {
    serde_json::to_string(r).unwrap()
}

fn show_map(m: &Map<String, Value>) -> String {
    serde_json::to_string(m).unwrap()
}

fn cmd(c: &str) -> CmdRunDecl {
    CmdRunDecl { cmd: c.into(), ..Default::default() }
}

fn map_of(pairs: &[(&str, Value)]) -> IndexMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

mod process_lifetime {
    use super::*;

    // runs_parts Edge case: slate-runs.test.ts:377 (the record half, restartedRecord and rewoundRecord)
    #[test]
    fn stopall_quiet_drops_completions_and_writes_nothing_restart_and_rewind_rewrite_ru() {
        let was = rec(json!({"state": "running", "runs": 2, "startedAt": 1}));
        assert_eq!(
            show(&restarted_record(&was, 9)),
            r#"{"state":"failed","why":"the host restarted while it ran","exit":null,"runs":2,"startedAt":1,"endedAt":9}"#
        );
        let rewound = rewound_record(&was, 9);
        assert_eq!(rewound.0.get("state"), Some(&json!("cancelled")));
        assert_eq!(rewound.0.get("why"), Some(&json!("the thread was rewound")));
        let done = rec(json!({"state": "done", "runs": 1}));
        assert_eq!(show(&restarted_record(&done, 9)), r#"{"state":"done","runs":1}"#);
    }
}

mod then {
    use super::*;

    // runs_parts Behaviour 7: slate-runs.test.ts:395 (the key lines: then is part of the key)
    #[test]
    fn reshapes_the_output_into_json_keeps_out_raw_is_part_of_the_key_and_shows_on_the() {
        let shape = r#""/usr/bin/node" "/x/lines-to-json.mjs""#;
        let decl = CmdRunDecl { then: Some(shape.into()), ..cmd("printf 'x'") };
        assert_ne!(run_key(&decl), run_key(&cmd("printf 'x'")));
        assert_ne!(run_key(&decl), run_key(&CmdRunDecl { then: Some("cat".into()), ..decl.clone() }));
    }
}

// runs_parts Behaviour 1: the six states name themselves, and parse back
#[test]
fn run_state_names_and_parses_the_six_states() {
    for (state, name) in [
        (RunState::Idle, "idle"),
        (RunState::Held, "held"),
        (RunState::Running, "running"),
        (RunState::Done, "done"),
        (RunState::Failed, "failed"),
        (RunState::Cancelled, "cancelled"),
    ] {
        assert_eq!(state.as_str(), name);
        assert_eq!(RunState::parse(name), Some(state));
    }
    assert_eq!(RunState::parse("paused"), None);
    assert_eq!(RunState::parse("Done"), None);
    assert_eq!(RunState::parse(""), None);
}

// runs_parts Behaviour 1: who started a run names itself
#[test]
fn run_by_names_each_starter() {
    assert_eq!(RunBy::Person.as_str(), "person");
    assert_eq!(RunBy::Agent.as_str(), "agent");
    assert_eq!(RunBy::Reaction.as_str(), "reaction");
    assert_eq!(RunBy::Timer.as_str(), "timer");
}

// runs_parts Behaviour 1: the getters read state, runs (0 when absent), why and startedAt
#[test]
fn a_record_reads_its_state_runs_why_and_start() {
    let r = rec(json!({"state": "held", "why": "needs your approval", "runs": 3, "startedAt": 7, "out": "x"}));
    assert_eq!(r.state(), Some(RunState::Held));
    assert_eq!(r.runs(), 3);
    assert_eq!(r.why(), Some("needs your approval"));
    assert_eq!(r.started_at(), Some(7));
    assert_eq!(r.get("out"), Some(&json!("x")));
    assert_eq!(r.get("missing"), None);
    let bare = rec(json!({"state": "idle"}));
    assert_eq!(bare.runs(), 0);
    assert_eq!(bare.why(), None);
    assert_eq!(bare.started_at(), None);
    let odd = rec(json!({"state": "paused", "runs": 1}));
    assert_eq!(odd.state(), None);
}

// runs_parts Behaviour 1: a record comes from a stored object whose state is a string and runs a number
#[test]
fn a_record_is_read_from_a_stored_object_and_written_back_in_its_order() {
    let stored = json!({"state": "done", "exit": 0, "out": "x\n", "startedAt": 1, "endedAt": 2, "ms": 1, "runs": 1});
    let r = RunRecord::from_json(&stored).expect("a record");
    assert_eq!(serde_json::to_string(&r.to_json()).unwrap(), serde_json::to_string(&stored).unwrap());
    assert!(RunRecord::from_json(&json!({"state": "done"})).is_none());
    assert!(RunRecord::from_json(&json!({"state": 1, "runs": 1})).is_none());
    assert!(RunRecord::from_json(&json!({"state": "done", "runs": "1"})).is_none());
    assert!(RunRecord::from_json(&json!([1])).is_none());
    assert!(RunRecord::from_json(&json!(null)).is_none());
}

// runs_parts Behaviour 3: a running record keeps its result only when refreshing is true, in RESULT_FIELDS order
#[test]
fn last_result_of_a_refreshing_running_record_is_its_result_fields_in_order() {
    let r = rec(
        json!({"state": "running", "ms": 3, "out": "x", "exit": null, "runs": 2, "startedAt": 5, "refreshing": true, "endedAt": 9, "why": "w", "json": null}),
    );
    let got = last_result(&r).expect("a result");
    assert_eq!(show_map(&got), r#"{"exit":null,"out":"x","json":null,"endedAt":9,"ms":3}"#);
}

// runs_parts Behaviour 3: a running record that is not refreshing keeps nothing
#[test]
fn last_result_of_a_running_record_that_is_not_refreshing_is_none() {
    assert!(last_result(&rec(json!({"state": "running", "runs": 1, "startedAt": 5, "out": "x"}))).is_none());
    assert!(last_result(&rec(json!({"state": "running", "runs": 1, "startedAt": 5, "out": "x", "refreshing": false}))).is_none());
}

// runs_parts Behaviour 3: done, failed and cancelled keep their result when they have startedAt
#[test]
fn last_result_of_a_finished_record_with_a_start_is_its_result_fields() {
    for state in ["done", "failed", "cancelled"] {
        let r = rec(
            json!({"state": state, "why": "w", "exit": 1, "err": "e", "out": "o", "lines": ["a"], "startedAt": 1, "endedAt": 4, "ms": 3, "cut": true, "runs": 2}),
        );
        let got = last_result(&r).expect(state);
        assert_eq!(show_map(&got), r#"{"exit":1,"out":"o","err":"e","lines":["a"],"endedAt":4,"ms":3,"cut":true}"#, "{state}");
    }
}

// runs_parts Behaviour 3: a finished record that never started keeps nothing
#[test]
fn last_result_of_a_finished_record_without_a_start_is_none() {
    for state in ["done", "failed", "cancelled"] {
        assert!(last_result(&rec(json!({"state": state, "exit": null, "runs": 1, "endedAt": 4}))).is_none(), "{state}");
    }
}

// runs_parts Behaviour 3: a held, idle or never-started record keeps nothing, whatever else it holds
#[test]
fn last_result_of_a_held_or_idle_record_is_none() {
    assert!(last_result(&rec(json!({"state": "held", "why": "needs your approval", "runs": 1, "startedAt": 1, "out": "x"}))).is_none());
    assert!(last_result(&rec(json!({"state": "idle", "runs": 0, "startedAt": 1, "out": "x"}))).is_none());
    assert!(last_result(&rec(json!({"state": "idle", "runs": 0}))).is_none());
}

// runs_parts Behaviour 3: a finished record with no result field answers an empty result, not none
#[test]
fn last_result_of_a_finished_record_with_no_result_field_is_empty() {
    let got = last_result(&rec(json!({"state": "done", "startedAt": 1, "runs": 1}))).expect("empty result");
    assert_eq!(show_map(&got), "{}");
}

// runs_parts Behaviour 2: a key holding null counts as present
#[test]
fn last_result_keeps_a_null_exit_and_a_null_json() {
    let got = last_result(&rec(json!({"state": "cancelled", "exit": null, "json": null, "startedAt": 1, "runs": 1}))).expect("a result");
    assert_eq!(show_map(&got), r#"{"exit":null,"json":null}"#);
}

// runs_parts Behaviour 2: the result fields and their order
#[test]
fn the_result_fields_are_these_eight_in_this_order() {
    let sentence = ["exit", "out", "err", "json", "lines", "endedAt", "ms", "cut"];
    let all = rec(
        json!({"state": "done", "cut": true, "ms": 1, "endedAt": 2, "lines": [], "json": 0, "err": "", "out": "", "exit": 0, "startedAt": 1, "runs": 1}),
    );
    let got = last_result(&all).expect("a result");
    assert_eq!(got.keys().map(|k| k.as_str()).collect::<Vec<_>>(), sentence);
    assert_eq!(RESULT_FIELDS, sentence);
}

// runs_parts Behaviour 1: running without a kept result is state, runs, startedAt
#[test]
fn running_record_without_a_result_is_state_runs_started_at() {
    assert_eq!(show(&running_record(None, 1, 5)), r#"{"state":"running","runs":1,"startedAt":5}"#);
}

// runs_parts Behaviour 1: running with a kept result is the result, then state, runs, startedAt, refreshing
#[test]
fn running_record_with_a_result_keeps_it_then_state_runs_started_at_refreshing() {
    let done = rec(json!({"state": "done", "exit": 0, "out": "o\n", "json": {"a": 1}, "startedAt": 1, "endedAt": 4, "ms": 3, "runs": 2}));
    let last = last_result(&done).expect("a result");
    assert_eq!(
        show(&running_record(Some(&last), 3, 7)),
        r#"{"exit":0,"out":"o\n","json":{"a":1},"endedAt":4,"ms":3,"state":"running","runs":3,"startedAt":7,"refreshing":true}"#
    );
}

// runs_parts Behaviour 1: an empty kept result still marks the record refreshing
#[test]
fn running_record_with_an_empty_result_is_still_refreshing() {
    let empty = Map::new();
    assert_eq!(show(&running_record(Some(&empty), 2, 9)), r#"{"state":"running","runs":2,"startedAt":9,"refreshing":true}"#);
}

// runs_parts Behaviour 1: ended from outside is state, why, exit null, runs, startedAt, endedAt
#[test]
fn ended_record_drops_the_kept_result_and_keeps_the_start() {
    let was = rec(json!({"exit": 0, "out": "o", "state": "running", "runs": 2, "startedAt": 1, "refreshing": true}));
    assert_eq!(
        show(&ended_record(&was, RunState::Failed, "gone", 9)),
        r#"{"state":"failed","why":"gone","exit":null,"runs":2,"startedAt":1,"endedAt":9}"#
    );
    assert_eq!(
        show(&ended_record(&was, RunState::Cancelled, "stopped", 12)),
        r#"{"state":"cancelled","why":"stopped","exit":null,"runs":2,"startedAt":1,"endedAt":12}"#
    );
}

// runs_parts Behaviour 1: a record with no startedAt ends without one
#[test]
fn ended_record_without_a_start_has_no_started_at() {
    let was = rec(json!({"state": "running", "runs": 1}));
    assert_eq!(
        show(&ended_record(&was, RunState::Failed, "gone", 9)),
        r#"{"state":"failed","why":"gone","exit":null,"runs":1,"endedAt":9}"#
    );
}

// runs_parts Behaviour 4: a running record drops its kept result when the host restarted
#[test]
fn restarted_record_drops_the_kept_result_of_a_refreshing_record() {
    let was = rec(json!({"exit": 0, "out": "o", "state": "running", "runs": 2, "startedAt": 1, "refreshing": true}));
    assert_eq!(
        show(&restarted_record(&was, 9)),
        r#"{"state":"failed","why":"the host restarted while it ran","exit":null,"runs":2,"startedAt":1,"endedAt":9}"#
    );
    let unstarted = rec(json!({"state": "running", "runs": 1}));
    assert_eq!(
        show(&restarted_record(&unstarted, 9)),
        r#"{"state":"failed","why":"the host restarted while it ran","exit":null,"runs":1,"endedAt":9}"#
    );
}

// runs_parts Behaviour 4: any other record comes back unchanged
#[test]
fn restarted_record_leaves_every_other_state_as_it_is() {
    for text in [
        r#"{"state":"done","exit":0,"out":"x","startedAt":1,"endedAt":2,"ms":1,"runs":1}"#,
        r#"{"state":"held","why":"needs your approval","runs":1}"#,
        r#"{"state":"idle","runs":0}"#,
        r#"{"state":"failed","why":"w","exit":null,"runs":1,"endedAt":3}"#,
        r#"{"state":"cancelled","why":"w","exit":null,"runs":1,"endedAt":3}"#,
    ] {
        let was = rec(serde_json::from_str(text).unwrap());
        assert_eq!(show(&restarted_record(&was, 9)), text);
    }
}

// runs_parts Behaviour 4: a rewind makes a running record cancelled, saying so
#[test]
fn rewound_record_makes_a_running_record_cancelled() {
    let was = rec(json!({"state": "running", "runs": 2, "startedAt": 1}));
    assert_eq!(
        show(&rewound_record(&was, 9)),
        r#"{"state":"cancelled","why":"the thread was rewound","exit":null,"runs":2,"startedAt":1,"endedAt":9}"#
    );
    let kept = rec(json!({"exit": 0, "state": "running", "runs": 2, "startedAt": 1, "refreshing": true}));
    assert_eq!(
        show(&rewound_record(&kept, 4)),
        r#"{"state":"cancelled","why":"the thread was rewound","exit":null,"runs":2,"startedAt":1,"endedAt":4}"#
    );
}

// runs_parts Behaviour 4: a rewind leaves every other record as it is
#[test]
fn rewound_record_leaves_every_other_state_as_it_is() {
    for text in [r#"{"state":"done","runs":1}"#, r#"{"state":"held","why":"needs your approval","runs":1}"#, r#"{"state":"idle","runs":0}"#]
    {
        let was = rec(serde_json::from_str(text).unwrap());
        assert_eq!(show(&rewound_record(&was, 9)), text);
    }
}

// runs_parts Edge case: oracle spent
#[test]
fn spent_keeps_the_stamps_under_a_minute_and_refuses_past_twelve() {
    assert_eq!(spent(None, 0), Some(vec![0]));
    let twelve: Vec<i64> = (0..12).collect();
    assert_eq!(spent(Some(&twelve), 59_999), None);
    assert_eq!(spent(Some(&twelve), 60_000), Some(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 60_000]));
}

// runs_parts Behaviour 5: under twelve the stamp is appended, and the twelfth stamp fills the budget
#[test]
fn spent_appends_the_stamp_until_twelve_are_kept() {
    assert_eq!(spent(Some(&[1, 2]), 5), Some(vec![1, 2, 5]));
    assert_eq!(spent(Some(&[]), 5), Some(vec![5]));
    let eleven: Vec<i64> = (0..11).collect();
    let twelve = spent(Some(&eleven), 20).expect("the twelfth");
    assert_eq!(twelve.len(), 12);
    assert_eq!(spent(Some(&twelve), 21), None);
}

// runs_parts Behaviour 5: a stamp exactly 60,000 ms old has aged out
#[test]
fn spent_drops_a_stamp_that_is_exactly_a_minute_old() {
    assert_eq!(spent(Some(&[0, 1]), 60_000), Some(vec![1, 60_000]));
    assert_eq!(spent(Some(&[0, 1]), 59_999), Some(vec![0, 1, 59_999]));
}

// runs_parts Behaviour 6: oracle stableJson
#[test]
fn stable_json_sorts_object_keys_and_keeps_array_order() {
    let v = json!({"b": [1, {"d": null, "c": "x"}], "a": null, "\u{e9}": true});
    assert_eq!(stable_json(Some(&v)), "{\"a\":null,\"b\":[1,{\"c\":\"x\",\"d\":null}],\"\u{e9}\":true}");
}

// runs_parts Behaviour 6: oracle stableJson.undefined
#[test]
fn stable_json_of_nothing_is_null() {
    assert_eq!(stable_json(None), "null");
    assert_eq!(stable_json(Some(&json!(null))), "null");
}

// runs_parts Behaviour 6: numbers are written as JS writes them
#[test]
fn stable_json_writes_numbers_as_js_does() {
    assert_eq!(stable_json(Some(&json!(20))), "20");
    assert_eq!(stable_json(Some(&json!(20.0))), "20");
    assert_eq!(stable_json(Some(&json!(2.5))), "2.5");
    assert_eq!(stable_json(Some(&json!(1e21))), "1e+21");
    assert_eq!(stable_json(Some(&json!([30.0, {"n": 0.5}]))), r#"[30,{"n":0.5}]"#);
}

// runs_parts Behaviour 6: strings and keys are written as JSON.stringify writes them
#[test]
fn stable_json_escapes_strings_and_keys() {
    let v = json!({"a\"b": "x\ny\"z\\", "t": true, "f": false, "e": [], "o": {}});
    assert_eq!(stable_json(Some(&v)), r#"{"a\"b":"x\ny\"z\\","e":[],"f":false,"o":{},"t":true}"#);
}

// runs_parts Behaviour 6 / Rule 8.8: keys sort by UTF-16 units, so a character past U+FFFF sorts before U+FF5E
#[test]
fn stable_json_sorts_keys_by_utf16_units() {
    let v = json!({"\u{ff5e}": 2, "\u{1f600}": 1});
    assert_eq!(stable_json(Some(&v)), "{\"\u{1f600}\":1,\"\u{ff5e}\":2}");
}

// runs_parts Edge case: oracle key.plain
#[test]
fn run_key_of_a_plain_command_is_the_oracles_hash() {
    assert_eq!(run_key(&cmd("true")), "6144e0b9c343b38f3a9a13eb194e63bf");
}

// runs_parts Edge case: oracle key.env
#[test]
fn run_key_with_an_env_binding_is_the_oracles_hash() {
    let decl = CmdRunDecl { env: Some(map_of(&[("ID", json!({"bind": "$id"}))])), ..cmd("echo \"$ID\"") };
    assert_eq!(run_key(&decl), "d8ddf13625ab00f9e8014751fb9a4720");
}

// runs_parts Edge case: oracle key.full
#[test]
fn run_key_of_the_full_declaration_is_the_oracles_hash() {
    let decl = CmdRunDecl {
        cmd: "x".into(),
        env: Some(map_of(&[("B", json!("$b")), ("A", json!({"bind": "$a"}))])),
        args: Some(vec![json!("$x"), json!({"bind": "$y"})]),
        stdin: Some(json!({"format": "${$z}"})),
        on: Some("host".into()),
        cwd: Some("sub".into()),
        timeout: Some(JsNumber(20.0)),
        stream: Some(true),
        confirm: None,
        every: Some(JsNumber(30.0)),
        always: Some(true),
        once: Some(true),
        then: Some("jq .".into()),
        files: Some(IndexMap::from([("b.sh".to_string(), "2".to_string()), ("a.sh".to_string(), "1".to_string())])),
        scripts: Some(IndexMap::from([("x.sh".to_string(), "abc".to_string())])),
    };
    assert_eq!(run_key(&decl), "4fd3f52da753cc4eadb19e8406b302e1");
}

// runs_parts Edge case: oracle key.timeout60_same_as_absent
#[test]
fn run_key_of_timeout_60_is_the_key_of_no_timeout() {
    assert_eq!(run_key(&CmdRunDecl { timeout: Some(JsNumber(60.0)), ..cmd("true") }), run_key(&cmd("true")));
    assert_eq!(run_key(&CmdRunDecl { timeout: Some(JsNumber(60.0)), ..cmd("true") }), "6144e0b9c343b38f3a9a13eb194e63bf");
}

// runs_parts Edge case: oracle key.on_thread_same_as_absent
#[test]
fn run_key_of_on_thread_is_the_key_of_no_on() {
    assert_eq!(run_key(&CmdRunDecl { on: Some("thread".into()), ..cmd("true") }), run_key(&cmd("true")));
    assert_eq!(run_key(&CmdRunDecl { on: Some("thread".into()), ..cmd("true") }), "6144e0b9c343b38f3a9a13eb194e63bf");
}

// runs_parts Edge case: oracle key.confirm_not_in_key
#[test]
fn run_key_does_not_move_with_confirm() {
    assert_eq!(run_key(&CmdRunDecl { confirm: Some("Sure?".into()), ..cmd("true") }), "6144e0b9c343b38f3a9a13eb194e63bf");
}

// runs_parts Behaviour 7: a literal null stdin differs from no stdin
#[test]
fn run_key_keeps_a_null_stdin_apart_from_none() {
    let none = run_key(&cmd("true"));
    let null = run_key(&CmdRunDecl { stdin: Some(Value::Null), ..cmd("true") });
    assert_ne!(none, null);
    assert_eq!(null.len(), 32);
    assert_ne!(null, run_key(&CmdRunDecl { stdin: Some(json!("")), ..cmd("true") }));
}

// runs_parts Behaviour 7 / Rule 3.11: the env names are sorted for the key, the expressions are not
#[test]
fn run_key_sorts_env_names_and_not_args() {
    let ab = CmdRunDecl { env: Some(map_of(&[("A", json!("$a")), ("B", json!("$b"))])), ..cmd("x") };
    let ba = CmdRunDecl { env: Some(map_of(&[("B", json!("$b")), ("A", json!("$a"))])), ..cmd("x") };
    assert_eq!(run_key(&ab), run_key(&ba));
    let swapped = CmdRunDecl { env: Some(map_of(&[("A", json!("$b")), ("B", json!("$a"))])), ..cmd("x") };
    assert_ne!(run_key(&ab), run_key(&swapped));
    let xy = CmdRunDecl { args: Some(vec![json!("$x"), json!("$y")]), ..cmd("x") };
    let yx = CmdRunDecl { args: Some(vec![json!("$y"), json!("$x")]), ..cmd("x") };
    assert_ne!(run_key(&xy), run_key(&yx));
}

// runs_parts Behaviour 7: every part the key lists moves it
#[test]
fn run_key_moves_with_each_part_it_lists() {
    let base = run_key(&cmd("x"));
    let moved = [
        CmdRunDecl { cmd: "y".into(), ..cmd("x") },
        CmdRunDecl { cwd: Some("sub".into()), ..cmd("x") },
        CmdRunDecl { on: Some("host".into()), ..cmd("x") },
        CmdRunDecl { stream: Some(true), ..cmd("x") },
        CmdRunDecl { every: Some(JsNumber(30.0)), ..cmd("x") },
        CmdRunDecl { always: Some(true), ..cmd("x") },
        CmdRunDecl { timeout: Some(JsNumber(20.0)), ..cmd("x") },
        CmdRunDecl { once: Some(true), ..cmd("x") },
        CmdRunDecl { then: Some("cat".into()), ..cmd("x") },
        CmdRunDecl { files: Some(IndexMap::from([("a.sh".to_string(), "1".to_string())])), ..cmd("x") },
        CmdRunDecl { scripts: Some(IndexMap::from([("a.sh".to_string(), "1".to_string())])), ..cmd("x") },
        CmdRunDecl { args: Some(vec![json!("$x")]), ..cmd("x") },
    ];
    let keys: Vec<String> = moved.iter().map(run_key).collect();
    for k in &keys {
        assert_ne!(*k, base);
    }
    let mut unique = keys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), keys.len());
}

// runs_parts Behaviour 7: stream, always and once count only when true
#[test]
fn run_key_counts_stream_always_and_once_only_when_true() {
    let base = run_key(&cmd("x"));
    assert_eq!(run_key(&CmdRunDecl { stream: Some(false), always: Some(false), once: Some(false), ..cmd("x") }), base);
}

// runs_parts Edge case: oracle asText
#[test]
fn as_text_writes_each_kind_of_value() {
    assert_eq!(as_text(&json!(null)), "");
    assert_eq!(as_text(&json!("s")), "s");
    assert_eq!(as_text(&json!(1.5)), "1.5");
    assert_eq!(as_text(&json!(true)), "true");
    assert_eq!(as_text(&json!([1, "a"])), r#"[1,"a"]"#);
    assert_eq!(as_text(&json!({"b": 1, "a": 2})), r#"{"b":1,"a":2}"#);
    assert_eq!(as_text(&json!(1e21)), "1e+21");
    assert_eq!(as_text(&json!(-0.0)), "0");
}

// runs_parts Behaviour 8: a bool, a whole number and an empty string
#[test]
fn as_text_writes_false_whole_numbers_and_the_empty_string() {
    assert_eq!(as_text(&json!(false)), "false");
    assert_eq!(as_text(&json!(5)), "5");
    assert_eq!(as_text(&json!(20.0)), "20");
    assert_eq!(as_text(&json!("")), "");
}

// runs_parts Behaviour 9: every string through the function, keys untouched
#[test]
fn map_strings_passes_every_string_through_and_leaves_keys_and_the_rest() {
    let v = json!({"a": "x", "k": ["y", 1, null, true, {"z": "w"}], "n": 2.5});
    let got = map_strings(&v, &|s| format!("<{s}>"));
    assert_eq!(serde_json::to_string(&got).unwrap(), r#"{"a":"<x>","k":["<y>",1,null,true,{"z":"<w>"}],"n":2.5}"#);
    assert_eq!(map_strings(&json!("top"), &|s| s.to_uppercase()), json!("TOP"));
    assert_eq!(map_strings(&json!(7), &|s| s.to_uppercase()), json!(7));
}

// runs_parts Edge case: oracle timeoutOf
#[test]
fn timeout_of_clamps_to_one_and_600_and_keeps_fractions() {
    assert_eq!(timeout_of(&cmd("x")), 60.0);
    assert_eq!(timeout_of(&CmdRunDecl { timeout: Some(JsNumber(0.0)), ..cmd("x") }), 1.0);
    assert_eq!(timeout_of(&CmdRunDecl { timeout: Some(JsNumber(0.5)), ..cmd("x") }), 1.0);
    assert_eq!(timeout_of(&CmdRunDecl { timeout: Some(JsNumber(601.0)), ..cmd("x") }), 600.0);
    assert_eq!(timeout_of(&CmdRunDecl { timeout: Some(JsNumber(-5.0)), ..cmd("x") }), 1.0);
    assert_eq!(timeout_of(&CmdRunDecl { timeout: Some(JsNumber(2.5)), ..cmd("x") }), 2.5);
}

// runs_parts Behaviour 11: env name `^[A-Za-z_][A-Za-z0-9_]*$`
#[test]
fn is_env_name_follows_the_pattern() {
    for ok in ["A", "_", "_a1", "ID", "a_B9", "PATH"] {
        assert!(is_env_name(ok), "{ok}");
    }
    for bad in ["", "1a", "a-b", "a b", "a.b", "\u{e9}", "a\n", "$A", "A=1"] {
        assert!(!is_env_name(bad), "{bad:?}");
    }
}

// runs_parts slate-runs.ts:458: the sentence a failed files write gives
#[test]
fn files_unwritten_names_the_cause() {
    assert_eq!(files_unwritten("disk full"), "the slate's files were not written: disk full");
}

// runs_parts Behaviour 11: the constants
#[test]
fn the_constants_have_the_ts_text_and_values() {
    assert!(is_env_name("A_1"));
    assert_eq!(NO_FOLDER, "this host knows no folder for the thread, so the command did not start");
    assert_eq!(HELD_APPROVAL, "needs your approval");
    assert_eq!(HELD_BUSY, "4 runs are already running");
    assert_eq!(HELD_BUDGET, "started 12 times in a minute; press to run it again");
    assert_eq!(HELD_PRESSED, "pressed 12 times in a minute; it can start again in a minute");
    assert_eq!(HELD_ASLEEP, "the box was asleep, so this tick did not wake it; press to run it now");
    assert_eq!(BACKED_OFF, "five starts in a row failed, so its timer starts it once every 5 minutes until one succeeds");
    assert_eq!(SECRET_DOTS, "\u{2022}\u{2022}\u{2022}\u{2022}");
    assert_eq!(STARTS_PER_MINUTE, 12);
    assert_eq!(RUNNING_MAX, 4);
    assert_eq!(LINES_KEPT, 500);
    assert_eq!(TIMER_FLOOR_S, 10);
    assert_eq!(BACKOFF_AFTER, 5);
    assert_eq!(BACKOFF_MS, 300_000);
    assert_eq!(CLOSE_GRACE_MS, 1_000);
    assert_eq!(DEFAULT_TIMEOUT_S, 60);
}
