// SPDX-License-Identifier: AGPL-3.0-only
//! The one vocabulary for a thread's state, read off the rows the runtime writes, and when a read thread settles.
//! Port of packages/protocol/src/thread-state.ts at a3b870a6e. See spec/thread_state.md.

use wsp_wire::views::session::{SubagentState, ThreadView};
use wsp_wire::views::session_events::SessionPermissionEvent;

// thread-state.ts:14
// export type ThreadState = SessionStatus | "waiting" | "done";
/// SessionStatus's words, then "waiting" and "done" (thread-state.ts:12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadState {
    Running,
    Completed,
    Interrupted,
    Failed,
    Waiting,
    Done,
}

/// The latest moment no window has shown the thread since, of its latest turn's end and its snooze's end, off the read
/// stamp the host keeps per thread; undefined once a window has shown both, and on a thread still working. What a
/// window showing the thread stamps past.
// thread-state.ts:19
// export function threadUnseenAt(thread: Pick<ThreadView, "status" | "endedAt" | "readAt" | "wokeAt">): number | undefined {
//   if (thread.status === "running") return undefined;
//   const unseen = [thread.endedAt, thread.wokeAt].filter((at): at is number => at !== undefined && (thread.readAt === undefined || thread.readAt < at));
//   return unseen.length === 0 ? undefined : Math.max(...unseen);
// }
pub fn thread_unseen_at(thread: &ThreadView) -> Option<f64> {
    todo!()
}

/// Whether the thread's latest turn ended, or its snooze ended, and no window has shown the thread since. A failed
/// turn is not a finish: it reads Failed whether it was seen or not.
// thread-state.ts:27
// export function threadUnread(thread: Pick<ThreadView, "status" | "endedAt" | "readAt" | "wokeAt">): boolean {
//   return (thread.status === "completed" || thread.status === "interrupted") && threadUnseenAt(thread) !== undefined;
// }
pub fn thread_unread(thread: &ThreadView) -> bool {
    todo!()
}

/// Whether the thread waits on the person: it asks, or is stopped behind a thread that asks, or a finish, a failure or
/// a snooze's end sits there that no window has shown, or a sub-thread's work not pushed holds its machine. A turn the
/// person stopped asks nothing of them, so its end does not count. What the dock's count, the jump to the next thread and the menu bar read.
// thread-state.ts:34
// export function threadNeedsYou(thread: Pick<ThreadView, "status" | "asking" | "waitingOn" | "endedAt" | "readAt" | "wokeAt"> & { sub?: Pick<SubRow, "held"> | undefined }): boolean {
//   if (thread.asking !== undefined || thread.waitingOn !== undefined) return true;
//   // A sub-thread done with work not pushed holds a machine that costs money until someone acts, read or not.
//   if (thread.sub?.held !== undefined && thread.status !== "running") return true;
//   return threadUnseenAt(thread.status === "interrupted" ? { ...thread, endedAt: undefined } : thread) !== undefined;
// }
pub fn thread_needs_you(thread: &ThreadView) -> bool {
    todo!()
}

/// What a thread reads as, from its folded row alone.
// thread-state.ts:42
// export function threadState(thread: Pick<ThreadView, "status" | "asking" | "waitingOn" | "endedAt" | "readAt" | "wokeAt">): ThreadState {
//   if (thread.asking !== undefined || thread.waitingOn !== undefined) return "waiting";
//   return threadUnread(thread) ? "done" : thread.status;
// }
pub fn thread_state(thread: &ThreadView) -> ThreadState {
    todo!()
}

/// What a waiting thread is stopped on, in one line: its own open prompt, else the question the thread it is behind
/// has open, named as the answer somebody else's row is holding. Nothing for a thread waiting on nobody.
// thread-state.ts:49
// export function waitingLine(thread: Pick<ThreadView, "asking" | "waitingOn">): string | undefined {
//   if (thread.asking !== undefined) return thread.asking;
//   return thread.waitingOn === undefined ? undefined : `${askingLine(thread.waitingOn.prompt)} needs an answer`;
// }
pub fn waiting_line(thread: &ThreadView) -> Option<String> {
    todo!()
}

// thread-state.ts:54
// private in the TS: port it as a private helper
// const WORDS: Record<ThreadState, string> = {
//   waiting: "Needs you",
//   running: "Working",
//   completed: "Idle",
//   interrupted: "Idle",
//   failed: "Failed",
//   done: "Done",
// };

// thread-state.ts:63
// export function threadStateWord(state: ThreadState): string {
//   return WORDS[state];
// }
pub fn thread_state_word(state: ThreadState) -> String {
    todo!()
}

