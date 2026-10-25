// SPDX-License-Identifier: AGPL-3.0-only
// The TS engine's own answers for the rest slice (project state's scripts, the Solari create body, the shared brew
// step, the login lines, the brew and du parses, ages, token expiries, fingerprints and a sealed vault), written as
// tests/fixtures/oracle/*.json for the Rust hard tests (spec/*.md names which). Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 rest.oracle
import { generateKeyPairSync } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { CATALOG_AGENTS } from "../../../../packages/catalog/src/index.js";
import type { SealedImage } from "../../../../packages/protocol/src/index.js";
import { PY_PREAMBLE, pyData } from "../../../../packages/engine/src/project-state/py.js";
import { ROOTS_STEP, filterStep, keyedStep, listScript, parseStateListing, storeCopy } from "../../../../packages/engine/src/project-state/listing.js";
import { jsonlCwdStep, mergeScript, parseMergeOutput } from "../../../../packages/engine/src/project-state/merge.js";
import { movedColumn, sqliteFilter, sqliteMergeStep, underPath, type MergeTable } from "../../../../packages/engine/src/project-state/sqlite.js";
import { PROJECT_STATE_RESOLVERS, agentHome, agentHomes, guestAgentHomes, stateListing } from "../../../../packages/engine/src/project-state/index.js";
import { insideFolder, movedPath, underHome } from "../../../../packages/engine/src/project-state/resolver.js";
import { piProjectKey } from "../../../../packages/engine/src/project-state/pi.js";
import { brewSharedCheck, brewSharedDeps } from "../../../../packages/engine/src/brew-shared.js";
import { asLogin, loginShellPath, targetLogin, type TargetLogin } from "../../../../packages/engine/src/target-line.js";
import { assumedSize, brewTable, parseBrewInfo, parseDu, sourceOf } from "../../../../packages/engine/src/tool-sizes.js";
import { SolariBackend, previewTokenExpiry } from "../../../../packages/engine/src/solari-backend.js";
import { describeAge } from "../../../../packages/engine/src/orphans.js";
import { keyFingerprint } from "../../../../packages/engine/src/key-fingerprint.js";
import { publicKeyCipher } from "../../../../packages/engine/src/vault-cipher.js";
import { missingCommands } from "../../../../packages/engine/src/golden-tools.js";
import { envExec, envInput } from "../../../../packages/engine/src/run-env.js";
import { INSTALL_STORES, checkScripts, installScript, projectInstalls } from "../../../../packages/engine/src/project-install.js";
import { goldenName, nameOwner, projectSnapshotName } from "../../../../packages/engine/src/snapshot-names.js";
import { diskUseOf } from "../../../../packages/engine/src/disk-sync.js";
import { lineFeed } from "../../../../packages/engine/src/child-exec.js";
import type { Machine } from "../../../../packages/engine/src/machine.js";

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const write = (name: string, value: unknown): void => {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, name), `${JSON.stringify(value, null, 1)}\n`);
};
const thrown = (f: () => unknown): { ok: unknown } | { error: string } => {
  try {
    return { ok: f() };
  } catch (e) {
    return { error: (e as Error).message };
  }
};

/** A machine whose exec records the command and answers these lines. */
const recording = (stdout: string, exitCode = 0) => {
  const ran: string[] = [];
  const machine = { exec: async (cmd: string) => (ran.push(cmd), { exitCode, stdout, stderr: "" }) } as unknown as Machine;
  return { ran, machine };
};

