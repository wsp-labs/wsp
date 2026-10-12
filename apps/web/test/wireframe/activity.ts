// SPDX-License-Identifier: AGPL-3.0-only
// The Activity screens' computer: five threads on a Mac with what each
// started, wsp's own parts, a terminal and an orphan, and the rest of the
// computer, three containers, fed to the page as the daemon's frames would come, one every two
// seconds of the frame's own clock. The first minutes go in at once, fast, so a
// shot's lines have a history to draw. ?energy=0 drops the energy counter, as a
// box has none; the hot screen holds a build past its marks.
import type { AppProc, ProcContainer, ProcEntry, SessionView, SysSample } from "@wsp/protocol";
import { syncPorts } from "../../src/machine/ports";
import { getLive } from "../../src/machine/live";
import { getProcs } from "../../src/machine/procs";
import { HERE_KEY } from "../../src/terminal/computer";

const MB = 1024 * 1024;
const GB = 1024 * MB;
const AT = Date.parse("2026-09-17T09:00:00.000Z");

const session = (id: string, workspaceId: string, prompt: string, pid: number | undefined, over: Partial<SessionView> = {}): SessionView =>
  ({ id: `s_${id}`, workspaceId, threadId: id, harness: "claude", status: pid === undefined ? "completed" : "running", prompt, startedBy: "person", startedAt: AT, ...(pid === undefined ? { endedAt: AT + 60_000 } : { pid }), ...over }) as SessionView;

/** Five threads on the Mac's two folders of spoo: four with a turn running, one resting with its agent kept. The
 * sampler is the design round's child, which its Processes pane folds under it. */
export const ACTIVITY_SESSIONS: Record<string, SessionView[]> = {
  ws_here: [session("th_design", "ws_here", "Task manager design round", 40009), session("th_sampler", "ws_here", "Daemon libproc sampler", 38810, { parentThreadId: "th_design", startedBy: "agent" } as Partial<SessionView>)],
  ws_copy: [
    session("th_landing", "ws_copy", "Landing hero copy", 36120, { asking: "Which headline do you want?" } as Partial<SessionView>),
    session("th_legend", "ws_copy", "Fix the usage page legend", 35002, { harness: "codex" }),
    session("th_browser", "ws_copy", "Browser tab phase 1", 31800, { harness: "opencode" }),
  ],
};

interface Row {
  pid: number;
  ppid: number;
  comm: string;
  cmdline?: string;
  rss: number;
  cpu: number;
  mw: number;
  pty?: string;
  /** What the daemon stamps once it knows: a slate run's and a browser tab's thread. */
  stamp?: Pick<ProcEntry, "owner" | "kind">;
}

const HOST = 46470;
const DAEMON = 50033;

