// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/place-kind.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::place_kind::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_place_kind.json");

/// 3 cases: what zod's parse of PlaceKind wrote, or the first issue it raised.
#[test]
fn place_kind_reads_and_writes_as_zod_does() {
    cases::<PlaceKind>(FIXTURES, "PlaceKind");
}
