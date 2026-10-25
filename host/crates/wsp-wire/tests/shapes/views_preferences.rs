// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/preferences.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::preferences::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_preferences.json");

/// 12 cases: what zod's parse of ComputerIcon wrote, or the first issue it raised.
#[test]
fn computer_icon_reads_and_writes_as_zod_does() {
    cases::<ComputerIcon>(FIXTURES, "ComputerIcon");
}

/// 5 cases: what zod's parse of ComputerLook wrote, or the first issue it raised.
#[test]
fn computer_look_reads_and_writes_as_zod_does() {
    cases::<ComputerLook>(FIXTURES, "ComputerLook");
}

/// 5 cases: what zod's parse of EditorChoice wrote, or the first issue it raised.
#[test]
fn editor_choice_reads_and_writes_as_zod_does() {
    cases::<EditorChoice>(FIXTURES, "EditorChoice");
}

/// 15 cases: what zod's parse of EditorId wrote, or the first issue it raised.
#[test]
fn editor_id_reads_and_writes_as_zod_does() {
    cases::<EditorId>(FIXTURES, "EditorId");
}

/// 8 cases: what zod's parse of HistoryPage wrote, or the first issue it raised.
#[test]
fn history_page_reads_and_writes_as_zod_does() {
    cases::<HistoryPage>(FIXTURES, "HistoryPage");
}

/// 33 cases: what zod's parse of Preferences wrote, or the first issue it raised.
#[test]
fn preferences_reads_and_writes_as_zod_does() {
    cases::<Preferences>(FIXTURES, "Preferences");
}

/// 3 cases: what zod's parse of PreferencesChangedEvent wrote, or the first issue it raised.
#[test]
fn preferences_changed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<PreferencesChangedEvent>(FIXTURES, "PreferencesChangedEvent", "type", PreferencesChangedEvent::TYPE);
}

/// 32 cases: what zod's parse of PreferencesPatch wrote, or the first issue it raised.
#[test]
fn preferences_patch_reads_and_writes_as_zod_does() {
    cases::<PreferencesPatch>(FIXTURES, "PreferencesPatch");
}

/// 4 cases: what zod's parse of PreferencesTarget wrote, or the first issue it raised.
#[test]
fn preferences_target_reads_and_writes_as_zod_does() {
    cases::<PreferencesTarget>(FIXTURES, "PreferencesTarget");
}

/// 6 cases: what zod's parse of ProjectLook wrote, or the first issue it raised.
#[test]
fn project_look_reads_and_writes_as_zod_does() {
    cases::<ProjectLook>(FIXTURES, "ProjectLook");
}

/// 5 cases: what zod's parse of RecipeLook wrote, or the first issue it raised.
#[test]
fn recipe_look_reads_and_writes_as_zod_does() {
    cases::<RecipeLook>(FIXTURES, "RecipeLook");
}

/// 3 cases: what zod's parse of SidebarMode wrote, or the first issue it raised.
#[test]
fn sidebar_mode_reads_and_writes_as_zod_does() {
    cases::<SidebarMode>(FIXTURES, "SidebarMode");
}

/// 3 cases: what zod's parse of TerminalSizeSource wrote, or the first issue it raised.
#[test]
fn terminal_size_source_reads_and_writes_as_zod_does() {
    cases::<TerminalSizeSource>(FIXTURES, "TerminalSizeSource");
}

/// 2 cases: what zod's parse of ThemeId wrote, or the first issue it raised.
#[test]
fn theme_id_reads_and_writes_as_zod_does() {
    cases::<ThemeId>(FIXTURES, "ThemeId");
}

/// 4 cases: what zod's parse of ThemePreference wrote, or the first issue it raised.
#[test]
fn theme_preference_reads_and_writes_as_zod_does() {
    cases::<ThemePreference>(FIXTURES, "ThemePreference");
}

/// 12 cases: what zod's parse of ThreadFacts wrote, or the first issue it raised.
#[test]
fn thread_facts_reads_and_writes_as_zod_does() {
    cases::<ThreadFacts>(FIXTURES, "ThreadFacts");
}

/// 8 cases: what zod's parse of ThreadHead wrote, or the first issue it raised.
#[test]
fn thread_head_reads_and_writes_as_zod_does() {
    cases::<ThreadHead>(FIXTURES, "ThreadHead");
}

/// 5 cases: what zod's parse of ThreadHeadEvent wrote, or the first issue it raised.
#[test]
fn thread_head_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ThreadHeadEvent>(FIXTURES, "ThreadHeadEvent", "type", ThreadHeadEvent::TYPE);
}

/// 3 cases: what zod's parse of ThreadMarkedEvent wrote, or the first issue it raised.
#[test]
fn thread_marked_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ThreadMarkedEvent>(FIXTURES, "ThreadMarkedEvent", "type", ThreadMarkedEvent::TYPE);
}

/// 3 cases: what zod's parse of ThreadRewoundEvent wrote, or the first issue it raised.
#[test]
fn thread_rewound_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ThreadRewoundEvent>(FIXTURES, "ThreadRewoundEvent", "type", ThreadRewoundEvent::TYPE);
}

/// 32 cases: what zod's parse of PreferencesStored wrote, or the first issue it raised.
#[test]
fn preferences_stored_reads_and_writes_as_zod_does() {
    cases::<PreferencesStored>(FIXTURES, "PreferencesStored");
}
