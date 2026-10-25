// SPDX-License-Identifier: AGPL-3.0-only
//! Unit import_roads (spec/golden-import_roads.md): golden-import.ts:20-52 (the row) and 641-1081 at a4033dc34. What
//! a recipe row installs by and what a step reads off its road: the PATH line, a road's script, a row's road, the
//! Brewfile, the rows outside the catalog.
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_catalog::catalog::{Below as CatalogBelow, ToolEntry};
use wsp_catalog::pending::ToolPin;
use wsp_catalog::roads::{InstallRoad, RoadName};
use wsp_wire::types::init_job::LoginChoice;
use wsp_wire::views::golden_image::RecipeCustomRow;

use super::{PinRead, SkippedItem, ToolInstall};

/// golden-import.ts:22 `RecipeEntry`: the collector's manifest row with the person's tick, as a recipe file carries it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeEntry {
    pub rung: String,
    pub id: String,
    pub label: String,
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excludes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volatile: Option<Vec<String>>,
    pub bytes: u64,
    /// `bring` or `skip`.
    pub default: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bring: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<LoginChoice>,
    /// `yes`, `no` or `unknown`: whether a formula has a Linux bottle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<ToolPin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

/// golden-import.ts:777 `ToolSource`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSource {
    pub repo: String,
    pub tag: String,
}

/// golden-import.ts:783 `BrewFormula`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrewFormula {
    pub name: String,
    pub full_name: String,
    pub deps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub macos_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ToolSource>,
}

/// golden-import.ts:798 `BrewTable`, by full name.
pub type BrewTable = IndexMap<String, BrewFormula>;

/// golden-import.ts:742 `BaseRow`.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseRow {
    pub id: String,
    pub name: String,
    pub entry: &'static ToolEntry,
    pub version: Option<String>,
}

/// golden-import.ts:752 `PlannedRoad`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedRoad {
    pub id: String,
    pub name: String,
    pub source: ToolSource,
    pub pin: Option<ToolPin>,
}

/// golden-import.ts:731 `Brewfile`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Brewfile {
    pub text: String,
    pub taps: Vec<String>,
    pub formulae: Vec<String>,
    pub skipped: Vec<SkippedItem>,
    pub roads: Vec<PlannedRoad>,
    pub base: Vec<BaseRow>,
}

/// golden-import.ts:986 `PlannedRow`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedRow {
    pub road: InstallRoad,
    pub bin: Option<String>,
    pub pinned: Option<String>,
    pub below: Option<CatalogBelow>,
    pub after: Option<String>,
    pub note: Option<String>,
}

/// golden-import.ts:704 and 714: what a step reads off its road. `check` is set only by `road_step_reads`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoadReads {
    pub present: Option<String>,
    pub bins: Option<Vec<String>>,
    pub check: Option<String>,
}

/// golden-import.ts:761.
pub const CATALOG_PREFIX: &str = "tools/catalog/";
/// golden-import.ts:763.
pub const UNMEASURED_ROAD: &str = "by an unmeasured road";
/// golden-import.ts:902.
pub const MANAGER_STEP: &str = "tools/manager/";
/// golden-import.ts:1018.
pub const CUSTOM_PREFIX: &str = "tools/custom/";
/// golden-import.ts:801, in the TS's order.
pub const MACOS_ONLY_FORMULAE: [&str; 4] =
    ["felixkratz/formulae/sketchybar", "felixkratz/formulae/borders", "koekeishiya/formulae/yabai", "koekeishiya/formulae/skhd"];

// TS golden-import.ts:689-699
// /** The version a road's install line asks for, for a presence read to compare against: the road's own where the
//  * road fixes one, nothing where it installs whatever its source serves that day. */
// export function asksVersion(road: InstallRoad): string | undefined {
//   return fixesVersion(road) ? versionOf(road) : undefined;
// }
//
// /** A step's `asks` as a field, so a caller spreads it rather than reading the rule twice. */
// const asksOf = (road: InstallRoad): { asks?: string } => {
//   const asks = asksVersion(road);
//   return asks === undefined ? {} : { asks };
// };
/// golden-import.ts:691 `asksVersion`: the road's version where the road fixes one.
pub fn asks_version(road: &InstallRoad) -> Option<String> {
    todo!()
}

