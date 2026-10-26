// SPDX-License-Identifier: AGPL-3.0-only
//! usage.ts:111-160, 181-201, 457-618 at 65f6b89c7: accounts and their labels, computer names, banked resets, the
//! burn, the price table and the account rows. Spec: `spec/usage_accounts.md`.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::{FutureExt, LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};

use wsp_core::types::pending::{Clock, Store};
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::usage::{
    AccountLimit, AccountLimitCredits, AccountNote, AccountRoad, AccountRow, AccountRowBurn,
    AccountRowCredits, ResetCreditStatus, PRICES_TTL_MS, USAGE_WORDS,
};
use wsp_wire::views::place_more::PlaceView;
use wsp_words::exit::HostError;
use wsp_words::js::collate;
use wsp_words::place_name::{here_name, place_name};
use wsp_words::place_word::{provider_key_name, provider_key_words};
use wsp_words::usage::{account_words, parse_rate_table, AccountWho, Rate, RateTable};

use super::{Vaulted, BURN_WINDOW_MS, COMPUTER_NAMES, PRICES, RESET_DETAILS_MS};
use crate::unwound::unwound;

/// An agent's account as the usage records key and name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub key: String,
    pub label: String,
    pub road: AccountRoad,
}

/// The account a harness named: its id and label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedAccount {
    pub id: String,
    pub label: Option<String>,
}

/// A computer by id and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputerName {
    pub id: String,
    pub name: String,
}

fn word(key: &str) -> String {
    USAGE_WORDS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| (*v).to_string())
        .expect("a word the usage table holds")
}

fn who(agent_name: &str) -> AccountWho {
    AccountWho {
        agent_name: agent_name.to_string(),
        ..AccountWho::default()
    }
}

/// usage.ts:117-122 (C1).
pub fn account_of(
    agent: &str,
    agent_name: &str,
    named: Option<&NamedAccount>,
    vaulted: Option<Vaulted>,
    computer: &ComputerName,
) -> Account {
    if let Some(named) = named {
        return Account {
            key: format!("{agent}:{}", named.id),
            label: named.label.clone().unwrap_or_else(|| named.id.clone()),
            road: AccountRoad::Named,
        };
    }
    match vaulted {
        Some(Vaulted::Token) => Account {
            key: format!("{agent}:vault-token"),
            label: account_words(&AccountWho {
                vaulted: Some(true),
                ..who(agent_name)
            }),
            road: AccountRoad::Vault,
        },
        Some(Vaulted::Key) => Account {
            key: format!("{agent}:vault-key"),
            label: account_words(&AccountWho {
                keyed: Some(true),
                ..who(agent_name)
            }),
            road: AccountRoad::Vault,
        },
        None => Account {
            key: format!("{agent}@{}", computer.id),
            label: account_words(&AccountWho {
                own_on: Some(computer.name.clone()),
                ..who(agent_name)
            }),
            road: AccountRoad::Own,
        },
    }
}

