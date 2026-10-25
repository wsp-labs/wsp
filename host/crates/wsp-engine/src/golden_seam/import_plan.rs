// SPDX-License-Identifier: AGPL-3.0-only
//! Unit import_plan (spec/golden-import_plan.md): golden-import.ts:1009-1015 and 1073-1234 at a4033dc34. A recipe's
//! ticked tools rows as the steps of the one tools loop: Homebrew and its toolchain, the managers, the rows, the rows
//! outside the catalog, each waiting on what it needs.
use std::collections::HashSet;

use indexmap::IndexMap;
use wsp_catalog::catalog::{below_line, base_entry_for, catalog_tool_for, ToolEntry};
use wsp_catalog::road_modules::{
    as_linuxbrew, brew_has_check, install_homes, road_module, road_module_named, APT_INDEX, APT_UPDATE, BREW, BREW_PREFIX, BREW_REAL,
    BREW_REPO, BREW_SHIM_PRESENT, HOMEBREW_COMMIT, HOMEBREW_STEP, HOMEBREW_TAG, LINUXBREW_HOME, LINUXBREW_SHIM,
};
use wsp_catalog::roads::{is_road, InstallRoad, RoadName, APT_ENV};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::{package_of, tool_row_id, tool_row_prefix, RecipeCustomRow, BREW_ID_PREFIX};

use super::import_roads::{
    apt_index_step, asks_version, base_row_for, brew_tap_check, brewfile_for, catalog_tool_of, custom_installs_for, manager_behind,
    manager_formula, path_line, pin_read_of, road_reads, road_step_reads, row_road, via_road, Brewfile, PlannedRow, ViaRoad,
    CATALOG_PREFIX, MANAGER_ORDER, MANAGER_STEP,
};
use super::{BaseRow, Below, BrewTable, RecipeEntry, SkippedItem, ToolInstall};
use crate::brew_shared::{brew_shared_check, brew_shared_deps, BREW_TOOLCHAIN};
use crate::errors::{EngineError, Result};

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
    let brew = brewfile_for(entries, table);
    let empty = BrewTable::new();
    let tool_rows: Vec<&RecipeEntry> =
        entries.iter().filter(|e| e.bring == Some(true) && e.rung == "tools" && base_row_for(e, &empty).is_none()).collect();
    let mut skipped = brew.skipped.clone();
    let mut catalog: Vec<(RecipeEntry, PlannedRow)> = Vec::new();
    for e in &tool_rows {
        if !e.id.starts_with(CATALOG_PREFIX) && catalog_tool_of(e).is_none() {
            continue;
        }
        match row_road(e, table) {
            Some(planned) => catalog.push(((*e).clone(), planned)),
            None => skipped.push(SkippedItem { id: e.id.clone(), note: "not in the catalog".to_string() }),
        }
    }
    let npm_ticked: HashSet<String> =
        tool_rows.iter().filter(|e| e.id.starts_with(&tool_row_prefix("npm"))).map(|e| package_of(&e.id)).collect();
    let mut plan = Plan {
        entries,
        table,
        custom,
        path,
        prefix,
        tool_rows,
        catalog,
        npm_ticked,
        toolchain: Vec::new(),
        toolchain_last: "tools/homebrew".to_string(),
        installs: Vec::new(),
        skipped,
        managers: IndexMap::new(),
        manager_formulae: Vec::new(),
        resolved: HashSet::new(),
        brought: HashSet::new(),
        brew,
    };
    plan.run()?;
    Ok(ToolsPlan { installs: plan.installs, skipped: plan.skipped, base: plan.brew.base, brewfile: plan.brew.text })
}

/// What a manager is brought by (golden-import.ts:1116): what rows of its roads wait on, and the step or the catalog
/// row that brings it.
struct Brings {
    after: String,
    step: Option<ToolInstall>,
    row: Option<(RecipeEntry, PlannedRow)>,
}

