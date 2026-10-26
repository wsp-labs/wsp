// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:578-865: the import stages on a builder, skipping what the ledger says is already there for the same
//! recipe hash.
use std::sync::Arc;

use wsp_catalog::road_modules::road_step;
use wsp_catalog::roads::{RoadName, NODE_PATH_LINE};
use wsp_wire::views::golden_image::{GoldenMissingTool, GoldenMissingToolOutcome, GoldenStage, ALREADY_APPLIED};
use wsp_words::js::{js_trim, to_fixed, to_number};
use wsp_words::words::base::plural;
use wsp_words::words::units::fmt_bytes;

use super::smoke::{join_smoke, manager_check_of, next_smoke};
use super::types::{plain, AgentResult, Applied, ApplyImportOptions, ImportLedger, ImportResult, ImportStage, ResultFiles, Stage};
use crate::dotfiles_presets::PRELUDE;
use crate::errors::{EngineError, RetryClock};
use crate::exec_detached::INLINE_EXEC_MS;
use crate::fetch::Fetch;
use crate::golden_seam::{
    apply_mcp, closing, free_bytes, free_note, guard_deadline_ms, guarded, install_tools_at, mcp_tally, reason_of, sweep_caches, FreeDisk,
    GoldenImport, InstallToolsOptions, LoginShell, NodeInstall, PackedFiles, ToolResult, TOOLS_PATH,
};
use crate::machine::{ExecOptions, Machine, OnLine, RunOptions};
use crate::machine_context::{apply_machine_context, bounded_command, ApplyContextOptions};
use crate::vault::{import_into, VaultOptions};

const MIB: u64 = 1024 * 1024;
/// golden.ts:579. Extraction needs the archive and its contents at once, plus what the agents install after.
const UPLOAD_HEADROOM: u64 = 256 * MIB;
/// golden.ts:581. The Claude installer peaks near 410 MB; an agent does not start under this.
const AGENTS_DISK_FLOOR: u64 = 800 * MIB;
const AGENT_TIMEOUT_S: u64 = 900;
const SHELL_TIMEOUT_S: u64 = 300;
const SHELL_CHECK_S: u64 = 60;

/// golden.ts:586 `guardedHarness`: a harness-stage install under the guard, with every road's network lines ahead of it.
fn guarded_harness(script: &str) -> String {
    let mut lines: Vec<String> = road_step(RoadName::Script).env.to_vec();
    lines.push(script.to_string());
    guarded(&lines.join("\n"), AGENT_TIMEOUT_S)
}

/// golden.ts:593 `shellCheck`: one start of the login shell the way the app's pty runs it, bounded as its own group.
fn shell_check(shell: LoginShell) -> String {
    format!(
        "{PRELUDE}\n{} || exit 124",
        bounded_command(SHELL_CHECK_S as u32, &format!("TERM=xterm-256color {} -lic true </dev/null", shell.as_str()))
    )
}

fn on_line(f: impl Fn(&str) + 'static) -> Option<OnLine> {
    Some(Arc::new(f))
}

fn run_options(timeout_s: u64, on_line: Option<OnLine>) -> RunOptions {
    RunOptions { deadline_ms: guard_deadline_ms(timeout_s), on_line, ..RunOptions::default() }
}

fn exec_options(timeout_ms: u64) -> ExecOptions {
    ExecOptions { timeout_ms: Some(timeout_ms), ..ExecOptions::default() }
}

