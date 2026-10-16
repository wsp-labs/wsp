// SPDX-License-Identifier: AGPL-3.0-only
// A brokered launch run in a real shell, the program in place of codex a stand-in that keeps the line it was handed
// and the environment it ran under, so what is read here is the auth.json the launch itself wrote on the machine.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { BROKER_PORT, HOST_TOKEN_ENV, PLACEHOLDER_KEY_PREFIX, PLACEHOLDER_REFRESH_PREFIX, PLACEHOLDER_SIGNATURE, programWord, TURN_CLAIM } from "@wsp/protocol";
import { afterEach, describe, expect, it, vi } from "vitest";
import { writeStub } from "../../protocol/test/stub-script.js";
import { createCodexAdapter } from "../src/adapter.js";
import { catalogProbeCommand, versionProbeCommand } from "../src/catalog.js";
import { brokeredCodexEnv, buildCommand, buildEnv, codexForShell, codexLaunch, codexWord, KEY_FILE_ENV, providerFlags, PROVIDER_KEY_ENV, type CodexBroker } from "../src/command.js";
import { itemsCommand, listCommand, turnsCommand } from "../src/conversations.js";
import { draftForCommand, renameCommand, titleForCommand } from "../src/session-title.js";

const TOKEN = "AbC-123_xyz0AbC-123_xyz0AbC-123_x";
const KEY = `${PLACEHOLDER_KEY_PREFIX}${TOKEN}`;
const CLAIMS = { accountId: "11111111-2222-4333-8444-555555555555", planType: "plus", userId: "user-acme0001", fedramp: false };
const CHATGPT: CodexBroker = { mode: "chatgpt", claims: CLAIMS };
const YEAR_S = 365 * 24 * 60 * 60;
const DOOR = `http://127.0.0.1:${BROKER_PORT}`;
const TOKEN_WORD = "$" + HOST_TOKEN_ENV;
/** The launch with the overrides ahead of whatever follows, as the terminal's codex runs it. */
const ahead = (launch: { program: string }, broker: CodexBroker): string => {
  const { word, flags } = codexLaunch(launch, broker);
  return [word, ...flags].join(" ");
};

const made: string[] = [];
afterEach(() => {
  for (const dir of made.splice(0)) rmSync(dir, { recursive: true, force: true });
});

/** A folder holding the stand-in for codex, which keeps its words and its environment there. */
function standIn(): { dir: string; program: string } {
  const dir = mkdtempSync(join(tmpdir(), "codex-broker-"));
  made.push(dir);
  return { dir, program: writeStub(join(dir, "codex"), `#!/bin/sh\nprintf '%s\\n' "$@" > "${dir}/args"\nenv > "${dir}/env"\n`) };
}

/** A line run in bash under buildEnv's environment, as the machine runs it. A key turn is handed the placeholder key
 * as the runtime shapes it from the turn token. */
function run(dir: string, line: string, broker: CodexBroker, env: Record<string, string>): number {
  const shellEnv = { PATH: process.env.PATH ?? "/usr/bin:/bin", ...buildEnv({ home: join(dir, "home"), broker, base: env, ...(broker.mode === "apikey" ? { apiKey: KEY } : {}) }) };
  try {
    execFileSync("bash", ["-c", line], { env: shellEnv, stdio: "pipe", input: "" });
    return 0;
  } catch (e) {
    return (e as { status: number }).status;
  }
}

/** A turn's launch run, returning the folder it ran against. */
function launch(broker: CodexBroker, env: Record<string, string> = { [HOST_TOKEN_ENV]: TOKEN }): { dir: string; command: string; status: number } {
  const { dir, program } = standIn();
  const command = buildCommand({ cwd: dir, broker, launch: { program } });
  return { dir, command, status: run(dir, command, broker, env) };
}

/** A JWT read the way codex-cli 0.162.1 reads one (decode_jwt_payload, codex-rs/login/src/token_data.rs): three
 * parts, none empty, the payload base64url with no padding. */
function payloadOf(jwt: string): Record<string, unknown> {
  const parts = jwt.split(".");
  expect(parts).toHaveLength(3);
  for (const part of parts) expect(part).not.toBe("");
  expect(parts[2]).toBe(PLACEHOLDER_SIGNATURE);
  expect(parts[1]).toMatch(/^[A-Za-z0-9_-]+$/);
  return JSON.parse(Buffer.from(parts[1]!, "base64url").toString("utf8")) as Record<string, unknown>;
}

