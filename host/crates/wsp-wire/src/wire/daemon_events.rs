// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/daemon-events.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::{Check, JsNumber, SchemaError};
use crate::wire::daemon::GuestKind;
use crate::wire::limits::RelayPort;
use crate::wire::machine_link::{ProcChanges, ProcSnapshot, SysSample};
use wsp_frames::{False, True};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PtyModeMode {
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "raw")]
    Raw,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonEvent {
    #[serde(rename = "daemon.hello", rename_all = "camelCase")]
    DaemonHello { root: String, version: i64 },
    #[serde(rename = "pty.data", rename_all = "camelCase")]
    PtyData { pty_id: String, data: String },
    #[serde(rename = "pty.exit", rename_all = "camelCase")]
    PtyExit {
        pty_id: String,
        exit_code: JsNumber,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signal: Option<JsNumber>,
    },
    #[serde(rename = "port.open", rename_all = "camelCase")]
    PortOpen {
        port: JsNumber,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pid: Option<JsNumber>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        process: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        loopback: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        watch: Option<String>,
    },
    #[serde(rename = "port.close", rename_all = "camelCase")]
    PortClose {
        port: JsNumber,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pid: Option<JsNumber>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        process: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exited: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        left: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        watch: Option<String>,
    },
    #[serde(rename = "inbox.file", rename_all = "camelCase")]
    InboxFile { path: String, bytes: JsNumber },
    #[serde(rename = "pty.mode", rename_all = "camelCase")]
    PtyMode { pty_id: String, mode: PtyModeMode, echo: bool, foreground: String },
    #[serde(rename = "browser.open", rename_all = "camelCase")]
    BrowserOpen {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port: Option<RelayPort>,
    },
    #[serde(rename = "callback.port", rename_all = "camelCase")]
    CallbackPort { port: RelayPort },
    #[serde(rename = "tunnel.data", rename_all = "camelCase")]
    TunnelData {
        tunnel_id: String,
        data: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        machine_id: Option<String>,
    },
    #[serde(rename = "tunnel.end", rename_all = "camelCase")]
    TunnelEnd {
        tunnel_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        machine_id: Option<String>,
    },
    #[serde(rename = "stdio.out", rename_all = "camelCase")]
    StdioOut { stdio_id: String, data: String },
    #[serde(rename = "stdio.err", rename_all = "camelCase")]
    StdioErr { stdio_id: String, data: String },
    #[serde(rename = "stdio.exit", rename_all = "camelCase")]
    StdioExit {
        stdio_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<i64>,
    },
    #[serde(rename = "localhost.url", rename_all = "camelCase")]
    LocalhostUrl { port: RelayPort },
    #[serde(rename = "sys.sample")]
    SysSample(SysSample),
    #[serde(rename = "proc.snapshot")]
    ProcSnapshot(ProcSnapshot),
    #[serde(rename = "proc.changes")]
    ProcChanges(ProcChanges),
    #[serde(rename = "guest.opened", rename_all = "camelCase")]
    GuestOpened {
        session: String,
        life: String,
        kind: GuestKind,
        token: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn_token: Option<String>,
        argv: Vec<String>,
        cwd: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        machine_id: Option<String>,
    },
    #[serde(rename = "guest.message", rename_all = "camelCase")]
    GuestMessage {
        session: String,
        message: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        machine_id: Option<String>,
    },
    #[serde(rename = "guest.closed", rename_all = "camelCase")]
    GuestClosed {
        session: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        machine_id: Option<String>,
    },
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// Refinements on the variants of the unions above, each an issue when it fails:
// - DaemonEvent::BrowserOpen.url: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=

/// Reads one daemon event as zod's discriminated union on `type` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `type` is
/// one invalid_union_discriminator issue on ["type"] whose detail `options` lists every type of DaemonEvent in order and
/// whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the variant's first failing
/// field, then its Check. Unknown keys are dropped.
pub fn parse_daemon_event(frame: &Value) -> Result<DaemonEvent, SchemaError> {
    let _ = frame;
    todo!()
}

/// Every type DaemonEvent holds, in the TS union's order.
pub const DAEMON_EVENT_TYPES: &[&str] = &[
    "daemon.hello",
    "pty.data",
    "pty.exit",
    "port.open",
    "port.close",
    "inbox.file",
    "pty.mode",
    "browser.open",
    "callback.port",
    "tunnel.data",
    "tunnel.end",
    "stdio.out",
    "stdio.err",
    "stdio.exit",
    "localhost.url",
    "sys.sample",
    "proc.snapshot",
    "proc.changes",
    "guest.opened",
    "guest.message",
    "guest.closed",
];

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for PtyModeMode {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
