// SPDX-License-Identifier: AGPL-3.0-only
// The laws judge the tree that lands, the branch merged with origin/main, so a branch that passes alone and fails
// once it meets main is caught before its report. Driven in a throwaway origin and clone whose one law, a stand-in
// for test-files.sh, holds the notes folder to three files: main adding one and the branch adding one each pass alone.
// The full run then runs the test files the change reaches through imports, as the real affected-tests.mjs names them.
import { execFileSync, spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { writeStub } from "../../protocol/test/stub-script.js";

const SCRIPT = fileURLToPath(new URL("../../../scripts/pre-review.sh", import.meta.url));
const AFFECTED = fileURLToPath(new URL("../../../scripts/affected-tests.mjs", import.meta.url));

/** Each run says where it ran, whether it saw this clone's installs and how pnpm was told to treat them. */
const LAW = `#!/bin/sh
printf '%s %s %s\\n' "$(pwd -P)" "$(cat node_modules/marker 2>/dev/null)" "\${pnpm_config_verify_deps_before_run:-check}" >>"$LAW_RUNS"
if [ "$(ls notes | wc -l)" -le 3 ]; then echo " Test Files  1 passed (1)"; exit 0; fi
echo " FAIL  notes.test.ts > the notes stay at three files"
echo " Test Files  1 failed (1)"
exit 1
`;

const git = (dir: string, ...args: string[]): string => execFileSync("git", ["-C", dir, ...args], { encoding: "utf8" }).trimEnd();

let dirs: string[] = [];
afterEach(() => {
  for (const dir of dirs) rmSync(dir, { recursive: true, force: true });
  dirs = [];
});

function write(dir: string, files: Record<string, string>): void {
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), text);
  }
}

function commit(dir: string, files: Record<string, string>, message: string): void {
  write(dir, files);
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "-m", message);
}

/** Each run writes the files it was handed on one line and passes. */
const RECORD = `#!/bin/sh
printf '%s\\n' "$*" >>"$LAW_RUNS"
echo " Test Files  $# passed ($#)"
`;

/** An origin holding the script and the law, and a clone on a branch off its main with installs git ignores. */
function clone(law = LAW, files: Record<string, string> = {}): { origin: string; work: string; runs: string } {
  const top = mkdtempSync(join(tmpdir(), "wsp-pre-review-"));
  dirs.push(top);
  const origin = join(top, "origin");
  const work = join(top, "work");
  mkdirSync(origin);
  git(origin, "init", "-q", "-b", "main");
  git(origin, "config", "user.email", "pre-review@example.invalid");
  git(origin, "config", "user.name", "pre-review");
  mkdirSync(join(origin, "scripts"));
  copyFileSync(SCRIPT, join(origin, "scripts/pre-review.sh"));
  writeStub(join(origin, "scripts/test-files.sh"), law);
  writeStub(join(origin, "scripts/heavy.sh"), '#!/bin/sh\nexec "$@"\n');
  write(origin, { ".gitignore": "node_modules/\n", "notes/one.md": "one\n", "notes/shared.md": "as it was\n", ...files });
  commit(origin, {}, "base");
  execFileSync("git", ["clone", "-q", origin, work]);
  git(work, "config", "user.email", "pre-review@example.invalid");
  git(work, "config", "user.name", "pre-review");
  git(work, "switch", "-q", "-c", "feature");
  write(work, { "node_modules/marker": "installed\n" });
  return { origin, work, runs: join(top, "runs.txt") };
}

/** This run's environment less the pnpm settings the pnpm that started it hands down, which the script decides. */
const pnplessEnv = (): NodeJS.ProcessEnv => Object.fromEntries(Object.entries(process.env).filter(([k]) => !k.startsWith("pnpm_config_")));

function laws(work: string, runs: string, args = ["--laws"]): { status: number | null; out: string; ran: string[] } {
  const run = spawnSync("bash", [join(work, "scripts/pre-review.sh"), ...args], { cwd: work, encoding: "utf8", env: { ...pnplessEnv(), LAW_RUNS: runs } });
  const ran = existsSync(runs) ? readFileSync(runs, "utf8").split("\n").filter(Boolean) : [];
  rmSync(runs, { force: true });
  const logs = /^logs: (.+)$/m.exec(run.stdout);
  if (logs) dirs.push(logs[1]!);
  return { status: run.status, out: run.stdout + run.stderr, ran };
}

