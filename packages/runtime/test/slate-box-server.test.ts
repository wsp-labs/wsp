// SPDX-License-Identifier: AGPL-3.0-only
// A slate's tool run from a thread in a folder on a computer the person joined: its server is read off that
// computer's agent config and started there through the daemon's stdio.open, as the login there, its variables on
// its own stdin, and kept running between calls; this computer's config and sign-ins are never read for it. A server
// that computer's config does not name, and a command run on="host", are refused as the slate is written.
import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it, vi } from "vitest";
import { slateServerNotSetUp } from "@wsp/protocol/slate";
import { slateServerBehindLine, STDIO_DAEMON_VERSION, THREAD_ENV, threadCgroup, type ProcEntry, type SlateView } from "@wsp/protocol";
import { SERVER_NOT_SET_UP, type AgentsOn, type AgentsReader } from "../src/agents-read.js";
import { boxSlateDir } from "../src/slate-mcp-on.js";
import { answering, asThread, joined, type Started, type Stdio } from "./box-fixture.js";

const STUB = fileURLToPath(new URL("./fixtures/stub-mcp.mjs", import.meta.url));
const TOKEN = "tok_TESTONLY_9f2c";
const roots: string[] = [];
afterEach(() => {
  for (const at of roots.splice(0)) rmSync(at, { recursive: true, force: true });
});

/** The daemon there as stdio.open runs a command: bash -c in a process group of its own, with the frame's env over
 * the daemon's own, stdout and stderr pushed up the link apart, base64, and its exit; stdio.close kills the group.
 * `marked` says, for each command, whether the thread's mark stood in its environment as it was spawned, before any
 * write reached it; `closing` hears each close. */
function commands(): { stdio: Stdio; running: Map<string, ChildProcess>; marked: Map<string, boolean>; closing: { hear: () => void } } {
  const running = new Map<string, ChildProcess>();
  const marked = new Map<string, boolean>();
  const closing = { hear: (): void => {} };
  const stdio: Stdio = (frame, push) => {
    const stdioId = String(frame["stdioId"]);
    const child = running.get(stdioId);
    if (frame["op"] === "stdio.write") child?.stdin?.write(Buffer.from(String(frame["data"]), "base64"));
    if (frame["op"] === "stdio.close") closing.hear();
    if (frame["op"] === "stdio.close" && child?.pid !== undefined) process.kill(-child.pid, "SIGKILL");
    if (frame["op"] !== "stdio.open") return {};
    const env = typeof frame["env"] === "object" && frame["env"] !== null ? (frame["env"] as Record<string, string>) : {};
    const started = spawn("bash", ["-c", String(frame["cmd"])], { detached: true, stdio: ["pipe", "pipe", "pipe"], env: { ...process.env, ...env } });
    // What it was spawned with, read here: a Mac shows no environment of a platform binary such as /bin/bash, and the
    // daemon's own test reads the mark off the spawned bash itself.
    marked.set(stdioId, env[THREAD_ENV] !== undefined);
    running.set(stdioId, started);
    started.stdin.on("error", () => {});
    started.stdout.on("data", (b: Buffer) => push({ type: "stdio.out", stdioId, data: b.toString("base64") }));
    started.stderr.on("data", (b: Buffer) => push({ type: "stdio.err", stdioId, data: b.toString("base64") }));
    started.on("exit", code => {
      running.delete(stdioId);
      push({ type: "stdio.exit", stdioId, ...(code !== null ? { code } : {}) });
    });
    return {};
  };
  return { stdio, running, marked, closing };
}

/** A thread on hetzner, and an agents reader that answers that computer's config with the stub server alone. */
async function boxThread(o: { daemonVersion?: number } = {}) {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "wsp-slate-box-")));
  roots.push(root);
  const starts = join(root, "starts");
  const asked: AgentsOn[] = [];
  const agentsReader: AgentsReader = {
    read: () => Promise.reject(new Error("not read here")),
    tools: () => Promise.reject(new Error("not asked here")),
    server: async (on, ask) => {
      asked.push(on);
      if (on.kind === "here") throw new Error("this computer's config was read for a thread on hetzner");
      if (ask.name !== "stub") throw Object.assign(new Error(`no MCP server called ${ask.name} is in Claude Code's config there`), { code: SERVER_NOT_SET_UP });
      return {
        transport: { kind: "stdio", command: process.execPath, args: [STUB], env: { STUB_STARTS: starts } },
        cwd: root,
        env: { STUB_KEY: TOKEN },
        secrets: [TOKEN],
        login: { platform: "linux", home: root, user: "root" },
      };
    },
  };
  const started: Started[] = [];
  const { stdio, running, marked, closing } = commands();
  const { rt, project, seen } = await joined({ adapters: { claude: answering(started) }, agentsReader, stdio, daemonVersion: o.daemonVersion ?? STDIO_DAEMON_VERSION });
  const at = await rt.workspaces.folderFor({ project: project.id });
  const first = await rt.sessions.start(at.workspace.id, { prompt: "show my zones", harness: "claude", permissionMode: "default" });
  await first.finished;
  const threadId = first.view().threadId!;
  const caller = asThread({ kind: "thread", threadId, workspaceId: at.workspace.id, rootThreadId: threadId });
  return { rt, seen, running, marked, closing, asked, starts, threadId, caller };
}

