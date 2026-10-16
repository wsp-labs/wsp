// SPDX-License-Identifier: AGPL-3.0-only
// The copy of the agent configs to a provider's machine: every secret value a
// config holds blanked or left out, a note naming what stayed here, and a
// config with no secret landing byte for byte.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import type { ManifestEntry } from "@wsp/collect";
import { CLAUDE_CONFIG_REL, GUEST_REWRITES } from "@wsp/catalog";
import { planFiles, type SkippedPath } from "@wsp/engine";
import { parse as parseToml } from "smol-toml";
import { afterEach, describe, expect, it } from "vitest";
import { serverVault, type ServerVault } from "../src/env-keys.js";
import { packPlan, statOf } from "../src/init-import.js";
import { tokenShaped } from "../src/token-shapes.js";

const dirs: string[] = [];
afterEach(() => {
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
});

const FAKE_KEY = "sk-ant-api03-FAKE000000000000000000000";
const FAKE_GH = "ghp_FAKE0000000000000000000000000000";

function home(files: Record<string, string>): string {
  const h = realpathSync(mkdtempSync(join(tmpdir(), "wsp-cloud-home-")));
  dirs.push(h);
  for (const [rel, text] of Object.entries(files)) {
    mkdirSync(dirname(join(h, rel)), { recursive: true });
    writeFileSync(join(h, rel), text);
  }
  return h;
}

const row = (id: string, paths: string[], rung: ManifestEntry["rung"] = "agents"): ManifestEntry => ({ rung, id, label: id, paths, bytes: 0, default: "bring", bring: true });

interface Landed {
  root: string;
  skipped: SkippedPath[];
  blanked: SkippedPath[];
}

async function landed(h: string, rows: ManifestEntry[], cloud: boolean, o: { vault?: ServerVault; secrets?: Map<string, string>; platform?: "darwin" | "linux" } = {}): Promise<Landed> {
  const plan = planFiles(rows, { home: h, stat: statOf, platform: o.platform ?? "linux", rewrites: GUEST_REWRITES, read: f => (existsSync(f) ? readFileSync(f, "utf8") : undefined) });
  const packed = await packPlan(plan, { secrets: o.secrets ?? new Map(), home: h, ...(cloud ? { cloud: true } : {}), ...(o.vault !== undefined ? { vault: o.vault } : {}) });
  const root = mkdtempSync(join(tmpdir(), "wsp-cloud-x-"));
  dirs.push(root);
  writeFileSync(join(root, "a.tgz"), packed.tar);
  execFileSync("tar", ["-xzf", join(root, "a.tgz"), "-C", root]);
  rmSync(join(root, "a.tgz"));
  return { root, skipped: packed.skipped, blanked: packed.blanked ?? [] };
}

const read = (root: string, rel: string): string => readFileSync(join(root, rel), "utf8");
const notes = (skipped: readonly SkippedPath[], path: string): string => skipped.filter(s => s.path === path).map(s => s.note).join(" | ");

function files(dir: string): string[] {
  return readdirSync(dir).flatMap(name => (statSync(join(dir, name)).isDirectory() ? files(join(dir, name)) : [join(dir, name)]));
}

const CLAUDE_JSON = {
  numStartups: 3,
  hasCompletedOnboarding: true,
  oauthAccount: { accountUuid: "acct-1", emailAddress: "me@example.com", organizationUuid: "org-1" },
  customApiKeyResponses: { approved: ["0000000000FAKE"], rejected: [] },
  primaryApiKey: FAKE_KEY,
  mcpServers: {
    zap: { type: "http", url: "https://actions.example.com/mcp/ak1b2c3d4e5f6g7h8i9j0k1l2m3/sse?api_key=abc123&transport=sse" },
    behind: { type: "http", url: "https://me:pa55word@mcp.example.com/sse" },
    fs: { type: "stdio", command: "npx", args: ["-y", "@acme/lab-server", "--api-key", "plainvalue1", "--token=xyz789", "LAB_SECRET=hush", "--passwd", "hunter", "--credential", "hunter", "--credentials", "hunter", "--stdio", "/srv/data"], env: {} },
  },
};

