// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's row in the sidebar, one kind per state of its machine on Boat
// as spec 07's table says: Waiting, Starting, Working, Not pushed and Done,
// each with the lines its card opens to, the machine said where the computer's
// name would be.
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaceView, SubRow } from "@wsp/protocol";
import { threadIndicator, type SidebarThreadSnapshot } from "../src/adapt/index.js";
import { SidebarProvider } from "../src/components/ui/sidebar.js";
import { useStore } from "../src/protocol/store.js";
import { ThreadTile, type TilePlace } from "../src/sidebar/ThreadTile.js";
import { threadSection } from "../src/sidebar/Sidebar.logic.js";
import { APP_LOCALE } from "../src/lib/timestampFormat.js";

const MIN = 60_000;
const BRANCH = "wsp/fix-the-flaky-upload-test";
const HERE: PlaceView = { id: "here", kind: "computer", name: "zingzy-mbp", label: "zingzy's MacBook Pro", mac: "macbook", default: true, present: true, takesForks: false };
const PLACE: TilePlace = { projectId: "pr_web", project: "web", computer: "Boat" };
const sub = (over: Partial<SubRow>): SubRow => ({ state: "running", on: "box", machine: "Fix the flaky upload test", size: { cpu: 2, memMb: 4096, name: "small" }, rateUsdPerHour: 0.018, branch: BRANCH, ...over });

const thread = (over: Partial<SidebarThreadSnapshot>): SidebarThreadSnapshot => {
  const base = { id: "th_1", threadId: "th_1", sessionId: "s_1", workspaceId: "ws_fix", title: "Fix the flaky upload test", status: "running" as const, ran: true, startedAt: "2026-10-12T08:00:00.000Z", endedAt: null, harness: "claude", startedBy: "agent" as const, project: "web", parentThreadId: "th_lead", attempt: null, model: null, asking: null, costUsd: null, unread: false, readAt: null, settledAt: null, needsYou: false, pinnedAt: null, order: null, snoozedUntil: null, section: null, subagents: [], lastLine: null, failure: null, foldedAt: null, replaces: null, replacedBy: null };
  const merged = { ...base, ...over };
  return { ...merged, indicator: threadIndicator({ status: merged.status }) };
};

function mount(over: Partial<SidebarThreadSnapshot>) {
  render(
    <SidebarProvider defaultOpen>
      <ThreadTile thread={thread(over)} place={PLACE} model={null} time="3m" depth={1} active={false} renaming={false} saving={false} onSelect={() => {}} onContextMenu={() => {}} onRename={() => {}} onRenameCancel={() => {}} />
    </SidebarProvider>,
  );
}

const slot = (): HTMLElement => document.querySelector<HTMLElement>("[data-sidebar-row] [data-thread-status]")!;

/** The card's status word and reason, its computer line, and its notes, once the pointer rested on the tile. */
async function card(): Promise<{ word: string | undefined; reason: string | null; machine: string | null; notes: string[] }> {
  const tile = document.querySelector<HTMLElement>("[data-sidebar-row]")!;
  fireEvent.pointerEnter(tile, { pointerType: "mouse" });
  fireEvent.mouseEnter(tile);
  fireEvent.mouseMove(tile);
  await act(async () => void vi.advanceTimersByTime(600));
  const line = document.querySelector<HTMLElement>("[data-tile-card] [data-status-line]");
  return {
    word: line?.dataset["statusLine"],
    reason: document.querySelector("[data-tile-card] [data-status-reason]")?.textContent ?? null,
    machine: document.querySelector('[data-tile-card] [data-tile-card-line="computer"]')?.textContent ?? null,
    notes: [...document.querySelectorAll<HTMLElement>('[data-tile-card] [data-tile-card-line="note"]')].map(note => note.textContent ?? ""),
  };
}

beforeEach(() => {
  vi.useFakeTimers({ now: Date.parse("2026-10-12T08:13:00.000Z") });
  useStore.setState({ places: [HERE], costs: { ws_fix: { rateUsdPerHour: 0.018, accruedUsd: 0.004, at: new Date().toISOString() } } });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  useStore.setState({ places: [], costs: {} });
});

