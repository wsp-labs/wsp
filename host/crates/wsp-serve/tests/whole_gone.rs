// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/gone.test.ts at 65f6b89c7, case for case. A machine the provider stopped knowing settles its
//! record to gone through one road, whichever call saw the 404: the poll, a nap, a wake's read or the sweep. The awake
//! stretch closes there, the idle clock is dropped, the row's words name the call and the time, and the host log carries
//! one line. A poll that began before the record settled lands nothing. A nap the provider refuses in words (Not
//! pausable) is a refusal that stands for the machine, not a vanish.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::gone::*;
use common::stub_backend::missing;
use common::*;
use serde_json::{json, Value};
use wsp_core::console::capture;
use wsp_core::status::{Reconcile, StatusListOptions, StatusWatchOptions};
use wsp_core::types::wiring::{OriginStatusApi, RuntimeOptions};
use wsp_engine::errors::EngineError;
use wsp_engine::gone::GONE_READS;
use wsp_engine::machine::MachineBackend;
use wsp_store::store::Store;
use wsp_words::words::thread::GoneSeenBy;

// "a 404 settles the record gone through one road" > "from the status poll: the stretch closes with a cost tick at that instant, the idle clock is dropped, the words name the poll and the time, one log line, and a poll that began before lands nothing"
#[test]
fn from_the_status_poll_the_stretch_closes_with_a_cost_tick_at_that_instant_the_idle_clock_is_dropped_the_words_name_the_poll_and_the_time_one_log_line_and_a_poll_that_began_before_lands_nothing(
) {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        let reads = Reads::of(&m);
        let pauses = m.calls.pause.get();
        let stop = t.fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until_n(|| reads.n() >= 1).await;
        t.fc.advance(COST);
        let polls = (COST / POLL) as usize;
        until_n(|| edge.held() >= polls).await;
        // All but one of the polls the advance fired land now; the last stays in flight across the kill.
        for res in edge.take_n(polls - 1) {
            edge.answer(res).await;
        }
        until_n(|| t.costs().iter().any(|c| c["awakeMs"] == COST) && reads.n() as usize >= polls).await;
        assert_matches(&t.last_cost(), json!({ "phase": "running", "awakeMs": COST }));
        assert!((t.last_cost()["rateUsdPerHour"].as_f64().expect("a rate") - 0.11).abs() < 0.005);

        let gets = t.backend.calls.get_ids.borrow().len();
        m.killed.set(true);
        t.fc.advance(POLL);
        let seen_at = t.fc.now();
        until_n(|| edge.held() >= 2).await;
        // `edge.answer(edge.held.pop()!)`: the newest probe is answered and the older one stays held, in flight across the kill.
        let in_flight = edge.take().expect("the older probe");
        edge.answer(edge.take().expect("the newest probe")).await;
        until(|| async { t.phase(&id).await == "gone" && t.last_cost()["phase"] == "gone" }).await;

        let record = t.record(&id).await;
        assert_eq!(record["gone"], words(GoneSeenBy::StatusPoll, seen_at, None));
        assert_eq!(record["gone"], "machine m1 is gone at the provider: the status poll found it gone at 2026-09-07T01:21:15Z");
        let pushed = t.last_status();
        assert_matches(
            &pushed,
            json!({ "id": id, "phase": "gone", "machineState": "gone", "reach": { "state": "gone" }, "reason": record["gone"] }),
        );
        assert!(pushed.get("idleAt").is_none());
        assert!(status_rows(&t.fx.rt).await[0].get("idleAt").is_none());
        // The tick at the gone instant: the stretch ends at the last proof the machine was awake, and no rate follows.
        assert_matches(&t.last_cost(), json!({ "phase": "gone", "rateUsdPerHour": 0, "awakeMs": COST, "at": iso(seen_at) }));
        let accrued = t.last_cost()["accruedUsd"].clone();
        t.fc.advance(COST);
        let now = iso(t.fc.now());
        until_n(|| t.last_cost()["at"] == now.as_str()).await;
        assert_matches(&t.last_cost(), json!({ "phase": "gone", "rateUsdPerHour": 0, "awakeMs": COST, "accruedUsd": accrued }));
        // The poll left in flight lands now, a minute on: its reach failed, it asks the provider and hears gone, and
        // the row it built on the running record it read is dropped, so nothing after the gone row says running.
        let rows = t.statuses().len();
        // The kill pass read the state once, the poll's read, and confirmed its verdict with GONE_READS reads of the machine.
        assert_eq!(reads.n() as usize, polls + 1);
        assert_eq!(t.backend.calls.get_ids.borrow()[gets..].iter().filter(|g| g.as_str() == m.id()).count(), GONE_READS as usize);
        edge.drop_request(in_flight);
        until_n(|| reads.n() as usize >= polls + 2).await;
        tokio::task::yield_now().await;
        assert!(t.statuses()[rows..].is_empty());
        assert_eq!(edge.held(), 0);
        assert_eq!(gone_lines(&warned), [format!("workspace {id} is gone: {}", record["gone"].as_str().expect("words"))]);
        t.fc.advance(WINDOW * 2);
        assert_eq!(m.calls.pause.get(), pauses);
        stop();
    });
}

// "from an idle nap: gone, not a refusal; no window re-armed, the words name the pause and quote the 404, the stretch closes, one log line"
#[test]
fn from_an_idle_nap_gone_not_a_refusal_no_window_re_armed_the_words_name_the_pause_and_quote_the_404_the_stretch_closes_one_log_line() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let calls = Rc::new(Cell::new(0));
        let counted = calls.clone();
        let vanished = m.clone();
        m.set_pause(Rc::new(move || {
            counted.set(counted.get() + 1);
            vanished.killed.set(true);
            Box::pin(async { Err(missing("Sandbox not found")) })
        }));
        t.fc.advance(WINDOW);
        let seen_at = t.fc.now();
        until(|| async { t.phase(&id).await == "gone" && t.last_cost()["phase"] == "gone" }).await;

        let record = t.record(&id).await;
        assert_eq!(record["gone"], words(GoneSeenBy::Pause, seen_at, Some("404 Sandbox not found")));
        assert_eq!(
            record["gone"],
            "machine m1 is gone at the provider: the pause found it gone at 2026-09-07T01:25:00Z (404 Sandbox not found)"
        );
        let phases: Vec<Value> = t.statuses().iter().map(|s| s["phase"].clone()).collect();
        assert_eq!(phases, [json!("pausing"), json!("gone")]);
        assert_matches(&t.last_status(), json!({ "machineState": "gone", "reach": { "state": "gone" }, "reason": record["gone"] }));
        assert!(t.last_status().get("idleAt").is_none());
        assert!(status_rows(&t.fx.rt).await[0].get("idleAt").is_none());
        assert_matches(&t.last_cost(), json!({ "phase": "gone", "rateUsdPerHour": 0, "at": iso(seen_at) }));
        assert_eq!(warned.lines(), [format!("workspace {id} is gone: {}", record["gone"].as_str().expect("words"))]);
        t.fc.advance(WINDOW * 2);
        tokio::task::yield_now().await;
        assert_eq!(calls.get(), 1);
    });
}

