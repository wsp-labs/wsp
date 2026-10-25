// SPDX-License-Identifier: AGPL-3.0-only
//! Which MCP servers a turn of an agent gets in a folder, by scope, and the values the vault hands one turn's launch.
//! Port of packages/catalog/src/mcp-launch.ts at a3b870a6e; the spec is spec/mcp_launch.md. The TS functions that
//! return a Promise do so only to load a TOML parser; the port is synchronous.
use std::cell::RefCell;
use std::collections::HashSet;

use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};

use crate::smol_toml::parse_toml;
use crate::json_parse::parse_json;
use crate::config_write::store_base;
use crate::json_edit::{edit_json, JsonEdit};
use crate::jsonc::{js_stringify_pretty, json_object, read_jsonc};
use crate::jsonc_tree::PathSeg;
use crate::mcp::{McpFormat, McpServer, McpServerScope};
use crate::mcp_codex::codex_trusts;
use crate::mcp_config::HeadlessKey;
use crate::mcp_json::{entry_of, ref_re};
use wsp_words::exit::HostError;

/// One config file as that computer holds it (mcp-launch.ts:16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnFile {
    pub path: String,
    pub text: String,
}

/// mcp-launch.ts:23.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchConfigs {
    pub user: Option<TurnFile>,
    pub projects: Option<Vec<TurnFile>>,
    pub folder: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnScope {
    User,
    Local,
    Project,
}

/// mcp-launch.ts:39.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TurnServer {
    pub server: McpServer,
    pub scope: TurnScope,
    pub file: String,
}

/// mcp-launch.ts:99.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
    pub top: Option<String>,
    pub key: String,
}

/// `-C <folder> rev-parse --path-format=absolute --show-toplevel --git-common-dir` (mcp-launch.ts:105).
pub fn checkout_args(folder: &str) -> Vec<String> {
    ["-C", folder, "rev-parse", "--path-format=absolute", "--show-toplevel", "--git-common-dir"].iter().map(|s| s.to_string()).collect()
}

/// mcp-launch.ts:108.
pub fn checkout_of(said: Option<&str>, folder: &str) -> Checkout {
    let lines: Vec<&str> = said.unwrap_or("").split('\n').map(wsp_words::js::js_trim).collect();
    let top = lines.first().copied().unwrap_or("");
    if top.is_empty() || !top.starts_with('/') {
        return Checkout { top: None, key: folder.to_string() };
    }
    let key = match lines.get(1) {
        Some(common) if common.ends_with("/.git") => parent_of(common),
        _ => top.to_string(),
    };
    Checkout { top: Some(top.to_string()), key }
}

/// The first definition of each name in reading order, where it is on (mcp-launch.ts:148).
pub fn started_servers(servers: &[TurnServer]) -> Vec<TurnServer> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for s in servers {
        if !seen.insert(s.server.name.as_str()) {
            continue;
        }
        if s.server.disabled != Some(true) {
            out.push(s.clone());
        }
    }
    out
}

/// Claude Code: every server the turn's folder starts that reads a name `values` holds, as its whole entry with those
/// filled in place and every other key as written (mcp-launch.ts:161).
pub fn claude_launch_servers(configs: &LaunchConfigs, values: &IndexMap<String, String>) -> IndexMap<String, Value> {
    let mut out = IndexMap::new();
    if values.is_empty() {
        return out;
    }
    let key = configs.key.clone().unwrap_or_else(|| configs.folder.clone());
    let mut texts: IndexMap<String, String> = IndexMap::new();
    for f in configs.user.iter().chain(configs.projects.iter().flatten()) {
        texts.insert(f.path.clone(), f.text.clone());
    }
    for s in started_servers(&claude_servers(configs)) {
        let Some(text) = texts.get(&s.file) else {
            continue;
        };
        let project = (s.scope == TurnScope::Local).then_some(key.as_str());
        let Some(entry) = entry_of(McpFormat::McpServersJson, text, &s.server.name, project) else {
            continue;
        };
        let Ok(parsed) = parse_json(&entry) else {
            continue;
        };
        let (value, any) = filled(&parsed, values);
        if any {
            out.insert(s.server.name.clone(), value);
        }
    }
    out
}

