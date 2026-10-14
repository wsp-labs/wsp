// SPDX-License-Identifier: AGPL-3.0-only
// This computer's own folders, one level at a time, for the folder picker a
// browser tab has: the desktop shell opens the system dialog and hands the
// page a path, and no web picker can give one. Folders only, nothing here
// reads a file, and only inside the roots, which are the home folder and each
// imported project's folder. The lexical check runs before anything under a
// path is read and the realpath check refuses a symlink that leaves the roots,
// so neither a typed path nor a link inside home reaches the rest of the disk.
import { homedir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { CACHE_DIRS, foldersOutsideLine, guardedFolder, hiddenFolder, REPO_CAP, REPO_DEPTH, type HostFolder, type HostFolderListing, type WorkspaceView } from "@wsp/protocol";
import type { HostFolders } from "@wsp/runtime";
import { FolderSilent, folderReads, type FolderReads } from "./folder-reads.js";
import { under } from "./init-import.js";

export interface HostFolderPaths {
  /** This computer's home folder: the first root, and where a listing starts. */
  home?: string;
  /** Each imported project's own folder, so a project the home folder does not hold is browsable too. */
  projects?: readonly string[];
  /** What this computer runs, for the folders a Mac keeps in every home; absent reads the process's own. */
  platform?: string;
  /** This computer's own window: the whole disk is a root after the home folder. */
  wide?: boolean;
  /** The folders wsp made as workspace copies, which are repos of their own and never a project to add. */
  copies?: readonly string[];
  /** Where the line goes for a folder that did not answer and was skipped. */
  log?: (line: string) => void;
}

/** The project folders the records name, each once: an import lands a folder on the machine at the path it has here,
 * so a record's dest names the folder on this computer as well. */
export function importedProjectFolders(workspaces: readonly WorkspaceView[]): string[] {
  return [...new Set(workspaces.map(w => w.project.path))];
}

/** Runs `fn` over a reader of its own, closed whatever happens. */
async function reading<T>(fn: (reads: FolderReads) => Promise<T>): Promise<T> {
  const reads = folderReads();
  try {
    return await fn(reads);
  } finally {
    reads.close();
  }
}

/** What one path answered, or nothing where it raised or did not answer in time, which the log is told. */
function answered<T>(answer: T | Error, paths: HostFolderPaths): T | undefined {
  if (!(answer instanceof Error)) return answer;
  if (answer instanceof FolderSilent) (paths.log ?? console.warn)(answer.message);
  return undefined;
}

/** What the one path asked answered, raised as the error it read as. */
async function one<T>(answers: Promise<(T | Error)[]>): Promise<T> {
  const [answer] = await answers;
  if (answer instanceof Error) throw answer;
  return answer!;
}

/** The folders every level is browsed from: the home folder, then each imported project folder that is a folder here
 * and that the home folder does not already hold. */
async function rootsOf(paths: HostFolderPaths, reads: FolderReads): Promise<string[]> {
  const home = resolve(paths.home ?? homedir());
  if (paths.wide === true) return [home, "/"];
  const projects = [...new Set((paths.projects ?? []).map(p => resolve(p)).filter(p => !under(p, home)))];
  const facts = await reads.read("folder", projects.map(path => ({ path })));
  return [home, ...projects.filter((_, i) => answered(facts[i]!, paths)?.dir === true)];
}

export async function hostFolderRoots(paths: HostFolderPaths = {}): Promise<string[]> {
  return reading(reads => rootsOf(paths, reads));
}

/** Whether a path is inside a root, the disk's own root included, which no path is under by a separator after it. */
const inside = (path: string, root: string): boolean => root === "/" || under(path, root);

function outside(dir: string, roots: readonly string[]): Error {
  return new Error(foldersOutsideLine(dir, roots.join(", "), "this computer"));
}

/** Which folder a listing is for: absent gives the first root, and so does one inside the roots that is gone, which
 * is what a picker opening on a folder that has since moved should show. Anything outside them is refused, and so is
 * one that does not answer. */
async function folderToList(dir: string | undefined, roots: readonly string[], realRoots: readonly string[], reads: FolderReads): Promise<string> {
  if (dir === undefined || dir === "") return roots[0]!;
  if (!isAbsolute(dir)) throw outside(dir, roots);
  const asked = resolve(dir);
  if (!roots.some(root => inside(asked, root))) throw outside(dir, roots);
  const facts = await one(reads.read("folder", [{ path: asked }]));
  if (!facts.dir) return roots[0]!;
  if (facts.real === null || !realRoots.some(root => inside(facts.real!, root))) throw outside(dir, roots);
  return asked;
}

/** One level: the folders directly inside `dir`, sorted by name, each marked when git tracks it, with the hidden ones
 * counted rather than listed unless `hidden`. The one rule both folder browsers read, told the home it is walking
 * and what this computer is, since the Library a Mac hides is the home's own and not every folder of that name. A
 * folder that exists and cannot be read raises, as does a path outside the roots and one that does not answer. A
 * symlink is a row when it points at a folder the roots hold, so no row leads out of them. A folder a Mac guards is
 * a row that is never looked inside, and a folder in the level that does not answer is left out with a line. */
export async function listHostFolders(req: { dir?: string; hidden?: boolean } = {}, paths: HostFolderPaths = {}): Promise<HostFolderListing> {
  return reading(async reads => {
    const roots = await rootsOf(paths, reads);
    const realRoots = (await reads.read("folder", roots.map(path => ({ path })))).flatMap(a => answered(a, paths)?.real ?? []);
    const dir = await folderToList(req.dir, roots, realRoots, reads);
    const machine = { home: roots[0]!, mac: (paths.platform ?? process.platform) === "darwin" };
    const entries = (await one(reads.read("level", [{ path: dir }]))).filter(e => e.dir || e.link);
    const facts = await reads.read("folder", entries.map(e => ({ path: join(dir, e.name), peek: !guardedFolder(join(dir, e.name), machine) })));
    const rows = entries.flatMap((e, i) => {
      const f = answered(facts[i]!, paths);
      if (f === undefined || !f.dir) return [];
      if (e.link && (f.real === null || !realRoots.some(root => inside(f.real!, root)))) return [];
      return [{ name: e.name, repo: f.repo }];
    });
    rows.sort((a, b) => (a.name < b.name ? -1 : 1));
    const named = rows.filter(row => !hiddenFolder(join(dir, row.name), machine));
    const shown = req.hidden === true ? rows : named;
    const folders: HostFolder[] = shown.map(row => ({ path: join(dir, row.name), repo: row.repo }));
    return { dir, roots, folders, hidden: rows.length - named.length };
  });
}

/** Whether a path is a folder a Mac guards or sits inside one. */
function inGuarded(path: string, machine: { home: string; mac: boolean }): boolean {
  for (let at = path; under(at, machine.home); at = dirname(at)) if (guardedFolder(at, machine)) return true;
  return false;
}

/** Every repo under the home folder and the project roots, most recently written first. Hidden folders, the Mac's
 * Library and the dependency and cache folders are not walked into, nor is a repo, whose own folders are its code,
 * nor a folder a Mac guards, unless a project was added from inside one, which the person typed. A linked worktree,
 * whose .git is a file, is left out: the repo it belongs to is listed at its own checkout. So is a workspace copy wsp
 * made, which is a repo of its own and belongs to the project it was copied from. A folder that does not answer is
 * skipped with a line and the walk goes on. */
export async function listHostRepos(paths: HostFolderPaths = {}): Promise<HostFolderListing> {
  return reading(async reads => {
    const roots = await rootsOf({ ...paths, wide: false }, reads);
    const machine = { home: roots[0]!, mac: (paths.platform ?? process.platform) === "darwin" };
    const typed = [...new Set((paths.projects ?? []).map(p => resolve(p)))].filter(p => inGuarded(p, machine));
    const copies = new Set((paths.copies ?? []).map(p => resolve(p)));
    const found: HostFolder[] = [];
    let level = [...roots, ...typed];
    for (let depth = 0; level.length > 0 && found.length < REPO_CAP; depth++) {
      const steps = await reads.read("walk", level.map(path => ({ path })));
      const next: string[] = [];
      level.forEach((dir, i) => {
        const step = answered(steps[i]!, paths);
        if (step === undefined) return;
        if ("git" in step) {
          if (step.git.dir && !copies.has(dir) && !found.some(f => f.path === dir)) found.push({ path: dir, repo: true, ...(step.git.branch !== undefined ? { branch: step.git.branch } : {}), touchedAt: step.git.touchedAt });
          return;
        }
        if (depth >= REPO_DEPTH) return;
        for (const name of step.dirs) {
          const path = join(dir, name);
          if (!CACHE_DIRS.has(name) && !hiddenFolder(path, machine) && !guardedFolder(path, machine)) next.push(path);
        }
      });
      level = next;
    }
    found.splice(REPO_CAP);
    found.sort((a, b) => (b.touchedAt ?? 0) - (a.touchedAt ?? 0));
    return { dir: roots[0]!, roots, folders: found, hidden: 0 };
  });
}

/** The host's side of host.folders. The records are read on every ask, so a project imported while the app is open is
 * browsable at once, and the home folder is this computer's own. */
export function hostFolders(workspaces: () => Promise<readonly WorkspaceView[]>): HostFolders {
  return {
    list: async req => {
      const all = await workspaces();
      const copies = all.flatMap(w => (w.worktree === undefined ? [] : [w.worktree.path]));
      const paths = { projects: importedProjectFolders(all), wide: req.wide === true, copies };
      return req.repos === true ? listHostRepos(paths) : listHostFolders(req, paths);
    },
  };
}
