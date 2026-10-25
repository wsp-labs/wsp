// SPDX-License-Identifier: AGPL-3.0-only
//! usage.ts:111-160, 181-201, 457-618 at 65f6b89c7: accounts and their labels, computer names, banked resets, the
//! burn, the price table and the account rows. Spec: `spec/usage_accounts.md`.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;

use wsp_core::types::pending::{Clock, Store};
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::usage::{AccountLimit, AccountLimitCredits, AccountRoad, AccountRow, AccountRowBurn, AccountRowCredits};
use wsp_wire::views::place_more::PlaceView;
use wsp_words::exit::HostError;
use wsp_words::usage::RateTable;

use super::Vaulted;

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

/// usage.ts:117-122 (C1).
pub fn account_of(agent: &str, agent_name: &str, named: Option<&NamedAccount>, vaulted: Option<Vaulted>, computer: &ComputerName) -> Account {
    todo!()
}

/// usage.ts:127-130 (C2): the key and label.
pub fn account_on_computer(agent: &str, agent_name: &str, computer: &ComputerName, limits: &[AccountLimit], vaulted: Option<Vaulted>) -> (String, String) {
    todo!()
}

/// usage.ts:135-140 (C3).
pub fn usage_computer_name(places: &[PlaceView], id: &str, kept: &IndexMap<String, String>) -> String {
    todo!()
}

/// usage.ts:146-160 (C4).
pub async fn kept_computer_names(store: &dyn Store, places: &[PlaceView]) -> Result<IndexMap<String, String>, HostError> {
    todo!()
}

/// usage.ts:194-195 (C5).
pub fn reset_details_due(held: Option<&AccountLimit>, now: i64) -> bool {
    todo!()
}

/// usage.ts:198-201 (C6).
pub fn row_credits(held: &AccountLimitCredits) -> AccountRowCredits {
    todo!()
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

impl Burn {
    pub fn new(clock: Rc<dyn Clock>) -> Burn {
        todo!()
    }

    pub fn add(&self, call: BurnCall) {
        todo!()
    }

    pub fn of(&self, account: &str) -> Option<AccountRowBurn> {
        todo!()
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
        todo!()
    }

    pub async fn get(self: &Rc<Self>) -> RateTable {
        todo!()
    }
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
    todo!()
}

/// usage.ts:564-618 (C10).
pub fn account_rows(o: &AccountRowsAsk) -> Vec<AccountRow> {
    todo!()
}
