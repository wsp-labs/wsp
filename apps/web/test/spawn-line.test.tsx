// SPDX-License-Identifier: AGPL-3.0-only
// The line under the message a sub-thread's machine was made for: where it
// stands in the timeline, the steps it opens to off the row's spawn with the
// real facts (the branch and sha, Boat's machine and its states, the picked
// files, the agent and its picks), the live, done and failed lines, and Try
// again sending that message again.
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SubRow, SubSpawn } from "@wsp/protocol";
import type { MachineStart, MessagesTimelineRow, TimelineEntry } from "../src/adapt/index.js";
import { machineStartAt } from "../src/adapt/timeline-rows.js";
import { SpawnLine } from "../src/components/chat/SpawnLine.js";
import { spawnLogOf } from "../src/components/chat/spawnLog.js";
import { useStore } from "../src/protocol/store.js";
import { useNotices } from "../src/notices/store.js";

const T0 = Date.parse("2026-10-12T08:00:00.000Z");
const BRANCH = "feat/upload";
const SHA = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";
const spawned = (over: Partial<SubSpawn> = {}): SubSpawn => ({
  startedAt: T0,
  branch: { branch: BRANCH, sha: SHA, ms: 200 },
  machine: { since: T0 + 200, id: "bx_7k2q9", states: ["provisioning", "cloning", "ready"], ms: 31_000 },
  files: { since: T0 + 31_200, paths: [".env", ".env.local"], bytes: 4096, ms: 1_400 },
  uncommitted: 3,
  readyAt: T0 + 33_000,
  log: ["starting Fix the flaky upload test on Boat", "Boat's machine bx_7k2q9 is provisioning", "copying 2 files", "ready"],
  ...over,
});
const sub = (spawn: SubSpawn): SubRow => ({ state: "running", on: "box", machine: "Fix the flaky upload test", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, branch: "wsp/fix-the-flaky-upload-test", spawn });
const start = (spawn: SubSpawn): MachineStart => ({ workspaceId: "ws_fix", threadId: "th_fix", harness: "claude", title: "Fix the flaky upload test", sub: sub(spawn), agent: "Claude Code", model: "Opus 5.5", effort: "high", project: "web" });
const message = (id: string, role: "user" | "assistant", at: number, text = id): TimelineEntry => ({ id, kind: "message", createdAt: new Date(at).toISOString(), message: { id, role, text, turnId: null, streaming: false, createdAt: new Date(at).toISOString() } as never });
const line = (spawn: SubSpawn, answeredAt: number | null, prompt: string | null = "Fix the flaky upload test."): Extract<MessagesTimelineRow, { kind: "machine-start" }> => ({ kind: "machine-start", id: "machine-start", createdAt: null, start: start(spawn), answeredAt, prompt });

afterEach(cleanup);

describe("where the line stands", () => {
  it("after the person's first message at or after the start, with when the agent first answered and that message's words", () => {
    const entries = [message("old", "user", T0 - 3_600_000), message("old-reply", "assistant", T0 - 3_590_000), message("brief", "user", T0 + 34_000, "Fix it."), message("reply", "assistant", T0 + 49_000)];
    expect(machineStartAt(entries, start(spawned()))).toEqual({ at: 3, answeredAt: T0 + 49_000, prompt: "Fix it." });
  });

  it("last while the start is under way and its message is not in yet, and nowhere for a finished one whose message is not read", () => {
    const entries = [message("old", "user", T0 - 3_600_000)];
    expect(machineStartAt(entries, start(spawned({ readyAt: undefined })))).toEqual({ at: 1, answeredAt: null, prompt: null });
    expect(machineStartAt(entries, start(spawned()))).toBeNull();
  });
});

describe("the steps", () => {
  it("are the start's own facts in the order the host takes them, the uncommitted changes a warning between", () => {
    const log = spawnLogOf(sub(spawned()), { agent: "Claude Code", model: "Opus 5.5", effort: "high", project: "web", answeredAt: T0 + 49_000 })!;
    expect(log.steps.map(s => [s.state, s.name, s.note, s.ms])).toEqual([
      ["done", "Checked the branch", `${BRANCH} is pushed at a1b2c3d`, 200],
      ["done", "Asked Boat for a machine", "bx_7k2q9, provisioning, cloning, ready", 31_000],
      ["warned", `3 uncommitted changes in web did not travel; the sub-thread starts at ${BRANCH} as pushed (a1b2c3d).`, undefined, undefined],
      ["done", "Copied 2 picked files", ".env, .env.local, 4 KB", 1_400],
      ["done", "Starting Claude Code", "Opus 5.5, high", 16_000],
    ]);
    expect(log.tookMs).toBe(49_000);
  });

  it("read the machine under way while Boat makes it, and the steps after a failed one not started", () => {
    const live = spawnLogOf(sub(spawned({ machine: { since: T0 + 200, id: "bx_7k2q9", states: ["provisioning"] }, files: undefined, readyAt: undefined })), { agent: "Claude Code", project: "web", answeredAt: null })!;
    expect(live.steps.map(s => [s.id, s.state])).toEqual([["branch", "done"], ["machine", "working"], ["uncommitted", "warned"], ["agent", "waiting"]]);
    expect(live.steps[1]!.name).toBe("Asking Boat for a machine");
    const failed = spawnLogOf(sub(spawned({ machine: { since: T0 + 200, states: [] }, files: undefined, readyAt: undefined, uncommitted: undefined, failed: { step: "machine", said: "Boat has no free machines of this size right now", fix: "Start it again in a few minutes." } })), { agent: "Claude Code", project: "web", answeredAt: null })!;
    expect(failed.failed).toBe(true);
    expect(failed.steps.map(s => [s.id, s.state, s.said])).toEqual([["branch", "done", undefined], ["machine", "failed", "Boat has no free machines of this size right now"], ["agent", "waiting", undefined]]);
  });
});

