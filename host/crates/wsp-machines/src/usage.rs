// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/usage.ts` at 65f6b89c7: the ledger of what was used and each account's last reading
//! (`ledger`, spec `usage_ledger.md`), and the accounts, computer names, burn, price table and account rows
//! (`accounts`, spec `usage_accounts.md`). Kept apart and never added together.
pub mod accounts;
pub mod ledger;

use std::rc::Rc;

use wsp_wire::types::usage::{AccountRoad, HarnessLimit, UsageRange, UsageSplit};

/// usage.ts:42: one blob a day.
pub const USAGE_DAYS: &str = "usage-days";
/// usage.ts:44: the days the ledger holds.
pub const USAGE_INDEX: &str = "usage-index";
/// usage.ts:45.
pub const LIMITS: &str = "limits";
/// usage.ts:46.
pub const ACCOUNT_LABELS: &str = "usage-accounts";
/// usage.ts:47.
pub const PRICES: &str = "prices";
/// usage.ts:143.
pub const COMPUTER_NAMES: &str = "usage-computers";
/// usage.ts:50.
pub const USAGE_RETENTION_DAYS: u32 = 90;
/// usage.ts:191.
pub const RESET_DETAILS_MS: i64 = 24 * 3_600_000;
/// usage.ts:458.
pub const BURN_WINDOW_MS: i64 = 15 * 60_000;

/// The five token counts an entry may carry, each absent read as 0 (`Partial<TurnTokens>`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntryTokens {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cached: Option<f64>,
    pub cache_write: Option<f64>,
    pub reasoning: Option<f64>,
}

/// Where a row came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageSource {
    Wsp,
    Log,
}

/// usage.ts:58-74 `UsageEntry`.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageEntry {
    pub at: i64,
    pub agent: String,
    pub account: String,
    pub account_label: String,
    pub computer: String,
    pub project: String,
    pub model: Option<String>,
    pub turns: Option<f64>,
    pub tokens: Option<EntryTokens>,
    pub cost_usd: Option<f64>,
    pub session: Option<String>,
    pub source: UsageSource,
}

/// usage.ts:76-83 `UsedQuery`.
#[derive(Clone)]
pub struct UsedQuery {
    pub range: UsageRange,
    pub split: UsageSplit,
    pub label: Rc<dyn Fn(UsageSplit, &str) -> String>,
    /// None counts the logs.
    pub outside: Option<bool>,
}

/// usage.ts:85-94 `LimitReading`.
#[derive(Clone, Debug, PartialEq)]
pub struct LimitReading {
    pub key: String,
    pub agent: String,
    pub label: String,
    pub road: AccountRoad,
    pub computer: String,
    pub limit: HarnessLimit,
    pub at: Option<i64>,
}

/// usage.ts:112 `Vaulted`: what the vault holds for an agent's sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vaulted {
    Token,
    Key,
}
