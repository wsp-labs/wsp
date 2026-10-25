// SPDX-License-Identifier: AGPL-3.0-only
// The TS collect package's own answers on edge inputs, written as tests/fixtures/oracle/*.json for the Rust hard
// tests of the rest of collect (spec/*.md names which). Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-collect/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { CATALOG_AGENTS, MCP_AGENTS, checkoutArgs } from "../../../../packages/catalog/src/index.js";
import { readCheckout, readTurnServers } from "../../../../packages/collect/src/detect/turn-servers.js";
import { detectProjectMcp, outsideProjectLine } from "../../../../packages/collect/src/detect/mcp-project.js";
import { Manifest, ManifestEntry, parseManifest } from "../../../../packages/collect/src/manifest.js";
import { collect } from "../../../../packages/collect/src/collect.js";
import { bareUrls, fileSignals, keysSignal, nameSignal, pemSignal, topLevelKeys } from "../../../../packages/collect/src/credential-signals.js";
import { commandNames, installNames, splitCommands, withoutHeredocs } from "../../../../packages/collect/src/history/commands.js";
import { claudeSession } from "../../../../packages/collect/src/history/claude.js";
import { stem, timeOf } from "../../../../packages/collect/src/history/reader.js";
import { fileHistoryCache, readHistories } from "../../../../packages/collect/src/history/index.js";
import { usageOf, oneHolder } from "../../../../packages/collect/src/history/tally.js";
import { parseGhosttyDirectives, readGhosttyConfig, themeFor } from "../../../../packages/collect/src/ghostty-config.js";
import { seedMenu } from "../../../../packages/collect/src/seed.js";
import { scanProject } from "../../../../packages/collect/src/project/index.js";
import { fakeHost } from "../../../../packages/collect/test/fake-host.js";

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const write = (name: string, value: unknown): void => {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, name), `${JSON.stringify(value, null, 1)}\n`);
};

const ROW = { rung: "identity", id: "identity/git-user", label: "git name and email", paths: ["~/.gitconfig"], bytes: 512, default: "bring" };

