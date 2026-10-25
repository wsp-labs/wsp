// SPDX-License-Identifier: AGPL-3.0-only
// Tapes from the suites that drive the host from outside: each runs once with the tap on, and every sequence of
// frames a socket, the tool server or the command line was sent becomes a tape under parity/tapes/recorded. Its
// answers stay beside the frames, marked "outs": "recorded": a replay follows the ids they name and holds nothing to
// them, since the goldens are what a fresh host answers. A sequence of frames seen twice is kept once.
//
// The tool server's record (daemon/crates/wsp-mcp/tests/answers) joins them: each tool case it keeps becomes a tape that
// calls that tool with the case's arguments, so every tool is asked of a real host, not the record's fake one.
//
// node parity/record/collect.mjs --from <folder> --held <dir> takes the tapes a host served with WSP_TAPE_DIR=<folder>
// wrote, a day of real traffic, into the held-back folder's tapes/shadow instead, which no implementer is shown.
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
/** A test's tape past this is a test of a size limit, which says nothing about a host's answers that a short one does
 * not; a real host's longer connections are cut to it. */
const MAX_TAPE_BYTES = 64 * 1024;
/** A tape with the answers its recording's host gave beside its frames, past which the answers are left off. */
const ANSWERS_MAX_BYTES = 4 * MAX_TAPE_BYTES;
/** The temp folder of the run the tapes came from, as the test environment names it, which each replay points inside
 * its own folder: the harness's RUN_MARK. */
const RUN_DIR = new RegExp(`${tmpdir().replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}/wsp-run-\\d+`, "g");
const RUN_MARK = "<run>";
/** What the recording's own host and temp folders minted at random: a fresh host knows none of it, so each is numbered
 * in order of first sight on its tape, the same value the same number, and a second recording writes the same tape. */
const MINTED = [
  { shape: /\b[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b/g, as: n => `00000000-0000-4000-8000-${String(n).padStart(12, "0")}` },
  { shape: /\b[0-9a-f]{32}\b/g, as: n => String(n).padStart(32, "0") },
  { shape: /\b[a-z]{1,3}_[0-9a-f]{6,16}\b/g, as: (n, v) => `${v.split("_")[0]}_${String(n).padStart(8, "0")}` },
  { shape: new RegExp(`(?<=${RUN_MARK}/[\\w.]+(?:-[\\w.]+)*-)[A-Za-z0-9]{6}(?=[/"])`, "g"), as: n => String(n).padStart(6, "0") },
];

function steady(text) {
  for (const { shape, as } of MINTED) {
    const seen = new Map();
    text = text.replace(shape, v => seen.get(v) ?? seen.set(v, as(seen.size + 1, v)).get(v));
  }
  return text;
}

const MCP_ANSWERS = join(root, "daemon/crates/wsp-mcp/tests/answers");

/** A suite drives the host from outside when it serves a runtime, dials one, spawns the built command, serves the
 * tools on stdio or runs the command line; the live suites make real machines and stay out. */
const OUTSIDE_IN = String.raw`serveRuntime\(|ws-client|WsClient|startHost\(|serveHost\(|built-bin|serveMcp\(|[^a-zA-Z.]cli\(`;

/** Suites that run the leave sweep against the computer they run on, which as root on a box takes real installed
 * paths (/usr/local/bin/claude, wsp-map#2464): the daemon's wire suite, and every suite about a leave. */
const SWEEPS = [/^packages\/daemon\//, /leave/];

function suites() {
  const listed = spawnSync("git", ["grep", "-l", "-E", OUTSIDE_IN, "--", "packages/*/test/*.test.ts"], { cwd: root, encoding: "utf8" });
  return listed.stdout.split("\n").filter(f => f !== "" && !f.endsWith(".live.test.ts") && !SWEEPS.some(s => s.test(f)));
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

/** The handshake every tool server tape opens with, as a client that has never seen the server sends it. */
const HANDSHAKE = [
  { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "parity", version: "1" } } },
  { jsonrpc: "2.0", method: "notifications/initialized" },
];

function recordTapes(into) {
  let n = 0;
  for (const name of readdirSync(MCP_ANSWERS).filter(f => f.endsWith(".json")).sort()) {
    const { tool, cases } = JSON.parse(readFileSync(join(MCP_ANSWERS, name), "utf8"));
    for (const c of cases) {
      const meta = { door: "mcp", source: `mcp-record > ${tool} > ${c.case}` };
      const call = { jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: tool, arguments: c.arguments } };
      const lines = [{ meta }, ...[...HANDSHAKE, call].map(f => ({ in: f }))];
      writeFileSync(join(into, `mcp-record-${++n}.tape`), lines.map(l => JSON.stringify(l)).join("\n") + "\n");
    }
  }
  console.log(`${n} tool cases read from the tool server's record`);
}