/** What the run could have written to the branch or its folder: its own refs, the status, the worktrees. */
function untouched(work: string): string {
  return [git(work, "for-each-ref", "--format=%(refname) %(objectname)", "refs/heads", "refs/tags"), git(work, "status", "--porcelain"), git(work, "worktree", "list", "--porcelain")].join("\n--\n");
}

describe("the laws on the branch merged with origin/main", () => {
  it("goes red on a branch that passes alone and fails once main moves, writing nothing to the branch or its folder", () => {
    const { origin, work, runs } = clone();
    commit(work, { "notes/two.md": "two\n" }, "feature");
    commit(origin, { "notes/three.md": "three\n" }, "main moves");
    const before = untouched(work);

    const run = laws(work, runs);
    expect(run.status, run.out).toBe(1);
    const merged = /a throwaway merge commit ([0-9a-f]+) in (\S+)/.exec(run.out);
    expect(run.out).toContain(`the laws run on this folder merged with origin/main ${git(origin, "rev-parse", "--short", "HEAD")}, a throwaway merge commit`);
    expect(merged, run.out).not.toBeNull();
    expect(run.out).toContain("FAIL  the laws: notes.test.ts > the notes stay at three files");
    expect(run.ran).toEqual([`${merged![2]} installed false`]);
    expect(untouched(work)).toBe(before);
    expect(existsSync(merged![2]!)).toBe(false);
    expect(git(work, "branch", "--all", "--contains", merged![1]!)).toBe("");
  });

  it("merges the folder as it stands, its uncommitted and untracked changes with it, and leaves them where they were", () => {
    const { origin, work, runs } = clone();
    commit(work, { "notes/two.md": "two\n" }, "feature");
    commit(origin, { "notes/three.md": "three\n" }, "main moves");
    rmSync(join(work, "notes/one.md"));
    const before = untouched(work);
    expect(laws(work, runs).status).toBe(0);
    expect(untouched(work)).toBe(before);

    write(work, { "notes/four.md": "four\n" });
    const untracked = laws(work, runs);
    expect(untracked.status, untracked.out).toBe(1);
    expect(git(work, "status", "--porcelain")).toBe(" D notes/one.md\n?? notes/four.md");
  });

  it("names the files a branch that conflicts with main conflicts in, and runs no law on a tree that cannot land", () => {
    const { origin, work, runs } = clone();
    commit(work, { "notes/shared.md": "as the branch has it\n", "notes/other.md": "x\n" }, "feature");
    commit(origin, { "notes/shared.md": "as main has it\n" }, "main moves");
    const before = untouched(work);

    const run = laws(work, runs);
    expect(run.status, run.out).toBe(1);
    expect(run.out).toContain(`FAIL  the laws: this folder conflicts with origin/main ${git(origin, "rev-parse", "--short", "HEAD")} in notes/shared.md, so no tree of it can land; merge origin/main in and resolve them`);
    expect(run.out).not.toContain("the laws run on");
    expect(run.ran).toEqual([]);
    expect(untouched(work)).toBe(before);
  });

  it("runs the size check on the merge too, as the landing does", () => {
    const size = 'import { readdirSync } from "node:fs";\nif (readdirSync("notes").length > 3) { console.error("notes holds too many files"); process.exit(1); }\n';
    const { origin, work, runs } = clone(RECORD, { "scripts/file-size-check.mjs": size });
    commit(work, { "notes/two.md": "two\n" }, "feature");
    expect(laws(work, runs).out).toContain("PASS  the size check");

    commit(origin, { "notes/three.md": "three\n" }, "main moves");
    const run = laws(work, runs);
    expect(run.status, run.out).toBe(1);
    expect(run.out).toContain("PASS  the laws");
    expect(run.out).toContain("FAIL  the size check: notes holds too many files");
  });

  it("runs on the folder itself while the branch holds origin/main", () => {
    const { work, runs } = clone();
    commit(work, { "notes/two.md": "two\n" }, "feature");
    const run = laws(work, runs);
    expect(run.status, run.out).toBe(0);
    expect(run.out).toContain(`the laws run on this folder, which holds origin/main ${git(work, "rev-parse", "--short", "origin/main")}`);
    expect(run.out).toContain("PASS  the laws: 1 passed (1)");
    expect(run.ran).toEqual([`${git(work, "rev-parse", "--show-toplevel")} installed check`]);
  });
});

