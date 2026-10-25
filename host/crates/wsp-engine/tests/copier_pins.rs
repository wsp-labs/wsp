// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests for copier: the empty and odd inputs the TS cases leave out.
mod common;

use wsp_engine::copier::*;

#[test]
fn a_refusal_that_said_nothing_has_its_own_sentence_and_stdout_stands_in_for_an_empty_stderr() {
    assert_eq!(refusal_line("/bin/wsp-daemon", &common::res(1, "", "  \n")), "the copy failed and said nothing");
    assert_eq!(refusal_line("/bin/wsp-daemon", &common::res(1, "first\nthe last word\n", "")), "the last word");
    assert_eq!(refusal_line("/bin/wsp-daemon", &common::res(1, "ignored", "only stderr counts\n")), "only stderr counts");
    // Exit 127, a child that never started, is not the verb's refusal.
    assert_eq!(
        refusal_line("/bin/wsp-daemon", &common::res(127, "", "")),
        "the daemon's copy verb failed (exit 127); the host log has its output"
    );
}

#[test]
fn a_copy_with_no_exclusion_base_or_road_is_the_bare_line() {
    let ask = CopyAsk { from: "/a b".into(), to: "/c'd".into(), base: None, exclude: vec![], size_line_bytes: 0, road: None };
    assert_eq!(argv_for(&ask), vec!["copy", "make", "--from", "/a b", "--to", "/c'd", "--size-line-bytes", "0"]);
}

#[tokio::test]
async fn the_stand_ins_worktree_folder_replaces_every_char_outside_the_safe_set() {
    let home = tempfile::tempdir().unwrap();
    let copier = fake_copier(None, None);
    let ask = WorktreeAsk {
        from: "/repo".into(),
        home: home.path().to_str().unwrap().into(),
        project: "prj_1".into(),
        branch: "feat/x y@é.1_-".into(),
        modules: vec![],
        checkpoint: None,
    };
    let made = copier.worktree(&ask).await.unwrap();
    let want = home.path().join("worktrees/prj_1/feat-x-y--.1_-");
    assert_eq!(made.path, want.to_str().unwrap());
    assert!(want.is_dir());
    assert_eq!(made.branch, "feat/x y@é.1_-");
    assert!(made.made && made.fresh && made.carried.is_empty() && made.modules.is_empty() && made.ms == 1);
    assert_eq!(copier.log.lock().unwrap().worktrees, vec![ask]);
}
