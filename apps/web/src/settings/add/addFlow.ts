// SPDX-License-Identifier: AGPL-3.0-only
// Add a computer as the dialog walks it: where, the checks and the install,
// the picks step by step, the summary, then the setup running. The install is
// the host's add over ssh and its steps; the picks are kept on the pending add
// as they move, so closing the dialog leaves the computer pending with them;
// Set up hands them to the host. Which step a pending add was left at is this
// window's own, kept beside its id.
import { create } from "zustand";
import { PLACE_HOST_KEY_KIND, PLACE_SUDO_KIND, SETUP_WORDS, placeName, placeWord, setupNeedsYouLine, waitLine, type PendingComputer, type RecipeSignIn, type PlaceAddJob, type PlaceEstimate, type PlaceSetup, type PlaceView, type ProjectIcon, type RecipeFile, type RecipeOptions } from "@wsp/protocol";
import { addNotice, noticeFailure } from "../../notices/store.js";
import type { Api } from "../../protocol/client.js";
import { useStore } from "../../protocol/store.js";
import { failureOf, type Failure } from "../../protocol/failure.js";
import { addOverSsh, useAdds } from "../adds.js";
import { ADD_COMPUTER_WORDS } from "../format.js";
import { useSettingsStore } from "../settingsStore.js";
import { leftRows, setupRows } from "./setup.js";

/** The steps a person walks, in the order they run. Start from stands only where a recipe is saved. */
export const ADD_STEPS = ["where", "checks", "startfrom", "agents", "mcp", "clis", "skills", "plugins", "github", "projects", "other", "summary"] as const;
export type AddStep = (typeof ADD_STEPS)[number] | "running" | "ready";

/** The title of each step a person walks. */
export const STEP_TITLES: Record<Exclude<AddStep, "running" | "ready">, string> = {
  where: ADD_COMPUTER_WORDS.title,
  checks: "Checks",
  startfrom: "Start from",
  agents: "Agents",
  mcp: "MCP servers",
  clis: "CLIs",
  skills: "Skills",
  plugins: "Plugins",
  github: "GitHub",
  projects: "Import projects",
  other: "Other config",
  summary: "Summary",
};

/** The line under each step's title: what the step does and why, `box` the computer being added and `here` the one
 * the host runs on. */
export const stepLine = (step: Exclude<AddStep, "where" | "running" | "ready">, box: string, here: string): string =>
  ({
    checks: `Making sure wsp can reach ${box} and run there.`,
    startfrom: `Begin with everything on ${here}, a saved recipe, or nothing.`,
    agents: `The coding agents to install on ${box}, signed in so threads run there.`,
    mcp: `The tool servers your agents use, set up for them on ${box}.`,
    clis: `The command-line tools your agents run, to install on ${box}. Most used first.`,
    skills: `Your agents' skills, copied so they work the same on ${box}.`,
    plugins: `Claude Code plugins to install on ${box}.`,
    github: `How ${box} signs in to GitHub to clone and push your repos.`,
    projects: `Projects on ${here} to clone onto ${box}.`,
    other: `Your git and shell settings, so ${box} behaves like ${here}.`,
    summary: `What goes on ${box}, before it starts.`,
  })[step];

/** The first step past the install: Start from where a recipe is saved, else Agents. */
export const firstPick = (recipes: number): AddStep => (recipes > 0 ? "startfrom" : "agents");

/** The steps this add walks, Start from left out where no recipe is saved. */
export const stepsFor = (recipes: number): readonly AddStep[] => ADD_STEPS.filter(step => step !== "startfrom" || recipes > 0);

interface SaveAs {
  on: boolean;
  name: string;
  icon: ProjectIcon;
}

interface AddFlowState {
  open: boolean;
  step: AddStep;
  /** The address typed or picked. */
  address: string;
  /** The install this dialog started, by its stream. */
  addId: string | null;
  /** The computer once it joined. */
  placeId: string | null;
  /** The pending add standing for it until Set up. */
  pendingId: string | null;
  picks: RecipeFile | null;
  /** The saved recipe the picks started from, by slug; "here" is everything, "none" is nothing. */
  from: string;
  options: RecipeOptions | null;
  optionsRefused: Failure | null;
  saveAs: SaveAs;
  /** How many saves have landed in this dialog: the foot's check draws in once more on each. */
  saves: number;
  /** Set up asked and not answered, and its refusal where it was refused. */
  starting: boolean;
  refused: Failure | null;
  /** What the picks weigh against the computer's room, read on the summary; null until the host answered. */
  estimate: PlaceEstimate | null;
  /** The host key the person trusted for the address, sent again with every add of that address from this dialog,
   * so the one that carries a sudo password is held against it before the password leaves. */
  trustedKey: string | null;
}

