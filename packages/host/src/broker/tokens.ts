// SPDX-License-Identifier: AGPL-3.0-only
// Where each client on a brokered machine carries the token it was handed, read
// per door. The machine holds placeholders of the real credential's kind, so the
// token sits wherever that kind would: Claude Code's key or bearer, Codex's
// placeholder JWT or its sk-wsp- key, git's proxy password, gh's `token`.
import type { IncomingMessage } from "node:http";
import type { PipeDoor } from "@wsp/protocol";

/** The claim in Codex's placeholder access token that holds the turn token. */
export const TURN_CLAIM = "https://wsp.dev/turn";
/** What Codex's placeholder OpenAI key starts with in apikey mode. */
export const KEY_PREFIX = "sk-wsp-";
/** Connector calls arrive with no Authorization at all; the one Codex turn on the workspace is whose they are. */
export const CONNECTOR_PATH = "/backend-api/ps/mcp";

/** Codex's refresh carries no Authorization: the turn token rides the body's refresh token, behind this prefix. */
export const REFRESH_PATH = "/oauth/token";
export const REFRESH_PREFIX = "rt_wsp_";
/** The most of a refresh body the broker reads for its token; a larger one is no refresh Codex sends. */
export const REFRESH_BODY_MAX = 16 * 1024;

/** The headers a caller may carry a credential in, dropped before any service sees the request. */
export const CALLER_AUTH = ["authorization", "x-api-key", "proxy-authorization", "cookie", "cookie2"] as const;

const header = (req: IncomingMessage, name: string): string | undefined => {
  const v = req.headers[name];
  return typeof v === "string" && v !== "" ? v : undefined;
};

/** A header value split into its scheme and the rest, the scheme lowercased. */
const scheme = (value: string): { scheme: string; rest: string } => {
  const at = value.indexOf(" ");
  return at < 0 ? { scheme: "", rest: value } : { scheme: value.slice(0, at).toLowerCase(), rest: value.slice(at + 1).trim() };
};

/** The password of a Basic proxy login, which is where git puts the token through `http.proxy`. */
function proxyPassword(value: string): string | undefined {
  const { scheme: s, rest } = scheme(value);
  if (s !== "basic") return undefined;
  const plain = Buffer.from(rest, "base64").toString("utf8");
  const colon = plain.indexOf(":");
  return colon < 0 ? undefined : plain.slice(colon + 1) || undefined;
}

/** The turn token in an unsigned JWT's claim; Codex's placeholder access token is one. */
function jwtClaim(jwt: string): string | undefined {
  const parts = jwt.split(".");
  if (parts.length !== 3) return undefined;
  try {
    const claims: unknown = JSON.parse(Buffer.from(parts[1]!, "base64url").toString("utf8"));
    const turn = typeof claims === "object" && claims !== null ? (claims as Record<string, unknown>)[TURN_CLAIM] : undefined;
    return typeof turn === "string" && turn !== "" ? turn : undefined;
  } catch {
    return undefined;
  }
}

function bearer(value: string): string | undefined {
  const { scheme: s, rest } = scheme(value);
  if (s !== "bearer" || rest === "") return undefined;
  if (rest.startsWith(KEY_PREFIX)) return rest.slice(KEY_PREFIX.length) || undefined;
  if (rest.split(".").length === 3) return jwtClaim(rest);
  return rest;
}

/** The token a request carries, read only where its door's clients put it; nothing when it carries none. */
export function tokenOf(req: IncomingMessage, door: PipeDoor): string | undefined {
  if (door === "gh") {
    const auth = header(req, "authorization");
    if (auth === undefined) return undefined;
    const { scheme: s, rest } = scheme(auth);
    return s === "token" && rest !== "" ? rest : undefined;
  }
  const proxy = header(req, "proxy-authorization");
  if (proxy !== undefined) return proxyPassword(proxy);
  const key = header(req, "x-api-key");
  if (key !== undefined) return key;
  const auth = header(req, "authorization");
  return auth === undefined ? undefined : bearer(auth);
}

/** The request with every header a caller could carry a credential in taken out, from the parsed headers, the distinct
 * ones and the raw list, since a service passing headers byte for byte reads the raw one. Both parsed readings are
 * taken before the raw list shrinks: node builds each lazily from the raw list by the count its parser recorded, and
 * would read past the shortened end. */
export function dropCallerAuth(req: IncomingMessage): void {
  const drop = new Set<string>(CALLER_AUTH);
  const distinct = req.headersDistinct;
  for (const name of drop) {
    delete req.headers[name];
    delete distinct[name];
  }
  const raw: string[] = [];
  for (let i = 0; i + 1 < req.rawHeaders.length; i += 2) {
    if (!drop.has(req.rawHeaders[i]!.toLowerCase())) raw.push(req.rawHeaders[i]!, req.rawHeaders[i + 1]!);
  }
  req.rawHeaders.splice(0, req.rawHeaders.length, ...raw);
}

/** The path of a request target with no query, for an absolute-form target too, so a log line never keeps a query. */
export function pathOf(target: string | undefined): string {
  const t = target ?? "/";
  const path = /^[a-z][a-z0-9+.-]*:\/\//i.test(t) ? (() => {
    try {
      const u = new URL(t);
      return `${u.host}${u.pathname}`;
    } catch {
      return t;
    }
  })() : t;
  const q = path.search(/[?#]/);
  return q < 0 ? path : path.slice(0, q);
}

/** Whether a connector call may borrow the workspace's one Codex turn: on the HTTP door, under the connector path,
 * with no dot segment or encoded slash that an upstream could resolve to a path outside it. */
export function isConnectorCall(req: IncomingMessage, door: PipeDoor): boolean {
  const path = pathOf(req.url);
  const under = path === CONNECTOR_PATH || path.startsWith(`${CONNECTOR_PATH}/`);
  return door === "http" && under && !/(^|\/)\.\.?(\/|$)|%2e|%2f|%5c|\\/i.test(path);
}

/** Whether a call is Codex's refresh, whose token is read from its body: a POST to the refresh path on the HTTP door. */
export const isRefreshCall = (req: IncomingMessage, door: PipeDoor): boolean => door === "http" && req.method === "POST" && pathOf(req.url) === REFRESH_PATH;

/** The turn token in a refresh body's `refresh_token`, which the machine's placeholder auth file writes. */
export function refreshToken(body: Buffer): string | undefined {
  try {
    const refresh = (JSON.parse(body.toString("utf8")) as { refresh_token?: unknown }).refresh_token;
    return typeof refresh === "string" && refresh.startsWith(REFRESH_PREFIX) && refresh.length > REFRESH_PREFIX.length ? refresh.slice(REFRESH_PREFIX.length) : undefined;
  } catch {
    return undefined;
  }
}

/** The host a gh call names, lowercased with no port, which a storage host on the gh door is matched by. */
export const hostOf = (req: IncomingMessage): string => (req.headers.host ?? "").toLowerCase().replace(/:\d+$/, "");
