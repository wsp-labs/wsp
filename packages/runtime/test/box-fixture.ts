// SPDX-License-Identifier: AGPL-3.0-only
// A computer the person joined, as its daemon answers the host over the link:
// the login read, the login shell's PATH, the folder an add claims, a turn's
// launch, its polls and its signals, a pane's ptys, and every frame kept; and a
// lead thread on the computer the app runs on, beside one such computer.
import { execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdirSync } from "node:fs";
import { join as joinPath } from "node:path";
import { HERE_PLACE_ID, inCgroup, unknownOpLine, type Caller, type ProcEntry, type ThreadScope, type TurnResult } from "@wsp/protocol";
import type { HarnessAdapterFactory, HarnessStartOptions } from "../src/runtime.js";
import type { Store } from "../src/store.js";
import type { Clock } from "../src/clock.js";
import type { AgentsReader, ServersActs } from "../src/agents-read.js";
import { HANDSHAKE, MCP_READ_END } from "@wsp/engine";
import { ctx, sockets, serving, code, join, KEEPS_NO_IMAGE } from "./places-fixture.js";
import { branchDaemons, fakeLocal } from "./stub-backend.js";
import { report } from "./place-join.js";
import type { WsClient } from "./ws-client.js";

/** One frame's command and what rode its input, as the computer was sent it. */
export interface Exec {
  cmd: string;
  stdin: string;
}

export interface Box {
  ops: string[];
  execs: Exec[];
  /** Every frame that is no command, as it reached the computer. */
  frames: Record<string, unknown>[];
  /** Folders already standing in the home, which an add's claim finds taken. */
  taken: Set<string>;
  /** The process group signals sent, in order. */
  kills: string[];
  /** An event the computer's daemon pushes up the link. */
  push(event: Record<string, unknown>): void;
  /** The files the agents' configs read finds there, by path. */
  configs: Map<string, string>;
  /** How the configs read answers, given what it printed: as printed and exiting 0 unless a test says otherwise. */
  readAs?: (stdout: string) => { stdout: string; exitCode: number; stderr?: string };
  /** The computer answers no frame, a ping included, as one whose network was cut while its socket still stands. */
  quiet?: boolean;
  /** A turn's launch and a thread's end, in the order they reached the computer and it answered them. */
  order: string[];
  /** How long the computer takes to answer a thread's end, given its command; at once unless a test says otherwise. */
  endMs?: (cmd: string) => number;
  /** A thread's end there leaves pid 4242 standing, as a process stuck in the kernel does. */
  endFails?: boolean;
  /** The branch the folders there have checked out; none answers as a detached head does. */
  head?: string;
  /** The processes its daemon lists and the ports they listen on; a thread's end takes those in its cgroup. */
  procs: ProcEntry[];
  ports: { port: number; pid: number }[];
  /** Its daemon predates proc.list and refuses it as an op it does not know. */
  listless?: boolean;
  /** Each thread cgroup's memory as the daemon reads it, beside the rows proc.list answers. */
  cgroups?: { cgroup: string; memory: number }[];
  /** The branches the folders' remote holds, as git ls-remote --heads lists them there. */
  remoteHeads?: string[];
  /** With `agent`, the runs launched there by their path, each Claude Code on stream-json answering every message its
   * input takes from one process, as `<message> from run <n> turn <t>`. */
  runs: Map<string, AgentRun>;
  /** With `agent`, the size a read of an agent's session file there answers; none answers nothing. A title written into
   * that file grows it by 200 bytes and answers after `titleMs`. */
  sessionBytes?: number;
  titleMs?: number;
}

/** One agent process a launch started there: the launch it was, how many messages it answered, what it printed, and
 * the exit its run left once its input closed or a signal reached its group. */
export interface AgentRun {
  n: number;
  turns: number;
  log: Buffer;
  exit?: string;
}

/** How the computer answers stdio.open, stdio.write and stdio.close, for a case that runs their commands: the reply,
 * with each event pushed up the link as the daemon there pushes them. */
export type Stdio = (frame: Record<string, unknown>, push: (event: Record<string, unknown>) => void) => Record<string, unknown>;

/** How the computer answers http.post, for a case that posts from there: the reply. */
export type Posts = (frame: Record<string, unknown>) => Promise<Record<string, unknown>>;

