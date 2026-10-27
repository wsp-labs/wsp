// SPDX-License-Identifier: AGPL-3.0-only
//! What gone.test.ts (packages/runtime/test/gone.test.ts at 65f6b89c7) wires around its cases: `testRuntime` over a fake
//! clock with the idle, status and gone-confirm numbers it names, the edge that answers for the machine, and the counters
//! `countReads` and `countAsks` stand for.
#![allow(dead_code)]
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use super::*;
use serde_json::Value;
use wsp_core::clock::FakeClock;
use wsp_core::console::Captured;
use wsp_core::status::{Reconcile, StatusListOptions, StatusWatchOptions};
use wsp_core::types::wiring::OriginStatusApi;
use wsp_core::types::wiring::RuntimeOptions;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::MachineBackend;
use wsp_store::store::Store;
use wsp_words::words::thread::{gone_words, GoneSeenBy, GoneSighting};

pub const WINDOW: i64 = 5 * 60_000;
pub const POLL: i64 = 15_000;
pub const COST: i64 = 60_000;
/// 2026-09-07T01:20:00Z
pub const T0: i64 = 1_788_744_000_000;

/// `provider` serves the runtime in place of the stub, for a test whose machines live behind a real backend.
#[derive(Default)]
pub struct Seed {
    pub backend: Option<Arc<StubBackend>>,
    pub provider: Option<Arc<dyn MachineBackend>>,
    pub store: Option<Rc<dyn Store>>,
}

pub struct T {
    pub fx: Fx,
    pub backend: Arc<StubBackend>,
    pub store: Rc<dyn Store>,
    pub fc: FakeClock,
    pub events: Events,
    pub _state: Option<State>,
}

impl T {
    pub fn statuses(&self) -> Vec<Value> {
        self.events.statuses()
    }
    pub fn costs(&self) -> Vec<Value> {
        self.events.of("workspace.cost")
    }
    pub fn gone_costs(&self) -> Vec<Value> {
        self.costs().into_iter().filter(|c| c["phase"] == "gone").collect()
    }
    pub fn last_cost(&self) -> Value {
        self.costs().last().cloned().expect("a cost tick")
    }
    pub fn last_status(&self) -> Value {
        self.statuses().last().cloned().expect("a status")
    }
    pub async fn record(&self, id: &str) -> Value {
        j(&self.fx.rt.workspaces().get(id, None, None).await.expect("the workspace"))
    }
    pub async fn phase(&self, id: &str) -> Value {
        self.record(id).await["phase"].clone()
    }
}

pub fn test_runtime(extra: impl FnOnce(&mut RuntimeOptions) + 'static, seed: Seed) -> T {
    let backend = seed.backend.unwrap_or_else(|| StubBackend::new(None));
    let (store, state) = match seed.store {
        Some(store) => (store, None),
        None => {
            let state = sqlite_state();
            (state.store.clone(), Some(state))
        }
    };
    let fc = FakeClock::new(T0);
    let wiring = match seed.provider {
        Some(provider) => over(provider),
        None => runtime(&backend),
    };
    let fx = wiring
        .store(&store)
        .clock(&fc)
        .with(move |o| {
            o.idle_default_window_ms = Some(WINDOW as u64);
            o.status = Some(StatusWatchOptions {
                cost_interval_ms: Some(COST),
                poll_interval_ms: Some(POLL),
                reconcile_min_ms: Some(0),
                probe_timeout_ms: Some(5_000),
                ..Default::default()
            });
            // The confirming read runs on the same tick unless a test is about the wait itself.
            o.gone_confirm_ms = Some(0);
            extra(o);
        })
        .build();
    let events = fx.events();
    T { fx, backend, store, fc, events, _state: state }
}

/// The provider's answer to a pause it will not do, as the Solari backend types its 409.
pub fn not_pausable(machine: &str) -> EngineError {
    EngineError::nap_refused(machine, "Not pausable")
}

/// The edge answers for the machine, not the daemon, and only when the test says so: a probe waits in `held` until the
/// test answers it (404) or drops it (a failed reach); either way the poll goes on to ask the provider, in the order
/// the test picks.
pub async fn edge_holding(m: &Arc<StubMachine>) -> Edge {
    let edge = Edge::start().await;
    m.set_preview_port_url(&format!("http://127.0.0.1:{}", edge.port));
    edge
}

/// `countReads(m)`: how many times the provider was asked for the machine's state from now on.
pub struct Reads {
    pub m: Arc<StubMachine>,
    pub base: u32,
}

impl Reads {
    pub fn of(m: &Arc<StubMachine>) -> Reads {
        Reads { m: m.clone(), base: m.calls.state.get() }
    }
    pub fn n(&self) -> u32 {
        self.m.calls.state.get() - self.base
    }
}

/// `countAsks(m)`: the state reads and the metrics reads from now on.
pub struct Asks {
    pub m: Arc<StubMachine>,
    pub state: u32,
    pub metrics: u32,
}

impl Asks {
    pub fn of(m: &Arc<StubMachine>) -> Asks {
        Asks { m: m.clone(), state: m.calls.state.get(), metrics: m.calls.metrics.get() }
    }
    pub fn state(&self) -> u32 {
        self.m.calls.state.get() - self.state
    }
    pub fn metrics(&self) -> u32 {
        self.m.calls.metrics.get() - self.metrics
    }
    pub fn both(&self) -> (u32, u32) {
        (self.state(), self.metrics())
    }
}

pub fn gone_lines(warned: &Captured) -> Vec<String> {
    warned.lines().into_iter().filter(|l| l.contains("is gone")).collect()
}

pub fn iso(ms: i64) -> String {
    wsp_wire::views::activity::iso_time(ms)
}

pub fn words(by: GoneSeenBy, at: i64, answer: Option<&str>) -> String {
    gone_words("m1", Some(&GoneSighting { by, at: at as f64, answer: answer.map(str::to_string) }))
}

pub async fn until_n(mut cond: impl FnMut() -> bool) {
    until(|| {
        let held = cond();
        async move { held }
    })
    .await;
}

/// `await new Promise(r => setImmediate(r))`.
pub async fn immediate() {
    sleep_ms(1).await;
}

pub fn doubt_lines(warned: &Captured) -> Vec<String> {
    warned.lines().into_iter().filter(|l| l.starts_with("host metrics for")).collect()
}

pub fn doubt_line(id: &str) -> String {
    format!("host metrics for m1 (workspace {id}) answered 404 host no longer knows this VM; the state read says running and the record follows the state read")
}

/// One status pass over the edge the test controls: the pass is awaited, so what it asked is known when it returns.
pub async fn pass(t: &T, edge: &Edge, reconcile: Reconcile) -> Value {
    let rt = t.fx.rt.clone();
    let rows = tokio::task::spawn_local(async move {
        OriginStatusApi::list(&*rt.status(), Some(StatusListOptions { reconcile: Some(reconcile), ..Default::default() }), None).await
    });
    until_n(|| edge.held() >= 1).await;
    edge.answer(edge.take().expect("a probe")).await;
    j(&rows.await.expect("the task").expect("a status list")[0])
}
