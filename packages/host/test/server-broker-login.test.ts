// SPDX-License-Identifier: AGPL-3.0-only
// The broker borrows the logins the person's own Claude Code and Codex keep in
// the folders their login shell names, on a host launched without them (the
// desktop app under launchd, a service), and never puts those folders on the
// host's own environment, where they would move its local turns' stores. A
// file of its own: reading the login shell sets this worker's environment.
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { writeStub } from "../../protocol/test/stub-script.js";
import { adoptLoginPath, loginFolders } from "../src/login-path.js";
import { hostBroker, personSignIns, signInEnv } from "../src/server-broker.js";

const saved = { ...process.env };
const dirs: string[] = [];
afterEach(() => {
  for (const name of Object.keys(process.env)) if (!(name in saved)) delete process.env[name];
  Object.assign(process.env, saved);
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

it("reads the Claude Code and Codex logins in the folders the login shell names, and leaves the host's environment alone", async () => {
  const dir = mkdtempSync(join(tmpdir(), "wsp-broker-login-"));
  dirs.push(dir);
  const home = join(dir, "home");
  const claudeDir = join(dir, "claude-cfg");
  const codexDir = join(dir, "codex");
  for (const d of [home, claudeDir, codexDir]) mkdirSync(d);
  const b64 = (o: object): string => Buffer.from(JSON.stringify(o)).toString("base64url");
  const exp = Math.floor(Date.now() / 1000) + 5 * 86_400;
  const jwt = `${b64({ alg: "none" })}.${b64({ exp, "https://api.openai.com/auth": { chatgpt_account_id: "acct-acme" } })}.sig`;
  writeFileSync(join(codexDir, "auth.json"), JSON.stringify({ tokens: { access_token: jwt, account_id: "acct-acme" } }));
  writeFileSync(join(claudeDir, ".credentials.json"), JSON.stringify({ claudeAiOauth: { accessToken: "acme-oauth", expiresAt: Date.now() + 86_400_000 } }));
  const shell = writeStub(join(dir, "shell.sh"), `#!/bin/sh\nprintf 'PATH=/usr/bin:/bin\\0CLAUDE_CONFIG_DIR=${claudeDir}\\0CODEX_HOME=${codexDir}\\0'\n`);
  // A host the desktop app launched: launchd's PATH and none of the login folders on its own environment.
  process.env["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin";
  process.env["SHELL"] = shell;
  process.env["HOME"] = home;
  delete process.env["CLAUDE_CONFIG_DIR"];
  delete process.env["CODEX_HOME"];
  await adoptLoginPath(() => undefined);
  expect(loginFolders()).toEqual({ claudeConfigDir: claudeDir, codexHome: codexDir });

  const env = signInEnv();
  expect(env).toMatchObject({ CLAUDE_CONFIG_DIR: claudeDir, CODEX_HOME: codexDir });
  expect(process.env["CLAUDE_CONFIG_DIR"]).toBeUndefined();
  expect(process.env["CODEX_HOME"]).toBeUndefined();

  // The Claude reader reads the credentials file, as on Linux, so nothing here reaches the Keychain.
  const broker = hostBroker({ statePath: join(dir, "state.json"), vault: () => ({}), services: [], signIns: personSignIns(env, { platform: "linux" }), run: async () => ({ code: 1, stdout: "" }) });
  await broker.start({ rt: { events: { on: () => () => undefined } } as never, server: {} as never });
  try {
    expect(await broker.port.codexClaims()).toMatchObject({ accountId: "acct-acme" });
    expect(await broker.port.status!()).toMatchObject({ claude: "login", codex: "chatgpt" });
  } finally {
    broker.stop();
  }
});
