// SPDX-License-Identifier: AGPL-3.0-only
// Where Settings is open and what its search field holds: a small store
// beside the right panel's, read by the settings sidebar, the page, the
// breadcrumb and the header's Restore defaults. The open page is kept in this
// browser's storage, never on the host's record, so Settings reopens where it
// was closed in this window and a browser tab on the same host keeps its own.
// The host's reads the pages draw from live here too, so one reader in the
// page serves the sidebar's search as well.
import type { AccountView, DeviceView, EditorChoice, InitSetup, PlaceView, ProjectView, SealedImageView, TerminalConfig } from "@wsp/protocol";
import { create } from "zustand";
import { notRead } from "../notices/store.js";
import { forgetHeld, heldFresh, hold } from "../protocol/held.js";
import { openCloudAdd } from "./add/cloudFlow.js";
import { isSettingsGroupId, type SettingsGroupId } from "./groupIds.js";

/** A page of Settings: a group, a computer's own page, a project's or an agent's. */
export type SettingsAt = { readonly kind: "group"; readonly group: SettingsGroupId } | { readonly kind: "computer"; readonly id: string } | { readonly kind: "project"; readonly id: string } | { readonly kind: "agent"; readonly id: string };

/** The page a fresh window opens on. */
export const FIRST_PAGE: SettingsAt = { kind: "group", group: "appearance" };
const AT_KEY = "wsp:settings-at";

/** The one word for a page, worn by the page root as `data-settings-at` and by the sidebar's rows as their id. */
export function atId(at: SettingsAt): string {
  return at.kind === "group" ? at.group : `${at.kind}:${at.id}`;
}

export const sameAt = (a: SettingsAt, b: SettingsAt): boolean => atId(a) === atId(b);

const GROUP_OF_PAGE = { computer: "computers", project: "projects", agent: "agents" } as const;

/** The group a page belongs to: its own for a group page, Computers for a computer's, Projects for a project's,
 * Agents for an agent's. */
export const groupOf = (at: SettingsAt): SettingsGroupId => (at.kind === "group" ? at.group : GROUP_OF_PAGE[at.kind]);

/** A page read back off storage, or nothing for a word that names no page. */
export function parseAt(raw: string | null): SettingsAt | null {
  if (raw === null) return null;
  const [kind, id] = raw.includes(":") ? [raw.slice(0, raw.indexOf(":")), raw.slice(raw.indexOf(":") + 1)] : [raw, ""];
  if ((kind === "computer" || kind === "project" || kind === "agent") && id !== "") return { kind, id };
  return isSettingsGroupId(raw) ? { kind: "group", group: raw } : null;
}

/** A remembered page whose noun is gone falls back to its group, checked against the lists on read. */
export function resolveAt(at: SettingsAt, places: ReadonlyArray<Pick<PlaceView, "id">>, projects: ReadonlyArray<Pick<ProjectView, "id">>): SettingsAt {
  if (at.kind === "computer" && !places.some(place => place.id === at.id)) return { kind: "group", group: "computers" };
  if (at.kind === "project" && !projects.some(project => project.id === at.id)) return { kind: "group", group: "projects" };
  return at;
}

const readStored = (): SettingsAt => {
  try {
    return parseAt(window.localStorage.getItem(AT_KEY)) ?? FIRST_PAGE;
  } catch {
    return FIRST_PAGE;
  }
};

/** What the pages draw from the host, read once by the page and kept here for the sidebar's search. `null` is a
 * read not yet answered or refused; a refusal is its own flag where the page says a sentence for it. */
export interface SettingsReads {
  readonly setup: InitSetup | null;
  readonly file: TerminalConfig | null;
  readonly account: AccountView | null;
  readonly devices: ReadonlyArray<DeviceView> | null;
  readonly devicesRefused: boolean;
  readonly image: SealedImageView | null;
  /** Whether a read of the image is out, so a row with no answer yet says it is reading rather than nothing. */
  readonly imageReading: boolean;
  /** Whether the answer was read before the latest seal, so a row says it is reading rather than what it held. */
  readonly imageStale: boolean;
  /** The editors installed on the computer the host runs on; null before an answer. */
  readonly editors: ReadonlyArray<EditorChoice> | null;
  /** Whether the person's ~/.ssh/config reads wsp's own ssh config; null before an answer and where the host carries
   * no editor's ssh. */
  readonly sshInclude: boolean | null;
  /** Whether this computer's service starts wsp at login; null before an answer, in a browser tab, and where no
   * service is registered. */
  readonly loginStart: boolean | null;
  /** Whether the desktop shell hands this app over to the Experimental one; null before an answer, in a browser tab,
   * on a shell from before the switch, in a window on a host elsewhere and where no Experimental app is installed. */
  readonly experimental: boolean | null;
}

