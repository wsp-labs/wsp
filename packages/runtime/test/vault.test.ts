// SPDX-License-Identifier: AGPL-3.0-only
// The vault at a turn's launch: the variables the wsp home's .env holds reach
// every adapter through the runtime, read at each launch and never copied, so
// a token minted after the host started is in the next turn. Nothing of it is
// on a machine.
import { execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { LocalBackend } from "@wsp/engine";
import type { HarnessAdapterContext, HarnessAdapterFactory, LocalWiring, RuntimeOptions } from "../src/runtime.js";
import type { ServersActs } from "../src/agents-read.js";
import { createRuntime } from "../src/runtime.js";
import { secretsOf } from "../src/adapters.js";
import { localExecStream } from "../src/local-exec.js";
import { memoryStore } from "../src/store.js";
import { newPlaceKeyPair } from "../src/places.js";
import { serveRuntime } from "../src/serve.js";
import { stubBackend, copyingFake, createOn, tempRepo, testPlatform } from "./stub-backend.js";
import { report, wiring } from "./place-join.js";
import { ctx as places, sockets, code, join as joinPlace, forks, asRoot, KEEPS_NO_IMAGE, ROOT_LOGIN } from "./places-fixture.js";
import { until } from "./until.js";
import { writeStub } from "../../protocol/test/stub-script.js";
import { HOST_TOKEN_ENV, THREAD_ENV, TURN_TOKEN_ENV, brokeredAgentRefusal, type BrokerStatus, type TurnResult } from "@wsp/protocol";
import { buildEnv } from "@wsp/adapter-claude";

/** A folder with a first commit in it, which is what a project on this computer is made of. */
function repoAt(): string {
  const at = tempRepo();
  execFileSync("git", ["-C", at, "commit", "-q", "--allow-empty", "-m", "first"], { env: { ...process.env, GIT_AUTHOR_NAME: "t", GIT_AUTHOR_EMAIL: "t@example.com", GIT_COMMITTER_NAME: "t", GIT_COMMITTER_EMAIL: "t@example.com" } });
  return at;
}

const TOKEN = "sk-ant-oat01-TESTONLY";
const OPENAI = "sk-x-fake-openai";
/** A vault holding a sign-in for each agent, a GitHub token and two values MCP servers read by name. */
const HELD = { CLAUDE_CODE_OAUTH_TOKEN: TOKEN, OPENAI_API_KEY: OPENAI, GH_TOKEN: "ghp_TESTONLY", WSP_MCP_LINEAR_AUTHORIZATION: "lin_api_TESTONLY", NOTION_TOKEN: "ntn_TESTONLY" };

/** An adapter that reports every context it was built with and ends its turn at once. */
function recording(): { factory: HarnessAdapterFactory; contexts: HarnessAdapterContext[] } {
  const contexts: HarnessAdapterContext[] = [];
  const factory: HarnessAdapterFactory = ctx => {
    contexts.push(ctx);
    return {
      steers: false,
      start: ({ onEvent }) => {
        const sessionId = randomUUID();
        const result: TurnResult = { status: "completed", text: "ok" };
        onEvent({ type: "session.start", sessionId });
        onEvent({ type: "turn.done", sessionId, result });
        onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
        return { localId: sessionId, finished: Promise.resolve(result), interrupt: async () => {} };
      },
    };
  };
  return { factory, contexts };
}

describe("the vault a turn launches with", () => {
  it("reaches a turn on a cloud fork, read at the launch, so a token minted after the host started is in the next turn", async () => {
    const { factory, contexts } = recording();
    let held: Record<string, string> = {};
    const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory }, vault: () => held });
    const ws = await createOn(rt, { golden: "snap_g", name: "x" });
    await (await rt.sessions.start(ws.id, { prompt: "one" })).finished;
    expect(contexts.at(-1)!.vault).toEqual({});
    held = { CLAUDE_CODE_OAUTH_TOKEN: TOKEN };
    await (await rt.sessions.start(ws.id, { prompt: "two" })).finished;
    expect(contexts.at(-1)!.vault).toEqual({ CLAUDE_CODE_OAUTH_TOKEN: TOKEN });
  });

  it("sets every name an MCP server's definition reads in the turn's environment, and a row's token only through the row", async () => {
    const { factory, contexts } = recording();
    const held = { CLAUDE_CODE_OAUTH_TOKEN: TOKEN, WSP_MCP_LINEAR_AUTHORIZATION: "lin_api_TESTONLY", NOTION_TOKEN: "ntn_TESTONLY" };
    const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory }, vault: () => held });
    const ws = await createOn(rt, { golden: "snap_g", name: "x" });
    await (await rt.sessions.start(ws.id, { prompt: "one" })).finished;
    const env = contexts.find(c => TURN_TOKEN_ENV in c.env)!.env;
    // Every other road an adapter is built for starts no agent, so no server value rides it.
    for (const c of contexts.filter(c => !(TURN_TOKEN_ENV in c.env))) expect(c.env["WSP_MCP_LINEAR_AUTHORIZATION"]).toBeUndefined();
    expect(env).toMatchObject({ WSP_MCP_LINEAR_AUTHORIZATION: "lin_api_TESTONLY", NOTION_TOKEN: "ntn_TESTONLY" });
    expect(env["CLAUDE_CODE_OAUTH_TOKEN"]).toBeUndefined();
    // Claude Code's own environment is the one it expands ${WSP_MCP_LINEAR_AUTHORIZATION} out of.
    expect(buildEnv({ base: env })).toMatchObject({ WSP_MCP_LINEAR_AUTHORIZATION: "lin_api_TESTONLY", NOTION_TOKEN: "ntn_TESTONLY" });
  });

  it("hands a server added from a provider's workspace the store that workspace's Claude Code turn reads its servers under", async () => {
    const { factory, contexts } = recording();
    const handed: (string | undefined)[] = [];
    const serversActs: ServersActs = {
      add: async on => (handed.push(on.kind === "here" ? "here" : on.stores?.["claude"]), { file: "" }),
      remove: async () => ({ file: "" }),
      toggle: async () => ({ file: "" }),
    };
    const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory }, vault: () => ({ CLAUDE_CODE_OAUTH_TOKEN: TOKEN }), serversActs });
    const ws = await createOn(rt, { golden: "snap_g", name: "x" });
    await rt.agents.serversAdd({ workspaceId: ws.id }, { agent: "claude", name: "acme", command: "acme-mcp" });
    await (await rt.sessions.start(ws.id, { prompt: "one" })).finished;
    const env = contexts.find(c => TURN_TOKEN_ENV in c.env)!.env;
    expect(env["CLAUDE_CONFIG_DIR"]).toBeDefined();
    expect(handed).toEqual([env["CLAUDE_CONFIG_DIR"]]);
  });

  it("is empty for a host that wired none, so a turn carries nothing of one", async () => {
    const { factory, contexts } = recording();
    const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory } });
    const ws = await createOn(rt, { golden: "snap_g", name: "x" });
    await (await rt.sessions.start(ws.id, { prompt: "one" })).finished;
    for (const ctx of contexts) expect(ctx.vault).toEqual({});
  });

  it("reads a login on the computer the app runs on off that agent's own store there, and never on a fork", async () => {
    const root = mkdtempSync(join(tmpdir(), "wsp-login-"));
    const codexHome = join(root, ".codex");
    mkdirSync(codexHome, { recursive: true });
    const { factory, contexts } = recording();
    const local: LocalWiring = {
      backend: new LocalBackend({ root }),
      execStream: o => localExecStream({ root, runDir: join(root, "runs"), ...o }),
      home: id => join(root, `.${id}`),
      homeDir: root,
      rootsPath: join(root, "roots"),
      env: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
      platform: testPlatform(),
      copier: copyingFake(),
    };
    const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory }, local });
    const folder = repoAt();
    const project = await rt.projects.add({ source: folder });
    const here = await rt.workspaces.create({ project: project.id, name: "here" });
    await (await rt.sessions.start(here.id, { prompt: "one" })).finished;
    // Nothing under the home Codex reads on this computer, so a key the vault holds is the turn's to run on.
    expect(contexts.at(-1)!.loginStands("codex")).toBe(false);

    writeFileSync(join(codexHome, "auth.json"), "{}");
    await (await rt.sessions.start(here.id, { prompt: "two" })).finished;
    expect(contexts.at(-1)!.loginStands("codex")).toBe(true);
    // Claude Code keeps no shared login file at all, so there is never one of its own to stand here.
    expect(contexts.at(-1)!.loginStands("claude")).toBe(false);

    // An image never carries a sign-in, so a fork at a provider always takes the vault's key.
    const cloud = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory } });
    const fork = await createOn(cloud, { golden: "snap_g", name: "x" });
    await (await cloud.sessions.start(fork.id, { prompt: "one" })).finished;
    expect(contexts.at(-1)!.loginStands("codex")).toBe(false);
    for (const at of [root, folder]) rmSync(at, { recursive: true, force: true });
  });

  it("hands each agent the variables its own catalog row declares, from one record, and its token in place of its key", () => {
    const record = { CLAUDE_CODE_OAUTH_TOKEN: TOKEN, ANTHROPIC_API_KEY: "sk-ant-x", OPENAI_API_KEY: OPENAI, SOLARI_API_KEY: "slr_live_x" };
    // An API key beside the token would win inside Claude Code and bill the key on every turn, so the vault hands
    // the token alone where it holds one.
    expect(secretsOf(record, "claude")).toEqual({ oauthToken: TOKEN });
    // The key comes back with the variable it travels under, since the sentence for a key the provider turns down
    // has to name it.
    expect(secretsOf(record, "codex")).toEqual({ apiKey: OPENAI, keyEnv: "OPENAI_API_KEY" });
    expect(secretsOf({ ANTHROPIC_API_KEY: "sk-ant-x" }, "claude")).toEqual({ apiKey: "sk-ant-x", keyEnv: "ANTHROPIC_API_KEY" });
    expect(secretsOf({}, "claude")).toEqual({});
    expect(secretsOf({}, "codex")).toEqual({});
  });

  it("hands a turn on a provider fork every name the vault holds, GH_TOKEN and the servers' values among them, while the broker is not ready", async () => {
    const env = async (broker?: RuntimeOptions["broker"]) => {
      const { factory, contexts } = recording();
      const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory }, vault: () => HELD, ...(broker !== undefined ? { broker } : {}) });
      const ws = await createOn(rt, { golden: "snap_g", name: "x" });
      await (await rt.sessions.start(ws.id, { prompt: "one" })).finished;
      const turn = contexts.find(c => TURN_TOKEN_ENV in c.env)!;
      const { [TURN_TOKEN_ENV]: _turn, [THREAD_ENV]: _thread, [HOST_TOKEN_ENV]: _host, ...rest } = turn.env;
      return { env: rest, vault: turn.vault, brokered: turn.brokered };
    };
    const today = await env();
    expect(today.brokered).toBeUndefined();
    expect(today.env).toMatchObject({ GH_TOKEN: "ghp_TESTONLY", WSP_MCP_LINEAR_AUTHORIZATION: "lin_api_TESTONLY" });
    expect(today.vault).toEqual(HELD);
    expect(await env({ ready: () => false, codexClaims: async () => undefined })).toEqual(today);
  });

  it("holds the key back where a login of that agent's own already stands, since the key would outrank it", () => {
    const record = { CLAUDE_CODE_OAUTH_TOKEN: TOKEN, OPENAI_API_KEY: OPENAI };
    // Codex reads a key in its environment ahead of the login under its home, so a key handed on a computer the
    // person signed Codex in on would bill the key and leave that sign-in unused.
    expect(secretsOf(record, "codex", true)).toEqual({});
    expect(secretsOf(record, "codex", false)).toEqual({ apiKey: OPENAI, keyEnv: "OPENAI_API_KEY" });
    // Claude Code ranks its token ahead of its own login too, so a box it signed in on there gets neither.
    expect(secretsOf(record, "claude", true)).toEqual({});
    expect(secretsOf(record, "claude", false)).toEqual({ oauthToken: TOKEN });
  });
});

