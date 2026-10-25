// SPDX-License-Identifier: AGPL-3.0-only
// The fixture host as the settings shots reach it: its page's address and token off the out.json fixture.sh wrote,
// and one runtime op sent over its socket with the host's own token, the caller the GPUI app is.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

/** The host a fixture's out.json names: the page's address, the token file and the token. */
export function hostOf(fixturePath) {
  const fixture = JSON.parse(readFileSync(fixturePath, "utf8"));
  const stateDir = dirname(fixture.state);
  const lock = JSON.parse(readFileSync(join(stateDir, "host.lock"), "utf8"));
  const tokenFile = join(stateDir, "host-token");
  return { ...fixture, url: `http://127.0.0.1:${lock.port}/`, tokenFile, token: readFileSync(tokenFile, "utf8").trim() };
}

/** Sends each frame in turn on one authed socket and answers their replies; a reply carrying ok:false throws. */
export async function ops(url, token, frames) {
  const ws = new WebSocket(`${url.replace(/^http/, "ws").replace(/\/$/, "")}/ws`);
  const replies = new Map();
  const waiting = new Map();
  ws.onmessage = m => {
    const msg = JSON.parse(String(m.data));
    if (typeof msg.id !== "number") return;
    const wait = waiting.get(msg.id);
    if (wait === undefined) replies.set(msg.id, msg);
    else waiting.delete(msg.id), wait(msg);
  };
  const reply = id => new Promise(resolve => (replies.has(id) ? resolve(replies.get(id)) : waiting.set(id, resolve)));
  await new Promise((resolve, reject) => ((ws.onopen = resolve), (ws.onerror = () => reject(new Error(`no socket at ${url}`)))));
  ws.send(JSON.stringify({ id: 0, op: "auth", token }));
  const authed = await reply(0);
  if (authed.ok === false) throw new Error(`auth refused: ${authed.error}`);
  const answers = [];
  try {
    for (const [i, frame] of frames.entries()) {
      ws.send(JSON.stringify({ ...frame, id: i + 1 }));
      const answer = await reply(i + 1);
      if (answer.ok === false) throw new Error(`${frame.op} refused: ${answer.error ?? JSON.stringify(answer)}`);
      answers.push(answer);
    }
  } finally {
    ws.close();
  }
  return answers;
}

/** The recipe the fixture saves, which a state that removes it saves again. */
export const RECIPE = { name: "Web app", agents: { claude: { signin: "machine" } }, mcp: { docs: { agents: ["claude"] } }, clis: { pnpm: { via: "npm" } }, skills: {}, plugins: {}, folders: {}, configs: { git: {} } };

/** Pairs a device by that name: a code the host's token issues, redeemed as the first frame of a socket of its own. */
export async function pair(url, token, name) {
  const [{ code }] = await ops(url, token, [{ op: "pair.issue" }]);
  const ws = new WebSocket(`${url.replace(/^http/, "ws").replace(/\/$/, "")}/ws`);
  const said = await new Promise((resolve, reject) => {
    ws.onopen = () => ws.send(JSON.stringify({ id: 1, op: "pair.redeem", code, name }));
    ws.onmessage = m => resolve(JSON.parse(String(m.data)));
    ws.onerror = () => reject(new Error("pair.redeem: no socket"));
  });
  ws.close();
  if (said.ok !== true) throw new Error(`pair.redeem refused: ${said.error}`);
}

/** One line of the fixture host's own command line, its node and bin.js read off its process, run in the fixture's
 * folder with the fixture's own environment and none of the caller's. */
function wsp(host, ...args) {
  const [node, bin] = readFileSync(`/proc/${host.pid}/cmdline`, "utf8").split("\0");
  if (!bin?.endsWith("bin.js")) throw new Error(`the fixture's host (pid ${host.pid}) runs no bin.js`);
  const home = dirname(dirname(host.state));
  const env = { HOME: home, PATH: [join(home, "bin"), dirname(node), "/usr/local/bin", "/usr/bin", "/bin"].join(":"), LANG: "en_US.UTF-8", WSP_UPDATE_CHECK: "0" };
  const said = spawnSync(node, [bin, ...args, "--state", host.state], { cwd: home, env, encoding: "utf8", timeout: 120_000 });
  if (said.status !== 0) throw new Error(`wsp ${args.join(" ")}: ${(said.stderr || said.stdout).trim()}`);
  return said.stdout;
}

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const threadRow = (host, id) =>
  wsp(host, "threads", "--json")
    .split("\n")
    .filter(l => l.trim() !== "")
    .flatMap(l => JSON.parse(l).threads ?? [JSON.parse(l)])
    .find(t => t.id === id || t.threadId === id);
