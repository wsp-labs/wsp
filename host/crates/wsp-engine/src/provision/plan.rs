// SPDX-License-Identifier: AGPL-3.0-only
//! provision.ts:21-250 at a4033dc34: the plan off an import, and what a computer already satisfies. Spec:
//! spec/provision_plan.md.
use std::collections::BTreeSet;
use std::sync::Arc;

use indexmap::IndexMap;
use wsp_catalog::catalog::{apt_need_rows, node_at_least, setup_need_rows};
use wsp_catalog::road_modules::road_module_named;
use wsp_catalog::roads::{is_road, InstallRoad};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::golden_image::agent_of_row;
use wsp_wire::views::place::{PlaceProvisionRow, PlaceProvisionRowOutcome};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::js::js_trim;
use wsp_words::words::units::present_elsewhere_line;

use crate::exec_detached::put::{markers_of, paged_reads};
use crate::errors::Result;
use crate::golden_seam::{
    agent_steps, path_line, via_road, AgentsPlan, GoldenImport, McpPlan, PackFiles, ToolInstall, ToolResult, ViaRoad,
};
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
    pub asked: Option<Arc<dyn Fn(&str) -> Option<String>>>,
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
pub type ProvisionStage = Arc<dyn Fn(&str, Option<&StageAt>, Option<&PlaceProvisionRow>)>;

/// provision.ts:70 `provisionPlanOf(imp, recipeAt, path, prefix, extras = { compiler: true })`. `extras: None` is
/// the TS default `{ compiler: true }`.
pub fn provision_plan_of(
    imp: &GoldenImport,
    recipe_at: &str,
    path: &str,
    prefix: Option<&str>,
    extras: Option<ProvisionExtras>,
) -> ProvisionPlan {
    let extras = extras.unwrap_or(ProvisionExtras { compiler: true, ..Default::default() });
    let installs = agent_steps(&AgentsPlan { installs: imp.agents.clone(), skipped: Vec::new(), node: imp.node.clone() }, path, prefix);
    let mut agents = installs.clone();
    agents.extend(apt_need_steps(&installs, path, prefix));
    let once = once_dests(imp);
    let files = imp.files.as_ref().filter(|f| !f.lands.is_empty()).map(|f| PlanFiles {
        lands: once_per_dest(&f.lands)
            .into_iter()
            .map(|mut land| {
                if once.contains(&land.dest) {
                    land.once = true;
                }
                land
            })
            .collect(),
        pack: f.pack.clone(),
    });
    let mut steps = agents.clone();
    steps.extend(imp.tools.iter().cloned());
    let skipped = imp
        .skipped_agents
        .iter()
        .map(|a| PlanSkipped { id: a.id.clone(), label: a.label.clone(), note: a.note.clone() })
        .chain(imp.skipped_tools.iter().map(|t| PlanSkipped { id: t.id.clone(), label: t.label.clone(), note: t.note.clone() }))
        .collect();
    ProvisionPlan {
        recipe_at: recipe_at.to_string(),
        path: path.to_string(),
        prefix: prefix.map(str::to_string),
        steps,
        skipped,
        files,
        mcp: imp.mcp.clone(),
        agents: agents.len(),
        compiler: extras.compiler,
        skills: extras.skills,
        configs: extras.configs,
        config_tools: extras.config_tools,
        github: extras.github,
        server_tools: None,
        plugins: extras.plugins,
    }
}

/// provision.ts:93 `aptNeedSteps`: per agent step in order, its catalog apt rows (each `after` that agent's step,
/// dropped where the road answers a note) then its setup rows.
pub fn apt_need_steps(agents: &[ToolInstall], path: &str, prefix: Option<&str>) -> Vec<ToolInstall> {
    agents
        .iter()
        .flat_map(|a| {
            let agent = agent_of_row(&a.id).unwrap_or_default();
            let packages: Vec<ToolInstall> = apt_need_rows(&agent)
                .into_iter()
                .filter_map(|need| {
                    let road = InstallRoad::Apt { packages: vec![need.package.clone()], pin: None };
                    match via_road(&road, &need.command, path, prefix) {
                        ViaRoad::Step { cmd, shown } => Some(ToolInstall {
                            id: need.id,
                            label: need.package,
                            manager: "apt".to_string(),
                            cmd,
                            shown: Some(shown),
                            bin: Some(need.command),
                            after: Some(a.id.clone()),
                            ..Default::default()
                        }),
                        ViaRoad::Note(_) => None,
                    }
                })
                .collect();
            let settles = setup_need_rows(&agent).into_iter().map(|need| ToolInstall {
                id: need.id,
                label: need.label.clone(),
                manager: "apt".to_string(),
                cmd: format!("{}\n{}", path_line(path, prefix), need.cmd),
                shown: Some(need.label),
                check: Some(need.check),
                after: Some(need.after),
                ..Default::default()
            });
            packages.into_iter().chain(settles).collect::<Vec<_>>()
        })
        .collect()
}

