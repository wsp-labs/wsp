// SPDX-License-Identifier: AGPL-3.0-only
//! spec/status_meter.md: the awake meter and the cost series of the status tracker, from the "status.watch cost
//! events", "a pause the provider made", "accrued cost", "status.history", "spend today" and "gone machines and the
//! meter" cases of packages/runtime/test/status.test.ts, driven at the tracker with the lifecycle events the runtime
//! would emit.
mod support;

use std::cell::Cell;
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::{json, Value};
use support::status::*;
use support::{run_local, settle, MemStore};
use wsp_core::clock::FakeClock;
use wsp_core::console;
use wsp_core::status::{StatusListOptions, StatusRecord, StatusTracker, StatusTrackerOptions, StatusWatchOptions, COST_HISTORIES};
use wsp_core::types::pending::{BlobMark, Store};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::place_more::{PlaceSpend, PlaceView};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::views::workspace_events::WorkspaceCostEvent;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_words::exit::HostError;

const TICK_MS: i64 = 10_000;
const HOUR: i64 = 3_600_000;

fn usd(rate: f64, ms: f64) -> f64 {
    rate * ms / 3_600_000.0
}

fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} is not {b}");
}

fn n(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or_else(|| panic!("{key} is not a number in {v}"))
}

fn ms_of(iso: &str) -> i64 {
    iso.parse::<jiff::Timestamp>().unwrap().as_millisecond()
}

/// The ISO stamp `off` ms after `T0`, for offsets inside the same day.
fn at(off: i64) -> String {
    let s = off / 1000;
    format!("2026-10-25T{:02}:{:02}:{:02}.{:03}Z", 10 + s / 3600, s / 60 % 60, s % 60, off % 1000)
}

fn ticking() -> StatusWatchOptions {
    StatusWatchOptions { cost_interval_ms: Some(TICK_MS), poll_interval_ms: Some(24 * HOUR), ..Default::default() }
}

fn every(ms: i64) -> StatusWatchOptions {
    StatusWatchOptions { cost_interval_ms: Some(ms), poll_interval_ms: Some(24 * HOUR), ..Default::default() }
}

fn assert_cost(c: &Value, phase: &str, rate: f64, awake_ms: i64) {
    assert_eq!(c["phase"], phase);
    assert_eq!(n(c, "rateUsdPerHour"), rate);
    assert_eq!(n(c, "awakeMs"), awake_ms as f64);
}

/// The cost event with the newest `at`, emitted or passed; an emitted one wins a tie.
fn newest(r: &Rig) -> Value {
    let mut all = r.bus.passed_costs();
    all.extend(r.bus.costs());
    all.into_iter().max_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str())).expect("a cost event")
}

/// Each key of the stored point is the same on the event: the TS `toMatchObject`.
fn assert_matches(event: &Value, point: &WorkspaceCostEvent) {
    let point = serde_json::to_value(point).unwrap();
    for (key, want) in point.as_object().unwrap() {
        assert_eq!(&event[key.as_str()], want, "{key}");
    }
}

fn awakes(h: &[WorkspaceCostEvent]) -> Vec<f64> {
    h.iter().map(|p| p.awake_ms.0).collect()
}

fn rates_and_awakes(h: &[WorkspaceCostEvent]) -> Vec<(f64, f64)> {
    h.iter().map(|p| (p.rate_usd_per_hour.0, p.awake_ms.0)).collect()
}

fn put(r: &Rig, roads: &Rc<Roads>, id: &str, phase: &str, rate: f64) -> WorkspaceView {
    let v = view(id, "m_1", phase);
    let mut rec = record(v.clone(), roads);
    rec.rate_usd_per_hour = rate;
    r.put(rec);
    v
}

fn start(r: &Rig, roads: &Rc<Roads>, id: &str, rate: f64) -> WorkspaceView {
    put(r, roads, id, "running", rate)
}

fn reprice(r: &Rig, id: &str, rate: f64) {
    r.records.borrow_mut().iter_mut().find(|x| x.view.id == id).unwrap().rate_usd_per_hour = rate;
}

async fn advance(r: &Rig, ms: i64) {
    r.fc.advance(ms);
    settle().await;
}

async fn create(r: &Rig, v: &WorkspaceView) {
    r.bus.fire(&created(v));
    settle().await;
}

async fn nap(r: &Rig, id: &str) {
    r.set_phase(id, WorkspacePhase::Napping);
    r.bus.fire(&napped(id, false));
    settle().await;
}

async fn wake(r: &Rig, id: &str) {
    r.set_phase(id, WorkspacePhase::Running);
    r.bus.fire(&woken(id, "m_1"));
    settle().await;
}

async fn go_gone(r: &Rig, id: &str) {
    r.set_phase(id, WorkspacePhase::Gone);
    r.bus.fire(&gone(id, "m_1"));
    settle().await;
}

/// A second host over the first's store, its clock at `at`.
fn restart(prev: &Rig, at: i64) -> Rig {
    let mut r = rig();
    r.store = prev.store.clone();
    r.fc = FakeClock::new(at);
    r
}

fn places() -> Vec<PlaceView> {
    serde_json::from_value(json!([
        {"id": "here", "kind": "computer", "name": "this-mac", "default": true},
        {"id": "box", "kind": "provider", "name": "box", "default": false, "takesForks": true, "cap": {"machines": 3, "spendPerDayUsd": 10}},
        {"id": "default", "kind": "provider", "name": "default", "default": false, "takesForks": true, "cap": {"machines": 3, "spendPerDayUsd": 10}}
    ]))
    .unwrap()
}

fn zeros(place: &str) -> PlaceSpend {
    PlaceSpend { place: place.into(), today_usd: JsNumber(0.0), month_usd: JsNumber(0.0), rate_usd_per_hour: JsNumber(0.0) }
}

async fn spend_on(t: &StatusTracker, place: &str, when: Option<i64>) -> PlaceSpend {
    t.spend(&places(), when).await.unwrap().into_iter().find(|x| x.place == place).unwrap()
}

fn stored(r: &Rig, id: &str) -> Value {
    serde_json::from_str(&r.store.text(COST_HISTORIES, id).expect("a stored document")).unwrap()
}

/// A store whose puts wait until the test lets one through.
struct GateStore {
    inner: Rc<MemStore>,
    allowed: Cell<u32>,
}

#[async_trait(?Send)]
impl Store for GateStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.inner.get(collection, id).await
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        while self.allowed.get() == 0 {
            tokio::task::yield_now().await;
        }
        self.allowed.set(self.allowed.get() - 1);
        self.inner.put(collection, id, value).await
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        self.inner.list(collection).await
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        self.inner.keys(collection).await
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete(collection, id).await
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.inner.get_blob(collection, id).await
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        self.inner.put_blob(collection, id, bytes).await
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete_blob(collection, id).await
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        self.inner.stat_blob(collection, id).await
    }
}

