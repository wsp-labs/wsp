// SPDX-License-Identifier: AGPL-3.0-only
//! solari_call's S-rules (spec/solari_call.md S1 to S13): the exact sleeps of every retry road and which counter each
//! reads, the bodies an answer can carry, the requests' headers, aborts and bodies, the snapshot wait's boundary, the
//! preview token's numbers, the backend's offers and what the key check reads. Values from running
//! solari-backend.ts and key-check.ts at a4033dc34 with a fixed draw of 0.5 (jitter 125).
#[path = "rest_common/mod.rs"]
mod common;

use std::sync::Arc;

use serde_json::{json, Value};

use common::{fetch_failed, no_road, reply, reply_text, reply_with, FakeFetch, MovingClock, Play, Warned};
use wsp_engine::errors::{ErrorKind, RetryClock};
use wsp_engine::fetch::Fetch;
use wsp_engine::key_check::{check_provider_key, KeyCheck};
use wsp_engine::machine::{BackendOffers, MachineBackend, MachineSpec};
use wsp_engine::solari_backend::{preview_token_expiry, SolariBackend, SolariBackendOptions};
use wsp_words::words::thread::{provider_road_retry_line, snapshot_listed_refused_line, snapshot_listed_wait_line};

fn build(f: &Arc<FakeFetch>, clock: &Arc<MovingClock>, warned: &Warned) -> Arc<SolariBackend> {
    let fetch: Arc<dyn Fetch> = f.clone();
    let clock: Arc<dyn RetryClock> = clock.clone();
    SolariBackend::new(SolariBackendOptions {
        api_key: "k".into(),
        fetch: Some(fetch),
        clock: Some(clock),
        warn: Some(warned.sink()),
        ..Default::default()
    })
}

fn spec(v: Value) -> MachineSpec {
    serde_json::from_value(v).unwrap()
}

fn created() -> Play {
    reply(201, json!({ "sandboxId": "sbx_1", "kind": "sandbox" }))
}

fn u503() -> Play {
    reply(503, json!({ "error": "u" }))
}

/// S1: a road drop sleeps `road_backoff_ms` of the count *after* it (3000 + jitter, then 6000 + jitter), warns "try 2
/// of 3" then "try 3 of 3", and the third drop is thrown with no sleep and no warn.
#[tokio::test]
async fn a_road_drop_sleeps_three_then_six_seconds_and_the_third_is_thrown_bare() {
    let f = FakeFetch::new(vec![("GET /x", vec![Play::Fail(no_road("ENOTFOUND"))])]);
    let (clock, warned) = (MovingClock::new(), Warned::default());
    let err = build(&f, &clock, &warned).request::<Value>("GET", "/x", None, None, None).await.unwrap_err();
    assert_eq!(err.to_string(), "fetch failed");
    assert_eq!(clock.waits(), vec![3125, 6125]);
    assert_eq!(f.calls().len(), 3);
    assert_eq!(
        warned.lines(),
        vec![provider_road_retry_line("GET /x", "ENOTFOUND", 2.0, 3.0), provider_road_retry_line("GET /x", "ENOTFOUND", 3.0, 3.0)]
    );
}

/// S2: a keyed call's resend after a thrown fetch sleeps `backoff_ms(attempt)` where attempt is the answers so far plus
/// one, and does not count as an answer; a road retry does not use it up.
#[tokio::test]
async fn the_keyed_resend_sleeps_by_the_answers_so_far_and_spends_no_answer() {
    let keyed = || spec(json!({ "kind": "sandbox", "idempotencyKey": "k" }));
    for (plays, waits, sends, said) in [
        (vec![u503(), u503(), Play::Fail(fetch_failed()), created()], vec![1125, 2125, 4125], 4, None),
        (vec![u503(), u503(), Play::Fail(fetch_failed()), u503()], vec![1125, 2125, 4125], 4, Some("u")),
        (
            vec![Play::Fail(no_road("ENOTFOUND")), Play::Fail(fetch_failed()), Play::Fail(fetch_failed())],
            vec![3125, 1125],
            3,
            Some("fetch failed"),
        ),
    ] {
        let f = FakeFetch::new(vec![("POST /sandboxes", plays)]);
        let (clock, warned) = (MovingClock::new(), Warned::default());
        let got = build(&f, &clock, &warned).create(keyed(), None, None).await;
        assert_eq!(clock.waits(), waits);
        assert_eq!(f.calls().len(), sends);
        assert!(f.calls().iter().all(|c| c.header("Idempotency-Key").as_deref() == Some("k")));
        match said {
            None => assert!(got.is_ok()),
            Some(m) => assert_eq!(got.err().unwrap().to_string(), m),
        }
    }
}