it("manifest", () => {
  const issues = (r: { success: boolean; error?: { issues: { path: (string | number)[]; message: string }[] } }) => (r.success ? null : r.error!.issues.map(i => ({ path: i.path, message: i.message })));
  const entries: [string, unknown][] = [
    ["a fresh row", ROW],
    ["no rung", { ...ROW, rung: undefined }],
    ["an unknown rung", { ...ROW, rung: "fonts" }],
    ["a numeric rung", { ...ROW, rung: 3 }],
    ["an empty id", { ...ROW, id: "" }],
    ["an id outside its rung", { ...ROW, id: "shell/git-user" }],
    ["an id outside its rung and a skip required row", { ...ROW, id: "shell/x", required: true, default: "skip" }],
    ["every rung-only field on an identity row", { ...ROW, choice: "copy", linux: "yes", version: "1", login: "zsh", font: "Hack", sources: ["~/x"], required: true, default: "skip" }],
    ["a choice on a consent row", { ...ROW, rung: "agents", id: "agents/x", consent: true, choice: "copy" }],
    ["a choice on a consent: false row", { ...ROW, rung: "agents", id: "agents/x", consent: false, choice: "copy" }],
    ["an unknown choice", { ...ROW, rung: "logins", id: "logins/x", choice: "maybe" }],
    ["a default outside bring or skip", { ...ROW, default: "maybe" }],
    ["a negative size", { ...ROW, bytes: -1 }],
    ["a fractional size", { ...ROW, bytes: 1.5 }],
    ["a size as text", { ...ROW, bytes: "512" }],
    ["no paths", { ...ROW, paths: undefined }],
    ["a path that is a number", { ...ROW, paths: ["~/a", 7] }],
    ["an empty version on a tools row", { ...ROW, rung: "tools", id: "tools/x", version: "" }],
    ["an empty login on a shell row", { ...ROW, rung: "shell", id: "shell/x", login: "" }],
    ["an empty source on a shell row", { ...ROW, rung: "shell", id: "shell/x", sources: [""] }],
    ["an empty key", { ...ROW, keys: [""] }],
    ["an unknown signIn", { ...ROW, signIn: "magic" }],
    ["a pin with an empty tag", { ...ROW, rung: "tools", id: "tools/x", pin: { tag: "" } }],
    ["an unknown key and arch", { ...ROW, arch: "arm64", extra: 1 }],
    ["two bad fields", { ...ROW, bytes: -1, default: "maybe" }],
    ["null", null],
    ["an array", []],
  ];
  const manifests: [string, unknown][] = [
    ["one row", { entries: [ROW] }],
    ["entries as text", { entries: "nope" }],
    ["no entries", {}],
    ["null", null],
    ["a duplicate id", { entries: [ROW, ROW] }],
    ["a duplicate id three times", { entries: [ROW, ROW, ROW] }],
    ["a duplicate id beside a custom issue", { entries: [ROW, { ...ROW, id: "shell/x" }, ROW] }],
    ["a duplicate id beside a type issue", { entries: [ROW, { ...ROW, rung: 3 }, ROW] }],
    ["a bad second row", { entries: [ROW, { ...ROW, rung: 3 }] }],
    ["a duplicate id beside a size as text", { entries: [{ ...ROW, id: "identity/a" }, { ...ROW, id: "identity/a", bytes: "0" }, { ...ROW, id: "identity/a" }] }],
    ["a duplicate id on a skip required row", { entries: [{ ...ROW, id: "identity/a" }, { ...ROW, id: "identity/a", required: true, default: "skip" }] }],
    ["no rows", { entries: [] }],
  ];
  // Rows with every key their rung allows, given in no particular order, as parseManifest writes them back.
  const order = [
    { keys: ["k"], signIn: "oauth", detail: "d", consent: true, choice: "copy", bring: true, required: true, group: "g", reason: "r", volatile: ["~/v"], excludes: ["~/e"], arch: "arm64", unknown: 1, secrets: ["S"], ...ROW, rung: "logins", id: "logins/x" },
    { pin: { latest: true, tag: "v1" }, version: "1", linux: "yes", arch: "arm64", ...ROW, rung: "tools", id: "tools/x" },
    { sources: ["~/s"], font: "F", login: "zsh", ...ROW, rung: "shell", id: "shell/x" },
  ];
  write("manifest.json", {
    entries: entries.map(([name, data]) => ({ name, data, issues: issues(ManifestEntry.safeParse(data)) })),
    manifests: manifests.map(([name, data]) => {
      let error: string | null = null;
      try {
        parseManifest(data);
      } catch (e) {
        error = (e as Error).message;
      }
      return { name, data, issues: issues(Manifest.safeParse(data)), error };
    }),
    keyOrder: order.map(data => ({ data, parsed: JSON.stringify(parseManifest({ entries: [data] }).entries[0]) })),
  });
});

