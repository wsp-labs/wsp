// SPDX-License-Identifier: AGPL-3.0-only

import { createHash } from "node:crypto";
import { request as httpRequest, type IncomingMessage, type ServerResponse } from "node:http";
import { request as httpsRequest } from "node:https";
import { pipeline } from "node:stream";
import { computerNameHere } from "../places/this-computer.js";

/** The person's Claude sign-in as the broker hands it over for one request: a setup-token or a claude.ai login's
 * access token goes as Bearer, a key as x-api-key, an apiKeyHelper's answer as both, as Claude Code itself sends it,
 * and a gateway's token as Bearer to the gateway's own URL with its headers. */
export type AnthropicCredential =
  | { kind: "token"; token: string; expiresAt?: number }
  | { kind: "key"; key: string }
  | { kind: "helper"; value: string }
  | { kind: "gateway"; url: string; token: string; headers?: Readonly<Record<string, string>> };

/** The parts of the broker's request and context this service reads, written here so it imports no broker. */
export interface AnthropicRequest {
  req: IncomingMessage;
  res: ServerResponse;
  grant: { workspaceId: string; threadId?: string };
}
export interface AnthropicContext {
  credential(kind: "anthropic"): Promise<AnthropicCredential | undefined>;
  refuse(res: ServerResponse, status: 403, said: string, shape: "anthropic"): void;
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
  /** When each call was cut at the wait, by its thread and its body, kept a minute. */
  const cut = new Map<string, number>();
  const retried = (call: string): boolean => {
    const now = Date.now();
    for (const [key, at] of cut) if (now - at > RETRY_WITHIN_MS) cut.delete(key);
    return cut.delete(call);
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
      const target = new URL(`${(credential.kind === "gateway" ? credential.url : upstream).replace(/\/+$/, "")}${req.url ?? "/"}`);
      const send = target.protocol === "https:" ? httpsRequest : httpRequest;
      const wait = credential.kind === "gateway" ? undefined : firstByteMs + Math.floor(Number(req.headers["content-length"] ?? 0) / 32_768) * PER_32KB_MS;
      await new Promise<void>(done => {
        let call: string | undefined;
        const out = send(target, { method: req.method, headers: upstreamHeaders(req.rawHeaders, credential, target.host), setHost: false }, up => {
          clearTimeout(timer);
          if (up.statusCode === 401) {
            up.resume();
            stop(res, ctx, credential.kind === "token" && credential.expiresAt !== undefined ? said.expired : said[credential.kind]);
            done();
            return;
          }
          res.writeHead(up.statusCode ?? 502, up.statusMessage, withoutHop(up.rawHeaders));
          res.flushHeaders();
          pipeline(up, res, () => done());
        });
        const timer = wait === undefined ? undefined : setTimeout(() => {
          if (call !== undefined) cut.set(call, Date.now());
          out.destroy(new Error("no reply started"));
        }, wait);
        if (timer !== undefined) {
          const body: Buffer[] = [];
          req.on("data", (chunk: Buffer) => body.push(chunk));
          req.on("end", () => {
            const whole = Buffer.concat(body);
            body.length = 0;
            call = `${grant.workspaceId} ${grant.threadId ?? ""} ${createHash("sha256").update(whole).digest("hex")}`;
            if (retried(call) || !streams(whole)) clearTimeout(timer);
          });
        }
        // Claude Code reads any status as an answer, a 503 overloaded_error as overloaded; only a cut reads as Anthropic not answering.
        out.on("error", () => {
          clearTimeout(timer);
          if (!res.writableEnded) res.destroy();
          done();
        });
        res.on("close", () => {
          clearTimeout(timer);
          if (!res.writableFinished) out.destroy();
        });
        pipeline(req, out, () => {});
      });
    },
  };
}
