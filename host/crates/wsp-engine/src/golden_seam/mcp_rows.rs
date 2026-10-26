// SPDX-License-Identifier: AGPL-3.0-only
//! Unit mcp_rows (spec/golden-mcp_rows.md): golden-mcp.ts:337-566 at a4033dc34. Which kept servers' commands the
//! machine lacks, the plan with those dropped, every server as a row with the words a person reads, and the builder's
//! whole round.
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use wsp_catalog::catalog::{base_floor, catalog_entry, catalog_id_of_row};
use wsp_catalog::roads::{RoadName, UV_INSTALL};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::agents_report::config_unread_line;

use super::import_roads::CUSTOM_PREFIX;
use super::mcp_files::{edit_scopes, land_configs, parse_configs, read_configs_cmd, read_failed, refused, scope_row_id, ReadAsk, READ_MS};
use super::tools_disk::{closing, free_note, guard_deadline_ms, guarded_road, reason_of, road_limit_s};
use super::{
    DropName, EditedName, EditedOutcome, GoldenStage, McpAgentPlan, McpPlan, McpResult, McpToolRow, ScopeOutcome, StageListener,
    ToolInstall, ToolResult, TOOLS_PATH,
};
use crate::exec_detached::INLINE_EXEC_MS;
use crate::machine::{ExecOptions, Machine, OnLine, RunOptions};

/// golden-mcp.ts:348 `FETCHERS`: the runner's basename and who fetches for it (`npx` npx, `uvx` uv, `uv` uv), in the
/// TS's order.
pub fn fetchers() -> IndexMap<&'static str, &'static str> {
    IndexMap::from([("npx", "npx"), ("uvx", "uv"), ("uv", "uv")])
}

// TS golden-mcp.ts:371-380
// /** The servers by outcome for the stage's end line, `11 installed, 9 skipped, 2 on first use`, the zero counts left
//  * out; nothing when the plan named no server. */
// export function mcpTally(rows: readonly McpResult[]): string | undefined {
//   const words: [McpResult["outcome"], string][] = [["installed", "installed"], ["skipped", "skipped"], ["fetched-on-first-use", "on first use"]];
//   const said = words.flatMap(([outcome, word]) => {
//     const n = rows.filter(r => r.outcome === outcome).length;
//     return n > 0 ? [`${n} ${word}`] : [];
//   });
//   return said.length > 0 ? said.join(", ") : undefined;
// }
/// golden-mcp.ts:373 `mcpTally`: `<n> installed, <n> skipped, <n> on first use`, zero counts left out; `None` for none.
pub fn mcp_tally(rows: &[McpResult]) -> Option<String> {
    let words = [("installed", "installed"), ("skipped", "skipped"), ("fetched-on-first-use", "on first use")];
    let said: Vec<String> = words
        .iter()
        .filter_map(|(outcome, word)| {
            let n = rows.iter().filter(|r| r.outcome == *outcome).count();
            (n > 0).then(|| format!("{n} {word}"))
        })
        .collect();
    (!said.is_empty()).then(|| said.join(", "))
}

/// A row `command_row` and `server_needs` read: its id and the command it names, where it names one.
pub trait CommandRowItem {
    fn row_id(&self) -> &str;
    fn row_bin(&self) -> Option<&str>;
}

impl CommandRowItem for McpToolRow {
    fn row_id(&self) -> &str {
        &self.id
    }
    fn row_bin(&self) -> Option<&str> {
        None
    }
}

impl CommandRowItem for ToolInstall {
    fn row_id(&self) -> &str {
        &self.id
    }
    fn row_bin(&self) -> Option<&str> {
        self.bin.as_deref()
    }
}

