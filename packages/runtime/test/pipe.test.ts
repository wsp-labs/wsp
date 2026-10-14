// SPDX-License-Identifier: AGPL-3.0-only
import { createHash, randomBytes } from "node:crypto";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import type { Duplex } from "node:stream";
import { afterEach, describe, expect, it } from "vitest";
import { WebSocket, WebSocketServer } from "ws";
import { realClock } from "../src/clock.js";
import type { RuntimeContext } from "../src/context.js";
import { PIPE_BEHIND_AGAIN_MS, PIPE_DIAL_RATE, PIPE_IDLE, PIPE_IDLE_MAX_MS, pipeArea, pipePool, type PipeHandler, type PipePool } from "../src/pipe.js";
import { eventBus } from "../src/types/events.js";
import { PIPE_DAEMON_VERSION, PIPE_POOL_FULL, PIPE_POOL_MAX, type EventUnion, type PipeOpen } from "@wsp/protocol";
import { until } from "./until.js";

const TOKEN = "pipe-token";

/** A daemon that answers a pipe's auth frame as a link's and parks it, and hands one over when the test plays a local
 * connection arriving at a door. `old` answers the socket as a link, with its hello; `refuse` refuses the token;
 * `grab` takes every pipe the moment it is authed and holds it; `full` refuses it as past its parked pipes. */
interface FakeDaemon {
  url: string;
  parked: WebSocket[];
  firstFrames: Record<string, unknown>[];
  /** Every pipe the daemon took, still open or not. */
  taken: WebSocket[];
  /** The daemon replaced by one of another kind, as an update does. */
  become(mode: "pipes" | "old" | "refuse" | "deny" | "grab" | "full"): void;
  take(door: "http" | "gh", heldMs?: number): WebSocket;
  /** The machine falls off the network: every socket cut, nothing listening. */
  drop(): Promise<void>;
  back(): Promise<void>;
  close(): Promise<void>;
}

const daemons: FakeDaemon[] = [];
const pools: PipePool[] = [];
const duplexes: Duplex[] = [];

afterEach(async () => {
  for (const p of pools.splice(0)) p.close();
  for (const d of duplexes.splice(0)) d.destroy();
  for (const d of daemons.splice(0)) await d.close();
});

async function fakeDaemon(first: "pipes" | "old" | "refuse" | "deny" | "grab" | "full" = "pipes"): Promise<FakeDaemon> {
  let mode = first;
  let http: Server = createServer();
  const wss = new WebSocketServer({ noServer: true });
  const parked: WebSocket[] = [];
  const taken: WebSocket[] = [];
  const firstFrames: Record<string, unknown>[] = [];
  const attach = (s: Server): void => {
    s.on("upgrade", (req, socket, head) => wss.handleUpgrade(req, socket, head, ws => wss.emit("connection", ws, req)));
  };
  wss.on("connection", ws => {
    ws.once("message", data => {
      const frame = JSON.parse(data.toString()) as Record<string, unknown>;
      firstFrames.push(frame);
      if (frame["token"] !== TOKEN || mode === "refuse") return ws.close(4401, "unauthorized");
      if (mode === "deny") return ws.send(JSON.stringify({ id: 0, ok: false, error: "unauthorized" }));
      if (mode === "full") return ws.close(1008, PIPE_POOL_FULL);
      ws.send(JSON.stringify({ id: 0, ok: true }));
      if (mode === "old") return ws.send(JSON.stringify({ type: "daemon.hello", root: "/root", version: 157 }));
      if (mode === "grab") {
        taken.push(ws);
        return ws.send(JSON.stringify({ op: "pipe.open", door: "http", heldMs: 0 }));
      }
      parked.push(ws);
      ws.once("close", () => {
        const at = parked.indexOf(ws);
        if (at >= 0) parked.splice(at, 1);
      });
    });
  });
  attach(http);
  await new Promise<void>(r => http.listen(0, "127.0.0.1", r));
  const port = (http.address() as AddressInfo).port;
  const d: FakeDaemon = {
    url: `http://127.0.0.1:${port}/`,
    parked,
    firstFrames,
    taken,
    become(next) {
      mode = next;
    },
    take(door, heldMs = 0) {
      const ws = parked.shift();
      if (ws === undefined) throw new Error("no pipe parked");
      taken.push(ws);
      ws.send(JSON.stringify({ op: "pipe.open", door, heldMs }));
      return ws;
    },
    async drop() {
      for (const ws of wss.clients) ws.terminate();
      await new Promise<void>(r => http.close(() => r()));
    },
    async back() {
      http = createServer();
      attach(http);
      await new Promise<void>(r => http.listen(port, "127.0.0.1", r));
    },
    async close() {
      for (const ws of wss.clients) ws.terminate();
      wss.close();
      if (http.listening) await new Promise<void>(r => http.close(() => r()));
    },
  };
  daemons.push(d);
  return d;
}

