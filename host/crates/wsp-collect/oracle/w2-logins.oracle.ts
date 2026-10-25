// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/logins.md: logins.ts on the edge inputs of the wave-2 audit (group f).
import { it } from "vitest";
import { apiKeyHelperOf, billedHere, claudeBills, claudeKeychainService, detectLogins, holdsClaudeLogin, loginDefault } from "../../../../packages/collect/src/detect/logins.js";
import { type Laptop, host, plain, settle, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const pure = (name: string, f: (...a: never[]) => unknown, ...args: unknown[]): Case => ({ name, args, out: plain((f as (...a: unknown[]) => unknown)(...args.map(a => (a === null ? undefined : a)))) });
const onHost = async (name: string, laptop: Laptop, run: (h: ReturnType<typeof host>) => unknown): Promise<Case> => {
  const h = host(laptop);
  const r = await settle(() => run(h));
  return { name, laptop, args: [], ...r, calls: h.calls };
};

const LOGIN = '{"claudeAiOauth":{"accessToken":"t"}}';
const SEC = "security find-generic-password -s ";

// Every candidate file of the LOGINS table and the catalog's key files, each with its own size.
const ALL: string[] = [
  "~/.config/gcloud/credentials.db", "~/.config/gcloud/access_tokens.db", "~/.config/gcloud/application_default_credentials.json", "~/.config/gcloud/configurations", "~/.config/gcloud/active_config", "~/.config/gcloud/legacy_credentials",
  "~/Library/Preferences/.wrangler/config/default.toml", "~/.config/.wrangler/config/default.toml", "~/.cloudflared/cert.pem",
  "~/Library/Application Support/com.vercel.cli/auth.json", "~/.config/com.vercel.cli/auth.json", "~/.vercel/auth.json", "~/.aws/credentials", "~/.aws/config", "~/.kube/config",
  "~/.codex/auth.json", "~/.gemini/oauth_creds.json", "~/.local/share/opencode/auth.json", "~/.pi/agent/auth.json", "~/.hermes/auth.json", "~/.config/gh/hosts.yml", "~/.hermes/.env",
];
const allFiles = Object.fromEntries(ALL.map((p, i) => [p, i + 1]));
const VERCEL = { "~/Library/Application Support/com.vercel.cli/auth.json": 100, "~/.vercel/auth.json": 5, "~/.config/com.vercel.cli/auth.json": 9 };

it("logins fixtures", async () => {
  const fixture: Record<string, Case[]> = {};

  fixture["login_default"] = ["gh", "kube", "hermes", "hermes-keys", "opencode", "gcloud", "codex", "claude", "some-new-tool", ""].map(id => pure(`default of ${JSON.stringify(id)}`, loginDefault, id));

  fixture["api_key_helper_of"] = [
    ["blank BOM", JSON.stringify({ apiKeyHelper: "﻿" })], ["blank NBSP", JSON.stringify({ apiKeyHelper: " " })], ["empty", JSON.stringify({ apiKeyHelper: "" })],
    ["blank tab and newline", JSON.stringify({ apiKeyHelper: "\t\n" })], ["padded helper comes back as written", JSON.stringify({ apiKeyHelper: " x " })], ["plain", JSON.stringify({ apiKeyHelper: "x" })],
    ["leading BOM in the text", '﻿{"apiKeyHelper":"x"}'], ["trailing comment", '{"apiKeyHelper":"x"} // c'], ["array helper", '{"apiKeyHelper":["x"]}'], ["a string", '"x"'], ["a number", "5"],
    ["empty text", ""], ["other key case", '{"apikeyhelper":"x"}'], ["null helper", '{"apiKeyHelper":null}'], ["duplicate key: the last wins", '{"apiKeyHelper":"a","apiKeyHelper":"b"}'],
    ["null", "null"], ["array", "[]"], ["not json", "not json"], ["empty object", "{}"],
  ].map(([n, t]) => pure(n!, apiKeyHelperOf, t)).concat([pure("no text", apiKeyHelperOf, null)]);

  fixture["holds_claude_login"] = [
    ["a blank token counts", '{"claudeAiOauth":{"accessToken":" "}}'], ["claudeAiOauth an array", '{"claudeAiOauth":[]}'], ["array", "[]"], ["string", '"x"'], ["number", "5"], ["true", "true"], ["empty text", ""],
    ["leading BOM", `﻿${LOGIN}`], ["claudeAiOauth a string", '{"claudeAiOauth":"x"}'], ["duplicate key: the last wins", '{"claudeAiOauth":{"accessToken":"t"},"claudeAiOauth":{"accessToken":""}}'],
    ["token an array", '{"claudeAiOauth":{"accessToken":["t"]}}'], ["trailing text", `${LOGIN} trailing`], ["a real login", '{"claudeAiOauth":{"accessToken":"placeholder"},"mcpOAuth":{}}'],
    ["only an mcp token", '{"mcpOAuth":{"x":{"accessToken":"t"}}}'], ["empty token", '{"claudeAiOauth":{"accessToken":""}}'], ["token a number", '{"claudeAiOauth":{"accessToken":5}}'],
    ["claudeAiOauth null", '{"claudeAiOauth":null}'], ["null", "null"], ["not json", "not json"],
  ].map(([n, t]) => pure(n!, holdsClaudeLogin, t)).concat([pure("no text", holdsClaudeLogin, null)]);

  fixture["claude_keychain_service"] = [
    pure("no dir", claudeKeychainService, null), pure("empty dir hashes the empty string", claudeKeychainService, ""), pure("a tilde dir hashes the raw text", claudeKeychainService, "~/cfg"),
    pure("composed spelling", claudeKeychainService, "/a/café"), pure("decomposed spelling gives the same item", claudeKeychainService, "/a/café"), pure("plain dir", claudeKeychainService, "/home/me/.claude-cfg"),
  ];

  const bills = (name: string, laptop: Laptop) => onHost(name, laptop, h => claudeBills(h));
  fixture["claude_bills"] = [
    await bills("a helper wins over a login file", { platform: "linux", files: { "~/.claude/settings.json": '{"apiKeyHelper":"x"}', "~/.claude/.credentials.json": LOGIN } }),
    await bills("an exported key wins over a login file", { platform: "linux", files: { "~/.bashrc": "export ANTHROPIC_API_KEY=x\n", "~/.claude/.credentials.json": LOGIN } }),
    await bills("a login file only", { platform: "linux", files: { "~/.claude/.credentials.json": LOGIN } }),
    await bills("nothing on linux: every rc file then the credentials file", { platform: "linux" }),
    await bills("nothing on darwin: every rc file then the Keychain", { platform: "darwin" }),
    await bills("a settings.json that is a folder", { platform: "linux", files: { "~/.claude/settings.json/x": 3 } }),
    await bills("darwin with a helper asks no Keychain and reads no rc file", { platform: "darwin", files: { "~/.claude/settings.json": '{"apiKeyHelper":"x"}' }, exec: { [`${SEC}Claude Code-credentials`]: "x" } }),
    await bills("darwin: a login file without a Keychain item is no login", { platform: "darwin", files: { "~/.claude/.credentials.json": LOGIN } }),
    await bills("linux: a Keychain answer is ignored and no security run is made", { platform: "linux", exec: { [`${SEC}Claude Code-credentials`]: "x" } }),
    await bills("an empty Keychain answer is a login", { platform: "darwin", exec: { [`${SEC}Claude Code-credentials`]: "" } }),
    await bills("reads stop at the first rc file that exports", { platform: "linux", files: { "~/.zshrc": "export ANTHROPIC_API_KEY=x\n", "~/.bashrc": "echo\n" } }),
    await bills("a config dir: settings still come from ~/.claude", { platform: "linux", claudeConfigDir: "/cfg", files: { "~/.claude/settings.json": '{"apiKeyHelper":"x"}' } }),
    await bills("a config dir: settings under the dir are ignored", { platform: "linux", claudeConfigDir: "/cfg", files: { "/cfg/settings.json": '{"apiKeyHelper":"x"}' } }),
    await bills("a tilde config dir is expanded for the credentials file", { platform: "linux", claudeConfigDir: "~/cfg", files: { "~/cfg/.credentials.json": LOGIN } }),
    await bills("an empty config dir reads /.credentials.json", { platform: "linux", claudeConfigDir: "", files: { "/.credentials.json": LOGIN, "~/.claude/.credentials.json": LOGIN } }),
    await bills("an empty config dir on darwin asks the hashed-empty service", { platform: "darwin", claudeConfigDir: "", exec: { [`${SEC}Claude Code-credentials-e3b0c442`]: "x" } }),
    await bills("a config dir on darwin asks its hashed service", { platform: "darwin", claudeConfigDir: "/home/me/.claude-cfg", exec: { [`${SEC}Claude Code-credentials`]: "x" } }),
  ];

  const billed = (name: string, laptop: Laptop) => onHost(name, laptop, h => billedHere(h));
  fixture["billed_here"] = [
    await billed("an rc export", { platform: "linux", files: { "~/.bashrc": "export ANTHROPIC_API_KEY=x\n" } }),
    await billed("a login file", { platform: "linux", files: { "~/.claude/.credentials.json": LOGIN } }),
    await billed("nothing", { platform: "linux" }),
  ];

  const rows = (name: string, laptop: Laptop) => onHost(name, laptop, h => detectLogins(h));
  const exported = (name: string, text: string, rc = "~/.zshrc") => rows(`export check: ${name}`, { platform: "linux", files: { [rc]: text } });
  fixture["detect_logins"] = [
    await rows("every candidate file on darwin plus op", { platform: "darwin", files: allFiles, which: ["op"] }),
    await rows("every candidate file on linux plus op", { platform: "linux", files: allFiles, which: ["op"] }),
    await rows("vercel on darwin: the platform path then the all path", { platform: "darwin", files: VERCEL }),
    await rows("vercel on linux: the platform path then the all path", { platform: "linux", files: VERCEL }),
    await rows("gh Keychain answer is the empty string", { platform: "darwin", files: { "~/.config/gh/hosts.yml": 5 }, exec: { [`${SEC}gh:github.com`]: "" } }),
    await rows("gh with no hosts.yml asks no gh Keychain item", { platform: "darwin", exec: { [`${SEC}gh:github.com`]: "x" } }),
    await rows("gh hosts.yml that is a folder", { platform: "darwin", files: { "~/.config/gh/hosts.yml/x": 9 }, exec: { [`${SEC}gh:github.com`]: "x" } }),
    await rows("claude on darwin by Keychain", { platform: "darwin", exec: { [`${SEC}Claude Code-credentials`]: "x" } }),
    await rows("claude on darwin by login file alone: no row", { platform: "darwin", files: { "~/.claude/.credentials.json": LOGIN } }),
    await rows("claude on linux by Keychain answer alone: no row", { platform: "linux", exec: { [`${SEC}Claude Code-credentials`]: "x" } }),
    await rows("claude with a helper on darwin: only the settings read", { platform: "darwin", files: { "~/.claude/settings.json": '{"apiKeyHelper":"x"}' }, exec: { [`${SEC}Claude Code-credentials`]: "x" } }),
    await rows("codex home elsewhere hides the default file", { platform: "darwin", codexHome: "/elsewhere", files: { "~/.codex/auth.json": 5 } }),
    await rows("codex home with a tilde", { platform: "darwin", codexHome: "~/c2", files: { "~/c2/auth.json": 5 } }),
    await rows("codex home empty reads /auth.json", { platform: "darwin", codexHome: "", files: { "/auth.json": 5, "~/.codex/auth.json": 5 } }),
    await rows("op alone on linux", { platform: "linux", which: ["op"] }),
    await rows("op alone on darwin", { platform: "darwin", which: ["op"] }),
    await exported("export in .zshrc", "export ANTHROPIC_API_KEY=sk\n"),
    await exported("commented export", "# export ANTHROPIC_API_KEY=sk\n"),
    await exported("bare export", "export ANTHROPIC_API_KEY\n"),
    await exported("inline env for a command", "ANTHROPIC_API_KEY=sk claude\n"),
    await exported("another variable that starts with the name", "export ANTHROPIC_API_KEY_2=sk\n"),
    await exported("fish set -gx in config.fish", "set -gx ANTHROPIC_API_KEY sk\n", "~/.config/fish/config.fish"),
    await exported("export in .zshenv", "export ANTHROPIC_API_KEY=sk\n", "~/.zshenv"),
    await exported("export in .profile", "export ANTHROPIC_API_KEY=sk\n", "~/.profile"),
    await exported("fish conf.d is not read", "set -gx ANTHROPIC_API_KEY sk\n", "~/.config/fish/conf.d/k.fish"),
    await exported("export with an empty value", "export ANTHROPIC_API_KEY=\n"),
    await exported("export among several assignments", "export A=1 ANTHROPIC_API_KEY=sk B=2\n"),
    await exported("empty rc file", ""),
  ];

  write("logins", fixture);
});
