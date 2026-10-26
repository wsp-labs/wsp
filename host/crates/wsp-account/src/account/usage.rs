// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/usage.ts at 65f6b89c7: the usage door, the agent logs and plan readings, and
//! two row sentences. The ledger, prices, burn and account rows are wsp-machines' (`usage.rs`), the alerts
//! wsp-core's, the resets `crate::usage_reset`'s; this unit wires them through `UsageCtx`. Spec: spec/usage.md.
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::{join_all, select, Either, FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_core::clock::Cancel;
use wsp_core::types::wiring::{ReadingsTarget, WorkspaceRecord, LOG_READ_EVERY_MS, LOG_READ_MS, PLAN_READ_EVERY_MS, RESET_EXEC_MS};
use wsp_engine::run_env::{env_input, with_env_from_input};
use wsp_frames::{UsageLogFormat, UsageLogsReply, UsageStore};
use wsp_wire::types::check::parse;
use wsp_wire::types::usage::{
    AccountsAnswer, HarnessLimit, HarnessLimitAccount, ReadingsRange, ResetAnswer, UsageRange, UsageSplit, UsedAnswer, READINGS_SPAN_MS, READINGS_STEP_MS, USAGE_WORDS,
};
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::wire::machine_link::{is_joined_computer, ReadingsAnswer};
use wsp_wire::wire::machine_replies::SysHistoryReply;
use wsp_words::daemon_contract::unknown_op_line;
use wsp_words::exit::HostError;
use wsp_words::project_path::under_project;
use wsp_words::sub_thread::{cloud_said, sub_cloud_gone_line};
use wsp_words::usage::reset_no_logins_line;
use wsp_words::workspace_state::absent_computer;
use wsp_words::words::computer::THIS_COMPUTER;

use crate::account::core_types::{AccountError, AccountResult, Entry, Frame};

/// A computer's name as a person reads it, by id (`nameOf`). A named alias so `async_trait` leaves its `&str` higher-ranked.
pub type NameOf = Rc<dyn for<'a> Fn(&'a str) -> String>;

/// A split value as a person reads it (`usageLabel(...)`), likewise an alias.
pub type LabelOf = Rc<dyn for<'a> Fn(UsageSplit, &'a str) -> String>;

/// usage.ts:25.
pub const SIGN_INS_HELD_MS: i64 = 60_000;

/// One log row filed into the ledger (`UsageEntry`, runtime usage.ts). wsp-machines' `usage.rs` is empty, so the row is held here (spec/join.md).
#[derive(Clone, Debug, PartialEq)]
pub struct UsageEntry {
    pub at: f64,
    pub agent: String,
    pub account: String,
    pub account_label: String,
    pub computer: String,
    pub project: String,
    pub model: String,
    pub tokens: Value,
    pub cost_usd: Option<f64>,
    pub session: String,
    /// Always `"log"` here.
    pub source: String,
}

/// One plan limit reading (`LimitReading`, runtime usage.ts). wsp-machines' `usage.rs` is empty, so the row is held here (spec/join.md).
#[derive(Clone, Debug, PartialEq)]
pub struct LimitReading {
    pub key: String,
    pub agent: String,
    pub label: String,
    pub road: String,
    pub computer: String,
    pub limit: HarnessLimit,
    /// `None` for a plan's own read (`filePlanLimit` sets none: the ledger stamps its own clock).
    pub at: Option<f64>,
}

/// An account as `accountOnComputer` and `accountOf` answer it.
#[derive(Clone, Debug, PartialEq)]
pub struct AccountKey {
    pub key: String,
    pub label: String,
    pub road: String,
}

/// A computer a plan's own read may run on (`ResetPlace`, usage-reset.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct ResetPlaceView {
    pub id: String,
    pub name: String,
    /// `here`, `box` or `provider`.
    pub kind: String,
    pub connected: bool,
}

/// Where an agent's login lives for a script run outside any turn (`ResetRoad`).
#[derive(Clone, Debug, PartialEq)]
pub struct PlanRoad {
    pub home: String,
    pub env: IndexMap<String, String>,
    pub launch_program: Option<String>,
}

