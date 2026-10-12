// SPDX-License-Identifier: AGPL-3.0-only
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { PlaceView, ProcEntry, SessionView, WorkspaceView } from "@wsp/protocol";
import { provideDaemonWire } from "../src/files/wire.js";
import { wireAppProcs } from "../src/machine/appProcs.js";
import { syncPorts } from "../src/machine/ports.js";
import { getProcs, resetProcs } from "../src/machine/procs.js";
import { useContextMenuStore } from "../src/actions/contextMenu.js";
import { useBrowserTabs } from "../src/browser/tabs.js";
import { useSlateStore } from "../src/slate/store.js";
import { useStore } from "../src/protocol/store.js";
import { useSettingsStore } from "../src/settings/settingsStore.js";
import { HERE_KEY } from "../src/terminal/computer.js";
import { SidebarCorner } from "../src/sidebar/SidebarCorner.js";
import { TooltipProvider } from "../src/components/ui/tooltip.js";
import { mountSettings, pageAt, resetSettings, settingsApi, settle } from "./settings-harness.js";
import { fakeWire, type FakeWire } from "./surface-harness.js";

const MB = 1024 * 1024;
const GB = 1024 * MB;
const HERE: PlaceView = { id: "here", kind: "computer", name: "lab-mbp", default: true, present: true } as PlaceView;
const WS = "ws_lab";
const workspace: WorkspaceView = { id: WS, name: "lab", kind: "local", machineId: "local", project: { id: "pr_lab", name: "lab", path: "/Users/dev/lab", computer: "here" }, phase: "running", golden: "", createdAt: "2026-10-11T00:00:00.000Z" } as WorkspaceView;
const session = (id: string, title: string, pid: number, over: Partial<SessionView> = {}): SessionView =>
  ({ id: `s_${id}`, workspaceId: WS, threadId: id, harness: "claude", status: "running", prompt: title, startedBy: "person", startedAt: Date.parse("2026-10-11T00:00:00.000Z"), pid, ...over }) as SessionView;
const proc = (pid: number, ppid: number, comm: string, extra: Partial<ProcEntry> = {}): ProcEntry => ({ pid, ppid, user: "dev", state: "S", comm, cmdline: comm, cpu: 0, rss: MB, startedAt: 0, ...extra });

const PROCS = (tick = 0): ProcEntry[] => [
  proc(10, 1, "node", { cmdline: "node wsp host", rss: 200 * MB, cpu: 1 }),
  proc(20, 10, "wsp-daemon", { rss: 4 * MB }),
  proc(30, 10, "claude", { rss: 1500 * MB, cpu: 15 + tick, energyMw: 90 }),
  proc(31, 30, "node", { cmdline: "node /opt/wsp/mcp.js", rss: 90 * MB, energyMw: 2 }),
  proc(32, 30, "node", { cmdline: "node node_modules/.bin/vite --port 5173", rss: 300 * MB, cpu: 2, energyMw: 11 }),
  proc(33, 10, "python3", { cmdline: "python3 slate.py", owner: { by: "root", thread: "t_one" }, kind: "slate", rss: 18 * MB, energyMw: 3 }),
  proc(34, 1, "wsp Helper", { owner: { by: "fork", thread: "t_one" }, rss: 96 * MB, energyMw: 14 }),
  proc(40, 10, "codex", { rss: 400 * MB, energyMw: 0 }),
  proc(41, 40, "rustc", { cmdline: "rustc --crate-name x", rss: 9 * GB, cpu: 420, energyMw: 3400 }),
  proc(90, 1, "Instagram", { rss: 2 * GB, cpu: 22 }),
];
const frame = (seq: number, procs = PROCS(seq)) => ({ type: "proc.snapshot" as const, at: 1_000 + seq * 2000, daemon: 20, total: procs.length, procs, seq, cost: { cpuMs: 4, held: 300_000 } });

