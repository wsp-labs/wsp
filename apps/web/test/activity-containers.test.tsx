// SPDX-License-Identifier: AGPL-3.0-only
// A computer's containers in Activity: each under the thread the daemon placed it
// with, or under No thread with its compose folder named, its own memory and CPU
// counted in its group's figures, and End, which asks the daemon to stop it.
import { cleanup, render, waitFor } from "@testing-library/react";
import { fromMenu } from "./row-menu.js";
import { afterEach, describe, expect, it } from "vitest";
import { applyProcChanges, fmtUptime, type ProcChanges, type ProcContainer, type ProcEntry, type ProcSnapshot } from "@wsp/protocol";
import { filterGroups, groupSpent, NO_PICKS, scopeOf, sortGroups, totalSpent, type ActivityContainer, type ActivityThread } from "../src/settings/activity/model.js";
import { ThreadsTable, type Acts, type Look } from "../src/settings/activity/tables.js";

afterEach(cleanup);

const NOW = 1_791_777_900_000;
const MIB = 1024 * 1024;
const proc = (pid: number, ppid: number, comm: string, extra: Partial<ProcEntry> = {}): ProcEntry => ({ pid, ppid, user: "me", state: "S", comm, cmdline: comm, cpu: 0, rss: 0, startedAt: 0, ...extra });
const thread = (id: string, extra: Partial<ActivityThread> = {}): ActivityThread => ({ id, title: `Thread ${id}`, harness: "claude", project: "lab", workspaceId: "ws_lab", sessionId: `s_${id}`, working: false, ...extra });
const id = (c: string): string => c.repeat(64);

const CONTAINERS: ProcContainer[] = [
  { id: id("a"), name: "lab-db-1", image: "postgres:17", state: "running", startedAt: NOW - 180_000, thread: "t1", placedBy: "compose", folder: "/srv/acme/lab", project: "lab", memory: 155 * MIB, cpu: 0.4 },
  { id: id("b"), name: "scratch", image: "busybox:1.37", state: "running", startedAt: NOW - 5_000, thread: "t2", placedBy: "label", memory: 2 * MIB },
  { id: id("c"), name: "other-web-1", image: "nginx:1.29-alpine", state: "running", folder: "/srv/other", project: "other", memory: 12 * MIB, cpu: 0.1 },
  // A label naming a thread this page does not know stands under No thread too.
  { id: id("d"), name: "left-over", image: "busybox:1.37", state: "running", thread: "t_gone", placedBy: "label" },
];
const snap = (procs: ProcEntry[], containers?: ProcContainer[]): ProcSnapshot => ({ type: "proc.snapshot", at: 1, daemon: 20, total: procs.length, procs, seq: 1, ...(containers === undefined ? {} : { containers }) });
const PROCS = [proc(1, 0, "launchd"), proc(20, 1, "wsp-daemon"), proc(30, 1, "claude", { rss: 400 * MIB, cpu: 2 })];

