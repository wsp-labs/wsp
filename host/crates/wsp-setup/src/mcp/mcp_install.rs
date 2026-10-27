// SPDX-License-Identifier: AGPL-3.0-only
//! `mcp-install.ts`: puts the MCP server into a local agent's config and the wsp skill into its skills folder, under the person's home.
use wsp_catalog::catalog::AgentEntry;
use wsp_wire::views::harness::McpServerSpec;
use wsp_words::exit::HostError;

/// `MCP_SERVER_NAME` (`mcp-install.ts:22`), re-exported from the protocol.
pub use wsp_wire::types::wsp_tools::MCP_SERVER_NAME;

/// `RunningWsp` (`mcp-install.ts:31`), what the install reads of the process it runs in. `tool_server` is the TS `toolServer` (`string | false`), where `Some(None)` is `false`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunningWsp {
    pub exec_path: String,
    pub exec_argv: Vec<String>,
    pub argv: Vec<String>,
    pub version: String,
    pub path: Option<String>,
    pub shim: Option<String>,
    pub tool_server: Option<Option<String>>,
}

/// `runningWsp` (`mcp-install.ts:45`).
pub fn running_wsp() -> RunningWsp {
    todo!()
}

/// `thisComputersPath` (`mcp-install.ts:52`).
pub fn this_computers_path(_path: Option<&str>, _temps: &[String]) -> String {
    todo!()
}

/// `onPath` (`mcp-install.ts:60`).
pub fn on_path(_bin: &str, _path: Option<&str>) -> Option<String> {
    todo!()
}

/// `agentsOnPath` (`mcp-install.ts:70`).
pub fn agents_on_path(_path: Option<&str>) -> Vec<String> {
    todo!()
}

/// `sameFile` (`mcp-install.ts:74`).
pub fn same_file(_a: &str, _b: &str) -> bool {
    todo!()
}

/// `wspCommand` (`mcp-install.ts:94`).
pub fn wsp_command(_run: &RunningWsp) -> McpServerSpec {
    todo!()
}

/// `carriesToolServer` (`mcp-install.ts:109`).
pub fn carries_tool_server(_binary: &str) -> bool {
    todo!()
}

/// `toolServerHere` (`mcp-install.ts:116`), where `None` is the TS `false`.
pub fn tool_server_here() -> Option<String> {
    todo!()
}

/// `toolServerOf` (`mcp-install.ts:127`), where `None` is the TS `false`.
pub fn tool_server_of(_run: &RunningWsp) -> Option<String> {
    todo!()
}

/// `mcpServerCommand` (`mcp-install.ts:143`).
pub fn mcp_server_command(_run: &RunningWsp) -> McpServerSpec {
    todo!()
}

/// `ToolServerLineOptions` (`mcp-install.ts:152`), the inline `given` of `toolServerLine`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolServerLineOptions {
    pub host: Option<String>,
    pub scoped: Option<bool>,
    pub no_slate: Option<bool>,
    pub no_look: Option<bool>,
    pub json: Option<bool>,
}

/// `toolServerLine` (`mcp-install.ts:152`).
pub fn tool_server_line(_state_path: &str, _given: &ToolServerLineOptions, _run: &RunningWsp) -> Option<McpServerSpec> {
    todo!()
}

/// `McpServerSpecOptions` (`mcp-install.ts:162`), the inline `opts` of `mcpServerSpec`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct McpServerSpecOptions {
    pub host: Option<String>,
}

/// `mcpServerSpec` (`mcp-install.ts:162`).
pub fn mcp_server_spec(_state_path: &str, _run: &RunningWsp, _opts: McpServerSpecOptions) -> McpServerSpec {
    todo!()
}

/// `refreshServers` (`mcp-install.ts:176`).
pub fn refresh_servers(_home: &str, _state_path: &str, _run: &RunningWsp) -> Vec<String> {
    todo!()
}

/// `serversRefreshedLine` (`mcp-install.ts:223`).
pub fn servers_refreshed_line(_files: &[String]) -> String {
    todo!()
}

/// `Installed` (`mcp-install.ts:225`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Installed {
    pub agent: String,
    pub path: Option<String>,
    pub skill: String,
    pub docs: Option<Vec<String>>,
}

