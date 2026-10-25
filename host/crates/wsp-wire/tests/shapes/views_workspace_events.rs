// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/workspace-events.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::workspace_events::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_workspace_events.json");

/// 3 cases: what zod's parse of InboxFileEvent wrote, or the first issue it raised.
#[test]
fn inbox_file_event_reads_and_writes_as_zod_does() {
    cases_tagged::<InboxFileEvent>(FIXTURES, "InboxFileEvent", "type", InboxFileEvent::TYPE);
}

/// 6 cases: what zod's parse of PortCloseEvent wrote, or the first issue it raised.
#[test]
fn port_close_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PortCloseEvent>(FIXTURES, "PortCloseEvent", "type", PortCloseEvent::TYPE);
}

/// 4 cases: what zod's parse of PortOpenEvent wrote, or the first issue it raised.
#[test]
fn port_open_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PortOpenEvent>(FIXTURES, "PortOpenEvent", "type", PortOpenEvent::TYPE);
}

/// 4 cases: what zod's parse of WorkspaceCostEvent wrote, or the first issue it raised.
#[test]
fn workspace_cost_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceCostEvent>(FIXTURES, "WorkspaceCostEvent", "type", WorkspaceCostEvent::TYPE);
}

/// 8 cases: what zod's parse of WorkspaceCreateStage wrote, or the first issue it raised.
#[test]
fn workspace_create_stage_reads_and_writes_as_zod_does() {
    cases::<WorkspaceCreateStage>(FIXTURES, "WorkspaceCreateStage");
}

/// 3 cases: what zod's parse of WorkspaceCreatedEvent wrote, or the first issue it raised.
#[test]
fn workspace_created_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceCreatedEvent>(FIXTURES, "WorkspaceCreatedEvent", "type", WorkspaceCreatedEvent::TYPE);
}

/// 5 cases: what zod's parse of WorkspaceCreatingEvent wrote, or the first issue it raised.
#[test]
fn workspace_creating_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceCreatingEvent>(FIXTURES, "WorkspaceCreatingEvent", "type", WorkspaceCreatingEvent::TYPE);
}

/// 3 cases: what zod's parse of WorkspaceDeletedEvent wrote, or the first issue it raised.
#[test]
fn workspace_deleted_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceDeletedEvent>(FIXTURES, "WorkspaceDeletedEvent", "type", WorkspaceDeletedEvent::TYPE);
}

/// 3 cases: what zod's parse of WorkspaceGoneEvent wrote, or the first issue it raised.
#[test]
fn workspace_gone_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceGoneEvent>(FIXTURES, "WorkspaceGoneEvent", "type", WorkspaceGoneEvent::TYPE);
}

/// 7 cases: what zod's parse of WorkspaceLookEvent wrote, or the first issue it raised.
#[test]
fn workspace_look_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceLookEvent>(FIXTURES, "WorkspaceLookEvent", "type", WorkspaceLookEvent::TYPE);
}

/// 5 cases: what zod's parse of WorkspaceNappedEvent wrote, or the first issue it raised.
#[test]
fn workspace_napped_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceNappedEvent>(FIXTURES, "WorkspaceNappedEvent", "type", WorkspaceNappedEvent::TYPE);
}

/// 3 cases: what zod's parse of WorkspaceReleasedEvent wrote, or the first issue it raised.
#[test]
fn workspace_released_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceReleasedEvent>(FIXTURES, "WorkspaceReleasedEvent", "type", WorkspaceReleasedEvent::TYPE);
}

/// 3 cases: what zod's parse of WorkspaceRenamedEvent wrote, or the first issue it raised.
#[test]
fn workspace_renamed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceRenamedEvent>(FIXTURES, "WorkspaceRenamedEvent", "type", WorkspaceRenamedEvent::TYPE);
}

/// 3 cases: what zod's parse of WorkspaceReviewEvent wrote, or the first issue it raised.
#[test]
fn workspace_review_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceReviewEvent>(FIXTURES, "WorkspaceReviewEvent", "type", WorkspaceReviewEvent::TYPE);
}

/// 3 cases: what zod's parse of WorkspaceStatusEvent wrote, or the first issue it raised.
#[test]
fn workspace_status_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceStatusEvent>(FIXTURES, "WorkspaceStatusEvent", "type", WorkspaceStatusEvent::TYPE);
}

/// 3 cases: what zod's parse of WorkspaceUpgradedEvent wrote, or the first issue it raised.
#[test]
fn workspace_upgraded_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceUpgradedEvent>(FIXTURES, "WorkspaceUpgradedEvent", "type", WorkspaceUpgradedEvent::TYPE);
}

/// 3 cases: what zod's parse of WorkspaceViewedEvent wrote, or the first issue it raised.
#[test]
fn workspace_viewed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceViewedEvent>(FIXTURES, "WorkspaceViewedEvent", "type", WorkspaceViewedEvent::TYPE);
}

/// 3 cases: what zod's parse of WorkspaceWokenEvent wrote, or the first issue it raised.
#[test]
fn workspace_woken_event_reads_and_writes_as_zod_does() {
    cases_tagged::<WorkspaceWokenEvent>(FIXTURES, "WorkspaceWokenEvent", "type", WorkspaceWokenEvent::TYPE);
}
