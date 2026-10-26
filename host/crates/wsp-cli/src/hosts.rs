// SPDX-License-Identifier: AGPL-3.0-only
//! The hosts on the person's account this computer can reach, and the one rule that decides which host a line runs
//! against. Port of `packages/host/src/hosts.ts` at 65f6b89c7; the spec is `spec/hosts.md`. Paths are strings joined
//! as node's `path.join` joins them.
use std::collections::HashMap;
use std::ffi::CStr;
use std::fs;
use std::io;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_store::own_file::write_own;
use wsp_wire::types::env::{HOME_ENV, HOST_ENV};
use wsp_wire::views::session_events::host_from_env;
use wsp_wire::wire::place_link::{host_no_key_line, LAUNCHED_WITH};
use wsp_words::app_ports::{is_loopback, is_url, served_hostname};
use wsp_words::exit::{auth_refusal, usage_refusal, HostError, Kind};
use wsp_words::js::{collate, js_trim};
use wsp_words::words::thread::run_for_the_list;

use crate::account::read_relay_record;
use crate::host_lock::serving_host;
use crate::repo_root::join;
use crate::serving_home::{default_home_in, home_named};

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
    default_home_in(&home_directory())
}

/// node's `os.homedir()`: `HOME` where it is set, else the login's own folder.
fn home_directory() -> String {
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        return home.to_string_lossy().into_owned();
    }
    // SAFETY: getpwuid returns a pointer into static storage that is read here before any other call to it.
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() {
            return "/".to_string();
        }
        CStr::from_ptr((*entry).pw_dir).to_string_lossy().into_owned()
    }
}

/// `process.env`: a variable that is not UTF-8 is read lossily, as node reads it.
pub fn process_env() -> Env {
    std::env::vars_os().map(|(key, value)| (key.to_string_lossy().into_owned(), value.to_string_lossy().into_owned())).collect()
}

/// `process.env[key]`, read lossily like `process_env`.
pub fn env_var(key: &str) -> Option<String> {
    std::env::var_os(key).map(|value| value.to_string_lossy().into_owned())
}

/// hosts.ts:48: `WSP_HOME` when non-empty, else the default home.
pub fn wsp_home(env: &Env) -> String {
    home_named(env.get(HOME_ENV).map(String::as_str)).unwrap_or_else(default_home)
}

/// hosts.ts:52.
pub fn hosts_dir(home: &str) -> String {
    join(&[home, "hosts"])
}

/// hosts.ts:57.
pub fn app_logs_dir(home: &str) -> String {
    join(&[home, "logs"])
}

/// hosts.ts:67.
pub const ALIAS_MAX: usize = 64;

/// hosts.ts:70: `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$` and no `..` anywhere.
pub fn alias_ok(alias: &str) -> bool {
    let opens = alias.chars().next().is_some_and(|c| c.is_ascii_alphanumeric());
    opens
        && alias.chars().count() <= ALIAS_MAX
        && alias.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !alias.contains("..")
}

/// hosts.ts:83-90.
pub fn alias_from(name: &str) -> String {
    let mut folded = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            folded.push(c);
        } else {
            folded.extend(std::iter::repeat_n('-', c.len_utf16()));
        }
    }
    let collapsed = collapse_dot_runs(&folded);
    let led = collapsed.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
    let cut: String = led.chars().take(ALIAS_MAX).collect();
    if alias_ok(&cut) {
        cut
    } else {
        "host".to_string()
    }
}

fn collapse_dot_runs(text: &str) -> String {
    let mut out = String::new();
    let mut last_dot = false;
    for c in text.chars() {
        if c == '.' && last_dot {
            continue;
        }
        last_dot = c == '.';
        out.push(c);
    }
    out
}

/// The alias itself, or the refusal for a word that could never be one: every road that names a file reads it here.
fn checked_alias(alias: &str) -> Result<&str, HostError> {
    if alias_ok(alias) {
        return Ok(alias);
    }
    Err(usage_refusal(
        &format!("{} is not a host alias.", serde_json::to_string(alias).unwrap_or_default()),
        &format!("A name is letters, digits, dots, dashes and underscores, opens with a letter or a digit, and is at most {ALIAS_MAX} characters."),
    ))
}

