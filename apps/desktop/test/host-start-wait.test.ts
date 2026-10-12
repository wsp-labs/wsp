// SPDX-License-Identifier: AGPL-3.0-only
// The launch's wait on the service it started: as long as the host's process lives and moves, and one line with the
// log behind a button once it has gone.
import { spawn, type ChildProcess } from "node:child_process";
import { appendFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { connect, createServer, type Server, type Socket } from "node:net";
import { createServer as createHttpServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { SERVICE_MANAGERS, dialHost, httpProbe, startHost, type CliIO, type HostHandle, type ServiceDeps, type ServiceRunner } from "@wsp/host";
import { createRuntime, memoryStore } from "@wsp/runtime";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SERVICE_WAIT_MS } from "../../../packages/host/src/host-lock.js";
import { stubBackend } from "../../../packages/host/test/stub-backend.js";
import { StartFailed, openHostReady, type OpenHostOptions } from "../src/host-lifecycle.js";
import { OPEN_LOG, QUIT, sayStartFailed, startFailedDialog, startingAfter } from "../src/starting.js";

const PAGE = `<html><body><script>window.__WSP__ = window.__WSP__ || { token: "" };</script></body></html>`;
const noPrompt = (q: string): Promise<string> => Promise.reject(new Error(`unexpected prompt: ${q}`));
const noAsk = (prompt: { message: string }): Promise<boolean> => Promise.reject(new Error(`unexpected ask: ${prompt.message}`));

let home: string;
let statePath: string;
let logPath: string;
let lines: string[];
const children: ChildProcess[] = [];
let host: HostHandle | undefined;
const servers: Server[] = [];
const sockets = new Set<Socket>();
const timers: ReturnType<typeof setTimeout>[] = [];

beforeEach(() => {
  home = mkdtempSync(join(tmpdir(), "wsp-start-wait-"));
  statePath = join(home, "state.json");
  logPath = join(home, "host.log");
  lines = [];
  vi.stubEnv("HOME", home);
  vi.stubEnv("WSP_HOME", home);
});
afterEach(async () => {
  for (const child of children.splice(0)) child.kill("SIGKILL");
  for (const timer of timers.splice(0)) clearTimeout(timer);
  for (const socket of sockets) socket.destroy();
  sockets.clear();
  await Promise.all(servers.splice(0).map(server => new Promise(r => server.close(r))));
  await host?.close();
  host = undefined;
  vi.unstubAllEnvs();
  rmSync(home, { recursive: true, force: true });
});

const io = (): CliIO => ({ log: l => lines.push(l), error: l => lines.push(l), ask: noPrompt, askSecret: noPrompt });

/** A process standing in for the host the manager runs: alive until killed, or for as long as its script runs. The
 * one alive until killed is the sleep itself, since killing a shell leaves the sleep it started running. */
function hostProcess(script?: string): ChildProcess {
  const child = script === undefined ? spawn("sleep", ["120"], { stdio: "ignore" }) : spawn("sh", ["-c", script], { stdio: "ignore" });
  children.push(child);
  return child;
}

async function freePort(): Promise<number> {
  const probe = createServer();
  await new Promise<void>(r => probe.listen(0, "127.0.0.1", r));
  const { port } = probe.address() as { port: number };
  await new Promise(r => probe.close(r));
  return port;
}

/** Serves a real host on the port and writes its lock and token as `wsp up` does, under the given pid, naming the
 * port the lock says it answers on. */
async function serveAs(pid: number, port: number, lockPort = port): Promise<void> {
  const web = join(home, "web");
  mkdirSync(join(web, "assets"), { recursive: true });
  writeFileSync(join(web, "index.html"), PAGE);
  host = await startHost({ runtime: createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: {} }), webDir: web, port });
  writeFileSync(join(home, "host-token"), `${host.authToken}\n`);
  writeFileSync(join(home, "host.lock"), JSON.stringify({ pid, port: lockPort, startedAt: new Date().toISOString(), startedBy: "service" }));
}

/** Writes the lock a started host takes before it binds anything, naming a live pid and a port nothing answers on. */
function lockAs(pid: number, port: number): void {
  writeFileSync(join(home, "host.lock"), JSON.stringify({ pid, port, startedAt: new Date().toISOString(), startedBy: "service" }));
}

/** Binds the port as a host whose event loop is busy: every connection after the first `pass` is taken and held,
 * unanswered, until `at`, and from then on passes to the host on `to`. */
