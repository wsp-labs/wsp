// SPDX-License-Identifier: AGPL-3.0-only
// The order a Claude launch signs in by: wsp's own token or key from the vault,
// else the person's own Claude sign-in on the computer, else a token or key the
// launch inherited from the login shell, which is a fallback and never a winner.
import { execFile, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import { afterAll, describe, expect, it } from "vitest";
import type { ExecStream, ExecStreamFactory } from "@wsp/protocol";
import { createClaudeAdapter } from "../src/adapter.js";
import { type BrokeredSignIn, buildEnv, signInGuard } from "../src/landmines.js";
import { writeStub } from "../../protocol/test/stub-script.js";

const RC_KEY = "sk-ant-x-from-zshrc";
const RC_TOKEN = "sk-ant-oat01-x-from-zshrc";
const WSP_TOKEN = "sk-ant-oat01-x-from-wsp-vault";

const roots: string[] = [];
afterAll(() => {
  for (const root of roots) rmSync(root, { recursive: true, force: true });
});

/** A stand-in claude: `auth status` answers as 2.1.296 prints it, signed in by whatever credential its environment
 * holds, else by the person's own login (a Keychain or file login) or by an apiKeyHelper in their settings, which
 * auth status reports without running; any other line reports the credentials it ran with in a print-mode result. */
function standIn(own: "none" | "login" | "helper"): { bin: string; home: string } {
  const root = mkdtempSync(join(tmpdir(), "wsp-claude-signin-"));
  roots.push(root);
  const login = join(root, "own-login");
  const helper = join(root, "helper");
  if (own === "login") writeFileSync(login, "");
  if (own === "helper") writeFileSync(helper, "");
  writeFileSync(join(root, "asked"), "a diff\n");
  writeStub(
    join(root, "claude"),
    [
      "#!/bin/sh",
      'if [ "$1" = auth ]; then',
      '  if [ -n "$ANTHROPIC_API_KEY$CLAUDE_CODE_OAUTH_TOKEN" ]; then printf \'{\\n  "loggedIn": true,\\n  "authMethod": "api_key"\\n}\\n\';',
      `  elif [ -f ${JSON.stringify(login)} ]; then printf '{\\n  "loggedIn": true,\\n  "authMethod": "claude.ai"\\n}\\n';`,
      `  elif [ -f ${JSON.stringify(helper)} ]; then printf '{\\n  "loggedIn": true,\\n  "authMethod": "api_key_helper"\\n}\\n';`,
      "  else printf '{\\n  \"loggedIn\": false,\\n  \"authMethod\": \"none\"\\n}\\n'; fi",
      "  exit 0",
      "fi",
      'cat > /dev/null; printf \'{"type":"result","is_error":false,"result":"key=%s token=%s"}\\n\' "${ANTHROPIC_API_KEY:-none}" "${CLAUDE_CODE_OAUTH_TOKEN:-none}"',
      "",
    ].join("\n"),
  );
  return { bin: root, home: root };
}

const PATH_TAIL = "/usr/bin:/bin:/usr/sbin:/sbin";

/** The environment a host on this computer hands a turn: its own, with whatever the login shell lent it. */
const inherited = (bin: string, lent: Record<string, string>): Record<string, string> => ({ PATH: `${bin}:${PATH_TAIL}`, HOME: bin, ...lent });

/** The turn's launch line as the adapter hands it to the exec, run the way this computer runs one: its environment as
 * the process's own. Answers what the stand-in signed in with. */
function launchSignsInWith(base: Record<string, string>, vault: { oauthToken?: string; apiKey?: string; brokered?: BrokeredSignIn } = {}): string {
  const calls: { command: string; env: Readonly<Record<string, string>> }[] = [];
  const factory: ExecStreamFactory = (command, { env }) => {
    calls.push({ command, env: env ?? {} });
    const stream: ExecStream = {
      run: { pid: 1 } as unknown as ExecStream["run"],
      lines: (async function* () {})(),
      teardown: () => {},
      kill: () => {},
      write: async () => "written" as const,
      closeInput: () => {},
      exited: Promise.resolve(0),
    };
    return stream;
  };
  const adapter = createClaudeAdapter({ exec: factory, configDir: join(base["HOME"]!, ".claude"), baseEnv: base, ...vault, resultExitMs: 5, interruptGraceMs: 5 });
  adapter.start({ prompt: "go", onEvent: () => {} });
  const call = calls[0]!;
  const ran = spawnSync("bash", ["-c", call.command], { env: { ...call.env }, input: "", encoding: "utf8" });
  return /key=[^ "]+ token=[^ "]+/.exec(ran.stdout)?.[0] ?? `no result: ${ran.stdout} ${ran.stderr}`;
}

const run = promisify(execFile);

/** A title question and a commit-message draft as this computer's exec runs them: bash under the host's own
 * environment, which holds what the login shell lent it, with the question's variables on its input as NAME=value NUL
 * entries. Answers what the stand-in signed in with, which must be the same for both. */
async function titleSignsInWith(base: Record<string, string>, vault: { oauthToken?: string; brokered?: BrokeredSignIn } = {}): Promise<string> {
  const adapter = createClaudeAdapter({ exec: () => { throw new Error("no turns here"); }, configDir: join(base["HOME"]!, ".claude"), baseEnv: base, ...vault });
  const seen: string[] = [];
  const exec = async (cmd: string, env?: Readonly<Record<string, string>>): Promise<string> => {
    const running = run("bash", ["-c", cmd], { env: { ...base } });
    running.child.stdin?.end(Object.entries(env ?? {}).map(([k, v]) => `${k}=${v}\0`).join(""));
    const out = (await running).stdout;
    seen.push(/key=[^ "]+ token=[^ "]+/.exec(out)?.[0] ?? `no result: ${out}`);
    return out;
  };
  await adapter.titleFor({ opening: "name it" }, exec);
  await adapter.draftFor({ promptFile: join(base["HOME"]!, "asked") }, exec);
  expect(seen[1], "the draft signs in as the title does").toBe(seen[0]);
  return seen[0]!;
}

describe("the order a Claude launch signs in by", () => {
  it("wsp's own token stands above a key from the login shell, and the two never ride together", async () => {
    const { bin } = standIn("none");
    const base = inherited(bin, { ANTHROPIC_API_KEY: RC_KEY });
    expect(buildEnv({ base, oauthToken: WSP_TOKEN })["ANTHROPIC_API_KEY"]).toBeUndefined();
    expect(signInGuard({ base, oauthToken: WSP_TOKEN })).toBe("");
    expect(launchSignsInWith(base, { oauthToken: WSP_TOKEN })).toBe(`key=none token=${WSP_TOKEN}`);
    expect(await titleSignsInWith(base, { oauthToken: WSP_TOKEN })).toBe(`key=none token=${WSP_TOKEN}`);
  });

  it("a brokered launch signs in with the turn token alone, above the vault, the person's own login and the login shell", async () => {
    const { bin } = standIn("login");
    const base = inherited(bin, { CLAUDE_CODE_OAUTH_TOKEN: RC_TOKEN, ANTHROPIC_API_KEY: RC_KEY });
    const turn = "wsp-turn-x-placeholder";
    const signIn = { oauthToken: WSP_TOKEN, brokered: { token: turn, kind: "login" as const } };
    expect(launchSignsInWith(base, signIn)).toBe(`key=none token=${turn}`);
    expect(await titleSignsInWith(base, signIn)).toBe(`key=none token=${turn}`);
    expect(launchSignsInWith(base, { brokered: { token: turn, kind: "key" } })).toBe(`key=${turn} token=none`);
  });

  it("the person's own Claude sign-in stands above a key or a token from the login shell, which are dropped", async () => {
    const { bin } = standIn("login");
    const lents: Record<string, string>[] = [{ ANTHROPIC_API_KEY: RC_KEY }, { CLAUDE_CODE_OAUTH_TOKEN: RC_TOKEN, ANTHROPIC_API_KEY: RC_KEY }];
    for (const lent of lents) {
      const base = inherited(bin, lent);
      expect(launchSignsInWith(base)).toBe("key=none token=none");
      expect(await titleSignsInWith(base)).toBe("key=none token=none");
    }
  });

  it("with neither, the key from the login shell signs the launch in", async () => {
    const { bin } = standIn("none");
    const base = inherited(bin, { ANTHROPIC_API_KEY: RC_KEY });
    expect(launchSignsInWith(base)).toBe(`key=${RC_KEY} token=none`);
    expect(await titleSignsInWith(base)).toBe(`key=${RC_KEY} token=none`);
  });

  it("with neither, a token from the login shell reaches the CLI, alone, since a key beside it would win", async () => {
    const { bin } = standIn("none");
    const base = inherited(bin, { CLAUDE_CODE_OAUTH_TOKEN: RC_TOKEN, ANTHROPIC_API_KEY: RC_KEY });
    expect(buildEnv({ base })).toMatchObject({ CLAUDE_CODE_OAUTH_TOKEN: RC_TOKEN });
    expect(buildEnv({ base })["ANTHROPIC_API_KEY"]).toBeUndefined();
    expect(launchSignsInWith(base)).toBe(`key=none token=${RC_TOKEN}`);
    expect(await titleSignsInWith(base)).toBe(`key=none token=${RC_TOKEN}`);
  });

  it("an apiKeyHelper leaves the key from the login shell in place, since Claude Code ranks that key above a helper", async () => {
    const { bin } = standIn("helper");
    const base = inherited(bin, { ANTHROPIC_API_KEY: RC_KEY });
    expect(launchSignsInWith(base)).toBe(`key=${RC_KEY} token=none`);
    expect(await titleSignsInWith(base)).toBe(`key=${RC_KEY} token=none`);
  });

  it("asks nothing where nothing was inherited", () => {
    const { bin } = standIn("none");
    expect(signInGuard({ base: inherited(bin, {}) })).toBe("");
  });
});
