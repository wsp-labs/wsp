// SPDX-License-Identifier: AGPL-3.0-only
//! The golden items provisioning runs on, live code whatever #2441 rules about the image machinery: golden-base.ts,
//! the install half of golden-tools.ts, the planning of tools and agents in golden-import.ts, all of golden-mcp.ts,
//! and the shapes of golden.ts the other slices read. Snapshotting, sealing and image records are not here.
//!
//! The shared types live in this file; each unit's functions live in its own file under `golden_seam/` and are
//! re-exported here, so every caller keeps `crate::golden_seam::<name>`. Units and specs (spec/golden-<unit>.md):
//! tools_disk, tools_loop, base, import_roads, import_plan, import_agents, mcp_files, mcp_rows.
use std::collections::BTreeSet;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};

pub use wsp_wire::views::golden_image::{GoldenBaseTool, GoldenStage};

mod base;
mod import_agents;
mod import_plan;
mod import_roads;
mod mcp_files;
mod mcp_rows;
mod tools_disk;
mod tools_loop;

pub use base::*;
pub use import_agents::*;
pub use import_plan::*;
pub use import_roads::*;
pub use mcp_files::*;
pub use mcp_rows::*;
pub use tools_disk::*;
pub use tools_loop::*;

// --- constants the TS printed (oracle/machines.oracle.ts and oracle/golden.oracle.ts), not retyped --------------

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

// --- golden.ts ------------------------------------------------------------------------------------------------

/// golden.ts:547 `ImportResult`, the fields machine-context's `merge_facts` reads and nothing more; every other key
/// of a saved result is ignored on read.
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

/// golden-import.ts:109.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SkippedPath {
    /// The recipe row the path came from.
    #[serde(default)]
    pub id: String,
    pub path: String,
    pub note: String,
}

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
pub type LeaveOut = Arc<dyn Fn(Vec<StagedFile>) -> LocalBoxFuture<'static, crate::errors::Result<Vec<String>>>>;
/// golden.ts:473 `PackFiles`: `pack(leaveOut?)`. A pack that throws is an `Err`.
pub type PackFiles = Arc<dyn Fn(Option<LeaveOut>) -> LocalBoxFuture<'static, crate::errors::Result<PackedFiles>>>;

/// golden.ts:475 `GoldenImport`, the fields provisionPlanOf reads.
#[derive(Clone, Default)]
pub struct GoldenImport {
    pub agents: Vec<AgentInstall>,
    pub node: Option<NodeInstall>,
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
pub type StageListener = Arc<dyn Fn(GoldenStage, Option<&str>)>;

// --- golden-import.ts: the shapes the plan hands the loop ----------------------------------------------------

/// golden-import.ts:686 `PinRead`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinRead {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read: Option<String>,
    pub fixed: bool,
    pub words: String,
}

/// golden-import.ts:671 `below`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Below {
    pub node: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asks: Option<String>,
}

/// golden-import.ts:643 `ToolInstall`. `manager` is the road's name as the TS spells it (`apt`, `brew`, `npm`, ...).
/// Serializes with the TS's keys, absent fields left out, so a fixture compares as JSON.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInstall {
    pub id: String,
    pub label: String,
    pub manager: String,
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub present: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bins: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below: Option<Below>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<PinRead>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// golden-import.ts:1364 `AgentInstall`: an agent's installer with its row id. `road` is the road's name.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstall {
    pub id: String,
    pub name: String,
    pub install: String,
    pub smoke: String,
    pub road: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<PinRead>,
}

/// golden-import.ts:1424 `NodeInstall`: the one Node step for the ticked agents' floors.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInstall {
    pub floor: u32,
    pub version: String,
    /// The agents whose engines asked for it, by name, in recipe order.
    pub agents: Vec<String>,
    pub cmd: String,
}

/// golden-import.ts:726 `SkippedItem`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SkippedItem {
    pub id: String,
    pub note: String,
}

/// golden-import.ts:1446 `AgentsPlan`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsPlan {
    pub installs: Vec<AgentInstall>,
    pub skipped: Vec<SkippedItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeInstall>,
}

// --- golden-tools.ts: what the loop answers -----------------------------------------------------------------

/// golden-tools.ts:27 `ToolRoad`: the road a source install took, off its last `WSP_ROAD` line. `kind` is `release`
/// or `go`; `arch` is `x86_64` or `aarch64`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToolRoad {
    pub kind: String,
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
}

/// golden-tools.ts:15 `ToolResult`. `outcome` is `installed`, `failed` or `skipped`. `ms` is the loop's own clock;
/// fixtures carry only whether it was set.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ToolResult {
    pub id: String,
    pub label: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<ToolRoad>,
}

/// A tools-loop stage line: `(stage, detail)`. The TS's third argument, the step a screen clocks, is the image
/// machinery's and not passed here.
pub type ToolStage = Arc<dyn Fn(GoldenStage, Option<&str>)>;
/// `onTool`: awaited for each row as the loop reaches it.
pub type OnTool = Arc<dyn Fn(ToolResult) -> LocalBoxFuture<'static, ()>>;

/// golden-tools.ts:348 `InstallToolsOptions`.
#[derive(Clone, Default)]
pub struct InstallToolsOptions {
    pub present: BTreeSet<String>,
    /// `caches: "keep"`; false is the TS's `"sweep"` and its absent value.
    pub keep_caches: bool,
    pub floor: Option<u64>,
    pub path: Option<String>,
    pub prefix: Option<String>,
    pub on_tool: Option<OnTool>,
}

// --- golden-mcp.ts: the plan and what a round answers -------------------------------------------------------

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
pub type ScopeEdit = Arc<dyn Fn(Option<&str>) -> String>;

/// golden-mcp.ts:28.
#[derive(Clone)]
pub struct McpScope {
    pub files: Vec<String>,
    pub format: wsp_catalog::mcp::McpFormat,
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

/// golden-mcp.ts:270 `EditedName["outcome"]`: an edit's word (`written`, `missing`, `dropped`) or a merge's (`added`,
/// `replaced`, `same`, `theirs`, `missing`, `dropped`, `left`). Serializes as the TS's word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditedOutcome {
    Written,
    Added,
    Replaced,
    Same,
    Theirs,
    Missing,
    Dropped,
    Left,
}

/// golden-mcp.ts:268.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditedName {
    pub name: String,
    pub outcome: EditedOutcome,
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

/// golden-mcp.ts:83 `McpResult`. `outcome` is `installed`, `fetched-on-first-use` or `skipped`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpResult {
    pub id: String,
    pub agent: String,
    pub name: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
}

// ---- Read by the rest slice (orphans, drop_template, tool_sizes).

/// golden.ts:378. The idle window a builder is created with: six hours.
pub const BUILDER_IDLE_MS: u64 = 6 * 60 * 60_000;

/// golden.ts:291 `templatesOf`, as a test: the backend says it has templates and offers all four template methods.
pub fn has_templates(backend: &dyn crate::machine::MachineBackend) -> bool {
    backend.capabilities().templates && backend.offers().templates
}

/// golden-import.ts:856.
pub const TAP_PREFIX: &str = "tools/brew-tap/";
