// SPDX-License-Identifier: AGPL-3.0-only
//! Turns folded into the threads the sidebar lists. Port of the functions of packages/protocol/src/views/session.ts that wsp-wire left to this crate, at
//! the cut (383708c78); that file's shapes are wsp-wire's. See spec/views_session.md.

use indexmap::IndexMap;

use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::session::{thread_key_of, thread_ran, SessionOrigin, SessionView, ThreadView};

use crate::words::turn::{opening_title, title_line};

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
    let mut by_thread: IndexMap<String, Vec<SessionView>> = IndexMap::new();
    for session in sessions {
        by_thread.entry(thread_key_of(session)).or_default().push(session.clone());
    }
    by_thread
        .into_iter()
        .map(|(id, turns)| {
            let first = &turns[0];
            let latest = &turns[turns.len() - 1];
            let title = if let Some(harness_title) = &latest.harness_title {
                title_line(harness_title)
            } else if let Some(prompt) = &first.prompt {
                opening_title(prompt)
            } else {
                first.claude_session_id.clone().unwrap_or_else(|| first.id.clone())
            };
            let said: Vec<f64> = turns.iter().filter_map(|turn| turn.cost_usd.as_ref().map(|cost| cost.0)).collect();
            let cost_usd = (!said.is_empty()).then(|| JsNumber(said.iter().fold(0.0, |sum, cost| sum + cost)));
            ThreadView {
                id,
                thread_id: first.thread_id.clone(),
                workspace_id: first.workspace_id.clone(),
                harness: first.harness.clone(),
                started_by: first.started_by.unwrap_or(SessionOrigin::Person),
                status: latest.status,
                title,
                session_id: latest.id.clone(),
                claude_session_id: latest.claude_session_id.clone(),
                started_at: latest.started_at,
                ended_at: latest.ended_at,
                cwd: latest.cwd.clone(),
                turns: JsInt::from(turns.len() as i64),
                ran: thread_ran(&turns),
                parent_thread_id: first.parent_thread_id.clone(),
                root_thread_id: first.root_thread_id.clone(),
                attempt: first.attempt.clone(),
                asking: latest.asking.clone(),
                waiting_on: latest.waiting_on.clone(),
                setup_refusal: latest.setup_refusal.clone(),
                sub: latest.sub.clone(),
                disk_kept_until: latest.disk_kept_until,
                disk_gone: latest.disk_gone.clone(),
                capped: latest.capped.clone(),
                limit: latest.limit.clone(),
                resume_at: latest.resume_at,
                cost_usd,
                last_line: latest.last_line.clone(),
                failure: latest.failure.clone(),
                pid: latest.pid,
                agent_pid: latest.agent_pid,
                read_at: latest.read_at,
                settled_at: latest.settled_at,
                pinned_at: latest.pinned_at,
                order: latest.order,
                folded_at: latest.folded_at,
                replaces: latest.replaces.clone(),
                replaced_by: latest.replaced_by.clone(),
                snoozed_until: latest.snoozed_until,
                woke_at: latest.woke_at,
                section: latest.section.clone(),
                rewound_at: latest.rewound_at,
                forked_from: latest.forked_from.clone(),
                permission_mode: latest.permission_mode.clone(),
                fast: (latest.fast == Some(true)).then_some(true),
                subagents: latest.subagents.clone().filter(|subagents| !subagents.is_empty()),
            }
        })
        .collect()
}
