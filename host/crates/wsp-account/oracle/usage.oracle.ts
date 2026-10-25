// The TS answers the usage tests compare to: usageArea (packages/runtime/src/account/usage.ts at 65f6b89c7) run on a
// scripted context whose ledger, alerts, account rows, price table, plan scripts and reset spender are recorders, over
// the worlds below. Each scenario records what the area asked of those parts and what it warned. The sentences the area
// builds from @wsp/protocol are written beside them. Written as tests/fixtures/oracle/usage.json with the worlds. Run
// from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 usage.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";
import { absentComputer, cloudSaid, resetNoLoginsLine, subCloudGoneLine, THIS_COMPUTER, unknownOpLine, USAGE_WORDS } from "@wsp/protocol";
import { envInput, withEnvFromInput } from "@wsp/engine";

const H = vi.hoisted(() => ({ now: (() => 0) as () => number, w: undefined as any, rec: undefined as any, bus: [] as (() => void)[], resetOpts: undefined as any }));

vi.mock("../../../../packages/runtime/src/usage.ts", async importOriginal => {
  const orig = await importOriginal<any>();
  return {
    ...orig,
    createPriceTable: () => ({ get: async () => ({}) }),
    createBurn: () => ({ of: () => undefined }),
    createUsageLedger: () => ({
      limits: async () => H.w.ledgerLimits,
      fileLogs: async (entries: unknown[], read: string[]) => {
        if (H.w.fileError !== undefined) throw new Error(H.w.fileError);
        H.rec.filed.push({ entries, read });
      },
      limit: async (r: any) => {
        H.rec.readings.push(r);
        const after = { key: r.key, readAt: r.at ?? H.now() };
        return H.w.stale.includes(r.key) ? { before: after, after } : { before: undefined, after };
      },
      accountLabels: async () => new Map(Object.entries(H.w.accountLabels)),
      used: async (o: any) => {
        H.rec.used.push({ range: o.range, split: o.split, outside: o.outside, labels: Object.entries(H.w.labelProbe).map(([split, values]) => [split, (values as string[]).map(v => o.label(split, v))]) });
        return { range: o.range, split: o.split, rows: [], series: [], since: 0, until: 0 };
      },
    }),
    accountRows: (o: any) => {
      H.rec.rows.push({ limits: o.limits, places: o.places, names: H.w.nameProbe.map((id: string) => o.nameOf(id)) });
      return [];
    },
    accountNames: (o: any) => {
      H.rec.names.push({ limits: o.limits, names: H.w.nameProbe.map((id: string) => o.nameOf(id)) });
      return new Map(Object.entries(H.w.accountNames));
    },
  };
});
vi.mock("../../../../packages/runtime/src/plan-alerts.ts", () => ({ planAlerts: () => ({ resume: async () => {}, read: (b: unknown, a: unknown) => void H.rec.alerts.push([b ?? null, a]) }) }));
vi.mock("../../../../packages/runtime/src/adapters.ts", async importOriginal => ({
  ...(await importOriginal<any>()),
  LOG_LIMITS: { codex: (snapshot: unknown) => (H.rec.snapshots.push(snapshot), { windows: [], plan: JSON.stringify(snapshot) }) },
  PLAN_RESETS: {
    codex: {
      readCommand: (road: unknown) => `read ${JSON.stringify(road)}`,
      parseRead: (out: string) => {
        if (out.startsWith("limit:")) return { limit: JSON.parse(out.slice(6)) };
        return undefined;
      },
    },
  },
}));
vi.mock("../../../../packages/runtime/src/usage-reset.ts", () => ({
  usageResets: (o: unknown) => {
    H.resetOpts = o;
    return async (ask: unknown) => (H.rec.spends.push(ask), H.w.spendAnswer);
  },
}));

const { NO_PLACE_DOOR } = await import("../../../../packages/runtime/src/places.ts");
const { usageArea } = await import("../../../../packages/runtime/src/account/usage.ts");
const { memoryStore } = await import("../../../../packages/runtime/src/store.ts");
const { fakeClock } = await import("../../../../packages/runtime/test/fake-clock.ts");
const out = fileURLToPath(new URL("../tests/fixtures/oracle/usage.json", import.meta.url));

