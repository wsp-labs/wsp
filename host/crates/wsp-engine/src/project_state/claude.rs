// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/claude.ts. Spec: spec/project_state_agents.md.
use std::path::Path;

use async_trait::async_trait;
use serde_json::json;

use wsp_wire::views::project_bundle::ProjectCarry;
use wsp_words::project_path::{claude_memory_dir, claude_project_key};

use crate::errors::Result;
use crate::project_state::listing::keyed_step;
use crate::project_state::py::py_data;
use crate::project_state::resolver::{join_posix, keyed_entries, keyed_sessions, move_keyed_directories, MovedState, ProjectStateResolver};

/// claude.ts:10. The key rule for the machine, a python lambda; pinned by the oracle.
pub fn key_py() -> String {
    format!("lambda p: re.sub({}, \"-\", p)", py_data(&json!("[^A-Za-z0-9]")))
}
/// claude.ts:11.
pub const PROJECTS: &str = "projects";

/// claude.ts:14. carry moves; states session transcripts, auto memory; roots projects; key claude_project_key; listing one keyed step over <home>/projects.
pub struct ClaudeResolver;

#[async_trait(?Send)]
impl ProjectStateResolver for ClaudeResolver {
    fn agent(&self) -> &'static str {
        "claude"
    }
    fn states(&self) -> Vec<&'static str> {
        vec!["session transcripts", "auto memory"]
    }
    fn roots(&self) -> Vec<&'static str> {
        vec![PROJECTS]
    }
    fn carry(&self) -> ProjectCarry {
        ProjectCarry::Moves
    }
    async fn move_state(&self, home: &str, from: &str, to: &str) -> Result<Vec<MovedState>> {
        let moved = move_keyed_directories(&join_posix(&[home, PROJECTS]), claude_project_key, from, to).await?;
        if moved.changed == 0 {
            return Ok(vec![]);
        }
        let mut out = vec![MovedState { state: "session transcripts".into(), files: moved.files, changed: moved.changed, skipped: None }];
        let memory = claude_memory_dir(home, &claude_project_key(to));
        if Path::new(&memory).exists() {
            out.push(MovedState { state: "auto memory".into(), files: vec![memory], changed: 1, skipped: None });
        }
        Ok(out)
    }
    async fn sessions(&self, home: &str, path: &str) -> Result<u64> {
        keyed_sessions(&join_posix(&[home, PROJECTS]), claude_project_key, path).await
    }
    async fn entries(&self, home: &str, path: &str) -> Result<Vec<String>> {
        keyed_entries(&join_posix(&[home, PROJECTS]), claude_project_key, path).await
    }
    fn key(&self, path: &str) -> Option<String> {
        Some(claude_project_key(path))
    }
    fn listing(&self, home: &str, _path: &str) -> Option<Vec<String>> {
        Some(vec![keyed_step(&join_posix(&[home, PROJECTS]), &key_py())])
    }
}