/// The rig's tracker over another store.
fn tracker_over(r: &Rig, store: Rc<dyn Store>) -> Rc<StatusTracker> {
    let records = r.records.clone();
    let (b1, b2, b3) = (r.bus.clone(), r.bus.clone(), r.bus.clone());
    StatusTracker::new(StatusTrackerOptions {
        records: Rc::new(move || {
            let answer: Result<Vec<StatusRecord>, String> = Ok(records.borrow().clone());
            Box::pin(async move { answer })
        }),
        store,
        emit: Rc::new(move |e| {
            b1.emitted.borrow_mut().push(e.clone());
            b1.fire(&e);
        }),
        pass: Rc::new(move |e| b2.passed.borrow_mut().push(e)),
        on: Rc::new(move |t, l| {
            b3.listeners.borrow_mut().push((t, l));
            Rc::new(|| {})
        }),
        defaults: Default::default(),
        on_polled: None,
        on_gone: None,
        clock: Some(r.fc.clock()),
        zone: Some(jiff::tz::TimeZone::UTC),
    })
}

// ---- status.watch cost events ----

#[test]
fn emits_workspace_cost_on_a_timer_with_awake_time_accrual_zero_rate_while_napping() {
    // Rules 2, 4, 10, 11.
    run_local(async {
        let r = rig();
        let roads = Rc::new(Roads::default());
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        let release = t.watch(every(15));
        settle().await;
        advance(&r, 15).await;

        let costs = r.bus.costs();
        assert_eq!(costs.len(), 2);
        assert_eq!(costs[0]["workspaceId"], "ws_1");
        assert_cost(&costs[0], "running", 0.5, 0);
        assert_eq!(n(&costs[0], "accruedUsd"), 0.0);
        assert_eq!(costs[0]["at"], at(0));
        let running = &costs[1];
        assert_eq!(running["workspaceId"], "ws_1");
        assert_cost(running, "running", 0.5, 15);
        assert_close(n(running, "accruedUsd"), 0.5 * 15.0 / 3_600_000.0);
        assert!(n(running, "accruedUsd") > 0.0);
        assert_eq!(running["at"], at(15));

        nap(&r, "ws_1").await;
        let costs = r.bus.costs();
        assert_eq!(costs.len(), 3);
        let napping = costs[2].clone();
        assert_cost(&napping, "napping", 0.0, 15);
        assert_close(n(&napping, "accruedUsd"), 0.5 * 15.0 / 3_600_000.0);

        // Napping accrues nothing: the repeats carry the same total and stay out of the replay list.
        advance(&r, 15).await;
        advance(&r, 15).await;
        assert_eq!(r.bus.costs().len(), 3);
        let passed = r.bus.passed_costs();
        assert_eq!(passed.len(), 2);
        for (p, off) in passed.iter().zip([30, 45]) {
            assert_cost(p, "napping", 0.0, 15);
            assert_close(n(p, "accruedUsd"), n(&napping, "accruedUsd"));
            assert_eq!(p["at"], at(off));
        }

        // Unwatched means no timers running.
        release();
        let (emitted, passed) = (r.bus.costs(), r.bus.passed_costs());
        advance(&r, 40).await;
        assert_eq!(r.bus.costs(), emitted);
        assert_eq!(r.bus.passed_costs(), passed);
    });
}

#[test]
fn keeps_the_total_and_keeps_ticking_through_a_pause_the_provider_takes_100_ms_over() {
    // Rules 2, 4, 10, 11.
    run_local(async {
        let r = rig();
        let roads = Rc::new(Roads::default());
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        let release = t.watch(every(15));
        settle().await;
        advance(&r, 15).await;
        advance(&r, 15).await;
        r.set_phase("ws_1", WorkspacePhase::Pausing);
        for _ in 0..6 {
            advance(&r, 15).await;
        }
        nap(&r, "ws_1").await;
        release();

        let costs = r.bus.costs();
        let pausing: Vec<&Value> = costs.iter().filter(|c| c["phase"] == "pausing").collect();
        assert_eq!(pausing.len(), 6);
        for (i, c) in pausing.iter().enumerate() {
            let awake = 15 * (i as i64 + 3);
            assert_cost(c, "pausing", 0.5, awake);
            assert_close(n(c, "accruedUsd"), usd(0.5, awake as f64));
        }
        let totals: Vec<f64> = costs.iter().map(|c| n(c, "accruedUsd")).collect();
        assert!(totals.windows(2).all(|w| w[0] <= w[1]), "{totals:?}");
        let last_running = costs.iter().rev().find(|c| c["phase"] == "running").unwrap();
        assert_close(n(last_running, "accruedUsd"), usd(0.5, 30.0));
        let last = costs.last().unwrap();
        assert_cost(last, "napping", 0.0, 120);
        assert_close(n(last, "accruedUsd"), usd(0.5, 120.0));
        assert!(n(last, "accruedUsd") > n(last_running, "accruedUsd"));
    });
}

#[test]
fn sends_a_folder_nobody_bills_once_and_none_of_the_timers_ticks_after() {
    // Rule 11.
    run_local(async {
        let r = rig();
        let roads = Rc::new(Roads::default());
        let v = start(&r, &roads, "folder", 0.0);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        let release = t.watch(ticking());
        settle().await;
        for _ in 0..8 {
            advance(&r, TICK_MS).await;
        }
        release();
        let costs = r.bus.costs();
        assert_eq!(costs.len(), 1);
        assert_eq!(costs[0]["workspaceId"], "folder");
        assert_eq!(costs[0]["phase"], "running");
        assert_eq!(n(&costs[0], "rateUsdPerHour"), 0.0);
        assert_eq!(n(&costs[0], "accruedUsd"), 0.0);
        assert!(r.bus.passed_costs().is_empty());
    });
}

// ---- a pause the provider made ----

#[test]
fn a_machine_paused_while_the_host_was_down_bills_nothing_for_the_gap() {
    // Rules 4, 7, 10.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r1 = rig();
        let v = start(&r1, &roads, "ws_1", 0.5);
        let t1 = r1.tracker(Default::default());
        create(&r1, &v).await;
        let release = t1.watch(ticking());
        settle().await;
        advance(&r1, TICK_MS).await;
        release();
        assert_cost(&newest(&r1), "running", 0.5, TICK_MS);

        // An hour down, and the provider paused the machine meanwhile: the next host reads the record napping.
        let r2 = restart(&r1, r1.fc.now() + HOUR);
        put(&r2, &roads, "ws_1", "napping", 0.5);
        let t2 = r2.tracker(Default::default());
        let release = t2.watch(ticking());
        settle().await;
        advance(&r2, TICK_MS).await;
        advance(&r2, TICK_MS).await;
        // The hour down bills nothing: the stretch ended at the last tick that proved the machine awake.
        let (emitted, passed) = (r2.bus.costs(), r2.bus.passed_costs());
        assert_eq!(emitted.len(), 1);
        assert_eq!(passed.len(), 1);
        for c in emitted.iter().chain(passed.iter()) {
            assert_cost(c, "napping", 0.0, TICK_MS);
            assert_close(n(c, "accruedUsd"), usd(0.5, TICK_MS as f64));
        }

        // The wake resumes the machine the provider paused, and the stretch starts there, not an hour ago.
        wake(&r2, "ws_1").await;
        assert_cost(&newest(&r2), "running", 0.5, TICK_MS);
        advance(&r2, TICK_MS).await;
        release();
        assert_cost(&newest(&r2), "running", 0.5, 2 * TICK_MS);
    });
}

