// SPDX-License-Identifier: AGPL-3.0-only
// Called by fixture.sh as `node fixture.mjs <empty temp folder> <out.json>`, with the environment already cleared and
// HOME that folder: starts a throwaway host on a fresh state file there, records the bench's threads on it through the
// wsp command run in this process, and leaves the host serving until `stop.sh <out.json>`.
import { spawn, spawnSync } from "node:child_process";
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { THREADS, turnsOf } from "./content.mjs";

const MARK = ".wsp-bench-fixture";
const here = dirname(fileURLToPath(import.meta.url));
const HOST = join(here, "../../../packages/host/dist");

const [dir, out] = process.argv.slice(2);
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

const log = openSync(join(dir, "up.log"), "a");
const host = spawn(process.execPath, [join(HOST, "bin.js"), "up", "--port", "0", "--no-relay", "--state", state], { cwd: dir, stdio: ["ignore", log, log], detached: true });
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

/** One wsp line against the fixture's host, run in this process; `false` for a host start means a line that would
 * start one is refused instead. */
async function wsp(...args) {
  const said = [];
  const asked = question => Promise.reject(new Error(`wsp ${args[0]} asked: ${question}`));
  const io = { log: line => void said.push(line), error: line => void said.push(line), ask: asked, askSecret: asked };
  const code = await cli([...args, "--state", state], io, undefined, process.env, false, { cwd: dir });
  if (code !== 0) fail(`wsp ${args.slice(0, 2).join(" ")}`, said.join("\n"));
  return said.join("\n");
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
  await new Promise(resolve => setTimeout(resolve, 100));
}

const folder = join(dir, "bench");
mkdirSync(folder);
const git = (...args) => spawnSync("git", ["-c", "user.name=bench", "-c", "user.email=bench@example.com", "-c", "commit.gpgsign=false", ...args], { cwd: folder, stdio: "ignore" });
git("init", "-q", "-b", "main");
git("commit", "-q", "--allow-empty", "-m", "bench");
await wsp("add", folder, "--name", "bench");

const threads = {};
for (const name of Object.keys(THREADS)) {
  const started = Date.now();
  for (const [i, turn] of turnsOf(name).entries()) {
    writeFileSync(join(dir, "bench-turn.json"), JSON.stringify({ thread: name, turn: i }));
    if (i === 0) {
      const said = await wsp("run", "bench", "--agent", "claude", turn.prompt);
      threads[name] = /\bthread ([0-9a-f-]{36})\b/.exec(said)?.[1] ?? fail(`reading ${name}'s thread id`, said);
    } else {
      await wsp("send", threads[name], turn.prompt);
    }
    if ((i + 1) % 100 === 0) console.log(`fixture: ${name}, ${i + 1} turns`);
  }
  console.log(`fixture: ${name} ${threads[name]}, ${THREADS[name].turns} turns in ${((Date.now() - started) / 1000).toFixed(1)} s`);
}

writeFileSync(out, `${JSON.stringify({ state, pid: host.pid, threads }, null, 2)}\n`);
console.log(`fixture: host pid ${host.pid} serves ${state}, made in ${((Date.now() - began) / 1000).toFixed(1)} s`);
process.exit(0);