// "from a nap asked for: the caller hears the provider's error and the record is gone"
#[test]
fn from_a_nap_asked_for_the_caller_hears_the_providers_error_and_the_record_is_gone() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let m = t.backend.machine(0);
        let vanished = m.clone();
        m.set_pause(Rc::new(move || {
            vanished.killed.set(true);
            Box::pin(async { Err(missing("Sandbox not found")) })
        }));
        let refused = t.fx.rt.workspaces().nap(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("Sandbox not found"), "{}", refused.message);
        let record = t.record(&ws.view.id).await;
        assert_eq!(record["phase"], "gone");
        let gone = record["gone"].as_str().expect("words");
        assert!(
            gone.starts_with("machine m1 is gone at the provider: the pause found it gone at ")
                && gone.ends_with(" (404 Sandbox not found)"),
            "{gone}"
        );
        let wake = t.fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        assert!(
            wake.message.contains(&wsp_words::workspace_state::gone_refusal(Some(&ws.view.name), "wake", Some(gone))),
            "{}",
            wake.message
        );
    });
}

// "from a wake's read of a record that says running: gone, and the wake is refused with the gone sentence"
#[test]
fn from_a_wakes_read_of_a_record_that_says_running_gone_and_the_wake_is_refused_with_the_gone_sentence() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let m = t.backend.machine(0);
        m.killed.set(true);
        let refused = t.fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("a's machine is gone with its disk, so work that was not pushed is lost; rebuild it to wake, which brings back its home folder from the last saved nap"), "{}", refused.message);
        let record = t.record(&ws.view.id).await;
        assert_eq!(record["phase"], "gone");
        let gone = record["gone"].as_str().expect("words");
        assert!(
            gone.starts_with("machine m1 is gone at the provider: the wake found it gone at ")
                && gone.ends_with('Z')
                && !gone.contains(" (404"),
            "{gone}"
        );
        assert_matches(&t.last_status(), json!({ "phase": "gone", "reason": record["gone"] }));
        assert_eq!(m.resumes.get(), 0);
        assert_eq!(gone_lines(&warned).len(), 1);
    });
}

// "from the sweep: a recorded machine the listing lacks is read once; gone settles the record, a machine still there is left alone"
#[test]
fn from_the_sweep_a_recorded_machine_the_listing_lacks_is_read_once_gone_settles_the_record_a_machine_still_there_is_left_alone() {
    drive(|| async {
        let stub = StubBackend::new(None);
        let lag = Lagging::over(&stub);
        let t = test_runtime(|_| {}, Seed { backend: Some(stub), provider: Some(lag.clone()), ..Default::default() });
        let warned = capture();
        let a = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let b = create_on(&t.fx.rt, CreateOn::golden("b"), None).await.expect("a workspace").view.id;
        let c = create_on(&t.fx.rt, CreateOn::golden("c"), None).await.expect("a workspace").view.id;
        let (ma, mb, mc) = (t.backend.machine(0), t.backend.machine(1), t.backend.machine(2));
        ma.killed.set(true);
        // The listing lags: b is missing from it but the provider still has it; c answers 404 in words to the read.
        let (b_id, c_id) = (mb.id().to_string(), mc.id().to_string());
        let readers = Reads::of(&mb);
        mc.set_state(Rc::new(|| Box::pin(async { Err(missing("Sandbox not found")) })));
        let get = t.backend.get_fn();
        let c_id_get = c_id.clone();
        t.backend.set_get(Rc::new(move |id| {
            if id == c_id_get {
                return Box::pin(async { Err(missing("Sandbox not found")) });
            }
            get(id)
        }));
        lag.hide(&[b_id.clone(), c_id.clone()]);
        t.fx.rt.reap(None, None).await.expect("a sweep");

        let a_gone = t.record(&a).await["gone"].as_str().expect("words").to_string();
        assert!(
            a_gone.starts_with("machine m1 is gone at the provider: the sweep found it gone at ")
                && a_gone.ends_with('Z')
                && !a_gone.contains(" (404"),
            "{a_gone}"
        );
        assert_eq!(t.phase(&b).await, "running");
        assert_eq!(readers.n(), 1);
        let c_gone = t.record(&c).await["gone"].as_str().expect("words").to_string();
        assert!(
            c_gone.starts_with("machine m3 is gone at the provider: the sweep found it gone at ")
                && c_gone.ends_with("Z (404 Sandbox not found)"),
            "{c_gone}"
        );
        let mut settled: Vec<String> =
            t.statuses().iter().filter(|s| s["phase"] == "gone").map(|s| s["id"].as_str().expect("an id").to_string()).collect();
        settled.sort();
        let mut want = vec![a.clone(), c.clone()];
        want.sort();
        assert_eq!(settled, want);
        assert_eq!(gone_lines(&warned).len(), 2);
        // A second sweep asks nothing about a record already gone.
        let again = Reads::of(&ma);
        t.fx.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(again.n(), 0);
    });
}

// "the host's metrics as the early warning" > "a metrics 404 with the daemon's probe missed on the same pass leaves the record running: the state read said running, so the row does, and the gap is one logged line"
#[test]
fn a_metrics_404_with_the_daemons_probe_missed_on_the_same_pass_leaves_the_record_running_the_state_read_said_running_so_the_row_does_and_the_gap_is_one_logged_line(
) {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        let asks = Asks::of(&m);
        let stop = t.fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until_n(|| asks.state() >= 1 && asks.metrics() >= 1).await;
        assert_eq!(t.phase(&id).await, "running");

        m.host_lost.set(true);
        t.fc.advance(POLL);
        until_n(|| edge.held() >= 1).await;
        edge.drop_request(edge.take().expect("a probe"));
        until_n(|| asks.state() >= 2 && doubt_lines(&warned).len() == 1).await;
        // A second pass with the guest missing again: still one witness, and one witness is not a verdict.
        t.fc.advance(POLL);
        until_n(|| edge.held() >= 1).await;
        edge.drop_request(edge.take().expect("a probe"));
        until_n(|| asks.state() >= 3).await;

        let record = t.record(&id).await;
        assert!(!m.killed.get());
        assert_eq!(record["phase"], "running");
        assert!(record.get("gone").is_none());
        assert!(t.statuses().iter().all(|s| s["machineState"] != "gone"));
        assert_matches(&t.last_status(), json!({ "id": id, "phase": "running", "machineState": "running" }));
        assert_eq!(t.last_status()["idleAt"], T0 + WINDOW);
        assert!(t.gone_costs().is_empty());
        assert!(gone_lines(&warned).is_empty());
        assert_eq!(doubt_lines(&warned), [doubt_line(&id)]);
        assert_eq!(asks.both(), (3, 3));
        stop();
    });
}

// "a metrics 404 over a machine the state read answers 404 for settles the record gone as before, and the words quote that read"
#[test]
fn a_metrics_404_over_a_machine_the_state_read_answers_404_for_settles_the_record_gone_as_before_and_the_words_quote_that_read() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        let asks = Asks::of(&m);
        let stop = t.fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until_n(|| asks.state() >= 1 && asks.metrics() >= 1).await;

        m.host_lost.set(true);
        m.killed.set(true);
        t.fc.advance(POLL);
        let seen_at = t.fc.now();
        until_n(|| edge.held() >= 1).await;
        edge.drop_request(edge.take().expect("a probe"));
        until(|| async { t.phase(&id).await == "gone" && t.last_cost()["phase"] == "gone" }).await;

        let record = t.record(&id).await;
        assert_eq!(record["gone"], words(GoneSeenBy::StatusPoll, seen_at, None));
        assert_eq!(record["gone"], "machine m1 is gone at the provider: the status poll found it gone at 2026-09-07T01:20:15Z");
        assert_matches(
            &t.last_status(),
            json!({ "id": id, "phase": "gone", "machineState": "gone", "reach": { "state": "gone" }, "reason": record["gone"] }),
        );
        assert!(t.last_status().get("idleAt").is_none());
        assert_matches(&t.last_cost(), json!({ "phase": "gone", "rateUsdPerHour": 0, "at": iso(seen_at) }));
        assert_eq!(gone_lines(&warned), [format!("workspace {id} is gone: {}", record["gone"].as_str().expect("words"))]);
        // The metrics read never spoke: the state read answered 404 before it was reached.
        assert!(doubt_lines(&warned).is_empty());
        assert_eq!(asks.metrics(), 1);
        stop();
    });
}

