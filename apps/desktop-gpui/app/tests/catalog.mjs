// SPDX-License-Identifier: AGPL-3.0-only
// What the Settings pages copy out of packages/catalog and packages/protocol, read off their built modules and printed
// as JSON for the tests in agents/catalog.rs and settings/words.rs to hold their copies to. Run as node catalog.mjs <catalog> <protocol>.
import { readFileSync } from "node:fs";

const catalogPath = process.argv[2];
const protocolPath = process.argv[3];
const cat = await import(catalogPath);
const pro = await import(protocolPath);

const NAMES = ["notion", "my server", "o'k"];

const loginOf = login => {
  if (login === undefined) return null;
  return NAMES.map(name =>
    login.inside !== undefined
      ? { name, inside: login.inside(name), line: null, pasted: null }
      : { name, inside: null, line: login.command(name), pasted: login.pasted === undefined ? null : login.pasted(name) },
  );
};

const agents = cat.CATALOG.filter(e => e.kind === "agent").map(e => {
  const s = e.signIn;
  const mcp = cat.MCP_AGENTS.find(a => a.id === e.id)?.mcp;
  return {
    id: e.id,
    name: e.name,
    bin: e.bin,
    planBrand: e.planBrand ?? null,
    description: e.about?.description ?? null,
    mint: s !== undefined && cat.mintsToken(s) ? s.mint : null,
    mcp:
      mcp === undefined
        ? null
        : { files: mcp.files, projectFiles: mcp.projectFiles, switch: cat.mcpSwitch(e.id), login: loginOf(mcp.login) },
    ownSkills: [cat.ownSkillFolder(e, false) !== undefined, cat.ownSkillFolder(e, true) !== undefined],
  };
});

const skills = [cat.WSP_SKILL_NAME, cat.SKILL_NAME, "wsp-x", "Wsp", "WSP", "machine", "skill", ""];
const rows = ["agents/claude", "agents/mcp/claude/notion", "agents/mcp", "agents/", "agents", "tools/brew/jq", "agentsx/claude", "files/agents/x"];

// Every printable ASCII character and a few past it, each after a letter, then the refusals that come before them.
const chars = [];
for (let c = 0x20; c < 0x7f; c += 1) chars.push(String.fromCharCode(c));
const modelIds = [...chars.map(c => `a${c}`), "é", "a日", "a\t", "", "-x", "a b", "claude-opus-4-5[1m]", "provider/model:tag", "x@2024+1"];

// The process set's own literal, then each of its names spelled as a person might, beside names that are not in it.
const built = readFileSync(new URL(protocolPath), "utf8");
const listed = (name, open) => {
  const found = new RegExp(`\\b${name} = (?:/\\* @__PURE__ \\*/ )?${open}\\[([^\\]]*)\\]`).exec(built);
  if (found === null) throw new Error(`${name} is not in ${protocolPath}`);
  return JSON.parse(`[${found[1]}]`);
};
const processEnv = listed("PROCESS_ENV", "new Set\\(");
const processEnvPrefixes = listed("PROCESS_ENV_PREFIXES", "");
const envNames = [
  ...processEnv,
  ...processEnv.map(n => n.toLowerCase()),
  ...processEnvPrefixes.map(p => `${p}PRELOAD`),
  ...processEnvPrefixes.map(p => `${p.toLowerCase()}library_path`),
  "WSP_TOKEN",
  "wsp_x",
  "ANTHROPIC_API_KEY",
  "PATHS",
  "XPATH",
  "LD",
  "",
];

// What the Settings pages copy: the catalog's tools by name, the agents that sign in once on a computer, each
// cloud's key words and mark, and each figure the pages write beside what the protocol writes for it.
const GIB = 1024 ** 3;
const figures = {
  bytes: [0, 1, 512, 1023, 1024, 1536, 1048575, 1048576, 300 * 1024 * 1024, GIB, 1.05 * GIB, 1.25 * GIB, 1.55 * GIB, 1.95 * GIB, 1.96 * GIB, 2 * GIB, 10.25 * GIB].map(Math.floor),
  tokens: [0, -5, 1, 2.5, 999, 999.5, 1000, 1005, 1125, 4270, 9995, 999499, 999500, 1125000, 22600000, 999500000, 7370000000],
  duration: [-1, 0, 0.4, 400, 999.6, 4250, 9950, 9999, 12400, 59499, 59500, 60000, 125000, 3600000, 3725000],
  offline: [-5, 0, 59000, 300000, 3599999, 3600000, 86399999, 86400000, 50 * 3600000],
};
const settings = {
  tools: cat.CATALOG.filter(e => e.kind !== "agent").map(e => [e.id, e.name]),
  sharedAgents: cat.sharedAgentsOn(cat.CATALOG_AGENTS.map(a => a.id)),
  signInWays: cat.CATALOG_AGENTS.map(a => [a.id, cat.signInWaysOf(a.id), a.signIn === undefined ? null : (cat.keyEnvOf(a.signIn) ?? null)]),
  signInWayWords: {
    labels: ["token", "key", "machine"].map(way => [way, pro.signInWayLabel(way, "box1")]),
    fix: pro.signInWaysFix(["token", "key", "machine"], { name: "Claude Code", here: "mac", box: "box1", field: true, mint: "claude setup-token", keyEnv: "ANTHROPIC_API_KEY" }),
  },
  providerKeyWords: pro.PROVIDER_KEY_WORDS,
  providerMarks: cat.PROVIDER_MARKS.map(m => [m.provider, m.svg]),
  figures: {
    bytes: figures.bytes.map(n => [n, pro.fmtBytes(n)]),
    tokens: figures.tokens.map(n => [n, pro.fmtTokens(n)]),
    duration: figures.duration.map(n => [n, pro.fmtDuration(n)]),
    offline: figures.offline.map(n => [n, pro.offlineFor(n)]),
    capitalised: ["", "a", "ready", "\u00e9clair", "\u00df"].map(w => [w, pro.capitalised(w)]),
    listWords: [[], ["a"], ["a", "b"], ["a", "b", "c"]].map(w => [w, pro.listWords(w)]),
  },
};

console.log(
  JSON.stringify({
    ...settings,
    agents,
    systemSkill: skills.map(name => [name, cat.isSystemSkill(name)]),
    agentOfRow: rows.map(id => [id, pro.agentOfRow({ id }) ?? null]),
    modelId: modelIds.map(id => [id, pro.modelIdRefusal(id)]),
    processEnv,
    processEnvPrefixes,
    env: envNames.map(name => [name, pro.agentEnvRefusal(name, "Agent")]),
  }),
);