const CLOSED: AddFlowState = { open: false, step: "where", address: "", addId: null, placeId: null, pendingId: null, picks: null, from: "here", options: null, optionsRefused: null, saveAs: { on: false, name: "", icon: "rocket" }, saves: 0, starting: false, refused: null, estimate: null, trustedKey: null };

export const useAddFlow = create<AddFlowState>(() => CLOSED);

/** The dialog closed or opened afresh, the options it read kept: a step drawn again never stands blank while the host
 * answers once more. */
const fresh = (): AddFlowState => ({ ...CLOSED, options: useAddFlow.getState().options });

const REACHED_KEY = "wsp:add-reached";

/** The record last parsed and the text it was parsed from, so a render reads a string and parses only a change. */
let reached: { raw: string | null; held: Record<string, AddStep> } = { raw: null, held: {} };

/** The step each pending add was left at in this window, by the pending add's id. */
export function reachedSteps(): Record<string, AddStep> {
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(REACHED_KEY);
  } catch {
    return {};
  }
  if (raw === reached.raw) return reached.held;
  let held: unknown = {};
  try {
    held = JSON.parse(raw ?? "{}") as unknown;
  } catch {
    held = {};
  }
  reached = { raw, held: held !== null && typeof held === "object" ? (held as Record<string, AddStep>) : {} };
  return reached.held;
}

function keepReached(pendingId: string, step: AddStep | null): void {
  if (step !== null && reachedSteps()[pendingId] === step) return;
  const held = { ...reachedSteps() };
  if (step === null) delete held[pendingId];
  else held[pendingId] = step;
  try {
    localStorage.setItem(REACHED_KEY, JSON.stringify(held));
  } catch {
    // A window that keeps nothing opens a pending add at its first pick.
  }
}

export function openAdd(): void {
  useAddFlow.setState({ ...fresh(), open: true });
}

/** A pending add opened again where it stands: an install still going, or one closed on its checks before a pick, on
 * its checks (the add's id is the pending add's); a computer whose setup started, on its running steps; else where it
 * was left, with its picks. An add the host holds no install for opens at Where. */
export function openPending(pending: PendingComputer, recipes: number): void {
  const job = useAdds.getState().jobs[pending.id];
  if (pending.placeId === undefined && job?.placeId === undefined) {
    useAddFlow.setState({ ...fresh(), open: true, address: pending.address, pendingId: pending.id, ...(job === undefined ? {} : { addId: pending.id, step: "checks" as const }) });
    return;
  }
  const placeId = pending.placeId ?? job!.placeId!;
  if (useStore.getState().places.find(p => p.id === placeId)?.setup !== undefined) return openSetup(placeId);
  const reached = reachedSteps()[pending.id];
  // Closed on its checks, it opens on them again, where Continue goes on to the picks.
  if (reached === undefined && job !== undefined) {
    useAddFlow.setState({ ...fresh(), open: true, address: pending.address, addId: pending.id, placeId, pendingId: pending.id, picks: pending.choices, from: pending.recipe ?? "here", step: "checks" });
    return;
  }
  const step = reached ?? firstPick(recipes);
  useAddFlow.setState({ ...fresh(), open: true, address: pending.address, placeId, pendingId: pending.id, picks: pending.choices, from: pending.recipe ?? "here", step: step === "startfrom" && recipes === 0 ? "agents" : step });
}

/** A computer being set up, opened on its running steps. */
export function openSetup(placeId: string): void {
  useAddFlow.setState({ ...fresh(), open: true, placeId, step: "running" });
}

/** What was said of each setup, by the run's id and the line: each line once a run, whichever road says it first. */
const told = new Set<string>();

const firstTold = (addId: string, line: "going" | "end"): boolean => {
  const key = `${line}:${addId}`;
  if (told.has(key)) return false;
  told.add(key);
  return true;
};

/** A setup's dialog closed, said where the person lands: one still running or waiting on a sign-in goes on, and says
 * so with what waits; one that ended says how it ended. A failed one says nothing more than its row. */
export function sayClosed(place: PlaceView): void {
  const setup = place.setup;
  if (setup === undefined || setup.state === "failed") return;
  // The host ends a setup's steps with a sign-in still waiting on the person: that setup is not over.
  const wait = setup.waiting[0];
  if (setup.state !== "running" && wait === undefined) return sayEnded(place);
  if (!firstTold(setup.addId, "going")) return;
  const name = placeName(place);
  const placeId = place.id;
  addNotice({ kind: "note", text: ADD_COMPUTER_WORDS.beingSetUp(name), detail: wait === undefined ? ADD_COMPUTER_WORDS.saysWhenReady : `${waitLine(wait)}.`, where: name, action: { word: "Open", run: () => openSetup(placeId) } });
}

