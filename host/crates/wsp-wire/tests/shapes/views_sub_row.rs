// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/sub-row.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::views::sub_row::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_sub_row.json");

/// 6 cases: what zod's parse of SubHeld wrote, or the first issue it raised.
#[test]
fn sub_held_reads_and_writes_as_zod_does() {
    cases::<SubHeld>(FIXTURES, "SubHeld");
}

/// 11 cases: what zod's parse of SubResult wrote, or the first issue it raised.
#[test]
fn sub_result_reads_and_writes_as_zod_does() {
    cases::<SubResult>(FIXTURES, "SubResult");
}

/// 5 cases: what zod's parse of SubRow wrote, or the first issue it raised.
#[test]
fn sub_row_reads_and_writes_as_zod_does() {
    cases::<SubRow>(FIXTURES, "SubRow");
}

/// 6 cases: what zod's parse of SubSpawn wrote, or the first issue it raised.
#[test]
fn sub_spawn_reads_and_writes_as_zod_does() {
    cases::<SubSpawn>(FIXTURES, "SubSpawn");
}

/// 6 cases: what zod's parse of SubState wrote, or the first issue it raised.
#[test]
fn sub_state_reads_and_writes_as_zod_does() {
    cases::<SubState>(FIXTURES, "SubState");
}
