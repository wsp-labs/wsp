// SPDX-License-Identifier: AGPL-3.0-only
// The TS provision slice's own answers (provision-files.ts, provision.ts, vault.ts, provision-mcp.ts at a4033dc34),
// written as tests/fixtures/oracle/provision.json for the Rust hard tests (spec/*.md names which). Every script text
// here is a contract: the Rust port must print it byte for byte. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/provision.vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { it, vi } from "vitest";
import { CODEX_TOML, MCP_SERVERS_JSON } from "../../../../packages/catalog/src/index.js";
import {
  closeFilesScript,
  filesRows,
  landFilesScript,
  landedAt,
  landedFilesScript,
  landedServersScript,
  mergedIntoLine,
  outsideAfterScript,
  outsideBeforeScript,
  outsideMarks,
  outsideSweepScript,
  ownMarks,
  parseLanded,
  parseLandedServers,
  parseStanding,
  serverDigest,
  serversOutLines,
  standingScript,
  storeRoots,
  unlandScript,
  agentStateFile,
} from "../../../../packages/engine/src/provision-files.js";
import { atHome, keyUnreachedLine, noCopyLine, theirServerLine, untrustedLine } from "../../../../packages/engine/src/provision-mcp.js";
import { outsideRoots, presenceTests, presentByWhatWaits, presentElsewhere, provisionCountsOf, type ProvisionPlan } from "../../../../packages/engine/src/provision.js";
import { exportPaths, fitsTar, folderExportScript, importInto, landBundle, tarOf, type CacheRule } from "../../../../packages/engine/src/vault.js";
import type { ToolInstall } from "../../../../packages/engine/src/golden-import.js";
import type { McpPlan } from "../../../../packages/engine/src/golden-mcp.js";
import type { ExecResult, Machine } from "../../../../packages/engine/src/machine.js";

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const fixture: Record<string, unknown> = {};

const ROOTS: [string, string][] = [[".codex", "/var/lib/wsp/logins/codex"], [".odd'dir", "/srv/store with space"]];
const ONCE = [".claude-cfg/.claude.json", ".codex/config.toml", "odd*glob\\path"];
const RULE: CacheRule = { dirs: ["node_modules", ".next", "it's"], files: ["dist"], markers: ["CACHEDIR.TAG", ".wsp-cache"] };

/** A machine that records every exec and run script and answers ok, with an archive of `bytes` at every path. */
function recorder(): { machine: Machine; execs: string[]; runs: string[] } {
  const execs: string[] = [];
  const runs: string[] = [];
  const ok = (stdout = ""): ExecResult => ({ exitCode: 0, stdout, stderr: "" });
  const machine = {
    id: "m",
    kind: "sandbox",
    exec: async (cmd: string) => {
      execs.push(cmd);
      return cmd.startsWith("wc -c") ? ok("12\n") : cmd.startsWith("test -e") ? ok("no\n") : ok();
    },
    run: async (script: string) => {
      runs.push(script);
      return ok();
    },
    downloadUrl: async (p: string) => `https://x/dl?path=${encodeURIComponent(p)}`,
    uploadUrl: async (p: string) => `https://x/ul?path=${encodeURIComponent(p)}`,
  } as unknown as Machine;
  return { machine, execs, runs };
}

const fetchOk = (async (_url: string, init?: RequestInit) => (init?.method === "PUT" ? new Response(null, { status: 200 }) : new Response(new Uint8Array(Buffer.from("tgz-bytes-12")), { status: 200 }))) as unknown as typeof fetch;

