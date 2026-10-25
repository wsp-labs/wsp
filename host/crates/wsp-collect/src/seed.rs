// SPDX-License-Identifier: AGPL-3.0-only
//! The seed menu for one folder on this computer: what a project added from it would carry, read off git's own
//! listing of what it ignores, one size pass and the agent's memory folder. Pure over the Host seam; no file's content
//! is read and nothing leaves the computer. Port of packages/collect/src/seed.ts at a3b870a6e; the spec is spec/seed.md.
use wsp_wire::views::workspace::{SeedFile, SeedPlan, SeedPlanUnpushed};

use crate::host::Host;

/// How long one git or du call gets, in ms (seed.ts:13).
pub const CALL_MS: u64 = 120_000;

/// The TS options object `{ claudeStateHome }`.
#[derive(Debug, Clone)]
pub struct SeedOptions {
    pub claude_state_home: String,
}

/// Every path git ignores in the folder, collapsed to the directory where a whole directory is ignored, with the rows
/// the catalog opens broken out, each sized once, plus the branch, the remote, the unpushed commits, the edits that
/// stay here and the agent's memory folder for this folder. `provider` is absent and `remembered` false.
// seed.ts:15
// /** What a seed reads the folder with, so a test hands in the same shape a real one gets. */
// interface Git {
//   (...args: string[]): Promise<string | undefined>;
// }
//
// /** Every path git ignores in the folder, collapsed to the directory where a whole directory is ignored, with the
//  * rows the catalog opens broken out, each sized once, plus the branch, the remote, the unpushed commits, the edits
//  * that stay here and the agent's memory folder for this folder. */
// export async function seedMenu(host: Host, folder: string, o: { claudeStateHome: string }): Promise<SeedPlan> {
//   const git: Git = (...args) => host.exec.run("git", ["-C", folder, ...args], { timeoutMs: CALL_MS });
//   const listed = zsplit(await git("ls-files", "-z", "-o", "-i", "--exclude-standard", "--directory"));
//   const opened = new Map<string, string[]>();
//   for (const entry of listed) {
//     if (!isDir(entry) || !opens(entry)) continue;
//     // Inside a directory the catalog opens, so a config file the person needs is its own row rather than a byte of
//     // a folder nobody ticks. Only the paths a row names are broken out; the rest stays counted in the folder.
//     const inside = zsplit(await git("ls-files", "-z", "-o", "-i", "--exclude-standard", "--", trim(entry)));
//     opened.set(entry, inside.filter(path => seedRowFor(path, false) !== undefined));
//   }
//   const measured = [...listed, ...[...opened.values()].flat()];
//   const sizes = await duBytes(host, folder, measured);
//   const files: SeedFile[] = [];
//   const keep = (row: SeedFile | undefined): void => {
//     if (row !== undefined) files.push(row);
//   };
//   for (const entry of listed) {
//     const path = trim(entry);
//     const broken = opened.get(entry) ?? [];
//     for (const inner of broken) keep(rowFor(inner, false, sizes.get(inner) ?? 0));
//     // What is left of an opened folder once the rows broken out of it are counted on their own.
//     const own = (sizes.get(path) ?? 0) - broken.reduce((sum, inner) => sum + (sizes.get(inner) ?? 0), 0);
//     keep(rowFor(path, isDir(entry), Math.max(own, 0)));
//   }
//   const shown = files.sort((a, b) => a.path.localeCompare(b.path));
//   const remote = oneLine(await git("remote", "get-url", "origin"));
//   const branch = oneLine(await git("rev-parse", "--abbrev-ref", "HEAD")) ?? "";
//   const head = oneLine(await git("symbolic-ref", "refs/remotes/origin/HEAD"));
//   const defaultBranch = head === undefined ? null : head.replace(/^refs\/remotes\/origin\//, "");
//   return {
//     source: folder,
//     remote: remote ?? null,
//     branch,
//     defaultBranch,
//     unpushed: await unpushed(git, defaultBranch),
//     uncommitted: lines(await git("status", "--porcelain")).length,
//     memory: await memoryRow(host, folder, o.claudeStateHome),
//     files: shown,
//     remembered: false,
//   };
// }
pub async fn seed_menu(host: &Host, folder: &str, o: &SeedOptions) -> SeedPlan {
    todo!()
}

