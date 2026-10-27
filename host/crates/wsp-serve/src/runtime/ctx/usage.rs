// SPDX-License-Identifier: AGPL-3.0-only
//! The context's usage and status readings (usage.ts, usage-reset.ts, status.ts, runtime.ts:480 and :484): the places,
//! this computer and the daemons the usage area reads, the ledger, prices, burn, plan alerts and resets it shares, and
//! the status area's rows, sweep and tracker. `UsageParts` is built once per context and handed to every reader.
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::{FutureExt as _, StreamExt as _};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_account::account::core_types::{AccountError, AccountResult, Entry as AccountEntry, Frame, Work};
use wsp_account::account::status::{Confirmed, ListedRow, ReadFailure, StatusArea};
use wsp_account::account::usage::{AccountKey, LabelOf, LimitReading, NameOf, PlanRoad, ResetPlaceView, UsageCtx, UsageEntry};
use wsp_account::usage_reset::{usage_resets, FileAsk, PlaceOf, ResetAsk, ResetPlace, ResetPlaceKind, Resets, UsageResetDeps, UsageResets};
use wsp_adapters::port::{AgentLaunch, ExecOptions as RunOptions, PlanResets, ResetReading, ResetRoad, ResetSpend};
use wsp_core::clock::Cancel;
use wsp_core::plan_alerts::{plan_alerts, PlanAlerts, PlanAlertsDeps};
use wsp_core::status::{DaemonAnswers, RecordCall, RecordExec, RouteReach, StatusRecord, StatusTracker, StatusTrackerOptions};
use wsp_core::types::events::EventBus as _;
use wsp_core::types::wiring::{ReadingsTarget, WorkspaceRecord};
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{ExecOptions, Machine, MachineState, Size};
use wsp_engine::orphans::ListedMachine;
use wsp_frames::{UsageLogsReply, UsageStore};
use wsp_machines::machines::boot::entries::EntryAsk;
use wsp_machines::places::types::{PlaceDoor, PlaceExecOpts};
use wsp_machines::usage::accounts::{
    account_names, account_of, account_on_computer, account_rows, kept_computer_names, usage_computer_name, AccountRowsAsk, Burn,
    ComputerName, NamedAccount, PriceTable, SignedInPlace,
};
use wsp_machines::usage::ledger::{UsageLedger, UsageLedgerOptions};
use wsp_machines::usage::{EntryTokens, UsageSource, UsedQuery, Vaulted};
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::usage::{
    AccountLimit, AccountRoad, AccountsAnswer, HarnessLimit, HarnessLimitAccount, ResetAnswer, UsageRange, UsageSplit, UsedAnswer,
};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::{Caller, ReachState, WorkspaceStatus};
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_wire::wire::machine_link::is_joined_computer;
use wsp_words::exit::HostError;
use wsp_words::words::computer::THIS_COMPUTER;
use wsp_words::workspace_state::absent_computer;

use super::Ctx;
use crate::runtime::join::spawn_detached;

/// What `usageArea` builds once and shares (usage.ts:46-50, 292-303): the price table, the ledger, the plan alerts, the
/// burn and the resets. The usage area, the status tracker and the turns' filings all read these one values.
pub struct UsageParts {
    pub prices: Rc<PriceTable>,
    pub ledger: Rc<UsageLedger>,
    pub alerts: Rc<PlanAlerts>,
    pub burn: Burn,
    pub resets: UsageResets,
}

impl Ctx {
    /// The usage parts of this context, built on first use and then the same ones.
    pub fn usage_parts(&self) -> Rc<UsageParts> {
        if !self.usage_parts.is_filled() {
            self.usage_parts.fill(usage_parts_of(self));
        }
        self.usage_parts.get()
    }

    /// `void alerts.resume().catch(warn)` (usage.ts:49): arms the plan alerts the state holds.
    pub fn usage_arm(self: &Rc<Ctx>) {
        let alerts = self.usage_parts().alerts.clone();
        spawn_detached(async move {
            if let Err(e) = alerts.resume().await {
                wsp_core::console::warn(&format!("the plan alerts were not armed: {}", e.message));
            }
        });
    }

    /// `createStatusTracker({..})` (status.ts:11-69): the tracker over the status rows and the poll's callbacks.
    pub fn usage_status_tracker(self: &Rc<Ctx>) -> Rc<StatusTracker> {
        let area = self.status.get();
        let records_ctx = Rc::downgrade(self);
        let records_area = area.clone();
        let polled_area = area.clone();
        let gone_area = area;
        let bus = self.bus.clone();
        let pass_bus = self.bus.clone();
        let on_bus = self.bus.clone();
        StatusTracker::new(StatusTrackerOptions {
            records: Rc::new(move || {
                let (ctx, area) = (records_ctx.upgrade(), records_area.clone());
                async move {
                    let ctx = ctx.ok_or_else(|| "the runtime is gone".to_string())?;
                    status_records(&ctx, &area).await.map_err(|e| e.message)
                }
                .boxed_local()
            }),
            store: self.store.clone(),
            emit: Rc::new(move |e| {
                if let Err(err) = bus.emit(e) {
                    wsp_core::console::warn(&err.message);
                }
            }),
            pass: Rc::new(move |e| {
                if let Err(err) = pass_bus.pass(e) {
                    wsp_core::console::warn(&err.message);
                }
            }),
            on: Rc::new(move |kind, listener| {
                let heard: wsp_core::types::events::EventListener = Rc::new(move |e: &EventUnion| {
                    listener(e);
                    Ok(())
                });
                Rc::from(on_bus.on(kind, heard))
            }),
            defaults: self.opts.status.unwrap_or_default(),
            on_polled: Some(Rc::new(move |statuses: &[WorkspaceStatus]| {
                let rows: Vec<Value> = statuses.iter().filter_map(|s| serde_json::to_value(s).ok()).collect();
                polled_area.on_polled(&rows);
            })),
            on_gone: Some(Rc::new(move |id: &str, machine_id: &str, reason: &str| gone_area.on_gone(id, machine_id, reason))),
            clock: Some(self.clock.clone()),
            zone: None,
        })
    }
}

