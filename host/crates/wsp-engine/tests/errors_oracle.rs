// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of errors against the TS's own answers and the spec's formulas (spec/errors.md).
#[path = "oracle/mod.rs"]
mod oracle;

use wsp_engine::errors::*;

fn road(code_on_error: Option<&str>, cause_code: Option<&str>) -> EngineError {
    EngineError::Road(RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: code_on_error.map(str::to_string),
        cause: cause_code.map(|c| RoadCause { message: format!("connect {c}"), code: Some(c.to_string()), errors: None }),
    })
}

fn aggregate(codes: &[Option<&str>]) -> EngineError {
    EngineError::Road(RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause {
            message: "".into(),
            code: None,
            errors: Some(
                codes
                    .iter()
                    .map(|c| RoadCause { message: format!("{c:?} on one address"), code: c.map(str::to_string), errors: None })
                    .collect(),
            ),
        }),
    })
}

#[test]
fn classify_reads_every_status_the_way_the_ts_does_with_each_field() {
    for case in oracle::load("errors.json")["classify"].as_array().unwrap() {
        let status = case["status"].as_u64().unwrap() as u16;
        let got = classify(status, case["code"].as_str(), case["error"].as_str(), case["requestId"].as_str());
        let want = &case["out"];
        let kind: ErrorKind = serde_json::from_value(want["kind"].clone()).unwrap();
        assert_eq!(got.kind, kind, "{case}");
        assert_eq!(got.status, status);
        assert_eq!(got.code.as_deref(), want["code"].as_str(), "{case}");
        assert_eq!(got.message, oracle::s(&want["message"]), "{case}");
        // An empty request id rides as given; only an absent one is absent.
        assert_eq!(got.request_id.as_deref(), want.get("requestId").and_then(|v| v.as_str()), "{case}");
        assert_eq!(got.window, None);
        assert_eq!(got.said, None);
    }
}

#[test]
fn fetch_cap_ms_rounds_up_never_below_zero_and_never_past_the_timer() {
    for case in oracle::load("errors.json")["fetchCapMs"].as_array().unwrap() {
        assert_eq!(fetch_cap_ms(case["in"].as_f64().unwrap()), case["out"].as_u64().unwrap(), "{case}");
    }
}

#[test]
fn each_typed_error_says_the_ts_sentence() {
    let m = &oracle::load("errors.json")["messages"];
    let e = EngineError::NotFirstLife { machine_id: "m1".into(), action: "snapshot".into() };
    assert_eq!(e.to_string(), oracle::s(&m["notFirstLife"]));
    let e = EngineError::EnvNotLanded { machine_id: "m1".into(), exit_code: 124, detail: "timed out".into() };
    assert_eq!(e.to_string(), oracle::s(&m["envNotLanded"]));
    // The lines below come from wsp-words; the variant carries the line it was made with.
    assert_eq!(wsp_words::words::thread::guest_unusable_line("box", "m1", "the shell died"), oracle::s(&m["guestUnusable"]));
    assert_eq!(wsp_words::words::thread::exec_failed_line("exec failed"), oracle::s(&m["execFailed"]));
    assert_eq!(wsp_words::words::thread::stop_refused_line("its latest snapshot failed"), oracle::s(&m["stopRefused"]));
    assert_eq!(wsp_words::words::thread::nap_refused_line("no"), oracle::s(&m["napRefused"]));
    assert_eq!(wsp_words::words::thread::machine_unreached_line(4.0, 31_500.0), oracle::s(&m["unreached"]));
    assert_eq!(PROVIDER_READ_CAP_MS, 20_000);
    assert_eq!(ROAD_TRIES, 3);
}

#[test]
fn backoff_doubles_from_half_a_second_to_eight_and_adds_up_to_249_ms_of_draw() {
    let want = [
        (0, 0.0, 500),
        (1, 0.0, 1000),
        (2, 0.0, 2000),
        (3, 0.0, 4000),
        (4, 0.0, 8000),
        (5, 0.0, 8000),
        (9, 0.0, 8000),
        (1, 0.999_999, 1249),
        (0, 0.5, 625),
    ];
    for (attempt, draw, ms) in want {
        assert_eq!(backoff_ms(attempt, draw), ms, "attempt {attempt} draw {draw}");
    }
}

#[test]
fn road_backoff_is_three_seconds_a_retry_plus_the_draw() {
    assert_eq!(road_backoff_ms(1, 0.0), 3000);
    assert_eq!(road_backoff_ms(2, 0.0), 6000);
    assert_eq!(road_backoff_ms(2, 0.999_999), 6249);
    assert_eq!(road_backoff_ms(0, 0.0), 0);
}

