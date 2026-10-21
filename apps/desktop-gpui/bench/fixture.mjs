// SPDX-License-Identifier: AGPL-3.0-only
// Called by fixture.sh as `node fixture.mjs <empty temp folder> <out.json> <n more> [bare]`, with the environment already cleared and
// HOME that folder: starts a throwaway host on a fresh state file there, records the bench's threads on it through the
// wsp command run in this process, and leaves the host serving until `stop.sh <out.json>`.
import { spawn, spawnSync } from "node:child_process";
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { THREADS, turnsOf } from "./content.mjs";
import { sessionNames } from "./session.mjs";
import { seededState } from "./seed.mjs";
import { shot } from "./real.mjs";
import { encode } from "../parity/png.mjs";

const MARK = ".wsp-bench-fixture";
const here = dirname(fileURLToPath(import.meta.url));
const HOST = join(here, "../../../packages/host/dist");

const [dir, out, more = "0", only] = process.argv.slice(2);
// The n short threads alone: the long and real-shaped threads take minutes on a busy machine and a turn's snapshot can
// time out there, and a bench that steps through top-level threads never opens them.
const bare = only === "bare";
if (out === undefined || process.env.HOME !== dir) {
  console.error("run fixture.sh <out.json>, which clears the environment first");
  process.exit(2);
}
for (const built of [join(HOST, "bin.js"), join(here, "../../web/dist/index.html")]) {
  if (!existsSync(built)) {
    console.error(`${built} is not built: run pnpm install && pnpm build`);
    process.exit(1);
  }
}
const { cli } = await import(pathToFileURL(join(HOST, "index.js")).href);

writeFileSync(join(dir, MARK), "");
mkdirSync(join(dir, "bin"));
mkdirSync(join(dir, "state"));
symlinkSync(join(here, "claude-stub.mjs"), join(dir, "bin", "claude"));
const state = join(dir, "state", "state.json");
const began = Date.now();
if (process.env.BENCH_SEED) {
  const { STATE_SHAPE, DAEMON_VERSION } = await import(pathToFileURL(join(here, "../../../packages/protocol/dist/index.js")).href);
  const seeded = seededState({ threads: Number(process.env.BENCH_SEED), workspaces: Math.ceil(Number(process.env.BENCH_SEED) / 10), dir });
  for (const project of Object.values(seeded.projects)) mkdirSync(project.path, { recursive: true });
  const shape = { shape: STATE_SHAPE, wsp: "fixture", daemon: DAEMON_VERSION, bin: "fixture.mjs", at: new Date().toISOString() };
  writeFileSync(state, `${JSON.stringify({ $shape: shape, ...seeded })}\n`);
}

const log = openSync(join(dir, "up.log"), "a");
// The real host's shape: this computer carries a display name apart from its hostname, as a Mac's does (real-host.mjs).
const LABEL = "zingzy's MacBook Pro";
const labelled = { ...process.env, NODE_OPTIONS: `--import=${pathToFileURL(join(here, "real-host.mjs")).href}`, WSP_FIXTURE_LABEL: LABEL };
const host = spawn(process.execPath, [join(HOST, "bin.js"), "up", "--port", "0", "--no-relay", "--state", state], { cwd: dir, env: labelled, stdio: ["ignore", log, log], detached: true });
host.unref();
closeSync(log);
// Written again with the threads at the end; written now so the caller can stop the host if this process is interrupted.
writeFileSync(out, `${JSON.stringify({ state, pid: host.pid, threads: {} }, null, 2)}\n`);

function fail(step, said) {
  console.error(`fixture: ${step} failed\n${said ?? ""}\nhost log:\n${readFileSync(join(dir, "up.log"), "utf8").slice(-4000)}`);
  writeFileSync(join(dir, "partial.json"), JSON.stringify({ state, pid: host.pid }));
  spawnSync(join(here, "stop.sh"), [join(dir, "partial.json")], { stdio: "inherit" });
  process.exit(1);
}

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

/** How many times a line the host did not answer is run again, and the pause before each. */
const RETRIES = 4;
const RETRY_MS = 3_000;

/** One wsp line against the fixture's host, run in this process; `false` for a host start means a line that would
 * start one is refused instead. */