// TS golden-mcp.ts:384-397
// const tail = (id: string): string => id.slice(id.lastIndexOf("/") + 1);
//
// /** The commands a tool row puts on that the row or the catalog names: its own, its catalog entry's and what that brings. */
// function namedCommands(row: { id: string; bin?: string }): string[] {
//   const entry = catalogEntry(catalogIdOfRow(row) ?? "");
//   return [row.bin, entry?.bin, ...(entry?.kind === "tool" ? (entry.brings ?? []).map(b => b.bin) : [])].filter((b): b is string => b !== undefined);
// }
//
// /** The row that puts a command on: the one whose named commands hold it, else a row installing one binary whose last
//  * word it is. */
// export function commandRow<R extends { id: string; bin?: string }>(command: string, rows: readonly R[]): R | undefined {
//   const name = basename(command);
//   return rows.find(r => namedCommands(r).includes(name)) ?? rows.find(r => BINARY_ROW.test(r.id) && tail(r.id) === name);
// }
/// golden-mcp.ts:394 `commandRow`.
pub fn command_row<'a, R: CommandRowItem>(command: &str, rows: &'a [R]) -> Option<&'a R> {
    let name = basename(command);
    rows.iter()
        .find(|r| named_commands(*r).iter().any(|n| n == name))
        .or_else(|| rows.iter().find(|r| is_binary_row(r.row_id()) && basename(r.row_id()) == name))
}

// TS golden-mcp.ts:399-408
// /** The rows a kept server needs on before the servers round looks for its command, which drops one that is not
//  * there: none for a server run through npx, uvx or uv, which is never dropped, or by a command the floor or `ready`
//  * puts on first; the row that puts its command on; else every row putting on a command nothing names, a binary row
//  * or one the person wrote, since any of them may be it. */
// export function serverNeeds<R extends { id: string; bin?: string }>(command: string, rows: readonly R[], ready: ReadonlySet<string> = new Set()): R[] {
//   const name = basename(command);
//   if (FETCHERS[name] !== undefined || ready.has(name) || BASE_FLOOR.some(e => e.bin === name || (e.brings ?? []).some(b => b.bin === name))) return [];
//   const row = commandRow(command, rows);
//   return row !== undefined ? [row] : rows.filter(r => (BINARY_ROW.test(r.id) || r.id.startsWith(CUSTOM_PREFIX)) && namedCommands(r).length === 0);
// }
/// golden-mcp.ts:403 `serverNeeds`, in `rows`' order.
pub fn server_needs<'a, R: CommandRowItem>(command: &str, rows: &'a [R], ready: &IndexSet<String>) -> Vec<&'a R> {
    let name = basename(command);
    let floor = base_floor();
    if fetchers().contains_key(name)
        || ready.contains(name)
        || floor.iter().any(|e| e.bin == name || e.brings.iter().flatten().any(|b| b.bin == name))
    {
        return Vec::new();
    }
    match command_row(command, rows) {
        Some(row) => vec![row],
        None => rows
            .iter()
            .filter(|r| (is_binary_row(r.row_id()) || r.row_id().starts_with(CUSTOM_PREFIX)) && named_commands(*r).is_empty())
            .collect(),
    }
}

// TS golden-mcp.ts:410-420
// /** Why a server whose command the machine does not have is skipped: when the recipe has a tool row for that
//  * binary, what became of the row. */
// export function absentReason(command: string, plan: McpPlan, tools: readonly ToolResult[]): string {
//   const base = "command not on the machine";
//   const row = commandRow(command, plan.tools);
//   if (row === undefined) return base;
//   if (!row.ticked) return `${base}; ${row.id} was not ticked${row.reason !== undefined ? ` (${row.reason})` : ""}`;
//   const result = tools.find(t => t.id === row.id);
//   if (result?.outcome === "skipped" || result?.outcome === "failed") return `${base}; ${row.id} ${result.outcome === "failed" ? "failed" : "was skipped"}${result.note !== undefined ? ` (${result.note})` : ""}`;
//   return `${base}; ${row.id} was ticked, but nothing by that name is on PATH`;
// }
/// golden-mcp.ts:412 `absentReason`.
pub fn absent_reason(command: &str, plan: &McpPlan, tools: &[ToolResult]) -> String {
    let base = "command not on the machine";
    let Some(row) = command_row(command, &plan.tools) else {
        return base.to_string();
    };
    if !row.ticked {
        let reason = row.reason.as_ref().map(|r| format!(" ({r})")).unwrap_or_default();
        return format!("{base}; {} was not ticked{reason}", row.id);
    }
    match tools.iter().find(|t| t.id == row.id) {
        Some(result) if result.outcome == "skipped" || result.outcome == "failed" => {
            let verb = if result.outcome == "failed" { "failed" } else { "was skipped" };
            let note = result.note.as_ref().map(|n| format!(" ({n})")).unwrap_or_default();
            format!("{base}; {} {verb}{note}", row.id)
        }
        _ => format!("{base}; {} was ticked, but nothing by that name is on PATH", row.id),
    }
}

