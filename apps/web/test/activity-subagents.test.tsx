// SPDX-License-Identifier: AGPL-3.0-only
// A thread's subagents in Activity: the ones running and the ones that ended in
// the last minute, whichever turn started them, each drawn under its thread with
// its title, state, time, tokens and model, and no figures or act of its own.
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { fmtDuration, type ProcEntry, type ProcSnapshot, type SubagentView } from "@wsp/protocol";
import { NO_PICKS, SUBAGENT_SHOWN_MS, filterGroups, scopeOf, subagentsOf, type ActivityThread } from "../src/settings/activity/model.js";
import { ThreadsTable, type Acts, type Look } from "../src/settings/activity/tables.js";
import { groupActions } from "../src/settings/activity/actions.js";
import { resolveActions } from "../src/actions/registry.js";

afterEach(cleanup);

const NOW = 1_791_768_900_000;
const proc = (pid: number, ppid: number, comm: string, extra: Partial<ProcEntry> = {}): ProcEntry => ({ pid, ppid, user: "me", state: "S", comm, cmdline: comm, cpu: 0, rss: 0, startedAt: 0, ...extra });
const snap = (procs: ProcEntry[]): ProcSnapshot => ({ type: "proc.snapshot", at: 1, daemon: 20, total: procs.length, procs, seq: 1 });
const thread = (id: string, extra: Partial<ActivityThread> = {}): ActivityThread => ({ id, title: `Thread ${id}`, harness: "claude", project: "lab", workspaceId: "ws_lab", sessionId: `s_${id}`, working: true, ...extra });
const tokens = (input: number, output: number) => ({ input, output, cached: 0, cacheWrite: 0, reasoning: 0 });

/** One running from the turn now going, one that ended 20 s ago in an earlier turn, and one that ended past the minute. */
const SUBAGENTS: SubagentView[] = [
  { id: "a_old", title: "Probe zero", state: "stopped", startedAt: NOW - 200_000, endedAt: NOW - SUBAGENT_SHOWN_MS - 1, model: "claude-haiku-4-5" },
  { id: "a_one", title: "Probe one", state: "done", startedAt: NOW - 40_000, endedAt: NOW - 20_000, model: "claude-sonnet-5-5", usage: { tokens: tokens(21_274, 19), toolUses: 1, durationMs: 14_370 } },
  { id: "a_two", title: "Probe two", state: "running", startedAt: NOW - 12_000, usage: { tokens: tokens(10_310, 16), toolUses: 1, durationMs: 1_129 } },
];

describe("which subagents Activity draws", () => {
  it("draws the running ones and those that ended in the last minute, whatever turn started them, with the time each ran and what its calls spent", () => {
    expect(subagentsOf(SUBAGENTS, NOW)).toEqual([
      { id: "a_one", title: "Probe one", state: "done", ms: 14_370, tokens: 21_274 + 19, model: "claude-sonnet-5-5" },
      { id: "a_two", title: "Probe two", state: "running", ms: 12_000, tokens: 10_310 + 16, model: null },
    ]);
    // An ended one with no run time of its agent's is timed by its rows; one with no spend counted has no tokens.
    expect(subagentsOf([{ id: "c", title: "Codex child", state: "done", startedAt: NOW - 9_000, endedAt: NOW - 1_000 }], NOW)).toEqual([{ id: "c", title: "Codex child", state: "done", ms: 8_000, tokens: null, model: null }]);
    expect(subagentsOf(SUBAGENTS, NOW + SUBAGENT_SHOWN_MS).map(s => s.id)).toEqual(["a_two"]);
  });

  it("keeps a thread whose agent has left the computer while a subagent of it is still drawn, and lets a filter and the kind chips reach them", () => {
    const threads = [thread("t1", { pid: 30, subagents: SUBAGENTS }), thread("t2", { subagents: [SUBAGENTS[1]!] }), thread("t3", { subagents: [SUBAGENTS[0]!] })];
    const scope = scopeOf({ snapshot: snap([proc(1, 0, "launchd"), proc(20, 1, "wsp-daemon"), proc(30, 1, "claude")]), threads, ports: new Map(), now: NOW });
    expect(scope.groups.filter(g => g.kind === "thread").map(g => [g.id, g.procs.length, g.subagents.map(s => s.id)])).toEqual([
      ["t1", 1, ["a_one", "a_two"]],
      ["t2", 0, ["a_one"]],
    ]);
    expect(filterGroups(scope.groups, "probe two", NO_PICKS).map(g => [g.id, g.procs.length, g.subagents.map(s => s.id)])).toEqual([["t1", 0, ["a_two"]]]);
    expect(filterGroups(scope.groups, "", { ...NO_PICKS, kinds: ["agent"] }).map(g => [g.id, g.subagents.length])).toEqual([["t1", 0]]);
  });
});

describe("a subagent's row under its thread", () => {
  const acts: Acts = { ending: new Set(), end: () => {}, free: () => {}, stop: () => {}, openPort: () => {}, openThread: () => {}, copy: () => {}, endAll: () => {}, workspaceOf: () => "ws_lab" };
  const look: Look = { sort: "mem", onSort: () => {}, energy: false };

  it("says its title, state, time, tokens and model, and holds no memory, CPU or End", () => {
    const scope = scopeOf({ snapshot: snap([proc(20, 1, "wsp-daemon"), proc(30, 1, "claude", { rss: 1_500 * 1024 * 1024, cpu: 15 })]), threads: [thread("t1", { pid: 30, subagents: SUBAGENTS })], ports: new Map(), now: NOW });
    const { container } = render(<ThreadsTable name="Here" groups={scope.groups} statusOf={() => undefined} ageOf={() => undefined} box={false} cost={null} filtering acts={acts} look={look} />);
    const rows = [...container.querySelectorAll<HTMLElement>("[data-subagent-row]")];
    expect(rows.map(r => r.dataset["subagentRow"])).toEqual(["a_one", "a_two"]);
    const [one, two] = rows;
    expect(one!.textContent).toContain("Probe one");
    const facts = (row: HTMLElement) => [...row.querySelectorAll("[data-subagent-facts] > span")].map(fact => fact.textContent);
    expect(facts(one!)).toEqual(["Done", fmtDuration(14_370), "21.3k tokens", "claude-sonnet-5-5"]);
    expect(facts(two!)).toEqual(["Running", fmtDuration(12_000), "10.3k tokens"]);
    for (const row of rows) {
      expect(row.querySelector("button")).toBeNull();
      for (const col of ["mem", "cpu", "energy"]) expect(row.querySelector(`[data-col="${col}"]`)!.textContent).toBe("");
    }
    // The agent's own row beside them still carries its figures.
    expect(container.querySelector('[data-proc-row="30"] [data-col="mem"]')!.textContent).not.toBe("");
  });

  it("offers no End all on a thread whose agent has left the computer and whose subagents are all it draws", () => {
    const scope = scopeOf({ snapshot: snap([proc(20, 1, "wsp-daemon")]), threads: [thread("t1", { subagents: SUBAGENTS })], ports: new Map(), now: NOW });
    const { container } = render(<ThreadsTable name="Here" groups={scope.groups} statusOf={() => undefined} ageOf={() => undefined} box={false} cost={null} filtering acts={acts} look={look} />);
    expect(container.querySelectorAll("[data-subagent-row]")).toHaveLength(2);
    expect(container.querySelector('[data-act="end-all"]')).toBeNull();
    expect(resolveActions(groupActions, scope.groups.find(g => g.id === "t1")!, acts).map(a => a.id)).toEqual(["open-thread"]);
  });
});