/** How long a read made as Settings opens stands: what these say moves on the person's own acts here, which write
 * their answers back, or on an event the page follows. */
export const SETTINGS_HOLD_MS = 60_000;

const IMAGE_KEY = "settings:image";

/** The image's read, asked as Settings opens and at each seal wherever the person is, so a seal's answer has mostly
 * landed before Settings shows. Nothing inside the hold; an answer asked before the latest seal lands on nothing. */
export function readImage(read: (() => Promise<SealedImageView>) | undefined): void {
  if (read === undefined || heldFresh(IMAGE_KEY, SETTINGS_HOLD_MS)) return;
  const { imageAsked, setReads } = useSettingsStore.getState();
  const newest = (): boolean => useSettingsStore.getState().imageAsked === imageAsked;
  setReads({ imageReading: true });
  void hold(IMAGE_KEY, read, { holdMs: SETTINGS_HOLD_MS }).then(
    view => newest() && setReads({ image: view, imageReading: false, imageStale: false }),
    e => {
      if (newest()) setReads({ imageReading: false });
      notRead("Image")(e);
    },
  );
}

export const NO_READS: SettingsReads = { setup: null, file: null, account: null, devices: null, devicesRefused: false, image: null, imageReading: false, imageStale: false, editors: null, sshInclude: null, loginStart: null, experimental: null };

export interface SettingsState {
  readonly at: SettingsAt;
  readonly search: string;
  readonly reads: SettingsReads;
  /** The Add a project sheet asked for from the Projects page, keyed by the ask so a second one is a fresh sheet. */
  readonly addProjectAt: number | null;
  /** How many times the devices list has been asked for again, which the read effect follows. */
  readonly devicesAsked: number;
  /** How many times the image has been asked for again, which the read effect follows. */
  readonly imageAsked: number;
  /** The computer whose image build a card is drawing now, which is where that build's waits are said. */
  readonly buildShown: string | null;
  /** The computer whose Image card opens its recipe when it is drawn: an Edit image pressed outside that card. Moving
   * to another page drops it, so an ask no card took never opens a recipe later. */
  readonly recipeAsked: string | null;
  /** The Usage page's tab, kept while the window is open so coming back finds the same one. */
  readonly usageTab: UsageTab;
  /** Each ask to read usage and limits again, from the head's refresh, the minute's tick or the window's focus. */
  readonly usageAsked: number;
  /** When the page's two answers were read, the older of them, and whether a read is out, for the head to say. */
  readonly usageRead: { readonly at: number | null; readonly reading: boolean };
  /** The Agents page's tab and the computer it reads, kept while the window is open. Null reads the one wsp runs on. */
  readonly agentsTab: AgentsTab;
  readonly agentsPlace: string | null;
  /** The page under the Agents page's tab that stands in place of its list: one server or skill, or an add. Moving to
   * another page, tab or computer drops it. */
  readonly agentsLevel: AgentsLevel | null;
  /** Activity's view and the computer it reads, kept while the window is open. Null reads the one wsp runs on. */
  readonly activityView: ActivityView;
  readonly activityPlace: string | null;
  /** The computer a remove is taking out while its dialog still has something to say: its page stays drawn from the
   * last copy of it after the host drops it from the list, until the dialog closes. */
  readonly parting: string | null;
  go(at: SettingsAt): void;
  setSearch(search: string): void;
  setReads(patch: Partial<SettingsReads>): void;
  openAddProject(): void;
  closeAddProject(): void;
  rereadDevices(): void;
  /** A seal filed a copy the held answer does not have: it stands as stale, so no row draws it as current, and the
   * image is read again with `read`, the store's client, wherever the person is. */
  rereadImage(read?: () => Promise<SealedImageView>): void;
  showBuild(placeId: string): void;
  hideBuild(placeId: string): void;
  askRecipe(placeId: string | null): void;
  /** Add a cloud provider, opened on the setup that stands where one does. */
  askAdd(road: AddRoad): void;
  pickUsageTab(tab: UsageTab): void;
  askUsage(): void;
  setUsageRead(read: { at: number | null; reading: boolean }): void;
  pickAgentsTab(tab: AgentsTab): void;
  pickAgentsPlace(placeId: string | null): void;
  openAgentsLevel(level: AgentsLevel | null): void;
  pickActivityView(view: ActivityView): void;
  pickActivityPlace(placeId: string | null): void;
  holdParting(placeId: string | null): void;
}

