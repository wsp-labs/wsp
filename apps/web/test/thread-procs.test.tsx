// SPDX-License-Identifier: AGPL-3.0-only
// The Processes pane of a thread on this computer and on a box the person
// joined: the open thread's tree and its children folded under it in
// Activity's rows, never a sibling's from the same folder, End on a row
// ending its tree, the watch held only while the pane stands, and the road
// to Activity for the whole computer.
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { PlaceView, ProcEntry, SessionView, WorkspaceView } from "@wsp/protocol";
import { ProcessesSurface } from "../src/components/procs/ProcessesSurface.js";
import { TooltipProvider } from "../src/components/ui/tooltip.js";
import { provideDaemonWire } from "../src/files/wire.js";
import { getProcs, resetProcs } from "../src/machine/procs.js";
import { useStore } from "../src/protocol/store.js";
import { useSettingsStore } from "../src/settings/settingsStore.js";
import { fakeWire, resetSurfaces, type FakeWire } from "./surface-harness.js";

const MB = 1024 * 1024;
const HERE: PlaceView = { id: "here", kind: "computer", name: "lab-mbp", default: true, present: true } as PlaceView;
const BOX: PlaceView = { id: "box_hz", kind: "computer", name: "hz", default: false, present: true } as PlaceView;
const WS = "ws_lab";
const WS_KID = "ws_lab_kid";
const at = (id: string, over: Partial<WorkspaceView> = {}): WorkspaceView =>
  ({ id, name: "lab", kind: "local", machineId: "local", project: { id: "pr_lab", name: "lab", path: "/Users/dev/lab", computer: "here" }, phase: "running", golden: "", createdAt: "2026-10-11T00:00:00.000Z", ...over }) as WorkspaceView;
const session = (id: string, title: string, over: Partial<SessionView> = {}): SessionView =>
  ({ id: `s_${id}`, workspaceId: WS, threadId: id, harness: "claude", status: "running", prompt: title, startedBy: "person", startedAt: Date.parse("2026-10-11T00:00:00.000Z"), ...over }) as SessionView;
const proc = (pid: number, ppid: number, comm: string, extra: Partial<ProcEntry> = {}): ProcEntry => ({ pid, ppid, user: "dev", state: "S", comm, cmdline: comm, cpu: 0, rss: MB, startedAt: 1_757_000_000_000, ...extra });

// Two threads of one folder and a child of the first in a worktree of its own, all on this computer. Each of the two
// left a dev server whose shell has exited; the daemon stamped those by the group it saw them in.
const PROCS: ProcEntry[] = [
  proc(10, 1, "node", { cmdline: "node wsp host", rss: 200 * MB }),
  proc(20, 10, "wsp-daemon", { rss: 4 * MB }),
  proc(30, 10, "claude", { rss: 900 * MB, cpu: 12 }),
  proc(31, 30, "node", { cmdline: "node /opt/wsp/mcp.js", rss: 90 * MB }),
  proc(32, 1, "node", { cmdline: "node node_modules/.bin/vite --port 5174", owner: { by: "group", thread: "t_one" }, rss: 300 * MB, cpu: 2, id: "vite-identity" }),
  proc(33, 32, "esbuild", { cmdline: "esbuild --service=0.21.5 --ping", owner: { by: "root", thread: "t_one" }, rss: 20 * MB }),
  proc(40, 10, "claude", { rss: 700 * MB, cpu: 4 }),
  proc(41, 1, "node", { cmdline: "node node_modules/.bin/vite --port 5175", owner: { by: "group", thread: "t_two" }, rss: 280 * MB }),
  proc(50, 10, "claude", { rss: 600 * MB }),
  proc(51, 50, "python3", { cmdline: "python3 -m pytest", rss: 60 * MB }),
  proc(90, 1, "Instagram", { rss: 2048 * MB, cpu: 22 }),
];
const frame = (procs: ProcEntry[]) => ({ type: "proc.snapshot" as const, at: 1_000, daemon: 20, total: procs.length, procs, seq: 1 });

let wire: FakeWire;

const sessions = () => ({
  [WS]: [session("t_one", "Wire the page", { pid: 30 }), session("t_two", "Fast build", { pid: 40 }), session("t_idle", "Read the notes", { status: "completed" })],
  [WS_KID]: [session("t_kid", "Run the tests", { pid: 50, workspaceId: WS_KID, parentThreadId: "t_one" })],
});
const show = (threadId: string | null, fresh = false) => act(() => useStore.setState({ selectedId: WS, selectedThreadId: threadId, freshThread: fresh }));

beforeEach(() => {
  resetSurfaces();
  resetProcs();
  useSettingsStore.setState({ activityPlace: null });
  wire = fakeWire({ "proc.watch": {}, "proc.unwatch": {}, "proc.kill": {} });
  provideDaemonWire(WS, wire);
  getProcs(WS).feedStatus("live");
  useStore.setState({ places: [HERE], workspaces: [at(WS), at(WS_KID, { project: { id: "pr_lab", name: "lab", path: "/Users/dev/lab-kid", computer: "here" } } as Partial<WorkspaceView>)], sessions: sessions() });
});

afterEach(() => {
  cleanup();
  provideDaemonWire(WS, null);
});

const mount = () =>
  render(
    <TooltipProvider>
      <ProcessesSurface workspaceId={WS} />
    </TooltipProvider>,
  );
const feed = (procs = PROCS) => act(() => getProcs(WS).feed(frame(procs)));
const groups = () => [...document.querySelectorAll<HTMLElement>("[data-activity-group]")].map(g => g.dataset["activityGroup"]);
const rowsOf = (group: string) => [...document.querySelectorAll<HTMLElement>(`[data-activity-group="${group}"] [data-proc-row]`)].map(r => Number(r.dataset["procRow"]));
const allRows = () => [...document.querySelectorAll<HTMLElement>("[data-proc-row]")].map(r => Number(r.dataset["procRow"]));
const calls = (op: string) => wire.calls.filter(([o]) => o === op).map(([, p]) => p);

