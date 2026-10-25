// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/changes.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use indexmap::IndexMap;

/// TS: packages/protocol/src/changes.ts:53 at a3b870a6e.
pub const DRAFT_DIFF_MAX_BYTES: u64 = 49_152;

/// TS: packages/protocol/src/changes.ts:120 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const DRAFT_NOTES: &[(&str, &str)] = &[
    ("noAgent", "No agent here drafts commit messages; write it yourself."),
    ("noAnswer", "The agent did not draft a message; write it yourself."),
    ("nothing", "Nothing is ticked to draft a message from."),
];

/// TS: packages/protocol/src/changes.ts:11 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkout {
    pub branch: String,
    pub ahead: i64,
    pub behind: i64,
    pub changed: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits_unread: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts_unknown: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stashes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    pub read_at: JsNumber,
}

impl Check for Checkout {
    /// Rules: stashes: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/changes.ts:41 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutReply {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<Checkout>,
}

impl Check for CheckoutReply {
    /// Nested checks: checkout.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/changes.ts:45 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDraft {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Check for CommitDraft {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/changes.ts:49 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewedMarks {
    pub viewed: IndexMap<String, String>,
}

impl Check for ViewedMarks {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
