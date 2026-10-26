// What every cli-c probe shares: a scripted host a helper function is called against, the sinks a helper says its
// lines to, and the one way a result or a thrown error is written down. Read-only over /root/wsp-vm/wt-2441-cut
// (65f6b89c7). A probe file lists its cases and calls `record("<unit>", cases)`.
import { vi } from "vitest";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

export const R = "/root/wsp-vm/wt-2441-cut";
export const OUT = process.env.OUT ?? "/root/wsp-vm/wt-2449-cli-c/host/crates/wsp-cli/tests/fixtures/cli_c";
export const ANSWERS = "/root/wsp-vm/wt-2449-cli-c/daemon/crates/wsp-mcp/tests/answers";

export type Obj = Record<string, unknown>;

/** The host a case scripts. Keys are an op, or `<op> #<n>` for its nth ask.
 * `replies`: the frame text answering an op (an op no reply names is refused `<op> is not in this record`).
 * `pushed`: frames every listener hears while the op is under way, before its reply.
 * `after`: frames every listener hears once the op's reply is back, on a later turn of the loop.
 * `closes`: the socket goes during the op, after its pushed frames and with no reply, under that close code.
 * `closesAfter`: the socket goes under that code on a later turn, once the op's reply (and its `after` frames) is back.
 * The text `<request id>` in a pushed or after frame is the `requestId` the op was sent with.
 * `redials`: what each later `deps.client()` answers: another script, or `{ refuses: "<message>" }`. */
export interface Script {
  replies?: Record<string, string>;
  pushed?: Record<string, string[]>;
  after?: Record<string, string[]>;
  closes?: Record<string, number>;
  closesAfter?: Record<string, number>;
  redials?: (Script | { refuses: string })[];
}

/** One case: a helper called with `args` against `script`, in `env` and the files, the cloud on or off. */
export interface Case {
  fn: string;
  case: string;
  args?: Obj;
  script?: Script;
  env?: Record<string, string>;
  files?: Record<string, string>;
  cloud?: boolean;
  /** Wall-clock milliseconds the call is given before it is cut off and recorded as hanging. */
  within?: number;
}

export const NOT_IN_RECORD = "is not in this record";
export const HOST_STOPPING_CLOSE = 4001;

const protocol = (): Promise<Obj> => import(`${R}/packages/protocol/src/index.ts`);

export interface Sinks {
  frames: string[];
  said: string[];
  stdout: string[];
  stderr: string[];
  streamed: string;
  emitted: { value: unknown; line?: string }[];
  dials: number;
}

export function host(script: Script, log: string[], proto: Obj, ids: string[], label = ""): Obj {
  const listeners = new Set<(f: Obj) => void>();
  const seen = new Map<string, number>();
  let next = 2;
  let code: number | undefined;
  let goneNow = false;
  let settle: (c: number | undefined) => void = () => {};
  const closed = new Promise<number | undefined>(r => (settle = r));
  let subscribed: Promise<void> | undefined;
  const hear = (frames: string[] | undefined, requestId?: unknown): void => {
    for (const f of frames ?? []) for (const fn of [...listeners]) fn(JSON.parse(typeof requestId === "string" ? f.split("<request id>").join(requestId) : f) as Obj);
  };
  const closeWords = (): string => (code === HOST_STOPPING_CLOSE ? (proto["HOST_STOPPING_LINE"] as string) : (proto["HOST_CLOSED_LINE"] as string));
  const request = async (op: string, params: Obj = {}): Promise<Obj> => {
    if (goneNow) throw new Error(closeWords());
    const id = next++;
    if (typeof params["requestId"] === "string") ids.push(params["requestId"]);
    const nth = (seen.get(op) ?? 0) + 1;
    seen.set(op, nth);
    const text = JSON.stringify({ ...params, id, op }).replace(/"requestId":"[^"]*"/, '"requestId":"<request id>"');
    log.push(`${label}${text}`);
    const key = (table: Record<string, unknown> | undefined): string | undefined => (table === undefined ? undefined : table[`${op} #${nth}`] !== undefined ? `${op} #${nth}` : table[op] !== undefined ? op : undefined);
    const at = (table: Record<string, string[]> | undefined): string[] | undefined => (key(table) === undefined ? undefined : table![key(table)!]);
    hear(at(script.pushed), params["requestId"]);
    const closing = key(script.closes);
    if (closing !== undefined) {
      code = script.closes![closing];
      goneNow = true;
      settle(code);
      throw new Error(closeWords());
    }
    const k = key(script.replies);
    const answer = k !== undefined ? script.replies![k]! : op === "events.subscribe" ? JSON.stringify({ id: 1, ok: true }) : JSON.stringify({ ok: false, error: `${op} ${NOT_IN_RECORD}` });
    const frame = JSON.parse(answer) as Obj;
    const later = at(script.after);
    const closeLater = key(script.closesAfter);
    if (later !== undefined || closeLater !== undefined) {
      setTimeout(() => {
        hear(later, params["requestId"]);
        if (closeLater !== undefined) {
          code = script.closesAfter![closeLater];
          goneNow = true;
          settle(code);
        }
      }, 0);
    }
    if (frame["ok"] !== true) {
      const problem = (proto["problemListsOf"] as (f: Obj) => Obj)(frame);
      throw Object.assign(new Error(typeof frame["error"] === "string" ? frame["error"] : `${op} failed`), typeof frame["kind"] === "string" ? { kind: frame["kind"] } : {}, problem);
    }
    return frame;
  };
  return {
    request,
    events: () => (subscribed ??= request("events.subscribe").then(() => undefined)),
    onFrame: (fn: (f: Obj) => void) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    closed,
    closeWords,
    close: () => {},
    terminate: () => {},
  };
}