interface Handed {
  workspaceId: string;
  open: PipeOpen;
  duplex: Duplex;
}

function poolOn(
  d: FakeDaemon,
  o: { ready?: () => boolean; version?: () => number | null; idleMaxMs?: number; behindAgainMs?: number; dialRate?: number; backoffMs?: (attempt: number) => number; handler?: PipeHandler | null } = {},
): { pool: PipePool; handed: Handed[]; logs: string[] } {
  const handed: Handed[] = [];
  const logs: string[] = [];
  const handler: PipeHandler =
    o.handler ??
    ((workspaceId, open, duplex) => {
      duplexes.push(duplex);
      handed.push({ workspaceId, open, duplex });
    });
  const pool = pipePool({
    reach: async () => ({ url: d.url, token: TOKEN }),
    version: async () => (o.version === undefined ? PIPE_DAEMON_VERSION : o.version()),
    handler: () => (o.handler === null ? undefined : handler),
    ready: o.ready ?? (() => true),
    backoffMs: o.backoffMs ?? (() => 20),
    ...(o.dialRate !== undefined ? { dialRate: o.dialRate } : {}),
    ...(o.idleMaxMs !== undefined ? { idleMaxMs: o.idleMaxMs } : {}),
    ...(o.behindAgainMs !== undefined ? { behindAgainMs: o.behindAgainMs } : {}),
    log: line => logs.push(line),
  });
  pools.push(pool);
  return { pool, handed, logs };
}

const sleep = (ms: number): Promise<void> => new Promise(r => setTimeout(r, ms));
const sha = (b: Buffer): string => createHash("sha256").update(b).digest("hex");

/** The next binary frame the daemon's end of a pipe receives. */
const nextFrame = (ws: WebSocket): Promise<Buffer> => new Promise(r => ws.once("message", (data: Buffer) => r(data)));

