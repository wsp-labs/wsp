// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/daemon-events.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::helpers::ReqId;
use crate::types::check::{leaf, parse, refine, Zod};
use crate::types::{Check, JsNumber, SchemaError};
use crate::wire::daemon::GuestKind;
use crate::wire::events_union::{not_an_object, union_discriminator};
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

/// A struct variant's fields read as a struct of their own: the internally tagged enum answers field errors at the root.
macro_rules! struct_variants {
    ($($tag:literal => $variant:ident as $body:ident { $($key:literal => $field:ident : $ty:ty),* $(,)? })*) => {
        $(
            #[derive(Deserialize)]
            struct $body {
                $(#[serde(rename = $key)] $field: $ty,)*
            }

            impl Check for $body {
                fn describe(path: &[Value]) -> Option<Zod> {
                    match path.split_first() {
                        None => Some(Zod::Word("object")),
                        Some((key, rest)) => match key.as_str()? {
                            $($key => <$ty as Check>::describe(rest),)*
                            _ => None,
                        },
                    }
                }
            }
        )*

        fn read_struct_variant(tag: &str, obj: &Map<String, Value>) -> Option<Result<DaemonEvent, SchemaError>> {
            match tag {
                $($tag => Some(read_body::<$body>(obj).map(|$body { $($field),* }| DaemonEvent::$variant { $($field),* })),)*
                _ => None,
            }
        }
    };
}

struct_variants! {
    "daemon.hello" => DaemonHello as DaemonHelloBody { "root" => root: String, "version" => version: i64 }
    "pty.data" => PtyData as PtyDataBody { "ptyId" => pty_id: String, "data" => data: String }
    "pty.exit" => PtyExit as PtyExitBody { "ptyId" => pty_id: String, "exitCode" => exit_code: JsNumber, "signal" => signal: Option<JsNumber> }
    "port.open" => PortOpen as PortOpenBody { "port" => port: JsNumber, "pid" => pid: Option<JsNumber>, "process" => process: Option<String>, "loopback" => loopback: Option<bool>, "watch" => watch: Option<String> }
    "port.close" => PortClose as PortCloseBody { "port" => port: JsNumber, "pid" => pid: Option<JsNumber>, "process" => process: Option<String>, "command" => command: Option<String>, "exited" => exited: Option<bool>, "left" => left: Option<bool>, "at" => at: Option<String>, "watch" => watch: Option<String> }
    "inbox.file" => InboxFile as InboxFileBody { "path" => path: String, "bytes" => bytes: JsNumber }
    "pty.mode" => PtyMode as PtyModeBody { "ptyId" => pty_id: String, "mode" => mode: PtyModeMode, "echo" => echo: bool, "foreground" => foreground: String }
    "browser.open" => BrowserOpen as BrowserOpenBody { "url" => url: String, "port" => port: Option<RelayPort> }
    "callback.port" => CallbackPort as CallbackPortBody { "port" => port: RelayPort }
    "tunnel.data" => TunnelData as TunnelDataBody { "tunnelId" => tunnel_id: String, "data" => data: String, "machineId" => machine_id: Option<String> }
    "tunnel.end" => TunnelEnd as TunnelEndBody { "tunnelId" => tunnel_id: String, "machineId" => machine_id: Option<String> }
    "stdio.out" => StdioOut as StdioOutBody { "stdioId" => stdio_id: String, "data" => data: String }
    "stdio.err" => StdioErr as StdioErrBody { "stdioId" => stdio_id: String, "data" => data: String }
    "stdio.exit" => StdioExit as StdioExitBody { "stdioId" => stdio_id: String, "code" => code: Option<i64> }
    "localhost.url" => LocalhostUrl as LocalhostUrlBody { "port" => port: RelayPort }
    "guest.opened" => GuestOpened as GuestOpenedBody { "session" => session: String, "life" => life: String, "kind" => kind: GuestKind, "token" => token: String, "turnToken" => turn_token: Option<String>, "argv" => argv: Vec<String>, "cwd" => cwd: String, "machineId" => machine_id: Option<String> }
    "guest.message" => GuestMessage as GuestMessageBody { "session" => session: String, "message" => message: Value, "machineId" => machine_id: Option<String> }
    "guest.closed" => GuestClosed as GuestClosedBody { "session" => session: String, "error" => error: Option<String>, "machineId" => machine_id: Option<String> }
}

fn read_body<T: DeserializeOwned + Check>(obj: &Map<String, Value>) -> Result<T, SchemaError> {
    let mut rest = obj.clone();
    rest.shift_remove("type");
    parse(&Value::Object(rest))
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
}

// Refinements on the variants of the unions above, each an issue when it fails:
// - DaemonEvent::BrowserOpen.url: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=

/// Reads one daemon event as zod's discriminated union on `type` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `type` is
/// one invalid_union_discriminator issue on ["type"] whose detail `options` lists every type of DaemonEvent in order and
/// whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the variant's first failing
/// field, then its Check. Unknown keys are dropped.
pub fn parse_daemon_event(frame: &Value) -> Result<DaemonEvent, SchemaError> {
    let Value::Object(obj) = frame else { return Err(not_an_object(frame)) };
    let tag = obj.get("type").and_then(Value::as_str).unwrap_or_default();
    let event = match tag {
        "sys.sample" => read_body(obj).map(DaemonEvent::SysSample)?,
        "proc.snapshot" => read_body(obj).map(DaemonEvent::ProcSnapshot)?,
        "proc.changes" => read_body(obj).map(DaemonEvent::ProcChanges)?,
        _ => read_struct_variant(tag, obj).ok_or_else(|| union_discriminator(DAEMON_EVENT_TYPES))??,
    };
    event.check()?;
    Ok(event)
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