#[test]
fn should_retry_is_a_transient_answer_before_the_third_attempt_only() {
    let w = |kind| WspError { kind, status: 502, code: None, message: String::new(), request_id: None, window: None, said: None };
    assert!(should_retry(&w(ErrorKind::Transient), 1));
    assert!(should_retry(&w(ErrorKind::Transient), 2));
    assert!(!should_retry(&w(ErrorKind::Transient), 3));
    assert!(!should_retry(&w(ErrorKind::Concurrency), 1));
    assert!(!should_retry(&w(ErrorKind::SnapshotUnavailable), 1));
}

#[test]
fn a_network_failure_is_read_off_the_error_or_its_cause_and_a_gateway_answer_is_not_one() {
    for code in NETWORK_CODES {
        assert!(is_network_error(&road(None, Some(code))), "cause {code}");
        assert!(is_network_error(&EngineError::Other { message: "socket".into(), code: Some(code.to_string()) }), "own {code}");
    }
    // "fetch failed" alone is the network, whatever its cause.
    assert!(is_network_error(&road(None, None)));
    assert!(is_network_error(&road(None, Some("EWHATEVER"))));
    assert!(!is_network_error(&EngineError::Other { message: "nope".into(), code: Some("EACCES".into()) }));
    let gateway = EngineError::Provider(classify(502, None, Some("bad gateway"), None));
    assert!(!is_network_error(&gateway));
}

#[test]
fn never_sent_is_only_the_four_codes_of_a_call_that_never_left() {
    for code in UNSENT_CODES {
        assert!(never_sent(&road(None, Some(code))), "{code}");
    }
    assert!(!never_sent(&road(None, Some("ECONNRESET"))));
    assert!(!never_sent(&road(None, Some("ETIMEDOUT"))));
    assert!(!never_sent(&road(None, None)));
}

#[test]
fn the_far_end_answered_where_the_error_carries_a_kind_or_a_status() {
    assert!(far_end_answered(&EngineError::Provider(classify(404, None, None, None))));
    let carried = Carried { kind: None, status: Some(409), code: None, request_id: None };
    assert!(far_end_answered(&EngineError::MachineStopped { machine_id: "m".into(), message: "x".into(), carried }));
    assert!(far_end_answered(&EngineError::GuestUnusable {
        machine_id: "m".into(),
        provider: "box".into(),
        detail: "d".into(),
        status: 200,
        line: "l".into()
    }));
    assert!(!far_end_answered(&road(None, Some("ECONNRESET"))));
    assert!(!far_end_answered(&EngineError::Other { message: "Bad Gateway".into(), code: None }));
}

#[test]
fn road_code_names_the_first_offline_code_only_when_every_address_failed_offline() {
    assert_eq!(road_code(&road(None, Some("ENOTFOUND"))).as_deref(), Some("ENOTFOUND"));
    assert_eq!(road_code(&road(None, Some("ECONNREFUSED"))), None);
    assert_eq!(road_code(&road(None, None)), None);
    assert_eq!(road_code(&aggregate(&[Some("ENETUNREACH"), Some("EHOSTUNREACH")])).as_deref(), Some("ENETUNREACH"));
    assert_eq!(road_code(&aggregate(&[Some("ENETUNREACH"), Some("ECONNREFUSED")])), None);
    assert_eq!(road_code(&aggregate(&[Some("ENETUNREACH"), None])), None);
    // An empty AggregateError names nothing.
    assert_eq!(road_code(&aggregate(&[])), None);
    // Only a TypeError "fetch failed" is a road at all.
    let other = EngineError::Road(RoadError { name: "Error".into(), message: "fetch failed".into(), code: None, cause: None });
    assert_eq!(road_code(&other), None);
    assert!(road_failed(&road(None, Some("EAI_AGAIN"))));
    assert!(!road_failed(&road(None, Some("ETIMEDOUT"))));
}

#[test]
fn is_capped_reads_the_timeout_name_and_nothing_else() {
    let capped = EngineError::Road(RoadError {
        name: "TimeoutError".into(),
        message: "The operation was aborted due to timeout".into(),
        code: None,
        cause: None,
    });
    assert!(is_capped(&capped));
    assert!(!is_capped(&aborted()));
    assert!(!is_capped(&road(None, Some("ETIMEDOUT"))));
}

#[test]
fn aborted_is_nodes_abort_error_with_no_string_code() {
    let e = aborted();
    assert_eq!(e.to_string(), "This operation was aborted");
    assert_eq!(e.name(), "AbortError");
    assert_eq!(e.code(), None);
}

#[test]
fn failure_words_spell_out_what_fetch_hid_in_its_cause() {
    assert_eq!(failure_words(&road(None, Some("ECONNREFUSED"))), "fetch failed: connect ECONNREFUSED");
    let agg = aggregate(&[Some("ENETUNREACH"), Some("EHOSTUNREACH")]);
    assert_eq!(failure_words(&agg), "fetch failed: Some(\"ENETUNREACH\") on one address; Some(\"EHOSTUNREACH\") on one address");
    assert_eq!(failure_words(&road(None, None)), "fetch failed");
    assert_eq!(failure_words(&EngineError::Other { message: "plain".into(), code: None }), "plain");
}