#[test]
fn a_pause_found_after_a_gap_ends_the_awake_stretch_at_the_last_proof() {
    // Rule 5: a pause found ends at awake_until, the last instant the provider or the daemon answered awake.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        let release = t.watch(ticking());
        // The poll a watch runs at once has finished, so its proof of an awake machine lands before the clock moves.
        settle().await;
        advance(&r, TICK_MS).await;
        assert_cost(&newest(&r), "running", 0.5, TICK_MS);
        // The last proof the machine was awake: the provider says running at this instant.
        t.list(StatusListOptions::default()).await.unwrap();
        release();

        // An hour with nothing watching: no tick and no poll runs, and the provider pauses the machine within it.
        advance(&r, HOUR).await;
        r.set_phase("ws_1", WorkspacePhase::Napping);
        r.bus.fire(&napped("ws_1", true));
        settle().await;
        let found = newest(&r);
        assert_cost(&found, "napping", 0.0, TICK_MS);
        assert_close(n(&found, "accruedUsd"), usd(0.5, TICK_MS as f64));

        let release = t.watch(ticking());
        settle().await;
        advance(&r, TICK_MS).await;
        release();
        assert_cost(&newest(&r), "napping", 0.0, TICK_MS);
        let history = t.history("ws_1").await;
        let last = history.last().unwrap();
        assert_eq!(last.phase, WorkspacePhase::Napping);
        assert_eq!(last.awake_ms.0, TICK_MS as f64);
    });
}

#[test]
fn a_pause_this_host_made_ends_the_stretch_now() {
    // Rule 5: a nap that was not found ends at the clock, not at the last proof.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        advance(&r, HOUR).await;
        nap(&r, "ws_1").await;
        let c = newest(&r);
        assert_cost(&c, "napping", 0.0, HOUR);
        assert_close(n(&c, "accruedUsd"), usd(0.5, HOUR as f64));
        assert_eq!(c["at"], at(HOUR));
        let last = t.history("ws_1").await.pop().unwrap();
        assert_eq!(last.awake_ms.0, HOUR as f64);
    });
}

#[test]
fn a_gone_verdict_unmade_by_a_wake_reopens_the_stretch_it_closed() {
    // Rules 4, 5: a record that leaves gone was never paused, so the gap belongs to the stretch.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        advance(&r, TICK_MS).await;
        t.list(StatusListOptions::default()).await.unwrap();
        advance(&r, TICK_MS).await;
        go_gone(&r, "ws_1").await;
        let verdict = newest(&r);
        assert_cost(&verdict, "gone", 0.0, TICK_MS);
        assert_close(n(&verdict, "accruedUsd"), usd(0.5, TICK_MS as f64));

        advance(&r, 2 * TICK_MS).await;
        wake(&r, "ws_1").await;
        let reopened = newest(&r);
        assert_cost(&reopened, "running", 0.5, 4 * TICK_MS);
        assert_close(n(&reopened, "accruedUsd"), usd(0.5, TICK_MS as f64));
        assert_eq!(reopened["at"], at(4 * TICK_MS));
    });
}

#[test]
fn a_rebuild_after_gone_starts_a_fresh_stretch() {
    // Rules 4, 5: an upgrade makes the closed stretch the old machine's, so the wake after it opens a new one.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        advance(&r, TICK_MS).await;
        t.list(StatusListOptions::default()).await.unwrap();
        advance(&r, TICK_MS).await;
        go_gone(&r, "ws_1").await;
        assert_cost(&newest(&r), "gone", 0.0, TICK_MS);

        advance(&r, 10 * TICK_MS).await;
        r.bus.fire(&upgraded("ws_1"));
        settle().await;
        wake(&r, "ws_1").await;
        let fresh = newest(&r);
        assert_cost(&fresh, "running", 0.5, TICK_MS);
        assert_close(n(&fresh, "accruedUsd"), usd(0.5, TICK_MS as f64));
        assert_eq!(fresh["at"], at(12 * TICK_MS));
    });
}

// ---- accrued cost ----

#[test]
fn is_each_awake_stretch_billed_at_the_rate_that_held_over_it() {
    // Rules 9, 10: a pause, an unwatched wake, a restart.
    run_local(async {
        let small = 0.11;
        let roads = Rc::new(Roads::default());
        let r1 = rig();
        let v = start(&r1, &roads, "ws_1", small);
        let t1 = r1.tracker(Default::default());
        create(&r1, &v).await;
        let release = t1.watch(ticking());
        settle().await;
        advance(&r1, TICK_MS).await;
        advance(&r1, TICK_MS).await;
        let c = newest(&r1);
        assert_cost(&c, "running", small, 2 * TICK_MS);
        assert_close(n(&c, "accruedUsd"), usd(small, 2.0 * TICK_MS as f64));

        // A pause adds nothing, tick after tick.
        nap(&r1, "ws_1").await;
        advance(&r1, TICK_MS).await;
        advance(&r1, TICK_MS).await;
        let c = newest(&r1);
        assert_cost(&c, "napping", 0.0, 2 * TICK_MS);
        assert_close(n(&c, "accruedUsd"), usd(small, 2.0 * TICK_MS as f64));
        release();

        // Nobody watching. The wake lands its own tick, then an hour passes with nothing looking: the meter reads wall
        // time, so the first tick after it carries that hour rather than only the ticks that ran.
        wake(&r1, "ws_1").await;
        assert_cost(&newest(&r1), "running", small, 2 * TICK_MS);
        advance(&r1, HOUR).await;

        let release = t1.watch(ticking());
        settle().await;
        advance(&r1, TICK_MS).await;
        advance(&r1, TICK_MS).await;
        release();
        let awake = 4 * TICK_MS + HOUR;
        let billed = usd(small, awake as f64);
        let c = newest(&r1);
        assert_cost(&c, "running", small, awake);
        assert_close(n(&c, "accruedUsd"), billed);

        // An hour down with the machine running: the next host meters on from the stored total at the stored rate.
        let r2 = restart(&r1, r1.fc.now() + HOUR);
        put(&r2, &roads, "ws_1", "running", small);
        let t2 = r2.tracker(Default::default());
        let release = t2.watch(ticking());
        settle().await;
        advance(&r2, TICK_MS).await;
        release();
        let c = newest(&r2);
        assert_cost(&c, "running", small, awake + HOUR + TICK_MS);
        assert_close(n(&c, "accruedUsd"), billed + usd(small, (HOUR + TICK_MS) as f64));
        let last = t2.history("ws_1").await.pop().unwrap();
        assert_close(last.accrued_usd.0, n(&c, "accruedUsd"));
    });
}

