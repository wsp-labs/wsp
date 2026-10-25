// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/usage.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;

/// TS: packages/protocol/src/usage.ts:11 at a3b870a6e.
pub const LIMIT_KINDS: &[&str] = &["session", "week", "week_opus", "week_sonnet", "month", "overage"];

/// TS: packages/protocol/src/usage.ts:23 at a3b870a6e.
pub const RESET_CREDIT_STATUSES: &[&str] = &["available", "redeeming", "redeemed", "unknown"];

/// TS: packages/protocol/src/usage.ts:79 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const USAGE_WORDS: &[(&str, &str)] = &[
    ("notPriced", "not priced"),
    ("listPrice", "list price"),
    ("noLimit", "reports no plan limit"),
    ("keyed", "pays per token, no plan limit"),
    ("unread", "not read yet: shows after its next turn"),
    ("reached", "limit reached"),
    ("noUse", "Nothing used in this range"),
    ("wspThreads", "wsp threads"),
    ("outsideWsp", "Outside wsp"),
    ("removedComputer", "a removed computer"),
];

/// TS: packages/protocol/src/usage.ts:128 at a3b870a6e.
pub const RESET_OUTCOMES: &[&str] = &["reset", "nothingToReset", "noCredit", "alreadyRedeemed", "earlier", "unknown"];

/// TS: packages/protocol/src/usage.ts:169 at a3b870a6e.
pub const USAGE_RANGES: &[&str] = &["day", "week", "month"];

/// TS: packages/protocol/src/usage.ts:173 at a3b870a6e.
pub const USAGE_SPLITS: &[&str] = &["agent", "account", "computer", "project", "model", "source"];

/// TS: packages/protocol/src/usage.ts:182 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const RANGE_DAYS: &[(&str, f64)] = &[("day", 1.0), ("week", 7.0), ("month", 30.0)];

/// TS: packages/protocol/src/usage.ts:186 at a3b870a6e.
pub const READINGS_RANGES: &[&str] = &["15m", "day", "week", "month"];

/// TS: packages/protocol/src/usage.ts:191 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const READINGS_SPAN_MS: &[(&str, f64)] = &[("15m", 900000.0), ("day", 86400000.0), ("week", 604800000.0), ("month", 2592000000.0)];

/// TS: packages/protocol/src/usage.ts:195 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const READINGS_STEP_MS: &[(&str, f64)] = &[("15m", 15000.0), ("day", 300000.0), ("week", 1800000.0), ("month", 7200000.0)];

/// TS: packages/protocol/src/usage.ts:306 at a3b870a6e.
pub const PRICES_TTL_MS: u64 = 86_400_000;

/// TS: packages/protocol/src/usage.ts:307 at a3b870a6e.
pub const PRICES_URL: &str = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// TS: packages/protocol/src/usage.ts:457 at a3b870a6e.
pub const LIMIT_RESUME_PROMPT: &str = "Your usage limit has reset. Go on with the task you were working on when it stopped you.";

/// TS: packages/protocol/src/usage.ts:12 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LimitKind {
    #[serde(rename = "session")]
    Session,
    #[serde(rename = "week")]
    Week,
    #[serde(rename = "week_opus")]
    WeekOpus,
    #[serde(rename = "week_sonnet")]
    WeekSonnet,
    #[serde(rename = "month")]
    Month,
    #[serde(rename = "overage")]
    Overage,
}

impl Check for LimitKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["session", "week", "week_opus", "week_sonnet", "month", "overage"]))
    }
}

/// TS: packages/protocol/src/usage.ts:17 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitWindow {
    pub kind: LimitKind,
    pub used_percent: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<JsNumber>,
}

impl Check for LimitWindow {
    /// Nested checks: kind.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <LimitKind as Check>::describe(rest),
                "usedPercent" => <JsNumber as Check>::describe(rest),
                "resetsAt" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:20 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LimitStatus {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "reached")]
    Reached,
}

