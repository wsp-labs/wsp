// SPDX-License-Identifier: AGPL-3.0-only
// The recipe beside this host's state file, planned for a computer somebody
// owns: the same rows a copy of the image is planned from, come to the steps
// that run on the computer itself.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Manifest } from "@wsp/collect";
import { TOOL_PREFIX, catalogEntry } from "@wsp/catalog";
import { MCP_ID_PREFIX, TOOLS_PATH, placeProvisionPaths, probePath, type Recipe, type RecipeFile } from "@wsp/protocol";
import { AGENT_NODE_STEP, closeAgentFiles, oncePathsOf, pathLine, provisionFiles, provisionMcp, unlandFiles, type ProvisionPlan } from "@wsp/engine";
import { nodeHost } from "@wsp/collect";
import { boxGuest, cleanGuests } from "../../engine/test/box-guest.js";
import { parseEnvFile, serverEnvFileFor } from "../src/env-keys.js";
import { serversActs } from "../src/servers-acts.js";
import { placeProvisioner, undoPlan } from "../src/place-provision.js";
import { FIXTURE, RECIPE } from "./init-fixture.js";

/** The recipe this computer holds: the fixture's, with Codex ticked and one row the catalog has none for. */
const SMALL: Recipe = {
  ...RECIPE,
  rows: RECIPE.rows.map(r => (r.id === "codex" ? { ...r, on: true } : r)),
  custom: [{ kind: "custom", id: "wsp-map", name: "wsp-map", install: ["npm install -g wsp-map@1.0.0"], check: "wsp-map --version", manager: "npm", why: "added by the agent" }],
};

