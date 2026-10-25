// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/index.ts. Spec: spec/project_state_core.md.
//!
//! The registry of project-state resolvers by catalog agent id, and the core that walks the catalog's agents and
//! applies each one. Nothing here switches on an agent id.
pub mod claude;
pub mod codex;
pub mod gemini;
pub mod hermes;
pub mod listing;
pub mod merge;
pub mod opencode;
pub mod pi;
pub mod py;
pub mod resolver;
pub mod sqlite;

use std::collections::HashSet;
use std::sync::Arc;

use indexmap::IndexMap;

use wsp_wire::views::project_bundle::ProjectAgent;

use crate::errors::Result;
use crate::machine::Machine;

pub use listing::{parse_state_listing, store_copy, StateListing, UnreadStore};
pub use merge::{parse_merge_output, MergeOutput};
pub use resolver::{files_under, inside_folder, resolve_project_path, under_home, MovedState, ProjectStateResolver, SharedStore};
pub use wsp_words::project_path::under_project;

/// index.ts:27. claude, codex, gemini, hermes, opencode, pi, in that order, keyed by `agent()`; the same map on every
/// call (built once).
pub fn project_state_resolvers() -> &'static IndexMap<&'static str, Arc<dyn ProjectStateResolver>> {
    todo!()
}

/// index.ts:38. The agent's module's key for `resolve_project_path(path)`; `None` for an agent with no module or no key.
pub fn project_state_key(agent: &str, path: &str) -> Option<String> {
    todo!()
}

/// index.ts:42.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectStateMove {
    /// The project's old absolute path, as the agents stored it.
    pub from: String,
    /// Where the project now sits.
    pub to: String,
    /// Each agent's home by catalog id.
    pub homes: IndexMap<String, String>,
}

/// index.ts:52.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentOutcome {
    Moved(Vec<MovedState>),
    Nothing,
    TranscriptOnly,
    Failed(String),
}

/// index.ts:52.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMoveReport {
    pub agent: String,
    pub outcome: AgentOutcome,
}

/// index.ts:59. One report per agent in `agents`' order (the catalog's agent ids when `None`). Rules in
/// spec/project_state_core.md; one agent's failure is its row, the walk goes on.
pub async fn move_project_state(mv: &ProjectStateMove, agents: Option<&[String]>) -> Vec<AgentMoveReport> {
    todo!()
}

/// index.ts:91. Every catalog agent's home by id, catalog order: the folder its `state_home_env` names where `vars`
/// sets it to a non-empty value, else `join(home_dir, state_home)`.
pub fn agent_homes(home_dir: &str, vars: Option<&IndexMap<String, String>>) -> IndexMap<String, String> {
    todo!()
}

/// index.ts:100. `agent_homes`' entry for the id, else `join(home_dir, "." + agent_id)`.
pub fn agent_home(home_dir: &str, agent_id: &str, vars: Option<&IndexMap<String, String>>) -> String {
    todo!()
}

/// index.ts:105. Every catalog agent's home on the guest: its guest state home, else `join(GUEST_HOME, state_home)`.
pub fn guest_agent_homes() -> IndexMap<String, String> {
    todo!()
}

/// index.ts:118. The one command the machine runs to name the paths a trip pulls. An id the catalog does not know fails
/// with `unknown_agent_line(id, catalog ids)` before anything is built. Empty where no agent qualifies; else `set -e`
/// then one `python3 -c <q script>` per agent, newline-joined.
pub fn state_listing(homes: &IndexMap<String, String>, path: &str, scratch: &str, ids: Option<&[String]>) -> Result<String> {
    todo!()
}

/// index.ts:136. One row per agent with sessions for the folder, `agents`' order (the catalog's ids and names when
/// `None`). Rules in spec/project_state_core.md.
pub async fn count_project_state(path: &str, homes: &IndexMap<String, String>, agents: Option<&[(String, String)]>) -> Vec<ProjectAgent> {
    todo!()
}

/// index.ts:156. Which of the named catalog agents are on the machine: golden-tools' `missingCommands` over their bins.
/// No named agent asks nothing. A read that failed is `could not see which agents are on the machine: <failed>`.
pub async fn agents_on_machine(machine: &dyn Machine, ids: &[String]) -> Result<HashSet<String>> {
    todo!()
}
