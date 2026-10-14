import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { describe, expect, it } from "vitest";

import { BROKER_DOOR_URL, OFF_SERVERS_FILE, buildCommand, buildEnv, forwardsSubagentText, newSessionId, PROJECT_DIR_ENV, SAVED_SPEND_TAIL_BYTES, savedSpendCommand, signInGuard, userMessageLine } from "../src/landmines.js";
import { agentWord, shellQuote, SLATE_BRIEF } from "@wsp/protocol";
import { writeStub } from "../../protocol/test/stub-script.js";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

describe("buildEnv", () => {
  const base = {
    CLAUDECODE: "1",
    CLAUDE_CODE_ENTRYPOINT: "cli",
    CLAUDE_CODE_SSE_PORT: "12345",
    FORCE_CODE_TERMINAL: "true",
    PATH: "/root/.local/bin:/usr/bin",
    HOME: "/Users/z",
  };

  it("strips inherited CLAUDE_CODE_*, CLAUDECODE and FORCE_CODE_TERMINAL", () => {
    const env = buildEnv({ base });
    expect(env.CLAUDECODE).toBeUndefined();
    expect(env.CLAUDE_CODE_ENTRYPOINT).toBeUndefined();
    expect(env.CLAUDE_CODE_SSE_PORT).toBeUndefined();
    expect(env.FORCE_CODE_TERMINAL).toBeUndefined();
    expect(env.PATH).toBe(base.PATH);
  });

  it("turns on the task list tools a headless run leaves off, so the agent keeps its list as it does in its own terminal", () => {
    expect(buildEnv({ base }).CLAUDE_CODE_ENABLE_TODO_TOOLS).toBe("1");
  });

  it("drops an inherited project key and sets the one the caller named, after the strip", () => {
    // The strip takes every inherited CLAUDE_CODE_* as a nesting mark, so a key merged into the base is thrown
    // away; the key a workspace's own kind answers is set after it, which is what makes it reach the CLI.
    expect(buildEnv({ base: { ...base, [PROJECT_DIR_ENV]: "-Users-z-repo" } })[PROJECT_DIR_ENV]).toBeUndefined();
    const keyed = buildEnv({ base, projectDirName: "-Users-z-repo" });
    expect(keyed[PROJECT_DIR_ENV]).toBe("-Users-z-repo");
    // A base carrying its own is still dropped, and the named one is what is set.
    expect(buildEnv({ base: { ...base, [PROJECT_DIR_ENV]: "-stale" }, projectDirName: "-Users-z-repo" })[PROJECT_DIR_ENV]).toBe("-Users-z-repo");
    // Nobody naming one leaves the CLI keying off the folder each turn runs in.
    expect(PROJECT_DIR_ENV in buildEnv({ base })).toBe(false);
  });

  it("never sets CLAUDE_CONFIG_DIR or IS_SANDBOX of its own: the login environment carries them where they are true", () => {
    // Claude keys its Keychain item by whether the variable is set, so a person's own Mac must see it unset unless
    // their shell sets it; a machine's login carries its own, the sandbox flag with it.
    const own = buildEnv({ base });
    expect(own.CLAUDE_CONFIG_DIR).toBeUndefined();
    expect(own.IS_SANDBOX).toBeUndefined();
    expect(own.HOME).toBe("/Users/z");
    const theirs = buildEnv({ base: { ...base, CLAUDE_CONFIG_DIR: "/Users/z/elsewhere" } });
    expect(theirs.CLAUDE_CONFIG_DIR).toBe("/Users/z/elsewhere");
    const guest = buildEnv({ base: { HOME: "/root", CLAUDE_CONFIG_DIR: "/root/.claude-cfg", IS_SANDBOX: "1" } });
    expect(guest.CLAUDE_CONFIG_DIR).toBe("/root/.claude-cfg");
    expect(guest.IS_SANDBOX).toBe("1");
  });

  it("sets the headless flags: IDE-discovery suppression", () => {
    const env = buildEnv({ base });
    expect(env.CLAUDE_CODE_AUTO_CONNECT_IDE).toBe("0");
    expect(env.CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL).toBe("1");
    const leaked = Object.keys(env).filter(
      (k) =>
        (k.startsWith("CLAUDE_CODE_") || k === "CLAUDECODE") &&
        k !== "CLAUDE_CODE_AUTO_CONNECT_IDE" &&
        k !== "CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL" &&
        k !== "CLAUDE_CODE_ENABLE_TODO_TOOLS" &&
        k !== "CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS",
    );
    expect(leaked).toEqual([]);
  });

  it("lets a background subagent run as long as the turn holds its reply: the CLI ends none on a clock of its own", () => {
    expect(buildEnv({ base }).CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS).toBe("0");
    expect(buildEnv({ base: { ...base, CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS: "600000" } }).CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS).toBe("0");
  });

  it("passes the API key through when given", () => {
    const env = buildEnv({ base: {}, apiKey: "sk-ant-x" });
    expect(env.ANTHROPIC_API_KEY).toBe("sk-ant-x");
  });

  it("sets our own long-lived token after the strip, so an inherited one of the same name never rides along beside it", () => {
    const env = buildEnv({ base: { ...base, CLAUDE_CODE_OAUTH_TOKEN: "inherited" }, oauthToken: "sk-ant-oat01-TESTONLY" });
    expect(env.CLAUDE_CODE_OAUTH_TOKEN).toBe("sk-ant-oat01-TESTONLY");
    expect(buildEnv({ base: { ...base, CLAUDE_CODE_OAUTH_TOKEN: "inherited" }, apiKey: "sk-ant-x" }).CLAUDE_CODE_OAUTH_TOKEN).toBeUndefined();
    expect(buildEnv({ base: {} }).CLAUDE_CODE_OAUTH_TOKEN).toBeUndefined();
  });
});

