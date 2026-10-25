// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/place.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::place::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_place.json");

/// 7 cases: what zod's parse of AddLine wrote, or the first issue it raised.
#[test]
fn add_line_reads_and_writes_as_zod_does() {
    cases::<AddLine>(FIXTURES, "AddLine");
}

/// 4 cases: what zod's parse of AsideTextEvent wrote, or the first issue it raised.
#[test]
fn aside_text_event_reads_and_writes_as_zod_does() {
    cases_tagged::<AsideTextEvent>(FIXTURES, "AsideTextEvent", "type", AsideTextEvent::TYPE);
}

/// 6 cases: what zod's parse of CloudCap wrote, or the first issue it raised.
#[test]
fn cloud_cap_reads_and_writes_as_zod_does() {
    cases::<CloudCap>(FIXTURES, "CloudCap");
}

/// 4 cases: what zod's parse of CloudLimits wrote, or the first issue it raised.
#[test]
fn cloud_limits_reads_and_writes_as_zod_does() {
    cases::<CloudLimits>(FIXTURES, "CloudLimits");
}

/// 7 cases: what zod's parse of CloudPlan wrote, or the first issue it raised.
#[test]
fn cloud_plan_reads_and_writes_as_zod_does() {
    cases::<CloudPlan>(FIXTURES, "CloudPlan");
}

/// 2 cases: what zod's parse of CloudStarts wrote, or the first issue it raised.
#[test]
fn cloud_starts_reads_and_writes_as_zod_does() {
    cases::<CloudStarts>(FIXTURES, "CloudStarts");
}

/// 4 cases: what zod's parse of ComputerCap wrote, or the first issue it raised.
#[test]
fn computer_cap_reads_and_writes_as_zod_does() {
    cases::<ComputerCap>(FIXTURES, "ComputerCap");
}

/// 4 cases: what zod's parse of CopyWord wrote, or the first issue it raised.
#[test]
fn copy_word_reads_and_writes_as_zod_does() {
    cases::<CopyWord>(FIXTURES, "CopyWord");
}

/// 4 cases: what zod's parse of DoctorLineEvent wrote, or the first issue it raised.
#[test]
fn doctor_line_event_reads_and_writes_as_zod_does() {
    cases_tagged::<DoctorLineEvent>(FIXTURES, "DoctorLineEvent", "type", DoctorLineEvent::TYPE);
}

/// 6 cases: what zod's parse of ForwardCloseEvent wrote, or the first issue it raised.
#[test]
fn forward_close_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ForwardCloseEvent>(FIXTURES, "ForwardCloseEvent", "type", ForwardCloseEvent::TYPE);
}

/// 3 cases: what zod's parse of ForwardOpenEvent wrote, or the first issue it raised.
#[test]
fn forward_open_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ForwardOpenEvent>(FIXTURES, "ForwardOpenEvent", "type", ForwardOpenEvent::TYPE);
}

/// 3 cases: what zod's parse of HostNoticeEvent wrote, or the first issue it raised.
#[test]
fn host_notice_event_reads_and_writes_as_zod_does() {
    cases_tagged::<HostNoticeEvent>(FIXTURES, "HostNoticeEvent", "type", HostNoticeEvent::TYPE);
}

/// 6 cases: what zod's parse of MacKind wrote, or the first issue it raised.
#[test]
fn mac_kind_reads_and_writes_as_zod_does() {
    cases::<MacKind>(FIXTURES, "MacKind");
}

/// 7 cases: what zod's parse of MachineBind wrote, or the first issue it raised.
#[test]
fn machine_bind_reads_and_writes_as_zod_does() {
    cases::<MachineBind>(FIXTURES, "MachineBind");
}

/// 4 cases: what zod's parse of MachineShare wrote, or the first issue it raised.
#[test]
fn machine_share_reads_and_writes_as_zod_does() {
    cases::<MachineShare>(FIXTURES, "MachineShare");
}

/// 6 cases: what zod's parse of PendingComputer wrote, or the first issue it raised.
#[test]
fn pending_computer_reads_and_writes_as_zod_does() {
    cases::<PendingComputer>(FIXTURES, "PendingComputer");
}

/// 6 cases: what zod's parse of PendingStep wrote, or the first issue it raised.
#[test]
fn pending_step_reads_and_writes_as_zod_does() {
    cases::<PendingStep>(FIXTURES, "PendingStep");
}

/// 4 cases: what zod's parse of PlaceAbsentEvent wrote, or the first issue it raised.
#[test]
fn place_absent_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PlaceAbsentEvent>(FIXTURES, "PlaceAbsentEvent", "type", PlaceAbsentEvent::TYPE);
}

/// 6 cases: what zod's parse of PlaceAddJob wrote, or the first issue it raised.
#[test]
fn place_add_job_reads_and_writes_as_zod_does() {
    cases::<PlaceAddJob>(FIXTURES, "PlaceAddJob");
}

