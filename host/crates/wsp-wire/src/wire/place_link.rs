// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/place-link.ts.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::{js_key_order, ReqId};
use crate::types::check::{num_at_least, num_at_most, refine, regex_check, text_at_least, text_at_most};
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
/// zod's `.url()`: the WHATWG parser takes the string.
fn url_string(s: &str) -> Result<(), SchemaError> {
    if url::Url::parse(s).is_ok() {
        return Ok(());
    }
    let mut detail = indexmap::IndexMap::new();
    detail.insert("validation".to_string(), Value::from("url"));
    Err(SchemaError { code: "invalid_string".into(), path: Vec::new(), message: "Invalid url".into(), detail })
}

/// zod's `.datetime()` with no offset allowed: `YYYY-MM-DDTHH:MM:SS`, an optional fraction, then `Z`.
fn datetime_string(s: &str) -> Result<(), SchemaError> {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| b.get(r).is_some_and(|d| d.iter().all(u8::is_ascii_digit));
    let shape = digits(0..4)
        && b.get(4) == Some(&b'-')
        && digits(5..7)
        && b.get(7) == Some(&b'-')
        && digits(8..10)
        && b.get(10) == Some(&b'T')
        && digits(11..13)
        && b.get(13) == Some(&b':')
        && digits(14..16)
        && b.get(16) == Some(&b':')
        && digits(17..19);
    let fraction_ok = s
        .get(19..)
        .and_then(|t| t.strip_suffix('Z'))
        .is_some_and(|f| f.is_empty() || f.strip_prefix('.').is_some_and(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit())));
    if shape && fraction_ok {
        return Ok(());
    }
    let mut detail = indexmap::IndexMap::new();
    detail.insert("validation".to_string(), Value::from("datetime"));
    Err(SchemaError { code: "invalid_string".into(), path: Vec::new(), message: "Invalid datetime".into(), detail })
}

/// The daemon unit name zod's regex takes: word characters, `@.:-`, then `.service`.
fn is_daemon_unit(name: &str) -> bool {
    name.strip_suffix(".service")
        .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_alphanumeric() || b"_@.:-".contains(&b)))
}

/// The text after the first line that is exactly WSP_PLACE_HELD, past what a login printed before it.
fn held_text(stdout: &str) -> Option<&str> {
    let mark = format!("{}\n", crate::types::project_path::PLACE_HELD_LINE);
    let at = format!("\n{stdout}").find(&format!("\n{mark}"))?;
    Some(&stdout[at + mark.len()..])
}

