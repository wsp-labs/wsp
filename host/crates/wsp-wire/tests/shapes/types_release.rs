// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of release.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::release::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_release.json");

/// 4 cases: what zod's parse of HostShape wrote, or the first issue it raised.
#[test]
fn host_shape_reads_and_writes_as_zod_does() {
    cases::<HostShape>(FIXTURES, "HostShape");
}

/// 3 cases: what zod's parse of ReleaseChangedEvent wrote, or the first issue it raised.
#[test]
fn release_changed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ReleaseChangedEvent>(FIXTURES, "ReleaseChangedEvent", "type", ReleaseChangedEvent::TYPE);
}

/// 3 cases: what zod's parse of ReleaseLatest wrote, or the first issue it raised.
#[test]
fn release_latest_reads_and_writes_as_zod_does() {
    cases::<ReleaseLatest>(FIXTURES, "ReleaseLatest");
}

/// 5 cases: what zod's parse of ReleaseState wrote, or the first issue it raised.
#[test]
fn release_state_reads_and_writes_as_zod_does() {
    cases::<ReleaseState>(FIXTURES, "ReleaseState");
}

/// 6 cases: what zod's parse of ReleaseView wrote, or the first issue it raised.
#[test]
fn release_view_reads_and_writes_as_zod_does() {
    cases::<ReleaseView>(FIXTURES, "ReleaseView");
}
