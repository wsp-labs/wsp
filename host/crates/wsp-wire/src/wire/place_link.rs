// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/place-link.ts.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::place::CopyWord;
use crate::wire::capabilities::WorkspaceSize;
use wsp_frames::{False, True};
/// base64 of 32, 44, 64 and 32 bytes, read as zod's base64(n) reads them.
pub use wsp_frames::{PlaceEphemeral, PlaceNonce, PlacePublicKey, PlaceSignature};

/// The bytes both ends of a link sign, and the bytes a host signs to refuse a place: wsp-frames builds them for the
/// daemon's side, so the host reads the one function.
pub use wsp_frames::{place_link_transcript, place_refusal_transcript, LinkEphemerals, LinkRole};
/// What a computer joined as a place keeps about the wsp it belongs to (place-link.ts:410), as parse_place_file reads
/// it: the six fields it checks, `joinedAt` as whatever the file holds there (never checked), and every other key
/// kept in its order, since place_file_text writes back what was read. wsp_frames::PlaceFile is the daemon's stricter
/// reading and is not this one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceFile {
    pub place_id: String,
    pub name: String,
    pub host_name: String,
    pub host_urls: Vec<String>,
    pub host_public_key: String,
    pub key_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub joined_at: Option<Value>,
    #[serde(flatten, serialize_with = "crate::types::check::js_record")]
    pub rest: IndexMap<String, Value>,
}

/// The engine a project's own containers would run on: Docker first, then podman, else none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaceEngine {
    None,
    Docker,
    Podman,
}

/// What the held read of a box says stands there: a join, or the path of what a join left broken.
#[derive(Debug, Clone, PartialEq)]
pub enum HeldPlace {
    Joined(PlaceFile),
    Broken(String),
}

/// The most agents one place report may name, and the most keys its agentVersions may hold.
pub const AGENTS_REPORTED_MAX: usize = 32;

// place-link.ts:387
// export const JOIN_ADDRESS_LINE: TwoPartRefusal = {
//   what: "That is not an address.",
//   fix: `Type it as the other screen shows it, like 192.168.1.20:${DEFAULT_PLACE_PORT}.`,
// };
/// What the sheet says for a join address that is none, with the place door's default port in it.
pub fn join_address_line() -> TwoPartRefusal {
    todo!()
}

pub const PLACE_LINK_NONCE_BYTES: i64 = 32;
pub const SEAL_CLIENT: &str = "client";
pub const SEAL_UNSERVED: &str = "this host holds no key to prove itself with; the host that serves the app wires one";
pub const LAUNCHED_WITH: &str = "the launch this turn started with";
pub const PLACE_CODE_REFUSAL: &str = "that join code is not one this host is waiting for; run wsp add on the host for a fresh one";
pub const PLACE_UNSEALED_JOIN_REFUSAL: &str =
    "that computer's wsp is older than this host and seals no link; update wsp there and join again";
pub const PLACE_KEY_REFUSAL: &str =
    "that place's key does not match the one this host learned at join; wsp remove it here and join it again";
pub const PLACE_UNKNOWN_REFUSAL: &str = "this host holds no place by that id; join it with a code from wsp add";
pub const NO_PLACE_INSTALLER: &str =
    "this host cannot install the agent on a computer over ssh; run wsp add with no argument for the line to type on that computer";
pub const PLACES_TICKET_REFUSAL: &str =
    "a socket let in on a ticket cannot see or change the places this host holds; run wsp computers on the computer the host runs on";
pub const DAEMON_OPEN_ONE_OF: &str = "daemon.open opens a channel to one daemon: name workspaceId or placeId, not both";
pub const SYS_SUBSCRIBE_ONE_OF: &str = "sys.subscribe and sys.unsubscribe read one daemon: name workspaceId or placeId, not both";
pub const GIT_FORGE_HOSTS: &[&str] =
    &["github.com", "gitlab.com", "bitbucket.org", "codeberg.org", "sr.ht", "ssh.dev.azure.com", "vs-ssh.visualstudio.com"];
pub const MINT_JOIN_REFUSAL: &str =
    "only a socket holding this host's own token may mint a join code; run wsp add on the computer the host runs on";