describe("a brokered launch", () => {
  const token = "turn-token-TESTONLY";
  const real = {
    ANTHROPIC_API_KEY: "sk-ant-api03-REAL",
    CLAUDE_CODE_OAUTH_TOKEN: "sk-ant-oat01-REAL",
    ANTHROPIC_AUTH_TOKEN: "gateway-REAL",
    ANTHROPIC_CUSTOM_HEADERS: "x-gateway-key: REAL",
    ANTHROPIC_FOUNDRY_API_KEY: "foundry-REAL",
    AWS_BEARER_TOKEN_BEDROCK: "bedrock-REAL",
    ANTHROPIC_BASE_URL: "https://gateway.acme.example",
  };
  const base = { ...real, HOME: "/root", PATH: "/usr/bin", IS_SANDBOX: "1", DISABLE_AUTOUPDATER: "1", WSP_HOST_TOKEN: token };
  const shared = {
    ANTHROPIC_BASE_URL: BROKER_DOOR_URL,
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: "1",
    CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST: "1",
    DISABLE_UPDATES: "1",
    API_TIMEOUT_MS: "3600000",
  };

  it("carries the placeholder of the credential's kind and every variable of the brokered road", () => {
    expect(buildEnv({ base, brokered: { token, kind: "token" } })).toMatchObject({ ...shared, CLAUDE_CODE_OAUTH_TOKEN: token, ENABLE_TOOL_SEARCH: "true" });
    expect(buildEnv({ base, brokered: { token, kind: "login" } })).toEqual(buildEnv({ base, brokered: { token, kind: "token" } }));
    expect(buildEnv({ base, brokered: { token, kind: "key" } })).toMatchObject({ ...shared, ANTHROPIC_API_KEY: token, ENABLE_TOOL_SEARCH: "true" });
    expect(buildEnv({ base, brokered: { token, kind: "gateway" } })).toMatchObject({ ...shared, ANTHROPIC_AUTH_TOKEN: token, ANTHROPIC_API_KEY: "" });
  });

  it("sends one placeholder per kind, since the CLI picks its header and betas by which one it finds", () => {
    expect(buildEnv({ base, brokered: { token, kind: "token" } })).not.toHaveProperty("ANTHROPIC_API_KEY");
    expect(buildEnv({ base, brokered: { token, kind: "token" } })).not.toHaveProperty("ANTHROPIC_AUTH_TOKEN");
    expect(buildEnv({ base, brokered: { token, kind: "key" } })).not.toHaveProperty("CLAUDE_CODE_OAUTH_TOKEN");
    expect(buildEnv({ base, brokered: { token, kind: "key" } })).not.toHaveProperty("ANTHROPIC_AUTH_TOKEN");
    expect(buildEnv({ base, brokered: { token, kind: "gateway" } })).not.toHaveProperty("CLAUDE_CODE_OAUTH_TOKEN");
  });

  it("turns tool search on only where the upstream is first party, which a gateway's is not", () => {
    expect(buildEnv({ base, brokered: { token, kind: "gateway" } })).not.toHaveProperty("ENABLE_TOOL_SEARCH");
  });

  it("holds no real credential, whatever the login or the vault carried", () => {
    for (const kind of ["login", "token", "key", "gateway"] as const) {
      const env = buildEnv({ base, apiKey: "sk-ant-api03-VAULT", oauthToken: "sk-ant-oat01-VAULT", brokered: { token, kind } });
      const values = Object.values(env);
      for (const value of [...Object.values(real), "sk-ant-api03-VAULT", "sk-ant-oat01-VAULT"]) expect(values).not.toContain(value);
      expect(env).not.toHaveProperty("ANTHROPIC_CUSTOM_HEADERS");
      expect(env).not.toHaveProperty("ANTHROPIC_FOUNDRY_API_KEY");
      expect(env).not.toHaveProperty("AWS_BEARER_TOKEN_BEDROCK");
    }
  });

  it("keeps the rest of the launch: the login's own variables, the headless flags and the project key", () => {
    const env = buildEnv({ base, brokered: { token, kind: "token" }, projectDirName: "-root-lab" });
    expect(env).toMatchObject({ HOME: "/root", PATH: "/usr/bin", IS_SANDBOX: "1", DISABLE_AUTOUPDATER: "1", WSP_HOST_TOKEN: token });
    expect(env).toMatchObject({ CLAUDE_CODE_AUTO_CONNECT_IDE: "0", CLAUDE_CODE_ENABLE_TODO_TOOLS: "1", [PROJECT_DIR_ENV]: "-root-lab" });
  });

  it("asks the CLI about no other login, since the placeholder is the only sign-in it may use", () => {
    expect(signInGuard({ base, brokered: { token, kind: "token" } })).toBe("");
    expect(signInGuard({ base })).not.toBe("");
  });

  it("leaves a launch that is not brokered as it was", () => {
    const vaulted = buildEnv({ base, oauthToken: "sk-ant-oat01-VAULT" });
    const inherited = buildEnv({ base });
    for (const env of [vaulted, inherited]) {
      for (const name of ["CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST", "DISABLE_UPDATES", "API_TIMEOUT_MS", "ENABLE_TOOL_SEARCH"]) expect(env).not.toHaveProperty(name);
      expect(env).toMatchObject({ ANTHROPIC_BASE_URL: real.ANTHROPIC_BASE_URL, ANTHROPIC_AUTH_TOKEN: real.ANTHROPIC_AUTH_TOKEN });
    }
    expect(vaulted.CLAUDE_CODE_OAUTH_TOKEN).toBe("sk-ant-oat01-VAULT");
    expect(inherited.CLAUDE_CODE_OAUTH_TOKEN).toBe(real.CLAUDE_CODE_OAUTH_TOKEN);
  });
});