/// What the area reads off the runtime's context and the modules it wires.
#[async_trait::async_trait(?Send)]
pub trait UsageCtx {
    fn now(&self) -> i64;
    /// `clock.schedule(f, ms)`, for the bound `bounded(work, ms, what)` puts on a read: `what was not answered in <ms/1000>s`.
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel;
    /// `(await ctx.preferences.get()).usageLogs`.
    async fn usage_logs_on(&self) -> bool;
    /// `placeDoor?.list(now) ?? []`.
    async fn places(&self, now: i64) -> Vec<PlaceView>;
    fn has_door(&self) -> bool;
    fn place_away(&self, place_id: &str) -> bool;
    fn name_of(&self, place_id: &str) -> String;
    /// `placeDoor?.link(id) !== undefined`.
    fn linked(&self, place_id: &str) -> bool;
    /// `placeDoor.signInsAt(id)?.[agent] === "signed-in"`.
    fn signed_in_at(&self, place_id: &str, agent: &str) -> bool;
    /// `(await placeDoor.placeFor(word))?.placeId`.
    async fn place_for(&self, word: &str) -> Option<String>;
    /// `(await placeDoor.reportOf(id))?.login["PATH"]`.
    async fn box_login_path(&self, place_id: &str) -> Option<String>;
    /// `placeDoor.exec(id, script, { timeoutMs, stdin })`'s stdout.
    async fn box_exec(&self, place_id: &str, script: &str, timeout_ms: i64, stdin: &str) -> AccountResult<String>;
    /// `local !== undefined` and `local.daemonRoad !== undefined`.
    fn local_wired(&self) -> bool;
    fn local_has_daemon(&self) -> bool;
    fn local_env(&self) -> IndexMap<String, String>;
    fn local_home(&self, agent: &str) -> String;
    /// `local.execStream({ idleMs, deadlineMs })(script, { env })`, its lines, then its exit.
    async fn local_exec_lines(&self, script: &str, env: IndexMap<String, String>, idle_ms: i64, deadline_ms: i64) -> AccountResult<Vec<String>>;
    /// `usage.logs` on a computer: this one through `onThisComputer`, a joined one over its link (absent →
    /// `absentComputer`), parsed as `UsageLogsReply`.
    async fn read_logs_on(&self, place_id: &str, stores: &[UsageStore]) -> AccountResult<UsageLogsReply>;
    /// `sys.history` to a target's daemon (U12), the reply as the daemon wrote it.
    async fn sys_history(&self, target: &ReadingsTarget, frame: Frame, origin: Option<&Caller>) -> AccountResult<Value>;
    async fn ready(&self);
    /// `claudeSessionId` of every session row.
    fn wsp_sessions(&self) -> Vec<String>;
    /// Each project on a computer: id and path.
    fn projects_on(&self, place_id: &str) -> Vec<(String, String)>;
    fn project_name(&self, id: &str) -> Option<String>;
    fn vaulted_for(&self, agent: &str) -> Option<Value>;
    /// `setups.launchOf(place, agent)`: its env, config dir and launch program.
    fn launch_of(&self, place_id: &str, agent: &str) -> (IndexMap<String, String>, Option<String>, Option<String>);
    /// The logins a place lists, and the agent's shared login, as `loginHomeIn` reads them.
    async fn login_home(&self, place_id: &str, agent: &str) -> Option<String>;
    /// `opts.agentsReader?.read({ kind: "here" }, { latest: false })`: each agent and its sign-in state.
    async fn read_sign_ins_here(&self) -> AccountResult<Option<Vec<(String, String)>>>;
    fn on_agents_changed(&self, f: Rc<dyn Fn()>);

