// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of sub-thread.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::sub_thread::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_sub_thread.json");

/// 5 cases: what zod's parse of SubOut wrote, or the first issue it raised.
#[test]
fn sub_out_reads_and_writes_as_zod_does() {
    cases::<SubOut>(FIXTURES, "SubOut");
}

/// 3 cases: what zod's parse of SubStarted wrote, or the first issue it raised.
#[test]
fn sub_started_reads_and_writes_as_zod_does() {
    cases::<SubStarted>(FIXTURES, "SubStarted");
}
