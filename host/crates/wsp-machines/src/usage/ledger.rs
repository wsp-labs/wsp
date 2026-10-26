// SPDX-License-Identifier: AGPL-3.0-only
//! usage.ts:162-455 at 65f6b89c7 (`createUsageLedger`): the ledger of what was used, one blob a day, and each
//! account's last reading. Spec: `spec/usage_ledger.md`.
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use futures::{FutureExt, channel::oneshot};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use wsp_core::types::pending::{Clock, Store};
use wsp_wire::types::check;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::usage::{
    AccountLimit, AccountLimitCredits, BankedResets, UsageDay, UsageRange, UsageRow, UsageRowSource, UsageSplit, UsageTokens, UsedAnswer,
    UsedAnswerLinesItem, UsedAnswerLogs, UsedAnswerSeriesItem, UsedRow, UsedRowTokens,
};
use wsp_words::exit::HostError;
use wsp_words::js::{collate, utf16};
use wsp_words::js_date::{fields, DateZone};
use wsp_words::usage::{base_model, day_key_of, hour_of, price_of, rate_of, PricedTokens, RateTable};

use super::{LimitReading, UsageEntry, UsageSource, UsedQuery, ACCOUNT_LABELS, LIMITS, USAGE_DAYS, USAGE_INDEX, USAGE_RETENTION_DAYS};

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

/// `createUsageLedger`'s options (usage.ts:207).
#[derive(Clone)]
pub struct UsageLedgerOptions {
    pub store: Rc<dyn Store>,
    pub clock: Rc<dyn Clock>,
    /// An IANA zone; none is the system's.
    pub time_zone: Option<String>,
    pub prices: Rc<dyn Fn() -> LocalBoxFuture<'static, RateTable>>,
    pub retention_days: Option<u32>,
}

/// `limit`'s answer: the reading it replaced and the one now held.
#[derive(Clone, Debug, PartialEq)]
pub struct LimitKept {
    pub before: Option<AccountLimit>,
    pub after: AccountLimit,
}

/// `createUsageLedger(o)` (usage.ts:207).
pub struct UsageLedger {
    pub o: UsageLedgerOptions,
    /// `writes`: the chain every write takes its turn on (U2).
    pub writes: RefCell<Shared<LocalBoxFuture<'static, ()>>>,
}

fn plain(e: impl std::fmt::Display) -> HostError {
    HostError::new(e.to_string())
}

fn zone_of(time_zone: Option<&str>) -> DateZone {
    match time_zone {
        Some(name) => DateZone::named(name).expect("a time zone Intl takes"),
        None => DateZone::system(),
    }
}

fn zeros() -> UsageTokens {
    UsageTokens {
        input: JsNumber(0.0),
        output: JsNumber(0.0),
        cached: JsNumber(0.0),
        cache_write: JsNumber(0.0),
        reasoning: JsNumber(0.0),
    }
}

fn add_tokens(into: &mut UsageTokens, from: &UsageTokens) {
    into.input = JsNumber(into.input.0 + from.input.0);
    into.output = JsNumber(into.output.0 + from.output.0);
    into.cached = JsNumber(into.cached.0 + from.cached.0);
    into.cache_write = JsNumber(into.cache_write.0 + from.cache_write.0);
    into.reasoning = JsNumber(into.reasoning.0 + from.reasoning.0);
}

fn split_value(row: &UsageRow, split: UsageSplit) -> String {
    match split {
        UsageSplit::Agent => row.agent.clone(),
        UsageSplit::Account => row.account.clone(),
        UsageSplit::Computer => row.computer.clone(),
        UsageSplit::Project => row.project.clone(),
        UsageSplit::Model => base_model(&row.model),
        UsageSplit::Source => match row.source {
            UsageRowSource::Wsp => "wsp".to_string(),
            UsageRowSource::Log => "log".to_string(),
        },
    }
}

/// The splits whose every value is one agent's: an account signs in to one agent, and a model is filed per agent.
fn one_agent(split: UsageSplit) -> bool {
    matches!(split, UsageSplit::Agent | UsageSplit::Account | UsageSplit::Model)
}

fn days_of(range: UsageRange) -> i64 {
    match range {
        UsageRange::Day => 1,
        UsageRange::Week => 7,
        UsageRange::Month => 30,
    }
}

