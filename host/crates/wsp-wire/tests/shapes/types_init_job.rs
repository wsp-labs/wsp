// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of init-job.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::init_job::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_init_job.json");

/// 5 cases: what zod's parse of InitAgent wrote, or the first issue it raised.
#[test]
fn init_agent_reads_and_writes_as_zod_does() {
    cases::<InitAgent>(FIXTURES, "InitAgent");
}

/// 3 cases: what zod's parse of InitChoice wrote, or the first issue it raised.
#[test]
fn init_choice_reads_and_writes_as_zod_does() {
    cases::<InitChoice>(FIXTURES, "InitChoice");
}

/// 3 cases: what zod's parse of InitDraft wrote, or the first issue it raised.
#[test]
fn init_draft_reads_and_writes_as_zod_does() {
    cases::<InitDraft>(FIXTURES, "InitDraft");
}

/// 5 cases: what zod's parse of InitFooterLine wrote, or the first issue it raised.
#[test]
fn init_footer_line_reads_and_writes_as_zod_does() {
    cases::<InitFooterLine>(FIXTURES, "InitFooterLine");
}

/// 10 cases: what zod's parse of InitJob wrote, or the first issue it raised.
#[test]
fn init_job_reads_and_writes_as_zod_does() {
    cases::<InitJob>(FIXTURES, "InitJob");
}

/// 3 cases: what zod's parse of InitJobEvent wrote, or the first issue it raised.
#[test]
fn init_job_event_reads_and_writes_as_zod_does() {
    cases_tagged::<InitJobEvent>(FIXTURES, "InitJobEvent", "type", InitJobEvent::TYPE);
}

/// 2 cases: what zod's parse of InitKeys wrote, or the first issue it raised.
#[test]
fn init_keys_reads_and_writes_as_zod_does() {
    cases::<InitKeys>(FIXTURES, "InitKeys");
}

/// 5 cases: what zod's parse of InitNeedsYou wrote, or the first issue it raised.
#[test]
fn init_needs_you_reads_and_writes_as_zod_does() {
    cases::<InitNeedsYou>(FIXTURES, "InitNeedsYou");
}

/// 3 cases: what zod's parse of InitNeedsYouEvent wrote, or the first issue it raised.
#[test]
fn init_needs_you_event_reads_and_writes_as_zod_does() {
    cases_tagged::<InitNeedsYouEvent>(FIXTURES, "InitNeedsYouEvent", "type", InitNeedsYouEvent::TYPE);
}

/// 11 cases: what zod's parse of InitPhase wrote, or the first issue it raised.
#[test]
fn init_phase_reads_and_writes_as_zod_does() {
    cases::<InitPhase>(FIXTURES, "InitPhase");
}

/// 4 cases: what zod's parse of InitRoad wrote, or the first issue it raised.
#[test]
fn init_road_reads_and_writes_as_zod_does() {
    cases::<InitRoad>(FIXTURES, "InitRoad");
}

/// 7 cases: what zod's parse of InitRow wrote, or the first issue it raised.
#[test]
fn init_row_reads_and_writes_as_zod_does() {
    cases::<InitRow>(FIXTURES, "InitRow");
}

/// 9 cases: what zod's parse of InitRowKind wrote, or the first issue it raised.
#[test]
fn init_row_kind_reads_and_writes_as_zod_does() {
    cases::<InitRowKind>(FIXTURES, "InitRowKind");
}

/// 4 cases: what zod's parse of InitSaidLine wrote, or the first issue it raised.
#[test]
fn init_said_line_reads_and_writes_as_zod_does() {
    cases::<InitSaidLine>(FIXTURES, "InitSaidLine");
}

/// 5 cases: what zod's parse of InitScreen wrote, or the first issue it raised.
#[test]
fn init_screen_reads_and_writes_as_zod_does() {
    cases::<InitScreen>(FIXTURES, "InitScreen");
}

/// 6 cases: what zod's parse of InitScreenId wrote, or the first issue it raised.
#[test]
fn init_screen_id_reads_and_writes_as_zod_does() {
    cases::<InitScreenId>(FIXTURES, "InitScreenId");
}

/// 6 cases: what zod's parse of InitScreenItem wrote, or the first issue it raised.
#[test]
fn init_screen_item_reads_and_writes_as_zod_does() {
    cases::<InitScreenItem>(FIXTURES, "InitScreenItem");
}

/// 7 cases: what zod's parse of LoginChoice wrote, or the first issue it raised.
#[test]
fn login_choice_reads_and_writes_as_zod_does() {
    cases::<LoginChoice>(FIXTURES, "LoginChoice");
}

/// 7 cases: what zod's parse of LoginState wrote, or the first issue it raised.
#[test]
fn login_state_reads_and_writes_as_zod_does() {
    cases::<LoginState>(FIXTURES, "LoginState");
}

/// 5 cases: what zod's parse of SetupPlace wrote, or the first issue it raised.
#[test]
fn setup_place_reads_and_writes_as_zod_does() {
    cases::<SetupPlace>(FIXTURES, "SetupPlace");
}

/// 4 cases: what zod's parse of SignInFinish wrote, or the first issue it raised.
#[test]
fn sign_in_finish_reads_and_writes_as_zod_does() {
    cases::<SignInFinish>(FIXTURES, "SignInFinish");
}