    // The ledger, alerts and burn (wsp-machines' usage.rs, wsp-core's plan alerts).
    async fn limits(&self) -> AccountResult<Vec<Value>>;
    async fn file_logs(&self, entries: Vec<UsageEntry>, read: Vec<String>) -> AccountResult<()>;
    /// `ledger.limit(reading)`: before (`Value::Null` where the ledger held none) and after, as the alerts compare them.
    async fn file_limit(&self, reading: LimitReading) -> AccountResult<(Value, Value)>;
    async fn alerts_read(&self, before: Value, after: Value) -> AccountResult<()>;
    async fn used(&self, range: UsageRange, split: UsageSplit, outside: bool, labels: LabelOf) -> AccountResult<UsedAnswer>;
    async fn account_labels(&self) -> AccountResult<IndexMap<String, String>>;
    /// `accountOnComputer(...)` for an agent on a computer.
    fn account_on_computer(&self, agent: &str, place_id: &str, computer_name: &str, limits: &[Value]) -> AccountKey;
    /// `accountOf(...)` for a plan's reading.
    fn account_of(&self, agent: &str, named: Option<&HarnessLimitAccount>, place_id: &str, computer_name: &str) -> AccountKey;
    /// `harnessCatalog(agent)?.label ?? agent`.
    fn agent_label(&self, agent: &str) -> String;
    /// `modelLabel(model)`.
    fn model_label(&self, model: &str) -> String;
    /// `LOG_LIMITS[agent]?.(snapshot)`.
    fn log_limit(&self, agent: &str, snapshot: &Value) -> Option<HarnessLimit>;
    /// The agents with plan resets, in `PLAN_RESETS`' order, and each one's read command and parse.
    fn plan_agents(&self) -> Vec<String>;
    fn plan_read_command(&self, agent: &str, road: &PlanRoad) -> String;
    fn plan_parse(&self, agent: &str, out: &str) -> Option<HarnessLimit>;
    /// `accountRows({ limits, places, nameOf, ... })`: the ledger's limits, the listed computers (id, name, sign-ins)
    /// and the namer, the plan brands, vaulted, prints-limits and burn being the context's own.
    async fn account_rows(&self, limits: Vec<Value>, places: Vec<(String, String, Option<Value>)>, name_of: NameOf) -> AccountResult<AccountsAnswer>;
    /// `accountNames({ limits, nameOf, ... })`: an account a turn read limits for, named as the limits name it.
    async fn account_names(&self, limits: Vec<Value>, name_of: NameOf) -> AccountResult<IndexMap<String, String>>;
    /// `keptComputerNames(store, places)`: every name kept, the listed ones written.
    async fn kept_computer_names(&self, places: &[PlaceView]) -> AccountResult<IndexMap<String, String>>;
    /// `usageComputerName(places, id, kept)`; `kept` is `None` where the TS passes none.
    fn computer_name(&self, places: &[PlaceView], place_id: &str, kept: Option<&IndexMap<String, String>>) -> String;
    /// `spendReset({ account, creditId?, on? })` (usage_reset).
    async fn spend_reset(&self, account: &str, credit_id: Option<&str>, on: Option<&str>) -> Result<ResetAnswer, HostError>;
    /// The row parts `rowReason` reads.
    fn nap_refused_reason(&self, entry: &Entry) -> Option<String>;
    fn stop_refused_reason(&self, entry: &Entry) -> Option<String>;
    fn disk_reason(&self, entry: &Entry) -> Option<String>;
    fn spawn(&self, f: LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
}

/// usage.ts:27 `usageArea(ctx)`.
pub struct UsageArea {
    pub(crate) ctx: Rc<dyn UsageCtx>,
    /// usage.ts:85.
    pub(crate) logs_read_at: RefCell<Option<i64>>,
    /// usage.ts:164: when the hold began and the shared read.
    pub(crate) held_sign_ins: RefCell<Option<(i64, Shared<LocalBoxFuture<'static, IndexMap<String, String>>>)>>,
    /// usage.ts:268.
    pub(crate) plan_read_at: RefCell<IndexMap<String, i64>>,
    /// usage.ts:269.
    pub(crate) plan_reading: RefCell<Option<Shared<LocalBoxFuture<'static, ()>>>>,
}

/// The label of one split value (usage.ts:59, `usageLabel`), off the context and the names it is given.
fn usage_label_with(ctx: &dyn UsageCtx, computer_name: &dyn Fn(&str) -> String, accounts: &IndexMap<String, String>, split: UsageSplit, value: &str) -> String {
    match split {
        UsageSplit::Agent => ctx.agent_label(value),
        UsageSplit::Project => {
            if value.is_empty() {
                "No project".to_string()
            } else {
                ctx.project_name(value).unwrap_or_else(|| value.to_string())
            }
        }
        UsageSplit::Computer => computer_name(value),
        UsageSplit::Account => accounts.get(value).cloned().unwrap_or_else(|| value.to_string()),
        UsageSplit::Model => ctx.model_label(value),
        UsageSplit::Source => usage_word(if value == "log" { "outsideWsp" } else { "wspThreads" }),
    }
}

fn usage_word(name: &str) -> String {
    USAGE_WORDS.iter().find(|(key, _)| *key == name).map(|(_, text)| text.to_string()).unwrap_or_default()
}

/// The `Map` of two maps, the later winning, as `new Map([...a, ...b])` reads it.
fn merged(first: IndexMap<String, String>, later: IndexMap<String, String>) -> IndexMap<String, String> {
    let mut out = first;
    for (key, value) in later {
        out.insert(key, value);
    }
    out
}

/// The row of a limit read's account in the ledger's list (`limits.find(l => l.key === key)?.road ?? "own"`).
fn road_of(limits: &[Value], key: &str) -> String {
    limits.iter().find(|l| l["key"] == json!(key)).and_then(|l| l["road"].as_str()).unwrap_or("own").to_string()
}

impl UsageArea {
    /// usage.ts:27: registers the `agents.changed` listener that drops the sign-in hold.
    pub fn new(ctx: Rc<dyn UsageCtx>) -> Rc<UsageArea> {
        let area = Rc::new(UsageArea {
            ctx: ctx.clone(),
            logs_read_at: RefCell::new(None),
            held_sign_ins: RefCell::new(None),
            plan_read_at: RefCell::new(IndexMap::new()),
            plan_reading: RefCell::new(None),
        });
        let weak = Rc::downgrade(&area);
        ctx.on_agents_changed(Rc::new(move || {
            if let Some(area) = weak.upgrade() {
                *area.held_sign_ins.borrow_mut() = None;
            }
        }));
        area
    }

