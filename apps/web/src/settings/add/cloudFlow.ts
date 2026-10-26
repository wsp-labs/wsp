// SPDX-License-Identifier: AGPL-3.0-only
// Add a cloud provider as the dialog walks it: the provider where more than one
// is offered, then its key checked live and saved.
import { create } from "zustand";
import { KEY_REFUSED, type ProviderKeyWorks } from "@wsp/protocol";
import type { Api } from "../../protocol/client.js";
import { failureOf, type Failure } from "../../protocol/failure.js";
import { useSettingsStore } from "../settingsStore.js";
import { cloudsOffered } from "../providers.js";
import { FIRST_CLOUD } from "./cloudKey.js";

export type CloudStep = "provider" | "key";

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

/** Opens Add a cloud provider for `provider`. Named by nobody, it opens on the provider step where the host offers
 * more than one, else at the one cloud's key. */
export function openCloudAdd(provider?: string): void {
  const offered = cloudsOffered(useSettingsStore.getState().reads.setup);
  const id = provider ?? offered[0] ?? FIRST_CLOUD;
  const choose = provider === undefined && offered.length > 1;
  useCloudFlow.setState({ open: true, provider: id, step: choose ? "provider" : "key", check: { state: "idle" } });
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
