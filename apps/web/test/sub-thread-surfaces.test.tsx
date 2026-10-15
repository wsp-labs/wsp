// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread open: the one strip over its composer for each state that holds
// or blocks it, with that state's one act, and the machine pane's rate, what it
// has cost so far, and when it is deleted, with no pause, wake, rebuild or
// bring back.
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { subPushAgainPrompt, type PlaceView, type SessionView, type SubRow, type WorkspaceStatus, type WorkspaceView } from "@wsp/protocol";
import { SubThreadStrip } from "../src/components/chat/SubStrip.js";
import { MachineSurface } from "../src/components/machine/MachineSurface.js";
import { resetLive } from "../src/machine/live.js";
import { useStore } from "../src/protocol/store.js";
import { useCloudFlow } from "../src/settings/add/cloudFlow.js";
import { useNotices } from "../src/notices/store.js";
import { APP_LOCALE } from "../src/lib/timestampFormat.js";

const MIN = 60_000;
const WS = "ws_fix";
const BRANCH = "wsp/fix-the-flaky-upload-test";
const HERE: PlaceView = { id: "here", kind: "computer", name: "zingzy-mbp", label: "zingzy's MacBook Pro", mac: "macbook", default: true, present: true, takesForks: false };
/** Boat on the $20 plan, which sells small, default and large, as places.list reads it. */
const BOAT: PlaceView = {
  id: "box",
  kind: "provider",
  name: "box",
  default: false,
  takesForks: true,
  sizes: [
    { cpu: 2, memMb: 4096, rateUsdPerHour: 0.018 },
    { cpu: 4, memMb: 8192, rateUsdPerHour: 0.036 },
    { cpu: 8, memMb: 16384, rateUsdPerHour: 0.072 },
    { cpu: 16, memMb: 32768, rateUsdPerHour: 0.2 },
  ],
  plan: { key: "box_20", trial: false, machinesAtOnce: 100, sizes: ["2x4", "4x8", "8x16"] },
} as PlaceView;
const WORKSPACE: WorkspaceView = { id: WS, name: "Fix the flaky upload test", kind: "cloud", provider: "box", place: "box", machineId: "bx_7k2q9", project: { id: "pr_web", name: "web", path: "/root/web", computer: "here" }, phase: "napping", golden: "img", createdAt: "2026-10-12T08:00:00Z" } as WorkspaceView;
const LEAD: WorkspaceView = { id: "ws_lead", name: "web", kind: "local", machineId: "local", project: { id: "pr_web", name: "web", path: "/Users/dev/web", computer: "here" }, phase: "running", golden: "", createdAt: "2026-10-12T07:00:00Z", place: "here" } as WorkspaceView;
const sub = (over: Partial<SubRow>): SubRow => ({ state: "napping", on: "box", machine: "Fix the flaky upload test", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, branch: BRANCH, ...over });
const row = (over: Partial<SessionView>): SessionView => ({ id: "s_fix", threadId: "th_fix", workspaceId: WS, harness: "claude", status: "completed", prompt: "Fix it.", harnessTitle: "Fix the flaky upload test", startedBy: "agent", parentThreadId: "th_lead", rootThreadId: "th_lead", startedAt: Date.now() - 30 * MIN, endedAt: Date.now() - 4 * MIN, ...over });
const LEAD_ROW: SessionView = { id: "s_lead", threadId: "th_lead", workspaceId: LEAD.id, harness: "claude", status: "completed", prompt: "Fan out", startedBy: "person", startedAt: Date.now() - 80 * MIN, endedAt: Date.now() - 70 * MIN };

const strip = () => document.querySelector<HTMLElement>("[data-sub-strip]");
const said = () => ({ k: strip()?.dataset["subStrip"], title: strip()?.querySelector("[data-sub-strip-title]")?.textContent, note: strip()?.querySelector("[data-sub-strip-note]")?.textContent, acts: [...document.querySelectorAll<HTMLElement>("[data-sub-strip-act]")].map(b => b.textContent) });
const act_ = (k: string) => document.querySelector<HTMLButtonElement>(`[data-sub-strip-act="${k}"]`)!;

