// SPDX-License-Identifier: AGPL-3.0-only
//! Unit helpers: spec/helpers.md. No TS unit tests exist for these four leaves; each test is one Behaviour or Edge
//! case of the spec, with the values of `oracle/helpers.out`.
mod support;

use serde_json::{json, Value};
use support::ManualClock;
use wsp_slates::pending::slate::{PathSeg, SlateDoc, SlateProblem};
use wsp_slates::slate_message::{cut_at, defanged, longest, Longest};
use wsp_slates::slate_read::{is_run_record, misread, run_shown, READ_OUTPUT_CHARS};
use wsp_slates::slate_refused::invalid;
use wsp_slates::slate_writes::{write_budget, WriteBudget};
use wsp_words::exit::Kind;

fn show(v: &Value) -> String {
    serde_json::to_string(v).unwrap()
}

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_string())
}

fn problem(code: &str, name: &str, message: &str) -> SlateProblem {
    SlateProblem { code: code.into(), name: name.into(), message: message.into(), ..Default::default() }
}

fn doc() -> SlateDoc {
    serde_json::from_value(json!({
        "schema": 2,
        "root": "p",
        "values": { "v": { "start": 0 } },
        "derived": { "d": "$v" },
        "runs": { "r": { "kind": "cmd", "cmd": "true" } },
        "reactions": [],
        "pieces": { "p": { "type": "column" } }
    }))
    .unwrap()
}

// helpers Behaviour 1: slate-read.ts:6-7
#[test]
fn is_run_record_wants_an_object_with_a_string_state_and_a_number_runs() {
    assert!(is_run_record(Some(&json!({"state": "done", "runs": 1}))));
    assert!(is_run_record(Some(&json!({"state": "", "runs": 0, "out": "x"}))));
    assert!(!is_run_record(Some(&json!({"state": "done"}))));
    assert!(!is_run_record(Some(&json!({"runs": 1}))));
    assert!(!is_run_record(Some(&json!({"state": 1, "runs": 1}))));
    assert!(!is_run_record(Some(&json!({"state": "x", "runs": "1"}))));
    assert!(!is_run_record(Some(&json!(null))));
    assert!(!is_run_record(Some(&json!([{"state": "done", "runs": 1}]))));
    assert!(!is_run_record(Some(&json!("done"))));
    assert!(!is_run_record(None));
}

// helpers Behaviour 2: READ_OUTPUT_CHARS counts UTF-16 units, 4,000 of them
#[test]
fn read_output_chars_is_4000_utf16_units() {
    assert_eq!(READ_OUTPUT_CHARS, 4_000);
    let whole = "😀".repeat(2_000);
    assert_eq!(run_shown(Some(&json!({"state": "done", "out": whole, "runs": 1})), "r", false).get("out"), Some(&json!(whole)));
    let over = "😀".repeat(2_001);
    let shown = run_shown(Some(&json!({"state": "done", "out": over, "runs": 1})), "r", false);
    assert_eq!(shown["out"], json!(format!("{}… (4002 characters; values [\"$r.out\"] reads it whole)", "😀".repeat(2_000))));
}

