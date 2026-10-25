// SPDX-License-Identifier: AGPL-3.0-only
// The TS golden seam's own answers (golden-base.ts, golden-tools.ts, golden-import.ts, golden-mcp.ts at a4033dc34),
// written as tests/fixtures/oracle/golden_<unit>.json for the Rust hard tests (spec/golden-<unit>.md names which).
// Every script text here is a contract: the Rust port prints it byte for byte. A machine here is scripted by rules
// that tests/common/golden.rs reads the same way. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/golden.vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

vi.mock("node:crypto", async orig => ({ ...(await orig<typeof import("node:crypto")>()), randomBytes: (n: number) => Buffer.alloc(n, 0xab) }));

import { BASE_FLOOR, CATALOG_AGENTS, CODEX_TOML, MCP_SERVERS_JSON, OPENCODE_JSON, ROADS, ROAD_STEPS, catalogEntry, type InstallRoad, type McpFormat } from "../../../../packages/catalog/src/index.js";
import { MCP_ID_PREFIX, TOOL_PREFIX } from "../../../../packages/protocol/src/index.js";
import { BASE_VERSION_LINES, PROFILE_PATH_LINE, baseInstalls, baseVersionsCmd, carriedByImage, installBase, parseVersions, profilePathLine, versionsLine } from "../../../../packages/engine/src/golden-base.js";
import {
  AGENT_INSTALLERS,
  EXEC_ENV,
  PATH_LINE,
  TOOLS_PATH,
  agentInstallsFor,
  agentSteps,
  aptIndexStep,
  asksVersion,
  baseRowFor,
  brewHousekeeping,
  brewTapCheck,
  brewfileFor,
  catalogToolOf,
  customInstallsFor,
  customPrelude,
  managerFormula,
  nodeFloorCheck,
  nodeMajorFor,
  pathLine,
  pinReadOf,
  roadReads,
  roadStepReads,
  rowRoad,
  shownOf,
  toolInstallsFor,
  viaRoad,
  type BrewTable,
  type RecipeEntry,
  type ToolInstall,
} from "../../../../packages/engine/src/golden-import.js";
import {
  ALREADY_ON_MACHINE,
  FREE_KB_CMD,
  GUARD_SLACK_S,
  TOOLS_DISK_FLOOR,
  USED_KB_CMD,
  closing,
  dfKbCmd,
  diskUse,
  freeBytes,
  freeNote,
  guardDeadlineMs,
  guarded,
  guardedRoad,
  installTools,
  missingCommands,
  ownedFloorBytes,
  prefixVolume,
  reasonOf,
  roadLimitS,
  roadOf,
  sweepCaches,
  type ToolResult,
} from "../../../../packages/engine/src/golden-tools.js";
import {
  FETCHERS,
  MCP_READ_END,
  MCP_READ_MARK,
  READ_MS,
  absentCommands,
  absentReason,
  applyMcp,
  asRun,
  commandRow,
  landConfigs,
  mcpOpening,
  mcpPlanFor,
  mcpRowId,
  mcpRows,
  mcpTally,
  parseConfigs,
  parseMcpId,
  readConfigsCmd,
  readFailed,
  readReason,
  readWhole,
  refused,
  rewriteString,
  scopeRowId,
  serverNeeds,
  withoutAbsent,
  type McpAgentPlan,
  type McpPlan,
  type ScopeOutcome,
} from "../../../../packages/engine/src/golden-mcp.js";
import type { ExecResult, Machine } from "../../../../packages/engine/src/machine.js";

const OUT = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const write = (name: string, value: unknown): void => {
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, `golden_${name}.json`), `${JSON.stringify(value, (_k, v) => (v instanceof Set ? [...v] : v instanceof Map ? [...v] : v), 2)}\n`);
};

// --- the scripted machine (tests/common/golden.rs reads these rules the same way) ------------------------------------

/** One answer: an exit and its output, the lines a run streams first, or a rejection with these words. */
interface Answer {
  exit?: number;
  stdout?: string;
  stderr?: string;
  lines?: string[];
  throws?: string;
}
/** The first rule whose `has` is in the call's text (and whose `kind` is the call's, when it names one) answers with
 * its next answer, the last one again once they run out; a call no rule matches answers exit 0 with nothing. */
interface Rule {
  has: string;
  kind?: "exec" | "run" | "put";
  seq: Answer[];
}
interface Call {
  kind: "exec" | "run" | "put";
  text: string;
  timeoutMs?: number;
  deadlineMs?: number;
  unlogged?: boolean;
}

function scripted(rules: Rule[], opts: { put?: boolean } = {}): { machine: Machine; calls: Call[] } {
  const calls: Call[] = [];
  const used = rules.map(() => 0);
  const answer = async (kind: Call["kind"], text: string, onLine?: (line: string) => void): Promise<ExecResult> => {
    const at = rules.findIndex(r => text.includes(r.has) && (r.kind === undefined || r.kind === kind));
    if (at === -1) return { exitCode: 0, stdout: "", stderr: "" };
    const r = rules[at]!;
    const a = r.seq[Math.min(used[at]!, r.seq.length - 1)]!;
    used[at]!++;
    if (a.throws !== undefined) throw new Error(a.throws);
    for (const line of a.lines ?? []) onLine?.(line);
    return { exitCode: a.exit ?? 0, stdout: a.stdout ?? "", stderr: a.stderr ?? "" };
  };
  const machine = {
    id: "m1",
    kind: "sandbox",
    exec: async (cmd: string, o: { timeoutMs?: number } = {}) => {
      calls.push({ kind: "exec", text: cmd, ...(o.timeoutMs !== undefined ? { timeoutMs: o.timeoutMs } : {}) });
      return answer("exec", cmd);
    },
    run: async (script: string, o: { deadlineMs?: number; unlogged?: boolean; onLine?: (l: string) => void } = {}) => {
      calls.push({ kind: "run", text: script, ...(o.deadlineMs !== undefined ? { deadlineMs: o.deadlineMs } : {}), ...(o.unlogged === true ? { unlogged: true } : {}) });
      return answer("run", script, o.onLine);
    },
    ...(opts.put === true
      ? {
          putBytes: async (path: string, bytes: Uint8Array, o: { timeoutMs?: number } = {}) => {
            calls.push({ kind: "put", text: `${path}\n${Buffer.from(bytes).toString("utf8")}`, ...(o.timeoutMs !== undefined ? { timeoutMs: o.timeoutMs } : {}) });
            await answer("put", path);
            return undefined;
          },
        }
      : {}),
    uploadUrl: async () => "https://signed.example/ul",
  } as unknown as Machine;
  return { machine, calls };
}