it("commands", () => {
  const lines = [
    "git log --oneline | head -5 && gh pr view; (cd x && pnpm test) || echo no", "sudo -u root -g wheel apt-get install -y jq", "xargs -n 1 -I {} rm {}", "nice -n 10 make", "doas -u me ls",
    "env -i FOO=1 BAR=2 node x.js", "! grep x y", "if test -f x; then cat x; fi", "while read l; do echo $l; done < f", "a=1 b=2", "export X=1; X=2 ls", "cat <<-'EOF'\nrm -rf /\n\tEOF\nls",
    'cat << "END"\nx\nEND\nwc', "echo \"$(date) and `whoami`\"", "echo '$(not run)'", "ls # comment | grep x\npwd", "ls#notacomment", "ls \\| grep", "echo \"a \\\" b\" | wc", "{ ls; pwd; }",
    "a & b", "a &>/dev/null; b 2>&1 | c", "f() { ls; }", "a\nb\n\nc", "", "   ", "python3 -m pip install x", "npm i -g @scope/pkg@1.2.3 other@latest", "pnpm add --global tsx", "bun install -g bunx-thing",
    "uv tool install 'ruff>=0.5' black[jupyter]", "pipx run cowsay hello", "pipx install --python 3.12 poetry==1.8", "cargo install --locked ripgrep", "go install golang.org/x/tools/gopls@v0.16.2",
    "go install example.com/mod/v2@latest", "brew reinstall --cask iterm2", "brew upgrade gh", "apt install -y curl", "npx create-next-app@latest my-app", "bunx cowsay", "uvx ruff check", "npm install -g",
    "command -V ls", "command ls", "builtin cd x", "type ls", "which -a node", "node -v x", "java -version", "/usr/bin/env python3 x", "time -p ls", "caffeinate -i make build", "exec ./run.sh",
    "${HOME}/bin/x", "echo ${a:-$(git status)}", "case $x in a) ls;; esac", "[[ -f x ]] && ls", ": noop; true; false || ls", "source ~/.zshrc && . ./env && ls", "eval \"$(brew shellenv)\"",
    "cd /tmp && rm -rf x", "kill %1", "sleep 1 &\nwait", "git commit -m \"fix #12\" # done", "echo 'unclosed | ls", "echo \"unclosed | ls", "a | b |& c", "nohup sh -c 'x' > /dev/null 2>&1 &",
  ];
  write("commands.json", lines.map(line => ({ line, split: splitCommands(withoutHeredocs(line)), heredocless: withoutHeredocs(line), names: commandNames(line), installs: installNames(line) })));
});

it("credential signals", () => {
  const names = [
    "credentials", "Credentials.json", ".credentials", "credentials.go", "auth.json", "Auth.json", "hosts.yml", "hosts.yaml", "access_token", "ACCESS-TOKEN.txt", "x.pem", "x.PEM", "x.key", "key", "KEY",
    "id_ed25519", "id_ed25519.pub", "id_", "id_RSA", ".netrc", "netrc", ".env", ".env.local", ".env.", ".envrc", ".env.example", ".npmrc", "npmrc", ".pypirc", ".git-credentials", "token", "tokens",
    "my.token.db", "tokenizer", "secret", "SECRETS", "top_secret_", "secretary", "a.lock", "secrets.lock", "x_history", "history", ".z", "z", "known_hosts", ".viminfo", "file.ts", "secret.ts", "secret.md",
    "secret.", ".secret", "a.b.tokens", "", ".", "..", "token.json.sample", "token.dist", "key.template",
  ];
  const texts = [
    '{"token":"x"}', '{"a":{"b":{"c":{"password":"x"}}}}', '{"a":{"b":{"password":"x"}}}', '{"a":{"b":{"c":"x"}}}', '{"apiKey":"x"}', '{"api-key":"x"}', '{"APIKey":"x"}', '{"clientSecret":"x"}',
    '{"github_token":"x"}', '{"my_private_key":"x"}', '{"privateKey":"x"}', '{"key":"x"}', '{"password":null}', '{"password":7}', '{"password":{}}', '{"token":"<x>"}', '{"token":"$X"}', '{"token":"${X}"}',
    '{"token":"$1"}', '{"token":" "}', "[1,2]", "null", "7", '"str"', "TOKEN = abc", "token=\n", "token = ''", 'token = ""', "token: |", "  - token: x", "token:x", "x.token: y", "a-b.c_d: v", "1abc: v",
    "//r/:_authToken=${REG_VAR}", "//r/:_authToken=abc", "# token: x", "token: a # b", "token: a#b", "token: 'a # b'", 'token: "x", ', "PASSWORD=hunter2\r", "-----BEGIN EC PRIVATE KEY-----", "  \n-----BEGIN PRIVATE KEY-----",
    "x\n-----BEGIN RSA PRIVATE KEY-----", "-----BEGIN PUBLIC KEY-----", "https://u:p@h", "https://u:p@h/path?q", "postgres://:p@h:5432/db", "ftp://u:${P}@h", "x https://a:b@c and http://d:e@f.", "https://a@b", "mailto:a:b@c",
    "HTTPS://U:P@H", "git+ssh://u:p@h", "https://u:p:q@h", "s3://key:secret@bucket", "http://u:<p>@h", "http://u:$P@h", "http://u:$1@h",
  ];
  write("credential-signals.json", {
    names: names.map(name => ({ name, signal: nameSignal(name) })),
    texts: texts.map(text => ({ text, keys: topLevelKeys(text), keysSignal: keysSignal(text), pem: pemSignal(text), bare: bareUrls(text) })),
  });
});

