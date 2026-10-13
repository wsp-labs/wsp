// SPDX-License-Identifier: AGPL-3.0-only
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { codexLogin, RETRY_AFTER_MS } from "../src/codexlogin/login.js";
import { writeStub } from "../../protocol/test/stub-script.js";

const HOUR = 3600_000;

const jwt = (claims: Record<string, unknown>): string =>
  [{ alg: "none" }, claims].map(part => Buffer.from(JSON.stringify(part)).toString("base64url")).join(".") + ".sig";

/** An `auth.json` as Codex writes it, its access token running out `leftMs` from now. */
function authJson(leftMs: number, tag: string, account = "acct-acme"): string {
  const now = Math.floor(Date.now() / 1000);
  const auth = { chatgpt_account_id: account, chatgpt_plan_type: "plus", chatgpt_user_id: "user-acme" };
  return JSON.stringify({
    auth_mode: "chatgpt",
    OPENAI_API_KEY: null,
    tokens: {
      id_token: jwt({ exp: now, email: "lab@example.com", "https://api.openai.com/auth": auth }),
      access_token: jwt({ exp: now + Math.floor(leftMs / 1000), tag, "https://api.openai.com/auth": auth }),
      refresh_token: `rt_${tag}`,
      account_id: account,
    },
    last_refresh: new Date().toISOString(),
  });
}

const tagOf = (token: string | undefined): unknown => (token === undefined ? undefined : JSON.parse(Buffer.from(token.split(".")[1]!, "base64url").toString()).tag);

// The person's codex: answers initialize, and on `account/read {refreshToken: true}` logs the run, waits a moment so
// a second caller can arrive, writes `$CODEX_FRESH` over the login when it can, then answers. `getAuthStatus` answers
// the token a Keychain login, kept at `$STUB_STORE`, holds, refreshed the same way when asked. It ends when its stdin
// closes, as the real server does.
const CODEX = `#!/usr/bin/env node
const fs = require("node:fs"), path = require("node:path");
const file = path.join(process.env.CODEX_HOME || path.join(process.env.HOME, ".codex"), "auth.json");
let pending = "";
const say = m => process.stdout.write(JSON.stringify(m) + "\\n");
process.stdin.setEncoding("utf8").on("data", chunk => {
  pending += chunk;
  for (let at; (at = pending.indexOf("\\n")) >= 0; ) {
    const m = JSON.parse(pending.slice(0, at));
    pending = pending.slice(at + 1);
    if (m.method === "initialize") say({ id: m.id, result: {} });
    if (m.method === "account/read" && m.params.refreshToken) {
      fs.appendFileSync(process.env.CODEX_RUNS, "refresh\\n");
      setTimeout(() => {
        try { fs.writeFileSync(file, fs.readFileSync(process.env.CODEX_FRESH)); } catch {}
        say({ id: m.id, result: { account: { type: "chatgpt" }, requiresOpenaiAuth: true } });
      }, 300);
    }
    if (m.method === "getAuthStatus") {
      fs.appendFileSync(process.env.CODEX_RUNS, "status " + m.params.refreshToken + "\\n");
      const store = process.env.STUB_STORE;
      setTimeout(() => {
        if (m.params.refreshToken) try { fs.writeFileSync(store, fs.readFileSync(process.env.CODEX_FRESH)); } catch {}
        let token = null;
        try { token = JSON.parse(fs.readFileSync(store, "utf8")).tokens.access_token; } catch {}
        say({ id: m.id, result: { authMethod: token === null ? null : "chatgpt", authToken: token, requiresOpenaiAuth: true } });
      }, 300);
    }
  }
}).on("end", () => process.exit(0));
`;

/** A codex that starts a child holding its stdout and never answers, as npm's launcher leaves the native server once
 * it is killed. The child's pid goes to `$CODEX_RUNS.child`. */
const HANG = `#!/bin/sh
sleep 30 &
echo $! > "$CODEX_RUNS.child"
wait
`;

