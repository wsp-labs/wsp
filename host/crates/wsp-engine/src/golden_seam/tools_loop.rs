// SPDX-License-Identifier: AGPL-3.0-only
//! Unit tools_loop (spec/golden-tools_loop.md): golden-tools.ts:237-345 and 374-490 at a4033dc34. The one tools loop:
//! every install as one guarded run in plan order, the disk read before each against the floor, the cleanup at the
//! first reading under it, the checks after, Homebrew's housekeeping and the closing line.
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use wsp_catalog::road_modules::{road_module_named, road_step, BREW_PREFIX, HOMEBREW_COMMIT, HOMEBREW_TAG};
use wsp_catalog::roads::{is_road, RoadName};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::words::base::{listed_name, name_list};
use wsp_words::words::thread::{step_retry_line, timed_out_line};
use wsp_words::words::units::fmt_bytes;

use super::import_roads::{brew_housekeeping as brew_housekeeping_cmds, path_line};
use super::tools_disk::{
    closing, free_bytes, free_note, guard_deadline_ms, guarded_road, inline_s, missing_commands, reason_of, road_limit_s, road_of,
    sweep_caches, FreeDisk, MissingCommands, ALREADY_ON_MACHINE,
};
use super::{GoldenStage, InstallToolsOptions, OnTool, ToolInstall, ToolResult, ToolStage, TOOLS_DISK_FLOOR, TOOLS_PATH};
use crate::errors::{EngineError, Result};
use crate::exec_detached::{markers_of, paged_reads, INLINE_EXEC_MS};
use crate::machine::{ExecOptions, Machine, OnLine, RunOptions};

/// golden-tools.ts:39 `homebrew`: the checkout the formulae installed under, `HOMEBREW_TAG` and `HOMEBREW_COMMIT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomebrewCheckout {
    pub tag: String,
    pub commit: String,
}

/// golden-tools.ts:36 `ToolsOutcome`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolsOutcome {
    pub tools: Vec<ToolResult>,
    /// Set when a `tools/homebrew` step installed in this run.
    pub homebrew: Option<HomebrewCheckout>,
}

// TS golden-tools.ts:237-242
// /** Every skipped tool by name, those sharing a reason together: the reason is one line, the names are what the person ticked. */
// function skippedByReason(skipped: readonly ToolResult[]): string {
//   const byNote = new Map<string, string[]>();
//   for (const t of skipped) byNote.set(t.note ?? "no reason given", [...(byNote.get(t.note ?? "no reason given") ?? []), t.label]);
//   return [...byNote].map(([note, names]) => `${nameList(names)} (${note})`).join("; ");
// }
fn skipped_by_reason(skipped: &[&ToolResult]) -> String {
    let mut by_note: IndexMap<String, Vec<String>> = IndexMap::new();
    for t in skipped {
        by_note.entry(t.note.clone().unwrap_or_else(|| "no reason given".to_string())).or_default().push(t.label.clone());
    }
    by_note.iter().map(|(note, names)| format!("{} ({note})", name_list(names))).collect::<Vec<_>>().join("; ")
}

// TS golden-tools.ts:244-276
// type Run = (cmd: string, label: string, road: RoadName, step?: GoldenStep) => Promise<ExecResult>;
//
// /** Homebrew's autoremove then cleanup, each under the guard; the phrase says what came back or what failed, nothing when neither. */
// async function brewHousekeeping(machine: Machine, run: Run, path: string, prefix?: string): Promise<string | undefined> {
//   const before = await freeBytes(machine, prefix);
//   const failed: string[] = [];
//   for (const cmd of brewHousekeepingCmds(path)) {
//     const res = await run(cmd, "Homebrew cleanup", "brew");
//     if (res.exitCode !== 0) failed.push(reasonOf(res, roadLimitS("brew")));
//   }
//   const after = await freeBytes(machine, prefix);
//   if (failed.length > 0) return `Homebrew cleanup failed (${failed.join("; ")})`;
//   if (before.kind === "free" && after.kind === "free" && after.bytes > before.bytes) return `Homebrew cleanup freed ${fmtBytes(after.bytes - before.bytes)}`;
//   return undefined;
// }
//
// function summarize(tools: ToolResult[], housekeeping: string | undefined): string {
//   const parts: string[] = [];
//   const n = (o: ToolResult["outcome"]) => tools.filter(t => t.outcome === o);
//   // An install is named when it says something more than that it landed: the road it took, a note its plan carried.
//   // The note is bracketed and the label quoted when it carries this list's own separator, so neither reads as another tool.
//   const named = n("installed").flatMap(t => {
//     const road = t.road !== undefined ? ` ${ROAD_MODULES[t.road.kind].words}` : "";
//     const note = t.note !== undefined ? ` (${t.note})` : "";
//     return road === "" && note === "" ? [] : [`${listedName(t.label)}${road}${note}`];
//   });
//   parts.push(`${n("installed").length} installed${named.length > 0 ? ` (${named.join(", ")})` : ""}`);
//   const failed = n("failed");
//   if (failed.length > 0) parts.push(`${failed.length} failed: ${failed.map(t => `${listedName(t.label)} (${t.note})`).join(", ")}`);
//   const skipped = n("skipped");
//   if (skipped.length > 0) parts.push(`${skipped.length} skipped: ${skippedByReason(skipped)}`);
//   return housekeeping !== undefined ? `${parts.join(", ")}; ${housekeeping}` : parts.join(", ");
// }