async function busyUntil(port: number, to: number, at: number, pass = 0): Promise<void> {
  let taken = 0;
  const server = createServer(socket => {
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
    socket.on("error", () => socket.destroy());
    taken += 1;
    timers.push(
      setTimeout(
        () => {
          if (socket.destroyed) return;
          const up = connect(to, "127.0.0.1");
          sockets.add(up);
          up.on("close", () => sockets.delete(up));
          up.on("error", () => socket.destroy());
          socket.pipe(up).pipe(socket);
        },
        taken <= pass ? 0 : Math.max(0, at - Date.now()),
      ),
    );
  });
  servers.push(server);
  await new Promise<void>(r => server.listen(port, "127.0.0.1", r));
}

/** launchd as the launch reads it, with nothing serving yet: `print` names the process the unit runs, which
 * `bootstrap` starts through `start`, and starts again once it has exited where the unit is kept alive. */
function launchd(start: () => ChildProcess | undefined, waitMs = SERVICE_WAIT_MS, keepAlive = false): { road: ServiceDeps } {
  let current: ChildProcess | undefined;
  let loaded = false;
  const run: ServiceRunner = async argv => {
    const verb = argv[1];
    if (verb === "print") {
      if (!loaded) return { code: 113, output: "Could not find service" };
      const exited = current !== undefined && (current.exitCode !== null || current.signalCode !== null);
      if (exited && keepAlive) current = start();
      const alive = current !== undefined && current.exitCode === null && current.signalCode === null;
      return { code: 0, output: alive ? `\tstate = running\n\tpid = ${current!.pid}\n` : "\tstate = not running\n" };
    }
    if (verb === "bootstrap") {
      loaded = true;
      current = start();
    }
    return { code: 0, output: "" };
  };
  return {
    road: { platform: "darwin", manager: SERVICE_MANAGERS.launchd, run, waitMs, answers: httpProbe, keys: { env: {}, cwd: tmpdir() }, dial: dialHost, stop: () => {}, here: async () => undefined },
  };
}

function options(service: ServiceDeps): OpenHostOptions {
  return { statePath, home, shim: join(home, "bin", "wsp"), io: io(), ask: noAsk, service };
}

