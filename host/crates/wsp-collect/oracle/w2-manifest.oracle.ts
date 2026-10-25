// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/manifest.md: manifest.ts and collect.ts on the edge inputs of the wave-2 audit (group a).
// parse_entry / safe_parse_manifest cases: {name, args: [data], out: {ok: <parsed>, text} | {issues: [{path, message}]}}.
//   A case with "args_json" instead of "args" gives the data as JSON TEXT: parse it with serde_json::from_str, since
//   JSON.stringify of a number such as 1.0 would lose how it was written. The TS parsed that text with JSON.parse.
// parse_manifest cases: {name, args: [data], out: {ok, text} | {error: message}}.
// collect cases: {name, laptop, args: [], out: {ok: manifest, text} | {error}, on_rung: [[rung, count], ...], calls}.
import { it } from "vitest";
import { collect } from "../../../../packages/collect/src/collect.js";
import { Manifest, ManifestEntry, parseManifest } from "../../../../packages/collect/src/manifest.js";
import { type Laptop, host, plain, write } from "./w2-host.js";

type Case = Record<string, unknown>;
type Parsed = { success: boolean; data?: unknown; error?: { issues: { path: (string | number)[]; message: string }[] } };

const result = (r: Parsed): Case => (r.success ? { ok: plain(r.data), text: JSON.stringify(r.data) } : { issues: r.error!.issues.map(i => ({ path: i.path, message: i.message })) });
const ROW = { rung: "identity", id: "identity/x", label: "l", paths: ["~/a"], bytes: 1, default: "bring" };
const ROW_TEXT = (bytes: string): string => `{"rung":"identity","id":"identity/x","label":"l","paths":[],"bytes":${bytes},"default":"bring"}`;

const entry = (name: string, data: unknown): Case => ({ name, args: [data], ...result(ManifestEntry.safeParse(data) as Parsed) });
const entryText = (name: string, text: string): Case => ({ name, args_json: text, ...result(ManifestEntry.safeParse(JSON.parse(text)) as Parsed) });
const envelope = (name: string, data: unknown): Case => ({ name, args: [data], ...result(Manifest.safeParse(data) as Parsed) });
const parsed = (name: string, data: unknown): Case => {
  try {
    const m = parseManifest(data);
    return { name, args: [data], out: { ok: plain(m), text: JSON.stringify(m) } };
  } catch (e) {
    return { name, args: [data], out: { error: (e as Error).message } };
  }
};

const onHost = async (name: string, laptop: Laptop): Promise<Case> => {
  const h = host(laptop);
  const on_rung: [string, number][] = [];
  let out: Case;
  try {
    const m = await collect(h, { onRung: (r, c) => on_rung.push([r, c]) });
    out = { ok: plain(m), text: JSON.stringify(m) };
  } catch (e) {
    out = { error: (e as Error).message };
  }
  return { name, laptop, args: [], out, on_rung, calls: h.calls };
};

