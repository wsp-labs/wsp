// SPDX-License-Identifier: AGPL-3.0-only
//! Ports the exec cases and the four describe blocks about a machine's exec, pause and resume of
//! packages/engine/test/solari-backend.test.ts: exec on a machine the provider cannot reach, pause the provider
//! refuses, a pause and a resume the provider does not answer, and the cap the caller puts on one call.
#[path = "rest_common/mod.rs"]
mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::Engine as _;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use wsp_engine::errors::{fetch_cap_ms, is_capped, EngineError, ErrorKind, RetryClock, PROVIDER_READ_CAP_MS};
use wsp_engine::golden_seam::EXEC_ENV;
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineBackend, MachineSpec};
use wsp_engine::solari_backend::{MoveBudgets, MoveBudgetsGiven, SolariBackend, SolariBackendOptions, RESUME_CAP_MS, SOLARI_INLINE_MAX_MS};
use wsp_words::words::computer::RESUME_UNANSWERED;
use wsp_words::words::thread::{exec_failed_line, machine_unreachable_line, nap_refused_line};

use common::{fetch_failed, reply, FakeFetch, FrozenClock, NoWaitClock, Play, Warned};

fn sandbox() -> MachineSpec {
    serde_json::from_value(json!({ "kind": "sandbox" })).unwrap()
}

fn backend(f: &Arc<FakeFetch>) -> Arc<SolariBackend> {
    SolariBackend::new(SolariBackendOptions { api_key: "k".into(), fetch: Some(f.clone()), ..Default::default() })
}

fn backend_on(f: &Arc<FakeFetch>, clock: Arc<dyn RetryClock>) -> Arc<SolariBackend> {
    SolariBackend::new(SolariBackendOptions { api_key: "k".into(), fetch: Some(f.clone()), clock: Some(clock), ..Default::default() })
}

fn exec_opts(timeout_ms: u64) -> ExecOptions {
    ExecOptions { timeout_ms: Some(timeout_ms), ..Default::default() }
}

fn body_of(f: &FakeFetch, call: usize) -> Value {
    f.calls()[call].body.clone().unwrap()
}

// ---- exec

// TS: packages/engine/test/solari-backend.test.ts:173
#[tokio::test]
async fn every_exec_runs_under_bash_c_with_home_and_user_exported_and_no_shell() {
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", reply(201, json!({ "sandboxId": "x", "kind": "sandbox" }))),
        ("POST /sandboxes/x/exec", reply(200, json!({ "exitCode": 0, "stdout": "", "stderr": "" }))),
    ]);
    let b = backend(&f);
    let m = b.create(sandbox(), None, None).await.unwrap();
    m.exec("go env GOPATH", exec_opts(5_000)).await.unwrap();
    let body = body_of(&f, 1);
    assert_eq!(body, json!({ "cmd": "bash", "args": ["-c", format!("{EXEC_ENV}\ngo env GOPATH")], "timeoutMs": 5_000 }));
    assert_eq!(EXEC_ENV, "export HOME=/root USER=root");
    assert!(!body["args"][1].as_str().unwrap().contains("SHELL"));
    assert_eq!(body["args"][0], "-c");
}

