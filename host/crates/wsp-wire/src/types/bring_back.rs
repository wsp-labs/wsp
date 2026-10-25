// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/bring-back.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::pull_request::PullRequest;

/// TS: packages/protocol/src/bring-back.ts:11 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPushReply;

impl Check for GitPushReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/bring-back.ts:22 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::GitPrReply;

impl Check for GitPrReply {
    /// Nested checks: pr.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/bring-back.ts:29 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BringBackResult {
    pub branch: String,
    pub base: String,
    pub ahead: i64,
    pub uncommitted: i64,
    pub stat: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PullRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
}

impl Check for BringBackResult {
    /// Nested checks: pr.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
