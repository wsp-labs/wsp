// SPDX-License-Identifier: AGPL-3.0-only
//! What goes to a sub-thread's machine from a project folder git ignores: the picks, the menu's lines and the
//! refusals of a pick off the menu. Port of the functions of packages/protocol/src/project-seed.ts at a3b870a6e; its
//! shapes and flat constants are wsp_wire::types::project_seed. See spec/project_seed.md.

use wsp_wire::views::workspace::ProjectSeed;
use wsp_wire::views::workspace::SeedChoice;
use wsp_wire::views::workspace::SeedFile;
use wsp_wire::views::workspace::SeedPlan;

use serde::Serialize;

use crate::exit::HostError;

// project-seed.ts:16
// in wsp_wire::types::project_seed: SEED_DIR

// project-seed.ts:17
// in wsp_wire::types::project_seed: SEED_MEMORY_DIR

// project-seed.ts:18
// in wsp_wire::types::project_seed: SEED_PATCH

/// Whether a menu row may leave this computer on this plan's seed: a login never, a file of values or a key only
/// to the person's own box, never to a provider place.
// project-seed.ts:22
// export const seedTravels = (plan: Pick<SeedPlan, "provider">, file: Pick<SeedFile, "kind">): boolean =>
//   file.kind !== "never" && !(file.kind === "secret" && plan.provider === true);
pub fn seed_travels(plan: &SeedPlan, file: &SeedFile) -> bool {
    todo!()
}

/// The paths of a remembered choice this plan holds back rather than drops: the files of values and keys a provider
/// place never gets. Saving picks made off a provider's menu keeps them, so a box's next add still finds them ticked.
// project-seed.ts:27
// export const heldBackPicks = (plan: SeedPlan, remembered: readonly string[]): string[] =>
//   remembered.filter(path => plan.files.some(f => f.path === path && f.kind === "secret" && !seedTravels(plan, f)));
pub fn held_back_picks(plan: &SeedPlan, remembered: &[impl AsRef<str>]) -> Vec<String> {
    todo!()
}

/// What the menu starts with: whatever each row is ticked as. A plan nobody has answered for carries the
/// catalogue's own judgment, every configuration row and nothing else; one built from a choice remembered for that
/// folder carries theirs, so an untick they asked to be kept is still unticked here. A row that does not travel on
/// this plan is never ticked whatever a remembered choice says. The memory folder and the unpushed commits ride on
/// there being any.
// project-seed.ts:35
// export function defaultSeedChoice(plan: SeedPlan): SeedChoice {
//   return {
//     files: plan.files.filter(f => f.ticked && seedTravels(plan, f)).map(f => f.path),
//     memory: plan.memory !== null,
//     commits: plan.unpushed !== null,
//   };
// }
pub fn default_seed_choice(plan: &SeedPlan) -> SeedChoice {
    todo!()
}

/// The choice a person's words make of the menu: the default, then the paths they named to keep and the ones they
/// named to cut, then the two flags that drop the memory folder and the patch.
// project-seed.ts:45
// export function seedChoiceFrom(
//   plan: SeedPlan,
//   keep: readonly string[],
//   cut: readonly string[],
//   flags: { memory?: boolean; commits?: boolean; remember?: boolean } = {},
// ): SeedChoice {
//   const base = defaultSeedChoice(plan);
//   return {
//     files: seedPicks(plan, base.files, keep, cut).files,
//     memory: flags.memory === false ? false : base.memory,
//     commits: flags.commits === false ? false : base.commits,
//     ...(flags.remember === true ? { remember: true } : {}),
//   };
// }
/// The TS flags `{ memory?, commits?, remember? }`; the TS default `{}` is `SeedFlags::default()`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeedFlags {
    pub memory: Option<bool>,
    pub commits: Option<bool>,
    pub remember: Option<bool>,
}

/// The TS throw of `seedPicks` is the Err.
pub fn seed_choice_from(plan: &SeedPlan, keep: &[impl AsRef<str>], cut: &[impl AsRef<str>], flags: &SeedFlags) -> Result<SeedChoice, HostError> {
    todo!()
}