// TS golden-import.ts:701-709
// /** What a step carries off its road for reading it back later: the road's own presence test where it has one, and
//  * the directories that road links its commands into. One reader, so every step planned on a road answers the same
//  * two questions however it was planned. */
// export function roadReads(road: InstallRoad, bin: string, prefix?: string): { present?: string; bins?: readonly string[] } {
//   const mod = roadModule(road);
//   const present = mod.present?.(road, bin);
//   const bins = mod.bins(road, installHomes(prefix));
//   return { ...(present !== undefined ? { present } : {}), ...(bins.length > 0 ? { bins } : {}) };
// }
/// golden-import.ts:704 `roadReads`.
pub fn road_reads(road: &InstallRoad, bin: &str, prefix: Option<&str>) -> RoadReads {
    todo!()
}

// TS golden-import.ts:711-717
// /** What a step reads off its road: `roadReads`, and the road's own check where the road has one of its own. Read
//  * by the steps whose reads are the road's word and nothing else, a formula the plan installs and a row outside the
//  * catalog whose manager names a road, so both answer the same questions however the row reached the plan. */
// export function roadStepReads(road: InstallRoad, bin: string, prefix?: string): { present?: string; bins?: readonly string[]; check?: string } {
//   const check = roadModule(road).check?.(road, bin);
//   return { ...roadReads(road, bin, prefix), ...(check !== undefined ? { check } : {}) };
// }
/// golden-import.ts:714 `roadStepReads`.
pub fn road_step_reads(road: &InstallRoad, bin: &str, prefix: Option<&str>) -> RoadReads {
    todo!()
}

// TS golden-import.ts:719-724
// /** The pin read a road gives a step: its module's version line on the command it puts on PATH, whether it fixes one, and its words. */
// export function pinReadOf(road: InstallRoad, bin: string, prefix?: string): PinRead {
//   const mod = roadModule(road);
//   const read = mod.installed?.(road, bin, installHomes(prefix));
//   return { ...(read !== undefined ? { read } : {}), fixed: fixesVersion(road), words: mod.words };
// }
/// golden-import.ts:720 `pinReadOf`.
pub fn pin_read_of(road: &InstallRoad, bin: &str, prefix: Option<&str>) -> PinRead {
    todo!()
}

// TS golden-import.ts:765-774
// /** The base row a tools row stands for, when the base stage installs the same tool on every golden. The version
//  * read here is the row's, or the brew table's for a formula. */
// export function baseRowFor(e: RecipeEntry, brew: BrewTable = new Map()): BaseRow | undefined {
//   if (e.rung !== "tools") return undefined;
//   const pkg = packageOf(e);
//   const entry = baseEntryFor(pkg);
//   if (entry === undefined) return undefined;
//   const version = e.version ?? brew.get(pkg)?.version;
//   return { id: e.id, name: entry.name, entry, ...(version !== undefined ? { version } : {}) };
// }
/// golden-import.ts:767 `baseRowFor`.
pub fn base_row_for(e: &RecipeEntry, brew: &BrewTable) -> Option<BaseRow> {
    todo!()
}

/// golden-import.ts:858 `isTap`: the id is under `tools/brew-tap/`.
pub fn is_tap(id: &str) -> bool {
    todo!()
}

/// golden-import.ts:862 `formulaOf`: the id after `tools/brew/`.
pub fn formula_of(id: &str) -> Option<String> {
    todo!()
}

