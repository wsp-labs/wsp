// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the agents area against the TS at a3b870a6e. Every test fails today for the reason in its name; every
//! expected value is what node printed for the TS function (commands in the review report). The differential
//! harness that found them is kept outside the crate, in /root/rvw2445/agents/probe/.

use serde_json::{json, Value};
use wsp_wire::types::attachments::AttachmentRecord;
use wsp_wire::types::thread_read::ThreadMessage;
use wsp_wire::views::activity::{ActivityRead, ActivityScope};
use wsp_words::attachments::{attachment_line, files_blocked, image_path_in, is_image};
use wsp_words::changes::cut_diff;
use wsp_words::pull_request::tail_within;
use wsp_words::run_block::terminal_text;
use wsp_words::thread_read::thread_markdown;
use wsp_words::views::activity::end_plan;
use wsp_words::wsp_tools::{threads_followed, Followed};

fn record(media_type: &str, bytes: u64) -> AttachmentRecord {
    serde_json::from_value(json!({ "mediaType": media_type, "bytes": bytes })).unwrap()
}

/// TS: attachments.ts:18 IMAGE_TYPES is a plain object, so `IMAGE_TYPES["constructor"]` is `Object` and reads as an
/// image type (spec attachments.md, After the port: the Rust must copy it).
/// node -e 'A=M["attachments"]; A.isImage("constructor")' -> true; A.attachmentLine({mediaType:"toString",bytes:5})
/// -> "[image 5 B undefined]"; A.imagePathIn("/d",0,"toString") -> "/d/1.undefined"; A.filesBlocked([{mediaType:
/// "constructor",bytes:1}],undefined,"cursor") -> "cursor takes no image with a message; ..."
#[test]
fn attachments_read_the_keys_of_object_prototype_as_image_types_like_the_ts() {
    assert!(is_image("constructor"));
    assert!(is_image("toString"));
    assert_eq!(attachment_line(&record("toString", 5)), "[image 5 B undefined]");
    assert_eq!(image_path_in("/d", 0.0, "toString").map_err(|e| e.message), Ok("/d/1.undefined".to_string()));
    assert_eq!(
        files_blocked(&[record("constructor", 1)], None, "cursor"),
        Some("cursor takes no image with a message; describe it in words, or open the thread on an agent that reads images".to_string())
    );
}

