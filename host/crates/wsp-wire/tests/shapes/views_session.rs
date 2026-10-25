// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/session.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::views::session::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_session.json");

/// 6 cases: what zod's parse of ForkSource wrote, or the first issue it raised.
#[test]
fn fork_source_reads_and_writes_as_zod_does() {
    cases::<ForkSource>(FIXTURES, "ForkSource");
}

/// 4 cases: what zod's parse of ForkedFrom wrote, or the first issue it raised.
#[test]
fn forked_from_reads_and_writes_as_zod_does() {
    cases::<ForkedFrom>(FIXTURES, "ForkedFrom");
}

/// 5 cases: what zod's parse of PermissionOutcome wrote, or the first issue it raised.
#[test]
fn permission_outcome_reads_and_writes_as_zod_does() {
    cases::<PermissionOutcome>(FIXTURES, "PermissionOutcome");
}

/// 4 cases: what zod's parse of SessionOrigin wrote, or the first issue it raised.
#[test]
fn session_origin_reads_and_writes_as_zod_does() {
    cases::<SessionOrigin>(FIXTURES, "SessionOrigin");
}

/// 5 cases: what zod's parse of SessionStatus wrote, or the first issue it raised.
#[test]
fn session_status_reads_and_writes_as_zod_does() {
    cases::<SessionStatus>(FIXTURES, "SessionStatus");
}

/// 11 cases: what zod's parse of SessionView wrote, or the first issue it raised.
#[test]
fn session_view_reads_and_writes_as_zod_does() {
    cases::<SessionView>(FIXTURES, "SessionView");
}

/// 5 cases: what zod's parse of SubagentState wrote, or the first issue it raised.
#[test]
fn subagent_state_reads_and_writes_as_zod_does() {
    cases::<SubagentState>(FIXTURES, "SubagentState");
}

/// 6 cases: what zod's parse of SubagentUsage wrote, or the first issue it raised.
#[test]
fn subagent_usage_reads_and_writes_as_zod_does() {
    cases::<SubagentUsage>(FIXTURES, "SubagentUsage");
}

/// 7 cases: what zod's parse of SubagentView wrote, or the first issue it raised.
#[test]
fn subagent_view_reads_and_writes_as_zod_does() {
    cases::<SubagentView>(FIXTURES, "SubagentView");
}

/// 7 cases: what zod's parse of ThreadCapWait wrote, or the first issue it raised.
#[test]
fn thread_cap_wait_reads_and_writes_as_zod_does() {
    cases::<ThreadCapWait>(FIXTURES, "ThreadCapWait");
}

/// 6 cases: what zod's parse of ThreadMarks wrote, or the first issue it raised.
#[test]
fn thread_marks_reads_and_writes_as_zod_does() {
    cases::<ThreadMarks>(FIXTURES, "ThreadMarks");
}

/// 4 cases: what zod's parse of ThreadPlacement wrote, or the first issue it raised.
#[test]
fn thread_placement_reads_and_writes_as_zod_does() {
    cases::<ThreadPlacement>(FIXTURES, "ThreadPlacement");
}

/// 4 cases: what zod's parse of ThreadPrompt wrote, or the first issue it raised.
#[test]
fn thread_prompt_reads_and_writes_as_zod_does() {
    cases::<ThreadPrompt>(FIXTURES, "ThreadPrompt");
}

/// 5 cases: what zod's parse of ThreadSection wrote, or the first issue it raised.
#[test]
fn thread_section_reads_and_writes_as_zod_does() {
    cases::<ThreadSection>(FIXTURES, "ThreadSection");
}

/// 12 cases: what zod's parse of ThreadView wrote, or the first issue it raised.
#[test]
fn thread_view_reads_and_writes_as_zod_does() {
    cases::<ThreadView>(FIXTURES, "ThreadView");
}

/// 3 cases: what zod's parse of ThreadWaitingOn wrote, or the first issue it raised.
#[test]
fn thread_waiting_on_reads_and_writes_as_zod_does() {
    cases::<ThreadWaitingOn>(FIXTURES, "ThreadWaitingOn");
}

/// 4 cases: what zod's parse of TitleSource wrote, or the first issue it raised.
#[test]
fn title_source_reads_and_writes_as_zod_does() {
    cases::<TitleSource>(FIXTURES, "TitleSource");
}

/// 2 cases: what zod's parse of TurnRefusal wrote, or the first issue it raised.
#[test]
fn turn_refusal_reads_and_writes_as_zod_does() {
    cases::<TurnRefusal>(FIXTURES, "TurnRefusal");
}
