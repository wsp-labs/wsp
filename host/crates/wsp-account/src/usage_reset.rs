// SPDX-License-Identifier: AGPL-3.0-only
//! Spending one of an account's banked plan resets on a computer of the person's that holds the account's login: a
//! read that checks the login and the count, then the spend under an idempotency key kept across a timeout, a lost
//! link and a host restart. A second press while one runs joins it. Port of runtime/src/usage-reset.ts at 65f6b89c7;
//! spec/usage_reset.md.
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use async_trait::async_trait;
use futures::future::{FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_adapters::port::{ResetReading, ResetRoad, ResetSpend};
use wsp_core::console::warn;
use wsp_core::types::pending::Store;
use wsp_wire::types::usage::{AccountLimit, AccountNote, AccountRoad, AccountRow, HarnessLimit, ResetAnswer, ResetOutcome};
use wsp_words::exit::{marked_cut, HostError, Kind};
use wsp_words::place_word::provider_key_name;
use wsp_words::usage::{
    no_such_account_line, reset_keyed_line, reset_mismatch_line, reset_none_line, reset_not_on_line, reset_provider_only_line,
    reset_refused_line, reset_signed_out_line, reset_silent_line, reset_unread_line, reset_who, reset_words,
};
use wsp_words::workspace_state::absent_computer;

/// The idempotency key of a spend the agent has not answered, by account, with the count banked before it.
/// usage-reset.ts:40.
pub const PENDING: &str = "usage-reset-pending";

/// `"here" | "box" | "provider"`: this computer, a box the person joined, or a provider's machines, which no reset
/// runs on. usage-reset.ts:47.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetPlaceKind {
    Here,
    Box,
    Provider,
}

/// A computer a reset may run on, as the host knows it now. usage-reset.ts:43-51.
#[derive(Clone)]
pub struct ResetPlace {
    pub id: String,
    pub name: String,
    pub kind: ResetPlaceKind,
    pub connected: bool,
    /// Whether the agent's own login stands there.
    pub signed_in: Rc<dyn Fn(&str) -> bool>,
}

/// Each computer by id as the host knows it at the moment a spend starts; an id the host does not know still answers a
/// place (usage-reset.ts:59).
pub type PlaceOf = Rc<dyn Fn(&str) -> ResetPlace>;

/// The agent's two reset scripts, as `PlanResets` (protocol adapter-port.ts:577). Either command may refuse, as the TS
/// throws on a relative home or a credit id the agent would not take (usage-reset.ts:146-151).
/// `wsp_adapters::port::PlanResets` answers the same two `Result`s but is `Send`-free only since the adapters' fix
/// round; this copy predates it, and the wiring can use the adapter's trait directly once the tests' fake moves to it.
pub trait Resets {
    /// `readCommand(road)`; `Err` carries the thrown message, which usage-reset.ts:140 lets out as a plain error.
    fn read_command(&self, road: &ResetRoad) -> Result<String, String>;
    fn parse_read(&self, stdout: &str) -> Option<ResetReading>;
    /// `spendCommand({...road, idempotencyKey, creditId?})`; `Err` carries the thrown message.
    fn spend_command(&self, road: &ResetRoad, idempotency_key: &str, credit_id: Option<&str>) -> Result<String, String>;
    fn parse_spend(&self, stdout: &str) -> ResetSpend;
    fn too_old(&self) -> String;
}

/// What a filing is asked: `{ agent, place, limit }`. usage-reset.ts:65.
pub struct FileAsk<'a> {
    pub agent: &'a str,
    pub place: &'a ResetPlace,
    pub limit: &'a HarnessLimit,
}

