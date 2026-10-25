// SPDX-License-Identifier: AGPL-3.0-only
//! The few items of golden-base.ts, golden-import.ts, golden-tools.ts and golden.ts that the ported modules read,
//! while golden*.ts waits on #2441's ruling. One seam for the whole crate: a slice that needs another golden item
//! adds it here. The constants are the TS values printed by oracle/machines.oracle.ts, not retyped.
use serde::{Deserialize, Serialize};

pub use wsp_wire::views::golden_image::GoldenBaseTool;

/// golden-base.ts:108. One `echo "VERSION <name>: ..."` line per base floor command.
pub const BASE_VERSION_LINES: &str = r#"echo "VERSION curl: $(curl --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION uv: $(uv --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION python3: $(python3 --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION git: $(git --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION jq: $(jq --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION rg: $(rg --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION cc: $(cc --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION cmake: $(cmake --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION ninja: $(ninja --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION fd: $(fd --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION sqlite3: $(sqlite3 --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION wget: $(wget --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION zip: $(zip --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION unzip: $(unzip -v 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION xz: $(xz --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION rsync: $(rsync --version 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')"
echo "VERSION sshd: $(sh -c 'sshd -V 2>&1' 2>/dev/null | grep -m1 -E '[0-9]+\.[0-9]+')""#;

/// golden-import.ts. The PATH every install and probe runs on.
pub const TOOLS_PATH: &str = r#"/root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin"#;

/// golden-import.ts:821. The environment line every box command runs under.
pub const EXEC_ENV: &str = r#"export HOME=/root USER=root"#;

/// golden-tools.ts:72. 2 GiB, in bytes.
pub const TOOLS_DISK_FLOOR: u64 = 2147483648;

/// golden-tools.ts:44. df's columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DfColumn {
    Size,
    Used,
    Free,
}

/// golden-tools.ts:49. `df -Pk <path> | awk 'NR==2{print $2, $4}'`, the columns in the order asked; `path` is
/// written as given, unquoted; `None` is `/root`.
pub fn df_kb_cmd(columns: &[DfColumn], path: Option<&str>) -> String {
    todo!()
}

/// golden-base.ts:114. Each `VERSION <name>: <line>` whose line carries `\d+\.\d+(\.\d+)?`, as that number alone;
/// lines are read with their trailing whitespace trimmed; `<name>` stops at the first colon.
pub fn parse_versions(stdout: &str) -> Vec<GoldenBaseTool> {
    todo!()
}

/// golden.ts:547 `ImportResult`, the fields machine-context's `merge_facts` reads and nothing more.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub recipe_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<ImportFiles>,
    pub tools: Vec<ImportRow>,
    pub agents: Vec<ImportAgentRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired: Option<Vec<RetiredRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Vec<ImportRow>>,
}

/// A tool or base row's outcome (`ToolResult`): its id, label, outcome word and note.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportRow {
    pub id: String,
    pub label: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// An agent row's outcome (`AgentResult`): `name` where a tool row says `label`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportAgentRow {
    pub id: String,
    pub name: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetiredRow {
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportFiles {
    pub bytes: u64,
    pub skipped: Vec<SkippedPath>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SkippedPath {
    /// golden-import.ts:109: the recipe row the path came from.
    #[serde(default)]
    pub id: String,
    pub path: String,
    pub note: String,
}

// --- provision's golden items (provision.ts, provision-files.ts, provision-mcp.ts at a4033dc34) ------------------
// What follows is the provision slice's: the golden-import, golden-tools, golden-base, golden and golden-mcp items its
// three modules call, with the TS signature. A provision test that reaches one of these bodies stays red until the
// golden port; spec/provision-slice.md lists which.
use std::collections::BTreeSet;
use std::sync::Arc;

use futures::future::BoxFuture;
use indexmap::{IndexMap, IndexSet};

use wsp_catalog::mcp::{McpFormat, MergeOutcome};
pub use wsp_wire::views::golden_image::GoldenStage;
use wsp_wire::wire::machine_link::ExecResult;

use crate::errors::Result;
use crate::machine::Machine;

// --- golden-import.ts ------------------------------------------------------------------------------------------

/// golden-import.ts:686 `PinRead`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinRead {
    pub read: Option<String>,
    pub fixed: bool,
    pub words: String,
}

/// golden-import.ts:671 `below`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Below {
    pub node: String,
    pub asks: Option<String>,
}

