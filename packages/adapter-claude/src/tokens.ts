// SPDX-License-Identifier: AGPL-3.0-only
// What one call's usage line counts, read the one way the turn's tokens, the
// context meter and a subagent's spend all read it.
import type { TurnTokens, UsageTokens } from "@wsp/protocol";
import { num } from "./fields.js";

/** Every token a call's usage says the model read: the fresh input, the part written to the cache and the part read
 * back from it. */
export function inputTokens(usage: Record<string, unknown>): number {
  return (num(usage.input_tokens) ?? 0) + (num(usage.cache_creation_input_tokens) ?? 0) + (num(usage.cache_read_input_tokens) ?? 0);
}

/** What the model held after one call, off that call's own usage: what it read and what it wrote. */
export function heldTokens(usage: Record<string, unknown>): number {
  return inputTokens(usage) + (num(usage.output_tokens) ?? 0);
}

/** The tokens one usage counts, as a turn's tokens read. */
export function usageTokens(usage: Record<string, unknown>): TurnTokens {
  const cached = num(usage.cache_read_input_tokens);
  const cacheWrite = num(usage.cache_creation_input_tokens);
  return { input: inputTokens(usage), output: num(usage.output_tokens) ?? 0, ...(cached !== undefined ? { cached } : {}), ...(cacheWrite !== undefined ? { cacheWrite } : {}) };
}

export function plusTokens(a: TurnTokens | undefined, b: TurnTokens): TurnTokens {
  if (a === undefined) return b;
  const sum = (k: "cached" | "cacheWrite" | "reasoning"): Partial<TurnTokens> => (a[k] === undefined && b[k] === undefined ? {} : { [k]: (a[k] ?? 0) + (b[k] ?? 0) });
  return { input: a.input + b.input, output: a.output + b.output, ...sum("cached"), ...sum("cacheWrite"), ...sum("reasoning") };
}

/** A turn's tokens as one model's entry holds them, every count present. */
export const usageOf = (t: TurnTokens): UsageTokens => ({ input: t.input, output: t.output, cached: t.cached ?? 0, cacheWrite: t.cacheWrite ?? 0, reasoning: t.reasoning ?? 0 });