export interface BoxLogin {
  home: string;
  owner: string;
  /** What the login's own login shell prints for its PATH; nothing prints none. */
  shellPath?: string;
}

/** A line handed to the owner of the home rides inside one quoted argument; read it back out. */
export const handedLine = (cmd: string): string => {
  const handed = /^runuser -u '[^']+' -- bash -c '(.*)'$/s.exec(cmd);
  return handed === null ? cmd : handed[1]!.replaceAll("'\\''", "'");
};

/** The frames a pane sends its shells by, which a box whose `ptys` is given hands to that real daemon. */
const PTY_OPS = new Set(["pty.create", "pty.attach", "pty.write", "pty.resize", "pty.kill", "pty.detach", "pty.list", "pty.tab"]);

/** The files an upload lands, each by its path, decoded, and whether it is appended. */
const landed = (cmd: string): { path: string; text: string; append: boolean }[] =>
  [...cmd.matchAll(/printf %s '([A-Za-z0-9+/=]*)' \| base64 -d (>>?) '([^']+)'/g)].map(([, b64, how, path]) => ({ path: path!, text: Buffer.from(b64!, "base64").toString("utf8"), append: how === ">>" }));

/** Claude Code on stream-json answering one line of its input, onto its run's log. */
function answer(run: AgentRun, at: { session: string; cwd: string }, line: string): void {
  const m = JSON.parse(line) as { type?: string; message?: { content?: { text?: string }[] } };
  if (m.type !== "user") return;
  run.turns += 1;
  const text = `${m.message?.content?.at(-1)?.text ?? ""} from run ${run.n} turn ${run.turns}`;
  const say = (o: Record<string, unknown>): void => void (run.log = Buffer.concat([run.log, Buffer.from(`${JSON.stringify(o)}\n`)]));
  say({ type: "system", subtype: "init", session_id: at.session, cwd: at.cwd, model: "claude-sonnet-4-5", tools: [], permissionMode: "default", slash_commands: [], apiKeySource: "none" });
  say({ type: "result", subtype: "success", is_error: false, duration_ms: 5, num_turns: 1, result: text, session_id: at.session, total_cost_usd: 0, usage: { input_tokens: 1, output_tokens: 1 } });
}

/** What a computer running agent processes answers about them: a launch, a message written to one, a poll of its log,
 * its input closed, a signal to its group, the reap of its files and a read of its session file. Nothing for any
 * other command. */
function agentAnswers(seen: Box, sessions: Map<string, { session: string; cwd: string }>, cmd: string, login: BoxLogin): string | undefined {
  const files = landed(cmd);
  if (cmd.includes("WSP_LAUNCHED")) {
    const script = files.find(f => f.path.endsWith(".sh"));
    if (script === undefined) return undefined;
    const base = script.path.slice(0, -".sh".length);
    const run: AgentRun = { n: seen.runs.size + 1, turns: 0, log: Buffer.alloc(0) };
    seen.runs.set(base, run);
    sessions.set(base, { session: /--(?:session-id|resume) '?([0-9a-f-]{36})/.exec(script.text)?.[1] ?? "", cwd: /cd '([^']+)' && /.exec(script.text)?.[1] ?? login.home });
    for (const line of (files.find(f => f.path === `${base}.in`)?.text ?? "").split("\n").filter(Boolean)) answer(run, sessions.get(base)!, line);
    return "WSP_LAUNCHED\n";
  }
  const write = files.find(f => f.append && f.path.endsWith(".in"));
  if (write !== undefined) {
    const base = write.path.slice(0, -".in".length);
    const run = seen.runs.get(base);
    if (run === undefined || run.exit !== undefined) return `${HANDSHAKE.gone}\n`;
    for (const line of write.text.split("\n").filter(Boolean)) answer(run, sessions.get(base)!, line);
    return `${HANDSHAKE.written}\n`;
  }
  const sentinel = /(__WSP_EOF_[0-9a-f]+__)/.exec(cmd)?.[1];
  const polled = /tail -c \+(\d+) '([^']+)'\.log/.exec(cmd);
  if (sentinel !== undefined && polled !== null) {
    const run = seen.runs.get(polled[2]!);
    if (run === undefined) return `\n${sentinel}  down \n`;
    return `${run.log.subarray(Number(polled[1]) - 1).toString("base64")}\n${sentinel} ${run.exit ?? ""} ${run.exit === undefined ? "up" : "down"} \n`;
  }
  const base = /cat '([^']+)'\.(?:pid|tail)/.exec(cmd)?.[1];
  const run = base === undefined ? undefined : seen.runs.get(base);
  if (base !== undefined && cmd.includes("read -r s")) return "";
  if (run !== undefined && /\.tail 2>\/dev\/null\); \[ -n "\$P" \] && kill -TERM "\$P"/.test(cmd)) {
    run.exit ??= "0";
    return "";
  }
  if (run !== undefined && (cmd.includes("kill -TERM -- -$P") || cmd.includes("kill -KILL -- -$P"))) {
    seen.kills.push(cmd);
    run.exit ??= "143";
    if (cmd.includes(`rm -rf '${base}'.*`)) seen.runs.delete(base!);
    return "";
  }
  if (base === undefined && /rm -rf '([^']+)'\.\*/.test(cmd)) return "";
  if (cmd.includes("wc -c <")) return seen.sessionBytes === undefined ? "" : `${seen.sessionBytes}\n`;
  return undefined;
}

