// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of changes.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::types::changes::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_changes.json");

/// 11 cases: what zod's parse of Checkout wrote, or the first issue it raised.
#[test]
fn checkout_reads_and_writes_as_zod_does() {
    cases::<Checkout>(FIXTURES, "Checkout");
}

/// 3 cases: what zod's parse of CheckoutReply wrote, or the first issue it raised.
#[test]
fn checkout_reply_reads_and_writes_as_zod_does() {
    cases::<CheckoutReply>(FIXTURES, "CheckoutReply");
}

/// 5 cases: what zod's parse of CommitDraft wrote, or the first issue it raised.
#[test]
fn commit_draft_reads_and_writes_as_zod_does() {
    cases::<CommitDraft>(FIXTURES, "CommitDraft");
}

/// 3 cases: what zod's parse of ViewedMarks wrote, or the first issue it raised.
#[test]
fn viewed_marks_reads_and_writes_as_zod_does() {
    cases::<ViewedMarks>(FIXTURES, "ViewedMarks");
}
