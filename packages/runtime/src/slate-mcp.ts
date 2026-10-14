// SPDX-License-Identifier: AGPL-3.0-only
// The tool and resource runs a slate starts on its thread's own MCP servers (07, "MCP tools and resources"), on the
// computer the thread runs on. The host hands over a server as the thread's agent configures it there, already
// resolved, with the road its command starts by on another computer (slate-mcp-on.ts); this file holds one
// live client per server per thread, asks the person exactly where the thread's agent would ask before that tool (once
// per piece of code, never at the agent's bypass), calls the tool with the arguments the declaration evaluates to, and
// writes the result into the run's record scrubbed of every secret. A secret reaches a server only inside a call's
// arguments, which ride the server's stdin or the request body, never a command line.
import { spawn, type ChildProcess } from "node:child_process";
import { createHash } from "node:crypto";
import type { McpTransport } from "@wsp/catalog";
import { EXEC_OUTPUT_MAX, fmtBytes, runOutputTail, slateMachineOnBox } from "@wsp/protocol";
import type { HttpBearer, SlateAsk } from "@wsp/protocol";
import { slateDescriptionMissing, slateHostAway, slateProblem, slateServerNotSetUp, type SlateDoc, type SlateJson, type SlateProblem, type SlateRunDecl } from "@wsp/protocol/slate";
import { HELD_APPROVAL, HELD_BUDGET, HELD_PRESSED, lastResult, mapStrings, runningRecord, SECRET_DOTS, spent, stableJson, type Reshape, type RunResult, type RunApprovals, type RunBy, type RunRecord, type RunStartAnswer } from "./slate-runs.js";

/** The thread's computer where it is not this one, and this one's name. */
export interface Away {
  computer: string;
  host: string;
  /** The machine wsp made on that computer the thread runs on, where it is one: its server would start on the
   * computer outside it, so it gets no tool run. */
  machine?: string;
}

/** Why a run on="host" does not start from a thread off this computer, which a slate stored before it moved holds. */
export const hostAway = (away: Away | undefined, run: string): string | undefined => (away === undefined ? undefined : slateHostAway(`$${run}`, away.computer, away.host));

/** What a slate is refused as it is written from a thread off this computer: a command run on="host", which would run
 * here with the sign-ins here, and a run on a server the agent's config there does not name. Only servers the slate
 * before did not name are asked about, so a write that keeps its servers asks that computer nothing. */
export async function refusedAway(
  before: SlateDoc | null,
  doc: SlateDoc | null | undefined,
  away: Away | undefined,
  missingOn: ((servers: string[]) => Promise<{ agent: { id: string; name: string }; thread: string; missing: string[] }>) | undefined,
): Promise<SlateProblem[]> {
  if (away === undefined || !doc) return [];
  const runs = Object.entries(doc.runs);
  const problems = runs.flatMap(([name, d]) => (d.kind === "cmd" && d.on === "host" ? [slateProblem("K711", slateHostAway(`$${name}`, away.computer, away.host))] : []));
  const tools = runs.flatMap(([name, d]) => (d.kind === "cmd" ? [] : [name]));
  if (away.machine !== undefined) return tools.length === 0 ? problems : [...problems, slateProblem("K712", slateMachineOnBox(tools, away.machine, away.computer))];
  const known = new Set(Object.values(before?.runs ?? {}).flatMap(d => (d.kind === "cmd" ? [] : [d.server])));
  const asked = [...new Set(runs.flatMap(([, d]) => (d.kind === "cmd" || known.has(d.server) ? [] : [d.server])))];
  if (asked.length === 0 || missingOn === undefined) return problems;
  const { agent, thread, missing } = await missingOn(asked);
  return [...problems, ...missing.map(server => slateProblem("K710", slateServerNotSetUp(server, away.computer, agent, thread)))];
}

/** `files` is the text of each slate file its then reads, as on a command. */
export type McpRunDecl = Extract<SlateRunDecl, { kind: "tool" | "resource" }> & { files?: Record<string, string> };

/** A server as the thread's agent would start it there: references read, its own variables in `env`. `secrets` are
 * the values handed to it, hidden from whatever it says back. */
export interface McpServerSpec {
  transport: McpTransport;
  cwd: string;
  env: Readonly<Record<string, string>>;
  secrets: readonly string[];
  /** Where a command server runs: on the thread's own computer through its daemon. Absent runs it on this one. */
  open?: McpPipeOpen;
  /** Where an http server is asked from, one road per connection, handed this computer's own fetch: the thread's own
   * computer through its daemon, or this one with the sign-in the agent keeps here. Absent asks it from this one. */
  post?: (here: McpPost) => McpPost;
  /** An http server's sign-in the thread's agent keeps on its own computer, read there: by its daemon, or by this host
   * where that computer is this one. */
  signIn?: McpSignIn;
}

/** Where the bearer is, read there; the agent's own word on it asked again there, true where it holds one, which
 * renews a token near its end the way the agent does; and the line the person signs in with there. */
export interface McpSignIn {
  bearer: HttpBearer;
  renew(): Promise<boolean>;
  line: string;
  /** On this computer, the bearer read here at each message; absent where only that computer's daemon reads it. */
  read?(): Promise<string | undefined>;
}

/** What an http server answered one message, wherever it was asked from. */
export interface McpHttpAnswer {
  status: number;
  contentType?: string;
  sessionId?: string;
  body: string;
}

/** One message to an http server and the end of the road it goes by. */
export interface McpPost {
  send(t: Extract<McpTransport, { kind: "http" }>, req: { headers: Record<string, string>; body: string; ms: number }): Promise<McpHttpAnswer>;
  close(): void;
}

/** What a command server's process says, wherever it runs. `exit` is said once, with why it ended. */
export interface McpPipeEvents {
  out(chunk: Buffer): void;
  err(text: string): void;
  exit(why: string): void;
}

/** A command server's stdin and the stop of its whole process group; `pid` where it runs on this computer. */
export interface McpPipe {
  write(text: string): void;
  kill(): void;
  readonly pid?: number;
}

export type McpPipeOpen = (t: Extract<McpTransport, { kind: "stdio" }>, spec: McpServerSpec, on: McpPipeEvents) => McpPipe;

export interface McpToolInfo {
  name: string;
  title?: string;
  description?: string;
  inputSchema?: unknown;
  outputSchema?: unknown;
  annotations?: { title?: string; readOnlyHint?: boolean; destructiveHint?: boolean; idempotentHint?: boolean; openWorldHint?: boolean };
}