/** One package whose test imports its source, another test beside it that imports nothing, and the hook pre-review
 * runs, passing. */
const PACKAGE = {
  "scripts/affected-tests.mjs": readFileSync(AFFECTED, "utf8"),
  ".githooks/pre-commit": "exit 0\n",
  "pnpm-workspace.yaml": 'packages:\n  - "packages/*"\n',
  "packages/a/package.json": JSON.stringify({ name: "@t/a" }),
  "packages/a/src/thing.ts": "export const thing = 1;\n",
  "packages/a/test/thing.test.ts": 'import { thing } from "../src/thing.js";\n',
  "packages/a/test/other.test.ts": "\n",
};

describe("the test files the change reaches", () => {
  it("runs the test that imports a source file the branch changes, on a branch that touches no test", () => {
    const { work, runs } = clone(RECORD, PACKAGE);
    commit(work, { "packages/a/src/thing.ts": "export const thing = 2;\n" }, "feature");

    const run = laws(work, runs, []);
    expect(run.status, run.out).toBe(0);
    expect(run.out).toContain("SKIP  the touched tests: the branch changes no test file outside the laws");
    expect(run.out).toContain("PASS  the affected tests, files affected-tests.mjs names: 1; 1 passed (1)");
    expect(run.out).toContain("WARN  the affected tests: CI runs every test of packages/a/; of those, only the files the change reaches through imports ran here");
    expect(run.ran.filter(files => !files.startsWith("--tools "))).toEqual(["packages/a/test/thing.test.ts"]);
  });

  it("runs the touched files alone when the change reaches the whole suite, and says so", () => {
    const { work, runs } = clone(RECORD, PACKAGE);
    commit(work, { "packages/a/src/thing.ts": "export const thing = 2;\n", "packages/a/test/other.test.ts": "//\n", "tsconfig.json": "{}\n" }, "feature");

    const run = laws(work, runs, []);
    expect(run.out).toContain("PASS  the touched tests: 1 passed (1)");
    expect(run.out).toContain("WARN  the affected tests: the change reaches the whole suite (tsconfig.json is outside the packages and the folders mapped above, so any test may read it), so only the touched files ran; the rest waits on CI");
    expect(run.ran.filter(files => !files.startsWith("--tools "))).toEqual(["packages/a/test/other.test.ts"]);
  });

  it("fails, running no affected file, when affected-tests.mjs fails", () => {
    const { work, runs } = clone(RECORD, { ...PACKAGE, "scripts/affected-tests.mjs": 'console.error("no workspace file"); process.exit(1);\n' });
    commit(work, { "packages/a/src/thing.ts": "export const thing = 2;\n" }, "feature");

    const run = laws(work, runs, []);
    expect(run.status, run.out).toBe(1);
    expect(run.out).toContain("FAIL  the affected tests: scripts/affected-tests.mjs failed, so no affected file ran: no workspace file");
    expect(run.ran.filter(files => !files.startsWith("--tools "))).toEqual([]);
  });
});

const VITEST = fileURLToPath(new URL("../../../node_modules/vitest/vitest.mjs", import.meta.url));

/** Each check names the folder it was handed and fails on one holding a file named broken, as tsc words it. */
const TSC = `#!/bin/sh
printf 'tsc %s\\n' "$*" >>"$LAW_RUNS"
for a in "$@"; do [ "$prev" = -p ] && dir=$a; prev=$a; done
[ -f "$dir/broken" ] || exit 0
echo "$dir/src/index.ts(1,14): error TS2322: Type 'string' is not assignable to type 'number'."
exit 2
`;