describe("a copy to a provider's machine", () => {
  it("leaves the Claude account fields and the /login key out of ~/.claude.json and names them", async () => {
    const h = home({ ".claude.json": `${JSON.stringify(CLAUDE_JSON, null, 2)}\n` });
    const { root, skipped, blanked } = await landed(h, [row("agents/claude", ["~/.claude.json"])], true);
    const json = JSON.parse(read(root, `${CLAUDE_CONFIG_REL}/.claude.json`)) as Record<string, unknown>;
    // The file travelled, so its note is with what was cut and never with what was skipped.
    expect(notes(skipped, "~/.claude.json")).toBe("");
    expect(json.oauthAccount).toBeUndefined();
    expect(json.customApiKeyResponses).toBeUndefined();
    expect(json.primaryApiKey).toBeUndefined();
    expect(json.numStartups).toBe(3);
    expect(json.hasCompletedOnboarding).toBe(true);
    expect(notes(blanked, "~/.claude.json")).toMatch(/oauthAccount, customApiKeyResponses, primaryApiKey/);
    expect(notes(blanked, "~/.claude.json")).toMatch(/on this computer/);
  });

  it("blanks the MCP literals in arguments and addresses, keeping every other argument", async () => {
    const codex = [
      "[mcp_servers.lab]",
      'command = "npx"',
      "args = [",
      '  "-y", "@acme/lab-server",',
      '  "--access-token", "plainvalue2",',
      "]",
      "",
      "[mcp_servers.far]",
      'url = "https://mcp.example.com/mcp?token=abc123"',
      "",
    ].join("\n");
    const h = home({ ".claude.json": `${JSON.stringify(CLAUDE_JSON, null, 2)}\n`, ".codex/config.toml": codex });
    const { root, blanked } = await landed(h, [row("agents/claude", ["~/.claude.json"]), row("agents/codex", ["~/.codex/config.toml"])], true);
    const servers = (JSON.parse(read(root, `${CLAUDE_CONFIG_REL}/.claude.json`)) as typeof CLAUDE_JSON).mcpServers;
    expect(servers.fs.args).toEqual(["-y", "@acme/lab-server", "--api-key", "", "--token=", "LAB_SECRET=", "--passwd", "", "--credential", "", "--credentials", "", "--stdio", "/srv/data"]);
    expect(servers.zap.url).toBe("https://actions.example.com/mcp//sse?api_key=&transport=sse");
    expect(servers.behind.url).toBe("https://mcp.example.com/sse");
    expect(notes(blanked, "~/.claude.json")).toMatch(/MCP/);
    const toml = read(root, ".codex/config.toml");
    expect(toml).toContain('"--access-token", "",');
    expect(toml).toContain('"-y", "@acme/lab-server",');
    expect(toml).toContain('url = "https://mcp.example.com/mcp?token="');
    expect(toml).not.toMatch(/plainvalue2|abc123/);
    expect(notes(blanked, "~/.codex/config.toml")).toMatch(/on this computer/);
  });

  it("leaves the secret names of the Claude settings env block out and blanks a hook's inline secret", async () => {
    const settings = {
      model: "opus",
      env: { GITHUB_TOKEN: FAKE_GH, OPENAI_API_KEY: "plain-key-1", ANTHROPIC_BASE_URL: "https://gateway.example.com", DISABLE_TELEMETRY: "1", LAB_TOKEN: "${LAB_TOKEN}" },
      hooks: { Stop: [{ hooks: [{ type: "command", command: "SLACK_TOKEN=xoxb-plain curl -s https://hooks.example.com/stop" }] }] },
    };
    const h = home({ ".claude/settings.json": `${JSON.stringify(settings, null, 2)}\n` });
    const { root, blanked } = await landed(h, [row("agents/claude", ["~/.claude/settings.json"])], true);
    const out = JSON.parse(read(root, `${CLAUDE_CONFIG_REL}/settings.json`)) as typeof settings;
    expect(out.env).toEqual({ ANTHROPIC_BASE_URL: "https://gateway.example.com", DISABLE_TELEMETRY: "1", LAB_TOKEN: "${LAB_TOKEN}" });
    expect(out.hooks.Stop[0]!.hooks[0]!.command).toBe("SLACK_TOKEN= curl -s https://hooks.example.com/stop");
    expect(out.model).toBe("opus");
    expect(notes(blanked, "~/.claude/settings.json")).toMatch(/GITHUB_TOKEN, OPENAI_API_KEY/);
    expect(notes(blanked, "~/.claude/settings.json")).toMatch(/SLACK_TOKEN/);
  });

  it("leaves mcp-remote's sign-ins out", async () => {
    const h = home({ ".mcp-auth/mcp-remote-v1/0123456789abcdef0123456789abcdef_tokens.json": '{"access_token":"plain"}\n' });
    const { root, skipped } = await landed(h, [row("agents/mcp/mcp-remote", ["~/.mcp-auth"])], true);
    expect(existsSync(join(root, ".mcp-auth"))).toBe(false);
    expect(notes(skipped, "~/.mcp-auth")).toMatch(/on this computer/);
  });

  it("leaves the git credential helpers and url rewrites out, with git asking gh instead", async () => {
    const git = ["[user]", "\tname = Me", "\temail = me@example.com", '[credential "https://github.com"]', "\thelper = !/opt/homebrew/bin/gh auth git-credential", `[url "https://x-access-token:${FAKE_GH}@github.com/"]`, "\tinsteadOf = https://github.com/", "[alias]", "\tst = status", ""].join("\n");
    const h = home({ ".gitconfig": git });
    const { root, blanked } = await landed(h, [row("identity/git-user", ["~/.gitconfig"], "identity")], true);
    const out = read(root, ".gitconfig");
    expect(out).toContain("\tname = Me");
    expect(out).toContain("\tst = status");
    expect(out).not.toContain("insteadOf");
    expect(out).not.toContain(FAKE_GH);
    expect(out).not.toContain("/opt/homebrew");
    expect(out).toMatch(/\[credential\]\n\thelper = !gh auth git-credential\n$/);
    const helpers = execFileSync("git", ["config", "--file", join(root, ".gitconfig"), "--get-regexp", "^(credential|url)\\."], { cwd: root, encoding: "utf8" });
    expect(helpers).toBe("credential.helper !gh auth git-credential\n");
    expect(notes(blanked, "~/.gitconfig")).toMatch(/\[credential\].*\[url\]/);
  });

  it("leaves fish's universal variables out and keeps the rest of its config", async () => {
    const h = home({ ".config/fish/config.fish": "set -g fish_greeting\n", ".config/fish/fish_variables": `SETUVAR --export GH_TOKEN:${FAKE_GH}\n` });
    const { root, skipped } = await landed(h, [row("shell/fish", ["~/.config/fish"], "shell")], true);
    expect(read(root, ".config/fish/config.fish")).toBe("set -g fish_greeting\n");
    expect(existsSync(join(root, ".config/fish/fish_variables"))).toBe(false);
    expect(notes(skipped, "~/.config/fish/fish_variables")).toMatch(/on this computer/);
  });

  it("blanks a token-shaped value anywhere it lands and leaves a private key out, so no copied file holds a token", async () => {
    const h = home({
      ".claude/skills/lab/SKILL.md": `# lab\nCall it with ${FAKE_GH} as the key.\n`,
      ".claude/skills/lab/deploy.pem": "-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----\n",
      ".claude.json": `${JSON.stringify(CLAUDE_JSON, null, 2)}\n`,
    });
    const { root, skipped, blanked } = await landed(h, [row("agents/claude", ["~/.claude/skills", "~/.claude.json"])], true);
    expect(read(root, `${CLAUDE_CONFIG_REL}/skills/lab/SKILL.md`)).toBe("# lab\nCall it with  as the key.\n");
    expect(existsSync(join(root, `${CLAUDE_CONFIG_REL}/skills/lab/deploy.pem`))).toBe(false);
    expect(notes(skipped, "~/.claude/skills/lab/deploy.pem")).toMatch(/private key/);
    expect(notes(blanked, "~/.claude/skills/lab/SKILL.md")).toMatch(/token/);
    for (const f of files(root)) expect(tokenShaped(readFileSync(f, "utf8")), f).toBe(false);
  });

  it("lands a config with no secret byte for byte", async () => {
    const servers = {
      fs: { command: "npx", args: ["-y", "@acme/lab-server", "--stdio", "/srv/data"] },
      es: { command: "npx", args: ["-y", "mcp-server-elasticsearch-v2"] },
      cf: { command: "npx", args: ["@cloudflare/mcp-server-cloudflare", "run", "0123456789abcdef0123456789abcdef"] },
      db: { command: "lab-db", args: ["--database", "123e4567-e89b-12d3-a456-426614174000"] },
      open: { command: "lab-files", args: ["--no-auth", "/srv/data", "--auth", "oauth", "--token-file", "/srv/token", "--cert", "/srv/c.pem"] },
      remote2: { command: "npx", args: ["-y", "mcp-remote", "https://mcp.example.com/sse", "--header", "Authorization:${AUTH_HEADER}"] },
      ended: { command: "lab", args: ["--", "--token", "plainvalue3"] },
      docs: { type: "http", url: "https://docs.example.com/acme/lab-extension-samples-collection" },
    };
    const settings = {
      model: "opus",
      env: { DISABLE_TELEMETRY: "1" },
      hooks: {
        Stop: [{ hooks: [{ type: "command", command: "GITHUB_TOKEN=$(gh auth token) curl -s https://hooks.example.com/stop" }] }],
        SessionStart: [{ hooks: [{ type: "command", command: "LAB_TOKEN=`gh auth token` curl -s https://hooks.example.com/start" }] }],
      },
    };
    const quiet = {
      ".claude.json": `${JSON.stringify({ numStartups: 3, mcpServers: servers }, null, 2)}\n`,
      ".claude/settings.json": `{\n  // the person's own comment\n${JSON.stringify(settings, null, 2).slice(2)}\n`,
      ".claude/skills/lab/SKILL.md": "# lab\nNothing secret here.\n",
      ".codex/config.toml": [
        "[mcp_servers.lab]",
        'command = "npx"',
        'args = ["-y", "@acme/lab-server"]',
        "",
        "[mcp_servers.remote2]",
        'command = "npx"',
        'args = ["-y", "mcp-remote", "https://mcp.example.com/sse", "--header", "Authorization:${AUTH_HEADER}"]',
        "",
        "[shell_environment_policy]",
        'set = { LAB_MODE = "fast", LAB_TOKEN = "${LAB_TOKEN}" }',
        "",
      ].join("\n"),
      ".gitconfig": "[user]\n\tname = Me\n[alias]\n\tst = status\n",
      ".config/fish/config.fish": "set -g fish_greeting\n",
    };
    const h = home(quiet);
    // servers.env holds the header's value, so the line that names it is the reference alone.
    const vault = serverVault(join(h, "state.json"));
    vault.hold({ AUTH_HEADER: "Bearer plain-header-1" }, "remote2");
    const rows = [
      row("agents/claude", ["~/.claude.json", "~/.claude/settings.json", "~/.claude/skills"]),
      row("agents/codex", ["~/.codex/config.toml"]),
      row("identity/git-user", ["~/.gitconfig"], "identity"),
      row("shell/fish", ["~/.config/fish"], "shell"),
    ];
    const { root, skipped, blanked } = await landed(h, rows, true, { vault });
    expect(read(root, `${CLAUDE_CONFIG_REL}/.claude.json`)).toBe(quiet[".claude.json"]);
    expect(read(root, `${CLAUDE_CONFIG_REL}/settings.json`)).toBe(quiet[".claude/settings.json"]);
    expect(read(root, `${CLAUDE_CONFIG_REL}/skills/lab/SKILL.md`)).toBe(quiet[".claude/skills/lab/SKILL.md"]);
    expect(read(root, ".codex/config.toml")).toBe(quiet[".codex/config.toml"]);
    expect(read(root, ".gitconfig")).toBe(quiet[".gitconfig"]);
    expect(read(root, ".config/fish/config.fish")).toBe(quiet[".config/fish/config.fish"]);
    expect(blanked).toEqual([]);
    expect(skipped.filter(s => /on this computer/.test(s.note))).toEqual([]);
  });

  it("leaves out the env variables an agent reads as header lines", async () => {
    const settings = { env: { ANTHROPIC_CUSTOM_HEADERS: "x-api-key: plain0customheader", OTEL_EXPORTER_OTLP_HEADERS: "Authorization=Bearer plain0otel", CLAUDE_CODE_OTEL_HEADERS_HELPER_DEBOUNCE_MS: "1000", DISABLE_TELEMETRY: "1" } };
    const h = home({ ".claude/settings.json": `${JSON.stringify(settings, null, 2)}\n` });
    const { root, blanked } = await landed(h, [row("agents/claude", ["~/.claude/settings.json"])], true);
    expect(JSON.parse(read(root, `${CLAUDE_CONFIG_REL}/settings.json`)).env).toEqual({ CLAUDE_CODE_OTEL_HEADERS_HELPER_DEBOUNCE_MS: "1000", DISABLE_TELEMETRY: "1" });
    expect(notes(blanked, "~/.claude/settings.json")).toMatch(/^ANTHROPIC_CUSTOM_HEADERS, OTEL_EXPORTER_OTLP_HEADERS blanked or left out/);
  });

  it("blanks a signed address's signature in a hook, an MCP address and Codex's query_params", async () => {
    const settings = { hooks: { Stop: [{ hooks: [{ type: "command", command: "curl -s 'https://cdn.example.com/stop?Expires=1767290400&Signature=plain0hook&Key-Pair-Id=K2LAB'" }] }] } };
    const claude = { mcpServers: { lab: { type: "http", url: "https://lab.blob.core.windows.net/mcp?sv=2024-11-04&sig=plain0mcp" } } };
    const codex = ["[model_providers.az]", 'base_url = "https://lab.openai.azure.com/v1"', 'query_params = { "api-version" = "2025-01-01", sig = "plain0codex" }', ""].join("\n");
    const h = home({ ".claude/settings.json": `${JSON.stringify(settings, null, 2)}\n`, ".claude.json": `${JSON.stringify(claude, null, 2)}\n`, ".codex/config.toml": codex });
    const { root, blanked } = await landed(h, [row("agents/claude", ["~/.claude/settings.json", "~/.claude.json"]), row("agents/codex", ["~/.codex/config.toml"])], true);
    for (const f of files(root)) expect(read(root, f.slice(root.length + 1)), f).not.toContain("plain0");
    expect(JSON.parse(read(root, `${CLAUDE_CONFIG_REL}/settings.json`)).hooks.Stop[0].hooks[0].command).toBe("curl -s 'https://cdn.example.com/stop?Expires=1767290400&Signature=&Key-Pair-Id='");
    expect(JSON.parse(read(root, `${CLAUDE_CONFIG_REL}/.claude.json`)).mcpServers.lab.url).toBe("https://lab.blob.core.windows.net/mcp?sv=2024-11-04&sig=");
    expect(parseToml(read(root, ".codex/config.toml"))).toEqual({ model_providers: { az: { base_url: "https://lab.openai.azure.com/v1", query_params: { "api-version": "2025-01-01", sig: "" } } } });
    expect(notes(blanked, "~/.claude/settings.json")).toMatch(/^Signature, Key-Pair-Id blanked or left out/);
    expect(notes(blanked, "~/.claude.json")).toMatch(/^sig of the MCP server lab blanked or left out/);
    expect(notes(blanked, "~/.codex/config.toml")).toMatch(/^sig blanked or left out/);
  });

  it("leaves out every header of Codex's otel exporter and model providers that holds a value, and the file still parses", async () => {
    const codex = [
      "[otel]",
      'exporter = { otlp-http = { endpoint = "https://otel.example.com", headers = { "x-lab-team" = "plain0codexotel", "x-var" = "${V}" } } }',
      "",
      "[model_providers.gw]",
      'base_url = "https://gw.example.com/v1"',
      'http_headers = { "X-Gw-Key" = "plain0codexprov" }',
      "",
      "[model_providers.lab.http_headers]",
      '"X-Lab-Key" = "Bearer plain0codextable"',
      '"X-Lab-Ref" = "${LAB_REF}"',
      "",
    ].join("\n");
    const { root, blanked } = await landed(home({ ".codex/config.toml": codex }), [row("agents/codex", ["~/.codex/config.toml"])], true);
    const text = read(root, ".codex/config.toml");
    expect(text).not.toContain("plain0");
    expect(parseToml(text)).toEqual({
      otel: { exporter: { "otlp-http": { endpoint: "https://otel.example.com", headers: { "x-var": "${V}" } } } },
      model_providers: { gw: { base_url: "https://gw.example.com/v1", http_headers: {} }, lab: { http_headers: { "X-Lab-Ref": "${LAB_REF}" } } },
    });
    expect(notes(blanked, "~/.codex/config.toml")).toMatch(/^the x-lab-team header, the X-Gw-Key header, the X-Lab-Key header blanked or left out/);
  });

  it("leaves out a Codex header, query secret, key and shell secret however the TOML spells it, and the file still parses", async () => {
    const codex = [
      'model_providers.top.http_headers.X-Top = "plain0top"',
      "",
      "[model_providers.gw]",
      'experimental_bearer_token = "plain0bearer"',
      'http_headers.X-Gw-Key = "plain0dotted"',
      'query_params = { "api-version" = "2025-01-01", "api-key" = "plain0query" }',
      "",
      "[model_providers.ml]",
      "http_headers = {",
      '  "X-Multi" = "plain0multi",',
      '  "X-Ref" = "${REF}",',
      "}",
      "",
      "[otel.exporter.otlp-grpc]",
      'headers.x-team = "plain0otel"',
      "",
      "[shell_environment_policy]",
      "set = {",
      '  GITHUB_TOKEN = "plain0shell",',
      '  PLAIN = "ok",',
      "}",
      "",
    ].join("\n");
    const { root, blanked } = await landed(home({ ".codex/config.toml": codex }), [row("agents/codex", ["~/.codex/config.toml"])], true);
    const text = read(root, ".codex/config.toml");
    expect(text).not.toContain("plain0");
    expect(parseToml(text)).toEqual({
      model_providers: {
        top: { http_headers: { "X-Top": "" } },
        gw: { experimental_bearer_token: "", http_headers: { "X-Gw-Key": "" }, query_params: { "api-version": "2025-01-01", "api-key": "" } },
        ml: { http_headers: { "X-Multi": "", "X-Ref": "${REF}" } },
      },
      otel: { exporter: { "otlp-grpc": { headers: { "x-team": "" } } } },
      shell_environment_policy: { set: { GITHUB_TOKEN: "", PLAIN: "ok" } },
    });
    expect(notes(blanked, "~/.codex/config.toml")).toMatch(/^the X-Top header, experimental_bearer_token, the X-Gw-Key header, api-key, the X-Multi header, the x-team header, GITHUB_TOKEN blanked or left out/);
  });

  it("leaves out the API key OpenCode sends for a provider", async () => {
    const opencode = { provider: { openai: { options: { baseURL: "https://gw.example.com/v1", apiKey: "plain0key" } }, ref: { options: { apiKey: "{env:REF_KEY}" } } } };
    const h = home({ ".config/opencode/opencode.json": `${JSON.stringify(opencode, null, 2)}\n` });
    const { root, blanked } = await landed(h, [row("agents/opencode", ["~/.config/opencode/opencode.json"])], true);
    expect(JSON.parse(read(root, ".config/opencode/opencode.json")).provider).toEqual({ openai: { options: { baseURL: "https://gw.example.com/v1" } }, ref: { options: { apiKey: "{env:REF_KEY}" } } });
    expect(notes(blanked, "~/.config/opencode/opencode.json")).toMatch(/^apiKey blanked or left out/);
  });

  it("leaves the secret names of Codex's shell environment out, inline or as a table", async () => {
    const cut = async (codex: string): Promise<{ text: string; note: string }> => {
      const { root, blanked } = await landed(home({ ".codex/config.toml": codex }), [row("agents/codex", ["~/.codex/config.toml"])], true);
      return { text: read(root, ".codex/config.toml"), note: notes(blanked, "~/.codex/config.toml") };
    };
    const inline = await cut(["[shell_environment_policy]", 'set = { OPENAI_API_KEY = "plain-openai-1", LAB_MODE = "fast" }', ""].join("\n"));
    expect(inline.text).toBe(["[shell_environment_policy]", 'set = { LAB_MODE = "fast" }', ""].join("\n"));
    expect(inline.note).toMatch(/^OPENAI_API_KEY blanked or left out/);
    const table = await cut(["[shell_environment_policy.set]", 'LAB_SECRET = "plain-secret-1"', 'LAB_REGION = "eu"', ""].join("\n"));
    expect(table.text).toBe(["[shell_environment_policy.set]", 'LAB_REGION = "eu"', ""].join("\n"));
    expect(table.note).toMatch(/^LAB_SECRET blanked or left out/);
  });

  it("leaves a login the person answered copy as the copy wrote it", async () => {
    const hosts = "github.com:\n    git_protocol: https\n    users:\n        me:\n    user: me\n";
    const h = home({ ".config/gh/hosts.yml": hosts });
    const gh = { ...row("logins/gh", ["~/.config/gh/hosts.yml", "Keychain: gh:github.com"], "logins"), choice: "copy" as const };
    const { root, blanked } = await landed(h, [gh], true, { platform: "darwin", secrets: new Map([["gh:github.com (me)", FAKE_GH]]) });
    expect(read(root, ".config/gh/hosts.yml")).toContain(FAKE_GH);
    expect(blanked).toEqual([]);
  });

  it("keeps today's copy for a computer the person owns", async () => {
    const h = home({ ".claude.json": `${JSON.stringify(CLAUDE_JSON, null, 2)}\n`, ".config/fish/fish_variables": "SETUVAR x:1\n" });
    const { root } = await landed(h, [row("agents/claude", ["~/.claude.json"]), row("shell/fish", ["~/.config/fish"], "shell")], false);
    const json = JSON.parse(read(root, `${CLAUDE_CONFIG_REL}/.claude.json`)) as Record<string, unknown>;
    expect(json.primaryApiKey).toBe(FAKE_KEY);
    expect(existsSync(join(root, ".config/fish/fish_variables"))).toBe(true);
  });
});
