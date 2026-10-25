// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/codex.ts. Spec: spec/project_state_agents.md.
use async_trait::async_trait;

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;
use crate::project_state::resolver::{MovedState, ProjectStateResolver, SharedStore};

/// codex.ts:9.
pub const SESSIONS_DIR: &str = "sessions";
/// codex.ts:11. The index every project's threads sit in.
pub const INDEX: &str = "state_5.sqlite";

/// codex.ts:18. The rollout under this home: the tail after the last `/sessions/` of the recorded path, under
/// `<home>/sessions` (`under_home`); `None` without that segment or where the tail climbs out.
pub fn rollout_under(home: &str, recorded: &str) -> Option<String> {
    todo!()
}

/// codex.ts:36. carry transcript-only; states thread index, rollout transcript; roots state_5.sqlite, sessions; shared store the index filtered to threads under cwd.
pub struct CodexResolver;

#[async_trait(?Send)]
impl ProjectStateResolver for CodexResolver {
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
    fn listing(&self, home: &str, path: &str) -> Option<Vec<String>> {
        todo!()
    }
    fn shared(&self) -> Option<SharedStore> {
        todo!()
    }
    fn has_merge(&self) -> bool {
        todo!()
    }
    async fn merge(&self, home: &str, from: &str, to: &str, guest_home: &str) -> Result<Option<String>> {
        todo!()
    }
}