fn usage_parts_of(ctx: &Ctx) -> Rc<UsageParts> {
    let prices_fetch = ctx.opts.prices_fetch.clone();
    let fetch: Rc<dyn Fn() -> LocalBoxFuture<'static, Result<Value, HostError>>> = Rc::new(move || match &prices_fetch {
        Some(fetch) => fetch(),
        None => Box::pin(async { Ok(json!({})) }),
    });
    let prices = PriceTable::new(ctx.store.clone(), ctx.clock.clone(), fetch, Rc::new(|line: &str| wsp_core::console::warn(line)));
    let priced = prices.clone();
    let ledger = UsageLedger::new(UsageLedgerOptions {
        store: ctx.store.clone(),
        clock: ctx.clock.clone(),
        time_zone: None,
        prices: Rc::new(move || {
            let prices = priced.clone();
            Box::pin(async move { prices.get().await })
        }),
        retention_days: None,
    });
    let bus = ctx.bus.clone();
    let held = ledger.clone();
    let alerts = plan_alerts(PlanAlertsDeps {
        clock: ctx.clock.clone(),
        emit: Rc::new(move |e| {
            if let Err(err) = bus.emit(EventUnion::UsageAlert(Sequenced { event: e, seq: None })) {
                wsp_core::console::warn(&err.message);
            }
        }),
        limits: Rc::new(move || {
            let ledger = held.clone();
            Box::pin(async move { ledger.limits().await })
        }),
    });
    Rc::new(UsageParts {
        prices,
        ledger,
        alerts,
        burn: Burn::new(ctx.clock.clone()),
        resets: usage_resets(Rc::new(ResetDeps(ctx.me.clone()))),
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Small readings shared by the three traits below.

fn list_places_of(ctx: &Ctx) -> Option<Rc<dyn PlaceDoor>> {
    ctx.wired_door()
}

async fn usage_places(ctx: &Ctx, now: i64) -> Vec<PlaceView> {
    match list_places_of(ctx) {
        Some(door) => door.list(now.max(0) as u64).await.unwrap_or_default(),
        None => Vec::new(),
    }
}

/// `placeAway` (runtime.ts:852): a door is wired and holds neither a backend nor a link to the place.
fn usage_place_away(ctx: &Ctx, place_id: &str) -> bool {
    match list_places_of(ctx) {
        Some(door) => ctx.places.backend(place_id).is_none() && door.link(place_id).is_none(),
        None => false,
    }
}

fn usage_vaulted(ctx: &Ctx, agent: &str) -> Option<Vaulted> {
    ctx.threads.get().vaulted_for(agent, None, None).map(|word| if word == "token" { Vaulted::Token } else { Vaulted::Key })
}

fn usage_agent_label(agent: &str) -> String {
    wsp_core::harness_catalog::harness_catalog(agent).map(|c| c.label).unwrap_or_else(|| agent.to_string())
}

fn kind_word(place: &PlaceView) -> &'static str {
    match place.kind {
        PlaceKind::Computer => "computer",
        PlaceKind::Provider => "provider",
    }
}

fn road_word(road: AccountRoad) -> String {
    serde_json::to_value(road).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn account_limits(values: &[Value]) -> Vec<AccountLimit> {
    values.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect()
}

fn limit_values(limits: Vec<AccountLimit>) -> Vec<Value> {
    limits.iter().filter_map(|l| serde_json::to_value(l).ok()).collect()
}

fn road_of(road: &str) -> AccountRoad {
    serde_json::from_value(json!(road)).unwrap_or(AccountRoad::Own)
}

fn sign_ins_of(value: &Value) -> Option<IndexMap<String, AgentSignInState>> {
    serde_json::from_value(value.clone()).ok()
}

fn rows_ask(ctx: &Ctx, limits: Vec<Value>, places: Vec<SignedInPlace>, name_of: NameOf) -> AccountRowsAsk {
    let vaulted_by = ctx.me.clone();
    let parts = ctx.usage_parts();
    AccountRowsAsk {
        limits: account_limits(&limits),
        places,
        name_of,
        agent_name: Rc::new(|agent: &str| usage_agent_label(agent)),
        plan_brand: Some(Rc::new(|agent: &str| {
            wsp_catalog::catalog::catalog_agents().into_iter().find(|a| a.id == agent).and_then(|a| a.plan_brand.clone())
        })),
        vaulted: Rc::new(move |agent: &str| vaulted_by.upgrade().and_then(|ctx| usage_vaulted(&ctx, agent))),
        prints_limits: Rc::new(|agent: &str| {
            wsp_catalog::catalog::catalog_agents().into_iter().find(|a| a.id == agent).is_some_and(|a| a.prints_limits == Some(true))
        }),
        burn: Some(Rc::new(move |account: &str| parts.burn.of(account))),
    }
}

fn plan_resets_of(agent: &str) -> Option<Arc<dyn PlanResets>> {
    wsp_core::adapters::plan_resets().get(agent).cloned()
}

fn entry_tokens(value: &Value) -> Option<EntryTokens> {
    let n = |key: &str| value.get(key).and_then(Value::as_f64);
    Some(EntryTokens {
        input: n("input"),
        output: n("output"),
        cached: n("cached"),
        cache_write: n("cacheWrite"),
        reasoning: n("reasoning"),
    })
}

fn ledger_entry(e: UsageEntry) -> wsp_machines::usage::UsageEntry {
    wsp_machines::usage::UsageEntry {
        at: e.at as i64,
        agent: e.agent,
        account: e.account,
        account_label: e.account_label,
        computer: e.computer,
        project: e.project,
        model: Some(e.model),
        turns: None,
        tokens: entry_tokens(&e.tokens),
        cost_usd: e.cost_usd,
        session: Some(e.session),
        source: if e.source == "log" { UsageSource::Log } else { UsageSource::Wsp },
    }
}

fn status_word<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn reach_state(word: &str) -> ReachState {
    serde_json::from_value(json!(word)).unwrap_or(ReachState::Unsupported)
}

fn missing_or_failed(e: &EngineError) -> ReadFailure {
    if wsp_engine::errors::is_missing(e) {
        ReadFailure::Missing
    } else {
        ReadFailure::Failed(e.to_string())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// UsageCtx (usage.ts:27-344).

#[async_trait::async_trait(?Send)]
impl UsageCtx for Ctx {
    fn now(&self) -> i64 {
        self.clock.now()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(f, ms)
    }
    async fn usage_logs_on(&self) -> bool {
        self.preferences.get().get().await.map(|p| p.usage_logs).unwrap_or(false)
    }
    async fn places(&self, now: i64) -> Vec<PlaceView> {
        usage_places(self, now).await
    }
    fn has_door(&self) -> bool {
        list_places_of(self).is_some()
    }
    fn place_away(&self, place_id: &str) -> bool {
        usage_place_away(self, place_id)
    }
    fn name_of(&self, place_id: &str) -> String {
        match list_places_of(self) {
            Some(door) => door.name_of(place_id),
            None => place_id.to_string(),
        }
    }
    fn linked(&self, place_id: &str) -> bool {
        list_places_of(self).is_some_and(|door| door.link(place_id).is_some())
    }
    fn signed_in_at(&self, place_id: &str, agent: &str) -> bool {
        list_places_of(self)
            .and_then(|door| door.sign_ins_at(place_id, None))
            .is_some_and(|signs| signs.get(agent) == Some(&AgentSignInState::SignedIn))
    }
    async fn place_for(&self, word: &str) -> Result<Option<String>, HostError> {
        let Some(door) = list_places_of(self) else { return Ok(None) };
        Ok(door.place_for(word).await?.place_id)
    }
    async fn box_login_path(&self, place_id: &str) -> Option<String> {
        let door = list_places_of(self)?;
        door.report_of(place_id).await.ok().flatten().and_then(|report| report.login.get("PATH").cloned())
    }
    async fn box_exec(&self, place_id: &str, script: &str, timeout_ms: i64, stdin: &str) -> AccountResult<String> {
        let door = list_places_of(self).ok_or_else(|| HostError::new(wsp_machines::places::types::NO_PLACE_DOOR))?;
        let ran = door
            .exec(place_id, script, PlaceExecOpts { timeout_ms: Some(timeout_ms.max(0) as u64), stdin: Some(stdin.as_bytes().to_vec()) })
            .await?;
        Ok(ran.stdout)
    }
    fn local_wired(&self) -> bool {
        self.opts.local.is_some()
    }
    fn local_has_daemon(&self) -> bool {
        self.opts.local.as_ref().is_some_and(|local| local.daemon_road.is_some())
    }
    fn local_env(&self) -> IndexMap<String, String> {
        self.opts.local.as_ref().map(|local| (local.env)()).unwrap_or_default()
    }
    fn local_home(&self, agent: &str) -> String {
        self.opts.local.as_ref().map(|local| (local.home)(agent)).unwrap_or_default()
    }
    async fn local_exec_lines(
        &self,
        script: &str,
        env: IndexMap<String, String>,
        idle_ms: i64,
        deadline_ms: i64,
    ) -> AccountResult<Vec<String>> {
        let local = self.opts.local.as_ref().ok_or_else(|| HostError::new(absent_computer(THIS_COMPUTER, None).sentence))?;
        let factory = (local.exec_stream)(
            Some(wsp_core::types::pending::MachineExecOptions {
                idle_ms: Some(idle_ms as f64),
                deadline_ms: Some(deadline_ms as f64),
                ..Default::default()
            }),
            None,
        );
        let stream = factory.launch(script, RunOptions { env, ..Default::default() });
        let mut lines = stream.lines();
        let mut out = Vec::new();
        while let Some(line) = lines.next().await {
            out.push(line.map_err(|e| HostError::new(e.message))?);
        }
        let _ = stream.exited().await;
        Ok(out)
    }
    async fn read_logs_on(&self, place_id: &str, stores: &[UsageStore]) -> AccountResult<UsageLogsReply> {
        let mut frame = Frame::new();
        frame.insert("op".to_string(), json!("usage.logs"));
        frame.insert("stores".to_string(), serde_json::to_value(stores).unwrap_or(Value::Null));
        let work: Work<UsageLogsReply> = Box::new(move |ask| {
            Box::pin(async move {
                let reply = ask(frame).await?;
                serde_json::from_value::<UsageLogsReply>(Value::Object(reply)).map_err(|e| HostError::new(e.to_string()).into())
            })
        });
        if place_id == HERE_PLACE_ID {
            return self.channels.get().on_this_computer(Box::new(move |ask, _home| work(ask))).await;
        }
        let link = list_places_of(self).and_then(|door| door.channel(place_id, Rc::new(|_| {})));
        match link {
            Some(link) => self.channels.get().over_channel(link, work).await,
            None => {
                let name = list_places_of(self).map(|door| door.name_of(place_id)).unwrap_or_else(|| place_id.to_string());
                Err(HostError::new(absent_computer(&name, None).sentence).into())
            }
        }
    }
    async fn sys_history(&self, target: &ReadingsTarget, frame: Frame, origin: Option<&Caller>) -> AccountResult<Value> {
        let work: Work<Value> = Box::new(move |ask| Box::pin(async move { Ok(Value::Object(ask(frame).await?)) }));
        match target {
            ReadingsTarget::Workspace(id) => {
                let entry = self.boot.get().entry_of(id, origin, EntryAsk::default()).await?;
                self.channels.get().with_daemon(&entry, work).await
            }
            ReadingsTarget::Place(id) if id == HERE_PLACE_ID => {
                let road = self.kinds.get().local_road().await.map_err(|e| AccountError::from(e.into_host()))?;
                let channel = self.channels.get().channel_over(&road, THIS_COMPUTER, Rc::new(|_: Value| {})).await?;
                self.channels.get().over_channel(channel, work).await
            }
            ReadingsTarget::Place(id) => match list_places_of(self).and_then(|door| door.channel(id, Rc::new(|_| {}))) {
                Some(link) => self.channels.get().over_channel(link, work).await,
                None => {
                    let name = list_places_of(self).map(|door| door.name_of(id)).unwrap_or_else(|| id.clone());
                    Err(HostError::new(absent_computer(&name, None).sentence).into())
                }
            },
        }
    }
    async fn ready(&self) {
        let _ = self.booted().await;
    }
    fn wsp_sessions(&self) -> Vec<String> {
        self.sessions.borrow().values().filter_map(|row| row.borrow().view.claude_session_id.clone()).collect()
    }
    fn projects_on(&self, place_id: &str) -> Vec<(String, String)> {
        self.projects_held.borrow().values().filter(|p| p.computer == place_id).map(|p| (p.id.clone(), p.path.clone())).collect()
    }
    fn project_name(&self, id: &str) -> Option<String> {
        self.projects_held.borrow().get(id).map(|p| p.name.clone())
    }
    fn vaulted_for(&self, agent: &str) -> Option<Value> {
        self.threads.get().vaulted_for(agent, None, None).map(|word| json!(word))
    }
    fn launch_of(&self, place_id: &str, agent: &str) -> (IndexMap<String, String>, Option<String>, Option<String>) {
        let setup = self.setups.launch_of(place_id, agent);
        (setup.env, setup.config_dir, setup.launch.and_then(|launch| launch.program))
    }
    async fn login_home(&self, place_id: &str, agent: &str) -> Option<String> {
        let logins = usage_places(self, self.clock.now()).await.into_iter().find(|view| view.id == place_id)?.logins?;
        let shared = wsp_catalog::catalog::shared_on(agent)?;
        Some(wsp_catalog::signin::login_home_in(&logins, shared))
    }
    async fn read_sign_ins_here(&self) -> AccountResult<Option<Vec<(String, String)>>> {
        // `opts.agents_reader` is the host's port (ruling 3): no Rust body exists to wire, so nothing here reads one.
        Ok(None)
    }
    fn on_agents_changed(&self, f: Rc<dyn Fn()>) {
        let _ = self.hear("agents.changed", move |_| f());
    }
    async fn limits(&self) -> AccountResult<Vec<Value>> {
        Ok(limit_values(self.usage_parts().ledger.limits().await?))
    }
    async fn file_logs(&self, entries: Vec<UsageEntry>, read: Vec<String>) -> AccountResult<()> {
        Ok(self.usage_parts().ledger.file_logs(entries.into_iter().map(ledger_entry).collect(), read).await?)
    }
    async fn file_limit(&self, reading: LimitReading) -> AccountResult<(Value, Value)> {
        let kept = self
            .usage_parts()
            .ledger
            .limit(wsp_machines::usage::LimitReading {
                key: reading.key,
                agent: reading.agent,
                label: reading.label,
                road: road_of(&reading.road),
                computer: reading.computer,
                limit: reading.limit,
                at: reading.at.map(|at| at as i64),
            })
            .await?;
        let before = kept.before.and_then(|l| serde_json::to_value(l).ok()).unwrap_or(Value::Null);
        Ok((before, serde_json::to_value(kept.after).unwrap_or(Value::Null)))
    }
    async fn alerts_read(&self, before: Value, after: Value) -> AccountResult<()> {
        let before: Option<AccountLimit> = serde_json::from_value(before).ok();
        let after: AccountLimit = serde_json::from_value(after).map_err(|e| HostError::new(e.to_string()))?;
        self.usage_parts().alerts.read(before.as_ref(), &after);
        Ok(())
    }
    async fn used(&self, range: UsageRange, split: UsageSplit, outside: bool, labels: LabelOf) -> AccountResult<UsedAnswer> {
        let label: Rc<dyn Fn(UsageSplit, &str) -> String> = labels;
        Ok(self.usage_parts().ledger.used(UsedQuery { range, split, label, outside: Some(outside) }).await?)
    }
    async fn account_labels(&self) -> AccountResult<IndexMap<String, String>> {
        Ok(self.usage_parts().ledger.account_labels().await?)
    }
    fn account_on_computer(&self, agent: &str, place_id: &str, computer_name: &str, limits: &[Value]) -> AccountKey {
        let computer = ComputerName { id: place_id.to_string(), name: computer_name.to_string() };
        let held = account_limits(limits);
        let (key, label) = account_on_computer(agent, &usage_agent_label(agent), &computer, &held, usage_vaulted(self, agent));
        let road = held.iter().find(|l| l.key == key).and_then(|l| l.road).map(road_word).unwrap_or_else(|| "own".to_string());
        AccountKey { key, label, road }
    }
    fn account_of(&self, agent: &str, named: Option<&HarnessLimitAccount>, place_id: &str, computer_name: &str) -> AccountKey {
        let named = named.map(|n| NamedAccount { id: n.id.clone(), label: n.label.clone() });
        let computer = ComputerName { id: place_id.to_string(), name: computer_name.to_string() };
        let account = account_of(agent, &usage_agent_label(agent), named.as_ref(), None, &computer);
        AccountKey { key: account.key, label: account.label, road: road_word(account.road) }
    }
    fn agent_label(&self, agent: &str) -> String {
        usage_agent_label(agent)
    }
    fn model_label(&self, model: &str) -> String {
        wsp_core::harness_catalog::model_label(model)
    }
    fn log_limit(&self, agent: &str, snapshot: &Value) -> Option<HarnessLimit> {
        let read = *wsp_core::adapters::log_limits().get(agent)?;
        read(snapshot.as_object()?)
    }
    fn plan_agents(&self) -> Vec<String> {
        wsp_core::adapters::plan_resets().keys().map(|agent| agent.to_string()).collect()
    }
    fn plan_read_command(&self, agent: &str, road: &PlanRoad) -> String {
        let Some(resets) = plan_resets_of(agent) else { return String::new() };
        let road = ResetRoad {
            home: road.home.clone(),
            env: Some(road.env.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect()),
            launch: road.launch_program.clone().map(|program| AgentLaunch { program: Some(program), args: None }),
        };
        // The TS throws on a home that is not absolute and the plan read warns; the trait answers a plain string, so
        // the refusal is no script at all (named in the report for the join).
        resets.read_command(&road).unwrap_or_default()
    }
    fn plan_parse(&self, agent: &str, out: &str) -> Option<HarnessLimit> {
        plan_resets_of(agent)?.parse_read(out)?.limit
    }
    async fn account_rows(
        &self,
        limits: Vec<Value>,
        places: Vec<(String, String, Option<Value>)>,
        name_of: NameOf,
    ) -> AccountResult<AccountsAnswer> {
        let places = places
            .into_iter()
            .map(|(id, name, signs)| SignedInPlace { id, name, sign_ins: signs.as_ref().and_then(sign_ins_of) })
            .collect();
        Ok(AccountsAnswer { accounts: account_rows(&rows_ask(self, limits, places, name_of)) })
    }
    async fn account_names(&self, limits: Vec<Value>, name_of: NameOf) -> AccountResult<IndexMap<String, String>> {
        Ok(account_names(&rows_ask(self, limits, Vec::new(), name_of)))
    }
    async fn kept_computer_names(&self, places: &[PlaceView]) -> AccountResult<IndexMap<String, String>> {
        Ok(kept_computer_names(&*self.store, places).await?)
    }
    fn computer_name(&self, places: &[PlaceView], place_id: &str, kept: Option<&IndexMap<String, String>>) -> String {
        match kept {
            Some(kept) => usage_computer_name(places, place_id, kept),
            None => usage_computer_name(places, place_id, &IndexMap::new()),
        }
    }
    async fn spend_reset(&self, account: &str, credit_id: Option<&str>, on: Option<&str>) -> Result<ResetAnswer, HostError> {
        let ask = ResetAsk { account: account.to_string(), credit_id: credit_id.map(str::to_string), on: on.map(str::to_string) };
        self.usage_parts().resets.reset(ask).await
    }
    fn nap_refused_reason(&self, entry: &AccountEntry) -> Option<String> {
        self.reach.get().nap_refused_reason(entry)
    }
    fn stop_refused_reason(&self, entry: &AccountEntry) -> Option<String> {
        self.reach.get().stop_refused_reason(entry)
    }
    fn disk_reason(&self, entry: &AccountEntry) -> Option<String> {
        self.reach.get().disk_reason(entry)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// UsageResetDeps and Resets (usage-reset.ts:53-68, usage.ts:208-263, adapters.ts:85).

/// `PlanResets` as the reset area reads it.
struct PlanResetsOf(Arc<dyn PlanResets>);

impl Resets for PlanResetsOf {
    fn read_command(&self, road: &ResetRoad) -> Result<String, String> {
        self.0.read_command(road)
    }
    fn parse_read(&self, stdout: &str) -> Option<ResetReading> {
        self.0.parse_read(stdout)
    }
    fn spend_command(&self, road: &ResetRoad, idempotency_key: &str, credit_id: Option<&str>) -> Result<String, String> {
        self.0.spend_command(road, idempotency_key, credit_id)
    }
    fn parse_spend(&self, stdout: &str) -> ResetSpend {
        self.0.parse_spend(stdout)
    }
    fn too_old(&self) -> String {
        self.0.too_old().to_string()
    }
}

/// The context holds the resets' deps through a weak handle, so the resets it keeps do not hold it.
struct ResetDeps(Weak<Ctx>);

impl ResetDeps {
    fn ctx(&self) -> Result<Rc<Ctx>, HostError> {
        self.0.upgrade().ok_or_else(|| HostError::new("the runtime is gone"))
    }
}

fn reset_view(place: &ResetPlace) -> ResetPlaceView {
    let kind = match place.kind {
        ResetPlaceKind::Here => "here",
        ResetPlaceKind::Box => "box",
        ResetPlaceKind::Provider => "provider",
    };
    ResetPlaceView { id: place.id.clone(), name: place.name.clone(), kind: kind.to_string(), connected: place.connected }
}

fn reset_road_of(road: PlanRoad) -> ResetRoad {
    ResetRoad {
        home: road.home,
        env: Some(road.env.into_iter().map(|(k, v)| (k, Some(v))).collect()),
        launch: road.launch_program.map(|program| AgentLaunch { program: Some(program), args: None }),
    }
}

#[async_trait::async_trait(?Send)]
impl UsageResetDeps for ResetDeps {
    fn store(&self) -> Rc<dyn wsp_core::types::pending::Store> {
        UsageResetDeps::store(&*self.ctx().expect("the context is alive while a reset runs"))
    }
    async fn limits(&self) -> Result<Vec<AccountLimit>, HostError> {
        UsageResetDeps::limits(&*self.ctx()?).await
    }
    fn agent_name(&self, agent: &str) -> String {
        usage_agent_label(agent)
    }
    fn resets(&self, agent: &str) -> Option<Rc<dyn Resets>> {
        plan_resets_of(agent).map(|resets| Rc::new(PlanResetsOf(resets)) as Rc<dyn Resets>)
    }
    async fn places(&self) -> Result<PlaceOf, HostError> {
        UsageResetDeps::places(&*self.ctx()?).await
    }
    async fn road(&self, agent: &str, place: &ResetPlace) -> Result<ResetRoad, HostError> {
        UsageResetDeps::road(&*self.ctx()?, agent, place).await
    }
    async fn run(&self, place: &ResetPlace, script: &str, agent: &str) -> Result<String, HostError> {
        UsageResetDeps::run(&*self.ctx()?, place, script, agent).await
    }
    async fn file(&self, ask: FileAsk<'_>) -> Result<String, HostError> {
        UsageResetDeps::file(&*self.ctx()?, ask).await
    }
    async fn row(&self, key: &str) -> Result<Option<wsp_wire::types::usage::AccountRow>, HostError> {
        UsageResetDeps::row(&*self.ctx()?, key).await
    }
    fn uuid(&self) -> String {
        UsageResetDeps::uuid(&*self.ctx().expect("the context is alive while a reset runs"))
    }
}

#[async_trait::async_trait(?Send)]
impl UsageResetDeps for Ctx {
    fn store(&self) -> Rc<dyn wsp_core::types::pending::Store> {
        self.store.clone()
    }
    async fn limits(&self) -> Result<Vec<AccountLimit>, HostError> {
        self.usage_parts().ledger.limits().await
    }
    fn agent_name(&self, agent: &str) -> String {
        usage_agent_label(agent)
    }
    fn resets(&self, agent: &str) -> Option<Rc<dyn Resets>> {
        plan_resets_of(agent).map(|resets| Rc::new(PlanResetsOf(resets)) as Rc<dyn Resets>)
    }
    async fn places(&self) -> Result<PlaceOf, HostError> {
        let views = usage_places(self, self.clock.now()).await;
        let here = self.usage.get().here_sign_ins().await;
        let wired = self.opts.local.is_some();
        let door = list_places_of(self);
        Ok(Rc::new(move |id: &str| {
            let name = usage_computer_name(&views, id, &IndexMap::new());
            if id == HERE_PLACE_ID {
                let here = here.clone();
                return ResetPlace {
                    id: id.to_string(),
                    name,
                    kind: ResetPlaceKind::Here,
                    connected: wired,
                    signed_in: Rc::new(move |agent: &str| here.get(agent).map(String::as_str) == Some("signed-in")),
                };
            }
            match views.iter().find(|view| view.id == id) {
                Some(view) if is_joined_computer(&view.id, kind_word(view)) => {
                    let (at, held) = (id.to_string(), door.clone());
                    ResetPlace {
                        id: id.to_string(),
                        name,
                        kind: ResetPlaceKind::Box,
                        connected: door.as_ref().is_some_and(|door| door.link(id).is_some()),
                        signed_in: Rc::new(move |agent: &str| {
                            held.as_ref()
                                .and_then(|door| door.sign_ins_at(&at, None))
                                .is_some_and(|signs| signs.get(agent) == Some(&AgentSignInState::SignedIn))
                        }),
                    }
                }
                _ => ResetPlace {
                    id: id.to_string(),
                    name,
                    kind: ResetPlaceKind::Provider,
                    connected: false,
                    signed_in: Rc::new(|_: &str| false),
                },
            }
        }))
    }
    async fn road(&self, agent: &str, place: &ResetPlace) -> Result<ResetRoad, HostError> {
        let road = self.usage.get().plan_road(agent, &reset_view(place)).await?;
        Ok(reset_road_of(road))
    }
    async fn run(&self, place: &ResetPlace, script: &str, agent: &str) -> Result<String, HostError> {
        Ok(self.usage.get().reset_run(&reset_view(place), script, agent).await?)
    }
    async fn file(&self, ask: FileAsk<'_>) -> Result<String, HostError> {
        Ok(self.usage.get().file_plan_limit(ask.agent, &reset_view(ask.place), ask.limit.clone()).await?)
    }
    async fn row(&self, key: &str) -> Result<Option<wsp_wire::types::usage::AccountRow>, HostError> {
        let answer = self.usage.get().usage_accounts().await?;
        Ok(answer.accounts.into_iter().find(|row| row.key == key))
    }
    fn uuid(&self) -> String {
        self.ids.uuid()
    }
}

/// The skeleton's `impl Resets for Ctx`: the context answers as the one agent whose plan resets exist (codex), so
/// `every_trait_answers` can call a word of it.
impl Resets for Ctx {
    fn read_command(&self, road: &ResetRoad) -> Result<String, String> {
        codex_resets().read_command(road)
    }
    fn parse_read(&self, stdout: &str) -> Option<ResetReading> {
        codex_resets().parse_read(stdout)
    }
    fn spend_command(&self, road: &ResetRoad, idempotency_key: &str, credit_id: Option<&str>) -> Result<String, String> {
        codex_resets().spend_command(road, idempotency_key, credit_id)
    }
    fn parse_spend(&self, stdout: &str) -> ResetSpend {
        codex_resets().parse_spend(stdout)
    }
    fn too_old(&self) -> String {
        codex_resets().too_old().to_string()
    }
}

fn codex_resets() -> Arc<dyn PlanResets> {
    wsp_core::adapters::plan_resets().into_values().next().expect("the plan resets table holds codex")
}

// ---------------------------------------------------------------------------------------------------------------
// StatusCtx (status.ts:9-182).

/// `ctx.view(record)` as the JSON object.
fn view_map(ctx: &Ctx, record: &WorkspaceRecord) -> Result<Map<String, Value>, HostError> {
    let view = ctx.records.get().view(record)?;
    match serde_json::to_value(view).map_err(|e| HostError::new(e.to_string()))? {
        Value::Object(map) => Ok(map),
        _ => Ok(Map::new()),
    }
}

/// Every row `StatusArea::records` builds, joined with the callbacks its null stand-ins name (status.ts:13-43).
async fn status_records(ctx: &Rc<Ctx>, area: &Rc<StatusArea>) -> Result<Vec<StatusRecord>, HostError> {
    let rows = area.records().await?;
    let mut records = Vec::new();
    for row in rows {
        let Some(id) = row.get("id").and_then(Value::as_str).map(str::to_string) else { continue };
        let Some(entry) = ctx.live.borrow().get(&id).cloned() else { continue };
        let record = entry.borrow().record.borrow().clone();
        let has = |key: &str| row.contains_key(key);
        let provider = entry.clone();
        let exec_entry = entry.clone();
        let daemon_reach: Option<RecordCall<RouteReach>> = has("daemonReach").then(|| {
            let (ctx, entry) = (ctx.clone(), entry.clone());
            Rc::new(move || {
                let (ctx, entry) = (ctx.clone(), entry.clone());
                async move {
                    let kind = entry.borrow().record.borrow().kind;
                    let module = ctx.kinds.get().module_of(kind).map_err(|e| EngineError::Other { message: e.message, code: None })?;
                    let reading = module.daemon_road(&entry.borrow());
                    let road = reading.await.map_err(|e| EngineError::Other { message: e.message, code: None })?;
                    Ok(RouteReach { url: road.url, expires_at: road.expires_at.0 })
                }
                .boxed_local()
            }) as RecordCall<RouteReach>
        });
        let daemon_answers: Option<DaemonAnswers> = has("daemonAnswers").then(|| {
            let entry = entry.clone();
            Rc::new(move |timeout_ms: Option<i64>| {
                let machine = entry.borrow().machine.clone();
                async move { machine.daemon_answers(timeout_ms.map(|ms| ms.max(0) as u64)).await }.boxed_local()
            }) as DaemonAnswers
        });
        let metrics: Option<RecordCall<()>> = has("metrics").then(|| {
            let entry = entry.clone();
            Rc::new(move || {
                let machine = entry.borrow().machine.clone();
                async move { machine.metrics().await }.boxed_local()
            }) as RecordCall<()>
        });
        let facts = has("facts").then(|| {
            let entry = entry.clone();
            Rc::new(move || {
                let machine = entry.borrow().machine.clone();
                async move { machine.facts().await }.boxed_local()
            }) as RecordCall<wsp_engine::machine::MachineFacts>
        });
        let live = entry.borrow();
        let Ok(view) = ctx.records.get().view(&record) else { continue };
        records.push(StatusRecord {
            view,
            size: record.size.clone(),
            rate_usd_per_hour: row.get("rateUsdPerHour").and_then(Value::as_f64).unwrap_or(0.0),
            generation: live.generation as i64,
            idle_at: row.get("idleAt").and_then(Value::as_f64),
            reason: row.get("reason").and_then(Value::as_str).map(str::to_string),
            checkout: live.checkout.clone(),
            pr: live.pr.clone(),
            tree: None,
            wake_ask: row.get("wakeAsk").and_then(|v| serde_json::from_value(v.clone()).ok()),
            daemon_reach,
            daemon_answers,
            daemon_answers_prompt_ms: row.get("daemonAnswersPromptMs").and_then(Value::as_f64).map(|ms| ms as i64),
            provider_state: Rc::new(move || {
                let machine = provider.borrow().machine.clone();
                async move { machine.state().await }.boxed_local()
            }) as RecordCall<MachineState>,
            metrics,
            facts,
            exec: Rc::new(move |cmd: String, timeout_ms: Option<i64>| {
                let machine = exec_entry.borrow().machine.clone();
                async move {
                    machine.exec(&cmd, ExecOptions { timeout_ms: timeout_ms.map(|ms| ms.max(0) as u64), ..Default::default() }).await
                }
                .boxed_local()
            }) as RecordExec,
            away: row.get("away").and_then(Value::as_str).map(str::to_string),
            unreached: row.get("unreached").and_then(Value::as_str).map(str::to_string),
        });
    }
    Ok(records)
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::status::StatusCtx for Ctx {
    async fn ready(&self) {
        let _ = self.booted().await;
    }
    fn held(&self) -> Vec<AccountEntry> {
        self.rules.get().held()
    }
    fn view(&self, record: &WorkspaceRecord) -> Result<Map<String, Value>, HostError> {
        view_map(self, record)
    }
    fn rate_usd_per_hour(&self, record: &WorkspaceRecord) -> f64 {
        match self.kinds.get().backend_for(record) {
            Ok(backend) => backend.pricing().rate_usd_per_hour(Size { cpu: record.size.cpu.0, mem_mb: record.size.mem_mb.0 }),
            Err(_) => 0.0,
        }
    }
    fn row_reason(&self, entry: &AccountEntry) -> Option<String> {
        self.usage.get().row_reason(entry)
    }
    fn idle_at(&self, workspace_id: &str) -> Option<f64> {
        self.idle.get().idle_at(workspace_id).map(|at| at as f64)
    }
    fn away_line(&self, record: &WorkspaceRecord) -> Option<String> {
        self.usage.get().away_line(record)
    }
    fn unreached_of(&self, entry: &AccountEntry) -> Option<Value> {
        self.daemon.get().unreached_of(entry).map(Value::String)
    }
    fn reach_of(&self, entry: &AccountEntry) -> String {
        status_word(self.reach.get().reach_of(entry).unwrap_or(ReachState::Unsupported))
    }
    fn has_daemon(&self, entry: &AccountEntry) -> bool {
        let kind = entry.borrow().record.borrow().kind;
        match self.kinds.get().module_of(kind) {
            Ok(module) => module.has_daemon(&entry.borrow()),
            Err(_) => false,
        }
    }
    fn machine_reads(&self, entry: &AccountEntry) -> (bool, Option<f64>, bool, bool) {
        let machine = entry.borrow().machine.clone();
        let offers = machine.offers();
        (offers.daemon_answers, machine.daemon_answers_prompt_ms().map(|ms| ms as f64), offers.metrics, offers.facts)
    }
    fn polled_reach_set(&self, workspace_id: &str, machine_id: &str, reach: &str) {
        self.reach.get().polled_reach.borrow_mut().insert(
            workspace_id.to_string(),
            wsp_machines::machines::reach::ForMachine { machine_id: machine_id.to_string(), value: reach_state(reach) },
        );
    }
    fn polled_reach_delete(&self, workspace_id: &str) {
        self.reach.get().polled_reach.borrow_mut().shift_remove(workspace_id);
    }
    fn boot_work_has(&self, workspace_id: &str) -> bool {
        self.boot_work.borrow().contains_key(workspace_id)
    }
    fn revive_daemon(&self, entry: &AccountEntry, reach: &str) {
        if let Err(e) = self.daemon.get().revive_daemon(entry, reach_state(reach)) {
            wsp_core::console::warn(&e.message);
        }
    }
    fn offer_daemon_again(&self, entry: &AccountEntry, status: &Value) {
        if let Ok(status) = serde_json::from_value::<WorkspaceStatus>(status.clone()) {
            self.daemon.get().offer_daemon_again(entry, &status);
        }
    }
    fn read_version_again(&self, entry: &AccountEntry, status: &Value) {
        if let Ok(status) = serde_json::from_value::<WorkspaceStatus>(status.clone()) {
            self.daemon.get().read_version_again(entry, &status);
        }
    }
    async fn adopt_gone(&self, entry: &AccountEntry, reason: &str) -> AccountResult<()> {
        Ok(self.machines.get().adopt_gone(entry, reason).await?)
    }
    fn live(&self, workspace_id: &str) -> Option<AccountEntry> {
        self.live.borrow().get(workspace_id).cloned()
    }
    async fn store_get(&self, collection: &str, id: &str) -> AccountResult<Option<Value>> {
        Ok(self.store.get(collection, id).await?)
    }
    async fn store_put(&self, collection: &str, id: &str, value: Value) -> AccountResult<()> {
        Ok(self.store.put(collection, id, value).await?)
    }
    async fn store_list(&self, collection: &str) -> AccountResult<Vec<Value>> {
        Ok(self.store.list(collection).await?)
    }
    async fn store_delete(&self, collection: &str, id: &str) -> AccountResult<()> {
        Ok(self.store.delete(collection, id).await?)
    }
    fn now(&self) -> i64 {
        self.clock.now()
    }
    fn wall_now(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    }
    fn owner(&self) -> String {
        self.state.with(|state| state.owner.clone())
    }
    fn lost_workspace(&self, row: &ListedRow, owner: &str, now: i64) -> bool {
        let Ok(state) = serde_json::from_value::<MachineState>(json!(row.state)) else { return false };
        wsp_engine::orphans::lost_workspace(
            &ListedMachine { id: row.id.clone(), state, labels: row.labels.clone(), size: None },
            owner,
            now,
        )
    }
    fn project_held(&self, id: &str) -> bool {
        self.projects_held.borrow().contains_key(id)
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        self.create.get().name_refusal(name)
    }
    fn places_wired(&self) -> String {
        self.places.wired()
    }
    async fn golden_head(&self) -> Option<String> {
        let manifest = self.golden.get().get(None).await.ok().flatten();
        wsp_wire::views::golden_image::golden_head(manifest.as_ref()).map(|head| head.snapshot_id.clone())
    }
    async fn confirm(&self, place: &str, machine_id: &str) -> Result<Confirmed, ReadFailure> {
        let backend = self.places.backend(place).ok_or_else(|| ReadFailure::Failed(format!("no backend at {place}")))?;
        let machine = backend.get(machine_id).await.map_err(|e| missing_or_failed(&e))?;
        let machine = self.machines.get().observed(machine);
        let state = match machine.seen().map(|seen| seen.state) {
            Some(state) => state,
            None => machine.state().await.map_err(|e| ReadFailure::StateFailed(e.to_string()))?,
        };
        let shape = self.records.get().shape_of(machine.clone()).await;
        let default = backend.pricing().default_size();
        let asked = wsp_wire::wire::capabilities::WorkspaceSize {
            cpu: wsp_wire::types::js_number::JsNumber(default.cpu),
            mem_mb: wsp_wire::types::js_number::JsNumber(default.mem_mb),
        };
        let size = self.records.get().size_built(shape.as_ref(), &asked);
        let stream_url = machine.stream_url().map(str::to_string);
        Ok(Confirmed { state: status_word(state), stream_url, size: serde_json::to_value(size).unwrap_or(Value::Null), machine })
    }
    async fn stop_unrecorded(&self, place: &str, machine_id: &str) -> Result<(), ReadFailure> {
        let backend = self.places.backend(place).ok_or_else(|| ReadFailure::Failed(format!("no backend at {place}")))?;
        let machine = backend.get(machine_id).await.map_err(|e| missing_or_failed(&e))?;
        self.gone.stop(backend, machine).await.map_err(|e| missing_or_failed(&e))
    }
    fn attach(&self, record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Result<AccountEntry, HostError> {
        self.machines.get().attach(&Rc::new(RefCell::new(record)), machine)
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()> {
        Ok(self.records.get().persist(record).await?)
    }
    fn sync_daemon(&self, entry: &AccountEntry) {
        let started = self.daemon.get().sync_daemon(entry);
        spawn_detached(async move {
            let _ = started.await;
        });
    }
    fn emit(&self, event: Value) {
        match serde_json::from_value::<EventUnion>(event) {
            Ok(event) => {
                if let Err(e) = self.bus.emit(event) {
                    wsp_core::console::warn(&e.message);
                }
            }
            Err(e) => wsp_core::console::warn(&e.to_string()),
        }
    }
    async fn emit_status(&self, entry: &AccountEntry, reach: &str, reason: &str) {
        if let Err(e) = self.reach.get().emit_status(entry, reach_state(reach), Some(reason)).await {
            wsp_core::console::warn(&e.message());
        }
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
}