describe("where Activity puts a container", () => {
  const scope = scopeOf({ snapshot: snap(PROCS, CONTAINERS), threads: [thread("t1", { pid: 30 }), thread("t2")], ports: new Map(), now: NOW });
  const byId = new Map(scope.groups.map(g => [g.id, g]));

  it("under the thread the daemon named, a thread with no process of its own here included, else under No thread", () => {
    expect(scope.groups.map(g => [g.id, g.containers.map(c => c.name)])).toEqual([
      ["wsp", []],
      ["t1", ["lab-db-1"]],
      ["t2", ["scratch"]],
      ["loose", ["other-web-1", "left-over"]],
    ]);
    expect(byId.get("loose")!.containers[0]).toEqual<ActivityContainer>({ id: id("c"), name: "other-web-1", image: "nginx:1.29-alpine", upMs: null, mem: 12 * MIB, cpu: 0.1, folder: "/srv/other", placedBy: null, owned: false });
    expect(byId.get("t1")!.containers[0]!.upMs).toBe(180_000);
  });

  it("counts a container's own memory and CPU in its group's figures and the page's totals, and no process twice", () => {
    expect(groupSpent(byId.get("t1")!)).toMatchObject({ mem: 400 * MIB + 155 * MIB, cpu: 2.4, count: 1 });
    expect(groupSpent(byId.get("t2")!)).toMatchObject({ mem: 2 * MIB, cpu: 0, count: 0 });
    expect(totalSpent(scope.groups).mem).toBe((400 + 155 + 2 + 12) * MIB);
    // The heaviest thread first, by what its containers hold too.
    expect(sortGroups(scope.groups, "mem").map(g => g.id)).toEqual(["wsp", "t1", "t2", "loose"]);
  });

  it("lets a filter find a container by its name, image or folder, and a kind chip drop it", () => {
    expect(filterGroups(scope.groups, "postgres", NO_PICKS).map(g => [g.id, g.containers.map(c => c.name)])).toEqual([["t1", ["lab-db-1"]]]);
    expect(filterGroups(scope.groups, "/srv/other", NO_PICKS).map(g => [g.id, g.containers.map(c => c.name)])).toEqual([["loose", ["other-web-1"]]]);
    expect(filterGroups(scope.groups, "", { ...NO_PICKS, kind: "dev" }).flatMap(g => g.containers)).toEqual([]);
  });

  it("holds the containers a changes frame carried, and those of the snapshot where it carried none", () => {
    const held = snap(PROCS, CONTAINERS);
    const changes = (containers?: ProcContainer[]): ProcChanges => ({ type: "proc.changes", at: 3, daemon: 20, total: 3, procs: [], gone: [], seq: 2, base: 1, ...(containers === undefined ? {} : { containers }) });
    expect(applyProcChanges(held, changes())!.containers).toEqual(CONTAINERS);
    expect(applyProcChanges(held, changes([]))!.containers).toEqual([]);
    expect(applyProcChanges(snap(PROCS), changes())!.containers).toBeUndefined();
  });
});

describe("a container's row", () => {
  const look: Look = { sort: "mem", onSort: () => {}, energy: true };
  const acts = (endContainer: (c: ActivityContainer) => void, stopping: ReadonlySet<string> = new Set()): Acts => ({ ending: new Set(), end: () => {}, stop: () => {}, openPort: () => {}, openThread: () => {}, copy: () => {}, endAll: () => {}, workspaceOf: () => "ws_lab", stopping, endContainer, free: () => {} });
  const scope = scopeOf({ snapshot: snap(PROCS, CONTAINERS), threads: [thread("t1", { pid: 30 }), thread("t2")], ports: new Map(), now: NOW });

  it("names its image and time up, under No thread its folder, carries its own figures, and End in its menu asks for its stop", async () => {
    const asked: string[] = [];
    const { container } = render(<ThreadsTable name="Here" groups={scope.groups} statusOf={() => undefined} ageOf={() => undefined} box cost={null} filtering acts={acts(c => asked.push(c.id))} look={look} />);
    const row = (c: string) => container.querySelector<HTMLElement>(`[data-container-row="${id(c)}"]`)!;
    const facts = (c: string) => [...row(c).querySelectorAll("[data-container-facts] > span")].map(f => f.textContent);
    expect(facts("a")).toEqual(["postgres:17", `Up ${fmtUptime(180_000)}`]);
    expect(facts("c")).toEqual(["nginx:1.29-alpine", "/srv/other"]);
    expect(row("a").querySelector('[data-col="mem"]')!.textContent).toBe("155 MB");
    expect(row("a").querySelector('[data-col="cpu"]')!.textContent).toBe("0.4%");
    // A container not read yet holds no figure, never a zero it does not have.
    expect(row("d").querySelector('[data-col="mem"]')!.textContent).toBe("");
    expect(row("a").closest("[data-activity-group]")!.getAttribute("data-activity-group")).toBe("t1");
    expect(row("c").closest("[data-activity-group]")!.getAttribute("data-activity-group")).toBe("loose");
    expect(row("b").querySelector('[data-act="end-container"]')).toBeNull();
    await fromMenu(row("b"), "end");
    await waitFor(() => expect(asked).toEqual([id("b")]));
  });

  it("shows the spinner in place of its menu while its stop is under way", () => {
    const { container } = render(<ThreadsTable name="Here" groups={scope.groups} statusOf={() => undefined} ageOf={() => undefined} box cost={null} filtering acts={acts(() => {}, new Set([id("b")]))} look={look} />);
    const row = container.querySelector<HTMLElement>(`[data-container-row="${id("b")}"]`)!;
    expect(row.querySelector('[data-act="menu"]')).toBeNull();
    expect(row.textContent).toContain("Ending");
  });
});
