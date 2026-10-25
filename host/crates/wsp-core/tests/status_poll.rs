// SPDX-License-Identifier: AGPL-3.0-only
//! spec/status_poll.md; ported from packages/runtime/test/status.test.ts: status.list, a machine that answers for its
//! own daemon, status.watch provider calls, the status poll and a machine's uptime, the status ticks, status zombie at
//! rest, a machine the provider answered it cannot reach, and the reach word a row shows. The TS drove the whole
//! runtime over a stub backend; here each test drives the tracker over a record whose roads it controls.
mod support;

use std::cell::Cell;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use serde_json::json;
use support::net::{http_stub, Delay, HttpStub};
use support::status::*;
use support::{run_local, settle, until};
use tokio::task::JoinHandle;
use wsp_core::status::{Reconcile, RouteReach, StatusListOptions, StatusReader, StatusTracker, StatusWatchOptions, POLL_INTERVAL_MS};
use wsp_engine::errors::EngineError;
use wsp_engine::link_backend::LINK_PROMPT_MS;
use wsp_wire::views::workspace::{MachineFacts, MachineState, ReachState, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceStatus;

use MachineState::{Gone, Paused, Running, Starting};
use ReachState::{NoDaemon, Reachable, Slow, Unreachable, Unsupported, Zombie};

const EXPIRES: f64 = 1_900_000_000_000.0;
const LINE: &str = "the provider cannot reach the machine (Sandbox is not reachable); pause and wake the workspace, or delete it";
const UP: (MachineState, ReachState) = (Running, Reachable);
const DARK: (MachineState, ReachState) = (Running, Unreachable);

fn roads() -> Rc<Roads> {
    Rc::new(Roads::default())
}

fn serve(roads: &Rc<Roads>, stub: &HttpStub) {
    *roads.route.borrow_mut() = Some(Ok(route(&stub.url())));
}

fn on_failure() -> StatusListOptions {
    StatusListOptions { reconcile: Some(Reconcile::OnFailure), ..Default::default() }
}

fn seen(row: &WorkspaceStatus) -> (MachineState, ReachState) {
    (row.machine_state, row.reach.state)
}

fn row<'a>(rows: &'a [WorkspaceStatus], name: &str) -> &'a WorkspaceStatus {
    rows.iter().find(|r| r.name == name).unwrap()
}

fn answered(v: Result<bool, EngineError>) -> LocalBoxFuture<'static, Result<bool, EngineError>> {
    Box::pin(async move { v })
}

fn never() -> LocalBoxFuture<'static, Result<bool, EngineError>> {
    Box::pin(std::future::pending())
}

fn facts_now(f: MachineFacts) -> LocalBoxFuture<'static, Result<MachineFacts, EngineError>> {
    Box::pin(async move { Ok(f) })
}

async fn list_one(t: &Rc<StatusTracker>, o: StatusListOptions) -> WorkspaceStatus {
    t.list(o).await.unwrap().remove(0)
}

fn list_task(t: &Rc<StatusTracker>, o: StatusListOptions) -> JoinHandle<Vec<WorkspaceStatus>> {
    let t = t.clone();
    tokio::task::spawn_local(async move { t.list(o).await.unwrap() })
}

fn counting_facts(rec: &mut wsp_core::status::StatusRecord) -> Rc<Cell<u32>> {
    let reads = Rc::new(Cell::new(0u32));
    let n = reads.clone();
    rec.facts = Some(Rc::new(move || {
        n.set(n.get() + 1);
        facts_now(facts(1_000.0 * n.get() as f64))
    }));
    reads
}

/// A poll tick per interval, each one's probe landed on `stub` and its tick ended.
async fn ticks(rig: &Rig, stub: &HttpStub, interval: i64, count: u32) {
    for n in 1..=count {
        if n > 1 {
            rig.fc.advance(interval);
        }
        until("the probe lands", 5_000, || stub.hits.get() >= n).await;
        until("the tick ends", 5_000, || rig.polled.borrow().len() >= n as usize).await;
    }
}

/// The stub answers 40 ms late in real time; the fake clock moves 20 ms once the request has landed, so the probe
/// reads past the 10 ms prompt whatever the machine's own speed.
async fn slow_tick(rig: &Rig, stub: &HttpStub, nth: u32) {
    until("the probe lands", 5_000, || stub.hits.get() >= nth).await;
    rig.fc.advance(20);
    until("the tick ends", 5_000, || rig.polled.borrow().len() >= nth as usize).await;
}

async fn slow_machine(roads: &Rc<Roads>) -> HttpStub {
    let stub = http_stub(426, Delay::Ms(40)).await;
    serve(roads, &stub);
    stub
}

fn zombie_opts() -> StatusWatchOptions {
    StatusWatchOptions {
        cost_interval_ms: Some(3_600_000),
        poll_interval_ms: Some(POLL_INTERVAL_MS),
        prompt_ms: Some(10),
        probe_timeout_ms: Some(500),
        zombie_window_ms: Some(20_000),
        zombie_probe_timeout_ms: Some(200),
        reconcile_min_ms: None,
    }
}

#[test]
fn enriches_views_with_machine_state_reach_size_and_rate_from_backend_pricing() {
    run_local(async {
        // rules 1, 2, 5
        let rig = rig();
        let r = roads();
        let mut rec = record(view("alpha", "m1", "running"), &r);
        rec.rate_usd_per_hour = 0.11;
        let size = serde_json::to_value(&rec.size).unwrap();
        rig.put(rec);
        let t = rig.tracker(Default::default());
        let rows = t.list(StatusListOptions::default()).await.unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!((row.name.as_str(), row.phase), ("alpha", WorkspacePhase::Running));
        // a record with no route and no answer of its own mints no preview URL
        assert_eq!(seen(row), (Running, Unsupported));
        assert_eq!(field(row, "size"), size);
        assert_eq!(row.rate_usd_per_hour.0, 0.11);
        assert_eq!(r.state_asks.get(), 1);
    });
}

#[test]
fn probes_the_daemon_through_a_minted_preview_url_and_reuses_fresh_reach() {
    run_local(async {
        // rules 5, 7, 10
        let rig = rig();
        let r = roads();
        // plain HTTP against the daemon's ws port answers 426
        let probe = http_stub(426, Delay::Ms(0)).await;
        serve(&r, &probe);
        rig.put(record(view("alpha", "m1", "running"), &r));
        let t = rig.tracker(Default::default());

        let first = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(seen(&first), UP);
        assert!(first.reach.url.as_deref().unwrap().contains(&format!("127.0.0.1:{}", probe.port)));
        assert_eq!(first.reach.expires_at.map(|n| n.0), Some((T0 + 3_600_000) as f64));
        assert_eq!(probe.hits.get(), 1);

        // 502 comes from the edge when nothing listens in the guest
        let dead = http_stub(502, Delay::Ms(0)).await;
        serve(&r, &dead);
        let again = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(seen(&again), UP);
        assert!(again.reach.url.as_deref().unwrap().contains(&format!("127.0.0.1:{}", dead.port)));
        assert_eq!(dead.hits.get(), 1);
        assert_eq!(r.route_asks.get(), 2);
    });
}

#[test]
fn maps_napping_and_gone_machines_without_probing() {
    run_local(async {
        // rules 3, 4
        let rig = rig();
        let ra = roads();
        let rb = roads();
        let stub = http_stub(426, Delay::Ms(0)).await;
        serve(&ra, &stub);
        *ra.state.borrow_mut() = Ok(Paused);
        rig.put(record(view("a", "m1", "napping"), &ra));
        let mut gone_view = view("b", "m2", "gone");
        gone_view.gone = Some("machine m2 is gone at the provider".into());
        rig.put(record(gone_view, &rb));
        let t = rig.tracker(Default::default());

        let rows = t.list(StatusListOptions::default()).await.unwrap();
        assert_eq!(seen(row(&rows, "a")), (Paused, ReachState::Napping));
        assert_eq!(seen(row(&rows, "b")), (Gone, ReachState::Gone));
        assert_eq!(row(&rows, "b").reason.as_deref(), Some("machine m2 is gone at the provider"));
        assert_eq!((stub.hits.get(), ra.route_asks.get(), rb.state_asks.get()), (0, 0, 0));
        assert!(rig.gone.borrow().is_empty());
    });
}

