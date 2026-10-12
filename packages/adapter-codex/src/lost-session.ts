// SPDX-License-Identifier: AGPL-3.0-only
// A Codex start that resumes a thread whose rollout the server does not hold,
// as on a new machine the kept file never reached: a new thread handed the
// thread so far, through the same road as Claude's adapter.
import { startAfterLoss } from "@wsp/protocol";
import type { CodexSession, CodexStartOptions } from "./adapter.js";

/** What a server answers a resume of a thread its home holds no rollout file for, as 0.162.1 words it. */
const noRolloutLine = (threadId: string): string => `codex could not open the thread: no rollout found for thread id ${threadId}`;

/** A launch, and where it resumes a thread whose rollout the server does not hold and a seed is at hand, a second
 * launch on a new thread handed the thread so far; the session answered is the one running at the moment. */
export function startSeeded(launchTurn: (options: CodexStartOptions) => CodexSession, sessions: Map<string, CodexSession>, options: CodexStartOptions): CodexSession {
  const { resume, seed } = options;
  if (resume === undefined || seed === undefined || options.copy === true || options.fork !== undefined) return launchTurn(options);
  const { first, now, finished } = startAfterLoss({ ...options, resume, seed }, noRolloutLine(resume), launchTurn);
  const session: CodexSession = {
    localId: first.localId,
    get threadId() {
      return now().threadId;
    },
    get command() {
      return now().command;
    },
    get run() {
      return now().run;
    },
    get pid() {
      return now().pid;
    },
    get from() {
      return now().from;
    },
    finished,
    interrupt: () => now().interrupt(),
    kept: () => now().kept?.(),
    steer: prompt => now().steer(prompt),
    answer: (askId, answer) => now().answer(askId, answer),
    stopTask: task => now().stopTask(task),
  };
  sessions.set(first.localId, session);
  return session;
}
