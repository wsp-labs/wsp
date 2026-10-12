// SPDX-License-Identifier: AGPL-3.0-only
// Settings > Computers > Boat: the plan line and today's starts under the name,
// and Limits leading with machines at once, spend per day, the size a
// sub-thread gets and how long a finished one's machine stays, each saved
// through the host and taken back to its default with the row's arrow.
import { act, cleanup, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_PREFERENCES, placeSettingDropped, type SessionView, type WorkspaceView, type InitSetup, type PlaceSettings, type PlaceSettingsAsk, type PlaceSettingWord, type PlaceView } from "@wsp/protocol";
import { useStore } from "../src/protocol/store.js";
import { COMPUTER_PAGE_WORDS as W } from "../src/settings/format.js";
import { pickOption } from "./select.js";
import { mountSettings, resetSettings, settingsApi, settle } from "./settings-harness.js";

const MIN = 60_000;
const T0 = Date.parse("2026-10-12T08:00:00.000Z");
const SMALL = { cpu: 2, memMb: 4096 };
const DEFAULTS = { cap: { machines: 3, spendPerDayUsd: 10 }, size: SMALL, deleteAfterMs: 30 * MIN };
const here: PlaceView = { id: "here", kind: "computer", name: "zingzy-mbp", label: "zingzy's MacBook Pro", default: true, present: true, takesForks: false };
const setup = { keys: { box: true, solari: false }, home: "/Users/dev", agents: [], pricing: null, job: null } as unknown as InitSetup;

/** Boat as places.list reads it on the $20 plan, with the person's settings applied over its defaults. */
function boatRow(settings: PlaceSettings = {}, plan: Partial<NonNullable<PlaceView["plan"]>> = {}): PlaceView {
  return {
    id: "box",
    kind: "provider",
    name: "box",
    default: false,
    takesForks: true,
    rateUsdPerHour: 0.018,
    sizes: [
      { name: "small", cpu: 2, memMb: 4096, rateUsdPerHour: 0.018 },
      { name: "default", cpu: 4, memMb: 8192, rateUsdPerHour: 0.036 },
      { name: "large", cpu: 8, memMb: 16384, rateUsdPerHour: 0.072 },
      { name: "extra large", cpu: 16, memMb: 32768, rateUsdPerHour: 0.2 },
    ],
    cap: { machines: settings.machines ?? 3, spendPerDayUsd: settings.spendPerDayUsd ?? 10 },
    capDefault: DEFAULTS.cap,
    settings,
    plan: { key: "box_20", trial: false, machinesAtOnce: 100, sizes: ["2x4", "4x8", "8x16"], running: 7, ...plan },
    starts: { minute: { used: 1, limit: 12 }, hour: { used: 4, limit: 60 }, day: { used: 187, limit: 200 } },
    size: settings.size ?? SMALL,
    sizeDefault: SMALL,
    deleteAfterMs: settings.deleteAfterMs ?? DEFAULTS.deleteAfterMs,
    napMs: 20 * MIN,
    napDefault: 20 * MIN,
    turnLimitMs: 6 * 60 * MIN,
    turnLimitDefault: 6 * 60 * MIN,
  } as PlaceView;
}

let stored: PlaceSettings;
let asked: Array<[string, PlaceSettingsAsk, readonly PlaceSettingWord[]]>;

beforeEach(() => {
  resetSettings();
  stored = {};
  asked = [];
  useStore.setState({ preferences: { ...DEFAULT_PREFERENCES, labs: false }, places: [here, boatRow()] });
});
afterEach(cleanup);

async function openBoat(): Promise<Element> {
  const { api } = settingsApi({
    initGet: async () => setup,
    placesSet: async (placeId, ask, reset = []) => {
      asked.push([placeId, ask, reset]);
      stored = reset.reduce((at, word) => placeSettingDropped(at, word), { ...stored, ...ask } as PlaceSettings);
      return boatRow(stored);
    },
  });
  mountSettings({ api, at: { kind: "computer", id: "box" } });
  await settle();
  return document.querySelector("[data-settings-page]")!;
}