/// golden-tools.ts:81: `ROAD_MODULES[t.road.kind].words`; a kind that is no road reads as itself.
fn road_words(kind: &str) -> String {
    is_road(Some(kind)).map_or_else(|| kind.to_string(), |road| road_module_named(road).words())
}

fn summarize(tools: &[ToolResult], housekeeping: Option<&str>) -> String {
    let of = |outcome: &str| -> Vec<&ToolResult> { tools.iter().filter(|t| t.outcome == outcome).collect() };
    let mut parts: Vec<String> = Vec::new();
    let installed = of("installed");
    let named: Vec<String> = installed
        .iter()
        .filter_map(|t| {
            let road = t.road.as_ref().map(|r| format!(" {}", road_words(&r.kind))).unwrap_or_default();
            let note = t.note.as_ref().map(|n| format!(" ({n})")).unwrap_or_default();
            (!(road.is_empty() && note.is_empty())).then(|| format!("{}{road}{note}", listed_name(&t.label)))
        })
        .collect();
    let named = if named.is_empty() { String::new() } else { format!(" ({})", named.join(", ")) };
    parts.push(format!("{} installed{named}", installed.len()));
    let failed = of("failed");
    if !failed.is_empty() {
        let list: Vec<String> =
            failed.iter().map(|t| format!("{} ({})", listed_name(&t.label), t.note.as_deref().unwrap_or("undefined"))).collect();
        parts.push(format!("{} failed: {}", failed.len(), list.join(", ")));
    }
    let skipped = of("skipped");
    if !skipped.is_empty() {
        parts.push(format!("{} skipped: {}", skipped.len(), skipped_by_reason(&skipped)));
    }
    let joined = parts.join(", ");
    match housekeeping {
        Some(h) => format!("{joined}; {h}"),
        None => joined,
    }
}

