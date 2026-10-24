// SPDX-License-Identifier: AGPL-3.0-only
// The Experimental switch: where it is kept, how the GPUI app is found and
// started on each platform, and what a hand-over does when that app opens and
// when it does not. Every GPUI app here is a stub script in a temp folder.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { writeStub } from "../../../packages/protocol/test/stub-script.js";
import { GPUI_PATH_FILE, SWITCH_FILE, experimentalSwitch, gpuiApp, gpuiLaunch, handOver, keepSwitch, openGpui, turnExperimental, type HandOverDeps } from "../src/experimental.js";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "wsp-experimental-"));
});
afterEach(() => rmSync(dir, { recursive: true, force: true }));

/** A GPUI app that notes each start and its arguments in a file beside it, then does what `body` says. */
function stub(body: string): { path: string; starts: () => number; args: () => string[] } {
  const path = join(dir, "desktop-gpui");
  const said = join(dir, "starts");
  writeStub(path, `#!/bin/sh\necho "started $*" >> '${said}'\n${body}\n`);
  const lines = (): string[] => (existsSync(said) ? readFileSync(said, "utf8").trim().split("\n") : []);
  return { path, starts: () => lines().length, args: () => lines().map(line => line.slice("started".length).trim()) };
}

const installed = (path: string): void => writeFileSync(join(dir, GPUI_PATH_FILE), `${path}\n`);
const launchOf = (path: string) => gpuiLaunch("linux", path, { PATH: "/usr/bin:/bin", HOME: dir }, join(dir, "state.json"));

function deps(): HandOverDeps & { quits: number; lines: string[]; errors: string[] } {
  const d = {
    dir,
    quits: 0,
    lines: [] as string[],
    errors: [] as string[],
    leaving: () => false,
    open: (path: string, link?: string) => openGpui(gpuiLaunch("linux", path, { PATH: "/usr/bin:/bin", HOME: dir }, join(dir, "state.json"), link), 300),
    quit: () => void (d.quits += 1),
    log: (line: string) => void d.lines.push(line),
    error: (line: string) => void d.errors.push(line),
  };
  return d;
}

describe("where the switch is kept and how the GPUI app is found", () => {
  it("finds the app the home's path file names, and nothing where the file is missing, relative or names nothing there", () => {
    expect(gpuiApp(dir)).toBeUndefined();
    writeFileSync(join(dir, GPUI_PATH_FILE), "desktop-gpui\n");
    expect(gpuiApp(dir)).toBeUndefined();
    installed(join(dir, "gone"));
    expect(gpuiApp(dir)).toBeUndefined();
    const app = stub("exit 0");
    installed(app.path);
    expect(gpuiApp(dir)).toBe(app.path);
  });

  it("reads nothing where no GPUI app is installed, and on only while the file stands", () => {
    keepSwitch(dir, true);
    expect(experimentalSwitch(dir)).toBeNull();
    installed(stub("exit 0").path);
    expect(experimentalSwitch(dir)).toBe(true);
    keepSwitch(dir, false);
    expect(existsSync(join(dir, SWITCH_FILE))).toBe(false);
    expect(experimentalSwitch(dir)).toBe(false);
  });
});

describe("how the GPUI app is started on each platform", () => {
  const env = { PATH: "/usr/bin:/bin", HOME: "/home/dev", DISPLAY: ":0", WSP_HOME: "/home/dev/.wsp-b", APPDIR: "/tmp/.mount_wsp", APPIMAGE: "/home/dev/wsp.AppImage", ELECTRON_RUN_AS_NODE: "1", GH_TOKEN: "x" };

  const state = "/home/dev/wsp/.wsp/state.json";

  it("on Linux runs the executable itself on this app's state, handed only the names a desktop app needs, a named home among them", () => {
    const launch = gpuiLaunch("linux", "/opt/wsp/desktop-gpui", env, state);
    expect(launch.command).toBe("/opt/wsp/desktop-gpui");
    expect(launch.args).toEqual(["--state", state]);
    expect(Object.keys(launch.env).sort()).toEqual(["DISPLAY", "HOME", "PATH", "WSP_HOME"]);
    expect(gpuiLaunch("linux", "/opt/wsp/desktop-gpui", env, state, "wsp://thread/t1").args).toEqual(["wsp://thread/t1", "--state", state]);
  });

  it("on a Mac opens the bundle the executable sits in by the system's opener, carrying a named home in --env and the state after --args", () => {
    const launch = gpuiLaunch("darwin", "/Applications/wsp Experimental.app/Contents/MacOS/desktop-gpui", env, state);
    expect(launch.command).toBe("/usr/bin/open");
    expect(launch.args).toEqual(["--env", "WSP_HOME=/home/dev/.wsp-b", "-a", "/Applications/wsp Experimental.app", "--args", "--state", state]);
    expect(gpuiLaunch("darwin", "/Applications/wsp Experimental.app", { HOME: "/Users/dev" }, state, "wsp://thread/t1").args).toEqual(["-a", "/Applications/wsp Experimental.app", "wsp://thread/t1", "--args", "--state", state]);
    expect(Object.keys(launch.env)).not.toContain("APPDIR");
  });

  it("on a Mac runs a bare executable, a cargo build's, itself on this app's state", () => {
    const launch = gpuiLaunch("darwin", "/Users/dev/wsp/target/release/desktop-gpui", env, state, "wsp://thread/t1");
    expect([launch.command, ...launch.args]).toEqual(["/Users/dev/wsp/target/release/desktop-gpui", "wsp://thread/t1", "--state", state]);
    expect(gpuiLaunch("darwin", "/Users/dev/wsp/target/release/desktop-gpui", env, state).args).toEqual(["--state", state]);
  });
});