describe("the launch's wait on a host that is starting", () => {
  it("opens on a host that takes 40 s to start while its process lives, showing that it is starting meanwhile", async () => {
    expect(SERVICE_WAIT_MS).toBe(20_000);
    // The host the manager runs reads its machines for 40 s before it takes its lock and serves, saying so in its
    // log as it goes, as a host whose boxes and provider answer slowly does.
    const port = await freePort();
    let ticker: ReturnType<typeof setInterval> | undefined;
    let serving: ReturnType<typeof setTimeout> | undefined;
    const service = launchd(() => {
      const child = hostProcess();
      ticker = setInterval(() => appendFileSync(logPath, `daemon on bx_${Date.now()}: version not read within 5 s; asking again at the next reach probe\n`), 5_000);
      serving = setTimeout(() => {
        clearInterval(ticker);
        void serveAs(child.pid!, port);
      }, 40_000);
      return child;
    });
    const shown: string[] = [];
    const starting = startingAfter(() => {
      shown.push("starting");
      return () => shown.push("gone");
    });
    try {
      const ready = await openHostReady(options(service.road));
      starting.end();
      expect(ready.session.url).toBe(`http://127.0.0.1:${port}`);
      expect(shown).toEqual(["starting", "gone"]);
    } finally {
      clearInterval(ticker);
      clearTimeout(serving);
      starting.end();
    }
  }, 60_000);

  it("waits on a host that holds the lock and answers after 40 s, binding late and busy at first, showing that it is starting meanwhile", async () => {
    expect(SERVICE_WAIT_MS).toBe(20_000);
    // A relaunch's new host has its lock from the start, binds its port at 37 s while it still sweeps its machines,
    // and answers its first request at 40 s, writing to its log every 5 s the whole time.
    const port = await freePort();
    const behind = await freePort();
    const child = hostProcess();
    lockAs(child.pid!, port);
    const ticker = setInterval(() => appendFileSync(logPath, `ended 2 harness runs on ws_${Date.now()} that no thread here holds\n`), 5_000);
    const t0 = Date.now();
    timers.push(setTimeout(() => void busyUntil(port, behind, t0 + 40_000).then(() => serveAs(child.pid!, behind, port)), 37_000));
    const shown: string[] = [];
    const starting = startingAfter(() => {
      shown.push("starting");
      return () => shown.push("gone");
    });
    try {
      const ready = await openHostReady(options(launchd(() => undefined).road));
      starting.end();
      expect(ready.session.url).toBe(`http://127.0.0.1:${port}`);
      expect(shown).toEqual(["starting", "gone"]);
    } finally {
      clearInterval(ticker);
      starting.end();
    }
  }, 60_000);

  it("waits on a host that holds the lock and is too busy to answer the launch's first read", async () => {
    // update.sh opens the app once the host answers once; the host then sweeps its machines and the app's own read
    // of its page goes unanswered for 3 s.
    const port = await freePort();
    const behind = await freePort();
    const child = hostProcess();
    await serveAs(child.pid!, behind, port);
    await busyUntil(port, behind, Date.now() + 3_000);
    const ready = await openHostReady(options(launchd(() => undefined).road));
    expect(ready.session.url).toBe(`http://127.0.0.1:${port}`);
  }, 20_000);

  it("waits on a host that answers its page and then stalls past the dial's window", async () => {
    // The page read gets through, then the host sweeps its machines for 7 s and the dial's 5 s window runs out.
    const port = await freePort();
    const behind = await freePort();
    const child = hostProcess();
    await serveAs(child.pid!, behind, port);
    await busyUntil(port, behind, Date.now() + 7_000, 1);
    const ready = await openHostReady(options(launchd(() => undefined).road));
    expect(ready.session.url).toBe(`http://127.0.0.1:${port}`);
  }, 20_000);

  it("gives up on a host whose process lives and has neither answered nor written a line for the longer wait", async () => {
    const service = launchd(() => hostProcess(), 300);
    const failed = await openHostReady(options(service.road)).catch((e: unknown) => e);
    expect(failed).toBeInstanceOf(StartFailed);
    // Six of the 300 ms waits with nothing moving.
    expect((failed as StartFailed).message).toMatch(/^wsp did not start within \d\.\ds: it neither answered nor wrote to its log$/);
  }, 10_000);

  it("a host that exits as it starts is one line naming what it said, with its log to open, and the log's lines in the app's log", async () => {
    // Under KeepAlive the manager starts it again after it exits, and that one exits as well.
    const script = `sleep 0.5; echo "listening on 127.0.0.1:4400" >> '${logPath}'; echo "wsp: port 4400 is held by another program" >> '${logPath}'; exit 1`;
    const service = launchd(() => hostProcess(script), 5_000, true);
    const t0 = Date.now();
    const failed = await openHostReady(options(service.road)).catch((e: unknown) => e);
    expect(failed).toBeInstanceOf(StartFailed);
    expect((failed as StartFailed).message).toBe("wsp stopped as it started: wsp: port 4400 is held by another program");
    expect((failed as StartFailed).logPath).toBe(logPath);
    expect(lines).toContain("host: wsp: port 4400 is held by another program");
    // Two of its processes were seen to exit, so the wait did not run out the whole start's wait for another.
    expect(Date.now() - t0).toBeLessThan(5_000);

    const { options: dialog, logPath: opens } = startFailedDialog("wsp could not start", failed, { host: logPath, app: join(home, "logs", "app.log") });
    expect(dialog.message).toBe("wsp could not start");
    expect(dialog.detail).toBe("wsp stopped as it started: wsp: port 4400 is held by another program");
    expect(dialog.detail).not.toContain("\n");
    expect(dialog.buttons).toEqual(["Open log", "Quit"]);
    expect(opens).toBe(logPath);
    const opened: string[] = [];
    const quits: string[] = [];
    const ui = (response: number) => ({ show: async () => ({ response }), open: async (path: string) => (opened.push(path), ""), quit: () => void quits.push("quit") });
    await sayStartFailed("wsp could not start", failed, { host: logPath, app: "" }, ui(OPEN_LOG));
    expect([opened, quits]).toEqual([[logPath], []]);
    await sayStartFailed("wsp could not start", failed, { host: logPath, app: "" }, ui(QUIT));
    expect([opened, quits]).toEqual([[logPath], ["quit"]]);
  }, 20_000);

  it("a host the manager never runs is one line saying so", async () => {
    const service = launchd(() => undefined, 400);
    const failed = await openHostReady(options(service.road)).catch((e: unknown) => e);
    expect((failed as StartFailed).message).toMatch(/^wsp did not start within \d+ms: nothing ran its service$/);
  });
});

