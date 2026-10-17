// SPDX-License-Identifier: AGPL-3.0-only
// A provider key put to the provider before it is saved: it works, with the
// account, plan and machines it reads, or it is refused in the person's words.
// Boat answers a wrong key 401 and a key without an action 403
// api_key_action_forbidden naming the action (measured 2026-10-13, and Boat's
// Scopes page). The check reads the account (account.read) and lists machines
// (sandbox.read). A ci key made before Boat added resume and delete-own to the
// preset passes both: Boat checks the action before the box, so a resume of a
// box that does not exist answers 403 without the action and 404 with it, and
// delete-own, which only a box the key made can show, is read off the key's
// own row where Boat lists it (measured 2026-10-13). wsp never uses Boat's
// SSH, so a key without it passes. The first fork proves the rest.
import { BOX_API_URL, PROVIDER_READ_CAP_MS, limitsOf, type BoxLimits } from "@wsp/engine";
import { KEY_REFUSED, KEY_UNCHECKED, providerSaidLine, type ProviderKeyWorks } from "@wsp/protocol";

/** The actions Boat added to the ci preset after it shipped that wsp uses, which an older ci key lacks (Boat's Scopes
 * page). */
const LATE_ACTIONS: readonly string[] = ["sandbox.resume", "sandbox.delete-own"];

/** A box id no account holds, for the probe that must answer about the action and never touch a box. */
const NO_BOX = "bx_wspkeycheck0000000000";

/** What a missing action stops, in the words the key step names it by. */
const ACTION_DOES: Readonly<Record<string, string>> = { "sandbox.read": "list machines", "account.read": "read the account" };

/** The sentences a key check is refused with, by the provider's name. */
export const KEY_CHECK_WORDS = {
  refused: (name: string) => `${name} refused this key. Check you copied all of it.`,
  expired: (name: string) => `This key has expired. Make a new one on ${name}'s API keys page.`,
  lacks: (action: string) => `This key cannot ${ACTION_DOES[action] ?? `use ${action}`}. Give it ${action} and the other actions above.`,
  other: (name: string, said: string) => `${name} refused this key: ${said}`,
  older: (name: string) => `This key was made before ${name} added resume and delete-own to the ci preset, so make a new ci key or use an admin key.`,
  unreachable: (name: string) => `wsp cannot reach ${name} right now. Try again.`,
};

/** A refusal as the host answers it: the sentence, whether the provider answered about the key or never did, and the
 * provider's own status and word, which the key save says after its own words. */
export type KeyRefusal = Error & { kind: typeof KEY_REFUSED | typeof KEY_UNCHECKED; said: string };

const refusal = (sentence: string, kind: KeyRefusal["kind"], said: string): KeyRefusal => Object.assign(new Error(sentence), { kind, said });

/** A read's status and envelope; status 0 is a read nothing answered. */
type Answer = { status: number; body: Record<string, unknown> };

async function read(base: string, path: string, key: string, post?: object): Promise<Answer> {
  try {
    const res = await fetch(`${base}${path}`, { method: post === undefined ? "GET" : "POST", headers: { Authorization: `Bearer ${key}`, Accept: "application/json", ...(post === undefined ? {} : { "Content-Type": "application/json" }) }, ...(post === undefined ? {} : { body: JSON.stringify(post) }), signal: AbortSignal.timeout(PROVIDER_READ_CAP_MS) });
    const body = (await res.json().catch(() => ({}))) as unknown;
    return { status: res.status, body: body !== null && typeof body === "object" ? (body as Record<string, unknown>) : {} };
  } catch {
    return { status: 0, body: {} };
  }
}

/** The envelope's code, message and the action a 403 names, at the top or under `error.details`. */
function envelope(body: Record<string, unknown>): { code?: string; message?: string; action?: string } {
  const error = (body["error"] ?? {}) as Record<string, unknown>;
  const details = (error["details"] ?? {}) as Record<string, unknown>;
  const text = (v: unknown): string | undefined => (typeof v === "string" && v !== "" ? v : undefined);
  const code = text(body["code"]) ?? text(error["code"]);
  const message = text(body["message"]) ?? text(error["message"]);
  const named = text(details["action"]) ?? /cannot perform ([\w*-]+(?:\.[\w*-]+)*)/.exec(message ?? "")?.[1];
  return { ...(code === undefined ? {} : { code }), ...(message === undefined ? {} : { message }), ...(named === undefined ? {} : { action: named }) };
}

/** The one refusal a set of answers reads as: a key Boat does not know before a key that expired, before an action it
 * lacks (in the order the reads were made), before any other refusal, before Boat not answering at all. */
