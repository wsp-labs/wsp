// SPDX-License-Identifier: AGPL-3.0-only
//! usage.ts:162-455 at 65f6b89c7 (`createUsageLedger`): the ledger of what was used, one blob a day, and each
//! account's last reading. Spec: `spec/usage_ledger.md`.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;

use wsp_core::types::pending::{Clock, Store};
use wsp_wire::types::usage::{AccountLimit, AccountLimitCredits, BankedResets, UsageDay, UsedAnswer};
use wsp_words::exit::HostError;
use wsp_words::usage::RateTable;

use super::{LimitReading, UsageEntry, UsedQuery};

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

impl UsageLedger {
    pub fn new(o: UsageLedgerOptions) -> Rc<UsageLedger> {
        todo!()
    }

    /// usage.ts:291-298 (U7).
    pub async fn add(self: &Rc<Self>, entry: UsageEntry) -> Result<(), HostError> {
        todo!()
    }

    /// usage.ts:300-329 (U8).
    pub async fn file_logs(self: &Rc<Self>, entries: Vec<UsageEntry>, computers: Vec<String>) -> Result<(), HostError> {
        todo!()
    }

    /// usage.ts:331-410 (U9 to U11).
    pub async fn used(&self, q: UsedQuery) -> Result<UsedAnswer, HostError> {
        todo!()
    }

    /// usage.ts:218-223 (U3).
    pub async fn day(&self, day: &str) -> Result<Option<UsageDay>, HostError> {
        todo!()
    }

    /// usage.ts:224-227 (U3).
    pub async fn days(&self) -> Result<Vec<String>, HostError> {
        todo!()
    }

    /// usage.ts:412-437 (U12).
    pub async fn limit(self: &Rc<Self>, r: LimitReading) -> Result<LimitKept, HostError> {
        todo!()
    }

    /// usage.ts:439-443 (U14).
    pub async fn limits(&self) -> Result<Vec<AccountLimit>, HostError> {
        todo!()
    }

    /// usage.ts:445-452 (U14).
    pub async fn account_labels(&self) -> Result<IndexMap<String, String>, HostError> {
        todo!()
    }
}

/// usage.ts:163-173: the instant the zone's day holding `at` began.
pub fn day_start_of(at: i64, time_zone: Option<&str>) -> i64 {
    todo!()
}

/// usage.ts:184-188 (U13).
pub fn credits_held(held: Option<&AccountLimitCredits>, read: Option<&BankedResets>, now: i64) -> Option<AccountLimitCredits> {
    todo!()
}
