// SPDX-License-Identifier: AGPL-3.0-only
// A Codex start that resumes a thread whose rollout the server does not hold,
// as on a new machine the kept file never reached: a new thread handed the
// thread so far, as Claude's adapter does.
import { LOST_SESSION_NOTE, lostSessionPrompt } from "@wsp/protocol";
import type { CodexSession, CodexStartOptions } from "./adapter.js";

/** What a server answers a resume of a thread its home holds no rollout file for, as 0.162.1 words it. */
const noRolloutLine = (threadId: string): string => `codex could not open the thread: no rollout found for thread id ${threadId}`;

/** A launch, and where it resumes a thread whose rollout the server does not hold and a seed is at hand, a second
 * launch on a new thread that is handed the thread so far, as Claude's start does: the failed resume is no turn of
 * the thread's, so its end is held back, and the new thread's note says what happened. */
export function startSeeded(launchTurn: (options: CodexStartOptions) => CodexSession, sessions: Map<string, CodexSession>, options: CodexStartOptions): CodexSession {
  const { resume, seed } = options;
  if (resume === undefined || seed === undefined || options.copy === true || options.fork !== undefined) return launchTurn(options);
  let lost = false;
  const first = launchTurn({
    ...options,
    onEvent: event => {
      if (event.type === "turn.done" && event.result.error === noRolloutLine(resume)) lost = true;
      else if (!(lost && event.type === "session.end")) options.onEvent(event);
    },
  });
  let current = first;
  const finished = first.finished.then(async result => {
    if (!lost) return result;
    const { resume: _gone, seed: _seed, ...fresh } = options;
    let noted = false;
    current = launchTurn({
      ...fresh,
      prompt: lostSessionPrompt(await seed(), options.prompt),
      onEvent: event => {
        options.onEvent(event);
        if (event.type !== "session.start" || noted) return;
        noted = true;
        options.onEvent({ type: "turn.delta", sessionId: event.sessionId, kind: "note", text: LOST_SESSION_NOTE });
      },
    });
    return current.finished;
  });
  const session: CodexSession = {
    localId: first.localId,
    get threadId() {
      return current.threadId;
    },
    get command() {
      return current.command;
    },
    get run() {
      return current.run;
    },
    get pid() {
      return current.pid;
    },
    get from() {
      return current.from;
    },
    finished,
    interrupt: () => current.interrupt(),
    kept: () => current.kept?.(),
    steer: prompt => current.steer(prompt),
    answer: (askId, answer) => current.answer(askId, answer),
    stopTask: task => current.stopTask(task),
  };
  sessions.set(first.localId, session);
  return session;
}
