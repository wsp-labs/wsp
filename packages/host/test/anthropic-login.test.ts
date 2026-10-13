// SPDX-License-Identifier: AGPL-3.0-only
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir, userInfo } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { writeStub } from "../../protocol/test/stub-script.js";
import { claudeLogin, keychainItemOf, LOGIN_RENEW_MS } from "../src/anthropic/login.js";

let dir: string;
let store: string;
let security: string;
let claude: string;
let ps: string;
const MCP = { mcpOAuth: { "acme|0123": { accessToken: "mcp-token-for-test" } } };
const stored = (accessToken: string, expiresAt: number) => JSON.stringify({ claudeAiOauth: { accessToken, refreshToken: "refresh-for-test", expiresAt, scopes: ["user:inference"], subscriptionType: "max" }, ...MCP });
const lines = (name: string): string[] => {
  try {
    return readFileSync(join(dir, name), "utf8").trim().split("\n").filter(Boolean);
  } catch {
    return [];
  }
};
/** The processes the stub ps lists, as `uid name` lines. */
const running = (...names: string[]) => writeFileSync(join(dir, "procs"), [`${process.getuid!()} /bin/zsh`, ...names].join("\n") + "\n");
// The environment the host would hand its renewal: the person's sign-in variables beside ones that must reach it.
const env = { PATH: process.env.PATH, ANTHROPIC_API_KEY: "key-name-only", CLAUDE_CODE_OAUTH_TOKEN: "token-name-only", ACME_KEPT: "1" };

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "wsp-claude-login-"));
  store = join(dir, "store.json");
  // Prints the stored item as `security find-generic-password -w` does, or exits 44 as it does for a missing item.
  security = writeStub(join(dir, "security"), `#!/bin/sh\necho "$*" >> "${dir}/security.log"\n[ -f "${store}" ] || exit 44\ncat "${store}"\n`);
  // The person's claude: notes each run, then renews the login when the test left a renewal for it.
  claude = writeStub(join(dir, "claude"), `#!/bin/sh\necho "$*" >> "${dir}/claude.log"\npwd >> "${dir}/claude-cwd.log"\nenv | cut -d= -f1 >> "${dir}/claude-env.log"\necho "\${CLAUDE_CONFIG_DIR:-}" >> "${dir}/claude-config.log"\nsleep 0.2\n[ -f "${dir}/renewed.json" ] && cp "${dir}/renewed.json" "${store}"\nexit 1\n`);
  ps = writeStub(join(dir, "ps"), `#!/bin/sh\necho "$*" >> "${dir}/ps.log"\ncat "${dir}/procs"\n`);
  running();
});
afterEach(() => rmSync(dir, { recursive: true, force: true }));