/// golden.ts:620 `upload`: the packed archive onto the machine, after a look at the free disk.
async fn upload(
    machine: &Arc<dyn Machine>,
    stage: &Stage,
    fetch: &Option<Arc<dyn Fetch>>,
    clock: &dyn RetryClock,
    packed: &PackedFiles,
    label: &str,
) -> Result<(), EngineError> {
    let free = free_bytes(&**machine, None).await?;
    let need = packed.bytes + packed.unpacked + UPLOAD_HEADROOM;
    match free {
        FreeDisk::Unknown { reason } => {
            stage.say(
                GoldenStage::UploadingFiles,
                &format!("free disk unknown ({reason}); uploading {} anyway", fmt_bytes(packed.bytes as f64)),
            );
        }
        FreeDisk::Free { bytes } if need as f64 > bytes => {
            return Err(plain(format!(
                "your files need {} packed and {} unpacked, plus {} of headroom, but the machine has {} free",
                fmt_bytes(packed.bytes as f64),
                fmt_bytes(packed.unpacked as f64),
                fmt_bytes(UPLOAD_HEADROOM as f64),
                fmt_bytes(bytes)
            )));
        }
        FreeDisk::Free { .. } => {}
    }
    let t0 = clock.now();
    let part_stage = stage.clone();
    let imported = import_into(
        machine.clone(),
        &packed.tar,
        "/root",
        &VaultOptions {
            overlay: true,
            timeout_ms: Some(300_000),
            on_part: Some(Arc::new(move |p| {
                if p.parts > 1 {
                    part_stage.say(
                        GoldenStage::UploadingFiles,
                        &format!("part {} of {}, {} of {}", p.part, p.parts, fmt_bytes(p.bytes as f64), fmt_bytes(p.total as f64)),
                    );
                }
            })),
            fetch: fetch.clone(),
            ..VaultOptions::default()
        },
    )
    .await?;
    let parts = if imported.parts > 1 { format!(" in {} parts", imported.parts) } else { String::new() };
    let took = format!("{label}{}{parts} in {}s", fmt_bytes(packed.bytes as f64), to_fixed((clock.now() - t0) as f64 / 1000.0, 1));
    let note = free_note(&**machine, None).await?;
    stage.say(GoldenStage::UploadingFiles, &closing(&[Some(took.as_str()), note.as_deref()]));
    Ok(())
}

fn notes_of(skipped: &[crate::golden_seam::SkippedPath]) -> Vec<String> {
    skipped.iter().map(|s| format!("{} ({})", s.path, s.note)).collect()
}