/// usage.ts:127-130 (C2): the key and label.
pub fn account_on_computer(
    agent: &str,
    agent_name: &str,
    computer: &ComputerName,
    limits: &[AccountLimit],
    vaulted: Option<Vaulted>,
) -> (String, String) {
    let ran = |vault: bool| {
        limits
            .iter()
            .find(|l| {
                l.agent == agent
                    && (l.road == Some(AccountRoad::Vault)) == vault
                    && l.computers.contains(&computer.id)
            })
            .map(|l| (l.key.clone(), l.label.clone()))
    };
    ran(false)
        .or_else(|| {
            if vaulted == Some(Vaulted::Key) {
                ran(true)
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            let account = account_of(agent, agent_name, None, None, computer);
            (account.key, account.label)
        })
}

/// usage.ts:135-140 (C3).
pub fn usage_computer_name(
    places: &[PlaceView],
    id: &str,
    kept: &IndexMap<String, String>,
) -> String {
    if let Some(place) = places.iter().find(|p| p.id == id) {
        return place_name(place);
    }
    if id == HERE_PLACE_ID {
        let here = here_name(places);
        return if here.is_empty() {
            wsp_words::words::computer::THIS_COMPUTER.to_string()
        } else {
            here
        };
    }
    if provider_key_words(id).is_some() {
        return provider_key_name(id);
    }
    kept.get(id)
        .cloned()
        .unwrap_or_else(|| word("removedComputer"))
}

/// usage.ts:146-160 (C4).
pub async fn kept_computer_names(
    store: &dyn Store,
    places: &[PlaceView],
) -> Result<IndexMap<String, String>, HostError> {
    let mut kept: IndexMap<String, String> = IndexMap::new();
    for raw in store.list(COMPUTER_NAMES).await? {
        if let (Some(id), Some(name)) = (
            raw.get("id").and_then(Value::as_str),
            raw.get("name").and_then(Value::as_str),
        ) {
            kept.insert(id.to_string(), name.to_string());
        }
    }
    for place in places {
        if place.id == HERE_PLACE_ID || provider_key_words(&place.id).is_some() {
            continue;
        }
        let name = place_name(place);
        if kept.get(&place.id) == Some(&name) {
            continue;
        }
        kept.insert(place.id.clone(), name.clone());
        store
            .put(
                COMPUTER_NAMES,
                &place.id,
                json!({ "id": place.id, "name": name }),
            )
            .await?;
    }
    Ok(kept)
}

/// usage.ts:194-195 (C5).
pub fn reset_details_due(held: Option<&AccountLimit>, now: i64) -> bool {
    let keyed = held.and_then(|h| h.keyed) == Some(true);
    let detail_at = held
        .and_then(|h| h.credits.as_ref())
        .and_then(|c| c.detail_at);
    !keyed && detail_at.is_none_or(|at| now as f64 - at.0 >= RESET_DETAILS_MS as f64)
}

/// usage.ts:198-201 (C6).
pub fn row_credits(held: &AccountLimitCredits) -> AccountRowCredits {
    let lapses = held
        .credits
        .iter()
        .flatten()
        .filter(|c| c.status == ResetCreditStatus::Available)
        .filter_map(|c| c.expires_at.map(|e| e.0));
    let next = lapses.fold(None, |least: Option<f64>, at| {
        Some(least.map_or(at, |l| l.min(at)))
    });
    AccountRowCredits {
        count: held.count,
        credits: held.credits.clone(),
        next_expires_at: next.map(JsNumber),
        read_at: held.read_at,
        detail_at: held.detail_at,
    }
}

/// usage.ts:462-465 `ReplayedStamp`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayedStamp {
    pub run: String,
    pub at: i64,
}

/// One call the burn files.
#[derive(Clone, Debug, PartialEq)]
pub struct BurnCall {
    pub account: String,
    pub thread_id: String,
    pub tokens: f64,
    pub replayed: Option<ReplayedStamp>,
}

/// `createBurn(clock)` (usage.ts:472-499) (C7). Memory only.
pub struct Burn {
    pub clock: Rc<dyn Clock>,
    pub calls: RefCell<Vec<(i64, BurnCall)>>,
    /// Each re-read run's newest stamp and when it arrived.
    pub newest: RefCell<IndexMap<String, (i64, i64)>>,
}

/// usage.ts:476-479: when a call is filed, by its age against its run's newest stamp where it was replayed.
fn filed_at(received: i64, call: &BurnCall, newest: &IndexMap<String, (i64, i64)>) -> i64 {
    match call
        .replayed
        .as_ref()
        .and_then(|r| newest.get(&r.run).map(|top| (r, top)))
    {
        Some((replayed, &(top_at, top_received))) => {
            received.min(top_received.saturating_sub(top_at.saturating_sub(replayed.at)))
        }
        None => received,
    }
}

impl Burn {
    pub fn new(clock: Rc<dyn Clock>) -> Burn {
        Burn {
            clock,
            calls: RefCell::default(),
            newest: RefCell::default(),
        }
    }