#[test]
fn bills_a_pause_until_the_provider_has_it_and_the_history_and_spend_read_its_total() {
    // Rules 10, 16.
    run_local(async {
        let rate = 0.11;
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", rate);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        let release = t.watch(ticking());
        settle().await;
        advance(&r, TICK_MS).await;
        advance(&r, TICK_MS).await;
        r.set_phase("ws_1", WorkspacePhase::Pausing);
        advance(&r, TICK_MS).await;
        let c = newest(&r);
        assert_cost(&c, "pausing", rate, 3 * TICK_MS);
        assert_close(n(&c, "accruedUsd"), usd(rate, 3.0 * TICK_MS as f64));
        advance(&r, TICK_MS / 2).await;
        let mid_pause = spend_on(&t, "default", None).await;
        assert_eq!(mid_pause.rate_usd_per_hour.0, rate);
        assert_close(mid_pause.month_usd.0, usd(rate, 3.5 * TICK_MS as f64));

        nap(&r, "ws_1").await;
        release();
        let billed = usd(rate, 3.5 * TICK_MS as f64);
        let last = t.history("ws_1").await.pop().unwrap();
        assert_eq!(last.rate_usd_per_hour.0, 0.0);
        assert_eq!(last.awake_ms.0, 3.5 * TICK_MS as f64);
        assert_close(last.accrued_usd.0, billed);
        let c = newest(&r);
        assert_eq!(c["phase"], "napping");
        assert_eq!(n(&c, "rateUsdPerHour"), 0.0);
        assert_close(n(&c, "accruedUsd"), billed);
        let spent = spend_on(&t, "default", None).await;
        assert_close(spent.today_usd.0, billed);
        assert_close(spent.month_usd.0, billed);
        assert_eq!(spent.rate_usd_per_hour.0, 0.0);
    });
}

#[test]
fn a_rebuild_at_an_unchanged_rate_lands_no_cost_event() {
    // Rule 11: an event's tick rides the bus only when it changed the series' rate.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        let first = t.history("ws_1").await;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].phase, WorkspacePhase::Running);
        assert_eq!(first[0].awake_ms.0, 0.0);
        assert_eq!(first[0].accrued_usd.0, 0.0);
        let release = t.watch(ticking());
        settle().await;
        advance(&r, TICK_MS).await;
        advance(&r, TICK_MS).await;
        let before = r.bus.costs();
        assert_eq!(before.len(), 3);

        r.bus.fire(&upgraded("ws_1"));
        settle().await;
        advance(&r, TICK_MS).await;
        release();
        // The timer's tick is the only one that landed: the rebuild's folded into the run and reported nothing.
        let costs = r.bus.costs();
        assert_eq!(costs.len(), before.len() + 1);
        assert_eq!(&costs[..before.len()], &before[..]);
        assert_cost(costs.last().unwrap(), "running", 0.5, 3 * TICK_MS);
        assert_close(n(costs.last().unwrap(), "accruedUsd"), usd(0.5, 3.0 * TICK_MS as f64));
        assert!(r.bus.passed_costs().is_empty());
        let history = t.history("ws_1").await;
        assert_eq!(awakes(&history), vec![0.0, 3.0 * TICK_MS as f64]);
    });
}

#[test]
fn an_event_tick_at_a_changed_rate_is_emitted_at_once() {
    // Rule 11: a size change an hour after an unwatched wake still bills that hour at the size it woke at.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        assert_eq!(r.bus.costs().len(), 1);
        advance(&r, HOUR).await;
        reprice(&r, "ws_1", 1.0);
        r.bus.fire(&upgraded("ws_1"));
        settle().await;

        let costs = r.bus.costs();
        assert_eq!(costs.len(), 2);
        assert_eq!(costs[1]["workspaceId"], "ws_1");
        assert_cost(&costs[1], "running", 1.0, HOUR);
        assert_close(n(&costs[1], "accruedUsd"), 0.5 * HOUR as f64 / 3_600_000.0);
        assert_eq!(costs[1]["at"], at(HOUR));
        assert!(r.bus.passed_costs().is_empty());
        let history = t.history("ws_1").await;
        assert_eq!(rates_and_awakes(&history), vec![(0.5, 0.0), (1.0, HOUR as f64)]);
    });
}

// ---- gone machines and the meter ----

#[test]
fn a_gone_record_drops_its_mark_without_folding_and_a_rebuild_does_not_bill_the_downtime() {
    // Rule 10.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r1 = rig();
        let v = start(&r1, &roads, "ws_1", 0.5);
        let t1 = r1.tracker(Default::default());
        create(&r1, &v).await;
        let release = t1.watch(ticking());
        settle().await;
        advance(&r1, TICK_MS).await;
        release();
        assert_cost(&newest(&r1), "running", 0.5, TICK_MS);

        // An hour down, and the machine deleted at the provider meanwhile: the next host finds it gone.
        let r2 = restart(&r1, r1.fc.now() + HOUR);
        put(&r2, &roads, "ws_1", "gone", 0.5);
        let t2 = r2.tracker(Default::default());
        let release = t2.watch(ticking());
        settle().await;
        advance(&r2, TICK_MS).await;
        advance(&r2, TICK_MS).await;
        let (emitted, passed) = (r2.bus.costs(), r2.bus.passed_costs());
        assert_eq!((emitted.len(), passed.len()), (1, 1));
        for c in emitted.iter().chain(passed.iter()) {
            assert_cost(c, "gone", 0.0, TICK_MS);
            assert_close(n(c, "accruedUsd"), usd(0.5, TICK_MS as f64));
        }

        // The rebuild starts the stretch with a tick at that instant: the hour the host was down and the machine did not
        // exist is not on the bill.
        r2.set_phase("ws_1", WorkspacePhase::Running);
        r2.bus.fire(&upgraded("ws_1"));
        settle().await;
        assert_cost(&newest(&r2), "running", 0.5, TICK_MS);
        advance(&r2, TICK_MS).await;
        release();
        assert_cost(&newest(&r2), "running", 0.5, 2 * TICK_MS);
    });
}

// ---- status.history ----