describe("a turn on a machine whose credentials the host keeps", () => {
  const roots: string[] = [];
  afterEach(() => {
    for (const at of roots.splice(0)) rmSync(at, { recursive: true, force: true });
  });
  /** This computer, whose gh answers a brokered start's reads of the project's repo as GitHub does. */
  const here = (): { local: LocalWiring } => {
    const root = mkdtempSync(join(tmpdir(), "wsp-brokered-"));
    roots.push(root);
    mkdirSync(join(root, "bin"));
    writeStub(join(root, "bin", "gh"), `#!/bin/sh\ncase "$2" in\n  repos/*/commits/*) echo 1d2c3b4a5f6e7d8c9b0a1d2c3b4a5f6e7d8c9b0a ;;\n  repos/*) printf '4242\\tacme\\tweb\\n' ;;\n  *) exit 1 ;;\nesac\n`);
    const local: LocalWiring = {
      backend: new LocalBackend({ root, env: { ...process.env, PATH: `${join(root, "bin")}:${process.env["PATH"] ?? ""}` } }),
      execStream: o => localExecStream({ root, runDir: join(root, "runs"), ...o }),
      home: id => join(root, `.${id}`),
      homeDir: root,
      rootsPath: join(root, "roots"),
      env: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
      platform: testPlatform(),
      copier: copyingFake(),
    };
    return { local };
  };
  /** A broker that is up and adds Claude Code's token and Codex's key, unless told what it holds. */
  const ready = (status: BrokerStatus = { claude: "token", codex: "key" }): RuntimeOptions["broker"] => ({ ready: () => true, codexClaims: async () => undefined, status: async () => status });
  const READY = ready();
  const turnOf = async (rt: ReturnType<typeof createRuntime>, contexts: HarnessAdapterContext[], create: Parameters<typeof createOn>[1] = { golden: "snap_g", name: "x" }) => {
    const ws = await createOn(rt, create);
    await (await rt.sessions.start(ws.id, { prompt: "one", harness: "claude" })).finished;
    return contexts.find(c => TURN_TOKEN_ENV in c.env)!;
  };
  /** git's configuration as the environment's GIT_CONFIG_COUNT pairs hand it, key and value in order. */
  const gitPairs = (env: Readonly<Record<string, string>>): [string, string][] =>
    Array.from({ length: Number(env["GIT_CONFIG_COUNT"] ?? 0) }, (_, i) => [env[`GIT_CONFIG_KEY_${i}`]!, env[`GIT_CONFIG_VALUE_${i}`]!]);

  it("holds none of the vault's names or values and no server value, and the turn token in each place 01 lists", async () => {
    const { factory, contexts } = recording();
    const backend = stubBackend();
    const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: factory }, vault: () => HELD, broker: READY, ...here() });
    const turn = await turnOf(rt, contexts);
    const token = turn.env[HOST_TOKEN_ENV]!;
    expect(token).toMatch(/^\S{16,}$/);
    // Nothing the vault holds reaches any road an adapter is built for, under any name or inside any value.
    for (const c of contexts) {
      for (const [name, value] of [...Object.entries(c.env), ...Object.entries(c.vault)]) for (const held of Object.values(HELD)) expect(value, name).not.toContain(held);
    }
    // GH_TOKEN is the one name the vault and the turn share, and it carries the turn token.
    expect(Object.keys(turn.env).filter(name => name in HELD)).toEqual(["GH_TOKEN"]);
    expect(turn.env).toMatchObject({ [HOST_TOKEN_ENV]: token, GH_TOKEN: token, GH_CONFIG_DIR: "/root/.wsp/gh", GH_TELEMETRY: "false", GIT_TERMINAL_PROMPT: "0", CARGO_NET_GIT_FETCH_WITH_CLI: "true" });
    expect(gitPairs(turn.env)).toEqual([
      ["url.http://github.com/.insteadOf", "https://github.com/"],
      ["url.http://github.com/.insteadOf", "git@github.com:"],
      ["url.http://github.com/.insteadOf", "ssh://git@github.com/"],
      ["url.http://github.com/.insteadOf", "ssh://git@ssh.github.com:443/"],
      ["http.http://github.com/.proxy", `http://wsp:${token}@127.0.0.1:7071`],
      ["http.proxyAuthMethod", "basic"],
      ["credential.helper", ""],
    ]);
    // Each agent is handed the turn token as the kind of credential the broker says it adds: Claude Code's as the kind
    // its adapter writes the placeholder of, Codex's key as a key in the vault.
    expect(turn.brokered).toEqual({ token, kind: "token" });
    expect(secretsOf(turn.vault, "claude")).toEqual({});
    expect(secretsOf(turn.vault, "codex")).toEqual({ apiKey: `sk-wsp-${token}`, keyEnv: "OPENAI_API_KEY" });
    // Claude Code's own environment keeps gh's and git's variables, which it strips nothing of.
    expect(buildEnv({ base: turn.env })).toMatchObject({ GH_TOKEN: token, GIT_CONFIG_COUNT: "7", GH_CONFIG_DIR: "/root/.wsp/gh" });
    // The launch text writes the config naming the gh door, since gh reads the socket from that key alone.
    const launch = turn.execStream("claude -p hi", { env: turn.env });
    // The stub never says the run is up, so the launch ends refused; it is read from the start so nothing goes unheard.
    const read = (async () => {
      for await (const _line of launch.lines);
    })().catch(() => undefined);
    const machine = backend.machines[0]!;
    // The run's script goes up base64 through the guest's file road, so it is read back the way the guest decodes it.
    const scripts = (): string[] => machine.execLog.flatMap(line => [...line.matchAll(/printf %s '([A-Za-z0-9+/=]+)' \| base64 -d > '[^']*\.sh'/g)].map(m => Buffer.from(m[1]!, "base64").toString("utf8")));
    await until(async () => scripts().some(script => script.includes("claude -p hi")));
    const launched = scripts().find(script => script.includes("claude -p hi"))!;
    expect(launched).toContain("mkdir -p /root/.wsp/gh && printf '%s\\n' 'http_unix_socket: /root/.wsp/broker.sock' > /root/.wsp/gh/config.yml && claude -p hi");
    // The provider's exec text carries the launch's environment on such a machine, and none of the vault with it.
    for (const text of [...machine.execLog, ...scripts()]) for (const held of Object.values(HELD)) expect(text).not.toContain(held);
    await read;
    await launch.exited.catch(() => undefined);
  });

  it("hands Claude Code the kind the broker says it adds, whatever names the vault holds", async () => {
    const cases: [BrokerStatus | undefined, "token" | "key" | "gateway"][] = [
      [{ claude: "login" }, "token"],
      [{ claude: "token" }, "token"],
      [{ claude: "key" }, "key"],
      [{ claude: "gateway" }, "gateway"],
      [{}, "token"],
      [undefined, "token"],
    ];
    for (const [status, kind] of cases) {
      const { factory, contexts } = recording();
      // The vault holds a key alone, which the kind is never read off once the broker is up.
      const broker = status === undefined ? { ready: () => true, codexClaims: async () => undefined } : ready(status);
      const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory }, vault: () => ({ ANTHROPIC_API_KEY: "sk-ant-api03-TESTONLY", OPENAI_API_KEY: OPENAI }), broker, ...here() });
      const turn = await turnOf(rt, contexts);
      const token = turn.env[HOST_TOKEN_ENV]!;
      expect(turn.brokered, JSON.stringify(status)).toEqual({ token, kind });
      // Codex on the host's ChatGPT login, or with nothing named, gets no key in the vault.
      expect(secretsOf(turn.vault, "codex")).toEqual({});
    }
  });

  it("refuses a launch of an agent the broker adds no key for, before anything runs", async () => {
    const { factory, contexts } = recording();
    const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory, opencode: factory }, vault: () => HELD, broker: READY, ...here() });
    const ws = await createOn(rt, { golden: "snap_g", name: "x" });
    await expect(rt.sessions.start(ws.id, { prompt: "one", harness: "opencode" })).rejects.toThrow(brokeredAgentRefusal("OpenCode", ["Claude Code", "Codex"]).message);
    expect(contexts.filter(c => TURN_TOKEN_ENV in c.env)).toEqual([]);
    expect(brokeredAgentRefusal("OpenCode", ["Claude Code", "Codex"]).message).toBe("OpenCode does not run on a cloud machine yet: wsp keeps no key there. Use Claude Code or Codex.");
  });

  it("launches an aside with a token of its own as the placeholder and no server value", async () => {
    const contexts: HarnessAdapterContext[] = [];
    const factory: HarnessAdapterFactory = ctx => {
      contexts.push(ctx);
      return {
        steers: false,
        start: ({ onEvent }) => {
          const result: TurnResult = { status: "completed", text: "ok" };
          onEvent({ type: "session.start", sessionId: "s-aside", cwd: "/root/x" });
          onEvent({ type: "turn.done", sessionId: "s-aside", result });
          onEvent({ type: "session.end", sessionId: "s-aside", exitCode: 0, sawResult: true });
          return { localId: "s-aside", finished: Promise.resolve(result), interrupt: async () => {} };
        },
        aside: async () => ({ text: "an answer" }),
      };
    };
    const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory }, vault: () => HELD, broker: READY, ...here() });
    const ws = await createOn(rt, { golden: "snap_g", name: "x" });
    const turn = await rt.sessions.start(ws.id, { prompt: "one", harness: "claude" });
    await turn.finished;
    const before = contexts.length;
    expect(await rt.sessions.aside(turn.id, "what did I ask?")).toEqual({ text: "an answer" });
    const aside = contexts.slice(before).find(c => HOST_TOKEN_ENV in c.env)!;
    const token = aside.env[HOST_TOKEN_ENV]!;
    expect(aside.brokered).toEqual({ token, kind: "token" });
    expect(aside.env).toMatchObject({ GH_TOKEN: token, GH_CONFIG_DIR: "/root/.wsp/gh" });
    for (const c of contexts.slice(before)) {
      for (const [name, value] of [...Object.entries(c.env), ...Object.entries(c.vault)]) for (const held of Object.values(HELD)) expect(value, name).not.toContain(held);
    }
  });

  it("brokers a fork at a provider the host is not wired to, the record naming that provider as its place", async () => {
    const solari = stubBackend();
    const box = stubBackend();
    const { factory, contexts } = recording();
    const rt = createRuntime({
      backend: solari,
      store: memoryStore(),
      adapters: { claude: factory },
      vault: () => HELD,
      broker: READY,
      ...here(),
      places: { wired: "solari", backend: place => ({ solari, box })[place], list: () => ["solari", "box"] },
      placeLinks: wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 }),
    });
    places.runtime = rt;
    places.srv = await serveRuntime(rt, { port: 0, authToken: "host-token", devices: rt.devices });
    const turn = await turnOf(rt, contexts, { golden: "snap_g", name: "x", on: "box" });
    expect((await rt.workspaces.get(turn.workspaceId)).place).toBe("box");
    expect(turn.env["GH_TOKEN"]).toBe(turn.env[HOST_TOKEN_ENV]);
    expect(turn.env["WSP_MCP_LINEAR_AUTHORIZATION"]).toBeUndefined();
  });

  it("leaves a fork on a computer the person joined on today's road, the broker ready or not", async () => {
    const run = async (broker?: RuntimeOptions["broker"]) => {
      const { factory, contexts } = recording();
      const hostKey = newPlaceKeyPair();
      const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: factory }, vault: () => HELD, placeLinks: wiring(hostKey), ...(broker !== undefined ? { broker, ...here() } : {}) });
      places.runtime = rt;
      places.srv = await serveRuntime(rt, { port: 0, authToken: "host-token", devices: rt.devices });
      const { client } = await joinPlace(hostKey, { code: await code(), report: report("srv", { agents: ["claude", "codex"], login: ROOT_LOGIN }), answers: c => forks(c, undefined, asRoot, KEEPS_NO_IMAGE) });
      sockets.push(client.ws);
      // A cloud record standing on that computer, made as places-fork.test.ts's oldFork makes one: a copy on that
      // computer's own backend, which a fork there still is.
      const door = rt.places!;
      const joined = door.joined;
      door.joined = () => false;
      const ws = await createOn(rt, { golden: "snap_g", name: "x", on: "srv" }).finally(() => (door.joined = joined));
      expect(ws.kind).toBe("cloud");
      expect(door.joined(ws.place!)).toBe(true);
      await (await rt.sessions.start(ws.id, { prompt: "one", harness: "claude" })).finished;
      const turn = contexts.find(c => TURN_TOKEN_ENV in c.env)!;
      const { [TURN_TOKEN_ENV]: _turn, [THREAD_ENV]: _thread, [HOST_TOKEN_ENV]: _host, ...rest } = turn.env;
      for (const ws of sockets.splice(0)) ws.close();
      await places.srv.close();
      await rt.close();
      places.srv = undefined;
      places.runtime = undefined;
      return { env: rest, vault: turn.vault, brokered: turn.brokered };
    };
    const today = await run();
    expect(today.brokered).toBeUndefined();
    expect(today.env["WSP_MCP_LINEAR_AUTHORIZATION"]).toBe("lin_api_TESTONLY");
    expect(today.env["GH_CONFIG_DIR"]).toBeUndefined();
    expect(today.vault).toEqual(HELD);
    expect(await run(READY)).toEqual(today);
  });
});
