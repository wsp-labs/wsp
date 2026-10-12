// SPDX-License-Identifier: AGPL-3.0-only
// Activity's End and End all against the daemon's answer: End all stops the turn or frees the kept agent first and
// only then ends what outlives the agent, its tool servers and input pump left to it; an End a daemon from before kill
// modes answered says its update line and offers KILL.
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fmtBytes, killTooOldOfLine, type ActivityEnded, type PlaceView, type ProcEntry, type SessionView, type WorkspaceView } from "@wsp/protocol";
import { provideDaemonWire } from "../src/files/wire.js";
import { getProcs, resetProcs } from "../src/machine/procs.js";
import { useNotices } from "../src/notices/store.js";
import { useStore } from "../src/protocol/store.js";
import { HERE_KEY } from "../src/terminal/computer.js";
import { mountSettings, resetSettings, settingsApi, settle } from "./settings-harness.js";
import { fakeWire, type FakeWire } from "./surface-harness.js";

const MB = 1024 * 1024;
const HERE: PlaceView = { id: "here", kind: "computer", name: "lab-mbp", default: true, present: true } as PlaceView;
const WS = "ws_lab";
const workspace: WorkspaceView = { id: WS, name: "lab", kind: "local", machineId: "local", project: { id: "pr_lab", name: "lab", path: "/Users/dev/lab", computer: "here" }, phase: "running", golden: "", createdAt: "2026-10-11T00:00:00.000Z" } as WorkspaceView;
const session = (id: string, pid: number, status: SessionView["status"]): SessionView =>
  ({ id: `s_${id}`, workspaceId: WS, threadId: id, harness: "claude", status, prompt: id, startedBy: "person", startedAt: Date.parse("2026-10-11T00:00:00.000Z"), pid }) as SessionView;
const proc = (pid: number, ppid: number, comm: string, extra: Partial<ProcEntry> = {}): ProcEntry => ({ pid, ppid, user: "dev", state: "S", comm, cmdline: comm, cpu: 0, rss: MB, startedAt: 0, ...extra });

/** A resting thread whose kept agent has a tool server under it and its turn's input pump under pid 1, and a dev server
 * with its child that outlive the agent; a working thread with a dev server of its own. */
const PROCS: ProcEntry[] = [
  proc(10, 1, "node", { cmdline: "node wsp host" }),
  proc(20, 10, "wsp-daemon"),
  proc(30, 10, "claude", { rss: 400 * MB, startedAt: 3_000 }),
  proc(31, 30, "node", { cmdline: "node /opt/wsp/mcp.js", rss: 90 * MB }),
  proc(35, 1, "tail", { cmdline: "tail -n +1 -f /Users/dev/.wsp/runs/3d518f162d41.in", owner: { by: "group", thread: "t_rest" } }),
  proc(50, 1, "node", { cmdline: "node node_modules/.bin/vite", rss: 300 * MB, owner: { by: "fork", thread: "t_rest" }, startedAt: 5_000, id: "5000" }),
  proc(51, 50, "esbuild", { cmdline: "esbuild --service", rss: 20 * MB, owner: { by: "fork", thread: "t_rest" }, startedAt: 5_100, id: "5100" }),
  proc(40, 10, "claude", { rss: 500 * MB, startedAt: 4_000 }),
  proc(60, 1, "node", { cmdline: "node node_modules/.bin/vitest --watch", rss: 200 * MB, owner: { by: "fork", thread: "t_work" }, startedAt: 6_000, id: "6000" }),
];
const frame = { type: "proc.snapshot" as const, at: 3_000, daemon: 20, total: PROCS.length, procs: PROCS, seq: 1 };

let wire: FakeWire;
let order: string[];

beforeEach(() => {
  resetSettings();
  resetProcs();
  order = [];
  wire = fakeWire({ "proc.watch": {}, "proc.unwatch": {}, "proc.kill": (p: Record<string, unknown>) => (order.push(`kill ${String(p["pid"])}`), { signalled: [p["pid"]], forced: [], left: [] }) });
  provideDaemonWire(HERE_KEY, wire);
  getProcs(HERE_KEY).feedStatus("live");
  useStore.setState({ places: [HERE], workspaces: [workspace], sessions: { [WS]: [session("t_rest", 30, "completed"), session("t_work", 40, "running")] } });
  useNotices.getState().clear();
});

afterEach(() => {
  cleanup();
  provideDaemonWire(HERE_KEY, null);
  vi.useRealTimers();
});

const open = async (over: Parameters<typeof settingsApi>[0] = {}): Promise<void> => {
  mountSettings({ api: settingsApi({ watchAppProcs: async () => [], unwatchAppProcs: async () => {}, onAppProcs: () => () => {}, ...over }).api, at: { kind: "group", group: "activity" } });
  await settle();
  act(() => getProcs(HERE_KEY).feed(frame));
};
const toggle = (id: string) => fireEvent.click(document.querySelector(`[data-group="${id}"]`)!);
const kills = () => wire.calls.filter(([op]) => op === "proc.kill").map(([, p]) => p as Record<string, unknown>);

