// SPDX-License-Identifier: AGPL-3.0-only
//! The catalog: every agent wsp ships and every tool agents reach for, one entry each, and the readers of the table.
//! Port of packages/catalog/src/catalog.ts at a3b870a6e (the rows are in catalog_data); the spec is spec/catalog.md.
//! Each entry serializes as the oracle fixture writes it: a module a TS row holds as an object is written by its id.
use std::cmp::Ordering;
use std::sync::LazyLock;

use indexmap::IndexMap;
use serde::Serialize;

use crate::catalog_data::{agent_modules, tool_rows};
use crate::context::AgentContext;
use crate::hooks::HookCarry;
use crate::mcp_config::McpConfig;
use crate::mcp_login::{login_road, ServerSignInRoad};
use crate::plugins::PluginShelf;
use crate::road_modules::{road_module, InstallHomes};
use crate::roads::{InstallRoad, RoadName};
use crate::signin::{
    has_login, key_env_of, keys_id_of, keys_row_of, login_id_of, login_there, mints_token,
    shared_login_of, KeyFiles, SharedLogin, SignIn,
};
use crate::skills::{BundledSkills, PluginRoad, PluginSkills, SkillRoots};
use wsp_wire::types::agents_report::PageReach;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::{agent_of_row, package_of, tool_row_prefix};
use wsp_words::exit::HostError;
use wsp_words::js::{number_string, to_number};
use wsp_words::semver::compare_versions;
use wsp_words::views::place_setup_words::SignInWay;
use wsp_words::words::computer::{this_computer, Platform};

static CATALOG: LazyLock<Vec<CatalogEntry>> = LazyLock::new(|| {
    agent_modules()
        .into_iter()
        .map(CatalogEntry::Agent)
        .chain(tool_rows().into_iter().map(CatalogEntry::Tool))
        .collect()
});

static LOGIN_ROWS: LazyLock<Vec<LoginRow>> = LazyLock::new(|| {
    let mut rows = Vec::new();
    for e in catalog() {
        let s = e.sign_in();
        match has_login(s) {
            None => {
                if mints_token(s).is_some() || s.note().is_some() {
                    rows.push(LoginRow {
                        id: login_id_of(e.id()),
                        entry: e,
                        sign_in: s.clone(),
                        keys: None,
                    });
                }
            }
            Some(login) => {
                rows.push(LoginRow {
                    id: login_id_of(e.id()),
                    entry: e,
                    sign_in: s.clone(),
                    keys: None,
                });
                if let Some(keys) = &login.keys {
                    rows.push(LoginRow {
                        id: keys_id_of(e.id()),
                        entry: e,
                        sign_in: keys_row_of(keys, s.status()),
                        keys: Some(keys.clone()),
                    });
                }
            }
        }
    }
    rows
});

fn agent_entry(id: &str) -> Option<&'static AgentEntry> {
    catalog_agents().into_iter().find(|a| a.id == id)
}

fn tools_prefix() -> String {
    let prefix = tool_row_prefix("");
    prefix[..prefix.len() - 1].to_string()
}

/// JS `Number(part)` as the text the awk line reads (catalog.ts:414).
fn js_number_text(part: &str) -> String {
    number_string(to_number(part))
}

fn store_line(agent_id: &str, stores: Option<&IndexMap<String, String>>) -> String {
    let variable = agent_entry(agent_id).and_then(|a| a.state_home_env.clone());
    match (variable, stores.and_then(|s| s.get(agent_id))) {
        (Some(variable), Some(store)) => format!("export {variable}={}; ", shell_quote(store)),
        _ => String::new(),
    }
}

