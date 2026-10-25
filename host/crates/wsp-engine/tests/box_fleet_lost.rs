// SPDX-License-Identifier: AGPL-3.0-only
//! A create whose answer is lost: asked again under the same key with the same bytes, the two-minute window, the
//! attempt's bytes made once, the replay of an attempt, and the boxes found by the name asked. Ported from
//! packages/engine/test/box-backend.test.ts:1761-1954.
mod common;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_engine::box_backend::BoxBudgetsAsk;
use wsp_engine::errors::{EngineError, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::machine::{MachineBackend, MachineKind, MachineSpec, MachineState, NamedMachine};
use wsp_wire::types::js_number::JsNumber;

use common::fake_box::*;

// packages/engine/src/labels.ts:5 WSP_LABEL, :14 OWNER_LABEL, :17 WORKSPACE_LABEL.
const WSP_LABEL: &str = "wsp";
const OWNER_LABEL: &str = "wsp-owner";
const WORKSPACE_LABEL: &str = "wsp-workspace";

const KEY: &str = "workspace/ws_1:0011223344556677";

/// box-backend.test.ts:1773 `losing`: the fake's fetch with the answers to its first `lost` creates dropped on the way
/// back: Boat made the box, and the reply never arrived.
struct Losing {
    api: Arc<FakeBox>,
    left: Mutex<i64>,
}

#[async_trait]
impl Fetch for Losing {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let creates = req.method == "POST" && url::Url::parse(&req.url).unwrap().path().ends_with("/sandboxes");
        let res = self.api.fetch(req).await?;
        if creates {
            let mut left = self.left.lock().unwrap();
            let lose = *left > 0;
            *left -= 1;
            if lose {
                return Err(dropped());
            }
        }
        Ok(res)
    }
}

/// box-backend.test.ts:1777 `backendLosing`.
fn backend_losing(api: &Arc<FakeBox>, lost: i64) -> On {
    api.on("GET", "/limits", vec![limits("standard")]);
    let fetch = Arc::new(Losing { api: api.clone(), left: Mutex::new(lost) });
    backend_with(fetch as Arc<dyn Fetch>, BoxBudgetsAsk { poll_ms: Some(10), create_ms: Some(100), restore_ms: Some(100), ..Default::default() })
}

/// box-backend.test.ts:1782 `MADE`.
fn made(id: &str, status: &str) -> Answer {
    let state = if status == "provisioning" { "provisioning" } else { "ready" };
    Answer::Reply(Reply {
        status: 202,
        body: json!({ "ok": true, "type": "sandbox.created", "status": status, "sandbox": box_view(id, state, json!({ "name": "wsp-mac-default-v4" })) }),
    })
}

fn labels(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// box-backend.test.ts:1783 `SPEC`.
fn spec() -> MachineSpec {
    MachineSpec {
        kind: MachineKind::Sandbox,
        template: None,
        from_snapshot: Some("wsp-mac-default-v4".into()),
        cpu: Some(JsNumber(2.0)),
        mem_mb: Some(JsNumber(4096.0)),
        disk_gb: None,
        envs: None,
        labels: Some(labels(&[(OWNER_LABEL, "h_1")])),
        on_idle: None,
        idle_timeout_ms: Some(JsNumber(20.0 * 60_000.0)),
        idempotency_key: Some(KEY.into()),
        engine: None,
        copy: None,
        shares: None,
        binds: None,
    }
}

fn creates(api: &FakeBox) -> Vec<Seen> {
    api.seen().into_iter().filter(|s| s.method == "POST" && s.path == "/sandboxes").collect()
}

fn rejected<T>(r: Result<T, EngineError>) -> EngineError {
    match r {
        Ok(_) => panic!("expected the call to fail"),
        Err(e) => e,
    }
}

fn iso(ms: i64) -> String {
    jiff::Timestamp::from_millisecond(ms).unwrap().to_string()
}

fn lost_line(why: &str, id: &str) -> String {
    format!("the answer to Boat's create was lost ({why}); wsp asked again and found {id}, which this create goes on with.")
}

/// box-backend.test.ts:1783
#[tokio::test]
async fn is_asked_again_at_once_with_the_same_key_and_bytes_and_the_box_boat_made_is_adopted() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![made("bx_ca2bqxuk", "ready")])
        .on("GET", "/sandboxes/bx_ca2bqxuk", vec![info("bx_ca2bqxuk", "ready", json!({ "name": "wsp-mac-default-v4" }))])
        .on("PATCH", "/sandboxes/bx_ca2bqxuk", vec![info("bx_ca2bqxuk", "ready", json!({ "name": "wsp;o=h_1" }))])
        .on("POST", "/sandboxes/bx_ca2bqxuk/commands", vec![command("", Some(0), json!({}))]);
    let on = backend_losing(&api, 1);
    let machine = on.backend.create(spec(), None, None).await.unwrap();
    assert_eq!(machine.id(), "bx_ca2bqxuk");
    // Asked again by this create, so the box is this attempt's own and not a replay of an earlier one.
    assert!(!machine.replayed());
    let asked = creates(&api);
    assert_eq!(asked.len(), 2);
    let (first, again) = (&asked[0], &asked[1]);
    assert_eq!(again.raw, first.raw);
    assert_eq!(again.headers.get("idempotency-key").map(String::as_str), Some(KEY));
    assert_eq!(first.headers.get("idempotency-key").map(String::as_str), Some(KEY));
    // Adopted: named back, and nothing deleted.
    assert_eq!(api.took("PATCH", "/sandboxes/bx_ca2bqxuk").unwrap().body, json!({ "name": "wsp;o=h_1" }));
    assert!(!api.calls().iter().any(|c| c.starts_with("DELETE")), "{:?}", api.calls());
    assert!(on.warned.lines().contains(&lost_line("fetch failed: ECONNRESET", "bx_ca2bqxuk")), "{:?}", on.warned.lines());
}