it("provision-files scripts and parsers", () => {
  fixture["standingScript"] = { "/root": standingScript("/root"), "/home/dev o'q": standingScript("/home/dev o'q") };
  fixture["landFilesScript"] = {
    plain: landFilesScript("/root"),
    onceAndRoots: landFilesScript("/root", ONCE, ROOTS),
  };
  fixture["landedServersScript"] = landedServersScript("/root");
  fixture["landedFilesScript"] = { "/root": landedFilesScript("/root"), "{home}": landedFilesScript("{home}") };
  fixture["closeFilesScript"] = { plain: closeFilesScript("/root"), onceAndRoots: closeFilesScript("/root", ONCE, ROOTS) };
  fixture["unlandScript"] = {
    oneDest: unlandScript("/root", [".claude/skills/a"]),
    withRoots: unlandScript("/root", [".claude/skills/a", ".codex/prompts", ".codex", ".odd'dir/x y"], ROOTS),
  };
  const walked = ["/usr/local/bin", "/opt", "/usr/local/lib/node_modules"];
  fixture["outside"] = {
    before: outsideBeforeScript("clis", walked),
    beforeRooted: outsideBeforeScript("floor", walked, "/tmp/scratch root"),
    beforeNone: outsideBeforeScript("clis", []),
    after: outsideAfterScript("clis", walked),
    afterRooted: outsideAfterScript("floor", walked, "/tmp/scratch root"),
    afterNone: outsideAfterScript("clis", []),
    sweep: outsideSweepScript(),
    sweepTemplate: outsideSweepScript("{root}"),
  };
  const standingIn = "wsp-stand\td1\t.a/b\0wsp-ledger\t.a/b\td2\0wsp-stand\td3\tpath\twith\ttabs\0wsp-stand\tonly-two\0junk\0wsp-ledger\tx\ty\tz\0wsp-stand\td4\t.a/b\0\0";
  const standing = parseStanding(standingIn);
  fixture["parseStanding"] = { input: standingIn, at: [...standing.at], listed: [...standing.listed] };
  const landedIn = ["wsp-land\tinstalled\td\ta/b", "wsp-land\tbogus\td\tx", "wsp-land\tkept\td", "wsp-land\tpresent\td\tp\tq", "xwsp-land\tfailed\td\tz", "wsp-land\tfailed\td\t", ""].join("\n");
  fixture["parseLanded"] = { input: landedIn, out: parseLanded(landedIn) };
  const serversIn = ["wsp-server\td1\tagents/mcp/claude/a", "wsp-server\td2", "wsp-server\td3\tagents/mcp/x\ty", "wsp-server\td4\tagents/mcp/claude/a", "other\td\tid"].join("\n");
  fixture["parseLandedServers"] = { input: serversIn, out: [...parseLandedServers(serversIn)] };
  const ownIn = ["wsp-own\t.a/b", "wsp-own\t", "wsp-own\t/abs", "wsp-own\ta/../b", "wsp-own\ta/..b", "wsp-own\tt\tab", "nope\tx", "wsp-own\t.."].join("\n");
  fixture["ownMarks"] = { input: ownIn, out: ownMarks(ownIn) };
  const outsideIn = "wsp-outside\t/usr/local/bin/claude\nsomething else\nwsp-outside\trelative\nwsp-outside\t\nwsp-outside\t/opt/a\tb\n";
  fixture["outsideMarks"] = { input: outsideIn, out: outsideMarks(outsideIn) };
  fixture["serverDigest"] = { "": serverDigest(""), "{\"command\":\"npx\"}": serverDigest('{"command":"npx"}'), "é": serverDigest("é") };
  fixture["mergedIntoLine"] = { none: mergedIntoLine([]), two: mergedIntoLine(["a", "b"]) };
  fixture["serversOutLines"] = { took: serversOutLines({ path: "/root/.claude.json", names: ["a", "b"] }), unread: serversOutLines({ path: "/root/.claude.json", unread: "it is a link" }) };
  const stores = { codex: "/var/lib/wsp/logins/codex", claude: "/root/.claude-cfg", gemini: "/root/.gemini" };
  const roots = storeRoots("/root", stores);
  fixture["storeRoots"] = { stores, roots, landedAt: [".codex", ".codex/AGENTS.md", ".codexx/a", ".claude-cfg/CLAUDE.md", ".gemini/GEMINI.md"].map(rel => [rel, landedAt("/root", roots, rel)]) };
  fixture["agentStateFile"] = [
    ["agents/claude", "/root/.claude/skills/x/SKILL.md"],
    ["agents/claude", "/root/.claude"],
    ["agents/claude", "/root/.claudex/a"],
    ["agents/mcp/claude/github", "/root/.claude.json"],
    ["agents/mcp/claude/@%2Froot%2Frepo/github", "/root/.claude.json"],
    ["agents/mcp/mcp-remote", "/root/.mcp-auth/x"],
    ["agents/codex", "/root/.codex/config.toml"],
    ["agents/opencode", "/root/.config/opencode/opencode.json"],
    ["agents/nobody", "/root/.nobody/x"],
  ].map(([id, source]) => [id, source, agentStateFile({ id: id!, source: source! }, "/root")]);
  const lands = [
    { id: "agents/claude", label: "Claude Code", dest: ".claude-cfg/skills" },
    { id: "agents/claude", label: "Claude Code", dest: ".claude-cfg/skills/why" },
    { id: "agents/codex", label: "Codex", dest: ".codex/AGENTS.md" },
    { id: "agents/codex", label: "Codex", dest: ".codex/config.toml", once: true as const },
    { id: "agents/gemini", label: "Gemini CLI", dest: ".gemini/GEMINI.md" },
    { id: "agents/claude", label: "Claude Code", dest: ".claude-cfg/CLAUDE.md" },
  ];
  const landed = parseLanded(
    [
      "wsp-land\tinstalled\td\t.claude-cfg/skills/a/SKILL.md",
      "wsp-land\tkept\td\t.claude-cfg/skills/b/SKILL.md",
      "wsp-land\tkept\td\t.claude-cfg/skills/c/SKILL.md",
      "wsp-land\tkept\td\t.claude-cfg/skills/d/SKILL.md",
      "wsp-land\tkept\td\t.claude-cfg/skills/e/SKILL.md",
      "wsp-land\tpresent\td\t.claude-cfg/skills/why/SKILL.md",
      "wsp-land\tfailed\td\t.claude-cfg/skills/why/refs.md",
      "wsp-land\tpresent\td\t.codex/config.toml",
      "wsp-land\tkept\td\t.gemini/GEMINI.md",
      "wsp-land\tinstalled\td\t.loose/hook.sh",
      "wsp-land\tpresent\td\t.loose/two.sh",
    ].join("\n"),
  );
  fixture["filesRows"] = { lands, landed, rows: filesRows(lands, landed, "/root", roots) };
});