#[test]
fn the_wrappers_carry_the_providers_kind_status_code_and_request_id_and_its_message() {
    let w = WspError {
        kind: ErrorKind::Conflict,
        status: 409,
        code: Some("sandbox_restoring".into()),
        message: "restoring".into(),
        request_id: Some("req_9".into()),
        window: None,
        said: None,
    };
    let carried = Carried { kind: Some(w.kind), status: Some(w.status), code: w.code.clone(), request_id: w.request_id.clone() };
    let e = EngineError::RestoreUnfinished { machine_id: "bx_1".into(), message: w.message.clone(), carried };
    assert_eq!(e.to_string(), "restoring");
    assert_eq!(e.kind(), Some(ErrorKind::Conflict));
    assert_eq!(e.status(), Some(409));
    assert_eq!(e.code(), Some("sandbox_restoring"));
    assert_eq!(e.request_id(), Some("req_9"));
    assert_eq!(e.name(), "RestoreUnfinishedError");
    let resume = EngineError::ResumeUnanswered { message: "x".into(), cause: None };
    assert!(resume.is_move_unanswered());
    let exec = EngineError::MachineUnreachable {
        machine_id: "m".into(),
        said: "exec failed".into(),
        status: 502,
        line: "l".into(),
        exec_failed: true,
    };
    assert!(exec.is_machine_unreachable());
    assert_eq!(exec.name(), "ExecFailedError");
}

#[test]
fn missing_and_account_refusals_read_the_kind_alone() {
    assert!(is_missing(&EngineError::Provider(classify(404, None, None, None))));
    assert!(!is_missing(&EngineError::Other { message: "404".into(), code: None }));
    assert!(is_account_refusal(&EngineError::Provider(classify(401, None, None, None))));
    assert!(is_account_refusal(&EngineError::Provider(classify(403, None, None, None))));
    assert!(!is_account_refusal(&EngineError::Provider(classify(429, None, None, None))));
}

#[test]
fn the_constructors_make_the_ts_lines() {
    let m = &oracle::load("errors.json")["messages"];
    let e = EngineError::guest_unusable("m1", "box", "the shell died", 500);
    assert_eq!(e.to_string(), oracle::s(&m["guestUnusable"]));
    assert_eq!((e.status(), e.name()), (Some(500), "GuestUnusableError"));
    let e = EngineError::machine_unreachable("m1", "Sandbox is not reachable", 502, "LINE");
    assert_eq!(e.to_string(), oracle::s(&m["machineUnreachable"]));
    assert!(e.is_machine_unreachable());
    assert_eq!(e.name(), "MachineUnreachableError");
    let e = EngineError::exec_failed("m1", "exec failed", 502);
    assert_eq!(e.to_string(), oracle::s(&m["execFailed"]));
    assert!(e.is_machine_unreachable());
    assert_eq!(EngineError::stop_refused("m1", "its latest snapshot failed").to_string(), oracle::s(&m["stopRefused"]));
    assert_eq!(EngineError::nap_refused("m1", "no").to_string(), oracle::s(&m["napRefused"]));
    let e = EngineError::unreached(4, 31_500, aborted());
    assert_eq!(e.to_string(), oracle::s(&m["unreached"]));
    // A plain error carries nothing onto the wrapper; its message becomes the wrapper's.
    let e = EngineError::machine_stopped("m1", &EngineError::Other { message: "Box machine is not running.".into(), code: None });
    assert_eq!(e.to_string(), "Box machine is not running.");
    assert_eq!((e.kind(), e.status(), e.code(), e.request_id()), (None, None, None, None));
    let cause = EngineError::Provider(classify(409, Some("machine_not_running"), Some("stopped"), None));
    let e = EngineError::machine_stopped("m1", &cause);
    assert_eq!((e.kind(), e.status(), e.code(), e.request_id()), (Some(ErrorKind::Conflict), Some(409), Some("machine_not_running"), None));
    assert_eq!(e.name(), "MachineStoppedError");
}

#[tokio::test(start_paused = true)]
async fn abort_gives_a_fresh_cap_each_call_and_says_which_fired() {
    assert!(abort(None, None).is_none());
    let first = abort(Some(1000.0), None).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    let second = abort(Some(1000.0), None).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(first.token.is_cancelled());
    assert!(first.timed_out.load(std::sync::atomic::Ordering::SeqCst));
    assert!(!second.token.is_cancelled(), "the second call got the whole cap again");
    let caller = tokio_util::sync::CancellationToken::new();
    let both = abort(Some(1000.0), Some(&caller)).unwrap();
    caller.cancel();
    tokio::task::yield_now().await;
    assert!(both.token.is_cancelled());
    assert!(!both.timed_out.load(std::sync::atomic::Ordering::SeqCst), "the caller's cancel is no timeout");
    let only = abort(None, Some(&caller)).unwrap();
    assert!(only.token.is_cancelled());
}
