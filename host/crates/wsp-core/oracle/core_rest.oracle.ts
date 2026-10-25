// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the core-rest tests pin, for the wsp-core files no other slice wrote, at 65f6b89c7: one JSON file
// per unit under tests/fixtures/core_rest/. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-core/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 host/crates/wsp-core/oracle/core_rest.oracle.ts
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it } from "vitest";

const S = "../../../../packages/runtime/src";
const roots = await import(`${S}/daemon-roots.ts`);
const token = await import(`${S}/daemon-token.ts`);
const hostId = await import(`${S}/host-id.ts`);
const catalog = await import(`${S}/harness-catalog.ts`);
const landing = await import(`${S}/project-landing.ts`);
const sources = await import(`${S}/project-sources.ts`);
const recipe = await import(`${S}/recipe-sync.ts`);
const setup = await import(`${S}/agent-setup.ts`);
const adapters = await import(`${S}/adapters.ts`);
const protocol = await import("@wsp/protocol");
const store = await import(`${S}/store.ts`);
const claudeAdapter = await import("@wsp/adapter-claude");
const catalogPkg = await import("@wsp/catalog");

const out = fileURLToPath(new URL("../tests/fixtures/core_rest/", import.meta.url));
const write = (name: string, value: unknown): void => {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, `${name}.json`), `${JSON.stringify(value, null, 1)}\n`);
};
const tryIt = (f: () => unknown): unknown => {
  try {
    return { ok: f() ?? null };
  } catch (e) {
    return { err: (e as Error).message, ...((e as { kind?: string }).kind !== undefined ? { kind: (e as { kind?: string }).kind } : {}) };
  }
};
const tryAsync = async (f: () => Promise<unknown>): Promise<unknown> => {
  try {
    return { ok: (await f()) ?? null };
  } catch (e) {
    return { err: (e as Error).message, ...((e as { kind?: string }).kind !== undefined ? { kind: (e as { kind?: string }).kind } : {}) };
  }
};

const TOKEN = "0123456789abcdef".repeat(3);

it("daemon files", () => {
  write("daemon_files", {
    rootsScript: [
      { roots: [], path: null, script: roots.writeDaemonRootsScript([]) },
      { roots: ["/srv/web"], path: null, script: roots.writeDaemonRootsScript(["/srv/web"]) },
      { roots: ["/srv/it's", "/home/dev/a b"], path: null, script: roots.writeDaemonRootsScript(["/srv/it's", "/home/dev/a b"]) },
      { roots: ["/x"], path: "/home/dev/.wsp/roots", script: roots.writeDaemonRootsScript(["/x"], "/home/dev/.wsp/roots") },
      { roots: ["/x"], path: "roots", script: roots.writeDaemonRootsScript(["/x"], "roots") },
      { roots: ["/x"], path: "/roots", script: roots.writeDaemonRootsScript(["/x"], "/roots") },
    ],
    tokenFor: [
      { seed: TOKEN, machine: "m1", out: tryIt(() => token.daemonTokenFor(TOKEN, "m1")) },
      { seed: TOKEN, machine: "m2", out: tryIt(() => token.daemonTokenFor(TOKEN, "m2")) },
      { seed: "beefcafe".repeat(3), machine: "m1", out: tryIt(() => token.daemonTokenFor("beefcafe".repeat(3), "m1")) },
      { seed: TOKEN, machine: "", out: tryIt(() => token.daemonTokenFor(TOKEN, "")) },
      { seed: TOKEN, machine: "ws_é", out: tryIt(() => token.daemonTokenFor(TOKEN, "ws_é")) },
      { seed: "it's not hex", machine: "m1", out: tryIt(() => token.daemonTokenFor("it's not hex", "m1")) },
      { seed: "ABCDEF", machine: "m1", out: tryIt(() => token.daemonTokenFor("ABCDEF", "m1")) },
    ],
    shape: ["", "0", "abc", "ABC", "0123x", "it's-not-hex-at-all-000", TOKEN, "ab\n"].map(t => ({ token: t, out: tryIt(() => token.assertTokenShape(t)) })),
    writeScript: [
      { token: TOKEN, path: null, out: tryIt(() => token.writeDaemonTokenScript(TOKEN)) },
      { token: TOKEN, path: "/tmp/stand-in/m1/.wsp-daemon-token", out: tryIt(() => token.writeDaemonTokenScript(TOKEN, "/tmp/stand-in/m1/.wsp-daemon-token")) },
      { token: "'; rm -rf /; echo '0123456789abcdef", path: null, out: tryIt(() => token.writeDaemonTokenScript("'; rm -rf /; echo '0123456789abcdef")) },
    ],
    rotateScript: [
      { token: TOKEN, path: null, out: tryIt(() => token.rotateDaemonTokenScript(TOKEN)) },
      { token: TOKEN, path: "/home/dev/.wsp/daemon-token", out: tryIt(() => token.rotateDaemonTokenScript(TOKEN, "/home/dev/.wsp/daemon-token")) },
      { token: "nothex!", path: null, out: tryIt(() => token.rotateDaemonTokenScript("nothex!")) },
    ],
    constants: { set: token.DAEMON_TOKEN_SET, none: token.DAEMON_TOKEN_NONE, path: protocol.DAEMON_TOKEN_PATH, rootsPath: protocol.DAEMON_ROOTS_PATH },
    templateHost: ["zingzys-MacBook-Pro.local:9f3a1c2b", "dev.box:00ff00ff", "zingzys-MacBook-Pro.local", "", ":", "a:b:C-1_2", "host:", "Ünïcode:ÄB9", "x:İ"].map(h => [h, hostId.templateHost(h)]),
  });
});

