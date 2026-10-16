// SPDX-License-Identifier: AGPL-3.0-only
// Where each agent loads skills from. An agent entry names its own folders
// here; the reader lists every folder of every entry, so an agent the catalog
// gains needs nothing in the reader.
import { PLUGIN_SEED_ENV, shellQuote } from "@wsp/protocol";
import { SKILL_NAME } from "./context.js";
import { editJson } from "./mcp.js";
import { CLAUDE_CONFIG_DIR, CLAUDE_PLUGIN_SEED } from "./roads.js";

/** One folder an agent loads skills from, one folder per skill with a SKILL.md inside: `~/`-relative for the
 * person's own, project-relative for a project's. `lands` is how an install puts a skill there: its files, or a
 * link to the one copy in the shared folder. */
export interface SkillRoot {
  dir: string;
  lands: "copy" | "link";
}

/** The folder several agents read as their own and the skills CLI keeps the one copy of a skill in. */
export const SHARED_SKILLS = "~/.agents/skills";

/** The same folder inside a project. */
export const PROJECT_SHARED_SKILLS = ".agents/skills";

/** The XDG agents folder, which several agents read beside the shared one and none of them owns. */
export const XDG_SHARED_SKILLS = "~/.config/agents/skills";

/** An agent's skill folders: its own first, which is where the wsp skill goes with the MCP server, then the ones it
 * also reads; and the folders it reads inside a project. */
export interface SkillRoots {
  user: readonly [SkillRoot, ...SkillRoot[]];
  project: readonly SkillRoot[];
}

/** The index file that names each installed plugin and its folder, which the recipe reads for the plugins it offers.
 * Read, never written: a plugin's skills come and go with the plugin, and the plugin module reads them. */