/// The commits on this branch the remote does not have, against the upstream or else origin's default branch.
// seed.ts:66
// /** The commits on this branch the remote does not have, and the commit they start from: the upstream branch when
//  * the branch has one, else the remote's own default branch. None where nothing is ahead, and none where neither
//  * ref is there, which is a branch the remote has never seen. */
// async function unpushed(git: Git, defaultBranch: string | null): Promise<SeedPlan["unpushed"]> {
//   const upstream = oneLine(await git("rev-parse", "--abbrev-ref", "@{upstream}"));
//   const against = upstream ?? (defaultBranch === null ? undefined : `origin/${defaultBranch}`);
//   if (against === undefined) return null;
//   const base = oneLine(await git("merge-base", "HEAD", against));
//   if (base === undefined) return null;
//   const count = Number(oneLine(await git("rev-list", "--count", `${base}..HEAD`)) ?? "0");
//   return Number.isInteger(count) && count > 0 ? { commits: count, base } : null;
// }
pub async fn unpushed(host: &Host, folder: &str, default_branch: Option<&str>) -> Option<SeedPlanUnpushed> {
    todo!()
}

/// One row of the menu for a path, or `None` where the catalog calls it junk; ticked for config and secret kinds.
// seed.ts:90
// /** One row of the menu for a path, or nothing where the catalog hides it: the row and kind that judged it, its
//  * size, and whether it starts ticked, which configuration and files of values do; the host unticks the latter for a
//  * provider place. */
// function rowFor(path: string, dir: boolean, bytes: number): SeedFile | undefined {
//   const found = seedRowFor(path, dir);
//   if (found?.kind === "junk") return undefined;
//   const kind: SeedKind = found === undefined ? "unknown" : found.kind;
//   return { path, dir, bytes, kind, ...(found === undefined ? {} : { row: { id: found.row.id, name: found.row.name } }), ticked: kind === "config" || kind === "secret" };
// }
pub fn row_for(path: &str, dir: bool, bytes: i64) -> Option<SeedFile> {
    todo!()
}

/// Whether any catalog row opens this directory entry (`name/` as git lists it).
// seed.ts:120
// /** Whether any catalog row opens this directory: the listing is collapsed to the directory, and a row that names a
//  * file inside one asks for that directory to be listed again. */
// function opens(entry: string): boolean {
//   return SEED_ROWS.some(row => (row.opens ?? []).some(name => entry === name || entry.endsWith(`/${name}`)));
// }
pub fn opens(entry: &str) -> bool {
    todo!()
}

/// du's `<kibibytes>\t<path>` lines read back against the folder: relative path to bytes (kb × 1024). A line with no
/// tab, a size that is not a number, or a path outside `<folder>/` is skipped.
// seed.ts:100
// /** The bytes every listed path holds, in one pass: du reports kibibytes per argument, so the whole menu is sized
//  * by one child rather than one per row. A path du could not read is absent and its row reads zero. */
// async function duBytes(host: Host, folder: string, paths: readonly string[]): Promise<Map<string, number>> {
//   const bytes = new Map<string, number>();
//   const relative = paths.map(trim);
//   if (relative.length === 0) return bytes;
//   // Absolute, since a child of this process starts in the collector's own folder and not in the project's: du is
//   // handed the whole path and its answer is read back against the folder it was asked about.
//   const at = `${folder.replace(/\/+$/, "")}/`;
//   const out = await host.exec.run("du", ["-sk", ...relative.map(path => `${at}${path}`)], { timeoutMs: CALL_MS });
//   for (const line of lines(out)) {
//     const tab = line.indexOf("\t");
//     if (tab < 0) continue;
//     const kb = Number(line.slice(0, tab));
//     const path = trim(line.slice(tab + 1));
//     if (Number.isFinite(kb) && path.startsWith(at)) bytes.set(path.slice(at.length), kb * 1024);
//   }
//   return bytes;
// }
pub fn du_sizes(out: Option<&str>, folder: &str) -> indexmap::IndexMap<String, i64> {
    todo!()
}

// seed.ts:126
// const isDir = (entry: string): boolean => entry.endsWith("/");
// const trim = (path: string): string => path.replace(/\/+$/, "");
// const lines = (out: string | undefined): string[] => (out ?? "").split("\n").filter(line => line.trim() !== "");
// const zsplit = (out: string | undefined): string[] => (out ?? "").split("\0").filter(word => word !== "");
// const oneLine = (out: string | undefined): string | undefined => {
//   const line = (out ?? "").split("\n")[0]?.trim();
//   return line === undefined || line === "" ? undefined : line;
// };
pub fn one_line(out: Option<&str>) -> Option<String> {
    todo!()
}
