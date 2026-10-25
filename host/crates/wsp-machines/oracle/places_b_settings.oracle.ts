// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers of the place door's settings unit (set with a rename, a login and a recipe together, the read of a
// new ssh login, the holder of a back forward, the setup log read) on fakes, for tests/places_manage_settings.rs;
// written as tests/fixtures/oracle/places_b_settings.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 places_b_settings.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { DAEMON_VERSION, RecipeFile, heldPlaceScript, placeProvisionPaths, SETUP_LOG_TAIL_BYTES } from "@wsp/protocol";

const { makePlaceDoor, newPlaceKeyPair } = await import("../../../../packages/runtime/src/places.ts");
const { memoryStore } = await import("../../../../packages/runtime/src/store.ts");
const { makeDevices } = await import("../../../../packages/runtime/src/devices.ts");

const out = fileURLToPath(new URL("../tests/fixtures/oracle/places_b_settings.json", import.meta.url));
const NOW = 1_700_000_000_000;
const ISO = new Date(NOW).toISOString();
const ID1 = "p_0000000000000001";
const ID2 = "p_0000000000000002";

const outcome = async (f: () => Promise<unknown>): Promise<unknown> => {
  try {
    const v = await f();
    return { ok: v === undefined ? null : v };
  } catch (e) {
    const x = e as Error & { kind?: string; fix?: string };
    return { err: x.message, kind: x.kind ?? null, fix: x.fix ?? null };
  }
};

const report = (over: Record<string, unknown> = {}) => ({
  name: "old-macbook",
  platform: "linux",
  arch: "x64",
  os: "Ubuntu 24.04",
  shape: { cpu: 4, memMb: 4096 },
  diskFreeBytes: 1000,
  diskSizeBytes: 50_000_000_000,
  model: "Mac14,3",
  login: { HOME: "/home/maya", USER: "maya", PATH: "/usr/bin" },
  runsWorkspaces: true,
  engine: "none",
  daemonVersion: DAEMON_VERSION,
  uptimeMs: 5000,
  agents: ["claude", "codex"],
  logins: [],
  wsp: ["/home/maya/.npm-global/bin/wsp"],
  dialed: "http://192.168.1.20:14621",
  ...over,
});
const picks = RecipeFile.parse({ name: "web", plugins: { lint: { agent: "claude" } } });
const record = (over: Record<string, unknown> = {}) => ({
  id: ID1,
  name: "old-macbook",
  publicKey: "PUBKEY",
  joinedAt: "2026-10-01T00:00:00.000Z",
  lastSeenAt: "2026-10-02T00:00:00.000Z",
  reportedAt: "2026-10-02T00:00:00.000Z",
  report: report(),
  road: { ssh: "maya@10.0.0.5", keyPath: "/home/dev/.ssh/id", hostKey: "ssh-ed25519 AAAA", from: "10.0.0.5" },
  picks,
  recipe: "web",
  ...over,
});
const pendingOf = (id: string, placeId: string, name: string) => ({ id, address: "10.0.0.5", placeId, name, step: "choosing", choices: picks, startedAt: ISO });

const hostKey = newPlaceKeyPair();
const heldOut = (placeId: string): string => `\nWSP_PLACE_HELD\n${JSON.stringify({ placeId, name: "x", hostName: "zingzys-mac", hostUrls: [], hostPublicKey: hostKey.publicKey, keyPath: "/k" }, null, 2)}\n`;

