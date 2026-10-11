// SPDX-License-Identifier: AGPL-3.0-only
// The app's own processes, named for the host on this computer. Electron's
// metrics say which pid is the main process, the graphics and the network
// helpers; the frames of each page say which renderer is the window's and
// which is a browser tab's. A tab's frames also stand in the window's frame
// tree, under the webview that holds them, so the tabs are read first. Read
// only while the host says a page watches, every time the daemon samples.
import { APP_PROC_ORIGIN_CHARS, APP_PROC_ORIGINS_MAX, DAEMON_SAMPLER_INTERVAL_MS, type AppProc, type AppProcKind } from "@wsp/protocol";

/** The parts of Electron's ProcessMetric this reads. */
export interface AppMetric {
  pid: number;
  type: string;
  serviceName?: string;
}

/** One frame of a page, as Electron's WebFrameMain names it. */
export interface PageFrame {
  pid: number;
  url: string;
}

/** One page the app holds, the window's own or a browser tab's guest, its top frame first. */
export interface AppPage {
  guest: boolean;
  frames: readonly PageFrame[];
}

/** The parts of Electron's WebFrameMain and WebContents a page is read off. */
interface FrameOf {
  osProcessId: number;
  url: string;
  framesInSubtree: FrameOf[];
}
export interface ContentsOf {
  getType(): string;
  mainFrame: FrameOf;
}

/** A page as its contents hold it now: its top frame, then every frame inside it, a webview's guest's included. */
export function pageOf(contents: ContentsOf): AppPage {
  const top = contents.mainFrame;
  const frames = [top, ...top.framesInSubtree.filter(f => f !== top)];
  return { guest: contents.getType() === "webview", frames: frames.map(f => ({ pid: f.osProcessId, url: f.url })) };
}

/** Every page that can be read now: a contents whose renderer is gone throws on its frames, and is left out alone. */
export function pagesOf(all: readonly ContentsOf[]): AppPage[] {
  const pages: AppPage[] = [];
  for (const contents of all) {
    try {
      pages.push(pageOf(contents));
    } catch {
      continue;
    }
  }
  return pages;
}

const NETWORK_SERVICE = "network.mojom.NetworkService";

/** A page's origin, or its scheme where it has none: the row names the site and never carries a path or a query. */
function originOf(url: string): string {
  try {
    const at = new URL(url);
    return at.origin !== "null" ? at.origin : at.protocol;
  } catch {
    return "";
  }
}

/** Each of the app's processes and what it is, in the order Electron lists them. A renderer no page names, such as
 * the one Chromium keeps spare for the next tab, is a helper. */
export function appProcLabels(metrics: readonly AppMetric[], pages: readonly AppPage[]): AppProc[] {
  const named = new Map<number, { kind: AppProcKind; origins: string[] }>();
  const name = (pid: number, kind: AppProcKind, url: string | undefined): void => {
    let held = named.get(pid);
    if (held === undefined) {
      held = { kind, origins: [] };
      named.set(pid, held);
    }
    if (held.kind !== kind || url === undefined) return;
    const origin = originOf(url);
    // The host refuses a whole list over one origin past the schema's length, so that one is left out.
    if (origin !== "" && origin.length <= APP_PROC_ORIGIN_CHARS && !held.origins.includes(origin) && held.origins.length < APP_PROC_ORIGINS_MAX) held.origins.push(origin);
  };
  for (const page of pages) if (page.guest && page.frames[0] !== undefined) name(page.frames[0].pid, "tab", page.frames[0].url);
  for (const page of pages) if (!page.guest && page.frames[0] !== undefined) name(page.frames[0].pid, "renderer", undefined);
  for (const page of pages) for (const frame of page.frames.slice(1)) name(frame.pid, "frame", frame.url);
  return metrics.map(({ pid, type, serviceName }): AppProc => {
    if (type === "Browser") return { pid, kind: "main" };
    if (type === "GPU") return { pid, kind: "gpu" };
    if (type === "Utility" && serviceName === NETWORK_SERVICE) return { pid, kind: "network" };
    const page = type === "Tab" ? named.get(pid) : undefined;
    if (page === undefined) return { pid, kind: "helper" };
    return page.origins.length > 0 ? { pid, kind: page.kind, origins: page.origins } : { pid, kind: page.kind };
  });
}

/** The parts of the host socket the offer rides. */
export interface OfferSocket {
  request(op: string, params?: Record<string, unknown>): Promise<Record<string, unknown>>;
  onFrame(fn: (frame: Record<string, unknown>) => void): () => void;
  readonly closed: Promise<unknown>;
}

export interface OfferDeps {
  read(): AppProc[];
  everyMs?: number;
  log?(line: string): void;
}

/** Offers the app's processes on one socket to the host: read and sent at once when a page starts watching and then
 * every sample, and not read at all while nobody watches. Ends with the socket, or at the call it returns. A host
 * that predates the offer refuses it, which leaves nothing running. */
export function offerAppProcs(socket: OfferSocket, deps: OfferDeps): () => void {
  const everyMs = deps.everyMs ?? DAEMON_SAMPLER_INTERVAL_MS;
  let timer: ReturnType<typeof setInterval> | undefined;
  let over = false;
  const say = (e: unknown): void => deps.log?.(`app processes: ${e instanceof Error ? e.message : String(e)}`);
  const send = (): void => {
    let procs: AppProc[];
    try {
      procs = deps.read();
    } catch (e) {
      say(e);
      return;
    }
    socket.request("appProcs.set", { procs }).catch(say);
  };
  const want = (on: boolean): void => {
    if (timer !== undefined) clearInterval(timer);
    timer = undefined;
    if (!on || over) return;
    send();
    timer = setInterval(send, everyMs);
  };
  const off = socket.onFrame(frame => {
    if (frame["type"] === "app.procs.wanted") want(frame["wanted"] === true);
  });
  const end = (): void => {
    over = true;
    want(false);
    off();
  };
  socket.request("appProcs.offer").then(reply => want(reply["wanted"] === true), say);
  void socket.closed.then(end);
  return end;
}