// "a metrics 404 while the daemon answers on the same pass is one logged line and nothing else, on an explicit refresh too"
#[test]
fn a_metrics_404_while_the_daemon_answers_on_the_same_pass_is_one_logged_line_and_nothing_else_on_an_explicit_refresh_too() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        edge.from_daemon.set(true);
        let asks = Asks::of(&m);
        m.host_lost.set(true);

        let first = pass(&t, &edge, Reconcile::Always).await;
        assert_matches(&first, json!({ "phase": "running", "machineState": "running", "reach": { "state": "reachable" } }));
        assert!(first.get("reason").is_none());
        assert_eq!(asks.both(), (1, 1));
        assert_eq!(doubt_lines(&warned), [doubt_line(&id)]);

        let second = pass(&t, &edge, Reconcile::Always).await;
        assert_matches(&second, json!({ "machineState": "running", "reach": { "state": "reachable" } }));
        assert_eq!(asks.both(), (2, 2));
        assert_eq!(doubt_lines(&warned).len(), 1);
        assert!(gone_lines(&warned).is_empty());
        assert_eq!(t.phase(&id).await, "running");
        assert!(t.statuses().iter().all(|s| s["machineState"] != "gone"));
    });
}

// "the edge speaking for the machine is not the daemon missing: a metrics 404 there leaves the record running while the guest still answers exec"
#[test]
fn the_edge_speaking_for_the_machine_is_not_the_daemon_missing_a_metrics_404_there_leaves_the_record_running_while_the_guest_still_answers_exec(
) {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        let asks = Asks::of(&m);
        m.host_lost.set(true);
        let stop = t.fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until_n(|| doubt_lines(&warned).len() == 1).await;
        t.fc.advance(POLL);
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until_n(|| asks.state() >= 2).await;
        stop();
        // A full pass after the poll's: anything the poll settled has landed by now.
        let row = pass(&t, &edge, Reconcile::OnFailure).await;
        assert_matches(&row, json!({ "phase": "running", "machineState": "running" }));
        assert_eq!(asks.both(), (3, 3));
        assert_eq!(doubt_lines(&warned).len(), 1);
        assert!(gone_lines(&warned).is_empty());
        assert_eq!(t.phase(&id).await, "running");
        assert!(t.statuses().iter().all(|s| s["machineState"] != "gone"));
        let said = m.exec("echo ok", Default::default()).await.expect("an exec");
        assert_eq!(said.exit_code.as_f64(), 0.0);

        // The guest missing on a later pass adds no witness the state read has not already outvoted.
        let rt = t.fx.rt.clone();
        let later = tokio::task::spawn_local(async move {
            OriginStatusApi::list(
                &*rt.status(),
                Some(StatusListOptions { reconcile: Some(Reconcile::OnFailure), ..Default::default() }),
                None,
            )
            .await
        });
        until_n(|| edge.held() >= 1).await;
        edge.drop_request(edge.take().expect("a probe"));
        let rows = later.await.expect("the task").expect("a status list");
        assert_matches(&j(&rows[0]), json!({ "machineState": "running" }));
        assert!(gone_lines(&warned).is_empty());
    });
}

// "a metrics read that fails any other way changes nothing, and metrics ride only the passes that ask the provider"
#[test]
fn a_metrics_read_that_fails_any_other_way_changes_nothing_and_metrics_ride_only_the_passes_that_ask_the_provider() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        let metrics = m.metrics_fn();
        let asks = Asks::of(&m);
        let failing = Rc::new(Cell::new(true));
        let still = failing.clone();
        m.set_metrics(Rc::new(move || {
            if still.get() {
                return Box::pin(async {
                    Err(EngineError::Refused {
                        message: "Bad Gateway".into(),
                        kind: Some(wsp_engine::errors::ErrorKind::Transient),
                        status: Some(502),
                        code: None,
                    })
                });
            }
            metrics()
        }));
        let stop = t.fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until_n(|| asks.metrics() >= 1 && !t.statuses().is_empty()).await;
        assert_eq!(asks.both(), (1, 1));
        assert_eq!(t.phase(&id).await, "running");
        assert_matches(&t.last_status(), json!({ "phase": "running", "machineState": "running" }));
        assert!(warned.lines().is_empty(), "{:?}", warned.lines());
        stop();

        // The guest answers the next passes itself: the provider is not asked, so neither are its metrics.
        failing.set(false);
        edge.from_daemon.set(true);
        assert_matches(
            &pass(&t, &edge, Reconcile::OnFailure).await,
            json!({ "machineState": "running", "reach": { "state": "reachable" } }),
        );
        assert_matches(
            &pass(&t, &edge, Reconcile::OnFailure).await,
            json!({ "machineState": "running", "reach": { "state": "reachable" } }),
        );
        assert_eq!(asks.both(), (1, 1));
        assert_eq!(t.phase(&id).await, "running");
    });
}

/// A machine whose next state read answers 404 and whose later ones answer as they always did.
fn flake_once(m: &Arc<StubMachine>) {
    let state = m.state_fn();
    let flaky = Rc::new(Cell::new(true));
    m.set_state(Rc::new(move || {
        if !flaky.get() {
            return state();
        }
        flaky.set(false);
        Box::pin(async { Err(missing("Sandbox not found")) })
    }));
}

async fn list_answered(t: &T, edge: &Edge) -> Value {
    let rt = t.fx.rt.clone();
    let listed = tokio::task::spawn_local(async move { OriginStatusApi::list(&*rt.status(), None, None).await });
    until_n(|| edge.held() >= 1).await;
    edge.answer(edge.take().expect("a probe")).await;
    j(&listed.await.expect("the task").expect("a status list")[0])
}

const CONFIRM: i64 = 4_000;

fn with_confirm(o: &mut RuntimeOptions) {
    o.gone_confirm_ms = Some(CONFIRM as u64);
}

fn not_gone_line(id: &str, saw: &str, then: &str) -> String {
    format!("workspace {id} is not gone: {saw}, and the state read that followed said {then}")
}