/** The road the add asked from outside Settings' own buttons opens on. */
export type AddRoad = "cloud";
export type UsageTab = "used" | "limits";
export type AgentsTab = "agents" | "servers" | "skills" | "plugins";
export type ActivityView = "threads" | "processes" | "kinds" | "ports";

/** A page under the Agents page's list, by its name for the crumb: one item of the tab's kind by its key, the tab's
 * add, or one thing the add found, by its key and the search that found it. `up` is where the crumb's back goes. */
export type AgentsLevel =
  | { readonly kind: "item"; readonly key: string; readonly name: string }
  | { readonly kind: "add"; readonly name: string; readonly query?: string }
  | { readonly kind: "found"; readonly key: string; readonly name: string; readonly query: string; readonly up: AgentsLevel };

export const useSettingsStore = create<SettingsState>(set => ({
  at: typeof window === "undefined" ? FIRST_PAGE : readStored(),
  search: "",
  reads: NO_READS,
  addProjectAt: null,
  devicesAsked: 0,
  imageAsked: 0,
  buildShown: null,
  recipeAsked: null,
  usageTab: "used",
  usageAsked: 0,
  usageRead: { at: null, reading: false },
  agentsTab: "agents",
  agentsPlace: null,
  agentsLevel: null,
  activityView: "threads",
  activityPlace: null,
  parting: null,
  go(at) {
    set({ at, search: "", recipeAsked: null, agentsLevel: null });
    try {
      window.localStorage.setItem(AT_KEY, atId(at));
    } catch {
      // A browser that refuses storage keeps the page for this window alone.
    }
  },
  pickUsageTab(usageTab) {
    set({ usageTab });
  },
  askUsage() {
    set(s => ({ usageAsked: s.usageAsked + 1 }));
  },
  setUsageRead(usageRead) {
    set({ usageRead });
  },
  pickAgentsTab(agentsTab) {
    set({ agentsTab, agentsLevel: null });
  },
  pickAgentsPlace(agentsPlace) {
    set({ agentsPlace, agentsLevel: null });
  },
  openAgentsLevel(agentsLevel) {
    set({ agentsLevel });
  },
  pickActivityView(activityView) {
    set({ activityView });
  },
  pickActivityPlace(activityPlace) {
    set({ activityPlace });
  },
  holdParting(parting) {
    set({ parting });
  },

  setSearch(search) {
    set({ search });
  },
  setReads(patch) {
    set(s => ({ reads: { ...s.reads, ...patch } }));
  },
  openAddProject() {
    set({ addProjectAt: Date.now() });
  },
  closeAddProject() {
    set({ addProjectAt: null });
  },
  rereadDevices() {
    set(s => ({ devicesAsked: s.devicesAsked + 1 }));
  },
  rereadImage(read) {
    forgetHeld(IMAGE_KEY);
    set(s => ({ reads: { ...s.reads, imageStale: s.reads.image !== null }, imageAsked: s.imageAsked + 1 }));
    readImage(read);
  },
  showBuild(placeId) {
    set({ buildShown: placeId });
  },
  hideBuild(placeId) {
    set(s => (s.buildShown === placeId ? { buildShown: null } : {}));
  },
  askRecipe(placeId) {
    set({ recipeAsked: placeId });
  },
  askAdd() {
    openCloudAdd();
  },
}));