const loginOf = (dir: string) => JSON.parse(readFileSync(join(dir, "home", "auth.json"), "utf8")) as { auth_mode: string; OPENAI_API_KEY: string | null; tokens: Record<string, string>; last_refresh: string };
const argsOf = (dir: string): string[] => readFileSync(join(dir, "args"), "utf8").trim().split("\n");
const envOf = (dir: string): Record<string, string> =>
  Object.fromEntries(
    readFileSync(join(dir, "env"), "utf8")
      .trim()
      .split("\n")
      .map(line => [line.slice(0, line.indexOf("=")), line.slice(line.indexOf("=") + 1)]),
  );

describe("a brokered Codex launch on a ChatGPT login", () => {
  it("writes auth.json as codex parses a ChatGPT login: the real account id, a far exp, last_refresh now and the turn claim", () => {
    const before = Date.now();
    const { dir, command, status } = launch(CHATGPT);
    expect(status).toBe(0);
    const file = loginOf(dir);
    expect(file.auth_mode).toBe("chatgpt");
    expect(file.OPENAI_API_KEY).toBeNull();
    // Without the account id codex skips reload and refresh on a 401, and a pinned workspace logs the file out.
    expect(file.tokens.account_id).toBe(CLAIMS.accountId);
    expect(file.tokens.refresh_token).toBe(`${PLACEHOLDER_REFRESH_PREFIX}${TOKEN}`);
    const refreshed = Date.parse(file.last_refresh);
    expect(refreshed).toBeGreaterThanOrEqual(before - 1000);
    expect(refreshed).toBeLessThanOrEqual(Date.now());
    for (const jwt of [file.tokens.id_token!, file.tokens.access_token!]) {
      const claims = payloadOf(jwt);
      expect(claims["https://api.openai.com/auth"]).toEqual({ chatgpt_account_id: CLAIMS.accountId, chatgpt_plan_type: "plus", chatgpt_user_id: CLAIMS.userId, chatgpt_account_is_fedramp: false });
      expect(claims[TURN_CLAIM]).toBe(TOKEN);
      expect(claims.exp).toBeGreaterThanOrEqual(Math.floor(before / 1000) + YEAR_S - 1);
    }
    expect(statSync(join(dir, "home", "auth.json")).mode & 0o777).toBe(0o600);
    // The token is read off the turn's environment where the line runs, so the line names the variable.
    expect(command).toContain(`"${TOKEN_WORD}"`);
  });

  it("writes the account id alone where the host's login names no plan, user or FedRAMP flag", () => {
    const { dir } = launch({ mode: "chatgpt", claims: { accountId: CLAIMS.accountId } });
    expect(payloadOf(loginOf(dir).tokens.access_token!)["https://api.openai.com/auth"]).toEqual({ chatgpt_account_id: CLAIMS.accountId, chatgpt_account_is_fedramp: false });
  });

  it("carries a FedRAMP account's flag in the claim codex reads it from", () => {
    const { dir } = launch({ mode: "chatgpt", claims: { ...CLAIMS, fedramp: true, planType: "enterprise" } });
    expect(payloadOf(loginOf(dir).tokens.access_token!)["https://api.openai.com/auth"]).toMatchObject({ chatgpt_account_is_fedramp: true, chatgpt_plan_type: "enterprise" });
  });

  it("points both base URLs at the door under two origins, reads the login from its file, turns analytics off and sends refresh and revoke to the door", () => {
    const { dir } = launch(CHATGPT);
    const args = argsOf(dir);
    expect(args).toContain(`openai_base_url="${DOOR}/backend-api/codex"`);
    // One origin for both makes codex-cli 0.162.1 send model calls to the origin its account check answers.
    expect(args).toContain(`chatgpt_base_url="http://localhost:${BROKER_PORT}/backend-api/"`);
    // A copied config saying keyring makes codex look only there, and a machine has no secret service.
    expect(args).toContain('cli_auth_credentials_store="file"');
    expect(args).toContain("analytics.enabled=false");
    const env = envOf(dir);
    expect(env.CODEX_REFRESH_TOKEN_URL_OVERRIDE).toBe(`${DOOR}/oauth/token`);
    expect(env.CODEX_REVOKE_TOKEN_URL_OVERRIDE).toBe(`${DOOR}/oauth/revoke`);
    expect(env.CODEX_API_KEY).toBeUndefined();
    expect(env.OPENAI_API_KEY).toBeUndefined();
  });

  it("starts no codex and writes no file when the launch carries no turn token, on either login", () => {
    for (const broker of [CHATGPT, { mode: "apikey" }] as const) {
      const { dir, status } = launch(broker, {});
      expect(status).not.toBe(0);
      expect(existsSync(join(dir, "home", "auth.json"))).toBe(false);
      expect(existsSync(join(dir, "args"))).toBe(false);
    }
  });
});