describe("opening the GPUI app", () => {
  it("went where the app is still running after the grace, or ended cleanly before it", async () => {
    expect(await openGpui(launchOf(stub("exec sleep 2").path), 300)).toEqual({ ok: true });
    expect(await openGpui(launchOf(stub("exit 0").path), 5_000)).toEqual({ ok: true });
  });

  it("did not go where the app exits with a failure or cannot be started, and says which", async () => {
    expect(await openGpui(launchOf(stub("exit 1").path), 5_000)).toEqual({ ok: false, error: "The Experimental app did not open: it exited with code 1" });
    const missing = await openGpui(launchOf(join(dir, "nothing-here")), 5_000);
    expect(missing.ok).toBe(false);
    expect(!missing.ok && missing.error).toMatch(/^The Experimental app did not open: .*ENOENT/);
  });
});

describe("the switch turned from Settings", () => {
  it("on opens the GPUI app, then quits, keeping the switch on", async () => {
    const app = stub("exit 0");
    installed(app.path);
    const d = deps();
    expect(await turnExperimental(true, d)).toBe(true);
    expect(app.starts()).toBe(1);
    expect(d.quits).toBe(1);
  });

  it("on with an app that does not open leaves the switch off, quits nothing and refuses in the app's words", async () => {
    const app = stub("exit 1");
    installed(app.path);
    const d = deps();
    await expect(turnExperimental(true, d)).rejects.toThrow("The Experimental app did not open: it exited with code 1");
    expect(app.starts()).toBe(1);
    expect(d.quits).toBe(0);
    expect(existsSync(join(dir, SWITCH_FILE))).toBe(false);
  });

  it("off is the write alone", async () => {
    const app = stub("exit 0");
    installed(app.path);
    keepSwitch(dir, true);
    const d = deps();
    expect(await turnExperimental(false, d)).toBe(false);
    expect(app.starts()).toBe(0);
    expect(d.quits).toBe(0);
  });

  it("on where no GPUI app is installed writes nothing and opens nothing", async () => {
    const d = deps();
    expect(await turnExperimental(true, d)).toBeNull();
    expect(existsSync(join(dir, SWITCH_FILE))).toBe(false);
  });
});

describe("a way into this app while the switch is on", () => {
  it("is not handed over while the switch is off", async () => {
    const app = stub("exit 0");
    installed(app.path);
    const d = deps();
    expect(await handOver(d)("launch")).toBe(false);
    expect(app.starts()).toBe(0);
  });

  it("opens the GPUI app and quits; ways in during it share the one open, and after it none opens again", async () => {
    const app = stub("exit 0");
    installed(app.path);
    keepSwitch(dir, true);
    const d = deps();
    const gate = handOver(d);
    expect(await Promise.all([gate("launch"), gate("Dock")])).toEqual([true, true]);
    expect(await gate("link")).toBe(true);
    expect(app.starts()).toBe(1);
    expect(d.quits).toBe(1);
    expect(d.lines).toEqual([`launch: opened the Experimental app at ${app.path}; quitting`]);
  });

  it("hands the GPUI app the wsp:// link that came in", async () => {
    const app = stub("exit 0");
    installed(app.path);
    keepSwitch(dir, true);
    expect(await handOver(deps())("link", "wsp://thread/t1")).toBe(true);
    expect(app.args()).toEqual([`wsp://thread/t1 --state ${join(dir, "state.json")}`]);
  });

  it("goes on as with the switch off where the GPUI app did not open, saying why in the log", async () => {
    installed(stub("exit 1").path);
    keepSwitch(dir, true);
    const d = deps();
    expect(await handOver(d)("second launch")).toBe(false);
    expect(d.quits).toBe(0);
    expect(d.errors).toEqual(["second launch: The Experimental app did not open: it exited with code 1"]);
  });

  it("is not handed over while this app is quitting", async () => {
    const app = stub("exit 0");
    installed(app.path);
    keepSwitch(dir, true);
    expect(await handOver({ ...deps(), leaving: () => true })("Dock")).toBe(false);
    expect(app.starts()).toBe(0);
  });

  it("is not handed over where the switch is on and the app it named is gone", async () => {
    mkdirSync(join(dir, "old"));
    installed(join(dir, "old", "desktop-gpui"));
    keepSwitch(dir, true);
    expect(await handOver(deps())("launch")).toBe(false);
  });
});
