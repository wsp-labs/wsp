// SPDX-License-Identifier: AGPL-3.0-only
// The tokens the host mints for a machine whose credentials it keeps: one per
// op whose git or gh reaches the git host, taken away at its reply, and one per
// shell a pane opens, taken away when the shell ends. The daemon is a fake that
// keeps every frame and asks the host's own token table whether each token it
// is handed is live while it answers.
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join as joinPath } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HOST_TOKEN_ENV, PIPE_DAEMON_VERSION, THREAD_ENV, TOOLS_PATH, type DaemonFrame, type DaemonResponse, type TurnResult } from "@wsp/protocol";
import { buildEnv as claudeEnv, type BrokeredSignIn } from "@wsp/adapter-claude";
import { KEY_FILE_ENV } from "@wsp/adapter-codex";
import { createRuntime, type HarnessAdapterFactory, type Runtime, type RuntimeOptions } from "../src/runtime.js";
import { INLINE_EXEC_MS, PROFILE_PATH_FILE } from "@wsp/engine";
import { BROKERED_BIN_DIR, TERMINAL_ENV, TERMINAL_PROFILE } from "../src/machines/brokered.js";
import type { DaemonChannel, DaemonChannelOptions } from "../src/daemon-channel.js";
import { memoryStore, type Store } from "../src/store.js";
import { createOn, projectOn, stubBackend, tokenGuest, withDaemonRoads } from "./stub-backend.js";
import { answering, box, HETZNER, type Box, type Started } from "./box-fixture.js";
import { ctx, sockets, code, join } from "./places-fixture.js";
import { report, wiring } from "./place-join.js";
import { newPlaceKeyPair } from "../src/places.js";
import { serveRuntime } from "../src/serve.js";

const DAEMON_TOKEN = "cafef00d".repeat(3);

/** The environment each adapter was made with, a turn's launch among them. */
const adapterEnvs: Readonly<Record<string, string>>[] = [];
/** The Claude Code sign-in each adapter was handed on a brokered machine, a turn's launch among them. */
const adapterSignIns: (BrokeredSignIn | undefined)[] = [];
/** A turn that answers at once, so a thread stands on the machine with no turn running. */
const answers: HarnessAdapterFactory = made => (adapterEnvs.push(made.env), adapterSignIns.push(made.brokered), {
  steers: false,
  start: ({ onEvent }) => {
    const sessionId = "44444444-4444-4444-8444-444444444444";
    const result: TurnResult = { status: "completed", text: "done" };
    onEvent({ type: "session.start", sessionId });
    onEvent({ type: "turn.done", sessionId, result });
    onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
    return { localId: sessionId, finished: Promise.resolve(result), interrupt: async () => {} };
  },
});

interface Daemon {
  open: (o: DaemonChannelOptions) => Promise<DaemonChannel>;
  frames: Record<string, unknown>[];
  /** Whether the token each frame carried named a device of the host's while the daemon answered it. */
  liveAtOp: boolean[];
  /** The name of that device, where there was one. */
  namesAtOp: string[];
  /** Pushes an event to every channel still open, as the daemon pushes a pty's exit. */
  push: (event: Record<string, unknown>) => void;
  /** The ptys the daemon holds, which a pty.list answers with. */
  ptys: Set<string>;
  /** Ops the daemon refuses, after asking about the token like any other. */
  refuses: Set<string>;
  /** What a pty.create waits on before the daemon answers it, where set. */
  gate: { create?: Promise<void> };
}