/// The runtime's half, one method per field of the TS deps object. usage-reset.ts:53-68.
#[async_trait(?Send)]
pub trait UsageResetDeps {
    fn store(&self) -> Rc<dyn Store>;
    async fn limits(&self) -> Result<Vec<AccountLimit>, HostError>;
    fn agent_name(&self, agent: &str) -> String;
    fn resets(&self, agent: &str) -> Option<Rc<dyn Resets>>;
    async fn places(&self) -> Result<PlaceOf, HostError>;
    /// Where the agent's login lives on that computer; refuses with the sentence when it cannot be read.
    async fn road(&self, agent: &str, place: &ResetPlace) -> Result<ResetRoad, HostError>;
    /// One script there for the agent's login, answering what it printed.
    async fn run(&self, place: &ResetPlace, script: &str, agent: &str) -> Result<String, HostError>;
    /// Files a reading the way a turn's is filed, answering the key it went under.
    async fn file(&self, ask: FileAsk<'_>) -> Result<String, HostError>;
    async fn row(&self, key: &str) -> Result<Option<AccountRow>, HostError>;
    fn uuid(&self) -> String;
}

/// What `usage.reset` asks. usage-reset.ts:70-75.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResetAsk {
    pub account: String,
    pub credit_id: Option<String>,
    /// The place to spend it on, where the person named one.
    pub on: Option<String>,
}

/// The spend in flight for one account, which a second press joins.
pub type RunningReset = Shared<LocalBoxFuture<'static, Result<ResetAnswer, HostError>>>;

/// The function `usageResets(d)` answers: one spend per account at a time, joined. usage-reset.ts:81-183.
pub struct UsageResets {
    deps: Rc<dyn UsageResetDeps>,
    running: Rc<RefCell<IndexMap<String, RunningReset>>>,
}

/// usage-reset.ts:81 `usageResets(d)`.
pub fn usage_resets(deps: Rc<dyn UsageResetDeps>) -> UsageResets {
    UsageResets { deps, running: Rc::new(RefCell::new(IndexMap::new())) }
}

impl UsageResets {
    /// Joins the spend in flight for `ask.account`, else starts one; the account is let go when it settles.
    pub fn reset(&self, ask: ResetAsk) -> RunningReset {
        let held = self.running.borrow().get(&ask.account).cloned();
        if let Some(held) = held {
            return held;
        }
        let account = ask.account.clone();
        let deps = self.deps.clone();
        let running: Weak<RefCell<IndexMap<String, RunningReset>>> = Rc::downgrade(&self.running);
        let settled = account.clone();
        let run: RunningReset = async move {
            let answer = spend(deps, ask).await;
            if let Some(running) = running.upgrade() {
                running.borrow_mut().shift_remove(&settled);
            }
            answer
        }
        .boxed_local()
        .shared();
        self.running.borrow_mut().insert(account, run.clone());
        run
    }
}

/// `usage(sentence, kind)`: a host error of the usage class (usage-reset.ts:79).
fn usage(sentence: &str) -> HostError {
    HostError::with_kind(sentence, Kind::Usage)
}

/// `/^([^:@]+)@(.+)$/`: an own login's agent and computer id, else None.
fn own_login(account: &str) -> Option<(String, String)> {
    let at = account.find('@')?;
    let (agent, computer) = (&account[..at], &account[at + 1..]);
    if agent.is_empty() || agent.contains(':') || computer.is_empty() || computer.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    Some((agent.to_string(), computer.to_string()))
}

/// `s.slice(from)` in UTF-16 units.
fn utf16_tail(s: &str, from: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    String::from_utf16_lossy(&units[from.min(units.len())..])
}

/// A banked count as the JSON number the TS writes, an integer.
fn count_value(n: f64) -> Value {
    json!(n as i64)
}

/// `d.row(key)` plus the answer's shape (usage-reset.ts:135).
async fn answer(deps: &dyn UsageResetDeps, outcome: ResetOutcome, said: String, key: &str) -> Result<ResetAnswer, HostError> {
    let account = deps.row(key).await?;
    Ok(ResetAnswer { outcome, said, account })
}

