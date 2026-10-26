// SPDX-License-Identifier: AGPL-3.0-only
//! The plan windows a turn's rate_limit_event lines report. TS: packages/adapter-claude/src/limits.ts at a3b870a6e.

use indexmap::IndexMap;
use serde_json::Value;

use crate::claude::fields::{num, rec, string, Obj};
use crate::port::{HarnessLimit, TurnResult, TurnStatus};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::{LimitKind, LimitStatus, LimitWindow, TurnLimit};
use wsp_words::js::js_round;

/// The windows a turn's readings say stop the agent, by the CLI's `rateLimitType`, each with its reset in ms where
/// named. Insertion order is the order they were first rejected.
pub type Rejected = IndexMap<String, Option<f64>>;

/// limits.ts:22. unifiedWindows first, in its key order, then the top-level rateLimitType; the first window of a kind
/// wins. usedPercent is `round(utilization * 1000) / 10`, resetsAt is seconds * 1000. `None` where no window read.
pub fn limit_of(info: Option<&Obj>) -> Option<HarnessLimit> {
    let info = info?;
    let mut windows = Vec::new();
    for (name, reading) in rec(info.get("unifiedWindows")).into_iter().flatten() {
        add_window(&mut windows, name, rec(Some(reading)));
    }
    if let Some(top) = string(info.get("rateLimitType")) {
        add_window(&mut windows, top, Some(info));
    }
    if windows.is_empty() {
        return None;
    }
    let status = match string(info.get("status")) {
        Some("allowed") => Some(LimitStatus::Ok),
        Some("allowed_warning") => Some(LimitStatus::Warning),
        Some("rejected") => Some(LimitStatus::Reached),
        _ => None,
    };
    Some(HarnessLimit { windows, plan: None, status, account: None, keyed: None, credits: None })
}

fn kind_of(name: &str) -> Option<LimitKind> {
    Some(match name {
        "five_hour" => LimitKind::Session,
        "seven_day" => LimitKind::Week,
        "seven_day_opus" => LimitKind::WeekOpus,
        "seven_day_sonnet" => LimitKind::WeekSonnet,
        "seven_day_overage_included" | "overage" => LimitKind::Overage,
        _ => return None,
    })
}

fn add_window(windows: &mut Vec<LimitWindow>, name: &str, reading: Option<&Obj>) {
    let Some(kind) = kind_of(name) else {
        return;
    };
    let Some(used) = num(reading.and_then(|r| r.get("utilization"))) else {
        return;
    };
    if windows.iter().any(|w| w.kind == kind) {
        return;
    }
    let resets_at = num(reading.and_then(|r| r.get("resetsAt")));
    windows.push(LimitWindow {
        kind,
        used_percent: JsNumber(js_round(used * 1000.0) / 10.0),
        resets_at: resets_at.map(|r| JsNumber(r * 1000.0)),
    });
}

/// limits.ts:42.
pub fn note_rejected(info: Option<&Obj>, rejected: &mut Rejected) {
    let Some(info) = info else {
        return;
    };
    let kind = string(info.get("rateLimitType")).unwrap_or("").to_string();
    let overage = matches!(info.get("isUsingOverage"), Some(Value::Bool(true)))
        || matches!(string(info.get("overageStatus")), Some("allowed" | "allowed_warning"));
    if string(info.get("status")) != Some("rejected") || overage {
        rejected.shift_remove(&kind);
        return;
    }
    rejected.insert(kind, num(info.get("resetsAt")).map(|r| r * 1000.0));
}

/// limits.ts:56. A failed result with any rejected window gets `limit`: `{ resetsAt: max }` where every window named
/// a reset, else `{}`.
pub fn with_limit(result: TurnResult, rejected: &Rejected) -> TurnResult {
    if !matches!(result.status, TurnStatus::Failed) || rejected.is_empty() {
        return result;
    }
    let resets_at = if rejected.values().all(Option::is_some) {
        Some(JsNumber(rejected.values().flatten().copied().fold(f64::NEG_INFINITY, f64::max)))
    } else {
        None
    };
    TurnResult { limit: Some(TurnLimit { resets_at }), ..result }
}