// TS golden-import.ts:856-899
// const TAP_PREFIX = "tools/brew-tap/";
// /** A tap row: the tap it names, not a formula or a command. */
// export const isTap = (e: Pick<RecipeEntry, "id">): boolean => e.id.startsWith(TAP_PREFIX);
// const tapOf = (e: Pick<RecipeEntry, "id">): string => e.id.slice(TAP_PREFIX.length);
//
// /** The formula a Homebrew row names; nothing for a tap or any other row. */
// export const formulaOf = (e: Pick<RecipeEntry, "id">): string | undefined => (e.id.startsWith(BREW_ID_PREFIX) ? e.id.slice(BREW_ID_PREFIX.length) : undefined);
// const macOsOnly = (formula: string, info: BrewFormula | undefined): boolean => MACOS_ONLY_FORMULAE.has(formula) || info?.macosOnly === true;
//
// /** A tap formula with no Linux bottle whose GitHub release the Mac's Homebrew names installs from that release, with
//  * the pin its first install recorded. Only a tap formula takes the road: a core formula unknown to the snapshot may
//  * well have a Linux bottle by now. The one place that decides it, read by the Brewfile, the plan and the digest. */
// function releaseFor(e: RecipeEntry, brew: BrewTable): PlannedRoad | undefined {
//   const formula = formulaOf(e);
//   const info = formula === undefined ? undefined : brew.get(formula);
//   if (formula === undefined || e.linux !== "unknown" || !formula.includes("/") || info?.source === undefined || macOsOnly(formula, info)) return undefined;
//   return { id: e.id, name: info.name, source: info.source, ...pinOf(e) };
// }
//
// export function brewfileFor(entries: readonly RecipeEntry[], brew: BrewTable = new Map()): Brewfile {
//   const out: Brewfile = { text: "", taps: [], formulae: [], skipped: [], roads: [], base: [] };
//   for (const e of entries) {
//     if (!ticked(e) || e.rung !== "tools") continue;
//     const base = baseRowFor(e, brew);
//     const formula = formulaOf(e);
//     if (base !== undefined) {
//       out.base.push(base);
//     } else if (isTap(e)) {
//       out.taps.push(tapOf(e));
//     } else if (formula !== undefined) {
//       // A formula the catalog brings as a manager's toolchain takes the catalog's road, so it is no formula here.
//       if (catalogToolOf(e) !== undefined) continue;
//       const road = releaseFor(e, brew);
//       if (e.linux === "no") out.skipped.push({ id: e.id, note: "no Linux bottle" });
//       else if (e.linux === "unknown" && macOsOnly(formula, brew.get(formula))) out.skipped.push({ id: e.id, note: "macOS only" });
//       else if (road !== undefined) out.roads.push(road);
//       else if (e.linux === "unknown") out.skipped.push({ id: e.id, note: "no Linux bottle known" });
//       else out.formulae.push(formula);
//     }
//   }
//   const lines = [...out.taps.map(t => `tap "${t}"`), ...out.formulae.map(f => `brew "${f}"`)];
//   out.text = lines.length > 0 ? `${lines.join("\n")}\n` : "";
//   return out;
// }
/// golden-import.ts:875 `brewfileFor`.
pub fn brewfile_for(entries: &[RecipeEntry], brew: &BrewTable) -> Brewfile {
    todo!()
}

// TS golden-import.ts:904-935
// /** The manager whose own toolchain an install runs on top of, when what it waits on names one: npm's node.
//  * Nothing for an install that waits on the apt index, on Homebrew, or on a row the floor carries. */
// function managerBehind(dep: string | undefined): RoadName | undefined {
//   return dep === undefined ? undefined : MANAGER_ORDER.find(m => catalogToolFor(m)?.id === dep);
// }
//
// /** What a planned row waits on: the road the row takes where it names one of its own, else what that road's module
//  * runs on top of. The one reading, so the loop that resolves a manager and the one that plans the row cannot part. */
// const depOf = (planned: PlannedRow): string | undefined => planned.after ?? ROAD_MODULES[planned.road.road].after;
//
// /** The manager rows the collector writes (`tools/<manager>/<package>`), in the order their steps run. */
// const MANAGER_ORDER: readonly ("npm" | "pnpm" | "bun" | "uv" | "pipx" | "cargo" | "go")[] = ["npm", "pnpm", "bun", "uv", "pipx", "cargo", "go"];
//
// /** The catalog tool this Mac's formula stands for, when a manager comes by that tool and the catalog brings it some
//  * other way: cargo's toolchain is rustup's whatever the Brewfile names, so the row takes the catalog's road and
//  * Homebrew's formula never installs beside it. */
// function managerTool(formula: string): ToolEntry | undefined {
//   const entry = catalogToolFor(formula);
//   if (entry === undefined || entry.installRoad.road === "brew") return undefined;
//   return MANAGER_ORDER.some(m => catalogToolFor(m)?.id === entry.id) ? entry : undefined;
// }
//
// /** The catalog tool a tools row is: a catalog row by the id after its prefix, this Mac's formula for a manager's
//  * toolchain by the formula; nothing for any other row. */
// export function catalogToolOf(e: RecipeEntry): ToolEntry | undefined {
//   const pkg = packageOf(e);
//   if (e.id.startsWith(CATALOG_PREFIX)) {
//     const entry = catalogEntry(pkg);
//     return entry?.kind === "tool" ? entry : undefined;
//   }
//   return e.id.startsWith(BREW_ID_PREFIX) ? managerTool(pkg) : undefined;
// }
/// golden-import.ts:928 `catalogToolOf`.
pub fn catalog_tool_of(e: &RecipeEntry) -> Option<&'static ToolEntry> {
    todo!()
}