it("harness catalog", () => {
  const probe = {
    version: "2.1.257",
    models: [
      { slug: "claude-opus-5-5", label: "Opus", description: "Opus 5.5 with 1M context", efforts: ["low", "high"], contextWindows: ["200k", "1m"], isDefault: true },
      { slug: "claude-haiku-4-5-20251001", label: "Haiku", efforts: [], contextWindows: [], isDefault: false },
      { slug: "claude-next-6", label: "Next", efforts: ["low", "turbo"], contextWindows: [], isDefault: false },
    ],
    efforts: ["low", "high", "turbo"],
    permissionModes: ["default", "acceptEdits", "yolo"],
  };
  const codexProbe = {
    version: "0.153.0",
    models: [
      { slug: "gpt-5.6-sol", label: "gpt-5.6-sol", efforts: ["low", "high"], defaultEffort: "low", contextWindows: [], isDefault: true },
      { slug: "gpt-5.5", label: "gpt-5.5", efforts: ["low", "medium"], defaultEffort: "low", contextWindows: [], isDefault: false },
    ],
    efforts: ["low", "medium", "high"],
    permissionModes: ["read-only"],
  };
  const claude = catalog.harnessCatalog("claude");
  const codex = catalog.harnessCatalog("codex");
  const probes: [string, unknown][] = [
    ["claude+probe", catalog.catalogFromProbe(claude, probe)],
    ["claude+probe, efforts low turbo", catalog.catalogFromProbe(claude, { ...probe, efforts: ["low", "turbo"] })],
    ["claude+probe+opus-4-8", catalog.catalogFromProbe(claude, { ...probe, models: [...probe.models, { slug: "claude-opus-4-8", label: "Opus", efforts: ["low"], contextWindows: [], isDefault: false }] })],
    ["codex+codexProbe", catalog.catalogFromProbe(codex, codexProbe)],
    ["codex+codexProbe, default gpt-5.5", catalog.catalogFromProbe(codex, { ...codexProbe, models: codexProbe.models.map(m => ({ ...m, isDefault: m.slug === "gpt-5.5" })) })],
    ["claude+probe, no version", catalog.catalogFromProbe(claude, { ...probe, version: null })],
    ["codex+routed", catalog.catalogFromProbe(codex, { ...probe, models: [{ slug: "anthropic/claude-sonnet-4.5", label: "anthropic/claude-sonnet-4.5", contextWindows: [], isDefault: true }, ...probe.models.map(m => ({ ...m, isDefault: false }))] })],
    ["claude+perModel", catalog.catalogFromProbe(claude, { ...probe, models: probe.models.map(m => (m.slug === "claude-next-6" ? { ...m, defaultEffort: "turbo" } : m)) })],
    ["codex+probe default turbo", catalog.catalogFromProbe(codex, { ...probe, models: probe.models.map(m => (m.isDefault ? { ...m, defaultEffort: "turbo" } : m)) })],
    ["codex+probe", catalog.catalogFromProbe(codex, probe)],
    ["codex+probe default nope", catalog.catalogFromProbe(codex, { ...probe, models: probe.models.map(m => (m.isDefault ? { ...m, defaultEffort: "nope" } : m)) })],
    ["claude flagged+probe", catalog.catalogFromProbe({ ...claude, isDefault: true, steers: true }, probe)],
    ["codex+astra", catalog.catalogFromProbe(codex, { version: "0.155.1", models: [{ slug: "gpt-6-astra", label: "GPT-6-Astra", contextWindows: [], isDefault: true, fast: true }, { slug: "gpt-5.2", label: "GPT-5.2", contextWindows: [], isDefault: false }], efforts: [], permissionModes: ["read-only", "workspace-write", "danger-full-access"] })],
    ["claude+bare opus", catalog.catalogFromProbe(claude, { version: "2.1.283", models: [{ slug: "claude-opus-5-5", label: "Opus 5.5", contextWindows: [], isDefault: true }], efforts: [], permissionModes: [] })],
    ["claude+empty", catalog.catalogFromProbe(claude, { version: "9.9.9", models: [], efforts: [], permissionModes: [] })],
    ["pi+probe", catalog.catalogFromProbe(catalog.harnessCatalog("pi"), probe)],
  ];
  const labels = ["", "claude-opus-5-5", "anthropic/claude-opus-5-5", "claude-sonnet-4-5-20250929", "claude-haiku-4-5", "claude-haiku-4-5-20251001", "gpt-5.5", "gpt-6-astra", "openrouter/gpt-6-astra", "gpt-4o-mini", "claude-opus-4-1", "claude-3-5-sonnet-20241022", "llama-3-70b", "o3", "gemini-2.5-pro", "a--b", "x/", "/", "-", "deepseek-v3.1", "qwen3-coder-30b-a3b", "mistral-large-2411"];
  const asks: [string | null, string | null][] = [
    ["claude", null], ["claude", "bypassPermissions"], ["claude", "default"], ["claude", "acceptEdits"], ["codex", "danger-full-access"], ["codex", "workspace-write"], ["codex", "read-only"], ["codex", null], ["opencode", "auto"], ["opencode", "other"], [null, "default"], [null, null], ["nobody", "default"], ["pi", "x"], ["cursor", "force"], ["cursor", "default"], ["gemini", "yolo"],
  ];
  const lists = catalog.catalogFromProbe(codex, codexProbe);
  const notRun: [string, unknown][] = [
    ["none", catalog.notRunRefused(undefined, "gpt-5.5")],
    ["codex lists, gpt-5.2", catalog.notRunRefused(lists, "gpt-5.2")?.message ?? null],
    ["codex lists, gpt-5.5", catalog.notRunRefused(lists, "gpt-5.5")?.message ?? null],
    ["codex lists, no model", catalog.notRunRefused(lists, undefined)?.message ?? null],
    ["codex lists, unknown", catalog.notRunRefused(lists, "gpt-9")?.message ?? null],
    ["codex table, gpt-5.2", catalog.notRunRefused(codex, "gpt-5.2")?.message ?? null],
  ];
  write("harness_catalog", {
    catalogs: catalog.HARNESS_CATALOGS,
    namedOnly: catalog.NAMED_ONLY,
    unknown: catalog.harnessCatalog("aider") ?? null,
    probes: probes.map(([name, value]) => ({ name, value })),
    labels: labels.map(l => [l, catalog.modelLabel(l)]),
    toolAsks: asks.map(([h, m]) => [h, m, catalog.toolAsks(h ?? undefined, m ?? undefined)]),
    smallest: [
      ["claude", catalog.smallestModel(claude) ?? null],
      ["codex", catalog.smallestModel(codex) ?? null],
      ["gemini", catalog.smallestModel(catalog.harnessCatalog("gemini")) ?? null],
      ["none", catalog.smallestModel(undefined) ?? null],
      ["codex+codexProbe", catalog.smallestModel(lists) ?? null],
    ],
    notRun,
  });
});

/** A machine that answers each command by the first rule whose needle it holds, recording every command. */
function machine(rules: [string, { exitCode: number; stdout: string; stderr: string }][] = [], fail?: string) {
  const ran: { cmd: string; timeoutMs?: number; stdin?: string }[] = [];
  return {
    ran,
    m: {
      id: "p",
      exec: async (cmd: string, o?: { timeoutMs?: number; stdin?: Uint8Array }) => {
        ran.push({ cmd, ...(o?.timeoutMs !== undefined ? { timeoutMs: o.timeoutMs } : {}), ...(o?.stdin !== undefined ? { stdin: Buffer.from(o.stdin).toString("utf8") } : {}) });
        if (fail !== undefined && cmd.includes(fail)) throw new Error("the link went");
        return rules.find(([needle]) => cmd.includes(needle))?.[1] ?? { exitCode: 0, stdout: "", stderr: "" };
      },
    },
  };
}
const ok = (stdout = "") => ({ exitCode: 0, stdout, stderr: "" });

