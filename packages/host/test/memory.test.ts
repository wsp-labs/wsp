// SPDX-License-Identifier: AGPL-3.0-only
// What the host costs the computer it runs on, measured rather than claimed:
// one child process is the host, a scripted day of threads and turns goes
// through it, and what it still holds once it goes quiet is read off V8 after
// a full collection. Resident size is read too and named on a red run, but it
// is not the budget: an empty node is already past this budget in mapped
// binary alone (41 MB on this Linux runner), and resident size follows V8's
// high water mark rather than what the host kept.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DIST, describeWithDists, distOf } from "./built-bin.js";

/** The budget the landing page promises for what the host still holds after a day of agents: change it here and the
 * page goes red until it says the same thing. */
const HOST_MEMORY_BUDGET_MB = 40;

/** What a day of turns may add to what the host holds once it has started. Start-up is the code and the schemas every
 * feature loads, so a cap on the whole moved with each one and failed branches that never touched what a turn keeps:
 * under Node 22 on Linux the day added 3.7 MB on 2026-10-06 before the Slate landed (31.2 MB at start-up), with it
 * (33.3) and once start-up shed 9 MB (23.4), and 3.9 under Node 24. Start-up answers to the budget alone. On
 * 2026-10-11 the transcript caps moved from each folder to each thread, so the day's four threads keep all their events
 * where the folder kept 5000, and the index of their words took the day from 4.9 to 5.2. */
const HOST_DAY_CAP_MB = 6;

/** The scripted day: ten threads that run one turn first thing, each replying 18 KB that ends on one word, then four
 * threads a person keeps open in the project's folder, thirty turns each, forty deltas a turn
 * and a reply whose last line runs to the 200 characters a row keeps, and four subagents a turn that each start with
 * the 280 characters of what they were asked, name their model, say ten lines and end, then one thread on a branch in
 * a worktree of its own with one turn. That is past the 5000 events a folder kept before each thread kept its own, so the
 * host is measured holding every thread's events whole and every child they hold kept under its thread. */
const THREADS = 4;
const TURNS_PER_THREAD = 30;
const DELTAS_PER_TURN = 40;
const SUBAGENTS_PER_TURN = 4;
const SUBAGENT_DELTAS = 10;
/** Threads that ran one turn first thing and finished, each reply about 18 KB ending on one word. Each row also holds
 * its latest turn's whole result through the turn's handle until the thread's next turn, so the day reads the same
 * whatever the line keeps; what the line keeps is read on its own below. */
const MORNING_THREADS = 10;
const MORNING_REPLY_WORDS = 3000;
const DAY_TURNS = MORNING_THREADS + THREADS * TURNS_PER_THREAD + 1;
/** Projects that each wear an image of their own through the day. */
const IMAGES = 50;

interface Reading {
  heldMb: number;
  /** What the host held once started, with its project, workspace and worktree made and no turn run yet. */
  startMb?: number;
  rssMb: number;
  turns: number;
  /** Each collection's figure from the last turn until one freed nothing more. */
  readings?: number[];
  /** The folder of threads that talk: what a search found and skipped, and how long a search and a delete held the loop. */
  talk?: { found: number; unsearched: number; opened: number; searchMs: number; deleteMs?: number; deleteHeapMb?: number };
}

/** The host as its own process: the wiring `wsp up` builds, a local workspace, and one harness that answers with a
 * turn's worth of events instead of starting an agent, so no machine and no agent is involved in the measurement. */