describe("a brokered Codex launch on an API key", () => {
  it("writes the key file codex login --with-api-key writes, holding the placeholder key as handed, and no key in codex's environment", () => {
    const { dir, command, status } = launch({ mode: "apikey" });
    expect(status).toBe(0);
    expect(loginOf(dir)).toEqual({ auth_mode: "apikey", OPENAI_API_KEY: KEY });
    expect(command).toContain(`"$${KEY_FILE_ENV}"`);
    const env = envOf(dir);
    // app-server reads neither variable; the variable the key file was written from is gone before codex starts.
    expect(env.CODEX_API_KEY).toBeUndefined();
    expect(env.OPENAI_API_KEY).toBeUndefined();
    expect(env[KEY_FILE_ENV]).toBeUndefined();
    expect(Object.values(env)).not.toContain(KEY);
    expect(argsOf(dir)).toContain(`openai_base_url="${DOOR}/v1"`);
  });

  it("refuses a key beside the host's ChatGPT login, and a key turn handed no placeholder key", () => {
    expect(() => buildEnv({ home: "/root/.codex", apiKey: KEY, broker: CHATGPT })).toThrow("carries no key");
    // A launch with no turn token carries no key and runs on the key file the last turn left; a turn refuses to start.
    expect(buildEnv({ home: "/root/.codex", broker: { mode: "apikey" } })).not.toHaveProperty(KEY_FILE_ENV);
    const { dir, program } = standIn();
    const shellEnv = { PATH: process.env.PATH ?? "/usr/bin:/bin", ...buildEnv({ home: join(dir, "home"), broker: { mode: "apikey" }, base: { [HOST_TOKEN_ENV]: TOKEN } }) };
    expect(() => execFileSync("bash", ["-c", buildCommand({ cwd: dir, broker: { mode: "apikey" }, launch: { program } })], { env: shellEnv, stdio: "pipe" })).toThrow("without its placeholder key");
    expect(existsSync(join(dir, "home", "auth.json"))).toBe(false);
    expect(existsSync(join(dir, "args"))).toBe(false);
    expect(() => buildEnv({ home: "/root/.codex", apiKey: `${PLACEHOLDER_KEY_PREFIX}"x`, broker: { mode: "apikey" } })).toThrow("one plain word");
  });
});

