// SPDX-License-Identifier: AGPL-3.0-only

import { createHash } from "node:crypto";
import { request as httpRequest, type IncomingMessage, type ServerResponse } from "node:http";
import { request as httpsRequest } from "node:https";
import { pipeline } from "node:stream";
import { StringDecoder } from "node:string_decoder";
import { anthropicRetryWhy } from "@wsp/protocol";
import { computerNameHere } from "../places/this-computer.js";

/** The person's Claude sign-in as the broker hands it over for one request: a setup-token or a claude.ai login's
 * access token goes as Bearer, a key as x-api-key, an apiKeyHelper's answer as both, as Claude Code itself sends it,
 * and a gateway's token as Bearer to the gateway's own URL with its headers. */
export type AnthropicCredential =
  | { kind: "token"; token: string; expiresAt?: number }
  | { kind: "key"; key: string }
  | { kind: "helper"; value: string }
  | { kind: "gateway"; url: string; token: string; headers?: Readonly<Record<string, string>>; invalidHeader?: string };

/** The parts of the broker's request and context this service reads, written here so it imports no broker. */
export interface AnthropicRequest {
  req: IncomingMessage;
  res: ServerResponse;
  grant: { workspaceId: string; threadId?: string };
}
export interface AnthropicContext {
  credential(kind: "anthropic"): Promise<AnthropicCredential | undefined>;
  refuse(res: ServerResponse, status: 403, said: string, shape: "anthropic"): void;
  retrying(retry: { attempt: number; why: string }): void;
}

const ALLOWED = new Set(["POST /v1/messages", "POST /v1/messages/count_tokens", "GET /v1/models", "HEAD /api/hello"]);
const callOf = (req: IncomingMessage): string => `${req.method} ${(req.url ?? "/").split("?")[0]}`;
const DROPPED = new Set(["authorization", "x-api-key", "cookie", "proxy-authorization", "host"]);
// Hop-by-hop headers describe one connection; the client's and the upstream's are not the same connection.
const HOP = new Set(["connection", "keep-alive", "proxy-connection", "te", "trailer", "upgrade"]);

function credentialHeaders(credential: AnthropicCredential): string[] {
  switch (credential.kind) {
    case "token":
      return ["Authorization", `Bearer ${credential.token}`];
    case "key":
      return ["x-api-key", credential.key];
    case "helper":
      return ["x-api-key", credential.value, "Authorization", `Bearer ${credential.value}`];
    case "gateway":
      // A gateway whose key rides in its custom headers has an empty token, and goes with no Authorization.
      return [...(credential.token === "" ? [] : ["Authorization", `Bearer ${credential.token}`]), ...Object.entries(credential.headers ?? {}).flat()];
  }
}

/** The client's headers in their order and case with the credential put where the client's own was, so nothing but
 * the credential and the Host differ from what Claude Code sent. */
function upstreamHeaders(raw: readonly string[], credential: AnthropicCredential, host: string): string[] {
  const added = credentialHeaders(credential);
  const replaced = new Set(added.filter((_, i) => i % 2 === 0).map(name => name.toLowerCase()));
  const out: string[] = [];
  let placed = false;
  for (let i = 0; i < raw.length; i += 2) {
    const name = raw[i]!.toLowerCase();
    if (name === "host") out.push(raw[i]!, host);
    if (DROPPED.has(name) || replaced.has(name) || HOP.has(name)) {
      if (!placed && (name === "authorization" || name === "x-api-key")) out.push(...added);
      placed ||= name === "authorization" || name === "x-api-key";
      continue;
    }
    out.push(raw[i]!, raw[i + 1]!);
  }
  return placed ? out : [...out, ...added];
}

function withoutHop(raw: readonly string[]): string[] {
  const out: string[] = [];
  for (let i = 0; i < raw.length; i += 2) if (!HOP.has(raw[i]!.toLowerCase())) out.push(raw[i]!, raw[i + 1]!);
  return out;
}

const sentences = (computer: string) => ({
  missing: `wsp: Claude is not signed in on ${computer}. Sign it in from Settings, Agents.`,
  expired: `wsp: Claude's sign-in on ${computer} has expired. Open Claude Code there to renew it, or sign in again from Settings, Agents.`,
  token: `wsp: Claude's token on ${computer} has expired. Make a new one in Settings, Agents.`,
  key: `wsp: Anthropic refused the API key on ${computer}. Set a working key in Settings, Agents.`,
  helper: `wsp: Anthropic refused the key your apiKeyHelper gave on ${computer}. Check the helper, then Settings, Agents.`,
  gateway: `wsp: your gateway refused the token on ${computer}. Check it in Settings, Agents.`,
  header: (header: string) => `wsp: ${header} in ANTHROPIC_CUSTOM_HEADERS on ${computer} has a name HTTP does not allow, so your gateway was not called. Fix that line there, which Claude Code refuses too.`,
});