/** A setup that ended, said once a run in the words the notification outside the app says it in: Needs you where a
 * folder or the GitHub sign-in did not land, else ready, with what it left and the computer's page to finish it on,
 * or the dialog's ready steps where it left nothing. */
export function sayEnded(place: PlaceView): void {
  if (place.setup === undefined || !firstTold(place.setup.addId, "end")) return;
  const name = placeName(place);
  const placeId = place.id;
  const state = placeWord(place, null);
  if (state.word === SETUP_WORDS.needsYou) return void addNotice({ kind: "note", text: setupNeedsYouLine(name, state.sentence ?? ""), where: name, action: { word: "Open", run: () => openSetup(placeId) } });
  const left = leftRows(setupRows(place, name));
  const open = (): void => {
    useSettingsStore.getState().go({ kind: "computer", id: placeId });
    useStore.getState().openSettings();
  };
  addNotice({ kind: "done", text: ADD_COMPUTER_WORDS.ready(name), where: name, ...(left.length === 0 ? { action: { word: "Open", run: () => openSetup(placeId) } } : { detail: ADD_COMPUTER_WORDS.left(left, name), action: { word: "Open", run: open } }) });
}

/** Closes the dialog: picks not yet kept are kept now, and the next open starts from what it opens on. */
export function closeAdd(): void {
  keepNow();
  useAddFlow.setState(fresh());
}

export function go(step: AddStep): void {
  const { pendingId } = useAddFlow.getState();
  useAddFlow.setState({ step });
  if (pendingId !== null && step !== "running" && step !== "ready") keepReached(pendingId, step);
}

/** Asks the host to add the computer over ssh, with the key the person trusted and the sudo password they typed,
 * where either was asked. */
export function connect(api: Api, address: string, hostKey?: string, sudoPassword?: string): void {
  const at = useAddFlow.getState();
  const key = hostKey ?? (at.address === address.trim() ? at.trustedKey : null) ?? undefined;
  const addId = addOverSsh(api, { address: address.trim(), ...(key === undefined ? {} : { hostKey: key }), ...(sudoPassword === undefined ? {} : { sudoPassword }) });
  useAddFlow.setState({ address: address.trim(), addId, step: "checks", trustedKey: key ?? null });
}

/** The key a computer this one never dialled answered with, on an add refused for it, for the person to trust. */
export const askedHostKey = (job: PlaceAddJob | undefined): string | undefined => (job?.state === "failed" && job.kind === PLACE_HOST_KEY_KIND ? job.hostKey : undefined);

/** Whether the add stopped for the password the login's sudo asks for, which the root row then asks for. */
export const askedSudo = (job: PlaceAddJob | undefined): boolean => job?.state === "failed" && job.kind === PLACE_SUDO_KIND;

/** Reads what the picks can be made from on every open: the host answers what it last read at once and reads again
 * behind it. Options already here stand through a refusal, which is said only where there is nothing to draw. */
export function readOptions(api: Api): void {
  if (api.recipesOptions === undefined) return;
  void api.recipesOptions().then(
    options => useAddFlow.setState({ options, optionsRefused: null }),
    (e: unknown) => useAddFlow.setState({ optionsRefused: failureOf(e) }),
  );
}

/** The setup a start answered, onto the computer's row at once: its frames fold onto that run, not the one before. */
function showSetup(answer: { place: PlaceView; setup?: PlaceSetup }): void {
  useStore.setState(s => ({ places: s.places.map(p => (p.id === answer.place.id ? { ...answer.place, ...(answer.setup === undefined ? {} : { setup: answer.setup }) } : p)) }));
}

/** How long picks rest before they are kept: a burst of ticks or a name typed is one write. */
const KEEP_AFTER_MS = 400;
let keeping: { timer: ReturnType<typeof setTimeout>; write: () => void } | undefined;

/** The write that waits, made now. */
function keepNow(): void {
  if (keeping === undefined) return;
  clearTimeout(keeping.timer);
  const { write } = keeping;
  keeping = undefined;
  write();
}

