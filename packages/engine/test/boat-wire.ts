// SPDX-License-Identifier: AGPL-3.0-only
// The Boat canary's record of every call wsp's backend sends, and the two readings it fails on: a call Boat refused,
// quoted with the body sent, and a reply missing a field wsp reads. Kept apart from the live file so both readings
// are proved without a key (boat-wire.test.ts).

type Fetch = typeof globalThis.fetch;

export interface Exchange {
  method: string;
  /** The path under the base URL, query included. */
  path: string;
  /** The body as sent, byte for byte. */
  sent: string | undefined;
  /** null when the road failed before an answer came back. */
  status: number | null;
  text: string;
  /** When the request left, and how long its answer took, in ms. */
  at: number;
  ms: number;
  /** The answer's Date header: Boat's own clock, which a reading of its instants is checked against. */
  date?: string;
}

/** The fields wsp reads off each answer, by call: a leaf may be null, its key must be there. `[]` walks an array. */
export const READS: readonly { method: string; path: RegExp; fields: readonly string[] }[] = [
  { method: "GET", path: /^\/limits$/, fields: ["accessTier", "maxActiveSandboxes", "activeSandboxes", "canStart", "startBlockedReason", "sandboxPlanKey", "creditBalanceHours", ...["minute", "hour", "day"].flatMap(w => ["limit", "used", "remaining"].map(f => `starts.${w}.${f}`))] },
  { method: "GET", path: /^\/named-snapshots$/, fields: ["snapshots[].name", "snapshots[].status", "snapshots[].createdAt"] },
  { method: "POST", path: /^\/sandboxes$/, fields: ["status", "sandbox.id"] },
  { method: "GET", path: /^\/sandboxes\?/, fields: ["sandboxes[].id", "sandboxes[].name", "sandboxes[].state"] },
  { method: "GET", path: /^\/sandboxes\/[^/?]+$/, fields: ["sandbox.id", "sandbox.state"] },
  { method: "POST", path: /^\/sandboxes\/[^/]+\/commands$/, fields: ["exitCode", "stdout", "stderr"] },
  { method: "POST", path: /^\/sandboxes\/[^/]+\/host$/, fields: ["url"] },
  { method: "GET", path: /^\/sandboxes\/[^/]+\/files\?/, fields: ["content", "encoding", "size"] },
];

/** A fetch that records every call through it and hands the answer on unchanged. */
export function recordingFetch(wire: Exchange[], baseUrl: string, base: Fetch = globalThis.fetch): Fetch {
  return async (input, init) => {
    const url = String(input);
    const exchange: Exchange = {
      method: init?.method ?? "GET",
      path: url.startsWith(baseUrl) ? url.slice(baseUrl.length) : url,
      sent: typeof init?.body === "string" ? init.body : undefined,
      status: null,
      text: "",
      at: Date.now(),
      ms: 0,
    };
    wire.push(exchange);
    let res: Response;
    try {
      res = await base(input, init);
    } catch (e) {
      exchange.ms = Date.now() - exchange.at;
      exchange.text = e instanceof Error ? e.message : String(e);
      throw e;
    }
    exchange.status = res.status;
    exchange.text = await res.text();
    const date = res.headers.get("date");
    if (date !== null) exchange.date = date;
    exchange.ms = Date.now() - exchange.at;
    // Response throws on a body, even an empty one, with these.
    const bodyless = res.status === 204 || res.status === 205 || res.status === 304;
    return new Response(bodyless ? null : exchange.text, { status: res.status, statusText: res.statusText, headers: res.headers });
  };
}

const parsed = (text: string | undefined): unknown => {
  if (text === undefined || text === "") return undefined;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return text;
  }
};

/** What may be printed of one body: file bytes by count, a hosted route without its query (the route's token), a
 * long command cut. */
function printable(e: Exchange, which: "sent" | "reply"): unknown {
  const body = parsed(which === "sent" ? e.sent : e.text);
  if (body === null || typeof body !== "object") return body;
  const out: Record<string, unknown> = { ...(body as Record<string, unknown>) };
  if (typeof out.content === "string") out.content = `<${out.content.length} chars>`;
  if (typeof out.url === "string") out.url = out.url.replace(/\?.*$/, "?<token>");
  if (typeof out.command === "string" && out.command.length > 300) out.command = `${out.command.slice(0, 300)}...`;
  return out;
}

export function shown(e: Exchange): string {
  const answer = e.status === null ? `failed before an answer (${e.text})` : `answered ${e.status} ${JSON.stringify(printable(e, "reply"))}`;
  return `${e.method} ${e.path} ${answer}${e.sent !== undefined ? `; body sent: ${JSON.stringify(printable(e, "sent"))}` : ""}`;
}

/** The last call since `from` that Boat refused or never answered, quoted. */
export function boatSaid(wire: readonly Exchange[], from = 0): string | undefined {
  const refused = wire.slice(from).filter(e => e.status === null || e.status >= 400).at(-1);
  return refused === undefined ? undefined : shown(refused);
}

function present(value: unknown, path: readonly string[]): boolean {
  if (path.length === 0) return true;
  const [head, ...rest] = path as [string, ...string[]];
  if (value === null || typeof value !== "object") return false;
  if (head.endsWith("[]")) {
    const list = (value as Record<string, unknown>)[head.slice(0, -2)];
    return Array.isArray(list) && list.every(item => present(item, rest));
  }
  return head in value && (rest.length === 0 || present((value as Record<string, unknown>)[head], rest));
}

/** The first field wsp reads that this answer no longer carries, or undefined. Only an answered success is read. */
export function missingField(e: Exchange, fields?: readonly string[]): string | undefined {
  if (e.status === null || e.status >= 400) return undefined;
  const reads = fields ?? READS.find(r => r.method === e.method && r.path.test(e.path))?.fields ?? [];
  const reply = parsed(e.text);
  return reads.find(f => !present(reply, f.split(".")));
}

/** Throws naming the call and Boat's answer when any answer since `from` lost a field wsp reads. */
export function checkReads(wire: readonly Exchange[], from = 0): void {
  for (const e of wire.slice(from)) {
    const gone = missingField(e);
    if (gone !== undefined) throw new Error(`Boat's answer lost ${gone}, which wsp reads: ${shown(e)}`);
  }
}