describe("every other Codex launch on a brokered machine", () => {
  it("writes the placeholder login and runs codex with the broker's overrides ahead of the subcommand", () => {
    const { dir, program } = standIn();
    expect(run(dir, `${ahead({ program }, CHATGPT)} exec --json -`, CHATGPT, { [HOST_TOKEN_ENV]: TOKEN })).toBe(0);
    expect(payloadOf(loginOf(dir).tokens.access_token!)[TURN_CLAIM]).toBe(TOKEN);
    const args = argsOf(dir);
    expect(args.slice(-3)).toEqual(["exec", "--json", "-"]);
    for (const flag of [`openai_base_url="${DOOR}/backend-api/codex"`, `chatgpt_base_url="http://localhost:${BROKER_PORT}/backend-api/"`, 'cli_auth_credentials_store="file"', "analytics.enabled=false"]) {
      expect(args.indexOf(flag)).toBeGreaterThan(-1);
      expect(args.indexOf(flag)).toBeLessThan(args.indexOf("exec"));
    }
    expect(envOf(dir).CODEX_REFRESH_TOKEN_URL_OVERRIDE).toBe(`${DOOR}/oauth/token`);
  });

  it("with no turn token, writes nothing and still runs at the door on the login the last turn left", () => {
    const { dir, program } = standIn();
    expect(run(dir, `${ahead({ program }, CHATGPT)} --version`, CHATGPT, {})).toBe(0);
    expect(existsSync(join(dir, "home", "auth.json"))).toBe(false);
    expect(argsOf(dir)).toContain(`openai_base_url="${DOOR}/backend-api/codex"`);
  });

  it("on an API key, writes the key file and leaves the key out of codex's environment", () => {
    const { dir, program } = standIn();
    const broker: CodexBroker = { mode: "apikey" };
    expect(run(dir, `${ahead({ program }, broker)} --version`, broker, { [HOST_TOKEN_ENV]: TOKEN })).toBe(0);
    expect(loginOf(dir)).toEqual({ auth_mode: "apikey", OPENAI_API_KEY: KEY });
    expect(envOf(dir)[KEY_FILE_ENV]).toBeUndefined();
  });

  it("is the plain program word on today's road", () => {
    expect(codexWord(undefined, undefined)).toBe(programWord("codex", undefined));
  });

  it("a probe, a rename or a conversation read reads no login, not even the one the last turn left, and runs no plugin sync", () => {
    // Outside a turn the last turn's token is dead: on codex-cli 0.163.0 a probe on that login made 11 calls the door
    // refused 403 at every wake, and one with neither the login nor plugins made none.
    for (const [broker, env] of [[CHATGPT, {}], [CHATGPT, { [HOST_TOKEN_ENV]: TOKEN }], [{ mode: "apikey" }, {}]] as const) {
      const { dir, program } = standIn();
      mkdirSync(join(dir, "home"));
      writeFileSync(join(dir, "home", "auth.json"), "last turn's");
      expect(run(dir, `${codexWord({ program }, broker)} app-server`, broker, env)).toBe(0);
      const args = argsOf(dir);
      expect(args).toContain('cli_auth_credentials_store="ephemeral"');
      expect(args).not.toContain('cli_auth_credentials_store="file"');
      expect(args).toContain("features.plugins=false");
      expect(args.at(-1)).toBe("app-server");
      expect(args).toContain(broker.mode === "chatgpt" ? `chatgpt_base_url="http://localhost:${BROKER_PORT}/backend-api/"` : `openai_base_url="${DOOR}/v1"`);
      expect(readFileSync(join(dir, "home", "auth.json"), "utf8")).toBe("last turn's");
      expect(envOf(dir)[KEY_FILE_ENV]).toBeUndefined();
    }
  });

  it("a brokered probe asks nothing of the sign-in, which the host holds", () => {
    expect(catalogProbeCommand({ broker: CHATGPT })).not.toContain("account/read");
    expect(catalogProbeCommand()).toContain("account/read");
  });

  it("a title hands codex the broker's overrides after exec, beside its own, where codex keeps both", () => {
    // codex-cli 0.162.1 drops every root -c once `exec` names one of its own: a title with the overrides ahead of exec
    // went to chatgpt.com (measured).
    const { dir, program } = standIn();
    expect(run(dir, titleForCommand({ prompt: "name this", launch: { program }, broker: CHATGPT }), CHATGPT, { [HOST_TOKEN_ENV]: TOKEN })).toBe(0);
    const args = argsOf(dir);
    expect(args[0]).toBe("exec");
    expect(args).toContain(`openai_base_url="${DOOR}/backend-api/codex"`);
    expect(args).toContain('cli_auth_credentials_store="file"');
    expect(args).toContain('sandbox_mode="read-only"');
    expect(payloadOf(loginOf(dir).tokens.access_token!)[TURN_CLAIM]).toBe(TOKEN);
  });

  it("titles, drafts, renames, probes and conversation reads each run codex through it", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(1_791_900_000_000);
    try {
      const question = codexLaunch(undefined, CHATGPT);
      for (const line of [titleForCommand({ prompt: "name this", broker: CHATGPT }), draftForCommand({ promptFile: "/tmp/acme-draft", broker: CHATGPT })]) {
        expect(line).toContain([question.word, "exec", ...question.flags].join(" "));
      }
      const word = codexWord(undefined, CHATGPT);
      const lines = [
        renameCommand({ threadId: "t1", title: "acme", broker: CHATGPT }),
        catalogProbeCommand({ broker: CHATGPT }),
        versionProbeCommand({ broker: CHATGPT }),
        listCommand({ cwds: ["/root/acme"], broker: CHATGPT }),
        turnsCommand({ threadId: "t1", probe: true, broker: CHATGPT }),
        itemsCommand({ threadId: "t1", broker: CHATGPT }),
      ];
      for (const line of lines) expect(line).toContain(word);
      expect(titleForCommand({ prompt: "name this" })).not.toContain("cli_auth_credentials_store");
    } finally {
      now.mockRestore();
    }
  });

  it("the adapter hands a title its brokered word and the turn's own environment", async () => {
    const asked: { command: string; env?: Readonly<Record<string, string>> }[] = [];
    const adapter = createCodexAdapter({ exec: () => Promise.reject(new Error("no turn here")) as never, home: "/root/.codex", login: "codex login", broker: CHATGPT, baseEnv: { [HOST_TOKEN_ENV]: TOKEN } });
    await adapter
      .titleFor?.({ opening: "hi", reply: "hello" }, (command, env) => {
        asked.push({ command, ...(env !== undefined ? { env } : {}) });
        return Promise.resolve("");
      })
      .catch(() => undefined);
    expect(asked[0]!.command).toContain('cli_auth_credentials_store=');
    expect(asked[0]!.env).toEqual(adapter.env);
    expect(asked[0]!.env?.CODEX_REFRESH_TOKEN_URL_OVERRIDE).toBe(`${DOOR}/oauth/token`);
  });
});