/// golden-mcp.ts:423 `asRun`: a leading `~/` is the plan's guest home.
pub fn as_run(plan: &McpPlan, command: &str) -> String {
    match command.strip_prefix("~/") {
        Some(rest) => format!("{}/{rest}", plan.guest_home),
        None => command.to_string(),
    }
}

// TS golden-mcp.ts:425-437
// /** Which commands of a plan's kept servers the machine does not have on its tools PATH, asked in one call; none
//  * when no kept server names a command, and none when the call itself did not answer. */
// export async function absentCommands(machine: Machine, plan: McpPlan, read: readonly ScopeOutcome[], path: string = TOOLS_PATH): Promise<string[]> {
//   const commands = [...new Set(read.flatMap(s => s.results.flatMap(r => (r.command !== undefined ? [asRun(plan, r.command)] : []))))];
//   if (commands.length === 0) return [];
//   const check = await machine
//     .exec(`export PATH=${path}\n${commands.map(c => `if command -v ${shellQuote(c)} >/dev/null 2>&1; then echo ${shellQuote(`ok ${c}`)}; else echo ${shellQuote(`no ${c}`)}; fi`).join("\n")}`, { timeoutMs: INLINE_EXEC_MS })
//     .catch(refused);
//   return check.stdout
//     .split("\n")
//     .filter(line => line.startsWith("no "))
//     .map(line => line.slice(3));
// }
/// golden-mcp.ts:427 `absentCommands(machine, plan, read, path)`: the commands the check printed `no ` for, in its
/// order; nothing asked when no kept server names a command; a rejected exec reads as a refused one. Never fails.
pub async fn absent_commands(machine: &dyn Machine, plan: &McpPlan, read: &[ScopeOutcome], path: &str) -> Vec<String> {
    let mut commands: IndexSet<String> = IndexSet::new();
    for outcome in read {
        for r in &outcome.results {
            if let Some(command) = &r.command {
                commands.insert(as_run(plan, command));
            }
        }
    }
    if commands.is_empty() {
        return Vec::new();
    }
    let checks: Vec<String> = commands
        .iter()
        .map(|c| {
            format!(
                "if command -v {} >/dev/null 2>&1; then echo {}; else echo {}; fi",
                shell_quote(c),
                shell_quote(&format!("ok {c}")),
                shell_quote(&format!("no {c}"))
            )
        })
        .collect();
    let script = format!("export PATH={path}\n{}", checks.join("\n"));
    let check = match machine.exec(&script, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await {
        Ok(res) => res,
        Err(e) => refused(&e.to_string()),
    };
    check.stdout.split('\n').filter_map(|line| line.strip_prefix("no ").map(str::to_string)).collect()
}

// TS golden-mcp.ts:439-456
// /** The agents with every kept server whose command is neither on the machine nor fetched on first use moved to
//  * the drops, with its reason. */
// export function withoutAbsent(plan: McpPlan, agents: readonly McpAgentPlan[], read: readonly ScopeOutcome[], missing: ReadonlySet<string>, tools: readonly ToolResult[]): McpAgentPlan[] {
//   let at = 0;
//   return agents.map(agent => ({
//     ...agent,
//     scopes: agent.scopes.map(scope => {
//       const results = read[at++]?.results ?? [];
//       const commandOf = (name: string): string | undefined => results.find(r => r.name === name)?.command;
//       const absent = scope.keep.filter(name => {
//         const command = commandOf(name);
//         return command !== undefined && FETCHERS[basename(command)] === undefined && missing.has(asRun(plan, command));
//       });
//       if (absent.length === 0) return scope;
//       return { ...scope, keep: scope.keep.filter(name => !absent.includes(name)), drop: [...scope.drop, ...absent.map(name => ({ name, reason: absentReason(commandOf(name)!, plan, tools) }))] };
//     }),
//   }));
// }
/// golden-mcp.ts:441 `withoutAbsent`.
pub fn without_absent(
    plan: &McpPlan,
    agents: &[McpAgentPlan],
    read: &[ScopeOutcome],
    missing: &IndexSet<String>,
    tools: &[ToolResult],
) -> Vec<McpAgentPlan> {
    let mut at = 0;
    let mut out = Vec::with_capacity(agents.len());
    for agent in agents {
        let mut scopes = Vec::with_capacity(agent.scopes.len());
        for scope in &agent.scopes {
            let results: &[EditedName] = read.get(at).map_or(&[], |o| o.results.as_slice());
            at += 1;
            let command_of = |name: &str| results.iter().find(|r| r.name == name).and_then(|r| r.command.clone());
            let absent: Vec<String> = scope
                .keep
                .iter()
                .filter(|name| {
                    command_of(name.as_str()).is_some_and(|c| !fetchers().contains_key(basename(&c)) && missing.contains(&as_run(plan, &c)))
                })
                .cloned()
                .collect();
            if absent.is_empty() {
                scopes.push(scope.clone());
                continue;
            }
            let mut next = scope.clone();
            next.keep.retain(|n| !absent.contains(n));
            for name in &absent {
                let command = command_of(name.as_str()).unwrap_or_default();
                next.drop.push(DropName { name: name.clone(), reason: absent_reason(&command, plan, tools), keys: None });
            }
            scopes.push(next);
        }
        out.push(McpAgentPlan { scopes, ..agent.clone() });
    }
    out
}

/// golden-mcp.ts:459 `mcpOpening`: `<label> <count>` per agent, joined with `, `.
pub fn mcp_opening(agents: &[McpAgentPlan]) -> String {
    agents
        .iter()
        .map(|a| {
            let kept: usize = a.scopes.iter().map(|s| s.keep.len() + s.drop.len()).sum();
            format!("{} {}", a.label, kept + a.aside.len())
        })
        .collect::<Vec<String>>()
        .join(", ")
}

/// golden-mcp.ts:468 `mcpRows`'s options.
#[derive(Clone)]
pub struct McpRowsOptions {
    pub agents: Vec<McpAgentPlan>,
    pub report: Option<Vec<ScopeOutcome>>,
    pub failure: Option<String>,
    pub missing: IndexSet<String>,
    pub present: IndexSet<String>,
    pub stage: StageListener,
}

// TS golden-mcp.ts:337-369
// interface Pending extends McpResult {
//   /** The command as the machine will run it; absent for a remote server or a skipped row. */
//   command?: string;
//   /** Full sentences for the saved result. */
//   notes: string[];
//   /** The summary's words, one per note. */
//   shorts: string[];
// }
//
// const basename = (p: string): string => p.slice(p.lastIndexOf("/") + 1);
// /** Runners that pull the server's package down when the agent first starts it: the definition is in place, the package is not. */
// export const FETCHERS: Record<string, string> = { npx: "npx", uvx: "uv", uv: "uv" };
//
// /** The servers that were already there by name, then the ones installed; then, per distinct wording, the servers
//  * whose package is fetched on first use or whose runner was installed or is missing; then each skipped server with
//  * its reason. `present` is the rows the round read as already there, and a server in it is said that way and no
//  * other, so these words never read installed one line above a row reading present. */
// function summarize(rows: readonly Pending[], present: ReadonlySet<string>): string {
//   const parts: string[] = [];
//   const already = rows.filter(r => present.has(r.id));
//   if (already.length > 0) parts.push(`${already.map(r => r.name).join(", ")} already there`);
//   const installed = rows.filter(r => r.outcome === "installed" && r.shorts.length === 0 && !present.has(r.id));
//   if (installed.length > 0) parts.push(`${installed.map(r => r.name).join(", ")} installed`);
//   const byNote = new Map<string, string[]>();
//   for (const r of rows) {
//     if (r.outcome === "skipped" || r.shorts.length === 0 || present.has(r.id)) continue;
//     const key = r.shorts.join(", ");
//     (byNote.get(key) ?? byNote.set(key, []).get(key)!).push(r.name);
//   }
//   for (const [short, names] of byNote) parts.push(`${names.join(", ")}: ${short}`);
//   for (const r of rows) if (r.outcome === "skipped") parts.push(`${r.name} skipped (${r.notes[0] ?? "no reason given"})`);
//   return parts.length > 0 ? parts.join("; ") : "nothing to do";
// }
// TS golden-mcp.ts:382-384
// const strip = (r: Pending): McpResult => ({ id: r.id, agent: r.agent, name: r.name, outcome: r.outcome, ...(r.notes.length > 0 ? { note: r.notes.join("; ") } : {}), ...(r.keys !== undefined ? { keys: r.keys } : {}) });
//
// const tail = (id: string): string => id.slice(id.lastIndexOf("/") + 1);
// TS golden-mcp.ts:462-534
// /** Every server of a plan as a row, and the words a person reads for them, whichever road wrote the configs: one
//  * row per kept name, per dropped name and per name set aside, uv installed where a kept server runs through it and
//  * is missing, each server whose command the machine does not have named on its row, and the closing line. `report`
//  * is what became of each scope, in plan order; `failure` is a sentence every kept server is skipped with, for a
//  * round that never got its configs off the machine or back onto it; `present` is the rows the caller reads as
//  * already there, which the closing words then say the same way. */
// export async function mcpRows(
//   machine: Machine,
//   plan: McpPlan,
//   o: { agents: readonly McpAgentPlan[]; report?: readonly ScopeOutcome[]; failure?: string; missing: ReadonlySet<string>; present?: ReadonlySet<string>; stage: StageListener },
// ): Promise<McpResult[]> {
//   const { agents, report, failure, missing, stage } = o;
//   const rows: Pending[] = [];
//   let at = 0;
//   for (const agent of agents) {
//     for (const scope of agent.scopes) {
//       const outcome = report?.[at++];
//       const id = (name: string): string => scopeRowId(agent.id, scope, name);
//       const skippedKeep =
//         failure ??
//         (outcome === undefined || outcome.file === null
//           ? `${agent.label}'s config is not on the machine`
//           : outcome.unread !== undefined
//             ? configUnreadLine(outcome.file, outcome.unread)
//             : outcome.error !== undefined
//               ? `${agent.label}'s config on the machine did not parse (${outcome.error})`
//               : undefined);
//       for (const name of scope.keep) {
//         const r = outcome?.results.find(x => x.name === name);
//         if (skippedKeep !== undefined || r === undefined || r.outcome === "missing") {
//           rows.push(skipped(id(name), agent.label, name, skippedKeep ?? "not in the config that travelled"));
//           continue;
//         }
//         const fetcher = r.command !== undefined ? FETCHERS[basename(r.command)] : undefined;
//         rows.push({
//           id: id(name), agent: agent.label, name,
//           outcome: fetcher === undefined ? "installed" : "fetched-on-first-use",
//           ...(r.command !== undefined ? { command: r.command } : {}),
//           notes: fetcher === undefined ? [] : [`${fetcher} fetches the package on first use`],
//           shorts: fetcher === undefined ? [] : [`package fetched on first use by ${fetcher}`],
//         });
//       }
//       for (const d of scope.drop) rows.push(skipped(id(d.name), agent.label, d.name, d.reason, d.keys));
//     }
//     for (const a of agent.aside) rows.push(skipped(a.id, agent.label, a.name, a.reason));
//   }
//
//   const viaUv = rows.filter(r => r.command !== undefined && ["uv", "uvx"].includes(basename(r.command)) && missing.has(asRun(plan, r.command)));
//   if (viaUv.length > 0) {
//     stage("installing-mcp", `uv for ${viaUv.map(r => r.name).join(", ")}`);
//     // uv's catalog row walks the script road, so the road carries the strict shell line and uv's own limit; the stage writes neither.
//     const install = await machine.run(guardedRoad("script", UV_INSTALL), { deadlineMs: guardDeadlineMs(roadLimitS("script")), onLine: line => stage("installing-mcp", `uv: ${line}`) }).catch(refused);
//     for (const r of viaUv) {
//       if (install.exitCode === 0) {
//         r.notes.unshift("uv installed for it");
//         r.shorts.unshift("uv installed");
//       } else {
//         const short = `uv did not install (${reasonOf(install, roadLimitS("script"))})`;
//         r.shorts.unshift(short);
//         r.notes.unshift(`${short}; the server starts once it is installed there`);
//       }
//     }
//   }
//   const viaUvNames = new Set(viaUv.map(r => r.id));
//   for (const r of rows) {
//     if (r.command === undefined || viaUvNames.has(r.id) || !missing.has(asRun(plan, r.command))) continue;
//     const short = `${basename(r.command)} is not on the machine`;
//     r.shorts.push(short);
//     r.notes.push(`${short}; the server starts once it is installed there`);
//   }
//   stage("installing-mcp", closing(summarize(rows, o.present ?? new Set()), await freeNote(machine).catch(() => undefined)));
//   return rows.map(strip);
// }
// TS golden-mcp.ts:566-566
// const skipped = (id: string, agent: string, name: string, note: string, keys?: string[]): Pending => ({ id, agent, name, outcome: "skipped", notes: [note], shorts: [], ...(keys !== undefined ? { keys } : {}) });
/// golden-mcp.ts:468 `mcpRows`. Never fails: the uv install's rejection reads as a refused run, and a df that
/// rejects leaves the free reading off the closing line.
pub async fn mcp_rows(machine: &dyn Machine, plan: &McpPlan, o: McpRowsOptions) -> Vec<McpResult> {
    let McpRowsOptions { agents, report, failure, missing, present, stage } = o;
    let mut rows: Vec<Pending> = Vec::new();
    let mut at = 0;
    for agent in &agents {
        for scope in &agent.scopes {
            let outcome = report.as_ref().and_then(|r| r.get(at));
            at += 1;
            let id = |name: &str| scope_row_id(&agent.id, scope.project.as_ref(), scope.folder.as_deref(), name);
            let not_on_machine = format!("{}'s config is not on the machine", agent.label);
            let skipped_keep = failure.clone().or_else(|| match outcome {
                Some(ScopeOutcome { file: Some(file), unread: Some(why), .. }) => Some(config_unread_line(file, why)),
                Some(ScopeOutcome { file: Some(_), error: Some(error), .. }) => {
                    Some(format!("{}'s config on the machine did not parse ({error})", agent.label))
                }
                Some(ScopeOutcome { file: Some(_), .. }) => None,
                _ => Some(not_on_machine),
            });
            for name in scope.keep.iter().map(String::as_str) {
                let found = outcome.and_then(|o| o.results.iter().find(|x| x.name == name));
                let Some(r) = found.filter(|r| skipped_keep.is_none() && r.outcome != EditedOutcome::Missing) else {
                    rows.push(skipped(
                        id(name),
                        &agent.label,
                        name,
                        skipped_keep.as_deref().unwrap_or("not in the config that travelled"),
                        None,
                    ));
                    continue;
                };
                let fetcher = r.command.as_deref().and_then(|c| fetchers().get(basename(c)).copied());
                rows.push(Pending {
                    id: id(name),
                    agent: agent.label.clone(),
                    name: name.to_string(),
                    outcome: if fetcher.is_some() { "fetched-on-first-use" } else { "installed" }.to_string(),
                    command: r.command.clone(),
                    notes: fetcher.map(|f| format!("{f} fetches the package on first use")).into_iter().collect(),
                    shorts: fetcher.map(|f| format!("package fetched on first use by {f}")).into_iter().collect(),
                    keys: None,
                });
            }
            for d in &scope.drop {
                rows.push(skipped(id(d.name.as_str()), &agent.label, &d.name, &d.reason, d.keys.clone()));
            }
        }
        for a in &agent.aside {
            rows.push(skipped(a.id.clone(), &agent.label, &a.name, &a.reason, None));
        }
    }

    let via_uv: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.command.as_deref().is_some_and(|c| matches!(basename(c), "uv" | "uvx") && missing.contains(&as_run(plan, c))))
        .map(|(i, _)| i)
        .collect();
    if !via_uv.is_empty() {
        let names: Vec<&str> = via_uv.iter().map(|&i| rows[i].name.as_str()).collect();
        let text = format!("uv for {}", names.join(", "));
        stage(GoldenStage::InstallingMcp, Some(text.as_str()));
        let line_stage = stage.clone();
        let on_line: OnLine = Arc::new(move |line: &str| {
            let text = format!("uv: {line}");
            line_stage(GoldenStage::InstallingMcp, Some(text.as_str()));
        });
        let script = guarded_road(RoadName::Script, &UV_INSTALL);
        let install = match machine
            .run(
                &script,
                RunOptions { deadline_ms: guard_deadline_ms(road_limit_s(RoadName::Script)), on_line: Some(on_line), ..Default::default() },
            )
            .await
        {
            Ok(res) => res,
            Err(e) => refused(&e.to_string()),
        };
        let limit = road_limit_s(RoadName::Script) as f64;
        for &i in &via_uv {
            let row = &mut rows[i];
            if install.exit_code.get() == 0 {
                row.notes.insert(0, "uv installed for it".to_string());
                row.shorts.insert(0, "uv installed".to_string());
            } else {
                let short = format!("uv did not install ({})", reason_of(&install, limit));
                row.notes.insert(0, format!("{short}; the server starts once it is installed there"));
                row.shorts.insert(0, short);
            }
        }
    }
    let via_uv_ids: IndexSet<String> = via_uv.iter().map(|&i| rows[i].id.clone()).collect();
    for row in rows.iter_mut() {
        let Some(command) = row.command.clone() else {
            continue;
        };
        if via_uv_ids.contains(&row.id) || !missing.contains(&as_run(plan, &command)) {
            continue;
        }
        let short = format!("{} is not on the machine", basename(&command));
        row.notes.push(format!("{short}; the server starts once it is installed there"));
        row.shorts.push(short);
    }

    let summary = summarize(&rows, &present);
    let free = free_note(machine, None).await.ok().flatten();
    let text = closing(&[Some(summary.as_str()), free.as_deref()]);
    stage(GoldenStage::InstallingMcp, Some(text.as_str()));
    rows.into_iter().map(strip).collect()
}

