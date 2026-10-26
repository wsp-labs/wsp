// SPDX-License-Identifier: AGPL-3.0-only
//! provision.ts:248-551 at a4033dc34: one step of a setup on the computer itself. Spec: spec/provision_step.md.
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::channel::{mpsc, oneshot};
use futures::future::LocalBoxFuture;
use futures::StreamExt;
use indexmap::{IndexMap, IndexSet};

use wsp_catalog::catalog::{catalog_entry, catalog_id_of_row, CatalogEntry};
use wsp_catalog::road_modules::road_module_named;
use wsp_catalog::roads::is_road;
use wsp_wire::types::daemon_contract::TOOL_PREFIX;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::golden_image::{agent_of_row, GoldenStage, GoldenStep};
use wsp_wire::views::place::{PlaceProvisionRow, PlaceProvisionRowOutcome, PlaceSetupStep};
use wsp_words::js::{js_trim, utf16};
use wsp_words::views::place_setup_words::provision_servers_line;
use wsp_words::words::base::plural;

use super::plan::{
    present_by_what_waits, present_steps, row, row_line, row_of, PresentRead, ProvisionPlan, ProvisionStage, StageAt, Versions,
};
use crate::errors::Result;
use crate::golden_seam::{
    base_installs, disk_use_size, install_base_with, install_tools_at, owned_floor_bytes, path_line, ImportFiles, ImportResult, ImportRow,
    InstallBaseOptions, InstallToolsOptions, McpPlan, OnTool, SkippedPath, ToolInstall, ToolResult, ToolStage,
};
use crate::machine::{ExecOptions, Machine, RunOptions};
use crate::machine_context::{apply_machine_context, ApplyContextOptions, ProbeShells};
use crate::provision_files::{
    close_agent_files, landed_at, merged_into_line, once_paths_of, outside_after_script, outside_before_script, provision_files,
    store_roots, OwnedPaths, ProvisionFilesAsk, ProvisionLanding,
};
use crate::provision_mcp::{headless_keys, provision_mcp, ProvisionMcpOptions};

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
        match self {
            EngineStep::Floor => "floor",
            EngineStep::Agents => "agents",
            EngineStep::Clis => "clis",
            EngineStep::Mcp => "mcp",
            EngineStep::Skills => "skills",
            EngineStep::Plugins => "plugins",
            EngineStep::Configs => "configs",
            EngineStep::Github => "github",
            EngineStep::Context => "context",
        }
    }
    /// The step as a row's `step` field.
    pub fn setup_step(self) -> PlaceSetupStep {
        match self {
            EngineStep::Floor => PlaceSetupStep::Floor,
            EngineStep::Agents => PlaceSetupStep::Agents,
            EngineStep::Clis => PlaceSetupStep::Clis,
            EngineStep::Mcp => PlaceSetupStep::Mcp,
            EngineStep::Skills => PlaceSetupStep::Skills,
            EngineStep::Plugins => PlaceSetupStep::Plugins,
            EngineStep::Configs => PlaceSetupStep::Configs,
            EngineStep::Github => PlaceSetupStep::Github,
            EngineStep::Context => PlaceSetupStep::Context,
        }
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
    Arc::new(Mutex::new(SetupRun::default()))
}

/// provision.ts:281 `leftOff`: `{ COMPILER_ROW }` unless the plan needs the compiler.
pub fn left_off(compiler: bool) -> BTreeSet<String> {
    if compiler {
        BTreeSet::new()
    } else {
        BTreeSet::from([wsp_catalog::catalog::COMPILER_ROW.to_string()])
    }
}

/// provision.ts:367 `projectServersStep`.
pub async fn project_servers_step(
    machine: &dyn Machine,
    mcp: &McpPlan,
    path: &str,
    stage: ProvisionStage,
    on: &ProvisionOn,
) -> Vec<PlaceProvisionRow> {
    let listener_stage = stage.clone();
    let servers = provision_mcp(
        machine,
        mcp,
        ProvisionMcpOptions {
            home: on.home.clone(),
            landed: OwnedPaths::new(),
            tools: Vec::new(),
            path: path.to_string(),
            held: on.held.clone(),
            stores: on.stores.clone(),
            stage: Arc::new(move |_which: GoldenStage, detail: Option<&str>| {
                if let Some(detail) = detail {
                    listener_stage(detail, None, None);
                }
            }),
            into: None,
        },
    )
    .await;
    for server in &servers {
        stage(&row_line(server), None, Some(server));
    }
    close_agent_files(machine, &on.home, &[], &stores_of(on)).await;
    servers
}

