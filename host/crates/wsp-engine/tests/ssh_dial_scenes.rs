// SPDX-License-Identifier: AGPL-3.0-only
//! The held forward and the detached run replaying `oracle/roads_scenes.oracle.ts`'s scenes against what the TS did
//! (spec/ssh_dial.md, rules D-H).
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

/// The coordinator's ruling on #2446: a cut that lands inside a surrogate pair writes U+FFFD for the half left.
#[test]
fn a_cut_inside_an_emoji_writes_the_replacement_character_for_the_half() {
    use wsp_engine::ssh_backend::box_word;
    assert_eq!(box_word("ab😀", 3), "ab\u{FFFD}");
    assert_eq!(box_word("😀😀", 3), "😀\u{FFFD}");
    assert_eq!(box_word("\x1b😀", 1), "\u{FFFD}");
}