// "a gone verdict is checked against the state read before it ends a turn" > "one 404 over a machine that is there: the record stays running, the row never moves, and the log says it is not gone"
#[test]
fn one_404_over_a_machine_that_is_there_the_record_stays_running_the_row_never_moves_and_the_log_says_it_is_not_gone() {
    drive(|| async {
        let t = test_runtime(with_confirm, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        let stop = t.fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until_n(|| !t.statuses().is_empty()).await;

        flake_once(&m);
        let reads = Reads::of(&m);
        t.fc.advance(POLL);
        let seen_at = t.fc.now();
        until_n(|| edge.held() >= 1).await;
        let timers = t.fc.pending();
        edge.drop_request(edge.take().expect("a probe"));
        // The verdict is waiting on its confirming read: nothing has ended, and the record has not moved.
        until_n(|| t.fc.pending() > timers).await;
        assert_eq!(t.phase(&id).await, "running");
        assert_eq!(reads.n(), 1);

        t.fc.advance(CONFIRM);
        until_n(|| !warned.lines().is_empty()).await;
        immediate().await;
        let saw = words(GoneSeenBy::StatusPoll, seen_at, Some("404 Sandbox not found"));
        assert_eq!(warned.lines(), [not_gone_line(&id, &saw, "running")]);
        assert!(!t.events.has("workspace.gone"));
        let record = t.record(&id).await;
        assert_eq!(record["phase"], "running");
        assert!(record.get("gone").is_none());
        assert_matches(&t.last_status(), json!({ "id": id, "phase": "running", "machineState": "running" }));
        assert!(t.last_status().get("reason").is_none());
        // The window was never dropped, so it still runs from the create that armed it.
        assert_eq!(t.last_status()["idleAt"], T0 + WINDOW);
        stop();
    });
}

// "the pushed row: one 404 on the poll leaves it as it was, and it turns gone only when the provider answers gone twelve reads in a row"
#[test]
fn the_pushed_row_one_404_on_the_poll_leaves_it_as_it_was_and_it_turns_gone_only_when_the_provider_answers_gone_twelve_reads_in_a_row() {
    drive(|| async {
        let t = test_runtime(with_confirm, Seed::default());
        let _warned = capture();
        let warned = _warned;
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        let stop = t.fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until_n(|| !t.statuses().is_empty()).await;
        let shown = t.last_status();

        flake_once(&m);
        let reads = Reads::of(&m);
        let mut rows = t.statuses().len();
        t.fc.advance(POLL);
        until_n(|| edge.held() >= 1).await;
        edge.drop_request(edge.take().expect("a probe"));
        until_n(|| reads.n() >= 1).await;
        immediate().await;
        assert!(t.statuses()[rows..].is_empty());
        assert_matches(
            &list_answered(&t, &edge).await,
            json!({ "machineState": "running", "reach": { "state": shown["reach"]["state"] } }),
        );
        t.fc.advance(CONFIRM);
        until_n(|| !warned.lines().is_empty()).await;
        immediate().await;
        assert!(t.statuses()[rows..].is_empty());

        m.killed.set(true);
        let gets = t.backend.calls.get_ids.borrow().len();
        rows = t.statuses().len();
        t.fc.advance(POLL - CONFIRM);
        until_n(|| edge.held() >= 1).await;
        let before = reads.n();
        edge.drop_request(edge.take().expect("a probe"));
        until_n(|| reads.n() > before).await;
        immediate().await;
        // A read of the rows while the verdict waits reads what the record says and starts no second confirmation.
        assert_matches(&list_answered(&t, &edge).await, json!({ "phase": "running", "machineState": "running" }));
        assert!(t.statuses()[rows..].iter().all(|s| s["machineState"] != "gone"));
        t.fc.advance(CONFIRM);
        until_n(|| t.statuses().iter().any(|s| s["machineState"] == "gone")).await;
        assert_eq!(t.backend.calls.get_ids.borrow()[gets..].iter().filter(|g| g.as_str() == m.id()).count(), GONE_READS as usize);
        assert_eq!(t.statuses()[rows..].iter().map(|s| s["machineState"].clone()).collect::<Vec<_>>(), [json!("gone")]);
        assert_matches(&t.last_status(), json!({ "id": id, "phase": "gone", "reach": { "state": "gone" } }));
        stop();
    });
}

// "the state read agrees after the wait: the record settles gone and its turn ends, and nothing settled before the wait was up"
#[test]
fn the_state_read_agrees_after_the_wait_the_record_settles_gone_and_its_turn_ends_and_nothing_settled_before_the_wait_was_up() {
    drive(|| async {
        let t = test_runtime(with_confirm, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        let stop = t.fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until(|| async { t.phase(&id).await == "running" }).await;

        m.killed.set(true);
        t.fc.advance(POLL);
        let seen_at = t.fc.now();
        until_n(|| edge.held() >= 1).await;
        let timers = t.fc.pending();
        edge.drop_request(edge.take().expect("a probe"));
        until_n(|| t.fc.pending() > timers).await;
        assert_eq!(t.phase(&id).await, "running");

        t.fc.advance(CONFIRM);
        until(|| async { t.phase(&id).await == "gone" }).await;
        assert_eq!(t.record(&id).await["gone"], words(GoneSeenBy::StatusPoll, seen_at, None));
        assert_eq!(t.events.of("workspace.gone").len(), 1);
        assert_eq!(gone_lines(&warned).len(), 1);
        stop();
    });
}

// "the state read a short wait later says paused: a machine the provider holds is not a machine it lost"
#[test]
fn the_state_read_a_short_wait_later_says_paused_a_machine_the_provider_holds_is_not_a_machine_it_lost() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let edge = edge_holding(&m).await;
        let stop = t.fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.held() >= 1).await;
        edge.answer(edge.take().expect("a probe")).await;
        until(|| async { t.phase(&id).await == "running" }).await;

        // The 404 the poll met is followed by a read that finds the machine paused at the provider.
        let state = m.state_fn();
        let flaky = Rc::new(Cell::new(true));
        let held = m.clone();
        m.set_state(Rc::new(move || {
            if !flaky.get() {
                return state();
            }
            flaky.set(false);
            held.paused.set(true);
            Box::pin(async { Err(missing("Sandbox not found")) })
        }));
        t.fc.advance(POLL);
        let seen_at = t.fc.now();
        until_n(|| edge.held() >= 1).await;
        edge.drop_request(edge.take().expect("a probe"));
        until_n(|| !warned.lines().is_empty()).await;

        assert!(t.record(&id).await.get("gone").is_none());
        assert!(!t.events.has("workspace.gone"));
        assert_eq!(warned.lines(), [not_gone_line(&id, &words(GoneSeenBy::StatusPoll, seen_at, Some("404 Sandbox not found")), "paused")]);
        stop();
    });
}

// "a pause the provider answers 404 for over a machine that is there is a refused pause, not a vanish"
#[test]
fn a_pause_the_provider_answers_404_for_over_a_machine_that_is_there_is_a_refused_pause_not_a_vanish() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        m.set_pause(Rc::new(|| Box::pin(async { Err(missing("Sandbox not found")) })));
        let refused = t.fx.rt.workspaces().nap(&id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("Sandbox not found"), "{}", refused.message);
        let record = t.record(&id).await;
        assert_eq!(record["phase"], "running");
        assert!(record.get("gone").is_none());
        assert_eq!(t.statuses().iter().map(|s| s["phase"].clone()).collect::<Vec<_>>(), [json!("pausing"), json!("running")]);
        assert_eq!(warned.lines(), [not_gone_line(&id, &words(GoneSeenBy::Pause, T0, Some("404 Sandbox not found")), "running")]);
        // The row carries the provider's answer, as any refused pause does.
        assert_matches(&t.last_status(), json!({ "phase": "running", "machineState": "running", "reason": "Sandbox not found" }));
    });
}

fn said_by_load(words: &str) -> bool {
    // `^machine m1 is gone at the provider: the record load found it gone at \S+Z \(404 [^)]*\)$`
    words
        .strip_prefix("machine m1 is gone at the provider: the record load found it gone at ")
        .and_then(|rest| rest.split_once(" (404 "))
        .is_some_and(|(at, answer)| {
            !at.is_empty()
                && !at.contains(char::is_whitespace)
                && at.ends_with('Z')
                && answer.ends_with(')')
                && !answer[..answer.len() - 1].contains(')')
        })
}

/// A record made on the stub, then served by the Solari backend over the split gateway, whose holder runs its machine;
/// the next `lies` reads of it land on the copy that never held it.
struct Split {
    ws_id: String,
    machine_id: String,
    store: Rc<dyn Store>,
    gw: Rc<common::gateway::SplitGateway>,
    lies: Rc<Cell<u32>>,
    solari: Arc<dyn MachineBackend>,
    _first: T,
}

