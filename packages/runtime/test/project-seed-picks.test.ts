// SPDX-License-Identifier: AGPL-3.0-only
// A project's seed picks: the menu of what git ignores in its folder on this
// computer, the picks the folder's remembered choice leaves on it, the words
// that move them, and the save, which is the record an add with remember
// writes. A thread's token reads the menu and is refused the save.
import { execFileSync } from "node:child_process";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { exitClassOf, neverGitLine, neverTravelsLine, notInTheMenuLine, projectSeedLines, seedSaveRefusal, staysHereLine, type Caller, type SeedPlan } from "@wsp/protocol";
import { copyKey, createRuntime, type Runtime } from "../src/runtime.js";
import { newPlaceKeyPair } from "../src/places.js";
import { serveRuntime } from "../src/serve.js";
import { SEED_CHOICES } from "../src/types/internal.js";
import { memoryStore, type Store } from "../src/store.js";
import { branchDaemons, fakeLocal, stubBackend, withDaemonRoads } from "./stub-backend.js";
import { joinAt, wiring } from "./place-join.js";
import { ctx, forks, HOLDS_PROJECTS, sockets } from "./places-fixture.js";
import { WsClient } from "./ws-client.js";

const IMAGE = { head: 1, versions: [{ version: 1, snapshotId: "snap_golden-v1", baseTemplate: "base", setupSha: "s1", createdAt: "2026-09-17T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 } }] };

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

const made = (prefix: string): string => {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), prefix)));
  roots.push(dir);
  return dir;
};

/** The menu the host's reader would answer for the folder: the catalog's own ticks on, nothing remembered. */
const menu = (folder: string): SeedPlan => ({
  source: folder,
  remote: "https://github.com/acme/lab.git",
  branch: "main",
  defaultBranch: "main",
  unpushed: null,
  uncommitted: 0,
  memory: null,
  files: [
    { path: ".env", dir: false, bytes: 1_126, kind: "config", row: { id: "dotenv", name: "environment file" }, ticked: true },
    { path: ".env.local", dir: false, bytes: 132, kind: "config", row: { id: "dotenv", name: "environment file" }, ticked: true },
    { path: ".git-credentials", dir: false, bytes: 300, kind: "never", row: { id: "logins", name: "logins" }, ticked: false },
    { path: "node_modules", dir: true, bytes: 831_520_768, kind: "rebuilt", row: { id: "node", name: "Node" }, ticked: false },
    { path: "deploy.pem", dir: false, bytes: 4096, kind: "secret", row: { id: "certificates", name: "certificates" }, ticked: true },
  ],
  remembered: false,
});

/** A host with an image on the stub provider, so a folder here can also seed a project there with remember, and a
 * folder project here named lab. */
async function withLab(): Promise<{ rt: Runtime; store: Store; folder: string }> {
  const folder = made("wsp-seed-lab-");
  execFileSync("git", ["init", "-q", folder]);
  const store = memoryStore();
  await store.put("goldens", copyKey("default", "default"), IMAGE);
  const rt = createRuntime({
    backend: stubBackend(),
    store,
    adapters: {},
    local: fakeLocal(made("wsp-seed-home-")),
    seed: { plan: async f => menu(f), pack: async ({ choice }) => ({ tar: Buffer.from("seed"), files: choice.files.length, bytes: 1, commits: 0, left: [] }) },
  });
  await rt.projects.add({ source: folder, name: "lab" });
  return { rt, store, folder };
}

const picked = (rows: readonly { path: string; picked: boolean }[]): string[] => rows.filter(r => r.picked).map(r => r.path);