/// Why a path somebody asked to keep is not on the menu: the menu is git's own listing of what it ignores, so a
/// path that is not on it is either tracked already, which means the clone carries it, or not there at all.
// project-seed.ts:62
// export const notInTheMenuLine = (path: string, shown: readonly string[]): string =>
//   `${path} is not on the seed menu; git tracks it or it is not there, and the menu holds ${shown.length === 0 ? "nothing" : shown.join(", ")}`;
pub fn not_in_the_menu_line(path: &str, shown: &[impl AsRef<str>]) -> String {
    todo!()
}

/// Why a login is refused even when it was ticked: it is signed in once per computer and a copy of one in a
/// project would be a second place the person's credential lives.
// project-seed.ts:67
// export const neverTravelsLine = (path: string, row: string | undefined): string =>
//   `${path} is ${row === undefined ? "a login" : `${row}, a login`}, and a login never travels in a seed; the computer's image carries its own sign-in`;
pub fn never_travels_line(path: &str, row: Option<&str>) -> String {
    todo!()
}

/// Why a file of values or a key is refused for a provider place even when it was named: the provider's machine
/// would hold the person's values, and only their own box gets those.
// project-seed.ts:72
// export const staysHereLine = (path: string): string =>
//   `${path} holds values or a key, which stay on this computer for a provider place; only a box of your own gets them`;
pub fn stays_here_line(path: &str) -> String {
    todo!()
}

/// Why the repository's own folder is refused: the computer clones the repo itself, so its history never rides a
/// seed, and a choice naming it is a choice built from something other than the menu.
// project-seed.ts:77
// export const neverGitLine = (path: string): string => `${path} is the repository's own folder, which a seed never carries: the computer clones the repo itself`;
pub fn never_git_line(path: &str) -> String {
    todo!()
}

// What a pick names a path by: the menu's own path, so a folder typed with its slash names its row.
// project-seed.ts:80
// private in the TS: port it as a private helper
// const pickPath = (path: string): string => path.replace(/\/+$/, "");

// project-seed.ts:82
// private in the TS: port it as a private helper
// const invalid = (line: string): Error => Object.assign(new Error(line), { kind: "invalid" });

/// The one rule every road that names menu paths reads them by: the start, plus `keep`, less `cut`, each named as
/// the menu names it. A kept path must be a row of the menu and never a login; a cut path a row or a start path; and
/// neither may be the repository's own folder, which git's listing never names but a choice from elsewhere could.
/// A start path the menu no longer shows is missing rather than refused, so a saved pick whose file went away is
/// named and kept, not dropped.
// project-seed.ts:89
// export function seedPicks(plan: SeedPlan, start: readonly string[], keep: readonly string[], cut: readonly string[]): { files: string[]; missing: string[] } {
//   const keeps = keep.map(pickPath);
//   const cuts = cut.map(pickPath);
//   const known = new Map(plan.files.map(f => [f.path, f]));
//   const missing = start.filter(path => !known.has(path));
//   for (const path of [...keeps, ...cuts]) if (path === ".git" || path.startsWith(".git/")) throw invalid(neverGitLine(path));
//   for (const path of keeps) {
//     const file = known.get(path);
//     if (file === undefined) throw invalid(notInTheMenuLine(path, [...known.keys()]));
//     if (file.kind === "never") throw invalid(neverTravelsLine(path, file.row?.name));
//     if (!seedTravels(plan, file)) throw invalid(staysHereLine(path));
//   }
//   for (const path of cuts) if (!known.has(path) && !missing.includes(path)) throw invalid(notInTheMenuLine(path, [...known.keys()]));
//   return {
//     files: plan.files.filter(f => seedTravels(plan, f) && (start.includes(f.path) || keeps.includes(f.path)) && !cuts.includes(f.path)).map(f => f.path),
//     missing: missing.filter(path => !cuts.includes(path)),
//   };
// }
/// The TS `{ files, missing }`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SeedPicks {
    pub files: Vec<String>,
    pub missing: Vec<String>,
}

/// The TS throws (an invalid refusal) are the Err.
pub fn seed_picks(plan: &SeedPlan, start: &[impl AsRef<str>], keep: &[impl AsRef<str>], cut: &[impl AsRef<str>]) -> Result<SeedPicks, HostError> {
    todo!()
}

