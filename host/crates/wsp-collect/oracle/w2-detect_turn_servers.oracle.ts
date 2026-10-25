// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/detect_turn_servers.md on the edge inputs of the wave-2 audit (group c).
// read_turn_servers cases: args [agent_id, folder, {checkout?, own?}]; detect_project_mcp cases: args [folder, {agents: [ids], keep}],
// out {servers: [{agent: id, turn, row}], read, checkout}. A store is `stores` in the laptop.
import { it } from "vitest";
import { MCP_AGENTS, checkoutArgs } from "../../../../packages/catalog/src/index.js";
import { detectProjectMcp, outsideProjectLine } from "../../../../packages/collect/src/detect/mcp-project.js";
import { readCheckout, readTurnServers } from "../../../../packages/collect/src/detect/turn-servers.js";
import { host, settle, write, type Laptop } from "./w2-host.js";

const P = "/Users/dev/proj";
const rev = (folder = P): string => `git ${checkoutArgs(folder).join(" ")}`;
const lsf = (top: string, ...rel: string[]): string => `git -C ${top} ls-files -- ${rel.join(" ")}`;
const agentOf = (id: string) => MCP_AGENTS.find(a => a.id === id)!;
const J = (o: unknown): string => JSON.stringify(o);
const gitP = { [rev()]: `${P}\n${P}/.git\n` };

type Opts = { checkout?: { top?: string; key: string }; own?: true };
const turn = async (name: string, laptop: Laptop, agent: string, folder: string, o: Opts = {}) => {
  const h = host(laptop);
  return { name, laptop, args: [agent, folder, o], ...(await settle(() => readTurnServers(h, agentOf(agent), folder, o))), calls: h.calls };
};
const proj = async (name: string, laptop: Laptop, agents: string[], keep: string[] = [], folder = P) => {
  const h = host(laptop);
  const r = await settle(async () => {
    const x = await detectProjectMcp(h, folder, { agents: agents.map(agentOf), keep });
    return { servers: x.servers.map(s => ({ agent: s.agent.id, turn: s.turn, row: s.row })), read: x.read, checkout: x.checkout };
  });
  return { name, laptop, args: [folder, { agents, keep }], ...r, calls: h.calls };
};
const claudeJson = (local: Record<string, unknown> = {}, extra: Record<string, unknown> = {}) => J({ mcpServers: { homesrv: { command: "npx", args: ["home-mcp"] } }, projects: { [P]: { mcpServers: local, ...extra } } });
const mcp = (servers: Record<string, unknown>) => J({ mcpServers: servers });
const npx = (name: string, arg = name) => ({ [name]: { command: "npx", args: [arg] } });

