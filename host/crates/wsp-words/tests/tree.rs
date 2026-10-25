// SPDX-License-Identifier: AGPL-3.0-only
//! tree.ts has no TS test of its own at a3b870a6e (its words are checked through the runtime's tree tests); every
//! case below is pinned off the TS under node.

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::tree::*;

    use wsp_wire::types::tree::MergeInResult;

    #[test]
    fn a_merge_says_what_came_in_what_stopped_it_and_what_stayed_out() {
        assert_eq!(wsp_words::tree::merged_in_line("01HZZZZZZZ", "wsp/x", "01HABCDEFG", 1.0), "01HZZZZZZZ: merged wsp/x from 01HABCDEFG, 1 commit"); // mergedInLine("01HZZZZZZZ", "wsp/x", "01HABCDEFG", 1)
        assert_eq!(wsp_words::tree::merged_in_line("01HZZZZZZZ", "wsp/x", "01HABCDEFG", 2.0), "01HZZZZZZZ: merged wsp/x from 01HABCDEFG, 2 commits"); // mergedInLine("01HZZZZZZZ", "wsp/x", "01HABCDEFG", 2)
        assert_eq!(wsp_words::tree::merge_conflicts_line("01HZZZZZZZ", "wsp/x", &[] as &[&str]), "01HZZZZZZZ: conflicts with wsp/x in "); // mergeConflictsLine("01HZZZZZZZ", "wsp/x", [])
        assert_eq!(wsp_words::tree::merge_conflicts_line("01HZZZZZZZ", "wsp/x", &["a.ts"] as &[&str]), "01HZZZZZZZ: conflicts with wsp/x in a.ts"); // mergeConflictsLine("01HZZZZZZZ", "wsp/x", ["a.ts"])
        assert_eq!(wsp_words::tree::merge_conflicts_line("01HZZZZZZZ", "wsp/x", &["a.ts","b.ts"] as &[&str]), "01HZZZZZZZ: conflicts with wsp/x in a.ts, b.ts"); // mergeConflictsLine("01HZZZZZZZ", "wsp/x", ["a.ts","b.ts"])
        assert_eq!(wsp_words::tree::fix_merge_child_line("01HZZZZZZZ", "Codex", "01HABCDEFG"), "01HZZZZZZZ: asked Codex to merge 01HABCDEFG"); // fixMergeChildLine("01HZZZZZZZ", "Codex", "01HABCDEFG")
        assert_eq!(wsp_words::tree::not_in_merge_line("01HABCDEFG", 1.0), "01HABCDE holds 1 commit not pushed on its machine; it is not in this merge."); // notInMergeLine("01HABCDEFG", 1)
        assert_eq!(wsp_words::tree::not_in_merge_line("01HABCDEFG", 3.0), "01HABCDE holds 3 commits not pushed on its machine; they are not in this merge."); // notInMergeLine("01HABCDEFG", 3)
        assert_eq!(wsp_words::tree::merge_in_line(&MergeInResult { lead: "01HZZZZZZZ".into(), child: "child-long-name".into(), branch: "wsp/x".into(), merged: true, commits: 2, conflicts: vec![], unpushed: None }, None), "01HZZZZZZZ: merged wsp/x from child-long-name, 2 commits"); // mergeInLine({"lead":"01HZZZZZZZ","child":"child-long-name","branch":"wsp/x","merged":true,"commits":2,"conflicts":[]})
        assert_eq!(wsp_words::tree::merge_in_line(&MergeInResult { lead: "01HZZZZZZZ".into(), child: "child-long-name".into(), branch: "wsp/x".into(), merged: true, commits: 0, conflicts: vec![], unpushed: None }, None), "01HZZZZZZZ: nothing to merge from child-long-name"); // mergeInLine({"lead":"01HZZZZZZZ","child":"child-long-name","branch":"wsp/x","merged":true,"commits":0,"conflicts":[]})
        assert_eq!(wsp_words::tree::merge_in_line(&MergeInResult { lead: "01HZZZZZZZ".into(), child: "child-long-name".into(), branch: "wsp/x".into(), merged: false, commits: 1, conflicts: vec!["a.ts".into()], unpushed: None }, None), "01HZZZZZZZ: conflicts with wsp/x in a.ts"); // mergeInLine({"lead":"01HZZZZZZZ","child":"child-long-name","branch":"wsp/x","merged":false,"commits":1,"conflicts":["a.ts"]})
        assert_eq!(wsp_words::tree::merge_in_line(&MergeInResult { lead: "01HZZZZZZZ".into(), child: "child-long-name".into(), branch: "wsp/x".into(), merged: true, commits: 2, conflicts: vec![], unpushed: Some(1) }, Some("01HABCDEFGHJ")), "01HZZZZZZZ: merged wsp/x from child-long-name, 2 commits\n01HABCDE holds 1 commit not pushed on its machine; it is not in this merge."); // mergeInLine({"lead":"01HZZZZZZZ","child":"child-long-name","branch":"wsp/x","merged":true,"commits":2,"conflicts":[],"unpushed":1}, "01HABCDEFGHJ")
        assert_eq!(wsp_words::tree::merge_in_line(&MergeInResult { lead: "01HZZZZZZZ".into(), child: "child-long-name".into(), branch: "wsp/x".into(), merged: true, commits: 2, conflicts: vec![], unpushed: Some(1) }, None), "01HZZZZZZZ: merged wsp/x from child-long-name, 2 commits\nchild-lo holds 1 commit not pushed on its machine; it is not in this merge."); // mergeInLine({"lead":"01HZZZZZZZ","child":"child-long-name","branch":"wsp/x","merged":true,"commits":2,"conflicts":[],"unpushed":1})
        assert_eq!(wsp_words::tree::nothing_to_merge_line("01HZZZZZZZ", "01HABCDEFG"), "01HZZZZZZZ: nothing to merge from 01HABCDEFG"); // nothingToMergeLine("01HZZZZZZZ", "01HABCDEFG")
        assert_eq!(wsp_words::tree::merge_child_prompt(&MergeChild { lead_branch: "main".into(), child_branch: "wsp/x".into(), remote: "origin".into(), conflicts: vec![] }), "Fetch origin wsp/x and merge it into main with a merge commit; resolve the conflicts; run the tests; commit."); // mergeChildPrompt({leadBranch: "main", childBranch: "wsp/x", remote: "origin", conflicts: []})
        assert_eq!(wsp_words::tree::merge_child_prompt(&MergeChild { lead_branch: "main".into(), child_branch: "wsp/x".into(), remote: "origin".into(), conflicts: vec!["a.ts".into(), "b.ts".into()] }), "Fetch origin wsp/x and merge it into main with a merge commit; resolve the conflicts in a.ts, b.ts; run the tests; commit."); // mergeChildPrompt({leadBranch: "main", childBranch: "wsp/x", remote: "origin", conflicts: ["a.ts","b.ts"]})
    }

    #[test]
    fn a_childs_branch_and_its_push() {
        assert_eq!(wsp_words::tree::child_started_line("01HABCDEFG", "wsp/x"), "01HABCDEFG starts on wsp/x as pushed"); // childStartedLine("01HABCDEFG", "wsp/x")
        assert_eq!(wsp_words::tree::pushed_for_child_line("wsp/x", "01HABCDEFG"), "pushed wsp/x so 01HABCDEFG starts from your work"); // pushedForChildLine("wsp/x", "01HABCDEFG")
        assert_eq!(wsp_words::tree::uncommitted_stayed(1.0, "01HZZZZZZZ"), "1 uncommitted change in 01HZZZZZZZ did not travel"); // uncommittedStayed(1, "01HZZZZZZZ")
        assert_eq!(wsp_words::tree::uncommitted_stayed(3.0, "01HZZZZZZZ"), "3 uncommitted changes in 01HZZZZZZZ did not travel"); // uncommittedStayed(3, "01HZZZZZZZ")
        assert_eq!(wsp_words::tree::fork_needs_push_line("01HZZZZZZZ", "no remote"), "the child was not made, since 01HZZZZZZZ's branch could not reach it: no remote"); // forkNeedsPushLine("01HZZZZZZZ", "no remote")
        assert_eq!(wsp_words::tree::no_remote_for_tree_line("web"), "web has no remote, so a child on another computer cannot be merged; add a remote to the project to merge across computers"); // noRemoteForTreeLine("web")
        assert_eq!(wsp_words::tree::cannot_push_from_line("spoo", "no gh"), "can't push from spoo: no gh"); // cannotPushFromLine("spoo", "no gh")
    }

    #[test]
    fn a_merge_refused() {
        assert_eq!(wsp_words::tree::lead_busy_refusal("01HZZZZZZZ", &["01HABCDEFGHJ"] as &[&str]), "01HZZZZZZZ has a turn running in thread 01HABCDE; merge once it ends, or from inside that turn"); // leadBusyRefusal("01HZZZZZZZ", ["01HABCDEFGHJ"])
        assert_eq!(wsp_words::tree::lead_busy_refusal("01HZZZZZZZ", &["01HABCDEFGHJ","01HQQQQQQQQQ"] as &[&str]), "01HZZZZZZZ has a turn running in thread 01HABCDE, 01HQQQQQ; merge once it ends, or from inside that turn"); // leadBusyRefusal("01HZZZZZZZ", ["01HABCDEFGHJ","01HQQQQQQQQQ"])
        assert_eq!(wsp_words::tree::not_the_leads_child_refusal("01HABCDEFG", "01HZZZZZZZ"), "01HABCDEFG is not a child of 01HZZZZZZZ; a lead merges only its own children"); // notTheLeadsChildRefusal("01HABCDEFG", "01HZZZZZZZ")
        assert_eq!(wsp_words::tree::merge_into_own_refusal("01HZZZZZZZ"), "thread 01HZZZZZ may merge a child only into its own folder"); // mergeIntoOwnRefusal("01HZZZZZZZ")
        assert_eq!(wsp_words::tree::child_on_no_branch_refusal("01HABCDEFG"), "01HABCDEFG is on no branch, so there is nothing to merge from it"); // childOnNoBranchRefusal("01HABCDEFG")
    }

    #[test]
    fn the_tree_words() {
        assert_eq!(wsp_words::tree::tree_words::ahead_of(3.0, "main"), "3 ahead of main"); // TREE_WORDS.aheadOf(3, "main")
        assert_eq!(format!("{} / {}", wsp_words::tree::tree_words::conflicts_in(1.0), wsp_words::tree::tree_words::conflicts_in(4.0)), "conflicts in 1 file / conflicts in 4 files"); // TREE_WORDS.conflictsIn(1) + " / " + TREE_WORDS.conflictsIn(4)
    }
}
