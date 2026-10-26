// SPDX-License-Identifier: AGPL-3.0-only
// What an add does on the computer holding the project: the clone, the files
// the person ticked, the install the lockfile picks, and on a provider the
// image every workspace of the project forks. Over the stub backend, so every
// command the machine was asked to run is read back here.
import { execFileSync } from "node:child_process";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { HERE_PLACE_ID, type AdapterEvent, type MachineSpec, type SeedChoice, type SeedPlan, type TurnResult } from "@wsp/protocol";
import { copyKey, createRuntime, type HarnessAdapterFactory, type Runtime, type SeedWiring } from "../src/runtime.js";
import { memoryStore, type Store } from "../src/store.js";
import { fakeLocal, stubBackend, type StubBackend } from "./stub-backend.js";

const version = { version: 1, snapshotId: "snap_golden-v1", baseTemplate: "base", setupSha: "s1", createdAt: "2026-09-17T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 } };
const REMOTE = "https://github.com/spoo-me/frontend.git";

/** What every adapter this file wires was handed at its launch: the harness, the environment and the folder its
 * agent keys this project's sessions and memory to. */
const launches: { harness: string; env: Record<string, string>; projectKey?: string }[] = [];

/** An adapter that records the environment it was built with and answers a turn at once, one per harness. */
function recordingAdapter(harness: string): HarnessAdapterFactory {
  return ctx => {
    launches.push({ harness, env: { ...ctx.env }, ...(ctx.projectKey !== undefined ? { projectKey: ctx.projectKey } : {}) });
    return {
      steers: false,
      start: o => {
        const sessionId = `00000000-0000-4000-8000-${String(launches.length).padStart(12, "0")}`;
        const result: TurnResult = { status: "completed", text: "ok" };
        const finished = (async () => {
          for (const e of [
            { type: "session.start", sessionId, model: "m" },
            { type: "turn.done", sessionId, result },
            { type: "session.end", sessionId, exitCode: 0, sawResult: true },
          ] as AdapterEvent[]) {
            o.onEvent(e);
          }
          return result;
        })();
        return { localId: sessionId, finished, interrupt: async () => {} };
      },
    };
  };
}

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

/** A git repo in a folder of its own on this computer: the source a seed reads. */
/** Resolved, as the host keys a folder's remembered picks: a Mac's tmpdir sits behind the /var link. */
function repo(): string {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), "wsp-seed-add-")));
  roots.push(dir);
  execFileSync("git", ["init", "-q", dir]);
  return dir;
}

function plan(folder: string, over: Partial<SeedPlan> = {}): SeedPlan {
  return {
    source: folder,
    remote: REMOTE,
    branch: "refactor/dashboard-polish",
    defaultBranch: "main",
    unpushed: null,
    uncommitted: 2,
    memory: { key: "-Users-dev-spoo-landing", files: 3, bytes: 20_000 },
    files: [
      { path: ".env.local", dir: false, bytes: 4096, kind: "config", row: { id: "next", name: "Next" }, ticked: true },
      { path: "node_modules", dir: true, bytes: 2_600_000_000, kind: "rebuilt", row: { id: "node", name: "Node" }, ticked: false },
    ],
    remembered: false,
    ...over,
  };
}

/** A host with an image sealed on the computer named, the stub provider as its backend and a folder reader that
 * answers the plan handed in: what an add of a folder on this computer onto a computer that clones needs. */
async function withImage(o: { at?: string; plan: SeedPlan; packed?: Buffer; left?: string[]; vault?: Record<string, string> } = { plan: plan("/x") }): Promise<{ rt: Runtime; backend: StubBackend; store: Store; seed: SeedWiring; packs: { plan: SeedPlan; choice: SeedChoice }[] }> {
  const backend = stubBackend();
  const store = memoryStore();
  await store.put("goldens", copyKey(o.at ?? "default", "default"), { head: 1, versions: [version] });
  const packs: { plan: SeedPlan; choice: SeedChoice }[] = [];
  const seed: SeedWiring = {
    plan: async () => o.plan,
    pack: async ({ plan: p, choice }) => {
      packs.push({ plan: p, choice });
      return { tar: o.packed ?? Buffer.from("seed archive"), files: choice.files.length, bytes: 12, commits: 0, left: o.left ?? [] };
    },
  };
  const root = mkdtempSync(join(tmpdir(), "wsp-add-local-"));
  roots.push(root);
  const adapters = { claude: recordingAdapter("claude"), codex: recordingAdapter("codex") };
  return { rt: createRuntime({ backend, store, adapters, local: fakeLocal(root), seed, killConfirm: { graceMs: 20, pollMs: 1 }, ...(o.vault !== undefined ? { vault: () => o.vault! } : {}) }), backend, store, seed, packs };
}