export function box(client: WsClient, login: BoxLogin, o: { failClone?: boolean; logins?: string; ptys?: Ptys; agent?: boolean; runs?: Map<string, AgentRun>; stdio?: Stdio; posts?: Posts } = {}): Box {
  let ptys = 0;
  const seen: Box = { ops: [], execs: [], frames: [], taken: new Set(), kills: [], push: event => client.say(event), configs: new Map(), order: [], procs: [], ports: [], runs: o.runs ?? new Map() };
  const sessions = new Map<string, { session: string; cwd: string }>();
  /** The runs a signal reached, by the run's own path off the command. */
  const stopped = new Set<string>();
  const runOf = (cmd: string): string | undefined => /cat (\S+)\.pid/.exec(cmd)?.[1];
  client.onFrame(raw => {
    const frame = raw as unknown as Record<string, unknown>;
    const op = typeof frame["op"] === "string" ? frame["op"] : undefined;
    if (op === undefined || seen.quiet === true) return;
    const say = (payload: Record<string, unknown>): void => client.say({ id: frame["id"], ok: true, ...payload });
    seen.ops.push(op);
    if (op === "machine.backend") return say(o.logins === undefined ? KEEPS_NO_IMAGE : { ...KEEPS_NO_IMAGE, logins: o.logins });
    if (op === "machine.capacity") return say({ cores: 4, memMb: 8192, memRoomMb: 4096, machineMemMb: 4096, diskFreeBytes: 10 * 1024 ** 3, images: [], machines: { running: 0, paused: 0 } });
    if (op !== "exec") {
      seen.frames.push(frame);
      if (o.ptys !== undefined && PTY_OPS.has(op)) return void o.ptys(frame).then(reply => client.say({ ...reply, id: frame["id"] }), (e: unknown) => client.say({ id: frame["id"], ok: false, error: String(e) }));
      if (o.stdio !== undefined && op.startsWith("stdio.")) return say(o.stdio(frame, seen.push));
      if (o.posts !== undefined && op === "http.post") return void o.posts(frame).then(say, (e: unknown) => client.say({ id: frame["id"], ok: false, error: String(e) }));
      if (op === "pty.create") return say({ ptyId: `p${++ptys}` });
      if (op === "proc.list" && seen.listless === true) return client.say({ id: frame["id"], ok: false, error: unknownOpLine("proc.list") });
      if (op === "proc.list") return say({ at: Date.now(), daemon: 1, total: seen.procs.length, procs: seen.procs, ...(seen.cgroups === undefined ? {} : { cgroups: seen.cgroups }) });
      if (op === "ports.watch") return say({ ports: seen.ports });
      if (op === "git.status") return say({ branch: { oid: "abc", head: "main", upstream: "origin/main", ahead: 0, behind: 0 }, entries: [], root: String(frame["cwd"]) });
      return say({});
    }
    const cmd = String(frame["cmd"]);
    const stdin = frame["stdin"] === undefined ? "" : Buffer.from(String(frame["stdin"]), "base64").toString("utf8");
    seen.execs.push({ cmd, stdin });
    const out = (stdout: string, exitCode = 0): void => say({ exitCode, stdout, stderr: exitCode === 0 ? "" : "fatal: repository not found", truncated: false });
    if (cmd.includes("command -v runuser")) return out(`Linux\n0\nroot\n${login.owner}\n1\n${login.home}\n/usr/bin\n`);
    const line = handedLine(cmd);
    if (line.includes("-ilc")) return login.shellPath === undefined ? out("", 1) : out(`welcome back\n${login.shellPath}`);
    const claim = /mkdir '([^']+)'"\$n"/.exec(line);
    if (claim !== null) {
      const at = [claim[1]!, `${claim[1]!}-2`, `${claim[1]!}-3`].find(path => !seen.taken.has(path))!;
      seen.taken.add(at);
      return out(`${at}\n`);
    }
    if (o.failClone === true && line.includes("git clone")) return out("", 128);
    if (o.agent === true && line.includes("custom-title") && line.includes(`>> "$f"`)) {
      seen.sessionBytes = (seen.sessionBytes ?? 0) + 200;
      return void setTimeout(() => out("wrote\n"), seen.titleMs ?? 0);
    }
    if (o.agent === true) {
      const said = agentAnswers(seen, sessions, line, login);
      if (said !== undefined) {
        if (cmd.includes("WSP_LAUNCHED")) seen.order.push("launch");
        return out(said);
      }
    }
    if (line.includes("ls-remote") && line.includes("--heads")) return out((seen.remoteHeads ?? []).map(h => `${"0".repeat(40)}\trefs/heads/${h}\n`).join(""));
    if (line.includes("symbolic-ref --quiet --short HEAD")) return seen.head === undefined ? out("", 1) : out(`${seen.head}\n`);
    if (line.includes("wsp_mcp_read()")) {
      const said = [...line.matchAll(/^wsp_mcp_read (\d+) '([^']+)'$/gm)].map(([, i, path]) => {
        const text = seen.configs.get(path!);
        return text === undefined ? `wsp-mcp ${i} - -` : `wsp-mcp ${i} 0 ${Buffer.from(text).toString("base64")}`;
      });
      const printed = `${[...said, MCP_READ_END].join("\n")}\n`;
      const read = seen.readAs?.(printed) ?? { stdout: printed, exitCode: 0 };
      return say({ exitCode: read.exitCode, stdout: read.stdout, stderr: read.stderr ?? "", truncated: false });
    }
    // A detached run's poll: it ended at once with nothing said, as a read over a home with nothing of wsp's in it does.
    if (cmd.includes("echo WSP_POLL")) return out(["WSP_POLL", "0", "", "", "down", "WSP_POLL_END"].join("\n"));
    if (cmd.includes("WSP_LAUNCHED")) {
      seen.order.push("launch");
      return out("WSP_LAUNCHED\n");
    }
    if (cmd.includes("wsp_end()")) {
      seen.order.push("end starts");
      setTimeout(() => {
        seen.order.push("end answers");
        const cgroup = /wsp_end '([^']+)'/.exec(cmd)?.[1] ?? "";
        if (seen.endFails === true) say({ exitCode: 1, stdout: "", stderr: `processes 4242 of ${cgroup} did not end\n`, truncated: false });
        else {
          seen.procs = seen.procs.filter(p => !inCgroup(p.cgroup, cgroup));
          out("");
        }
      }, seen.endMs?.(cmd) ?? 0);
      return;
    }
    const sentinel = /(__WSP_EOF_[0-9a-f]+__)/.exec(cmd)?.[1];
    if (sentinel !== undefined) return out(stopped.has(runOf(cmd) ?? "") ? `\n${sentinel} 143 down \n` : `\n${sentinel}  up \n`);
    if (cmd.includes("kill -TERM -- -$P") || cmd.includes("kill -KILL -- -$P")) {
      seen.kills.push(cmd);
      stopped.add(runOf(cmd) ?? "");
    }
    return out("");
  });
  return seen;
}

