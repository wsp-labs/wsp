// SPDX-License-Identifier: AGPL-3.0-only
// Add a cloud provider, the dialog: the key checked live with the provider's
// refusals in plain words, Continue saving it and starting the setup, the
// host's own setup screens as Add a computer's pick lists, the summary held
// on the sign-ins here, and an Add pressed while a setup stands opening that
// setup rather than another.
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CLOUD_SETUP_WORDS, DEFAULT_PREFERENCES, KEY_REFUSED, KEY_UNCHECKED, type AgentsReport, type Preferences, type InitJob, type InitScreen, type InitSetup, type PlaceView, type RecipeOptions, type WorkspaceView } from "@wsp/protocol";
import { RequestError, type Api } from "../src/protocol/client.js";
import { forgetHeld } from "../src/protocol/held.js";
import { useStore } from "../src/protocol/store.js";
import { useComposerDraftStore } from "../src/components/chat/composerDraftStore.js";
import { closeCloudAdd, openCloudAdd, useCloudFlow } from "../src/settings/add/cloudFlow.js";
import { ADD_COMPUTER_WORDS } from "../src/settings/format.js";
import { useSettingsStore } from "../src/settings/settingsStore.js";
import { useHostNotices } from "../src/notices/hostNotices.js";
import { useNotices } from "../src/notices/store.js";
import { mountSettings, resetSettings, settingsApi, settle } from "./settings-harness.js";

const here: PlaceView = { id: "here", kind: "computer", name: "acme-laptop", default: true, present: true, takesForks: false, engine: "none", buildsImages: false };
const boat: PlaceView = { id: "box", kind: "provider", name: "box", default: false, rateUsdPerHour: 0.018, takesForks: true, buildsImages: true };
const setupOf = (keys: Record<string, boolean>): InitSetup => ({ keys, home: "/Users/dev", agents: [], pricing: null, job: null }) as InitSetup;

const screenOf = (id: InitScreen["id"], items: readonly [string, string][], ticks: readonly string[] = []): InitScreen => ({ id, title: id, top: "", items: items.map(([item, label]) => ({ id: item, label, detail: [] })), ticks: [...ticks], answers: {}, footer: [] });
const SCREENS: InitScreen[] = [
  screenOf("agents", [["claude", "Claude Code"], ["codex", "Codex"]], ["claude"]),
  screenOf("tools", [["gh", "GitHub CLI"], ["jq", "jq"], ["rg", "ripgrep"]], ["gh", "jq"]),
  screenOf("also", [["brew:fd", "fd"]]),
];
const jobOf = (over: Partial<InitJob> = {}): InitJob => ({ id: "init_1", road: "manual", phase: "answering", keys: { box: true }, step: 0, stoppable: true, screens: SCREENS, rows: [], progress: { done: 0, total: 0 }, log: [], place: { id: "box", name: "box" }, ...over }) as InitJob;

const signedIn = (agents: readonly string[], signIn = "signed-in"): AgentsReport =>
  ({ target: { placeId: "here" }, home: "/Users/dev", user: "dev", readAt: "2026-10-13T00:00:00Z", agents: agents.map(id => ({ id, name: id, installed: true, road: "own", signIn, signInRoad: "device", wspTools: true })), skills: [], servers: [], refused: [] }) as unknown as AgentsReport;
const OPTIONS = { configs: [{ id: "github", label: "GitHub", account: "acme" }] } as unknown as RecipeOptions;