/// A project's seed picks as every door answers them: each row of the menu marked picked or not.
// project-seed.ts:109
// export function projectSeedOf(project: string, plan: SeedPlan, picks: { files: readonly string[]; missing: readonly string[] }, saved: boolean): ProjectSeed {
//   const picked = plan.files.filter(f => picks.files.includes(f.path));
//   return {
//     project,
//     folder: plan.source,
//     rows: plan.files.map(f => ({ path: f.path, dir: f.dir, bytes: f.bytes, kind: f.kind, why: seedRowWords(f, plan), picked: picks.files.includes(f.path) })),
//     picked: { paths: picked.map(f => f.path), bytes: picked.reduce((sum, f) => sum + f.bytes, 0), missing: [...picks.missing] },
//     saved,
//   };
// }
pub fn project_seed_of(project: &str, plan: &SeedPlan, picks: &SeedPicks, saved: bool) -> ProjectSeed {
    todo!()
}

/// The picks as a terminal reads them: the head line, the rows as cells for the caller's table, picked first and
/// then the widest, and the lines under them: the saved picks no longer there, and whether anything was saved.
// project-seed.ts:122
// export function projectSeedLines(seed: ProjectSeed): { head: string; rows: string[][]; foot: string[] } {
//   const rows = [...seed.rows].sort((a, b) => Number(b.picked) - Number(a.picked) || b.bytes - a.bytes || a.path.localeCompare(b.path));
//   const count = `${seed.picked.paths.length} picked${seed.picked.paths.length === 0 ? "" : `, ${fmtBytes(seed.picked.bytes)}`}.`;
//   return {
//     head: `${seed.project}, ${seed.folder}: what git ignores, and what goes to a sub-thread's machine.`,
//     rows: rows.length === 0 ? [] : [["", "PATH", "SIZE", "WHY"], ...rows.map(r => [r.picked ? "x" : " ", r.dir ? `${r.path}/` : r.path, fmtBytes(r.bytes), r.why])],
//     foot: [
//       ...(seed.picked.missing.length === 0 ? [] : [`Picked but not in the folder now, so a sub-thread starts without: ${seed.picked.missing.join(", ")}.`]),
//       seed.saved ? `${count} Saved: every later sub-thread of ${seed.project} gets these.` : `${count} Nothing was saved; add --yes to save these picks.`,
//     ],
//   };
// }
/// The TS `{ head, rows, foot }`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SeedLines {
    pub head: String,
    pub rows: Vec<Vec<String>>,
    pub foot: Vec<String>,
}

pub fn project_seed_lines(seed: &ProjectSeed) -> SeedLines {
    todo!()
}

/// Why a thread's own token may read a project's seed picks and not save them, with the line that saves the same
/// picks for the person to run.
// project-seed.ts:137
// export const seedSaveRefusal = (project: string, o: { keep?: readonly string[]; cut?: readonly string[]; none?: boolean }): Error =>
//   refusal(
//     "picking the files that leave this computer is the person's act",
//     `Hand them the line ${shellLine(["wsp", "projects", "seed", project, ...(o.none === true ? ["--none"] : []), ...(o.keep ?? []).flatMap(p => ["--keep", p]), ...(o.cut ?? []).flatMap(p => ["--cut", p]), "--yes"])}`,
//     "usage",
//   );
/// The TS `{ keep?, cut?, none? }`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeedAsked {
    pub keep: Option<Vec<String>>,
    pub cut: Option<Vec<String>>,
    pub none: Option<bool>,
}

pub fn seed_save_refusal(project: &str, o: &SeedAsked) -> HostError {
    todo!()
}

/// Why a project on another computer has no seed picks: the menu is read off a folder on this computer alone.
// project-seed.ts:145
// export const seedNotHereRefusal = (project: string, computer: string): Error =>
//   refusal(`${project} lives on ${computer}, and picks are read from folders on this computer only`, `A sub-thread of ${project} starts with no git-ignored files.`, "usage");
pub fn seed_not_here_refusal(project: &str, computer: &str) -> HostError {
    todo!()
}