async fn on_split_gateway() -> Split {
    use common::gateway::{Side, SplitGateway};
    let t = test_runtime(|_| {}, Seed::default());
    let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
    t.fx.rt.close().await.expect("a close");
    let lies = Rc::new(Cell::new(0u32));
    let left = lies.clone();
    let gw = SplitGateway::new(move |_, method| {
        if method == "GET" && left.get() > 0 {
            left.set(left.get() - 1);
            Side::Empty
        } else {
            Side::Holder
        }
    });
    gw.holder.borrow_mut().insert(ws.view.machine_id.clone(), "running".into());
    let solari: Arc<dyn MachineBackend> =
        wsp_engine::solari_backend::SolariBackend::new(wsp_engine::solari_backend::SolariBackendOptions {
            api_key: "k".into(),
            fetch: Some(gw.fetch()),
            ..Default::default()
        });
    Split { ws_id: ws.view.id.clone(), machine_id: ws.view.machine_id.clone(), store: t.store.clone(), gw, lies, solari, _first: t }
}

// "a 404 settles nothing until the provider answers gone twelve reads in a row" > "the record load: two 404s over a machine the holder runs leave the record running, never gone at rate 0"
#[test]
fn the_record_load_two_404s_over_a_machine_the_holder_runs_leave_the_record_running_never_gone_at_rate_0() {
    drive(|| async {
        let warned = capture();
        let s = on_split_gateway().await;
        s.lies.set(2);
        let t = test_runtime(|_| {}, Seed { store: Some(s.store.clone()), provider: Some(s.solari.clone()), ..Default::default() });
        let record = t.record(&s.ws_id).await;
        assert_eq!(record["phase"], "running");
        assert!(record.get("gone").is_none());
        assert_matches(&s.store.get("workspaces", &s.ws_id).await.expect("a read").expect("a record"), json!({ "phase": "running" }));
        assert!(!t.events.has("workspace.gone"));
        assert!(t.gone_costs().is_empty());
        assert!(gone_lines(&warned).is_empty());
        t.fx.rt.close().await.expect("a close");
    });
}

// "the record load: a machine the provider answers 404 for twelve reads in a row settles gone once the host serves, through the one road to gone"
#[test]
fn the_record_load_a_machine_the_provider_answers_404_for_twelve_reads_in_a_row_settles_gone_once_the_host_serves_through_the_one_road_to_gone(
) {
    drive(|| async {
        let warned = capture();
        let s = on_split_gateway().await;
        s.gw.holder.borrow_mut().shift_remove(&s.machine_id);
        let t = test_runtime(|_| {}, Seed { store: Some(s.store.clone()), provider: Some(s.solari.clone()), ..Default::default() });
        until(|| async { t.phase(&s.ws_id).await == "gone" }).await;
        assert!(s.gw.gets() > GONE_READS as usize, "{}", s.gw.gets());
        let words = t.record(&s.ws_id).await["gone"].as_str().expect("words").to_string();
        assert!(said_by_load(&words), "{words}");
        assert_matches(
            &s.store.get("workspaces", &s.ws_id).await.expect("a read").expect("a record"),
            json!({ "phase": "gone", "gone": words }),
        );
        // The event, the row and the meter move with the record, as they do on every other road to gone.
        let gone = t.events.of("workspace.gone");
        assert_eq!(gone.len(), 1);
        assert_matches(&gone[0], json!({ "workspaceId": s.ws_id, "machineId": "m1", "reason": words }));
        assert_matches(&t.last_status(), json!({ "id": s.ws_id, "phase": "gone", "reason": words }));
        until_n(|| t.costs().iter().any(|c| c["workspaceId"] == s.ws_id.as_str() && c["phase"] == "gone")).await;
        assert!(t.gone_costs().iter().all(|c| c["rateUsdPerHour"] == 0));
        assert_eq!(gone_lines(&warned), [format!("workspace {} is gone: {words}", s.ws_id)]);
        t.fx.rt.close().await.expect("a close");
    });
}

// "a host start over records whose machines answer 404 serves at once, and confirms them after"
#[test]
fn a_host_start_over_records_whose_machines_answer_404_serves_at_once_and_confirms_them_after() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let mut made = vec![];
        for name in ["a", "b", "c"] {
            made.push(create_on(&t.fx.rt, CreateOn::golden(name), None).await.expect("a workspace").view.id);
        }
        t.fx.rt.close().await.expect("a close");
        // Every machine is gone, and every read after the first one of each waits until the test lets it answer.
        for m in t.backend.machines() {
            m.killed.set(true);
        }
        let held = Gate::new();
        let first: Rc<RefCell<Vec<String>>> = Rc::default();
        let get = t.backend.get_fn();
        let (waits, seen) = (held.clone(), first.clone());
        t.backend.set_get(Rc::new(move |id| {
            let (waits, seen, get) = (waits.clone(), seen.clone(), get.clone());
            Box::pin(async move {
                if seen.borrow().contains(&id) {
                    waits.wait().await;
                }
                seen.borrow_mut().push(id.clone());
                get(id).await
            })
        }));
        let second = test_runtime(|_| {}, Seed { store: Some(t.store.clone()), backend: Some(t.backend.clone()), ..Default::default() });
        let listed = second.fx.rt.workspaces().list(None).await.expect("a list");
        assert_eq!(
            listed.iter().map(|w| j(w)["phase"].clone()).collect::<Vec<_>>(),
            [json!("running"), json!("running"), json!("running")]
        );
        assert!(!second.events.has("workspace.gone"));
        held.open();
        until(|| async { second.fx.rt.workspaces().list(None).await.expect("a list").iter().all(|w| j(w)["phase"] == "gone") }).await;
        let mut gone: Vec<String> =
            second.events.of("workspace.gone").iter().map(|e| e["workspaceId"].as_str().expect("an id").to_string()).collect();
        gone.sort();
        made.sort();
        assert_eq!(gone, made);
        second.fx.rt.close().await.expect("a close");
    });
}

// "a wake's read: two 404s over a machine the holder runs leave the record running and the wake answers running"
#[test]
fn a_wakes_read_two_404s_over_a_machine_the_holder_runs_leave_the_record_running_and_the_wake_answers_running() {
    drive(|| async {
        let _warned = capture();
        let s = on_split_gateway().await;
        let t = test_runtime(|_| {}, Seed { store: Some(s.store.clone()), provider: Some(s.solari.clone()), ..Default::default() });
        assert_eq!(t.phase(&s.ws_id).await, "running");
        s.lies.set(2);
        let woken = t.fx.rt.workspaces().wake(&s.ws_id, None).await.expect("a wake");
        assert_matches(&j(&woken), json!({ "phase": "running", "machineId": s.machine_id }));
        let record = t.record(&s.ws_id).await;
        assert_eq!(record["phase"], "running");
        assert!(record.get("gone").is_none());
        assert!(!t.events.has("workspace.gone"));
        assert!(t.gone_costs().is_empty());
        assert!(t.statuses().iter().all(|s| s["phase"] != "gone"));
        t.fx.rt.close().await.expect("a close");
    });
}

// "the record load: %s leave the host serving and the record as it was, and the next sweep reads it again" (the rows: "a 404 and then reads that fail" with 1 lie, "a first read that fails" with 0)
#[test]
fn the_record_load_a_404_and_then_reads_that_fail_leave_the_host_serving_and_the_record_as_it_was_and_the_next_sweep_reads_it_again() {
    load_that_fails(1);
}

#[test]
fn the_record_load_a_first_read_that_fails_leave_the_host_serving_and_the_record_as_it_was_and_the_next_sweep_reads_it_again() {
    load_that_fails(0);
}

