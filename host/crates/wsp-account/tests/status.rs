// SPDX-License-Identifier: AGPL-3.0-only
//! The status rows and the re-recording of lost machines (spec/status.md). The area is driven through a fake context
//! whose clock, store, listing and machines the test sets; every expected sentence is the TS's own, written out in
//! the TS tests it is ported from (the `// TS:` line above each test), and the words and limits it imports come from
//! tests/fixtures/oracle/status.json (oracle/status.oracle.ts).
//!
//! One row of the rows' map is a TS function (`daemonReach`, `daemonAnswers`, `providerState`, `metrics`, `facts`,
//! `exec`): the Rust map holds `null` under that key, so the key's presence and place in the order are the contract.
mod common;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use common::{in_flight, live, oracle, plain, record, run, settle};
use wsp_account::account::core_types::{AccountResult, Entry};
use wsp_account::account::status::{Confirmed, ListedRow, ReadFailure, StatusArea, StatusCtx};
use wsp_core::types::wiring::{AdoptedMachine, WakeAsk, WorkspaceRecord};
use wsp_engine::orphans::{ReapFailure, ReapReason, ReapedMachine};
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::pull_request::PullRequestSeen;
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::exit::HostError;

/// Date.parse("2026-10-25T12:00:00.000Z"): where the fake clock starts.
const T0: i64 = 1_792_929_600_000;
const DAY: i64 = 86_400_000;
const MIN: i64 = 60_000;
const OWNER: &str = "h_owner";

fn civil_days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `new Date(ms).toISOString()`.
fn iso(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    let rem = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z", rem / 3600, rem % 3600 / 60, rem % 60, ms.rem_euclid(1000))
}

/// `Date.parse` of the one shape `iso` writes; anything else is NaN (None).
fn parse_iso(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 24 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'.' || b[23] != b'Z' {
        return None;
    }
    let n = |a: usize, z: usize| s[a..z].parse::<i64>().ok();
    Some(civil_days(n(0, 4)?, n(5, 7)?, n(8, 10)?) * DAY + n(11, 13)? * 3_600_000 + n(14, 16)? * MIN + n(17, 19)? * 1000 + n(20, 23)?)
}

fn fx() -> Value {
    oracle("status")
}

struct Facts {
    reason: Option<String>,
    idle_at: Option<f64>,
    away: Option<String>,
    unreached: Option<Value>,
    reach: String,
    daemon: bool,
    reads: (bool, Option<f64>, bool, bool),
    rate: f64,
}

impl Default for Facts {
    fn default() -> Self {
        Facts {
            reason: None,
            idle_at: None,
            away: None,
            unreached: None,
            reach: "ok".into(),
            daemon: false,
            reads: (false, None, false, false),
            rate: 0.11,
        }
    }
}

#[derive(Clone)]
struct MachineAt {
    state: String,
    get_fail: Option<ReadFailure>,
    stop_fail: Option<ReadFailure>,
    stream_url: Option<String>,
}

type Store = Rc<RefCell<IndexMap<String, IndexMap<String, Value>>>>;

struct World {
    clock: Rc<Cell<i64>>,
    store: Store,
    live: RefCell<Vec<Entry>>,
    facts: RefCell<HashMap<String, Facts>>,
    projects: Vec<String>,
    machines: RefCell<HashMap<String, MachineAt>>,
    reads: RefCell<Vec<String>>,
    stops: RefCell<Vec<String>>,
    events: RefCell<Vec<Value>>,
    log: RefCell<Vec<String>>,
    warns: RefCell<Vec<String>>,
    golden: RefCell<Option<String>>,
    adopt_gone_fails: RefCell<Option<String>>,
    boot_work: RefCell<HashSet<String>>,
    wired: String,
    ready_before_held: Cell<bool>,
    readied: Cell<bool>,
}

fn new_world(clock: Rc<Cell<i64>>, store: Store) -> Rc<World> {
    Rc::new(World {
        clock,
        store,
        live: RefCell::new(vec![]),
        facts: RefCell::new(HashMap::new()),
        projects: vec!["p1".into(), "p2".into()],
        machines: RefCell::new(HashMap::new()),
        reads: RefCell::new(vec![]),
        stops: RefCell::new(vec![]),
        events: RefCell::new(vec![]),
        log: RefCell::new(vec![]),
        warns: RefCell::new(vec![]),
        golden: RefCell::new(None),
        adopt_gone_fails: RefCell::new(None),
        boot_work: RefCell::new(HashSet::new()),
        wired: "wired".into(),
        ready_before_held: Cell::new(false),
        readied: Cell::new(false),
    })
}

struct Lab {
    w: Rc<World>,
    area: Rc<StatusArea>,
    clock: Rc<Cell<i64>>,
    store: Store,
}

fn lab() -> Lab {
    assert_eq!(parse_iso("2026-10-25T12:00:00.000Z"), Some(T0));
    assert_eq!(iso(T0 + DAY + 1), "2026-10-26T12:00:00.001Z");
    let clock = Rc::new(Cell::new(T0));
    let store: Store = Rc::new(RefCell::new(IndexMap::new()));
    let w = new_world(clock.clone(), store.clone());
    let area = StatusArea::new(w.clone());
    Lab { w, area, clock, store }
}

impl Lab {
    /// A second host over the same store and clock: what a restart leaves.
    fn restarted(&self) -> Lab {
        let w = new_world(self.clock.clone(), self.store.clone());
        *w.live.borrow_mut() = self.w.live.borrow().clone();
        *w.machines.borrow_mut() = self.w.machines.borrow().clone();
        let area = StatusArea::new(w.clone());
        Lab { w, area, clock: self.clock.clone(), store: self.store.clone() }
    }
    fn advance(&self, ms: i64) {
        self.clock.set(self.clock.get() + ms);
    }
    fn put(&self, collection: &str, id: &str, value: Value) {
        self.store.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), value);
    }
    fn get(&self, collection: &str, id: &str) -> Option<Value> {
        self.store.borrow().get(collection).and_then(|c| c.get(id)).cloned()
    }
    fn keys(&self, collection: &str) -> Vec<String> {
        self.store.borrow().get(collection).map(|c| c.keys().cloned().collect()).unwrap_or_default()
    }
    /// A machine of the provider at `place` and its listing row, `age_ms` old by its `createdAt` label.
    fn row(&self, id: &str, state: &str, workspace: Option<&str>, age_ms: i64) -> ListedRow {
        let mut labels: IndexMap<String, String> = IndexMap::new();
        labels.insert("wsp".into(), "1".into());
        labels.insert("wsp-host".into(), "1".into());
        labels.insert("wsp-owner".into(), OWNER.into());
        if let Some(ws) = workspace {
            labels.insert("wsp-workspace".into(), ws.into());
        }
        labels.insert("createdAt".into(), iso(self.clock.get() - age_ms));
        self.row_with(id, state, labels)
    }
    fn row_with(&self, id: &str, state: &str, labels: IndexMap<String, String>) -> ListedRow {
        self.w
            .machines
            .borrow_mut()
            .insert(id.into(), MachineAt { state: state.into(), get_fail: None, stop_fail: None, stream_url: None });
        ListedRow { id: id.into(), state: state.into(), labels }
    }
    fn machine(&self, id: &str, f: impl FnOnce(&mut MachineAt)) {
        f(self.w.machines.borrow_mut().get_mut(id).expect("a registered machine"));
    }
    fn hold(&self, record_json: Value) -> Entry {
        let entry = live(record(&record_json));
        self.w.live.borrow_mut().push(entry.clone());
        entry
    }
    fn names(&self, ws: &str, name: &str, project: Option<&str>) {
        let mut doc = json!({ "workspaceId": ws, "name": name });
        if let Some(p) = project {
            doc["project"] = json!(p);
        }
        self.put("workspace-names", ws, doc);
    }
    async fn sweep(&self, listing: &[ListedRow], known: &[&str]) -> Sweep {
        self.sweep_at(listing, known, "box").await
    }
    async fn sweep_at(&self, listing: &[ListedRow], known: &[&str], place: &str) -> Sweep {
        let mut known: BTreeSet<String> = known.iter().map(|k| k.to_string()).collect();
        let mut failed = vec![];
        let (adopted, reaped) = self.area.adopt_lost(listing, &mut known, &mut failed, place).await.expect("adopt_lost answers");
        Sweep { adopted, reaped, known, failed }
    }
}

