// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/pi.ts. Spec: spec/project_state_agents.md.
use async_trait::async_trait;

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;
use crate::project_state::resolver::{MovedState, ProjectStateResolver, SharedStore};

/// pi.ts:9. `--`, the path without its leading `/` with every `/`, `\\` and `:` a `-`, then `--`.
pub fn pi_project_key(path: &str) -> String {
    todo!()
}

/// pi.ts:11. The key rule for the machine; pinned by the oracle.
pub fn key_py() -> String {
    todo!()
}

/// pi.ts:12.
pub const SESSIONS: &str = "sessions";

/// pi.ts:15. carry moves; states sessions; roots sessions; no key; listing one keyed step over <home>/sessions.
pub struct PiResolver;

#[async_trait]
impl ProjectStateResolver for PiResolver {
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
}