fn no_pin(road: &InstallRoad) -> AtPin {
    AtPin {
        road: road.clone(),
        pinned: None,
        note: None,
        below: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Agent,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RoadEvidence {
    Measured,
    Unmeasured,
}

/// catalog.ts:28.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub sessions: u32,
    pub images: u32,
    pub road: RoadEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StateStatus {
    Measured,
    Inferred,
}

/// catalog.ts:38.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectState {
    pub state: String,
    pub location: String,
    pub key: String,
    pub path_fields: Vec<String>,
    #[serde(rename = "move")]
    pub move_: String,
    pub status: StateStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryFormat {
    ClaudeJsonl,
    CodexRollout,
    HermesSqlite,
}

/// catalog.ts:66, in this order.
pub const HISTORY_FORMATS: [HistoryFormat; 3] = [
    HistoryFormat::ClaudeJsonl,
    HistoryFormat::CodexRollout,
    HistoryFormat::HermesSqlite,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionHistory {
    pub format: HistoryFormat,
    pub root: String,
}

/// catalog.ts:78.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentAbout {
    pub creator: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    pub license: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ink {
    pub light: String,
    pub dark: String,
}

/// catalog.ts:92.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentMark {
    pub svg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inks: Option<Vec<Ink>>,
    pub source: String,
    pub license: String,
}

/// catalog.ts:104.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "from", rename_all = "lowercase")]
pub enum LatestSource {
    Npm { package: String },
    Github { repo: String },
    Text { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsageLog {
    pub format: String,
    pub root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AptNeed {
    pub package: String,
    pub command: String,
}

/// catalog.ts:525.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetupNeed {
    pub id: String,
    pub label: String,
    pub after: String,
    pub cmd: String,
    pub check: String,
    pub off: String,
}

/// catalog.ts:107, its base fields first.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEntry {
    pub id: String,
    pub name: String,
    pub kind: EntryKind,
    pub bin: String,
    pub install_road: InstallRoad,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<u32>,
    pub sign_in: SignIn,
    pub config_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volatile: Option<Vec<String>>,
    pub source: Evidence,
    pub size: Size,
    pub about: AgentAbout,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark: Option<AgentMark>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<LatestSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_line: Option<String>,
    pub state_home: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_state_home: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_home_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_key_env: Option<String>,
    pub project_state: Vec<ProjectState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<SessionHistory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prints_limits: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_log: Option<UsageLog>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<AgentContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HookCarry>,
    pub skill_roots: SkillRoots,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_skills: Option<PluginSkills>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundled_skills: Option<BundledSkills>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginRoad>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_shelf: Option<PluginShelf>,
    pub project_docs: Vec<String>,
    pub first_move: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apt_needs: Option<Vec<AptNeed>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_needs: Option<Vec<SetupNeed>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_road: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Brings {
    pub bin: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Major {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolHook {
    pub on: String,
    pub off: String,
}

/// catalog.ts:176, its base fields first.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolEntry {
    pub id: String,
    pub name: String,
    pub kind: EntryKind,
    pub bin: String,
    pub install_road: InstallRoad,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<u32>,
    pub sign_in: SignIn,
    pub config_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volatile: Option<Vec<String>>,
    pub source: Evidence,
    pub size: Size,
    pub default_on: bool,
    pub floor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brings: Option<Vec<Brings>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub covers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends: Option<IndexMap<RoadName, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major: Option<Major>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook: Option<ToolHook>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_node: Option<String>,
}

/// catalog.ts:201, serialized as the entry itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum CatalogEntry {
    Agent(AgentEntry),
    Tool(ToolEntry),
}

impl CatalogEntry {
    pub fn id(&self) -> &str {
        match self {
            CatalogEntry::Agent(a) => &a.id,
            CatalogEntry::Tool(t) => &t.id,
        }
    }
    pub fn name(&self) -> &str {
        match self {
            CatalogEntry::Agent(a) => &a.name,
            CatalogEntry::Tool(t) => &t.name,
        }
    }
    pub fn kind(&self) -> EntryKind {
        match self {
            CatalogEntry::Agent(_) => EntryKind::Agent,
            CatalogEntry::Tool(_) => EntryKind::Tool,
        }
    }
    pub fn bin(&self) -> &str {
        match self {
            CatalogEntry::Agent(a) => &a.bin,
            CatalogEntry::Tool(t) => &t.bin,
        }
    }
    pub fn install_road(&self) -> &InstallRoad {
        match self {
            CatalogEntry::Agent(a) => &a.install_road,
            CatalogEntry::Tool(t) => &t.install_road,
        }
    }
    pub fn sign_in(&self) -> &SignIn {
        match self {
            CatalogEntry::Agent(a) => &a.sign_in,
            CatalogEntry::Tool(t) => &t.sign_in,
        }
    }
    pub fn size(&self) -> &Size {
        match self {
            CatalogEntry::Agent(a) => &a.size,
            CatalogEntry::Tool(t) => &t.size,
        }
    }
    pub fn as_agent(&self) -> Option<&AgentEntry> {
        match self {
            CatalogEntry::Agent(a) => Some(a),
            CatalogEntry::Tool(_) => None,
        }
    }
    pub fn as_tool(&self) -> Option<&ToolEntry> {
        match self {
            CatalogEntry::Agent(_) => None,
            CatalogEntry::Tool(t) => Some(t),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SizeMethod {
    Df,
    Apt,
    Brew,
    Du,
    Unpacked,
}

/// catalog.ts:205: each method's sentence.
pub fn size_method_words(method: SizeMethod) -> &'static str {
    match method {
        SizeMethod::Df => "df before and after the install on a Linux machine, caches included",
        SizeMethod::Apt => "apt Installed-Size summed over the packages the install adds to debian:bookworm",
        SizeMethod::Brew => "installed_size on the x86_64_linux bottle manifests of the formula and its runtime dependencies",
        SizeMethod::Du => "du over what the install wrote, before and after, on a Debian bookworm host",
        SizeMethod::Unpacked => "the Linux x86_64 download, unpacked where it is an archive",
    }
}

/// catalog.ts:215.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Size {
    Measured {
        bytes: u64,
        on: String,
        method: SizeMethod,
    },
    Unmeasured {
        unmeasured: String,
    },
}

/// catalog.ts:218.
pub fn size_bytes(s: &Size) -> Option<u64> {
    match s {
        Size::Measured { bytes, .. } => Some(*bytes),
        Size::Unmeasured { .. } => None,
    }
}

/// Every entry: the agents, then the tools (catalog.ts:232).
pub fn catalog() -> &'static [CatalogEntry] {
    CATALOG.as_slice()
}

/// catalog.ts:315.
pub fn catalog_agents() -> Vec<&'static AgentEntry> {
    catalog()
        .iter()
        .filter_map(CatalogEntry::as_agent)
        .collect()
}

/// The first agent (catalog.ts:320).
pub fn default_agent() -> &'static AgentEntry {
    catalog()
        .iter()
        .find_map(CatalogEntry::as_agent)
        .expect("the catalog has no agent")
}

/// catalog.ts:321.
pub fn catalog_tools() -> Vec<&'static ToolEntry> {
    catalog().iter().filter_map(CatalogEntry::as_tool).collect()
}

/// The state home's variable at the guest's home, then the plugin seed's variable at its folder (catalog.ts:325).
pub fn guest_env(a: &AgentEntry) -> IndexMap<String, String> {
    let mut env = IndexMap::new();
    if let (Some(variable), Some(home)) = (&a.state_home_env, &a.guest_state_home) {
        env.insert(variable.clone(), home.clone());
    }
    if let Some(seed) = a.plugins.and_then(PluginRoad::seed) {
        env.insert(seed.env().to_string(), seed.dir().to_string());
    }
    env
}

/// The agents whose config the catalog knows, in catalog order (catalog.ts:333).
pub fn mcp_agents() -> Vec<&'static AgentEntry> {
    catalog_agents()
        .into_iter()
        .filter(|a| a.mcp.is_some())
        .collect()
}

