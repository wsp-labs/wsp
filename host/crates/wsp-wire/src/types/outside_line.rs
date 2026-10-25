// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/outside-line.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};

/// TS: packages/protocol/src/outside-line.ts:18 at a3b870a6e.
pub const OUTSIDE_HELD: u64 = 50;

/// TS: packages/protocol/src/outside-line.ts:13 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutsideLine {
    pub title: String,
    pub body: String,
    pub show: bool,
    pub sound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl Check for OutsideLine {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