/// TS: wsp-tools.ts:75 reads the call's input with JSON.parse, which takes `1e400` (as Infinity) and any depth the
/// stack allows; serde_json refuses a number out of range and nesting past 128, so the Rust answers None.
/// node: threadsFollowed({toolName:"mcp__wsp__send",input:'{"thread":"t1","x":1e400}'}) -> {"named":["t1"]}
#[test]
fn threads_followed_reads_json_the_way_json_parse_does() {
    assert_eq!(threads_followed("mcp__wsp__send", r#"{"thread":"t1","x":1e400}"#), Some(Followed::Named(vec!["t1".to_string()])));
    let deep = format!(r#"{{"thread":"t1","x":{}{}}}"#, "[".repeat(200), "]".repeat(200));
    assert_eq!(threads_followed("mcp__wsp__send", &deep), Some(Followed::Named(vec!["t1".to_string()])));
}

fn thread_row(at: f64) -> ThreadMessage {
    serde_json::from_value(json!({ "who": "person", "text": "x", "at": at })).unwrap()
}

/// TS: words/turn.ts:239 fmtClock is `new Date(at).toTimeString().slice(0, 8)`; a time outside +-8.64e15 is an
/// Invalid Date, whose text starts "Invalid ". The Rust prints a clock made of the number's remainder.
/// node: M["thread-read"].threadMarkdown([{who:"person",text:"x",at:1e16}]) -> "**You** Invalid \n\nx"
#[test]
fn thread_markdown_prints_invalid_for_a_time_past_the_date_range() {
    assert_eq!(thread_markdown(&[thread_row(1e16)]), "**You** Invalid \n\nx");
    assert_eq!(thread_markdown(&[thread_row(-8.64e15 - 1.0)]), "**You** Invalid \n\nx");
}

/// TS: run-block.ts:48 walks UTF-16 units, so a character above U+FFFF fills two columns: a carriage return writes
/// over two letters, and it takes two backspaces to take it off. The Rust counts one column per char.
/// node: terminalText("ab\r😀") -> "😀"; terminalText("Loading...\r🚀 Done") -> "🚀 Done..."; terminalText("x😀\b\bY") -> "xY"
#[test]
fn terminal_text_counts_columns_in_utf16_units_so_an_emoji_takes_two() {
    assert_eq!(terminal_text("ab\r\u{1F600}"), "\u{1F600}");
    assert_eq!(terminal_text("Loading...\r\u{1F680} Done"), "\u{1F680} Done...");
    assert_eq!(terminal_text("x\u{1F600}\u{8}\u{8}Y"), "xY");
}

/// TS: changes.ts:56 cutDiff with max NaN: `bytes.length <= NaN` is false and `subarray(0, NaN)` is empty.
/// node: M["changes"].cutDiff("abc", NaN) -> ""
#[test]
fn cut_diff_with_a_nan_cap_keeps_nothing() {
    assert_eq!(cut_diff("abc", Some(f64::NAN)), "");
}

/// TS: pull-request.ts:191 tailWithin takes `bytes.subarray(bytes.length - max)`, which truncates a fractional start
/// and reads a NaN cap as a start of NaN, so the whole text stays; the Rust casts max to usize first.
/// node: tailWithin("abc", NaN) -> "abc"; tailWithin("abc", 1.5) -> "bc"; tailWithin("abcdef", 3.5) -> "cdef"
#[test]
fn tail_within_reads_a_nan_or_fractional_cap_the_way_subarray_does() {
    assert_eq!(tail_within("abc", f64::NAN), "abc");
    assert_eq!(tail_within("abc", 1.5), "bc");
    assert_eq!(tail_within("abcdef", 3.5), "cdef");
}

fn proc_row(pid: i64, ppid: i64, start: i64, owner: bool) -> Value {
    let mut v = json!({ "pid": pid, "ppid": ppid, "user": "ada", "state": "S", "comm": "node", "cmdline": "node x.js", "cpu": 1, "rss": 1, "startedAt": start + pid });
    if owner {
        v["owner"] = json!({ "by": "env", "thread": "t1" });
    }
    v
}

fn read_of(procs: Vec<Value>) -> ActivityRead {
    ActivityRead {
        computer: "c".to_string(),
        at: wsp_wire::types::js_number::JsNumber(1_760_000_000_000.0),
        procs: serde_json::from_value(Value::Array(procs)).unwrap(),
        daemon: 1,
        host: 2,
        scopes: vec![ActivityScope { thread_id: "t1".into(), title: "T".into(), workspace_id: "w".into(), roots: vec![], ..Default::default() }],
        agent_bins: vec![],
        ports: None,
        cgroups: None,
    }
}

const START: i64 = 1_760_000_000_000;
const CYCLE_TEST: &str = "an_end_over_two_live_rows_that_name_each_other_as_parent_answers_an_error";

/// TS: views/activity.ts:511-518 `take` recurses with no seen set, so two live rows naming each other as parent end
/// in a RangeError ("Maximum call stack size exceeded"), which the op reports. The spec lists it under After the
/// port as TS behaviour to copy. The Rust walk is a loop over a Vec that never ends: memory grows until the host is
/// killed. The test runs the walk in a child under `ulimit -v` so a failing run stays small.
/// node: endPlan(read of rows 100 and 101 naming each other, 100, start, () => true) -> RangeError
#[test]
fn an_end_over_two_live_rows_that_name_each_other_as_parent_answers_an_error() {
    if std::env::var("WSP_REVIEW_CYCLE_CHILD").is_ok() {
        let read = read_of(vec![proc_row(100, 101, START, true), proc_row(101, 100, START, true), proc_row(1, 0, START, false)]);
        let answer = end_plan(&read, 100.0, (START + 100) as f64, &|_| true);
        assert_eq!(answer.map(|_| ()).map_err(|e| e.message), Err("Maximum call stack size exceeded".to_string()));
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("ulimit -v 1000000; exec timeout 60 \"$0\" --exact {CYCLE_TEST} --nocapture"))
        .arg(exe)
        .env("WSP_REVIEW_CYCLE_CHILD", "1")
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stderr).lines().filter(|l| l.contains("memory allocation") || l.contains("panicked")).collect::<Vec<_>>().join(" | ");
    assert!(out.status.success(), "the walk did not end in an error ({:?}): {said}; the TS throws RangeError", out.status);
}

/// TS: views/activity.ts:497 builds the reused-pid refusal with `new Date(asked).toISOString()`, which throws
/// RangeError "Invalid time value" past +-8.64e15; the Rust writes a date for any number (wsp-wire's
/// end_reused_line, reached from end_plan with `started_at as i64`).
/// node: endPlan(read, 100, 1e17, () => true) -> RangeError: Invalid time value
#[test]
fn an_end_naming_a_start_past_the_date_range_throws_the_ts_range_error() {
    let read = read_of(vec![proc_row(100, 1, START, true), proc_row(1, 0, START, false)]);
    let answer = end_plan(&read, 100.0, 1e17, &|_| true);
    assert_eq!(answer.map(|_| ()).map_err(|e| e.message), Err("Invalid time value".to_string()));
}
