// SPDX-License-Identifier: AGPL-3.0-only
//! The agents rung's agent rows: the catalog's agents in its order, then Aider. Port of
//! packages/collect/src/detect/agents.ts at a3b870a6e; the spec is spec/detect_agents.md.
use wsp_catalog::hooks::HookCarry;
use wsp_words::exit::HostError;

use crate::host::Host;
use crate::manifest::ManifestEntry;

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
// detect/agents.ts:22
// /** Aider is not a catalog agent (its project state has no measured resolver, so wsp does not ship it); its row stays for laptops that have it. */
// const AIDER: Agent = { id: "aider", label: "Aider", bin: "aider", config: ["~/.aider.conf.yml", "~/.aider.model.settings.yml", "~/.aider.model.metadata.json"] };
//
// /** The catalog's agents in its order, each with the config that travels, then Aider. */
// export const AGENTS: readonly Agent[] = [...CATALOG_AGENTS.map((a): Agent => ({ id: a.id, label: a.name, bin: a.bin, config: a.configPaths, ...(a.volatile !== undefined ? { volatile: a.volatile } : {}), ...(a.hooks !== undefined ? { hooks: a.hooks } : {}) })), AIDER];
pub fn agents() -> Vec<Agent> {
    todo!()
}

/// `agents/<id>` (agents.ts:29).
pub fn agent_row_id(id: &str) -> String {
    todo!()
}

// detect/agents.ts:31
// export async function detectAgents(host: Host): Promise<ManifestEntry[]> {
//   const rows: ManifestEntry[] = [];
//   for (const a of AGENTS) {
//     const p = await presenceOf(host, { configPaths: [...a.config, ...(await hookScripts(host, a))], bin: a.bin });
//     if (p === undefined) continue;
//     rows.push(entry({ rung: "agents", id: agentRowId(a.id), label: a.label, paths: p.paths, bytes: p.bytes, ...(a.volatile !== undefined ? { volatile: a.volatile } : {}) }));
//   }
//   return rows;
// }
pub async fn detect_agents(host: &Host) -> Result<Vec<ManifestEntry>, HostError> {
    todo!()
}

/// The files the agent's hooks run that no config path of its own already holds, `~/`-relative; a hooks file that
/// cannot be read gives none. An `Err` from the catalog's `hook_files` is returned as is: the TS throw is caught
/// nowhere, so it fails the whole agents rung (coordinator's ruling on #2446).
// detect/agents.ts:41
// /** The files the agent's hooks run that no config path of its own already holds, `~/`-relative. */
// async function hookScripts(host: Host, a: Agent): Promise<string[]> {
//   const text = a.hooks === undefined ? undefined : await host.fs.readText(expand(host, a.hooks.file));
//   if (a.hooks === undefined || text === undefined) return [];
//   const out: string[] = [];
//   for (const abs of hookFiles(a.hooks, text, host.home)) {
//     const rel = tilde(host.home, abs);
//     if (a.config.some(c => rel === c || rel.startsWith(`${c}/`))) continue;
//     if ((await host.fs.stat(abs))?.kind === "file") out.push(rel);
//   }
//   return out;
// }
pub async fn hook_scripts(host: &Host, a: &Agent) -> Result<Vec<String>, HostError> {
    todo!()
}