#[test]
fn an_explicit_list_asks_the_provider_once_per_machine_and_never_list() {
    run_local(async {
        // rules 3, 14
        let rig = rig();
        let r = roads();
        rig.put(record(view("alpha", "m1", "running"), &r));
        let t = rig.tracker(Default::default());

        let first = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(first.machine_state, Running);
        assert_eq!(r.state_asks.get(), 1);

        // the provider paused it behind our back
        *r.state.borrow_mut() = Ok(Paused);
        let paused = list_one(&t, StatusListOptions::default()).await;
        assert_eq!((paused.phase, paused.machine_state), (WorkspacePhase::Running, Paused));

        *r.state.borrow_mut() = Err(missing("sandbox not found"));
        let lost = list_one(&t, StatusListOptions::default()).await;
        // One gone read is not gone: the row reads the record's word until the reads that confirm it agree.
        assert_eq!((lost.phase, lost.machine_state), (WorkspacePhase::Running, Running));
        assert_eq!(
            *rig.gone.borrow(),
            vec![(
                "alpha".to_string(),
                "m1".to_string(),
                "machine m1 is gone at the provider: the status poll found it gone at 2026-10-25T10:00:00Z (404 sandbox not found)".to_string()
            )]
        );

        rig.set_phase("alpha", WorkspacePhase::Gone);
        let asked = r.state_asks.get();
        let gone = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(seen(&gone), (Gone, ReachState::Gone));
        assert_eq!(r.state_asks.get(), asked);
        assert_eq!(rig.gone.borrow().len(), 1);
    });
}

#[test]
fn maps_a_prompt_502_to_no_daemon_a_late_answer_to_slow_silence_to_unreachable_and() {
    run_local(async {
        // rules 5, 10, 11
        // The stub moves the clock as the request lands, so "late" is what the clock says and not how fast the box answered.
        // The probe timeout is a real abort: only the silent case keeps it short, the answering ones get more than any local fetch needs.
        let cases: [(u16, Delay, i64, i64, ReachState); 9] = [
            (426, Delay::Ms(0), 0, 5_000, Reachable),
            (502, Delay::Ms(0), 0, 5_000, NoDaemon),
            // Not the guest's answer, so it decides nothing on its own: an edge refusal must not take a live workspace
            // out of service in the row, it only sends the poll to the provider for the machine's real state.
            (200, Delay::Ms(0), 0, 5_000, Reachable),
            (401, Delay::Ms(0), 0, 5_000, Reachable),
            (404, Delay::Ms(0), 0, 5_000, Reachable),
            (503, Delay::Ms(0), 0, 5_000, Reachable),
            (502, Delay::Ms(200), 150, 5_000, Slow),
            (426, Delay::Ms(200), 150, 5_000, Slow),
            (426, Delay::Never, 0, 300, Unreachable),
        ];
        for (code, delay, took, timeout, expected) in cases {
            let rig = rig();
            let r = roads();
            let stub = http_stub(code, delay).await;
            serve(&r, &stub);
            rig.put(record(view("alpha", "m1", "running"), &r));
            let t = rig.tracker(Default::default());
            let o = StatusListOptions { probe_timeout_ms: Some(timeout), prompt_ms: Some(60), ..Default::default() };
            let h = list_task(&t, o);
            if took > 0 {
                until("the request lands", 5_000, || stub.hits.get() >= 1).await;
                rig.fc.advance(took);
            }
            let rows = h.await.unwrap();
            assert_eq!(seen(&rows[0]), (Running, expected), "{code} after {took}");
            stub.close();
        }
    });
}

#[test]
fn takes_the_guests_own_word_and_never_dials_the_route_a_container_whose_published() {
    run_local(async {
        // rules 5, 10
        let rig = rig();
        let r = roads();
        // The route mints and the row carries it, but nothing on this computer answers it: on a box dialling another
        // computer's Docker daemon that is the whole of what the old reading saw.
        let edge = http_stub(502, Delay::Ms(0)).await;
        *r.route.borrow_mut() = Some(Ok(RouteReach { url: edge.url(), expires_at: 9_007_199_254_740_991.0 }));
        *r.answers.borrow_mut() = Some(Ok(true));
        let mut rec = record(view("alpha", "m1", "running"), &r);
        answering(&mut rec, &r);
        rig.put(rec);
        let t = rig.tracker(Default::default());

        let row = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(seen(&row), UP);
        assert!(row.reach.url.as_deref().unwrap().contains(&format!("127.0.0.1:{}", edge.port)));
        assert_eq!(edge.hits.get(), 0);
        assert_eq!(r.answer_asks.get(), 1);
    });
}

#[test]
fn a_guest_that_says_the_daemons_port_is_dead_reads_unreachable_and_a_machine_that() {
    run_local(async {
        // rules 5, 7, 10
        let rig = rig();
        let r = roads();
        *r.route.borrow_mut() = Some(Ok(RouteReach { url: "http://127.0.0.1:1/".into(), expires_at: 9_007_199_254_740_991.0 }));
        *r.answers.borrow_mut() = Some(Ok(false));
        let mut rec = record(view("alpha", "m1", "running"), &r);
        answering(&mut rec, &r);
        rig.put(rec);
        let t = rig.tracker(Default::default());

        let dead = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(seen(&dead), (Running, NoDaemon));

        *r.answers.borrow_mut() = Some(Err(other("container 7d8f is not running")));
        let silent = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(seen(&silent), (Running, Unreachable));
    });
}

#[test]
fn a_route_the_machine_has_none_of_is_no_silence_the_reach_is_still_the_guests_word() {
    run_local(async {
        // rules 5, 9, 10
        let rig = rig();
        let r = roads();
        *r.route.borrow_mut() = Some(Err(other("container 7d8f publishes no port 7070; only the daemon's own port is published")));
        *r.answers.borrow_mut() = Some(Ok(true));
        let mut rec = record(view("alpha", "m1", "running"), &r);
        answering(&mut rec, &r);
        rig.put(rec);
        let t = rig.tracker(Default::default());

        let row = list_one(&t, on_failure()).await;
        assert_eq!(seen(&row), UP);
        assert_eq!(field(&row, "reach"), json!({"state": "reachable"}));
        // Nothing failed, so the provider is left alone: a route the app has none of is not a reach that missed.
        assert_eq!(r.state_asks.get(), 0);
        assert_eq!(r.route_asks.get(), 1);
    });
}

#[test]
fn a_machine_with_no_route_at_all_is_asked_all_the_same_only_a_machine_with_neither() {
    run_local(async {
        // rules 5, 7, 10, 18
        let rig = rig();
        let r = roads();
        *r.answers.borrow_mut() = Some(Ok(true));
        let mut rec = record(view("alpha", "m1", "running"), &r);
        answering(&mut rec, &r);
        rig.put(rec);
        let t = rig.tracker(Default::default());

        let reach = |row: &WorkspaceStatus| field(row, "reach");
        assert_eq!(reach(&list_one(&t, StatusListOptions::default()).await), json!({"state": "reachable"}));
        *r.answers.borrow_mut() = Some(Ok(false));
        // The first silence after an answer keeps the answer's word, as it does on the edge road; the second turns it.
        assert_eq!(reach(&list_one(&t, StatusListOptions::default()).await), json!({"state": "reachable"}));
        assert_eq!(reach(&list_one(&t, StatusListOptions::default()).await), json!({"state": "no-daemon"}));
        // Neither road: nothing to ask, and that alone is unsupported.
        rig.put(record(view("alpha", "m1", "running"), &r));
        let last = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(reach(&last), json!({"state": "unsupported"}));
        assert_eq!(last.machine_state, Running);
    });
}

