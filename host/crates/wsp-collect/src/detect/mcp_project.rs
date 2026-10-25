// SPDX-License-Identifier: AGPL-3.0-only
//! The servers a project carries to another computer: of those a turn in its folder gets, the ones its checkout does
//! not bring along. Port of packages/collect/src/detect/mcp-project.ts at a3b870a6e; the spec is
//! spec/detect_turn_servers.md.
use wsp_catalog::catalog::AgentEntry;
use wsp_catalog::mcp_launch::{Checkout, TurnServer};

use crate::host::Host;
use crate::manifest::ManifestEntry;

/// mcp-project.ts:14.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectServer {
    pub agent: &'static AgentEntry,
    pub turn: TurnServer,
    pub row: ManifestEntry,
}

/// Why a server of the folder stays here though a turn there reads it.
pub const SWITCHED_OFF_HERE: &str = "switched off for this folder here";

// detect/mcp-project.ts:25
// /** Why a project's server stays here: its command or a path it runs against sits in the home outside the project, and
//  * nothing carries it, so it is not on that computer. */
// export const outsideProjectLine = (paths: readonly string[]): string =>
//   `runs against ${paths.join(", ")}, outside the project, which nothing carries to that computer`;
pub fn outside_project_line(paths: &[String]) -> String {
    todo!()
}

/// The TS options object `{ agents, keep? }`.
#[derive(Debug, Clone, Default)]
pub struct ProjectMcpOptions {
    /// TS `McpAgent`s: catalog entries with `mcp` set.
    pub agents: Vec<&'static AgentEntry>,
    /// Untracked files the project keeps, relative to the folder.
    pub keep: Vec<String>,
}

/// The TS answer `{ servers, read, checkout }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectMcp {
    pub servers: Vec<ProjectServer>,
    /// Each agent that reads a project file in the folder at all.
    pub read: Vec<String>,
    pub checkout: Checkout,
}

// detect/mcp-project.ts:23
// const inside = (path: string, folder: string): boolean => path.startsWith(`${folder.replace(/\/+$/, "")}/`);
// detect/mcp-project.ts:30
// /** The home paths a server runs against, its command among them, that sit outside the checkout `top`: what would not
//  * be on the computer the project goes to. ~/.local/bin is left out, since a command there is found there by name. */
// function outsidePaths(server: TurnServer["server"], home: string, top: string): string[] {
//   const t = server.transport;
//   if (t.kind === "http") return [];
//   const command = t.command.startsWith("~/") ? `${home}${t.command.slice(1)}` : t.command;
//   const ran = command.startsWith(`${home}/`) && !command.startsWith(`${home}/.local/bin/`) ? [tilde(home, command)] : [];
//   const at = (p: string): string => (p.startsWith("~/") ? `${home}${p.slice(1)}` : p);
//   return [...new Set([...ran, ...homePaths(server, home)])].filter(p => !inside(at(p), top) && at(p) !== top);
// }
//
// /** The files of `files` inside the checkout `top` that it brings along: tracked by git, or among the untracked files
//  * the project keeps, which `keep` names from `folder`. */
// async function travelling(host: Host, folder: string, top: string, files: readonly string[], keep: readonly string[]): Promise<Set<string>> {
//   const rel = files.filter(f => inside(f, top)).map(f => posix.relative(top, f));
//   if (rel.length === 0) return new Set();
//   const tracked = (await host.exec.run("git", ["-C", top, "ls-files", "--", ...rel])) ?? "";
//   const kept = new Set([...tracked.split("\n").filter(l => l !== ""), ...rel.filter(r => keep.includes(posix.relative(folder, posix.join(top, r))))]);
//   return new Set([...kept].map(r => posix.join(top, r)));
// }
//
// /** The servers a turn of each agent in `folder` gets that the folder's checkout does not bring along, one per name as
//  * the agent reads them, each with the scan's row: only an agent whose servers wsp knows where to carry. A server the
//  * folder switches off, or one that runs against a home path outside the project, is a row that stays here. `read` names each agent that reads a project file in the folder at
//  * all, carried or not, which for Codex is one whose own file trusts it; `checkout` is the folder's. */
// export async function detectProjectMcp(host: Host, folder: string, o: { agents: readonly McpAgent[]; keep?: readonly string[] }): Promise<{ servers: ProjectServer[]; read: string[]; checkout: Checkout }> {
//   const tokens = await mcpRemoteTokens(host);
//   const checkout = await readCheckout(host, folder);
//   const out: ProjectServer[] = [];
//   const read: string[] = [];
//   for (const agent of o.agents) {
//     if (LAUNCH_SERVER_ROADS[agent.id] === undefined) continue;
//     const servers = (await readTurnServers(host, agent, folder, { checkout })).filter(s => s.scope !== "user");
//     if (servers.some(s => s.scope === "project")) read.push(agent.id);
//     const brought = await travelling(host, folder, checkout.top ?? folder, [...new Set(servers.filter(s => s.scope === "project").map(s => s.file))], o.keep ?? []);
//     const seen = new Set<string>();
//     for (const turn of servers) {
//       if (seen.has(turn.server.name)) continue;
//       seen.add(turn.server.name);
//       if (turn.scope === "project" && brought.has(turn.file)) continue;
//       const row = await mcpServerRow(host, agent, { ...turn.server, scope: "user" }, tokens);
//       const outside = outsidePaths(turn.server, host.home, checkout.top ?? folder);
//       const reason = turn.server.disabled === true ? SWITCHED_OFF_HERE : outside.length > 0 ? outsideProjectLine(outside) : undefined;
//       out.push({ agent, turn, row: reason === undefined ? row : { ...row, default: "skip", reason } });
//     }
//   }
//   return { servers: out, read, checkout };
// }
pub async fn detect_project_mcp(host: &Host, folder: &str, o: &ProjectMcpOptions) -> ProjectMcp {
    todo!()
}
