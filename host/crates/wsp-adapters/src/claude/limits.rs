// SPDX-License-Identifier: AGPL-3.0-only
//! The plan windows a turn's rate_limit_event lines report. TS: packages/adapter-claude/src/limits.ts at a3b870a6e.

use indexmap::IndexMap;

use crate::claude::fields::Obj;
use crate::port::{HarnessLimit, TurnResult};

/// The windows a turn's readings say stop the agent, by the CLI's `rateLimitType`, each with its reset in ms where
/// named. Insertion order is the order they were first rejected.
pub type Rejected = IndexMap<String, Option<f64>>;

/// limits.ts:22. unifiedWindows first, in its key order, then the top-level rateLimitType; the first window of a kind
/// wins. usedPercent is `round(utilization * 1000) / 10`, resetsAt is seconds * 1000. `None` where no window read.
pub fn limit_of(info: Option<&Obj>) -> Option<HarnessLimit> {
    todo!()
}

/// limits.ts:42.
pub fn note_rejected(info: Option<&Obj>, rejected: &mut Rejected) {
    todo!()
}

/// limits.ts:56. A failed result with any rejected window gets `limit`: `{ resetsAt: max }` where every window named
/// a reset, else `{}`.
pub fn with_limit(result: TurnResult, rejected: &Rejected) -> TurnResult {
    todo!()
}
