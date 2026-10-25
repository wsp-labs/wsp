// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/devices.ts.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::workspace::ThreadScope;
use crate::wire::machine_link::DaemonResponse;
use wsp_frames::{False, True};

use crate::views::workspace::WorkspaceOrigin;

/// A join or pairing token read back: the code as the host takes it, and the host key where the token carried one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinToken {
    pub code: String,
    pub host_key: Option<String>,
}

// devices.ts:184
// export const TICKET_ORIGIN: Record<TicketPurpose, WorkspaceOrigin> = { connect: "here", relay: "relayed" };
/// Where a socket redeeming a ticket of each purpose reached the host from: connect is here, relay is relayed.
pub fn ticket_origin(purpose: TicketPurpose) -> WorkspaceOrigin {
    match purpose {
        TicketPurpose::Connect => WorkspaceOrigin::Here,
        TicketPurpose::Relay => WorkspaceOrigin::Relayed,
    }
}

pub const PAIR_CODE_TTL_MS: i64 = 600000;
pub const PAIR_CODE_LENGTH: i64 = 8;
pub const PAIR_CODE_ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const PAIR_CODE_SHAPE_REFUSAL: &str =
    "that is not a pairing code, which is 8 letters and digits like ABCD-EFGH; paste only the code wsp host pair printed";
pub const ACCOUNT_TICKET_REFUSAL: &str =
    "a socket let in on a ticket cannot see the account this host is signed in to; run wsp login on the computer the host runs on";
pub const DEFAULT_RELAY: &str = "https://relay.usewsp.com";
pub const DEVICE_AUTH_REFUSAL: &str = "this host admits a computer on the account only on an admission signed by a key it already trusts; run wsp login to read this computer's id on a computer that is already in, then wsp login <id> there, and dial again";
pub const DEVICE_REVOKED_REFUSAL: &str = "this host took this computer's token away; it is admitted through the account no longer, and wsp host pair on the host is the way back in";
pub const DEVICE_ACCOUNT_UNSERVED: &str =
    "this host is on no account, so it admits no computer through one; run wsp host link on the computer it runs on to put it on yours";
pub const LOGIN_NO_KEY_REFUSAL: &str = "that word names no key for the computer signing in, so nothing here could say which key it would be admitting; run wsp login there again and copy the whole word it prints";
pub const ACCOUNT_UNSERVED: &str = "this host keeps no account records; wsp up serves them";
pub const PAIR_ISSUE_REFUSAL: &str =
    "only a socket holding this host's own token may mint a pairing code; run wsp host pair on the computer the host runs on";