    /// usage.ts:480-486: drops the calls filed at or before the window's start, and the newest stamps of runs with none left.
    fn recent(&self) {
        let since = self.clock.now() - BURN_WINDOW_MS;
        {
            let newest = self.newest.borrow();
            self.calls
                .borrow_mut()
                .retain(|(received, call)| filed_at(*received, call, &newest) > since);
        }
        let runs: IndexSet<String> = self
            .calls
            .borrow()
            .iter()
            .filter_map(|(_, c)| c.replayed.as_ref().map(|r| r.run.clone()))
            .collect();
        self.newest.borrow_mut().retain(|run, _| runs.contains(run));
    }

    pub fn add(&self, call: BurnCall) {
        let received = self.clock.now();
        if let Some(replayed) = &call.replayed {
            let held_at = self.newest.borrow().get(&replayed.run).map(|top| top.0);
            if held_at.is_none_or(|at| replayed.at >= at) {
                self.newest
                    .borrow_mut()
                    .insert(replayed.run.clone(), (replayed.at, received));
            }
        }
        self.recent();
        self.calls.borrow_mut().push((received, call));
    }

    pub fn of(&self, account: &str) -> Option<AccountRowBurn> {
        self.recent();
        let calls = self.calls.borrow();
        let on: Vec<&BurnCall> = calls
            .iter()
            .filter(|(_, c)| c.account == account)
            .map(|(_, c)| c)
            .collect();
        if on.is_empty() {
            return None;
        }
        let tokens: f64 = on.iter().fold(0.0, |n, c| n + c.tokens);
        let threads: IndexSet<&str> = on.iter().map(|c| c.thread_id.as_str()).collect();
        Some(AccountRowBurn {
            tokens_per_minute: JsNumber(tokens / (BURN_WINDOW_MS as f64 / 60_000.0)),
            threads: JsInt(JsNumber(threads.len() as f64)),
        })
    }
}

/// `createPriceTable(o)` (usage.ts:503-537) (C8).
pub struct PriceTable {
    pub store: Rc<dyn Store>,
    pub clock: Rc<dyn Clock>,
    pub fetch: Rc<dyn Fn() -> LocalBoxFuture<'static, Result<serde_json::Value, HostError>>>,
    pub warn: Rc<dyn Fn(&str)>,
    /// `held`: the table and when it was fetched.
    pub held: RefCell<Option<(i64, RateTable)>>,
    /// `reading`: the one read in flight.
    pub reading: RefCell<Option<Shared<LocalBoxFuture<'static, RateTable>>>>,
}

impl PriceTable {
    pub fn new(
        store: Rc<dyn Store>,
        clock: Rc<dyn Clock>,
        fetch: Rc<dyn Fn() -> LocalBoxFuture<'static, Result<serde_json::Value, HostError>>>,
        warn: Rc<dyn Fn(&str)>,
    ) -> Rc<PriceTable> {
        Rc::new(PriceTable {
            store,
            clock,
            fetch,
            warn,
            held: RefCell::default(),
            reading: RefCell::default(),
        })
    }

    fn fresh(&self) -> Option<RateTable> {
        let held = self.held.borrow();
        held.as_ref()
            .filter(|(at, _)| self.clock.now().saturating_sub(*at) < PRICES_TTL_MS as i64)
            .map(|(_, table)| table.clone())
    }

    /// usage.ts:507-529.
    async fn refresh(&self) -> RateTable {
        if self.held.borrow().is_none() {
            if let Ok(Some(bytes)) = self.store.get_blob(PRICES, "litellm").await {
                let kept: Option<(i64, RateTable)> =
                    serde_json::from_slice::<Value>(&bytes).ok().and_then(|v| {
                        let at = v.get("fetchedAt")?.as_f64()?;
                        Some((at as i64, stored_table(v.get("table")?)?))
                    });
                if let Some(kept) = kept {
                    *self.held.borrow_mut() = Some(kept);
                }
            }
            if let Some(table) = self.fresh() {
                return table;
            }
        }
        match (self.fetch)().await {
            Ok(document) => {
                let table = parse_rate_table(&document);
                if !table.is_empty() {
                    let at = self.clock.now();
                    *self.held.borrow_mut() = Some((at, table.clone()));
                    let kept = json!({ "fetchedAt": at, "table": table });
                    if let Err(e) = self
                        .store
                        .put_blob(PRICES, "litellm", kept.to_string().into_bytes())
                        .await
                    {
                        self.warn_unread(&e.message);
                    }
                }
            }
            Err(e) => self.warn_unread(&e.message),
        }
        self.held
            .borrow()
            .as_ref()
            .map(|(_, table)| table.clone())
            .unwrap_or_default()
    }