/// The words one menu row ends in, which say why it is ticked or not.
// project-seed.ts:149
// export function seedRowWords(file: SeedFile, plan: Pick<SeedPlan, "provider"> = {}): string {
//   if (file.kind === "secret") return seedTravels(plan, file) ? (file.row?.name ?? "values") : "stays on this computer";
//   if (file.kind === "config") return file.row?.name ?? "config";
//   if (file.kind === "rebuilt") return "rebuilt on the box";
//   if (file.kind === "data") return "database, stop what uses it first";
//   if (file.kind === "never") return "never travels";
//   return "not in the catalogue";
// }
pub fn seed_row_words(file: &SeedFile, plan: Option<&SeedPlan>) -> String {
    todo!()
}

/// The bytes a seed's ticked files come to, which is the number the menu shows and the number the record keeps:
/// the files the person ticked and nothing else, so the row they read and the record they can read back later say
/// one thing. The archive those files travel in is bigger, and its size is nobody's question.
// project-seed.ts:161
// export function seedBytes(plan: SeedPlan, choice: SeedChoice): number {
//   return plan.files.filter(f => choice.files.includes(f.path)).reduce((sum, f) => sum + f.bytes, 0);
// }
pub fn seed_bytes(plan: &SeedPlan, choice: &SeedChoice) -> f64 {
    todo!()
}

/// What the seeding says as the person's files go to that computer: the same counts the menu showed them, in the
/// same order, and the folder they came from. The memory folder is counted on a row of its own, as it is on the
/// menu, so a seed of nothing but memory reads as nought files here too.
// project-seed.ts:168
// export function seedingLine(plan: SeedPlan, choice: SeedChoice): string {
//   const files = plan.files.filter(f => choice.files.includes(f.path)).length;
//   const parts = [files === 0 ? "0 files" : `${plural(files, "file")} (${fmtBytes(seedBytes(plan, choice))})`];
//   if (choice.memory && plan.memory !== null) parts.push(`Claude Code memory (${plural(plan.memory.files, "file")}, ${fmtBytes(plan.memory.bytes)})`);
//   if (choice.commits && plan.unpushed !== null) parts.push(`${plural(plan.unpushed.commits, "commit")} the remote does not have`);
//   return `Seeding ${parts.join(", ")}, from ${plan.source}.`;
// }
pub fn seeding_line(plan: &SeedPlan, choice: &SeedChoice) -> String {
    todo!()
}

/// What the seeding says once the computer's own git has answered, where the seed carried commits at all: the
/// count read back off the checkout there, which is nought where that clone already had the person's work. A
/// patch git refused says so in its own sentence and is not said twice.
// project-seed.ts:179
// export const seedCommitsLandedLine = (commits: number): string => (commits === 0 ? "no commits to land" : `${plural(commits, "commit")} landed`);
pub fn seed_commits_landed_line(commits: f64) -> String {
    todo!()
}

/// What the person is told a seed would carry, before anything leaves this computer: the ticked paths with their
/// bytes, the memory folder, the patch, and the edits that stay here. One line each, in that order.
// project-seed.ts:183
// export function seedSummaryLines(plan: SeedPlan, choice: SeedChoice): string[] {
//   const ticked = plan.files.filter(f => choice.files.includes(f.path));
//   const bytes = seedBytes(plan, choice);
//   const lines = [ticked.length === 0 ? "no files travel" : `${plural(ticked.length, "file")}, ${fmtBytes(bytes)}: ${ticked.map(f => f.path).join(", ")}`];
//   if (choice.memory && plan.memory !== null) lines.push(`Claude Code memory, ${plural(plan.memory.files, "file")}, ${fmtBytes(plan.memory.bytes)}`);
//   if (choice.commits && plan.unpushed !== null) lines.push(`${plural(plan.unpushed.commits, "commit")} the remote does not have, as a patch from ${plan.unpushed.base.slice(0, 7)}`);
//   if (plan.uncommitted > 0) lines.push(`${plural(plan.uncommitted, "uncommitted change")} stay on this computer`);
//   return lines;
// }
pub fn seed_summary_lines(plan: &SeedPlan, choice: &SeedChoice) -> Vec<String> {
    todo!()
}

