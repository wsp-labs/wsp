// SPDX-License-Identifier: AGPL-3.0-only
// This computer's folders as a browser tab's picker walks them: folders only,
// sorted, repositories marked, the dot-named ones counted rather than listed
// until they are asked for, the roots being the home folder and each imported
// project's own folder, and every path outside those roots refused, by a typed
// path or by a symlink that leaves them.
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { WorkspaceView } from "@wsp/protocol";
import { afterEach, describe, expect, it } from "vitest";
import { hostFolderRoots, hostFolders, importedProjectFolders, listHostFolders, listHostRepos } from "../src/host-folders.js";
import { withChildFs } from "./child-fs.js";

const dirs: string[] = [];
afterEach(() => {
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
});

/** A home folder with a repository, a plain folder and two dot-named ones inside it, a project folder outside home,
 * and a folder on this disk that no root holds. */
function tree(): { home: string; project: string; elsewhere: string } {
  const root = mkdtempSync(join(tmpdir(), "wsp-folders-"));
  dirs.push(root);
  const home = join(root, "home");
  const project = join(root, "work", "api");
  const elsewhere = join(root, "elsewhere");
  mkdirSync(join(home, "code", "spoo", ".git"), { recursive: true });
  mkdirSync(join(home, "code", "notes"), { recursive: true });
  mkdirSync(join(home, "code", ".cache"), { recursive: true });
  mkdirSync(join(home, "code", ".config"), { recursive: true });
  mkdirSync(join(home, "Applications"), { recursive: true });
  writeFileSync(join(home, "code", "README.md"), "read me\n");
  mkdirSync(join(project, "src"), { recursive: true });
  mkdirSync(join(elsewhere, "secrets"), { recursive: true });
  return { home, project, elsewhere };
}

const names = (folders: readonly { path: string }[]): string[] => folders.map(f => f.path.slice(f.path.lastIndexOf("/") + 1));
const workspace = (over: Partial<WorkspaceView>): WorkspaceView => ({
  id: "ws_a",
  name: "api",
  machineId: "m_a",
  phase: "running",
  golden: "snap_g",
  createdAt: "2026-09-06T10:00:00Z",
  project: { id: "pr_a", name: "api", path: "/root/api", computer: "default" },
  ...over,
});