async function waitThread(host, id, fits, what) {
  for (const end = Date.now() + 30_000; ; await sleep(250)) {
    const t = threadRow(host, id);
    if (t !== undefined && fits(t)) return;
    if (Date.now() > end) throw new Error(`thread ${id} was not ${what} in 30 s (it is ${t?.status ?? "not listed"})`);
  }
}

/** The threads a thread frame made in this process, by prompt, for the dropThread frame after the shot. */
const made = new Map();
/** What a usage frame kept of the ledger and the limits, put back by its restore. */
let usageKept;
/** The files a keepFile frame held, by their path under the fixture's folder, null for one that was not there. */
const filesKept = new Map();
const USAGE_ROWS = { collections: ["limits", "usage-accounts", "usage-index"], blobs: ["usage-days"] };

async function stateDb(host, work) {
  process.removeAllListeners("warning");
  const { DatabaseSync } = await import("node:sqlite");
  const db = new DatabaseSync(join(dirname(host.state), "state.db"));
  db.exec("pragma busy_timeout = 10000");
  try {
    return work(db);
  } finally {
    db.close();
  }
}

/** A frame's `{name}` words read as that project's id, off projects.list's answer by name. */
async function named(host, frame) {
  const text = JSON.stringify(frame);
  if (!/\{\w+\}/.test(text)) return frame;
  const [{ projects }] = await ops(host.url, host.token, [{ op: "projects.list" }]);
  return JSON.parse(text.replace(/\{(\w+)\}/g, (_, name) => projects.find(p => p.name === name)?.id ?? `missing-project-${name}`));
}

/** A state's before or after ops, with the names states.mjs uses read against the host: a device revoked by its name,
 * the fixture's recipe file, a device paired by its name, a project's `{name}` as its id, and these frames of the
 * shooters' own:
 * - `{ project: name }` a git folder in the fixture's folder added as a project; `{ dropProject: name }` removes it.
 * - `{ thread: prompt, reply, title?, project? }` a thread whose agent plays `reply` (parity/claude-stub.mjs's parts),
 *   waited for until it runs where the reply holds or else until it ends; `{ dropThread: prompt }` stops and deletes it.
 * - `{ usage: "keep" }` keeps the ledger and the plan limits as they stand; `{ usage: "restore" }` puts them back, so a
 *   state's turns leave no use or limit behind for the next.
 * - `{ keepFile: path }` keeps a file under the fixture's folder as it stands; `{ restoreFile: path }` writes it back,
 *   for what no host op takes back: a key in the vault (state/.env), a server wsp mcp install wrote.
 * - `{ wsp: [...args] }` one line of the fixture host's command line. */