const door = async (o: { records?: Record<string, unknown>[]; pending?: Record<string, unknown>[]; wiring?: Record<string, unknown>; events?: unknown[]; setups?: unknown[]; recordingLog?: string[] } = {}) => {
  const store = memoryStore();
  for (const r of o.records ?? [record()]) await store.put("places", r["id"] as string, r);
  for (const p of o.pending ?? []) await store.put("pending-computers", p["id"] as string, p);
  const d = makePlaceDoor({
    store,
    devices: makeDevices(store),
    wiring: { hostKey, provider: () => ({ id: "boat", rateUsdPerHour: 0.25 }), here: () => ({ name: "zingzys-mac", os: "macOS 15.0", shape: { cpu: 8, memMb: 16384 }, mac: "mac-mini", home: "/home/dev" }), hostName: () => "zingzys-mac", ...(o.wiring ?? {}) } as never,
    recording: { projectsOn: async () => [], holdsOn: async () => ({ forks: [], projects: [], unsaved: [] }), dropOn: async () => ({ forks: [], projects: [] }), forgetOn: async () => ({ forks: [], projects: [] }), runningOn: async () => 0, removeFolder: async () => undefined, folderLook: async () => undefined } as never,
    vault: () => ({ ANTHROPIC_API_KEY: "k" }),
    now: () => NOW,
    schedule: () => () => undefined,
    onSetup: e => o.setups?.push(e),
  });
  d.on(e => o.events?.push(e));
  await d.load();
  return { d, store };
};

const doc = async (store: { get(c: string, id: string): Promise<unknown> }, c: string, id: string) => (await store.get(c, id)) ?? null;

