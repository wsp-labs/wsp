// SPDX-License-Identifier: AGPL-3.0-only
//! Unit import_plan (spec/golden-import_plan.md): golden-import.ts:1009-1015 and 1073-1234 at a4033dc34. A recipe's
//! ticked tools rows as the steps of the one tools loop: Homebrew and its toolchain, the managers, the rows, the rows
//! outside the catalog, each waiting on what it needs.
use wsp_wire::views::golden_image::RecipeCustomRow;

use super::{BaseRow, BrewTable, RecipeEntry, SkippedItem, ToolInstall};
use crate::errors::Result;

/// golden-import.ts:1009 `ToolsPlan`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolsPlan {
    pub installs: Vec<ToolInstall>,
    pub skipped: Vec<SkippedItem>,
    pub base: Vec<BaseRow>,
    pub brewfile: String,
}

// TS golden-import.ts:1073-1081
// /** A formula's step, for the plan's own brew lines: the toolchain, a manager's formula. */
// function viaBrew(formula: string, path: string, prefix?: string): { cmd: string; shown: string } {
//   const step = viaRoad({ road: "brew", formula }, formula, path, prefix);
//   if (!("cmd" in step)) throw new Error(`${formula}: ${step.note}`);
//   return step;
// }
//
// /** The road a row outside the catalog names as its manager, when it names one the catalog knows. */
// const managerRoad = (c: RecipeCustomRow): RoadName | undefined => ROADS.find(r => r === c.manager);
// TS golden-import.ts:1083-1234
// export function toolInstallsFor(entries: readonly RecipeEntry[], table: BrewTable = new Map(), custom: readonly RecipeCustomRow[] = [], path: string = TOOLS_PATH, prefix?: string): ToolsPlan {
//   const brew = brewfileFor(entries, table);
//   const installs: ToolInstall[] = [];
//   const skipped: SkippedItem[] = [...brew.skipped];
//   const toolRows = entries.filter(e => ticked(e) && e.rung === "tools" && baseRowFor(e) === undefined);
//   const rowsOf = (manager: RoadName): RecipeEntry[] => toolRows.filter(e => e.id.startsWith(toolRowPrefix(manager)));
//   // A catalog row installs by its entry's road; a package road's rows go with the manager's, the rest after everything else.
//   // This Mac's formula for a manager's toolchain is one of them: the catalog's road, once, under the row's own id.
//   const catalog = toolRows.flatMap(e => {
//     if (!e.id.startsWith(CATALOG_PREFIX) && catalogToolOf(e) === undefined) return [];
//     const planned = rowRoad(e, table);
//     if (planned === undefined) skipped.push({ id: e.id, note: "not in the catalog" });
//     return planned === undefined ? [] : [{ e, planned }];
//   });
//   const catalogRowsOf = (manager: RoadName): RecipeEntry[] => catalog.filter(c => c.planned.road.road === manager).map(c => c.e);
//   const npmTicked = new Set(rowsOf("npm").map(e => packageOf(e)));
//
//   // Homebrew's own toolchain, each step waiting on the one before; everything brew installs waits on the last.
//   const toolchain = BREW_TOOLCHAIN.reduce<{ steps: ToolInstall[]; last: string }>(
//     (acc, f) => {
//       const id = `tools/brew-toolchain/${f}`;
//       acc.steps.push({ id, label: `Homebrew's ${f}`, manager: "brew", ...viaBrew(f, path, prefix), after: acc.last, check: brewHasCheck(f) });
//       return { steps: acc.steps, last: id };
//     },
//     { steps: [], last: "tools/homebrew" },
//   );
//
//   // A row outside the catalog calls its manager's own command, so it counts as a row of that manager here: the
//   // manager is brought onto the machine the one way this function knows, and the row waits on it.
//   const customOf = (manager: RoadName): RecipeCustomRow[] => custom.filter(c => managerRoad(c) === manager);
//   // One step per manager that has rows, unless the base carries it or it already comes along as a formula, a ticked
//   // catalog row or an npm global; the step takes the manager's catalog road, or its formula when the catalog has none.
//   // npm's manager is node, which no image carries by default, so it is resolved on demand as well: whatever first
//   // runs on node asks for it, and it lands once.
//   const managers = new Map<RoadName, { after: string; step?: ToolInstall; row?: { e: RecipeEntry; planned: PlannedRow } }>();
//   const managerFormulae: string[] = [];
//   const resolved = new Set<RoadName>();
//   const resolveManager = (manager: RoadName): void => {
//     if (resolved.has(manager)) return;
//     resolved.add(manager);
//     if (baseEntryFor(manager) !== undefined) return;
//     const own = `${MANAGER_STEP}${manager}`;
//     const entry = catalogToolFor(manager);
//     const formula = managerFormula(manager);
//     const fromCatalog = entry === undefined ? undefined : catalog.find(c => catalogToolOf(c.e)?.id === entry.id);
//     // This Mac's Brewfile may already carry the manager, under its formula's name or its own.
//     const macFormula = [formula, manager].find(f => f !== undefined && brew.formulae.includes(f));
//     if (macFormula !== undefined) managers.set(manager, { after: `${BREW_ID_PREFIX}${macFormula}` });
//     else if (fromCatalog !== undefined) managers.set(manager, { after: fromCatalog.e.id, row: fromCatalog });
//     else if (npmTicked.has(manager)) managers.set(manager, { after: toolRowId("npm", manager) });
//     else if (formula !== undefined) {
//       managers.set(manager, { after: own, step: { id: own, label: manager, manager: "brew", ...viaBrew(formula, path, prefix), ...roadReads({ road: "brew", formula }, manager, prefix), after: toolchain.last, bin: manager } });
//       managerFormulae.push(formula);
//     } else if (entry !== undefined) {
//       const step = viaRoad(entry.installRoad, entry.bin, path, prefix);
//       if (!("cmd" in step)) throw new Error(`${entry.id}: ${step.note}`);
//       managers.set(manager, { after: own, step: { id: own, label: entry.name, manager: entry.installRoad.road, ...step, bin: entry.bin } });
//     }
//     else throw new Error(`${manager} is neither in the base, nor in the catalog, nor a formula`);
//   };
//   for (const manager of MANAGER_ORDER) {
//     if (rowsOf(manager).length + catalogRowsOf(manager).length + customOf(manager).length === 0) continue;
//     resolveManager(manager);
//   }
//   // A catalog row that runs on another manager's toolchain resolves that manager here and not when the row is
//   // planned: the step it brings may be a ticked row of the recipe's own, and what stands for a manager has to be
//   // settled before the first row is planned or that row is planned twice.
//   for (const { planned } of catalog) {
//     const owner = managerBehind(depOf(planned));
//     if (owner !== undefined) resolveManager(owner);
//   }
//   const catalogFormulae = catalog.flatMap(c => (c.planned.road.road === "brew" ? roadModule(c.planned.road).names(c.planned.road) : []));
//
//   if (brew.taps.length + brew.formulae.length > 0 || managerFormulae.length + catalogFormulae.length + customOf("brew").length > 0) {
//     installs.push({ id: "tools/homebrew", label: "Homebrew", manager: "brew", cmd: withPath(homebrewBootstrap(), path, prefix), shown: `git clone github.com/Homebrew/brew at ${HOMEBREW.tag}`, bin: "brew", present: BREW_SHIM_PRESENT });
//     installs.push(...toolchain.steps);
//     for (const t of brew.taps) installs.push({ id: `tools/brew-tap/${t}`, label: t, manager: "brew", cmd: withPath(asLinuxbrew(`tap ${t}`), path, prefix), shown: `brew tap ${t}`, after: toolchain.last, check: brewTapCheck(t) });
//     const formulae = [...brew.formulae, ...managerFormulae, ...catalogFormulae];
//     // The check reads the dependencies this step installs, not the formulae they belong to: a formula whose own
//     // step failed is that row's failure, and this one did its work.
//     if (formulae.length > 1) installs.push({ id: "tools/brew-shared", label: "shared Homebrew dependencies", manager: "brew", cmd: withPath(brewSharedDeps(formulae), path, prefix), shown: `brew install the dependencies ${formulae.join(", ")} share`, after: toolchain.last, check: brewSharedCheck(formulae) });
//     for (const f of brew.formulae) installs.push({ id: `${BREW_ID_PREFIX}${f}`, label: f, manager: "brew", ...viaBrew(f, path, prefix), ...roadStepReads({ road: "brew", formula: f }, f, prefix), after: toolchain.last, pin: pinReadOf({ road: "brew", formula: f }, f, prefix) });
//   }
//   // What a row waits on: the apt index read once by its own step, Homebrew's toolchain, node for a road that runs on
//   // it, the manager's step otherwise; a floor row is there already.
//   const APT_STEP = `tools/${APT_INDEX}`;
//   const afterDep = (dep: string | undefined, road: RoadName): string | undefined => {
//     if (dep === APT_INDEX) {
//       if (!installs.some(t => t.id === APT_STEP)) installs.push(aptIndexStep(APT_STEP, withPath(APT_UPDATE, path, prefix)));
//       return APT_STEP;
//     }
//     if (dep === HOMEBREW_STEP) return toolchain.last;
//     // A road that names a catalog row it runs on (npm's node) waits on the manager that row is the toolchain of,
//     // whether or not the recipe has rows of that manager's own.
//     const owner = managerBehind(dep);
//     if (owner !== undefined) resolveManager(owner);
//     return bringManager(owner ?? road);
//   };
//   const afterRoad = (road: RoadName): string | undefined => afterDep(ROAD_MODULES[road].after, road);
//   // The manager's own steps, pushed the first time anything waits on them, so the step that brings a manager stands
//   // ahead of every row that needs it whichever road asked.
//   const brought = new Set<RoadName>();
//   function bringManager(manager: RoadName): string | undefined {
//     const brings = managers.get(manager);
//     if (brings === undefined || brought.has(manager)) return brings?.after;
//     brought.add(manager);
//     if (brings.step !== undefined) {
//       const after = afterRoad(brings.step.manager);
//       installs.push(after === undefined ? brings.step : { ...brings.step, after });
//     }
//     if (brings.row !== undefined) plan(brings.row.e, brings.row.planned);
//     return brings.after;
//   }
//   const plan = (e: RecipeEntry, planned: PlannedRow): void => {
//     const step = viaRoad(planned.road, planned.bin ?? packageOf(e), path, prefix);
//     if (!("cmd" in step)) {
//       skipped.push({ id: e.id, note: step.note });
//       return;
//     }
//     const after = afterDep(depOf(planned), planned.road.road);
//     const bin = planned.bin ?? packageOf(e);
//     const below = planned.below === undefined ? {} : { cmd: withPath(belowLine(planned.road, planned.below, bin, installHomes(prefix)), path, prefix), below: { node: planned.below.node, ...asksOf(planned.below.road) } };
//     installs.push({ id: e.id, label: e.label, manager: planned.road.road, ...step, ...below, ...roadReads(planned.road, bin, prefix), ...(after !== undefined ? { after } : {}), ...(planned.bin !== undefined ? { bin: planned.bin } : {}), ...(planned.note !== undefined ? { note: planned.note } : {}), ...asksOf(planned.road), pin: pinReadOf(planned.road, bin, prefix) });
//   };
//   // A catalog row that is a manager's own toolchain is planned with that manager, not again with the road it takes.
//   const asManager = (id: string): boolean => [...managers.values()].some(m => m.row?.e.id === id);
//   for (const manager of MANAGER_ORDER) {
//     const rows = rowsOf(manager);
//     const fromCatalog = catalogRowsOf(manager).filter(e => !asManager(e.id));
//     if (rows.length + fromCatalog.length + customOf(manager).length === 0) continue;
//     bringManager(manager);
//     for (const e of rows) {
//       const planned = rowRoad(e, table);
//       if (planned !== undefined) plan(e, planned);
//     }
//     for (const e of fromCatalog) plan(e, rowRoad(e, table)!);
//   }
//   // Last, after any go the plan brings: a road install needs no brew and waits on nothing.
//   for (const r of brew.roads) {
//     const e = entries.find(x => x.id === r.id)!;
//     plan(e, rowRoad(e, table)!);
//   }
//   // The catalog rows on every other road: Homebrew's (unless one went out as a manager's step above), apt's, a release, a vendor's, a script.
//   for (const { e, planned } of catalog) if (!(MANAGER_ORDER as readonly string[]).includes(planned.road.road) && !asManager(e.id)) plan(e, planned);
//   // Last of all: the rows the catalog does not carry, each after the manager its line calls, so every road they
//   // lean on has already run.
//   installs.push(...customInstallsFor(custom, c => {
//     const road = managerRoad(c);
//     return road === undefined ? undefined : afterRoad(road);
//   }, path, prefix));
//   return { installs, skipped, base: brew.base, brewfile: brew.text };
// }
/// golden-import.ts:1083 `toolInstallsFor(entries, table, custom, path, prefix)`. Where the TS throws (a manager that
/// is neither in the base, nor in the catalog, nor a formula; a manager road with no line) the `Err` carries its
/// words as `EngineError::Other`.
pub fn tool_installs_for_rows(
    entries: &[RecipeEntry],
    table: &BrewTable,
    custom: &[RecipeCustomRow],
    path: &str,
    prefix: Option<&str>,
) -> Result<ToolsPlan> {
    todo!()
}

/// `toolInstallsFor(ids.map(row), new Map(), [], path, prefix).installs`, as the provision tests call it: each id a
/// ticked row as provision.test.ts:121 `row(id, "tools")` builds it: `bring: true`, `default: "bring"`, no paths,
/// 0 bytes, the id as its label, and the id's first segment as its rung. Panics where the plan is an `Err`.
pub fn tool_installs_for(ids: &[&str], path: &str, prefix: Option<&str>) -> Vec<ToolInstall> {
    let rows: Vec<RecipeEntry> = ids
        .iter()
        .map(|id| RecipeEntry {
            rung: id.split('/').next().unwrap_or_default().to_string(),
            id: id.to_string(),
            label: id.to_string(),
            default: "bring".to_string(),
            bring: Some(true),
            ..RecipeEntry::default()
        })
        .collect();
    tool_installs_for_rows(&rows, &BrewTable::new(), &[], path, prefix).unwrap_or_else(|e| panic!("{e}")).installs
}