/** A daemon of `version` that says hello as each channel opens and answers what a machine would. */
function fakeDaemon(live: () => Runtime, version = PIPE_DAEMON_VERSION): Daemon {
  const frames: Record<string, unknown>[] = [];
  const liveAtOp: boolean[] = [];
  const namesAtOp: string[] = [];
  const listeners = new Set<(event: Record<string, unknown>) => void>();
  const ptys = new Set<string>();
  const refuses = new Set<string>();
  const gate: { create?: Promise<void> } = {};
  let made = 0;
  const answer = (frame: Record<string, unknown>): Record<string, unknown> => {
    switch (frame["op"]) {
      case "git.push":
        return { branch: "pricing-page", base: String(frame["base"] ?? ""), remote: "origin", ahead: 1, uncommitted: 0, stat: [] };
      case "git.pr":
        return { pr: { number: 7, url: "https://github.com/acme/lab/pull/7", state: "open", host: "github.com", draft: false, base: "main", branch: "pricing-page", headOid: "abc1234", headSubject: "Work", mergeable: "unknown", mergeState: "unknown", review: "none", checks: [], additions: 1, deletions: 0, changedFiles: 1, commits: 1 }, created: true };
      case "pty.create": {
        const ptyId = `p${++made}`;
        ptys.add(ptyId);
        return { ptyId, pid: 4000 + made };
      }
      case "pty.kill":
        ptys.delete(String(frame["ptyId"]));
        return {};
      case "pty.list":
        return { ptys: [...ptys].map(id => ({ id })) };
      default:
        return {};
    }
  };
  return {
    frames,
    liveAtOp,
    namesAtOp,
    ptys,
    refuses,
    gate,
    push: event => {
      for (const fn of listeners) fn(event);
    },
    open: async o => {
      const heard = (event: Record<string, unknown>): void => o.onEvent?.(event);
      listeners.add(heard);
      // The hello follows the auth reply, which is what resolves the dial.
      setImmediate(() => o.onEvent?.({ type: "daemon.hello", version }));
      return {
        send: async (frame: DaemonFrame) => {
          const held = frame as unknown as Record<string, unknown>;
          frames.push(held);
          if (held["op"] === "pty.create") await gate.create;
          const env = held["env"] as Record<string, string> | undefined;
          const token = typeof held["token"] === "string" ? held["token"] : env?.[HOST_TOKEN_ENV];
          if (token !== undefined) {
            const device = await live().devices!.match(token);
            liveAtOp.push(device !== undefined);
            if (device !== undefined) namesAtOp.push(device.name);
          }
          if (refuses.has(String(held["op"]))) return { id: 1, ok: false, error: `${String(held["op"])} was refused` };
          return { id: 1, ok: true, ...answer(held) } as DaemonResponse;
        },
        close: () => listeners.delete(heard),
        closed: new Promise(() => {}),
      };
    },
  };
}

/** The branch a brokered create would otherwise read with gh on this computer, which no runtime here wires. */
const ofRecord = (branch: string) => ({ repoId: 4242, owner: "acme", name: "lab", branch });

let rt: Runtime | undefined;
afterEach(async () => {
  await rt?.close();
  rt = undefined;
});

/** A runtime whose broker reads `ready`, with one fork at its own provider and a thread on it. */
async function brokeredFork(o: { ready?: () => boolean; version?: number; store?: Store; noThread?: true; broker?: Omit<NonNullable<RuntimeOptions["broker"]>, "ready">; harness?: "codex" } = {}): Promise<{ daemon: Daemon; id: string; threadId: string; folder: string; store: Store; backend: ReturnType<typeof stubBackend>; projectId: string }> {
  const backend = stubBackend();
  backend.execImpl = tokenGuest;
  withDaemonRoads(backend);
  const daemon = fakeDaemon(() => rt!, o.version);
  const store = o.store ?? memoryStore();
  rt = createRuntime({ backend, store, adapters: { claude: answers, codex: answers }, daemonToken: DAEMON_TOKEN, daemonChannel: daemon.open, broker: { codexClaims: async () => undefined, ...o.broker, ready: o.ready ?? (() => true) } });
  const project = await projectOn(rt, undefined, undefined, { base: "main" });
  const ws = await createOn(rt, { project: project.id, golden: "snap_g", name: "pricing page", branchOfRecord: ofRecord("wsp/pricing-page") });
  if (o.noThread === true) return { daemon, id: ws.id, threadId: "", folder: "", store, backend, projectId: project.id };
  const turn = await rt.sessions.start(ws.id, { prompt: "build the pricing page", ...(o.harness !== undefined ? { harness: o.harness } : {}) });
  await turn.finished;
  // Where the thread works, as its turn's own reads of the checkout named it.
  const folder = String(daemon.frames.find(f => typeof f["cwd"] === "string")?.["cwd"]);
  return { daemon, id: ws.id, threadId: turn.view().threadId!, folder, store, backend, projectId: project.id };
}

const tokensOf = (frames: Record<string, unknown>[]): unknown[] => frames.map(f => f["token"]);
/** The bring back's two ops, which reach the git host, out of every frame the daemon got. */
const bringBackOps = (frames: Record<string, unknown>[]): Record<string, unknown>[] => frames.filter(f => f["op"] === "git.push" || f["op"] === "git.pr");