export interface McpResourceInfo {
  uri: string;
  name?: string;
  mimeType?: string;
  description?: string;
}

export interface McpRunStart {
  threadId: string;
  run: string;
  decl: McpRunDecl;
  by: RunBy;
  /** The tool's arguments evaluated now, a secret as slateSecretMark(name) wherever it stands. */
  args: () => Record<string, SlateJson>;
  /** The run's record before this start, whose result stays in view while this one runs. */
  last?: RunRecord;
}

/** When the thread's agent asks before an MCP tool at its mode now: never (its bypass), before every tool, or before
 * every tool its server does not mark read-only. */
export type ToolAsks = "never" | "every" | "unless-read-only";

export interface SlateMcpDeps {
  /** The thread's agent's server by name, resolved; throws with the reason where there is none. */
  server(threadId: string, name: string): Promise<McpServerSpec>;
  onRecord(threadId: string, run: string, record: RunRecord): void;
  /** The thread's held asks changed after the fact (a tool list arrived, a tool that asks was found). */
  onAsks?(threadId: string): void;
  asks(threadId: string): ToolAsks;
  approvals: RunApprovals;
  secrets: { plaintext(threadId: string, name: string): string | undefined; scrub(threadId: string, text: string): string };
  /** A run's `then` on this computer, in the thread's folder, the raw result already scrubbed on its stdin. */
  reshape(threadId: string, cmd: string, input: string): Reshape;
  computer(threadId: string): string;
  /** Names a stdio server's process as the thread's tool server until the returned release. */
  own?: (threadId: string, pid: number, kind: "tool") => () => void;
  now?: () => number;
  /** How long a connection lives unused while its slate is not shown and no `always` timer keeps it. */
  idleMs?: number;
  /** A call's deadline where the server's config names none. */
  callMs?: number;
}

export interface SlateMcp {
  /** What a start inside a batch will give, before it is started, with the arguments it would send: held where it is
   * known to ask, else running. */
  provisional(threadId: string, decl: McpRunDecl, last: RunRecord | undefined, args: Record<string, SlateJson>): RunRecord;
  start(req: McpRunStart): RunStartAnswer;
  /** The person's answer to a held run, by the key its ask carried. Every run held on that key moves. */
  approve(threadId: string, key: string, scope: "once" | "thread"): void;
  deny(threadId: string, key: string): void;
  /** Whether a key is one of this module's: a tool or resource run's code. */
  owns(threadId: string, key: string): boolean;
  held(threadId: string): SlateAsk[];
  /** The held run a key asks for and what it sends, a secret as dots, so two allowed of one tool read apart. */
  named(threadId: string, key: string): { run: string; cmd: string } | undefined;
  /** Settles once every tool list and connect a start of this thread is waiting on has, or after `ms`. */
  pending(threadId: string, ms?: number): Promise<void>;
  cancel(threadId: string, run: string): void;
  stopAll(threadId: string, opts?: { quiet?: boolean; why?: string }): void;
  /** The servers the slate's document names, and which of them an `always` timer keeps open. Others close. */
  servers(threadId: string, names: readonly string[], kept: readonly string[]): void;
  shown(threadId: string, shown: boolean): void;
  /** Closes the thread's servers and keeps its runs: its machine napped, and the next start opens them again. */
  closeServers(threadId: string): void;
  /** slate_catalog <server>: its tools, each input schema folded to a line per property, and how this thread asks. */
  catalog(threadId: string, server: string): Promise<string>;
  /** What a write is refused for its runs: what the thread's computer refused (`away`), else each tool run the write
   * adds or changes that says nothing of what it does, on a tool its server does not mark read-only. A run kept as it
   * was is never judged; a server is asked when its list is not held, and one that does not answer marks nothing
   * read-only. */
  refusedRuns(threadId: string, before: SlateDoc | null, doc: SlateDoc | null | undefined, away: SlateProblem[]): Promise<SlateProblem[]>;
  /** Forgets the thread's runs and servers; with `keep`, a server and a run going on are left running. */
  drop(threadId: string, keep?: boolean): void;
  close(): void;
}

/** What the person allows of a tool or resource run: its server, its tool or uri, the arguments as they will be sent (a
 * secret as its mark, which names it and never holds its value), and its then with the files that reads. Anything
 * that changes what is sent, a value or a formula alike, asks again. */
export const codeKey = (decl: McpRunDecl, args: Record<string, SlateJson>): string =>
  `mcp:${decl.server}#${createHash("sha256")
    .update(stableJson([decl.kind, decl.server, decl.kind === "tool" ? decl.tool : decl.uri, decl.kind === "tool" ? args : null, decl.then ?? null, Object.entries(decl.files ?? {}).sort()]))
    .digest("hex")
    .slice(0, 32)}`;

const SECRET_MARK = "\u0000wsp-secret:";
/** Where a secret stands in a tool's evaluated arguments: replaced by its plaintext at the call, by dots on a sheet. */
export const slateSecretMark = (name: string): string => `${SECRET_MARK}${name}\u0000`;
const MARKED = /\u0000wsp-secret:([A-Za-z_][A-Za-z0-9_-]*)\u0000/g;

const PROTOCOL_VERSION = "2025-06-18";
const CONNECT_MS = 20_000;
const CALL_MS = 60_000;
const IDLE_MS = 5 * 60_000;
const SWEEP_MS = 30_000;
const ERR_KEPT = 4_000;
/** How long a write waits for a server's tool list to tell whether a new run needs its description. */
const DESCRIBE_MS = 10_000;
/** The most one message from a server may be, on either transport: a command's own cap, eight times what a slate's
 * values hold, so any answer worth keeping fits and a runaway one is cut off before it is parsed. */
const MESSAGE_MAX_BYTES = EXEC_OUTPUT_MAX;
const TOO_BIG = `the server sent a message over ${fmtBytes(MESSAGE_MAX_BYTES)}`;
/** How deep and how wide a parsed answer may be before anything walks it. */
const DEPTH_MAX = 64;
const ITEMS_MAX = 100_000;

/** Why a parsed answer is too deep or too wide to walk, or nothing. Iterative, so a deep one cannot take the stack. */
function shapeRefusal(value: unknown): string | undefined {
  const stack: [unknown, number][] = [[value, 0]];
  let items = 0;
  while (stack.length > 0) {
    const [v, depth] = stack.pop()!;
    if (v === null || typeof v !== "object") continue;
    if (depth >= DEPTH_MAX) return `the server's answer is nested over ${DEPTH_MAX} deep`;
    const children = Array.isArray(v) ? v : Object.values(v);
    items += children.length;
    if (items > ITEMS_MAX) return `the server's answer holds over ${ITEMS_MAX} entries`;
    for (const c of children) stack.push([c, depth + 1]);
  }
  return undefined;
}