/** A host as the dialog reaches it: the key check answering per key, every save, start, answer and build kept. */
function host(over: Partial<Api> = {}, record: Preferences = { ...DEFAULT_PREFERENCES, labs: false }) {
  const asked: { op: string; o?: unknown }[] = [];
  const fake = settingsApi({
    initGet: async () => setupOf({ box: false }),
    recipesOptions: async () => OPTIONS,
    agentsRead: async () => signedIn(["claude"]),
    providersCheckKey: async ({ key }: { provider: string; key: string }) => {
      asked.push({ op: "check", o: key });
      if (key === "boat_wrong") throw new RequestError("Boat refused this key. Check you copied all of it.", KEY_REFUSED);
      if (key === "boat_no_read") throw new RequestError("This key cannot list machines. Give it sandbox.read and the other actions above.", KEY_REFUSED);
      if (key === "boat_away") throw new RequestError("wsp cannot reach Boat right now. Try again.", KEY_UNCHECKED);
      return { works: true as const, account: "acme", plan: "$20 plan", running: 2, limit: 100 };
    },
    initKeys: async (o: unknown) => (asked.push({ op: "keys", o }), setupOf({ box: true })),
    initStart: async (o: unknown) => (asked.push({ op: "start", o }), jobOf({ phase: "reading", screens: [] })),
    initAnswer: async (o: unknown) => (asked.push({ op: "answer", o }), jobOf()),
    initDraft: async (o: unknown) => (asked.push({ op: "draft", o }), jobOf()),
    initStep: async (o: unknown) => (asked.push({ op: "step", o }), jobOf()),
    initBuild: async (o: unknown) => (asked.push({ op: "build", o }), jobOf({ phase: "building" })),
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
const pushJob = (job: InitJob): void => act(() => useStore.getState().applyEvent({ type: "init.job", job } as never));

beforeEach(() => {
  resetSettings();
  closeCloudAdd();
  useComposerDraftStore.setState({ drafts: {} });
  useStore.setState({ preferences: { ...DEFAULT_PREFERENCES, labs: false }, places: [here], initJob: null });
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

  it("saves the key on Continue and starts the setup for that cloud, then walks the host's screens", async () => {
    const fake = host();
    await openFromComputers(fake.api);
    paste("boat_good");
    await settle();
    fireEvent.click(proceed());
    await settle();
    expect(fake.asked.filter(a => a.op !== "check")).toEqual([
      { op: "keys", o: { provider: "box", key: "boat_good" } },
      { op: "start", o: { road: "manual", on: "box" } },
    ]);
    expect(stepOf()).toBe("agents");
    pushJob(jobOf());
    await settle();
    expect([...document.querySelectorAll("[data-pick-list='cloud-agents'] [data-pick-row]")].map(r => r.getAttribute("data-pick-row"))).toEqual(["claude", "codex"]);
    // Add a computer's own list tools stand over the rows.
    for (const k of ["pick-filter", "pick-all", "pick-none", "pick-invert"]) expect(document.querySelector(`[data-pick-list='cloud-agents'] [data-k='${k}']`)).not.toBeNull();
    expect(document.querySelector("[data-pick-row='claude'] [data-pick-note]")?.textContent).toBe("Uses your sign-in on acme-laptop.");
  });
});

describe("the CLIs step", () => {
  it("says a CLI that signs in goes on installed signed out, its calls in the mono tag", async () => {
    const tools: InitScreen = { ...screenOf("tools", [["gh", "GitHub CLI"], ["jq", "jq"]], ["gh", "jq"]), items: [{ id: "gh", label: "GitHub CLI", why: "41 calls", detail: [] }, { id: "jq", label: "jq", why: "12 calls", detail: [] }] };
    useStore.setState({ places: [here, boat], initJob: jobOf({ step: 1, screens: [SCREENS[0]!, tools, SCREENS[2]!] }) });
    mountSettings({ api: host().api });
    act(() => openCloudAdd("box"));
    await settle();
    expect(stepOf()).toBe("tools");
    expect(document.querySelector("[data-pick-row='gh'] [data-pick-note]")?.textContent).toBe("Installed signed out.");
    expect(document.querySelector("[data-pick-row='jq'] [data-pick-note]")).toBeNull();
    expect(document.querySelector("[data-pick-row='jq']")?.textContent).toContain("12 calls");
  });

  it("draws a row the host fixed on or off as fixed, says always on, and a click leaves each as it was", async () => {
    const tools: InitScreen = { ...screenOf("tools", [["curl", "curl"], ["jq", "jq"]], ["jq"]), items: [{ id: "curl", label: "curl", detail: [], lock: "on" }, { id: "jq", label: "jq", why: "12 calls", detail: [] }, { id: "rg", label: "ripgrep", detail: [], lock: "off" }] };
    useStore.setState({ places: [here, boat], initJob: jobOf({ step: 1, screens: [SCREENS[0]!, tools, SCREENS[2]!] }) });
    mountSettings({ api: host().api });
    act(() => openCloudAdd("box"));
    await settle();
    const box = (id: string) => document.querySelector<HTMLElement>(`[data-pick-row='${id}'] [data-slot=checkbox]`)!;
    expect(box("curl").hasAttribute("data-disabled")).toBe(true);
    expect(box("jq").hasAttribute("data-disabled")).toBe(false);
    expect(box("rg").hasAttribute("data-disabled")).toBe(true);
    expect(box("rg").getAttribute("aria-checked")).toBe("false");
    expect(document.querySelector("[data-pick-row='curl'] [data-pick-note]")?.textContent).toBe(CLOUD_SETUP_WORDS.screen.alwaysOn);
    fireEvent.click(box("curl"));
    fireEvent.click(box("rg"));
    await settle();
    expect(box("curl").getAttribute("aria-checked")).toBe("true");
    expect(box("rg").getAttribute("aria-checked")).toBe("false");
  });
});

describe("the provider step", () => {
  it("stands first where the host offers more than one cloud, and the key step names the one picked", async () => {
    await openFromComputers(host({ initGet: async () => setupOf({ box: false, solari: false }) } as unknown as Partial<Api>).api);
    expect(stepOf()).toBe("provider");
    expect([...document.querySelectorAll("[data-add-cloud] [data-choice]")].map(c => c.getAttribute("data-choice"))).toEqual(["box", "solari"]);
    expect(document.querySelector("[data-add-cloud] [data-k='progress']")?.textContent).toBe("1 of 6");
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
    expect(stepOf()).toBe("agents");
  });
});

describe("Add pressed while a setup stands", () => {
  it("opens the image building rather than a new setup, from the Add button and from the cloud's row", async () => {
    const fake = host();
    useStore.setState({ places: [here, boat], initJob: jobOf({ phase: "building", rows: [{ id: "boot", kind: "stage", label: "Start a builder on Boat", state: "running" }] }) });
    await openFromComputers(fake.api);
    expect(stepOf()).toBe("building");
    expect(dialog()!.querySelector("[data-slot=dialog-title]")?.textContent).toBe("Setting up Boat");
    expect(document.querySelector("[data-step-row='boot']")?.getAttribute("data-state")).toBe("working");
    expect(fake.asked.filter(a => a.op === "start" || a.op === "keys")).toEqual([]);
    act(() => closeCloudAdd());
    fireEvent.click(document.querySelector("[data-place-row='box']")!);
    await settle();
    expect(stepOf()).toBe("building");
    expect(fake.asked.filter(a => a.op === "start")).toEqual([]);
  });

  it("opens the picks where the host stands while they are being made, and never on another computer's setup", () => {
    useStore.setState({ initJob: jobOf({ step: 1 }) });
    openCloudAdd("box");
    expect(useCloudFlow.getState().step).toBe("tools");
    useStore.setState({ initJob: jobOf({ phase: "building", place: { id: "p_2", name: "hetzner" } }) });
    openCloudAdd("box");
    expect(useCloudFlow.getState().step).toBe("key");
  });
});

describe("the summary and the build", () => {
  it("holds Build image until every sign-in here is ready, then builds on that cloud", async () => {
    const fake = host({ agentsRead: async () => signedIn(["claude"], "none") } as unknown as Partial<Api>);
    useStore.setState({ places: [here, boat], initJob: jobOf({ step: 3 }) });
    mountSettings({ api: fake.api });
    act(() => openCloudAdd("box"));
    await settle();
    expect(stepOf()).toBe("summary");
    expect(document.querySelector("[data-settings-row='claude']")?.textContent).toContain("Not signed in on acme-laptop");
    expect(held(proceed())).toBe(true);
    cleanup();
    act(() => closeCloudAdd());
    forgetHeld();
    const ready = host();
    mountSettings({ api: ready.api });
    act(() => openCloudAdd("box"));
    await settle();
    expect(document.querySelector("[data-settings-row='claude']")?.textContent).toContain("Signed in on acme-laptop");
    expect(document.querySelector("[data-settings-row='github']")?.textContent).toContain("Signed in as acme");
    expect(held(proceed())).toBe(false);
    fireEvent.click(proceed());
    await waitFor(() => expect(ready.asked.some(a => a.op === "build")).toBe(true));
    expect(ready.asked.find(a => a.op === "build")?.o).toEqual({ on: "box" });
    await settle();
    expect(stepOf()).toBe("building");
  });

  it("builds on a cloud whose key was saved in the dialog, on a host that held no key when it opened", async () => {
    // The host as it stands with no key: the cloud is no place yet, so a read naming it is refused until a key is saved.
    let saved = false;
    const fake = host({
      initGet: async (o?: { on?: string }) => {
        if (o?.on !== undefined && !saved) throw new RequestError(`no place named ${o.on}; you have acme-laptop`, "missing");
        return setupOf({ box: saved });
      },
      initKeys: async () => ((saved = true), setupOf({ box: true })),
    } as unknown as Partial<Api>);
    await openFromComputers(fake.api);
    paste("boat_good");
    await settle();
    fireEvent.click(proceed());
    await settle();
    pushJob(jobOf({ step: 3 }));
    act(() => useCloudFlow.setState({ step: "summary" }));
    await settle();
    expect(stepOf()).toBe("summary");
    expect(held(proceed())).toBe(false);
    fireEvent.click(proceed());
    await waitFor(() => expect(fake.asked.find(a => a.op === "build")?.o).toEqual({ on: "box" }));
  });

  it("says the host's refusal of the setup read where Build image is held, never a silent grey button", async () => {
    useStore.setState({ places: [here, boat], initJob: jobOf({ step: 3 }) });
    mountSettings({ api: host({ initGet: async () => Promise.reject(new RequestError("wsp could not read the agents on acme-laptop.", "provider")) } as unknown as Partial<Api>).api });
    act(() => openCloudAdd("box"));
    await settle();
    expect(stepOf()).toBe("summary");
    expect(held(proceed())).toBe(true);
    expect(document.querySelector("[data-k='cloud-refusal']")?.textContent).toContain("wsp could not read the agents on acme-laptop.");
  });

  it("takes the setup a saved key answers with, so a refused read holds nothing once the key is saved", async () => {
    useStore.setState({ places: [here, boat] });
    mountSettings({ api: host({ initGet: async () => Promise.reject(new RequestError("wsp could not read the agents on acme-laptop.", "provider")) } as unknown as Partial<Api>).api });
    act(() => openCloudAdd("box"));
    await settle();
    paste("boat_good");
    await settle();
    fireEvent.click(proceed());
    await settle();
    pushJob(jobOf({ step: 3 }));
    act(() => useCloudFlow.setState({ step: "summary" }));
    await settle();
    expect(document.querySelector("[data-k='cloud-refusal']")).toBeNull();
    expect(held(proceed())).toBe(false);
  });

  it("says the host's refusal to start the setup on the key step, and stays there", async () => {
    await openFromComputers(host({ initStart: async () => Promise.reject(new RequestError("no place named box; you have acme-laptop", "missing")) } as unknown as Partial<Api>).api);
    paste("boat_good");
    await settle();
    fireEvent.click(proceed());
    await settle();
    expect(stepOf()).toBe("key");
    expect(document.querySelector("[data-k='key-refused']")?.textContent).toContain("no place named box; you have acme-laptop");
  });

  it("says the host's refusal to build on the summary", async () => {
    useStore.setState({ places: [here, boat], initJob: jobOf({ step: 3 }) });
    mountSettings({ api: host({ initBuild: async () => Promise.reject(new RequestError("Boat refused this key. Check you copied all of it.", "auth")) } as unknown as Partial<Api>).api });
    act(() => openCloudAdd("box"));
    await settle();
    fireEvent.click(proceed());
    await settle();
    expect(stepOf()).toBe("summary");
    expect(document.querySelector("[data-k='cloud-refusal']")?.textContent).toContain("Boat refused this key. Check you copied all of it.");
  });

  it("says it is reading the sign-ins here until this computer's report answers, never Not signed in", async () => {
    useStore.setState({ places: [here, boat], initJob: jobOf({ step: 3 }) });
    mountSettings({ api: host({ agentsRead: () => new Promise(() => undefined) } as unknown as Partial<Api>).api });
    act(() => openCloudAdd("box"));
    await settle();
    expect(document.querySelector("[data-settings-row='claude'] [data-settings-description]")?.textContent).toBe("Reading the sign-ins on acme-laptop");
    expect(document.querySelector("[data-settings-row='claude'] [data-k^='sign-in-']")).toBeNull();
    expect(held(proceed())).toBe(true);
  });

  it("names how each agent is signed in here, ChatGPT for Codex and a subscription for Claude Code, and offers Sign in where it is not", async () => {
    const rows = [
      { id: "claude", name: "Claude Code", installed: true, road: "own", signIn: "signed-in", signInKind: "subscription", signInRoad: "device", wspTools: true },
      { id: "codex", name: "Codex", installed: true, road: "own", signIn: "signed-in", signInKind: "subscription", signInRoad: "device", wspTools: true },
    ];
    const report = { ...signedIn([]), agents: rows } as unknown as AgentsReport;
    const both = jobOf({ step: 3, screens: [screenOf("agents", [["claude", "Claude Code"], ["codex", "Codex"]], ["claude", "codex"]), ...SCREENS.slice(1)] });
    useStore.setState({ places: [here, boat], initJob: both });
    mountSettings({ api: host({ agentsRead: async () => report } as unknown as Partial<Api>).api });
    act(() => openCloudAdd("box"));
    await settle();
    expect(document.querySelector("[data-settings-row='claude'] [data-settings-description]")?.textContent).toBe("Signed in with a subscription");
    expect(document.querySelector("[data-settings-row='codex'] [data-settings-description]")?.textContent).toBe("Signed in with ChatGPT");
    cleanup();
    act(() => closeCloudAdd());
    forgetHeld();
    const keyed = { ...report, agents: [{ ...rows[0], signIn: "vault-key", signInKind: "api-key" }, { ...rows[1], signIn: "none", signInKind: undefined }] } as unknown as AgentsReport;
    mountSettings({ api: host({ agentsRead: async () => keyed } as unknown as Partial<Api>).api });
    act(() => openCloudAdd("box"));
    await settle();
    expect(document.querySelector("[data-settings-row='claude'] [data-settings-description]")?.textContent).toBe("Signed in with an API key");
    expect(document.querySelector("[data-settings-row='codex'] [data-k='sign-in-codex']")?.textContent).toContain("Sign in");
    expect(held(proceed())).toBe(true);
  });

  it("starts the first thread on the computer the host runs on with the sub-thread ask begun", async () => {
    const local = { id: "ws_here", name: "home", kind: "local", project: "p_home", phase: "running" } as unknown as WorkspaceView;
    useStore.setState({ places: [here, boat], workspaces: [local], initJob: jobOf({ phase: "done", golden: { version: 1 } }) });
    mountSettings({ api: host().api });
    act(() => {
      openCloudAdd("box");
      useCloudFlow.setState({ step: "ready" });
    });
    await settle();
    expect(document.querySelector("[data-k='ready-title']")?.textContent).toBe("Boat is ready");
    fireEvent.click(document.querySelector("[data-k='start-thread']")!);
    expect(useCloudFlow.getState().open).toBe(false);
    expect(useStore.getState().selectedId).toBe("ws_here");
    expect(useComposerDraftStore.getState().drafts["ws_here"]).toEqual({ prompt: "Start a sub-thread on Boat to ", cursor: "Start a sub-thread on Boat to ".length });
  });
});

describe("the ready page after the dialog closed mid-build", () => {
  const local = { id: "ws_here", name: "home", kind: "local", project: "p_home", phase: "running" } as unknown as WorkspaceView;
  const building = jobOf({ phase: "building", rows: [{ id: "boot", kind: "stage", label: "Start a builder on Boat", state: "running" }] });
  const sealed = jobOf({ phase: "done", golden: { version: 1 } });
  // A seal reads the host's capabilities again.
  const sealing = () => host({ capabilities: async () => ({ upgrade: false, snapshot: false }) } as unknown as Partial<Api>);
  function Notices() {
    useHostNotices();
    return null;
  }
  const seal = (push: (e: never) => void): void =>
    act(() => {
      useStore.getState().applyEvent({ type: "init.job", job: sealed } as never);
      push({ type: "init.job", job: sealed } as never);
    });
  const readyAction = (): HTMLButtonElement => {
    const notice = [...document.querySelectorAll<HTMLElement>("[data-notice]")].find(n => n.querySelector("[data-notice-title]")?.textContent === "Boat is ready");
    return notice!.querySelector<HTMLButtonElement>("[data-notice-action]")!;
  };

  it("opens the ready page from the seal's Open after a reload, and its act starts the first thread", async () => {
    const fake = sealing();
    useStore.setState({ places: [here, boat], workspaces: [local], initJob: building });
    mountSettings({ api: fake.api, children: <Notices /> });
    act(() => openCloudAdd("box"));
    await settle();
    expect(stepOf()).toBe("building");
    fireEvent.click(document.querySelector("[data-add-cloud] [data-k='close']")!);
    expect(useCloudFlow.getState().open).toBe(false);
    // The window reloads: the page and its stores start again, the browser's storage stays.
    cleanup();
    forgetHeld();
    act(() => useNotices.getState().clear());
    useStore.setState({ settingsOpen: false });
    const after = sealing();
    mountSettings({ api: after.api, children: <Notices /> });
    act(() => useStore.setState({ settingsOpen: false }));
    await settle();
    seal(after.push);
    await settle();
    fireEvent.click(readyAction());
    await settle();
    expect(stepOf()).toBe("ready");
    expect(document.querySelector("[data-k='ready-title']")?.textContent).toBe("Boat is ready");
    fireEvent.click(document.querySelector("[data-k='start-thread']")!);
    expect(useStore.getState().selectedId).toBe("ws_here");
    expect(useComposerDraftStore.getState().drafts["ws_here"]).toEqual({ prompt: "Start a sub-thread on Boat to ", cursor: "Start a sub-thread on Boat to ".length });
  });

  it("goes to the cloud's page once its ready page was shown, and for a build the dialog never showed", async () => {
    const fake = sealing();
    useStore.setState({ places: [here, boat], workspaces: [local], initJob: building });
    mountSettings({ api: fake.api, children: <Notices /> });
    act(() => openCloudAdd("box"));
    await settle();
    fireEvent.click(document.querySelector("[data-add-cloud] [data-k='close']")!);
    act(() => useStore.setState({ settingsOpen: false }));
    seal(fake.push);
    await settle();
    const open = useNotices.getState().notices.find(n => n.text === "Boat is ready")!.action!;
    fireEvent.click(readyAction());
    await settle();
    expect(stepOf()).toBe("ready");
    fireEvent.click(document.querySelector("[data-k='ready-close']")!);
    await settle();
    expect(useCloudFlow.getState().open).toBe(false);
    act(() => open.run());
    await settle();
    expect(useCloudFlow.getState().open).toBe(false);
    expect(useSettingsStore.getState().at).toEqual({ kind: "computer", id: "box" });
    // A rebuild from Settings the dialog never stood on owes no ready page.
    cleanup();
    act(() => useNotices.getState().clear());
    useStore.setState({ settingsOpen: false, initJob: { ...building, id: "init_2" } });
    const again = sealing();
    mountSettings({ api: again.api, children: <Notices /> });
    act(() => useStore.setState({ settingsOpen: false }));
    await settle();
    act(() => {
      useStore.getState().applyEvent({ type: "init.job", job: { ...sealed, id: "init_2" } } as never);
      again.push({ type: "init.job", job: { ...sealed, id: "init_2" } } as never);
    });
    await settle();
    fireEvent.click(readyAction());
    await settle();
    expect(useCloudFlow.getState().open).toBe(false);
    expect(useSettingsStore.getState().at).toEqual({ kind: "computer", id: "box" });
  });

  it("never opens the ready page for a later host life's build, whose id the host names afresh", async () => {
    const fake = sealing();
    useStore.setState({ places: [here, boat], workspaces: [local], initJob: building });
    mountSettings({ api: fake.api, children: <Notices /> });
    act(() => openCloudAdd("box"));
    await settle();
    fireEvent.click(document.querySelector("[data-add-cloud] [data-k='close']")!);
    // The host restarts before the seal and the window reloads; the next life's build is a rebuild from Settings.
    cleanup();
    act(() => useNotices.getState().clear());
    const later = { ...sealed, id: "init_7c1e9a52-3f0b-4d6e-9b1a-2e8f5c4d7a10" };
    useStore.setState({ settingsOpen: false, initJob: { ...building, id: later.id } });
    const after = sealing();
    mountSettings({ api: after.api, children: <Notices /> });
    act(() => useStore.setState({ settingsOpen: false }));
    await settle();
    act(() => {
      useStore.getState().applyEvent({ type: "init.job", job: later } as never);
      after.push({ type: "init.job", job: later } as never);
    });
    await settle();
    fireEvent.click(readyAction());
    await settle();
    expect(useCloudFlow.getState().open).toBe(false);
    expect(useSettingsStore.getState().at).toEqual({ kind: "computer", id: "box" });
  });

  it("says the seal it held back once Close shuts it on the sealed build, and that Open shows the ready page", async () => {
    const fake = sealing();
    useStore.setState({ places: [here, boat], workspaces: [local], initJob: building });
    mountSettings({ api: fake.api, children: <Notices /> });
    act(() => openCloudAdd("box"));
    act(() => useStore.setState({ settingsOpen: false }));
    await settle();
    seal(fake.push);
    await settle();
    expect(dialog()!.querySelector("[data-slot=dialog-title]")?.textContent).toBe("Boat is ready");
    expect(useNotices.getState().notices.filter(n => n.text === "Boat is ready")).toEqual([]);
    fireEvent.click(document.querySelector("[data-add-cloud] [data-k='close']")!);
    await settle();
    expect(useNotices.getState().notices.filter(n => n.text === "Boat is ready")).toHaveLength(1);
    fireEvent.click(readyAction());
    await settle();
    expect(stepOf()).toBe("ready");
    fireEvent.click(document.querySelector("[data-k='start-thread']")!);
    expect(useComposerDraftStore.getState().drafts["ws_here"]).toEqual({ prompt: "Start a sub-thread on Boat to ", cursor: "Start a sub-thread on Boat to ".length });
  });

  it("says nothing when sent to the background mid-build", async () => {
    useStore.setState({ places: [here, boat], workspaces: [local], initJob: building });
    mountSettings({ api: sealing().api, children: <Notices /> });
    act(() => openCloudAdd("box"));
    await settle();
    fireEvent.click(document.querySelector("[data-add-cloud] [data-k='close']")!);
    await settle();
    expect(useCloudFlow.getState().open).toBe(false);
    expect(useNotices.getState().notices).toEqual([]);
  });

  it("sends Open to Settings once the cloud is gone, never to an empty dialog", async () => {
    const fake = sealing();
    useStore.setState({ places: [here, boat], workspaces: [local], initJob: building });
    mountSettings({ api: fake.api, children: <Notices /> });
    act(() => openCloudAdd("box"));
    await settle();
    fireEvent.click(document.querySelector("[data-add-cloud] [data-k='close']")!);
    act(() => useStore.setState({ settingsOpen: false }));
    seal(fake.push);
    await settle();
    act(() => useStore.setState({ places: [here] }));
    fireEvent.click(readyAction());
    await settle();
    expect(useCloudFlow.getState().open).toBe(false);
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

  it("is gone once a cloud is added, and gives the foot to the cloud's build, which opens that build", async () => {
    useStore.setState({ places: [here, boat] });
    await mountSidebar(host().api);
    expect(card()).toBeNull();
    useStore.setState({ places: [here, boat], initJob: jobOf({ phase: "building", rows: [{ id: "boot", kind: "stage", label: "Start a builder on Boat", state: "running" }] }) });
    await settle();
    const building = document.querySelector<HTMLElement>("[data-setup-card='box']")!;
    expect(building.textContent).toContain("Start a builder on Boat");
    fireEvent.click(building.querySelector("button")!);
    expect(useCloudFlow.getState()).toMatchObject({ open: true, provider: "box", step: "building" });
  });

  it("waits while a computer's setup stands in the foot", async () => {
    useStore.setState({ places: [here, { ...here, id: "p_2", name: "hetzner", default: false, setup: { addId: "a_1", state: "running", startedAt: "2026-10-13T00:00:00Z", steps: [{ step: "agents", state: "running" }], waiting: [] } } as PlaceView] });
    await mountSidebar(host().api);
    expect(document.querySelector("[data-setup-card='p_2']")).not.toBeNull();
    expect(card()).toBeNull();
  });
});