async function wsp(...args) {
  for (let retry = 1; ; retry++) {
    const said = [];
    const asked = question => Promise.reject(new Error(`wsp ${args[0]} asked: ${question}`));
    const io = { log: line => void said.push(line), error: line => void said.push(line), ask: asked, askSecret: asked };
    const code = await cli([...args, "--state", state], io, undefined, process.env, false, { cwd: dir });
    if (code === 0) return said.join("\n");
    // The dial's window covers the socket and the auth alone, so a line refused in it never reached the host.
    const unanswered = said.find(line => /\bthe host at \S+ did not answer\b/.test(line));
    if (unanswered === undefined || retry > RETRIES) fail(`wsp ${args.slice(0, 2).join(" ")}`, said.join("\n"));
    console.log(`fixture: retry ${retry}: ${unanswered}`);
    await sleep(RETRY_MS);
  }
}

const until = Date.now() + 30_000;
/** The port the host's lock names, once it names one: until the host has bound a port the lock holds the 0 asked for. */
const lockedPort = () => {
  try {
    return JSON.parse(readFileSync(join(dir, "state", "host.lock"), "utf8")).port || 0;
  } catch {
    return 0;
  }
};
while (!existsSync(join(dir, "state", "host-token")) || lockedPort() === 0) {
  if (host.exitCode !== null || Date.now() > until) fail("wsp up");
  await sleep(100);
}

const folder = join(dir, "bench");
mkdirSync(folder);
const git = (...args) => spawnSync("git", ["-c", "user.name=bench", "-c", "user.email=bench@example.com", "-c", "commit.gpgsign=false", ...args], { cwd: folder, stdio: "ignore" });
git("init", "-q", "-b", "main");
git("commit", "-q", "--allow-empty", "-m", "bench");
await wsp("add", folder, "--name", "bench");

/** The host log's length in bytes, so a wait reads only what the host logged after it. */
const logged = () => statSync(join(dir, "up.log")).size;

/** Waits until the host has recorded what `n` turns of the thread changed: it reads a turn's changes after the turn
 * ends, a minute or more later on a busy machine, and a turn sent before that read lands can leave the card out. False
 * once the host logs after byte `from` that it took no snapshot of the folder in time, and the turn records nothing. */
async function changesRecorded(thread, n, from) {
  const until = Date.now() + 1_800_000;
  for (;;) {
    const read = JSON.parse(await wsp("thread", "read", thread, "--json")).messages;
    if (read.filter(r => r.who === "tool" && /^Changed \d+ files?/.test(r.text)).length >= n) return true;
    if (readFileSync(join(dir, "up.log")).subarray(from).toString().includes("no snapshot of ")) return false;
    if (Date.now() > until) fail(`waiting for what turn ${n} of ${thread} changed`);
    await sleep(250);
  }
}

/** How many times a turn whose snapshot the host missed is sent again before the fixture gives up. */
const RESENDS = 3;
/** The host's window for a turn's snapshot (`turnSnapshotMs`), whose timer it starts before a turn's end reaches a line. */
const SNAPSHOT_MS = 3_000;