/** The picks as they now stand, kept on the pending add a moment after the last change, and said saved. */
export function setPicks(api: Api | null, picks: RecipeFile, from?: string): void {
  useAddFlow.setState(s => ({ picks, ...(from === undefined ? {} : { from }), saves: s.saves + 1, estimate: null }));
  const { placeId, from: started } = useAddFlow.getState();
  if (keeping !== undefined) clearTimeout(keeping.timer);
  const write = (): void => {
    if (api?.placesChoose === undefined || placeId === null) return;
    void api.placesChoose(placeId, picks, started === "here" || started === "none" ? undefined : started).catch(() => undefined);
  };
  keeping = {
    timer: setTimeout(() => {
      keeping = undefined;
      write();
    }, KEEP_AFTER_MS),
    write,
  };
}

/** Weighs the picks against the computer's room. An answer for picks that have moved on since is dropped, and a host
 * that would not weigh them leaves the summary on the room alone. */
export function weigh(api: Api, placeId: string, picks: RecipeFile): void {
  if (api.placesEstimate === undefined) return;
  void api.placesEstimate(placeId, picks).then(
    estimate => {
      if (useAddFlow.getState().picks === picks) useAddFlow.setState({ estimate });
    },
    () => undefined,
  );
}

/** Whether the picks with the room the setup keeps free need more than the computer has free, as last weighed. */
export const tooBig = (estimate: PlaceEstimate | null): boolean => estimate?.freeBytes !== undefined && estimate.freeBytes < estimate.neededBytes + estimate.keptBytes;

export function setSaveAs(next: Partial<SaveAs>): void {
  useAddFlow.setState(s => ({ saveAs: { ...s.saveAs, ...next } }));
}

/** Set up: the picks to the host, and the running steps; then, where the person asked, the picks saved as a recipe
 * the computer follows, its icon kept with the person's other looks. A recipe the host would not save is said as a
 * notice, since the setup it would have named is already running. */
export async function setUp(api: Api, setIcon: (slug: string, icon: ProjectIcon) => void): Promise<void> {
  const { placeId, picks, saveAs, pendingId } = useAddFlow.getState();
  if (placeId === null || picks === null || api.placesSetup === undefined) return;
  if (keeping !== undefined) clearTimeout(keeping.timer);
  keeping = undefined;
  useAddFlow.setState({ starting: true, refused: null });
  try {
    showSetup(await api.placesSetup(placeId, { choices: picks }));
  } catch (e) {
    useAddFlow.setState({ starting: false, refused: failureOf(e) });
    return;
  }
  if (pendingId !== null) keepReached(pendingId, null);
  useAddFlow.setState({ starting: false, step: "running" });
  if (!saveAs.on || saveAs.name.trim() === "" || api.recipesSave === undefined) return;
  await api.recipesSave(saveAs.name.trim(), { computer: placeId }).then(
    saved => setIcon(saved.slug, saveAs.icon),
    (e: unknown) => noticeFailure(e),
  );
}

/** Skip for now on one row of a computer's setup, a sign-in that waits or an item that failed: the host sets it aside
 * and answers the computer's row, which stands at once. */
export async function skipRow(api: Api | null, placeId: string, row: string): Promise<Failure | null> {
  if (api?.placesSkip === undefined) return null;
  try {
    showSetup({ place: await api.placesSkip(placeId, row) });
    return null;
  } catch (e) {
    return failureOf(e);
  }
}

/** Copy the keys on a computer that follows no recipe: its picks with the yes, and the server's row run again, which
 * puts on the servers that waited on it. */
export async function copyKeysOn(api: Api | null, place: Pick<PlaceView, "id" | "picks">, row: string): Promise<Failure | null> {
  if (api?.placesSetup === undefined || place.picks === undefined) return null;
  try {
    showSetup(await api.placesSetup(place.id, { choices: { ...place.picks, copyKeys: true }, row }));
    return null;
  } catch (e) {
    return failureOf(e);
  }
}

/** Retry on one row of a computer's setup: the host runs that row's step again, and every other row stands. */
export async function retrySetup(api: Api | null, placeId: string, row: string): Promise<Failure | null> {
  if (api?.placesSetup === undefined) return null;
  try {
    showSetup(await api.placesSetup(placeId, { row }));
    return null;
  } catch (e) {
    return failureOf(e);
  }
}

/** One agent's sign-in row run again on another way. The host sets a computer up from a saved recipe's own file, so a
 * computer that followed one follows its own picks from here. */
export async function setupWithWay(api: Api | null, place: PlaceView, agent: string, way: RecipeSignIn, row: string): Promise<Failure | null> {
  if (api?.placesSetup === undefined || place.picks === undefined) return null;
  const choices: RecipeFile = { ...place.picks, agents: { ...place.picks.agents, [agent]: { ...place.picks.agents[agent], signin: way } } };
  try {
    showSetup(await api.placesSetup(place.id, { choices, row }));
    return null;
  } catch (e) {
    return failureOf(e);
  }
}
