// SPDX-License-Identifier: AGPL-3.0-only
//! The failing tests of the Box and detached-exec review (findings 1 to 15 and the Error-reason guess), each on a scripted
//! Boat API built here. What the TS answered is written in each test as plain values, with the TS line and the line the
//! reviewer's probe printed it on (`review_eng_box.rs` holds the same scenes as gzip fixtures and the clean sweeps).
mod common;

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::fake_box::*;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_engine::box_backend::{box_labels, daemon_exits_of, BoxBudgetsAsk, BoxMachine, Timer};
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::{EngineError, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::machine::{ExecOptions, Machine, MachineKind, MachineLife, MachineSpec, MachineState, RunOptions, SnapshotOptions};

const ID: &str = "bx_tumrjngm";

fn path(suffix: &str) -> String {
    format!("/sandboxes/{ID}{suffix}")
}

fn reply(status: u16, body: Value) -> Answer {
    Answer::Reply(Reply { status, body })
}

fn ip_limit(retry_after: Value) -> Answer {
    reply(
        429,
        json!({ "ok": false, "code": "IP_RATE_LIMIT_EXCEEDED", "message": "Too many requests", "requestId": "req_1", "retryAfter": retry_after }),
    )
}

fn ok() -> Answer {
    reply(200, json!({ "ok": true }))
}

fn cmd(stdout: &str, exit: i64) -> Answer {
    command(stdout, Some(exit), json!({}))
}

/// A box row as the listing carries it, named for its workspace.
fn row(id: &str) -> Value {
    box_view(id, "running", json!({ "name": format!("wsp;w={id}") }))
}

fn listing(rows: Vec<Value>) -> Answer {
    reply(200, json!({ "sandboxes": rows, "pageInfo": { "nextCursor": null } }))
}

fn spec(extra: Value) -> MachineSpec {
    let mut v = json!({ "kind": "sandbox", "idempotencyKey": "key-1" });
    for (k, val) in extra.as_object().unwrap() {
        v[k] = val.clone();
    }
    serde_json::from_value(v).unwrap()
}

/// The routes a create needs: Boat answers provisioning, the box reads ready, one command proves the road.
fn create_routes(api: &Arc<FakeBox>, post: Answer) {
    api.on("POST", "/sandboxes", vec![post]);
    api.on("GET", "/sandboxes/bx_new", vec![info("bx_new", "ready", json!({ "name": "wsp" }))]);
    api.on("POST", "/sandboxes/bx_new/commands", vec![cmd("", 0)]);
    api.on("PATCH", "/sandboxes/bx_new", vec![ok()]);
    api.on("DELETE", "/sandboxes/bx_new", vec![ok()]);
}

fn provisioning(extra: Value) -> Answer {
    let mut body = json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_new", "provisioning", json!({ "name": "wsp" })) });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    reply(201, body)
}

// 1. A cancel with a reason that lands in a request-limit wait.
// TS box-backend.ts:642 `opts.signal?.throwIfAborted()` throws the reason; Rust request.rs:510-512 answers `aborted()`.
// Probe lines cancel_ip_limit_wait_string / cancel_ip_limit_wait_error: err.message "stop now" / "the person stopped waiting",
// sleeps [2000], warns [.. "POST /sandboxes/bx_tumrjngm/resume met Boat's limit of 1,000 requests a minute from one address; asking again in 2s, try 2 of 3"].
#[tokio::test]
async fn a_cancel_with_a_reason_that_lands_in_a_request_limit_wait_answers_that_reason() {
    let mut bad = Vec::new();
    for reason in ["stop now", "the person stopped waiting"] {
        let stop = Cancel::new();
        let firing = stop.clone();
        let api = FakeBox::new()
            .on(
                "POST",
                &path("/resume"),
                vec![with(move |_| {
                    firing.cancel(Some(reason));
                    ip_limit(json!(2))
                })],
            )
            .on("GET", &path(""), vec![info(ID, "running", json!({}))])
            .on("POST", &path("/commands"), vec![cmd("", 0)]);
        let (on, machine) = machine_on(&api, ID);
        let e = machine.resume(Some(&stop)).await.unwrap_err();
        assert_eq!(on.clock.sleeps(), vec![2000]);
        if e.to_string() != reason {
            bad.push(format!("cancel reason {reason:?}: the TS answers the reason itself, the Rust {:?}", e.to_string()));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// 2. read_bytes on a size no file has. TS box-backend.ts:1408 `Array.from({ length: pieces })` throws RangeError; Rust
// machine.rs:624-625 collects `0..pieces`. Probe line read_stat_huge_1e300: err.message "Invalid array length" (so too for 1e21,
// 99999999999999999999 and 18014398509481984000).
async fn read_after_stat_says(size: &str) -> Result<Vec<u8>, EngineError> {
    let api = FakeBox::new().on("POST", &path("/commands"), vec![cmd(size, 0), cmd("", 0)]).on(
        "GET",
        &path("/files"),
        vec![reply(200, json!({ "content": "AQID" }))],
    );
    let (_on, machine) = machine_on(&api, ID);
    machine.read_bytes("/root/x", None, None).await
}

#[tokio::test]
async fn a_size_the_machine_prints_that_no_file_has_is_a_failure_not_a_panic() {
    let r = AssertUnwindSafe(read_after_stat_says("1e300")).catch_unwind().await;
    let r = r.expect("read_bytes panicked (capacity overflow) on a stat answer of 1e300");
    assert!(r.is_err());
}

#[tokio::test]
async fn a_size_the_machine_prints_that_would_need_trillions_of_pieces_is_a_failure_not_an_abort() {
    assert!(read_after_stat_says("99999999999999999999").await.is_err());
}

#[tokio::test]
async fn a_size_of_1e21_the_machine_prints_is_a_failure_not_an_abort() {
    assert!(read_after_stat_says("1e21").await.is_err());
}

// 3. File content read as Node reads base64. TS box-backend.ts:1398 `Buffer.from(read.content ?? "", "base64")`; Rust machine.rs:374-376
// `STANDARD.decode`. Probe lines read_content_"…" (the straight road, size 3, path /tmp/x). Each input here fails in Rust
// (Invalid padding, Invalid symbol, Invalid input length, Invalid last symbol).
#[tokio::test]
async fn file_content_is_read_as_node_reads_base64() {
    let table: &[(&str, &[u8])] = &[
        ("AQI", &[1, 2]),
        ("AQ", &[1]),
        ("AQ=", &[1]),
        ("AQID\n", &[1, 2, 3]),
        ("AQ ID", &[1, 2, 3]),
        ("AQID\r\nAQID", &[1, 2, 3, 1, 2, 3]),
        ("-_8=", &[251, 255]),
        ("-_8", &[251, 255]),
        ("not*base64!", &[158, 139, 91, 106, 199, 186]),
        ("ünï", &[]),
        ("AQ==AQ==", &[1]),
        ("====", &[]),
        ("=AQID", &[]),
        ("AQIDA", &[1, 2, 3]),
        ("AQIDAQ", &[1, 2, 3, 1]),
        ("A", &[]),
        ("AB", &[0]),
        ("ABC=", &[0, 16]),
        ("ABD=", &[0, 16]),
        ("AB==", &[0]),
        ("A===", &[]),
    ];
    let mut bad = Vec::new();
    for (content, want) in table {
        let api = FakeBox::new().on("GET", &path("/files"), vec![reply(200, json!({ "ok": true, "content": content }))]);
        let (_on, machine) = machine_on(&api, ID);
        let got = machine.read_bytes("/tmp/x", Some(3), None).await;
        if got.as_deref().ok() != Some(*want) {
            bad.push(format!("{content:?}: the TS reads {want:?}, the Rust {got:?}"));
        }
    }
    assert!(bad.is_empty(), "{} of {} inputs differ:\n{}", bad.len(), table.len(), bad.join("\n"));
}

// 4. A cut inside a surrogate pair. TS box-backend.ts:1351 (`slice(-200)`) and :1103 (`slice(0, 120)`) keep a lone surrogate, which
// every UTF-8 sink writes as U+FFFD; Rust machine.rs:350/:435 and :88 (`utf16::tail`, `utf16::cut`) drop the half. The roads
// ruling is U+FFFD. Probe lines fw_stderr_with_emoji_split (warns) and daemon_exits (answer 2).
#[tokio::test]
async fn a_cut_inside_a_surrogate_pair_leaves_the_replacement_character_in_the_firewalls_and_the_daemons_words() {
    let mut bad = Vec::new();
    let stderr = format!("😀{}", "b".repeat(199));
    let api = FakeBox::new().on("POST", &path("/commands"), vec![cmd("", 1), command("", Some(1), json!({ "stderr": stderr }))]).on(
        "POST",
        &path("/host"),
        vec![reply(200, json!({ "ok": true, "url": "https://3000-x.on.boat.dev/?_token=t9" }))],
    );
    let on = backend_on(&api, "trial", BoxBudgetsAsk { poll_ms: Some(1000), restore_ms: Some(20_000), ..Default::default() });
    let machine = BoxMachine::new(on.backend.clone(), ID, MachineKind::Sandbox, None, None, false);
    machine.preview_url(3000).await.unwrap();
    let want =
        format!("{ID}: the firewall did not hold a rule for port 3000 (\u{fffd}{}); the route is minted without one", "b".repeat(199));
    if on.warned.lines() != vec![want.clone()] {
        bad.push(format!(
            "firewall warn: the TS keeps the half as U+FFFD ({} chars after the bracket), the Rust drops it ({:?} chars)",
            "200",
            on.warned.lines().first().map(|l| l.chars().count() as i64 - want.chars().count() as i64)
        ));
    }

    let stdout = format!("WSP_DAEMON_EXITS failed x 0\n{}\nshort\nshort\n  \nlast {}", "y".repeat(130), "😀".repeat(70));
    let api = FakeBox::new().on("POST", &path("/commands"), vec![cmd(&stdout, 0)]);
    let (_on, machine) = machine_on(&api, ID);
    let want = format!("{}…; short; last {}\u{fffd}…", "y".repeat(120), "😀".repeat(57));
    let got = machine.daemon_exits().await.unwrap();
    if got.as_deref() != Some(want.as_str()) {
        bad.push(format!(
            "daemon words: the TS ends `last …😀\u{fffd}…`, the Rust ends {:?}",
            got.map(|g| g.chars().rev().take(4).collect::<String>().chars().rev().collect::<String>())
        ));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// 5a. One malformed row fails the whole listing. TS box-backend.ts:932-958 reads every row it can; Rust request.rs:33 (BoxView) and
// fleet.rs:277 (`every_box`) are typed. Probe lines list_row_missing_state (ids bx_1, bx_2, bx_3, bx_2 running) and
// list_row_error_is_object (ids bx_1). A row with no `id` gives the TS a row with no id (not expressible in Rust).
#[tokio::test]
async fn one_malformed_box_row_does_not_fail_the_whole_listing() {
    let no_state = json!({ "id": "bx_2", "name": "wsp;w=bx_2" });
    let error_is_an_object = box_view("bx_1", "running", json!({ "name": "wsp;w=bx_1", "error": { "code": 1 } }));
    let cases: [(&str, Vec<Value>, Vec<&str>); 2] = [
        ("a row with no state", vec![row("bx_1"), no_state, row("bx_3")], vec!["bx_1", "bx_2", "bx_3"]),
        ("a row whose error is an object", vec![error_is_an_object], vec!["bx_1"]),
    ];
    let mut bad = Vec::new();
    for (what, rows, want) in cases {
        let api = FakeBox::new().on("GET", "/sandboxes", vec![listing(rows), listing(vec![])]);
        let on = backend_on(&api, "trial", Default::default());
        match on.backend.box_list(None).await {
            Ok(got) if got.iter().map(|r| r.id.as_str()).collect::<Vec<_>>() == want => {}
            other => bad.push(format!("{what}: the TS lists {want:?}, the Rust {:?}", other.map(|r| r.len()))),
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// 5b. The same for named snapshots. Rust request.rs:77 (NamedSnapshotView) needs name, status and createdAt. Probe lines
// snapshots_named_row_missing_created_at (listSnapshots ids a, b; templates a ready) and snapshots_named_row_missing_status
// (templates: a building createdAt "x", b ready).
#[tokio::test]
async fn a_snapshot_row_missing_a_field_still_lists() {
    let mut bad = Vec::new();
    let api = FakeBox::new()
        .on(
            "GET",
            "/named-snapshots",
            vec![reply(200, json!({ "snapshots": [{ "name": "a", "status": "ready" }, named("b", "ready", json!({}))] }))],
        )
        .on("GET", "/snapshots", vec![reply(200, json!({ "snapshots": [] }))]);
    let on = backend_on(&api, "trial", Default::default());
    match on.backend.box_list_snapshots(None).await {
        Ok(rows) if rows.iter().map(|s| s.id.as_str()).collect::<Vec<_>>() == ["a", "b"] => {}
        other => bad.push(format!("no createdAt on the first row: the TS lists ids [a, b], the Rust {:?}", other.map(|r| r.len()))),
    }

    let api = FakeBox::new().on(
        "GET",
        "/named-snapshots",
        vec![reply(200, json!({ "snapshots": [{ "name": "a", "createdAt": "x" }, named("b", "ready", json!({}))] }))],
    );
    let on = backend_on(&api, "trial", Default::default());
    match on.backend.box_list_templates().await {
        Ok(t) if serde_json::to_value(&t).unwrap()[0]["status"] == "building" => {}
        other => {
            bad.push(format!("no status on the first row: the TS lists template a as building, the Rust {:?}", other.map(|t| t.len())))
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// 5c. A read with no state is waited on in the TS. TS box-backend.ts:757-769: `STATE_MAP[undefined]` is not running, so settle polls
// to its budget. Probe line resume_state_missing: err.message "resume of bx_tumrjngm still reads undefined after 0 s", sleeps [10 x 10]
// (eleven reads). Probe line snapshot_created_at_missing_ready: the snapshot read as ready without createdAt answers "wsp-v1".
#[tokio::test]
async fn a_read_without_a_state_or_a_created_at_is_waited_on_as_the_ts_waits() {
    let mut bad = Vec::new();
    let api = FakeBox::new()
        .on("POST", &path("/resume"), vec![ok()])
        .on("GET", &path(""), vec![reply(200, json!({ "sandbox": { "id": ID } }))])
        .on("POST", &path("/commands"), vec![cmd("", 0)]);
    let (on, machine) = machine_on(&api, ID);
    let e = machine.resume(None).await.unwrap_err();
    let reads = api.calls().iter().filter(|c| **c == format!("GET {}", path(""))).count();
    if e.to_string() != "resume of bx_tumrjngm still reads undefined after 0 s" || on.clock.sleeps() != vec![10; 10] || reads != 11 {
        bad.push(format!(
            "no state: the TS polls 11 reads and 10 sleeps of 10 ms and says \"resume of bx_tumrjngm still reads undefined after 0 s\"; the Rust made {reads} read(s), {} sleep(s) and says {:?}",
            on.clock.sleeps().len(),
            e.to_string()
        ));
    }

    let api = FakeBox::new().on("POST", "/named-snapshots", vec![ok()]).on(
        "GET",
        "/named-snapshots/wsp-v1",
        vec![reply(200, json!({ "snapshot": { "name": "wsp-v1", "status": "ready" } }))],
    );
    let (_on, machine) = machine_on(&api, ID);
    let saved = machine.snapshot("wsp-v1", MachineLife { first_life: false }, SnapshotOptions::default()).await;
    if saved.as_deref().ok() != Some("wsp-v1") {
        bad.push(format!("ready without createdAt: the TS answers \"wsp-v1\", the Rust {saved:?}"));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// 5d (NIT). Fields of the wrong type pass through in the TS. Probe lines exec_exit_code_float15 (exitCode 1.5), exec_timed_out_string_true
// (exitCode -1) and pause_running_after_stop_status_not_string (StopRefused said "it reported no snapshot attempt, and the last that
// completed was none"). Other wrong types in the probes (stdout 5, exit code "1", operation id 7, vcpu "2") have no Rust value to hold them.
#[tokio::test]
async fn a_field_of_the_wrong_type_is_read_as_the_ts_reads_it() {
    let mut bad = Vec::new();
    let exec = |body: Value| async move {
        let api = FakeBox::new().on("POST", &path("/commands"), vec![reply(200, body)]);
        let (_on, machine) = machine_on(&api, ID);
        machine.exec("echo hi", ExecOptions::default()).await
    };
    match exec(json!({ "exitCode": 1.5, "stdout": "out", "stderr": "err" })).await {
        Ok(r) if r.exit_code.as_f64() == 1.5 => {}
        other => bad.push(format!("exitCode 1.5: the TS answers exit 1.5, the Rust {other:?}")),
    }
    match exec(json!({ "exitCode": null, "timedOut": "true" })).await {
        Ok(r) if r.exit_code.get() == -1 => {}
        other => bad.push(format!("timedOut \"true\": the TS answers exit -1, the Rust {other:?}")),
    }
    let api = FakeBox::new()
        .on(
            "GET",
            &path(""),
            vec![info(ID, "running", json!({})), info(ID, "running", json!({ "lastSnapshotStatus": 5, "snapshotCompletedAt": 7 }))],
        )
        .on("POST", &path("/stop"), vec![ok()]);
    let (_on, machine) = machine_on(&api, ID);
    let said = "it reported no snapshot attempt, and the last that completed was none";
    match machine.pause(None).await {
        Err(EngineError::StopRefused { said: got, .. }) if got == said => {}
        other => bad.push(format!("lastSnapshotStatus 5: the TS refuses the stop saying {said:?}, the Rust {other:?}")),
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// 6. The state guards survive a dropped future. TS box-backend.ts:1019-1037 and :677-688: the promise runs on and `finally` clears the
// guard. Rust fleet.rs:390/:393 (`backstops_sending`) and request.rs:568 (`limits_asked`). Probe line drop_backstop_abandoned_then_asked_again:
// PATCH bodies {"ttlSeconds":1000} then {"ttlSeconds":970}, last timer {until 2000000, spanMs 970000}. Probe line
// drop_limits_abandoned_then_asked_again: the second call answers the limits (trial false, sizes small default large) after a second GET /limits.
#[tokio::test]
async fn a_backstop_the_caller_stopped_waiting_on_does_not_block_the_next_one() {
    let api = FakeBox::new().on("PATCH", &path(""), vec![Answer::Hang, ok()]);
    let on = backend_on(&api, "paid", BoxBudgetsAsk { backstop_ms: Some(40), ..Default::default() });
    let gave_up = tokio::time::timeout(Duration::from_millis(5), on.backend.box_backstop(ID, 2_000_000, false)).await;
    assert!(gave_up.is_err());
    tokio::time::sleep(Duration::from_millis(120)).await;
    on.clock.advance(30_000);
    on.backend.box_backstop(ID, 2_000_000, false).await.unwrap();
    let patches: Vec<Option<String>> = api.seen().into_iter().filter(|s| s.method == "PATCH").map(|s| s.raw).collect();
    assert_eq!(patches, vec![Some(r#"{"ttlSeconds":1000}"#.to_string()), Some(r#"{"ttlSeconds":970}"#.to_string())]);
    assert_eq!(on.backend.last_timer(ID), Some(Timer { until: 2_000_000, span_ms: 970_000 }));
}

#[tokio::test]
async fn a_limits_read_the_caller_stopped_waiting_on_does_not_answer_the_next_call() {
    let api = FakeBox::new().on("GET", "/limits", vec![Answer::Hang, limits("paid")]);
    let on = backend_with(api.clone() as Arc<dyn Fetch>, BoxBudgetsAsk { read_ms: Some(40), ..Default::default() });
    let gave_up = tokio::time::timeout(Duration::from_millis(5), on.backend.limits()).await;
    assert!(gave_up.is_err());
    tokio::time::sleep(Duration::from_millis(120)).await;
    let second = on.backend.limits().await;
    assert_eq!(api.calls(), vec!["GET /limits", "GET /limits"], "the TS reads again");
    assert!(second.as_ref().is_ok_and(|l| !l.trial), "the TS answers the paid account; the Rust answers {:?}", second.map(|l| l.trial));
}

// 7 (NIT). A retryAfter under a millisecond. TS box-backend.ts:448 passes `s * 1000` on as it is. Rust request.rs:195 truncates to whole
// ms first. Probe line ip_limit_topTiny (retryAfter 0.0004): sleeps [0.4] (a u64 sleep cannot say that) and the second warn ends
// "asking again in 1s, try 2 of 3".
#[tokio::test]
async fn a_retry_after_under_a_millisecond_is_waited_and_said_as_the_ts_says_it() {
    let api = FakeBox::new().on("GET", &path(""), vec![ip_limit(json!(0.0004)), info(ID, "running", json!({}))]);
    let (on, machine) = machine_on(&api, ID);
    assert_eq!(machine.state().await.unwrap(), MachineState::Running);
    assert_eq!(
        on.warned.lines()[1],
        "GET /sandboxes/bx_tumrjngm met Boat's limit of 1,000 requests a minute from one address; asking again in 1s, try 2 of 3"
    );
}

// 8 (NIT). An exit file holding an integer past 2^63. TS exec-detached.ts:387-388 keeps `Number(poll.exit)`; Rust run.rs:245 `code as i64`
// saturates. Probe lines run_exit_file_"99999999999999999999" / "1e300" / "1e21": exitCode 100000000000000000000 / 1e+300 / 1e+21.
#[tokio::test]
async fn an_exit_file_holding_an_integer_past_two_to_the_63_answers_that_number() {
    let mut bad = Vec::new();
    for (exit, want) in [("99999999999999999999", 1e20), ("1e300", 1e300), ("1e21", 1e21)] {
        let poll = format!("WSP_POLL\n{exit}\neA==\n\ndown\nWSP_POLL_END\n");
        let api = FakeBox::new().on("POST", &path("/commands"), vec![cmd("WSP_LAUNCHED\n", 0), cmd(&poll, 0), cmd("", 0)]);
        let (_on, machine) = machine_on(&api, ID);
        let got = machine.run("echo hi", RunOptions { deadline_ms: 5000, ..Default::default() }).await.unwrap();
        if got.exit_code.as_f64() != want {
            bad.push(format!("exit file {exit:?}: the TS answers {want:e}, the Rust {:e}", got.exit_code.as_f64()));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// 9 (NIT). An idle window past an i64. TS box-backend.ts:795 adds in floating point; Rust fleet.rs:83 and :149 `now + ms.0 as i64`.
// Probe line create_idle_1e19_paid_overflows_an_i64: the create goes through and posts
// {"type":"small","ttlSeconds":10000000000000000,"noEnv":true,"name":"wsp"}.
#[tokio::test]
async fn an_idle_window_past_an_i64_is_not_a_panic() {
    let api = FakeBox::new();
    create_routes(&api, provisioning(json!({})));
    let on = backend_on(&api, "paid", Default::default());
    let r = AssertUnwindSafe(on.backend.box_create(spec(json!({ "idleTimeoutMs": 1e19 })), None, None)).catch_unwind().await;
    r.expect("box_create panicked (attempt to add with overflow) on an idle window of 1e19 ms").unwrap();
    let posted = api.took("POST", "/sandboxes").unwrap().raw;
    assert_eq!(posted.as_deref(), Some(r#"{"type":"small","ttlSeconds":10000000000000000,"noEnv":true,"name":"wsp"}"#));
}

// 9 (NIT). A fractional idle window. TS ttlSeconds `Math.ceil(1000.5 / 1000)` is 2; Rust fleet.rs:83 truncates to 1000 ms first.
// Probe lines create_idle_1000.5_trial and create_idle_1000.5_paid: {"type":"small","ttlSeconds":2,"noEnv":true,"name":"wsp"}.
#[tokio::test]
async fn a_fractional_idle_window_is_rounded_up_as_the_ts_rounds_it() {
    for tier in ["trial", "paid"] {
        let api = FakeBox::new();
        create_routes(&api, provisioning(json!({})));
        let on = backend_on(&api, tier, Default::default());
        on.backend.box_create(spec(json!({ "idleTimeoutMs": 1000.5 })), None, None).await.unwrap();
        let posted = api.took("POST", "/sandboxes").unwrap().raw;
        assert_eq!(posted.as_deref(), Some(r#"{"type":"small","ttlSeconds":2,"noEnv":true,"name":"wsp"}"#), "{tier}");
    }
}

// 10 (NIT, sweep: 2,000 random names, every difference had `__proto__` or an integer-like key). TS box-backend.ts:328 assigns into an
// object: integer-like keys come first ascending and `__proto__` is no key. Rust plan.rs:375-391 fills an IndexMap in order.
// Probe (patch.probe.ts) Object.keys(boxLabels(name)):
#[test]
fn labels_read_back_in_javascripts_key_order() {
    let table: &[(&str, &[&str])] = &[
        ("wsp;5=x;1=y;__proto__=p;constructor=c", &["1", "5", "wsp", "constructor"]),
        ("wsp;10=a;2=b;zeta=z", &["2", "10", "wsp", "zeta"]),
        ("wsp;__proto__=p;o=1", &["wsp", "wsp-owner"]),
        ("wsp;b=2;1=a;a=1", &["1", "wsp", "wsp-builder", "a"]),
    ];
    let mut bad = Vec::new();
    for (name, want) in table {
        let got: Vec<String> = box_labels(Some(name)).unwrap().keys().cloned().collect();
        if got != *want {
            bad.push(format!("{name:?}: the TS keys {want:?}, the Rust {got:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// 11 (NIT). A numeric next cursor. TS box-backend.ts:925 `nextCursor(page) ?? undefined` follows 5 (`cursor=5`); Rust fleet.rs:266 reads
// strings only. Probe line list_cursor_number: GETs [limit=200], [limit=200, cursor=5], [state=archived, limit=200]; ids bx_1, bx_2, bx_3.
#[tokio::test]
async fn a_next_cursor_that_is_a_number_is_followed() {
    let page = |id: &str, next: Value| reply(200, json!({ "sandboxes": [row(id)], "pageInfo": { "nextCursor": next } }));
    let api = FakeBox::new().on("GET", "/sandboxes", vec![page("bx_1", json!(5)), page("bx_2", Value::Null), page("bx_3", Value::Null)]);
    let on = backend_on(&api, "trial", Default::default());
    let ids: Vec<String> = on.backend.box_list(None).await.unwrap().into_iter().map(|r| r.id).collect();
    let cursors: Vec<Option<String>> =
        api.seen().iter().filter(|s| s.path == "/sandboxes").map(|s| s.query_get("cursor").map(str::to_string)).collect();
    assert_eq!(cursors, vec![None, Some("5".to_string()), None]);
    assert_eq!(ids, vec!["bx_1", "bx_2", "bx_3"]);
}

// 12 (NIT). A create answered with `status: null`. TS box-backend.ts:846 `res.status !== undefined && res.status !== "provisioning"` is true
// for null: replayed. Rust fleet.rs:160 reads null as no status. Probe line create_status_null: replayed true.
#[tokio::test]
async fn a_create_answered_with_a_null_status_reads_as_replayed() {
    let api = FakeBox::new();
    create_routes(&api, provisioning(json!({ "status": null })));
    let on = backend_on(&api, "trial", Default::default());
    let machine = on.backend.box_create(spec(json!({})), None, None).await.unwrap();
    assert!(machine.replayed);
}

// 13 (NIT). A refusal whose body is JSON null. TS box-backend.ts:633 throws a TypeError out of `request` after one send ("Cannot read
// properties of null (reading 'code')": no status, no retry, no warn). The ruling on Solari's null refusal body (an error with no status
// after one send) holds for Box. Rust request.rs:486 reads an empty envelope and sends the 502 again. Probe line refusal_body_null_502.
#[tokio::test]
async fn a_refusal_whose_body_is_json_null_is_one_send_and_an_error_with_no_status() {
    let api = FakeBox::new().on("GET", &path(""), vec![reply(502, Value::Null), info(ID, "running", json!({}))]);
    let (_on, machine) = machine_on(&api, ID);
    let answer = machine.state().await;
    let sends = api.calls().iter().filter(|c| **c == format!("GET {}", path(""))).count();
    assert_eq!(sends, 1, "the 502 with a null body was sent again");
    assert!(answer.is_err_and(|e| e.status().is_none()));
}

/// Answers the first request with these bytes as a 429; every later one is the fake's.
struct RawFirst {
    inner: Arc<FakeBox>,
    raw: &'static str,
    spent: AtomicBool,
}

#[async_trait(?Send)]
impl Fetch for RawFirst {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        if self.spent.swap(true, Ordering::SeqCst) {
            return self.inner.fetch(req).await;
        }
        self.inner.seen.lock().unwrap().push(FakeBox::seen_of(&req));
        Ok(FetchResponse { status: 429, headers: vec![], body: self.raw.as_bytes().to_vec() })
    }
}

// 14 (NIT). A number past f64 in a refusal body. TS JSON.parse reads 1e400 as Infinity and keeps the envelope; Rust request.rs:486 serde
// refuses the whole body, so code and wait are lost. Probe line ip_limit_retryAfter_overflow_literal: state "running", sleeps [60000], second
// warn "GET /sandboxes/bx_tumrjngm met Boat's limit of 1,000 requests a minute from one address; asking again in 60s, try 2 of 3".
#[tokio::test]
async fn a_number_past_f64_in_a_refusal_body_does_not_hide_its_code() {
    let inner = FakeBox::new().on("GET", &path(""), vec![info(ID, "running", json!({}))]);
    let raw = RawFirst { inner, raw: r#"{"code":"IP_RATE_LIMIT_EXCEEDED","retryAfter":1e400}"#, spent: AtomicBool::new(false) };
    let on = backend_with(Arc::new(raw), Default::default());
    let machine = BoxMachine::new(on.backend.clone(), ID, MachineKind::Sandbox, None, None, false);
    assert_eq!(machine.state().await.unwrap(), MachineState::Running);
    assert_eq!(on.clock.sleeps(), vec![60_000]);
}

// 15 (NIT, sweep: the unit's words split on JavaScript's /\s+/). Rust machine.rs:92 `split_whitespace` is Unicode White_Space: U+FEFF is
// not one, U+0085 is. Probe (patch.probe.ts) daemonExitsOf(...):
#[test]
fn daemon_exits_of_splits_the_words_on_javascripts_whitespace() {
    let table: &[(&str, &str)] = &[
        ("WSP_DAEMON_EXITS auto-restart\u{feff}exit-code 3", "exit status 3"),
        ("WSP_DAEMON_EXITS auto-restart exit-code\u{85}7", "exit-code\u{85}7"),
    ];
    let mut bad = Vec::new();
    for (stdout, want) in table {
        let got = daemon_exits_of(stdout);
        if got.as_deref() != Some(*want) {
            bad.push(format!("{stdout:?}: the TS answers {want:?}, the Rust {got:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// TS box-backend.ts:760 settle throws the cancel's reason where it is an Error (box-backend.test.ts:849-865 aborts with
// `new Error("the person stopped waiting")` and expects those words). The Rust Cancel carries words, and the coordinator's ruling 4
// (comment 6405457065) reads every reason as an Error's message. Probe line cancel_resume_error_fetch2: err.message "the person stopped waiting".
#[tokio::test]
async fn a_cancel_whose_reason_was_an_error_ends_a_wait_with_that_errors_words() {
    let stop = Cancel::new();
    let firing = stop.clone();
    let api = FakeBox::new()
        .on(
            "POST",
            &path("/resume"),
            vec![with(move |_| {
                firing.cancel(Some("the person stopped waiting"));
                ok()
            })],
        )
        .on("GET", &path(""), vec![info(ID, "archived", json!({}))]);
    let (_on, machine) = machine_on(&api, ID);
    let e = machine.resume(Some(&stop)).await.unwrap_err();
    assert_eq!(e.to_string(), "the person stopped waiting");
}