describe("the pipe pool", () => {
  it("keeps 4 idle pipes, each authed with the daemon's token as a pipe, and dials no more", async () => {
    const d = await fakeDaemon();
    const { pool, logs } = poolOn(d);
    pool.up("ws-a");
    await until(() => d.parked.length === PIPE_IDLE);
    await sleep(100);
    expect(PIPE_IDLE).toBe(4);
    expect(d.parked).toHaveLength(4);
    expect(d.firstFrames).toEqual(Array(4).fill({ id: 0, op: "auth", token: TOKEN, pipe: true }));
    expect(pool.idle("ws-a")).toBe(4);
    expect(logs).toEqual(["ws-a: dialling 4 pipes, 4 kept idle"]);
  });

  it("dials a replacement for each pipe taken and hands it over under the workspace it was dialled for", async () => {
    const d = await fakeDaemon();
    const { pool, handed, logs } = poolOn(d);
    pool.up("ws-a");
    await until(() => d.parked.length === 4);
    d.take("http", 1200);
    await until(() => handed.length === 1 && d.parked.length === 4);
    expect(handed[0]!.workspaceId).toBe("ws-a");
    expect(handed[0]!.open).toEqual({ op: "pipe.open", door: "http", heldMs: 1200 });
    expect(d.firstFrames).toHaveLength(5);
    expect(logs.slice(1)).toEqual(["ws-a: a pipe opened on the http door after 1200 ms held, dialling its replacement"]);
    for (let i = 0; i < 4; i++) d.take(i % 2 === 0 ? "gh" : "http");
    await until(() => handed.length === 5 && d.parked.length === 4);
    expect(d.firstFrames).toHaveLength(9);
    // Four sockets, so the host may read their pipe.open frames in any order.
    expect(handed.slice(1).map(h => h.open.door).sort()).toEqual(["gh", "gh", "http", "http"]);
    await sleep(100);
    expect(d.firstFrames).toHaveLength(9);
  });

  it("carries a taken pipe's bytes both ways, binary and unchanged, and either end closing ends the other", async () => {
    const d = await fakeDaemon();
    const { pool, handed } = poolOn(d);
    pool.up("ws-a");
    await until(() => d.parked.length === 4);
    const far = d.take("http");
    await until(() => handed.length === 1);
    const { duplex } = handed[0]!;
    const request = Buffer.from("POST /v1/messages HTTP/1.1\r\nhost: 127.0.0.1:7071\r\ncontent-length: 2\r\n\r\n{}");
    const read: Buffer[] = [];
    duplex.on("data", (c: Buffer) => read.push(c));
    far.send(request);
    await until(() => Buffer.concat(read).equals(request));
    const reply = randomBytes(70_000);
    const got = nextFrame(far);
    duplex.write(reply);
    expect(sha(await got)).toBe(sha(reply));
    const ended = new Promise<void>(r => duplex.once("end", () => r()));
    far.close(1000);
    await ended;
    // And the other way: the handler ending its side closes the pipe at the daemon.
    const far2 = d.take("gh");
    await until(() => handed.length === 2);
    const closed = new Promise<void>(r => far2.once("close", () => r()));
    handed[1]!.duplex.resume();
    handed[1]!.duplex.end();
    await closed;
  });

  it("holds the daemon back while the handler reads nothing, and gives every byte once it reads", async () => {
    const d = await fakeDaemon();
    const { pool, handed } = poolOn(d);
    pool.up("ws-a");
    await until(() => d.parked.length === 4);
    const far = d.take("http");
    await until(() => handed.length === 1);
    const { duplex } = handed[0]!;
    const hash = createHash("sha256");
    const chunk = randomBytes(1 << 20);
    let sent = 0;
    // Sent until the daemon's own socket holds 16 MiB it cannot hand on: the host stopped reading, so TCP pushed back.
    while (far.bufferedAmount < 16 << 20 && sent < 512 << 20) {
      far.send(chunk);
      hash.update(chunk);
      sent += chunk.length;
      await sleep(1);
    }
    await sleep(200);
    expect(far.bufferedAmount).toBeGreaterThanOrEqual(8 << 20);
    expect(duplex.readableLength).toBeLessThan(4 << 20);
    const want = hash.digest("hex");
    const got = createHash("sha256");
    let read = 0;
    duplex.on("data", (c: Buffer) => {
      got.update(c);
      read += c.length;
    });
    await until(() => read === sent, 20_000);
    expect(got.digest("hex")).toBe(want);
  }, 30_000);

  it("holds the handler's writes back while the daemon reads nothing, and delivers every byte once it reads", async () => {
    const d = await fakeDaemon();
    const { pool, handed } = poolOn(d);
    pool.up("ws-a");
    await until(() => d.parked.length === 4);
    const far = d.take("http");
    await until(() => handed.length === 1);
    const { duplex } = handed[0]!;
    const got = createHash("sha256");
    let received = 0;
    far.on("message", (data: Buffer) => {
      got.update(data);
      received += data.length;
    });
    far.pause();
    const hash = createHash("sha256");
    const chunk = randomBytes(1 << 20);
    let written = 0;
    let stuck = false;
    // Written as a well-behaved writer writes, waiting for each drain, until one does not come: the daemon reads
    // nothing, so once the kernel's buffers are full the duplex holds what it was given and asks for no more.
    while (!stuck && written < 512 << 20) {
      hash.update(chunk);
      written += chunk.length;
      if (duplex.write(chunk)) continue;
      stuck = !(await Promise.race([new Promise<boolean>(r => duplex.once("drain", () => r(true))), sleep(300).then(() => false)]));
    }
    expect(stuck).toBe(true);
    expect(received).toBe(0);
    expect(duplex.writableLength).toBeLessThanOrEqual(2 << 20);
    const drained = new Promise<void>(r => duplex.once("drain", () => r()));
    far.resume();
    await drained;
    await until(() => received === written, 20_000);
    expect(got.digest("hex")).toBe(hash.digest("hex"));
  }, 30_000);

  it("replaces a pipe left idle past its limit with a fresh one, and logs no dial for it", async () => {
    expect(PIPE_IDLE_MAX_MS).toBe(600_000);
    const d = await fakeDaemon();
    const { pool, logs } = poolOn(d, { idleMaxMs: 300 });
    pool.up("ws-a");
    await until(() => d.parked.length === 4);
    const first = [...d.parked];
    const closed = first.map(ws => new Promise<void>(r => ws.once("close", () => r())));
    await Promise.all(closed);
    await until(() => d.parked.length === 4 && d.parked.every(ws => !first.includes(ws)));
    expect(d.firstFrames.length).toBeGreaterThanOrEqual(8);
    expect(logs).toEqual(["ws-a: dialling 4 pipes, 4 kept idle"]);
  });

  it("closes the connection a pipe brings when nothing serves pipes yet, and still dials its replacement", async () => {
    const d = await fakeDaemon();
    const { pool, logs } = poolOn(d, { handler: null });
    pool.up("ws-a");
    await until(() => d.parked.length === 4);
    const far = d.take("http");
    await new Promise<void>(r => far.once("close", () => r()));
    await until(() => d.parked.length === 4);
    expect(logs).toContain("ws-a: nothing here serves pipes yet, so that connection was closed");
  });

  it("dials nothing while the switch reads off, and fills the pool once it reads on", async () => {
    const d = await fakeDaemon();
    let on = false;
    const { pool } = poolOn(d, { ready: () => on });
    pool.up("ws-a");
    await sleep(150);
    expect(d.firstFrames).toHaveLength(0);
    on = true;
    await until(() => d.parked.length === 4);
  });

  it("dials no pipe to a daemon older than the pipes, and fills the pool once an update brings one new enough", async () => {
    expect(PIPE_BEHIND_AGAIN_MS).toBe(60_000);
    const d = await fakeDaemon();
    let version = PIPE_DAEMON_VERSION - 1;
    let asked = 0;
    const { pool, logs } = poolOn(d, { behindAgainMs: 150, version: () => (asked++, version) });
    pool.up("ws-a");
    await until(() => asked === 2);
    expect(d.firstFrames).toHaveLength(0);
    expect(logs).toEqual([`ws-a: its daemon (${PIPE_DAEMON_VERSION - 1}) serves no pipes yet, so it is asked again each minute`]);
    version = PIPE_DAEMON_VERSION;
    await until(() => d.parked.length === 4);
    expect(pool.idle("ws-a")).toBe(4);
  });

  it("closes a pipe the daemon answers as a link and asks again each minute, saying so once", async () => {
    const d = await fakeDaemon("old");
    const { pool, logs } = poolOn(d, { behindAgainMs: 150 });
    pool.up("ws-a");
    await until(() => d.firstFrames.length === 4);
    await sleep(50);
    expect(pool.idle("ws-a")).toBe(0);
    await until(() => d.firstFrames.length === 8);
    expect(logs.filter(l => l === "ws-a: its daemon answered a pipe as a link, so it is asked again each minute")).toHaveLength(1);
    d.become("pipes");
    await until(() => d.parked.length === 4);
  });

  it("stops dialling a daemon that refuses the token, by its close or its answer, until the link comes up again", async () => {
    for (const mode of ["refuse", "deny"] as const) {
      const d = await fakeDaemon(mode);
      const { pool, logs } = poolOn(d);
      pool.up("ws-a");
      await until(() => d.firstFrames.length >= 1);
      await sleep(200);
      expect(d.firstFrames.length).toBeLessThanOrEqual(4);
      expect(pool.idle("ws-a")).toBe(0);
      expect(logs.at(-1)).toBe("ws-a: its daemon refused the token a pipe was dialled with, so none are dialled until its link comes up again");
      d.become("pipes");
      await sleep(100);
      expect(d.parked).toHaveLength(0);
      pool.down("ws-a");
      pool.up("ws-a");
      await until(() => d.parked.length === 4);
    }
  });
});