impl Check for LimitStatus {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["ok", "warning", "reached"]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResetCreditStatus {
    #[serde(rename = "available")]
    Available,
    #[serde(rename = "redeeming")]
    Redeeming,
    #[serde(rename = "redeemed")]
    Redeemed,
    #[serde(rename = "unknown")]
    Unknown,
}

impl Check for ResetCreditStatus {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["available", "redeeming", "redeemed", "unknown"]))
    }
}

/// TS: packages/protocol/src/usage.ts:27 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCredit {
    pub id: String,
    pub status: ResetCreditStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<JsNumber>,
}

impl Check for ResetCredit {
    fn check(&self) -> Result<(), SchemaError> {
        self.status.check().map_err(|e| e.under("status"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "status" => <ResetCreditStatus as Check>::describe(rest),
                "expiresAt" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:32 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankedResets {
    pub count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<Vec<ResetCredit>>,
}

impl Check for BankedResets {
    /// Rules: count: >= 0.
    /// Nested checks: credits.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.count as f64, 0.0).map_err(|e| e.under("count"))?;
        self.credits.check().map_err(|e| e.under("credits"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "count" => <i64 as Check>::describe(rest),
                "credits" => <Option<Vec<ResetCredit>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessLimitAccount {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Check for HarnessLimitAccount {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "label" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:38 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessLimit {
    pub windows: Vec<LimitWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<LimitStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<HarnessLimitAccount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<BankedResets>,
}

impl Check for HarnessLimit {
    /// Nested checks: windows, status, account, credits.
    fn check(&self) -> Result<(), SchemaError> {
        self.windows.check().map_err(|e| e.under("windows"))?;
        self.status.check().map_err(|e| e.under("status"))?;
        self.account.check().map_err(|e| e.under("account"))?;
        self.credits.check().map_err(|e| e.under("credits"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "windows" => <Vec<LimitWindow> as Check>::describe(rest),
                "plan" => <Option<String> as Check>::describe(rest),
                "status" => <Option<LimitStatus> as Check>::describe(rest),
                "account" => <Option<HarnessLimitAccount> as Check>::describe(rest),
                "keyed" => <Option<bool> as Check>::describe(rest),
                "credits" => <Option<BankedResets> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:57 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccountRoad {
    #[serde(rename = "named")]
    Named,
    #[serde(rename = "vault")]
    Vault,
    #[serde(rename = "own")]
    Own,
}

impl Check for AccountRoad {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["named", "vault", "own"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLimitCredits {
    pub count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<Vec<ResetCredit>>,
    pub read_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_at: Option<JsNumber>,
}

impl Check for AccountLimitCredits {
    /// Rules: count: >= 0.
    /// Nested checks: credits.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.count as f64, 0.0).map_err(|e| e.under("count"))?;
        self.credits.check().map_err(|e| e.under("credits"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "count" => <i64 as Check>::describe(rest),
                "credits" => <Option<Vec<ResetCredit>> as Check>::describe(rest),
                "readAt" => <JsNumber as Check>::describe(rest),
                "detailAt" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:62 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLimit {
    pub key: String,
    pub agent: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<AccountRoad>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    pub windows: Vec<LimitWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<LimitStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyed: Option<bool>,
    pub read_at: JsNumber,
    pub computers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<AccountLimitCredits>,
}

impl Check for AccountLimit {
    /// Nested checks: road, windows, status, credits.
    fn check(&self) -> Result<(), SchemaError> {
        self.road.check().map_err(|e| e.under("road"))?;
        self.windows.check().map_err(|e| e.under("windows"))?;
        self.status.check().map_err(|e| e.under("status"))?;
        self.credits.check().map_err(|e| e.under("credits"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "key" => <String as Check>::describe(rest),
                "agent" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "road" => <Option<AccountRoad> as Check>::describe(rest),
                "plan" => <Option<String> as Check>::describe(rest),
                "windows" => <Vec<LimitWindow> as Check>::describe(rest),
                "status" => <Option<LimitStatus> as Check>::describe(rest),
                "keyed" => <Option<bool> as Check>::describe(rest),
                "readAt" => <JsNumber as Check>::describe(rest),
                "computers" => <Vec<String> as Check>::describe(rest),
                "credits" => <Option<AccountLimitCredits> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:95 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccountNote {
    #[serde(rename = "reports no plan limit")]
    ReportsNoPlanLimit,
    #[serde(rename = "pays per token, no plan limit")]
    PaysPerTokenNoPlanLimit,
    #[serde(rename = "not read yet: shows after its next turn")]
    NotReadYetShowsAfterItsNextTurn,
}

impl Check for AccountNote {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["reports no plan limit", "pays per token, no plan limit", "not read yet: shows after its next turn"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRowBurn {
    pub tokens_per_minute: JsNumber,
    pub threads: i64,
}

impl Check for AccountRowBurn {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "tokensPerMinute" => <JsNumber as Check>::describe(rest),
                "threads" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRowCredits {
    pub count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<Vec<ResetCredit>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_expires_at: Option<JsNumber>,
    pub read_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_at: Option<JsNumber>,
}

impl Check for AccountRowCredits {
    /// Rules: count: >= 0.
    /// Nested checks: credits.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.count as f64, 0.0).map_err(|e| e.under("count"))?;
        self.credits.check().map_err(|e| e.under("credits"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "count" => <i64 as Check>::describe(rest),
                "credits" => <Option<Vec<ResetCredit>> as Check>::describe(rest),
                "nextExpiresAt" => <Option<JsNumber> as Check>::describe(rest),
                "readAt" => <JsNumber as Check>::describe(rest),
                "detailAt" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:99 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRow {
    pub key: String,
    pub agent: String,
    pub label: String,
    pub computers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<Vec<LimitWindow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<LimitStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<AccountNote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burn: Option<AccountRowBurn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<AccountRowCredits>,
}

impl Check for AccountRow {
    /// Nested checks: windows, status, note, burn, credits.
    fn check(&self) -> Result<(), SchemaError> {
        self.windows.check().map_err(|e| e.under("windows"))?;
        self.status.check().map_err(|e| e.under("status"))?;
        self.note.check().map_err(|e| e.under("note"))?;
        self.burn.check().map_err(|e| e.under("burn"))?;
        self.credits.check().map_err(|e| e.under("credits"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "key" => <String as Check>::describe(rest),
                "agent" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "computers" => <Vec<String> as Check>::describe(rest),
                "plan" => <Option<String> as Check>::describe(rest),
                "windows" => <Option<Vec<LimitWindow>> as Check>::describe(rest),
                "status" => <Option<LimitStatus> as Check>::describe(rest),
                "readAt" => <Option<JsNumber> as Check>::describe(rest),
                "note" => <Option<AccountNote> as Check>::describe(rest),
                "burn" => <Option<AccountRowBurn> as Check>::describe(rest),
                "address" => <Option<String> as Check>::describe(rest),
                "credits" => <Option<AccountRowCredits> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:120 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAccountEvent {
    pub key: String,
    pub row: AccountRow,
}

impl UsageAccountEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "usage.account";
}

impl Check for UsageAccountEvent {
    /// Nested checks: row.
    fn check(&self) -> Result<(), SchemaError> {
        self.row.check().map_err(|e| e.under("row"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "key" => <String as Check>::describe(rest),
                "row" => <AccountRow as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:123 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsAnswer {
    pub accounts: Vec<AccountRow>,
}

impl Check for AccountsAnswer {
    /// Nested checks: accounts.
    fn check(&self) -> Result<(), SchemaError> {
        self.accounts.check().map_err(|e| e.under("accounts"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "accounts" => <Vec<AccountRow> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:129 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResetOutcome {
    #[serde(rename = "reset")]
    Reset,
    #[serde(rename = "nothingToReset")]
    NothingToReset,
    #[serde(rename = "noCredit")]
    NoCredit,
    #[serde(rename = "alreadyRedeemed")]
    AlreadyRedeemed,
    #[serde(rename = "earlier")]
    Earlier,
    #[serde(rename = "unknown")]
    Unknown,
}

impl Check for ResetOutcome {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["reset", "nothingToReset", "noCredit", "alreadyRedeemed", "earlier", "unknown"]))
    }
}

/// TS: packages/protocol/src/usage.ts:133 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetAnswer {
    pub outcome: ResetOutcome,
    pub said: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountRow>,
}

impl Check for ResetAnswer {
    /// Nested checks: outcome, account.
    fn check(&self) -> Result<(), SchemaError> {
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        self.account.check().map_err(|e| e.under("account"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "outcome" => <ResetOutcome as Check>::describe(rest),
                "said" => <String as Check>::describe(rest),
                "account" => <Option<AccountRow> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:170 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UsageRange {
    #[serde(rename = "day")]
    Day,
    #[serde(rename = "week")]
    Week,
    #[serde(rename = "month")]
    Month,
}

impl Check for UsageRange {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["day", "week", "month"]))
    }
}

/// TS: packages/protocol/src/usage.ts:174 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UsageSplit {
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "account")]
    Account,
    #[serde(rename = "computer")]
    Computer,
    #[serde(rename = "project")]
    Project,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "source")]
    Source,
}

impl Check for UsageSplit {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["agent", "account", "computer", "project", "model", "source"]))
    }
}

/// TS: packages/protocol/src/usage.ts:187 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReadingsRange {
    #[serde(rename = "15m")]
    V15m,
    #[serde(rename = "day")]
    Day,
    #[serde(rename = "week")]
    Week,
    #[serde(rename = "month")]
    Month,
}

impl Check for ReadingsRange {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["15m", "day", "week", "month"]))
    }
}

/// TS: packages/protocol/src/usage.ts:198 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageKey {
    pub day: String,
    pub hour: i64,
    pub agent: String,
    pub account: String,
    pub computer: String,
    pub project: String,
    pub model: String,
}

impl Check for UsageKey {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "day" => <String as Check>::describe(rest),
                "hour" => <i64 as Check>::describe(rest),
                "agent" => <String as Check>::describe(rest),
                "account" => <String as Check>::describe(rest),
                "computer" => <String as Check>::describe(rest),
                "project" => <String as Check>::describe(rest),
                "model" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:209 at a3b870a6e.
/// Tokens by kind. zod reads each as a plain number, so the fields are JsNumber.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTokens {
    pub input: JsNumber,
    pub output: JsNumber,
    pub cached: JsNumber,
    pub cache_write: JsNumber,
    pub reasoning: JsNumber,
}

impl Check for UsageTokens {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "input" => <JsNumber as Check>::describe(rest),
                "output" => <JsNumber as Check>::describe(rest),
                "cached" => <JsNumber as Check>::describe(rest),
                "cacheWrite" => <JsNumber as Check>::describe(rest),
                "reasoning" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UsageRowSource {
    #[serde(rename = "wsp")]
    Wsp,
    #[serde(rename = "log")]
    Log,
}

impl Check for UsageRowSource {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["wsp", "log"]))
    }
}

/// TS: packages/protocol/src/usage.ts:215 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    pub day: String,
    pub hour: i64,
    pub agent: String,
    pub account: String,
    pub computer: String,
    pub project: String,
    pub model: String,
    pub turns: i64,
    pub tokens: UsageTokens,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_reported: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unreported: Option<UsageTokens>,
    pub source: UsageRowSource,
}

impl Check for UsageRow {
    /// Nested checks: tokens, unreported.
    fn check(&self) -> Result<(), SchemaError> {
        self.tokens.check().map_err(|e| e.under("tokens"))?;
        self.unreported.check().map_err(|e| e.under("unreported"))?;
        self.source.check().map_err(|e| e.under("source"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "day" => <String as Check>::describe(rest),
                "hour" => <i64 as Check>::describe(rest),
                "agent" => <String as Check>::describe(rest),
                "account" => <String as Check>::describe(rest),
                "computer" => <String as Check>::describe(rest),
                "project" => <String as Check>::describe(rest),
                "model" => <String as Check>::describe(rest),
                "turns" => <i64 as Check>::describe(rest),
                "tokens" => <UsageTokens as Check>::describe(rest),
                "costReported" => <Option<JsNumber> as Check>::describe(rest),
                "unreported" => <Option<UsageTokens> as Check>::describe(rest),
                "source" => <UsageRowSource as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:226 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageDay {
    pub day: String,
    pub rows: Vec<UsageRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessions: Option<Vec<String>>,
}

impl Check for UsageDay {
    /// Nested checks: rows.
    fn check(&self) -> Result<(), SchemaError> {
        self.rows.check().map_err(|e| e.under("rows"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "day" => <String as Check>::describe(rest),
                "rows" => <Vec<UsageRow> as Check>::describe(rest),
                "sessions" => <Option<Vec<String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsedRowTokens {
    pub input: JsNumber,
    pub output: JsNumber,
    pub cached: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<JsNumber>,
}

impl Check for UsedRowTokens {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "input" => <JsNumber as Check>::describe(rest),
                "output" => <JsNumber as Check>::describe(rest),
                "cached" => <JsNumber as Check>::describe(rest),
                "cacheWrite" => <Option<JsNumber> as Check>::describe(rest),
                "reasoning" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:232 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsedRow {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub tokens: UsedRowTokens,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_reported: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_list: Option<JsNumber>,
    pub priced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turns: Option<i64>,
}

impl Check for UsedRow {
    /// Nested checks: tokens.
    fn check(&self) -> Result<(), SchemaError> {
        self.tokens.check().map_err(|e| e.under("tokens"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "key" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "agent" => <Option<String> as Check>::describe(rest),
                "tokens" => <UsedRowTokens as Check>::describe(rest),
                "costReported" => <Option<JsNumber> as Check>::describe(rest),
                "costList" => <Option<JsNumber> as Check>::describe(rest),
                "priced" => <bool as Check>::describe(rest),
                "estimate" => <Option<JsNumber> as Check>::describe(rest),
                "saved" => <Option<JsNumber> as Check>::describe(rest),
                "turns" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsedAnswerSeriesItem {
    pub t: JsNumber,
    pub tokens: JsNumber,
}

impl Check for UsedAnswerSeriesItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "t" => <JsNumber as Check>::describe(rest),
                "tokens" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsedAnswerLogs {
    pub agents: Vec<String>,
    pub computers: Vec<String>,
}

impl Check for UsedAnswerLogs {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agents" => <Vec<String> as Check>::describe(rest),
                "computers" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsedAnswerLinesItem {
    pub key: String,
    pub label: String,
    pub points: Vec<JsNumber>,
}

impl Check for UsedAnswerLinesItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "key" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "points" => <Vec<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:250 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsedAnswer {
    pub range: UsageRange,
    pub split: UsageSplit,
    pub rows: Vec<UsedRow>,
    pub series: Vec<UsedAnswerSeriesItem>,
    pub since: JsNumber,
    pub until: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs: Option<UsedAnswerLogs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<UsedAnswerLinesItem>>,
}

impl Check for UsedAnswer {
    /// Nested checks: range, split, rows, series, logs, lines.
    fn check(&self) -> Result<(), SchemaError> {
        self.range.check().map_err(|e| e.under("range"))?;
        self.split.check().map_err(|e| e.under("split"))?;
        self.rows.check().map_err(|e| e.under("rows"))?;
        self.series.check().map_err(|e| e.under("series"))?;
        self.logs.check().map_err(|e| e.under("logs"))?;
        self.lines.check().map_err(|e| e.under("lines"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "range" => <UsageRange as Check>::describe(rest),
                "split" => <UsageSplit as Check>::describe(rest),
                "rows" => <Vec<UsedRow> as Check>::describe(rest),
                "series" => <Vec<UsedAnswerSeriesItem> as Check>::describe(rest),
                "since" => <JsNumber as Check>::describe(rest),
                "until" => <JsNumber as Check>::describe(rest),
                "logs" => <Option<UsedAnswerLogs> as Check>::describe(rest),
                "lines" => <Option<Vec<UsedAnswerLinesItem>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/usage.ts:432 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnLimit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<JsNumber>,
}

impl Check for TurnLimit {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "resetsAt" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