/// Each `${NAME}` and `${NAME:-default}` of a name `values` holds written as its value, the default with it; whether any
/// was. Every other reference stays, for the agent to read from its own environment (mcp-launch.ts:43).
fn filled(v: &Value, values: &IndexMap<String, String>) -> (Value, bool) {
    match v {
        Value::String(s) => {
            let mut any = false;
            let mut out = String::new();
            let mut last = 0;
            for caps in ref_re(McpFormat::McpServersJson).captures_iter(s) {
                let whole = caps.get(0).expect("group 0 always matches");
                let name = caps.get(1).map_or("", |m| m.as_str());
                out.push_str(&s[last..whole.start()]);
                last = whole.end();
                match values.get(name) {
                    Some(got) => {
                        any = true;
                        out.push_str(got);
                    }
                    None => out.push_str(whole.as_str()),
                }
            }
            out.push_str(&s[last..]);
            (Value::String(out), any)
        }
        Value::Array(items) => {
            let each: Vec<(Value, bool)> = items.iter().map(|x| filled(x, values)).collect();
            let any = each.iter().any(|(_, a)| *a);
            (Value::Array(each.into_iter().map(|(x, _)| x).collect()), any)
        }
        Value::Object(map) => {
            let mut any = false;
            let mut out = Map::new();
            for (k, x) in map {
                let (value, a) = filled(x, values);
                any |= a;
                out.insert(k.clone(), value);
            }
            (Value::Object(out), any)
        }
        other => (other.clone(), false),
    }
}

/// The ones of `keys` a TOML config sets no top-level value of, read by a TOML parser; none where the text does not
/// parse (mcp-launch.ts:180).
pub fn unset_toml_keys(text: &str, keys: &[HeadlessKey]) -> Vec<HeadlessKey> {
    let Ok(top) = parse_toml(text) else {
        return Vec::new();
    };
    keys.iter().filter(|k| !top.contains_key(&k.key)).cloned().collect()
}

fn plain_key(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Codex: the config keys a thread's start sets so each server that passes a name `values` holds through its env_vars or
/// env_http_headers gets it (mcp-launch.ts:194).
pub fn codex_launch_config(configs: &LaunchConfigs, values: &IndexMap<String, String>) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    if values.is_empty() {
        return out;
    }
    for ts in started_servers(&codex_servers(configs)) {
        let s = &ts.server;
        let Some(reads) = &s.reads else {
            continue;
        };
        if !plain_key(&s.name) {
            continue;
        }
        for name in &reads.env {
            if let Some(v) = values.get(name).filter(|_| plain_key(name)) {
                out.insert(format!("mcp_servers.{}.env.{name}", s.name), v.clone());
            }
        }
        for (header, name) in &reads.headers {
            if let Some(v) = values.get(name).filter(|_| plain_key(header)) {
                out.insert(format!("mcp_servers.{}.http_headers.{header}", s.name), v.clone());
            }
        }
    }
    out
}

/// The values one launch hands an agent's servers, as its CLI takes them (mcp-launch.ts:206).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LaunchServerValues {
    pub entries: Option<IndexMap<String, Value>>,
    pub config: Option<IndexMap<String, String>>,
}

/// A folder's own switches by list key (mcp-launch.ts:212).
pub type FolderSwitches = IndexMap<String, Vec<String>>;

/// mcp-launch.ts:233.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedServers {
    pub files: Vec<String>,
    pub base: String,
    pub folder: Option<String>,
    pub trust: bool,
}

/// The agents whose CLI takes servers at launch (mcp-launch.ts:258).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchServerRoad {
    Claude,
    Codex,
}

/// The road of an agent id, if it has one.
pub fn launch_server_road(agent: &str) -> Option<LaunchServerRoad> {
    match agent {
        "claude" => Some(LaunchServerRoad::Claude),
        "codex" => Some(LaunchServerRoad::Codex),
        _ => None,
    }
}

/// The lists Claude Code keeps under a folder's entry in its user file that switch servers off there (mcp-launch.ts:226).
const CLAUDE_SWITCHES: [&str; 2] = ["disabledMcpServers", "disabledMcpjsonServers"];

/// The user file's `projects[folder]` entry, or nothing where the file holds none (mcp-launch.ts:116).
fn folder_entry(text: &str, folder: &str) -> Option<Map<String, Value>> {
    let root = read_jsonc(text).ok()?;
    let projects = root.as_object()?.get("projects")?;
    json_object(projects)?.get(folder).and_then(|e| e.as_object()).cloned()
}