describe("the pipe pool and a machine that takes every pipe", () => {
  const openAt = (d: FakeDaemon): number => d.taken.filter(ws => ws.readyState === WebSocket.OPEN).length;

  it("dials no more than 4 pipes a second for a workspace whose machine takes each one at once", async () => {
    expect(PIPE_DIAL_RATE).toBe(4);
    const d = await fakeDaemon("grab");
    const { pool, handed } = poolOn(d);
    pool.up("ws-a");
    await sleep(1600);
    // 4 at once, then 4 more each second.
    expect(d.firstFrames.length).toBeGreaterThanOrEqual(8);
    expect(d.firstFrames.length).toBeLessThanOrEqual(12);
    expect(handed.length).toBe(d.firstFrames.length);
  });

  it("holds no more than 64 pipes a workspace's machine has taken, and dials again only as one closes", async () => {
    const d = await fakeDaemon("grab");
    const { pool, logs } = poolOn(d, { dialRate: 1000 });
    pool.up("ws-a");
    await until(() => openAt(d) === PIPE_POOL_MAX);
    await sleep(200);
    expect(PIPE_POOL_MAX).toBe(64);
    expect(openAt(d)).toBe(64);
    expect(d.firstFrames).toHaveLength(64);
    expect(logs).toContain("ws-a: 64 pipes are in use, so the next connection waits for one to close");
    // 4 idle at the start and one more per take reach 64 after 60 takes; the 4 taken after that dial nothing.
    expect(logs.filter(l => l.endsWith(", dialling its replacement"))).toHaveLength(60);
    expect(logs.filter(l => l.endsWith(", its replacement waits for a pipe to close"))).toHaveLength(4);
    for (const ws of d.taken.slice(0, 3)) ws.close(1000);
    await until(() => d.firstFrames.length === 67 && openAt(d) === 64);
    await sleep(200);
    expect(d.firstFrames).toHaveLength(67);
  });

  it("backs off further each time the daemon refuses a pipe as past its parked pipes", async () => {
    const d = await fakeDaemon("full");
    const attempts: number[] = [];
    const { pool } = poolOn(d, { dialRate: 1000, backoffMs: a => (attempts.push(a), 40) });
    pool.up("ws-a");
    await until(() => attempts.length >= 5);
    pool.down("ws-a");
    expect(attempts.slice(0, 5)).toEqual([1, 2, 3, 4, 5]);
  });
});

