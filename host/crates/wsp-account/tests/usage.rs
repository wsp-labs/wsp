// SPDX-License-Identifier: AGPL-3.0-only
//! The usage area (spec/usage.md). Every expected value is the TS's: oracle/usage.oracle.ts ran `usageArea` at
//! 65f6b89c7 on a scripted context (the ledger, alerts, account rows, plan scripts and reset spender are recorders)
//! over the worlds written beside the answers in tests/fixtures/oracle/usage.json, and `World` below is the same
//! context built from one of those worlds. Where the TS reads a thing the oracle cannot run (the daemon road of a
//! readings ask, the spender's own rules) the expected value is the TS source's literal.
//!
//! Case list, one test per spec rule and one per ported TS case:
//!   U1  u1_away_line_table
//!   U2  u2_row_reason_order
//!   U3  u3_usage_stores_from_the_catalog
//!   U4  u4_logs_off_when_switched_off, u4_logs_read_once_a_window, u4_logs_skip_wsp_sessions,
//!       u4_logs_project_and_account, u4_an_old_daemon_is_quiet, u4_targets_are_this_computer_then_linked_boxes,
//!       u4_a_silent_computer_times_out_with_its_label, u4_a_read_that_filed_nothing_is_not_filed,
//!       u4_a_stale_reading_is_not_told_to_the_alerts
//!   U5  u5_sign_ins_held_a_minute_and_dropped_on_change, u5_a_failed_or_absent_read_lists_none_and_is_held
//!   U6  u6_accounts_add_this_computer
//!   U7  u7_reset_run_box_and_here
//!   U8  u8_plan_road_table, u8_plan_places_table
//!   U9  u9_plans_read_once_a_window, u9_plans_skip_a_vault_key_here, u9_a_failed_plan_read_warns_and_the_rest_go_on,
//!       u9_a_silent_plan_read_times_out_with_its_label, u9_one_plan_read_is_shared_while_it_is_out
//!   U10 u10_reset_names_the_place
//!   U11 u11_used_labels_table, u11_logs_are_read_where_outside_or_source_asks, u11_a_logs_failure_is_warned_and_used_answers
//!   U12 u12_readings_frame_and_answer
//!   U13 u13_fresh_accounts_read_the_plans
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use common::{error_json, in_flight, live, oracle, plain, record, run, settle};
use wsp_account::account::core_types::{AccountResult, Frame, Entry};
use wsp_core::clock::Cancel;
use wsp_account::account::usage::{LabelOf, NameOf, AccountKey, LimitReading, PlanRoad, ResetPlaceView, UsageArea, UsageCtx, UsageEntry};
use wsp_core::types::wiring::{DeleteSaid, ReadingsTarget, WorkspaceRecord};
use wsp_frames::{UsageLogsReply, UsageStore};
use wsp_wire::types::usage::{AccountsAnswer, HarnessLimit, HarnessLimitAccount, ReadingsRange, ResetAnswer, ResetOutcome, UsageRange, UsageSplit, UsedAnswer};
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::HostError;

/// Integral floats as integers, so a number the TS wrote as `40` equals one Rust wrote as `40.0`.
fn canon(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => json!(f as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(canon).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), canon(x))).collect()),
        _ => v.clone(),
    }
}

fn said(e: &wsp_account::account::core_types::AccountError) -> String {
    match e {
        wsp_account::account::core_types::AccountError::Host(h) => h.message.clone(),
        other => panic!("not a host error: {other:?}"),
    }
}

fn sorted(list: &[Value]) -> Vec<String> {
    let mut out: Vec<String> = list.iter().map(|v| canon(v).to_string()).collect();
    out.sort();
    out
}

fn words() -> Value {
    oracle("usage")["words"].clone()
}

fn word(name: &str) -> String {
    words()[name].as_str().unwrap().to_string()
}

fn scenario(name: &str) -> Value {
    oracle("usage")["scenarios"][name].clone()
}

struct Timer {
    id: u64,
    due: i64,
    live: Rc<Cell<bool>>,
    f: Option<Box<dyn FnOnce()>>,
}

/// The oracle's world as a context: what the test set is answered, what the area filed or asked is recorded.
struct World {
    w: RefCell<Value>,
    clock: Cell<i64>,
    timers: RefCell<Vec<Timer>>,
    next_timer: Cell<u64>,
    door: Cell<bool>,
    events: RefCell<Vec<String>>,
    warns: RefCell<Vec<String>>,
    asked: RefCell<Vec<Value>>,
    filed: RefCell<Vec<Value>>,
    readings: RefCell<Vec<Value>>,
    alerts: RefCell<Vec<Value>>,
    snapshots: RefCell<Vec<Value>>,
    used: RefCell<Vec<Value>>,
    rows: RefCell<Vec<Value>>,
    names: RefCell<Vec<Value>>,
    execs: RefCell<Vec<Value>>,
    spends: RefCell<Vec<Value>>,
    place_for_words: RefCell<Vec<String>>,
    spend_answer: RefCell<Result<ResetAnswer, HostError>>,
    histories: RefCell<Vec<Value>>,
    history_answer: RefCell<Result<Value, String>>,
    sign_in_reads: Cell<usize>,
    plan_reads_held: Cell<bool>,
    plan_gate: Rc<tokio::sync::Notify>,
    ready_calls: Cell<usize>,
    changed: RefCell<Vec<Rc<dyn Fn()>>>,
    nap: RefCell<Option<String>>,
    stop: RefCell<Option<String>>,
    disk: RefCell<Option<String>>,
}

fn world(name: &str) -> Rc<World> {
    let fixture = oracle("usage");
    World::of(fixture["worlds"][name].clone())
}

impl World {
    fn of(w: Value) -> Rc<World> {
        Rc::new(World {
            clock: Cell::new(w["now"].as_i64().unwrap()),
            w: RefCell::new(w),
            timers: RefCell::new(vec![]),
            next_timer: Cell::new(0),
            door: Cell::new(true),
            events: RefCell::new(vec![]),
            warns: RefCell::new(vec![]),
            asked: RefCell::new(vec![]),
            filed: RefCell::new(vec![]),
            readings: RefCell::new(vec![]),
            alerts: RefCell::new(vec![]),
            snapshots: RefCell::new(vec![]),
            used: RefCell::new(vec![]),
            rows: RefCell::new(vec![]),
            names: RefCell::new(vec![]),
            execs: RefCell::new(vec![]),
            spends: RefCell::new(vec![]),
            place_for_words: RefCell::new(vec![]),
            spend_answer: RefCell::new(Ok(ResetAnswer { outcome: ResetOutcome::Reset, said: "ok".into(), account: None })),
            histories: RefCell::new(vec![]),
            history_answer: RefCell::new(Ok(json!({ "points": [], "stepMs": 15000, "truncated": false }))),
            sign_in_reads: Cell::new(0),
            plan_reads_held: Cell::new(false),
            plan_gate: Rc::new(tokio::sync::Notify::new()),
            ready_calls: Cell::new(0),
            changed: RefCell::new(vec![]),
            nap: RefCell::new(None),
            stop: RefCell::new(None),
            disk: RefCell::new(None),
        })
    }

    fn get(&self, key: &str) -> Value {
        self.w.borrow()[key].clone()
    }

    fn edit(&self, f: impl FnOnce(&mut Value)) {
        f(&mut self.w.borrow_mut());
    }

    fn place_values(&self) -> Vec<Value> {
        self.get("places").as_array().cloned().unwrap_or_default()
    }

    fn views(&self) -> Vec<PlaceView> {
        self.place_values()
            .into_iter()
            .map(|mut p| {
                p["default"] = json!(false);
                serde_json::from_value(p).unwrap()
            })
            .collect()
    }

