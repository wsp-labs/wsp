// SPDX-License-Identifier: AGPL-3.0-only
//! The held forward and the detached run replaying `oracle/roads_scenes.oracle.ts`'s scenes against what the TS did
//! (spec/ssh_dial.md rules D-H1..D-H10, spec/exec_run.md rules R-1..R-10).
mod roads_common;

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use futures::FutureExt;
use roads_common::*;
use serde_json::{json, Value};
use tokio::io::{AsyncWriteExt, DuplexStream};
use tokio::sync::oneshot;
use wsp_engine::exec_detached::exec_detached_in;
use wsp_engine::machine::{ExecOptions, RunOptions};
use wsp_engine::ssh_backend::{hold_back_forward, ChildEnd, HeldChild, SshCarried, SshReach};

fn scenes() -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/roads.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn find(list: &str, name: &str) -> Value {
    scenes()[list].as_array().unwrap().iter().find(|s| s["name"] == name).unwrap_or_else(|| panic!("no scene {name}")).clone()
}

/// As the oracle writes a string past 1,000 UTF-16 units: its length and its two ends.
fn short(s: &str) -> Value {
    let units: Vec<u16> = s.encode_utf16().collect();
    if units.len() <= 1000 {
        return json!(s);
    }
    json!({ "len": units.len(), "head": String::from_utf16_lossy(&units[..20]), "tail": String::from_utf16_lossy(&units[units.len() - 20..]) })
}

async fn within<T>(fut: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), fut).await.expect("timed out")
}

struct Fake {
    stdout: Option<DuplexStream>,
    stderr: Option<DuplexStream>,
    _stdin: DuplexStream,
    end: Option<oneshot::Sender<ChildEnd>>,
}

fn fake_child() -> (HeldChild, Fake) {
    let (stdout_w, stdout_r) = tokio::io::duplex(1 << 20);
    let (stderr_w, stderr_r) = tokio::io::duplex(1 << 20);
    let (stdin_w, stdin_r) = tokio::io::duplex(1 << 16);
    let (end_tx, end_rx) = oneshot::channel();
    let child = HeldChild {
        stdin: Box::new(stdin_w),
        stdout: Box::new(stdout_r),
        stderr: Box::new(stderr_r),
        kill: Box::new(|_: &str| true),
        exit: async move { end_rx.await.unwrap_or_else(|_| ChildEnd::Failed("dropped".into())) }.boxed(),
    };
    (child, Fake { stdout: Some(stdout_w), stderr: Some(stderr_w), _stdin: stdin_r, end: Some(end_tx) })
}