/** `deps` as `waitThrough`, `restartHost` and `followVerb` read them: `client()` with no argument answers the case's own
 * socket (`first`); `client({ withinMs })`, the dial again after a host stopped, and `redial()`, the callback a start or
 * a follow is handed, both answer the next of `redials` in order, logging `<dialled again>` where the frames are; a dial
 * beyond them refuses `no host serving the state file`. */
export function depsOf(script: Script, log: string[], proto: Obj, first: Obj, ids: string[]): Obj {
  let r = 0;
  const dial = async (): Promise<Obj> => {
    const next = script.redials?.[r++];
    if (next === undefined) throw new Error("no host serving the state file");
    if ("refuses" in next) throw new Error(next.refuses);
    log.push("<dialled again>");
    return host(next, log, proto, ids);
  };
  return {
    hostWaitMs: 200,
    client: async (again?: { withinMs: number }) => (again === undefined ? first : dial()),
    redial: () => dial(),
  };
}

/** A value as JSON the way the zod-parsed object it is would be written: undefined dropped, a Map as its entries, a
 * Set as an array, an Error as its message and stamps. */
export function norm(v: unknown): unknown {
  if (v === undefined) return null;
  if (v instanceof Error) return { error: errorOf(v) };
  return JSON.parse(JSON.stringify(v, (_k, x) => (x instanceof Map ? [...x.entries()] : x instanceof Set ? [...x] : typeof x === "number" && !Number.isFinite(x) ? null : x))) as unknown;
}

export function errorOf(e: unknown): Obj {
  if (!(e instanceof Error)) return { thrown: String(e) };
  const o = e as unknown as Obj;
  const marks = Object.keys(o).filter(k => o[k] === true);
  return {
    message: e.message,
    ...(typeof o["fix"] === "string" ? { fix: o["fix"] } : {}),
    ...(typeof o["kind"] === "string" ? { kind: o["kind"] } : {}),
    ...(o["errors"] !== undefined ? { errors: norm(o["errors"]) } : {}),
    ...(o["warnings"] !== undefined ? { warnings: norm(o["warnings"]) } : {}),
    ...(o["left"] !== undefined ? { left: norm(o["left"]) } : {}),
    ...(typeof o["hostKey"] === "string" ? { hostKey: o["hostKey"] } : {}),
    ...(marks.length > 0 ? { marks } : {}),
  };
}

/** How a case's terminal is set up: `muted` wraps every muted line in `<m>..</m>`, `sameScreen` says the stream is the
 * person's own screen, `key` is the key a person types at a prompt (an `answerKey` is offered only where one is given). */
export interface IoOpts {
  muted?: boolean;
  sameScreen?: boolean;
  key?: string;
}

/** The `io` and a `VerbContext` of a case: the lines go to `sinks`, the formatter is the verbs' own, `--json` is `json`. */
export function ctxOf(o: { mod: Obj; deps: Obj; sinks: Sinks; flags?: Obj; io?: IoOpts; json?: boolean; usage?: string }): Obj {
  const io: Obj = {
    log: (l: string) => o.sinks.stdout.push(l),
    error: (l: string) => o.sinks.stderr.push(l),
    stream: (t: string) => (o.sinks.streamed += t),
    ...(o.io?.muted === true ? { muted: (l: string) => `<m>${l}</m>` } : {}),
    ...(o.io?.sameScreen === true ? { sameScreen: true } : {}),
    ...(o.io?.key !== undefined ? { answerKey: () => Promise.resolve(o.io!.key) } : {}),
  };
  const out = ((o.mod["client"] as Obj)["formatter"] as (io: Obj, json: boolean) => Obj)(io, o.json === true);
  return { ...o.deps, args: [], flags: { ...(o.flags ?? {}), ...(o.json === true ? { json: true } : {}) }, io, out, aim: { kind: "here" }, usage: o.usage ?? "wsp <verb>" };
}

export type Run = (ctx: { c: Case; args: Obj; client: Obj; deps: Obj; sinks: Sinks; mod: Obj; proto: Obj; cwd: string }) => Promise<unknown>;

let plain: Promise<Obj> | undefined;
let cloudy: Promise<Obj> | undefined;