it("writes the places b settings fixture", async () => {
  const cases: Record<string, unknown> = {};

  // A name, a setting and a recipe in one call, with a pending add naming the place and one naming another.
  {
    const events: unknown[] = [];
    const setups: unknown[] = [];
    const { d, store } = await door({ records: [record(), record({ id: ID2, name: "box2", recipe: undefined })], pending: [pendingOf("a_000000000001", ID1, "old-macbook"), pendingOf("a_000000000003", ID2, "box2")], events, setups });
    cases["together"] = await outcome(() => d.set(ID1, { threads: 2 } as never, [], { name: "  renamed  ", recipe: "api" }));
    cases["togetherRecord"] = await doc(store, "places", ID1);
    cases["togetherCaps"] = await doc(store, "caps", ID1);
    cases["togetherPending"] = await store.list("pending-computers");
    cases["togetherEvents"] = events.length;
    cases["togetherSetups"] = setups;
  }

  // A recipe or a name or a login alone settles nothing and writes no caps.
  {
    const { d, store } = await door({ records: [record({ recipe: undefined })] });
    cases["recipeAlone"] = await outcome(() => d.set(ID1, {} as never, [], { recipe: "web" }));
    cases["recipeAloneCaps"] = await doc(store, "caps", ID1);
    cases["blankRename"] = await outcome(() => d.set(ID1, {} as never, [], { name: "   " }));
    cases["hereRename"] = await outcome(() => d.set("here", {} as never, [], { name: "x" }));
    cases["hereSsh"] = await outcome(() => d.set("here", {} as never, [], { ssh: "root@x" }));
    cases["boatSsh"] = await outcome(() => d.set("boat", {} as never, [], { ssh: "root@x" }));
    cases["badSsh"] = await outcome(() => d.set(ID1, {} as never, [], { ssh: "nope" }));
    cases["blankSsh"] = await outcome(() => d.set(ID1, {} as never, [], { ssh: "  " }));
    cases["providerRecipe"] = await outcome(() => d.set("boat", {} as never, [], { recipe: "web" }));
    cases["emptySet"] = await outcome(() => d.set(ID1, {} as never));
    cases["bothSetAndReset"] = await outcome(() => d.set(ID1, { threads: 2 } as never, ["threads"]));
    cases["writes"] = [await doc(store, "caps", ID1), (await store.list("places")).length];
  }

  // Refusals before a write: a bad setting with a rename leaves the name alone.
  {
    const { d, store } = await door();
    cases["badSettingWithRename"] = await outcome(() => d.set(ID1, { machines: 2 } as never, [], { name: "renamed" }));
    cases["badSettingWithRenameRecord"] = ((await store.get("places", ID1)) as { name: string }).name;
  }

  // A login: read as the same computer, another, unread; and what is left alone.
  const login = async (runOver: ((login: unknown, script: string, ms: number, pw?: string) => Promise<unknown>) | undefined, ssh: string, name: string, extra: Record<string, unknown> = {}) => {
    const scripts: unknown[] = [];
    const events: unknown[] = [];
    const wiring = runOver === undefined ? { ...extra } : { runOver: async (l: unknown, s: string, ms: number, pw?: string) => (scripts.push([l, s, ms, pw ?? null]), runOver(l, s, ms, pw)), ...extra };
    const { d, store } = await door({ records: [record(), record({ id: ID2, name: "box2" })], wiring, events });
    const answer = await outcome(() => d.set(ID1, {} as never, [], { ssh }));
    return { answer, record: await doc(store, "places", ID1), scripts, events: events.length, name };
  };
  cases["loginSame"] = await login(async () => ({ exitCode: 0, stdout: heldOut(ID1), stderr: "" }), "root@10.0.0.9", "same");
  cases["loginOther"] = await login(async () => ({ exitCode: 0, stdout: heldOut("p_00000000000000ff"), stderr: "" }), "root@10.0.0.9", "other");
  cases["loginNamesOtherRecord"] = await login(async () => ({ exitCode: 0, stdout: heldOut(ID2), stderr: "" }), "root@10.0.0.9", "otherRecord");
  cases["loginNoFile"] = await login(async () => ({ exitCode: 0, stdout: "\nWSP_PLACE_HELD\n", stderr: "" }), "root@10.0.0.9", "noFile");
  cases["loginExit"] = await login(async () => ({ exitCode: 3, stdout: "", stderr: "first\nlast line\n" }), "root@10.0.0.9", "exit");
  cases["loginExitSilent"] = await login(async () => ({ exitCode: 3, stdout: "", stderr: "" }), "root@10.0.0.9", "exitSilent");
  cases["loginThrows"] = await login(async () => { throw new Error("ssh: connect to host 10.0.0.9 port 22: Connection refused"); }, "root@10.0.0.9", "throws");
  cases["loginNoRoad"] = await login(undefined, "root@10.0.0.9", "noRoad");
  cases["loginOwn"] = await login(async () => ({ exitCode: 0, stdout: heldOut("p_00000000000000ff"), stderr: "" }), "maya@10.0.0.5", "own");
  cases["loginSpaced"] = await login(async () => ({ exitCode: 0, stdout: heldOut(ID1), stderr: "" }), "  root@10.0.0.9  ", "spaced");

  // A new login on a record that dials back through a forward: the old login's forward is let go unless another
  // record holds it, and the new one is held.
  const back = async (others: Record<string, unknown>[]) => {
    const log: string[] = [];
    const wiring = {
      runOver: async () => ({ exitCode: 0, stdout: heldOut(ID1), stderr: "" }),
      back: { hold: async (l: { ssh: string }, _b: unknown, on: { home: string; computer: { id: string; name: string } }) => (log.push(`hold ${l.ssh} ${on.home} ${on.computer.id}`), { boxPort: 7001 }), release: (l: { ssh: string }) => void log.push(`release ${l.ssh}`), door: () => undefined, close: () => undefined },
    };
    const { d, store } = await door({ records: [record({ road: { ssh: "maya@10.0.0.5", from: "10.0.0.5", back: { boxPort: 7000 } } }), ...others], wiring });
    const answer = await outcome(() => d.set(ID1, {} as never, [], { ssh: "root@10.0.0.9" }));
    return { answer, log, record: await doc(store, "places", ID1) };
  };
  cases["backAlone"] = await back([]);
  cases["backShared"] = await back([record({ id: ID2, name: "box2", road: { ssh: "maya@10.0.0.5", from: "10.0.0.5" } })]);

  // Two sets at once on one place both stand.
  {
    const { d, store } = await door();
    await Promise.all([d.set(ID1, { threads: 2 } as never), d.set(ID1, { napMs: 1_800_000 } as never), d.set(ID1, { spawn: { maxDepth: 1 } } as never)]);
    cases["twoAtOnceCaps"] = await doc(store, "caps", ID1);
  }

  // A reset of a part of the agents' caps leaves the other part; nothing left deletes the document.
  {
    const { d, store } = await door();
    await d.set(ID1, { spawn: { maxMachines: 5, maxDepth: 1 } } as never);
    cases["spawnBoth"] = await doc(store, "caps", ID1);
    await d.set(ID1, {} as never, ["max-depth"] as never);
    cases["spawnAfterDepthReset"] = await doc(store, "caps", ID1);
    await d.set(ID1, {} as never, ["max-machines"] as never);
    cases["spawnAfterBothReset"] = await doc(store, "caps", ID1);
    await d.set(ID1, { turnLimitMs: null } as never);
    cases["turnLimitNull"] = await doc(store, "caps", ID1);
  }


  // A rename onto a name another row answers to, whatever its case, and onto a cloud's.
  {
    const { d } = await door({ records: [record(), record({ id: ID2, name: "box2" })] });
    cases["renameToProvider"] = await outcome(() => d.set(ID1, {} as never, [], { name: "BOAT" }));
    cases["renameToOtherRecord"] = await outcome(() => d.set(ID1, {} as never, [], { name: "box2" }));
    cases["renameToOwn"] = await outcome(() => d.set(ID1, {} as never, [], { name: "old-macbook" }));
  }

  // skip off the record: a wait is put back last as skipped, and the frame goes only once the setup is no longer running.
  const wait = (row: string, label: string) => ({ row, label, url: "https://auth.example/x", code: "C1", expiresAt: ISO, state: "waiting" });
  const setupOf = (state: string, waiting: unknown[]) => ({ state, addId: "a_000000000009", startedAt: ISO, steps: [], waiting });
  const skipCase = async (name: string, over: Record<string, unknown>, row: string) => {
    const setups: unknown[] = [];
    const events: unknown[] = [];
    const { d, store } = await door({ records: [record(over)], setups, events });
    const answer = await outcome(() => d.skip(ID1, row));
    cases[name] = { answer, record: await doc(store, "places", ID1), setups, events: events.length };
  };
  const rowOf = (id: string, outcomeWord: string, extra: Record<string, unknown> = {}) => ({ id, label: id.toUpperCase(), outcome: outcomeWord, ...extra });
  await skipCase("skipWaitingRunning", { setup: setupOf("running", [wait("signins/codex", "Codex"), wait("signins/claude", "Claude Code")]) }, "signins/codex");
  await skipCase("skipWaitingDone", { setup: setupOf("done", [wait("signins/codex", "Codex")]) }, "signins/codex");
  await skipCase("skipWaitingNoApplied", { applied: undefined, setup: setupOf("done", [wait("signins/codex", "Codex")]) }, "signins/codex");
  await skipCase("skipStepOffAnyRow", { setup: setupOf("done", [wait("signins/codex", "Codex")]), applied: { hash: "h1", at: ISO, items: { "clis/jq": "1.7" }, rows: [rowOf("signins/codex", "present", { step: "signins" }), rowOf("plugins/lint", "installed", { step: "plugins" })] } }, "signins/codex");
  await skipCase("skipFailedNotLast", { applied: { hash: "h1", at: ISO, items: { "clis/jq": "1.7" }, rows: [rowOf("clis/gh", "failed", { step: "clis", note: "no network", fix: "retry" }), rowOf("plugins/lint", "installed", { step: "plugins" })] } }, "clis/gh");
  await skipCase("skipFailedLabelOverWait", { setup: setupOf("done", [wait("clis/gh", "Wait label")]), applied: { hash: "h1", at: ISO, rows: [rowOf("clis/gh", "failed", { step: "clis" })] } }, "clis/gh");
  await skipCase("skipPresentRowNothing", { setup: setupOf("done", []) }, "plugins/lint");
  await skipCase("skipSetupRunningFailedRow", { setup: setupOf("running", []), applied: { hash: "h1", at: ISO, rows: [rowOf("clis/gh", "failed", { step: "clis" })] } }, "clis/gh");

  const held = heldPlaceScript("/home/maya");
  const log = placeProvisionPaths("/home/maya").log;
  writeFileSync(out, `${JSON.stringify({ heldScript: held, setupLogPath: log, tailBytes: SETUP_LOG_TAIL_BYTES, cases }, null, 2)}\n`);
});
