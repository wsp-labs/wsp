// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of outside-line.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::outside_line::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_outside_line.json");

/// 6 cases: what zod's parse of OutsideLine wrote, or the first issue it raised.
#[test]
fn outside_line_reads_and_writes_as_zod_does() {
    cases::<OutsideLine>(FIXTURES, "OutsideLine");
}
