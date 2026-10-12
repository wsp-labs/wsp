// SPDX-License-Identifier: AGPL-3.0-only
// What the setup job puts on Settings, Computers: the Pending list, each row
// carrying on where it was left, and a computer's page with the recipe it
// follows and its setup's steps.
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { RecipeFile, type PendingComputer, type PlaceSetup, type PlaceView, type RecipeView } from "@wsp/protocol";
import type { Api } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { closeAdd, useAddFlow } from "../src/settings/add/addFlow.js";
import { mountSettings, resetSettings, settingsApi, settle } from "./settings-harness.js";

const here: PlaceView = { id: "here", kind: "computer", name: "zingzy-mbp", default: true, present: true, takesForks: false };
const AT = "2026-10-03T10:00:00.000Z";
const running: PlaceSetup = { state: "running", addId: "a_1", startedAt: AT, steps: [{ step: "floor", state: "done" }, { step: "agents", state: "done" }, { step: "skills", state: "running" }], waiting: [] };
const box = (id: string, name: string, over: Partial<PlaceView> = {}): PlaceView => ({ id, kind: "computer", name, default: false, present: true, ...over });

beforeEach(() => resetSettings());
afterEach(() => {
  act(() => closeAdd());
  cleanup();
});

describe("the Computers page's Pending list", () => {
  it("lists the adds not set up, a joined one out of the computers, each opening Add a computer where it was left", async () => {
    const jump: PendingComputer = { id: "a_jump", address: "root@jump", name: "jumpbox", step: "choosing", placeId: "p_jump", startedAt: AT, choices: RecipeFile.parse({ name: "jumpbox" }) };
    const refused: PendingComputer = { id: "a_old", address: "ubuntu@old", step: "check", startedAt: AT, choices: RecipeFile.parse({ name: "old" }), failed: { said: "old logs in as a user that is not root" } };
    useStore.setState({ places: [here, box("p_jump", "jumpbox")], pending: [jump, refused] });
    localStorage.setItem("wsp:add-reached", JSON.stringify({ a_jump: "skills" }));
    mountSettings({ api: settingsApi({ initGet: async () => null } as unknown as Partial<Api>).api, at: { kind: "group", group: "computers" } });
    await settle();
    expect([...document.querySelectorAll("[data-grid='computers'] [data-place-row]")].map(r => r.getAttribute("data-place-row"))).toEqual(["here"]);
    const rows = [...document.querySelectorAll<HTMLElement>("[data-grid='pending'] [data-pending-row]")];
    expect(rows.map(r => r.querySelector("[data-grid-name]")?.textContent)).toEqual(["jumpbox", "ubuntu@old"]);
    expect(rows[0]!.textContent).toContain("Chosen up to Skills");
    expect(rows.map(r => r.querySelector("[data-state-mark]")?.getAttribute("data-state-mark"))).toEqual(["pending", "failed"]);
    fireEvent.click(rows[0]!);
    expect(useAddFlow.getState()).toMatchObject({ open: true, placeId: "p_jump", step: "skills" });
    act(() => closeAdd());
    fireEvent.click(rows[1]!);
    expect(useAddFlow.getState()).toMatchObject({ open: true, step: "where", address: "ubuntu@old" });
  });
});