/** Whether a running process's environment holds a variable as its computer's daemon reads it for the thread's mark:
 * /proc on Linux, ps on a Mac. Only the answer leaves here, never the environment, which carries the test's own. */
function holds(pid: number, has: (entry: string) => boolean): boolean {
  let entries: string[];
  try {
    entries = readFileSync(`/proc/${pid}/environ`, "utf8").split("\0");
  } catch {
    entries = execFileSync("ps", ["-E", "-ww", "-o", "command=", "-p", String(pid)], { encoding: "utf8" }).trim().split(" ");
  }
  return entries.some(has);
}

describe("a box slate's runs and the server it keeps", () => {
  it("carry the thread's mark, so the box's daemon names the kept server the thread's, and Activity draws both under the thread", async () => {
    const { rt, seen, running, marked: atSpawn, threadId, caller } = await boxThread();
    await rt.slates.write({ text: `<slate title="Items"><run name="items" tool="stub.list_items" args={{ params: { account: "admin", limit: 2 } }} description="Lists the newest items" /><column><button id="go" label="Go" onPress={start($items)} /></column></slate>` }, caller);
    await rt.slates.settled();
    const view = async (): Promise<SlateView> => (await rt.slates.get(threadId))!;
    const ask = await rt.slates.event({ threadId, version: (await view()).version, piece: "go", event: "press", requestId: "go-0" });
    await rt.slates.approve({ threadId, key: ask.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["items"]).toMatchObject({ state: "done" }), { timeout: 15_000 });

    // The server the daemon there runs, read as that daemon reads it: the mark the host put last on its variables.
    const server = [...running.values()][0]!;
    expect(holds(server.pid!, e => e.startsWith("STUB_KEY="))).toBe(true);
    const marked = holds(server.pid!, e => e === `${THREAD_ENV}=${threadId}`);
    expect(marked).toBe(true);
    expect(holds(server.pid!, e => e.startsWith("SLATE_DIR=") && e.endsWith(`/${threadId}`))).toBe(true);
    const first = Buffer.from(String(seen.frames.find(f => f["op"] === "stdio.write")!["data"]), "base64").toString("utf8");
    expect(first.indexOf(`${THREAD_ENV}\0`)).toBeGreaterThan(first.indexOf("STUB_KEY\0"));
    // And from the server line's own start, before its pairs arrive: a sample then would otherwise read no mark and
    // keep that read for as long as the box's sampler runs.
    const opened = seen.frames.find(f => f["op"] === "stdio.open")!;
    expect(opened["env"]).toEqual({ [THREAD_ENV]: threadId, SLATE_DIR: boxSlateDir(threadId) });
    expect(atSpawn.get(String(opened["stdioId"]))).toBe(true);

    // The daemon's rung for that mark (proc_owner.rs): the thread, kind slate beside SLATE_DIR. A run stands in the
    // thread's cgroup, where the slate's launcher starts it.
    const row = (pid: number, ppid: number, comm: string, over: Partial<ProcEntry>): ProcEntry => ({ pid, ppid, user: "root", state: "S", comm, cmdline: comm, cpu: 0, rss: 10_000_000, startedAt: 1_760_000_000_000 + pid, ...over });
    seen.procs = [
      row(1, 0, "wsp-daemon", {}),
      row(7300, 1, "python3", { cmdline: `python3 ${boxSlateDir(threadId)}/feed.py`, cgroup: threadCgroup(threadId) }),
      row(server.pid!, 1, "node", { cmdline: `node ${STUB}`, ...(marked ? { owner: { by: "env" as const, thread: threadId }, kind: "slate" as const } : {}) }),
    ];
    const read = await rt.activity.read({ thread: threadId });
    expect(read.threads.find(t => t.threadId === threadId)?.processes.map(p => [p.pid, p.kind])).toEqual(expect.arrayContaining([[7300, "slate"], [server.pid!, "slate"]]));
    expect(read.unowned.map(p => p.pid)).not.toContain(server.pid);
    await rt.slates.forget(threadId);
    await vi.waitFor(() => expect(running.size).toBe(0), { timeout: 5_000 });
    await rt.close();
  }, 40_000);
});