    /// U1, usage.ts:32.
    pub fn away_line(&self, record: &WorkspaceRecord) -> Option<String> {
        if let Some(sub) = &record.sub {
            if sub.cloud_gone == Some(true) && record.phase != WorkspacePhase::Released {
                return Some(sub_cloud_gone_line(&cloud_said(&sub.on)));
            }
        }
        let at = record.place.as_deref()?;
        if !self.ctx.has_door() || !self.ctx.place_away(at) {
            return None;
        }
        Some(absent_computer(&self.ctx.name_of(at), None).sentence)
    }

    /// U2, usage.ts:44.
    pub fn row_reason(&self, entry: &Entry) -> Option<String> {
        let (delete_said, wake_said, wake_refused, phase) = {
            let live = entry.borrow();
            let record = live.record.borrow();
            (live.delete_said.clone(), live.wake_said.clone(), record.wake_refused.clone(), record.phase)
        };
        let delete_line = delete_said.filter(|said| said.phase == phase).map(|said| said.line);
        delete_line
            .or(wake_said)
            .or(wake_refused)
            .or_else(|| self.ctx.nap_refused_reason(entry))
            .or_else(|| self.ctx.stop_refused_reason(entry))
            .or_else(|| self.ctx.disk_reason(entry))
    }

    /// U3, usage.ts:18, from the catalog.
    pub fn usage_stores() -> Vec<UsageStore> {
        wsp_catalog::catalog::catalog_agents()
            .into_iter()
            .filter_map(|agent| {
                if let Some(history) = &agent.history {
                    let format = match history.format {
                        wsp_catalog::catalog::HistoryFormat::ClaudeJsonl => Some(UsageLogFormat::ClaudeJsonl),
                        wsp_catalog::catalog::HistoryFormat::CodexRollout => Some(UsageLogFormat::CodexRollout),
                        _ => None,
                    };
                    if let Some(format) = format {
                        return Some(UsageStore { agent: agent.id.clone(), format, root: history.root.clone() });
                    }
                }
                let log = agent.usage_log.as_ref()?;
                let format = serde_json::from_value::<UsageLogFormat>(json!(log.format)).ok()?;
                Some(UsageStore { agent: agent.id.clone(), format, root: log.root.clone() })
            })
            .collect()
    }

    /// Runs a read no longer than `ms`: where it is not answered by then, the read fails with `<what> was not answered
    /// in <ms/1000>s` (helpers.ts `bounded`).
    async fn bounded<T: 'static>(&self, work: LocalBoxFuture<'static, AccountResult<T>>, ms: i64, what: &str) -> AccountResult<T> {
        let (fired_tx, fired_rx) = oneshot::channel::<()>();
        let cancel = self.ctx.schedule(ms, Box::new(move || {
            let _ = fired_tx.send(());
        }));
        let answer = match select(work, fired_rx).await {
            Either::Left((answer, _)) => answer,
            Either::Right(_) => Err(HostError::new(format!("{what} was not answered in {}s", (ms as f64 / 1000.0).round() as i64)).into()),
        };
        cancel();
        answer
    }

