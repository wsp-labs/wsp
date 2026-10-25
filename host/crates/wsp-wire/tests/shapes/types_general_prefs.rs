// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of general-prefs.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::general_prefs::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_general_prefs.json");

/// 3 cases: what zod's parse of MidTurn wrote, or the first issue it raised.
#[test]
fn mid_turn_reads_and_writes_as_zod_does() {
    cases::<MidTurn>(FIXTURES, "MidTurn");
}

/// 3 cases: what zod's parse of NewThreadIn wrote, or the first issue it raised.
#[test]
fn new_thread_in_reads_and_writes_as_zod_does() {
    cases::<NewThreadIn>(FIXTURES, "NewThreadIn");
}

/// 5 cases: what zod's parse of NotifyChoice wrote, or the first issue it raised.
#[test]
fn notify_choice_reads_and_writes_as_zod_does() {
    cases::<NotifyChoice>(FIXTURES, "NotifyChoice");
}

/// 4 cases: what zod's parse of OnQuit wrote, or the first issue it raised.
#[test]
fn on_quit_reads_and_writes_as_zod_does() {
    cases::<OnQuit>(FIXTURES, "OnQuit");
}

/// 3 cases: what zod's parse of SendKey wrote, or the first issue it raised.
#[test]
fn send_key_reads_and_writes_as_zod_does() {
    cases::<SendKey>(FIXTURES, "SendKey");
}

/// 6 cases: what zod's parse of SettleAfter wrote, or the first issue it raised.
#[test]
fn settle_after_reads_and_writes_as_zod_does() {
    cases::<SettleAfter>(FIXTURES, "SettleAfter");
}