it("project landing scripts", async () => {
  const gh = sources.projectSource("github");
  const git = sources.projectSource("git");
  const SHA = "1d2c3b4a5f6e7d8c9b0a1d2c3b4a5f6e7d8c9b0a";
  const lab = (branch: string, startSha?: string, repo = { owner: "acme", name: "lab" }, held = false) => ({ ofRecord: { repoId: 7, ...repo, branch, ...(startSha !== undefined ? { startSha } : {}) }, origin: "acme/lab", held });
  const o = { source: gh, remote: "https://github.com/acme/lab", checkout: "/root/lab", computer: "Boat", branch: "main" };
  write("landing_scripts", {
    cloneLines: [
      ["brokered", landing.cloneLines({ ...o, brokered: lab("wsp/pricing-page", SHA) })],
      ["brokered fork", landing.cloneLines({ ...o, brokered: lab("main", SHA, { owner: "ana", name: "lab-fork" }) })],
      ["brokered held own", landing.cloneLines({ ...o, brokered: lab("fix-typo", SHA, undefined, true) })],
      ["brokered held fork", landing.cloneLines({ ...o, brokered: lab("main", SHA, { owner: "ana", name: "lab-fork" }, true) })],
      ["brokered no sha", landing.cloneLines({ ...o, brokered: lab("wsp/x") })],
      ["brokered held no sha", landing.cloneLines({ ...o, brokered: lab("wsp/x", undefined, undefined, true) })],
      ["brokered no branch", landing.cloneLines({ source: gh, remote: o.remote, checkout: "/root/lab", computer: "Boat", brokered: lab("wsp/x", SHA) })],
      ["brokered case origin", landing.cloneLines({ ...o, brokered: { ...lab("b", SHA, { owner: "ACME", name: "Lab" }), origin: "acme/lab" } })],
      ["github", landing.cloneLines(o)],
      ["github no setup", landing.cloneLines({ ...o, setupGit: false })],
      ["github setup true", landing.cloneLines({ ...o, setupGit: true })],
      ["gitlab", landing.cloneLines({ source: sources.projectSource("gitlab"), remote: "https://gitlab.com/dev/thing.git", checkout: "/root/thing", computer: "hetzner" })],
      ["git", landing.cloneLines({ source: git, remote: "https://example.com/x.git", checkout: "/root/x", computer: "Boat" })],
      ["git branch", landing.cloneLines({ source: git, remote: "https://example.com/x.git", checkout: "/root/x y/it's", computer: "Boat", branch: "feat/a b" })],
      ["folder", landing.cloneLines({ source: sources.projectSource("folder"), remote: "git@github.com:a/b.git", checkout: "x", computer: "Boat" })],
      ["git root checkout", landing.cloneLines({ source: git, remote: "r", checkout: "/x", computer: "c" })],
    ],
    onRecord: [
      ["held clone", landing.onRecordLines({ checkout: "/srv/held", ...lab("wsp/pricing-page") })],
      ["sha", landing.onRecordLines({ checkout: "/srv/held", ...lab("wsp/pricing-page", SHA) })],
      ["fork held", landing.onRecordLines({ checkout: "/srv/held", ...lab("main", SHA, { owner: "ana", name: "lab-fork" }, true) })],
    ],
    gitEnv: [
      ["plain", landing.brokeredGitEnv("wsp-op-token")],
      ["public", landing.brokeredGitEnv("wsp-op-token", { owner: "acme", name: "web", public: true })],
      ["private", landing.brokeredGitEnv("wsp-op-token", { owner: "acme", name: "web" })],
      ["direct", landing.brokeredGitEnv("t", undefined, ["anthropics/claude-plugins", "acme/market"])],
      ["public and direct overlap", landing.brokeredGitEnv("t", { owner: "acme", name: "web", public: true }, ["acme/web", "z/y"])],
    ],
    cloneScript: [
      ["git with seed", landing.cloneScript({ source: git, remote: "/srv/origin.git", checkout: "/srv/checkout", computer: "spoo", seedTar: "/tmp/seed it.tgz" })],
      ["github no seed", landing.cloneScript({ ...o })],
    ],
    patchScript: landing.patchScript({ branch: "refactor/dashboard-polish", base: "abc123", back: "main", checkout: "/srv/co" }),
    patchCleanup: [
      ["own branch", landing.patchCleanupScript({ branch: "refactor/dashboard-polish", base: "abc123", back: "main", checkout: "/srv/co" })],
      ["same branch", landing.patchCleanupScript({ branch: "main", base: "abc123", back: "main", checkout: "/srv/co" })],
    ],
    unsavedScript: landing.unsavedScript("/srv/it's here"),
    seedRest: [
      ["memory", landing.seedRestScript({ checkout: "/srv/co", memoryDir: "/root/.claude-cfg/projects/-srv-co/memory", seedTar: "/tmp/seed.tgz", memory: true })],
      ["no memory", landing.seedRestScript({ checkout: "/srv/co", memoryDir: "/m", seedTar: "/tmp/seed.tgz", memory: false })],
      ["memory no slash", landing.seedRestScript({ checkout: "co", memoryDir: "mem", seedTar: "s.tgz", memory: true })],
    ],
    seedInto: landing.seedIntoScript({ checkout: "/srv/legacy", seedTar: "/root/.wsp/put/seed-pr_1.tgz" }),
    freeFolder: [
      ["root", landing.freeFolderScript("/root", "spoo-ts")],
      ["slashes", landing.freeFolderScript("/home/dev///", "it's")],
    ],
    underLoginHome: [["/home/dev", "/root/.claude-cfg"], ["/home/dev/", "/root"], ["/home/dev", "/rootless"], ["/home/dev", "/etc/x"], ["/", "/root/a"], ["/home/dev//", "/root/"]].map(([h, p]) => [h, p, landing.underLoginHome(h, p)]),
    guestMemoryDir: [["-srv-web", landing.guestMemoryDir("-srv-web")], ["", landing.guestMemoryDir("")]],
    memoryKeptMark: landing.MEMORY_KEPT_MARK,
    unsavedOf: [
      [{ exitCode: 0, stdout: "0 0 0\n", stderr: "" }],
      [{ exitCode: 0, stdout: "1 0 0\n", stderr: "" }],
      [{ exitCode: 0, stdout: "1 1 1\n", stderr: "" }],
      [{ exitCode: 0, stdout: "2 3 0\n", stderr: "" }],
      [{ exitCode: 0, stdout: "0 0 4\n", stderr: "" }],
      [{ exitCode: 0, stdout: "wsp-no-checkout\n", stderr: "" }],
      [{ exitCode: 1, stdout: "wsp-no-checkout\n", stderr: "" }],
      [{ exitCode: 1, stdout: "", stderr: "" }],
      [{ exitCode: 128, stdout: "", stderr: "fatal: bad\nfatal: worse\n" }],
      [{ exitCode: 0, stdout: "half\n", stderr: "" }],
      [{ exitCode: 0, stdout: "", stderr: "" }],
      [{ exitCode: 0, stdout: "1 2\n", stderr: "" }],
      [{ exitCode: 0, stdout: "1 -2 0\n", stderr: "" }],
      [{ exitCode: 0, stdout: "1.5 0 0\n", stderr: "" }],
      [{ exitCode: 0, stdout: " 1\t2  3 \n", stderr: "" }],
      [{ exitCode: 0, stdout: "noise\n0 0 0\n", stderr: "" }],
      [{ exitCode: 0, stdout: "0x1 0 0\n", stderr: "" }],
      [{ exitCode: 0, stdout: "1e1 0 0\n", stderr: "" }],
    ].map(([ran]) => [ran, landing.unsavedOf("web", ran) ?? null]),
    readUnsaved: [
      ["throws", await landing.readUnsaved("x", "/srv/x", () => Promise.reject(new Error("the fork did not wake")))],
      ["throws string", await landing.readUnsaved("x", "/srv/x", () => Promise.reject("plain"))],
      ["clean", await landing.readUnsaved("x", "/srv/x", async () => ok("0 0 0\n"))],
    ],
  });
});

