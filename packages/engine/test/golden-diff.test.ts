// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { INSTALLER_MOVED_LINE, LOGIN_CHOICES, type LoginChoice, type RecipeDigest } from "@wsp/protocol";
import { diffRecipes, isEmptyDiff, retiredBy, rowsToApply, type RecipeDiff } from "../src/golden-diff.js";
import type { RecipeEntry } from "../src/golden-import.js";

const row = (rung: string, id: string, over: Partial<RecipeEntry> = {}): RecipeEntry => ({ rung, id, label: id.slice(id.lastIndexOf("/") + 1), paths: [], bytes: 0, default: "bring", bring: true, ...over });
type DigestFile = RecipeDigest["files"][number];
/** A digested file; the two numbers stand in for the bytes that would be hashed. */
const file = (id: string, dest: string, size = 10, mtimeMs = 1000, path = `~/${dest}`): DigestFile => ({ id, dest, path, digest: `${size}:${mtimeMs}` });
/** The digest the seal writes: the ticked rows as ticks, the planned files as given; `rows` keeps the fixture rows for sizes. */
const snap = (entries: RecipeEntry[], files: DigestFile[] = []): RecipeDigest & { rows: RecipeEntry[] } => ({
  ticks: entries.filter(e => e.bring === true).map(e => ({ id: e.id, ...(e.choice !== undefined ? { choice: e.choice } : {}), ...(e.version !== undefined ? { version: e.version } : {}) })),
  files,
  rows: entries,
});
const bytesIn = (to: { rows: RecipeEntry[] }) => (id: string): number => to.rows.find(e => e.id === id)?.bytes ?? 0;
const EMPTY: RecipeDiff = { files: [], tools: [], agents: [], logins: [] };