/// 12 cases: what zod's parse of PlaceAddStep wrote, or the first issue it raised.
#[test]
fn place_add_step_reads_and_writes_as_zod_does() {
    cases::<PlaceAddStep>(FIXTURES, "PlaceAddStep");
}

/// 4 cases: what zod's parse of PlaceApplied wrote, or the first issue it raised.
#[test]
fn place_applied_reads_and_writes_as_zod_does() {
    cases::<PlaceApplied>(FIXTURES, "PlaceApplied");
}

/// 5 cases: what zod's parse of PlaceBack wrote, or the first issue it raised.
#[test]
fn place_back_reads_and_writes_as_zod_does() {
    cases::<PlaceBack>(FIXTURES, "PlaceBack");
}

/// 2 cases: what zod's parse of PlaceCap wrote, or the first issue it raised.
#[test]
fn place_cap_reads_and_writes_as_zod_does() {
    cases::<PlaceCap>(FIXTURES, "PlaceCap");
}

/// 8 cases: what zod's parse of PlaceCapSet wrote, or the first issue it raised.
#[test]
fn place_cap_set_reads_and_writes_as_zod_does() {
    cases::<PlaceCapSet>(FIXTURES, "PlaceCapSet");
}

/// 4 cases: what zod's parse of PlaceChangedEvent wrote, or the first issue it raised.
#[test]
fn place_changed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PlaceChangedEvent>(FIXTURES, "PlaceChangedEvent", "type", PlaceChangedEvent::TYPE);
}

/// 4 cases: what zod's parse of PlaceDial wrote, or the first issue it raised.
#[test]
fn place_dial_reads_and_writes_as_zod_does() {
    cases::<PlaceDial>(FIXTURES, "PlaceDial");
}

/// 8 cases: what zod's parse of PlaceDialled wrote, or the first issue it raised.
#[test]
fn place_dialled_reads_and_writes_as_zod_does() {
    cases::<PlaceDialled>(FIXTURES, "PlaceDialled");
}

/// 4 cases: what zod's parse of PlaceHolds wrote, or the first issue it raised.
#[test]
fn place_holds_reads_and_writes_as_zod_does() {
    cases::<PlaceHolds>(FIXTURES, "PlaceHolds");
}

/// 4 cases: what zod's parse of PlaceJoinedEvent wrote, or the first issue it raised.
#[test]
fn place_joined_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PlaceJoinedEvent>(FIXTURES, "PlaceJoinedEvent", "type", PlaceJoinedEvent::TYPE);
}

/// 4 cases: what zod's parse of PlacePendingEvent wrote, or the first issue it raised.
#[test]
fn place_pending_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PlacePendingEvent>(FIXTURES, "PlacePendingEvent", "type", PlacePendingEvent::TYPE);
}

/// 3 cases: what zod's parse of PlacePresentEvent wrote, or the first issue it raised.
#[test]
fn place_present_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PlacePresentEvent>(FIXTURES, "PlacePresentEvent", "type", PlacePresentEvent::TYPE);
}

/// 4 cases: what zod's parse of PlaceProvisionKind wrote, or the first issue it raised.
#[test]
fn place_provision_kind_reads_and_writes_as_zod_does() {
    cases::<PlaceProvisionKind>(FIXTURES, "PlaceProvisionKind");
}

/// 11 cases: what zod's parse of PlaceProvisionRow wrote, or the first issue it raised.
#[test]
fn place_provision_row_reads_and_writes_as_zod_does() {
    cases::<PlaceProvisionRow>(FIXTURES, "PlaceProvisionRow");
}

/// 7 cases: what zod's parse of PlaceRemoved wrote, or the first issue it raised.
#[test]
fn place_removed_reads_and_writes_as_zod_does() {
    cases::<PlaceRemoved>(FIXTURES, "PlaceRemoved");
}

/// 3 cases: what zod's parse of PlaceRemovedEvent wrote, or the first issue it raised.
#[test]
fn place_removed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PlaceRemovedEvent>(FIXTURES, "PlaceRemovedEvent", "type", PlaceRemovedEvent::TYPE);
}

/// 5 cases: what zod's parse of PlaceRoad wrote, or the first issue it raised.
#[test]
fn place_road_reads_and_writes_as_zod_does() {
    cases::<PlaceRoad>(FIXTURES, "PlaceRoad");
}

/// 10 cases: what zod's parse of PlaceSettingWord wrote, or the first issue it raised.
#[test]
fn place_setting_word_reads_and_writes_as_zod_does() {
    cases::<PlaceSettingWord>(FIXTURES, "PlaceSettingWord");
}

/// 17 cases: what zod's parse of PlaceSettings wrote, or the first issue it raised.
#[test]
fn place_settings_reads_and_writes_as_zod_does() {
    cases::<PlaceSettings>(FIXTURES, "PlaceSettings");
}

/// 17 cases: what zod's parse of PlaceSettingsAsk wrote, or the first issue it raised.
#[test]
fn place_settings_ask_reads_and_writes_as_zod_does() {
    cases::<PlaceSettingsAsk>(FIXTURES, "PlaceSettingsAsk");
}