/// The names a list holds that are strings, in order (mcp-launch.ts:43, `namesIn`).
fn names_in(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

/// The folder's switches its user file holds under `key`, the lists that name any server; nothing where none does
/// (mcp-launch.ts:228).
fn claude_switches_of(text: &str, key: &str) -> Option<FolderSwitches> {
    let entry = folder_entry(text, key);
    let mut out = FolderSwitches::new();
    for k in CLAUDE_SWITCHES {
        let names = names_in(entry.as_ref().and_then(|e| e.get(k)));
        if !names.is_empty() {
            out.insert(k.to_string(), names);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// The user file's text with each list of `switches` under `folder`'s entry, joined with the names already there
/// (mcp-launch.ts:226).
fn claude_switches_put(text: Option<&str>, folder: &str, switches: &FolderSwitches) -> Result<String, HostError> {
    let Some(text) = text.filter(|t| !wsp_words::js::js_trim(t).is_empty()) else {
        let mut entry = Map::new();
        for (k, names) in switches {
            entry.insert(k.clone(), Value::Array(names.iter().map(|n| Value::String(n.clone())).collect()));
        }
        let mut projects = Map::new();
        projects.insert(folder.to_string(), Value::Object(entry));
        let mut root = Map::new();
        root.insert("projects".to_string(), Value::Object(projects));
        return Ok(format!("{}\n", js_stringify_pretty(&Value::Object(root))));
    };
    let existing = folder_entry(text, folder);
    let mut edits: Vec<JsonEdit> = Vec::new();
    for (k, names) in switches {
        let held = names_in(existing.as_ref().and_then(|e| e.get(k)));
        let merged: IndexSet<String> = held.into_iter().chain(names.iter().cloned()).collect();
        let path = vec![PathSeg::Key("projects".into()), PathSeg::Key(folder.into()), PathSeg::Key(k.clone())];
        edits.push((path, Some(Value::Array(merged.into_iter().map(Value::String).collect()))));
    }
    edit_json(text, &edits)
}

/// The servers a Claude Code turn starts, as 2.1.296 loads them (mcp-launch.ts:58).
fn claude_servers(configs: &LaunchConfigs) -> Vec<TurnServer> {
    let key = configs.key.clone().unwrap_or_else(|| configs.folder.clone());
    let user = configs.user.as_ref();
    let entry = user.and_then(|u| folder_entry(&u.text, &key));
    let off: HashSet<String> = names_in(entry.as_ref().and_then(|e| e.get("disabledMcpServers"))).into_iter().collect();
    let left: HashSet<String> = names_in(entry.as_ref().and_then(|e| e.get("disabledMcpjsonServers"))).into_iter().collect();
    let own = match user {
        Some(u) => McpFormat::McpServersJson.read(&u.text, &key),
        None => Vec::new(),
    };
    let mut project: Vec<TurnServer> = Vec::new();
    for f in configs.projects.iter().flatten() {
        for server in McpFormat::McpServersJson.read(&f.text, &key) {
            if server.scope == McpServerScope::User && !left.contains(&server.name) {
                project.push(TurnServer { server, scope: TurnScope::Project, file: f.path.clone() });
            }
        }
    }
    let mine = |scope: McpServerScope| -> Vec<TurnServer> {
        own.iter()
            .filter(|s| s.scope == scope)
            .map(|s| TurnServer {
                server: s.clone(),
                scope: if scope == McpServerScope::Home { TurnScope::Local } else { TurnScope::User },
                file: user.map(|u| u.path.clone()).unwrap_or_default(),
            })
            .collect()
    };
    let mut all = mine(McpServerScope::Home);
    all.extend(project);
    all.extend(mine(McpServerScope::User));
    all.into_iter()
        .map(|mut s| {
            if off.contains(&s.server.name) {
                s.server.disabled = Some(true);
            }
            s
        })
        .collect()
}

/// The servers a Codex turn starts, as 0.162.1 loads them (mcp-launch.ts:136).
fn codex_servers(configs: &LaunchConfigs) -> Vec<TurnServer> {
    let Some(user) = configs.user.as_ref() else {
        return Vec::new();
    };
    let files: &[TurnFile] = configs.projects.as_deref().unwrap_or(&[]);
    let trusted = !files.is_empty() && codex_trusts(&user.text, configs.key.as_deref().unwrap_or(&configs.folder));
    let mut out = Vec::new();
    if trusted {
        for f in files {
            for server in McpFormat::CodexToml.read(&f.text, &configs.folder) {
                out.push(TurnServer { server, scope: TurnScope::Project, file: f.path.clone() });
            }
        }
    }
    for server in McpFormat::CodexToml.read(&user.text, &configs.folder) {
        out.push(TurnServer { server, scope: TurnScope::User, file: user.path.clone() });
    }
    out
}

/// `path.split("/")` with its `.` and `..` parts read and no empty part, rebuilt as an absolute path (mcp-launch.ts:65).
fn normal(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    format!("/{}", out.join("/"))
}

fn parent_of(path: &str) -> String {
    match path.rfind('/') {
        Some(i) if i > 0 => path[..i].to_string(),
        _ => "/".to_string(),
    }
}

fn join(folder: &str, rel: &str) -> String {
    normal(&format!("{folder}/{rel}"))
}

/// Each folder from `folder` up to the root, nearest first, or up to `top` where `top` holds it (mcp-launch.ts:73).
fn up_from(folder: &str, top: Option<&str>) -> Vec<String> {
    let mut out = vec![normal(folder)];
    let end = top.map(normal);
    let holds = match &end {
        Some(e) => *e == out[0] || out[0].starts_with(&format!("{}/", if e == "/" { "" } else { e.as_str() })),
        None => false,
    };
    while out.last().is_some_and(|l| l != "/") && !(holds && end.as_ref() == out.last()) {
        let last = out.last().cloned().unwrap_or_default();
        out.push(parent_of(&last));
    }
    if holds || end.is_none() {
        out
    } else {
        out.truncate(1);
        out
    }
}

impl LaunchServerRoad {
    /// `<store or home>/.claude.json`, or `<store or home/.codex>/config.toml`.
    pub fn user(self, store: Option<&str>, home: &str) -> String {
        match self {
            LaunchServerRoad::Claude => format!("{}/.claude.json", store.unwrap_or(home)),
            LaunchServerRoad::Codex => match store {
                Some(s) => format!("{s}/config.toml"),
                None => format!("{home}/.codex/config.toml"),
            },
        }
    }

    /// Claude: `.mcp.json` in each folder from `folder` up to `/`; Codex: `.codex/config.toml` up to `top` (or the
    /// folder alone where `top` does not hold it). Paths normalised as mcp-launch.ts:84 does.
    pub fn project_files(self, folder: &str, top: Option<&str>) -> Vec<String> {
        match self {
            LaunchServerRoad::Claude => up_from(folder, None).iter().map(|d| join(d, ".mcp.json")).collect(),
            LaunchServerRoad::Codex => up_from(folder, Some(top.unwrap_or(folder))).iter().map(|d| join(d, ".codex/config.toml")).collect(),
        }
    }

    pub fn servers(self, configs: &LaunchConfigs) -> Vec<TurnServer> {
        match self {
            LaunchServerRoad::Claude => claude_servers(configs),
            LaunchServerRoad::Codex => codex_servers(configs),
        }
    }

    pub fn carry(self, folder: &str, own_files: &[String], own_base: &str) -> CarriedServers {
        match self {
            LaunchServerRoad::Claude => {
                CarriedServers { files: own_files.to_vec(), base: own_base.to_string(), folder: Some(folder.to_string()), trust: false }
            }
            LaunchServerRoad::Codex => {
                CarriedServers { files: vec![join(folder, ".codex/config.toml")], base: folder.to_string(), folder: None, trust: true }
            }
        }
    }

    /// Claude Code's `disabledMcpServers` and `disabledMcpjsonServers` under the folder's entry; `None` for Codex,
    /// `Some(None)` where no list names a server.
    pub fn switches_of(self, text: &str, key: &str) -> Option<Option<FolderSwitches>> {
        match self {
            LaunchServerRoad::Claude => Some(claude_switches_of(text, key)),
            LaunchServerRoad::Codex => None,
        }
    }

    /// The user file with each list joined into the folder's entry; `None` for Codex (mcp-launch.ts:226).
    pub fn switches_put(self, text: Option<&str>, folder: &str, switches: &FolderSwitches) -> Option<Result<String, HostError>> {
        match self {
            LaunchServerRoad::Claude => Some(claude_switches_put(text, folder, switches)),
            LaunchServerRoad::Codex => None,
        }
    }

    pub fn fill(self, configs: &LaunchConfigs, values: &IndexMap<String, String>) -> LaunchServerValues {
        match self {
            LaunchServerRoad::Claude => LaunchServerValues { entries: Some(claude_launch_servers(configs, values)), config: None },
            LaunchServerRoad::Codex => LaunchServerValues { entries: None, config: Some(codex_launch_config(configs, values)) },
        }
    }

    pub fn hands(self, server: &McpServer, name: &str) -> bool {
        match self {
            LaunchServerRoad::Claude => true,
            LaunchServerRoad::Codex => match &server.reads {
                Some(reads) => reads.env.iter().any(|e| e == name) || reads.headers.values().any(|h| h == name),
                None => false,
            },
        }
    }
}

/// An agent as the resolver reads it (mcp-launch.ts:279).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnAgent {
    pub id: String,
    pub format: McpFormat,
    pub files: Vec<String>,
    pub project_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerFiles {
    pub user: Vec<String>,
    pub base: String,
    pub projects: Vec<String>,
}

/// mcp-launch.ts:285.
pub fn turn_server_files(agent: &TurnAgent, folder: &str, home: &str, store: Option<&str>, top: Option<&str>) -> ServerFiles {
    let own = own_server_config(&agent.id, &agent.files, home, store);
    let projects: Vec<String> = match launch_server_road(&agent.id) {
        Some(road) => road.project_files(folder, top),
        None => agent.project_files.iter().flatten().map(|f| join(folder, f)).collect(),
    };
    ServerFiles { projects: projects.into_iter().filter(|f| !own.files.contains(f)).collect(), user: own.files, base: own.base }
}

/// mcp-launch.ts:296.
pub fn turn_servers(agent: &TurnAgent, configs: &LaunchConfigs) -> Vec<TurnServer> {
    if let Some(road) = launch_server_road(&agent.id) {
        return road.servers(configs);
    }
    let mut out = Vec::new();
    if let Some(first) = configs.projects.iter().flatten().next() {
        for server in agent.format.read(&first.text, &configs.folder).into_iter().filter(|s| s.scope == McpServerScope::User) {
            out.push(TurnServer { server, scope: TurnScope::Project, file: first.path.clone() });
        }
    }
    if let Some(user) = &configs.user {
        for server in agent.format.read(&user.text, &configs.folder).into_iter().filter(|s| s.scope == McpServerScope::User) {
            out.push(TurnServer { server, scope: TurnScope::User, file: user.path.clone() });
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnConfig {
    pub files: Vec<String>,
    pub base: String,
}

/// mcp-launch.ts:310.
pub fn own_server_config(agent: &str, files: &[String], home: &str, store: Option<&str>) -> OwnConfig {
    if let (Some(store), Some(road)) = (store, launch_server_road(agent)) {
        return OwnConfig { files: vec![road.user(Some(store), home)], base: store_base(store, home) };
    }
    let files = files
        .iter()
        .map(|f| match f.strip_prefix("~/") {
            Some(rest) => format!("{home}/{rest}"),
            None => f.clone(),
        })
        .collect();
    OwnConfig { files, base: home.to_string() }
}

/// The held names a server reads that the launch cannot hand it, sorted (mcp-launch.ts:319).
pub fn launch_misses(agent: &str, format: McpFormat, server: &McpServer, held: &IndexSet<String>) -> Vec<String> {
    let read: RefCell<IndexSet<String>> = RefCell::new(IndexSet::new());
    format.resolve(server, &|name| {
        read.borrow_mut().insert(name.to_string());
        Some(String::new())
    });
    let road = launch_server_road(agent);
    let mut out: Vec<String> =
        read.into_inner().into_iter().filter(|name| held.contains(name) && road.is_none_or(|r| !r.hands(server, name))).collect();
    out.sort_by(|a, b| wsp_words::js::utf16::cmp(a, b));
    out
}
