// SPDX-License-Identifier: AGPL-3.0-only
// Real project files pass over a fake joined link; no destination project or Git host is involved.
import { execFileSync, spawnSync } from "node:child_process";
import { appendFileSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { afterEach, expect, it } from "vitest";
import { lostWorkspace, reap } from "@wsp/engine";
import { DaemonRequest, type WorkspaceCreatingEvent, type Caller } from "@wsp/protocol";
import { ISOLATED_GROUPS } from "../src/machines/isolated.js";
import { code, ctx, forks, join as joinHost, KEEPS_NO_IMAGE, serving, sockets, relink, type ForkingPlace } from "./places-fixture.js";
import { branchDaemons, fakeLocal } from "./stub-backend.js";
import { leading, type Held } from "./box-fixture.js";
import { report } from "./place-join.js";

let root: string | undefined;
const groups = new Set<string>();
afterEach(() => { for (const id of groups) rmSync(`/tmp/wsp-children/${id}`, { recursive: true, force: true }); groups.clear(); if (root !== undefined) rmSync(root, { recursive: true, force: true }); root = undefined; });
const git = (folder: string, ...args: string[]) => execFileSync("git", ["-C", folder, ...args], { encoding: "utf8" }).trim();
async function fixture() {
  root = mkdtempSync(join(tmpdir(), "wsp-isolated-"));
  const repo = join(root, "project"); mkdirSync(repo); git(repo, "init", "-q", "-b", "main");
  git(repo, "config", "user.name", "Test"); git(repo, "config", "user.email", "test@example.com");
  writeFileSync(join(repo, "file"), "original\n"); git(repo, "add", "."); git(repo, "commit", "-qm", "initial");
  const starts: Held[] = [];
  const control = { oldDaemon: false };
  const base = branchDaemons({ branchOf: () => "main" });
  const options: Parameters<typeof serving>[0] = { local: { ...fakeLocal(join(root, "home")), daemonRoad: async () => ({ url: "http://127.0.0.1:1", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "t" }) }, adapters: { claude: leading(starts) }, daemonChannel: async o => {
    const channel = await base.open(o);
    return { ...channel, send: async frame => {
      if (frame.op !== "git.mergeIn") return channel.send(frame);
      if (control.oldDaemon) throw new Error("a merge from a copy's folder runs only on the computer the copies sit on; merge from the remote here");
      const request = DaemonRequest.parse({ ...frame, id: frame.id ?? 0 });
      if (request.op !== "git.mergeIn") throw new Error("expected merge op");
      const before = git(request.cwd, "rev-parse", "HEAD");
      git(request.cwd, "fetch", "-q", "--no-tags", request.from!, request.branch);
      git(request.cwd, "merge", "--ff-only", "FETCH_HEAD");
      return { id: request.id, ok: true as const, branch: request.branch, merged: true, commits: Number(git(request.cwd, "rev-list", "--count", `${before}..HEAD`)), oid: git(request.cwd, "rev-parse", "HEAD"), head: "main", conflicts: [] };
    } };
  } };
  const { hostKey, store } = await serving(options);
  let seen!: ForkingPlace;
  const run = (cmd: string) => {
    // These file transport tests never write a guest's global context onto the computer running Vitest.
    if (/hostname/.test(cmd) || /-C (?:'\/'|"\/"|\/)(?:\s|$)/.test(cmd) || cmd.includes("/root/") || cmd.includes("/etc/")) return { exitCode: 0, stdout: "child\n", stderr: "" };
    const result = spawnSync("bash", ["-c", cmd.replace("setsid nohup", "nohup")], { encoding: "utf8", maxBuffer: 4 * 1024 * 1024 });
    return { exitCode: result.status ?? 1, stdout: result.stdout, stderr: result.stderr };
  };
  const joined = await joinHost(hostKey, { code: await code(), report: report("hetzner"), answers: client => { seen = forks(client, undefined, run, KEEPS_NO_IMAGE); } });
  sockets.push(joined.client.ws);
  const files = (frame: Record<string, unknown>) => {
    if (frame["op"] === "machine.create") {
      const id = (frame["spec"] as { labels?: Record<string, string> }).labels?.["wsp.isolated"];
      if (id !== undefined) groups.add(id);
    }
    if (frame["op"] === "machine.putBytes") {
      const path = String(frame["path"]); mkdirSync(dirname(path), { recursive: true });
      const data = Buffer.from(String(frame["data"]), "base64");
      if (frame["seq"] === 0) writeFileSync(path, data); else appendFileSync(path, data);
      return {};
    }
    return undefined;
  };
  seen.answer = files;
  const restart = async () => {
    joined.client.close();
    await ctx.srv!.close();
    await ctx.runtime!.close();
    await serving({ ...options, store, hostKey });
    const again = await relink(hostKey, joined.placeId, joined.pair, report("hetzner"), client => { seen = forks(client, undefined, run, KEEPS_NO_IMAGE); seen.answer = files; });
    sockets.push(again.client.ws);
    return { rt: ctx.runtime!, seen };
  };
  const rt = ctx.runtime!;
  const project = await rt.projects.add({ source: repo });
  const folder = await rt.workspaces.create({ project: project.id, name: "lead", agents: { spawn: true, maxMachines: 1 } });
  const turn = await rt.sessions.start(folder.id, { harness: "claude", prompt: "coordinate" });
  const threadId = turn.view().threadId!;
  const lead: Caller = { origin: "here", by: { kind: "thread", threadId, workspaceId: folder.id, rootThreadId: threadId } };
  return { rt, project, repo, store, lead, seen, starts, control, turn, threadId, restart, place: joined.placeId };
}
it("copies current work to a connected computer with no destination project and shares one runtime across concurrent siblings", async () => {
  const f = await fixture(); writeFileSync(join(f.repo, "file"), "dirty\n");
  const frames: WorkspaceCreatingEvent[] = [];
  f.rt.events.on("workspace.creating", e => { if (e.type === "workspace.creating") frames.push(e); });
  const make = (name: string) => f.rt.workspaces.create({ project: "", name, sub: { on: "hetzner" } }, f.lead);
  const [a, b] = await Promise.all([make("first"), make("second")]);
  expect(f.seen.created).toHaveLength(1); expect(a.machineId).toBe(b.machineId); expect(a.id).not.toBe(b.id);
  expect(a.project.id).toBe(f.project.id); expect(a.sub!.folder).not.toBe(b.sub!.folder);
  expect(readFileSync(join(a.sub!.folder, "file"), "utf8")).toBe("dirty\n"); expect(git(a.sub!.folder, "branch", "--show-current")).toBe(a.sub!.out.branch);
  expect(await f.rt.projects.list()).toHaveLength(1);
  const progress = frames.filter(e => e.workspaceId === a.id && e.stage === "project-cloned");
  expect(progress.length).toBeGreaterThan(1);
  expect(progress.slice(1).every(e => e.waiting === true)).toBe(true);
  const child = await f.rt.sessions.start(a.id, { harness: "claude", prompt: "hello", cwd: a.sub!.folder }, f.lead);
  await child.finished; expect(f.starts.at(-1)!.o.cwd).toBe(a.sub!.folder);
  expect((await f.rt.sessions.list(a.id)).at(-1)?.sub).toMatchObject({ mode: "isolated", spawn: { readyAt: expect.any(Number), files: { ms: expect.any(Number) } } });
  expect(child.view()).toMatchObject({ parentThreadId: f.threadId, rootThreadId: f.threadId });
  expect(f.seen.killed).toHaveLength(0);
  // Held work prevents deletion of the shared runtime.
  await expect(f.rt.workspaces.delete(a.id, f.lead)).rejects.toThrow(/not returned/);
  expect(f.seen.killed).toHaveLength(0);
  for (const made of [a, b]) { git(made.sub!.folder, "reset", "--hard", "HEAD"); await f.rt.workspaces.delete(made.id, f.lead); }
  expect(f.seen.killed).toHaveLength(1); expect(await f.store.list(ISOLATED_GROUPS)).toEqual([]);
  f.starts[0]!.answer("done"); await f.turn.finished;
});
it("refuses an owner call or a different source project before allocating compute", async () => {
  const f = await fixture();
  await expect(f.rt.workspaces.create({ project: "", name: "no lead", sub: { on: "hetzner" } })).rejects.toThrow(/thread/);
  await expect(f.rt.workspaces.create({ project: "another", name: "wrong", sub: { on: "hetzner" } }, f.lead)).rejects.toThrow(/asking thread/);
  expect(f.seen.created).toHaveLength(0);
  f.starts[0]!.answer("done"); await f.turn.finished;
});

it("returns a child branch over the link with no remote, and releases only after the lead is settled", async () => {
  const f = await fixture();
  const made = await f.rt.workspaces.create({ project: "", name: "result", sub: { on: "hetzner" } }, f.lead);
  const child = await f.rt.sessions.start(made.id, { harness: "claude", prompt: "hello", cwd: made.sub!.folder }, f.lead);
  await child.finished;
  writeFileSync(join(made.sub!.folder, "result"), "done");
  git(made.sub!.folder, "add", "."); git(made.sub!.folder, "-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-qm", "result");
  const result = await f.rt.workspaces.mergeIn({ workspaceId: f.turn.view().workspaceId, child: made.id, threadId: f.threadId, childThreadId: child.view().threadId }, f.lead);
  expect(result).toMatchObject({ merged: true, commits: 1 }); expect(readFileSync(join(f.repo, "result"), "utf8")).toBe("done");
  expect(f.seen.killed).toHaveLength(0);
  f.starts[0]!.answer("done"); await f.turn.finished;
  expect(f.seen.killed).toHaveLength(0);
  await f.rt.sessions.settle([f.threadId]);
  await expect.poll(async () => (await f.rt.workspaces.get(made.id)).phase).toBe("released");
  expect(f.seen.killed).toHaveLength(1);
});

it("reuses the persisted shared runtime and child checkout after the host restarts", async () => {
  const f = await fixture();
  const first = await f.rt.workspaces.create({ project: "", name: "before restart", sub: { on: "hetzner" } }, f.lead);
  f.starts[0]!.answer("done"); await f.turn.finished;
  const again = await f.restart();
  const held = await again.rt.workspaces.get(first.id);
  expect(held.machineId).toBe(first.machineId);
  const sibling = await again.rt.workspaces.create({ project: "", name: "after restart", sub: { on: "hetzner" } }, f.lead);
  expect(sibling.machineId).toBe(first.machineId); expect(again.seen.created).toHaveLength(0);
  expect(sibling.sub!.folder).not.toBe(first.sub!.folder);
  await again.rt.workspaces.delete(first.id, f.lead);
  expect(again.seen.killed).toHaveLength(0);
  await again.rt.workspaces.delete(sibling.id, f.lead);
  expect(again.seen.killed).toEqual([first.machineId]);
});
it("keeps the first child's compute when its sibling transfer fails", async () => {
  const f = await fixture();
  const first = await f.rt.workspaces.create({ project: "", name: "survivor", sub: { on: "hetzner" } }, f.lead);
  const answer = f.seen.answer!;
  f.seen.answer = frame => frame["op"] === "machine.putBytes" ? {} : answer(frame);
  await expect(f.rt.workspaces.create({ project: "", name: "bad transfer", sub: { on: "hetzner" } }, f.lead)).rejects.toThrow(/transfer|hash/i);
  expect(f.seen.killed).toHaveLength(0); expect(readFileSync(join(first.sub!.folder, "file"), "utf8")).toBe("original\n");
  expect((await f.store.list(ISOLATED_GROUPS))[0]).toMatchObject({ members: [first.id] });
  f.seen.answer = answer;
  await f.rt.workspaces.delete(first.id, f.lead);
  f.starts[0]!.answer("done"); await f.turn.finished;
});
it("makes shared compute foreign to the legacy orphan reaper and adopter", async () => {
  const f = await fixture();
  const made = await f.rt.workspaces.create({ project: "", name: "safe rollback", sub: { on: "hetzner" } }, f.lead);
  const spec = f.seen.created[0]!;
  const group = (await f.store.list(ISOLATED_GROUPS))[0] as { spec: { labels: Record<string, string> } };
  const labels: Record<string, string> = { ...group.spec.labels, createdAt: "2020-01-01T00:00:00.000Z" };
  const owner = labels["wsp-owner"]!.split("/isolated/")[0]!;
  const row = { id: made.machineId, state: "running" as const, labels };
  expect(lostWorkspace(row, owner, Date.now())).toBe(false);
  const result = await reap({ owner, listing: [row], knownIds: () => [], backend: { pricing: { defaultSize: spec, rateUsdPerHour: () => 0 } } as never, stop: async () => { throw new Error("legacy reaper must not delete shared compute"); } });
  expect(result.reaped).toEqual([]); expect(result.spared[0]).toMatchObject({ whose: "foreign" });
  await f.rt.workspaces.delete(made.id, f.lead);
  f.starts[0]!.answer("done"); await f.turn.finished;
});

it("refuses a new send while shared-compute cleanup owns the checkout", async () => {
  const f = await fixture();
  const made = await f.rt.workspaces.create({ project: "", name: "cleanup race", sub: { on: "hetzner" } }, f.lead);
  const child = await f.rt.sessions.start(made.id, { harness: "claude", prompt: "hello" }, f.lead); await child.finished;
  const answer = f.seen.answer!;
  let entered!: () => void; const reading = new Promise<void>(resolve => { entered = resolve; });
  let resume!: () => void; const held = new Promise<void>(resolve => { resume = resolve; });
  let reads = 0;
  f.seen.answer = frame => {
    // delete checks once before changing state, then release owns cleanup while checking again.
    if (frame["op"] === "machine.exec" && String(frame["cmd"]).includes("status --porcelain") && ++reads === 2) {
      entered(); return held.then(() => ({ result: { exitCode: 0, stdout: "", stderr: "" } }));
    }
    return answer(frame);
  };
  const deletion = f.rt.workspaces.delete(made.id, f.lead);
  try {
    await reading;
    await expect(f.rt.sessions.start(made.id, { harness: "claude", prompt: "race", thread: child.view().threadId }, f.lead)).rejects.toThrow(/released|cleanup/i);
  } finally { resume(); }
  await deletion; expect(f.seen.killed).toHaveLength(1);
  f.starts[0]!.answer("done"); await f.turn.finished;
});
it("retains ownership and retries when the connected computer refuses the final delete", async () => {
  const f = await fixture();
  const made = await f.rt.workspaces.create({ project: "", name: "delete failure", sub: { on: "hetzner" } }, f.lead);
  f.seen.refuseKill = new Set([made.machineId]);
  await expect(f.rt.workspaces.delete(made.id, f.lead)).rejects.toThrow();
  expect(await f.store.list(ISOLATED_GROUPS)).toHaveLength(1); expect(await f.rt.workspaces.get(made.id)).toMatchObject({ machineId: made.machineId });
  f.seen.refuseKill.clear(); await f.rt.workspaces.delete(made.id, f.lead);
  expect(await f.store.list(ISOLATED_GROUPS)).toEqual([]);
  f.starts[0]!.answer("done"); await f.turn.finished;
});

it("keeps different leads on separate runtimes on the same connected computer", async () => {
  const f = await fixture();
  const first = await f.rt.workspaces.create({ project: "", name: "first tree", sub: { on: "hetzner" } }, f.lead);
  const other = await f.rt.sessions.start(f.turn.view().workspaceId, { harness: "claude", prompt: "another lead" });
  const threadId = other.view().threadId!;
  const caller: Caller = { origin: "here", by: { kind: "thread", workspaceId: other.view().workspaceId, threadId, rootThreadId: threadId } };
  const second = await f.rt.workspaces.create({ project: "", name: "second tree", sub: { on: "hetzner" } }, caller);
  expect(first.machineId).not.toBe(second.machineId); expect(await f.store.list(ISOLATED_GROUPS)).toHaveLength(2);
  await f.rt.workspaces.delete(first.id, f.lead); expect(f.seen.killed).toEqual([first.machineId]);
  await f.rt.workspaces.delete(second.id, caller);
  f.starts[1]!.answer("done"); await other.finished;
  f.starts[0]!.answer("done"); await f.turn.finished;
});

it("keeps child work and removes the returned bundle when an older daemon refuses it", async () => {
  const f = await fixture();
  const made = await f.rt.workspaces.create({ project: "", name: "old daemon", sub: { on: "hetzner" } }, f.lead);
  f.control.oldDaemon = true;
  await expect(f.rt.workspaces.mergeIn({ workspaceId: f.turn.view().workspaceId, child: made.id, threadId: f.threadId }, f.lead)).rejects.toThrow(/Update the computer/);
  expect(readdirSync(f.repo).some(name => name.startsWith(".wsp-return-"))).toBe(false);
  expect(readFileSync(join(made.sub!.folder, "file"), "utf8")).toBe("original\n"); expect(f.seen.killed).toEqual([]);
  await f.rt.workspaces.delete(made.id, f.lead);
  f.starts[0]!.answer("done"); await f.turn.finished;
});

it("finishes an interrupted checkout release after restart without deleting its active sibling", async () => {
  const f = await fixture();
  const a = await f.rt.workspaces.create({ project: "", name: "kept sibling", sub: { on: "hetzner" } }, f.lead);
  const b = await f.rt.workspaces.create({ project: "", name: "interrupted release", sub: { on: "hetzner" } }, f.lead);
  f.starts[0]!.answer("done"); await f.turn.finished;
  const record = await f.store.get("workspaces", b.id) as { isolated: { state: string } };
  record.isolated.state = "releasing"; await f.store.put("workspaces", b.id, record);
  rmSync(b.sub!.folder, { recursive: true });
  const again = await f.restart();
  await expect.poll(async () => (await again.rt.workspaces.get(b.id)).phase).toBe("released");
  expect(again.seen.killed).toEqual([]); expect(readFileSync(join(a.sub!.folder, "file"), "utf8")).toBe("original\n");
  expect((await f.store.list(ISOLATED_GROUPS))[0]).toMatchObject({ members: [a.id] });
  await again.rt.workspaces.delete(a.id, f.lead); expect(again.seen.killed).toEqual([a.machineId]);
});

it("reports a failed restart in the shared startup line instead of spinning forever", async () => {
  const f = await fixture();
  const made = await f.rt.workspaces.create({ project: "", name: "restart failure", sub: { on: "hetzner" } }, f.lead);
  const child = await f.rt.sessions.start(made.id, { harness: "claude", prompt: "hello" }, f.lead); await child.finished;
  f.starts[0]!.answer("done"); await f.turn.finished;
  await f.rt.sessions.settle([f.threadId]);
  await expect.poll(async () => (await f.rt.workspaces.get(made.id)).phase).toBe("released");
  rmSync(f.repo, { recursive: true });
  await expect(f.rt.sessions.start(made.id, { harness: "claude", prompt: "continue", thread: child.view().threadId })).rejects.toThrow();
  const sub = (await f.rt.sessions.list(made.id)).at(-1)?.sub;
  expect(sub?.mode).toBe("isolated"); expect(sub?.spawn?.failed?.said).toMatch(/transfer|checkout|folder/i);
});