/// provision.ts:110 `onceDests`.
pub fn once_dests(imp: &GoldenImport) -> BTreeSet<String> {
    let home = imp.mcp.as_ref().map(|m| m.guest_home.clone());
    let configs = imp
        .mcp
        .iter()
        .flat_map(|m| m.agents.iter())
        .flat_map(|a| a.scopes.iter())
        .flat_map(|s| s.files.iter())
        .filter_map(|f| f.strip_prefix(&format!("{}/", home.as_deref()?)).map(str::to_string));
    let rewritten = imp.recipe_files.iter().filter(|f| f.volatile == Some(true) && agent_of_row(&f.id).is_some()).map(|f| f.dest.clone());
    configs.chain(rewritten).collect()
}

/// provision.ts:119 `oncePerDest`: the first landing per dest, in order.
pub fn once_per_dest(lands: &[ProvisionLanding]) -> Vec<ProvisionLanding> {
    lands.iter().enumerate().filter(|(at, land)| lands.iter().position(|o| o.dest == land.dest) == Some(*at)).map(|(_, land)| land.clone()).collect()
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
    ProvisionCounts {
        tool: plan.steps.len(),
        file: plan.files.as_ref().map_or(0, |f| f.lands.len()),
        server: plan.mcp.as_ref().map_or(0, |m| {
            m.agents.iter().map(|a| a.aside.len() + a.scopes.iter().map(|s| s.keep.len() + s.drop.len()).sum::<usize>()).sum()
        }),
    }
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
    let mut tests: Vec<String> = Vec::new();
    if let Some(own) = step.present.as_ref().or(step.check.as_ref()) {
        tests.push(format!("( {own} ) >/dev/null 2>&1"));
    }
    if let Some(bin) = &step.bin {
        tests.push(format!("command -v {} >/dev/null 2>&1", shell_quote(bin)));
    }
    let version: Option<&str> = step.pin.as_ref().and_then(|p| p.read.as_deref());
    if let Some(version) = version.filter(|_| step.asks.is_some() || tests.is_empty()) {
        let read = format!("\"$( ( {version} ) 2>/dev/null | head -n 1 | tr -d '[:space:]' )\"");
        let agent = agent_of_row(&step.id).is_some();
        let asked = |asks: &str| {
            if agent {
                format!("printf '%s\\n' {} {read} | sort -V -C", shell_quote(asks))
            } else {
                format!("[ {read} = {} ]", shell_quote(asks))
            }
        };
        let is = |asks: Option<&str>| match asks {
            Some(asks) if versions == Versions::Asked => asked(asks),
            _ => format!("[ -n {read} ]"),
        };
        let test = match &step.below {
            Some(below) if versions == Versions::Asked => format!(
                "if {}; then {}; else {}; fi",
                node_at_least(&below.node),
                is(step.asks.as_deref()),
                is(below.asks.as_deref())
            ),
            _ => is(step.asks.as_deref()),
        };
        tests.push(test);
    }
    tests
}

