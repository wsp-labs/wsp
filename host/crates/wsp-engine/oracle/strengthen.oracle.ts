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
import { BASE_FLOOR } from "../../../../packages/catalog/src/index.js";
import { baseVersionsCmd } from "../../../../packages/engine/src/golden-base.js";
import type { ToolInstall } from "../../../../packages/engine/src/golden-import.js";
import { FREE_KB_CMD } from "../../../../packages/engine/src/golden-tools.js";
import type { ExecResult, Machine } from "../../../../packages/engine/src/machine.js";
import { newSetupRun, provisionStep, type ProvisionPlan } from "../../../../packages/engine/src/provision.js";
import { boxGuest, cleanGuests, type BoxGuest } from "../../../../packages/engine/test/box-guest.js";

const OUT = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const HOME = "/Users/dev";
const guests: BoxGuest[] = [];
const out: Record<string, unknown> = {};
const step: Record<string, unknown> = {};
afterAll(() => {
  cleanGuests(guests.splice(0));
  mkdirSync(OUT, { recursive: true });
  // A run filtered with -t leaves some of these empty; an empty one is not written over a full one.
  if (Object.keys(out).length > 0) writeFileSync(join(OUT, "strengthen_mcp.json"), `${JSON.stringify(out, null, 2)}\n`);
  if (Object.keys(step).length > 0) writeFileSync(join(OUT, "strengthen_step.json"), `${JSON.stringify(step, null, 2)}\n`);
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

// ---- provision_step: the tools loop and the plugins step (strengthen_step.json)

const FLOOR_READ = [...BASE_FLOOR.flatMap(e => [`VERSION ${e.bin}: ${e.bin} ${e.major === undefined ? "9.9.9" : `${e.major.version}.0`}`, ...(e.brings ?? []).map(b => `VERSION ${b.bin}: 9.9.9`)]), "VERSION sshd: OpenSSH_9.6p1"].join("\n");
function boxMachine(answer: (cmd: string) => ExecResult | undefined): Machine {
  return {
    id: "spoo",
    kind: "sandbox",
    exec: async (cmd: string) => {
      const said = answer(cmd);
      if (said !== undefined) return said;
      if (cmd === FREE_KB_CMD) return { exitCode: 0, stdout: "9000000\n", stderr: "" };
      if (cmd === baseVersionsCmd(TOOLS_PATH)) return { exitCode: 0, stdout: `${FLOOR_READ}\n`, stderr: "" };
      return { exitCode: 0, stdout: "", stderr: "" };
    },
    run: async (script: string) => answer(script) ?? { exitCode: 0, stdout: "", stderr: "" },
    putBytes: async () => undefined,
  } as unknown as Machine;
}
const tool = (over: Partial<ToolInstall> & Pick<ToolInstall, "id" | "label">): ToolInstall => ({ manager: "script", cmd: `install ${over.id}`, ...over });
const planOf = (steps: ToolInstall[], over: Partial<ProvisionPlan> = {}): ProvisionPlan => ({ recipeAt: "x", path: TOOLS_PATH, steps, skipped: [], agents: 0, compiler: false, ...over });
const bare = (r: { id: string; label: string; outcome: string; note?: string; step?: string }) => ({ id: r.id, label: r.label, outcome: r.outcome, note: r.note ?? null, step: r.step ?? null });

it("the tools loop: a check that fails after install, a hook that fails, every stage call in order", async () => {
  const machine = boxMachine(cmd => {
    if (cmd.includes("wsp-check")) return { exitCode: 0, stdout: "wsp-check 0 Error: brew cannot read the folder.\n", stderr: "" };
    if (cmd.includes("git lfs install")) return { exitCode: 2, stdout: "", stderr: "warming up\ngit: 'lfs' is not a git command\n" };
    return undefined;
  });
  const steps = [
    tool({ id: "tools/brew/bat", label: "bat", manager: "brew", check: "brew list --versions bat" }),
    tool({ id: "tools/brew/git-lfs", label: "Git LFS", bin: "git-lfs" }),
    tool({ id: "tools/brew/jq", label: "jq", bin: "jq" }),
  ];
  const run = newSetupRun();
  const staged: unknown[] = [];
  const rows = await provisionStep(machine, planOf(steps), "clis", run, (detail, at, row) => void staged.push([detail, at === undefined ? null : `${at.index}/${at.of}: ${at.label}`, row === undefined ? null : bare(row)]), { home: "/root" });
  step.toolsLoop = { staged, rows: rows.map(bare), runTools: run.tools.map(t => [t.id, t.outcome, t.note ?? null]) };
});

it("the plugins step: blank output, a rejected run, stdout when stderr is blank, a 400-character last line", async () => {
  const long = "y".repeat(400);
  const machine = {
    id: "spoo",
    kind: "sandbox",
    exec: async () => ({ exitCode: 0, stdout: "", stderr: "" }),
    run: async (script: string) => {
      if (script.endsWith("\nadd blank")) return { exitCode: 3, stdout: "", stderr: "" };
      if (script.endsWith("\nadd gone")) throw new Error("the machine went away\nconnection reset");
      if (script.endsWith("\nadd quiet")) return { exitCode: 1, stdout: "  first\nlast of stdout  \n", stderr: "" };
      if (script.endsWith("\nadd long")) return { exitCode: 1, stdout: "", stderr: `${long}\n\n` };
      return { exitCode: 0, stdout: "", stderr: "" };
    },
  } as unknown as Machine;
  const plugin = (name: string) => ({ id: `plugins/${name}`, label: name, cmd: `add ${name}`, asked: (out: string) => (out.includes("never") ? "asked" : undefined) });
  const staged: unknown[] = [];
  const rows = await provisionStep(machine, planOf([], { plugins: ["blank", "gone", "quiet", "long", "fine"].map(plugin) }), "plugins", newSetupRun(), (detail, _at, row) => void staged.push([detail, row === undefined ? null : bare(row)]), { home: "/root" });
  step.plugins = { rows: rows.map(bare), staged };
});

// ---- solari_machine: which read settles a pause or a resume, and the warn lines (strengthen_solari.json)
import { SolariBackend } from "../../../../packages/engine/src/solari-backend.js";

it("solari: the state read after a call that did not answer, and every warn line", async () => {
  const solari: Record<string, unknown> = {};
  type Play = "hang" | "netfail" | { status: number; body: unknown };
  const reads = (state: string): Play => ({ status: 200, body: { sandboxId: "x", kind: "sandbox", state } });
  const notFound: Play = { status: 404, body: { error: "sandbox not found" } };
  async function scenario(move: "pause" | "resume", routes: Record<string, Play[]>) {
    const sent: Record<string, number> = {};
    const fetch = (async (url: string, init: { method?: string; signal?: AbortSignal }) => {
      const key = `${init.method ?? "GET"} ${new URL(url).pathname}`;
      sent[key] = (sent[key] ?? 0) + 1;
      if (key === "POST /sandboxes") return new Response(JSON.stringify({ sandboxId: "x", kind: "sandbox" }), { status: 201 });
      const plays = routes[key] ?? [];
      const play = plays[Math.min(sent[key]! - 1, plays.length - 1)];
      if (play === "hang") return new Promise((_, reject) => init.signal?.addEventListener("abort", () => reject(init.signal!.reason)));
      if (play === "netfail" || play === undefined) throw new TypeError("fetch failed");
      return new Response(JSON.stringify(play.body), { status: play.status });
    }) as never;
    const warned: string[] = [];
    const warn = console.warn;
    console.warn = (line: string) => void warned.push(line);
    try {
      const b = new SolariBackend({ apiKey: "k", fetch, budgets: { pauseMs: 40, resumeCapMs: 20, stateReadMs: 20 } });
      const m = await b.create({ kind: "sandbox" } as never);
      let error: string | null = null;
      let name: string | null = null;
      try {
        await m[move]();
      } catch (e) {
        error = (e as Error).message.replace(/in \d+ms/, "in <n>ms");
        name = (e as Error).name;
      }
      return { error, name, sent: { move: sent[`POST /sandboxes/x/${move}`] ?? 0, state: sent["GET /sandboxes/x"] ?? 0 }, warned: warned.map(l => l.replace(/in \d+ms/, "in <n>ms")) };
    } finally {
      console.warn = warn;
    }
  }
  solari.pauseStarting = await scenario("pause", { "POST /sandboxes/x/pause": ["hang"], "GET /sandboxes/x": [reads("starting")] });
  solari.pauseArchived = await scenario("pause", { "POST /sandboxes/x/pause": ["hang"], "GET /sandboxes/x": [reads("archived")] });
  solari.pauseNotFound = await scenario("pause", { "POST /sandboxes/x/pause": ["hang"], "GET /sandboxes/x": [notFound] });
  solari.pauseUnknownWord = await scenario("pause", { "POST /sandboxes/x/pause": ["hang", "hang"], "GET /sandboxes/x": [reads("hibernating"), reads("paused")] });
  solari.pauseLands = await scenario("pause", { "POST /sandboxes/x/pause": [{ status: 200, body: {} }] });
  solari.pauseRefused = await scenario("pause", { "POST /sandboxes/x/pause": [{ status: 409, body: { error: "Not pausable" } }] });
  solari.resumePaused = await scenario("resume", { "POST /sandboxes/x/resume": ["hang"], "GET /sandboxes/x": [reads("paused")] });
  solari.resumeNotFound = await scenario("resume", { "POST /sandboxes/x/resume": ["hang"], "GET /sandboxes/x": [notFound] });
  solari.resumeUnread = await scenario("resume", { "POST /sandboxes/x/resume": ["hang"], "GET /sandboxes/x": ["netfail"] });
  solari.resumeRunning = await scenario("resume", { "POST /sandboxes/x/resume": ["hang"], "GET /sandboxes/x": [reads("running")] });
  solari.resumeStarting = await scenario("resume", { "POST /sandboxes/x/resume": ["hang"], "GET /sandboxes/x": [reads("starting")] });
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, "strengthen_solari.json"), `${JSON.stringify(solari, null, 2)}\n`);
}, 60_000);

