// SPDX-License-Identifier: AGPL-3.0-only
//! What this crate holds for crates that do not hold it yet; each moves out when its crate lands (spec/join.md).
//!
//! - `slate`: the slate module of `protocol/src/slate/` (parser, validator, formula language, batch, sketch, catalog),
//!   ported whole; it has no crate of its own yet.
//! - `wire`: `slate/wire.ts`'s view and answers, which `wsp_wire::types::slate_wire` does not hold.
//! - `code_of`: `HostError::code`, for the tests.
//! - `null_kept`: serde plumbing, not a gap.

use wsp_words::exit::HostError;

/// `e.code`, the code a slate refusal stamps (`Z803`, `V750`, ...).
pub fn code_of(e: &HostError) -> Option<String> {
    e.code.clone()
}

/// Serde plumbing for a field the TS writes as absent, a value, or `null`, all three apart: `None` is absent,
/// `Some(Value::Null)` (or `Some(None)`) is a JSON null. serde's own `Option` reads `null` as absent.
pub mod null_kept {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
        T::deserialize(d).map(Some)
    }
}

pub mod slate;
pub mod wire;
