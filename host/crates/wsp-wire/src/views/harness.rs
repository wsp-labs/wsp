// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/harness.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::thread_defaults::AccessChoice;
use crate::views::workspace::WorkspaceOut;
use indexmap::IndexMap;
use wsp_frames::True;

/// TS: packages/protocol/src/views/harness.ts:13 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessOption {
    pub value: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
}

impl Check for HarnessOption {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/harness.ts:26 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessModel {
    pub value: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efforts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_windows: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added: Option<True>,
}

impl Check for HarnessModel {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/harness.ts:39 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HarnessCatalogSource {
    #[serde(rename = "harness")]
    Harness,
    #[serde(rename = "table")]
    Table,
}

impl Check for HarnessCatalogSource {}

/// TS: packages/protocol/src/views/harness.ts:44 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScreenControl {
    #[serde(rename = "sign-in")]
    SignIn,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "access")]
    Access,
    #[serde(rename = "settings")]
    Settings,
    #[serde(rename = "docs")]
    Docs,
}

impl Check for ScreenControl {}

/// TS: packages/protocol/src/views/harness.ts:48 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenCommand {
    pub name: String,
    pub control: ScreenControl,
}

impl Check for ScreenCommand {
    /// Nested checks: control.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessCatalogUnshaped {
    pub models: Vec<HarnessModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_models: Option<Vec<HarnessModel>>,
    pub efforts: Vec<HarnessOption>,
    pub permission_modes: Vec<HarnessOption>,
}

impl Check for HarnessCatalogUnshaped {
    /// Nested checks: models, legacyModels, efforts, permissionModes.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/harness.ts:55 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessCatalog {
    pub harness: String,
    pub label: String,
    pub source: HarnessCatalogSource,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub version: Option<String>,
    pub models: Vec<HarnessModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_models: Option<Vec<HarnessModel>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_listed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_models: Option<Vec<HarnessModel>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unshaped: Option<HarnessCatalogUnshaped>,
    pub efforts: Vec<HarnessOption>,
    pub context_windows: Vec<HarnessOption>,
    pub permission_modes: Vec<HarnessOption>,
    pub steers: bool,
    pub renames: bool,
    pub images: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_commands: Option<Vec<ScreenCommand>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moves_access: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steers_images: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asides: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compacts: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_resume: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewinds_conversation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewinds_by_count: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forks_by_count: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypass_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only_tools_free: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<IndexMap<AccessChoice, String>>,
}

impl Check for HarnessCatalog {
    /// Nested checks: source, models, legacyModels, hiddenModels, unshaped, efforts, contextWindows, permissionModes, screenCommands, access.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/harness.ts:152 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResult {
    pub workspace: WorkspaceOut,
    pub thread_id: String,
    pub session_id: String,
}

impl Check for StartResult {
    /// Nested checks: workspace.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/harness.ts:155 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPostResult {
    pub url: String,
    pub number: i64,
    pub comments: i64,
    pub folded: i64,
}

impl Check for ReviewPostResult {
    /// Rules: number: >= 0; comments: >= 0; folded: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// ---- Hand-written below: the functions of the unit.

// views/harness.ts:161 at a3b870a6e
// export interface McpServerSpec {
//   command: string;
//   args: readonly string[];
//   /** The wsp server of a thread another thread started, which has no slate: the launch says nothing of one. Read by
//    * the adapter that builds the launch and never put on its line, which a box's older wsp would refuse. */
//   noSlate?: true;
// }
/// An MCP server as every agent's config names it and as a launch may carry it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSpec {
    pub command: String,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_slate: Option<wsp_frames::True>,
}

// views/harness.ts:172 at a3b870a6e
// export function noMcpServersLine(harness: string): string {
//   return `${harness} takes no MCP server with a launch, so its thread would run without them; open the thread on an agent that takes them`;
// }
pub fn no_mcp_servers_line(harness: &str) -> String {
    todo!()
}

// views/harness.ts:179 at a3b870a6e
// export function mcpServersBlocked(servers: Readonly<Record<string, McpServerSpec>> | undefined, takes: true | undefined, harness: string): string | null {
//   if (servers === undefined || Object.keys(servers).length === 0) return null;
//   return takes === true ? null : noMcpServersLine(harness);
// }
/// None where no server is named (absent or empty) or the harness takes them; else `no_mcp_servers_line`.
pub fn mcp_servers_blocked(servers: Option<&indexmap::IndexMap<String, McpServerSpec>>, takes: bool, harness: &str) -> Option<String> {
    todo!()
}