describe("a sub-thread holding work not pushed", () => {
  it("files under Needs you, read or not, since its machine costs money until someone acts", () => {
    expect(threadSection({ status: "completed", asking: null, unread: false, sub: { held: { commits: 2, files: 3 } } })).toBe("needs-you");
    expect(threadSection({ status: "running", asking: null, unread: false, sub: { held: { commits: 2, files: 3 } } })).toBe("working");
    expect(threadSection({ status: "completed", asking: null, unread: false, sub: {} })).toBe("idle");
  });
});

describe("a sub-thread's row reads its machine", () => {
  it("Waiting while Boat's machines at once hold its spawn, the hourglass and no crab, the card naming the count and the raise", async () => {
    mount({ capped: { placeId: "box", place: "Boat", running: 3, atOnce: 3, why: "slot" }, sub: sub({ state: "waiting" }) });
    expect(slot().dataset["threadStatus"]).toBe("waiting");
    expect(slot().querySelector("svg")!.getAttribute("class")).toContain("lucide-hourglass");
    expect(document.querySelector("[data-sidebar-row] [data-crab]")).toBeNull();
    const seen = await card();
    expect(seen.word).toBe("waiting");
    expect(seen.reason).toBe("Waiting for a machine on Boat: 3 of 3 running");
    expect(seen.machine).toBe("Boat, small, 2 vCPU, 4 GB, $0.018/hr");
    expect(seen.notes).toEqual(["Raise machines at once on Boat in Settings to start it now"]);
  });

  it("Waiting for Boat's next minute of starts, with no raise, since no setting moves that limit", async () => {
    mount({ capped: { placeId: "box", place: "Boat", running: 12, atOnce: 12, why: "minute" }, sub: sub({ state: "waiting" }) });
    expect(slot().dataset["threadStatus"]).toBe("waiting");
    const seen = await card();
    expect(seen.reason).toBe("Waiting for Boat's next minute of starts: 12 of 12 used");
    expect(seen.notes).toEqual([]);
  });

  it("Starting with the crab while Boat makes its machine, the card naming the size and the rate", async () => {
    mount({ sub: sub({ state: "starting" }) });
    expect(slot().dataset["threadStatus"]).toBe("starting");
    expect(document.querySelector("[data-sidebar-row] [data-crab]")).not.toBeNull();
    const seen = await card();
    expect(seen.reason).toBe("Starting a machine on Boat (small, 2 vCPU, 4 GB)");
    expect(seen.machine).toBe("Boat, small, 2 vCPU, 4 GB, $0.018/hr, $0.004 so far");
  });

  it("Starting too for a send into a released sub-thread, whose row is still its last finished turn's", () => {
    mount({ status: "completed", endedAt: "2026-10-12T07:00:00.000Z", sub: sub({ state: "starting", releasedAt: Date.now() - 30 * MIN }) });
    expect(slot().dataset["threadStatus"]).toBe("starting");
    cleanup();
    mount({ status: "failed", endedAt: "2026-10-12T08:12:00.000Z", sub: sub({ state: "starting" }) });
    expect(slot().dataset["threadStatus"]).toBe("failed");
  });

  it("names the machine's size by the name its row carries", async () => {
    mount({ sub: sub({ state: "running", size: { cpu: 4, memMb: 8192, name: "medium" } }) });
    expect((await card()).machine).toBe("Boat, medium, 4 vCPU, 8 GB, $0.018/hr, $0.004 so far");
  });

  it("keeps a released sub-thread's tile as it was when its restart failed", () => {
    const failed = { startedAt: Date.now() - MIN, machine: { since: Date.now() - MIN, states: [] }, failed: { step: "machine" as const, said: "Boat has no free machines of this size right now" }, log: [] };
    mount({ status: "completed", endedAt: "2026-10-12T07:00:00.000Z", unread: true, sub: sub({ state: "released", releasedAt: Date.now() - 40 * MIN, spawn: failed }) });
    expect(slot().dataset["threadStatus"]).toBe("done");
  });

  it("Working while its turn runs, the card adding what it has cost so far", async () => {
    mount({ sub: sub({ state: "running" }) });
    expect(slot().dataset["threadStatus"]).toBe("working");
    expect((await card()).machine).toBe("Boat, small, 2 vCPU, 4 GB, $0.018/hr, $0.004 so far");
  });

  it("Not pushed in Needs you's ink with the upload glyph while its work holds the machine, the card saying what and why it stays", async () => {
    mount({ status: "completed", endedAt: "2026-10-12T08:09:00.000Z", sub: sub({ state: "napping", held: { commits: 2, files: 3 } }) });
    expect(slot().dataset["threadStatus"]).toBe("not-pushed");
    expect(slot().dataset["tone"]).toBe("input");
    expect(slot().querySelector("svg")!.getAttribute("class")).toContain("lucide-upload");
    const seen = await card();
    expect(seen.reason).toBe(`2 commits and 3 changed files on ${BRANCH} are not pushed, so its machine stays.`);
    expect(seen.machine).toBe("Boat, small, napping, kept until its work is pushed");
  });

  it("Done once everything is pushed, the card counting down to the delete, or saying Boat would not stop it", async () => {
    mount({ status: "completed", endedAt: "2026-10-12T08:09:00.000Z", unread: true, sub: sub({ state: "napping", deleteAt: Date.now() + 18 * MIN }) });
    expect(slot().dataset["threadStatus"]).toBe("done");
    expect((await card()).machine).toBe("Boat, small, napping; its machine is deleted in 18 min");
    cleanup();
    mount({ status: "completed", endedAt: "2026-10-12T08:09:00.000Z", unread: true, sub: sub({ state: "running", deleteAt: Date.now() + 18 * MIN }) });
    expect((await card()).machine).toBe("Boat, small, running (Boat would not stop it); its machine is deleted in 18 min");
  });

  it("Released in the row's own ink with the cloud-off glyph once its machine is deleted, unless a finish nobody read says more", async () => {
    mount({ status: "completed", endedAt: "2026-10-12T07:00:00.000Z", sub: sub({ state: "released", releasedAt: Date.parse("2026-10-12T07:31:00.000Z") }) });
    expect(slot().dataset["threadStatus"]).toBe("released");
    expect(slot().dataset["tone"]).toBeUndefined();
    expect(slot().getAttribute("aria-label")).toBe("Released");
    expect(slot().querySelector("svg")!.getAttribute("class")).toContain("lucide-cloud-off");
    expect(document.querySelector("[data-thread-title]")!.className).toContain("text-sidebar-muted-foreground");
    cleanup();
    mount({ status: "completed", endedAt: "2026-10-12T07:00:00.000Z", unread: true, sub: sub({ state: "released", releasedAt: Date.parse("2026-10-12T07:31:00.000Z") }) });
    expect(slot().dataset["threadStatus"]).toBe("done");
  });

  it("a released sub-thread's card says when its machine was deleted and that a message starts another; one whose Boat left says so", async () => {
    const releasedAt = Date.parse("2026-10-12T07:31:00.000Z");
    // The app's own locale on a 24 hour clock, as every stamp it draws, never the runtime's.
    const local = vi.spyOn(Date.prototype, "toLocaleTimeString").mockReturnValue("the runtime's clock");
    mount({ status: "completed", endedAt: "2026-10-12T07:00:00.000Z", sub: sub({ state: "released", releasedAt }) });
    const at = new Intl.DateTimeFormat(APP_LOCALE, { hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(releasedAt);
    expect((await card()).machine).toBe(`Boat, its machine was deleted ${at}; a message starts a new one`);
    local.mockRestore();
    cleanup();
    mount({ status: "completed", endedAt: "2026-10-12T07:00:00.000Z", sub: sub({ state: "released", releasedAt, cloudGone: true }) });
    expect((await card()).machine).toBe("Boat was removed from zingzy's MacBook Pro");
  });
});
