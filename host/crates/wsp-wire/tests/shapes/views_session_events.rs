// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/session-events.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::session_events::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_session_events.json");

/// 6 cases: what zod's parse of DeltaKind wrote, or the first issue it raised.
#[test]
fn delta_kind_reads_and_writes_as_zod_does() {
    cases::<DeltaKind>(FIXTURES, "DeltaKind");
}

/// 4 cases: what zod's parse of FilePatch wrote, or the first issue it raised.
#[test]
fn file_patch_reads_and_writes_as_zod_does() {
    cases::<FilePatch>(FIXTURES, "FilePatch");
}

/// 7 cases: what zod's parse of PatchHunk wrote, or the first issue it raised.
#[test]
fn patch_hunk_reads_and_writes_as_zod_does() {
    cases::<PatchHunk>(FIXTURES, "PatchHunk");
}

/// 4 cases: what zod's parse of PlanStep wrote, or the first issue it raised.
#[test]
fn plan_step_reads_and_writes_as_zod_does() {
    cases::<PlanStep>(FIXTURES, "PlanStep");
}

/// 5 cases: what zod's parse of RunState wrote, or the first issue it raised.
#[test]
fn run_state_reads_and_writes_as_zod_does() {
    cases::<RunState>(FIXTURES, "RunState");
}

/// 7 cases: what zod's parse of RunStep wrote, or the first issue it raised.
#[test]
fn run_step_reads_and_writes_as_zod_does() {
    cases::<RunStep>(FIXTURES, "RunStep");
}

/// 6 cases: what zod's parse of SessionBehindEvent wrote, or the first issue it raised.
#[test]
fn session_behind_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionBehindEvent>(FIXTURES, "SessionBehindEvent", "type", SessionBehindEvent::TYPE);
}

/// 9 cases: what zod's parse of SessionCappedEvent wrote, or the first issue it raised.
#[test]
fn session_capped_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionCappedEvent>(FIXTURES, "SessionCappedEvent", "type", SessionCappedEvent::TYPE);
}

/// 6 cases: what zod's parse of SessionChangesEvent wrote, or the first issue it raised.
#[test]
fn session_changes_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionChangesEvent>(FIXTURES, "SessionChangesEvent", "type", SessionChangesEvent::TYPE);
}

/// 6 cases: what zod's parse of SessionCheckpointEvent wrote, or the first issue it raised.
#[test]
fn session_checkpoint_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionCheckpointEvent>(FIXTURES, "SessionCheckpointEvent", "type", SessionCheckpointEvent::TYPE);
}

/// 8 cases: what zod's parse of SessionCompactedEvent wrote, or the first issue it raised.
#[test]
fn session_compacted_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionCompactedEvent>(FIXTURES, "SessionCompactedEvent", "type", SessionCompactedEvent::TYPE);
}

/// 6 cases: what zod's parse of SessionContextEvent wrote, or the first issue it raised.
#[test]
fn session_context_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionContextEvent>(FIXTURES, "SessionContextEvent", "type", SessionContextEvent::TYPE);
}

/// 16 cases: what zod's parse of SessionDeltaEvent wrote, or the first issue it raised.
#[test]
fn session_delta_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionDeltaEvent>(FIXTURES, "SessionDeltaEvent", "type", SessionDeltaEvent::TYPE);
}

/// 6 cases: what zod's parse of SessionDoneEvent wrote, or the first issue it raised.
#[test]
fn session_done_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionDoneEvent>(FIXTURES, "SessionDoneEvent", "type", SessionDoneEvent::TYPE);
}

/// 7 cases: what zod's parse of SessionEarlierEvent wrote, or the first issue it raised.
#[test]
fn session_earlier_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionEarlierEvent>(FIXTURES, "SessionEarlierEvent", "type", SessionEarlierEvent::TYPE);
}

/// 8 cases: what zod's parse of SessionEndEvent wrote, or the first issue it raised.
#[test]
fn session_end_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionEndEvent>(FIXTURES, "SessionEndEvent", "type", SessionEndEvent::TYPE);
}

/// 25 cases: what zod's parse of SessionEvent wrote, or the first issue it raised.
#[test]
fn session_event_reads_and_writes_as_zod_does() {
    cases::<SessionEvent>(FIXTURES, "SessionEvent");
}

/// 3 cases: what zod's parse of SessionHarness wrote, or the first issue it raised.
#[test]
fn session_harness_reads_and_writes_as_zod_does() {
    cases::<SessionHarness>(FIXTURES, "SessionHarness");
}

/// 4 cases: what zod's parse of SessionHeldEvent wrote, or the first issue it raised.
#[test]
fn session_held_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionHeldEvent>(FIXTURES, "SessionHeldEvent", "type", SessionHeldEvent::TYPE);
}

/// 6 cases: what zod's parse of SessionMovedEvent wrote, or the first issue it raised.
#[test]
fn session_moved_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionMovedEvent>(FIXTURES, "SessionMovedEvent", "type", SessionMovedEvent::TYPE);
}