it("project state scripts", () => {
  // JSON as the TS reads it: the input is JSON text, parsed by JSON.parse, so integer-like keys already lead.
  const values = [
    '"plain"',
    '"/Users/ada/my project/é 日本 🎉"',
    '"tab\\tquote\\"backslash\\\\nl\\ncr\\rnul\\u0000del\\u007fls\\u2028"',
    "1",
    "1.5",
    "-0",
    "1e21",
    "0.30000000000000004",
    "123456789012345680000",
    "null",
    "true",
    "[]",
    "{}",
    '{"b":1,"2":2,"1":3,"a":{"10":1,"9":2}}',
    '[["/a","/b","slug"],{"x":null}]',
  ];
  const pyDataCases = values.map(text => ({ text, py: pyData(JSON.parse(text)) }));
  const resolvers = [...PROJECT_STATE_RESOLVERS.values()].map(r => ({
    agent: r.agent,
    states: r.states,
    roots: r.roots,
    carry: r.carry,
    key: r.key === undefined ? null : r.key("/Users/ada/b_2.x"),
    listing: r.listing === undefined ? null : r.listing("/root/.agent home", "/Users/ada/b_2.x"),
    shared: r.shared ?? null,
    hasMerge: r.merge !== undefined,
  }));
  const homes = { claude: "/root/.claude", codex: "/root/.codex", gemini: "/root/.gemini", hermes: "/root/.hermes", opencode: "/root/.local/share/opencode", pi: "/root/.pi/agent", cursor: "/root/.cursor" };
  const stateListings = [
    { ids: null, homes, script: stateListing(homes, "/Users/ada/b_2.x", "/tmp/wsp-scratch") },
    { ids: ["codex", "claude"], homes, script: stateListing(homes, "/Users/ada/b_2.x", "/tmp/wsp-scratch", ["codex", "claude"]) },
    { ids: [], homes, script: stateListing(homes, "/Users/ada/b_2.x", "/tmp/wsp-scratch", []) },
    { ids: ["cursor"], homes, script: stateListing(homes, "/Users/ada/b_2.x", "/tmp/wsp-scratch", ["cursor"]) },
    { ids: null, homes: { cursor: "/root/.cursor" }, script: stateListing({ cursor: "/root/.cursor" }, "/p", "/s") },
  ];
  const unknownAgent = thrown(() => stateListing(homes, "/p", "/s", ["claude", "nope"]));
  const tables: MergeTable[] = [
    {
      table: "threads",
      key: ["id"],
      set: ["cwd", "rollout_path"],
      rows: [
        { id: "t1", cwd: "/root/work/b_2.x", n: 7, r: 1.5, whole: 1, none: null, blob: new Uint8Array([0, 1, 254, 255]) },
        { id: "t2", cwd: "/root/work/b_2.x/sub", n: -3, r: 0.1, whole: 2, none: null, blob: new Uint8Array([]) },
      ],
    },
    { table: "messages", by: "session_id", drop: ["id"], rows: [{ id: 9, session_id: "s1", body: "hé\nllo" }] },
  ];
  write("project-state-scripts.json", {
    pyPreamble: PY_PREAMBLE,
    rootsStep: ROOTS_STEP,
    pyData: pyDataCases,
    listScript: listScript("claude", "/root/.claude", "/Users/ada/b_2.x/", ["projects", "a/../b"], ["say(1)", "for x in y:\n    say(x)"]),
    listScriptNoSteps: listScript("pi", "/h", "/p", [], [ROOTS_STEP]),
    keyedStep: keyedStep("/root/.claude/projects", "lambda p: p"),
    filterStep: filterStep("/tmp/wsp-scratch", "/root/.codex", { store: "state_5.sqlite", filter: ["FILTERED = 1", "if x:\n    y"] }, ["say(COPY)"]),
    filterStepNoSteps: filterStep("/s", "/h", { store: "s.db", filter: [] }, []),
    storeCopy: [
      ["/tmp/s", "/root/.codex", "state_5.sqlite"],
      ["/tmp/s/", "/root/.codex/", "x/"],
      ["/tmp/s", "/root/../a/./b", "c"],
      ["", "", ""],
      ["rel", "a//b", "../c"],
    ].map(([s, h, st]) => ({ scratch: s, home: h, store: st, copy: storeCopy(s!, h!, st!) })),
    mergeScript: mergeScript("/Users/ada/b_2.x", "/root/work/b_2.x", ["merged += 1", "kept += 2"]),
    mergeScriptNoSteps: mergeScript("/a", "/b", []),
    jsonlCwdStep: jsonlCwdStep(["/root/.codex/sessions/a.jsonl", "/root/x y.jsonl"], "payload"),
    jsonlCwdStepNoHolder: jsonlCwdStep([], undefined),
    underPath: underPath("cwd"),
    movedColumn: movedColumn("git_repo_root"),
    sqliteFilter: sqliteFilter([{ table: "threads", where: underPath("cwd") }, { table: "m", where: "session_id in (select id from s)" }]),
    sqliteFilterNone: sqliteFilter([]),
    sqliteMergeStep: sqliteMergeStep("/root/.codex/state_5.sqlite", tables),
    sqliteMergeStepRows: tables.map(t => ({ table: t.table, rows: t.rows.map(r => Object.entries(r).map(([k, v]) => [k, v === null ? "null" : v instanceof Uint8Array ? "blob" : typeof v === "string" ? "text" : Number.isInteger(v) && k !== "r" ? "int" : "real", v instanceof Uint8Array ? Buffer.from(v).toString("base64") : v])) })),
    resolvers,
    stateListings,
    unknownAgent,
    piKeys: ["/Users/ada/b_2.x", "/", "", "C:\\Users\\a:b", "/a/b/", "//x", "rel/path"].map(p => ({ path: p, key: piProjectKey(p) })),
    parseStateListing: parseStateListing(`/a\n\n!unread ${Buffer.from(JSON.stringify({ agent: "codex", store: "/root/.codex/state_5.sqlite", why: "database is locked" })).toString("base64")}\n/b c\n`),
    parseMergeOutput: ['{"merged": 2, "kept": 1}', 'noise\n{"merged": 0, "kept": 0, "note": "n"}\n\n', '{"waiting": "no db", "merged": 1, "kept": 1}', '{"merged": 1, "kept": 1, "note": 5}', "", "  \n", "not json", '{"merged": "1", "kept": 1}', "[1,2]", "null"].map(stdout => ({ stdout, out: thrown(() => parseMergeOutput(stdout)) })),
    paths: {
      insideFolder: ["", "a", "..", "../a", "..cache", "a/../..", "/abs", "a/b"].map(rel => ({ rel, inside: insideFolder(rel) })),
      underHome: [["/h", ["a"]], ["/h", ["../x"]], ["/h", ["a/../../x"]], ["/h", [""]], ["/h", ["..cache", "y"]], ["/h/", ["/abs"]]].map(([h, parts]) => ({ home: h, parts, at: underHome(h as string, ...(parts as string[])) ?? null })),
      movedPath: [["/a/b", "/a/b", "/c"], ["/a/b/x", "/a/b", "/c"], ["/a/bx", "/a/b", "/c"], ["/a", "/a/b", "/c"]].map(([p, f, t]) => ({ path: p, from: f, to: t, moved: movedPath(p!, f!, t!) ?? null })),
    },
    agentHomes: { plain: agentHomes("/Users/ada"), env: agentHomes("/Users/ada", { CLAUDE_CONFIG_DIR: "/opt/claude", CODEX_HOME: "" }), guest: guestAgentHomes(), one: agentHome("/Users/ada", "nope"), catalog: CATALOG_AGENTS.map(a => a.id) },
  });
});