// TS golden-import.ts:937-945
// /** The formula a manager with no catalog row comes by. */
// const MANAGER_FORMULA: Readonly<Partial<Record<RoadName, string>>> = { pipx: "pipx" };
//
// /** The Homebrew formula a manager comes by: its catalog row's, when that row takes the brew road, else the one named
//  * here; nothing for a manager the catalog installs another way or the base carries. */
// export function managerFormula(manager: RoadName): string | undefined {
//   const road = catalogToolFor(manager)?.installRoad;
//   return road === undefined ? MANAGER_FORMULA[manager] : road.road === "brew" ? road.formula : undefined;
// }
/// golden-import.ts:942 `managerFormula`.
pub fn manager_formula(manager: RoadName) -> Option<String> {
    todo!()
}

// TS golden-import.ts:947-996
// const pinOf = (e: { pin?: ToolPin }): { pin?: ToolPin } => (e.pin !== undefined ? { pin: e.pin } : {});
//
// /** The release road of a tap row whose GitHub release is known, with the pin its first install recorded and the
//  * repository's main package for `go install` to fall back to. */
// const releaseRoad = (r: Pick<PlannedRoad, "source" | "pin">): InstallRoad => ({ road: "release", repo: r.source.repo, version: r.source.tag, ...pinOf(r), go: `github.com/${r.source.repo}@${r.source.tag}` });
//
// /** What a tools row installs by, with the command it puts on PATH where known: a catalog row its entry's road at the
//  * row's version where the road pins one (noted when no golden build has proven the road, or when the version could not
//  * be pinned), a tap formula with no Linux bottle its GitHub release when this Mac's Homebrew (`brew`) names one, any
//  * other formula row the brew road, a manager row its manager's road at the row's version; a package the catalog pins
//  * never below its pin (`atCatalogPin`). A road carries its first install's pin. The plan, digest and Behind read this. */
// export function rowRoad(e: RecipeEntry, brew: BrewTable): PlannedRow | undefined {
//   const pkg = packageOf(e);
//   const known = catalogToolOf(e);
//   if (known !== undefined) {
//     const mod = roadModule(known.installRoad);
//     const at = atCatalogPin(e.version !== undefined && mod.at !== undefined ? mod.at(known.installRoad, e.version) : known.installRoad, e.version);
//     // A road that takes no version installs the catalog's pinned release, or the source's current one, whatever the row asked.
//     const pinned = "version" in known.installRoad ? known.installRoad.version : undefined;
//     const notes = [
//       ...(known.source.road === "unmeasured" ? [UNMEASURED_ROAD] : []),
//       ...(at.note !== undefined ? [at.note] : e.version !== undefined && mod.at === undefined && e.version !== pinned ? [pinned !== undefined ? pinnedNote(e.version, pinned) : `${e.version} asked, installed ${mod.words} at its current version`] : []),
//     ];
//     const after = installAfter(known);
//     return { road: { ...at.road, ...pinOf(e) }, bin: known.bin, ...(after !== undefined ? { after } : {}), ...(notes.length > 0 ? { note: notes.join("; ") } : {}), ...(at.pinned !== undefined ? { pinned: at.pinned } : {}), ...(at.below !== undefined ? { below: { node: at.below.node, road: { ...at.below.road, ...pinOf(e) } } } : {}) };
//   }
//   const release = releaseFor(e, brew);
//   if (release !== undefined) return { road: releaseRoad(release), bin: release.name };
//   if (e.id.startsWith(CATALOG_PREFIX)) return undefined;
//   // The manager the row's id names, and the road its own module reads off such a row; a manager whose module reads
//   // none (apt, which every machine has already and no row of its own brings) leaves the row to the hand that added it.
//   const manager = ROADS.find(m => e.id.startsWith(toolRowPrefix(m)));
//   const fromRow = manager === undefined ? undefined : rowRoadReader(manager);
//   if (fromRow === undefined) return undefined;
//   const { road, pinned, note, below } = atCatalogPin(fromRow({ name: pkg, ...(e.version !== undefined ? { version: e.version } : {}), paths: e.paths, label: e.label }), e.version);
//   const bin = roadModule(road).bin?.(road);
//   return { road, ...(bin !== undefined ? { bin } : {}), ...(note !== undefined ? { note } : {}), ...(pinned !== undefined ? { pinned } : {}), ...(below !== undefined ? { below } : {}) };
// }
//
// export interface PlannedRow {
//   road: InstallRoad;
//   bin?: string;
//   /** The catalog's pin, where it won over the version the row asked. */
//   pinned?: string;
//   /** The row's own road, for a computer whose Node is older than the pin's engines take. */
//   below?: { node: string; road: InstallRoad };
//   /** What the catalog says the row runs on top of; a row outside the catalog takes its road's word. */
//   after?: string;
//   note?: string;
// }
/// golden-import.ts:958 `rowRoad`.
pub fn row_road(e: &RecipeEntry, brew: &BrewTable) -> Option<PlannedRow> {
    todo!()
}