#[test]
fn a_guest_that_takes_longer_than_the_prompt_reads_slow_and_one_inside_it_reads_rea() {
    run_local(async {
        // rules 5, 10, 11
        let rig = rig();
        let r = roads();
        let mut rec = record(view("alpha", "m1", "running"), &r);
        // The clock moves as the guest answers, so slow is what the clock says and not how fast this machine ran.
        let took = Rc::new(Cell::new(0i64));
        let (fc, tk) = (rig.fc.clone(), took.clone());
        rec.daemon_answers = Some(Rc::new(move |_: Option<i64>| {
            fc.advance(tk.get());
            answered(Ok(true))
        }));
        rig.put(rec);
        let t = rig.tracker(Default::default());
        let o = StatusListOptions { prompt_ms: Some(60), probe_timeout_ms: Some(5_000), ..Default::default() };

        took.set(40);
        assert_eq!(seen(&list_one(&t, o).await), UP);
        took.set(150);
        // Three Docker API round trips ride this road where the edge road rode one, so the slow word is reachable here.
        assert_eq!(seen(&list_one(&t, o).await), (Running, Slow));
        assert_eq!(seen(&list_one(&t, o).await), (Running, Slow));
    });
}

#[test]
fn the_slow_prompt_is_the_roads_own_the_loopback_socket_keeps_2_5_s_and_a_joined_co() {
    run_local(async {
        // rules 5, 10, 11
        // The loopback socket: one HTTP GET, the clock moving once the daemon's answer is on its way.
        let rig_socket = rig();
        let r = roads();
        let socket = http_stub(426, Delay::Ms(200)).await;
        serve(&r, &socket);
        rig_socket.put(record(view("socket", "m1", "running"), &r));
        let t = rig_socket.tracker(Default::default());
        let o = StatusListOptions { probe_timeout_ms: Some(5_000), ..Default::default() };
        for (n, took, expected) in [(1u32, 2_400i64, Reachable), (2, 2_600, Slow)] {
            let h = list_task(&t, o);
            until("the request lands", 5_000, || socket.hits.get() >= n).await;
            rig_socket.fc.advance(took);
            let rows = h.await.unwrap();
            assert_eq!(seen(&rows[0]), (Running, expected), "socket after {took}");
        }

        // The joined computer's road is the link machine's own, with the prompt the link declares.
        let rig_box = rig();
        let r = roads();
        let mut rec = record(view("box", "m2", "running"), &r);
        let took = Rc::new(Cell::new(0i64));
        let (fc, tk) = (rig_box.fc.clone(), took.clone());
        rec.daemon_answers = Some(Rc::new(move |_: Option<i64>| {
            fc.advance(tk.get());
            answered(Ok(true))
        }));
        rec.daemon_answers_prompt_ms = Some(LINK_PROMPT_MS as i64);
        rig_box.put(rec);
        let t = rig_box.tracker(Default::default());
        let o = StatusListOptions { probe_timeout_ms: Some(60_000), ..Default::default() };
        // Past the socket's prompt and well inside the link's: a healthy guest over a joined computer's road.
        for (ms, expected) in [(2_600, Reachable), (LINK_PROMPT_MS as i64 - 100, Reachable), (LINK_PROMPT_MS as i64 + 100, Slow)] {
            took.set(ms);
            assert_eq!(seen(&list_one(&t, o).await), (Running, expected), "box after {ms}");
        }
    });
}

#[test]
fn the_callers_bound_is_the_whole_ask_a_machine_that_never_answers_reads_unreachabl() {
    run_local(async {
        // rules 5, 11
        let rig = rig();
        let r = roads();
        *r.route.borrow_mut() = Some(Ok(RouteReach { url: "http://127.0.0.1:1/".into(), expires_at: 9_007_199_254_740_991.0 }));
        let mut rec = record(view("alpha", "m1", "running"), &r);
        let bound: Rc<Cell<Option<i64>>> = Rc::default();
        let b = bound.clone();
        rec.daemon_answers = Some(Rc::new(move |timeout: Option<i64>| {
            b.set(timeout);
            never()
        }));
        rig.put(rec);
        let t = rig.tracker(Default::default());

        let h = list_task(&t, StatusListOptions { probe_timeout_ms: Some(300), ..Default::default() });
        settle().await;
        assert!(!h.is_finished());
        rig.fc.advance(300);
        settle().await;
        let rows = h.await.unwrap();
        assert_eq!(seen(&rows[0]), (Running, Unreachable));
        assert_eq!(bound.get(), Some(300));
        assert_eq!(rows[0].reach.offline, None);
    });
}

#[test]
fn asks_the_provider_nothing_across_healthy_polls() {
    run_local(async {
        // rules 14, 16, 19, 20
        let rig = rig();
        let alpha = roads();
        let bare = roads();
        let probe = http_stub(426, Delay::Ms(0)).await;
        serve(&alpha, &probe);
        rig.put(record(view("alpha", "m1", "running"), &alpha));
        // unsupported reach is healthy too
        rig.put(record(view("no-preview", "m2", "running"), &bare));
        let o = StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(5_000), ..Default::default() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        ticks(&rig, &probe, 5_000, 8).await;
        stop();
        assert_eq!((alpha.state_asks.get(), bare.state_asks.get()), (0, 0));
        let first = rig.polled.borrow()[0].clone();
        assert_eq!(seen(row(&first, "alpha")), UP);
        assert_eq!(seen(row(&first, "no-preview")), (Running, Unsupported));
    });
}

#[test]
fn asks_the_provider_once_after_a_failed_reach_then_not_again_inside_the_reconcile() {
    run_local(async {
        // rules 14, 16, 19, 20
        let rig = rig();
        let r = roads();
        let dead = http_stub(502, Delay::Ms(0)).await;
        serve(&r, &dead);
        rig.put(record(view("alpha", "m1", "running"), &r));
        let o = StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(5_000), reconcile_min_ms: Some(60_000), ..Default::default() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        ticks(&rig, &dead, 5_000, 8).await;
        stop();
        assert_eq!(r.state_asks.get(), 1);
        let statuses = rig.bus.statuses();
        assert_eq!(seen(statuses.last().unwrap()), (Running, NoDaemon));
    });
}

#[test]
fn emits_workspace_status_when_the_enriched_status_changes_not_on_every_poll() {
    run_local(async {
        // rules 19, 20, 21
        let rig = rig();
        let r = roads();
        rig.put(record(view("alpha", "m1", "running"), &r));
        let o = StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(5_000), ..Default::default() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        // first poll baselines every workspace once
        until("the first status", 5_000, || !rig.bus.statuses().is_empty()).await;
        settle().await;
        let baseline = rig.bus.statuses();
        assert_eq!(baseline.len(), 1);
        assert_eq!((baseline[0].phase, baseline[0].machine_state), (WorkspacePhase::Running, Running));

        *r.state.borrow_mut() = Ok(Paused);
        rig.set_phase("alpha", WorkspacePhase::Napping);
        rig.fc.advance(5_000);
        settle().await;
        let moved = rig.bus.statuses();
        assert_eq!(moved.len(), 2);
        assert_eq!((moved[1].id.as_str(), moved[1].phase, moved[1].machine_state, moved[1].reach.state), ("alpha", WorkspacePhase::Napping, Paused, ReachState::Napping));

        for _ in 0..3 {
            rig.fc.advance(5_000);
            settle().await;
        }
        stop();
        assert_eq!(rig.bus.statuses().len(), 2);
        assert_eq!(rig.polled.borrow().len(), 5);
    });
}