function rows(hot: boolean): Row[] {
  return [
    { pid: 1, ppid: 0, comm: "launchd", rss: 12 * MB, cpu: 0.4, mw: 2 },
    { pid: HOST, ppid: 1, comm: "node", cmdline: "node /Applications/wsp.app/Contents/Resources/host.js --state ~/.wsp/state.db", rss: 208 * MB, cpu: 1.6, mw: 9 },
    { pid: DAEMON, ppid: HOST, comm: "wsp-daemon", cmdline: "wsp-daemon --local", rss: 3.6 * MB, cpu: 0.1, mw: 0.2 },
    { pid: 50100, ppid: DAEMON, comm: "zsh", cmdline: "-zsh", rss: 4 * MB, cpu: 0, mw: 0, pty: "pty_1" },
    { pid: 40009, ppid: HOST, comm: "claude", cmdline: "claude --output-format stream-json --input-format stream-json --verbose", rss: 1.54 * GB, cpu: 15.9, mw: 94 },
    { pid: 40112, ppid: 40009, comm: "node", cmdline: "node /opt/homebrew/lib/node_modules/wsp/mcp.js --thread th_design", rss: 92 * MB, cpu: 0.4, mw: 2 },
    { pid: 40131, ppid: 40009, comm: "node", cmdline: "node /Users/dev/.npm/_npx/playwright-mcp/cli.js", rss: 64 * MB, cpu: 0, mw: 0 },
    { pid: 41877, ppid: 40009, comm: "node", cmdline: "node apps/web/node_modules/.bin/vite --port 5173", rss: 312 * MB, cpu: 2.4, mw: 11 },
    { pid: 41902, ppid: 41877, comm: "esbuild", cmdline: "esbuild --service=0.25.0 --ping", rss: 38 * MB, cpu: 0.3, mw: 1 },
    { pid: 38810, ppid: HOST, comm: "claude", cmdline: "claude --output-format stream-json", rss: 1.21 * GB, cpu: 4.2, mw: 31 },
    { pid: 43311, ppid: 38810, comm: "cargo", cmdline: "cargo test -p wsp-daemon proc", rss: 41 * MB, cpu: 0.8, mw: 4 },
    hot
      ? { pid: 43390, ppid: 43311, comm: "rustc", cmdline: "rustc --crate-name wsp_daemon --edition=2021", rss: 9.4 * GB, cpu: 412, mw: 3420 }
      : { pid: 43390, ppid: 43311, comm: "rustc", cmdline: "rustc --crate-name wsp_daemon --edition=2021", rss: 1.12 * GB, cpu: 96.4, mw: 610 },
    { pid: 36120, ppid: HOST, comm: "claude", cmdline: "claude --output-format stream-json", rss: 980 * MB, cpu: 0.2, mw: 1 },
    { pid: 36400, ppid: 36120, comm: "node", cmdline: "node node_modules/.bin/next dev", rss: 640 * MB, cpu: 1.1, mw: 6 },
    { pid: 36433, ppid: 36400, comm: "next-server", cmdline: "next-server (v16.1.0)", rss: 410 * MB, cpu: 0.7, mw: 4 },
    { pid: 35002, ppid: HOST, comm: "codex", cmdline: "codex app-server", rss: 410 * MB, cpu: 0, mw: 0 },
    { pid: 35510, ppid: 35002, comm: "node", cmdline: "node node_modules/.bin/vitest --watch", rss: 120 * MB, cpu: 0.4, mw: 2 },
    { pid: 35521, ppid: 35510, comm: "node", cmdline: "node vitest/dist/workers/forks.js", rss: 182 * MB, cpu: 0, mw: 0 },
    { pid: 31800, ppid: HOST, comm: "opencode", cmdline: "opencode serve", rss: 260 * MB, cpu: 0.3, mw: 1 },
    { pid: 31877, ppid: 31800, comm: "node", cmdline: "node node_modules/.bin/electron-vite dev", rss: 451 * MB, cpu: 0.9, mw: 5 },
    { pid: 42290, ppid: HOST, comm: "python3", cmdline: "python3 /Users/dev/.wsp/slates/th_design/sampler.py", rss: 18 * MB, cpu: 0.6, mw: 3, stamp: { owner: { by: "root", thread: "th_design" }, kind: "slate" } },
    { pid: 46301, ppid: 1, comm: "wsp", cmdline: "/Applications/wsp.app/Contents/MacOS/wsp", rss: 70 * MB, cpu: 1.2, mw: 9, },
    { pid: 50877, ppid: 46301, comm: "wsp Helper (Renderer)", cmdline: "wsp Helper (Renderer) --type=renderer", rss: 131 * MB, cpu: 10.4, mw: 75, },
    { pid: 47195, ppid: 46301, comm: "wsp Helper (GPU)", cmdline: "wsp Helper (GPU) --type=gpu-process", rss: 168 * MB, cpu: 3.1, mw: 22, },
    { pid: 51102, ppid: 46301, comm: "wsp Helper (Renderer)", cmdline: "wsp Helper (Renderer) --type=renderer --webview-tag", rss: 96 * MB, cpu: 2.0, mw: 14, stamp: { owner: { by: "fork", thread: "th_browser" } } },
    { pid: 30212, ppid: 1, comm: "node", cmdline: "node server.js", rss: 84 * MB, cpu: 0.1, mw: 0.5 },
    { pid: 812, ppid: 1, comm: "Instagram", cmdline: "/Applications/Instagram.app/Contents/MacOS/Instagram", rss: 2.31 * GB, cpu: 22, mw: 140 },
    { pid: 488, ppid: 1, comm: "WindowServer", cmdline: "/System/Library/PrivateFrameworks/SkyLight.framework/Resources/WindowServer", rss: 610 * MB, cpu: 6.8, mw: 40 },
  ];
}