fn host_file(home: &str, alias: &str) -> String {
    join(&[hosts_dir(home).as_str(), &format!("{alias}.json")])
}

/// The record under this alias, or nothing when this computer holds none: a name that could not be a file, a file
/// that is not there and a file somebody hand-edited into nonsense all read the same.
pub fn read_host(home: &str, alias: &str) -> Option<HostRecord> {
    if !alias_ok(alias) {
        return None;
    }
    let parsed: Value = serde_json::from_str(&fs::read_to_string(host_file(home, alias)).ok()?).ok()?;
    if parsed.pointer("/via/kind").and_then(Value::as_str) != Some("account") {
        return None;
    }
    serde_json::from_value(parsed).ok()
}

/// hosts.ts:117-120: `JSON.stringify(record, null, 2)` and a newline through `write_own(home, "hosts/<alias>.json")`.
/// A bad alias refuses with `checkedAlias`'s usage refusal before anything is written.
pub fn write_host(home: &str, alias: &str, record: &HostRecord) -> Result<(), HostError> {
    checked_alias(alias)?;
    let text = format!("{}\n", serde_json::to_string_pretty(record).map_err(|e| HostError::new(e.to_string()))?);
    write_own(Path::new(home), &format!("hosts/{alias}.json"), text).map_err(|e: io::Error| HostError::new(e.to_string()))
}

/// hosts.ts:122-131.
pub fn list_hosts(home: &str) -> Vec<HostEntry> {
    let dir = hosts_dir(home);
    if !Path::new(&dir).exists() {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, HostRecord)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter_map(|name| name.strip_suffix(".json").map(str::to_string))
        .filter_map(|alias| read_host(home, &alias).map(|record| (alias, record)))
        .collect();
    found.sort_by(|a, b| collate(&a.0, &b.0));
    found.into_iter().map(|(alias, record)| HostEntry { alias, url: record.url, device_id: record.device_id }).collect()
}

/// Takes the record away. True when there was one to take; an error when one was there and would not go.
pub fn try_remove_host(home: &str, alias: &str) -> Result<bool, HostError> {
    if read_host(home, alias).is_none() {
        return Ok(false);
    }
    fs::remove_file(host_file(home, alias)).map_err(|e| HostError::new(e.to_string()))?;
    Ok(true)
}

/// `try_remove_host` where a failure is not answered: the TS throws there, and a bool cannot carry it.
pub fn remove_host(home: &str, alias: &str) -> bool {
    try_remove_host(home, alias).unwrap_or_else(|e| panic!("{}", e.message))
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
    let known: Vec<String> = list_hosts(home).into_iter().map(|h| h.alias).collect();
    no_such_host_among(alias, &known)
}

/// hosts.ts:166-169.
pub fn no_such_host_among(alias: &str, known: &[String]) -> String {
    let has = if known.is_empty() { "this computer holds none".to_string() } else { format!("this computer holds {}", known.join(", ")) };
    format!("no host named {alias} is connected; {has}, and wsp login puts the hosts on your account here.")
}

/// hosts.ts:173: `runForTheList("wsp hosts")`.
pub fn read_the_hosts() -> String {
    run_for_the_list("wsp hosts")
}

/// One record with the alias it is under.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldHost {
    pub alias: String,
    pub record: HostRecord,
}

/// hosts.ts:177-182.
pub fn account_records(home: &str) -> Vec<HeldHost> {
    list_hosts(home)
        .into_iter()
        .filter_map(|entry| read_host(home, &entry.alias).map(|record| HeldHost { alias: entry.alias, record }))
        .collect()
}

/// hosts.ts:188-191: less the record whose `via.hostId` is this state's relay record's `hostId`.
pub fn account_hosts(state_path: &str, home: &str) -> Vec<HeldHost> {
    let own_id = read_relay_record(state_path).map(|r| r.host_id);
    account_records(home).into_iter().filter(|held| Some(held.record.via.host_id.as_str()) != own_id.as_deref()).collect()
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
    let mut held = account_hosts(state_path, home);
    match held.len() {
        0 => AccountAim::None,
        1 => {
            let only = held.remove(0);
            AccountAim::One { alias: only.alias, record: only.record }
        }
        _ => AccountAim::Several { aliases: held.into_iter().map(|h| h.alias).collect() },
    }
}

