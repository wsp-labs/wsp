// SPDX-License-Identifier: AGPL-3.0-only
//! Stand-ins for what this crate names from crates that do not hold it yet. Each is a gap in a crate below, listed in
//! spec/join.md; every function body is `todo!()`.
//!
//! - `slate`: `wsp-slate`, which has no crate (protocol/src/slate/ less wire.ts), in this one file.
//! - `wire`: `slate/wire.ts`'s view and answers, which `wsp_wire::types::slate_wire` does not hold.
//! - `code_of`: `HostError::code`, which the words fix round has not added.
//! - `null_kept`: serde plumbing, not a gap.

use wsp_words::exit::HostError;

/// `e.code`, the code a slate refusal stamps (`Z803`, `V750`, ...). `HostError` gains `code` in the words fix round
/// (ruling 6402177374 point 3) and has none in the base; until then the tests read it through this.
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