describe("recipe diff", () => {
  const cases: { name: string; from: RecipeDigest; to: RecipeDigest; diff: RecipeDiff; apply: string[] }[] = [
    {
      name: "a file added",
      from: snap([row("shell", "shell/zshrc")], [file("shell/zshrc", ".zshrc")]),
      to: snap([row("shell", "shell/zshrc"), row("shell", "shell/starship")], [file("shell/zshrc", ".zshrc"), file("shell/starship", ".config/starship.toml")]),
      diff: { ...EMPTY, files: [{ id: "shell/starship", dest: ".config/starship.toml", change: "added" }] },
      apply: ["shell/starship"],
    },
    {
      name: "a file whose bytes changed",
      from: snap([row("shell", "shell/zshrc")], [file("shell/zshrc", ".zshrc", 10)]),
      to: snap([row("shell", "shell/zshrc")], [file("shell/zshrc", ".zshrc", 11)]),
      diff: { ...EMPTY, files: [{ id: "shell/zshrc", dest: ".zshrc", change: "changed" }] },
      apply: ["shell/zshrc"],
    },
    {
      name: "a volatile file whose bytes moved is no change: its tool rewrites it, or the machine renders it",
      from: snap([row("logins", "logins/gh", { choice: "copy" })], [{ ...file("logins/gh", ".config/gh/hosts.yml", 10, 1000), volatile: true }]),
      to: snap([row("logins", "logins/gh", { choice: "copy" })], [{ ...file("logins/gh", ".config/gh/hosts.yml", 10, 2000), volatile: true }]),
      diff: EMPTY,
      apply: [],
    },
    {
      name: "a file removed (its row unticked)",
      from: snap([row("shell", "shell/zshrc"), row("shell", "shell/starship")], [file("shell/zshrc", ".zshrc"), file("shell/starship", ".config/starship.toml")]),
      to: snap([row("shell", "shell/zshrc"), row("shell", "shell/starship", { bring: false })], [file("shell/zshrc", ".zshrc")]),
      diff: { ...EMPTY, files: [{ id: "shell/starship", dest: ".config/starship.toml", change: "removed" }] },
      apply: [],
    },
    {
      name: "a file whose dest moved is a removal and an addition",
      from: snap([row("agents", "agents/claude")], [file("agents/claude", ".claude", 10, 1000, "~/.claude")]),
      to: snap([row("agents", "agents/claude")], [file("agents/claude", ".claude-cfg", 10, 1000, "~/.claude")]),
      diff: { ...EMPTY, files: [{ id: "agents/claude", dest: ".claude-cfg", change: "added" }, { id: "agents/claude", dest: ".claude", change: "removed" }] },
      apply: ["agents/claude"],
    },
    {
      name: "a tool added",
      from: snap([]),
      to: snap([row("tools", "tools/brew/jq")]),
      diff: { ...EMPTY, tools: [{ id: "tools/brew/jq", label: "jq", change: "added" }] },
      apply: ["tools/brew/jq"],
    },
    {
      name: "a tool's pin changed",
      from: snap([row("tools", "tools/npm/bun", { version: "1.4.0" })]),
      to: snap([row("tools", "tools/npm/bun", { version: "1.5.0" })]),
      diff: { ...EMPTY, tools: [{ id: "tools/npm/bun", label: "bun", change: "changed", from: "1.4.0", to: "1.5.0" }] },
      apply: ["tools/npm/bun"],
    },
    {
      name: "a tool removed",
      from: snap([row("tools", "tools/brew/jq"), row("tools", "tools/npm/bun", { version: "1.4.0" })]),
      to: snap([row("tools", "tools/brew/jq")]),
      diff: { ...EMPTY, tools: [{ id: "tools/npm/bun", label: "bun", change: "removed", from: "1.4.0" }] },
      apply: [],
    },
    {
      name: "an agent added",
      from: snap([row("agents", "agents/claude")]),
      to: snap([row("agents", "agents/claude"), row("agents", "agents/codex")]),
      diff: { ...EMPTY, agents: [{ id: "agents/codex", label: "codex", change: "added" }] },
      apply: ["agents/codex"],
    },
    {
      name: "an agent removed",
      from: snap([row("agents", "agents/claude"), row("agents", "agents/codex")]),
      to: snap([row("agents", "agents/claude")]),
      diff: { ...EMPTY, agents: [{ id: "agents/codex", label: "codex", change: "removed" }] },
      apply: [],
    },
    {
      name: "a login now copied",
      from: snap([row("logins", "logins/gh", { choice: "machine" })]),
      to: snap([row("logins", "logins/gh", { choice: "copy" })], [file("logins/gh", ".config/gh/hosts.yml")]),
      diff: { ...EMPTY, files: [{ id: "logins/gh", dest: ".config/gh/hosts.yml", change: "added" }], logins: [{ id: "logins/gh", label: "gh", from: "machine", to: "copy" }] },
      apply: ["logins/gh"],
    },
    {
      name: "a login no longer copied takes its files off",
      from: snap([row("logins", "logins/gh", { choice: "copy" })], [file("logins/gh", ".config/gh/hosts.yml"), { id: "logins/gh", dest: ".config/gh/hosts.yml", path: "Keychain: gh:github.com", digest: "k", volatile: true }]),
      to: snap([row("logins", "logins/gh", { choice: "machine" })]),
      // The file and the Keychain secret land on one guest path, so one removal takes both off.
      diff: { ...EMPTY, files: [{ id: "logins/gh", dest: ".config/gh/hosts.yml", change: "removed" }], logins: [{ id: "logins/gh", label: "gh", from: "copy", to: "machine" }] },
      apply: [],
    },
    {
      name: "a login row unticked",
      from: snap([row("logins", "logins/codex", { choice: "copy" })]),
      to: snap([row("logins", "logins/codex", { bring: false, choice: "copy" })]),
      diff: { ...EMPTY, logins: [{ id: "logins/codex", label: "codex", from: "copy" }] },
      apply: [],
    },
    {
      name: "a ticked file gone from this computer is kept on the golden with a note, never removed",
      from: snap([row("shell", "shell/zshrc"), row("shell", "shell/starship")], [file("shell/zshrc", ".zshrc"), file("shell/starship", ".config/starship.toml")]),
      to: snap([row("shell", "shell/zshrc"), row("shell", "shell/starship")], [file("shell/zshrc", ".zshrc")]),
      diff: { ...EMPTY, files: [{ id: "shell/starship", dest: ".config/starship.toml", change: "missing" }] },
      apply: [],
    },
    {
      name: "the same recipe is no change, whatever the row order or the hash",
      from: snap([row("shell", "shell/zshrc"), row("tools", "tools/brew/jq")], [file("shell/zshrc", ".zshrc")]),
      to: snap([row("tools", "tools/brew/jq"), row("shell", "shell/zshrc")], [file("shell/zshrc", ".zshrc")]),
      diff: EMPTY,
      apply: [],
    },
  ];

  it.each(cases)("$name", ({ from, to, diff, apply }) => {
    const got = diffRecipes(from, to);
    expect(got).toEqual(diff);
    expect([...rowsToApply(got)].sort()).toEqual(apply.sort());
    expect(isEmptyDiff(got)).toBe(diff === EMPTY);
    const dropped = diff.files.filter(f => f.change === "removed").map(f => f.id).concat(diff.tools.filter(t => t.change === "removed").map(t => t.id), diff.agents.filter(a => a.change === "removed").map(a => a.id));
    // A row this delta ships again is not retired, whatever else the diff says about it: a dest that moved is a
    // removal and an addition of one id, and the id stays in the recipe.
    expect(retiredBy(got).map(r => r.id)).toEqual(dropped.filter(id => !rowsToApply(got).has(id)));
  });

  it("an unticked row on either side is not a tool, agent or login of that side", () => {
    const from = snap([row("tools", "tools/brew/jq", { bring: false }), row("agents", "agents/codex", { bring: false })]);
    const to = snap([row("tools", "tools/brew/jq"), row("agents", "agents/codex")]);
    expect(diffRecipes(from, to)).toEqual({ ...EMPTY, tools: [{ id: "tools/brew/jq", label: "jq", change: "added" }], agents: [{ id: "agents/codex", label: "codex", change: "added" }] });
  });
});

