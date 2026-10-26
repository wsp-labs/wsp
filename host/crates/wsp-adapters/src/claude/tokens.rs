// SPDX-License-Identifier: AGPL-3.0-only
//! What one call's usage line counts. TS: packages/adapter-claude/src/tokens.ts at a3b870a6e.

use crate::claude::fields::{num, Obj};
use crate::port::TurnTokens;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::UsageTokens;

/// input_tokens + cache_creation_input_tokens + cache_read_input_tokens, each 0 where absent. tokens.ts:9.
pub fn input_tokens(usage: &Obj) -> f64 {
    count(usage, "input_tokens") + count(usage, "cache_creation_input_tokens") + count(usage, "cache_read_input_tokens")
}

fn count(usage: &Obj, key: &str) -> f64 {
    num(usage.get(key)).unwrap_or(0.0)
}

/// input_tokens(usage) + output_tokens. tokens.ts:14.
pub fn held_tokens(usage: &Obj) -> f64 {
    input_tokens(usage) + count(usage, "output_tokens")
}

/// tokens.ts:19: `cached` and `cacheWrite` only where the usage names them; never reasoning, context or window.
pub fn usage_tokens(usage: &Obj) -> TurnTokens {
    TurnTokens {
        input: input_tokens(usage).into(),
        output: count(usage, "output_tokens").into(),
        cached: num(usage.get("cache_read_input_tokens")).map(JsNumber),
        cache_write: num(usage.get("cache_creation_input_tokens")).map(JsNumber),
        reasoning: None,
        context: None,
        window: None,
    }
}

/// tokens.ts:25. `a` absent answers `b` as it is. cached, cacheWrite and reasoning are present on the sum where
/// either side has them; context and window are dropped.
pub fn plus_tokens(a: Option<&TurnTokens>, b: &TurnTokens) -> TurnTokens {
    let Some(a) = a else {
        return b.clone();
    };
    TurnTokens {
        input: (a.input.0 + b.input.0).into(),
        output: (a.output.0 + b.output.0).into(),
        cached: sum(a.cached, b.cached),
        cache_write: sum(a.cache_write, b.cache_write),
        reasoning: sum(a.reasoning, b.reasoning),
        context: None,
        window: None,
    }
}

fn sum(a: Option<JsNumber>, b: Option<JsNumber>) -> Option<JsNumber> {
    match (a, b) {
        (None, None) => None,
        (a, b) => Some(JsNumber(a.map_or(0.0, |n| n.0) + b.map_or(0.0, |n| n.0))),
    }
}

/// tokens.ts:32: every count present, 0 where absent.
pub fn usage_of(t: &TurnTokens) -> UsageTokens {
    UsageTokens {
        input: t.input,
        output: t.output,
        cached: t.cached.unwrap_or_default(),
        cache_write: t.cache_write.unwrap_or_default(),
        reasoning: t.reasoning.unwrap_or_default(),
    }
}