fn same_key(row: &UsageRow, hour: f64, entry: &UsageEntry, source: UsageRowSource) -> bool {
    let same_hour = row.hour.0 .0 == hour || (row.hour.0 .0.is_nan() && hour.is_nan());
    same_hour
        && row.agent == entry.agent
        && row.account == entry.account
        && row.computer == entry.computer
        && row.project == entry.project
        && row.model == entry.model.clone().unwrap_or_default()
        && row.source == source
}

/// usage.ts:163-173: the instant the zone's day holding `at` began.
pub fn day_start_of(at: i64, time_zone: Option<&str>) -> i64 {
    let f = fields(at, zone_of(time_zone).offset_seconds(at));
    let into_day = (i64::from(f.hour) * 60 + i64::from(f.minute)) * 60 * 1000 + i64::from(f.second) * 1000;
    at - into_day - at % 1000
}

/// A row as the TS fold's literal writes it: the key, then turns, tokens and the costs.
fn row_json(row: &UsageRow) -> Value {
    let mut m = Map::new();
    m.insert("day".to_string(), json!(row.day));
    m.insert("hour".to_string(), serde_json::to_value(row.hour).unwrap_or(Value::Null));
    m.insert("agent".to_string(), json!(row.agent));
    m.insert("account".to_string(), json!(row.account));
    m.insert("computer".to_string(), json!(row.computer));
    m.insert("project".to_string(), json!(row.project));
    m.insert("model".to_string(), json!(row.model));
    m.insert("source".to_string(), serde_json::to_value(row.source).unwrap_or(Value::Null));
    m.insert("turns".to_string(), serde_json::to_value(row.turns).unwrap_or(Value::Null));
    m.insert("tokens".to_string(), serde_json::to_value(&row.tokens).unwrap_or(Value::Null));
    if let Some(cost) = row.cost_reported {
        m.insert("costReported".to_string(), serde_json::to_value(cost).unwrap_or(Value::Null));
    }
    if let Some(unreported) = &row.unreported {
        m.insert("unreported".to_string(), serde_json::to_value(unreported).unwrap_or(Value::Null));
    }
    Value::Object(m)
}

/// A day's blob: the day, its rows, then the sessions a wsp turn ran under.
fn day_json(held: &UsageDay) -> Value {
    let mut m = Map::new();
    m.insert("day".to_string(), json!(held.day));
    m.insert("rows".to_string(), Value::Array(held.rows.iter().map(row_json).collect()));
    if let Some(sessions) = &held.sessions {
        m.insert("sessions".to_string(), json!(sessions));
    }
    Value::Object(m)
}

/// usage.ts:184-188 (U13).
pub fn credits_held(held: Option<&AccountLimitCredits>, read: Option<&BankedResets>, now: i64) -> Option<AccountLimitCredits> {
    let Some(read) = read else { return held.cloned() };
    let now = JsNumber(now as f64);
    if read.credits.is_some() {
        return Some(AccountLimitCredits { count: read.count, credits: read.credits.clone(), read_at: now, detail_at: Some(now) });
    }
    match held {
        Some(held) if held.count == read.count => Some(AccountLimitCredits { read_at: now, ..held.clone() }),
        _ => Some(AccountLimitCredits { count: read.count, credits: None, read_at: now, detail_at: None }),
    }
}

impl UsageLedger {
    pub fn new(o: UsageLedgerOptions) -> Rc<UsageLedger> {
        Rc::new(UsageLedger { o, writes: RefCell::new(futures::future::ready(()).boxed_local().shared()) })
    }

    fn zone(&self) -> Option<&str> {
        self.o.time_zone.as_deref()
    }

    fn keep_days(&self) -> u32 {
        self.o.retention_days.unwrap_or(USAGE_RETENTION_DAYS)
    }

    /// Takes this write's turn after every write asked before it; a failure stays with its own caller.
    async fn in_turn<T>(&self, work: impl Future<Output = Result<T, HostError>>) -> Result<T, HostError> {
        let (done, released) = oneshot::channel::<()>();
        let next = async move {
            let _ = released.await;
        }
        .boxed_local()
        .shared();
        let prev = std::mem::replace(&mut *self.writes.borrow_mut(), next);
        prev.await;
        let result = work.await;
        let _ = done.send(());
        result
    }

