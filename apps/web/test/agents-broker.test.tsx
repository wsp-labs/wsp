// SPDX-License-Identifier: AGPL-3.0-only
// Settings > Agents on this computer while a provider is set up: Claude Code's and Codex's rows say the kind of
// credential the host adds for machines there, off the host's broker, and a host that refuses the read leaves the
// rows as they were.
import { cleanup, fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_PREFERENCES, type AgentsReport, type BrokerStatus, type InitSetup, type PlaceView } from "@wsp/protocol";
import type { Api } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { AGENTS_LIST_WORDS } from "../src/components/agents/agentsRows.js";
import { capitalised } from "../src/settings/format.js";
import { useSettingsStore } from "../src/settings/settingsStore.js";
import { AGENTS_REPORT } from "./fixtures/agents-report.js";
import { mountSettings, resetSettings, settingsApi, settle } from "./settings-harness.js";

const here: PlaceView = { id: "here", kind: "computer", name: "zingzy-mbp", label: "zingzy's MacBook Pro", default: true, present: true, takesForks: false, shape: { cpu: 8, memMb: 16384 }, diskFreeBytes: 210 * 1024 ** 3 };
const boat: PlaceView = { id: "box", kind: "provider", name: "box", default: false, present: true, takesForks: true, shape: { cpu: 2, memMb: 4096 }, diskFreeBytes: 0 };
const setup = { keys: { box: true, solari: false }, home: "/Users/dev", agents: [], pricing: null, job: null } as unknown as InitSetup;

/** Claude Code signed in here with its own login, Codex not signed in here: the broker's word is what each row says. */
const REPORT: AgentsReport = AGENTS_REPORT;

const row = (id: string): HTMLElement => document.querySelector<HTMLElement>(`[data-settings-page] [data-agent-row="${id}"]`)!;
const status = (id: string): string | undefined => row(id).querySelector("[data-k=agent-status]")?.textContent ?? undefined;
const note = (id: string): string | undefined => row(id).querySelector("[data-settings-description]")?.textContent ?? undefined;
const signIn = (id: string): string | undefined => row(id).querySelector("[data-settings-slot] [data-k='act-sign-in']")?.textContent ?? undefined;

async function mountAgents(over: Partial<Api>): Promise<void> {
  useSettingsStore.getState().pickAgentsPlace(null);
  mountSettings({ api: settingsApi({ initGet: async () => setup, agentsRead: async () => REPORT, ...over }).api, at: { kind: "group", group: "agents" } });
  await settle();
}

beforeEach(() => {
  resetSettings();
  useStore.setState({ preferences: { ...DEFAULT_PREFERENCES, labs: false } });
});
afterEach(cleanup);

describe("this computer's agents while a provider is set up", () => {
  it("say the kind of credential the host adds for machines at the provider, and that cloud threads use it", async () => {
    let asked = 0;
    const broker: BrokerStatus = { claude: "token", codex: "chatgpt", github: "zingzy" };
    useStore.setState({ places: [here, boat] });
    await mountAgents({ brokerStatus: async () => (asked++, broker) });
    expect(asked).toBe(1);
    expect(status("claude")).toBe("Signed in with a token");
    expect(status("codex")).toBe("Signed in with ChatGPT");
    expect(note("claude")).toBe(`v2.1.281, ${AGENTS_LIST_WORDS.usedByClouds}; a token: wsp agents key claude, a key: add --api-key`);
    expect(note("codex")).toBe(`v0.62.0, ${AGENTS_LIST_WORDS.usedByClouds}`);
    expect(signIn("codex")).toBeUndefined();
  });

  it("reads a borrowed claude.ai login as a subscription, never a token", async () => {
    useStore.setState({ places: [here, boat] });
    await mountAgents({ brokerStatus: async () => ({ claude: "login", codex: "key" }) });
    expect(status("claude")).toBe("Signed in with a subscription");
    expect(status("codex")).toBe("Signed in with an API key");
  });

  it("offers Sign in on an agent the host holds no credential for, whatever its own sign-in here says, and each runs agents.cloudSignIn for that agent", async () => {
    const cloud: string[] = [];
    const plain: string[] = [];
    const press = async (id: string): Promise<void> => {
      fireEvent.click(row(id).querySelector<HTMLElement>("[data-settings-slot] [data-k='act-sign-in']")!);
      await settle();
    };
    const api: Partial<Api> = {
      agentsCloudSignIn: async agent => (cloud.push(agent), { signInId: `s_${agent}`, stop: () => undefined, off: () => undefined }),
      agentsSignIn: async (_target, agent) => (plain.push(agent), { signInId: "s_plain", stop: () => undefined, off: () => undefined }),
    };
    useStore.setState({ places: [here, boat] });
    await mountAgents({ ...api, brokerStatus: async () => ({ claude: "token" }) });
    expect(status("claude")).toBe("Signed in with a token");
    expect(signIn("codex")).toBe(AGENTS_LIST_WORDS.signIn);
    await press("codex");
    cleanup();
    resetSettings();
    useStore.setState({ places: [here, boat], preferences: { ...DEFAULT_PREFERENCES, labs: false } });
    // Claude Code signed in here on its own: the host holds nothing for cloud threads, so its Sign in is the login road,
    // never the setup-token paste its row offers without a provider.
    await mountAgents({ ...api, brokerStatus: async () => ({ codex: "chatgpt" }) });
    expect(signIn("claude")).toBe(AGENTS_LIST_WORDS.signIn);
    await press("claude");
    expect(document.querySelector("[data-settings-page] [data-k='sign-in-flow'] input[type=password]")).toBeNull();
    expect([cloud, plain]).toEqual([["codex", "claude"], []]);
  });

  it("asks nothing and says what it said before with no provider set up, or from a host that refuses the read", async () => {
    let asked = 0;
    useStore.setState({ places: [here] });
    await mountAgents({ brokerStatus: async () => (asked++, { claude: "key" }) });
    expect(asked).toBe(0);
    expect([status("claude"), note("claude")]).toEqual([capitalised(AGENTS_LIST_WORDS.signedIn), "v2.1.281"]);
    cleanup();
    resetSettings();
    useStore.setState({ places: [here, boat], preferences: { ...DEFAULT_PREFERENCES, labs: false } });
    await mountAgents({ brokerStatus: async () => Promise.reject(new Error("unknown op broker.status")) });
    expect([status("claude"), note("claude"), signIn("codex")]).toEqual([capitalised(AGENTS_LIST_WORDS.signedIn), "v2.1.281", AGENTS_LIST_WORDS.signIn]);
  });
});
