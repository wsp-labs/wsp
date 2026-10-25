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
        let Some(tag) = obj.get("type") else { return Some(vec![crate::types::check::missing::<Self>("type")]); };
        match tag.as_str() {
            Some("daemon.hello") => {
                let mut out = Vec::new();
                match obj.get("root") { None => out.push(crate::types::check::required::<String>("root")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("root"))), } } }
                match obj.get("version") { None => out.push(crate::types::check::required::<JsInt>("version")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.data") => {
                let mut out = Vec::new();
                match obj.get("ptyId") { None => out.push(crate::types::check::required::<String>("ptyId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ptyId"))), } } }
                match obj.get("data") { None => out.push(crate::types::check::required::<String>("data")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("data"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.exit") => {
                let mut out = Vec::new();
                match obj.get("ptyId") { None => out.push(crate::types::check::required::<String>("ptyId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ptyId"))), } } }
                match obj.get("exitCode") { None => out.push(crate::types::check::required::<JsNumber>("exitCode")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exitCode"))), } } }
                if let Some(v) = obj.get("signal") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signal"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("port.open") => {
                let mut out = Vec::new();
                match obj.get("port") { None => out.push(crate::types::check::required::<JsNumber>("port")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } } }
                if let Some(v) = obj.get("pid") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } }
                if let Some(v) = obj.get("process") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("process"))), } }
                if let Some(v) = obj.get("loopback") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("loopback"))), } }
                if let Some(v) = obj.get("watch") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("watch"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("port.close") => {
                let mut out = Vec::new();
                match obj.get("port") { None => out.push(crate::types::check::required::<JsNumber>("port")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } } }
                if let Some(v) = obj.get("pid") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } }
                if let Some(v) = obj.get("process") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("process"))), } }
                if let Some(v) = obj.get("command") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("command"))), } }
                if let Some(v) = obj.get("exited") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exited"))), } }
                if let Some(v) = obj.get("left") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("left"))), } }
                if let Some(v) = obj.get("at") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } }
                if let Some(v) = obj.get("watch") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("watch"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("inbox.file") => {
                let mut out = Vec::new();
                match obj.get("path") { None => out.push(crate::types::check::required::<String>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
                match obj.get("bytes") { None => out.push(crate::types::check::required::<JsNumber>("bytes")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("bytes"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.mode") => {
                let mut out = Vec::new();
                match obj.get("ptyId") { None => out.push(crate::types::check::required::<String>("ptyId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ptyId"))), } } }
                match obj.get("mode") { None => out.push(crate::types::check::required::<PtyModeMode>("mode")), Some(v) => { match crate::types::check::read::<PtyModeMode>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("mode"))), } } }
                match obj.get("echo") { None => out.push(crate::types::check::required::<bool>("echo")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("echo"))), } } }
                match obj.get("foreground") { None => out.push(crate::types::check::required::<String>("foreground")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("foreground"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("browser.open") => {
                let mut out = Vec::new();
                match obj.get("url") { None => out.push(crate::types::check::required::<String>("url")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } } }
                if let Some(v) = obj.get("port") { match crate::types::check::read_present::<RelayPort>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("callback.port") => {
                let mut out = Vec::new();
                match obj.get("port") { None => out.push(crate::types::check::required::<RelayPort>("port")), Some(v) => { match crate::types::check::read::<RelayPort>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("tunnel.data") => {
                let mut out = Vec::new();
                match obj.get("tunnelId") { None => out.push(crate::types::check::required::<String>("tunnelId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tunnelId"))), } } }
                match obj.get("data") { None => out.push(crate::types::check::required::<String>("data")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("data"))), } } }
                if let Some(v) = obj.get("machineId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("tunnel.end") => {
                let mut out = Vec::new();
                match obj.get("tunnelId") { None => out.push(crate::types::check::required::<String>("tunnelId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tunnelId"))), } } }
                if let Some(v) = obj.get("machineId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.out") => {
                let mut out = Vec::new();
                match obj.get("stdioId") { None => out.push(crate::types::check::required::<String>("stdioId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stdioId"))), } } }
                match obj.get("data") { None => out.push(crate::types::check::required::<String>("data")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("data"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.err") => {
                let mut out = Vec::new();
                match obj.get("stdioId") { None => out.push(crate::types::check::required::<String>("stdioId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stdioId"))), } } }
                match obj.get("data") { None => out.push(crate::types::check::required::<String>("data")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("data"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.exit") => {
                let mut out = Vec::new();
                match obj.get("stdioId") { None => out.push(crate::types::check::required::<String>("stdioId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stdioId"))), } } }
                if let Some(v) = obj.get("code") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("code"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("localhost.url") => {
                let mut out = Vec::new();
                match obj.get("port") { None => out.push(crate::types::check::required::<RelayPort>("port")), Some(v) => { match crate::types::check::read::<RelayPort>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.sample") => { let mut body = obj.clone(); body.shift_remove("type"); Some(crate::types::check::issues_of::<SysSample>(&serde_json::Value::Object(body))) }
            Some("proc.snapshot") => { let mut body = obj.clone(); body.shift_remove("type"); Some(crate::types::check::issues_of::<ProcSnapshot>(&serde_json::Value::Object(body))) }
            Some("proc.changes") => { let mut body = obj.clone(); body.shift_remove("type"); Some(crate::types::check::issues_of::<ProcChanges>(&serde_json::Value::Object(body))) }
            Some("guest.opened") => {
                let mut out = Vec::new();
                match obj.get("session") { None => out.push(crate::types::check::required::<String>("session")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("session"))), } } }
                match obj.get("life") { None => out.push(crate::types::check::required::<String>("life")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("life"))), } } }
                match obj.get("kind") { None => out.push(crate::types::check::required::<GuestKind>("kind")), Some(v) => { match crate::types::check::read::<GuestKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
                match obj.get("token") { None => out.push(crate::types::check::required::<String>("token")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("token"))), } } }
                if let Some(v) = obj.get("turnToken") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnToken"))), } }
                match obj.get("argv") { None => out.push(crate::types::check::required::<Vec<String>>("argv")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("argv"))), } } }
                match obj.get("cwd") { None => out.push(crate::types::check::required::<String>("cwd")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cwd"))), } } }
                if let Some(v) = obj.get("machineId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("guest.message") => {
                let mut out = Vec::new();
                match obj.get("session") { None => out.push(crate::types::check::required::<String>("session")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("session"))), } } }
                match obj.get("message") { None => out.push(crate::types::check::required::<Value>("message")), Some(v) => { match crate::types::check::read::<Value>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("message"))), } } }
                if let Some(v) = obj.get("machineId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("guest.closed") => {
                let mut out = Vec::new();
                match obj.get("session") { None => out.push(crate::types::check::required::<String>("session")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("session"))), } } }
                if let Some(v) = obj.get("error") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("error"))), } }
                if let Some(v) = obj.get("machineId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machineId"))), } }
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