/// What one step's presence read said beyond that the step is there: the path its command answered from, where the
/// read asked for a command at all. Read to tell a row its own road installed from a row of the same name another
/// road put somewhere else.
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
) -> Result<IndexMap<String, PresentRead>> {
    let asked: Vec<(ToolInstall, Vec<String>)> = steps
        .iter()
        .filter_map(|step| {
            let tests = presence_tests(step, versions);
            (!tests.is_empty()).then(|| (step.clone(), tests))
        })
        .collect();
    let pages = paged_reads(
        machine,
        &asked,
        |(step, tests), at| {
            let command = step.bin.as_ref().map(|bin| format!("$(command -v {} 2>/dev/null)", shell_quote(bin))).unwrap_or_default();
            format!("if {}; then printf '{PRESENT} %s %s\\n' {at} \"{command}\"; fi", tests.join(" && "))
        },
        &path_line(path, prefix),
    )
    .await?;
    let mut present: IndexMap<String, PresentRead> = IndexMap::new();
    for page in pages {
        if page.res.exit_code.get() != 0 {
            continue;
        }
        let marked = markers_of(&page.res.stdout, PRESENT);
        for (at, (step, _)) in page.rows.iter().enumerate() {
            let Some(said) = marked.get(&at.to_string()) else {
                continue;
            };
            let said = js_trim(said);
            let read = if said.is_empty() { PresentRead::default() } else { PresentRead { path: Some(said.to_string()) } };
            present.insert(step.id.clone(), read);
        }
    }
    Ok(present)
}

/// provision.ts:207 `presentByWhatWaits`: the ids, in step order, of steps with no presence test that at least one step
/// waits on (`after == id`) and all of whose waiters are in `present`. The apt index of a real plan is the case.
pub fn present_by_what_waits(steps: &[ToolInstall], present: &BTreeSet<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for step in steps {
        if !presence_tests(step, Versions::Asked).is_empty() {
            continue;
        }
        let waiting: Vec<&ToolInstall> = steps.iter().filter(|s| s.after.as_deref() == Some(step.id.as_str())).collect();
        if !waiting.is_empty() && waiting.iter().all(|s| present.contains(&s.id)) && !out.contains(&step.id) {
            out.push(step.id.clone());
        }
    }
    out
}

/// provision.ts:221 `presentElsewhere`.
pub fn present_elsewhere(step: &ToolInstall, read: Option<&PresentRead>) -> Option<String> {
    let bins = step.bins.as_deref().unwrap_or(&[]);
    let path = read?.path.as_deref()?;
    let bin = step.bin.as_deref()?;
    if bins.is_empty() || bins.iter().any(|dir| path.starts_with(&format!("{dir}/"))) {
        return None;
    }
    let road = is_road(Some(&step.manager))?;
    Some(present_elsewhere_line(bin, path, &road_module_named(road).words(), bins))
}

fn outcome_of(word: &str) -> PlaceProvisionRowOutcome {
    match word {
        "installed" => PlaceProvisionRowOutcome::Installed,
        "present" => PlaceProvisionRowOutcome::Present,
        "skipped" => PlaceProvisionRowOutcome::Skipped,
        _ => PlaceProvisionRowOutcome::Failed,
    }
}

fn outcome_word(outcome: PlaceProvisionRowOutcome) -> &'static str {
    match outcome {
        PlaceProvisionRowOutcome::Installed => "installed",
        PlaceProvisionRowOutcome::Present => "present",
        PlaceProvisionRowOutcome::Failed => "failed",
        PlaceProvisionRowOutcome::Skipped => "skipped",
    }
}

/// provision.ts:232 `rowOf`.
pub fn row_of(result: &ToolResult, present: &IndexMap<String, PresentRead>, step: Option<&ToolInstall>) -> PlaceProvisionRow {
    let read = present.get(&result.id);
    let outcome = if result.outcome == "installed" && read.is_some() { "present" } else { result.outcome.as_str() };
    let note = if outcome == "present" { step.and_then(|s| present_elsewhere(s, read)) } else { result.note.clone() };
    PlaceProvisionRow {
        ms: result.ms.map(JsInt::from),
        ..row(&result.id, &result.label, outcome_of(outcome), note)
    }
}

/// A row with no kind, step, fix, keys, ms, earlier or project yet.
pub(super) fn row(id: &str, label: &str, outcome: PlaceProvisionRowOutcome, note: Option<String>) -> PlaceProvisionRow {
    PlaceProvisionRow {
        id: id.to_string(),
        label: label.to_string(),
        outcome,
        kind: None,
        step: None,
        note,
        fix: None,
        keys: None,
        ms: None,
        earlier: None,
        project: None,
    }
}

/// provision.ts:246 `rowLine`: `<label>: <outcome word>` and ` (<note>)` where the row has one.
pub fn row_line(row: &PlaceProvisionRow) -> String {
    let note = row.note.as_ref().map_or(String::new(), |note| format!(" ({note})"));
    format!("{}: {}{note}", row.label, outcome_word(row.outcome))
}
