// SPDX-License-Identifier: AGPL-3.0-only
//! The small leaf files of the slate module: limits, paths, problems, steps, sources, chart, shape and image.
//! Expected values are the TS's answers at 65f6b89c7 (the TS tests' literals, or the output of the TS run under node
//! on the same input).

use serde_json::{json, Value};
use wsp_slates::pending::slate::*;

fn vals(v: Value) -> SlateValues {
    v.as_object().expect("an object").iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_string())
}

fn problem(code: &str, name: &str, message: &str) -> SlateProblem {
    SlateProblem { code: code.into(), name: name.into(), message: message.into(), ..Default::default() }
}

fn names(v: &SlateValues) -> Vec<&str> {
    v.keys().map(String::as_str).collect()
}

// limits.ts

#[test]
fn slate_bytes_counts_utf8_bytes() {
    assert_eq!(slate_bytes("abc"), 3);
    assert_eq!(slate_bytes("é"), 2);
    assert_eq!(slate_bytes("😀"), 4);
    assert_eq!(slate_bytes(""), 0);
}

#[test]
fn slate_limits_hold_the_ts_values() {
    assert_eq!(SLATE_LIMITS.document_bytes, 65536);
    assert_eq!(SLATE_LIMITS.values_bytes, 262144);
    assert_eq!(SLATE_LIMITS.pieces, 300);
    assert_eq!(SLATE_LIMITS.list_items, 5000);
    assert_eq!(SLATE_LIMITS.images_per_list, 60);
    assert_eq!(SLATE_LIMITS.eval_steps, 20000);
    assert_eq!(SLATE_LIMITS.pipeline_visits, 100000);
    assert_eq!(SLATE_LIMITS.timeout_max_s, 600);
    assert_eq!(SLATE_LIMITS.catalog_entry_tokens, 200);
}

// types.ts functions

