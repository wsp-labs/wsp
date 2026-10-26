// SPDX-License-Identifier: AGPL-3.0-only
// Replays every fenced `tape` block in docs/rust-host-spec/*.md against a fresh TypeScript host and diffs what the
// host answers with what the tape says, after the normalizing in normal(). Each tape gets its own host: an empty temp
// folder, env -i, its own --state, a free port, no relay. Exits 1 on any difference, and on finding no tapes.
//   node docs/rust-host-spec/checker/check.mjs [file.md ...]
//   node docs/rust-host-spec/checker/check.mjs --record <in-lines.jsonl>   prints a tape for the `in` lines given
// Needs the host built (pnpm --filter @wsp/host build). The comparison harness (#2443) reads the same blocks.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync, mkdirSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const specDir = join(here, "..");
const repo = join(specDir, "..", "..");
const bin = join(repo, "packages", "host", "dist", "bin.js");
const REPLY_WAIT_MS = 10_000;
const SETTLE_MS = 250;

/** Every tape block in a file: its line number and its parsed lines. */
function tapesIn(file) {
  const lines = readFileSync(file, "utf8").split("\n");
  const out = [];
  for (let i = 0; i < lines.length; i++) {
    if (!/^```tape\s*$/.test(lines[i])) continue;
    const start = i + 1;
    let j = start;
    while (j < lines.length && !/^```\s*$/.test(lines[j])) j++;
    out.push({ at: `${basename(file)}:${start}`, lines: lines.slice(start, j).filter(l => l.trim() !== "").map(l => JSON.parse(l)) });
    i = j;
  }
  return out;
}

function freePort() {
  return new Promise((resolve, reject) => {
    const s = createServer();
    s.once("error", reject);
    s.listen(0, "127.0.0.1", () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });
}

/** A host of its own in an empty temp folder, started the way the lane's notes say: env -i, its own --state. */
async function startHost() {
  const dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
  mkdirSync(join(dir, "home"));
  mkdirSync(join(dir, "web"));
  writeFileSync(join(dir, "web", "index.html"), '<!doctype html><script>window.__WSP__ = window.__WSP__ || { token: "" };</script>\n');
  const port = await freePort();
  const env = { HOME: join(dir, "home"), PATH: "/usr/bin:/bin", WSP_HOME: join(dir, "home", ".wsp"), WSP_WEB_DIR: join(dir, "web") };
  const child = spawn(process.execPath, [bin, "up", "--port", String(port), "--state", join(dir, "state.json"), "--no-relay"], { cwd: dir, env, stdio: ["ignore", "pipe", "pipe"] });
  let said = "";
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`host did not serve in 20 s:\n${said}`)), 20_000);
    const read = chunk => {
      said += chunk;
      if (said.includes("runtime ws")) { clearTimeout(timer); resolve(); }
    };
    child.stdout.on("data", read);
    child.stderr.on("data", read);
    child.once("exit", code => { clearTimeout(timer); reject(new Error(`host exited ${code}:\n${said}`)); });
  });
  const token = readFileSync(join(dir, "host-token"), "utf8").trim();
  const stop = async () => {
    child.removeAllListeners("exit");
    const gone = new Promise(r => child.once("exit", r));
    child.kill("SIGTERM");
    await Promise.race([gone, new Promise(r => setTimeout(r, 2_000))]);
    if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
    rmSync(dir, { recursive: true, force: true });
  };
  return { url: `ws://127.0.0.1:${port}/ws`, token, dir, stop };
}

/** Sends a tape's `in` frames in order and answers every line that passed on the socket, in order. A frame with an id
 * waits for the reply carrying that id; every frame then waits SETTLE_MS for events that trail the reply. */
async function replay(host, ins) {
  const got = [];
  const waiting = new Map();
  const ws = new WebSocket(host.url);
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = () => reject(new Error(`could not open ${host.url}`));
  });
  let closed = false;
  ws.onclose = e => {
    closed = true;
    got.push({ close: { code: e.code, reason: e.reason } });
    for (const r of waiting.values()) r();
  };
  ws.onmessage = ev => {
    const frame = JSON.parse(String(ev.data));
    got.push({ out: frame });
    if (typeof frame.op !== "string" && waiting.has(frame.id)) {
      waiting.get(frame.id)();
      waiting.delete(frame.id);
    }
  };
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  for (const frame of ins) {
    if (closed) break;
    got.push({ in: frame });
    const text = (typeof frame === "string" ? frame : JSON.stringify(frame).replaceAll("<host-token>", host.token)).replaceAll("<tmp>", host.dir);
    const replied = typeof frame === "object" && frame.id !== undefined ? new Promise(r => waiting.set(frame.id, r)) : Promise.resolve();
    ws.send(text);
    let timer;
    await Promise.race([replied, new Promise(r => { timer = setTimeout(r, REPLY_WAIT_MS); })]);
    clearTimeout(timer);
    await sleep(SETTLE_MS);
  }
  if (!closed) ws.close();
  return got.map(line => JSON.parse(JSON.stringify(line).replaceAll(host.token, "<host-token>")));
}