it("provision-mcp lines and atHome", () => {
  fixture["mcpLines"] = {
    their: theirServerLine("Claude Code", "gsc", "/root/.claude.json"),
    unreached: keyUnreachedLine("Codex", ["GITHUB_TOKEN", "API_KEY"]),
    untrusted: untrustedLine("Codex", "/root/repo", "the file is a link"),
    noCopy: noCopyLine("/root/.claude.json"),
  };
  const plan: McpPlan = {
    agents: [
      {
        id: "claude",
        label: "Claude Code",
        scopes: [
          { files: ["/root/.claude-cfg/.claude.json", "/rootx/a", "/root"], format: MCP_SERVERS_JSON, keep: ["a"], drop: [], project: { from: "/Users/dev/repo", to: "/root/repo" } },
          { files: ["/elsewhere/c.json"], format: MCP_SERVERS_JSON, keep: [], drop: [{ name: "b", reason: "unticked" }] },
        ],
        aside: [],
      },
      { id: "codex", label: "Codex", scopes: [{ files: ["/root/.codex/config.toml"], format: CODEX_TOML, keep: ["c"], drop: [] }], aside: [] },
    ],
    guestHome: "/root",
    rewrites: [["/Users/dev/", "/root/"], ["/Users/dev/bin", "/root/bin"], ["/opt/x/", "/opt/y/"], ["/Users/dev", "/root"]],
    binDirs: ["/Users/dev/.local/bin/"],
    tools: [],
  };
  const moved = atHome(plan, "/home/dev");
  fixture["atHome"] = {
    same: atHome(plan, "/root") === plan,
    rewrites: moved.rewrites,
    guestHome: moved.guestHome,
    binDirs: moved.binDirs,
    scopes: moved.agents.map(a => a.scopes.map(s => ({ files: s.files, project: s.project ?? null }))),
  };
});