export type Started = { o: HarnessStartOptions; env: Readonly<Record<string, string>>; vault?: Readonly<Record<string, string>> };

/** A harness that records what each turn was started with and answers at once. */
export function answering(starts: Started[]): HarnessAdapterFactory {
  return hctx => ({
    steers: false,
    start: o => {
      starts.push({ o, env: hctx.env, vault: hctx.vault });
      const sessionId = randomUUID();
      const result: TurnResult = { status: "completed", text: "ok" };
      o.onEvent({ type: "session.start", sessionId });
      o.onEvent({ type: "turn.done", sessionId, result });
      o.onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
      return { localId: sessionId, finished: Promise.resolve(result), interrupt: async () => {} };
    },
  });
}

/** A real daemon answering a box's pty frames, its own events pushed up the link by the test. */
export type Ptys = (frame: Record<string, unknown>) => Promise<Record<string, unknown>>;

export const HETZNER: BoxLogin = { home: "/root", owner: "root" };

/** A host holding one joined computer, hetzner, its login root unless named, and a project added there by url. */
/** GitHub set up on that computer as the setup records it: from the vault, by its own login there, or skipped. */
export async function pickGitHub(store: Store, placeId: string, signin: "vault" | "machine" | "skip"): Promise<void> {
  const record = (await store.get("places", placeId)) as Record<string, unknown>;
  const outcome = signin === "skip" ? "skipped" : "present";
  await store.put("places", placeId, { ...record, picks: { configs: { github: { signin } } }, applied: { at: new Date().toISOString(), rows: [{ id: "github", label: "GitHub", outcome }] } });
  await ctx.runtime!.places!.load();
}

