// SPDX-License-Identifier: AGPL-3.0-only
//! What zod did beside the shape, held to zod's own answers: the first issue a refused value raises (code, path,
//! message, the rest of its keys and their order), a JavaScript number's text, a literal, the null-or-absent of an
//! optional nullable, the required nullable, and the UTF-16 count every length rule reads. The issues come from
//! fixtures/rules/issues.json, written by running zod on the TS schemas. spec/check.md lists the rule each test pins.

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use wsp_wire::types::check::{from_json, parse, utf16_len, Check, SchemaError};
use wsp_wire::types::thread_defaults::ProjectOverrides;
use wsp_wire::types::JsNumber;
use wsp_wire::views::app_procs::{AppProc, AppProcList};
use wsp_wire::views::place::WorkspaceCopy;
use wsp_wire::views::preferences::{PreferencesPatch, PreferencesTarget};
use wsp_wire::views::session_events::SessionEvent;
use wsp_wire::views::sub_row::SubResult;
use wsp_wire::views::workspace::ThreadScope;

const ISSUES: &str = include_str!("fixtures/rules/issues.json");

fn zod_case<T: DeserializeOwned + Check + std::fmt::Debug>(ty: &str) {
    let all: Vec<Value> = serde_json::from_str(ISSUES).unwrap();
    let mine: Vec<&Value> = all.iter().filter(|c| c["type"] == ty).collect();
    assert!(!mine.is_empty(), "no issue cases for {ty}");
    for c in mine {
        let what = c["case"].as_str().unwrap();
        let err = parse::<T>(&c["input"]).expect_err(&format!("{ty}, {what}: zod refused it"));
        let issue = &c["issue"];
        assert_eq!(err.code, issue["code"].as_str().unwrap(), "{ty}, {what}: code");
        assert_eq!(Value::Array(err.path.clone()), issue["path"], "{ty}, {what}: path");
        assert_eq!(err.message, issue["message"].as_str().unwrap(), "{ty}, {what}: message");
        let detail: IndexMap<String, Value> = issue
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, _)| !["code", "path", "message"].contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        assert_eq!(err.detail.keys().collect::<Vec<_>>(), detail.keys().collect::<Vec<_>>(), "{ty}, {what}: detail keys in zod's order");
        assert_eq!(*err.detail, detail, "{ty}, {what}: detail values");
        assert_eq!(err.issue_text(), c["text"].as_str().unwrap(), "{ty}, {what}: the issue as ZodError.message writes it");
    }
}

mod the_first_issue {
    use super::*;

    #[test]
    fn a_missing_key_a_wrong_type_a_fraction_a_negative_count_and_a_missing_nullable_are_zod_s_issues() {
        zod_case::<SubResult>("SubResult");
    }

    #[test]
    fn a_bound_a_utf16_length_an_item_count_and_an_enum_word_are_zod_s_issues_at_zod_s_paths() {
        // One origin is 129 emoji: 129 chars but 258 UTF-16 units, past the 256 the rule counts in.
        zod_case::<AppProc>("AppProc");
    }

    #[test]
    fn an_issue_in_the_second_row_of_a_list_has_the_index_as_a_number_first_in_its_path() {
        zod_case::<AppProcList>("AppProcList");
    }

    #[test]
    fn a_strict_object_refuses_every_unknown_key_in_one_issue_at_its_own_path() {
        zod_case::<PreferencesTarget>("PreferencesTarget");
    }

    #[test]
    fn a_kept_literal_refuses_another_word_with_invalid_literal() {
        zod_case::<ThreadScope>("ThreadScope");
    }

    #[test]
    fn a_tagged_union_refuses_an_unknown_or_missing_tag_naming_every_tag_it_takes() {
        zod_case::<SessionEvent>("SessionEvent");
    }

    #[test]
    fn a_failed_regex_is_invalid_string_and_a_failed_refine_is_custom_with_the_refine_s_words() {
        zod_case::<ProjectOverrides>("ProjectOverrides");
        zod_case::<WorkspaceCopy>("WorkspaceCopy");
        zod_case::<PreferencesPatch>("PreferencesPatch");
    }

    #[test]
    fn under_puts_a_segment_in_front_of_the_path_and_keeps_the_rest() {
        let e = SchemaError { code: "too_small".into(), path: vec![json!("pid")], message: "m".into(), detail: Box::default() };
        let moved = e.clone().under(3).under("procs");
        assert_eq!(moved.path, vec![json!("procs"), json!(3), json!("pid")]);
        assert_eq!((moved.code, moved.message, moved.detail), (e.code, e.message, e.detail));
    }
}

mod javascript_numbers {
    use super::*;