/// Their ids joined by ", " (catalog.ts:335).
pub fn mcp_agent_ids() -> String {
    mcp_agents()
        .iter()
        .map(|a| a.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// catalog.ts:337.
pub fn mcp_switch(id: &str) -> bool {
    agent_entry(id)
        .and_then(|a| a.mcp.as_ref())
        .is_some_and(|m| m.format.has_enable())
}

/// catalog.ts:351.
pub fn harness_line(
    agent_id: &str,
    line: &str,
    stores: Option<&IndexMap<String, String>>,
    folder: Option<&str>,
) -> String {
    let folder_part = folder
        .map(|f| format!("cd {} 2>/dev/null; ", shell_quote(f)))
        .unwrap_or_default();
    format!("{folder_part}{}{line}", store_line(agent_id, stores))
}

/// `None` for an agent with no MCP login (catalog.ts:357).
pub fn server_sign_in_road(
    agent_id: &str,
    name: &str,
    reach: PageReach,
    brokered: bool,
) -> Option<Result<ServerSignInRoad, HostError>> {
    let login = agent_entry(agent_id)
        .and_then(|a| a.mcp.as_ref())
        .and_then(|m| m.login)?;
    Some(login_road(login, name, reach, brokered))
}

/// catalog.ts:365.
pub fn base_floor() -> Vec<&'static ToolEntry> {
    catalog_tools().into_iter().filter(|t| t.floor).collect()
}

pub const COMPILER_ROW: &str = "build-essential";

/// catalog.ts:371.
pub fn floor_bytes(compiler: bool) -> u64 {
    base_floor()
        .iter()
        .filter(|t| compiler || t.id != COMPILER_ROW)
        .map(|t| size_bytes(&t.size).unwrap_or(0))
        .sum()
}

/// catalog.ts:375.
pub fn install_after(e: &ToolEntry) -> Option<String> {
    road_module(&e.install_road)
        .after()
        .map(str::to_string)
        .or_else(|| e.after.clone())
}

/// catalog.ts:382.
pub fn catalog_tool_by_road(road: RoadName, pkg: &str) -> Option<&'static ToolEntry> {
    catalog_tools().into_iter().find(|e| {
        e.install_road.name() == road
            && road_module(&e.install_road)
                .names(&e.install_road)
                .iter()
                .any(|n| n == pkg)
    })
}

