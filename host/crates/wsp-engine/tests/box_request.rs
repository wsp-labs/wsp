// SPDX-License-Identifier: AGPL-3.0-only
//! How `request()` reads Boat's refusals and its limits: the codes, the request-limit waits, the log lines, the
//! limits kept 10 s and the starts counted on them, and the cap on a read nothing answers. Ported from
//! packages/engine/test/box-backend.test.ts:1461-1760 (less the class table, which is box_plan_limits.rs's),
//! box-limits.test.ts:52 and provider-reads.test.ts (Box's cases).
mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_engine::box_backend::{BoxBackend, BoxBackendOptions, BoxBudgetsAsk, BoxClassType, BoxMachine, RequestOpts};
use wsp_engine::errors::{EngineError, ErrorKind, RetryClock, RoadError, StartWindow, WspError, PROVIDER_READ_CAP_MS};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend, MachineKind, MachineSpec, MachineState};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::place_more::StartsWindow;

use common::fake_box::*;
use common::fake_clock::FakeClock;

// packages/engine/src/labels.ts:7 BUILDER_LABEL, :20 NAME_LABEL.
const BUILDER_LABEL: &str = "wsp-builder";
const NAME_LABEL: &str = "wsp-name";

/// box-backend.test.ts:1423 `OWNER_LIMITS`: the owner's account read with `GET /limits` on 2026-10-11 17:06 UTC.
fn owner_limits() -> Value {
    json!({
        "accessTier": "standard",
        "maxActiveSandboxes": 100,
        "activeSandboxes": 11,
        "startLimits": { "perMinute": 12, "perHour": 60, "perDay": 200 },
        "starts": {
            "unlimited": false,
            "minute": { "limit": 12, "used": 0, "remaining": 12 },
            "hour": { "limit": 60, "used": 9, "remaining": 51 },
            "day": { "limit": 200, "used": 187, "remaining": 13 }
        },
        "activeStates": ["provisioned", "cloning", "ready", "idle", "running"],
        "canStart": true,
        "startBlockedReason": null,
        "creditBalanceHours": 81.6,
        "sandboxPlanKey": "box_20"
    })
}

/// box-backend.test.ts:1437 `TRIAL_LIMITS`: a trial account with the trial's own numbers.
fn trial_limits() -> Value {
    json!({
        "accessTier": "trial",
        "maxActiveSandboxes": 2,
        "activeSandboxes": 0,
        "startLimits": { "perMinute": 5, "perHour": 25, "perDay": 75 },
        "starts": {
            "unlimited": false,
            "minute": { "limit": 5, "used": 0, "remaining": 5 },
            "hour": { "limit": 25, "used": 0, "remaining": 25 },
            "day": { "limit": 75, "used": 0, "remaining": 75 }
        },
        "trialLimits": { "activeSandboxes": 2, "creationRatePerMinute": 5, "creationRequestsPerHour": 25, "creationRequestsPerDay": 75 },
        "canStart": true,
        "startBlockedReason": null,
        "billingStatus": "trial",
        "creditBalanceHours": 25,
        "sandboxPlanKey": "trial"
    })
}

/// box-backend.test.ts:1450 `limitsReply`.
fn limits_reply(body: Value) -> Answer {
    let mut all = json!({ "ok": true, "type": "limits.info" });
    for (k, v) in body.as_object().unwrap() {
        all[k] = v.clone();
    }
    Answer::Reply(Reply { status: 200, body: all })
}

/// box-backend.test.ts:1452 `spent`: the owner's answer with one window spent.
fn spent(window: &str) -> Answer {
    let mut body = owner_limits();
    let limit = body["starts"][window]["limit"].clone();
    body["starts"][window]["used"] = limit;
    body["starts"][window]["remaining"] = json!(0);
    limits_reply(body)
}

fn spec(labels: Option<IndexMap<String, String>>) -> MachineSpec {
    MachineSpec {
        kind: MachineKind::Sandbox,
        template: None,
        from_snapshot: None,
        cpu: None,
        mem_mb: None,
        disk_gb: None,
        envs: None,
        labels,
        on_idle: None,
        idle_timeout_ms: None,
        idempotency_key: None,
        engine: None,
        copy: None,
        shares: None,
        binds: None,
    }
}

/// box-backend.test.ts:1459 `NAMED_SPEC`.
fn named_spec() -> MachineSpec {
    spec(Some(IndexMap::from([(NAME_LABEL.to_string(), "Fix the flaky upload test".to_string())])))
}