    #[test]
    fn a_number_s_text_is_javascript_s_string_of_it() {
        let cases: [(f64, &str); 17] = [
            (3.0, "3"),
            (-0.0, "0"),
            (2.5, "2.5"),
            (0.1 + 0.2, "0.30000000000000004"),
            (9007199254740992.0, "9007199254740992"),
            (9007199254740994.0, "9007199254740994"),
            (1e20, "100000000000000000000"),
            (123456789012345680000.0, "123456789012345680000"),
            (1e21, "1e+21"),
            (1.5e300, "1.5e+300"),
            (1e-6, "0.000001"),
            (1e-7, "1e-7"),
            (123e-20, "1.23e-18"),
            (-1e-7, "-1e-7"),
            (-42.0, "-42"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
        ];
        for (n, want) in cases {
            assert_eq!(JsNumber(n).js_string(), want, "{n:e}");
        }
    }

    #[test]
    fn an_integral_number_under_2_53_writes_as_a_json_integer_and_minus_zero_as_0() {
        for (n, want) in [(3.0, json!(3)), (-0.0, json!(0)), (-42.0, json!(-42)), (9007199254740991.0, json!(9007199254740991_i64))] {
            let v = serde_json::to_value(JsNumber(n)).unwrap();
            assert_eq!(v, want, "{n}");
            assert!(v.is_i64() || v.is_u64(), "{n} is written as an integer, not a float");
            assert_eq!(serde_json::to_string(&JsNumber(n)).unwrap(), want.to_string());
        }
    }

    #[test]
    fn a_fraction_writes_as_javascript_writes_it_where_serde_json_can_carry_that_text() {
        for (n, want) in [(2.5, "2.5"), (0.1 + 0.2, "0.30000000000000004"), (1e-7, "1e-7"), (123e-20, "1.23e-18"), (5e-324, "5e-324")] {
            assert_eq!(serde_json::to_string(&JsNumber(n)).unwrap(), want, "{n:e}");
        }
    }

    #[test]
    fn any_json_number_reads_and_a_string_does_not() {
        assert_eq!(serde_json::from_str::<JsNumber>("3").unwrap(), JsNumber(3.0));
        assert_eq!(serde_json::from_str::<JsNumber>("-2.5e3").unwrap(), JsNumber(-2500.0));
        assert_eq!(serde_json::from_str::<JsNumber>("18446744073709551616").unwrap(), JsNumber(18446744073709551616.0));
        assert!(serde_json::from_str::<JsNumber>("\"3\"").is_err());
        assert!(serde_json::from_str::<JsNumber>("null").is_err());
    }
}

mod literals_and_nulls {
    use super::*;

    #[test]
    fn a_kept_literal_writes_its_word_and_reads_only_it() {
        let scope: ThreadScope =
            serde_json::from_value(json!({ "kind": "thread", "threadId": "t", "workspaceId": "w", "rootThreadId": "r" })).unwrap();
        assert_eq!(serde_json::to_string(&scope).unwrap(), r#"{"kind":"thread","threadId":"t","workspaceId":"w","rootThreadId":"r"}"#);
        let refused =
            serde_json::from_value::<ThreadScope>(json!({ "kind": "lead", "threadId": "t", "workspaceId": "w", "rootThreadId": "r" }))
                .unwrap_err();
        assert!(refused.to_string().contains(r#"Invalid literal value, expected "thread""#), "{refused}");
    }

    #[test]
    fn an_optional_nullable_reads_absent_null_and_a_value_apart_and_writes_each_back_as_it_was() {
        for (input, want) in
            [(json!({}), None), (json!({ "sidebarWidth": null }), Some(None)), (json!({ "sidebarWidth": 300 }), Some(Some(300.into())))]
        {
            let patch: PreferencesPatch = serde_json::from_value(input.clone()).unwrap();
            assert_eq!(patch.sidebar_width, want, "{input}");
            assert_eq!(serde_json::to_value(&patch).unwrap(), input, "written back as read");
        }
    }

    #[test]
    fn a_required_nullable_reads_null_as_none_and_refuses_a_missing_key() {
        let base = json!({ "branch": "b", "commits": 0, "unpushed": 0, "changed": 0, "readAt": "t" });
        let mut with_null = base.clone();
        with_null["pushedSha"] = Value::Null;
        let read: SubResult = serde_json::from_value(with_null).unwrap();
        assert_eq!(read.pushed_sha, None);
        assert!(serde_json::to_string(&read).unwrap().contains(r#""pushedSha":null"#), "null is written, not left out");
        let mut with_sha = base.clone();
        with_sha["pushedSha"] = json!("abc");
        assert_eq!(serde_json::from_value::<SubResult>(with_sha).unwrap().pushed_sha.as_deref(), Some("abc"));
        let missing = serde_json::from_value::<SubResult>(base).unwrap_err();
        assert!(missing.to_string().contains("missing field `pushedSha`"), "{missing}");
    }

    #[test]
    fn a_default_is_the_json_the_ts_default_holds() {
        let v: Vec<String> = from_json(r#"["a","b"]"#);
        assert_eq!(v, vec!["a", "b"]);
        let n: Option<i64> = from_json("null");
        assert_eq!(n, None);
    }
}

#[test]
fn a_length_counts_utf16_units_as_javascript_s_length_does() {
    for (s, n) in [("", 0), ("a", 1), ("é", 1), ("e\u{301}", 2), ("😀", 2), ("a😀b", 4), ("\u{10FFFF}", 2), ("中文", 2)] {
        assert_eq!(utf16_len(s), n, "{s:?}");
    }
}

#[test]
fn a_list_an_option_and_a_map_check_each_value_with_its_index_or_key_in_front() {
    let bad: AppProc = serde_json::from_value(json!({ "pid": 0, "kind": "main" })).unwrap();
    let good: AppProc = serde_json::from_value(json!({ "pid": 1, "kind": "main" })).unwrap();
    let list = vec![good.clone(), bad.clone()];
    assert_eq!(list.check().unwrap_err().path, vec![json!(1), json!("pid")]);
    assert!(Some(good.clone()).check().is_ok());
    assert_eq!(Some(bad.clone()).check().unwrap_err().path, vec![json!("pid")]);
    let mut map: IndexMap<String, AppProc> = IndexMap::new();
    map.insert("ok".into(), good);
    map.insert("bad".into(), bad);
    assert_eq!(map.check().unwrap_err().path, vec![json!("bad"), json!("pid")]);
    assert!(None::<AppProc>.check().is_ok());
}