/// golden.ts:598. Runs the import stages and the harness on a builder, skipping what the ledger says is already there
/// for the same recipe hash. Files and upload fail the build; each tool and agent fails alone and is named in the stage
/// detail.
pub async fn apply_golden_import(
    machine: &Arc<dyn Machine>,
    opts: ApplyImportOptions,
    clock: &dyn RetryClock,
) -> Result<Applied, EngineError> {
    let stage = Stage::new(opts.on_stage.clone());
    let none = GoldenImport::default();
    let imp = opts.import.as_ref().unwrap_or(&none);
    let prior = opts.ledger.as_ref().filter(|l| l.recipe_hash == imp.recipe_hash);
    let recipe = imp.recipe.clone().or_else(|| prior.and_then(|p| p.recipe.clone()));
    let mut ledger = ImportLedger {
        recipe_hash: imp.recipe_hash.clone(),
        applied: prior.map(|p| p.applied.clone()).unwrap_or_default(),
        smoke: prior.map(|p| p.smoke.clone()).unwrap_or_else(|| "true".to_string()),
        recipe,
        missing_tools: prior.and_then(|p| p.missing_tools.clone()),
        silenced: prior.and_then(|p| p.silenced.clone()),
        shell_noise: prior.and_then(|p| p.shell_noise.clone()),
        left_behind: prior.and_then(|p| p.left_behind.clone()),
    };
    let mut result = ImportResult {
        recipe_hash: imp.recipe_hash.clone(),
        base: opts.base.clone(),
        retired: opts.retired.clone(),
        ..ImportResult::default()
    };
    let only = opts.import.is_none();
    let mut ran = false;
    let done = |ledger: &ImportLedger, s: ImportStage| ledger.applied.contains(&s);
    let mark = |ledger: &mut ImportLedger, s: ImportStage| {
        if !ledger.applied.contains(&s) {
            ledger.applied.push(s);
        }
    };

    if !only {
        if done(&ledger, ImportStage::UploadingFiles) {
            stage.say(GoldenStage::ApplyingSetup, ALREADY_APPLIED);
            match imp.files.as_ref().and_then(|f| f.volatile.as_ref()) {
                None => stage.say(GoldenStage::UploadingFiles, ALREADY_APPLIED),
                Some(volatile) => {
                    // A volatile file is not in the hash, so the builder may hold an older copy; the latest one goes up
                    // again. A re-import that fails is reported, never failed.
                    let attempt = async {
                        let packed = (volatile.pack)(None).await?;
                        let n = volatile.paths.len();
                        stage.say(
                            GoldenStage::UploadingFiles,
                            &format!("{n} volatile file{}, {}", if n == 1 { "" } else { "s" }, fmt_bytes(packed.bytes as f64)),
                        );
                        upload(machine, &stage, &opts.fetch, clock, &packed, &format!("{} re-imported, ", volatile.paths.join(", "))).await
                    };
                    if let Err(e) = attempt.await {
                        stage.say(GoldenStage::UploadingFiles, &format!("{} not re-imported: {e}", volatile.paths.join(", ")));
                    }
                }
            }
        } else if imp.files.as_ref().is_none_or(|f| f.count == 0) {
            let skipped = imp.files.as_ref().map(|f| f.skipped.clone()).unwrap_or_default();
            let notes = notes_of(&skipped);
            let said =
                if notes.is_empty() { "nothing ticked".to_string() } else { format!("nothing left to pack; skipped {}", notes.join(", ")) };
            stage.say(GoldenStage::ApplyingSetup, &said);
            stage.say(GoldenStage::UploadingFiles, "nothing to upload");
            // No pack ran, so nothing is known about cuts; a reader falls back to the recipe's own names.
            result.files = Some(ResultFiles { bytes: 0, skipped, cut: None, blanked: None });
            ledger.left_behind = None;
            mark(&mut ledger, ImportStage::ApplyingSetup);
            mark(&mut ledger, ImportStage::UploadingFiles);
        } else {
            let files = imp.files.as_ref().expect("files with a count");
            let rungs: Vec<String> = files.rungs.iter().map(|(r, n)| format!("{r} {n}")).collect();
            stage.say(
                GoldenStage::ApplyingSetup,
                &format!("{} file{}: {}", files.count, if files.count == 1 { "" } else { "s" }, rungs.join(", ")),
            );
            let mut packed = (files.pack)(None).await?;
            packed.skipped = files.skipped.iter().cloned().chain(packed.skipped.drain(..)).collect();
            ledger.left_behind = packed.left_behind.clone().filter(|l| !l.is_empty());
            let notes = notes_of(&packed.skipped);
            let silenced =
                if packed.silenced.is_empty() { String::new() } else { format!("; silenced in the shell: {}", packed.silenced.join(", ")) };
            let mac_paths =
                if packed.mac_paths.is_empty() { String::new() } else { format!("; Mac paths: {}", packed.mac_paths.join(", ")) };
            let blanked_rows = packed.blanked.clone().unwrap_or_default();
            let blanked = if blanked_rows.is_empty() {
                String::new()
            } else {
                format!("; copied with values cut: {}", notes_of(&blanked_rows).join(", "))
            };
            stage.say(
                GoldenStage::ApplyingSetup,
                &format!(
                    "{} packed{}{blanked}{silenced}{mac_paths}",
                    fmt_bytes(packed.bytes as f64),
                    if notes.is_empty() { String::new() } else { format!("; skipped {}", notes.join(", ")) }
                ),
            );
            ledger.silenced = (!packed.silenced.is_empty()).then(|| packed.silenced.clone());
            mark(&mut ledger, ImportStage::ApplyingSetup);
            // The frameworks clone into empty homes, so the shell goes on before the files land; the upload's mark covers it.
            if let Some(shell) = &imp.shell {
                let name = shell.shell.as_str();
                let named = shell.frameworks.join(", ");
                let with = if named.is_empty() { String::new() } else { format!(", with {named}") };
                stage.say(GoldenStage::ApplyingSetup, &format!("{name}: installing{with}"));
                let lines = stage.clone();
                let prefix = name.to_string();
                let res = machine
                    .run(
                        &guarded(&shell.cmd, SHELL_TIMEOUT_S),
                        run_options(
                            SHELL_TIMEOUT_S,
                            on_line(move |line| lines.say(GoldenStage::ApplyingSetup, &format!("{prefix}: {line}"))),
                        ),
                    )
                    .await?;
                if res.exit_code.get() == 0 {
                    let installed = format!(
                        "{name} installed as the login shell{}",
                        if named.is_empty() { String::new() } else { format!("; {named} reinstalled") }
                    );
                    let note = free_note(&**machine, None).await?;
                    stage.say(GoldenStage::ApplyingSetup, &closing(&[Some(installed.as_str()), note.as_deref()]));
                } else {
                    stage.say(
                        GoldenStage::ApplyingSetup,
                        &format!("{name} step failed, chsh skipped: {}", reason_of(&res, SHELL_TIMEOUT_S as f64)),
                    );
                }
            }
            stage.say(GoldenStage::UploadingFiles, &fmt_bytes(packed.bytes as f64));
            ran = true;
            upload(machine, &stage, &opts.fetch, clock, &packed, "").await?;
            result.files = Some(ResultFiles {
                bytes: packed.bytes,
                skipped: packed.skipped.clone(),
                cut: Some(packed.cut.clone()),
                blanked: packed.blanked.clone().filter(|b| !b.is_empty()),
            });
            mark(&mut ledger, ImportStage::UploadingFiles);
        }
    }

    let harness_ran = !done(&ledger, ImportStage::InstallingHarness);
    if !harness_ran {
        stage.say(GoldenStage::InstallingHarness, ALREADY_APPLIED);
    } else {
        stage.bare(GoldenStage::InstallingHarness);
        let lines = stage.clone();
        let res = machine
            .run(
                &guarded_harness(&opts.setup),
                run_options(AGENT_TIMEOUT_S, on_line(move |line| lines.say(GoldenStage::InstallingHarness, line))),
            )
            .await?;
        if res.exit_code.get() != 0 {
            return Err(plain(format!("golden setup failed: {}", reason_of(&res, AGENT_TIMEOUT_S as f64))));
        }
        if !only {
            ran = true;
            let node = match &imp.node {
                Some(n) => Some(install_node(&**machine, n, &stage).await?),
                None => None,
            };
            for a in &imp.skipped_agents {
                result.agents.push(AgentResult {
                    id: a.id.clone(),
                    name: a.label.clone(),
                    outcome: "skipped".into(),
                    note: Some(a.note.clone()),
                    ms: None,
                });
            }
            for (i, agent) in imp.agents.iter().enumerate() {
                let t0 = clock.now();
                if let Some(node) = &node {
                    if let (Some(failed), Some(floor)) = (&node.failed, agent.node) {
                        if floor > node.have_major {
                            result.agents.push(AgentResult {
                                id: agent.id.clone(),
                                name: agent.name.clone(),
                                outcome: "failed".into(),
                                note: Some(failed.clone()),
                                ms: Some(0),
                            });
                            continue;
                        }
                    }
                }
                if let FreeDisk::Free { bytes } = free_bytes(&**machine, None).await? {
                    if bytes < AGENTS_DISK_FLOOR as f64 {
                        let note = format!("{} free, keeping {} free", fmt_bytes(bytes), fmt_bytes(AGENTS_DISK_FLOOR as f64));
                        result.agents.push(AgentResult {
                            id: agent.id.clone(),
                            name: agent.name.clone(),
                            outcome: "failed".into(),
                            note: Some(note),
                            ms: Some(0),
                        });
                        continue;
                    }
                }
                stage.say(GoldenStage::InstallingHarness, &format!("{} ({}/{})", agent.name, i + 1, imp.agents.len()));
                let lines = stage.clone();
                let name = agent.name.clone();
                let install = machine
                    .run(
                        &guarded_harness(&format!("{NODE_PATH_LINE}\n{}", agent.install)),
                        run_options(
                            AGENT_TIMEOUT_S,
                            on_line(move |line| lines.say(GoldenStage::InstallingHarness, &format!("{name}: {line}"))),
                        ),
                    )
                    .await?;
                let check = if install.exit_code.get() == 0 {
                    machine
                        .run(&format!("{NODE_PATH_LINE}\n{}", agent.smoke), RunOptions { deadline_ms: 60_000, ..RunOptions::default() })
                        .await?
                } else {
                    install
                };
                let ms = clock.now() - t0;
                if check.exit_code.get() == 0 {
                    result.agents.push(AgentResult {
                        id: agent.id.clone(),
                        name: agent.name.clone(),
                        outcome: "installed".into(),
                        note: None,
                        ms: Some(ms),
                    });
                } else {
                    result.agents.push(AgentResult {
                        id: agent.id.clone(),
                        name: agent.name.clone(),
                        outcome: "failed".into(),
                        note: Some(reason_of(&check, AGENT_TIMEOUT_S as f64)),
                        ms: Some(ms),
                    });
                }
            }
            let installed: Vec<&crate::golden_seam::AgentInstall> =
                imp.agents.iter().filter(|a| result.agents.iter().any(|r| r.id == a.id && r.outcome == "installed")).collect();
            let failed: Vec<&AgentResult> = result.agents.iter().filter(|r| r.outcome == "failed").collect();
            // A golden with an agent missing is not sealed: the person ticked it, and the hand-off would open a
            // terminal on a machine without it. All-skipped is the same refusal with the plan's reasons.
            if !failed.is_empty() || (!result.agents.is_empty() && installed.is_empty()) {
                if let Some(report) = &imp.on_result {
                    report(&result);
                }
                let header = if failed.is_empty() {
                    "no agent installed, so there is nothing to seal:".to_string()
                } else {
                    format!(
                        "{} did not install, so nothing is sealed:",
                        if failed.len() == 1 { "an agent".to_string() } else { format!("{} agents", failed.len()) }
                    )
                };
                let mut lines = vec![header];
                lines.extend(
                    result
                        .agents
                        .iter()
                        .filter(|r| r.outcome != "installed")
                        .map(|a| format!("{}: {}", a.name, a.note.as_deref().unwrap_or("unknown reason"))),
                );
                return Err(plain(lines.join("\n")));
            }
            if let Some(plugins) = &imp.plugins {
                super::plugins::apply_plugins(&**machine, plugins, stage.listener()).await;
            }
            ledger.smoke = join_smoke(&installed.iter().map(|a| a.smoke.clone()).collect::<Vec<_>>());
            let summary = if result.agents.is_empty() { "no agent ticked".to_string() } else { summarize_agents(&result.agents) };
            let detail = if !imp.agents.is_empty() || node.is_some() {
                let swept = sweep_caches(&**machine, TOOLS_PATH).await?;
                let free = free_note(&**machine, None).await?;
                closing(&[Some(summary.as_str()), Some(swept.as_str()), free.as_deref()])
            } else {
                summary
            };
            stage.say(GoldenStage::InstallingHarness, &detail);
        }
        mark(&mut ledger, ImportStage::InstallingHarness);
    }

    let tools_ran = !only && !done(&ledger, ImportStage::InstallingTools);
    if !only {
        if !tools_ran {
            stage.say(GoldenStage::InstallingTools, ALREADY_APPLIED);
        } else {
            for t in &imp.skipped_tools {
                result.tools.push(ToolResult {
                    id: t.id.clone(),
                    label: t.label.clone(),
                    outcome: "skipped".into(),
                    note: Some(t.note.clone()),
                    ..ToolResult::default()
                });
            }
            for t in &imp.base_tools {
                result.tools.push(ToolResult {
                    id: t.id.clone(),
                    label: t.label.clone(),
                    outcome: "installed".into(),
                    note: Some(t.note.clone()),
                    ..ToolResult::default()
                });
            }
            if imp.tools.is_empty() {
                stage.say(GoldenStage::InstallingTools, "nothing ticked");
            } else {
                ran = true;
                let tools = install_tools_at(
                    &**machine,
                    &imp.tools,
                    stage.tool_listener(),
                    GoldenStage::InstallingTools,
                    InstallToolsOptions::default(),
                )
                .await?;
                result.tools.extend(tools.tools.iter().cloned());
                if let Some(h) = tools.homebrew {
                    result.homebrew = Some(h);
                }
                let checks: Vec<String> =
                    tools.tools.iter().filter(|t| t.outcome == "installed").filter_map(|t| manager_check_of(&t.id)).collect();
                ledger.smoke = next_smoke(&ledger.smoke, &[], &join_smoke(&checks));
            }
            let all: Vec<ToolResult> = result.base.iter().flatten().chain(result.tools.iter()).cloned().collect();
            let missing = missing_tools_of(&all);
            ledger.missing_tools = (!missing.is_empty()).then_some(missing);
            mark(&mut ledger, ImportStage::InstallingTools);
        }
        // What the person sees before the first prompt: the rc files call tools and agents, so the shell starts once
        // everything that installs is on the machine, and only on a pass that changed it.
        let mut shell_line: Option<String> = None;
        if let Some(shell) = &imp.shell {
            if ran || harness_ran {
                let res = machine.run(&guarded(&shell_check(shell.shell), SHELL_CHECK_S), run_options(SHELL_CHECK_S, None)).await?;
                let noise: Vec<String> = if res.exit_code.get() == 124 {
                    vec![reason_of(&res, SHELL_CHECK_S as f64)]
                } else {
                    res.stderr.split('\n').filter(|l| !js_trim(l).is_empty()).map(str::to_string).collect()
                };
                if noise.is_empty() {
                    shell_line = Some(format!("{} starts quiet", shell.shell.as_str()));
                    ledger.shell_noise = None;
                } else {
                    let said = if res.exit_code.get() == 124 {
                        noise[0].clone()
                    } else {
                        format!("{}, {}", noise[0], plural(noise.len() as f64, "line"))
                    };
                    shell_line = Some(format!("shell noise: {said}"));
                    ledger.shell_noise = Some(said);
                }
            }
        }
        // The edit runs on every pass that has a plan: an attach re-uploads the volatile ~/.claude.json, which brings
        // every laptop definition back as it was. It does not count as a run for the result.
        let mut edited = false;
        let mut servers: Option<String> = None;
        if let Some(plan) = &imp.mcp {
            edited = true;
            let rows = apply_mcp(&**machine, plan, stage.listener(), &result.tools).await;
            servers = mcp_tally(&rows);
            result.mcp = Some(rows);
        } else if done(&ledger, ImportStage::InstallingMcp) {
            stage.say(GoldenStage::InstallingMcp, ALREADY_APPLIED);
        } else {
            stage.say(GoldenStage::InstallingMcp, "none configured");
        }
        // The stage is applied once the context is on the machine, so an attach after a failed write runs it again.
        let context_was_on = done(&ledger, ImportStage::InstallingMcp);
        if harness_ran || ran || edited || !context_was_on {
            // After the tools, so the document can name what did not install.
            let seen = with_skipped_stages(&result, &ledger, imp, tools_ran, harness_ran);
            let context = apply_machine_context(
                machine,
                ApplyContextOptions { result: Some(seen.to_context()), fetch: opts.fetch.clone(), ..ApplyContextOptions::default() },
            )
            .await;
            result.context = Some(context.context.clone());
            match &context.failure {
                Some(failure) => result.context_failure = Some(failure.clone()),
                None => mark(&mut ledger, ImportStage::InstallingMcp),
            }
            // A refused rewrite over a document that landed leaves the saved result true: the machine still holds it.
            if !ran && (context.failure.is_none() || !context_was_on) {
                if let Some(on_context) = &imp.on_context {
                    on_context(&context.context, context.failure.as_deref());
                }
            }
            // The stage's last detail is what the terminal keeps as its end line.
            let machine_context = format!("machine context: {}", context.summary);
            stage.say(GoldenStage::InstallingMcp, &closing(&[servers.as_deref(), shell_line.as_deref(), Some(machine_context.as_str())]));
        }
        if ran || edited {
            // A builder whose exec died (a full disk did it once) would be sealed and handed off answering nothing.
            let answer = machine.exec("echo ok", exec_options(INLINE_EXEC_MS)).await?;
            if answer.exit_code.get() != 0 || js_trim(&answer.stdout) != "ok" {
                if let Some(report) = &imp.on_result {
                    report(&result);
                }
                return Err(plain(format!(
                    "the machine stopped answering commands after the installs (exit {}); nothing is sealed",
                    answer.exit_code.get()
                )));
            }
        }
    }
    if !only && ran {
        if let Some(report) = &imp.on_result {
            report(&result);
        }
    }
    Ok(Applied { ledger, result })
}

