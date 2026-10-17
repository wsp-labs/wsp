// SPDX-License-Identifier: AGPL-3.0-only
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { dialHost, startHost } from "@wsp/host";
import { AppProcList, appProcLine, type AppProc } from "@wsp/protocol";
import { createRuntime, memoryStore } from "@wsp/runtime";
import { afterEach, describe, expect, it, vi } from "vitest";
import { stubBackend } from "../../../packages/host/test/stub-backend.js";
import { appProcLabels, offerAppProcs, pageOf, pagesOf, type AppMetric, type ContentsOf, type OfferSocket } from "../src/app-procs.js";
import { hostFeed } from "../src/host-feed.js";

/** What Electron 43.5.1 answered on Linux for a window holding a cross-site iframe and two browser tabs on
 * different sites, each with a frame from a third site and one from its own; the frames as framesInSubtree gave
 * them, a guest's frames standing in the window's tree too. */
const METRICS: AppMetric[] = [
  { pid: 57592, type: "Browser" },
  { pid: 57621, type: "GPU", serviceName: "GPU" },
  { pid: 57625, type: "Utility", serviceName: "network.mojom.NetworkService" },
  { pid: 57642, type: "Tab" },
  { pid: 57655, type: "Tab" },
  { pid: 57663, type: "Tab" },
  { pid: 57673, type: "Tab" },
  { pid: 57698, type: "Tab" },
  { pid: 57700, type: "Utility", serviceName: "storage.mojom.StorageService" },
  { pid: 57701, type: "Tab" },
];
const tab = (pid: number, site: string, id = pid - 57000) => ({
  id,
  guest: true,
  frames: [
    { pid, url: `http://${site}:45025/tab?code=s3cret` },
    { pid: 57698, url: "http://c.test:45025/frame" },
    { pid, url: `http://${site}:45025/frame` },
  ],
});
const PAGES = [
  tab(57673, "b.test"),
  tab(57663, "a.test"),
  {
    id: 1,
    guest: false,
    frames: [
      { pid: 57642, url: "http://127.0.0.1:45025/" },
      { pid: 57655, url: "http://x.test:45025/frame" },
      { pid: 57663, url: "http://a.test:45025/tab" },
      { pid: 57673, url: "http://b.test:45025/tab" },
      { pid: 57698, url: "http://c.test:45025/frame" },
    ],
  },
];

describe("the app's processes", () => {
  it("are named off Electron's metrics and each page's frames, a tab before the window it stands in", () => {
    expect(appProcLabels(METRICS, PAGES)).toEqual([
      { pid: 57592, kind: "main" },
      { pid: 57621, kind: "gpu" },
      { pid: 57625, kind: "network" },
      { pid: 57642, kind: "renderer" },
      { pid: 57655, kind: "frame", origins: ["http://x.test:45025"] },
      { pid: 57663, kind: "tab", origins: ["http://a.test:45025"], contents: [663] },
      { pid: 57673, kind: "tab", origins: ["http://b.test:45025"], contents: [673] },
      { pid: 57698, kind: "frame", origins: ["http://c.test:45025"], contents: [673, 663] },
      { pid: 57700, kind: "helper" },
      { pid: 57701, kind: "helper" },
    ]);
  });

  it("name every tab one renderer holds, and never a page's path or query", () => {
    const shared = [tab(57663, "a.test", 7), tab(57663, "b.test", 8), tab(57663, "a.test", 9)];
    const [, , , , , named] = appProcLabels(METRICS, shared);
    expect(named).toEqual({ pid: 57663, kind: "tab", origins: ["http://a.test:45025", "http://b.test:45025"], contents: [7, 8, 9] });
    expect(appProcLine(named!)).toBe("browser tab a.test:45025 +1");
    expect(JSON.stringify(appProcLabels(METRICS, shared))).not.toContain("s3cret");
  });

  it("leave out an origin longer than the host takes, so the rest of the list still reaches it", () => {
    const long = `${"a".repeat(63)}.${"b".repeat(63)}.${"c".repeat(63)}.${"d".repeat(61)}`;
    expect(AppProcList.safeParse(appProcLabels(METRICS, [...PAGES, tab(57701, long)])).success).toBe(true);
  });

  it("read a page off its contents with its top frame first", () => {
    const top = { osProcessId: 1, url: "http://a.test/", framesInSubtree: [] as ContentsOf["mainFrame"][] };
    const inner = { osProcessId: 2, url: "http://c.test/", framesInSubtree: [] };
    top.framesInSubtree.push(inner, top);
    expect(pageOf({ id: 12, getType: () => "webview", mainFrame: top })).toEqual({
      id: 12,
      guest: true,
      frames: [
        { pid: 1, url: "http://a.test/" },
        { pid: 2, url: "http://c.test/" },
      ],
    });
  });

  it("leave out a page whose renderer is gone and read the rest", () => {
    const gone = {
      id: 4,
      getType: () => "webview",
      get mainFrame(): ContentsOf["mainFrame"] {
        throw new Error("Render frame was disposed before WebFrameMain could be accessed");
      },
    };
    const window = { id: 1, getType: () => "window", mainFrame: { osProcessId: 3, url: "http://127.0.0.1:1/", framesInSubtree: [] } };
    expect(pagesOf([gone, window])).toEqual([{ id: 1, guest: false, frames: [{ pid: 3, url: "http://127.0.0.1:1/" }] }]);
  });
});