describe("End all", () => {
  it("frees a resting thread's kept agent first, then ends each tree that outlives it in one kill by its identity, never the agent's tool server or input pump", async () => {
    const endActivity = vi.fn(async (pid: number): Promise<ActivityEnded> => {
      order.push(`free ${pid}`);
      return { pid, ended: [], forced: [], left: [], freed: "t_rest" };
    });
    await open({ endActivity });
    fireEvent.click(document.querySelector("[data-group=t_rest] [data-act=end-all]")!);
    await waitFor(() => expect(document.querySelector("[data-k=activity-confirm]")).not.toBeNull());
    expect(screen.getByText(`2 processes holding ${fmtBytes(320 * MB)} end, after the agent it keeps for its next turn.`)).toBeTruthy();
    fireEvent.click(document.querySelector("[data-k=activity-confirm]")!);
    await settle();
    expect(endActivity).toHaveBeenCalledWith(30, 3_000);
    expect(order).toEqual(["free 30", "kill 50"]);
    expect(kills()).toEqual([{ pid: 50, signal: "TERM", mode: "tree", startedAt: 5_000, identity: "5000" }]);
  });

  it("waits for a working thread's stop before it signals anything", async () => {
    let stopped: () => void = () => {};
    const interruptSession = vi.fn(() => new Promise<{ outcome: "accepted" }>(resolve => (stopped = () => (order.push("stopped"), resolve({ outcome: "accepted" })))));
    await open({ interruptSession });
    fireEvent.click(document.querySelector("[data-group=t_work] [data-act=end-all]")!);
    await waitFor(() => expect(document.querySelector("[data-k=activity-confirm]")).not.toBeNull());
    fireEvent.click(document.querySelector("[data-k=activity-confirm]")!);
    await settle();
    expect(interruptSession).toHaveBeenCalledWith("s_t_work");
    expect(kills()).toEqual([]);
    await act(async () => stopped());
    await settle();
    expect(order).toEqual(["stopped", "kill 60"]);
  });
});

describe("End on a daemon from before kill modes", () => {
  it("says the computer's update line and offers KILL once TERM has done nothing for five seconds", async () => {
    wire.replies["proc.kill"] = (p: Record<string, unknown>) => (order.push(`${String(p["signal"])} ${String(p["pid"])}`), {});
    await open();
    toggle("t_rest");
    vi.useFakeTimers();
    fireEvent.click(document.querySelector("[data-proc-row='50'] [data-act=end]")!);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
    });
    const said = useNotices.getState().notices[0]!;
    expect(said.text).toBe(killTooOldOfLine("lab-mbp", [{ pid: 50, command: "node node_modules/.bin/vite" }]));
    expect(said.action?.word).toBe("Force end");
    // Its spinner goes: nothing more will take the row away on its own.
    expect(document.querySelector("[data-proc-row='50']")!.textContent).not.toContain("Ending");
    await act(async () => said.action!.run());
    expect(order).toEqual(["TERM 50", "KILL 50"]);
    expect(kills().at(-1)).toEqual({ pid: 50, signal: "KILL", startedAt: 5_000, identity: "5000" });
  });

  it("End all says it once for the computer, naming each process TERM went to alone, and Force end sends KILL to each still listed", async () => {
    wire.replies["proc.kill"] = (p: Record<string, unknown>) => (order.push(`${String(p["signal"])} ${String(p["pid"])}`), {});
    const endActivity = vi.fn(async (pid: number): Promise<ActivityEnded> => ({ pid, ended: [], forced: [], left: [], freed: "t_rest" }));
    await open({ endActivity });
    const worker = proc(52, 1, "node", { cmdline: "node worker.js", owner: { by: "fork", thread: "t_rest" }, startedAt: 5_200, id: "5200" });
    act(() => getProcs(HERE_KEY).feed({ ...frame, seq: 2, at: 5_000, procs: [...PROCS, worker], total: PROCS.length + 1 }));
    vi.useFakeTimers();
    fireEvent.click(document.querySelector("[data-group=t_rest] [data-act=end-all]")!);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    fireEvent.click(document.querySelector("[data-k=activity-confirm]")!);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
    });
    const toasts = useNotices.getState().notices;
    expect(toasts).toHaveLength(1);
    expect(toasts[0]!.text).toBe(
      killTooOldOfLine("lab-mbp", [
        { pid: 50, command: "node node_modules/.bin/vite" },
        { pid: 52, command: "node worker.js" },
      ]),
    );
    await act(async () => toasts[0]!.action!.run());
    expect(order.filter(o => o.startsWith("KILL")).sort()).toEqual(["KILL 50", "KILL 52"]);
  });
});
