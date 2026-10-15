// SPDX-License-Identifier: AGPL-3.0-only
// The host's pool of pipes to each brokered machine's daemon. A pipe is one
// more WebSocket to the daemon's own route, authed like the link, that the
// daemon parks until a process on the machine connects to one of its doors;
// then it carries that one connection's bytes and closes. The host keeps a few
// dialled ahead, since a cold dial through the provider's route costs most of
// a second, and dials a replacement as each is taken.
import type { Duplex } from "node:stream";
import WebSocket, { createWebSocketStream } from "ws";
import { DaemonResponse, daemonRefusedToken, linkBackoffMs, PIPE_DAEMON_VERSION, PIPE_POOL_MAX, pipeOpenOf, type PipeOpen } from "@wsp/protocol";
import { realClock, type Clock } from "./clock.js";
import type { RuntimeContext } from "./context.js";
import { daemonWsUrl } from "./reach.js";
import type { LiveWorkspace, WorkspaceRecord } from "./types/wiring.js";

/** Idle pipes the host keeps dialled per brokered workspace whose link is up. */
export const PIPE_IDLE = 4;
/** A pipe idle this long is replaced: 600 s idle is the longest the provider's route was measured to keep one. */
export const PIPE_IDLE_MAX_MS = 10 * 60_000;
/** The bound on one dial: the route and the daemon's token read, then the upgrade. */
export const PIPE_DIAL_MS = 15_000;
/** How long before a daemon too old for pipes is asked its version again: the host updates it on its own, and the new
 * one restarts with nothing telling the pool. */
export const PIPE_BEHIND_AGAIN_MS = 60_000;
/** The most pipes dialled to one workspace in any second. A machine's agent is root there and can take every pipe
 * the moment it is authed, so the host's own sockets are bounded by this and by PIPE_POOL_MAX, never by the machine. */
export const PIPE_DIAL_RATE = 4;
/** The longest wait between two writes of the host's token to a machine whose daemon keeps refusing it. A refusal
 * within twice this of the host's last write follows that write, since another writer waits no longer than this. */
export const TOKEN_REFUSED_MAX_MS = 30 * 60_000;

/** One road's count of a daemon refusing the host's token. The host writes its own token there again and dials after
 * PIPE_BEHIND_AGAIN_MS, and twice as long for each refusal that follows its own write: two writers of one file cut each
 * other's sockets, and backing off is what keeps that from happening every minute. */
export function tokenRefusals(firstMs = PIPE_BEHIND_AGAIN_MS): { refused(now: number): { count: number; waitMs: number }; wrote(now: number): void } {
  let count = 0;
  let wroteAt: number | undefined;
  return {
    refused(now) {
      count = wroteAt !== undefined && now - wroteAt < 2 * TOKEN_REFUSED_MAX_MS ? count + 1 : 1;
      wroteAt = undefined;
      return { count, waitMs: Math.min(firstMs * 2 ** (count - 1), TOKEN_REFUSED_MAX_MS) };
    },
    wrote(now) {
      wroteAt = now;
    },
  };
}

const waitWords = (ms: number): string => (ms < 120_000 ? `${Math.round(ms / 1000)} s` : `${Math.round(ms / 60_000)} min`);

/** What a road says the count'th time in a row a daemon refuses the host's token: the first refusal and the first one
 * after the host's own write, and nothing after that until a refusal stops following a write. */
export function tokenRefusedLine(who: string, road: string, count: number, waitMs: number): string | undefined {
  if (count === 1) return `${who}: its daemon refused the token ${road}, so the host writes its own there again and dials in ${waitWords(waitMs)}`;
  if (count === 2)
    return `${who}: its daemon refused the host's token again right after the host wrote it, so something else writes that file too; the host writes its own there again and dials in ${waitWords(waitMs)}, waiting twice as long each time up to ${waitWords(TOKEN_REFUSED_MAX_MS)}`;
  return undefined;
}