fn load_that_fails(lies: u32) {
    drive(move || async move {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let other = create_on(&t.fx.rt, CreateOn::golden("b"), None).await.expect("a workspace").view.id;
        t.fx.rt.close().await.expect("a close");
        let m = t.backend.machine(0);
        let get = t.backend.get_fn();
        let left = Rc::new(Cell::new(lies));
        let down = Rc::new(Cell::new(true));
        let (left_now, down_now, target) = (left.clone(), down.clone(), m.id().to_string());
        t.backend.set_get(Rc::new(move |id| {
            if id == target && left_now.get() > 0 {
                left_now.set(left_now.get() - 1);
                return Box::pin(async { Err(missing("Sandbox not found")) });
            }
            if id == target && down_now.get() {
                return Box::pin(async {
                    Err(EngineError::Refused { message: "Bad gateway".into(), kind: None, status: Some(502), code: None })
                });
            }
            get(id)
        }));
        let second = test_runtime(|_| {}, Seed { store: Some(t.store.clone()), backend: Some(t.backend.clone()), ..Default::default() });
        assert_matches(&second.record(&ws).await, json!({ "phase": "running", "machineId": m.id() }));
        assert_matches(&second.record(&other).await, json!({ "phase": "running" }));
        assert_matches(&t.store.get("workspaces", &ws).await.expect("a read").expect("a record"), json!({ "phase": "running" }));
        assert!(!second.events.has("workspace.gone"));

        // The sweep's read finds the machine, and the record is served on it: a pause reaches the provider.
        down.set(false);
        second.fx.rt.reap(None, None).await.expect("a sweep");
        assert_matches(
            &j(&second.fx.rt.workspaces().nap(&ws, None).await.expect("a nap")),
            json!({ "phase": "napping", "machineId": m.id() }),
        );
        assert!(m.paused.get());
        second.fx.rt.close().await.expect("a close");
    });
}

// "reads that fail after the 404 leave the machine claimed and running, and the next sweep asks again"
#[test]
fn reads_that_fail_after_the_404_leave_the_machine_claimed_and_running_and_the_next_sweep_asks_again() {
    drive(|| async {
        let stub = StubBackend::new(None);
        let lag = Lagging::over(&stub);
        let t = test_runtime(|_| {}, Seed { backend: Some(stub), provider: Some(lag.clone()), ..Default::default() });
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        flake_once(&m);
        // Every read after the wake's 404 fails as a gateway that answers nothing.
        let get = t.backend.get_fn();
        let down = Rc::new(Cell::new(true));
        let (down_now, target) = (down.clone(), m.id().to_string());
        t.backend.set_get(Rc::new(move |asked| {
            if down_now.get() && asked == target {
                return Box::pin(async {
                    Err(EngineError::Refused { message: "Bad gateway".into(), kind: None, status: Some(502), code: None })
                });
            }
            get(asked)
        }));
        assert_matches(&j(&t.fx.rt.workspaces().wake(&id, None).await.expect("a wake")), json!({ "phase": "running" }));
        assert!(t.record(&id).await.get("gone").is_none());
        assert_eq!(
            warned.lines(),
            [format!(
                "workspace {id} is not gone: {}, and the reads that followed failed: Bad gateway",
                words(GoneSeenBy::Wake, T0, Some("404 Sandbox not found"))
            )]
        );

        // The sweep meets the same: its row says nothing was learned, not that the machine was found.
        lag.hide(&[m.id().to_string()]);
        flake_once(&m);
        t.fx.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(t.phase(&id).await, "running");
        assert_matches(&t.last_status(), json!({ "id": id, "phase": "running", "reason": wsp_words::words::thread::GONE_UNCHECKED }));
        down.set(false);
        let swept = t.fx.rt.reap(None, None).await.expect("a sweep");
        assert!(swept.reaped.reaped.is_empty());
        assert_eq!(t.phase(&id).await, "running");
        m.killed.set(true);
        t.fx.rt.reap(None, None).await.expect("a sweep");
        let gone = t.record(&id).await["gone"].as_str().expect("words").to_string();
        assert!(
            gone.starts_with("machine m1 is gone at the provider: the sweep found it gone at ")
                && gone[gone.rfind(" at ").unwrap_or(0)..].contains('Z'),
            "{gone}"
        );
    });
}

/// A record settled gone by a wake over a machine the provider then has again: what the incident left behind.
async fn wrongly_gone(t: &T) -> (String, String, Arc<StubMachine>) {
    let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
    let id = ws.view.id.clone();
    let m = t.backend.machine(0);
    m.killed.set(true);
    let refused = t.fx.rt.workspaces().wake(&id, None).await.expect_err("a refusal");
    let gone = t.record(&id).await["gone"].as_str().expect("words").to_string();
    assert!(
        refused.message.contains(&wsp_words::workspace_state::gone_refusal(Some(&ws.view.name), "wake", Some(&gone))),
        "{}",
        refused.message
    );
    assert_eq!(t.phase(&id).await, "gone");
    m.killed.set(false);
    (id, ws.view.name.clone(), m)
}

// "a record marked gone recovers on a state read that says running" > "a wake: the record follows the read back to running and nothing is resumed"
#[test]
fn a_wake_the_record_follows_the_read_back_to_running_and_nothing_is_resumed() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let (id, _, m) = wrongly_gone(&t).await;
        assert_matches(&j(&t.fx.rt.workspaces().wake(&id, None).await.expect("a wake")), json!({ "phase": "running", "machineId": "m1" }));
        let record = t.record(&id).await;
        assert_eq!(record["phase"], "running");
        assert!(record.get("gone").is_none());
        assert_eq!(m.resumes.get(), 0);
        assert_eq!(t.backend.made(), 1);
        assert_matches(
            &t.last_status(),
            json!({ "id": id, "phase": "running", "machineState": "running", "reason": wsp_words::words::thread::NOT_GONE }),
        );
        assert_matches(&status_rows(&t.fx.rt).await[0], json!({ "phase": "running", "machineState": "running", "machineId": "m1" }));
    });
}

// "a rebuild: the healthy machine is kept rather than abandoned, and no second machine is built"
#[test]
fn a_rebuild_the_healthy_machine_is_kept_rather_than_abandoned_and_no_second_machine_is_built() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let (id, _, m) = wrongly_gone(&t).await;
        assert_matches(
            &j(&t.fx.rt.workspaces().rebuild(&id, None).await.expect("a rebuild")),
            json!({ "phase": "running", "machineId": "m1" }),
        );
        assert_eq!(t.backend.made(), 1);
        assert!(!m.killed.get());
        assert!(t.record(&id).await.get("gone").is_none());
    });
}

// "the sweep: a machine the listing still carries under a gone record is read once and the record follows it"
#[test]
fn the_sweep_a_machine_the_listing_still_carries_under_a_gone_record_is_read_once_and_the_record_follows_it() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let (id, _, m) = wrongly_gone(&t).await;
        let reads = Reads::of(&m);
        t.fx.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(reads.n(), 1);
        assert_matches(&t.record(&id).await, json!({ "phase": "running", "machineId": "m1" }));
        assert!(t.record(&id).await.get("gone").is_none());
        assert_eq!(t.backend.made(), 1);
    });
}