#[test]
fn does_not_send_a_row_again_when_only_the_machines_uptime_moved() {
    run_local(async {
        // rules 2, 13, 19, 20, 21
        let rig = rig();
        let r = roads();
        let mut rec = record(view("ws_up", "m_up", "running"), &r);
        let reads = counting_facts(&mut rec);
        rig.put(rec);
        let o = StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(5_000), ..Default::default() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        settle().await;
        for _ in 0..5 {
            rig.fc.advance(5_000);
            settle().await;
        }
        stop();
        assert!(reads.get() >= 6);
        let sent = rig.bus.statuses();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].facts.as_ref().unwrap().uptime_ms.0, 1_000.0);
    });
}

#[test]
fn does_not_send_a_row_again_after_a_push_that_carried_no_facts_and_an_older_read_s() {
    run_local(async {
        // rules 19, 20, 21
        let rig = rig();
        let r = roads();
        let mut rec = record(view("ws_push", "m_push", "running"), &r);
        let reads = counting_facts(&mut rec);
        // The runtime's records carry the checkout last read, as the rows it polls do.
        rec.checkout = Some(serde_json::from_value(json!({"branch": "b1", "ahead": 0, "behind": 0, "changed": 0, "readAt": 1})).unwrap());
        rig.put(rec);
        let o = StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(5_000), ..Default::default() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        until("the first status", 5_000, || rig.bus.statuses().len() == 1).await;
        settle().await;

        // The runtime's own push after a checkout read: no facts, no route, a fresher stamp, and nothing a client draws moved.
        let mut pushed = serde_json::to_value(&rig.bus.statuses()[0]).unwrap();
        pushed.as_object_mut().unwrap().remove("facts");
        pushed["checkout"]["readAt"] = json!(2);
        rig.bus.fire(&event(json!({"type": "workspace.status", "status": pushed})));

        let after = reads.get();
        for _ in 0..4 {
            rig.fc.advance(5_000);
            settle().await;
        }
        stop();
        assert!(reads.get() >= after + 4);
        assert_eq!(rig.bus.statuses().len(), 1);
    });
}

#[test]
fn an_answer_the_guest_did_not_send_sends_the_poll_to_the_provider_once_and_leaves() {
    run_local(async {
        // rules 10, 14, 16
        let rig = rig();
        let r = roads();
        // The edge refusing a token it minted itself: the request never reached the guest, so it says nothing about it.
        let edge = http_stub(401, Delay::Ms(0)).await;
        serve(&r, &edge);
        rig.put(record(view("alpha", "m1", "running"), &r));
        let o = StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(5_000), reconcile_min_ms: Some(60_000), ..Default::default() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        ticks(&rig, &edge, 5_000, 8).await;
        stop();
        assert_eq!(r.state_asks.get(), 1);
        let statuses = rig.bus.statuses();
        let last = statuses.last().unwrap();
        assert_eq!((last.phase, seen(last)), (WorkspacePhase::Running, UP));
    });
}

#[test]
fn a_workspace_that_goes_takes_its_facts_backoff_with_it_so_the_next_workspace_unde() {
    run_local(async {
        // rules 2, 13
        let rig = rig();
        let r = roads();
        // A machine that is off: every read of what it is costs a dial and answers nothing.
        *r.facts.borrow_mut() = Some(Err(other("did not say what it is over ssh")));
        rig.put(record(view("ws_a", "m_box", "running"), &r));
        let t = rig.tracker(Default::default());

        let first = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(r.facts_asks.get(), 1);
        assert_eq!((first.facts.is_none(), seen(&first)), (true, (Running, Unsupported)));
        // Backed off: a second read inside the window spends no dial.
        let second = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(r.facts_asks.get(), 1);
        assert!(second.facts.is_none());
        // The workspace is deleted. Its entry goes with every other per-workspace thing the tracker held, so a record
        // that turns up under that id again is a new workspace and is asked at once rather than serving out the
        // window the dead one earned.
        rig.bus.fire(&deleted("ws_a"));
        let third = list_one(&t, StatusListOptions::default()).await;
        assert_eq!(r.facts_asks.get(), 2);
        assert!(third.facts.is_none());
    });
}

#[test]
fn a_tick_that_throws_is_logged_and_the_next_tick_still_runs_nothing_reaches_the_pr() {
    run_local(async {
        // rules 19, 20
        let rig = rig();
        let r = roads();
        rig.put(record(view("alpha", "m1", "running"), &r));
        rig.records_fail.set(true);
        let warned = wsp_core::console::capture();
        let o = StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(5_000), ..Default::default() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        settle().await;
        for _ in 0..3 {
            rig.fc.advance(5_000);
            settle().await;
        }
        stop();
        assert_eq!(rig.listed.get(), 4);
        let failed = warned.lines().iter().filter(|l| l.contains("status poll tick failed")).count();
        assert_eq!(failed, 4);
        assert!(rig.bus.statuses().is_empty());
        assert!(rig.polled.borrow().is_empty());
    });
}

#[test]
fn reach_slow_past_the_window_and_a_failing_exec_probe_mark_the_workspace_zombie_wi() {
    run_local(async {
        // rules 8, 11, 12, 17
        let rig = rig();
        let r = roads();
        let stub = slow_machine(&r).await;
        *r.exec.borrow_mut() = Some(Ok(exec_failed(1, "502 exec failed")));
        rig.put(record(view("ws_z", "m1", "running"), &r));
        let warned = wsp_core::console::capture();
        let o = zombie_opts();
        let t = rig.tracker(o);
        let stop = t.watch(o);
        slow_tick(&rig, &stub, 1).await;
        for n in 2..=3 {
            rig.fc.advance(POLL_INTERVAL_MS);
            slow_tick(&rig, &stub, n).await;
        }

        let statuses = rig.bus.statuses();
        let zombie = statuses.iter().find(|s| s.reach.state == Zombie).unwrap();
        assert_eq!((zombie.id.as_str(), zombie.phase, zombie.machine_state, zombie.machine_id.as_str()), ("ws_z", WorkspacePhase::Running, Running, "m1"));
        // the spell began when the first slow answer landed, 20 ms in, and the third landed 30 060 ms in
        let reason = "m1 reports running at the provider; reach slow since 2026-10-25T10:00:00.020Z (30 s); exec probe \"echo ok\" failed after 0 ms (exit 1: 502 exec failed)";
        assert_eq!(zombie.reason.as_deref(), Some(reason));
        assert!(statuses.iter().any(|s| s.reach.state == Slow && s.reason.is_none()));

        // The mark sticks: later polls keep saying zombie without probing again.
        for n in 4..=5 {
            rig.fc.advance(POLL_INTERVAL_MS);
            slow_tick(&rig, &stub, n).await;
        }
        stop();
        assert_eq!(r.exec_asks.get(), 1);
        let statuses = rig.bus.statuses();
        assert_eq!(statuses.iter().filter(|s| s.reach.state == Zombie).count(), 1);
        assert_eq!(statuses.last().unwrap().reach.state, Zombie);
        assert_eq!(warned.lines(), vec![format!("zombie on m1 (workspace ws_z): {reason}")]);
    });
}

