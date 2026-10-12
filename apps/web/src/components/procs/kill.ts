// SPDX-License-Identifier: AGPL-3.0-only
// Ending a process from the table is two presses with no dialog: the first
// arms, the second within two seconds asks the daemon to end it and what it
// started. The daemon sends TERM and, to whatever is still there 2 s on, KILL,
// and answers once it is over, so the row offers nothing more until then. A
// daemon too old for that sends TERM to the pid alone; for it alone the row
// offers KILL once TERM has done nothing for five seconds, two presses again.
import { stillArmed } from "../arm.js";

export const ESCALATE_MS = 5000;

/** What the confirming press sends: the end the daemon carries out, or KILL to a daemon too old for it. */
export type KillSend = "end" | "force";

export type KillState = { step: "idle" } | { step: "armed"; at: number; send: KillSend } | { step: "sent" } | { step: "termed"; at: number };

export const IDLE: KillState = { step: "idle" };

/** A daemon too old to end what the process started took TERM for the pid alone at this time. */
export const termed = (at: number): KillState => ({ step: "termed", at });

/** Whether the row's button shows, what its press leads to, and whether the next press is the confirming one. null hides it. */
export function offered(state: KillState, now: number): { send: KillSend; confirm: boolean } | null {
  switch (state.step) {
    case "idle":
      return { send: "end", confirm: false };
    case "armed":
      return { send: state.send, confirm: stillArmed(state.at, now) };
    case "sent":
      return null;
    case "termed":
      return now - state.at >= ESCALATE_MS ? { send: "force", confirm: false } : null;
  }
}

export function press(state: KillState, now: number): { state: KillState; send: KillSend | undefined } {
  const offer = offered(state, now);
  if (offer === null) return { state, send: undefined };
  if (offer.confirm) return { state: { step: "sent" }, send: offer.send };
  return { state: { step: "armed", at: now, send: offer.send }, send: undefined };
}

/** Time passing: an unconfirmed arm lapses to where it was armed from; everything else keeps its state, the offer reads the clock. */
export function settle(state: KillState, now: number): KillState {
  if (state.step === "armed" && !stillArmed(state.at, now)) return state.send === "force" ? termed(now - ESCALATE_MS) : IDLE;
  return state;
}