struct Sweep {
    adopted: Vec<AdoptedMachine>,
    reaped: Vec<ReapedMachine>,
    known: BTreeSet<String>,
    failed: Vec<ReapFailure>,
}

impl Sweep {
    fn failed_lines(&self) -> Vec<(Option<String>, String)> {
        self.failed.iter().map(|f| (f.id.clone(), f.message.clone())).collect()
    }
    fn known_ids(&self) -> Vec<String> {
        self.known.iter().cloned().collect()
    }
}

/// The area's calls on the context that make a record real, in order.
fn steps_of(lab: &Lab) -> Vec<String> {
    lab.w
        .log
        .borrow()
        .iter()
        .filter(|l| ["attach ", "persist ", "sync ", "emit ", "status "].iter().any(|p| l.starts_with(p)))
        .cloned()
        .collect()
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

fn ids(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn unreached_line(whose: &str, state: &str, goes_at: i64) -> String {
    format!(
        "not recorded: {whose}, so nothing here reaches this {state} machine of yours; deleted at {}, a day after this host first saw it",
        iso(goes_at)
    )
}

#[async_trait(?Send)]
impl StatusCtx for World {
    async fn ready(&self) {
        self.readied.set(true);
        tokio::task::yield_now().await;
    }
    fn held(&self) -> Vec<Entry> {
        self.ready_before_held.set(self.readied.get());
        self.live.borrow().clone()
    }
    fn view(&self, record: &WorkspaceRecord) -> Result<Map<String, Value>, HostError> {
        let mut m = Map::new();
        m.insert("id".into(), json!(record.id));
        m.insert("name".into(), json!(record.name));
        m.insert("kind".into(), serde_json::to_value(record.kind).unwrap());
        m.insert("phase".into(), serde_json::to_value(record.phase).unwrap());
        Ok(m)
    }
    fn rate_usd_per_hour(&self, record: &WorkspaceRecord) -> f64 {
        self.facts.borrow().get(&record.id).map(|f| f.rate).unwrap_or(if record.kind == WorkspaceKind::Local { 0.0 } else { 0.11 })
    }
    fn row_reason(&self, entry: &Entry) -> Option<String> {
        self.facts.borrow().get(&entry.borrow().record.borrow().id).and_then(|f| f.reason.clone())
    }
    fn idle_at(&self, workspace_id: &str) -> Option<f64> {
        self.facts.borrow().get(workspace_id).and_then(|f| f.idle_at)
    }
    fn away_line(&self, record: &WorkspaceRecord) -> Option<String> {
        self.facts.borrow().get(&record.id).and_then(|f| f.away.clone())
    }
    fn unreached_of(&self, entry: &Entry) -> Option<Value> {
        self.facts.borrow().get(&entry.borrow().record.borrow().id).and_then(|f| f.unreached.clone())
    }
    fn reach_of(&self, entry: &Entry) -> String {
        self.facts.borrow().get(&entry.borrow().record.borrow().id).map(|f| f.reach.clone()).unwrap_or_else(|| "ok".into())
    }
    fn has_daemon(&self, entry: &Entry) -> bool {
        self.facts.borrow().get(&entry.borrow().record.borrow().id).is_some_and(|f| f.daemon)
    }
    fn machine_reads(&self, entry: &Entry) -> (bool, Option<f64>, bool, bool) {
        self.facts.borrow().get(&entry.borrow().record.borrow().id).map(|f| f.reads).unwrap_or((false, None, false, false))
    }
    fn polled_reach_set(&self, workspace_id: &str, machine_id: &str, reach: &str) {
        self.log.borrow_mut().push(format!("set {workspace_id} {machine_id} {reach}"));
    }
    fn polled_reach_delete(&self, workspace_id: &str) {
        self.log.borrow_mut().push(format!("delete {workspace_id}"));
    }
    fn boot_work_has(&self, workspace_id: &str) -> bool {
        self.boot_work.borrow().contains(workspace_id)
    }
    fn revive_daemon(&self, entry: &Entry, reach: &str) {
        self.log.borrow_mut().push(format!("revive {} {reach}", entry.borrow().record.borrow().id));
    }
    fn offer_daemon_again(&self, entry: &Entry, status: &Value) {
        self.log.borrow_mut().push(format!("offer {} {status}", entry.borrow().record.borrow().id));
    }
    fn read_version_again(&self, entry: &Entry, status: &Value) {
        self.log.borrow_mut().push(format!("version {} {status}", entry.borrow().record.borrow().id));
    }
    async fn adopt_gone(&self, entry: &Entry, reason: &str) -> AccountResult<()> {
        self.log.borrow_mut().push(format!("adopt_gone {} {reason}", entry.borrow().record.borrow().id));
        match self.adopt_gone_fails.borrow().clone() {
            Some(m) => Err(plain(&m).into()),
            None => Ok(()),
        }
    }
    fn live(&self, workspace_id: &str) -> Option<Entry> {
        self.live.borrow().iter().find(|e| e.borrow().record.borrow().id == workspace_id).cloned()
    }
    async fn store_get(&self, collection: &str, id: &str) -> AccountResult<Option<Value>> {
        tokio::task::yield_now().await;
        Ok(self.store.borrow().get(collection).and_then(|c| c.get(id)).cloned())
    }
    async fn store_put(&self, collection: &str, id: &str, value: Value) -> AccountResult<()> {
        tokio::task::yield_now().await;
        self.log.borrow_mut().push(format!("put {collection} {id}"));
        self.store.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), value);
        Ok(())
    }
    async fn store_list(&self, collection: &str) -> AccountResult<Vec<Value>> {
        tokio::task::yield_now().await;
        Ok(self.store.borrow().get(collection).map(|c| c.values().cloned().collect()).unwrap_or_default())
    }
    async fn store_delete(&self, collection: &str, id: &str) -> AccountResult<()> {
        tokio::task::yield_now().await;
        self.log.borrow_mut().push(format!("delete {collection} {id}"));
        if let Some(c) = self.store.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    fn now(&self) -> i64 {
        self.clock.get()
    }
    fn wall_now(&self) -> i64 {
        self.clock.get()
    }
    fn owner(&self) -> String {
        OWNER.into()
    }
    fn lost_workspace(&self, row: &ListedRow, owner: &str, now: i64) -> bool {
        if row.state != "running" && row.state != "paused" {
            return false;
        }
        let l = &row.labels;
        if l.contains_key("poc") || l.get("wsp").map(String::as_str) != Some("1") || l.get("wsp-owner").map(String::as_str) != Some(owner) {
            return false;
        }
        if l.get("wsp-builder").map(String::as_str) == Some("1") || l.get("wsp-smoke").map(String::as_str) == Some("1") {
            return false;
        }
        match l.get("createdAt").and_then(|c| parse_iso(c)) {
            None => true,
            Some(at) => now - at >= MIN,
        }
    }
    fn project_held(&self, id: &str) -> bool {
        self.projects.iter().any(|p| p == id)
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        self.live.borrow().iter().any(|e| e.borrow().record.borrow().name == name).then(|| format!("{name} is taken"))
    }
    fn places_wired(&self) -> String {
        self.wired.clone()
    }
    async fn golden_head(&self) -> Option<String> {
        self.golden.borrow().clone()
    }
    async fn confirm(&self, _place: &str, machine_id: &str) -> Result<Confirmed, ReadFailure> {
        self.reads.borrow_mut().push(machine_id.to_string());
        match self.machines.borrow().get(machine_id) {
            None => Err(ReadFailure::Missing),
            Some(m) => match &m.get_fail {
                Some(f) => Err(f.clone()),
                None => Ok(Confirmed {
                    state: m.state.clone(),
                    stream_url: m.stream_url.clone(),
                    size: json!({ "cpu": 2, "memMb": 4096 }),
                    machine: common::FakeMachine::new(machine_id),
                }),
            },
        }
    }
    async fn stop_unrecorded(&self, place: &str, machine_id: &str) -> Result<(), ReadFailure> {
        self.log.borrow_mut().push(format!("stop {place} {machine_id}"));
        let fail = self.machines.borrow().get(machine_id).and_then(|m| m.stop_fail.clone());
        match fail {
            Some(f) => Err(f),
            None => {
                self.stops.borrow_mut().push(machine_id.to_string());
                Ok(())
            }
        }
    }
    fn attach(&self, record: WorkspaceRecord, _machine: std::sync::Arc<dyn wsp_engine::machine::Machine>) -> Result<Entry, HostError> {
        Ok(self.attach_plain(record))
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()> {
        self.log.borrow_mut().push(format!("persist {}", record.id));
        self.store.borrow_mut().entry("workspaces".into()).or_default().insert(record.id.clone(), serde_json::to_value(record).unwrap());
        Ok(())
    }
    fn sync_daemon(&self, entry: &Entry) {
        self.log.borrow_mut().push(format!("sync {}", entry.borrow().record.borrow().id));
    }
    fn emit(&self, event: Value) {
        self.log.borrow_mut().push(format!("emit {}", event["type"].as_str().unwrap_or("")));
        self.events.borrow_mut().push(event);
    }
    async fn emit_status(&self, entry: &Entry, reach: &str, reason: &str) {
        self.log.borrow_mut().push(format!("status {} {reach} {reason}", entry.borrow().record.borrow().id));
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
    fn warn(&self, line: &str) {
        self.warns.borrow_mut().push(line.to_string());
    }
}

fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64().unwrap()),
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), norm(v))).collect()),
        other => other.clone(),
    }
}