// TS: packages/engine/test/solari-backend.test.ts:187
#[tokio::test]
async fn an_exec_asked_for_longer_than_the_providers_cap_runs_detached_under_the_cap() {
    let asked: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let polls = Arc::new(AtomicUsize::new(0));
    let (asked_in, polls_in) = (asked.clone(), polls.clone());
    let f = FakeFetch::handled(move |seen| match seen.key().as_str() {
        "POST /sandboxes" => Some(reply(201, json!({ "sandboxId": "x", "kind": "sandbox" }))),
        "POST /sandboxes/x/exec" => {
            let body = seen.body.clone().unwrap();
            asked_in.lock().unwrap().push(body["timeoutMs"].as_u64().unwrap());
            let cmd = body["args"][1].as_str().unwrap().to_string();
            if cmd.contains("echo WSP_LAUNCHED") {
                return Some(reply(200, json!({ "exitCode": 0, "stdout": "WSP_LAUNCHED\n", "stderr": "" })));
            }
            if cmd.contains("echo WSP_POLL") {
                // The first poll carries the output and no exit code yet; the second carries the code and nothing new.
                let first = polls_in.fetch_add(1, Ordering::SeqCst) == 0;
                let out = if first { base64::engine::general_purpose::STANDARD.encode("2.1.280\n") } else { String::new() };
                let code = if first { "" } else { "0" };
                return Some(reply(
                    200,
                    json!({ "exitCode": 0, "stdout": format!("WSP_POLL\n{code}\n{out}\n\nup\nWSP_POLL_END\n"), "stderr": "" }),
                ));
            }
            Some(reply(200, json!({ "exitCode": 0, "stdout": "", "stderr": "" })))
        }
        _ => None,
    });
    let b = backend(&f);
    let m = b.create(sandbox(), None, None).await.unwrap();
    let res = m.exec("claude --version", exec_opts(30_000)).await.unwrap();
    assert_eq!(res, ExecResult { exit_code: 0, stdout: "2.1.280\n".into(), stderr: String::new() });
    // The provider answers 400 to any of these above the cap, so the whole road has to stay under it.
    let asked = asked.lock().unwrap().clone();
    let over: Vec<u64> = asked.iter().copied().filter(|ms| *ms > SOLARI_INLINE_MAX_MS).collect();
    assert_eq!(over, Vec::<u64>::new());
    assert!(asked.len() > 1);
    assert_eq!(SOLARI_INLINE_MAX_MS, 26_000);
}

// TS: packages/engine/test/solari-backend.test.ts:216
#[tokio::test]
async fn an_exec_at_the_cap_is_one_call_with_the_timeout_it_asked_for() {
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", reply(201, json!({ "sandboxId": "x", "kind": "sandbox" }))),
        ("POST /sandboxes/x/exec", reply(200, json!({ "exitCode": 0, "stdout": "here\n", "stderr": "" }))),
    ]);
    let b = backend(&f);
    let m = b.create(sandbox(), None, None).await.unwrap();
    let res = m.exec("echo here", exec_opts(SOLARI_INLINE_MAX_MS)).await.unwrap();
    assert_eq!(res, ExecResult { exit_code: 0, stdout: "here\n".into(), stderr: String::new() });
    let bodies: Vec<Value> = f.calls().iter().skip(1).map(|c| c.body.clone().unwrap()).collect();
    assert_eq!(bodies, vec![json!({ "cmd": "bash", "args": ["-c", format!("{EXEC_ENV}\necho here")], "timeoutMs": SOLARI_INLINE_MAX_MS })]);
}

// ---- exec on a machine the provider cannot reach

fn refusing_exec(status: u16, error: &str) -> Arc<FakeFetch> {
    FakeFetch::fixed(vec![
        ("POST /sandboxes", reply(201, json!({ "sandboxId": "sbx_1", "kind": "sandbox" }))),
        ("POST /sandboxes/sbx_1/exec", reply(status, json!({ "error": error }))),
    ])
}

fn exec_calls(f: &FakeFetch) -> usize {
    f.calls().iter().filter(|c| c.url.ends_with("/exec")).count()
}

// TS: packages/engine/test/solari-backend.test.ts:453
#[tokio::test]
async fn a_502_with_the_providers_words_is_the_typed_refusal_once_the_gateway_retries_are_spent() {
    let f = refusing_exec(502, "Sandbox is not reachable");
    let m = backend_on(&f, NoWaitClock::new()).create(sandbox(), None, None).await.unwrap();
    let e = m.exec("true", ExecOptions::default()).await.unwrap_err();
    assert_eq!(e.name(), "MachineUnreachableError");
    assert_eq!(
        e,
        EngineError::MachineUnreachable {
            machine_id: "sbx_1".into(),
            said: "Sandbox is not reachable".into(),
            status: 502,
            line: machine_unreachable_line("Sandbox is not reachable"),
            exec_failed: false,
        }
    );
    assert!(e.is_machine_unreachable());
    assert_eq!(e.to_string(), machine_unreachable_line("Sandbox is not reachable"));
    assert_eq!(exec_calls(&f), 3);
}