/** Where each taken pipe goes: the workspace whose link it was dialled on, never anything the machine said. */
export type PipeHandler = (workspaceId: string, open: PipeOpen, duplex: Duplex) => void;

export interface PipeReach {
  /** The daemon's route, as the link dials it. */
  url: string;
  /** The daemon's own token, sent in the first frame and never in the url. */
  token: string;
}

export interface PipePoolOptions {
  /** The link's own reach, read at each dial; undefined while the guest holds no daemon token. */
  reach(workspaceId: string): Promise<PipeReach | undefined>;
  /** The version the workspace's daemon announces, null when it does not answer: pipes are dialled only to one at or
   * above PIPE_DAEMON_VERSION. */
  version(workspaceId: string): Promise<number | null>;
  /** Read at each take, so a handler wired after the pool started still gets every pipe. */
  handler(): PipeHandler | undefined;
  /** The switch: nothing is dialled while it reads false. */
  ready(): boolean;
  /** The daemon refused the token: the next reach writes the host's token on the machine again. */
  rewrite(workspaceId: string): void;
  clock?: Clock;
  backoffMs?: (attempt: number) => number;
  idleMaxMs?: number;
  behindAgainMs?: number;
  dialRate?: number;
  log?: (line: string) => void;
}

export interface PipePool {
  /** The workspace's link is up: keep PIPE_IDLE pipes dialled to it. */
  up(workspaceId: string): void;
  /** The link stopped: every pipe of the workspace closes, the taken ones too, and nothing is dialled again until up. */
  down(workspaceId: string): void;
  /** Pipes open and waiting for the daemon to take one. */
  idle(workspaceId: string): number;
  close(): void;
}

interface Pipe {
  ws?: WebSocket;
  open: boolean;
  /** The daemon took its token. */
  authed: boolean;
  /** Taken, retired or dropped by this end: its close is nobody's news. */
  done: boolean;
  cancelIdle?: () => void;
}

interface Held {
  id: string;
  /** Dialling or dialled and untaken. */
  idle: Set<Pipe>;
  taken: Set<WebSocket>;
  attempt: number;
  retry?: () => void;
  /** When each dial of the last second went out, and the wait for the next one where the second is used up. */
  dialled: number[];
  pace?: () => void;
  stopped: boolean;
  refusals: ReturnType<typeof tokenRefusals>;
  /** The daemon's version was read at or above PIPE_DAEMON_VERSION. */
  serves: boolean;
  asking: boolean;
  /** Said once that the daemon serves no pipes, until a pipe is taken. */
  saidBehind: boolean;
  saidDialling: boolean;
  /** Said once that PIPE_POOL_MAX pipes are taken, until one closes. */
  saidFull: boolean;
}

/** The daemon's answer to a pipe's auth frame: true where it took the token, false where it refused it, undefined for
 * any other frame. */
function authAnswer(text: string): boolean | undefined {
  let frame: unknown;
  try {
    frame = JSON.parse(text);
  } catch {
    return undefined;
  }
  const reply = DaemonResponse.safeParse(frame);
  return reply.success && reply.data.id === 0 ? reply.data.ok : undefined;
}