/** a at the bottom, b and the host declaring it, c declaring nothing, each but the host with its own tsconfig. */
const TYPED = {
  ...PACKAGE,
  "packages/a/tsconfig.json": "{}\n",
  "packages/b/package.json": JSON.stringify({ name: "@t/b", dependencies: { "@t/a": "workspace:*" } }),
  "packages/b/tsconfig.json": "{}\n",
  "packages/b/src/index.ts": 'export { thing } from "@t/a";\n',
  "packages/c/package.json": JSON.stringify({ name: "@t/c" }),
  "packages/c/tsconfig.json": "{}\n",
  "packages/c/src/index.ts": "export const c = 1;\n",
  "packages/host/package.json": JSON.stringify({ name: "@t/host", dependencies: { "@t/a": "workspace:*" } }),
  "packages/host/test/host.test.ts": "\n",
};

function typedClone(files: Record<string, string> = {}): { origin: string; work: string; runs: string } {
  const made = clone(RECORD, { ...TYPED, ...files });
  mkdirSync(join(made.work, "node_modules/.bin"), { recursive: true });
  writeStub(join(made.work, "node_modules/.bin/tsc"), TSC);
  return made;
}

describe("--changed, a builder's gate", () => {
  it("typechecks the package the change touches and every package declaring it, each from its own cache", () => {
    const { work, runs } = typedClone();
    commit(work, { "packages/a/src/thing.ts": "export const thing = 2;\n" }, "feature");

    const run = laws(work, runs, ["--changed"]);
    expect(run.status, run.out).toBe(0);
    expect(run.out).toContain("PASS  the typecheck of packages/a packages/b");
    const cache = join(realpathSync(work), "node_modules/.cache/pre-review-tsc");
    expect(run.ran.filter(line => line.startsWith("tsc "))).toEqual([
      `tsc --noEmit -p packages/a --incremental --tsBuildInfoFile ${cache}/packages-a.tsbuildinfo`,
      `tsc --noEmit -p packages/b --incremental --tsBuildInfoFile ${cache}/packages-b.tsbuildinfo`,
    ]);
    const tests = run.ran.filter(line => !line.startsWith("tsc "));
    expect(tests).toHaveLength(1);
    expect(tests[0]).toMatch(/^--tools packages\/host\/test\/words\.test\.ts .* packages\/a\/test\/thing\.test\.ts$/);
    expect(tests[0]).toContain(" packages/host/test/contract.test.ts ");
    expect(run.out).toContain("PASS  the laws, the touched tests (0) and the affected tests (1), in one run: ");

    const whole = laws(work, runs, []);
    expect(whole.out).not.toContain("the typecheck");
    expect(whole.ran.filter(line => line.startsWith("tsc "))).toEqual([]);
  });

  it("leaves out the host's contract laws and the tool record when the change reaches no test of the host", () => {
    const { work, runs } = typedClone();
    commit(work, { "packages/c/src/index.ts": "export const c = 2;\n" }, "feature");

    const run = laws(work, runs, ["--changed"]);
    expect(run.status, run.out).toBe(0);
    expect(run.out).toContain("SKIP  the parity, skill, contract and tool record tests: the change reaches no test of packages/host");
    const tests = run.ran.filter(line => !line.startsWith("tsc "));
    expect(tests).toHaveLength(1);
    expect(tests[0]).toMatch(/^packages\/host\/test\/words\.test\.ts /);
    expect(tests[0]!.split(" ").filter(f => /^packages\/host\/test\/(parity|skill|contract)\.test\.ts$/.test(f))).toEqual([]);
  });

  it("keeps a host law the branch changed, though the change reaches no other test of the host", () => {
    const { work, runs } = typedClone();
    commit(work, { "packages/host/test/parity.test.ts": "\n" }, "feature");

    const run = laws(work, runs, ["--changed"]);
    expect(run.out).not.toContain("SKIP  the parity");
    expect(run.ran.filter(line => !line.startsWith("tsc "))[0]).toMatch(/^--tools .* packages\/host\/test\/parity\.test\.ts /);
  });

  it("runs the laws on their own when main has moved, since they judge the merge and the other tests this folder", () => {
    const { origin, work, runs } = typedClone();
    commit(work, { "packages/a/src/thing.ts": "export const thing = 2;\n" }, "feature");
    commit(origin, { "notes/three.md": "three\n" }, "main moves");

    const run = laws(work, runs, ["--changed"]);
    expect(run.out).toContain("PASS  the laws: ");
    expect(run.out).toContain("PASS  the affected tests, files affected-tests.mjs names: 1; 1 passed (1)");
    expect(run.ran.filter(line => !line.startsWith("tsc ") && !line.startsWith("--tools "))).toEqual(["packages/a/test/thing.test.ts"]);
  });

  it("fails on a package declaring the changed one that no longer typechecks, with tsc's words", () => {
    const { work, runs } = typedClone({ "packages/b/broken": "\n" });
    commit(work, { "packages/a/src/thing.ts": 'export const thing = "two";\n' }, "feature");

    const run = laws(work, runs, ["--changed"]);
    expect(run.status, run.out).toBe(1);
    expect(run.out).toContain("FAIL  the typecheck of packages/b: packages/b/src/index.ts(1,14): error TS2322: Type 'string' is not assignable to type 'number'.");
  });

  it("refuses two runs at once", () => {
    const { work, runs } = typedClone();
    const run = laws(work, runs, ["--laws", "--review"]);
    expect(run.status).toBe(2);
    expect(run.out).toContain("refused: --laws and --review are two runs; pick one");
  });
});

