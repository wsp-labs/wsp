// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/place-setup-words.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::views::place_setup_words::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_place_setup_words.json");

/// 11 cases: what zod's parse of PlaceEstimate wrote, or the first issue it raised.
#[test]
fn place_estimate_reads_and_writes_as_zod_does() {
    cases::<PlaceEstimate>(FIXTURES, "PlaceEstimate");
}