struct Plan<'a> {
    entries: &'a [RecipeEntry],
    table: &'a BrewTable,
    custom: &'a [RecipeCustomRow],
    path: &'a str,
    prefix: Option<&'a str>,
    tool_rows: Vec<&'a RecipeEntry>,
    catalog: Vec<(RecipeEntry, PlannedRow)>,
    npm_ticked: HashSet<String>,
    toolchain: Vec<ToolInstall>,
    toolchain_last: String,
    installs: Vec<ToolInstall>,
    skipped: Vec<SkippedItem>,
    managers: IndexMap<RoadName, Brings>,
    manager_formulae: Vec<String>,
    resolved: HashSet<RoadName>,
    brought: HashSet<RoadName>,
    brew: Brewfile,
}

fn failure(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

/// golden-import.ts:1073 `viaBrew`.
fn via_brew(formula: &str, path: &str, prefix: Option<&str>) -> Result<(String, String)> {
    match via_road(&InstallRoad::Brew { formula: formula.to_string(), pin: None }, formula, path, prefix) {
        ViaRoad::Step { cmd, shown } => Ok((cmd, shown)),
        ViaRoad::Note(note) => Err(failure(format!("{formula}: {note}"))),
    }
}

/// golden-import.ts:1080 `managerRoad`.
fn manager_road(c: &RecipeCustomRow) -> Option<RoadName> {
    is_road(c.manager.as_deref())
}

/// golden-import.ts:359 `depOf`.
fn dep_of(planned: &PlannedRow) -> Option<String> {
    planned.after.clone().or_else(|| road_module(&planned.road).after().map(str::to_string))
}

impl Plan<'_> {
    fn rows_of(&self, manager: RoadName) -> Vec<&RecipeEntry> {
        let prefix = tool_row_prefix(manager.as_str());
        self.tool_rows.iter().copied().filter(|e| e.id.starts_with(&prefix)).collect()
    }

    fn catalog_rows_of(&self, manager: RoadName) -> Vec<(RecipeEntry, PlannedRow)> {
        self.catalog.iter().filter(|(_, p)| p.road.name() == manager).cloned().collect()
    }

    fn custom_of(&self, manager: RoadName) -> usize {
        self.custom.iter().filter(|c| manager_road(c) == Some(manager)).count()
    }

    fn as_manager(&self, id: &str) -> bool {
        self.managers.values().any(|m| m.row.as_ref().is_some_and(|(e, _)| e.id == id))
    }

    fn run(&mut self) -> Result<()> {
        let (path, prefix) = (self.path, self.prefix);
        let mut last = "tools/homebrew".to_string();
        for f in BREW_TOOLCHAIN {
            let id = format!("tools/brew-toolchain/{f}");
            let (cmd, shown) = via_brew(f, path, prefix)?;
            self.toolchain.push(ToolInstall {
                id: id.clone(),
                label: format!("Homebrew's {f}"),
                manager: "brew".to_string(),
                cmd,
                shown: Some(shown),
                after: Some(last),
                check: Some(brew_has_check(&[f])),
                ..Default::default()
            });
            last = id;
        }
        self.toolchain_last = last;

        for manager in MANAGER_ORDER {
            if self.rows_of(manager).len() + self.catalog_rows_of(manager).len() + self.custom_of(manager) == 0 {
                continue;
            }
            self.resolve_manager(manager)?;
        }
        let owners: Vec<RoadName> = self.catalog.iter().filter_map(|(_, p)| manager_behind(dep_of(p).as_deref())).collect();
        for owner in owners {
            self.resolve_manager(owner)?;
        }
        let catalog_formulae: Vec<String> = self
            .catalog
            .iter()
            .filter(|(_, p)| p.road.name() == RoadName::Brew)
            .flat_map(|(_, p)| road_module(&p.road).names(&p.road))
            .collect();

        if self.brew.taps.len() + self.brew.formulae.len() > 0
            || self.manager_formulae.len() + catalog_formulae.len() + self.custom_of(RoadName::Brew) > 0
        {
            self.installs.push(ToolInstall {
                id: "tools/homebrew".to_string(),
                label: "Homebrew".to_string(),
                manager: "brew".to_string(),
                cmd: format!("{}\n{}", path_line(path, prefix), homebrew_bootstrap()),
                shown: Some(format!("git clone github.com/Homebrew/brew at {HOMEBREW_TAG}")),
                bin: Some("brew".to_string()),
                present: Some(BREW_SHIM_PRESENT.clone()),
                ..Default::default()
            });
            self.installs.extend(self.toolchain.clone());
            for t in &self.brew.taps {
                self.installs.push(ToolInstall {
                    id: format!("tools/brew-tap/{t}"),
                    label: t.clone(),
                    manager: "brew".to_string(),
                    cmd: format!("{}\n{}", path_line(path, prefix), as_linuxbrew(&format!("tap {t}"))),
                    shown: Some(format!("brew tap {t}")),
                    after: Some(self.toolchain_last.clone()),
                    check: Some(brew_tap_check(t)),
                    ..Default::default()
                });
            }
            let formulae: Vec<String> =
                self.brew.formulae.iter().chain(&self.manager_formulae).chain(&catalog_formulae).cloned().collect();
            if formulae.len() > 1 {
                self.installs.push(ToolInstall {
                    id: "tools/brew-shared".to_string(),
                    label: "shared Homebrew dependencies".to_string(),
                    manager: "brew".to_string(),
                    cmd: format!("{}\n{}", path_line(path, prefix), brew_shared_deps(&formulae)),
                    shown: Some(format!("brew install the dependencies {} share", formulae.join(", "))),
                    after: Some(self.toolchain_last.clone()),
                    check: Some(brew_shared_check(&formulae)),
                    ..Default::default()
                });
            }
            for f in self.brew.formulae.clone() {
                let road = InstallRoad::Brew { formula: f.clone(), pin: None };
                let (cmd, shown) = via_brew(&f, path, prefix)?;
                let reads = road_step_reads(&road, &f, prefix);
                self.installs.push(ToolInstall {
                    id: format!("{BREW_ID_PREFIX}{f}"),
                    label: f.clone(),
                    manager: "brew".to_string(),
                    cmd,
                    shown: Some(shown),
                    present: reads.present,
                    bins: reads.bins,
                    check: reads.check,
                    after: Some(self.toolchain_last.clone()),
                    pin: Some(pin_read_of(&road, &f, prefix)),
                    ..Default::default()
                });
            }
        }

        for manager in MANAGER_ORDER {
            let rows: Vec<RecipeEntry> = self.rows_of(manager).into_iter().cloned().collect();
            let from_catalog: Vec<(RecipeEntry, PlannedRow)> =
                self.catalog_rows_of(manager).into_iter().filter(|(e, _)| !self.as_manager(&e.id)).collect();
            if rows.len() + from_catalog.len() + self.custom_of(manager) == 0 {
                continue;
            }
            self.bring_manager(manager)?;
            for e in &rows {
                if let Some(planned) = row_road(e, self.table) {
                    self.plan(e, &planned)?;
                }
            }
            for (e, planned) in &from_catalog {
                self.plan(e, planned)?;
            }
        }
        for r in self.brew.roads.clone() {
            let e = self.entries.iter().find(|x| x.id == r.id).expect("a planned road names a row of the recipe");
            let planned = row_road(e, self.table).expect("a planned road is a row's road");
            self.plan(e, &planned)?;
        }
        for (e, planned) in self.catalog.clone() {
            if !MANAGER_ORDER.contains(&planned.road.name()) && !self.as_manager(&e.id) {
                self.plan(&e, &planned)?;
            }
        }
        let mut afters: Vec<Option<String>> = Vec::new();
        for c in self.custom {
            afters.push(match manager_road(c) {
                Some(road) => self.after_road(road)?,
                None => None,
            });
        }
        let custom = self.custom;
        let installs = custom_installs_for(
            custom,
            &|c| afters[custom.iter().position(|x| std::ptr::eq(x, c)).expect("a row of the plan's own")].clone(),
            path,
            prefix,
        );
        self.installs.extend(installs);
        Ok(())
    }

    fn resolve_manager(&mut self, manager: RoadName) -> Result<()> {
        if !self.resolved.insert(manager) {
            return Ok(());
        }
        let name = manager.as_str();
        if base_entry_for(name).is_some() {
            return Ok(());
        }
        let own = format!("{MANAGER_STEP}{name}");
        let entry: Option<&'static ToolEntry> = catalog_tool_for(name);
        let formula = manager_formula(manager);
        let from_catalog = entry.and_then(|entry| {
            self.catalog.iter().find(|(e, _)| catalog_tool_of(e).is_some_and(|t| t.id == entry.id)).cloned()
        });
        let mac_formula = [formula.as_deref(), Some(name)].into_iter().flatten().find(|f| self.brew.formulae.iter().any(|x| x == f));
        let (path, prefix) = (self.path, self.prefix);
        if let Some(f) = mac_formula {
            self.managers.insert(manager, Brings { after: format!("{BREW_ID_PREFIX}{f}"), step: None, row: None });
        } else if let Some((e, planned)) = from_catalog {
            self.managers.insert(manager, Brings { after: e.id.clone(), step: None, row: Some((e, planned)) });
        } else if self.npm_ticked.contains(name) {
            self.managers.insert(manager, Brings { after: tool_row_id("npm", name), step: None, row: None });
        } else if let Some(formula) = formula {
            let (cmd, shown) = via_brew(&formula, path, prefix)?;
            let reads = road_reads(&InstallRoad::Brew { formula: formula.clone(), pin: None }, name, prefix);
            let step = ToolInstall {
                id: own.clone(),
                label: name.to_string(),
                manager: "brew".to_string(),
                cmd,
                shown: Some(shown),
                present: reads.present,
                bins: reads.bins,
                after: Some(self.toolchain_last.clone()),
                bin: Some(name.to_string()),
                ..Default::default()
            };
            self.managers.insert(manager, Brings { after: own, step: Some(step), row: None });
            self.manager_formulae.push(formula);
        } else if let Some(entry) = entry {
            let (cmd, shown) = match via_road(&entry.install_road, &entry.bin, path, prefix) {
                ViaRoad::Step { cmd, shown } => (cmd, shown),
                ViaRoad::Note(note) => return Err(failure(format!("{}: {note}", entry.id))),
            };
            let step = ToolInstall {
                id: own.clone(),
                label: entry.name.clone(),
                manager: entry.install_road.name().as_str().to_string(),
                cmd,
                shown: Some(shown),
                bin: Some(entry.bin.clone()),
                ..Default::default()
            };
            self.managers.insert(manager, Brings { after: own, step: Some(step), row: None });
        } else {
            return Err(failure(format!("{name} is neither in the base, nor in the catalog, nor a formula")));
        }
        Ok(())
    }

    fn after_dep(&mut self, dep: Option<&str>, road: RoadName) -> Result<Option<String>> {
        if dep == Some(APT_INDEX) {
            let step_id = format!("tools/{APT_INDEX}");
            if !self.installs.iter().any(|t| t.id == step_id) {
                let cmd = format!("{}\n{}", path_line(self.path, self.prefix), *APT_UPDATE);
                self.installs.push(apt_index_step(&step_id, &cmd));
            }
            return Ok(Some(step_id));
        }
        if dep == Some(HOMEBREW_STEP) {
            return Ok(Some(self.toolchain_last.clone()));
        }
        let owner = manager_behind(dep);
        if let Some(owner) = owner {
            self.resolve_manager(owner)?;
        }
        self.bring_manager(owner.unwrap_or(road))
    }

    fn after_road(&mut self, road: RoadName) -> Result<Option<String>> {
        self.after_dep(road_module_named(road).after(), road)
    }

    fn bring_manager(&mut self, manager: RoadName) -> Result<Option<String>> {
        let Some(brings) = self.managers.get(&manager) else { return Ok(None) };
        let after = brings.after.clone();
        if !self.brought.insert(manager) {
            return Ok(Some(after));
        }
        let (step, row) = (brings.step.clone(), brings.row.clone());
        if let Some(mut step) = step {
            let behind = self.after_road(is_road(Some(&step.manager)).expect("a manager step names a road"))?;
            if behind.is_some() {
                step.after = behind;
            }
            self.installs.push(step);
        }
        if let Some((e, planned)) = row {
            self.plan(&e, &planned)?;
        }
        Ok(Some(after))
    }

    fn plan(&mut self, e: &RecipeEntry, planned: &PlannedRow) -> Result<()> {
        let (path, prefix) = (self.path, self.prefix);
        let bin = planned.bin.clone().unwrap_or_else(|| package_of(&e.id));
        let (mut cmd, shown) = match via_road(&planned.road, &bin, path, prefix) {
            ViaRoad::Step { cmd, shown } => (cmd, shown),
            ViaRoad::Note(note) => {
                self.skipped.push(SkippedItem { id: e.id.clone(), note });
                return Ok(());
            }
        };
        let after = self.after_dep(dep_of(planned).as_deref(), planned.road.name())?;
        let mut below = None;
        if let Some(b) = &planned.below {
            let line = below_line(&planned.road, b, &bin, &install_homes(prefix)).map_err(|err| failure(err.message))?;
            cmd = format!("{}\n{line}", path_line(path, prefix));
            below = Some(Below { node: b.node.clone(), asks: asks_version(&b.road) });
        }
        let reads = road_reads(&planned.road, &bin, prefix);
        self.installs.push(ToolInstall {
            id: e.id.clone(),
            label: e.label.clone(),
            manager: planned.road.name().as_str().to_string(),
            cmd,
            shown: Some(shown),
            below,
            present: reads.present,
            bins: reads.bins,
            after,
            bin: planned.bin.clone(),
            note: planned.note.clone(),
            asks: asks_version(&planned.road),
            pin: Some(pin_read_of(&planned.road, &bin, prefix)),
            ..Default::default()
        });
        Ok(())
    }
}

