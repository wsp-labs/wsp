// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/release.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};

/// TS: packages/protocol/src/release.ts:52 at a3b870a6e.
pub const UP_RESTART_LINE: &str = "Ctrl-C the terminal running wsp up and run it again.";

/// TS: packages/protocol/src/release.ts:55 at a3b870a6e.
pub const HOST_NO_RESTART_LINE: &str = "This host cannot restart itself.";

/// TS: packages/protocol/src/release.ts:58 at a3b870a6e.
pub const HOST_RESTART_TICKET_REFUSAL: &str = "a socket let in on a ticket cannot restart this host; restart it on the computer it runs on";

/// TS: packages/protocol/src/release.ts:9 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseLatest {
    pub version: String,
    pub tag: String,
    pub url: String,
    pub published_at: String,
}

impl Check for ReleaseLatest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/release.ts:14 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReleaseState {
    #[serde(rename = "checking")]
    Checking,
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "unreached")]
    Unreached,
    #[serde(rename = "off")]
    Off,
}

impl Check for ReleaseState {}

/// TS: packages/protocol/src/release.ts:18 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostShape {
    #[serde(rename = "verb")]
    Verb,
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "service")]
    Service,
}

impl Check for HostShape {}

/// TS: packages/protocol/src/release.ts:28 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseView {
    pub state: ReleaseState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<ReleaseLatest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tried_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<HostShape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart_refusal: Option<String>,
}

impl Check for ReleaseView {
    /// Nested checks: state, latest, shape.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/release.ts:41 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseChangedEvent {
    pub release: ReleaseView,
}

impl ReleaseChangedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "release.changed";
}

impl Check for ReleaseChangedEvent {
    /// Nested checks: release.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