let started: Array<Record<string, unknown>>;
let trips: Array<Record<string, unknown>>;
beforeEach(() => {
  started = [];
  trips = [];
  vi.useFakeTimers({ now: Date.parse("2026-10-12T08:13:00.000Z"), toFake: ["Date"] });
  useStore.setState({
    places: [HERE, BOAT],
    workspaces: [WORKSPACE, LEAD],
    sessions: { [LEAD.id]: [LEAD_ROW] },
    api: { startSession: async (o: Record<string, unknown>) => (started.push(o), { id: "s", workspaceId: String(o["workspaceId"]) }), exportProject: async () => ({}) } as never,
  });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  useStore.setState({ places: [], workspaces: [], sessions: {}, statuses: {}, costs: {}, api: null });
});

const mountStrip = (r: SessionView) => {
  useStore.setState(s => ({ sessions: { ...s.sessions, [WS]: [r] } }));
  return render(<SubThreadStrip workspaceId={WS} row={r} />);
};

describe("the strip over a sub-thread's composer", () => {
  it("says a released machine was deleted and that a message starts a new one, with no act, the composer left to send", () => {
    mountStrip(row({ sub: sub({ state: "released", releasedAt: Date.now() - 9 * MIN }) }));
    expect(said()).toEqual({ k: "released", title: "This thread's machine was deleted", note: "A message starts a new one on Boat, and the conversation goes on.", acts: [] });
  });

  it("says work not pushed and why the push was refused, with Copy to the computer by its name and Push again", () => {
    mountStrip(row({ sub: sub({ held: { commits: 2, files: 3 }, result: { branch: BRANCH, pushedSha: null, commits: 2, unpushed: 2, changed: 3, readAt: "2026-10-12T08:09:00Z", pushRefused: "Permission to acme/web.git denied to wsp-bot" } }) }));
    expect(said()).toEqual({
      k: "not-pushed",
      title: "2 commits and 3 changed files not pushed",
      note: `The push to ${BRANCH} was refused: Permission to acme/web.git denied to wsp-bot. This machine stays until the work is pushed or copied to your computer.`,
      acts: ["Copy to zingzy's MacBook Pro", "Push again"],
    });
    fireEvent.click(act_("push-again"));
    expect(started).toEqual([expect.objectContaining({ workspaceId: WS, thread: "th_fix", prompt: subPushAgainPrompt(BRANCH) })]);
  });

  it("draws no strip for a sub-thread whose restart failed: the start line is the one place that says it", () => {
    const failed = { startedAt: Date.now() - MIN, machine: { since: Date.now() - MIN, states: [] }, failed: { step: "machine" as const, said: "Boat has no free machines of this size right now", fix: "Start it again in a few minutes." }, asked: "what was the word?", log: [] };
    mountStrip(row({ sub: sub({ state: "released", releasedAt: Date.now() - 40 * MIN, spawn: failed }) }));
    expect(strip()).toBeNull();
  });

  it("names the branch the work is on where no push was refused", () => {
    mountStrip(row({ sub: sub({ held: { commits: 1, files: 0 } }) }));
    expect(said().note).toBe(`They are on ${BRANCH} on this machine. This machine stays until the work is pushed or copied to your computer.`);
  });

  it("names the day from which Boat may delete the disk of a napping machine held over work not pushed", () => {
    const until = Date.parse("2026-10-26T12:00:00.000Z");
    mountStrip(row({ diskKeptUntil: until, sub: sub({ held: { commits: 1, files: 0 } }) }));
    const day = new Intl.DateTimeFormat(APP_LOCALE, { month: "short", day: "numeric" }).format(until);
    expect(said().note).toBe(`They are on ${BRANCH} on this machine. This machine stays until the work is pushed or copied to your computer, but Boat may delete its disk from ${day}.`);
  });

  it("counts down to the delete of a machine whose work is all pushed", () => {
    mountStrip(row({ sub: sub({ deleteAt: Date.now() + 18 * MIN }) }));
    expect(said()).toEqual({ k: "deleting", title: "Its machine is deleted in 18 min", note: `Everything is pushed to ${BRANCH}. The transcript stays, and a message after the delete starts a new machine.`, acts: [] });
  });

  it("says a kill for memory once, and Start again asks the lead to start the work at the next size", () => {
    const failure = "The agent on Fix the flaky upload test was killed for memory: its machine is small, 2 vCPU, 4 GB, and Boat's own processes hold part of that. Start the work again with size 4x8.";
    mountStrip(row({ status: "failed", failure, sub: sub({ deleteAt: Date.now() + 27 * MIN }) }));
    expect(said()).toMatchObject({ k: "oom", title: "Killed for memory", note: failure, acts: ["Start again at 4x8"] });
    fireEvent.click(act_("start-again"));
    expect(started).toEqual([expect.objectContaining({ workspaceId: LEAD.id, thread: "th_lead", prompt: 'Start the work of the sub-thread "Fix the flaky upload test" again on Boat at size 4x8: its agent was killed for memory.' })]);
  });

  it("offers no Start again where the plan sells no bigger size, as the line above it says: a large machine on the $20 plan, a default one on the trial", () => {
    const failure = "The agent on Fix the flaky upload test was killed for memory: its machine is large, 8 vCPU, 16 GB, and Boat's own processes hold part of that. Boat sells no bigger size, so the work has to use less memory.";
    const { unmount } = mountStrip(row({ status: "failed", failure, sub: sub({ size: { cpu: 8, memMb: 16384 }, deleteAt: Date.now() + 27 * MIN }) }));
    expect(said()).toMatchObject({ k: "oom", acts: [] });
    unmount();
    useStore.setState({ places: [HERE, { ...BOAT, plan: { trial: true, machinesAtOnce: 2, sizes: ["2x4", "4x8"] } }] });
    mountStrip(row({ status: "failed", failure, sub: sub({ size: { cpu: 4, memMb: 8192 }, deleteAt: Date.now() + 27 * MIN }) }));
    expect(said()).toMatchObject({ k: "oom", acts: [] });
  });

  it("says Boat left this host, and Add Boat again opens Add a cloud provider", () => {
    mountStrip(row({ sub: sub({ state: "released", releasedAt: Date.now() - 2 * MIN, cloudGone: true }) }));
    expect(said()).toEqual({ k: "forgotten", title: "Boat was removed from zingzy's MacBook Pro", note: "This thread has no machine to start until Boat is added again. Its transcript stays.", acts: ["Add Boat again"] });
    fireEvent.click(act_("add-again"));
    expect(useCloudFlow.getState()).toMatchObject({ open: true, provider: "box", step: "key" });
  });

  it("says a held spawn waits for a machine, with Raise machines at once, or for the next minute of starts, with none", () => {
    const { unmount } = mountStrip(row({ status: "running", endedAt: undefined, capped: { placeId: "box", place: "Boat", running: 3, atOnce: 3, why: "slot" }, sub: sub({ state: "waiting" }) }));
    expect(said()).toEqual({ k: "waiting-machines", title: "Waiting for a machine on Boat", note: "3 of 3 machines are running. It starts on its own when one frees.", acts: ["Raise machines at once"] });
    unmount();
    mountStrip(row({ status: "running", endedAt: undefined, capped: { placeId: "box", place: "Boat", running: 12, atOnce: 12, why: "minute" }, sub: sub({ state: "waiting" }) }));
    expect(said()).toEqual({ k: "waiting-minute", title: "Waiting for Boat's next minute of starts", note: "12 of 12 starts this minute are used. It starts on its own within the minute.", acts: [] });
  });

  it("says a send the host refuses as a notice naming the thread, for Push again and for Start again", async () => {
    useNotices.setState({ notices: [] });
    useStore.setState({ api: { startSession: async () => Promise.reject(new Error("thread th_fix is running a turn")) } as never });
    mountStrip(row({ sub: sub({ held: { commits: 1, files: 0 } }) }));
    fireEvent.click(act_("push-again"));
    await waitFor(() => expect(useNotices.getState().notices.map(n => n.text)).toEqual(["thread th_fix is running a turn"]));
    cleanup();
    useNotices.setState({ notices: [] });
    const failure = "The agent on Fix the flaky upload test was killed for memory: its machine is small, 2 vCPU, 4 GB, and Boat's own processes hold part of that. Start the work again with size 4x8.";
    mountStrip(row({ status: "failed", failure, sub: sub({ deleteAt: Date.now() + 27 * MIN }) }));
    fireEvent.click(act_("start-again"));
    await waitFor(() => expect(useNotices.getState().notices.map(n => n.text)).toEqual(["thread th_fix is running a turn"]));
  });

  it("draws nothing while the turn runs on a standing machine", () => {
    mountStrip(row({ status: "running", endedAt: undefined, sub: sub({ state: "running" }) }));
    expect(strip()).toBeNull();
  });
});

