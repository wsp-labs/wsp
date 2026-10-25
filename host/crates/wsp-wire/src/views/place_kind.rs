// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/place-kind.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::Check;

/// TS: packages/protocol/src/views/place-kind.ts:6 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    #[serde(rename = "computer")]
    Computer,
    #[serde(rename = "provider")]
    Provider,
}

impl Check for PlaceKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["computer", "provider"]))
    }
}
