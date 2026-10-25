// SPDX-License-Identifier: AGPL-3.0-only
// Settings > General's Experimental card: the desktop app's switch that hands
// it over to the GPUI app, asked of the shell, and drawn nowhere a shell cannot
// hand this computer's app over or has no GPUI app to hand it to.
import { cleanup, fireEvent, waitFor } from "@testing-library/react";
import type { DesktopBridge, PlaceView } from "@wsp/protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useStore } from "../src/protocol/store.js";
import { GENERAL_WORDS } from "../src/settings/format.js";
import { descriptionOf, mountSettings, resetSettings, rowOf, rowTitles, settingsApi, settle } from "./settings-harness.js";
import { lastNotice } from "./notice-text.js";

const here: PlaceView = { id: "here", kind: "computer", name: "zingzy-mbp", label: "zingzy's MacBook Pro", default: true, present: true, takesForks: false };

beforeEach(() => {
  resetSettings();
  useStore.setState({ places: [here] });
});

afterEach(() => {
  cleanup();
  document.body.innerHTML = "";
  delete window.wsp;
});

const general = async (): Promise<void> => {
  mountSettings({ api: settingsApi({ editorList: async () => [] }).api, at: { kind: "group", group: "general" } });
  await settle();
};

const toggle = (): HTMLElement => rowOf("experimental-app")!.querySelector<HTMLElement>("[data-k=experimental-app]")!;
const cardIds = (): string[] => [...document.querySelectorAll<HTMLElement>("[data-settings-page] [data-settings-card]")].map(card => card.dataset["settingsCard"] ?? "");

/** A shell whose switch reads `start` and turns as the shell answers, recording each turn asked. */
function shellWith(start: boolean | null, turn: (on: boolean) => Promise<boolean | null> = async on => on): { turned: boolean[] } {
  const turned: boolean[] = [];
  window.wsp = { experimental: async () => start, setExperimental: (on: boolean) => (turned.push(on), turn(on)) } satisfies Partial<DesktopBridge>;
  return { turned };
}

describe("Settings > General's Experimental switch", () => {
  it("is General's last card of its own, Version under it, one row naming the computer, off at first; on asks the shell to hand over", async () => {
    const { turned } = shellWith(false);
    await general();
    expect(cardIds().slice(-2)).toEqual(["experimental", "version"]);
    expect(rowOf("experimental-app")?.querySelector("[data-settings-title]")?.textContent).toBe(GENERAL_WORDS.experimentalApp);
    expect(descriptionOf("experimental-app")).toBe("Opens the new native wsp app in place of this one. Threads keep running on zingzy's MacBook Pro.");
    expect(toggle().getAttribute("aria-checked")).toBe("false");
    expect(rowOf("experimental-app")!.querySelector("[data-k=row-reset]")).toBeNull();
    fireEvent.click(toggle());
    await waitFor(() => expect(toggle().getAttribute("aria-checked")).toBe("true"));
    expect(turned).toEqual([true]);
  });

  it("on carries the arrow back to off, which is the write alone", async () => {
    const { turned } = shellWith(true);
    await general();
    expect(toggle().getAttribute("aria-checked")).toBe("true");
    fireEvent.click(rowOf("experimental-app")!.querySelector<HTMLElement>("[data-k=row-reset]")!);
    await waitFor(() => expect(toggle().getAttribute("aria-checked")).toBe("false"));
    expect(turned).toEqual([false]);
  });

  it("says where the GPUI app did not open, and stays off", async () => {
    shellWith(false, async () => Promise.reject(new Error("The Experimental app did not open: it exited with code 1")));
    await general();
    fireEvent.click(toggle());
    await settle();
    expect(lastNotice()).toContain("The Experimental app did not open: it exited with code 1");
    expect(toggle().getAttribute("aria-checked")).toBe("false");
  });

  it("draws no card where the shell finds no GPUI app installed, its read null", async () => {
    shellWith(null);
    await general();
    expect(rowOf("experimental-app")).toBeNull();
    expect(cardIds()).not.toContain("experimental");
  });

  it("draws no card in a browser tab, or in a shell from before the switch", async () => {
    await general();
    expect(rowOf("experimental-app")).toBeNull();
    cleanup();
    window.wsp = { loginStart: async () => true, openLogs: async () => {} };
    await general();
    expect(rowOf("experimental-app")).toBeNull();
    expect(cardIds()).not.toContain("experimental");
  });

  it("draws no card and asks nothing in a window on a host elsewhere", async () => {
    const experimental = vi.fn(async () => false);
    window.wsp = { experimental, hosts: async () => ({ here: "zingzy's MacBook Pro", current: "box", hosts: [] }) };
    await general();
    expect(rowOf("experimental-app")).toBeNull();
    expect(experimental).not.toHaveBeenCalled();
  });

  it("is found by searching its name", async () => {
    shellWith(false);
    await general();
    fireEvent.change(document.querySelector<HTMLInputElement>("[data-k=settings-search] input, input[data-k=settings-search]")!, { target: { value: "experimental" } });
    expect(rowTitles()).toEqual([GENERAL_WORDS.experimentalApp]);
  });
});
