// SPDX-License-Identifier: AGPL-3.0-only
//! spec/status_probe.md; the probe cases of packages/runtime/test/status.test.ts (:177-216, :303-386, :1994-2018) and
//! its socket filter case (:1495-1499), at the level of the free functions they exercise.
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde_json::json;
use support::net::{http_stub, Delay};
use support::status::{event, missing, offline, other, refused_port};
use support::{run_local, settle};
use wsp_core::clock::{Clock, FakeClock};
use wsp_core::status::{
    ask_daemon, bounded, cost_moved, facts_backoff_ms, machine_state_of, missed, phase_leaving_gone, probe_reach, provider_said, DaemonAnswers,
    ProbeOptions, Probed, COST_INTERVAL_MS, DAEMON_ANSWER, FACTS_BACKOFF_MAX_MS, POLL_INTERVAL_MS, PROBE_TIMEOUT_MS, PROMPT_MS, RECONCILE_MIN_MS,
    ZOMBIE_PROBE_CMD, ZOMBIE_PROBE_TIMEOUT_MS, ZOMBIE_WINDOW_MS,
};
use wsp_engine::errors::EngineError;
use wsp_wire::views::workspace::{MachineState, ReachState, WorkspacePhase};

fn probed(state: ReachState, from_daemon: bool, offline: bool) -> Probed {
    Probed { state, from_daemon, offline }
}

/// A clock read that jumps by `took` between the probe's first and second read: the request landed late.
fn stepping(took: i64) -> impl Fn() -> i64 {
    let calls = Cell::new(0);
    move || {
        let n = calls.get();
        calls.set(n + 1);
        if n == 0 {
            0
        } else {
            took
        }
    }
}

#[test]
fn machine_state_of_maps_every_phase() {
    use WorkspacePhase::*;
    let read: Vec<MachineState> = [Running, Pausing, Napping, Waking, Queued, Gone, Released].into_iter().map(machine_state_of).collect();
    use MachineState as M;
    assert_eq!(read, [M::Running, M::Running, M::Paused, M::Starting, M::Starting, M::Gone, M::Gone]);
}

#[test]
fn phase_leaving_gone_reads_running_and_paused_only() {
    assert_eq!(phase_leaving_gone(MachineState::Running), Some(WorkspacePhase::Running));
    assert_eq!(phase_leaving_gone(MachineState::Paused), Some(WorkspacePhase::Napping));
    assert_eq!(phase_leaving_gone(MachineState::Starting), None);
    assert_eq!(phase_leaving_gone(MachineState::Gone), None);
}

#[test]
fn provider_said_quotes_the_status_and_the_providers_own_words() {
    assert_eq!(provider_said(&missing("sandbox sb_1 not found")), "404 sandbox sb_1 not found");
    let unreachable = EngineError::MachineUnreachable {
        machine_id: "m1".into(),
        said: "the guest agent is not answering".into(),
        status: 502,
        line: "wsp's own sentence".into(),
        exec_failed: false,
    };
    assert_eq!(provider_said(&unreachable), "502 the guest agent is not answering", "the typed refusal quotes the provider, not wsp");
    assert_eq!(provider_said(&other("socket hang up")), "socket hang up");
}

#[test]
fn missed_marks_offline_only_a_request_that_never_left_this_computer() {
    assert_eq!(missed(&offline()), probed(ReachState::Unreachable, false, true));
    assert_eq!(missed(&refused_port()), probed(ReachState::Unreachable, false, false));
    assert_eq!(missed(&other("timed out after 10 ms")), probed(ReachState::Unreachable, false, false));
}

#[test]
fn maps_a_prompt_502_to_no_daemon_a_late_answer_to_slow_silence_to_unreachable_and_any_other_answer_to_reachable() {
    run_local(async {
        let cases: [(u16, i64, &str); 8] = [
            (426, 0, "reachable"),
            (502, 0, "no-daemon"),
            // Not the guest's answer, so it decides nothing on its own: an edge refusal must not take a live workspace
            // out of service in the row, it only sends the poll to the provider for the machine's real state.
            (200, 0, "reachable"),
            (401, 0, "reachable"),
            (404, 0, "reachable"),
            (503, 0, "reachable"),
            (502, 150, "slow"),
            (426, 150, "slow"),
        ];
        for (code, took, expected) in cases {
            let stub = http_stub(code, Delay::Ms(0)).await;
            let got = probe_reach(&stub.url(), ProbeOptions { prompt_ms: 60, timeout_ms: 5_000 }, &stepping(took)).await;
            assert_eq!(serde_json::to_value(got.state).unwrap(), json!(expected), "{code} after {took}");
            assert_eq!(got.from_daemon, code == 426, "{code}");
            assert!(!got.offline);
            assert_eq!(stub.hits.get(), 1);
            stub.close();
        }
        // At the prompt exactly is not late: only past it.
        let stub = http_stub(502, Delay::Ms(0)).await;
        let at = probe_reach(&stub.url(), ProbeOptions { prompt_ms: 60, timeout_ms: 5_000 }, &stepping(60)).await;
        assert_eq!(at.state, ReachState::NoDaemon);
    });
}

