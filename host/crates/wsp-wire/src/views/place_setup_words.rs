// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/place-setup-words.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};

/// TS: packages/protocol/src/views/place-setup-words.ts:218 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceEstimate {
    pub needed_bytes: i64,
    pub kept_bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_bytes: Option<i64>,
    pub unmeasured: i64,
}

impl Check for PlaceEstimate {
    /// Rules: neededBytes: >= 0; keptBytes: >= 0; freeBytes: >= 0; unmeasured: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
