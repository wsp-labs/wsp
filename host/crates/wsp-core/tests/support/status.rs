// SPDX-License-Identifier: AGPL-3.0-only
//! The status tracker's fixtures: a record builder whose roads count their calls, a bus that hands the tracker's own
//! emits back to its listeners as the runtime wires it, the lifecycle events, and the tracker over them.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use serde_json::{json, Value};
use wsp_core::clock::{Clock, FakeClock};
use wsp_core::status::{DaemonAnswers, RouteReach, StatusRecord, StatusTracker, StatusTrackerOptions, StatusWatchOptions};
use wsp_engine::errors::{EngineError, RoadCause, RoadError, WspError};
use wsp_engine::machine::ExecResult;
use wsp_wire::views::workspace::{MachineFacts, MachineState, WorkspacePhase};
use wsp_wire::views::workspace_more::{WorkspaceStatus, WorkspaceView};
use wsp_wire::wire::events_union::EventUnion;

use super::MemStore;

/// 2026-10-25T10:00:00Z: a fixed start, so ISO stamps in the tests are stable.
pub const T0: i64 = 1_792_922_400_000;

pub fn view(id: &str, machine: &str, phase: &str) -> WorkspaceView {
    serde_json::from_value(json!({
        "id": id, "name": id, "machineId": machine, "phase": phase, "kind": "cloud", "golden": "snap_g",
        "createdAt": "2026-10-25T10:00:00.000Z",
        "project": {"id": "pr_1a2b3c4d", "name": "box", "path": "/home/dev/box", "computer": "pl_box"},
        "place": "default", "provider": "default"
    }))
    .unwrap()
}

/// What a record's roads have been asked, and the answers they give; a test changes the answers between reads.
pub struct Roads {
    pub state: RefCell<Result<MachineState, EngineError>>,
    pub state_asks: Cell<u32>,
    pub metrics: RefCell<Option<Result<(), EngineError>>>,
    pub metrics_asks: Cell<u32>,
    pub facts: RefCell<Option<Result<MachineFacts, EngineError>>>,
    pub facts_asks: Cell<u32>,
    /// `None` never answers.
    pub exec: RefCell<Option<Result<ExecResult, EngineError>>>,
    pub exec_asks: Cell<u32>,
    pub route: RefCell<Option<Result<RouteReach, EngineError>>>,
    pub route_asks: Cell<u32>,
    /// `None` never answers; `Some(ms)` answers `up` after `ms` on the test's clock.
    pub answers: RefCell<Option<Result<bool, EngineError>>>,
    pub answer_asks: Cell<u32>,
}

impl Default for Roads {
    fn default() -> Self {
        Roads {
            state: RefCell::new(Ok(MachineState::Running)),
            state_asks: Cell::new(0),
            metrics: RefCell::new(None),
            metrics_asks: Cell::new(0),
            facts: RefCell::new(None),
            facts_asks: Cell::new(0),
            exec: RefCell::new(Some(Ok(exec_ok()))),
            exec_asks: Cell::new(0),
            route: RefCell::new(None),
            route_asks: Cell::new(0),
            answers: RefCell::new(None),
            answer_asks: Cell::new(0),
        }
    }
}

pub fn exec_ok() -> ExecResult {
    serde_json::from_value(json!({"exitCode": 0, "stdout": "ok\n", "stderr": ""})).unwrap()
}

pub fn exec_failed(code: i64, stderr: &str) -> ExecResult {
    serde_json::from_value(json!({"exitCode": code, "stdout": "", "stderr": stderr})).unwrap()
}

pub fn facts(uptime_ms: f64) -> MachineFacts {
    serde_json::from_value(json!({"os": "Linux", "uptimeMs": uptime_ms, "folder": "/root/box"})).unwrap()
}

/// The provider's 404 for a machine it no longer knows.
pub fn missing(said: &str) -> EngineError {
    EngineError::Provider(
        serde_json::from_value::<WspError>(json!({"kind": "missing", "message": said, "status": 404}))
            .unwrap_or_else(|_| panic!("WspError shape")),
    )
}

/// A request that never left this computer: no DNS.
pub fn offline() -> EngineError {
    EngineError::Road(RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: "getaddrinfo ENOTFOUND edge.example".into(), code: Some("ENOTFOUND".into()), errors: None }),
    })
}

/// A request that left and met nothing: a refused port.
pub fn refused_port() -> EngineError {
    EngineError::Road(RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: "getaddrinfo ECONNREFUSED edge.example".into(), code: Some("ECONNREFUSED".into()), errors: None }),
    })
}

pub fn other(message: &str) -> EngineError {
    EngineError::Other { message: message.into(), code: None }
}

