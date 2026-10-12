// SPDX-License-Identifier: AGPL-3.0-only
// The one question a settle puts when the threads it takes still run processes
// beside their agents. The host refuses a settle sent with ask while any run,
// carrying them; the person stops them or keeps them running, and the settle
// goes again with that answer. Escape settles nothing.
import { create } from "zustand";
import { procName, standingAfterStop, type ActivityEnded, type LeftThread, type ProcessesChoice, type SessionSettleResult } from "@wsp/protocol";
import { ACTIVITY_WORDS as W } from "../../settings/format.js";

export interface SettleAsk {
  readonly left: readonly LeftThread[];
  /** Sends the settle again with the person's answer. */
  readonly settle: (choice: ProcessesChoice) => Promise<SessionSettleResult>;
  /** What the settle answered, or undefined where the person cancelled. */
  readonly done: (answer: SessionSettleResult | undefined) => void;
}

export const useSettleAsk = create<{ ask: SettleAsk | null }>(() => ({ ask: null }));

/** Asks what to do with the processes a settle was refused over and settles as answered; undefined where it was
 * cancelled. A second ask cancels the first. */
export function askSettleProcesses(left: readonly LeftThread[], settle: SettleAsk["settle"]): Promise<SessionSettleResult | undefined> {
  useSettleAsk.getState().ask?.done(undefined);
  return new Promise(resolve => {
    const ask: SettleAsk = {
      left,
      settle,
      done: answer => {
        if (useSettleAsk.getState().ask === ask) useSettleAsk.setState({ ask: null });
        resolve(answer);
      },
    };
    useSettleAsk.setState({ ask });
  });
}

/** The sentence naming what a stop left running, or null where it ended everything. */
export function standingLine(ended: readonly ActivityEnded[] | undefined): string | null {
  const standing = standingAfterStop(ended ?? []);
  return standing.length === 0 ? null : W.standing(standing.map(p => `${p.pid} ${procName({ comm: p.name, cmdline: p.command })}`));
}