/** A host socket that answers the offer and records each set, and pushes what the case sends it. */
function fakeSocket(wanted: boolean | Error) {
  const sets: unknown[] = [];
  let push: (frame: Record<string, unknown>) => void = () => {};
  let end: () => void = () => {};
  const socket: OfferSocket = {
    request: async (op, params) => {
      if (op === "appProcs.offer") {
        if (wanted instanceof Error) throw wanted;
        return { wanted };
      }
      sets.push(params?.["procs"]);
      return {};
    },
    onFrame: fn => {
      push = fn;
      return () => (push = () => {});
    },
    closed: new Promise<void>(resolve => (end = resolve)),
  };
  return { socket, sets, push: (frame: Record<string, unknown>) => push(frame), end: () => end() };
}

describe("the offer", () => {
  afterEach(() => void vi.useRealTimers());

  it("reads Electron only while a page watches: at once, then every sample, and never after the last one goes", async () => {
    vi.useFakeTimers();
    const fake = fakeSocket(false);
    let reads = 0;
    const procs: AppProc[] = [{ pid: 9, kind: "main" }];
    offerAppProcs(fake.socket, { read: () => (reads++, procs), everyMs: 2_000 });
    await vi.advanceTimersByTimeAsync(10_000);
    expect(reads).toBe(0);

    fake.push({ type: "app.procs.wanted", wanted: true });
    expect(reads).toBe(1);
    await vi.advanceTimersByTimeAsync(6_000);
    expect(reads).toBe(4);
    expect(fake.sets).toEqual([procs, procs, procs, procs]);

    fake.push({ type: "app.procs.wanted", wanted: false });
    await vi.advanceTimersByTimeAsync(10_000);
    expect(reads).toBe(4);
  });

  it("starts at once when a page already watches, and stops with the socket", async () => {
    vi.useFakeTimers();
    const fake = fakeSocket(true);
    let reads = 0;
    offerAppProcs(fake.socket, { read: () => (reads++, []), everyMs: 2_000 });
    await vi.advanceTimersByTimeAsync(0);
    expect(reads).toBe(1);
    fake.end();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(reads).toBe(1);
    // A push that lands after the socket went starts nothing.
    fake.push({ type: "app.procs.wanted", wanted: true });
    expect(reads).toBe(1);
  });

  it("leaves nothing running on a host that refuses the offer", async () => {
    vi.useFakeTimers();
    const fake = fakeSocket(new Error("unknown op appProcs.offer"));
    const said: string[] = [];
    let reads = 0;
    offerAppProcs(fake.socket, { read: () => (reads++, []), log: line => said.push(line) });
    await vi.advanceTimersByTimeAsync(10_000);
    expect(reads).toBe(0);
    expect(said).toEqual(["app processes: unknown op appProcs.offer"]);
  });
});

describe("the feed on a real host", () => {
  it("sends the app's processes to a page only while it watches", async () => {
    const home = mkdtempSync(join(tmpdir(), "wsp-app-procs-"));
    const statePath = join(home, "state.json");
    const web = join(home, "web");
    mkdirSync(web);
    writeFileSync(join(web, "index.html"), `<!doctype html><html><body><script>window.__WSP__ = window.__WSP__ || { token: "" };</script></body></html>`);
    const host = await startHost({ runtime: createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: {} }), webDir: web, port: 0 });
    try {
      writeFileSync(join(home, "host.lock"), JSON.stringify({ pid: process.pid, port: host.port, startedAt: new Date().toISOString() }));
      writeFileSync(join(home, "host-token"), `${host.authToken}\n`);
      const dial = () => dialHost(statePath, { aim: { kind: "here" }, home });
      let reads = 0;
      const procs: AppProc[] = [
        { pid: 601, kind: "main" },
        { pid: 602, kind: "tab", origins: ["https://example.com"] },
      ];
      const feed = hostFeed({ dial, changed: () => {}, settleMs: 0, retryMs: 50, appProcs: () => (reads++, procs) });
      await new Promise(r => setTimeout(r, 300));
      expect(reads).toBe(0);

      const page = await dial();
      const seen: unknown[] = [];
      page.onFrame(frame => void (frame["type"] === "app.procs" && seen.push(frame["procs"])));
      await page.request("appProcs.watch");
      await vi.waitFor(() => expect(seen).toEqual([procs]));
      expect(reads).toBe(1);

      page.close();
      await page.closed;
      await new Promise(r => setTimeout(r, 300));
      const after = reads;
      await new Promise(r => setTimeout(r, 2_500));
      expect(reads).toBe(after);
      feed.close();
    } finally {
      await host.close();
      rmSync(home, { recursive: true, force: true });
    }
  });
});