it("manifest fixtures", async () => {
  const fixture: Record<string, Case[]> = {};
  const withKey = (patch: object, base: object = ROW): object => ({ ...base, ...patch });

  const pe: Case[] = [];
  // JSON numbers
  for (const [n, t] of [["1.0", "1.0"], ["1e3", "1e3"], ["1e20", "1e20"], ["above 2^53 (the TS rounds it)", "9007199254740993"], ["-0", "-0"], ["0.0", "0.0"]]) pe.push(entryText(`bytes written as ${n}`, ROW_TEXT(t!)));
  for (const [n, v] of [["-0.5", -0.5], ["-1.5", -1.5], ["1.5", 1.5], ["-1", -1], ["true", true], ["null", null], ["a string", "1"]] as const) pe.push(entry(`bytes ${n}`, withKey({ bytes: v })));
  // rule 4: dirty runs the refine, aborted does not (a bad id makes the refine visible)
  const bad = { ...ROW, id: "x" };
  for (const [n, patch] of [
    ["bytes 1.5", { bytes: 1.5 }], ["bytes -1", { bytes: -1 }], ["bytes -1.5", { bytes: -1.5 }], ["bytes a string", { bytes: "1" }], ["label a number", { label: 5 }], ["paths [1]", { paths: [1] }], ["paths a string", { paths: "x" }],
    ["sources [\"\"]", { sources: [""] }], ["secrets [1]", { secrets: [1] }], ["default a bad string", { default: "no" }], ["default a number", { default: 3 }],
    ["pin.latest false", { pin: { tag: "v", latest: false } }], ["pin.tag empty", { pin: { tag: "" } }], ["pin.sha256 empty", { pin: { tag: "v", sha256: "" } }], ["pin.arch bad", { pin: { tag: "v", arch: "arm64" } }],
    ["an unknown key", { zzz: 1 }], ["version empty", { version: "" }], ["keys [\"\"]", { keys: [""] }], ["signIn bad", { signIn: "magic" }], ["choice bad", { choice: "maybe" }], ["linux bad", { linux: "maybe" }],
    ["required a string", { required: "yes" }], ["id empty", { id: "" }],
  ] as [string, object][]) pe.push(entry(`bad id plus ${n}`, withKey(patch, bad)));
  const { label: _l, ...noLabel } = bad;
  pe.push(entry("bad id with the label missing", noLabel));
  // null on every optional key
  for (const k of ["excludes", "volatile", "reason", "group", "required", "bring", "choice", "linux", "version", "pin", "consent", "detail", "secrets", "login", "font", "sources", "signIn", "keys"]) pe.push(entry(`${k}: null`, withKey({ [k]: null })));
  // containers and items
  pe.push(entry("paths an array of mixed items", withKey({ paths: [1, "a", null, ["x"]] })));
  pe.push(entry("paths an object", withKey({ paths: {} })));
  pe.push(entry("excludes a string", withKey({ excludes: "x" })));
  pe.push(entry("secrets a number and an empty string", withKey({ secrets: [1, ""] })));
  pe.push(entry("volatile an object item", withKey({ volatile: [{}] })));
  pe.push(entry("label a number", withKey({ label: 5 })));
  pe.push(entry("id a number", withKey({ id: 5 })));
  // enums by received type
  for (const v of [1, null, ["bring"], true, {}, "", "Bring", "it's", "a'b'c"]) pe.push(entry(`default ${JSON.stringify(v)}`, withKey({ default: v })));
  for (const v of ["", "Identity"]) pe.push(entry(`rung ${JSON.stringify(v)}`, withKey({ rung: v })));
  pe.push(entry("linux a number", withKey({ rung: "tools", id: "tools/x", linux: 1 })));
  pe.push(entry("linux Yes", withKey({ rung: "tools", id: "tools/x", linux: "Yes" })));
  pe.push(entry("signIn a number", withKey({ signIn: 1 })));
  pe.push(entry("choice empty", withKey({ rung: "logins", id: "logins/x", choice: "" })));
  pe.push(entry("choice a number", withKey({ rung: "logins", id: "logins/x", choice: 3 })));
  // pin
  for (const [n, pin] of [["empty", {}], ["a string", "x"], ["an array", []], ["tag a number", { tag: 1 }], ["sha256 empty", { tag: "v", sha256: "" }], ["arch bad", { tag: "v", arch: "arm64" }], ["arch a number", { tag: "v", arch: 1 }],
    ["latest false", { tag: "v", latest: false }], ["latest a string", { tag: "v", latest: "yes" }], ["latest null", { tag: "v", latest: null }], ["tag and sha256 empty", { tag: "", sha256: "" }], ["sha256 a number, no tag", { sha256: 1 }]] as [string, unknown][])
    pe.push(entry(`pin ${n}`, withKey({ pin })));
  pe.push(entry("pin key order and stripping", withKey({ pin: { latest: true, arch: "aarch64", sha256: "ab", tag: "v1", extra: 1 } })));
  pe.push(entry("pin with only tag and sha256", withKey({ pin: { sha256: "ab", tag: "v1" } })));
  // refine order
  pe.push(entry("shape issues then refine issues", withKey({ rung: "tools", id: "x", choice: "copy", required: true, default: "skip", login: "", font: "", sources: ["a", ""], version: "" })));
  pe.push(entry("every refine at once", withKey({ id: "shell/x", choice: "copy", linux: "yes", version: "1", login: "zsh", font: "F", sources: ["a"], required: true, default: "skip" })));
  pe.push(entry("empty id, empty source, required skip", withKey({ id: "", sources: [""], required: true, default: "skip" })));
  for (const id of ["identity/", "identity//x", "identity", "Identity/x", " identity/x"]) pe.push(entry(`id ${JSON.stringify(id)}`, withKey({ id })));
  pe.push(entry("shape issues come in schema order, not input order", { default: "no", bytes: -1, paths: 1, label: 1, id: 1, rung: 1 }));
  pe.push(entry("every key missing", {}));
  pe.push(entry("only the rung", { rung: "identity" }));
  pe.push(entry("rung and an unprefixed id only", { rung: "identity", id: "x" }));
  pe.push(entry("rung and a good id only", { rung: "identity", id: "identity/x" }));
  for (const [n, v] of [["a string", "x"], ["a number", 3], ["true", true], ["null", null], ["an array", []], ["an array of a row", [ROW]]] as [string, unknown][]) pe.push(entry(`a row that is ${n}`, v));
  fixture["parse_entry"] = pe;

  const sp: Case[] = [];
  for (const [n, v] of [["an array", []], ["a string", "x"], ["a number", 3], ["true", true], ["null", null]] as [string, unknown][]) sp.push(envelope(`an envelope that is ${n}`, v));
  sp.push(envelope("entries null", { entries: null }));
  sp.push(envelope("entries an object", { entries: {} }));
  sp.push(envelope("entries missing", {}));
  sp.push(envelope("entries holding null", { entries: [null] }));
  sp.push(envelope("entries holding an array", { entries: [[]] }));
  sp.push(envelope("a string row between two good ones", { entries: [ROW, "x", ROW] }));
  sp.push(envelope("extra envelope keys are dropped", { entries: [], x: 1 }));
  sp.push(envelope("every later duplicate is reported", { entries: [ROW, ROW, { ...ROW, id: "identity/y" }, { ...ROW, id: "identity/y" }, ROW] }));
  sp.push(envelope("ids are case sensitive", { entries: [ROW, { ...ROW, id: "IDENTITY/X" }] }));
  sp.push(envelope("the same id under another rung", { entries: [ROW, { ...ROW, rung: "shell", id: "identity/x" }] }));
  sp.push(envelope("a duplicate after a too-small number still runs the check, after the row issues", { entries: [ROW, ROW, { ...ROW, id: "identity/z", bytes: -1 }] }));
  sp.push(envelope("a duplicate after a type error is not checked", { entries: [ROW, ROW, { ...ROW, id: "identity/z", bytes: "1" }] }));
  sp.push(envelope("a duplicate after an id outside its rung", { entries: [ROW, ROW, { ...ROW, id: "identity/z", rung: "tools" }] }));
  sp.push(envelope("an item type error inside a row", { entries: [{ ...ROW, paths: [1] }] }));
  fixture["safe_parse_manifest"] = sp;

  fixture["parse_manifest"] = [
    parsed("an array", []),
    parsed("one duplicate", { entries: [ROW, ROW] }),
    parsed("an empty id", { entries: [{ ...ROW, id: "" }] }),
    parsed("an empty row", { entries: [{}] }),
    parsed("two item errors", { entries: [{ ...ROW, paths: [1, 2] }] }),
    parsed("six shape issues joined", { entries: [{ default: "no", bytes: -1, paths: 1, label: 1, id: 1, rung: 1 }] }),
    parsed("an empty list", { entries: [] }),
    parsed("extra envelope keys", { entries: [ROW], x: 1 }),
  ];

  // collect over a fake host
  const FIXTURE: Laptop = {
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
  };
  fixture["collect"] = [
    await onHost("the fixture laptop: rows, counts and the whole call sequence", FIXTURE),
    await onHost("duplicate brew formulae: the parse fails after every rung ran", { which: ["brew"], exec: { "HOMEBREW_NO_AUTO_UPDATE=1 brew bundle dump --file=- --formula --taps --no-restart": 'brew "gh"\nbrew "gh"\nbrew "jq"\n' } }),
    await onHost("the shell rung: rows, sources, then the font row, counted together", { terminal: "ghostty", shell: "/bin/zsh", files: { "~/.zshrc": "source ~/.cargo/env\nexport A=1\n", "~/.cargo/env": 10, "~/.config/ghostty/config": "font-family = Hack\n" } }),
    await onHost("the agents rung: agent rows, then mcp rows", { terminal: "ghostty", shell: "/bin/zsh", which: ["claude", "codex"], files: { "~/.zshrc": 300, "~/.config/ghostty/config": "font-family = Hack\n", "~/.claude/settings.json": 10, "~/.claude.json": JSON.stringify({ mcpServers: { s1: { command: "npx", args: ["x"] } } }), "~/.codex/config.toml": 5 } }),
    await onHost("an empty linux host", { platform: "linux" }),
    await onHost("an empty darwin host", {}),
  ];

  write("manifest", fixture);
}, 60000);