fn provider(e: &EngineError) -> &WspError {
    match e {
        EngineError::Provider(w) => w,
        other => panic!("expected a provider's refusal, got {other:?}"),
    }
}

fn count(api: &FakeBox, call: &str) -> usize {
    api.calls().iter().filter(|c| c.as_str() == call).count()
}

/// box-backend.test.ts:1530 `refusedOn`: a create of the named spec on a standard account whose POST /sandboxes
/// answers `answer`.
async fn refused_on(answer: Reply, limits: Answer) -> (EngineError, Arc<FakeBox>, On) {
    let api = FakeBox::new().on("POST", "/sandboxes", vec![answer.into()]);
    let on = backend_on(&api, "standard", Default::default());
    api.on("GET", "/limits", vec![limits]);
    let e = on.backend.create(named_spec(), None, None).await.map(|_| ()).unwrap_err();
    (e, api, on)
}

async fn refused_default(answer: Reply) -> (EngineError, Arc<FakeBox>, On) {
    refused_on(answer, limits_reply(owner_limits())).await
}

fn window(used: f64, limit: f64) -> wsp_engine::box_backend::StartCount {
    wsp_engine::box_backend::StartCount { used, limit }
}

fn offered_cpus(caps: &wsp_engine::machine::Capabilities) -> Vec<f64> {
    caps.sizes.iter().map(|s| s.cpu.0).collect()
}

/// box-backend.test.ts:1478
#[tokio::test]
async fn reads_the_owners_answer_of_17_06_utc_the_20_plan_100_machines_three_windows_small_to_large() {
    let api = FakeBox::new().on("GET", "/limits", vec![limits_reply(owner_limits())]);
    let on = backend_with(api.clone() as Arc<dyn Fetch>, Default::default());
    let read = on.backend.limits().await.unwrap();
    assert_eq!(read.trial, false);
    assert_eq!(read.plan_key.as_deref(), Some("box_20"));
    assert_eq!(read.plan_usd, Some(20.0));
    assert_eq!(read.machines_at_once, Some(100.0));
    assert_eq!(read.active_boxes, Some(11.0));
    let starts = read.starts.expect("the three windows");
    assert_eq!((starts.minute, starts.hour, starts.day), (window(0.0, 12.0), window(9.0, 60.0), window(187.0, 200.0)));
    assert_eq!(read.sizes, vec![BoxClassType::Small, BoxClassType::Default, BoxClassType::Large]);
    assert_eq!(read.can_start, true);
    assert_eq!(read.start_blocked_reason, None);
    assert_eq!(read.credit_balance_hours, Some(81.6));
    assert_eq!(read.billing_status, None);
    let offer = |name: &str, cpu: f64, mem_mb: f64, rate: f64| (Some(name.to_string()), JsNumber(cpu), JsNumber(mem_mb), JsNumber(rate));
    let sizes: Vec<_> = on.backend.capabilities().sizes.into_iter().map(|s| (s.name, s.cpu, s.mem_mb, s.rate_usd_per_hour)).collect();
    assert_eq!(sizes, vec![offer("small", 2.0, 4096.0, 0.018), offer("default", 4.0, 8192.0, 0.036), offer("large", 8.0, 16384.0, 0.072)]);
}

/// box-backend.test.ts:1499
#[tokio::test]
async fn a_trial_offers_small_and_default_and_reads_2_machines_a_100_plan_adds_xlarge() {
    let api = FakeBox::new().on("GET", "/limits", vec![limits_reply(trial_limits())]);
    let on = backend_with(api.clone() as Arc<dyn Fetch>, Default::default());
    let read = on.backend.limits().await.unwrap();
    assert_eq!(read.trial, true);
    assert_eq!(read.machines_at_once, Some(2.0));
    assert_eq!(read.sizes, vec![BoxClassType::Small, BoxClassType::Default]);
    let starts = read.starts.expect("the three windows");
    assert_eq!((starts.minute.limit, starts.hour.limit, starts.day.limit), (5.0, 25.0, 75.0));
    assert_eq!(read.credit_balance_hours, Some(25.0));
    assert_eq!(offered_cpus(&on.backend.capabilities()), vec![2.0, 4.0]);

    let mut plan = owner_limits();
    plan["sandboxPlanKey"] = json!("box_100");
    plan["maxActiveSandboxes"] = json!(300);
    let big = FakeBox::new().on("GET", "/limits", vec![limits_reply(plan)]);
    let bigger = backend_with(big.clone() as Arc<dyn Fetch>, Default::default());
    assert_eq!(
        bigger.backend.limits().await.unwrap().sizes,
        vec![BoxClassType::Small, BoxClassType::Default, BoxClassType::Large, BoxClassType::Xlarge]
    );
}

