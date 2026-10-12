// SPDX-License-Identifier: AGPL-3.0-only
// What each of a turn's subagents spent: the sum of its own calls, each message
// counted once, and the tool calls and run time its progress lines name. The
// total_tokens on those lines follows the size of what the subagent holds, not
// what it spent (measured on 2.1.296), so nothing here reads it.
import type { SubagentUsage, UsageTokens } from "@wsp/protocol";
import { num, rec, str } from "./fields.js";
import { usageOf, usageTokens } from "./tokens.js";

const FIELDS = ["input", "output", "cached", "cacheWrite", "reasoning"] as const;

export interface SpendBook {
  /** Each subagent's calls by message id: every block of a message repeats its usage, and a block prints before the
   * call ends, so a message is held at the largest output any of its lines showed. */
  readonly calls: Map<string, Map<string, UsageTokens>>;
  /** Each subagent's calls summed, moved by each line rather than summed again at each progress line, which grew as
   * the square of its calls. */
  readonly totals: Map<string, UsageTokens>;
  /** The tool calls and run time each subagent's latest progress or end line named. */
  readonly said: Map<string, { toolUses?: number; durationMs?: number }>;
}

export const newSpendBook = (): SpendBook => ({ calls: new Map(), totals: new Map(), said: new Map() });

/** One of a subagent's own lines into its spend; a line that is no call counts nothing. */
export function noteChildCall(book: SpendBook, task: string, event: Record<string, unknown>): void {
  const message = rec(event.message);
  const id = str(message?.id);
  const usage = rec(message?.usage);
  if (id === undefined || usage === undefined) return;
  const calls = book.calls.get(task) ?? new Map<string, UsageTokens>();
  const held = calls.get(id);
  const tokens = usageOf(usageTokens(usage));
  const call = { ...tokens, output: Math.max(held?.output ?? 0, tokens.output) };
  const total = book.totals.get(task);
  book.totals.set(task, Object.fromEntries(FIELDS.map(k => [k, (total?.[k] ?? 0) - (held?.[k] ?? 0) + call[k]])) as UsageTokens);
  calls.set(id, call);
  book.calls.set(task, calls);
}

/** The subagent a progress or end line names, once what it says is noted; undefined for any other line, and for a
 * task that is no subagent of this turn (a background command's lines are the same shape). */
export function noteProgress(book: SpendBook, event: Record<string, unknown>, subagents: ReadonlyMap<string, unknown>): string | undefined {
  const subtype = str(event.subtype);
  if (str(event.type) !== "system" || (subtype !== "task_progress" && subtype !== "task_notification")) return undefined;
  const task = str(event.task_id);
  const usage = rec(event.usage);
  if (task === undefined || usage === undefined || !subagents.has(task)) return undefined;
  const toolUses = num(usage.tool_uses);
  const durationMs = num(usage.duration_ms);
  book.said.set(task, { ...(toolUses !== undefined ? { toolUses } : {}), ...(durationMs !== undefined ? { durationMs } : {}) });
  return task;
}

/** What a subagent has spent so far; undefined while nothing of it has been counted. */
export function subagentSpent(book: SpendBook, task: string): SubagentUsage | undefined {
  const tokens = book.totals.get(task);
  const said = book.said.get(task);
  if (tokens === undefined && said === undefined) return undefined;
  return { ...(tokens !== undefined ? { tokens } : {}), ...said };
}

/** What a subagent spent in all, at the line that ended it: a run time only where that line names one, since a kill
 * ends on a line with no usage and the last progress line's run time is from before it (measured on 2.1.296). */
export function subagentSpentAtEnd(book: SpendBook, task: string, end: Record<string, unknown>): SubagentUsage | undefined {
  const spent = subagentSpent(book, task);
  if (spent === undefined) return undefined;
  const { durationMs: _progressMs, ...rest } = spent;
  const durationMs = num(rec(end.usage)?.duration_ms);
  return durationMs === undefined ? rest : { ...rest, durationMs };
}