#[test]
fn a_slow_spell_where_exec_answers_never_flags_and_the_probe_repeats_once_per_windo() {
    run_local(async {
        // rules 8, 11, 12
        let rig = rig();
        let r = roads();
        let stub = slow_machine(&r).await;
        *r.exec.borrow_mut() = Some(Ok(exec_ok()));
        rig.put(record(view("ws_w", "m1", "running"), &r));
        let o = zombie_opts();
        let t = rig.tracker(o);
        let stop = t.watch(o);
        slow_tick(&rig, &stub, 1).await;
        for n in 2..=5 {
            rig.fc.advance(POLL_INTERVAL_MS);
            slow_tick(&rig, &stub, n).await;
        }
        stop();
        // The first probe lands in the third poll, its answer restarts the window, and the second lands in the fifth.
        assert_eq!(r.exec_asks.get(), 2);
        assert!(stub.hits.get() > r.exec_asks.get() * 2);
        let statuses = rig.bus.statuses();
        assert!(statuses.iter().all(|s| s.reach.state == Slow && s.reason.is_none() && s.machine_state == Running));
        assert_eq!(statuses.len(), 1);
    });
}

#[test]
fn an_exec_probe_that_never_returns_is_cut_at_its_timeout_and_counts_as_failed() {
    run_local(async {
        // rules 12, 17
        let rig = rig();
        let r = roads();
        let stub = slow_machine(&r).await;
        *r.exec.borrow_mut() = None;
        rig.put(record(view("ws_h", "m1", "running"), &r));
        let warned = wsp_core::console::capture();
        let o = StatusWatchOptions { zombie_probe_timeout_ms: Some(60), ..zombie_opts() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        slow_tick(&rig, &stub, 1).await;
        rig.fc.advance(POLL_INTERVAL_MS);
        slow_tick(&rig, &stub, 2).await;
        rig.fc.advance(POLL_INTERVAL_MS);
        until("the probe starts", 5_000, || stub.hits.get() >= 3).await;
        rig.fc.advance(20);
        until("the exec probe is asked", 5_000, || r.exec_asks.get() >= 1).await;
        rig.fc.advance(60);
        until("the tick ends", 5_000, || rig.polled.borrow().len() >= 3).await;
        stop();
        let statuses = rig.bus.statuses();
        let zombie = statuses.iter().find(|s| s.reach.state == Zombie).unwrap();
        assert_eq!(zombie.machine_state, Running);
        assert_eq!(
            zombie.reason.as_deref(),
            Some("m1 reports running at the provider; reach slow since 2026-10-25T10:00:00.020Z (30 s); exec probe \"echo ok\" failed after 60 ms (timed out after 60 ms)")
        );
        assert_eq!(warned.lines().len(), 1);
    });
}

#[test]
fn a_listing_that_will_not_wait_spends_no_exec_probe_on_a_machine_dark_past_the_win() {
    run_local(async {
        // rules 8, 12
        let rig = rig();
        let r = roads();
        let stub = slow_machine(&r).await;
        // A probe that never returns: had the listing started one it would have cost the whole zombie timeout.
        *r.exec.borrow_mut() = None;
        rig.put(record(view("ws_d", "m1", "running"), &r));
        let o = zombie_opts();
        let t = rig.tracker(o);
        let listing = StatusListOptions { reconcile: Some(Reconcile::OnFailure), zombie_probe: Some(false), prompt_ms: Some(10), probe_timeout_ms: Some(500), ..Default::default() };

        let h = list_task(&t, listing);
        until("the request lands", 5_000, || stub.hits.get() >= 1).await;
        rig.fc.advance(20);
        assert_eq!(seen(&h.await.unwrap()[0]), (Running, Slow));

        // The listing is past the window when its slow answer lands.
        let h = list_task(&t, listing);
        until("the request lands", 5_000, || stub.hits.get() >= 2).await;
        rig.fc.advance(20_020);
        let past = h.await.unwrap().remove(0);
        assert_eq!(seen(&past), (Running, Slow));
        assert_eq!(r.exec_asks.get(), 0);

        // The verdict is the poller's, and it still reaches it on the same machine.
        let stop = t.watch(o);
        until("the poll's request lands", 5_000, || stub.hits.get() >= 3).await;
        rig.fc.advance(20);
        until("the exec probe is asked", 5_000, || r.exec_asks.get() >= 1).await;
        rig.fc.advance(200);
        until("a zombie status", 5_000, || rig.bus.statuses().iter().any(|s| s.reach.state == Zombie)).await;
        stop();
        let statuses = rig.bus.statuses();
        let zombie = statuses.iter().find(|s| s.reach.state == Zombie).unwrap();
        assert_eq!(zombie.machine_state, Running);
        assert_eq!(r.exec_asks.get(), 1);
    });
}

#[test]
fn a_machine_the_provider_says_is_paused_gets_no_probe_the_divergence_already_expla() {
    run_local(async {
        // rules 12
        let rig = rig();
        let r = roads();
        let stub = slow_machine(&r).await;
        *r.state.borrow_mut() = Ok(Paused);
        rig.put(record(view("ws_p", "m1", "running"), &r));
        let o = zombie_opts();
        let t = rig.tracker(o);
        let stop = t.watch(o);
        slow_tick(&rig, &stub, 1).await;
        for n in 2..=3 {
            rig.fc.advance(POLL_INTERVAL_MS);
            slow_tick(&rig, &stub, n).await;
        }
        let paused = rig.bus.statuses().into_iter().find(|s| s.machine_state == Paused).unwrap();
        assert_eq!((paused.reach.state, paused.reason.clone()), (Slow, None));

        for n in 4..=5 {
            rig.fc.advance(POLL_INTERVAL_MS);
            slow_tick(&rig, &stub, n).await;
        }
        stop();
        assert_eq!(r.exec_asks.get(), 0);
        assert!(rig.bus.statuses().iter().all(|s| s.reach.state != Zombie));
    });
}

struct Marked {
    rig: Rig,
    roads: Rc<Roads>,
    tracker: Rc<StatusTracker>,
}

impl Marked {
    /// A running fork whose daemon answers over its route while the provider refuses every command on it.
    async fn new(phase: &str, exec: Option<Result<wsp_engine::machine::ExecResult, EngineError>>) -> Marked {
        let rig = rig();
        let roads = roads();
        let daemon = http_stub(426, Delay::Ms(0)).await;
        *roads.route.borrow_mut() = Some(Ok(RouteReach { url: format!("http://127.0.0.1:{}/?port=7681", daemon.port), expires_at: EXPIRES }));
        *roads.exec.borrow_mut() = exec;
        let mut rec = record(view("ws_u", "sbx_1", phase), &roads);
        rec.rate_usd_per_hour = 0.11;
        rec.unreached = Some(LINE.into());
        rig.put(rec);
        let tracker = rig.tracker(StatusWatchOptions { zombie_probe_timeout_ms: Some(2_000), ..Default::default() });
        Marked { rig, roads, tracker }
    }

    async fn refused() -> Marked {
        Marked::new("running", Some(Err(other(LINE)))).await
    }

    /// Down, the mint fails the way a lookup on this computer does, which is a tick that learns nothing.
    fn road_down(&self) {
        *self.roads.route.borrow_mut() = Some(Err(offline()));
    }

    async fn poll(&self, o: StatusListOptions) -> WorkspaceStatus {
        self.rig.fc.advance(POLL_INTERVAL_MS);
        list_one(&self.tracker, o).await
    }
}

#[test]
fn reads_unreachable_on_the_first_poll_with_the_providers_sentence_asks_the_provide() {
    run_local(async {
        // rules 8, 14
        let m = Marked::refused().await;
        let status = m.poll(StatusListOptions::default()).await;
        assert_eq!(seen(&status), DARK);
        assert_eq!(status.reason.as_deref(), Some(LINE));
        settle().await;
        assert_eq!(m.roads.state_asks.get(), 1);
        assert_eq!(m.roads.exec_asks.get(), 1);
    });
}

#[test]
fn keeps_the_route_minted_and_probed_as_every_running_records_so_the_apps_preview_k() {
    run_local(async {
        // rules 5, 8
        let m = Marked::refused().await;
        let status = m.poll(StatusListOptions::default()).await;
        assert_eq!(m.roads.route_asks.get(), 1);
        let url = status.reach.url.clone().unwrap();
        assert!(url.starts_with("http://127.0.0.1:") && url.ends_with("/?port=7681"), "{url}");
        assert_eq!(status.reach.expires_at.map(|n| n.0), Some(EXPIRES));
        assert_eq!(seen(&status), DARK);
    });
}

#[test]
fn keeps_unreachable_and_the_sentence_across_a_tick_where_this_computers_own_road_f() {
    run_local(async {
        // rules 6, 8
        let m = Marked::refused().await;
        assert_eq!(m.poll(StatusListOptions::default()).await.reach.state, Unreachable);
        m.road_down();
        let status = m.poll(StatusListOptions::default()).await;
        assert_eq!((seen(&status), status.reach.offline), (DARK, Some(true)));
        assert_eq!(status.reason.as_deref(), Some(LINE));
    });
}

#[test]
fn reads_unreachable_with_the_sentence_when_this_computers_own_road_is_down_from_a() {
    run_local(async {
        // rules 6, 8
        let m = Marked::refused().await;
        m.road_down();
        let table = StatusListOptions { zombie_probe: Some(false), reader: Some(StatusReader::Table), ..Default::default() };
        for status in [m.poll(StatusListOptions::default()).await, m.poll(table).await] {
            assert_eq!((seen(&status), status.reach.offline), (DARK, Some(true)));
            assert_eq!(status.reason.as_deref(), Some(LINE));
        }
    });
}

#[test]
fn spends_no_exec_on_a_listing_that_will_not_wait_and_keeps_the_sentence_and_the_ro() {
    run_local(async {
        // rules 8
        let m = Marked::refused().await;
        let table = m.poll(StatusListOptions { zombie_probe: Some(false), reader: Some(StatusReader::Table), ..Default::default() }).await;
        assert_eq!(seen(&table), DARK);
        assert_eq!(table.reason.as_deref(), Some(LINE));
        assert_eq!(table.reach.expires_at.map(|n| n.0), Some(EXPIRES));
        assert!(table.reach.url.as_deref().unwrap().contains("127.0.0.1"));
        settle().await;
        assert_eq!(m.roads.exec_asks.get(), 0);
        let polled = m.poll(StatusListOptions::default()).await;
        assert_eq!(polled.reason.as_deref(), Some(LINE));
        settle().await;
        assert_eq!(m.roads.exec_asks.get(), 1);
    });
}

#[test]
fn holds_one_exec_probe_in_flight_however_many_ticks_pass_while_the_provider_hangs() {
    run_local(async {
        // rules 8, 17
        let m = Marked::new("running", None).await;
        assert_eq!(seen(&m.poll(StatusListOptions::default()).await), DARK);
        assert_eq!(seen(&m.poll(StatusListOptions::default()).await), DARK);
        let third = m.poll(StatusListOptions::default()).await;
        assert_eq!((seen(&third), third.reason.as_deref()), (DARK, Some(LINE)));
        settle().await;
        assert_eq!(m.roads.exec_asks.get(), 1);
    });
}

#[test]
fn past_the_zombie_window_with_the_probe_failing_reads_zombie_the_sentence_still_it() {
    run_local(async {
        // rules 8, 12
        let m = Marked::refused().await;
        let warned = wsp_core::console::capture();
        assert_eq!(m.poll(StatusListOptions::default()).await.reach.state, Unreachable);
        m.rig.fc.advance(3 * 60_000);
        let status = m.poll(StatusListOptions::default()).await;
        assert_eq!(seen(&status), (Running, Zombie));
        assert_eq!(status.reason.as_deref(), Some(LINE));
        let lines = warned.lines();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("zombie on sbx_1 (workspace ws_u): "), "{}", lines[0]);
        assert!(lines[0].contains("exec probe \"echo ok\" failed after 0 ms ("), "{}", lines[0]);
    });
}