/// catalog.ts:392.
pub fn catalog_id_of_row(id: &str) -> Option<String> {
    if let Some(agent) = agent_of_row(id) {
        return Some(agent);
    }
    if id.starts_with(&tools_prefix()) {
        return catalog_tool_for(&package_of(id)).map(|t| t.id.clone());
    }
    None
}

/// catalog.ts:400.
pub fn catalog_tool_for_dependency(road: RoadName, pkg: &str) -> Option<&'static ToolEntry> {
    catalog_tools().into_iter().find(|e| {
        e.depends
            .as_ref()
            .and_then(|d| d.get(&road))
            .is_some_and(|names| names.iter().any(|n| n == pkg))
    })
}

/// catalog.ts:406.
pub fn catalog_tool_for(pkg: &str) -> Option<&'static ToolEntry> {
    catalog_tools().into_iter().find(|e| {
        e.id == pkg
            || e.bin == pkg
            || road_module(&e.install_road)
                .names(&e.install_road)
                .iter()
                .any(|n| n == pkg)
            || e.covers
                .as_ref()
                .is_some_and(|c| c.iter().any(|n| n == pkg))
            || e.brings
                .as_ref()
                .is_some_and(|b| b.iter().any(|x| x.bin == pkg))
    })
}

/// catalog.ts:411.
pub fn pinned_note(asked: &str, pinned: &str) -> String {
    format!("asked {asked}, installed at the catalog's pinned {pinned}")
}

/// catalog.ts:414; the floor's parts read as `Number` reads them.
pub fn node_at_least(floor: &str) -> String {
    let mut parts = floor.split('.');
    let major = js_number_text(parts.next().unwrap_or(""));
    let minor = js_number_text(parts.next().unwrap_or("0"));
    format!(
        "node --version 2>/dev/null | awk -F. -v a={major} -v b={minor} 'NR==1{{sub(/^v/,\"\",$1); ok=($1+0>a||($1+0==a&&$2+0>=b))}} END{{exit !ok}}'"
    )
}