const hostScript = (home: string): string => `
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { crc32, deflateSync } from "node:zlib";
import { createRuntime, sqliteStore } from ${JSON.stringify(distOf("runtime"))};
import { recipeShelf, startHost, localWiring, stateWriterHere } from ${JSON.stringify(DIST)};
import { fakeCopier, NoProviderBackend } from ${JSON.stringify(distOf("engine"))};
import { DAEMON_VERSION } from ${JSON.stringify(distOf("protocol"))};

const home = ${JSON.stringify(home)};
const webDir = join(home, "web");
mkdirSync(join(webDir, "assets"), { recursive: true });
writeFileSync(join(webDir, "assets", "app.js"), "console.log('app')\\n");
writeFileSync(join(webDir, "index.html"), '<!doctype html><html><head><script type="module" crossorigin src="/assets/app.js"></script></head><body><div id="root"></div><script>window.__WSP__ = window.__WSP__ || { token: "" };</script></body></html>');
const statePath = join(home, "state", "state.json");
mkdirSync(join(home, "state"), { recursive: true });

let nth = 0;
const scripted = () => ({
  steers: false,
  start: ({ onEvent, prompt }) => {
    const sessionId = \`00000000-0000-4000-8000-\${String(++nth).padStart(12, "0")}\`;
    const finished = (async () => {
      onEvent({ type: "session.start", sessionId });
      for (let i = 0; i < ${DELTAS_PER_TURN}; i++) onEvent({ type: "turn.delta", sessionId, kind: "text", text: "token ".repeat(16) });
      for (let c = 0; c < ${SUBAGENTS_PER_TURN}; c++) {
        const task = \`a\${nth.toString(16).padStart(12, "0")}\${c}\`;
        const parentToolUseId = \`toolu_\${task}\`;
        onEvent({ type: "subagent", sessionId, task, state: "running", parentToolUseId, title: "Count to thirty with one Bash call per number", depth: 1, asked: "count ".repeat(47).slice(0, 280) });
        onEvent({ type: "subagent", sessionId, task, state: "running", parentToolUseId, model: "claude-haiku-4-5" });
        for (let i = 0; i < ${SUBAGENT_DELTAS}; i++) onEvent({ type: "turn.delta", sessionId, kind: "text", text: "token ".repeat(16), parentToolUseId });
        onEvent({ type: "subagent", sessionId, task, state: "done", parentToolUseId, summary: "COUNT-FINISHED" });
      }
      // A morning thread's reply runs long and ends on one word, a pull request's address: the line its row keeps is
      // cut from it, and the day's turns after it push the reply itself out of the transcript.
      const text = prompt === "morning" ? "reply ".repeat(${MORNING_REPLY_WORDS}) + "\\nhttps://example.invalid/acme/lab/pull/" + nth : "done\\n" + "reply ".repeat(40);
      const result = { status: "completed", text };
      onEvent({ type: "turn.done", sessionId, result });
      onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
      return result;
    })();
    return { localId: sessionId, finished, interrupt: async () => {} };
  },
});

// The worktree road and the daemon beside the host are the fakes: this checkout stages no daemon binary, and what is
// measured is the host's own bookkeeping.
const copier = fakeCopier();
const daemon = async () => ({ version: DAEMON_VERSION, road: { url: "http://127.0.0.1:1", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "t" }, sysSamples: async () => () => {}, close: async () => {} });
const store = sqliteStore(statePath, stateWriterHere());
const runtime = createRuntime({
  statePath,
  backend: new NoProviderBackend(),
  local: localWiring(home, undefined, daemon, statePath, copier),
  store,
  adapters: { claude: scripted },
});
const host = await startHost({ runtime, webDir, port: 0, statePath });
// Threads run in the project's folder, and one more in a worktree of a branch of its own.
const folder = join(home, "repo");
mkdirSync(folder, { recursive: true });
execFileSync("git", ["init", "-q", "-b", "main", folder]);
const project = await host.addProject(folder);
const workspace = await host.createWorkspace("here", undefined, project.id);
const worktree = (await runtime.workspaces.folderFor({ project: project.id, branch: "feat/side" })).workspace;
// Fifty more projects, each to wear an image of its own during the day.
const worn = [];
for (let i = 0; i < ${IMAGES}; i++) {
  const at = join(home, \`lab-\${i}\`);
  mkdirSync(at, { recursive: true });
  worn.push((await host.addProject(at)).id);
}

// The host writes its index and its transcripts behind a queue. What it holds is only known once it has been left
// alone the way an idle minute leaves it, so this reads until a collection frees nothing more: the drain falls from
// about 41 MB to the floor, and stopping at two reads within a megabyte could stop on the way down.
// external covers the buffers a socket frame and a queued write live in, arrayBuffers among them, which the heap
// alone does not count.
const held = () => {
  global.gc();
  global.gc();
  const m = process.memoryUsage();
  return (m.heapUsed + m.external) / 1048576;
};
const quiet = async () => {
  let before = Infinity;
  let after = held();
  const readings = [after];
  for (let i = 0; i < 60 && Math.abs(before - after) > 0.1; i++) {
    await sleep(250);
    before = after;
    after = held();
    readings.push(after);
  }
  return readings;
};
const start = (await quiet()).at(-1);

const turn = async (at, thread, prompt = "go") => {
  const handle = await runtime.sessions.start(at.id, { prompt, harness: "claude", ...(thread === undefined ? {} : { thread }) });
  await handle.finished?.catch(() => {});
  return { at, thread: handle.view().threadId ?? handle.view().id };
};
for (let t = 0; t < ${MORNING_THREADS}; t++) await turn(workspace, undefined, "morning");
const threads = [];
for (let t = 0; t < ${THREADS}; t++) threads.push(await turn(workspace, undefined));
for (let n = 1; n < ${TURNS_PER_THREAD}; n++) for (const { at, thread } of threads) await turn(at, thread);
await turn(worktree, undefined);

// A recipe saved and listed, and a computer that joined and waits on its picks, as an add from the app leaves one.
const picks = {
  name: "laptop",
  agents: { claude: { signin: "vault" }, codex: { signin: "machine" } },
  mcp: { linear: { agents: ["claude", "codex"] } },
  clis: Object.fromEntries(["gh", "jq", "ripgrep", "fd", "bat"].map(name => [name, { via: "brew" }])),
  skills: Object.fromEntries(Array.from({ length: 20 }, (_, i) => [\`skill-\${i}\`, { from: "~/.claude/skills" }])),
  folders: { repo: { from: "~/repo", keep: [] } },
  configs: { git: {}, shell: {}, github: { signin: "vault" } },
};
const shelf = recipeShelf({ statePath, home });
await shelf.save(picks);
await shelf.list();
await store.put("pending-computers", "a_mem", { id: "a_mem", address: "root@10.0.0.9", step: "choosing", choices: picks, recipe: "laptop", startedAt: new Date().toISOString(), placeId: "p_mem" });

// Each project wears a distinct 128 px image of noise, the most a kept PNG weighs, and every window's batch read of all
// fifty is asked ten times: the files stay on disk and the host holds none of their bytes between asks.
const chunk = (type, data) => {
  const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0);
  body.copy(out, 4);
  out.writeUInt32BE(crc32(body), 8 + data.length);
  return out;
};
const head = Buffer.alloc(13);
head.writeUInt32BE(128, 0);
head.writeUInt32BE(128, 4);
head.set([8, 6, 0, 0, 0], 8);
const hashes = [];
for (const [i, id] of worn.entries()) {
  const rows = Buffer.alloc(128 * 513);
  for (let b = 0; b < rows.length; b++) rows[b] = b % 513 === 0 ? 0 : (Math.imul(b + 1, 2654435761 + i) >>> 13) & 255;
  const png = Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", head), chunk("IDAT", deflateSync(rows)), chunk("IEND", Buffer.alloc(0))]);
  hashes.push((await runtime.projects.icon(id, png.toString("base64"))).image);
}
for (let n = 0; n < 10; n++) if (Object.values(await runtime.projects.icons(hashes)).some(url => url === null)) throw new Error("an image went missing");

const readings = await quiet();
const after = readings.at(-1);
console.log(\`measured \${JSON.stringify({ heldMb: +after.toFixed(1), startMb: +start.toFixed(1), rssMb: +(process.memoryUsage().rss / 1048576).toFixed(1), turns: ${DAY_TURNS}, readings: readings.map(r => +r.toFixed(1)) })}\`);
await host.close();
process.exit(0);
`;

