// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/wire.ts:33-123, 207-269 at 65f6b89c7: the view and the answers, for `wsp_wire::types::slate_wire`.
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::slate::SlateProblem;

/// wire.ts:37.
pub const SLATE_HELD_APPROVAL: &str = "needs your approval";
/// wire.ts:94.
pub const SLATE_SEND_KEY: &str = "send";

/// wire.ts:33.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlateEmpty {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "cleared")]
    Cleared,
    #[serde(rename = "rewound-before")]
    RewoundBefore,
}

/// wire.ts:40-66, keys in the order slates.ts:648-664 writes them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlateCmdAsk {
    pub key: String,
    pub run: String,
    pub cmd: String,
    pub env: IndexMap<String, String>,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
    pub computer: String,
    pub folder: String,
    #[serde(rename = "timeoutS")]
    pub timeout_s: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_always: Option<bool>,
    pub why: String,
}

/// wire.ts:40-90: `kind` is written after `run` for a command (slates.ts:651); a tool ask is the MCP table's own
/// JSON (wire.ts:69-88), kept as it comes.
#[derive(Clone, Debug, PartialEq)]
pub enum SlateAsk {
    Cmd(SlateCmdAsk),
    Tool(Value),
}

impl SlateAsk {
    pub fn key(&self) -> String {
        todo!()
    }
    pub fn run(&self) -> String {
        todo!()
    }
    /// The ask as the wire carries it: `{key, run, kind:"cmd", cmd, ...}` or the tool ask's JSON.
    pub fn to_json(&self) -> Value {
        todo!()
    }
}

/// wire.ts:97: an approval as a window reads it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlateApprovalView {
    pub state: String,
    pub at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts: Option<IndexMap<String, String>>,
}

/// wire.ts:101-122, keys in the order slates.ts:669-684 writes them.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateView {
    pub thread_id: String,
    pub workspace_id: String,
    pub version: i64,
    pub revision: i64,
    pub document: Option<Value>,
    pub values: IndexMap<String, Value>,
    pub empty: Option<SlateEmpty>,
    pub comments: Vec<Value>,
    pub approvals: IndexMap<String, SlateApprovalView>,
    pub asks: Vec<SlateAsk>,
    pub problems: Vec<SlateProblem>,
    pub shown_once: bool,
    pub can_undo: bool,
    pub updated_at: i64,
}

impl SlateView {
    /// The view as the wire carries it, in the TS's key order.
    pub fn to_json(&self) -> Value {
        todo!()
    }
}

/// wire.ts:212.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlateWriteAnswer {
    pub version: i64,
    pub text: String,
    pub warnings: Vec<SlateProblem>,
    pub problems: Vec<SlateProblem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<Vec<String>>,
}

/// wire.ts:216.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlateStateAnswer {
    pub version: i64,
    pub text: String,
    pub problems: Vec<SlateProblem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_started: Option<Vec<String>>,
}

/// wire.ts:220-236, keys in the order slates.ts:1247-1260 writes them.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateReadAnswer {
    pub version: i64,
    pub text: String,
    /// `Some` only with `document: true`; `Some(None)` when there is no document.
    pub document: Option<Option<Value>>,
    pub values: IndexMap<String, Value>,
    pub state: IndexMap<String, Value>,
    pub derived: IndexMap<String, Value>,
    pub runs: IndexMap<String, Value>,
    pub problems: Vec<SlateProblem>,
    pub waiting: Vec<String>,
    pub comments: Vec<Value>,
    pub approvals: IndexMap<String, String>,
}

/// wire.ts:243.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlateEventOutcome {
    Started,
    Steered,
    Queued,
    Held,
    Done,
}

/// wire.ts:246-253.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateEventAnswer {
    pub outcome: SlateEventOutcome,
    pub said: String,
    pub turn_id: Option<String>,
    pub ask: Option<SlateAsk>,
}

/// wire.ts:263-268, as JSON.
pub type SlatesImageAnswer = Value;