/** JSON a tool put in its text, or nothing where it does not parse or is too deep or wide to walk. */
function parsedText(text: string): SlateJson | undefined {
  try {
    const value = JSON.parse(text) as SlateJson;
    return shapeRefusal(value) === undefined ? value : undefined;
  } catch {
    return undefined;
  }
}

/** An HTTP answer's body, read no further than one message may be. */
async function boundedText(res: Response): Promise<string> {
  if (Number(res.headers.get("content-length") ?? 0) > MESSAGE_MAX_BYTES) {
    await res.body?.cancel();
    throw new Error(TOO_BIG);
  }
  const reader = res.body?.getReader();
  if (reader === undefined) return "";
  const parts: Uint8Array[] = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > MESSAGE_MAX_BYTES) {
      await reader.cancel();
      throw new Error(TOO_BIG);
    }
    parts.push(value);
  }
  return Buffer.concat(parts).toString("utf8");
}



const messageOf = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/** How this thread's tool runs ask, as slate_catalog tells the agent. */
const ASKS_LINE: Record<ToolAsks, string> = {
  never: "This thread's agent runs every tool without asking, and so do its slate's runs.",
  every: "This thread's agent asks before every tool, so the person allows each run's code once, on the slate: new arguments, a new then or a new tool ask again.",
  "unless-read-only": "This thread's agent asks before a tool not marked read-only, so the person allows each such run's code once, on the slate: new arguments, a new then or a new tool ask again.",
};

// ---- the client ----

interface RpcError {
  code?: number;
  message?: string;
}

interface Client {
  request(method: string, params: Record<string, unknown>, ms: number): { id: number; answer: Promise<unknown> };
  notify(method: string, params?: Record<string, unknown>): void;
  close(): void;
  readonly closed: boolean;
  /** What the server said on stderr, its tail, for a failure's why. */
  said(): string;
  /** The server's process on this computer, for a stdio server. */
  readonly pid?: number;
}

class RpcFailed extends Error {
  constructor(readonly error: RpcError) {
    super(`the server answered with error ${error.code ?? "?"}${typeof error.message === "string" ? `: ${error.message}` : ""}`);
  }
}

/** The server's command on this computer, in a process group of its own. */
const spawnHere: McpPipeOpen = (t, spec, on) => {
  const child: ChildProcess = spawn(t.command, t.args, { cwd: t.cwd ?? spec.cwd, env: { ...spec.env, ...t.env }, detached: true, stdio: ["pipe", "pipe", "pipe"] });
  child.stdin?.on("error", () => {});
  child.stderr?.setEncoding("utf8");
  child.stderr?.on("data", on.err);
  child.stdout?.on("data", on.out);
  child.on("error", e => on.exit(`${t.command} could not be started: ${e.message}`));
  child.on("exit", (code, signal) => on.exit(`the server exited${code !== null ? ` with ${code}` : signal !== null ? ` on ${signal}` : ""}`));
  return {
    ...(child.pid !== undefined ? { pid: child.pid } : {}),
    write: text => void child.stdin?.write(text),
    kill() {
      if (child.pid === undefined) return;
      try {
        process.kill(-child.pid, "SIGKILL");
      } catch {
        // Gone already.
      }
    },
  };
};