it("provision plan reads", () => {
  const step = (o: Partial<ToolInstall>): ToolInstall => ({ id: "tools/x", label: "x", manager: "apt", cmd: "true", ...o });
  const cases: [string, ToolInstall][] = [
    ["nothing", step({})],
    ["check", step({ check: "x --version" })],
    ["present over check", step({ check: "brew list --versions x", present: "test -e /home/linuxbrew/.linuxbrew/opt/x" })],
    ["bin", step({ bin: "x" })],
    ["check and bin", step({ check: "x --help", bin: "it's" })],
    ["pin read alone", step({ pin: { read: "x --version", fixed: true, words: "w" } })],
    ["pin read with asks", step({ bin: "x", asks: "1.2.3", pin: { read: "x --version", fixed: true, words: "w" } })],
    ["agent pin read with asks", step({ id: "agents/claude", bin: "claude", asks: "2.1.0", pin: { read: "claude --version | cut -d' ' -f1", fixed: true, words: "w" } })],
    ["below", step({ bin: "x", asks: "1.2.3", below: { node: "20.0.0", asks: "1.0.0" }, pin: { read: "x -v", fixed: true, words: "w" } })],
    ["below no asks", step({ bin: "x", asks: "1.2.3", below: { node: "20.0.0" }, pin: { read: "x -v", fixed: true, words: "w" } })],
    ["pin read but check", step({ check: "c", pin: { read: "x -v", fixed: true, words: "w" } })],
    // Added by the spec strengthening (provision_plan.md R-rules): below with no asks of the step's own, the agent
    // form on both sides of below, below without asks on an agent row, agent-ness by id prefix, empty strings.
    ["below, step asks nothing", step({ below: { node: "20.0.0", asks: "1.0.0" }, pin: { read: "x -v", fixed: true, words: "w" } })],
    ["agent below with asks", step({ id: "agents/claude", bin: "claude", asks: "2.1.0", below: { node: "22.1.0", asks: "1.0.0" }, pin: { read: "x -v", fixed: true, words: "w" } })],
    ["agent below no asks", step({ id: "agents/claude", bin: "claude", asks: "2.1.0", below: { node: "22.1.0" }, pin: { read: "x -v", fixed: true, words: "w" } })],
    ["agents/node is an agent row", step({ id: "agents/node", bin: "node", asks: "22.0.0", pin: { read: "x -v", fixed: true, words: "w" } })],
    ["agents/codex/bubblewrap is an agent row", step({ id: "agents/codex/bubblewrap", bin: "bwrap", asks: "1.0.0", pin: { read: "x -v", fixed: true, words: "w" } })],
    ["agents/mcp/claude/x is not", step({ id: "agents/mcp/claude/x", bin: "x", asks: "1.0.0", pin: { read: "x -v", fixed: true, words: "w" } })],
    ["empty check", step({ check: "" })],
    ["empty bin", step({ bin: "" })],
    ["empty asks", step({ bin: "x", asks: "", pin: { read: "x -v", fixed: true, words: "w" } })],
    ["asks with a quote", step({ bin: "x", asks: "1'2", pin: { read: "x -v", fixed: true, words: "w" } })],
    ["asks, pin without read", step({ bin: "x", asks: "1.0.0", pin: { fixed: true, words: "w" } })],
    ["asks, no pin", step({ bin: "x", asks: "1.0.0" })],
    ["pin read with a newline", step({ bin: "x", asks: "1", pin: { read: "a\nb", fixed: true, words: "w" } })],
  ];
  fixture["presenceTests"] = cases.map(([name, s]) => ({ name, step: s, asked: presenceTests(s), any: presenceTests(s, "any") }));
  const steps = [step({ id: "apt-index", label: "apt index" }), step({ id: "tools/a", bin: "a", after: "apt-index" }), step({ id: "tools/b", bin: "b", after: "apt-index" }), step({ id: "lonely", label: "lonely" }), step({ id: "tools/c", after: "lonely2", bin: "c" })];
  fixture["presentByWhatWaits"] = {
    all: [...presentByWhatWaits(steps, new Set(["tools/a", "tools/b"]))],
    one: [...presentByWhatWaits(steps, new Set(["tools/a"]))],
  };
  fixture["presentElsewhere"] = [
    presentElsewhere(step({ bin: "rg", manager: "brew", bins: ["/home/linuxbrew/.linuxbrew/bin"] }), { path: "/usr/bin/rg" }) ?? null,
    presentElsewhere(step({ bin: "rg", manager: "brew", bins: ["/home/linuxbrew/.linuxbrew/bin"] }), { path: "/home/linuxbrew/.linuxbrew/bin/rg" }) ?? null,
    presentElsewhere(step({ bin: "rg", manager: "brew", bins: ["/home/linuxbrew/.linuxbrew/bin"] }), { path: "/home/linuxbrew/.linuxbrew/binx/rg" }) ?? null,
    presentElsewhere(step({ bin: "rg", manager: "brew" }), { path: "/usr/bin/rg" }) ?? null,
    presentElsewhere(step({ manager: "brew", bins: ["/x"] }), { path: "/usr/bin/rg" }) ?? null,
    presentElsewhere(step({ bin: "rg", manager: "brew", bins: ["/x"] }), {}) ?? null,
  ];
  const plan = (steps: ToolInstall[], agents = 0): ProvisionPlan => ({ recipeAt: "x", path: "/bin", prefix: "/opt/wsp", steps, skipped: [], agents, compiler: false });
  const tool = (manager: ToolInstall["manager"], bins?: string[]): ToolInstall => ({ id: `tools/${manager}`, label: manager, manager, cmd: "true", ...(bins === undefined ? {} : { bins }) });
  const managers: ToolInstall["manager"][] = ["apt", "brew", "npm", "pnpm", "bun", "uv", "pipx", "cargo", "go", "release", "vendor", "script"];
  fixture["outsideRoots"] = {
    each: managers.map(m => [m, outsideRoots(plan([tool(m)]), "clis")]),
    mixed: outsideRoots(plan([tool("npm"), tool("vendor"), tool("go", ["/usr/local/bin", "/opt/wsp/go/bin", "/usr/local/go/bin"])]), "clis"),
    agentsOnly: outsideRoots(plan([tool("npm"), tool("release")], 1), "agents"),
    clisAfterAgents: outsideRoots(plan([tool("npm"), tool("release")], 1), "clis"),
  };
  fixture["provisionCountsOf"] = provisionCountsOf({
    ...plan([tool("apt"), tool("npm")]),
    files: { lands: [{ id: "a", label: "a", dest: "a" }, { id: "b", label: "b", dest: "b" }, { id: "c", label: "c", dest: "c" }], pack: async () => { throw new Error("unused"); } },
    mcp: { agents: [{ id: "claude", label: "C", scopes: [{ files: [], format: MCP_SERVERS_JSON, keep: ["a", "b"], drop: [{ name: "c", reason: "r" }] }, { files: [], format: MCP_SERVERS_JSON, keep: [], drop: [] }], aside: [{ id: "x", name: "x", reason: "r" }] }], guestHome: "/root", rewrites: [], binDirs: [], tools: [] },
  });
});