/** Every command every machine of this backend was asked to run, oldest first. */
const commands = (backend: StubBackend): string => backend.machines.flatMap(m => m.execLog).join("\n");
/** The spec of every machine it was asked for, in order. */
const specs = (backend: StubBackend): MachineSpec[] => backend.machines.map(m => m.spec);
/** What the machine answers one command with, over the probe the stub answers by default. */
function answering(backend: StubBackend, reply: (cmd: string) => { exitCode: number; stdout: string; stderr: string } | undefined): void {
  const own = backend.execImpl;
  backend.execImpl = (m, cmd) => reply(cmd) ?? own(m, cmd);
}

describe("a repo named by owner and repo", () => {
  it("is cloned through the host's own command line, which the computer's image is checked for first", async () => {
    const { rt, backend } = await withImage({ plan: plan("/x") });
    answering(backend, () => ({ exitCode: 0, stdout: "", stderr: "" }));
    const project = await rt.projects.add({ source: "spoo-me/frontend", on: "default" });
    expect(project.source).toEqual({ kind: "github", repo: "spoo-me/frontend" });
    expect(project.remote).toBe("https://github.com/spoo-me/frontend.git");
    // Nothing runs at the add for a repo the computer clones by itself; the workspace's own create does that.
    expect(commands(backend)).not.toContain("gh repo clone");
    const before = backend.machines.length;
    await rt.workspaces.create({ project: project.id, name: "work" });
    const ran = commands(backend);
    expect(ran).toContain("command -v gh");
    expect(ran).toContain("gh auth setup-git");
    expect(ran).toContain("gh repo clone spoo-me/frontend /root/frontend");
    expect(specs(backend)[before]?.binds).toBeUndefined();
  });

  it("a gitlab repo is named with its host and cloned through glab", async () => {
    const { rt, backend } = await withImage({ plan: plan("/x") });
    answering(backend, () => ({ exitCode: 0, stdout: "", stderr: "" }));
    const project = await rt.projects.add({ source: "gitlab.com/dev/thing", on: "default" });
    expect(project.source).toEqual({ kind: "gitlab", repo: "dev/thing" });
    await rt.workspaces.create({ project: project.id, name: "work" });
    expect(commands(backend)).toContain("glab repo clone dev/thing /root/thing");
  });
});

describe("a file of values or a key on the menu", () => {
  const values = { path: ".env", dir: false, bytes: 120, kind: "secret" as const, row: { id: "node", name: "Node" }, ticked: true };

  it("stays ticked for a computer of the person's own, remembered or not, and is unticked for a provider place, which the plan names", async () => {
    const folder = repo();
    const { rt, backend, store } = await withImage({ plan: plan(folder, { files: [values] }) });
    answering(backend, () => ({ exitCode: 0, stdout: "", stderr: "" }));
    expect((await rt.projects.seedPlan(folder)).files.filter(f => f.ticked).map(f => f.path)).toEqual([".env"]);
    const cloud = await rt.projects.seedPlan(folder, "default");
    expect(cloud.provider).toBe(true);
    expect(cloud.files.filter(f => f.ticked)).toEqual([]);
    // A choice remembered from an add to a box keeps the file ticked for the next box, and no provider gets it.
    await store.put("seed-choices", folder, { files: [".env"], memory: false, commits: false, remember: true });
    const box = await rt.projects.seedPlan(folder);
    expect([box.remembered, box.files.filter(f => f.ticked).map(f => f.path)]).toEqual([true, [".env"]]);
    const remembered = await rt.projects.seedPlan(folder, "default");
    expect([remembered.remembered, remembered.files.filter(f => f.ticked)]).toEqual([true, []]);
  });

});

describe("a project taken away from this computer or from a provider", () => {

  it("a folder here is recorded on a host that has sealed no image, since nothing of it is forked anywhere", async () => {
    const backend = stubBackend();
    const root = mkdtempSync(join(tmpdir(), "wsp-add-noimage-"));
    roots.push(root);
    // No goldens document at all: this host has sealed nothing.
    const rt = createRuntime({ backend, store: memoryStore(), adapters: {}, local: fakeLocal(root) });
    // The folder as this host resolved it, which on a Mac is the real path under /private.
    const recorded = await rt.projects.add({ source: repo() });
    expect(recorded).toMatchObject({ computer: HERE_PLACE_ID, source: { kind: "folder" } });
    expect(recorded.path).toMatch(/wsp-seed-add-/);
    expect(backend.machines).toEqual([]);
  });
});