/// golden-import.ts:643 `ToolInstall`. `manager` is the road's name as the TS spells it (`apt`, `brew`, `npm`, ...).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolInstall {
    pub id: String,
    pub label: String,
    pub manager: String,
    pub cmd: String,
    pub after: Option<String>,
    pub bin: Option<String>,
    pub check: Option<String>,
    pub present: Option<String>,
    pub bins: Option<Vec<String>>,
    pub asks: Option<String>,
    pub below: Option<Below>,
    pub shown: Option<String>,
    pub note: Option<String>,
    pub pin: Option<PinRead>,
    pub bytes: Option<u64>,
}

/// golden-import.ts AgentsPlan, as provisionPlanOf hands it to agentSteps: the agents' installs and the node step.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgentsPlan {
    pub installs: Vec<serde_json::Value>,
    pub node: Option<serde_json::Value>,
}

/// golden-import.ts:1512 `agentSteps(plan, CATALOG_AGENTS, path, prefix)`.
pub fn agent_steps(plan: &AgentsPlan, path: &str, prefix: Option<&str>) -> Vec<ToolInstall> {
    todo!()
}

/// golden-import.ts:1462 `agentInstallsFor(entries)`, as the provision tests call it: the recipe rows by id, the
/// catalog's agents, now.
pub fn agent_installs_for(ids: &[&str]) -> AgentsPlan {
    todo!()
}

/// golden-import.ts:1083 `toolInstallsFor(entries, new Map(), [], path, prefix).tools`, as the provision tests call it.
pub fn tool_installs_for(ids: &[&str], path: &str, prefix: Option<&str>) -> Vec<ToolInstall> {
    todo!()
}

/// golden-import.ts:812 `pathLine(path, prefix)`: `export PATH=<path> NAME=value ...` with the install homes' env.
pub fn path_line(path: &str, prefix: Option<&str>) -> String {
    todo!()
}

/// golden-import.ts:1058 `viaRoad`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViaRoad {
    Step { cmd: String, shown: String },
    Note(String),
}

/// golden-import.ts:1058 `viaRoad(road, bin, path, prefix)`.
pub fn via_road(road: &wsp_catalog::roads::InstallRoad, bin: &str, path: &str, prefix: Option<&str>) -> ViaRoad {
    todo!()
}

// --- golden-tools.ts -------------------------------------------------------------------------------------------

/// golden-tools.ts:15 `ToolResult`. `outcome` is `installed`, `failed` or `skipped`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolResult {
    pub id: String,
    pub label: String,
    pub outcome: String,
    pub note: Option<String>,
    pub ms: Option<i64>,
    pub bytes: Option<i64>,
}

/// golden-tools.ts:50 `FREE_KB_CMD`.
pub fn free_kb_cmd() -> String {
    todo!()
}

/// golden-tools.ts:128 `diskUse`: `Some(size_bytes)` for `{ kind: "use" }`, `None` for `{ kind: "unknown" }`.
pub async fn disk_use_size(machine: &dyn Machine, prefix: Option<&str>) -> Option<u64> {
    todo!()
}

/// golden-tools.ts:372 `ownedFloorBytes`.
pub fn owned_floor_bytes(size_bytes: Option<u64>) -> u64 {
    todo!()
}

/// A tools-loop stage line: `(stage, detail)`.
pub type ToolStage = Arc<dyn Fn(GoldenStage, Option<&str>) + Send + Sync>;
/// `onTool`: awaited for each row as the loop reaches it.
pub type OnTool = Arc<dyn Fn(ToolResult) -> BoxFuture<'static, ()> + Send + Sync>;

/// golden-tools.ts InstallToolsOptions, the fields provision passes.
#[derive(Clone, Default)]
pub struct InstallToolsOptions {
    pub present: BTreeSet<String>,
    /// `caches: "keep"`.
    pub keep_caches: bool,
    pub floor: Option<u64>,
    pub path: Option<String>,
    pub prefix: Option<String>,
    pub on_tool: Option<OnTool>,
}

/// golden-tools.ts:379 `installTools(machine, tools, onStage, "installing-tools", opts)`'s `tools`.
pub async fn install_tools(
    machine: &dyn Machine,
    tools: &[ToolInstall],
    on_stage: ToolStage,
    opts: InstallToolsOptions,
) -> Result<Vec<ToolResult>> {
    todo!()
}

// --- golden-base.ts --------------------------------------------------------------------------------------------

/// golden-base.ts:36 `baseInstalls(carried, path, prefix, left)`.
pub fn base_installs(carried: &BTreeSet<String>, path: &str, prefix: Option<&str>, left: &BTreeSet<String>) -> Vec<ToolInstall> {
    todo!()
}

/// golden-base.ts:160 `BaseOutcome`, the fields provision reads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BaseOutcome {
    pub tools: Vec<ToolResult>,
    pub line: String,
}

