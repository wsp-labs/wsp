// SPDX-License-Identifier: AGPL-3.0-only
// The shell's own ways in with the Experimental switch on, through main.ts as
// it runs: a launch, a Dock click, a second launch and a wsp:// link each open
// the GPUI app and quit, and the launch opens no window of its own. Electron
// is a stand-in that records what main asks of it; the GPUI app is a stub
// script that notes each start, and the home is a temp folder.
import { EventEmitter } from "node:events";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { writeStub } from "../../../packages/protocol/test/stub-script.js";

const shell = vi.hoisted(() => ({ ready: false, quits: 0, windows: 0, links: [] as string[] }));

vi.mock("electron", async () => {
  const { EventEmitter } = await import("node:events");
  const app = Object.assign(new EventEmitter(), {
    name: "wsp",
    isPackaged: false,
    setPath: () => {},
    getPath: (name: string) => join(tmpdir(), name),
    requestSingleInstanceLock: () => true,
    exit: () => {},
    getVersion: () => "0.3.7",
    whenReady: () => (shell.ready ? Promise.resolve() : new Promise<void>(() => {})),
    isReady: () => true,
    quit: () => void (shell.quits += 1),
    focus: () => {},
    setAsDefaultProtocolClient: () => true,
    isInApplicationsFolder: () => true,
  });
  class BrowserWindow {
    constructor() {
      shell.windows += 1;
    }
    static getAllWindows = () => [];
    static fromWebContents = () => null;
  }
  return {
    app,
    BrowserWindow,
    Menu: { setApplicationMenu: () => {}, buildFromTemplate: () => ({}) },
    Notification: { isSupported: () => false },
    Tray: class {},
    crashReporter: { start: () => {} },
    dialog: {},
    ipcMain: { handle: () => {}, on: () => {}, removeHandler: () => {} },
    nativeImage: {},
    nativeTheme: {},
    powerSaveBlocker: {},
    session: {},
    shell: { openPath: async () => "" },
    webContents: { getAllWebContents: () => [] },
  };
});

vi.mock("@wsp/host", async importOriginal => ({ ...(await importOriginal<typeof import("@wsp/host")>()), adoptLoginPath: async () => ({}), connectPatiently: () => {} }));
// A launch that went on to its window would start this computer's service: here it is refused before anything runs.
vi.mock("../src/host-lifecycle.js", async importOriginal => ({ ...(await importOriginal<typeof import("../src/host-lifecycle.js")>()), openHostReady: () => Promise.reject(new Error("no host in this test")), openHost: () => Promise.reject(new Error("no host in this test")) }));

let home: string;
let starts: string;

beforeEach(() => {
  vi.resetModules();
  Object.assign(shell, { ready: false, quits: 0, windows: 0 });
  home = mkdtempSync(join(tmpdir(), "wsp-experimental-main-"));
  starts = join(home, "starts");
  const gpui = join(home, "desktop-gpui");
  writeStub(gpui, `#!/bin/sh\necho "started $*" >> '${starts}'\n`);
  mkdirSync(join(home, "desktop"));
  writeFileSync(join(home, "desktop", "desktop-gpui.path"), `${gpui}\n`);
  writeFileSync(join(home, "desktop", "experimental"), "");
  vi.stubEnv("WSP_HOME", home);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  rmSync(home, { recursive: true, force: true });
});

const startLines = (): string[] => (existsSync(starts) ? readFileSync(starts, "utf8").trim().split("\n") : []);
const started = (): number => startLines().length;
const onState = (): string => `--state ${join(home, "state.json")}`;

async function main(): Promise<EventEmitter> {
  await import("../src/main.js");
  return (await import("electron")).app as unknown as EventEmitter;
}

describe("main's ways in with the Experimental switch on", { timeout: 30_000 }, () => {
  it("a launch writes the wsp command, then opens the GPUI app and quits with no window of its own", async () => {
    shell.ready = true;
    await main();
    await vi.waitFor(() => expect(shell.quits).toBe(1), { timeout: 15_000 });
    expect(startLines()).toEqual([`started ${onState()}`]);
    expect(shell.windows).toBe(0);
    expect(existsSync(join(home, "bin", "wsp"))).toBe(true);
  });

  it("a Dock click opens the GPUI app and quits", async () => {
    (await main()).emit("activate");
    await vi.waitFor(() => expect(shell.quits).toBe(1), { timeout: 15_000 });
    expect(startLines()).toEqual([`started ${onState()}`]);
  });

  it("an unpackaged launch in a checkout with no wsp home named hands the GPUI app the checkout's own state", async () => {
    const checkout = join(home, "checkout");
    mkdirSync(join(checkout, ".wsp"), { recursive: true });
    writeFileSync(join(checkout, "package.json"), JSON.stringify({ name: "wsp" }));
    renameSync(join(home, "desktop"), join(checkout, ".wsp", "desktop"));
    vi.stubEnv("WSP_HOME", undefined);
    vi.stubEnv("HOME", home);
    vi.spyOn(process, "cwd").mockReturnValue(checkout);
    (await main()).emit("open-url", { preventDefault: () => {} }, "wsp://thread/t4");
    await vi.waitFor(() => expect(shell.quits).toBe(1), { timeout: 15_000 });
    expect(startLines()).toEqual([`started wsp://thread/t4 --state ${join(checkout, ".wsp", "state.json")}`]);
  });

  it("a second launch opens the GPUI app and quits", async () => {
    (await main()).emit("second-instance", {}, ["wsp"]);
    await vi.waitFor(() => expect(shell.quits).toBe(1), { timeout: 15_000 });
    expect(startLines()).toEqual([`started ${onState()}`]);
  });

  it("a second launch carrying a wsp:// link hands the link to the GPUI app", async () => {
    (await main()).emit("second-instance", {}, ["wsp", "wsp://thread/t2"]);
    await vi.waitFor(() => expect(shell.quits).toBe(1), { timeout: 15_000 });
    expect(startLines()).toEqual([`started wsp://thread/t2 ${onState()}`]);
  });

  it("a wsp:// link opens the GPUI app on that link and quits", async () => {
    (await main()).emit("open-url", { preventDefault: () => {} }, "wsp://thread/t1");
    await vi.waitFor(() => expect(shell.quits).toBe(1), { timeout: 15_000 });
    expect(startLines()).toEqual([`started wsp://thread/t1 ${onState()}`]);
  });

  it("a launch by a wsp:// link hands the link to the GPUI app", async () => {
    shell.ready = true;
    process.argv.push("wsp://thread/t3");
    try {
      await main();
      await vi.waitFor(() => expect(shell.quits).toBe(1), { timeout: 15_000 });
    } finally {
      process.argv.pop();
    }
    expect(startLines()).toEqual([`started wsp://thread/t3 ${onState()}`]);
  });

  it("with the switch off, a second launch opens nothing and quits nothing", async () => {
    rmSync(join(home, "desktop", "experimental"));
    (await main()).emit("second-instance", {}, ["wsp"]);
    await new Promise(resolve => setTimeout(resolve, 200));
    expect(started()).toBe(0);
    expect(shell.quits).toBe(0);
  });
});