fn keys_of(row: &Map<String, Value>) -> Vec<&str> {
    row.keys().map(String::as_str).collect()
}

fn entry_json(id: &str, kind: &str, phase: &str) -> Value {
    json!({ "id": id, "name": format!("name of {id}"), "kind": kind, "phase": phase, "project": "p1", "machineId": format!("m_{id}") })
}

fn full_facts() -> Facts {
    Facts {
        reason: some("disk is past the line"),
        idle_at: Some(1_700_000_000_000.0),
        away: some("no answer 5 min, is it on?"),
        unreached: Some(json!({ "since": 5 })),
        reach: "ok".into(),
        daemon: true,
        reads: (true, Some(5000.0), true, true),
        rate: 0.11,
    }
}

fn load(entry: &Entry) {
    entry.borrow_mut().generation = 3;
    entry.borrow_mut().wake_ask = Some(WakeAsk { ask: 2, of: 3 });
    entry.borrow_mut().checkout =
        Some(serde_json::from_value::<Checkout>(json!({ "branch": "main", "ahead": 1, "behind": 0, "changed": 2, "readAt": 7 })).unwrap());
    entry.borrow_mut().pr = Some(serde_json::from_value::<PullRequestSeen>(json!({ "why": "gh is not signed in", "readAt": 5 })).unwrap());
}

const FULL_KEYS: [&str; 21] = [
    "id",
    "name",
    "kind",
    "phase",
    "size",
    "rateUsdPerHour",
    "generation",
    "reason",
    "wakeAsk",
    "checkout",
    "pr",
    "idleAt",
    "away",
    "unreached",
    "daemonReach",
    "daemonAnswers",
    "daemonAnswersPromptMs",
    "providerState",
    "metrics",
    "facts",
    "exec",
];

// S1
#[test]
fn s1_row_keys_and_order() {
    run(async {
        let lab = lab();
        let full = lab.hold(entry_json("w_full", "cloud", "running"));
        load(&full);
        let bare = lab.hold(entry_json("w_bare", "cloud", "running"));
        let napping = lab.hold(entry_json("w_nap", "cloud", "napping"));
        let _ = (&bare, &napping);
        lab.w.facts.borrow_mut().insert("w_full".into(), full_facts());
        lab.w.facts.borrow_mut().insert("w_nap".into(), Facts { idle_at: Some(5.0), ..Facts::default() });

        let rows = lab.area.records().await.expect("rows");

        assert!(lab.w.ready_before_held.get(), "the rows wait for ready before they read the held entries");
        assert_eq!(
            rows.iter().map(|r| r["id"].as_str().unwrap().to_string()).collect::<Vec<_>>(),
            ids(&["w_full", "w_bare", "w_nap"]),
            "live map order"
        );
        assert_eq!(keys_of(&rows[0]), FULL_KEYS.to_vec());
        let want = json!({
            "id": "w_full", "name": "name of w_full", "kind": "cloud", "phase": "running",
            "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0.11, "generation": 3,
            "reason": "disk is past the line", "wakeAsk": { "ask": 2, "of": 3 },
            "checkout": { "branch": "main", "ahead": 1, "behind": 0, "changed": 2, "readAt": 7 },
            "pr": { "why": "gh is not signed in", "readAt": 5 },
            "idleAt": 1_700_000_000_000.0_f64, "away": "no answer 5 min, is it on?", "unreached": { "since": 5 },
            "daemonReach": null, "daemonAnswers": null, "daemonAnswersPromptMs": 5000.0,
            "providerState": null, "metrics": null, "facts": null, "exec": null
        });
        assert_eq!(norm(&Value::Object(rows[0].clone())), norm(&want));
        let bare_keys = vec!["id", "name", "kind", "phase", "size", "rateUsdPerHour", "generation", "providerState", "exec"];
        assert_eq!(keys_of(&rows[1]), bare_keys);
        assert_eq!(keys_of(&rows[2]), bare_keys, "an idle mark rides only a running record's row");
        assert_eq!(rows[2]["phase"], json!("napping"));
    });
}

#[test]
fn s1_facts_skipped_when_unreachable() {
    run(async {
        let lab = lab();
        lab.hold(entry_json("w_up", "cloud", "running"));
        lab.hold(entry_json("w_down", "cloud", "running"));
        let reads = (true, None, true, true);
        lab.w.facts.borrow_mut().insert("w_up".into(), Facts { reads, ..Facts::default() });
        lab.w.facts.borrow_mut().insert("w_down".into(), Facts { reads, reach: "unreachable".into(), ..Facts::default() });

        let rows = lab.area.records().await.expect("rows");

        assert_eq!(
            keys_of(&rows[0]),
            vec![
                "id",
                "name",
                "kind",
                "phase",
                "size",
                "rateUsdPerHour",
                "generation",
                "daemonAnswers",
                "providerState",
                "metrics",
                "facts",
                "exec"
            ]
        );
        assert_eq!(
            keys_of(&rows[1]),
            vec![
                "id",
                "name",
                "kind",
                "phase",
                "size",
                "rateUsdPerHour",
                "generation",
                "daemonAnswers",
                "providerState",
                "metrics",
                "exec"
            ]
        );
    });
}

// TS: test/status.test.ts:100
#[test]
fn ported_the_row_carries_the_record_size_and_the_rate_the_context_prices() {
    run(async {
        let lab = lab();
        lab.hold(entry_json("w1", "cloud", "running"));
        let rows = lab.area.records().await.expect("rows");
        assert_eq!(rows[0]["size"], json!({ "cpu": 2, "memMb": 4096 }));
        assert_eq!(rows[0]["phase"], json!("running"));
        assert!((rows[0]["rateUsdPerHour"].as_f64().unwrap() - 0.11).abs() < 1e-5);
    });
}

// TS: test/local-workspace.test.ts:1358 and test/status.test.ts:477
#[test]
fn ported_a_row_nobody_bills_carries_a_zero_rate() {
    run(async {
        let lab = lab();
        lab.hold(entry_json("w_mac", "local", "running"));
        let rows = lab.area.records().await.expect("rows");
        assert_eq!(rows[0]["rateUsdPerHour"].as_f64(), Some(0.0));
        assert_eq!(rows[0]["kind"], json!("local"));
    });
}