function stdioClient(t: Extract<McpTransport, { kind: "stdio" }>, spec: McpServerSpec, onClose: () => void): Client {
  const waiting = new Map<number, { ok: (v: unknown) => void; no: (e: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  let next = 1;
  let closed = false;
  let err = "";
  let buffer = Buffer.alloc(0);
  let pipe: McpPipe | undefined;
  const finish = (why: string): void => {
    if (closed) return;
    closed = true;
    for (const w of waiting.values()) {
      clearTimeout(w.timer);
      w.no(new Error(why));
    }
    waiting.clear();
    pipe?.kill();
    onClose();
  };
  const send = (msg: Record<string, unknown>): void => {
    if (!closed) pipe?.write(`${JSON.stringify(msg)}\n`);
  };
  const out = (chunk: Buffer): void => {
    if (closed) return;
    buffer = Buffer.concat([buffer, chunk]);
    let at: number;
    while ((at = buffer.indexOf(10)) >= 0) {
      if (at > MESSAGE_MAX_BYTES) return finish(TOO_BIG);
      const line = buffer.subarray(0, at).toString("utf8").trim();
      buffer = buffer.subarray(at + 1);
      if (line === "") continue;
      let msg: { id?: unknown; method?: unknown; result?: unknown; error?: RpcError };
      try {
        msg = JSON.parse(line);
      } catch {
        continue;
      }
      if (typeof msg.method === "string") {
        // A request of the server's own: ping is answered, anything else is a method this client does not offer.
        if (msg.id !== undefined) send(msg.method === "ping" ? { jsonrpc: "2.0", id: msg.id, result: {} } : { jsonrpc: "2.0", id: msg.id, error: { code: -32601, message: "not offered" } });
        continue;
      }
      if (typeof msg.id !== "number") continue;
      const w = waiting.get(msg.id);
      if (w === undefined) continue;
      waiting.delete(msg.id);
      clearTimeout(w.timer);
      const bad = shapeRefusal(msg.result);
      if (msg.error !== undefined) w.no(new RpcFailed(msg.error));
      else if (bad !== undefined) w.no(new Error(bad));
      else w.ok(msg.result);
    }
    // A message still unended past the cap is never going to be one worth reading.
    if (buffer.length > MESSAGE_MAX_BYTES) finish(TOO_BIG);
  };
  pipe = (spec.open ?? spawnHere)(t, spec, { out, err: text => (err = (err + text).slice(-ERR_KEPT)), exit: finish });
  return {
    request(method, params, ms) {
      const id = next++;
      const answer = new Promise<unknown>((ok, no) => {
        if (closed) return no(new Error("the server's connection is closed"));
        const timer = setTimeout(() => {
          waiting.delete(id);
          no(new Error(`the server did not answer ${method} within ${Math.round(ms / 1000)} s`));
        }, ms);
        waiting.set(id, { ok, no, timer });
        send({ jsonrpc: "2.0", id, method, params });
      });
      return { id, answer };
    },
    notify: (method, params) => send({ jsonrpc: "2.0", method, ...(params !== undefined ? { params } : {}) }),
    close: () => finish("the connection was closed"),
    get closed() {
      return closed;
    },
    said: () => err,
    ...(pipe.pid !== undefined ? { pid: pipe.pid } : {}),
  };
}

/** This computer's own fetch, no redirect followed, a failed answer's body never read. */
const fetchHere: McpPost = {
  async send(t, req) {
    const res = await fetch(t.url, { method: "POST", redirect: "error", signal: AbortSignal.timeout(req.ms), headers: req.headers, body: req.body });
    const answer = { status: res.status, ...(res.headers.get("content-type") !== null ? { contentType: res.headers.get("content-type")! } : {}), ...(res.headers.get("mcp-session-id") !== null ? { sessionId: res.headers.get("mcp-session-id")! } : {}) };
    if (!res.ok) await res.body?.cancel();
    return { ...answer, body: res.ok ? await boundedText(res) : "" };
  },
  close() {},
};

/** Streamable HTTP: one POST per message, the answer as JSON or on an event stream's data lines. No redirect is
 * followed, so the config's headers reach its own address alone. */
function httpClient(t: Extract<McpTransport, { kind: "http" }>, spec: McpServerSpec, onClose: () => void): Client {
  let session: string | undefined;
  let next = 1;
  let closed = false;
  const road = spec.post?.(fetchHere) ?? fetchHere;
  const post = async (body: Record<string, unknown>, ms: number): Promise<McpHttpAnswer> => {
    const headers = { ...t.headers, "Content-Type": "application/json", Accept: "application/json, text/event-stream", "MCP-Protocol-Version": PROTOCOL_VERSION, ...(session !== undefined ? { "Mcp-Session-Id": session } : {}) };
    const res = await road.send(t, { headers, body: JSON.stringify(body), ms });
    session = res.sessionId ?? session;
    return res;
  };
  return {
    request(method, params, ms) {
      const id = next++;
      const answer = (async () => {
        if (closed) throw new Error("the server's connection is closed");
        const res = await post({ jsonrpc: "2.0", id, method, params }, ms);
        if (res.status === 401 || res.status === 403) throw new Error(`its address wants a sign-in (${res.status}); sign in where the agent keeps it`);
        if (res.status < 200 || res.status > 299) throw new Error(`its address answered ${method} with ${res.status}`);
        const lines = (res.contentType ?? "").includes("event-stream") ? res.body.split("\n").filter(l => l.startsWith("data:")).map(l => l.slice(5).trim()) : [res.body];
        for (const line of lines) {
          let msg: { id?: unknown; result?: unknown; error?: RpcError };
          try {
            msg = JSON.parse(line);
          } catch {
            continue;
          }
          if (msg.id !== id) continue;
          if (msg.error !== undefined) throw new RpcFailed(msg.error);
          const bad = shapeRefusal(msg.result);
          if (bad !== undefined) throw new Error(bad);
          return msg.result;
        }
        throw new Error(`its answer to ${method} could not be read`);
      })();
      return { id, answer };
    },
    notify(method, params) {
      if (!closed) void post({ jsonrpc: "2.0", method, ...(params !== undefined ? { params } : {}) }, CONNECT_MS).catch(() => {});
    },
    close() {
      if (closed) return;
      closed = true;
      road.close();
      onClose();
    },
    get closed() {
      return closed;
    },
    said: () => "",
  };
}

interface Conn {
  client: Client;
  spec: McpServerSpec;
  /** Whether the server offers resources, off its initialize answer. */
  resources: boolean;
  tools?: Promise<McpToolInfo[]>;
  usedAt: number;
  busy: number;
}

// ---- the schema, folded ----

const sentence = (text: unknown, max = 160): string | undefined => {
  if (typeof text !== "string") return undefined;
  const one = text.replace(/\s+/g, " ").trim();
  if (one === "") return undefined;
  const cut = one.match(/^.*?[.!?](?=\s+[A-Z(]|$)/)?.[0] ?? one;
  return cut.length > max ? `${cut.slice(0, max - 1)}…` : cut;
};

type Schema = Record<string, unknown>;

/** A local `$ref` followed, and an `allOf` of one schema taken as that schema, as pydantic writes a model field. */
function deref(p: Schema, root: Schema, seen = 0): Schema {
  const ref = p["$ref"];
  if (typeof ref === "string" && ref.startsWith("#/") && seen < 8) {
    let at: unknown = root;
    for (const seg of ref.slice(2).split("/")) at = typeof at === "object" && at !== null ? (at as Schema)[seg.replace(/~1/g, "/").replace(/~0/g, "~")] : undefined;
    if (typeof at === "object" && at !== null) return deref({ ...(at as Schema), ...Object.fromEntries(Object.entries(p).filter(([k]) => k !== "$ref")) }, root, seen + 1);
  }
  const all = p["allOf"];
  if (Array.isArray(all) && all.length === 1 && typeof all[0] === "object" && all[0] !== null) return deref({ ...(all[0] as Schema), ...Object.fromEntries(Object.entries(p).filter(([k]) => k !== "allOf")) }, root, seen + 1);
  return p;
}

function typeOf(raw: Schema, root: Schema): string {
  const p = deref(raw, root);
  if (Array.isArray(p["enum"])) return p["enum"].map(v => JSON.stringify(v)).join(" | ");
  if (p["const"] !== undefined) return JSON.stringify(p["const"]);
  const t = p["type"];
  const named = typeof t === "string" ? t : Array.isArray(t) ? t.filter(x => typeof x === "string").join(" | ") : undefined;
  if (named === "array") {
    const items = p["items"];
    return typeof items === "object" && items !== null ? `${typeOf(items as Schema, root)}[]` : "array";
  }
  if (named !== undefined && named !== "") return named;
  for (const k of ["oneOf", "anyOf"]) if (Array.isArray(p[k])) return (p[k] as Schema[]).map(x => typeOf(x, root)).join(" | ");
  return "any";
}

/** One line per property, nested objects (and objects inside arrays) as dotted names, three deep. */
export function foldSchema(schema: unknown, prefix = "", depth = 0, root: Schema = schema as Schema): string[] {
  if (typeof schema !== "object" || schema === null) return [];
  const { properties, required } = deref(schema as Schema, root) as { properties?: unknown; required?: unknown };
  if (typeof properties !== "object" || properties === null) return [];
  const needed = new Set(Array.isArray(required) ? required : []);
  const lines: string[] = [];
  for (const [name, raw] of Object.entries(properties as Record<string, unknown>)) {
    if (typeof raw !== "object" || raw === null) continue;
    const p = deref(raw as Schema, root);
    const facts = [typeOf(p, root), ...(needed.has(name) ? ["required"] : []), ...(p["default"] !== undefined ? [`default ${JSON.stringify(p["default"])}`] : [])];
    for (const [k, word] of [["minimum", "min"], ["maximum", "max"], ["maxLength", "max length"], ["format", "format"]] as const) if (p[k] !== undefined) facts.push(`${word} ${String(p[k])}`);
    const about = sentence(p["description"]);
    lines.push(`  ${prefix}${name}: ${facts.join(", ")}${about !== undefined ? `. ${about}` : ""}`);
    if (depth >= 2) continue;
    if (p["properties"] !== undefined) lines.push(...foldSchema(p, `${prefix}${name}.`, depth + 1, root));
    const items = typeof p["items"] === "object" && p["items"] !== null ? deref(p["items"] as Schema, root) : undefined;
    if (items?.["properties"] !== undefined) lines.push(...foldSchema(items, `${prefix}${name}[].`, depth + 1, root));
  }
  return lines;
}

const hintsOf = (tool: McpToolInfo, structured: boolean | undefined): string[] => {
  const shape = tool.outputSchema !== undefined || structured === true ? "returns data" : "text only";
  const a = tool.annotations;
  if (a === undefined) return [shape];
  return [shape, ...(a.readOnlyHint === true ? ["read-only"] : []), ...(a.destructiveHint === true ? ["destructive"] : []), ...(a.idempotentHint === true ? ["idempotent"] : []), ...(a.openWorldHint === true ? ["reaches outside"] : [])];
};

// ---- the runs ----

interface Live {
  record: RunRecord;
  gen: number;
  /** The call in flight, for a cancel to tell the server. */
  inflight?: { conn: Conn; id: number };
  reshaping?: Reshape;
  /** The last call's result, kept in view while the next one runs and through a hold. */
  result?: RunResult;
  /** A held start, its arguments read once so the person allows exactly what then goes, and the key it asks by. */
  held?: { req: McpRunStart; ask: HeldAsk; key?: string };
  starts: number[];
  /** The person's own starts, on a budget of their own. */
  pressed?: number[];
}

type HeldAsk = "code" | "budget" | "pressed";

interface ThreadMcp {
  runs: Map<string, Live>;
  conns: Map<string, Promise<Conn>>;
  named: Set<string>;
  kept: Set<string>;
  shown: boolean;
  pending: Set<Promise<unknown>>;
}

export function createSlateMcp(deps: SlateMcpDeps): SlateMcp {
  const now = deps.now ?? Date.now;
  const idleMs = deps.idleMs ?? IDLE_MS;
  const threads = new Map<string, ThreadMcp>();
  /** Tool lists by thread and server, kept past a closed connection so a sheet and a provisional start can read them. */
  const lists = new Map<string, McpToolInfo[]>();
  const listKey = (threadId: string, server: string): string => `${threadId}\u0000${server}`;
  /** Whether a tool's answers carried a structured result, by thread, server and tool, off the runs that called it. */
  const seen = new Map<string, boolean>();
  const seenKey = (threadId: string, server: string, tool: string): string => `${listKey(threadId, server)}\u0000${tool}`;

  const thread = (threadId: string): ThreadMcp => {
    let t = threads.get(threadId);
    if (t === undefined) {
      t = { runs: new Map(), conns: new Map(), named: new Set(), kept: new Set(), shown: false, pending: new Set() };
      threads.set(threadId, t);
    }
    return t;
  };
  const live = (threadId: string, run: string, last: RunRecord | undefined): Live => {
    const t = thread(threadId);
    let l = t.runs.get(run);
    if (l === undefined) {
      const result = last === undefined ? undefined : lastResult(last);
      l = { record: { state: "idle", runs: last?.runs ?? 0 }, gen: 0, starts: [], ...(result !== undefined ? { result } : {}) };
      t.runs.set(run, l);
    }
    return l;
  };
  const track = <T>(threadId: string, p: Promise<T>): Promise<T> => {
    const t = thread(threadId);
    t.pending.add(p);
    const off = (): void => void t.pending.delete(p);
    p.then(off, off);
    return p;
  };

  /** Every secret's plaintext and the server's own values put out of what it said. */
  const clean = (threadId: string, spec: McpServerSpec | undefined, text: string): string => {
    let out = deps.secrets.scrub(threadId, text);
    for (const v of [...(spec?.secrets ?? [])].sort((a, b) => b.length - a.length)) if (v.length >= 4 && out.includes(v)) out = out.split(v).join("***");
    return out;
  };

  const connect = (threadId: string, server: string): Promise<Conn> => {
    const t = thread(threadId);
    const was = t.conns.get(server);
    if (was !== undefined) return was;
    const made = (async (): Promise<Conn> => {
      const spec = await deps.server(threadId, server);
      let release: (() => void) | undefined;
      const gone = (): void => {
        release?.();
        if (t.conns.get(server) === made) t.conns.delete(server);
      };
      const client = spec.transport.kind === "stdio" ? stdioClient(spec.transport, spec, gone) : httpClient(spec.transport, spec, gone);
      if (!client.closed && client.pid !== undefined) release = deps.own?.(threadId, client.pid, "tool");
      try {
        const init = (await client.request("initialize", { protocolVersion: PROTOCOL_VERSION, capabilities: {}, clientInfo: { name: "wsp", version: "1" } }, CONNECT_MS).answer) as { capabilities?: { resources?: unknown } } | undefined;
        client.notify("notifications/initialized");
        return { client, spec, resources: init?.capabilities?.resources !== undefined, usedAt: now(), busy: 0 };
      } catch (e) {
        const said = client.said().trim().split("\n").pop();
        client.close();
        throw new Error(clean(threadId, spec, `${server} did not start: ${messageOf(e)}${said !== undefined && said !== "" ? `; it said: ${said}` : ""}`));
      }
    })();
    t.conns.set(server, made);
    made.catch(() => t.conns.get(server) === made && t.conns.delete(server));
    return made;
  };

  const toolsOf = (threadId: string, server: string, conn: Conn): Promise<McpToolInfo[]> => {
    conn.tools ??= (async () => {
      const tools: McpToolInfo[] = [];
      let cursor: string | undefined;
      for (let page = 0; page < 20; page++) {
        const got = (await conn.client.request("tools/list", cursor !== undefined ? { cursor } : {}, CONNECT_MS).answer) as { tools?: unknown[]; nextCursor?: unknown } | undefined;
        for (const raw of got?.tools ?? []) if (typeof raw === "object" && raw !== null && typeof (raw as { name?: unknown }).name === "string") tools.push(raw as McpToolInfo);
        if (typeof got?.nextCursor !== "string") break;
        cursor = got.nextCursor;
      }
      lists.set(listKey(threadId, server), tools);
      return tools;
    })();
    conn.tools.catch(() => delete conn.tools);
    return conn.tools;
  };

  /** A server's tool list, connecting for it; a failure is an empty list, the call itself says why. */
  const listFor = (threadId: string, server: string): Promise<McpToolInfo[]> =>
    track(
      threadId,
      connect(threadId, server)
        .then(c => toolsOf(threadId, server, c))
        .catch(() => lists.get(listKey(threadId, server)) ?? []),
    );

  const write = (threadId: string, run: string, l: Live, record: RunRecord): RunRecord => {
    l.record = record;
    const result = record.state === "running" ? undefined : lastResult(record);
    if (result !== undefined) l.result = result;
    deps.onRecord(threadId, run, record);
    return record;
  };

  const shownArgs = (threadId: string, args: Record<string, SlateJson>): Record<string, SlateJson> =>
    mapStrings(args, s =>
      s.replace(MARKED, (_m, name: string) => {
        const text = deps.secrets.plaintext(threadId, name);
        return text === undefined ? `${SECRET_DOTS} (not filled)` : `${SECRET_DOTS} (${text.length})`;
      }),
    ) as Record<string, SlateJson>;

  const listed = (threadId: string, decl: McpRunDecl): McpToolInfo | undefined => (decl.kind === "tool" ? lists.get(listKey(threadId, decl.server))?.find(t => t.name === decl.tool) : undefined);

  /** Whether this start waits for the person: every start of a run that names confirm; else, where the thread's agent
   * asks before this tool, until its code is allowed. Undefined where only the hints of a tool not listed yet decide. */
  const asksNow = (threadId: string, decl: McpRunDecl, info: McpToolInfo | undefined, args: Record<string, SlateJson>): boolean | undefined => {
    if (decl.kind === "tool" && decl.confirm !== undefined) return true;
    const rule = deps.asks(threadId);
    if (rule === "never" || deps.approvals.has(threadId, codeKey(decl, args))) return false;
    if (rule === "every") return true;
    if (decl.kind === "resource") return false;
    return info === undefined ? undefined : info.annotations?.readOnlyHint !== true;
  };

  const askOf = (threadId: string, run: string, req: McpRunStart, key: string): SlateAsk => {
    const d = req.decl;
    const description = d.description ?? sentence(listed(threadId, d)?.description, 240);
    return {
      key,
      run,
      kind: "tool",
      server: d.server,
      tool: d.kind === "tool" ? d.tool : d.uri,
      ...(d.kind === "resource" ? { resource: true as const } : {}),
      computer: deps.computer(threadId),
      why: HELD_APPROVAL,
      ...(description !== undefined ? { description } : {}),
      args: d.kind === "tool" ? shownArgs(threadId, req.args()) : {},
      ...(d.kind === "tool" && typeof d.confirm === "string" ? { confirm: d.confirm } : {}),
      ...(d.then !== undefined ? { then: d.then } : {}),
      ...(d.files !== undefined ? { files: d.files } : {}),
    };
  };

  const hold = (req: McpRunStart, l: Live, ask: HeldAsk, runs: number, beside = false): RunStartAnswer => {
    l.held = { req, ask, ...(ask === "code" ? { key: codeKey(req.decl, req.args()) } : {}) };
    // A start held beside a call still running leaves that call and its record be: it stops only once the new start
    // launches.
    const record = beside ? l.record : write(req.threadId, req.run, l, { ...(l.result ?? {}), state: "held", why: ask === "code" ? HELD_APPROVAL : ask === "pressed" ? HELD_PRESSED : HELD_BUDGET, runs });
    // The sheet says what the tool does in the server's words where the agent gave none, off its list.
    if (ask === "code" && !lists.has(listKey(req.threadId, req.decl.server))) void listFor(req.threadId, req.decl.server).then(() => deps.onAsks?.(req.threadId));
    return { outcome: "held", record };
  };

  /** The tool's arguments with every secret in place, for the call alone. */
  const realArgs = (threadId: string, args: Record<string, SlateJson>): Record<string, SlateJson> | { missing: string } => {
    let missing: string | undefined;
    const out = mapStrings(args, s =>
      s.replace(MARKED, (_m, name: string) => {
        const text = deps.secrets.plaintext(threadId, name);
        if (text === undefined) missing ??= name;
        return text ?? "";
      }),
    ) as Record<string, SlateJson>;
    return missing !== undefined ? { missing } : out;
  };

  /** The record a call's answer gives, and the raw result a `then` reads: the structured result as JSON, else the text. */
  const resultOf = (threadId: string, spec: McpServerSpec, decl: McpRunDecl, got: unknown): { record: Omit<RunRecord, "runs">; raw: string } => {
    const scrubbed = (text: string): string => clean(threadId, spec, text);
    if (decl.kind === "resource") {
      const contents = Array.isArray((got as { contents?: unknown })?.contents) ? ((got as { contents: { text?: unknown; mimeType?: unknown; blob?: unknown }[] }).contents) : [];
      const texts = contents.flatMap(c => (typeof c.text === "string" ? [c.text] : []));
      const out = scrubbed(texts.join("\n"));
      const parsed = texts.length === 1 ? parsedText(texts[0]!) : undefined;
      const json = parsed === undefined ? undefined : mapStrings(parsed, scrubbed);
      return { record: { state: "done", exit: 0, out: runOutputTail(out), ...(json !== undefined ? { json } : {}) }, raw: out };
    }
    const r = (got ?? {}) as { content?: { type?: unknown; text?: unknown }[]; structuredContent?: unknown; isError?: unknown };
    const texts = (Array.isArray(r.content) ? r.content : []).flatMap(c => (c?.type === "text" && typeof c.text === "string" ? [c.text] : []));
    const text = scrubbed(texts.join("\n"));
    if (r.isError === true) return { record: { state: "failed", why: "the tool said it failed", exit: 1, out: "", err: runOutputTail(text) }, raw: text };
    const structured = r.structuredContent !== undefined && r.structuredContent !== null;
    if (decl.kind === "tool") {
      const at = seenKey(threadId, decl.server, decl.tool);
      seen.set(at, seen.get(at) === true || structured);
    }
    const parsed = structured ? (r.structuredContent as SlateJson) : texts.length > 0 ? parsedText(texts.length === 1 ? texts[0]! : texts.join("\n")) : undefined;
    const json = parsed === undefined ? undefined : mapStrings(parsed, scrubbed);
    return { record: { state: "done", exit: 0, out: runOutputTail(text), ...(json !== undefined ? { json } : {}), ...(structured ? {} : { text: true as const }) }, raw: structured ? JSON.stringify(json) : text };
  };

  /** The run goes running now and its call follows; a tool found on the way to be one that asks holds it for the person. */
  const launch = (req: McpRunStart, l: Live, confirmed: boolean): RunStartAnswer => {
    const { threadId, run, decl } = req;
    // Starting again stops the call still running, now that this one goes.
    if (l.record.state === "running") stop(threadId, run, l, undefined);
    const gen = ++l.gen;
    const prior = l.record.runs;
    const runs = prior + 1;
    const startedAt = now();
    const record = write(threadId, run, l, runningRecord(l.result, runs, startedAt));
    const work = (async (): Promise<void> => {
      let spec: McpServerSpec | undefined;
      try {
        const conn = await connect(threadId, decl.server);
        spec = conn.spec;
        if (l.gen !== gen) return;
        conn.busy += 1;
        try {
          let method: string;
          let params: Record<string, unknown>;
          if (decl.kind === "tool") {
            const tools = await toolsOf(threadId, decl.server, conn);
            if (l.gen !== gen) return;
            const info = tools.find(x => x.name === decl.tool);
            if (info === undefined) throw new Error(`${decl.server} has no tool ${decl.tool}; slate_catalog ${decl.server} lists its tools`);
            if (!confirmed && asksNow(threadId, decl, info, req.args()) === true) {
              hold(req, l, "code", prior);
              deps.onAsks?.(threadId);
              return;
            }
            const args = realArgs(threadId, req.args());
            if ("missing" in args) throw new Error(`the secret $${args.missing} is not filled`);
            method = "tools/call";
            params = { name: decl.tool, arguments: args };
          } else {
            method = "resources/read";
            params = { uri: decl.uri };
          }
          const timeoutSec = conn.spec.transport.kind === "stdio" ? conn.spec.transport.toolTimeoutSec : undefined;
          const call = conn.client.request(method, params, timeoutSec !== undefined ? timeoutSec * 1000 : (deps.callMs ?? CALL_MS));
          l.inflight = { conn, id: call.id };
          const got = await call.answer;
          if (l.gen !== gen) return;
          delete l.inflight;
          const { record: result, raw } = resultOf(threadId, conn.spec, decl, got);
          let record: Omit<RunRecord, "runs"> = result;
          if (decl.then !== undefined && result.state === "done") {
            const shaping = deps.reshape(threadId, decl.then, raw);
            l.reshaping = shaping;
            const answer = await shaping.done;
            if (l.gen !== gen) return;
            delete l.reshaping;
            const { json: _json, text: _text, ...kept } = result;
            record = "json" in answer ? { ...kept, json: answer.json } : { ...kept, state: "failed", why: answer.why, exit: answer.exit, err: answer.err };
          }
          const endedAt = now();
          write(threadId, run, l, { ...record, startedAt, endedAt, ms: endedAt - startedAt, runs });
        } finally {
          conn.busy -= 1;
          conn.usedAt = now();
        }
      } catch (e) {
        if (l.gen !== gen) return;
        delete l.inflight;
        const endedAt = now();
        write(threadId, run, l, { state: "failed", why: clean(threadId, spec, messageOf(e)), exit: null, startedAt, endedAt, ms: endedAt - startedAt, runs });
      }
    })();
    void track(threadId, work);
    return { outcome: "running", record };
  };

  const stop = (threadId: string, run: string, l: Live, why: string | undefined): void => {
    const was = l.record.state;
    l.gen += 1;
    if (l.inflight !== undefined) {
      l.inflight.conn.client.notify("notifications/cancelled", { requestId: l.inflight.id, reason: why ?? "cancelled" });
      delete l.inflight;
    }
    l.reshaping?.kill();
    delete l.reshaping;
    delete l.held;
    if (why === undefined || (was !== "running" && was !== "held")) return;
    write(threadId, run, l, { state: "cancelled", why, exit: null, runs: l.record.runs, endedAt: now() });
  };

  const closeConn = (t: ThreadMcp, server: string): void => {
    const c = t.conns.get(server);
    t.conns.delete(server);
    void c?.then(conn => conn.client.close(), () => {});
  };

  const sweep = setInterval(() => {
    const at = now();
    for (const t of threads.values()) {
      for (const [server, c] of t.conns) {
        void c.then(
          conn => {
            if (conn.busy === 0 && !t.kept.has(server) && !t.shown && at - conn.usedAt >= idleMs && t.conns.get(server) === c) closeConn(t, server);
          },
          () => {},
        );
      }
    }
  }, Math.min(SWEEP_MS, idleMs));
  sweep.unref();

  const heldOn = (threadId: string, key: string): [string, Live][] =>
    [...(threads.get(threadId)?.runs ?? [])].filter(([, l]) => l.held?.ask === "code" && l.held.key === key);

  return {
    provisional(threadId, decl, last, args) {
      const runs = last?.runs ?? 0;
      if (asksNow(threadId, decl, listed(threadId, decl), args) === true) return { state: "held", why: HELD_APPROVAL, runs };
      return runningRecord(last === undefined ? undefined : lastResult(last), runs + 1, now());
    },

    start(asked) {
      // Read once, so the arguments the person allows are the ones the call sends.
      const sent = asked.args();
      const req: McpRunStart = { ...asked, args: () => sent };
      const l = live(req.threadId, req.run, req.last);
      const beside = l.record.state === "running";
      if (beside && req.decl.kind === "tool" && req.decl.once === true) return { outcome: "noop", record: l.record };
      delete l.held;
      // The person's starts have a budget of their own, as a command's do.
      const person = req.by === "person";
      const times = spent(person ? l.pressed : l.starts, now());
      if (times === undefined) return hold(req, l, person ? "pressed" : "budget", l.record.runs, beside);
      if (person) l.pressed = times;
      else l.starts = times;
      // Beside a call still running, the ask comes before that call stops.
      if (asksNow(req.threadId, req.decl, listed(req.threadId, req.decl), sent) === true) {
        const asked = hold(req, l, "code", l.record.runs, beside);
        if (beside) deps.onAsks?.(req.threadId);
        return asked;
      }
      return launch(req, l, false);
    },

    approve(threadId, key, scope) {
      if (scope === "thread") deps.approvals.allow(threadId, key);
      for (const [, l] of heldOn(threadId, key)) {
        const req = l.held!.req;
        delete l.held;
        launch(req, l, true);
      }
    },

    deny(threadId, key) {
      // Turning down a new start leaves the call still running to finish.
      for (const [run, l] of heldOn(threadId, key)) {
        if (l.record.state === "running") delete l.held;
        else stop(threadId, run, l, "you said not to run it");
      }
    },

    owns(_threadId, key) {
      return key.startsWith("mcp:");
    },

    held(threadId) {
      const t = threads.get(threadId);
      if (t === undefined) return [];
      return [...t.runs].flatMap(([run, l]) => (l.held?.ask === "code" && l.held.key !== undefined ? [askOf(threadId, run, l.held.req, l.held.key)] : []));
    },

    named(threadId, key) {
      const asked = this.held(threadId).find(a => a.key === key);
      return asked?.kind === "tool" ? { run: asked.run, cmd: `${asked.server}.${asked.tool} ${JSON.stringify(asked.args)}`.slice(0, 120) } : undefined;
    },

    async pending(threadId, ms = CONNECT_MS) {
      const t = threads.get(threadId);
      if (t === undefined) return;
      const deadline = new Promise<void>(r => setTimeout(r, ms).unref());
      for (let i = 0; i < 4 && t.pending.size > 0; i++) await Promise.race([Promise.allSettled([...t.pending]), deadline]);
    },

    cancel(threadId, run) {
      const l = threads.get(threadId)?.runs.get(run);
      if (l !== undefined) stop(threadId, run, l, "cancelled");
    },

    stopAll(threadId, opts) {
      for (const [run, l] of threads.get(threadId)?.runs ?? []) stop(threadId, run, l, opts?.quiet === true ? undefined : (opts?.why ?? "cancelled"));
    },

    servers(threadId, names, kept) {
      const t = thread(threadId);
      t.named = new Set(names);
      t.kept = new Set(kept);
      for (const server of [...t.conns.keys()]) if (!t.named.has(server)) closeConn(t, server);
    },

    shown(threadId, on) {
      const t = thread(threadId);
      // Hidden starts the idle clock from now, not from the last call.
      if (t.shown && !on) for (const c of t.conns.values()) void c.then(conn => (conn.usedAt = now()), () => {});
      t.shown = on;
    },

    closeServers(threadId) {
      const t = threads.get(threadId);
      if (t !== undefined) for (const server of [...t.conns.keys()]) closeConn(t, server);
    },

    async catalog(threadId, server) {
      const conn = await connect(threadId, server);
      const tools = await toolsOf(threadId, server, conn);
      let resources: McpResourceInfo[] = [];
      if (conn.resources) {
        try {
          const got = (await conn.client.request("resources/list", {}, CONNECT_MS).answer) as { resources?: McpResourceInfo[] } | undefined;
          resources = (got?.resources ?? []).filter(r => typeof r?.uri === "string");
        } catch {
          resources = [];
        }
      }
      conn.usedAt = now();
      const head = [
        `${server}: an MCP server of this thread's agent, ${tools.length} tool${tools.length === 1 ? "" : "s"}${resources.length > 0 ? `, ${resources.length} resource${resources.length === 1 ? "" : "s"}` : ""}.`,
        ASKS_LINE[deps.asks(threadId)],
        `A run on a tool not marked read-only takes description="what it does, in one plain line", which the person reads before allowing it; a write without one is refused.`,
        `A run calls a tool: <run name="x" tool="${server}.<tool>" args={{ <property>: <expression> }} every={60} />; a resource: <run name="x" resource="${server}:<uri>" />. $x.json holds the structured result (else the text parsed as JSON), $x.out the text, $x.err the error.`,
        "Each tool says whether it returns data (a structured result fills $x.json) or text only (prose for a model: draw $x.out, or reshape it with then=).",
      ];
      const body = tools.map(tool => {
        const hints = hintsOf(tool, seen.get(seenKey(threadId, server, tool.name)));
        const about = sentence(tool.description, 240);
        const props = foldSchema(tool.inputSchema);
        return [`${tool.name}${hints.length > 0 ? ` [${hints.join(", ")}]` : ""}${about !== undefined ? `: ${about}` : ""}`, ...(props.length > 0 ? props : ["  (no arguments)"])].join("\n");
      });
      const res = resources.slice(0, 20).map(r => `resource ${r.uri}${r.mimeType !== undefined ? ` (${r.mimeType})` : ""}${sentence(r.description) !== undefined ? `: ${sentence(r.description)}` : ""}`);
      return clean(threadId, conn.spec, [...head, "", ...body, ...(res.length > 0 ? ["", ...res] : [])].join("\n"));
    },

    async refusedRuns(threadId, before, doc, away) {
      if (away.length > 0 || !doc) return away;
      const bare = Object.entries(doc.runs).flatMap(([name, d]) =>
        d.kind === "tool" && d.description === undefined && stableJson(before?.runs[name] ?? null) !== stableJson(d) ? [{ name, server: d.server, tool: d.tool }] : [],
      );
      if (bare.length === 0) return [];
      const deadline = (): Promise<McpToolInfo[]> => new Promise(r => setTimeout(() => r([]), DESCRIBE_MS).unref());
      const servers = [...new Set(bare.map(b => b.server))];
      const tools = new Map(await Promise.all(servers.map(async server => [server, lists.get(listKey(threadId, server)) ?? (await Promise.race([listFor(threadId, server), deadline()]))] as const)));
      return bare.filter(b => tools.get(b.server)?.find(t => t.name === b.tool)?.annotations?.readOnlyHint !== true).map(b => slateProblem("K713", slateDescriptionMissing(`$${b.name}`, b.server, b.tool)));
    },

    drop(threadId, keep = false) {
      const t = threads.get(threadId);
      if (t === undefined) return;
      if (!keep) for (const [run, l] of t.runs) stop(threadId, run, l, undefined);
      if (!keep) for (const server of [...t.conns.keys()]) closeConn(t, server);
      threads.delete(threadId);
      for (const k of [...lists.keys()]) if (k.startsWith(`${threadId}\u0000`)) lists.delete(k);
      for (const k of [...seen.keys()]) if (k.startsWith(`${threadId}\u0000`)) seen.delete(k);
    },

    close() {
      clearInterval(sweep);
      for (const threadId of [...threads.keys()]) this.drop(threadId);
    },
  };
}