#[test]
fn holds_the_folded_ticks_since_metering_began_and_hands_them_out() {
    // Rules 10, 15.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        // The create opens the stretch with a tick of its own, before anything watches.
        create(&r, &v).await;
        let first = t.history("ws_1").await;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].workspace_id, "ws_1");
        assert_eq!(first[0].phase, WorkspacePhase::Running);
        assert_eq!(first[0].awake_ms.0, 0.0);
        assert_eq!(first[0].accrued_usd.0, 0.0);
        let release = t.watch(ticking());
        settle().await;
        for _ in 0..4 {
            advance(&r, TICK_MS).await;
        }
        // One rate the whole way: the create's tick and the newest, nothing between.
        let history = t.history("ws_1").await;
        assert_eq!(rates_and_awakes(&history), vec![(0.5, 0.0), (0.5, 4.0 * TICK_MS as f64)]);
        let emitted = r.bus.costs();
        assert_matches(&emitted[4], &history[1]);

        nap(&r, "ws_1").await;
        for _ in 0..3 {
            advance(&r, TICK_MS).await;
        }
        release();
        let history = t.history("ws_1").await;
        // The last running tick and the first napping one bracket the change; the napping run folds to its newest.
        let shape: Vec<(WorkspacePhase, f64, f64)> = history.iter().map(|p| (p.phase, p.rate_usd_per_hour.0, p.awake_ms.0)).collect();
        let four = 4.0 * TICK_MS as f64;
        assert_eq!(
            shape,
            vec![(WorkspacePhase::Running, 0.5, 0.0), (WorkspacePhase::Running, 0.5, four), (WorkspacePhase::Napping, 0.0, four), (WorkspacePhase::Napping, 0.0, four)]
        );
        assert_matches(&r.bus.costs()[5], &history[2]);
        assert_matches(r.bus.passed_costs().last().unwrap(), &history[3]);
        assert!(t.history("ws_nobody").await.is_empty());
    });
}

#[test]
fn survives_a_host_restart_the_series_reloads_and_meters_on() {
    // Rules 7, 8, 14.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r1 = rig();
        let v = start(&r1, &roads, "ws_1", 0.5);
        let t1 = r1.tracker(Default::default());
        create(&r1, &v).await;
        let release = t1.watch(ticking());
        settle().await;
        for _ in 0..3 {
            advance(&r1, TICK_MS).await;
        }
        release();
        let first = t1.history("ws_1").await;
        assert_eq!(awakes(&first), vec![0.0, 3.0 * TICK_MS as f64]);
        // The create's tick and the first timer tick add points; ticks of one rate then replace the newest in memory and
        // reach the store only when a point is added.
        assert_eq!(r1.store.puts.get(), 2);

        // An hour down, the machine still running and billing the whole time. The store's newest point is the tick that
        // added it, so the series it hands out before its first tick ends where the rate run began.
        let r2 = restart(&r1, r1.fc.now() + HOUR);
        put(&r2, &roads, "ws_1", "running", 0.5);
        let t2 = r2.tracker(Default::default());
        let handed = t2.history("ws_1").await;
        assert_eq!(awakes(&handed), vec![0.0, TICK_MS as f64]);
        assert_eq!(handed[0], first[0]);
        assert_eq!(handed[1].workspace_id, "ws_1");
        assert_eq!(handed[1].phase, WorkspacePhase::Running);
        assert_eq!(handed[1].rate_usd_per_hour, first[1].rate_usd_per_hour);

        let release = t2.watch(ticking());
        settle().await;
        advance(&r2, TICK_MS).await;
        // The meter runs on from the stored point through the hour down, as the provider billed it.
        let tick = r2.bus.costs()[0].clone();
        assert_cost(&tick, "running", 0.5, 4 * TICK_MS + HOUR);
        assert_close(n(&tick, "accruedUsd"), first[1].accrued_usd.0 + first[1].rate_usd_per_hour.0 * (TICK_MS + HOUR) as f64 / 3_600_000.0);
        // The run continues at one rate: the stored first tick stays, the newest moves, no point was added.
        let history = t2.history("ws_1").await;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], first[0]);
        assert_eq!(history[1].awake_ms.0, n(&tick, "awakeMs"));
        assert_eq!(history[1].accrued_usd.0, n(&tick, "accruedUsd"));
        assert_eq!(r2.store.puts.get(), 2);

        // A nap after the restart is a rate change: it adds two points and both reach the store.
        nap(&r2, "ws_1").await;
        for _ in 0..3 {
            advance(&r2, TICK_MS).await;
        }
        release();
        let awake = n(&tick, "awakeMs");
        assert_eq!(rates_and_awakes(&t2.history("ws_1").await), vec![(0.5, 0.0), (0.5, awake), (0.0, awake), (0.0, awake)]);
        assert_eq!(r2.store.puts.get(), 4);
        let r3 = restart(&r2, r2.fc.now());
        let t3 = r3.tracker(Default::default());
        let restored = t3.history("ws_1").await;
        assert_eq!(restored.len(), 4);
        assert_eq!(restored.last().unwrap().rate_usd_per_hour.0, 0.0);
        assert_eq!(restored.last().unwrap().awake_ms.0, awake);
    });
}

#[test]
fn a_workspace_napping_across_the_restart_keeps_its_total_and_accrues_nothing() {
    // Rules 7, 8, 14: until it wakes.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r1 = rig();
        let v = start(&r1, &roads, "ws_1", 0.5);
        let t1 = r1.tracker(Default::default());
        create(&r1, &v).await;
        let release = t1.watch(ticking());
        settle().await;
        for _ in 0..3 {
            advance(&r1, TICK_MS).await;
        }
        nap(&r1, "ws_1").await;
        advance(&r1, TICK_MS).await;
        release();
        let total = 3 * TICK_MS;
        assert_cost(&newest(&r1), "napping", 0.0, total);
        // The create's tick, the last running one, the nap's own and the newest.
        assert_eq!(t1.history("ws_1").await.len(), 4);

        let r2 = restart(&r1, r1.fc.now() + HOUR);
        put(&r2, &roads, "ws_1", "napping", 0.5);
        let t2 = r2.tracker(Default::default());
        let release = t2.watch(ticking());
        settle().await;
        advance(&r2, TICK_MS).await;
        let c = newest(&r2);
        assert_cost(&c, "napping", 0.0, total);
        assert_close(n(&c, "accruedUsd"), usd(0.5, total as f64));
        wake(&r2, "ws_1").await;
        advance(&r2, TICK_MS).await;
        release();
        assert_cost(&newest(&r2), "running", 0.5, total + TICK_MS);
    });
}

#[test]
fn a_host_that_died_between_a_nap_and_its_tick_does_not_bill_the_nap() {
    // Rules 7, 8, 14: the wake starts the stretch afresh.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r1 = rig();
        let v = start(&r1, &roads, "ws_1", 0.5);
        let t1 = r1.tracker(Default::default());
        create(&r1, &v).await;
        let release = t1.watch(ticking());
        settle().await;
        advance(&r1, TICK_MS).await;
        advance(&r1, TICK_MS).await;
        release();
        // The store's newest point still says running when the nap lands and the host dies before its tick is written.
        r1.store.fail_put.set(true);
        nap(&r1, "ws_1").await;
        assert_cost(&newest(&r1), "napping", 0.0, 2 * TICK_MS);

        let r2 = restart(&r1, r1.fc.now() + HOUR);
        r1.store.fail_put.set(false);
        put(&r2, &roads, "ws_1", "napping", 0.5);
        let t2 = r2.tracker(Default::default());
        // The store holds the points that were added: the create's tick and the first timer tick; the second only moved
        // the newest in memory.
        let last = t2.history("ws_1").await.pop().unwrap();
        assert_eq!(last.phase, WorkspacePhase::Running);
        assert_eq!(last.awake_ms.0, TICK_MS as f64);
        // The record says napping, so the stored running tick is not carried on through the hour the host was down.
        assert_close(spend_on(&t2, "default", None).await.month_usd.0, 0.5 * TICK_MS as f64 / 3_600_000.0);
        let release = t2.watch(ticking());
        settle().await;
        advance(&r2, TICK_MS).await;
        // The hour between the nap and the restart is not on the bill: the stretch ended at the newest stored tick.
        assert_cost(&newest(&r2), "napping", 0.0, TICK_MS);
        wake(&r2, "ws_1").await;
        advance(&r2, TICK_MS).await;
        release();
        assert_cost(&newest(&r2), "running", 0.5, 2 * TICK_MS);
    });
}

