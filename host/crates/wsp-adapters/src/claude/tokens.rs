// SPDX-License-Identifier: AGPL-3.0-only
//! What one call's usage line counts. TS: packages/adapter-claude/src/tokens.ts at a3b870a6e.

use crate::claude::fields::Obj;
use crate::port::TurnTokens;
use wsp_wire::types::usage::UsageTokens;

/// input_tokens + cache_creation_input_tokens + cache_read_input_tokens, each 0 where absent. tokens.ts:9.
pub fn input_tokens(usage: &Obj) -> f64 {
    todo!()
}

/// input_tokens(usage) + output_tokens. tokens.ts:14.
pub fn held_tokens(usage: &Obj) -> f64 {
    todo!()
}

/// tokens.ts:19: `cached` and `cacheWrite` only where the usage names them; never reasoning, context or window.
pub fn usage_tokens(usage: &Obj) -> TurnTokens {
    todo!()
}

/// tokens.ts:25. `a` absent answers `b` as it is. cached, cacheWrite and reasoning are present on the sum where
/// either side has them; context and window are dropped.
pub fn plus_tokens(a: Option<&TurnTokens>, b: &TurnTokens) -> TurnTokens {
    todo!()
}

/// tokens.ts:32: every count present, 0 where absent.
pub fn usage_of(t: &TurnTokens) -> UsageTokens {
    todo!()
}