it("reader helpers", () => {
  const stamps = ["2026-09-06T10:01:00.000Z", "2026-09-06T10:01:00Z", "2026-09-06T10:01:00.5+05:30", "2026-09-06T10:01:00", "2026-09-06", "2026-13-01T00:00:00Z", "not a time", "", "1999-12-31T23:59:59.999Z"];
  const values: unknown[] = [...stamps, 1757152860000, null, undefined, {}];
  write("reader.json", {
    timeOf: values.map(v => ({ value: v === undefined ? "<undefined>" : v, at: timeOf(v) ?? null })),
    stem: ["/a/b/c.jsonl", "/a/b/.hidden", "/a/b/c.d.jsonl", "c", "/a/b/", "/a.b/c", "rollout-2026-06-01T10-00-00-t1.jsonl", ".jsonl"].map(path => ({ path, stem: stem(path) })),
    claudeSession: ["-p/abc.jsonl", "-p/abc", "-p/abc/subagents/a.jsonl", "-p/_cleared_sessions/old.jsonl", "-p/_cleared_sessions", "-p/_cleared_sessions/x/y.jsonl", "-p", "", "-p/.jsonl", "-p/a.b.jsonl"].map(rel => ({ rel, session: claudeSession(rel) ?? null })),
  });
});

it("tally", () => {
  const b = (session: string, at: number | undefined, commands: Record<string, number>, folder?: string) => ({ session, ...(folder !== undefined ? { folder } : {}), ...(at !== undefined ? { at } : {}), calls: Object.values(commands).reduce((s, n) => s + n, 0), commands, installs: {} });
  const held = [
    { holder: "A", buckets: [b("s1", 5, { gh: 1 }), b("s2", undefined, { ls: 2 })] },
    { holder: "B", buckets: [b("s1", 5, { git: 1 }), b("s2", 1, { ls: 1 })] },
    { holder: "C", buckets: [b("s1", 9, { jq: 1 }), b("s3", undefined, { b: 1, a: 1 })] },
    { holder: "D", buckets: [b("s3", undefined, { c: 1 })] },
  ];
  const kept = oneHolder(held);
  const usage = usageOf(kept);
  const plain = (m: Map<string, unknown>) => [...m];
  write("tally.json", { held, kept, usage: { sessions: usage.sessions, calls: usage.calls, commands: plain(usage.commands), installs: plain(usage.installs), tools: plain(usage.tools) } });
});