describe("a lock the launch will not wait on", () => {
  /** The dialog the failure shows: its one line, and the log Open log opens. */
  const dialogOf = (failed: unknown) => {
    const { options: dialog, logPath: opens } = startFailedDialog("wsp could not start", failed, { host: logPath, app: join(home, "logs", "app.log") });
    expect(dialog.buttons).toEqual(["Open log", "Quit"]);
    return { line: dialog.detail, opens };
  };

  it("a live pid whose port answers as something other than wsp is one line at once", async () => {
    const other = createHttpServer((_req, res) => res.end("<html>hello</html>"));
    servers.push(other);
    const port = await freePort();
    await new Promise<void>(r => other.listen(port, "127.0.0.1", r));
    const child = hostProcess();
    lockAs(child.pid!, port);
    const failed = await openHostReady(options(launchd(() => undefined, 2_000).road)).catch((e: unknown) => e);
    expect(dialogOf(failed).line).toBe(`a host (pid ${child.pid}) holds ${join(home, "host.lock")} on port ${port} but no wsp host answers there: stop that process or run wsp down, then open wsp again`);
  });

  for (const status of [404, 500]) {
    it(`a live pid whose port answers ${status} is one line at once`, async () => {
      const other = createHttpServer((_req, res) => res.writeHead(status).end());
      servers.push(other);
      const port = await freePort();
      await new Promise<void>(r => other.listen(port, "127.0.0.1", r));
      const child = hostProcess();
      lockAs(child.pid!, port);
      const failed = await openHostReady(options(launchd(() => undefined, 2_000).road)).catch((e: unknown) => e);
      expect(dialogOf(failed).line).toBe(`a host (pid ${child.pid}) holds ${join(home, "host.lock")} on port ${port} but no wsp host answers there: stop that process or run wsp down, then open wsp again`);
    });
  }

  it("a live pid whose port answers the probe and never serves its page is one line once the longer wait runs out", async () => {
    // Headers come back at once and the body never does, so every page read runs out its 2 s and every probe answers.
    const other = createHttpServer((_req, res) => res.writeHead(200).write("<html>"));
    servers.push(other);
    const port = await freePort();
    await new Promise<void>(r => other.listen(port, "127.0.0.1", r));
    other.on("connection", socket => {
      sockets.add(socket);
      socket.on("close", () => sockets.delete(socket));
    });
    const child = hostProcess();
    lockAs(child.pid!, port);
    const failed = await openHostReady(options(launchd(() => undefined, 300).road)).catch((e: unknown) => e);
    expect(dialogOf(failed).line).toMatch(new RegExp(`^after \\d\\.\\ds of waiting, a host \\(pid ${child.pid}\\) holds .* on port ${port} but no wsp host answers there`));
  }, 10_000);

  it("a live pid that serves its page and never answers the dial is one line once the longer wait runs out", async () => {
    const port = await freePort();
    const behind = await freePort();
    const child = hostProcess();
    await serveAs(child.pid!, behind, port);
    await busyUntil(port, behind, Date.now() + 60_000, 1);
    const failed = await openHostReady(options(launchd(() => undefined, 300).road)).catch((e: unknown) => e);
    expect(dialogOf(failed).line).toMatch(new RegExp(`^after \\d\\.\\ds of waiting, the host at 127\\.0\\.0\\.1:${port} did not answer: nothing came back within 5000 ms$`));
  }, 10_000);

  it("a live pid that never answers and never writes to the host's log is one line once the longer wait runs out", async () => {
    const port = await freePort();
    const child = hostProcess();
    lockAs(child.pid!, port);
    writeFileSync(logPath, "wsp: an earlier host's last line\n");
    const failed = await openHostReady(options(launchd(() => undefined, 300).road)).catch((e: unknown) => e);
    const { line, opens } = dialogOf(failed);
    expect(line).toMatch(new RegExp(`^after \\d\\.\\ds of waiting, a host \\(pid ${child.pid}\\) holds .* on port ${port} but no wsp host answers there`));
    expect(opens).toBe(logPath);
  }, 10_000);

  it("a lock naming a pid that has exited is passed over for the service, and a service that runs nothing is one line", async () => {
    const gone = hostProcess("exit 0");
    await new Promise(r => gone.once("exit", r));
    lockAs(gone.pid!, await freePort());
    const failed = await openHostReady(options(launchd(() => undefined, 400).road)).catch((e: unknown) => e);
    expect(failed).toBeInstanceOf(StartFailed);
    expect(dialogOf(failed).line).toMatch(/^wsp did not start within \d+ms: nothing ran its service$/);
  });
});

describe("the dialog a failed start shows", () => {
  it("quits on Quit when it answers a later open, where no window is left whose closing would quit", async () => {
    const quits: string[] = [];
    const opened: string[] = [];
    await sayStartFailed("wsp could not open", new Error("wsp stopped as it started: a reason"), { host: logPath, app: join(home, "logs", "app.log") }, {
      show: async options => ({ response: options.cancelId! }),
      open: async path => (opened.push(path), ""),
      quit: () => void quits.push("quit"),
    });
    expect([quits, opened]).toEqual([["quit"], []]);
  });

  it("takes the first line of any other failure and opens the host's log where one was written, else the app's", () => {
    const app = join(home, "logs", "app.log");
    expect(startFailedDialog("wsp could not open", new Error("a refusal\nand what came after it"), { host: logPath, app })).toMatchObject({ options: { detail: "a refusal" }, logPath: app });
    writeFileSync(logPath, "wsp: something\n");
    expect(startFailedDialog("wsp could not open", "said as a string", { host: logPath, app }).logPath).toBe(logPath);
  });
});