describe("the line", () => {
  let started: Array<Record<string, unknown>>;
  beforeEach(() => {
    started = [];
    useStore.setState({ api: { startSession: async (o: Record<string, unknown>) => (started.push(o), { id: "s", workspaceId: "ws_fix" }) } as never });
  });
  afterEach(() => useStore.setState({ api: null }));

  it("says Boat is starting a machine with the time climbing and the spinner, shut, then opens to the steps and the raw log", async () => {
    vi.useFakeTimers({ now: T0 + 40_000 });
    try {
      render(<SpawnLine row={line(spawned(), null)} />);
      const head = document.querySelector<HTMLElement>("[data-k=spawn-line]")!;
      expect(document.querySelector("[data-spawn-line]")!.getAttribute("data-spawn-line")).toBe("live");
      expect(head.textContent).toBe("Starting a machine on Boat40s");
      expect(document.querySelector("[data-spawn-steps]")).toBeNull();
      fireEvent.click(head);
      expect(document.querySelector('[data-spawn-step="agent"]')!.getAttribute("data-state")).toBe("working");
      expect(document.querySelector("[data-spawn-log]")).toBeNull();
      fireEvent.click(document.querySelector("[data-k=spawn-details]")!);
      expect(document.querySelector("[data-spawn-log]")!.textContent).toContain("Boat's machine bx_7k2q9 is provisioning");
      await act(async () => void vi.advanceTimersByTime(2_000));
      expect(head.textContent).toBe("Starting a machine on Boat42s");
    } finally {
      vi.useRealTimers();
    }
  });

  it("folds into a quiet done line once the agent answered", () => {
    render(<SpawnLine row={line(spawned(), T0 + 49_000)} />);
    expect(document.querySelector("[data-spawn-line]")!.getAttribute("data-spawn-line")).toBe("done");
    expect(document.querySelector("[data-k=spawn-line]")!.textContent).toBe("Started on Boat in 49s");
  });

  it("stands open in the refusal ink on a failed start, Boat's words and the fix under the step, and Try again sends the message again", () => {
    render(<SpawnLine row={line(spawned({ machine: { since: T0 + 200, states: [] }, files: undefined, readyAt: undefined, failed: { step: "machine", said: "Boat has no free machines of this size right now", fix: "Start it again in a few minutes." } }), null, "what was the word?")} />);
    expect(document.querySelector("[data-spawn-line]")!.getAttribute("data-spawn-line")).toBe("failed");
    expect(document.querySelector("[data-k=spawn-line]")!.textContent).toBe("Could not start a machine on Boat");
    expect(document.querySelector('[data-spawn-step="machine"]')!.textContent).toContain("Boat has no free machines of this size right now Start it again in a few minutes.");
    fireEvent.click(document.querySelector("[data-k=spawn-try-again]")!);
    expect(started).toEqual([expect.objectContaining({ workspaceId: "ws_fix", thread: "th_fix", prompt: "what was the word?" })]);
  });

  it("says a Try again whose start failed again on the line alone, with no notice beside it", async () => {
    useNotices.setState({ notices: [] });
    const said = "Boat gave two machines in a row that could not run commands (bx_3, bx_4); Boat is restoring badly right now.";
    // The host puts the row of the start that failed again on the bus before it answers the refusal.
    const failedAgain = { id: "s_fix", threadId: "th_fix", workspaceId: "ws_fix", harness: "claude", status: "completed", sub: sub(spawned({ failed: { step: "machine", said, fix: "Start it again in a few minutes." } })) };
    useStore.setState({
      api: {
        startSession: async () => {
          useStore.setState({ sessions: { ws_fix: [failedAgain as never] } });
          throw new Error(`${said} Start it again in a few minutes.`);
        },
      } as never,
    });
    render(<SpawnLine row={line(spawned({ machine: { since: T0 + 200, states: [] }, files: undefined, readyAt: undefined, failed: { step: "machine", said: "Boat has no free machines of this size right now", fix: "Start it again in a few minutes." } }), null, "what was the word?")} />);
    fireEvent.click(document.querySelector("[data-k=spawn-try-again]")!);
    await act(async () => {
      for (let i = 0; i < 5; i++) await Promise.resolve();
    });
    expect(useNotices.getState().notices).toEqual([]);
    useStore.setState({ sessions: {} });
  });

  it("says a Try again the host refuses as a notice naming the thread", async () => {
    useNotices.setState({ notices: [] });
    useStore.setState({ api: { startSession: async () => Promise.reject(new Error("Boat's starts for today are used")) } as never });
    render(<SpawnLine row={line(spawned({ machine: { since: T0 + 200, states: [] }, files: undefined, readyAt: undefined, failed: { step: "machine", said: "Boat has no free machines of this size right now", fix: "Start it again in a few minutes." } }), null, "what was the word?")} />);
    fireEvent.click(document.querySelector("[data-k=spawn-try-again]")!);
    // A refusal no start reached is a notice at once: no row is waited for, and no timer runs.
    await act(async () => {
      for (let i = 0; i < 5; i++) await Promise.resolve();
    });
    expect(useNotices.getState().notices.map(n => n.text)).toEqual(["Boat's starts for today are used"]);
  });
});
