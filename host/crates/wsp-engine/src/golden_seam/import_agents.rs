// SPDX-License-Identifier: AGPL-3.0-only
//! Unit import_agents (spec/golden-import_agents.md): golden-import.ts:1348-1403 and 1422-1534 at a4033dc34. The
//! ticked agents' installers, the one Node step their floors ask for, and the agents as steps of the tools loop.
use indexmap::IndexMap;
use wsp_catalog::catalog::AgentEntry;

use super::{AgentsPlan, PinRead, RecipeEntry, ToolInstall};

/// golden-import.ts:1350 `AgentInstaller`. `road` is the road's name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgentInstaller {
    pub name: String,
    pub install: String,
    pub smoke: String,
    pub road: String,
    pub node: Option<u32>,
    pub pin: Option<PinRead>,
}

/// golden-import.ts:1374.
pub const CURRENT_LTS: u32 = 22;

/// golden-import.ts:1497.
pub const AGENT_NODE_STEP: &str = "agents/node";

// TS golden-import.ts:1368-1371
// /** An agent's installer as the catalog gives it: its road's line, its version check, its Node floor and its pin read. */
// function agentInstaller(a: AgentEntry): AgentInstaller {
//   return { name: a.name, install: installLine(a), smoke: smokeOf(a), road: a.installRoad.road, ...(a.node !== undefined ? { node: a.node } : {}), pin: pinReadOf(a.installRoad, a.bin) };
// }
// TS golden-import.ts:1386-1394
// /** Every installer pins a version; npm checks the registry's integrity hash for each tarball, uv is checksummed
//  * by its release, git checkouts compare the commit. The catalog's agents install by their roads. Aider is not a
//  * catalog agent; its line stays for recipes that tick it: https://aider.chat/docs/install.html, the uv tool line. */
// export function agentInstallers(agents: readonly AgentEntry[]): Record<string, AgentInstaller> {
//   return {
//     ...Object.fromEntries(agents.map(a => [a.id, agentInstaller(a)])),
//     aider: { name: "Aider", install: `${UV_INSTALL}\nuv tool install --force --python 3.12 --with pip aider-chat==0.86.2`, smoke: "aider --version", road: "uv" },
//   };
// }
/// golden-import.ts:1389 `agentInstallers(agents)`: each agent's by its id in the slice's order, then `aider`'s line.
/// An agent whose road gives no install line is a catalog fault: the TS throws at load, and this panics.
pub fn agent_installers(agents: &[AgentEntry]) -> IndexMap<String, AgentInstaller> {
    todo!()
}

// TS golden-import.ts:1376-1384
// /** The major a set of engines floors gets: the lowest pinned major at or above the
//  * floor that is still in active or maintenance support on `now`, else the current
//  * LTS; nothing when the floor is above every pinned major. */
// export function nodeMajorFor(floor: number, now: Date): NodeMajor | undefined {
//   const majors = (Object.keys(NODE_RELEASES).map(Number) as NodeMajor[]).sort((a, b) => a - b);
//   if (floor > majors.at(-1)!) return undefined;
//   const supported = (m: NodeMajor): boolean => now.getTime() < Date.parse(`${NODE_RELEASES[m].eol}T23:59:59Z`);
//   return majors.find(m => m >= floor && supported(m)) ?? CURRENT_LTS;
// }
/// golden-import.ts:1379 `nodeMajorFor(floor, now)`, `now` in ms since the epoch: the lowest pinned major at or above
/// `floor` still supported at `now` (before `<eol>T23:59:59Z`), else `CURRENT_LTS`; `None` above every pinned major.
pub fn node_major_for(floor: u32, now_ms: i64) -> Option<u32> {
    todo!()
}

// TS golden-import.ts:1499-1503
// /** Whether the node on the machine already meets an agents plan's floor, read the way the install script reads it
//  * so the two cannot disagree: the step keeps such a node and installs nothing, so this is also what says the step
//  * has nothing to do on a computer that has been provisioned before. */
// export const nodeFloorCheck = (floor: number): string =>
//   `major="$(node --version 2>/dev/null | sed 's/^v//; s/\\..*//')"; [ "\${major:-0}" -ge ${floor} ]`;
/// golden-import.ts:1502 `nodeFloorCheck`.
pub fn node_floor_check(floor: u32) -> String {
    todo!()
}

