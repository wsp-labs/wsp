// SPDX-License-Identifier: AGPL-3.0-only
// What the key step of Add a cloud provider says about each provider's key:
// where it is made, the preset that holds every action wsp needs and those
// actions by name, and what the machines cost. One entry per provider.

export interface CloudKeyStep {
  /** The provider's API keys page, without the scheme, as a person reads it. */
  readonly keysPage: string;
  /** The scoped preset that holds every action below. */
  readonly preset: string;
  /** The actions wsp's setup and threads use. */
  readonly actions: readonly string[];
  readonly cost: string;
  /** The cheapest machine's rate, as the cost row's fact. */
  readonly from: string;
}

/** Boat's, by its provider id. The `ci` preset holds all fourteen actions, read off Boat's own key catalog
 * (`GET /api-keys`, 2026-10-13); the rate and the plan are Boat's pricing page, read the same day. */
export const CLOUD_KEY_STEPS: Readonly<Partial<Record<string, CloudKeyStep>>> = {
  box: {
    keysPage: "boat.dev/dashboard?tab=api-keys",
    preset: "ci",
    actions: ["sandbox.create", "sandbox.read", "sandbox.fork", "sandbox.resume", "sandbox.stop", "sandbox.delete-own", "sandbox.update", "exec", "file.read", "file.write", "host", "snapshot.read", "snapshot.write", "account.read"],
    cost: "Boat bills by the second, only while something runs. The $20 plan includes $20 of that time.",
    from: "from $0.018/h",
  },
};

/** The cloud offered where none is named: the first with a key step, Boat, which every build registers. */
export const FIRST_CLOUD: string = Object.keys(CLOUD_KEY_STEPS)[0] ?? "box";