const CASE = (value: string): string => `import { expect, it } from "vitest";\nimport { thing } from "../src/thing.js";\nit("is ${value}", () => expect(thing).toBe(${value}));\n`;

/** A clone whose vitest is the real one, run without pnpm as the red proof runs it. */
function reviewClone(files: Record<string, string> = {}): { work: string; runs: string } {
  const made = typedClone({ "packages/a/test/thing.test.ts": CASE("1"), ...files });
  writeStub(join(made.work, "node_modules/.bin/vitest"), `#!/bin/sh\nexec "${process.execPath}" "${VITEST}" "$@"\n`);
  return made;
}

describe("--review, a reviewer's run", () => {
  it("runs only the changed test files, on the branch and then on the merge-base's sources, and no law", () => {
    const { work, runs } = reviewClone();
    commit(work, { "packages/a/src/thing.ts": "export const thing = 2;\n", "packages/a/test/thing.test.ts": CASE("2"), "packages/a/test/kept.test.ts": 'import { it } from "vitest";\nit("holds", () => {});\n' }, "feature");

    const run = laws(work, runs, ["--review"]);
    expect(run.status, run.out).toBe(0);
    expect(run.out).toContain("PASS  the changed tests: 2 passed (2)");
    expect(run.out).toMatch(/^ {2}red {4}packages\/a\/test\/thing\.test\.ts: 1 of 1 red: /m);
    expect(run.out).toContain("  green  packages/a/test/kept.test.ts: passes on the base, so it shows nothing this change fixed");
    expect(run.out).not.toMatch(/the laws|the size check|affected|touched|one commit ahead|typecheck/);
    expect(run.ran).toEqual([]);
  });

  it("runs a changed test that sits beside its source on the merge-base as the branch has it", () => {
    const beside = (value: string): string => CASE(value).replace("../src/thing.js", "./thing.js");
    const { work, runs } = reviewClone({ "packages/a/src/thing.test.ts": beside("1") });
    commit(work, { "packages/a/src/thing.ts": "export const thing = 2;\n", "packages/a/src/thing.test.ts": beside("2") }, "feature");

    const run = laws(work, runs, ["--review"]);
    expect(run.status, run.out).toBe(0);
    expect(run.out).toMatch(/^ {2}red {4}packages\/a\/src\/thing\.test\.ts: 1 of 1 red: /m);
  });

  it("fails when a changed test file fails on the branch", () => {
    const { work, runs } = reviewClone();
    commit(work, { "packages/a/test/thing.test.ts": CASE("3") }, "feature");

    const run = laws(work, runs, ["--review"]);
    expect(run.status, run.out).toBe(1);
    expect(run.out).toContain("FAIL  the changed tests: packages/a/test/thing.test.ts > is 3");
  });
});