describe("a settle or a delete of a box thread whose slate keeps a server there", () => {
  /** The thread's slate with its server started there, a run of it in the thread's cgroup, and the server outside it,
   * stamped as the box's daemon stamps it off its mark; a close takes the server's row. */
  async function keeping() {
    const b = await boxThread();
    await b.rt.slates.write({ text: `<slate title="Items"><run name="items" tool="stub.list_items" args={{ params: { account: "admin", limit: 2 } }} description="Lists the newest items" /><column><button id="go" label="Go" onPress={start($items)} /></column></slate>` }, b.caller);
    await b.rt.slates.settled();
    const view = async (): Promise<SlateView> => (await b.rt.slates.get(b.threadId))!;
    const ask = await b.rt.slates.event({ threadId: b.threadId, version: (await view()).version, piece: "go", event: "press", requestId: "go-0" });
    await b.rt.slates.approve({ threadId: b.threadId, key: ask.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["items"]).toMatchObject({ state: "done" }), { timeout: 15_000 });
    const server = [...b.running.values()][0]!.pid!;
    const row = (pid: number, comm: string, over: Partial<ProcEntry>): ProcEntry => ({ pid, ppid: 1, user: "root", state: "S", comm, cmdline: comm, cpu: 0, rss: 10_000_000, startedAt: 1_760_000_000_000 + pid, ...over });
    b.seen.procs = [
      row(7300, "python3", { cmdline: `python3 ${boxSlateDir(b.threadId)}/feed.py`, cgroup: threadCgroup(b.threadId), owner: { by: "env", thread: b.threadId }, kind: "slate" }),
      row(server, "node", { cmdline: `node ${STUB}`, owner: { by: "env", thread: b.threadId }, kind: "slate" }),
    ];
    b.closing.hear = () => void (b.seen.procs = b.seen.procs.filter(p => p.pid !== server));
    const closes = (): number => b.seen.frames.filter(f => f["op"] === "stdio.close").length;
    return { ...b, server, closes };
  }

  it("settle asks about the kept server, and stop closes it with the cgroup's end, leaving nothing standing", async () => {
    const k = await keeping();
    const refused = await k.rt.sessions.settle([k.threadId], undefined, { processes: "ask" }).then(() => "", (e: unknown) => (e as Error).message);
    expect(refused).toContain(`${k.server} node (slate) of ${k.threadId.slice(0, 8)} on hetzner`);
    const answer = await k.rt.sessions.settle([k.threadId], undefined, { processes: "stop" });
    expect(answer.ended).toEqual([expect.objectContaining({ left: [] })]);
    expect(k.closes()).toBe(1);
    await vi.waitFor(() => expect(k.running.size).toBe(0), { timeout: 5_000 });
    await k.rt.close();
  }, 40_000);

  it("delete with keep leaves the kept server running, and with stop closes it and names nothing still standing", async () => {
    const k = await keeping();
    await k.rt.sessions.delete(k.threadId, undefined, { processes: "keep" });
    await new Promise(resolve => setTimeout(resolve, 300));
    expect([k.closes(), k.running.size]).toEqual([0, 1]);
    for (const child of k.running.values()) process.kill(-child.pid!, "SIGKILL");
    await vi.waitFor(() => expect(k.running.size).toBe(0), { timeout: 5_000 });
    await k.rt.close();

    const s = await keeping();
    const answer = await s.rt.sessions.delete(s.threadId, undefined, { processes: "stop" });
    expect(answer.ended).toEqual([expect.objectContaining({ left: [] })]);
    expect(s.closes()).toBe(1);
    await vi.waitFor(() => expect(s.running.size).toBe(0), { timeout: 5_000 });
    await s.rt.close();
  }, 60_000);
});