/** A figure that breathes from frame to frame, the same way on every load so shots compare. */
const wave = (base: number, tick: number, pid: number, swing: number): number => Math.max(0, base * (1 + swing * Math.sin(tick / 4 + pid % 7)));

function frame(tick: number, hot: boolean, energy: boolean): ProcEntry[] {
  return rows(hot).map(r => ({
    pid: r.pid,
    ppid: r.ppid,
    user: r.pid === 1 || r.pid === 488 ? "root" : "dev",
    state: "S",
    comm: r.comm,
    cmdline: r.cmdline ?? r.comm,
    cpu: Number(wave(r.cpu, tick, r.pid, 0.45).toFixed(1)),
    rss: Math.round(wave(r.rss, tick, r.pid, 0.04)),
    startedAt: AT - 3_600_000,
    ...(r.pty === undefined ? {} : { pty: r.pty }),
    ...r.stamp,
    ...(energy ? { energyMw: Number(wave(r.mw, tick, r.pid, 0.45).toFixed(1)) } : {}),
  }));
}

/** Three containers: the landing thread's compose project, one the sampler thread's docker labelled, and one the
 * person started by hand in a compose folder no thread noted. */
function containers(tick: number): ProcContainer[] {
  const id = (c: string): string => c.repeat(64);
  return [
    { id: id("a"), name: "landing-db-1", image: "postgres:17", state: "running", startedAt: AT - 1_800_000, thread: "th_landing", placedBy: "compose", folder: "/srv/acme/landing", project: "landing", memory: Math.round(wave(163 * MB, tick, 11, 0.02)), cpu: Number(wave(0.6, tick, 11, 0.4).toFixed(1)) },
    { id: id("b"), name: "redis-probe", image: "redis:8-alpine", state: "running", startedAt: AT - 240_000, thread: "th_sampler", placedBy: "label", memory: Math.round(wave(9 * MB, tick, 12, 0.02)), cpu: Number(wave(0.2, tick, 12, 0.4).toFixed(1)) },
    { id: id("c"), name: "lab-web-1", image: "nginx:1.29-alpine", state: "running", startedAt: AT - 86_400_000, folder: "/srv/acme/lab", project: "lab", memory: 12 * MB, cpu: 0 },
  ];
}

const BACKFILL = 150;

/** The app's own processes as the desktop app names them to the host. */
export const ACTIVITY_APP_PROCS: AppProc[] = [
  { pid: 46301, kind: "main" },
  { pid: 50877, kind: "renderer" },
  { pid: 47195, kind: "gpu" },
  { pid: 51102, kind: "tab", origins: ["http://localhost:5174"] },
];

/** Feeds the page as the daemon would, over the link `key` names: the minutes before the shot fast, then a frame every
 * two seconds. */
export function feedActivity(hot: boolean, energy: boolean, key: string = HERE_KEY): void {
  let tick = 0;
  const send = (): void => {
    tick++;
    const at = AT + tick * 2000;
    getProcs(key).feed({ type: "proc.snapshot", at, daemon: DAEMON, total: 1104, procs: frame(tick, hot, energy), containers: containers(tick), seq: tick, cost: { cpuMs: 1.3 + (tick % 3) * 0.2, held: 412_000 } });
    getProcs(key).feedStatus("live");
    const sample: SysSample = { type: "sys.sample", cpu: 21, load1: 3.2, mem: { used: 24.1 * GB, total: 36 * GB }, disk: { used: 400 * GB, total: 1000 * GB }, at } as SysSample;
    getLive(HERE_KEY).feedSample(sample);
  };
  syncPorts(key, [
    { port: 5173, pid: 41877, process: "node", loopback: true },
    { port: 3000, pid: 36400, process: "node", loopback: false },
    { port: 5174, pid: 31877, process: "node", loopback: true },
    { port: 7420, pid: HOST, process: "node", loopback: true },
    { port: 8080, pid: 30212, process: "node", loopback: false },
  ]);
  // One frame a paint, so each lands and draws before the next, as frames two seconds apart do.
  const step = (): void => {
    send();
    if (tick < BACKFILL) requestAnimationFrame(() => setTimeout(step, 0));
    else setInterval(send, 2000);
  };
  step();
}