// TS golden-mcp.ts:536-564
// /** Runs the plan on the builder in two edits of the text it read off the machine: the first says each kept server's
//  * command as the machine will run it, every command is looked for on the machine's PATH, and the second is what
//  * lands, with the servers whose command is neither there nor fetched on first use taken out. uv is installed by its
//  * checksummed release when a server runs through it and it is missing. Each server is named on the stage as
//  * installed or skipped with its reason; `tools` is what the tools stage did, so a skipped server can name the row
//  * that would have brought its command. Nothing here fails the build. */
// export async function applyMcp(machine: Machine, plan: McpPlan, stage: StageListener, tools: readonly ToolResult[] = []): Promise<McpResult[]> {
//   stage("installing-mcp", mcpOpening(plan.agents));
//   const missing = new Set<string>();
//   let agents = plan.agents;
//   const scopes = plan.agents.flatMap(a => a.scopes);
//   // The answer is every config whole, the servers' env and headers with it. No backend the seal runs on carries a
//   // byte road out of a machine (a signed download URL is minted by one provider of the several), so the read goes
//   // by the road every backend has and says that its output is not a log's.
//   const res = await machine.run(readConfigsCmd(scopes.map(s => ({ files: s.files, base: plan.guestHome }))), { deadlineMs: READ_MS, unlogged: true }).catch(refused);
//   let failure = res.exitCode === 0 ? undefined : readFailed(res);
//   let report: ScopeOutcome[] | undefined;
//   const files = failure === undefined ? parseConfigs(res.stdout, scopes) : undefined;
//   if (files === undefined) failure ??= readFailed(res);
//   else {
//     const first = editScopes(plan, agents, files);
//     for (const command of await absentCommands(machine, plan, first.outcomes)) missing.add(command);
//     agents = withoutAbsent(plan, agents, first.outcomes, missing, tools);
//     const second = editScopes(plan, agents, files);
//     report = second.outcomes;
//     failure = await landConfigs(machine, plan.guestHome, files, second.texts);
//   }
//   return mcpRows(machine, plan, { agents, report, missing, stage, ...(failure !== undefined ? { failure } : {}) });
// }
/// golden-mcp.ts:542 `applyMcp(machine, plan, stage, tools)`: the builder's round. Never fails.
pub async fn apply_mcp(machine: &dyn Machine, plan: &McpPlan, stage: StageListener, tools: &[ToolResult]) -> Vec<McpResult> {
    stage(GoldenStage::InstallingMcp, Some(mcp_opening(&plan.agents).as_str()));
    let mut missing: IndexSet<String> = IndexSet::new();
    let mut agents = plan.agents.clone();
    let asks: Vec<ReadAsk> = plan
        .agents
        .iter()
        .flat_map(|a| a.scopes.iter())
        .map(|s| ReadAsk { files: s.files.clone(), base: plan.guest_home.clone() })
        .collect();
    let res = match machine.run(&read_configs_cmd(&asks), RunOptions { deadline_ms: READ_MS, unlogged: true, ..Default::default() }).await {
        Ok(res) => res,
        Err(e) => refused(&e.to_string()),
    };
    let mut failure: Option<String> = (res.exit_code.get() != 0).then(|| read_failed(&res));
    let mut report: Option<Vec<ScopeOutcome>> = None;
    let files = if failure.is_none() { parse_configs(&res.stdout, &asks) } else { None };
    match files {
        None => failure = failure.or_else(|| Some(read_failed(&res))),
        Some(files) => {
            let first = edit_scopes(plan, &agents, &files);
            for command in absent_commands(machine, plan, &first.outcomes, TOOLS_PATH).await {
                missing.insert(command);
            }
            agents = without_absent(plan, &agents, &first.outcomes, &missing, tools);
            let second = edit_scopes(plan, &agents, &files);
            let base = plan.guest_home.clone();
            failure = land_configs(machine, &|_: &str| base.clone(), &files, &second.texts).await;
            report = Some(second.outcomes);
        }
    }
    mcp_rows(machine, plan, McpRowsOptions { agents, report, failure, missing, present: IndexSet::new(), stage }).await
}

