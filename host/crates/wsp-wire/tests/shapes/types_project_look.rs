// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of project-look.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::types::project_look::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_project_look.json");

/// 10 cases: what zod's parse of ProjectHue wrote, or the first issue it raised.
#[test]
fn project_hue_reads_and_writes_as_zod_does() {
    cases::<ProjectHue>(FIXTURES, "ProjectHue");
}

/// 21 cases: what zod's parse of ProjectIcon wrote, or the first issue it raised.
#[test]
fn project_icon_reads_and_writes_as_zod_does() {
    cases::<ProjectIcon>(FIXTURES, "ProjectIcon");
}

/// 1 case: what zod's parse of ProjectIconHash wrote, or the first issue it raised.
#[test]
fn project_icon_hash_reads_and_writes_as_zod_does() {
    cases::<ProjectIconHash>(FIXTURES, "ProjectIconHash");
}