pub const SSH_HOSTS_REFUSAL: &str =
    "only a socket holding this host's own token may read the ssh hosts on this computer; open wsp on the computer the host runs on";
pub const PLACE_DOOR_REFUSAL: &str =
    "a socket let in on a ticket cannot open the door computers you own dial; run wsp add on the computer the host runs on";
pub const PLACE_DOOR_UNSERVED: &str = "this host opens no door for computers you own; wsp up serves one";
pub const CODE_GOOD_LINE: &str = "the code is good for 10 minutes";
pub const CODE_EXPIRED_LINE: &str = "the code expired; press New code";
pub const PLACE_FILE_MODE: i64 = 384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceReportPlatform {
    #[serde(rename = "darwin")]
    Darwin,
    #[serde(rename = "linux")]
    Linux,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceReportEngine {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "docker")]
    Docker,
    #[serde(rename = "podman")]
    Podman,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceReport {
    pub name: String,
    pub platform: PlaceReportPlatform,
    pub arch: String,
    pub os: String,
    pub shape: WorkspaceSize,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_free_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_size_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(serialize_with = "crate::types::check::js_record")]
    pub login: IndexMap<String, String>,
    pub runs_workspaces: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub workspaces_blocked: Option<String>,
    pub engine: PlaceReportEngine,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copies: Option<CopyWord>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub uptime_ms: Option<i64>,
    pub daemon_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_port: Option<i64>,
    pub wsp: Vec<String>,
    pub agents: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present", serialize_with = "crate::types::check::js_record_opt")]
    pub agent_versions: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub logins: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wsp_door: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wsp_door_blocked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub takes_runtime: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wsp_daemon_version: Option<i64>,
    pub dialed: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceJoinRequest {
    pub id: ReqId,
    pub public_key: PlacePublicKey,
    pub nonce: PlaceNonce,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ephemeral: Option<PlaceEphemeral>,
}

