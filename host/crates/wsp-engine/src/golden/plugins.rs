// SPDX-License-Identifier: AGPL-3.0-only
//! golden-tools.ts:495-545: what a build does for the ticked agents' plugins.
use crate::golden_seam::{PluginSeedPlan, PluginsPlan, StageListener};
use crate::machine::Machine;

/// golden-tools.ts:509 `applyPlugins`. Takes off what an older image copied, whether or not there is a plugin to seed,
/// then builds the seed. Both fail soft: the image is sealed without what did not take.
pub async fn apply_plugins(_machine: &dyn Machine, _plan: &PluginsPlan, _on_stage: StageListener) {
    todo!("golden-tools.ts:509")
}

/// golden-tools.ts:522 `buildPluginSeed`. Builds each plugin into the seed, then the links the agent finds them by.
pub async fn build_plugin_seed(_machine: &dyn Machine, _seed: &PluginSeedPlan, _on_stage: StageListener) {
    todo!("golden-tools.ts:522")
}