it("project landing roads", async () => {
  const project = (over: Record<string, unknown> = {}) => ({ id: "pr_1", name: "spoo-ts", computer: "p", source: { kind: "git", url: "https://github.com/acme/lab.git" }, path: "", remote: "https://github.com/acme/lab.git", defaultBranch: "main", memoryKey: "", memoryDir: "", createdAt: "", ...over });
  const deps = (m: unknown, over: Record<string, unknown> = {}) => ({
    computer: { machine: m, home: "/root" },
    computerName: "hetzner",
    macStateHome: "/Users/dev/.claude",
    scratch: () => "/scratch",
    land: async (_m: unknown, path: string, bytes: Uint8Array) => landed.push({ path, bytes: Buffer.from(bytes).toString("utf8") }),
    branchHere: async (p: string) => (p === "/Users/dev/web" ? "main" : undefined),
    now: () => Date.parse("2026-10-25T10:00:00.000Z"),
    ...over,
  });
  let landed: { path: string; bytes: string }[] = [];
  const reports: [string, string][] = [];
  const report = (stage: string, message: string) => void reports.push([stage, message]);
  const box = landing.projectLanding("box");
  const provider = landing.projectLanding("provider");
  const mac = landing.projectLanding("mac");
  const folderSource = { kind: "folder", path: "/srv/legacy" };
  const gitSource = { kind: "git", url: "https://github.com/acme/lab.git" };
  const ghSource = { kind: "github", repo: "acme/lab" };
  const none = machine();

  const plain = machine([["mkdir '/root/spoo-ts'", ok("/root/spoo-ts\n")], ["ls -A", ok("package-lock.json\nREADME.md\n")]]);
  const plainLanded = await tryAsync(() => box.land({ project: project(), source: sources.projectSource("git"), report }, deps(plain.m, { cloneEnv: () => ({}) })));
  const plainReports = reports.splice(0);

  const vault = machine([["mkdir '/root/spoo-ts'", ok("/root/spoo-ts-2\n")], ["ls -A", ok("Cargo.lock\npnpm-lock.yaml\n")]]);
  const vaultLanded = await tryAsync(() => box.land({ project: project({ base: "dev" }), source: sources.projectSource("github"), report }, deps(vault.m, { cloneEnv: () => ({ GH_TOKEN: "gho_x" }) })));
  const vaultReports = reports.splice(0);

  const seedPlan = { source: "/Users/dev/web", remote: "https://github.com/acme/lab.git", branch: "refactor/x", defaultBranch: "main", unpushed: { commits: 2, base: "abc123" }, uncommitted: 0, memory: { key: "-Users-dev-web", files: 3, bytes: 20 }, files: [{ path: ".env.local", dir: false, bytes: 12, kind: "config", row: { id: "next", name: "Next" }, ticked: true }], remembered: false };
  const seeded = machine([["mkdir '/root/spoo-ts'", ok("/root/spoo-ts\n")], ["symbolic-ref", ok("main\n")], ["am --3way", ok("x\n2\n")], ["rmdir", ok("wsp-memory-kept\n")]]);
  landed = [];
  const seededLanded = await tryAsync(() => box.land({ project: project({ source: { kind: "folder", path: "/Users/dev/web" } }), source: sources.projectSource("folder"), seed: { tar: Buffer.from("seed archive"), choice: { files: [".env.local"], memory: true, commits: true }, plan: seedPlan }, report }, deps(seeded.m)));
  const seededReports = reports.splice(0);
  const seededLand = landed.splice(0);

  const refused = machine([["mkdir '/root/spoo-ts'", ok("/root/spoo-ts\n")], ["symbolic-ref", { exitCode: 128, stdout: "", stderr: "fatal: not a symbolic ref\n" }], ["am --3way", { exitCode: 1, stdout: "", stderr: "error: patch failed\n" }]]);
  const refusedLanded = await tryAsync(() => box.land({ project: project({ source: { kind: "folder", path: "/Users/dev/web" }, base: "dev" }), source: sources.projectSource("folder"), seed: { tar: Buffer.from("t"), choice: { files: [], memory: false, commits: true }, plan: seedPlan }, report }, deps(refused.m)));
  const refusedReports = reports.splice(0);

  const cloneFails = machine([["mkdir '/root/spoo-ts'", ok("/root/spoo-ts\n")], ["git clone", { exitCode: 128, stdout: "", stderr: "Cloning...\nfatal: repository not found\n" }]]);
  const cloneFailed = await tryAsync(() => box.land({ project: project(), source: sources.projectSource("git"), report }, deps(cloneFails.m)));
  reports.splice(0);

  const installFails = machine([["mkdir '/root/spoo-ts'", ok("/root/spoo-ts\n")], ["ls -A", ok("package-lock.json\n")], ["npm ci", { exitCode: 1, stdout: "", stderr: "npm ERR! nope\n" }]]);
  const installFailed = await tryAsync(() => box.land({ project: project(), source: sources.projectSource("git"), report }, deps(installFails.m)));
  reports.splice(0);

  const noFolder = machine([["mkdir '/root/spoo-ts'", { exitCode: 1, stdout: "", stderr: "every name from spoo-ts to spoo-ts-99 is taken in /root\n" }]]);
  const noFolderLanded = await tryAsync(() => box.land({ project: project(), source: sources.projectSource("git"), report }, deps(noFolder.m)));

  const sweepFails = machine([["mkdir '/root/spoo-ts'", ok("/root/spoo-ts\n")], ["git clone", { exitCode: 1, stdout: "", stderr: "" }]], "rm -rf");
  const sweepFailed = await tryAsync(() => box.land({ project: project(), source: sources.projectSource("git"), report }, deps(sweepFails.m)));
  reports.splice(0);

  const stands = (code: number) => machine([["test -e", { exitCode: code, stdout: "", stderr: code > 1 ? "test: broke\n" : "" }]]);
  const branchRead = (r: { exitCode: number; stdout: string; stderr: string } | "throw") => machine(r === "throw" ? [] : [["symbolic-ref", r]], r === "throw" ? "symbolic-ref" : undefined);
  const intoOk = machine();
  landed = [];
  await box.seedInto!(project({ path: "/srv/legacy" }), Buffer.from("tar"), deps(intoOk.m, { computer: { machine: intoOk.m, home: "/home/dev" } }));
  const intoLand = landed.splice(0);
  const intoFails = machine([["tar -xzf", { exitCode: 2, stdout: "", stderr: "tar: broke\n" }]]);
  const unsavedRead = machine([["rev-parse --git-dir", ok("1 0 0\n")]]);

  write("landing_roads", {
    kinds: [...landing.PROJECT_LANDINGS.keys()],
    unknown: tryIt(() => landing.projectLanding("moon")),
    path: [
      ["box folder", box.path({ name: "legacy", source: folderSource, deps: deps(none.m) })],
      ["box git", box.path({ name: "lab", source: gitSource, deps: deps(none.m, { computer: { machine: none.m, home: "/home/dev/" } }) })],
      ["box no computer", tryIt(() => box.path({ name: "lab", source: gitSource, deps: deps(none.m, { computer: undefined }) }))],
      ["provider git", provider.path({ name: "lab", source: gitSource, deps: deps(none.m) })],
      ["provider folder", provider.path({ name: "legacy", source: folderSource, deps: deps(none.m) })],
      ["mac folder", mac.path({ name: "legacy", source: folderSource, deps: deps(none.m) })],
      ["mac github", tryIt(() => mac.path({ name: "lab", source: ghSource, deps: deps(none.m) }))],
    ],
    places: [
      ["box", box.places({ project: project(), memoryKey: "-root-lab", deps: deps(none.m, { computer: { machine: none.m, home: "/home/dev" } }) })],
      ["provider", provider.places({ project: project(), memoryKey: "-root-lab", deps: deps(none.m) })],
      ["mac folder", mac.places({ project: project({ source: folderSource, path: "/elsewhere" }), memoryKey: "-srv-legacy", deps: deps(none.m) })],
      ["mac git", mac.places({ project: project({ path: "/Users/dev/lab" }), memoryKey: "-Users-dev-lab", deps: deps(none.m) })],
    ],
    landsAtAdd: [box.landsAtAdd({ seeding: false }), provider.landsAtAdd({ seeding: true }), mac.landsAtAdd({ seeding: true })],
    refusal: [box.refusal(project(), deps(none.m)) ?? null, provider.refusal(project(), deps(none.m)) ?? null, mac.refusal(project(), deps(none.m)) ?? null],
    remove: [await box.remove(project({ path: "/root/spoo-ts" }), deps(none.m)), await provider.remove(project(), deps(none.m)), await mac.remove(project(), deps(none.m))],
    binds: [box.workspaceBinds(project()), provider.workspaceBinds(project()), mac.workspaceBinds(project())],
    optional: [["box", !!box.folderStands, !!box.seedInto, !!box.unsaved], ["provider", !!provider.folderStands, !!provider.seedInto, !!provider.unsaved], ["mac", !!mac.folderStands, !!mac.seedInto, !!mac.unsaved]],
    branch: [
      ["provider base", await provider.branch(project({ base: "dev" }), deps(none.m))],
      ["provider default", await provider.branch(project(), deps(none.m))],
      ["mac here", await mac.branch(project({ path: "/Users/dev/web" }), deps(none.m))],
      ["mac detached", await mac.branch(project({ path: "/Users/dev/other" }), deps(none.m))],
      ["box main", await box.branch(project({ path: "/root/x" }), deps(branchRead(ok(" main \n")).m))],
      ["box detached", await box.branch(project({ path: "/root/x" }), deps(branchRead({ exitCode: 1, stdout: "", stderr: "" }).m))],
      ["box empty", await box.branch(project({ path: "/root/x" }), deps(branchRead(ok("\n")).m))],
      ["box throws", await box.branch(project({ path: "/root/x" }), deps(branchRead("throw").m))],
      ["box no computer", await box.branch(project({ path: "/root/x" }), deps(none.m, { computer: undefined }))],
    ],
    folderStands: await Promise.all([0, 1, 2].map(async c => [c, await tryAsync(() => box.folderStands!(project({ path: "/srv/it's" }), deps(stands(c).m)))])),
    folderStandsCmd: stands(0).m && (await (async () => { const s = stands(0); await box.folderStands!(project({ path: "/srv/it's" }), deps(s.m)); return s.ran; })()),
    seedInto: { ran: intoOk.ran, land: intoLand, fails: await tryAsync(() => box.seedInto!(project({ path: "/srv/legacy" }), Buffer.from("t"), deps(intoFails.m))) },
    unsaved: [
      ["no checkout", (await box.unsaved!(project({ path: "/root/x" }), deps(none.m))) ?? null],
      ["checkout", (await box.unsaved!(project({ name: "web", path: "/root/x", checkout: "/root/web-2" }), deps(unsavedRead.m))) ?? null, unsavedRead.ran],
    ],
    land: {
      folderNoSeed: { out: await tryAsync(() => box.land({ project: project({ source: folderSource, path: "/srv/legacy" }), source: {} as never, report }, deps(none.m))), ran: none.ran },
      providerLand: await provider.land({ project: project(), source: sources.projectSource("git"), report }, deps(none.m)),
      macLand: await mac.land({ project: project(), source: sources.projectSource("git"), report }, deps(none.m)),
      plain: { out: plainLanded, ran: plain.ran, reports: plainReports },
      vault: { out: vaultLanded, ran: vault.ran, reports: vaultReports },
      seeded: { out: seededLanded, ran: seeded.ran, reports: seededReports, land: seededLand },
      refused: { out: refusedLanded, ran: refused.ran, reports: refusedReports },
      cloneFailed: { out: cloneFailed, ran: cloneFails.ran },
      installFailed: { out: installFailed, ran: installFails.ran },
      noFolder: { out: noFolderLanded, ran: noFolder.ran },
      sweepFailed: { out: sweepFailed, ran: sweepFails.ran },
      noComputer: await tryAsync(() => box.land({ project: project(), source: sources.projectSource("git"), report }, deps(none.m, { computer: undefined }))),
    },
  });
});