/** A connection that ran for hours, as an app's socket on a real host does, cut into tapes under the size cap, each
 * opening the way the connection did: its auth, or the tool server's handshake. */
function pieces(door, ins) {
  if (JSON.stringify(ins).length <= MAX_TAPE_BYTES) return [ins];
  const opening = door === "ws" ? (ins[0]?.op === "auth" ? 1 : 0) : door === "mcp" ? ins.findIndex(f => f?.method === "notifications/initialized") + 1 : 0;
  const head = ins.slice(0, opening);
  const out = [];
  let piece = [...head];
  let size = JSON.stringify(head).length;
  for (const frame of ins.slice(opening)) {
    const more = JSON.stringify(frame).length + 1;
    if (piece.length > head.length && size + more > MAX_TAPE_BYTES) {
      out.push(piece);
      piece = [...head];
      size = JSON.stringify(head).length;
    }
    piece.push(frame);
    size += more;
  }
  if (piece.length > head.length) out.push(piece);
  return out;
}

function keep(from, out, { cut = false } = {}) {
  const seen = new Set();
  const kept = [];
  let read = 0;
  let big = 0;
  for (const name of readdirSync(from).sort()) {
    read++;
    const text = steady(readFileSync(join(from, name), "utf8").replace(RUN_DIR, RUN_MARK));
    const lines = text.trimEnd().split("\n").map(l => JSON.parse(l));
    const meta = lines[0].meta;
    const all = lines.filter(l => "in" in l).map(l => l.in);
    if (all.length === 0) continue;
    for (const ins of cut ? pieces(meta.door, all) : [all]) {
      const key = `${meta.door}\n${JSON.stringify(ins)}`;
      if (seen.has(key)) continue;
      seen.add(key);
      if (key.length > MAX_TAPE_BYTES) {
        big++;
        continue;
      }
      // What the recording's own host answered, kept beside what it was sent so a replay follows an id the test's
      // host minted to the one a fresh host mints; a cut piece no longer lines up with them, and a page of answers
      // past the cap is left out rather than the tape.
      const frames = lines.filter(l => "in" in l || "out" in l);
      const answered = !cut && frames.length > ins.length && JSON.stringify(frames).length <= ANSWERS_MAX_BYTES;
      kept.push(answered ? { meta: { ...meta, outs: "recorded" }, lines: frames } : { meta, lines: ins.map(f => ({ in: f })) });
    }
  }
  rmSync(out, { recursive: true, force: true });
  const taken = new Map();
  for (const { meta, lines } of kept) {
    const [file, ...names] = meta.source.split(" > ");
    const folder = slug((file ?? "host").replace(/^packages\//, "").replace(/\.test\.ts$/, "").replace("/test/", "-"));
    const base = join(meta.door, folder, slug(names.join(" ")) || "case");
    const n = (taken.get(base) ?? 0) + 1;
    taken.set(base, n);
    const path = join(out, `${base}${n === 1 ? "" : `-${n}`}.tape`);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, [JSON.stringify({ meta }), ...lines.map(l => JSON.stringify(l))].join("\n") + "\n");
  }
  const doors = {};
  for (const { meta } of kept) doors[meta.door] = (doors[meta.door] ?? 0) + 1;
  console.log(`${read} tapes written, ${kept.length} kept once each (${Object.entries(doors).map(([d, n]) => `${d} ${n}`).join(", ")}) under ${out.replace(root, "")}, ${big} over ${MAX_TAPE_BYTES} bytes left out`);
}

function main() {
  const from = process.argv.indexOf("--from");
  if (from !== -1) {
    const dir = process.argv[from + 1];
    const held = process.argv.indexOf("--held");
    if (dir === undefined) throw new Error("--from names the folder a host wrote its tapes into");
    if (held === -1 || process.argv[held + 1] === undefined) throw new Error("--held names the held-back folder the day goes into, outside the repository");
    keep(dir, join(process.argv[held + 1], "tapes/shadow"), { cut: true });
    return;
  }
  const tapes = runSuites();
  recordTapes(tapes);
  keep(tapes, join(root, "parity/tapes/recorded"));
  rmSync(tapes, { recursive: true, force: true });
}

main();
