// SPDX-License-Identifier: AGPL-3.0-only
import type { GoldenStage, InitSetup, WorkspaceSize } from "../index.js";
import { PROVIDER_KEY_WORDS, providerKeyName } from "../place-word.js";
import { fmtBytes, fmtSize, KIB } from "./units.js";
/** Every word of the steps that choose and build the image, drawn under a computer's Image card and, where a word is
 * the host's, in the terminal too. Headlines are a step's title, tops the one sentence under it, keycaps the one
 * primary button a step has. Nothing here asks the person to run a command, and nothing here is a word for a
 * computer or a provider: those are PLACES_WORDS. */
export const CLOUD_SETUP_WORDS = {
  choice: {
    headline: "What goes on your image",
    top: "What your agents need goes on one image, built once and copied for every task",
    manual: "Choose what goes on the image",
    agent: "Let an agent choose from your usage",
    agentWith: "with",
    /** An agent here whose thread wsp cannot hand the recipe tools at launch: shown, never offered. */
    noTools: "no wsp tools yet",
    /** What the agent row says with no agent on this computer at all. */
    none: "no agent here",
    keycap: "Continue",
  },
  keys: {
    saved: "saved",
    unset: "not set",
    /** Under the field when the provider answered and refused what was typed; its own status and word follow. */
    refused: (provider: string): string => `${providerKeyName(provider)} refused this key`,
    /** The build's line for a key already saved that the provider refuses, read before the first stage, naming that
     * provider by PROVIDER_KEY_WORDS. */
    refusedSaved: (provider: string): string => `${providerKeyName(provider)} refused the saved key`,
    /** Under the field when nothing came back about the key at all; what this computer saw follows. */
    unchecked: (provider: string): string => `${providerKeyName(provider)} could not be reached to check the key`,
    /** What the build offers when the saved key was refused: that provider's key field, not another build. */
    changeKey: "Change the key",
  },
  screen: {
    /** A row wsp puts on every image, so its tick cannot be taken off. */
    alwaysOn: "always on",
    /** What that row says in place of always on once it is pressed. */
    alwaysOnWhy: "wsp needs it on every image, so it stays on.",
    keycap: "Continue",
    back: "Back",
    again: "Start over",
    /** A sign-in row whose tool is off the image: its picker is fixed on skip. */
    notOnImage: "not on the image",
    /** A sign-in row the catalog locked out. */
    leftAlone: "left alone",
    /** A sign-in row whose tool stops on a question nobody but the person can answer, so the machine is no road for it. */
    asksYou: "asks questions only you can answer",
  },
  build: {
    headline: "Building your image",
    top: "The computer starts, installs what you ticked and is saved as the image every task starts from",
    /** The one stage row the sign-ins fold into, its sub-rows one per sign-in. */
    signingIn: "Signing in on the computer",
    /** The slide the build becomes while that stage runs: room to act on each sign-in. */
    slideHeadline: "Sign in on the computer",
    slideTop: "Each one opens a page on this computer, and the image keeps the sign-in",
    open: "Open sign-in",
    retry: "Retry",
    codeAsk: "Paste the code from the page",
    codeSubmit: "Submit",
    cancel: "Cancel the build",
    /** Why Cancel is held while the seal runs: the host's refusal and the line under the build, one sentence. */
    cannotStop: "The image is being saved. The snapshot and the save cannot be stopped.",
    cancelSure: "Stop the build",
    cancelWhy: "The computer it was building on is thrown away and nothing is saved",
    cancelKeep: "Keep building",
    done: "Your image is ready",
    failed: "The build stopped",
    /** The headline of a build the person stopped, so the screen never reads as the machine's doing. */
    stopped: "You stopped the build",
    again: "Start over",
  },
  agent: {
    headline: "Reading what your agents used",
    top: "Your agent reads this computer and writes the recipe the next screens start from",
    /** What the thread this road opens is called, which is what the sidebar's row for it reads. */
    title: "Build your image",
    /** The link to the thread doing the work, which the sidebar focuses. */
    open: "Open the thread",
    /** The headline once the turn ended without the recipe, and the two ways on from there. */
    failed: "Your agent stopped",
    /** The sentence under it when nothing named a reason, which is a job stopped from another client. */
    stopped: "The thread ended before the recipe was written",
    retry: "Retry",
    again: "Start over",
    /** What the block under the title says before the thread's first line. */
    waiting: "waiting for the thread's first line",
  },
  reading: {
    headline: "Reading this computer",
    top: "What is installed here and what your agents used decides what the image starts with",
    /** The one row the card shows until the first fact lands, so the work reads as started. */
    first: "This computer",
  },
} as const;

/** Every build stage in plain words, the one table the app's rows and the terminal's lines read. */
export const GOLDEN_STAGE_WORDS: Record<Exclude<GoldenStage, "failed">, string> = {
  creating: "Creating the machine",
  "deploying-daemon": "Installing the base tools",
  "applying-setup": "Applying your setup",
  "uploading-files": "Copying your files",
  "installing-harness": "Installing agents",
  "installing-tools": "Installing tools",
  "installing-mcp": "Installing MCP servers",
  ready: "Checking the machine answers",
  snapshotting: "Taking the snapshot",
  promoting: "Saving the image",
  "smoke-forking": "Checking a fork boots",
  sealed: "Finishing",
};

/** The snapshot stage's first line, while the guest's dirty pages are written to its disk ahead of the copy. */
export const DISK_SYNC_LINE = "syncing the disk";