// usage-reset.ts:86 `pick`: the computer the spend runs on. `own` is the computer id of an own login (`codex@here`).
/// The computer the spend runs on (spec rule 7).
pub fn pick(
    deps: &dyn UsageResetDeps,
    place_of: &PlaceOf,
    ask: &ResetAsk,
    agent: &str,
    label: &str,
    limit: Option<&AccountLimit>,
    own: Option<&str>,
) -> Result<ResetPlace, HostError> {
    let ids: Vec<String> = match own {
        Some(own) => vec![own.to_string()],
        None => limit.map(|l| l.computers.clone()).unwrap_or_default(),
    };
    let mine: Vec<ResetPlace> = ids.iter().map(|id| place_of(id)).filter(|p| p.kind != ResetPlaceKind::Provider).collect();
    if mine.is_empty() {
        let line = match ids.first() {
            None => reset_signed_out_line(label, &deps.agent_name(agent)),
            Some(provider) => reset_provider_only_line(label, &provider_key_name(provider), &deps.agent_name(agent)),
        };
        return Err(usage(&line));
    }
    let holding: Vec<ResetPlace> = if own.is_some() { mine } else { mine.into_iter().filter(|p| (p.signed_in)(agent)).collect() };
    if holding.is_empty() {
        return Err(usage(&reset_signed_out_line(label, &deps.agent_name(agent))));
    }
    if let Some(on) = &ask.on {
        let Some(named) = holding.iter().find(|p| &p.id == on) else {
            let names: Vec<String> = holding.iter().map(|p| p.name.clone()).collect();
            return Err(usage(&reset_not_on_line(label, &place_of(on).name, &names)));
        };
        if !named.connected {
            return Err(HostError::new(absent_computer(&named.name, None).sentence));
        }
        return Ok(named.clone());
    }
    if let Some(first) = holding.iter().find(|p| p.connected) {
        return Ok(first.clone());
    }
    match holding.first() {
        Some(first) => Err(HostError::new(absent_computer(&first.name, None).sentence)),
        None => Err(usage(&reset_signed_out_line(label, &deps.agent_name(agent)))),
    }
}