it("history cache file", async () => {
  const line = (o: unknown): string => JSON.stringify(o);
  const claudeLine = (sessionId: string, at: string | undefined, command: string) =>
    line({ type: "assistant", cwd: "/Users/dev/proj", sessionId, ...(at !== undefined ? { timestamp: at } : {}), message: { role: "assistant", content: [{ type: "tool_use", id: "tool-01", name: "Bash", input: { command } }] } });
  const files = {
    "~/.claude/projects/-Users-dev-proj/s1.jsonl": [claudeLine("s1", "2026-09-06T10:01:00.000Z", "gh pr view && brew install jq"), claudeLine("s1", undefined, "gh pr list")].join("\n"),
    "~/.claude/projects/-Users-dev-proj/s2.jsonl": claudeLine("s2", undefined, "pnpm test"),
  };
  const dir = mkdtempSync(join(tmpdir(), "wsp-oracle-"));
  const path = join(dir, "history-cache.json");
  const agents = CATALOG_AGENTS.filter(a => a.id === "claude");
  await readHistories(fakeHost({ files, mtimes: { "~/.claude/projects/-Users-dev-proj/s2.jsonl": 1500.25 } }), agents, { cache: fileHistoryCache(path) });
  write("history-cache.json", { files, text: readFileSync(path, "utf8") });
});

it("collect", async () => {
  const LAPTOP = fakeHost({
    files: {
      "~/.gitconfig": 512, "~/.ssh/config": 1200, "~/.ssh/id_ed25519": 400, "~/.ssh/id_ed25519.pub": 100, "~/.ssh/known_hosts": 3000, "~/.zshrc": 3000, "~/.zshenv": 200, "~/.config/starship.toml": 900,
      "~/.tmux.conf": 600, "~/.zsh_history": 800_000, "~/.config/nvim/init.lua": 120_000, "~/Library/Application Support/Code/User/settings.json": 2000, "~/.config/mise/config.toml": 300,
      "~/.claude/settings.json": 400, "~/.claude/CLAUDE.md": 900, "~/.claude/.credentials.json": 800, "~/.codex/config.toml": 50, "~/.codex/auth.json": 900, "~/.config/gh/hosts.yml": 200,
      "~/.aws/config": 300, "~/go/bin/gopls": 1,
    },
    which: ["git", "brew", "mise", "npm", "claude", "codex", "go"],
    exec: {
      "git config --global --get user.name": "Dev Person\n", "git config --global --get user.email": "dev@example.com\n", "git config --global --get gpg.format": "ssh\n",
      "HOMEBREW_NO_AUTO_UPDATE=1 brew bundle dump --file=- --formula --taps --no-restart": 'tap "homebrew/bundle"\nbrew "gh"\nbrew "jq"\n',
      "npm ls -g --depth=0 --json": JSON.stringify({ dependencies: { npm: { version: "10" }, pnpm: { version: "9.12.0" } } }), "go version": "go version go1.23.1 darwin/arm64\n",
      "go version -m /Users/dev/go/bin/gopls": "x\n\tpath\tgolang.org/x/tools/gopls\n\tmod\tgolang.org/x/tools/gopls\tv0.16.2\th1:abc=\n", "security find-generic-password -s Claude Code-credentials": "found\n",
    },
  });
  write("collect.json", { text: JSON.stringify(await collect(LAPTOP)) });
});

it("ghostty", () => {
  const texts = ["﻿font-size = 12", "a=b=c", " = x", "key =", 'k = "q"', 'k = "a"b"', "k = 'a'", "#c = d", "  # c", "k=v\r", "k\t=\tv"];
  write("ghostty.json", {
    directives: texts.map(text => ({ text, directives: parseGhosttyDirectives(text) })),
    themeFor: ["Mocha", " Mocha ", "light:A,dark:B", "dark:B", "light:A", "light:A,Mocha", ":x", "a:b:c", "light: A , dark: B ", "light:,dark:B", ""].flatMap(value =>
      (["light", "dark"] as const).map(scheme => ({ value, scheme, theme: themeFor(value, scheme) })),
    ),
  });
});