/// A road below the pin, for a computer whose Node is older than `node`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Below {
    pub node: String,
    pub road: InstallRoad,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AtPin {
    pub road: InstallRoad,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below: Option<Below>,
}

/// catalog.ts:423.
pub fn at_catalog_pin(road: &InstallRoad, asked: Option<&str>) -> AtPin {
    if road.package().is_none() {
        return no_pin(road);
    }
    let entry = catalog_tools().into_iter().find(|e| {
        e.install_road.name() == road.name()
            && e.install_road.package() == road.package()
            && e.install_road.version().is_some()
    });
    let found = entry.and_then(|e| e.install_road.version().map(|v| (e, v.to_string())));
    let Some((entry, version)) = found else {
        return no_pin(road);
    };
    if asked.is_some_and(|a| compare_versions(a, &version) != Ordering::Less) {
        return no_pin(road);
    }
    let pinned_road = road_module(road)
        .at(road, &version)
        .unwrap_or_else(|| road.clone());
    let pinned = Some(version.clone());
    match (asked, &entry.pin_node) {
        (None, _) => AtPin { road: pinned_road, pinned, note: None, below: None },
        (Some(asked), None) => AtPin { road: pinned_road, pinned, note: Some(pinned_note(asked, &version)), below: None },
        (Some(asked), Some(node)) => AtPin {
            road: pinned_road,
            pinned,
            note: Some(format!(
                "{}, or {asked} where the computer's Node is older than {node}, which {} {version} needs",
                pinned_note(asked, &version),
                road.package().unwrap_or_default()
            )),
            below: Some(Below { node: node.clone(), road: road.clone() }),
        },
    }
}

/// catalog.ts:436; errors "<bin>: <note>" where a road has no line.
pub fn below_line(
    road: &InstallRoad,
    below: &Below,
    bin: &str,
    homes: &InstallHomes,
) -> Result<String, HostError> {
    let line = |r: &InstallRoad| {
        road_module(r)
            .install(r, bin, Some(homes))
            .map_err(|n| HostError::new(format!("{bin}: {}", n.0)))
    };
    Ok(format!(
        "if {}; then\n{}\nelse\n{}\nfi",
        node_at_least(&below.node),
        line(road)?,
        line(&below.road)?
    ))
}

/// catalog.ts:446.
pub fn base_entry_for(pkg: &str) -> Option<&'static ToolEntry> {
    catalog_tool_for(pkg).filter(|e| e.floor)
}

/// catalog.ts:455.
pub fn base_note(e: &ToolEntry, here_version: Option<&str>, platform: Platform) -> String {
    let own = format!("{} is part of the base", e.name);
    let (Some(major), Some(here_version)) = (&e.major, here_version) else {
        return own;
    };
    let here = here_version
        .strip_prefix('v')
        .unwrap_or(here_version)
        .split('.')
        .take(major.version.split('.').count())
        .collect::<Vec<_>>()
        .join(".");
    if here == major.version {
        own
    } else {
        format!(
            "{} {} is part of the base; {} runs {} {here}",
            major.name,
            major.version,
            this_computer(platform),
            major.name
        )
    }
}

/// catalog.ts:463.
pub fn catalog_entry(id: &str) -> Option<&'static CatalogEntry> {
    catalog().iter().find(|e| e.id() == id)
}

/// catalog.ts:468.
pub fn agent_name(id: &str) -> String {
    catalog_entry(id)
        .map(|e| e.name().to_string())
        .unwrap_or_else(|| id.to_string())
}

pub const UNMARKED_AGENTS: [&str; 1] = ["crush"];

/// catalog.ts:477.
pub fn agent_mark(id: &str) -> Option<&'static AgentMark> {
    catalog_entry(id)
        .and_then(|e| e.as_agent())
        .and_then(|a| a.mark.as_ref())
}

