// SPDX-License-Identifier: AGPL-3.0-only
// The test budget: a package past the tests or seconds scripts/test-budget.json allows fails the laws until a line in
// its over list names the package, the new number and why. Driven in throwaway repositories, with this tree's script
// run in each and the budget it is given.
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";

const SCRIPT = fileURLToPath(new URL("../../../scripts/test-budget.mjs", import.meta.url));

let dirs: string[] = [];
afterEach(() => {
  for (const dir of dirs) rmSync(dir, { recursive: true, force: true });
  dirs = [];
});

type Budget = { packages: Record<string, { tests: number; seconds: number }>; over?: object[]; timed?: Record<string, object> };

/** A repository holding these files, untracked as a builder's new ones are, and the budget it is given. */
function repo(files: Record<string, string>, budget: Budget): string {
  const dir = mkdtempSync(join(tmpdir(), "wsp-test-budget-"));
  dirs.push(dir);
  execFileSync("git", ["init", "-q", dir]);
  for (const [path, text] of Object.entries({ ...files, "scripts/test-budget.json": JSON.stringify(budget) })) {
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), text);
  }
  return dir;
}

const check = (dir: string, ...flags: string[]) => spawnSync(process.execPath, [SCRIPT, ...flags], { cwd: dir, encoding: "utf8" });

const tests = (n: number): string => Array.from({ length: n }, (_, i) => `it("case ${i}", () => {});\n`).join("");

describe("the test budget", () => {
  it("counts each it and test once, each row of an it.each table and each daemon #[test], and no fixture", () => {
    const dir = repo(
      {
        "packages/a/test/forms.test.ts": [
          'describe("a group", () => {',
          '  it("one", () => expect(/x/.test("x")).toBe(true));',
          '  it.skipIf(process.env.CI)("two", () => {});',
          '  test.each([1, 2, 3])("three %s", () => {});',
          '  for (const name of ["four", "five"]) it(name, () => {});',
          "});",
        ].join("\n"),
        "packages/a/test/fixtures/not-run.test.ts": tests(9),
        "daemon/crates/x/src/lib.rs": '#[test]\nfn a() {}\n#[tokio::test(flavor = "multi_thread")]\nasync fn b() {}\n#[test_case]\nfn c() {}\n',
      },
      { packages: {} },
    );
    expect(check(dir, "--counts").stdout).toBe("daemon 2\npackages/a 6\n");
  });

  it("fails a package one test past its budget until an over line names it, the new number and why", () => {
    const files = { "packages/a/test/a.test.ts": tests(106) };
    const packages = { "packages/a": { tests: 100, seconds: 10 } };

    const over = check(repo(files, { packages }));
    expect(over.status).toBe(1);
    expect(over.stderr).toContain("packages/a has 106 tests, past the 105 scripts/test-budget.json allows it: cut tests, or add an over line naming packages/a, the new number and why");

    const raised = check(repo(files, { packages, over: [{ package: "packages/a", tests: 106, why: "the new road's six cases" }] }));
    expect(raised.stderr).toBe("");
    expect(raised.status).toBe(0);

    const silent = check(repo(files, { packages, over: [{ package: "packages/a", tests: 106, why: "" }] }));
    expect(silent.status).toBe(1);
    expect(silent.stderr).toContain("over line 1 raises packages/a with no why");
  });

  it("fails a package with no line, and a timed suite past its seconds until an over line raises them", () => {
    const files = { "packages/a/test/a.test.ts": tests(1), "apps/b/src/b.test.tsx": tests(1) };
    const timed = { "packages/a": { seconds: 115.1, commit: "abc1234", load: 3 } };

    const run = check(repo(files, { packages: { "packages/a": { tests: 1, seconds: 100 } }, timed }));
    expect(run.status).toBe(1);
    expect(run.stderr).toContain("apps/b has 1 tests and no line in scripts/test-budget.json: add it");
    expect(run.stderr).toContain("packages/a's suite took 115.1 s at abc1234, past the 115 s scripts/test-budget.json allows it");

    const raised = check(
      repo(files, { packages: { "packages/a": { tests: 1, seconds: 100 }, "apps/b": { tests: 1, seconds: 1 } }, timed, over: [{ package: "packages/a", seconds: 116, why: "a real git per case" }] }),
    );
    expect(raised.stderr).toBe("");
    expect(raised.status).toBe(0);
  });

  it("holds this tree to the budget it commits, so a landing's laws refuse a package past it", () => {
    const run = check(fileURLToPath(new URL("../../..", import.meta.url)));
    expect(run.stderr).toBe("");
    expect(run.status).toBe(0);
  });
});
