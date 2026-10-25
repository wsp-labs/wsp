// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/daemon-events.ts.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::helpers::ReqId;
use crate::types::check::{leaf, parse, refine, Zod};
use crate::types::{Check, JsNumber, SchemaError};
use crate::wire::daemon::GuestKind;
use crate::wire::limits::{is_http_url, RelayPort};
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
    DaemonHello { root: String, version: JsInt },
    #[serde(rename = "pty.data", rename_all = "camelCase")]
    PtyData { pty_id: String, data: String },
    #[serde(rename = "pty.exit", rename_all = "camelCase")]
    PtyExit {
        pty_id: String,
        exit_code: JsNumber,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        signal: Option<JsNumber>,
    },
    #[serde(rename = "port.open", rename_all = "camelCase")]
    PortOpen {
        port: JsNumber,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        pid: Option<JsNumber>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        process: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        loopback: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        watch: Option<String>,
    },
    #[serde(rename = "port.close", rename_all = "camelCase")]
    PortClose {
        port: JsNumber,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        pid: Option<JsNumber>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        process: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        command: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        exited: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        left: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        at: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        watch: Option<String>,
    },
    #[serde(rename = "inbox.file", rename_all = "camelCase")]
    InboxFile { path: String, bytes: JsNumber },
    #[serde(rename = "pty.mode", rename_all = "camelCase")]
    PtyMode { pty_id: String, mode: PtyModeMode, echo: bool, foreground: String },
    #[serde(rename = "browser.open", rename_all = "camelCase")]
    BrowserOpen {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        port: Option<RelayPort>,
    },
    #[serde(rename = "callback.port", rename_all = "camelCase")]
    CallbackPort { port: RelayPort },
    #[serde(rename = "tunnel.data", rename_all = "camelCase")]
    TunnelData {
        tunnel_id: String,
        data: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        machine_id: Option<String>,
    },
    #[serde(rename = "tunnel.end", rename_all = "camelCase")]
    TunnelEnd {
        tunnel_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        machine_id: Option<String>,
    },
    #[serde(rename = "stdio.out", rename_all = "camelCase")]
    StdioOut { stdio_id: String, data: String },
    #[serde(rename = "stdio.err", rename_all = "camelCase")]
    StdioErr { stdio_id: String, data: String },
    #[serde(rename = "stdio.exit", rename_all = "camelCase")]
    StdioExit {
        stdio_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        code: Option<JsInt>,
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
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        turn_token: Option<String>,
        argv: Vec<String>,
        cwd: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        machine_id: Option<String>,
    },
    #[serde(rename = "guest.message", rename_all = "camelCase")]
    GuestMessage {
        session: String,
        message: Value,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        machine_id: Option<String>,
    },
    #[serde(rename = "guest.closed", rename_all = "camelCase")]
    GuestClosed {
        session: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        error: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        machine_id: Option<String>,
    },
}

/// The refinements of the variants: browser.open's url is an http or https URL, and every RelayPort is checked.
impl Check for DaemonEvent {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            DaemonEvent::BrowserOpen { url, port } => {
                refine(is_http_url(url), "http or https URL").map_err(|e| e.under("url"))?;
                port.as_ref().map_or(Ok(()), |p| p.check().map_err(|e| e.under("port")))
            }
            DaemonEvent::CallbackPort { port } | DaemonEvent::LocalhostUrl { port } => port.check().map_err(|e| e.under("port")),
            DaemonEvent::PtyMode { mode, .. } => mode.check(),
            DaemonEvent::SysSample(v) => v.check(),
            DaemonEvent::ProcSnapshot(v) => v.check(),
            DaemonEvent::ProcChanges(v) => v.check(),
            _ => Ok(()),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::tag_describe(path, "type", DAEMON_EVENT_TYPES)
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("type") else {
            return Some(vec![crate::types::check::missing::<Self>("type")]);
        };
        match tag.as_str() {
            Some("daemon.hello") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "root", &mut out);
                crate::types::keys::req_key::<JsInt>(obj, "version", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.data") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.exit") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::req_key::<JsNumber>(obj, "exitCode", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "signal", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("port.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<JsNumber>(obj, "port", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "pid", &mut out);
                crate::types::keys::opt_key::<String>(obj, "process", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "loopback", &mut out);
                crate::types::keys::opt_key::<String>(obj, "watch", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("port.close") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<JsNumber>(obj, "port", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "pid", &mut out);
                crate::types::keys::opt_key::<String>(obj, "process", &mut out);
                crate::types::keys::opt_key::<String>(obj, "command", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "exited", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "left", &mut out);
                crate::types::keys::opt_key::<String>(obj, "at", &mut out);
                crate::types::keys::opt_key::<String>(obj, "watch", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("inbox.file") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::req_key::<JsNumber>(obj, "bytes", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.mode") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::req_key::<PtyModeMode>(obj, "mode", &mut out);
                crate::types::keys::req_key::<bool>(obj, "echo", &mut out);
                crate::types::keys::req_key::<String>(obj, "foreground", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("browser.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "url", &mut out);
                crate::types::keys::opt_key::<RelayPort>(obj, "port", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("callback.port") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<RelayPort>(obj, "port", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("tunnel.data") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "tunnelId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("tunnel.end") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "tunnelId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.out") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "stdioId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.err") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "stdioId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.exit") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "stdioId", &mut out);
                crate::types::keys::opt_key::<JsInt>(obj, "code", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("localhost.url") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<RelayPort>(obj, "port", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.sample") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<SysSample>(&serde_json::Value::Object(body)))
            }
            Some("proc.snapshot") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<ProcSnapshot>(&serde_json::Value::Object(body)))
            }
            Some("proc.changes") => {
                let mut body = obj.clone();
                body.shift_remove("type");
                Some(crate::types::check::issues_of::<ProcChanges>(&serde_json::Value::Object(body)))
            }
            Some("guest.opened") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "session", &mut out);
                crate::types::keys::req_key::<String>(obj, "life", &mut out);
                crate::types::keys::req_key::<GuestKind>(obj, "kind", &mut out);
                crate::types::keys::req_key::<String>(obj, "token", &mut out);
                crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
                crate::types::keys::req_key::<Vec<String>>(obj, "argv", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("guest.message") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "session", &mut out);
                crate::types::keys::req_key::<Value>(obj, "message", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("guest.closed") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "session", &mut out);
                crate::types::keys::opt_key::<String>(obj, "error", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("type", tag)]),
        }
    }
}

// Refinements on the variants of the unions above, each an issue when it fails:
// - DaemonEvent::BrowserOpen.url: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=

/// Reads one daemon event as zod's discriminated union on `type` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `type` is
/// one invalid_union_discriminator issue on ["type"] whose detail `options` lists every type of DaemonEvent in order and
/// whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the variant's fields in the
/// schema's order, then its Check. Unknown keys are dropped.
pub fn parse_daemon_event(frame: &Value) -> Result<DaemonEvent, SchemaError> {
    parse::<DaemonEvent>(frame)
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
        Ok(())
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["line", "raw"]))
    }
}