// TS: packages/engine/test/solari-backend.test.ts:463
#[tokio::test]
async fn the_same_words_on_a_400_are_the_same_refusal_carrying_the_400() {
    let f = refusing_exec(400, "Sandbox is not reachable");
    let m = backend_on(&f, NoWaitClock::new()).create(sandbox(), None, None).await.unwrap();
    let e = m.exec("true", ExecOptions::default()).await.unwrap_err();
    assert_eq!(e.name(), "MachineUnreachableError");
    assert_eq!(
        e,
        EngineError::MachineUnreachable {
            machine_id: "sbx_1".into(),
            said: "Sandbox is not reachable".into(),
            status: 400,
            line: machine_unreachable_line("Sandbox is not reachable"),
            exec_failed: false,
        }
    );
    assert_eq!(e.to_string(), machine_unreachable_line("Sandbox is not reachable"));
    assert_eq!(exec_calls(&f), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:472
#[tokio::test]
async fn a_502_with_any_other_words_stays_the_plain_error_it_was() {
    let f = refusing_exec(502, "Bad gateway");
    let m = backend_on(&f, NoWaitClock::new()).create(sandbox(), None, None).await.unwrap();
    let e = m.exec("true", ExecOptions::default()).await.unwrap_err();
    assert_eq!(e.name(), "Error");
    assert_eq!(e.to_string(), "Bad gateway");
    assert_eq!(e.kind(), Some(ErrorKind::Transient));
    assert_eq!(e.status(), Some(502));
    assert_eq!(exec_calls(&f), 3);
}

// TS: packages/engine/test/solari-backend.test.ts:480
#[tokio::test]
async fn a_502_exec_failed_is_the_same_refusal_with_its_own_sentence_once_the_gateway_retries_are_spent() {
    let f = refusing_exec(502, "exec failed");
    let m = backend_on(&f, NoWaitClock::new()).create(sandbox(), None, None).await.unwrap();
    let e = m.exec("true", ExecOptions::default()).await.unwrap_err();
    assert!(matches!(e, EngineError::MachineUnreachable { exec_failed: true, .. }));
    assert!(e.is_machine_unreachable());
    assert_eq!(e.name(), "ExecFailedError");
    assert_eq!(
        e,
        EngineError::MachineUnreachable {
            machine_id: "sbx_1".into(),
            said: "exec failed".into(),
            status: 502,
            line: exec_failed_line("exec failed"),
            exec_failed: true,
        }
    );
    assert_eq!(e.to_string(), exec_failed_line("exec failed"));
    assert_eq!(exec_calls(&f), 3);
}

// TS: packages/engine/test/solari-backend.test.ts:491
#[tokio::test]
async fn exec_failed_on_any_other_status_stays_the_plain_error_it_was() {
    let f = refusing_exec(500, "exec failed");
    let m = backend_on(&f, NoWaitClock::new()).create(sandbox(), None, None).await.unwrap();
    let e = m.exec("true", ExecOptions::default()).await.unwrap_err();
    assert!(!e.is_machine_unreachable());
    assert_eq!(e.name(), "Error");
    assert_eq!(e.to_string(), "exec failed");
    assert_eq!(e.status(), Some(500));
}

// ---- pause the provider refuses

fn refusing_pause(error: &str) -> Arc<FakeFetch> {
    FakeFetch::fixed(vec![
        ("POST /sandboxes", reply(201, json!({ "sandboxId": "sbx_1", "kind": "sandbox" }))),
        ("POST /sandboxes/sbx_1/pause", reply(409, json!({ "error": error }))),
    ])
}

fn pause_calls(f: &FakeFetch) -> usize {
    f.calls().iter().filter(|c| c.url.ends_with("/pause")).count()
}

// TS: packages/engine/test/solari-backend.test.ts:508
#[tokio::test]
async fn a_409_whose_words_are_not_pausable_is_the_typed_refusal_asked_once() {
    let f = refusing_pause("Not pausable");
    let m = backend(&f).create(sandbox(), None, None).await.unwrap();
    let e = m.pause(None).await.unwrap_err();
    assert_eq!(e.name(), "NapRefusedError");
    assert_eq!(
        e,
        EngineError::NapRefused { machine_id: "sbx_1".into(), said: "Not pausable".into(), line: nap_refused_line("Not pausable") }
    );
    assert_eq!(e.to_string(), nap_refused_line("Not pausable"));
    assert_eq!(pause_calls(&f), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:518
#[tokio::test]
async fn a_409_with_any_other_words_stays_the_plain_conflict_it_was() {
    let f = refusing_pause("sandbox is not running");
    let m = backend(&f).create(sandbox(), None, None).await.unwrap();
    let e = m.pause(None).await.unwrap_err();
    assert!(!matches!(e, EngineError::NapRefused { .. }));
    assert_eq!(e.name(), "Error");
    assert_eq!(e.to_string(), "sandbox is not running");
    assert_eq!(e.kind(), Some(ErrorKind::Conflict));
    assert_eq!(e.status(), Some(409));
}

// ---- a pause and a resume the provider does not answer

const PAUSE: &str = "POST /sandboxes/x/pause";
const RESUME: &str = "POST /sandboxes/x/resume";
const STATE: &str = "GET /sandboxes/x";
const PAUSE_MS: f64 = 40.0;
const STATE_READ_MS: f64 = 20.0;

fn reads(state: &str) -> Play {
    reply(200, json!({ "sandboxId": "x", "kind": "sandbox", "state": state }))
}

fn created() -> Play {
    reply(201, json!({ "sandboxId": "x", "kind": "sandbox" }))
}

fn ok() -> Play {
    common::reply_text(200, "{}")
}

fn no_road() -> Play {
    Play::Fail(fetch_failed())
}

fn scripted(routes: Vec<(&str, Vec<Play>)>) -> Arc<FakeFetch> {
    FakeFetch::new(routes)
}

/// A machine handle off a create, so the first read the test sees is the move's own.
async fn machine(f: &Arc<FakeFetch>, pause_ms: Option<f64>, clock: Option<Arc<dyn RetryClock>>, warned: &Warned) -> Arc<dyn Machine> {
    let b = SolariBackend::new(SolariBackendOptions {
        api_key: "k".into(),
        fetch: Some(f.clone()),
        clock,
        budgets: MoveBudgetsGiven {
            pause_ms: Some(pause_ms.unwrap_or(PAUSE_MS)),
            resume_cap_ms: Some(20.0),
            state_read_ms: Some(STATE_READ_MS),
            read_ms: None,
        },
        warn: Some(warned.sink()),
        ..Default::default()
    });
    b.create(sandbox(), None, None).await.unwrap()
}

/// `^pause did not complete in \d+ms; <rest>$`
fn assert_pause_words(message: &str, rest: &str) {
    let after = message.strip_prefix("pause did not complete in ").unwrap_or_else(|| panic!("not the pause words: {message}"));
    let digits = after.chars().take_while(|c| c.is_ascii_digit()).count();
    assert!(digits > 0, "no milliseconds in: {message}");
    assert_eq!(&after[digits..], format!("ms; {rest}"));
}

fn cause_is_capped(e: &EngineError) -> bool {
    match e {
        EngineError::MoveUnanswered { cause: Some(c), .. } | EngineError::ResumeUnanswered { cause: Some(c), .. } => is_capped(c),
        _ => false,
    }
}

// TS: packages/engine/test/solari-backend.test.ts:789
#[tokio::test]
async fn a_pause_the_provider_never_finishes_is_sent_twice_and_ends_unanswered_with_the_rows_words() {
    let f = scripted(vec![("POST /sandboxes", vec![created()]), (PAUSE, vec![Play::Hangs]), (STATE, vec![reads("running")])]);
    let warned = Warned::default();
    let m = machine(&f, None, None, &warned).await;
    let started = Instant::now();
    let err = m.pause(None).await.unwrap_err();
    assert!(err.is_move_unanswered());
    assert!(matches!(err, EngineError::MoveUnanswered { .. }));
    assert_pause_words(&err.to_string(), "the provider did not answer and reads the machine running; try again");
    assert!(cause_is_capped(&err));
    assert!(started.elapsed().as_millis() as f64 >= PAUSE_MS - 5.0);
    assert_eq!(f.sent(PAUSE), 2);
    assert_eq!(f.sent(STATE), 2);
}

// TS: packages/engine/test/solari-backend.test.ts:807
#[tokio::test]
async fn a_pause_whose_call_never_answers_but_landed_is_a_pause_one_call_one_read() {
    let f = scripted(vec![("POST /sandboxes", vec![created()]), (PAUSE, vec![Play::Hangs]), (STATE, vec![reads("paused")])]);
    let m = machine(&f, None, None, &Warned::default()).await;
    m.pause(None).await.unwrap();
    assert_eq!(f.sent(PAUSE), 1);
    assert_eq!(f.sent(STATE), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:815
#[tokio::test]
async fn a_provider_that_cannot_be_read_after_the_call_ends_the_move_at_once_with_that_in_the_words() {
    let f = scripted(vec![("POST /sandboxes", vec![created()]), (PAUSE, vec![Play::Hangs]), (STATE, vec![no_road()])]);
    let m = machine(&f, None, None, &Warned::default()).await;
    let err = m.pause(None).await.unwrap_err();
    assert_pause_words(&err.to_string(), "the provider did not answer and could not be read about the machine; try again");
    assert_eq!(f.sent(PAUSE), 1);
    assert_eq!(f.sent(STATE), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:828
#[tokio::test]
async fn a_read_that_hangs_is_given_up_on_within_its_own_bound() {
    let f = scripted(vec![("POST /sandboxes", vec![created()]), (PAUSE, vec![Play::Hangs]), (STATE, vec![Play::Hangs])]);
    let m = machine(&f, None, None, &Warned::default()).await;
    let started = Instant::now();
    let err = m.pause(None).await.unwrap_err();
    assert!(err.to_string().contains("could not be read about the machine"), "{err}");
    // Half the pause budget on the call, then the read's own bound, and nothing more.
    assert!((started.elapsed().as_millis() as f64) < PAUSE_MS + STATE_READ_MS + 500.0);
    assert_eq!(f.sent(PAUSE), 1);
    assert_eq!(f.sent(STATE), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:844
#[tokio::test]
async fn a_pause_that_fails_on_a_network_error_is_read_and_sent_once_more_then_ends_with_the_words() {
    let f = scripted(vec![("POST /sandboxes", vec![created()]), (PAUSE, vec![no_road(), no_road()]), (STATE, vec![reads("running")])]);
    let m = machine(&f, None, None, &Warned::default()).await;
    let err = m.pause(None).await.unwrap_err();
    assert_pause_words(&err.to_string(), "the provider did not answer and reads the machine running; try again");
    assert_eq!(f.sent(PAUSE), 2);
    assert_eq!(f.sent(STATE), 2);
}

// TS: packages/engine/test/solari-backend.test.ts:857
#[tokio::test]
async fn a_pause_that_fails_once_on_a_network_error_and_lands_on_the_second_call_is_a_pause() {
    let f = scripted(vec![("POST /sandboxes", vec![created()]), (PAUSE, vec![no_road(), ok()]), (STATE, vec![reads("running")])]);
    let m = machine(&f, None, None, &Warned::default()).await;
    m.pause(None).await.unwrap();
    assert_eq!(f.sent(PAUSE), 2);
    assert_eq!(f.sent(STATE), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:865
#[tokio::test]
async fn a_pause_the_provider_refuses_is_not_retried_and_an_odd_millisecond_is_no_reason_to_fail() {
    let f = scripted(vec![
        ("POST /sandboxes", vec![created()]),
        (PAUSE, vec![reply(400, json!({ "error": "upstream request timeout" }))]),
        (STATE, vec![reads("running")]),
    ]);
    // The timer refuses the 19.5ms this budget leaves the first attempt unless the cap is made whole.
    let m = machine(&f, Some(39.0), Some(Arc::new(FrozenClock)), &Warned::default()).await;
    let err = m.pause(None).await.unwrap_err();
    assert_eq!(err.to_string(), "upstream request timeout");
    assert!(!err.is_move_unanswered());
    assert_eq!(f.sent(PAUSE), 1);
    assert_eq!(f.sent(STATE), 0);
}

// TS: packages/engine/test/solari-backend.test.ts:876
#[tokio::test]
async fn a_resume_whose_call_never_answers_but_landed_goes_through_one_call_one_read() {
    let f = scripted(vec![("POST /sandboxes", vec![created()]), (RESUME, vec![Play::Hangs]), (STATE, vec![reads("running")])]);
    let m = machine(&f, None, None, &Warned::default()).await;
    m.resume(None).await.unwrap();
    assert_eq!(f.sent(RESUME), 1);
    assert_eq!(f.sent(STATE), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:884
#[tokio::test]
async fn a_resume_cut_off_at_its_cap_with_the_machine_still_paused_ends_unanswered_nothing_sent_twice() {
    let f = scripted(vec![("POST /sandboxes", vec![created()]), (RESUME, vec![Play::Hangs]), (STATE, vec![reads("paused")])]);
    let m = machine(&f, None, None, &Warned::default()).await;
    let err = m.resume(None).await.unwrap_err();
    assert!(matches!(err, EngineError::ResumeUnanswered { .. }));
    assert!(err.is_move_unanswered());
    assert_eq!(err.to_string(), RESUME_UNANSWERED);
    assert!(cause_is_capped(&err));
    assert_eq!(f.sent(RESUME), 1);
    assert_eq!(f.sent(STATE), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:901
#[tokio::test]
async fn a_resume_the_network_dropped_is_read_like_a_capped_one_and_starting_is_a_resume_that_landed() {
    let f = scripted(vec![("POST /sandboxes", vec![created()]), (RESUME, vec![no_road()]), (STATE, vec![reads("starting")])]);
    let m = machine(&f, None, None, &Warned::default()).await;
    m.resume(None).await.unwrap();
    assert_eq!(f.sent(RESUME), 1);
    assert_eq!(f.sent(STATE), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:909
#[test]
fn ships_four_minutes_for_a_pause_half_a_minute_for_a_resume_and_a_read_and_takes_shorter_for_tests() {
    let b = backend(&FakeFetch::new(vec![]));
    assert_eq!(
        b.budgets(),
        MoveBudgets {
            pause_ms: 4.0 * 60_000.0,
            resume_cap_ms: RESUME_CAP_MS as f64,
            state_read_ms: 30_000.0,
            read_ms: PROVIDER_READ_CAP_MS as f64
        }
    );
    let short = SolariBackend::new(SolariBackendOptions {
        api_key: "k".into(),
        fetch: Some(FakeFetch::new(vec![])),
        budgets: MoveBudgetsGiven { pause_ms: Some(40.0), ..Default::default() },
        ..Default::default()
    });
    assert_eq!(
        short.budgets(),
        MoveBudgets { pause_ms: 40.0, resume_cap_ms: 30_000.0, state_read_ms: 30_000.0, read_ms: PROVIDER_READ_CAP_MS as f64 }
    );
}

// ---- the cap the caller puts on one call

// TS: packages/engine/test/solari-backend.test.ts:917
#[tokio::test]
async fn a_resume_carries_its_cap_a_pause_half_its_budget_and_a_read_the_reads_cap_into_the_fetch() {
    let f = FakeFetch::handled(|_| Some(reply(200, json!({ "sandboxId": "x", "kind": "sandbox", "state": "paused" }))));
    let b = backend(&f);
    let m = b.get("x").await.unwrap();
    m.pause(None).await.unwrap();
    m.resume(None).await.unwrap();
    let seen: Vec<bool> = f.calls().iter().map(|c| c.had_abort).collect();
    assert_eq!(seen, vec![true, true, true]);
}

// TS: packages/engine/test/solari-backend.test.ts:930
#[tokio::test]
async fn the_callers_own_signal_ends_the_call_so_a_resume_nobody_waits_on_is_not_left_running() {
    let stop = CancellationToken::new();
    let stop_in = stop.clone();
    let f = FakeFetch::handled(move |_| {
        stop_in.cancel();
        Some(Play::Hangs)
    });
    let b = backend(&f);
    // The cap rides the same fetch, so the failure is the stop's own and not the cap's: the runtime tells a wake
    // the person ended from one the provider never answered by exactly this.
    let failed = b.request::<Value>("POST", "/sandboxes/x/resume", Some(json!({})), Some(30_000.0), Some(stop)).await.unwrap_err();
    assert!(!is_capped(&failed));
    assert_eq!(failed.name(), "AbortError");
    assert_eq!(f.calls().len(), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:948
#[tokio::test]
async fn a_provider_that_answers_nothing_inside_the_cap_ends_the_resume_there() {
    let f = FakeFetch::handled(|_| Some(Play::Hangs));
    let b = backend(&f);
    let failed = b.request::<Value>("POST", "/sandboxes/x/resume", Some(json!({})), Some(20.0), None).await.unwrap_err();
    assert!(is_capped(&failed));
    assert_eq!(f.calls().len(), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:962
#[test]
fn the_cap_a_fetch_is_given_is_a_whole_number_of_milliseconds_inside_the_timers_range() {
    assert_eq!(fetch_cap_ms(19.5), 20);
    assert_eq!(fetch_cap_ms(30_000.0), 30_000);
    assert_eq!(fetch_cap_ms(2f64.powi(32)), 2_147_483_647);
    assert_eq!(fetch_cap_ms(-1.0), 0);
}
