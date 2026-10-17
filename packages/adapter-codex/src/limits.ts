// SPDX-License-Identifier: AGPL-3.0-only
// The plan's windows and banked resets as the app server reports them, and the reset a turn stopped at the limit waits on.
import { limitKindOfMinutes, RESET_CREDIT_STATUSES, type HarnessLimit, type LimitWindow, type ResetCredit } from "@wsp/protocol";
import { count, rec, str } from "./fields.js";

/** A window's reset as ms epoch: the server sends unix seconds, as the rollout's resets_at is. */
const resetMs = (value: number): number => (value < 1e11 ? value * 1000 : value);

/** The resets the plan has banked, off a rate-limits answer, read fail closed: a count that is not a whole number at
 * or above zero reads the whole field absent, and a credit of a kind other than this plan's windows, or in a state
 * this code does not know, reads unknown and never available. */
export function creditsOf(answer: Record<string, unknown> | undefined): HarnessLimit["credits"] {
  const summary = rec(answer?.rateLimitResetCredits);
  const banked = summary?.availableCount;
  if (typeof banked !== "number" || !Number.isInteger(banked) || banked < 0) return undefined;
  if (!Array.isArray(summary?.credits)) return { count: banked };
  const credits = summary.credits.flatMap((raw): ResetCredit[] => {
    const credit = rec(raw);
    const id = str(credit?.id);
    if (credit === undefined || id === undefined) return [];
    const said = RESET_CREDIT_STATUSES.find(s => s === credit.status);
    const expiresAt = count(credit.expiresAt);
    return [{ id, status: credit.resetType === "codexRateLimits" && said !== undefined ? said : "unknown", ...(expiresAt !== undefined ? { expiresAt: resetMs(expiresAt) } : {}) }];
  });
  return { count: banked, credits };
}

/** The reset a turn the server stopped at the plan's limit waits on: the fullest window whose reset is still ahead.
 * At the cap the server reads 99 as often as 100, so no threshold says which window it was. */
export function stoppedUntil(windows: readonly LimitWindow[], now: number): number | undefined {
  const ahead = windows.filter((w): w is LimitWindow & { resetsAt: number } => w.resetsAt !== undefined && w.resetsAt > now);
  return ahead.sort((a, b) => b.usedPercent - a.usedPercent || b.resetsAt - a.resetsAt)[0]?.resetsAt;
}

/** The server's codexErrorInfo on an error the plan's usage limit raised. */
export const USAGE_LIMIT_CODE = "usageLimitExceeded";

/** The snapshot a rate-limits answer reads the windows off: the legacy single bucket can name another meter than
 * codex's own, and the buckets by id say which is which. */
export const snapshotOf = (answer: Record<string, unknown> | undefined): Record<string, unknown> | undefined => rec(rec(answer?.rateLimitsByLimitId)?.codex) ?? rec(answer?.rateLimits);

/** The plan's windows off a rate-limit snapshot, each read as a kind by its length (the primary is the session and the
 * secondary the week where the server names no length), with the plan and the account account/read named. A limit
 * the backend says was reached reads reached. */
export function limitOf(snapshot: Record<string, unknown>, account: { id?: string; label?: string; plan?: string }, credits?: HarnessLimit["credits"]): HarnessLimit | undefined {
  const windows: LimitWindow[] = [];
  for (const [slot, fallback] of [["primary", 300], ["secondary", 10_080]] as const) {
    const w = rec(snapshot[slot]);
    const used = count(w?.usedPercent);
    if (w === undefined || used === undefined) continue;
    const resetsAt = count(w.resetsAt);
    windows.push({ kind: limitKindOfMinutes(count(w.windowDurationMins) ?? fallback), usedPercent: used, ...(resetsAt !== undefined ? { resetsAt: resetMs(resetsAt) } : {}) });
  }
  if (windows.length === 0) return undefined;
  const plan = str(snapshot.planType) ?? account.plan;
  const id = account.id ?? account.label;
  return {
    windows,
    ...(plan !== undefined ? { plan } : {}),
    status: snapshot.rateLimitReachedType === undefined || snapshot.rateLimitReachedType === null ? "ok" : "reached",
    ...(id !== undefined ? { account: { id, ...(account.label !== undefined ? { label: account.label } : {}) } } : {}),
    ...(credits !== undefined ? { credits } : {}),
  };
}