/// The `{ tilde, abs }` pair `mcpConfigFile` and `skillFile` return (`mcp-install.ts:236`).
#[derive(Clone, Debug, PartialEq)]
pub struct InstallFile {
    pub tilde: String,
    pub abs: String,
}

/// `mcpConfigFile` (`mcp-install.ts:236`), where the TS `McpAgent` is the catalog's `AgentEntry` with an MCP config.
pub fn mcp_config_file(_agent: &AgentEntry, _home: &str) -> InstallFile {
    todo!()
}

/// `skillFile` (`mcp-install.ts:242`).
pub fn skill_file(_agent: &AgentEntry, _home: &str) -> InstallFile {
    todo!()
}

/// `refreshSkills` (`mcp-install.ts:260`).
pub fn refresh_skills(_home: &str, _state_path: &str) -> Vec<String> {
    todo!()
}

/// `skillsRefreshedLine` (`mcp-install.ts:279`).
pub fn skills_refreshed_line(_files: &[String]) -> String {
    todo!()
}

/// `installMcp` (`mcp-install.ts:291`).
pub fn install_mcp(_agent_id: &str, _server: &McpServerSpec, _home: &str, _project: Option<&str>) -> Result<Installed, HostError> {
    todo!()
}

/// `Removed` (`mcp-install.ts:313`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Removed {
    pub agent: String,
    pub docs: Vec<String>,
}

/// `removeMcp` (`mcp-install.ts:319`).
pub fn remove_mcp(_agent_id: &str, _project: &str) -> Result<Removed, HostError> {
    todo!()
}

/// `InstallFailure` (`mcp-install.ts:331`), the `{ id, error }` a failed agent leaves in either report.
#[derive(Clone, Debug, PartialEq)]
pub struct InstallFailure {
    pub id: String,
    pub error: String,
}

/// `Installed & { id }` (`mcp-install.ts:330`), one agent that took the server or the skill, under the id it was asked by.
#[derive(Clone, Debug, PartialEq)]
pub struct InstalledWithId {
    pub id: String,
    pub agent: String,
    pub path: Option<String>,
    pub skill: String,
    pub docs: Option<Vec<String>>,
}

/// `Removed & { id }` (`mcp-install.ts:336`), one agent `--remove` left, under the id it was asked by.
#[derive(Clone, Debug, PartialEq)]
pub struct RemovedWithId {
    pub id: String,
    pub agent: String,
    pub docs: Vec<String>,
}

/// `InstallReport` (`mcp-install.ts:327`), one `wsp mcp install` as a machine reads it.
pub struct InstallReport {
    pub server: McpServerSpec,
    pub installed: Vec<InstalledWithId>,
    pub failures: Vec<InstallFailure>,
}

/// `RemoveReport` (`mcp-install.ts:335`), one `wsp mcp install --remove` as a machine reads it.
pub struct RemoveReport {
    pub removed: Vec<RemovedWithId>,
    pub failures: Vec<InstallFailure>,
}

/// `installEach` (`mcp-install.ts:357`).
pub fn install_each(_agent_ids: &[String], _server: &McpServerSpec, _home: &str, _project: Option<&str>) -> InstallReport {
    todo!()
}

/// `removeEach` (`mcp-install.ts:363`).
pub fn remove_each(_agent_ids: &[String], _project: &str) -> RemoveReport {
    todo!()
}

/// `installLines` (`mcp-install.ts:370`).
pub fn install_lines(_placed: &Installed) -> Vec<String> {
    todo!()
}

/// `removeLines` (`mcp-install.ts:378`).
pub fn remove_lines(_removed: &Removed) -> Vec<String> {
    todo!()
}

/// `registeredLine` (`mcp-install.ts:385`).
pub fn registered_line(_report: &InstallReport) -> Option<String> {
    todo!()
}

/// `nextLine` (`mcp-install.ts:391`).
pub fn next_line(_report: &InstallReport) -> Result<Option<String>, HostError> {
    todo!()
}

/// `notAProjectLine` (`mcp-install.ts:399`).
pub fn not_a_project_line(_folder: &str) -> String {
    todo!()
}
