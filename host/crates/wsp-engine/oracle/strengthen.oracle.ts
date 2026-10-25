// SPDX-License-Identifier: AGPL-3.0-only
// Scenarios the first specs left out, run through the TS at a4033dc34 on the box guest (a temp directory as the
// computer), recorded as tests/fixtures/oracle/strengthen_*.json for the Rust hard tests the specs' S-rules name.
// Every temp root is written as {root}. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/strengthen.vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, it } from "vitest";
import { CODEX_TOML, MCP_SERVERS_JSON } from "../../../../packages/catalog/src/index.js";
import { TOOLS_PATH, placeProvisionPaths } from "../../../../packages/protocol/src/index.js";
import type { McpPlan } from "../../../../packages/engine/src/golden-mcp.js";
import type { PackedFiles } from "../../../../packages/engine/src/golden.js";
import { closeAgentFiles, oncePathsOf, provisionFiles, type ProvisionLanding } from "../../../../packages/engine/src/provision-files.js";
import { provisionMcp } from "../../../../packages/engine/src/provision-mcp.js";
import { tarOf } from "../../../../packages/engine/src/vault.js";
import { boxGuest, cleanGuests, type BoxGuest } from "../../../../packages/engine/test/box-guest.js";

const OUT = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const HOME = "/Users/dev";
const guests: BoxGuest[] = [];
const out: Record<string, unknown> = {};
afterAll(() => {
  cleanGuests(guests.splice(0));
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, "strengthen_mcp.json"), `${JSON.stringify(out, null, 2)}\n`);
});

const box = (present: string[] = ["npx"]): BoxGuest => {
  const g = boxGuest(present);
  guests.push(g);
  return g;
};
const norm = (root: string, v: unknown): unknown => JSON.parse(JSON.stringify(v ?? null).split(root).join("{root}"));
const packed = (tar: Buffer): PackedFiles => ({ tar, bytes: tar.length, unpacked: tar.length, skipped: [], cut: [], silenced: [], macPaths: [] });
const pretty = (v: unknown): string => `${JSON.stringify(v, null, 2)}\n`;

const CLAUDE_ON_BOX = pretty({ numStartups: 41, mcpServers: { mine: { command: "/usr/local/bin/mine", args: [] } } });
const CLAUDE_TRAVELLED = pretty({
  numStartups: 7,
  mcpServers: { gsc: { command: "npx", args: ["-y", "gsc-mcp", "--stdio"] }, notes: { command: `${HOME}/Library/Notes/mcp`, args: [] }, mine: { command: "npx", args: ["theirs-on-the-mac"] } },
});
const CODEX_TRAVELLED = ['model = "gpt-5"', "", "[mcp_servers.context7]", 'command = "npx"', 'args = ["-y", "context7"]', "", "[mcp_servers.grafana]", 'command = "npx"', ""].join("\n");
const LANDS: ProvisionLanding[] = [
  { id: "agents/claude", label: "Claude Code", dest: ".claude-cfg/.claude.json", once: true },
  { id: "agents/codex", label: "Codex", dest: ".codex/config.toml", once: true },
];
const planOn = (root: string): McpPlan => ({
  agents: [
    {
      id: "claude",
      label: "Claude Code",
      scopes: [{ files: [join(root, ".claude-cfg/.claude.json")], format: MCP_SERVERS_JSON, keep: ["gsc", "mine"], drop: [{ name: "notes", reason: "command is macOS-only, will not run" }] }],
      aside: [],
    },
    { id: "codex", label: "Codex", scopes: [{ files: [join(root, ".codex/config.toml")], format: CODEX_TOML, keep: ["context7"], drop: [{ name: "grafana", reason: "unticked" }] }], aside: [] },
  ],
  guestHome: root,
  rewrites: [[`${HOME}/`, `${root}/`]],
  binDirs: [],
  tools: [],
});
const landing = (root: string): string[] => (existsSync(placeProvisionPaths(root).landing) ? readFileSync(placeProvisionPaths(root).landing, "utf8").split("\n").filter(l => l !== "") : []);
const listed = (root: string): string[] => (existsSync(placeProvisionPaths(root).landed) ? readFileSync(placeProvisionPaths(root).landed, "utf8").split("\n").filter(l => l !== "") : []);
const rows = (rs: { id: string; outcome: string; note?: string }[]) => rs.map(r => [r.id, r.outcome, r.note ?? null]);