#[test]
fn a_426_is_the_daemons_own_answer() {
    run_local(async {
        let stub = http_stub(426, Delay::Ms(0)).await;
        let got = probe_reach(&stub.url(), ProbeOptions { prompt_ms: 2_500, timeout_ms: 5_000 }, &|| 0).await;
        assert_eq!(got, probed(ReachState::Reachable, true, false));
    });
}

#[test]
fn the_probe_tells_this_computers_road_from_the_machines_silence() {
    run_local(async {
        let probe = ProbeOptions { prompt_ms: 2_500, timeout_ms: 300 };
        let held = http_stub(426, Delay::Never).await;
        assert_eq!(probe_reach(&held.url(), probe, &|| 0).await, probed(ReachState::Unreachable, false, false));
        // A port that takes the connection and drops it: the machine's miss, not this computer's.
        let dropping = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = dropping.local_addr().unwrap().port();
        tokio::task::spawn_local(async move {
            while let Ok((sock, _)) = dropping.accept().await {
                drop(sock);
            }
        });
        assert_eq!(probe_reach(&format!("http://127.0.0.1:{port}/"), probe, &|| 0).await, probed(ReachState::Unreachable, false, false));
        // A name that resolves nowhere never leaves this computer.
        assert_eq!(probe_reach("http://no-such-host.invalid/", probe, &|| 0).await, probed(ReachState::Unreachable, false, true));
    });
}

fn answers(fc: &FakeClock, up: Result<bool, EngineError>, took: i64, asked: &Rc<RefCell<Vec<Option<i64>>>>) -> DaemonAnswers {
    let (fc, asked) = (fc.clone(), asked.clone());
    Rc::new(move |timeout| {
        asked.borrow_mut().push(timeout);
        fc.advance(took);
        let up = up.clone();
        Box::pin(async move { up })
    })
}

#[test]
fn a_guest_that_takes_longer_than_the_prompt_reads_slow_and_one_inside_it_reads_reachable() {
    run_local(async {
        let fc = FakeClock::new(1_000);
        let clock: Rc<dyn Clock> = fc.clock();
        let asked = Rc::new(RefCell::new(Vec::new()));
        let o = ProbeOptions { prompt_ms: 2_500, timeout_ms: 10_000 };
        assert_eq!(ask_daemon(&answers(&fc, Ok(true), 2_500, &asked), o, &clock).await, probed(ReachState::Reachable, true, false));
        assert_eq!(ask_daemon(&answers(&fc, Ok(true), 2_501, &asked), o, &clock).await, probed(ReachState::Slow, true, false));
        assert_eq!(ask_daemon(&answers(&fc, Ok(false), 2_501, &asked), o, &clock).await, probed(ReachState::Slow, false, false));
        assert_eq!(*asked.borrow(), vec![Some(10_000); 3], "the bound rides into the machine's own ask");
    });
}

#[test]
fn a_guest_that_says_its_port_is_dead_reads_no_daemon() {
    run_local(async {
        let fc = FakeClock::new(1_000);
        let clock: Rc<dyn Clock> = fc.clock();
        let asked = Rc::new(RefCell::new(Vec::new()));
        let o = ProbeOptions { prompt_ms: 2_500, timeout_ms: 10_000 };
        assert_eq!(ask_daemon(&answers(&fc, Ok(false), 0, &asked), o, &clock).await, probed(ReachState::NoDaemon, false, false));
        assert_eq!(ask_daemon(&answers(&fc, Err(offline()), 0, &asked), o, &clock).await, probed(ReachState::Unreachable, false, true));
        assert_eq!(ask_daemon(&answers(&fc, Err(refused_port()), 0, &asked), o, &clock).await, probed(ReachState::Unreachable, false, false));
    });
}

#[test]
fn the_callers_bound_is_the_whole_ask() {
    run_local(async {
        let fc = FakeClock::new(1_000);
        let clock: Rc<dyn Clock> = fc.clock();
        let never: DaemonAnswers = Rc::new(|_| Box::pin(std::future::pending()));
        let o = ProbeOptions { prompt_ms: 2_500, timeout_ms: 300 };
        let asking = tokio::task::spawn_local(async move { ask_daemon(&never, o, &clock).await });
        settle().await;
        fc.advance(299);
        settle().await;
        assert!(!asking.is_finished());
        fc.advance(1);
        settle().await;
        assert_eq!(asking.await.unwrap(), probed(ReachState::Unreachable, false, false), "a bound that ran out is the machine's silence");
    });
}

