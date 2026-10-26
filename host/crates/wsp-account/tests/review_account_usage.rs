// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the usage area's reads of computers: how long a read may run and which computers a plan read asks. Expected
//! values are the TS's, printed by node (`oracle/zz-review-account-usage.oracle.ts`, text and output in the review report).
#![allow(dead_code, unused_imports)]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Value};

use common::{plain, run, settle};
use wsp_account::account::core_types::{AccountResult, Entry, Frame};
use wsp_account::account::usage::{AccountKey, LabelOf, LimitReading, NameOf, PlanRoad, ResetPlaceView, UsageArea, UsageCtx, UsageEntry};
use wsp_core::clock::{Cancel, Clock, FakeClock};
use wsp_core::types::wiring::{ReadingsTarget, WorkspaceRecord};
use wsp_frames::{UsageLogsReply, UsageStore};
use wsp_wire::types::usage::{
    AccountsAnswer, HarnessLimit, HarnessLimitAccount, ReadingsRange, ResetAnswer, UsageRange, UsageSplit, UsedAnswer,
};
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

fn view(id: &str, name: &str) -> PlaceView {
    serde_json::from_value(json!({ "id": id, "kind": "computer", "name": name, "default": false })).unwrap()
}

/// The smallest context the two reads need: this computer and linked boxes, a read that answers after `slow_ms` of the
/// fake clock, and the lists a place door answers call by call (the last one repeats).
struct Bare {
    clock: FakeClock,
    warns: RefCell<Vec<String>>,
    closed: RefCell<Vec<i64>>,
    slow_ms: i64,
    lists: Vec<Vec<PlaceView>>,
    list_calls: Cell<usize>,
    box_execs: RefCell<Vec<String>>,
}

impl Bare {
    fn new(lists: Vec<Vec<PlaceView>>, slow_ms: i64) -> Rc<Bare> {
        Rc::new(Bare {
            clock: FakeClock::new(0),
            warns: RefCell::default(),
            closed: RefCell::default(),
            slow_ms,
            lists,
            list_calls: Cell::new(0),
            box_execs: RefCell::default(),
        })
    }
    async fn advance(&self, ms: i64) {
        self.clock.advance(ms);
        settle().await;
    }
}