describe("this computer's folder listing", () => {
  it("lists folders only, sorted, marks the repository, and counts the hidden ones without listing them", async () => {
    const { home } = tree();
    const listing = await listHostFolders({ dir: join(home, "code") }, { home });
    expect(listing.dir).toBe(join(home, "code"));
    expect(names(listing.folders)).toEqual(["notes", "spoo"]);
    expect(listing.folders.map(f => f.repo)).toEqual([false, true]);
    expect(listing.folders.map(f => f.path)).toEqual([join(home, "code", "notes"), join(home, "code", "spoo")]);
    expect(listing.hidden).toBe(2);
  });

  it("raises for a folder inside the roots this Mac will not let it read, so the picker reads the refusal instead of an empty level", async () => {
    const { home } = tree();
    const shut = join(home, "code");
    await withChildFs({ refuse: shut }, async () => {
      await expect(listHostFolders({ dir: shut }, { home })).rejects.toThrow(`EACCES: permission denied, scandir '${shut}'`);
    });
    // Nothing is wrong with the folder above it, so the level the picker was on still lists.
    expect(names((await listHostFolders({ dir: home }, { home })).folders)).toEqual(["Applications", "code"]);
  });

  it("hides the Library a Mac keeps in the home itself, wherever that home sits, and lists one anywhere else", async () => {
    const { home } = tree();
    mkdirSync(join(home, "Library"), { recursive: true });
    mkdirSync(join(home, "code", "Library"), { recursive: true });
    // A home under a temp folder, which is where every lab account and every test home sits: the home decides, so
    // the path's shape has no say.
    const onAMac = await listHostFolders({ dir: home }, { home, platform: "darwin" });
    expect(names(onAMac.folders)).toEqual(["Applications", "code"]);
    expect(onAMac.hidden).toBe(1);
    // Asked for, it is a row like any other.
    expect(names((await listHostFolders({ dir: home, hidden: true }, { home, platform: "darwin" })).folders)).toContain("Library");
    // A Library inside their own work is theirs, on a Mac as anywhere.
    expect(names((await listHostFolders({ dir: join(home, "code") }, { home, platform: "darwin" })).folders)).toContain("Library");
    // No other computer keeps one in the home, so nothing is hidden there.
    const elsewhere = await listHostFolders({ dir: home }, { home, platform: "linux" });
    expect(names(elsewhere.folders)).toEqual(["Applications", "Library", "code"]);
    expect(elsewhere.hidden).toBe(0);
  });

  it("lists the dot-named folders too when they are asked for, and still says how many of the level they are", async () => {
    const { home } = tree();
    const listing = await listHostFolders({ dir: join(home, "code"), hidden: true }, { home });
    expect(names(listing.folders)).toEqual([".cache", ".config", "notes", "spoo"]);
    expect(listing.hidden).toBe(2);
  });

  it("browses from the home folder and every imported project folder, and starts at home when nothing was asked for", async () => {
    const { home, project } = tree();
    expect(await hostFolderRoots({ home, projects: [project] })).toEqual([home, project]);
    const listing = await listHostFolders({}, { home, projects: [project] });
    expect(listing.dir).toBe(home);
    expect(listing.roots).toEqual([home, project]);
    expect(names(listing.folders)).toEqual(["Applications", "code"]);
    expect(names((await listHostFolders({ dir: project }, { home, projects: [project] })).folders)).toEqual(["src"]);
  });

  it("keeps out a project folder the home folder already holds, one that is gone, and a second copy of the same one", async () => {
    const { home, project } = tree();
    const inside = join(home, "code", "spoo");
    expect(await hostFolderRoots({ home, projects: [inside, project, project, join(project, "gone")] })).toEqual([home, project]);
  });

  it("refuses a path outside the roots, before anything under it is read, and a relative one", async () => {
    const { home, project, elsewhere } = tree();
    const paths = { home, projects: [project] };
    const refusal = `${elsewhere} is outside the folders wsp browses on this computer: ${home}, ${project}`;
    await expect(listHostFolders({ dir: elsewhere }, paths)).rejects.toThrow(refusal);
    await expect(listHostFolders({ dir: join(elsewhere, "secrets") }, paths)).rejects.toThrow("is outside the folders wsp browses");
    await expect(listHostFolders({ dir: `${home}/code/../../elsewhere` }, paths)).rejects.toThrow("is outside the folders wsp browses");
    await expect(listHostFolders({ dir: "code/spoo" }, paths)).rejects.toThrow("is outside the folders wsp browses");
    // The project folder alone is no longer a root once no record names it.
    await expect(listHostFolders({ dir: project }, { home })).rejects.toThrow("is outside the folders wsp browses");
  });

  it("refuses a symlink inside the roots that points out of them, and leaves it out of the level it sits in", async () => {
    const { home, elsewhere } = tree();
    const out = join(home, "code", "out");
    symlinkSync(elsewhere, out);
    expect(names((await listHostFolders({ dir: join(home, "code") }, { home })).folders)).toEqual(["notes", "spoo"]);
    await expect(listHostFolders({ dir: out }, { home })).rejects.toThrow("is outside the folders wsp browses");
  });

  it("follows a symlink to a folder the roots hold, and leaves a symlink to a file out", async () => {
    const { home } = tree();
    symlinkSync(join(home, "code", "spoo"), join(home, "code", "current"));
    symlinkSync(join(home, "code", "README.md"), join(home, "code", "readme-link"));
    expect(names((await listHostFolders({ dir: join(home, "code") }, { home })).folders)).toEqual(["current", "notes", "spoo"]);
  });

  it("falls back to the first root for a folder inside the roots that is gone or is a file", async () => {
    const { home } = tree();
    for (const dir of [join(home, "code", "moved-away"), join(home, "code", "README.md")]) {
      expect((await listHostFolders({ dir }, { home })).dir).toBe(home);
    }
  });

  it("reads the project folders off the records, each once, and answers a listing over them", async () => {
    const { project } = tree();
    const held = { id: "pr_api", name: "api", path: project, computer: "here" };
    const records = [workspace({ project: held }), workspace({ id: "ws_b", project: held }), workspace({ id: "ws_c" })];
    expect(importedProjectFolders(records)).toEqual([project, "/root/api"]);
    expect(importedProjectFolders([])).toEqual([]);
    const folders = hostFolders(async () => records);
    // hostFolders reads this computer's own home folder, which the paths argument is not given for.
    expect((await folders.list({ dir: project })).roots).toEqual([...(await hostFolderRoots()), project]);
  });
});