// TS: test/local-workspace.test.ts:1366
#[test]
fn ported_facts_ride_the_row_of_a_machine_that_has_them_alone() {
    run(async {
        let lab = lab();
        lab.hold(entry_json("w_mac", "local", "running"));
        lab.hold(entry_json("w_b1", "cloud", "running"));
        lab.w.facts.borrow_mut().insert("w_mac".into(), Facts { reads: (false, None, false, true), ..Facts::default() });
        let rows = lab.area.records().await.expect("rows");
        assert!(rows[0].contains_key("facts"));
        assert!(!rows[1].contains_key("facts"));
    });
}

// TS: test/status.test.ts:986
#[test]
fn ported_a_rename_leaves_the_idle_mark_where_it_was() {
    run(async {
        let lab = lab();
        let entry = lab.hold(entry_json("w1", "cloud", "running"));
        lab.w.facts.borrow_mut().insert("w1".into(), Facts { idle_at: Some(T0 as f64 + 20.0 * MIN as f64), ..Facts::default() });
        let first = lab.area.records().await.expect("rows");
        entry.borrow().record.borrow_mut().name = "the name he typed".into();
        let second = lab.area.records().await.expect("rows");
        assert_eq!(first[0]["idleAt"].as_f64(), Some(T0 as f64 + 20.0 * MIN as f64));
        assert_eq!(second[0]["idleAt"], first[0]["idleAt"]);
        assert_eq!(second[0]["name"], json!("the name he typed"));
    });
}

// S2
#[test]
fn s2_on_polled_order() {
    run(async {
        let lab = lab();
        lab.hold(entry_json("w1", "cloud", "running"));
        lab.hold(entry_json("w2", "cloud", "napping"));
        lab.hold(entry_json("w3", "cloud", "running"));
        lab.w.boot_work.borrow_mut().insert("w3".into());
        let s1 = json!({ "id": "w1", "machineId": "m_w1", "phase": "running", "reach": { "state": "ok" } });
        let s2 = json!({ "id": "w2", "machineId": "m_w2", "phase": "napping", "reach": { "state": "unreachable" } });
        let gone = json!({ "id": "w_unknown", "machineId": "m_x", "phase": "running", "reach": { "state": "ok" } });
        let replaced = json!({ "id": "w1", "machineId": "m_old", "phase": "running", "reach": { "state": "ok" } });
        let s3 = json!({ "id": "w3", "machineId": "m_w3", "phase": "running", "reach": { "state": "ok" } });

        lab.area.on_polled(&[s1.clone(), s2.clone(), gone, replaced, s3]);

        assert_eq!(
            *lab.w.log.borrow(),
            vec![
                "set w1 m_w1 ok".to_string(),
                "revive w1 ok".to_string(),
                format!("offer w1 {s1}"),
                format!("version w1 {s1}"),
                "delete w2".to_string(),
                "revive w2 unreachable".to_string(),
                format!("offer w2 {s2}"),
                format!("version w2 {s2}"),
                "set w3 m_w3 ok".to_string(),
            ]
        );
    });
}

// S3
#[test]
fn s3_on_gone_adopts_only_the_machine_the_record_still_holds() {
    run(async {
        let lab = lab();
        lab.hold(entry_json("w1", "cloud", "running"));
        lab.area.on_gone("w1", "m_old", "was replaced");
        lab.area.on_gone("w_unknown", "m_x", "nobody");
        settle().await;
        assert!(lab.w.log.borrow().is_empty(), "a sighting of a machine since replaced says nothing about the one now under the record");
        lab.area.on_gone("w1", "m_w1", "the provider says gone");
        settle().await;
        assert_eq!(*lab.w.log.borrow(), vec!["adopt_gone w1 the provider says gone".to_string()]);
        assert!(lab.w.warns.borrow().is_empty());
    });
}

#[test]
fn s3_a_gone_reading_that_fails_is_warned_whole() {
    run(async {
        let lab = lab();
        lab.hold(entry_json("w1", "cloud", "running"));
        *lab.w.adopt_gone_fails.borrow_mut() = some("the provider timed out");
        lab.area.on_gone("w1", "m_w1", "gone");
        settle().await;
        assert_eq!(*lab.w.warns.borrow(), vec!["w1's gone reading was not confirmed: the provider timed out".to_string()]);
    });
}

// S4
#[test]
fn s4_dropped_rows_older_than_the_watch_are_deleted() {
    run(async {
        let fx = fx();
        let watch = fx["droppedWatchMs"].as_i64().unwrap();
        let lab = lab();
        lab.put("dropped-machines", "m_old", json!({ "machineId": "m_old", "at": iso(T0 - watch) }));
        lab.put("dropped-machines", "m_young", json!({ "machineId": "m_young", "at": iso(T0 - watch + 1) }));
        let swept = lab.sweep(&[], &[]).await;
        assert!(swept.adopted.is_empty() && swept.reaped.is_empty() && swept.failed.is_empty() && swept.known.is_empty());
        assert_eq!(lab.keys("dropped-machines"), ids(&["m_young"]));
    });
}

// TS: test/reconcile.test.ts:220
#[test]
fn ported_a_deleted_machine_is_forgotten_a_week_on_by_the_runtimes_clock() {
    run(async {
        let watch = fx()["droppedWatchMs"].as_i64().unwrap();
        let lab = lab();
        lab.put("dropped-machines", "m1", json!({ "machineId": "m1", "at": iso(T0) }));
        lab.advance(watch - 1);
        lab.sweep(&[], &[]).await;
        assert_eq!(lab.keys("dropped-machines"), ids(&["m1"]));
        lab.advance(1);
        lab.sweep(&[], &[]).await;
        assert!(lab.keys("dropped-machines").is_empty());
    });
}

// S5
#[test]
fn s5_skips_table() {
    run(async {
        let lab = lab();
        let known = lab.row("m_known", "running", Some("ws_a"), 3 * DAY);
        let dropped_running = lab.row("m_dr", "running", Some("ws_b"), 3 * DAY);
        let dropped_paused = lab.row("m_dp", "paused", Some("ws_c"), 3 * DAY);
        let young = lab.row("m_young", "running", Some("ws_d"), 30_000);
        let stored = lab.row("m_stored", "paused", Some("ws_stored"), 3 * DAY);
        let holder_running = lab.row("m_h1", "running", Some("ws_h1"), 3 * DAY);
        let mut listing = vec![known, dropped_running, dropped_paused, young, stored, holder_running];
        for (ws, flag) in [("ws_creating", 0), ("ws_waking", 1), ("ws_deleting", 2), ("ws_napping", 3)] {
            let entry = lab.hold(
                json!({ "id": ws, "name": ws, "kind": "cloud", "phase": "running", "project": "p1", "machineId": format!("held_{ws}") }),
            );
            match flag {
                0 => entry.borrow_mut().creating = true,
                1 => entry.borrow_mut().waking = Some(in_flight()),
                2 => entry.borrow_mut().deleting = Some(in_flight()),
                _ => entry.borrow_mut().napping = Some(in_flight()),
            }
            listing.push(lab.row(&format!("m_{ws}"), "paused", Some(ws), 3 * DAY));
        }
        lab.hold(json!({ "id": "ws_h1", "name": "h1", "kind": "cloud", "phase": "running", "project": "p1", "machineId": "other" }));
        lab.put("dropped-machines", "m_dr", json!({ "machineId": "m_dr", "at": iso(T0 - DAY) }));
        lab.put("dropped-machines", "m_dp", json!({ "machineId": "m_dp", "at": iso(T0 - DAY) }));
        lab.put("workspaces", "ws_stored", json!({ "id": "ws_stored" }));

        let swept = lab.sweep(&listing, &["m_known"]).await;

        assert_eq!(
            swept.failed_lines(),
            vec![
                (some("m_dp"), unreached_line("no workspace of this host is ws_c", "paused", T0 + DAY)),
                (some("m_stored"), "not recorded: ws_stored is stored here but was not read at start, so its paused machine is left as it is until the host starts again".to_string()),
            ]
        );
        assert_eq!(swept.known_ids(), ids(&["m_dp", "m_known", "m_stored"]));
        assert!(swept.adopted.is_empty() && swept.reaped.is_empty());
        assert_eq!(lab.keys("unrecorded-machines"), ids(&["m_dp"]));
        assert!(lab.w.reads.borrow().is_empty(), "nothing is read off the provider for a row that is skipped or reported");
        assert!(lab.w.stops.borrow().is_empty());
    });
}