describe("the person's own providers on a brokered launch", () => {
  it("moves an entry to the door and gives the credential field it holds the placeholder key, never taking one away", () => {
    // codex merges a -c table into the copied entry (measured on 0.162.1): an env_key beside the copy's auth command
    // left both and codex refused the whole config, so each entry keeps its own credential field.
    expect(providerFlags("acme", { base_url: "https://gw.acme.dev/v1", auth: { command: "acme-token" } }, "apikey").flags).toEqual([
      `-c model_providers.acme.base_url='"${DOOR}/v1"'`,
      `-c model_providers.acme.auth.command='"sh"'`,
      `-c model_providers.acme.auth.args='${JSON.stringify(["-c", `printf %s "${PLACEHOLDER_KEY_PREFIX}${TOKEN_WORD}"`])}'`,
    ]);
    expect(providerFlags("acme", { base_url: "https://gw.acme.dev/v1", experimental_bearer_token: "tok-1" }, "apikey").flags).toContain(`-c model_providers.acme.experimental_bearer_token='"${PLACEHOLDER_KEY_PREFIX}'"${TOKEN_WORD}"'"'`);
    expect(providerFlags("acme", { base_url: "https://gw.acme.dev/v1", env_key: "ACME_KEY" }, "apikey").flags).toContain(`-c model_providers.acme.env_key='"${PROVIDER_KEY_ENV}"'`);
  });

  it("each credential field, run in a shell, holds the placeholder key, and codex's words keep no secret", () => {
    const providers = {
      by_auth: { base_url: "https://gw.acme.dev/v1", auth: { command: "acme-token" }, http_headers: { "X-Acme-Key": "s3cret" } },
      by_bearer: { base_url: "https://gw.acme.dev/v1", experimental_bearer_token: "tok-1" },
      by_key: { base_url: "https://gw.acme.dev/v1", env_key: "ACME_KEY" },
    };
    const { dir } = launch({ ...CHATGPT, providers });
    const args = argsOf(dir);
    expect(args).toContain(`model_providers.by_bearer.experimental_bearer_token="${KEY}"`);
    expect(args).toContain(`model_providers.by_auth.http_headers={"X-Acme-Key"=""}`);
    expect(args.join("\n")).not.toContain("s3cret");
    expect(args.join("\n")).not.toContain("tok-1");
    expect(envOf(dir)[PROVIDER_KEY_ENV]).toBe(KEY);
    // The auth command codex runs prints the key off the environment it runs in.
    const flag = args.find(a => a.startsWith("model_providers.by_auth.auth.args="))!;
    const words = JSON.parse(flag.slice(flag.indexOf("=") + 1)) as string[];
    expect(execFileSync("sh", words, { env: { [HOST_TOKEN_ENV]: TOKEN } }).toString()).toBe(KEY);
  });

  it("names an entry codex read as Azure by its address azure, and leaves a name that already reads azure", () => {
    expect(providerFlags("work", { name: "Work", base_url: "https://acme.openai.azure.com/openai", env_key: "AZURE_OPENAI_API_KEY" }, "apikey").flags).toContain(`-c model_providers.work.name='"azure"'`);
    expect(providerFlags("work", { name: "Azure", base_url: "https://example.com" }, "apikey").flags.join(" ")).not.toContain(".name=");
  });

  it("sends an entry that asks for OpenAI's own sign-in down the placeholder login, naming no key", () => {
    const { flags, keyed } = providerFlags("corp", { base_url: "https://gw.acme.dev/backend-api/codex", requires_openai_auth: true }, "chatgpt");
    expect(flags).toEqual([`-c model_providers.corp.base_url='"${DOOR}/backend-api/codex"'`]);
    expect(keyed).toBe(false);
  });

  it("leaves an AWS-signed entry on today's road, and exports no placeholder key when no entry reads one", () => {
    const { dir, command } = launch({ mode: "apikey", providers: { bedrock: { name: "Bedrock", aws: { region: "us-east-1" } } } });
    expect(command).not.toContain("model_providers");
    expect(envOf(dir)[PROVIDER_KEY_ENV]).toBeUndefined();
  });

  it("refuses a provider id that is not one plain word of a config key", () => {
    expect(() => buildCommand({ broker: { mode: "apikey", providers: { "acme.gw": { base_url: "https://gw.acme.dev/v1" } } } })).toThrow("one plain word");
  });
});

