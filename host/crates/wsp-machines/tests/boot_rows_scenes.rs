// SPDX-License-Identifier: AGPL-3.0-only
//! Spec boot_rows.md: the scenes of `boot_rows.json` that are not the doors (those are `boot_entries.rs`): listings,
//! thread facts, settled reads, the row push, the marks and Resume at reset, replayed through `support/boot.rs`.
mod common;
#[path = "support/boot.rs"]
mod support;

use support::check_rows_scene;

#[test]
fn listed_rows_every_fact_a_listing_carries_and_none_a_row_does_not() {
    check_rows_scene("listed rows: every fact a listing carries, and none a row does not");
}

#[test]
fn thread_facts_the_listing_s_thread_with_the_latest_row_s_model_and_the_running_tu() {
    check_rows_scene("thread facts: the listing's thread with the latest row's model and the running turn");
}

#[test]
fn settled_now_by_hand_by_quiet_or_by_the_stamp_alone_with_no_row() {
    check_rows_scene("settled now: by hand, by quiet, or by the stamp alone with no row");
}

#[test]
fn mark_every_thread_is_checked_before_any_moves_then_stamps_land_each_workspace_wr() {
    check_rows_scene("mark: every thread is checked before any moves, then stamps land, each workspace written and told once");
}

#[test]
fn end_snooze_only_a_standing_snooze_of_the_thread_or_its_root_ends_as_now() {
    check_rows_scene("end snooze: only a standing snooze of the thread or its root ends, as now");
}

#[test]
fn arm_resume_off_clears_on_needs_a_known_reset_on_the_latest_row() {
    check_rows_scene("arm resume: off clears, on needs a known reset on the latest row");
}

#[test]
fn resume_after_limit_the_stopped_turn_goes_on_at_the_reset() {
    check_rows_scene("resume after limit: the stopped turn goes on at the reset");
}

#[test]
fn resume_after_limit_a_newer_turn_takes_the_arm_off_and_sends_nothing() {
    check_rows_scene("resume after limit: a newer turn takes the arm off and sends nothing");
}

#[test]
fn resume_after_limit_a_thread_waiting_on_the_person_sends_nothing() {
    check_rows_scene("resume after limit: a thread waiting on the person sends nothing");
}

#[test]
fn resume_after_limit_a_plan_still_held_past_the_reset_moves_the_row_s_reset() {
    check_rows_scene("resume after limit: a plan still held past the reset moves the row's reset");
}

#[test]
fn resume_after_limit_a_settled_thread_sends_nothing() {
    check_rows_scene("resume after limit: a settled thread sends nothing");
}

#[test]
fn resume_after_limit_no_arm_does_nothing() {
    check_rows_scene("resume after limit: no arm does nothing");
}

#[test]
fn row_push_one_row_per_moved_thread_after_the_tick_the_prompt_on_its_first_push_on() {
    check_rows_scene("row push: one row per moved thread after the tick, the prompt on its first push only, gone ids last");
}
