// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:983-1060 and 1428-1453: the first half of a golden (a fresh machine with the harness on it), the npm
//! folder read, and the scripted pipeline that runs both halves.
use super::types::{BuildGoldenOptions, Builder, BuiltGolden, GoldenError, PrepareBuilderOptions};
use crate::errors::{EngineError, RetryClock};
use crate::machine::Machine;

/// golden.ts:983. What a builder runs first, so the builder and every machine forked from its image keep command lines
/// off disk.
pub const QUIET_LOGS: &str = "";

/// golden.ts:1065. Makes the pinned agents' folder afresh with a link to the native claude the agents stage installed.
pub fn claude_link_line() -> String {
    todo!("golden.ts:1065")
}

/// golden.ts:378. How long a builder may sit with no API activity before it is killed.
pub use crate::golden_seam::BUILDER_IDLE_MS;

/// golden.ts:989. Boots a fresh machine, installs the harness and, with an import, the person's files, agents and tools.
pub async fn prepare_builder(_opts: PrepareBuilderOptions, _clock: &dyn RetryClock) -> Result<Builder, GoldenError> {
    todo!("golden.ts:989")
}

/// golden.ts:1055. The folder npm's global installs put their commands in on a builder, where the tools PATH names no
/// such folder.
pub async fn npm_bin_of(_machine: &dyn Machine) -> Result<Option<String>, EngineError> {
    todo!("golden.ts:1055")
}

/// golden.ts:1429. The scripted pipeline: prepare then seal, no one in between.
pub async fn build_golden(_opts: BuildGoldenOptions, _clock: &dyn RetryClock) -> Result<BuiltGolden, GoldenError> {
    todo!("golden.ts:1429")
}
