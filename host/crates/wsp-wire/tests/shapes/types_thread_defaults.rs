// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of thread-defaults.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::types::thread_defaults::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_thread_defaults.json");

/// 5 cases: what zod's parse of AccessChoice wrote, or the first issue it raised.
#[test]
fn access_choice_reads_and_writes_as_zod_does() {
    cases::<AccessChoice>(FIXTURES, "AccessChoice");
}

/// 5 cases: what zod's parse of AgentDefaults wrote, or the first issue it raised.
#[test]
fn agent_defaults_reads_and_writes_as_zod_does() {
    cases::<AgentDefaults>(FIXTURES, "AgentDefaults");
}

/// 5 cases: what zod's parse of AgentDefaultsPatch wrote, or the first issue it raised.
#[test]
fn agent_defaults_patch_reads_and_writes_as_zod_does() {
    cases::<AgentDefaultsPatch>(FIXTURES, "AgentDefaultsPatch");
}

/// 7 cases: what zod's parse of AgentSetupSet wrote, or the first issue it raised.
#[test]
fn agent_setup_set_reads_and_writes_as_zod_does() {
    cases::<AgentSetupSet>(FIXTURES, "AgentSetupSet");
}

/// 6 cases: what zod's parse of AgentSetupView wrote, or the first issue it raised.
#[test]
fn agent_setup_view_reads_and_writes_as_zod_does() {
    cases::<AgentSetupView>(FIXTURES, "AgentSetupView");
}

/// 5 cases: what zod's parse of DefaultFrom wrote, or the first issue it raised.
#[test]
fn default_from_reads_and_writes_as_zod_does() {
    cases::<DefaultFrom>(FIXTURES, "DefaultFrom");
}

/// 2 cases: what zod's parse of EnvName wrote, or the first issue it raised.
#[test]
fn env_name_reads_and_writes_as_zod_does() {
    cases::<EnvName>(FIXTURES, "EnvName");
}

/// 4 cases: what zod's parse of ModelPicker wrote, or the first issue it raised.
#[test]
fn model_picker_reads_and_writes_as_zod_does() {
    cases::<ModelPicker>(FIXTURES, "ModelPicker");
}

/// 6 cases: what zod's parse of ProjectOverrides wrote, or the first issue it raised.
#[test]
fn project_overrides_reads_and_writes_as_zod_does() {
    cases::<ProjectOverrides>(FIXTURES, "ProjectOverrides");
}

/// 6 cases: what zod's parse of ProjectOverridesPatch wrote, or the first issue it raised.
#[test]
fn project_overrides_patch_reads_and_writes_as_zod_does() {
    cases::<ProjectOverridesPatch>(FIXTURES, "ProjectOverridesPatch");
}

/// 5 cases: what zod's parse of ResolvedAccess wrote, or the first issue it raised.
#[test]
fn resolved_access_reads_and_writes_as_zod_does() {
    cases::<ResolvedAccess>(FIXTURES, "ResolvedAccess");
}

/// 4 cases: what zod's parse of ResolvedPick wrote, or the first issue it raised.
#[test]
fn resolved_pick_reads_and_writes_as_zod_does() {
    cases::<ResolvedPick>(FIXTURES, "ResolvedPick");
}

/// 4 cases: what zod's parse of ThreadDefaults wrote, or the first issue it raised.
#[test]
fn thread_defaults_reads_and_writes_as_zod_does() {
    cases::<ThreadDefaults>(FIXTURES, "ThreadDefaults");
}

/// 1 case: what zod's parse of TrackerRepo wrote, or the first issue it raised.
#[test]
fn tracker_repo_reads_and_writes_as_zod_does() {
    cases::<TrackerRepo>(FIXTURES, "TrackerRepo");
}