describe("the recipe this host holds, planned for a computer you own", () => {
  let dir: string;
  let home: string;
  let statePath: string;

  const planner = () =>
    placeProvisioner({
      statePath,
      home,
      platform: "linux",
      collect: async (): Promise<Manifest> => FIXTURE,
      brew: async () => new Map(),
    });

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "wsp-place-provision-"));
    home = join(dir, "home");
    statePath = join(dir, "state.json");
    mkdirSync(join(home, ".claude"), { recursive: true });
    mkdirSync(join(home, ".codex"), { recursive: true });
    writeFileSync(join(home, ".gitconfig"), "[user]\n\tname = Test\n");
    writeFileSync(join(home, ".zshrc"), "export PS1='$ '\n");
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it("plans neither wsp's own MCP server nor the package that installs the wsp command where saved picks still hold them, and the rows beside them as before", async () => {
    writeFileSync(join(home, ".claude.json"), '{ "mcpServers": { "github": { "command": "npx" }, "wsp": { "command": "/Users/z/wsp/bin/wsp", "args": ["mcp"] } } }\n');
    const server = (name: string) => ({ rung: "agents" as const, id: `${MCP_ID_PREFIX}claude/${name}`, label: name, group: "Claude Code MCP servers", paths: ["~/.claude.json"], bytes: 300, default: "bring" as const });
    const tool = (via: string, name: string) => ({ rung: "tools" as const, id: `tools/${via}/${name}`, label: name, paths: [], bytes: 0, default: "bring" as const });
    const manifest = { ...FIXTURE, entries: [...FIXTURE.entries, server("github"), server("wsp"), tool("pnpm", "@wsp/host"), tool("npm", "litmus-cli")] };
    // wsp's package is the one that installs the wsp command, whatever it is called and whichever manager put it here.
    const at = { "@wsp/host": join(home, "pnpm/global/v11/21d31-1a1176c18dc/node_modules/@wsp/host"), "litmus-cli": join(home, "npm-root/litmus-cli") };
    for (const [name, bin] of [["@wsp/host", { wsp: "dist/bin.js" }], ["litmus-cli", { litmus: "bin.js" }]] as const) {
      mkdirSync(at[name], { recursive: true });
      writeFileSync(join(at[name], "package.json"), JSON.stringify({ name, bin }));
    }
    const listing = JSON.stringify([{ path: join(home, "pnpm/global/v11"), dependencies: { "@wsp/host": { from: "@wsp/host", version: "0.1.0", path: at["@wsp/host"] } } }]);
    const here = { ...nodeHost(), home, exec: { which: async () => true, run: async (cmd: string, args: readonly string[]) => (cmd === "pnpm" && args[0] === "ls" ? listing : cmd === "npm" && args[0] === "root" ? join(home, "npm-root") : undefined) } };
    const planner = placeProvisioner({ statePath, home, platform: "linux", collect: async () => manifest, brew: async () => new Map(), here });
    const saved = { name: "default", agents: { claude: { signin: "vault" as const } }, mcp: { github: { agents: ["claude"] }, wsp: { agents: ["claude"] } }, clis: { "@wsp/host": { via: "pnpm" }, "litmus-cli": { via: "npm" } }, skills: {}, plugins: {}, folders: {}, configs: {} };
    const plan = await planner.setup(saved, { home });
    expect(plan.mcp?.agents.map(a => [a.id, a.scopes.flatMap(sc => sc.keep)])).toEqual([["claude", ["github"]]]);
    const installs = JSON.stringify(plan.steps);
    expect(installs).toContain("litmus-cli");
    expect(installs).not.toContain("@wsp/host");
    expect((await planner.estimate!(saved)).unmeasured).toBe((await planner.estimate!({ ...saved, mcp: { github: saved.mcp.github }, clis: { "litmus-cli": saved.clis["litmus-cli"] } })).unmeasured);
  });

  it("names the CLIs the kept servers run, read off this computer's config, so the servers wait on those alone", async () => {
    writeFileSync(join(home, ".claude.json"), JSON.stringify({ mcpServers: { yaml: { command: "/opt/homebrew/bin/yq" }, diff: { command: "delta" }, web: { command: "npx", args: ["-y", "web-mcp"] }, off: { command: "gh" } } }));
    const server = (name: string) => ({ rung: "agents" as const, id: `${MCP_ID_PREFIX}claude/${name}`, label: name, group: "Claude Code MCP servers", paths: ["~/.claude.json"], bytes: 300, default: "bring" as const });
    const delta = { rung: "tools" as const, id: "tools/brew/git-delta", label: "git-delta", group: "Homebrew", paths: ["Brewfile"], bytes: 0, default: "bring" as const };
    const manifest = { ...FIXTURE, entries: [...FIXTURE.entries, delta, server("yaml"), server("diff"), server("web"), server("off")] };
    const picks = { name: "laptop", agents: { claude: { signin: "vault" as const } }, mcp: { yaml: { agents: ["claude"] }, diff: { agents: ["claude"] }, web: { agents: ["claude"] } }, clis: { gh: { via: "brew" }, yq: { via: "brew" }, "git-delta": { via: "brew" } }, skills: {}, plugins: {}, folders: {}, configs: {} };
    const plan = await placeProvisioner({ statePath, home, platform: "linux", collect: async () => manifest, brew: async () => new Map() }).setup(picks, { home });
    expect(plan.steps.map(s => s.id)).toEqual(expect.arrayContaining(["tools/brew/gh", "tools/brew/yq", "tools/brew/git-delta"]));
    // yq and delta are kept servers' commands, delta by the catalog's name for git-delta's; npx comes with node,
    // and gh is the command of a server that was not picked.
    expect([...(plan.serverTools ?? [])].sort()).toEqual(["tools/brew/git-delta", "tools/brew/yq"]);
  });

  const tool = (id: string) => ({ rung: "tools" as const, id, label: id.slice(id.lastIndexOf("/") + 1), group: "CLIs", paths: [], bytes: 0, default: "bring" as const });
  const server = (name: string) => ({ rung: "agents" as const, id: `${MCP_ID_PREFIX}claude/${name}`, label: name, group: "Claude Code MCP servers", paths: ["~/.claude.json"], bytes: 300, default: "bring" as const });
  const clisOf = (plan: ProvisionPlan): string[] => plan.steps.slice(plan.agents).map(s => s.id);
  // A picked npm row has the planner ask the managers here for wsp's own package; these cases ask none.
  const noManagers = () => ({ ...nodeHost(), home, exec: { which: async () => false, run: async () => undefined } });

  it("has the servers wait on every CLI nothing names when it cannot name the one a server runs, and on none for uv, npx or the floor's", async () => {
    writeFileSync(
      join(home, ".claude.json"),
      JSON.stringify({ mcpServers: { files: { command: "mcp-server-filesystem" }, browser: { command: "playwright-mcp" }, py: { command: "uvx", args: ["mcp-py"] }, script: { command: "python3", args: ["s.py"] } } }),
    );
    const npm = ["tools/npm/@modelcontextprotocol/server-filesystem", "tools/npm/@playwright/mcp"];
    const manifest = { ...FIXTURE, entries: [...FIXTURE.entries, ...[...npm, "tools/brew/uv"].map(tool), ...["files", "browser", "py", "script"].map(server)] };
    const picks = {
      name: "laptop",
      agents: { claude: { signin: "vault" as const } },
      mcp: { files: { agents: ["claude"] }, browser: { agents: ["claude"] }, py: { agents: ["claude"] }, script: { agents: ["claude"] } },
      clis: { "@modelcontextprotocol/server-filesystem": { via: "npm" }, "@playwright/mcp": { via: "npm" }, yq: { via: "brew" }, uv: { via: "brew" } },
      skills: {}, plugins: {}, folders: {}, configs: {},
    };
    const plan = await placeProvisioner({ statePath, home, platform: "linux", collect: async () => manifest, brew: async () => new Map(), here: noManagers() }).setup(picks, { home });
    expect(clisOf(plan)).toEqual(expect.arrayContaining([...npm, "tools/brew/yq"]));
    // Neither npm row names its command, so either may be the one the first two servers run; yq names its own, and
    // uv comes with the floor, which python3 does too.
    expect([...(plan.serverTools ?? [])].sort()).toEqual(npm);
  });

  it("has a server running an agent's own command wait on no CLI, since the agents step put it on", async () => {
    writeFileSync(join(home, ".claude.json"), JSON.stringify({ mcpServers: { codex: { command: "codex", args: ["mcp-server"] } } }));
    const manifest = { ...FIXTURE, entries: [...FIXTURE.entries, tool("tools/brew/act"), tool("tools/npm/some-cli"), server("codex")] };
    const picks = {
      name: "laptop",
      agents: { claude: { signin: "vault" as const }, codex: { signin: "machine" as const } },
      mcp: { codex: { agents: ["claude"] } },
      clis: { act: { via: "brew" }, "some-cli": { via: "npm" } },
      skills: {}, plugins: {}, folders: {}, configs: {},
    };
    const plan = await placeProvisioner({ statePath, home, platform: "linux", collect: async () => manifest, brew: async () => new Map(), here: noManagers() }).setup(picks, { home });
    expect(plan.steps.slice(0, plan.agents).map(s => s.bin)).toContain("codex");
    expect(clisOf(plan)).toEqual(expect.arrayContaining(["tools/brew/act", "tools/npm/some-cli"]));
    expect(plan.serverTools).toBeUndefined();
  });

  it("has the servers wait on every CLI when this computer's config does not read", async () => {
    writeFileSync(join(home, ".claude.json"), "{ not json");
    const manifest = { ...FIXTURE, entries: [...FIXTURE.entries, tool("tools/brew/jq"), server("yaml")] };
    const picks = { name: "laptop", agents: { claude: { signin: "vault" as const } }, mcp: { yaml: { agents: ["claude"] } }, clis: { yq: { via: "brew" }, jq: { via: "brew" } }, skills: {}, plugins: {}, folders: {}, configs: {} };
    const plan = await placeProvisioner({ statePath, home, platform: "linux", collect: async () => manifest, brew: async () => new Map() }).setup(picks, { home });
    expect(clisOf(plan).length).toBeGreaterThan(1);
    expect(plan.serverTools).toEqual(clisOf(plan));
  });

});