describe("codex typed in a terminal on a brokered machine", () => {
  /** `line` typed in a shell whose PATH finds codex's program for the shell first and the stand-in for codex after it,
   * under the environment the shell is opened with. */
  function typed(broker: CodexBroker, line: string, env: Record<string, string> = { [HOST_TOKEN_ENV]: TOKEN }): { dir: string; status: number } {
    const { dir } = standIn();
    const bin = join(dir, "bin");
    mkdirSync(bin);
    writeStub(join(bin, "codex"), codexForShell(broker, bin));
    const apiKey = broker.mode === "apikey" && env[HOST_TOKEN_ENV] !== undefined ? KEY : undefined;
    const shellEnv = { CODEX_HOME: join(dir, "home"), ...env, PATH: `${bin}:${dir}:${process.env.PATH ?? "/usr/bin:/bin"}`, ...brokeredCodexEnv(broker, apiKey) };
    try {
      execFileSync("sh", ["-c", line], { env: shellEnv, stdio: "pipe", input: "" });
      return { dir, status: 0 };
    } catch (e) {
      return { dir, status: (e as { status: number }).status };
    }
  }

  it("runs the real codex at the door on the shell's token, with what was typed after the overrides", () => {
    const { dir, status } = typed(CHATGPT, 'codex exec "say ok"');
    expect(status).toBe(0);
    const args = argsOf(dir);
    expect(args.slice(-2)).toEqual(["exec", "say ok"]);
    expect(args).toContain(`openai_base_url="${DOOR}/backend-api/codex"`);
    expect(args).toContain(`chatgpt_base_url="http://localhost:${BROKER_PORT}/backend-api/"`);
    expect(args).toContain('cli_auth_credentials_store="file"');
    expect(payloadOf(loginOf(dir).tokens.access_token!)[TURN_CLAIM]).toBe(TOKEN);
    // The real codex is found past the program's own folder, which it takes off the PATH codex inherits.
    expect(envOf(dir).PATH?.split(":")).not.toContain(join(dir, "bin"));
    expect(envOf(dir).CODEX_REFRESH_TOKEN_URL_OVERRIDE).toBe(`${DOOR}/oauth/token`);
  });

  it("writes the key file from the shell's placeholder key and leaves the key out of codex's environment", () => {
    const { dir, status } = typed({ mode: "apikey" }, "codex --version");
    expect(status).toBe(0);
    expect(loginOf(dir)).toEqual({ auth_mode: "apikey", OPENAI_API_KEY: KEY });
    expect(envOf(dir)[KEY_FILE_ENV]).toBeUndefined();
    expect(argsOf(dir)).toContain(`openai_base_url="${DOOR}/v1"`);
  });

  it("writes the login where codex itself looks, in a shell that names no CODEX_HOME", () => {
    const home = mkdtempSync(join(tmpdir(), "codex-home-"));
    made.push(home);
    const { dir, status } = typed(CHATGPT, 'codex exec "say ok"', { [HOST_TOKEN_ENV]: TOKEN, HOME: home, CODEX_HOME: "" });
    expect(status).toBe(0);
    expect(payloadOf((JSON.parse(readFileSync(join(home, ".codex", "auth.json"), "utf8")) as { tokens: Record<string, string> }).tokens.access_token!)[TURN_CLAIM]).toBe(TOKEN);
    expect(envOf(dir).CODEX_HOME).toBe(join(home, ".codex"));
  });

  it("runs codex on the login file already there, writing none, in a shell that carries no token", () => {
    const { dir, status } = typed(CHATGPT, "codex", {});
    expect(status).toBe(0);
    expect(existsSync(join(dir, "home", "auth.json"))).toBe(false);
    expect(argsOf(dir)).toContain('cli_auth_credentials_store="file"');
  });
});
