// SPDX-License-Identifier: AGPL-3.0-only
//! The held forward and the detached run replaying `oracle/roads_scenes.oracle.ts`'s scenes against what the TS did
//! (spec/exec_run.md, rules R).
mod roads_common;

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use futures::FutureExt;
use roads_common::*;
use serde_json::{json, Value};
use wsp_engine::exec_detached::exec_detached_in;
use wsp_engine::machine::{ExecOptions, RunOptions};

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
            .boxed_local()
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
