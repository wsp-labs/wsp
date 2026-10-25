// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of BoxBackend::request and the limits it keeps (spec/box_request.md).
mod common;

use std::sync::Arc;

use common::fake_box::*;
use serde_json::json;
use wsp_engine::box_backend::*;
use wsp_engine::errors::{EngineError, ErrorKind};
use wsp_engine::fetch::Fetch;

#[tokio::test]
async fn a_request_sends_authorization_first_then_its_own_headers_then_the_content_type() {
    let api = FakeBox::new().on("PATCH", "/sandboxes/bx_1", vec![info("bx_1", "ready", json!({}))]);
    let on = backend_on(&api, "trial", Default::default());
    let opts = RequestOpts { body: Some(json!({ "name": "wsp" })), headers: vec![("X-One".into(), "1".into())], ..Default::default() };
    let got = on.backend.request("PATCH", "/sandboxes/bx_1", opts).await.unwrap();
    assert_eq!(got["sandbox"]["id"], "bx_1");
    let seen = api.took("PATCH", "/sandboxes/bx_1").unwrap();
    let names: Vec<&str> = seen.headers.keys().map(String::as_str).collect();
    assert_eq!(names, vec!["authorization", "x-one", "content-type"]);
    assert_eq!(seen.headers["authorization"], "Bearer box_x");
    assert_eq!(seen.headers["content-type"], "application/json");
    assert_eq!(seen.raw.as_deref(), Some(r#"{"name":"wsp"}"#));
}

#[tokio::test]
async fn bytes_win_over_a_body_and_a_call_with_neither_sends_no_content_type() {
    let api = FakeBox::new().on("POST", "/x", vec![Answer::Reply(Reply { status: 200, body: json!({}) })]);
    let on = backend_on(&api, "trial", Default::default());
    let opts = RequestOpts { body: Some(json!({ "a": 1 })), bytes: Some(r#"{"b":2}"#.into()), ..Default::default() };
    on.backend.request("POST", "/x", opts).await.unwrap();
    on.backend.request("POST", "/x", RequestOpts::default()).await.unwrap();
    let seen = api.seen();
    assert_eq!(seen[0].raw.as_deref(), Some(r#"{"b":2}"#));
    assert!(!seen[1].headers.contains_key("content-type"));
    assert_eq!(seen[1].raw, None);
}

/// A fetch whose 2xx answers an empty body.
struct Empty;
#[async_trait::async_trait]
impl Fetch for Empty {
    async fn fetch(
        &self,
        _req: wsp_engine::fetch::FetchRequest,
    ) -> Result<wsp_engine::fetch::FetchResponse, wsp_engine::errors::RoadError> {
        Ok(wsp_engine::fetch::FetchResponse { status: 202, headers: vec![], body: vec![] })
    }
}

#[tokio::test]
async fn an_empty_answer_reads_as_an_empty_object() {
    let on = backend_with(Arc::new(Empty), Default::default());
    assert_eq!(on.backend.request("POST", "/sandboxes/bx_1/stop", RequestOpts::default()).await.unwrap(), json!({}));
}

#[tokio::test]
async fn a_refusal_that_is_not_json_says_its_text_and_reads_its_status_alone() {
    let api = FakeBox::new().on(
        "GET",
        "/sandboxes/bx_1",
        vec![with(|_| Answer::Reply(Reply { status: 403, body: json!("Forbidden by the edge") }))],
    );
    let on = backend_on(&api, "trial", Default::default());
    let e = on.backend.view("bx_1").await.unwrap_err();
    match e {
        // The body was the JSON string "Forbidden by the edge": not an envelope, so the said words are the text.
        EngineError::Provider(w) => {
            assert_eq!((w.kind, w.status, w.code.as_deref()), (ErrorKind::Plan, 403, None));
            assert_eq!(w.message, "\"Forbidden by the edge\"");
            assert_eq!(w.request_id, None);
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_gateway_answer_is_sent_three_times_on_the_engines_backoff_and_once_where_asked_once() {
    let api = FakeBox::new().on("GET", "/sandboxes/bx_1", vec![Answer::Reply(error(502, "bad_gateway", "Bad Gateway"))]);
    let on = backend_on(&api, "trial", Default::default());
    let e = on.backend.view("bx_1").await.unwrap_err();
    assert_eq!(e.status(), Some(502));
    assert_eq!(api.calls().iter().filter(|c| *c == "GET /sandboxes/bx_1").count(), 3);
    // Two waits: backoff_ms(1) and backoff_ms(2) at the clock's draw of 0.
    assert_eq!(on.clock.sleeps(), vec![1000, 2000]);
    let once = RequestOpts { once: true, ..Default::default() };
    let before = api.seen().len();
    on.backend.request("GET", "/sandboxes/bx_1", once).await.unwrap_err();
    assert_eq!(api.seen().len() - before, 1);
}

#[tokio::test]
async fn a_road_that_never_left_is_tried_three_times_in_all_and_any_other_road_failure_is_the_callers_at_once() {
    let api = FakeBox::new().on("GET", "/sandboxes/bx_1", vec![Answer::Road(offline())]);
    let on = backend_on(&api, "trial", Default::default());
    let e = on.backend.view("bx_1").await.unwrap_err();
    assert!(wsp_engine::errors::road_failed(&e));
    assert_eq!(api.seen().len(), 3);
    assert_eq!(on.clock.sleeps(), vec![3000, 6000]);
    assert!(on.warned.lines().iter().any(|l| l.contains("GET /sandboxes/bx_1")), "{:?}", on.warned.lines());

    let api = FakeBox::new().on("GET", "/sandboxes/bx_1", vec![Answer::Road(dropped())]);
    let on = backend_on(&api, "trial", Default::default());
    on.backend.view("bx_1").await.unwrap_err();
    assert_eq!(api.seen().len(), 1);
    assert_eq!(on.clock.sleeps(), Vec::<u64>::new());
}

#[tokio::test]
async fn the_same_refusal_of_the_same_call_is_logged_once_a_minute_and_a_new_code_logs_at_once() {
    let api = FakeBox::new().on("GET", "/sandboxes/bx_1", vec![Answer::Reply(error(409, "sandbox_restoring", "restoring"))]);
    let on = backend_on(&api, "trial", Default::default());
    for _ in 0..3 {
        on.backend.view("bx_1").await.unwrap_err();
    }
    assert_eq!(on.warned.lines().len(), 1);
    on.clock.advance(60_000);
    on.backend.view("bx_1").await.unwrap_err();
    assert_eq!(on.warned.lines().len(), 2);
    api.on("GET", "/sandboxes/bx_1", vec![Answer::Reply(error(409, "sandbox_starting", "starting"))]);
    on.backend.view("bx_1").await.unwrap_err();
    assert_eq!(on.warned.lines().len(), 3);
}

#[tokio::test]
async fn callers_of_limits_while_a_read_is_out_share_it_and_a_failed_read_is_not_kept() {
    let api = FakeBox::new().on("GET", "/limits", vec![Answer::Reply(error(500, "boom", "boom")), limits("standard")]);
    let on = backend_with(api.clone(), Default::default());
    let (a, b) = tokio::join!(on.backend.limits(), on.backend.limits());
    assert_eq!(a.unwrap_err().to_string(), "boom");
    assert_eq!(b.unwrap_err().to_string(), "boom");
    assert_eq!(api.calls().iter().filter(|c| *c == "GET /limits").count(), 1);
    // Nothing kept: the next call reads again, and that answer is kept for ten seconds.
    let l = on.backend.limits().await.unwrap();
    assert!(!l.trial);
    on.backend.limits().await.unwrap();
    assert_eq!(api.calls().iter().filter(|c| *c == "GET /limits").count(), 2);
    on.clock.advance(10_000);
    on.backend.limits().await.unwrap();
    assert_eq!(api.calls().iter().filter(|c| *c == "GET /limits").count(), 3);
}

#[tokio::test]
async fn a_counted_start_adds_one_to_every_window_on_the_kept_read_and_nothing_without_one() {
    let api = FakeBox::new().on(
        "GET",
        "/limits",
        vec![Answer::Reply(Reply { status: 200, body: json!({ "accessTier": "standard", "maxActiveSandboxes": 5, "starts": { "minute": { "limit": 5, "used": 1 }, "hour": { "limit": 25, "used": 2 }, "day": { "limit": 75, "used": 3 } } }) })],
    );
    let on = backend_with(api.clone(), Default::default());
    on.backend.counted();
    let l = on.backend.limits().await.unwrap();
    let s = l.starts.unwrap();
    assert_eq!((s.minute.used, s.hour.used, s.day.used), (1.0, 2.0, 3.0));
    on.backend.counted();
    on.backend.counted();
    let s = on.backend.limits().await.unwrap().starts.unwrap();
    assert_eq!((s.minute.used, s.hour.used, s.day.used), (3.0, 4.0, 5.0));
    assert_eq!((s.minute.limit, s.hour.limit, s.day.limit), (5.0, 25.0, 75.0));
}

#[tokio::test]
async fn a_failed_read_before_a_start_goes_on_with_the_kept_one_and_fails_with_none_kept() {
    let api = FakeBox::new().on("GET", "/limits", vec![limits("standard"), Answer::Reply(error(500, "boom", "boom"))]);
    let on = backend_with(api.clone(), Default::default());
    assert!(!on.backend.before_start().await.unwrap().trial);
    on.clock.advance(10_000);
    assert!(!on.backend.before_start().await.unwrap().trial, "the kept read stands in");
    let fresh = FakeBox::new().on("GET", "/limits", vec![Answer::Reply(error(500, "boom", "boom"))]);
    let on = backend_with(fresh, Default::default());
    assert_eq!(on.backend.before_start().await.unwrap_err().to_string(), "boom");
    assert_eq!(on.backend.place_limits().await, None);
}

#[tokio::test]
async fn the_seconds_to_a_stop_are_rounded_up_at_least_one_and_capped_on_the_trial() {
    let api = FakeBox::new();
    let on = backend_on(&api, "trial", Default::default());
    let now = on.clock.now_ms();
    assert_eq!(on.backend.ttl_cap().await.unwrap(), Some(TRIAL_TTL_S));
    assert_eq!(on.backend.ttl_seconds_for(None).await.unwrap(), Some(7200));
    assert_eq!(on.backend.ttl_seconds_for(Some(now + 1_001)).await.unwrap(), Some(2));
    assert_eq!(on.backend.ttl_seconds_for(Some(now - 50_000)).await.unwrap(), Some(1));
    assert_eq!(on.backend.ttl_seconds_for(Some(now + 3 * 3_600_000)).await.unwrap(), Some(7200));
    let paid = FakeBox::new();
    let on = backend_on(&paid, "standard", Default::default());
    assert_eq!(on.backend.ttl_seconds_for(None).await.unwrap(), None);
    assert_eq!(on.backend.ttl_seconds_for(Some(on.clock.now_ms() + 3 * 3_600_000)).await.unwrap(), Some(10_800));
}

#[tokio::test]
async fn settle_reads_until_done_then_fails_naming_the_state_after_its_budget_in_whole_seconds() {
    let api = FakeBox::new().on("GET", "/sandboxes/bx_1", vec![info("bx_1", "provisioning", json!({}))]);
    let on = backend_on(&api, "trial", Default::default());
    let e = on.backend.settle("bx_1", &|v| v.state == "ready", 1_499, "create", None, None, None).await.unwrap_err();
    assert_eq!(e.to_string(), "create of bx_1 still reads provisioning after 1 s");
    let reads = api.calls().iter().filter(|c| *c == "GET /sandboxes/bx_1").count();
    // Reads at 0, 10, ... 1490 and the one at 1500 that is past the budget: one more than the polls.
    assert_eq!(reads, 151);
    let stop = tokio_util::sync::CancellationToken::new();
    stop.cancel();
    let e = on.backend.settle("bx_1", &|_| true, 100, "resume", Some(&stop), None, None).await.unwrap_err();
    assert_eq!(e, wsp_engine::errors::aborted());
}
