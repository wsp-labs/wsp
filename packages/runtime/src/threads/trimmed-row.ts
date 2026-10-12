// SPDX-License-Identifier: AGPL-3.0-only
// The row a thread is listed by once every row of its own fell off the cap.
import type { SessionView } from "@wsp/protocol";
import type { RuntimeContext } from "../context.js";
import type { ThreadRecord } from "../types/internal.js";

/** The row a thread whose rows all fell off the cap is named by, built off its record and the transcript: its id is
 * the thread's, and no turn of it runs, since the cap never drops a running row. */
export function trimmedRow(ctx: RuntimeContext, threadId: string, r: ThreadRecord & { workspaceId: string }, ended = r.ended): SessionView {
  const resume = ctx.startedAs(r.workspaceId, threadId);
  const cwd = resume === undefined ? undefined : ctx.folderOf(r.workspaceId, resume);
  // Its first words name it and its last place it in time, as its row did.
  const words = ctx.transcriptIndex.get(r.workspaceId)?.words.get(threadId);
  return {
    id: threadId,
    threadId,
    workspaceId: r.workspaceId,
    harness: r.harness,
    status: ended ?? "completed",
    ...(words?.first !== undefined ? { prompt: words.first } : {}),
    ...(words !== undefined && words.last > 0 ? { startedAt: words.last, endedAt: words.last } : {}),
    ...(r.parentThreadId !== undefined ? { startedBy: "agent" as const, parentThreadId: r.parentThreadId } : {}),
    ...(r.rootThreadId !== undefined ? { rootThreadId: r.rootThreadId } : {}),
    ...(resume !== undefined ? { claudeSessionId: resume } : {}),
    ...(cwd !== undefined ? { cwd } : {}),
    ...(r.permissionMode !== undefined ? { permissionMode: r.permissionMode } : {}),
    ...(r.forkedFrom !== undefined ? { forkedFrom: r.forkedFrom } : {}),
  };
}