/// box-backend.test.ts:1810
#[tokio::test]
async fn waits_two_seconds_on_idempotency_in_progress_and_five_between_lost_answers_same_bytes() {
    let api = FakeBox::new();
    let on = backend_losing(&api, 2);
    let asked: Arc<Mutex<Vec<i64>>> = Arc::new(Mutex::new(Vec::new()));
    let at = |answer: Answer| {
        let (asked, clock) = (asked.clone(), on.clock.clone());
        with(move |_| {
            asked.lock().unwrap().push(clock.now_ms());
            answer.clone()
        })
    };
    api.on(
        "POST",
        "/sandboxes",
        vec![
            at(made("bx_1", "provisioning")),
            at(made("bx_1", "provisioning")),
            at(Answer::Reply(error(409, "idempotency_in_progress", "A request with this key is still being processed"))),
            at(made("bx_1", "ready")),
        ],
    )
    .on("GET", "/sandboxes/bx_1", vec![info("bx_1", "ready", json!({ "name": "wsp;o=h_1" }))])
    .on("POST", "/sandboxes/bx_1/commands", vec![command("", Some(0), json!({}))]);
    assert_eq!(on.backend.create(spec(), None, None).await.unwrap().id(), "bx_1");
    let raws: Vec<Option<String>> = creates(&api).into_iter().map(|s| s.raw).collect();
    assert_eq!(raws.len(), 4);
    assert!(raws.iter().all(|r| *r == raws[0]), "{raws:?}");
    let asked = asked.lock().unwrap().clone();
    let gaps: Vec<i64> = asked.windows(2).map(|w| w[1] - w[0]).collect();
    assert_eq!(gaps, vec![0, 5_000, 2_000]);
}

/// box-backend.test.ts:1834
#[tokio::test]
async fn a_500_is_asked_again_as_a_lost_answer_and_a_refusal_below_500_is_not() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![Answer::Reply(error(500, "http_500", "Internal error")), made("bx_1", "ready")])
        .on("GET", "/sandboxes/bx_1", vec![info("bx_1", "ready", json!({ "name": "wsp;o=h_1" }))])
        .on("POST", "/sandboxes/bx_1/commands", vec![command("", Some(0), json!({}))]);
    let on = backend_losing(&api, 0);
    assert_eq!(on.backend.create(spec(), None, None).await.unwrap().id(), "bx_1");
    assert!(on.warned.lines().contains(&lost_line("500 Internal error", "bx_1")), "{:?}", on.warned.lines());
    let raws: Vec<Option<String>> = creates(&api).into_iter().map(|s| s.raw).collect();
    assert!(raws.iter().all(|r| *r == raws[0]), "{raws:?}");
    assert_eq!(raws.len(), 2);

    let refused = FakeBox::new().on("POST", "/sandboxes", vec![error(409, "idempotency_key_reused", "This key was used with another body").into()]);
    let e = rejected(backend_losing(&refused, 0).backend.create(spec(), None, None).await);
    assert!(e.to_string().contains("This key was used with another body"), "{e}");
    assert_eq!(creates(&refused).len(), 1);
}

/// box-backend.test.ts:1855 `silent`: the first create goes out and nothing answers it until its own abort fires.
struct Silent {
    api: Arc<FakeBox>,
    silent: Mutex<Vec<String>>,
}