    /// U4, usage.ts:86. The snapshot handed to `log_limit` is the reading less `agent` and `at`, keys in the reply's order
    /// (with `preserve_order` a `Map::remove` swaps the last key in: use `shift_remove`). A frame's numbers are integers.
    pub async fn read_logs(self: &Rc<Self>) -> AccountResult<()> {
        if !self.ctx.usage_logs_on().await {
            return Ok(());
        }
        let now = self.ctx.now();
        if let Some(at) = *self.logs_read_at.borrow() {
            if now - at < LOG_READ_EVERY_MS as i64 {
                return Ok(());
            }
        }
        *self.logs_read_at.borrow_mut() = Some(now);
        let places = self.ctx.places(self.ctx.now()).await;
        let stores = Self::usage_stores();
        let boxes: Vec<String> = places
            .iter()
            .filter(|place| is_joined_computer(&place.id, kind_word(place)) && place.id != wsp_wire::types::place_word::HERE_PLACE_ID && self.ctx.linked(&place.id))
            .map(|place| place.id.clone())
            .collect();
        let mut targets: Vec<String> = Vec::new();
        if self.ctx.local_has_daemon() {
            targets.push(wsp_wire::types::place_word::HERE_PLACE_ID.to_string());
        }
        targets.extend(boxes);
        let reads = targets.iter().map(|id| {
            let name = self.ctx.computer_name(&places, id, None);
            let what = format!("usage.logs on {name}");
            let ctx = self.ctx.clone();
            let work_id = id.clone();
            let work_stores = stores.clone();
            let work: LocalBoxFuture<'static, AccountResult<UsageLogsReply>> = Box::pin(async move { ctx.read_logs_on(&work_id, &work_stores).await });
            let id = id.clone();
            async move {
                match self.bounded(work, LOG_READ_MS as i64, &what).await {
                    Ok(reply) => Some((id, reply)),
                    Err(e) => {
                        let said = e.message();
                        if said != unknown_op_line("usage.logs") {
                            self.ctx.warn(&format!("the agent logs on {name} were not read for usage: {said}"));
                        }
                        None
                    }
                }
            }
        });
        let read: Vec<(String, UsageLogsReply)> = join_all(reads).await.into_iter().flatten().collect();
        if read.is_empty() {
            return Ok(());
        }
        let limits = self.ctx.limits().await?;
        self.ctx.ready().await;
        let threads: HashSet<String> = self.ctx.wsp_sessions().into_iter().collect();
        let mut entries: Vec<UsageEntry> = Vec::new();
        let mut readings: Vec<LimitReading> = Vec::new();
        for (id, reply) in &read {
            let projects = self.ctx.projects_on(id);
            let computer_name = self.ctx.computer_name(&places, id, None);
            let account_there = |agent: &str| self.ctx.account_on_computer(agent, id, &computer_name, &limits);
            for row in &reply.rows {
                if threads.contains(&row.session) {
                    continue;
                }
                let account = account_there(&row.agent);
                let project = row
                    .folder
                    .as_ref()
                    .and_then(|folder| projects.iter().find(|(_, path)| under_project(folder, path)).map(|(project, _)| project.clone()))
                    .unwrap_or_default();
                entries.push(UsageEntry {
                    at: row.at as f64,
                    agent: row.agent.clone(),
                    account: account.key,
                    account_label: account.label,
                    computer: id.clone(),
                    project,
                    model: row.model.clone(),
                    tokens: serde_json::to_value(&row.tokens).unwrap_or(Value::Null),
                    cost_usd: row.cost,
                    session: row.session.clone(),
                    source: "log".to_string(),
                });
            }
            for reading in &reply.limits {
                let snapshot = match serde_json::to_value(reading) {
                    Ok(Value::Object(mut map)) => {
                        map.shift_remove("agent");
                        map.shift_remove("at");
                        Value::Object(map)
                    }
                    _ => continue,
                };
                let Some(limit) = self.ctx.log_limit(&reading.agent, &snapshot) else {
                    continue;
                };
                let account = account_there(&reading.agent);
                readings.push(LimitReading {
                    road: road_of(&limits, &account.key),
                    key: account.key,
                    agent: reading.agent.clone(),
                    label: account.label,
                    computer: id.clone(),
                    limit,
                    at: Some(reading.at as f64),
                });
            }
        }
        let read_ids: Vec<String> = read.iter().map(|(id, _)| id.clone()).collect();
        self.ctx.file_logs(entries, read_ids).await?;
        for reading in readings {
            let (before, after) = self.ctx.file_limit(reading).await?;
            if after != before {
                self.ctx.alerts_read(before, after).await?;
            }
        }
        Ok(())
    }