const threads = {};
const changing = {};
let answered = 0;
for (const name of bare ? [] : [...Object.keys(THREADS), ...sessionNames(process.env.BENCH_SESSION)]) {
  const started = Date.now();
  for (const [i, turn] of turnsOf(name).entries()) {
    const writes = (turn.files ?? []).length > 0;
    if (writes) {
      // The last turn's closing snapshot is due by now, and a host that answers a line after that has logged any miss
      // of it, however late it runs, so a miss logged from here on is this turn's.
      await sleep(answered + SNAPSHOT_MS - Date.now());
      await wsp("thread", "head", threads[name]);
    }
    writeFileSync(join(dir, "bench-turn.json"), JSON.stringify({ thread: name, turn: i }));
    if (i === 0) {
      const said = await wsp("run", "bench", "--agent", "claude", turn.prompt);
      threads[name] = /\bthread ([0-9a-f-]{36})\b/.exec(said)?.[1] ?? fail(`reading ${name}'s thread id`, said);
    } else {
      const files = (turn.images ?? []).map(image => {
        const path = join(dir, "shots", image.name);
        mkdirSync(dirname(path), { recursive: true });
        writeFileSync(path, encode(shot(image), 2));
        return ["--file", path];
      });
      // What the turn's files held before it, put back before a resend so the stub's same bytes change them again.
      const before = (turn.files ?? []).map(f => join(folder, f.path)).map(path => [path, existsSync(path) ? readFileSync(path) : undefined]);
      let from = logged();
      await wsp("send", threads[name], ...files.flat(), turn.prompt);
      if (writes) changing[name] = (changing[name] ?? 0) + 1;
      for (let resend = 1; writes && !(await changesRecorded(threads[name], changing[name], from)); resend++) {
        if (resend > RESENDS) fail(`turn ${i + 1} of ${name}, sent ${resend} times with no snapshot taken`);
        for (const [path, was] of before) was === undefined ? rmSync(path, { force: true }) : writeFileSync(path, was);
        console.log(`fixture: resend ${resend} of turn ${i + 1} of ${name}, whose snapshot the host missed`);
        from = logged();
        await wsp("send", threads[name], ...files.flat(), turn.prompt);
      }
    }
    answered = Date.now();
    if ((i + 1) % 100 === 0) console.log(`fixture: ${name}, ${i + 1} turns`);
  }
  console.log(`fixture: ${name} ${threads[name]}, ${turnsOf(name).length} turns in ${((Date.now() - started) / 1000).toFixed(1)} s`);
}

// The real host's shape, kept apart from the bench's threads and their timing: a project with no thread, and one thread
// every right panel pane can open on, a slate included. The first run is an empty home, which baseline.mjs makes.
const realHost = { label: LABEL };
if (!bare) {
  const self = JSON.parse(await wsp("computers", "--json")).computers.find(c => c.id === "here");
  if (self?.label !== LABEL) fail("the display name", `this computer reads ${JSON.stringify(self)}, with no label ${LABEL}`);
  realHost.hostname = self.name;
  const quiet = join(dir, "quiet");
  mkdirSync(quiet);
  const quietGit = (...args) => spawnSync("git", ["-c", "user.name=bench", "-c", "user.email=bench@example.com", "-c", "commit.gpgsign=false", ...args], { cwd: quiet, stdio: "ignore" });
  quietGit("init", "-q", "-b", "main");
  quietGit("commit", "-q", "--allow-empty", "-m", "quiet");
  await wsp("add", quiet, "--name", "quiet");
  realHost.quietProject = { name: "quiet", path: quiet };
  writeFileSync(join(dir, "bench-turn.json"), JSON.stringify({ thread: "short-1", turn: 0 }));
  const said = await wsp("run", "bench", "--agent", "claude", "Every pane opens on this thread.");
  realHost.panesThread = /\bthread ([0-9a-f-]{36})\b/.exec(said)?.[1] ?? fail("reading the panes thread's id", said);
  const slate = join(dir, "real-host.slate");
  writeFileSync(slate, '<slate title="Real host">\n  <column>\n    <text value="Every pane opens once." />\n  </column>\n</slate>\n');
  await wsp("slate", "write", realHost.panesThread, slate);
  realHost.slate = slate;
  console.log(`fixture: real host shape: ${LABEL} on ${realHost.hostname}, project quiet with no thread, panes thread ${realHost.panesThread}`);
}

// Top-level threads of one turn each, newest at the top of the sidebar, for the Ctrl+Tab bench to step through.
const extra = [];
for (let i = 0; i < Number(more); i++) {
  writeFileSync(join(dir, "bench-turn.json"), JSON.stringify({ thread: "short-1", turn: 0 }));
  const said = await wsp("run", "bench", "--agent", "claude", `Step thread ${i + 1}.`);
  extra.push(/\bthread ([0-9a-f-]{36})\b/.exec(said)?.[1] ?? fail(`reading step thread ${i + 1}'s id`, said));
}
if (extra.length > 0) console.log(`fixture: ${extra.length} more threads, the newest ${extra.at(-1)}`);

writeFileSync(out, `${JSON.stringify({ state, pid: host.pid, threads, realHost, extra }, null, 2)}\n`);
console.log(`fixture: host pid ${host.pid} serves ${state}, made in ${((Date.now() - began) / 1000).toFixed(1)} s`);
process.exit(0);