#[test]
fn hands_a_deleted_workspaces_chart_to_nobody_and_keeps_what_it_spent_on_the_place() {
    // Rules 6, 13, 14, 15.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r1 = rig();
        let v = start(&r1, &roads, "ws_1", 0.5);
        let t1 = r1.tracker(Default::default());
        create(&r1, &v).await;
        let release = t1.watch(ticking());
        settle().await;
        advance(&r1, TICK_MS).await;
        advance(&r1, TICK_MS).await;
        let spent = spend_on(&t1, "default", None).await.month_usd.0;
        assert_close(spent, usd(0.5, 2.0 * TICK_MS as f64));
        assert!(spent > 0.0);
        r1.bus.fire(&deleted("ws_1"));
        r1.records.borrow_mut().clear();
        settle().await;
        release();
        // The pane is gone with the workspace and there is no record left to read a caller's right to the series off.
        assert!(t1.history("ws_1").await.is_empty());
        // The month keeps it up to the delete, and nothing is burning now: the delete ended the stretch wherever it stood.
        let kept = spend_on(&t1, "default", None).await;
        assert!(kept.month_usd.0 >= spent);
        assert_close(kept.month_usd.0, usd(0.5, 2.0 * TICK_MS as f64));
        assert_eq!(kept.rate_usd_per_hour.0, 0.0);
        assert_eq!(r1.store.deletes.get(), 0);
        let doc = stored(&r1, "ws_1");
        assert_eq!(doc["ended"], true);
        assert_eq!(doc["points"].as_array().unwrap().last().unwrap()["phase"], "gone");

        let r2 = restart(&r1, r1.fc.now());
        let t2 = r2.tracker(Default::default());
        assert!(t2.history("ws_1").await.is_empty());
        assert_eq!(t2.spend(&places(), None).await.unwrap(), vec![zeros("box"), kept]);
    });
}

#[test]
fn a_deleted_workspace_with_no_series_has_its_document_deleted() {
    // Rule 14: nothing was ever metered, so there is nothing to keep for the month.
    run_local(async {
        let r = rig();
        r.store.seed(COST_HISTORIES, "ws_empty", json!({"workspaceId": "ws_empty", "points": []}));
        let t = r.tracker(Default::default());
        assert!(t.history("ws_empty").await.is_empty());
        r.bus.fire(&deleted("ws_empty"));
        settle().await;
        assert!(r.store.text(COST_HISTORIES, "ws_empty").is_none());
        assert_eq!(r.store.deletes.get(), 1);
        assert_eq!(r.store.puts.get(), 0);
        assert!(r.store.ids(COST_HISTORIES).is_empty());
    });
}

#[test]
fn drops_a_deleted_workspaces_document_once_its_last_tick_falls_out_of_the_month() {
    // Rule 7: the store holds one month of them.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let mut r1 = rig();
        r1.fc = FakeClock::new(ms_of("2026-09-13T09:00:00.000Z"));
        let v = start(&r1, &roads, "ws_1", 0.5);
        let t1 = r1.tracker(Default::default());
        create(&r1, &v).await;
        let release = t1.watch(ticking());
        settle().await;
        advance(&r1, TICK_MS).await;
        r1.bus.fire(&deleted("ws_1"));
        r1.records.borrow_mut().clear();
        settle().await;
        release();
        assert_close(spend_on(&t1, "default", None).await.month_usd.0, usd(0.5, TICK_MS as f64));
        assert!(r1.store.text(COST_HISTORIES, "ws_1").is_some());

        // A host started the next month reads a series that ended in the one before it and lets it go.
        let r2 = restart(&r1, ms_of("2026-10-02T09:00:00.000Z"));
        let t2 = r2.tracker(Default::default());
        assert_eq!(t2.spend(&places(), None).await.unwrap(), vec![zeros("box"), zeros("default")]);
        settle().await;
        assert!(r2.store.deletes.get() >= 1);
        assert!(r2.store.text(COST_HISTORIES, "ws_1").is_none());
    });
}

#[test]
fn a_document_that_will_not_go_does_not_stop_the_load() {
    // Rule 7: losing the load with it would cost every live workspace its meter.
    run_local(async {
        let warned = console::capture();
        let r = rig();
        let old = json!({"workspaceId": "ws_old", "ended": true, "points": [
            {"workspaceId": "ws_old", "phase": "gone", "rateUsdPerHour": 0, "awakeMs": 1000, "accruedUsd": 0.1, "at": "2026-09-30T09:00:00.000Z"}
        ]});
        let live = json!({"workspaceId": "ws_live", "points": [
            {"workspaceId": "ws_live", "phase": "napping", "rateUsdPerHour": 0, "awakeMs": 5000, "accruedUsd": 0.25, "at": "2026-10-25T09:00:00.000Z"}
        ]});
        r.store.seed(COST_HISTORIES, "ws_old", old);
        r.store.seed(COST_HISTORIES, "ws_live", live);
        r.store.fail_delete.set(true);
        let t = r.tracker(Default::default());
        let history = t.history("ws_live").await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].awake_ms.0, 5000.0);
        assert_eq!(history[0].accrued_usd.0, 0.25);
        assert!(t.history("ws_old").await.is_empty());
        assert!(r.store.text(COST_HISTORIES, "ws_old").is_some());
        assert!(warned.lines().iter().any(|l| l.starts_with("the cost history of the deleted workspace ws_old could not be dropped")));
    });
}

#[test]
fn a_document_with_no_points_is_skipped_and_kept() {
    // Rule 7: no last point, or points that are not an array.
    run_local(async {
        let r = rig();
        let skipped = [
            ("ws_empty", json!({"workspaceId": "ws_empty", "points": []})),
            ("ws_bad", json!({"workspaceId": "ws_bad", "points": "nope"})),
            ("ws_old_empty", json!({"workspaceId": "ws_old_empty", "ended": true, "points": []})),
        ];
        for (id, doc) in &skipped {
            r.store.seed(COST_HISTORIES, id, doc.clone());
        }
        let point = json!({"workspaceId": "ws_ok", "phase": "napping", "rateUsdPerHour": 0, "awakeMs": 5000, "accruedUsd": 0.25, "at": "2026-10-25T09:00:00.000Z"});
        r.store.seed(COST_HISTORIES, "ws_ok", json!({"workspaceId": "ws_ok", "points": [point.clone()]}));
        let t = r.tracker(Default::default());

        let ok = t.history("ws_ok").await;
        assert_eq!(ok, vec![serde_json::from_value::<WorkspaceCostEvent>(point).unwrap()]);
        for (id, doc) in &skipped {
            assert!(t.history(id).await.is_empty());
            assert_eq!(&stored(&r, id), doc);
        }
        assert_eq!((r.store.deletes.get(), r.store.puts.get()), (0, 0));
    });
}