describe("a thread's Processes pane", () => {
  it("lists only the open thread's processes and its children's, never another thread's in the same folder", () => {
    show("t_one");
    mount();
    feed();
    expect(groups()).toEqual(["t_one", "t_kid"]);
    // Its agent with its tool server, and the dev server it left with esbuild under it, heaviest first.
    expect(rowsOf("t_one")).toEqual([30, 31, 32, 33]);
    // The child is a folded row until opened.
    expect(rowsOf("t_kid")).toEqual([]);
    fireEvent.click(document.querySelector('[data-group="t_kid"]')!);
    expect(rowsOf("t_kid")).toEqual([50, 51]);
    for (const other of [40, 41, 90, 10, 20]) expect(allRows()).not.toContain(other);

    // The sibling in the same folder sees its own and nothing of the first's.
    show("t_two");
    expect(groups()).toEqual(["t_two"]);
    expect(allRows()).toEqual([40, 41]);
  });

  it("draws each row as Activity does: its figures, its port and End, the agent with Stop", () => {
    show("t_one");
    mount();
    feed();
    const vite = document.querySelector<HTMLElement>("[data-proc-row='32']")!;
    expect(vite.dataset["kind"]).toBe("dev");
    expect([...vite.querySelectorAll("[data-col]")].map(c => c.textContent)).toEqual(["300 MB", "2.0%", ""]);
    expect(vite.querySelector("[data-act=end]")).not.toBeNull();
    const agent = document.querySelector<HTMLElement>("[data-proc-row='30']")!;
    expect(agent.querySelector("[data-act=end]")).toBeNull();
    expect(agent.querySelector("[data-act=stop]")).not.toBeNull();
  });

  it("End on a row ends that process and its tree by the identity it was read with", async () => {
    show("t_one");
    mount();
    feed();
    fireEvent.click(document.querySelector("[data-proc-row='32'] [data-act=end]")!);
    await act(async () => {});
    expect(calls("proc.kill")).toEqual([{ pid: 32, signal: "TERM", mode: "tree", startedAt: 1_757_000_000_000, identity: "vite-identity" }]);
  });

  it("End on a kept agent between turns frees it through the host, never by a signal", async () => {
    const ended: [number, number][] = [];
    act(() => useStore.setState({ api: { subscribe: () => () => {}, endActivity: async (pid: number, startedAt: number) => (ended.push([pid, startedAt]), { pid, ended: [], forced: [], left: [], freed: "t_idle" }) } as never }));
    show("t_idle");
    mount();
    feed([...PROCS, proc(60, 10, "claude", { kind: "agent", owner: { by: "root", thread: "t_idle" } })]);
    fireEvent.click(document.querySelector("[data-proc-row='60'] [data-act=free]")!);
    await act(async () => {});
    expect(ended).toEqual([[60, 1_757_000_000_000]]);
    expect(calls("proc.kill")).toEqual([]);
  });

  it("says nothing runs for a thread with no process, and on the New thread page shows no thread's", () => {
    show("t_idle");
    mount();
    feed();
    expect(screen.getByText("Nothing running for this thread")).toBeTruthy();
    expect(allRows()).toEqual([]);
    show(null, true);
    expect(screen.getByText("Nothing running for this thread")).toBeTruthy();
    expect(allRows()).toEqual([]);
  });

  it("watches this computer only while the pane stands", () => {
    show("t_one");
    const view = mount();
    expect(wire.calls.map(([op]) => op)).toEqual(["proc.watch"]);
    expect(calls("proc.watch")).toEqual([{}]);
    view.unmount();
    expect(wire.calls.map(([op]) => op)).toEqual(["proc.watch", "proc.unwatch"]);
  });

  it("on a box reads the threads' cgroups as one however many children a lead has, and draws only its own tree", () => {
    const kids = Array.from({ length: 16 }, (_, i) => session(`t_kid${i}`, "Run the tests", { workspaceId: WS_KID, parentThreadId: "t_one", status: "completed" }));
    act(() =>
      useStore.setState({
        places: [HERE, BOX],
        workspaces: [at(WS, { kind: "place", place: "box_hz" } as Partial<WorkspaceView>), at(WS_KID, { kind: "place", place: "box_hz" } as Partial<WorkspaceView>)],
        sessions: { [WS]: [session("t_one", "Wire the page"), session("t_two", "Fast build")], [WS_KID]: kids },
      }),
    );
    show("t_one");
    const view = mount();
    expect(calls("proc.watch")).toEqual([{ cgroups: ["/wsp-threads"] }]);
    feed([
      proc(300, 1, "bash", { cmdline: "bash run.sh", cgroup: "/wsp-threads/t_one" }),
      proc(301, 300, "claude", { cgroup: "/wsp-threads/t_one", rss: 500 * MB }),
      proc(400, 1, "node", { cmdline: "node server.js", cgroup: "/wsp-threads/t_two" }),
    ]);
    expect(groups()).toEqual(["t_one"]);
    expect(allRows()).not.toContain(400);
    view.unmount();
    expect(wire.calls.map(([op]) => op)).toEqual(["proc.watch", "proc.unwatch"]);
  });

  it("opens Activity on this computer for the whole of it", () => {
    show("t_one");
    mount();
    fireEvent.click(screen.getByText("Open in Activity"));
    expect(useSettingsStore.getState().activityPlace).toBe("here");
    expect(useStore.getState().settingsOpen).toBe(true);
  });
});
