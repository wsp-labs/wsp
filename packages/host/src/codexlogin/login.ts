// SPDX-License-Identifier: AGPL-3.0-only
// The ChatGPT login cloud threads ride on: the person's own Codex sign-in on
// this computer, borrowed the way a second Codex window borrows it. The host
// reads `auth.json` for every request and never writes it. Codex refreshes a
// token only once it is nearly spent (60 s left in a measured run, not 2 h), so
// with under a day left the host runs the person's own `codex app-server` and
// asks it for an early refresh, one run at a time, then reads the file again. A
// Codex the person already has open picks up the rotated file instead of
// spending its old refresh token (measured on 0.155.1). Codex rewrites the file
// in place, so a read can land on half of it; a parse error is read again.
//
// A Codex that keeps its login in the Keychain leaves no `auth.json`. Then the
// same run answers `getAuthStatus {includeToken: true}` and the host holds that
// token in memory, as a running Codex does, until under a day is left.
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { answersOf, initializeRequest, notification, request } from "@wsp/catalog";
import type { CodexClaims } from "@wsp/runtime";
import type { ChatgptCredential } from "../chatgpt/types.js";

export interface CodexLoginDeps {
  /** The host's own environment, which takes `CODEX_HOME` and `PATH` from the person's login shell: `CODEX_HOME` or
   * `HOME` find the login, and `PATH` finds the person's `codex`. */
  env: NodeJS.ProcessEnv;
  now?: () => number;
  read?: (path: string) => Promise<string>;
  refreshLimitMs?: number;
}

export interface CodexLogin {
  /** The person's ChatGPT access token and claims, refreshed through their own Codex when under a day is left, or
   * nothing where Codex is not signed in to ChatGPT here or the token has run out. */
  credential(): Promise<ChatgptCredential | undefined>;
  claims(): Promise<CodexClaims | undefined>;
}

export const REFRESH_UNDER_MS = 24 * 3600_000;
/** How soon a run that left no usable token is tried again, so a Codex that cannot refresh, or is signed out, is not
 * run on every request. */
export const RETRY_AFTER_MS = 5 * 60_000;
const REFRESH_LIMIT_MS = 30_000;
const PARSE_TRIES = 5;
const PARSE_WAIT_MS = 40;

type Login = ChatgptCredential & { expiresAt: number };

/** No `auth.json` at all: the login is in the Keychain, or there is none. */
const NO_FILE = Symbol("no file");

export function codexLogin(deps: CodexLoginDeps): CodexLogin {
  const now = deps.now ?? Date.now;
  const read = deps.read ?? ((path: string) => readFile(path, "utf8"));
  // `||`, not `??`: Codex reads an empty CODEX_HOME as unset.
  const file = join(deps.env.CODEX_HOME || join(deps.env.HOME || homedir(), ".codex"), "auth.json");
  const limitMs = deps.refreshLimitMs ?? REFRESH_LIMIT_MS;
  const nearExpiry = (login: Login): boolean => login.expiresAt - now() < REFRESH_UNDER_MS;
  let held: Login | undefined;
  let running: Promise<void> | undefined;
  let lastRun = -Infinity;

  async function readFileLogin(): Promise<Login | undefined | typeof NO_FILE> {
    for (let attempt = 1; ; attempt++) {
      let text: string;
      try {
        text = await read(file);
      } catch (err) {
        return (err as NodeJS.ErrnoException).code === "ENOENT" ? NO_FILE : undefined;
      }
      try {
        const tokens = rec(rec(JSON.parse(text))?.tokens);
        return loginOf(tokens?.access_token, tokens?.account_id);
      } catch {
        if (attempt === PARSE_TRIES) return undefined;
        await new Promise(resolve => setTimeout(resolve, PARSE_WAIT_MS));
      }
    }
  }

  const stored = async (): Promise<Login | undefined | typeof NO_FILE> => {
    const login = await readFileLogin();
    return login === NO_FILE && held !== undefined ? held : login;
  };

  async function run(): Promise<void> {
    // Read again inside the one run: a caller that read before another's run landed finds it fresh here.
    const login = await stored();
    if (login === undefined || (login !== NO_FILE && !nearExpiry(login)) || now() - lastRun < RETRY_AFTER_MS) return;
    lastRun = now();
    if (login !== NO_FILE && held !== login) {
      await askCodex(deps.env, limitMs, "account/read", { refreshToken: true });
      return;
    }
    const answer = await askCodex(deps.env, limitMs, "getAuthStatus", { includeToken: true, refreshToken: login !== NO_FILE });
    const got = answer?.authMethod === "chatgpt" ? loginOf(answer.authToken) : undefined;
    if (got !== undefined || login === NO_FILE || login.expiresAt <= now()) held = got;
  }

  async function credential(): Promise<ChatgptCredential | undefined> {
    const first = await stored();
    if (first === undefined) return undefined;
    if (first !== NO_FILE && !nearExpiry(first)) return credentialOf(first);
    running ??= run().finally(() => {
      running = undefined;
    });
    await running;
    const login = await stored();
    return login !== undefined && login !== NO_FILE && login.expiresAt > now() ? credentialOf(login) : undefined;
  }

  async function claims(): Promise<CodexClaims | undefined> {
    const got = await credential();
    if (got === undefined) return undefined;
    const { accessToken: _token, ...claims } = got;
    return claims;
  }

  return { credential, claims };
}

