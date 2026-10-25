// SPDX-License-Identifier: AGPL-3.0-only
//! Turns folded into the threads the sidebar lists. Port of the functions of packages/protocol/src/views/session.ts that wsp-wire left to this crate, at
//! a3b870a6e; that file's shapes are wsp-wire's. See spec/views_session.md.

use wsp_wire::views::session::SessionView;
use wsp_wire::views::session::ThreadView;

/// Folds the session index into threads, in the order each thread's first turn appears. The one place a row from
/// before provenance was recorded is read as a person's; clients print the answer and never decide it.
// session.ts:380
// export function foldThreads(sessions: ReadonlyArray<SessionView>): ThreadView[] {
//   const byThread = new Map<string, SessionView[]>();
//   for (const session of sessions) {
//     const key = threadKeyOf(session);
//     const turns = byThread.get(key);
//     if (turns === undefined) byThread.set(key, [session]);
//     else turns.push(session);
//   }
//   return [...byThread].map(([id, turns]) => {
//     const first = turns[0]!;
//     const latest = turns[turns.length - 1]!;
//     const spent = threadCost(turns);
//     // The one title rule every client reads: the harness's own name for the session the next send resumes wins, so
//     // a rename made inside the harness shows here, and the opening turn's first sentence stands until one is read.
//     const title = latest.harnessTitle !== undefined ? titleLine(latest.harnessTitle) : first.prompt !== undefined ? openingTitle(first.prompt) : first.claudeSessionId ?? first.id;
//     return {
//       id,
//       ...(first.threadId !== undefined ? { threadId: first.threadId } : {}),
//       workspaceId: first.workspaceId,
//       harness: first.harness,
//       startedBy: first.startedBy ?? "person",
//       status: latest.status,
//       title,
//       sessionId: latest.id,
//       ...(latest.claudeSessionId !== undefined ? { claudeSessionId: latest.claudeSessionId } : {}),
//       ...(latest.startedAt !== undefined ? { startedAt: latest.startedAt } : {}),
//       ...(latest.endedAt !== undefined ? { endedAt: latest.endedAt } : {}),
//       ...(latest.cwd !== undefined ? { cwd: latest.cwd } : {}),
//       ...(latest.asking !== undefined ? { asking: latest.asking } : {}),
//       ...(latest.waitingOn !== undefined ? { waitingOn: latest.waitingOn } : {}),
//       ...(latest.setupRefusal !== undefined ? { setupRefusal: latest.setupRefusal } : {}),
//       ...(latest.sub !== undefined ? { sub: latest.sub } : {}),
//       ...(latest.diskKeptUntil !== undefined ? { diskKeptUntil: latest.diskKeptUntil } : {}),
//       ...(latest.diskGone !== undefined ? { diskGone: latest.diskGone } : {}),
//       ...(latest.capped !== undefined ? { capped: latest.capped } : {}),
//       ...(latest.limit !== undefined ? { limit: latest.limit } : {}),
//       ...(latest.resumeAt !== undefined ? { resumeAt: latest.resumeAt } : {}),
//       ...(latest.lastLine !== undefined ? { lastLine: latest.lastLine } : {}),
//       ...(latest.failure !== undefined ? { failure: latest.failure } : {}),
//       ...(latest.pid !== undefined ? { pid: latest.pid } : {}),
//       ...(latest.agentPid !== undefined ? { agentPid: latest.agentPid } : {}),
//       ...(latest.readAt !== undefined ? { readAt: latest.readAt } : {}),
//       ...(latest.settledAt !== undefined ? { settledAt: latest.settledAt } : {}),
//       ...(latest.pinnedAt !== undefined ? { pinnedAt: latest.pinnedAt } : {}),
//       ...(latest.order !== undefined ? { order: latest.order } : {}),
//       ...(latest.foldedAt !== undefined ? { foldedAt: latest.foldedAt } : {}),
//       ...(latest.replaces !== undefined ? { replaces: latest.replaces } : {}),
//       ...(latest.replacedBy !== undefined ? { replacedBy: latest.replacedBy } : {}),
//       ...(latest.snoozedUntil !== undefined ? { snoozedUntil: latest.snoozedUntil } : {}),
//       ...(latest.wokeAt !== undefined ? { wokeAt: latest.wokeAt } : {}),
//       ...(latest.section !== undefined ? { section: latest.section } : {}),
//       ...(latest.rewoundAt !== undefined ? { rewoundAt: latest.rewoundAt } : {}),
//       ...(latest.forkedFrom !== undefined ? { forkedFrom: latest.forkedFrom } : {}),
//       ...(latest.permissionMode !== undefined ? { permissionMode: latest.permissionMode } : {}),
//       ...(latest.fast === true ? { fast: true } : {}),
//       ...(latest.subagents !== undefined && latest.subagents.length > 0 ? { subagents: latest.subagents } : {}),
//       turns: turns.length,
//       ran: threadRan(turns),
//       ...(first.parentThreadId !== undefined ? { parentThreadId: first.parentThreadId } : {}),
//       ...(first.rootThreadId !== undefined ? { rootThreadId: first.rootThreadId } : {}),
//       ...(first.attempt !== undefined ? { attempt: first.attempt } : {}),
//       ...(spent !== undefined ? { costUsd: spent } : {}),
//     };
//   });
// }
pub fn fold_threads(sessions: &[SessionView]) -> Vec<ThreadView> {
    todo!()
}