// TS: test/sweep-places.test.ts:153
#[test]
fn ported_a_record_the_start_left_as_it_was_is_never_adopted_over_the_stored_one() {
    run(async {
        let lab = lab();
        let stored_before = json!({ "id": "ws_1", "name": "there", "machineId": "m1", "phase": "napping", "theme": "rose" });
        lab.put("workspaces", "ws_1", stored_before.clone());
        lab.names("ws_1", "there", Some("p1"));
        let listing = [lab.row("m1", "paused", Some("ws_1"), 2 * MIN)];
        let swept = lab.sweep(&listing, &[]).await;
        assert!(swept.adopted.is_empty());
        assert_eq!(swept.failed_lines(), vec![(some("m1"), "not recorded: ws_1 is stored here but was not read at start, so its paused machine is left as it is until the host starts again".to_string())]);
        assert_eq!(lab.get("workspaces", "ws_1"), Some(stored_before));
        assert!(lab.keys("unrecorded-machines").is_empty());
        assert!(lab.w.stops.borrow().is_empty());
        assert_eq!(swept.known_ids(), ids(&["m1"]));
    });
}

// S6, TS: test/sweep-places.test.ts:40, :102, host/test/host.test.ts:621, test/sub-resume.test.ts:654
#[test]
fn s6_unreached_named_with_its_deletion_time() {
    run(async {
        let lab = lab();
        let none = lab.row("m_none", "paused", Some("ws_d913623a"), 3 * DAY);
        let running = lab.row("m_run", "running", Some("ws_07165147"), 3 * DAY);
        lab.hold(json!({ "id": "ws_idle", "name": "idle", "kind": "cloud", "phase": "released", "project": "p1", "machineId": "" }));
        lab.hold(json!({ "id": "ws_moved", "name": "moved", "kind": "cloud", "phase": "napping", "project": "p1", "machineId": "held" }));
        let empty = lab.row("m_empty", "paused", Some("ws_idle"), 3 * DAY);
        let moved = lab.row("m_moved", "paused", Some("ws_moved"), 3 * DAY);
        let listing = [none, running, empty, moved];

        for round in 0..2 {
            let swept = lab.sweep(&listing, &[]).await;
            let goes = T0 + DAY;
            assert_eq!(
                swept.failed_lines(),
                vec![
                    (some("m_none"), unreached_line("no workspace of this host is ws_d913623a", "paused", goes)),
                    (some("m_run"), unreached_line("no workspace of this host is ws_07165147", "running", goes)),
                    (some("m_empty"), unreached_line("ws_idle holds no machine", "paused", goes)),
                    (some("m_moved"), unreached_line("ws_moved runs on held", "paused", goes)),
                ],
                "sweep {round}"
            );
            assert_eq!(swept.known_ids(), ids(&["m_empty", "m_moved", "m_none", "m_run"]));
            assert!(swept.reaped.is_empty() && swept.adopted.is_empty());
            for id in ["m_none", "m_run", "m_empty", "m_moved"] {
                assert_eq!(
                    lab.get("unrecorded-machines", id).unwrap().to_string(),
                    format!("{{\"machineId\":\"{id}\",\"place\":\"box\",\"seenAt\":\"{}\"}}", iso(T0)),
                    "the first sighting is stored whole, in this key order"
                );
            }
            lab.advance(10 * MIN);
        }
        assert!(lab.w.stops.borrow().is_empty(), "nothing is stopped before the day is out");
        assert_eq!(lab.w.live.borrow()[1].borrow().record.borrow().machine_id, "held", "the record is left as it was");
    });
}

// TS: test/sweep-places.test.ts:54, :71, :235
#[test]
fn s6_deleted_a_day_after_first_seen() {
    run(async {
        let day = fx()["unrecordedDeleteMs"].as_i64().unwrap();
        assert_eq!(day, DAY);
        let lab = lab();
        let paused = lab.row("m_p", "paused", Some("ws_d913623a"), 3 * DAY);
        let running = lab.row("m_r", "running", Some("ws_07165147"), 3_600_000);
        let listing = [paused, running];
        lab.sweep(&listing, &[]).await;
        lab.advance(day - 1);
        let early = lab.sweep(&listing, &[]).await;
        assert!(early.reaped.is_empty());
        assert!(lab.w.stops.borrow().is_empty());
        assert_eq!(early.failed.len(), 2);
        lab.advance(1);

        let late = lab.sweep(&listing, &[]).await;

        assert!(late.failed.is_empty(), "the sweep that deletes it names it no more");
        assert_eq!(*lab.w.stops.borrow(), ids(&["m_p", "m_r"]));
        assert_eq!(
            lab.w.log.borrow().iter().filter(|l| l.starts_with("stop ")).cloned().collect::<Vec<_>>(),
            vec!["stop box m_p".to_string(), "stop box m_r".to_string()]
        );
        let mut labels_p: IndexMap<String, String> = IndexMap::new();
        labels_p.insert("wsp".into(), "1".into());
        labels_p.insert("wsp-host".into(), "1".into());
        labels_p.insert("wsp-owner".into(), OWNER.into());
        labels_p.insert("wsp-workspace".into(), "ws_d913623a".into());
        labels_p.insert("createdAt".into(), iso(T0 - 3 * DAY));
        assert_eq!(
            late.reaped,
            vec![
                ReapedMachine {
                    id: "m_p".into(),
                    labels: Some(labels_p),
                    builder: false,
                    reason: ReapReason::Unrecorded,
                    age_ms: Some(4 * DAY),
                    name: None
                },
                ReapedMachine {
                    id: "m_r".into(),
                    labels: Some(listing[1].labels.clone()),
                    builder: false,
                    reason: ReapReason::Unrecorded,
                    age_ms: Some(DAY + 3_600_000),
                    name: None
                },
            ]
        );
        assert!(lab.keys("unrecorded-machines").is_empty());
        assert_eq!(late.known_ids(), ids(&["m_p", "m_r"]));
    });
}

// S6 restart, TS: test/sweep-places.test.ts:54 (the stored first sighting is the only memory of it)
#[test]
fn s6_first_sighting_survives_a_restart() {
    run(async {
        let lab = lab();
        let row = lab.row("m1", "paused", Some("ws_d913623a"), 3 * DAY);
        let listing = [row];
        let first = lab.sweep(&listing, &[]).await;
        assert_eq!(
            first.failed_lines(),
            vec![(some("m1"), unreached_line("no workspace of this host is ws_d913623a", "paused", T0 + DAY))]
        );
        let stamp = lab.get("unrecorded-machines", "m1").unwrap().to_string();

        lab.advance(6 * 3_600_000);
        let again = lab.restarted();
        let second = again.sweep(&listing, &[]).await;

        assert_eq!(
            second.failed_lines(),
            vec![(some("m1"), unreached_line("no workspace of this host is ws_d913623a", "paused", T0 + DAY))],
            "the deletion time does not move with the restart"
        );
        assert_eq!(again.get("unrecorded-machines", "m1").unwrap().to_string(), stamp, "the stored first sighting is not written again");
        assert!(!again.w.log.borrow().iter().any(|l| l.starts_with("put unrecorded-machines")), "a host that finds the row writes nothing");

        again.advance(DAY - 6 * 3_600_000 - 1);
        let third = again.restarted().sweep(&listing, &[]).await;
        assert!(third.reaped.is_empty());
        again.advance(1);
        let last = again.restarted();
        let swept = last.sweep(&listing, &[]).await;
        assert_eq!(swept.reaped.len(), 1);
        assert_eq!(*last.w.stops.borrow(), ids(&["m1"]));
        assert!(last.keys("unrecorded-machines").is_empty());
    });
}