#[async_trait]
impl Fetch for Silent {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let creates = req.method == "POST" && req.url.ends_with("/sandboxes");
        if !creates || !self.silent.lock().unwrap().is_empty() {
            return self.api.fetch(req).await;
        }
        self.silent.lock().unwrap().push(String::from_utf8(req.body.clone().expect("a body")).unwrap());
        let abort = req.abort.expect("a create carries a cap");
        abort.token.cancelled().await;
        let name = if abort.timed_out.load(Ordering::SeqCst) { "TimeoutError" } else { "AbortError" };
        Err(RoadError { name: name.into(), message: "The operation was aborted due to timeout".into(), code: None, cause: None })
    }
}

/// box-backend.test.ts:1855
#[tokio::test]
async fn an_ask_that_hears_nothing_inside_a_reads_cap_is_a_lost_answer_asked_again_with_same_key_and_bytes() {
    let api = FakeBox::new()
        .on("GET", "/limits", vec![limits("standard")])
        .on("POST", "/sandboxes", vec![made("bx_1", "ready")])
        .on("GET", "/sandboxes/bx_1", vec![info("bx_1", "ready", json!({ "name": "wsp;o=h_1" }))])
        .on("POST", "/sandboxes/bx_1/commands", vec![command("", Some(0), json!({}))]);
    let silent = Arc::new(Silent { api: api.clone(), silent: Mutex::new(Vec::new()) });
    let on = backend_with(
        silent.clone() as Arc<dyn Fetch>,
        BoxBudgetsAsk { poll_ms: Some(10), create_ms: Some(100), restore_ms: Some(100), read_ms: Some(50), ..Default::default() },
    );
    assert_eq!(on.backend.create(spec(), None, None).await.unwrap().id(), "bx_1");
    assert!(on.warned.lines().contains(&lost_line("The operation was aborted due to timeout", "bx_1")), "{:?}", on.warned.lines());
    let silent = silent.silent.lock().unwrap().clone();
    let reached: Vec<(Option<String>, Option<String>)> = creates(&api).into_iter().map(|s| (s.raw, s.headers.get("idempotency-key").cloned())).collect();
    assert_eq!(reached, vec![(Some(silent[0].clone()), Some(KEY.to_string()))]);
}

/// box-backend.test.ts:1874
#[tokio::test]
async fn an_answer_lost_after_two_minutes_fails_the_create_with_the_roads_words_and_deletes_nothing() {
    let api = FakeBox::new().on("POST", "/sandboxes", vec![made("bx_1", "provisioning")]);
    let on = backend_losing(&api, 1_000);
    let began = on.clock.now_ms();
    let e = rejected(on.backend.create(spec(), None, None).await);
    assert!(e.to_string().contains("fetch failed"), "{e}");
    let took = on.clock.now_ms() - began;
    assert!(took <= 120_000, "{took}");
    assert!(took >= 115_000, "{took}");
    let raws: Vec<Option<String>> = creates(&api).into_iter().map(|s| s.raw).collect();
    assert!(raws.iter().all(|r| *r == raws[0]), "{raws:?}");
    assert!(!api.calls().iter().any(|c| c.starts_with("DELETE") || c.starts_with("GET /sandboxes/")), "{:?}", api.calls());
}

/// box-backend.test.ts:1885
#[tokio::test]
async fn an_attempts_bytes_are_made_once_a_create_handed_them_sends_them_as_they_are() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![made("bx_1", "provisioning")])
        .on("GET", "/sandboxes/bx_1", vec![info("bx_1", "ready", json!({ "name": "wsp;o=h_1" }))])
        .on("POST", "/sandboxes/bx_1/commands", vec![command("", Some(0), json!({}))]);
    let on = backend_losing(&api, 0);
    // The first read of the account's cap takes a second and a half, between the instant the timer is set for and the
    // clock the seconds to it are read off.
    let clock = on.clock.clone();
    api.on(
        "GET",
        "/limits",
        vec![with(move |_| {
            clock.advance(1_500);
            limits("standard")
        })],
    );
    let sent = on.backend.create_request(&spec()).await.unwrap();
    let parsed: Value = serde_json::from_str(&sent).unwrap();
    assert_eq!(parsed, json!({ "type": "small", "ttlSeconds": 1199, "noEnv": true, "name": "wsp;o=h_1", "from": "wsp-mac-default-v4" }));
    // Read again with the cap held, the same spec asks another stop timer: the body a replay must not send.
    let again: Value = serde_json::from_str(&on.backend.create_request(&spec()).await.unwrap()).unwrap();
    assert_eq!(again["ttlSeconds"], json!(1200));
    on.backend.create(spec(), None, Some(sent.clone())).await.unwrap();
    assert_eq!(creates(&api)[0].raw.as_deref(), Some(sent.as_str()));
}