/// box-backend.test.ts:1509
#[tokio::test]
async fn keeps_a_read_10_s_reads_before_every_create_and_resume_past_that_and_counts_each_start() {
    let made = Answer::Reply(Reply {
        status: 202,
        body: json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_c1", "provisioning", json!({ "name": "wsp" })) }),
    });
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![made])
        .on("GET", "/sandboxes/bx_c1", vec![info("bx_c1", "ready", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_c1/commands", vec![command("", Some(0), json!({}))])
        .on(
            "POST",
            "/sandboxes/bx_c1/resume",
            vec![Answer::Reply(Reply {
                status: 202,
                body: json!({ "ok": true, "sandbox": box_view("bx_c1", "provisioning", json!({})) }),
            })],
        );
    let on = backend_on(&api, "standard", Default::default());
    api.on("GET", "/limits", vec![limits_reply(owner_limits())]);
    on.backend.create(spec(None), None, None).await.unwrap();
    on.backend.create(spec(None), None, None).await.unwrap();
    assert_eq!(count(&api, "GET /limits"), 1);
    let starts = on.backend.limits().await.unwrap().starts.expect("the three windows");
    assert_eq!((starts.minute, starts.hour, starts.day), (window(2.0, 12.0), window(11.0, 60.0), window(189.0, 200.0)));
    on.clock.advance(10_000);
    BoxMachine::new(on.backend.clone(), "bx_c1", MachineKind::Sandbox, None, None, false).resume(None).await.unwrap();
    assert_eq!(count(&api, "GET /limits"), 2);
    let calls = api.calls();
    let resumed = calls.iter().position(|c| c == "POST /sandboxes/bx_c1/resume").expect("the resume was sent");
    let last_read = calls.iter().rposition(|c| c == "GET /limits").unwrap();
    assert!(resumed > last_read, "{calls:?}");
    assert_eq!(on.backend.limits().await.unwrap().starts.unwrap().day, window(188.0, 200.0));
}

/// box-backend.test.ts:1538
#[tokio::test]
async fn limit_reached_and_member_limit_reached_are_the_machine_cap() {
    for code in ["limit_reached", "member_limit_reached"] {
        let (e, _, _) = refused_default(error(429, code, "Concurrent box limit reached.")).await;
        let w = provider(&e);
        assert_eq!(w.kind, ErrorKind::Concurrency);
        assert_eq!(w.code.as_deref(), Some(code));
        assert_eq!(w.message, "Concurrent box limit reached.");
    }
}

/// box-backend.test.ts:1545
#[tokio::test]
async fn daily_limit_reached_is_the_day_in_boats_line_with_the_accounts_own_numbers_and_plan() {
    let said = "Rate limit hit: 200 box starts per day (create, fork and resume each count as one start). Your plan allows 12/min, 60/hour and 200/day.";
    let (e, api, _) = refused_on(error(429, "daily_limit_reached", said), spent("day")).await;
    let w = provider(&e);
    assert_ne!(w.kind, ErrorKind::Concurrency);
    assert_eq!(w.kind, ErrorKind::StartLimit);
    assert_eq!(w.window, Some(StartWindow::Day));
    assert_eq!(w.code.as_deref(), Some("daily_limit_reached"));
    assert_eq!(w.request_id.as_deref(), Some("req_b603629401be48d082e623575a1b9111"));
    assert_eq!(
        w.message,
        "Boat's starts for today are used (200 of 200 on the $20 plan), so Fix the flaky upload test was not started; Boat counts the day again tomorrow, or a plan with more starts is at boat.dev/dashboard."
    );
    // The window's numbers come off a fresh read, never the kept one.
    assert_eq!(count(&api, "GET /limits"), 2);
}

/// box-backend.test.ts:1555
#[tokio::test]
async fn rate_limited_is_the_hour_or_the_minute_told_apart_by_a_read_of_limits() {
    let rate = || error(429, "rate_limited", "Too many machine starts.");
    let (e, _, _) = refused_on(rate(), spent("hour")).await;
    let w = provider(&e);
    assert_eq!((w.kind, w.window), (ErrorKind::StartLimit, Some(StartWindow::Hour)));
    assert_eq!(
        w.message,
        "Boat's starts for this hour are used (60 of 60 on the $20 plan), so Fix the flaky upload test was not started; start it again later in the hour, or move to a plan with more at boat.dev/dashboard."
    );
    let (e, _, _) = refused_on(rate(), spent("minute")).await;
    let w = provider(&e);
    assert_eq!((w.kind, w.window), (ErrorKind::StartLimit, Some(StartWindow::Minute)));
    assert_eq!(w.message, "Boat's starts for this minute are used (12 of 12 on the $20 plan), so Fix the flaky upload test was not started; start it again in a minute.");
    // A trial names itself rather than a price.
    let mut trial = trial_limits();
    trial["starts"]["day"] = json!({ "limit": 75, "used": 75, "remaining": 0 });
    let (e, _, _) = refused_on(error(429, "daily_limit_reached", "Rate limit hit."), limits_reply(trial)).await;
    assert!(provider(&e).message.starts_with("Boat's starts for today are used (75 of 75 on the trial), so"), "{e}");
}

/// box-backend.test.ts:1567
#[tokio::test]
async fn a_start_with_no_name_such_as_a_builder_names_the_machine_in_the_line() {
    let api = FakeBox::new().on("POST", "/sandboxes", vec![error(429, "daily_limit_reached", "Rate limit hit.").into()]);
    let on = backend_on(&api, "standard", Default::default());
    api.on("GET", "/limits", vec![spent("day")]);
    let builder = spec(Some(IndexMap::from([(BUILDER_LABEL.to_string(), "1".to_string())])));
    let e = on.backend.create(builder, None, None).await.map(|_| ()).unwrap_err();
    assert_eq!(
        e.to_string(),
        "Boat's starts for today are used (200 of 200 on the $20 plan), so the machine was not started; Boat counts the day again tomorrow, or a plan with more starts is at boat.dev/dashboard."
    );
}

/// box-backend.test.ts:1575
#[tokio::test]
async fn rate_limited_keeps_boats_words_when_the_fresh_read_shows_neither_minute_nor_hour_spent() {
    let (e, _, _) = refused_default(error(429, "rate_limited", "Too many machine starts.")).await;
    let w = provider(&e);
    assert_eq!(w.kind, ErrorKind::StartLimit);
    assert_eq!(w.code.as_deref(), Some("rate_limited"));
    assert_eq!(w.message, "Too many machine starts.");
}

/// box-backend.test.ts:1580
#[tokio::test]
async fn the_first_create_after_a_host_start_checks_the_size_against_the_plan_and_refuses_one_not_sold() {
    let api = FakeBox::new().on("GET", "/limits", vec![limits_reply(owner_limits())]);
    let on = backend_with(api.clone() as Arc<dyn Fetch>, Default::default());
    // Nothing read yet, so the sizes offered are every class.
    assert_eq!(offered_cpus(&on.backend.capabilities()), vec![2.0, 4.0, 8.0, 16.0]);
    let mut big = spec(None);
    big.cpu = Some(JsNumber(16.0));
    big.mem_mb = Some(JsNumber(32768.0));
    let e = on.backend.create(big, None, None).await.map(|_| ()).unwrap_err();
    match &e {
        EngineError::Invalid { message } => assert_eq!(
            message,
            "16x32 is not a size this provider offers; the sizes are 2x4 ($0.018/hr), 4x8 ($0.036/hr), 8x16 ($0.072/hr). Ask for one of those instead."
        ),
        other => panic!("expected the invalid-size refusal, got {other:?}"),
    }
    assert_eq!(api.calls(), vec!["GET /limits".to_string()]);
}

/// box-backend.test.ts:1591
#[tokio::test]
async fn a_trial_is_read_off_the_plan_key_too_which_reads_trial_before_any_payment() {
    let mut no_tier = trial_limits();
    no_tier.as_object_mut().unwrap().shift_remove("accessTier");
    let api = FakeBox::new().on("GET", "/limits", vec![limits_reply(no_tier)]);
    let on = backend_with(api.clone() as Arc<dyn Fetch>, Default::default());
    let read = on.backend.limits().await.unwrap();
    assert_eq!(read.trial, true);
    assert_eq!(read.sizes, vec![BoxClassType::Small, BoxClassType::Default]);
}

/// box-backend.test.ts:1598
#[tokio::test]
async fn two_boxes_refused_in_turn_each_keep_their_own_quiet_minute_in_the_log() {
    let restoring = || vec![Answer::Reply(error(409, "sandbox_restoring", "Box is restoring."))];
    let api = FakeBox::new().on("GET", "/sandboxes/bx_a", restoring()).on("GET", "/sandboxes/bx_b", restoring());
    let on = backend_on(&api, "standard", Default::default());
    // 60 s of polls every 2 s, the two boxes in turn.
    for _ in 0..30 {
        let _ = on.backend.view("bx_a").await;
        let _ = on.backend.view("bx_b").await;
        on.clock.advance(2_000);
    }
    let lines: Vec<String> = on.warned.lines().into_iter().filter(|l| l.starts_with("Boat refused")).collect();
    assert_eq!(
        lines,
        vec![
            "Boat refused GET /sandboxes/bx_a: 409 code=sandbox_restoring requestId=req_b603629401be48d082e623575a1b9111: Box is restoring.".to_string(),
            "Boat refused GET /sandboxes/bx_b: 409 code=sandbox_restoring requestId=req_b603629401be48d082e623575a1b9111: Box is restoring.".to_string(),
        ]
    );
}

/// box-backend.test.ts:1621
#[tokio::test]
async fn a_resume_that_meets_a_start_limit_says_so_too() {
    let api = FakeBox::new().on("POST", "/sandboxes/bx_w1/resume", vec![error(429, "daily_limit_reached", "Rate limit hit.").into()]);
    let on = backend_on(&api, "standard", Default::default());
    api.on("GET", "/limits", vec![spent("day")]);
    let labels = IndexMap::from([(NAME_LABEL.to_string(), "Write the upload docs".to_string())]);
    let machine = BoxMachine::new(on.backend.clone(), "bx_w1", MachineKind::Sandbox, Some(labels), None, false);
    let e = machine.resume(None).await.unwrap_err();
    let w = provider(&e);
    assert_eq!((w.kind, w.window), (ErrorKind::StartLimit, Some(StartWindow::Day)));
    assert!(
        w.message.starts_with("Boat's starts for today are used (200 of 200 on the $20 plan), so Write the upload docs was not started;"),
        "{e}"
    );
}

/// box-backend.test.ts:1629
#[tokio::test]
async fn a_429_with_a_code_boat_never_listed_keeps_todays_reading_and_boats_own_words() {
    let (e, _, _) = refused_default(error(429, "slow_down", "Please slow down.")).await;
    let w = provider(&e);
    assert_eq!(w.kind, ErrorKind::Concurrency);
    assert_eq!(w.code.as_deref(), Some("slow_down"));
    assert_eq!(w.message, "Please slow down.");
    assert_eq!(w.said.as_deref(), Some("Please slow down."));
}

/// box-backend.test.ts:1637 `IP_LIMIT`: Boat's refusal of an address past 1,000 requests a minute: the code, and the
/// wait in the body under `retryAfter` in seconds (at the top or under error.details), with no Retry-After header.
fn ip_limit(retry_after: Option<u64>, in_details: bool) -> Reply {
    let mut body = error_body(429, "IP_RATE_LIMIT_EXCEEDED", "Too many requests from this IP address.");
    match retry_after {
        None => {}
        Some(n) if !in_details => body["retryAfter"] = json!(n),
        Some(n) => body["error"] = json!({ "code": "IP_RATE_LIMIT_EXCEEDED", "status": 429, "details": { "retryAfter": n } }),
    }
    Reply { status: 429, body }
}

/// box-backend.test.ts:1651
#[tokio::test]
async fn an_address_past_boats_request_limit_waits_the_seconds_boat_names_and_asks_again() {
    let created = FakeBox::new()
        .on(
            "POST",
            "/sandboxes",
            vec![
                ip_limit(Some(7), false).into(),
                Answer::Reply(Reply { status: 202, body: json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_c1", "provisioning", json!({ "name": "wsp" })) }) }),
            ],
        )
        .on("GET", "/sandboxes/bx_c1", vec![info("bx_c1", "ready", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_c1/commands", vec![command("", Some(0), json!({}))]);
    let on = backend_on(&created, "standard", Default::default());
    created.on("GET", "/limits", vec![limits_reply(owner_limits())]);
    assert_eq!(on.backend.create(spec(None), None, None).await.unwrap().id(), "bx_c1");
    let posts: Vec<Seen> = created.seen().into_iter().filter(|s| s.method == "POST" && s.path == "/sandboxes").collect();
    // The same bytes under the same key, so the ask that Boat refused and the one it took are one create.
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[1].raw, posts[0].raw);
    assert!(posts[0].headers.get("idempotency-key").is_some());
    assert_eq!(posts[1].headers.get("idempotency-key"), posts[0].headers.get("idempotency-key"));
    assert_eq!(on.clock.sleeps()[0], 7_000);
    assert!(
        on.warned.lines().contains(
            &"POST /sandboxes met Boat's limit of 1,000 requests a minute from one address; asking again in 7s, try 2 of 3".to_string()
        ),
        "{:?}",
        on.warned.lines()
    );

    let execd = FakeBox::new().on(
        "POST",
        "/sandboxes/bx_tumrjngm/commands",
        vec![ip_limit(Some(3), true).into(), command("hi\n", Some(0), json!({}))],
    );
    let (exec_on, machine) = machine_on(&execd, "bx_tumrjngm");
    let out = machine.exec("echo hi", ExecOptions::default()).await.unwrap();
    assert_eq!((out.exit_code, out.stdout.as_str()), (0.into(), "hi\n"));
    assert_eq!(execd.calls().iter().filter(|c| c.ends_with("/commands")).count(), 2);
    assert_eq!(exec_on.clock.sleeps(), vec![3_000]);

    let put = FakeBox::new().on(
        "PUT",
        "/sandboxes/bx_tumrjngm/files",
        vec![ip_limit(None, false).into(), Answer::Reply(Reply { status: 200, body: json!({ "ok": true }) })],
    );
    let (put_on, file) = machine_on(&put, "bx_tumrjngm");
    file.put_bytes("/tmp/note", b"hi", None).await.unwrap();
    assert_eq!(put.calls().iter().filter(|c| c.ends_with("/files")).count(), 2);
    // No wait named: the minute the limit counts.
    assert_eq!(put_on.clock.sleeps(), vec![60_000]);
}

/// box-backend.test.ts:1705
#[tokio::test]
async fn the_request_limit_is_asked_three_times_at_most_then_fails_in_its_own_line_not_as_the_cap() {
    let (e, api, _) = refused_default(ip_limit(Some(2), false)).await;
    let w = provider(&e);
    assert_ne!(w.kind, ErrorKind::Concurrency);
    assert_eq!(w.kind, ErrorKind::Transient);
    assert_eq!(w.status, 429);
    assert_eq!(w.code.as_deref(), Some("IP_RATE_LIMIT_EXCEEDED"));
    assert_eq!(w.said.as_deref(), Some("Too many requests from this IP address."));
    assert_eq!(
        w.message,
        "Boat is refusing requests from this computer's address, past its limit of 1,000 a minute, and still was after waiting and asking again; try again in a minute."
    );
    assert_eq!(count(&api, "POST /sandboxes"), 3);
    // The machine cap keeps its own reading and is asked once.
    let (cap, cap_api, _) = refused_default(error(429, "limit_reached", "Concurrent box limit reached.")).await;
    let w = provider(&cap);
    assert_eq!(w.kind, ErrorKind::Concurrency);
    assert_eq!(w.code.as_deref(), Some("limit_reached"));
    assert_eq!(w.message, "Concurrent box limit reached.");
    assert_eq!(count(&cap_api, "POST /sandboxes"), 1);
}

/// box-backend.test.ts:1722
#[tokio::test]
async fn a_402_and_a_503_out_of_capacity_print_boats_lines_503_only_once_the_creates_asks_run_out() {
    let (billing, _, _) = refused_default(error(402, "billing_required", "Add credits to continue.")).await;
    let w = provider(&billing);
    assert_eq!(w.kind, ErrorKind::Plan);
    assert_eq!(w.code.as_deref(), Some("billing_required"));
    assert_eq!(w.message, "Boat's balance is used up, so no machine starts; add time at boat.dev/dashboard. Boat stops running machines 24 hours after the balance ran out.");
    let (capped, _, _) = refused_default(error(402, "team_member_cap_reached", "Member usage cap reached.")).await;
    assert_eq!(provider(&capped).message, "this Boat organization caps your usage; ask its owner to raise your cap.");
    // The TS draws `Math.random()` for its jitter; 48 asks hold for a draw of 0.25 or more, 51 for less. Take the middle.
    let api = FakeBox::new().on("POST", "/sandboxes", vec![error(503, "out_of_capacity", "No capacity.").into()]);
    let on = backend_on(&api, "standard", Default::default());
    *on.clock.draw.lock().unwrap() = 0.5;
    api.on("GET", "/limits", vec![limits_reply(owner_limits())]);
    let full = on.backend.create(named_spec(), None, None).await.map(|_| ()).unwrap_err();
    let w = provider(&full);
    assert_eq!(w.kind, ErrorKind::Transient);
    assert_eq!(w.code.as_deref(), Some("out_of_capacity"));
    assert_eq!(w.message, "Boat has no free machines of this size right now; try again in a few minutes, or a smaller size.");
    // A 5xx on a create may come after Boat made the box, so it is asked again inside the lost-answer window first:
    // 16 asks of 3 tries each on the fake clock before the 120 s window ends.
    assert_eq!(count(&api, "POST /sandboxes"), 48);
}

/// box-backend.test.ts:1736
#[tokio::test]
async fn cancelled_reads_as_gone_and_ends_a_creates_wait_on_the_first_read_with_boats_reason() {
    let api = FakeBox::new()
        .on(
            "POST",
            "/sandboxes",
            vec![Answer::Reply(Reply {
                status: 202,
                body: json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_x1", "provisioning", json!({})) }),
            })],
        )
        .on("GET", "/sandboxes/bx_x1", vec![info("bx_x1", "cancelled", json!({ "error": "no machine available" }))])
        .on("DELETE", "/sandboxes/bx_x1", vec![error(404, "not_found", "not_found").into()]);
    let on = backend_on(&api, "standard", BoxBudgetsAsk { create_ms: Some(60_000), ..Default::default() });
    let e = on.backend.create(named_spec(), None, None).await.map(|_| ()).unwrap_err();
    assert!(
        e.to_string().contains(
            "Boat could not get a machine for Fix the flaky upload test and cancelled the create (no machine available); start it again."
        ),
        "{e}"
    );
    assert_eq!(count(&api, "GET /sandboxes/bx_x1"), 1);
    assert_eq!(on.backend.get("bx_x1").await.unwrap().state().await.unwrap(), MachineState::Gone);
}

/// box-backend.test.ts:1747
#[tokio::test]
async fn every_refused_call_goes_in_the_host_log_with_boats_code_and_request_id() {
    let (_, _, day) = refused_on(error(429, "daily_limit_reached", "Rate limit hit."), spent("day")).await;
    let (_, _, billing) = refused_default(error(402, "billing_required", "Add credits.")).await;
    let lines: Vec<String> = day.warned.lines().into_iter().chain(billing.warned.lines()).collect();
    assert!(
        lines.contains(
            &"Boat refused POST /sandboxes: 429 code=daily_limit_reached requestId=req_b603629401be48d082e623575a1b9111: Rate limit hit."
                .to_string()
        ),
        "{lines:?}"
    );
    assert!(
        lines.contains(
            &"Boat refused POST /sandboxes: 402 code=billing_required requestId=req_b603629401be48d082e623575a1b9111: Add credits."
                .to_string()
        ),
        "{lines:?}"
    );
}

/// box-limits.test.ts:56: a fetch answering the owner's limits, or Boat's `internal` under the status set.
struct Toggled {
    status: Mutex<u16>,
    asked: Mutex<Vec<String>>,
}

#[async_trait(?Send)]
impl Fetch for Toggled {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        self.asked.lock().unwrap().push(req.url.clone());
        let status = *self.status.lock().unwrap();
        let body = if status == 200 { owner_limits() } else { json!({ "code": "internal", "message": "down" }) };
        Ok(FetchResponse {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: serde_json::to_vec(&body).unwrap(),
        })
    }
}

/// box-limits.test.ts:52
#[tokio::test]
async fn reads_the_backends_own_kept_read_a_start_counted_on_it_and_nothing_once_a_read_fails() {
    let clock = FakeClock::at(0);
    let fetch = Arc::new(Toggled { status: Mutex::new(200), asked: Mutex::new(Vec::new()) });
    let boat = BoxBackend::new(BoxBackendOptions {
        api_key: "box_x".into(),
        fetch: Some(fetch.clone() as Arc<dyn Fetch>),
        clock: Some(clock.clone() as Arc<dyn RetryClock>),
        ..Default::default()
    });
    let day = |l: Option<wsp_engine::machine::CloudLimits>| l.and_then(|l| l.starts).map(|s| s.day);
    assert_eq!(day(boat.place_limits().await), Some(StartsWindow { used: 187.into(), limit: 200.into() }));
    boat.counted();
    assert_eq!(day(boat.place_limits().await), Some(StartsWindow { used: 188.into(), limit: 200.into() }));
    assert_eq!(fetch.asked.lock().unwrap().len(), 1);
    clock.set(60_000);
    *fetch.status.lock().unwrap() = 400;
    assert_eq!(boat.place_limits().await, None);
}

/// provider-reads.test.ts:20, Box's half (Solari's is its own slice's).
#[test]
fn reads_at_most_the_measured_default_by_default() {
    let box_ = BoxBackend::new(BoxBackendOptions { api_key: "box_x".into(), ..Default::default() });
    assert_eq!(box_.budgets().read_ms, PROVIDER_READ_CAP_MS);
}

/// provider-reads.test.ts:9 `neverAnswers`: a fetch that holds every request until its signal ends it, recording
/// whether one came with a signal at all.
struct NeverAnswers {
    asked: Mutex<Vec<(String, String, bool)>>,
}

impl NeverAnswers {
    fn new() -> Arc<Self> {
        Arc::new(NeverAnswers { asked: Mutex::new(Vec::new()) })
    }
    fn asked(&self) -> Vec<(String, String, bool)> {
        self.asked.lock().unwrap().clone()
    }
}

#[async_trait(?Send)]
impl Fetch for NeverAnswers {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let path = url::Url::parse(&req.url).unwrap().path().to_string();
        self.asked.lock().unwrap().push((req.method.clone(), path, req.abort.is_some()));
        let Some(abort) = req.abort else { return std::future::pending().await };
        abort.token.cancelled().await;
        let name = if abort.timed_out.load(std::sync::atomic::Ordering::SeqCst) { "TimeoutError" } else { "AbortError" };
        Err(RoadError { name: name.into(), message: "The operation was aborted".into(), code: None, cause: None })
    }
}

fn row(method: &str, path: &str, capped: bool) -> (String, String, bool) {
    (method.to_string(), path.to_string(), capped)
}

/// provider-reads.test.ts:25
#[tokio::test]
async fn ends_each_of_boxs_reads_of_a_machine_the_fleet_and_the_snapshots_at_the_reads_cap() {
    let api = NeverAnswers::new();
    let on = backend_with(api.clone() as Arc<dyn Fetch>, BoxBudgetsAsk { read_ms: Some(150), ..Default::default() });
    assert!(on.backend.get("bx_acme").await.is_err());
    assert!(on.backend.list(None).await.is_err());
    assert!(on.backend.list_snapshots(None).await.is_err());
    assert_eq!(
        api.asked(),
        vec![
            row("GET", "/api/v1/sandboxes/bx_acme", true),
            row("GET", "/api/v1/sandboxes", true),
            row("GET", "/api/v1/named-snapshots", true),
        ]
    );
}

/// provider-reads.test.ts:47
#[tokio::test]
async fn leaves_a_write_uncapped_where_its_own_road_gives_it_none() {
    let api = NeverAnswers::new();
    let on = backend_with(api.clone() as Arc<dyn Fetch>, BoxBudgetsAsk { read_ms: Some(150), ..Default::default() });
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(20),
        on.backend.request("POST", "/sandboxes/bx_acme/stop", RequestOpts::default()),
    )
    .await;
    assert_eq!(api.asked(), vec![row("POST", "/api/v1/sandboxes/bx_acme/stop", false)]);
}

/// provider-reads.test.ts:55
#[tokio::test]
async fn asks_box_for_the_accounts_limits_again_once_a_read_of_them_ended_at_its_cap() {
    let api = FakeBox::new().on(
        "GET",
        "/limits",
        vec![Answer::Hang, Answer::Reply(Reply { status: 200, body: json!({ "accessTier": "paid" }) })],
    );
    let on = backend_with(api.clone() as Arc<dyn Fetch>, BoxBudgetsAsk { read_ms: Some(150), ..Default::default() });
    assert!(on.backend.ttl_cap().await.is_err());
    assert_eq!(on.backend.ttl_cap().await.unwrap(), None);
    assert_eq!(api.seen().len(), 2);
}