#[test]
fn bounded_leaves_the_work_running_past_its_bound() {
    run_local(async {
        let fc = FakeClock::new(0);
        let clock: Rc<dyn Clock> = fc.clock();
        let (finish, finished) = tokio::sync::oneshot::channel::<()>();
        let ran = Rc::new(Cell::new(false));
        let r = ran.clone();
        let work = Box::pin(async move {
            let _ = finished.await;
            r.set(true);
            Ok::<u32, EngineError>(7)
        });
        let c = clock.clone();
        let waiting = tokio::task::spawn_local(async move { bounded(work, 100, &c).await });
        settle().await;
        fc.advance(100);
        settle().await;
        let answer = waiting.await.unwrap();
        assert_eq!(answer.err().unwrap().to_string(), "timed out after 100 ms");
        assert!(!ran.get());
        finish.send(()).unwrap();
        settle().await;
        assert!(ran.get(), "the bound answered; the work was not cancelled");
        let quick = bounded(Box::pin(async { Ok::<u32, EngineError>(3) }), 100, &clock).await;
        assert_eq!(quick.unwrap(), 3);
    });
}

#[test]
fn facts_backoff_doubles_from_one_poll_to_five_minutes() {
    assert_eq!((PROBE_TIMEOUT_MS, PROMPT_MS, RECONCILE_MIN_MS), (10_000, 2_500, 300_000));
    assert_eq!((ZOMBIE_WINDOW_MS, ZOMBIE_PROBE_TIMEOUT_MS, POLL_INTERVAL_MS), (180_000, 20_000, 15_000));
    assert_eq!((ZOMBIE_PROBE_CMD, FACTS_BACKOFF_MAX_MS, COST_INTERVAL_MS, DAEMON_ANSWER), ("echo ok", 300_000, 5_000, 426));
    let waits: Vec<i64> = (0..=7).map(facts_backoff_ms).collect();
    assert_eq!(waits, [15_000, 15_000, 30_000, 60_000, 120_000, 240_000, 300_000, 300_000]);
}

fn cost(id: &str, phase: &str, rate: f64, accrued: f64) -> wsp_wire::wire::events_union::EventUnion {
    event(json!({"type": "workspace.cost", "workspaceId": id, "phase": phase, "rateUsdPerHour": rate, "awakeMs": 1, "accruedUsd": accrued, "at": "1970-01-01T00:00:00.000Z"}))
}

#[test]
fn cost_moved_lets_a_figure_through_once_and_forgets_a_deleted_workspace() {
    let mut moved = cost_moved();
    let tick = cost("ws_a", "napping", 0.0, 0.5);
    let deleted = event(json!({"type": "workspace.deleted", "workspaceId": "ws_a"}));
    assert_eq!([moved.admit(&tick), moved.admit(&tick), moved.admit(&deleted), moved.admit(&tick)], [true, false, true, true]);
    // Each workspace keeps its own figures, and a figure that moves back is a move.
    assert!(moved.admit(&cost("ws_b", "napping", 0.0, 0.5)));
    assert!(moved.admit(&cost("ws_a", "running", 0.0, 0.5)));
    assert!(moved.admit(&cost("ws_a", "napping", 0.0, 0.5)));
    // awakeMs and at are no part of the figures.
    let later = event(json!({"type": "workspace.cost", "workspaceId": "ws_a", "phase": "napping", "rateUsdPerHour": 0, "awakeMs": 99, "accruedUsd": 0.5, "at": "2026-01-01T00:00:00.000Z"}));
    assert!(!moved.admit(&later));
}

#[test]
fn cost_moved_lets_every_other_event_through() {
    let mut moved = cost_moved();
    let status = event(json!({"type": "workspace.woken", "workspaceId": "ws_a", "machineId": "m1"}));
    assert!(moved.admit(&status));
    assert!(moved.admit(&status));
}

#[test]
fn cost_moved_formats_figures_as_javascript_does() {
    let mut moved = cost_moved();
    assert!(moved.admit(&cost("ws_a", "running", 0.5, 0.1 + 0.2)));
    assert!(!moved.admit(&cost("ws_a", "running", 0.5, 0.300_000_000_000_000_04)), "the same double is the same figure");
    assert!(moved.admit(&cost("ws_a", "running", 0.5, 0.3)), "0.3 prints differently from 0.1 + 0.2");
    assert!(moved.admit(&cost("ws_a", "running", 1.0, 0.3)));
    assert!(!moved.admit(&cost("ws_a", "running", 1.000_000_000_000_000_1, 0.3)), "1.0000000000000001 is the double 1");
}