const credentialOf = ({ expiresAt: _expiresAt, ...credential }: Login): ChatgptCredential => credential;

function rec(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}

const text = (value: unknown): string | undefined => (typeof value === "string" && value !== "" ? value : undefined);

function jwtClaims(token: string): Record<string, unknown> | undefined {
  const body = token.split(".")[1];
  if (body === undefined) return undefined;
  try {
    return rec(JSON.parse(Buffer.from(body, "base64url").toString("utf8")));
  } catch {
    return undefined;
  }
}

/** A ChatGPT login from an access token, or nothing for a token that does not decode or names no account. */
function loginOf(token: unknown, accountIdField?: unknown): Login | undefined {
  const accessToken = text(token);
  if (accessToken === undefined) return undefined;
  const jwt = jwtClaims(accessToken);
  const auth = rec(jwt?.["https://api.openai.com/auth"]);
  const accountId = text(accountIdField) ?? text(auth?.chatgpt_account_id);
  const planType = text(auth?.chatgpt_plan_type);
  const userId = text(auth?.chatgpt_user_id);
  if (typeof jwt?.exp !== "number" || accountId === undefined) return undefined;
  return {
    accessToken,
    expiresAt: jwt.exp * 1000,
    accountId,
    ...(planType !== undefined ? { planType } : {}),
    ...(userId !== undefined ? { userId } : {}),
    fedramp: auth?.chatgpt_account_is_fedramp === true,
  };
}

/** Runs the person's `codex app-server` with their own environment and puts one request to it, answering its result,
 * or nothing where it answered an error, failed to start or ran past the limit. The server runs in a group of its own:
 * npm's `codex` launcher passes SIGKILL on to nobody, and the native server it leaves behind keeps stdout open, so the
 * limit kills the group and settles the run itself rather than waiting for the pipe to close. */
function askCodex(env: NodeJS.ProcessEnv, limitMs: number, method: string, params: Record<string, unknown>): Promise<Record<string, unknown> | undefined> {
  return new Promise(resolve => {
    const child = spawn("codex", ["app-server"], { env, detached: true, stdio: ["pipe", "pipe", "ignore"] });
    let answer: Record<string, unknown> | undefined;
    const limit = setTimeout(() => {
      try {
        if (child.pid !== undefined) process.kill(-child.pid, "SIGKILL");
      } catch {
        // The group has already gone.
      }
      resolve(answer);
    }, limitMs);
    child.on("error", () => {
      clearTimeout(limit);
      resolve(undefined);
    });
    child.on("close", () => {
      clearTimeout(limit);
      resolve(answer);
    });
    let pending = "";
    let asked = false;
    child.stdin.on("error", () => {});
    child.stdout.setEncoding("utf8").on("data", (chunk: string) => {
      pending += chunk;
      const answers = answersOf(pending);
      // The server exits once its stdin closes, before answering what it was still reading, so the request goes only
      // after initialize is answered and stdin closes only after the request is. The limit still runs, so a server
      // that answers and stays is killed with its group.
      const got = answers.get(1);
      if (got !== undefined) {
        answer = "result" in got ? got.result : undefined;
        child.stdin.end();
        resolve(answer);
      } else if (answers.has(0) && !asked) {
        asked = true;
        child.stdin.write(`${notification("initialized")}\n${request(1, method, params)}\n`);
      }
    });
    child.stdin.write(`${initializeRequest(0)}\n`);
  });
}
