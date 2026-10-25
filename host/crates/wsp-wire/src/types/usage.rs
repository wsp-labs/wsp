// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/usage.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resets_at: Option<JsNumber>,
}

impl Check for LimitWindow {
    /// Nested checks: kind.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<LimitKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("usedPercent") { None => out.push(crate::types::check::missing::<Self>("usedPercent")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("usedPercent"))), } } }
        if let Some(v) = obj.get("resetsAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("resetsAt"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub expires_at: Option<JsNumber>,
}

impl Check for ResetCredit {
    fn check(&self) -> Result<(), SchemaError> {
        self.status.check().map_err(|e| e.under("status"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("status") { None => out.push(crate::types::check::missing::<Self>("status")), Some(v) => { match crate::types::check::read::<ResetCreditStatus>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } } }
        if let Some(v) = obj.get("expiresAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("expiresAt"))), } }
        Some(out)
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
    pub count: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub credits: Option<Vec<ResetCredit>>,
}

impl Check for BankedResets {
    /// Rules: count: >= 0.
    /// Nested checks: credits.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.count.0.0, 0.0).map_err(|e| e.under("count"))?;
        self.credits.check().map_err(|e| e.under("credits"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("count") { None => out.push(crate::types::check::missing::<Self>("count")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_count(&x) { out.push(e.under("count")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("count"))), } } }
        if let Some(v) = obj.get("credits") { match crate::types::check::read_present::<Vec<ResetCredit>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("credits"))), } }
        Some(out)
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
impl BankedResets {
    fn rule_count(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessLimitAccount {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub label: Option<String>,
}

impl Check for HarnessLimitAccount {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        if let Some(v) = obj.get("label") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("label"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub status: Option<LimitStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub account: Option<HarnessLimitAccount>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub keyed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("windows") { None => out.push(crate::types::check::missing::<Self>("windows")), Some(v) => { match crate::types::check::read::<Vec<LimitWindow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("windows"))), } } }
        if let Some(v) = obj.get("plan") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plan"))), } }
        if let Some(v) = obj.get("status") { match crate::types::check::read_present::<LimitStatus>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } }
        if let Some(v) = obj.get("account") { match crate::types::check::read_present::<HarnessLimitAccount>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("account"))), } }
        if let Some(v) = obj.get("keyed") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("keyed"))), } }
        if let Some(v) = obj.get("credits") { match crate::types::check::read_present::<BankedResets>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("credits"))), } }
        Some(out)
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
    pub count: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub credits: Option<Vec<ResetCredit>>,
    pub read_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub detail_at: Option<JsNumber>,
}