describe("newSessionId", () => {
  it("returns unique v4-shaped UUIDs", () => {
    const a = newSessionId();
    const b = newSessionId();
    expect(a).toMatch(UUID_RE);
    expect(b).toMatch(UUID_RE);
    expect(a).not.toBe(b);
  });
});

describe("buildCommand", () => {
  it("a fast turn turns fast mode on through the settings the launch carries, and a turn that is not fast carries none", () => {
    const fast = buildCommand({ sessionId: "11111111-2222-4333-8444-555555555555", fast: true });
    expect(fast).toContain(`--settings '{"fastMode":true}'`);
    expect(buildCommand({ sessionId: "11111111-2222-4333-8444-555555555555" })).not.toContain("--settings");
  });

  it("leaves out each server it is told to through the CLI's denylist, beside the person's own settings, and none where it is told none", () => {
    const cmd = buildCommand({ sessionId: "11111111-2222-4333-8444-555555555555", offServers: ["cloudflare", "posthog"], launch: { args: ["--settings", JSON.stringify({ theme: "dark" })] } });
    expect(cmd).toContain(`--settings '{"theme":"dark","deniedMcpServers":[{"serverName":"cloudflare"},{"serverName":"posthog"}]}'`);
    expect(buildCommand({ sessionId: "11111111-2222-4333-8444-555555555555", offServers: [] })).not.toContain("--settings");
  });

  it("writes the off servers into a managed drop-in before the launch, so a claude the agent runs there denies them too, and a write that fails still launches", () => {
    const root = mkdtempSync(join(tmpdir(), "wsp-off-"));
    try {
      const bin = join(root, "bin");
      mkdirSync(bin);
      writeStub(join(bin, "claude"), `#!/bin/sh\npwd > ${shellQuote(join(root, "ran"))}\n`);
      const run = (cmd: string, managed: string): void => void execFileSync("/bin/bash", ["-c", cmd.replaceAll(dirname(OFF_SERVERS_FILE), managed)], { env: { PATH: `${bin}:/usr/bin:/bin` } });
      const managed = join(root, "managed-settings.d");
      run(buildCommand({ sessionId: "11111111-2222-4333-8444-555555555555", cwd: root, offServers: ["cloudflare", "posthog"] }), managed);
      expect(JSON.parse(readFileSync(join(managed, basename(OFF_SERVERS_FILE)), "utf8"))).toEqual({ deniedMcpServers: [{ serverName: "cloudflare" }, { serverName: "posthog" }] });
      expect(readFileSync(join(root, "ran"), "utf8").trim()).toBe(root);
      rmSync(join(root, "ran"));
      writeFileSync(join(root, "blocked"), "");
      run(buildCommand({ sessionId: "11111111-2222-4333-8444-555555555555", cwd: root, offServers: ["cloudflare"] }), join(root, "blocked", "managed-settings.d"));
      expect(existsSync(join(root, "ran"))).toBe(true);
      expect(buildCommand({ sessionId: "11111111-2222-4333-8444-555555555555", offServers: [] })).not.toContain(OFF_SERVERS_FILE);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  const sessionId = "e16ed170-8257-4668-879e-fe836341633c";

  it("carries every required flag for a fresh session and reads its messages from stdin as stream-json", () => {
    const cmd = buildCommand({ sessionId });
    expect(cmd).toContain("--output-format stream-json");
    expect(cmd).toContain("--input-format stream-json");
    expect(cmd).toContain("--verbose");
    expect(cmd).toContain("--dangerously-skip-permissions");
    expect(cmd).toContain(`--session-id ${sessionId}`);
    // stdin is the message channel now: never closed on the command, never a positional prompt
    expect(cmd).not.toContain("</dev/null");
    expect(cmd).toMatch(/claude -p --/);
    expect(cmd).not.toContain("--resume");
  });

  it("forwards a subagent's own text and thinking only where asked, since an older CLI exits on the flag", () => {
    expect(buildCommand({ sessionId, subagentText: true })).toContain("--forward-subagent-text");
    expect(buildCommand({ sessionId })).not.toContain("--forward-subagent-text");
  });

  it("takes the subagent text flag from 2.1.270 on, the first CLI whose SDK passes it", () => {
    for (const version of ["2.1.270", "2.1.288", "2.2.0", "3.0.0"]) expect(forwardsSubagentText(version), version).toBe(true);
    for (const version of ["2.1.269", "2.0.99", "1.9.300", "", "not a version"]) expect(forwardsSubagentText(version), version).toBe(false);
    expect(forwardsSubagentText(undefined)).toBe(false);
  });

  it("uses --resume instead of --session-id when resuming", () => {
    const cmd = buildCommand({ resume: sessionId });
    expect(cmd).toContain(`--resume ${sessionId}`);
    expect(cmd).not.toContain("--session-id");
  });

  it("prefixes cd when a cwd is given", () => {
    const cmd = buildCommand({ sessionId, cwd: "/root/app" });
    expect(cmd.startsWith(`cd '/root/app' && ${agentWord("claude", undefined)} -p`)).toBe(true);
  });

  it("maps the picked model, effort and permission mode to the CLI's flags", () => {
    const cmd = buildCommand({ sessionId, model: "claude-opus-5", effort: "high", permissionMode: "acceptEdits" });
    expect(cmd).toContain("--model 'claude-opus-5'");
    expect(cmd).toContain("--effort 'high'");
    expect(cmd).toContain("--permission-mode 'acceptEdits'");
    expect(cmd).not.toContain("--dangerously-skip-permissions");
  });

  it("no permission mode and bypassPermissions both skip permissions and still route the question tool here", () => {
    // Without the prompt tool the CLI has no road to a person and switches AskUserQuestion off; with it, at
    // --dangerously-skip-permissions, every other tool still runs unasked (measured on this Mac, 2026-10-03).
    for (const cmd of [buildCommand({ sessionId }), buildCommand({ sessionId, permissionMode: "bypassPermissions" })]) {
      expect(cmd).toContain("--dangerously-skip-permissions");
      expect(cmd).not.toContain("--permission-mode");
      expect(cmd).toContain("--permission-prompt-tool stdio");
    }
  });

  it("default names itself and routes its prompts to this host, so the person's own settings do not decide the turn", () => {
    const cmd = buildCommand({ sessionId, permissionMode: "default" });
    expect(cmd).toContain("--permission-mode 'default'");
    expect(cmd).toContain("--permission-prompt-tool stdio");
    expect(cmd).not.toContain("--dangerously-skip-permissions");
  });

  it("every mode that can prompt routes its prompts here", () => {
    for (const mode of ["acceptEdits", "manual", "dontAsk", "auto"]) {
      expect(buildCommand({ sessionId, permissionMode: mode })).toContain("--permission-prompt-tool stdio");
    }
  });

  it("a context window rides the model as the CLI's own suffix; 200k is the plain slug", () => {
    expect(buildCommand({ sessionId, model: "claude-opus-5", contextWindow: "1m" })).toContain("--model 'claude-opus-5[1m]'");
    expect(buildCommand({ sessionId, model: "claude-opus-5", contextWindow: "200k" })).toContain("--model 'claude-opus-5'");
    expect(() => buildCommand({ sessionId, model: "claude-opus-5", contextWindow: "2m" })).toThrow(/contextWindow/);
    expect(() => buildCommand({ sessionId, contextWindow: "1m" })).toThrow(/contextWindow/);
  });

  it("servers handed to a turn ride one --mcp-config as the JSON a config file holds, and the config dir's own are kept", () => {
    const cmd = buildCommand({ sessionId, mcpServers: { wsp: { command: "/usr/local/bin/node", args: ["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"] } } });
    const flag = /--mcp-config '(.+?)' /.exec(cmd)?.[1];
    // The wsp server loads up front, so the slate tools that do not opt out of it are in the prompt with no search.
    expect(JSON.parse(flag!)).toEqual({ mcpServers: { wsp: { command: "/usr/local/bin/node", args: ["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"], alwaysLoad: true } } });
    const docs = /--mcp-config '(.+?)' /.exec(buildCommand({ sessionId, mcpServers: { docs: { command: "npx", args: [] } } }))?.[1];
    expect(JSON.parse(docs!)).toEqual({ mcpServers: { docs: { command: "npx", args: [] } } });
    // Strict would drop the servers the person's own config names, which this turn still wants.
    expect(cmd).not.toContain("--strict-mcp-config");
    expect(buildCommand({ sessionId })).not.toContain("--mcp-config");
    expect(buildCommand({ sessionId, mcpServers: {} })).not.toContain("--mcp-config");
  });

  it("a launch whose wsp server gives the thread a slate brings the slate's brief to the end of the system prompt, and no other launch does", () => {
    const scoped = buildCommand({ sessionId, mcpServers: { wsp: { command: "/opt/wsp", args: ["mcp", "--scoped"] } } });
    expect(scoped).toContain(`--append-system-prompt ${shellQuote(SLATE_BRIEF)}`);
    expect(SLATE_BRIEF).toContain("your first tool call is mcp__wsp__slate_catalog");
    expect(SLATE_BRIEF).toContain("Never read that file or put its value in a tool call.");
    // A thread on a box has a slate too, though its server there is the unscoped one the host serves as a guest.
    expect(buildCommand({ sessionId, mcpServers: { wsp: { command: "wsp", args: ["mcp"] } } })).toContain(`--append-system-prompt ${shellQuote(SLATE_BRIEF)}`);
    // A thread another thread started has no slate, so nothing tells it of one.
    expect(buildCommand({ sessionId, mcpServers: { wsp: { command: "/opt/wsp", args: ["mcp", "--scoped", "--no-slate"], noSlate: true } } })).not.toContain("--append-system-prompt");
    expect(buildCommand({ sessionId })).not.toContain("--append-system-prompt");
  });

  it("sends no model or effort flag when none was picked", () => {
    const cmd = buildCommand({ sessionId });
    expect(cmd).not.toContain("--model");
    expect(cmd).not.toContain("--effort");
  });

  it("rejects a picked value that is not a plain slug", () => {
    expect(() => buildCommand({ sessionId, model: "opus; rm -rf /" })).toThrow(/model/);
    expect(() => buildCommand({ sessionId, effort: "" })).toThrow(/effort/);
    expect(() => buildCommand({ sessionId, permissionMode: "accept edits" })).toThrow(/permissionMode/);
    expect(buildCommand({ sessionId, model: "claude-opus-5[1m]" })).toContain("--model 'claude-opus-5[1m]'");
  });

  it("rejects zero or two session identifiers", () => {
    expect(() => buildCommand({})).toThrow(/exactly one/);
    expect(() => buildCommand({ sessionId, resume: sessionId })).toThrow(
      /exactly one/,
    );
  });

  it("rejects a session identifier that is not a UUID", () => {
    expect(() => buildCommand({ resume: "$(rm -rf /)" })).toThrow(/UUID/);
    expect(() => buildCommand({ sessionId: "abc" })).toThrow(/UUID/);
  });
});

describe("userMessageLine", () => {
  const sessionId = "e16ed170-8257-4668-879e-fe836341633c";

  it("is one stream-json user line the CLI takes on stdin, with the text intact", () => {
    const text = "don't run $(reboot) `id`\nsecond line with \"quotes\"";
    const line = userMessageLine(text, sessionId);
    expect(line).not.toContain("\n");
    expect(JSON.parse(line)).toEqual({
      type: "user",
      message: { role: "user", content: [{ type: "text", text }] },
      parent_tool_use_id: null,
      session_id: sessionId,
    });
  });

  it("carries an image as a base64 content block ahead of the text, with the type the person's file was", () => {
    const bytes = Buffer.from("not really a png").toString("base64");
    const line = userMessageLine("what does this show?", sessionId, [{ mediaType: "image/png", bytes }]);
    expect(line).not.toContain("\n");
    expect(JSON.parse(line)).toEqual({
      type: "user",
      message: {
        role: "user",
        content: [
          { type: "image", source: { type: "base64", media_type: "image/png", data: bytes } },
          { type: "text", text: "what does this show?" },
        ],
      },
      parent_tool_use_id: null,
      session_id: sessionId,
    });
  });

  it("carries every image of the message, in the order the person added them, each with its own type", () => {
    const line = userMessageLine("these three", sessionId, [
      { mediaType: "image/png", bytes: "AAA=" },
      { mediaType: "image/jpeg", bytes: "BBB=" },
      { mediaType: "image/webp", bytes: "CCC=" },
    ]);
    const content = (JSON.parse(line) as { message: { content: { type: string; source?: { media_type: string; data: string } }[] } }).message.content;
    expect(content.map(block => block.type)).toEqual(["image", "image", "image", "text"]);
    expect(content.slice(0, 3).map(block => block.source?.media_type)).toEqual(["image/png", "image/jpeg", "image/webp"]);
    expect(content.slice(0, 3).map(block => block.source?.data)).toEqual(["AAA=", "BBB=", "CCC="]);
  });

  it("a message with no image is the one-block line it always was: nothing rides for free", () => {
    expect(JSON.parse(userMessageLine("plain", sessionId, []))).toEqual(JSON.parse(userMessageLine("plain", sessionId)));
  });
});

describe("the saved running cost a resume prints ahead of the CLI", () => {
  const SESSION = "e16ed170-8257-4668-879e-fe836341633c";
  const saved = (total: number) => JSON.stringify({ type: "cost-state", sessionId: SESSION, totalCostUSD: total, modelUsage: {} });
  const printed = (lines: string[]): string => {
    const dir = mkdtempSync(join(tmpdir(), "wsp-saved-spend-"));
    try {
      mkdirSync(join(dir, "projects", "-w"), { recursive: true });
      writeFileSync(join(dir, "projects", "-w", `${SESSION}.jsonl`), `${lines.join("\n")}\n`);
      return execFileSync("bash", ["-c", `${savedSpendCommand({ configDir: dir, sessionId: SESSION })}echo launched`], { encoding: "utf8" });
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  };

  it("is the last cost-state line of the session's file", () => {
    expect(printed([saved(1), '{"type":"user"}', saved(352.3), '{"type":"last-prompt"}'])).toBe(`${saved(352.3)}\nlaunched\n`);
  });

  it("is read off the file's last bytes alone: a line further back than that prints nothing, and the turn's cost is then not known", () => {
    const filler = JSON.stringify({ type: "user", text: "x".repeat(SAVED_SPEND_TAIL_BYTES) });
    expect(printed([saved(352.3), filler])).toBe("launched\n");
  });

  it("prints nothing for a session with no file", () => {
    expect(execFileSync("bash", ["-c", `${savedSpendCommand({ configDir: "/nonexistent-config", sessionId: SESSION })}echo launched`], { encoding: "utf8" })).toBe("launched\n");
  });
});