it("file signals", async () => {
  const cases: [string, number, number, string | undefined, boolean][] = [
    [".env", 0, 0o600, "A=1", true], [".env", 20, 0o644, "  {} ", false], [".env", 20, 0o644, "[\n]", false], [".env", 20, 0o644, "API_KEY=x", false], ["settings.json", 50, 0o600, '{"theme":"dark"}', true],
    ["settings.json", 50, 0o600, "not json at all", true], ["settings.json", 50, 0o600, "not json at all", false], ["state.yml", 50, 0o600, "- a\n- b", true], ["notes.txt", 50, 0o600, "token: x", true],
    ["config", 50, 0o600, undefined, true], ["config", 50, 0o644, "token: x", false], ["big.json", 200 * 1024, 0o600, '{"token":"x"}', true], ["id_rsa", 1700, 0o600, "-----BEGIN OPENSSH PRIVATE KEY-----\n", false],
    ["x.png", 50, 0o600, "token: x", true], ["db.sqlite", 50, 0o600, "token=x", true], ["db", 50, 0o600, "token=x\nhttps://a:b@c", false], ["key.pub", 50, 0o600, "ssh-ed25519 AAAA", true],
    ["creds.toml", 102400, 0o600, "", true], ["creds.toml", 102401, 0o644, "", true],
  ];
  const outs = [];
  for (const [name, bytes, mode, text, shallow] of cases) {
    let reads = 0;
    const signals = await fileSignals(name, { bytes, mode }, async () => {
      reads += 1;
      return text;
    }, shallow);
    outs.push({ name, bytes, mode, text: text ?? null, shallow, signals: signals ?? null, reads });
  }
  write("file-signals.json", outs);
});

it("ghostty configs", async () => {
  const BUNDLED = "/Applications/Ghostty.app/Contents/Resources/ghostty/themes";
  const cases: { name: string; files: Record<string, string>; platform?: "linux" | "darwin"; scheme?: "light" | "dark"; xdgConfigHome?: string }[] = [
    { name: "values that do and do not parse", files: { "~/.config/ghostty/config": "font-size = 0\nfont-size = -2\nfont-size = 11\nfont-size = 1e1\nbackground-opacity = 1.5\nwindow-padding-x = 1,2,3\nwindow-padding-y = 3,\ncursor-style-blink = true\ncursor-style-blink = \nbackground-blur = macos-glass-regular\nbackground-blur-radius = 1.5\npalette = 0b1111=#010203\npalette = 0o7=#040506\npalette = 16=#070809\npalette = x=#0a0b0c\npalette = 3=#zzzzzz\nselection-background = #ABCDEF\ncursor-color = 123456\nforeground =  #a1b2c3  \ncursor-style = block_hollow\n" } },
    { name: "opacity floor and blur off", files: { "~/.config/ghostty/config": "background-opacity = -1\nbackground-blur = false\nwindow-padding-x = 0\nwindow-padding-y = 2 , 4\n" } },
    { name: "an empty config-file line drops the includes named before it", files: { "~/.config/ghostty/config": "config-file = a\nconfig-file =\nconfig-file = b\nfont-size = 1\n", "~/.config/ghostty/a": "font-size = 2\n", "~/.config/ghostty/b": "font-size = 3\n" } },
    { name: "includes absolute, home and nested, each once", files: { "~/.config/ghostty/config": "config-file = ~/g/one\nconfig-file = /etc/ghostty/two\nconfig-file = \"?quoted\"\n", "~/g/one": "config-file = sub/three\nfont-family = One\n", "~/g/sub/three": "font-family = Three\nconfig-file = ../one\n", "/etc/ghostty/two": "font-family = Two\n", "~/.config/ghostty/quoted": "font-family = Q\n" } },
    { name: "the theme line is the last one of all files, includes among them", files: { "~/.config/ghostty/config": "theme = A\nconfig-file = more\n", "~/.config/ghostty/more": "theme = B\n", "~/.config/ghostty/themes/A": "background = #000001\n", "~/.config/ghostty/themes/B": "background = #000002\n" } },
    { name: "a light-only theme read in the dark is no theme", files: { "~/.config/ghostty/config": "theme = light:A\n", [`${BUNDLED}/A`]: "background = #000001\n" }, scheme: "dark" },
    { name: "linux reads the shared themes in order", platform: "linux", files: { "~/.config/ghostty/config": "theme = T\n", "/usr/share/ghostty/themes/T": "background = #000003\n", "/usr/local/share/ghostty/themes/T": "background = #000004\n" } },
    { name: "a theme's palette under the config's own", files: { "~/.config/ghostty/config": "palette = 1=#111111\ntheme = T\n", [`${BUNDLED}/T`]: "palette = 1=#999999\npalette = 2=#222222\nfont-family = ThemeFont\n" } },
    { name: "a BOM, CRLF lines and a key with spaces", files: { "~/.config/ghostty/config": "﻿font-size = 14\r\nfont-family = \"Fira Code\"\r\n  background   =   #102030  \r\n" } },
  ];
  const outs = [];
  for (const c of cases) {
    const host = fakeHost({ files: c.files, ...(c.platform !== undefined ? { platform: c.platform } : {}), ...(c.xdgConfigHome !== undefined ? { xdgConfigHome: c.xdgConfigHome } : {}) });
    outs.push({ ...c, config: await readGhosttyConfig(host, c.scheme ?? "dark") });
  }
  write("ghostty-configs.json", outs);
});