/// hosts.ts:206.
pub fn name_one_host() -> String {
    format!("Name one on the line with --host <name>.\n\n{}", read_the_hosts())
}

/// hosts.ts:210-211.
pub fn several_account_hosts_line(aliases: &[String]) -> String {
    format!(
        "this computer can reach {} hosts on your account ({}) and none of them is the one every line takes.",
        aliases.len(),
        aliases.join(", ")
    )
}

/// hosts.ts:215-218.
pub fn aimed_alias(state_path: &str, home: &str) -> Option<String> {
    match account_aim(state_path, home) {
        AccountAim::One { alias, .. } => Some(alias),
        _ => None,
    }
}

/// hosts.ts:222-224.
pub fn address_not_paired_line(url: &str) -> String {
    format!("--host takes the name of a host on your account; {url} is an address, and wsp hosts lists the names.")
}

/// hosts.ts:228-230.
pub fn device_refused_line(alias: &str) -> String {
    format!("the host {alias} refused this computer's token, which it has taken away; run wsp logout, wsp login and wsp hosts to sign this computer in again.")
}

/// hosts.ts:240-242: `the host at <where> did not answer: <why>`, kind `unreachable`, no fix.
pub fn no_answer_refusal(r#where: &str, why: &str) -> HostError {
    HostError::with_kind(format!("the host at {where} did not answer: {why}"), Kind::Unreachable)
}

