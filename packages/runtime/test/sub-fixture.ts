// SPDX-License-Identifier: AGPL-3.0-only
// A lead thread on this computer with Boat as its cloud: the lead's folder is a
// real repo here, Boat a stub backend that keeps every create, command and file
// and fails a create as told, and the lead's daemon a fake that keeps every frame.
import { execFileSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach } from "vitest";
import { BOX_CLASSES, BOX_WORK_DIR, NAME_LABEL, CreateCancelledError, EnvNotLandedError, GuestUnusableError, tarOf, type BoxLimits, type Machine, type MachineSpec } from "@wsp/engine";
import { boatCancelledLine, HERE_PLACE_ID, HOST_TOKEN_ENV, type SubSize, TURN_TOKEN_ENV, type Caller, type DaemonResponse, type McpServerSpec, type SeedPlan, type TurnResult } from "@wsp/protocol";
import { createRuntime, copyKey, wiredPlace, type HarnessAdapterFactory, type HarnessStartOptions, type Runtime } from "../src/runtime.js";
import type { Clock } from "../src/clock.js";
// The key the picks are kept under, as the seed verb writes it and the spawn reads it: through the folder's links.
import { folderNamed } from "../src/types/wiring.js";
import type { DaemonChannelOptions } from "../src/daemon-channel.js";
import { memoryStore, type Store } from "../src/store.js";
import { newPlaceKeyPair } from "../src/places.js";
import { serveRuntime } from "../src/serve.js";
import { wiring } from "./place-join.js";
import { ctx } from "./places-fixture.js";
import { fakeLocal, stubBackend, type StubBackend, type StubMachine } from "./stub-backend.js";

let root: string | undefined;
afterEach(() => {
  if (root !== undefined) rmSync(root, { recursive: true, force: true });
  root = undefined;
});

/** The owner's account at 17:06 UTC on 2026-10-11: the $20 plan, every class but xlarge, starts left in each window. */
export const PAID: BoxLimits = {
  trial: false,
  planKey: "box_20",
  planUsd: 20,
  machinesAtOnce: 100,
  sizes: ["small", "default", "large"],
  canStart: true,
  starts: { minute: { limit: 12, used: 0 }, hour: { limit: 60, used: 9 }, day: { limit: 200, used: 187 } },
};
export const TRIAL: BoxLimits = { ...PAID, trial: true, planKey: "trial", planUsd: undefined, machinesAtOnce: 2, sizes: ["small", "default"], starts: { minute: { limit: 5, used: 0 }, hour: { limit: 25, used: 0 }, day: { limit: 75, used: 0 } } };
/** The classes each plan sells, as a refusal names them. */
export const PAID_SIZES: SubSize[] = BOX_CLASSES.filter(c => PAID.sizes.includes(c.type)).map(c => ({ name: c.type, cpu: c.cpu, memMb: c.memMb, rateUsdPerHour: c.rateUsdPerHour }));
export const TRIAL_SIZES: SubSize[] = PAID_SIZES.filter(s => TRIAL.sizes.includes(s.name as "small"));