/// golden.ts:825 `installNode`'s answer: the guest's Node major after the step, and why it failed where it did.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeStep {
    pub have_major: u32,
    pub failed: Option<String>,
}

/// The first `<marker><v-prefixed word>` of `text`: the word after `marker`, up to white space.
fn word_after(text: &str, marker: &str) -> Option<String> {
    let mut from = 0;
    while let Some(i) = text[from..].find(marker) {
        let start = from + i + marker.len();
        let word: String = text[start..].chars().take_while(|c| !c.is_whitespace()).collect();
        if word.starts_with('v') && word.len() > 1 {
            return Some(word);
        }
        from = start;
    }
    None
}

/// golden.ts:825. The Node step, once: kept when the guest's major is between the floor and the pinned release's, else
/// the pinned release; a failed install names itself and fails only the agents whose floor the guest's Node does not meet.
pub(crate) async fn install_node(machine: &dyn Machine, node: &NodeInstall, stage: &Stage) -> Result<NodeStep, EngineError> {
    let agents = node.agents.join(", ");
    stage.say(GoldenStage::InstallingHarness, &format!("Node for {agents}"));
    let lines = stage.clone();
    let res = machine
        .run(
            &guarded_harness(&node.cmd),
            run_options(AGENT_TIMEOUT_S, on_line(move |line| lines.say(GoldenStage::InstallingHarness, &format!("Node: {line}")))),
        )
        .await?;
    // `/NODE_HAVE v(\d+)/`: the digits after the first `NODE_HAVE v` that has some.
    let have = {
        let mut found = None;
        let mut from = 0;
        while let Some(i) = res.stdout[from..].find("NODE_HAVE v") {
            let start = from + i + "NODE_HAVE v".len();
            let digits: String = res.stdout[start..].chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty() {
                found = Some(digits);
                break;
            }
            from = start;
        }
        found
    };
    let have_major = have.map_or(0, |d| to_number(&d) as u32);
    let kept = word_after(&res.stdout, "NODE_KEPT ");
    let got = word_after(&res.stdout, "NODE_INSTALLED ");
    if res.exit_code.get() == 0 {
        if let Some(kept) = kept {
            stage.say(GoldenStage::InstallingHarness, &format!("Node {kept} kept; {agents} run on it"));
            return Ok(NodeStep { have_major, failed: None });
        }
        if let Some(got) = got {
            stage.say(GoldenStage::InstallingHarness, &format!("Node {got} installed for {agents} (the base had v{have_major})"));
            let major = to_number(got[1..].split('.').next().unwrap_or("")) as u32;
            return Ok(NodeStep { have_major: major, failed: None });
        }
    }
    let mut shown = res.clone();
    shown.stdout = res.stdout.split('\n').filter(|l| !l.starts_with("NODE_")).collect::<Vec<_>>().join("\n");
    let failed = format!("Node {} did not install: {}", node.version, reason_of(&shown, AGENT_TIMEOUT_S as f64));
    stage.say(GoldenStage::InstallingHarness, &failed);
    Ok(NodeStep { have_major, failed: Some(failed) })
}

