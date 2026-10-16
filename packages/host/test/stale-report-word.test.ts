// SPDX-License-Identifier: AGPL-3.0-only
// The line under each of the four lists for a workspace read while it ran and
// paused since: stopped where its computer's nap ends every process, napping
// where the memory is kept, from the pause mode the report carries.
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { PauseMode } from "@wsp/protocol";
import { afterAll, describe, expect, it } from "vitest";
import { CLI_VERBS, runVerb, type HostClient, type VerbDeps } from "../src/verbs.js";
import { captured } from "./verbs-fixture.js";

const dir = mkdtempSync(join(tmpdir(), "wsp-stale-word-"));
afterAll(() => rmSync(dir, { recursive: true, force: true }));

const report = (pauseMode: PauseMode) => ({
  target: { workspaceId: "ws_1" },
  home: "/root",
  user: "root",
  readAt: "2026-10-16T12:00:00.000Z",
  stale: "napping",
  pauseMode,
  agents: [],
  skills: [],
  servers: [],
  plugins: [],
  refused: [],
});

function hostAnswering(pauseMode: PauseMode): HostClient {
  const client = {
    request: async <T,>(op: string): Promise<T> => {
      if (op === "sessions.list") return { sessions: [{ id: "s_1", threadId: "t_landing", workspaceId: "ws_1", harness: "claude", status: "completed", startedAt: 1, prompt: "build it", startedBy: "person" }] } as T;
      if (op === "workspaces.get")
        return { workspace: { id: "ws_1", name: "landing", machineId: "m1", phase: "napping", golden: "snap_gold", createdAt: "2026-10-16T00:00:00.000Z", project: { id: "pr_1", name: "api", path: "/root/api", computer: "here" } } } as T;
      if (op === "agents.read") return { report: report(pauseMode) } as T;
      throw new Error(`asked for ${op}`);
    },
    events: async (): Promise<void> => {},
    onFrame: () => () => {},
    closed: new Promise<void>(() => {}),
    closeWords: () => "closed",
    close: () => {},
    terminate: () => {},
  };
  return client as unknown as HostClient;
}

const LISTS = ["agents", "skills", "servers", "plugins"] as const;
const tail = (word: string) => `${word}: this is what stood there when it last ran`;

/** What one run of a list said for a mode: its last line, and the mode its answer carries. */
const said = (text: string, answer: Record<string, unknown>): string => `${text.split("\n").at(-1)} (${String(answer["pauseMode"])})`;

describe("a kept report of a paused workspace", () => {
  it("heads each list's line with stopped on a computer whose nap ends every process and napping on one that keeps the memory, and answers the mode", async () => {
    const modes = [
      ["disk", "stopped"],
      ["memory", "napping"],
    ] as const;
    const got: string[] = [];
    for (const [mode] of modes) {
      for (const list of LISTS) {
        const verb = CLI_VERBS.find(v => v.name === list)!;
        const dial = async () => hostAnswering(mode);
        const plain = captured();
        const json = captured();
        expect(await runVerb(verb, [list, "t_landing"], plain, () => join(dir, "state.json"), { env: {}, dial }), plain.errors.join(" ")).toBe(0);
        expect(await runVerb(verb, [list, "t_landing", "--json"], json, () => join(dir, "state.json"), { env: {}, dial }), json.errors.join(" ")).toBe(0);
        got.push(`wsp ${list} on ${mode}: ${said(plain.lines.join("\n"), JSON.parse(json.lines.at(-1)!) as Record<string, unknown>)}`);
        if (!("tool" in verb)) throw new Error(`${list} has no tool`);
        const deps = { statePath: join(dir, "state.json"), env: {}, client: dial } as unknown as VerbDeps;
        const answer = (await verb.tool.call({ thread: "t_landing" } as never, deps)) as { content: { text: string }[]; structuredContent: Record<string, unknown> };
        got.push(`${list} tool on ${mode}: ${said(answer.content[0]!.text, answer.structuredContent)}`);
      }
    }
    expect(got).toEqual(modes.flatMap(([mode, word]) => LISTS.flatMap(list => [`wsp ${list} on ${mode}: ${tail(word)} (${mode})`, `${list} tool on ${mode}: ${tail(word)} (${mode})`])));
  });
});