describe("the pipe pool and the link", () => {
  it("closes every pipe of a workspace when its link drops, and dials 4 again when it returns", async () => {
    const d = await fakeDaemon();
    const { pool, handed, logs } = poolOn(d);
    pool.up("ws-a");
    await until(() => d.parked.length === 4);
    d.take("http");
    await until(() => handed.length === 1 && d.parked.length === 4);
    const inFlight = new Promise<void>(r => handed[0]!.duplex.once("end", () => r()));
    handed[0]!.duplex.resume();
    await d.drop();
    await until(() => pool.idle("ws-a") === 0);
    await inFlight;
    await sleep(100);
    expect(pool.idle("ws-a")).toBe(0);
    await d.back();
    await until(() => d.parked.length === 4 && pool.idle("ws-a") === 4, 5000);
    await sleep(100);
    expect(d.parked).toHaveLength(4);
    expect(logs.slice(2)).toEqual(["ws-a: a pipe could not be dialled, so its others were closed and the pool is dialled again as the link comes back", "ws-a: 4 pipes idle"]);
  });

  it("closes every pipe, taken ones too, when the link stops, dials nothing after, and 4 again when it comes up", async () => {
    const d = await fakeDaemon();
    const { pool, handed } = poolOn(d);
    pool.up("ws-a");
    pool.up("ws-b");
    await until(() => d.parked.length === 8);
    const far = d.take("http");
    await until(() => handed.length === 1 && d.parked.length === 8);
    const aOnly = handed[0]!.workspaceId;
    const farClosed = new Promise<void>(r => far.once("close", () => r()));
    pool.down(aOnly);
    await farClosed;
    await until(() => d.parked.length === 4);
    await sleep(150);
    expect(d.parked).toHaveLength(4);
    expect(pool.idle(aOnly)).toBe(0);
    expect(pool.idle(aOnly === "ws-a" ? "ws-b" : "ws-a")).toBe(4);
    const dialled = d.firstFrames.length;
    pool.up(aOnly);
    await until(() => d.parked.length === 8);
    expect(d.firstFrames.length - dialled).toBe(4);
  });
});