/// golden-import.ts:828 `homebrewBootstrap`.
fn homebrew_bootstrap() -> String {
    let (brew, repo, real) = (BREW.as_str(), BREW_REPO.as_str(), BREW_REAL.as_str());
    [
        "set -euo pipefail".to_string(),
        APT_ENV.to_string(),
        format!("if [ ! -x {brew} ]; then"),
        "  if command -v apt-get >/dev/null 2>&1; then apt-get update -qq >/dev/null 2>&1 || true; apt-get install -y -qq procps curl file git >/dev/null 2>&1 || true; fi".to_string(),
        "  id -u linuxbrew >/dev/null 2>&1 || useradd -m -s /bin/bash linuxbrew".to_string(),
        format!("  git clone -q --depth 1 --branch {HOMEBREW_TAG} https://github.com/Homebrew/brew {repo}"),
        format!("  test \"$(git -C {repo} rev-parse HEAD)\" = \"{HOMEBREW_COMMIT}\""),
        format!("  git -C {repo} remote set-branches origin '*'"),
        format!("  git -C {repo} fetch -q --depth 1 origin main || echo \"origin/main did not fetch: brew update reads the branch in full\""),
        format!("  mkdir -p {BREW_PREFIX}/bin \"$(dirname {real})\""),
        format!("  ln -sfn ../Homebrew/bin/brew {real}"),
        format!("  chown -R linuxbrew:linuxbrew {LINUXBREW_HOME}"),
        "fi".to_string(),
        format!("rm -f {brew}"),
        format!("printf '%s\\n' {} > {brew}", shell_quote(&LINUXBREW_SHIM)),
        format!("chmod 0755 {brew}"),
        format!("{} >/dev/null", as_linuxbrew("--version")),
    ]
    .join("\n")
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