export interface PluginSkills {
  index: string;
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** A JSON object off a file's text; nothing for a file missing or not an object. */
function jsonOf(text: string | undefined): Record<string, unknown> | undefined {
  if (text === undefined) return undefined;
  try {
    const parsed: unknown = JSON.parse(text);
    return isObject(parsed) ? parsed : undefined;
  } catch {
    return undefined;
  }
}

/** Claude Code's installed_plugins.json, version 2 (read 2026-09-24 on 2.1.281). */
export const CLAUDE_PLUGIN_SKILLS: PluginSkills = { index: "~/.claude/plugins/installed_plugins.json" };

/** Skills an agent's own install puts in its skills folder, which come with the agent wherever it is installed and
 * so are never the person's to pick: the file the install writes naming each one it put there, and its names. */
export interface BundledSkills {
  manifest: string;
  names(text: string): string[];
}

/** Hermes's .bundled_manifest (tools/skills_sync.py at v2026.8.31): a line per skill it seeded into ~/.hermes/skills,
 * `name:hash`, or the bare name an older install wrote. Codex keeps its own under ~/.codex/skills/.system, a dot
 * folder the skills reader never lists. */
export const HERMES_BUNDLED_SKILLS: BundledSkills = {
  manifest: "~/.hermes/skills/.bundled_manifest",
  names: text =>
    text
      .split("\n")
      .map(line => line.split(":")[0]!.trim())
      .filter(name => name !== ""),
};

/** How an agent's plugins go on another computer: the file here naming each marketplace and where it is fetched
 * from, and the lines that put one plugin on by the agent's own commands there. The plugin's folder never travels. */
export interface PluginRoad {
  marketplaces: string;
  /** The agent's settings file here, which switches each installed plugin on or off. */
  settings: string;
  /** The plugins that file switches on, by `name@marketplace`; none off a file that does not parse. */
  enabled(text: string): string[];
  /** Where a marketplace is fetched from, off that file's text; nothing where it names none. */
  sourceOf(text: string, marketplace: string): string | undefined;
  /** The lines for one plugin, `name@marketplace`, from that source. */
  install(plugin: string, source: string): string;
  /** Why a plugin is set aside, off what its install printed, where it asked to run a command its marketplace
   * declares: a person accepts that command, never a setup. Nothing for any other outcome. */
  asked(out: string, plugin: string): string | undefined;
  /** The line that takes one plugin off again, by the agent's own command. */
  uninstall(plugin: string): string;
  /** Files on a guest, absolute, that an older image copied from this computer and no build copies now: every build
   * of the agent takes them off, plugins to seed or not. */
  copiedBefore?: readonly string[];
  /** Where an image holds its plugins, which a machine forked off it reads in place and never fetches. */
  seed?: PluginSeed;
  /** That settings file with every marketplace no switched-on plugin comes from taken out, since the agent fetches
   * each one it names on start; the text as it was where it does not parse or names none. */
  usedOnly?(text: string): string;
}

/** A folder of plugins built when the image is, which every turn on a machine forked off it is pointed at. */
export interface PluginSeed {
  /** Absolute on the guest. */
  dir: string;
  /** The variable that points the agent at the folder. */
  env: string;
  /** The lines that build one plugin into the folder from its marketplace's source, leaving the agent's own
   * settings as they were. */
  build(plugin: string, source: string): string;
  /** The links, relative to the folder, the agent needs beside what it built before it reads every plugin there:
   * `[link, target]`, off the folder's own files as `read` gives them by their path in it. */
  links(read: (rel: string) => Promise<string | undefined>): Promise<[string, string][]>;
}

/** Claude Code's plugins (2.1.281): known_marketplaces.json keys each marketplace to a github repo or a URL, and the
 * plugin installs once its marketplace is added; adding one already there exits non-zero and changes nothing. */
export const CLAUDE_PLUGINS: PluginRoad = {
  marketplaces: "~/.claude/plugins/known_marketplaces.json",
  sourceOf: (text, marketplace) => {
    try {
      const at = (JSON.parse(text) as Record<string, { source?: { source?: string; repo?: string; url?: string } }>)[marketplace]?.source;
      return at?.source === "github" ? at.repo : at?.url;
    } catch {
      return undefined;
    }
  },
  settings: "~/.claude/settings.json",
  enabled: text => {
    try {
      const on: unknown = (JSON.parse(text) as { enabledPlugins?: unknown }).enabledPlugins;
      return isObject(on) ? Object.keys(on).filter(name => on[name] === true) : [];
    } catch {
      return [];
    }
  },
  // --json with no input refuses a plugin that asks for a command at once, naming the command's sha256.
  install: (plugin, source) => [`claude plugin marketplace add ${shellQuote(source)} || true`, `claude plugin install ${shellQuote(plugin)} --json </dev/null`].join("\n"),
  asked: (out, plugin) => {
    const sha = /"shownCommand"\s*:\s*\{[^}]*"sha256"\s*:\s*"([0-9a-f]{64})"/.exec(out)?.[1];
    return sha === undefined ? undefined : `asks to run a command its marketplace declares; to accept it, run claude plugin install ${plugin} --accept-command ${sha} on that computer`;
  },
  uninstall: plugin => `claude plugin uninstall ${shellQuote(plugin)} </dev/null`,
  // An older image copied these from this computer: each names this computer's folders, and a turn fetches every
  // marketplace one lists that the seed lacks (2.1.296, 2.1.299).
  copiedBefore: [`${CLAUDE_CONFIG_DIR}/plugins/known_marketplaces.json`, `${CLAUDE_CONFIG_DIR}/plugins/installed_plugins.json`],
  // code.claude.com/docs/en/plugins/org, "Seed containers and CI". A fresh config folder per build, since adding a
  // marketplace writes it into the settings there (2.1.299).
  seed: {
    dir: CLAUDE_PLUGIN_SEED,
    env: PLUGIN_SEED_ENV,
    build: (plugin, source) =>
      [
        'seed_config="$(mktemp -d)"',
        `CLAUDE_CODE_PLUGIN_CACHE_DIR=${shellQuote(CLAUDE_PLUGIN_SEED)} CLAUDE_CONFIG_DIR="$seed_config" sh -c ${shellQuote(CLAUDE_PLUGINS.install(plugin, source))}`,
        "built=$?",
        'rm -rf "$seed_config"',
        'exit "$built"',
      ].join("\n"),
    // A plugin its marketplace pins to a commit of its own repo is filed under its version, and a turn reading the
    // seed looks for it under the first twelve of that sha and skips it; a link on any other plugin makes the turn
    // skip that one (2.1.296, 2.1.299 and 2.1.301).
    links: async read => {
      const index = jsonOf(await read("installed_plugins.json"))?.plugins;
      if (!isObject(index)) return [];
      const out: [string, string][] = [];
      const manifests = new Map<string, Promise<string | undefined>>();
      for (const [id, records] of Object.entries(index)) {
        const at = id.lastIndexOf("@");
        const record: unknown = Array.isArray(records) ? records[0] : undefined;
        if (at <= 0 || !isObject(record) || typeof record.installPath !== "string") continue;
        const [name, marketplace] = [id.slice(0, at), id.slice(at + 1)];
        if (!manifests.has(marketplace)) manifests.set(marketplace, read(`marketplaces/${marketplace}/.claude-plugin/marketplace.json`));
        const listed = jsonOf(await manifests.get(marketplace))?.plugins;
        const entry: unknown = Array.isArray(listed) ? listed.find(p => isObject(p) && p.name === name) : undefined;
        const sha = isObject(entry) && isObject(entry.source) && typeof entry.source.sha === "string" ? entry.source.sha.slice(0, 12) : undefined;
        const folder = record.installPath.slice(record.installPath.lastIndexOf("/") + 1);
        if (sha !== undefined && /^[0-9a-f]{12}$/.test(sha) && folder !== sha) out.push([`cache/${marketplace}/${name}/${sha}`, folder]);
      }
      return out;
    },
  },
  usedOnly: text => {
    let parsed: { enabledPlugins?: unknown; extraKnownMarketplaces?: unknown };
    try {
      parsed = JSON.parse(text) as typeof parsed;
    } catch {
      return text;
    }
    const known = parsed.extraKnownMarketplaces;
    if (!isObject(known)) return text;
    const on = CLAUDE_PLUGINS.enabled(text);
    const unused = Object.keys(known).filter(m => !on.some(p => p.endsWith(`@${m}`)));
    try {
      return unused.length === 0 ? text : editJson(text, unused.map(m => [["extraKnownMarketplaces", m], undefined]));
    } catch {
      return text;
    }
  },
};

/** Where an install puts a skill for one agent, a project's with `project`: nothing where the agent reads the shared
 * folder already, else its own folder with how that folder takes a skill. */
export function ownSkillFolder(a: { skillRoots: SkillRoots }, project: boolean): SkillRoot | undefined {
  const roots = project ? a.skillRoots.project : a.skillRoots.user;
  if (roots.some(r => r.dir === (project ? PROJECT_SHARED_SKILLS : SHARED_SKILLS))) return undefined;
  return roots[0];
}

/** The folder the agent's own skills go in, where the wsp skill is written. */
export const skillsDirOf = (a: { skillRoots: SkillRoots }): string => a.skillRoots.user[0].dir;

/** The skill's folder name under every agent's skills directory on the person's own computer, and its frontmatter name. */
export const WSP_SKILL_NAME = "wsp";

/** A skill wsp writes and rewrites itself, on the person's computer or on a machine it made: never the person's to
 * turn off or remove, since the next start puts it back. */
export const isSystemSkill = (name: string): boolean => name === WSP_SKILL_NAME || name === SKILL_NAME;
