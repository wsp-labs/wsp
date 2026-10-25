// SPDX-License-Identifier: AGPL-3.0-only
//! An op is answered when some tape gets past a refusal of it: a reply that only says the socket may not ask, or a
//! close, holds no port to what the op does (441 of 446 place.join tapes end at 4401, 136 websocket tapes answer only
//! unauthorized, and none of 172 sessions.start requests starts a turn).

use serde_json::json;
use std::fs;
use std::process::Command;
use xtask::check::{check, record, Options};
use xtask::host::{Host, Replay};
use xtask::normalize::Facts;
use xtask::seed::Order;
use xtask::tape::Tape;

/// A host that refuses every op but auth, as a host whose sign-in the tape's next frame could not make does.
struct Refusing;

impl Host for Refusing {
    fn name(&self) -> String {
        "a refusing host".into()
    }

    fn replay(&self, tape: &Tape, _: &mut Order) -> Result<Replay, String> {
        let replies = tape
            .ins
            .iter()
            .map(|f| match f["op"].as_str() {
                Some("auth") => json!({ "id": f["id"], "ok": true }),
                _ => json!({ "id": f["id"], "ok": false, "error": "unauthorized" }),
            })
            .collect();
        Ok(Replay { replies, events: Vec::new(), facts: Facts::default() })
    }
}

#[test]
fn an_op_only_ever_refused_is_not_counted_as_answered() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().unwrap().success());
    let tape = dir.path().join("parity/tapes/sessions/start.tape");
    fs::create_dir_all(tape.parent().unwrap()).unwrap();
    fs::write(&tape, format!("{}\n{}\n", json!({ "in": { "id": 1, "op": "auth" } }), json!({ "in": { "id": 2, "op": "sessions.start" } })))
        .unwrap();
    let ops = vec!["auth".to_string(), "sessions.start".to_string()];
    record(dir.path(), &Refusing, &Options { jobs: 1, ..Options::default() }, &ops).unwrap();
    let report = check(dir.path(), &Refusing, &Options { jobs: 1, ..Options::default() }).unwrap();
    let said = report.render();
    assert!(!said.contains("2 of 2 host ops have a tape, 2 of them an answer"), "{said}");
}

#[test]
fn a_run_with_no_tape_through_the_command_line_or_the_tool_server_fails() {
    // The command line, the tools and the skill are one contract: taking out all 490 cli and 410 mcp
    // tapes must fail the run as taking out one op family's does.
    let dir = tempfile::tempdir().unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().unwrap().success());
    let tape = dir.path().join("parity/tapes/ws/auth.tape");
    fs::create_dir_all(tape.parent().unwrap()).unwrap();
    fs::write(&tape, format!("{}\n", json!({ "in": { "id": 1, "op": "auth" } }))).unwrap();
    let ops = vec!["auth".to_string()];
    record(dir.path(), &Refusing, &Options { jobs: 1, ..Options::default() }, &ops).unwrap();
    let report = check(dir.path(), &Refusing, &Options { jobs: 1, ..Options::default() }).unwrap();
    assert!(!report.ok(), "{}", report.render());
}

#[test]
fn recording_some_tapes_leaves_the_rest_marked_as_recorded_from_the_tree_before() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(p).status().unwrap().success());
    git(&["init", "-q"]);
    fs::create_dir_all(p.join("packages/host/src")).unwrap();
    fs::write(p.join("packages/host/src/answer.ts"), "export const answer = 1;\n").unwrap();
    for id in ["a", "b"] {
        let tape = p.join(format!("parity/tapes/ws/{id}.tape"));
        fs::create_dir_all(tape.parent().unwrap()).unwrap();
        fs::write(&tape, format!("{}\n", json!({ "in": { "id": 1, "op": "auth" } }))).unwrap();
    }
    let ops = vec!["auth".to_string()];
    record(p, &Refusing, &Options { jobs: 1, ..Options::default() }, &ops).unwrap();
    fs::write(p.join("packages/host/src/answer.ts"), "export const answer = 2;\n").unwrap();
    record(p, &Refusing, &Options { jobs: 1, only: Some("ws/a".into()), ..Options::default() }, &ops).unwrap();
    let said = check(p, &Refusing, &Options { jobs: 1, ..Options::default() }).unwrap().render();
    assert!(said.contains("run cargo xtask parity record"), "ws/b was recorded from the tree before the change: {said}");
}

/// A host that takes every op but auth and does nothing, as the TS host answers an interrupt of a turn it never held.
struct Nothing;

impl Host for Nothing {
    fn name(&self) -> String {
        "a host that holds nothing".into()
    }

    fn replay(&self, tape: &Tape, _: &mut Order) -> Result<Replay, String> {
        let replies = tape
            .ins
            .iter()
            .map(|f| match f["op"].as_str() {
                Some("auth") => json!({ "id": f["id"], "ok": true }),
                _ => json!({ "id": f["id"], "ok": true, "outcome": "not-found" }),
            })
            .collect();
        Ok(Replay { replies, events: Vec::new(), facts: Facts::default() })
    }
}

#[test]
fn an_op_only_ever_answered_with_nothing_done_is_not_counted_as_answered() {
    // sessions.interrupt, steer, answer and rename answer only "not-found" on every tape; devices.revoke, places.remove
    // and appProcs.offer only revoked, removed or wanted false.
    let dir = tempfile::tempdir().unwrap();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().unwrap().success());
    let tape = dir.path().join("parity/tapes/sessions/interrupt.tape");
    fs::create_dir_all(tape.parent().unwrap()).unwrap();
    fs::write(
        &tape,
        format!("{}\n{}\n", json!({ "in": { "id": 1, "op": "auth" } }), json!({ "in": { "id": 2, "op": "sessions.interrupt" } })),
    )
    .unwrap();
    let ops = vec!["auth".to_string(), "sessions.interrupt".to_string()];
    record(dir.path(), &Nothing, &Options { jobs: 1, ..Options::default() }, &ops).unwrap();
    let said = check(dir.path(), &Nothing, &Options { jobs: 1, ..Options::default() }).unwrap().render();
    assert!(said.contains("2 of 2 host ops have a tape, 1 of them an answer"), "{said}");
}
