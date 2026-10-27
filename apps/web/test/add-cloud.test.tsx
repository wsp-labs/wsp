// SPDX-License-Identifier: AGPL-3.0-only
// Add a cloud provider, the dialog: the key checked live with the provider's
// refusals in plain words, and Continue saving it.
import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_PREFERENCES, KEY_REFUSED, KEY_UNCHECKED, type Preferences, type InitSetup, type PlaceView } from "@wsp/protocol";
import { RequestError, type Api } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { useComposerDraftStore } from "../src/components/chat/composerDraftStore.js";
import { closeCloudAdd, useCloudFlow } from "../src/settings/add/cloudFlow.js";
import { ADD_COMPUTER_WORDS } from "../src/settings/format.js";
import { mountSettings, resetSettings, settingsApi, settle } from "./settings-harness.js";

const here: PlaceView = { id: "here", kind: "computer", name: "acme-laptop", default: true, present: true, takesForks: false, buildsImages: false };
const boat: PlaceView = { id: "box", kind: "provider", name: "box", default: false, rateUsdPerHour: 0.018, takesForks: true, buildsImages: true };
const setupOf = (keys: Record<string, boolean>): InitSetup => ({ keys, home: "/Users/dev", pricing: null }) as InitSetup;

/** A host as the dialog reaches it: the key check answering per key, every save kept. */
function host(over: Partial<Api> = {}, record: Preferences = { ...DEFAULT_PREFERENCES, labs: false }) {
  const asked: { op: string; o?: unknown }[] = [];
  const fake = settingsApi({
    initGet: async () => setupOf({ box: false }),
    providersCheckKey: async ({ key }: { provider: string; key: string }) => {
      asked.push({ op: "check", o: key });
      if (key === "boat_wrong") throw new RequestError("Boat refused this key. Check you copied all of it.", KEY_REFUSED);
      if (key === "boat_no_read") throw new RequestError("This key cannot list machines. Give it sandbox.read and the other actions above.", KEY_REFUSED);
      if (key === "boat_away") throw new RequestError("wsp cannot reach Boat right now. Try again.", KEY_UNCHECKED);
      return { works: true as const, account: "acme", plan: "$20 plan", running: 2, limit: 100 };
    },
    initKeys: async (o: unknown) => (asked.push({ op: "keys", o }), setupOf({ box: true })),
    placesList: async () => ({ places: [here, boat], adds: [] }),
    ...over,
  } as unknown as Partial<Api>, record);
  return { ...fake, asked };
}

const dialog = (): HTMLElement | null => document.querySelector<HTMLElement>("[data-add-cloud]");
const stepOf = (): string | null => dialog()?.getAttribute("data-add-cloud") ?? null;
const keyCheck = (): string => document.querySelector("[data-k='key-check']")?.textContent ?? "";
const proceed = (): HTMLButtonElement => document.querySelector<HTMLButtonElement>("[data-add-cloud] [data-k='continue']")!;
const held = (b: HTMLElement): boolean => b.getAttribute("aria-disabled") === "true" || b.hasAttribute("disabled") || b.hasAttribute("data-held");
const paste = (key: string): void => {
  const field = document.querySelector<HTMLInputElement>("[data-k='cloud-key']")!;
  fireEvent.change(field, { target: { value: key } });
  fireEvent.keyDown(field, { key: "Enter" });
};

beforeEach(() => {
  resetSettings();
  closeCloudAdd();
  useComposerDraftStore.setState({ drafts: {} });
  useStore.setState({ preferences: { ...DEFAULT_PREFERENCES, labs: false }, places: [here] });
});

afterEach(() => {
  cleanup();
  closeCloudAdd();
  vi.unstubAllGlobals();
});

const openFromComputers = async (api: Api): Promise<void> => {
  mountSettings({ api, at: { kind: "group", group: "computers" } });
  await settle();
  fireEvent.click(screen.getByRole("button", { name: ADD_COMPUTER_WORDS.addCloud }));
  await settle();
};