/// hosts.ts:246-248.
pub fn no_answer_within(r#where: &str, window_ms: u64) -> HostError {
    no_answer_refusal(r#where, &format!("nothing came back within {window_ms} ms"))
}

/// hosts.ts:251.
pub const NEAR_WINDOW_MS: u64 = 5_000;
/// hosts.ts:256.
pub const FAR_WINDOW_MS: u64 = 15_000;

/// hosts.ts:260-266.
pub fn dial_window_ms(aim: &HostAim) -> u64 {
    if matches!(aim, HostAim::Here) {
        return NEAR_WINDOW_MS;
    }
    if is_loopback_aim(aim) {
        NEAR_WINDOW_MS
    } else {
        FAR_WINDOW_MS
    }
}

fn is_loopback_aim(aim: &HostAim) -> bool {
    aim_address(aim).and_then(|address| served_hostname(&address)).is_some_and(|host| is_loopback(&host))
}

/// hosts.ts:270.
pub const HOST_SIDE_ACCESS: &str = "Handing out access is the one thing a paired computer cannot do from here.";

/// hosts.ts:275-277.
pub fn host_side_only_line(word: &str, r#where: &str) -> String {
    format!("wsp {word} runs on the computer the host runs on, and this line is aimed at {where}.")
}

/// hosts.ts:280-282; `None` is `HOST_SIDE_ACCESS`.
pub fn host_side_only_fix(why: Option<&str>) -> String {
    format!("Run it in a terminal over there. {}", why.unwrap_or(HOST_SIDE_ACCESS))
}

/// hosts.ts:286-288.
pub fn state_ignored_line(r#where: &str) -> String {
    format!("--state names a file on this computer and this line runs against {where}, which serves its own, so it is not read.")
}

/// hosts.ts:291.
pub fn empty_host_refusal() -> HostError {
    usage_refusal("--host was given an empty word, which names no host.", "Name one as wsp hosts lists it, or leave the flag off.")
}

/// hosts.ts:296-303: the `--host` word, then `WSP_HOST`, each trimmed; `None` where neither names one.
pub fn named_host(pick: &HostPick) -> Result<Option<HostAim>, HostError> {
    let env = pick.env.clone().unwrap_or_else(process_env);
    if pick.host.as_deref().is_some_and(|host| js_trim(host).is_empty()) {
        return Err(empty_host_refusal());
    }
    let named =
        [pick.host.as_deref(), env.get(HOST_ENV).map(String::as_str)].into_iter().flatten().map(js_trim).find(|word| !word.is_empty());
    match named {
        None => Ok(None),
        Some(word) => {
            let home = pick.home.clone().unwrap_or_else(|| wsp_home(&env));
            aim_at(word, &home, &env).map(Some)
        }
    }
}

/// hosts.ts:308-326.
pub fn aimed_host(state_path: &str, pick: &HostPick) -> Result<HostAim, HostError> {
    let env = pick.env.clone().unwrap_or_else(process_env);
    let home = pick.home.clone().unwrap_or_else(|| wsp_home(&env));
    if let Some(named) = named_host(pick)? {
        return Ok(named);
    }
    if let Some(carried) = carried_pair(&env) {
        return keyed(HostAim::Url { url: carried.url, token: Some(carried.token), host_key: carried.host_key }, LAUNCHED_WITH);
    }
    if serving_host(state_path).is_some() {
        return Ok(HostAim::Here);
    }
    let mut held = account_hosts(state_path, &home);
    match held.len() {
        0 => Ok(HostAim::Here),
        1 => {
            let only = held.remove(0);
            let alias = only.alias;
            keyed(HostAim::Alias { alias: alias.clone(), record: only.record }, &alias)
        }
        _ => {
            let aliases: Vec<String> = held.into_iter().map(|h| h.alias).collect();
            Err(usage_refusal(&several_account_hosts_line(&aliases), &name_one_host()))
        }
    }
}

fn carried_pair(env: &Env) -> Option<wsp_wire::views::session_events::HostFromEnv> {
    let carried_env: HashMap<String, String> = env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    host_from_env(&carried_env)
}

fn aim_at(named: &str, home: &str, env: &Env) -> Result<HostAim, HostError> {
    if is_url(named) {
        let here = carried_pair(env).filter(|carried| carried.url == named);
        let (token, host_key) = match here {
            Some(carried) => (Some(carried.token), carried.host_key),
            None => (None, None),
        };
        return keyed(HostAim::Url { url: named.to_string(), token, host_key }, named);
    }
    let record = read_host(home, named).ok_or_else(|| usage_refusal(&no_such_host_line(named, home), &read_the_hosts()))?;
    keyed(HostAim::Alias { alias: named.to_string(), record }, named)
}

/// hosts.ts:347-353: an aim holding a token and no key refuses unless it dials this computer's own loopback.
fn keyed(aim: HostAim, where_: &str) -> Result<HostAim, HostError> {
    let held = aim_holds(&aim);
    if held.token.is_none() || held.host_key.is_some() || is_loopback_aim(&aim) {
        return Ok(aim);
    }
    Err(auth_refusal(&host_no_key_line(where_)))
}

/// What an aim elsewhere holds for it (hosts.ts:358-360).
#[derive(Clone, Debug, PartialEq)]
pub struct AimHolds {
    pub token: Option<String>,
    pub host_key: Option<String>,
}

/// hosts.ts:358-360. `Here` holds nothing.
pub fn aim_holds(aim: &HostAim) -> AimHolds {
    match aim {
        HostAim::Here => AimHolds { token: None, host_key: None },
        HostAim::Alias { record, .. } => AimHolds { token: Some(record.device_token.clone()), host_key: record.host_key.clone() },
        HostAim::Url { token, host_key, .. } => AimHolds { token: token.clone(), host_key: host_key.clone() },
    }
}

/// hosts.ts:363-365.
pub fn aim_name(aim: &HostAim) -> String {
    match aim {
        HostAim::Here => "this computer".to_string(),
        HostAim::Alias { alias, .. } => alias.clone(),
        HostAim::Url { url, .. } => url.clone(),
    }
}

/// hosts.ts:369-371. `Here` has no address: `None`.
pub fn aim_address(aim: &HostAim) -> Option<String> {
    match aim {
        HostAim::Here => None,
        HostAim::Alias { record, .. } => Some(record.url.clone()),
        HostAim::Url { url, .. } => Some(url.clone()),
    }
}
