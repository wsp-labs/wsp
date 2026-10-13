// SPDX-License-Identifier: AGPL-3.0-only
// How long a turn on a machine whose credentials this host keeps waited for it, written into the thread after the
// wait: nothing can show while the host is away, since the app is a window onto it.
import { WAITED_ROW_MS } from "@wsp/protocol";
import type { SessionEntry } from "../types/internal.js";
import type { RuntimeContext, WaitsArea } from "../context.js";

/** How long after one hold ends another of the same thread's still counts as the same wait: every request held while
 * the host was away takes a pipe the moment it is back, in no set order, and the longest of them is the wait. */
export const WAIT_SETTLE_MS = 2_000;

/** A hold of WAITED_ROW_MS or less writes nothing; holds ending together are one row of the longest; a sentence is one
 * row per turn however many requests it answered, without the "wsp: " that leads it in the agent's own error line. */
export function waitsArea(ctx: RuntimeContext): WaitsArea {
  const { clock, sessions } = ctx;
  const settling = new Map<string, { ms: number }>();
  /** The last sentence each turn's row said, held as long as the runtime holds the turn. */
  const said = new WeakMap<SessionEntry, string>();

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
  return { noteWaited };
}