/** The wait for an upstream's reply to start, Claude Code's own first-byte rule, which it keeps off a base URL such as
 * the door: 30 s plus a second per 32 KB of the request, since the API sends no header before the first token. Cut
 * there, the call reads to Claude Code as one Anthropic never answered, and it says it is trying again; its retry
 * inside a minute, the same bytes from the same thread, waits as long as Claude Code does, as its own retry does. A
 * gateway's call is never cut: a gateway may hold a reply until it is whole. */
const FIRST_BYTE_MS = 30_000;
const PER_32KB_MS = 1_000;
const RETRY_WITHIN_MS = 60_000;

/** Whether a call asks for its reply as a stream: a reply that is not streamed starts only once it is whole, so no
 * first-byte wait, Claude Code's own included, covers it. A body that does not parse keeps the wait. */
function streams(body: Buffer): boolean {
  try {
    return (JSON.parse(body.toString()) as { stream?: unknown }).stream === true;
  } catch {
    return true;
  }
}

/** Whether Claude Code makes a call again after this failed answer, its `tries`th, by its own rule, not its SDK's
 * (2.1.301): the API's x-should-retry, else a 408, 409 or any 5xx. A 429 it retries by the sign-in and the account's limit
 * headers, and never for a claude.ai login at its limit, so a 429 is left to the agent's own line; off a claude.ai login
 * it gives up at the third 529 in a row on Opus, Fable and Mythos; and it retries an overloaded_error body under
 * x-should-retry false, a body this reads none of. */
function retried(up: IncomingMessage, kind: AnthropicCredential["kind"], tries: number): boolean {
  const status = up.statusCode ?? 0;
  if (status < 400 || status === 401 || status === 429 || (status === 529 && kind !== "token" && tries >= 3)) return false;
  const said = up.headers["x-should-retry"];
  return said === "true" || (said !== "false" && (status === 408 || status === 409 || status >= 500));
}

/** The error events inside a 200 stream after which Claude Code makes its call again with no api_retry line, by the
 * category its own line would give (2.1.301): an overload again as a stream twice, then whole; a server or timeout
 * error whole at once; after a finished text block, a new call to resume, at most three, the fourth break ending it.
 * Under CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1 the third overloaded stream ends it. */
const STREAM_ERRORS = new Map([
  ["overloaded_error", "overloaded"],
  ["api_error", "server_error"],
  ["timeout_error", "server_error"],
]);

/** Calls `failed` with the category of an error event inside a 200 stream. An unfinished line keeps its last 4 KB: an
 * error's is short. */
function streamError(failed: (category: string) => void): (chunk: Buffer) => void {
  const text = new StringDecoder("utf8");
  let rest = "";
  let event = "";
  return chunk => {
    const lines = (rest + text.write(chunk)).split("\n");
    rest = lines.pop()!.slice(-4_096);
    for (const raw of lines) {
      const line = raw.endsWith("\r") ? raw.slice(0, -1) : raw;
      if (line === "") event = "";
      else if (line.startsWith("event:")) event = line.slice(6).trim();
      else if (event === "error" && line.startsWith("data:")) {
        try {
          const category = STREAM_ERRORS.get(String((JSON.parse(line.slice(5)) as { error?: { type?: unknown } }).error?.type));
          if (category !== undefined) failed(category);
        } catch {}
      }
    }
  };
}

const categoryOf = (status: number): string | undefined => (status === 529 ? "overloaded" : status >= 500 ? "server_error" : undefined);

/** How many times Claude Code makes a failed call again before it gives up, unless its launch sets
 * CLAUDE_CODE_MAX_RETRIES (2.1.301): a failure past it is never tried again. */
const CLI_RETRIES = 10;

/** 403 with x-should-retry false, which Claude Code obeys at once, printing the sentence. */
function stop(res: ServerResponse, ctx: AnthropicContext, said: string): void {
  res.setHeader("x-should-retry", "false");
  ctx.refuse(res, 403, said, "anthropic");
}

/** Claude Code's model calls from a brokered machine: the four paths it needs, to a fixed upstream, with
 * the person's credential swapped in and every other byte passed both ways as it streams. Never answers 401, which
 * ends the turn with "Please run /login" on a machine that has nothing to log in with. */