impl Check for AccountLimitCredits {
    /// Rules: count: >= 0.
    /// Nested checks: credits.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.count.0.0, 0.0).map_err(|e| e.under("count"))?;
        self.credits.check().map_err(|e| e.under("credits"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("count") { None => out.push(crate::types::check::missing::<Self>("count")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_count(&x) { out.push(e.under("count")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("count"))), } } }
        if let Some(v) = obj.get("credits") { match crate::types::check::read_present::<Vec<ResetCredit>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("credits"))), } }
        match obj.get("readAt") { None => out.push(crate::types::check::missing::<Self>("readAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } } }
        if let Some(v) = obj.get("detailAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("detailAt"))), } }
        Some(out)
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
impl AccountLimitCredits {
    fn rule_count(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/usage.ts:62 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLimit {
    pub key: String,
    pub agent: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub road: Option<AccountRoad>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plan: Option<String>,
    pub windows: Vec<LimitWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub status: Option<LimitStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub keyed: Option<bool>,
    pub read_at: JsNumber,
    pub computers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("key") { None => out.push(crate::types::check::missing::<Self>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("label") { None => out.push(crate::types::check::missing::<Self>("label")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("label"))), } } }
        if let Some(v) = obj.get("road") { match crate::types::check::read_present::<AccountRoad>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } }
        if let Some(v) = obj.get("plan") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plan"))), } }
        match obj.get("windows") { None => out.push(crate::types::check::missing::<Self>("windows")), Some(v) => { match crate::types::check::read::<Vec<LimitWindow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("windows"))), } } }
        if let Some(v) = obj.get("status") { match crate::types::check::read_present::<LimitStatus>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } }
        if let Some(v) = obj.get("keyed") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("keyed"))), } }
        match obj.get("readAt") { None => out.push(crate::types::check::missing::<Self>("readAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } } }
        match obj.get("computers") { None => out.push(crate::types::check::missing::<Self>("computers")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computers"))), } } }
        if let Some(v) = obj.get("credits") { match crate::types::check::read_present::<AccountLimitCredits>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("credits"))), } }
        Some(out)
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
    pub threads: JsInt,
}

impl Check for AccountRowBurn {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("tokensPerMinute") { None => out.push(crate::types::check::missing::<Self>("tokensPerMinute")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tokensPerMinute"))), } } }
        match obj.get("threads") { None => out.push(crate::types::check::missing::<Self>("threads")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threads"))), } } }
        Some(out)
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
    pub count: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub credits: Option<Vec<ResetCredit>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub next_expires_at: Option<JsNumber>,
    pub read_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub detail_at: Option<JsNumber>,
}

impl Check for AccountRowCredits {
    /// Rules: count: >= 0.
    /// Nested checks: credits.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.count.0.0, 0.0).map_err(|e| e.under("count"))?;
        self.credits.check().map_err(|e| e.under("credits"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("count") { None => out.push(crate::types::check::missing::<Self>("count")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_count(&x) { out.push(e.under("count")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("count"))), } } }
        if let Some(v) = obj.get("credits") { match crate::types::check::read_present::<Vec<ResetCredit>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("credits"))), } }
        if let Some(v) = obj.get("nextExpiresAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("nextExpiresAt"))), } }
        match obj.get("readAt") { None => out.push(crate::types::check::missing::<Self>("readAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } } }
        if let Some(v) = obj.get("detailAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("detailAt"))), } }
        Some(out)
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
impl AccountRowCredits {
    fn rule_count(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0.0, 0.0)?;
        Ok(())
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub windows: Option<Vec<LimitWindow>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub status: Option<LimitStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub read_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<AccountNote>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub burn: Option<AccountRowBurn>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("key") { None => out.push(crate::types::check::missing::<Self>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("label") { None => out.push(crate::types::check::missing::<Self>("label")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("label"))), } } }
        match obj.get("computers") { None => out.push(crate::types::check::missing::<Self>("computers")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computers"))), } } }
        if let Some(v) = obj.get("plan") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plan"))), } }
        if let Some(v) = obj.get("windows") { match crate::types::check::read_present::<Vec<LimitWindow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("windows"))), } }
        if let Some(v) = obj.get("status") { match crate::types::check::read_present::<LimitStatus>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("status"))), } }
        if let Some(v) = obj.get("readAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("readAt"))), } }
        if let Some(v) = obj.get("note") { match crate::types::check::read_present::<AccountNote>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("note"))), } }
        if let Some(v) = obj.get("burn") { match crate::types::check::read_present::<AccountRowBurn>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("burn"))), } }
        if let Some(v) = obj.get("address") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("address"))), } }
        if let Some(v) = obj.get("credits") { match crate::types::check::read_present::<AccountRowCredits>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("credits"))), } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("key") { None => out.push(crate::types::check::missing::<Self>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
        match obj.get("row") { None => out.push(crate::types::check::missing::<Self>("row")), Some(v) => { match crate::types::check::read::<AccountRow>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("row"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("accounts") { None => out.push(crate::types::check::missing::<Self>("accounts")), Some(v) => { match crate::types::check::read::<Vec<AccountRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("accounts"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub account: Option<AccountRow>,
}

impl Check for ResetAnswer {
    /// Nested checks: outcome, account.
    fn check(&self) -> Result<(), SchemaError> {
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        self.account.check().map_err(|e| e.under("account"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("outcome") { None => out.push(crate::types::check::missing::<Self>("outcome")), Some(v) => { match crate::types::check::read::<ResetOutcome>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outcome"))), } } }
        match obj.get("said") { None => out.push(crate::types::check::missing::<Self>("said")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("said"))), } } }
        if let Some(v) = obj.get("account") { match crate::types::check::read_present::<AccountRow>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("account"))), } }
        Some(out)
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
    pub hour: JsInt,
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("day") { None => out.push(crate::types::check::missing::<Self>("day")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("day"))), } } }
        match obj.get("hour") { None => out.push(crate::types::check::missing::<Self>("hour")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hour"))), } } }
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("account") { None => out.push(crate::types::check::missing::<Self>("account")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("account"))), } } }
        match obj.get("computer") { None => out.push(crate::types::check::missing::<Self>("computer")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computer"))), } } }
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        match obj.get("model") { None => out.push(crate::types::check::missing::<Self>("model")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("input") { None => out.push(crate::types::check::missing::<Self>("input")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("input"))), } } }
        match obj.get("output") { None => out.push(crate::types::check::missing::<Self>("output")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("output"))), } } }
        match obj.get("cached") { None => out.push(crate::types::check::missing::<Self>("cached")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cached"))), } } }
        match obj.get("cacheWrite") { None => out.push(crate::types::check::missing::<Self>("cacheWrite")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cacheWrite"))), } } }
        match obj.get("reasoning") { None => out.push(crate::types::check::missing::<Self>("reasoning")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reasoning"))), } } }
        Some(out)
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
    pub hour: JsInt,
    pub agent: String,
    pub account: String,
    pub computer: String,
    pub project: String,
    pub model: String,
    pub turns: JsInt,
    pub tokens: UsageTokens,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost_reported: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("day") { None => out.push(crate::types::check::missing::<Self>("day")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("day"))), } } }
        match obj.get("hour") { None => out.push(crate::types::check::missing::<Self>("hour")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hour"))), } } }
        match obj.get("agent") { None => out.push(crate::types::check::missing::<Self>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
        match obj.get("account") { None => out.push(crate::types::check::missing::<Self>("account")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("account"))), } } }
        match obj.get("computer") { None => out.push(crate::types::check::missing::<Self>("computer")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computer"))), } } }
        match obj.get("project") { None => out.push(crate::types::check::missing::<Self>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
        match obj.get("model") { None => out.push(crate::types::check::missing::<Self>("model")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } } }
        match obj.get("turns") { None => out.push(crate::types::check::missing::<Self>("turns")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turns"))), } } }
        match obj.get("tokens") { None => out.push(crate::types::check::missing::<Self>("tokens")), Some(v) => { match crate::types::check::read::<UsageTokens>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tokens"))), } } }
        if let Some(v) = obj.get("costReported") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("costReported"))), } }
        if let Some(v) = obj.get("unreported") { match crate::types::check::read_present::<UsageTokens>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("unreported"))), } }
        match obj.get("source") { None => out.push(crate::types::check::missing::<Self>("source")), Some(v) => { match crate::types::check::read::<UsageRowSource>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sessions: Option<Vec<String>>,
}

impl Check for UsageDay {
    /// Nested checks: rows.
    fn check(&self) -> Result<(), SchemaError> {
        self.rows.check().map_err(|e| e.under("rows"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("day") { None => out.push(crate::types::check::missing::<Self>("day")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("day"))), } } }
        match obj.get("rows") { None => out.push(crate::types::check::missing::<Self>("rows")), Some(v) => { match crate::types::check::read::<Vec<UsageRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rows"))), } } }
        if let Some(v) = obj.get("sessions") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessions"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cache_write: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reasoning: Option<JsNumber>,
}

impl Check for UsedRowTokens {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("input") { None => out.push(crate::types::check::missing::<Self>("input")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("input"))), } } }
        match obj.get("output") { None => out.push(crate::types::check::missing::<Self>("output")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("output"))), } } }
        match obj.get("cached") { None => out.push(crate::types::check::missing::<Self>("cached")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cached"))), } } }
        if let Some(v) = obj.get("cacheWrite") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cacheWrite"))), } }
        if let Some(v) = obj.get("reasoning") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reasoning"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agent: Option<String>,
    pub tokens: UsedRowTokens,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost_reported: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cost_list: Option<JsNumber>,
    pub priced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub estimate: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub saved: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub turns: Option<JsInt>,
}

impl Check for UsedRow {
    /// Nested checks: tokens.
    fn check(&self) -> Result<(), SchemaError> {
        self.tokens.check().map_err(|e| e.under("tokens"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("key") { None => out.push(crate::types::check::missing::<Self>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
        match obj.get("label") { None => out.push(crate::types::check::missing::<Self>("label")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("label"))), } } }
        if let Some(v) = obj.get("agent") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } }
        match obj.get("tokens") { None => out.push(crate::types::check::missing::<Self>("tokens")), Some(v) => { match crate::types::check::read::<UsedRowTokens>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tokens"))), } } }
        if let Some(v) = obj.get("costReported") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("costReported"))), } }
        if let Some(v) = obj.get("costList") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("costList"))), } }
        match obj.get("priced") { None => out.push(crate::types::check::missing::<Self>("priced")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("priced"))), } } }
        if let Some(v) = obj.get("estimate") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("estimate"))), } }
        if let Some(v) = obj.get("saved") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("saved"))), } }
        if let Some(v) = obj.get("turns") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turns"))), } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("t") { None => out.push(crate::types::check::missing::<Self>("t")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("t"))), } } }
        match obj.get("tokens") { None => out.push(crate::types::check::missing::<Self>("tokens")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tokens"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("agents") { None => out.push(crate::types::check::missing::<Self>("agents")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } } }
        match obj.get("computers") { None => out.push(crate::types::check::missing::<Self>("computers")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computers"))), } } }
        Some(out)
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("key") { None => out.push(crate::types::check::missing::<Self>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
        match obj.get("label") { None => out.push(crate::types::check::missing::<Self>("label")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("label"))), } } }
        match obj.get("points") { None => out.push(crate::types::check::missing::<Self>("points")), Some(v) => { match crate::types::check::read::<Vec<JsNumber>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("points"))), } } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub logs: Option<UsedAnswerLogs>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("range") { None => out.push(crate::types::check::missing::<Self>("range")), Some(v) => { match crate::types::check::read::<UsageRange>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("range"))), } } }
        match obj.get("split") { None => out.push(crate::types::check::missing::<Self>("split")), Some(v) => { match crate::types::check::read::<UsageSplit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("split"))), } } }
        match obj.get("rows") { None => out.push(crate::types::check::missing::<Self>("rows")), Some(v) => { match crate::types::check::read::<Vec<UsedRow>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rows"))), } } }
        match obj.get("series") { None => out.push(crate::types::check::missing::<Self>("series")), Some(v) => { match crate::types::check::read::<Vec<UsedAnswerSeriesItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("series"))), } } }
        match obj.get("since") { None => out.push(crate::types::check::missing::<Self>("since")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("since"))), } } }
        match obj.get("until") { None => out.push(crate::types::check::missing::<Self>("until")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("until"))), } } }
        if let Some(v) = obj.get("logs") { match crate::types::check::read_present::<UsedAnswerLogs>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("logs"))), } }
        if let Some(v) = obj.get("lines") { match crate::types::check::read_present::<Vec<UsedAnswerLinesItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lines"))), } }
        Some(out)
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resets_at: Option<JsNumber>,
}

impl Check for TurnLimit {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("resetsAt") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("resetsAt"))), } }
        Some(out)
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