fn ready<T: 'static>(v: T) -> LocalBoxFuture<'static, T> {
    Box::pin(async move { v })
}

/// A record over `roads`: the roads a test did not set are absent, as on a record whose kind has none.
pub fn record(v: WorkspaceView, roads: &Rc<Roads>) -> StatusRecord {
    let r = roads.clone();
    let provider_state: wsp_core::status::RecordCall<MachineState> = Rc::new(move || {
        r.state_asks.set(r.state_asks.get() + 1);
        ready(r.state.borrow().clone())
    });
    let r = roads.clone();
    let exec: wsp_core::status::RecordExec = Rc::new(move |_cmd, _timeout| {
        r.exec_asks.set(r.exec_asks.get() + 1);
        match r.exec.borrow().clone() {
            Some(answer) => ready(answer),
            None => Box::pin(std::future::pending()),
        }
    });
    let mut rec = StatusRecord {
        view: v,
        size: serde_json::from_value(json!({"cpu": 2, "memMb": 2048})).unwrap(),
        rate_usd_per_hour: 0.0,
        generation: 1,
        idle_at: None,
        reason: None,
        checkout: None,
        pr: None,
        tree: None,
        wake_ask: None,
        daemon_reach: None,
        daemon_answers: None,
        daemon_answers_prompt_ms: None,
        provider_state,
        metrics: None,
        facts: None,
        exec,
        away: None,
        unreached: None,
    };
    if roads.metrics.borrow().is_some() {
        let r = roads.clone();
        rec.metrics = Some(Rc::new(move || {
            r.metrics_asks.set(r.metrics_asks.get() + 1);
            ready(r.metrics.borrow().clone().unwrap())
        }));
    }
    if roads.facts.borrow().is_some() {
        let r = roads.clone();
        rec.facts = Some(Rc::new(move || {
            r.facts_asks.set(r.facts_asks.get() + 1);
            ready(r.facts.borrow().clone().unwrap())
        }));
    }
    if roads.route.borrow().is_some() {
        let r = roads.clone();
        rec.daemon_reach = Some(Rc::new(move || {
            r.route_asks.set(r.route_asks.get() + 1);
            ready(r.route.borrow().clone().unwrap())
        }));
    }
    rec
}

/// Gives the record a machine that answers for its own daemon: `up` at once, or never when `answers` is `None`.
pub fn answering(rec: &mut StatusRecord, roads: &Rc<Roads>) {
    let r = roads.clone();
    let answers: DaemonAnswers = Rc::new(move |_timeout| {
        r.answer_asks.set(r.answer_asks.get() + 1);
        match r.answers.borrow().clone() {
            Some(answer) => ready(answer),
            None => Box::pin(std::future::pending()),
        }
    });
    rec.daemon_answers = Some(answers);
}

pub fn route(url: &str) -> RouteReach {
    RouteReach { url: url.to_string(), expires_at: (T0 + 3_600_000) as f64 }
}