it("vault scripts, tar and refusals", async () => {
  vi.spyOn(Date, "now").mockReturnValue(1_760_000_000_123);
  vi.spyOn(Math, "random").mockReturnValue(0.5);
  fixture["folderExportScript"] = { plain: folderExportScript("/root/repo", RULE, "/tmp/wsp-out-1.tgz"), emptyRule: folderExportScript("/srv/a b", { dirs: [], files: [], markers: [] }, "/tmp/o.tgz") };
  const runsOf = async (paths: string[], opts: Parameters<typeof exportPaths>[2]): Promise<{ runs: string[]; execs: string[]; error?: string }> => {
    const r = recorder();
    try {
      await exportPaths(r.machine, paths, { fetch: fetchOk, ...opts });
      return { runs: r.runs, execs: r.execs };
    } catch (e) {
      return { runs: r.runs, execs: r.execs, error: (e as Error).message };
    }
  };
  fixture["exportPaths"] = {
    none: await runsOf([], {}),
    plain: await runsOf(["/root/.claude", "relative/dir", "/root/it's"], {}),
    leave: await runsOf(["/root/.claude"], { leave: ["/root/.claude/projects", "/root/it's here"] }),
    rule: await runsOf(["/root/repo", "/root/node_modules/", "/root/dist"], { exclude: RULE, leave: ["/root/repo/.env"] }),
    allCaches: await runsOf(["/root/node_modules", "/root/dist/"], { exclude: RULE }),
    capped: await runsOf(["/root/a"], { maxBytes: 11 }),
  };
  const imp = async (tar: Buffer, dest: string, opts: Parameters<typeof importInto>[3]): Promise<{ runs: string[]; execs: string[]; parts?: number; error?: string }> => {
    const r = recorder();
    try {
      const { parts } = await importInto(r.machine, tar, dest, { fetch: fetchOk, ...opts });
      return { runs: r.runs, execs: r.execs, parts };
    } catch (e) {
      return { runs: r.runs, execs: r.execs, error: (e as Error).message };
    }
  };
  const small = Buffer.from("not really a tgz");
  fixture["importInto"] = {
    sha256: (await import("node:crypto")).createHash("sha256").update(small).digest("hex"),
    replace: await imp(small, "/root", {}),
    overlay: await imp(small, "/root/dest dir", { overlay: true, tmpDir: "/root/.wsp/provision/" }),
    leave: await imp(small, "/root", { leave: ["/root/.claude/projects", "/root/it's"] }),
    empty: await imp(Buffer.alloc(0), "/root", {}),
  };
  const land = async (dest: string, replace: boolean): Promise<{ runs: string[]; execs: string[]; error?: string }> => {
    const r = recorder();
    try {
      await landBundle(r.machine, small, dest, { fetch: fetchOk, replace });
      return { runs: r.runs, execs: r.execs };
    } catch (e) {
      return { runs: r.runs, execs: r.execs, error: (e as Error).message };
    }
  };
  fixture["landBundle"] = { fresh: await land("/srv/app/", false), replace: await land("/srv/app", true), relative: await land("srv/app", false), root: await land("/", false), top: await land("/app", false) };
  vi.restoreAllMocks();
  const long = `${"d".repeat(60)}/${"e".repeat(60)}/${"f".repeat(60)}/name.txt`;
  const entries = [
    { path: "/root/.claude/CLAUDE.md", mode: 0o644, content: "his standing rules\n" },
    { path: "root/bin/run", mode: 0o755, content: new Uint8Array([0, 1, 2, 255]) },
    { path: "/root/empty", mode: 0o700, dir: true as const },
    { path: "/root/link", target: "../elsewhere/target" },
    { path: long, mode: 0o600, content: "x".repeat(513) },
    { path: "/root/é.txt", mode: 0o644, content: "" },
  ];
  vi.spyOn(Date, "now").mockReturnValue(1_760_000_000_999);
  const tar = gunzipSync(tarOf(entries));
  vi.restoreAllMocks();
  fixture["tarOf"] = { mtimeSecs: 1_760_000_000, entries: entries.map(e => ({ ...e, ...("content" in e ? { content: Buffer.from(e.content).toString("base64") } : {}) })), tarBase64: tar.toString("base64") };
  const refuse = (path: string, target?: string): string | null => {
    try {
      tarOf([target === undefined ? { path, mode: 0o644, content: "" } : { path, target }]);
      return null;
    } catch (e) {
      return (e as Error).message;
    }
  };
  const p100 = "a".repeat(100);
  const p101 = "a".repeat(101);
  const prefix155 = `${"p".repeat(155)}/${"n".repeat(100)}`;
  const prefix156 = `${"p".repeat(156)}/${"n".repeat(100)}`;
  const twoByte = `${"é".repeat(50)}`;
  const twoByteOver = `${"é".repeat(50)}a`;
  const fits = [p100, p101, prefix155, prefix156, `/${p100}`, `//${p100}`, twoByte, twoByteOver, `x/${p101}`, `${"p".repeat(10)}/${"n".repeat(101)}`];
  fixture["fitsTar"] = { paths: fits.map(p => [p, fitsTar(p)]), targets: [["l", "t".repeat(100), fitsTar("l", "t".repeat(100))], ["l", "t".repeat(101), fitsTar("l", "t".repeat(101))], ["l", "é".repeat(50), fitsTar("l", "é".repeat(50))], ["l", `${"é".repeat(50)}t`, fitsTar("l", `${"é".repeat(50)}t`)]] };
  fixture["tarRefusals"] = { tooLong: refuse(p101.repeat(3)), linkTooLong: refuse("l", "t".repeat(101)) };
});

it("writes the fixture", () => {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "provision.json"), `${JSON.stringify(fixture, null, 1)}\n`);
});
