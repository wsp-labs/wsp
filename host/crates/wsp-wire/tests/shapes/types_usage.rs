// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of usage.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::usage::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_usage.json");

/// 7 cases: what zod's parse of AccountLimit wrote, or the first issue it raised.
#[test]
fn account_limit_reads_and_writes_as_zod_does() {
    cases::<AccountLimit>(FIXTURES, "AccountLimit");
}

/// 4 cases: what zod's parse of AccountNote wrote, or the first issue it raised.
#[test]
fn account_note_reads_and_writes_as_zod_does() {
    cases::<AccountNote>(FIXTURES, "AccountNote");
}

/// 4 cases: what zod's parse of AccountRoad wrote, or the first issue it raised.
#[test]
fn account_road_reads_and_writes_as_zod_does() {
    cases::<AccountRoad>(FIXTURES, "AccountRoad");
}

/// 6 cases: what zod's parse of AccountRow wrote, or the first issue it raised.
#[test]
fn account_row_reads_and_writes_as_zod_does() {
    cases::<AccountRow>(FIXTURES, "AccountRow");
}

/// 3 cases: what zod's parse of AccountsAnswer wrote, or the first issue it raised.
#[test]
fn accounts_answer_reads_and_writes_as_zod_does() {
    cases::<AccountsAnswer>(FIXTURES, "AccountsAnswer");
}

/// 6 cases: what zod's parse of BankedResets wrote, or the first issue it raised.
#[test]
fn banked_resets_reads_and_writes_as_zod_does() {
    cases::<BankedResets>(FIXTURES, "BankedResets");
}

/// 6 cases: what zod's parse of HarnessLimit wrote, or the first issue it raised.
#[test]
fn harness_limit_reads_and_writes_as_zod_does() {
    cases::<HarnessLimit>(FIXTURES, "HarnessLimit");
}

/// 7 cases: what zod's parse of LimitKind wrote, or the first issue it raised.
#[test]
fn limit_kind_reads_and_writes_as_zod_does() {
    cases::<LimitKind>(FIXTURES, "LimitKind");
}

/// 4 cases: what zod's parse of LimitStatus wrote, or the first issue it raised.
#[test]
fn limit_status_reads_and_writes_as_zod_does() {
    cases::<LimitStatus>(FIXTURES, "LimitStatus");
}

/// 5 cases: what zod's parse of LimitWindow wrote, or the first issue it raised.
#[test]
fn limit_window_reads_and_writes_as_zod_does() {
    cases::<LimitWindow>(FIXTURES, "LimitWindow");
}

/// 5 cases: what zod's parse of ReadingsRange wrote, or the first issue it raised.
#[test]
fn readings_range_reads_and_writes_as_zod_does() {
    cases::<ReadingsRange>(FIXTURES, "ReadingsRange");
}

/// 5 cases: what zod's parse of ResetAnswer wrote, or the first issue it raised.
#[test]
fn reset_answer_reads_and_writes_as_zod_does() {
    cases::<ResetAnswer>(FIXTURES, "ResetAnswer");
}

/// 5 cases: what zod's parse of ResetCredit wrote, or the first issue it raised.
#[test]
fn reset_credit_reads_and_writes_as_zod_does() {
    cases::<ResetCredit>(FIXTURES, "ResetCredit");
}

/// 7 cases: what zod's parse of ResetOutcome wrote, or the first issue it raised.
#[test]
fn reset_outcome_reads_and_writes_as_zod_does() {
    cases::<ResetOutcome>(FIXTURES, "ResetOutcome");
}

/// 3 cases: what zod's parse of TurnLimit wrote, or the first issue it raised.
#[test]
fn turn_limit_reads_and_writes_as_zod_does() {
    cases::<TurnLimit>(FIXTURES, "TurnLimit");
}

/// 3 cases: what zod's parse of UsageAccountEvent wrote, or the first issue it raised.
#[test]
fn usage_account_event_reads_and_writes_as_zod_does() {
    cases_tagged::<UsageAccountEvent>(FIXTURES, "UsageAccountEvent", "type", UsageAccountEvent::TYPE);
}

/// 4 cases: what zod's parse of UsageDay wrote, or the first issue it raised.
#[test]
fn usage_day_reads_and_writes_as_zod_does() {
    cases::<UsageDay>(FIXTURES, "UsageDay");
}

/// 4 cases: what zod's parse of UsageKey wrote, or the first issue it raised.
#[test]
fn usage_key_reads_and_writes_as_zod_does() {
    cases::<UsageKey>(FIXTURES, "UsageKey");
}

/// 4 cases: what zod's parse of UsageRange wrote, or the first issue it raised.
#[test]
fn usage_range_reads_and_writes_as_zod_does() {
    cases::<UsageRange>(FIXTURES, "UsageRange");
}

/// 7 cases: what zod's parse of UsageRow wrote, or the first issue it raised.
#[test]
fn usage_row_reads_and_writes_as_zod_does() {
    cases::<UsageRow>(FIXTURES, "UsageRow");
}

/// 7 cases: what zod's parse of UsageSplit wrote, or the first issue it raised.
#[test]
fn usage_split_reads_and_writes_as_zod_does() {
    cases::<UsageSplit>(FIXTURES, "UsageSplit");
}

/// 3 cases: what zod's parse of UsageTokens wrote, or the first issue it raised.
#[test]
fn usage_tokens_reads_and_writes_as_zod_does() {
    cases::<UsageTokens>(FIXTURES, "UsageTokens");
}

/// 6 cases: what zod's parse of UsedAnswer wrote, or the first issue it raised.
#[test]
fn used_answer_reads_and_writes_as_zod_does() {
    cases::<UsedAnswer>(FIXTURES, "UsedAnswer");
}

/// 6 cases: what zod's parse of UsedRow wrote, or the first issue it raised.
#[test]
fn used_row_reads_and_writes_as_zod_does() {
    cases::<UsedRow>(FIXTURES, "UsedRow");
}