fn stores_of(on: &ProvisionOn) -> IndexMap<String, String> {
    on.stores.clone().unwrap_or_default()
}

/// provision.ts:386 `provisionStep`: the step's rows, each with `step`. Fails only when the computer stopped
/// answering.
pub async fn provision_step(
    machine: &Arc<dyn Machine>,
    plan: &ProvisionPlan,
    step: EngineStep,
    run: SharedRun,
    stage: ProvisionStage,
    on: &ProvisionOn,
) -> Result<Vec<PlaceProvisionRow>> {
    let marked: ProvisionStage = {
        let stage = stage.clone();
        Arc::new(move |detail: &str, at: Option<&StageAt>, row: Option<&PlaceProvisionRow>| {
            let row = row.map(|r| PlaceProvisionRow { step: Some(step.setup_step()), ..r.clone() });
            stage(detail, at, row.as_ref())
        })
    };
    let walked = if plan.prefix.is_none() { Vec::new() } else { outside_roots(plan, step) };
    let outside = ExecOptions { timeout_ms: Some(OUTSIDE_MS), ..Default::default() };
    if !walked.is_empty() {
        let _ = machine.exec(&outside_before_script(step.word(), &walked, ""), outside.clone()).await;
    }
    let rows = step_rows(machine, plan, step, &run, &marked, on).await;
    if !walked.is_empty() {
        let _ = machine.exec(&outside_after_script(step.word(), &walked, ""), outside).await;
    }
    Ok(rows?.into_iter().map(|row| PlaceProvisionRow { step: Some(step.setup_step()), ..row }).collect())
}