let wire: FakeWire;
let unwireApp: () => void = () => {};
/** What the desktop app names: one browser tab, whose thread the daemon stamped. */
const APP_PROCS = [{ pid: 34, kind: "tab" as const, origins: ["http://localhost:5173"] }];

beforeEach(() => {
  resetSettings();
  resetProcs();
  syncPorts(HERE_KEY, [{ port: 5173, pid: 32, process: "node", loopback: true }]);
  wire = fakeWire({ "proc.watch": {}, "proc.unwatch": {}, "proc.kill": {} });
  provideDaemonWire(HERE_KEY, wire);
  getProcs(HERE_KEY).feedStatus("live");
  useStore.setState({ places: [HERE], workspaces: [workspace], sessions: { [WS]: [session("t_one", "Wire the page", 30), session("t_two", "Fast build", 40, { harness: "codex" })] } });
});

afterEach(() => {
  cleanup();
  provideDaemonWire(HERE_KEY, null);
  unwireApp();
  unwireApp = () => {};
});

const open = async (): Promise<ReturnType<typeof mountSettings>> => {
  const view = mountSettings({ api: settingsApi({ watchAppProcs: async () => APP_PROCS, unwatchAppProcs: async () => {}, onAppProcs: () => () => {} }).api, at: { kind: "group", group: "activity" } });
  unwireApp = wireAppProcs(useStore);
  await settle();
  return view;
};
const feed = (seq: number, procs?: ProcEntry[]) => act(() => getProcs(HERE_KEY).feed(frame(seq, procs)));
const groups = () => [...document.querySelectorAll<HTMLElement>("[data-activity-group]")].map(g => g.dataset["activityGroup"]);
const rowPids = () => [...document.querySelectorAll<HTMLElement>("[data-proc-row]")].map(r => r.dataset["procRow"]);
const toggle = (id: string) => fireEvent.click(document.querySelector(`[data-group="${id}"]`)!);
const ops = () => wire.calls.map(([op]) => op);

