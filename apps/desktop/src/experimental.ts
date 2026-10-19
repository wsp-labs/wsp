// SPDX-License-Identifier: AGPL-3.0-only
// The Experimental switch, which hands this app over to the GPUI app. It is a
// shell setting, kept as a file in this app's own folder in the home, read
// before any window: no host field holds it, so no host of another release
// refuses it. The GPUI app names its own executable in the same folder each
// time it starts, and the switch is only offered where that names something
// that is there. Both apps serve the one host, so a hand-over opens the GPUI
// app and quits this one with nothing asked and nothing stopped. While the
// switch is on, every way into this app does the same; the GPUI app's way
// back deletes the file before it opens this app again.
import { spawn } from "node:child_process";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { isAbsolute, join } from "node:path";
import { HOME_ENV, type BundleOutcome } from "@wsp/protocol";
import { bundleOf } from "./self-update.js";

/** The switch, on while the file stands; the GPUI app deletes it on its way back. */
export const SWITCH_FILE = "experimental";
/** Where the GPUI app writes the path of its own executable each time it starts. */
export const GPUI_PATH_FILE = "desktop-gpui.path";

/** How long a started GPUI app is watched for a failed start before it counts as opened. */
export const OPEN_GRACE_MS = 1_500;

type Env = Readonly<Record<string, string | undefined>>;

/** The GPUI app this home names, or nothing where the file is missing or names nothing that is there. */
export function gpuiApp(dir: string): string | undefined {
  let named: string;
  try {
    named = readFileSync(join(dir, GPUI_PATH_FILE), "utf8").trim();
  } catch {
    return undefined;
  }
  return isAbsolute(named) && existsSync(named) ? named : undefined;
}

/** Whether the switch is on; null where no GPUI app is installed, so there is nothing to switch to. */
export function experimentalSwitch(dir: string): boolean | null {
  return gpuiApp(dir) === undefined ? null : existsSync(join(dir, SWITCH_FILE));
}

export function keepSwitch(dir: string, on: boolean): void {
  if (on) writeFileSync(join(dir, SWITCH_FILE), "", { mode: 0o600 });
  else rmSync(join(dir, SWITCH_FILE), { force: true });
}

/** What a desktop session's app needs from this launch's environment: whose login, which wsp home, and where to draw. */
const KEPT = ["HOME", "USER", "LOGNAME", "LANG", "LC_ALL", "PATH", "TMPDIR", "DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY", "XDG_RUNTIME_DIR", "XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP", "XDG_DATA_DIRS", "XDG_CONFIG_HOME", "DBUS_SESSION_BUS_ADDRESS", HOME_ENV];

export interface Launch {
  command: string;
  args: string[];
  env: Record<string, string>;
}

/** How the GPUI app at this path is started, with the wsp:// link that brought this app up: on a Mac its bundle by
 * the system's opener, as the update's swap opens one, which hands the link over as an open-url; a bare executable
 * or Linux's by itself, with the link as its argument. The opener hands an app none of its own environment, so a
 * named wsp home rides its --env. */
export function gpuiLaunch(platform: NodeJS.Platform, path: string, env: Env, link?: string): Launch {
  const kept = Object.fromEntries(KEPT.flatMap(name => (env[name] === undefined ? [] : [[name, env[name]!]])));
  const links = link === undefined ? [] : [link];
  const bundle = platform === "darwin" ? (path.endsWith(".app") ? path : bundleOf(path)) : undefined;
  if (bundle === undefined) return { command: path, args: links, env: kept };
  const home = env[HOME_ENV];
  return { command: "/usr/bin/open", args: [...(home === undefined ? [] : ["--env", `${HOME_ENV}=${home}`]), "-a", bundle, ...links], env: { PATH: "/usr/bin:/bin:/usr/sbin:/sbin", ...kept } };
}

export const EXPERIMENTAL_WORDS = {
  notOpened: (why: string): string => `The Experimental app did not open: ${why}`,
  exited: (code: number | null, signal: NodeJS.Signals | null): string => (code !== null ? `it exited with code ${code}` : `it was stopped by ${signal ?? "a signal"}`),
} as const;

/** Starts the GPUI app in its own session, so this app quitting takes nothing of it. It went once the opener ends
 * well, or once the app is still running after the grace; an early exit that is not clean is a start that failed. */
export function openGpui(launch: Launch, graceMs = OPEN_GRACE_MS): Promise<BundleOutcome> {
  return new Promise(resolve => {
    let settled = false;
    const done = (outcome: BundleOutcome): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      child.unref();
      resolve(outcome);
    };
    const child = spawn(launch.command, launch.args, { detached: true, stdio: "ignore", env: launch.env });
    const timer = setTimeout(() => done({ ok: true }), graceMs);
    child.once("error", e => done({ ok: false, error: EXPERIMENTAL_WORDS.notOpened(e.message) }));
    child.once("exit", (code, signal) => done(code === 0 ? { ok: true } : { ok: false, error: EXPERIMENTAL_WORDS.notOpened(EXPERIMENTAL_WORDS.exited(code, signal)) }));
  });
}

export interface HandOverDeps {
  /** The folder the switch and the GPUI app's path are kept in. */
  dir: string;
  /** Opens the GPUI app, handing it the wsp:// link this way in carried. */
  open(app: string, link?: string): Promise<BundleOutcome>;
  /** Whether this app is already on its way out, when a way in opens nothing: a stop may be taking the host down. */
  leaving(): boolean;
  quit(): void;
  log(line: string): void;
  error(line: string): void;
}

/** Hands this app over where the switch is on: the GPUI app opened, then this app quit. Answers whether it did; a
 * way in that is not handed over goes on as it would with the switch off. One hand-over at a time, which every way
 * in that arrives during it shares, and none after one went, while this app is on its way out. */
export function handOver(deps: HandOverDeps): (way: string, link?: string) => Promise<boolean> {
  let going: Promise<boolean> | undefined;
  let gone = false;
  const go = async (way: string, link: string | undefined): Promise<boolean> => {
    if (gone) return true;
    const app = gpuiApp(deps.dir);
    if (deps.leaving() || app === undefined || experimentalSwitch(deps.dir) !== true) return false;
    const opened = await deps.open(app, link);
    if (!opened.ok) {
      deps.error(`${way}: ${opened.error}`);
      return false;
    }
    gone = true;
    deps.log(`${way}: opened the Experimental app at ${app}; quitting`);
    deps.quit();
    return true;
  };
  return (way, link) => (going ??= go(way, link).finally(() => (going = undefined)));
}

/** The switch turned from Settings. On opens the GPUI app and quits only once it went; one that did not open turns the
 * switch back off and refuses in its own line, so the window stays and says it. Off is the write alone. */
export async function turnExperimental(on: boolean, deps: HandOverDeps): Promise<boolean | null> {
  const app = gpuiApp(deps.dir);
  if (!on || app === undefined) {
    keepSwitch(deps.dir, false);
    return experimentalSwitch(deps.dir);
  }
  keepSwitch(deps.dir, true);
  const opened = await deps.open(app);
  if (!opened.ok) {
    keepSwitch(deps.dir, false);
    deps.error(`switch: ${opened.error}`);
    throw new Error(opened.error);
  }
  deps.log(`switch: opened the Experimental app at ${app}; quitting`);
  deps.quit();
  return experimentalSwitch(deps.dir);
}
