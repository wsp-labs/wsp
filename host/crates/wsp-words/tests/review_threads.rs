// Review of the threads area: sub_thread, thread lines and forks, tools, tree, pull_request.
// Every expected value below was printed by node from /root/rvw2445/ts/bundle.mjs.
use serde_json::{json, Value};
use wsp_wire::wire::helpers::{PermissionEffect, PermissionOption};
use wsp_words::sub_thread::{is_git_push, is_sub_oom_line, sub_copying_line, sub_result_line, KeptUntil, SubResultFacts};
use wsp_words::words::thread::{iso_seconds, no_adapter_line, validator_refusal};
use wsp_words::words::tools::{permission_prompt_words, picked_options, question_answer_input, tool_activity_line};

fn unpushed_commit_facts(until: f64) -> SubResultFacts {
    let result = serde_json::from_value(json!({
        "branch": "wsp/x", "pushedSha": "abcdef0123456789", "commits": 2, "unpushed": 1, "changed": 0, "readAt": "2026-10-01T00:00:00.000Z"
    }))
    .unwrap();
    SubResultFacts {
        thread_id: "0123456789".into(),
        lead: "fedcba9876543210".into(),
        result,
        base: "main".into(),
        delete_in_ms: None,
        kept: Some(KeptUntil { on: "Boat".into(), until }),
    }
}

fn answer_options() -> Vec<PermissionOption> {
    vec![
        PermissionOption { id: "q0:o0".into(), label: "A".into(), effect: PermissionEffect::Answer, mode: None },
        PermissionOption { id: "q0:o1".into(), label: "B".into(), effect: PermissionEffect::Answer, mode: None },
    ]
}

// F1. sub-thread.ts:535 utcDay, end.rs:185 utc_day. A kept time in the last two days of year 9999 is a valid JS date;
// jiff stops at 9999-12-30T22:00:00Z and the `.expect` panics.
#[test]
fn f01_the_kept_day_of_the_last_day_of_9999() {
    assert_eq!(
        sub_result_line(&unpushed_commit_facts(253402300799999.0)),
        "1 commit on wsp/x is not pushed, so its machine naps at no cost, and Boat may delete its disk from 9999-12-31 UTC. Send to 01234567 to have it pushed, or ask the person to delete it."
    );
}

// F1 variant: from year 10000 the TS day is `+010000-01` (toISOString's extended year, cut to 10 characters).
#[test]
fn f01b_the_kept_day_of_year_10000() {
    assert_eq!(
        sub_result_line(&unpushed_commit_facts(253402300800000.0)),
        "1 commit on wsp/x is not pushed, so its machine naps at no cost, and Boat may delete its disk from +010000-01 UTC. Send to 01234567 to have it pushed, or ask the person to delete it."
    );
}

// F2. thread.ts:799 isoSeconds, forks.rs:755 iso_seconds: the same `.expect` on the same range.
#[test]
fn f02_iso_seconds_of_the_last_second_of_9999() {
    assert_eq!(iso_seconds(253402300799999.0), "9999-12-31T23:59:59Z");
}

// F3. sub-thread.ts:441 /^the agent on .+ was killed for memory: /i, sub_thread.rs:1337. The delimiter starts and ends
// with a space, so a second copy can share the first copy's last space; match_indices skips overlapping matches.
#[test]
fn f03_the_oom_delimiter_may_share_its_last_space() {
    assert!(is_sub_oom_line("the agent on  was killed for memory: was killed for memory: "));
}

// F4. thread.ts:443-445 `agents.join(", ") || "none"`, lines.rs:915 no_adapter_line: one empty id joins to "" which is falsy.
#[test]
fn f04_no_adapter_line_of_one_empty_id_says_none() {
    assert_eq!(
        no_adapter_line("h", &[""]),
        "no adapter registered for harness \"h\"; agents on this host: none"
    );
}

