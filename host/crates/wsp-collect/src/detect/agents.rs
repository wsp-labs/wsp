// SPDX-License-Identifier: AGPL-3.0-only
//! The agents rung's agent rows: the catalog's agents in its order, then Aider. Port of
//! packages/collect/src/detect/agents.ts at a3b870a6e; the spec is spec/detect_agents.md.
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::hooks::{hook_files, HookCarry};
use wsp_words::exit::HostError;

use crate::detect::common::{entry, EntrySpec, RowOptions};
use crate::detect::presence::presence_of;
use crate::host::{expand, tilde, Host, StatKind};
use crate::manifest::{ManifestEntry, Rung};

/// agents.ts:8.
#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    pub id: String,
    pub label: String,
    pub bin: String,
    /// Config that travels with the agent, `~`-relative.
    pub config: Vec<String>,
    pub volatile: Option<Vec<String>>,
    pub hooks: Option<HookCarry>,
}

/// The catalog's agents in its order (id, name, bin, configPaths, volatile, hooks), then Aider (agents.ts:23-26).
pub fn agents() -> Vec<Agent> {
    let mut out: Vec<Agent> = catalog_agents()
        .into_iter()
        .map(|a| Agent {
            id: a.id.clone(),
            label: a.name.clone(),
            bin: a.bin.clone(),
            config: a.config_paths.clone(),
            volatile: a.volatile.clone(),
            hooks: a.hooks,
        })
        .collect();
    out.push(Agent {
        id: "aider".into(),
        label: "Aider".into(),
        bin: "aider".into(),
        config: vec!["~/.aider.conf.yml".into(), "~/.aider.model.settings.yml".into(), "~/.aider.model.metadata.json".into()],
        volatile: None,
        hooks: None,
    });
    out
}

/// `agents/<id>` (agents.ts:29).
pub fn agent_row_id(id: &str) -> String {
    format!("agents/{id}")
}

pub async fn detect_agents(host: &Host) -> Result<Vec<ManifestEntry>, HostError> {
    let mut rows = Vec::new();
    for a in agents() {
        let mut config_paths = a.config.clone();
        config_paths.extend(hook_scripts(host, &a).await?);
        let Some(p) = presence_of(host, &config_paths, &a.bin).await else {
            continue;
        };
        rows.push(entry(EntrySpec {
            rung: Rung::Agents,
            id: agent_row_id(&a.id),
            label: a.label.clone(),
            paths: p.paths,
            bytes: p.bytes,
            opt: RowOptions { volatile: a.volatile.clone(), ..Default::default() },
        }));
    }
    Ok(rows)
}

/// The files the agent's hooks run that no config path of its own already holds, `~/`-relative; a hooks file that
/// cannot be read gives none. An `Err` from the catalog's `hook_files` is returned as is: the TS throw is caught
/// nowhere, so it fails the whole agents rung (coordinator's ruling on #2446).
pub async fn hook_scripts(host: &Host, a: &Agent) -> Result<Vec<String>, HostError> {
    let Some(hooks) = a.hooks else {
        return Ok(Vec::new());
    };
    let Some(text) = host.fs.read_text(&expand(&host.home, hooks.file())).await else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for abs in hook_files(hooks, &text, &host.home).map_err(|e| HostError::new(e.message))? {
        let rel = tilde(&host.home, &abs);
        if a.config.iter().any(|c| rel == *c || rel.starts_with(&format!("{c}/"))) {
            continue;
        }
        if host.fs.stat(&abs).await.is_some_and(|s| s.kind == StatKind::File) {
            out.push(rel);
        }
    }
    Ok(out)
}