it("project sources", async () => {
  const folderDeps = (seeding: boolean, plan: Record<string, unknown>, remote = { remote: "", defaultBranch: "" }) => ({ seeding, seedPlan: async () => plan, folderRemote: async () => remote });
  const plan = (over: Record<string, unknown>) => ({ source: "/Users/dev/web", remote: "git@github.com:acme/web.git", branch: "feat", defaultBranch: "main", unpushed: null, uncommitted: 0, memory: null, files: [], remembered: false, ...over });
  const folder = sources.projectSource("folder");
  const src = { kind: "folder", path: "/Users/dev/web" };
  write("project_sources", {
    kinds: [...sources.PROJECT_SOURCES.keys()],
    unknown: tryIt(() => sources.projectSource("svn")),
    defaultBranch: sources.DEFAULT_BRANCH,
    remote: [
      [{ kind: "folder", path: "/x" }, sources.projectRemote({ kind: "folder", path: "/x" })],
      [{ kind: "git", url: "ssh://git@host/x.git" }, sources.projectRemote({ kind: "git", url: "ssh://git@host/x.git" })],
      [{ kind: "github", repo: "acme/web" }, sources.projectRemote({ kind: "github", repo: "acme/web" })],
      [{ kind: "gitlab", repo: "dev/thing" }, sources.projectRemote({ kind: "gitlab", repo: "dev/thing" })],
    ],
    resolve: [
      ["folder here", await tryAsync(() => folder.resolve(src, folderDeps(false, plan({}), { remote: "https://github.com/acme/web.git", defaultBranch: "trunk" })))],
      ["folder here none", await tryAsync(() => folder.resolve(src, folderDeps(false, plan({}))))],
      ["folder seeding ssh", await tryAsync(() => folder.resolve(src, folderDeps(true, plan({}))))],
      ["folder seeding https", await tryAsync(() => folder.resolve(src, folderDeps(true, plan({ remote: "https://github.com/acme/web.git", defaultBranch: null }))))],
      ["folder seeding gitlab ssh", await tryAsync(() => folder.resolve(src, folderDeps(true, plan({ remote: "git@gitlab.com:dev/thing.git", defaultBranch: "" }))))],
      ["folder seeding other host", await tryAsync(() => folder.resolve(src, folderDeps(true, plan({ remote: "git@example.com:a/b.git", defaultBranch: undefined, branch: "" }))))],
      ["folder seeding no remote", await tryAsync(() => folder.resolve(src, folderDeps(true, plan({ remote: null }))))],
      ["folder seeding http github", await tryAsync(() => folder.resolve(src, folderDeps(true, plan({ remote: "http://github.com/acme/web" }))))],
      ["git", await sources.projectSource("git").resolve({ kind: "git", url: "https://example.com/team/thing.git" }, folderDeps(true, plan({})))],
      ["github", await sources.projectSource("github").resolve({ kind: "github", repo: "acme/web" }, folderDeps(true, plan({})))],
      ["gitlab", await sources.projectSource("gitlab").resolve({ kind: "gitlab", repo: "dev/thing" }, folderDeps(true, plan({})))],
    ],
    clone: [
      ["git", sources.projectSource("git").cloneCommand({ remote: "https://example.com/x.git", dest: "/root/x" })],
      ["git branch", sources.projectSource("git").cloneCommand({ remote: "https://example.com/x.git", dest: "/root/a b", branch: "feat/x" })],
      ["folder", folder.cloneCommand({ remote: "--upload-pack=evil", dest: "/root/x" })],
      ["github", sources.projectSource("github").cloneCommand({ remote: "https://github.com/acme/web.git", dest: "/root/web" })],
      ["github branch ignored", sources.projectSource("github").cloneCommand({ remote: "https://github.com/acme/web.git", dest: "/root/web", branch: "dev" })],
      ["github not a repo url", sources.projectSource("github").cloneCommand({ remote: "weird remote", dest: "/root/web" })],
      ["gitlab", sources.projectSource("gitlab").cloneCommand({ remote: "https://gitlab.com/dev/thing.git", dest: "/root/thing" })],
    ],
    cli: ["folder", "git", "github", "gitlab"].map(k => {
      const cli = sources.projectSource(k).cli;
      return [k, cli === undefined ? null : { bin: cli.bin, setupGit: cli.setupGit, missing: cli.missing("hetzner") }];
    }),
  });
});