describe("a computer's picks planned for a sync", () => {
  let dir: string;
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it("plans the skills alone without reading this computer's managers, agents or servers, and reads them for a step that needs them", async () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-place-sync-plan-"));
    const home = join(dir, "home");
    mkdirSync(join(home, ".claude", "skills", "unslop"), { recursive: true });
    writeFileSync(join(home, ".claude", "skills", "unslop", "SKILL.md"), "---\nname: unslop\n---\n");
    let reads = 0;
    const planner = placeProvisioner({ statePath: join(dir, "state.json"), home, platform: "linux", collect: async (): Promise<Manifest> => (reads++, FIXTURE), brew: async () => new Map() });
    const picks = { name: "laptop", agents: { claude: { signin: "vault" as const } }, mcp: {}, clis: {}, skills: { unslop: { from: "~/.claude/skills" } }, plugins: {}, folders: {}, configs: {} };
    const light = await planner.setup(picks, { home }, new Set(["skills"]));
    expect(reads).toBe(0);
    expect(light.steps).toEqual([]);
    expect(light.skills?.lands.map(l => l.dest)).toEqual([".claude/skills/unslop"]);
    await planner.setup(picks, { home }, new Set(["skills", "clis"]));
    expect(reads).toBe(1);
  });

  it("puts gh on for a GitHub row that signs in, none for one set aside, whose Sign in puts it on, and none where gh is a CLI", async () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-place-gh-plan-"));
    const home = join(dir, "home");
    mkdirSync(home, { recursive: true });
    const planner = placeProvisioner({ statePath: join(dir, "state.json"), home, platform: "linux", collect: async (): Promise<Manifest> => FIXTURE, brew: async () => new Map() });
    const picks = (configs: RecipeFile["configs"], clis: RecipeFile["clis"] = {}): RecipeFile => ({ name: "laptop", agents: {}, mcp: {}, clis, skills: {}, plugins: {}, folders: {}, configs });
    expect((await planner.setup(picks({ github: { signin: "machine" } }), { home }, new Set(["github"]))).github?.map(t => t.id)).toEqual(["github/gh"]);
    expect((await planner.setup(picks({ github: { signin: "skip" } }), { home }, new Set(["github"]))).github).toBeUndefined();
    expect((await planner.setup(picks({ github: { signin: "machine" } }, { gh: { via: "brew" } }), { home }, new Set(["github"]))).github).toBeUndefined();
    expect((await planner.setup(picks({}), { home }, new Set(["github"]))).github).toBeUndefined();
  });
});

