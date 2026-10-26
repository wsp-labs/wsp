// SPDX-License-Identifier: AGPL-3.0-only
//! What the checkout half hands the context beside the words of a request. The TS passes each as an argument the
//! Rust seam had no place for.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/git.rs"]
mod git;
include!("workspaces_git/world.rs");

// workspaces.ts:867 `putFiles(entry.machine, [{ path: promptFile, text: question }], { before: ["umask 077"] })`: the
// prompt file holds the person's uncommitted diff, so only the login that wrote it may read it.
#[test]
fn the_draft_prompt_file_is_written_under_umask_077() {
    run(async {
        let w = world(json!({}));
        w.base.sessions.borrow_mut().push(row("s1", "ws1", "th1", "codex", "completed", 10, Some("hello from thread B")));
        script(&w, "default_agent_of", json!("claude"));
        script(&w, "ask:git.diff", diff_reply(json!([a_ts()])));
        let ask = CommitDraftAsk { workspace_id: "ws1".into(), paths: Some(vec!["a.ts".into()]), thread_id: None };
        area::commit_draft(&w.ctx, ask, origin()).await.expect("a draft answers");
        assert_eq!(*w.fake.put_before.borrow(), vec![vec!["umask 077".to_owned()]]);
    })
}
