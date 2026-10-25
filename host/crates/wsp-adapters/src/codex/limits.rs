// SPDX-License-Identifier: AGPL-3.0-only
//! The plan's windows and banked resets as the app server reports them, and the reset a limit-stopped turn waits on.
//! TS adapter-codex/src/limits.ts. Spec: spec/codex_limits.md.

use serde_json::{Map, Value};

use crate::port::HarnessLimit;
use wsp_wire::types::usage::{BankedResets, LimitWindow};

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
    todo!()
}

/// The resets the plan has banked, read fail closed. TS limits.ts:56.
pub fn credits_of(answer: Option<&Map<String, Value>>) -> Option<BankedResets> {
    todo!()
}

/// The fullest window whose reset is still ahead of `now`. TS limits.ts:74.
pub fn stopped_until(windows: &[LimitWindow], now: f64) -> Option<f64> {
    todo!()
}

/// The codex bucket of a rate-limits answer, else its single snapshot. TS limits.ts:84.
pub fn snapshot_of(answer: Option<&Map<String, Value>>) -> Option<Map<String, Value>> {
    todo!()
}

/// The plan's windows off a snapshot; `None` where it names none. TS limits.ts:89.
pub fn limit_of(snapshot: &Map<String, Value>, account: &LimitAccount, credits: Option<BankedResets>) -> Option<HarnessLimit> {
    todo!()
}