#[test]
fn run_idle_is_the_idle_record_in_ts_key_order() {
    assert_eq!(slate_run_idle().to_string(), r#"{"state":"idle","runs":0}"#);
}

#[test]
fn is_slate_binding_reads_one_bind_key_that_is_a_string() {
    assert!(is_slate_binding(&json!({"bind": "$x"})));
    assert!(!is_slate_binding(&json!({"bind": 1})));
    assert!(!is_slate_binding(&json!({"bind": "x", "a": 1})));
    assert!(!is_slate_binding(&json!([])));
    assert!(!is_slate_binding(&json!("bind")));
    assert!(!is_slate_binding(&json!({})));
}

// paths.ts

#[test]
fn own_path_parses_its_name_and_steps() {
    let path = parse_slate_own_path("$a.b[2].c").expect("an own path");
    assert_eq!(path.name, "a");
    assert_eq!(path.segs, vec![key("b"), PathSeg::Index(2), key("c")]);
    assert_eq!(parse_slate_own_path("$a").unwrap().segs, vec![]);
    assert_eq!(parse_slate_own_path("$_x9").unwrap().name, "_x9");
    assert_eq!(parse_slate_own_path("$a[-1]").unwrap().segs, vec![PathSeg::Index(-1)]);
    assert_eq!(parse_slate_own_path("$a[-0]").unwrap().segs, vec![PathSeg::Index(0)]);
}

#[test]
fn own_path_refuses_what_is_not_an_own_path_or_steps() {
    for text in ["$", "$9x", "a.b", "$a..b", "$a.b.", "$a[]", "$a.1", "$a b", "$a[1]x", "$a[-]", "  $a"] {
        assert_eq!(parse_slate_own_path(text), None, "{text}");
    }
}

#[test]
fn own_name_is_trimmed_and_only_a_bare_name() {
    assert_eq!(slate_own_name("  $a  ").as_deref(), Some("a"));
    assert_eq!(slate_own_name(" $x ").as_deref(), Some("x"));
    assert_eq!(slate_own_name("$a.b"), None);
    assert_eq!(slate_own_name("$a[0]"), None);
    assert_eq!(slate_own_name(""), None);
}

#[test]
fn segments_read_fields_and_signed_indexes() {
    assert_eq!(slate_segments(".a[0].b[-2]"), Some(vec![key("a"), PathSeg::Index(0), key("b"), PathSeg::Index(-2)]));
    assert_eq!(slate_segments(".a[x]"), None);
    assert_eq!(slate_segments(""), Some(vec![]));
}

#[test]
fn get_reads_negative_indexes_and_answers_none_for_a_missing_step() {
    let values = vals(json!({"a": [1, {"b": "x"}, 3], "c": {"d": null}, "e": null, "z": "s"}));
    assert_eq!(get_slate_value(&values, "$a[0]"), Some(json!(1)));
    assert_eq!(get_slate_value(&values, "$a[-1]"), Some(json!(3)));
    assert_eq!(get_slate_value(&values, "$a[-3]"), Some(json!(1)));
    assert_eq!(get_slate_value(&values, "$a[-4]"), None);
    assert_eq!(get_slate_value(&values, "$a[3]"), None);
    assert_eq!(get_slate_value(&values, "$a[1].b"), Some(json!("x")));
    assert_eq!(get_slate_value(&values, "$c.d"), Some(Value::Null));
    assert_eq!(get_slate_value(&values, "$c.x"), None);
    assert_eq!(get_slate_value(&values, "$c.d.x"), None);
    assert_eq!(get_slate_value(&values, "$e"), Some(Value::Null));
    assert_eq!(get_slate_value(&values, "$e.x"), None);
    assert_eq!(get_slate_value(&values, "$z[0]"), None);
    assert_eq!(get_slate_value(&values, "$q"), None);
    assert_eq!(get_slate_value(&values, "$a.length"), None);
    assert_eq!(get_slate_value(&values, "$a.constructor"), None);
    assert_eq!(get_slate_value(&values, "$c.toString"), None);
}

#[test]
fn step_reads_one_child_or_none() {
    assert_eq!(slate_step(Some(json!([1, 2, 3])), &PathSeg::Index(-1)), Some(json!(3)));
    assert_eq!(slate_step(Some(json!({"a": 1})), &key("b")), None);
    assert_eq!(slate_step(Some(json!({"a": 1})), &PathSeg::Index(0)), None);
    assert_eq!(slate_step(Some(json!("abc")), &PathSeg::Index(0)), None);
    assert_eq!(slate_step(Some(json!({"constructor": 1})), &key("constructor")), None);
    assert_eq!(slate_step(None, &key("a")), None);
}

#[test]
fn set_pads_a_list_with_null_and_makes_missing_records() {
    let values = vals(json!({"x": [], "o": {"k": 1}, "n": 5}));
    assert_eq!(set_slate_value(&values, "$x[2]", json!(1)), Some(vals(json!({"x": [null, null, 1], "o": {"k": 1}, "n": 5}))));
    assert_eq!(set_slate_value(&values, "$x[0][1]", json!(1)), Some(vals(json!({"x": [[null, 1]], "o": {"k": 1}, "n": 5}))));
    assert_eq!(set_slate_value(&values, "$o.k", json!(2)), Some(vals(json!({"x": [], "o": {"k": 2}, "n": 5}))));
    assert_eq!(set_slate_value(&values, "$o.j.h", json!(3)), Some(vals(json!({"x": [], "o": {"k": 1, "j": {"h": 3}}, "n": 5}))));
    assert_eq!(
        set_slate_value(&values, "$new.q[1]", json!("v")),
        Some(vals(json!({"x": [], "o": {"k": 1}, "n": 5, "new": {"q": [null, "v"]}})))
    );
    assert_eq!(set_slate_value(&values, "$o", Value::Null), Some(vals(json!({"x": [], "o": null, "n": 5}))));
}

#[test]
fn set_refuses_a_negative_index_on_an_empty_list_and_an_index_past_the_cap() {
    let values = vals(json!({"x": []}));
    assert_eq!(set_slate_value(&values, "$x[-1]", json!(1)), None);
    assert_eq!(set_slate_value(&values, "$x[999999999]", json!(1)), None);
    assert_eq!(set_slate_value(&values, &format!("$x[{}]", SLATE_LIMITS.list_items), json!(1)), None);
    let last = set_slate_value(&values, "$x[4999]", json!(2)).expect("the last index the cap holds");
    assert_eq!(last["x"].as_array().map(Vec::len), Some(5000));
    assert_eq!(last["x"][4999], json!(2));
}

#[test]
fn set_walks_into_a_record_but_not_through_a_scalar_or_an_unsafe_name() {
    let values = vals(json!({"n": 5, "o": {"k": 1}}));
    assert_eq!(set_slate_value(&values, "$n.a", json!(1)), None);
    assert_eq!(set_slate_value(&values, "$o.__proto__", json!(1)), None);
    assert_eq!(set_slate_value(&values, "$__proto__", json!(1)), None);
    assert_eq!(set_slate_value(&values, "$constructor", json!(1)), None);
}

#[test]
fn set_names_a_prototype_word_as_a_value_but_not_a_path_through_it() {
    let values = vals(json!({"n": 5}));
    let set = set_slate_value(&values, "$toString", json!(7)).expect("a bare name is set");
    assert_eq!(names(&set), vec!["n", "toString"]);
    assert_eq!(set["toString"], json!(7));
    assert_eq!(set_slate_value(&values, "$toString.a", json!(7)), None);
}

#[test]
fn set_keeps_the_order_of_names_and_appends_a_new_one() {
    let values = vals(json!({"z": 1, "a": 2}));
    assert_eq!(names(&set_slate_value(&values, "$a", json!(3)).unwrap()), vec!["z", "a"]);
    assert_eq!(names(&set_slate_value(&values, "$b", json!(3)).unwrap()), vec!["z", "a", "b"]);
}

#[test]
fn set_puts_index_like_keys_of_a_record_first_as_js_does() {
    let values = vals(json!({"o": {"b": 1, "2": 5, "a": 2, "1": 4}}));
    let set = set_slate_value(&values, "$o.c", json!(9)).expect("a field is set");
    let keys: Vec<&str> = set["o"].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, vec!["1", "2", "b", "a", "c"]);
}

