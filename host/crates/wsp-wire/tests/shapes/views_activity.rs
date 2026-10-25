// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/activity.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::views::activity::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_activity.json");

/// 5 cases: what zod's parse of ActivityEnded wrote, or the first issue it raised.
#[test]
fn activity_ended_reads_and_writes_as_zod_does() {
    cases::<ActivityEnded>(FIXTURES, "ActivityEnded");
}

/// 8 cases: what zod's parse of ActivityProc wrote, or the first issue it raised.
#[test]
fn activity_proc_reads_and_writes_as_zod_does() {
    cases::<ActivityProc>(FIXTURES, "ActivityProc");
}

/// 3 cases: what zod's parse of ActivityThread wrote, or the first issue it raised.
#[test]
fn activity_thread_reads_and_writes_as_zod_does() {
    cases::<ActivityThread>(FIXTURES, "ActivityThread");
}

/// 3 cases: what zod's parse of ActivityView wrote, or the first issue it raised.
#[test]
fn activity_view_reads_and_writes_as_zod_does() {
    cases::<ActivityView>(FIXTURES, "ActivityView");
}

/// 4 cases: what zod's parse of LeftThread wrote, or the first issue it raised.
#[test]
fn left_thread_reads_and_writes_as_zod_does() {
    cases::<LeftThread>(FIXTURES, "LeftThread");
}

/// 4 cases: what zod's parse of ProcessesAsk wrote, or the first issue it raised.
#[test]
fn processes_ask_reads_and_writes_as_zod_does() {
    cases::<ProcessesAsk>(FIXTURES, "ProcessesAsk");
}