// F5. thread.ts:64,95 REQUEST_WORDS is a plain object, so `REQUEST_WORDS["constructor"]` is Object itself and is joined
// into the sentence; lines.rs:922 request_word is a closed match.
#[test]
fn f05_validator_refusal_reads_an_inherited_object_name_like_the_ts() {
    assert_eq!(
        validator_refusal(r#"[{"code":"x","path":["constructor"]}]"#).as_deref(),
        Some("the host would not read function Object() { [native code] } on this line")
    );
}

// F6. tools.ts:296 toolInput JSON.parse, rows.rs:131 tool_input serde_json::from_str: 1e999 is Infinity in JS and an
// error in serde_json, so the whole input is read as "not an object".
#[test]
fn f06_a_tool_input_with_a_number_past_f64_is_still_an_object() {
    assert_eq!(tool_activity_line(Some("Bash"), r#"{"a":1e999,"command":"ls"}"#), "$ ls");
}

// F6 variant: sub-thread.ts:406 shellCommandOf, sub_thread.rs:1258 shell_command_of.
#[test]
fn f06b_is_git_push_reads_command_of_json_with_a_number_past_f64() {
    assert!(is_git_push(r#"{"a":1e999,"command":"git\npush"}"#));
}

// F7. Same two functions: serde_json stops at 128 levels of nesting, JSON.parse does not.
#[test]
fn f07_a_tool_input_nested_200_deep_is_still_an_object() {
    let input = format!(r#"{{"x":{}{},"command":"ls"}}"#, "[".repeat(200), "]".repeat(200));
    assert_eq!(tool_activity_line(Some("Bash"), &input), "$ ls");
}

// F8. tools.ts:639 Object.entries(fields) and tools.ts:236 answers[question.key] (tools.rs:1105 and tools.rs:439): a JS
// object lists integer-like keys first in ascending order; the Rust maps keep insertion order.
#[test]
fn f08_prompt_rest_lists_integer_like_keys_first() {
    let words = permission_prompt_words("Bash", r#"{"1":"a","b":"c","0":"z"}"#, None);
    assert_eq!(words.rest, "0: z\n1: a\nb: c");
}

#[test]
fn f08b_answers_list_integer_like_question_texts_first() {
    let input = r#"{"questions":[{"question":"b","options":[{"label":"x"}]},{"question":"1","options":[{"label":"y"}]},{"question":"0","options":[{"label":"z"}]}]}"#;
    let picked = ["q0:o0", "q1:o0", "q2:o0"];
    let answered = question_answer_input("AskUserQuestion", input, &picked).map(Value::Object).map(|v| serde_json::to_string(&v).unwrap());
    assert_eq!(
        answered.as_deref(),
        Some(r#"{"questions":[{"question":"b","options":[{"label":"x"}]},{"question":"1","options":[{"label":"y"}]},{"question":"0","options":[{"label":"z"}]}],"answers":{"0":"z","1":"y","b":"x"}}"#)
    );
}

// F9. tools.ts:203 decodeURIComponent, rows.rs:222 decode_uri_component: u8::from_str_radix accepts a
// leading `+`, so `%+1` decodes to byte 0x01 where JS throws URIError and the pick is refused.
#[test]
fn f09_a_percent_plus_escape_is_not_a_typed_answer() {
    assert_eq!(picked_options(&answer_options(), "q0:other:%+1"), None);
}

// F10. tools.ts:203 Number(match[1]), rows.rs:239 parse::<usize>: a question number past u64 is a float in JS and
// no question matches it, but the typed answer is still read; Rust returns None for the whole pick.
#[test]
fn f10_a_typed_answer_for_a_huge_question_number_is_still_read() {
    assert_eq!(
        picked_options(&answer_options(), "q99999999999999999999:other:x"),
        Some(vec![PermissionOption { id: "q99999999999999999999:other:x".into(), label: "x".into(), effect: PermissionEffect::Answer, mode: None }])
    );
}

// F11. Not in this area's files: wsp-wire js_number.rs:24 JsNumber::js_string (reached here through sub_copying_line). A 16 or 17
// digit non-integer whose two shortest spellings are equally near is written by JS with the even last digit.
#[test]
fn f11_number_text_picks_the_even_digit_on_a_tie() {
    assert_eq!(sub_copying_line(997861641924828.2), "copying 997861641924828.2 files");
}