// TS golden-import.ts:1396-1403
// export const AGENT_INSTALLERS: Record<string, AgentInstaller> = agentInstallers(CATALOG_AGENTS);
//
// /** The package an installer's npm or uv line puts on the machine, read off the line's pinned spec. */
// const NPM_INSTALL_LINE = /^npm install -g (?:--ignore-scripts )?(\S+?)@\S+$/m;
// const UV_INSTALL_LINE = /^uv tool install .*?([\w.-]+)==[\w.-]+$/m;
//
// /** The Node major the catalog's own row pins, for an install that runs on node and names no floor of its own. */
// const CATALOG_NODE_MAJOR = Number(catalogToolFor("node")?.major?.version ?? 0);
// TS golden-import.ts:1452-1494
// /** The installer a row with a recorded pin gets: the catalog's road fixed to the pin's version where the road takes
//  * one, so a copy installs what the seal read; the catalog's own line where it does not, or where the pin is a
//  * latest mark. */
// function pinnedInstaller(a: AgentEntry, pin: ToolPin | undefined): AgentInstaller {
//   const at = pin === undefined || pin.latest === true ? undefined : roadModule(a.installRoad).at?.(a.installRoad, pin.tag);
//   return agentInstaller(at === undefined ? a : { ...a, installRoad: at });
// }
//
// /** The ticked agents with an installer, in recipe order, from the installers of `agents`, the catalog's by default;
//  * a row carrying a pin installs at it. */
// export function agentInstallsFor(entries: readonly RecipeEntry[], agents: readonly AgentEntry[] = CATALOG_AGENTS, now: Date = new Date()): AgentsPlan {
//   const table = agentInstallers(agents);
//   const out: AgentsPlan = { installs: [], skipped: [] };
//   // Node is not on the floor, so an agent whose own installer is an npm global asks for it here even where its
//   // package declares no floor of its own: the road it takes says it runs on node.
//   const onNode = new Set<string>();
//   for (const e of entries) {
//     if (!ticked(e) || e.rung !== "agents" || isMcpRow(e)) continue;
//     const entry = agents.find(a => a.id === name(e));
//     const installer = entry !== undefined ? pinnedInstaller(entry, e.pin) : table[name(e)];
//     if (installer) {
//       out.installs.push({ id: e.id, ...installer });
//       const road = entry === undefined ? undefined : ROAD_MODULES[entry.installRoad.road].after;
//       if (entry === undefined ? NPM_INSTALL_LINE.test(installer.install) : managerBehind(road) !== undefined) onNode.add(e.id);
//     } else out.skipped.push({ id: e.id, note: "no installer known" });
//   }
//   // An agent whose floor no pinned major meets is set aside rather than installed on a Node its engines refuse.
//   const pinnable = out.installs.filter(a => {
//     if (a.node === undefined || nodeMajorFor(a.node, now) !== undefined) return true;
//     out.skipped.push({ id: a.id, note: `needs Node ${a.node}, none pinned` });
//     return false;
//   });
//   out.installs = pinnable;
//   const floors = out.installs.filter(a => a.node !== undefined);
//   const needs = out.installs.filter(a => a.node !== undefined || onNode.has(a.id));
//   const floor = floors.length > 0 ? Math.max(...floors.map(a => a.node!)) : CATALOG_NODE_MAJOR;
//   const major = nodeMajorFor(floor, now);
//   if (needs.length > 0 && major !== undefined) {
//     const release = NODE_RELEASES[major];
//     out.node = { floor, version: release.version, agents: needs.map(a => a.name), cmd: nodeInstallScript(floor, release) };
//   }
//   return out;
// }
/// golden-import.ts:1462 `agentInstallsFor(entries, agents, now)`, `now` in ms since the epoch.
pub fn agent_installs_for_rows(entries: &[RecipeEntry], agents: &[AgentEntry], now_ms: i64) -> AgentsPlan {
    todo!()
}

/// `agentInstallsFor(ids.map(row))`, as the provision tests call it: each id a ticked row as provision.test.ts:121
/// `row(id)` builds it (rung `agents`, the id as its label), the catalog's agents, the clock now.
pub fn agent_installs_for(ids: &[&str]) -> AgentsPlan {
    todo!()
}

// TS golden-import.ts:1505-1534
// /** An agents plan as steps of the one tools loop: the node step first, then each agent, waiting on that step
//  * where its road runs on node. The install line runs on the tools PATH with the Node the step installed ahead of
//  * it, the agent's own version check is the step's check, the catalog's command is its bin and the version the
//  * catalog's road pins is what the step asks for, so a computer that already answers at that version or a newer
//  * one installs nothing. The road is the step's manager, which is what bounds the run.
//  *
//  * The plan's own skipped rows are not here: they never became steps, and whoever runs the plan reports them. */
// export function agentSteps(plan: AgentsPlan, agents: readonly AgentEntry[] = CATALOG_AGENTS, path: string = TOOLS_PATH, prefix?: string): ToolInstall[] {
//   const out: ToolInstall[] = [];
//   const node = plan.node;
//   if (node !== undefined) {
//     out.push({ id: AGENT_NODE_STEP, label: `Node ${node.version}`, manager: "script", cmd: node.cmd, shown: `Node ${node.version} for ${node.agents.join(", ")}`, check: nodeFloorCheck(node.floor), bin: NODE_BIN });
//   }
//   for (const a of plan.installs) {
//     const entry = agents.find(e => e.id === name(a));
//     const waits = node !== undefined && managerBehind(ROAD_MODULES[a.road].after) !== undefined ? { after: AGENT_NODE_STEP } : {};
//     out.push({
//       id: a.id,
//       label: a.name,
//       manager: a.road,
//       cmd: `${pathLine(path, prefix)}\n${NODE_PATH_LINE}\n${a.install}`,
//       shown: shownOf(a.install.split("\n")),
//       check: a.smoke,
//       ...waits,
//       ...(entry === undefined ? {} : { bin: entry.bin, ...roadReads(entry.installRoad, entry.bin, prefix), ...asksOf(entry.installRoad) }),
//       ...(a.pin === undefined ? {} : { pin: a.pin }),
//     });
//   }
//   return out;
// }
/// golden-import.ts:1512 `agentSteps(plan, agents, path, prefix)`.
pub fn agent_steps_with(plan: &AgentsPlan, agents: &[AgentEntry], path: &str, prefix: Option<&str>) -> Vec<ToolInstall> {
    todo!()
}

/// golden-import.ts:1512 `agentSteps(plan, CATALOG_AGENTS, path, prefix)`, as provision runs it.
pub fn agent_steps(plan: &AgentsPlan, path: &str, prefix: Option<&str>) -> Vec<ToolInstall> {
    todo!()
}
