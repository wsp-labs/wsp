// SPDX-License-Identifier: AGPL-3.0-only
// node scripts/test-budget.mjs [--counts | --time <package>...]
//
// Fails when a package holds more tests than test-budget.json allows, or its last timed suite took longer. A package's
// budget is the count and seconds its line holds plus the headroom below; going past it takes a line in the file's
// "over" list naming the package, the new number and why, so a reviewer sees each rise in the diff.
//
// The count is read off the test files' source, not vitest's own list: `vitest list` imports every file to collect it,
// 283 s on the owner's Mac for the packages it could list, and vitest 2.1.9's list crashes on apps/web and
// apps/desktop when a file logs as it loads. So each it( or test( call counts once, an it.each over an array literal
// once per row, and a daemon #[test] once; a case made in a loop or by .each over a name counts once, which leaves the
// TypeScript count 8% under vitest's.
// Seconds are a suite's wall time, which only a full run knows, so they come from the last run --time recorded.
// --counts prints each package's count, what a package's line is set from after a cut.
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { loadavg } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Tests: in the week before the cut the large packages grew 1 to 2.5% a day, so 1% lets a fix's few tests through and
 * makes a feature's batch take a line. Seconds: two timings of a package whose tests barely changed differed by up to
 * 9% (adapter-codex 4.4 and 4.8 s, apps/desktop 143 and 153 s), so 15% is past that noise.
 */
const HEADROOM = { tests: 0.01, minTests: 5, seconds: 0.15, minSeconds: 2 };
/** The base seconds were each timed with the one-minute load under this; untouched files ran 18 to 69% slower at 8 to 17. */
const MAX_LOAD = 8;
const TEST = /\.(test|spec)\.[cm]?[jt]sx?$/;
const RUST_TEST = /^\s*#\[(?:tokio::)?test\b/gm;

const root = execFileSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim();
const budgetPath = join(root, "scripts", "test-budget.json");
const file = "scripts/test-budget.json";
// TypeScript 6, the last with a parser in JavaScript, sits beside the script's own checkout rather than the tree it judges.
const ts = createRequire(new URL("../packages/protocol/package.json", import.meta.url))("typescript");

/** The package a test file belongs to: its first two folders, or the daemon for any Rust file under daemon/. */
const packageOf = path => (path.startsWith("daemon/") ? "daemon" : path.split("/").slice(0, 2).join("/"));

/** The rows of the .each or .for the call chain holds, or 1. */
function rows(callee) {
  for (let e = callee; ts.isPropertyAccessExpression(e) || ts.isCallExpression(e); e = e.expression) {
    if (ts.isCallExpression(e) && ts.isPropertyAccessExpression(e.expression) && ["each", "for"].includes(e.expression.name.text)) {
      const table = e.arguments[0];
      return table && ts.isArrayLiteralExpression(table) ? table.elements.length : 1;
    }
  }
  return 1;
}