// "the sweep, over a machine the provider holds paused: the record leaves gone as napping, on the same predicate"
#[test]
fn the_sweep_over_a_machine_the_provider_holds_paused_the_record_leaves_gone_as_napping_on_the_same_predicate() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let (id, _, m) = wrongly_gone(&t).await;
        m.paused.set(true);
        t.fx.rt.reap(None, None).await.expect("a sweep");
        let record = t.record(&id).await;
        assert_matches(&record, json!({ "phase": "napping", "machineId": "m1" }));
        assert!(record.get("gone").is_none());
        assert_eq!(t.backend.made(), 1);
        assert_matches(&t.last_status(), json!({ "id": id, "phase": "napping", "reason": wsp_words::words::thread::NOT_GONE }));
        // The wake road takes it from there the way it takes any napping record: one resume, the same machine.
        assert_matches(&j(&t.fx.rt.workspaces().wake(&id, None).await.expect("a wake")), json!({ "phase": "running", "machineId": "m1" }));
        assert_eq!(m.resumes.get(), 1);
        assert_eq!(t.backend.made(), 1);
    });
}

// "the record load, over a machine the provider holds paused: the record leaves gone as napping, on the same predicate"
#[test]
fn the_record_load_over_a_machine_the_provider_holds_paused_the_record_leaves_gone_as_napping_on_the_same_predicate() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let (id, _, m) = wrongly_gone(&t).await;
        m.paused.set(true);
        t.fx.rt.close().await.expect("a close");
        assert_matches(&t.store.get("workspaces", &id).await.expect("a read").expect("a record"), json!({ "phase": "gone" }));

        let second = test_runtime(|_| {}, Seed { store: Some(t.store.clone()), backend: Some(t.backend.clone()), ..Default::default() });
        assert_matches(&second.record(&id).await, json!({ "phase": "napping", "machineId": "m1" }));
        assert!(second.record(&id).await.get("gone").is_none());
        let stored = t.store.get("workspaces", &id).await.expect("a read").expect("a record");
        assert_matches(&stored, json!({ "phase": "napping" }));
        assert!(stored.get("gone").is_none());
        second.fx.rt.close().await.expect("a close");
    });
}

// "a stored gone record whose machine the provider still runs hydrates running, and the words go with it"
#[test]
fn a_stored_gone_record_whose_machine_the_provider_still_runs_hydrates_running_and_the_words_go_with_it() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let (id, _, _) = wrongly_gone(&t).await;
        t.fx.rt.close().await.expect("a close");
        assert_matches(&t.store.get("workspaces", &id).await.expect("a read").expect("a record"), json!({ "phase": "gone" }));

        let second = test_runtime(|_| {}, Seed { store: Some(t.store.clone()), backend: Some(t.backend.clone()), ..Default::default() });
        assert_matches(&second.record(&id).await, json!({ "phase": "running", "machineId": "m1" }));
        assert!(second.record(&id).await.get("gone").is_none());
        let stored = t.store.get("workspaces", &id).await.expect("a read").expect("a record");
        assert_matches(&stored, json!({ "phase": "running" }));
        assert!(stored.get("gone").is_none());
        second.fx.rt.close().await.expect("a close");
    });
}

const GAP: i64 = 3 * 60_000;

// "the awake meter across a verdict that did not hold" > "a recovery reopens the stretch the gone verdict closed, so the hours the machine went on billing are metered"
#[test]
fn a_recovery_reopens_the_stretch_the_gone_verdict_closed_so_the_hours_the_machine_went_on_billing_are_metered() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        until_n(|| t.costs().iter().any(|c| c["phase"] == "running")).await;
        assert_matches(&t.last_cost(), json!({ "phase": "running", "awakeMs": 0 }));

        m.killed.set(true);
        let refused = t.fx.rt.workspaces().wake(&id, None).await.expect_err("a refusal");
        let gone = t.record(&id).await["gone"].as_str().expect("words").to_string();
        assert!(
            refused.message.contains(&wsp_words::workspace_state::gone_refusal(Some(&ws.view.name), "wake", Some(&gone))),
            "{}",
            refused.message
        );
        until_n(|| t.costs().iter().any(|c| c["phase"] == "gone")).await;
        assert_matches(&t.last_cost(), json!({ "phase": "gone", "rateUsdPerHour": 0, "awakeMs": 0 }));

        // The machine was never gone: it ran and billed through the whole gap, and the read that unmakes the verdict
        // has to hand those hours back to the stretch rather than start a new one here.
        m.killed.set(false);
        t.fc.advance(GAP);
        assert_matches(&j(&t.fx.rt.workspaces().wake(&id, None).await.expect("a wake")), json!({ "phase": "running", "machineId": "m1" }));
        until_n(|| t.last_cost()["phase"] == "running").await;
        assert_matches(&t.last_cost(), json!({ "phase": "running", "awakeMs": GAP }));
        assert!((t.last_cost()["rateUsdPerHour"].as_f64().expect("a rate") - 0.11).abs() < 0.005);

        // The reopened stretch is the stored one, so a host that reads the series back meters those hours too.
        let history = OriginStatusApi::history(&*t.fx.rt.status(), &id, None).await.expect("a history");
        assert_matches(&j(history.last().expect("a point")), json!({ "phase": "running", "awakeMs": GAP }));
    });
}

// "a rebuild out of gone leaves the closed stretch closed: the new machine's first tick counts none of the gap"
#[test]
fn a_rebuild_out_of_gone_leaves_the_closed_stretch_closed_the_new_machines_first_tick_counts_none_of_the_gap() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        m.killed.set(true);
        let refused = t.fx.rt.workspaces().wake(&id, None).await.expect_err("a refusal");
        let gone = t.record(&id).await["gone"].as_str().expect("words").to_string();
        assert!(
            refused.message.contains(&wsp_words::workspace_state::gone_refusal(Some(&ws.view.name), "wake", Some(&gone))),
            "{}",
            refused.message
        );
        until_n(|| t.costs().iter().any(|c| c["phase"] == "gone")).await;
        t.fc.advance(GAP);
        assert_matches(
            &j(&t.fx.rt.workspaces().rebuild(&id, None).await.expect("a rebuild")),
            json!({ "phase": "running", "machineId": "m2" }),
        );
        until_n(|| t.last_cost()["phase"] == "running").await;
        assert_matches(&t.last_cost(), json!({ "phase": "running", "awakeMs": 0 }));
    });
}

/// A machine whose pause is refused until the test says otherwise, with every call counted.
struct Refusing {
    calls: Rc<Cell<u32>>,
    refusing: Rc<Cell<bool>>,
}

fn refusing_pause(m: &Arc<StubMachine>) -> Refusing {
    let pause = m.pause_fn();
    let said = Refusing { calls: Rc::new(Cell::new(0)), refusing: Rc::new(Cell::new(true)) };
    let (calls, refusing, id) = (said.calls.clone(), said.refusing.clone(), m.id().to_string());
    m.set_pause(Rc::new(move || {
        calls.set(calls.get() + 1);
        if refusing.get() {
            let refused = not_pausable(&id);
            return Box::pin(async move { Err(refused) });
        }
        pause()
    }));
    said
}

fn nap_refused() -> String {
    wsp_words::words::thread::nap_refused_line("Not pausable")
}

fn backstop() -> i64 {
    wsp_core::idle::backstop_ms(Some(WINDOW))
}

fn counting_over(state: &State) -> (Rc<dyn Store>, Rc<CountingStore>) {
    let counting = Rc::new(CountingStore { inner: state.store.clone(), puts: Cell::new(0) });
    (counting.clone() as Rc<dyn Store>, counting)
}

