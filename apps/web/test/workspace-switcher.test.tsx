// SPDX-License-Identifier: AGPL-3.0-only
// The palette's rows as buildPaletteItems makes them: where a thread runs, no
// row for a folder's record that holds no thread, Copy as Markdown, the marks a
// row wears, and the step along the open thread's level of the sidebar, named
// as the keybinding list names it and held where that level has no other thread.
import { render } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import type { PlaceView, SessionView, WorkspaceView } from "@wsp/protocol";
import { deriveSidebarProjects } from "../src/adapt/index.js";
import type { CommandPaletteActionItem } from "../src/components/palette/CommandPalette.logic.js";
import { buildPaletteItems } from "../src/components/palette/paletteItems.js";
import { KEYBINDING_WORDS } from "../src/settings/keybindingWords.js";

const view = (id: string, name: string, createdAt = "2026-09-01T00:00:00Z"): WorkspaceView => ({
  id,
  name,
  machineId: `m_${id}`,
  project: { id: "pr_1", name: "the-project", path: "/root", computer: "default" },
  phase: "running",
  golden: "snap_g",
  createdAt,
});

const session = (id: string, workspaceId: string, prompt: string, startedAt: number): SessionView => ({
  id,
  workspaceId,
  harness: "claude",
  status: "completed",
  prompt,
  startedBy: "person",
  startedAt,
  endedAt: startedAt + 60_000,
});

const WORKSPACES = [view("ws_a", "api", "2026-09-01T01:00:00Z"), view("ws_b", "web", "2026-09-01T02:00:00Z")];

describe("buildPaletteItems", () => {
  it("says where a thread on this computer runs only once the name is known, never leaving a line on a dangling on", () => {
    const here = { ...view("ws_a", "api"), kind: "local" as const, machineId: "local" };
    const lead = { ...session("s_lead", "ws_a", "The lead.", Date.parse("2026-09-01T01:00:00Z")), threadId: "thr_lead" };
    const child = { ...session("s_kid", "ws_a", "The child.", Date.parse("2026-09-01T02:00:00Z")), threadId: "thr_kid", parentThreadId: "thr_lead", startedBy: "agent" as const };
    const projects = deriveSidebarProjects({ workspaces: [here], sessions: { ws_a: [lead, child] } });
    const MAC: PlaceView = { id: "here", kind: "computer", name: "zingzy-mbp", label: "zingzy's MacBook Pro", default: true };
    const palette = (places: PlaceView[]) => buildPaletteItems({ projects, selectedId: null, query: "", messageHits: [], canCreate: false, recorded: [], picks: [], asks: false, handlers: {} as never, verbs: {} as never, places });
    const said = (node: ReactNode): HTMLElement => render(<>{node}</>).container;
    for (const item of [...palette([]).workspaceItems, ...palette([]).recentThreadItems]) expect(said(item.description).textContent).not.toMatch(/ on\s*$|this computer/);
    // The workspace and the computer are two facts a gap apart, each its own span, never a sentence joining them.
    const facts = said(palette([MAC]).recentThreadItems.find(item => item.title === "The child.")!.description);
    expect([...facts.querySelectorAll("[data-fact]")].map(fact => fact.textContent)).toEqual(["api", "zingzy's MacBook Pro"]);
  });

  it("lists no task in the palette for a folder's record on this computer that holds no thread", () => {
    const bare = { ...view("ws_a", "wsp"), kind: "local" as const, machineId: "local" };
    const held = { ...view("ws_b", "api"), kind: "local" as const, machineId: "local" };
    const projects = deriveSidebarProjects({ workspaces: [bare, held], sessions: { ws_b: [session("s_b", "ws_b", "The one.", Date.parse("2026-09-01T01:00:00Z"))] } });
    const items = buildPaletteItems({ projects, selectedId: null, query: "", messageHits: [], canCreate: false, recorded: [], picks: [], asks: false, handlers: {} as never, verbs: {} as never, places: [] });
    expect(items.workspaceItems.map(item => item.value)).toEqual(["workspace:ws_b"]);
  });

  it("offers Copy as Markdown for the thread on screen, and nothing where no thread is open", async () => {
    const copyThreadMarkdown = vi.fn(async () => {});
    const items = (copy: (() => Promise<void>) | null) =>
      buildPaletteItems({ projects: [], selectedId: null, query: "", messageHits: [], canCreate: false, recorded: [], picks: [], asks: false, handlers: { copyThreadMarkdown: copy } as never, verbs: {} as never, places: [] }).actionItems;
    const copy = items(copyThreadMarkdown).find((item): item is CommandPaletteActionItem => item.kind === "action" && item.title === "Copy as Markdown")!;
    expect(copy.disabled).not.toBe(true);
    await copy.run();
    expect(copyThreadMarkdown).toHaveBeenCalledTimes(1);
    expect(items(null).find(item => item.title === "Copy as Markdown")).toBeUndefined();
  });

  it("a thread in the palette wears the agent's mark and the one status slot, and a workspace no dot for its state", () => {
    const running = { ...session("s_run", "ws_a", "The running one.", Date.now() - 125_000), status: "running" as const, endedAt: undefined };
    const projects = deriveSidebarProjects({ workspaces: WORKSPACES, sessions: { ws_a: [running] } });
    const items = buildPaletteItems({ projects, selectedId: null, query: "", messageHits: [], canCreate: false, recorded: [], picks: [], asks: false, handlers: {} as never, verbs: {} as never, places: [] });
    const item = items.recentThreadItems.find(found => found.title === "The running one.")!;
    expect(render(<>{item.icon}</>).container.querySelector("[data-harness-mark='claude']")).not.toBeNull();
    const slot = render(<>{item.titleTrailingContent}</>).container.querySelector<HTMLElement>("[data-thread-status]")!;
    expect(slot.dataset.threadStatus).toBe("working");
    expect(slot.querySelector("[data-crab]")).not.toBeNull();
    for (const workspace of items.workspaceItems) expect(render(<>{workspace.icon}</>).container.querySelector(".rounded-full")).toBeNull();
  });

  it("names the step along the open thread's level as the keybinding list does, with its chords, and holds it where the level has no other thread", async () => {
    const nextAtLevel = vi.fn();
    const previousAtLevel = vi.fn();
    const rows = (levelThreads: number) =>
      buildPaletteItems({ projects: [], selectedId: null, levelThreads, query: "", messageHits: [], canCreate: false, recorded: [], picks: [], asks: false, handlers: { nextAtLevel, previousAtLevel } as never, verbs: {} as never, places: [] }).actionItems.filter(
        (item): item is CommandPaletteActionItem => item.kind === "action" && (item.shortcutCommand === "workspace.next" || item.shortcutCommand === "workspace.previous"),
      );
    const [next, previous] = rows(3);
    expect([next!.title, previous!.title]).toEqual([KEYBINDING_WORDS["workspace.next"], KEYBINDING_WORDS["workspace.previous"]]);
    expect(next!.disabled).not.toBe(true);
    await next!.run();
    await previous!.run();
    expect(nextAtLevel).toHaveBeenCalledTimes(1);
    expect(previousAtLevel).toHaveBeenCalledTimes(1);
    for (const alone of rows(1)) expect(alone).toMatchObject({ disabled: true, description: "No other thread at this level" });
  });
});