describe("the person's claude.ai login, read for each request", () => {
  it("reads the Keychain item with Claude Code's own line and nothing else", async () => {
    const expiresAt = Date.now() + 3_600_000;
    writeFileSync(store, stored("login-for-test", expiresAt));
    const credential = claudeLogin({ platform: "darwin", security, claude, ps, env });
    expect(await credential()).toEqual({ kind: "token", token: "login-for-test", expiresAt });
    expect(await credential()).toEqual({ kind: "token", token: "login-for-test", expiresAt });
    expect(lines("security.log")).toEqual(Array(2).fill(`find-generic-password -a ${userInfo().username} -w -s Claude Code-credentials`));
    expect(lines("claude.log")).toEqual([]);
  });

  it("reads the item a CLAUDE_CONFIG_DIR names, hashed as written", async () => {
    // Claude Code 2.1.296 asked for these two names with CLAUDE_CONFIG_DIR set to them.
    expect(keychainItemOf({ CLAUDE_CONFIG_DIR: "/tmp/trig2183.TgWyn1/m1/cfg" })).toBe("Claude Code-credentials-909dbccd");
    expect(keychainItemOf({ CLAUDE_CONFIG_DIR: "/tmp/trig2183.TgWyn1/m1/cfg/" })).toBe("Claude Code-credentials-ef9936d8");
    expect(keychainItemOf({ CLAUDE_CONFIG_DIR: "" })).toBe("Claude Code-credentials");
    writeFileSync(store, stored("login-for-test", Date.now() + 3_600_000));
    await claudeLogin({ platform: "darwin", security, claude, ps, env: { ...env, CLAUDE_CONFIG_DIR: "/tmp/trig2183.TgWyn1/m1/cfg" } })();
    expect(lines("security.log")).toEqual([`find-generic-password -a ${userInfo().username} -w -s Claude Code-credentials-909dbccd`]);
  });

  it("finds no login in a missing item, a blanked login or an item holding only MCP tokens", async () => {
    const credential = claudeLogin({ platform: "darwin", security, claude, ps, env });
    expect(await credential()).toBeUndefined();
    writeFileSync(store, stored("", 0));
    expect(await credential()).toBeUndefined();
    writeFileSync(store, JSON.stringify(MCP));
    expect(await credential()).toBeUndefined();
    expect(lines("claude.log")).toEqual([]);
  });

  it("reads .credentials.json in ~/.claude, or in CLAUDE_CONFIG_DIR, off a Mac", async () => {
    const expiresAt = Date.now() + 3_600_000;
    mkdirSync(join(dir, ".claude"));
    writeFileSync(join(dir, ".claude", ".credentials.json"), stored("home-login", expiresAt));
    mkdirSync(join(dir, "elsewhere"));
    writeFileSync(join(dir, "elsewhere", ".credentials.json"), stored("config-dir-login", expiresAt));
    expect(await claudeLogin({ platform: "linux", home: dir, security, claude, ps, env })()).toEqual({ kind: "token", token: "home-login", expiresAt });
    expect(await claudeLogin({ platform: "linux", home: dir, security, claude, ps, env: { ...env, CLAUDE_CONFIG_DIR: join(dir, "elsewhere") } })()).toEqual({ kind: "token", token: "config-dir-login", expiresAt });
    expect(lines("security.log")).toEqual([]);
  });
});

