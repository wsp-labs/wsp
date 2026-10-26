// SPDX-License-Identifier: AGPL-3.0-only
//! golden-tools.ts:495-545: what a build does for the ticked agents' plugins.
use async_trait::async_trait;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::GoldenStage;
use wsp_wire::wire::machine_link::ExecResult;

use wsp_catalog::roads::NODE_PATH_LINE;

use crate::exec_detached::INLINE_EXEC_MS;
use crate::golden_seam::{closing, guard_deadline_ms, guarded, reason_of, PluginSeedPlan, PluginsPlan, SeedRead, StageListener};
use crate::machine::{ExecOptions, Machine, RunOptions};

/// A failed call as the TS's `.catch((e): ExecResult => ({ exitCode: -1, stdout: "", stderr: message }))` reads it.
fn thrown(e: crate::errors::EngineError) -> ExecResult {
    ExecResult { exit_code: (-1i64).into(), stdout: String::new(), stderr: e.to_string() }
}

/// golden-tools.ts:509 `applyPlugins`. Takes off what an older image copied, whether or not there is a plugin to seed,
/// then builds the seed. Both fail soft: the image is sealed without what did not take, and a turn there fetches as it
/// did before.
pub async fn apply_plugins(machine: &dyn Machine, plan: &PluginsPlan, on_stage: StageListener) {
    if !plan.dropped.is_empty() {
        let quoted: Vec<String> = plan.dropped.iter().map(|p| shell_quote(p)).collect();
        let gone = machine
            .exec(&format!("rm -f {}", quoted.join(" ")), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() })
            .await
            .unwrap_or_else(thrown);
        if gone.exit_code.get() != 0 {
            let why = match gone.stderr.trim() {
                "" => format!("exit {}", gone.exit_code.get()),
                t => t.to_string(),
            };
            on_stage(GoldenStage::InstallingHarness, Some(&format!("left on the image: {} ({why})", plan.dropped.join(", "))));
        }
    }
    if let Some(seed) = &plan.seed {
        build_plugin_seed(machine, seed, on_stage).await;
    }
}

/// How long one plugin gets: its marketplace's clone and the plugin's own files.
const SEED_PLUGIN_S: u64 = 300;

struct MachineRead<'a>(&'a dyn Machine, &'a str);

#[async_trait(?Send)]
impl SeedRead for MachineRead<'_> {
    async fn read(&self, rel: &str) -> Option<String> {
        let res = self
            .0
            .exec(
                &format!("cat {}", shell_quote(&format!("{}/{rel}", self.1))),
                ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() },
            )
            .await;
        match res {
            Ok(r) if r.exit_code.get() == 0 => Some(r.stdout),
            _ => None,
        }
    }
}

/// golden-tools.ts:522 `buildPluginSeed`. Builds each plugin into the seed, then the links the agent finds them by. One
/// that does not build is said and left out, and the image is sealed without it.
pub async fn build_plugin_seed(machine: &dyn Machine, seed: &PluginSeedPlan, on_stage: StageListener) {
    let mut built: Vec<String> = Vec::new();
    let mut left: Vec<String> = Vec::new();
    for p in &seed.plugins {
        on_stage(GoldenStage::InstallingHarness, Some(&format!("plugin {}", p.label)));
        let res = machine
            .run(
                &guarded(&format!("{NODE_PATH_LINE}\n{}", p.cmd), SEED_PLUGIN_S),
                RunOptions { deadline_ms: guard_deadline_ms(SEED_PLUGIN_S), ..RunOptions::default() },
            )
            .await
            .unwrap_or_else(thrown);
        if res.exit_code.get() == 0 {
            built.push(p.label.clone());
        } else {
            let why = p.asked.as_ref().and_then(|f| f(&res.stdout)).unwrap_or_else(|| reason_of(&res, SEED_PLUGIN_S as f64));
            left.push(format!("{} ({why})", p.label));
        }
    }
    if !built.is_empty() {
        let pairs = seed.links.links(&MachineRead(machine, &seed.dir)).await;
        let links: Vec<String> = pairs
            .iter()
            .map(|(link, target)| format!("ln -sfn {} {}", shell_quote(target), shell_quote(&format!("{}/{link}", seed.dir))))
            .collect();
        if !links.is_empty() {
            let linked = machine
                .exec(&links.join("\n"), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() })
                .await
                .unwrap_or_else(thrown);
            if linked.exit_code.get() != 0 {
                let why = match linked.stderr.trim() {
                    "" => format!("exit {}", linked.exit_code.get()),
                    t => t.to_string(),
                };
                left.push(format!("the links into {} ({why})", seed.dir));
            }
        }
    }
    let built_part = (!built.is_empty()).then(|| format!("plugins built in: {}", built.join(", ")));
    let left_part = (!left.is_empty()).then(|| format!("left out: {}", left.join(", ")));
    on_stage(GoldenStage::InstallingHarness, Some(&closing(&[built_part.as_deref(), left_part.as_deref()])));
}