#[test]
fn reads_what_the_probe_says_again_once_the_mark_is_off() {
    run_local(async {
        // rules 8, 10
        let m = Marked::new("running", Some(Ok(exec_ok()))).await;
        assert_eq!(m.poll(StatusListOptions::default()).await.reason.as_deref(), Some(LINE));
        m.rig.records.borrow_mut()[0].unreached = None;
        let status = m.poll(StatusListOptions::default()).await;
        assert_eq!(seen(&status), UP);
        assert_eq!(status.reason, None);
    });
}

#[test]
fn a_napping_record_with_the_mark_reads_napping_and_asks_the_machine_nothing() {
    run_local(async {
        // rules 4, 8
        let m = Marked::new("napping", Some(Err(other(LINE)))).await;
        *m.roads.state.borrow_mut() = Ok(Paused);
        let status = m.poll(StatusListOptions::default()).await;
        assert_eq!(seen(&status), (Paused, ReachState::Napping));
        assert_eq!(status.reason, None);
        settle().await;
        assert_eq!(m.roads.exec_asks.get(), 0);
        assert_eq!(m.roads.route_asks.get(), 0);
    });
}

/// The probe's timeout is real time, so a held request costs the test 300 ms; the prompt bound never trips on the fake clock.
fn reach_opts() -> StatusListOptions {
    StatusListOptions { probe_timeout_ms: Some(300), prompt_ms: Some(5_000), reconcile: Some(Reconcile::OnFailure), ..Default::default() }
}

async fn poll(rig: &Rig, t: &Rc<StatusTracker>) -> WorkspaceStatus {
    rig.fc.advance(POLL_INTERVAL_MS);
    list_one(t, reach_opts()).await
}

fn dark(stub: &HttpStub) {
    stub.delay.set(Delay::Never);
}

fn answers(stub: &HttpStub) {
    stub.delay.set(Delay::Ms(0));
}

#[test]
fn one_silence_keeps_the_word_an_answer_earned_a_second_in_a_row_turns_it_unreachab() {
    run_local(async {
        // rules 7, 10, 11, 14, 16, 18
        let rig = rig();
        let r = roads();
        let daemon = http_stub(426, Delay::Ms(0)).await;
        serve(&r, &daemon);
        rig.put(record(view("steady", "m1", "running"), &r));
        let t = rig.tracker(Default::default());

        assert_eq!(seen(&poll(&rig, &t).await), UP);
        dark(&daemon);
        let first = poll(&rig, &t).await;
        assert_eq!(seen(&first), UP);
        assert_eq!(r.state_asks.get(), 1);
        let second = poll(&rig, &t).await;
        assert_eq!(seen(&second), DARK);
        answers(&daemon);
        let back = poll(&rig, &t).await;
        assert_eq!(seen(&back), UP);
        dark(&daemon);
        assert_eq!(seen(&poll(&rig, &t).await), UP);
        assert_eq!(seen(&poll(&rig, &t).await), DARK);
        assert_eq!(daemon.hits.get(), 6);
    });
}