it("seed menus", async () => {
  const F = "/Users/dev/app";
  const git = (...args: string[]): string => ["git", "-C", F, ...args].join(" ");
  const cases: { name: string; folder: string; exec: Record<string, string>; files?: Record<string, number> }[] = [
    { name: "a folder git answers nothing for", folder: F, exec: {} },
    {
      name: "a trailing slash, du lines that do not parse and a negative remainder", folder: `${F}/`,
      exec: {
        [git("ls-files", "-z", "-o", "-i", "--exclude-standard", "--directory").replace(`${F} `, `${F}/ `)]: [".claude/", "b.log", "Zeta/", "alpha/"].join("\0"),
        [git("ls-files", "-z", "-o", "-i", "--exclude-standard", "--", ".claude").replace(`${F} `, `${F}/ `)]: [".claude/settings.local.json", ".claude/x.txt"].join("\0") + "\0",
        [`du -sk ${F}/.claude ${F}/b.log ${F}/Zeta ${F}/alpha ${F}/.claude/settings.local.json`]: [`4\t${F}/.claude`, `x\t${F}/b.log`, `8 ${F}/Zeta`, `12\t/elsewhere/alpha`, `16\t${F}/.claude/settings.local.json`, ""].join("\n"),
        [git("rev-parse", "--abbrev-ref", "HEAD").replace(`${F} `, `${F}/ `)]: "main\nextra\n",
        [git("rev-parse", "--abbrev-ref", "@{upstream}").replace(`${F} `, `${F}/ `)]: "origin/main",
        [git("merge-base", "HEAD", "origin/main").replace(`${F} `, `${F}/ `)]: "abc",
        [git("rev-list", "--count", "abc..HEAD").replace(`${F} `, `${F}/ `)]: "two",
        [git("status", "--porcelain").replace(`${F} `, `${F}/ `)]: "\n M a\n\n   \n?? b\n",
      },
    },
    {
      name: "a remote default branch with nothing ahead and no upstream", folder: F,
      exec: { [git("symbolic-ref", "refs/remotes/origin/HEAD")]: "refs/remotes/origin/trunk\n", [git("merge-base", "HEAD", "origin/trunk")]: "def\n", [git("rev-list", "--count", "def..HEAD")]: "5\n", [git("remote", "get-url", "origin")]: "  git@github.com:o/r.git  \n" },
    },
  ];
  const outs = [];
  for (const c of cases) {
    const host = fakeHost({ exec: c.exec, ...(c.files !== undefined ? { files: c.files } : {}) });
    const plan = await seedMenu(host, c.folder, { claudeStateHome: "/Users/dev/.claude" });
    outs.push({ ...c, plan, text: JSON.stringify(plan), calls: host.calls });
  }
  write("seed.json", outs);
});