describe("a sub-machine's pane", () => {
  const status = (over: Partial<WorkspaceStatus> = {}): WorkspaceStatus => ({ ...WORKSPACE, machineState: "running", reach: { state: "ok" }, size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, ...over }) as WorkspaceStatus;
  const facts = () => [...document.querySelectorAll("[data-facts] [data-chip]")].map(c => c.textContent);
  const actions = () => [...document.querySelectorAll<HTMLButtonElement>("[data-machine-actions] button")].map(b => [b.dataset["action"], b.textContent]);

  beforeEach(() => resetLive());

  it("shows the rate and what it has cost so far while it runs, and no pause, wake, rebuild or bring back", () => {
    act(() => useStore.setState({ workspaces: [{ ...WORKSPACE, phase: "running" }, LEAD], statuses: { [WS]: status({ phase: "running" }) }, sessions: { [WS]: [row({ status: "running", endedAt: undefined, sub: sub({ state: "running" }) })], [LEAD.id]: [LEAD_ROW] }, costs: { [WS]: { rateUsdPerHour: 0.018, accruedUsd: 0.004, at: new Date().toISOString() } } }));
    render(<MachineSurface workspaceId={WS} />);
    expect(facts()).toContain("$0.018/hr, $0.004 so far");
    expect(actions()).toEqual([
      ["export-project", "Copy to zingzy's MacBook Pro"],
      ["delete", "Delete"],
    ]);
  });

  it("shows when a done machine is deleted, or that its work keeps it", () => {
    act(() => useStore.setState({ statuses: { [WS]: status({ phase: "napping", machineState: "paused" }) }, sessions: { [WS]: [row({ sub: sub({ deleteAt: Date.now() + 18 * MIN }) })], [LEAD.id]: [LEAD_ROW] }, costs: { [WS]: { rateUsdPerHour: 0, accruedUsd: 0.008, at: new Date().toISOString() } } }));
    const { unmount } = render(<MachineSurface workspaceId={WS} />);
    expect(facts()).toEqual(expect.arrayContaining(["$0.018/hr, $0.008 so far", "Deleted in 18 min"]));
    unmount();
    act(() => useStore.setState({ sessions: { [WS]: [row({ sub: sub({ held: { commits: 2, files: 3 } }) })], [LEAD.id]: [LEAD_ROW] } }));
    const kept = render(<MachineSurface workspaceId={WS} />);
    expect(facts()).toContain("Kept until its work is pushed");
    kept.unmount();
    const until = Date.parse("2026-10-26T12:00:00.000Z");
    act(() => useStore.setState({ sessions: { [WS]: [row({ diskKeptUntil: until, sub: sub({ held: { commits: 2, files: 3 } }) })], [LEAD.id]: [LEAD_ROW] } }));
    render(<MachineSurface workspaceId={WS} />);
    expect(facts()).toContain(`Boat may delete its disk from ${new Intl.DateTimeFormat(APP_LOCALE, { month: "short", day: "numeric" }).format(until)}`);
  });

  it("reads Deleted, not Gone, once the machine was released, the runtime saying its machine is gone", () => {
    act(() => useStore.setState({ workspaces: [{ ...WORKSPACE, phase: "released" }, LEAD], statuses: { [WS]: status({ phase: "released", machineState: "gone" }) }, sessions: { [WS]: [row({ sub: sub({ state: "released", releasedAt: Date.now() - 9 * MIN }) })], [LEAD.id]: [LEAD_ROW] } }));
    render(<MachineSurface workspaceId={WS} />);
    expect(facts()[0]).toBe("Deleted");
  });
});