/// golden.ts:860.
pub(crate) fn summarize_agents(agents: &[AgentResult]) -> String {
    let ok: Vec<&str> = agents.iter().filter(|a| a.outcome == "installed").map(|a| a.name.as_str()).collect();
    let mut parts: Vec<String> = Vec::new();
    if !ok.is_empty() {
        parts.push(format!("{} installed", ok.join(", ")));
    }
    parts.extend(
        agents
            .iter()
            .filter(|a| a.outcome == "failed")
            .map(|a| format!("{} failed ({})", a.name, a.note.as_deref().unwrap_or("undefined"))),
    );
    parts.extend(
        agents
            .iter()
            .filter(|a| a.outcome == "skipped")
            .map(|a| format!("{} skipped ({})", a.name, a.note.as_deref().unwrap_or("undefined"))),
    );
    parts.join("; ")
}

/// golden.ts:845.
pub(crate) fn missing_tools_of(tools: &[ToolResult]) -> Vec<GoldenMissingTool> {
    tools
        .iter()
        .filter(|t| t.outcome != "installed")
        .map(|t| GoldenMissingTool {
            id: t.id.clone(),
            name: t.label.clone(),
            outcome: if t.outcome == "failed" { GoldenMissingToolOutcome::Failed } else { GoldenMissingToolOutcome::Skipped },
            note: t.note.clone().unwrap_or_else(|| "no reason recorded".to_string()),
        })
        .collect()
}