/** What differs between two runs of one host and is not behaviour: the host's version, every uuid and prefixed id by
 * first sight (the event stream's id among them), its temp folder as a path and as the dashed key a project's memory
 * folder is named by, wall-clock times, and the minted values a fresh
 * host answers differently each run, each matched by its key and its shape: a ticket, a pairing code, the host key's
 * fingerprint. #2443's harness owns the full list. */
const MINTED = {
  ticket: /^[A-Za-z0-9_-]{32}$/,
  code: /^[0-9A-HJKMNP-TV-Z]{8}$/,
  hostKey: /^SHA256:[A-Za-z0-9+/]{43}$/,
};
const IDS = /\b(?:(ws|pr|h)_[0-9a-f]{8}|(p|d)_[0-9a-f]{16}|(a)_[0-9a-f]{12})\b/g;
const ISO = /\b\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{3})?Z\b/g;
function normal(lines, dir) {
  const seen = new Map();
  const counts = new Map();
  const first = (m, kind) => {
    if (!seen.has(m)) {
      counts.set(kind, (counts.get(kind) ?? 0) + 1);
      seen.set(m, `<${kind}-${counts.get(kind)}>`);
    }
    return seen.get(m);
  };
  const walk = (v, key) => {
    if (typeof v === "string") {
      if (MINTED[key]?.test(v)) return `<${key}>`;
      if (typeof key === "string" && /At$/.test(key) && /^<time[^>]* in ms>$/.test(v)) return `<${key}>`;
      let s = dir === undefined ? v : v.replaceAll(dir, "<tmp>");
      if (dir !== undefined) s = s.replaceAll(dir.replaceAll("/", "-"), "<tmp-key>");
      s = s.replace(/\/tmp\/wsp-tape-[A-Za-z0-9]+/g, "<tmp>").replace(/-tmp-wsp-tape-[A-Za-z0-9]+/g, "<tmp-key>");
      s = s.replace(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g, m => first(m, "uuid"));
      s = s.replace(IDS, (m, a, b, c) => first(m, a ?? b ?? c));
      s = s.replace(ISO, "<iso>");
      return key === "version" && /^\d+\.\d+\.\d+/.test(s) ? "<version>" : s;
    }
    if (typeof key === "string" && /At$/.test(key) && Number.isInteger(v) && v > 1e12) return `<${key}>`;
    if (Array.isArray(v)) return v.map(x => walk(x, key));
    if (v !== null && typeof v === "object") return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x, k)]));
    return v;
  };
  return lines.map(l => JSON.stringify(walk(l, undefined)));
}

async function record(file) {
  const ins = readFileSync(file, "utf8").split("\n").filter(l => l.trim() !== "").map(l => JSON.parse(l).in);
  const host = await startHost();
  try {
    const got = await replay(host, ins);
    process.stdout.write(got.map(l => JSON.stringify(l)).join("\n") + "\n");
  } finally {
    await host.stop();
  }
}

async function check(files) {
  const tapes = files.flatMap(tapesIn);
  if (tapes.length === 0) {
    console.log("no tape blocks found");
    process.exit(1);
  }
  let failed = 0;
  for (const tape of tapes) {
    const ins = tape.lines.filter(l => "in" in l).map(l => l.in);
    const host = await startHost();
    let got;
    try {
      got = await replay(host, ins);
    } catch (e) {
      failed++;
      console.log(`FAIL  ${tape.at}: ${e.message}`);
      await host.stop();
      continue;
    }
    await host.stop();
    const want = normal(tape.lines, undefined);
    const have = normal(got, host.dir);
    const n = Math.max(want.length, have.length);
    let diff;
    for (let i = 0; i < n; i++) if (want[i] !== have[i]) { diff = i; break; }
    if (diff === undefined) {
      console.log(`ok    ${tape.at}: ${tape.lines.length} lines`);
    } else {
      failed++;
      console.log(`FAIL  ${tape.at}: line ${diff + 1}\n  tape: ${want[diff] ?? "(nothing)"}\n  host: ${have[diff] ?? "(nothing)"}`);
    }
  }
  console.log(`\n${tapes.length} tapes, ${tapes.length - failed} ok, ${failed} failed`);
  process.exit(failed === 0 ? 0 : 1);
}

const args = process.argv.slice(2);
if (args[0] === "--record") await record(args[1]);
else await check(args.length > 0 ? args : readdirSync(specDir).filter(f => f.endsWith(".md")).sort().map(f => join(specDir, f)));
