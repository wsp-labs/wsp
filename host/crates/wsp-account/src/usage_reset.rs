// SPDX-License-Identifier: AGPL-3.0-only
//! Spending one of an account's banked plan resets on a computer of the person's that holds the account's login: a
//! read that checks the login and the count, then the spend under an idempotency key kept across a timeout, a lost
//! link and a host restart. A second press while one runs joins it. Port of runtime/src/usage-reset.ts at 65f6b89c7;
//! spec/usage_reset.md.
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use wsp_adapters::port::{ResetReading, ResetRoad, ResetSpend};
use wsp_core::types::pending::Store;
use wsp_wire::types::usage::{AccountLimit, AccountRow, HarnessLimit, ResetAnswer};
use wsp_words::exit::HostError;

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

/// The agent's two reset scripts, as `PlanResets` (protocol adapter-port.ts:577) less the `Send` bound, with a
/// `spend_command` that may refuse: the TS throws on a credit id the agent would not take (usage-reset.ts:147-151),
/// which `wsp_adapters::port::PlanResets` cannot say. The wiring adapts the adapter's table to this.
pub trait Resets {
    fn read_command(&self, road: &ResetRoad) -> String;
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
    running: Rc<std::cell::RefCell<indexmap::IndexMap<String, RunningReset>>>,
}

// export function usageResets(d: UsageResetDeps): (ask: ResetAsk) => Promise<ResetAnswer> {
//   const running = new Map<string, Promise<ResetAnswer>>();
pub fn usage_resets(deps: Rc<dyn UsageResetDeps>) -> UsageResets {
    todo!()
}

impl UsageResets {
    // return ask => {
    //   const held = running.get(ask.account);
    //   if (held !== undefined) return held;
    //   const run = spend(ask).finally(() => running.delete(ask.account));
    //   running.set(ask.account, run);
    //   return run;
    // };
    /// Joins the spend in flight for `ask.account`, else starts one; the account is let go when it settles.
    pub fn reset(&self, ask: ResetAsk) -> RunningReset {
        todo!()
    }
}

// const pick = (placeOf, ask, agent, label, limit, own): ResetPlace => {
//   const ids = own !== undefined ? [own] : (limit?.computers ?? []);
//   const mine = ids.map(placeOf).filter(p => p.kind !== "provider");
//   if (mine.length === 0) {
//     const provider = ids[0];
//     throw usage(provider === undefined ? resetSignedOutLine(label, d.agentName(agent)) : resetProviderOnlyLine(label, providerKeyName(provider), d.agentName(agent)));
//   }
//   const holding = own !== undefined ? mine : mine.filter(p => p.signedIn(agent));
//   if (holding.length === 0) throw usage(resetSignedOutLine(label, d.agentName(agent)));
//   if (ask.on !== undefined) {
//     const named = holding.find(p => p.id === ask.on);
//     if (named === undefined) throw usage(resetNotOnLine(label, placeOf(ask.on).name, holding.map(p => p.name)));
//     if (!named.connected) throw new Error(absentComputer(named.name, null).sentence);
//     return named;
//   }
//   const first = holding.find(p => p.connected);
//   if (first === undefined) throw new Error(absentComputer(holding[0]!.name, null).sentence);
//   return first;
// };
/// The computer the spend runs on (spec rule 7). `own` is the computer id of an own login (`codex@here`).
pub fn pick(
    deps: &dyn UsageResetDeps,
    place_of: &PlaceOf,
    ask: &ResetAsk,
    agent: &str,
    label: &str,
    limit: Option<&AccountLimit>,
    own: Option<&str>,
) -> Result<ResetPlace, HostError> {
    todo!()
}

// const spend = async (ask: ResetAsk): Promise<ResetAnswer> => { ... }  (usage-reset.ts:106-174; spec rules 2-22)
/// One spend, start to answer.
pub async fn spend(deps: Rc<dyn UsageResetDeps>, ask: ResetAsk) -> Result<ResetAnswer, HostError> {
    todo!()
}