/// golden.ts:851 `withSkippedStages`: a stage this pass skipped has no outcomes in the result, but what it left off
/// the machine is known: the ledger keeps the tools, the plan the agents and files it set aside.
fn with_skipped_stages(
    result: &ImportResult,
    ledger: &ImportLedger,
    imp: &GoldenImport,
    tools_ran: bool,
    harness_ran: bool,
) -> ImportResult {
    ImportResult {
        tools: if tools_ran {
            result.tools.clone()
        } else {
            ledger
                .missing_tools
                .iter()
                .flatten()
                .map(|m| ToolResult {
                    id: m.id.clone(),
                    label: m.name.clone(),
                    outcome: match m.outcome {
                        GoldenMissingToolOutcome::Failed => "failed",
                        GoldenMissingToolOutcome::Skipped => "skipped",
                    }
                    .to_string(),
                    note: Some(m.note.clone()),
                    ..ToolResult::default()
                })
                .collect()
        },
        agents: if harness_ran {
            result.agents.clone()
        } else {
            imp.skipped_agents
                .iter()
                .map(|a| AgentResult {
                    id: a.id.clone(),
                    name: a.label.clone(),
                    outcome: "skipped".into(),
                    note: Some(a.note.clone()),
                    ms: None,
                })
                .collect()
        },
        files: match (&result.files, &imp.files) {
            (None, Some(f)) => Some(ResultFiles { bytes: 0, skipped: f.skipped.clone(), cut: None, blanked: None }),
            _ => result.files.clone(),
        },
        ..result.clone()
    }
}

