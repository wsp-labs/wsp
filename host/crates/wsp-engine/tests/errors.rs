// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/errors.test.ts.
mod common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use common::fake_clock::FakeClock;
use wsp_engine::errors::{
    classify, is_network_error, should_retry, until_reached, EngineError, ErrorKind, RoadCause, RoadError, WspError,
};
use wsp_words::words::thread::{exec_failed_line, machine_unreachable_line, machine_unreached_line};

/// A TypeError "fetch failed" whose cause carries `code`, as node's fetch rejects.
fn fetch_failed(cause: Option<(&str, &str)>) -> EngineError {
    EngineError::Road(RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: cause.map(|(message, code)| RoadCause { message: message.into(), code: Some(code.into()), errors: None }),
    })
}

/// `Object.assign(new Error(message), classify(status, { error: message }))`.
fn refusal(status: u16, message: &str) -> EngineError {
    EngineError::Provider(classify(status, None, Some(message), None))
}

/// The link rule's waits, each with the jitter it draws on top of its own base.
fn expect_backoffs(waits: &[u64], bases: &[u64]) {
    assert_eq!(waits.len(), bases.len(), "{waits:?}");
    for (i, wait) in waits.iter().enumerate() {
        assert!(*wait >= bases[i], "wait {i} is {wait}, under {}", bases[i]);
        assert!(*wait < bases[i] + 250, "wait {i} is {wait}, not under {}", bases[i] + 250);
    }
}

/// errors.test.ts:15
#[test]
fn never_retries_429() {
    let e = classify(429, Some("ConcurrencyLimitExceeded"), Some("Too many concurrent sessions"), None);
    assert_eq!(e.kind, ErrorKind::Concurrency);
    assert!(!should_retry(&e, 1));
}

/// errors.test.ts:20
#[test]
fn retries_generic_502_up_to_3_attempts() {
    let e = classify(502, None, Some("upstream sad"), None);
    assert!(should_retry(&e, 1));
    assert!(!should_retry(&e, 3));
}

/// errors.test.ts:25
#[test]
fn does_not_retry_the_snapshot_502_signature() {
    let e = classify(502, None, Some("Failed to snapshot sandbox"), None);
    assert_eq!(e.kind, ErrorKind::SnapshotUnavailable);
    assert!(!should_retry(&e, 1));
}

/// errors.test.ts:30
#[test]
fn maps_402_and_404_to_permanent() {
    assert_eq!(classify(402, Some("InsufficientCredit"), None, None).kind, ErrorKind::Plan);
    assert_eq!(classify(404, None, None, None).kind, ErrorKind::Missing);
}

/// errors.test.ts:37
#[test]
fn exec_failed_is_the_unreachable_base_with_that_answers_own_sentence() {
    let e = EngineError::MachineUnreachable {
        machine_id: "m1".into(),
        said: "exec failed".into(),
        status: 502,
        line: exec_failed_line("exec failed"),
        exec_failed: true,
    };
    assert!(e.is_machine_unreachable());
    assert_eq!(e.to_string(), exec_failed_line("exec failed"));
    assert_eq!(e.name(), "ExecFailedError");
    let EngineError::MachineUnreachable { machine_id, said, status, .. } = &e else { panic!("{e:?}") };
    assert_eq!((machine_id.as_str(), said.as_str(), *status), ("m1", "exec failed", 502));
}

/// errors.test.ts:44
#[test]
fn unreachable_keeps_the_sentence_it_was_handed_and_is_no_exec_failed() {
    let said = "Sandbox is not reachable";
    let e = EngineError::MachineUnreachable {
        machine_id: "m1".into(),
        said: said.into(),
        status: 502,
        line: machine_unreachable_line(said),
        exec_failed: false,
    };
    assert_eq!(e.to_string(), machine_unreachable_line(said));
    assert_eq!(e.name(), "MachineUnreachableError");
    assert!(matches!(&e, EngineError::MachineUnreachable { machine_id, said: s, status: 502, exec_failed: false, .. } if machine_id == "m1" && s == said));
}

/// errors.test.ts:53
#[test]
fn is_network_error_reads_a_fetch_that_threw_or_a_socket_or_dns_code_on_the_error_or_its_cause() {
    assert!(is_network_error(&fetch_failed(Some(("getaddrinfo EAI_AGAIN api.example", "EAI_AGAIN")))));
    assert!(is_network_error(&fetch_failed(None)));
    assert!(is_network_error(&EngineError::Other { message: "read ECONNRESET".into(), code: Some("ECONNRESET".into()) }));
    let terminated = EngineError::Road(RoadError {
        name: "TypeError".into(),
        message: "terminated".into(),
        code: None,
        cause: Some(RoadCause { message: "other side closed".into(), code: Some("UND_ERR_SOCKET".into()), errors: None }),
    });
    assert!(is_network_error(&terminated));
}

