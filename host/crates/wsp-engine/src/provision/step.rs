// SPDX-License-Identifier: AGPL-3.0-only
//! provision.ts:248-551 at a4033dc34: one step of a setup on the computer itself. Spec: spec/provision_step.md.
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use indexmap::{IndexMap, IndexSet};

use wsp_wire::views::place::{PlaceProvisionRow, PlaceSetupStep};

use super::plan::{PlanFiles, ProvisionPlan, ProvisionStage};
use crate::errors::Result;
use crate::golden_seam::{McpPlan, SkippedPath, ToolInstall, ToolResult};
use crate::machine::Machine;
use crate::provision_files::OwnedPaths;

/// provision.ts:249-252.
pub const FILES_LABEL: &str = "your agents' files";
pub const SKILLS_LABEL: &str = "your skills";
pub const CONFIGS_LABEL: &str = "your configs";
pub const MCP_LABEL: &str = "MCP servers";

/// provision.ts:254 `ProvisionOn`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProvisionOn {
    pub home: String,
    pub held: Option<IndexSet<String>>,
    pub stores: Option<IndexMap<String, String>>,
}

/// provision.ts:266 `EngineStep`, in the TS's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineStep {
    Floor,
    Agents,
    Clis,
    Mcp,
    Skills,
    Plugins,
    Configs,
    Github,
    Context,
}

impl EngineStep {
    /// The step's own word (`floor`, `agents`, ...), as the outside scripts and the rows spell it.
    pub fn word(self) -> &'static str {
        todo!()
    }
    /// The step as a row's `step` field.
    pub fn setup_step(self) -> PlaceSetupStep {
        todo!()
    }
}

/// provision.ts:271 `SetupRun`: what one run carries from step to step.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SetupRun {
    pub base: Vec<ToolResult>,
    pub tools: Vec<ToolResult>,
    pub landed: OwnedPaths,
    pub skipped_files: Vec<SkippedPath>,
}

/// The run a step reads and writes while other steps may run beside it.
pub type SharedRun = Arc<Mutex<SetupRun>>;

/// provision.ts:278 `newSetupRun`.
pub fn new_setup_run() -> SharedRun {
    todo!()
}

/// provision.ts:281 `leftOff`: `{ COMPILER_ROW }` unless the plan needs the compiler.
pub fn left_off(compiler: bool) -> BTreeSet<String> {
    todo!()
}

/// provision.ts:367 `projectServersStep`.
pub async fn project_servers_step(
    machine: &dyn Machine,
    mcp: &McpPlan,
    path: &str,
    stage: ProvisionStage,
    on: &ProvisionOn,
) -> Vec<PlaceProvisionRow> {
    todo!()
}

/// provision.ts:386 `provisionStep`: the step's rows, each with `step`. Fails only when the computer stopped
/// answering.
pub async fn provision_step(
    machine: &dyn Machine,
    plan: &ProvisionPlan,
    step: EngineStep,
    run: SharedRun,
    stage: ProvisionStage,
    on: &ProvisionOn,
) -> Result<Vec<PlaceProvisionRow>> {
    todo!()
}

/// provision.ts:401 `stepInstalls`.
pub fn step_installs(plan: &ProvisionPlan, step: EngineStep) -> Vec<ToolInstall> {
    todo!()
}

/// provision.ts:422 `outsideRoots`: sorted, unique, none inside another, none under `/opt/wsp`.
pub fn outside_roots(plan: &ProvisionPlan, step: EngineStep) -> Vec<String> {
    todo!()
}

/// provision.ts:429.
pub const OUTSIDE_TOPS: [&str; 2] = ["/usr/local", "/opt"];
/// provision.ts:434.
pub const OUTSIDE_MS: u64 = 120_000;
/// provision.ts:528.
pub const HOOK_MS: u64 = 60_000;
/// provision.ts:548.
pub const PLUGIN_MS: u64 = 300_000;

/// provision.ts:430 `within`.
pub fn within(path: &str, dir: &str) -> bool {
    todo!()
}

/// provision.ts:531 `hookOf`: the hook a CLI row's catalog tool entry names.
pub fn hook_of(row_id: &str) -> Option<(String, String)> {
    todo!()
}

/// provision.ts:551 `lastWords`: the last line of the trimmed text, its first 300 UTF-16 units; `None` for empty.
pub fn last_words(text: &str) -> Option<String> {
    todo!()
}

/// One files round's argument (provision.ts:354).
pub type FilesRound<'a> = (&'a PlanFiles, &'a str);