function countTs(path, text) {
  const kind = /x$/.test(path) ? ts.ScriptKind.TSX : /\.[cm]?js$/.test(path) ? ts.ScriptKind.JS : ts.ScriptKind.TS;
  const source = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, false, kind);
  let n = 0;
  const visit = node => {
    if (ts.isCallExpression(node) && node.arguments.length > 0) {
      let head = node.expression;
      while (ts.isPropertyAccessExpression(head) || ts.isCallExpression(head)) head = head.expression;
      if (ts.isIdentifier(head) && (head.text === "it" || head.text === "test")) {
        n += rows(node.expression);
        // it.skipIf(x)("name", fn) is one test: the inner call is part of its name, not a second one.
        for (let e = node.expression; ts.isPropertyAccessExpression(e) || ts.isCallExpression(e); e = e.expression) {
          if (ts.isCallExpression(e)) e.arguments.forEach(visit);
        }
        node.arguments.forEach(visit);
        return;
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return n;
}

function count() {
  const counts = new Map();
  const listed = execFileSync("git", ["ls-files", "-z", "--cached", "--others", "--exclude-standard"], { cwd: root, encoding: "utf8", maxBuffer: 64 << 20 });
  for (const path of listed.split("\0")) {
    const rust = path.startsWith("daemon/") && path.endsWith(".rs");
    if (!rust && !(TEST.test(path) && !path.includes("/fixtures/"))) continue;
    if (!existsSync(join(root, path))) continue;
    const text = readFileSync(join(root, path), "utf8");
    const n = rust ? (text.match(RUST_TEST)?.length ?? 0) : countTs(path, text);
    if (n > 0) counts.set(packageOf(path), (counts.get(packageOf(path)) ?? 0) + n);
  }
  return counts;
}

/** Each package, over line and timing on a line of its own, so a rise reads as one line in the diff. */
function format({ packages, over, timed, ...rest }) {
  const block = (open, close, rows) => (rows.length === 0 ? open + close : `${open}\n${rows.map(row => `    ${row}`).join(",\n")}\n  ${close}`);
  const entries = object => Object.entries(object ?? {}).map(([k, v]) => `${JSON.stringify(k)}: ${JSON.stringify(v)}`);
  const head = Object.entries(rest).map(([k, v]) => `  ${JSON.stringify(k)}: ${JSON.stringify(v)},\n`).join("");
  return `{\n${head}  "packages": ${block("{", "}", entries(packages))},\n  "over": ${block("[", "]", (over ?? []).map(line => JSON.stringify(line)))},\n  "timed": ${block("{", "}", entries(timed))}\n}\n`;
}

if (process.argv[2] === "--counts") {
  for (const [pkg, n] of [...count()].sort(([a], [b]) => a.localeCompare(b))) process.stdout.write(`${pkg} ${n}\n`);
  process.exit(0);
}

const budget = JSON.parse(readFileSync(budgetPath, "utf8"));

if (process.argv[2] === "--time") {
  const named = process.argv.slice(3);
  if (named.length === 0) {
    process.stderr.write("refused: name the packages to time; usage: node scripts/test-budget.mjs --time <package>...\n");
    process.exit(2);
  }
  for (const pkg of named) {
    if (!Object.hasOwn(budget.packages, pkg)) {
      process.stderr.write(`refused: ${pkg} has no line in ${file}\n`);
      process.exit(2);
    }
    if (pkg === "daemon") {
      process.stderr.write("refused: time the daemon with the Mac CI job's cargo test line and write its seconds by hand\n");
      process.exit(2);
    }
  }
  if (!process.env.WSP_MCP_BIN || !existsSync(process.env.WSP_MCP_BIN)) {
    process.stderr.write("refused: set WSP_MCP_BIN to the debug daemon built with its mcp feature, as the base seconds were timed; without it the tool server suites skip\n");
    process.exit(2);
  }
  // heavy.sh can wait minutes for a slot, so the clock and the load are read only once this run holds one.
  if (process.platform === "darwin" && !process.env.WSP_HEAVY_SLOT) {
    const held = spawnSync(join(root, "scripts", "heavy.sh"), [process.execPath, fileURLToPath(import.meta.url), ...process.argv.slice(2)], { cwd: root, stdio: "inherit" });
    process.exit(held.status ?? 1);
  }
  const commit = execFileSync("git", ["rev-parse", "--short", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
  for (const pkg of named) {
    const load = Number(loadavg()[0].toFixed(2));
    if (load >= MAX_LOAD) {
      process.stderr.write(`refused: the load is ${load}, and the base seconds were timed under ${MAX_LOAD}; time ${pkg} when the Mac is quieter\n`);
      process.exit(2);
    }
    const started = Date.now();
    const run = spawnSync("pnpm", ["exec", "vitest", "run", "--minWorkers=1", "--maxWorkers=2", `${pkg}/`], { cwd: root, stdio: "inherit", env: { ...process.env, WSP_REQUIRE_DAEMON: "1" } });
    const seconds = Number(((Date.now() - started) / 1000).toFixed(1));
    if (run.status !== 0) {
      process.stderr.write(`${pkg}'s suite failed, so its ${seconds} s are not recorded\n`);
      process.exit(1);
    }
    budget.timed = { ...budget.timed, [pkg]: { seconds, commit, load } };
    writeFileSync(budgetPath, format(budget));
    process.stdout.write(`${pkg}: ${seconds} s at ${commit}, load ${load}, recorded in ${file}\n`);
  }
}

const failures = [];
const limits = new Map();
for (const [pkg, { tests, seconds }] of Object.entries(budget.packages)) {
  limits.set(pkg, {
    tests: tests + Math.max(Math.ceil(tests * HEADROOM.tests), HEADROOM.minTests),
    seconds: Number(Math.max(seconds * (1 + HEADROOM.seconds), seconds + HEADROOM.minSeconds).toFixed(1)),
  });
}
for (const [i, line] of (budget.over ?? []).entries()) {
  const named = ["tests", "seconds"].filter(k => typeof line[k] === "number");
  if (!limits.has(line.package)) failures.push(`${file} over line ${i + 1} names ${line.package}, which has no line in its packages`);
  else if (named.length === 0) failures.push(`${file} over line ${i + 1} names no new number of tests or seconds for ${line.package}`);
  else if (typeof line.why !== "string" || line.why.trim() === "") failures.push(`${file} over line ${i + 1} raises ${line.package} with no why: say why it needs more`);
  else for (const k of named) limits.get(line.package)[k] = line[k];
}

const counts = count();
for (const [pkg, n] of counts) {
  const limit = limits.get(pkg);
  if (!limit) failures.push(`${pkg} has ${n} tests and no line in ${file}: add it`);
  else if (n > limit.tests) failures.push(`${pkg} has ${n} tests, past the ${limit.tests} ${file} allows it: cut tests, or add an over line naming ${pkg}, the new number and why`);
}
for (const [pkg, { seconds, commit }] of Object.entries(budget.timed ?? {})) {
  const limit = limits.get(pkg);
  if (limit && seconds > limit.seconds) failures.push(`${pkg}'s suite took ${seconds} s at ${commit}, past the ${limit.seconds} s ${file} allows it: make it faster, or add an over line naming ${pkg}, the new seconds and why`);
}

for (const failure of failures) process.stderr.write(`${failure}\n`);
if (failures.length > 0) process.exit(1);
process.stdout.write(`${counts.size} packages within their test budget\n`);
