// SPDX-License-Identifier: AGPL-3.0-only
// Tapes from the suites that drive the host from outside: each runs once with the tap on, and every sequence of
// frames a socket, the tool server or the command line was sent becomes a tape under parity/tapes/recorded, its
// answers left out, since the goldens are what a fresh host answers. A sequence seen twice is kept once.
//
// node parity/record/collect.mjs --from <folder> takes the tapes a host served with WSP_TAPE_DIR=<folder> wrote, a
// day of real traffic, into parity/tapes/shadow instead, which no implementer is shown.
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
/** A tape past this is a test of a size limit, which says nothing about a host's answers that a short one does not. */
const MAX_TAPE_BYTES = 64 * 1024;
/** The temp folder of the run the tapes came from, as the test environment names it, which each replay points inside
 * its own folder: the harness's RUN_MARK. */
const RUN_DIR = new RegExp(`${tmpdir().replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}/wsp-run-\\d+`, "g");
const RUN_MARK = "<run>";

/** A suite drives the host from outside when it serves a runtime, dials one, spawns the built command, serves the
 * tools on stdio or runs the command line; the live suites make real machines and stay out. */
const OUTSIDE_IN = String.raw`serveRuntime\(|ws-client|WsClient|startHost\(|serveHost\(|built-bin|serveMcp\(|[^a-zA-Z.]cli\(`;

function suites() {
  const listed = spawnSync("git", ["grep", "-l", "-E", OUTSIDE_IN, "--", "packages/*/test/*.test.ts"], { cwd: root, encoding: "utf8" });
  return listed.stdout.split("\n").filter(f => f !== "" && !f.endsWith(".live.test.ts"));
}

const slug = text =>
  text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 72)
    .replace(/-$/, "");

function runSuites() {
  const tapes = mkdtempSync(join(tmpdir(), "wsp-tapes-"));
  const report = join(tapes, "..", `${tapes.split("/").pop()}-report.json`);
  const files = [...suites(), "parity/record/cli-lines.record.ts"];
  const started = Date.now();
  spawnSync("pnpm", ["exec", "vitest", "run", "--workspace", "parity/record/vitest.tape.ts", "--minWorkers=1", "--maxWorkers=2", "--reporter=dot", "--reporter=json", `--outputFile.json=${report}`, ...files], {
    cwd: root,
    env: { ...process.env, PARITY_TAPE_DIR: tapes },
    stdio: "ignore",
  });
  const seconds = ((Date.now() - started) / 1000).toFixed(0);
  const results = JSON.parse(readFileSync(report, "utf8"));
  rmSync(report, { force: true });
  console.log(`ran ${files.length} suites with the tap on in ${seconds} s: ${results.numTotalTests} cases, ${results.numPassedTests} passed, ${results.numFailedTests} failed, ${results.numPendingTests} skipped`);
  for (const file of results.testResults) {
    const failed = file.assertionResults.filter(a => a.status === "failed");
    if (failed.length > 0) console.log(`  failed in ${file.name.replace(root, "")}: ${failed.map(a => a.title).join("; ").slice(0, 300)}`);
  }
  return tapes;
}

function keep(from, out) {
  const seen = new Set();
  const kept = [];
  let read = 0;
  let big = 0;
  for (const name of readdirSync(from).sort()) {
    read++;
    const text = readFileSync(join(from, name), "utf8").replace(RUN_DIR, RUN_MARK);
    const lines = text.trimEnd().split("\n").map(l => JSON.parse(l));
    const meta = lines[0].meta;
    const ins = lines.filter(l => "in" in l).map(l => l.in);
    if (ins.length === 0) continue;
    const key = `${meta.door}\n${JSON.stringify(ins)}`;
    if (seen.has(key)) continue;
    seen.add(key);
    if (key.length > MAX_TAPE_BYTES) {
      big++;
      continue;
    }
    kept.push({ meta, ins });
  }
  rmSync(out, { recursive: true, force: true });
  const taken = new Map();
  for (const { meta, ins } of kept) {
    const [file, ...names] = meta.source.split(" > ");
    const folder = slug((file ?? "host").replace(/^packages\//, "").replace(/\.test\.ts$/, "").replace("/test/", "-"));
    const base = join(meta.door, folder, slug(names.join(" ")) || "case");
    const n = (taken.get(base) ?? 0) + 1;
    taken.set(base, n);
    const path = join(out, `${base}${n === 1 ? "" : `-${n}`}.tape`);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, [JSON.stringify({ meta }), ...ins.map(f => JSON.stringify({ in: f }))].join("\n") + "\n");
  }
  const doors = {};
  for (const { meta } of kept) doors[meta.door] = (doors[meta.door] ?? 0) + 1;
  console.log(`${read} tapes written, ${kept.length} kept once each (${Object.entries(doors).map(([d, n]) => `${d} ${n}`).join(", ")}) under ${out.replace(root, "")}, ${big} over ${MAX_TAPE_BYTES} bytes left out`);
}

function main() {
  const from = process.argv.indexOf("--from");
  if (from !== -1) {
    const dir = process.argv[from + 1];
    if (dir === undefined) throw new Error("--from names the folder a host wrote its tapes into");
    keep(dir, join(root, "parity/tapes/shadow"));
    return;
  }
  const tapes = runSuites();
  keep(tapes, join(root, "parity/tapes/recorded"));
  rmSync(tapes, { recursive: true, force: true });
}

main();