#[test]
fn s6_a_failed_delete_is_retried() {
    run(async {
        let lab = lab();
        let listing = [lab.row("m1", "paused", Some("ws_1"), 3 * DAY), lab.row("m2", "paused", Some("ws_2"), 3 * DAY)];
        lab.sweep(&listing, &[]).await;
        lab.advance(DAY);
        lab.machine("m1", |m| m.stop_fail = Some(ReadFailure::Failed("provider said no".into())));
        lab.machine("m2", |m| m.stop_fail = Some(ReadFailure::Missing));

        let first = lab.sweep(&listing, &[]).await;

        assert_eq!(
            first.failed_lines(),
            vec![(some("m1"), "could not delete: provider said no; no record of this host's reaches it, retried next sweep".to_string())]
        );
        assert_eq!(
            first.reaped.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
            ids(&["m2"]),
            "a machine the provider no longer has counts as stopped"
        );
        assert_eq!(first.known_ids(), ids(&["m1", "m2"]));
        assert_eq!(lab.keys("unrecorded-machines"), ids(&["m1"]), "the failed one is kept for the next sweep");

        lab.machine("m1", |m| m.stop_fail = None);
        let second = lab.sweep(&listing[..1], &[]).await;
        assert!(second.failed.is_empty());
        assert_eq!(second.reaped.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), ids(&["m1"]));
        assert!(lab.keys("unrecorded-machines").is_empty());
    });
}

#[test]
fn s6_a_reaped_machine_carries_its_age_where_its_label_parses() {
    run(async {
        let lab = lab();
        let mut bad: IndexMap<String, String> = IndexMap::new();
        for (k, v) in [("wsp", "1"), ("wsp-owner", OWNER), ("wsp-workspace", "ws_1"), ("createdAt", "yesterday")] {
            bad.insert(k.into(), v.into());
        }
        let mut absent = bad.clone();
        absent.shift_remove("createdAt");
        absent.insert("wsp-workspace".into(), "ws_2".into());
        let listing = [lab.row_with("m_bad", "paused", bad), lab.row_with("m_absent", "paused", absent)];
        lab.sweep(&listing, &[]).await;
        lab.advance(DAY);
        let swept = lab.sweep(&listing, &[]).await;
        assert_eq!(
            swept.reaped.iter().map(|r| (r.id.clone(), r.age_ms)).collect::<Vec<_>>(),
            vec![("m_bad".to_string(), None), ("m_absent".to_string(), None)]
        );
    });
}

// TS: test/sweep-places.test.ts:80
#[test]
fn ported_a_machine_the_listing_stops_carrying_is_forgotten() {
    run(async {
        let lab = lab();
        let listing = [lab.row("m1", "paused", Some("ws_d913623a"), 3 * DAY)];
        lab.sweep(&listing, &[]).await;
        assert_eq!(lab.keys("unrecorded-machines"), ids(&["m1"]));
        lab.advance(MIN);
        lab.sweep(&[], &[]).await;
        assert!(lab.keys("unrecorded-machines").is_empty());
    });
}

// S10
#[test]
fn s10_unrecorded_rows_the_listing_dropped_go() {
    run(async {
        let lab = lab();
        lab.put("unrecorded-machines", "m_gone", json!({ "machineId": "m_gone", "place": "box", "seenAt": iso(T0 - MIN) }));
        lab.put("unrecorded-machines", "m_elsewhere", json!({ "machineId": "m_elsewhere", "place": "other", "seenAt": iso(T0 - MIN) }));
        lab.put("unrecorded-machines", "m_still", json!({ "machineId": "m_still", "place": "box", "seenAt": iso(T0 - MIN) }));
        let listing = [lab.row("m_still", "paused", Some("ws_1"), 3 * DAY)];

        let swept = lab.sweep(&listing, &[]).await;

        assert_eq!(swept.failed.len(), 1);
        assert_eq!(lab.keys("unrecorded-machines"), ids(&["m_elsewhere", "m_still"]));
        assert_eq!(
            lab.get("unrecorded-machines", "m_still").unwrap()["seenAt"],
            json!(iso(T0 - MIN)),
            "a row already stored keeps its first sighting"
        );
    });
}

#[test]
fn an_empty_listing_adopts_nothing_and_still_prunes() {
    run(async {
        let lab = lab();
        lab.put("unrecorded-machines", "m1", json!({ "machineId": "m1", "place": "box", "seenAt": iso(T0) }));
        lab.put("dropped-machines", "m_old", json!({ "machineId": "m_old", "at": iso(T0 - 8 * DAY) }));
        let swept = lab.sweep(&[], &[]).await;
        assert!(swept.adopted.is_empty() && swept.reaped.is_empty() && swept.failed.is_empty() && swept.known.is_empty());
        assert!(lab.keys("unrecorded-machines").is_empty());
        assert!(lab.keys("dropped-machines").is_empty());
        assert!(lab.w.reads.borrow().is_empty());
    });
}

// S7, TS: test/sweep-places.test.ts:91, test/reconcile.test.ts:71 and :89
#[test]
fn s7_no_project_is_reported_and_left() {
    run(async {
        let lab = lab();
        let says = "not recorded: this host holds no project for it, and a workspace is one project's copy; it is a machine of yours still";
        lab.names("ws_kept", "kept", Some("p_gone"));
        lab.names("ws_bare", "bare", None);
        let unstamped = lab.row("m_unstamped", "running", None, 2 * MIN);
        let gone_project = lab.row("m_gone", "paused", Some("ws_kept"), 3 * DAY);
        let no_project = lab.row("m_noproject", "running", Some("ws_bare"), 3 * DAY);
        let listing = [unstamped, gone_project, no_project];

        for _ in 0..2 {
            let swept = lab.sweep(&listing, &[]).await;
            assert_eq!(
                swept.failed_lines(),
                vec![
                    (some("m_unstamped"), format!("{says} running")),
                    (some("m_gone"), format!("{says} paused")),
                    (some("m_noproject"), format!("{says} running"))
                ]
            );
            assert_eq!(swept.known_ids(), ids(&["m_gone", "m_noproject", "m_unstamped"]));
            lab.advance(DAY);
        }
        assert!(
            lab.w.reads.borrow().is_empty(),
            "a row reported off the listing alone is never read, since each read resets the provider's idle timer"
        );
        assert!(lab.w.stops.borrow().is_empty());
        assert!(lab.keys("unrecorded-machines").is_empty());
    });
}

// TS: test/reconcile.test.ts:89
#[test]
fn ported_a_machine_it_cannot_name_is_reported_off_the_listing_sweep_after_sweep() {
    run(async {
        let lab = lab();
        let stamped = lab.row("m_stamped", "running", Some("ws_1"), 2 * MIN);
        let unstamped = lab.row("m_unstamped", "running", None, 2 * MIN);
        let listing = [stamped, unstamped];
        for _ in 0..2 {
            let swept = lab.sweep(&listing, &[]).await;
            let mut got: Vec<String> = swept.failed.iter().map(|f| f.id.clone().unwrap()).collect();
            got.sort();
            assert_eq!(got, ids(&["m_stamped", "m_unstamped"]));
            assert_eq!(swept.failed[1].message, "not recorded: this host holds no project for it, and a workspace is one project's copy; it is a machine of yours still running");
            assert!(swept.failed[0].message.starts_with(
                "not recorded: no workspace of this host is ws_1, so nothing here reaches this running machine of yours; deleted at "
            ));
        }
        assert!(lab.w.reads.borrow().is_empty());
        assert!(lab.w.stops.borrow().is_empty());
    });
}

