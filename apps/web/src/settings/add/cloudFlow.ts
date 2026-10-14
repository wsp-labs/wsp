// SPDX-License-Identifier: AGPL-3.0-only
// Add a cloud provider as the dialog walks it: the provider's key checked live,
// the image's picks off the host's own setup screens, the summary with the
// sign-ins on the computer the host runs on, then the image building and the
// ready page. A cloud's setup is the host's one init job, so an open while one
// stands opens it where it stands and never starts another.
import { create } from "zustand";
import { KEY_REFUSED, initJobBuilding, initJobOver, initShownScreens, type InitJob, type InitScreen, type InitScreenId, type ProviderKeyWorks } from "@wsp/protocol";
import type { Api } from "../../protocol/client.js";
import { failureOf, type Failure } from "../../protocol/failure.js";
import { useStore } from "../../protocol/store.js";
import { useSettingsStore } from "../settingsStore.js";
import { cloudsOffered } from "../providers.js";
import { FIRST_CLOUD } from "./cloudKey.js";

/** The host's screens the dialog draws as pick steps, in its order; the sign-ins screen is not among them, since
 * nothing signs in on a cloud machine, and the wsp tools screen is answered on Build. */
export const CLOUD_PICKS = ["agents", "tools", "also"] as const satisfies readonly InitScreenId[];
export type CloudPick = (typeof CLOUD_PICKS)[number];

export type CloudStep = "provider" | "key" | CloudPick | "summary" | "building" | "ready";

/** What a key that works read off the provider. */
export type KeyFacts = Omit<ProviderKeyWorks, "works">;

/** The live check of the key in the field: nothing typed yet, asking, working, or refused with whether the provider
 * answered about the key at all. */
export type KeyCheck = { state: "idle" } | { state: "checking" } | { state: "works"; facts: KeyFacts } | { state: "refused"; failure: Failure } | { state: "unreachable"; failure: Failure };

interface CloudFlowState {
  open: boolean;
  provider: string;
  step: CloudStep;
  check: KeyCheck;
}

const CLOSED: CloudFlowState = { open: false, provider: FIRST_CLOUD, step: "key", check: { state: "idle" } };

export const useCloudFlow = create<CloudFlowState>(() => CLOSED);

/** The setup a job is, for this provider: the job a card for it started, and no other. */
export const jobFor = (job: InitJob | null, provider: string): InitJob | null => (job !== null && job.place?.id === provider ? job : null);

/** The pick screens a job shows, in the dialog's order: a screen with nothing to pick is not shown. */
export const pickScreens = (job: InitJob | null): InitScreen[] => initShownScreens(job?.screens ?? []).filter(s => (CLOUD_PICKS as readonly string[]).includes(s.id));

/** The steps the dialog counts: the provider where more than one is offered, the key, the pick screens the job shows
 * (all three until it has read), the summary. */
export function cloudSteps(job: InitJob | null, offered = 1): CloudStep[] {
  const shown = job === null || job.phase !== "answering" ? [...CLOUD_PICKS] : pickScreens(job).map(s => s.id as CloudPick);
  return [...(offered > 1 ? (["provider"] as const) : []), "key", ...shown, "summary"];
}

/** Where a provider's setup stands, as the step the dialog opens on: its building view while the image builds or once
 * a build failed, the step the host stands at while the picks are being made, and the key with no setup at all. */
export function cloudStepOf(job: InitJob | null, provider: string): CloudStep {
  const mine = jobFor(job, provider);
  if (mine === null) return "key";
  if (initJobBuilding(mine.phase) || mine.phase === "failed") return "building";
  if (initJobOver(mine.phase)) return "key";
  const at = mine.screens[mine.step]?.id;
  if (mine.phase !== "answering" || at === undefined) return mine.phase === "answering" ? "summary" : "agents";
  return (CLOUD_PICKS as readonly string[]).includes(at) ? (at as CloudPick) : "summary";
}

/** Whether a setup for this provider is under way: picks being made, an image building, or a build that failed and
 * waits on the person. */
export const settingUpCloud = (job: InitJob | null, provider: string): boolean => {
  const mine = jobFor(job, provider);
  return mine !== null && (!initJobOver(mine.phase) || mine.phase === "failed");
};

/** Opens Add a cloud provider for `provider`, on the setup that stands for it where one does, else at its key. Named
 * by nobody, it opens on the setup that stands for any cloud the host offers, else on the provider step where it
 * offers more than one, else at the one cloud's key. */
export function openCloudAdd(provider?: string): void {
  const job = useStore.getState().initJob;
  const offered = cloudsOffered(useSettingsStore.getState().reads.setup);
  const id = provider ?? offered.find(p => settingUpCloud(job, p)) ?? offered[0] ?? FIRST_CLOUD;
  const choose = provider === undefined && offered.length > 1 && !settingUpCloud(job, id);
  useCloudFlow.setState({ open: true, provider: id, step: choose ? "provider" : cloudStepOf(job, id), check: { state: "idle" } });
}

export function closeCloudAdd(): void {
  useCloudFlow.setState(CLOSED);
}

export function goCloud(step: CloudStep): void {
  useCloudFlow.setState({ step });
}

/** The words a key that works reads as: the account, the plan and the machines running of the most it may, each
 * left out where the provider did not say it. */
export function keyWorksLine(facts: KeyFacts): string {
  const parts = [facts.account === "" ? undefined : `Account ${facts.account}`, facts.plan === "" ? undefined : facts.plan, facts.limit === 0 ? undefined : `${facts.running} of ${facts.limit} running at once`].filter(Boolean);
  return parts.length === 0 ? "Works." : `Works. ${parts.join(", ")}.`;
}

/** The key put to the provider. An answer for a key that has since changed in the field is dropped by the caller's
 * `current`, so a slow check never stands over a newer one. */
export async function checkKey(api: Api | null, provider: string, key: string, current: () => string): Promise<void> {
  const typed = key.trim();
  if (typed === "") return useCloudFlow.setState({ check: { state: "idle" } });
  if (api?.providersCheckKey === undefined) return;
  useCloudFlow.setState({ check: { state: "checking" } });
  try {
    const { account, plan, running, limit } = await api.providersCheckKey({ provider, key: typed });
    if (current().trim() === typed) useCloudFlow.setState({ check: { state: "works", facts: { account, plan, running, limit } } });
  } catch (e) {
    if (current().trim() !== typed) return;
    const failure = failureOf(e);
    useCloudFlow.setState({ check: failure.kind === KEY_REFUSED ? { state: "refused", failure } : { state: "unreachable", failure } });
  }
}
