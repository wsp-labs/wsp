// SPDX-License-Identifier: AGPL-3.0-only
// A throwaway host holding one sub-thread still starting on Boat, for captures of both apps:
//   node starting-host.mjs <empty dir> <out.json> [--hold <seconds>] [--stood]
// With --stood the sub-thread's machine stood instead: readyAt set, the machine ready, and a finished turn whose first
// agent line lands STOOD_S seconds after the start began, which today's app reads as "Started on Boat in 49s".
// It seeds a state in that folder (a lead thread here and its sub-thread on a fork whose start has begun and neither
// stood nor failed, with a running row and no transcript), serves it with the stand-in provider reading as Boat, and
// polls the host's own socket until --hold runs out (default: until SIGTERM, SIGINT or SIGHUP), then stops the host.
// The fork's machine has no guest, so the row's run is never answered for and boot leaves it running; nothing in the
// host writes readyAt or failed on a start it did not make itself.
import { spawn, spawnSync } from "node:child_process";
import { closeSync, existsSync, mkdirSync, openSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { connectHost } from "./wave-2/gpui-shots.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const REPO = join(here, "..", "..", "..");
const HOST_BIN = join(REPO, "packages", "host", "dist", "bin.js");
const { CLOUD_ENV, DAEMON_VERSION, FAKE_AS_ENV, FAKE_RECORDS_ENV, STATE_SHAPE, startingLine } = await import(pathToFileURL(join(REPO, "packages", "protocol", "dist", "index.js")).href);

const USAGE = "usage: node starting-host.mjs <empty dir> <out.json> [--hold <seconds>] [--stood]";
const stood = process.argv.includes("--stood");
/** The seconds from the start's beginning to the stood sub-thread's first agent line, and to its machine standing. */
export const STOOD_S = 49;
const READY_S = 31;
const [dir, out] = process.argv.slice(2);
const holdAt = process.argv.indexOf("--hold");
const holdS = holdAt < 0 ? Infinity : Number(process.argv[holdAt + 1]);
if (dir === undefined || out === undefined || !isAbsolute(dir) || !isAbsolute(out) || !(holdS >= 0)) {
  console.error(USAGE);
  process.exit(2);
}
if (!existsSync(dir) || readdirSync(dir).length > 0) {
  console.error(`${dir} is not an empty folder`);
  process.exit(2);
}
for (const built of [HOST_BIN, join(REPO, "apps", "web", "dist", "index.html")]) {
  if (!existsSync(built)) {
    console.error(`${built} is not built: run pnpm install && pnpm build`);
    process.exit(1);
  }
}

const CLOUD = "box";
const ON = "boat";
const now = Date.now();
const uuid = n => `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;
const LEAD = { thread: uuid(1), turn: uuid(2), session: uuid(3) };
const SUB = { thread: uuid(11), turn: uuid(12), session: uuid(13) };
const SUB_PROMPT = "Move the cart total's rounding to the end of the sum and add a test for three lines at 0.335.";
const LEAD_PROMPT = "Split the cart fixes across sub-threads on Boat, one per bug.";
const MACHINE = "fk_starting_1";
const SHA = "1".repeat(40);
/** The sub-thread's folder on its cloud machine, which roots a clone at /root. */
const SUB_FOLDER = "/root/spoo";
const SIZE = { cpu: 4, memMb: 8192 };
const BEGAN = stood ? now - 600_000 : now - 20_000;
const REPLY = "On it: the rounding moves to the end of the sum.";

const stateDir = join(dir, "state");
const state = join(stateDir, "state.json");
const records = join(dir, "stand-in", "records.json");
const project = join(dir, "wsp-work", "spoo");
mkdirSync(stateDir);
mkdirSync(dirname(records));
mkdirSync(project, { recursive: true });
writeFileSync(join(dir, ".wsp-starting-fixture"), "");

const row = (workspaceId, ids, o) => ({ id: ids.session, workspaceId, turnId: ids.turn, threadId: ids.thread, harness: "claude", model: "opus", permissionMode: "default", ...o });
const seeded = {
  $shape: { shape: STATE_SHAPE, wsp: "fixture", daemon: DAEMON_VERSION, bin: "starting-host.mjs", at: new Date(now).toISOString() },
  projects: {
    pr_spoo: { id: "pr_spoo", name: "spoo", computer: "here", source: { kind: "folder", path: project }, path: project, remote: "git@github.com:you/spoo.git", defaultBranch: "main", memoryKey: project, memoryDir: project, createdAt: new Date(now - 3_600_000).toISOString() },
  },
  workspaces: {
    ws_lead: { id: "ws_lead", name: "lead", machineId: "local", phase: "running", kind: "local", golden: "", createdAt: new Date(now - 3_600_000).toISOString(), size: { cpu: 10, memMb: 32768 }, home: dir, folder: dir, spec: {}, project: "pr_spoo", agents: { spawn: true, maxMachines: 3, maxDepth: 1 } },
    ws_sub: {
      id: "ws_sub",
      name: "cart-rounding",
      kind: "cloud",
      machineId: MACHINE,
      phase: "running",
      golden: "fksnap_starting",
      createdAt: new Date(BEGAN).toISOString(),
      project: "pr_spoo",
      spec: {},
      home: "/root",
      size: SIZE,
      parentThreadId: LEAD.thread,
      rootThreadId: LEAD.thread,
      agents: { spawn: false },
      lead: { threadId: LEAD.thread, workspaceId: "ws_lead" },
      sub: {
        on: ON,
        repo: "you/spoo",
        base: { branch: "main", sha: SHA },
        branch: "wsp/cart-rounding",
        folder: SUB_FOLDER,
        rateUsdPerHour: 0.16,
        sizeName: "small",
        spawn: stood
          ? { startedAt: BEGAN, branch: { branch: "main", sha: SHA, ms: 400 }, machine: { since: BEGAN + 1_000, id: MACHINE, states: ["creating", "ready"], ms: (READY_S - 1) * 1000 }, readyAt: BEGAN + READY_S * 1000, log: [startingLine("cart-rounding", "Boat")] }
          : { startedAt: BEGAN, branch: { branch: "main", sha: SHA, ms: 400 }, machine: { since: BEGAN + 1_000, id: MACHINE, states: ["creating"] }, log: [startingLine("cart-rounding", "Boat")] },
      },
    },
  },
  sessions: {
    ws_lead: {
      workspaceId: "ws_lead",
      sessions: [row("ws_lead", LEAD, { status: "completed", startedBy: "person", prompt: LEAD_PROMPT, startedAt: now - 120_000, endedAt: now - 60_000, cwd: project })],
      threads: { [LEAD.thread]: { harness: "claude", readAt: now - 60_000 } },
    },
    ws_sub: {
      workspaceId: "ws_sub",
      // The run is one the machine never answers for, which is what keeps the row running across boot.
      sessions: [
        stood
          ? row("ws_sub", SUB, { status: "completed", startedBy: "agent", prompt: SUB_PROMPT, startedAt: BEGAN, endedAt: BEGAN + 60_000, cwd: SUB_FOLDER, parentThreadId: LEAD.thread, rootThreadId: LEAD.thread })
          : row("ws_sub", SUB, { status: "running", startedBy: "agent", prompt: SUB_PROMPT, startedAt: BEGAN, cwd: SUB_FOLDER, parentThreadId: LEAD.thread, rootThreadId: LEAD.thread, run: "run_starting" }),
      ],
      threads: { [SUB.thread]: { harness: "claude", parentThreadId: LEAD.thread, rootThreadId: LEAD.thread } },
    },
  },
  transcripts: {
    ws_lead: {
      workspaceId: "ws_lead",
      events: [
        { workspaceId: "ws_lead", sessionId: LEAD.session, threadId: LEAD.thread, turnId: LEAD.turn, type: "session.start", at: now - 120_000, prompt: LEAD_PROMPT, model: "opus", cwd: project },
        { workspaceId: "ws_lead", sessionId: LEAD.session, threadId: LEAD.thread, turnId: LEAD.turn, type: "session.delta", at: now - 61_000, kind: "text", text: "Started a sub-thread on Boat for the rounding bug." },
        { workspaceId: "ws_lead", sessionId: LEAD.session, threadId: LEAD.thread, turnId: LEAD.turn, type: "session.done", at: now - 60_000, result: { status: "completed", durationMs: 60_000, costUsd: 0.05, text: "Started a sub-thread on Boat for the rounding bug." } },
        { workspaceId: "ws_lead", sessionId: LEAD.session, threadId: LEAD.thread, turnId: LEAD.turn, type: "session.end", at: now - 60_000, exitCode: 0, sawResult: true },
      ],
    },
    ...(stood
      ? {
          ws_sub: {
            workspaceId: "ws_sub",
            events: [
              { workspaceId: "ws_sub", sessionId: SUB.session, threadId: SUB.thread, turnId: SUB.turn, type: "session.start", at: BEGAN, prompt: SUB_PROMPT, model: "opus", cwd: SUB_FOLDER },
              { workspaceId: "ws_sub", sessionId: SUB.session, threadId: SUB.thread, turnId: SUB.turn, type: "session.delta", at: BEGAN + STOOD_S * 1000, kind: "text", text: REPLY },
              { workspaceId: "ws_sub", sessionId: SUB.session, threadId: SUB.thread, turnId: SUB.turn, type: "session.done", at: BEGAN + 60_000, result: { status: "completed", durationMs: 60_000 - STOOD_S * 1000, costUsd: 0.02, text: REPLY } },
              { workspaceId: "ws_sub", sessionId: SUB.session, threadId: SUB.thread, turnId: SUB.turn, type: "session.end", at: BEGAN + 60_000, exitCode: 0, sawResult: true },
            ],
          },
        }
      : {}),
  },
};
writeFileSync(state, `${JSON.stringify(seeded, null, 2)}\n`);
writeFileSync(records, `${JSON.stringify({ machines: { [MACHINE]: { state: "running", shape: { ...SIZE, diskGb: 20, createdAt: seeded.workspaces.ws_sub.createdAt }, labels: {} } }, snapshots: [] }, null, 2)}\n`);

// Nothing of the caller's environment reaches the host: its own HOME, a system PATH, and the stand-in's words.
const env = {
  HOME: dir,
  PATH: `${dirname(process.execPath)}:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin`,
  LANG: "en_US.UTF-8",
  WSP_UPDATE_CHECK: "0",
  WSP_PROVIDER: "fake",
  [CLOUD_ENV]: "1",
  [FAKE_AS_ENV]: CLOUD,
  [FAKE_RECORDS_ENV]: records,
};
const log = openSync(join(dir, "up.log"), "a");
const host = spawn(process.execPath, [HOST_BIN, "up", "--port", "0", "--no-relay", "--state", state], { cwd: dir, env, stdio: ["ignore", log, log] });
closeSync(log);
const hostLog = () => readFileSync(join(dir, "up.log"), "utf8").slice(-4000);

/** True while the pid is still this process's own child, so a signal never lands on a reused pid. */
const ownChild = () => host.exitCode === null && host.signalCode === null && spawnSync("ps", ["-o", "ppid=", "-p", String(host.pid)], { encoding: "utf8", env: { PATH: "/usr/bin:/bin" } }).stdout.trim() === String(process.pid);
const stop = async code => {
  if (ownChild()) {
    host.kill("SIGTERM");
    const ended = new Promise(r => host.once("exit", r));
    if ((await Promise.race([ended.then(() => "ended"), new Promise(r => setTimeout(r, 20_000, "late"))])) === "late" && ownChild()) host.kill("SIGKILL");
  }
  const left = spawnSync("ps", ["-o", "pid=,args=", "-p", String(host.pid)], { encoding: "utf8", env: { PATH: "/usr/bin:/bin" } }).stdout.trim();
  console.log(left === "" ? `starting-host: host ${host.pid} stopped, nothing left at that pid` : `starting-host: pid ${host.pid} still shows: ${left}`);
  process.exit(code);
};
for (const sig of ["SIGTERM", "SIGINT", "SIGHUP"]) process.once(sig, () => void stop(0));

const lockedPort = () => {
  try {
    return JSON.parse(readFileSync(join(stateDir, "host.lock"), "utf8")).port || 0;
  } catch {
    return 0;
  }
};
const until = Date.now() + 30_000;
while (!existsSync(join(stateDir, "host-token")) || lockedPort() === 0) {
  if (host.exitCode !== null || Date.now() > until) {
    console.error(`starting-host: the host did not serve\n${hostLog()}`);
    await stop(1);
  }
  await new Promise(r => setTimeout(r, 100));
}
const port = lockedPort();
const tokenFile = join(stateDir, "host-token");
writeFileSync(out, `${JSON.stringify({ state, pid: host.pid, holder: process.pid, port, url: `http://127.0.0.1:${port}`, tokenFile, threads: { lead: LEAD.thread, starting: SUB.thread, sub: SUB.thread }, stood, workspaces: { lead: "ws_lead", starting: "ws_sub" } }, null, 2)}\n`);
console.log(`starting-host: host pid ${host.pid} serves ${state} at http://127.0.0.1:${port}; token in ${tokenFile}`);

/** What the host says of the starting sub-thread now: its row, its sub state and start, and how many events it holds. */
const reading = async () => {
  const socket = await connectHost(stateDir);
  const [listed, head, history, projects] = await Promise.all([
    socket.op("sessions.list", { workspaceId: "ws_sub" }),
    socket.op("sessions.head", { threadId: SUB.thread }),
    socket.op("sessions.history", { workspaceId: "ws_sub", threadId: SUB.thread, whole: true }),
    socket.op("projects.list"),
  ]).finally(socket.close);
  // The desktop app refuses the whole projects.list when a project lacks a field its view requires.
  const whole = projects.projects.every(p => typeof p.memoryKey === "string" && typeof p.memoryDir === "string");
  return { view: listed.sessions.find(r => r.threadId === SUB.thread), sub: head.facts.sub, events: history.events.length, whole };
};

const began = Date.now();
for (let round = 1; ; round++) {
  let r;
  try {
    r = await reading();
  } catch (e) {
    console.error(`starting-host: round ${round}: ${e instanceof Error ? e.message : String(e)}\n${hostLog()}`);
    await stop(1);
  }
  const spawnKeys = r.sub?.spawn === undefined ? "none" : Object.keys(r.sub.spawn).join(",");
  const ok = stood
    ? r.view?.status === "completed" && r.sub?.spawn?.readyAt === BEGAN + READY_S * 1000 && r.sub.spawn?.failed === undefined && r.events === 4 && r.whole
    : r.view?.status === "running" && r.sub?.state === "starting" && r.sub.spawn?.readyAt === undefined && r.sub.spawn?.failed === undefined && r.events === 0 && r.whole;
  console.log(
    `starting-host: round ${round} at ${Math.round((Date.now() - began) / 1000)} s: row ${r.view?.status ?? "absent"}, prompt ${JSON.stringify(r.view?.prompt?.slice(0, 40))}, startedAt ${r.view?.startedAt}, parent ${r.view?.parentThreadId === LEAD.thread ? "the lead" : r.view?.parentThreadId}, sub ${r.sub?.state ?? "absent"} on ${r.sub?.on}, spawn {${spawnKeys}}, events ${r.events}, projects ${r.whole ? "whole" : "missing fields"}: ${ok ? (stood ? "stood" : "starting") : stood ? "NOT stood" : "NOT starting"}`,
  );
  if (!ok) await stop(1);
  if (Date.now() - began >= holdS * 1000) await stop(0);
  await new Promise(res => setTimeout(res, Math.min(15_000, Math.max(0, holdS * 1000 - (Date.now() - began)))));
}