/** The setup image on Boat: v1, its sign-ins as the seal recorded them, and the agents its recipe carries. */
async function sealImage(store: Store, o: { logins?: { name: string; state: "copied" | "signed-in" | "skipped" }[]; agents?: string[] } = {}): Promise<void> {
  const logins = o.logins ?? [{ name: "GitHub CLI login", state: "copied" }];
  await store.put("goldens", copyKey("box", "default"), { head: 1, versions: [{ version: 1, snapshotId: "wsp-host-default-v1", baseTemplate: "base", setupSha: "abc", createdAt: "2026-10-07T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 }, logins }] });
  const rows = (o.agents ?? ["claude", "codex"]).map(id => ({ id, kind: "agent" as const, on: true, source: { kind: "used" as const, sessions: 1, calls: 1 } }));
  await store.put("images", "default", { name: "default", version: 1, hash: "a".repeat(64), recipeHash: "r", recipe: { version: 1, at: "2026-10-07T00:00:00.000Z", histories: [], rows }, pins: [], logins, sealedAt: "2026-10-07T00:00:00.000Z", sealedFrom: "mac", place: "box" });
}

/** What the lead's folder reads as through its daemon: on a branch, tracked and level with its upstream unless told. */
interface LeadGit {
  head: string;
  upstream?: string;
  ahead: number;
  uncommitted: number;
  /** git could not count the branch against its upstream. */
  countsUnknown?: true;
}

/** One failure Boat gives a create: a box that runs nothing, one whose environment did not land, a cancelled create;
 * `replayed` is a box Boat answered a replayed key with, which its own cleanup leaves standing. */
export type Fault = { kind: "unusable" | "env" | "cancelled"; replayed?: true };

export interface Boat extends StubBackend {
  creates: MachineSpec[];
  faults: Fault[];
  limitsRead: BoxLimits;
  /** Every file put on a machine, by its path there. */
  files: { machine: string; path: string; bytes: Buffer }[];
}

/** Boat as a stub: every class's figures, the plan it is asked for, a file road, and a create that fails as told. */
function fakeBoat(): Boat {
  const boat = stubBackend() as Boat;
  boat.creates = [];
  boat.faults = [];
  boat.files = [];
  boat.limitsRead = PAID;
  Object.assign(boat, { capabilities: { ...boat.capabilities, sizes: PAID_SIZES.map(s => ({ cpu: s.cpu, memMb: s.memMb, rateUsdPerHour: s.rateUsdPerHour })) } });
  boat.limits = async () => boat.limitsRead;
  const make = boat.create.bind(boat);
  boat.create = async (spec: MachineSpec): Promise<Machine> => {
    boat.creates.push(spec);
    const m = (await make(spec)) as StubMachine;
    m.putBytes = async (path: string, bytes: Uint8Array) => {
      boat.files.push({ machine: m.id, path, bytes: Buffer.from(bytes) });
    };
    const fault = boat.faults.shift();
    if (fault === undefined) return m;
    // Boat's own cleanup deletes a box it made fresh and leaves a replayed one.
    if (fault.replayed !== true) m.killed = true;
    if (fault.kind === "unusable") throw new GuestUnusableError(m.id, "Boat", `${BOX_WORK_DIR} is missing`, 200);
    if (fault.kind === "env") throw new EnvNotLandedError(m.id, 124, "");
    throw new CreateCancelledError(m.id, "no capacity", boatCancelledLine(spec.labels?.[NAME_LABEL], "no capacity"));
  };
  return boat;
}

/** The lead's daemon: git.status off `git`, every frame kept. */
function leadDaemon(git: LeadGit) {
  const frames: Record<string, unknown>[] = [];
  const open = async (_dial: DaemonChannelOptions) => ({
    send: async (frame: unknown): Promise<DaemonResponse> => {
      const f = frame as Record<string, unknown>;
      frames.push(f);
      if (f["op"] === "git.status") {
        const entries = Array.from({ length: git.uncommitted }, (_, i) => ({ path: `f${i}`, xy: " M" }));
        return { id: 1, ok: true, branch: { oid: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a", head: git.head, ...(git.upstream !== undefined ? { upstream: git.upstream } : {}), ahead: git.ahead, behind: 0 }, entries, root: String(f["cwd"]), ...(git.countsUnknown === true ? { countsUnknown: true } : {}) } as unknown as DaemonResponse;
      }
      return { id: 1, ok: false, error: `${String(f["op"])} was not answered` } as unknown as DaemonResponse;
    },
    close: () => {},
    closed: new Promise<{ code: number; reason: string }>(() => {}),
  });
  return { frames, open };
}

/** Where each agent keeps a session's one file on a machine, as their adapters say: Claude Code under its config dir by
 * folder, Codex under its home by date. */
export const CLAUDE_SESSION_FILE = (id: string) => ({ folder: "/root/.claude-cfg/projects", name: `${id}.jsonl`, depth: 1 });
export const CODEX_SESSION_FILE = (id: string) => ({ folder: "/root/.codex/sessions", name: `-${id}.jsonl`, depth: 3 });

/** The file a session's pack finds on a machine: Claude Code's under the folder of the turn's cwd, /root/web, and
 * Codex's rollout under the day it started. */
export function sessionPathOf(folder: string, name: string): string {
  return folder.endsWith("/sessions") ? `${folder}/2026/10/12/rollout-2026-10-12T06-00-00${name}` : `${folder}/-root-web/${name}`;
}

/** A harness whose first turn, the lead's, runs until `endLead` ends it, and whose every other turn answers at once,
 * but for one started while `hold` is set, which runs until `release` ends it. */
function harness(starts: { o: HarnessStartOptions; env: Readonly<Record<string, string>> }[], lead: { end?: () => void; hold?: boolean; release?: () => void }, sessionFile: (id: string) => { folder: string; name: string; depth: number }): HarnessAdapterFactory {
  return hctx => ({
    steers: false,
    sessionFile,
    start: o => {
      starts.push({ o, env: hctx.env });
      const sessionId = `s${starts.length}`;
      o.onEvent({ type: "session.start", sessionId });
      const result: TurnResult = { status: "completed", text: "done" };
      if (starts.length === 1 || lead.hold === true) {
        const first = starts.length === 1;
        lead.hold = false;
        const finished = new Promise<TurnResult>(resolve => {
          lead[first ? "end" : "release"] = () => {
            o.onEvent({ type: "turn.done", sessionId, result });
            o.onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
            resolve(result);
          };
        });
        return { localId: sessionId, finished, interrupt: async () => {} };
      }
      o.onEvent({ type: "turn.done", sessionId, result });
      o.onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
      return { localId: sessionId, finished: Promise.resolve(result), interrupt: async () => {} };
    },
  });
}

/** The menu of what git ignores in the lead's folder, read off the files standing there, and a real packed archive. */
function seedWiring(sizes: Record<string, number>) {
  return {
    plan: async (folder: string): Promise<SeedPlan> => ({
      source: folder,
      remote: null,
      branch: "feat/upload",
      defaultBranch: "main",
      unpushed: null,
      uncommitted: 0,
      memory: null,
      remembered: false,
      files: Object.entries(sizes).map(([path, bytes]) => ({ path, dir: false, bytes, kind: "config" as const, ticked: true })),
    }),
    pack: async (o: { plan: SeedPlan; choice: { files: string[] } }) => {
      // Random bytes, so the archive is as big packed as the files are.
      const entries = o.choice.files.map(path => ({ path, mode: 0o600, content: randomBytes(sizes[path]!) }));
      return { tar: tarOf(entries), files: entries.length, bytes: o.choice.files.reduce((n, p) => n + sizes[p]!, 0), commits: 0, left: [] };
    },
  };
}

export interface Setup {
  remote?: string | null;
  git?: Partial<LeadGit>;
  picks?: string[];
  sizes?: Record<string, number>;
  image?: false | Parameters<typeof sealImage>[1];
  agents?: { spawn: boolean; maxMachines?: number };
  /** Boat's spend per day, which wires the place door that keeps it. */
  spendPerDayUsd?: number;
  /** Served on a socket, the lead's launch carrying the host's address and its own token, as a thread's wsp dials. */
  served?: true;
  /** Branches the remote holds already. */
  remoteHeads?: string[];
  /** The host's clock, for a test that moves time on. */
  clock?: Clock;
  /** The agent a turn on a Boat machine runs, where it is not the stub that answers at once. */
  subAdapter?: HarnessAdapterFactory;
}

/** A host holding the lead's folder of acme/web on this computer, its thread running, and Boat as its cloud. */
export async function leadOnThisComputer(o: Setup = {}) {
  root = mkdtempSync(join(tmpdir(), "wsp-sub-spawn-"));
  const repo = join(root, "web");
  mkdirSync(repo);
  execFileSync("git", ["init", "-q", "-b", "feat/upload", repo]);
  execFileSync("git", ["-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "first"]);
  if (o.remote !== null) execFileSync("git", ["-C", repo, "remote", "add", "origin", o.remote ?? "git@github.com:acme/web.git"]);
  const sizes = o.sizes ?? { ".env": 1100, ".env.local": 143 };
  for (const [path, bytes] of Object.entries(sizes)) writeFileSync(join(repo, path), "x".repeat(bytes));
  const store = memoryStore();
  if (o.image !== false) await sealImage(store, o.image);
  const boat = fakeBoat();
  const git: LeadGit = { head: "feat/upload", upstream: "origin/feat/upload", ahead: 0, uncommitted: 0, ...o.git };
  const daemon = leadDaemon(git);
  const starts: { o: HarnessStartOptions; env: Readonly<Record<string, string>> }[] = [];
  const leadTurn: { end?: () => void; hold?: boolean; release?: () => void } = {};
  const local = { ...fakeLocal(join(root, "home")), daemonRoad: async () => ({ url: "http://127.0.0.1:1", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "t" }) };
  const placeLinks = o.spendPerDayUsd === undefined ? {} : { placeLinks: wiring(newPlaceKeyPair(), { id: "box", rateUsdPerHour: 0.018 }) };
  const here: { url?: string } = {};
  const agents = o.served === true ? { agents: { here, wspMcp: { command: "wsp", args: ["mcp"] } satisfies McpServerSpec } } : {};
  const claude = harness(starts, leadTurn, CLAUDE_SESSION_FILE);
  const onBoat = o.subAdapter;
  const adapters = { claude: onBoat === undefined ? claude : (hctx: Parameters<HarnessAdapterFactory>[0]) => (boat.machines.some(m => m.id === hctx.machine.id) ? onBoat : claude)(hctx), codex: harness(starts, leadTurn, CODEX_SESSION_FILE) };
  const options = { backend: boat, places: wiredPlace("box", boat), store, adapters, ...(onBoat !== undefined ? { machineExec: { pollMs: 5 } } : {}), local, daemonChannel: daemon.open, seed: seedWiring(sizes), killConfirm: { graceMs: 20, pollMs: 1 }, goneConfirmMs: 0, ...placeLinks, ...agents, ...(o.clock !== undefined ? { clock: o.clock } : {}) };
  const rt: Runtime = createRuntime(options);
  if (o.served === true) {
    ctx.runtime = rt;
    ctx.srv = await serveRuntime(rt, { port: 0, authToken: "host-token", devices: rt.devices });
    here.url = `ws://127.0.0.1:${ctx.srv.port}`;
  }
  if (o.spendPerDayUsd !== undefined) await rt.places!.set("box", { spendPerDayUsd: o.spendPerDayUsd });
  const project = await rt.projects.add({ source: repo, on: HERE_PLACE_ID, name: "web" });
  // The remote, recorded on the project as GitHub's, answers from a bare repo here, so no read leaves this computer.
  if (o.remote === undefined) {
    const bare = join(root, "remote.git");
    execFileSync("git", ["init", "-q", "--bare", bare]);
    for (const head of o.remoteHeads ?? []) execFileSync("git", ["-C", repo, "push", "-q", bare, `HEAD:refs/heads/${head}`]);
    execFileSync("git", ["-C", repo, "config", `url.${bare}.insteadOf`, "git@github.com:acme/web.git"]);
  }
  if (o.picks !== undefined) await store.put("seed-choices", folderNamed(repo), { files: o.picks, memory: false, commits: false, remember: true });
  const folder = await rt.workspaces.create({ project: project.id, name: "web", agents: { spawn: true, ...(o.agents ?? {}) } });
  const turn = await rt.sessions.start(folder.id, { prompt: "coordinate", harness: "claude" });
  const threadId = turn.view().threadId!;
  const lead: Caller = { origin: "here", by: { kind: "thread", threadId, workspaceId: folder.id, rootThreadId: threadId } };
  // The same host started again on the same state, as a restart leaves it, on a clock of its own where one is given.
  const restart = (clock?: Clock): Runtime => createRuntime(clock === undefined ? options : { ...options, clock });
  return { rt, store, boat, git, daemon, starts, project, folder, threadId, lead, endLead: (): Promise<unknown> => (leadTurn.end?.(), turn.finished), token: starts[0]!.env[TURN_TOKEN_ENV]!, launch: { [HOST_TOKEN_ENV]: starts[0]!.env[HOST_TOKEN_ENV] ?? "", [TURN_TOKEN_ENV]: starts[0]!.env[TURN_TOKEN_ENV]! }, repo, restart, turns: leadTurn };
}

/** What the result read prints on a machine whose branch is pushed, three commits on the lead's. */
export const PUSHED = `branch wsp/fix-the-flaky-upload-test\npushed 9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e\nunpushed 0\nchanged 0\ncommits 3\n`;
/** Two commits nobody pushed and three files changed. */
export const UNPUSHED = `branch wsp/fix-the-flaky-upload-test\npushed none\nunpushed 2\nchanged 3\n`;

export interface Answers {
  read: string | { exitCode: number; stderr: string };
  /** The packed session, or only its packed size, where the host is not to read it. */
  session: Buffer | number | null;
}

/** Boat answering the result read, the session's pack and its read as `answers` says, on every machine it makes. */
export function answering(boat: Boat, answers: Answers): void {
  const base = boat.execImpl;
  boat.execImpl = (m, cmd, stdin) => {
    if (cmd.includes("git rev-list --count --branches --not --remotes")) {
      return typeof answers.read === "string" ? { exitCode: 0, stdout: answers.read, stderr: "" } : { exitCode: answers.read.exitCode, stdout: "", stderr: answers.read.stderr };
    }
    if (cmd.includes("gzip -c")) {
      if (answers.session === null) return { exitCode: 0, stdout: "nofile\n", stderr: "" };
      const [, folder, name] = /find '([^']+)' .* -name '\*([^']+)'/.exec(cmd)!;
      const size = typeof answers.session === "number" ? answers.session : answers.session.length;
      return { exitCode: 0, stdout: `path ${sessionPathOf(folder!, name!)}\nsize ${size}\n`, stderr: "" };
    }
    return base(m, cmd, stdin);
  };
  const create = boat.create.bind(boat);
  boat.create = async spec => {
    const m = (await create(spec)) as StubMachine;
    m.readBytes = async (path: string, o?: { size?: number }) => {
      const packed = answers.session;
      if (!Buffer.isBuffer(packed) || !path.startsWith("/tmp/wsp-session-") || o?.size !== packed.length) throw new Error(`nothing packed at ${path}`);
      return packed;
    };
    return m;
  };
}


/** Real timers, and a now the test can move on: a jump fires every timer it passes at once. */
export function jumpClock(): { clock: Clock; jump(ms: number): void } {
  let offset = 0;
  const pending = new Set<{ at: number; fn: () => void; timer: ReturnType<typeof setTimeout> }>();
  const clock: Clock = {
    now: () => Date.now() + offset,
    schedule: (fn, ms, o) => {
      const p = { at: Date.now() + offset + ms, fn, timer: setTimeout(() => (pending.delete(p), fn()), ms) };
      if (o?.unref === true) p.timer.unref();
      pending.add(p);
      return () => {
        clearTimeout(p.timer);
        pending.delete(p);
      };
    },
  };
  const jump = (ms: number): void => {
    offset += ms;
    for (const p of [...pending].sort((a, b) => a.at - b.at)) {
      if (p.at > Date.now() + offset) continue;
      clearTimeout(p.timer);
      pending.delete(p);
      p.fn();
    }
  };
  return { clock, jump };
}

/** Waits for a condition with a time limit, polling. */
export async function until<T>(read: () => Promise<T | undefined> | T | undefined, what: string, ms = 5_000): Promise<T> {
  const end = Date.now() + ms;
  for (;;) {
    const got = await read();
    if (got !== undefined && got !== false) return got;
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
    await new Promise(r => setTimeout(r, 10));
  }
}
