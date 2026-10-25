// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/gemini.ts. Spec: spec/project_state_agents.md.
use async_trait::async_trait;

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;
use crate::project_state::resolver::{MovedState, ProjectStateResolver, SharedStore};

/// gemini.ts:16.
pub const REGISTRY: &str = "projects.json";
/// gemini.ts:23. The catalog rows a slug names, each with its directory under the home.
pub const SLUG_DIRS: [(&str, &str); 2] = [("project temp dir", "tmp"), ("shell history", "history")];

/// gemini.ts:26. carry transcript-only; states project registry, project temp dir, shell history; roots projects.json, tmp, history; shared store the registry filtered to keys under PATH.
pub struct GeminiResolver;

#[async_trait(?Send)]
impl ProjectStateResolver for GeminiResolver {
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