// TS golden-tools.ts:287-345
// /** The installs that name their command, checked by name on the tools PATH: an install that exited 0 without
//  * putting the command there is a failure, not an install, and so is one the check could not reach. */
// async function verifyCommands(machine: Machine, tools: readonly ToolInstall[], results: ToolResult[], stage: (detail: string) => void, path: string, prefix?: string): Promise<void> {
//   const named = results.filter(r => r.outcome === "installed").map(r => ({ result: r, bin: tools.find(t => t.id === r.id)?.bin })).filter((x): x is { result: ToolResult; bin: string } => x.bin !== undefined);
//   if (named.length === 0) return;
//   const { missing, failed } = await missingCommands(machine, named.map(x => x.bin), path, prefix);
//   if (failed !== undefined) stage(`the PATH check failed (${failed}): ${named.map(x => x.bin).join(", ")} count as failed`);
//   for (const x of named) {
//     if (failed === undefined && !missing.has(x.bin)) continue;
//     x.result.outcome = "failed";
//     x.result.note = failed === undefined ? `${x.bin} is not on PATH after the install` : `${x.bin} could not be checked on PATH: ${failed}`;
//     delete x.result.road;
//   }
// }
//
// /** What a batched check prints for a row that is not there: the marker, the row's place in the run, then the last
//  * line its check printed. The place and not the id, because an id is a custom row's own free text and can carry the
//  * space this line is read back on. Like the command check above, one run answers for every row rather than one round
//  * trip each. */
// const CHECK_FAILED = "wsp-check";
//
// /** The installs that carry a check of their own, read on the tools PATH after the stage, a page of reads to an
//  * exec: a row whose install exited 0 without leaving its tool on the machine is a failure, not an install, and so
//  * is every row of a page that could not be run at all, which is said in the stage detail rather than passing
//  * quietly. A row's note is what its check printed; the check itself goes to the stage detail, which is the log. Paged because a check can be a `brew list` of about a second and a recipe can carry a dozen: one read
//  * of all of them reaches the inline bound and would fail every row on the machine, agents included. */
// async function verifyChecks(machine: Machine, tools: readonly ToolInstall[], results: readonly ToolResult[], stage: (detail: string) => void, path: string, prefix?: string): Promise<void> {
//   const checked = results.flatMap(r => {
//     const check = r.outcome === "installed" ? tools.find(t => t.id === r.id)?.check : undefined;
//     return check === undefined ? [] : [{ result: r, check }];
//   });
//   if (checked.length === 0) return;
//   const pages = await pagedReads(
//     machine,
//     checked,
//     (c, at) => `if ! out="$( ( ${c.check} ) 2>&1 )"; then printf '${CHECK_FAILED} %s %s\\n' ${at} "$(printf '%s' "$out" | tail -1)"; fi`,
//     pathLine(path, prefix),
//   );
//   for (const { rows, res } of pages) {
//     if (res.exitCode !== 0) {
//       const why = reasonOf(res, INLINE_EXEC_MS / 1000);
//       stage(`the checks could not be run (${why}): ${nameList(rows.map(c => c.result.label))} count as failed`);
//       for (const c of rows) {
//         c.result.outcome = "failed";
//         c.result.note = `the check could not be run: ${why}`;
//         stage(`${c.result.label}: its check was ${c.check}`);
//       }
//       continue;
//     }
//     const failed = markersOf(res.stdout, CHECK_FAILED);
//     for (const [at, c] of rows.entries()) {
//       const why = failed.get(String(at));
//       if (why === undefined) continue;
//       c.result.outcome = "failed";
//       c.result.note = why === "" ? `${c.result.label} is not there after the install` : why;
//       stage(`${c.result.label}: the check did not pass: ${c.check}`);
//     }
//   }
// }
// TS golden-tools.ts:374-490
// /** Runs the plan's installs one at a time. Each tool fails alone and is named in the stage detail;
//  * one that waits on an install that failed is skipped with that install's name. At the first reading
//  * under the floor the cleanup runs once and df is read again, since the bottle cache alone held 2.4 GB
//  * at that point on one run; the loop stops only if the disk is still under the floor, and the tools
//  * left are skipped with the reading. */
// export async function installTools(machine: Machine, tools: readonly ToolInstall[], onStage: Stage, at: GoldenStage = "installing-tools", opts: InstallToolsOptions = {}): Promise<ToolsOutcome> {
//   const stage = (detail: string, step?: GoldenStep): void => onStage(at, detail, step);
//   const present = opts.present ?? new Set<string>();
//   const path = opts.path ?? TOOLS_PATH;
//   const keep = opts.floor ?? TOOLS_DISK_FLOOR;
//   /** Nothing on a machine whose caches are the person's own; the phrase where they are this run's to sweep. */
//   const sweep = async (): Promise<string | undefined> => (opts.caches === "keep" ? undefined : await sweepCaches(machine, path));
//   await machine.exec(SWEEP_TMP_CMD, { timeoutMs: INLINE_EXEC_MS });
//   const out: ToolsOutcome = { tools: [] };
//   /** One row's outcome: kept for the answer and handed to the caller watching, in the order the loop reaches them. */
//   const landed = async (result: ToolResult): Promise<void> => {
//     out.tools.push(result);
//     await opts.onTool?.(result);
//   };
//   const installed = new Set<string>();
//   const labelOf = (id: string): string => tools.find(t => t.id === id)?.label ?? id;
//   const run: Run = (cmd, label, road, step) => machine.run(guardedRoad(road, cmd), { deadlineMs: guardDeadlineMs(roadLimitS(road)), onLine: line => stage(`${label}: ${line}`, step) });
//   let floor: string | undefined;
//   let dfWarned = false;
//   let cleanedAtFloor = false;
//   /** Nothing touches the disk between the read after an install and the next tool's turn, so that read serves both. */
//   let reading: FreeDisk | undefined;
//   const floorNote = (reading: string): string => `${reading}, keeping ${fmtBytes(keep)} free`;
//   const cleanupAtFloor = async (why: string): Promise<FreeDisk | undefined> => {
//     if (cleanedAtFloor) return undefined;
//     cleanedAtFloor = true;
//     stage(`${why}; cleaning up before skipping`);
//     const brew = installed.has("tools/homebrew") ? await brewHousekeeping(machine, run, path, opts.prefix) : undefined;
//     const swept = await sweep();
//     const after = await freeBytes(machine, opts.prefix);
//     stage(closing(brew, swept, after.kind === "free" ? `${fmtBytes(after.bytes)} free` : after.reason));
//     return after;
//   };
//   for (const [i, tool] of tools.entries()) {
//     // Read before anything else about this step: a step the machine already satisfies at the version asked runs
//     // nothing, so neither the disk nor what it waits on has any bearing on it, and what waits on it waits on
//     // something that is already there.
//     if (present.has(tool.id)) {
//       installed.add(tool.id);
//       await landed({ id: tool.id, label: tool.label, outcome: "installed", note: ALREADY_ON_MACHINE });
//       continue;
//     }
//     if (tool.after !== undefined && !installed.has(tool.after)) {
//       await landed({ id: tool.id, label: tool.label, outcome: "skipped", note: `${labelOf(tool.after)} did not install` });
//       continue;
//     }
//     if (floor !== undefined) {
//       await landed({ id: tool.id, label: tool.label, outcome: "skipped", note: floor });
//       continue;
//     }
//     let free = reading ?? (await freeBytes(machine, opts.prefix));
//     reading = undefined;
//     if (free.kind === "unknown" && !dfWarned) {
//       dfWarned = true;
//       stage(`free disk unknown (${free.reason}); installing without the ${fmtBytes(keep)} floor`);
//     } else if (free.kind === "free" && free.bytes < keep) {
//       const after = await cleanupAtFloor(`${fmtBytes(free.bytes)} free, under the ${fmtBytes(keep)} floor`);
//       if (after === undefined || after.kind === "unknown" || after.bytes < keep) {
//         const words = after === undefined ? `${fmtBytes(free.bytes)} free` : after.kind === "free" ? `${fmtBytes(after.bytes)} free after cleanup` : `${fmtBytes(free.bytes)} free before cleanup and unknown after (${after.reason})`;
//         floor = floorNote(words);
//         await landed({ id: tool.id, label: tool.label, outcome: "skipped", note: floor });
//         continue;
//       }
//       free = after;
//     }
//     // A free reading over the floor says nothing about an install that takes more than the room left above it: one
//     // started 3 GB over the floor pulled 2.4 GB of LLVM and took the disk to nothing (2026-10-04). It is skipped
//     // alone, and what comes after it still gets its turn.
//     if (free.kind === "free" && tool.bytes !== undefined && free.bytes - tool.bytes < keep) {
//       const after = await cleanupAtFloor(`${tool.label} needs about ${fmtBytes(tool.bytes)} with ${fmtBytes(free.bytes)} free above a ${fmtBytes(keep)} floor`);
//       if (after?.kind === "free" && after.bytes - tool.bytes >= keep) free = after;
//     }
//     if (free.kind === "free" && tool.bytes !== undefined && free.bytes - tool.bytes < keep) {
//       reading = free;
//       await landed({ id: tool.id, label: tool.label, outcome: "skipped", note: floorNote(`needs about ${fmtBytes(tool.bytes)}, ${fmtBytes(free.bytes)} free`) });
//       continue;
//     }
//     const step: GoldenStep = { label: tool.label, command: tool.shown ?? tool.cmd };
//     const limit = roadLimitS(tool.manager);
//     stage(`${tool.label} (${i + 1}/${tools.length})`, step);
//     const t0 = Date.now();
//     let res = await run(tool.cmd, tool.label, tool.manager, step);
//     if (res.exitCode !== 0 && CELLAR_LOCKED.test(res.stderr)) {
//       stage(`${tool.label}: another brew holds its cellar; waiting for it, then once more`, step);
//       await machine.run(BREW_LOCK_WAIT_CMD, { deadlineMs: (BREW_LOCK_WAIT_S + 60) * 1000 });
//       res = await run(tool.cmd, tool.label, tool.manager, step);
//     }
//     let timeouts = res.exitCode === 124 ? 1 : 0;
//     if (timeouts === 1 && ROAD_STEPS[tool.manager].retry) {
//       stage(`${tool.label}: ${stepRetryLine(limit)}`, step);
//       res = await run(tool.cmd, tool.label, tool.manager, step);
//       if (res.exitCode === 124) timeouts = 2;
//     }
//     const ms = Date.now() - t0;
//     if (res.exitCode === 0) {
//       installed.add(tool.id);
//       if (tool.id === "tools/homebrew") out.homebrew = { ...HOMEBREW };
//       const road = roadOf(res.stdout);
//       const left = await freeBytes(machine, opts.prefix);
//       reading = left;
//       const bytes = free.kind === "free" && left.kind === "free" ? Math.max(0, free.bytes - left.bytes) : undefined;
//       await landed({ id: tool.id, label: tool.label, outcome: "installed", ...(tool.note !== undefined ? { note: tool.note } : {}), ms, ...(bytes !== undefined ? { bytes } : {}), ...(road !== undefined ? { road } : {}) });
//     } else {
//       await landed({ id: tool.id, label: tool.label, outcome: "failed", note: timeouts === 2 ? timedOutLine(limit, 2) : reasonOf(res, limit), ms });
//     }
//   }
//   await verifyCommands(machine, tools, out.tools, stage, path, opts.prefix);
//   await verifyChecks(machine, tools, out.tools, stage, path, opts.prefix);
//   const housekeeping = installed.has("tools/homebrew") ? await brewHousekeeping(machine, run, path, opts.prefix) : undefined;
//   stage(closing(summarize(out.tools, housekeeping), await sweep(), await freeNote(machine, opts.prefix)));
//   return out;
// }
const SWEEP_TMP_CMD: &str = "rm -f /tmp/wsp-vault-*.tgz";
const CELLAR_LOCKED: &str = "has already locked";
const BREW_LOCK_WAIT_S: u64 = 600;
const CHECK_FAILED: &str = "wsp-check";

