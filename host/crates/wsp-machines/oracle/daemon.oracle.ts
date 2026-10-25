// SPDX-License-Identifier: AGPL-3.0-only
// machines/daemon.ts driven through scenes with a fake runtime context; every call the area makes, every event and
// warning, and the records after, in order, written to tests/fixtures/oracle/daemon.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";
import { DAEMON_VERSION, machineLacking } from "@wsp/protocol";
import { daemonArea } from "../../../../packages/runtime/src/machines/daemon.js";
import { DAEMON_LACKS_AGAIN_MS, DAEMON_REVIVE_AGAIN_MS } from "../../../../packages/runtime/src/types/wiring.js";
import { fakeClock } from "../../../../packages/runtime/test/fake-clock.js";

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const T0 = 1_790_000_000_000;
const flush = async (): Promise<void> => {
  for (let i = 0; i < 30; i++) await new Promise<void>(r => setImmediate(r));
};

interface Rec { id: string; kind: string; machineId: string; phase: string; project: string; landed?: string[]; daemonRefusedAt?: { machineId: string; at: string; why: string } }
interface Mod { version?: number | null | "throws"; deploy?: "ok" | "lacks" | "unanswered" | "fails" | "hangs" | "none"; roots?: "ok" | "fails"; hasDaemon?: boolean }
interface WorldIn {
  entries: { record: Rec; machine?: { id: string; putBytes?: true }; served?: boolean; mod?: Mod; signedUrls?: boolean }[];
  sessions?: { id: string; workspaceId: string; status: string; threadId?: string; startedAt?: number; queued?: true }[];
  closed?: boolean;
  starts?: string[];
}

function world(w: WorldIn) {
  const log: unknown[] = [];
  const clock = fakeClock(T0);
  const listeners = new Map<string, Set<(e: unknown) => void>>();
  const bus = {
    on: (type: string, l: (e: unknown) => void) => {
      const set = listeners.get(type) ?? new Set();
      listeners.set(type, set);
      set.add(l);
      return () => void set.delete(l);
    },
    emit: (e: { type: string }) => {
      log.push(["emit", e]);
      for (const l of [...(listeners.get(e.type) ?? [])]) l(e);
      for (const l of [...(listeners.get("*") ?? [])]) l(e);
    },
  };
  const live = new Map<string, unknown>();
  const byId = new Map(w.entries.map(e => [e.record.id, e]));
  for (const e of w.entries) live.set(e.record.id, { record: e.record, machine: { id: e.machine?.id ?? e.record.machineId, ...(e.machine?.putBytes ? { putBytes: () => undefined } : {}) } });
  const sessions = new Map((w.sessions ?? []).map(s => [s.id, { view: { id: s.id, workspaceId: s.workspaceId, status: s.status, ...(s.threadId ? { threadId: s.threadId } : {}), ...(s.startedAt !== undefined ? { startedAt: s.startedAt } : {}) }, ...(s.queued ? { queued: true } : {}) }]));
  const daemonNotes = new Map<string, string>();
  const gates: (() => void)[] = [];
  const startHow: string[] = [...(w.starts ?? [])];
  const module = (kind: string) => {
    const at = [...byId.values()].find(e => e.record.kind === kind)!;
    const m = at.mod ?? {};
    return {
      roots: async (entry: { record: Rec }, dests: string[]) => {
        log.push(["roots", entry.record.id, dests]);
        if (m.roots === "fails") throw new Error(`x`.repeat(250) + " roots refused");
      },
      hasDaemon: (entry: { record: Rec }) => (log.push(["hasDaemon", entry.record.id]), m.hasDaemon ?? true),
      daemonVersion: async (entry: { record: Rec }) => {
        log.push(["daemonVersion", entry.record.id]);
        if (m.version === "throws") throw new Error("hello failed");
        return m.version === undefined ? DAEMON_VERSION : m.version;
      },
      ...(m.deploy === "none"
        ? {}
        : {
            deployDaemon: async (entry: { record: Rec }) => {
              log.push(["deployDaemon", entry.record.id]);
              if (m.deploy === "hangs") await new Promise<void>(r => gates.push(r));
              if (m.deploy === "lacks") throw machineLacking("gcc is missing on this machine, so the daemon cannot build");
              if (m.deploy === "unanswered") throw Object.assign(new Error("no answer"), { wspMachineUnanswered: true });
              if (m.deploy === "fails") throw new Error("npm ERR! long log");
              return undefined;
            },
          }),
    };
  };
  const ctx = {
    bus, clock: clock.clock, daemonHelloTimeoutMs: 8_000, live, sessions, daemonNotes,
    state: { closed: w.closed === true },
    sessionsApi: {
      start: (workspaceId: string, o: { prompt: string; thread?: string; requestId: string; onHeld: (h: unknown) => void }, origin: unknown) => {
        log.push(["start", workspaceId, { prompt: o.prompt, thread: o.thread ?? null, requestId: typeof o.requestId }, origin ?? null]);
        const how = startHow.shift() ?? "started";
        if (how === "fails") return Promise.reject(new Error("refused"));
        if (how === "queued") {
          bus.emit({ type: "session.queued", requestId: "other", harness: "claude" });
          bus.emit({ type: "session.queued", requestId: o.requestId, harness: "codex" });
          return new Promise((_r, reject) => setTimeout(() => reject(new Error("later failure")), 5));
        }
        if (how === "held") {
          o.onHeld({ view: { id: "row-1", workspaceId, harness: "claude", status: "running" }, wait: { place: "box", running: 2, limit: 2 } });
          return new Promise(() => {});
        }
        return Promise.resolve({ outcome: how, view: () => ({ id: "s-9", workspaceId, harness: "claude", ...(how === "steered" ? { threadId: "t-steer" } : {}) }) });
      },
    },
    pushStatus: async (entry: { record: Rec }) => void log.push(["pushStatus", entry.record.id, daemonNotes.get(entry.record.id) ?? null]),
    persist: async (record: Rec) => void log.push(["persist", record.id, JSON.parse(JSON.stringify(record))]),
    servedByItsComputer: (entry: { record: Rec }) => (byId.get(entry.record.id)?.served ? "computer" : undefined),
    moduleOf: module,
    projectHeld: (id: string) => ({ path: `/work/${id}` }),
    checkoutOf: (r: Rec) => `/work/${r.project}/${r.id}`,
    backendFor: (r: Rec) => ({ capabilities: { signedUrls: byId.get(r.id)?.signedUrls ?? true } }),
  };
  const warn = vi.spyOn(console, "warn").mockImplementation((line: unknown) => void log.push(["warn", String(line)]));
  const area = daemonArea(ctx as never);
  const entry = (id: string) => live.get(id) as never;
  return { log, area, entry, clock, bus, sessions, gates, live, done: () => warn.mockRestore(), records: () => [...live.values()].map(e => (e as { record: Rec }).record) };
}