// ---- child_exec: the drain after the exit (strengthen_child.json)
import { runChild } from "../../../../packages/engine/src/child-exec.js";

it("child exec: a signal death, the drain's quiet restarted by stderr, an exit with its pipes closed", async () => {
  const child: Record<string, unknown> = {};
  const lines: string[] = [];
  child.signalled = await runChild("/bin/sh", ["-c", "echo hi; kill -9 $$"]);
  const started = Date.now();
  const drained = await runChild("/bin/sh", ["-c", "(sleep 0.1; echo a; sleep 0.15; echo b >&2; sleep 0.15; echo c; sleep 3) & exit 4"], { onLine: l => void lines.push(l) });
  child.drained = { ...drained, lines, tookAtLeast500: Date.now() - started >= 500 };
  const t0 = Date.now();
  for (let i = 0; i < 10; i++) await runChild("/bin/sh", ["-c", "exit 5"]);
  child.tenQuickExitsMs = Date.now() - t0;
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, "strengthen_child.json"), `${JSON.stringify(child, null, 2)}\n`);
}, 60_000);

// ---- project state: half-done moves, the gemini registry, line rewrites (strengthen_pstate.json)
import { linkSync, mkdtempSync, readdirSync, lstatSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { countProjectState, filesUnder, moveProjectState } from "../../../../packages/engine/src/project-state/index.js";
import { geminiResolver } from "../../../../packages/engine/src/project-state/gemini.js";
import { rewriteCwd } from "../../../../packages/engine/src/project-state/resolver.js";

it("project state", async () => {
  const ps: Record<string, unknown> = {};
  const homeOf = (): string => mkdtempSync(join(tmpdir(), "wsp-ps-"));
  const put = (root: string, rel: string, text: string | Buffer): void => {
    mkdirSync(join(root, rel, ".."), { recursive: true });
    writeFileSync(join(root, rel), text);
  };
  const tree = (root: string): Record<string, string> => {
    const out: Record<string, string> = {};
    const walk = (rel: string): void => {
      for (const name of readdirSync(join(root, rel)).sort()) {
        const r = rel === "" ? name : `${rel}/${name}`;
        const st = lstatSync(join(root, r));
        if (st.isDirectory()) walk(r);
        else out[r] = st.isSymbolicLink() ? "<link>" : readFileSync(join(root, r), "utf8");
      }
    };
    walk("");
    return out;
  };
  const scrub = (h: string, v: unknown): unknown => JSON.parse(JSON.stringify(v).split(h).join("{home}"));
  const move = async (h: string, agent: string) => scrub(h, await moveProjectState({ from: "/root/p", to: "/new/p", homes: { [agent]: h } }, [{ id: agent }]));

  let h = homeOf();
  put(h, "projects/-root-p/S1.jsonl", '{"cwd":"/root/p"}\n');
  put(h, "projects/-root-p-sub/S2.jsonl", '{"cwd":"/root/p/sub"}\n');
  put(h, "projects/-new-p-sub/S9.jsonl", '{"cwd":"/new/p/sub"}\n');
  ps.claudeHalfMoved = { report: await move(h, "claude"), tree: tree(h) };

  h = homeOf();
  put(h, "projects.json", '{"zz":1,"projects":{"/other":"o","/root/p":"s","/root/p/sub":"s2"},"a":{"10":1,"9":[1.0,{}]},"n":1.0}');
  ps.geminiRegistry = { report: await move(h, "gemini"), tree: tree(h) };

  h = homeOf();
  put(h, "projects.json", '{"projects":{"/root/p":"s","/root/p/sub":"s2"}}\n');
  put(h, "tmp/s/.project_root", "/root/p");
  put(h, "tmp/s2/.project_root", "/root/p/sub");
  linkSync(join(h, "tmp/s2/.project_root"), join(h, "second-name"));
  put(h, "history/s/.project_root", "/root/p");
  ps.geminiMarkerLinked = { report: await move(h, "gemini"), tree: tree(h) };

  const registries = ['{"version":1}', '{"projects":{"/root/p":5}}', '{"projects":{"/root/p":"s","/new/p":null}}', '{"projects":{"/root/p":"../x","/new/p":"y"}}', '{"projects":{"/root/p":"../x","/root/p/q":"ok"}}', '{"projects":[]}'];
  ps.geminiRegistries = [];
  for (const text of registries) {
    h = homeOf();
    put(h, "projects.json", text);
    (ps.geminiRegistries as unknown[]).push({ registry: text, report: await move(h, "gemini"), after: readFileSync(join(h, "projects.json"), "utf8") });
  }

  h = homeOf();
  put(h, "projects.json", '{"projects":{"/root/p":"s","/root/p/sub":"s2","/other":"o"}}');
  ps.geminiMerge = scrub(h, await geminiResolver.merge!(h, "/root/p", "/new/p", "/g/.gemini"));

  h = homeOf();
  put(h, "projects.json", '{"projects":{"/root/p":"s","/root/p/sub":"s"}}');
  put(h, "tmp/s/chats/c1.jsonl", "{}\n");
  put(h, "tmp/s/.project_root", "/root/p\n");
  ps.geminiDuplicateSlugs = scrub(h, await countProjectState("/root/p", { gemini: h }, [{ id: "gemini", name: "Gemini" }]));

  const lines: [string, string | Buffer][] = [
    ["invalid utf8", Buffer.concat([Buffer.from('{"cwd":"/old"}\nraw '), Buffer.from([0xff, 0xfe]), Buffer.from(" tail\n")])],
    ["big integers", '{"cwd":"/old","id":9007199254740993,"u":12345678901234567890,"neg":-9007199254740993}\n'],
    ["overflow", '{"cwd":"/old","x":1e400}\n'],
    ["lone surrogate", '{"cwd":"/old","t":"\\ud83d","u":"\\ude00x\\ud83d\\ud83d"}\n'],
    ["number spelling", '{"cwd":"/old","z":-0,"e":1E2,"f":1.50,"g":0.0000001,"h":1e21}\n'],
    ["duplicate keys", '{"a":1,"cwd":"/x","cwd":"/old","b":2}\n'],
    ["index keys", '{"b":1,"01":2,"-1":3,"4294967295":4,"4294967294":5,"0":6,"1.5":7,"cwd":"/old","1e3":8,"10":9,"m":{"b":1,"10":2,"9":3},"arr":[{"2":1,"1":2}]}\n'],
    ["crlf and spaces", '{"cwd":"/old"}\r\n{"cwd":"/keep"}\r\n  {"cwd":"/old/x"}  \n'],
    ["bom", '﻿{"cwd":"/old"}\n'],
  ];
  ps.rewriteCwd = [];
  for (const [name, text] of lines) {
    h = homeOf();
    put(h, "f.jsonl", text);
    const n = await rewriteCwd(join(h, "f.jsonl"), "/old", "/new");
    (ps.rewriteCwd as unknown[]).push({ name, input: Buffer.from(text).toString("base64"), n, output: readFileSync(join(h, "f.jsonl")).toString("base64") });
  }

  h = homeOf();
  put(h, "real/a.jsonl", "a\n");
  put(h, "real/sub/b.jsonl", "b\n");
  mkdirSync(join(h, "t"));
  symlinkSync("../real", join(h, "t/linkdir"));
  put(h, "t/x.jsonl", "x\n");
  for (const f of ["S1.jsonl", "S1/a.jsonl", "S1-b.jsonl", "S1_c.jsonl", "S10.jsonl"]) put(h, `u/${f}`, "{}\n");
  ps.filesUnder = { linked: scrub(h, filesUnder(join(h, "t"), "")), sorted: scrub(h, filesUnder(join(h, "u"), ".jsonl")) };

  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, "strengthen_pstate.json"), `${JSON.stringify(ps, null, 2)}\n`);
}, 60_000);