// "a nap the provider refuses in words" > "Not pausable is a refusal that stands for the machine: the next window is the backstop, the sentence is on the row and on the poll, the log names the provider's answer once, and the record stays running"
#[test]
fn not_pausable_is_a_refusal_that_stands_for_the_machine_the_next_window_is_the_backstop_the_sentence_is_on_the_row_and_on_the_poll_the_log_names_the_providers_answer_once_and_the_record_stays_running(
) {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let pause = refusing_pause(&t.backend.machine(0));
        t.fc.advance(WINDOW);
        let answered = t.fc.now();
        until_n(|| t.statuses().iter().any(|s| s["phase"] == "running" && s["idleAt"] == answered + backstop())).await;
        assert_eq!(pause.calls.get(), 1);
        assert_eq!(t.phase(&id).await, "running");
        assert_matches(
            &t.last_status(),
            json!({ "phase": "running", "machineState": "running", "reason": nap_refused(), "idleAt": answered + backstop() }),
        );
        assert_matches(&status_rows(&t.fx.rt).await[0], json!({ "reason": nap_refused(), "idleAt": answered + backstop() }));
        assert_eq!(warned.lines(), [format!("the provider does not pause m1 of {id} (Not pausable); an idle nap waits for the backstop and exports no vault until a pause lands")]);
        t.fc.advance(backstop() - 1);
        immediate().await;
        assert_eq!(pause.calls.get(), 1);
        pause.refusing.set(false);
        t.fc.advance(1);
        until(|| async { t.phase(&id).await == "napping" }).await;
        assert_eq!(pause.calls.get(), 2);
    });
}

// "a second window with the refusal standing exports no vault, asks the provider once more only at the backstop, and logs nothing new"
#[test]
fn a_second_window_with_the_refusal_standing_exports_no_vault_asks_the_provider_once_more_only_at_the_backstop_and_logs_nothing_new() {
    drive(|| async {
        let state = sqlite_state();
        let (store, counting) = counting_over(&state);
        let t = test_runtime(|_| {}, Seed { store: Some(store), ..Default::default() });
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let pause = refusing_pause(&t.backend.machine(0));
        t.fc.advance(WINDOW);
        let first = t.fc.now();
        until_n(|| t.statuses().iter().any(|s| s["idleAt"] == first + backstop())).await;
        // The first refusal came after the export the nap always ran; nothing yet said the pause would be refused.
        assert_eq!(counting.puts.get(), 1);
        t.fc.advance(WINDOW);
        immediate().await;
        assert_eq!(pause.calls.get(), 1);
        t.fc.advance(backstop() - WINDOW);
        let second = t.fc.now();
        until_n(|| t.statuses().iter().any(|s| s["idleAt"] == second + backstop())).await;
        assert_eq!(pause.calls.get(), 2);
        assert_eq!(counting.puts.get(), 1);
        assert_matches(&t.last_status(), json!({ "phase": "running", "reason": nap_refused() }));
        assert_eq!(warned.lines().len(), 1);
        assert_eq!(t.phase(&ws.view.id).await, "running");
        drop(state);
    });
}

// "a pause that lands clears the refusal: after the wake the window is the full one again, the row carries no sentence, and the next nap exports its vault"
#[test]
fn a_pause_that_lands_clears_the_refusal_after_the_wake_the_window_is_the_full_one_again_the_row_carries_no_sentence_and_the_next_nap_exports_its_vault(
) {
    drive(|| async {
        let state = sqlite_state();
        let (store, counting) = counting_over(&state);
        let t = test_runtime(|_| {}, Seed { store: Some(store), ..Default::default() });
        let _warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let pause = refusing_pause(&t.backend.machine(0));
        t.fc.advance(WINDOW);
        let answered = t.fc.now();
        until_n(|| t.statuses().iter().any(|s| s["idleAt"] == answered + backstop())).await;
        pause.refusing.set(false);
        t.fx.rt.workspaces().nap(&id, None).await.expect("a nap");
        assert_eq!(t.phase(&id).await, "napping");
        assert_eq!(counting.puts.get(), 1);
        t.fx.rt.workspaces().wake(&id, None).await.expect("a wake");
        let woke = t.fc.now();
        let row = status_rows(&t.fx.rt).await.remove(0);
        assert_eq!(row["idleAt"], woke + WINDOW);
        assert!(row.get("reason").is_none());
        t.fc.advance(WINDOW);
        until(|| async { t.phase(&id).await == "napping" }).await;
        assert_eq!(counting.puts.get(), 2);
        assert_eq!(pause.calls.get(), 3);
        drop(state);
    });
}

// "a pause asked for is rejected with the typed refusal and its sentence, and the refusal stands as the idle nap's does"
#[test]
fn a_pause_asked_for_is_rejected_with_the_typed_refusal_and_its_sentence_and_the_refusal_stands_as_the_idle_naps_does() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let pause = refusing_pause(&t.backend.machine(0));
        let asked = t.fc.now();
        // The Rust `HostError` carries the message, the kind and the status but no class name, so the TS's
        // `toBeInstanceOf(NapRefusedError)` has no field to read here: the message is the sentence that class holds.
        let refused = t.fx.rt.workspaces().nap(&ws.view.id, None).await.expect_err("a refusal");
        assert_eq!(refused.message, nap_refused());
        assert_eq!(pause.calls.get(), 1);
        assert_matches(&t.last_status(), json!({ "phase": "running", "reason": nap_refused() }));
        assert_matches(&status_rows(&t.fx.rt).await[0], json!({ "reason": nap_refused(), "idleAt": asked + WINDOW }));
        assert_eq!(warned.lines().len(), 1);
    });
}

// "a stop the provider refuses while its snapshot fails stands on the row from the first one, in the provider's reading, until a pause lands"
#[test]
fn a_stop_the_provider_refuses_while_its_snapshot_fails_stands_on_the_row_from_the_first_one_in_the_providers_reading_until_a_pause_lands()
{
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m = t.backend.machine(0);
        let said = "its latest snapshot failed at 2026-10-01T00:40:00Z, and the last that completed was at 2026-09-30T21:02:00Z";
        let pause = m.pause_fn();
        let refusing = Rc::new(Cell::new(true));
        let (still, machine) = (refusing.clone(), m.id().to_string());
        m.set_pause(Rc::new(move || {
            if still.get() {
                let refused = EngineError::stop_refused(&machine, said);
                return Box::pin(async move { Err(refused) });
            }
            pause()
        }));
        let refused = t.fx.rt.workspaces().nap(&id, None).await.expect_err("a refusal");
        assert_eq!(refused.message, wsp_words::words::thread::stop_refused_line(said));
        assert_matches(&t.last_status(), json!({ "phase": "running", "reason": wsp_words::words::thread::stop_refused_line(said) }));
        // The poll builds the row from the record, so the sentence holds past the push that first carried it.
        assert_matches(&status_rows(&t.fx.rt).await[0], json!({ "reason": wsp_words::words::thread::stop_refused_line(said) }));
        refusing.set(false);
        t.fx.rt.workspaces().nap(&id, None).await.expect("a nap");
        t.fx.rt.workspaces().wake(&id, None).await.expect("a wake");
        assert!(status_rows(&t.fx.rt).await[0].get("reason").is_none());
    });
}

// "a rebuild that puts another machine under the record drops the refusal: the full window and no sentence on the new machine"
#[test]
fn a_rebuild_that_puts_another_machine_under_the_record_drops_the_refusal_the_full_window_and_no_sentence_on_the_new_machine() {
    drive(|| async {
        let t = test_runtime(|_| {}, Seed::default());
        let _warned = capture();
        let ws = create_on(&t.fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        refusing_pause(&t.backend.machine(0));
        t.fc.advance(WINDOW);
        let answered = t.fc.now();
        until_n(|| t.statuses().iter().any(|s| s["idleAt"] == answered + backstop())).await;
        t.fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        let rebuilt = t.fc.now();
        let row = status_rows(&t.fx.rt).await.remove(0);
        assert_eq!(row["machineId"], "m2");
        assert!(row.get("reason").is_none());
        assert_eq!(row["idleAt"], rebuilt + WINDOW);
    });
}