const NOW = 1_800_000_000_000;
const place = (id: string, kind: string, name: string, extra: object = {}) => ({ id, kind, name, ...extra });
const tokens = (input: number) => ({ input, output: 1, cached: 0, cacheWrite: 0, reasoning: 0 });

const base = {
  now: NOW,
  usageLogs: true,
  places: [] as any[],
  links: [] as string[],
  away: [] as string[],
  daemonRoad: true,
  localWired: true,
  localEnv: {} as Record<string, string>,
  homes: {} as Record<string, string>,
  logs: {} as Record<string, any>,
  projects: [] as any[],
  sessions: [] as string[],
  ledgerLimits: [] as any[],
  stale: [] as string[],
  accountLabels: {} as Record<string, string>,
  accountNames: {} as Record<string, string>,
  labelProbe: {} as Record<string, string[]>,
  nameProbe: [] as string[],
  keptNames: {} as Record<string, string>,
  signIns: null as null | { id: string; signIn: string }[] | "fail",
  signInsAt: {} as Record<string, Record<string, string>>,
  reports: {} as Record<string, any>,
  launch: {} as Record<string, any>,
  boxStdout: {} as Record<string, string>,
  localLines: [] as string[],
  placeFor: {} as Record<string, string>,
  spendAnswer: { outcome: "reset", said: "ok", account: null },
  fileError: undefined as string | undefined,
  boxError: {} as Record<string, string>,
  boxNever: [] as string[],
};
type World = typeof base;

/** A context over the world, the area on it, and the recorder every part writes to. */
function build(world: Partial<World>) {
  const w: World = { ...structuredClone(base), ...structuredClone(world) };
  const fc = fakeClock(w.now);
  const rec = { warns: [] as string[], asked: [] as any[], filed: [] as any[], readings: [] as any[], alerts: [] as any[], snapshots: [] as any[], used: [] as any[], rows: [] as any[], names: [] as any[], execs: [] as any[], spends: [] as any[], signInReads: 0, ready: 0 };
  H.w = w;
  H.now = fc.clock.now;
  H.rec = rec;
  H.bus = [];
  const warn = vi.spyOn(console, "warn").mockImplementation((m: unknown) => void rec.warns.push(String(m)));
  const answerFor = (id: string) => {
    const a = w.logs[id];
    if (a === undefined) throw new Error(`no logs world for ${id}`);
    if (a.never === true) return new Promise(() => {});
    if (a.error !== undefined) return Promise.reject(new Error(a.error));
    return Promise.resolve({ id: 1, ok: true, ...a });
  };
  const store = memoryStore();
  const execOk = async (id: string, script: string, o: any) => (rec.execs.push({ where: id, script, timeoutMs: o.timeoutMs, stdin: Buffer.from(o.stdin).toString("utf8") }), { stdout: w.boxStdout[id] ?? "" });
  const ctx: any = {
    opts: {
      pricesFetch: async () => ({}),
      agentsReader: w.signIns === null ? undefined : { read: async () => (rec.signInReads++, w.signIns === "fail" ? Promise.reject(new Error("no agents")) : { agents: w.signIns }) },
    },
    store,
    local: w.localWired
      ? {
          daemonRoad: w.daemonRoad ? {} : undefined,
          env: () => w.localEnv,
          home: (agent: string) => w.homes[agent],
          execStream: (o: unknown) => (script: string, so: any) => {
            rec.execs.push({ where: "here", script, env: so.env, options: o });
            return { lines: (async function* () { for (const l of w.localLines) yield l; })(), exited: Promise.resolve() };
          },
        }
      : undefined,
    placeDoor: {
      list: async () => w.places,
      channel: (id: string) => (w.links.includes(id) ? { id } : undefined),
      link: (id: string) => (w.links.includes(id) ? {} : undefined),
      nameOf: (id: string) => w.places.find(p => p.id === id)?.name ?? id,
      reportOf: async (id: string) => w.reports[id],
      exec: async (id: string, script: string, o: any) => {
        if (w.boxError[id] !== undefined) return Promise.reject(new Error(w.boxError[id]));
        if (w.boxNever.includes(id)) {
          rec.execs.push({ where: id, script, timeoutMs: o.timeoutMs, stdin: Buffer.from(o.stdin).toString("utf8") });
          return new Promise(() => {});
        }
        return execOk(id, script, o);
      },
      signInsAt: (id: string) => w.signInsAt[id],
      placeFor: async (word: string) => (w.placeFor[word] === undefined ? undefined : { placeId: w.placeFor[word] }),
    },
    bus: { emit: () => {}, on: (_t: string, l: () => void) => void H.bus.push(l) },
    clock: fc.clock,
    projectsHeld: new Map(w.projects.map(p => [p.id, p])),
    setups: { launchOf: (placeId: string, agent: string) => w.launch[`${placeId}/${agent}`] ?? { env: {} } },
    placeAway: (id: string) => w.away.includes(id),
    sessions: new Map(w.sessions.map((s, i) => [`s${i}`, { view: { claudeSessionId: s } }])),
    preferences: { get: async () => ({ usageLogs: w.usageLogs }) },
    ready: async () => void rec.ready++,
    onThisComputer: async (work: any) => work(async (frame: any) => (rec.asked.push({ place: "here", frame: JSON.stringify(frame) }), answerFor("here"))),
    overChannel: async (link: any, work: any) => work(async (frame: any) => (rec.asked.push({ place: link.id, frame: JSON.stringify(frame) }), answerFor(link.id))),
    vaultedFor: () => undefined,
    napRefusedReason: () => undefined,
    stopRefusedReason: () => undefined,
    diskReason: () => undefined,
  };
  void store.put("usage-computers", "gone", { id: "gone", name: "old-box" });
  for (const [id, name] of Object.entries(w.keptNames)) void store.put("usage-computers", id, { id, name });
  const area = usageArea(ctx);
  return { w, area, rec, fc, ctx, warn, advance: async (ms: number) => { fc.advance(ms); await vi.advanceTimersByTimeAsync(ms); } };
}

