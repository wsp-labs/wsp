// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of box_backend::request's readings (spec/box_request.md).
#[path = "oracle/mod.rs"]
mod oracle;

use serde_json::json;
use wsp_engine::box_backend::*;
use wsp_engine::errors::*;

fn refusal(status: u16, code: Option<&str>, message: &str) -> WspError {
    classify(status, code, Some(message), Some("req_1"))
}

#[test]
fn stop_refusal_of_says_the_providers_error_else_its_snapshot_history() {
    for case in oracle::load("box.json")["stopRefusalOf"].as_array().unwrap() {
        let view: BoxView = serde_json::from_value({
            let mut v = case["in"].clone();
            v["id"] = json!("bx_1");
            v["state"] = json!("running");
            v
        })
        .unwrap();
        assert_eq!(stop_refusal_of(&view), oracle::s(&case["out"]), "{}", case["in"]);
    }
}

#[test]
fn box_refusal_reads_each_code_into_its_kind_and_keeps_boats_words_in_said() {
    let limit = box_refusal(refusal(429, Some(REQUEST_LIMIT_CODE), "Too many requests"));
    assert_eq!(limit.kind, ErrorKind::Transient);
    assert_eq!(limit.message, wsp_words::words::units::BOAT_REQUEST_LIMIT_LINE);
    assert_eq!(limit.said.as_deref(), Some("Too many requests"));
    assert_eq!(limit.request_id.as_deref(), Some("req_1"));

    let daily = box_refusal(refusal(429, Some("daily_limit_reached"), "daily"));
    assert_eq!((daily.kind, daily.window), (ErrorKind::StartLimit, Some(StartWindow::Day)));
    assert_eq!(daily.message, "daily");
    let rate = box_refusal(refusal(429, Some("rate_limited"), "slow down"));
    assert_eq!((rate.kind, rate.window), (ErrorKind::StartLimit, None));

    for code in ["limit_reached", "member_limit_reached"] {
        assert_eq!(box_refusal(refusal(403, Some(code), "x")).kind, ErrorKind::Concurrency, "{code}");
    }
    // The trial names the cap in words under a status of its own.
    assert_eq!(box_refusal(refusal(400, None, "You have reached the concurrent Sandbox limit")).kind, ErrorKind::Concurrency);
    // A 429 nobody listed keeps classify's cap reading and says Boat's words.
    let other = box_refusal(refusal(429, Some("weird"), "weird words"));
    assert_eq!((other.kind, other.message.as_str(), other.said.as_deref()), (ErrorKind::Concurrency, "weird words", Some("weird words")));

    let balance = box_refusal(refusal(402, Some("billing_required"), "pay"));
    assert_eq!(
        (balance.kind, balance.message.as_str(), balance.said.as_deref()),
        (ErrorKind::Plan, wsp_words::words::units::BOAT_BALANCE_LINE, Some("pay"))
    );
    let cap = box_refusal(refusal(403, Some("team_member_cap_reached"), "members"));
    assert_eq!(cap.message, wsp_words::words::units::BOAT_MEMBER_CAP_LINE);
    let room = box_refusal(refusal(503, Some("out_of_capacity"), "full"));
    assert_eq!((room.kind, room.message.as_str()), (ErrorKind::Transient, wsp_words::words::units::BOAT_NO_CAPACITY_LINE));

    let plain = refusal(409, Some("conflict_x"), "no");
    assert_eq!(box_refusal(plain.clone()), plain);
}