it("solari create body", async () => {
  const bodies: string[] = [];
  const f = (async (_url: string, init?: RequestInit) => {
    bodies.push(String(init?.body));
    return new Response(JSON.stringify({ sandboxId: "s1", kind: "sandbox" }), { status: 201 });
  }) as unknown as typeof fetch;
  const b = new SolariBackend({ apiKey: "k", fetch: f });
  await b.create({ kind: "sandbox", template: "base", fromSnapshot: "snap_1", cpu: 2, memMb: 4096, diskGb: 20, envs: { HOME: "/root", B: "2", A: "1" }, labels: { wsp: "1", "wsp-owner": "h1" }, onIdle: "pause", idleTimeoutMs: 12 * 60 * 60_000, idempotencyKey: "k1" });
  await b.create({ kind: "desktop" });
  await b.create({ kind: "sandbox", cpu: 0, memMb: 0, diskGb: 0, idleTimeoutMs: 0, template: "" });
  write("solari.json", {
    bodies,
    capabilities: JSON.stringify(b.capabilities),
    expiry: [
      `${Buffer.from(JSON.stringify({ sandboxId: "a.b.c", exp: 1790000000123 })).toString("base64url")}.sig`,
      `${Buffer.from(JSON.stringify({ exp: 42 })).toString("base64url")}`,
      ".abc",
      "abc.",
      "not-a-token-at-all",
      "",
      `${Buffer.from('{"exp":1}').toString("base64")}`,
      `x.${Buffer.from('{"exp":7}').toString("base64url")}`,
    ].map(token => ({ token, at: previewTokenExpiry(token, 1000) })),
  });
});

it("brew shared step", () => {
  write("brew-shared.json", [["ffmpeg", "gh"], ["one"], [], ["it's", "a b"]].map(formulae => ({ formulae, deps: brewSharedDeps(formulae), check: brewSharedCheck(formulae) })));
});