    fn event(&self, line: impl Into<String>) {
        self.events.borrow_mut().push(line.into());
    }

    fn listed(&self, key: &str, id: &str) -> bool {
        self.get(key).as_array().unwrap().contains(&json!(id))
    }

    /// Moves the clock, firing the timers that fall due in order, then lets what they woke run.
    async fn advance(&self, ms: i64) {
        let target = self.clock.get() + ms;
        loop {
            let next = {
                let timers = self.timers.borrow();
                timers.iter().filter(|t| t.live.get() && t.due <= target).min_by_key(|t| (t.due, t.id)).map(|t| t.id)
            };
            let Some(id) = next else { break };
            let (due, f) = {
                let mut timers = self.timers.borrow_mut();
                let t = timers.iter_mut().find(|t| t.id == id).unwrap();
                t.live.set(false);
                (t.due, t.f.take())
            };
            self.clock.set(due);
            if let Some(f) = f {
                f();
            }
            settle().await;
        }
        self.clock.set(target);
        settle().await;
    }

    fn mark(&self, label: &str) -> Value {
        let mut warns: Vec<String> = self.warns.borrow().clone();
        warns.sort();
        json!({
            "label": label,
            "asked": self.asked.borrow().len(),
            "filed": self.filed.borrow().len(),
            "execs": self.execs.borrow().len(),
            "readings": self.readings.borrow().len(),
            "warns": warns,
            "signInReads": self.sign_in_reads.get(),
        })
    }

    fn compare_marks(&self, name: &str, got: &[Value]) {
        let mut want = scenario(name)["marks"].as_array().unwrap().clone();
        for m in want.iter_mut() {
            let mut warns: Vec<String> = m["warns"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_string()).collect();
            warns.sort();
            m["warns"] = json!(warns);
        }
        assert_eq!(Value::Array(got.to_vec()), Value::Array(want), "marks of {name}");
    }
}

fn probe_labels(probe: &Value, labels: &dyn Fn(UsageSplit, &str) -> String) -> Vec<Value> {
    probe
        .as_object()
        .unwrap()
        .iter()
        .map(|(s, values)| {
            let which: UsageSplit = serde_json::from_value(json!(s)).unwrap();
            json!([s, values.as_array().unwrap().iter().map(|v| labels(which, v.as_str().unwrap())).collect::<Vec<_>>()])
        })
        .collect()
}

fn probe_names(probe: &Value, name_of: &dyn Fn(&str) -> String) -> Vec<String> {
    probe.as_array().unwrap().iter().map(|id| name_of(id.as_str().unwrap())).collect()
}

fn entry_json(e: &UsageEntry) -> Value {
    let mut o = Map::new();
    o.insert("at".into(), canon(&json!(e.at)));
    o.insert("agent".into(), json!(e.agent));
    o.insert("account".into(), json!(e.account));
    o.insert("accountLabel".into(), json!(e.account_label));
    o.insert("computer".into(), json!(e.computer));
    o.insert("project".into(), json!(e.project));
    o.insert("model".into(), json!(e.model));
    o.insert("tokens".into(), e.tokens.clone());
    if let Some(c) = e.cost_usd {
        o.insert("costUsd".into(), json!(c));
    }
    o.insert("session".into(), json!(e.session));
    o.insert("source".into(), json!(e.source));
    Value::Object(o)
}

fn reading_json(r: &LimitReading) -> Value {
    let mut o = Map::new();
    o.insert("key".into(), json!(r.key));
    o.insert("agent".into(), json!(r.agent));
    o.insert("label".into(), json!(r.label));
    o.insert("road".into(), json!(r.road));
    o.insert("computer".into(), json!(r.computer));
    o.insert("limit".into(), serde_json::to_value(&r.limit).unwrap());
    if let Some(at) = r.at {
        o.insert("at".into(), json!(at));
    }
    canon(&Value::Object(o))
}

fn agent_label(agent: &str) -> String {
    match agent {
        "claude" => "Claude Code".into(),
        "codex" => "Codex".into(),
        other => other.into(),
    }
}

fn own_account(agent: &str, place_id: &str, name: &str) -> AccountKey {
    AccountKey { key: format!("{agent}@{place_id}"), label: format!("{} signed in on {name}", agent_label(agent)), road: "own".into() }
}

#[async_trait(?Send)]
impl UsageCtx for World {
    fn now(&self) -> i64 {
        self.clock.get()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        let id = self.next_timer.get() + 1;
        self.next_timer.set(id);
        let live = Rc::new(Cell::new(true));
        self.timers.borrow_mut().push(Timer { id, due: self.clock.get() + ms, live: live.clone(), f: Some(f) });
        Rc::new(move || live.set(false))
    }
    async fn usage_logs_on(&self) -> bool {
        self.w.borrow()["usageLogs"] == json!(true)
    }
    async fn places(&self, _now: i64) -> Vec<PlaceView> {
        self.event("places");
        self.views()
    }
    fn has_door(&self) -> bool {
        self.door.get()
    }
    fn place_away(&self, place_id: &str) -> bool {
        self.listed("away", place_id)
    }
    fn name_of(&self, place_id: &str) -> String {
        self.place_values().iter().find(|p| p["id"] == json!(place_id)).and_then(|p| p["name"].as_str().map(String::from)).unwrap_or(place_id.to_string())
    }
    fn linked(&self, place_id: &str) -> bool {
        self.listed("links", place_id)
    }
    fn signed_in_at(&self, place_id: &str, agent: &str) -> bool {
        self.w.borrow()["signInsAt"][place_id][agent] == json!("signed-in")
    }
    async fn place_for(&self, word: &str) -> Option<String> {
        self.place_for_words.borrow_mut().push(word.to_string());
        if !self.door.get() {
            return None;
        }
        self.w.borrow()["placeFor"][word].as_str().map(String::from)
    }
    async fn box_login_path(&self, place_id: &str) -> Option<String> {
        self.w.borrow()["reports"][place_id]["login"]["PATH"].as_str().map(String::from)
    }
    async fn box_exec(&self, place_id: &str, script: &str, timeout_ms: i64, stdin: &str) -> AccountResult<String> {
        if let Some(e) = self.w.borrow()["boxError"][place_id].as_str() {
            return Err(plain(e).into());
        }
        self.execs.borrow_mut().push(json!({ "where": place_id, "script": script, "timeoutMs": timeout_ms, "stdin": stdin }));
        self.event(format!("exec {place_id}"));
        if self.listed("boxNever", place_id) {
            std::future::pending::<()>().await;
        }
        if self.plan_reads_held.get() {
            self.plan_gate.notified().await;
        }
        Ok(self.w.borrow()["boxStdout"][place_id].as_str().unwrap_or("").to_string())
    }
    fn local_wired(&self) -> bool {
        self.w.borrow()["localWired"] == json!(true)
    }
    fn local_has_daemon(&self) -> bool {
        self.w.borrow()["daemonRoad"] == json!(true)
    }
    fn local_env(&self) -> IndexMap<String, String> {
        self.get("localEnv").as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect()
    }
    fn local_home(&self, agent: &str) -> String {
        self.w.borrow()["homes"][agent].as_str().unwrap().to_string()
    }
    async fn local_exec_lines(&self, script: &str, env: IndexMap<String, String>, idle_ms: i64, deadline_ms: i64) -> AccountResult<Vec<String>> {
        self.execs.borrow_mut().push(json!({ "where": "here", "script": script, "env": env, "options": { "idleMs": idle_ms, "deadlineMs": deadline_ms } }));
        self.event("exec here");
        if self.plan_reads_held.get() {
            self.plan_gate.notified().await;
        }
        Ok(self.get("localLines").as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect())
    }
    async fn read_logs_on(&self, place_id: &str, stores: &[UsageStore]) -> AccountResult<UsageLogsReply> {
        let frame = json!({ "op": "usage.logs", "stores": stores });
        self.asked.borrow_mut().push(json!({ "place": place_id, "frame": frame.to_string() }));
        let answer = self.get("logs")[place_id].clone();
        if answer["never"] == json!(true) {
            std::future::pending::<()>().await;
        }
        if let Some(e) = answer["error"].as_str() {
            return Err(plain(e).into());
        }
        Ok(serde_json::from_value(answer).unwrap())
    }
    async fn sys_history(&self, target: &ReadingsTarget, frame: Frame, origin: Option<&Caller>) -> AccountResult<Value> {
        self.histories.borrow_mut().push(json!({ "target": format!("{target:?}"), "frame": Value::Object(frame).to_string(), "origin": origin.is_some() }));
        self.history_answer.borrow().clone().map_err(|m| plain(&m).into())
    }
    async fn ready(&self) {
        self.ready_calls.set(self.ready_calls.get() + 1);
        self.event("ready");
    }
    fn wsp_sessions(&self) -> Vec<String> {
        self.get("sessions").as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
    }
    fn projects_on(&self, place_id: &str) -> Vec<(String, String)> {
        self.get("projects").as_array().unwrap().iter().filter(|p| p["computer"] == json!(place_id)).map(|p| (p["id"].as_str().unwrap().to_string(), p["path"].as_str().unwrap().to_string())).collect()
    }
    fn project_name(&self, id: &str) -> Option<String> {
        self.get("projects").as_array().unwrap().iter().find(|p| p["id"] == json!(id)).map(|p| p["name"].as_str().unwrap().to_string())
    }
    fn vaulted_for(&self, _agent: &str) -> Option<Value> {
        None
    }
    fn launch_of(&self, place_id: &str, agent: &str) -> (IndexMap<String, String>, Option<String>, Option<String>) {
        let setup = self.get("launch")[format!("{place_id}/{agent}")].clone();
        let env = setup["env"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect()).unwrap_or_default();
        (env, setup["configDir"].as_str().map(String::from), setup["launch"]["program"].as_str().map(String::from))
    }
    async fn login_home(&self, place_id: &str, agent: &str) -> Option<String> {
        let logins = self.place_values().iter().find(|p| p["id"] == json!(place_id)).and_then(|p| p["logins"].as_str().map(String::from))?;
        (agent == "codex").then(|| format!("{logins}/codex"))
    }
    async fn read_sign_ins_here(&self) -> AccountResult<Option<Vec<(String, String)>>> {
        let reader = self.get("signIns");
        if reader.is_null() {
            return Ok(None);
        }
        self.sign_in_reads.set(self.sign_in_reads.get() + 1);
        self.event("sign-ins read");
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        if reader == json!("fail") {
            return Err(plain("no agents").into());
        }
        Ok(Some(reader.as_array().unwrap().iter().map(|a| (a["id"].as_str().unwrap().to_string(), a["signIn"].as_str().unwrap().to_string())).collect()))
    }
    fn on_agents_changed(&self, f: Rc<dyn Fn()>) {
        self.changed.borrow_mut().push(f);
    }