fn brew_lock_wait_cmd() -> String {
    format!("for l in {BREW_PREFIX}/var/homebrew/locks/*.lock; do [ -e \"$l\" ] && flock -w {BREW_LOCK_WAIT_S} \"$l\" true; done; true")
}

fn road_named(manager: &str) -> Result<RoadName> {
    is_road(Some(manager))
        .ok_or_else(|| EngineError::Other { message: "Cannot read properties of undefined (reading 'limitS')".to_string(), code: None })
}

struct Loop<'a> {
    machine: &'a dyn Machine,
    on_stage: ToolStage,
    at: GoldenStage,
}

impl Loop<'_> {
    fn stage(&self, detail: &str) {
        (self.on_stage)(self.at, Some(detail));
    }

    async fn run(&self, cmd: &str, label: &str, road: RoadName) -> Result<ExecResult> {
        let on_stage = self.on_stage.clone();
        let at = self.at;
        let label = label.to_string();
        let on_line: OnLine = Arc::new(move |line: &str| on_stage(at, Some(&format!("{label}: {line}"))));
        let opts = RunOptions { deadline_ms: guard_deadline_ms(road_limit_s(road)), on_line: Some(on_line), ..Default::default() };
        self.machine.run(&guarded_road(road, cmd), opts).await
    }
}