it("login lines", async () => {
  const probe = async (given?: { HOME?: string; PATH?: string }) => {
    const { ran, machine } = recording("Linux\n1000\nada\nada\n1\n/home/ada\n/usr/bin\n");
    const login = await targetLogin(machine, given);
    return { given: given ?? null, cmd: ran[0], login };
  };
  const t: TargetLogin = { platform: "linux", home: "/home/a'da", path: "/usr/bin:/bin", user: "ada", runAs: "ada" };
  const shell = recording("hello\n/usr/local/bin:/usr/bin");
  await loginShellPath(shell.machine, t);
  write("login.json", {
    probes: [await probe(), await probe({ HOME: "/home/ada" }), await probe({ HOME: "/home/ada's", PATH: "/x" })],
    asLogin: [
      { t, line: "ls ~", out: asLogin(t, "ls ~") },
      { t: { platform: "linux", home: "/root", user: "root" }, line: "true", out: asLogin({ platform: "linux", home: "/root", user: "root" }, "true") },
      { t: { platform: "darwin", home: "/Users/ada", path: "/usr/bin", user: "ada" }, line: "echo 'x'", out: asLogin({ platform: "darwin", home: "/Users/ada", path: "/usr/bin", user: "ada" }, "echo 'x'") },
    ],
    loginShellCmd: shell.ran[0],
  });
});

it("missing commands and env exec", async () => {
  const m = recording("missing codex\n");
  await missingCommands(m.machine, ["claude", "codex", "it's"]);
  const pipe = recording("");
  await envExec(pipe.machine, "echo $A", { A: "1", B: "x'y\nz" });
  await envExec(pipe.machine, "true", {});
  write("commands.json", {
    missingCommands: m.ran[0],
    envExec: pipe.ran,
    envInput: Buffer.from(envInput({ A: "1", B: "x'y\nz", "": "" })).toString("base64"),
  });
});

it("tool sizes", () => {
  const formula = (over: Record<string, unknown>) => ({ tap: "homebrew/core", dependencies: [], build_dependencies: [], requirements: [], urls: { stable: { url: "https://example.org/src.tar.xz", tag: null } }, versions: { stable: "1.0", bottle: true }, installed: [{ version: "1.0", runtime_dependencies: [] }], ...over });
  const info = {
    formulae: [
      formula({ name: "ffmpeg", full_name: "ffmpeg", dependencies: ["x264", "openssl@3", "x264"], installed: [{ version: "8.1.2", runtime_dependencies: [{ full_name: "x264" }, { full_name: "openssl@3" }, { full_name: "ca-certificates" }, { name: "nofull" }, "str"] }] }),
      formula({ name: "nofull", installed: [] }),
      formula({ full_name: "noname" }),
      formula({ name: "gitonly", full_name: "someone/tap/gitonly", urls: { stable: { url: "https://github.com/someone/gitonly.git", tag: "v3.1.0" } } }),
      formula({ name: "rel", full_name: "rel", urls: { stable: { url: "https://github.com/o/r/releases/download/v1/r.tgz" } } }),
      formula({ name: "macthing", full_name: "someone/tap/macthing", requirements: [{ name: "macos" }] }),
      formula({ name: "sketchybar", full_name: "felixkratz/formulae/sketchybar" }),
      "not a record",
      formula({ name: "nourls", full_name: "nourls", urls: undefined }),
    ],
  };
  const du = "102400\t/opt/homebrew/Cellar/ffmpeg\n  20\t/x/y/  \n5 /opt/rel//\nbad line\n7\t/opt/homebrew/Cellar/ffmpeg\n\n";
  const urls: [string | undefined, string | null | undefined][] = [
    ["https://github.com/cli/cli/archive/refs/tags/v2.100.0.tar.gz", null],
    ["https://github.com/cli/cli/archive/v2.zip", null],
    ["https://github.com/Zingzy/diskbloom/releases/download/v0.1.0/x.tar.gz", null],
    ["https://github.com/someone/gitonly.git", "v3.1.0"],
    ["https://github.com/someone/gitonly", "v3"],
    ["https://github.com/someone/gitonly.git", null],
    ["https://github.com/someone/gitonly.git", ""],
    ["https://github.com/o/r/tree/main", "v1"],
    ["https://dl.example.org/x.tar.gz", "v1"],
    [undefined, "v1"],
    ["https://github.com/o", null],
    ["https://github.com/o/r.git/", "t"],
  ];
  const ids = ["tools/go/x", "tools/uv/y", "tools/npm/z", "tools/pnpm/z", "tools/bun/z", "tools/brew/w", "tools/gox", "tools/cargo/c"];
  const table = brewTable(info, parseDu(du));
  write("tool-sizes.json", {
    parseBrewInfo: parseBrewInfo(info),
    parseBrewInfoNotShaped: [parseBrewInfo(null), parseBrewInfo([]), parseBrewInfo({ formulae: {} })],
    parseDu: [...parseDu(du)],
    parseDuKeyed: [...parseDu(du, p => `k:${p}`)],
    brewTable: [...table],
    sourceOf: urls.map(([url, tag]) => ({ url: url ?? null, tag: tag === undefined ? "undefined" : tag, source: sourceOf(url, tag) ?? null })),
    assumed: [...ids.map(id => ({ rung: "tools", id })), { rung: "agents", id: "agents/claude" }].map(e => ({ ...e, size: assumedSize({ ...e, label: "", paths: [], bytes: 0, default: "bring" }) })),
  });
});