const caught = async <T>(f: () => Promise<T>): Promise<{ ok: T } | { error: string }> => {
  try {
    return { ok: await f() };
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
};

/** A result with its clock reading as whether it had one, since `ms` is the loop's own `Date.now()` difference. */
const timeless = (r: ToolResult): Record<string, unknown> => {
  const { ms, ...rest } = r;
  return { ...rest, ...(ms !== undefined ? { ms: true } : {}) };
};

const res = (exitCode: number, stdout = "", stderr = ""): ExecResult => ({ exitCode, stdout, stderr });

// --- tools_disk -------------------------------------------------------------------------------------------------

it("tools_disk", async () => {
  const roads = [...ROADS];
  const dfScenarios: { name: string; rules: Rule[]; prefix?: string }[] = [
    { name: "a pair", rules: [{ has: "df -Pk", seq: [{ stdout: "1200 50000\n" }] }] },
    { name: "a pair on a prefix", rules: [{ has: "df -Pk", seq: [{ stdout: "1200 50000\n" }] }], prefix: "/opt/wsp" },
    { name: "a full disk reads used size", rules: [{ has: "df -Pk", seq: [{ stdout: "0 50000\n" }] }] },
    { name: "a size of zero", rules: [{ has: "df -Pk", seq: [{ stdout: "10 0\n" }] }] },
    { name: "one number", rules: [{ has: "df -Pk", seq: [{ stdout: "1200\n" }] }] },
    { name: "three numbers", rules: [{ has: "df -Pk", seq: [{ stdout: "1 2 3\n" }] }] },
    { name: "a negative", rules: [{ has: "df -Pk", seq: [{ stdout: "-1 50000\n" }] }] },
    { name: "junk", rules: [{ has: "df -Pk", seq: [{ stdout: "Filesystem 1K-blocks\n" }] }] },
    { name: "a long junk line", rules: [{ has: "df -Pk", seq: [{ stdout: `${"x".repeat(300)}\n` }] }] },
    { name: "hex and exponent read as Number does", rules: [{ has: "df -Pk", seq: [{ stdout: "0x10 1e3\n" }] }] },
    { name: "tabs and spaces", rules: [{ has: "df -Pk", seq: [{ stdout: "  12\t\t34  \n" }] }] },
    { name: "exit 0 printing nothing", rules: [{ has: "df -Pk", seq: [{ stdout: "" }] }] },
    { name: "exit 1 with stderr", rules: [{ has: "df -Pk", seq: [{ exit: 1, stderr: "df: /root: No such file or directory\n" }] }] },
    { name: "exit 124", rules: [{ has: "df -Pk", seq: [{ exit: 124 }] }] },
    { name: "exit 2 printing a pair", rules: [{ has: "df -Pk", seq: [{ exit: 2, stdout: "1 2\n" }] }] },
    { name: "the exec rejects", rules: [{ has: "df -Pk", seq: [{ throws: "the machine went away" }] }] },
  ];
  const diskUses = [];
  for (const s of dfScenarios) {
    const { machine, calls } = scripted(s.rules);
    diskUses.push({ ...s, out: await caught(() => diskUse(machine, s.prefix)), calls });
  }
  const frees = [];
  for (const s of dfScenarios.map(d => ({ ...d, rules: d.rules.map(r => ({ ...r, seq: r.seq.map(a => (a.stdout === "1200 50000\n" ? { ...a, stdout: "1200\n" } : a)) })) }))) {
    const { machine, calls } = scripted(s.rules);
    frees.push({ ...s, out: await caught(() => freeBytes(machine, s.prefix)), note: await caught(() => freeNote(scripted(s.rules).machine, s.prefix)), calls });
  }
  const reasons = [
    res(124, "out", "err"),
    res(1, "", "==> Downloading\nError: no bottle for gh\n  brew install --build-from-source gh\n"),
    res(1, "", "Error: first\nError: second\nadvice\n"),
    res(1, "", "warning one\nthe last stderr line\n\n"),
    res(1, "stdout one\nstdout last\n", ""),
    res(1, "", ""),
    res(7, "  \n", "  \n"),
    res(1, "", `${"é".repeat(200)}\n`),
    res(1, "", `Error: ${"y".repeat(400)}`),
    res(-1, "", "refused by the provider"),
    res(1, "", "  Error: indented error  \nplain after\n"),
    res(1, "", "line\r\nlast\r\n"),
  ];
  const roadLines = [
    "",
    "WSP_ROAD release https://github.com/a/b/releases/download/v1/b.tgz\n",
    `WSP_ROAD release https://x/y.tgz ${"a".repeat(64)} v1.2.3 x86_64\n`,
    `WSP_ROAD go github.com/a/b@v1 ${"b".repeat(64)} v1\n`,
    `noise\nWSP_ROAD go github.com/a/b@v1\nmore\nWSP_ROAD release https://x/z.tgz ${"c".repeat(64)} v2 aarch64 trailing\n`,
    `WSP_ROAD release https://x/y.tgz ${"A".repeat(64)} v1\n`,
    `WSP_ROAD release https://x/y.tgz ${"a".repeat(63)} v1\n`,
    "  WSP_ROAD release https://x/y.tgz\n",
    "WSP_ROAD vendor https://x/y.tgz\n",
    `WSP_ROAD release https://x/y.tgz ${"d".repeat(64)} v1 arm64\n`,
    `WSP_ROAD release https://x/y.tgz ${"a".repeat(65)} v1\n`,
    `WSP_ROAD go m ${"a".repeat(64)} v1 x86_64x\n`,
    "x\rWSP_ROAD go m\n",
    "WSP_ROAD go\n",
  ];
  const missing = [];
  for (const s of [
    { bins: ["codex", "gh"], rules: [{ has: "command -v", seq: [{ stdout: "missing gh\n" }] }] as Rule[] },
    { bins: ["it's", "a b"], rules: [{ has: "command -v", seq: [{ stdout: "missing it's\nmissing  a b \nnoise\n" }] }] as Rule[], prefix: TOOL_PREFIX, path: "/x/bin:/usr/bin" },
    { bins: ["codex"], rules: [{ has: "command -v", seq: [{ exit: 1, stdout: "missing codex\n", stderr: "bash: boom\n" }] }] as Rule[] },
  ]) {
    const { machine, calls } = scripted(s.rules);
    missing.push({ ...s, out: await missingCommands(machine, s.bins, s.path, s.prefix), calls });
  }
  const sweeps = [];
  for (const s of [
    { name: "frees bytes", rules: [{ has: "NR==2{print $4}", seq: [{ stdout: "1000\n" }, { stdout: "3000\n" }] }] as Rule[] },
    { name: "frees nothing", rules: [{ has: "NR==2{print $4}", seq: [{ stdout: "1000\n" }] }] as Rule[] },
    { name: "fails", rules: [{ has: "go clean", seq: [{ exit: 1, stderr: "rm: cannot remove\n" }] }] as Rule[] },
    { name: "df unknown after", rules: [{ has: "NR==2{print $4}", seq: [{ stdout: "1000\n" }, { exit: 1 }] }] as Rule[], path: "/custom/bin" },
  ]) {
    const { machine, calls } = scripted(s.rules);
    sweeps.push({ ...s, out: await sweepCaches(machine, s.path), calls });
  }
  write("tools_disk", {
    FREE_KB_CMD,
    USED_KB_CMD,
    TOOLS_DISK_FLOOR,
    GUARD_SLACK_S,
    ALREADY_ON_MACHINE,
    dfKbCmd: [
      { columns: ["free"], out: dfKbCmd(["free"]) },
      { columns: ["used", "size"], out: dfKbCmd(["used", "size"]) },
      { columns: ["size", "used", "free"], path: "/data", out: dfKbCmd(["size", "used", "free"], "/data") },
      { columns: [], out: dfKbCmd([]) },
      { columns: ["free"], path: prefixVolume(TOOL_PREFIX), out: dfKbCmd(["free"], prefixVolume(TOOL_PREFIX)) },
    ],
    prefixVolume: ["/opt/wsp", "/srv/it's here", "relative/dir", ""].map(p => ({ prefix: p, out: prefixVolume(p) })),
    guarded: [
      { script: "echo hi", timeoutS: 30, out: guarded("echo hi", 30) },
      { script: "set -e\nprintf '%s' \"it's\"", timeoutS: 0, out: guarded("set -e\nprintf '%s' \"it's\"", 0) },
    ],
    roads: roads.map(r => ({ road: r, limitS: roadLimitS(r), retry: ROAD_STEPS[r].retry, deadlineMs: guardDeadlineMs(roadLimitS(r)), guarded: guardedRoad(r, "install it") })),
    guardDeadlineMs: [0, 1, 300, 1800].map(s => ({ s, out: guardDeadlineMs(s) })),
    reasonOf: reasons.flatMap(r => [30, 1.5].map(t => ({ res: r, timeoutS: t, out: reasonOf(r, t) }))),
    roadOf: roadLines.map(s => ({ stdout: s, out: roadOf(s) ?? null })),
    closing: [[], ["a"], ["a", undefined, "", "b"], [undefined], ["", ""], ["x; y", "z"]].map(parts => ({ parts: parts.map(p => p ?? null), out: closing(...parts) })),
    ownedFloorBytes: [undefined, 0, 1, 10 * 1024 ** 3, 20 * 1024 ** 3, 21474836480 + 9, 30 * 1024 ** 3, 512 * 1024 ** 3 + 7].map(n => ({ size: n ?? null, out: ownedFloorBytes(n) })),
    diskUse: diskUses,
    freeBytes: frees,
    missingCommands: missing,
    sweepCaches: sweeps,
  });
});

// --- tools_loop -------------------------------------------------------------------------------------------------

const FREE_HAS = "NR==2{print $4}";
const kb = (bytes: number): string => `${Math.floor(bytes / 1024)}\n`;
const GiB = 1024 ** 3;
const MiB = 1024 ** 2;

interface LoopScenario {
  name: string;
  tools: ToolInstall[];
  rules: Rule[];
  at?: "installing-tools" | "deploying-daemon";
  opts?: { present?: string[]; caches?: "sweep" | "keep"; floor?: number; path?: string; prefix?: string };
}

const step = (id: string, over: Partial<ToolInstall> = {}): ToolInstall => ({ id, label: id.slice(id.lastIndexOf("/") + 1), manager: "script", cmd: `install ${id}`, ...over });

const LOOPS: LoopScenario[] = [
  {
    name: "two steps, the second after the first, each checked after the loop",
    tools: [step("agents/node", { label: "Node 22.23.2", cmd: "install node", check: "node --version" }), step("agents/codex", { label: "Codex", manager: "npm", cmd: "install codex", after: "agents/node", bin: "codex" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }, { stdout: kb(8 * GiB) }, { stdout: kb(7 * GiB) }] }],
  },
  {
    name: "a failure is named by its Error line and skips what waits on it; the next one still runs",
    tools: [step("tools/brew/gh", { label: "gh", manager: "brew", cmd: "brew install gh" }), step("tools/brew/gh-dash", { label: "gh-dash", manager: "brew", after: "tools/brew/gh" }), step("tools/npm/turbo", { label: "turbo", manager: "npm" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "brew install gh", kind: "run", seq: [{ exit: 1, stderr: "==> Downloading\nError: no bottle for gh\n  advice\n", lines: ["==> Downloading", "Error: no bottle for gh"] }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a step already on the machine runs nothing and lets what waits on it run",
    tools: [step("agents/node", { label: "Node", check: "node --version" }), step("agents/codex", { label: "Codex", manager: "npm", after: "agents/node", bin: "codex" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }],
    opts: { present: ["agents/node"], caches: "keep" },
  },
  {
    name: "under the floor with no Homebrew the sweep alone runs and the install goes on when it clears",
    tools: [step("tools/x", { label: "x" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(1 * GiB) }, { stdout: kb(1 * GiB) }, { stdout: kb(3 * GiB) }, { stdout: kb(3 * GiB) }, { stdout: kb(2.5 * GiB) }] }],
  },
  {
    name: "under the floor and still under after the cleanup: every tool left is skipped with the reading",
    tools: [step("tools/a", { label: "a" }), step("tools/b", { label: "b" }), step("tools/c", { label: "c" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(1 * GiB) }] }],
  },
  {
    name: "df unknown after the cleanup",
    tools: [step("tools/a", { label: "a" }), step("tools/b", { label: "b" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(1 * GiB) }, { exit: 1, stderr: "df: gone\n" }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a full disk reads zero, not unknown",
    tools: [step("tools/a", { label: "a" }), step("tools/b", { label: "b" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: "0\n" }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a df that fails warns once and installs without the floor",
    tools: [step("tools/a", { label: "a" }), step("tools/b", { label: "b" })],
    rules: [{ has: FREE_HAS, seq: [{ exit: 1, stderr: "df: /root: No such file or directory\n" }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a df that exits 0 printing something it cannot use names what it printed",
    tools: [step("tools/a", { label: "a" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: "garbage here\n" }] }],
    opts: { caches: "keep" },
  },
  {
    name: "an install too big for the room above the floor is skipped alone, and what fits after it installs",
    tools: [step("tools/llvm", { label: "llvm", bytes: 5 * GiB }), step("tools/jq2", { label: "jq2", bytes: 10 * MiB })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(3 * GiB) }, { stdout: kb(3 * GiB) }, { stdout: kb(2.9 * GiB) }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a too-big install still too big after the cleanup stays skipped",
    tools: [step("tools/llvm", { label: "llvm", bytes: 2 * GiB })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(3 * GiB) }, { stdout: kb(5 * GiB) }, { stdout: kb(3 * GiB) }, { stdout: kb(3 * GiB) }] }],
  },
  {
    name: "a cellar lock waits for every Homebrew lock, then tries once more",
    tools: [step("tools/brew/gh", { label: "gh", manager: "brew", cmd: "brew install gh" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "brew install gh", kind: "run", seq: [{ exit: 1, stderr: "Error: Another active Homebrew process has already locked /home/linuxbrew/.linuxbrew/var/homebrew/locks/gh.formula.lock\n" }, { exit: 0 }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a download road that hits its limit is tried once more, and the second run is an install",
    tools: [step("tools/release/gh", { label: "gh", manager: "release", cmd: "fetch gh" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "fetch gh", kind: "run", seq: [{ exit: 124 }, { exit: 0 }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a download road that hits its limit twice is failed with both timeouts; a compile road is not tried again",
    tools: [step("tools/release/gh", { label: "gh", manager: "release", cmd: "fetch gh" }), step("tools/cargo/tokei", { label: "tokei", manager: "cargo", cmd: "cargo install tokei" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "fetch gh", kind: "run", seq: [{ exit: 124 }] }, { has: "cargo install tokei", kind: "run", seq: [{ exit: 124 }] }],
    opts: { caches: "keep" },
  },
  {
    name: "Homebrew installs: the result carries its checkout and the housekeeping after the loop says what it freed",
    tools: [step("tools/homebrew", { label: "Homebrew", manager: "brew", cmd: "bootstrap brew", bin: "brew" }), step("tools/brew/gh", { label: "gh", manager: "brew", cmd: "brew install gh", after: "tools/homebrew", bin: "gh" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }, { stdout: kb(8 * GiB) }, { stdout: kb(7 * GiB) }, { stdout: kb(7 * GiB) }, { stdout: kb(8 * GiB) }, { stdout: kb(8 * GiB) }, { stdout: kb(9 * GiB) }] }],
  },
  {
    name: "Homebrew cleanup that fails at the floor is named in the rescue's line",
    tools: [step("tools/homebrew", { label: "Homebrew", manager: "brew", cmd: "bootstrap brew" }), step("tools/brew/gh", { label: "gh", manager: "brew", cmd: "brew install gh", after: "tools/homebrew" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }, { stdout: kb(1 * GiB) }] }, { has: "autoremove", kind: "run", seq: [{ exit: 1, stderr: "Error: autoremove broke\n" }] }],
    opts: { caches: "keep" },
  },
  {
    name: "an install that exits 0 without its command on PATH is failed by name",
    tools: [step("agents/codex", { label: "Codex", manager: "npm", bin: "codex" }), step("tools/release/gh", { label: "gh", manager: "release", bin: "gh", cmd: "fetch gh" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "command -v \"$b\"", seq: [{ stdout: "missing gh\n" }] }, { has: "fetch gh", kind: "run", seq: [{ stdout: `WSP_ROAD release https://x/gh.tgz ${"e".repeat(64)} v2.1.0 x86_64\n` }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a PATH check that itself fails counts every named tool as failed",
    tools: [step("agents/codex", { label: "Codex", manager: "npm", bin: "codex" }), step("tools/x", { label: "x" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "command -v \"$b\"", seq: [{ exit: 1, stderr: "bash: boom\n" }] }],
    opts: { caches: "keep" },
  },
  {
    name: "checks read a page at a time: a page that printed failures fails those rows, a page that could not run fails its own",
    tools: Array.from({ length: 11 }, (_, i) => step(`tools/c${i}`, { label: `c${i}`, check: `check-${i} --version` })),
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "wsp-check", seq: [{ stdout: "wsp-check 1 c1: command not found\nwsp-check 3 \nnoise\n" }, { exit: 1, stderr: "timed out\n" }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a road install names the road in its result and the summary; a note and a label with a comma",
    tools: [step("tools/release/gh", { label: "gh", manager: "release", cmd: "fetch gh", note: "by an unmeasured road" }), step("tools/x", { label: "x, y", note: "a; b" }), step("tools/go/sqlc", { label: "sqlc", manager: "go", cmd: "go install sqlc" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "fetch gh", kind: "run", seq: [{ stdout: `WSP_ROAD release https://x/gh.tgz ${"f".repeat(64)} v2 aarch64\n` }] }, { has: "go install sqlc", kind: "run", seq: [{ stdout: "WSP_ROAD go github.com/sqlc-dev/sqlc/cmd/sqlc@v1\n" }] }],
    opts: { caches: "keep" },
  },
  {
    name: "a prefix reads the volume wsp installs onto and runs on the probe PATH, with a floor the caller holds",
    tools: [step("tools/uv/ruff", { label: "ruff", manager: "uv", bin: "ruff", check: "ruff --version" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(6 * GiB) }, { stdout: kb(5.5 * GiB) }] }],
    opts: { caches: "keep", floor: 5 * GiB, prefix: TOOL_PREFIX, path: "/opt/wsp/bin:/usr/bin:/bin" },
  },
  {
    name: "the base stage's own name on every line",
    tools: [step("base/login-path", { label: "login shell PATH", shown: "the tools PATH in /etc/profile.d/wsp-golden.sh" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }],
    at: "deploying-daemon",
  },
  {
    name: "a free read whose exec rejects throws out of the loop before anything installs",
    tools: [step("tools/a", { label: "a" })],
    rules: [{ has: FREE_HAS, seq: [{ throws: "the machine went away" }] }],
  },
  {
    name: "an install run that rejects throws out of the loop",
    tools: [step("tools/a", { label: "a", cmd: "install a" }), step("tools/b", { label: "b" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "install a", kind: "run", seq: [{ throws: "lost the machine" }] }],
  },
  {
    name: "skipped rows share a reason line",
    tools: [step("tools/a", { label: "a", cmd: "install a" }), step("tools/b", { label: "b", after: "tools/a" }), step("tools/c", { label: "c", after: "tools/a" }), step("tools/d", { label: "d", after: "tools/nowhere" })],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "install a", kind: "run", seq: [{ exit: 2 }] }],
    opts: { caches: "keep" },
  },
  {
    name: "no tools at all",
    tools: [],
    rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }],
  },
];

it("tools_loop", async () => {
  const out = [];
  for (const s of LOOPS) {
    const { machine, calls } = scripted(s.rules);
    const stages: [string, string | null][] = [];
    const seen: Record<string, unknown>[] = [];
    const o = s.opts ?? {};
    const r = await caught(() =>
      installTools(machine, s.tools, (stage, detail) => void stages.push([stage, detail ?? null]), s.at ?? "installing-tools", {
        ...(o.present !== undefined ? { present: new Set(o.present) } : {}),
        ...(o.caches !== undefined ? { caches: o.caches } : {}),
        ...(o.floor !== undefined ? { floor: o.floor } : {}),
        ...(o.path !== undefined ? { path: o.path } : {}),
        ...(o.prefix !== undefined ? { prefix: o.prefix } : {}),
        onTool: result => void seen.push(timeless({ ...result })),
      }),
    );
    out.push({ ...s, out: "ok" in r ? { tools: r.ok.tools.map(timeless), homebrew: r.ok.homebrew ?? null } : r, onTool: seen, stages, calls });
  }
  write("tools_loop", out);
});

// --- base -------------------------------------------------------------------------------------------------------

const VERSIONS_OUT = [
  "VERSION uv: uv 0.12.9",
  "VERSION python3: Python 3.12.13",
  "VERSION git: git version 2.43.0",
  "VERSION jq: jq-1.7.1",
  "VERSION rg: ripgrep 14.1.0 (rev 4649aa9700)",
  "VERSION curl: curl 8.5.0 (x86_64-pc-linux-gnu) libcurl/8.5.0",
  "VERSION cc: cc (Debian 12.2.0-14) 12.2.0",
  "",
].join("\n");
const ALL_VERSIONS = [
  "VERSION curl: curl 8.5.0",
  "VERSION uv: uv 0.12.9",
  "VERSION python3: Python 3.12.13",
  "VERSION git: git version 2.43.0",
  "VERSION jq: jq-1.7.1",
  "VERSION rg: ripgrep 14.1.0",
  "VERSION cc: cc (Debian 12.2.0-14) 12.2.0",
  "VERSION cmake: cmake version 3.28.3",
  "VERSION ninja: 1.11.1",
  "VERSION fd: fd 9.0.0",
  "VERSION sqlite3: 3.45.1 2024-01-30",
  "VERSION wget: GNU Wget 1.21.4",
  "VERSION zip: This is Zip 3.0 (July 5th 2008)",
  "VERSION unzip: UnZip 6.00 of 20 April 2009",
  "VERSION xz: xz (XZ Utils) 5.4.5",
  "VERSION rsync: rsync  version 3.2.7  protocol version 31",
  "VERSION sshd: OpenSSH_9.6p1 Ubuntu-3ubuntu13.5, OpenSSL 3.0.13 30 Jan 2024",
  "",
].join("\n");

it("base", async () => {
  const sets: { name: string; carried: string[]; path?: string; prefix?: string; left: string[] }[] = [
    { name: "a bare image", carried: [], left: [] },
    { name: "the image carries git, jq and the ssh server", carried: ["git", "jq", "ssh-server"], left: [] },
    { name: "the image carries every apt row", carried: BASE_FLOOR.map(e => e.id), left: [] },
    { name: "a computer somebody owns leaves the C toolchain off", carried: [], left: ["build-essential"], path: "/opt/wsp/bin:/usr/bin:/bin", prefix: TOOL_PREFIX },
    { name: "a row left and carried", carried: ["build-essential"], left: ["build-essential"] },
  ];
  const parse = [
    VERSIONS_OUT,
    ALL_VERSIONS,
    "",
    "VERSION uv: \nVERSION git:\nVERSION jq: jq-1\nVERSION x: 1.2.3.4.5\nVERSION a:b: 9.8\nVERSION zip: zip error\nThis is Zip 3.0\n",
    "VERSION node: v22.11.0   \r\n  VERSION indented: 1.2\nversion lower: 1.2\nVERSION spaced name: 10.20.30\nVERSION : 1.0\n",
    "VERSION tab:\t1.2\nVERSION two:  3.4\nVERSION uni: ⁱ1.2 ٣.٤\n",
  ];
  const carried = [parse[0]!, parse[1]!, "VERSION python3: Python 3.11.9\nVERSION uv: uv 0.12.9\n", "VERSION sshd: OpenSSH_9.6p1\n", "VERSION zip: 3.0\n"].map(p => ({ versions: parseVersions(p), out: [...carriedByImage(parseVersions(p))] }));
  const installs = [];
  const scenarios: { name: string; rules: Rule[]; path?: string; prefix?: string; left?: string[]; caches?: "keep" | "sweep" }[] = [
    {
      name: "a bare image: every row runs, the versions are read once after, sized by df",
      rules: [{ has: "VERSION curl", seq: [{ stdout: "" }, { stdout: ALL_VERSIONS }] }, { has: FREE_HAS, seq: [{ stdout: kb(10 * GiB) }, { stdout: kb(9.9 * GiB) }, { stdout: kb(9.8 * GiB) }, { stdout: kb(9.5 * GiB) }, { stdout: kb(9.4 * GiB) }] }],
      caches: "keep",
    },
    {
      name: "the image carries most rows: they read as on the machine and install nothing",
      rules: [{ has: "VERSION curl", seq: [{ stdout: VERSIONS_OUT }, { stdout: ALL_VERSIONS }] }, { has: FREE_HAS, seq: [{ stdout: kb(10 * GiB) }] }],
    },
    {
      name: "a floor step fails: named by its last line, and what waited on it is skipped",
      rules: [{ has: "VERSION curl", seq: [{ stdout: "" }, { stdout: VERSIONS_OUT }] }, { has: FREE_HAS, seq: [{ stdout: kb(10 * GiB) }] }, { has: "apt-get update", kind: "run", seq: [{ exit: 100, stderr: "E: Could not get lock /var/lib/dpkg/lock-frontend\n" }] }],
      caches: "keep",
    },
    {
      name: "a computer somebody owns: the probe PATH, the prefix, the C toolchain left off",
      rules: [{ has: "VERSION curl", seq: [{ stdout: "VERSION sshd: OpenSSH_9.6p1\n" }, { stdout: ALL_VERSIONS }] }, { has: FREE_HAS, seq: [{ stdout: kb(10 * GiB) }] }],
      path: "/opt/wsp/bin:/usr/bin:/bin",
      prefix: TOOL_PREFIX,
      left: ["build-essential"],
      caches: "keep",
    },
    {
      name: "a row left off that the image carries reads as on the machine, and what waited on a carried row waits on nothing",
      rules: [{ has: "VERSION curl", seq: [{ stdout: "VERSION cc: cc 12.2.0\nVERSION uv: uv 0.12.9\nVERSION cmake: 3.28.3\nVERSION ninja: 1.11.1\n" }, { stdout: ALL_VERSIONS }] }, { has: FREE_HAS, seq: [{ stdout: kb(10 * GiB) }] }],
      left: ["build-essential"],
      caches: "keep",
    },
    {
      name: "an install that exits 0 without its command on PATH is a failure, not a version",
      rules: [{ has: "VERSION curl", seq: [{ stdout: ALL_VERSIONS.replace("VERSION jq: jq-1.7.1\n", "") }] }, { has: FREE_HAS, seq: [{ stdout: kb(10 * GiB) }] }, { has: "command -v \"$b\"", seq: [{ stdout: "missing jq\n" }] }],
      caches: "keep",
    },
  ];
  for (const s of scenarios) {
    const { machine, calls } = scripted(s.rules);
    const stages: [string, string | null][] = [];
    const r = await caught(() =>
      installBase(machine, (st, d) => void stages.push([st, d ?? null]), {
        ...(s.caches !== undefined ? { caches: s.caches } : {}),
        ...(s.path !== undefined ? { path: s.path } : {}),
        ...(s.prefix !== undefined ? { prefix: s.prefix } : {}),
        ...(s.left !== undefined ? { left: new Set(s.left) } : {}),
      }),
    );
    installs.push({ ...s, out: "ok" in r ? { tools: r.ok.tools.map(timeless), versions: r.ok.versions, line: r.ok.line } : r, stages, calls });
  }
  const results = (rows: Partial<ToolResult>[]): ToolResult[] => rows.map(r => ({ id: "x", label: "x", outcome: "installed", ...r }) as ToolResult);
  write("base", {
    BASE_VERSION_LINES,
    PROFILE_PATH_LINE,
    profilePathLine: ["/x:/y", "/it's/bin"].map(p => ({ path: p, out: profilePathLine(p) })),
    baseVersionsCmd: [TOOLS_PATH, "/opt/wsp/bin:/usr/bin"].map(p => ({ path: p, out: baseVersionsCmd(p) })),
    baseInstalls: sets.map(s => ({ ...s, out: baseInstalls(new Set(s.carried), s.path, s.prefix, new Set(s.left)) })),
    parseVersions: parse.map(p => ({ stdout: p, out: parseVersions(p) })),
    carriedByImage: carried,
    versionsLine: [
      { versions: parseVersions(ALL_VERSIONS), results: results([{ id: "base/git", label: "git", bytes: 5 * MiB }, { id: "base/jq", label: "jq", bytes: 0 }, { id: "base/curl", label: "curl" }]) },
      { versions: parseVersions(VERSIONS_OUT), results: results([{ id: "base/git", label: "git", note: ALREADY_ON_MACHINE }, { id: "base/apt-index", label: "apt index", outcome: "failed", note: "E: lock" }, { id: "base/fd", label: "fd", outcome: "failed", note: "boom" }, { id: "base/zip", label: "zip", outcome: "skipped", note: "apt index did not install" }]) },
      { versions: [], results: [] },
      { versions: [], results: results([{ id: "base/sshd", label: "OpenSSH server", outcome: "failed" }]) },
      { versions: parseVersions("VERSION unzip: 6.00\nVERSION sshd: OpenSSH_9.6p1\n"), results: results([{ id: "base/zip", label: "zip", bytes: 2 * MiB }, { id: "base/ssh-server", label: "OpenSSH server", bytes: 9 * MiB }]) },
    ].map(v => ({ ...v, out: versionsLine(v.versions, v.results) })),
    installBase: installs,
  });
});

// --- import_roads -----------------------------------------------------------------------------------------------

const row = (over: Partial<RecipeEntry> & Pick<RecipeEntry, "rung" | "id">): RecipeEntry => ({ label: over.id, paths: [], bytes: 0, default: "bring", bring: true, ...over });
/** provision.test.ts:121's `row(id, rung)`: the id as the label, the rung the id's first segment. */
const prow = (id: string): RecipeEntry => ({ rung: id.slice(0, id.indexOf("/")), id, label: id, paths: [], bytes: 0, default: "bring", bring: true });
const tool = (id: string, over: Partial<RecipeEntry> = {}): RecipeEntry => row({ rung: "tools", id, label: id.slice(id.lastIndexOf("/") + 1), ...over });

const ROADS_SEEN: { road: InstallRoad; bin: string }[] = [
  { road: { road: "brew", formula: "gh" }, bin: "gh" },
  { road: { road: "brew", formula: "zingzy/tap/diskbloom" }, bin: "diskbloom" },
  { road: { road: "npm", package: "@openai/codex", version: "0.47.0" }, bin: "codex" },
  { road: { road: "npm", package: "turbo" }, bin: "turbo" },
  { road: { road: "pnpm", package: "turbo", version: "2.5.0" }, bin: "turbo" },
  { road: { road: "bun", package: "eslint", version: "9.0.0" }, bin: "eslint" },
  { road: { road: "uv", package: "ruff", version: "0.6.0" }, bin: "ruff" },
  { road: { road: "pipx", package: "black" }, bin: "black" },
  { road: { road: "cargo", package: "tokei", version: "12.1.2" }, bin: "tokei" },
  { road: { road: "go", module: "github.com/sqlc-dev/sqlc/cmd/sqlc", version: "v1.31.1" }, bin: "sqlc" },
  { road: { road: "apt", packages: ["ripgrep"] }, bin: "rg" },
  { road: { road: "script", script: "curl -fsSL https://x.example/install.sh -o /tmp/i.sh\nsh /tmp/i.sh", version: "1.0.0" }, bin: "x" },
  { road: { road: "release", repo: "cli/cli", version: "v2.60.0", go: "github.com/cli/cli/v2/cmd/gh@v2.60.0" }, bin: "gh" },
];

it("import_roads", async () => {
  const catalogRoads = ["gh", "node", "go", "ffmpeg", "ruby", "yarn", "rust", "pnpm", "bun", "kubectl", "playwright"].flatMap(id => {
    const e = catalogEntry(id);
    return e?.kind === "tool" ? [{ road: e.installRoad, bin: e.bin, from: id }] : [];
  });
  const roads = [...ROADS_SEEN, ...catalogRoads];
  const table: BrewTable = new Map([
    ["node", { name: "node", fullName: "node", deps: [], macosOnly: false, version: "24.1.0" }],
    ["zingzy/tap/diskbloom", { name: "diskbloom", fullName: "zingzy/tap/diskbloom", deps: [], macosOnly: false, version: "0.4.0", source: { repo: "zingzy/diskbloom", tag: "v0.4.0" } }],
    ["felixkratz/formulae/sketchybar", { name: "sketchybar", fullName: "felixkratz/formulae/sketchybar", deps: [], macosOnly: false, source: { repo: "felixkratz/sketchybar", tag: "v2" } }],
    ["acme/tap/mactool", { name: "mactool", fullName: "acme/tap/mactool", deps: [], macosOnly: true, source: { repo: "acme/mactool", tag: "v1" } }],
  ]);
  const rows: RecipeEntry[] = [
    tool("tools/brew-tap/zingzy/tap", { linux: "yes" }),
    tool("tools/brew/gh", { linux: "yes" }),
    tool("tools/brew/jq", { linux: "yes" }),
    tool("tools/brew/jq", { linux: "yes", bring: false }),
    tool("tools/brew/zingzy/tap/diskbloom", { linux: "unknown" }),
    tool("tools/brew/zingzy/tap/diskbloom", { linux: "unknown", pin: { tag: "v0.4.0", sha256: "a".repeat(64), arch: "x86_64" } }),
    tool("tools/brew/felixkratz/formulae/sketchybar", { linux: "unknown" }),
    tool("tools/brew/acme/tap/mactool", { linux: "unknown" }),
    tool("tools/brew/acme/tap/other", { linux: "unknown" }),
    tool("tools/brew/mas", { linux: "no" }),
    tool("tools/brew/bat"),
    tool("tools/brew/node", { linux: "yes" }),
    tool("tools/brew/rust", { linux: "yes" }),
    tool("tools/brew/rustup", { linux: "yes" }),
    tool("tools/brew/go", { linux: "yes" }),
    tool("tools/brew/python@3.12", { linux: "yes" }),
    tool("tools/npm/turbo", { version: "2.5.0" }),
    tool("tools/npm/pnpm"),
    tool("tools/npm/@monid-ai/cli", { label: "@monid-ai/cli@0.3.1", version: "0.3.1" }),
    tool("tools/pnpm/turbo", { version: "2.5.0" }),
    tool("tools/bun/eslint", { version: "9.0.0" }),
    tool("tools/uv/ty", { version: "0.0.56" }),
    tool("tools/uv/ruff"),
    tool("tools/pipx/black", { version: "24.1.0" }),
    tool("tools/cargo/tokei", { version: "12.1.2" }),
    tool("tools/cargo/ripgrep", { version: "14.1.0" }),
    tool("tools/go/sqlc", { label: "sqlc", paths: ["github.com/sqlc-dev/sqlc/cmd/sqlc@v1.31.1"], version: "v1.31.1" }),
    tool("tools/go/old", { label: "github.com/a/old@v1.0.0" }),
    tool("tools/apt/htop"),
    tool("tools/catalog/ffmpeg"),
    tool("tools/catalog/ruby", { version: "3.3.0" }),
    tool("tools/catalog/node", { version: "20.1.0" }),
    tool("tools/catalog/gh", { version: "2.0.0" }),
    tool("tools/catalog/not-a-tool"),
    tool("tools/catalog/claude"),
    tool("tools/custom/thing"),
    row({ rung: "agents", id: "agents/claude" }),
  ];
  const custom = [
    { kind: "custom" as const, id: "ace", name: "ace", install: ["curl -fsSL https://ace.example/i.sh -o /tmp/ace.sh", "sh /tmp/ace.sh"], check: "ace --version", why: "the person wrote it" },
    { kind: "custom" as const, id: "bee", name: "bee", install: ["npm install -g bee@1"], check: "bee --version", manager: "npm", why: "an npm global" },
    { kind: "custom" as const, id: "cee", name: "cee", install: ["brew install cee"], check: "cee --version", manager: "brew", why: "a formula" },
    { kind: "custom" as const, id: "dee", name: "dee", install: ["apt-get install -y dee"], check: "dee --version", manager: "apt", why: "an apt row" },
    { kind: "custom" as const, id: "eff", name: "eff", install: ["true"], check: "eff", manager: "nonsense", why: "a manager nobody knows" },
  ];
  const managers = ["npm", "pnpm", "bun", "uv", "pipx", "cargo", "go", "brew", "apt", "script", "release", "vendor"] as const;
  write("import_roads", {
    TOOLS_PATH,
    PATH_LINE,
    EXEC_ENV,
    pathLine: [
      { path: TOOLS_PATH, out: pathLine(TOOLS_PATH) },
      { path: "/opt/wsp/bin:/usr/bin:/bin", prefix: TOOL_PREFIX, out: pathLine("/opt/wsp/bin:/usr/bin:/bin", TOOL_PREFIX) },
      { path: "", out: pathLine("") },
      { path: "/a b:/c", prefix: "/srv/it's", out: pathLine("/a b:/c", "/srv/it's") },
    ],
    customPrelude: [{ path: TOOLS_PATH, out: customPrelude(TOOLS_PATH) }, { path: "/p", prefix: TOOL_PREFIX, out: customPrelude("/p", TOOL_PREFIX) }],
    brewHousekeeping: [TOOLS_PATH, "/p:/q"].map(p => ({ path: p, out: brewHousekeeping(p) })),
    brewTapCheck: ["zingzy/tap", "it's/tap"].map(t => ({ tap: t, out: brewTapCheck(t) })),
    aptIndexStep: [{ id: "tools/apt-index", cmd: "apt-get update", out: aptIndexStep("tools/apt-index", "apt-get update") }],
    shownOf: [[], ["a"], ["a", "b c", ""]].map(l => ({ lines: l, out: shownOf(l) })),
    roads: roads.flatMap(r =>
      [undefined, TOOL_PREFIX].map(prefix => ({
        ...r,
        prefix: prefix ?? null,
        viaRoad: viaRoad(r.road, r.bin, TOOLS_PATH, prefix),
        roadReads: roadReads(r.road, r.bin, prefix),
        roadStepReads: roadStepReads(r.road, r.bin, prefix),
        pinReadOf: pinReadOf(r.road, r.bin, prefix),
        asksVersion: asksVersion(r.road) ?? null,
      })),
    ),
    table: [...table],
    rows: rows.map(e => ({ row: e, baseRowFor: baseRowFor(e, table) ?? null, rowRoad: rowRoad(e, table) ?? null, catalogToolOf: catalogToolOf(e)?.id ?? null })),
    brewfileFor: [
      { rows, out: brewfileFor(rows, table) },
      { rows: [tool("tools/npm/bun", { label: "bun@1.4.0", version: "1.4.0" })], out: brewfileFor([tool("tools/npm/bun", { label: "bun@1.4.0", version: "1.4.0" })]) },
      { rows: [], out: brewfileFor([]) },
    ],
    managerFormula: managers.map(m => ({ manager: m, out: managerFormula(m) ?? null })),
    custom,
    customInstallsFor: [
      { path: TOOLS_PATH, out: customInstallsFor(custom) },
      { path: "/opt/wsp/bin:/usr/bin", prefix: TOOL_PREFIX, after: { bee: "tools/manager/npm", cee: "tools/brew-toolchain/gcc" }, out: customInstallsFor(custom, c => ({ bee: "tools/manager/npm", cee: "tools/brew-toolchain/gcc" })[c.id], "/opt/wsp/bin:/usr/bin", TOOL_PREFIX) },
    ],
  });
});

// --- import_plan ------------------------------------------------------------------------------------------------

it("import_plan", async () => {
  const table: BrewTable = new Map([
    ["node", { name: "node", fullName: "node", deps: [], macosOnly: false, version: "24.1.0" }],
    ["python", { name: "python", fullName: "python", deps: [], macosOnly: false, version: "3.14.0" }],
    ["python@3.12", { name: "python@3.12", fullName: "python@3.12", deps: [], macosOnly: false, version: "3.12.7" }],
    ["zingzy/tap/diskbloom", { name: "diskbloom", fullName: "zingzy/tap/diskbloom", deps: [], macosOnly: false, version: "0.4.0", source: { repo: "zingzy/diskbloom", tag: "v0.4.0" } }],
    ["acme/tap/hostctl", { name: "hostctl", fullName: "acme/tap/hostctl", deps: [], macosOnly: false, source: { repo: "acme/hostctl", tag: "v1.2.0" } }],
  ]);
  const custom = [
    { kind: "custom" as const, id: "ace", name: "ace", install: ["curl -fsSL https://ace.example/i.sh -o /tmp/ace.sh", "sh /tmp/ace.sh"], check: "ace --version", why: "w" },
    { kind: "custom" as const, id: "bee", name: "bee", install: ["npm install -g bee@1"], check: "bee --version", manager: "npm", why: "w" },
    { kind: "custom" as const, id: "cee", name: "cee", install: ["brew install cee"], check: "cee --version", manager: "brew", why: "w" },
    { kind: "custom" as const, id: "gee", name: "gee", install: ["go install example.com/gee@latest"], check: "gee", manager: "go", why: "w" },
  ];
  const recipes: { name: string; rows: RecipeEntry[]; table?: boolean; custom?: boolean; path?: string; prefix?: string }[] = [
    {
      name: "the TS test's recipe: Homebrew first, taps, formulae, each manager after its own install",
      rows: [
        tool("tools/go/sqlc", { label: "sqlc", paths: ["github.com/sqlc-dev/sqlc/cmd/sqlc@v1.31.1"], version: "v1.31.1" }),
        tool("tools/brew/gh", { linux: "yes" }),
        tool("tools/brew-tap/zingzy/tap", { linux: "yes" }),
        tool("tools/npm/bun", { label: "bun@1.4.0", version: "1.4.0" }),
        tool("tools/npm/@monid-ai/cli", { label: "@monid-ai/cli@0.3.1", version: "0.3.1" }),
        tool("tools/npm/pnpm", { label: "pnpm" }),
        tool("tools/pnpm/turbo", { label: "turbo@2.5.0", version: "2.5.0" }),
        tool("tools/bun/eslint", { label: "eslint@9.0.0", version: "9.0.0" }),
        tool("tools/uv/ty", { label: "ty v0.0.56", version: "0.0.56" }),
        tool("tools/pipx/black", { label: "black", version: "24.1.0" }),
        tool("tools/cargo/tokei", { label: "tokei", version: "12.1.2" }),
      ],
    },
    { name: "the provision tests' gh and turbo on the probe PATH", rows: [tool("tools/brew/gh"), tool("tools/npm/turbo")], path: "/opt/wsp/bin:/usr/bin:/bin" },
    { name: "ruff and tokei under a prefix", rows: [tool("tools/uv/ruff"), tool("tools/cargo/tokei")], path: "/opt/wsp/bin:/usr/bin:/bin", prefix: TOOL_PREFIX },
    {
      name: "rows the base floor covers are the base's",
      rows: [
        tool("tools/brew/jq", { linux: "yes" }),
        tool("tools/npm/pnpm", { label: "pnpm", version: "10.0.0" }),
        tool("tools/brew/python@3.12", { linux: "yes" }),
        tool("tools/brew/node", { linux: "yes" }),
        tool("tools/brew/python", { linux: "yes" }),
        tool("tools/brew/uv", { linux: "yes" }),
        tool("tools/cargo/ripgrep", { label: "ripgrep", version: "14.1.0" }),
        tool("tools/brew/python@3.14", { linux: "yes" }),
      ],
      table: true,
    },
    { name: "one formula: no shared step", rows: [tool("tools/brew/gh", { linux: "yes" })] },
    { name: "two formulae: one shared step", rows: [tool("tools/brew/gh", { linux: "yes" }), tool("tools/brew/yq", { linux: "yes" })] },
    { name: "eight formulae", rows: ["bat", "beads", "btop", "dust", "eza", "fzf", "gum", "yq"].map(n => tool(`tools/brew/${n}`)) },
    { name: "a manager needed only for its rows brings Homebrew when a formula is its road", rows: [tool("tools/pipx/black")] },
    { name: "cargo's rows bring rustup", rows: [tool("tools/cargo/tokei"), tool("tools/cargo/bat-extras")] },
    { name: "a Mac's rust formula is the catalog's rust row", rows: [tool("tools/brew/rust", { linux: "yes" }), tool("tools/cargo/tokei")] },
    { name: "a Mac's rustup formula", rows: [tool("tools/brew/rustup", { linux: "yes" })] },
    { name: "bun's rows wait on an npm install of bun", rows: [tool("tools/bun/eslint")] },
    { name: "the node step runs once ahead of every row on node", rows: [tool("tools/npm/pnpm"), tool("tools/npm/prettier"), tool("tools/catalog/yarn")] },
    { name: "the recipe's own node row is the step", rows: [tool("tools/catalog/node"), tool("tools/npm/pnpm")] },
    { name: "a Mac's node formula", rows: [tool("tools/brew/node", { linux: "yes" }), tool("tools/npm/pnpm")] },
    { name: "no Homebrew for unticked rows", rows: [tool("tools/brew/gh", { bring: false }), tool("tools/npm/pnpm", { bring: false })] },
    { name: "nothing", rows: [] },
    {
      name: "catalog rows on every road",
      rows: [tool("tools/catalog/ffmpeg"), tool("tools/catalog/ruby", { version: "3.3.0" }), tool("tools/catalog/gh", { version: "2.0.0" }), tool("tools/catalog/kubectl"), tool("tools/catalog/playwright"), tool("tools/catalog/not-a-tool"), tool("tools/catalog/jq")],
    },
    { name: "a catalog go row beside go rows", rows: [tool("tools/catalog/go"), tool("tools/go/sqlc", { label: "sqlc", paths: ["github.com/sqlc-dev/sqlc/cmd/sqlc@v1.31.1"], version: "v1.31.1" })] },
    {
      name: "tap formulae on the release road",
      rows: [tool("tools/brew/zingzy/tap/diskbloom", { linux: "unknown", pin: { tag: "v0.4.0", sha256: "b".repeat(64), arch: "x86_64" } }), tool("tools/brew/acme/tap/hostctl", { linux: "unknown" }), tool("tools/brew/acme/tap/missing", { linux: "unknown" }), tool("tools/brew/mas", { linux: "no" })],
      table: true,
    },
    { name: "custom rows after everything, each after its manager", rows: [tool("tools/brew/gh", { linux: "yes" })], custom: true },
    { name: "custom rows alone bring their managers", rows: [], custom: true, prefix: TOOL_PREFIX, path: "/opt/wsp/bin:/usr/bin:/bin" },
    { name: "provision's own rows: gh and turbo, the id as the label", rows: ["tools/brew/gh", "tools/npm/turbo"].map(prow), path: "/opt/wsp/bin:/usr/bin:/bin" },
    { name: "provision's own rows: ruff and tokei under a prefix", rows: ["tools/uv/ruff", "tools/cargo/tokei"].map(prow), path: "/opt/wsp/bin:/usr/bin:/bin", prefix: TOOL_PREFIX },
    { name: "an agent row and an apt row install nothing here", rows: [row({ rung: "agents", id: "agents/claude" }), tool("tools/apt/htop"), tool("tools/brew-tap/a/b")] },
  ];
  write("import_plan", {
    table: [...table],
    custom,
    recipes: recipes.map(r => {
      let out: unknown;
      try {
        out = toolInstallsFor(r.rows, r.table === true ? table : new Map(), r.custom === true ? custom : [], r.path ?? TOOLS_PATH, r.prefix);
      } catch (e) {
        out = { error: e instanceof Error ? e.message : String(e) };
      }
      return { ...r, path: r.path ?? TOOLS_PATH, out };
    }),
  });
});

// --- import_agents ----------------------------------------------------------------------------------------------

it("import_agents", async () => {
  const agent = (id: string, over: Partial<RecipeEntry> = {}): RecipeEntry => row({ rung: "agents", id, ...over });
  const dates = ["2026-09-03T00:00:00Z", "2027-05-01T00:00:00Z", "2031-01-01T00:00:00Z"];
  const ids = CATALOG_AGENTS.map(a => `agents/${a.id}`);
  const plans: { name: string; rows: RecipeEntry[]; now: string }[] = [
    { name: "claude and codex", rows: [agent("agents/claude"), agent("agents/codex")], now: dates[0]! },
    { name: "codex alone", rows: [agent("agents/codex")], now: dates[0]! },
    { name: "claude alone", rows: [agent("agents/claude")], now: dates[0]! },
    { name: "every catalog agent", rows: ids.map(id => agent(id)), now: dates[0]! },
    { name: "every catalog agent, later", rows: ids.map(id => agent(id)), now: dates[1]! },
    { name: "every catalog agent, past every pin's support", rows: ids.map(id => agent(id)), now: dates[2]! },
    { name: "ticks, an unknown agent, a shell row and an mcp row", rows: [agent("agents/claude"), agent("agents/gemini", { bring: false }), agent("agents/unknown-thing"), row({ rung: "shell", id: "shell/zshrc" }), agent(`${MCP_ID_PREFIX}claude/github`), agent("agents/aider")], now: dates[0]! },
    { name: "a pinned codex installs at its pin, a latest mark does not", rows: [agent("agents/codex", { pin: { tag: "0.40.0" } }), agent("agents/gemini", { pin: { tag: "9.9.9", latest: true } })], now: dates[0]! },
    { name: "nothing", rows: [], now: dates[0]! },
    { name: "provision's own rows: claude and codex, the id as the label", rows: ["agents/claude", "agents/codex"].map(prow), now: dates[0]! },
  ];
  const planned = plans.map(p => ({ ...p, out: agentInstallsFor(p.rows, CATALOG_AGENTS, new Date(p.now)) }));
  write("import_agents", {
    installers: Object.entries(AGENT_INSTALLERS).map(([id, a]) => ({ id, ...a })),
    nodeMajorFor: [0, 18, 20, 22, 23, 24, 25, 99].flatMap(floor => dates.map(now => ({ floor, now, out: nodeMajorFor(floor, new Date(now)) ?? null }))),
    nodeFloorCheck: [18, 22].map(f => ({ floor: f, out: nodeFloorCheck(f) })),
    agentInstallsFor: planned,
    agentSteps: planned.flatMap(p =>
      [
        { path: TOOLS_PATH },
        { path: "/opt/wsp/bin:/usr/bin:/bin", prefix: TOOL_PREFIX },
      ].map(o => ({ plan: p.name, ...o, out: agentSteps(p.out, CATALOG_AGENTS, o.path, o.prefix) })),
    ),
  });
});

// --- mcp_files --------------------------------------------------------------------------------------------------

const HOME = "/Users/dev";
const FORMAT_NAMES = new Map<McpFormat, string>([
  [MCP_SERVERS_JSON, "McpServersJson"],
  [CODEX_TOML, "CodexToml"],
  [OPENCODE_JSON, "OpencodeJson"],
]);
const planJson = (p: McpPlan): unknown => ({
  ...p,
  agents: p.agents.map(a => ({ ...a, scopes: a.scopes.map(s => ({ ...s, format: FORMAT_NAMES.get(s.format) ?? "unknown" })) })),
});
const agentsJson = (agents: readonly McpAgentPlan[]): unknown => planJson({ agents: [...agents], guestHome: "", rewrites: [], binDirs: [], tools: [] }) as { agents: unknown };

const CLAUDE = {
  numStartups: 3,
  mcpServers: {
    github: { type: "stdio", command: "npx", args: ["-y", "@modelcontextprotocol/server-github"], env: { GITHUB_TOKEN: "ghp_secret" } },
    gsc: { command: "uvx", args: ["mcp-search-console"], env: { GSC_CREDENTIALS_PATH: `${HOME}/.config/gsc/creds.json` } },
    notes: { command: `${HOME}/Library/Application Support/Notes/mcp`, args: [] },
    survivor: { command: "survivor-bin", args: [] },
    local: { command: `${HOME}/.local/bin/local-mcp`, args: ["--root=/Users/dev/code"] },
  },
  projects: {
    [HOME]: { allowedTools: ["Bash"], mcpServers: { zomato: { type: "stdio", command: "npx", args: ["mcp-remote", "https://mcp.zomato.com/mcp"] }, whatsapp: { command: `${HOME}/.local/bin/uv`, args: ["--directory", `${HOME}/code/wa`, "run", "main.py"] } } },
  },
};
const CODEX = ['model = "gpt-5"', "", "[mcp_servers.grafana]", 'command = "/opt/homebrew/bin/uvx"', 'args = ["mcp-grafana", "--config=/Users/dev/.config/grafana.toml"]', "", "[mcp_servers.sentry]", 'url = "https://mcp.sentry.dev/mcp"', "", "[mcp_servers.keeper]", 'command = "keeper"', ""].join("\n");

function mcpPlan(over: Partial<McpPlan> = {}): McpPlan {
  return {
    agents: [
      {
        id: "claude",
        label: "Claude Code",
        scopes: [
          { files: ["/root/.claude-cfg/.claude.json"], format: MCP_SERVERS_JSON, keep: ["github", "gsc", "survivor", "local"], drop: [{ name: "notes", reason: "macOS-only" }] },
          { files: ["/root/.claude-cfg/.claude.json"], format: MCP_SERVERS_JSON, project: { from: HOME, to: "/root" }, keep: ["zomato", "whatsapp"], drop: [] },
        ],
        aside: [],
      },
      { id: "codex", label: "Codex", scopes: [{ files: ["/root/.codex/config.toml", "/root/.codex/alt.toml"], format: CODEX_TOML, keep: ["grafana", "keeper"], drop: [{ name: "sentry", reason: "unticked", keys: ["SENTRY_TOKEN"] }] }], aside: [] },
      { id: "opencode", label: "OpenCode", scopes: [{ files: ["/root/.config/opencode/opencode.json"], format: OPENCODE_JSON, keep: ["ctx"], drop: [] }], aside: [{ id: `${MCP_ID_PREFIX}opencode/late`, name: "late", reason: "OpenCode is not ticked, so its config did not travel" }] },
    ],
    guestHome: "/root",
    rewrites: [[`${HOME}/`, "/root/"], ["/opt/homebrew/", "/home/linuxbrew/.linuxbrew/"]],
    binDirs: [`${HOME}/.local/bin/`, "~/.local/bin/", "/opt/homebrew/bin/", "/opt/homebrew/sbin/"],
    tools: [{ id: "tools/brew/survivor-bin", ticked: false, reason: "unticked by you" }, { id: "tools/npm/keeper", ticked: true }, { id: "tools/cargo/local-mcp", ticked: true }],
    ...over,
  };
}
const b64 = (s: string): string => Buffer.from(s, "utf8").toString("base64");
const READ_OUT = [`wsp-mcp 0 0 ${b64(`${JSON.stringify(CLAUDE, null, 2)}\n`)}`, `wsp-mcp 1 0 ${b64(`${JSON.stringify(CLAUDE, null, 2)}\n`)}`, `wsp-mcp 2 0 ${b64(CODEX)}`, "wsp-mcp 3 - -", MCP_READ_END, ""].join("\n");

it("mcp_files", async () => {
  const scopes = [{ files: ["/root/.claude-cfg/.claude.json"] }, { files: ["/root/.codex/config.toml", "/root/.codex/alt.toml"] }, { files: [] as string[] }];
  const parses = [
    "",
    "noise only\n",
    `wsp-mcp 0 0 ${b64('{"a":1}')}\n`,
    `wsp-mcp 0 0 ${b64('{"a":1}')}\nwsp-mcp 1 1 ${b64("x = 1")}\n${MCP_READ_END}\n`,
    "wsp-mcp 0 0\n",
    "wsp-mcp 0 0 \n",
    "wsp-mcp 0 - -\nwsp-mcp 1 - -\n",
    "wsp-mcp 0 0 -plain\nwsp-mcp 1 0 -cap\n",
    "wsp-mcp 7 0 eA==\nwsp-mcp 0 9 eA==\nwsp-mcp x y z\n",
    "  wsp-mcp 0 0 eA==  \r\n",
    "wsp-mcp 0 0 !!notbase64**\n",
    `wsp-mcp 0 0 ${b64("first")}\nwsp-mcp 0 0 ${b64("second")}\n`,
    "wsp-mcp 0 0 4pyTIMOp\n",
    "wsp-mcp 0 0 eA\n",
    "wsp-mcp  0 0 eA==\n",
  ];
  const plan = mcpPlan();
  const rewrites = [
    ["npx", true],
    [`${HOME}/.local/bin/uv`, true],
    [`${HOME}/.local/bin/sub/uv`, true],
    ["~/.local/bin/codebase-memory-mcp", true],
    ["/opt/homebrew/bin/uvx", true],
    ["/opt/homebrew/bin/uvx", false],
    [`${HOME}/.config/x.json`, false],
    [`--config=${HOME}/.config/grafana.toml`, false],
    [`-c=${HOME}/x`, false],
    [`--weird flag=${HOME}/x`, false],
    ["/opt/homebrew/lib/node", false],
    [`${HOME}`, false],
    [`${HOME}/`, false],
    ["", true],
  ] as const;
  const rows = [
    row({ rung: "agents", id: "agents/claude" }),
    row({ rung: "agents", id: "agents/codex" }),
    row({ rung: "agents", id: `${MCP_ID_PREFIX}claude/github` }),
    row({ rung: "agents", id: `${MCP_ID_PREFIX}claude/gsc`, bring: false, reason: "unticked by you" }),
    row({ rung: "agents", id: `${MCP_ID_PREFIX}claude/stripe`, consent: true }),
    row({ rung: "agents", id: `${MCP_ID_PREFIX}claude/home/zomato` }),
    row({ rung: "agents", id: `${MCP_ID_PREFIX}codex/grafana`, bring: false }),
    row({ rung: "agents", id: `${MCP_ID_PREFIX}gemini/memory` }),
    row({ rung: "agents", id: `${MCP_ID_PREFIX}mcp-remote` }),
    tool("tools/brew/gh", { bring: false, reason: "not wanted" }),
    tool("tools/npm/keeper"),
    tool("tools/brew-tap/a/b"),
    tool("tools/catalog/kubectl"),
  ];
  const AGENTS = {
    claude: { label: "Claude Code", format: MCP_SERVERS_JSON, files: ["/root/.claude-cfg/.claude.json"] },
    codex: { label: "Codex", format: CODEX_TOML, files: ["/root/.codex/config.toml"] },
    gemini: { label: "Gemini CLI", format: MCP_SERVERS_JSON, files: ["/root/.gemini/settings.json"] },
  };
  const lands = [];
  for (const s of [
    { name: "nothing changed lands nothing", read: [{ path: "/root/a.json", text: "{}\n", sum: "s1" }], texts: [["/root/a.json", "{}\n"]] as [string, string][], rules: [] as Rule[] },
    { name: "two changed files land through the byte road, then one write each", read: [{ path: "/root/a.json", text: "{}\n", sum: "s1" }, undefined, { path: "/root/c.toml", unread: "not a plain file" }], texts: [["/root/a.json", '{"x":1}\n'], ["/root/b.json", "{}\n"], ["/root/c.toml", "x"]] as [string, string][], rules: [] as Rule[] },
    { name: "a landing that fails is swept", read: [], texts: [["/root/a.json", "{}\n"]] as [string, string][], rules: [{ has: "/root/", kind: "put", seq: [{ throws: "the upload broke" }] }] as Rule[] },
    { name: "a write the machine refuses is swept with the refusal's words", read: [{ path: "/root/a.json", text: "{}\n", sum: "s1" }], texts: [["/root/a.json", '{"y":2}\n']] as [string, string][], rules: [{ has: "wsp-config", kind: "exec", seq: [{ exit: 3, stderr: "something else\n" }] }] as Rule[] },
    { name: "a write call that rejects", read: [], texts: [["/root/a.json", "{}\n"]] as [string, string][], rules: [{ has: "printf", kind: "exec", seq: [{ throws: "lost" }] }] as Rule[] },
  ]) {
    const { machine, calls } = scripted(s.rules, { put: true });
    const out = await landConfigs(machine, p => (p.includes(".toml") ? "/root/.codex" : "/root"), s.read, new Map(s.texts));
    lands.push({ ...s, read: s.read.map(r => r ?? null), out: out ?? null, calls });
  }
  write("mcp_files", {
    READ_MS,
    MCP_READ_MARK,
    MCP_READ_END,
    mcpRowId: [
      ["claude", false, "github"],
      ["claude", true, "zomato"],
      ["claude", "/root/code/my app", "db"],
      ["codex", "/root/a/b", "x/y"],
      ["claude", "", "e"],
    ].map(([a, at, n]) => ({ agent: a, at, name: n, out: mcpRowId(a as string, at as boolean | string, n as string) })),
    scopeRowId: [
      { agent: "claude", scope: {}, name: "github" },
      { agent: "claude", scope: { project: { from: HOME, to: "/root" } }, name: "zomato" },
      { agent: "claude", scope: { folder: "/root/code/app" }, name: "db" },
      { agent: "claude", scope: { folder: "/root/code/app", project: { from: HOME, to: "/root" } }, name: "db" },
      { agent: "claude", scope: { folder: "" }, name: "e" },
    ].map(s => ({ ...s, out: scopeRowId(s.agent, s.scope, s.name) })),
    parseMcpId: [
      `${MCP_ID_PREFIX}claude/github`,
      `${MCP_ID_PREFIX}claude/home/zomato`,
      `${MCP_ID_PREFIX}claude/home/a/b`,
      `${MCP_ID_PREFIX}claude/home`,
      `${MCP_ID_PREFIX}claude/@%2Froot%2Fcode%2Fmy%20app/db`,
      `${MCP_ID_PREFIX}claude/@%2Froot/a/b`,
      `${MCP_ID_PREFIX}claude/@x`,
      `${MCP_ID_PREFIX}claude/x/y/z`,
      `${MCP_ID_PREFIX}claude`,
      `${MCP_ID_PREFIX}`,
      `${MCP_ID_PREFIX}mcp-remote`,
      `${MCP_ID_PREFIX}mcp-remote/x`,
      "agents/claude",
      "tools/brew/gh",
      `${MCP_ID_PREFIX}/name`,
    ].map(id => ({ id, out: parseMcpId(id) ?? null })),
    refused: ["the provider refused the call", ""].map(m => ({ message: m, out: refused(new Error(m)) })),
    readReason: [res(124, "secret config", "err"), res(1, "secret\nmore", ""), res(1, "secret", "Error: cut\n"), res(-1, "", "refused by the provider")].flatMap(r => [undefined, 30].map(t => ({ res: r, timeoutS: t ?? null, out: readReason(r, t) }))),
    readFailed: [res(124, "x", ""), res(-1, "", "nope"), res(2, "only stdout", "")].map(r => ({ res: r, out: readFailed(r) })),
    readConfigsCmd: [
      { scopes: [{ files: ["/root/.claude-cfg/.claude.json"], base: "/root" }, { files: ["/root/.codex/config.toml", "/root/it's here.toml"], base: "/root/.codex" }] },
      { scopes: [] },
      { scopes: [{ files: [], base: "/root" }] },
    ].map(s => ({ ...s, out: readConfigsCmd(s.scopes) })),
    readWhole: ["", MCP_READ_END, `x\n  ${MCP_READ_END}  \n`, `${MCP_READ_END}x`].map(s => ({ stdout: s, out: readWhole(s) })),
    parseConfigs: parses.map(p => ({ stdout: p, scopes, out: parseConfigs(p, scopes) ?? null })),
    plan: planJson(plan),
    rewriteString: rewrites.map(([s, command]) => ({ s, command, out: rewriteString(plan, s, command) })),
    mcpPlanFor: [
      { rows, home: HOME, out: (() => { const p = mcpPlanFor(rows, { home: HOME, agents: AGENTS }); return p === undefined ? null : planJson(p); })() },
      { rows, home: HOME, guestHome: "/srv/guest", binDirs: ["/usr/local/bin/"], out: (() => { const p = mcpPlanFor(rows, { home: HOME, guestHome: "/srv/guest", agents: AGENTS, binDirs: ["/usr/local/bin/"] }); return p === undefined ? null : planJson(p); })() },
      { rows: [row({ rung: "agents", id: "agents/claude" })], home: HOME, out: mcpPlanFor([row({ rung: "agents", id: "agents/claude" })], { home: HOME, agents: AGENTS }) ?? null },
    ],
    landConfigs: lands,
  });
});

// --- mcp_rows ---------------------------------------------------------------------------------------------------

it("mcp_rows", async () => {
  const plan = mcpPlan();
  const outcome = (o: Partial<ScopeOutcome>): ScopeOutcome => ({ file: "/root/.claude-cfg/.claude.json", results: [], ...o });
  const report: ScopeOutcome[] = [
    outcome({ results: [{ name: "github", outcome: "written", command: "npx" }, { name: "gsc", outcome: "written", command: "uvx" }, { name: "survivor", outcome: "written", command: "survivor-bin" }, { name: "local", outcome: "written", command: "local-mcp" }, { name: "notes", outcome: "dropped" }] }),
    outcome({ results: [{ name: "zomato", outcome: "written", command: "npx" }, { name: "whatsapp", outcome: "missing" }] }),
    outcome({ file: "/root/.codex/config.toml", results: [{ name: "grafana", outcome: "written", command: "/home/linuxbrew/.linuxbrew/bin/uvx" }, { name: "keeper", outcome: "written", command: "~/.local/bin/keeper" }, { name: "sentry", outcome: "dropped" }] }),
    outcome({ file: null }),
  ];
  const absent = [];
  for (const s of [
    { name: "two commands missing", report, rules: [{ has: "command -v", seq: [{ stdout: "ok npx\nno uvx\nno survivor-bin\nok local-mcp\nno /root/.local/bin/keeper\nok /home/linuxbrew/.linuxbrew/bin/uvx\n" }] }] as Rule[] },
    { name: "no command at all asks nothing", report: [outcome({ results: [{ name: "a", outcome: "written" }] })], rules: [] as Rule[] },
    { name: "a check the machine refuses drops nothing", report, rules: [{ has: "command -v", seq: [{ throws: "refused" }] }] as Rule[], path: "/custom/bin" },
  ]) {
    const { machine, calls } = scripted(s.rules);
    absent.push({ name: s.name, report: s.report, path: s.path ?? null, out: await absentCommands(machine, plan, s.report, s.path), calls });
  }
  const tools: ToolResult[] = [{ id: "tools/npm/keeper", label: "keeper", outcome: "failed", note: "npm broke" }, { id: "tools/cargo/local-mcp", label: "local-mcp", outcome: "installed" }];
  const missingSets = [["uvx", "survivor-bin", "/root/.local/bin/keeper", "local-mcp"], [], ["npx", "uvx"]];
  const without = missingSets.map(m => ({ missing: m, tools, out: agentsJson(withoutAbsent(plan, plan.agents, report, new Set(m), tools)) }));
  const rowsRuns = [];
  for (const s of [
    { name: "every kind of row, uv installed for the servers that need it", report, missing: ["uvx", "survivor-bin"], present: [`${MCP_ID_PREFIX}claude/github`], rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "uv", kind: "run", seq: [{ lines: ["downloading uv"] }] }] as Rule[] },
    { name: "a failed uv install is named on every server that needed it", report, missing: ["uvx", "/home/linuxbrew/.linuxbrew/bin/uvx"], present: [], rules: [{ has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }, { has: "uv", kind: "run", seq: [{ exit: 1, stderr: "curl: (22) 404\n" }] }] as Rule[] },
    { name: "a failure skips every kept server with its words", report, failure: "the config edit did not run (exit 2)", missing: [], present: [], rules: [{ has: FREE_HAS, seq: [{ exit: 1 }] }] as Rule[] },
    { name: "no report: every config is not on the machine", report: undefined, missing: [], present: [], rules: [{ has: FREE_HAS, seq: [{ throws: "df rejected" }] }] as Rule[] },
    { name: "unread and error outcomes", report: [outcome({ unread: "not a plain file", results: [] }), outcome({ error: "Unexpected token } in JSON", results: [] }), outcome({ file: "/root/.codex/config.toml", results: [] }), outcome({ file: null })], missing: [], present: [], rules: [] as Rule[] },
  ]) {
    const { machine, calls } = scripted(s.rules);
    const stages: [string, string | null][] = [];
    const out = await mcpRows(machine, plan, { agents: plan.agents, ...(s.report !== undefined ? { report: s.report } : {}), ...(s.failure !== undefined ? { failure: s.failure } : {}), missing: new Set(s.missing), present: new Set(s.present), stage: (st, d) => void stages.push([st, d ?? null]) });
    rowsRuns.push({ ...s, report: s.report ?? null, failure: s.failure ?? null, out, stages, calls });
  }
  const applies = [];
  for (const s of [
    { name: "the whole round: read, both edits, the PATH check, the landing", rules: [{ has: "wsp_mcp_read", kind: "run", seq: [{ stdout: READ_OUT }] }, { has: "command -v", seq: [{ stdout: "ok npx\nno uvx\nno survivor-bin\nok local-mcp\nok /home/linuxbrew/.linuxbrew/bin/uvx\nok keeper\n" }] }, { has: FREE_HAS, seq: [{ stdout: kb(9 * GiB) }] }] as Rule[] },
    { name: "a read whose leader was killed says so by its exit code", rules: [{ has: "wsp_mcp_read", kind: "run", seq: [{ exit: 137, stdout: READ_OUT }] }] as Rule[] },
    { name: "a machine that refuses the script", rules: [{ has: "wsp_mcp_read", kind: "run", seq: [{ throws: "the request body is over the provider's cap" }] }] as Rule[] },
    { name: "a read that printed no line with the mark", rules: [{ has: "wsp_mcp_read", kind: "run", seq: [{ stdout: "bash: realpath: not found\n" }] }] as Rule[] },
  ]) {
    const { machine, calls } = scripted(s.rules, { put: true });
    const stages: [string, string | null][] = [];
    const out = await applyMcp(machine, plan, (st, d) => void stages.push([st, d ?? null]), tools);
    applies.push({ ...s, out, stages, calls });
  }
  const tally = [[], [{ outcome: "installed" }, { outcome: "skipped" }, { outcome: "installed" }, { outcome: "fetched-on-first-use" }], [{ outcome: "skipped" }]].map(r => ({ rows: r, out: mcpTally(r as never) ?? null }));
  const needRows = [{ id: "tools/brew/gh" }, { id: "tools/npm/keeper" }, { id: "tools/cargo/local-mcp", bin: "lm" }, { id: "tools/catalog/kubectl" }, { id: "tools/custom/thing" }, { id: "tools/brew-tap/a/b" }, { id: "tools/release/x" }];
  write("mcp_rows", {
    plan: planJson(plan),
    FETCHERS,
    report,
    absentCommands: absent,
    asRun: ["~/.local/bin/x", "~x", "/abs", "npx"].map(c => ({ command: c, out: asRun(plan, c) })),
    absentReason: ["survivor-bin", "/x/keeper", "local-mcp", "nothing-here", "/opt/homebrew/bin/gh"].map(c => ({ command: c, tools, out: absentReason(c, plan, tools) })),
    withoutAbsent: without,
    mcpOpening: [{ agents: agentsJson(plan.agents), out: mcpOpening(plan.agents) }, { agents: [], out: mcpOpening([]) }],
    mcpRows: rowsRuns,
    applyMcp: applies,
    mcpTally: tally,
    commandRow: ["gh", "/usr/bin/keeper", "lm", "kubectl", "thing", "b", "nope", "x"].map(c => ({ command: c, rows: needRows, out: commandRow(c, needRows) ?? null })),
    serverNeeds: ["npx", "/x/uvx", "uv", "git", "jq", "unzip", "gh", "keeper", "unknown-bin", "ready-one"].map(c => ({ command: c, rows: needRows, ready: ["ready-one"], out: serverNeeds(c, needRows, new Set(["ready-one"])) })),
  });
});
