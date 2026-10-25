// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/harness.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::harness_picks::{efforts_for, every_model, marked_default, model_of};
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
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "value" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "description" => <Option<String> as Check>::describe(rest),
                "isDefault" => <Option<bool> as Check>::describe(rest),
                "short" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
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
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "value" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "description" => <Option<String> as Check>::describe(rest),
                "isDefault" => <Option<bool> as Check>::describe(rest),
                "short" => <Option<String> as Check>::describe(rest),
                "efforts" => <Option<Vec<String>> as Check>::describe(rest),
                "defaultEffort" => <Option<String> as Check>::describe(rest),
                "contextWindows" => <Option<Vec<String>> as Check>::describe(rest),
                "fast" => <Option<bool> as Check>::describe(rest),
                "added" => None,
                _ => None,
            },
        }
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

impl Check for HarnessCatalogSource {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["harness", "table"]))
    }
}

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

impl Check for ScreenControl {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["sign-in", "model", "access", "settings", "docs"]))
    }
}

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
        self.control.check().map_err(|e| e.under("control"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "control" => <ScreenControl as Check>::describe(rest),
                _ => None,
            },
        }
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
        self.models.check().map_err(|e| e.under("models"))?;
        self.legacy_models.check().map_err(|e| e.under("legacyModels"))?;
        self.efforts.check().map_err(|e| e.under("efforts"))?;
        self.permission_modes.check().map_err(|e| e.under("permissionModes"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "models" => <Vec<HarnessModel> as Check>::describe(rest),
                "legacyModels" => <Option<Vec<HarnessModel>> as Check>::describe(rest),
                "efforts" => <Vec<HarnessOption> as Check>::describe(rest),
                "permissionModes" => <Vec<HarnessOption> as Check>::describe(rest),
                _ => None,
            },
        }
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
        self.source.check().map_err(|e| e.under("source"))?;
        self.models.check().map_err(|e| e.under("models"))?;
        self.legacy_models.check().map_err(|e| e.under("legacyModels"))?;
        self.hidden_models.check().map_err(|e| e.under("hiddenModels"))?;
        self.unshaped.check().map_err(|e| e.under("unshaped"))?;
        self.efforts.check().map_err(|e| e.under("efforts"))?;
        self.context_windows.check().map_err(|e| e.under("contextWindows"))?;
        self.permission_modes.check().map_err(|e| e.under("permissionModes"))?;
        self.screen_commands.check().map_err(|e| e.under("screenCommands"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "harness" => <String as Check>::describe(rest),
                "label" => <String as Check>::describe(rest),
                "source" => <HarnessCatalogSource as Check>::describe(rest),
                "version" => <Option<String> as Check>::describe(rest),
                "models" => <Vec<HarnessModel> as Check>::describe(rest),
                "legacyModels" => <Option<Vec<HarnessModel>> as Check>::describe(rest),
                "legacyListed" => <Option<bool> as Check>::describe(rest),
                "hiddenModels" => <Option<Vec<HarnessModel>> as Check>::describe(rest),
                "unshaped" => <Option<HarnessCatalogUnshaped> as Check>::describe(rest),
                "efforts" => <Vec<HarnessOption> as Check>::describe(rest),
                "contextWindows" => <Vec<HarnessOption> as Check>::describe(rest),
                "permissionModes" => <Vec<HarnessOption> as Check>::describe(rest),
                "steers" => <bool as Check>::describe(rest),
                "renames" => <bool as Check>::describe(rest),
                "images" => <bool as Check>::describe(rest),
                "mcpServers" => <Option<bool> as Check>::describe(rest),
                "screenCommands" => <Option<Vec<ScreenCommand>> as Check>::describe(rest),
                "movesAccess" => <Option<bool> as Check>::describe(rest),
                "steersImages" => <Option<bool> as Check>::describe(rest),
                "asides" => <Option<bool> as Check>::describe(rest),
                "compacts" => <Option<String> as Check>::describe(rest),
                "terminalResume" => <Option<String> as Check>::describe(rest),
                "rewindsConversation" => <Option<bool> as Check>::describe(rest),
                "rewindsByCount" => <Option<bool> as Check>::describe(rest),
                "forks" => <Option<bool> as Check>::describe(rest),
                "forksByCount" => <Option<bool> as Check>::describe(rest),
                "isDefault" => <Option<bool> as Check>::describe(rest),
                "refusal" => <Option<String> as Check>::describe(rest),
                "smallModel" => <Option<String> as Check>::describe(rest),
                "bypassMode" => <Option<String> as Check>::describe(rest),
                "readOnlyMode" => <Option<String> as Check>::describe(rest),
                "readOnlyToolsFree" => <Option<bool> as Check>::describe(rest),
                "access" => <Option<IndexMap<AccessChoice, String>> as Check>::describe(rest),
                _ => None,
            },
        }
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
        self.workspace.check().map_err(|e| e.under("workspace"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspace" => <WorkspaceOut as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "sessionId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
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
        crate::types::check::num_at_least(self.number as f64, 0.0).map_err(|e| e.under("number"))?;
        crate::types::check::num_at_least(self.comments as f64, 0.0).map_err(|e| e.under("comments"))?;
        crate::types::check::num_at_least(self.folded as f64, 0.0).map_err(|e| e.under("folded"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "url" => <String as Check>::describe(rest),
                "number" => <i64 as Check>::describe(rest),
                "comments" => <i64 as Check>::describe(rest),
                "folded" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
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
    format!("{harness} takes no MCP server with a launch, so its thread would run without them; open the thread on an agent that takes them")
}

// views/harness.ts:179 at a3b870a6e
// export function mcpServersBlocked(servers: Readonly<Record<string, McpServerSpec>> | undefined, takes: true | undefined, harness: string): string | null {
//   if (servers === undefined || Object.keys(servers).length === 0) return null;
//   return takes === true ? null : noMcpServersLine(harness);
// }
/// None where no server is named (absent or empty) or the harness takes them; else `no_mcp_servers_line`.
pub fn mcp_servers_blocked(servers: Option<&indexmap::IndexMap<String, McpServerSpec>>, takes: bool, harness: &str) -> Option<String> {
    match servers {
        Some(servers) if !servers.is_empty() && !takes => Some(no_mcp_servers_line(harness)),
        _ => None,
    }
}

// views/harness.ts:187 at a3b870a6e
// export function keepsRename(catalog: HarnessCatalog | null | undefined): boolean {
//   return catalog === null || catalog === undefined || catalog.source === "table" || catalog.renames;
// }
pub fn keeps_rename(catalog: Option<&HarnessCatalog>) -> bool {
    catalog.map_or(true, |c| c.source == HarnessCatalogSource::Table || c.renames)
}

// views/harness.ts:194 at a3b870a6e
// export function readsImages(catalog: HarnessCatalog | null | undefined): boolean {
//   return catalog === null || catalog === undefined || catalog.source === "table" || catalog.images;
// }
pub fn reads_images(catalog: Option<&HarnessCatalog>) -> bool {
    catalog.map_or(true, |c| c.source == HarnessCatalogSource::Table || c.images)
}

// views/harness.ts:201 at a3b870a6e
// export function takesMcpServers(catalog: Pick<HarnessCatalog, "mcpServers"> | null | undefined): boolean {
//   return catalog?.mcpServers === true;
// }
pub fn takes_mcp_servers(catalog: Option<&HarnessCatalog>) -> bool {
    catalog.is_some_and(|c| c.mcp_servers == Some(true))
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
    format!("{model} has no fast mode; pick a model that offers it, or send without it")
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
    let opening = catalog.filter(|_| opens_thread);
    let model = picks.model.clone().or_else(|| opening.and_then(|c| marked_default(&every_model(c)).map(|m| m.value.clone())));
    if let Some(catalog) = catalog {
        checked_against(catalog, picks, model.as_deref(), runs_on)?;
    }
    let effort = picks.effort.clone().or_else(|| opening.and_then(|c| marked_default(&efforts_for(c, model_of(c, model.as_deref()).as_ref())).map(|o| o.value.clone())));
    let permission_mode = picks.permission_mode.clone().or_else(|| opening.and_then(|c| marked_default(&c.permission_modes).map(|o| o.value.clone())));
    Ok(StartPicks {
        model,
        effort,
        permission_mode,
        fast: (picks.fast == Some(true)).then_some(true),
    })
}

/// The TS's private `checkedAgainst`: each named pick must be one its list carries, and a fast asked of a model with
/// no fast mode is refused.
fn checked_against(catalog: &HarnessCatalog, picks: &StartPicks, model: Option<&str>, runs_on: Option<&str>) -> Result<(), PickRefusal> {
    if !catalog.models.is_empty() {
        let models: Vec<(&str, &str)> = catalog.models.iter().map(|m| (m.value.as_str(), m.label.as_str())).collect();
        let legacy: Vec<(&str, &str)> = catalog.legacy_models.iter().flatten().map(|m| (m.value.as_str(), m.label.as_str())).collect();
        let hidden: Vec<(&str, &str)> = catalog.hidden_models.iter().flatten().map(|m| (m.value.as_str(), m.label.as_str())).collect();
        listed_refusal(&catalog.harness, "model", &models, picks.model.as_deref(), &legacy, &hidden)?;
    }
    let chosen = model_of(catalog, model);
    if !catalog.efforts.is_empty() {
        let subject = match &chosen {
            Some(m) if m.efforts.is_some() => m.label.as_str(),
            _ => catalog.harness.as_str(),
        };
        let efforts_of_model = efforts_for(catalog, chosen.as_ref());
        let efforts: Vec<(&str, &str)> = efforts_of_model.iter().map(|o| (o.value.as_str(), o.label.as_str())).collect();
        listed_refusal(subject, "effort", &efforts, picks.effort.as_deref(), &[], &[])?;
    }
    if !catalog.permission_modes.is_empty() {
        let modes: Vec<(&str, &str)> = catalog.permission_modes.iter().map(|o| (o.value.as_str(), o.label.as_str())).collect();
        listed_refusal(&catalog.harness, "access mode", &modes, picks.permission_mode.as_deref(), &[], &[])?;
    }
    let fast_on = model_of(catalog, model.or(runs_on));
    if picks.fast == Some(true) {
        if let Some(m) = fast_on.filter(|m| m.fast != Some(true)) {
            return Err(PickRefusal {
                message: no_fast_line(&m.label),
                kind: "invalid",
            });
        }
    }
    Ok(())
}

/// Wire's `listed` (helpers.ts:45), refusing a value none of the options, legacy or hidden ones carries. The helpers
/// unit owns `crate::wire::helpers::listed`; this copy stands in until it lands, and must give the same words.
fn listed_refusal(subject: &str, word: &str, options: &[(&str, &str)], value: Option<&str>, legacy: &[(&str, &str)], hidden: &[(&str, &str)]) -> Result<(), PickRefusal> {
    let Some(value) = value else {
        return Ok(());
    };
    if [options, legacy, hidden].iter().any(|list| list.iter().any(|(v, _)| *v == value)) {
        return Ok(());
    }
    let words = |list: &[(&str, &str)]| list.iter().map(|(v, l)| format!("{l} ({v})")).collect::<Vec<_>>().join(", ");
    let older = if legacy.is_empty() { String::new() } else { format!("; legacy: {}", words(legacy)) };
    let message = if options.is_empty() {
        format!("{subject} takes no {word}")
    } else {
        format!("{word} \"{value}\" is not one {subject} takes; one of: {}{older}", words(options))
    };
    Err(PickRefusal { message, kind: "usage" })
}
