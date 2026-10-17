// SPDX-License-Identifier: AGPL-3.0-only
// The desktop app's own processes, as the app names them: its main process,
// the graphics and network helpers, the window's page and each browser tab.
// Only the app can say which pid is which, so it reads Electron and hands the
// host here a list of pids and what each one is; the figures beside them stay
// the daemon's, since Electron's working set is the resident set on a Mac and
// not the footprint the rows read. The app reads Electron only while a page
// watches the list, and the list names pids alone: it grants nothing and no
// signal is aimed by it.
import { z } from "zod";

/** What a process of the app is. A renderer is the window's own page; a tab is a browser tab's page; a frame is a
 * page another site put inside one, which Chromium runs apart from the page holding it; a helper is any other
 * process Electron keeps, its sandbox helpers and its other utility services. */
export const AppProcKind = z.enum(["main", "gpu", "network", "renderer", "tab", "frame", "helper"]);
export type AppProcKind = z.infer<typeof AppProcKind>;

/** The pages one renderer can hold: Chromium puts several tabs in one process once it runs out of room for more. */
export const APP_PROC_ORIGINS_MAX = 20;
/** The longest origin a row carries; a host name may run to 253 characters before its scheme and port. */
export const APP_PROC_ORIGIN_CHARS = 256;
/** More processes than the app ever runs, so a list past it is somebody else's. */
export const APP_PROCS_MAX = 200;

export const AppProc = z.object({
  pid: z.number().int().positive(),
  kind: AppProcKind,
  /** The origin of each page a tab's or a frame's renderer holds, in the order the app found them; never a path or a
   * query, which can carry a sign-in's code. */
  origins: z.array(z.string().max(APP_PROC_ORIGIN_CHARS)).max(APP_PROC_ORIGINS_MAX).optional(),
  /** The WebContents id of each browser tab's guest page a tab's or a frame's renderer holds, which the window's page
   * reads the tab, and so the thread whose pane holds it, off. */
  contents: z.array(z.number().int().nonnegative()).max(APP_PROC_ORIGINS_MAX).optional(),
});
export type AppProc = z.infer<typeof AppProc>;

export const AppProcList = z.array(AppProc).max(APP_PROCS_MAX);

/** What the host pushes to a socket that watches the app's processes, each time the app reads them again. */
export const AppProcsEvent = z.object({ type: z.literal("app.procs"), procs: AppProcList });
export type AppProcsEvent = z.infer<typeof AppProcsEvent>;

/** What the host pushes to the app's socket when the first page starts watching and when the last one stops. */
export const AppProcsWantedEvent = z.object({ type: z.literal("app.procs.wanted"), wanted: z.boolean() });
export type AppProcsWantedEvent = z.infer<typeof AppProcsWantedEvent>;

export const APP_PROC_WORDS: Record<AppProcKind, string> = {
  main: "App",
  gpu: "Graphics",
  network: "Network",
  renderer: "Window",
  tab: "Browser tab",
  frame: "Frame",
  helper: "Helper",
};

const hostOf = (origin: string): string => {
  try {
    return new URL(origin).host || origin;
  } catch {
    return origin;
  }
};

/** A process's name in a row: the app's parts under wsp, and a tab or a frame by the site it shows, with how many
 * more pages share its process. */
export function appProcLine(proc: AppProc): string {
  const word = APP_PROC_WORDS[proc.kind];
  if (proc.kind !== "tab" && proc.kind !== "frame") return `wsp ${word.toLowerCase()}`;
  const [first, ...more] = proc.origins ?? [];
  const site = first === undefined ? "" : ` ${hostOf(first)}`;
  return `${word.toLowerCase()}${site}${more.length > 0 ? ` +${more.length}` : ""}`;
}
