// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of bring-back.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::types::bring_back::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_bring_back.json");

/// 6 cases: what zod's parse of BringBackResult wrote, or the first issue it raised.
#[test]
fn bring_back_result_reads_and_writes_as_zod_does() {
    cases::<BringBackResult>(FIXTURES, "BringBackResult");
}

/// 4 cases: what zod's parse of GitPrReply wrote, or the first issue it raised.
#[test]
fn git_pr_reply_reads_and_writes_as_zod_does() {
    cases::<GitPrReply>(FIXTURES, "GitPrReply");
}

/// 5 cases: what zod's parse of GitPushReply wrote, or the first issue it raised.
#[test]
fn git_push_reply_reads_and_writes_as_zod_does() {
    cases::<GitPushReply>(FIXTURES, "GitPushReply");
}