describe("the key step", () => {
  it("says where the key is made, the ci preset's actions and the cost, and holds Continue until the key works", async () => {
    await openFromComputers(host().api);
    expect(stepOf()).toBe("key");
    expect(dialog()!.querySelector("[data-slot=dialog-title]")?.textContent).toBe("Add Boat");
    expect(document.querySelector("[data-settings-row='keys-page'] [data-settings-description]")?.textContent).toBe("boat.dev/dashboard?tab=api-keys");
    expect(document.querySelector("[data-copy-row='key-actions']")?.textContent).toContain("sandbox.read");
    expect(document.querySelector("[data-settings-row='key-cost']")?.textContent).toContain("from $0.018/h");
    expect(held(proceed())).toBe(true);
  });

  it("refuses a wrong key and a key without sandbox.read in plain words, and goes on with one that works", async () => {
    const fake = host();
    await openFromComputers(fake.api);
    paste("boat_wrong");
    await settle();
    expect(keyCheck()).toBe("Boat refused this key. Check you copied all of it.");
    expect(held(proceed())).toBe(true);
    paste("boat_no_read");
    await settle();
    expect(keyCheck()).toBe("This key cannot list machines. Give it sandbox.read and the other actions above.");
    expect(held(proceed())).toBe(true);
    paste("boat_away");
    await settle();
    expect(keyCheck()).toContain("wsp cannot reach Boat right now. Try again.");
    expect(document.querySelector("[data-k='check-again']")).not.toBeNull();
    paste("boat_good");
    await settle();
    expect(keyCheck()).toBe("Works. Account acme, $20 plan, 2 of 100 running at once.");
    expect(held(proceed())).toBe(false);
    // Typing again drops the answer: Continue waits for the next check.
    fireEvent.change(document.querySelector("[data-k='cloud-key']")!, { target: { value: "boat_good2" } });
    expect(held(proceed())).toBe(true);
  });

  it("saves the key on Continue and closes on the cloud added", async () => {
    const fake = host();
    await openFromComputers(fake.api);
    paste("boat_good");
    await settle();
    fireEvent.click(proceed());
    await settle();
    expect(fake.asked.filter(a => a.op !== "check")).toEqual([{ op: "keys", o: { provider: "box", key: "boat_good" } }]);
    expect(dialog()).toBeNull();
  });

  it("says the host's refusal of the key on the key step, and stays there", async () => {
    await openFromComputers(host({ initKeys: async () => Promise.reject(new RequestError("no place named box; you have acme-laptop", "missing")) } as unknown as Partial<Api>).api);
    paste("boat_good");
    await settle();
    fireEvent.click(proceed());
    await settle();
    expect(stepOf()).toBe("key");
    expect(document.querySelector("[data-k='key-refused']")?.textContent).toContain("no place named box; you have acme-laptop");
  });
});

describe("the provider step", () => {
  it("stands first where the host offers more than one cloud, and the key step names the one picked", async () => {
    await openFromComputers(host({ initGet: async () => setupOf({ box: false, solari: false }) } as unknown as Partial<Api>).api);
    expect(stepOf()).toBe("provider");
    expect([...document.querySelectorAll("[data-add-cloud] [data-choice]")].map(c => c.getAttribute("data-choice"))).toEqual(["box", "solari"]);
    expect(document.querySelector("[data-add-cloud] [data-k='progress']")?.textContent).toBe("1 of 2");
    fireEvent.click(document.querySelector("[data-choice='solari']")!);
    fireEvent.click(proceed());
    await settle();
    expect(stepOf()).toBe("key");
    expect(dialog()!.querySelector("[data-slot=dialog-title]")?.textContent).toBe("Add Solari");
    fireEvent.click(document.querySelector("[data-add-cloud] [data-k='back']")!);
    expect(stepOf()).toBe("provider");
    fireEvent.click(proceed());
    paste("solari_good");
    await settle();
    fireEvent.click(proceed());
    await settle();
    expect(dialog()).toBeNull();
  });
});

describe("the sidebar's cloud card", () => {
  const card = (): HTMLElement | null => document.querySelector<HTMLElement>("[data-cloud-card]");
  const mountSidebar = async (api: Api, preferences: Preferences = { ...DEFAULT_PREFERENCES, labs: false }): Promise<void> => {
    useStore.setState({ preferences, preferencesRead: true });
    mountSettings({ api });
    act(() => useStore.setState({ settingsOpen: false }));
    await settle();
  };

  it("offers a cloud while none is added, opening Add a cloud provider at its key, and stays gone once dismissed", async () => {
    const fake = host();
    await mountSidebar(fake.api);
    expect(card()?.textContent).toContain("Run threads on a cloud");
    expect(card()?.querySelector("[data-k='cloud-card-line']")?.textContent).toBe("Boat from $0.018/h.");
    fireEvent.click(card()!.querySelector("[data-k='cloud-card-add']")!);
    expect(useCloudFlow.getState()).toMatchObject({ open: true, provider: "box", step: "key" });
    act(() => closeCloudAdd());
    fireEvent.click(card()!.querySelector("[data-k='cloud-card-dismiss']")!);
    await settle();
    expect(card()).toBeNull();
    // The host keeps the dismissal, so a window that reads its record shows no card.
    expect(fake.sets).toContainEqual({ cloudCardDismissed: true });
    cleanup();
    const kept = { ...DEFAULT_PREFERENCES, labs: false, cloudCardDismissed: true };
    await mountSidebar(host({}, kept).api, kept);
    expect(card()).toBeNull();
  });

  it("shows nothing before the host's record is read, so a dismissed card never flashes", async () => {
    mountSettings({ api: host().api });
    act(() => useStore.setState({ settingsOpen: false, preferencesRead: false }));
    expect(card()).toBeNull();
  });

  it("waits while a computer's setup stands in the foot", async () => {
    useStore.setState({ places: [here, { ...here, id: "p_2", name: "hetzner", default: false, setup: { addId: "a_1", state: "running", startedAt: "2026-10-13T00:00:00Z", steps: [{ step: "agents", state: "running" }], waiting: [] } } as PlaceView] });
    await mountSidebar(host().api);
    expect(document.querySelector("[data-setup-card='p_2']")).not.toBeNull();
    expect(card()).toBeNull();
  });
});