/// Writes the scene's steps to a fake child, each its own write with a pause after it, and holds them against what
/// the TS's `up` and `ended` said.
async fn forward(name: &str) {
    let want = find("forwards", name);
    let (child, mut fake) = fake_child();
    let slot = Mutex::new(Some(child));
    let spawn = Arc::new(move |_: &str, _: &[String]| slot.lock().unwrap().take().unwrap());
    let carried = SshCarried { reach: SshReach { user: "root".into(), host: "box".into(), port: 22, key_path: None }, options: vec![] };
    let held = hold_back_forward(&carried, 4640, 4640, spawn);
    for step in want["steps"].as_array().unwrap() {
        if let Some(t) = step.get("out").and_then(Value::as_str) {
            fake.stdout.as_mut().unwrap().write_all(t.as_bytes()).await.unwrap();
        } else if let Some(t) = step.get("err").and_then(Value::as_str) {
            fake.stderr.as_mut().unwrap().write_all(t.as_bytes()).await.unwrap();
        } else {
            let end = if let Some(f) = step.get("fail").and_then(Value::as_str) {
                ChildEnd::Failed(f.to_string())
            } else if let Some(code) = step["exit"]["code"].as_i64() {
                ChildEnd::Exited(code)
            } else {
                ChildEnd::Signalled(step["exit"]["signal"].as_str().unwrap().to_string())
            };
            fake.stdout.take();
            fake.stderr.take();
            fake.end.take().unwrap().send(end).unwrap();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let up = match within(held.up()).await {
        Ok(()) => "up".to_string(),
        Err(words) => format!("rejected: {words}"),
    };
    assert_eq!(short(&up), want["up"], "up");
    assert_eq!(short(&within(held.ended()).await), want["ended"], "ended");
}

#[tokio::test]
async fn the_cut_keeps_a_lines_head() {
    forward("the_cut_keeps_a_lines_head").await;
}

#[tokio::test]
async fn only_the_last_50_stderr_lines_are_kept() {
    forward("only_the_last_50_stderr_lines_are_kept").await;
}

#[tokio::test]
async fn the_last_line_client_words_keeps_is_the_words() {
    forward("the_last_line_client_words_keeps_is_the_words").await;
}

#[tokio::test]
async fn up_on_a_line_written_in_two_pieces_and_padded() {
    forward("up_on_a_line_written_in_two_pieces_and_padded").await;
}

#[tokio::test]
async fn the_up_word_with_more_on_its_line_is_not_up() {
    forward("the_up_word_with_more_on_its_line_is_not_up").await;
}

#[tokio::test]
async fn an_exit_with_code_zero_says_so() {
    forward("an_exit_with_code_zero_says_so").await;
}

#[tokio::test]
async fn a_child_that_fails_to_start_says_the_error() {
    forward("a_child_that_fails_to_start_says_the_error").await;
}

#[tokio::test]
async fn one_trailing_cr_goes_with_the_line_and_box_word_strips_the_rest() {
    forward("one_trailing_cr_goes_with_the_line_and_box_word_strips_the_rest").await;
}

#[tokio::test]
async fn a_stderr_tail_with_no_newline_is_never_read() {
    forward("a_stderr_tail_with_no_newline_is_never_read").await;
}

#[tokio::test]
async fn a_flood_without_a_newline_keeps_only_its_last_4096_units() {
    forward("a_flood_without_a_newline_keeps_only_its_last_4096_units").await;
}

/// `/tmp/wsp-run/<12 hex>` with the hex as `X`.
fn masked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("/tmp/wsp-run/") {
        out.push_str(&rest[..i + 13]);
        rest = &rest[i + 13..];
        if rest.len() >= 12 && rest[..12].chars().all(|c| c.is_ascii_hexdigit()) {
            out.push('X');
            rest = &rest[12..];
        }
    }
    out.push_str(rest);
    out
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The scene's polls as the oracle wrote them; the full-chunk scene's first poll, shortened in the file, is rebuilt.
fn polls_of(want: &Value) -> Vec<Result<String, String>> {
    want["polls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| match p {
            Value::String(s) => Ok(s.clone()),
            Value::Object(o) if o.contains_key("reject") => Err(o["reject"].as_str().unwrap().to_string()),
            Value::Object(_) => Ok(format!("WSP_POLL\n0\n{}\n\ndown\nWSP_POLL_END\n", b64(&vec![b'a'; 262_144]))),
            _ => unreachable!(),
        })
        .collect()
}

async fn run(name: &str) {
    let want = find("runs", name);
    let polls = polls_of(&want);
    let delay = want["opts"]["launchDelayMs"].as_u64();
    let at = Arc::new(Mutex::new(0usize));
    let machine = ScriptedMachine::with_future(
        "m1",
        Arc::new(move |cmd: &str, _o: &ExecOptions| {
            let cmd = cmd.to_string();
            let polls = polls.clone();
            let at = at.clone();
            async move {
                if cmd.contains("echo WSP_LAUNCHED") {
                    if let Some(ms) = delay {
                        tokio::time::sleep(Duration::from_millis(ms)).await;
                    }
                    return Ok(res(0, "WSP_LAUNCHED\n", ""));
                }
                if cmd.contains("echo WSP_POLL\n") {
                    let mut i = at.lock().unwrap();
                    let next = polls[(*i).min(polls.len() - 1)].clone();
                    *i += 1;
                    return next.map(|s| res(0, &s, "")).map_err(|m| plain(&m));
                }
                Ok(res(0, "", ""))
            }
            .boxed()
        }),
    );
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let l = lines.clone();
    let opts = RunOptions {
        deadline_ms: want["opts"]["deadlineMs"].as_u64().unwrap_or(60_000),
        poll_ms: Some(want["opts"]["pollMs"].as_u64().unwrap_or(1)),
        on_line: Some(Arc::new(move |line: &str| l.lock().unwrap().push(line.to_string()))),
        ..Default::default()
    };
    let got = within(exec_detached_in(machine.as_ref(), "echo hi", opts, "/tmp/wsp-run")).await;
    let calls: Vec<String> = machine.cmds().iter().map(|c| masked(c)).collect();
    let want_calls: Vec<String> = want["calls"].as_array().unwrap().iter().map(|c| masked(c.as_str().unwrap())).collect();
    assert_eq!(calls, want_calls, "commands");
    let lines: Vec<Value> = lines.lock().unwrap().iter().map(|l| short(l)).collect();
    assert_eq!(json!(lines), want["lines"], "lines handed over");
    match got {
        Ok(r) => assert_eq!(json!({ "exitCode": r.exit_code, "stdout": short(&r.stdout), "stderr": short(&r.stderr) }), want["result"]),
        Err(e) => assert_eq!(e.to_string(), want["err"].as_str().unwrap()),
    }
}

#[tokio::test]
async fn an_exit_in_hex_is_a_number() {
    run("an_exit_in_hex_is_a_number").await;
}

#[tokio::test]
async fn an_exit_with_a_fraction_is_minus_one_with_the_note() {
    run("an_exit_with_a_fraction_is_minus_one_with_the_note").await;
}

#[tokio::test]
async fn lines_go_stdout_then_stderr_per_poll_and_the_tails_at_the_end() {
    run("lines_go_stdout_then_stderr_per_poll_and_the_tails_at_the_end").await;
}

#[tokio::test]
async fn a_blank_line_is_not_handed_over_and_a_cr_is_kept() {
    run("a_blank_line_is_not_handed_over_and_a_cr_is_kept").await;
}

#[tokio::test]
async fn a_full_chunk_with_an_exit_reads_again_before_the_exit() {
    run("a_full_chunk_with_an_exit_reads_again_before_the_exit").await;
}

#[tokio::test]
async fn past_the_deadline_the_last_polls_exit_is_ignored() {
    run("past_the_deadline_the_last_polls_exit_is_ignored").await;
}

#[tokio::test]
async fn a_block_closed_late_is_no_answer() {
    run("a_block_closed_late_is_no_answer").await;
}

#[tokio::test]
async fn the_streams_decode_as_node_decodes_base64() {
    run("the_streams_decode_as_node_decodes_base64").await;
}

#[tokio::test]
async fn a_poll_that_rejects_is_no_answer_and_is_asked_again() {
    run("a_poll_that_rejects_is_no_answer_and_is_asked_again").await;
}

#[tokio::test]
async fn a_utf8_char_split_across_polls_reaches_the_listener_whole() {
    run("a_utf8_char_split_across_polls_reaches_the_listener_whole").await;
}

/// The coordinator's ruling on #2446: a cut that lands inside a surrogate pair writes U+FFFD for the half left.
#[test]
fn a_cut_inside_an_emoji_writes_the_replacement_character_for_the_half() {
    use wsp_engine::ssh_backend::box_word;
    assert_eq!(box_word("ab😀", 3), "ab\u{FFFD}");
    assert_eq!(box_word("😀😀", 3), "😀\u{FFFD}");
    assert_eq!(box_word("\x1b😀", 1), "\u{FFFD}");
}