/// S3: the road budget is never refilled by an answer, and a road drop spends no answer try.
#[tokio::test]
async fn an_answer_never_refills_the_road_budget() {
    let road = || Play::Fail(no_road("ENOTFOUND"));
    let f = FakeFetch::new(vec![("GET /x", vec![road(), u503(), road(), u503(), road(), reply(200, json!({}))])]);
    let (clock, warned) = (MovingClock::new(), Warned::default());
    let err = build(&f, &clock, &warned).request::<Value>("GET", "/x", None, None, None).await.unwrap_err();
    assert_eq!(err.to_string(), "fetch failed");
    assert_eq!(f.calls().len(), 5);
    assert_eq!(clock.waits(), vec![3125, 1125, 6125, 2125]);
    let f = FakeFetch::new(vec![("GET /x", vec![road(), u503(), u503(), u503()])]);
    let clock = MovingClock::new();
    let err = build(&f, &clock, &Warned::default()).request::<Value>("GET", "/x", None, None, None).await.unwrap_err();
    assert_eq!((err.to_string(), f.calls().len(), clock.waits()), ("u".to_string(), 4, vec![3125, 1125, 2125]));
}

/// S4: a refusal's body: JS trim (a BOM goes, U+0085 stays), the text cut at 300 UTF-16 units after the trim; JSON that
/// is not an object, an empty or null `error` read as said nothing (`<kind> (<status>)`); a number `error` is its digits;
/// a `null` body fails with no status, as the TS's TypeError does.
#[tokio::test]
async fn a_refusals_body_is_read_as_the_ts_reads_it() {
    let a400 = "a".repeat(400);
    let b310 = format!("  {}", "b".repeat(310));
    for (status, body, want) in [
        (500, "\"Bad Gateway\"", "unknown (500)".to_string()),
        (500, "[]", "unknown (500)".to_string()),
        (500, "true", "unknown (500)".to_string()),
        (500, r#"{"error":7}"#, "7".to_string()),
        (500, r#"{"error":""}"#, "unknown (500)".to_string()),
        (500, r#"{"error":null}"#, "unknown (500)".to_string()),
        (500, r#"{"message":"m"}"#, "unknown (500)".to_string()),
        (500, " Bad ", "Bad".to_string()),
        (500, "\u{85}Bad\u{85}", "\u{85}Bad\u{85}".to_string()),
        (500, "\u{FEFF}Bad gateway\u{FEFF}", "Bad gateway".to_string()),
        (500, "  {\"error\":\"padded\"}\n", "padded".to_string()),
        (500, a400.as_str(), "a".repeat(300)),
        (500, b310.as_str(), "b".repeat(300)),
    ] {
        let f = FakeFetch::new(vec![("GET /x", vec![reply_text(status, body)])]);
        let err = build(&f, &MovingClock::new(), &Warned::default()).request::<Value>("GET", "/x", None, None, None).await.unwrap_err();
        assert_eq!(err.to_string(), want, "{body:?}");
        assert_eq!(err.status(), Some(status));
        assert_eq!(f.calls().len(), 1, "{body:?}");
    }
    // `null` throws a TypeError out of the TS's classify (no status, not retried); the port fails without a status.
    let f = FakeFetch::new(vec![("GET /x", vec![reply_text(500, "null")])]);
    let err = build(&f, &MovingClock::new(), &Warned::default()).request::<Value>("GET", "/x", None, None, None).await.unwrap_err();
    assert_eq!((err.status(), f.calls().len()), (None, 1));
    let f = FakeFetch::new(vec![("GET /x", vec![reply_text(502, "502")])]);
    let err = build(&f, &MovingClock::new(), &Warned::default()).request::<Value>("GET", "/x", None, None, None).await.unwrap_err();
    assert_eq!((err.to_string(), f.calls().len()), ("transient (502)".to_string(), 3));
}

/// S5: a 2xx body: empty is `{}`, `null` is null; blank or not JSON is an error after one send, never resent even on a
/// keyed create (the resend wraps the fetch, not the parse).
#[tokio::test]
async fn a_2xx_body_empty_is_an_empty_object_and_blank_is_an_error() {
    for (body, want) in [("", Some(json!({}))), ("null", Some(Value::Null)), ("  ", None), ("OK", None)] {
        let f = FakeFetch::new(vec![("GET /x", vec![reply_text(200, body)])]);
        let got = build(&f, &MovingClock::new(), &Warned::default()).request::<Value>("GET", "/x", None, None, None).await;
        assert_eq!(got.ok(), want, "{body:?}");
        assert_eq!(f.calls().len(), 1);
    }
    let f = FakeFetch::new(vec![("POST /sandboxes", vec![reply_text(200, "OK")])]);
    assert!(build(&f, &MovingClock::new(), &Warned::default())
        .create(spec(json!({ "kind": "sandbox", "idempotencyKey": "k" })), None, None)
        .await
        .is_err());
    assert_eq!(f.calls().len(), 1);
}

/// S6: only a GET (exactly `GET`) or a call given a cap or a signal carries an abort; a body is sent with its
/// Content-Type whenever one is given, `null` included.
#[tokio::test]
async fn only_a_get_or_a_capped_call_carries_an_abort_and_a_null_body_is_a_body() {
    let f = FakeFetch::handled(|_| Some(reply(200, json!({}))));
    let b = build(&f, &MovingClock::new(), &Warned::default());
    b.request::<Value>("DELETE", "/p", None, None, None).await.unwrap();
    b.request::<Value>("POST", "/p", Some(json!({})), None, None).await.unwrap();
    b.request::<Value>("POST", "/p", Some(json!({})), Some(5000.0), None).await.unwrap();
    b.request::<Value>("get", "/p", None, None, None).await.unwrap();
    b.request::<Value>("GET", "/p", None, None, None).await.unwrap();
    b.request::<Value>("POST", "/p", Some(Value::Null), None, None).await.unwrap();
    b.request::<Value>("POST", "/p", None, None, None).await.unwrap();
    let calls = f.calls();
    assert_eq!(calls.iter().map(|c| c.had_abort).collect::<Vec<_>>(), [false, false, true, false, true, false, false]);
    let names = |i: usize| calls[i].headers.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
    assert_eq!(names(5), ["Authorization", "Content-Type"]);
    assert_eq!(calls[5].body, Some(Value::Null));
    assert_eq!(names(6), ["Authorization"]);
    assert_eq!(calls[6].body, None);
}

/// S7: the create body sends an empty `envs` and an empty `labels` (as `metadata`), a negative or fractional idle
/// window as it is; an empty idempotency key is a key (sent as `""`), and a missing one is a fresh v4 UUID.
#[tokio::test]
async fn an_empty_map_is_sent_and_an_empty_key_is_a_key() {
    let f = FakeFetch::new(vec![("POST /sandboxes", vec![created()])]);
    let b = build(&f, &MovingClock::new(), &Warned::default());
    b.create(spec(json!({ "kind": "sandbox", "envs": {}, "labels": {}, "idempotencyKey": "" })), None, None).await.unwrap();
    b.create(spec(json!({ "kind": "sandbox", "idleTimeoutMs": 1000.5, "onIdle": "pause" })), None, None).await.unwrap();
    b.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let calls = f.calls();
    assert_eq!(calls[0].body.as_ref().unwrap().to_string(), r#"{"kind":"sandbox","envs":{},"metadata":{}}"#);
    assert_eq!(calls[0].header("Idempotency-Key").as_deref(), Some(""));
    assert_eq!(calls[1].body.as_ref().unwrap().to_string(), r#"{"kind":"sandbox","lifecycle":{"onTimeout":"pause"},"timeoutMs":1000.5}"#);
    let key = calls[2].header("Idempotency-Key").unwrap();
    let parts: Vec<&str> = key.split('-').collect();
    assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12], "{key}");
    assert!(key.chars().all(|c| c == '-' || c.is_ascii_digit() || ('a'..='f').contains(&c)), "{key}");
    assert!(parts[2].starts_with('4') && "89ab".contains(&parts[3][..1]), "{key}");
}

/// S8: the snapshot wait sleeps exactly 15 s, twelve times (waited 165 s still waits: the test is `>`), then refuses at
/// 180 s as a transient 404 keeping the reply's code and request id; each wait warns once.
#[tokio::test]
async fn the_snapshot_wait_is_twelve_sleeps_of_fifteen_seconds_then_a_transient_404() {
    let f = FakeFetch::handled(|seen| {
        if seen.path == "/snapshots" {
            return Some(reply(200, json!({ "snapshots": [{ "id": "snap_1", "sizeBytes": 1 }] })));
        }
        Some(reply_with(404, json!({ "error": "Snapshot not found", "code": "nf" }), &[("x-request-id", "rq1")]))
    });
    let (clock, warned) = (MovingClock::new(), Warned::default());
    let err = build(&f, &clock, &warned)
        .create(spec(json!({ "kind": "sandbox", "fromSnapshot": "snap_1", "idempotencyKey": "k0" })), None, None)
        .await
        .err()
        .unwrap();
    assert_eq!(clock.waits(), vec![15_000; 12]);
    assert_eq!(f.sent("POST /sandboxes"), 13);
    assert_eq!(warned.lines().len(), 12);
    assert_eq!(warned.lines()[0], snapshot_listed_wait_line("snap_1", 15_000.0, 0.0));
    assert_eq!(warned.lines()[11], snapshot_listed_wait_line("snap_1", 15_000.0, 165_000.0));
    assert_eq!(err.kind(), Some(ErrorKind::Transient));
    assert_eq!(err.status(), Some(404));
    assert_eq!((err.code(), err.request_id()), (Some("nf"), Some("rq1")));
    assert_eq!(err.to_string(), snapshot_listed_refused_line("snap_1", 180_000.0));
}

/// S9: the wait is only for a 404 whose words are exactly `Snapshot not found` on a create from a snapshot (an empty
/// `fromSnapshot` counts as one); a trailing dot or a 409 is thrown at once with no listing read.
#[tokio::test]
async fn the_snapshot_wait_needs_the_exact_words_on_a_404() {
    for (status, words) in [(404, "Snapshot not found."), (409, "Snapshot not found")] {
        let f = FakeFetch::new(vec![("POST /sandboxes", vec![reply(status, json!({ "error": words }))])]);
        let err = build(&f, &MovingClock::new(), &Warned::default())
            .create(spec(json!({ "kind": "sandbox", "fromSnapshot": "snap_1" })), None, None)
            .await
            .err()
            .unwrap();
        assert_eq!(err.to_string(), words);
        assert_eq!(f.keys(), ["POST /sandboxes"]);
    }
    let f = FakeFetch::new(vec![
        ("POST /sandboxes", vec![reply(404, json!({ "error": "Snapshot not found" }))]),
        ("GET /snapshots", vec![reply(200, json!({ "snapshots": [] }))]),
    ]);
    build(&f, &MovingClock::new(), &Warned::default())
        .create(spec(json!({ "kind": "sandbox", "fromSnapshot": "" })), None, None)
        .await
        .err()
        .unwrap();
    assert_eq!(f.keys(), ["POST /sandboxes", "GET /snapshots"]);
    assert_eq!(f.calls()[0].body.as_ref().unwrap().to_string(), r#"{"kind":"sandbox"}"#);
}

/// S10: a transient answer spent is the last answer, its words and its request id.
#[tokio::test]
async fn the_last_transient_answer_is_the_one_thrown() {
    let said = |w: &str, id: &str| reply_with(503, json!({ "error": w }), &[("x-request-id", id)]);
    let f = FakeFetch::new(vec![("GET /x", vec![said("a", "r1"), said("b", "r2"), said("c", "r3")])]);
    let clock = MovingClock::new();
    let err = build(&f, &clock, &Warned::default()).request::<Value>("GET", "/x", None, None, None).await.unwrap_err();
    assert_eq!((err.to_string(), err.request_id()), ("c".to_string(), Some("r3")));
    assert_eq!(clock.waits(), vec![1125, 2125]);
}

/// S11: a preview token's expiry: the payload decoded as node decodes base64url (the standard letters too, stop at `=`,
/// junk skipped, the whole string tried when the part before the dot holds none), the first `"exp":<digits>` read as
/// a JS number (rounded to f64, saturating into i64), else now + one hour.
#[test]
fn a_preview_token_reads_its_expiry_as_node_does() {
    let b64 = |s: &str| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, s);
    let fallback = 1000 + 3_600_000;
    for (token, want) in [
        (format!("{}.sig", b64(r#"{"exp":12,"x":"???>>>"}"#)), 12),
        (s("eyJleHAiOjV9=ignoredeyJleHAiOjZ9"), 5),
        (s("eyJl eHAiOjl9"), 9),
        (s("eyJleHAiOjV9A"), 5),
        (s("abcd.eyJleHAiOjV9"), 5),
        (format!("{}.s", b64(r#"{"exp": 77}"#)), fallback),
        (format!("{}.s", b64(r#"{"exp":"77"}"#)), fallback),
        (format!("{}.s", b64(r#"{"exp":-5}"#)), fallback),
        (format!("{}.s", b64(r#"{"exp":123.9}"#)), 123),
        (format!("{}.s", b64(r#"{"exp":1,"exp":2}"#)), 1),
        (format!("{}.s", b64(r#"{"a":{"exp":3},"exp":4}"#)), 3),
        (format!("{}.s", b64(r#"{"exp":0}"#)), 0),
        (format!("{}.s", b64(r#"{"exp":9007199254740993}"#)), 9_007_199_254_740_992),
        (format!("{}.s", b64(r#"{"exp":99999999999999999999}"#)), i64::MAX),
    ] {
        assert_eq!(preview_token_expiry(&token, 1000) as i64, want, "{token}");
    }
}

fn s(t: &str) -> String {
    t.to_string()
}

/// S12: what the backend offers and what its snapshot storage costs.
#[test]
fn the_backend_offers_the_key_check_the_listing_and_templates_and_nothing_else() {
    let f = FakeFetch::new(vec![]);
    let b = build(&f, &MovingClock::new(), &Warned::default());
    assert_eq!(b.offers(), BackendOffers { check_key: true, list_snapshots: true, templates: true, ..BackendOffers::default() });
    let storage = b.pricing().snapshot_storage();
    assert_eq!((storage.wire.free_gb.0, storage.wire.usd_per_gb_month.0, storage.wire.billed_from.as_str()), (10.0, 0.05, "2026-10-01"));
    assert_eq!((storage.free_named, storage.usd_per_named_month), (None, None));
}

/// S13: the key check over Solari: an empty 401 or 402 is refused with the kind's own words, plain text is its own
/// words; a 429, a 404 and an empty 500 are unchecked with their message; a 2xx without a templates array is unchecked.
#[tokio::test]
async fn the_key_check_reads_each_answer_of_the_template_read() {
    for (play, want, sends) in [
        (reply_text(401, ""), KeyCheck::Refused { said: s("401 auth (401)") }, 1),
        (reply(402, json!({})), KeyCheck::Refused { said: s("402 plan (402)") }, 1),
        (reply_text(401, "Unauthorized"), KeyCheck::Refused { said: s("401 Unauthorized") }, 1),
        (reply(429, json!({ "error": "slow down" })), KeyCheck::Unchecked { said: s("slow down") }, 1),
        (reply(404, json!({ "error": "no" })), KeyCheck::Unchecked { said: s("no") }, 1),
        (reply_text(500, ""), KeyCheck::Unchecked { said: s("unknown (500)") }, 1),
        (reply(503, json!({})), KeyCheck::Unchecked { said: s("transient (503)") }, 3),
        (reply(200, json!({})), KeyCheck::Unchecked { said: s("GET /templates answered without a templates array") }, 1),
        (reply_text(200, ""), KeyCheck::Unchecked { said: s("GET /templates answered without a templates array") }, 1),
    ] {
        let f = FakeFetch::new(vec![("GET /templates", vec![play])]);
        let b = build(&f, &MovingClock::new(), &Warned::default());
        assert_eq!(check_provider_key(b.as_ref()).await, want);
        assert_eq!(f.calls().len(), sends);
    }
}