/** A folder of threads that talk: each one turn whose reply is this many characters of words, past what the host holds
 * of words for search all together. */
const TALKING_THREADS = 300;
const TALKING_REPLY = 30 * 1024;
/** The longest a search or a delete in that folder may hold the host's loop, the best of three tries: reading back every
 * thread whose words were let go held it 100 to 360 ms a search, and parsing the folder's every row twice held a delete
 * 1.1 s (2026-10-11). */
const LOOP_HELD_MS = 50;
/** What a delete there may grow the heap by: the folder parsed whole grew it 245 MB. */
const DELETE_HEAP_MB = 20;

/** The host on one folder of TALKING_THREADS threads, measured once they are quiet, or, started again on that state
 * as a host of its own, once it is. Both times it searches for the oldest thread's words, which it let go, times a
 * short search, opens the oldest thread and searches again; started again it also times deleting one thread. */
const talkingScript = (home: string, again: boolean): string => `
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { monitorEventLoopDelay } from "node:perf_hooks";
import { setTimeout as sleep } from "node:timers/promises";
import { createRuntime, sqliteStore } from ${JSON.stringify(distOf("runtime"))};
import { startHost, localWiring, stateWriterHere } from ${JSON.stringify(DIST)};
import { fakeCopier, NoProviderBackend } from ${JSON.stringify(distOf("engine"))};
import { DAEMON_VERSION } from ${JSON.stringify(distOf("protocol"))};

const home = ${JSON.stringify(home)};
const again = ${again};
const webDir = join(home, "web");
mkdirSync(webDir, { recursive: true });
writeFileSync(join(webDir, "index.html"), '<!doctype html><html><body><script>window.__WSP__ = window.__WSP__ || { token: "" };</script></body></html>');
const statePath = join(home, "state", "state.json");
mkdirSync(join(home, "state"), { recursive: true });
let nth = 0;
const talking = () => ({
  steers: false,
  start: ({ onEvent, prompt }) => {
    const sessionId = \`00000000-0000-4000-8000-\${String(++nth).padStart(12, "0")}\`;
    const finished = (async () => {
      onEvent({ type: "session.start", sessionId });
      // Text of its own in every piece, as a reply read off an agent's stream is: one string repeated would be held as
      // references to it, and the words would cost nothing until a restart read them back.
      const piece = i => \`\${nth}.\${i} \${"words of a long reply ".repeat(93)}\`;
      onEvent({ type: "turn.delta", sessionId, kind: "text", text: \`marker-\${prompt} \`, messageId: "m" });
      for (let i = 0; i * piece(0).length < ${TALKING_REPLY}; i++) onEvent({ type: "turn.delta", sessionId, kind: "text", text: piece(i), messageId: "m" });
      const result = { status: "completed", text: "done" };
      onEvent({ type: "turn.done", sessionId, result });
      onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
      return result;
    })();
    return { localId: sessionId, finished, interrupt: async () => {} };
  },
});
const daemon = async () => ({ version: DAEMON_VERSION, road: { url: "http://127.0.0.1:1", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "t" }, sysSamples: async () => () => {}, close: async () => {} });
const up = async () => {
  const runtime = createRuntime({ statePath, backend: new NoProviderBackend(), local: localWiring(home, undefined, daemon, statePath, fakeCopier()), store: sqliteStore(statePath, stateWriterHere()), adapters: { claude: talking } });
  return { runtime, host: await startHost({ runtime, webDir, port: 0, statePath }) };
};
const held = () => {
  global.gc();
  global.gc();
  const m = process.memoryUsage();
  return (m.heapUsed + m.external) / 1048576;
};
const quiet = async () => {
  let before = Infinity;
  let after = held();
  for (let i = 0; i < 60 && Math.abs(before - after) > 0.1; i++) {
    await sleep(250);
    before = after;
    after = held();
  }
  return after;
};
const { runtime, host } = await up();
if (!${again}) {
  const folder = join(home, "repo");
  mkdirSync(folder, { recursive: true });
  execFileSync("git", ["init", "-q", "-b", "main", folder]);
  const project = await host.addProject(folder);
  const workspace = await host.createWorkspace("here", undefined, project.id);
  for (let t = 0; t < ${TALKING_THREADS}; t++) {
    const handle = await runtime.sessions.start(workspace.id, { prompt: String(t).padStart(3, "0"), harness: "claude" });
    await handle.finished?.catch(() => {});
  }
}
const threads = (await runtime.sessions.list()).filter(v => v.threadId !== undefined).sort((a, b) => a.startedAt - b.startedAt);
const heldMb = await quiet();
/** The longest the loop was held while fn ran, and the most the heap grew over what it held before. */
const loopHeld = async fn => {
  const delay = monitorEventLoopDelay({ resolution: 1 });
  const before = process.memoryUsage().heapUsed;
  let peak = before;
  const probe = setInterval(() => (peak = Math.max(peak, process.memoryUsage().heapUsed)), 1);
  delay.enable();
  // The histogram's first sample is armed a tick after enable: a block in the same tick reads as none.
  await sleep(5);
  await fn();
  await sleep(5);
  delay.disable();
  clearInterval(probe);
  peak = Math.max(peak, process.memoryUsage().heapUsed);
  return { ms: +(delay.max / 1e6).toFixed(1), heapMb: +((peak - before) / 1048576).toFixed(1) };
};
/** The best of three tries, as a loaded machine can hold the loop 200 ms on an act that holds it 7 ms the next try. */
const bestOf = async fns => {
  const tries = [];
  for (const fn of fns) tries.push(await loopHeld(fn));
  return { ms: Math.min(...tries.map(t => t.ms)), heapMb: Math.min(...tries.map(t => t.heapMb)) };
};
const searched = await runtime.sessions.search("marker-000");
const short = await bestOf([0, 1, 2].map(() => () => runtime.sessions.search("th")));
const oldest = threads[0];
await runtime.sessions.page(oldest.workspaceId, { threadId: oldest.threadId });
let opened = 0;
for (let i = 0; i < 100 && opened === 0; i++, await sleep(10)) opened = (await runtime.sessions.search("marker-000")).hits.length;
const talk = { found: searched.hits.length, unsearched: searched.unsearched ?? 0, opened, searchMs: short.ms };
if (again) {
  const deleted = await bestOf([150, 151, 152].map(n => () => runtime.sessions.delete(threads[n].threadId)));
  Object.assign(talk, { deleteMs: deleted.ms, deleteHeapMb: deleted.heapMb });
}
console.log(\`measured \${JSON.stringify({ heldMb: +heldMb.toFixed(1), rssMb: +(process.memoryUsage().rss / 1048576).toFixed(1), turns: ${TALKING_THREADS}, talk })}\`);
await host.close();
process.exit(0);
`;