const ws = (id: string, o: Partial<Rec> = {}): Rec => ({ id, kind: "cloud", machineId: `m-${id}`, phase: "running", project: "p", ...o });

async function scene(name: string, w: WorldIn, act: (x: ReturnType<typeof world>) => Promise<void>) {
  const x = world(w);
  try {
    await act(x);
    await flush();
  } finally {
    x.done();
  }
  return { name, world: w, log: x.log, records: x.records() };
}

it("daemon", async () => {
  const at = (ms: number) => new Date(T0 - ms).toISOString();
  const scenes = [
    await scene("sync: current daemon writes roots and asks the version", { entries: [{ record: ws("a") }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: an old daemon is replaced, noted updating, roots written after", { entries: [{ record: ws("a"), mod: { version: 1 } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: no daemon yet is noted installing", { entries: [{ record: ws("a"), mod: { version: 1, hasDaemon: false } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: a version not read is marked for the next probe", { entries: [{ record: ws("a"), mod: { version: null } }] }, async x => {
      await x.area.syncDaemon(x.entry("a"));
      x.log.push(["unreadAt", [...x.area.unreadAt]]);
    }),
    await scene("sync: a version read that throws rejects the sync", { entries: [{ record: ws("a"), mod: { version: "throws" } }] }, async x => {
      await x.area.syncDaemon(x.entry("a")).catch((e: Error) => x.log.push(["rejected", e.message]));
    }),
    await scene("sync: served by its computer, a folder, writes roots only", { entries: [{ record: ws("a", { kind: "local" }), served: true }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: served by its computer, not a folder, does nothing", { entries: [{ record: ws("a"), served: true }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: a machine that said it lacks something inside the hour is left alone", { entries: [{ record: ws("a", { daemonRefusedAt: { machineId: "m-a", at: at(DAEMON_LACKS_AGAIN_MS - 1), why: "gcc" } }), mod: { version: 1 } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: the hour out, it is offered again and a deploy that lands clears the mark", { entries: [{ record: ws("a", { daemonRefusedAt: { machineId: "m-a", at: at(DAEMON_LACKS_AGAIN_MS), why: "gcc" } }), mod: { version: 1 } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: another machine's refusal is taken off and the sync goes on", { entries: [{ record: ws("a", { daemonRefusedAt: { machineId: "m-old", at: at(1), why: "gcc" } }), mod: { version: 1 } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: a deploy refused for what the machine lacks keeps its sentence and flashes it", { entries: [{ record: ws("a"), mod: { version: 1, deploy: "lacks" } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: a deploy the machine never answered keeps an old refusal", { entries: [{ record: ws("a", { daemonRefusedAt: { machineId: "m-a", at: at(DAEMON_LACKS_AGAIN_MS + 5), why: "gcc" } }), mod: { version: 1, deploy: "unanswered" } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: any other deploy failure drops the refusal and flashes update failed", { entries: [{ record: ws("a", { daemonRefusedAt: { machineId: "m-a", at: at(DAEMON_LACKS_AGAIN_MS + 5), why: "gcc" } }), mod: { version: 1, deploy: "fails" } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: an install that fails flashes install failed", { entries: [{ record: ws("a"), mod: { version: 1, hasDaemon: false, deploy: "fails" } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: no road to deploy asks no version", { entries: [{ record: ws("a"), mod: { deploy: "none" } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: no byte road and no signed urls asks no version", { entries: [{ record: ws("a"), signedUrls: false }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: a byte road with no signed urls deploys", { entries: [{ record: ws("a"), signedUrls: false, machine: { id: "m-a", putBytes: true }, mod: { version: 1 } }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: waits out a running turn, then deploys", { entries: [{ record: ws("a"), mod: { version: 1 } }], sessions: [{ id: "s1", workspaceId: "a", status: "running", threadId: "t1" }] }, async x => {
      const p = x.area.syncDaemon(x.entry("a"));
      await flush();
      x.log.push(["mark", "turn ends"]);
      (x.sessions.get("s1") as { view: { status: string } }).view.status = "done";
      x.bus.emit({ type: "session.delta", workspaceId: "a" });
      x.bus.emit({ type: "session.done", workspaceId: "a" });
      await p;
    }),
    await scene("sync: a queued row is no turn", { entries: [{ record: ws("a"), mod: { version: 1 } }], sessions: [{ id: "s1", workspaceId: "a", status: "running", queued: true }] }, async x => void (await x.area.syncDaemon(x.entry("a")))),
    await scene("sync: a machine that napped under the wait is left alone", { entries: [{ record: ws("a"), mod: { version: 1 } }], sessions: [{ id: "s1", workspaceId: "a", status: "running" }] }, async x => {
      const p = x.area.syncDaemon(x.entry("a"));
      await flush();
      (x.live.get("a") as { record: Rec }).record.phase = "napping";
      (x.sessions.get("s1") as { view: { status: string } }).view.status = "done";
      x.bus.emit({ type: "session.end", workspaceId: "a" });
      await p;
    }),
    await scene("sync: two at once on one machine share the work", { entries: [{ record: ws("a"), mod: { version: 1, deploy: "hangs" } }] }, async x => {
      const p1 = x.area.syncDaemon(x.entry("a"));
      const p2 = x.area.syncDaemon(x.entry("a"));
      x.log.push(["same", p1 === p2, x.area.daemonSyncs.size]);
      await flush();
      const settled = x.area.daemonSettled(x.entry("a"));
      x.gates.shift()!();
      await p1;
      await settled;
      await flush();
      x.log.push(["after", x.area.daemonSyncs.size]);
    }),
    await scene("roots: every record on the machine, deduped, landed folders after", { entries: [{ record: ws("a", { machineId: "m1", landed: ["/work/p", "/x"] }) }, { record: ws("b", { machineId: "m1", project: "q" }) }, { record: ws("c", { machineId: "m2" }) }] }, async x => void (await x.area.writeDaemonRoots(x.entry("a")))),
    await scene("roots: a closed host writes nothing", { entries: [{ record: ws("a") }], closed: true }, async x => void (await x.area.writeDaemonRoots(x.entry("a")))),
    await scene("roots: a failure warns the last 200 characters", { entries: [{ record: ws("a"), mod: { roots: "fails" } }] }, async x => void (await x.area.writeDaemonRoots(x.entry("a")))),
    await scene("roots: strict hands the failure back", { entries: [{ record: ws("a"), mod: { roots: "fails" } }] }, async x => {
      await x.area.writeDaemonRoots(x.entry("a"), true).catch((e: Error) => x.log.push(["rejected", e.message.length]));
    }),
    await scene("revive: no-daemon on a running machine deploys, notes restarting, clears", { entries: [{ record: ws("a") }] }, async x => {
      x.area.reviveDaemon(x.entry("a"), "no-daemon");
      await flush();
      x.log.push(["revivedAt", [...x.area.revivedAt]]);
      x.area.reviveDaemon(x.entry("a"), "no-daemon");
      x.clock.advance(DAEMON_REVIVE_AGAIN_MS - 1);
      x.area.reviveDaemon(x.entry("a"), "no-daemon");
      x.clock.advance(1);
      x.area.reviveDaemon(x.entry("a"), "no-daemon");
      await flush();
    }),
    await scene("revive: only for no-daemon on a running machine with a road and no unreached mark", { entries: [{ record: ws("a") }, { record: ws("b", { phase: "napping" }) }, { record: ws("c", { kind: "place" }), mod: { deploy: "none" } }, { record: ws("d") }] }, async x => {
      x.area.reviveDaemon(x.entry("a"), "reachable");
      x.area.reviveDaemon(x.entry("b"), "no-daemon");
      x.area.reviveDaemon(x.entry("c"), "no-daemon");
      x.area.unreached.set("d", { machineId: "m-d", line: "cannot reach" });
      x.area.reviveDaemon(x.entry("d"), "no-daemon");
      await flush();
    }),
    await scene("revive: a failed restart flashes restart failed", { entries: [{ record: ws("a"), mod: { deploy: "fails" } }] }, async x => {
      x.area.reviveDaemon(x.entry("a"), "no-daemon");
      await flush();
    }),
    await scene("revive: one at a time per machine", { entries: [{ record: ws("a"), mod: { deploy: "hangs" } }] }, async x => {
      x.area.reviveDaemon(x.entry("a"), "no-daemon");
      x.clock.advance(DAEMON_REVIVE_AGAIN_MS);
      x.area.reviveDaemon(x.entry("a"), "no-daemon");
      await flush();
      x.gates.shift()!();
      await flush();
    }),
    await scene("unreachedOf: a replaced machine's mark is dropped", { entries: [{ record: ws("a") }] }, async x => {
      x.area.unreached.set("a", { machineId: "m-old", line: "cannot reach" });
      x.log.push(["of", x.area.unreachedOf(x.entry("a")) ?? null, [...x.area.unreached]]);
      x.area.unreached.set("a", { machineId: "m-a", line: "cannot reach" });
      x.log.push(["of", x.area.unreachedOf(x.entry("a")) ?? null, [...x.area.unreached]]);
    }),
    await scene("offer again: only with a refusal and facts", { entries: [{ record: ws("a", { daemonRefusedAt: { machineId: "m-a", at: at(DAEMON_LACKS_AGAIN_MS), why: "gcc" } }), mod: { version: 1 } }, { record: ws("b") }] }, async x => {
      x.area.offerDaemonAgain(x.entry("a"), { phase: "running", reach: { state: "reachable" } } as never);
      x.area.offerDaemonAgain(x.entry("b"), { phase: "running", reach: { state: "reachable" }, facts: {} } as never);
      x.log.push(["mark", "with facts"]);
      x.area.offerDaemonAgain(x.entry("a"), { phase: "running", reach: { state: "reachable" }, facts: {} } as never);
      await flush();
    }),
    await scene("read again: after the window, reachable and running, same machine", { entries: [{ record: ws("a") }] }, async x => {
      const st = (phase: string, state: string) => ({ phase, reach: { state } }) as never;
      x.area.readVersionAgain(x.entry("a"), st("running", "reachable"));
      x.area.unreadAt.set("a", { machineId: "m-old", at: T0 });
      x.area.readVersionAgain(x.entry("a"), st("running", "reachable"));
      x.log.push(["unread", [...x.area.unreadAt]]);
      x.area.unreadAt.set("a", { machineId: "m-a", at: T0 });
      x.clock.advance(DAEMON_REVIVE_AGAIN_MS - 1);
      x.area.readVersionAgain(x.entry("a"), st("running", "reachable"));
      x.clock.advance(1);
      x.area.readVersionAgain(x.entry("a"), st("napping", "reachable"));
      x.area.readVersionAgain(x.entry("a"), st("running", "no-daemon"));
      x.log.push(["mark", "now"]);
      x.area.readVersionAgain(x.entry("a"), st("running", "reachable"));
      await flush();
      x.log.push(["unread", [...x.area.unreadAt]]);
    }),
    await scene("turnRuns counts a queued row, daemonSettled does not wait beside a turn", { entries: [{ record: ws("a"), mod: { version: 1, deploy: "hangs" } }], sessions: [{ id: "s1", workspaceId: "a", status: "running", queued: true }] }, async x => {
      x.log.push(["turnRuns", x.area.turnRuns("a"), x.area.turnRuns("b")]);
      const p = x.area.syncDaemon(x.entry("a"));
      await flush();
      let settled = false;
      void x.area.daemonSettled(x.entry("a")).then(() => (settled = true));
      await flush();
      x.log.push(["settled before the deploy ends", settled]);
      x.gates.shift()!();
      await p;
      await flush();
      x.log.push(["settled after", settled]);
    }),
    await scene("send detached: started", { entries: [{ record: ws("a") }], starts: ["started"] }, async x => void x.log.push(["answer", await x.area.sendDetached("a", { prompt: "hi" }, undefined)])),
    await scene("send detached: steered", { entries: [{ record: ws("a") }], starts: ["steered"] }, async x => void x.log.push(["answer", await x.area.sendDetached("a", { prompt: "hi", thread: "t1" }, { kind: "person" } as never)])),
    await scene("send detached: queued answers at the event with this request's id", { entries: [{ record: ws("a") }], starts: ["queued"] }, async x => {
      x.log.push(["answer", await x.area.sendDetached("a", { prompt: "hi", thread: "t1" }, undefined)]);
      await new Promise(r => setTimeout(r, 20));
    }),
    await scene("send detached: held answers the wait", { entries: [{ record: ws("a") }], starts: ["held"] }, async x => void x.log.push(["answer", await x.area.sendDetached("a", { prompt: "hi" }, undefined)])),
    await scene("send detached: a refused start rejects", { entries: [{ record: ws("a") }], starts: ["fails"] }, async x => {
      await x.area.sendDetached("a", { prompt: "hi" }, undefined).catch((e: Error) => x.log.push(["rejected", e.message]));
    }),
    await scene("to first thread: the earliest row with a thread, ties to the first listed", { entries: [{ record: ws("a") }, { record: ws("b") }], sessions: [{ id: "s1", workspaceId: "a", status: "done", threadId: "late", startedAt: 30 }, { id: "s2", workspaceId: "b", status: "done", threadId: "other", startedAt: 1 }, { id: "s3", workspaceId: "a", status: "done", startedAt: 0 }, { id: "s4", workspaceId: "a", status: "done", threadId: "early", startedAt: 10 }, { id: "s5", workspaceId: "a", status: "done", threadId: "tie", startedAt: 10 }] }, async x => void x.log.push(["answer", await x.area.toFirstThread("a", "go", undefined)])),
    await scene("to first thread: none opens one", { entries: [{ record: ws("a") }] }, async x => void x.log.push(["answer", await x.area.toFirstThread("a", "go", undefined)])),
    await scene("to first thread: a row with no startedAt counts as 0", { entries: [{ record: ws("a") }], sessions: [{ id: "s1", workspaceId: "a", status: "done", threadId: "dated", startedAt: 5 }, { id: "s2", workspaceId: "a", status: "running", threadId: "undated" }] }, async x => void x.log.push(["answer", await x.area.toFirstThread("a", "go", undefined)])),
  ];
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "daemon.json"), `${JSON.stringify({ T0, DAEMON_VERSION, DAEMON_LACKS_AGAIN_MS, DAEMON_REVIVE_AGAIN_MS, scenes }, null, 1)}\n`);
});