it("small values", () => {
  const ages = [undefined, -5, 0, 999, 59_999, 60_000, 3_599_999, 3_600_000, 4_500_000, 4_860_000, 5_400_000, 86_400_000];
  const blob = "AAAAC3NzaC1lZDI1NTE5AAAAIC6sV8jQCzynkpUOM40rRIjA7tPstUSjC+A/LVMnAZhP";
  const feed: string[] = [];
  const lf = lineFeed(l => feed.push(l), 4);
  lf.feed("ab\ncdéfgh");
  lf.feed("ij\r\nk");
  lf.feed("🎉🎉🎉");
  lf.flush();
  lf.flush();
  write("small.json", {
    ages: ages.map(ms => ({ ms: ms ?? null, words: describeAge(ms) })),
    fingerprints: [blob, blob.replaceAll("+", "-").replaceAll("/", "_"), ` ${blob.slice(0, 20)}\n${blob.slice(20)} `, `${blob}==`, `${blob}!!`, "", "=AAAA", "AA=AA", "QUJ", "QUJD", "not*base64!"].map(key => ({ key, fp: keyFingerprint(key) })),
    snapshotNames: {
      golden: [["h1", "default", 3], ["Bad", "g", 1], ["", "g", 1], ["h-1", "g", 1]].map(([h, g, v]) => ({ host: h, golden: g, version: v, out: thrown(() => goldenName(h as string, g as string, v as number)) })),
      project: thrown(() => projectSnapshotName("h1", "my-proj", "2026-10-25T00-00-00Z")),
      owners: [undefined, "wsp-h1-default-v3", "wsp-h1-", "wsp-H1-x", "wsp--x", "xwsp-h1-x", "wsp-h1-a-b-c", "wsp-h1-\nx"].map(n => ({ name: n ?? null, owner: nameOwner(n) ?? null })),
    },
    diskUse: ["100 300", " 100 300 \n", "0 0", "a b", "1 2\n3 4", "x\n5 6", "1  2", "7 8 9"].map(s => ({ stdout: s, use: diskUseOf(s) ?? null })),
    lineFeed: feed,
    installs: {
      projectInstalls: projectInstalls(["pnpm-lock.yaml", "package-lock.json", "bun.lockb", "uv.lock", "Cargo.lock"], "/root/my proj//"),
      stores: INSTALL_STORES,
      installScript: installScript({ row: "r", command: "pnpm install" }, { dir: "/root/my proj", log: "/root/.wsp/logs/install's.log" }),
      installScriptNoSlash: installScript({ row: "r", command: "x" }, { dir: "d", log: "plain.log" }),
      checkScripts: checkScripts("pnpm --version", { dir: "/root/p", env: { PNPM_HOME: "/root/.local/share/pnpm", A: "it's" } }),
    },
  });
});

it("sealed vault", () => {
  const image: SealedImage = {
    name: "default",
    version: 1,
    hash: "a".repeat(64),
    recipeHash: "rh",
    logins: [{ name: "codex", state: "signed-in" }],
    sealedAt: "2026-09-12T00:00:00.000Z",
    sealedFrom: "this-mac",
    vault: { sha256: "b".repeat(64), bytes: 128, paths: 2, takenAt: "2026-09-12T00:00:00.000Z" },
  };
  const place = generateKeyPairSync("x25519");
  const jwkPriv = place.privateKey.export({ format: "jwk" }) as { d: string; x: string };
  const plain = Buffer.from("tar bytes that are the person's logins\n\u0000\u00ff");
  const sealed = publicKeyCipher.seal(plain, image, { publicKey: place.publicKey });
  write("vault.json", {
    privateKey: Buffer.from(jwkPriv.d, "base64url").toString("base64"),
    publicKey: Buffer.from(jwkPriv.x, "base64url").toString("base64"),
    plain: plain.toString("base64"),
    sealed: sealed.toString("base64"),
    headerLine: sealed.subarray(0, sealed.indexOf(0x0a)).toString("utf8"),
  });
});
