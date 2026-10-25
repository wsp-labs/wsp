// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of tree.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::tree::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_tree.json");

/// 9 cases: what zod's parse of GitBranchCompareReply wrote, or the first issue it raised.
#[test]
fn git_branch_compare_reply_reads_and_writes_as_zod_does() {
    cases::<GitBranchCompareReply>(FIXTURES, "GitBranchCompareReply");
}

/// 7 cases: what zod's parse of GitMergeInReply wrote, or the first issue it raised.
#[test]
fn git_merge_in_reply_reads_and_writes_as_zod_does() {
    cases::<GitMergeInReply>(FIXTURES, "GitMergeInReply");
}

/// 3 cases: what zod's parse of GitStartOnReply wrote, or the first issue it raised.
#[test]
fn git_start_on_reply_reads_and_writes_as_zod_does() {
    cases::<GitStartOnReply>(FIXTURES, "GitStartOnReply");
}

/// 9 cases: what zod's parse of MergeInResult wrote, or the first issue it raised.
#[test]
fn merge_in_result_reads_and_writes_as_zod_does() {
    cases::<MergeInResult>(FIXTURES, "MergeInResult");
}

/// 9 cases: what zod's parse of TreeChild wrote, or the first issue it raised.
#[test]
fn tree_child_reads_and_writes_as_zod_does() {
    cases::<TreeChild>(FIXTURES, "TreeChild");
}

/// 3 cases: what zod's parse of TreeFact wrote, or the first issue it raised.
#[test]
fn tree_fact_reads_and_writes_as_zod_does() {
    cases::<TreeFact>(FIXTURES, "TreeFact");
}

/// 3 cases: what zod's parse of TreeRecord wrote, or the first issue it raised.
#[test]
fn tree_record_reads_and_writes_as_zod_does() {
    cases::<TreeRecord>(FIXTURES, "TreeRecord");
}