/// golden-base.ts:169 `installBase(machine, stage, { caches: "keep", path, prefix, left })`.
pub async fn install_base(
    machine: &dyn Machine,
    stage: ToolStage,
    path: &str,
    prefix: Option<&str>,
    left: &BTreeSet<String>,
) -> Result<BaseOutcome> {
    todo!()
}

// --- golden.ts -------------------------------------------------------------------------------------------------

/// golden.ts:461.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFile {
    pub dest: String,
    pub digest: String,
}

/// golden.ts:434, the fields provision reads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PackedFiles {
    pub tar: Vec<u8>,
    pub bytes: u64,
    pub unpacked: u64,
    pub skipped: Vec<SkippedPath>,
    pub files: Option<u64>,
    /// `None` is the TS's absent `stood`; `Some(vec![])` is a pack that was asked and left nothing home.
    pub stood: Option<Vec<String>>,
}

/// golden.ts:467 `LeaveOut`.
pub type LeaveOut = Arc<dyn Fn(Vec<StagedFile>) -> BoxFuture<'static, Result<Vec<String>>> + Send + Sync>;
/// golden.ts:473 `PackFiles`: `pack(leaveOut?)`. A pack that throws is an `Err`.
pub type PackFiles = Arc<dyn Fn(Option<LeaveOut>) -> BoxFuture<'static, Result<PackedFiles>> + Send + Sync>;

/// golden.ts:475 `GoldenImport`, the fields provisionPlanOf reads.
#[derive(Clone, Default)]
pub struct GoldenImport {
    pub agents: Vec<serde_json::Value>,
    pub node: Option<serde_json::Value>,
    pub tools: Vec<ToolInstall>,
    pub files: Option<GoldenFiles>,
    pub mcp: Option<McpPlan>,
    pub skipped_agents: Vec<SkippedRow>,
    pub skipped_tools: Vec<SkippedRow>,
    /// `recipe.files`: each file row's id, dest and `volatile`.
    pub recipe_files: Vec<RecipeFileRow>,
}

/// golden.ts:479 `files`, the fields read.
#[derive(Clone)]
pub struct GoldenFiles {
    pub lands: Vec<crate::provision_files::ProvisionLanding>,
    pub pack: PackFiles,
}

/// `skippedAgents` (`{ id, name, note }`) and `skippedTools` (`{ id, label, note }`), one shape: `label` is the
/// agent's `name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedRow {
    pub id: String,
    pub label: String,
    pub note: String,
}

/// A recipe file row as onceDests reads it (provision.ts:113).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeFileRow {
    pub id: String,
    pub dest: String,
    pub volatile: Option<bool>,
}

/// golden.ts:31 `StageListener`, as provision calls it: `(stage, detail)`.
pub type StageListener = Arc<dyn Fn(GoldenStage, Option<&str>) + Send + Sync>;

// --- golden-mcp.ts ---------------------------------------------------------------------------------------------

/// golden-mcp.ts:68.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolRow {
    pub id: String,
    pub ticked: bool,
    pub reason: Option<String>,
}

/// golden-mcp.ts:57.
#[derive(Clone)]
pub struct McpPlan {
    pub agents: Vec<McpAgentPlan>,
    pub guest_home: String,
    pub rewrites: Vec<(String, String)>,
    pub bin_dirs: Vec<String>,
    pub tools: Vec<McpToolRow>,
}

/// golden-mcp.ts:49.
#[derive(Clone)]
pub struct McpAgentPlan {
    pub id: String,
    pub label: String,
    pub scopes: Vec<McpScope>,
    pub aside: Vec<AsideRow>,
}

/// golden-mcp.ts:54 `aside`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsideRow {
    pub id: String,
    pub name: String,
    pub reason: String,
}

/// golden-mcp.ts:36 `drop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropName {
    pub name: String,
    pub reason: String,
    pub keys: Option<Vec<String>>,
}

/// golden-mcp.ts:33 `project`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMove {
    pub from: String,
    pub to: String,
}

/// golden-mcp.ts:40 `own`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnFile {
    pub files: Vec<String>,
    pub base: String,
}

/// golden-mcp.ts:46 `edit`.
pub type ScopeEdit = Arc<dyn Fn(Option<&str>) -> String + Send + Sync>;

/// golden-mcp.ts:28.
#[derive(Clone)]
pub struct McpScope {
    pub files: Vec<String>,
    pub format: McpFormat,
    pub project: Option<ProjectMove>,
    pub keep: Vec<String>,
    pub drop: Vec<DropName>,
    pub travelled: Option<String>,
    pub own: Option<OwnFile>,
    pub folder: Option<String>,
    pub trust: Option<String>,
    pub edit: Option<ScopeEdit>,
}