#[test]
fn equality_is_deep_over_json_and_an_absent_value_is_a_null() {
    assert!(slate_equal(Some(&json!(1)), Some(&json!(1.0))));
    assert!(slate_equal(None, None));
    assert!(slate_equal(None, Some(&Value::Null)));
    assert!(slate_equal(Some(&Value::Null), None));
    assert!(!slate_equal(None, Some(&json!(0))));
    assert!(!slate_equal(Some(&json!(0)), None));
    assert!(slate_equal(Some(&json!([1, {"a": 2}])), Some(&json!([1, {"a": 2}]))));
    assert!(slate_equal(Some(&json!({"a": 1, "b": 2})), Some(&json!({"b": 2, "a": 1}))));
    assert!(!slate_equal(Some(&json!({"a": 1})), Some(&json!({"a": 1, "b": null}))));
    assert!(!slate_equal(Some(&json!([1])), Some(&json!({"0": 1}))));
    assert!(!slate_equal(Some(&json!("1")), Some(&json!(1))));
    assert!(slate_equal(Some(&json!(0)), Some(&json!(-0.0))));
    assert!(!slate_equal(Some(&json!({})), Some(&json!([]))));
    assert!(!slate_equal(Some(&Value::Null), Some(&json!(0))));
    assert!(slate_equal(Some(&json!("x")), Some(&json!("x"))));
}

// problems.ts

#[test]
fn problem_takes_its_name_from_the_code_table() {
    assert_eq!(
        slate_problem("X401", "m", Some("$x")),
        SlateProblem {
            code: "X401".into(),
            name: "path-unknown".into(),
            message: "m".into(),
            fix: Some("$x".into()),
            ..Default::default()
        }
    );
    assert_eq!(slate_problem("R900", "m", None), problem("R900", "data-missing", "m"));
}

#[test]
fn code_table_holds_the_119_ts_codes() {
    assert_eq!(SLATE_CODES.len(), 119);
    assert_eq!(slate_problem("W021", "", None).name, "time-in-seconds");
    assert_eq!(slate_problem("K713", "", None).name, "description-missing");
}

#[test]
fn warnings_are_the_eighteen_ts_codes_that_never_block_a_write() {
    assert_eq!(SLATE_WARNINGS.len(), 18);
    assert!(SLATE_WARNINGS.contains(&"T311"));
    assert!(SLATE_WARNINGS.contains(&"W021"));
    assert!(!SLATE_WARNINGS.contains(&"R900"));
}

#[test]
fn nearest_takes_the_closest_option_within_its_edits_and_ties_to_the_first() {
    let opts = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<String>>();
    assert_eq!(slate_nearest("sttings", &opts(&["settings", "status"])).as_deref(), Some("settings"));
    assert_eq!(slate_nearest("abc", &opts(&["xyz"])), None);
    assert_eq!(slate_nearest("ab", &opts(&["ac", "abc"])).as_deref(), Some("ac"));
    assert_eq!(slate_nearest("servers", &opts(&["server", "serve"])).as_deref(), Some("server"));
    assert_eq!(slate_nearest("lstbl", &opts(&["list", "table"])), None);
    assert_eq!(slate_nearest("", &opts(&["a"])).as_deref(), Some("a"));
    assert_eq!(slate_nearest("Pieces", &opts(&["pieces"])).as_deref(), Some("pieces"));
    assert_eq!(slate_nearest("ta", &opts(&["table", "tab"])).as_deref(), Some("tab"));
    assert_eq!(slate_nearest("a", &[]), None);
    assert_eq!(slate_nearest("tbl", &opts(&["table", "tab", "text"])).as_deref(), Some("table"));
    assert_eq!(slate_nearest("trackbar", &opts(&["trackbars", "tracks"])).as_deref(), Some("trackbars"));
}

