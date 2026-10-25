// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers of places/types.ts's readers and lines, and of the place door's records, views and manage roads run on
// fakes (no link, no runtime), on the inputs the slice b Rust tests replay; written as tests/fixtures/oracle/places_b.json.
// Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 places_b.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { DAEMON_VERSION, RecipeFile } from "@wsp/protocol";

const t = await import("../../../../packages/runtime/src/places/types.ts");
const { makePlaceDoor, newPlaceKeyPair } = await import("../../../../packages/runtime/src/places.ts");
const { memoryStore } = await import("../../../../packages/runtime/src/store.ts");
const { makeDevices } = await import("../../../../packages/runtime/src/devices.ts");

const out = fileURLToPath(new URL("../tests/fixtures/oracle/places_b.json", import.meta.url));
const NOW = 1_700_000_000_000;
const ISO = new Date(NOW).toISOString();

const outcome = async (f: () => Promise<unknown>): Promise<unknown> => {
  try {
    const v = await f();
    return { ok: v === undefined ? null : v };
  } catch (e) {
    const x = e as Error & { kind?: string; fix?: string };
    return { err: x.message, kind: x.kind ?? null, fix: x.fix ?? null };
  }
};

/** The joined computer every door scenario holds, as a record the store keeps. */
export const report = (over: Record<string, unknown> = {}) => ({
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
const picks = RecipeFile.parse({ name: "web", plugins: { lint: { agent: "claude" }, fmt: { agent: "claude" }, old: { agent: "claude" } } });
const row = (id: string, outcomeWord: string, extra: Record<string, unknown> = {}) => ({ id, label: id.toUpperCase(), outcome: outcomeWord, ...extra });
export const record = (over: Record<string, unknown> = {}) => ({
  id: "p_0000000000000001",
  name: "old-macbook",
  publicKey: "PUBKEY",
  joinedAt: "2026-10-01T00:00:00.000Z",
  lastSeenAt: "2026-10-02T00:00:00.000Z",
  reportedAt: "2026-10-02T00:00:00.000Z",
  report: report(),
  road: { ssh: "maya@10.0.0.5", keyPath: "/home/dev/.ssh/id", hostKey: "ssh-ed25519 AAAA", from: "10.0.0.5" },
  applied: {
    hash: "h1",
    at: "2026-10-02T00:00:00.000Z",
    rows: [
      row("plugins/lint", "installed", { step: "plugins" }),
      row("plugins/fmt", "present", { step: "plugins" }),
      row("folders/site", "installed", { step: "folders", project: { id: "pr_00000001", name: "site" }, pick: { from: "/home/dev/site" }, createdAt: "2026-10-01T00:00:00.000Z" }),
      row("clis/gh", "failed", { step: "clis", note: "no network" }),
    ],
  },
  picks,
  recipe: "web",
  ...over,
});

const vault = { ANTHROPIC_API_KEY: "k" };
const fakeRecording = (log: string[], over: Record<string, unknown> = {}) => ({
  projectsOn: async () => [],
  holdsOn: async (_id: string, read?: boolean) => (log.push(`holdsOn read=${String(read)}`), { forks: [], projects: [], unsaved: [] }),
  dropOn: async () => (log.push("dropOn"), { forks: [], projects: [] }),
  forgetOn: async () => (log.push("forgetOn"), { forks: [{ name: "f1", threads: 0 }], projects: [] }),
  runningOn: async (id: string) => (id === "p_0000000000000001" ? 2 : 0),
  removeFolder: async () => undefined,
  folderLook: async () => undefined,
  ...over,
});

const door = async (o: { records?: Record<string, unknown>[]; caps?: Record<string, unknown>; mark?: string; wiring?: Record<string, unknown>; recording?: Record<string, unknown>; pending?: Record<string, unknown>[]; log?: string[]; events?: unknown[]; setups?: unknown[] } = {}) => {
  const store = memoryStore();
  for (const r of o.records ?? [record()]) await store.put("places", r["id"] as string, r);
  for (const [id, c] of Object.entries(o.caps ?? {})) await store.put("caps", id, c);
  if (o.mark !== undefined) await store.put("place-default", "default", { placeId: o.mark });
  for (const p of o.pending ?? []) await store.put("pending-computers", p["id"] as string, p);
  const log = o.log ?? [];
  const d = makePlaceDoor({
    store,
    devices: makeDevices(store),
    wiring: { hostKey: newPlaceKeyPair(), provider: () => ({ id: "boat", rateUsdPerHour: 0.25 }), here: () => ({ name: "zingzys-mac", os: "macOS 15.0", shape: { cpu: 8, memMb: 16384 }, mac: "mac-mini", home: "/home/dev" }), hostName: () => "zingzys-mac", ...(o.wiring ?? {}) } as never,
    recording: fakeRecording(log, o.recording) as never,
    vault: () => vault,
    now: () => NOW,
    schedule: () => () => undefined,
    onSetup: e => o.setups?.push(e),
  });
  d.on(e => o.events?.push(e));
  await d.load();
  return { d, store };
};

it("writes the places b fixture", async () => {
  const held = record();
  const types = {
    madeBySetup: t.madeBySetup(held as never, "plugins"),
    madeBySetupNoApplied: t.madeBySetup({ ...held, applied: undefined } as never, "plugins"),
    appliedView: t.appliedView(held.applied as never),
    projectsMade: [...t.projectsMade(held as never)],
    projectsMadeTwice: [...t.projectsMade({ ...held, applied: { ...held.applied, rows: [...held.applied.rows, row("folders/b", "installed", { project: { id: "pr_00000002", name: "b" } }), row("folders/c", "present", { project: { id: "pr_00000001", name: "site" } })] } } as never)],
    isPlaceRecord: [
      { v: { id: "a", publicKey: "k", name: "n" } },
      { v: { id: "", publicKey: "", name: "" } },
      { v: { id: "a", publicKey: "k" } },
      { v: { id: 1, publicKey: "k", name: "n" } },
      { v: null },
      { v: [] },
      { v: "a" },
      { v: { id: "a", publicKey: "k", name: "n", extra: true } },
    ].map(c => ({ ...c, out: t.isPlaceRecord(c.v) })),
    lines: {
      NO_PLACE_DOOR: t.NO_PLACE_DOOR,
      NO_PLACE_UPDATER: t.NO_PLACE_UPDATER,
      placeUpdateSlowLine: t.placeUpdateSlowLine("old-macbook", 60),
      placeSweptOverSshLine: t.placeSweptOverSshLine("old-macbook"),
      placeLoginRoadLine: t.placeLoginRoadLine("old-macbook", "maya@10.0.0.5"),
      placeLoginRoadLineSaid: t.placeLoginRoadLine("old-macbook", "maya@10.0.0.5", "systemctl failed"),
      placeLoginRoadLineEmpty: t.placeLoginRoadLine("old-macbook", "maya@10.0.0.5", ""),
      placeSweptOverLinkLine: t.placeSweptOverLinkLine("old-macbook", "maya@10.0.0.5", "systemctl failed"),
      placeElsewhereSweptOverLinkLine: t.placeElsewhereSweptOverLinkLine("old-macbook", "maya@10.0.0.5 reaches another machine"),
    },
  };

  // Rows: this computer, the joined one, the wired provider.
  const rowsPlain = await (await door()).d.rows();
  const rowsSet = await (await door({ caps: { p_0000000000000001: { threads: 3, napMs: null }, boat: { machines: 2, size: { cpu: 4, memMb: 8192 } } }, mark: "p_0000000000000001" })).d.rows();
  const rowsGoneMark = await (await door({ mark: "p_00000000000000ff" })).d.rows();
  const rowsBehind = await (await door({ records: [record({ report: report({ daemonVersion: DAEMON_VERSION - 1, takesRuntime: true, logins: ["codex/auth.json"] }), road: { keyPath: "/k" } })] })).d.rows();
  const listPlain = await (await door()).d.list(NOW);

  // set
  const setCases: Record<string, unknown> = {};
  {
    const events: unknown[] = [];
    const { d, store } = await door({ caps: { p_0000000000000001: { threads: 3, spawn: { maxMachines: 5 } } }, events });
    setCases["patch"] = await outcome(() => d.set("p_0000000000000001", { threads: 2, spawn: { maxDepth: 1 } } as never, ["nap"]));
    setCases["patchCaps"] = (await store.get("caps", "p_0000000000000001")) ?? null;
    setCases["patchEvents"] = events.length;
    setCases["resetAll"] = await outcome(() => d.set("p_0000000000000001", {} as never, ["threads", "max-machines", "max-depth"] as never));
    setCases["resetAllCaps"] = (await store.get("caps", "p_0000000000000001")) ?? null;
    setCases["nullNap"] = await outcome(() => d.set("p_0000000000000001", { napMs: null } as never));
    setCases["nullNapCaps"] = (await store.get("caps", "p_0000000000000001")) ?? null;
    setCases["unknown"] = await outcome(() => d.set("p_nope", { threads: 2 } as never));
    setCases["providerThreads"] = await outcome(() => d.set("boat", { threads: 2 } as never));
    setCases["hereRecipe"] = await outcome(() => d.set("here", {} as never, [], { recipe: "web" }));
    setCases["renameToHere"] = await outcome(() => d.set("p_0000000000000001", {} as never, [], { name: "zingzys-mac" }));
    setCases["rename"] = await outcome(() => d.set("p_0000000000000001", {} as never, [], { name: "  new-name  " }));
    setCases["renameRecord"] = (await store.get("places", "p_0000000000000001")) ?? null;
    setCases["renameCaps"] = (await store.get("caps", "p_0000000000000001")) ?? null;
  }

  // dial
  const dialCases: Record<string, unknown> = {};
  {
    const { d } = await door({ records: [record({ road: undefined })] });
    dialCases["noRoad"] = await outcome(() => d.dial("p_0000000000000001", NOW));
    dialCases["unknown"] = await outcome(() => d.dial("p_nope", NOW));
  }
  {
    const { d, store } = await door({ wiring: { dial: async () => undefined } });
    dialCases["sshOk"] = await outcome(() => d.dial("p_0000000000000001", NOW));
    dialCases["sshOkRecord"] = await store.get("places", "p_0000000000000001");
  }
  {
    const { d } = await door({ wiring: { dial: async () => { throw new Error("ssh: connect to host 10.0.0.5 port 22: Connection refused"); } } });
    dialCases["sshFails"] = await outcome(() => d.dial("p_0000000000000001", NOW));
  }

  // remove, holds, update
  const removeCases: Record<string, unknown> = {};
  {
    const log: string[] = [];
    const events: unknown[] = [];
    const { d, store } = await door({ log, events, mark: "p_0000000000000001", pending: [{ id: "a_000000000001", address: "10.0.0.5", placeId: "p_0000000000000001", step: "choosing", choices: picks, startedAt: ISO }] });
    removeCases["holdsAway"] = await outcome(() => d.holds("p_0000000000000001"));
    removeCases["holdsUnknown"] = await outcome(() => d.holds("p_nope"));
    removeCases["unknown"] = await outcome(() => d.remove("p_nope"));
    removeCases["stillInstalled"] = await outcome(() => d.remove("p_0000000000000001"));
    removeCases["log"] = log;
    removeCases["events"] = events;
    removeCases["after"] = { record: (await store.get("places", "p_0000000000000001")) ?? null, mark: (await store.get("place-default", "default")) ?? null, pending: await store.list("pending-computers") };
  }
  {
    const { d } = await door({ recording: { holdsOn: async () => ({ forks: [{ name: "f1", threads: 0 }], projects: [], unsaved: [] }) } });
    removeCases["awayWithForks"] = await outcome(() => d.remove("p_0000000000000001"));
    removeCases["awayForget"] = await outcome(() => d.remove("p_0000000000000001", { forget: true }));
  }
  {
    const { d } = await door({ recording: { holdsOn: async () => ({ forks: [], projects: [], unsaved: ["site has 2 commits no remote has"] }) } });
    removeCases["unsaved"] = await outcome(() => d.remove("p_0000000000000001"));
  }
  {
    const { d } = await door({ recording: { forgetRecordsOn: async () => undefined }, wiring: { forgetKey: async () => "the key is still in WSP_BOAT_KEY" } });
    removeCases["providerForget"] = await outcome(() => d.remove("boat", { forget: true }));
    removeCases["holdsProvider"] = await outcome(() => d.holds("boat"));
  }
  const updateCases: Record<string, unknown> = {
    current: await outcome(async () => (await door()).d.update("p_0000000000000001")),
    behind: await outcome(async () => (await door({ records: [record({ report: report({ daemonVersion: DAEMON_VERSION - 1 }) })] })).d.update("p_0000000000000001")),
    unknown: await outcome(async () => (await door()).d.update("p_nope")),
  };

  // skip, pending, choose, setUp, estimate, follow
  const skipCases: Record<string, unknown> = {};
  {
    const setups: unknown[] = [];
    const { d, store } = await door({ setups, records: [record({ setup: { state: "done", addId: "a_000000000009", startedAt: ISO, steps: [], waiting: [] } })] });
    skipCases["failed"] = await outcome(() => d.skip("p_0000000000000001", "clis/gh"));
    skipCases["record"] = await store.get("places", "p_0000000000000001");
    skipCases["setups"] = setups;
    skipCases["nothing"] = await outcome(() => d.skip("p_0000000000000001", "plugins/lint"));
    skipCases["unknown"] = await outcome(() => d.skip("p_nope", "x"));
  }
  const pendingCases: Record<string, unknown> = {};
  {
    const setups: unknown[] = [];
    const pend = [
      { id: "a_000000000002", address: "10.0.0.9", step: "wsp", choices: RecipeFile.parse({ name: "10.0.0.9" }), startedAt: "2026-10-03T00:00:00.000Z", keyPath: "/k", undo: "rm -rf x", login: "root@10.0.0.9" },
      { id: "a_000000000001", address: "10.0.0.5", name: "old-macbook", placeId: "p_0000000000000001", step: "choosing", choices: RecipeFile.parse({ name: "x" }), recipe: "web", startedAt: "2026-10-02T00:00:00.000Z" },
    ];
    const { d, store } = await door({ setups, pending: pend, records: [record({ picks: undefined })] });
    pendingCases["pending"] = await d.pending();
    pendingCases["choose"] = await outcome(() => d.choose("10.0.0.5", picks as never));
    pendingCases["chooseStored"] = await store.get("pending-computers", "a_000000000001");
    pendingCases["chooseUnknown"] = await outcome(() => d.choose("nobody", picks as never));
    pendingCases["setUpNotJoined"] = await outcome(() => d.setUp("10.0.0.9", {}));
    pendingCases["setUpUnknown"] = await outcome(() => d.setUp("nobody", {}));
    pendingCases["setups"] = setups;
  }
  {
    const { d } = await door({ records: [record({ picks: undefined })], pending: [] });
    pendingCases["setUpNoPicks"] = await outcome(() => d.setUp("old-macbook", {}));
  }
  {
    const { d } = await door({ records: [record(), record({ id: "p_0000000000000002" })] });
    pendingCases["setUpTwo"] = await outcome(() => d.setUp("old-macbook", {}));
    pendingCases["find"] = (await d.find("old-macbook")).map(r => r.id);
  }
  const estimateCases = {
    none: await outcome(async () => (await door()).d.estimate("old-macbook", picks as never)),
    weighed: await outcome(async () => (await door({ wiring: { provision: { estimate: async () => ({ bytes: 123, unmeasured: 2 }) } } })).d.estimate("old-macbook", picks as never)),
    nobody: await outcome(async () => (await door({ wiring: { provision: { estimate: async () => ({ bytes: 123, unmeasured: 2 }) } } })).d.estimate("nobody", picks as never)),
  };
  const followCases: Record<string, unknown> = {};
  {
    const setups: unknown[] = [];
    const { d, store } = await door({ setups, records: [record({ sync: { state: "behind", changes: ["clis/gh"], since: ISO } }), record({ id: "p_0000000000000002", name: "box2", recipe: "api" })] });
    followCases["followers"] = Object.fromEntries(await d.followers());
    followCases["followNone"] = await outcome(() => d.follow("p_0000000000000001", "none"));
    followCases["followNoneRecord"] = await store.get("places", "p_0000000000000001");
    followCases["followApi"] = await outcome(async () => (await d.follow("p_0000000000000001", "api")).recipe);
    followCases["unfollow"] = await d.unfollow("api");
    followCases["afterUnfollow"] = (await store.list("places")).map(r => [(r as { id: string }).id, (r as { recipe?: string }).recipe ?? null]);
    followCases["followUnknown"] = await outcome(() => d.follow("p_nope", "web"));
    followCases["setups"] = setups;
  }

  writeFileSync(
    out,
    `${JSON.stringify({ daemonVersion: DAEMON_VERSION, now: NOW, types, rows: { plain: rowsPlain, set: rowsSet, goneMark: rowsGoneMark, behind: rowsBehind, list: listPlain }, set: setCases, dial: dialCases, remove: removeCases, update: updateCases, skip: skipCases, pending: pendingCases, estimate: estimateCases, follow: followCases, record: record() }, null, 2)}\n`,
  );
});
