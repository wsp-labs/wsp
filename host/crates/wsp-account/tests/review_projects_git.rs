// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the doors that read in the checkout half (workspaces.ts:780-1053): what the merge in accepts from the
//! daemon, and where the pull request diff is first read. Expected values are read off the TS lines named on each test
//! and printed by oracle/zz-review-protocol.oracle.ts (`MERGEIN noConflicts false`, `MERGEIN noBranch false`).
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/git.rs"]
mod git;
include!("workspaces_git/world.rs");

// R-PRJ-G1. `GitMergeInReply.parse` (workspaces.ts:1041) needs `branch` and `conflicts` (tree.ts:22); a reply with
// either missing is a refusal and the child's record is not touched. The Rust reads the reply into a private struct
// that defaults `conflicts` and has no `branch` (git.rs:126-136), so a daemon that leaves them out has "merged" nothing
// and the child's record keeps `conflicts: []`, which reads as a merge with no conflicts.
#[test]
fn a_merge_reply_missing_its_branch_or_its_conflicts_is_refused_and_keeps_nothing() {
    run(async {
        for (name, reply) in [
            ("no conflicts", json!({ "branch": "child/one", "merged": false, "commits": 0 })),
            ("no branch", json!({ "merged": true, "commits": 2, "oid": "d00d", "conflicts": [] })),
        ] {
            let w = merge_in_world(json!({}), "cloud", "cloud", json!({}));
            script(&w, "ask:git.mergeIn", reply);
            let got = area::merge_in(&w.ctx, ask_in(None), origin()).await;
            assert!(got.is_err(), "{name}: the merge in answered {:?}", got.map(|done| j(&done)));
            assert!(w.base.args_of("keep_tree").is_empty(), "{name}: the child's record was written");
        }
    });
}

// R-PRJ-G2. The pull request diff is read on this computer first, with this computer's home as the folder the command
// runs in: `readHost` (account/pull-requests.ts:27) builds the frame with `home`, and only a computer with no host
// command line falls back to the copy's checkout (:31). The Rust builds the frame once, with the copy's checkout
// (git.rs:900-904), so this computer is asked to run in a folder that is the copy's, which a fork on a provider has only
// on its own machine.
#[test]
fn the_pull_request_diff_is_first_asked_in_this_computers_home() {
    run(async {
        let w = world(json!({}));
        script(&w, "read_host", json!({ "diff": "diff --git a/a.ts b/a.ts\n-one\n+two\n", "truncated": false, "left": [] }));
        area::pull_request_diff(&w.ctx, "ws1", origin()).await.expect("the diff reads");
        let asked = w.base.args_of("read_host");
        assert_eq!(asked[0]["ask"]["cwd"], json!(HOME), "the first road ran in {}", asked[0]["ask"]["cwd"]);
    });
}