    async fn limits(&self) -> AccountResult<Vec<Value>> {
        self.event("limits");
        Ok(self.get("ledgerLimits").as_array().cloned().unwrap_or_default())
    }
    async fn file_logs(&self, entries: Vec<UsageEntry>, read: Vec<String>) -> AccountResult<()> {
        if let Some(e) = self.get("fileError").as_str() {
            return Err(plain(e).into());
        }
        self.event("file_logs");
        self.filed.borrow_mut().push(json!({ "entries": entries.iter().map(entry_json).collect::<Vec<_>>(), "read": read }));
        Ok(())
    }
    async fn file_limit(&self, reading: LimitReading) -> AccountResult<(Value, Value)> {
        self.readings.borrow_mut().push(reading_json(&reading));
        self.event(format!("file_limit {}", reading.key));
        let after = json!({ "key": reading.key, "readAt": reading.at.map(|a| a as i64).unwrap_or(self.clock.get()) });
        if self.listed("stale", &reading.key) {
            Ok((after.clone(), after))
        } else {
            Ok((Value::Null, after))
        }
    }
    async fn alerts_read(&self, before: Value, after: Value) -> AccountResult<()> {
        self.alerts.borrow_mut().push(json!([before, after]));
        Ok(())
    }
    async fn used(&self, range: UsageRange, split: UsageSplit, outside: bool, labels: LabelOf) -> AccountResult<UsedAnswer> {
        let table = probe_labels(&self.get("labelProbe"), &*labels);
        self.used.borrow_mut().push(json!({ "range": range, "split": split, "outside": outside, "labels": table }));
        Ok(serde_json::from_value(json!({ "range": range, "split": split, "rows": [], "series": [], "since": 0, "until": 0 })).unwrap())
    }
    async fn account_labels(&self) -> AccountResult<IndexMap<String, String>> {
        Ok(self.get("accountLabels").as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect())
    }
    fn account_on_computer(&self, agent: &str, place_id: &str, computer_name: &str, limits: &[Value]) -> AccountKey {
        for l in limits {
            if l["agent"] == json!(agent) && l["road"] != json!("vault") && l["computers"].as_array().is_some_and(|c| c.contains(&json!(place_id))) {
                return AccountKey { key: l["key"].as_str().unwrap().into(), label: l["label"].as_str().unwrap().into(), road: l["road"].as_str().unwrap().into() };
            }
        }
        own_account(agent, place_id, computer_name)
    }
    fn account_of(&self, agent: &str, named: Option<&HarnessLimitAccount>, place_id: &str, computer_name: &str) -> AccountKey {
        match named {
            Some(n) => AccountKey { key: format!("{agent}:{}", n.id), label: n.label.clone().unwrap_or(n.id.clone()), road: "named".into() },
            None => own_account(agent, place_id, computer_name),
        }
    }
    fn agent_label(&self, agent: &str) -> String {
        agent_label(agent)
    }
    fn model_label(&self, model: &str) -> String {
        format!("model-label:{model}")
    }
    fn log_limit(&self, agent: &str, snapshot: &Value) -> Option<HarnessLimit> {
        if agent != "codex" {
            return None;
        }
        let seen = canon(snapshot);
        self.snapshots.borrow_mut().push(seen.clone());
        Some(serde_json::from_value(json!({ "windows": [], "plan": seen.to_string() })).unwrap())
    }
    fn plan_agents(&self) -> Vec<String> {
        vec!["codex".into()]
    }
    fn plan_read_command(&self, _agent: &str, road: &PlanRoad) -> String {
        let launch = road.launch_program.as_ref().map(|p| format!(",\"launch\":{{\"program\":{}}}", json!(p))).unwrap_or_default();
        assert!(road.env.is_empty(), "the road's env is always empty");
        format!("read {{\"home\":{},\"env\":{{}}{launch}}}", json!(road.home))
    }
    fn plan_parse(&self, _agent: &str, out: &str) -> Option<HarnessLimit> {
        out.strip_prefix("limit:").map(|j| serde_json::from_str(j).unwrap())
    }
    async fn account_rows(&self, limits: Vec<Value>, places: Vec<(String, String, Option<Value>)>, name_of: NameOf) -> AccountResult<AccountsAnswer> {
        self.event("account_rows");
        let listed: Vec<Value> = places
            .iter()
            .map(|(id, name, sign_ins)| {
                let mut o = Map::new();
                o.insert("id".into(), json!(id));
                o.insert("name".into(), json!(name));
                if let Some(s) = sign_ins {
                    o.insert("signIns".into(), s.clone());
                }
                Value::Object(o)
            })
            .collect();
        let names = probe_names(&self.get("nameProbe"), &*name_of);
        self.rows.borrow_mut().push(json!({ "limits": limits, "places": listed, "names": names }));
        Ok(AccountsAnswer { accounts: vec![] })
    }
    async fn account_names(&self, limits: Vec<Value>, name_of: NameOf) -> AccountResult<IndexMap<String, String>> {
        let names = probe_names(&self.get("nameProbe"), &*name_of);
        self.names.borrow_mut().push(json!({ "limits": limits, "names": names }));
        Ok(self.get("accountNames").as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect())
    }
    async fn kept_computer_names(&self, places: &[PlaceView]) -> AccountResult<IndexMap<String, String>> {
        self.event("kept names");
        let mut kept: IndexMap<String, String> = self.get("keptNames").as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        kept.insert("gone".into(), "old-box".into());
        for p in places {
            if p.id != "here" {
                kept.insert(p.id.clone(), p.name.clone());
            }
        }
        Ok(kept)
    }
    fn computer_name(&self, places: &[PlaceView], place_id: &str, kept: Option<&IndexMap<String, String>>) -> String {
        if let Some(p) = places.iter().find(|p| p.id == place_id) {
            return if p.kind == wsp_wire::views::place_kind::PlaceKind::Provider { "Solari".into() } else { p.name.clone() };
        }
        if place_id == "here" {
            return word("thisComputer");
        }
        kept.and_then(|k| k.get(place_id).cloned()).unwrap_or_else(|| word("removedComputer"))
    }
    async fn spend_reset(&self, account: &str, credit_id: Option<&str>, on: Option<&str>) -> Result<ResetAnswer, HostError> {
        let mut o = Map::new();
        o.insert("account".into(), json!(account));
        if let Some(c) = credit_id {
            o.insert("creditId".into(), json!(c));
        }
        if let Some(on) = on {
            o.insert("on".into(), json!(on));
        }
        self.spends.borrow_mut().push(Value::Object(o));
        self.spend_answer.borrow().clone()
    }
    fn nap_refused_reason(&self, _entry: &Entry) -> Option<String> {
        self.nap.borrow().clone()
    }
    fn stop_refused_reason(&self, _entry: &Entry) -> Option<String> {
        self.stop.borrow().clone()
    }
    fn disk_reason(&self, _entry: &Entry) -> Option<String> {
        self.disk.borrow().clone()
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
    fn warn(&self, line: &str) {
        self.warns.borrow_mut().push(line.to_string());
    }
}

fn area(w: &Rc<World>) -> Rc<UsageArea> {
    UsageArea::new(w.clone())
}

fn rec(phase: &str, place: Option<&str>, sub: Option<Value>) -> WorkspaceRecord {
    let mut r = json!({ "id": "w1", "name": "w", "kind": if place.is_some() { "place" } else { "cloud" }, "project": "p", "machineId": "m1", "phase": phase });
    if let Some(p) = place {
        r["place"] = json!(p);
    }
    if let Some(s) = sub {
        r["sub"] = s;
    }
    record(&r)
}

fn sub(cloud_gone: bool) -> Value {
    let mut s = json!({ "on": "solari", "repo": "acme/alpha", "base": { "branch": "main", "sha": "abc" }, "branch": "wsp/t", "folder": "/work/alpha" });
    if cloud_gone {
        s["cloudGone"] = json!(true);
    }
    s
}

fn place_view(w: &World, id: &str) -> ResetPlaceView {
    let views = w.views();
    let kind = if id == "here" {
        "here"
    } else if views.iter().any(|v| v.id == id && v.kind == wsp_wire::views::place_kind::PlaceKind::Computer) {
        "box"
    } else {
        "provider"
    };
    ResetPlaceView { id: id.into(), name: w.computer_name(&views, id, None), kind: kind.into(), connected: false }
}

// ---- U1, U2, U3 ---------------------------------------------------------------------------------------------

// TS: account/usage.ts:32-38
#[test]
fn u1_away_line_table() {
    run(async {
        let w = world("accounts_here_row_given");
        w.edit(|v| {
            v["places"] = json!([{ "id": "box1", "kind": "computer", "name": "studio" }]);
            v["away"] = json!(["box1"]);
        });
        let a = area(&w);
        let table = oracle("usage")["away"].clone();
        let got = json!({
            "cloud_gone": a.away_line(&rec("running", None, Some(sub(true)))),
            "cloud_gone_released": a.away_line(&rec("released", None, Some(sub(true)))),
            "cloud_gone_released_on_away_box": a.away_line(&rec("released", Some("box1"), Some(sub(true)))),
            "sub_not_gone": a.away_line(&rec("running", None, Some(sub(false)))),
            "away_box": a.away_line(&rec("running", Some("box1"), None)),
            "present_box": a.away_line(&rec("running", Some("box2"), None)),
            "here": a.away_line(&rec("running", None, None)),
        });
        assert_eq!(got, table);
        assert_eq!(got["cloud_gone"], json!(word("subGone")));
    });
}

// TS: account/usage.ts:41-44
#[test]
fn u2_row_reason_order() {
    run(async {
        let w = world("accounts_here_row_given");
        let a = area(&w);
        let entry = |refused: bool| {
            let mut r = json!({ "id": "w1", "name": "w", "kind": "cloud", "project": "p", "machineId": "m1", "phase": "running" });
            if refused {
                r["wakeRefused"] = json!("the record's refusal");
            }
            live(record(&r))
        };
        // The record's refusal outranks the nap, the stop and the disk reasons.
        *w.disk.borrow_mut() = Some("disk reason".into());
        *w.stop.borrow_mut() = Some("stop refusal".into());
        *w.nap.borrow_mut() = Some("nap refusal".into());
        assert_eq!(a.row_reason(&entry(true)), Some("the record's refusal".to_string()));
        // Without it: the nap refusal, then the stop refusal, then the disk reason, then nothing.
        let plain_entry = entry(false);
        assert_eq!(a.row_reason(&plain_entry), Some("nap refusal".to_string()));
        *w.nap.borrow_mut() = None;
        assert_eq!(a.row_reason(&plain_entry), Some("stop refusal".to_string()));
        *w.stop.borrow_mut() = None;
        assert_eq!(a.row_reason(&plain_entry), Some("disk reason".to_string()));
        *w.disk.borrow_mut() = None;
        assert_eq!(a.row_reason(&plain_entry), None);
        // The wake's own words come before the record's refusal, and the delete line before both.
        *w.nap.borrow_mut() = Some("nap refusal".into());
        let said = entry(true);
        said.borrow_mut().wake_said = Some("the wake said this".into());
        assert_eq!(a.row_reason(&said), Some("the wake said this".to_string()));
        said.borrow_mut().delete_said = Some(DeleteSaid { phase: WorkspacePhase::Running, line: "the delete line".into() });
        assert_eq!(a.row_reason(&said), Some("the delete line".to_string()));
        // A phase that moved since the sentence was said leaves it behind.
        said.borrow_mut().delete_said = Some(DeleteSaid { phase: WorkspacePhase::Napping, line: "the delete line".into() });
        assert_eq!(a.row_reason(&said), Some("the wake said this".to_string()));
    });
}

// TS: account/usage.ts:16-22
#[test]
fn u3_usage_stores_from_the_catalog() {
    let got = serde_json::to_value(UsageArea::usage_stores()).unwrap();
    assert_eq!(got, oracle("usage")["stores"]);
    assert_eq!(got.to_string(), oracle("usage")["stores"].to_string(), "keys in the order the daemon is sent them");
}

// ---- U4 ---------------------------------------------------------------------------------------------------

fn expect_logs(w: &World, name: &str) {
    let want = scenario(name);
    assert_eq!(Value::Array(w.asked.borrow().clone()), want["asked"].clone(), "frames asked of each computer in {name}");
    assert_eq!(Value::Array(w.filed.borrow().clone()).to_string(), want["filed"].to_string(), "entries and computers filed in {name}");
    assert_eq!(Value::Array(w.readings.borrow().clone()), canon(&want["readings"]), "limit readings filed in {name}");
    assert_eq!(Value::Array(w.alerts.borrow().clone()), canon(&want["alerts"]), "readings told to the alerts in {name}");
    assert_eq!(Value::Array(w.snapshots.borrow().clone()), canon(&want["snapshots"]), "snapshots the agents' readers were handed in {name}");
}

// TS: test/usage.test.ts:274, :308; test/usage-boxes.test.ts:48
#[test]
fn u4_logs_project_and_account() {
    run(async {
        let w = world("logs");
        let a = area(&w);
        a.read_logs().await.unwrap();
        settle().await;
        expect_logs(&w, "logs");
        assert!(w.warns.borrow().is_empty());
        let filed = w.filed.borrow()[0].clone();
        let entries = filed["entries"].as_array().unwrap();
        // The first project of that computer the folder is under; a folder under none, and a computer's own projects only.
        let projects: Vec<(&str, &str)> = entries.iter().map(|e| (e["session"].as_str().unwrap(), e["project"].as_str().unwrap())).collect();
        assert_eq!(projects, vec![("s-a", "pH"), ("s-b", ""), ("s-c", ""), ("s-d", "pB"), ("s-e", "")]);
        // An account a turn named stays the account; any other agent's work is the computer's own login.
        let accounts: Vec<(&str, &str)> = entries.iter().map(|e| (e["agent"].as_str().unwrap(), e["account"].as_str().unwrap())).collect();
        assert_eq!(accounts, vec![("claude", "claude@here"), ("codex", "codex:acct_1"), ("claude", "claude@here"), ("claude", "claude@box1"), ("claude", "claude@box1")]);
    });
}

// TS: test/usage.test.ts:361; test/usage-boxes.test.ts:83
#[test]
fn u4_logs_skip_wsp_sessions() {
    run(async {
        let w = world("logs");
        let a = area(&w);
        a.read_logs().await.unwrap();
        let filed = w.filed.borrow()[0].clone();
        let sessions: Vec<&str> = filed["entries"].as_array().unwrap().iter().map(|e| e["session"].as_str().unwrap()).collect();
        assert_eq!(sessions, vec!["s-a", "s-b", "s-c", "s-d", "s-e"]);
        assert!(!sessions.contains(&"sess-wsp-here"));
        // The sessions are read after the ledger's limits and the context is ready.
        let events = w.events.borrow().clone();
        let at = |what: &str| events.iter().position(|e| e == what).unwrap_or_else(|| panic!("{what} not in {events:?}"));
        assert!(at("limits") < at("ready"), "{events:?}");
        assert!(at("ready") < at("file_logs"), "{events:?}");
    });
}

#[test]
fn u4_targets_are_this_computer_then_linked_boxes() {
    run(async {
        // A box away (no link) and a provider with a link are not read; this computer is read where it has a daemon road.
        let w = world("logs");
        area(&w).read_logs().await.unwrap();
        let places: Vec<String> = w.asked.borrow().iter().map(|a| a["place"].as_str().unwrap().to_string()).collect();
        assert_eq!(places, vec!["here", "box1"]);
        let w = world("logs_without_a_daemon_here");
        area(&w).read_logs().await.unwrap();
        expect_logs(&w, "logs_without_a_daemon_here");
        assert_eq!(w.filed.borrow()[0]["read"], json!(["box1"]));
    });
}

// TS: test/usage.test.ts:274
#[test]
fn u4_logs_off_when_switched_off() {
    run(async {
        let w = world("logs_off");
        let a = area(&w);
        a.read_logs().await.unwrap();
        assert!(w.asked.borrow().is_empty());
        assert!(w.filed.borrow().is_empty());
        assert_eq!(w.ready_calls.get(), 0);
        // Nothing was stamped: the person turning it on is read at once.
        w.edit(|v| v["usageLogs"] = json!(true));
        a.read_logs().await.unwrap();
        assert_eq!(w.asked.borrow().len(), 2);
        assert_eq!(w.filed.borrow().len(), 1);
    });
}

#[test]
fn u4_logs_read_once_a_window() {
    run(async {
        let w = world("logs");
        let a = area(&w);
        let mut got = vec![];
        a.read_logs().await.unwrap();
        got.push(w.mark("0"));
        w.advance(59_999).await;
        a.read_logs().await.unwrap();
        got.push(w.mark("59999"));
        w.advance(1).await;
        a.read_logs().await.unwrap();
        got.push(w.mark("60000"));
        let mut want = scenario("logs_window")["marks"].as_array().unwrap().clone();
        for m in want.iter_mut() {
            m["warns"] = json!([]);
        }
        assert_eq!(Value::Array(got), Value::Array(want));
        expect_logs(&w, "logs_window");
    });
}

#[test]
fn u4_the_stamp_is_set_before_the_read() {
    run(async {
        // A read that failed everywhere still counts: the next ask inside the minute reads nothing.
        let w = world("logs_old_daemon_and_failures");
        let a = area(&w);
        a.read_logs().await.unwrap();
        a.read_logs().await.unwrap();
        assert_eq!(w.asked.borrow().len(), 2, "each computer asked once for the two reads");
    });
}

// TS: test/usage.test.ts (the old daemon's unknown op; the TS test map names it unreached)
#[test]
fn u4_an_old_daemon_is_quiet() {
    run(async {
        let w = world("logs_old_daemon_and_failures");
        let a = area(&w);
        a.read_logs().await.unwrap();
        // Only the box's failure is said, in the computer's own name; this computer's daemon named the unknown op.
        let want = scenario("logs_old_daemon_and_failures");
        assert_eq!(*w.warns.borrow(), vec!["the agent logs on studio were not read for usage: the socket closed".to_string()]);
        assert_eq!(Value::Array(w.warns.borrow().iter().map(|s| json!(s)).collect()), want["warns"]);
        assert!(w.filed.borrow().is_empty(), "no answers, nothing filed");
        assert_eq!(w.ready_calls.get(), 0);
        assert!(w.readings.borrow().is_empty());
        assert_eq!(word("unknownOp"), "unknown op: usage.logs");
    });
}

#[test]
fn u4_a_silent_computer_times_out_with_its_label() {
    run(async {
        let w = world("logs_timeout");
        let a = area(&w);
        let pending = {
            let a = a.clone();
            tokio::task::spawn_local(async move { a.read_logs().await })
        };
        settle().await;
        w.advance(29_999).await;
        let before = w.mark("before the bound");
        assert!(!pending.is_finished(), "the read waits on the silent computer until its bound");
        w.advance(1).await;
        pending.await.unwrap().unwrap();
        let after = w.mark("after the bound");
        let want = scenario("logs_timeout");
        assert_eq!(vec![before, after], want["marks"].as_array().unwrap().clone());
        assert_eq!(*w.warns.borrow(), vec!["the agent logs on studio were not read for usage: usage.logs on studio was not answered in 30s".to_string()]);
        // The computer that answered is filed alone.
        assert_eq!(Value::Array(w.filed.borrow().clone()).to_string(), want["filed"].to_string());
    });
}

#[test]
fn u4_a_read_that_filed_nothing_is_not_filed() {
    run(async {
        // Every computer failed: nothing is filed and no reading is told to the alerts.
        let w = world("logs_old_daemon_and_failures");
        area(&w).read_logs().await.unwrap();
        assert!(w.filed.borrow().is_empty() && w.readings.borrow().is_empty() && w.alerts.borrow().is_empty());
    });
}

#[test]
fn u4_a_stale_reading_is_not_told_to_the_alerts() {
    run(async {
        // codex@box1's reading is the ledger's own already (before equals after): filed, not told. The other one is told.
        let w = world("logs");
        area(&w).read_logs().await.unwrap();
        assert_eq!(w.readings.borrow().len(), 2);
        assert_eq!(*w.alerts.borrow(), vec![json!([null, { "key": "codex:acct_1", "readAt": 5000 }])]);
    });
}

// ---- U5, U6 -------------------------------------------------------------------------------------------------

// TS: test/usage.test.ts:207, :231
#[test]
fn u5_sign_ins_held_a_minute_and_dropped_on_change() {
    run(async {
        let w = world("sign_ins_hold");
        let a = area(&w);
        let mut got = vec![];
        let calls: Vec<_> = (0..40).map(|_| a.here_sign_ins()).collect();
        let all = futures::future::join_all(calls).await;
        let kept: IndexMap<String, String> = [("claude".to_string(), "signed-in".to_string()), ("codex".to_string(), "vault-key".to_string())].into_iter().collect();
        assert!(all.iter().all(|r| *r == kept), "only signed-in and vault-key are kept, in the reader's order: {:?}", all[0]);
        got.push(w.mark("40 at once"));
        w.advance(59_999).await;
        assert_eq!(a.here_sign_ins().await, kept);
        got.push(w.mark("59999"));
        w.advance(1).await;
        a.here_sign_ins().await;
        got.push(w.mark("60000"));
        let listeners = w.changed.borrow().clone();
        assert_eq!(listeners.len(), 1, "the area listens for agents.changed once");
        listeners[0]();
        a.here_sign_ins().await;
        got.push(w.mark("agents changed"));
        let want: Vec<Value> = scenario("sign_ins_hold")["marks"].as_array().unwrap().clone();
        let counts = |v: &[Value]| v.iter().map(|m| (m["label"].clone(), m["signInReads"].clone())).collect::<Vec<_>>();
        assert_eq!(counts(&got), counts(&want));
    });
}

#[test]
fn u5_a_failed_or_absent_read_lists_none_and_is_held() {
    run(async {
        let w = world("sign_ins_failed_read_held");
        let a = area(&w);
        assert!(a.here_sign_ins().await.is_empty());
        assert!(a.here_sign_ins().await.is_empty());
        assert_eq!(w.sign_in_reads.get(), 1, "a failed read is held like any other");
        w.advance(60_000).await;
        assert!(a.here_sign_ins().await.is_empty());
        assert_eq!(w.sign_in_reads.get(), 2);
        let w = world("sign_ins_no_reader");
        assert!(area(&w).here_sign_ins().await.is_empty());
        assert_eq!(w.sign_in_reads.get(), 0);
    });
}

// TS: test/usage.test.ts:182
#[test]
fn u6_accounts_add_this_computer() {
    run(async {
        for name in ["accounts_here_row_added", "accounts_here_row_given", "accounts_here_row_has_its_own"] {
            let w = world(name);
            let a = area(&w);
            a.usage_accounts().await.unwrap();
            let got = w.rows.borrow().clone();
            assert_eq!(Value::Array(got).to_string(), Value::Array(scenario(name)["rows"].as_array().unwrap().clone()).to_string(), "the rows asked for in {name}");
            assert_eq!(w.sign_in_reads.get(), 1);
        }
        // The names the namer gives: a computer listed, this computer where no row is listed, and one kept after its removal.
        let w = world("accounts_here_row_added");
        area(&w).usage_accounts().await.unwrap();
        assert_eq!(w.rows.borrow()[0]["names"], json!(["this computer", "studio", "old-box"]));
        // The names are kept before the sign-ins are read, and the limits come last.
        let events = w.events.borrow().clone();
        assert_eq!(events, vec!["places", "kept names", "sign-ins read", "limits", "account_rows"]);
    });
}

// ---- U7, U8 -------------------------------------------------------------------------------------------------

// TS: test/usage-reset-joined.test.ts:18; test/usage.test.ts:494
#[test]
fn u7_reset_run_box_and_here() {
    run(async {
        let w = world("reset_pieces");
        let a = area(&w);
        let want = scenario("reset_pieces");
        let mut got = Map::new();
        got.insert("box".into(), json!(a.reset_run(&place_view(&w, "box1"), "echo hi", "codex").await.unwrap()));
        got.insert("here".into(), json!(a.reset_run(&place_view(&w, "here"), "echo hi", "codex").await.unwrap()));
        got.insert("box_no_path".into(), json!(a.reset_run(&place_view(&w, "box5"), "echo hi", "codex").await.unwrap()));
        assert_eq!(Value::Object(got), want["runs"]);
        assert_eq!(*w.execs.borrow(), want["execs"].as_array().unwrap().clone());
        // The variables the person set ride the input, never the script's own text.
        let token = "sk-test-0123";
        assert!(w.execs.borrow()[0]["stdin"].as_str().unwrap().contains(token));
        assert!(!w.execs.borrow()[0]["script"].as_str().unwrap().contains(token));
        // On this computer the spawn's environment holds PATH, HOME and the launch variables, never a thread's own.
        let here = w.execs.borrow()[1].clone();
        assert_eq!(here["env"].to_string(), r#"{"PATH":"/usr/local/bin:/usr/bin","HOME":"/home/u","CODEX_GATEWAY_TOKEN":"sk-test-0123"}"#);

        // No door: the box branch refuses in the runtime's own words, running nothing.
        let w = world("reset_pieces");
        let a = area(&w);
        let at = place_view(&w, "box1");
        w.door.set(false);
        let e = a.reset_run(&at, "echo hi", "codex").await.unwrap_err();
        assert_eq!(said(&e), word("noPlaceDoor"));
        assert!(w.execs.borrow().is_empty());

        // This computer unwired: the computer is named as away.
        let w = world("reset_here_absent");
        let a = area(&w);
        let here = place_view(&w, "here");
        let e = a.reset_run(&here, "echo hi", "codex").await.unwrap_err();
        assert_eq!(said(&e), scenario("reset_here_absent")["trys"]["run"].as_str().unwrap());
        assert_eq!(said(&e), word("absentThis").replace("this computer", "this Mac"));
        assert!(w.execs.borrow().is_empty());
    });
}

// TS: test/places-settings.test.ts:515
#[test]
fn u8_plan_road_table() {
    run(async {
        let w = world("reset_pieces");
        let a = area(&w);
        let want = scenario("reset_pieces")["roads"].clone();
        let mut got = Map::new();
        for (name, agent, id) in [
            ("here", "codex", "here"),
            ("here_config_dir", "claude", "here"),
            ("box_logins", "codex", "box1"),
            ("box_config_dir_and_args_only", "codex", "box3"),
            ("box_logins_and_program", "codex", "box4"),
            ("box_no_logins", "codex", "box2"),
            ("provider_no_logins", "codex", "solari"),
        ] {
            let at = a.plan_place(id).await;
            let answer = match a.plan_road(agent, &at).await {
                Ok(r) => {
                    let mut o = Map::new();
                    o.insert("home".into(), json!(r.home));
                    o.insert("env".into(), json!(r.env));
                    if let Some(p) = r.launch_program {
                        o.insert("launch".into(), json!({ "program": p }));
                    }
                    Value::Object(o)
                }
                Err(e) => json!({ "error": e.message }),
            };
            got.insert(name.into(), answer);
        }
        assert_eq!(Value::Object(got).to_string(), want.to_string());
        assert_eq!(want["box_no_logins"]["error"], json!(word("noLoginsMini")));
        // This computer unwired: its road is a refusal naming it.
        let w = world("reset_here_absent");
        let a = area(&w);
        let here = a.plan_place("here").await;
        let e = a.plan_road("codex", &here).await.unwrap_err();
        assert_eq!(e.message, scenario("reset_here_absent")["trys"]["road"].as_str().unwrap());
    });
}

#[test]
fn u8_plan_places_table() {
    run(async {
        let w = world("reset_pieces");
        let a = area(&w);
        let want = scenario("reset_pieces")["placeTable"].clone();
        let mut got = Map::new();
        for id in ["here", "box1", "box2", "box3", "solari", "unknown"] {
            let p = a.plan_place(id).await;
            got.insert(id.into(), json!({ "id": p.id, "name": p.name, "kind": p.kind, "connected": p.connected }));
        }
        let mut expect = Map::new();
        for (id, v) in want.as_object().unwrap() {
            expect.insert(id.clone(), json!({ "id": v["id"], "name": v["name"], "kind": v["kind"], "connected": v["connected"] }));
        }
        assert_eq!(Value::Object(got), Value::Object(expect));
        // A provider is never connected, even where a link to it stands; this computer is connected where it is wired.
        let w = world("reset_here_absent");
        let here = area(&w).plan_place("here").await;
        assert!(!here.connected && here.kind == "here" && here.name == "this Mac");
    });
}

// ---- U9 ----------------------------------------------------------------------------------------------------

fn expect_plans(w: &World, name: &str) {
    let want = scenario(name);
    assert_eq!(sorted(&w.execs.borrow()), sorted(want["execs"].as_array().unwrap()), "scripts run in {name}");
    assert_eq!(sorted(&w.readings.borrow()), sorted(want["readings"].as_array().unwrap()), "readings filed in {name}");
    assert_eq!(sorted(&w.alerts.borrow()), sorted(want["alerts"].as_array().unwrap()), "readings told to the alerts in {name}");
    let mut warns = w.warns.borrow().clone();
    warns.sort();
    let mut want_warns: Vec<String> = want["warns"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
    want_warns.sort();
    assert_eq!(warns, want_warns, "warnings in {name}");
}

// TS: test/usage-reset-joined.test.ts:52
#[test]
fn u9_plans_read_once_a_window() {
    run(async {
        let w = world("plans");
        let a = area(&w);
        let mut got = vec![];
        for (label, wait) in [("first", 0), ("second", 0), ("59999", 59_999), ("60000", 1)] {
            w.advance(wait).await;
            a.accounts(true).await.unwrap();
            got.push(w.mark(label));
        }
        a.accounts(false).await.unwrap();
        got.push(w.mark("not fresh"));
        w.compare_marks("plans", &got);
        expect_plans(&w, "plans");
        // Every plan reading is filed without a moment of its own, the ledger stamping its clock; a named account's key.
        assert!(w.readings.borrow().iter().all(|r| r.get("at").is_none()));
        assert!(w.readings.borrow().iter().any(|r| r["key"] == json!("codex:acct_7f3a") && r["label"] == json!("dev@example.com") && r["road"] == json!("named")));
        // Told to the alerts every time, with no comparison.
        assert_eq!(w.alerts.borrow().len(), 6);
    });
}

#[test]
fn u9_plans_skip_a_vault_key_here() {
    run(async {
        let w = world("plans_here_vault_key");
        area(&w).accounts(true).await.unwrap();
        w.compare_marks("plans_here_vault_key", &[w.mark("vault key")]);
        assert!(w.execs.borrow().is_empty());
    });
}

#[test]
fn u9_a_failed_plan_read_warns_and_the_rest_go_on() {
    run(async {
        let w = world("plans_exec_error");
        area(&w).accounts(true).await.unwrap();
        expect_plans(&w, "plans_exec_error");
        assert_eq!(w.warns.borrow().len(), 2);
        // A reading that parses to nothing files nothing and warns of nothing.
        let w = world("plans_failures");
        area(&w).accounts(true).await.unwrap();
        expect_plans(&w, "plans_failures");
        assert!(w.readings.borrow().is_empty());
    });
}

#[test]
fn u9_a_silent_plan_read_times_out_with_its_label() {
    run(async {
        let w = world("plans_timeout");
        let a = area(&w);
        let pending = {
            let a = a.clone();
            tokio::task::spawn_local(async move { a.accounts(true).await })
        };
        settle().await;
        w.advance(29_999).await;
        let before = w.mark("before the bound");
        assert!(!pending.is_finished());
        w.advance(1).await;
        pending.await.unwrap().unwrap();
        let after = w.mark("after the bound");
        w.compare_marks("plans_timeout", &[before, after]);
        expect_plans(&w, "plans_timeout");
        assert!(w.warns.borrow().contains(&"Codex's limits on studio were not read: codex limits on studio was not answered in 30s".to_string()));
    });
}

#[test]
fn u9_one_plan_read_is_shared_while_it_is_out() {
    run(async {
        let w = world("plans");
        w.plan_reads_held.set(true);
        let a = area(&w);
        let first = {
            let a = a.clone();
            tokio::task::spawn_local(async move { a.read_plans_now().await })
        };
        settle().await;
        assert_eq!(w.execs.borrow().len(), 3, "one read per signed-in connected computer");
        let second = {
            let a = a.clone();
            tokio::task::spawn_local(async move { a.read_plans_now().await })
        };
        settle().await;
        assert!(!first.is_finished() && !second.is_finished(), "the second asker waits for the read that is out");
        assert_eq!(w.execs.borrow().len(), 3);
        for _ in 0..3 {
            w.plan_gate.notify_one();
        }
        settle().await;
        first.await.unwrap();
        second.await.unwrap();
        assert_eq!(w.execs.borrow().len(), 3);
        // Done: the read's slot is empty again, and the minute holds a third ask off.
        a.read_plans_now().await;
        assert_eq!(w.execs.borrow().len(), 3);
        w.advance(60_000).await;
        w.plan_reads_held.set(false);
        a.read_plans_now().await;
        assert_eq!(w.execs.borrow().len(), 6);
    });
}

// ---- U10 ---------------------------------------------------------------------------------------------------

// TS: test/usage.test.ts:466; test/places-settings.test.ts:515
#[test]
fn u10_reset_names_the_place() {
    run(async {
        let w = world("reset_names_the_place");
        let a = area(&w);
        a.reset("codex@here", None, Some("srv")).await.unwrap();
        a.reset("codex@here", Some("c1"), Some("nowhere")).await.unwrap();
        let answer = a.reset("codex@here", None, None).await.unwrap();
        assert_eq!(Value::Array(w.spends.borrow().clone()).to_string(), Value::Array(scenario("reset_names_the_place")["spends"].as_array().unwrap().clone()).to_string());
        // A word is read as a place only where the person named one; an unknown one is this computer.
        assert_eq!(*w.place_for_words.borrow(), vec!["srv".to_string(), "nowhere".to_string()]);
        assert_eq!(serde_json::to_value(&answer).unwrap(), json!({ "outcome": "reset", "said": "ok" }));
        // A refusal passes through as it was.
        *w.spend_answer.borrow_mut() = Err(plain("codex@here is away"));
        let e = a.reset("codex@here", None, None).await.unwrap_err();
        assert_eq!(error_json(&e), json!({ "message": "codex@here is away" }));
        // No door: the word is this computer.
        let w = world("reset_names_the_place");
        w.door.set(false);
        area(&w).reset("codex@here", None, Some("srv")).await.unwrap();
        assert_eq!(*w.spends.borrow(), vec![json!({ "account": "codex@here", "on": "here" })]);
    });
}

// ---- U11 ---------------------------------------------------------------------------------------------------

// TS: test/usage.test.ts:146, :152
#[test]
fn u11_used_labels_table() {
    run(async {
        let w = world("used_labels");
        let a = area(&w);
        a.used(UsageRange::Week, UsageSplit::Agent, None).await.unwrap();
        let want = scenario("used_labels");
        assert_eq!(Value::Array(w.used.borrow().clone()), canon(&Value::Array(want["used"].as_array().unwrap().clone())));
        assert_eq!(Value::Array(w.names.borrow().clone()), want["names"]);
        assert_eq!(w.names.borrow()[0]["limits"], json!([]), "the ledger's limits are the names' input");
        assert!(w.asked.borrow().is_empty(), "no logs read for an ask that did not want them");
        // The model split is the context's own label.
        let labels = |split: UsageSplit, v: &str| a.usage_label(&|id: &str| format!("computer:{id}"), &IndexMap::new(), split, v);
        assert_eq!(labels(UsageSplit::Model, "claude-opus-5"), "model-label:claude-opus-5");
        assert_eq!(labels(UsageSplit::Computer, "box1"), "computer:box1");
        assert_eq!(labels(UsageSplit::Source, "log"), word("outsideWsp"));
        assert_eq!(labels(UsageSplit::Source, "wsp"), word("wspThreads"));
        assert_eq!(labels(UsageSplit::Project, ""), "No project");
        assert_eq!(labels(UsageSplit::Account, "k9"), "k9");
        let accounts: IndexMap<String, String> = [("k9".to_string(), "Nine".to_string())].into_iter().collect();
        assert_eq!(a.usage_label(&|id: &str| id.to_string(), &accounts, UsageSplit::Account, "k9"), "Nine");
    });
}

// TS: test/usage.test.ts:331
#[test]
fn u11_logs_are_read_where_outside_or_source_asks() {
    run(async {
        for (name, range_split, outside) in [("used_flag_outside", UsageSplit::Agent, Some(true)), ("used_flag_source", UsageSplit::Source, None), ("used_flag_outside_false", UsageSplit::Agent, Some(false)), ("logs_not_asked", UsageSplit::Project, None)] {
            let w = world(name);
            area(&w).used(UsageRange::Day, range_split, outside).await.unwrap();
            let want = scenario(name);
            let got = w.used.borrow().clone();
            assert_eq!(got.len(), 1);
            assert_eq!(got[0]["outside"], want["used"][0]["outside"], "{name}");
            assert_eq!(got[0]["split"], want["used"][0]["split"], "{name}");
            let mark = w.mark("m");
            assert_eq!(mark["asked"], want["marks"][0]["asked"], "{name}");
            assert_eq!(mark["filed"], want["marks"][0]["filed"], "{name}");
        }
        // Turned off: nothing is read and `outside` reaches the ledger as false, whatever the door asked.
        let w = world("used_outside_off");
        area(&w).used(UsageRange::Day, UsageSplit::Source, Some(true)).await.unwrap();
        let want = scenario("used_outside_off");
        assert_eq!(w.used.borrow()[0]["outside"], json!(false));
        assert_eq!(want["used"][0]["outside"], json!(false));
        assert!(w.asked.borrow().is_empty());
    });
}

#[test]
fn u11_a_logs_failure_is_warned_and_used_answers() {
    run(async {
        let w = world("used_logs_failure");
        let answer = area(&w).used(UsageRange::Day, UsageSplit::Agent, Some(true)).await;
        assert!(answer.is_ok());
        let want = scenario("used_logs_failure");
        assert_eq!(*w.warns.borrow(), vec!["the agent logs were not read for usage: the ledger is gone".to_string()]);
        assert_eq!(Value::Array(w.warns.borrow().iter().map(|s| json!(s)).collect()), want["warns"]);
        assert_eq!(w.used.borrow().len(), 1);
        assert_eq!(w.used.borrow()[0]["outside"], json!(true));
        assert!(w.filed.borrow().is_empty());
    });
}

// ---- U12 ---------------------------------------------------------------------------------------------------

// TS: test/usage.test.ts:406, :415, :422
#[test]
fn u12_readings_frame_and_answer() {
    run(async {
        let w = world("accounts_here_row_given");
        let a = area(&w);
        let now = w.clock.get();
        let point = json!({ "at": now - 600_000, "cpu": 12.5, "load1": 0.3, "mem": { "used": 1, "total": 4 }, "disk": { "used": 2, "total": 8 } });
        // The range's span and step, as READINGS_SPAN_MS and READINGS_STEP_MS write them, in the frame's own key order.
        for (range, span, step) in [(ReadingsRange::V15m, 900_000i64, 15_000i64), (ReadingsRange::Day, 86_400_000, 300_000), (ReadingsRange::Week, 604_800_000, 1_800_000), (ReadingsRange::Month, 2_592_000_000, 7_200_000)] {
            w.histories.borrow_mut().clear();
            *w.history_answer.borrow_mut() = Ok(json!({ "points": [point], "stepMs": 600_000, "truncated": false }));
            let origin = Caller::Road(WorkspaceOrigin::Here);
            let answer = a.readings(ReadingsTarget::Workspace("w1".into()), range, Some(&origin)).await.unwrap();
            let frame = format!(r#"{{"op":"sys.history","from":{},"to":{},"stepMs":{}}}"#, now - span, now, step);
            assert_eq!(*w.histories.borrow(), vec![json!({ "target": r#"Workspace("w1")"#, "frame": frame, "origin": true })]);
            // The answer carries the daemon's own step, the points as it sent them, and the range asked.
            assert_eq!(canon(&serde_json::to_value(&answer).unwrap()), canon(&json!({ "points": [point], "stepMs": 600_000, "from": now - span, "to": now })));
            assert_eq!(serde_json::to_value(&answer).unwrap().as_object().unwrap().keys().collect::<Vec<_>>(), vec!["points", "stepMs", "from", "to"]);
        }
        // A target that is a computer, and no origin.
        w.histories.borrow_mut().clear();
        a.readings(ReadingsTarget::Place("here".into()), ReadingsRange::Day, None).await.unwrap();
        assert_eq!(w.histories.borrow()[0]["target"], json!(r#"Place("here")"#));
        assert_eq!(w.histories.borrow()[0]["origin"], json!(false));
        // A daemon's refusal is handed on as it said it.
        *w.history_answer.borrow_mut() = Err("unknown op: sys.history".into());
        let e = a.readings(ReadingsTarget::Workspace("w1".into()), ReadingsRange::Day, None).await.unwrap_err();
        assert_eq!(said(&e), "unknown op: sys.history");
    });
}

// ---- U13 ---------------------------------------------------------------------------------------------------

// TS: test/usage-reset-joined.test.ts:52
#[test]
fn u13_fresh_accounts_read_the_plans() {
    run(async {
        let w = world("plans");
        let a = area(&w);
        a.accounts(false).await.unwrap();
        assert!(w.execs.borrow().is_empty() && w.readings.borrow().is_empty(), "a plain ask reads no plan");
        a.accounts(true).await.unwrap();
        assert_eq!(w.execs.borrow().len(), 3);
        // The readings are filed before the rows are asked for.
        let events = w.events.borrow().clone();
        let rows_at = events.iter().rposition(|e| e == "account_rows").unwrap();
        let last_filed = events.iter().rposition(|e| e.starts_with("file_limit")).unwrap();
        assert!(last_filed < rows_at, "{events:?}");
        assert_eq!(w.rows.borrow().len(), 2);
        // Asked again inside the minute: no new read, the rows still come.
        a.accounts(true).await.unwrap();
        assert_eq!(w.execs.borrow().len(), 3);
        assert_eq!(w.rows.borrow().len(), 3);
    });
}