function refusedBy(answers: readonly Answer[], name: string): KeyRefusal | undefined {
  const failed = answers.filter(a => a.status !== 200);
  if (failed.length === 0) return undefined;
  const said = failed.flatMap(a => (a.status === 0 ? [] : [{ status: a.status, ...envelope(a.body) }]));
  const line = (a: (typeof said)[number]): string => providerSaidLine(a.status, a.message ?? a.code ?? "");
  const unknown = said.find(a => a.status === 401);
  if (unknown !== undefined) return refusal(KEY_CHECK_WORDS.refused(name), KEY_REFUSED, line(unknown));
  const expired = said.find(a => a.code === "api_key_expired");
  if (expired !== undefined) return refusal(KEY_CHECK_WORDS.expired(name), KEY_REFUSED, line(expired));
  const lacking = said.find(a => a.code === "api_key_action_forbidden" && a.action !== undefined);
  if (lacking !== undefined) return refusal(KEY_CHECK_WORDS.lacks(lacking.action!), KEY_REFUSED, line(lacking));
  const other = said.find(a => a.status >= 400 && a.status < 500 && a.status !== 408 && a.status !== 429);
  if (other !== undefined) return refusal(KEY_CHECK_WORDS.other(name, other.message ?? String(other.status)), KEY_REFUSED, line(other));
  const first = said[0];
  return refusal(KEY_CHECK_WORDS.unreachable(name), KEY_UNCHECKED, first === undefined ? "no answer" : line(first));
}

/** The late ci actions a key lacks: the probe's 403, and its own row's actions where Boat lists the key. */
function lateMissing(probes: readonly Answer[], keys: Answer, key: string): string[] {
  const refusedTo = probes.flatMap(a => (a.status === 403 ? [envelope(a.body)] : [])).filter(e => e.code === "api_key_action_forbidden").flatMap(e => (e.action === undefined ? [] : [e.action]));
  const rows = (keys.status === 200 && Array.isArray(keys.body["apiKeys"]) ? keys.body["apiKeys"] : []) as { keyPrefix?: unknown; keyLastFour?: unknown; scope?: { actions?: unknown } }[];
  const own = rows.find(r => typeof r.keyPrefix === "string" && typeof r.keyLastFour === "string" && key.startsWith(r.keyPrefix) && key.endsWith(r.keyLastFour));
  const held = Array.isArray(own?.scope?.actions) ? (own.scope.actions as unknown[]).filter((a): a is string => typeof a === "string") : undefined;
  const ownLacks = held === undefined || held.includes("*") ? [] : LATE_ACTIONS.filter(a => !held.includes(a));
  return LATE_ACTIONS.filter(a => refusedTo.includes(a) || ownLacks.includes(a));
}

/** A plan as a person names it: the trial, the plan's monthly dollars, or a paid plan whose price was not read. */
export const planWords = (limits: Pick<BoxLimits, "trial" | "planUsd">): string => (limits.trial ? "trial" : limits.planUsd === undefined ? "paid plan" : `$${limits.planUsd} plan`);

/** Boat's check, read at once: the account (`GET /me`), the plan and machines (`GET /limits`), one page of machines
 * (`GET /sandboxes`), a resume of a box that does not exist, and the key's own row (`GET /api-keys`). Throws the
 * refusal; answers the key's facts and its limits for the line `wsp add` prints. */
export async function boatKeyCheck(key: string, name = "Boat", base = BOX_API_URL): Promise<{ works: ProviderKeyWorks; limits: BoxLimits }> {
  const [me, limitsRead, boxes, resume, keys] = await Promise.all([
    read(base, "/me", key),
    read(base, "/limits", key),
    read(base, "/sandboxes?limit=1", key),
    read(base, `/sandboxes/${NO_BOX}/resume`, key, {}),
    read(base, "/api-keys", key),
  ]);
  const refused = refusedBy([me, limitsRead, boxes], name);
  if (refused !== undefined) throw refused;
  const late = lateMissing([resume], keys, key);
  if (late.length > 0) throw refusal(KEY_CHECK_WORDS.older(name), KEY_REFUSED, `missing ${late.join(", ")}`);
  const user = (me.body["user"] ?? {}) as Record<string, unknown>;
  const limits = limitsOf(limitsRead.body as Parameters<typeof limitsOf>[0]);
  const account = typeof user["login"] === "string" ? user["login"] : "";
  return { works: { works: true, account, plan: planWords(limits), running: limits.activeBoxes ?? 0, limit: limits.machinesAtOnce ?? 0 }, limits };
}