/// 5 cases: what zod's parse of PlaceSetup wrote, or the first issue it raised.
#[test]
fn place_setup_reads_and_writes_as_zod_does() {
    cases::<PlaceSetup>(FIXTURES, "PlaceSetup");
}

/// 5 cases: what zod's parse of PlaceSetupEvent wrote, or the first issue it raised.
#[test]
fn place_setup_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PlaceSetupEvent>(FIXTURES, "PlaceSetupEvent", "type", PlaceSetupEvent::TYPE);
}

/// 8 cases: what zod's parse of PlaceSetupLine wrote, or the first issue it raised.
#[test]
fn place_setup_line_reads_and_writes_as_zod_does() {
    cases::<PlaceSetupLine>(FIXTURES, "PlaceSetupLine");
}

/// 13 cases: what zod's parse of PlaceSetupStep wrote, or the first issue it raised.
#[test]
fn place_setup_step_reads_and_writes_as_zod_does() {
    cases::<PlaceSetupStep>(FIXTURES, "PlaceSetupStep");
}

/// 3 cases: what zod's parse of PlaceSpend wrote, or the first issue it raised.
#[test]
fn place_spend_reads_and_writes_as_zod_does() {
    cases::<PlaceSpend>(FIXTURES, "PlaceSpend");
}

/// 8 cases: what zod's parse of PlaceStageEvent wrote, or the first issue it raised.
#[test]
fn place_stage_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PlaceStageEvent>(FIXTURES, "PlaceStageEvent", "type", PlaceStageEvent::TYPE);
}

/// 4 cases: what zod's parse of PlaceSync wrote, or the first issue it raised.
#[test]
fn place_sync_reads_and_writes_as_zod_does() {
    cases::<PlaceSync>(FIXTURES, "PlaceSync");
}

/// 4 cases: what zod's parse of PlaceSyncEvent wrote, or the first issue it raised.
#[test]
fn place_sync_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PlaceSyncEvent>(FIXTURES, "PlaceSyncEvent", "type", PlaceSyncEvent::TYPE);
}

/// 5 cases: what zod's parse of PlaceUpdateReply wrote, or the first issue it raised.
#[test]
fn place_update_reply_reads_and_writes_as_zod_does() {
    cases::<PlaceUpdateReply>(FIXTURES, "PlaceUpdateReply");
}

/// 25 cases: what zod's parse of PlaceView wrote, or the first issue it raised.
#[test]
fn place_view_reads_and_writes_as_zod_does() {
    cases::<PlaceView>(FIXTURES, "PlaceView");
}

/// 5 cases: what zod's parse of PlaceWait wrote, or the first issue it raised.
#[test]
fn place_wait_reads_and_writes_as_zod_does() {
    cases::<PlaceWait>(FIXTURES, "PlaceWait");
}

/// 4 cases: what zod's parse of ProjectAddEvent wrote, or the first issue it raised.
#[test]
fn project_add_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ProjectAddEvent>(FIXTURES, "ProjectAddEvent", "type", ProjectAddEvent::TYPE);
}

/// 3 cases: what zod's parse of ProjectAddedEvent wrote, or the first issue it raised.
#[test]
fn project_added_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ProjectAddedEvent>(FIXTURES, "ProjectAddedEvent", "type", ProjectAddedEvent::TYPE);
}

/// 3 cases: what zod's parse of ProjectRemovedEvent wrote, or the first issue it raised.
#[test]
fn project_removed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ProjectRemovedEvent>(FIXTURES, "ProjectRemovedEvent", "type", ProjectRemovedEvent::TYPE);
}

/// 3 cases: what zod's parse of RecipesChangedEvent wrote, or the first issue it raised.
#[test]
fn recipes_changed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<RecipesChangedEvent>(FIXTURES, "RecipesChangedEvent", "type", RecipesChangedEvent::TYPE);
}

/// 4 cases: what zod's parse of SetupEnd wrote, or the first issue it raised.
#[test]
fn setup_end_reads_and_writes_as_zod_does() {
    cases::<SetupEnd>(FIXTURES, "SetupEnd");
}

/// 4 cases: what zod's parse of StartsWindow wrote, or the first issue it raised.
#[test]
fn starts_window_reads_and_writes_as_zod_does() {
    cases::<StartsWindow>(FIXTURES, "StartsWindow");
}

/// 2 cases: what zod's parse of SudoPassword wrote, or the first issue it raised.
#[test]
fn sudo_password_reads_and_writes_as_zod_does() {
    cases::<SudoPassword>(FIXTURES, "SudoPassword");
}

/// 4 cases: what zod's parse of WorkspaceCopy wrote, or the first issue it raised.
#[test]
fn workspace_copy_reads_and_writes_as_zod_does() {
    cases::<WorkspaceCopy>(FIXTURES, "WorkspaceCopy");
}