#[test]
fn the_table_keeps_its_own_run_of_probes_its_reads_never_spend_the_silence_the_side() {
    run_local(async {
        // rules 7, 18
        let rig = rig();
        let (rs, rd) = (roads(), roads());
        let daemon = http_stub(426, Delay::Ms(0)).await;
        let held = http_stub(426, Delay::Never).await;
        serve(&rs, &daemon);
        serve(&rd, &held);
        rig.put(record(view("steady", "m1", "running"), &rs));
        rig.put(record(view("dark", "m2", "running"), &rd));
        let t = rig.tracker(Default::default());
        let app = || async {
            rig.fc.advance(POLL_INTERVAL_MS);
            t.list(reach_opts()).await.unwrap()
        };
        let table = || async { t.list(StatusListOptions { reader: Some(StatusReader::Table), ..reach_opts() }).await.unwrap() };

        assert_eq!(seen(row(&app().await, "steady")), UP);
        assert_eq!(seen(row(&table().await, "steady")), UP);
        dark(&daemon);
        // One blip is not a machine gone dark on this door either: the table's own last read answered, so this one waits.
        assert_eq!(seen(row(&table().await, "steady")), UP);
        // The table's second silence in a row turns the table's word, and only the table's.
        assert_eq!(seen(row(&table().await, "steady")), DARK);
        // A machine no read of this door ever found answering is unreachable at once: what is dampened is a run of
        // probes, not a word a row is owed.
        assert_eq!(seen(row(&table().await, "dark")), DARK);
        // The poller's own next probe is still the first silence after an answer, however many table reads went by.
        assert_eq!(seen(row(&app().await, "steady")), UP);
        assert_eq!(seen(row(&app().await, "steady")), DARK);
        // And back the other way: the poller's answer is not the table's, so the table waits out its own silence again.
        answers(&daemon);
        assert_eq!(seen(row(&app().await, "steady")), UP);
        assert_eq!(seen(row(&table().await, "steady")), UP);
    });
}

#[test]
fn with_no_app_open_nothing_else_probes_and_the_table_is_dampened_by_its_own_last_r() {
    run_local(async {
        // rules 7, 18
        let rig = rig();
        let r = roads();
        let daemon = http_stub(426, Delay::Ms(0)).await;
        serve(&r, &daemon);
        rig.put(record(view("steady", "m1", "running"), &r));
        let t = rig.tracker(Default::default());
        // No watcher and no poll anywhere in this test: every probe of this machine is a table read's own.
        let table = || async {
            rig.fc.advance(POLL_INTERVAL_MS);
            list_one(&t, StatusListOptions { reader: Some(StatusReader::Table), ..reach_opts() }).await
        };

        assert_eq!(seen(&table().await), UP);
        dark(&daemon);
        assert_eq!(seen(&table().await), UP);
        answers(&daemon);
        assert_eq!(seen(&table().await), UP);
        dark(&daemon);
        assert_eq!(seen(&table().await), UP);
        assert_eq!(seen(&table().await), DARK);
    });
}

#[test]
fn a_dead_daemon_port_waits_out_the_same_window_one_502_keeps_running_the_second_re() {
    run_local(async {
        // rules 7, 10, 18
        let rig = rig();
        let r = roads();
        let server = http_stub(426, Delay::Ms(0)).await;
        serve(&r, &server);
        rig.put(record(view("steady", "m1", "running"), &r));
        let t = rig.tracker(Default::default());

        assert_eq!(poll(&rig, &t).await.reach.state, Reachable);
        server.status.set(502);
        let first = poll(&rig, &t).await;
        assert_eq!(seen(&first), UP);
        let second = poll(&rig, &t).await;
        assert_eq!(seen(&second), (Running, NoDaemon));
        server.status.set(426);
        assert_eq!(poll(&rig, &t).await.reach.state, Reachable);
    });
}

#[test]
fn over_the_poll_a_single_silence_pushes_no_status_at_all_and_two_push_unreachable() {
    run_local(async {
        // rules 7, 19, 20
        let rig = rig();
        let r = roads();
        let daemon = http_stub(426, Delay::Ms(0)).await;
        serve(&r, &daemon);
        rig.put(record(view("watched", "m1", "running"), &r));
        let o = StatusWatchOptions { probe_timeout_ms: Some(300), prompt_ms: Some(5_000), cost_interval_ms: Some(24 * 3_600_000), ..Default::default() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        until("the first status", 5_000, || rig.bus.statuses().len() == 1).await;
        assert_eq!(seen(&rig.bus.statuses()[0]), UP);

        dark(&daemon);
        let polled = daemon.hits.get();
        rig.fc.advance(POLL_INTERVAL_MS);
        until("the silent probe lands", 5_000, || daemon.hits.get() > polled).await;
        until("the tick ends", 5_000, || rig.polled.borrow().len() >= 2).await;
        assert_eq!(rig.bus.statuses().len(), 1);

        rig.fc.advance(POLL_INTERVAL_MS);
        until("the second silence is pushed", 5_000, || rig.bus.statuses().len() == 2).await;
        assert_eq!(seen(&rig.bus.statuses()[1]), DARK);

        answers(&daemon);
        rig.fc.advance(POLL_INTERVAL_MS);
        until("the answer is pushed", 5_000, || rig.bus.statuses().len() == 3).await;
        assert_eq!(seen(&rig.bus.statuses()[2]), UP);
        stop();
    });
}

#[test]
fn a_request_that_never_leaves_this_computer_marks_no_row_each_keeps_its_word_with() {
    run_local(async {
        // rules 6, 7, 14
        let rig = rig();
        let (ra, rb) = (roads(), roads());
        let (alpha, beta) = (http_stub(426, Delay::Ms(0)).await, http_stub(426, Delay::Ms(0)).await);
        serve(&ra, &alpha);
        serve(&rb, &beta);
        rig.put(record(view("alpha", "m1", "running"), &ra));
        rig.put(record(view("beta", "m2", "running"), &rb));
        let t = rig.tracker(Default::default());
        let poll_all = || async {
            rig.fc.advance(POLL_INTERVAL_MS);
            t.list(reach_opts()).await.unwrap()
        };
        // The mint fails the way a DNS miss does; the probe's own fetch has no seam to fail from here.
        let road = |up: bool| {
            for (r, stub) in [(&ra, &alpha), (&rb, &beta)] {
                *r.route.borrow_mut() = Some(if up { Ok(route(&stub.url())) } else { Err(offline()) });
            }
        };
        let asks = || ra.state_asks.get() + rb.state_asks.get();

        dark(&beta);
        poll_all().await;
        let mut rows = poll_all().await;
        assert_eq!(seen(row(&rows, "alpha")), UP);
        assert_eq!(seen(row(&rows, "beta")), DARK);
        assert!(rows.iter().all(|s| s.reach.offline != Some(true)));
        let asked = asks();
        let polled = alpha.hits.get() + beta.hits.get();

        road(false);
        for _ in 0..8 {
            rows = poll_all().await;
            assert_eq!((seen(row(&rows, "alpha")), row(&rows, "alpha").reach.offline), (UP, Some(true)));
            assert_eq!((seen(row(&rows, "beta")), row(&rows, "beta").reach.offline), (DARK, Some(true)));
        }
        road(true);
        // Eight polls, two minutes on the clock: no provider call, no exec probe, and nothing reached the daemons.
        assert_eq!(asks(), asked);
        assert_eq!((ra.exec_asks.get(), rb.exec_asks.get()), (0, 0));
        assert_eq!(alpha.hits.get() + beta.hits.get(), polled);

        answers(&beta);
        rows = poll_all().await;
        assert!(rows.iter().all(|s| s.reach.offline != Some(true)));
        assert_eq!(seen(row(&rows, "alpha")), UP);
        assert_eq!(seen(row(&rows, "beta")), UP);

        // One miss, the computer offline, then a miss: two misses in a row for the machine, so the word turns.
        dark(&alpha);
        assert_eq!(seen(row(&poll_all().await, "alpha")), UP);
        road(false);
        let offline_row = poll_all().await;
        assert_eq!((seen(row(&offline_row, "alpha")), row(&offline_row, "alpha").reach.offline), (UP, Some(true)));
        road(true);
        assert_eq!(seen(row(&poll_all().await, "alpha")), DARK);
    });
}

#[test]
fn a_queued_workspace_has_no_machine_to_ask_and_is_never_read_as_gone() {
    run_local(async {
        // rules 3, 4
        // Its status asks the provider nothing, and a machine id of none never reads as a machine gone.
        let rig = rig();
        let r = roads();
        let stub = http_stub(426, Delay::Ms(0)).await;
        serve(&r, &stub);
        rig.put(record(view("held", "", "queued"), &r));
        let t = rig.tracker(Default::default());

        let queued = list_one(&t, StatusListOptions::default()).await;
        assert_eq!((queued.phase, seen(&queued)), (WorkspacePhase::Queued, (Starting, ReachState::Napping)));
        assert_eq!((r.state_asks.get(), r.route_asks.get(), stub.hits.get()), (0, 0, 0));
        assert!(rig.gone.borrow().is_empty());
    });
}

#[test]
fn an_away_workspace_says_why_and_asks_nothing() {
    run_local(async {
        // rules 4
        let rig = rig();
        let (rr, rn) = (roads(), roads());
        let said = "the computer box is not connected";
        let stub = http_stub(426, Delay::Ms(0)).await;
        serve(&rr, &stub);
        let mut running = record(view("far", "m1", "running"), &rr);
        running.away = Some(said.into());
        let mut napping = record(view("asleep", "m2", "napping"), &rn);
        napping.away = Some(said.into());
        rig.put(running);
        rig.put(napping);
        let t = rig.tracker(Default::default());

        let rows = t.list(StatusListOptions::default()).await.unwrap();
        assert_eq!(seen(row(&rows, "far")), DARK);
        assert_eq!(seen(row(&rows, "asleep")), (Paused, ReachState::Napping));
        assert!(rows.iter().all(|s| s.reason.as_deref() == Some(said)));
        assert_eq!((rr.state_asks.get(), rr.route_asks.get(), stub.hits.get(), rn.state_asks.get()), (0, 0, 0, 0));
    });
}

#[test]
fn the_last_release_stops_the_ticks_and_clears_what_was_sent() {
    run_local(async {
        // rules 19
        let rig = rig();
        let r = roads();
        rig.put(record(view("alpha", "m1", "running"), &r));
        let o = StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(POLL_INTERVAL_MS), ..Default::default() };
        let t = rig.tracker(o);
        let (first, second) = (t.watch(o), t.watch(StatusWatchOptions { poll_interval_ms: Some(1), ..Default::default() }));
        settle().await;
        assert!(t.watched());
        assert_eq!(rig.bus.statuses().len(), 1);

        first();
        assert!(t.watched());
        let listed = rig.listed.get();
        rig.fc.advance(POLL_INTERVAL_MS);
        settle().await;
        assert!(rig.listed.get() > listed);

        second();
        second();
        assert!(!t.watched());
        let listed = rig.listed.get();
        rig.fc.advance(4 * POLL_INTERVAL_MS);
        settle().await;
        assert_eq!(rig.listed.get(), listed);

        // The release cleared the last-emitted rows: a new watcher's first poll sends the row again.
        let _again = t.watch(o);
        settle().await;
        assert!(t.watched());
        assert_eq!(rig.bus.statuses().len(), 2);
    });
}