const dirs: string[] = [];
/** Pids a hung stub left, killed after each case so a failed one stops what it started. */
const children: number[] = [];
/** Puts back the modes a case took away, so the scratch folder can be removed even when the case failed. */
const unlocks: (() => void)[] = [];
afterEach(() => {
  for (const pid of children.splice(0)) if (alive(pid)) process.kill(pid, "SIGKILL");
  for (const unlock of unlocks.splice(0)) unlock();
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

interface Rig {
  env: NodeJS.ProcessEnv;
  folder: string;
  file: string;
  runs(): number;
  log(): string[];
  /** The pid the hung stub's child wrote. */
  child(): number;
  fresh(json: string): void;
  /** Makes the login file and its folder read-only, so any write the host tried would throw, and answers what they
   * held. */
  readOnly(): Snapshot;
}

interface Snapshot {
  bytes: string;
  mtimeMs: number;
  names: string[];
}

/** A scratch home holding a login at `~/.codex`, or at its own `CODEX_HOME` beside it, and the person's codex on PATH. */
function rig(login: string, { codexHome = false, keychain = false, codex = CODEX }: { codexHome?: boolean; keychain?: boolean; codex?: string } = {}): Rig {
  const dir = mkdtempSync(join(tmpdir(), "wsp-codexlogin-"));
  dirs.push(dir);
  const home = join(dir, "home");
  const folder = codexHome ? join(dir, "codex") : join(home, ".codex");
  mkdirSync(folder, { recursive: true });
  mkdirSync(join(dir, "bin"));
  writeFileSync(keychain ? join(dir, "store.json") : join(folder, "auth.json"), login, { mode: 0o600 });
  writeStub(join(dir, "bin", "codex"), codex);
  const env: NodeJS.ProcessEnv = {
    HOME: home,
    PATH: `${join(dir, "bin")}:${process.env.PATH ?? "/usr/bin:/bin"}`,
    CODEX_RUNS: join(dir, "runs"),
    CODEX_FRESH: join(dir, "fresh.json"),
    STUB_STORE: join(dir, "store.json"),
    ...(codexHome ? { CODEX_HOME: folder } : {}),
  };
  const file = join(folder, "auth.json");
  const log = (): string[] => (existsSync(join(dir, "runs")) ? readFileSync(join(dir, "runs"), "utf8").split("\n").filter(Boolean) : []);
  return {
    env,
    folder,
    file,
    runs: () => log().length,
    log,
    child: () => {
      const pid = Number(readFileSync(join(dir, "runs.child"), "utf8").trim());
      children.push(pid);
      return pid;
    },
    fresh: json => writeFileSync(join(dir, "fresh.json"), json),
    readOnly: () => {
      unlocks.push(() => chmodSync(folder, 0o700));
      chmodSync(file, 0o400);
      chmodSync(folder, 0o500);
      return snapshot({ file, folder });
    },
  };
}

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

const snapshot = ({ file, folder }: { file: string; folder: string }): Snapshot => ({ bytes: readFileSync(file, "utf8"), mtimeMs: statSync(file).mtimeMs, names: readdirSync(folder) });

describe("the host's Codex login", () => {
  it("reads the person's login at CODEX_HOME and writes nothing there, with the file and its folder read-only and ~/.codex unreadable", async () => {
    const r = rig(authJson(100 * HOUR, "far"), { codexHome: true });
    const other = join(r.env.HOME!, ".codex");
    mkdirSync(other, { recursive: true });
    writeFileSync(join(other, "auth.json"), authJson(100 * HOUR, "home", "acct-other"));
    unlocks.push(() => chmodSync(other, 0o700));
    chmodSync(other, 0o000);
    const before = r.readOnly();
    const got = await codexLogin({ env: r.env }).credential();
    expect(tagOf(got?.accessToken)).toBe("far");
    expect(got).toEqual({ accessToken: got?.accessToken, accountId: "acct-acme", planType: "plus", userId: "user-acme", fedramp: false });
    expect(snapshot(r)).toEqual(before);
    expect(r.runs()).toBe(0);
  });

  it("reads ~/.codex when CODEX_HOME is unset", async () => {
    const r = rig(authJson(100 * HOUR, "far"));
    const before = r.readOnly();
    expect(await codexLogin({ env: r.env }).claims()).toEqual({ accountId: "acct-acme", planType: "plus", userId: "user-acme", fedramp: false });
    expect(snapshot(r)).toEqual(before);
  });

  it("near expiry with the login read-only, runs the person's codex once, writes nothing itself and serves the token it has", async () => {
    const r = rig(authJson(2 * HOUR, "old"));
    r.fresh(authJson(240 * HOUR, "new"));
    const before = r.readOnly();
    let clock = Date.now();
    const login = codexLogin({ env: r.env, now: () => clock });
    expect(tagOf((await login.credential())?.accessToken)).toBe("old");
    expect(tagOf((await login.credential())?.accessToken)).toBe("old");
    expect([r.runs(), snapshot(r)]).toEqual([1, before]);
    clock += RETRY_AFTER_MS;
    await login.credential();
    expect(r.runs()).toBe(2);
  });

  it("two requests at once while the token is near expiry cause one refresh, and both get the fresh token", async () => {
    const r = rig(authJson(2 * HOUR, "old"));
    r.fresh(authJson(240 * HOUR, "new"));
    const login = codexLogin({ env: r.env });
    const both = await Promise.all([login.credential(), login.credential()]);
    expect(both.map(c => tagOf(c?.accessToken))).toEqual(["new", "new"]);
    expect(r.log()).toEqual(["refresh"]);
    expect(tagOf((await login.credential())?.accessToken)).toBe("new");
    expect(r.runs()).toBe(1);
  });

  it("gives nothing for a run-out token the person's codex could not refresh", async () => {
    const r = rig(authJson(-HOUR, "spent"));
    expect(await codexLogin({ env: r.env }).credential()).toBeUndefined();
    expect(r.runs()).toBe(1);
  });

  it("serves the token it has when no codex is on the PATH", async () => {
    const r = rig(authJson(2 * HOUR, "old"));
    const got = await codexLogin({ env: { ...r.env, PATH: "/usr/bin:/bin" } }).credential();
    expect(tagOf(got?.accessToken)).toBe("old");
  });

  it("reads a half-written file again", async () => {
    const whole = authJson(100 * HOUR, "whole");
    const reads = [whole.slice(0, 40), whole.slice(0, -40), whole];
    const got = await codexLogin({ env: { HOME: "/nowhere" }, read: async () => reads.shift() ?? whole }).credential();
    expect([tagOf(got?.accessToken), reads.length]).toEqual(["whole", 0]);
  });

  it("gives nothing for an API key login, and never asks codex for one", async () => {
    const r = rig(JSON.stringify({ auth_mode: "apikey", OPENAI_API_KEY: "sk-placeholder", tokens: null }));
    expect(await codexLogin({ env: r.env }).credential()).toBeUndefined();
    expect(r.runs()).toBe(0);
  });

  it("leaves out a plan or user the claim lacks, and gives nothing for a login with no account", async () => {
    const now = Math.floor(Date.now() / 1000);
    const claimsOf = async (auth: Record<string, string>, accountId?: string): Promise<unknown> => {
      const tokens = { access_token: jwt({ exp: now + 100 * 3600, "https://api.openai.com/auth": auth }), ...(accountId !== undefined ? { account_id: accountId } : {}) };
      return codexLogin({ env: rig(JSON.stringify({ auth_mode: "chatgpt", tokens })).env }).claims();
    };
    expect(await claimsOf({ chatgpt_user_id: "user-acme" }, "acct-acme")).toEqual({ accountId: "acct-acme", userId: "user-acme", fedramp: false });
    expect(await claimsOf({ chatgpt_account_id: "acct-acme", chatgpt_plan_type: "plus" })).toEqual({ accountId: "acct-acme", planType: "plus", fedramp: false });
    expect(await claimsOf({ chatgpt_plan_type: "plus", chatgpt_user_id: "user-acme" })).toBeUndefined();
  });

  it("reads the same login when CODEX_HOME is empty, as Codex does", async () => {
    const r = rig(authJson(100 * HOUR, "far"));
    expect(tagOf((await codexLogin({ env: { ...r.env, CODEX_HOME: "" } }).credential())?.accessToken)).toBe("far");
  });

  it("with no auth.json, reads a Keychain login through codex once and holds it until under a day is left", async () => {
    const r = rig(authJson(100 * HOUR, "keychain"), { keychain: true });
    let clock = Date.now();
    const login = codexLogin({ env: r.env, now: () => clock });
    const both = await Promise.all([login.credential(), login.credential()]);
    expect(both.map(c => tagOf(c?.accessToken))).toEqual(["keychain", "keychain"]);
    expect(both[0]).toEqual({ accessToken: both[0]?.accessToken, accountId: "acct-acme", planType: "plus", userId: "user-acme", fedramp: false });
    expect(tagOf((await login.credential())?.accessToken)).toBe("keychain");
    expect(r.log()).toEqual(["status false"]);
    r.fresh(authJson(240 * HOUR, "new"));
    clock += 77 * HOUR + RETRY_AFTER_MS;
    expect(tagOf((await login.credential())?.accessToken)).toBe("new");
    expect(r.log()).toEqual(["status false", "status true"]);
  });

  it("with no login anywhere, asks codex once and not again on every request", async () => {
    const r = rig(JSON.stringify({}), { keychain: true });
    rmSync(r.env.STUB_STORE!);
    const login = codexLogin({ env: r.env });
    expect(await login.credential()).toBeUndefined();
    expect(await login.credential()).toBeUndefined();
    expect(r.log()).toEqual(["status false"]);
  });

  it("ends a refresh whose codex hangs at the limit, its whole group killed, and serves the token it has", async () => {
    const r = rig(authJson(2 * HOUR, "old"), { codex: HANG });
    const got = await codexLogin({ env: r.env, refreshLimitMs: 300 }).credential();
    expect(tagOf(got?.accessToken)).toBe("old");
    const child = r.child();
    await expect.poll(() => alive(child), { timeout: 2000 }).toBe(false);
  });
});