#[async_trait(?Send)]
impl UsageCtx for Bare {
    fn now(&self) -> i64 {
        self.clock.now()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(f, ms)
    }
    async fn usage_logs_on(&self) -> bool {
        true
    }
    async fn places(&self, _now: i64) -> Vec<PlaceView> {
        let n = self.list_calls.get();
        self.list_calls.set(n + 1);
        self.lists.get(n).or(self.lists.last()).cloned().unwrap_or_default()
    }
    fn has_door(&self) -> bool {
        true
    }
    fn place_away(&self, _place_id: &str) -> bool {
        false
    }
    fn name_of(&self, place_id: &str) -> String {
        place_id.to_string()
    }
    fn linked(&self, place_id: &str) -> bool {
        place_id != "here"
    }
    fn signed_in_at(&self, _place_id: &str, _agent: &str) -> bool {
        true
    }
    async fn place_for(&self, _word: &str) -> Option<String> {
        None
    }
    async fn box_login_path(&self, _place_id: &str) -> Option<String> {
        None
    }
    async fn box_exec(&self, place_id: &str, _script: &str, _timeout_ms: i64, _stdin: &str) -> AccountResult<String> {
        self.box_execs.borrow_mut().push(place_id.to_string());
        Ok("nothing".to_string())
    }
    fn local_wired(&self) -> bool {
        true
    }
    fn local_has_daemon(&self) -> bool {
        true
    }
    fn local_env(&self) -> IndexMap<String, String> {
        IndexMap::new()
    }
    fn local_home(&self, _agent: &str) -> String {
        "/home/u/.codex".to_string()
    }
    async fn local_exec_lines(
        &self,
        _script: &str,
        _env: IndexMap<String, String>,
        _idle: i64,
        _deadline: i64,
    ) -> AccountResult<Vec<String>> {
        Ok(vec!["nothing".to_string()])
    }
    async fn read_logs_on(&self, _place_id: &str, _stores: &[UsageStore]) -> AccountResult<UsageLogsReply> {
        let gate = Rc::new(Cell::new(false));
        let opened = gate.clone();
        self.clock.schedule(Box::new(move || opened.set(true)), self.slow_ms);
        while !gate.get() {
            tokio::task::yield_now().await;
        }
        self.closed.borrow_mut().push(self.clock.now());
        Ok(serde_json::from_value(json!({ "rows": [], "limits": [] })).unwrap())
    }
    async fn sys_history(&self, _target: &ReadingsTarget, _frame: Frame, _origin: Option<&Caller>) -> AccountResult<Value> {
        unreachable!()
    }
    async fn ready(&self) {}
    fn wsp_sessions(&self) -> Vec<String> {
        vec![]
    }
    fn projects_on(&self, _place_id: &str) -> Vec<(String, String)> {
        vec![]
    }
    fn project_name(&self, _id: &str) -> Option<String> {
        None
    }
    fn vaulted_for(&self, _agent: &str) -> Option<Value> {
        None
    }
    fn launch_of(&self, _place_id: &str, _agent: &str) -> (IndexMap<String, String>, Option<String>, Option<String>) {
        (IndexMap::new(), None, None)
    }
    async fn login_home(&self, _place_id: &str, _agent: &str) -> Option<String> {
        Some("/home/maya/logins/.codex".to_string())
    }
    async fn read_sign_ins_here(&self) -> AccountResult<Option<Vec<(String, String)>>> {
        Ok(Some(vec![("codex".to_string(), "signed-in".to_string())]))
    }
    fn on_agents_changed(&self, _f: Rc<dyn Fn()>) {}
    async fn limits(&self) -> AccountResult<Vec<Value>> {
        Ok(vec![])
    }
    async fn file_logs(&self, _entries: Vec<UsageEntry>, _read: Vec<String>) -> AccountResult<()> {
        Ok(())
    }
    async fn file_limit(&self, _reading: LimitReading) -> AccountResult<(Value, Value)> {
        Ok((Value::Null, Value::Null))
    }
    async fn alerts_read(&self, _before: Value, _after: Value) -> AccountResult<()> {
        Ok(())
    }
    async fn used(&self, _range: UsageRange, _split: UsageSplit, _outside: bool, _labels: LabelOf) -> AccountResult<UsedAnswer> {
        unreachable!()
    }
    async fn account_labels(&self) -> AccountResult<IndexMap<String, String>> {
        Ok(IndexMap::new())
    }
    fn account_on_computer(&self, agent: &str, place_id: &str, computer_name: &str, _limits: &[Value]) -> AccountKey {
        AccountKey { key: format!("{agent}@{place_id}"), label: format!("{agent} on {computer_name}"), road: "own".into() }
    }
    fn account_of(&self, agent: &str, _named: Option<&HarnessLimitAccount>, place_id: &str, name: &str) -> AccountKey {
        AccountKey { key: format!("{agent}@{place_id}"), label: format!("{agent} on {name}"), road: "own".into() }
    }
    fn agent_label(&self, agent: &str) -> String {
        agent.to_string()
    }
    fn model_label(&self, model: &str) -> String {
        model.to_string()
    }
    fn log_limit(&self, _agent: &str, _snapshot: &Value) -> Option<HarnessLimit> {
        None
    }
    fn plan_agents(&self) -> Vec<String> {
        vec!["codex".to_string()]
    }
    fn plan_read_command(&self, _agent: &str, _road: &PlanRoad) -> String {
        "read".to_string()
    }
    fn plan_parse(&self, _agent: &str, _out: &str) -> Option<HarnessLimit> {
        None
    }
    async fn account_rows(
        &self,
        _limits: Vec<Value>,
        _places: Vec<(String, String, Option<Value>)>,
        _name_of: NameOf,
    ) -> AccountResult<AccountsAnswer> {
        Ok(AccountsAnswer { accounts: vec![] })
    }
    async fn account_names(&self, _limits: Vec<Value>, _name_of: NameOf) -> AccountResult<IndexMap<String, String>> {
        Ok(IndexMap::new())
    }
    async fn kept_computer_names(&self, _places: &[PlaceView]) -> AccountResult<IndexMap<String, String>> {
        Ok(IndexMap::new())
    }
    fn computer_name(&self, places: &[PlaceView], place_id: &str, _kept: Option<&IndexMap<String, String>>) -> String {
        places.iter().find(|p| p.id == place_id).map(|p| p.name.clone()).unwrap_or_else(|| place_id.to_string())
    }
    async fn spend_reset(&self, _account: &str, _credit_id: Option<&str>, _on: Option<&str>) -> Result<ResetAnswer, HostError> {
        unreachable!()
    }
    fn nap_refused_reason(&self, _entry: &Entry) -> Option<String> {
        None
    }
    fn stop_refused_reason(&self, _entry: &Entry) -> Option<String> {
        None
    }
    fn disk_reason(&self, _entry: &Entry) -> Option<String> {
        None
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
    fn warn(&self, line: &str) {
        self.warns.borrow_mut().push(line.to_string());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-B1. helpers.ts:56-71 `bounded` and usage.ts:100-118. `bounded(work, ms, what)` is a race: when the timer wins it
// rejects, and the read it bounded runs on to its own end, where `overChannel`'s `finally` closes the channel it opened.
// usage.rs:340 `select(work, timer)` drops the read when the timer wins, so whatever the read does after its last await
// (close its channel) never runs: a channel opened for a read that took more than 30 s is left open, once a minute for as
// long as the computer is slow. Node (zz-review-account-usage.oracle.ts "probe: a read that outlives its bound"): at 30 s
// the warning, nothing closed; at 40 s the read's own end ran: closed at 40000.

#[test]
fn a_log_read_that_outlives_its_bound_still_runs_to_its_end() {
    run(async {
        let ctx = Bare::new(vec![vec![view("here", "this Mac")]], 40_000);
        let area = UsageArea::new(ctx.clone());
        let reading = area.clone();
        let pending = tokio::task::spawn_local(async move { reading.read_logs().await });
        settle().await;
        ctx.advance(29_999).await;
        assert!(ctx.warns.borrow().is_empty() && ctx.closed.borrow().is_empty());
        ctx.advance(1).await;
        pending.await.unwrap().unwrap();
        assert_eq!(
            *ctx.warns.borrow(),
            vec!["the agent logs on this Mac were not read for usage: usage.logs on this Mac was not answered in 30s".to_string()]
        );
        assert!(ctx.closed.borrow().is_empty());
        ctx.advance(10_000).await;
        assert_eq!(*ctx.closed.borrow(), vec![40_000], "node: the read ran to its end at 40 s; the Rust dropped it at 30 s");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-B2. usage.ts:228-238 and :270-279. `planPlaces()` reads the places list once and the `placeOf` it answers sorts
// every computer by that list; `readPlansNow` lists again for the ids it asks. A computer that appears only in the second
// list is a place `placeOf` does not know, a provider that is not connected, and is not asked. usage.rs:651-663 sorts each
// id by the second list. Node (zz-review-account-usage.oracle.ts "a computer that joins between the two lists of a plan
// read"): reads on here and box1 only.

#[test]
fn a_computer_that_shows_up_between_a_plan_reads_two_lists_is_not_asked_in_that_pass() {
    run(async {
        let here = view("here", "this Mac");
        let box1 = view("box1", "studio");
        let box2 = view("box2", "garage");
        let ctx = Bare::new(vec![vec![here.clone(), box1.clone()], vec![here, box1, box2]], 0);
        let area = UsageArea::new(ctx.clone());
        area.accounts(true).await.unwrap();
        assert_eq!(*ctx.box_execs.borrow(), vec!["box1".to_string()], "node asked here and box1; box2 was in the second list only");
    });
}