async fn sweep(machine: &dyn Machine, keep_caches: bool, path: &str) -> Result<Option<String>> {
    if keep_caches {
        return Ok(None);
    }
    Ok(Some(sweep_caches(machine, path).await?))
}

async fn brew_housekeeping(lp: &Loop<'_>, path: &str, prefix: Option<&str>) -> Result<Option<String>> {
    let before = free_bytes(lp.machine, prefix).await?;
    let mut failed = Vec::new();
    for cmd in brew_housekeeping_cmds(path) {
        let res = lp.run(&cmd, "Homebrew cleanup", RoadName::Brew).await?;
        if res.exit_code.get() != 0 {
            failed.push(reason_of(&res, road_limit_s(RoadName::Brew) as f64));
        }
    }
    let after = free_bytes(lp.machine, prefix).await?;
    if !failed.is_empty() {
        return Ok(Some(format!("Homebrew cleanup failed ({})", failed.join("; "))));
    }
    Ok(match (before, after) {
        (FreeDisk::Free { bytes: before }, FreeDisk::Free { bytes: after }) if after > before => {
            Some(format!("Homebrew cleanup freed {}", fmt_bytes(after - before)))
        }
        _ => None,
    })
}

async fn verify_commands(lp: &Loop<'_>, tools: &[ToolInstall], out: &mut [ToolResult], path: &str, prefix: Option<&str>) -> Result<()> {
    let named: Vec<(usize, String)> = out
        .iter()
        .enumerate()
        .filter(|(_, r)| r.outcome == "installed")
        .filter_map(|(i, r)| tools.iter().find(|t| t.id == r.id).and_then(|t| t.bin.clone()).map(|bin| (i, bin)))
        .collect();
    if named.is_empty() {
        return Ok(());
    }
    let bins: Vec<String> = named.iter().map(|(_, bin)| bin.clone()).collect();
    let MissingCommands { missing, failed } = missing_commands(lp.machine, &bins, path, prefix).await?;
    if let Some(why) = &failed {
        lp.stage(&format!("the PATH check failed ({why}): {} count as failed", bins.join(", ")));
    }
    for (i, bin) in named {
        if failed.is_none() && !missing.contains(&bin) {
            continue;
        }
        let row = &mut out[i];
        row.outcome = "failed".to_string();
        row.note = Some(match &failed {
            None => format!("{bin} is not on PATH after the install"),
            Some(why) => format!("{bin} could not be checked on PATH: {why}"),
        });
        row.road = None;
    }
    Ok(())
}

