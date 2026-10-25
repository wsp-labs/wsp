// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// What a computer joined as a place keeps about the wsp it belongs to (place-link.ts:410), as parse_place_file reads
/// it: the six fields it checks, `joinedAt` as whatever the file holds there (never checked), and every other key
/// kept in its order, since place_file_text writes back what was read. wsp_frames::PlaceFile is the daemon's stricter
/// reading and is not this one.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceFile {
    pub place_id: String,
    pub name: String,
    pub host_name: String,
    pub host_urls: Vec<String>,
    pub host_public_key: String,
    pub key_path: String,
    pub joined_at: Option<Value>,
    pub rest: IndexMap<String, Value>,
    /// The keys in the order the file held them; empty for a file built here, which writes in field order.
    pub keys: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaceFileKnown {
    place_id: String,
    name: String,
    host_name: String,
    host_urls: Vec<String>,
    host_public_key: String,
    key_path: String,
    #[serde(default, deserialize_with = "kept")]
    joined_at: Option<Value>,
    #[serde(flatten, deserialize_with = "crate::types::kept::record")]
    rest: IndexMap<String, Value>,
}

fn kept<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

impl<'de> Deserialize<'de> for PlaceFile {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = IndexMap::<String, Value>::deserialize(d)?;
        let keys = map.keys().cloned().collect();
        let known: PlaceFileKnown =
            serde_json::from_value(Value::Object(map.into_iter().collect())).map_err(<D::Error as serde::de::Error>::custom)?;
        Ok(PlaceFile {
            place_id: known.place_id,
            name: known.name,
            host_name: known.host_name,
            host_urls: known.host_urls,
            host_public_key: known.host_public_key,
            key_path: known.key_path,
            joined_at: known.joined_at,
            rest: known.rest,
            keys,
        })
    }
}
impl Serialize for PlaceFile {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut all: IndexMap<String, Value> = IndexMap::new();
        all.insert("placeId".into(), Value::from(self.place_id.as_str()));
        all.insert("name".into(), Value::from(self.name.as_str()));
        all.insert("hostName".into(), Value::from(self.host_name.as_str()));
        all.insert("hostUrls".into(), Value::from(self.host_urls.clone()));
        all.insert("hostPublicKey".into(), Value::from(self.host_public_key.as_str()));
        all.insert("keyPath".into(), Value::from(self.key_path.as_str()));
        if let Some(at) = &self.joined_at {
            all.insert("joinedAt".into(), at.clone());
        }
        all.extend(self.rest.clone());
        let mut written: IndexMap<String, Value> = IndexMap::new();
        for key in &self.keys {
            if let Some(v) = all.shift_remove(key) {
                written.insert(key.clone(), v);
            }
        }
        written.extend(all);
        crate::types::kept::serialize_object(&written.into_iter().collect::<Vec<_>>(), s)
    }
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
    Err(SchemaError { code: "invalid_string".into(), path: Vec::new(), message: "Invalid url".into(), detail: Box::new(detail) })
}

/// zod 3.25.76's `.datetime()` regex with no offset allowed: a real calendar date (leap years included), `T`, an hour
/// 00-23 and minute 00-59, optional seconds with an optional fraction, then `Z`.
fn datetime_ok(s: &str) -> bool {
    let b = s.as_bytes();
    let num = |at: usize, len: usize| -> Option<usize> {
        let part = b.get(at..at + len)?;
        part.iter().all(u8::is_ascii_digit).then(|| part.iter().fold(0, |n, d| n * 10 + usize::from(d - b'0')))
    };
    let (Some(year), Some(month), Some(day)) = (num(0, 4), num(5, 2), num(8, 2)) else { return false };
    if b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') || b.get(10) != Some(&b'T') {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    if day < 1 || day > days {
        return false;
    }
    let (Some(hour), Some(minute)) = (num(11, 2), num(14, 2)) else { return false };
    if b.get(13) != Some(&b':') || hour > 23 || minute > 59 {
        return false;
    }
    match &s[16..] {
        "Z" => true,
        rest => {
            let Some(seconds) = rest.strip_prefix(':').and_then(|r| r.strip_suffix('Z')) else { return false };
            let (whole, fraction) = seconds.split_at(seconds.find('.').unwrap_or(seconds.len()));
            whole.len() == 2
                && whole.bytes().all(|c| c.is_ascii_digit())
                && whole.as_bytes()[0] <= b'5'
                && (fraction.is_empty() || (fraction.len() > 1 && fraction[1..].bytes().all(|c| c.is_ascii_digit())))
        }
    }
}

pub(super) fn datetime_string(s: &str) -> Result<(), SchemaError> {
    if datetime_ok(s) {
        return Ok(());
    }
    let mut detail = indexmap::IndexMap::new();
    detail.insert("validation".to_string(), Value::from("datetime"));
    Err(SchemaError { code: "invalid_string".into(), path: Vec::new(), message: "Invalid datetime".into(), detail: Box::new(detail) })
}

/// The daemon unit name zod's regex takes: word characters, `@.:-`, then `.service`.
fn is_daemon_unit(name: &str) -> bool {
    name.strip_suffix(".service")
        .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_alphanumeric() || b"_@.:-".contains(&b)))
}

/// The text after the first line that is exactly WSP_PLACE_HELD, past what a login printed before it.
pub(super) fn held_text(stdout: &str) -> Option<&str> {
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
    #[serde(serialize_with = "crate::types::check::js_record", deserialize_with = "crate::types::kept::record")]
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
        deserialize_with = "crate::types::kept::record_opt",
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
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_disk_size_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
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
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_daemon_version(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
    fn rule_daemon_port(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
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