describe("the repos listing and this computer's own window", () => {
  it("answers every repo under home with its branch, newest first, leaving out dependency folders, linked worktrees and workspace copies", async () => {
    const { home } = tree();
    mkdirSync(join(home, "deep", "a", "b", "kart", ".git"), { recursive: true });
    writeFileSync(join(home, "deep", "a", "b", "kart", ".git", "HEAD"), "ref: refs/heads/feature/x\n");
    mkdirSync(join(home, "code", "spoo", "node_modules", "dep", ".git"), { recursive: true });
    mkdirSync(join(home, "node_modules", "lib", ".git"), { recursive: true });
    mkdirSync(join(home, "wt"), { recursive: true });
    writeFileSync(join(home, "wt", ".git"), "gitdir: /elsewhere/.git/worktrees/wt\n");
    mkdirSync(join(home, "code", "spoo-pricing", ".git"), { recursive: true });
    const listing = await listHostRepos({ home, copies: [join(home, "code", "spoo-pricing")] });
    const paths = listing.folders.map(f => f.path).sort();
    expect(paths).toEqual([join(home, "code", "spoo"), join(home, "deep", "a", "b", "kart")].sort());
    expect(listing.folders.find(f => f.path.endsWith("kart"))?.branch).toBe("feature/x");
    expect(listing.folders.every(f => f.repo && typeof f.touchedAt === "number")).toBe(true);
  });

  it("lets this computer's own window list a folder outside home, and nobody else", async () => {
    const { home, elsewhere } = tree();
    await expect(listHostFolders({ dir: elsewhere }, { home })).rejects.toThrow(/outside the folders/);
    expect((await listHostFolders({ dir: elsewhere }, { home, wide: true })).folders.map(f => f.path)).toEqual([join(elsewhere, "secrets")]);
  });
});

/** The longest the test's own loop went without a turn while `run` went on, which any read on this thread stalls. */
async function stallOf<T>(run: () => Promise<T>): Promise<{ value: T; stallMs: number }> {
  let last = Date.now();
  let stallMs = 0;
  const tick = setInterval(() => {
    stallMs = Math.max(stallMs, Date.now() - last);
    last = Date.now();
  }, 20);
  try {
    const value = await run();
    return { value, stallMs: Math.max(stallMs, Date.now() - last) };
  } finally {
    clearInterval(tick);
  }
}