#[test]
fn the_request_limit_waits_the_seconds_boat_names_never_past_its_minute() {
    assert_eq!(request_limit_wait_ms(&json!({ "retryAfter": 7 })), 7000);
    assert_eq!(request_limit_wait_ms(&json!({ "error": { "details": { "retryAfter": 2.5 } } })), 2500);
    assert_eq!(request_limit_wait_ms(&json!({ "retryAfter": 600 })), 60_000);
    assert_eq!(request_limit_wait_ms(&json!({ "retryAfter": 0 })), 60_000);
    assert_eq!(request_limit_wait_ms(&json!({ "retryAfter": "7" })), 60_000);
    assert_eq!(request_limit_wait_ms(&json!({})), 60_000);
    assert_eq!(request_limit_wait_ms(&serde_json::Value::Null), 60_000);
    // The top of the body wins over the envelope's details.
    assert_eq!(request_limit_wait_ms(&json!({ "retryAfter": 3, "error": { "details": { "retryAfter": 9 } } })), 3000);
    // `??`: only a null or absent top value falls through to the details, so a top "7", 0 or -3 waits the minute
    // even with 9 below (B-R2).
    assert_eq!(request_limit_wait_ms(&json!({ "retryAfter": "7", "error": { "details": { "retryAfter": 9 } } })), 60_000);
    assert_eq!(request_limit_wait_ms(&json!({ "retryAfter": 0, "error": { "details": { "retryAfter": 9 } } })), 60_000);
    assert_eq!(request_limit_wait_ms(&json!({ "retryAfter": null, "error": { "details": { "retryAfter": 9 } } })), 9000);
    assert_eq!(request_limit_wait_ms(&json!({ "retryAfter": 0.5 })), 500);
}

#[test]
fn still_restoring_is_a_gateway_or_the_two_restore_codes_and_never_the_request_limit() {
    let p = |w: WspError| EngineError::Provider(w);
    assert!(still_restoring(&p(refusal(502, None, "Bad Gateway"))));
    assert!(still_restoring(&p(refusal(409, Some("sandbox_restoring"), "restoring"))));
    assert!(still_restoring(&p(refusal(409, Some("sandbox_starting"), "starting"))));
    assert!(!still_restoring(&p(refusal(409, Some("machine_not_running"), "stopped"))));
    assert!(!still_restoring(&p(box_refusal(refusal(429, Some(REQUEST_LIMIT_CODE), "x")))));
    assert!(!still_restoring(&p(refusal(502, None, "Failed to snapshot sandbox"))));
    assert!(!still_restoring(&EngineError::Other { message: "x".into(), code: None }));
}

#[test]
fn road_words_say_the_gateway_status_or_the_systems_code() {
    let e = EngineError::Provider(refusal(502, None, "Bad Gateway"));
    assert_eq!(road_words(&e), "502 Bad Gateway");
    let dropped = EngineError::Road(RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: "read ECONNRESET".into(), code: Some("ECONNRESET".into()), errors: None }),
    });
    assert_eq!(road_words(&dropped), "fetch failed: ECONNRESET");
    let capped = EngineError::Road(RoadError {
        name: "TimeoutError".into(),
        message: "The operation was aborted due to timeout".into(),
        code: None,
        cause: None,
    });
    assert_eq!(road_words(&capped), "The operation was aborted due to timeout");
}

#[test]
fn fail_keeps_every_field_and_an_empty_message_stays_empty() {
    let w = refusal(500, Some("x"), "");
    match fail(w.clone()) {
        EngineError::Provider(got) => assert_eq!(got, w),
        other => panic!("{other:?}"),
    }
    // Object.assign copies the refusal's own empty message over the fallback, so the words stay empty (B-R1).
    assert_eq!(fail(w).to_string(), "");
    assert_eq!(fail(refusal(404, None, "gone")).to_string(), "gone");
}

#[test]
fn a_template_row_reads_the_snapshots_status_and_its_error_only_when_failed() {
    let v = |status: &str, error: Option<&str>| NamedSnapshotView {
        name: "wsp-g-v1".into(),
        status: status.into(),
        error: error.map(str::to_string),
        source_sandbox_id: None,
        snapshot_id: None,
        kind: None,
        size_bytes: None,
        created_at: Some("2026-09-11T06:47:23.959Z".into()),
    };
    let row = serde_json::to_value(template_row_of(&v("ready", Some("ignored")))).unwrap();
    assert_eq!(row, json!({ "id": "wsp-g-v1", "name": "wsp-g-v1", "status": "ready", "createdAt": "2026-09-11T06:47:23.959Z" }));
    let row = serde_json::to_value(template_row_of(&v("failed", Some("disk full")))).unwrap();
    assert_eq!(
        row,
        json!({ "id": "wsp-g-v1", "name": "wsp-g-v1", "status": "failed", "error": "disk full", "createdAt": "2026-09-11T06:47:23.959Z" })
    );
    let row = serde_json::to_value(template_row_of(&v("saving", None))).unwrap();
    assert_eq!(row["status"], "building");
    let row = serde_json::to_value(template_row_of(&v("queued", None))).unwrap();
    assert_eq!(row["status"], "building");
}
