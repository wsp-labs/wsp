// SPDX-License-Identifier: AGPL-3.0-only
// Codex's two roads through the host. The ChatGPT road carries every /backend-api/ path to chatgpt.com, or to the
// https origin chatgpt.com names for the sign-in's workspace, with the host's sign-in, and answers Codex's refresh
// and revoke itself so no refresh token ever leaves this computer. The OpenAI road carries /v1/ to api.openai.com, or
// to the person's own provider, with the key. Neither ever answers 401: Codex treats one on its refresh as a
// permanent sign-out and one on connectors as an OAuth server (codex login/src/auth/manager.rs,
// codex-mcp/src/mcp/mod.rs).
import type { IncomingMessage, ServerResponse } from "node:http";
import type { Duplex } from "node:stream";
import { outgoingHeaders, passHttp, passUpgrade, responseOn, type Sentences, type Target } from "./forward.js";
import { CHATGPT_ORIGIN, chatgptSignIn } from "./models.js";
import { codexPlaceholders, turnOfRefreshToken } from "./placeholders.js";
import type { BrokerContext, BrokerRequest, BrokerService, ChatgptCredential, Door, OpenaiCredential } from "./types.js";

const OPENAI_BASE = "https://api.openai.com/v1";
const BACKEND = "/backend-api/";
const V1 = "/v1/";
const TOKEN_PATH = "/oauth/token";
const REVOKE_PATH = "/oauth/revoke";
const ACCOUNTS_CHECK = "/backend-api/wham/accounts/check";
const FORM_MAX = 16 * 1024;

export const CHATGPT_WORDS = {
  signedOut: 'wsp: the Codex sign-in on your computer could not be read. Sign in to Codex there, and if it keeps the sign-in in the Keychain, set cli_auth_credentials_store = "file" in its config.toml and sign in again. Then send this again.',
  refused: "wsp: ChatGPT refused the Codex sign-in on your computer. Sign in to Codex there once more, then send this again.",
  unreachable: "wsp: your computer could not reach ChatGPT. Codex will try again.",
  notReady: "wsp: the Codex sign-in on your computer is not ready yet. Codex will try again.",
  notOurs: "wsp: this sign-in was not made by wsp for this thread. Start a new thread.",
} as const;

export const OPENAI_WORDS = {
  noKey: "wsp: there is no OpenAI key on your computer. Add one in Settings, Agents, then send this again.",
  refused: "wsp: the provider refused the OpenAI key on your computer. Replace it in Settings, Agents, then send this again.",
  unreachable: "wsp: your computer could not reach the OpenAI provider. Codex will try again.",
} as const;

// The path as the upstream will read it, dots resolved, so a claim and the forward agree on one path.
function doorUrl(req: IncomingMessage): URL | undefined {
  const raw = req.url ?? "";
  if (!raw.startsWith("/")) return undefined;
  return new URL(raw, "http://door");
}

const pathOf = (req: IncomingMessage) => doorUrl(req)?.pathname ?? "";

const CHATGPT_OWNED = ["chatgpt-account-id", "x-openai-fedramp"];
const OPENAI_OWNED = ["openai-organization", "openai-project"];

function chatgptHeaders(req: IncomingMessage, cred: ChatgptCredential, upgrade: boolean, extra: Record<string, string> = {}): string[] {
  return outgoingHeaders(req, CHATGPT_OWNED, { ...extra, ...chatgptSignIn(cred) }, upgrade);
}

function openaiHeaders(req: IncomingMessage, cred: OpenaiCredential, upgrade: boolean): string[] {
  const added: Record<string, string> = { Authorization: `Bearer ${cred.key}` };
  if (cred.organization) added["OpenAI-Organization"] = cred.organization;
  if (cred.project) added["OpenAI-Project"] = cred.project;
  return outgoingHeaders(req, OPENAI_OWNED, added, upgrade);
}

const refuser = (ctx: BrokerContext) => (res: ServerResponse, status: 403 | 503, said: string) => ctx.refuse(res, status, said, "openai");

function readForm(req: IncomingMessage): Promise<Record<string, unknown> | undefined> {
  return new Promise((resolve) => {
    const chunks: Buffer[] = [];
    let size = 0;
    req.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size <= FORM_MAX) chunks.push(chunk);
    });
    req.on("error", () => resolve(undefined));
    req.on("end", () => {
      if (size > FORM_MAX) return resolve(undefined);
      const text = Buffer.concat(chunks).toString("utf8");
      try {
        const parsed: unknown = JSON.parse(text);
        resolve(parsed !== null && typeof parsed === "object" ? (parsed as Record<string, unknown>) : undefined);
      } catch {
        resolve(Object.fromEntries(new URLSearchParams(text)));
      }
    });
  });
}

function answerJson(res: ServerResponse, status: number, body: object): void {
  const text = JSON.stringify(body);
  res.writeHead(status, { "content-type": "application/json", "content-length": Buffer.byteLength(text), "cache-control": "no-store" });
  res.end(text);
}

async function refresh(r: BrokerRequest, ctx: BrokerContext, now: () => number): Promise<void> {
  if (r.req.method !== "POST") {
    r.req.resume();
    return answerJson(r.res, 405, { error: "method_not_allowed" });
  }
  const form = await readForm(r.req);
  const turn = turnOfRefreshToken(form?.refresh_token);
  if (form?.grant_type !== "refresh_token" || !turn) return ctx.refuse(r.res, 403, CHATGPT_WORDS.notOurs, "openai");
  const cred = await ctx.credential("chatgpt");
  if (!cred) return ctx.refuse(r.res, 503, CHATGPT_WORDS.notReady, "openai");
  answerJson(r.res, 200, codexPlaceholders({ accountId: cred.accountId, planType: cred.planType, userId: cred.userId, fedramp: cred.fedramp }, turn, now()));
}

