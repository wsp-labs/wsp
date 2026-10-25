// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/usage.ts at 65f6b89c7: the usage door, the agent logs and plan readings, and
//! two row sentences. The ledger, prices, burn and account rows are wsp-machines' (`usage.rs`), the alerts
//! wsp-core's, the resets `crate::usage_reset`'s; this unit wires them through `UsageCtx`. Spec: spec/usage.md.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::Value;

use wsp_core::types::wiring::{ReadingsTarget, WorkspaceRecord};
use wsp_frames::{UsageLogsReply, UsageStore};
use wsp_wire::types::usage::{AccountsAnswer, HarnessLimit, HarnessLimitAccount, ReadingsRange, ResetAnswer, UsageRange, UsageSplit, UsedAnswer};
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::machine_link::ReadingsAnswer;
use wsp_words::exit::HostError;

use wsp_core::clock::Cancel;
use crate::account::core_types::{AccountResult, Entry, Frame};

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
#[async_trait(?Send)]
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

impl UsageArea {
    /// usage.ts:27: registers the `agents.changed` listener that drops the sign-in hold.
    pub fn new(ctx: Rc<dyn UsageCtx>) -> Rc<UsageArea> {
        todo!()
    }

    /// U1, usage.ts:32.
    pub fn away_line(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }

    /// U2, usage.ts:44.
    pub fn row_reason(&self, entry: &Entry) -> Option<String> {
        todo!()
    }

    /// U3, usage.ts:18, from the catalog.
    pub fn usage_stores() -> Vec<UsageStore> {
        todo!()
    }

    /// U4, usage.ts:86. The snapshot handed to `log_limit` is the reading less `agent` and `at`, keys in the reply's order
    /// (with `preserve_order` a `Map::remove` swaps the last key in: use `shift_remove`). A frame's numbers are integers.
    pub async fn read_logs(self: &Rc<Self>) -> AccountResult<()> {
        todo!()
    }

    /// U5, usage.ts:165: agent → `signed-in` or `vault-key`.
    pub async fn here_sign_ins(self: &Rc<Self>) -> IndexMap<String, String> {
        todo!()
    }

    /// U6, usage.ts:183.
    pub async fn usage_accounts(self: &Rc<Self>) -> AccountResult<AccountsAnswer> {
        todo!()
    }

    /// U7, usage.ts:208.
    pub async fn reset_run(&self, place: &ResetPlaceView, script: &str, agent: &str) -> AccountResult<String> {
        todo!()
    }

    /// U8, usage.ts:228: each computer a plan's read may run on, as the host knows it now.
    pub async fn plan_place(self: &Rc<Self>, place_id: &str) -> ResetPlaceView {
        todo!()
    }

    /// U8, usage.ts:241.
    pub async fn plan_road(&self, agent: &str, at: &ResetPlaceView) -> Result<PlanRoad, HostError> {
        todo!()
    }

    /// U9, usage.ts:258: the key the reading went under.
    pub async fn file_plan_limit(&self, agent: &str, place: &ResetPlaceView, limit: HarnessLimit) -> AccountResult<String> {
        todo!()
    }

    /// U9, usage.ts:270.
    pub async fn read_plans_now(self: &Rc<Self>) {
        todo!()
    }

    /// U10, usage.ts:306.
    pub async fn reset(&self, account: &str, credit_id: Option<&str>, on: Option<&str>) -> Result<ResetAnswer, HostError> {
        todo!()
    }

    /// U11, usage.ts:313.
    pub async fn used(self: &Rc<Self>, range: UsageRange, split: UsageSplit, outside: Option<bool>) -> AccountResult<UsedAnswer> {
        todo!()
    }

    /// U11, usage.ts:59: a split value as a person reads it.
    pub fn usage_label(&self, computer_name: &dyn Fn(&str) -> String, accounts: &IndexMap<String, String>, split: UsageSplit, value: &str) -> String {
        todo!()
    }

    /// U12, usage.ts:323.
    pub async fn readings(&self, target: ReadingsTarget, range: ReadingsRange, origin: Option<&Caller>) -> AccountResult<ReadingsAnswer> {
        todo!()
    }

    /// U13, usage.ts:338.
    pub async fn accounts(self: &Rc<Self>, fresh: bool) -> AccountResult<AccountsAnswer> {
        todo!()
    }
}