/// provision.ts:401 `stepInstalls`.
pub fn step_installs(plan: &ProvisionPlan, step: EngineStep) -> Vec<ToolInstall> {
    let split = plan.agents.min(plan.steps.len());
    match step {
        EngineStep::Floor => base_installs(&BTreeSet::new(), &plan.path, plan.prefix.as_deref(), &left_off(plan.compiler)),
        EngineStep::Agents => plan.steps[..split].to_vec(),
        EngineStep::Clis => plan.steps[split..].to_vec(),
        EngineStep::Github => plan.github.clone().unwrap_or_default(),
        EngineStep::Configs => plan.config_tools.clone().unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// provision.ts:422 `outsideRoots`: sorted, unique, none inside another, none under `/opt/wsp`.
pub fn outside_roots(plan: &ProvisionPlan, step: EngineStep) -> Vec<String> {
    let dirs: Vec<String> = step_installs(plan, step)
        .iter()
        .flat_map(|row| {
            let roots = is_road(Some(&row.manager)).map(|road| road_module_named(road).roots()).unwrap_or_default();
            roots.into_iter().chain(row.bins.clone().unwrap_or_default())
        })
        .collect();
    let mut clipped: Vec<String> = Vec::new();
    for dir in &dirs {
        for top in OUTSIDE_TOPS {
            if within(dir, top) {
                clipped.push(dir.clone());
            } else if within(top, dir) {
                clipped.push(top.to_string());
            }
        }
    }
    let mut unique: Vec<String> = clipped.into_iter().filter(|dir| !within(dir, TOOL_PREFIX)).collect();
    unique.sort_by(|a, b| utf16::cmp(a, b));
    unique.dedup();
    unique.iter().filter(|dir| !unique.iter().any(|other| other != *dir && within(dir, other))).cloned().collect()
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
    path == dir || path.starts_with(&format!("{dir}/"))
}

/// provision.ts:531 `hookOf`: the hook a CLI row's catalog tool entry names.
pub fn hook_of(row_id: &str) -> Option<(String, String)> {
    let id = catalog_id_of_row(row_id)?;
    match catalog_entry(&id)? {
        CatalogEntry::Tool(tool) => tool.hook.as_ref().map(|hook| (hook.on.clone(), hook.off.clone())),
        CatalogEntry::Agent(_) => None,
    }
}

/// provision.ts:551 `lastWords`: the last line of the trimmed text, its first 300 UTF-16 units; `None` for empty.
pub fn last_words(text: &str) -> Option<String> {
    let last = js_trim(text).split('\n').next_back()?;
    let cut = utf16::cut(last, 300);
    (!cut.is_empty()).then_some(cut)
}

/// One files round's argument (provision.ts:354).
pub type FilesRound<'a> = (&'a super::plan::PlanFiles, &'a str);

fn import_row(result: &ToolResult) -> ImportRow {
    ImportRow { id: result.id.clone(), label: result.label.clone(), outcome: result.outcome.clone(), note: result.note.clone() }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// A run's exec answer as the TS reads it, with a rejection as exit -1 and its message on stderr.
fn answer(res: Result<crate::machine::ExecResult>) -> (i64, String, String) {
    match res {
        Ok(res) => (res.exit_code.get(), res.stdout, res.stderr),
        Err(e) => (-1, String::new(), e.to_string()),
    }
}

/// The text a failed run's note says: its stderr, or its stdout where stderr is empty, else its exit.
fn words_or_exit(stdout: &str, stderr: &str, code: i64) -> String {
    let text = if stderr.is_empty() { stdout } else { stderr };
    last_words(text).unwrap_or_else(|| format!("exit {code}"))
}

/// provision.ts:540 `hooked`: an installed row with a catalog hook runs it, and a hook that fails fails the row.
async fn hooked(machine: &dyn Machine, plan: &ProvisionPlan, row: PlaceProvisionRow) -> PlaceProvisionRow {
    let Some((on, _)) = hook_of(&row.id) else {
        return row;
    };
    if !matches!(row.outcome, PlaceProvisionRowOutcome::Installed) {
        return row;
    }
    let cmd = format!("{}\n{on}", path_line(&plan.path, plan.prefix.as_deref()));
    let res = machine.exec(&cmd, ExecOptions { timeout_ms: Some(HOOK_MS), ..Default::default() }).await;
    let (code, stdout, stderr) = answer(res);
    if code == 0 {
        return row;
    }
    PlaceProvisionRow {
        outcome: PlaceProvisionRowOutcome::Failed,
        note: Some(format!("{on}: {}", words_or_exit(&stdout, &stderr, code))),
        ..row
    }
}

/// provision.ts:285 `floorStep`: the floor through installBase, keeping this computer's caches: only a failed floor row is a row.
async fn floor_step(
    machine: &dyn Machine,
    plan: &ProvisionPlan,
    run: &SharedRun,
    stage: &ProvisionStage,
) -> Result<Vec<PlaceProvisionRow>> {
    let listener = stage.clone();
    let on_stage: ToolStage = Arc::new(move |_which: GoldenStage, detail: Option<&str>, _step: Option<GoldenStep>| {
        if let Some(detail) = detail {
            listener(detail, None, None);
        }
    });
    let base = install_base_with(
        machine,
        on_stage,
        InstallBaseOptions { keep_caches: true, path: Some(plan.path.clone()), prefix: plan.prefix.clone(), left: left_off(plan.compiler) },
    )
    .await?;
    stage(&base.line, None, None);
    let failed = base
        .tools
        .iter()
        .filter(|t| t.outcome == "failed")
        .map(|t| row(&t.id, &t.label, PlaceProvisionRowOutcome::Failed, t.note.clone()))
        .collect();
    run.lock().unwrap().base = base.tools;
    Ok(failed)
}

/// provision.ts:296 `toolsStep`: some of the plan's steps through the one tools loop, those the computer already
/// satisfies read first, each row read once the loop's own checks have run.
async fn tools_step(
    machine: &dyn Machine,
    plan: &ProvisionPlan,
    steps: Vec<ToolInstall>,
    run: &SharedRun,
    stage: &ProvisionStage,
) -> Result<Vec<PlaceProvisionRow>> {
    if steps.is_empty() {
        return Ok(Vec::new());
    }
    let read = present_steps(machine, &steps, &plan.path, plan.prefix.as_deref(), Versions::Asked).await?;
    let read_ids: BTreeSet<String> = read.keys().cloned().collect();
    let mut present: IndexMap<String, PresentRead> = read;
    for id in present_by_what_waits(&steps, &read_ids) {
        present.insert(id, PresentRead::default());
    }
    let floor = owned_floor_bytes(disk_use_size(machine, plan.prefix.as_deref()).await);
    let done = Arc::new(Mutex::new(0usize));
    let at: Arc<dyn Fn() -> Option<StageAt>> = {
        let (done, steps) = (done.clone(), steps.clone());
        Arc::new(move || {
            let index = *done.lock().unwrap();
            steps.get(index).map(|s| StageAt { label: s.label.clone(), index: index + 1, of: steps.len() })
        })
    };
    let on_stage: ToolStage = {
        let (at, stage) = (at.clone(), stage.clone());
        Arc::new(move |_which: GoldenStage, detail: Option<&str>, _step: Option<GoldenStep>| {
            if let Some(detail) = detail {
                stage(detail, at().as_ref(), None);
            }
        })
    };
    let (tx, mut rx) = mpsc::unbounded::<(ToolResult, oneshot::Sender<()>)>();
    let on_tool: OnTool = Arc::new(move |result: ToolResult| {
        let (ack, acked) = oneshot::channel::<()>();
        let _ = tx.unbounded_send((result, ack));
        Box::pin(async move {
            let _ = acked.await;
        }) as LocalBoxFuture<'static, ()>
    });
    let opts = InstallToolsOptions {
        present: present.keys().cloned().collect(),
        keep_caches: true,
        floor: Some(floor),
        path: Some(plan.path.clone()),
        prefix: plan.prefix.clone(),
        on_tool: Some(on_tool),
    };
    let install = install_tools_at(machine, &steps, on_stage, GoldenStage::InstallingTools, opts);
    let drain = async {
        let mut said: IndexMap<String, String> = IndexMap::new();
        let mut hook_failed: IndexMap<String, PlaceProvisionRow> = IndexMap::new();
        while let Some((result, ack)) = rx.next().await {
            run.lock().unwrap().tools.push(result.clone());
            let step = steps.iter().find(|s| s.id == result.id);
            let row = hooked(machine, plan, row_of(&result, &present, step)).await;
            if matches!(row.outcome, PlaceProvisionRowOutcome::Failed) && result.outcome != "failed" {
                hook_failed.insert(row.id.clone(), row.clone());
            }
            let line = row_line(&row);
            said.insert(result.id.clone(), line.clone());
            *done.lock().unwrap() += 1;
            stage(&line, at().as_ref(), Some(&row));
            let _ = ack.send(());
        }
        (said, hook_failed)
    };
    let (outcome, (said, hook_failed)) = futures::join!(install, drain);
    let tools = outcome?.tools;
    {
        let mut run = run.lock().unwrap();
        for result in &tools {
            match run.tools.iter().position(|t| t.id == result.id) {
                Some(at) => run.tools[at] = result.clone(),
                None => run.tools.push(result.clone()),
            }
        }
    }
    let rows: Vec<PlaceProvisionRow> = tools
        .iter()
        .map(|result| {
            let row = row_of(result, &present, steps.iter().find(|s| s.id == result.id));
            if matches!(row.outcome, PlaceProvisionRowOutcome::Installed) {
                hook_failed.get(&row.id).cloned().unwrap_or(row)
            } else {
                row
            }
        })
        .collect();
    for row in &rows {
        let line = row_line(row);
        if said.get(&row.id) != Some(&line) {
            stage(&line, None, None);
        }
    }
    Ok(rows)
}

/// provision.ts:354 `filesRound`: one files round, the archive read off this computer and landed in the homes there.
/// A round the servers do not read is closed at once; the agents' own round stays open for the servers step.
async fn files_round(
    machine: &Arc<dyn Machine>,
    files: &super::plan::PlanFiles,
    label: &str,
    run: &SharedRun,
    stage: &ProvisionStage,
    on: &ProvisionOn,
    close: bool,
) -> Result<Vec<PlaceProvisionRow>> {
    stage(&format!("{label}: {}", plural(files.lands.len() as f64, "path")), None, None);
    let say = stage.clone();
    let landed = provision_files(
        machine,
        ProvisionFilesAsk {
            home: on.home.clone(),
            lands: files.lands.clone(),
            pack: files.pack.clone(),
            say: Some(Arc::new(move |line: &str| say(line, None, None))),
            stores: stores_of(on),
        },
    )
    .await;
    for row in &landed.rows {
        stage(&row_line(row), None, Some(row));
    }
    {
        let mut run = run.lock().unwrap();
        run.skipped_files.extend(landed.skipped.iter().cloned());
        if !close {
            run.landed = landed.owned.clone();
        }
    }
    if close {
        close_agent_files(&**machine, &on.home, &once_paths_of(&files.lands), &stores_of(on)).await;
    }
    Ok(landed.rows)
}

/// provision.ts:436 `stepRows`: the rows of one step, by the step's own rule.
async fn step_rows(
    machine: &Arc<dyn Machine>,
    plan: &ProvisionPlan,
    step: EngineStep,
    run: &SharedRun,
    stage: &ProvisionStage,
    on: &ProvisionOn,
) -> Result<Vec<PlaceProvisionRow>> {
    let handle = machine;
    let machine: &dyn Machine = &**handle;
    let split = plan.agents.min(plan.steps.len());
    match step {
        EngineStep::Floor => floor_step(machine, plan, run, stage).await,
        EngineStep::Agents => {
            let aside: Vec<PlaceProvisionRow> =
                plan.skipped.iter().map(|s| row(&s.id, &s.label, PlaceProvisionRowOutcome::Skipped, Some(s.note.clone()))).collect();
            for r in &aside {
                stage(&row_line(r), None, Some(r));
            }
            let mut rows = aside;
            rows.extend(tools_step(machine, plan, plan.steps[..split].to_vec(), run, stage).await?);
            Ok(rows)
        }
        EngineStep::Clis => tools_step(machine, plan, plan.steps[split..].to_vec(), run, stage).await,
        EngineStep::Mcp => mcp_step(handle, plan, run, stage, on).await,
        EngineStep::Skills => match &plan.skills {
            Some(skills) => files_round(handle, skills, SKILLS_LABEL, run, stage, on, true).await,
            None => Ok(Vec::new()),
        },
        EngineStep::Github => tools_step(machine, plan, plan.github.clone().unwrap_or_default(), run, stage).await,
        EngineStep::Plugins => {
            let mut rows = Vec::new();
            for plugin in plan.plugins.iter().flatten() {
                let started = now_ms();
                let cmd = format!("{}\n{}", path_line(&plan.path, plan.prefix.as_deref()), plugin.cmd);
                let res = machine.run(&cmd, RunOptions { deadline_ms: PLUGIN_MS, ..Default::default() }).await;
                let (code, stdout, stderr) = answer(res);
                let asked = if code == 0 { None } else { plugin.asked.as_ref().and_then(|asked| asked(&stdout)) };
                let plugin_row = if code == 0 {
                    PlaceProvisionRow {
                        ms: Some(JsInt::from(now_ms() - started)),
                        ..row(&plugin.id, &plugin.label, PlaceProvisionRowOutcome::Installed, None)
                    }
                } else if let Some(note) = asked {
                    row(&plugin.id, &plugin.label, PlaceProvisionRowOutcome::Skipped, Some(note))
                } else {
                    row(&plugin.id, &plugin.label, PlaceProvisionRowOutcome::Failed, Some(words_or_exit(&stdout, &stderr, code)))
                };
                stage(&row_line(&plugin_row), None, Some(&plugin_row));
                rows.push(plugin_row);
            }
            Ok(rows)
        }
        EngineStep::Configs => {
            let mut rows = tools_step(machine, plan, plan.config_tools.clone().unwrap_or_default(), run, stage).await?;
            if let Some(configs) = &plan.configs {
                rows.extend(files_round(handle, configs, CONFIGS_LABEL, run, stage, on, true).await?);
            }
            Ok(rows)
        }
        EngineStep::Context => {
            let result = {
                let run = run.lock().unwrap();
                ImportResult {
                    recipe_hash: String::new(),
                    base: Some(run.base.iter().map(import_row).collect()),
                    tools: run.tools.iter().map(import_row).collect(),
                    files: (!run.skipped_files.is_empty()).then(|| ImportFiles { bytes: 0, skipped: run.skipped_files.clone() }),
                    ..Default::default()
                }
            };
            let context = apply_machine_context(
                handle,
                ApplyContextOptions {
                    result: Some(result),
                    path: Some(plan.path.clone()),
                    shells: Some(ProbeShells::None),
                    ..Default::default()
                },
            )
            .await;
            stage(&format!("machine context: {}", context.summary), None, None);
            Ok(Vec::new())
        }
    }
}

/// The MCP step: the agents' own files round stays open, the servers merge into them, and the files are closed with
/// the run's landed list cleared.
async fn mcp_step(
    machine: &Arc<dyn Machine>,
    plan: &ProvisionPlan,
    run: &SharedRun,
    stage: &ProvisionStage,
    on: &ProvisionOn,
) -> Result<Vec<PlaceProvisionRow>> {
    let mut rows = match &plan.files {
        Some(files) => files_round(machine, files, FILES_LABEL, run, stage, on, false).await?,
        None => Vec::new(),
    };
    let stores = stores_of(on);
    if let Some(mcp) = &plan.mcp {
        stage(&format!("{MCP_LABEL}: {}", plural(mcp.agents.len() as f64, "agent")), None, None);
        let into: Arc<Mutex<IndexMap<String, Vec<String>>>> = Arc::new(Mutex::new(IndexMap::new()));
        let into_sink = into.clone();
        let listener = stage.clone();
        let (landed, tools) = {
            let run = run.lock().unwrap();
            (run.landed.clone(), run.tools.clone())
        };
        let servers = provision_mcp(
            &**machine,
            mcp,
            ProvisionMcpOptions {
                home: on.home.clone(),
                landed,
                tools,
                path: plan.path.clone(),
                held: on.held.clone(),
                stores: on.stores.clone(),
                stage: Arc::new(move |_which: GoldenStage, detail: Option<&str>| {
                    if let Some(detail) = detail {
                        listener(detail, None, None);
                    }
                }),
                into: Some(Arc::new(move |file: &str, names: &[String]| {
                    into_sink.lock().unwrap().insert(file.to_string(), names.to_vec());
                })),
            },
        )
        .await;
        let roots = store_roots(&on.home, &stores);
        for land in plan.files.iter().flat_map(|files| files.lands.iter()) {
            let names = if land.once { into.lock().unwrap().get(&landed_at(&on.home, &roots, &land.dest)).cloned() } else { None };
            let at =
                rows.iter().position(|r| r.id == format!("files/{}", land.dest) && matches!(r.outcome, PlaceProvisionRowOutcome::Present));
            if let (Some(names), Some(at)) = (names, at) {
                let note = [rows[at].note.clone(), Some(merged_into_line(&names))].into_iter().flatten().collect::<Vec<_>>().join("; ");
                rows[at].note = Some(note);
            }
        }
        let installed = servers.iter().filter(|r| matches!(r.outcome, PlaceProvisionRowOutcome::Installed)).count();
        stage(&provision_servers_line(installed as f64, servers.len() as f64), None, None);
        for server in &servers {
            stage(&row_line(server), None, Some(server));
        }
        rows.extend(servers);
    }
    if on.stores.is_some() {
        let set_up: BTreeSet<String> = plan.steps[..plan.agents.min(plan.steps.len())].iter().filter_map(|s| agent_of_row(&s.id)).collect();
        for line in headless_keys(&**machine, &on.home, &stores, &set_up).await {
            stage(&line, None, None);
        }
    }
    let lands: Vec<ProvisionLanding> = plan.files.as_ref().map(|f| f.lands.clone()).unwrap_or_default();
    close_agent_files(&**machine, &on.home, &once_paths_of(&lands), &stores).await;
    run.lock().unwrap().landed = OwnedPaths::new();
    Ok(rows)
}