function revoke(r: BrokerRequest): Promise<void> {
  return new Promise((resolve) => {
    r.req.on("end", () => {
      answerJson(r.res, 200, {});
      resolve();
    });
    r.req.on("error", () => resolve());
    r.req.resume();
  });
}

const CHATGPT_SAID: Sentences = { refused: CHATGPT_WORDS.refused, unreachable: CHATGPT_WORDS.unreachable };
const OPENAI_SAID: Sentences = { refused: OPENAI_WORDS.refused, unreachable: OPENAI_WORDS.unreachable };

function httpsOrigin(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  try {
    const url = new URL(value);
    return url.protocol === "https:" && !url.username && !url.password ? url.origin : undefined;
  } catch {
    return undefined;
  }
}

// Codex 0.162 refuses a workspace origin that is not https, and moves its turns to that origin when it shares one
// with the model provider, so every account answers with an https origin and the host, not the machine, goes there.
function rewriteAccounts(body: Buffer, accountId: string): { body: Buffer; origin?: string } | undefined {
  let parsed: { accounts?: unknown };
  try {
    parsed = JSON.parse(body.toString("utf8")) as { accounts?: unknown };
  } catch {
    return undefined;
  }
  if (!Array.isArray(parsed?.accounts)) return undefined;
  let origin: string | undefined;
  for (const account of parsed.accounts as Record<string, unknown>[]) {
    if (account === null || typeof account !== "object") continue;
    account.workspace_backend_origin = httpsOrigin(account.workspace_backend_origin) ?? CHATGPT_ORIGIN;
    if (account.id === accountId) origin = account.workspace_backend_origin as string;
  }
  return { body: Buffer.from(JSON.stringify(parsed)), origin };
}

function openaiTarget(req: IncomingMessage, cred: OpenaiCredential): Target {
  const at = doorUrl(req)!;
  const base = (cred.baseUrl ?? OPENAI_BASE).replace(/\/+$/, "");
  return { url: new URL(`${base}${at.pathname.slice(V1.length - 1)}${at.search}`) };
}

// dial: the address a public origin is reached at; tests point it at a stand-in.
export function chatgptService(opts: { dial?: (url: URL) => URL; now?: () => number } = {}): BrokerService {
  const now = opts.now ?? Date.now;
  const origins = new Map<string, string>();
  const target = (req: IncomingMessage, origin: string): Target => {
    const at = doorUrl(req)!;
    const url = new URL(`${origin}${at.pathname}${at.search}`);
    return { url, via: opts.dial?.(url) };
  };
  const routed = (req: IncomingMessage, cred: ChatgptCredential) => target(req, origins.get(cred.accountId) ?? CHATGPT_ORIGIN);
  return {
    id: "chatgpt",
    claims(req: IncomingMessage, door: Door, agent) {
      if (door !== "http" || agent !== "codex") return false;
      const path = pathOf(req);
      return path.startsWith(BACKEND) || path === TOKEN_PATH || path === REVOKE_PATH;
    },
    async serve(r, ctx) {
      const path = pathOf(r.req);
      if (path === TOKEN_PATH) return refresh(r, ctx, now);
      if (path === REVOKE_PATH) return revoke(r);
      const cred = await ctx.credential("chatgpt");
      if (!cred) return ctx.refuse(r.res, 403, CHATGPT_WORDS.signedOut, "openai");
      if (path !== ACCOUNTS_CHECK || r.req.method !== "GET") {
        return passHttp(r.req, r.res, routed(r.req, cred), chatgptHeaders(r.req, cred, false), refuser(ctx), CHATGPT_SAID);
      }
      const headers = chatgptHeaders(r.req, cred, false, { "Accept-Encoding": "identity" });
      await passHttp(r.req, r.res, target(r.req, CHATGPT_ORIGIN), headers, refuser(ctx), CHATGPT_SAID, (body) => {
        const answer = rewriteAccounts(body, cred.accountId);
        if (answer?.origin) origins.set(cred.accountId, answer.origin);
        return answer?.body;
      });
    },
    async upgrade(r: BrokerRequest, socket: Duplex, head: Buffer, ctx: BrokerContext) {
      if (!pathOf(r.req).startsWith(BACKEND)) return void responseOn(r.req, socket).writeHead(404, { "content-length": "0" }).end();
      const cred = await ctx.credential("chatgpt");
      if (!cred) return ctx.refuse(responseOn(r.req, socket), 403, CHATGPT_WORDS.signedOut, "openai");
      await passUpgrade(r.req, socket, head, routed(r.req, cred), chatgptHeaders(r.req, cred, true), refuser(ctx), CHATGPT_SAID);
    },
  };
}

export function openaiService(): BrokerService {
  return {
    id: "openai",
    claims(req: IncomingMessage, door: Door, agent) {
      return door === "http" && agent === "codex" && pathOf(req).startsWith(V1);
    },
    async serve(r, ctx) {
      const cred = await ctx.credential("openai");
      if (!cred) return ctx.refuse(r.res, 403, OPENAI_WORDS.noKey, "openai");
      await passHttp(r.req, r.res, openaiTarget(r.req, cred), openaiHeaders(r.req, cred, false), refuser(ctx), OPENAI_SAID);
    },
    async upgrade(r, socket, head, ctx) {
      const cred = await ctx.credential("openai");
      if (!cred) return ctx.refuse(responseOn(r.req, socket), 403, OPENAI_WORDS.noKey, "openai");
      await passUpgrade(r.req, socket, head, openaiTarget(r.req, cred), openaiHeaders(r.req, cred, true), refuser(ctx), OPENAI_SAID);
    },
  };
}