it("recipe sync", () => {
  const P = protocol.RecipeFile;
  const BEFORE = P.parse({ name: "laptop", agents: { claude: { signin: "vault" } }, clis: { jq: { via: "brew" } }, skills: { unslop: { from: "~/.claude/skills" } }, mcp: { linear: { agents: ["claude"] } }, configs: { git: {} } });
  const ITEMS = { "agents/claude": "a1", "clis/jq": "1.7", "skills/unslop": "s1", "mcp/linear/claude": "m1", "configs/git": "g1" };
  const cases: [string, unknown, unknown, unknown, unknown][] = [
    ["nothing moved", BEFORE, ITEMS, BEFORE, ITEMS],
    ["first apply", undefined, undefined, BEFORE, ITEMS],
    ["first apply with items before", undefined, ITEMS, BEFORE, {}],
    ["no items kept", BEFORE, undefined, BEFORE, ITEMS],
    ["no items kept, none now", BEFORE, undefined, BEFORE, {}],
    ["mixed", BEFORE, ITEMS, P.parse({ ...BEFORE, agents: { claude: { signin: "machine" } }, skills: { why: { from: "~/.claude/skills" } }, plugins: { "lint@acme": {} } }), { ...ITEMS, "clis/jq": "1.8", "skills/why": "w1" }],
    ["keys yes", BEFORE, ITEMS, P.parse({ ...BEFORE, copyKeys: true }), ITEMS],
    ["keys no", P.parse({ ...BEFORE, copyKeys: true }), ITEMS, BEFORE, ITEMS],
    ["keys yes, first", undefined, undefined, P.parse({ ...BEFORE, copyKeys: true }), ITEMS],
    ["item prefix only", BEFORE, ITEMS, BEFORE, { ...ITEMS, "mcp/linear2": "x", "mcp/linearx/claude": "y" }],
    ["item sub key", BEFORE, ITEMS, BEFORE, { ...ITEMS, "mcp/linear/codex": "m2" }],
    ["item dropped", BEFORE, ITEMS, BEFORE, { "agents/claude": "a1" }],
    ["all removed", BEFORE, ITEMS, P.parse({ name: "laptop" }), {}],
    ["agent added", BEFORE, ITEMS, P.parse({ ...BEFORE, agents: { ...BEFORE.agents, codex: { signin: "machine" } } }), ITEMS],
    ["cli needs", BEFORE, ITEMS, P.parse({ ...BEFORE, clis: { ...BEFORE.clis, nextest: { via: "cargo", needs: ["build-essential"] } } }), ITEMS],
    ["cli no needs", BEFORE, ITEMS, P.parse({ ...BEFORE, clis: { ...BEFORE.clis, rg: { via: "brew" } } }), ITEMS],
    ["cli changed needs", BEFORE, ITEMS, P.parse({ ...BEFORE, clis: { jq: { via: "brew", needs: ["x"] } } }), ITEMS],
    ["github config", BEFORE, ITEMS, P.parse({ ...BEFORE, configs: { git: {}, github: { signin: "vault" } } }), ITEMS],
    ["folder", BEFORE, ITEMS, P.parse({ ...BEFORE, folders: { app: { from: "~/code/app", keep: [] } } }), ITEMS],
    ["agent edited", BEFORE, ITEMS, BEFORE, { ...ITEMS, "agents/claude": "a2" }],
    ["cli removed", BEFORE, ITEMS, P.parse({ ...BEFORE, clis: {} }), ITEMS],
    ["agent removed", BEFORE, ITEMS, P.parse({ ...BEFORE, agents: {} }), ITEMS],
    ["config removed", BEFORE, ITEMS, P.parse({ ...BEFORE, configs: {} }), ITEMS],
  ];
  write("recipe_sync", {
    kinds: protocol.RECIPE_KINDS,
    cases: cases.map(([name, before, beforeItems, after, afterItems]) => {
      const changes = recipe.recipeChanges(before, beforeItems, after, afterItems);
      return { name, before: before ?? null, beforeItems: beforeItems ?? null, after, afterItems, changes, steps: [...recipe.stepsFor(changes, after)] };
    }),
    stepsOnly: [
      ["removed mcp and clis", [...recipe.stepsFor([{ key: "mcp/a", kind: "mcp", name: "a", how: "removed" }, { key: "clis/b", kind: "clis", name: "b", how: "removed" }, { key: "skills/c", kind: "skills", name: "c", how: "removed" }], BEFORE)]],
      ["added cli not in after", [...recipe.stepsFor([{ key: "clis/zz", kind: "clis", name: "zz", how: "added" }], BEFORE)]],
      ["order", [...recipe.stepsFor([{ key: "folders/f", kind: "folders", name: "f", how: "added" }, { key: "plugins/p", kind: "plugins", name: "p", how: "edited" }, { key: "configs/x", kind: "configs", name: "x", how: "changed" }], BEFORE)]],
      ["none", [...recipe.stepsFor([], BEFORE)]],
    ],
  });
});