/// The bus as the runtime wires it: what the tracker emits comes back to its own listeners, typed ones first, then
/// `"*"` ones, each in the order added.
#[derive(Default)]
pub struct Bus {
    pub emitted: RefCell<Vec<EventUnion>>,
    pub passed: RefCell<Vec<EventUnion>>,
    pub listeners: RefCell<Vec<(&'static str, wsp_core::status::Listener)>>,
}

pub fn type_of(e: &EventUnion) -> String {
    serde_json::to_value(e).unwrap()["type"].as_str().unwrap().to_string()
}

impl Bus {
    pub fn new() -> Rc<Bus> {
        Rc::new(Bus::default())
    }
    /// Delivers to the tracker's listeners without recording it: an event another area emitted.
    pub fn fire(&self, e: &EventUnion) {
        let t = type_of(e);
        let listeners: Vec<_> = self.listeners.borrow().iter().filter(|(k, _)| *k == t).map(|(_, l)| l.clone()).collect();
        let any: Vec<_> = self.listeners.borrow().iter().filter(|(k, _)| *k == "*").map(|(_, l)| l.clone()).collect();
        for l in listeners.into_iter().chain(any) {
            l(e);
        }
    }
    pub fn costs(&self) -> Vec<Value> {
        self.emitted.borrow().iter().filter(|e| type_of(e) == "workspace.cost").map(|e| serde_json::to_value(e).unwrap()).collect()
    }
    pub fn passed_costs(&self) -> Vec<Value> {
        self.passed.borrow().iter().filter(|e| type_of(e) == "workspace.cost").map(|e| serde_json::to_value(e).unwrap()).collect()
    }
    pub fn statuses(&self) -> Vec<WorkspaceStatus> {
        self.emitted
            .borrow()
            .iter()
            .filter_map(|e| match e {
                EventUnion::WorkspaceStatus(s) => Some(s.event.status.clone()),
                _ => None,
            })
            .collect()
    }
}

pub fn event(v: Value) -> EventUnion {
    serde_json::from_value(v).unwrap()
}

pub fn created(v: &WorkspaceView) -> EventUnion {
    event(json!({"type": "workspace.created", "workspace": v}))
}

pub fn woken(id: &str, machine: &str) -> EventUnion {
    event(json!({"type": "workspace.woken", "workspaceId": id, "machineId": machine}))
}

pub fn upgraded(id: &str) -> EventUnion {
    event(json!({"type": "workspace.upgraded", "workspaceId": id, "machineId": "sbx_1"}))
}

pub fn napped(id: &str, found: bool) -> EventUnion {
    if found {
        event(json!({"type": "workspace.napped", "workspaceId": id, "found": true}))
    } else {
        event(json!({"type": "workspace.napped", "workspaceId": id}))
    }
}

pub fn gone(id: &str, machine: &str) -> EventUnion {
    event(json!({"type": "workspace.gone", "workspaceId": id, "machineId": machine, "reason": "machine gone"}))
}

pub fn deleted(id: &str) -> EventUnion {
    event(json!({"type": "workspace.deleted", "workspaceId": id}))
}

pub type Records = Rc<RefCell<Vec<StatusRecord>>>;

pub struct Rig {
    pub fc: FakeClock,
    pub store: Rc<MemStore>,
    pub bus: Rc<Bus>,
    pub records: Records,
    pub gone: Rc<RefCell<Vec<(String, String, String)>>>,
    pub polled: Rc<RefCell<Vec<Vec<WorkspaceStatus>>>>,
    /// How many times the tracker listed the records.
    pub listed: Rc<Cell<u32>>,
    /// Set to make the record listing fail.
    pub records_fail: Rc<Cell<bool>>,
}

pub fn rig() -> Rig {
    Rig {
        fc: FakeClock::new(T0),
        store: MemStore::new(),
        bus: Bus::new(),
        records: Rc::default(),
        gone: Rc::default(),
        polled: Rc::default(),
        listed: Rc::default(),
        records_fail: Rc::default(),
    }
}

impl Rig {
    pub fn clock(&self) -> Rc<dyn Clock> {
        self.fc.clock()
    }

    pub fn tracker(&self, defaults: StatusWatchOptions) -> Rc<StatusTracker> {
        let (records, listed, fail) = (self.records.clone(), self.listed.clone(), self.records_fail.clone());
        let (b1, b2, b3) = (self.bus.clone(), self.bus.clone(), self.bus.clone());
        let (gone, polled) = (self.gone.clone(), self.polled.clone());
        StatusTracker::new(StatusTrackerOptions {
            records: Rc::new(move || {
                listed.set(listed.get() + 1);
                let answer = if fail.get() { Err("the store is gone".to_string()) } else { Ok(records.borrow().clone()) };
                Box::pin(async move { answer })
            }),
            store: self.store.clone(),
            emit: Rc::new(move |e| {
                b1.emitted.borrow_mut().push(e.clone());
                b1.fire(&e);
            }),
            pass: Rc::new(move |e| b2.passed.borrow_mut().push(e)),
            on: Rc::new(move |t, l| {
                b3.listeners.borrow_mut().push((t, l));
                Rc::new(|| {})
            }),
            defaults,
            on_polled: Some(Rc::new(move |rows| polled.borrow_mut().push(rows.to_vec()))),
            on_gone: Some(Rc::new(move |id, machine, reason| gone.borrow_mut().push((id.into(), machine.into(), reason.into())))),
            clock: Some(self.fc.clock()),
            zone: Some(jiff::tz::TimeZone::UTC),
        })
    }

    /// Puts a record in the listing, or replaces the one with its id.
    pub fn put(&self, rec: StatusRecord) {
        let mut rs = self.records.borrow_mut();
        match rs.iter().position(|r| r.view.id == rec.view.id) {
            Some(i) => rs[i] = rec,
            None => rs.push(rec),
        }
    }

    pub fn set_phase(&self, id: &str, phase: WorkspacePhase) {
        let mut rs = self.records.borrow_mut();
        let r = rs.iter_mut().find(|r| r.view.id == id).unwrap();
        r.view.phase = phase;
        r.generation += 1;
    }
}

/// A row's field as JSON, for assertions on what the wire would carry.
pub fn field(row: &WorkspaceStatus, key: &str) -> Value {
    serde_json::to_value(row).unwrap().get(key).cloned().unwrap_or(Value::Null)
}
