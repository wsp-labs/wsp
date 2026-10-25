// SPDX-License-Identifier: AGPL-3.0-only
//! Unit mcp_rows (spec/golden-mcp_rows.md): golden-mcp.ts:337-566 at a4033dc34. Which kept servers' commands the
//! machine lacks, the plan with those dropped, every server as a row with the words a person reads, and the builder's
//! whole round.
use indexmap::{IndexMap, IndexSet};

use super::{McpAgentPlan, McpPlan, McpResult, McpToolRow, ScopeOutcome, StageListener, ToolInstall, ToolResult};
use crate::machine::Machine;

/// golden-mcp.ts:348 `FETCHERS`: the runner's basename and who fetches for it (`npx` npx, `uvx` uv, `uv` uv), in the
/// TS's order.
pub fn fetchers() -> IndexMap<&'static str, &'static str> {
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
}

/// golden-mcp.ts:423 `asRun`: a leading `~/` is the plan's guest home.
pub fn as_run(plan: &McpPlan, command: &str) -> String {
    todo!()
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
    todo!()
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
    todo!()
}

/// golden-mcp.ts:459 `mcpOpening`: `<label> <count>` per agent, joined with `, `.
pub fn mcp_opening(agents: &[McpAgentPlan]) -> String {
    todo!()
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
    todo!()
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
    todo!()
}