    /// U5, usage.ts:165: agent → `signed-in` or `vault-key`.
    pub async fn here_sign_ins(self: &Rc<Self>) -> IndexMap<String, String> {
        let now = self.ctx.now();
        let held = self.held_sign_ins.borrow().as_ref().filter(|(at, _)| now - at < SIGN_INS_HELD_MS).map(|(_, value)| value.clone());
        if let Some(value) = held {
            return value.await;
        }
        let ctx = self.ctx.clone();
        let read: LocalBoxFuture<'static, IndexMap<String, String>> = Box::pin(async move {
            match ctx.read_sign_ins_here().await {
                Ok(Some(agents)) => {
                    let mut kept = IndexMap::new();
                    for (id, state) in agents {
                        if state == "signed-in" || state == "vault-key" {
                            kept.insert(id, state);
                        }
                    }
                    kept
                }
                _ => IndexMap::new(),
            }
        });
        let shared = read.shared();
        *self.held_sign_ins.borrow_mut() = Some((now, shared.clone()));
        shared.await
    }

    /// The names of the computers as the usage records kept them (`computerNamer`).
    async fn computer_namer(&self, places: &[PlaceView]) -> AccountResult<NameOf> {
        let kept = self.ctx.kept_computer_names(places).await?;
        let ctx = self.ctx.clone();
        let places = places.to_vec();
        Ok(Rc::new(move |id: &str| ctx.computer_name(&places, id, Some(&kept))))
    }

    /// U6, usage.ts:183.
    pub async fn usage_accounts(self: &Rc<Self>) -> AccountResult<AccountsAnswer> {
        let places = self.ctx.places(self.ctx.now()).await;
        let name_of = self.computer_namer(&places).await?;
        let here = self.here_sign_ins().await;
        let here_value = Value::Object(here.iter().map(|(agent, state)| (agent.clone(), Value::String(state.clone()))).collect());
        let mut listed: Vec<(String, String, Option<Value>)> = places
            .iter()
            .map(|place| (place.id.clone(), place.name.clone(), place.sign_ins.as_ref().map(|signs| Value::Object(signs.iter().map(|(a, s)| (a.clone(), json!(s))).collect()))))
            .collect();
        match listed.iter_mut().find(|row| row.0 == wsp_wire::types::place_word::HERE_PLACE_ID) {
            Some(row) => {
                if row.2.is_none() {
                    row.2 = Some(here_value);
                }
            }
            None => listed.push((wsp_wire::types::place_word::HERE_PLACE_ID.to_string(), THIS_COMPUTER.to_string(), Some(here_value))),
        }
        let limits = self.ctx.limits().await?;
        self.ctx.account_rows(limits, listed, name_of).await
    }

    /// U7, usage.ts:208.
    pub async fn reset_run(&self, place: &ResetPlaceView, script: &str, agent: &str) -> AccountResult<String> {
        let (launch_env, _, _) = self.ctx.launch_of(&place.id, agent);
        if place.kind == "box" {
            if !self.ctx.has_door() {
                return Err(HostError::new(wsp_machines::places::types::NO_PLACE_DOOR).into());
            }
            let mut env: IndexMap<String, String> = IndexMap::new();
            if let Some(path) = self.ctx.box_login_path(&place.id).await {
                env.insert("PATH".to_string(), path);
            }
            env.extend(launch_env);
            let stdin = String::from_utf8_lossy(&env_input(&env)).into_owned();
            return self.ctx.box_exec(&place.id, &with_env_from_input(script), RESET_EXEC_MS as i64, &stdin).await;
        }
        if !self.ctx.local_wired() {
            return Err(HostError::new(absent_computer(&place.name, None).sentence).into());
        }
        let local = self.ctx.local_env();
        let mut env: IndexMap<String, String> = IndexMap::new();
        if let Some(path) = local.get("PATH") {
            env.insert("PATH".to_string(), path.clone());
        }
        if let Some(home) = local.get("HOME") {
            env.insert("HOME".to_string(), home.clone());
        }
        env.extend(launch_env);
        let lines = self.ctx.local_exec_lines(script, env, RESET_EXEC_MS as i64, RESET_EXEC_MS as i64).await?;
        Ok(lines.join("\n"))
    }

    /// U8, usage.ts:228: each computer a plan's read may run on, as the host knows it now.
    pub async fn plan_place(self: &Rc<Self>, place_id: &str) -> ResetPlaceView {
        let views = self.ctx.places(self.ctx.now()).await;
        self.place_view(place_id, &views)
    }

    /// The plan read's view of one computer, off the listed places (`planPlaces`' answer for an id).
    fn place_view(&self, place_id: &str, views: &[PlaceView]) -> ResetPlaceView {
        let name = self.ctx.computer_name(views, place_id, None);
        if place_id == wsp_wire::types::place_word::HERE_PLACE_ID {
            return ResetPlaceView { id: place_id.to_string(), name, kind: "here".to_string(), connected: self.ctx.local_wired() };
        }
        match views.iter().find(|view| view.id == place_id) {
            Some(view) if is_joined_computer(&view.id, kind_word(view)) => ResetPlaceView {
                id: place_id.to_string(),
                name,
                kind: "box".to_string(),
                connected: self.ctx.linked(place_id),
            },
            _ => ResetPlaceView { id: place_id.to_string(), name, kind: "provider".to_string(), connected: false },
        }
    }

    /// Whether the place is signed in to the agent (`signedIn(agent)` of usage.ts's `ResetPlace`).
    fn signed_in(&self, place: &ResetPlaceView, here: &IndexMap<String, String>, agent: &str) -> bool {
        match place.kind.as_str() {
            "here" => here.get(agent).map(String::as_str) == Some("signed-in"),
            "box" => self.ctx.signed_in_at(&place.id, agent),
            _ => false,
        }
    }

    /// U8, usage.ts:241.
    pub async fn plan_road(&self, agent: &str, at: &ResetPlaceView) -> Result<PlanRoad, HostError> {
        let (_, config_dir, program) = self.ctx.launch_of(&at.id, agent);
        if at.kind == "here" {
            if !self.ctx.local_wired() {
                return Err(HostError::new(absent_computer(&at.name, None).sentence));
            }
            return Ok(PlanRoad { home: config_dir.unwrap_or_else(|| self.ctx.local_home(agent)), env: IndexMap::new(), launch_program: program });
        }
        if let Some(home) = config_dir {
            return Ok(PlanRoad { home, env: IndexMap::new(), launch_program: program });
        }
        match self.ctx.login_home(&at.id, agent).await {
            Some(home) => Ok(PlanRoad { home, env: IndexMap::new(), launch_program: program }),
            None => Err(HostError::new(reset_no_logins_line(&at.name, &self.ctx.agent_label(agent)))),
        }
    }

    /// U9, usage.ts:258: the key the reading went under.
    pub async fn file_plan_limit(&self, agent: &str, place: &ResetPlaceView, limit: HarnessLimit) -> AccountResult<String> {
        let account = self.ctx.account_of(agent, limit.account.as_ref(), &place.id, &place.name);
        let reading = LimitReading {
            key: account.key.clone(),
            agent: agent.to_string(),
            label: account.label,
            road: account.road,
            computer: place.id.clone(),
            limit,
            at: None,
        };
        let (before, after) = self.ctx.file_limit(reading).await?;
        self.ctx.alerts_read(before, after).await?;
        Ok(account.key)
    }

    /// U9, usage.ts:270.
    pub async fn read_plans_now(self: &Rc<Self>) {
        let existing = self.plan_reading.borrow().clone();
        if let Some(read) = existing {
            read.await;
            return;
        }
        let this = self.clone();
        let pass: LocalBoxFuture<'static, ()> = Box::pin(async move {
            this.plan_pass().await;
            *this.plan_reading.borrow_mut() = None;
        });
        let shared = pass.shared();
        *self.plan_reading.borrow_mut() = Some(shared.clone());
        shared.await;
    }

    async fn plan_pass(self: &Rc<Self>) {
        let _ = self.ctx.places(self.ctx.now()).await;
        let here = self.here_sign_ins().await;
        let views = self.ctx.places(self.ctx.now()).await;
        let mut ids: Vec<String> = vec![wsp_wire::types::place_word::HERE_PLACE_ID.to_string()];
        for view in views.iter().filter(|view| is_joined_computer(&view.id, kind_word(view))) {
            if !ids.contains(&view.id) {
                ids.push(view.id.clone());
            }
        }
        let mut asks: Vec<LocalBoxFuture<'_, ()>> = Vec::new();
        for agent in self.ctx.plan_agents() {
            for id in &ids {
                let place = self.place_view(id, &views);
                let key = format!("{agent}@{id}");
                let now = self.ctx.now();
                let stamped = self.plan_read_at.borrow().get(&key).copied();
                if place.kind == "provider" || !place.connected || !self.signed_in(&place, &here, &agent) {
                    continue;
                }
                if stamped.is_some_and(|at| now - at < PLAN_READ_EVERY_MS as i64) {
                    continue;
                }
                self.plan_read_at.borrow_mut().insert(key, now);
                asks.push(Box::pin(self.plan_ask(agent.clone(), place)));
            }
        }
        join_all(asks).await;
    }

    /// One agent's plan read on one computer: the read is bounded, a reading with a limit is filed, a failure warns.
    async fn plan_ask(self: &Rc<Self>, agent: String, place: ResetPlaceView) {
        let label = self.ctx.agent_label(&agent);
        let outcome: AccountResult<()> = async {
            let road = self.plan_road(&agent, &place).await?;
            let command = self.ctx.plan_read_command(&agent, &road);
            let this = self.clone();
            let (run_place, run_agent) = (place.clone(), agent.clone());
            let work: LocalBoxFuture<'static, AccountResult<String>> = Box::pin(async move { this.reset_run(&run_place, &command, &run_agent).await });
            let out = self.bounded(work, RESET_EXEC_MS as i64, &format!("{agent} limits on {}", place.name)).await?;
            if let Some(limit) = self.ctx.plan_parse(&agent, &out) {
                self.file_plan_limit(&agent, &place, limit).await?;
            }
            Ok(())
        }
        .await;
        if let Err(e) = outcome {
            self.ctx.warn(&format!("{label}'s limits on {} were not read: {}", place.name, e.message()));
        }
    }

    /// U10, usage.ts:306.
    pub async fn reset(&self, account: &str, credit_id: Option<&str>, on: Option<&str>) -> Result<ResetAnswer, HostError> {
        let on_place = match on {
            Some(word) => Some(self.ctx.place_for(word).await.unwrap_or_else(|| wsp_wire::types::place_word::HERE_PLACE_ID.to_string())),
            None => None,
        };
        self.ctx.spend_reset(account, credit_id, on_place.as_deref()).await
    }

    /// U11, usage.ts:313.
    pub async fn used(self: &Rc<Self>, range: UsageRange, split: UsageSplit, outside: Option<bool>) -> AccountResult<UsedAnswer> {
        let wants_logs = outside == Some(true) || split == UsageSplit::Source;
        let outside = wants_logs && self.ctx.usage_logs_on().await;
        if outside {
            if let Err(e) = self.read_logs().await {
                self.ctx.warn(&format!("the agent logs were not read for usage: {}", e.message()));
            }
        }
        let places = self.ctx.places(self.ctx.now()).await;
        let name_of = self.computer_namer(&places).await?;
        let limits = self.ctx.limits().await?;
        let names = self.ctx.account_names(limits, name_of.clone()).await?;
        let labels = self.ctx.account_labels().await?;
        let accounts = merged(labels, names);
        let label = self.label_of(name_of, accounts);
        self.ctx.used(range, split, outside, label).await
    }

    /// The label function `used` hands the ledger, over this area's context.
    fn label_of(&self, name_of: NameOf, accounts: IndexMap<String, String>) -> LabelOf {
        let ctx = self.ctx.clone();
        Rc::new(move |split: UsageSplit, value: &str| usage_label_with(&*ctx, &*name_of, &accounts, split, value))
    }

    /// U11, usage.ts:59: a split value as a person reads it.
    pub fn usage_label(&self, computer_name: &dyn Fn(&str) -> String, accounts: &IndexMap<String, String>, split: UsageSplit, value: &str) -> String {
        usage_label_with(&*self.ctx, computer_name, accounts, split, value)
    }

    /// U12, usage.ts:323.
    pub async fn readings(&self, target: ReadingsTarget, range: ReadingsRange, origin: Option<&Caller>) -> AccountResult<ReadingsAnswer> {
        let to = self.ctx.now();
        let range_key = serde_json::to_value(range).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        let span = READINGS_SPAN_MS.iter().find(|(key, _)| *key == range_key).map(|(_, ms)| *ms as i64).unwrap_or(0);
        let step = READINGS_STEP_MS.iter().find(|(key, _)| *key == range_key).map(|(_, ms)| *ms as i64).unwrap_or(0);
        let from = to - span;
        let mut frame = Frame::new();
        frame.insert("op".to_string(), json!("sys.history"));
        frame.insert("from".to_string(), json!(from));
        frame.insert("to".to_string(), json!(to));
        frame.insert("stepMs".to_string(), json!(step));
        let raw = self.ctx.sys_history(&target, frame, origin).await?;
        let reply: SysHistoryReply = parse(&raw).map_err(|e| AccountError::Host(HostError::new(e.issue_text())))?;
        Ok(ReadingsAnswer {
            points: reply.points,
            step_ms: reply.step_ms,
            from: wsp_wire::types::js_number::JsNumber(from as f64),
            to: wsp_wire::types::js_number::JsNumber(to as f64),
        })
    }

    /// U13, usage.ts:338.
    pub async fn accounts(self: &Rc<Self>, fresh: bool) -> AccountResult<AccountsAnswer> {
        if fresh {
            self.read_plans_now().await;
        }
        self.usage_accounts().await
    }
}

/// `place.kind` as the wire spells it, for `isJoinedComputer`.
fn kind_word(place: &PlaceView) -> &'static str {
    match place.kind {
        wsp_wire::views::place_kind::PlaceKind::Computer => "computer",
        wsp_wire::views::place_kind::PlaceKind::Provider => "provider",
        _ => "",
    }
}