// TS golden-import.ts:803-814
// /** Where the guest finds what the tools stage installs; each install line exports it so it does not depend on the
//  * machine's own environment, login shells get it from profile.d, and a workspace on a computer somebody owns boots
//  * with it. One PATH for a machine's tools, so it is the protocol's and neither side spells its own order. */
// export { TOOLS_PATH } from "@wsp/protocol";
// /** The line every script of a job exports its PATH and its managers' knobs with. Both are handed in rather than
//  * fixed: a machine wsp forked is root's alone and takes the tools PATH with each manager's own folders under its
//  * home, and a computer somebody owns shares its home with the workspaces on it, so the job there runs on the probe
//  * list and names the folder of wsp's own every manager installs under. One line for an install, a presence read
//  * and a version read alike, so no two of them reach different folders. */
// export const pathLine = (path: string, prefix?: string): string =>
//   ["export", `PATH=${path}`, ...Object.entries(installEnv(installHomes(prefix))).map(([name, value]) => `${name}=${value}`)].join(" ");
// export const PATH_LINE = pathLine(TOOLS_PATH);
/// golden-import.ts:812 `pathLine(path, prefix)`: `export PATH=<path> NAME=value ...` with the install homes' env, in
/// `install_env`'s order, nothing quoted.
pub fn path_line(path: &str, prefix: Option<&str>) -> String {
    todo!()
}

// TS golden-import.ts:824-826
// /** After the tools loop: dependencies no formula needs any more (a failed formula
//  * left a 2.4 GB llvm@21 behind), then the bottle cache and old kegs (5.5 GB measured). */
// export const brewHousekeeping = (path: string): readonly string[] => [withPath(asLinuxbrew("autoremove"), path), withPath(asLinuxbrew("cleanup -s --prune=all"), path)];
/// golden-import.ts:826 `brewHousekeeping`: autoremove, then `cleanup -s --prune=all`, each as linuxbrew on the PATH.
pub fn brew_housekeeping(path: &str) -> Vec<String> {
    todo!()
}

// TS golden-import.ts:1020-1022
// /** What a custom row runs with: the catalog roads' own PATH and apt environment, and Homebrew's build environment,
//  * which the shim on that PATH carries through to brew. */
// export const customPrelude = (path: string, prefix?: string): string => [pathLine(path, prefix), APT_ENV, `export ${BREW_ENV}`].join("\n");
/// golden-import.ts:1022 `customPrelude`.
pub fn custom_prelude(path: &str, prefix: Option<&str>) -> String {
    todo!()
}

