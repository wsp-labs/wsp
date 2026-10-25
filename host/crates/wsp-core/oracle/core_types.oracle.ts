// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the core-types tests pin, for runtime/src/types at 65f6b89c7: one JSON file per unit under
// tests/fixtures/core_types/. randomUUID is pinned. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-core/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

vi.mock("node:crypto", async importOriginal => {
  const real = await importOriginal<typeof import("node:crypto")>();
  return { ...real, randomUUID: () => "11111111-2222-4333-8444-555555555555" };
});

const T = "../../../../packages/runtime/src/types";
const events = await import(`${T}/events.ts`);
const harness = await import(`${T}/harness.ts`);
const wiring = await import(`${T}/wiring.ts`);
const internal = await import(`${T}/internal.ts`);

const out = fileURLToPath(new URL("../tests/fixtures/core_types/", import.meta.url));
const write = (name: string, value: unknown): void => {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, `${name}.json`), `${JSON.stringify(value, null, 1)}\n`);
};
const tryIt = (f: () => unknown): unknown => {
  try {
    return { ok: f() ?? null };
  } catch (e) {
    return { err: (e as Error).message };
  }
};
const tryAsync = async (f: () => Promise<unknown>): Promise<unknown> => {
  try {
    return { ok: (await f()) ?? null };
  } catch (e) {
    return { err: (e as Error).message, name: (e as Error).name };
  }
};

const renamed = (n: number): Record<string, unknown> => ({ type: "workspace.renamed", workspaceId: `w${n}`, name: `n${n}` });

it("events", () => {
  const scenes: Record<string, unknown> = {};
  {
    const bus = events.eventBus();
    const seen: unknown[] = [];
    bus.on("*", (e: unknown) => seen.push(["any", e]));
    bus.on("workspace.renamed", (e: unknown) => seen.push(["typed", e]));
    scenes.fresh = bus.since(0, undefined);
    scenes.freshAfter1 = bus.since(1, undefined);
    scenes.freshEmptyStream = bus.since(0, "");
    scenes.freshNoAfterForeign = bus.since(undefined, "other");
    bus.pass(renamed(0));
    bus.emit(renamed(1));
    bus.emit(renamed(2));
    bus.pass(renamed(3));
    scenes.delivered = seen;
    scenes.after0 = bus.since(0, undefined);
    scenes.after2 = bus.since(2, undefined);
    scenes.after3 = bus.since(3, undefined);
    scenes.after1Own = bus.since(1, "11111111-2222-4333-8444-555555555555");
    scenes.after1Foreign = bus.since(1, "x");
  }
  {
    const bus = events.eventBus();
    for (let i = 1; i <= 5003; i++) bus.emit(renamed(i));
    const a = bus.since(0, undefined);
    const b = bus.since(2, undefined);
    const c = bus.since(3, undefined);
    const d = bus.since(5003, undefined);
    const e = bus.since(5002, undefined);
    scenes.wrapped = {
      after0: { gap: a.gap, head: a.head, n: a.events.length },
      after2: { gap: b.gap, n: b.events.length },
      after3: { gap: c.gap, n: c.events.length, first: c.events[0], last: c.events.at(-1) },
      after5003: { gap: d.gap, n: d.events.length },
      after5002: { gap: e.gap, n: e.events.length, first: e.events[0] },
    };
  }
  {
    const bus = events.eventBus();
    const order: string[] = [];
    let offB: (() => void) | undefined;
    bus.on("*", (e: { workspaceId: string; seq: number }) => order.push(`any1:${e.workspaceId}:${e.seq}`));
    bus.on("workspace.renamed", (e: { workspaceId: string; seq: number }) => {
      order.push(`typed1:${e.workspaceId}:${e.seq}`);
      if (e.workspaceId === "w1") {
        bus.emit(renamed(9));
        offB?.();
        bus.on("workspace.renamed", (x: { workspaceId: string; seq: number }) => order.push(`added:${x.workspaceId}:${x.seq}`));
      }
    });
    offB = bus.on("workspace.renamed", (e: { workspaceId: string; seq: number }) => order.push(`typedB:${e.workspaceId}:${e.seq}`));
    bus.emit(renamed(1));
    scenes.nested = order;
  }
  {
    const bus = events.eventBus();
    const order: string[] = [];
    bus.on("workspace.renamed", () => {
      order.push("first");
      throw new Error("listener broke");
    });
    bus.on("workspace.renamed", () => order.push("second"));
    bus.on("*", () => order.push("any"));
    const thrown = tryIt(() => bus.emit(renamed(1)));
    scenes.throwing = { order, thrown, since: bus.since(0, undefined) };
  }
  {
    const bus = events.eventBus();
    const order: string[] = [];
    const off = bus.on("workspace.renamed", () => order.push("x"));
    off();
    off();
    bus.on("workspace.renamed", () => order.push("y"));
    bus.emit(renamed(1));
    scenes.unlistenTwice = order;
  }
  scenes.constants = {
    EVENT_RING_CAP: events.EVENT_RING_CAP,
    SLATE_PR_POLL_MS: events.SLATE_PR_POLL_MS,
    CATALOG_TTL_MS: events.CATALOG_TTL_MS,
    CATALOG_PROBE_TIMEOUT_MS: events.CATALOG_PROBE_TIMEOUT_MS,
    FIRST_RUN_READ_MS: events.FIRST_RUN_READ_MS,
    AGENT_VERSION_READ_MS: events.AGENT_VERSION_READ_MS,
    SESSION_TITLE_TTL_MS: events.SESSION_TITLE_TTL_MS,
    CHECKOUT_TTL_MS: events.CHECKOUT_TTL_MS,
    PR_PAGE_HOLD_MS: events.PR_PAGE_HOLD_MS,
    RATE_LIMIT_HOLD_MS: events.RATE_LIMIT_HOLD_MS,
    SESSION_TITLE_TIMEOUT_MS: events.SESSION_TITLE_TIMEOUT_MS,
    SESSION_TITLE_REFRESH_MAX: events.SESSION_TITLE_REFRESH_MAX,
    TITLE_MAKE_TIMEOUT_MS: events.TITLE_MAKE_TIMEOUT_MS,
  };
  write("events", scenes);
});