// TS golden-tools.ts:287-345 (verifyChecks, above): one page of reads per exec, the rows of a failed page named.
#[derive(Clone)]
struct CheckRow {
    idx: usize,
    label: String,
    check: String,
}

async fn verify_checks(lp: &Loop<'_>, tools: &[ToolInstall], out: &mut [ToolResult], path: &str, prefix: Option<&str>) -> Result<()> {
    let checked: Vec<CheckRow> = out
        .iter()
        .enumerate()
        .filter(|(_, r)| r.outcome == "installed")
        .filter_map(|(i, r)| {
            tools.iter().find(|t| t.id == r.id).and_then(|t| t.check.clone()).map(|check| CheckRow {
                idx: i,
                label: r.label.clone(),
                check,
            })
        })
        .collect();
    if checked.is_empty() {
        return Ok(());
    }
    let head = path_line(path, prefix);
    let pages = paged_reads(
        lp.machine,
        &checked,
        |c, at| {
            format!(
                "if ! out=\"$( ( {} ) 2>&1 )\"; then printf 'wsp-check %s %s\\n' {at} \"$(printf '%s' \"$out\" | tail -1)\"; fi",
                c.check
            )
        },
        &head,
    )
    .await?;
    for page in pages {
        if page.res.exit_code.get() != 0 {
            let why = reason_of(&page.res, inline_s());
            let labels: Vec<String> = page.rows.iter().map(|c| c.label.clone()).collect();
            lp.stage(&format!("the checks could not be run ({why}): {} count as failed", name_list(&labels)));
            for c in &page.rows {
                let row = &mut out[c.idx];
                row.outcome = "failed".to_string();
                row.note = Some(format!("the check could not be run: {why}"));
                lp.stage(&format!("{}: its check was {}", c.label, c.check));
            }
            continue;
        }
        let failed = markers_of(&page.res.stdout, CHECK_FAILED);
        for (at, c) in page.rows.iter().enumerate() {
            let Some(why) = failed.get(&at.to_string()) else { continue };
            let row = &mut out[c.idx];
            row.outcome = "failed".to_string();
            row.note = Some(if why.is_empty() { format!("{} is not there after the install", c.label) } else { why.clone() });
            lp.stage(&format!("{}: the check did not pass: {}", c.label, c.check));
        }
    }
    Ok(())
}

fn row(tool: &ToolInstall, outcome: &str, note: Option<String>) -> ToolResult {
    ToolResult { id: tool.id.clone(), label: tool.label.clone(), outcome: outcome.to_string(), note, ms: None, bytes: None, road: None }
}

async fn landed(rows: &mut Vec<ToolResult>, on_tool: Option<&OnTool>, result: ToolResult) {
    rows.push(result.clone());
    if let Some(on_tool) = on_tool {
        on_tool(result).await;
    }
}