describe("what taking rows out of a computer's picks runs there", () => {
  it("takes the bubblewrap Codex's row put on off with Codex, only where that row put it on", async () => {
    const before = { name: "laptop", agents: { codex: { signin: "vault" as const } }, mcp: {}, clis: {}, skills: {}, plugins: {}, folders: {}, configs: {} };
    const undo = await undoPlan(before, [{ kind: "agents", name: "codex" }], { home: "/root" }, async () => new Map());
    const by = new Map(undo.map(u => [u.key, u]));
    expect(by.get("agents/codex/bubblewrap")).toMatchObject({ ids: ["agents/codex/bubblewrap"], owner: "agents/codex/bubblewrap", cmd: expect.stringContaining("apt-get purge -y -qq bubblewrap") });
    // The profile comes off only where its row loaded it; a computer that already let bwrap through reads present.
    expect(by.get("agents/codex/bwrap-apparmor")).toMatchObject({ ids: ["agents/codex/bwrap-apparmor"], owner: "agents/codex/bwrap-apparmor", cmd: expect.stringContaining('apparmor_parser -R "$f"') });
    expect(by.get("agents/codex/bwrap-apparmor")?.cmd).toContain('rm -f "$f" /var/cache/apparmor/*/bwrap-userns-restrict');
  });

  it("takes gh off for the GitHub row only by the row that put gh on, and git-lfs's filters off before the tool", async () => {
    const before = { name: "laptop", agents: {}, mcp: {}, clis: { "git-lfs": { via: "brew" }, gh: { via: "brew" } }, skills: {}, plugins: {}, folders: {}, configs: { github: { signin: "machine" as const } } };
    const undo = await undoPlan(before, [{ kind: "configs", name: "github" }, { kind: "clis", name: "git-lfs" }], { home: "/root" }, async () => new Map());
    const by = new Map(undo.map(u => [u.key, u]));
    expect(by.get("configs/github")?.owner).toBe("github/gh");
    const lfs = by.get("clis/git-lfs")?.cmd ?? "";
    expect(lfs).toContain("git lfs uninstall --system");
    expect(lfs.indexOf("git lfs uninstall --system")).toBeLessThan(lfs.indexOf("brew uninstall git-lfs"));
  });

  it("takes a CLI and an agent off by their own roads where they have one, a skill and an agent's own files by the ledger, a plugin by claude's command and a folder by its record", async () => {
    const before = {
      name: "laptop",
      agents: { claude: { signin: "vault" as const } },
      mcp: { linear: { agents: ["claude"] } },
      clis: { cowsay: { via: "npm" }, jq: { via: "apt" } },
      skills: { why: { from: "~/.claude/skills" } },
      plugins: { "lint@acme": {} },
      folders: { app: { from: "~/app", name: "app", keep: [] } },
      configs: { git: {} },
    };
    const removed = [
      { kind: "agents" as const, name: "claude" },
      { kind: "clis" as const, name: "cowsay" },
      { kind: "clis" as const, name: "jq" },
      { kind: "skills" as const, name: "why" },
      { kind: "plugins" as const, name: "lint@acme" },
      { kind: "folders" as const, name: "app" },
      { kind: "mcp" as const, name: "linear" },
      { kind: "configs" as const, name: "git" },
    ];
    const undo = await undoPlan(before, removed, { home: "/root" }, async () => new Map());
    const by = new Map(undo.map(u => [u.key, u]));
    // Named where the plan landed them on that computer, under Claude Code's store there.
    expect(by.get("agents/claude")).toMatchObject({ ids: expect.arrayContaining(["agents/claude", "signins/claude", "files/.claude-cfg/settings.json"]), dests: expect.arrayContaining([".claude-cfg/settings.json", ".claude-cfg/CLAUDE.md"]) });
    expect(by.get("agents/claude")?.dests).not.toContain(".claude-cfg/.claude.json");
    // Claude Code's script road names no uninstall, which its row says.
    expect(by.get("agents/claude")?.note).toContain("no uninstaller");
    expect(by.get("clis/cowsay")).toMatchObject({ ids: ["tools/npm/cowsay"], cmd: expect.stringContaining("npm uninstall -g cowsay") });
    // A floor row is the base's and stays, which its row says.
    expect(by.get("clis/jq")).toMatchObject({ ids: ["tools/apt/jq"], note: "jq is part of the base and stays" });
    expect(by.get("skills/why")).toMatchObject({ dests: [".claude/skills/why"] });
    expect(by.get("plugins/lint@acme")?.cmd).toContain("claude plugin uninstall 'lint@acme'");
    expect(by.get("folders/app")).toMatchObject({ folder: "app", ids: ["folders/app"], owner: "folders/app" });
    // A road runs only where the row it installed reads installed: the agent's own row, the CLI's tool row.
    expect(by.get("agents/claude")?.owner).toBe("agents/claude");
    expect(by.get("clis/cowsay")?.owner).toBe("tools/npm/cowsay");
    expect(by.get("plugins/lint@acme")?.owner).toBe("plugins/lint@acme");
    expect(by.get("mcp/linear")).toMatchObject({ ids: [`${MCP_ID_PREFIX}claude/linear`] });
    expect(by.get("mcp/linear")?.cmd).toBeUndefined();
    expect(by.get("configs/git")).toMatchObject({ dests: [".gitconfig", ".config/git/config", ".config/git/ignore", ".config/git/attributes"] });
  });
});