// S8, TS: test/reconcile.test.ts:105 and :150
#[test]
fn s8_the_confirming_read() {
    run(async {
        let lab = lab();
        lab.names("ws_a", "a", Some("p1"));
        lab.names("ws_b", "b", Some("p1"));
        lab.names("ws_c", "c", Some("p1"));
        lab.names("ws_d", "d", Some("p1"));
        let missing = lab.row("m_missing", "running", Some("ws_a"), 2 * MIN);
        let refused = lab.row("m_refused", "running", Some("ws_b"), 2 * MIN);
        let stopped = lab.row("m_stopped", "running", Some("ws_c"), 2 * MIN);
        let starting = lab.row("m_starting", "paused", Some("ws_d"), 2 * MIN);
        lab.w.machines.borrow_mut().remove("m_missing");
        lab.machine("m_refused", |m| m.get_fail = Some(ReadFailure::Failed("Bad Gateway".into())));
        lab.machine("m_stopped", |m| m.state = "stopped".into());
        lab.machine("m_starting", |m| m.state = "starting".into());

        let swept = lab.sweep(&[missing, refused, stopped, starting], &[]).await;

        assert!(swept.adopted.is_empty());
        assert_eq!(swept.failed_lines(), vec![(some("m_refused"), "not recorded: Bad Gateway; retried next sweep".to_string())]);
        assert_eq!(
            swept.known_ids(),
            ids(&["m_refused", "m_starting", "m_stopped"]),
            "a machine the provider no longer knows is left to the listing's lag, not claimed"
        );
        assert_eq!(*lab.w.reads.borrow(), ids(&["m_missing", "m_refused", "m_stopped", "m_starting"]));
        assert!(lab.w.live.borrow().is_empty());
        assert!(lab.w.stops.borrow().is_empty());
    });
}

// S8: a failing state ask is outside the TS `try` (status.ts:149): the adopt fails whole, the row after it is never
// read, and nothing is recorded.
#[test]
fn s8_a_failed_state_read_fails_the_adopt() {
    run(async {
        let lab = lab();
        lab.names("ws_a", "a", Some("p1"));
        lab.names("ws_b", "b", Some("p1"));
        let first = lab.row("m_a", "running", Some("ws_a"), 2 * MIN);
        let second = lab.row("m_b", "running", Some("ws_b"), 2 * MIN);
        lab.machine("m_a", |m| m.get_fail = Some(ReadFailure::StateFailed("state read timed out".into())));
        let mut known: BTreeSet<String> = BTreeSet::new();
        let mut failed = vec![];
        let err = lab.area.adopt_lost(&[first, second], &mut known, &mut failed, "box").await.expect_err("the adopt fails whole");
        assert_eq!(err.message, "state read timed out");
        assert_eq!(*lab.w.reads.borrow(), ids(&["m_a"]), "the row after it is never read");
        assert!(failed.is_empty());
        assert!(lab.w.live.borrow().is_empty());
    });
}

// TS: test/reconcile.test.ts:150
#[test]
fn ported_a_confirming_read_the_provider_refuses_is_tried_again_next_sweep() {
    run(async {
        let lab = lab();
        lab.names("ws_1", "first", Some("p1"));
        let listing = [lab.row("m1", "running", Some("ws_1"), 2 * MIN)];
        lab.machine("m1", |m| m.get_fail = Some(ReadFailure::Failed("Bad Gateway".into())));
        let first = lab.sweep(&listing, &[]).await;
        assert!(first.adopted.is_empty() && first.reaped.is_empty());
        assert_eq!(first.failed_lines(), vec![(some("m1"), "not recorded: Bad Gateway; retried next sweep".to_string())]);
        assert!(lab.w.live.borrow().is_empty());
        lab.machine("m1", |m| m.get_fail = None);
        let second = lab.sweep(&listing, &[]).await;
        assert_eq!(
            second.adopted,
            vec![AdoptedMachine { id: "m1".into(), workspace_id: "ws_1".into(), name: "first".into(), phase: WorkspacePhase::Running }]
        );
    });
}

// TS: test/reconcile.test.ts:105
#[test]
fn ported_a_machine_inside_its_create_minute_and_one_the_provider_no_longer_knows_are_left() {
    run(async {
        let lab = lab();
        lab.names("ws_1", "first", Some("p1"));
        let young = lab.row("m1", "running", Some("ws_1"), 30_000);
        let swept = lab.sweep(&[young], &[]).await;
        assert!(swept.adopted.is_empty() && swept.failed.is_empty() && swept.known.is_empty() && swept.reaped.is_empty());
        assert!(lab.w.reads.borrow().is_empty());
        let old = lab.row("m1", "running", Some("ws_1"), 2 * MIN);
        lab.w.machines.borrow_mut().remove("m1");
        let swept = lab.sweep(&[old], &[]).await;
        assert!(swept.adopted.is_empty() && swept.failed.is_empty() && swept.known.is_empty() && swept.reaped.is_empty());
        assert!(lab.w.live.borrow().is_empty());
    });
}

// TS: test/reconcile.test.ts:123
#[test]
fn ported_a_body_a_rebuild_replaced_is_not_recorded_twice() {
    run(async {
        let lab = lab();
        lab.hold(json!({ "id": "ws_1", "name": "first", "kind": "cloud", "phase": "running", "project": "p1", "machineId": "m1" }));
        let body = lab.row("m_body", "running", Some("ws_1"), 2 * MIN);
        let swept = lab.sweep(&[body], &[]).await;
        assert!(
            swept.adopted.is_empty() && swept.failed.is_empty() && swept.known.is_empty(),
            "the engine kills it, so it is neither claimed nor reported"
        );
        assert_eq!(lab.w.live.borrow().len(), 1);
        assert!(lab.w.reads.borrow().is_empty());
    });
}

// TS: test/reconcile.test.ts:175, :200, :510
#[test]
fn ported_a_deleted_workspaces_machine_that_comes_back() {
    run(async {
        let lab = lab();
        lab.put("dropped-machines", "m1", json!({ "machineId": "m1", "at": iso(T0 - 3_600_000) }));
        let running = lab.row("m1", "running", Some("ws_1"), 3_600_000);
        let swept = lab.sweep(&[running], &[]).await;
        assert!(
            swept.failed.is_empty() && swept.adopted.is_empty() && swept.known.is_empty(),
            "a dropped row that runs is left unclaimed so the engine kills it"
        );
        assert!(lab.w.live.borrow().is_empty());

        let paused = lab.row("m1", "paused", Some("ws_1"), 3_600_000);
        let swept = lab.sweep(&[paused], &[]).await;
        assert_eq!(swept.failed_lines(), vec![(some("m1"), unreached_line("no workspace of this host is ws_1", "paused", T0 + DAY))]);
        assert!(lab.w.stops.borrow().is_empty());
        assert_eq!(swept.known_ids(), ids(&["m1"]));
    });
}

// S9, TS: test/reconcile.test.ts:44, :63, :139, :446, test/lost-create.test.ts:134, test/sweep-places.test.ts:131
#[test]
fn s9_adopted_record_whole() {
    run(async {
        let fx = fx();
        let restored = fx["recordRestored"].as_str().unwrap().to_string();
        let lab = lab();
        lab.names("ws_1", "first", Some("p1"));
        let mut labels: IndexMap<String, String> = IndexMap::new();
        for (k, v) in [
            ("wsp", "1"),
            ("wsp-host", "1"),
            ("wsp-owner", OWNER),
            ("wsp-workspace", "ws_1"),
            ("wsp-name", "the fork's name"),
            ("wsp-golden", "snap_g"),
        ] {
            labels.insert(k.into(), v.into());
        }
        labels.insert("createdAt".into(), iso(T0 - 2 * MIN));
        let row = lab.row_with("m1", "running", labels.clone());

        let swept = lab.sweep_at(&[row], &[], "wired").await;

        assert_eq!(
            swept.adopted,
            vec![AdoptedMachine { id: "m1".into(), workspace_id: "ws_1".into(), name: "first".into(), phase: WorkspacePhase::Running }]
        );
        assert!(swept.failed.is_empty() && swept.reaped.is_empty() && swept.known.is_empty());
        let want = json!({
            "id": "ws_1", "name": "first", "kind": "cloud", "project": "p1", "machineId": "m1", "phase": "running", "golden": "snap_g",
            "createdAt": iso(T0 - 2 * MIN), "spec": { "labels": labels }, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false
        });
        assert_eq!(lab.get("workspaces", "ws_1"), Some(want.clone()));
        let view = json!({ "id": "ws_1", "name": "first", "kind": "cloud", "phase": "running" });
        assert_eq!(
            lab.w.events.borrow().iter().map(|e| e.to_string()).collect::<Vec<_>>(),
            vec![json!({ "type": "workspace.created", "workspace": view }).to_string()]
        );
        assert_eq!(
            steps_of(&lab),
            vec![
                "attach ws_1".to_string(),
                "persist ws_1".to_string(),
                "sync ws_1".to_string(),
                "emit workspace.created".to_string(),
                format!("status ws_1 ok {restored}")
            ],
            "attach, persist, sync the daemon, announce, then the status"
        );
        // The second sweep finds the workspace held and its machine running: nothing is adopted twice.
        let again = lab.sweep_at(&[ListedRow { id: "m1".into(), state: "running".into(), labels: labels.clone() }], &[], "wired").await;
        assert!(again.adopted.is_empty() && again.failed.is_empty() && again.known.is_empty());
    });
}

