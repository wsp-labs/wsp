// SPDX-License-Identifier: AGPL-3.0-only
//! Unit tools_loop (spec/golden-tools_loop.md): golden-tools.ts:237-345 and 374-490 at a4033dc34. The one tools loop:
//! every install as one guarded run in plan order, the disk read before each against the floor, the cleanup at the
//! first reading under it, the checks after, Homebrew's housekeeping and the closing line.
use super::{GoldenStage, InstallToolsOptions, ToolInstall, ToolResult, ToolStage};
use crate::errors::Result;
use crate::machine::Machine;

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
/// golden-tools.ts:379 `installTools(machine, tools, onStage, at, opts)`. Every stage line goes out as `(at, detail)`.
/// A rejected exec or run anywhere in the loop is the `Err`, and the loop stops there.
pub async fn install_tools_at(
    machine: &dyn Machine,
    tools: &[ToolInstall],
    on_stage: ToolStage,
    at: GoldenStage,
    opts: InstallToolsOptions,
) -> Result<ToolsOutcome> {
    todo!()
}

/// golden-tools.ts:379 `installTools(machine, tools, onStage, "installing-tools", opts).tools`, as provision runs it.
pub async fn install_tools(
    machine: &dyn Machine,
    tools: &[ToolInstall],
    on_stage: ToolStage,
    opts: InstallToolsOptions,
) -> Result<Vec<ToolResult>> {
    todo!()
}