describe("a folder that does not answer", () => {
  const opened: ChildProcess[] = [];
  afterEach(() => {
    for (const p of opened.splice(0)) p.kill("SIGKILL");
  });

  it("is skipped by the repos listing with a line after 2 s, while the host's own thread keeps turning and the walk goes on past it", async () => {
    const { home } = tree();
    const stuck = join(home, "code", "stuck");
    mkdirSync(join(stuck, ".git"), { recursive: true });
    mkdirSync(join(home, "zed", ".git"), { recursive: true });
    // A HEAD that is a pipe holds the open that reads it until a writer comes, a real open() that does not return,
    // as one inside a folder macOS guards does while its prompt waits. The writer comes after 4 s so a read on the
    // host's own thread ends at all.
    spawnSync("mkfifo", [join(stuck, ".git", "HEAD")]);
    const writer = spawn("/bin/sh", ["-c", 'sleep 4; exec 3<>"$0"', join(stuck, ".git", "HEAD")], { stdio: "ignore" });
    opened.push(writer);
    const lines: string[] = [];
    const { value, stallMs } = await stallOf(async () => listHostRepos({ home, log: line => lines.push(line) }));
    expect(stallMs).toBeLessThan(500);
    // The writer is still asleep, so nothing but the folder's own deadline ended the read.
    expect(writer.exitCode).toBeNull();
    expect(value.folders.map(f => f.path).sort()).toEqual([join(home, "code", "spoo"), join(home, "zed")]);
    expect(lines).toEqual([`${stuck} did not answer within 2 s, so wsp skipped it`]);
  });

  it("refuses the listing of that folder with the same line, and leaves a folder inside a level out of it with a line", async () => {
    const { home } = tree();
    const mount = join(home, "code", "mount");
    mkdirSync(mount);
    const lines: string[] = [];
    await withChildFs({ guard: mount }, async () => {
      const level = await stallOf(async () => listHostFolders({ dir: join(home, "code") }, { home, log: line => lines.push(line) }));
      expect(level.stallMs).toBeLessThan(500);
      expect(names(level.value.folders)).toEqual(["notes", "spoo"]);
      expect(lines).toEqual([`${mount} did not answer within 2 s, so wsp skipped it`]);
      const asked = await stallOf(() => listHostFolders({ dir: mount }, { home }).then(String, (e: Error) => e.message));
      expect(asked.stallMs).toBeLessThan(500);
      expect(asked.value).toBe(`${mount} did not answer within 2 s, so wsp skipped it`);
    });
  });

  it("takes no line something else in the child prints for an answer", async () => {
    const { home } = tree();
    await withChildFs({ print: "{}" }, async () => {
      expect(names((await listHostFolders({ dir: join(home, "code") }, { home })).folders)).toEqual(["notes", "spoo"]);
    });
  });
});

describe("the folders a Mac guards in the home", () => {
  const GUARDED = ["Desktop", "Documents", "Downloads", "Pictures", "Movies", "Music"];

  it("are never walked into by the repos listing, nor looked inside by the home's own level, unless a path was typed into one", async () => {
    const { home } = tree();
    for (const name of GUARDED) mkdirSync(join(home, name, "repo", ".git"), { recursive: true });
    mkdirSync(join(home, "Library", "Mobile Documents", "com~apple~CloudDocs", "repo", ".git"), { recursive: true });
    mkdirSync(join(home, "Documents", "GitHub", "added", ".git"), { recursive: true });
    const mac = { home, platform: "darwin" };
    // Anything inside the Desktop holds forever here, as it does while macOS asks the person: nothing may wait on it.
    await withChildFs({ guard: join(home, "Desktop") }, async () => {
      const lines: string[] = [];
      const repos = await listHostRepos({ ...mac, log: line => lines.push(line) });
      expect(repos.folders.map(f => f.path)).toEqual([join(home, "code", "spoo")]);
      const level = await listHostFolders({ dir: home }, { ...mac, log: line => lines.push(line) });
      expect(names(level.folders)).toEqual(["Applications", "Desktop", "Documents", "Downloads", "Movies", "Music", "Pictures", "code"]);
      expect(level.folders.find(f => f.path === join(home, "Desktop"))?.repo).toBe(false);
      expect(lines).toEqual([]);
      // Typed, the Desktop is read like any folder, and here it does not answer.
      await expect(listHostFolders({ dir: join(home, "Desktop") }, mac)).rejects.toThrow(`${join(home, "Desktop")} did not answer within 2 s`);
    });
    // A folder the person typed inside one and added as a project is walked, so its repos are listed.
    const added = await listHostRepos({ ...mac, projects: [join(home, "Documents", "GitHub")] });
    expect(added.folders.map(f => f.path).sort()).toEqual([join(home, "Documents", "GitHub", "added"), join(home, "code", "spoo")]);
    // Typed, a guarded folder lists and marks what it holds.
    expect(names((await listHostFolders({ dir: join(home, "Documents") }, mac)).folders)).toEqual(["GitHub", "repo"]);
    // No other computer guards them, so the walk goes in.
    const linux = await listHostRepos({ home, platform: "linux" });
    expect(linux.folders.map(f => f.path)).toHaveLength(GUARDED.length + 3);
  });
});