#[test]
fn a_failed_load_starts_empty_and_warns() {
    // Rule 7: the tracker starts empty and the series begins at the first tick.
    run_local(async {
        let warned = console::capture();
        let roads = Rc::new(Roads::default());
        let r = rig();
        let point = json!({"workspaceId": "ws_1", "phase": "napping", "rateUsdPerHour": 0, "awakeMs": 5000, "accruedUsd": 0.25, "at": "2026-10-25T09:00:00.000Z"});
        r.store.seed(COST_HISTORIES, "ws_1", json!({"workspaceId": "ws_1", "points": [point]}));
        r.store.fail_list.set(true);
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        assert!(t.history("ws_1").await.is_empty());
        assert!(warned.lines().iter().any(|l| l.starts_with("cost histories not loaded; the series begins at the first tick")));

        create(&r, &v).await;
        let history = t.history("ws_1").await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].phase, WorkspacePhase::Running);
        assert_eq!(history[0].awake_ms.0, 0.0);
        assert_eq!(stored(&r, "ws_1")["points"].as_array().unwrap().len(), 1);
    });
}

#[test]
fn only_an_added_point_writes_the_document_and_the_write_carries_the_state_when_it_runs() {
    // Rules 8, 10: ticks of one rate reach the store only when a point is added, and a queued write builds its
    // document when it runs, not when it was queued.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        let release = t.watch(ticking());
        settle().await;
        for _ in 0..3 {
            advance(&r, TICK_MS).await;
        }
        release();
        assert_eq!(r.store.puts.get(), 2);
        let doc = stored(&r, "ws_1");
        let points = doc["points"].as_array().unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(n(&points[1], "awakeMs"), TICK_MS as f64);
        assert_eq!(doc["where"], json!({"kind": "cloud", "machineId": "m_1", "place": "default", "provider": "default"}));
        assert!(doc.get("ended").is_none());
        assert_eq!(awakes(&t.history("ws_1").await), vec![0.0, 3.0 * TICK_MS as f64]);

        let r = rig();
        let gate = Rc::new(GateStore { inner: r.store.clone(), allowed: Cell::new(0) });
        let v = start(&r, &roads, "ws_1", 0.5);
        let _t = tracker_over(&r, gate.clone());
        create(&r, &v).await;
        advance(&r, TICK_MS).await;
        reprice(&r, "ws_1", 1.0);
        r.bus.fire(&upgraded("ws_1"));
        settle().await;
        advance(&r, TICK_MS).await;
        r.bus.fire(&deleted("ws_1"));
        settle().await;
        assert!(r.store.text(COST_HISTORIES, "ws_1").is_none());

        // The first write built its document before the second point existed.
        gate.allowed.set(1);
        settle().await;
        let first = stored(&r, "ws_1");
        assert_eq!(first["points"].as_array().unwrap().len(), 1);
        assert_eq!(first["where"]["machineId"], "m_1");
        assert!(first.get("ended").is_none());

        // The second was queued with two points and no end, and wrote what stood when it ran.
        gate.allowed.set(1);
        settle().await;
        let second = stored(&r, "ws_1");
        let points = second["points"].as_array().unwrap();
        assert_eq!(points.len(), 3);
        assert_eq!(n(&points[1], "rateUsdPerHour"), 1.0);
        assert_eq!(n(&points[1], "awakeMs"), TICK_MS as f64);
        assert_eq!(points[2]["phase"], "gone");
        assert_eq!(n(&points[2], "rateUsdPerHour"), 0.0);
        assert_eq!(n(&points[2], "awakeMs"), 2.0 * TICK_MS as f64);
        assert_close(n(&points[2], "accruedUsd"), usd(0.5, TICK_MS as f64) + usd(1.0, TICK_MS as f64));
        assert_eq!(points[2]["at"], at(2 * TICK_MS));
        assert_eq!(second["ended"], true);

        gate.allowed.set(1);
        settle().await;
        let text = r.store.text(COST_HISTORIES, "ws_1").unwrap();
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), second);
        assert!(text.starts_with("{\"workspaceId\":\"ws_1\",\"points\":["));
        assert!(text.ends_with("],\"where\":{\"kind\":\"cloud\",\"machineId\":\"m_1\",\"place\":\"default\",\"provider\":\"default\"},\"ended\":true}"));
    });
}

#[test]
fn a_tick_that_throws_is_warned_and_never_reaches_the_emitter() {
    // Rule 12.
    run_local(async {
        let warned = console::capture();
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = start(&r, &roads, "ws_1", 0.5);
        let _t = r.tracker(Default::default());
        r.records_fail.set(true);
        create(&r, &v).await;
        assert!(r.bus.costs().is_empty());
        assert!(r.bus.passed_costs().is_empty());
        assert!(warned.lines().iter().any(|l| l.starts_with("cost tick failed")));

        r.records_fail.set(false);
        wake(&r, "ws_1").await;
        let costs = r.bus.costs();
        assert_eq!(costs.len(), 1);
        assert_eq!(costs[0]["workspaceId"], "ws_1");
        assert_cost(&costs[0], "running", 0.5, 0);
    });
}

#[test]
fn totals_each_place_over_the_workspaces_metered_on_it_and_burns_only_for_the_ones_awake() {
    // Rule 16.
    run_local(async {
        let rate = 0.5;
        let roads = Rc::new(Roads::default());
        let r = rig();
        let alpha = start(&r, &roads, "ws_alpha", rate);
        let beta = start(&r, &roads, "ws_beta", rate);
        let t = r.tracker(Default::default());
        create(&r, &alpha).await;
        create(&r, &beta).await;
        let release = t.watch(ticking());
        settle().await;
        for _ in 0..3 {
            advance(&r, TICK_MS).await;
        }
        let both = t.spend(&places(), None).await.unwrap();
        // Both forks stand on the provider the records name rather than the first provider on the list, which reads
        // zeros; the computer the host runs on is not on it, since a computer takes no spend limit and nothing was
        // metered there.
        assert_eq!(both.iter().map(|x| x.place.as_str()).collect::<Vec<_>>(), vec!["box", "default"]);
        assert_eq!(both[0], zeros("box"));
        assert_close(both[1].rate_usd_per_hour.0, 2.0 * rate);
        let alone = t.history("ws_alpha").await.last().unwrap().accrued_usd.0 + t.history("ws_beta").await.last().unwrap().accrued_usd.0;
        assert_close(both[1].month_usd.0, alone);
        assert_close(both[1].month_usd.0, 2.0 * usd(rate, 3.0 * TICK_MS as f64));

        // A nap lands a tick of its own, which is where the rate it left reads.
        nap(&r, "ws_beta").await;
        let napped = spend_on(&t, "default", None).await;
        assert_close(napped.rate_usd_per_hour.0, rate);
        // What a place took before a nap is still what it took: the total holds where the meter stopped.
        assert!(napped.month_usd.0 >= both[1].month_usd.0 - 1e-9);
        release();
    });
}