/// catalog.ts:484.
#[derive(Debug, Clone, PartialEq)]
pub struct LoginRow {
    pub id: String,
    pub entry: &'static CatalogEntry,
    pub sign_in: SignIn,
    pub keys: Option<KeyFiles>,
}

/// catalog.ts:492.
pub fn login_rows() -> &'static [LoginRow] {
    LOGIN_ROWS.as_slice()
}

/// catalog.ts:501.
pub fn login_row(id: &str) -> Option<&'static LoginRow> {
    login_rows().iter().rfind(|r| r.id == id)
}

/// catalog.ts:511.
pub fn login_sign_in(row: &str) -> Option<&'static SignIn> {
    let name = row.strip_prefix("logins/").unwrap_or(row);
    if name.contains('/') {
        return None;
    }
    login_row(name).map(|r| &r.sign_in)
}

/// catalog.ts:518.
pub fn shared_on(agent: &str) -> Option<&'static SharedLogin> {
    login_sign_in(agent).and_then(shared_login_of)
}

/// catalog.ts:535.
pub fn setup_need_rows(agent: &str) -> Vec<SetupNeed> {
    agent_entry(agent)
        .and_then(|a| a.setup_needs.as_ref())
        .map(|needs| {
            needs
                .iter()
                .map(|n| SetupNeed {
                    id: format!("agents/{agent}/{}", n.id),
                    after: format!("agents/{agent}/{}", n.after),
                    ..n.clone()
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AptNeedRow {
    pub id: String,
    pub package: String,
    pub command: String,
}

/// catalog.ts:540.
pub fn apt_need_rows(agent: &str) -> Vec<AptNeedRow> {
    agent_entry(agent)
        .and_then(|a| a.apt_needs.as_ref())
        .map(|needs| {
            needs
                .iter()
                .map(|n| AptNeedRow {
                    id: format!("agents/{agent}/{}", n.package),
                    package: n.package.clone(),
                    command: n.command.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// catalog.ts:546.
pub fn sign_in_ways_of(agent: &str) -> Vec<SignInWay> {
    let Some(s) = agent_entry(agent).map(|a| &a.sign_in) else {
        return Vec::new();
    };
    if mints_token(s).is_none() {
        return Vec::new();
    }
    let mut ways = vec![SignInWay::Token];
    if key_env_of(s).is_some() {
        ways.push(SignInWay::Key);
    }
    if login_there(s).is_some() {
        ways.push(SignInWay::Machine);
    }
    ways
}

/// catalog.ts:553.
pub fn shared_agents_on(agents: &[String]) -> Vec<String> {
    agents
        .iter()
        .filter(|id| shared_on(id).is_some())
        .cloned()
        .collect()
}

/// `<bin> --version` (catalog.ts:558).
pub fn smoke_of(e: &CatalogEntry) -> String {
    format!("{} --version", e.bin())
}

/// `{ line }` or `{ words }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallShown {
    Line(String),
    Words(String),
}

/// catalog.ts:564.
pub fn install_shown(e: &CatalogEntry) -> Result<InstallShown, HostError> {
    let road = e.install_road();
    let module = road_module(road);
    let shown = module.shown(road, e.bin());
    if let Some(words) = shown.as_ref().filter(|_| !module.pastes()) {
        return Ok(InstallShown::Words(words.clone()));
    }
    let line = match shown {
        Some(line) => line,
        None => install_line(e)?,
    };
    Ok(if line.contains('\n') {
        InstallShown::Words(module.words())
    } else {
        InstallShown::Line(line)
    })
}

/// catalog.ts:574; errors "<id>: <note>".
pub fn install_line(e: &CatalogEntry) -> Result<String, HostError> {
    road_module(e.install_road())
        .install(e.install_road(), e.bin(), None)
        .map_err(|n| HostError::new(format!("{}: {}", e.id(), n.0)))
}