// helpers Behaviour 3 / Edge case: oracle runShown.named
#[test]
fn run_shown_named_keeps_only_the_summary_keys_in_the_records_order() {
    let rec = json!({"state": "done", "why": "w", "exit": 0, "out": "x", "err": "e", "json": {}, "ms": 3, "runs": 1, "stale": true, "endedAt": 9, "startedAt": 2, "lines": ["a"]});
    let shown = run_shown(Some(&rec), "r", true);
    assert_eq!(show(&shown), r#"{"state":"done","why":"w","exit":0,"ms":3,"runs":1,"stale":true,"endedAt":9}"#);
}

// helpers Behaviour 3: the key list names refreshing, in the record's own order
#[test]
fn run_shown_named_keeps_refreshing_and_follows_the_records_key_order() {
    let rec = json!({"refreshing": true, "runs": 2, "startedAt": 5, "state": "running", "json": 1});
    let shown = run_shown(Some(&rec), "r", true);
    assert_eq!(show(&shown), r#"{"refreshing":true,"runs":2,"state":"running"}"#);
}

// helpers Behaviour 3 / Edge case: oracle runShown.json_drops_out
#[test]
fn run_shown_leaves_out_the_out_when_the_record_has_json() {
    let shown = run_shown(Some(&json!({"state": "done", "out": "{}", "json": {}, "runs": 1})), "r", false);
    assert_eq!(show(&shown), r#"{"state":"done","json":{},"runs":1}"#);
}

// helpers Behaviour 3: a json of null counts as the key being there
#[test]
fn run_shown_counts_a_json_of_null_as_json() {
    let shown = run_shown(Some(&json!({"state": "done", "out": "null", "json": null, "runs": 1})), "r", false);
    assert_eq!(show(&shown), r#"{"state":"done","json":null,"runs":1}"#);
}

// helpers Behaviour 3: without json the out stays, every key in order
#[test]
fn run_shown_keeps_every_key_in_order_when_there_is_no_json() {
    let rec = json!({"state": "done", "exit": 0, "out": "hi", "err": "", "startedAt": 7, "runs": 1, "extra": [1]});
    assert_eq!(show(&run_shown(Some(&rec), "r", false)), show(&rec));
}

// helpers Behaviour 3 / Edge case: oracle runShown.cut
#[test]
fn run_shown_cuts_an_out_of_4001_and_says_where_the_rest_is() {
    let shown = run_shown(Some(&json!({"state": "done", "out": "y".repeat(4001), "runs": 1})), "raw", false);
    assert_eq!(shown["out"], json!(format!("{}… (4001 characters; values [\"$raw.out\"] reads it whole)", "y".repeat(4000))));
}

// helpers Edge case: oracle runShown.cut4000
#[test]
fn run_shown_does_not_cut_an_out_of_exactly_4000() {
    let shown = run_shown(Some(&json!({"state": "done", "out": "y".repeat(4000), "runs": 1})), "raw", false);
    assert_eq!(shown["out"], json!("y".repeat(4000)));
}

// helpers Behaviour 3: an err is cut the same way, named by its own field
#[test]
fn run_shown_cuts_a_long_err_naming_the_err_field() {
    let shown = run_shown(Some(&json!({"state": "failed", "err": "e".repeat(4005), "json": 1, "runs": 1})), "r", false);
    assert_eq!(shown["err"], json!(format!("{}… (4005 characters; values [\"$r.err\"] reads it whole)", "e".repeat(4000))));
}

// helpers Behaviour 3 / Edge case: oracle runShown.lines
#[test]
fn run_shown_keeps_the_last_50_lines() {
    let lines: Vec<String> = (0..60).map(|i| i.to_string()).collect();
    let shown = run_shown(Some(&json!({"state": "done", "lines": lines, "runs": 1})), "r", false);
    let kept: Vec<String> = (10..60).map(|i| i.to_string()).collect();
    assert_eq!(shown["lines"], json!(kept));
    let few = run_shown(Some(&json!({"state": "done", "lines": ["a", "b"], "runs": 1})), "r", false);
    assert_eq!(few["lines"], json!(["a", "b"]));
}

// helpers Behaviour 3: anything else is kept as is, a non-string out and a non-array lines too
#[test]
fn run_shown_keeps_a_non_string_out_and_non_array_lines_as_they_are() {
    let rec = json!({"state": "done", "out": 5, "err": null, "lines": "not a list", "runs": 1});
    assert_eq!(show(&run_shown(Some(&rec), "r", false)), show(&rec));
}

// helpers Edge case: oracle runShown.notrecord, runShown.runsString
#[test]
fn run_shown_answers_a_non_record_whole() {
    assert_eq!(show(&run_shown(Some(&json!({"state": "done"})), "r", false)), r#"{"state":"done"}"#);
    assert_eq!(show(&run_shown(Some(&json!({"state": "x", "runs": "1"})), "r", true)), r#"{"state":"x","runs":"1"}"#);
    assert_eq!(show(&run_shown(Some(&json!([1, 2])), "r", false)), "[1,2]");
    assert_eq!(show(&run_shown(Some(&json!("text")), "r", true)), r#""text""#);
}

// helpers Edge case: oracle runShown.undefined
#[test]
fn run_shown_of_nothing_is_null() {
    assert_eq!(run_shown(None, "r", false), Value::Null);
    assert_eq!(run_shown(None, "r", true), Value::Null);
}

// helpers Behaviour 4: a run of the document
#[test]
fn misread_names_a_run_read_without_its_dollar() {
    let d = doc();
    let problems = misread(Some(&d), "r.out");
    assert_eq!(
        serde_json::to_string(&problems).unwrap(),
        r#"[{"code":"X401","name":"path-unknown","message":"r.out is not a source: $r is a run, read as $r.out; its record is also under runs.$r","fix":"$r.out"}]"#
    );
}

// helpers Behaviour 4: the root is the text before the first `.` or `[`
#[test]
fn misread_cuts_the_root_at_a_bracket_too() {
    let d = doc();
    let problems = misread(Some(&d), "r[0]");
    assert_eq!(
        serde_json::to_string(&problems).unwrap(),
        r#"[{"code":"X401","name":"path-unknown","message":"r[0] is not a source: $r is a run, read as $r[0]; its record is also under runs.$r","fix":"$r[0]"}]"#
    );
    let bare = misread(Some(&d), "r");
    assert_eq!(
        serde_json::to_string(&bare).unwrap(),
        r#"[{"code":"X401","name":"path-unknown","message":"r is not a source: $r is a run, read as $r; its record is also under runs.$r","fix":"$r"}]"#
    );
}

// helpers Behaviour 4: a value of the document
#[test]
fn misread_names_a_value_read_without_its_dollar() {
    let d = doc();
    let problems = misread(Some(&d), "v.a[1]");
    assert_eq!(
        serde_json::to_string(&problems).unwrap(),
        r#"[{"code":"X401","name":"path-unknown","message":"v.a[1] is not a source: $v is a value, read as $v.a[1]","fix":"$v.a[1]"}]"#
    );
}

// helpers Behaviour 4: a derived value reads as a value
#[test]
fn misread_names_a_derived_value_as_a_value() {
    let d = doc();
    let problems = misread(Some(&d), "d");
    assert_eq!(
        serde_json::to_string(&problems).unwrap(),
        r#"[{"code":"X401","name":"path-unknown","message":"d is not a source: $d is a value, read as $d","fix":"$d"}]"#
    );
}

// helpers Behaviour 4: a run is checked before a value
#[test]
fn misread_checks_a_run_before_a_value() {
    let mut d = doc();
    d.values.insert("r".into(), serde_json::from_value(json!({"start": 1})).unwrap());
    let problems = misread(Some(&d), "r.x");
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].message, "r.x is not a source: $r is a run, read as $r.x; its record is also under runs.$r");
}

// helpers Behaviour 4: a root that is no source and no name of the slate
#[test]
fn misread_refuses_a_root_that_is_no_source_and_not_the_slates() {
    let d = doc();
    let problems = misread(Some(&d), "nope.x");
    assert_eq!(
        serde_json::to_string(&problems).unwrap(),
        r#"[{"code":"X401","name":"path-unknown","message":"nope is not a source and not this slate's; a slate's own names start with $"}]"#
    );
    assert_eq!(problems[0].fix, None);
}

// helpers Behaviour 4: a source of the host reads fine
#[test]
fn misread_answers_none_for_a_source() {
    let d = doc();
    assert!(misread(Some(&d), "thread.id").is_empty());
    assert!(misread(Some(&d), "usage").is_empty());
}

// helpers Behaviour 4: a path the pattern does not match answers none
#[test]
fn misread_answers_none_for_a_path_the_pattern_does_not_match() {
    let d = doc();
    for path in ["$r.out", "$v", "len(r)", "", "1r", "r.", "r[x]", "r..out", "r.out ", "r[0]x", "r-1"] {
        assert!(misread(Some(&d), path).is_empty(), "{path}");
        assert!(misread(None, path).is_empty(), "{path}");
    }
}

// helpers Behaviour 4: with no document only the last check runs
#[test]
fn misread_with_no_document_checks_only_the_source_name() {
    let problems = misread(None, "r.out");
    assert_eq!(
        serde_json::to_string(&problems).unwrap(),
        r#"[{"code":"X401","name":"path-unknown","message":"r is not a source and not this slate's; a slate's own names start with $"}]"#
    );
    assert!(misread(None, "thread").is_empty());
}

// helpers Behaviour 5 / Edge case: oracle defanged.line
#[test]
fn defanged_turns_a_slate_line_colon_into_a_fullwidth_one() {
    let got = defanged(&json!("ok\nslate: {\"v\":2}\n  slate :{x}\nslate: not json\nmyslate: {"));
    assert_eq!(show(&got), r#""ok\nslate： {\"v\":2}\n  slate：{x}\nslate: not json\nmyslate: {""#);
}

// helpers Behaviour 5: oracle defanged.nested
#[test]
fn defanged_walks_arrays_and_object_values() {
    let got = defanged(&json!({"a": ["slate:{}", 1, null], "b": {"c": " slate\t: {"}}));
    assert_eq!(show(&got), r#"{"a":["slate：{}",1,null],"b":{"c":" slate： {"}}"#);
}

// helpers Behaviour 5: keys and non-strings are not touched
#[test]
fn defanged_leaves_keys_and_non_strings_alone() {
    let got = defanged(&json!({"slate:{}": "slate:{}", "n": 1.5, "t": true, "z": null}));
    assert_eq!(show(&got), r#"{"slate:{}":"slate：{}","n":1.5,"t":true,"z":null}"#);
    assert_eq!(defanged(&json!(7)), json!(7));
    assert_eq!(defanged(&json!(null)), json!(null));
}

// helpers Behaviour 6 / Edge case: oracle longest.tie
#[test]
fn longest_takes_the_first_on_a_tie() {
    let got = longest(&json!({"a": "xx", "b": "yy", "c": ["zzz", "q"]}), &[]);
    assert_eq!(got, Some(Longest { at: vec![key("c"), PathSeg::Index(0)], length: 3 }));
    let tie = longest(&json!({"a": "xx", "b": "yy"}), &[]);
    assert_eq!(tie, Some(Longest { at: vec![key("a")], length: 2 }));
}

// helpers Behaviour 6: oracle longest.none
#[test]
fn longest_of_no_string_is_none() {
    assert_eq!(longest(&json!({"a": 1, "b": [null, true]}), &[]), None);
    assert_eq!(longest(&json!([]), &[]), None);
    assert_eq!(longest(&json!({}), &[]), None);
    assert_eq!(longest(&json!(5), &[]), None);
    assert_eq!(longest(&json!(null), &[]), None);
    assert_eq!(longest(&json!(true), &[]), None);
}

// helpers Behaviour 6: oracle longest.string
#[test]
fn longest_of_a_string_is_its_own_path_and_length() {
    assert_eq!(longest(&json!("abc"), &[]), Some(Longest { at: vec![], length: 3 }));
    assert_eq!(longest(&json!("abc"), &[key("p"), PathSeg::Index(2)]), Some(Longest { at: vec![key("p"), PathSeg::Index(2)], length: 3 }));
}

// helpers Behaviour 6: lengths are UTF-16 units
#[test]
fn longest_counts_utf16_units() {
    let got = longest(&json!({"a": "abc", "b": "a😀"}), &[]);
    assert_eq!(got, Some(Longest { at: vec![key("a")], length: 3 }));
    let wide = longest(&json!({"a": "abcd", "b": "😀😀"}), &[]);
    assert_eq!(wide, Some(Longest { at: vec![key("a")], length: 4 }));
    let wider = longest(&json!(["😀😀😀"]), &[]);
    assert_eq!(wider, Some(Longest { at: vec![PathSeg::Index(0)], length: 6 }));
}

// helpers Behaviour 7: oracle cutAt.obj
#[test]
fn cut_at_cuts_the_string_at_the_path_and_keeps_the_rest() {
    let got = cut_at(&json!({"a": "abcdefgh", "b": ["x", "yyyyyy"]}), &[key("b"), PathSeg::Index(1)], 2);
    assert_eq!(show(&got), r#"{"a":"abcdefgh","b":["x","yy (cut)"]}"#);
}

// helpers Behaviour 7: oracle cutAt.root
#[test]
fn cut_at_the_root_cuts_a_string() {
    assert_eq!(show(&cut_at(&json!("abcdef"), &[], 3)), r#""abc (cut)""#);
}

// helpers Edge case: oracle cutAt.keep0
#[test]
fn cut_at_keep_zero_is_just_the_marker() {
    assert_eq!(show(&cut_at(&json!("abcdef"), &[], 0)), r#"" (cut)""#);
}

// helpers Edge case: oracle cutAt.nonstring
#[test]
fn cut_at_a_non_string_is_unchanged() {
    assert_eq!(show(&cut_at(&json!(5), &[], 3)), "5");
    assert_eq!(show(&cut_at(&json!({"a": 5, "b": "xyz"}), &[key("a")], 1)), r#"{"a":5,"b":"xyz"}"#);
    assert_eq!(show(&cut_at(&json!(null), &[], 1)), "null");
}

// helpers Behaviour 7: in an array only the element at the head is walked
#[test]
fn cut_at_walks_only_the_element_at_the_head_of_an_array() {
    let got = cut_at(&json!(["aaaa", "bbbb", ["cccc"]]), &[PathSeg::Index(1)], 1);
    assert_eq!(show(&got), r#"["aaaa","b (cut)",["cccc"]]"#);
    let deep = cut_at(&json!(["aaaa", "bbbb", ["cccc", "dddd"]]), &[PathSeg::Index(2), PathSeg::Index(0)], 2);
    assert_eq!(show(&deep), r#"["aaaa","bbbb",["cc (cut)","dddd"]]"#);
}

// helpers Behaviour 7: in an object the key keeps its place
#[test]
fn cut_at_keeps_an_objects_key_order() {
    let got = cut_at(&json!({"b": "abcd", "a": "x", "c": "yy"}), &[key("b")], 1);
    assert_eq!(show(&got), r#"{"b":"a (cut)","a":"x","c":"yy"}"#);
}

// helpers Behaviour 7: keep counts UTF-16 units
#[test]
fn cut_at_keep_counts_utf16_units() {
    assert_eq!(show(&cut_at(&json!("😀😀😀"), &[], 4)), r#""😀😀 (cut)""#);
}

fn budget(clock: &ManualClock) -> WriteBudget {
    write_budget(clock.clock())
}

fn spends(b: &WriteBudget, thread: &str, n: usize) -> Vec<Option<i64>> {
    (0..n).map(|_| b.spend(thread)).collect()
}

// helpers Behaviour 8 / Edge case: oracle budget.burst22
#[test]
fn a_burst_of_20_passes_and_the_21st_and_22nd_wait_one_second() {
    let clock = ManualClock::new(0);
    let b = budget(&clock);
    let mut want: Vec<Option<i64>> = vec![None; 20];
    want.extend([Some(1), Some(1)]);
    assert_eq!(spends(&b, "a", 22), want);
}

// helpers Edge case: oracle budget.after100ms
#[test]
fn half_a_token_after_100_ms_still_waits_one_second() {
    let clock = ManualClock::new(0);
    let b = budget(&clock);
    spends(&b, "a", 22);
    clock.advance(100);
    assert_eq!(b.spend("a"), Some(1));
}

// helpers Edge case: oracle budget.after200ms
#[test]
fn a_whole_token_is_back_after_200_ms() {
    let clock = ManualClock::new(0);
    let b = budget(&clock);
    spends(&b, "a", 22);
    clock.advance(100);
    assert_eq!(b.spend("a"), Some(1));
    clock.advance(100);
    assert_eq!(b.spend("a"), None);
    assert_eq!(b.spend("a"), Some(1));
}

// helpers Edge case: oracle budget.10s
#[test]
fn ten_seconds_later_three_writes_pass() {
    let clock = ManualClock::new(0);
    let b = budget(&clock);
    spends(&b, "a", 22);
    clock.advance(10_000);
    assert_eq!(spends(&b, "a", 3), vec![None, None, None]);
}

// helpers Behaviour 8: the bucket refills no further than the burst
#[test]
fn the_bucket_refills_no_further_than_20() {
    let clock = ManualClock::new(0);
    let b = budget(&clock);
    spends(&b, "a", 20);
    clock.advance(100_000);
    let mut want: Vec<Option<i64>> = vec![None; 20];
    want.push(Some(1));
    assert_eq!(spends(&b, "a", 21), want);
}

// helpers Behaviour 8: a refused spend takes nothing
#[test]
fn a_refused_spend_takes_nothing() {
    let clock = ManualClock::new(0);
    let b = budget(&clock);
    spends(&b, "a", 20);
    assert_eq!(spends(&b, "a", 5), vec![Some(1); 5]);
    clock.advance(1_000);
    assert_eq!(spends(&b, "a", 6), vec![None, None, None, None, None, Some(1)]);
}

// helpers Behaviour 8: every thread has its own bucket
#[test]
fn each_thread_has_its_own_bucket() {
    let clock = ManualClock::new(0);
    let b = budget(&clock);
    spends(&b, "a", 20);
    assert_eq!(b.spend("a"), Some(1));
    assert_eq!(b.spend("b"), None);
}

// helpers Behaviour 8: oracle budget.hour.600th_and_601st, budget.hour.at600s, budget.hour.at3600s
#[test]
fn the_601st_write_of_an_hour_waits_and_the_oldest_ages_out_at_3600_seconds() {
    let clock = ManualClock::new(0);
    let b = budget(&clock);
    let mut res: Vec<Option<i64>> = Vec::new();
    for i in 0..601 {
        if i > 0 {
            clock.advance(1_000);
        }
        res.push(b.spend("h"));
    }
    assert_eq!(&res[598..], &[None, None, Some(3_000)]);
    assert_eq!(b.spend("h"), Some(3_000));
    clock.advance(3_000_000);
    assert_eq!(b.spend("h"), None);
}

// helpers Behaviour 8: delete answers whether a bucket was there
#[test]
fn delete_answers_whether_a_bucket_was_there_and_the_thread_starts_full_again() {
    let clock = ManualClock::new(0);
    let b = budget(&clock);
    assert!(!b.delete("a"));
    spends(&b, "a", 21);
    assert!(b.delete("a"));
    assert!(!b.delete("a"));
    assert_eq!(spends(&b, "a", 20), vec![None; 20]);
    assert_eq!(b.spend("a"), Some(1));
}

fn errs(n: usize) -> Vec<SlateProblem> {
    (0..n).map(|i| problem(&format!("C{i}"), "n", "m")).collect()
}

fn json_of(p: &[SlateProblem]) -> Vec<Value> {
    p.iter().map(|x| serde_json::to_value(x).unwrap()).collect()
}

// helpers Behaviour 9 / Edge case: oracle invalid.one
#[test]
fn invalid_with_one_error_says_it_alone_with_its_place_and_fix() {
    let one = SlateProblem {
        code: "S501".into(),
        name: "name-undeclared".into(),
        message: "$nope is not declared".into(),
        line: Some(1),
        piece: Some("text-1".into()),
        prop: Some("value".into()),
        fix: Some("$no".into()),
        ..Default::default()
    };
    let e = invalid(std::slice::from_ref(&one), &[]);
    assert_eq!(e.message, "slate refused: 1 error: line 1, text-1.value: S501 name-undeclared $nope is not declared. Did you mean $no?");
    assert_eq!(e.kind, Some(Kind::Invalid));
    assert_eq!(e.errors, Some(json_of(&[one])));
    assert_eq!(e.warnings, Some(vec![]));
    assert_eq!(e.fix, None);
}

// helpers Behaviour 9 / Edge case: oracle invalid.two
#[test]
fn invalid_with_two_errors_numbers_them_and_keeps_the_warnings() {
    let a = problem("S501", "a", "m1");
    let b = SlateProblem { piece: Some("meter-1".into()), ..problem("T306", "b", "m2") };
    let w = problem("W1", "w", "x");
    let e = invalid(&[a.clone(), b.clone()], std::slice::from_ref(&w));
    assert_eq!(e.message, "slate refused: 2 errors: 1) S501 a m1 2) meter-1: T306 b m2");
    assert_eq!(e.kind, Some(Kind::Invalid));
    assert_eq!(e.errors, Some(json_of(&[a, b])));
    assert_eq!(e.warnings, Some(json_of(&[w])));
    assert_eq!(e.fix, None);
}

// helpers Edge case: oracle invalid.zero
#[test]
fn invalid_with_no_errors_keeps_the_trailing_space() {
    let e = invalid(&[], &[]);
    assert_eq!(e.message, "slate refused: 0 errors: ");
    assert_eq!(e.kind, Some(Kind::Invalid));
    assert_eq!(e.errors, Some(vec![]));
    assert_eq!(e.warnings, Some(vec![]));
    assert_eq!(e.fix, None);
}

// helpers Behaviour 9 / Edge case: oracle invalid.22
#[test]
fn invalid_lists_the_first_20_errors_and_counts_the_rest() {
    let all = errs(22);
    let e = invalid(&all, &[]);
    assert!(e.message.starts_with("slate refused: 22 errors: 1) C0 n m 2) C1 n m 3) C2 n m"), "{}", e.message);
    assert!(e.message.ends_with(" n m 19) C18 n m 20) C19 n m And 2 more."), "{}", e.message);
    assert!(!e.message.contains("21)"));
    assert_eq!(e.errors, Some(json_of(&all[..20])));
}

// helpers Behaviour 9: exactly 20 errors are all listed and nothing is added
#[test]
fn invalid_with_exactly_20_errors_adds_no_more_line() {
    let all = errs(20);
    let e = invalid(&all, &[]);
    assert!(e.message.starts_with("slate refused: 20 errors: 1) C0 n m "), "{}", e.message);
    assert!(e.message.ends_with("20) C19 n m"), "{}", e.message);
    assert_eq!(e.errors, Some(json_of(&all)));
    let more = invalid(&errs(21), &[]);
    assert!(more.message.ends_with("20) C19 n m And 1 more."), "{}", more.message);
}

// helpers Behaviour 9: said is `[line L][, piece[.prop]]: ` with only the parts present
#[test]
fn invalid_said_joins_only_the_parts_that_are_present() {
    let line_only = SlateProblem { line: Some(3), ..problem("S1", "n", "m") };
    assert_eq!(invalid(&[line_only], &[]).message, "slate refused: 1 error: line 3: S1 n m");
    let piece_prop = SlateProblem { piece: Some("text-1".into()), prop: Some("value".into()), ..problem("S1", "n", "m") };
    assert_eq!(invalid(&[piece_prop], &[]).message, "slate refused: 1 error: text-1.value: S1 n m");
    let prop_alone = SlateProblem { prop: Some("value".into()), ..problem("S1", "n", "m") };
    assert_eq!(invalid(&[prop_alone], &[]).message, "slate refused: 1 error: S1 n m");
    let line_piece = SlateProblem { line: Some(2), piece: Some("p".into()), ..problem("S1", "n", "m") };
    assert_eq!(invalid(&[line_piece], &[]).message, "slate refused: 1 error: line 2, p: S1 n m");
    let fix_only = SlateProblem { fix: Some("$x".into()), ..problem("S1", "n", "m") };
    assert_eq!(invalid(&[fix_only], &[]).message, "slate refused: 1 error: S1 n m. Did you mean $x?");
}
