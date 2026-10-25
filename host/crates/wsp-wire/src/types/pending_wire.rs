// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/pending_wire.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use wsp_frames::RelayPort;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortForwardKind {
    #[serde(rename = "url")]
    Url,
    #[serde(rename = "callback")]
    Callback,
    #[serde(rename = "editor")]
    Editor,
}

impl Check for PortForwardKind {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortForward {
    pub workspace_id: String,
    pub port: RelayPort,
    pub started_at: String,
    pub name: String,
    pub kind: PortForwardKind,
}

impl Check for PortForward {
    /// Nested checks: port.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineSizeOffer {
    pub cpu: JsNumber,
    pub mem_mb: JsNumber,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Check for MachineSizeOffer {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub id: String,
    pub label: String,
    pub effect: PermissionEffect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

impl Check for PermissionOption {
    /// Nested checks: effect.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSlateEvent {
    pub workspace_id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i64>,
    pub cause: SlateCause,
    pub version: i64,
    pub by: SlateBy,
    pub pieces: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

impl SessionSlateEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "session.slate";
}

impl Check for SessionSlateEvent {
    /// Rules: pos: > 0; pieces: at most 20 items.
    /// Nested checks: cause, by.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionEffect {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "mode")]
    Mode,
    #[serde(rename = "answer")]
    Answer,
}

impl Check for PermissionEffect {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlateCause {
    #[serde(rename = "write")]
    Write,
    #[serde(rename = "state")]
    State,
    #[serde(rename = "clear")]
    Clear,
    #[serde(rename = "undo")]
    Undo,
    #[serde(rename = "restore")]
    Restore,
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "run")]
    Run,
}

impl Check for SlateCause {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlateBy {
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "person")]
    Person,
    #[serde(rename = "host")]
    Host,
    #[serde(rename = "reaction")]
    Reaction,
    #[serde(rename = "timer")]
    Timer,
}

impl Check for SlateBy {}
