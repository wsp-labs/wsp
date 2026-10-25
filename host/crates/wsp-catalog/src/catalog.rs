// SPDX-License-Identifier: AGPL-3.0-only
//! The catalog: every agent wsp ships and every tool agents reach for, one entry each, and the readers of the table.
//! Port of packages/catalog/src/catalog.ts at a3b870a6e (the rows are in catalog_data); the spec is spec/catalog.md.
//! Each entry serializes as the oracle fixture writes it: a module a TS row holds as an object is written by its id.
use indexmap::IndexMap;
use serde::Serialize;

use crate::context::AgentContext;
use crate::hooks::HookCarry;
use crate::mcp_config::McpConfig;
use crate::mcp_login::ServerSignInRoad;
use wsp_words::exit::HostError;
use wsp_wire::types::agents_report::PageReach;
use wsp_words::words::computer::Platform;
use wsp_words::views::place_setup_words::SignInWay;
use crate::plugins::PluginShelf;
use crate::road_modules::InstallHomes;
use crate::roads::{InstallRoad, RoadName};
use crate::signin::{KeyFiles, SharedLogin, SignIn};
use crate::skills::{BundledSkills, PluginRoad, PluginSkills, SkillRoots};

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
pub const HISTORY_FORMATS: [HistoryFormat; 3] = [HistoryFormat::ClaudeJsonl, HistoryFormat::CodexRollout, HistoryFormat::HermesSqlite];

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
        todo!()
    }
    pub fn name(&self) -> &str {
        todo!()
    }
    pub fn kind(&self) -> EntryKind {
        todo!()
    }
    pub fn bin(&self) -> &str {
        todo!()
    }
    pub fn install_road(&self) -> &InstallRoad {
        todo!()
    }
    pub fn sign_in(&self) -> &SignIn {
        todo!()
    }
    pub fn size(&self) -> &Size {
        todo!()
    }
    pub fn as_agent(&self) -> Option<&AgentEntry> {
        todo!()
    }
    pub fn as_tool(&self) -> Option<&ToolEntry> {
        todo!()
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
    todo!()
}

/// catalog.ts:215.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Size {
    Measured { bytes: u64, on: String, method: SizeMethod },
    Unmeasured { unmeasured: String },
}

/// catalog.ts:218.
pub fn size_bytes(s: &Size) -> Option<u64> {
    todo!()
}

/// Every entry: the agents, then the tools (catalog.ts:232).
pub fn catalog() -> &'static [CatalogEntry] {
    todo!()
}

/// catalog.ts:315.
pub fn catalog_agents() -> Vec<&'static AgentEntry> {
    todo!()
}

/// The first agent (catalog.ts:320).
pub fn default_agent() -> &'static AgentEntry {
    todo!()
}

/// catalog.ts:321.
pub fn catalog_tools() -> Vec<&'static ToolEntry> {
    todo!()
}

/// The state home's variable at the guest's home, then the plugin seed's variable at its folder (catalog.ts:325).
pub fn guest_env(a: &AgentEntry) -> IndexMap<String, String> {
    todo!()
}

/// The agents whose config the catalog knows, in catalog order (catalog.ts:333).
pub fn mcp_agents() -> Vec<&'static AgentEntry> {
    todo!()
}

/// Their ids joined by ", " (catalog.ts:335).
pub fn mcp_agent_ids() -> String {
    todo!()
}

/// catalog.ts:337.
pub fn mcp_switch(id: &str) -> bool {
    todo!()
}

/// catalog.ts:351.
pub fn harness_line(agent_id: &str, line: &str, stores: Option<&IndexMap<String, String>>, folder: Option<&str>) -> String {
    todo!()
}

/// `None` for an agent with no MCP login (catalog.ts:357).
pub fn server_sign_in_road(agent_id: &str, name: &str, reach: PageReach, brokered: bool) -> Option<Result<ServerSignInRoad, HostError>> {
    todo!()
}

/// catalog.ts:365.
pub fn base_floor() -> Vec<&'static ToolEntry> {
    todo!()
}

pub const COMPILER_ROW: &str = "build-essential";

/// catalog.ts:371.
pub fn floor_bytes(compiler: bool) -> u64 {
    todo!()
}

/// catalog.ts:375.
pub fn install_after(e: &ToolEntry) -> Option<String> {
    todo!()
}

/// catalog.ts:382.
pub fn catalog_tool_by_road(road: RoadName, pkg: &str) -> Option<&'static ToolEntry> {
    todo!()
}

/// catalog.ts:392.
pub fn catalog_id_of_row(id: &str) -> Option<String> {
    todo!()
}

/// catalog.ts:400.
pub fn catalog_tool_for_dependency(road: RoadName, pkg: &str) -> Option<&'static ToolEntry> {
    todo!()
}

/// catalog.ts:406.
pub fn catalog_tool_for(pkg: &str) -> Option<&'static ToolEntry> {
    todo!()
}

/// catalog.ts:411.
pub fn pinned_note(asked: &str, pinned: &str) -> String {
    todo!()
}

/// catalog.ts:414; the floor's parts read as `Number` reads them.
pub fn node_at_least(floor: &str) -> String {
    todo!()
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
    todo!()
}

/// catalog.ts:436; errors "<bin>: <note>" where a road has no line.
pub fn below_line(road: &InstallRoad, below: &Below, bin: &str, homes: &InstallHomes) -> Result<String, HostError> {
    todo!()
}

/// catalog.ts:446.
pub fn base_entry_for(pkg: &str) -> Option<&'static ToolEntry> {
    todo!()
}

/// catalog.ts:455.
pub fn base_note(e: &ToolEntry, here_version: Option<&str>, platform: Platform) -> String {
    todo!()
}

/// catalog.ts:463.
pub fn catalog_entry(id: &str) -> Option<&'static CatalogEntry> {
    todo!()
}

/// catalog.ts:468.
pub fn agent_name(id: &str) -> String {
    todo!()
}

pub const UNMARKED_AGENTS: [&str; 1] = ["crush"];

/// catalog.ts:477.
pub fn agent_mark(id: &str) -> Option<&'static AgentMark> {
    todo!()
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
    todo!()
}

/// catalog.ts:501.
pub fn login_row(id: &str) -> Option<&'static LoginRow> {
    todo!()
}

/// catalog.ts:511.
pub fn login_sign_in(row: &str) -> Option<&'static SignIn> {
    todo!()
}

/// catalog.ts:518.
pub fn shared_on(agent: &str) -> Option<&'static SharedLogin> {
    todo!()
}

/// catalog.ts:535.
pub fn setup_need_rows(agent: &str) -> Vec<SetupNeed> {
    todo!()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AptNeedRow {
    pub id: String,
    pub package: String,
    pub command: String,
}

/// catalog.ts:540.
pub fn apt_need_rows(agent: &str) -> Vec<AptNeedRow> {
    todo!()
}

/// catalog.ts:546.
pub fn sign_in_ways_of(agent: &str) -> Vec<SignInWay> {
    todo!()
}

/// catalog.ts:553.
pub fn shared_agents_on(agents: &[String]) -> Vec<String> {
    todo!()
}

/// `<bin> --version` (catalog.ts:558).
pub fn smoke_of(e: &CatalogEntry) -> String {
    todo!()
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
    todo!()
}

/// catalog.ts:574; errors "<id>: <note>".
pub fn install_line(e: &CatalogEntry) -> Result<String, HostError> {
    todo!()
}
