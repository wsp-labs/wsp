// SPDX-License-Identifier: AGPL-3.0-only
//! The plan's windows and banked resets as the app server reports them, and the reset a limit-stopped turn waits on.
//! TS adapter-codex/src/limits.ts. Spec: spec/codex_limits.md.

use std::cmp::Ordering;

use serde_json::{Map, Value};

use super::fields::{count, rec, str};
use crate::pending::limit_kind_of_minutes;
use crate::port::HarnessLimit;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::usage::{BankedResets, HarnessLimitAccount, LimitStatus, LimitWindow, ResetCredit, ResetCreditStatus};

/// The server's codexErrorInfo on an error the plan's usage limit raised. TS limits.ts:80.
pub const USAGE_LIMIT_CODE: &str = "usageLimitExceeded";

/// The account a limit is read for, as the turn has learned it. TS limits.ts:89's `account`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LimitAccount {
    pub id: Option<String>,
    pub label: Option<String>,
    pub plan: Option<String>,
}

/// A window's reset as ms epoch: seconds below 1e11 are made ms. TS limits.ts:51.
pub fn reset_ms(value: f64) -> f64 {
    if value < 1e11 {
        value * 1000.0
    } else {
        value
    }
}

/// The resets the plan has banked, read fail closed. TS limits.ts:56.
pub fn credits_of(answer: Option<&Map<String, Value>>) -> Option<BankedResets> {
    let summary = rec(answer.and_then(|a| a.get("rateLimitResetCredits")))?;
    let banked = count(summary.get("availableCount")).filter(|n| n.fract() == 0.0 && *n >= 0.0)?;
    let banked_count = JsInt(JsNumber(banked));
    let Some(raw) = summary.get("credits").and_then(Value::as_array) else {
        return Some(BankedResets { count: banked_count, credits: None });
    };
    let credits = raw
        .iter()
        .filter_map(|raw| {
            let credit = rec(Some(raw))?;
            let id = str(credit.get("id"))?;
            let said = match credit.get("status").and_then(Value::as_str) {
                Some("available") => ResetCreditStatus::Available,
                Some("redeeming") => ResetCreditStatus::Redeeming,
                Some("redeemed") => ResetCreditStatus::Redeemed,
                _ => ResetCreditStatus::Unknown,
            };
            let status =
                if credit.get("resetType").and_then(Value::as_str) == Some("codexRateLimits") { said } else { ResetCreditStatus::Unknown };
            let expires_at = count(credit.get("expiresAt")).map(|n| JsNumber(reset_ms(n)));
            Some(ResetCredit { id: id.to_string(), status, expires_at })
        })
        .collect();
    Some(BankedResets { count: banked_count, credits: Some(credits) })
}

/// The fullest window whose reset is still ahead of `now`. TS limits.ts:74.
pub fn stopped_until(windows: &[LimitWindow], now: f64) -> Option<f64> {
    let mut ahead: Vec<(f64, f64)> =
        windows.iter().filter_map(|w| Some((w.used_percent.0, w.resets_at?.0))).filter(|&(_, at)| at > now).collect();
    ahead.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal).then(b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal)));
    ahead.first().map(|&(_, at)| at)
}

/// The codex bucket of a rate-limits answer, else its single snapshot. TS limits.ts:84.
pub fn snapshot_of(answer: Option<&Map<String, Value>>) -> Option<Map<String, Value>> {
    let codex = rec(answer.and_then(|a| a.get("rateLimitsByLimitId")).and_then(|b| b.get("codex")));
    codex.or_else(|| rec(answer.and_then(|a| a.get("rateLimits")))).cloned()
}

/// The plan's windows off a snapshot; `None` where it names none. TS limits.ts:89.
pub fn limit_of(snapshot: &Map<String, Value>, account: &LimitAccount, credits: Option<BankedResets>) -> Option<HarnessLimit> {
    let mut windows = Vec::new();
    for (slot, fallback) in [("primary", 300.0), ("secondary", 10_080.0)] {
        let Some(w) = rec(snapshot.get(slot)) else {
            continue;
        };
        let Some(used) = count(w.get("usedPercent")) else {
            continue;
        };
        let minutes = count(w.get("windowDurationMins")).unwrap_or(fallback);
        let resets_at = count(w.get("resetsAt")).map(|n| JsNumber(reset_ms(n)));
        windows.push(LimitWindow { kind: limit_kind_of_minutes(minutes), used_percent: JsNumber(used), resets_at });
    }
    if windows.is_empty() {
        return None;
    }
    let plan = str(snapshot.get("planType")).map(String::from).or_else(|| account.plan.clone());
    let status = match snapshot.get("rateLimitReachedType") {
        None | Some(Value::Null) => LimitStatus::Ok,
        Some(_) => LimitStatus::Reached,
    };
    let id = account.id.clone().or_else(|| account.label.clone());
    let account = id.map(|id| HarnessLimitAccount { id, label: account.label.clone() });
    Some(HarnessLimit { windows, plan, status: Some(status), account, keyed: None, credits })
}