describe("an op token on a machine whose credentials the host keeps", () => {
  it("rides each git op as a fresh token, live while the daemon answers it and taken away at its reply", async () => {
    const { daemon, id, threadId } = await brokeredFork();
    await rt!.workspaces.bringBack({ workspaceId: id, title: "the pricing page" });
    const ops = bringBackOps(daemon.frames);
    expect(ops.map(f => f["op"])).toEqual(["git.push", "git.pr"]);
    const [push, pr] = tokensOf(ops) as string[];
    expect(typeof push).toBe("string");
    expect(typeof pr).toBe("string");
    expect(push).not.toBe(pr);
    expect(daemon.liveAtOp).toEqual([true, true]);
    expect(daemon.namesAtOp).toEqual(["op git.push", "op git.pr"]);
    expect(await rt!.devices!.match(push!)).toBeUndefined();
    expect(await rt!.devices!.match(pr!)).toBeUndefined();
    // Scoped to this workspace's thread, which is what the broker matches against the link the pipe came on.
    const scopes = (await rt!.devices!.list()).flatMap(d => (d.scope === undefined ? [] : [d.scope]));
    expect(scopes.every(s => s.workspaceId === id && s.threadId === threadId)).toBe(true);
  });

  it("is taken away when the daemon refuses the op too", async () => {
    const { daemon, id } = await brokeredFork();
    daemon.refuses.add("git.push");
    await expect(rt!.workspaces.bringBack({ workspaceId: id })).rejects.toThrow("git.push was refused");
    const [push] = tokensOf(daemon.frames.filter(f => f["op"] === "git.push")) as string[];
    expect(daemon.liveAtOp).toEqual([true]);
    expect(await rt!.devices!.match(push!)).toBeUndefined();
  });

  it("is not minted while the broker is not ready, nor for a daemon from before op tokens", async () => {
    const off = await brokeredFork({ ready: () => false });
    await rt!.workspaces.bringBack({ workspaceId: off.id });
    expect(tokensOf(bringBackOps(off.daemon.frames))).toEqual([undefined, undefined]);
    await rt!.close();
    const old = await brokeredFork({ version: PIPE_DAEMON_VERSION - 1 });
    await rt!.workspaces.bringBack({ workspaceId: old.id });
    expect(tokensOf(bringBackOps(old.daemon.frames))).toEqual([undefined, undefined]);
    expect((await rt!.devices!.list()).filter(d => d.scope !== undefined)).toEqual([]);
  });
});

describe("an op on a machine whose credentials the host keeps, with no thread on it yet", () => {
  it("is refused once the broker is ready, before anything reaches the daemon", async () => {
    const { daemon, id } = await brokeredFork({ noThread: true });
    const refused = await rt!.workspaces.bringBack({ workspaceId: id }).catch((e: unknown) => e as Error & { kind?: string });
    expect(refused).toBeInstanceOf(Error);
    expect((refused as Error).message).toBe("pricing page has no thread yet, so wsp cannot reach GitHub for it. Start a thread on it first.");
    expect((refused as { kind?: string }).kind).toBe("usage");
    expect(bringBackOps(daemon.frames)).toEqual([]);
  });

  it("runs as it always has while the broker is not ready", async () => {
    const { daemon, id } = await brokeredFork({ noThread: true, ready: () => false });
    await rt!.workspaces.bringBack({ workspaceId: id });
    expect(tokensOf(bringBackOps(daemon.frames))).toEqual([undefined, undefined]);
  });
});

describe("a joined computer's frames", () => {
  afterEach(async () => {
    for (const ws of sockets.splice(0)) ws.close();
    await ctx.srv?.close();
    ctx.srv = undefined;
    ctx.runtime = undefined;
  });

  it("carry no token while the broker is ready, from a thread in a folder there", async () => {
    const starts: Started[] = [];
    // The box fixture's own joined(), with a broker that is ready.
    const hostKey = newPlaceKeyPair();
    ctx.runtime = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: answering(starts) }, placeLinks: wiring(hostKey), broker: { ready: () => true, codexClaims: async () => undefined } });
    ctx.srv = await serveRuntime(ctx.runtime, { port: 0, authToken: "host-token", devices: ctx.runtime.devices });
    let seen!: Box;
    const { client } = await join(hostKey, { code: await code(), report: report("hetzner", { login: { HOME: HETZNER.home, USER: "root", PATH: "/usr/bin" } }), answers: c => (seen = box(c, HETZNER)) });
    sockets.push(client.ws);
    const project = await ctx.runtime.projects.add({ source: "https://github.com/spoo-me/spoo-ts", on: "hetzner", name: "spoo-ts" });
    const joined = { rt: ctx.runtime, project, seen };
    const folder = await joined.rt.workspaces.create({ project: joined.project.id, name: "spoo-ts" });
    await (await joined.rt.sessions.start(folder.id, { prompt: "fix the redirect" })).finished;
    await joined.rt.workspaces.bringBack({ workspaceId: folder.id }).catch(() => undefined);
    const channel = await joined.rt.workspaces.daemonChannel(folder.id, () => {});
    await channel.send({ op: "pty.create", cols: 80, rows: 24 } as never);
    channel.close();
    const push = joined.seen.frames.filter(f => f["op"] === "git.push");
    expect(push.length).toBe(1);
    expect(push[0]).not.toHaveProperty("token");
    const pty = joined.seen.frames.filter(f => f["op"] === "pty.create");
    expect(pty.length).toBe(1);
    expect(Object.keys((pty[0]!["env"] ?? {}) as Record<string, string>)).not.toContain(HOST_TOKEN_ENV);
    expect((await joined.rt.devices!.list()).filter(d => d.scope !== undefined && d.scope.workspaceId === folder.id)).toEqual([]);
  });
});

