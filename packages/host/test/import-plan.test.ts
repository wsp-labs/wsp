// SPDX-License-Identifier: AGPL-3.0-only
// The rows a computer's setup is planned from: this computer with a recipe written over it, and every sign-in
// skipped because the vault already holds it.
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Manifest, ManifestEntry } from "@wsp/collect";
import type { Recipe, SealedImage } from "@wsp/protocol";
import { brewTableFor, copyRows, wantsBrew } from "../src/import-plan.js";
import { ROOT, sourceFiles } from "../../protocol/test/source-files.js";
import { readFileSync } from "node:fs";
import { answeredRows, defaultAnswers, manifestFor } from "../src/init-recipe.js";
import { FIXTURE, RECIPE } from "./init-fixture.js";

/** The laptop the fixture describes, with the files its rows name actually there: the plan reads this computer, so
 * a row whose path is missing plans nothing and the two plans would agree for the wrong reason. */
function laptop(home: string): void {
  mkdirSync(join(home, ".codex"), { recursive: true });
  mkdirSync(join(home, ".claude"), { recursive: true });
  mkdirSync(join(home, ".config", "gh"), { recursive: true });
  writeFileSync(join(home, ".gitconfig"), "[user]\n\tname = Test\n");
  writeFileSync(join(home, ".zshrc"), "export PS1='$ '\n");
  writeFileSync(join(home, ".claude", "settings.json"), "{}\n");
  writeFileSync(join(home, ".codex", "auth.json"), '{"token":"not-a-real-token"}\n');
  writeFileSync(join(home, ".config", "gh", "hosts.yml"), "github.com:\n  oauth_token: not-a-real-token\n");
}

/** The recipe that laptop's seal was planned from: the fixture's own, with Codex on so its login row has a tool, and
 * both sign-ins answered copy, as the screens left them at the seal. A copy's build has to put those answers back to
 * skip itself; a plan that read the record's answers as they stand would carry the person's logins twice. */
const SMALL: Recipe = { ...RECIPE, rows: RECIPE.rows.map(r => (r.id === "codex" ? { ...r, on: true, signIn: "copy" as const } : r.id === "gh" ? { ...r, signIn: "copy" as const } : r)) };

const RECORD: SealedImage = {
  name: "default",
  version: 1,
  hash: "a".repeat(64),
  recipeHash: "h1",
  recipe: SMALL,
  logins: [{ name: "codex", state: "copied" }],
  sealedAt: "2026-09-12T00:00:00.000Z",
  sealedFrom: "laptop",
  vault: { sha256: "b".repeat(64), bytes: 400, paths: 2, takenAt: "2026-09-12T00:00:00.000Z" },
};

describe("the rows a setup is planned from", () => {
  let dir: string;
  let home: string;
  let statePath: string;
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "wsp-copy-recipe-"));
    home = join(dir, "home");
    statePath = join(dir, "state.json");
    mkdirSync(home, { recursive: true });
    laptop(home);
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it("sets every sign-in row to skip and leaves every other row's answer as the record left it", () => {
    const rows = copyRows(FIXTURE, { ...RECORD, recipe: SMALL }, { home, brew: new Map() });
    const logins = rows.filter(e => e.rung === "logins");
    expect(logins.length).toBeGreaterThan(0);
    expect(logins.every(e => e.choice === "skip" && e.bring !== true)).toBe(true);
    // The agents and tools the record's recipe ticks are ticked here, untouched by the sign-ins going.
    const ticked = new Set(rows.filter(e => e.bring).map(e => e.id));
    expect(ticked).toContain("agents/claude");
    expect(ticked).toContain("agents/codex");
    expect(ticked).toContain("tools/brew/gh");
  });

  it("ticks the tool a sign-in needs, even where the record's recipe carries that tool off", () => {
    // The seal ticks gh's row because its login was answered copy; the small recipe records the answer and not the
    // tick, so a copy that read the rows as written would land the login on a machine with no gh on it.
    const withoutTool: Recipe = { ...SMALL, rows: SMALL.rows.map(r => (r.id === "gh" ? { ...r, on: false } : r)) };
    const rows = copyRows(FIXTURE, { ...RECORD, recipe: withoutTool }, { home, brew: new Map() });
    const ticked = new Set(rows.filter(e => e.bring).map(e => e.id));
    expect(ticked).toContain("tools/brew/gh");
    // The sign-in itself is still skipped: the vault holds it.
    expect(rows.find(e => e.id === "logins/gh")).toMatchObject({ choice: "skip" });
    expect(ticked).not.toContain("logins/gh");
  });

});

describe("the one reading of whether this computer needs Homebrew read", () => {
  const withBrew = FIXTURE;
  const without: Manifest = { entries: FIXTURE.entries.filter(e => !e.id.startsWith("tools/brew/")) };

  it("is a brew row of this computer's own, and nothing else", () => {
    expect(wantsBrew(withBrew)).toBe(true);
    expect(wantsBrew(without)).toBe(false);
  });

  it("reads brew only then, answers an empty table where brew will not answer, and never asks twice", async () => {
    const asked: string[] = [];
    const table = new Map([["gh", { name: "gh", fullName: "gh", deps: [], macosOnly: false }]]);
    expect(await brewTableFor(withBrew, async () => (asked.push("read"), table))).toBe(table);
    expect(asked).toEqual(["read"]);
    // A computer with no brew row of its own is not read at all, and a brew that throws leaves an empty table
    // rather than failing the plan it was asked for.
    expect([...(await brewTableFor(without, async () => (asked.push("read again"), table)))]).toEqual([]);
    expect([...(await brewTableFor(withBrew, async () => { throw new Error("brew info failed"); }))]).toEqual([]);
    // A caller with no reader at all, which is every road that was handed none.
    expect([...(await brewTableFor(withBrew, undefined))]).toEqual([]);
    expect(asked).toEqual(["read"]);
  });

  it("is the reading the planner for a computer you own takes, so neither road reads this Mac its own way", () => {
    // The predicate lives once: a second copy is what let the two roads drift on which computer needs brew.
    const copies = sourceFiles().filter(rel => rel !== join("packages", "host", "src", "import-plan.ts") && /entries\.some\([^)]*BREW_ID_PREFIX/.test(readFileSync(join(ROOT, rel), "utf8")));
    expect(copies).toEqual([]);
  });
});
