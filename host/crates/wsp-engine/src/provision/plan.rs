// SPDX-License-Identifier: AGPL-3.0-only
//! provision.ts:21-250 at a4033dc34: the plan off an import, and what a computer already satisfies. Spec:
//! spec/provision_plan.md.
use std::collections::BTreeSet;
use std::sync::Arc;

use indexmap::IndexMap;

use wsp_wire::views::place::PlaceProvisionRow;

use crate::golden_seam::{GoldenImport, McpPlan, PackFiles, ToolInstall, ToolResult};
use crate::machine::Machine;
use crate::provision_files::ProvisionLanding;

/// One files round of a plan: where each path lands and the archive read off this computer.
#[derive(Clone)]
pub struct PlanFiles {
    pub lands: Vec<ProvisionLanding>,
    pub pack: PackFiles,
}

/// provision.ts:57 one plugin: `asked` reads off what a failed install printed why it is set aside.
#[derive(Clone)]
pub struct PlanPlugin {
    pub id: String,
    pub label: String,
    pub cmd: String,
    pub asked: Option<Arc<dyn Fn(&str) -> Option<String> + Send + Sync>>,
}

/// provision.ts:34 a row set aside before anything ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSkipped {
    pub id: String,
    pub label: String,
    pub note: String,
}

/// provision.ts:24 `ProvisionPlan`. Every `Option` is a TS optional field.
#[derive(Clone, Default)]
pub struct ProvisionPlan {
    pub recipe_at: String,
    pub path: String,
    pub prefix: Option<String>,
    pub steps: Vec<ToolInstall>,
    pub skipped: Vec<PlanSkipped>,
    pub files: Option<PlanFiles>,
    pub mcp: Option<McpPlan>,
    /// How many of `steps` are the agents' own installs, the node step among them.
    pub agents: usize,
    pub compiler: bool,
    pub skills: Option<PlanFiles>,
    pub configs: Option<PlanFiles>,
    pub config_tools: Option<Vec<ToolInstall>>,
    pub github: Option<Vec<ToolInstall>>,
    pub server_tools: Option<Vec<String>>,
    pub plugins: Option<Vec<PlanPlugin>>,
}

/// provision.ts:62 `ProvisionExtras`: `compiler` and the optional skills, configs, configTools, plugins, github,
/// spread over the plan after `agents` (so an extra named here overwrites nothing the import set but these).
#[derive(Clone, Default)]
pub struct ProvisionExtras {
    pub compiler: bool,
    pub skills: Option<PlanFiles>,
    pub configs: Option<PlanFiles>,
    pub config_tools: Option<Vec<ToolInstall>>,
    pub plugins: Option<Vec<PlanPlugin>>,
    pub github: Option<Vec<ToolInstall>>,
}

/// The row under way: `{ label, index, of }` (provision.ts:65).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageAt {
    pub label: String,
    pub index: usize,
    pub of: usize,
}

/// provision.ts:65 `ProvisionStage`: a line, the row under way, and a row's outcome the moment it has one.
pub type ProvisionStage = Arc<dyn Fn(&str, Option<&StageAt>, Option<&PlaceProvisionRow>) + Send + Sync>;

/// provision.ts:70 `provisionPlanOf(imp, recipeAt, path, prefix, extras = { compiler: true })`. `extras: None` is
/// the TS default `{ compiler: true }`.
pub fn provision_plan_of(
    imp: &GoldenImport,
    recipe_at: &str,
    path: &str,
    prefix: Option<&str>,
    extras: Option<ProvisionExtras>,
) -> ProvisionPlan {
    todo!()
}

/// provision.ts:93 `aptNeedSteps`: per agent step in order, its catalog apt rows (each `after` that agent's step,
/// dropped where the road answers a note) then its setup rows.
pub fn apt_need_steps(agents: &[ToolInstall], path: &str, prefix: Option<&str>) -> Vec<ToolInstall> {
    todo!()
}

/// provision.ts:110 `onceDests`.
pub fn once_dests(imp: &GoldenImport) -> BTreeSet<String> {
    todo!()
}

/// provision.ts:119 `oncePerDest`: the first landing per dest, in order.
pub fn once_per_dest(lands: &[ProvisionLanding]) -> Vec<ProvisionLanding> {
    todo!()
}

/// provision.ts:122 `provisionCountsOf`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvisionCounts {
    pub tool: usize,
    pub file: usize,
    pub server: usize,
}

/// provision.ts:122 `provisionCountsOf`.
pub fn provision_counts_of(plan: &ProvisionPlan) -> ProvisionCounts {
    todo!()
}

/// provision.ts:132.
pub const PRESENT: &str = "wsp-present";

/// provision.ts:149 `presenceTests(step, versions = "asked")`'s `versions`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Versions {
    #[default]
    Asked,
    Any,
}

/// provision.ts:149 `presenceTests`. Byte for byte the TS's texts, in this order: own read, command, version.
pub fn presence_tests(step: &ToolInstall, versions: Versions) -> Vec<String> {
    todo!()
}

/// provision.ts:168 `PresentRead`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PresentRead {
    pub path: Option<String>,
}

/// provision.ts:177 `presentSteps(machine, steps, path = TOOLS_PATH, prefix, versions = "asked")`: step id to its
/// read, in the order of `steps`. Rejects only where `paged_reads` rejects.
pub async fn present_steps(
    machine: &dyn Machine,
    steps: &[ToolInstall],
    path: &str,
    prefix: Option<&str>,
    versions: Versions,
) -> crate::errors::Result<IndexMap<String, PresentRead>> {
    todo!()
}

/// provision.ts:207 `presentByWhatWaits`: the ids, in step order, of steps with no presence test that something
/// waits on and whose every waiter is in `present`.
pub fn present_by_what_waits(steps: &[ToolInstall], present: &BTreeSet<String>) -> Vec<String> {
    todo!()
}

/// provision.ts:221 `presentElsewhere`.
pub fn present_elsewhere(step: &ToolInstall, read: Option<&PresentRead>) -> Option<String> {
    todo!()
}

/// provision.ts:232 `rowOf`.
pub fn row_of(result: &ToolResult, present: &IndexMap<String, PresentRead>, step: Option<&ToolInstall>) -> PlaceProvisionRow {
    todo!()
}

/// provision.ts:246 `rowLine`: `<label>: <outcome>` and ` (<note>)` where the row has one.
pub fn row_line(row: &PlaceProvisionRow) -> String {
    todo!()
}