    fn warn_unread(&self, message: &str) {
        let fallback = if self.held.borrow().is_none() {
            "no row is priced"
        } else {
            "the last one read is used"
        };
        (self.warn)(&format!(
            "the price table was not read ({message}); {fallback}"
        ));
    }

    pub async fn get(self: &Rc<Self>) -> RateTable {
        if let Some(table) = self.fresh() {
            return table;
        }
        let reading = {
            let mut slot = self.reading.borrow_mut();
            match slot.as_ref() {
                Some(reading) => reading.clone(),
                None => {
                    let me = self.clone();
                    let read = async move {
                        let table = unwound(me.refresh()).await;
                        *me.reading.borrow_mut() = None;
                        table.unwrap_or_default()
                    }
                    .boxed_local()
                    .shared();
                    *slot = Some(read.clone());
                    read
                }
            }
        };
        reading.await
    }
}

/// The rate table a kept blob holds, read as the TS reads it: each model's entry with the rates it has.
fn stored_table(value: &Value) -> Option<RateTable> {
    let entries = value.as_object()?;
    Some(
        entries
            .iter()
            .filter_map(|(model, raw)| {
                let raw = raw.as_object()?;
                let rate = Rate {
                    input: raw.get("input")?.as_f64()?,
                    output: raw.get("output")?.as_f64()?,
                    cache_read: raw.get("cacheRead").and_then(Value::as_f64),
                    cache_write: raw.get("cacheWrite").and_then(Value::as_f64),
                    provider: raw
                        .get("provider")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                };
                Some((model.clone(), rate))
            })
            .collect(),
    )
}

/// What `accountNames` and `accountRows` read (usage.ts:543, 564).
#[derive(Clone)]
pub struct AccountRowsAsk {
    pub limits: Vec<AccountLimit>,
    pub places: Vec<SignedInPlace>,
    pub name_of: Rc<dyn Fn(&str) -> String>,
    pub agent_name: Rc<dyn Fn(&str) -> String>,
    pub plan_brand: Option<Rc<dyn Fn(&str) -> Option<String>>>,
    pub vaulted: Rc<dyn Fn(&str) -> Option<Vaulted>>,
    pub prints_limits: Rc<dyn Fn(&str) -> bool>,
    pub burn: Option<Rc<dyn Fn(&str) -> Option<AccountRowBurn>>>,
}

/// A place as the account rows read it.
#[derive(Clone, Debug, PartialEq)]
pub struct SignedInPlace {
    pub id: String,
    pub name: String,
    pub sign_ins: Option<IndexMap<String, AgentSignInState>>,
}

/// usage.ts:543-559 (C9).
pub fn account_names(o: &AccountRowsAsk) -> IndexMap<String, String> {
    let mut names: IndexMap<String, String> = IndexMap::new();
    for l in &o.limits {
        let label = account_words(&AccountWho {
            agent_name: (o.agent_name)(&l.agent),
            keyed: l.keyed,
            plan: l.plan.clone(),
            plan_brand: o.plan_brand.as_ref().and_then(|f| f(&l.agent)),
            named: (l.road == Some(AccountRoad::Named)).then(|| l.label.clone()),
            vaulted: Some(l.road == Some(AccountRoad::Vault)),
            own_on: match (l.road, l.computers.first()) {
                (Some(AccountRoad::Own), Some(first)) => Some((o.name_of)(first)),
                _ => None,
            },
        });
        names.insert(l.key.clone(), label);
    }
    let labels: Vec<String> = names.values().cloned().collect();
    let shared: IndexSet<String> = labels
        .iter()
        .enumerate()
        .filter(|(at, label)| labels.iter().position(|x| x == *label) != Some(*at))
        .map(|(_, l)| l.clone())
        .collect();
    for l in &o.limits {
        let Some(label) = names.get(&l.key).cloned() else {
            continue;
        };
        if !shared.contains(&label) {
            continue;
        }
        match (l.road, l.computers.first()) {
            (Some(AccountRoad::Named), _) if !label.ends_with(&format!(" as {}", l.label)) => {
                names.insert(l.key.clone(), format!("{label} as {}", l.label));
            }
            (Some(AccountRoad::Own), Some(first)) => {
                names.insert(l.key.clone(), format!("{label} on {}", (o.name_of)(first)));
            }
            _ => {}
        }
    }
    names
}