it("harness", () => {
  write("harness", {
    machineSandbox: Object.entries(harness.MACHINE_SANDBOX_ENV),
    guestLogin: Object.entries(harness.GUEST_LOGIN_ENV),
    placeLogin: Object.entries(harness.PLACE_LOGIN_ENV),
    onGuest: Object.entries(harness.loginEnvOn(undefined)),
    onPlace: Object.entries(harness.loginEnvOn("p1")),
    onGuestNpm: Object.entries(harness.loginEnvOn(undefined, "/opt/npm/bin")),
    onPlaceNpm: Object.entries(harness.loginEnvOn("p1", "/opt/npm/bin")),
    onEmptyPlace: Object.entries(harness.loginEnvOn("")),
  });
});

const agentResult = (o: Record<string, unknown>): Record<string, unknown> => ({ agent: "claude", files: 1, bytes: 2, ...o });

it("wiring", async () => {
  const lastLines = ["", "a", "a\nb\n  ", "a\n\n", "  x  ", "a\r\nb", "a\nb\r", "x\n  \n", "\n\n", "a\n  b  \n", "only\t"];
  const outcomes = [
    agentResult({ outcome: "moved" }),
    agentResult({ outcome: "moved", rows: 0 }),
    agentResult({ outcome: "moved", rows: 1 }),
    agentResult({ outcome: "moved", rows: 3, note: "kept 2" }),
    agentResult({ outcome: "moved", note: "n" }),
    agentResult({ outcome: "transcript-only" }),
    agentResult({ outcome: "transcript-only", note: "later" }),
    agentResult({ outcome: "carried", note: "ignored" }),
    agentResult({ outcome: "nothing", rows: 4 }),
    agentResult({ outcome: "failed" }),
    agentResult({ outcome: "failed", error: "boom", note: "x" }),
  ];
  const homes = [
    { ...agentResult({ outcome: "moved" }), name: "Claude Code" },
    { ...agentResult({ outcome: "moved", sessions: 0 }), name: "Codex" },
    { ...agentResult({ outcome: "moved", sessions: 1, skipped: 0 }), name: "Codex" },
    { ...agentResult({ outcome: "transcript-only", sessions: 2, skipped: 1 }), name: "Codex" },
    { ...agentResult({ outcome: "nothing", skipped: 3 }), name: "Cursor" },
    { ...agentResult({ outcome: "carried" }), name: "OpenCode" },
    { ...agentResult({ outcome: "failed", sessions: 1 }), name: "Claude Code" },
    { ...agentResult({ outcome: "failed", error: "disk full", skipped: 2 }), name: "Claude Code" },
  ];
  const hostnames = ["My Box!", "---", "", "a--b", "ÉCOLE", "Café 2", "İstanbul", "x".repeat(70), `${"a".repeat(62)}-b`, `${"a".repeat(62)}--b`, "  spaced  out  ", "UPPER_case.name", "😀 emoji", "-lead-trail-"];
  const shapes = [
    [{}, {}],
    [{ cpu: 2, memMb: 4096 }, { cpu: 2, memMb: 4096 }],
    [{ cpu: 2, memMb: 4096 }, { cpu: 4, memMb: 2048 }],
    [{ cpu: 2 }, { memMb: 8 }],
    [{ cpu: 2, memMb: 4096, diskGb: 10 }, { cpu: 2, memMb: 4096, diskGb: 20, createdAt: "x" }],
    [{ memMb: 1.5 }, { memMb: 2 }],
  ];
  const sources = [
    [{ kind: "git", url: "u" }, { kind: "git", url: "u" }],
    [{ kind: "git", url: "u" }, { kind: "git", url: "v" }],
    [{ kind: "folder", path: "/a" }, { kind: "folder", path: "/a" }],
    [{ kind: "folder", path: "/a" }, { kind: "folder", path: "/a/" }],
    [{ kind: "git", url: "/a" }, { kind: "folder", path: "/a" }],
  ];
  const machine = (run: () => Promise<unknown>, log: string[]): unknown => ({
    id: "m1",
    run: async (cmd: string, o: unknown) => {
      log.push(`run ${cmd} ${JSON.stringify(o)}`);
      return run();
    },
    exec: async (cmd: string) => {
      log.push(`exec ${cmd}`);
      if (cmd.startsWith("rm")) throw new Error("cleanup refused");
      return { exitCode: 0, stdout: "", stderr: "" };
    },
  });
  const merges: unknown[] = [];
  const mergeCases: [string, () => Promise<unknown>][] = [
    ["exit2", async () => ({ exitCode: 2, stdout: "", stderr: "first\nlast line  \n" })],
    ["exit1Quiet", async () => ({ exitCode: 1, stdout: "{}", stderr: "" })],
    ["merged", async () => ({ exitCode: 0, stdout: 'noise\n{"merged": 3, "kept": 1}\n', stderr: "" })],
    ["mergedNote", async () => ({ exitCode: 0, stdout: '{"merged": 0, "kept": 0, "note": "store empty"}', stderr: "" })],
    ["waiting", async () => ({ exitCode: 0, stdout: '{"waiting": "no store yet", "merged": 1, "kept": 1}', stderr: "" })],
    ["empty", async () => ({ exitCode: 0, stdout: "", stderr: "" })],
    ["notJson", async () => ({ exitCode: 0, stdout: "done", stderr: "" })],
    ["runThrows", async () => { throw new Error("machine gone"); }],
  ];
  for (const [name, run] of mergeCases) {
    const log: string[] = [];
    const res = await tryAsync(() => wiring.mergeOnMachine(machine(run, log) as never, "/tmp/wsp-merge/x y/claude.py", agentResult({ outcome: "nothing", note: "before" }) as never));
    merges.push({ name, res, log });
  }
  const hostSet: unknown[] = [];
  for (const [name, exec] of [
    ["ok", async () => ({ exitCode: 0, stdout: "", stderr: "" })],
    ["refused", async () => ({ exitCode: 1, stdout: "", stderr: "  hostname: you must be root \n" })],
    ["throws", async () => { throw new Error("socket closed"); }],
  ] as const) {
    const log: string[] = [];
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const res = await wiring.setHostname({ id: "m9", exec: async (c: string) => { log.push(c); return exec(); } } as never, "My Box!");
    hostSet.push({ name, res, log, warned: warn.mock.calls.map(c => c[0]) });
    warn.mockRestore();
  }
  const fakeClock = () => {
    let now = 1000;
    const timers: { at: number; fn: () => void; on: boolean }[] = [];
    const log: string[] = [];
    return {
      log,
      now: () => now,
      schedule(fn: () => void, ms: number) {
        log.push(`schedule ${ms}`);
        const t = { at: now + ms, fn, on: true };
        timers.push(t);
        return () => { log.push("cancel"); t.on = false; };
      },
      advance(ms: number) {
        now += ms;
        for (const t of timers) if (t.on && t.at <= now) { t.on = false; t.fn(); }
      },
    };
  };
  const untils: Record<string, unknown> = {};
  {
    const c = fakeClock();
    const p = wiring.until(new Promise(() => {}), 1500, "the probe", c as never);
    c.advance(600);
    untils.deadline = { res: await tryAsync(() => p), log: c.log, isDeadline: await p.catch((e: unknown) => e instanceof wiring.DeadlineError) };
  }
  {
    const c = fakeClock();
    const p = wiring.until(Promise.resolve(7), 1500, "x", c as never);
    untils.resolves = { res: await tryAsync(() => p), log: c.log };
  }
  {
    const c = fakeClock();
    const p = wiring.until(Promise.reject(new Error("own")), 1500, "x", c as never);
    untils.rejects = { res: await tryAsync(() => p), log: c.log };
  }
  {
    const c = fakeClock();
    const p = wiring.until(new Promise(() => {}), 400, "late", c as never);
    c.advance(0);
    untils.past = { res: await tryAsync(() => p), log: c.log };
  }
  const backend = { name: "b" };
  let id = "solari";
  const fixed = wiring.wiredPlace("solari", backend as never);
  const moving = wiring.wiredPlace(() => id, backend as never);
  const wired: Record<string, unknown> = {
    fixed: { wired: fixed.wired, list: fixed.list(), solari: fixed.backend("solari") === backend, other: fixed.backend("boat") === undefined },
  };
  id = "boat";
  wired.moving = { wired: moving.wired, list: moving.list(), boat: moving.backend("boat") === backend, solari: moving.backend("solari") === undefined };
  write("wiring", {
    lastLines: lastLines.map(s => [s, wiring.lastLineOf(s)]),
    outcomes: outcomes.map(a => [a, wiring.outcomeWords(a as never)]),
    homes: homes.map(a => [a, wiring.homeOutcome(a as never)]),
    hostnames: hostnames.map(s => [s, wiring.hostnameFor(s)]),
    shapes: shapes.map(([a, b]) => [a, b, wiring.shapeFault(a as never, b as never) ?? null]),
    sources: sources.map(([a, b]) => [a, b, wiring.sameSource(a as never, b as never)]),
    atProvider: [
      [null, wiring.atProvider(undefined, { list: () => ["solari"] })],
      ["solari", wiring.atProvider("solari", { list: () => ["solari"] })],
      ["mac", wiring.atProvider("mac", { list: () => ["solari"] })],
      ["", wiring.atProvider("", { list: () => [""] })],
    ],
    settled: ["settled", "not-gone", "unchecked", "moot"].map(o => [o, wiring.settled(o as never)]),
    merges,
    hostSet,
    untils,
    wired,
    constants: {
      PROVIDER_READ_MS: wiring.PROVIDER_READ_MS,
      GONE_CONFIRM_MS: wiring.GONE_CONFIRM_MS,
      WAKE_LATE_READ_MS: wiring.WAKE_LATE_READ_MS,
      DAEMON_HELLO_TIMEOUT_MS: wiring.DAEMON_HELLO_TIMEOUT_MS,
      PORT_PROBE_TIMEOUT_MS: wiring.PORT_PROBE_TIMEOUT_MS,
      CLONE_MS: wiring.CLONE_MS,
      ADD_IS_A_COMPUTER_LINE: wiring.ADD_IS_A_COMPUTER_LINE,
      NO_COPIER_HERE: wiring.NO_COPIER_HERE,
      MERGE_DEADLINE_MS: wiring.MERGE_DEADLINE_MS,
      LISTING_DEADLINE_MS: wiring.LISTING_DEADLINE_MS,
      PORT_PROBE_BODY_CAP: wiring.PORT_PROBE_BODY_CAP,
      VAULT_CAP_BYTES: wiring.VAULT_CAP_BYTES,
      VAULTS: wiring.VAULTS,
      VAULT_SKIP: [...wiring.VAULT_SKIP],
      GRACE_MS: wiring.GRACE_MS,
      DAEMON_REVIVE_AGAIN_MS: wiring.DAEMON_REVIVE_AGAIN_MS,
      DAEMON_LACKS_AGAIN_MS: wiring.DAEMON_LACKS_AGAIN_MS,
      LOG_READ_EVERY_MS: wiring.LOG_READ_EVERY_MS,
      LOG_READ_MS: wiring.LOG_READ_MS,
      RESET_EXEC_MS: wiring.RESET_EXEC_MS,
      PLAN_READ_EVERY_MS: wiring.PLAN_READ_EVERY_MS,
      OUTCOME_WORDS: wiring.OUTCOME_WORDS,
      HOME_WORDS: wiring.HOME_WORDS,
    },
  });
});

