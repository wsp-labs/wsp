// SPDX-License-Identifier: AGPL-3.0-only
//! The hosts on the person's account this computer can reach, and the one rule that decides which host a line runs
//! against. Port of `packages/host/src/hosts.ts` at 65f6b89c7; the spec is `spec/hosts.md`. Paths are strings joined
//! as node's `path.join` joins them.
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_words::exit::HostError;

/// The environment a line reads, in the order the process holds it.
pub type Env = IndexMap<String, String>;

/// Which account a record came off (hosts.ts:32). `kind` is always `"account"`; a file with any other is no record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Via {
    pub kind: String,
    pub host_id: String,
}

/// hosts.ts:22-33. Written `url, deviceId, deviceToken, hostKey?, pairedAt, via` in the order the writer's struct
/// holds them; the TS writes the object it was handed, so a record keeps the key order its writer built. Read with
/// only `url`, `deviceId`, `deviceToken` (strings) and `via.kind == "account"`, `via.hostId` (string) checked:
/// `pairedAt` may be missing or any value and the record still reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRecord {
    pub url: String,
    pub device_id: String,
    pub device_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paired_at: Option<serde_json::Value>,
    pub via: Via,
}

/// One host as a listing reads it: never the token (hosts.ts:36).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostEntry {
    pub alias: String,
    pub url: String,
    pub device_id: String,
}

/// `join(homedir(), ".wsp")` (hosts.ts:43).
pub fn default_home() -> String {
    todo!()
}

/// hosts.ts:48: `WSP_HOME` when non-empty, else the default home.
pub fn wsp_home(env: &Env) -> String {
    todo!()
}

/// hosts.ts:52.
pub fn hosts_dir(home: &str) -> String {
    todo!()
}

/// hosts.ts:57.
pub fn app_logs_dir(home: &str) -> String {
    todo!()
}

/// hosts.ts:67.
pub const ALIAS_MAX: usize = 64;

/// hosts.ts:70: `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$` and no `..` anywhere.
pub fn alias_ok(alias: &str) -> bool {
    todo!()
}

/// hosts.ts:83-90.
pub fn alias_from(name: &str) -> String {
    todo!()
}

/// hosts.ts:107.
pub fn read_host(home: &str, alias: &str) -> Option<HostRecord> {
    todo!()
}

/// hosts.ts:117-120: `JSON.stringify(record, null, 2)` and a newline through `write_own(home, "hosts/<alias>.json")`.
/// A bad alias refuses with `checkedAlias`'s usage refusal before anything is written.
pub fn write_host(home: &str, alias: &str, record: &HostRecord) -> Result<(), HostError> {
    todo!()
}

/// hosts.ts:122-131.
pub fn list_hosts(home: &str) -> Vec<HostEntry> {
    todo!()
}

/// hosts.ts:134-138. A bad alias reads as no record, so false.
pub fn remove_host(home: &str, alias: &str) -> bool {
    todo!()
}

/// hosts.ts:142, re-exported: the protocol's rule.
pub use wsp_words::app_ports::ws_url_of;

/// hosts.ts:146.
#[derive(Clone, Debug, PartialEq)]
pub enum HostAim {
    Here,
    Alias { alias: String, record: HostRecord },
    Url { url: String, token: Option<String>, host_key: Option<String> },
}

/// hosts.ts:154-158. `env` absent is the process's own environment; `home` absent is `wsp_home(env)`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostPick {
    pub host: Option<String>,
    pub env: Option<Env>,
    pub home: Option<String>,
}

/// hosts.ts:161.
pub fn no_such_host_line(alias: &str, home: &str) -> String {
    todo!()
}

/// hosts.ts:166-169.
pub fn no_such_host_among(alias: &str, known: &[String]) -> String {
    todo!()
}

/// hosts.ts:173: `runForTheList("wsp hosts")`.
pub fn read_the_hosts() -> String {
    todo!()
}

/// One record with the alias it is under.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldHost {
    pub alias: String,
    pub record: HostRecord,
}

/// hosts.ts:177-182.
pub fn account_records(home: &str) -> Vec<HeldHost> {
    todo!()
}

/// hosts.ts:188-191: less the record whose `via.hostId` is this state's relay record's `hostId`.
pub fn account_hosts(state_path: &str, home: &str) -> Vec<HeldHost> {
    todo!()
}

/// hosts.ts:196.
#[derive(Clone, Debug, PartialEq)]
pub enum AccountAim {
    One { alias: String, record: HostRecord },
    Several { aliases: Vec<String> },
    None,
}

/// hosts.ts:198-203.
pub fn account_aim(state_path: &str, home: &str) -> AccountAim {
    todo!()
}

/// hosts.ts:206.
pub fn name_one_host() -> String {
    todo!()
}

/// hosts.ts:210-211.
pub fn several_account_hosts_line(aliases: &[String]) -> String {
    todo!()
}

/// hosts.ts:215-218.
pub fn aimed_alias(state_path: &str, home: &str) -> Option<String> {
    todo!()
}

/// hosts.ts:222-224.
pub fn address_not_paired_line(url: &str) -> String {
    todo!()
}

/// hosts.ts:228-230.
pub fn device_refused_line(alias: &str) -> String {
    todo!()
}

/// hosts.ts:240-242: `the host at <where> did not answer: <why>`, kind `unreachable`, no fix.
pub fn no_answer_refusal(r#where: &str, why: &str) -> HostError {
    todo!()
}

/// hosts.ts:246-248.
pub fn no_answer_within(r#where: &str, window_ms: u64) -> HostError {
    todo!()
}

/// hosts.ts:251.
pub const NEAR_WINDOW_MS: u64 = 5_000;
/// hosts.ts:256.
pub const FAR_WINDOW_MS: u64 = 15_000;

/// hosts.ts:260-266.
pub fn dial_window_ms(aim: &HostAim) -> u64 {
    todo!()
}

/// hosts.ts:270.
pub const HOST_SIDE_ACCESS: &str = "Handing out access is the one thing a paired computer cannot do from here.";

/// hosts.ts:275-277.
pub fn host_side_only_line(word: &str, r#where: &str) -> String {
    todo!()
}

/// hosts.ts:280-282; `None` is `HOST_SIDE_ACCESS`.
pub fn host_side_only_fix(why: Option<&str>) -> String {
    todo!()
}

/// hosts.ts:286-288.
pub fn state_ignored_line(r#where: &str) -> String {
    todo!()
}

/// hosts.ts:291.
pub fn empty_host_refusal() -> HostError {
    todo!()
}

/// hosts.ts:296-303: the `--host` word, then `WSP_HOST`, each trimmed; `None` where neither names one.
pub fn named_host(pick: &HostPick) -> Result<Option<HostAim>, HostError> {
    todo!()
}

/// hosts.ts:308-326.
pub fn aimed_host(state_path: &str, pick: &HostPick) -> Result<HostAim, HostError> {
    todo!()
}

/// What an aim elsewhere holds for it (hosts.ts:358-360).
#[derive(Clone, Debug, PartialEq)]
pub struct AimHolds {
    pub token: Option<String>,
    pub host_key: Option<String>,
}

/// hosts.ts:358-360. `Here` holds nothing.
pub fn aim_holds(aim: &HostAim) -> AimHolds {
    todo!()
}

/// hosts.ts:363-365.
pub fn aim_name(aim: &HostAim) -> String {
    todo!()
}

/// hosts.ts:369-371. `Here` has no address: `None`.
pub fn aim_address(aim: &HostAim) -> Option<String> {
    todo!()
}