/** An agent's sign-in on that computer as the setup records the way it was picked. */
export async function pickSignIn(store: Store, placeId: string, agent: string, signin: "vault" | "machine" | "token" | "key"): Promise<void> {
  const record = (await store.get("places", placeId)) as Record<string, unknown>;
  await store.put("places", placeId, { ...record, picks: { agents: { [agent]: { signin } }, configs: {} } });
  await ctx.runtime!.places!.load();
}

export async function joined(o: { login?: BoxLogin; adapters?: Record<string, HarnessAdapterFactory>; taken?: string[]; store?: Store; vault?: Record<string, string>; failClone?: boolean; serversActs?: ServersActs; agentsReader?: AgentsReader; daemonVersion?: number; stdio?: Stdio; posts?: Posts; logins?: string; clock?: Clock; ptys?: Ptys; github?: "vault" | "machine" | "skip"; agent?: boolean; memMb?: number } = {}) {
  const login = o.login ?? HETZNER;
  const { hostKey, store } = await serving({ adapters: o.adapters ?? {}, ...(o.store !== undefined ? { store: o.store } : {}), ...(o.vault !== undefined ? { vault: o.vault } : {}), ...(o.serversActs !== undefined ? { serversActs: o.serversActs } : {}), ...(o.agentsReader !== undefined ? { agentsReader: o.agentsReader } : {}), ...(o.clock !== undefined ? { clock: o.clock } : {}) });
  let seen!: Box;
  const { client, placeId, pair } = await join(hostKey, {
    code: await code(),
    report: report("hetzner", { login: { HOME: login.home, USER: "root", PATH: "/usr/bin" }, ...(o.memMb !== undefined ? { shape: { cpu: 2, memMb: o.memMb } } : {}), ...(o.daemonVersion !== undefined ? { daemonVersion: o.daemonVersion } : {}) }),
    answers: c => {
      seen = box(c, login, { ...(o.failClone === true ? { failClone: true } : {}), ...(o.logins !== undefined ? { logins: o.logins } : {}), ...(o.ptys !== undefined ? { ptys: o.ptys } : {}), ...(o.agent === true ? { agent: true } : {}), ...(o.stdio !== undefined ? { stdio: o.stdio } : {}), ...(o.posts !== undefined ? { posts: o.posts } : {}) });
      for (const path of o.taken ?? []) seen.taken.add(path);
    },
  });
  sockets.push(client.ws);
  const rt = ctx.runtime!;
  if (o.github !== undefined) await pickGitHub(store, placeId, o.github);
  const project = o.failClone === true ? undefined : await rt.projects.add({ source: "https://github.com/spoo-me/spoo-ts", on: "hetzner", name: "spoo-ts" });
  return { rt, placeId, project: project!, seen, pair, hostKey };
}