it("agent setups and adapters", () => {
  const vaults: [string, Record<string, string>, string, boolean][] = [
    ["claude token", { CLAUDE_CODE_OAUTH_TOKEN: "tok", ANTHROPIC_API_KEY: "key" }, "claude", false],
    ["claude key", { ANTHROPIC_API_KEY: "key" }, "claude", false],
    ["claude login stands", { CLAUDE_CODE_OAUTH_TOKEN: "tok" }, "claude", true],
    ["claude empty", {}, "claude", false],
    ["claude empty token", { CLAUDE_CODE_OAUTH_TOKEN: "" }, "claude", false],
    ["codex key", { OPENAI_API_KEY: "sk", CODEX_API_KEY: "ck" }, "codex", false],
    ["codex token", { CODEX_OAUTH_TOKEN: "t" }, "codex", false],
    ["opencode", { OPENROUTER_API_KEY: "or", OPENAI_API_KEY: "sk", ANTHROPIC_API_KEY: "a" }, "opencode", false],
    ["cursor", { CURSOR_API_KEY: "key_x" }, "cursor", false],
    ["cursor stands", { CURSOR_API_KEY: "key_x" }, "cursor", true],
    ["unknown", { X: "y" }, "aider", false],
    ["gemini", { GEMINI_API_KEY: "g" }, "gemini", false],
  ];
  const homes = ["/srv/x/../y/./claude", "relative/dir", "/", "/a//b/", "/it's here"];
  write("adapters_setups", {
    secrets: vaults.map(([name, vault, id, stands]) => ({ name, vault, id, stands, secrets: adapters.secretsOf(vault, id, stands), env: adapters.secretEnvOf(vault, id, stands) })),
    adapters: Object.keys(adapters.HARNESS_ADAPTERS),
    brokered: [...adapters.BROKERED_AGENTS],
    planResets: Object.keys(adapters.PLAN_RESETS),
    logLimits: Object.keys(adapters.LOG_LIMITS),
    agentSetups: setup.AGENT_SETUPS,
    keyOf: setup.keyOf("here", "claude"),
    realFolderScript: homes.map(p => [p, setup.realFolderScript(p)]),
  });
});

