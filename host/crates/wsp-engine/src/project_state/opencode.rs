// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/opencode.ts. Spec: spec/project_state_agents.md.
use async_trait::async_trait;

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;
use crate::project_state::resolver::{MovedState, ProjectStateResolver, SharedStore};

/// opencode.ts:12.
pub const DB: &str = "opencode.db";

/// opencode.ts:26. carry transcript-only; states project, project directories, sessions; roots opencode.db; shared store project, project_directory, session, message, part; entries always empty.
pub struct OpencodeResolver;

#[async_trait(?Send)]
impl ProjectStateResolver for OpencodeResolver {
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