fn note_for(o: &AccountRowsAsk, agent: &str, keyed: bool) -> AccountNote {
    if keyed {
        AccountNote::PaysPerTokenNoPlanLimit
    } else if (o.prints_limits)(agent) {
        AccountNote::NotReadYetShowsAfterItsNextTurn
    } else {
        AccountNote::ReportsNoPlanLimit
    }
}

/// usage.ts:564-618 (C10).
pub fn account_rows(o: &AccountRowsAsk) -> Vec<AccountRow> {
    let mut rows: IndexMap<String, AccountRow> = IndexMap::new();
    let names = account_names(o);
    for l in &o.limits {
        let read = !l.windows.is_empty();
        let mut computers: Vec<String> = Vec::new();
        for id in &l.computers {
            let name = (o.name_of)(id);
            if !computers.contains(&name) {
                computers.push(name);
            }
        }
        let row = AccountRow {
            key: l.key.clone(),
            agent: l.agent.clone(),
            label: names.get(&l.key).cloned().unwrap_or_default(),
            computers,
            plan: l.plan.clone(),
            windows: read.then(|| l.windows.clone()),
            status: if read { l.status } else { None },
            read_at: read.then_some(l.read_at),
            note: (!read).then(|| note_for(o, &l.agent, l.keyed == Some(true))),
            burn: None,
            address: (l.road == Some(AccountRoad::Named)).then(|| l.label.clone()),
            credits: l.credits.as_ref().map(row_credits),
        };
        rows.insert(l.key.clone(), row);
    }
    for place in &o.places {
        for (agent, state) in place.sign_ins.iter().flatten() {
            if *state == AgentSignInState::None {
                continue;
            }
            let agent_name = (o.agent_name)(agent);
            let vaulted_now = (o.vaulted)(agent);
            let held = if *state == AgentSignInState::VaultKey {
                vaulted_now
            } else {
                None
            };
            let computer = ComputerName {
                id: place.id.clone(),
                name: (o.name_of)(&place.id),
            };
            let (key, label) = match held {
                Some(_) => {
                    let account = account_of(agent, &agent_name, None, held, &computer);
                    (account.key, account.label)
                }
                None => account_on_computer(agent, &agent_name, &computer, &o.limits, vaulted_now),
            };
            let name = (o.name_of)(&place.id);
            let row = rows.entry(key.clone()).or_insert_with(|| AccountRow {
                key: key.clone(),
                agent: agent.clone(),
                label,
                computers: Vec::new(),
                plan: None,
                windows: None,
                status: None,
                read_at: None,
                note: Some(note_for(o, agent, held == Some(Vaulted::Key))),
                burn: None,
                address: None,
                credits: None,
            });
            if !row.computers.contains(&name) {
                row.computers.push(name);
            }
        }
    }
    for row in rows.values_mut() {
        if let Some(burn) = o.burn.as_ref().and_then(|f| f(&row.key)) {
            row.burn = Some(burn);
        }
    }
    let mut out: Vec<AccountRow> = rows.into_values().collect();
    out.sort_by(|a, b| collate(&a.agent, &b.agent).then_with(|| collate(&a.label, &b.label)));
    out
}