#[test]
fn counts_a_month_from_its_first_day() {
    // Rule 16: a series that ran out in the month before is on no total of this one.
    run_local(async {
        let roads = Rc::new(Roads::default());
        let mut r = rig();
        r.fc = FakeClock::new(ms_of("2026-09-15T09:00:00.000Z"));
        let v = start(&r, &roads, "ws_1", 0.5);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        let release = t.watch(ticking());
        settle().await;
        for _ in 0..3 {
            advance(&r, TICK_MS).await;
        }
        let month = spend_on(&t, "default", None).await.month_usd.0;
        assert_close(month, usd(0.5, 3.0 * TICK_MS as f64));
        assert!(month > 0.0);
        release();
        // A machine still running would go on into the next month; one napped ran out where it napped.
        nap(&r, "ws_1").await;
        // The same series read a month later: every tick of it is behind that month's first day, so it owes nothing.
        assert_eq!(spend_on(&t, "default", Some(ms_of("2026-10-15T09:00:00.000Z"))).await, zeros("default"));
    });
}

#[test]
fn counts_of_a_stretch_that_began_yesterday_only_the_part_since_midnight() {
    // Rule 16: and the month the whole of it.
    run_local(async {
        let rate = 0.6;
        let midnight = ms_of("2026-09-16T00:00:00.000Z");
        let roads = Rc::new(Roads::default());
        let mut r = rig();
        r.fc = FakeClock::new(midnight - HOUR / 2);
        let v = start(&r, &roads, "ws_1", rate);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        assert_eq!(n(&r.bus.costs()[0], "rateUsdPerHour"), rate);
        let release = t.watch(every(10 * 60_000));
        settle().await;
        for _ in 0..6 {
            advance(&r, 10 * 60_000).await;
        }
        release();
        assert_eq!(r.fc.now(), midnight + HOUR / 2);
        let spent = spend_on(&t, "default", None).await;
        assert_close(spent.today_usd.0, rate / 2.0);
        assert_close(spent.month_usd.0, rate);
        assert!(spent.today_usd.0 <= spent.month_usd.0);
        // Read a minute before midnight, the same series is still yesterday's: 29 minutes of it.
        assert_close(spend_on(&t, "default", Some(midnight - 60_000)).await.today_usd.0, rate * 29.0 / 60.0);
    });
}

#[test]
fn reads_a_running_machine_live_between_ticks_while_nobody_watches() {
    // Rule 16: from the rate it was made at.
    run_local(async {
        let rate = 0.6;
        let midnight = ms_of("2026-09-16T00:00:00.000Z");
        let roads = Rc::new(Roads::default());
        let mut r = rig();
        r.fc = FakeClock::new(midnight + HOUR);
        let v = start(&r, &roads, "ws_1", rate);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        assert_eq!(n(&r.bus.costs()[0], "rateUsdPerHour"), rate);
        advance(&r, 2 * HOUR).await;
        assert_eq!(r.bus.costs().len(), 1);
        let spent = spend_on(&t, "default", None).await;
        assert_eq!(spent.rate_usd_per_hour.0, rate);
        assert_close(spent.today_usd.0, 2.0 * rate);
        assert_close(spent.month_usd.0, 2.0 * rate);
        advance(&r, HOUR).await;
        assert_close(spend_on(&t, "default", None).await.today_usd.0, 3.0 * rate);
    });
}

#[test]
fn holds_the_figure_where_a_nap_stopped_the_machine_however_long_it_sleeps() {
    // Rule 16.
    run_local(async {
        let rate = 0.6;
        let midnight = ms_of("2026-09-16T00:00:00.000Z");
        let roads = Rc::new(Roads::default());
        let mut r = rig();
        r.fc = FakeClock::new(midnight + HOUR);
        let v = start(&r, &roads, "ws_1", rate);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        advance(&r, HOUR).await;
        nap(&r, "ws_1").await;
        let napped = spend_on(&t, "default", None).await;
        assert_close(napped.today_usd.0, rate);
        assert_eq!(napped.rate_usd_per_hour.0, 0.0);
        advance(&r, 3 * HOUR).await;
        assert_eq!(spend_on(&t, "default", None).await, napped);
    });
}

#[test]
fn reads_a_deleted_workspaces_day_on_its_cloud_and_carries_it_no_further() {
    // Rules 13, 16: what a deleted workspace spent today stays on its cloud.
    run_local(async {
        let rate = 0.6;
        let midnight = ms_of("2026-09-16T00:00:00.000Z");
        let roads = Rc::new(Roads::default());
        let mut r = rig();
        r.fc = FakeClock::new(midnight + HOUR);
        let v = start(&r, &roads, "ws_1", rate);
        let t = r.tracker(Default::default());
        create(&r, &v).await;
        // Nothing watches, so the hour it ran lands on no tick before the delete.
        advance(&r, HOUR).await;
        r.bus.fire(&deleted("ws_1"));
        r.records.borrow_mut().clear();
        settle().await;
        let kept = spend_on(&t, "default", None).await;
        assert_close(kept.today_usd.0, rate);
        assert_close(kept.month_usd.0, rate);
        assert_eq!(kept.rate_usd_per_hour.0, 0.0);
        advance(&r, 2 * HOUR).await;
        assert_eq!(spend_on(&t, "default", None).await, kept);
    });
}

#[test]
fn answers_every_limited_place_a_row_and_leaves_the_rest_off() {
    // Rule 16: zeros where nothing was ever metered, and this computer off.
    run_local(async {
        let r = rig();
        let t = r.tracker(Default::default());
        assert_eq!(t.spend(&places(), None).await.unwrap(), vec![zeros("box"), zeros("default")]);
    });
}

#[test]
fn registers_its_listeners_in_the_tss_order() {
    run_local(async {
        // Rules 1, 3: the meter's listener of a type runs before that type's tick, and the deleted listener drops
        // every per-workspace map the tracker holds.
        let r = rig();
        let _t = r.tracker(Default::default());
        let order: Vec<&str> = r.bus.listeners.borrow().iter().map(|(t, _)| *t).collect();
        assert_eq!(
            order,
            [
                "workspace.created",
                "workspace.woken",
                "workspace.upgraded",
                "workspace.napped",
                "workspace.gone",
                "workspace.deleted",
                "workspace.status",
                "workspace.created",
                "workspace.woken",
                "workspace.upgraded",
                "workspace.napped",
                "workspace.gone",
            ]
        );
    });
}
