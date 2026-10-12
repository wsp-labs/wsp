// SPDX-License-Identifier: AGPL-3.0-only
// A box thread's memory in Activity: its cgroup's anon and kernel bytes as the
// daemon read them, never its rows' resident figures added up, and the rows'
// own sum where the daemon read no figure for its cgroup.
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { applyProcChanges, fmtBytes, threadCgroup, type CgroupMemory, type ProcChanges, type ProcEntry, type ProcSnapshot } from "@wsp/protocol";
import { groupSpent, scopeOf, totalSpent, type ActivityThread } from "../src/settings/activity/model.js";
import { ThreadsTable, type Acts, type Look } from "../src/settings/activity/tables.js";

afterEach(cleanup);

const MIB = 1024 * 1024;
const T1 = threadCgroup("t1");
const T2 = threadCgroup("t2");
const proc = (pid: number, ppid: number, comm: string, extra: Partial<ProcEntry> = {}): ProcEntry => ({ pid, ppid, user: "dev", state: "S", comm, cmdline: comm, cpu: 0, rss: 0, startedAt: 0, ...extra });
const thread = (id: string): ActivityThread => ({ id, title: `Thread ${id}`, harness: "claude", project: "lab", workspaceId: "ws_lab", sessionId: `s_${id}`, working: false, cgroup: threadCgroup(id) });
// Two node processes of t1 sharing their binary, and one of t2, on a box whose t1 cgroup the daemon read.
const PROCS = [
  proc(1, 0, "systemd"),
  proc(20, 1, "wsp-daemon"),
  proc(30, 1, "claude", { cgroup: T1, rss: 300 * MIB, cpu: 2 }),
  proc(31, 30, "node", { cgroup: T1, rss: 40 * MIB, cpu: 1 }),
  proc(40, 1, "claude", { cgroup: T2, rss: 250 * MIB, cpu: 0.5 }),
];
const CGROUPS: CgroupMemory[] = [{ cgroup: T1, memory: 247 * MIB }];
const snap = (cgroups?: CgroupMemory[]): ProcSnapshot => ({ type: "proc.snapshot", at: 1, daemon: 20, total: PROCS.length, procs: PROCS, seq: 1, ...(cgroups === undefined ? {} : { cgroups }) });

describe("a box thread's memory", () => {
  const scope = scopeOf({ snapshot: snap(CGROUPS), threads: [thread("t1"), thread("t2")], ports: new Map() });
  const group = (id: string) => scope.groups.find(g => g.id === id)!;

  it("is its cgroup's figure where the daemon read one, else its rows' sum, and the totals add each group's", () => {
    expect(groupSpent(group("t1")).mem).toBe(247 * MIB);
    expect(groupSpent(group("t1")).cpu).toBe(3);
    expect(groupSpent(group("t2")).mem).toBe(250 * MIB);
    expect(totalSpent(scope.groups).mem).toBe(497 * MIB);
    const before = scopeOf({ snapshot: snap(), threads: [thread("t1"), thread("t2")], ports: new Map() });
    expect(groupSpent(before.groups.find(g => g.id === "t1")!).mem).toBe(340 * MIB);
  });

  it("is what the thread's row draws", () => {
    const look: Look = { sort: "mem", onSort: () => {}, energy: false };
    const acts: Acts = { ending: new Set(), end: () => {}, stop: () => {}, openPort: () => {}, openThread: () => {}, copy: () => {}, endAll: () => {}, workspaceOf: () => "ws_lab", stopping: new Set(), endContainer: () => {}, free: () => {} };
    const { container } = render(<ThreadsTable name="hetzner" groups={scope.groups} statusOf={() => undefined} ageOf={() => undefined} box cost={null} filtering={false} acts={acts} look={look} />);
    const head = container.querySelector('[data-activity-group="t1"]')!.querySelector('[data-col="mem"]')!;
    expect(head.textContent).toBe(fmtBytes(247 * MIB));
  });

  it("rides every changes frame whole, so a frame with none leaves no older tick's figure", () => {
    const changes = (cgroups?: CgroupMemory[]): ProcChanges => ({ type: "proc.changes", at: 3, daemon: 20, total: PROCS.length, procs: [], gone: [], seq: 2, base: 1, ...(cgroups === undefined ? {} : { cgroups }) });
    expect(applyProcChanges(snap(CGROUPS), changes([{ cgroup: T1, memory: 250 * MIB }]))!.cgroups).toEqual([{ cgroup: T1, memory: 250 * MIB }]);
    expect(applyProcChanges(snap(CGROUPS), changes())!.cgroups).toBeUndefined();
  });
});
