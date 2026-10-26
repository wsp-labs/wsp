// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:578-865: the import stages on a builder, skipping what the ledger says is already there for the same
//! recipe hash.
use std::sync::Arc;

use super::types::{AgentResult, Applied, ApplyImportOptions, ImportResult};
use crate::errors::{EngineError, RetryClock};
use crate::golden_seam::{NodeInstall, ToolResult};
use crate::machine::Machine;

/// golden.ts:598. Runs the import stages and the harness on a builder. Files and upload fail the build; each tool and
/// agent fails alone and is named in the stage detail.
pub async fn apply_golden_import(
    _machine: &Arc<dyn Machine>,
    _opts: ApplyImportOptions,
    _clock: &dyn RetryClock,
) -> Result<Applied, EngineError> {
    todo!("golden.ts:598")
}

/// golden.ts:825 `installNode`'s answer: the guest's Node major after the step, and why it failed where it did.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeStep {
    pub have_major: u32,
    pub failed: Option<String>,
}

/// golden.ts:825. The Node step, once.
pub(crate) async fn install_node(_machine: &dyn Machine, _node: &NodeInstall) -> NodeStep {
    todo!("golden.ts:825")
}

/// golden.ts:860.
pub(crate) fn summarize_agents(_agents: &[AgentResult]) -> String {
    todo!("golden.ts:860")
}

/// golden.ts:845.
pub(crate) fn missing_tools_of(_tools: &[ToolResult]) -> Vec<wsp_wire::views::golden_image::GoldenMissingTool> {
    todo!("golden.ts:845")
}

impl ImportResult {
    /// The part of the result machine context reads (`golden_seam::ImportResult`).
    pub fn to_context(&self) -> crate::golden_seam::ImportResult {
        todo!("golden.ts:547")
    }
}