#[test]
fn or_list_joins_with_commas_and_a_final_or() {
    assert_eq!(or_list(&["a"]), "a");
    assert_eq!(or_list(&["a", "b"]), "a or b");
    assert_eq!(or_list(&["a", "b", "c"]), "a, b or c");
    assert_eq!(or_list::<&str>(&[]), "");
}

#[test]
fn unknown_piece_says_the_html_habit_or_the_nearest_piece() {
    let pieces: Vec<String> = ["table", "text", "chart", "heading", "column"].iter().map(|s| s.to_string()).collect();
    let p = slate_unknown_piece("p", &pieces);
    assert_eq!(p.message, r#""p" is not a piece; words go in <text>...</text>"#);
    assert_eq!(p.fix, None);
    let div = slate_unknown_piece("DIV", &pieces);
    assert_eq!(div.message, r#""DIV" is not a piece; a group is a <column>, a <row> or a <section>"#);
    let tabel = slate_unknown_piece("tabel", &pieces);
    assert_eq!(tabel.message, r#""tabel" is not a piece; did you mean table? slate_catalog lists them"#);
    assert_eq!(tabel.fix.as_deref(), Some("table"));
    let h9 = slate_unknown_piece("h9", &pieces);
    assert_eq!(h9.message, r#""h9" is not a piece slate_catalog lists them"#);
    assert_eq!(h9.fix, None);
}

#[test]
fn sentences_say_what_the_ts_says() {
    assert_eq!(
        slate_server_not_set_up("linear", "mac-mini", "claude", "Claude", "thr_1"),
        "linear is not set up on mac-mini: add it to Claude's MCP config there with wsp servers add linear thr_1 --agent claude --command \"<its command line>\" (or --url <its address>), then write the run again"
    );
    assert_eq!(
        slate_host_away("$deploy", "dev-box", "this Mac"),
        "$deploy runs on=\"host\", which is this Mac with the sign-ins there, and this thread runs on dev-box: drop on=\"host\" and it runs on dev-box"
    );
    assert_eq!(
        slate_description_missing("$inbox", "zoho", "zoho_list"),
        "$inbox calls zoho.zoho_list, which its server does not mark read-only, so it needs description=\"...\" saying in one plain line what it does; the person reads it before allowing the run"
    );
}

// steps.ts

#[test]
fn step_table_has_the_eleven_ts_kinds_and_a_lookup() {
    assert_eq!(SLATE_STEPS.len(), 11);
    assert_eq!(slate_step_module("fill").map(|s| s.runs), Some("window"));
    assert_eq!(slate_step_module("send").map(|s| s.runs), Some("host"));
    assert_eq!(slate_step_module("constructor"), None);
}

#[test]
fn link_domain_is_the_lowercased_host_of_a_web_link_only() {
    assert_eq!(slate_link_domain("https://GitHub.com/x").as_deref(), Some("github.com"));
    assert_eq!(slate_link_domain("http://[::1]:8080/a").as_deref(), Some("[::1]"));
    assert_eq!(slate_link_domain("mailto:a@b.c"), None);
    assert_eq!(slate_link_domain("ftp://x.y"), None);
    assert_eq!(slate_link_domain("nope"), None);
}

#[test]
fn domain_key_is_kept_under_the_domain_prefix() {
    assert_eq!(slate_domain_key("x.com"), "domain:x.com");
}

// sources.ts

#[test]
fn known_sources_are_the_twelve_registry_names() {
    assert_eq!(SLATE_SOURCE_NAMES.len(), 12);
    for name in ["thread", "tree", "usage", "cost", "time", "git", "pr", "machine", "processes", "ports", "notifications", "mcp"] {
        assert!(is_slate_source(name), "{name}");
    }
    assert!(!is_slate_source("toString"));
    assert!(!is_slate_source("constructor"));
    assert!(!is_slate_source("nope"));
}

// chart.ts

#[test]
fn chart_axis_rounds_to_four_or_five_steps_holding_every_value() {
    let cases: &[(&[f64], f64, f64, usize)] = &[
        (&[1.0, 2.0, 3.0], 0.0, 4.0, 4),
        (&[0.0, 5.0], 0.0, 5.0, 5),
        (&[0.0, 100.0], 0.0, 100.0, 4),
        (&[5.0, 5.0], 4.95, 5.05, 4),
        (&[-3.0, -1.0], -3.0, -1.0, 4),
        (&[0.001, 0.0123], 0.0, 0.0125, 5),
        (&[10.0, 12.0, 15.0, 99.5], 0.0, 100.0, 4),
        (&[0.0], 0.0, 0.5, 5),
        (&[7.0], 6.9, 7.1, 4),
        (&[-0.5, 0.25, 1.0], -0.5, 1.5, 4),
        (&[1e6, 3.2e6], 0.0, 4e6, 4),
        (&[0.1, 0.2, 0.3], 0.0, 0.4, 4),
    ];
    for (values, from, to, parts) in cases {
        let axis = slate_chart_axis(values);
        assert_eq!((axis.from, axis.to, axis.parts), (*from, *to, *parts), "{values:?}");
    }
}

#[test]
fn chart_axis_survives_a_spread_that_overflows_or_underflows() {
    for values in [[-1e308, 1e308], [0.0, 5e-324], [-1.2e308, 0.59e308]] {
        let axis = slate_chart_axis(&values);
        assert!(axis.parts > 0);
        assert!(axis.from <= values.iter().copied().fold(f64::INFINITY, f64::min));
        assert!(axis.to >= values.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    }
    let wide = slate_chart_axis(&[-1e308, 1e308]);
    assert_eq!((wide.from, wide.to, wide.parts), (-1e308, 1e308, 4));
}

#[test]
fn chart_axis_of_no_values_is_not_a_number() {
    let axis = slate_chart_axis(&[]);
    assert!(axis.from.is_nan() && axis.to.is_nan());
    assert_eq!(axis.parts, 4);
}

#[test]
fn axis_word_reads_a_clock_time_and_anything_else_as_written() {
    assert_eq!(slate_axis_word(Some(&json!("hello"))), "hello");
    assert_eq!(slate_axis_word(Some(&json!(5))), "5");
    assert_eq!(slate_axis_word(Some(&json!(1e12))), "1000000000000");
    assert_eq!(slate_axis_word(Some(&json!("2026-10-04"))), "2026-10-04");
    assert_eq!(slate_axis_word(Some(&json!(""))), "");
    assert_eq!(slate_axis_word(Some(&Value::Null)), "");
    assert_eq!(slate_axis_word(None), "");
    assert_eq!(slate_axis_word(Some(&json!(true))), "true");
    assert_eq!(slate_axis_word(Some(&json!([1, 2]))), "[1,2]");
    assert_eq!(slate_axis_word(Some(&json!({"a": 1}))), r#"{"a":1}"#);
    let clock = slate_axis_word(Some(&json!(1700000000000i64)));
    assert_eq!(clock.len(), 5);
    assert_eq!(&clock[2..3], ":");
    assert_eq!(slate_axis_word(Some(&json!("2026-10-04T09:12:00Z"))).len(), 5);
}

#[test]
fn time_reads_a_number_or_a_string_date_parse_takes() {
    assert_eq!(slate_time(Some(&json!(1700000000000i64))), Some(1700000000000.0));
    assert_eq!(slate_time(Some(&json!("2026-10-04T09:12:00Z"))), Some(1791105120000.0));
    assert_eq!(slate_time(Some(&json!("Oct 4 2026 10:00 UTC"))), Some(1791108000000.0));
    assert_eq!(slate_time(Some(&json!("  "))), None);
    assert_eq!(slate_time(Some(&json!("nope"))), None);
    assert_eq!(slate_time(Some(&Value::Null)), None);
    assert_eq!(slate_time(None), None);
}

#[test]
fn span_state_is_the_given_word_else_from_its_start_and_end() {
    assert_eq!(slate_span_state(Some(&json!("done")), Some(1.0), Some(2.0)), "done");
    assert_eq!(slate_span_state(Some(&json!("bogus")), Some(1.0), Some(2.0)), "done");
    assert_eq!(slate_span_state(None, None, Some(2.0)), "waiting");
    assert_eq!(slate_span_state(None, Some(1.0), None), "running");
    assert_eq!(slate_span_state(None, Some(1.0), Some(2.0)), "done");
    assert_eq!(slate_span_state(Some(&json!("failed")), None, None), "failed");
    assert_eq!(slate_span_state(Some(&json!(5)), Some(1.0), None), "running");
}

#[test]
fn shares_sort_biggest_first_clip_negatives_and_sum_the_rest_as_other() {
    let parts = slate_shares(&[("a", 3.0), ("b", -2.0), ("c", f64::NAN), ("d", 9.0), ("e", 1.0), ("f", 4.0), ("g", 2.0)]);
    let shown: Vec<(&str, f64, bool)> = parts.iter().map(|p| (p.name.as_str(), p.value, p.other)).collect();
    assert_eq!(
        shown,
        vec![("d", 9.0, false), ("f", 4.0, false), ("a", 3.0, false), ("g", 2.0, false), ("e", 1.0, false), ("Other", 0.0, true)]
    );
    let small = slate_shares(&[("x", 1.0)]);
    assert_eq!(small, vec![SlateShare { name: "x".into(), value: 1.0, other: false }]);
}

#[test]
fn time_floor_is_2000_utc() {
    assert_eq!(SLATE_TIME_FLOOR, 946684800000.0);
    assert_eq!(SLATE_DONUT_PARTS, 5);
}

// shape.ts

fn emails() -> Value {
    json!([
        {"messageId": "m1", "fromAddress": "billing@vercel.com", "subject": "Your invoice", "receivedTime": "2026-10-04 09:12", "flags": {"seen": true}},
        {"messageId": "m2", "fromAddress": "ops@example.com", "subject": "Traffic report", "receivedTime": "2026-10-04 08:40", "flags": {"seen": false}},
    ])
}

#[test]
fn a_list_of_records_is_a_table_with_titled_columns() {
    match slate_result_shape(Some(&emails())) {
        SlateResultShape::Table { rows, columns } => {
            assert_eq!(rows.len(), 2);
            let got: Vec<(&str, &str)> = columns.iter().map(|c| (c.key.as_str(), c.title.as_str())).collect();
            assert_eq!(
                got,
                vec![
                    ("messageId", "Message id"),
                    ("fromAddress", "From address"),
                    ("subject", "Subject"),
                    ("receivedTime", "Received time")
                ]
            );
        }
        other => panic!("not a table: {other:?}"),
    }
}

#[test]
fn a_record_has_facts_and_names_its_lists_and_records() {
    let record = json!({"account": "admin", "total": 2, "tags": ["a", "b"], "emails": emails(), "deep": {"a": {"b": 1}}});
    match slate_result_shape(Some(&record)) {
        SlateResultShape::Record { facts, nested } => {
            let got: Vec<(&str, &str)> = facts.iter().map(|f| (f.label.as_str(), f.value.as_str())).collect();
            assert_eq!(got, vec![("Account", "admin"), ("Total", "2"), ("Tags", "a, b")]);
            let titles: Vec<(&str, &str)> = nested
                .iter()
                .map(|n| {
                    let kind = match &n.shape {
                        SlateResultShape::Table { .. } => "table",
                        SlateResultShape::Record { .. } => "record",
                        SlateResultShape::Text(_) => "text",
                        SlateResultShape::None => "none",
                    };
                    (n.title.as_str(), kind)
                })
                .collect();
            assert_eq!(titles, vec![("Emails", "table"), ("Deep", "record")]);
        }
        other => panic!("not a record: {other:?}"),
    }
}

#[test]
fn text_and_nothing_have_their_own_kinds() {
    assert_eq!(slate_result_shape(Some(&json!("No new mail."))), SlateResultShape::Text("No new mail.".into()));
    assert_eq!(slate_result_shape(Some(&json!([]))), SlateResultShape::None);
    assert_eq!(slate_result_shape(Some(&Value::Null)), SlateResultShape::None);
    assert_eq!(slate_result_shape(Some(&json!(""))), SlateResultShape::None);
    assert_eq!(slate_result_shape(None), SlateResultShape::None);
    assert_eq!(slate_result_shape(Some(&json!([1, "a", null]))), SlateResultShape::Text("1\na\nnull".into()));
    assert_eq!(slate_result_shape(Some(&json!([{"a": 1}, 2]))), SlateResultShape::Text(r#"[{"a":1},2]"#.into()));
}

#[test]
fn the_sketch_names_a_table_by_its_size_and_titles() {
    let table = slate_result_shape(Some(&emails()));
    assert_eq!(sketch_slate_result(&table), vec!["table of 2 rows: Message id, From address, Subject, Received time"]);
    let one = slate_result_shape(Some(&json!([{"a": 1, "b": 2, "c": 3, "d": 4, "e": 5, "f": 6, "g": 7, "h": 8}])));
    assert_eq!(sketch_slate_result(&one), vec!["table of 1 row: A, B, C, D, E, F"]);
    let many = slate_result_shape(Some(&json!((0..52).map(|i| json!({"i": i})).collect::<Vec<_>>())));
    assert_eq!(sketch_slate_result(&many), vec!["table of 50 rows and 2 more: I"]);
}

#[test]
fn the_sketch_nests_a_record_and_cuts_a_long_fact() {
    let record = slate_result_shape(Some(&json!({"note": "y".repeat(40), "n": 1})));
    assert_eq!(sketch_slate_result(&record), vec!["Note yyyyyyyyyyyyyyyyyyyyyyy…, N 1"]);
    let nested = slate_result_shape(Some(&json!({"k": {"x": {"y": 1}}})));
    assert_eq!(sketch_slate_result(&nested), vec![r#"K: X {"y":1}"#]);
    let user = slate_result_shape(Some(&json!({"user": {"name": "ann", "profile": {"age": 3, "tags": ["x"]}}})));
    assert_eq!(sketch_slate_result(&user), vec![r#"User: Name ann, Profile {"age":3,"tags":["x"]}"#]);
}

#[test]
fn the_sketch_of_text_cuts_at_sixty_and_names_the_lines() {
    assert_eq!(sketch_slate_result(&slate_result_shape(Some(&json!("x".repeat(70))))), vec![format!("text: {}…", "x".repeat(59))]);
    assert_eq!(
        sketch_slate_result(&slate_result_shape(Some(&json!("line one is here\nline two\nline three")))),
        vec!["text, 3 lines: line one is here"]
    );
    assert_eq!(sketch_slate_result(&slate_result_shape(Some(&json!("No new mail.")))), vec!["text: No new mail."]);
    assert!(sketch_slate_result(&SlateResultShape::None).is_empty());
}

#[test]
fn field_words_split_camel_and_snake_case_and_capitalise_the_first() {
    let cases = [
        ("receivedTime", "Received time"),
        ("received_time", "Received time"),
        ("messageId", "Message id"),
        ("HTTPServer", "Httpserver"),
        ("x", "X"),
        ("", ""),
        ("__", "__"),
        ("a-b_c", "A b c"),
        ("already Done", "Already done"),
        ("item2Name", "Item2 name"),
        ("ABC", "Abc"),
        ("a1B2", "A1 b2"),
    ];
    for (key, words) in cases {
        assert_eq!(slate_field_words(key), words, "{key:?}");
    }
}

// image.ts

fn src_problem(src: &str) -> SlateProblem {
    match slate_image_source(src) {
        SlateImageSource::Problem(p) => p,
        other => panic!("{src:?} was not refused: {other:?}"),
    }
}

#[test]
fn an_image_path_is_a_path_on_the_thread_computer() {
    for path in
        ["/tmp/home.png", "shots/home.png", "../elsewhere/plot.png", "/etc/passwd", "C:\\shots\\a.png", "c:/x.png", "  /spaced.png  "]
    {
        let want = path.trim();
        assert_eq!(slate_image_source(path), SlateImageSource::Path(want.to_string()), "{path}");
    }
}

#[test]
fn an_image_address_is_read_with_its_domain_lowercased() {
    assert_eq!(
        slate_image_source("https://GitHub.com/acme/lab/raw/main/a.png"),
        SlateImageSource::Url { url: "https://github.com/acme/lab/raw/main/a.png".into(), domain: "github.com".into() }
    );
    assert_eq!(
        slate_image_source("http://img.acme.test:8080/a.png"),
        SlateImageSource::Url { url: "http://img.acme.test:8080/a.png".into(), domain: "img.acme.test".into() }
    );
    assert_eq!(
        slate_image_source("https://:@acme.test/a.png"),
        SlateImageSource::Url { url: "https://acme.test/a.png".into(), domain: "acme.test".into() }
    );
    assert_eq!(
        slate_image_source("https://[::1]/a.png"),
        SlateImageSource::Url { url: "https://[::1]/a.png".into(), domain: "[::1]".into() }
    );
}

#[test]
fn an_image_src_refused_names_its_code_and_sentence() {
    let cases = [
        ("", "R900", "data-missing", "an image names no file; give a path such as /tmp/home.png or an https address"),
        ("  ", "R900", "data-missing", "an image names no file; give a path such as /tmp/home.png or an https address"),
        (
            "~/.ssh/id_ed25519.png",
            "R900",
            "data-missing",
            "~/.ssh/id_ed25519.png starts at a home folder, which wsp does not guess; write the whole path",
        ),
        (
            "file:///etc/hosts",
            "R917",
            "address-refused",
            "file:///etc/hosts is not an http or https address; an image is a path on the thread's computer or a web address",
        ),
        (
            "data:image/png;base64,AA",
            "R917",
            "address-refused",
            "data:image/png;base64,AA is not an http or https address; an image is a path on the thread's computer or a web address",
        ),
        (
            "javascript:alert(1)",
            "R917",
            "address-refused",
            "javascript:alert(1) is not an http or https address; an image is a path on the thread's computer or a web address",
        ),
        (
            "https://me:secret@acme.test/a.png",
            "R917",
            "address-refused",
            "https://me:secret@acme.test/a.png carries a login, which wsp never sends",
        ),
        (
            "a+b:/x",
            "R917",
            "address-refused",
            "a+b:/x is not an http or https address; an image is a path on the thread's computer or a web address",
        ),
    ];
    for (src, code, name, message) in cases {
        assert_eq!(src_problem(src), problem(code, name, message), "{src:?}");
    }
}

#[test]
fn an_image_too_big_is_said_with_its_size_rounded_up() {
    assert_eq!(
        slate_image_too_big("/tmp/a.png", Some(10 * 1024 * 1024 + 1)),
        problem("R915", "image-too-big", "/tmp/a.png is 10.1 MB, over the 10 MB an image may weigh; save it smaller, or show a part of it")
    );
    assert!(slate_image_too_big("/tmp/a.png", Some((10.4 * 1024.0 * 1024.0) as u64)).message.contains("is 10.4 MB, over"));
    assert_eq!(
        slate_image_too_big("/tmp/a.png", None).message,
        "/tmp/a.png is more than the 10 MB an image may weigh; save it smaller, or show a part of it"
    );
    assert_eq!(
        slate_image_too_big("/tmp/a.png", Some(2048)).message,
        "/tmp/a.png is 2 KB, over the 10 MB an image may weigh; save it smaller, or show a part of it"
    );
    assert_eq!(SLATE_IMAGE_MAX_BYTES, 10 * 1024 * 1024);
}

#[test]
fn bytes_that_are_no_image_name_an_svg_and_the_types() {
    assert_eq!(slate_not_an_image("/k.pem", false), problem("R916", "not-an-image", "/k.pem is not a PNG, JPEG, GIF or WebP image"));
    assert_eq!(
        slate_not_an_image("/a.svg", true),
        problem("R916", "not-an-image", "/a.svg is an SVG, which a slate does not show since it can load other things; save it as a PNG")
    );
    assert_eq!(slate_not_an_image("/a.svg", false).message, "/a.svg is not a PNG, JPEG, GIF or WebP image");
}

#[test]
fn svg_is_judged_by_its_name_or_by_an_svg_tag_in_the_head() {
    let cases: [(&str, &str, bool); 11] = [
        ("/x/a.svg", "", true),
        ("/x/a.svg?v=2", "", true),
        ("/x/a.SVGZ", "", true),
        ("/x/a.txt", "<svg xmlns", true),
        ("/x/a.txt", "<SVG>", true),
        ("/x/a.txt", "<svgx", false),
        ("/x/a.txt", "<svg\n", true),
        ("/x/a.txt", "junk <svg/", false),
        ("/x/a.png", "PNG", false),
        ("/x/a.txt", "a<svg\t", true),
        ("/x/a.txt", "no", false),
    ];
    for (name, head, want) in cases {
        assert_eq!(slate_looks_svg(name, head.as_bytes()), want, "{name} {head:?}");
    }
}

#[test]
fn an_svg_tag_past_the_first_kilobyte_is_not_seen() {
    assert!(!slate_looks_svg("/x/a.txt", format!("{}<svg>", "x".repeat(1100)).as_bytes()));
    assert!(slate_looks_svg("/x/a.txt", format!("{}<svg>", "x".repeat(1000)).as_bytes()));
}

#[test]
fn a_list_too_long_names_the_bound_when_it_is_bound() {
    assert_eq!(
        slate_list_too_long(61, 60, "images", None),
        problem("R905", "draw-failed", "this list holds 61 images and a slate shows at most 60, so it shows none; show a page of them")
    );
    assert_eq!(
        slate_list_too_long(61, 60, "images", Some("shots")).message,
        "this list holds 61 images and a slate shows at most 60, so it shows none; show a page of them, like items={shots | take(60)}"
    );
    assert_eq!(
        slate_list_too_long(0, 3, "rows", None).message,
        "this list holds 0 rows and a slate shows at most 3, so it shows none; show a page of them"
    );
}

#[test]
fn the_address_missing_and_not_a_file_sentences() {
    assert_eq!(slate_address_refused("https://a/b", "why here"), problem("R917", "address-refused", "https://a/b why here"));
    assert_eq!(slate_image_missing("/x"), problem("R900", "data-missing", "no file at /x"));
    assert_eq!(slate_image_not_a_file("/d"), problem("R914", "not-a-file", "/d is not a file; an image is a regular file"));
}

#[test]
fn the_layouts_are_the_three_ts_words() {
    assert_eq!(SLATE_IMAGE_LAYOUTS, &["strip", "gallery", "compare"]);
}
