// SPDX-License-Identifier: AGPL-3.0-only
// A sidebar row names a nap off the computer its own machine stands on, which a
// sub-thread on another cloud does not share with its project.
import { renderHook } from "@testing-library/react";
import type { WorkspaceView } from "@wsp/protocol";
import { afterEach, describe, expect, it } from "vitest";
import { useSidebarProjects, useStore } from "../src/protocol/store.js";
import { caps } from "./caps.js";

const PROJECT = { id: "pr_1", name: "web", path: "/root/web", computer: "solari" };
const record = (id: string, over: Partial<WorkspaceView> = {}): WorkspaceView => ({ id, name: id, machineId: `m_${id}`, project: PROJECT, phase: "napping", golden: "snap_g", createdAt: "2026-09-01T00:00:00Z", ...over });

afterEach(() => useStore.setState({ workspaces: [], statuses: {}, landings: {}, capabilities: null }));

describe("a sidebar row's nap word", () => {
  it("a Boat sub-thread of a project on a cloud that keeps the memory reads Stopped, its lead Paused", () => {
    const lead = record("ws_lead", { pauseMode: "memory" });
    const kid = record("ws_kid", { parentThreadId: "thr_lead", rootThreadId: "thr_lead", place: "box", provider: "box", pauseMode: "disk" });
    useStore.setState({ capabilities: caps(), landings: { [PROJECT.id]: { name: "Solari", capabilities: caps() } }, workspaces: [lead, kid] });
    const { result } = renderHook(() => useSidebarProjects());
    expect(Object.fromEntries(result.current.map(p => [p.id, p.indicator.label]))).toEqual({ ws_lead: "Paused", ws_kid: "Stopped" });
  });
});