export function pipePool(o: PipePoolOptions): PipePool {
  const clock = o.clock ?? realClock;
  const backoff = o.backoffMs ?? linkBackoffMs;
  const idleMaxMs = o.idleMaxMs ?? PIPE_IDLE_MAX_MS;
  const behindAgainMs = o.behindAgainMs ?? PIPE_BEHIND_AGAIN_MS;
  const dialRate = o.dialRate ?? PIPE_DIAL_RATE;
  const log = o.log ?? ((line: string) => console.warn(line));
  const held = new Map<string, Held>();

  const end = (pipe: Pipe): void => {
    pipe.done = true;
    pipe.cancelIdle?.();
    pipe.ws?.terminate();
  };
  const endIdle = (h: Held): void => {
    for (const pipe of h.idle) end(pipe);
    h.idle.clear();
  };

  const again = (h: Held, ms: number): void => {
    if (h.retry !== undefined) return;
    h.retry = clock.schedule(
      () => {
        h.retry = undefined;
        fill(h);
      },
      ms,
      { unref: true },
    );
  };
  const wait = (h: Held): void => again(h, backoff(++h.attempt));

  /** Dials up to PIPE_IDLE idle pipes, with every socket of the workspace, taken or idle, inside PIPE_POOL_MAX: past
   * it the daemon holds the next connection until a taken pipe closes. */
  const fill = (h: Held): void => {
    if (h.stopped || h.asking || h.retry !== undefined || h.pace !== undefined) return;
    if (!o.ready()) return wait(h);
    if (!h.serves) return ask(h);
    if (h.taken.size >= PIPE_POOL_MAX && !h.saidFull) {
      h.saidFull = true;
      log(`${h.id}: ${PIPE_POOL_MAX} pipes are in use, so the next connection waits for one to close`);
    }
    const short = Math.min(PIPE_IDLE - h.idle.size, PIPE_POOL_MAX - h.taken.size - h.idle.size);
    if (short <= 0) return;
    const now = clock.now();
    h.dialled = h.dialled.filter(at => now - at < 1000);
    const allowed = Math.min(short, dialRate - h.dialled.length);
    if (!h.saidDialling) log(`${h.id}: dialling ${short} pipe${short === 1 ? "" : "s"}, ${PIPE_IDLE} kept idle`);
    h.saidDialling = true;
    for (let i = 0; i < allowed; i++) {
      h.dialled.push(now);
      dial(h);
    }
    if (allowed < short) {
      h.pace = clock.schedule(
        () => {
          h.pace = undefined;
          fill(h);
        },
        h.dialled[0]! + 1000 - now,
        { unref: true },
      );
    }
  };

  /** A daemon below PIPE_DAEMON_VERSION reads `pipe` as nothing and answers the socket as a link, so it is asked its
   * version first and dialled only once it is new enough. */
  const ask = (h: Held): void => {
    h.asking = true;
    void o.version(h.id).then(
      v => answered(h, v),
      () => answered(h, null),
    );
  };
  const answered = (h: Held, version: number | null): void => {
    h.asking = false;
    if (h.stopped) return;
    if (version === null) return wait(h);
    if (version >= PIPE_DAEMON_VERSION) {
      h.serves = true;
      return fill(h);
    }
    behind(h, `${h.id}: its daemon (${version}) serves no pipes yet, so it is asked again each minute`);
  };
  const behind = (h: Held, line: string): void => {
    if (!h.saidBehind) log(line);
    h.saidBehind = true;
    h.serves = false;
    endIdle(h);
    again(h, behindAgainMs);
  };

  /** One pipe that could not be dialled, or closed while it waited: the link is down, so the rest go too, since an idle
   * pipe sends nothing and would learn it only when the daemon took it, and the pool is dialled again on the link's
   * own backoff. */
  const linkDown = (h: Held): void => {
    if (h.attempt === 0) log(`${h.id}: a pipe could not be dialled, so its others were closed and the pool is dialled again as the link comes back`);
    endIdle(h);
    if (!h.stopped) wait(h);
  };

  /** The pipes of one dial go together: the rest are ended here, so their own refusals are nobody's news. */
  const refused = (h: Held): void => {
    endIdle(h);
    const { count, waitMs } = h.refusals.refused(clock.now());
    const line = tokenRefusedLine(h.id, "a pipe was dialled with", count, waitMs);
    if (line !== undefined) log(line);
    o.rewrite(h.id);
    h.retry?.();
    h.retry = clock.schedule(
      () => {
        h.retry = undefined;
        h.refusals.wrote(clock.now());
        fill(h);
      },
      waitMs,
      { unref: true },
    );
  };

  const retire = (h: Held, pipe: Pipe): void => {
    if (pipe.done) return;
    h.idle.delete(pipe);
    pipe.done = true;
    pipe.ws?.close(1000);
    fill(h);
  };

  const take = (h: Held, pipe: Pipe, ws: WebSocket, open: PipeOpen): void => {
    h.idle.delete(pipe);
    pipe.done = true;
    pipe.cancelIdle?.();
    h.attempt = 0;
    h.saidBehind = false;
    h.taken.add(ws);
    ws.once("close", () => {
      h.taken.delete(ws);
      if (h.taken.size < PIPE_POOL_MAX) h.saidFull = false;
      fill(h);
    });
    // Wrapped before anything else runs, so the binary frames right behind pipe.open reach the stream.
    const duplex = createWebSocketStream(ws);
    duplex.on("error", () => {});
    const handler = o.handler();
    const room = h.taken.size + h.idle.size < PIPE_POOL_MAX;
    log(`${h.id}: a pipe opened on the ${open.door} door${open.heldMs > 0 ? ` after ${open.heldMs} ms held` : ""}, ${room ? "dialling its replacement" : "its replacement waits for a pipe to close"}`);
    if (handler === undefined) {
      log(`${h.id}: nothing here serves pipes yet, so that connection was closed`);
      duplex.destroy();
    } else {
      try {
        handler(h.id, open, duplex);
      } catch (e) {
        log(`${h.id}: the pipe's handler failed (${e instanceof Error ? e.message : String(e)})`);
        duplex.destroy();
      }
    }
    fill(h);
  };

  const dial = (h: Held): void => {
    const pipe: Pipe = { open: false, authed: false, done: false };
    h.idle.add(pipe);
    const failed = (): void => {
      if (pipe.done) return;
      end(pipe);
      h.idle.delete(pipe);
      if (!h.stopped) linkDown(h);
    };
    let timer: (() => void) | undefined = clock.schedule(failed, PIPE_DIAL_MS, { unref: true });
    void o.reach(h.id).then(reach => {
      if (pipe.done) return;
      if (reach === undefined) return failed();
      const ws = new WebSocket(daemonWsUrl(reach.url), { handshakeTimeout: PIPE_DIAL_MS });
      pipe.ws = ws;
      ws.on("error", () => {});
      ws.once("open", () => {
        timer?.();
        timer = undefined;
        if (pipe.done) return;
        pipe.open = true;
        ws.send(JSON.stringify({ id: 0, op: "auth", token: reach.token, pipe: true }));
        pipe.cancelIdle = clock.schedule(() => retire(h, pipe), idleMaxMs, { unref: true });
      });
      // The daemon answers the auth frame as it answers a link's, then says nothing until a connection takes the
      // pipe. Any other frame is a daemon that serves no pipes answering the socket as a link, with its hello.
      const frame = (data: Buffer, isBinary: boolean): void => {
        if (pipe.done) return;
        const text = isBinary ? undefined : data.toString();
        const auth = text === undefined ? undefined : authAnswer(text);
        if (auth === true) {
          pipe.authed = true;
          // Back once the daemon has taken the token on a full pool, not on a socket that opened: a daemon refusing
          // pipes past its own cap closes them after they open, and its refusals keep backing off.
          if (h.attempt > 0 && [...h.idle].filter(p => p.authed).length === PIPE_IDLE) {
            h.attempt = 0;
            log(`${h.id}: ${PIPE_IDLE} pipes idle`);
          }
          return;
        }
        ws.off("message", frame);
        if (auth === false) return refused(h);
        const open = text === undefined ? undefined : pipeOpenOf(text);
        if (open === undefined) return behind(h, `${h.id}: its daemon answered a pipe as a link, so it is asked again each minute`);
        take(h, pipe, ws, open);
      };
      ws.on("message", frame);
      ws.once("close", (code: number, reason: Buffer) => {
        timer?.();
        timer = undefined;
        if (pipe.done) return;
        if (daemonRefusedToken(code, reason.toString())) {
          pipe.done = true;
          h.idle.delete(pipe);
          return refused(h);
        }
        failed();
      });
    }, failed);
  };

  const down = (id: string): void => {
    const h = held.get(id);
    if (h === undefined) return;
    held.delete(id);
    h.stopped = true;
    h.retry?.();
    h.pace?.();
    endIdle(h);
    for (const ws of h.taken) ws.terminate();
    h.taken.clear();
  };

  return {
    up(id) {
      if (held.has(id)) return;
      const h: Held = { id, idle: new Set(), taken: new Set(), attempt: 0, dialled: [], stopped: false, refusals: tokenRefusals(behindAgainMs), serves: false, asking: false, saidBehind: false, saidDialling: false, saidFull: false };
      held.set(id, h);
      fill(h);
    },
    down,
    idle: id => [...(held.get(id)?.idle ?? [])].filter(p => p.open).length,
    close() {
      for (const id of [...held.keys()]) down(id);
    },
  };
}