/** How long a measuring run may take before it is ended, so a host that will not close cannot outlive this file. */
const RUN_CAP_MS = 240_000;

/** Runs a script under a node of its own with collection exposed, and answers with everything it said. */
function ran(script: string, home: string): Promise<{ out: string; code: number | null }> {
  return new Promise((resolve, reject) => {
    // V8 keeps a function's bytecode until six collections pass with no call, and a forced collection does not count:
    // a host that made less garbage on the way reached the quiet minute holding 1.5 MB more of its start-up code than
    // one that collected ten times, with the same records held. So code no turn runs is let go at every collection.
    const child = spawn(process.execPath, ["--expose-gc", "--stress-flush-code", "--input-type=module", "-e", script], {
      cwd: home,
      env: { ...process.env, HOME: home, WSP_HOME: home },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let out = "";
    child.stdout.on("data", (chunk: Buffer) => (out += chunk.toString()));
    child.stderr.on("data", (chunk: Buffer) => (out += chunk.toString()));
    const cap = setTimeout(() => child.kill("SIGKILL"), RUN_CAP_MS);
    child.once("error", error => {
      clearTimeout(cap);
      reject(error);
    });
    child.once("exit", code => {
      clearTimeout(cap);
      resolve({ out, code });
    });
  });
}

const reading = (out: string, who: string): Reading => {
  const line = out.split("\n").find(l => l.startsWith("measured "));
  if (line === undefined) throw new Error(`${who} printed no measurement:\n${out}`);
  return JSON.parse(line.slice("measured ".length)) as Reading;
};

/** Every module the host's library and the runtime it starts load, as URLs. register, not registerHooks: the engines
 * floor is Node 22.0 and registerHooks came in 22.15. Its hooks run on a thread of their own, so the list is read once
 * the hook has seen the last import, which the port keeps in order. */
async function startLoads(): Promise<string[]> {
  const hook = "let port; export const initialize = data => { port = data.port; }; export const load = (url, context, next) => { port.postMessage(url); return next(url, context); };";
  const script = `
import { register } from "node:module";
import { MessageChannel } from "node:worker_threads";
const { port1, port2 } = new MessageChannel();
register(${JSON.stringify(`data:text/javascript,${encodeURIComponent(hook)}`)}, { data: { port: port2 }, transferList: [port2] });
const last = "data:text/javascript,export%20default%200";
const seen = new Promise(done => port1.on("message", url => (console.log(url), url === last && done())));
await import(${JSON.stringify(DIST)});
await import(${JSON.stringify(distOf("runtime"))});
await import(last);
await seen;
port1.close();
`;
  const run = await ran(script, tmpdir());
  expect(run.code, run.out).toBe(0);
  return run.out.split("\n").filter(url => url !== "");
}

describeWithDists("what the host loads to start", ["host", "runtime", "protocol"], () => {
  it("leaves the tool server's library until a tool server opens", async () => {
    // The library's schemas alone were 8 MB of the budget, held by every host whether or not an agent asked for tools.
    expect((await startLoads()).filter(url => url.includes("@modelcontextprotocol"))).toEqual([]);
  });

  it("leaves the TOML parser until a copy to a provider cuts a TOML config", async () => {
    // The parser rides inlined in the host's chunks, so each of its modules a start-up file holds is named by the
    // bundler's path line, and only one held as a factory waits for the first require.
    const eager = (await startLoads()).flatMap(url =>
      url.includes("smol-toml") ? [url] : url.startsWith("file:") ? [...readFileSync(fileURLToPath(url), "utf8").matchAll(/^\/\/ (\S*\/smol-toml\/\S+)\n(.*)$/gm)].filter(m => !/= __commonJS\(\{$/.test(m[2]!)).map(m => m[1]!) : [],
    );
    expect(eager).toEqual([]);
  });

  it("leaves the slate's code until a thread uses a slate", async () => {
    // Read off the files themselves, as the bundler names the chunk the parser and the slate store land in.
    const holding = (await startLoads()).filter(url => url.startsWith("file:") && /function (parseSlate|createSlates)\(/.test(readFileSync(fileURLToPath(url), "utf8")));
    expect(holding).toEqual([]);
  });
});

/** The lines a row keeps from replies, failures and prompts that run long and end or start on one word, such as an
 * address: past 12 characters V8 answers a piece of a string as a view into the whole. */
const LINES_KEPT = 500;

describeWithDists("what a row's line keeps of the text it was cut from", ["protocol"], () => {
  let home: string;

  beforeEach(() => {
    home = mkdtempSync(join(tmpdir(), "wsp-lines-"));
  });
  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
  });

  it(`holds ${LINES_KEPT * 3} last lines, failures and asks cut from 50 KB texts on one word in under 2 MB`, async () => {
    const script = `
import { listedFailure, listedLastLine, subagentAsked } from ${JSON.stringify(distOf("protocol"))};
global.gc();
const before = process.memoryUsage().heapUsed;
const kept = [];
for (let i = 0; i < ${LINES_KEPT}; i++) {
  kept.push(listedLastLine("reply ".repeat(8000) + "\\nhttps://example.invalid/acme/lab/pull/" + i));
  kept.push(listedFailure("https://example.invalid/acme/lab/runs/" + i + "\\n" + "x ".repeat(25000)));
  kept.push(subagentAsked("https://example.invalid/acme/lab/issues/" + i + "y".repeat(50000)));
}
global.gc();
global.gc();
console.log(\`measured \${JSON.stringify({ heldMb: +((process.memoryUsage().heapUsed - before) / 1048576).toFixed(1), rssMb: 0, turns: kept.length })}\`);
`;
    const run = await ran(script, home);
    expect(run.code, run.out).toBe(0);
    const held = reading(run.out, "the lines");
    expect(held.heldMb, `${held.turns} lines held ${held.heldMb} MB`).toBeLessThan(2);
  }, 120_000);
});

describeWithDists("what the host holds after a day of agents", ["host", "runtime", "engine"], () => {
  let home: string;

  beforeEach(() => {
    home = mkdtempSync(join(tmpdir(), "wsp-memory-"));
  });
  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
  });

  it(`stays under ${HOST_MEMORY_BUDGET_MB} MB and gains under ${HOST_DAY_CAP_MB} MB with ${DAY_TURNS} turns through it, the last in a worktree`, async () => {
    const empty = await ran('global.gc(); console.log(`measured ${JSON.stringify({ heldMb: 0, rssMb: +(process.memoryUsage().rss / 1048576).toFixed(1), turns: 0 })}`)', home);
    expect(empty.code, `an empty node on this runner said: ${empty.out}`).toBe(0);
    const floor = reading(empty.out, "an empty node");
    const run = await ran(hostScript(home), home);
    expect(run.code, run.out).toBe(0);
    const held = reading(run.out, "the host");
    const day = +(held.heldMb - held.startMb!).toFixed(1);
    const said = `the host held ${held.heldMb} MB after ${held.turns} turns, ${held.startMb} MB of it at start-up and ${day} MB from the day (resident ${held.rssMb} MB, an empty node on this runner ${floor.rssMb} MB; read ${(held.readings ?? []).join(", ")})`;
    // Printed on a pass too, so the margin a run kept can be read off CI before it is gone.
    console.log(said);
    expect(held.heldMb, said).toBeLessThanOrEqual(HOST_MEMORY_BUDGET_MB);
    expect(day, said).toBeLessThanOrEqual(HOST_DAY_CAP_MB);
  }, 300_000);
});

describeWithDists("what the host holds of a folder of threads that talk", ["host", "runtime", "engine"], () => {
  let home: string;

  beforeEach(() => {
    home = mkdtempSync(join(tmpdir(), "wsp-talking-"));
  });
  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
  });

  it(`stays under ${HOST_MEMORY_BUDGET_MB} MB with ${TALKING_THREADS} threads of ${TALKING_REPLY / 1024} KB of words in one folder, searches and deletes under ${LOOP_HELD_MS} ms, and finds the oldest one's once it is opened`, async () => {
    const day = await ran(talkingScript(home, false), home);
    expect(day.code, day.out).toBe(0);
    const restart = await ran(talkingScript(home, true), home);
    expect(restart.code, restart.out).toBe(0);
    const [first, again] = [reading(day.out, "the host"), reading(restart.out, "the host started again")];
    const said = `the host held ${first.heldMb} MB after ${first.turns} threads and ${again.heldMb} MB started again (resident ${first.rssMb} and ${again.rssMb} MB); searches ${JSON.stringify(first.talk)} and started again ${JSON.stringify(again.talk)}`;
    console.log(said);
    expect(Math.max(first.heldMb, again.heldMb), said).toBeLessThanOrEqual(HOST_MEMORY_BUDGET_MB);
    for (const { talk } of [first, again]) {
      expect(talk, said).toMatchObject({ found: 0, opened: 1 });
      expect(talk!.unsearched, said).toBeGreaterThan(TALKING_THREADS / 2);
      expect(talk!.searchMs, said).toBeLessThan(LOOP_HELD_MS);
    }
    expect(again.talk!.deleteMs, said).toBeLessThan(LOOP_HELD_MS);
    expect(again.talk!.deleteHeapMb, said).toBeLessThan(DELETE_HEAP_MB);
  }, 300_000);
});