const W = "w1";
const ev = (type: string, o: Record<string, unknown>): Record<string, unknown> => ({ type, workspaceId: W, sessionId: "s1", ...o });

it("internal", async () => {
  const fold = (events: Record<string, unknown>[]): unknown => JSON.parse(internal.indexBytes(internal.indexOf(events as never), undefined).toString("utf8"));
  const scenes: Record<string, unknown> = {};
  const chat = [
    ev("session.start", { threadId: "t1", turnId: "u1", at: 10, pos: 1, prompt: "Find the BUG in parser", requestId: "r1", cwd: "/w", model: "opus" }),
    ev("session.delta", { threadId: "t1", turnId: "u1", at: 11, pos: 2, kind: "text", text: "Looking ", messageId: "m1" }),
    ev("session.delta", { threadId: "t1", turnId: "u1", at: 12, pos: 3, kind: "text", text: "now.", messageId: "m1" }),
    ev("session.delta", { threadId: "t1", turnId: "u1", at: 13, pos: 4, kind: "text", text: "sub says", messageId: "m1", parentToolUseId: "tool9" }),
    ev("session.delta", { threadId: "t1", turnId: "u1", at: 14, pos: 5, kind: "tool_use", text: "ls" }),
    ev("session.delta", { threadId: "t1", turnId: "u1", at: 15, pos: 6, kind: "text", text: "Second message", messageId: "m2" }),
    ev("session.steer", { threadId: "t1", turnId: "u1", at: 16, pos: 7, prompt: "also tests", requestId: "r2" }),
    ev("session.delta", { threadId: "t1", turnId: "u1", at: 17, pos: 8, kind: "text", text: "ok", messageId: "m2" }),
    ev("session.end", { threadId: "t1", turnId: "u1", at: 18, pos: 9, exitCode: null, sawResult: false }),
    ev("session.start", { sessionId: "s2", threadId: "t2", turnId: "u2", at: 20, pos: 10, prompt: "😀 emoji first", permissionMode: "plan" }),
    ev("session.earlier", { sessionId: "s2", threadId: "t2", at: 5, who: "person", text: "earlier ask" }),
    ev("session.earlier", { sessionId: "s2", threadId: "t2", at: 6, who: "tool", text: "tool row" }),
    ev("session.end", { sessionId: "s2", threadId: "t2", turnId: "u2", at: 21, pos: 11, exitCode: 0, sawResult: true }),
    ev("session.delta", { threadId: "t3", turnId: "u3", at: 30, pos: 12, kind: "text", text: "copied words", copiedFrom: "t1" }),
    ev("session.start", { threadId: "t3", turnId: "u3", at: 31, pos: 13, prompt: "copied", copiedFrom: "t1", requestId: "r3" }),
    ev("session.delta", { turnId: "u9", at: 40, pos: 14, kind: "text", text: "no thread" }),
    ev("session.end", { threadId: "t2", turnId: "u4", at: 41, pos: 15, exitCode: null, sawResult: false, unstarted: true }),
  ];
  scenes.chat = { events: chat, index: fold(chat) };
  const promptless = [
    ev("session.start", { threadId: "t1", turnId: "u1", at: 1, prompt: "a", requestId: "r1" }),
    ev("session.steer", { threadId: "t1", turnId: "u1", at: 2, prompt: "b", requestId: "r2" }),
    ev("session.start", { threadId: "t1", turnId: "u2", at: 3, prompt: "c", requestId: "r3" }),
    ev("session.end", { threadId: "t1", turnId: "u1", at: 4, exitCode: null, sawResult: false, promptless: true }),
  ];
  scenes.promptless = { events: promptless, index: fold(promptless) };
  const kids = [
    ev("session.start", { threadId: "t1", turnId: "u1", at: 1, prompt: "go" }),
    ev("session.subagent", { threadId: "t1", turnId: "u1", at: 2, task: "k1", state: "running", title: "Explore", depth: 1, model: "haiku", asked: "look" }),
    ev("session.subagent", { threadId: "t1", turnId: "u1", at: 3, task: "k1", state: "running", usage: { tokens: { input: 10, output: 2, cached: 0, cacheWrite: 0, reasoning: 0 }, toolUses: 3, durationMs: 50 } }),
    ev("session.subagent", { threadId: "t1", turnId: "u1", at: 4, task: "k2", state: "running" }),
    ev("session.subagent", { threadId: "t1", turnId: "u1", at: 5, task: "k2", state: "done", summary: "line one\nfinal line\n" }),
    ev("session.subagent", { threadId: "t1", turnId: "u1", at: 6, task: "k3", state: "running", title: "Fail" }),
    ev("session.subagent", { threadId: "t1", turnId: "u1", at: 7, task: "k3", state: "failed", summary: "why it failed\nmore" }),
    ev("session.subagent", { threadId: "t1", turnId: "u1", at: 8, task: "ghost", state: "done", summary: "no start" }),
    ev("session.subagent", { threadId: "t1", turnId: "u0", at: 9, task: "k4", state: "running" }),
    ev("session.end", { threadId: "t1", turnId: "u1", at: 10, exitCode: 0, sawResult: true }),
    ev("session.subagent", { turnId: "u1", at: 11, task: "nothread", state: "running" }),
  ];
  scenes.children = { events: kids, index: fold(kids) };
  {
    const index = internal.indexOf(kids as never);
    internal.forgetChild(index, kids[1] as never);
    const afterWrongRow = JSON.parse(internal.indexBytes(index, undefined).toString("utf8"));
    internal.forgetChild(index, { ...kids[1], at: 3 } as never);
    const afterOwnRow = JSON.parse(internal.indexBytes(index, undefined).toString("utf8"));
    const lone = internal.indexOf([kids[8]] as never);
    internal.forgetChild(lone, kids[8] as never);
    scenes.forget = { afterWrongRow, afterOwnRow, lone: JSON.parse(internal.indexBytes(lone, undefined).toString("utf8")) };
  }
  {
    const index = internal.indexOf(chat as never);
    const cold = JSON.parse(internal.indexBytes(index, { bytes: 99, at: 7 }, false).toString("utf8"));
    scenes.indexBytesCold = cold;
    scenes.indexBytesRaw = internal.indexBytes(index, { bytes: 99, at: 7 }).toString("utf8");
  }
  const reads: Record<string, unknown> = {};
  const roundTrip = (s: string): unknown => {
    const r = internal.indexRead(Buffer.from(s, "utf8"));
    return r === undefined ? null : { of: r.of ?? null, index: JSON.parse(internal.indexBytes(r.index, r.of).toString("utf8")) };
  };
  reads.full = roundTrip(scenes.indexBytesRaw as string);
  reads.notJson = roundTrip("{nope");
  reads.noTaken = roundTrip('{"words":[],"starts":[],"cut":[],"facts":[],"children":[],"pos":0}');
  reads.noChildren = roundTrip('{"words":[],"starts":[],"cut":[],"facts":[],"taken":[],"pos":0}');
  reads.noPos = roundTrip('{"words":[],"starts":[],"cut":[],"facts":[],"taken":[],"children":[]}');
  reads.oldWords = roundTrip('{"words":[["t1",{"lines":["hello there","x"],"last":5}],["t2",{"lines":[],"last":0}]],"starts":[],"cut":[],"facts":[],"taken":[],"children":[],"pos":3}');
  reads.noTrimmed = roundTrip('{"words":[],"starts":[["t1","s1"]],"cut":[["t1",true]],"facts":[["s1",{"model":"m"}]],"taken":[],"children":[["t1",[["k",{"id":"k","title":"k","state":"running","startedAt":1}]]]],"pos":9}');
  reads.empty = roundTrip("");
  scenes.reads = reads;
  {
    const w = internal.threadWordsOf(chat as never, "t1");
    const long = { lines: ["aaaa", "bbbbbb", "cc", "😀😀"], open: undefined, last: 9, size: 16, first: "aaaa" };
    scenes.words = {
      t1: w,
      none: internal.threadWordsOf(chat as never, "nobody"),
      cold: internal.coldWords({ ...long, opened: 4 } as never),
      newestFits: internal.newestOf(long as never, 16),
      newest6: internal.newestOf(long as never, 6),
      newest5: internal.newestOf(long as never, 5),
      newest0: internal.newestOf(long as never, 0),
      newestOfCold: internal.newestOf({ ...long, cold: true } as never, 100),
      heldWarm: internal.heldOf(long as never),
      heldCold: internal.heldOf({ ...long, lines: ["ab", "😀"], cold: true } as never),
      firstOfSplit: JSON.stringify(internal.threadWordsOf([ev("session.steer", { threadId: "t", prompt: `${"é".repeat(199)}😀tail` })] as never, "t").first),
      firstOfLong: internal.threadWordsOf([ev("session.steer", { threadId: "t", prompt: `${"é".repeat(150)}${"😀".repeat(40)}` })] as never, "t").first,
    };
  }
  {
    const index = internal.indexOf(chat as never);
    index.words.set("t9", { lines: [], open: undefined, last: 50, size: 40, cold: true } as never);
    index.words.set("t8", { lines: ["needle in a cold thread"], open: undefined, last: 51, size: 400, cold: true, room: 30 } as never);
    const found: unknown[] = [];
    const skipped = internal.searchIndex("w1", index, "bug", () => true, found as never);
    const found2: unknown[] = [];
    const skipped2 = internal.searchIndex("w1", index, "needle", (t: string) => t !== "t9", found2 as never);
    const found3: unknown[] = [];
    internal.searchIndex("w1", index, "ok", () => true, found3 as never);
    const found4: unknown[] = [];
    internal.searchIndex("w1", index, "emoji", () => true, found4 as never);
    const text = `${"x ".repeat(40)}target${" tail\n\t".repeat(30)}`;
    scenes.search = {
      bug: { found, skipped },
      needle: { found: found2, skipped: skipped2 },
      ok: found3,
      emoji: found4,
      snippets: [
        [text, text.indexOf("target"), 6, internal.snippetAround(text, text.indexOf("target"), 6)],
        ["short target here", 6, 6, internal.snippetAround("short target here", 6, 6)],
        ["😀😀😀 hit", 7, 3, internal.snippetAround("😀😀😀 hit", 7, 3)],
        [`${"😀".repeat(30)}hit${"😀".repeat(50)}`, 60, 3, internal.snippetAround(`${"😀".repeat(30)}hit${"😀".repeat(50)}`, 60, 3)],
        ["split", 41, 3, JSON.stringify(internal.snippetAround(`a${"😀".repeat(30)}hit`, 61, 3))],
        ["   lead and trail   ", 3, 4, internal.snippetAround("   lead and trail   ", 3, 4)],
      ],
    };
  }
  {
    const index = internal.indexOf(chat as never);
    const t1 = chat.filter(e => e.threadId === "t1").slice(4);
    internal.reindexThreads(index, new Set(["t1"]), (t: string) => (t === "t1" ? t1 : []) as never);
    scenes.reindex = JSON.parse(internal.indexBytes(index, undefined).toString("utf8"));
  }
  const turn = [
    ev("session.start", { threadId: "t1", turnId: "u1", at: 1 }),
    ev("session.delta", { threadId: "t1", turnId: "u1", kind: "text", text: "a", line: 1 }),
    ev("session.delta", { threadId: "t1", turnId: "u1", kind: "text", text: "b", line: 7 }),
    ev("session.delta", { threadId: "t1", turnId: "u1", kind: "text", text: "c" }),
    ev("session.subagent", { threadId: "t1", turnId: "u1", task: "k", state: "running", line: 2 }),
    ev("session.compacted", { threadId: "t1", turnId: "u1", line: 3 }),
    ev("session.retry", { threadId: "t1", turnId: "u1", attempt: 1, line: 4 }),
    ev("session.retry", { threadId: "t1", turnId: "u1", attempt: 2 }),
    ev("session.delta", { threadId: "t1", turnId: "u2", kind: "text", text: "other", line: 99 }),
  ];
  scenes.turnWritten = {
    u1: internal.turnWritten(turn as never, "u1"),
    u1Done: internal.turnWritten([...turn, ev("session.done", { threadId: "t1", turnId: "u1", result: { status: "completed" } }), ev("session.changes", { threadId: "t1", turnId: "u1", from: "a", to: "b", files: [], moved: [] })] as never, "u1"),
    startOnly: internal.turnWritten([turn[0]] as never, "u1"),
    evicted: internal.turnWritten(turn.slice(2) as never, "u1"),
    retryUnlined: internal.turnWritten([turn[7]] as never, "u1"),
    none: internal.turnWritten(turn as never, "u7"),
    doneOnly: internal.turnWritten([ev("session.done", { turnId: "u5", result: { status: "failed", error: "x" } })] as never, "u5"),
  };
  {
    const view = { id: "s1", workspaceId: W, harness: "claude", status: "completed", lastLine: "said", failure: "why", subagents: [{ id: "k", title: "K", state: "done", asked: "q", lastLine: "l", failure: "f", startedAt: 1, depth: 1 }] };
    const lines1 = { ...view } as Record<string, unknown>;
    internal.writeLines(lines1 as never, { lastLine: "new" });
    const lines2 = { ...view } as Record<string, unknown>;
    internal.writeLines(lines2 as never, { failure: "f2" });
    scenes.views = { namedOnly: internal.namedOnly(view as never), namedOnlyBare: internal.namedOnly({ id: "s", workspaceId: W, harness: "claude", status: "running" } as never), lines1, lines2 };
  }
  scenes.keys = {
    copyKey: internal.copyKey("solari", "default"),
    parts: ["solari/default", "default", "a/b/c", "/x", "x/"].map(k => [k, internal.copyKeyParts(k)]),
    recipeKey: internal.recipeKey("default", 3),
    vaultKey: internal.vaultKey("img", 12),
  };
  scenes.lines = {
    labelOf: [internal.labelOf({ record: { name: "box" } }, undefined), internal.labelOf({ record: { name: "box" } }, "0123456789abcdef")],
    gone: internal.goneLogLine("w1", "404"),
    swept1: internal.sweptRunsLogLine("w1", ["r1"]),
    swept2: internal.sweptRunsLogLine("w1", ["r1", "r2"]),
    swept0: internal.sweptRunsLogLine("w1", []),
    unread: internal.serverValuesUnreadLine("Claude Code", "timed out"),
    left: internal.serverValuesLeftLine("Codex", "config.toml is a link out of its folder"),
    cut: internal.serverValuesCutLine("Codex", ["a.json", "b.toml"]),
    noTitle: internal.noTitleLogLine("0123456789", "w1", "gone"),
    noTitleEmoji: internal.noTitleLogLine("😀😀😀😀😀", "w1", "gone"),
    noMadeTitle: internal.noMadeTitleLogLine("abcdefghij", "w1", "slow"),
    noCheckpoint: internal.noCheckpointLogLine("abcdefghij", "w1", "dirty"),
    noNameWrite: internal.noNameWriteLogLine("abc", "w1", "refused"),
    lostSession: internal.lostSessionLogLine("sess-full-id", "thread-long-id", "w1", "no file"),
    stop: internal.stopLogLine("thread-long-id", undefined, "stopped"),
    stopTask: internal.stopLogLine("thread-long-id", "task-long-id", "gone", "3 left"),
    restart: [0, 999, 61_000, 3_600_000 + 5_000].map(ms => internal.restartCutLine(ms)),
    transcriptUnread: internal.transcriptUnreadLine("w1", "bad json"),
  };
  scenes.fingerprint = {
    plain: internal.fingerprint({ kind: "sandbox", cpu: 2, memMb: 4096 } as never),
    keyed: internal.fingerprint({ kind: "sandbox", cpu: 2, memMb: 4096, idempotencyKey: "k1", labels: { createdAt: "now", wsp: "1" } } as never),
    keyed2: internal.fingerprint({ kind: "sandbox", cpu: 2, memMb: 4096, idempotencyKey: "k2", labels: { createdAt: "later", wsp: "1" } } as never),
    labelsFirst: internal.fingerprint({ labels: { wsp: "1" }, kind: "sandbox", cpu: 2, memMb: 4096 } as never),
    emptyLabels: internal.fingerprint({ kind: "sandbox", cpu: 2, memMb: 4096, labels: {} } as never),
    hashOf: (await import("node:crypto")).createHash("sha256").update('{"kind":"sandbox","cpu":2,"memMb":4096,"labels":{}}').digest("hex"),
  };
  scenes.prepare = [
    [null, null],
    ["b1", null],
    ["b1", { kept: true }],
    ["b1", { left: "provider said no" }],
  ].map(([id, o]) => {
    const e = new internal.PrepareStoppedError(id ?? undefined, o ?? undefined);
    return { id, o, message: e.message, name: e.name, builderId: e.builderId ?? null, kept: e.kept, left: e.left ?? null };
  });
  {
    const dead = internal.deadMachine("m1");
    const absentErr = Object.assign(new Error("the computer is away"), { kind: "unreachable" });
    const absent = internal.absentMachine("m2", "sandbox", () => { throw absentErr; });
    const tries = async (m: Record<string, (...a: unknown[]) => Promise<unknown>>): Promise<unknown> => {
      const out: Record<string, unknown> = {};
      for (const k of ["exec", "run", "snapshot", "pause", "resume", "kill", "state", "downloadUrl", "uploadUrl"]) {
        try {
          out[k] = { ok: (await m[k]!()) ?? null };
        } catch (e) {
          out[k] = { err: (e as Error).message, kind: (e as { kind?: string }).kind ?? null, status: (e as { status?: number }).status ?? null };
        }
      }
      return out;
    };
    scenes.machines = {
      dead: { id: dead.id, kind: dead.kind, streamUrl: dead.streamUrl ?? null, calls: await tries(dead as never), absent: internal.isAbsentMachine(dead) },
      absent: { id: absent.id, kind: absent.kind, calls: await tries(absent as never), absent: internal.isAbsentMachine(absent) },
    };
  }
  {
    const body = (chunks: (Uint8Array | Error)[]): Response =>
      new Response(
        new ReadableStream({
          pull(c) {
            const next = chunks.shift();
            if (next === undefined) c.close();
            else if (next instanceof Error) c.error(next);
            else c.enqueue(next);
          },
        }),
      );
    const enc = new TextEncoder();
    scenes.bodies = {
      under: await internal.readBodyUpTo(body([enc.encode("hello "), enc.encode("world")]), 2048),
      cut: await internal.readBodyUpTo(body([enc.encode("hello "), enc.encode("world")]), 8),
      cutExact: await internal.readBodyUpTo(body([enc.encode("hello "), enc.encode("world")]), 6),
      splitChar: await internal.readBodyUpTo(body([enc.encode("ab😀cd")]), 4),
      dies: await internal.readBodyUpTo(body([enc.encode("part"), new Error("reset")]), 100),
      empty: await internal.readBodyUpTo(new Response(null), 10),
      zeroCap: await internal.readBodyUpTo(body([enc.encode("x")]), 0),
    };
  }
  {
    const r = (code: string | undefined): unknown => {
      const e = new internal.DaemonRefusal(code, "said");
      return { code: code ?? null, name: e.name, message: e.message, host: internal.isNoHostCli(e), cred: internal.isNoGitCredential(e), def: internal.isOnDefaultBranch(e) };
    };
    const plain = Object.assign(new Error("x"), { code: "no-host-cli" });
    const roads = ["fork", "run", "bring back"].map(road => { const d = internal.defaultBranchRefused(road as never, "my-copy", "main") as Error & { kind?: string; fix?: string }; return { road, message: d.message, kind: d.kind ?? null, fix: d.fix ?? null }; });
    scenes.refusals = {
      codes: [r("no-host-cli"), r("no-git-credential"), r("on-default-branch"), r(undefined), r("other")],
      plainErrorIsNotRefusal: internal.isNoHostCli(plain),
      defaultBranch: roads,
      capRefusal: [internal.isCapRefusal({ kind: "concurrency" }), internal.isCapRefusal({ kind: "quota" }), internal.isCapRefusal(new Error("x"))],
    };
  }
  const scope = { kind: "thread", threadId: "t1", workspaceId: "w1", rootThreadId: "t0" };
  scenes.reading = {
    steered: [
      null,
      [],
      "x",
      {},
      { a: { prompt: "p", startedBy: "person" } },
      { a: { prompt: "p", startedBy: "nobody" } },
      { a: { startedBy: "person" } },
      { a: { prompt: "p", startedBy: "person", by: scope, road: "relayed", requestId: "r1", attachments: [] } },
      { a: { prompt: "p", startedBy: "person", by: { bad: 1 } } },
      { a: { prompt: "p", startedBy: "person", road: "nowhere" } },
      { a: { prompt: "p", startedBy: "person", requestId: 5, attachments: "x" } },
      { a: null, b: { prompt: "q", startedBy: "person" } },
    ].map(raw => [raw, internal.readSteered(raw) ?? null]),
    asked: [
      null,
      "p",
      { prompt: 1 },
      { prompt: "p" },
      { prompt: "p", effort: "high", typed: "t", requestId: "r", owed: true, awaitsPrompt: true },
      { prompt: "p", effort: 3, typed: null, owed: "yes", awaitsPrompt: false },
      ["p"],
    ].map(raw => [raw, internal.readAsked(raw) ?? null]),
    scope: [undefined, null, scope, { ...scope, kind: "device" }, { threadId: 1 }].map(raw => [raw ?? null, internal.readScope(raw) ?? null]),
    road: [undefined, null, "relayed", "here", "paired", "nowhere"].map(raw => [raw ?? null, internal.readRoad(raw) ?? null]),
  };
  scenes.launches = [
    [{ fixed: "a", picks: {} }, { fixed: "a", picks: {} }],
    [{ fixed: "a", picks: { model: "m" } }, { fixed: "a", picks: {} }],
    [{ fixed: "a", picks: {} }, { fixed: "a", picks: { model: "m" } }],
    [{ fixed: "a", picks: { model: "m" } }, { fixed: "a", picks: { model: "m" } }],
    [{ fixed: "a", picks: { model: "m" } }, { fixed: "b", picks: { model: "m" } }],
    [{ fixed: "a", picks: { fast: false } }, { fixed: "a", picks: { fast: false } }],
    [{ fixed: "a", picks: { fast: false } }, { fixed: "a", picks: { fast: null } }],
  ].map(([k, s]) => [k, s, internal.launchesAs(k as never, s as never)]);
  {
    const root = mkdtempSync(join(tmpdir(), "core-types-"));
    const mk = (p: string, bytes = 3): void => {
      mkdirSync(join(root, p, ".."), { recursive: true });
      writeFileSync(join(root, p), "x".repeat(bytes));
    };
    mk("a/s-abc.jsonl", 3);
    mk("B/s-abc.jsonl", 5);
    mk("2026-09-30/s-abc.jsonl", 7);
    mk("2026-10-01/s-other.jsonl", 9);
    mk("flat-s-abc.jsonl", 11);
    const rel = (p: string | undefined): string | null => (p === undefined ? null : p.slice(root.length));
    scenes.files = {
      depth1: rel(internal.findSessionFile(root, "s-abc.jsonl", 1)),
      depth0: rel(internal.findSessionFile(root, "s-abc.jsonl", 0)),
      depth2: rel(internal.findSessionFile(root, "s-abc.jsonl", 2)),
      missing: rel(internal.findSessionFile(join(root, "nope"), "s-abc.jsonl", 1)),
      sizeMissing: internal.sizeOf(join(root, "nope")),
      order: ["a", "A", "_z", "Z9", "z10", "z9", "B", "b", "2026-10-01", "2026-9-30", "é", "e", "f"].sort((a, b) => b.localeCompare(a)),
      layout: ["a/s-abc.jsonl:3", "B/s-abc.jsonl:5", "2026-09-30/s-abc.jsonl:7", "2026-10-01/s-other.jsonl:9", "flat-s-abc.jsonl:11"],
    };
  }
  scenes.constants = {
    WORKSPACES: internal.WORKSPACES, PROJECTS: internal.PROJECTS, SEED_CHOICES: internal.SEED_CHOICES, GOLDENS: internal.GOLDENS,
    GOLDEN_RECIPES: internal.GOLDEN_RECIPES, PROJECT_GOLDENS: internal.PROJECT_GOLDENS, IMAGES: internal.IMAGES, IMAGE_VAULTS: internal.IMAGE_VAULTS,
    TRANSCRIPTS: internal.TRANSCRIPTS, TRANSCRIPT_HEADS: internal.TRANSCRIPT_HEADS, TRANSCRIPT_INDEX: internal.TRANSCRIPT_INDEX,
    ATTACHMENTS: internal.ATTACHMENTS, ATTACHMENT_KEYS: internal.ATTACHMENT_KEYS, SESSIONS: internal.SESSIONS, NOTIFY_OWED: internal.NOTIFY_OWED,
    HELD_STARTS: internal.HELD_STARTS, SUB_STARTS: internal.SUB_STARTS, WORKSPACE_NAMES: internal.WORKSPACE_NAMES, DROPPED: internal.DROPPED,
    UNRECORDED: internal.UNRECORDED, PREFERENCES: internal.PREFERENCES, PREFERENCES_ID: internal.PREFERENCES_ID, READS: internal.READS,
    READS_ID: internal.READS_ID, AGENT_LISTS: internal.AGENT_LISTS, BUILDERS: internal.BUILDERS, OWNER: internal.OWNER, CREATES: internal.CREATES,
    NO_COPY_RECIPE: internal.NO_COPY_RECIPE, NO_SEED_WIRING: internal.NO_SEED_WIRING,
    PAUSED_REASON: internal.PAUSED_REASON, DELETED_REASON: internal.DELETED_REASON, RESTARTED_REASON: internal.RESTARTED_REASON, GONE_REASON: internal.GONE_REASON,
    OWED_RETRY_MS: internal.OWED_RETRY_MS, OWED_RETRY_MAX_MS: internal.OWED_RETRY_MAX_MS, OWED_FOR_MS: internal.OWED_FOR_MS,
    DROPPED_WATCH_MS: internal.DROPPED_WATCH_MS, UNRECORDED_DELETE_MS: internal.UNRECORDED_DELETE_MS, DAEMON_TOKEN_MISS_TTL_MS: internal.DAEMON_TOKEN_MISS_TTL_MS,
    TRANSCRIPT_CAP: internal.TRANSCRIPT_CAP, TRANSCRIPT_BYTES: internal.TRANSCRIPT_BYTES, TOOL_RESULT_KEPT: internal.TOOL_RESULT_KEPT,
    TRANSCRIPTS_HELD: internal.TRANSCRIPTS_HELD, PENDING_FLUSH_BYTES: internal.PENDING_FLUSH_BYTES, MAX_TIMER_MS: internal.MAX_TIMER_MS,
    WORDS_HELD: internal.WORDS_HELD, SESSION_INDEX_CAP: internal.SESSION_INDEX_CAP, TRANSCRIPT_FLUSH_MS: internal.TRANSCRIPT_FLUSH_MS,
    KEPT_CLOSE_WAIT_MS: internal.KEPT_CLOSE_WAIT_MS, KEY_PURPOSE_MAX: internal.KEY_PURPOSE_MAX, LOST_CREATE_MS: internal.LOST_CREATE_MS,
    HELD_TTL_MS: internal.HELD_TTL_MS, HEARTBEAT_MS: internal.HEARTBEAT_MS, SESSION_FACTS: [...internal.SESSION_FACTS],
    OPTIONAL_BACKEND_CALLS: [...internal.OPTIONAL_BACKEND_CALLS],
  };
  write("internal", scenes);
});
