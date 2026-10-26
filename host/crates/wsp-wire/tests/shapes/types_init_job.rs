// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of setup.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::types::init_job::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_init_job.json");

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

/// 2 cases: what zod's parse of InitKeys wrote, or the first issue it raised.
#[test]
fn init_keys_reads_and_writes_as_zod_does() {
    cases::<InitKeys>(FIXTURES, "InitKeys");
}