describe("a slate's tool run from a thread on a computer you joined", () => {
  it("starts its server on that computer through the daemon with that computer's config, answers the tool, and keeps the one server between calls", async () => {
    const { rt, seen, running, asked, starts, threadId, caller } = await boxThread();
    await rt.slates.write({ text: `<slate title="Items"><run name="items" tool="stub.list_items" args={{ params: { account: "admin", limit: 2 } }} description="Lists the newest items" /><column><button id="go" label="Go" onPress={start($items)} /></column></slate>` }, caller);
    const view = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    const press = async (n: number) => rt.slates.event({ threadId, version: (await view()).version, piece: "go", event: "press", requestId: `go-${n}` });
    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["items"]).toMatchObject({ state: "done", runs: 1, json: { account: "admin" } }), { timeout: 15_000 });
    await press(1);
    await vi.waitFor(async () => expect((await view()).values["items"]).toMatchObject({ state: "done", runs: 2 }), { timeout: 15_000 });

    // One server for the thread, started once on hetzner and still running: the variables reached it off its stdin.
    const opened = seen.frames.filter(f => f["op"] === "stdio.open");
    expect(opened).toHaveLength(1);
    expect(readFileSync(starts, "utf8").trim().split("\n")).toHaveLength(1);
    expect(running.size).toBe(1);
    // No value rides the command line; the first bytes on its stdin are the NUL-ended pairs.
    expect(String(opened[0]!["cmd"])).not.toContain(TOKEN);
    const first = seen.frames.find(f => f["op"] === "stdio.write")!;
    expect(Buffer.from(String(first["data"]), "base64").toString("utf8")).toContain(`STUB_KEY\0${TOKEN}\0`);
    // The config read was hetzner's, for the project's folder there, and this computer's never.
    expect(asked.length).toBeGreaterThan(0);
    expect(asked.every(on => on.kind === "box" && on.name === "hetzner" && on.projects?.[0]?.path === "/root/spoo-ts")).toBe(true);

    // The thread's delete stops its server there.
    await rt.slates.forget(threadId);
    await vi.waitFor(() => expect(running.size).toBe(0), { timeout: 5_000 });
    expect(seen.frames.filter(f => f["op"] === "stdio.close")).toHaveLength(1);
    await rt.close();
  }, 40_000);

  it("refuses at write a server that computer's config does not name, and a command run on the host, naming that computer", async () => {
    const { rt, seen, threadId, caller } = await boxThread();
    const missing = `<slate title="Zones"><run name="zones" tool="cloudflare.zones_list" description="Lists the account's zones" /><run name="items" tool="stub.list_items" description="Lists the newest items" /><column><output run={$zones} /></column></slate>`;
    await expect(rt.slates.write({ text: missing }, caller)).rejects.toMatchObject({ kind: "invalid", message: `slate refused: 1 error: K710 server-not-set-up ${slateServerNotSetUp("cloudflare", "hetzner", { id: "claude", name: "Claude Code" }, threadId)}` });
    const onHost = `<slate title="Me"><run name="me" cmd="whoami" on="host" /><column><output run={$me} /></column></slate>`;
    await expect(rt.slates.write({ text: onHost }, caller)).rejects.toThrow(/K711 host-away \$me runs on="host", which is .+ with the sign-ins there, and this thread runs on hetzner: drop on="host" and it runs on hetzner$/);
    expect(await rt.slates.get(threadId)).toBeNull();
    // A server it does have writes, and nothing starts until the slate runs it.
    await rt.slates.write({ text: `<slate title="Items"><run name="items" tool="stub.list_items" description="Lists the newest items" /><column><output run={$items} /></column></slate>` }, caller);
    expect(seen.frames.filter(f => f["op"] === "stdio.open")).toHaveLength(0);
    await rt.close();
  }, 40_000);

  it("is refused on a computer whose daemon cannot hold a command open, naming the update, and sends it nothing", async () => {
    const { rt, seen, threadId, caller } = await boxThread({ daemonVersion: STDIO_DAEMON_VERSION - 1 });
    await rt.slates.write({ text: `<slate title="Items"><run name="items" tool="stub.list_items" description="Lists the newest items" /><column><button id="go" label="Go" onPress={start($items)} /></column></slate>` }, caller);
    const view = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    const asked = await rt.slates.event({ threadId, version: (await view()).version, piece: "go", event: "press", requestId: "go-0" });
    await rt.slates.approve({ threadId, key: asked.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["items"]).toMatchObject({ state: "failed", why: `stub did not start: it could not be started on hetzner: ${slateServerBehindLine("hetzner")}` }), { timeout: 15_000 });
    expect(seen.frames.filter(f => String(f["op"]).startsWith("stdio."))).toEqual([]);
    await rt.close();
  }, 40_000);
});