describe("renewing a login near its expiry through the person's own claude", () => {
  it("leaves a login with five minutes or more alone", async () => {
    writeFileSync(store, stored("login-for-test", Date.now() + LOGIN_RENEW_MS + 5_000));
    expect((await claudeLogin({ platform: "darwin", security, claude, ps, env })())?.kind).toBe("token");
    expect(lines("claude.log")).toEqual([]);
    expect(lines("ps.log")).toEqual([]);
  });

  it("with more than a minute left, serves the token it read at once while the renewal runs, and the next request the renewed one", async () => {
    const old = Date.now() + 4 * 60_000;
    writeFileSync(store, stored("old-login", old));
    const fresh = Date.now() + 8 * 3_600_000;
    writeFileSync(join(dir, "renewed.json"), stored("new-login", fresh));
    const credential = claudeLogin({ platform: "darwin", security, claude, ps, env });
    expect(await credential()).toEqual({ kind: "token", token: "old-login", expiresAt: old });
    expect(lines("claude.log")).toEqual([]);
    for (let i = 0; i < 100 && readFileSync(store, "utf8").includes("old-login"); i++) await new Promise(done => setTimeout(done, 20));
    expect(await credential()).toEqual({ kind: "token", token: "new-login", expiresAt: fresh });
    expect(lines("claude.log")).toEqual(["mcp get wsp-login-refresh"]);
  });

  it("with a claude of the person's running and minutes left, waits for it in the background, not in the request", async () => {
    running(`${process.getuid!()} claude`);
    writeFileSync(store, stored("old-login", Date.now() + 4 * 60_000));
    const started = Date.now();
    expect((await claudeLogin({ platform: "darwin", security, claude, ps, env, pollMs: 50, waitMs: 5_000 })())?.kind).toBe("token");
    expect(Date.now() - started).toBeLessThan(1_000);
    // Their session renews; the wait reads it and ends with no refresh of the host's.
    writeFileSync(store, stored("their-login", Date.now() + 8 * 3_600_000));
    const reads = lines("security.log").length;
    for (let i = 0; i < 100 && lines("security.log").length === reads; i++) await new Promise(done => setTimeout(done, 20));
    await new Promise(done => setTimeout(done, 100));
    expect(lines("claude.log")).toEqual([]);
  });

  it("with no claude of the person's running, runs claude mcp get once under a minute, away from any project, without the variables that pick another sign-in, and reads again", async () => {
    writeFileSync(store, stored("old-login", Date.now() + 30_000));
    const fresh = Date.now() + 8 * 3_600_000;
    writeFileSync(join(dir, "renewed.json"), stored("new-login", fresh));
    expect(await claudeLogin({ platform: "darwin", security, claude, ps, env: { ...env, CLAUDE_CONFIG_DIR: join(dir, "elsewhere") } })()).toEqual({ kind: "token", token: "new-login", expiresAt: fresh });
    expect(lines("ps.log")).toEqual(["-A -o uid=,comm="]);
    expect(lines("claude.log")).toEqual(["mcp get wsp-login-refresh"]);
    expect(lines("claude-cwd.log")[0]).not.toContain(process.cwd());
    expect(lines("claude-env.log")).toContain("ACME_KEPT");
    expect(lines("claude-env.log").filter(name => ["ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN"].includes(name))).toEqual([]);
    expect(lines("claude-config.log")).toEqual([join(dir, "elsewhere")]);
  });

  it("counts a claude of another user's, or a process merely named like it, as none running", async () => {
    running(`${process.getuid!() + 1} claude`, `${process.getuid!()} /usr/local/bin/claude-helper`);
    writeFileSync(store, stored("old-login", Date.now() - 1_000));
    writeFileSync(join(dir, "renewed.json"), stored("new-login", Date.now() + 8 * 3_600_000));
    expect((await claudeLogin({ platform: "darwin", security, claude, ps, env, pollMs: 50, waitMs: 5_000 })())?.kind).toBe("token");
    expect(lines("claude.log")).toHaveLength(1);
    // Read, the renewal's own check, read again: no polls, so it never waited.
    expect(lines("security.log")).toHaveLength(3);
  });

  it("with a claude of the person's running, waits and reads again instead of refreshing, and serves what it saved", async () => {
    running(`${process.getuid!()} /usr/local/bin/claude`);
    writeFileSync(store, stored("old-login", Date.now() + 30_000));
    const fresh = Date.now() + 8 * 3_600_000;
    setTimeout(() => writeFileSync(store, stored("their-login", fresh)), 150);
    expect(await claudeLogin({ platform: "darwin", security, claude, ps, env, pollMs: 50, waitMs: 5_000 })()).toEqual({ kind: "token", token: "their-login", expiresAt: fresh });
    expect(lines("claude.log")).toEqual([]);
    expect(lines("security.log").length).toBeGreaterThan(2);
  });

  it("refreshes itself once the wait for a running session runs out", async () => {
    running(`${process.getuid!()} claude`);
    writeFileSync(store, stored("old-login", Date.now() + 30_000));
    writeFileSync(join(dir, "renewed.json"), stored("new-login", Date.now() + 8 * 3_600_000));
    const started = Date.now();
    expect((await claudeLogin({ platform: "darwin", security, claude, ps, env, pollMs: 50, waitMs: 300 })())?.kind).toBe("token");
    expect(Date.now() - started).toBeGreaterThanOrEqual(300);
    expect(lines("claude.log")).toHaveLength(1);
  });

  it("runs one renewal for requests that arrive together", async () => {
    writeFileSync(store, stored("old-login", Date.now() + 30_000));
    writeFileSync(join(dir, "renewed.json"), stored("new-login", Date.now() + 8 * 3_600_000));
    const credential = claudeLogin({ platform: "darwin", security, claude, ps, env });
    const got = await Promise.all([credential(), credential(), credential(), credential()]);
    expect(got.map(c => (c?.kind === "token" ? c.token : undefined))).toEqual(Array(4).fill("new-login"));
    expect(lines("claude.log")).toHaveLength(1);
  });

  it("hands the old login on when a renewal changed nothing, and runs none again for five minutes", async () => {
    let t = 1_000_000_000_000;
    writeFileSync(store, stored("old-login", t + 30_000));
    const credential = claudeLogin({ platform: "darwin", security, claude, ps, env, now: () => t });
    expect(await credential()).toEqual({ kind: "token", token: "old-login", expiresAt: t + 30_000 });
    t += 4 * 60_000;
    await credential();
    expect(lines("claude.log")).toHaveLength(1);
    t += 61_000;
    await credential();
    expect(lines("claude.log")).toHaveLength(2);
  });
});