impl ImportResult {
    /// The part of the result machine context reads (`golden_seam::ImportResult`).
    pub fn to_context(&self) -> crate::golden_seam::ImportResult {
        use crate::golden_seam as seam;
        let row =
            |t: &ToolResult| seam::ImportRow { id: t.id.clone(), label: t.label.clone(), outcome: t.outcome.clone(), note: t.note.clone() };
        seam::ImportResult {
            recipe_hash: self.recipe_hash.clone(),
            files: self.files.as_ref().map(|f| seam::ImportFiles { bytes: f.bytes, skipped: f.skipped.clone() }),
            tools: self.tools.iter().map(row).collect(),
            agents: self
                .agents
                .iter()
                .map(|a| seam::ImportAgentRow { id: a.id.clone(), name: a.name.clone(), outcome: a.outcome.clone(), note: a.note.clone() })
                .collect(),
            retired: self.retired.as_ref().map(|r| r.iter().map(|g| seam::RetiredRow { id: g.id.clone() }).collect()),
            base: self.base.as_ref().map(|b| b.iter().map(row).collect()),
        }
    }
}

impl ImportResult {
    /// The result as the TS's object writes it: the keys in the order the TS assigns them, absent ones left out.
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::{json, to_value, Map, Value};
        let mut out = Map::new();
        out.insert("recipeHash".into(), json!(self.recipe_hash));
        out.insert("tools".into(), to_value(&self.tools).unwrap_or(Value::Null));
        out.insert("agents".into(), to_value(&self.agents).unwrap_or(Value::Null));
        if let Some(base) = &self.base {
            out.insert("base".into(), to_value(base).unwrap_or(Value::Null));
        }
        if let Some(retired) = &self.retired {
            out.insert("retired".into(), to_value(retired).unwrap_or(Value::Null));
        }
        if let Some(files) = &self.files {
            out.insert("files".into(), to_value(files).unwrap_or(Value::Null));
        }
        if let Some(h) = &self.homebrew {
            out.insert("homebrew".into(), json!({ "tag": h.tag, "commit": h.commit }));
        }
        if let Some(mcp) = &self.mcp {
            out.insert("mcp".into(), to_value(mcp).unwrap_or(Value::Null));
        }
        if let Some(context) = &self.context {
            let rows: Vec<Value> = context
                .iter()
                .map(|c| {
                    let mut row = Map::new();
                    row.insert("agent".into(), json!(c.agent));
                    row.insert("outcome".into(), to_value(c.outcome).unwrap_or(Value::Null));
                    if let Some(p) = &c.path {
                        row.insert("path".into(), json!(p));
                    }
                    if let Some(n) = &c.note {
                        row.insert("note".into(), json!(n));
                    }
                    row.insert("skill".into(), json!(c.skill));
                    Value::Object(row)
                })
                .collect();
            out.insert("context".into(), Value::Array(rows));
        }
        if let Some(f) = &self.context_failure {
            out.insert("contextFailure".into(), json!(f));
        }
        Value::Object(out)
    }
}
