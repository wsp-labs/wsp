// SPDX-License-Identifier: AGPL-3.0-only
// The lines that put an agent's plugins on another computer by the agent's own
// commands, each from the marketplace this computer's index names for it: the
// picks' plugins on a computer somebody owns, and on the image the ones this
// computer switches on. The plugins' folder never travels; each plugin installs
// there afresh.
import { existsSync, readFileSync } from "node:fs";
import { posix } from "node:path";
import { CATALOG_AGENTS, GUEST_HOME, harnessLine, inStore, type AgentEntry } from "@wsp/catalog";
import { expand } from "@wsp/collect";
import { guestAgentHomes, type ImagePlugins, type PluginInstall } from "@wsp/engine";
import { userPlugins } from "./recipe-options.js";

type Skipped = { id: string; label: string; note: string };

const readHere = (home: string, path: string): string | undefined => {
  const at = expand({ home }, path);
  return existsSync(at) ? readFileSync(at, "utf8") : undefined;
};

/** The plugins named, as the lines that put each on by `agent`'s own commands pointed at the folder its threads read.
 * One whose marketplace this computer cannot name, or with no ticked agent that takes plugins, is set aside. */
export function pluginInstalls(names: readonly string[], agent: AgentEntry | undefined, home: string, stores?: Readonly<Record<string, string>>): { plugins: PluginInstall[]; skipped: Skipped[] } {
  const road = agent?.plugins;
  const plugins: PluginInstall[] = [];
  const skipped: Skipped[] = [];
  const index = road === undefined ? undefined : readHere(home, road.marketplaces);
  for (const name of names) {
    const marketplace = name.split("@")[1];
    const source = road === undefined || index === undefined || marketplace === undefined ? undefined : road.sourceOf(index, marketplace);
    if (agent === undefined || road === undefined || source === undefined) skipped.push({ id: `plugins/${name}`, label: name, note: road === undefined ? "no picked agent takes plugins" : "this computer names no marketplace for it" });
    else plugins.push({ id: `plugins/${name}`, label: name, cmd: harnessLine(agent.id, road.install(name, source), { stores }), asked: out => road.asked(out, name) });
  }
  return { plugins, skipped };
}

/** The plugins `agent` has installed for the person here and switches on, by `name@marketplace`: the ones the add's
 * default picks tick. */
export function enabledPlugins(agent: AgentEntry, home: string): string[] {
  const road = agent.plugins;
  const index = agent.pluginSkills?.index;
  if (road === undefined || index === undefined) return [];
  const settings = readHere(home, road.settings);
  const on = new Set(settings === undefined ? [] : road.enabled(settings));
  return userPlugins(readHere(home, index)).filter(name => on.has(name));
}

/** What the image carries of the ticked agent that takes plugins: the ones this computer switches on, installed into
 * the store its turns read on the machine. Absent when no ticked agent takes plugins. */
export function imagePlugins(ticked: (agentId: string) => boolean, home: string): ImagePlugins | undefined {
  const agent = CATALOG_AGENTS.find(a => a.plugins !== undefined && ticked(a.id));
  if (agent?.plugins === undefined) return undefined;
  const stores = guestAgentHomes();
  const { plugins, skipped } = pluginInstalls(enabledPlugins(agent, home), agent, home, stores);
  return { agent: `agents/${agent.id}`, folder: inStore(agent, posix.dirname(agent.plugins.marketplaces), { home: GUEST_HOME, store: stores[agent.id] }), installs: plugins, skipped };
}
