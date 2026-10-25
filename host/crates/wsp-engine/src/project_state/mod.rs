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
use std::path::Path;
use std::sync::Arc;

use indexmap::IndexMap;

use wsp_catalog::catalog::{catalog_agents, AgentEntry};
use wsp_catalog::roads::GUEST_HOME;
use wsp_wire::types::daemon_contract::TOOLS_PATH;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::project_bundle::ProjectAgent;
use wsp_words::words::thread::unknown_agent_line;

use crate::errors::{EngineError, Result};
use crate::golden_seam::missing_commands;
use crate::machine::Machine;
use crate::project_state::claude::ClaudeResolver;
use crate::project_state::codex::CodexResolver;
use crate::project_state::gemini::GeminiResolver;
use crate::project_state::hermes::HermesResolver;
use crate::project_state::listing::{filter_step, list_script, ROOTS_STEP};
use crate::project_state::opencode::OpencodeResolver;
use crate::project_state::pi::PiResolver;
use crate::project_state::resolver::join_posix;

pub use listing::{parse_state_listing, store_copy, StateListing, UnreadStore};
pub use merge::{parse_merge_output, MergeOutput};
pub use resolver::{files_under, inside_folder, resolve_project_path, under_home, MovedState, ProjectStateResolver, SharedStore};
pub use wsp_words::project_path::under_project;

thread_local! {
    static RESOLVERS: &'static IndexMap<&'static str, Arc<dyn ProjectStateResolver>> = Box::leak(Box::new(build_resolvers()));
}

fn build_resolvers() -> IndexMap<&'static str, Arc<dyn ProjectStateResolver>> {
    let modules: [Arc<dyn ProjectStateResolver>; 6] = [
        Arc::new(ClaudeResolver),
        Arc::new(CodexResolver),
        Arc::new(GeminiResolver),
        Arc::new(HermesResolver),
        Arc::new(OpencodeResolver),
        Arc::new(PiResolver),
    ];
    modules.into_iter().map(|m| (m.agent(), m)).collect()
}

fn catalog_ids() -> Vec<String> {
    catalog_agents().iter().map(|a| a.id.clone()).collect()
}