#[test]
fn a_row_built_on_a_record_that_moved_is_dropped() {
    run_local(async {
        // rules 20
        let rig = rig();
        let r = roads();
        *r.route.borrow_mut() = Some(Ok(RouteReach { url: "http://127.0.0.1:1/".into(), expires_at: EXPIRES }));
        let mut rec = record(view("alpha", "m1", "running"), &r);
        answering(&mut rec, &r);
        rig.put(rec);
        let o = StatusWatchOptions { probe_timeout_ms: Some(1_000), cost_interval_ms: Some(3_600_000), ..Default::default() };
        let t = rig.tracker(o);
        let stop = t.watch(o);
        settle().await;

        // The record is written while the poll's probe is in flight.
        rig.records.borrow_mut()[0].generation += 1;
        rig.fc.advance(1_000);
        settle().await;
        assert_eq!(rig.polled.borrow().len(), 1);
        assert!(rig.polled.borrow()[0].is_empty());
        assert!(rig.bus.statuses().is_empty());

        // The next poll builds on the record as it stands now, and its row goes out.
        rig.fc.advance(14_000);
        settle().await;
        rig.fc.advance(1_000);
        settle().await;
        stop();
        assert_eq!(rig.polled.borrow().len(), 2);
        let statuses = rig.bus.statuses();
        assert_eq!(statuses.len(), 1);
        assert_eq!(seen(&statuses[0]), DARK);
    });
}

#[test]
fn poll_reads_only_the_named_workspaces_while_watched() {
    run_local(async {
        // rules 22
        let rig = rig();
        let (ra, rb) = (roads(), roads());
        rig.put(record(view("a", "m1", "running"), &ra));
        rig.put(record(view("b", "m2", "running"), &rb));
        let o = StatusWatchOptions { cost_interval_ms: Some(3_600_000), poll_interval_ms: Some(3_600_000), ..Default::default() };
        let t = rig.tracker(o);

        t.poll(&["a".to_string()]).await;
        assert_eq!(rig.listed.get(), 0);

        let stop = t.watch(o);
        settle().await;
        assert_eq!(rig.bus.statuses().len(), 2);
        let listed = rig.listed.get();
        t.poll(&[]).await;
        assert_eq!(rig.listed.get(), listed);

        rig.set_phase("a", WorkspacePhase::Napping);
        rig.set_phase("b", WorkspacePhase::Napping);
        t.poll(&["a".to_string()]).await;
        stop();
        let statuses = rig.bus.statuses();
        assert_eq!(statuses.len(), 3);
        assert_eq!((statuses[2].id.as_str(), statuses[2].phase, seen(&statuses[2])), ("a", WorkspacePhase::Napping, (Paused, ReachState::Napping)));
    });
}

#[test]
fn a_missing_metrics_read_is_warned_once_per_machine_and_never_turns_the_row() {
    run_local(async {
        // rules 14, 15
        let rig = rig();
        let roads = Rc::new(Roads::default());
        *roads.metrics.borrow_mut() = Some(Err(missing("no metrics for sandbox m1")));
        rig.put(record(view("ws_1", "m1", "running"), &roads));
        let warned = wsp_core::console::capture();
        let t = rig.tracker(Default::default());
        let line = "host metrics for m1 (workspace ws_1) answered 404 no metrics for sandbox m1; the state read says running and the record follows the state read";
        for _ in 0..2 {
            let rows = t.list(StatusListOptions { zombie_probe: Some(false), ..Default::default() }).await.unwrap();
            assert_eq!(rows[0].machine_state, MachineState::Running, "the state read is the word, not the metrics read");
        }
        assert_eq!(roads.metrics_asks.get(), 2);
        assert_eq!(warned.lines(), vec![line.to_string()], "once per spell");
        // An answer clears the doubt, so the next 404 is a new spell and is warned again.
        *roads.metrics.borrow_mut() = Some(Ok(()));
        t.list(StatusListOptions { zombie_probe: Some(false), ..Default::default() }).await.unwrap();
        *roads.metrics.borrow_mut() = Some(Err(missing("no metrics for sandbox m1")));
        t.list(StatusListOptions { zombie_probe: Some(false), ..Default::default() }).await.unwrap();
        assert_eq!(warned.lines(), vec![line.to_string(), line.to_string()]);
        // A failure that is not a 404 says nothing.
        *roads.metrics.borrow_mut() = Some(Ok(()));
        t.list(StatusListOptions { zombie_probe: Some(false), ..Default::default() }).await.unwrap();
        *roads.metrics.borrow_mut() = Some(Err(other("socket hang up")));
        t.list(StatusListOptions { zombie_probe: Some(false), ..Default::default() }).await.unwrap();
        assert_eq!(warned.lines().len(), 2);
    });
}