async fn cleanup(
    lp: &Loop<'_>,
    keep_caches: bool,
    path: &str,
    prefix: Option<&str>,
    installed: &HashSet<String>,
    why: &str,
) -> Result<FreeDisk> {
    lp.stage(&format!("{why}; cleaning up before skipping"));
    let brew = if installed.contains("tools/homebrew") { brew_housekeeping(lp, path, prefix).await? } else { None };
    let swept = sweep(lp.machine, keep_caches, path).await?;
    let after = free_bytes(lp.machine, prefix).await?;
    let reading = match &after {
        FreeDisk::Free { bytes } => format!("{} free", fmt_bytes(*bytes)),
        FreeDisk::Unknown { reason } => reason.clone(),
    };
    lp.stage(&closing(&[brew.as_deref(), swept.as_deref(), Some(&reading)]));
    Ok(after)
}

fn floor_note(reading: &str, keep: f64) -> String {
    format!("{reading}, keeping {} free", fmt_bytes(keep))
}

fn too_big(free: &FreeDisk, need: Option<u64>, keep: f64) -> Option<(f64, f64)> {
    match (free, need) {
        (FreeDisk::Free { bytes }, Some(need)) => {
            let need = need as f64;
            (bytes - need < keep).then_some((*bytes, need))
        }
        _ => None,
    }
}