/// The menu as a terminal draws it: one row per path git ignores, ticked or not, with its size and the words its
/// catalog row ends in, then the memory folder, the unpushed commits and what stays here. The rows are cells for
/// the caller's own table, so the command line and the app pad them the way they pad every other table.
/// The rows are widest first: a package tree of three gigabytes is the thing a person looks for.
// project-seed.ts:197
// export function seedMenuRows(plan: SeedPlan, choice: SeedChoice): string[][] {
//   const tick = (on: boolean): string => (on ? "x" : " ");
//   const files = [...plan.files].sort((a, b) => b.bytes - a.bytes || a.path.localeCompare(b.path));
//   return [
//     ["", "PATH", "SIZE", "WHY"],
//     ...files.map(f => [`[${tick(choice.files.includes(f.path))}]`, f.dir ? `${f.path}/` : f.path, fmtBytes(f.bytes), seedRowWords(f, plan)]),
//     ...(plan.memory === null ? [] : [[`[${tick(choice.memory)}]`, `${plan.memory.key}/memory`, fmtBytes(plan.memory.bytes), `Claude Code memory, ${plural(plan.memory.files, "file")}`]]),
//     ...(plan.unpushed === null ? [] : [[`[${tick(choice.commits)}]`, `${plan.branch} as a patch`, "", `${plural(plan.unpushed.commits, "commit")} the remote does not have`]]),
//   ];
// }
pub fn seed_menu_rows(plan: &SeedPlan, choice: &SeedChoice) -> Vec<Vec<String>> {
    todo!()
}

/// What the command line says under the menu when nobody has consented yet: nothing was sent, and the two lines
/// that send it. Written here so the app's own dialog says the same about the same menu.
// project-seed.ts:210
// export function seedConsentLines(plan: SeedPlan, computer: string, addLine: (flags: string) => string): string[] {
//   return [
//     `nothing was sent. ${plan.uncommitted > 0 ? `${plural(plan.uncommitted, "uncommitted change")} stay on this computer, and ` : ""}${computer} clones ${plan.remote ?? "the repo"} itself.`,
//     `to send the ticked rows:   ${addLine("--yes")}`,
//     `to change what travels:    ${addLine("--yes --keep <path> --cut <path> --no-memory --no-commits")}`,
//   ];
// }
pub fn seed_consent_lines(plan: &SeedPlan, computer: &str, add_line: &dyn Fn(&str) -> String) -> Vec<String> {
    todo!()
}

/// What the add says about the paths a ticked folder held that do not travel: a login, or a key bound for a
/// provider place, inside a folder somebody ticked stays on this computer, and the line names each one, so nothing
/// they asked to carry is dropped in silence.
// project-seed.ts:221
// export const leftBehindLine = (paths: readonly string[]): string =>
//   `${plural(paths.length, "file")} inside the folders you ticked stayed on this computer: ${paths.join(", ")}`;
pub fn left_behind_line(paths: &[impl AsRef<str>]) -> String {
    todo!()
}

/// What the add says when git refused the commits the remote has never seen: the person's own count and branch,
/// git's own last line for why, and where the checkout every copy of it takes now stands. The commits stay on the
/// person's computer, so this is a notice on an add that landed rather than a refusal.
// project-seed.ts:227
// export const seedCommitsLostLine = (commits: number, branch: string, computer: string, said: string, back: string): string =>
//   `the ${plural(commits, "commit")} on ${branch} did not land on ${computer}: ${said}; the checkout is on ${back}`;
pub fn seed_commits_lost_line(commits: f64, branch: &str, computer: &str, said: &str, back: &str) -> String {
    todo!()
}

/// Why a folder with no remote records nothing: the computer clones the repo, and a folder git has no remote for
/// gives it nothing to clone.
// project-seed.ts:232
// export const noRemoteLine = (folder: string): string =>
//   `${folder} has no origin remote, and the computer clones the repo rather than copying the folder; push it somewhere and add it again`;
pub fn no_remote_line(folder: &str) -> String {
    todo!()
}

