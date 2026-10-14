// SPDX-License-Identifier: AGPL-3.0-only

import { execFile, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { platform as osPlatform, tmpdir, userInfo } from "node:os";
import { basename, join } from "node:path";
import type { AnthropicCredential } from "./service.js";
import { configDirOf, type ClaudeHere } from "./settings.js";

/** Under this much left the host renews the login: the line Claude Code renews at itself. */
export const LOGIN_RENEW_MS = 5 * 60_000;
/** Under this much left a request waits for the renewal rather than go out on a token about to lapse; above it, it
 * goes on the token it read while the renewal runs. */
const HOLD_UNDER_MS = 60_000;
/** A host renewal that left the login near expiry is not run again sooner than this. */
const FAILED_GAP_MS = 5 * 60_000;
/** While one of the person's own sessions runs, the host waits this long for it to renew the login, reading every poll. */
const WAIT_MS = 2 * 60_000;
const POLL_MS = 15_000;
const RENEW_LIMIT_MS = 60_000;
/** `claude mcp get` loads the login, waits for its renewal and saves it, then finds no server by this name and starts
 * none. `claude auth status` sends the renewal and exits before the answer, losing the rotated token. */
export const RENEW_ARGS = ["mcp", "get", "wsp-login-refresh"] as const;
// Any of these makes the person's claude read another sign-in than their login, or spend the run retrying calls.
// Their CLAUDE_CONFIG_DIR passes: it names the login being renewed.
const NOT_PASSED = /^(ANTHROPIC_|CLAUDE_CODE_OAUTH_|CLAUDE_CODE_USE_)/;

interface Login {
  accessToken: string;
  expiresAt: number;
}

/** The claude.ai login out of Claude Code's stored credentials, which also hold every MCP server's OAuth tokens;
 * a login Claude Code blanked after a dead refresh token is no login. */
export function loginOf(text: string): Login | undefined {
  try {
    const oauth = (JSON.parse(text) as { claudeAiOauth?: { accessToken?: unknown; expiresAt?: unknown } }).claudeAiOauth;
    if (typeof oauth?.accessToken !== "string" || oauth.accessToken === "" || typeof oauth.expiresAt !== "number") return undefined;
    return { accessToken: oauth.accessToken, expiresAt: oauth.expiresAt };
  } catch {
    return undefined;
  }
}

interface LoginOptions extends ClaudeHere {
  platform?: NodeJS.Platform;
  security?: string;
  claude?: string;
  ps?: string;
  waitMs?: number;
  pollMs?: number;
  now?: () => number;
}

/** The Keychain item Claude Code keeps the login in: a CLAUDE_CONFIG_DIR, exactly as written, names its own. */
export function keychainItemOf(env: NodeJS.ProcessEnv): string {
  const dir = env["CLAUDE_CONFIG_DIR"];
  return dir ? `Claude Code-credentials-${createHash("sha256").update(dir).digest("hex").slice(0, 8)}` : "Claude Code-credentials";
}

/** Claude Code's own read of its stored credentials, the login beside every MCP server's sign-in: the Keychain item on
 * a Mac (`security` made it, so reading it asks nothing), the credentials file elsewhere. Never written. */
export function claudeStore(options: Pick<LoginOptions, "platform" | "security" | "env" | "home"> = {}): Promise<string | undefined> {
  const { platform = osPlatform(), security = "/usr/bin/security" } = options;
  if (platform !== "darwin") return readFile(join(configDirOf(options), ".credentials.json"), "utf8").catch(() => undefined);
  return new Promise(done => {
    execFile(security, ["find-generic-password", "-a", userInfo().username, "-w", "-s", keychainItemOf(options.env ?? process.env)], { timeout: 10_000, maxBuffer: 8 << 20 }, (error, stdout) => done(error ? undefined : stdout));
  });
}

/** One listing of process names, no command lines: is a claude of this user's running? A listing that fails counts
 * as one running, so the host waits rather than refresh beside a session it could not see. */
function claudeRunning(ps: string): Promise<boolean> {
  const uid = process.getuid?.();
  return new Promise(done => {
    execFile(ps, ["-A", "-o", "uid=,comm="], { timeout: 10_000, maxBuffer: 8 << 20 }, (error, stdout) => {
      done(error !== null || stdout.split("\n").some(line => {
        const [owner, ...name] = line.trim().split(/\s+/);
        return Number(owner) === uid && basename(name.join(" ")) === "claude";
      }));
    });
  });
}

function renew(claude: string, passed: NodeJS.ProcessEnv): Promise<void> {
  const env = Object.fromEntries(Object.entries(passed).filter(([name]) => !NOT_PASSED.test(name)));
  return new Promise(done => {
    const child = spawn(claude, RENEW_ARGS, { cwd: tmpdir(), env, stdio: "ignore", timeout: RENEW_LIMIT_MS });
    child.on("error", () => done());
    child.on("close", () => done());
  });
}

/** The person's claude.ai login for the Anthropic service, read fresh for each request and never renewed by the host
 * itself. Two Claude Code 2.1.296 processes sharing one store both used a rotated refresh token, so two refreshes in
 * the same seconds can sign the person out; whether Anthropic refuses a refresh token reused within seconds was not
 * measured. So with under five minutes left the host first waits for a running session of theirs to renew it, serving
 * the still-valid token meanwhile, and only when none is running, or none renewed it in two minutes, runs their claude
 * once and reads again. */
export function claudeLogin(options: LoginOptions = {}): () => Promise<AnthropicCredential | undefined> {
  const now = options.now ?? Date.now;
  const { claude = "claude", ps = "/bin/ps", waitMs = WAIT_MS, pollMs = POLL_MS } = options;
  let renewing: Promise<void> | undefined;
  let lastFailed = -Infinity;
  const read = async () => {
    const text = await claudeStore(options);
    return text === undefined ? undefined : loginOf(text);
  };
  const fresh = (login: Login | undefined) => login !== undefined && login.expiresAt - now() >= LOGIN_RENEW_MS;
  const renewal = async () => {
    if (await claudeRunning(ps)) {
      for (let waited = 0; waited < waitMs; waited += pollMs) {
        await new Promise(done => setTimeout(done, pollMs));
        if (fresh(await read())) return;
      }
    }
    if (now() - lastFailed < FAILED_GAP_MS) return;
    await renew(claude, options.env ?? process.env);
    if (!fresh(await read())) lastFailed = now();
  };
  return async () => {
    let login = await read();
    if (login !== undefined && !fresh(login)) {
      renewing ??= renewal().finally(() => (renewing = undefined));
      if (login.expiresAt - now() < HOLD_UNDER_MS) {
        await renewing;
        login = await read();
      }
    }
    return login && { kind: "token", token: login.accessToken, expiresAt: login.expiresAt };
  };
}