/// golden-tools.ts:379 `installTools(machine, tools, onStage, at, opts)`. Every stage line goes out as `(at, detail)`.
/// A rejected exec or run anywhere in the loop is the `Err`, and the loop stops there.
pub async fn install_tools_at(
    machine: &dyn Machine,
    tools: &[ToolInstall],
    on_stage: ToolStage,
    at: GoldenStage,
    opts: InstallToolsOptions,
) -> Result<ToolsOutcome> {
    let lp = Loop { machine, on_stage, at };
    let path = opts.path.clone().unwrap_or_else(|| TOOLS_PATH.to_string());
    let prefix = opts.prefix.clone();
    let prefix = prefix.as_deref();
    let keep = opts.floor.unwrap_or(TOOLS_DISK_FLOOR) as f64;
    let on_tool = opts.on_tool.as_ref();
    machine.exec(SWEEP_TMP_CMD, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await?;
    let mut out = ToolsOutcome::default();
    let mut installed: HashSet<String> = HashSet::new();
    let label_of = |id: &str| tools.iter().find(|t| t.id == id).map_or_else(|| id.to_string(), |t| t.label.clone());
    let mut floor: Option<String> = None;
    let mut df_warned = false;
    let mut cleaned_at_floor = false;
    let mut reading: Option<FreeDisk> = None;
    for (i, tool) in tools.iter().enumerate() {
        if opts.present.contains(&tool.id) {
            installed.insert(tool.id.clone());
            landed(&mut out.tools, on_tool, row(tool, "installed", Some(ALREADY_ON_MACHINE.to_string()))).await;
            continue;
        }
        if let Some(after) = tool.after.as_ref().filter(|after| !installed.contains(*after)) {
            let note = format!("{} did not install", label_of(after));
            landed(&mut out.tools, on_tool, row(tool, "skipped", Some(note))).await;
            continue;
        }
        if let Some(words) = &floor {
            landed(&mut out.tools, on_tool, row(tool, "skipped", Some(words.clone()))).await;
            continue;
        }
        let mut free = match reading.take() {
            Some(r) => r,
            None => free_bytes(machine, prefix).await?,
        };
        if let FreeDisk::Unknown { reason } = &free {
            if !df_warned {
                df_warned = true;
                lp.stage(&format!("free disk unknown ({reason}); installing without the {} floor", fmt_bytes(keep)));
            }
        } else if let FreeDisk::Free { bytes } = free {
            if bytes < keep {
                let after = if cleaned_at_floor {
                    None
                } else {
                    cleaned_at_floor = true;
                    let why = format!("{} free, under the {} floor", fmt_bytes(bytes), fmt_bytes(keep));
                    Some(cleanup(&lp, opts.keep_caches, &path, prefix, &installed, &why).await?)
                };
                match after {
                    Some(FreeDisk::Free { bytes: now }) if now >= keep => free = FreeDisk::Free { bytes: now },
                    other => {
                        let words = match other {
                            None => format!("{} free", fmt_bytes(bytes)),
                            Some(FreeDisk::Free { bytes: now }) => format!("{} free after cleanup", fmt_bytes(now)),
                            Some(FreeDisk::Unknown { reason }) => {
                                format!("{} free before cleanup and unknown after ({reason})", fmt_bytes(bytes))
                            }
                        };
                        let note = floor_note(&words, keep);
                        floor = Some(note.clone());
                        landed(&mut out.tools, on_tool, row(tool, "skipped", Some(note))).await;
                        continue;
                    }
                }
            }
        }
        if let Some((have, need)) = too_big(&free, tool.bytes, keep) {
            let why =
                format!("{} needs about {} with {} free above a {} floor", tool.label, fmt_bytes(need), fmt_bytes(have), fmt_bytes(keep));
            let after = if cleaned_at_floor {
                None
            } else {
                cleaned_at_floor = true;
                Some(cleanup(&lp, opts.keep_caches, &path, prefix, &installed, &why).await?)
            };
            if let Some(after @ FreeDisk::Free { .. }) = after {
                if too_big(&after, tool.bytes, keep).is_none() {
                    free = after;
                }
            }
        }
        if let Some((have, need)) = too_big(&free, tool.bytes, keep) {
            reading = Some(free.clone());
            let note = floor_note(&format!("needs about {}, {} free", fmt_bytes(need), fmt_bytes(have)), keep);
            landed(&mut out.tools, on_tool, row(tool, "skipped", Some(note))).await;
            continue;
        }
        let road = road_named(&tool.manager)?;
        let limit = road_limit_s(road);
        lp.stage(&format!("{} ({}/{})", tool.label, i + 1, tools.len()));
        let t0 = Instant::now();
        let mut res = lp.run(&tool.cmd, &tool.label, road).await?;
        if res.exit_code.get() != 0 && res.stderr.contains(CELLAR_LOCKED) {
            lp.stage(&format!("{}: another brew holds its cellar; waiting for it, then once more", tool.label));
            machine.run(&brew_lock_wait_cmd(), RunOptions { deadline_ms: (BREW_LOCK_WAIT_S + 60) * 1000, ..Default::default() }).await?;
            res = lp.run(&tool.cmd, &tool.label, road).await?;
        }
        let mut timeouts = if res.exit_code.get() == 124 { 1 } else { 0 };
        if timeouts == 1 && road_step(road).retry {
            lp.stage(&format!("{}: {}", tool.label, step_retry_line(limit as f64)));
            res = lp.run(&tool.cmd, &tool.label, road).await?;
            if res.exit_code.get() == 124 {
                timeouts = 2;
            }
        }
        let ms = t0.elapsed().as_millis() as i64;
        if res.exit_code.get() == 0 {
            installed.insert(tool.id.clone());
            if tool.id == "tools/homebrew" {
                out.homebrew = Some(HomebrewCheckout { tag: HOMEBREW_TAG.to_string(), commit: HOMEBREW_COMMIT.to_string() });
            }
            let road_seen = road_of(&res.stdout);
            let left = free_bytes(machine, prefix).await?;
            let bytes = match (&free, &left) {
                (FreeDisk::Free { bytes: had }, FreeDisk::Free { bytes: now }) => Some((had - now).max(0.0) as i64),
                _ => None,
            };
            reading = Some(left);
            let result = ToolResult {
                id: tool.id.clone(),
                label: tool.label.clone(),
                outcome: "installed".to_string(),
                note: tool.note.clone(),
                ms: Some(ms),
                bytes,
                road: road_seen,
            };
            landed(&mut out.tools, on_tool, result).await;
        } else {
            let note = if timeouts == 2 { timed_out_line(limit as f64, Some(2.0)) } else { reason_of(&res, limit as f64) };
            let mut failed = row(tool, "failed", Some(note));
            failed.ms = Some(ms);
            landed(&mut out.tools, on_tool, failed).await;
        }
    }
    verify_commands(&lp, tools, &mut out.tools, &path, prefix).await?;
    verify_checks(&lp, tools, &mut out.tools, &path, prefix).await?;
    let housekeeping = if installed.contains("tools/homebrew") { brew_housekeeping(&lp, &path, prefix).await? } else { None };
    let summary = summarize(&out.tools, housekeeping.as_deref());
    let swept = sweep(machine, opts.keep_caches, &path).await?;
    let note = free_note(machine, prefix).await?;
    lp.stage(&closing(&[Some(summary.as_str()), swept.as_deref(), note.as_deref()]));
    Ok(out)
}

/// golden-tools.ts:379 `installTools(machine, tools, onStage, "installing-tools", opts).tools`, as provision runs it.
pub async fn install_tools(
    machine: &dyn Machine,
    tools: &[ToolInstall],
    on_stage: ToolStage,
    opts: InstallToolsOptions,
) -> Result<Vec<ToolResult>> {
    Ok(install_tools_at(machine, tools, on_stage, GoldenStage::InstallingTools, opts).await?.tools)
}