/// errors.test.ts:59. The TS case's last line, `isNetworkError("fetch failed")` of a bare string, has no Rust form:
/// the function takes an EngineError.
#[test]
fn is_network_error_is_not_an_answer_or_a_plain_error() {
    assert!(!is_network_error(&refusal(503, "upstream sad")));
    assert!(!is_network_error(&refusal(404, "gone")));
    assert!(!is_network_error(&refusal(409, "paused")));
    assert!(!is_network_error(&refusal(502, "Failed to snapshot sandbox")));
    assert!(!is_network_error(&EngineError::Other { message: "exit 1: no such file".into(), code: None }));
}

/// A clock the retry's own sleeps move, and a call that costs five seconds and fails `times` times.
/// errors.test.ts:71 `fixture`. The clock's draw is the middle of the jitter, so a wait is its base plus 125.
struct Fixture {
    clock: Arc<FakeClock>,
    calls: AtomicU32,
    times: u32,
    make: fn() -> EngineError,
}

impl Fixture {
    fn new(times: u32, make: fn() -> EngineError) -> Self {
        let clock = FakeClock::at(0);
        *clock.draw.lock().unwrap() = 0.5;
        Fixture { clock, calls: AtomicU32::new(0), times, make }
    }
    async fn once(&self) -> Result<String, EngineError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        self.clock.advance(5_000);
        if n <= self.times {
            return Err((self.make)());
        }
        Ok("answered".to_string())
    }
    fn calls(&self) -> u32 {
        self.calls.load(Ordering::SeqCst)
    }
}

/// errors.test.ts:94
#[tokio::test]
async fn retries_a_call_nothing_answered_at_the_link_backoff_and_returns_the_answer_that_came() {
    let f = Fixture::new(2, || fetch_failed(None));
    let once = &f;
    let answer = until_reached(|| once.once(), &*f.clock).await.unwrap();
    assert_eq!(answer, "answered");
    assert_eq!(f.calls(), 3);
    expect_backoffs(&f.clock.sleeps(), &[500, 1_000]);
}

/// errors.test.ts:101
#[tokio::test]
async fn rethrows_a_failure_that_is_not_the_network_at_once_after_one_call_and_no_wait() {
    let f = Fixture::new(9, || refusal(404, "gone"));
    let once = &f;
    let e = until_reached(|| once.once(), &*f.clock).await.unwrap_err();
    assert!(e.to_string().contains("gone"), "{e}");
    assert_eq!(f.calls(), 1);
    assert_eq!(f.clock.sleeps(), Vec::<u64>::new());
}

/// errors.test.ts:108
#[tokio::test]
async fn gives_up_once_the_next_wait_would_end_past_the_link_window_with_the_count_and_the_time() {
    let f = Fixture::new(99, || fetch_failed(Some(("getaddrinfo EAI_AGAIN", "EAI_AGAIN"))));
    let once = &f;
    let e = until_reached(|| once.once(), &*f.clock).await.unwrap_err();
    let EngineError::Unreached { attempts, elapsed_ms, line, cause } = &e else { panic!("{e:?}") };
    // Five-second calls with the rule's waits between them: the seventh ends a minute and half a second in, past
    // the window a name that will not resolve is given, so no eighth is made.
    assert_eq!(*attempts, 7);
    expect_backoffs(&f.clock.sleeps(), &[500, 1_000, 2_000, 4_000, 8_000, 10_000]);
    assert!(*elapsed_ms >= 7 * 5_000 + 25_500, "{elapsed_ms}");
    assert!(*elapsed_ms < 7 * 5_000 + 27_000, "{elapsed_ms}");
    assert_eq!(*line, machine_unreached_line(7.0, *elapsed_ms as f64));
    assert_eq!(e.to_string(), *line);
    assert_eq!(cause.to_string(), "fetch failed");
    assert_eq!(f.calls(), 7);
}

/// errors.test.ts:126
#[test]
fn the_request_id_rides_on_the_error_when_the_reply_carried_one() {
    let e = classify(502, None, Some("Failed to snapshot sandbox"), Some("req_7"));
    assert_eq!(
        e,
        WspError {
            kind: ErrorKind::SnapshotUnavailable,
            status: 502,
            code: None,
            message: "Failed to snapshot sandbox".into(),
            request_id: Some("req_7".into()),
            window: None,
            said: None,
        }
    );
}

/// errors.test.ts:126. Absent, not empty: no `requestId` key on the error's own JSON either.
#[test]
fn the_request_id_is_absent_not_empty_when_the_reply_carried_none() {
    let e = classify(502, None, Some("Failed to snapshot sandbox"), None);
    assert_eq!(e.request_id, None);
    let json = serde_json::to_value(&e).unwrap();
    assert!(json.get("requestId").is_none(), "{json}");
}