const row = (id: string): Element => document.querySelector(`[data-settings-page] [data-settings-row=${id}]`)!;
const reset = (id: string): Element | null => row(id).querySelector("[data-k=row-reset]");

describe("Boat's page", () => {
  it("heads with the plan line from Boat's limits read and today's starts under it, and a trial's few figures", async () => {
    const page = await openBoat();
    expect(page.querySelector("[data-k=cloud-plan]")?.textContent).toBe("$20 plan: 100 machines at once, 12 starts a minute, 60 an hour, 200 a day");
    expect(page.querySelector("[data-k=cloud-starts]")?.textContent).toBe("Starts today: 187 of 200");
    cleanup();
    resetSettings();
    useStore.setState({ places: [here, { ...boatRow({}, { key: "trial", trial: true, machinesAtOnce: 2, sizes: ["2x4", "4x8"] }), starts: { minute: { used: 0, limit: 5 }, hour: { used: 0, limit: 25 }, day: { used: 3, limit: 75 } } }] });
    const trial = await openBoat();
    expect(trial.querySelector("[data-k=cloud-plan]")?.textContent).toBe("Trial: 2 machines at once, 75 starts a day, small and default sizes");
  });

  it("leads Limits with Boat's four rows, then the nap and the turn limit", async () => {
    await openBoat();
    const titles = [...document.querySelectorAll("[data-settings-page] [data-settings-card=computer-limits] [data-settings-row] [data-settings-title]")].map(t => t.textContent);
    expect(titles).toEqual([W.machinesTitle, W.spendTitle, W.sizeTitle, W.deleteTitle, W.napTitle, W.turnLimitTitle]);
    expect(row("machines-at-once").querySelector("[data-settings-description]")?.textContent).toBe("How many machines may run on Boat at once. A sub-thread past it waits for one to free. Boat's plan allows 100.");
    expect(row("spend-per-day").querySelector("[data-settings-description]")?.textContent).toBe("What Boat may spend in a day before no new machine starts. Machines already running are left alone.");
  });

  it("steps machines at once through the host up to the plan's cap, and takes it back to 3", async () => {
    await openBoat();
    expect(row("machines-at-once").querySelector("[data-k=machines-at-once-value]")?.textContent).toBe("3");
    expect(reset("machines-at-once")).toBeNull();
    await act(async () => fireEvent.click(row("machines-at-once").querySelector(`[aria-label="${W.more}"]`)!));
    await settle();
    expect(asked.at(-1)).toEqual(["box", { machines: 4 }, []]);
    expect(row("machines-at-once").querySelector("[data-k=machines-at-once-value]")?.textContent).toBe("4");
    await act(async () => fireEvent.click(reset("machines-at-once")!));
    expect(asked.at(-1)).toEqual(["box", {}, ["machines"]]);
    await waitFor(() => expect(reset("machines-at-once")).toBeNull());
    cleanup();
    resetSettings();
    useStore.setState({ places: [here, boatRow({ machines: 100 })] });
    await openBoat();
    expect(row("machines-at-once").querySelector<HTMLButtonElement>(`[aria-label="${W.more}"]`)!.disabled).toBe(true);
  });

  it("saves the spend per day typed in dollars on Enter, leaves a word that is no figure, and takes it back to $10", async () => {
    await openBoat();
    const field = row("spend-per-day").querySelector<HTMLInputElement>("[data-k=spend-per-day]")!;
    expect(field.value).toBe("10");
    fireEvent.change(field, { target: { value: "lots" } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(asked).toEqual([]);
    fireEvent.change(field, { target: { value: "25" } });
    await act(async () => void fireEvent.keyDown(field, { key: "Enter" }));
    await settle();
    expect(asked.at(-1)).toEqual(["box", { spendPerDayUsd: 25 }, []]);
    await act(async () => fireEvent.click(reset("spend-per-day")!));
    expect(asked.at(-1)).toEqual(["box", {}, ["spend"]]);
  });

  it("offers only the sizes the plan sells, each with its rate, saves the one picked and takes it back to small", async () => {
    await openBoat();
    const trigger = row("sub-size").querySelector("[data-k=sub-size]")!;
    expect(trigger.textContent).toBe("Small, 2 vCPU, 4 GB");
    const offered = await pickOption(trigger, "Default, 4 vCPU, 8 GB, $0.036/hr");
    expect(offered).toEqual(["Small, 2 vCPU, 4 GB, $0.018/hr", "Default, 4 vCPU, 8 GB, $0.036/hr", "Large, 8 vCPU, 16 GB, $0.072/hr"]);
    await waitFor(() => expect(asked.at(-1)).toEqual(["box", { size: { cpu: 4, memMb: 8192 } }, []]));
    await waitFor(() => expect(reset("sub-size")).not.toBeNull());
    await act(async () => fireEvent.click(reset("sub-size")!));
    expect(asked.at(-1)).toEqual(["box", {}, ["size"]]);
  });

  it("names each size by the name the cloud's row carries for it", async () => {
    useStore.setState({ places: [here, { ...boatRow(), sizes: [{ cpu: 2, memMb: 4096, rateUsdPerHour: 0.018, name: "tiny" }, { cpu: 4, memMb: 8192, rateUsdPerHour: 0.036, name: "medium" }] }] });
    await openBoat();
    expect(await pickOption(row("sub-size").querySelector("[data-k=sub-size]")!, "Medium, 4 vCPU, 8 GB, $0.036/hr")).toEqual(["Tiny, 2 vCPU, 4 GB, $0.018/hr", "Medium, 4 vCPU, 8 GB, $0.036/hr"]);
  });

  it("offers At once up to 3 hours for a finished sub-thread's machine, saves the one picked, and takes it back to 30 minutes", async () => {
    await openBoat();
    const trigger = row("delete-after").querySelector("[data-k=delete-after]")!;
    expect(trigger.textContent).toBe("30 minutes");
    const offered = await pickOption(trigger, "1 hour");
    expect(offered).toEqual(["At once", "10 minutes", "30 minutes", "1 hour", "2 hours", "3 hours"]);
    await waitFor(() => expect(asked.at(-1)).toEqual(["box", { deleteAfterMs: 60 * MIN }, []]));
    await pickOption(row("delete-after").querySelector("[data-k=delete-after]")!, "At once");
    await waitFor(() => expect(asked.at(-1)).toEqual(["box", { deleteAfterMs: 0 }, []]));
    await waitFor(() => expect(reset("delete-after")).not.toBeNull());
    await act(async () => fireEvent.click(reset("delete-after")!));
    expect(asked.at(-1)).toEqual(["box", {}, ["delete-after"]]);
  });
});

describe("Boat's row on the Computers list", () => {
  const band = (): Element => document.querySelector("[data-settings-page] [data-place-row=box] [data-limit-band]")!;
  const columns = (): Array<[string | undefined, string | null | undefined, string | undefined]> =>
    [...band().querySelectorAll<HTMLElement>("[data-limit-column]")].map(c => [c.querySelector("span")?.textContent ?? undefined, c.querySelector("[data-k^=limit-]")?.textContent, c.querySelector("[data-k^=limit-]")?.className.match(/text-(warning|destructive)-foreground/)?.[1]]);

  async function openList(row: PlaceView): Promise<void> {
    useStore.setState({ places: [here, row] });
    const { api } = settingsApi({ initGet: async () => setup });
    mountSettings({ api, at: { kind: "group", group: "computers" } });
    await settle();
  }

  it("draws Boat's plan panel under its name: the machines and the three windows of starts against their limits, in neutral ink", async () => {
    await openList({ ...boatRow(), starts: { minute: { used: 1, limit: 12 }, hour: { used: 4, limit: 60 }, day: { used: 66, limit: 200 } } });
    expect(columns()).toEqual([
      ["Machines at once", "7 of 100", undefined],
      ["Starts this minute", "1 of 12", undefined],
      ["Starts this hour", "4 of 60", undefined],
      ["Starts today", "66 of 200", undefined],
    ]);
    expect(band().querySelector("p")?.textContent).toBe("Starts count over rolling windows; a create, a fork and a wake each count as one.");
  });

  it("inks a window in the warning ink from nine tenths and in the danger ink at its limit", async () => {
    await openList(boatRow());
    useStore.setState({ places: [here, { ...boatRow(), starts: { minute: { used: 12, limit: 12 }, hour: { used: 41, limit: 60 }, day: { used: 187, limit: 200 } } }] });
    await settle();
    expect(columns().map(([label, , ink]) => [label, ink])).toEqual([
      ["Machines at once", undefined],
      ["Starts this minute", "destructive"],
      ["Starts this hour", undefined],
      ["Starts today", "warning"],
    ]);
  });

  it("says the credit left and the day's spend as words in the row's note, with no bar", async () => {
    await openList({ ...boatRow(), creditHours: 41.5, spentTodayUsd: 1.2 });
    expect(document.querySelector("[data-settings-page] [data-place-row=box] [data-grid-note]")?.textContent).toBe("42 h of credit left, $1.20 of $10 today");
  });
});

describe("Boat's Remove", () => {
  const sub = (id: string, title: string, phase: "napping" | "released", held?: { commits: number; files: number }) => ({
    workspace: { id, name: title, kind: "cloud", provider: "box", place: "box", machineId: phase === "released" ? "" : `bx_${id}`, project: { id: "pr_web", name: "web", path: "/root/web", computer: "here" }, phase, golden: "g", createdAt: "2026-10-12T08:00:00Z" } as unknown as WorkspaceView,
    row: { id: `s_${id}`, threadId: `th_${id}`, workspaceId: id, harness: "claude", status: "completed", prompt: title, harnessTitle: title, startedBy: "agent", parentThreadId: "th_lead", startedAt: T0, endedAt: T0 + MIN, sub: { state: phase, on: "box", machine: title, size: SMALL, branch: `wsp/${id}`, ...(held === undefined ? {} : { held }) } } as SessionView,
  });

  it("counts the machines still standing, keeps the sub-threads, and names the ones whose work goes with their machines by their titles", async () => {
    const subs = [sub("fix", "Fix the flaky upload test", "napping", { commits: 2, files: 3 }), sub("cap", "Bump the upload size cap", "napping", { commits: 1, files: 0 }), sub("events", "Upload progress events", "released")];
    useStore.setState({ places: [here, boatRow()], workspaces: subs.map(x => x.workspace), sessions: Object.fromEntries(subs.map(x => [x.workspace.id, [x.row]])) });
    const { api } = settingsApi({ initGet: async () => setup, placeHolds: async () => ({ forks: [], projects: [], unsaved: ["e17a90c3 holds 2 commits on wsp/fix", "9b04d1f2 holds 1 commit on wsp/cap"] }) } as never);
    mountSettings({ api, at: { kind: "computer", id: "box" } });
    await settle();
    fireEvent.click(document.querySelector("[data-settings-page] [data-k='remove']")!);
    expect(document.querySelector("[data-k='remove-sentence']")?.textContent).toBe("Its 2 machines are deleted at Boat and the key is forgotten on zingzy's MacBook Pro. The 3 sub-threads stay with their transcripts, and a message to one asks for Boat again.");
    await waitFor(() => expect(document.querySelector("[data-k='unsaved']")).not.toBeNull());
    const note = document.querySelector("[data-k='unsaved']")!.textContent!;
    expect(note).toMatch(/^Work not pushed goes with their machines: (Fix the flaky upload test, Bump the upload size cap|Bump the upload size cap, Fix the flaky upload test)\. Ask each to push first\.$/);
    expect(document.querySelector("[data-k='remove-confirm']")?.textContent).toBe("Remove anyway");
  });
});