export function anthropicService(options: { upstream?: string; computer?: () => string; firstByteMs?: number } = {}) {
  const upstream = options.upstream ?? "https://api.anthropic.com";
  const computer = options.computer ?? computerNameHere;
  const firstByteMs = options.firstByteMs ?? FIRST_BYTE_MS;
  /** Each call that failed in the last minute and was not made again yet, by its thread and its body, which its retry
   * repeats byte for byte: how many times, and whether it was cut at the wait, which spares the retry the wait once.
   * One entry per failure, since subagents given one prompt send one body and each retries its own. */
  const failed = new Map<string, Array<{ at: number; tries: number; cut: boolean }>>();
  const takeFailed = (call: string): { at: number; tries: number; cut: boolean } | undefined => {
    const now = Date.now();
    for (const [key, list] of failed) {
      const kept = list.filter(f => now - f.at <= RETRY_WITHIN_MS);
      if (kept.length === 0) failed.delete(key);
      else failed.set(key, kept);
    }
    const taken = failed.get(call)?.shift();
    if (failed.get(call)?.length === 0) failed.delete(call);
    return taken;
  };
  return {
    id: "anthropic" as const,
    // git reaches the same door as its proxy, with absolute-form requests (http://github.com/...), which match no call.
    claims(req: IncomingMessage, door: "http" | "gh", agent: "claude" | "codex" | undefined): boolean {
      return door === "http" && ALLOWED.has(callOf(req)) && (agent === "claude" || (agent === undefined && req.headers["anthropic-version"] !== undefined));
    },
    async serve({ req, res, grant }: AnthropicRequest, ctx: AnthropicContext): Promise<void> {
      if (!ALLOWED.has(callOf(req))) {
        req.resume();
        res.writeHead(404, { "content-type": "application/json", "x-should-retry": "false" });
        res.end(JSON.stringify({ type: "error", error: { type: "not_found_error", message: `wsp: ${callOf(req)} is not a model call a cloud thread makes.` } }));
        return;
      }
      const said = sentences(computer());
      const credential = await ctx.credential("anthropic");
      if (credential === undefined || (credential.kind === "token" && credential.expiresAt !== undefined && credential.expiresAt <= Date.now())) {
        req.resume();
        stop(res, ctx, credential === undefined ? said.missing : said.expired);
        return;
      }
      if (credential.kind === "gateway" && credential.invalidHeader !== undefined) {
        req.resume();
        stop(res, ctx, said.header(credential.invalidHeader));
        return;
      }
      const target = new URL(`${(credential.kind === "gateway" ? credential.url : upstream).replace(/\/+$/, "")}${req.url ?? "/"}`);
      const send = target.protocol === "https:" ? httpsRequest : httpRequest;
      const wait = credential.kind === "gateway" ? undefined : firstByteMs + Math.floor(Number(req.headers["content-length"] ?? 0) / 32_768) * PER_32KB_MS;
      await new Promise<void>(done => {
        let call: string | undefined;
        let before: { tries: number; cut: boolean } | undefined;
        let left = false;
        let told = false;
        /** The call failed and Claude Code makes it again: the thread is told, since for a background subagent's call
         * the CLI prints nothing, and for every other it prints its own line, which the thread keeps in place of this. */
        const again = (why: string, cut: boolean): void => {
          if (told || left) return;
          told = true;
          const tries = (before?.tries ?? 0) + 1;
          if (call !== undefined) failed.set(call, [...(failed.get(call) ?? []), { at: Date.now(), tries, cut }]);
          if (tries <= CLI_RETRIES) ctx.retrying({ attempt: tries, why });
        };
        const out = send(target, { method: req.method, headers: upstreamHeaders(req.rawHeaders, credential, target.host), setHost: false }, up => {
          clearTimeout(timer);
          if (retried(up, credential.kind, (before?.tries ?? 0) + 1)) again(anthropicRetryWhy(categoryOf(up.statusCode ?? 0), up.statusCode), false);
          if (up.statusCode === 401) {
            up.resume();
            stop(res, ctx, credential.kind === "token" && credential.expiresAt !== undefined ? said.expired : said[credential.kind]);
            done();
            return;
          }
          res.writeHead(up.statusCode ?? 502, up.statusMessage, withoutHop(up.rawHeaders));
          res.flushHeaders();
          if (up.statusCode === 200 && String(up.headers["content-type"]).startsWith("text/event-stream")) up.on("data", streamError(category => again(anthropicRetryWhy(category, undefined), false)));
          pipeline(up, res, () => done());
        });
        const timer = wait === undefined ? undefined : setTimeout(() => {
          again(anthropicRetryWhy(undefined, undefined), true);
          out.destroy(new Error("no reply started"));
        }, wait);
        const body: Buffer[] = [];
        req.on("data", (chunk: Buffer) => body.push(chunk));
        req.on("end", () => {
          const whole = Buffer.concat(body);
          body.length = 0;
          call = `${grant.workspaceId} ${grant.threadId ?? ""} ${createHash("sha256").update(whole).digest("hex")}`;
          before = takeFailed(call);
          if (before?.cut !== true && (timer === undefined || streams(whole))) return;
          clearTimeout(timer);
        });
        // Claude Code reads any status as an answer, a 503 overloaded_error as overloaded; only a cut reads as Anthropic not answering.
        out.on("error", () => {
          clearTimeout(timer);
          if (!res.headersSent) again(anthropicRetryWhy(undefined, undefined), false);
          if (!res.writableEnded) res.destroy();
          done();
        });
        res.on("close", () => {
          clearTimeout(timer);
          if (res.writableFinished) return;
          left = true;
          out.destroy();
        });
        pipeline(req, out, () => {});
      });
    },
  };
}