describe("a tool's road, pin and install lines", () => {
  type Tick = RecipeDigest["ticks"][number];
  const ID = "tools/catalog/gh";
  const gh = (over: Partial<Tick>): RecipeDigest => ({ ticks: [{ id: ID, ...over }], files: [] });
  const pin = (tag: string, sha = "b") => ({ tag, sha256: sha.repeat(64) });
  const release = (over: Partial<Tick> = {}): RecipeDigest => gh({ road: "release", installer: "i".repeat(64), ...over });
  const changed = (why: string): RecipeDiff => ({ ...EMPTY, tools: [{ id: ID, label: "gh", change: "changed", why }] });

  const cases: { name: string; from: RecipeDigest; to: RecipeDigest; diff: RecipeDiff }[] = [
    // A pin is what a build read back, never what the recipe asks: the sealed digest carries one and the plan none, and nothing moves.
    { name: "a pin on the sealed side and none on the plan is no change", from: release({ pin: pin("v2.86.0") }), to: release(), diff: EMPTY },
    { name: "the same pin folds away", from: release({ pin: pin("v2.86.0") }), to: release({ pin: pin("v2.86.0") }), diff: EMPTY },
    { name: "a pin that differs alone, tag or sum, is no change", from: release({ pin: pin("v2.86.0", "b") }), to: release({ pin: pin("v2.85.0", "c") }), diff: EMPTY },
    { name: "a latest mark on the sealed side is no change", from: release({ pin: { ...pin("v2.86.0"), latest: true } }), to: release(), diff: EMPTY },
    { name: "a road that moved is a changed row, in the roads' own words", from: gh({ road: "brew", installer: "a" }), to: gh({ road: "script", installer: "b" }), diff: changed("now by its own installer, was with Homebrew") },
    { name: "install lines that moved under the same road and pin are a changed row", from: release({ installer: "a" }), to: release({ installer: "b" }), diff: changed(INSTALLER_MOVED_LINE) },
    { name: "a version that moved is said by version, whatever moved with it", from: release({ version: "v2.86.0" }), to: release({ version: "v2.87.0", installer: "j" }), diff: { ...EMPTY, tools: [{ id: ID, label: "gh", change: "changed", from: "v2.86.0", to: "v2.87.0" }] } },
    { name: "a tick sealed before the digest carried a road moves nothing on its account", from: gh({}), to: release({ pin: pin("v2.86.0") }), diff: EMPTY },
    { name: "the same road and lines with no pin on either side is no change", from: release(), to: release(), diff: EMPTY },
  ];

  it.each(cases)("$name", ({ from, to, diff }) => {
    const got = diffRecipes(from, to);
    expect(got).toEqual(diff);
    expect([...rowsToApply(got)]).toEqual(diff === EMPTY ? [] : [ID]);
    expect(isEmptyDiff(got)).toBe(diff === EMPTY);
  });

});

describe("rows the recipe stopped asking for", () => {
  const retired = (entries: RecipeEntry[], files: DigestFile[] = [], previous: { id: string; name: string }[] = []) => retiredBy(diffRecipes(snap(entries, files), snap([])), previous);

  it("a file, a tool and an agent are recorded by id with the person's name for them, and nothing is uninstalled", () => {
    expect(retired([row("shell", "shell/zshrc"), row("tools", "tools/brew/yq"), row("agents", "agents/codex")], [file("shell/zshrc", ".zshrc")])).toEqual([
      { id: "shell/zshrc", name: "~/.zshrc" },
      { id: "tools/brew/yq", name: "yq" },
      { id: "agents/codex", name: "codex" },
    ]);
  });

  it("what the version being updated retired rides along, and a row ticked again leaves the list", () => {
    const was = [{ id: "tools/brew/yq", name: "yq" }, { id: "tools/npm/bun", name: "bun" }];
    const from = snap([row("tools", "tools/brew/jq")]);
    const to = snap([row("tools", "tools/brew/jq"), row("tools", "tools/npm/bun")]);
    expect(retiredBy(diffRecipes(from, to), was)).toEqual([{ id: "tools/brew/yq", name: "yq" }]);
  });

  it("a row retired twice is recorded once", () => {
    expect(retired([row("tools", "tools/brew/yq")], [], [{ id: "tools/brew/yq", name: "yq" }])).toEqual([{ id: "tools/brew/yq", name: "yq" }]);
  });
});

describe("a login answered with an API key or a token", () => {
  const keyed = (from: LoginChoice, to: LoginChoice) => diffRecipes(snap([row("logins", "logins/claude", { choice: from })]), snap([row("logins", "logins/claude", { choice: to })]));

  it("brings nothing and retires nothing of its own: the key is not a row with bytes", () => {
    expect([...rowsToApply(keyed("machine", "key"))]).toEqual([]);
    expect(retiredBy(keyed("machine", "key"))).toEqual([]);
  });

});
