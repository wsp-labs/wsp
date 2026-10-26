// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:1322-1426: the recipe delta on a fork of the golden, or on the kept builder.
use std::sync::Arc;

use super::types::{Applied, ApplyDeltaOptions, Builder, GoldenDelta, GoldenError, UpgradeBuilderOptions};
use crate::errors::{EngineError, RetryClock};
use crate::machine::Machine;

/// golden.ts:1326. Runs the delta through the import stages. Nothing is taken off the machine: a row the recipe
/// dropped keeps its bytes and rides the version's retired list.
pub async fn apply_delta(
    _machine: &Arc<dyn Machine>,
    _delta: GoldenDelta,
    _opts: ApplyDeltaOptions,
    _clock: &dyn RetryClock,
) -> Result<Applied, EngineError> {
    todo!("golden.ts:1326")
}

/// golden.ts:1375. Forks the golden into a fresh first-life machine and applies the delta on it: a builder to seal as
/// the next version.
pub async fn upgrade_builder(_opts: UpgradeBuilderOptions, _clock: &dyn RetryClock) -> Result<Builder, GoldenError> {
    todo!("golden.ts:1375")
}
