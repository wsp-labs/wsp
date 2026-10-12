// SPDX-License-Identifier: AGPL-3.0-only
// A start that resumes a session the agent's own store no longer holds, as on
// a new machine its file never reached: the failed resume is no turn of the
// thread's, so its end is held back, and a second launch in a new session is
// handed the thread so far, its note saying what happened. Each adapter wraps
// the session running at the moment in its own answer.
import type { AdapterEvent } from "./adapter-port.js";
import type { TurnResult } from "./views/session-events.js";
import { LOST_SESSION_NOTE, lostSessionPrompt } from "./words/turn.js";

/** What a start that may resume a lost session takes. */
export interface LostSessionStart {
  prompt: string;
  resume?: string;
  seed?: () => Promise<string>;
  onEvent: (event: AdapterEvent) => void;
}

/** The first launch, the session running at the moment, and the turn's end, which is the second launch's where the
 * first ended with `lost`, the agent's own words for a session its store does not hold. */
export function startAfterLoss<O extends LostSessionStart, S extends { readonly finished: Promise<TurnResult> }>(
  options: O & { resume: string; seed: () => Promise<string> },
  lost: string,
  launch: (options: O) => S,
): { first: S; now: () => S; finished: Promise<TurnResult> } {
  let gone = false;
  const first = launch({
    ...options,
    onEvent: event => {
      if (event.type === "turn.done" && event.result.error === lost) gone = true;
      else if (!(gone && event.type === "session.end")) options.onEvent(event);
    },
  });
  let current = first;
  const finished = first.finished.then(async result => {
    if (!gone) return result;
    const { resume: _gone, seed: _seed, ...fresh } = options;
    let noted = false;
    current = launch({
      ...(fresh as unknown as O),
      prompt: lostSessionPrompt(await options.seed(), options.prompt),
      onEvent: event => {
        options.onEvent(event);
        if (event.type !== "session.start" || noted) return;
        noted = true;
        options.onEvent({ type: "turn.delta", sessionId: event.sessionId, kind: "note", text: LOST_SESSION_NOTE });
      },
    });
    return current.finished;
  });
  return { first, now: () => current, finished };
}