describe("a terminal token on a machine whose credentials the host keeps", () => {
  /** The pty.create frames the daemon got after a pane on the workspace sent each frame in `sent`, on one channel. */
  async function opened(daemon: Daemon, id: string, sent: Record<string, unknown>[]): Promise<{ frames: Record<string, unknown>[]; channel: DaemonChannel }> {
    const before = daemon.frames.length;
    const channel = await rt!.workspaces.daemonChannel(id, () => {});
    for (const frame of sent) await channel.send({ op: "pty.create", cols: 80, rows: 24, ...frame } as never);
    return { frames: daemon.frames.slice(before).filter(f => f["op"] === "pty.create"), channel };
  }
  const tokenIn = (frame: Record<string, unknown>): string => (frame["env"] as Record<string, string>)[HOST_TOKEN_ENV]!;

  it("opens the first tab in the thread's folder with a token of its own, and every later tab with another", async () => {
    const { daemon, id, threadId, folder } = await brokeredFork();
    const { frames, channel } = await opened(daemon, id, [{ thread: threadId }, {}]);
    channel.close();
    expect(frames.map(f => f["cwd"])).toEqual([folder, folder]);
    expect(frames[0]).not.toHaveProperty("thread");
    const [first, second] = frames.map(tokenIn);
    expect(typeof first).toBe("string");
    expect(first).not.toBe(second);
    expect((frames[0]!["env"] as Record<string, string>)[THREAD_ENV]).toBe(threadId);
    expect(daemon.liveAtOp).toEqual([true, true]);
    const device = await rt!.devices!.match(first!);
    expect(device?.scope).toEqual({ kind: "thread", threadId, workspaceId: id, rootThreadId: threadId });
    // A pane that goes leaves its shells running on the machine, and their tokens with them.
    expect(await rt!.devices!.match(second!)).toBeDefined();
  });

  it("carries the git and gh variables a turn's launch does, on the shell's own token", async () => {
    const { daemon, id, threadId } = await brokeredFork();
    const turn = [...adapterEnvs].reverse().find(env => env[HOST_TOKEN_ENV] !== undefined)!;
    const { frames, channel } = await opened(daemon, id, [{ thread: threadId }]);
    channel.close();
    const shell = frames[0]!["env"] as Record<string, string>;
    const gitAndGh = (env: Readonly<Record<string, string>>): Record<string, string> =>
      Object.fromEntries(Object.entries(env).filter(([name]) => /^(GIT_|GH_)/.test(name)).map(([name, value]) => [name, value.replaceAll(env[HOST_TOKEN_ENV]!, "<token>")]));
    expect(gitAndGh(turn)).toMatchObject({ GIT_TERMINAL_PROMPT: "0", GH_TOKEN: "<token>" });
    expect(gitAndGh(shell)).toEqual(gitAndGh(turn));
    expect(shell["GH_TOKEN"]).toBe(tokenIn(frames[0]!));
  });

  it("carries the sign-in a Claude Code turn is handed, on the shell's own token", async () => {
    const { daemon, id, threadId } = await brokeredFork();
    const turn = [...adapterSignIns].reverse().find(signIn => signIn !== undefined)!;
    const { frames, channel } = await opened(daemon, id, [{ thread: threadId }]);
    channel.close();
    const shell = frames[0]!["env"] as Record<string, string>;
    // What Claude Code's launch puts on a brokered turn and on no other, as its adapter builds the turn's environment.
    const brokered = claudeEnv({ brokered: turn });
    const plain = claudeEnv({});
    const names = Object.keys(brokered).filter(name => brokered[name] !== plain[name]);
    expect(names).toEqual(expect.arrayContaining(["ANTHROPIC_BASE_URL", "CLAUDE_CODE_OAUTH_TOKEN"]));
    const signIn = (env: Readonly<Record<string, string>>, token: string): Record<string, string | undefined> => Object.fromEntries(names.map(name => [name, env[name]?.replaceAll(token, "<token>")]));
    expect(signIn(shell, tokenIn(frames[0]!))).toEqual(signIn(brokered, turn.token));
  });

  /** A fork whose host holds a Codex login of `codex`'s kind, its thread on `harness`, whose machine answers the line
   * writing codex's program for a shell with `answer` once `written` resolves, and the lines it was handed. */
  async function codexFork(codex: "chatgpt" | "key", o: { harness?: "codex"; written?: Promise<void>; answer?: { exitCode: number; stderr: string } } = { harness: "codex" }) {
    const fork = await brokeredFork({ broker: { status: async () => ({ codex }), codexClaims: async () => ({ accountId: "acct-acme" }) }, ...(o.harness !== undefined ? { harness: o.harness } : {}) });
    const lines: string[] = [];
    const guest = fork.backend.execImpl;
    const written = o.written ?? Promise.resolve();
    fork.backend.execImpl = (m, cmd, stdin) => (cmd.includes(BROKERED_BIN_DIR) ? (lines.push(cmd), written.then(() => ({ exitCode: 0, stderr: "", ...o.answer, stdout: "" }))) : guest(m, cmd, stdin));
    return { ...fork, lines };
  }

  it("opens a Codex thread's shell marked for the profile that finds codex's program first", async () => {
    const { daemon, id, threadId, lines } = await codexFork("chatgpt");
    const { frames, channel } = await opened(daemon, id, [{ thread: threadId }]);
    channel.close();
    const shell = frames[0]!["env"] as Record<string, string>;
    expect(lines).toHaveLength(1);
    expect(lines[0]).toContain("openai_base_url");
    expect(lines[0]).toContain(`> ${TERMINAL_PROFILE}.$$`);
    // profile.d runs in name order, and the image's own file sets the PATH outright.
    expect([TERMINAL_PROFILE, PROFILE_PATH_FILE].sort()).toEqual([PROFILE_PATH_FILE, TERMINAL_PROFILE]);
    expect(shell[TERMINAL_ENV]).toBe("codex");
    expect(shell["PATH"]).toBe(TOOLS_PATH);
    expect(shell["CODEX_REFRESH_TOKEN_URL_OVERRIDE"]).toMatch(/^http:\/\/127\.0\.0\.1:\d+\/oauth\/token$/);
    expect(shell).not.toHaveProperty(KEY_FILE_ENV);
    // The broker sends this thread's calls to OpenAI alone, so Claude Code there gets no placeholder to send.
    expect(shell).not.toHaveProperty("ANTHROPIC_BASE_URL");
  });

  it("opens a machine's first Codex shell once codex's program is written there, and a later one without waiting on a write", async () => {
    let write!: () => void;
    const { daemon, id, threadId, lines, backend } = await codexFork("chatgpt", { harness: "codex", written: new Promise(resolve => (write = resolve)) });
    const channel = await rt!.workspaces.daemonChannel(id, () => {});
    const creates = (): number => daemon.frames.filter(f => f["op"] === "pty.create").length;
    const first = channel.send({ op: "pty.create", cols: 80, rows: 24, thread: threadId } as never);
    await vi.waitFor(() => expect(lines).toHaveLength(1));
    expect(creates()).toBe(0);
    write();
    await first;
    expect(creates()).toBe(1);
    const guest = backend.execImpl;
    backend.execImpl = (m, cmd, stdin) => (cmd.includes(BROKERED_BIN_DIR) ? (lines.push(cmd), new Promise(() => {})) : guest(m, cmd, stdin));
    await channel.send({ op: "pty.create", cols: 80, rows: 24, thread: threadId } as never);
    channel.close();
    expect(lines).toHaveLength(2);
    expect(creates()).toBe(2);
  });

  it("opens a machine's first Codex shell after an inline exec's span where the write there never answers", async () => {
    const { daemon, id, threadId, lines } = await codexFork("chatgpt", { harness: "codex", written: new Promise(() => {}) });
    const channel = await rt!.workspaces.daemonChannel(id, () => {});
    const creates = (): number => daemon.frames.filter(f => f["op"] === "pty.create").length;
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      const first = channel.send({ op: "pty.create", cols: 80, rows: 24, thread: threadId } as never);
      await vi.waitFor(() => expect(lines).toHaveLength(1));
      await vi.advanceTimersByTimeAsync(INLINE_EXEC_MS - 1_000);
      expect(creates()).toBe(0);
      await vi.advanceTimersByTimeAsync(1_000);
      await first;
      expect(creates()).toBe(1);
    } finally {
      vi.useRealTimers();
      channel.close();
    }
  });

  it("opens the shell where the machine refuses the write, and says in the host's log why its codex is not signed in", async () => {
    const warned = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const { daemon, id, threadId, lines } = await codexFork("chatgpt", { harness: "codex", answer: { exitCode: 1, stderr: "mkdir: cannot create directory '/usr/local/libexec/wsp-terminal': Read-only file system\nsecond line\n" } });
      const { frames, channel } = await opened(daemon, id, [{ thread: threadId }]);
      channel.close();
      expect(lines).toHaveLength(1);
      expect(frames[0]!["env"]).toMatchObject({ [TERMINAL_ENV]: "codex" });
      const said = warned.mock.calls.map(call => String(call[0])).filter(line => line.includes("terminal programs"));
      expect(said).toHaveLength(1);
      expect(said[0]).toContain("for codex on");
      expect(said[0]).toMatch(/exit 1, mkdir: cannot create directory '\/usr\/local\/libexec\/wsp-terminal': Read-only file system$/);
    } finally {
      warned.mockRestore();
    }
  });

  it("writes a profile that puts codex's program first in a login shell carrying the mark, and in no other", async () => {
    const { daemon, id, threadId, lines } = await codexFork("chatgpt");
    const { channel } = await opened(daemon, id, [{ thread: threadId }]);
    channel.close();
    const dir = mkdtempSync(joinPath(tmpdir(), "terminal-codex-"));
    try {
      const line = lines[0]!.slice(lines[0]!.indexOf("mkdir -p")).replaceAll(BROKERED_BIN_DIR, joinPath(dir, "bin")).replaceAll(TERMINAL_PROFILE, joinPath(dir, "profile.sh"));
      execFileSync("sh", ["-c", line]);
      // As the image's profile leaves it, the PATH set outright before this one runs.
      const found = (env: Record<string, string>): string => execFileSync("sh", ["-c", `PATH=/usr/bin:/bin; . "${dir}/profile.sh"; command -v codex || true`], { env, encoding: "utf8" }).trim();
      expect(found({ [TERMINAL_ENV]: "codex" })).toBe(joinPath(dir, "bin", "codex", "codex"));
      expect(found({ [TERMINAL_ENV]: "claude" })).toBe("");
      expect(found({})).toBe("");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("hands codex its placeholder key on the shell's own token, where the host holds a Codex key", async () => {
    const { daemon, id, threadId } = await codexFork("key");
    const { frames, channel } = await opened(daemon, id, [{ thread: threadId }]);
    channel.close();
    expect((frames[0]!["env"] as Record<string, string>)[KEY_FILE_ENV]).toBe(`sk-wsp-${tokenIn(frames[0]!)}`);
  });

  /** `typed` in a login shell marked with `own`, on a machine where terminals' opens ran `lines` in turn, their paths
   * moved into a folder of their own: what it printed on each stream and how it exited. */
  function typedAfter(lines: readonly string[], typed: string, own: string): { out: string; said: string; status: number | null } {
    const dir = mkdtempSync(joinPath(tmpdir(), "terminal-"));
    try {
      for (const line of lines) execFileSync("sh", ["-c", line.slice(line.indexOf("mkdir -p")).replaceAll(BROKERED_BIN_DIR, joinPath(dir, "bin")).replaceAll(TERMINAL_PROFILE, joinPath(dir, "profile.sh"))]);
      const ran = spawnSync("sh", ["-c", `PATH=/usr/bin:/bin; . "${dir}/profile.sh"; ${typed}`], { env: { [TERMINAL_ENV]: own }, encoding: "utf8", input: "" });
      return { out: ran.stdout.replaceAll(dir, "<dir>"), said: ran.stderr, status: ran.status };
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }

  it("answers codex typed on a Claude Code thread with why it cannot sign in there, though the host holds a Codex login", async () => {
    const { daemon, id, threadId, lines } = await codexFork("chatgpt", {});
    const { frames, channel } = await opened(daemon, id, [{ thread: threadId }]);
    channel.close();
    expect(frames[0]!["env"]).toMatchObject({ ANTHROPIC_BASE_URL: expect.any(String), [TERMINAL_ENV]: "claude" });
    expect(frames[0]!["env"]).not.toHaveProperty("CODEX_REFRESH_TOKEN_URL_OVERRIDE");
    expect(lines).toHaveLength(1);
    expect(lines[0]).not.toContain("openai_base_url");
    expect(typedAfter(lines, 'codex exec "say ok"', "claude")).toEqual({ out: "", said: "wsp: Codex cannot sign in on this machine. This terminal's thread runs Claude Code, and your computer signs in only that agent here. Start a thread on Codex to use it.\n", status: 1 });
  });

  it("answers claude typed on a Codex thread with why it cannot sign in there", async () => {
    const { daemon, id, threadId, lines } = await codexFork("chatgpt");
    const { frames, channel } = await opened(daemon, id, [{ thread: threadId }]);
    channel.close();
    expect(frames[0]!["env"]).not.toHaveProperty("ANTHROPIC_BASE_URL");
    expect(typedAfter(lines, 'claude -p "say ok"', "codex")).toEqual({ out: "", said: "wsp: Claude Code cannot sign in on this machine. This terminal's thread runs Codex, and your computer signs in only that agent here. Start a thread on Claude Code to use it.\n", status: 1 });
  });

  it("signs in each thread's own agent in its own terminal where threads of both agents share the machine", async () => {
    const { daemon, id, threadId: claudeThread, lines } = await codexFork("chatgpt", {});
    const codexTurn = await rt!.sessions.start(id, { prompt: "review the pricing page", harness: "codex" });
    await codexTurn.finished;
    const codexPane = await opened(daemon, id, [{ thread: codexTurn.view().threadId }]);
    const claudePane = await opened(daemon, id, [{ thread: claudeThread }]);
    codexPane.channel.close();
    claudePane.channel.close();
    expect(codexPane.frames[0]!["env"]).toMatchObject({ [TERMINAL_ENV]: "codex", CODEX_REFRESH_TOKEN_URL_OVERRIDE: expect.any(String) });
    expect(claudePane.frames[0]!["env"]).toMatchObject({ [TERMINAL_ENV]: "claude", ANTHROPIC_BASE_URL: expect.any(String) });
    // The Claude Code thread's terminal opened last, and codex in the Codex thread's terminal is still its program.
    expect(typedAfter(lines, "command -v codex", "codex").out).toBe("<dir>/bin/codex/codex\n");
    expect(typedAfter(lines, 'codex exec "say ok"', "claude").said).toMatch(/^wsp: Codex cannot sign in on this machine\. This terminal's thread runs Claude Code\b/);
    expect(typedAfter(lines, 'claude -p "say ok"', "codex").said).toMatch(/^wsp: Claude Code cannot sign in on this machine\. This terminal's thread runs Codex\b/);
  });

  it("dies with its shell: at the exit the daemon pushes, at a kill, and at a list that no longer holds it", async () => {
    const { daemon, id } = await brokeredFork();
    const { frames, channel } = await opened(daemon, id, [{}, {}, {}]);
    const [exits, killed, vanished] = frames.map(tokenIn);
    const [exitsPty, killedPty, vanishedPty] = ["p1", "p2", "p3"];
    daemon.push({ type: "pty.exit", ptyId: exitsPty, code: 0 });
    await expect.poll(async () => rt!.devices!.match(exits!)).toBeUndefined();
    expect(await rt!.devices!.match(killed!)).toBeDefined();
    await channel.send({ op: "pty.kill", ptyId: killedPty } as never);
    expect(await rt!.devices!.match(killed!)).toBeUndefined();
    channel.close();
    // Gone while no pane listened: the next pane's list is what says so.
    daemon.ptys.delete(vanishedPty!);
    const next = await rt!.workspaces.daemonChannel(id, () => {});
    await next.send({ op: "pty.list" } as never);
    next.close();
    expect(await rt!.devices!.match(vanished!)).toBeUndefined();
  });

  it("outlives a host restart while its shell stands, with no turn running, and dies with the shell after", async () => {
    const store = memoryStore();
    const { daemon, id, backend } = await brokeredFork({ store });
    const { frames, channel } = await opened(daemon, id, [{}]);
    channel.close();
    const token = tokenIn(frames[0]!);
    await rt!.close();
    rt = createRuntime({ backend, store, adapters: { claude: answers }, daemonToken: DAEMON_TOKEN, daemonChannel: daemon.open, broker: { ready: () => true, codexClaims: async () => undefined } });
    // The boot has run, and with it the sweep of every token whose thread has no turn running.
    await rt.workspaces.list();
    expect(await rt.devices!.match(token)).toBeDefined();
    const pane = await rt.workspaces.daemonChannel(id, () => {});
    await pane.send({ op: "pty.list" } as never);
    pane.close();
    expect(await rt.devices!.match(token)).toBeDefined();
    daemon.ptys.clear();
    const next = await rt.workspaces.daemonChannel(id, () => {});
    await next.send({ op: "pty.list" } as never);
    next.close();
    expect(await rt.devices!.match(token)).toBeUndefined();
  });

  it("outlives a host restart that finds its thread's turn gone from the machine", async () => {
    const backend = stubBackend();
    backend.execImpl = tokenGuest;
    withDaemonRoads(backend);
    const daemon = fakeDaemon(() => rt!);
    // A turn still running when the host stops, whose run the machine no longer holds when the next host asks.
    const gone: HarnessAdapterFactory = () => ({
      steers: false,
      start: ({ onEvent }) => {
        queueMicrotask(() => onEvent({ type: "session.start", sessionId: "55555555-5555-4555-8555-555555555555" }));
        return { localId: "55555555-5555-4555-8555-555555555555", run: "/tmp/wsp-run/000000000001", finished: new Promise<TurnResult>(() => {}), interrupt: async () => {} };
      },
      attach: async () => "gone",
    });
    const o = { backend, store: memoryStore(), adapters: { claude: gone }, daemonToken: DAEMON_TOKEN, daemonChannel: daemon.open, broker: { ready: () => true, codexClaims: async () => undefined } };
    rt = createRuntime(o);
    const ws = await createOn(rt, { project: (await projectOn(rt, undefined, undefined, { base: "main" })).id, golden: "snap_g", name: "pricing page", branchOfRecord: ofRecord("wsp/pricing-page") });
    const turn = await rt.sessions.start(ws.id, { prompt: "build the pricing page" });
    await expect.poll(async () => (await rt!.sessions.history(ws.id)).some(e => e.type === "session.start")).toBe(true);
    const { frames, channel } = await opened(daemon, ws.id, [{ thread: turn.view().threadId }]);
    channel.close();
    await rt.close();
    rt = createRuntime(o);
    await rt.workspaces.list();
    await expect.poll(async () => (await rt!.sessions.list(ws.id))[0]?.status).toBe("failed");
    await expect.poll(async () => (await rt!.devices!.list()).filter(d => d.scope !== undefined).length).toBe(1);
    expect(await rt.devices!.match(tokenIn(frames[0]!))).toBeDefined();
  });

  it("is not minted while the broker is not ready", async () => {
    const { daemon, id, folder } = await brokeredFork({ ready: () => false });
    const { frames, channel } = await opened(daemon, id, [{}]);
    channel.close();
    expect(frames.map(f => f["cwd"])).toEqual([folder]);
    expect(frames[0]).not.toHaveProperty("env");
  });

  it("is taken away at once when the shell's open answers after its workspace was deleted, and no record is kept", async () => {
    const { daemon, id, store } = await brokeredFork();
    let answer!: () => void;
    daemon.gate.create = new Promise<void>(resolve => (answer = resolve));
    const channel = await rt!.workspaces.daemonChannel(id, () => {});
    const opening = channel.send({ op: "pty.create", cols: 80, rows: 24 } as never);
    await expect.poll(() => daemon.frames.some(f => f["op"] === "pty.create")).toBe(true);
    const token = tokenIn(daemon.frames.find(f => f["op"] === "pty.create")!);
    await rt!.workspaces.delete(id);
    answer();
    await opening;
    channel.close();
    expect(await rt!.devices!.match(token)).toBeUndefined();
    expect(await store.keys("pty-tokens")).not.toContain(id);
  });

  it("is scoped to this workspace's own thread when the pane names a thread of another workspace", async () => {
    const { daemon, id, threadId, projectId } = await brokeredFork();
    const other = await createOn(rt!, { project: projectId, golden: "snap_g", name: "checkout page", branchOfRecord: ofRecord("wsp/checkout-page") });
    const theirs = (await (async () => {
      const turn = await rt!.sessions.start(other.id, { prompt: "build the checkout page" });
      await turn.finished;
      return turn.view().threadId!;
    })());
    expect(theirs).not.toBe(threadId);
    const { frames, channel } = await opened(daemon, id, [{ thread: theirs }]);
    channel.close();
    const device = await rt!.devices!.match(tokenIn(frames[0]!));
    expect(device?.scope).toEqual({ kind: "thread", threadId, workspaceId: id, rootThreadId: threadId });
    expect((frames[0]!["env"] as Record<string, string>)[THREAD_ENV]).toBe(threadId);
  });

  it("goes with its workspace", async () => {
    const { daemon, id } = await brokeredFork();
    const { frames, channel } = await opened(daemon, id, [{}]);
    channel.close();
    const token = tokenIn(frames[0]!);
    await rt!.workspaces.delete(id);
    await expect.poll(async () => rt!.devices!.match(token)).toBeUndefined();
  });
});