struct Pending {
    id: String,
    agent: String,
    name: String,
    outcome: String,
    command: Option<String>,
    notes: Vec<String>,
    shorts: Vec<String>,
    keys: Option<Vec<String>>,
}

fn skipped(id: String, agent: &str, name: &str, note: &str, keys: Option<Vec<String>>) -> Pending {
    Pending {
        id,
        agent: agent.to_string(),
        name: name.to_string(),
        outcome: "skipped".to_string(),
        command: None,
        notes: vec![note.to_string()],
        shorts: Vec::new(),
        keys,
    }
}

fn strip(r: Pending) -> McpResult {
    McpResult {
        id: r.id,
        agent: r.agent,
        name: r.name,
        outcome: r.outcome,
        note: (!r.notes.is_empty()).then(|| r.notes.join("; ")),
        keys: r.keys,
    }
}

fn summarize(rows: &[Pending], present: &IndexSet<String>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let already: Vec<&str> = rows.iter().filter(|r| present.contains(&r.id)).map(|r| r.name.as_str()).collect();
    if !already.is_empty() {
        parts.push(format!("{} already there", already.join(", ")));
    }
    let installed: Vec<&str> = rows
        .iter()
        .filter(|r| r.outcome == "installed" && r.shorts.is_empty() && !present.contains(&r.id))
        .map(|r| r.name.as_str())
        .collect();
    if !installed.is_empty() {
        parts.push(format!("{} installed", installed.join(", ")));
    }
    let mut by_note: IndexMap<String, Vec<&str>> = IndexMap::new();
    for r in rows {
        if r.outcome == "skipped" || r.shorts.is_empty() || present.contains(&r.id) {
            continue;
        }
        by_note.entry(r.shorts.join(", ")).or_default().push(r.name.as_str());
    }
    for (short, names) in &by_note {
        parts.push(format!("{}: {short}", names.join(", ")));
    }
    for r in rows.iter().filter(|r| r.outcome == "skipped") {
        let note = r.notes.first().map_or("no reason given", String::as_str);
        parts.push(format!("{} skipped ({note})", r.name));
    }
    if parts.is_empty() {
        "nothing to do".to_string()
    } else {
        parts.join("; ")
    }
}

fn named_commands<R: CommandRowItem>(row: &R) -> Vec<String> {
    let entry = catalog_entry(&catalog_id_of_row(row.row_id()).unwrap_or_default());
    let mut names: Vec<String> = row.row_bin().map(str::to_string).into_iter().collect();
    if let Some(entry) = entry {
        names.push(entry.bin().to_string());
        if let Some(tool) = entry.as_tool() {
            names.extend(tool.brings.iter().flatten().map(|b| b.bin.clone()));
        }
    }
    names
}

fn basename(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, last)| last)
}

fn is_binary_row(id: &str) -> bool {
    id.strip_prefix("tools/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(manager, _)| ["brew", "go", "cargo", "npm", "pnpm", "bun", "pipx", "uv", "hand"].contains(&manager))
}