/// The word a thread's row shows, the short road every surface takes.
// thread-state.ts:68
// export function threadWordOf(thread: Pick<ThreadView, "status" | "asking" | "waitingOn" | "endedAt" | "readAt" | "wokeAt">): string {
//   return threadStateWord(threadState(thread));
// }
pub fn thread_word_of(thread: &ThreadView) -> String {
    todo!()
}

// thread-state.ts:72
// private in the TS: port it as a private helper
// const SUBAGENT_WORDS: Record<SubagentState, string> = { running: "Working", done: "Done", failed: "Failed", stopped: "Stopped" };

/// The word one of an agent's own subagents shows where a thread's would.
// thread-state.ts:75
// export function subagentStateWord(state: SubagentState): string {
//   return SUBAGENT_WORDS[state];
// }
pub fn subagent_state_word(state: SubagentState) -> String {
    todo!()
}

/// What a terminal watching a turn prints the moment that turn stops on a prompt: the thread's own state word, in
/// the lowercase a line of work reads, and under it the lead the app puts the question by. The word comes off the
/// table above, so the terminal and the sidebar cannot say two things about one stopped thread.
// thread-state.ts:82
// export function needsYouLine(ask: Pick<SessionPermissionEvent, "toolName" | "input" | "detail">): string {
//   return `${threadStateWord("waiting").toLowerCase()}: ${askingLine(ask)}`;
// }
pub fn needs_you_line(ask: &SessionPermissionEvent) -> String {
    todo!()
}

/// A thread as the Settled fold reads it, its times ms epoch on the host's clock.
// thread-state.ts:87
// export interface SettleFacts {
//   working: boolean;
//   asking: boolean;
//   failed: boolean;
//   startedAt: number | null;
//   endedAt: number | null;
//   readAt: number | null;
//   settledAt: number | null;
// }
/// What a settle reads of one thread (thread-state.ts:120-128); None is the TS null.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SettleFacts {
    pub working: bool,
    pub asking: bool,
    pub failed: bool,
    pub started_at: Option<f64>,
    pub ended_at: Option<f64>,
    pub read_at: Option<f64>,
    pub settled_at: Option<f64>,
}

/// Whether the thread belongs in the Settled fold, the one rule the sidebar folds by and the host stops a settled
/// slate's timers by: nothing running and nothing asked, and either the person settled it by hand with nothing
/// happening since, or it was seen after it ended and has been quiet `settleMs` since the later of its last activity
/// and that showing. A null `settleMs` is never, so only a hand settles; a thread nobody has seen since it finished, one
/// that failed, and one `held` (open, or in a pinned tree) never fold by time; one with no times has no quiet to read.
/// Whether a window has shown the thread since its latest turn ended.
// thread-state.ts:103
// export const threadSeen = (t: { endedAt: number | null; readAt: number | null }): boolean => t.endedAt === null || (t.readAt !== null && t.readAt >= t.endedAt);
/// Reads only `ended_at` and `read_at`.
pub fn thread_seen(t: &SettleFacts) -> bool {
    todo!()
}

// thread-state.ts:105
// export function threadSettled(t: SettleFacts, nowMs: number, settleMs: number | null, held = false): boolean {
//   const at = settleMoment(t, settleMs, held);
//   return at !== null && (at.byHand || nowMs >= at.ms);
// }
/// `settle_ms` None is the TS null; `held` defaults to false.
pub fn thread_settled(t: &SettleFacts, now_ms: f64, settle_ms: Option<f64>, held: bool) -> bool {
    todo!()
}

/// When the rule above settles the thread: the hand's stamp, or the end of the quiet it waits out; null while it never will.
// thread-state.ts:111
// export function settleMoment(t: SettleFacts, settleMs: number | null, held = false): { ms: number; byHand: boolean } | null {
//   if (t.asking || t.working) return null;
//   const times = [t.startedAt, t.endedAt].filter((at): at is number => at !== null);
//   const last = times.length === 0 ? null : Math.max(...times);
//   if (t.settledAt !== null && (last === null || t.settledAt >= last)) return { ms: t.settledAt, byHand: true };
//   if (held || settleMs === null || t.failed || last === null || !threadSeen(t)) return null;
//   return { ms: Math.max(last, t.readAt ?? last) + settleMs, byHand: false };
// }
/// When a thread settles, and whether a person settled it by hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettleMoment {
    pub ms: f64,
    pub by_hand: bool,
}

/// None is the TS null.
pub fn settle_moment(t: &SettleFacts, settle_ms: Option<f64>, held: bool) -> Option<SettleMoment> {
    todo!()
}