it("project scans", async () => {
  const P = "/Users/dev/proj";
  const cases: Record<string, string>[] = [
    { "package.json": "not json" },
    { "package.json": "[1,2]" },
    { "package.json": JSON.stringify({ packageManager: "", engines: { node: 22 }, scripts: { a: 1, b: "VAR=1 vercel deploy" } }) },
    { "package.json": JSON.stringify({ packageManager: "@yarnpkg/cli@4" }) },
    { ".tool-versions": "nodejs\n  python 3.12 3.11 # pinned\n#all\n", ".mise.toml": "[tools]\n'node' = { version = '22' }\nrust = 'stable'\n[tools.extra]\nzig = '0.13'\n" },
    { ".github/workflows/a.yml": "steps:\n  - uses: 'actions/setup-node@v4'\n  - run: >-\n      pnpm i\n      gh pr view\n  - run:\n  - uses: oven-sh/setup-bun@v2\n  - uses: astral-sh/setup-uv@v3\n  - uses: actions/checkout@v4\n", ".github/workflows/b.yaml": "  - run: |\n      docker build .\n    env: {}\n" },
    { "apps/web/package.json": JSON.stringify({ scripts: { e2e: "playwright test" } }), "packages/x/README.md": "no", "Dockerfile": "FROM x" },
  ];
  const outs = [];
  for (const files of cases) {
    const host = fakeHost({ files: Object.fromEntries(Object.entries(files).map(([k, v]) => [`${P}/${k}`, v])) });
    outs.push({ files, scan: await scanProject(host, P) });
  }
  write("project.json", outs);
});

it("turn servers and project servers", async () => {
  const P = "/Users/dev/proj";
  const claude = MCP_AGENTS.find(a => a.id === "claude")!;
  const codex = MCP_AGENTS.find(a => a.id === "codex")!;
  const rev = `git ${checkoutArgs(P).join(" ")}`;
  const files = {
    "~/.claude.json": JSON.stringify({ mcpServers: { home: { command: "npx", args: ["-y", "home-mcp"] } }, projects: { [P]: { mcpServers: { mine: { command: "/Users/dev/bin/mine", args: ["--db", "/Users/dev/data/x.db"] }, off: { command: "node", args: ["s.js"] } }, disabledMcpjsonServers: ["shared"] } } }),
    [`${P}/.mcp.json`]: JSON.stringify({ mcpServers: { shared: { command: "npx", args: ["shared-mcp"] }, tracked: { type: "http", url: "https://t.example/mcp" } } }),
    "~/.codex/config.toml": `[projects."${P}"]\ntrust_level = "trusted"\n`,
    [`${P}/.codex/config.toml`]: `[mcp_servers.local]\ncommand = "uvx"\nargs = ["local-mcp"]\n`,
  };
  const exec = { [rev]: `${P}\n${P}/.git\n`, [`git -C ${P} ls-files -- .mcp.json`]: ".mcp.json\n" };
  const host = () => fakeHost({ files, exec });
  const h1 = host();
  const turn = await readTurnServers(h1, claude, P);
  const h2 = host();
  const own = await readTurnServers(h2, claude, h2.home, { own: true });
  const h3 = host();
  const project = await detectProjectMcp(h3, P, { agents: [claude, codex], keep: [] });
  const h4 = host();
  const checkout = await readCheckout(h4, P);
  const none = await readCheckout(fakeHost({ exec: { [rev]: "fatal: not a git repository\n" } }), P);
  write("turn-servers.json", {
    files, exec, rev,
    turn: { servers: turn, calls: h1.calls },
    own: { servers: own, calls: h2.calls },
    project: { result: project, calls: h3.calls },
    checkout, none,
    outsideLine: outsideProjectLine(["~/bin/mine", "~/data/x.db"]),
  });
});