describe("a project's seed picks", () => {
  it("print the menu with nothing picked where the folder has no saved picks, and save nothing", async () => {
    const { rt, store, folder } = await withLab();
    const seed = await rt.projects.seed("lab", {});
    expect(seed).toEqual({
      project: "lab",
      folder,
      rows: [
        { path: ".env", dir: false, bytes: 1_126, kind: "config", why: "environment file", picked: false },
        { path: ".env.local", dir: false, bytes: 132, kind: "config", why: "environment file", picked: false },
        { path: ".git-credentials", dir: false, bytes: 300, kind: "never", why: "never travels", picked: false },
        { path: "node_modules", dir: true, bytes: 831_520_768, kind: "rebuilt", why: "rebuilt on the box", picked: false },
        { path: "deploy.pem", dir: false, bytes: 4096, kind: "secret", why: "stays on this computer", picked: false },
      ],
      picked: { paths: [], bytes: 0, missing: [] },
      saved: false,
    });
    expect(await store.get(SEED_CHOICES, folder)).toBeUndefined();
  });

  it("read as a provider's menu, since only a sub-thread reads them: a key stays here, is refused by name, and a save keeps the folder's own pick of it until it is cut", async () => {
    const { rt, store, folder } = await withLab();
    await store.put(SEED_CHOICES, folder, { files: [".env", "deploy.pem"], memory: false, commits: false, remember: true });
    const seed = await rt.projects.seed("lab", {});
    expect(seed.picked).toEqual({ paths: [".env"], bytes: 1_126, missing: [] });
    expect(projectSeedLines(seed).rows).toContainEqual([" ", "deploy.pem", "4 KB", "stays on this computer"]);
    await expect(rt.projects.seed("lab", { keep: ["deploy.pem"] })).rejects.toThrow(staysHereLine("deploy.pem"));
    // A box's add reads this same record, so a save made off the provider's menu keeps the key in it.
    await rt.projects.seed("lab", { cut: [".env"], save: true });
    expect(((await store.get(SEED_CHOICES, folder)) as { files: string[] }).files).toEqual(["deploy.pem"]);
    await rt.projects.seed("lab", { cut: ["deploy.pem"], save: true });
    expect(((await store.get(SEED_CHOICES, folder)) as { files: string[] }).files).toEqual([]);
  });

  it("move by --none, then --keep, then --cut, a folder's slash named or not", async () => {
    const { rt } = await withLab();
    await rt.projects.seed("lab", { keep: [".env", ".env.local"], save: true });
    expect(picked((await rt.projects.seed("lab", { none: true })).rows)).toEqual([]);
    expect(picked((await rt.projects.seed("lab", { none: true, keep: ["node_modules/"] })).rows)).toEqual(["node_modules"]);
    expect(picked((await rt.projects.seed("lab", { cut: [".env"] })).rows)).toEqual([".env.local"]);
    const emptied = await rt.projects.seed("lab", { none: true, save: true });
    expect(emptied).toMatchObject({ picked: { paths: [], bytes: 0 }, saved: true });
  });

  it("keep a saved pick whose file is gone on a save and name it, until it is cut", async () => {
    const { rt, store, folder } = await withLab();
    // Saved by an add while .env.old was there; the folder no longer holds it.
    await store.put(SEED_CHOICES, folder, { files: [".env", ".env.old"], memory: false, commits: false, remember: true });
    const read = await rt.projects.seed("lab", {});
    expect(read.picked).toEqual({ paths: [".env"], bytes: 1_126, missing: [".env.old"] });
    expect(read.rows.map(r => r.path)).not.toContain(".env.old");

    expect((await rt.projects.seed("lab", { keep: [".env.local"], save: true })).picked.missing).toEqual([".env.old"]);
    expect(await store.get(SEED_CHOICES, folder)).toMatchObject({ files: [".env", ".env.local", ".env.old"] });

    const refused = await rt.projects.seed("lab", { keep: [".env.old"] }).catch((e: unknown) => e);
    expect(refused).toMatchObject({ message: notInTheMenuLine(".env.old", [".env", ".env.local", ".git-credentials", "node_modules", "deploy.pem"]) });
    expect((await rt.projects.seed("lab", { cut: [".env.old"], save: true })).picked.missing).toEqual([]);
    expect(await store.get(SEED_CHOICES, folder)).toMatchObject({ files: [".env", ".env.local"] });
  });

  it("refuse a path git tracks or that is not there, a login and .git, as usage, in the add's own words", async () => {
    const { rt, store, folder } = await withLab();
    const shown = [".env", ".env.local", ".git-credentials", "node_modules", "deploy.pem"];
    for (const [o, line] of [
      [{ keep: ["package.json"] }, notInTheMenuLine("package.json", shown)],
      [{ cut: ["src"] }, notInTheMenuLine("src", shown)],
      [{ cut: [".env.locl"] }, notInTheMenuLine(".env.locl", shown)],
      [{ keep: [".git-credentials"] }, neverTravelsLine(".git-credentials", "logins")],
      [{ keep: [".git"] }, neverGitLine(".git")],
      [{ keep: [".git/config"], save: true }, neverGitLine(".git/config")],
    ] as const) {
      const refused = await rt.projects.seed("lab", o).catch((e: unknown) => e);
      expect(refused, JSON.stringify(o)).toMatchObject({ message: line });
      expect(exitClassOf(refused)).toBe("usage");
    }
    expect(await store.get(SEED_CHOICES, folder)).toBeUndefined();
  });

  it("are read by a thread's token and refused it the save, with the line that saves them for the person, as usage", async () => {
    const { rt, store, folder } = await withLab();
    const [lab] = await rt.projects.list();
    const lead = await rt.workspaces.create({ project: lab!.id, name: "lead" });
    const asLead: Caller = { origin: "here", by: { kind: "thread", threadId: "lead-thread", workspaceId: lead.id, rootThreadId: "lead-thread" } };
    expect((await rt.projects.seed("lab", { keep: [".env"] }, asLead)).picked.paths).toEqual([".env"]);
    const refused = await rt.projects.seed("lab", { keep: [".env"], save: true }, asLead).catch((e: unknown) => e);
    expect(refused).toMatchObject({ message: seedSaveRefusal("lab", { keep: [".env"] }).message });
    expect((refused as Error).message).toBe("picking the files that leave this computer is the person's act. Hand them the line wsp projects seed lab --keep .env --yes");
    expect(exitClassOf(refused)).toBe("usage");
    expect(await store.get(SEED_CHOICES, folder)).toBeUndefined();
  });

  it("refuse a project on a box, naming the box", async () => {
    const hostKey = newPlaceKeyPair();
    const rt = createRuntime({ backend: withDaemonRoads(stubBackend()), store: memoryStore(), adapters: {}, local: fakeLocal(made("wsp-seed-home-")), placeLinks: wiring(hostKey), daemonChannel: branchDaemons().open, seed: { plan: async f => menu(f), pack: async () => ({ tar: Buffer.from("seed"), files: 0, bytes: 0, commits: 0, left: [] }) } });
    ctx.runtime = rt;
    ctx.srv = await serveRuntime(rt, { port: 0, authToken: "host-token", devices: rt.devices });
    const minted = await WsClient.connect(ctx.srv.port, { token: "host-token" });
    const code = String((await minted.request("pair.issue"))["code"]);
    minted.close();
    const { client } = await joinAt(ctx.srv.port, hostKey, { code, name: "srv", answers: c => forks(c, undefined, undefined, HOLDS_PROJECTS) });
    sockets.push(client.ws);
    await rt.projects.add({ source: "https://github.com/acme/web.git", on: "srv", name: "web" });
    const refused = await rt.projects.seed("web", {}).catch((e: unknown) => e);
    expect(refused).toMatchObject({ message: "web lives on srv, and picks are read from folders on this computer only. A sub-thread of web starts with no git-ignored files." });
    expect(exitClassOf(refused)).toBe("usage");
  });
});