// TS golden-import.ts:1024-1051
// /** The road a row outside the catalog installs by, read off the manager it names by that manager's own module;
//  * nothing for a row that names no manager, or one whose module reads no row of its own (apt, on every machine
//  * already). The row carries a name and nothing else a road reads. */
// const customRoad = (c: RecipeCustomRow): InstallRoad | undefined => (c.manager === undefined ? undefined : rowRoadReader(c.manager)?.({ name: c.name, paths: [], label: c.name }));
//
// /** The rows outside the catalog as installs, in the order the recipe carries them; each runs its own lines as given.
//  * A row whose manager names a road takes its reads off that road's module, the road's presence read and the road's
//  * own check, the same two a catalog row of that road gets: the check the scan wrote beside the row is a command
//  * frozen on the day it ran, and a recipe carries it unchanged for as long as the row lives. The row's own check
//  * stands where no module reads the row. `after` is what the plan brings the row's manager by, when the row names
//  * one and the plan brings it. */
// export function customInstallsFor(custom: readonly RecipeCustomRow[], after: (c: RecipeCustomRow) => string | undefined = () => undefined, path: string = TOOLS_PATH, prefix?: string): ToolInstall[] {
//   return custom.map(c => {
//     const waits = after(c);
//     const road = customRoad(c);
//     const reads = road === undefined ? undefined : roadStepReads(road, c.name, prefix);
//     return {
//       id: `${CUSTOM_PREFIX}${c.id}`,
//       label: c.name,
//       manager: "script" as const,
//       cmd: [customPrelude(path, prefix), ...c.install].join("\n"),
//       ...reads,
//       check: reads?.check ?? c.check,
//       shown: shownOf(c.install),
//       ...(waits !== undefined ? { after: waits } : {}),
//     };
//   });
// }
/// golden-import.ts:1035 `customInstallsFor(custom, after, path, prefix)`, in the recipe's order.
pub fn custom_installs_for(
    custom: &[RecipeCustomRow],
    after: &dyn Fn(&RecipeCustomRow) -> Option<String>,
    path: &str,
    prefix: Option<&str>,
) -> Vec<ToolInstall> {
    todo!()
}

/// golden-import.ts:1054 `shownOf`: the lines joined with `; `.
pub fn shown_of(lines: &[&str]) -> String {
    todo!()
}

/// golden-import.ts:1058 `viaRoad`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViaRoad {
    Step { cmd: String, shown: String },
    Note(String),
}

// TS golden-import.ts:1056-1063
// /** A step's script and the line shown for it, through the road's module: the module's own line when the script is
//  * not one a person reads, else the script itself. */
// export function viaRoad(road: InstallRoad, bin: string, path: string = TOOLS_PATH, prefix?: string): { cmd: string; shown: string } | { note: string } {
//   const mod = roadModule(road);
//   const line = mod.install(road, bin, installHomes(prefix));
//   if (typeof line !== "string") return line;
//   return { cmd: withPath(line, path, prefix), shown: mod.shown?.(road, bin) ?? shownOf(line.split("\n")) };
// }
/// golden-import.ts:1058 `viaRoad(road, bin, path, prefix)`.
pub fn via_road(road: &InstallRoad, bin: &str, path: &str, prefix: Option<&str>) -> ViaRoad {
    todo!()
}

// TS golden-import.ts:1065-1068
// /** The apt index read once, as a step every apt row waits on; the base and the recipe plan each have one. */
// export function aptIndexStep(id: string, cmd: string): ToolInstall {
//   return { id, label: "apt index", manager: "apt", cmd, shown: "apt-get update" };
// }
/// golden-import.ts:1066 `aptIndexStep`.
pub fn apt_index_step(id: &str, cmd: &str) -> ToolInstall {
    todo!()
}

/// golden-import.ts:1071 `brewTapCheck`.
pub fn brew_tap_check(tap: &str) -> String {
    todo!()
}