export async function sendOps(host, frames) {
  for (const raw of frames ?? []) {
    const frame = await named(host, raw);
    if (frame.pair !== undefined) {
      await pair(host.url, host.token, frame.pair);
    } else if (frame.op === "devices.revoke") {
      const [{ devices }] = await ops(host.url, host.token, [{ op: "devices.list" }]);
      for (const d of devices.filter(d => d.name === frame.device)) await ops(host.url, host.token, [{ op: "devices.revoke", deviceId: d.id }]);
    } else if (frame.thread !== undefined) {
      const turns = join(dirname(dirname(host.state)), "parity-turns.json");
      writeFileSync(turns, `${JSON.stringify({ ...(existsSync(turns) ? JSON.parse(readFileSync(turns, "utf8")) : {}), [frame.thread]: frame.reply }, null, 2)}\n`);
      const held = [frame.reply].flat().some(p => ["hold", "question", "permission"].includes(p.kind ?? p));
      const said = wsp(host, "run", frame.project ?? "bench", "--agent", "claude", ...(frame.title ? ["--title", frame.title] : []), ...(held ? ["--detach"] : []), frame.thread);
      const id = /\bthread ([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b/.exec(said)?.[1];
      if (id === undefined) throw new Error(`wsp run printed no thread: ${said.trim()}`);
      made.set(frame.thread, id);
      await waitThread(host, id, t => (held ? t.status === "running" : t.status !== "running"), held ? "running" : "finished");
    } else if (frame.project !== undefined) {
      const folder = join(dirname(dirname(host.state)), "projects", frame.project.replace(/[^A-Za-z0-9._-]+/g, "-").slice(0, 40));
      mkdirSync(folder, { recursive: true });
      const git = (...args) => spawnSync("git", ["-c", "user.name=bench", "-c", "user.email=bench@example.com", "-c", "commit.gpgsign=false", ...args], { cwd: folder, env: { PATH: "/usr/bin:/bin" }, stdio: "ignore" });
      if (!existsSync(join(folder, ".git"))) git("init", "-q", "-b", "main"), git("commit", "-q", "--allow-empty", "-m", "bench");
      await ops(host.url, host.token, [{ op: "projects.add", source: folder, name: frame.project }]);
    } else if (frame.dropProject !== undefined) {
      const [{ projects }, { workspaces }] = await ops(host.url, host.token, [{ op: "projects.list" }, { op: "workspaces.list" }]);
      for (const p of projects.filter(p => p.name === frame.dropProject)) {
        for (const w of workspaces.filter(w => w.project?.id === p.id)) await ops(host.url, host.token, [{ op: "workspaces.delete", workspaceId: w.id }]).catch(() => {});
        await ops(host.url, host.token, [{ op: "projects.remove", projectId: p.id, force: true }]);
      }
    } else if (frame.dropThread !== undefined) {
      const id = made.get(frame.dropThread);
      if (id === undefined) continue;
      made.delete(frame.dropThread);
      if (threadRow(host, id)?.status === "running") {
        wsp(host, "stop", id);
        await waitThread(host, id, t => t.status !== "running", "stopped");
      }
      await ops(host.url, host.token, [{ op: "sessions.delete", threadId: id }]);
    } else if (frame.usage === "keep") {
      usageKept = await stateDb(host, db => ({
        collections: USAGE_ROWS.collections.flatMap(c => db.prepare("select collection, id, json from collections where collection = ?").all(c)),
        blobs: USAGE_ROWS.blobs.flatMap(c => db.prepare("select collection, id, bytes, at from blobs where collection = ?").all(c)),
      }));
    } else if (frame.usage === "restore") {
      if (usageKept === undefined) continue;
      const kept = usageKept;
      usageKept = undefined;
      await stateDb(host, db => {
        db.exec("begin");
        for (const c of USAGE_ROWS.collections) db.prepare("delete from collections where collection = ?").run(c);
        for (const c of USAGE_ROWS.blobs) db.prepare("delete from blobs where collection = ?").run(c);
        for (const r of kept.collections) db.prepare("insert into collections (collection, id, json) values (?, ?, ?)").run(r.collection, r.id, r.json);
        for (const r of kept.blobs) db.prepare("insert into blobs (collection, id, bytes, at) values (?, ?, ?, ?)").run(r.collection, r.id, r.bytes, r.at);
        db.exec("commit");
      });
    } else if (frame.keepFile !== undefined) {
      const file = join(dirname(dirname(host.state)), frame.keepFile);
      filesKept.set(frame.keepFile, existsSync(file) ? readFileSync(file) : null);
    } else if (frame.restoreFile !== undefined) {
      const file = join(dirname(dirname(host.state)), frame.restoreFile);
      const kept = filesKept.get(frame.restoreFile);
      filesKept.delete(frame.restoreFile);
      if (kept === null) rmSync(file, { force: true });
      else if (kept !== undefined) writeFileSync(file, kept, { mode: 0o600 });
    } else if (frame.wsp !== undefined) {
      wsp(host, ...frame.wsp);
    } else {
      await ops(host.url, host.token, [frame.file === "fixture" ? { ...frame, file: RECIPE } : frame]);
    }
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [fixturePath, op, params] = process.argv.slice(2);
  if (op === undefined) {
    console.error("usage: node host.mjs <out.json> <op> [json params]");
    process.exit(2);
  }
  const host = hostOf(fixturePath);
  const [answer] = await ops(host.url, host.token, [{ op, ...(params === undefined ? {} : JSON.parse(params)) }]);
  console.log(JSON.stringify(answer, null, 2));
}
