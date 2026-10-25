// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/harness.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::harness::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_harness.json");

/// 20 cases: what zod's parse of HarnessCatalog wrote, or the first issue it raised.
#[test]
fn harness_catalog_reads_and_writes_as_zod_does() {
    cases::<HarnessCatalog>(FIXTURES, "HarnessCatalog");
}

/// 3 cases: what zod's parse of HarnessCatalogSource wrote, or the first issue it raised.
#[test]
fn harness_catalog_source_reads_and_writes_as_zod_does() {
    cases::<HarnessCatalogSource>(FIXTURES, "HarnessCatalogSource");
}

/// 6 cases: what zod's parse of HarnessModel wrote, or the first issue it raised.
#[test]
fn harness_model_reads_and_writes_as_zod_does() {
    cases::<HarnessModel>(FIXTURES, "HarnessModel");
}

/// 5 cases: what zod's parse of HarnessOption wrote, or the first issue it raised.
#[test]
fn harness_option_reads_and_writes_as_zod_does() {
    cases::<HarnessOption>(FIXTURES, "HarnessOption");
}

/// 9 cases: what zod's parse of ReviewPostResult wrote, or the first issue it raised.
#[test]
fn review_post_result_reads_and_writes_as_zod_does() {
    cases::<ReviewPostResult>(FIXTURES, "ReviewPostResult");
}

/// 4 cases: what zod's parse of ScreenCommand wrote, or the first issue it raised.
#[test]
fn screen_command_reads_and_writes_as_zod_does() {
    cases::<ScreenCommand>(FIXTURES, "ScreenCommand");
}

/// 6 cases: what zod's parse of ScreenControl wrote, or the first issue it raised.
#[test]
fn screen_control_reads_and_writes_as_zod_does() {
    cases::<ScreenControl>(FIXTURES, "ScreenControl");
}

/// 3 cases: what zod's parse of StartResult wrote, or the first issue it raised.
#[test]
fn start_result_reads_and_writes_as_zod_does() {
    cases::<StartResult>(FIXTURES, "StartResult");
}