// usage-reset.ts:106 `spend`, rules 2-22 of spec/usage_reset.md.
/// One spend, start to answer.
pub async fn spend(deps: Rc<dyn UsageResetDeps>, ask: ResetAsk) -> Result<ResetAnswer, HostError> {
    let limits = deps.limits().await?;
    let limit = limits.iter().find(|l| l.key == ask.account);
    let row = deps.row(&ask.account).await?;
    let own = own_login(&ask.account);
    let agent: Option<String> = limit
        .map(|l| l.agent.clone())
        .or_else(|| row.as_ref().map(|r| r.agent.clone()))
        .or_else(|| own.as_ref().map(|(agent, _)| agent.clone()));
    let Some(agent) = agent else {
        return Err(HostError::with_kind(no_such_account_line(&ask.account), Kind::NotFound));
    };
    let Some(resets) = deps.resets(&agent) else {
        return Err(usage(&reset_none_line(&deps.agent_name(&agent))));
    };
    let label = row.as_ref().map(|r| r.label.clone()).or_else(|| limit.map(|l| l.label.clone())).unwrap_or_else(|| ask.account.clone());
    let keyed = limit.is_some_and(|l| l.keyed == Some(true) || l.road == Some(AccountRoad::Vault))
        || row.as_ref().is_some_and(|r| r.note == Some(AccountNote::PaysPerTokenNoPlanLimit));
    if keyed {
        return Err(usage(&reset_keyed_line(&label)));
    }
    let place_of = deps.places().await?;
    let place = pick(&*deps, &place_of, &ask, &agent, &label, limit, own.as_ref().map(|(_, computer)| computer.as_str()))?;
    let name = deps.agent_name(&agent);
    let who = reset_who(&label, own.is_some(), &place.name);
    let road = deps.road(&agent, &place).await?;

    let script = resets.read_command(&road).map_err(HostError::new)?;
    let stdout = deps.run(&place, &script, &agent).await?;
    let read = resets.parse_read(&stdout);
    if read.as_ref().is_some_and(|r| r.keyed) {
        return Err(usage(&reset_keyed_line(&label)));
    }
    let Some(read_limit) = read.and_then(|r| r.limit) else {
        return Err(HostError::new(reset_unread_line(&name, &place.name, &label)));
    };
    let signed_in = read_limit.account.clone();
    let expected = utf16_tail(&ask.account, agent.encode_utf16().count() + 1);
    if let Some(named) = limit.filter(|l| l.road == Some(AccountRoad::Named)) {
        let by_id = signed_in.as_ref().is_some_and(|account| account.id == expected);
        let by_label = signed_in.as_ref().and_then(|account| account.label.as_deref()) == Some(expected.as_str());
        if !by_id && !by_label {
            let other = signed_in
                .as_ref()
                .and_then(|account| account.label.clone().or_else(|| Some(account.id.clone())))
                .unwrap_or_else(|| "another account".to_string());
            return Err(HostError::new(reset_mismatch_line(&name, &place.name, &other, &named.label)));
        }
    }
    let filed = deps.file(FileAsk { agent: &agent, place: &place, limit: &read_limit }).await?;
    let Some(banked) = read_limit.credits.as_ref().map(|credits| credits.count.as_f64()) else {
        return Err(HostError::new(format!("{name} on {} {}", place.name, resets.too_old())));
    };

    let pending = deps.store().get(PENDING, &ask.account).await?;
    let pending_key: Option<String> = pending.as_ref().and_then(|p| p.get("key")).and_then(Value::as_str).map(str::to_string);
    let before: Option<f64> = pending.as_ref().and_then(|p| p.get("before")).and_then(Value::as_f64);
    if let (Some(_), Some(before)) = (&pending_key, before) {
        if banked < before {
            deps.store().delete(PENDING, &ask.account).await?;
            return answer(&*deps, ResetOutcome::Earlier, reset_words::earlier(&who, banked), &filed).await;
        }
    }
    if banked == 0.0 {
        return answer(&*deps, ResetOutcome::NoCredit, reset_words::no_credit(&who), &filed).await;
    }

    let key = pending_key.unwrap_or_else(|| deps.uuid());
    let script = resets.spend_command(&road, &key, ask.credit_id.as_deref()).map_err(|message| usage(&message))?;
    deps.store().put(PENDING, &ask.account, json!({ "key": key, "before": count_value(before.unwrap_or(banked)) })).await?;
    let stdout = deps.run(&place, &script, &agent).await?;
    let (outcome_word, spent_limit) = match resets.parse_spend(&stdout) {
        ResetSpend::Unanswered => {
            warn(&format!("the reset of {label} on {} went unanswered; the same request goes again on the next press", place.name));
            return Err(HostError::new(reset_silent_line(&name, &place.name)));
        }
        ResetSpend::Refused { refused, too_old } => {
            deps.store().delete(PENDING, &ask.account).await?;
            let line = if too_old {
                format!("{name} on {} {}", place.name, resets.too_old())
            } else {
                reset_refused_line(&name, &place.name, &label, &marked_cut(&refused, 400))
            };
            return Err(HostError::new(line));
        }
        ResetSpend::Outcome { outcome, limit } => {
            deps.store().delete(PENDING, &ask.account).await?;
            (outcome, limit)
        }
    };
    let after = match &spent_limit {
        None => filed,
        Some(spent_limit) => deps.file(FileAsk { agent: &agent, place: &place, limit: spent_limit }).await?,
    };
    let left = spent_limit.as_ref().and_then(|l| l.credits.as_ref()).map(|credits| credits.count.as_f64());
    let outcome = match outcome_word.as_str() {
        "reset" => ResetOutcome::Reset,
        "nothingToReset" => ResetOutcome::NothingToReset,
        "noCredit" => ResetOutcome::NoCredit,
        "alreadyRedeemed" => ResetOutcome::AlreadyRedeemed,
        _ => ResetOutcome::Unknown,
    };
    let said = match outcome {
        ResetOutcome::Reset => reset_words::reset(&who, left),
        ResetOutcome::NothingToReset => reset_words::nothing_to_reset(&who),
        ResetOutcome::NoCredit => reset_words::no_credit(&who),
        ResetOutcome::AlreadyRedeemed => reset_words::already_redeemed(&who),
        _ => reset_words::unknown(&name, &outcome_word, &who),
    };
    answer(&*deps, outcome, said, &after).await
}