/// A member of a union tagged on `op`: the union writes and reads the tag, so the struct holds none.
impl PlaceJoinRequest {
    pub const OP: &str = "place.join";
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceJoinReply {
    pub place_id: String,
    pub host_public_key: PlacePublicKey,
    pub nonce: PlaceNonce,
    pub signature: PlaceSignature,
    pub ephemeral: PlaceEphemeral,
    pub host_name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceJoinDevice {
    pub device_id: String,
    pub device_token: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAuthRequest {
    pub id: ReqId,
    pub place_id: String,
    pub nonce: PlaceNonce,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ephemeral: Option<PlaceEphemeral>,
}

/// A member of a union tagged on `op`: the union writes and reads the tag, so the struct holds none.
impl PlaceAuthRequest {
    pub const OP: &str = "place.auth";
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAuthReply {
    pub nonce: PlaceNonce,
    pub host_public_key: PlacePublicKey,
    pub signature: PlaceSignature,
    pub ephemeral: PlaceEphemeral,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAuthRefusal {
    pub host_public_key: PlacePublicKey,
    pub signature: PlaceSignature,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceProveRequestClient {
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceProveRequest {
    pub id: ReqId,
    pub signature: PlaceSignature,
    pub report: PlaceReport,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub client: Option<PlaceProveRequestClient>,
}

/// A member of a union tagged on `op`: the union writes and reads the tag, so the struct holds none.
impl PlaceProveRequest {
    pub const OP: &str = "place.prove";
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealOpenRequest {
    pub id: ReqId,
    pub nonce: PlaceNonce,
    pub ephemeral: PlaceEphemeral,
}

/// A member of a union tagged on `op`: the union writes and reads the tag, so the struct holds none.
impl SealOpenRequest {
    pub const OP: &str = "seal.open";
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealOpenReply {
    pub nonce: PlaceNonce,
    pub host_public_key: PlacePublicKey,
    pub signature: PlaceSignature,
    pub ephemeral: PlaceEphemeral,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAuthRequest {
    pub id: ReqId,
    pub public_key: PlacePublicKey,
    pub name: String,
    pub signature: PlaceSignature,
}

/// A member of a union tagged on `op`: the union writes and reads the tag, so the struct holds none.
impl DeviceAuthRequest {
    pub const OP: &str = "device.auth";
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceDoorView {
    pub port: JsInt,
    pub addresses: Vec<String>,
    pub host_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub relay: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRoad {
    pub url: String,
    pub line: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinMint {
    pub joins: Vec<JoinRoad>,
    pub expires_at: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SshHostSuggestionFrom {
    #[serde(rename = "config")]
    Config,
    #[serde(rename = "known_hosts")]
    KnownHosts,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHostSuggestion {
    pub alias: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub host_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub port: Option<JsInt>,
    pub from: SshHostSuggestionFrom,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoPartRefusal {
    pub what: String,
    pub fix: String,
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `name`: string min 1
/// - `name`: string max 200
/// - `arch`: string max 32
/// - `os`: string max 200
/// - `diskFreeBytes`: int min 0
/// - `diskSizeBytes`: int min 0
/// - `model`: string max 200
/// - `uptimeMs`: int min 0
/// - `daemonVersion`: int min 0
/// - `daemonPort`: int min 1
/// - `daemonPort`: int max 65535
/// - `wsp`: array min 1
/// - `agents[]`: string max 32
/// - `agents`: array max 32
/// - `agentVersions`: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
/// - `agentVersions{}`: string max 64
/// - `logins[]`: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
/// - `logins[]`: string max 200
/// - `logins`: array max 64
/// - `daemonUnit`: string max 256
/// - `daemonUnit`: string regex /^[\w@.:-]+\.service$/
/// - `dialed`: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
impl Check for PlaceReport {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("platform") { None => out.push(crate::types::check::missing::<Self>("platform")), Some(v) => { match crate::types::check::read::<PlaceReportPlatform>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("platform"))), } } }
        match obj.get("arch") { None => out.push(crate::types::check::missing::<Self>("arch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("arch"))), } } }
        match obj.get("os") { None => out.push(crate::types::check::missing::<Self>("os")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("os"))), } } }
        match obj.get("shape") { None => out.push(crate::types::check::missing::<Self>("shape")), Some(v) => { match crate::types::check::read::<WorkspaceSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("shape"))), } } }
        if let Some(v) = obj.get("diskFreeBytes") { match crate::types::check::read_present::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskFreeBytes"))), } }
        if let Some(v) = obj.get("diskSizeBytes") { match crate::types::check::read_present::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("diskSizeBytes"))), } }
        if let Some(v) = obj.get("model") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } }
        match obj.get("login") { None => out.push(crate::types::check::missing::<Self>("login")), Some(v) => { match crate::types::check::read::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("login"))), } } }
        match obj.get("runsWorkspaces") { None => out.push(crate::types::check::missing::<Self>("runsWorkspaces")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("runsWorkspaces"))), } } }
        if let Some(v) = obj.get("workspacesBlocked") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspacesBlocked"))), } }
        match obj.get("engine") { None => out.push(crate::types::check::missing::<Self>("engine")), Some(v) => { match crate::types::check::read::<PlaceReportEngine>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("engine"))), } } }
        if let Some(v) = obj.get("copies") { match crate::types::check::read_present::<CopyWord>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("copies"))), } }
        if let Some(v) = obj.get("uptimeMs") { match crate::types::check::read_present::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("uptimeMs"))), } }
        match obj.get("daemonVersion") { None => out.push(crate::types::check::missing::<Self>("daemonVersion")), Some(v) => { match crate::types::check::read::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonVersion"))), } } }
        if let Some(v) = obj.get("daemonPort") { match crate::types::check::read_present::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonPort"))), } }
        match obj.get("wsp") { None => out.push(crate::types::check::missing::<Self>("wsp")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wsp"))), } } }
        match obj.get("agents") { None => out.push(crate::types::check::missing::<Self>("agents")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } } }
        if let Some(v) = obj.get("agentVersions") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agentVersions"))), } }
        if let Some(v) = obj.get("logins") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("logins"))), } }
        if let Some(v) = obj.get("wspDoor") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wspDoor"))), } }
        if let Some(v) = obj.get("wspDoorBlocked") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wspDoorBlocked"))), } }
        if let Some(v) = obj.get("daemonUnit") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("daemonUnit"))), } }
        if let Some(v) = obj.get("takesRuntime") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("takesRuntime"))), } }
        if let Some(v) = obj.get("wspDaemonVersion") { match crate::types::check::read_present::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("wspDaemonVersion"))), } }
        match obj.get("dialed") { None => out.push(crate::types::check::missing::<Self>("dialed")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dialed"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "place.join"
impl Check for PlaceJoinRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("publicKey") { None => out.push(crate::types::check::missing::<Self>("publicKey")), Some(v) => { match crate::types::check::read::<PlacePublicKey>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("publicKey"))), } } }
        match obj.get("nonce") { None => out.push(crate::types::check::missing::<Self>("nonce")), Some(v) => { match crate::types::check::read::<PlaceNonce>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("nonce"))), } } }
        if let Some(v) = obj.get("ephemeral") { match crate::types::check::read_present::<PlaceEphemeral>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ephemeral"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `hostName`: string min 1
/// - `hostName`: string max 200
impl Check for PlaceJoinReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("placeId") { None => out.push(crate::types::check::missing::<Self>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
        match obj.get("hostPublicKey") { None => out.push(crate::types::check::missing::<Self>("hostPublicKey")), Some(v) => { match crate::types::check::read::<PlacePublicKey>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostPublicKey"))), } } }
        match obj.get("nonce") { None => out.push(crate::types::check::missing::<Self>("nonce")), Some(v) => { match crate::types::check::read::<PlaceNonce>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("nonce"))), } } }
        match obj.get("signature") { None => out.push(crate::types::check::missing::<Self>("signature")), Some(v) => { match crate::types::check::read::<PlaceSignature>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signature"))), } } }
        match obj.get("ephemeral") { None => out.push(crate::types::check::missing::<Self>("ephemeral")), Some(v) => { match crate::types::check::read::<PlaceEphemeral>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ephemeral"))), } } }
        match obj.get("hostName") { None => out.push(crate::types::check::missing::<Self>("hostName")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostName"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `deviceToken`: string min 1
impl Check for PlaceJoinDevice {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("deviceId") { None => out.push(crate::types::check::missing::<Self>("deviceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("deviceId"))), } } }
        match obj.get("deviceToken") { None => out.push(crate::types::check::missing::<Self>("deviceToken")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("deviceToken"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "place.auth"
/// - `placeId`: string max 64
impl Check for PlaceAuthRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("placeId") { None => out.push(crate::types::check::missing::<Self>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
        match obj.get("nonce") { None => out.push(crate::types::check::missing::<Self>("nonce")), Some(v) => { match crate::types::check::read::<PlaceNonce>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("nonce"))), } } }
        if let Some(v) = obj.get("ephemeral") { match crate::types::check::read_present::<PlaceEphemeral>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ephemeral"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceAuthReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("nonce") { None => out.push(crate::types::check::missing::<Self>("nonce")), Some(v) => { match crate::types::check::read::<PlaceNonce>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("nonce"))), } } }
        match obj.get("hostPublicKey") { None => out.push(crate::types::check::missing::<Self>("hostPublicKey")), Some(v) => { match crate::types::check::read::<PlacePublicKey>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostPublicKey"))), } } }
        match obj.get("signature") { None => out.push(crate::types::check::missing::<Self>("signature")), Some(v) => { match crate::types::check::read::<PlaceSignature>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signature"))), } } }
        match obj.get("ephemeral") { None => out.push(crate::types::check::missing::<Self>("ephemeral")), Some(v) => { match crate::types::check::read::<PlaceEphemeral>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ephemeral"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceAuthRefusal {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("hostPublicKey") { None => out.push(crate::types::check::missing::<Self>("hostPublicKey")), Some(v) => { match crate::types::check::read::<PlacePublicKey>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostPublicKey"))), } } }
        match obj.get("signature") { None => out.push(crate::types::check::missing::<Self>("signature")), Some(v) => { match crate::types::check::read::<PlaceSignature>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signature"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "place.prove"
/// - `code`: string max 64
impl Check for PlaceProveRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("signature") { None => out.push(crate::types::check::missing::<Self>("signature")), Some(v) => { match crate::types::check::read::<PlaceSignature>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signature"))), } } }
        match obj.get("report") { None => out.push(crate::types::check::missing::<Self>("report")), Some(v) => { match crate::types::check::read::<PlaceReport>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("report"))), } } }
        if let Some(v) = obj.get("code") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("code"))), } }
        if let Some(v) = obj.get("client") { match crate::types::check::read_present::<PlaceProveRequestClient>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("client"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `name`: string min 1
/// - `name`: string max 200
impl Check for PlaceProveRequestClient {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "seal.open"
impl Check for SealOpenRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("nonce") { None => out.push(crate::types::check::missing::<Self>("nonce")), Some(v) => { match crate::types::check::read::<PlaceNonce>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("nonce"))), } } }
        match obj.get("ephemeral") { None => out.push(crate::types::check::missing::<Self>("ephemeral")), Some(v) => { match crate::types::check::read::<PlaceEphemeral>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ephemeral"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SealOpenReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("nonce") { None => out.push(crate::types::check::missing::<Self>("nonce")), Some(v) => { match crate::types::check::read::<PlaceNonce>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("nonce"))), } } }
        match obj.get("hostPublicKey") { None => out.push(crate::types::check::missing::<Self>("hostPublicKey")), Some(v) => { match crate::types::check::read::<PlacePublicKey>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostPublicKey"))), } } }
        match obj.get("signature") { None => out.push(crate::types::check::missing::<Self>("signature")), Some(v) => { match crate::types::check::read::<PlaceSignature>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signature"))), } } }
        match obj.get("ephemeral") { None => out.push(crate::types::check::missing::<Self>("ephemeral")), Some(v) => { match crate::types::check::read::<PlaceEphemeral>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ephemeral"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "device.auth"
/// - `name`: string min 1
/// - `name`: string max 200
impl Check for DeviceAuthRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("publicKey") { None => out.push(crate::types::check::missing::<Self>("publicKey")), Some(v) => { match crate::types::check::read::<PlacePublicKey>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("publicKey"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("signature") { None => out.push(crate::types::check::missing::<Self>("signature")), Some(v) => { match crate::types::check::read::<PlaceSignature>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signature"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `port`: int min 1
/// - `port`: int max 65535
/// - `addresses[]`: string url
/// - `addresses`: array min 1
/// - `hostKey`: string min 1
/// - `hostKey`: string max 200
/// - `relay`: string url
impl Check for PlaceDoorView {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("port") { None => out.push(crate::types::check::missing::<Self>("port")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } } }
        match obj.get("addresses") { None => out.push(crate::types::check::missing::<Self>("addresses")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("addresses"))), } } }
        match obj.get("hostKey") { None => out.push(crate::types::check::missing::<Self>("hostKey")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostKey"))), } } }
        if let Some(v) = obj.get("relay") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("relay"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `url`: string url
/// - `line`: string min 1
impl Check for JoinRoad {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("url") { None => out.push(crate::types::check::missing::<Self>("url")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } } }
        match obj.get("line") { None => out.push(crate::types::check::missing::<Self>("line")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("line"))), } } }
        if let Some(v) = obj.get("note") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("note"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `joins`: array min 1
/// - `expiresAt`: string datetime
impl Check for JoinMint {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("joins") { None => out.push(crate::types::check::missing::<Self>("joins")), Some(v) => { match crate::types::check::read::<Vec<JoinRoad>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("joins"))), } } }
        match obj.get("expiresAt") { None => out.push(crate::types::check::missing::<Self>("expiresAt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("expiresAt"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `alias`: string min 1
/// - `alias`: string max 300
/// - `hostName`: string max 300
/// - `user`: string max 300
/// - `port`: int min 1
/// - `port`: int max 65535
impl Check for SshHostSuggestion {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("alias") { None => out.push(crate::types::check::missing::<Self>("alias")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("alias"))), } } }
        if let Some(v) = obj.get("hostName") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostName"))), } }
        if let Some(v) = obj.get("user") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("user"))), } }
        if let Some(v) = obj.get("port") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } }
        match obj.get("from") { None => out.push(crate::types::check::missing::<Self>("from")), Some(v) => { match crate::types::check::read::<SshHostSuggestionFrom>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for TwoPartRefusal {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("what") { None => out.push(crate::types::check::missing::<Self>("what")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("what"))), } } }
        match obj.get("fix") { None => out.push(crate::types::check::missing::<Self>("fix")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fix"))), } } }
        Some(out)
    }

}

// place-link.ts:30
// export const engineWord = (hasDocker: boolean, hasPodman: boolean): PlaceEngine => (hasDocker ? "docker" : hasPodman ? "podman" : "none");
pub fn engine_word(has_docker: bool, has_podman: bool) -> PlaceEngine {
    todo!()
}
// place-link.ts:213
// /** The refusal a line gets for aiming at a host it holds no key for: a record somebody edited by hand, or a turn
//  * launched by a host older than this one. `where` names which. */
// export const hostNoKeyLine = (where: string): string =>
//   `${where} names a host and no key for it, so this computer cannot tell which host it would be sending its token to; run wsp hosts to read the account's hosts again`;
pub fn host_no_key_line(r#where: &str) -> String {
    todo!()
}
// place-link.ts:227
// /** The refusal for a word two computers on this host answer to: ids tell them apart, and the person picks one.
//  * `typed` is the word that was written, since more than one word names a computer and each says its own back. A
//  * relink cannot take another computer's name, so two by one name are two a person joined under one word, and
//  * every road that resolves a word reads this one sentence. */
// export const twoPlacesRefusal = (typed: string, ids: readonly string[]): string =>
//   `${typed}: this host holds ${ids.length} places by that name; name one by its id (${ids.join(", ")}).`;
pub fn two_places_refusal(typed: &str, ids: &[String]) -> String {
    todo!()
}
// place-link.ts:247
// /** The refusal a joining computer prints when the host at that address could not prove the key this computer
//  * learned at join, so nothing of this computer's went to it. */
// export const hostKeyRefusal = (url: string): string => `the host at ${url} did not prove the key this computer learned at join; nothing was sent to it`;
pub fn host_key_refusal(url: &str) -> String {
    todo!()
}
// place-link.ts:260
// /** What a remove took off that computer by its agent's own command: a plugin the setup put on there. */
// export const pluginOffLine = (name: string): string => `plugin ${name}`;
pub fn plugin_off_line(name: &str) -> String {
    todo!()
}
// place-link.ts:263
// /** What a remove says of the plugins the setup put on that computer and could not take off: they stay there. */
// export const pluginsKeptLine = (name: string, plugins: readonly string[]): string =>
//   `${plugins.join(", ")} ${plugins.length === 1 ? "is" : "are"} still on ${name}: wsp could not take ${plugins.length === 1 ? "it" : "them"} off`;
pub fn plugins_kept_line(name: &str, plugins: &[String]) -> String {
    todo!()
}
// place-link.ts:267
// /** What a place that is connected but has never said which port its daemon bound is refused with: a pane needs
//  * that port to carry to, and only that computer knows it. */
// export const placeNoDaemonPortLine = (name: string): string => `${name} is connected but has not said which port its daemon is on, so nothing can carry a pane to it yet; it says so on its next link`;
pub fn place_no_daemon_port_line(name: &str) -> String {
    todo!()
}
// place-link.ts:271
// /** What an install is refused with when the computer took the daemon and never dialled back: the join landed, so
//  * the computer belongs to this wsp, and what is missing is a road from it to here. */
// export const placeNoLinkLine = (name: string): string => `${name} took the daemon and has not dialled this host yet; check that it can reach this computer on the address it was given, and wsp computers shows it the moment it does`;
pub fn place_no_link_line(name: &str) -> String {
    todo!()
}
// place-link.ts:275
// /** What a stage reads while the computer it is running on has no link: the requests behind it are held until that
//  * computer opens a socket again, and a stage with no line of its own reads as one that stopped. */
// export const placeDialBackLine = (name: string): string => `waiting for ${name} to dial back`;
pub fn place_dial_back_line(name: &str) -> String {
    todo!()
}
// place-link.ts:279
// /** A Browser pane asking for a port of a computer the person joined that this computer would need root to open. */
// export const paneForwardFloorLine = (port: number): string => `localhost:${port} is below 1024, which this computer opens only as root; serve it on a port from 1024 up`;
pub fn pane_forward_floor_line(port: i64) -> String {
    todo!()
}
// place-link.ts:282
// /** A Browser pane asking for one port more than a workspace may hold open on this computer. */
// export const paneForwardCapLine = (name: string, cap: number): string => `${name} already has ${cap} ports open on this computer; stop one in Ports first`;
pub fn pane_forward_cap_line(name: &str, cap: i64) -> String {
    todo!()
}
// place-link.ts:285
// /** A Browser pane asking again for a port whose forward a quiet hour ended. */
// export const paneForwardQuietLine = (port: number): string => `localhost:${port} closed after an hour with nothing connecting to it; open the address again to forward it`;
pub fn pane_forward_quiet_line(port: i64) -> String {
    todo!()
}
// place-link.ts:288
// /** A Browser pane asking again for a port whose forward the person stopped in Ports. */
// export const paneForwardStoppedLine = (port: number): string => `localhost:${port} was stopped in Ports; open the address again to forward it`;
pub fn pane_forward_stopped_line(port: i64) -> String {
    todo!()
}
// place-link.ts:291
// /** A pane's fetch of a port whose forward does not stand: the fetch never opens one, the pane's own ask does. */
// export const paneForwardGoneLine = (port: number): string => `localhost:${port} is not forwarded to this computer now`;
pub fn pane_forward_gone_line(port: i64) -> String {
    todo!()
}
// place-link.ts:294
// /** Why a build on a joined computer stopped when that computer's link went and it never dialled back in time. */
// export const placeWentAwayLine = (name: string): string => `${name} went away before the build finished`;
pub fn place_went_away_line(name: &str) -> String {
    todo!()
}
// place-link.ts:353
// /** A url's host as a name: lowercase, without its port or a trailing root dot. */
// export const bareHost = (host: string): string => host.replace(/:\d*$/, "").toLowerCase().replace(/\.$/, "");
pub fn bare_host(host: &str) -> String {
    todo!()
}
// place-link.ts:356
// /** Whether a host is the domain or a host under it, in any case, with a port or a root dot or neither. */
// export function hostUnder(host: string, domain: string): boolean {
//   const name = bareHost(host);
//   return name === domain || name.endsWith(`.${domain}`);
// }
pub fn host_under(host: &str, domain: &str) -> bool {
    todo!()
}
// place-link.ts:362
// /** Whether a host name is one of GIT_FORGE_HOSTS or under one. */
// export function isGitForge(host: string): boolean {
//   return GIT_FORGE_HOSTS.some(forge => hostUnder(host, forge));
// }
pub fn is_git_forge(host: &str) -> bool {
    todo!()
}
// place-link.ts:396
// /** What the sheet says when somebody else already holds the door's port: a fixed port, since the place file on the
//  * other computer names it for good, so a fallback port would be a computer that can never dial back. */
// export const doorPortHeldLine = (port: number): string =>
//   `port ${port} is held by another program on this computer, so no computer you own can reach this wsp; free it and open Add a computer again`;
pub fn door_port_held_line(port: i64) -> String {
    todo!()
}
// place-link.ts:401
// /** Said when the door port of a host's last start is taken and it opens on another: a computer joined at the old one
//  * dials it for good. */
// export const doorPortMovedLine = (port: number): string =>
//   `port ${port}, where computers you joined last reached this wsp, is held by another program, so the door opened on another; free it and start this wsp again`;
pub fn door_port_moved_line(port: i64) -> String {
    todo!()
}
// place-link.ts:426
// /** The place file a text holds, or nothing when that text is not one. A file that is there and is not one reads the
//  * same as none: the one road that writes it is wsp join, and anything else there is not a place to dial with. */
// export function parsePlaceFile(text: string): PlaceFile | undefined {
//   let held: unknown;
//   try {
//     held = JSON.parse(text);
//   } catch {
//     return undefined;
//   }
//   const f = held as PlaceFile | undefined;
//   const ok =
//     typeof f === "object" &&
//     f !== null &&
//     typeof f.placeId === "string" &&
//     typeof f.name === "string" &&
//     typeof f.hostName === "string" &&
//     Array.isArray(f.hostUrls) &&
//     f.hostUrls.every(u => typeof u === "string") &&
//     typeof f.hostPublicKey === "string" &&
//     typeof f.keyPath === "string";
//   return ok ? f : undefined;
// }
pub fn parse_place_file(text: &str) -> Option<PlaceFile> {
    todo!()
}
// place-link.ts:457
// /** The place file the held read found, read past what the login printed before it. */
// export const heldPlaceFile = (stdout: string): PlaceFile | undefined => parsePlaceFile(heldText(stdout) ?? "");
pub fn held_place_file(stdout: &str) -> Option<PlaceFile> {
    todo!()
}
// place-link.ts:460
// /** What the held read of a box says stands there, by the rule the join on that box refuses by: a place file that
//  * parses is a join; anything else it answered after its own line is what a join left broken, named by the path
//  * that stands. */
// export function readHeldPlace(stdout: string, home: string): { joined: PlaceFile } | { broken: string } | undefined {
//   const text = heldText(stdout);
//   if (text === undefined || text === "") return undefined;
//   const joined = parsePlaceFile(text);
//   if (joined !== undefined) return { joined };
//   const at = placeDaemonPaths(home);
//   return { broken: text.trim() === `${PLACE_LEFT_LINE} key` ? at.placeKey : at.placeFile };
// }
pub fn read_held_place(stdout: &str, home: &str) -> Option<HeldPlace> {
    todo!()
}
// place-link.ts:472
// /** Whether a place file names one of these computers and this host's key: the one test every road over a computer's
//  * ssh login holds the machine it reached to before it changes anything there. */
// export const placeFileNames = (file: PlaceFile | undefined, placeIds: readonly string[], hostPublicKey: string): file is PlaceFile =>
//   file !== undefined && placeIds.includes(file.placeId) && file.hostPublicKey === hostPublicKey;
pub fn place_file_names(file: Option<&PlaceFile>, place_ids: &[String], host_public_key: &str) -> bool {
    todo!()
}
// place-link.ts:477
// /** The text the file holds, which parsePlaceFile reads back. */
// export const placeFileText = (file: PlaceFile): string => `${JSON.stringify(file, null, 2)}\n`;
pub fn place_file_text(file: &PlaceFile) -> String {
    todo!()
}

// The shapes wsp-frames holds read every bound while deserializing, so each Check has nothing left to refuse.
impl Check for PlaceFile {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("placeId") { None => out.push(crate::types::check::missing::<Self>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
        match obj.get("name") { None => out.push(crate::types::check::missing::<Self>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
        match obj.get("hostName") { None => out.push(crate::types::check::missing::<Self>("hostName")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostName"))), } } }
        match obj.get("hostUrls") { None => out.push(crate::types::check::missing::<Self>("hostUrls")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostUrls"))), } } }
        match obj.get("hostPublicKey") { None => out.push(crate::types::check::missing::<Self>("hostPublicKey")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostPublicKey"))), } } }
        match obj.get("keyPath") { None => out.push(crate::types::check::missing::<Self>("keyPath")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("keyPath"))), } } }
        if let Some(v) = obj.get("joinedAt") { match crate::types::check::read_present::<Value>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("joinedAt"))), } }
        match obj.get("rest") { None => out.push(crate::types::check::missing::<Self>("rest")), Some(v) => { match crate::types::check::read::<IndexMap<String, Value>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rest"))), } } }
        Some(out)
    }

}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for PlaceEngine {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for PlaceReportPlatform {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for PlaceReportEngine {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SshHostSuggestionFrom {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// The place link's base64 values are wsp_frames' and read every bound at deserialize; nothing is left to check.
impl<const N: usize> Check for wsp_frames::Base64Bytes<N> {}
