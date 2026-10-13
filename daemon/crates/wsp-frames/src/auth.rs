// SPDX-License-Identifier: AGPL-3.0-only
use std::num::NonZeroU16;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::RequestId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum AuthTag {
    #[serde(rename = "auth")]
    Auth,
}

/// The first frame on every daemon socket. port scopes the socket to one guest port: only tunnel ops on that port
/// and ping are answered on it. pipe asks for a pipe rather than a control socket: one local connection's bytes,
/// opened by `PipeOpen`. A daemon from before pipes ignores the field and answers it as a control socket, so the
/// host reads the first frame back before it takes the socket for a pipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DaemonAuthRequest {
    pub id: RequestId,
    #[ts(type = "\"auth\"")]
    op: AuthTag,
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub port: Option<NonZeroU16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pipe: Option<bool>,
}

impl DaemonAuthRequest {
    pub fn new(id: RequestId, token: impl Into<String>, port: Option<NonZeroU16>) -> Self {
        DaemonAuthRequest { id, op: AuthTag::Auth, token: token.into(), port, pipe: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum PipeOpenTag {
    #[serde(rename = "pipe.open")]
    PipeOpen,
}

/// Which loopback door on the machine a pipe's local connection came in on: the HTTP door for model calls and git,
/// the gh door for gh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum PipeDoor {
    Http,
    Gh,
}

/// The daemon's one text frame on a pipe, sent when a local connection takes it; the connection's bytes follow as
/// binary frames both ways. held_ms is how long that connection waited for a pipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PipeOpen {
    #[ts(type = "\"pipe.open\"")]
    op: PipeOpenTag,
    pub door: PipeDoor,
    #[ts(type = "number")]
    pub held_ms: u64,
}

impl PipeOpen {
    pub fn new(door: PipeDoor, held_ms: u64) -> Self {
        PipeOpen { op: PipeOpenTag::PipeOpen, door, held_ms }
    }
}
