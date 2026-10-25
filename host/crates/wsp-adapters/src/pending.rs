// SPDX-License-Identifier: AGPL-3.0-only
//! PENDING: shapes this crate needs that wave 1 (`wsp-wire`, `wsp-words`, `wsp-catalog`, branches `rust/2445-*`) does
//! not carry yet on the branches read at a3b870a6e. Each names the TS it stands for. When the wave-1 crate gains it,
//! this declaration goes and its users import the wave-1 one.

use serde::{Deserialize, Serialize};

/// PENDING(wsp-wire): packages/protocol/src/views/harness.ts:161, a TS interface the zod generator skipped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSpec {
    pub command: String,
    pub args: Vec<String>,
    /// `Some(true)` only; never put on a launch line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_slate: Option<bool>,
}