it("agent setups store and the protocol helpers it reads", async () => {
  const kept = { program: "/opt/claude", configDir: "/root/c", args: ["--x"], env: { A: "1", B: "2" } };
  const patched: [string, unknown, unknown][] = [
    ["empty over nothing", undefined, {}],
    ["on false", undefined, { on: false }],
    ["on true clears", { on: false, program: "/p" }, { on: true }],
    ["program null clears", kept, { program: null }],
    ["env set and drop", kept, { env: { A: null, C: "3" } }],
    ["env drop all", { env: { A: "1" } }, { env: { A: null } }],
    ["config and args", undefined, { configDir: "/root/c", args: ["-v", "x"] }],
    ["args null", kept, { args: null, configDir: null, program: null, env: { A: null, B: null } }],
    ["keep all", kept, {}],
    ["order", { env: { Z: "z" }, args: ["a"], configDir: "/c", program: "/p", on: false }, { env: { A: "a" } }],
  ];
  const strip = claudeAdapter.ENV_STRIP_PATTERNS;
  const names = ["FOO", "wsp_token", "WSP_X", "PATH", "path", "Home", "LD_PRELOAD", "ld_library_path", "DYLD_X", "NODE_OPTIONS", "GIT_SSH_COMMAND", "CLAUDE_CODE_USE_BEDROCK", "CLAUDECODE", "FORCE_CODE_TERMINAL", "claude_code_x", "ANTHROPIC_API_KEY", "IFS", "ENV", "ENVIRONMENT"];
  const real = (folder: string, home = "/home/dev", kept: string[] = ["/home/dev/.wsp"]) => ({ folder, home, kept });
  const dirs: [string, string, unknown][] = [
    ["home", "~", real("/home/dev")],
    ["home slash", "~/", real("/home/dev/", "/home/dev//")],
    ["under", "/home/dev/c", real("/home/dev/c")],
    ["outside", "/etc/c", real("/etc/c")],
    ["prefix sibling", "/home/devil/c", real("/home/devil/c")],
    ["kept", "/home/dev/.wsp/x", real("/home/dev/.wsp/x")],
    ["kept itself", "/home/dev/.wsp", real("/home/dev/.wsp/")],
    ["kept prefix sibling", "/home/dev/.wspx", real("/home/dev/.wspx")],
    ["kept slash", "/home/dev/.wsp/y", real("/home/dev/.wsp/y", "/home/dev", ["/home/dev/.wsp///"])],
  ];
  const codex = catalog.harnessCatalog("codex");
  const lists = (version: string | null, models: string[]) => ({ ...codex, source: "harness", version, models: models.map(v => ({ value: v, label: v.toUpperCase() })), legacyModels: [] });
  const notRun: [string, unknown, unknown, unknown, unknown][] = [
    ["table lists", codex, codex, "gpt-5.2", "npm i -g @openai/codex"],
    ["older binary", lists("0.150.0", ["gpt-5.5"]), codex, "gpt-5.6-sol", "npm i -g @openai/codex"],
    ["older, no update", lists("0.150.0", ["gpt-5.5"]), codex, "gpt-5.6-sol", undefined],
    ["newer binary", lists("0.160.0", ["gpt-6", "gpt-6-mini"]), codex, "gpt-5.2", "npm i -g @openai/codex"],
    ["listed", lists("0.160.0", ["gpt-5.2"]), codex, "gpt-5.2", "u"],
    ["unknown model", lists("0.160.0", ["gpt-6"]), codex, "gpt-9", "u"],
    ["no version", lists(null, ["gpt-6"]), codex, "gpt-5.2", "u"],
    ["no model", lists("0.160.0", ["gpt-6"]), codex, undefined, "u"],
    ["no table", lists("0.160.0", ["gpt-6"]), undefined, "gpt-5.2", "u"],
    ["same version", lists("0.153.0", ["gpt-6"]), codex, "gpt-5.2", "u"],
    ["prerelease", lists("0.153.0-alpha.1", ["gpt-6"]), codex, "gpt-5.2", "u"],
    ["unreadable pin", lists("0.1.0", ["gpt-6"]), { ...codex, version: "app-server x" }, "gpt-5.2", "u"],
  ];
  const err = (e: Error | null) => (e === null ? null : { message: e.message, fix: (e as { fix?: string }).fix ?? null, kind: (e as { kind?: string }).kind ?? null });

  const mem = store.memoryStore();
  const s = setup.agentSetups(mem);
  await s.load();
  const folder = async (p: string) => ({ folder: p === "/home/dev/link" ? "/home/dev/real" : p, home: "/home/dev", kept: ["/home/dev/.wsp"] });
  const o = { folder, agentName: "Claude Code" };
  const steps: unknown[] = [];
  const step = async (name: string, f: () => Promise<unknown>) => steps.push({ name, out: await tryAsync(f), doc: (await mem.get("agentSetups", "here:claude")) ?? null, keys: await mem.keys("agentSetups") });
  await step("set env", () => s.set("here", "claude", { env: { FOO: "bar", CLAUDE_CODE_X: null } }, o));
  await step("refused env", () => s.set("here", "claude", { env: { PATH: "/x" } }, o));
  await step("refused dropped", () => s.set("here", "claude", { env: { CLAUDE_CODE_USE_BEDROCK: "1" } }, o));
  await step("relative dir", () => s.set("here", "claude", { configDir: "rel/c" }, o));
  await step("dir through link", () => s.set("here", "claude", { configDir: "/home/dev/link" }, o));
  await step("dir is home", () => s.set("here", "claude", { configDir: "/home/dev" }, o));
  await step("program and args", () => s.set("here", "claude", { program: "/opt/claude", args: ["--verbose"] }, o));
  await step("off", () => s.set("here", "claude", { on: false }, o));
  const launchClaude = s.launchOf("here", "claude");
  const offClaude = s.off("here", "claude");
  await step("codex dir refused", () => s.set("here", "codex", { configDir: "/home/dev/cx" }, { folder, agentName: "Codex" }));
  await step("cursor no dir", () => s.set("here", "cursor", { configDir: "/home/dev/cu" }, { folder, agentName: "Cursor" }));
  await step("back to nothing", () => s.set("here", "claude", { on: true, program: null, args: null, configDir: null, env: { FOO: null } }, o));
  await mem.put("agentSetups", "box:claude", { configDir: "/root/cfg", env: { PATH: "/kept/before", GOOD: "1", CLAUDECODE: "1" } });
  const s2 = setup.agentSetups(mem);
  await s2.load();
  write("agent_setups", {
    patched: patched.map(([name, k, set]) => ({ name, kept: k ?? null, set, out: protocol.setupPatched(k, set) ?? null })),
    envRefusal: names.flatMap(n => [[n, "claude", true, protocol.agentEnvRefusal(n, "Claude Code", strip)], [n, "codex", false, protocol.agentEnvRefusal(n, "Codex")]]),
    configDirRefusal: dirs.map(([name, given, r]) => [name, given, r, protocol.configDirRefusal("Claude Code", given, r)]),
    noConfigDirLine: protocol.noConfigDirLine("Cursor"),
    notRun: notRun.map(([name, l, t, m, u]) => [name, err(protocol.notRunRefusal(l, t, m, u))]),
    stateHomeEnv: ["claude", "codex", "opencode", "cursor"].map(a => [a, catalogPkg.CATALOG_AGENTS.find((r: { id: string }) => r.id === a)?.stateHomeEnv ?? null]),
    steps,
    launchClaude,
    offClaude,
    afterLoad: { get: s2.get("box", "claude") ?? null, launch: s2.launchOf("box", "claude"), codexLaunch: s2.launchOf("box", "codex"), off: s2.off("box", "claude"), none: s2.get("nowhere", "claude") ?? null },
  });
});
