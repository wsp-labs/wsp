// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/claude.ts. Spec: spec/project_state_agents.md.
use async_trait::async_trait;

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;
use crate::project_state::resolver::{MovedState, ProjectStateResolver, SharedStore};

/// claude.ts:10. The key rule for the machine, a python lambda; pinned by the oracle.
pub fn key_py() -> String {
    todo!()
}
/// claude.ts:11.
pub const PROJECTS: &str = "projects";

/// claude.ts:14. carry moves; states session transcripts, auto memory; roots projects; key claude_project_key; listing one keyed step over <home>/projects.
pub struct ClaudeResolver;

#[async_trait(?Send)]
impl ProjectStateResolver for ClaudeResolver {
    fn agent(&self) -> &'static str {
        todo!()
    }
    fn states(&self) -> Vec<&'static str> {
        todo!()
    }
    fn roots(&self) -> Vec<&'static str> {
        todo!()
    }
    fn carry(&self) -> ProjectCarry {
        todo!()
    }
    async fn move_state(&self, home: &str, from: &str, to: &str) -> Result<Vec<MovedState>> {
        todo!()
    }
    async fn sessions(&self, home: &str, path: &str) -> Result<u64> {
        todo!()
    }
    async fn entries(&self, home: &str, path: &str) -> Result<Vec<String>> {
        todo!()
    }
    fn key(&self, path: &str) -> Option<String> {
        todo!()
    }
    fn listing(&self, home: &str, path: &str) -> Option<Vec<String>> {
        todo!()
    }
}