// views/harness.ts:187 at a3b870a6e
// export function keepsRename(catalog: HarnessCatalog | null | undefined): boolean {
//   return catalog === null || catalog === undefined || catalog.source === "table" || catalog.renames;
// }
pub fn keeps_rename(catalog: Option<&HarnessCatalog>) -> bool {
    todo!()
}

// views/harness.ts:194 at a3b870a6e
// export function readsImages(catalog: HarnessCatalog | null | undefined): boolean {
//   return catalog === null || catalog === undefined || catalog.source === "table" || catalog.images;
// }
pub fn reads_images(catalog: Option<&HarnessCatalog>) -> bool {
    todo!()
}

// views/harness.ts:201 at a3b870a6e
// export function takesMcpServers(catalog: Pick<HarnessCatalog, "mcpServers"> | null | undefined): boolean {
//   return catalog?.mcpServers === true;
// }
pub fn takes_mcp_servers(catalog: Option<&HarnessCatalog>) -> bool {
    todo!()
}

// views/harness.ts:240 at a3b870a6e
// export interface StartPicks {
//   model?: string;
//   effort?: string;
//   permissionMode?: string;
//   fast?: boolean;
// }
/// The picks a start names, as sessions.start carries them; keys in the order `start_picks` writes them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPicks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
}

/// Why `start_picks` refused: wire's `listed` refusal (kind `usage`, its message word for word), or a fast asked of a
/// model with no fast mode (kind `invalid`, `no_fast_line` of that model's label).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickRefusal {
    pub message: String,
    pub kind: &'static str,
}

// views/harness.ts:248 at a3b870a6e
// export const noFastLine = (model: string): string => `${model} has no fast mode; pick a model that offers it, or send without it`;
pub fn no_fast_line(model: &str) -> String {
    todo!()
}

// views/harness.ts:250 at a3b870a6e (private)
// function checkedAgainst(catalog: HarnessCatalog, picks: StartPicks, model: string | undefined, runsOn: string | undefined): void {
//   if (catalog.models.length > 0) listed(catalog.harness, "model", catalog.models, picks.model, catalog.legacyModels, catalog.hiddenModels);
//   const chosen = modelOf(catalog, model);
//   if (catalog.efforts.length > 0) listed(chosen?.efforts !== undefined ? chosen.label : catalog.harness, "effort", effortsFor(catalog, chosen), picks.effort);
//   if (catalog.permissionModes.length > 0) listed(catalog.harness, "access mode", catalog.permissionModes, picks.permissionMode);
//   const fastOn = modelOf(catalog, model ?? runsOn);
//   if (picks.fast === true && fastOn !== null && fastOn.fast !== true) throw Object.assign(new Error(noFastLine(fastOn.label)), { kind: "invalid" });
// }
// views/harness.ts:266 at a3b870a6e
// export function startPicks(catalog: HarnessCatalog | undefined, picks: StartPicks, opensThread: boolean, runsOn?: string): StartPicks {
//   const model = picks.model ?? (opensThread && catalog !== undefined ? markedDefault(everyModel(catalog))?.value : undefined);
//   if (catalog !== undefined) checkedAgainst(catalog, picks, model, runsOn);
//   const effort = picks.effort ?? (opensThread && catalog !== undefined ? markedDefault(effortsFor(catalog, modelOf(catalog, model)))?.value : undefined);
//   // The access is filled in like the other two, so what the picker shows is what the CLI is told: an unnamed access
//   // used to reach the adapter as nothing, which every adapter here reads as its own skip-everything flag. The mark
//   // is the one markedFor placed from the person's defaults, else the table's own.
//   const permissionMode = picks.permissionMode ?? (opensThread && catalog !== undefined ? markedDefault(catalog.permissionModes)?.value : undefined);
//   return {
//     ...(model !== undefined ? { model } : {}),
//     ...(effort !== undefined ? { effort } : {}),
//     ...(permissionMode !== undefined ? { permissionMode } : {}),
//     ...(picks.fast === true ? { fast: true } : {}),
//   };
// }
/// The picks a start runs with, checked against the catalog (the TS's private `checkedAgainst`, through
/// `crate::wire::helpers::listed`) and, for a start that opens a thread, filled from the marked defaults.
pub fn start_picks(
    catalog: Option<&HarnessCatalog>,
    picks: &StartPicks,
    opens_thread: bool,
    runs_on: Option<&str>,
) -> Result<StartPicks, PickRefusal> {
    todo!()
}