fn other(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

/// index.ts:27. claude, codex, gemini, hermes, opencode, pi, in that order, keyed by `agent()`; the same map on every
/// call (built once).
pub fn project_state_resolvers() -> &'static IndexMap<&'static str, Arc<dyn ProjectStateResolver>> {
    RESOLVERS.with(|resolvers| *resolvers)
}

/// index.ts:38. The agent's module's key for `resolve_project_path(path)`; `None` for an agent with no module or no key.
pub fn project_state_key(agent: &str, path: &str) -> Option<String> {
    project_state_resolvers().get(agent)?.key(&resolve_project_path(path))
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
    let from = resolve_project_path(&mv.from);
    let to = resolve_project_path(&mv.to);
    let ids: Vec<String> = match agents {
        Some(ids) => ids.to_vec(),
        None => catalog_ids(),
    };
    let mut report = Vec::with_capacity(ids.len());
    for agent in ids {
        let outcome = match project_state_resolvers().get(agent.as_str()) {
            _ if from == to => AgentOutcome::Nothing,
            None => AgentOutcome::TranscriptOnly,
            Some(resolver) => match mv.homes.get(&agent) {
                Some(home) if Path::new(home).exists() => match resolver.move_state(home, &from, &to).await {
                    Ok(moved) if moved.is_empty() => AgentOutcome::Nothing,
                    Ok(moved) => AgentOutcome::Moved(moved),
                    Err(e) => AgentOutcome::Failed(e.to_string()),
                },
                _ => AgentOutcome::Nothing,
            },
        };
        report.push(AgentMoveReport { agent, outcome });
    }
    report
}

/// index.ts:91. Every catalog agent's home by id, catalog order: the folder its `state_home_env` names where `vars`
/// sets it to a non-empty value, else `join(home_dir, state_home)`.
pub fn agent_homes(home_dir: &str, vars: Option<&IndexMap<String, String>>) -> IndexMap<String, String> {
    catalog_agents()
        .into_iter()
        .map(|agent| {
            let own = agent.state_home_env.as_deref().and_then(|name| vars.and_then(|env| env.get(name))).filter(|dir| !dir.is_empty());
            let home = own.cloned().unwrap_or_else(|| join_posix(&[home_dir, &agent.state_home]));
            (agent.id.clone(), home)
        })
        .collect()
}

/// index.ts:100. `agent_homes`' entry for the id, else `join(home_dir, "." + agent_id)`.
pub fn agent_home(home_dir: &str, agent_id: &str, vars: Option<&IndexMap<String, String>>) -> String {
    agent_homes(home_dir, vars).get(agent_id).cloned().unwrap_or_else(|| join_posix(&[home_dir, &format!(".{agent_id}")]))
}

/// index.ts:105. Every catalog agent's home on the guest: its guest state home, else `join(GUEST_HOME, state_home)`.
pub fn guest_agent_homes() -> IndexMap<String, String> {
    catalog_agents()
        .into_iter()
        .map(|agent| {
            let home = agent.guest_state_home.clone().unwrap_or_else(|| join_posix(&[GUEST_HOME, &agent.state_home]));
            (agent.id.clone(), home)
        })
        .collect()
}

/// index.ts:118. The one command the machine runs to name the paths a trip pulls. An id the catalog does not know fails
/// with `unknown_agent_line(id, catalog ids)` before anything is built. Empty where no agent qualifies; else `set -e`
/// then one `python3 -c <q script>` per agent, newline-joined.
pub fn state_listing(homes: &IndexMap<String, String>, path: &str, scratch: &str, ids: Option<&[String]>) -> Result<String> {
    let known = catalog_ids();
    if let Some(unknown) = ids.and_then(|ids| ids.iter().find(|id| !known.contains(*id))) {
        return Err(other(unknown_agent_line(unknown, &known)));
    }
    let mut scripts = Vec::new();
    for agent in catalog_agents() {
        let id = agent.id.as_str();
        let (Some(home), Some(resolver)) = (homes.get(id), project_state_resolvers().get(id)) else {
            continue;
        };
        if ids.is_some_and(|ids| !ids.iter().any(|named| named.as_str() == id)) {
            continue;
        }
        let own = resolver.listing(home, path).unwrap_or_default();
        let steps = match resolver.shared() {
            None => own,
            Some(shared) => vec![filter_step(scratch, home, &shared, &own)],
        };
        let steps = if steps.is_empty() { vec![ROOTS_STEP.to_string()] } else { steps };
        let roots: Vec<String> = resolver.roots().iter().map(|root| root.to_string()).collect();
        scripts.push(list_script(id, home, path, &roots, &steps));
    }
    if scripts.is_empty() {
        return Ok(String::new());
    }
    let mut lines = vec!["set -e".to_string()];
    lines.extend(scripts.iter().map(|script| format!("python3 -c {}", shell_quote(script))));
    Ok(lines.join("\n"))
}

/// index.ts:136. One row per agent with sessions for the folder, `agents`' order (the catalog's ids and names when
/// `None`). Rules in spec/project_state_core.md.
pub async fn count_project_state(path: &str, homes: &IndexMap<String, String>, agents: Option<&[(String, String)]>) -> Vec<ProjectAgent> {
    let root = resolve_project_path(path);
    let catalog: Vec<(String, String)>;
    let agents: &[(String, String)] = match agents {
        Some(named) => named,
        None => {
            catalog = catalog_agents().iter().map(|a| (a.id.clone(), a.name.clone())).collect();
            &catalog
        }
    };
    let mut rows = Vec::new();
    for (agent, name) in agents {
        let (Some(resolver), Some(home)) = (project_state_resolvers().get(agent.as_str()), homes.get(agent)) else {
            continue;
        };
        if !Path::new(home).exists() {
            continue;
        }
        let carry = resolver.carry();
        let counted: Result<Option<(u64, f64)>> = async {
            let sessions = resolver.sessions(home, &root).await?;
            if sessions == 0 {
                return Ok(None);
            }
            let mut bytes = 0.0;
            for file in resolver.entries(home, &root).await? {
                let size = std::fs::metadata(&file).map_err(|e| EngineError::Other { message: e.to_string(), code: None })?.len();
                bytes += size as f64;
            }
            Ok(Some((sessions, bytes)))
        }
        .await;
        match counted {
            Ok(None) => {}
            Ok(Some((sessions, bytes))) => rows.push(ProjectAgent {
                agent: agent.clone(),
                name: name.clone(),
                sessions: JsNumber(sessions as f64),
                bytes: JsNumber(bytes),
                carry,
                error: None,
            }),
            Err(e) => rows.push(ProjectAgent {
                agent: agent.clone(),
                name: name.clone(),
                sessions: JsNumber(0.0),
                bytes: JsNumber(0.0),
                carry,
                error: Some(e.to_string()),
            }),
        }
    }
    rows
}

/// index.ts:156. Which of the named catalog agents are on the machine: golden-tools' `missingCommands` over their bins.
/// No named agent asks nothing. A read that failed is `could not see which agents are on the machine: <failed>`.
pub async fn agents_on_machine(machine: &dyn Machine, ids: &[String]) -> Result<HashSet<String>> {
    let agents: Vec<&AgentEntry> = catalog_agents().into_iter().filter(|a| ids.contains(&a.id)).collect();
    if agents.is_empty() {
        return Ok(HashSet::new());
    }
    let bins: Vec<String> = agents.iter().map(|a| a.bin.clone()).collect();
    let checked = missing_commands(machine, &bins, TOOLS_PATH, None).await?;
    if let Some(failed) = checked.failed {
        return Err(other(format!("could not see which agents are on the machine: {failed}")));
    }
    Ok(agents.iter().filter(|a| !checked.missing.contains(&a.bin)).map(|a| a.id.clone()).collect())
}
