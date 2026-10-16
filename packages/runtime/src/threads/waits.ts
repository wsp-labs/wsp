// SPDX-License-Identifier: AGPL-3.0-only
// How long a turn on a machine whose credentials this host keeps waited for it, written into the thread after the
// wait: nothing can show while the host is away, since the app is a window onto it. And the retries of its model calls
// the host saw fail, which its agent may never say.
import { WAITED_ROW_MS } from "@wsp/protocol";
import type { SessionEntry } from "../types/internal.js";
import type { RuntimeContext, WaitsArea } from "../context.js";

/** How long after one hold ends another of the same thread's still counts as the same wait: every request held while
 * the host was away takes a pipe the moment it is back, in no set order, and the longest of them is the wait. */
export const WAIT_SETTLE_MS = 2_000;

/** How long a retry the host saw waits for the agent to say it: Claude Code prints its own the moment a call fails, for
 * every call but a background subagent's, and the line crosses the machine's link after the host saw the failure. */
export const RETRY_SETTLE_MS = 2_000;

/** A hold of WAITED_ROW_MS or less writes nothing; holds ending together are one row of the longest; a sentence is one
 * row per turn however many requests it answered, without the "wsp: " that leads it in the agent's own error line. */
export function waitsArea(ctx: RuntimeContext): WaitsArea {
  const { clock, sessions } = ctx;
  const settling = new Map<string, { ms: number }>();
  /** The last sentence each turn's row said, held as long as the runtime holds the turn. */
  const said = new WeakMap<SessionEntry, string>();
  /** The retries the host saw in each turn that its agent has not said yet, by turn, in the order seen. */
  const unsaid = new Map<string, Array<{ attempt: number; why: string }>>();
  const drop = (turnId: string, i: number): boolean => {
    const notes = unsaid.get(turnId);
    if (notes === undefined || i < 0) return false;
    notes.splice(i, 1);
    if (notes.length === 0) unsaid.delete(turnId);
    return true;
  };
  /** The agent's own line stands for the newest note of its reason, else the newest: it says a retry moments after the
   * host saw that call fail, and an older note may be a background subagent's, which it never says. */
  const agentRetried = (turnId: string, why?: string): void => {
    const notes = unsaid.get(turnId) ?? [];
    let i = notes.length - 1;
    for (let j = i; j >= 0; j--) {
      if (notes[j]!.why !== why) continue;
      i = j;
      break;
    }
    drop(turnId, i);
  };

  const latestTurn = (threadId: string): SessionEntry | undefined => {
    let latest: SessionEntry | undefined;
    for (const s of sessions.values()) {
      if (s.view.threadId === threadId && (latest === undefined || (s.view.startedAt ?? 0) >= (latest.view.startedAt ?? 0))) latest = s;
    }
    return latest;
  };
  const write = (threadId: string, ms: number, text?: string): void => {
    const s = latestTurn(threadId);
    if (s === undefined || ctx.state.closing) return;
    if (text !== undefined) {
      if (said.get(s) === text) return;
      said.set(s, text);
    }
    ctx.record({
      type: "thread.waited",
      workspaceId: s.view.workspaceId,
      sessionId: s.view.claudeSessionId ?? s.turnId,
      turnId: s.turnId,
      threadId,
      ms: Math.max(0, Math.round(ms)),
      ...(text !== undefined ? { said: text } : {}),
    });
  };

  const noteWaited = async (threadId: string, ms: number, text?: string): Promise<void> => {
    if (text !== undefined) return write(threadId, ms, text.replace(/^wsp: /, ""));
    if (!(ms > WAITED_ROW_MS)) return;
    const held = settling.get(threadId);
    if (held !== undefined) {
      held.ms = Math.max(held.ms, ms);
      return;
    }
    const wait = { ms };
    settling.set(threadId, wait);
    clock.schedule(
      () => {
        settling.delete(threadId);
        write(threadId, wait.ms);
      },
      WAIT_SETTLE_MS,
      { unref: true },
    );
  };
  /** No line of its own: the turn's run log holds only the agent's retries, and a host re-reading it after a restart
   * counts the lines it wrote of those. */
  const noteRetry = (threadId: string, retry: { attempt: number; why: string }): void => {
    const s = latestTurn(threadId);
    if (s === undefined || s.view.status !== "running" || ctx.state.closing) return;
    const { turnId } = s;
    const note = { attempt: retry.attempt, why: retry.why };
    unsaid.set(turnId, [...(unsaid.get(turnId) ?? []), note]);
    clock.schedule(
      () => {
        if (!drop(turnId, unsaid.get(turnId)?.indexOf(note) ?? -1) || s.view.status !== "running" || ctx.state.closing) return;
        ctx.record({ type: "session.retry", workspaceId: s.view.workspaceId, sessionId: s.view.claudeSessionId ?? turnId, turnId, threadId, attempt: note.attempt, why: note.why });
      },
      RETRY_SETTLE_MS,
      { unref: true },
    );
  };
  return { noteWaited, noteRetry, agentRetried };
}