pub fn join_address_line() -> TwoPartRefusal {
    TwoPartRefusal {
        what: "That is not an address.".into(),
        fix: format!("Type it as the other screen shows it, like 192.168.1.20:{}.", crate::types::app_ports::DEFAULT_PLACE_PORT),
    }
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
    pub disk_free_bytes: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_size_bytes: Option<JsInt>,
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
    pub uptime_ms: Option<JsInt>,
    pub daemon_version: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_port: Option<JsInt>,
    pub wsp: Vec<String>,
    pub agents: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::check::present",
        serialize_with = "crate::types::check::js_record_opt"
    )]
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
    pub wsp_daemon_version: Option<JsInt>,
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
        text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        text_at_most(&self.name, 200).map_err(|e| e.under("name"))?;
        text_at_most(&self.arch, 32).map_err(|e| e.under("arch"))?;
        text_at_most(&self.os, 200).map_err(|e| e.under("os"))?;
        self.shape.check().map_err(|e| e.under("shape"))?;
        if let Some(v) = self.disk_free_bytes {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("diskFreeBytes"))?;
        }
        if let Some(v) = self.disk_size_bytes {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("diskSizeBytes"))?;
        }
        if let Some(model) = &self.model {
            text_at_most(model, 200).map_err(|e| e.under("model"))?;
        }
        self.copies.check().map_err(|e| e.under("copies"))?;
        if let Some(v) = self.uptime_ms {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("uptimeMs"))?;
        }
        num_at_least(self.daemon_version.as_f64(), 0.0).map_err(|e| e.under("daemonVersion"))?;
        if let Some(v) = self.daemon_port {
            num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("daemonPort"))?;
            num_at_most(v.as_f64(), 65535.0).map_err(|e| e.under("daemonPort"))?;
        }
        crate::types::check::items_at_least(self.wsp.len(), 1).map_err(|e| e.under("wsp"))?;
        crate::types::check::items_at_most(self.agents.len(), AGENTS_REPORTED_MAX).map_err(|e| e.under("agents"))?;
        for (i, agent) in self.agents.iter().enumerate() {
            text_at_most(agent, 32).map_err(|e| e.under(i).under("agents"))?;
        }
        if let Some(said) = &self.agent_versions {
            for (agent, version) in said {
                text_at_most(version, 64).map_err(|e| e.under(agent.as_str()).under("agentVersions"))?;
            }
            refine(said.len() <= AGENTS_REPORTED_MAX, &format!("at most {AGENTS_REPORTED_MAX} agents"))
                .map_err(|e| e.under("agentVersions"))?;
        }
        if let Some(logins) = &self.logins {
            crate::types::check::items_at_most(logins.len(), 64).map_err(|e| e.under("logins"))?;
            for (i, name) in logins.iter().enumerate() {
                text_at_most(name, 200).map_err(|e| e.under(i).under("logins"))?;
                refine(crate::wire::limits::is_under_path(name), "a name under a folder").map_err(|e| e.under(i).under("logins"))?;
            }
        }
        if let Some(unit) = &self.daemon_unit {
            text_at_most(unit, 256).map_err(|e| e.under("daemonUnit"))?;
            regex_check(is_daemon_unit(unit)).map_err(|e| e.under("daemonUnit"))?;
        }
        refine(crate::wire::limits::is_http_url(&self.dialed), "http or https URL").map_err(|e| e.under("dialed"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "name", &mut out, Self::rule_name);
        crate::types::keys::req_key::<PlaceReportPlatform>(obj, "platform", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "arch", &mut out, Self::rule_arch);
        crate::types::keys::req_key_with::<String>(obj, "os", &mut out, Self::rule_os);
        crate::types::keys::req_key::<WorkspaceSize>(obj, "shape", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "diskFreeBytes", &mut out, Self::rule_disk_free_bytes);
        crate::types::keys::opt_key_with::<JsInt>(obj, "diskSizeBytes", &mut out, Self::rule_disk_size_bytes);
        crate::types::keys::opt_key_with::<String>(obj, "model", &mut out, Self::rule_model);
        crate::types::keys::req_key::<IndexMap<String, String>>(obj, "login", &mut out);
        crate::types::keys::req_key::<bool>(obj, "runsWorkspaces", &mut out);
        crate::types::keys::opt_key::<String>(obj, "workspacesBlocked", &mut out);
        crate::types::keys::req_key::<PlaceReportEngine>(obj, "engine", &mut out);
        crate::types::keys::opt_key::<CopyWord>(obj, "copies", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "uptimeMs", &mut out, Self::rule_uptime_ms);
        crate::types::keys::req_key_with::<JsInt>(obj, "daemonVersion", &mut out, Self::rule_daemon_version);
        crate::types::keys::opt_key_with::<JsInt>(obj, "daemonPort", &mut out, Self::rule_daemon_port);
        crate::types::keys::req_key_with::<Vec<String>>(obj, "wsp", &mut out, Self::rule_wsp);
        crate::types::keys::req_key_with::<Vec<String>>(obj, "agents", &mut out, Self::rule_agents);
        crate::types::keys::opt_key_with::<IndexMap<String, String>>(obj, "agentVersions", &mut out, Self::rule_agent_versions);
        crate::types::keys::opt_key_with::<Vec<String>>(obj, "logins", &mut out, Self::rule_logins);
        crate::types::keys::opt_key::<bool>(obj, "wspDoor", &mut out);
        crate::types::keys::opt_key::<String>(obj, "wspDoorBlocked", &mut out);
        crate::types::keys::opt_key_with::<String>(obj, "daemonUnit", &mut out, Self::rule_daemon_unit);
        crate::types::keys::opt_key::<bool>(obj, "takesRuntime", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "wspDaemonVersion", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "dialed", &mut out, Self::rule_dialed);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "platform" => <PlaceReportPlatform as Check>::describe(rest),
                "arch" => <String as Check>::describe(rest),
                "os" => <String as Check>::describe(rest),
                "shape" => <WorkspaceSize as Check>::describe(rest),
                "diskFreeBytes" => <Option<i64> as Check>::describe(rest),
                "diskSizeBytes" => <Option<i64> as Check>::describe(rest),
                "model" => <Option<String> as Check>::describe(rest),
                "login" => <IndexMap<String, String> as Check>::describe(rest),
                "runsWorkspaces" => <bool as Check>::describe(rest),
                "workspacesBlocked" => <Option<String> as Check>::describe(rest),
                "engine" => <PlaceReportEngine as Check>::describe(rest),
                "copies" => <Option<CopyWord> as Check>::describe(rest),
                "uptimeMs" => <Option<i64> as Check>::describe(rest),
                "daemonVersion" => <i64 as Check>::describe(rest),
                "daemonPort" => <Option<i64> as Check>::describe(rest),
                "wsp" => <Vec<String> as Check>::describe(rest),
                "agents" => <Vec<String> as Check>::describe(rest),
                "agentVersions" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "logins" => <Option<Vec<String>> as Check>::describe(rest),
                "wspDoor" => <Option<bool> as Check>::describe(rest),
                "wspDoorBlocked" => <Option<String> as Check>::describe(rest),
                "daemonUnit" => <Option<String> as Check>::describe(rest),
                "takesRuntime" => <Option<bool> as Check>::describe(rest),
                "wspDaemonVersion" => <Option<i64> as Check>::describe(rest),
                "dialed" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceReport {
    fn rule_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 200)?;
        Ok(())
    }
    fn rule_arch(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_most(x, 32)?;
        Ok(())
    }
    fn rule_os(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_most(x, 200)?;
        Ok(())
    }
    fn rule_disk_free_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_disk_size_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_model(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(model) = x {
            text_at_most(model, 200)?;
        }
        Ok(())
    }
    fn rule_uptime_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_daemon_version(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
    fn rule_daemon_port(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 1.0)?;
            num_at_most(v.as_f64(), 65535.0)?;
        }
        Ok(())
    }
    fn rule_wsp(x: &Vec<String>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_at_least((*x).len(), 1)?;
        Ok(())
    }
    fn rule_agents(x: &Vec<String>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_at_most((*x).len(), AGENTS_REPORTED_MAX)?;
        for (i, agent) in (*x).iter().enumerate() {
            text_at_most(agent, 32).map_err(|e| e.under(i))?;
        }
        Ok(())
    }
    fn rule_agent_versions(x: &Option<IndexMap<String, String>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(said) = x {
            for (agent, version) in said {
                text_at_most(version, 64).map_err(|e| e.under(agent.as_str()))?;
            }
            refine(said.len() <= AGENTS_REPORTED_MAX, &format!("at most {AGENTS_REPORTED_MAX} agents"))?;
        }
        Ok(())
    }
    fn rule_logins(x: &Option<Vec<String>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(logins) = x {
            crate::types::check::items_at_most(logins.len(), 64)?;
            for (i, name) in logins.iter().enumerate() {
                text_at_most(name, 200).map_err(|e| e.under(i))?;
                refine(crate::wire::limits::is_under_path(name), "a name under a folder").map_err(|e| e.under(i))?;
            }
        }
        Ok(())
    }
    fn rule_daemon_unit(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(unit) = x {
            text_at_most(unit, 256)?;
            regex_check(is_daemon_unit(unit))?;
        }
        Ok(())
    }
    fn rule_dialed(x: &String) -> Result<(), crate::types::check::SchemaError> {
        refine(crate::wire::limits::is_http_url(x), "http or https URL")?;
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "place.join"
impl Check for PlaceJoinRequest {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
        crate::types::keys::req_key::<PlacePublicKey>(obj, "publicKey", &mut out);
        crate::types::keys::req_key::<PlaceNonce>(obj, "nonce", &mut out);
        crate::types::keys::opt_key::<PlaceEphemeral>(obj, "ephemeral", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "publicKey" => <String as Check>::describe(rest),
                "nonce" => <String as Check>::describe(rest),
                "ephemeral" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `hostName`: string min 1
/// - `hostName`: string max 200
impl Check for PlaceJoinReply {
    fn check(&self) -> Result<(), SchemaError> {
        text_at_least(&self.host_name, 1).map_err(|e| e.under("hostName"))?;
        text_at_most(&self.host_name, 200).map_err(|e| e.under("hostName"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::req_key::<PlacePublicKey>(obj, "hostPublicKey", &mut out);
        crate::types::keys::req_key::<PlaceNonce>(obj, "nonce", &mut out);
        crate::types::keys::req_key::<PlaceSignature>(obj, "signature", &mut out);
        crate::types::keys::req_key::<PlaceEphemeral>(obj, "ephemeral", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "hostName", &mut out, Self::rule_host_name);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "hostPublicKey" => <String as Check>::describe(rest),
                "nonce" => <String as Check>::describe(rest),
                "signature" => <String as Check>::describe(rest),
                "ephemeral" => <String as Check>::describe(rest),
                "hostName" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceJoinReply {
    fn rule_host_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 200)?;
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `deviceToken`: string min 1
impl Check for PlaceJoinDevice {
    fn check(&self) -> Result<(), SchemaError> {
        text_at_least(&self.device_token, 1).map_err(|e| e.under("deviceToken"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "deviceId", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "deviceToken", &mut out, Self::rule_device_token);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "deviceId" => <String as Check>::describe(rest),
                "deviceToken" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceJoinDevice {
    fn rule_device_token(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "place.auth"
/// - `placeId`: string max 64
impl Check for PlaceAuthRequest {
    fn check(&self) -> Result<(), SchemaError> {
        text_at_most(&self.place_id, 64).map_err(|e| e.under("placeId"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "placeId", &mut out, Self::rule_place_id);
        crate::types::keys::req_key::<PlaceNonce>(obj, "nonce", &mut out);
        crate::types::keys::opt_key::<PlaceEphemeral>(obj, "ephemeral", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "placeId" => <String as Check>::describe(rest),
                "nonce" => <String as Check>::describe(rest),
                "ephemeral" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceAuthRequest {
    fn rule_place_id(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_most(x, 64)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceAuthReply {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceNonce>(obj, "nonce", &mut out);
        crate::types::keys::req_key::<PlacePublicKey>(obj, "hostPublicKey", &mut out);
        crate::types::keys::req_key::<PlaceSignature>(obj, "signature", &mut out);
        crate::types::keys::req_key::<PlaceEphemeral>(obj, "ephemeral", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "nonce" => <String as Check>::describe(rest),
                "hostPublicKey" => <String as Check>::describe(rest),
                "signature" => <String as Check>::describe(rest),
                "ephemeral" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlaceAuthRefusal {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlacePublicKey>(obj, "hostPublicKey", &mut out);
        crate::types::keys::req_key::<PlaceSignature>(obj, "signature", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "hostPublicKey" => <String as Check>::describe(rest),
                "signature" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "place.prove"
/// - `code`: string max 64
impl Check for PlaceProveRequest {
    fn check(&self) -> Result<(), SchemaError> {
        self.report.check().map_err(|e| e.under("report"))?;
        if let Some(code) = &self.code {
            text_at_most(code, 64).map_err(|e| e.under("code"))?;
        }
        self.client.check().map_err(|e| e.under("client"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
        crate::types::keys::req_key::<PlaceSignature>(obj, "signature", &mut out);
        crate::types::keys::req_key::<PlaceReport>(obj, "report", &mut out);
        crate::types::keys::opt_key_with::<String>(obj, "code", &mut out, Self::rule_code);
        crate::types::keys::opt_key::<PlaceProveRequestClient>(obj, "client", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "signature" => <String as Check>::describe(rest),
                "report" => <PlaceReport as Check>::describe(rest),
                "code" => <Option<String> as Check>::describe(rest),
                "client" => <Option<PlaceProveRequestClient> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceProveRequest {
    fn rule_code(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(code) = x {
            text_at_most(code, 64)?;
        }
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `name`: string min 1
/// - `name`: string max 200
impl Check for PlaceProveRequestClient {
    fn check(&self) -> Result<(), SchemaError> {
        text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        text_at_most(&self.name, 200).map_err(|e| e.under("name"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "name", &mut out, Self::rule_name);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceProveRequestClient {
    fn rule_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 200)?;
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "seal.open"
impl Check for SealOpenRequest {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
        crate::types::keys::req_key::<PlaceNonce>(obj, "nonce", &mut out);
        crate::types::keys::req_key::<PlaceEphemeral>(obj, "ephemeral", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "nonce" => <String as Check>::describe(rest),
                "ephemeral" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SealOpenReply {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceNonce>(obj, "nonce", &mut out);
        crate::types::keys::req_key::<PlacePublicKey>(obj, "hostPublicKey", &mut out);
        crate::types::keys::req_key::<PlaceSignature>(obj, "signature", &mut out);
        crate::types::keys::req_key::<PlaceEphemeral>(obj, "ephemeral", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "nonce" => <String as Check>::describe(rest),
                "hostPublicKey" => <String as Check>::describe(rest),
                "signature" => <String as Check>::describe(rest),
                "ephemeral" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "device.auth"
/// - `name`: string min 1
/// - `name`: string max 200
impl Check for DeviceAuthRequest {
    fn check(&self) -> Result<(), SchemaError> {
        text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        text_at_most(&self.name, 200).map_err(|e| e.under("name"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
        crate::types::keys::req_key::<PlacePublicKey>(obj, "publicKey", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "name", &mut out, Self::rule_name);
        crate::types::keys::req_key::<PlaceSignature>(obj, "signature", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "publicKey" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "signature" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl DeviceAuthRequest {
    fn rule_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 200)?;
        Ok(())
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
        num_at_least(self.port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
        num_at_most(self.port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
        crate::types::check::items_at_least(self.addresses.len(), 1).map_err(|e| e.under("addresses"))?;
        for (i, address) in self.addresses.iter().enumerate() {
            url_string(address).map_err(|e| e.under(i).under("addresses"))?;
        }
        text_at_least(&self.host_key, 1).map_err(|e| e.under("hostKey"))?;
        text_at_most(&self.host_key, 200).map_err(|e| e.under("hostKey"))?;
        if let Some(relay) = &self.relay {
            url_string(relay).map_err(|e| e.under("relay"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "port", &mut out, Self::rule_port);
        crate::types::keys::req_key_with::<Vec<String>>(obj, "addresses", &mut out, Self::rule_addresses);
        crate::types::keys::req_key_with::<String>(obj, "hostKey", &mut out, Self::rule_host_key);
        crate::types::keys::opt_key_with::<String>(obj, "relay", &mut out, Self::rule_relay);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "port" => <i64 as Check>::describe(rest),
                "addresses" => <Vec<String> as Check>::describe(rest),
                "hostKey" => <String as Check>::describe(rest),
                "relay" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceDoorView {
    fn rule_port(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 1.0)?;
        num_at_most((*x).as_f64(), 65535.0)?;
        Ok(())
    }
    fn rule_addresses(x: &Vec<String>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_at_least((*x).len(), 1)?;
        for (i, address) in (*x).iter().enumerate() {
            url_string(address).map_err(|e| e.under(i))?;
        }
        Ok(())
    }
    fn rule_host_key(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 200)?;
        Ok(())
    }
    fn rule_relay(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(relay) = x {
            url_string(relay)?;
        }
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `url`: string url
/// - `line`: string min 1
impl Check for JoinRoad {
    fn check(&self) -> Result<(), SchemaError> {
        url_string(&self.url).map_err(|e| e.under("url"))?;
        text_at_least(&self.line, 1).map_err(|e| e.under("line"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "url", &mut out, Self::rule_url);
        crate::types::keys::req_key_with::<String>(obj, "line", &mut out, Self::rule_line);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "url" => <String as Check>::describe(rest),
                "line" => <String as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl JoinRoad {
    fn rule_url(x: &String) -> Result<(), crate::types::check::SchemaError> {
        url_string(x)?;
        Ok(())
    }
    fn rule_line(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `joins`: array min 1
/// - `expiresAt`: string datetime
impl Check for JoinMint {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::items_at_least(self.joins.len(), 1).map_err(|e| e.under("joins"))?;
        self.joins.check().map_err(|e| e.under("joins"))?;
        datetime_string(&self.expires_at).map_err(|e| e.under("expiresAt"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<Vec<JoinRoad>>(obj, "joins", &mut out, Self::rule_joins);
        crate::types::keys::req_key_with::<String>(obj, "expiresAt", &mut out, Self::rule_expires_at);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "joins" => <Vec<JoinRoad> as Check>::describe(rest),
                "expiresAt" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl JoinMint {
    fn rule_joins(x: &Vec<JoinRoad>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_at_least((*x).len(), 1)?;
        Ok(())
    }
    fn rule_expires_at(x: &String) -> Result<(), crate::types::check::SchemaError> {
        datetime_string(x)?;
        Ok(())
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
        text_at_least(&self.alias, 1).map_err(|e| e.under("alias"))?;
        text_at_most(&self.alias, 300).map_err(|e| e.under("alias"))?;
        if let Some(host_name) = &self.host_name {
            text_at_most(host_name, 300).map_err(|e| e.under("hostName"))?;
        }
        if let Some(user) = &self.user {
            text_at_most(user, 300).map_err(|e| e.under("user"))?;
        }
        if let Some(port) = self.port {
            num_at_least(port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
            num_at_most(port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "alias", &mut out, Self::rule_alias);
        crate::types::keys::opt_key_with::<String>(obj, "hostName", &mut out, Self::rule_host_name);
        crate::types::keys::opt_key_with::<String>(obj, "user", &mut out, Self::rule_user);
        crate::types::keys::opt_key_with::<JsInt>(obj, "port", &mut out, Self::rule_port);
        crate::types::keys::req_key::<SshHostSuggestionFrom>(obj, "from", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "alias" => <String as Check>::describe(rest),
                "hostName" => <Option<String> as Check>::describe(rest),
                "user" => <Option<String> as Check>::describe(rest),
                "port" => <Option<i64> as Check>::describe(rest),
                "from" => <SshHostSuggestionFrom as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SshHostSuggestion {
    fn rule_alias(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 300)?;
        Ok(())
    }
    fn rule_host_name(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(host_name) = x {
            text_at_most(host_name, 300)?;
        }
        Ok(())
    }
    fn rule_user(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(user) = x {
            text_at_most(user, 300)?;
        }
        Ok(())
    }
    fn rule_port(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(port) = (*x) {
            num_at_least(port.as_f64(), 1.0)?;
            num_at_most(port.as_f64(), 65535.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for TwoPartRefusal {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "what", &mut out);
        crate::types::keys::req_key::<String>(obj, "fix", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "what" => <String as Check>::describe(rest),
                "fix" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// place-link.ts:30
// export const engineWord = (hasDocker: boolean, hasPodman: boolean): PlaceEngine => (hasDocker ? "docker" : hasPodman ? "podman" : "none");
pub fn engine_word(has_docker: bool, has_podman: bool) -> PlaceEngine {
    if has_docker {
        PlaceEngine::Docker
    } else if has_podman {
        PlaceEngine::Podman
    } else {
        PlaceEngine::None
    }
}
// place-link.ts:213
// /** The refusal a line gets for aiming at a host it holds no key for: a record somebody edited by hand, or a turn
//  * launched by a host older than this one. `where` names which. */
// export const hostNoKeyLine = (where: string): string =>
//   `${where} names a host and no key for it, so this computer cannot tell which host it would be sending its token to; run wsp hosts to read the account's hosts again`;
pub fn host_no_key_line(r#where: &str) -> String {
    format!("{where} names a host and no key for it, so this computer cannot tell which host it would be sending its token to; run wsp hosts to read the account's hosts again")
}
// place-link.ts:227
// /** The refusal for a word two computers on this host answer to: ids tell them apart, and the person picks one.
//  * `typed` is the word that was written, since more than one word names a computer and each says its own back. A
//  * relink cannot take another computer's name, so two by one name are two a person joined under one word, and
//  * every road that resolves a word reads this one sentence. */
// export const twoPlacesRefusal = (typed: string, ids: readonly string[]): string =>
//   `${typed}: this host holds ${ids.length} places by that name; name one by its id (${ids.join(", ")}).`;
pub fn two_places_refusal(typed: &str, ids: &[String]) -> String {
    format!("{typed}: this host holds {} places by that name; name one by its id ({}).", ids.len(), ids.join(", "))
}
// place-link.ts:247
// /** The refusal a joining computer prints when the host at that address could not prove the key this computer
//  * learned at join, so nothing of this computer's went to it. */
// export const hostKeyRefusal = (url: string): string => `the host at ${url} did not prove the key this computer learned at join; nothing was sent to it`;
pub fn host_key_refusal(url: &str) -> String {
    format!("the host at {url} did not prove the key this computer learned at join; nothing was sent to it")
}
// place-link.ts:260
// /** What a remove took off that computer by its agent's own command: a plugin the setup put on there. */
// export const pluginOffLine = (name: string): string => `plugin ${name}`;
pub fn plugin_off_line(name: &str) -> String {
    format!("plugin {name}")
}
// place-link.ts:263
// /** What a remove says of the plugins the setup put on that computer and could not take off: they stay there. */
// export const pluginsKeptLine = (name: string, plugins: readonly string[]): string =>
//   `${plugins.join(", ")} ${plugins.length === 1 ? "is" : "are"} still on ${name}: wsp could not take ${plugins.length === 1 ? "it" : "them"} off`;
pub fn plugins_kept_line(name: &str, plugins: &[String]) -> String {
    let (verb, them) = if plugins.len() == 1 { ("is", "it") } else { ("are", "them") };
    format!("{} {verb} still on {name}: wsp could not take {them} off", plugins.join(", "))
}
// place-link.ts:267
// /** What a place that is connected but has never said which port its daemon bound is refused with: a pane needs
//  * that port to carry to, and only that computer knows it. */
// export const placeNoDaemonPortLine = (name: string): string => `${name} is connected but has not said which port its daemon is on, so nothing can carry a pane to it yet; it says so on its next link`;
pub fn place_no_daemon_port_line(name: &str) -> String {
    format!("{name} is connected but has not said which port its daemon is on, so nothing can carry a pane to it yet; it says so on its next link")
}
// place-link.ts:271
// /** What an install is refused with when the computer took the daemon and never dialled back: the join landed, so
//  * the computer belongs to this wsp, and what is missing is a road from it to here. */
// export const placeNoLinkLine = (name: string): string => `${name} took the daemon and has not dialled this host yet; check that it can reach this computer on the address it was given, and wsp computers shows it the moment it does`;
pub fn place_no_link_line(name: &str) -> String {
    format!("{name} took the daemon and has not dialled this host yet; check that it can reach this computer on the address it was given, and wsp computers shows it the moment it does")
}
// place-link.ts:275
// /** What a stage reads while the computer it is running on has no link: the requests behind it are held until that
//  * computer opens a socket again, and a stage with no line of its own reads as one that stopped. */
// export const placeDialBackLine = (name: string): string => `waiting for ${name} to dial back`;
pub fn place_dial_back_line(name: &str) -> String {
    format!("waiting for {name} to dial back")
}
// place-link.ts:279
// /** A Browser pane asking for a port of a computer the person joined that this computer would need root to open. */
// export const paneForwardFloorLine = (port: number): string => `localhost:${port} is below 1024, which this computer opens only as root; serve it on a port from 1024 up`;
pub fn pane_forward_floor_line(port: i64) -> String {
    format!("localhost:{port} is below 1024, which this computer opens only as root; serve it on a port from 1024 up")
}
// place-link.ts:282
// /** A Browser pane asking for one port more than a workspace may hold open on this computer. */
// export const paneForwardCapLine = (name: string, cap: number): string => `${name} already has ${cap} ports open on this computer; stop one in Ports first`;
pub fn pane_forward_cap_line(name: &str, cap: i64) -> String {
    format!("{name} already has {cap} ports open on this computer; stop one in Ports first")
}
// place-link.ts:285
// /** A Browser pane asking again for a port whose forward a quiet hour ended. */
// export const paneForwardQuietLine = (port: number): string => `localhost:${port} closed after an hour with nothing connecting to it; open the address again to forward it`;
pub fn pane_forward_quiet_line(port: i64) -> String {
    format!("localhost:{port} closed after an hour with nothing connecting to it; open the address again to forward it")
}
// place-link.ts:288
// /** A Browser pane asking again for a port whose forward the person stopped in Ports. */
// export const paneForwardStoppedLine = (port: number): string => `localhost:${port} was stopped in Ports; open the address again to forward it`;
pub fn pane_forward_stopped_line(port: i64) -> String {
    format!("localhost:{port} was stopped in Ports; open the address again to forward it")
}
// place-link.ts:291
// /** A pane's fetch of a port whose forward does not stand: the fetch never opens one, the pane's own ask does. */
// export const paneForwardGoneLine = (port: number): string => `localhost:${port} is not forwarded to this computer now`;
pub fn pane_forward_gone_line(port: i64) -> String {
    format!("localhost:{port} is not forwarded to this computer now")
}
// place-link.ts:294
// /** Why a build on a joined computer stopped when that computer's link went and it never dialled back in time. */
// export const placeWentAwayLine = (name: string): string => `${name} went away before the build finished`;
pub fn place_went_away_line(name: &str) -> String {
    format!("{name} went away before the build finished")
}
// place-link.ts:353
// /** A url's host as a name: lowercase, without its port or a trailing root dot. */
// export const bareHost = (host: string): string => host.replace(/:\d*$/, "").toLowerCase().replace(/\.$/, "");
pub fn bare_host(host: &str) -> String {
    let without_port = match host.rfind(':') {
        Some(at) if host[at + 1..].bytes().all(|b| b.is_ascii_digit()) => &host[..at],
        _ => host,
    };
    let lower = without_port.to_lowercase();
    lower.strip_suffix('.').unwrap_or(&lower).to_string()
}
// place-link.ts:356
// /** Whether a host is the domain or a host under it, in any case, with a port or a root dot or neither. */
// export function hostUnder(host: string, domain: string): boolean {
//   const name = bareHost(host);
//   return name === domain || name.endsWith(`.${domain}`);
// }
pub fn host_under(host: &str, domain: &str) -> bool {
    let name = bare_host(host);
    name == domain || name.ends_with(&format!(".{domain}"))
}
// place-link.ts:362
// /** Whether a host name is one of GIT_FORGE_HOSTS or under one. */
// export function isGitForge(host: string): boolean {
//   return GIT_FORGE_HOSTS.some(forge => hostUnder(host, forge));
// }
pub fn is_git_forge(host: &str) -> bool {
    GIT_FORGE_HOSTS.iter().any(|forge| host_under(host, forge))
}
// place-link.ts:396
// /** What the sheet says when somebody else already holds the door's port: a fixed port, since the place file on the
//  * other computer names it for good, so a fallback port would be a computer that can never dial back. */
// export const doorPortHeldLine = (port: number): string =>
//   `port ${port} is held by another program on this computer, so no computer you own can reach this wsp; free it and open Add a computer again`;
pub fn door_port_held_line(port: i64) -> String {
    format!("port {port} is held by another program on this computer, so no computer you own can reach this wsp; free it and open Add a computer again")
}
// place-link.ts:401
// /** Said when the door port of a host's last start is taken and it opens on another: a computer joined at the old one
//  * dials it for good. */
// export const doorPortMovedLine = (port: number): string =>
//   `port ${port}, where computers you joined last reached this wsp, is held by another program, so the door opened on another; free it and start this wsp again`;
pub fn door_port_moved_line(port: i64) -> String {
    format!("port {port}, where computers you joined last reached this wsp, is held by another program, so the door opened on another; free it and start this wsp again")
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
    serde_json::from_value(serde_json::from_str::<Value>(text).ok()?).ok()
}
// place-link.ts:457
// /** The place file the held read found, read past what the login printed before it. */
// export const heldPlaceFile = (stdout: string): PlaceFile | undefined => parsePlaceFile(heldText(stdout) ?? "");
pub fn held_place_file(stdout: &str) -> Option<PlaceFile> {
    parse_place_file(held_text(stdout).unwrap_or_default())
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
    let text = held_text(stdout)?;
    if text.is_empty() {
        return None;
    }
    if let Some(file) = parse_place_file(text) {
        return Some(HeldPlace::Joined(file));
    }
    let paths = wsp_frames::place_daemon_paths(std::path::Path::new(home));
    let broken = if crate::types::check::js_trim(text) == format!("{} key", crate::types::project_path::PLACE_LEFT_LINE) {
        paths.place_key
    } else {
        paths.place_file
    };
    Some(HeldPlace::Broken(broken.to_string_lossy().into_owned()))
}
// place-link.ts:472
// /** Whether a place file names one of these computers and this host's key: the one test every road over a computer's
//  * ssh login holds the machine it reached to before it changes anything there. */
// export const placeFileNames = (file: PlaceFile | undefined, placeIds: readonly string[], hostPublicKey: string): file is PlaceFile =>
//   file !== undefined && placeIds.includes(file.placeId) && file.hostPublicKey === hostPublicKey;
pub fn place_file_names(file: Option<&PlaceFile>, place_ids: &[String], host_public_key: &str) -> bool {
    file.is_some_and(|f| place_ids.contains(&f.place_id) && f.host_public_key == host_public_key)
}
// place-link.ts:477
// /** The text the file holds, which parsePlaceFile reads back. */
// export const placeFileText = (file: PlaceFile): string => `${JSON.stringify(file, null, 2)}\n`;
pub fn place_file_text(file: &PlaceFile) -> String {
    let mut written = file.clone();
    js_key_order(&mut written.rest);
    format!("{}\n", serde_json::to_string_pretty(&written).expect("a place file writes as JSON"))
}

// The shapes wsp-frames holds read every bound while deserializing, so each Check has nothing left to refuse.
impl Check for PlaceFile {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "hostName", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "hostUrls", &mut out);
        crate::types::keys::req_key::<String>(obj, "hostPublicKey", &mut out);
        crate::types::keys::req_key::<String>(obj, "keyPath", &mut out);
        crate::types::keys::opt_key::<Value>(obj, "joinedAt", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "hostName" => <String as Check>::describe(rest),
                "hostUrls" => <Vec<String> as Check>::describe(rest),
                "hostPublicKey" => <String as Check>::describe(rest),
                "keyPath" => <String as Check>::describe(rest),
                "joinedAt" => <Option<serde_json::Value> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for PlaceEngine {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["none", "docker", "podman"]))
    }
}
impl Check for PlaceReportPlatform {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["darwin", "linux"]))
    }
}
impl Check for PlaceReportEngine {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["none", "docker", "podman"]))
    }
}
impl Check for SshHostSuggestionFrom {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["config", "known_hosts"]))
    }
}

/// The place link's base64 values are wsp_frames' and read every bound at deserialize; nothing is left to check.
impl<const N: usize> Check for wsp_frames::Base64Bytes<N> {}