export const asThread = (scope: ThreadScope): Caller => ({ origin: "relayed", by: scope });

/** What a message starts with to hold its turn open until the case answers it. */
export const HOLD = "hold: ";
/** What a message starts with for a turn that fails at once. */
export const FAIL = "fail: ";

export interface Held {
  o: HarnessStartOptions;
  env: Readonly<Record<string, string>>;
  answer(text: string): void;
}

/** A harness whose lead turn runs until the case answers it, as a coordinator's does while its children work, and
 * whose every other turn answers at once, but one the case marks to hold the same way, as a builder's turn runs. */
export function leading(starts: Held[]): HarnessAdapterFactory {
  return hctx => ({
    steers: false,
    start: o => {
      const sessionId = randomUUID();
      let answer = (_text: string): void => {};
      const finished = new Promise<TurnResult>(resolve => {
        answer = text => {
          const result: TurnResult = o.prompt.startsWith(FAIL) ? { status: "failed", error: text } : { status: "completed", text };
          o.onEvent({ type: "turn.done", sessionId, result });
          o.onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
          resolve(result);
        };
      });
      starts.push({ o, env: hctx.env, answer });
      o.onEvent({ type: "session.start", sessionId });
      if (o.prompt !== "coordinate" && !o.prompt.startsWith(HOLD)) answer(o.prompt.startsWith(FAIL) ? "the build broke" : "built it");
      return { localId: sessionId, finished, interrupt: async () => answer("stopped") };
    },
  });
}

/** A host holding this computer's folder of acme/lab in `root`, with a lead thread running on it, hetzner joined as
 * root, the same repository added there and another repository there. With `reach`, the lead's launch carries the
 * host's address and its own token, as a launch on this computer does under the host. With `forks`, a daemon answers
 * the lead's folder on main, so the lead forks a cloud machine of its repository the way it does on this computer. */
export async function leadAndBox(root: string, o: { agents?: { spawn: boolean; maxDepth?: number }; reach?: true; forks?: true } = {}) {
  const repo = joinPath(root, "lab");
  mkdirSync(repo);
  execFileSync("git", ["init", "-q", "-b", "main", repo]);
  execFileSync("git", ["-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "first"]);
  execFileSync("git", ["-C", repo, "remote", "add", "origin", "git@github.com:acme/lab.git"]);
  const starts: Held[] = [];
  const here: { url?: string } = {};
  const local = fakeLocal(joinPath(root, "home"));
  const daemon = o.forks === true ? { local: { ...local, daemonRoad: async () => ({ url: "http://127.0.0.1:1", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "t" }) }, daemonChannel: branchDaemons({ branchOf: () => "main" }).open } : { local };
  const { hostKey, store } = await serving({ adapters: { claude: leading(starts) }, ...daemon, ...(o.reach === true ? { agents: { here, wspMcp: { command: "wsp", args: ["mcp"] } } } : {}) });
  here.url = `ws://127.0.0.1:${ctx.srv!.port}`;
  let seen!: Box;
  const { client, placeId } = await join(hostKey, {
    code: await code(),
    report: report("hetzner", { login: { HOME: HETZNER.home, USER: "root", PATH: "/usr/bin" } }),
    answers: c => void (seen = box(c, HETZNER)),
  });
  sockets.push(client.ws);
  const rt = ctx.runtime!;
  const mac = await rt.projects.add({ source: repo, on: HERE_PLACE_ID, name: "lab" });
  const onBox = await rt.projects.add({ source: "https://github.com/acme/lab", on: "hetzner", name: "lab-box" });
  await rt.projects.add({ source: "https://github.com/acme/else", on: "hetzner", name: "else-box" });
  const folder = await rt.workspaces.create({ project: mac.id, name: "lab", agents: o.agents ?? { spawn: true } });
  const turn = await rt.sessions.start(folder.id, { prompt: "coordinate", harness: "claude" });
  const threadId = turn.view().threadId!;
  const lead: Caller = { origin: "here", by: { kind: "thread", threadId, workspaceId: folder.id, rootThreadId: threadId } };
  return { rt, seen, starts, mac, onBox, folder, threadId, lead, launch: starts[0]!.env, turn, hostKey, store, placeId, repo };
}