    async fn read_day(&self, day: &str) -> Result<Option<UsageDay>, HostError> {
        let Some(bytes) = self.o.store.get_blob(USAGE_DAYS, day).await? else {
            return Ok(None);
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(plain)?;
        Ok(check::parse::<UsageDay>(&value).ok())
    }

    async fn index(&self) -> Result<Vec<String>, HostError> {
        let held = self.o.store.get(USAGE_INDEX, "days").await?;
        Ok(held
            .as_ref()
            .and_then(|v| v.get("days"))
            .and_then(Value::as_array)
            .map(|days| days.iter().filter_map(|d| d.as_str().map(str::to_string)).collect())
            .unwrap_or_default())
    }

    /// usage.ts:273-282 (U5): the day's blob, then retention over the index, then the index.
    async fn write(&self, held: &UsageDay) -> Result<(), HostError> {
        let bytes = serde_json::to_vec(&day_json(held)).map_err(plain)?;
        self.o.store.put_blob(USAGE_DAYS, &held.day, bytes).await?;
        let oldest = day_key_of((self.o.clock.now() - (i64::from(self.keep_days()) - 1) * DAY) as f64, self.zone());
        let mut days = self.index().await?;
        if !days.contains(&held.day) {
            days.push(held.day.clone());
        }
        days.sort_by(|a, b| utf16::cmp(a, b));
        days.dedup();
        for gone in days.iter().filter(|d| d.as_str() < oldest.as_str()) {
            self.o.store.delete_blob(USAGE_DAYS, gone).await?;
        }
        let kept: Vec<String> = days.into_iter().filter(|d| d.as_str() >= oldest.as_str()).collect();
        self.o.store.put(USAGE_INDEX, "days", json!({ "days": kept })).await
    }

    /// usage.ts:284-289 (U6).
    async fn name_accounts(&self, named: &[(String, String)]) -> Result<(), HostError> {
        let mut latest: IndexMap<String, String> = IndexMap::new();
        for (key, label) in named {
            latest.insert(key.clone(), label.clone());
        }
        for (key, label) in latest {
            let held = self.o.store.get(ACCOUNT_LABELS, &key).await?;
            if held.as_ref().and_then(|v| v.get("label")).and_then(Value::as_str) != Some(label.as_str()) {
                self.o.store.put(ACCOUNT_LABELS, &key, json!({ "key": key, "label": label })).await?;
            }
        }
        Ok(())
    }

    /// usage.ts:230-270 (U4): one entry into a day's rows; a session a wsp turn ran under is noted once.
    fn fold(&self, held: &mut UsageDay, entry: &UsageEntry, source: UsageSource) {
        let hour = hour_of(entry.at as f64, self.zone());
        let row_source = match source {
            UsageSource::Wsp => UsageRowSource::Wsp,
            UsageSource::Log => UsageRowSource::Log,
        };
        let t = entry.tokens.clone().unwrap_or_default();
        let tokens = UsageTokens {
            input: JsNumber(t.input.unwrap_or(0.0)),
            output: JsNumber(t.output.unwrap_or(0.0)),
            cached: JsNumber(t.cached.unwrap_or(0.0)),
            cache_write: JsNumber(t.cache_write.unwrap_or(0.0)),
            reasoning: JsNumber(t.reasoning.unwrap_or(0.0)),
        };
        let turns = entry.turns.unwrap_or(1.0);
        match held.rows.iter().position(|r| same_key(r, hour, entry, row_source)) {
            None => held.rows.push(UsageRow {
                day: held.day.clone(),
                hour: JsInt(JsNumber(hour)),
                agent: entry.agent.clone(),
                account: entry.account.clone(),
                computer: entry.computer.clone(),
                project: entry.project.clone(),
                model: entry.model.clone().unwrap_or_default(),
                turns: JsInt(JsNumber(turns)),
                tokens: tokens.clone(),
                cost_reported: entry.cost_usd.map(JsNumber),
                unreported: None,
                source: row_source,
            }),
            Some(i) => {
                let found = &mut held.rows[i];
                if entry.cost_usd.is_some() && found.cost_reported.is_none() {
                    found.unreported = Some(found.tokens.clone());
                } else if entry.cost_usd.is_none() && found.cost_reported.is_some() {
                    add_tokens(found.unreported.get_or_insert_with(zeros), &tokens);
                }
                found.turns = JsInt(JsNumber(found.turns.0 .0 + turns));
                add_tokens(&mut found.tokens, &tokens);
                if let Some(cost) = entry.cost_usd {
                    found.cost_reported = Some(JsNumber(found.cost_reported.map_or(0.0, |c| c.0) + cost));
                }
            }
        }
        if source == UsageSource::Wsp {
            if let Some(session) = &entry.session {
                if !held.sessions.as_deref().unwrap_or(&[]).contains(session) {
                    held.sessions.get_or_insert_with(Vec::new).push(session.clone());
                }
            }
        }
    }

    /// usage.ts:291-298 (U7).
    pub async fn add(self: &Rc<Self>, entry: UsageEntry) -> Result<(), HostError> {
        self.in_turn(async {
            self.name_accounts(&[(entry.account.clone(), entry.account_label.clone())]).await?;
            let day = day_key_of(entry.at as f64, self.zone());
            let mut held = self.read_day(&day).await?.unwrap_or_else(|| UsageDay { day: day.clone(), rows: Vec::new(), sessions: None });
            self.fold(&mut held, &entry, entry.source);
            self.write(&held).await
        })
        .await
    }

    /// usage.ts:300-329 (U8).
    pub async fn file_logs(self: &Rc<Self>, entries: Vec<UsageEntry>, computers: Vec<String>) -> Result<(), HostError> {
        self.in_turn(async {
            let named: Vec<(String, String)> = entries.iter().map(|e| (e.account.clone(), e.account_label.clone())).collect();
            self.name_accounts(&named).await?;
            let oldest = day_key_of((self.o.clock.now() - (i64::from(self.keep_days()) - 1) * DAY) as f64, self.zone());
            let mut days: IndexMap<String, UsageDay> = IndexMap::new();
            for day in self.index().await? {
                if let Some(held) = self.read_day(&day).await? {
                    days.insert(day, held);
                }
            }
            let ran: IndexSet<String> = days.values().flat_map(|d| d.sessions.clone().unwrap_or_default()).collect();
            let read: IndexSet<String> = computers.into_iter().collect();
            let from_read = |r: &UsageRow| r.source == UsageRowSource::Log && read.contains(&r.computer);
            let mut touched: IndexSet<String> = IndexSet::new();
            for held in days.values_mut() {
                if !held.rows.iter().any(from_read) {
                    continue;
                }
                held.rows.retain(|r| !from_read(r));
                touched.insert(held.day.clone());
            }
            for entry in &entries {
                if entry.session.as_ref().is_some_and(|s| ran.contains(s)) {
                    continue;
                }
                let day = day_key_of(entry.at as f64, self.zone());
                if day.as_str() < oldest.as_str() {
                    continue;
                }
                let held = days.entry(day.clone()).or_insert_with(|| UsageDay { day: day.clone(), rows: Vec::new(), sessions: None });
                self.fold(held, entry, UsageSource::Log);
                touched.insert(day);
            }
            for day in touched {
                if let Some(held) = days.get(&day) {
                    self.write(held).await?;
                }
            }
            Ok(())
        })
        .await
    }

    /// usage.ts:331-410 (U9 to U11).
    pub async fn used(&self, q: UsedQuery) -> Result<UsedAnswer, HostError> {
        let pending = self.writes.borrow().clone();
        pending.await;
        let zone = self.zone();
        let now = self.o.clock.now();
        let today = day_start_of(now, zone);
        let count = days_of(q.range);
        let since = if count == 1 { today } else { day_start_of(today - (count - 1) * DAY + HOUR * 12, zone) };
        let starts: Vec<i64> = (0..count).map(|i| day_start_of(today - (count - 1 - i) * DAY + HOUR * 12, zone)).collect();
        let mut table: Option<RateTable> = None;
        let outside = q.outside.unwrap_or(true);
        let mut rows: IndexMap<String, UsedRow> = IndexMap::new();
        let mut logged: IndexSet<String> = IndexSet::new();
        let mut logged_on: IndexSet<String> = IndexSet::new();
        let mut series: Vec<UsedAnswerSeriesItem> = if count == 1 {
            (0..24).map(|h| UsedAnswerSeriesItem { t: JsNumber((today + h * HOUR) as f64), tokens: JsNumber(0.0) }).collect()
        } else {
            starts.iter().map(|t| UsedAnswerSeriesItem { t: JsNumber(*t as f64), tokens: JsNumber(0.0) }).collect()
        };
        let mut points: IndexMap<String, Vec<f64>> = IndexMap::new();
        for (at, start) in starts.iter().enumerate() {
            let Some(held) = self.read_day(&day_key_of((start + HOUR * 12) as f64, zone)).await? else {
                continue;
            };
            for row in &held.rows {
                if row.source == UsageRowSource::Log && !outside {
                    continue;
                }
                if row.source == UsageRowSource::Log {
                    logged.insert(row.agent.clone());
                    logged_on.insert(row.computer.clone());
                }
                let value = split_value(row, q.split);
                let key = if q.split == UsageSplit::Model { format!("{}:{value}", row.agent) } else { value.clone() };
                let line = rows.entry(key.clone()).or_insert_with(|| UsedRow {
                    key: key.clone(),
                    label: (q.label)(q.split, &value),
                    agent: one_agent(q.split).then(|| row.agent.clone()),
                    tokens: UsedRowTokens { input: JsNumber(0.0), output: JsNumber(0.0), cached: JsNumber(0.0), cache_write: Some(JsNumber(0.0)), reasoning: Some(JsNumber(0.0)) },
                    cost_reported: None,
                    cost_list: None,
                    priced: true,
                    estimate: None,
                    saved: None,
                    turns: None,
                });
                line.tokens.input = JsNumber(line.tokens.input.0 + row.tokens.input.0);
                line.tokens.output = JsNumber(line.tokens.output.0 + row.tokens.output.0);
                line.tokens.cached = JsNumber(line.tokens.cached.0 + row.tokens.cached.0);
                line.tokens.cache_write = Some(JsNumber(line.tokens.cache_write.map_or(0.0, |v| v.0) + row.tokens.cache_write.0));
                line.tokens.reasoning = Some(JsNumber(line.tokens.reasoning.map_or(0.0, |v| v.0) + row.tokens.reasoning.0));
                if row.source == UsageRowSource::Wsp {
                    line.turns = Some(JsInt(JsNumber(line.turns.map_or(0.0, |t| t.0 .0) + row.turns.0 .0)));
                }
                if table.is_none() {
                    table = Some((self.o.prices)().await);
                }
                let rates = table.as_ref().expect("the table is read just above");
                let listed = price_of(&row.model, &tokens_priced(&row.tokens), rates);
                let rate = rate_of(&row.model, rates);
                if let (Some(listed), Some(rate)) = (listed, rate.as_ref()) {
                    line.estimate = Some(JsNumber(line.estimate.map_or(0.0, |e| e.0) + listed));
                    line.saved = Some(JsNumber(line.saved.map_or(0.0, |s| s.0) + row.tokens.cached.0 * (rate.input - rate.cache_read.unwrap_or(rate.input))));
                }
                if let Some(cost) = row.cost_reported {
                    line.cost_reported = Some(JsNumber(line.cost_reported.map_or(0.0, |c| c.0) + cost.0));
                }
                let bare = if row.cost_reported.is_none() { Some(row.tokens.clone()) } else { row.unreported.clone() };
                let bare_listed = bare.as_ref().and_then(|b| price_of(&row.model, &tokens_priced(b), rates));
                match (bare_listed, bare.as_ref()) {
                    (Some(listed), _) => line.cost_list = Some(JsNumber(line.cost_list.map_or(0.0, |c| c.0) + listed)),
                    (None, Some(b)) if b.input.0 + b.output.0 > 0.0 => line.priced = false,
                    _ => {}
                }
                let total = row.tokens.input.0 + row.tokens.output.0;
                let step = if count == 1 { row.hour.0 .0 as usize } else { at };
                if let Some(item) = series.get_mut(step) {
                    item.tokens = JsNumber(item.tokens.0 + total);
                    let line_points = points.entry(key).or_insert_with(|| vec![0.0; series_len(count)]);
                    if let Some(p) = line_points.get_mut(step) {
                        *p += total;
                    }
                }
            }
        }
        let mut ordered: Vec<UsedRow> = rows.into_values().collect();
        let total_of = |r: &UsedRow| r.tokens.input.0 + r.tokens.output.0;
        ordered.sort_by(|a, b| total_of(b).partial_cmp(&total_of(a)).unwrap_or(std::cmp::Ordering::Equal).then_with(|| collate(&a.key, &b.key)));
        let logs = if logged.is_empty() {
            None
        } else {
            let mut agents: Vec<String> = logged.iter().map(|a| (q.label)(UsageSplit::Agent, a)).collect();
            agents.sort_by(|a, b| utf16::cmp(a, b));
            let mut computers: Vec<String> = logged_on.iter().map(|c| (q.label)(UsageSplit::Computer, c)).collect();
            computers.sort_by(|a, b| utf16::cmp(a, b));
            Some(UsedAnswerLogs { agents, computers })
        };
        let lines: Vec<UsedAnswerLinesItem> = ordered
            .iter()
            .filter(|r| total_of(r) > 0.0)
            .map(|r| UsedAnswerLinesItem {
                key: r.key.clone(),
                label: r.label.clone(),
                points: points.get(&r.key).cloned().unwrap_or_else(|| vec![0.0; series_len(count)]).into_iter().map(JsNumber).collect(),
            })
            .collect();
        Ok(UsedAnswer {
            range: q.range,
            split: q.split,
            rows: ordered,
            series,
            lines: Some(lines),
            since: JsNumber(since as f64),
            until: JsNumber(day_start_of(today + DAY + HOUR * 12, zone) as f64),
            logs,
        })
    }

    /// usage.ts:218-223 (U3).
    pub async fn day(&self, day: &str) -> Result<Option<UsageDay>, HostError> {
        self.read_day(day).await
    }

    /// usage.ts:224-227 (U3).
    pub async fn days(&self) -> Result<Vec<String>, HostError> {
        self.index().await
    }

    /// usage.ts:412-437 (U12).
    pub async fn limit(self: &Rc<Self>, r: LimitReading) -> Result<LimitKept, HostError> {
        self.in_turn(async {
            let held = self.o.store.get(LIMITS, &r.key).await?.and_then(|raw| check::parse::<AccountLimit>(&raw).ok());
            if let (Some(held_now), Some(at)) = (held.as_ref(), r.at) {
                if held_now.read_at.0 >= at as f64 {
                    return Ok(LimitKept { before: Some(held_now.clone()), after: held_now.clone() });
                }
            }
            let now = self.o.clock.now();
            let plan = r.limit.plan.clone().or_else(|| held.as_ref().and_then(|h| h.plan.clone()));
            let status = r.limit.status.or_else(|| held.as_ref().and_then(|h| h.status));
            let keyed = r.limit.keyed.or_else(|| held.as_ref().and_then(|h| h.keyed));
            let credits = if keyed == Some(true) { None } else { credits_held(held.as_ref().and_then(|h| h.credits.as_ref()), r.limit.credits.as_ref(), now) };
            self.name_accounts(&[(r.key.clone(), r.label.clone())]).await?;
            let mut computers: Vec<String> = held.as_ref().map(|h| h.computers.clone()).unwrap_or_default();
            if !computers.contains(&r.computer) {
                computers.push(r.computer.clone());
            }
            let windows = if !r.limit.windows.is_empty() || r.limit.keyed == Some(true) {
                r.limit.windows.clone()
            } else {
                held.as_ref().map(|h| h.windows.clone()).unwrap_or_default()
            };
            let next = AccountLimit {
                key: r.key.clone(),
                agent: r.agent.clone(),
                label: r.label.clone(),
                road: Some(r.road),
                plan,
                windows,
                status,
                keyed,
                read_at: JsNumber(r.at.unwrap_or(now) as f64),
                computers,
                credits,
            };
            self.o.store.put(LIMITS, &r.key, serde_json::to_value(&next).map_err(plain)?).await?;
            Ok(LimitKept { before: held, after: next })
        })
        .await
    }

    /// usage.ts:439-443 (U14).
    pub async fn limits(&self) -> Result<Vec<AccountLimit>, HostError> {
        Ok(self.o.store.list(LIMITS).await?.iter().filter_map(|raw| check::parse::<AccountLimit>(raw).ok()).collect())
    }

    /// usage.ts:445-452 (U14).
    pub async fn account_labels(&self) -> Result<IndexMap<String, String>, HostError> {
        let mut labels = IndexMap::new();
        for raw in self.o.store.list(ACCOUNT_LABELS).await? {
            let key = raw.get("key").and_then(Value::as_str);
            let label = raw.get("label").and_then(Value::as_str);
            if let (Some(key), Some(label)) = (key, label) {
                labels.insert(key.to_string(), label.to_string());
            }
        }
        Ok(labels)
    }
}

fn series_len(count: i64) -> usize {
    if count == 1 { 24 } else { count as usize }
}

fn tokens_priced(t: &UsageTokens) -> PricedTokens {
    PricedTokens { input: t.input.0, output: t.output.0, cached: Some(t.cached.0), cache_write: Some(t.cache_write.0) }
}
