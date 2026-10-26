// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/pi.ts. Spec: spec/project_state_agents.md.
use async_trait::async_trait;
use serde_json::json;

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;
use crate::project_state::listing::keyed_step;
use crate::project_state::py::py_data;
use crate::project_state::resolver::{join_posix, keyed_entries, keyed_sessions, move_keyed_directories, MovedState, ProjectStateResolver};

/// pi.ts:9. `--`, the path without its leading `/` with every `/`, `\\` and `:` a `-`, then `--`.
pub fn pi_project_key(path: &str) -> String {
    let bare = path.strip_prefix('/').unwrap_or(path);
    format!("--{}--", bare.chars().map(|c| if matches!(c, '/' | '\\' | ':') { '-' } else { c }).collect::<String>())
}

/// pi.ts:11. The key rule for the machine; pinned by the oracle.
pub fn key_py() -> String {
    format!("lambda p: \"--\" + re.sub({}, \"-\", re.sub({}, \"\", p)) + \"--\"", py_data(&json!("[/\\\\:]")), py_data(&json!("^/")))
}

/// pi.ts:12.
pub const SESSIONS: &str = "sessions";

/// pi.ts:15. carry moves; states sessions; roots sessions; no key; listing one keyed step over <home>/sessions.
pub struct PiResolver;

#[async_trait(?Send)]
impl ProjectStateResolver for PiResolver {
    fn agent(&self) -> &'static str {
        "pi"
    }
    fn states(&self) -> Vec<&'static str> {
        vec!["sessions"]
    }
    fn roots(&self) -> Vec<&'static str> {
        vec![SESSIONS]
    }
    fn carry(&self) -> ProjectCarry {
        ProjectCarry::Moves
    }
    async fn move_state(&self, home: &str, from: &str, to: &str) -> Result<Vec<MovedState>> {
        let moved = move_keyed_directories(&join_posix(&[home, SESSIONS]), pi_project_key, from, to).await?;
        if moved.changed == 0 {
            return Ok(vec![]);
        }
        Ok(vec![MovedState { state: "sessions".into(), files: moved.files, changed: moved.changed, skipped: None }])
    }
    async fn sessions(&self, home: &str, path: &str) -> Result<u64> {
        keyed_sessions(&join_posix(&[home, SESSIONS]), pi_project_key, path).await
    }
    async fn entries(&self, home: &str, path: &str) -> Result<Vec<String>> {
        keyed_entries(&join_posix(&[home, SESSIONS]), pi_project_key, path).await
    }
    fn listing(&self, home: &str, _path: &str) -> Option<Vec<String>> {
        Some(vec![keyed_step(&join_posix(&[home, SESSIONS]), &key_py())])
    }
}