/** The modules a probe calls into, loaded with the cloud off, or with it on (`WSP_CLOUD=1`, read at import). */
export async function modules(cloud: boolean): Promise<{ turns: Obj; workspaces: Obj; agents: Obj; client: Obj }> {
  const load = async (): Promise<Obj> => ({
    turns: await import(`${R}/packages/host/src/verbs/turns-help.ts`),
    workspaces: await import(`${R}/packages/host/src/verbs/workspaces-help.ts`),
    agents: await import(`${R}/packages/host/src/verbs/agents-help.ts`),
    client: await import(`${R}/packages/host/src/verbs/client.ts`),
  });
  plain ??= load();
  if (!cloud) return (await plain) as never;
  cloudy ??= (async () => {
    vi.resetModules();
    vi.stubEnv("WSP_CLOUD", "1");
    try {
      return await load();
    } finally {
      vi.unstubAllEnvs();
    }
  })();
  return (await cloudy) as never;
}

/** A recorded reply of the tools' answers (`daemon/crates/wsp-mcp/tests/answers/<tool>.json`) by case name. */
export const recorded = (tool: string, name: string): Record<string, string> => {
  const cases = (JSON.parse(readFileSync(join(ANSWERS, `${tool}.json`), "utf8")) as { cases: { case: string; replies: Record<string, string> }[] }).cases;
  const hit = cases.find(c => c.case === name);
  if (hit === undefined) throw new Error(`no recorded case ${tool} / ${name}`);
  return hit.replies;
};
export const frame = (o: Obj): string => JSON.stringify({ id: 1, ok: true, ...o });
export const refused = (error: string, kind?: string): string => JSON.stringify({ id: 1, ok: false, error, ...(kind !== undefined ? { kind } : {}) });

/** Runs every case of a unit with `run(fn)` and writes `<unit>.json`. A call that has not settled within the case's
 * `within` (2 s unless the case says) is recorded `{ hung: true }`. */
export async function record(unit: string, cases: Case[], runs: Record<string, Run>): Promise<void> {
  const out: unknown[] = [];
  const only = process.env.FNS?.split(",");
  for (const given of cases) {
    if (only !== undefined && !only.includes(given.fn)) continue;
    const run = runs[given.fn];
    if (run === undefined) throw new Error(`no run for ${given.fn}`);
    const mod = await modules(given.cloud === true);
    const proto = await protocol();
    const sinks: Sinks = { frames: [], said: [], stdout: [], stderr: [], streamed: "", emitted: [], dials: 0 };
    const cwd = mkdtempSync(join(tmpdir(), "cli-c-"));
    // The text `<cwd>` in a case's arguments, script, files and environment is this case's own folder.
    const c = JSON.parse(JSON.stringify(given).split("<cwd>").join(cwd)) as Case;
    const script = c.script ?? {};
    for (const [name, text] of Object.entries(c.files ?? {})) {
      mkdirSync(join(cwd, name, ".."), { recursive: true });
      writeFileSync(join(cwd, name), text);
    }
    const ids: string[] = [];
    const client = host(script, sinks.frames, proto, ids);
    const deps = depsOf(script, sinks.frames, proto, client, ids);
    let result: Obj;
    try {
      const value = await Promise.race([
        run({ c, args: c.args ?? {}, client, deps, sinks, mod, proto, cwd }),
        new Promise<never>((_, no) => setTimeout(() => no(Object.assign(new Error("hung"), { hung: true })), c.within ?? 2000)),
      ]);
      result = { value: norm(value) ?? null };
    } catch (e) {
      result = (e as { hung?: boolean }).hung === true ? { hung: true } : { error: errorOf(e) };
    }
    // The request id a start carries is minted fresh each run: written `<request id>` wherever it shows.
    const scrub = (s: string): string => ids.reduce((t, id) => t.split(id).join("<request id>"), s.split(cwd).join("<cwd>"));
    if ("value" in result) result = { value: JSON.parse(scrub(JSON.stringify(result["value"]))) as unknown };
    out.push({
      fn: given.fn,
      case: given.case,
      ...(given.args !== undefined ? { args: given.args } : {}),
      script: given.script ?? {},
      ...(given.env !== undefined ? { env: given.env } : {}),
      ...(given.files !== undefined ? { files: given.files } : {}),
      ...(c.cloud === true ? { cloud: true } : {}),
      ...(c.within !== undefined ? { within: c.within } : {}),
      frames: sinks.frames.map(scrub),
      said: sinks.said.map(scrub),
      ...(sinks.stdout.length > 0 ? { stdout: sinks.stdout.map(scrub) } : {}),
      ...(sinks.stderr.length > 0 ? { stderr: sinks.stderr.map(scrub) } : {}),
      ...(sinks.streamed !== "" ? { streamed: scrub(sinks.streamed) } : {}),
      ...(sinks.emitted.length > 0 ? { emitted: sinks.emitted } : {}),
      ...(result as Obj),
    });
  }
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, `${unit}.json`), `${JSON.stringify({ source: "TS helpers at 65f6b89c7 called against a scripted host; recorded by cli-c's recorder", cases: out }, null, 1)}\n`);
  console.log(unit, out.length);
}