const outs: Record<string, unknown> = {};
const worlds: Record<string, unknown> = {};
const scenario = async (name: string, world: Partial<World>, steps: (b: ReturnType<typeof build>, mark: (label: string) => void) => Promise<void>) => {
  const b = build(world);
  const marks: Record<string, unknown>[] = [];
  const snap = (label: string) => marks.push({ label, asked: b.rec.asked.length, filed: b.rec.filed.length, execs: b.rec.execs.length, readings: b.rec.readings.length, warns: [...b.rec.warns], signInReads: b.rec.signInReads });
  await steps(b, snap);
  const { fc: _fc, ctx: _ctx, warn, advance: _a, area: _area, w, ...rest } = b;
  warn.mockRestore();
  worlds[name] = w;
  outs[name] = { ...rest.rec, marks };
};

it("writes the usage oracle", async () => {
  vi.useFakeTimers();
  const used = (area: any, q: any = { range: "day", split: "source" }) => area.usage.used(q);

  // The logs: here and two joined computers, one away, a provider with a link that is never read.
  const logsWorld: Partial<World> = {
    places: [place("here", "computer", "this Mac"), place("box1", "computer", "studio"), place("box2", "computer", "garage"), place("solari", "provider", "solari")],
    links: ["box1", "solari"],
    projects: [
      { id: "pH", name: "alpha", computer: "here", path: "/home/u/alpha" },
      { id: "pH2", name: "alpha-sub", computer: "here", path: "/home/u/alpha/sub" },
      { id: "pB", name: "app", computer: "box1", path: "/srv/app" },
    ],
    sessions: ["sess-wsp-here"],
    ledgerLimits: [{ key: "codex:acct_1", agent: "codex", label: "Codex with ChatGPT Plus", road: "named", computers: ["here"] }],
    stale: ["codex@box1"],
    logs: {
      here: {
        rows: [
          { agent: "claude", session: "sess-wsp-here", at: 900, model: "claude-opus-5", folder: "/home/u/alpha", tokens: tokens(99) },
          { agent: "claude", session: "s-a", at: 1000, model: "claude-opus-5", folder: "/home/u/alpha/sub/x", tokens: tokens(40), cost: 0.5 },
          { agent: "codex", session: "s-b", at: 2000, model: "gpt-5", folder: "/tmp/else", tokens: tokens(5) },
          { agent: "claude", session: "s-c", at: 3000, model: "claude-opus-5", tokens: tokens(7) },
        ],
        limits: [
          { agent: "codex", at: 5000, primary: { usedPercent: 40, windowDurationMins: 300, resetsAt: 1_790_690_000 }, planType: "pro" },
          { agent: "claude", at: 6000, primary: { usedPercent: 1 } },
        ],
      },
      box1: {
        rows: [
          { agent: "claude", session: "s-d", at: 4000, model: "claude-opus-5", folder: "/srv/app/lib", tokens: tokens(11) },
          { agent: "claude", session: "s-e", at: 4100, model: "claude-opus-5", folder: "/home/u/alpha/sub", tokens: tokens(3) },
        ],
        limits: [{ agent: "codex", at: 7000, secondary: { usedPercent: 12, windowDurationMins: 10080 } }],
      },
    },
  };
  await scenario("logs", logsWorld, async ({ area, rec }, mark) => {
    await used(area);
    mark("first");
    await used(area);
    mark("second");
  });
  await scenario("logs_window", logsWorld, async ({ area, advance }, mark) => {
    await used(area);
    mark("0");
    await advance(59_999);
    await used(area);
    mark("59999");
    await advance(1);
    await used(area);
    mark("60000");
  });
  await scenario("logs_off", { ...logsWorld, usageLogs: false }, async ({ area }, mark) => {
    await used(area, { range: "day", split: "project", outside: true });
    mark("off");
  });
  await scenario("logs_not_asked", logsWorld, async ({ area }, mark) => {
    await used(area, { range: "day", split: "project" });
    mark("not asked");
  });
  await scenario("logs_old_daemon_and_failures", { ...logsWorld, logs: { ...(logsWorld.logs as object), here: { error: unknownOpLine("usage.logs") }, box1: { error: "the socket closed" } } }, async ({ area }, mark) => {
    await used(area);
    mark("both fail");
  });
  await scenario("logs_timeout", { ...logsWorld, logs: { ...(logsWorld.logs as object), box1: { never: true } } }, async ({ area, advance }, mark) => {
    const pending = used(area);
    await vi.advanceTimersByTimeAsync(0);
    await advance(29_999);
    mark("before the bound");
    await advance(1);
    await pending;
    mark("after the bound");
  });
  await scenario("logs_without_a_daemon_here", { ...logsWorld, daemonRoad: false }, async ({ area }, mark) => {
    await used(area);
    mark("no road here");
  });

  // The sign-ins held a minute.
  const signedIn = [{ id: "claude", signIn: "signed-in" }, { id: "codex", signIn: "vault-key" }, { id: "gemini", signIn: "none" }, { id: "opencode", signIn: "not-signed-in" }];
  await scenario("accounts_here_row_added", { places: [place("box1", "computer", "studio", { signIns: { codex: "signed-in" } })], signIns: signedIn, nameProbe: ["here", "box1", "gone"], ledgerLimits: [{ key: "k" }] }, async ({ area }, mark) => {
    await area.usageAccounts();
    mark("one");
  });
  await scenario("accounts_here_row_given", { places: [place("here", "computer", "this Mac"), place("box1", "computer", "studio", { signIns: { codex: "signed-in" } })], signIns: signedIn }, async ({ area }, mark) => {
    await area.usageAccounts();
    mark("one");
  });
  await scenario("accounts_here_row_has_its_own", { places: [place("here", "computer", "this Mac", { signIns: { claude: "signed-in" } })], signIns: signedIn }, async ({ area }, mark) => {
    await area.usageAccounts();
    mark("one");
  });
  await scenario("sign_ins_hold", { places: [], signIns: signedIn }, async ({ area, advance }, mark) => {
    await Promise.all(Array.from({ length: 40 }, () => area.usageAccounts()));
    mark("40 at once");
    await advance(59_999);
    await area.usageAccounts();
    mark("59999");
    await advance(1);
    await area.usageAccounts();
    mark("60000");
    H.bus.forEach(l => l());
    await area.usageAccounts();
    mark("agents changed");
  });
  await scenario("sign_ins_failed_read_held", { places: [], signIns: "fail" }, async ({ area, advance }, mark) => {
    await area.usageAccounts();
    await area.usageAccounts();
    mark("failed twice");
    await advance(60_000);
    await area.usageAccounts();
    mark("a minute on");
  });
  await scenario("sign_ins_no_reader", { places: [], signIns: null }, async ({ area }, mark) => {
    await area.usageAccounts();
    mark("none");
  });

  // The plan reads.
  const planWorld: Partial<World> = {
    places: [
      place("here", "computer", "this Mac"),
      place("box1", "computer", "studio", { logins: "/home/maya/logins" }),
      place("box2", "computer", "garage"),
      place("box3", "computer", "attic"),
      place("box4", "computer", "pantry", { logins: "/srv/logins" }),
      place("box5", "computer", "shed"),
      place("solari", "provider", "solari"),
    ],
    links: ["box1", "box3", "box4", "box5", "solari"],
    signIns: [{ id: "claude", signIn: "signed-in" }, { id: "codex", signIn: "signed-in" }],
    signInsAt: { box1: { codex: "signed-in" }, box2: { codex: "signed-in" }, box3: { codex: "not-signed-in" }, box4: { codex: "signed-in" }, box5: { codex: "signed-in" }, solari: { codex: "signed-in" } },
    homes: { codex: "/home/u/.codex" },
    launch: { "box1/codex": { configDir: "/home/maya/.codex-work", env: {} }, "box4/codex": { env: {}, launch: { program: "wrap" } } },
    boxStdout: {
      box1: `limit:${JSON.stringify({ windows: [{ kind: "session", usedPercent: 12, resetsAt: 1_790_690_000_000 }], plan: "plus", account: { id: "acct_7f3a", label: "dev@example.com" } })}`,
      box4: `limit:${JSON.stringify({ windows: [{ kind: "week", usedPercent: 50 }] })}`,
    },
    localLines: [`limit:${JSON.stringify({ windows: [{ kind: "week", usedPercent: 3 }] })}`],
  };
  await scenario("plans", planWorld, async ({ area, advance }, mark) => {
    await area.usage.accounts({ fresh: true });
    mark("first");
    await area.usage.accounts({ fresh: true });
    mark("second");
    await advance(59_999);
    await area.usage.accounts({ fresh: true });
    mark("59999");
    await advance(1);
    await area.usage.accounts({ fresh: true });
    mark("60000");
    await area.usage.accounts();
    mark("not fresh");
  });
  await scenario("plans_here_vault_key", { ...planWorld, signIns: [{ id: "codex", signIn: "vault-key" }], places: [place("here", "computer", "this Mac")] }, async ({ area }, mark) => {
    await area.usage.accounts({ fresh: true });
    mark("vault key");
  });
  await scenario("plans_exec_error", { ...planWorld, boxError: { box1: "the link dropped" } }, async ({ area }, mark) => {
    await area.usage.accounts({ fresh: true });
    mark("exec error");
  });
  await scenario("plans_timeout", { ...planWorld, boxNever: ["box1"] }, async ({ area, advance }, mark) => {
    const pending = area.usage.accounts({ fresh: true });
    await vi.advanceTimersByTimeAsync(0);
    await advance(29_999);
    mark("before the bound");
    await advance(1);
    await pending;
    mark("after the bound");
  });
  await scenario("plans_failures", { ...planWorld, localLines: ["nothing"], boxStdout: { box1: "garbage", box4: "nothing" } }, async ({ area }, mark) => {
    await area.usage.accounts({ fresh: true });
    mark("failures");
  });

  // The reset's pieces, through the spender's own deps.
  await scenario("reset_pieces", { ...planWorld, localEnv: { PATH: "/usr/local/bin:/usr/bin", HOME: "/home/u", WSP_THREAD_HOME: "/x" }, reports: { box1: { login: { PATH: "/opt/codex/bin:/usr/bin" } } }, launch: { "here/codex": { env: { CODEX_GATEWAY_TOKEN: "sk-test-0123" } }, "here/claude": { configDir: "/custom/claude", env: {} }, "box1/codex": { env: { CODEX_GATEWAY_TOKEN: "sk-test-0123" } }, "box3/codex": { configDir: "/x/.codex-w", env: {}, launch: { args: ["--x"] } }, "box4/codex": { env: {}, launch: { program: "wrap" } }, "box5/codex": { env: { CODEX_GATEWAY_TOKEN: "sk-test-0123" } } }, localLines: ["a", "b"], boxStdout: { box1: "boxed" } }, async ({ area, rec }) => {
    const o = H.resetOpts;
    const placeOf = await o.places();
    const table: Record<string, unknown> = {};
    for (const id of ["here", "box1", "box2", "box3", "solari", "unknown"]) {
      const p = placeOf(id);
      table[id] = { id: p.id, name: p.name, kind: p.kind, connected: p.connected, codexSignedIn: p.signedIn("codex"), claudeSignedIn: p.signedIn("claude") };
    }
    (rec as any).placeTable = table;
    const run: Record<string, unknown> = {};
    run["box"] = await o.run(placeOf("box1"), "echo hi", "codex");
    run["here"] = await o.run(placeOf("here"), "echo hi", "codex");
    run["box_no_path"] = await o.run(placeOf("box5"), "echo hi", "codex");
    (rec as any).runs = run;
    (rec as any).envInput = Buffer.from(envInput({ PATH: "/opt/codex/bin:/usr/bin", CODEX_GATEWAY_TOKEN: "sk-test-0123" })).toString("utf8");
    (rec as any).envInputOnlyLaunch = Buffer.from(envInput({ CODEX_GATEWAY_TOKEN: "sk-test-0123" })).toString("utf8");
    (rec as any).scriptWithEnv = withEnvFromInput("echo hi");
    const road: Record<string, unknown> = {};
    const tryRoad = async (name: string, agent: string, id: string) => {
      try {
        road[name] = await o.road(agent, placeOf(id));
      } catch (e) {
        road[name] = { error: (e as Error).message };
      }
    };
    await tryRoad("here", "codex", "here");
    await tryRoad("here_config_dir", "claude", "here");
    await tryRoad("box_logins", "codex", "box1");
    await tryRoad("box_config_dir_and_args_only", "codex", "box3");
    await tryRoad("box_logins_and_program", "codex", "box4");
    await tryRoad("box_no_logins", "codex", "box2");
    await tryRoad("provider_no_logins", "codex", "solari");
    (rec as any).roads = road;
    const key = await o.file({ agent: "codex", place: placeOf("box1"), limit: { windows: [], account: { id: "acct_7f3a", label: "dev@example.com" } } });
    const key2 = await o.file({ agent: "codex", place: placeOf("box1"), limit: { windows: [] } });
    (rec as any).filed_keys = [key, key2];
  });
  await scenario("reset_here_absent", { ...planWorld, localWired: false }, async ({ area, rec }) => {
    const o = H.resetOpts;
    const placeOf = await o.places();
    (rec as any).placeHere = { name: placeOf("here").name, connected: placeOf("here").connected };
    const trys: Record<string, unknown> = {};
    for (const [n, f] of [["run", () => o.run(placeOf("here"), "echo hi", "codex")], ["road", () => o.road("codex", placeOf("here"))]] as const) {
      try {
        await f();
        trys[n] = "ok";
      } catch (e) {
        trys[n] = (e as Error).message;
      }
    }
    (rec as any).trys = trys;
  });
  await scenario("reset_names_the_place", { placeFor: { srv: "box1" } }, async ({ area, rec }) => {
    await area.usage.reset({ account: "codex@here", on: "srv" });
    await area.usage.reset({ account: "codex@here", on: "nowhere", creditId: "c1" });
    await area.usage.reset({ account: "codex@here" });
  });

  // The used labels.
  await scenario("used_labels", {
    places: [place("here", "computer", "this Mac"), place("box1", "computer", "studio")],
    projects: [{ id: "p1", name: "alpha", computer: "here", path: "/a" }],
    keptNames: { gone: "old-box" },
    accountLabels: { k1: "Filed label", k2: "Other filed" },
    accountNames: { k2: "Read name", k3: "Only read" },
    labelProbe: { agent: ["claude", "nobody"], project: ["", "p1", "pX"], computer: ["here", "box1", "gone", "unknown"], account: ["k1", "k2", "k3", "k4"], source: ["log", "wsp", "anything"] },
    nameProbe: ["here", "gone"],
  }, async ({ area }, mark) => {
    await used(area, { range: "week", split: "agent" });
    mark("agent");
  });
  await scenario("used_flag_outside", { ...logsWorld }, async ({ area }, mark) => {
    await used(area, { range: "day", split: "agent", outside: true });
    mark("outside");
  });
  await scenario("used_flag_source", { ...logsWorld }, async ({ area }, mark) => {
    await used(area, { range: "day", split: "source" });
    mark("source");
  });
  await scenario("used_flag_outside_false", { ...logsWorld }, async ({ area }, mark) => {
    await used(area, { range: "day", split: "agent", outside: false });
    mark("outside false");
  });
  await scenario("used_logs_failure", { ...logsWorld, fileError: "the ledger is gone" }, async ({ area }, mark) => {
    await used(area, { range: "day", split: "agent", outside: true });
    mark("filing fails");
  });
  await scenario("used_outside_off", { ...logsWorld, usageLogs: false }, async ({ area }) => {
    await used(area, { range: "day", split: "source", outside: true });
  });

  const words = {
    absentSrv: absentComputer("srv", null).sentence,
    absentMini: absentComputer("mini", null).sentence,
    absentThis: absentComputer(THIS_COMPUTER, null).sentence,
    subGone: subCloudGoneLine(cloudSaid("solari")),
    noLogins: resetNoLoginsLine("srv", "Codex"),
    noLoginsMini: resetNoLoginsLine("garage", "Codex"),
    unknownOp: unknownOpLine("usage.logs"),
    thisComputer: THIS_COMPUTER,
    outsideWsp: USAGE_WORDS.outsideWsp,
    wspThreads: USAGE_WORDS.wspThreads,
    removedComputer: USAGE_WORDS.removedComputer,
    unread: USAGE_WORDS.unread,
    noPlaceDoor: "",
  };
  words.noPlaceDoor = NO_PLACE_DOOR;
  const stores = JSON.parse((outs["logs"] as any).asked[0].frame).stores;
  const away = await (async () => {
    const b = build({ places: [place("box1", "computer", "studio")], away: ["box1"] });
    const rows: Record<string, unknown> = {};
    rows["cloud_gone"] = b.area.awayLine({ phase: "running", sub: { on: "solari", cloudGone: true } } as any) ?? null;
    rows["cloud_gone_released"] = b.area.awayLine({ phase: "released", sub: { on: "solari", cloudGone: true } } as any) ?? null;
    rows["cloud_gone_released_on_away_box"] = b.area.awayLine({ phase: "released", place: "box1", sub: { on: "solari", cloudGone: true } } as any) ?? null;
    rows["sub_not_gone"] = b.area.awayLine({ phase: "running", sub: { on: "solari" } } as any) ?? null;
    rows["away_box"] = b.area.awayLine({ phase: "running", place: "box1" } as any) ?? null;
    rows["present_box"] = b.area.awayLine({ phase: "running", place: "box2" } as any) ?? null;
    rows["here"] = b.area.awayLine({ phase: "running" } as any) ?? null;
    b.warn.mockRestore();
    return rows;
  })();

  writeFileSync(out, JSON.stringify({ words, stores, away, scenarios: outs, worlds }, null, 2));
});