describe("the pipe pool in the runtime", () => {
  const closers: { close(): void }[] = [];
  afterEach(() => {
    for (const c of closers.splice(0)) c.close();
  });

  /** The parts of the runtime the pool reads: its live records, its bus, a daemon token and the broker the host wired.
   * A record on `boat` is brokered and one on `hetzner` is not. */
  function runtimeOn(d: FakeDaemon, broker: { ready(): boolean; pipe?: PipeHandler } | undefined) {
    const bus = eventBus();
    const live = new Map<string, { record: { id: string; phase: string; place: string }; ws: { daemonReach(): Promise<{ url: string }> }; machine: { id: string } }>();
    const put = (id: string, place: "boat" | "hetzner", phase = "running"): void => {
      live.set(id, { record: { id, phase, place }, ws: { daemonReach: async () => ({ url: d.url }) }, machine: { id: `m-${id}` } });
    };
    const ctx = {
      opts: broker === undefined ? {} : { broker },
      live,
      bus,
      clock: realClock,
      daemonTokenOf: async () => TOKEN,
      moduleOf: () => ({ daemonVersion: async () => PIPE_DAEMON_VERSION }),
      ready: async () => {},
    } as unknown as RuntimeContext;
    const area = (): { close(): void } | undefined => {
      const a = pipeArea(ctx, r => (r as unknown as { place: string }).place === "boat");
      if (a !== undefined) closers.push(a);
      return a;
    };
    return { put, area, emit: (e: unknown) => bus.emit(e as EventUnion), live };
  }

  it("dials nothing on a host that wired no broker", async () => {
    const d = await fakeDaemon();
    const rt = runtimeOn(d, undefined);
    rt.put("ws-a", "boat");
    expect(rt.area()).toBeUndefined();
    await sleep(150);
    expect(d.firstFrames).toHaveLength(0);
  });

  it("keeps 4 pipes to each running brokered workspace, none to a joined computer's, and hands each to the broker", async () => {
    const d = await fakeDaemon();
    const handed: string[] = [];
    const rt = runtimeOn(d, {
      ready: () => true,
      pipe: (id, open, duplex) => {
        duplexes.push(duplex);
        handed.push(`${id} ${open.door}`);
      },
    });
    rt.put("ws-a", "boat");
    rt.put("ws-b", "hetzner");
    rt.put("ws-c", "boat", "napping");
    rt.area();
    await until(() => d.parked.length === 4);
    await sleep(150);
    expect(d.parked).toHaveLength(4);
    d.take("gh");
    await until(() => handed.length === 1 && d.parked.length === 4);
    expect(handed).toEqual(["ws-a gh"]);
  });

  it("closes a workspace's pipes as it naps, dials 4 again as it wakes, and none after it is deleted", async () => {
    const d = await fakeDaemon();
    const rt = runtimeOn(d, { ready: () => true, pipe: (_id, _h, duplex) => duplexes.push(duplex) });
    rt.area();
    rt.put("ws-a", "boat");
    rt.emit({ type: "workspace.created", workspace: { id: "ws-a", phase: "running" } });
    await until(() => d.parked.length === 4);
    rt.live.get("ws-a")!.record.phase = "napping";
    rt.emit({ type: "workspace.napped", workspaceId: "ws-a" });
    await until(() => d.parked.length === 0);
    rt.live.get("ws-a")!.record.phase = "running";
    rt.emit({ type: "workspace.woken", workspaceId: "ws-a", machineId: "m-ws-a" });
    await until(() => d.parked.length === 4);
    const dialled = d.firstFrames.length;
    rt.emit({ type: "workspace.deleted", workspaceId: "ws-a" });
    await until(() => d.parked.length === 0);
    await sleep(150);
    expect(d.firstFrames.length).toBe(dialled);
  });
});