/// 6 cases: what zod's parse of SessionNoticeEvent wrote, or the first issue it raised.
#[test]
fn session_notice_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionNoticeEvent>(FIXTURES, "SessionNoticeEvent", "type", SessionNoticeEvent::TYPE);
}

/// 6 cases: what zod's parse of SessionNotifyEvent wrote, or the first issue it raised.
#[test]
fn session_notify_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionNotifyEvent>(FIXTURES, "SessionNotifyEvent", "type", SessionNotifyEvent::TYPE);
}

/// 7 cases: what zod's parse of SessionPermissionClosedEvent wrote, or the first issue it raised.
#[test]
fn session_permission_closed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionPermissionClosedEvent>(FIXTURES, "SessionPermissionClosedEvent", "type", SessionPermissionClosedEvent::TYPE);
}

/// 6 cases: what zod's parse of SessionPermissionEvent wrote, or the first issue it raised.
#[test]
fn session_permission_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionPermissionEvent>(FIXTURES, "SessionPermissionEvent", "type", SessionPermissionEvent::TYPE);
}

/// 6 cases: what zod's parse of SessionPlanEvent wrote, or the first issue it raised.
#[test]
fn session_plan_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionPlanEvent>(FIXTURES, "SessionPlanEvent", "type", SessionPlanEvent::TYPE);
}

/// 4 cases: what zod's parse of SessionQueuedEvent wrote, or the first issue it raised.
#[test]
fn session_queued_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionQueuedEvent>(FIXTURES, "SessionQueuedEvent", "type", SessionQueuedEvent::TYPE);
}

/// 12 cases: what zod's parse of SessionRetryEvent wrote, or the first issue it raised.
#[test]
fn session_retry_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionRetryEvent>(FIXTURES, "SessionRetryEvent", "type", SessionRetryEvent::TYPE);
}

/// 4 cases: what zod's parse of SessionRowEvent wrote, or the first issue it raised.
#[test]
fn session_row_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionRowEvent>(FIXTURES, "SessionRowEvent", "type", SessionRowEvent::TYPE);
}

/// 9 cases: what zod's parse of SessionRunEvent wrote, or the first issue it raised.
#[test]
fn session_run_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionRunEvent>(FIXTURES, "SessionRunEvent", "type", SessionRunEvent::TYPE);
}

/// 7 cases: what zod's parse of SessionStartEvent wrote, or the first issue it raised.
#[test]
fn session_start_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionStartEvent>(FIXTURES, "SessionStartEvent", "type", SessionStartEvent::TYPE);
}

/// 4 cases: what zod's parse of SessionStartingEvent wrote, or the first issue it raised.
#[test]
fn session_starting_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionStartingEvent>(FIXTURES, "SessionStartingEvent", "type", SessionStartingEvent::TYPE);
}

/// 7 cases: what zod's parse of SessionSteerEvent wrote, or the first issue it raised.
#[test]
fn session_steer_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionSteerEvent>(FIXTURES, "SessionSteerEvent", "type", SessionSteerEvent::TYPE);
}

/// 11 cases: what zod's parse of SessionSubagentEvent wrote, or the first issue it raised.
#[test]
fn session_subagent_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionSubagentEvent>(FIXTURES, "SessionSubagentEvent", "type", SessionSubagentEvent::TYPE);
}

/// 8 cases: what zod's parse of SessionUncappedEvent wrote, or the first issue it raised.
#[test]
fn session_uncapped_event_reads_and_writes_as_zod_does() {
    cases_tagged::<SessionUncappedEvent>(FIXTURES, "SessionUncappedEvent", "type", SessionUncappedEvent::TYPE);
}

/// 8 cases: what zod's parse of ThreadWaitedEvent wrote, or the first issue it raised.
#[test]
fn thread_waited_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ThreadWaitedEvent>(FIXTURES, "ThreadWaitedEvent", "type", ThreadWaitedEvent::TYPE);
}

/// 3 cases: what zod's parse of TurnChangedFile wrote, or the first issue it raised.
#[test]
fn turn_changed_file_reads_and_writes_as_zod_does() {
    cases::<TurnChangedFile>(FIXTURES, "TurnChangedFile");
}

/// 6 cases: what zod's parse of TurnResult wrote, or the first issue it raised.
#[test]
fn turn_result_reads_and_writes_as_zod_does() {
    cases::<TurnResult>(FIXTURES, "TurnResult");
}

/// 4 cases: what zod's parse of TurnStatus wrote, or the first issue it raised.
#[test]
fn turn_status_reads_and_writes_as_zod_does() {
    cases::<TurnStatus>(FIXTURES, "TurnStatus");
}

/// 4 cases: what zod's parse of TurnTokens wrote, or the first issue it raised.
#[test]
fn turn_tokens_reads_and_writes_as_zod_does() {
    cases::<TurnTokens>(FIXTURES, "TurnTokens");
}