#[test]
fn s9_a_paused_machine_at_another_place_hydrates_napping_there() {
    run(async {
        let restored = fx()["recordRestored"].as_str().unwrap().to_string();
        let lab = lab();
        lab.names("ws_2", "there", Some("p2"));
        *lab.w.golden.borrow_mut() = some("snap_head");
        let mut labels: IndexMap<String, String> = IndexMap::new();
        for (k, v) in [("wsp", "1"), ("wsp-owner", OWNER), ("wsp-workspace", "ws_2"), ("createdAt", "not a date")] {
            labels.insert(k.into(), v.into());
        }
        let row = lab.row_with("m2", "paused", labels.clone());
        lab.machine("m2", |m| m.stream_url = some("https://screen/m2"));

        let swept = lab.sweep_at(&[row], &[], "box").await;

        assert_eq!(
            swept.adopted,
            vec![AdoptedMachine { id: "m2".into(), workspace_id: "ws_2".into(), name: "there".into(), phase: WorkspacePhase::Napping }]
        );
        let want = json!({
            "id": "ws_2", "name": "there", "kind": "cloud", "project": "p2", "place": "box", "machineId": "m2", "phase": "napping", "golden": "snap_head",
            "createdAt": iso(T0), "spec": { "labels": labels }, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false, "screen": { "streamUrl": "https://screen/m2" }
        });
        assert_eq!(
            lab.get("workspaces", "ws_2"),
            Some(want),
            "the golden head stands in for a missing label and now for an unreadable createdAt"
        );
        let log = lab.w.log.borrow().clone();
        assert!(!log.iter().any(|l| l.starts_with("sync ")), "a paused machine's daemon is not synced");
        assert_eq!(
            steps_of(&lab),
            vec![
                "attach ws_2".to_string(),
                "persist ws_2".to_string(),
                "emit workspace.created".to_string(),
                format!("status ws_2 napping {restored}")
            ]
        );
    });
}

#[test]
fn s9_the_golden_falls_back_to_empty_and_the_name_to_the_machine() {
    run(async {
        let lab = lab();
        lab.hold(
            json!({ "id": "ws_other", "name": "first", "kind": "cloud", "phase": "running", "project": "p1", "machineId": "m_other" }),
        );
        lab.names("ws_1", "first", Some("p1"));
        let mut labels: IndexMap<String, String> = IndexMap::new();
        for (k, v) in [("wsp", "1"), ("wsp-owner", OWNER), ("wsp-workspace", "ws_1")] {
            labels.insert(k.into(), v.into());
        }
        let row = lab.row_with("m1", "running", labels);
        let swept = lab.sweep_at(&[row], &[], "wired").await;
        assert_eq!(
            swept.adopted,
            vec![AdoptedMachine { id: "m1".into(), workspace_id: "ws_1".into(), name: "m1".into(), phase: WorkspacePhase::Running }],
            "a name a live workspace holds is not taken twice"
        );
        let stored = lab.get("workspaces", "ws_1").unwrap();
        assert_eq!(stored["golden"], json!(""));
        assert_eq!(stored["name"], json!("m1"));
        assert_eq!(stored["createdAt"], json!(iso(T0)));
    });
}

// TS: test/lost-create.test.ts:134
#[test]
fn ported_the_sweep_adopts_the_box_when_its_name_was_kept_and_no_record_holds_the_workspace() {
    run(async {
        let lab = lab();
        lab.names("ws_kept", "kept", Some("p1"));
        let row = lab.row("m_lost", "running", Some("ws_kept"), 3 * MIN);
        let swept = lab.sweep_at(&[row], &[], "wired").await;
        assert_eq!(
            swept.adopted,
            vec![AdoptedMachine {
                id: "m_lost".into(),
                workspace_id: "ws_kept".into(),
                name: "kept".into(),
                phase: WorkspacePhase::Running
            }]
        );
        assert!(lab.w.stops.borrow().is_empty());
        let stored = lab.get("workspaces", "ws_kept").unwrap();
        assert_eq!(stored["machineId"], json!("m_lost"));
    });
}

// TS: test/sweep-places.test.ts:131
#[test]
fn ported_a_lost_record_at_a_place_is_recorded_again_there() {
    run(async {
        let lab = lab();
        lab.names("ws_1", "there", Some("p1"));
        let row = lab.row("m1", "paused", Some("ws_1"), 2 * MIN);
        let swept = lab.sweep_at(&[row], &[], "box").await;
        assert_eq!(
            swept.adopted,
            vec![AdoptedMachine { id: "m1".into(), workspace_id: "ws_1".into(), name: "there".into(), phase: WorkspacePhase::Napping }]
        );
        let stored = lab.get("workspaces", "ws_1").unwrap();
        assert_eq!((stored["place"].clone(), stored["machineId"].clone()), (json!("box"), json!("m1")));
    });
}

// TS: test/reconcile.test.ts:446
#[test]
fn ported_the_name_a_person_gave_outranks_the_forks() {
    run(async {
        let lab = lab();
        lab.names("ws_1", "the name he typed", Some("p1"));
        let mut labels: IndexMap<String, String> = IndexMap::new();
        for (k, v) in [("wsp", "1"), ("wsp-owner", OWNER), ("wsp-workspace", "ws_1"), ("wsp-name", "first")] {
            labels.insert(k.into(), v.into());
        }
        labels.insert("createdAt".into(), iso(T0 - 2 * MIN));
        let row = lab.row_with("m1", "running", labels);
        let swept = lab.sweep_at(&[row], &[], "wired").await;
        assert_eq!(swept.adopted[0].name, "the name he typed");
        assert_eq!(lab.get("workspaces", "ws_1").unwrap()["name"], json!("the name he typed"));
    });
}

// TS: host/test/host.test.ts:621
#[test]
fn ported_a_box_first_seen_a_day_ago_is_deleted_and_a_young_one_named() {
    run(async {
        let lab = lab();
        let young = lab.row("m_young", "paused", Some("ws_young"), 3_600_000);
        let old = lab.row("m_old", "paused", Some("ws_old"), 3_600_000);
        lab.put("unrecorded-machines", "m_old", json!({ "machineId": "m_old", "place": "default", "seenAt": iso(T0 - 25 * 3_600_000) }));
        let swept = lab.sweep_at(&[young, old], &[], "default").await;
        assert_eq!(
            swept.failed_lines(),
            vec![(some("m_young"), unreached_line("no workspace of this host is ws_young", "paused", T0 + DAY))]
        );
        assert_eq!(swept.reaped.len(), 1);
        assert_eq!((swept.reaped[0].id.clone(), swept.reaped[0].age_ms), ("m_old".to_string(), Some(3_600_000)));
        assert_eq!(*lab.w.stops.borrow(), ids(&["m_old"]));
        assert_eq!(lab.keys("unrecorded-machines"), ids(&["m_young"]));
    });
}

impl World {
    fn attach_plain(&self, record: WorkspaceRecord) -> Entry {
        self.log.borrow_mut().push(format!("attach {}", record.id));
        let entry = live(record);
        self.live.borrow_mut().push(entry.clone());
        entry
    }
}
