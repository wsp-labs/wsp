// SPDX-License-Identifier: AGPL-3.0-only
// The host's one log line when a resume finds the agent's session gone and the
// turn falls back to a new one: what the machine holds for the lost session,
// read with Codex's own reading in bash over a CODEX_HOME built here, so the
// next loss names its own cause. The adapter's start goes through the
// protocol's startAfterLoss, the road both real adapters take.
import { execFile } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import { parseSessionHeld, sessionHeldCommand } from "@wsp/adapter-codex";
import { startAfterLoss, type AdapterEvent, type TurnResult } from "@wsp/protocol";
import { afterAll, afterEach, describe, expect, it, vi } from "vitest";
import { createRuntime, type HarnessAdapterFactory, type HarnessStartOptions } from "../src/runtime.js";
import { seedAfterLoss } from "../src/threads/launch.js";
import { memoryStore } from "../src/store.js";
import { stubBackend, createOn } from "./stub-backend.js";
import { until } from "./until.js";

const FIRST = "01a1424a-7c2e-7d31-9f00-5b2a1c3d4e5f";
const SECOND = "01a1424b-fa29-7aa0-8c11-0d9e8f7a6b5c";
const LOST = `codex could not open the thread: no rollout found for thread id ${FIRST}`;
const OPENING = "Remember the word lantern.";
const run = promisify(execFile);

const homes: string[] = [];
afterAll(() => {
  for (const home of homes) rmSync(home, { recursive: true, force: true });
});
afterEach(() => {
  vi.restoreAllMocks();
});

/** A home whose index still names the first session's rollout, a paginated row whose file is gone. */
async function lostHome(): Promise<{ home: string; named: string }> {
  const home = mkdtempSync(join(tmpdir(), "wsp-lost-session-"));
  homes.push(home);
  const named = join(home, `sessions/2026/10/16/rollout-2026-10-16T01-19-52-${FIRST}.jsonl`);
  await run("sqlite3", [
    join(home, "state_5.sqlite"),
    `create table threads (id text primary key, rollout_path text not null, title text not null, history_mode text not null default 'legacy');
     insert into threads values ('${FIRST}', '${named}', '${OPENING}', 'paginated');`,
  ]);
  return { home, named };
}

/** An agent whose resume of `lost` is refused the way Codex refuses one, and whose fresh start announces SECOND; its
 * reading of the store is Codex's own, over `home`. */
function losingAdapter(home: string, starts: HarnessStartOptions[], lost: boolean): HarnessAdapterFactory {
  const launch = (o: HarnessStartOptions) => {
    starts.push(o);
    const sessionId = o.resume ?? (starts.length === 1 ? FIRST : SECOND);
    const refused = lost && o.resume !== undefined;
    const result: TurnResult = refused ? { status: "failed", error: LOST, text: "" } : { status: "completed", text: "ok" };
    const emit = (e: AdapterEvent): void => o.onEvent(e);
    const finished = (async () => {
      await Promise.resolve();
      if (!refused) emit({ type: "session.start", sessionId });
      emit({ type: "turn.done", sessionId, result });
      emit({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
      return result;
    })();
    return { localId: sessionId, finished, interrupt: async () => {} };
  };
  return () => ({
    steers: false,
    sessionHeld: (sessionId, exec) => exec(sessionHeldCommand({ home, threadId: sessionId })).then(parseSessionHeld),
    start: o => {
      if (o.resume === undefined || o.seed === undefined) return launch(o);
      const { first, now, finished } = startAfterLoss({ ...o, resume: o.resume, seed: o.seed }, LOST, launch);
      return { localId: first.localId, finished, interrupt: () => now().interrupt() };
    },
  });
}

/** A runtime on a stub machine whose guest runs the store reading in bash here, and a thread's two turns on it. */
async function twoTurns(lost: boolean): Promise<{ lines: string[]; starts: HarnessStartOptions[]; named: string; workspaceId: string }> {
  const { home, named } = await lostHome();
  const backend = stubBackend();
  const inner = backend.execImpl;
  backend.execImpl = async (m, cmd, stdin) => (cmd.includes("/proc/sys/kernel/random/boot_id") ? { exitCode: 0, stdout: (await run("bash", ["-c", cmd])).stdout, stderr: "" } : inner(m, cmd, stdin));
  const lines: string[] = [];
  vi.spyOn(console, "warn").mockImplementation((...words: unknown[]) => void lines.push(words.map(String).join(" ")));
  const starts: HarnessStartOptions[] = [];
  const rt = createRuntime({ backend, store: memoryStore(), adapters: { codex: losingAdapter(home, starts, lost) } });
  try {
    const ws = await createOn(rt, { golden: "snap_g", name: "a" });
    const first = await rt.sessions.start(ws.id, { prompt: OPENING, harness: "codex" });
    await first.finished;
    const threadId = first.view().threadId!;
    await (await rt.sessions.start(ws.id, { thread: threadId, prompt: "what was the word?", harness: "codex" })).finished;
    // The reading runs beside the turn, so its line lands a moment after; a line that never comes fails on the count.
    if (lost) await until(() => lines.some(line => line.includes(`session ${FIRST}`)), 5000).catch(() => undefined);
    return { lines: lines.filter(line => line.includes("was gone from")), starts, named, workspaceId: ws.id };
  } finally {
    await rt.close();
  }
}

describe("a resume that finds the agent's session gone", () => {
  it("logs one line naming what the machine holds for it: its rollout files, its index row and the path it names, the boot", async () => {
    const { lines, starts, named, workspaceId } = await twoTurns(true);
    expect(starts.map(s => s.resume)).toEqual([undefined, FIRST, undefined]);
    expect(lines).toHaveLength(1);
    const line = lines[0]!;
    expect(line).toMatch(new RegExp(`^session ${FIRST} of thread [0-9a-f]{8} was gone from ${workspaceId}, so its turn started a new one: boot (unread|[0-9a-f-]{36}); `));
    expect(line).toContain("; rollout files: none; ");
    expect(line).toContain(`; index row: state_5.sqlite {"path":"${named}","mode":"paginated"}, the file it names is missing`);
    expect(line).not.toContain(OPENING);
    expect(line).not.toContain("what was the word?");
  });

  it("hands the seed over and logs why when the reading throws before it returns", async () => {
    const lines: string[] = [];
    vi.spyOn(console, "warn").mockImplementation((...words: unknown[]) => void lines.push(words.map(String).join(" ")));
    const machine = { exec: () => Promise.reject(new Error("no machine")), takesStdin: true as const };
    const refused = { sessionHeld: (): Promise<string> => { throw new Error("threadId must be a plain slug"); } };
    const seed = seedAfterLoss(refused, machine, { sessionId: FIRST, threadId: SECOND, workspaceId: "ws_a" }, async () => "the thread so far");
    await expect(Promise.resolve().then(seed)).resolves.toBe("the thread so far");
    await until(() => lines.length === 1, 1000);
    expect(lines[0]).toMatch(/: not read: threadId must be a plain slug$/);
  });

  it("logs nothing when the resume opens the session", async () => {
    const { lines, starts } = await twoTurns(false);
    expect(starts.map(s => s.resume)).toEqual([undefined, FIRST]);
    expect(lines).toEqual([]);
  });
});