pub const PAIR_CODE_REFUSAL: &str = "that pairing code is not one this host is waiting for; run wsp host pair on the host for a fresh one";
pub const DEVICES_TICKET_REFUSAL: &str = "a socket let in on a ticket cannot see or change the devices paired with this host; run wsp host devices on the computer the host runs on";
pub const API_UNAUTHORIZED: &str = "this route needs a token in an Authorization header, the host's own from the token file beside its state or a paired device's; run wsp host pair on the computer the host runs on for one";
pub const JOIN_TOKEN_MARK: &str = ".";
pub const JOIN_NO_KEY_REFUSAL: &str = "that join line names no key for the host, so this computer cannot tell which host it is joining; run wsp add on the host again and copy the whole code it prints";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TicketPurpose {
    #[serde(rename = "connect")]
    Connect,
    #[serde(rename = "relay")]
    Relay,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceVia {
    pub kind: String,
    pub relay_device_id: String,
    pub fingerprint: String,
    pub public_key: String,
    pub admitted_by: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub scope: Option<ThreadScope>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub here: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub via: Option<DeviceVia>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub signed_in: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub login: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Admission {
    pub device: String,
    pub by: String,
    pub issued_at: String,
    pub signature: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDeviceAdmission {
    pub by: String,
    pub issued_at: String,
    pub signature: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDevice {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub admissions: Vec<AccountDeviceAdmission>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDevices {
    pub devices: Vec<AccountDevice>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonFrame {
    pub op: String,
    /// zod passthrough: unknown keys kept, in the order read.
    #[serde(flatten, serialize_with = "crate::types::check::js_record")]
    pub rest: IndexMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonOpenReply {
    pub channel: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonSendReply {
    pub reply: DaemonResponse,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonEventEvent {
    pub r#type: String,
    /// zod passthrough: unknown keys kept, in the order read.
    #[serde(flatten, serialize_with = "crate::types::check::js_record")]
    pub rest: IndexMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonChannelEvent {
    #[serde(rename = "daemon.event", rename_all = "camelCase")]
    DaemonEvent { channel: String, event: DaemonEventEvent },
    #[serde(rename = "daemon.closed", rename_all = "camelCase")]
    DaemonClosed { channel: String, code: JsInt, reason: String },
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `kind`: literal "account"
impl Check for DeviceVia {
    fn check(&self) -> Result<(), SchemaError> {
        if self.kind != "account" {
            let mut detail = indexmap::IndexMap::new();
            detail.insert("received".to_string(), serde_json::Value::from(self.kind.as_str()));
            detail.insert("expected".to_string(), serde_json::Value::from("account"));
            let message = "Invalid literal value, expected \"account\"".to_string();
            return Err(SchemaError { code: "invalid_literal".into(), path: vec![serde_json::Value::from("kind")], message, detail });
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_kind(&x) { out.push(e.under("kind")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        match obj.get("relayDeviceId") { None => out.push(crate::types::check::missing::<Self>("relayDeviceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("relayDeviceId"))), } } }
        match obj.get("fingerprint") { None => out.push(crate::types::check::missing::<Self>("fingerprint")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fingerprint"))), } } }
        match obj.get("publicKey") { None => out.push(crate::types::check::missing::<Self>("publicKey")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("publicKey"))), } } }
        match obj.get("admittedBy") { None => out.push(crate::types::check::missing::<Self>("admittedBy")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("admittedBy"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => crate::types::check::leaf(rest, crate::types::check::Zod::Literal(|| serde_json::Value::from("account"))),
                "relayDeviceId" => <String as Check>::describe(rest),
                "fingerprint" => <String as Check>::describe(rest),
                "publicKey" => <String as Check>::describe(rest),
                "admittedBy" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl DeviceVia {
    fn rule_kind(x: &String) -> Result<(), crate::types::check::SchemaError> {
        if (*x) != "account" {
            let mut detail = indexmap::IndexMap::new();
            detail.insert("received".to_string(), serde_json::Value::from((*x).as_str()));
            detail.insert("expected".to_string(), serde_json::Value::from("account"));
            let message = "Invalid literal value, expected \"account\"".to_string();
            return Err(SchemaError { code: "invalid_literal".into(), path: Vec::new(), message, detail });
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DeviceView {
    fn check(&self) -> Result<(), SchemaError> {
        self.scope.check().map_err(|e| e.under("scope"))?;
        self.via.check().map_err(|e| e.under("via"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("createdAt") { None => out.push(crate::types::check::missing::<Self>("createdAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("createdAt"))), } } }
        match obj.get("lastSeenAt") { None => out.push(crate::types::check::missing::<Self>("lastSeenAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("lastSeenAt"))), } } }
        if let Some(v) = obj.get("scope") { match crate::types::check::read_present::<ThreadScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } }
        if let Some(v) = obj.get("here") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("here"))), } }
        if let Some(v) = obj.get("via") { match crate::types::check::read_present::<DeviceVia>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("via"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "lastSeenAt" => <String as Check>::describe(rest),
                "scope" => <Option<ThreadScope> as Check>::describe(rest),
                "here" => crate::types::check::leaf(rest, crate::types::check::Zod::Literal(|| serde_json::Value::from(true))),
                "via" => <Option<DeviceVia> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for AccountView {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("signedIn") { None => out.push(crate::types::check::missing::<Self>("signedIn")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signedIn"))), } } }
        if let Some(v) = obj.get("login") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("login"))), } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "signedIn" => <bool as Check>::describe(rest),
                "login" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for Admission {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("device") { None => out.push(crate::types::check::missing::<Self>("device")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("device"))), } } }
        match obj.get("by") { None => out.push(crate::types::check::missing::<Self>("by")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("by"))), } } }
        match obj.get("issuedAt") { None => out.push(crate::types::check::missing::<Self>("issuedAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("issuedAt"))), } } }
        match obj.get("signature") { None => out.push(crate::types::check::missing::<Self>("signature")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signature"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "device" => <String as Check>::describe(rest),
                "by" => <String as Check>::describe(rest),
                "issuedAt" => <String as Check>::describe(rest),
                "signature" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for AccountDevice {
    fn check(&self) -> Result<(), SchemaError> {
        self.admissions.check().map_err(|e| e.under("admissions"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("fingerprint") { None => out.push(crate::types::check::missing::<Self>("fingerprint")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fingerprint"))), } } }
        match obj.get("admissions") { None => out.push(crate::types::check::missing::<Self>("admissions")), Some(v) => { match crate::types::check::read::<Vec<AccountDeviceAdmission>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("admissions"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "fingerprint" => <String as Check>::describe(rest),
                "admissions" => <Vec<AccountDeviceAdmission> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for AccountDeviceAdmission {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("by") { None => out.push(crate::types::check::missing::<Self>("by")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("by"))), } } }
        match obj.get("issuedAt") { None => out.push(crate::types::check::missing::<Self>("issuedAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("issuedAt"))), } } }
        match obj.get("signature") { None => out.push(crate::types::check::missing::<Self>("signature")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signature"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "by" => <String as Check>::describe(rest),
                "issuedAt" => <String as Check>::describe(rest),
                "signature" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for AccountDevices {
    fn check(&self) -> Result<(), SchemaError> {
        self.devices.check().map_err(|e| e.under("devices"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("devices") { None => out.push(crate::types::check::missing::<Self>("devices")), Some(v) => { match crate::types::check::read::<Vec<AccountDevice>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("devices"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "devices" => <Vec<AccountDevice> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
impl Check for DaemonFrame {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::refine(self.op != "auth", "the host authenticates the channel").map_err(|e| e.under("op"))
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("op") { None => out.push(crate::types::check::missing::<Self>("op")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(x) => { if let Err(e) = Self::rule_op(&x) { out.push(e.under("op")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("op"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "op" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl DaemonFrame {
    fn rule_op(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::refine((*x) != "auth", "the host authenticates the channel")?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonOpenReply {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("channel") { None => out.push(crate::types::check::missing::<Self>("channel")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("channel"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "channel" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonSendReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.reply.check().map_err(|e| e.under("reply"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("reply") { None => out.push(crate::types::check::missing::<Self>("reply")), Some(v) => { match crate::types::check::read::<DaemonResponse>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reply"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "reply" => <DaemonResponse as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonChannelEvent {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            DaemonChannelEvent::DaemonEvent { event, .. } => event.check().map_err(|e| e.under("event")),
            DaemonChannelEvent::DaemonClosed { .. } => Ok(()),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::tag_describe(path, "type", &["daemon.event", "daemon.closed"])
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("type") else { return Some(vec![crate::types::check::missing::<Self>("type")]); };
        match tag.as_str() {
            Some("daemon.event") => {
                let mut out = Vec::new();
                match obj.get("channel") { None => out.push(crate::types::check::required::<String>("channel")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("channel"))), } } }
                match obj.get("event") { None => out.push(crate::types::check::required::<DaemonEventEvent>("event")), Some(v) => { match crate::types::check::read::<DaemonEventEvent>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("event"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("daemon.closed") => {
                let mut out = Vec::new();
                match obj.get("channel") { None => out.push(crate::types::check::required::<String>("channel")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("channel"))), } } }
                match obj.get("code") { None => out.push(crate::types::check::required::<JsInt>("code")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("code"))), } } }
                match obj.get("reason") { None => out.push(crate::types::check::required::<String>("reason")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reason"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("type", tag)]),
        }
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonEventEvent {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("type") { None => out.push(crate::types::check::missing::<Self>("type")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("type"))), } } }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "type" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// devices.ts:68
// /** A pairing code as every screen shows it: the alphabet's letters in two halves, which is how a person reads one
//  * across a room. The one grouping, so the sheet that shows a code and the field that takes one agree. */
// export function shownPairCode(code: string): string {
//   const letters = code.replace(/-/g, "").toUpperCase().slice(0, PAIR_CODE_LENGTH);
//   const half = Math.ceil(PAIR_CODE_LENGTH / 2);
//   return letters.length <= half ? letters : `${letters.slice(0, half)}-${letters.slice(half)}`;
// }
pub fn shown_pair_code(code: &str) -> String {
    let letters: Vec<u16> = sent_pair_code(code).encode_utf16().take(PAIR_CODE_LENGTH as usize).collect();
    let half = (PAIR_CODE_LENGTH as usize).div_ceil(2);
    if letters.len() <= half {
        return String::from_utf16_lossy(&letters);
    }
    let (first, rest) = letters.split_at(half);
    format!("{}-{}", String::from_utf16_lossy(first), String::from_utf16_lossy(rest))
}
// devices.ts:76
// /** A pairing code as the host takes it, whichever screen it was copied off: the letters alone, upper case. A
//  * person copies the code they can read, so the dash the screens put in it is one this reading takes back out. */
// export const sentPairCode = (shown: string): string => shown.replace(/-/g, "").toUpperCase();
pub fn sent_pair_code(shown: &str) -> String {
    shown.replace('-', "").to_uppercase()
}
// devices.ts:85
// /** Whether a code as the host takes it could be one a host is holding: the alphabet's characters at the length a code
//  * is. What tells a code from a paste that caught a label, a space for the dash or a character too many. */
// export const isPairCode = (code: string): boolean => code.length === PAIR_CODE_LENGTH && [...code].every(c => PAIR_CODE_ALPHABET.includes(c));
pub fn is_pair_code(code: &str) -> bool {
    code.encode_utf16().count() == PAIR_CODE_LENGTH as usize && code.chars().all(|c| PAIR_CODE_ALPHABET.contains(c))
}
// devices.ts:137
// /** What a device signs to prove it may come in through the account: the key admitted, the key that signed for it
//  * and the moment, built by one function so the wsp that signs and the host that verifies cannot drift. The host
//  * is not inside it: one admission stands at every host on the account whose trust the signer already has, which
//  * is what saves a person a code per host. */
// export function deviceAdmissionTranscript(device: string, by: string, issuedAt: string): Uint8Array {
//   return new TextEncoder().encode(`wsp device admission v1\n${device}\n${by}\n${issuedAt}\n`);
// }
pub fn device_admission_transcript(device: &str, by: &str, issued_at: &str) -> Vec<u8> {
    format!("wsp device admission v1\n{device}\n{by}\n{issued_at}\n").into_bytes()
}
// devices.ts:160
// /** What a computer reads when the host it dialled answered device.auth with its own request schema's refusal: a
//  * host of an older wsp, whose door knows no road in for a computer on the account. Told apart from a refusal of this build by
//  * the kind on the frame, which an older host's schema refusal carries none of, so a token this computer never sent
//  * is never read as one that was taken away. */
// export const deviceAuthOldHostLine = (where: string): string =>
//   `the host at ${where} runs an older wsp, whose door does not know how a computer on the account comes in; update wsp on that computer and run wsp up there again`;
pub fn device_auth_old_host_line(r#where: &str) -> String {
    format!("the host at {where} runs an older wsp, whose door does not know how a computer on the account comes in; update wsp on that computer and run wsp up there again")
}
// devices.ts:191
// /** The refusal a write route and a browser's upgrade answer with when the page that sent them was loaded at
//  * another name than the one this host was reached at. A page may only drive the host it was served by, and the
//  * hostname is the whole of the reading: a page on the app's port dialling the runtime's is the same page. */
// export function crossOriginRefusal(origin: string, host: string): string {
//   return `this request came from ${origin} and this host was reached at ${host}; the page and its socket open from the address the host answers at`;
// }
pub fn cross_origin_refusal(origin: &str, host: &str) -> String {
    format!("this request came from {origin} and this host was reached at {host}; the page and its socket open from the address the host answers at")
}
// devices.ts:225
// /** The one word a person copies off a join line: the single-use code and the fingerprint of the key the host will
//  * prove, as one string, so a join stays two things to copy and the screens keep the fields they have. */
// export const joinToken = (code: string, hostKey: string): string => `${shownPairCode(code)}${JOIN_TOKEN_MARK}${hostKey}`;
pub fn join_token(code: &str, host_key: &str) -> String {
    format!("{}{JOIN_TOKEN_MARK}{host_key}", shown_pair_code(code))
}
// devices.ts:229
// /** The same token read back on the computer being joined, whichever road it came by: a person's paste, the flag,
//  * or the file an install over ssh landed. The code is taken as any screen's code is taken; the fingerprint is left
//  * exactly as it was written, since its own alphabet is case sensitive. A token that carries no fingerprint answers
//  * none, and the caller refuses rather than dialling. */
// export function readJoinToken(typed: string): { code: string; hostKey?: string } {
//   const trimmed = typed.trim();
//   const at = trimmed.indexOf(JOIN_TOKEN_MARK);
//   if (at === -1) return { code: sentPairCode(trimmed) };
//   const hostKey = trimmed.slice(at + 1).trim();
//   const code = sentPairCode(trimmed.slice(0, at));
//   return hostKey === "" ? { code } : { code, hostKey };
// }
pub fn read_join_token(typed: &str) -> JoinToken {
    let trimmed = crate::types::check::js_trim(typed);
    match trimmed.find(JOIN_TOKEN_MARK) {
        None => JoinToken { code: sent_pair_code(trimmed), host_key: None },
        Some(at) => {
            let key = crate::types::check::js_trim(&trimmed[at + 1..]);
            JoinToken { code: sent_pair_code(&trimmed[..at]), host_key: (!key.is_empty()).then(|| key.to_string()) }
        }
    }
}
// devices.ts:246
// /** The refusal a join gets when the host at that address proved a key that is not the one the join line named:
//  * something answered where the host was expected. Nothing of this computer's went to it. */
// export const joinKeyRefusal = (url: string): string => `the host at ${url} proved a key the join line did not name, so it is not the host that printed that line; nothing was sent to it`;
pub fn join_key_refusal(url: &str) -> String {
    format!(
        "the host at {url} proved a key the join line did not name, so it is not the host that printed that line; nothing was sent to it"
    )
}
// devices.ts:250
// /** The one word a person copies off wsp host pair, which is the join line's token under the name the pairing road
//  * reads it by: the code and the fingerprint of the key the host will prove, so a client pins that key before it
//  * spends the code. `readJoinToken` reads both roads' tokens, since they are one shape. */
// export const pairToken = joinToken;
pub fn pair_token(code: &str, host_key: &str) -> String {
    join_token(code, host_key)
}
// devices.ts:255
// /** The refusal a dial gets when whatever answered at that address did not prove the key this computer holds that
//  * host to, whether it proved another one or signed nothing this computer could verify: it is not that host,
//  * whichever check caught it, and no token of this computer's went to it. */
// export const pairKeyRefusal = (url: string): string => `the host at ${url} did not prove the key this computer holds for it, so it is not that host; nothing was sent to it`;
pub fn pair_key_refusal(url: &str) -> String {
    format!("the host at {url} did not prove the key this computer holds for it, so it is not that host; nothing was sent to it")
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for TicketPurpose {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["connect", "relay"]))
    }
}