/** The snapshot stage's line when the guest's counters read non-zero once the sync returned, in kB as /proc/meminfo
 * counts them. The snapshot goes ahead; nothing waits on a writer. */
export const diskUnsettledLine = (reading: { dirtyKb: number; writebackKb: number }): string =>
  `synced, Dirty ${fmtBytes(reading.dirtyKb * KIB)} and Writeback ${fmtBytes(reading.writebackKb * KIB)} remain; a writer is still running`;

/** A snapshot refused because the guest's sync did not succeed, with the machine's own answer. */
export const diskSyncFailedLine = (answer: string): string => `the disk could not be synced (${answer}); nothing was snapshotted`;

/** The snapshot stage's one line: what is being snapshotted in words a person can use, never the snapshot's name,
 * and the size when the builder's disk could be read. */
export const snapshotStageLine = (bytes: number | undefined): string => `snapshotting${bytes === undefined ? "" : ` about ${fmtBytes(bytes)}`}, usually under a minute`;

/** The snapshot stage's line while the layer is written: the bytes so far, over what the machine had written once
 * the backend has counted it. The stage's own clock says how long it has taken. */
export const snapshotProgressLine = (bytes: number, total: number | undefined): string =>
  total === undefined ? `snapshotting, ${fmtBytes(bytes)} written` : `snapshotting, ${fmtBytes(bytes)} of about ${fmtBytes(total)} written`;

/** The save stage's one line. */
export const SAVING_IMAGE_LINE = "saving the image";

/** A stage's line for a machine its rollback could not remove, with the provider's own refusal: it goes in the stage's
 * block, never in the headline, which stays the failure's own sentence; the machine's row carries the retries. */
export const machineLeftLine = (reason: string): string => `the machine could not be removed and bills on: ${reason}`;

/** The failures a provider client raises when the network is gone rather than when the provider refused. */
const OFFLINE_FAILURE = /^fetch failed$|\b(ENOTFOUND|EAI_AGAIN|ECONNREFUSED|ENETDOWN|ENETUNREACH|EHOSTUNREACH)\b/;

/** What a stopped build says when every part of the error is the network going away. */
const NETWORK_LOST_LINE = "This computer lost its network";

/** The sentence a stopped build shows for what happened: this computer's own word when every part of the error is
 * the network going away, else the error's parts said once. A run that failed the same way twice reports it twice;
 * the person reads one reason, not a list. */
export function initStoppedLine(error: string): string {
  const parts = [...new Set(error.split(";").map(p => p.trim()).filter(p => p !== ""))];
  if (parts.length === 0) return error;
  return parts.every(p => OFFLINE_FAILURE.test(p)) ? NETWORK_LOST_LINE : parts.join("; ");
}

/** What the app says when it needs the person: the toast's opening and a system notification's title. */
export const NEEDS_YOU = "wsp needs you";

/** What a window or tab title leads with while a need stands, so a person reading only the title sees it. */
export const NEEDS_YOU_MARK = "\u2022 ";

/** The title with the mark on it while a need stands and without it otherwise, from a title that may already carry
 * one: the same title goes through this on every change, so the mark can never double or stick. */
export function titleWithNeed(title: string, needed: boolean): string {
  const plain = title.startsWith(NEEDS_YOU_MARK) ? title.slice(NEEDS_YOU_MARK.length) : title;
  return needed ? `${NEEDS_YOU_MARK}${plain}` : plain;
}

/** The provider's own answer about a key: the status it replied with and the word it used, so a person reads whose
 * refusal they are looking at rather than ours. */
export function providerSaidLine(status: number, said: string): string {
  return said === "" ? String(status) : `${status} ${said}`;
}

/** Under the keys field when the provider answered and refused the key typed there; `provider` is its id. */
export function keyRefusedLine(said: string, provider: string): string {
  return `${CLOUD_SETUP_WORDS.keys.refused(provider)}: ${said}`;
}

/** The same for a key already saved, which is what the build reads before its first stage; `provider` is the id of
 * the provider that refused it. */
export function savedKeyRefusedLine(said: string, provider: string): string {
  return `${CLOUD_SETUP_WORDS.keys.refusedSaved(provider)}: ${said}`;
}

/** Either place when nothing came back about the key at all: what this computer saw instead, with Try again beside it. */
export function keyUncheckedLine(said: string, provider: string): string {
  return `${CLOUD_SETUP_WORDS.keys.unchecked(provider)}: ${said}`;
}

/** What the machine the build boots costs, said once under the key screen's title from the backend's own rate. */
export function initCostLine(size: WorkspaceSize, rateUsdPerHour: number): string {
  return `A ${fmtSize(size)} workspace costs about $${rateUsdPerHour.toFixed(2)} an hour while it runs and naps when idle`;
}

/** The cloud setup as wsp setup prints it: which keys are held and the price of the machine the build boots. */
export function initSetupLines(setup: InitSetup): string[] {
  const held = (yes: boolean): string => (yes ? CLOUD_SETUP_WORDS.keys.saved : CLOUD_SETUP_WORDS.keys.unset);
  const lines = Object.entries(setup.keys).map(([provider, yes]) => `${PROVIDER_KEY_WORDS[provider]?.keyName ?? provider}: ${held(yes)}`);
  if (setup.pricing !== null) lines.push(initCostLine(setup.pricing.size, setup.pricing.rateUsdPerHour));
  return lines;
}