/// golden-mcp.ts:214.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeFile {
    pub path: String,
    pub text: String,
    pub sum: String,
}

/// golden-mcp.ts:227 `ScopeRead = ScopeFile | ScopeUnread`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeRead {
    File(ScopeFile),
    Unread { path: String, unread: String },
}

/// golden-mcp.ts:268. `outcome` is a merge outcome (`added`, `replaced`, `same`, `theirs`, `missing`, `dropped`,
/// `left`) or an edit outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditedName {
    pub name: String,
    pub outcome: MergeOutcome,
    pub command: Option<String>,
}

/// golden-mcp.ts:276. `file: None` is the TS's `null`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScopeOutcome {
    pub file: Option<String>,
    pub error: Option<String>,
    pub unread: Option<String>,
    pub results: Vec<EditedName>,
}

/// golden-mcp.ts McpResult, the fields provisionMcp reads off mcpRows. `outcome` is `installed`,
/// `fetched-on-first-use` or `skipped`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpResult {
    pub id: String,
    pub agent: String,
    pub name: String,
    pub outcome: String,
    pub note: Option<String>,
    pub keys: Option<Vec<String>>,
}

/// golden-mcp.ts:161.
pub const READ_MS: u64 = 120_000;

/// golden-mcp.ts:103 `scopeRowId`.
pub fn scope_row_id(agent: &str, project: Option<&ProjectMove>, folder: Option<&str>, name: &str) -> String {
    todo!()
}

/// golden-mcp.ts:107 `parseMcpId`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpId {
    pub agent: String,
    pub home: bool,
    pub folder: Option<String>,
    pub name: String,
}

/// golden-mcp.ts:107 `parseMcpId`.
pub fn parse_mcp_id(id: &str) -> Option<McpId> {
    todo!()
}

/// golden-mcp.ts:167 `refused`: a call the machine never ran as exit -1 with the error's words on stderr.
pub fn refused(message: &str) -> ExecResult {
    todo!()
}

/// golden-mcp.ts:175 `readFailed`.
pub fn read_failed(res: &ExecResult) -> String {
    todo!()
}

/// One scope as the read is asked for it (golden-mcp.ts:186).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadAsk {
    pub files: Vec<String>,
    pub base: String,
}

/// golden-mcp.ts:186 `readConfigsCmd`.
pub fn read_configs_cmd(scopes: &[ReadAsk]) -> String {
    todo!()
}

/// golden-mcp.ts:235 `parseConfigs`: one entry per scope; `None` when no line carried the mark.
pub fn parse_configs(stdout: &str, scopes: &[ReadAsk]) -> Option<Vec<Option<ScopeRead>>> {
    todo!()
}

/// golden-mcp.ts:257 `rewriteString`.
pub fn rewrite_string(plan: &McpPlan, s: &str, command: bool) -> String {
    todo!()
}

/// golden-mcp.ts:317 `landConfigs(machine, base, read, texts)`: the words when a config did not land.
pub async fn land_configs(
    machine: &dyn Machine,
    base: &(dyn Fn(&str) -> String + Send + Sync),
    read: &[Option<ScopeRead>],
    texts: &IndexMap<String, String>,
) -> Option<String> {
    todo!()
}

/// golden-mcp.ts:427 `absentCommands`.
pub async fn absent_commands(machine: &dyn Machine, plan: &McpPlan, read: &[ScopeOutcome], path: &str) -> Vec<String> {
    todo!()
}

/// golden-mcp.ts:441 `withoutAbsent`.
pub fn without_absent(
    plan: &McpPlan,
    agents: &[McpAgentPlan],
    read: &[ScopeOutcome],
    missing: &IndexSet<String>,
    tools: &[ToolResult],
) -> Vec<McpAgentPlan> {
    todo!()
}

/// golden-mcp.ts:459 `mcpOpening`.
pub fn mcp_opening(agents: &[McpAgentPlan]) -> String {
    todo!()
}

/// golden-mcp.ts:468 `mcpRows`'s options.
#[derive(Clone)]
pub struct McpRowsOptions {
    pub agents: Vec<McpAgentPlan>,
    pub report: Option<Vec<ScopeOutcome>>,
    pub failure: Option<String>,
    pub missing: IndexSet<String>,
    pub present: IndexSet<String>,
    pub stage: StageListener,
}

/// golden-mcp.ts:468 `mcpRows`.
pub async fn mcp_rows(machine: &dyn Machine, plan: &McpPlan, o: McpRowsOptions) -> Vec<McpResult> {
    todo!()
}