/// box-backend.test.ts:1905
#[tokio::test]
async fn a_replay_sends_the_attempts_bytes_under_its_key_names_the_box_back_and_hands_it_back() {
    let api = FakeBox::new()
        .on(
            "POST",
            "/sandboxes",
            vec![Answer::Reply(Reply {
                status: 200,
                body: json!({ "ok": true, "type": "sandbox.created", "status": "ready", "sandbox": box_view("bx_sxtfdf4c", "ready", json!({ "name": "wsp-mac-default-v4" })) }),
            })],
        )
        .on("PATCH", "/sandboxes/bx_sxtfdf4c", vec![info("bx_sxtfdf4c", "ready", json!({ "name": "wsp;o=h_1" }))]);
    let on = backend_losing(&api, 0);
    let mut owned = spec();
    owned.labels = Some(labels(&[(OWNER_LABEL, "h_1"), (WORKSPACE_LABEL, "ws_1")]));
    let sent = on.backend.create_request(&owned).await.unwrap();
    let machine = on.backend.replay_create(KEY, &sent, &iso(on.clock.now_ms())).await.unwrap().expect("the box Boat made");
    assert_eq!(machine.id(), "bx_sxtfdf4c");
    assert_eq!(machine.labels(), Some(&labels(&[(WSP_LABEL, "1"), (OWNER_LABEL, "h_1"), (WORKSPACE_LABEL, "ws_1")])));
    let asked = creates(&api);
    assert_eq!(asked[0].raw.as_deref(), Some(sent.as_str()));
    assert_eq!(asked[0].headers.get("idempotency-key").map(String::as_str), Some(KEY));
    let sent_body: Value = serde_json::from_str(&sent).unwrap();
    assert_eq!(api.took("PATCH", "/sandboxes/bx_sxtfdf4c").unwrap().body, json!({ "name": sent_body["name"] }));
}

/// box-backend.test.ts:1919
#[tokio::test]
async fn a_replay_of_an_attempt_as_old_as_boat_keeps_a_key_sends_nothing_and_answers_no_box() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![made("bx_new", "provisioning")])
        .on("PATCH", "/sandboxes/bx_new", vec![info("bx_new", "ready", json!({ "name": "wsp;o=h_1" }))]);
    let on = backend_losing(&api, 0);
    let mut owned = spec();
    owned.labels = Some(labels(&[(OWNER_LABEL, "h_1"), (WORKSPACE_LABEL, "ws_1")]));
    let sent = on.backend.create_request(&owned).await.unwrap();
    let made_at = |ms: i64| iso(on.clock.now_ms() - ms);
    assert!(on.backend.replay_create(KEY, &sent, &made_at(24 * 3_600_000 - 125_000)).await.unwrap().is_none());
    assert!(creates(&api).is_empty());
    let replayed = on.backend.replay_create(KEY, &sent, &made_at(24 * 3_600_000 - 126_000)).await.unwrap().expect("inside the key's window");
    assert_eq!(replayed.id(), "bx_new");
    assert_eq!(creates(&api).len(), 1);
}

/// box-backend.test.ts:1930
#[tokio::test]
async fn lists_the_boxes_named_exactly_one_of_the_names_asked_running_or_archived_and_no_other() {
    let api = FakeBox::new().on(
        "GET",
        "/sandboxes",
        vec![with(|seen| {
            let sandboxes = if seen.query_get("state") == Some("archived") {
                vec![box_view("bx_old", "archived", json!({ "name": "wsp-mac-default-v3", "createdAt": "2026-10-11T09:00:00.000Z" }))]
            } else {
                vec![
                    box_view("bx_ca2bqxuk", "ready", json!({ "name": "wsp-mac-default-v4", "createdAt": "2026-10-11T09:48:00.000Z" })),
                    box_view("bx_copy", "ready", json!({ "name": "wsp-mac-default-v4-copy" })),
                    box_view("bx_other", "ready", json!({ "name": "wsp-otherhost-default-v4" })),
                    box_view("bx_person", "ready", json!({ "name": "my box" })),
                    box_view("bx_ours", "ready", json!({ "name": "wsp;o=h_1" })),
                ]
            };
            Answer::Reply(Reply { status: 200, body: json!({ "ok": true, "sandboxes": sandboxes, "pageInfo": { "nextCursor": null } }) })
        })],
    );
    let on = backend_losing(&api, 0);
    let names = ["wsp-mac-default-v4", "wsp-mac-default-v3"].iter().map(|n| n.to_string()).collect();
    let named = on.backend.named_after(&names).await.unwrap();
    assert_eq!(
        named,
        vec![
            NamedMachine { id: "bx_ca2bqxuk".into(), name: "wsp-mac-default-v4".into(), state: MachineState::Running, created_at: Some("2026-10-11T09:48:00.000Z".into()) },
            NamedMachine { id: "bx_old".into(), name: "wsp-mac-default-v3".into(), state: MachineState::Paused, created_at: Some("2026-10-11T09:00:00.000Z".into()) },
        ]
    );
}