describe("Settings > Activity", () => {
  it("watches the computer only while open and draws its parts, each thread and the rest apart", async () => {
    const view = await open();
    expect(pageAt()).toBe("activity");
    expect(ops()).toEqual(["proc.watch"]);
    expect(document.querySelector("[data-k=activity-loading]")).not.toBeNull();
    feed(1);
    expect(groups()).toEqual(["wsp", "t_two", "t_one"]);
    view.unmount();
    expect(ops()).toEqual(["proc.watch", "proc.unwatch"]);
  });

  it("opens a thread onto its agent, tool server, dev server, slate run and browser tab, each with memory, CPU and energy", async () => {
    await open();
    feed(1);
    toggle("t_one");
    const kinds = [...document.querySelectorAll<HTMLElement>("[data-activity-group=t_one] [data-proc-row]")].map(r => [r.dataset["procRow"], r.dataset["kind"]]);
    expect(kinds).toEqual([
      ["30", "agent"],
      ["32", "dev"],
      ["31", "tool"],
      ["34", "tab"],
      ["slate-heading", "slate"],
      ["33", "slate"],
    ]);
    expect(document.querySelector("[data-proc-row=slate-heading]")!.textContent).toContain("Slate");
    act(() => useSlateStore.setState({ byThread: { t_one: { record: { document: { title: "Gold price" } } } as never } }));
    expect(document.querySelector("[data-proc-row=slate-heading]")!.textContent).toContain("Slate: Gold price");
    act(() => useSlateStore.setState({ byThread: {} }));
    expect(document.querySelector("[data-proc-row=slate-heading] [data-act]")).toBeNull();
    const vite = document.querySelector<HTMLElement>("[data-proc-row='32']")!;
    expect([...vite.querySelectorAll("[data-col]")].map(c => c.textContent)).toEqual(["300 MB", "2.0%", "11 mW"]);
    expect(vite.textContent).toContain(":5173");
  });

  it("puts the task manager's own cost last under wsp", async () => {
    await open();
    feed(1);
    feed(2);
    toggle("wsp");
    const row = document.querySelector<HTMLElement>("[data-proc-row=sampler]")!;
    expect(row.textContent).toContain("Task manager");
    // 4 ms of cpu over a 2 s tick.
    expect(row.querySelector("[data-col=cpu]")!.textContent).toBe("0.2%");
    expect(rowPids().at(-1)).toBe("sampler");
  });

  it("sorts by each column, filters as it is typed and narrows by the chips", async () => {
    await open();
    feed(1);
    fireEvent.click(document.querySelector("[data-sort=cpu]")!);
    expect(groups()).toEqual(["wsp", "t_two", "t_one"]);
    fireEvent.click(document.querySelector("[data-sort=energy]")!);
    expect(document.querySelector("[data-sort=energy]")!.getAttribute("aria-pressed")).toBe("true");
    const filter = document.querySelector<HTMLInputElement>("[data-k=activity-filter] input, input[data-k=activity-filter]")!;
    fireEvent.change(filter, { target: { value: "517" } });
    expect(groups()).toEqual(["t_one"]);
    expect(rowPids()).toEqual(["30", "32", "34"]);
    fireEvent.change(filter, { target: { value: "" } });
    fireEvent.click(document.querySelector("[data-chip-set=threads] [data-chip=t_two]")!);
    expect(groups()).toEqual(["t_two"]);
    fireEvent.click(document.querySelector("[data-chip-set=threads] [data-chip=t_two]")!);
    fireEvent.click(document.querySelector("[data-chip-set=kinds] [data-chip=slate]")!);
    expect(groups()).toEqual(["t_one"]);
  });

  it("draws the Processes, Kinds and Ports views over the same watch", async () => {
    await open();
    feed(1);
    act(() => useSettingsStore.getState().pickActivityView("processes"));
    expect(rowPids().slice(0, 2)).toEqual(["41", "30"]);
    expect(rowPids()).not.toContain("90");
    fireEvent.click(screen.getByRole("switch"));
    expect(rowPids()).toContain("90");
    act(() => useSettingsStore.getState().pickActivityView("kinds"));
    expect([...document.querySelectorAll<HTMLElement>("[data-activity-kind]")].map(k => k.dataset["activityKind"])).toEqual(["agent", "tool", "dev", "build", "slate", "tab", "wsp"]);
    act(() => useSettingsStore.getState().pickActivityView("ports"));
    expect([...document.querySelectorAll<HTMLElement>("[data-port-row]")].map(r => r.textContent)).toEqual([expect.stringContaining("5173")]);
    expect(document.querySelector("[data-port-row]")!.textContent).toContain("Local only");
    expect(ops()).toEqual(["proc.watch"]);
  });

  it("draws a line of the last minutes in each total, a point a frame", async () => {
    await open();
    for (let seq = 1; seq <= 4; seq++) feed(seq);
    feed(4);
    for (const k of ["mem", "cpu", "energy"]) expect(document.querySelector<HTMLElement>(`[data-tile-line=${k}]`)!.dataset["points"]).toBe("4");
    expect(document.querySelectorAll("[data-tile-line=cpu] [data-chart-line]").length).toBe(1);
  });

  it("draws CPU in sky, memory in violet and Power in teal, never the warning's amber, with no dot at a line's end", async () => {
    await open();
    for (let seq = 1; seq <= 3; seq++) feed(seq);
    const ink = (k: string) => document.querySelector<HTMLElement>(`[data-tile-line=${k}] > div`)!.className;
    expect([ink("cpu"), ink("mem"), ink("energy")]).toEqual([expect.stringContaining("text-sky-500"), expect.stringContaining("text-violet-500"), expect.stringContaining("text-teal-500")]);
    expect(document.querySelector<HTMLElement>("[data-k=activity-cpu] [data-tint]")!.dataset["tint"]).toBe("danger");
    expect(ink("cpu")).not.toMatch(/destructive|warning/);
    expect(document.querySelectorAll("[data-tile-line] [data-chart-end]").length).toBe(0);
  });

  it("gives End and End all the danger ink under the pointer, and the agent Stop and never End", async () => {
    await open();
    feed(1);
    toggle("t_one");
    const danger = ["border-destructive/50", "text-destructive-foreground"];
    for (const c of danger) expect(document.querySelector("[data-proc-row='32'] [data-act=end]")!.className).toContain(c);
    for (const c of danger) expect(document.querySelector("[data-group=t_one] [data-act=end-all]")!.className).toContain(c);
    expect(document.querySelector("[data-proc-row='30'] [data-act=end]")).toBeNull();
    expect(document.querySelector("[data-proc-row='30'] [data-act=stop]")!.className).not.toContain("border-destructive/50");
  });

  it("gives every process row a menu, the road to its acts below 768 px, where the inline acts stand down", async () => {
    await open();
    feed(1);
    toggle("t_one");
    const row = document.querySelector<HTMLElement>("[data-proc-row='32']")!;
    expect(row.querySelector("[data-act=end]")!.parentElement!.className).toContain("max-md:hidden");
    const menu = row.querySelector<HTMLElement>("[data-act=menu]")!;
    expect(menu.parentElement!.className).toContain("max-md:opacity-100");
    expect(menu.parentElement!.className).not.toContain("max-md:hidden");
    fireEvent.click(menu);
    await waitFor(() => expect(useContextMenuStore.getState().menu).not.toBeNull());
    expect(useContextMenuStore.getState().menu!.items.map(i => i.label)).toEqual(["Open :5173 in a browser tab", "End", "Open thread", "Copy command", "Copy PID"]);
    act(() => useContextMenuStore.getState().choose("end"));
    await settle();
    expect(wire.calls.at(-1)).toEqual(["proc.kill", { pid: 32, signal: "TERM", mode: "tree", startedAt: 0 }]);
    fireEvent.click(document.querySelector<HTMLElement>("[data-proc-row='30'] [data-act=menu]")!);
    await waitFor(() => expect(useContextMenuStore.getState().menu).not.toBeNull());
    expect(useContextMenuStore.getState().menu!.items.map(i => i.id)).toEqual(["stop", "open-thread", "copy-command", "copy-pid"]);
    act(() => useContextMenuStore.getState().choose(null));
  });

  it("gives a thread's head a menu with End all and Open thread, its road below 768 px, where the act cell stands down", async () => {
    await open();
    feed(1);
    const head = document.querySelector<HTMLElement>("[data-group='t_one']")!;
    expect(head.querySelector("[data-act=end-all]")!.parentElement!.className).toContain("max-md:hidden");
    const menu = head.querySelector<HTMLElement>("[data-act=head-menu]")!;
    expect(menu.className).toContain("md:hidden");
    expect(menu.className).not.toContain("max-md:hidden");
    fireEvent.click(menu);
    await waitFor(() => expect(useContextMenuStore.getState().menu).not.toBeNull());
    expect(useContextMenuStore.getState().menu!.items.map(i => i.label)).toEqual(["End all", "Open thread"]);
    act(() => useContextMenuStore.getState().choose("end-all"));
    await waitFor(() => expect(document.querySelector("[data-k=activity-confirm]")).not.toBeNull());
    expect(head.getAttribute("aria-expanded")).toBe("false");
    expect(document.querySelector("[data-group='wsp'] [data-act=head-menu]")).toBeNull();
  });

  it("opens a port in a browser tab of its thread from the Ports view, beside End", async () => {
    await open();
    feed(1);
    act(() => useSettingsStore.getState().pickActivityView("ports"));
    const row = document.querySelector<HTMLElement>("[data-port-row='5173']")!;
    expect([...row.querySelectorAll<HTMLElement>("[data-act]")].map(b => b.dataset["act"])).toEqual(["open-port", "end", "menu"]);
    fireEvent.click(row.querySelector("[data-act=open-port]")!);
    await settle();
    expect(useStore.getState().selectedThreadId).toBe("t_one");
    expect(useStore.getState().settingsOpen).toBe(false);
    expect(Object.values(useBrowserTabs.getState().byWorkspaceId[WS] ?? {}).flatMap(t => t.entries)).toContainEqual({ port: 5173, path: "/" });
  });

  it("inks a figure only past its marks: the build in the danger ink, the agent in its own", async () => {
    await open();
    feed(1);
    toggle("t_two");
    const build = document.querySelector<HTMLElement>("[data-proc-row='41']")!;
    expect(build.querySelector("[data-col=mem]")!.className).toContain("text-destructive-foreground");
    expect(build.querySelector("[data-col=cpu]")!.className).toContain("text-destructive-foreground");
    expect(build.querySelector("[data-col=energy]")!.className).toContain("text-destructive-foreground");
    const agent = document.querySelector<HTMLElement>("[data-proc-row='40']")!;
    for (const col of ["mem", "cpu", "energy"]) expect(agent.querySelector(`[data-col=${col}]`)!.className).not.toMatch(/warning-foreground|destructive-foreground/);
    expect(document.querySelector<HTMLElement>("[data-k=activity-cpu] [data-tint]")!.dataset["tint"]).toBe("danger");
  });

  it("ends a process with TERM and keeps its spinner until it leaves the frame", async () => {
    await open();
    feed(1);
    toggle("t_one");
    fireEvent.click(document.querySelector("[data-proc-row='32'] [data-act=end]")!);
    await settle();
    // The process and everything under it, checked against the start its row was read with.
    expect(wire.calls.at(-1)).toEqual(["proc.kill", { pid: 32, signal: "TERM", mode: "tree", startedAt: 0 }]);
    expect(document.querySelector("[data-proc-row='32']")!.textContent).toContain("Ending");
    feed(2, PROCS(2).filter(p => p.pid !== 32));
    expect(document.querySelector("[data-proc-row='32']")).toBeNull();
  });

  it("opens from a computer's page scoped to that computer", async () => {
    mountSettings({ api: settingsApi().api, at: { kind: "computer", id: "here" } });
    await settle();
    const row = document.querySelector<HTMLElement>("[data-k=activity-on]")!;
    expect(row.textContent).toContain("2 threads running");
    fireEvent.click(row);
    await settle();
    expect(pageAt()).toBe("activity");
    expect(useSettingsStore.getState().activityPlace).toBe("here");
    expect(document.querySelector<HTMLElement>("[data-activity-at]")!.dataset["activityAt"]).toBe("here");
  });

  it("stands third in the sidebar's corner, an icon button like its two neighbours, and opens Activity", async () => {
    render(
      <TooltipProvider>
        <SidebarCorner />
      </TooltipProvider>,
    );
    const buttons = [...document.querySelectorAll<HTMLElement>("[data-sidebar-corner] button")];
    expect(buttons.map(b => b.getAttribute("aria-label"))).toEqual(["Settings", "Usage", "Activity"]);
    expect(new Set(buttons.map(b => b.className)).size).toBe(1);
    const activity = buttons[2]!;
    expect(activity.textContent).toBe("");
    expect(activity.querySelector("svg.lucide-activity")).not.toBeNull();
    fireEvent.focus(activity);
    await waitFor(() => expect(document.querySelector("[data-slot=tooltip-popup], [data-slot=tooltip-content]")?.textContent).toBe("Activity"));
    fireEvent.click(activity);
    expect(useStore.getState().settingsOpen).toBe(true);
    expect(useSettingsStore.getState().at).toEqual({ kind: "group", group: "activity" });
  });

  it("on a box reads only what stands under the threads' cgroups each tick, however many, and the whole table once Every process asks for it", async () => {
    const BOX: PlaceView = { id: "p_box", kind: "computer", name: "lab-box", default: false, present: true } as PlaceView;
    const onBox: WorkspaceView = { ...workspace, id: "ws_box", kind: "place", place: "p_box" } as WorkspaceView;
    const boxWire = fakeWire({ "proc.watch": {}, "proc.unwatch": {} });
    provideDaemonWire("ws_box", boxWire);
    getProcs("ws_box").feedStatus("live");
    useStore.setState({ places: [HERE, BOX], workspaces: [workspace, onBox], sessions: { ws_box: Array.from({ length: 17 }, (_, i) => ({ ...session(`t_box${i}`, "Box build", 0), workspaceId: "ws_box", pid: undefined }) as SessionView) } });
    useSettingsStore.getState().pickActivityPlace("p_box");
    const view = await open();
    expect(boxWire.calls).toEqual([["proc.watch", { cgroups: ["/wsp-threads"] }]]);
    act(() => useSettingsStore.getState().pickActivityView("processes"));
    fireEvent.click(screen.getByRole("switch"));
    await settle();
    expect(boxWire.calls.at(-1)).toEqual(["proc.watch", {}]);
    fireEvent.click(screen.getByRole("switch"));
    await settle();
    expect(boxWire.calls.at(-1)).toEqual(["proc.watch", { cgroups: ["/wsp-threads"] }]);
    view.unmount();
    expect(boxWire.calls.at(-1)).toEqual(["proc.unwatch", {}]);
    provideDaemonWire("ws_box", null);
  });

  it("on a box offers Stop on the agent its cgroup holds, never End, and says a stop ends everything the thread started there", async () => {
    const BOX: PlaceView = { id: "p_box", kind: "computer", name: "lab-box", default: false, present: true } as PlaceView;
    const onBox: WorkspaceView = { ...workspace, id: "ws_box", kind: "place", place: "p_box" } as WorkspaceView;
    const boxWire = fakeWire({ "proc.watch": {}, "proc.unwatch": {}, "proc.kill": {} });
    provideDaemonWire("ws_box", boxWire);
    getProcs("ws_box").feedStatus("live");
    useStore.setState({ places: [HERE, BOX], workspaces: [workspace, onBox], sessions: { ws_box: [{ ...session("t_box", "Box build", 0), workspaceId: "ws_box", pid: undefined } as SessionView] } });
    useSettingsStore.getState().pickActivityPlace("p_box");
    await open();
    const cg = { cgroup: "/wsp-threads/t_box" };
    const procs = [proc(5, 1, "wsp-daemon"), proc(100, 1, "bash", { cmdline: "bash /root/.wsp/run/r1.sh", ...cg }), proc(101, 100, "claude", { cmdline: "claude -p", ...cg }), proc(102, 101, "node", { cmdline: "node /opt/wsp/mcp.js", ...cg })];
    act(() => getProcs("ws_box").feed({ type: "proc.snapshot", at: 3_000, daemon: 5, total: procs.length, procs, seq: 1 }));
    toggle("t_box");
    const agent = document.querySelector<HTMLElement>("[data-proc-row='101']")!;
    expect(agent.dataset["kind"]).toBe("agent");
    expect(document.querySelector("[data-proc-row='100']")).toBeNull();
    expect([...agent.querySelectorAll<HTMLElement>("[data-act]")].map(b => b.dataset["act"])).toEqual(["stop", "menu"]);
    fireEvent.click(agent.querySelector("[data-act=stop]")!);
    await waitFor(() => expect(document.querySelector("[data-k=activity-confirm]")).not.toBeNull());
    expect(screen.getByText(/ends with it, its dev and tool servers included/)).toBeTruthy();
    provideDaemonWire("ws_box", null);
  });

  it("says a stop on this computer leaves the dev and tool servers running", async () => {
    await open();
    feed(1);
    toggle("t_one");
    fireEvent.click(document.querySelector("[data-proc-row='30'] [data-act=stop]")!);
    await waitFor(() => expect(document.querySelector("[data-k=activity-confirm]")).not.toBeNull());
    expect(screen.getByText(/The dev server and tool servers it started keep running/)).toBeTruthy();
  });
});