describe("a computer's page after a setup", () => {
  it("names the recipe it follows, held until the host can move it, and lists the setup's steps", async () => {
    const builders: RecipeView = { name: "Builders", slug: "builders", summary: "1 agent", machines: ["studio"], file: RecipeFile.parse({ name: "Builders" }) };
    const studio = box("p_studio", "studio", { setup: running, picks: RecipeFile.parse({ name: "Builders" }), recipe: "builders" });
    useStore.setState({ places: [here, studio] });
    mountSettings({ api: settingsApi({ initGet: async () => null, recipesList: async () => [builders] } as unknown as Partial<Api>).api, at: { kind: "computer", id: "p_studio" } });
    await settle();
    const row = document.querySelector("[data-settings-row='follows']")!;
    expect(row.querySelector("[data-settings-description]")?.textContent).toBe("Changes to it reach studio on their own.");
    await waitFor(() => expect(row.querySelector("[data-slot=select-trigger]")?.textContent).toBe("Builders"));
    expect(row.querySelector("[data-slot=select-trigger]")?.hasAttribute("disabled") || row.querySelector("[data-slot=select-trigger]")?.getAttribute("data-disabled") !== null).toBe(true);
    expect(document.querySelector("[data-settings-card='setup'] [data-settings-head]")?.textContent).toBe("Setup");
    expect([...document.querySelectorAll("[data-settings-card='setup'] [data-step-row]")].map(r => r.getAttribute("data-step-row")).slice(0, 4)).toEqual(["wsp", "floor", "agents", "mcp"]);
    // A setup under way stands open, with no row folding it.
    expect(document.querySelector("[data-settings-row='setup-summary']")).toBeNull();
  });

  it("opens a step of the setup on its last lines of output, and stands a failed one open", async () => {
    const failed = { ...running, state: "failed" as const, said: "apt-get exited 100", steps: [{ step: "floor" as const, state: "done" as const }, { step: "agents" as const, state: "failed" as const }] };
    const studio = box("p_studio", "studio", { setup: failed, picks: RecipeFile.parse({ name: "studio" }) });
    useStore.setState({ places: [here, studio] });
    const log = ["2026-10-04T10:00:00Z [floor] apt-get install -y curl", "2026-10-04T10:00:01Z [agents] npm ERR! code E404"];
    mountSettings({ api: settingsApi({ initGet: async () => null, placesSetupLog: async () => log } as unknown as Partial<Api>).api, at: { kind: "computer", id: "p_studio" } });
    await settle();
    const row = (id: string): HTMLElement => document.querySelector<HTMLElement>(`[data-settings-card='setup'] [data-step-row='${id}']`)!;
    await waitFor(() => expect(row("agents").querySelector("[data-k=step-log]")?.textContent).toBe("npm ERR! code E404"));
    expect(document.querySelector("[data-settings-row='setup-summary']")).toBeNull();
    expect(row("floor").querySelector("[data-k=step-log]")).toBeNull();
    fireEvent.click(row("floor").querySelector("[data-k=step-toggle]")!);
    await waitFor(() => expect(row("floor").querySelector("[data-k=step-log]")?.textContent).toBe("apt-get install -y curl"));
  });

  it("folds a setup that ended done into one Setup row, its log behind Show setup log", async () => {
    const done: PlaceSetup = { ...running, state: "done", steps: running.steps.map(l => ({ ...l, state: "done" as const })) };
    const applied = { hash: "h", at: AT, rows: [{ id: "agents/claude", label: "Claude Code", outcome: "installed" as const, step: "agents" as const }] };
    useStore.setState({ places: [here, box("p_studio", "studio", { setup: done, applied, picks: RecipeFile.parse({ name: "studio" }) })] });
    mountSettings({ api: settingsApi({ initGet: async () => null } as unknown as Partial<Api>).api, at: { kind: "computer", id: "p_studio" } });
    await settle();
    const card = document.querySelector("[data-settings-card='setup']")!;
    const summary = card.querySelector("[data-settings-row='setup-summary']")!;
    expect(summary.querySelector("[data-settings-title]")?.textContent).toBe("Setup");
    expect(summary.querySelector("[data-k=setup-word]")?.textContent).toBe("Done");
    expect(card.querySelectorAll("[data-step-row]")).toHaveLength(0);
    const disclosure = summary.querySelector<HTMLElement>("[data-k=setup-log]")!;
    expect([disclosure.textContent, disclosure.getAttribute("aria-expanded")]).toEqual(["Show setup log", "false"]);
    fireEvent.click(disclosure);
    expect([disclosure.textContent, disclosure.getAttribute("aria-expanded")]).toEqual(["Hide setup log", "true"]);
    expect([...card.querySelectorAll("[data-step-row]")].map(r => r.getAttribute("data-step-row")).slice(0, 3)).toEqual(["wsp", "floor", "agents"]);
  });

  it("reads Done where the computer's word is Ready, naming the rows that failed after it", async () => {
    const done: PlaceSetup = { ...running, state: "done", steps: [...running.steps.map(l => ({ ...l, state: "done" as const })), { step: "clis", state: "failed" }] };
    const applied = { hash: "h", at: AT, rows: [{ id: "tools/brew/codex", label: "Codex CLI", outcome: "failed" as const, step: "clis" as const, note: "no bottle" }] };
    useStore.setState({ places: [here, box("p_studio", "studio", { setup: done, applied, picks: RecipeFile.parse({ name: "studio" }) })] });
    mountSettings({ api: settingsApi({ initGet: async () => null } as unknown as Partial<Api>).api, at: { kind: "computer", id: "p_studio" } });
    await settle();
    const card = document.querySelector("[data-settings-card='setup']")!;
    const summary = card.querySelector("[data-settings-row='setup-summary']")!;
    expect(summary.querySelector("[data-k=setup-word]")?.textContent).toBe("Done");
    expect(summary.querySelector("[data-settings-description]")?.textContent).toBe("Codex CLI failed");
    fireEvent.click(summary.querySelector("[data-k=setup-log]")!);
    expect(card.querySelector("[data-step-row='tools/brew/codex'] [data-k=retry]")).not.toBeNull();
  });

  it("names the rows still waiting on the person on the one row, and a row's Retry from the log runs that row alone", async () => {
    const done: PlaceSetup = {
      ...running,
      state: "done",
      steps: [...running.steps.map(l => ({ ...l, state: "done" as const })), { step: "clis", state: "failed" }],
      waiting: [{ row: "signins/codex", label: "Codex", expiresAt: AT, state: "waiting" }],
    };
    const applied = { hash: "h", at: AT, rows: [{ id: "tools/brew/gh", label: "GitHub CLI", outcome: "failed" as const, step: "clis" as const, note: "no bottle" }] };
    const studio = box("p_studio", "studio", { setup: done, applied, picks: RecipeFile.parse({ name: "studio" }) });
    const setups: { ref: string; o: unknown }[] = [];
    useStore.setState({ places: [here, studio] });
    const api = settingsApi({ initGet: async () => null, placesSetup: async (ref: string, o: unknown) => (setups.push({ ref, o }), { addId: "a_2", place: studio }) } as unknown as Partial<Api>).api;
    mountSettings({ api, at: { kind: "computer", id: "p_studio" } });
    await settle();
    const card = document.querySelector("[data-settings-card='setup']")!;
    const summary = card.querySelector("[data-settings-row='setup-summary']")!;
    expect(summary.querySelector("[data-k=setup-word]")?.textContent).toBe("Needs you");
    expect(summary.querySelector("[data-settings-description]")?.textContent).toBe("Codex sign-in, GitHub CLI");
    expect(card.querySelectorAll("[data-step-row]")).toHaveLength(0);
    fireEvent.click(summary.querySelector("[data-k=setup-log]")!);
    fireEvent.click(card.querySelector("[data-step-row='tools/brew/gh'] [data-k=retry]")!);
    await waitFor(() => expect(setups).toEqual([{ ref: "p_studio", o: { row: "tools/brew/gh" } }]));
  });

  it("moves the computer onto another recipe through the host, the answer standing on its row", async () => {
    const builders: RecipeView = { name: "Builders", slug: "builders", summary: "1 agent", machines: ["studio"], file: RecipeFile.parse({ name: "Builders" }) };
    const minimal: RecipeView = { name: "Minimal", slug: "minimal", summary: "1 CLI", machines: [], file: RecipeFile.parse({ name: "Minimal" }) };
    const studio = box("p_studio", "studio", { setup: running, picks: RecipeFile.parse({ name: "Builders" }), recipe: "builders" });
    const followed: { placeId: string; recipe: string }[] = [];
    let lists = 0;
    useStore.setState({ places: [here, studio] });
    const api = settingsApi({
      initGet: async () => null,
      recipesList: async () => {
        lists++;
        return [builders, minimal];
      },
      placesFollow: async (placeId: string, recipe: string) => {
        followed.push({ placeId, recipe });
        return { ...studio, recipe };
      },
    } as unknown as Partial<Api>).api;
    mountSettings({ api, at: { kind: "computer", id: "p_studio" } });
    await settle();
    const trigger = (): HTMLElement => document.querySelector<HTMLElement>("[data-settings-row='follows'] [data-slot=select-trigger]")!;
    await waitFor(() => expect(trigger().textContent).toBe("Builders"));
    expect(trigger().hasAttribute("disabled") || trigger().hasAttribute("data-disabled")).toBe(false);
    const read = lists;
    fireEvent.click(trigger());
    const options = await screen.findAllByRole("option");
    expect(options.map(o => o.textContent)).toEqual(["Builders", "Minimal", "None"]);
    await act(async () => void (await new Promise(r => setTimeout(r, 0))));
    fireEvent.keyDown(options[1]!, { key: "Enter" });
    fireEvent.click(options[1]!);
    await waitFor(() => expect(followed).toEqual([{ placeId: "p_studio", recipe: "minimal" }]));
    await waitFor(() => expect(useStore.getState().places.find(p => p.id === "p_studio")?.recipe).toBe("minimal"));
    await waitFor(() => expect(trigger().textContent).toBe("Minimal"));
    // Which computers follow each recipe moved, so the list is read again.
    await waitFor(() => expect(lists).toBeGreaterThan(read));
    cleanup();
  });
});