async function round(g: BoxGuest, plan: McpPlan, o: { files?: boolean; into?: [string, string[]][] } = {}) {
  const landed = o.files === false ? { owned: new Map() } : await provisionFiles(g.machine, { home: g.root, lands: LANDS, pack: async () => packed(tarOf([{ path: ".claude-cfg/.claude.json", mode: 0o600, content: CLAUDE_TRAVELLED }, { path: ".codex/config.toml", mode: 0o600, content: CODEX_TRAVELLED }])) });
  const before = landing(g.root).length;
  const servers = await provisionMcp(g.machine, plan, {
    home: g.root,
    landed: landed.owned as never,
    tools: [],
    path: TOOLS_PATH,
    stage: () => {},
    ...(o.into !== undefined ? { into: (file: string, names: readonly string[]) => o.into!.push([file, [...names]]) } : {}),
  });
  const appended = landing(g.root).slice(before);
  await closeAgentFiles(g.machine, g.root, oncePathsOf(LANDS));
  return { rows: rows(servers), appended, listed: listed(g.root) };
}

it("two rounds over a person's own server: records, then tombstones, in planned order", async () => {
  const g = box();
  mkdirSync(join(g.root, ".claude-cfg"), { recursive: true });
  writeFileSync(join(g.root, ".claude-cfg/.claude.json"), CLAUDE_ON_BOX);
  const into: [string, string[]][] = [];
  const first = await round(g, planOn(g.root), { into });
  const second = await round(g, planOn(g.root));
  out.theirsTwoRounds = norm(g.root, { first, firstInto: into, second, claude: readFileSync(join(g.root, ".claude-cfg/.claude.json"), "utf8"), codex: readFileSync(join(g.root, ".codex/config.toml"), "utf8") });
});

it("a scope whose folder cannot be marked trusted: every kept name aside, nothing planned for it", async () => {
  const g = box();
  mkdirSync(join(g.root, ".codex"), { recursive: true });
  const codexOnBox = ['model = "o3"', "", 'projects = { "/root/repo" = { trust_level = "untrusted" } }', ""].join("\n");
  writeFileSync(join(g.root, ".codex/config.toml"), codexOnBox);
  const plan: McpPlan = {
    ...planOn(g.root),
    agents: [
      {
        id: "codex",
        label: "Codex",
        scopes: [
          { files: [join(g.root, ".codex/config.toml")], format: CODEX_TOML, keep: ["context7"], drop: [{ name: "grafana", reason: "unticked" }], travelled: CODEX_TRAVELLED, folder: "/root/repo", trust: "/root/repo" },
        ],
        aside: [],
      },
    ],
  };
  const into: [string, string[]][] = [];
  const r = await round(g, plan, { files: false, into });
  out.untrusted = norm(g.root, { ...r, into, codex: readFileSync(join(g.root, ".codex/config.toml"), "utf8"), codexOnBox });
});

it("a round whose copy never travelled: the file is read, not written, and nothing is appended", async () => {
  const g = box();
  mkdirSync(join(g.root, ".claude-cfg"), { recursive: true });
  const onBox = pretty({ mcpServers: { gsc: { command: "npx", args: ["-y", "gsc-mcp", "--stdio"] } } });
  writeFileSync(join(g.root, ".claude-cfg/.claude.json"), onBox);
  const into: [string, string[]][] = [];
  const r = await round(g, { ...planOn(g.root), agents: planOn(g.root).agents.slice(0, 1) }, { files: false, into });
  out.noCopy = norm(g.root, { ...r, into, claude: readFileSync(join(g.root, ".claude-cfg/.claude.json"), "utf8"), onBox });
});

it("a read that exits non-zero: every kept server skipped, no into, nothing appended", async () => {
  const g = box();
  const run = g.machine.run.bind(g.machine);
  Object.assign(g.machine, { run: async (script: string, opts: never) => (script.startsWith("wsp_mcp_read() {") ? { exitCode: 3, stdout: "", stderr: "read: permission denied" } : run(script, opts)) });
  const into: [string, string[]][] = [];
  const r = await round(g, planOn(g.root), { into });
  out.readFailed = norm(g.root, { ...r, into, runs: g.runs.length });
});
