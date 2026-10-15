// SPDX-License-Identifier: AGPL-3.0-only
// A slate's run on the thread's own box, started by its timer: each run there moves the time its record ends at,
// and a label that reads that time through ago() moves with the clock between runs and starts over at each run.
import { execFileSync } from "node:child_process";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ExecResult, Machine } from "@wsp/engine";
import type { Caller } from "@wsp/protocol";
import { boxSlateDir } from "../src/slate-mcp-on.js";
import { createSlates } from "../src/slates.js";
import { memoryStore } from "../src/store.js";

const made: string[] = [];
const open: { close(): void }[] = [];
afterEach(() => {
  for (const slates of open.splice(0)) slates.close();
  vi.useRealTimers();
  for (const dir of made.splice(0)) rmSync(dir, { recursive: true, force: true });
});
const temp = (): string => {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), "slate-box-timer-")));
  made.push(dir);
  return dir;
};

/** This computer's bash as the box: every exec and run is one bash to its end. */
function bashMachine(): Machine {
  const bash = async (cmd: string): Promise<ExecResult> => {
    try {
      return { exitCode: 0, stdout: execFileSync("bash", ["-c", cmd], { encoding: "utf8" }), stderr: "" };
    } catch (e) {
      const x = e as { status?: number; stdout?: string; stderr?: string };
      return { exitCode: x.status ?? 1, stdout: x.stdout ?? "", stderr: x.stderr ?? "" };
    }
  };
  return { id: "m1", kind: "sandbox", exec: bash, run: bash } as unknown as Machine;
}

describe("a box run on its timer", () => {
  it("moves the time a label reads with each run there and with the clock between them", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"] });
    vi.setSystemTime(new Date("2026-10-15T09:00:00Z"));
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const folder = temp();
    const slates = createSlates({
      store: memoryStore(),
      now: () => Date.now(),
      record: () => {},
      emit: () => {},
      thread: () => ({ workspaceId: "w1", rootThreadId: thread, sessionId: "s1", folder, hostFolder: folder, computer: "hetzner" }),
      machineOf: () => bashMachine(),
      under: lead => [lead],
      threadOfToken: () => thread,
      sources: (threadId, workspaceId) => ({ threadId, workspaceId, now: Date.now(), rows: () => [] }) as never,
      deliver: async () => ({ outcome: "started" }),
      runEnv: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
    });
    open.push(slates);
    const asThread: Caller = { origin: "here", by: { kind: "thread", threadId: thread, workspaceId: "w1", rootThreadId: thread } };
    await slates.write({ text: `<slate title="Feed"><run name="check" cmd="echo ok" every={60} timeout={20} /><column><text>Checked {ago($check.endedAt)}</text></column></slate>` }, asThread);
    slates.subscribe({ threadId: thread, sources: [] });
    const check = async () => (await slates.get(thread))!.values["check"] as { state: string; runs?: number; endedAt?: number };
    const label = async () => /Checked \S+ ago/.exec((await slates.read({ threadId: thread, text: false }, asThread)).text)?.[0];
    for (let i = 0; i < 50 && (await slates.get(thread))!.asks.length === 0; i++) await new Promise(r => setTimeout(r, 20));
    for (const ask of (await slates.get(thread))!.asks) await slates.approve({ threadId: thread, key: ask.key, scope: "thread" });
    const ran = async (runs: number) => {
      for (let i = 0; i < 200 && !((await check()).state === "done" && (await check()).runs === runs); i++) await new Promise(r => setTimeout(r, 20));
      return check();
    };
    const first = await ran(1);
    expect(await label()).toBe("Checked 0s ago");
    vi.advanceTimersByTime(54_000);
    expect(await label()).toBe("Checked 54s ago");
    // The next tick runs it again there, and the label starts over from that run's end.
    vi.advanceTimersByTime(6_000);
    const second = await ran(2);
    expect(second.endedAt! - first.endedAt!).toBe(60_000);
    expect(await label()).toBe("Checked 0s ago");
    vi.advanceTimersByTime(30_000);
    expect(await label()).toBe("Checked 30s ago");
  }, 30_000);
});