it("detect_turn_servers fixtures", async () => {
  const files = {
    "~/.claude.json": J({ mcpServers: { home: { command: "npx" } }, projects: { [P]: { mcpServers: { loc: { command: "x" } } }, "/Users/dev/main": { mcpServers: { mainloc: { command: "y" } } } } }),
    [`${P}/sub/.mcp.json`]: mcp({ subs: { command: "s" } }),
    [`${P}/.mcp.json`]: mcp({ rootp: { command: "r" } }),
    "/Users/dev/.mcp.json": mcp({ abovep: { command: "a" } }),
    "/Users/.mcp.json": mcp({ rootabove: { command: "b" } }),
    "/Users/dev/store/.claude.json": mcp({ stored: { command: "z" } }),
  };
  const sub = `${P}/sub`;
  const inGit = (folder: string) => ({ [rev(folder)]: `${P}\n${P}/.git\n` });
  const codexTrust = `[projects."${P}"]\ntrust_level = "trusted"\n[mcp_servers.u]\ncommand = "uvx"\n`;
  const c = (s: string): string => `[mcp_servers.${s}]\ncommand = "${s}"\n`;

  const turns = [
    await turn("claude, folder under the top: scopes and the files up to /", { files, exec: inGit(sub) }, "claude", sub),
    await turn("claude, a worktree is keyed by the main checkout", { files, exec: { [rev("/Users/dev/wt")]: "/Users/dev/wt\n/Users/dev/main/.git\n" } }, "claude", "/Users/dev/wt"),
    await turn("claude, git says not a repository", { files, exec: { [rev(sub)]: "fatal: not a git repository\n" } }, "claude", sub),
    await turn("claude, git gives no answer", { files }, "claude", sub),
    await turn("claude own, the home folder", { files }, "claude", "/Users/dev", { own: true }),
    await turn("claude own, a project folder keys by the folder", { files }, "claude", P, { own: true }),
    await turn("claude, a checkout with top handed in", { files }, "claude", sub, { checkout: { top: P, key: P } }),
    await turn("claude, a checkout with only a key handed in", { files }, "claude", sub, { checkout: { key: sub } }),
    await turn("claude with a store", { files, stores: { claude: "/Users/dev/store" } }, "claude", P, { checkout: { top: P, key: P } }),
    await turn("claude with a store that holds no file", { files, stores: { claude: "/Users/dev/nostore" } }, "claude", P, { checkout: { top: P, key: P } }),
    await turn("codex trusted, folder under the top", { files: { "~/.codex/config.toml": codexTrust, [`${sub}/.codex/config.toml`]: c("s"), [`${P}/.codex/config.toml`]: c("r") }, exec: inGit(sub) }, "codex", sub),
    await turn("codex untrusted: the project file is still read", { files: { "~/.codex/config.toml": c("u"), [`${P}/.codex/config.toml`]: c("r") }, exec: gitP }, "codex", P),
    await turn("codex with a store", { files: { "/Users/dev/cx/config.toml": c("st"), "~/.codex/config.toml": c("u") }, stores: { codex: "/Users/dev/cx" } }, "codex", P, { checkout: { key: P } }),
    await turn("opencode, both own files present: the first only", { files: { "~/.config/opencode/opencode.json": J({ mcp: { a1: { type: "local", command: ["x"] } } }), "~/.config/opencode/opencode.jsonc": J({ mcp: { a2: { type: "local", command: ["y"] } } }) } }, "opencode", P, { checkout: { key: P } }),
    await turn("opencode, only the second own file", { files: { "~/.config/opencode/opencode.jsonc": J({ mcp: { a2: { type: "local", command: ["y"] } } }) } }, "opencode", P, { checkout: { key: P } }),
    await turn("opencode with a store is ignored", { files: { "~/.config/opencode/opencode.json": J({ mcp: { a1: { type: "local", command: ["x"] } } }) }, stores: { opencode: "/Users/dev/os" } }, "opencode", P, { checkout: { key: P } }),
  ];

  const checks = ["", "\n", "/a/b\n", "/a/b\n/a/b/.git\n", "/a/b\n/main/.git\n", "/a/b\n/main/repo.git\n", "rel/b\n/x/.git\n", "  /a/b  \r\n  /main/.git  \r\n", "/a/b\n\n", "\n/a/b\n/x/.git", "/a/b\n/.git\n", "fatal: not a git repository\n", "/a/b\n.git\n", "/a/b\n/x/y/.git/worktrees/w\n", "/\n/.git\n"];
  const checkouts = [];
  for (const said of checks) {
    const laptop: Laptop = { exec: { [rev()]: said } };
    const h = host(laptop);
    checkouts.push({ name: J(said), laptop, args: [P], ...(await settle(() => readCheckout(h, P))), calls: h.calls });
  }
  { const h = host({}); checkouts.push({ name: "git gives no answer", laptop: {}, args: [P], ...(await settle(() => readCheckout(h, P))), calls: h.calls }); }

  const out = (n: string, command: string, args?: string[]) => ({ [n]: { command, ...(args ? { args } : {}) } });
  const outsideLocals = { ...out("a", "~/.local/bin/tool"), ...out("b", "/Users/dev/.local/bin"), ...out("c", "/Users/dev/.local/bin2/x"), ...out("d", "~/bin/d", ["~/notes", "/Users/dev/proj/in.db", "/Users/dev/data/x.db", "--cfg=/Users/dev/etc/c.json", "/Users/dev/proj"]), ...out("e", "/Users/dev/proj/bin/e", ["/Users/dev/proj/in"]), ...out("f", "/opt/x/f", ["/tmp/y"]), ...out("g", "/Users/dev/proj", []), h: { type: "http", url: "https://h.example/mcp" }, ...out("i", "npx", ["-y", "/Users/dev/other/pkg"]), ...out("j", "/Users/dev/projx/bin/j"), ...out("k", "/Users/dev/bin/k", ["/Users/dev/bin/k"]) };

  const projects = [
    await proj("outside git, an untracked project file is carried", { files: { "~/.claude.json": claudeJson(), [`${P}/.mcp.json`]: mcp(npx("p1")) }, exec: { [rev()]: "fatal: not a git repository\n" } }, ["claude"]),
    await proj("outside git, keep brings it", { files: { "~/.claude.json": claudeJson(), [`${P}/.mcp.json`]: mcp(npx("p1")) }, exec: { [rev()]: "fatal\n" } }, ["claude"], [".mcp.json"]),
    await proj("a tracked line with CRLF is not tracked", { files: { "~/.claude.json": claudeJson(), [`${P}/.mcp.json`]: mcp(npx("p1")) }, exec: { ...gitP, [lsf(P, ".mcp.json")]: ".mcp.json\r\n" } }, ["claude"]),
    await proj("blank lines in git's answer", { files: { "~/.claude.json": claudeJson(), [`${P}/.mcp.json`]: mcp(npx("p1")) }, exec: { ...gitP, [lsf(P, ".mcp.json")]: "\n\n.mcp.json\n\n" } }, ["claude"]),
    await proj("ls-files gives no answer", { files: { "~/.claude.json": claudeJson(), [`${P}/.mcp.json`]: mcp(npx("p1")) }, exec: gitP }, ["claude"]),
    await proj("folder under the top, both files tracked", { files: { "~/.claude.json": claudeJson(), [`${sub}/.mcp.json`]: mcp(npx("s1")), [`${P}/.mcp.json`]: mcp(npx("r1")), "/Users/dev/.mcp.json": mcp(npx("a1")) }, exec: { [rev(sub)]: `${P}\n${P}/.git\n`, [lsf(P, "sub/.mcp.json", ".mcp.json")]: ".mcp.json\nsub/.mcp.json\n" } }, ["claude"], [], sub),
    await proj("folder under the top, nothing tracked", { files: { "~/.claude.json": claudeJson(), [`${sub}/.mcp.json`]: mcp(npx("s1")), [`${P}/.mcp.json`]: mcp(npx("r1")) }, exec: { [rev(sub)]: `${P}\n${P}/.git\n` } }, ["claude"], [], sub),
    await proj("keep names are relative to the folder", { files: { "~/.claude.json": claudeJson(), [`${sub}/.mcp.json`]: mcp(npx("s1")), [`${P}/.mcp.json`]: mcp(npx("r1")) }, exec: { [rev(sub)]: `${P}\n${P}/.git\n` } }, ["claude"], [".mcp.json", "../.mcp.json"], sub),
    await proj("a keep path above the top brings nothing", { files: { "~/.claude.json": claudeJson(), [`${sub}/.mcp.json`]: mcp(npx("s1")), "/Users/dev/.mcp.json": mcp(npx("a1")) }, exec: { [rev(sub)]: `${P}\n${P}/.git\n` } }, ["claude"], ["../../.mcp.json", "../../../.mcp.json", ".mcp.json"], sub),
    await proj("a brought nearest definition shadows a farther one of the same name", { files: { "~/.claude.json": claudeJson(), [`${sub}/.mcp.json`]: mcp(npx("x", "nearest")), "/Users/dev/.mcp.json": mcp({ ...npx("x", "farther"), ...npx("y") }) }, exec: { [rev(sub)]: `${P}\n${P}/.git\n`, [lsf(P, "sub/.mcp.json")]: "sub/.mcp.json\n" } }, ["claude"], [], sub),
    await proj("a local server wins over a project one of the same name", { files: { "~/.claude.json": claudeJson(npx("x", "local")), [`${P}/.mcp.json`]: mcp(npx("x", "project")) }, exec: gitP }, ["claude"]),
    await proj("disabled and outside at once", { files: { "~/.claude.json": claudeJson({ off: { command: "/Users/dev/bin/tool", args: ["/Users/dev/data/a"] } }, { disabledMcpServers: ["off"] }) }, exec: gitP }, ["claude"]),
    await proj("a disabled project server whose file is tracked", { files: { "~/.claude.json": claudeJson({}, { disabledMcpServers: ["p1"] }), [`${P}/.mcp.json`]: mcp(npx("p1")) }, exec: { ...gitP, [lsf(P, ".mcp.json")]: ".mcp.json\n" } }, ["claude"]),
    await proj("a disabled project server that is not tracked", { files: { "~/.claude.json": claudeJson({}, { disabledMcpServers: ["p1"] }), [`${P}/.mcp.json`]: mcp(npx("p1")) }, exec: gitP }, ["claude"]),
    await proj("the forms of an outside path", { files: { "~/.claude.json": claudeJson(outsideLocals) }, exec: gitP }, ["claude"]),
    await proj("agents with no launch road read nothing", { files: { "~/.gemini/settings.json": mcp({ g: { command: "npx" } }), "~/.claude.json": claudeJson() }, exec: gitP }, ["gemini", "opencode"]),
    await proj("no agents", { exec: gitP }, []),
    await proj("codex then claude, every project server tracked", { files: { "~/.claude.json": claudeJson(), [`${P}/.mcp.json`]: mcp(npx("p1")), "~/.codex/config.toml": codexTrust, [`${P}/.codex/config.toml`]: "[mcp_servers.c1]\ncommand = \"uvx\"\nargs = [\"c1\"]\n" }, exec: { ...gitP, [lsf(P, ".mcp.json")]: ".mcp.json\n", [lsf(P, ".codex/config.toml")]: ".codex/config.toml\n" } }, ["codex", "claude"]),
    await proj("codex untracked project server is carried", { files: { "~/.codex/config.toml": codexTrust, [`${P}/.codex/config.toml`]: "[mcp_servers.c1]\ncommand = \"uvx\"\nargs = [\"c1\"]\n" }, exec: gitP }, ["codex"]),
    await proj("a local-only claude server: no git ls-files call", { files: { "~/.claude.json": claudeJson(npx("l1")) }, exec: gitP }, ["claude"]),
    await proj("a project http server with a secret header", { files: { "~/.claude.json": claudeJson(), [`${P}/.mcp.json`]: mcp({ h: { type: "http", url: "https://h.example/mcp", headers: { Authorization: "Bearer ${TOK}" } } }) }, exec: gitP }, ["claude"]),
    await proj("outside git, a folder with a trailing slash", { files: { "~/.claude.json": claudeJson(), [`${P}/.mcp.json`]: mcp(npx("p1")) }, exec: { [rev(`${P}/`)]: "fatal\n" } }, ["claude"], [], `${P}/`),
    await proj("in git, a folder with a trailing slash", { files: { "~/.claude.json": claudeJson(), [`${P}/.mcp.json`]: mcp(npx("p1")) }, exec: { [rev(`${P}/`)]: `${P}\n${P}/.git\n`, [lsf(P, ".mcp.json")]: "" } }, ["claude"], [], `${P}/`),
    await proj("two project files give one ls-files call in server order", { files: { "~/.claude.json": claudeJson(), [`${sub}/.mcp.json`]: mcp(npx("s1")), [`${P}/.mcp.json`]: mcp(npx("r1")) }, exec: inGit(sub) }, ["claude"], [], sub),
  ];

  write("detect_turn_servers", {
    read_checkout: checkouts,
    read_turn_servers: turns,
    detect_project_mcp: projects,
    outside_project_line: [[], ["~/a"], ["~/a", "~/b", "~/c"], ["~/bin/mine", "~/data/x.db"]].map(p => ({ name: J(p), args: [p], out: outsideProjectLine(p) })),
  });
});