/** Cuts a machine off at the host's broker before its provider is asked to delete it, so no call from it lands in
 * between, nor after a delete the provider refuses. A broker that fails says so, and the delete goes on. */
export async function cutOff(ctx: Pick<RuntimeContext, "opts">, entry: LiveWorkspace): Promise<void> {
  await ctx.opts.broker?.revoke?.(entry.record.id).catch((e: unknown) => console.warn(`${entry.record.name} was not cut off from your computer before its delete: ${e instanceof Error ? e.message : String(e)}`));
}

/** The pool as the runtime runs it: a brokered workspace's pipes stand while it runs, by the events the host's held
 * link to it follows, and are dialled again on a new machine. Nothing here where the host wired no broker. */
export function pipeArea(ctx: RuntimeContext, brokered: (record: WorkspaceRecord) => boolean): { close(): void } | undefined {
  const broker = ctx.opts.broker;
  if (broker === undefined) return undefined;
  const pool = pipePool({
    reach: async id => {
      const entry = ctx.live.get(id);
      if (entry === undefined) return undefined;
      const route = await entry.ws.daemonReach();
      const token = await ctx.daemonTokenOf(entry.machine);
      return token === undefined ? undefined : { url: route.url, token };
    },
    version: async id => {
      const entry = ctx.live.get(id);
      return entry === undefined ? null : ctx.moduleOf(entry.record.kind).daemonVersion(entry);
    },
    handler: () => (broker.pipe === undefined ? undefined : broker.pipe.bind(broker)),
    ready: () => broker.ready(),
    rewrite: id => {
      const entry = ctx.live.get(id);
      if (entry !== undefined) ctx.daemonTokens.delete(entry.machine.id);
    },
    clock: ctx.clock,
  });
  const upIf = (id: string): void => {
    const entry = ctx.live.get(id);
    if (entry !== undefined && entry.record.phase === "running" && brokered(entry.record)) pool.up(id);
  };
  const detach = ctx.bus.on("*", e => {
    switch (e.type) {
      // A create's own clone reaches GitHub through these pipes, so they are dialled from its first stage after the
      // machine is bound; one that fails takes them down with it.
      case "workspace.creating":
        if (e.stage === "failed") pool.down(e.workspaceId);
        else upIf(e.workspaceId);
        return;
      case "workspace.created":
        if (e.workspace.phase === "running") upIf(e.workspace.id);
        return;
      case "workspace.woken":
      case "workspace.upgraded":
        pool.down(e.workspaceId);
        upIf(e.workspaceId);
        return;
      case "workspace.napped":
      case "workspace.gone":
      case "workspace.deleted":
        pool.down(e.workspaceId);
        return;
      default:
        return;
    }
  });
  void ctx.ready().then(
    () => {
      for (const id of ctx.live.keys()) upIf(id);
    },
    () => {},
  );
  return {
    close() {
      detach();
      pool.close();
    },
  };
}
