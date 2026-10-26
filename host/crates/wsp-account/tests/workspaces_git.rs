// SPDX-License-Identifier: AGPL-3.0-only
//! Everything that runs in or reads a workspace's checkout: commands, bring back, the folder doors, the Changes pane,
//! the pull request page and its acts, fix, merge, update and merge in. Ported from workspaces.ts:647-1053 at
//! 65f6b89c7 and the cases of changes, bring-back, tree, pull-request and folder-threads tests (packages/runtime/test)
//! that pin it, over a fake context whose daemon `ask` records each op. Every sentence is the TS protocol's own text.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/git.rs"]
mod git;
include!("workspaces_git/world.rs");

// ---------------------------------------------------------------------------------------------------------------
// Commands

// workspaces.ts:647-652; local-workspace.test.ts:891
#[test]
fn exec_mounts_the_worktree_after_the_copy_block() {
    run(async {
        let w = world(json!({}));
        script(&w, "machine_exec", json!({ "exitCode": 3, "stdout": "out", "stderr": "err" }));
        let got = area::exec(&w.ctx, "ws1", "make test", Some(5000), origin()).await.expect("exec answers");
        assert_eq!(j(&got), json!({ "exitCode": 3, "stdout": "out", "stderr": "err" }));
        assert_eq!(w.base.names(), vec!["entry_of", "copy_blocked", "worktree_mounted", "machine_exec"]);
        assert_eq!(w.base.args_of("entry_of"), vec![json!({ "id": "ws1", "thread": null, "act": "exec", "scoped": false })]);
        assert_eq!(w.base.args_of("machine_exec"), vec![json!({ "id": "ws1", "cmd": "make test", "timeoutMs": 5000 })]);

        let blocked = world(json!({}));
        script(&blocked, "copy_blocked", err_answer("ws1 runs on a computer that blocks copies", Some("usage")));
        refusal(
            area::exec(&blocked.ctx, "ws1", "make test", None, origin()).await,
            "ws1 runs on a computer that blocks copies",
            Some(Kind::Usage),
        );
        assert_eq!(blocked.base.names(), vec!["entry_of", "copy_blocked"]);
    })
}

// workspaces.ts:654-691; local-workspace.test.ts:865, sub-oom.test.ts:53. The ended-reason race and the `execs` registry
// are not in GitCtx, so only what the area does with the context's stream is pinned here.
#[test]
fn exec_stream_ends_the_reader_with_the_reason_and_unregisters() {
    run(async {
        let w = world(json!({}));
        script(&w, "exec_stream_of", json!({ "lines": ["a", "b"], "exit": 0, "ranIn": CWD }));
        let argv: Vec<String> = vec!["ls".into(), "-la".into()];
        let got = area::exec_stream(&w.ctx, "ws1", argv, Some("sub".into()), origin()).await.expect("a stream starts");
        assert_eq!(w.base.names(), vec!["entry_of", "copy_blocked", "worktree_mounted", "exec_stream_of"]);
        assert_eq!(w.base.args_of("entry_of"), vec![json!({ "id": "ws1", "thread": null, "act": "exec", "scoped": false })]);
        assert_eq!(w.base.args_of("exec_stream_of"), vec![json!({ "id": "ws1", "argv": ["ls", "-la"], "cwd": "sub" })]);
        assert_eq!(got.ran_in, Some(CWD.to_owned()));
        let lines: Vec<String> = got.stream.lines().collect::<Vec<_>>().await.into_iter().map(|l| l.expect("a line")).collect();
        assert_eq!(lines, vec!["a", "b"]);
        assert_eq!(got.stream.exited().await, Some(0));

        let blocked = world(json!({}));
        script(&blocked, "copy_blocked", err_answer("blocked", Some("usage")));
        assert!(area::exec_stream(&blocked.ctx, "ws1", vec!["ls".into()], None, origin()).await.is_err());
        assert_eq!(blocked.base.names(), vec!["entry_of", "copy_blocked"]);
    })
}

// workspaces.ts:647-1053, the ten doors the spec's trap #1383 names; box-lead.test.ts:226
#[test]
fn copy_blocked_comes_first_at_every_door_that_acts_in_a_copy() {
    run(async {
        for door in ["exec", "exec_stream", "bring_back", "discard", "commit", "commit_draft", "send", "fix", "update", "merge_in"] {
            let w = world(json!({}));
            add(&w, "kid1", "cloud", json!({ "name": "helper" }));
            script(&w, "copy_blocked", err_answer("no acting in a copy here", Some("usage")));
            let said = err_of(knock(&w, door).await);
            assert_eq!(said.message, "no acting in a copy here", "{door}");
            assert_eq!(said.kind, Some(Kind::Usage), "{door}");
            assert_eq!(w.base.names().last().map(String::as_str), Some("copy_blocked"), "{door}: nothing runs after the block");
            assert!(w.base.asks().is_empty(), "{door}: no daemon op");
            assert!(w.base.persisted.borrow().is_empty() && w.base.events.borrow().is_empty(), "{door}: nothing stored or said");
            for acting in
                ["queued", "machine_exec", "exec_stream_of", "update_copy", "put_files", "read_checkout", "to_first_thread", "keep_tree"]
            {
                assert!(!w.base.names().iter().any(|n| n == acting), "{door}: {acting} ran past the block");
            }
        }
    })
}

// workspaces.ts:796-807, 893-898, 981-1006: the three doors that do not call copyBlocked
#[test]
fn checkout_view_and_merge_never_ask_copy_blocked() {
    run(async {
        let w = world(json!({}));
        script(&w, "copy_blocked", err_answer("blocked", Some("usage")));
        hold_fact(&w, json!({}));
        script(&w, "merge_settings", json!({ "methods": ["merge", "squash"], "defaultMethod": "squash", "autoMerge": false }));
        script(&w, "ask:git.prMerge", json!({ "merged": true, "autoArmed": false }));
        area::checkout(&w.ctx, "ws1", origin(), true).await.expect("checkout reads");
        area::pull_request_view(&w.ctx, "ws1", false, origin()).await.expect("the page reads");
        area::merge(&w.ctx, MergeAsk { workspace_id: "ws1".into(), ..Default::default() }, origin()).await.expect("the merge runs");
        assert!(!w.base.names().iter().any(|n| n == "copy_blocked"));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Bring back

// bring-back.test.ts:88
#[test]
fn bring_back_pushes_and_opens_a_pr_in_the_queue() {
    run(async {
        let w = world(json!({}));
        script(&w, "ask:git.push", push_reply());
        script(&w, "ask:git.pr", json!({ "pr": pr_json(), "created": true }));
        let back = area::bring_back(&w.ctx, "ws1", Some("the pricing page".into()), Some("what it does".into()), origin())
            .await
            .expect("bring back answers");
        assert_eq!(
            w.base.asks(),
            vec![
                json!({ "op": "git.push", "cwd": CWD, "base": "main" }),
                json!({ "op": "git.pr", "cwd": CWD, "base": "main", "title": "the pricing page", "body": "what it does" }),
            ]
        );
        assert_eq!(
            names_after(
                &w,
                &[
                    "spawn_guard",
                    "entry_of",
                    "copy_blocked",
                    "queued",
                    "with_daemon",
                    "ask",
                    "take_pull_request",
                    "read_lead_of",
                    "keep_tree"
                ]
            ),
            vec!["spawn_guard", "entry_of", "copy_blocked", "queued", "with_daemon", "ask", "ask", "take_pull_request", "read_lead_of"]
        );
        assert_eq!(w.base.args_of("spawn_guard"), vec![json!({ "act": "bring_back", "scoped": false })]);
        assert_eq!(w.base.args_of("queued"), vec![json!({ "id": "ws1" })]);
        let mut expected = pushed();
        expected["pr"] = pr_json();
        assert_eq!(j(&back), expected);
        let mut taken = pr_json();
        taken["readAt"] = json!(1_000_000);
        assert_eq!(w.base.args_of("take_pull_request"), vec![json!({ "id": "ws1", "pr": taken })]);
        assert_eq!(w.base.args_of("read_lead_of"), vec![json!({ "id": "ws1" })]);
        assert!(w.base.persisted.borrow().is_empty() && w.base.events.borrow().is_empty());
    })
}

// bring-back.test.ts:108
#[test]
fn bring_back_a_project_that_named_no_base_leaves_it_to_the_checkout() {
    run(async {
        let w = world_on(json!({}), json!({ "base": null }));
        script(&w, "ask:git.push", push_reply());
        script(&w, "ask:git.pr", json!({ "pr": pr_json(), "created": true }));
        area::bring_back(&w.ctx, "ws1", None, None, origin()).await.expect("bring back answers");
        assert_eq!(w.base.asks(), vec![json!({ "op": "git.push", "cwd": CWD }), json!({ "op": "git.pr", "cwd": CWD })]);
    })
}

// bring-back.test.ts:143, 173: a child measures against the branch its parent was on at the fork, off its own record
#[test]
fn bring_back_a_child_brings_back_against_the_branch_its_record_was_forked_from() {
    run(async {
        let w = world(json!({ "base": "pricing-page" }));
        script(&w, "ask:git.push", push_reply());
        script(&w, "ask:git.pr", json!({ "pr": pr_json(), "created": true }));
        area::bring_back(&w.ctx, "ws1", None, None, origin()).await.expect("bring back answers");
        assert_eq!(
            w.base.asks(),
            vec![
                json!({ "op": "git.push", "cwd": CWD, "base": "pricing-page" }),
                json!({ "op": "git.pr", "cwd": CWD, "base": "pricing-page" })
            ]
        );
        assert!(w.base.names().iter().all(|n| n != "read_checkout" && n != "child_of"), "nothing is asked of the parent");
    })
}

// workspaces.ts:716-745; tree.test.ts:208 (a child of a lead thread opens no pull request)
#[test]
fn bring_back_of_a_lead_threads_child_opens_nothing() {
    run(async {
        let w = world(json!({ "parentThreadId": "lt1", "parentWorkspaceId": "lead1" }));
        script(&w, "ask:git.push", push_reply());
        let back = area::bring_back(&w.ctx, "ws1", Some("title".into()), None, origin()).await.expect("bring back answers");
        assert_eq!(w.base.asks(), vec![json!({ "op": "git.push", "cwd": CWD, "base": "main" })]);
        let mut expected = pushed();
        expected["note"] = json!("pricing-page is pushed; the lead opens the pull request");
        assert_eq!(j(&back), expected);
        assert!(w.base.names().iter().all(|n| n != "take_pull_request"));
        assert_eq!(w.base.args_of("read_lead_of"), vec![json!({ "id": "ws1" })]);
    })
}

// start-review.test.ts:230
#[test]
fn bring_back_refuses_an_unpushable_fork() {
    run(async {
        let from = json!({ "kind": "pull_request", "repo": "o/r", "number": 7, "url": "u", "title": "t", "head": { "branch": "b", "fork": { "owner": "waldyrious", "pushable": false } } });
        let w = world(json!({ "from": from }));
        refusal(
            area::bring_back(&w.ctx, "ws1", None, None, origin()).await,
            "can't push to waldyrious's fork: they did not allow edits from maintainers",
            None,
        );
        assert_eq!(w.base.names(), vec!["spawn_guard", "entry_of", "copy_blocked"]);
        assert!(w.base.asks().is_empty() && w.base.persisted.borrow().is_empty());

        let open = json!({ "kind": "pull_request", "repo": "o/r", "number": 7, "url": "u", "title": "t", "head": { "branch": "b", "fork": { "owner": "waldyrious", "pushable": true } } });
        let ok = world(json!({ "from": open }));
        script(&ok, "ask:git.push", push_reply());
        script(&ok, "ask:git.pr", json!({ "pr": pr_json(), "created": false }));
        area::bring_back(&ok.ctx, "ws1", None, None, origin()).await.expect("a pushable fork pushes");
        assert_eq!(ok.base.asks().len(), 2);
    })
}

// workspaces.ts:696-700, withCloses (start.ts:150)
#[test]
fn bring_back_closes_the_issue_it_came_from() {
    run(async {
        let from = json!({ "kind": "issue", "repo": "o/r", "number": 9, "url": "u", "title": "t" });
        for (given, sent) in
            [(Some("fixes the cart"), "fixes the cart\n\nCloses #9"), (None, "Closes #9"), (Some("Fixes #9 for good"), "Fixes #9 for good")]
        {
            let w = world(json!({ "from": from }));
            script(&w, "ask:git.push", push_reply());
            script(&w, "ask:git.pr", json!({ "pr": pr_json(), "created": true }));
            area::bring_back(&w.ctx, "ws1", Some("title".into()), given.map(str::to_owned), origin()).await.expect("bring back answers");
            assert_eq!(w.base.asks()[1], json!({ "op": "git.pr", "cwd": CWD, "base": "main", "title": "title", "body": sent }));
        }
    })
}

// workspaces.ts:715; defaultBranchRefused (internal.ts:1089), defaultBranchRefusal (words/computer.ts:268)
#[test]
fn bring_back_on_the_default_branch_says_so() {
    run(async {
        let w = world(json!({}));
        script(&w, "ask:git.push", err_answer("on main", Some("on-default-branch")));
        script(&w, "ask:git.status", status_json("main", json!([])));
        let said = "cart is on main, the project's default branch, with commits the remote lacks, and wsp never pushes main.";
        let fix = "Move the commits in cart onto a branch of their own, then bring it back again.";
        let e = refusal(area::bring_back(&w.ctx, "ws1", None, None, origin()).await, &format!("{said} {fix}"), Some(Kind::Usage));
        assert_eq!(e.fix.as_deref(), Some(fix));
        assert_eq!(w.base.asks(), vec![json!({ "op": "git.push", "cwd": CWD, "base": "main" }), json!({ "op": "git.status", "cwd": CWD })]);
        assert!(w.base.names().iter().all(|n| n != "keep_tree" && n != "take_pull_request"));
    })
}

// tree.test.ts:351 (keeps a push the child could not make on the child's row, and clears it once a push lands)
#[test]
fn bring_back_keeps_a_push_refused_on_the_childs_tree() {
    run(async {
        let said = "github.com has no sign-in on this computer";
        let child = json!({ "parentThreadId": "lt1", "parentWorkspaceId": "lead1", "tree": { "conflicts": ["a.txt"] } });
        let w = world(child.clone());
        script(&w, "ask:git.push", err_answer(said, Some("no-git-credential")));
        let e = refusal(area::bring_back(&w.ctx, "ws1", None, None, origin()).await, said, Some(Kind::Other("no-git-credential".into())));
        assert!(e.fix.is_none());
        assert_eq!(w.base.args_of("keep_tree"), vec![json!({ "id": "ws1", "tree": { "conflicts": ["a.txt"], "pushRefused": said } })]);
        assert_eq!(w.base.asks(), vec![json!({ "op": "git.push", "cwd": CWD, "base": "main" })]);

        let root = world(json!({ "tree": { "conflicts": ["a.txt"] } }));
        script(&root, "ask:git.push", err_answer(said, Some("no-git-credential")));
        assert!(area::bring_back(&root.ctx, "ws1", None, None, origin()).await.is_err());
        assert!(root.base.args_of("keep_tree").is_empty(), "only a child's tree keeps a refused push");

        let later = world(
            json!({ "parentThreadId": "lt1", "parentWorkspaceId": "lead1", "tree": { "conflicts": ["a.txt"], "pushRefused": said } }),
        );
        script(&later, "ask:git.push", push_reply());
        area::bring_back(&later.ctx, "ws1", None, None, origin()).await.expect("the push lands");
        assert_eq!(later.base.args_of("keep_tree"), vec![json!({ "id": "ws1", "tree": { "conflicts": ["a.txt"] } })]);
    })
}

// bring-back.test.ts:212
#[test]
fn bring_back_answers_a_pr_refusal_beside_the_push() {
    run(async {
        let said = "gh said: could not create pull request";
        let w = world(json!({}));
        script(&w, "ask:git.push", push_reply());
        script(&w, "ask:git.pr", err_answer(said, None));
        let back = area::bring_back(&w.ctx, "ws1", None, None, origin()).await.expect("the push half stands");
        let mut expected = pushed();
        expected["refused"] = json!(said);
        assert_eq!(j(&back), expected);
        assert!(w.base.names().iter().all(|n| n != "take_pull_request"));
        assert_eq!(w.base.args_of("read_lead_of"), vec![json!({ "id": "ws1" })]);
    })
}

// bring-back.test.ts:201
#[test]
fn bring_back_a_machine_with_no_command_line_for_the_git_host_comes_back_as_the_note() {
    run(async {
        let note = "no signed-in command line for github.com is on this machine, so no pull request is opened";
        let w = world(json!({}));
        script(&w, "ask:git.push", push_reply());
        script(&w, "ask:git.pr", err_answer(note, Some("no-host-cli")));
        let back = area::bring_back(&w.ctx, "ws1", None, None, origin()).await.expect("the push half stands");
        let mut expected = pushed();
        expected["note"] = json!(note);
        assert_eq!(j(&back), expected);
    })
}

// bring-back.test.ts:192, 230
#[test]
fn bring_back_a_refused_push_is_the_whole_verbs_refusal_and_asks_nothing_after() {
    run(async {
        let said = "main is the project's base; bring work back from a branch of its own";
        let w = world(json!({}));
        script(&w, "ask:git.push", err_answer(said, None));
        refusal(area::bring_back(&w.ctx, "ws1", None, None, origin()).await, said, None);
        assert_eq!(w.base.asks(), vec![json!({ "op": "git.push", "cwd": CWD, "base": "main" })]);
        assert!(w.base.names().iter().all(|n| n != "take_pull_request" && n != "read_lead_of"));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Folder doors

// workspaces.ts:748-757
#[test]
fn folder_for_and_take_setup_line_wait_for_the_host() {
    run(async {
        let w = world(json!({}));
        script(&w, "folder_for", json!({ "id": "ws1", "cwd": "/x", "setup": "pnpm install" }));
        script(&w, "take_setup_line", json!("pnpm install"));
        let ask = FolderForAsk { project: Some("cart".into()), branch: Some("feat/a".into()), cwd: None, picks: None, opens: false };
        let got = area::folder_for(&w.ctx, ask, origin()).await.expect("folder_for answers");
        assert_eq!(j(&got.workspace), j(&view_of(&w.base, &rec(&w.ws))));
        assert_eq!(got.cwd.as_deref(), Some("/x"));
        assert_eq!(got.setup.as_deref(), Some("pnpm install"));
        assert_eq!(w.base.names(), vec!["ready", "folder_for"]);
        assert_eq!(w.base.args_of("folder_for"), vec![json!({ "project": "cart", "branch": "feat/a", "cwd": null, "opens": false })]);

        let line = area::take_setup_line(&w.ctx, "ws1", origin()).await.expect("the line answers");
        assert_eq!(line.as_deref(), Some("pnpm install"));
        assert_eq!(w.base.names(), vec!["ready", "folder_for", "ready", "entry_of", "take_setup_line"]);
        assert_eq!(w.base.args_of("take_setup_line"), vec![json!({ "id": "ws1" })]);
    })
}

// folder-threads.test.ts:199; workspaces.ts:759-765
#[test]
fn folder_refuses_off_this_computer() {
    run(async {
        let off = world_on(json!({}), json!({ "computer": "solari" }));
        refusal(
            area::folder(&off.ctx, "cart", origin()).await,
            "cart is not on this computer, so no thread runs in its folder here",
            Some(Kind::Usage),
        );
        assert_eq!(off.base.names(), vec!["ready", "resolve_project"]);

        let here = world(json!({}));
        let got = area::folder(&here.ctx, "p1", origin()).await.expect("the folder's view answers");
        assert_eq!(j(&got), j(&view_of(&here.base, &rec(&here.ws))));
        assert_eq!(here.base.names(), vec!["ready", "resolve_project", "refuse_recording", "project_folder"]);
        assert_eq!(here.base.args_of("refuse_recording"), vec![json!({ "word": "cart" })]);
        assert_eq!(here.base.args_of("project_folder"), vec![json!({ "project": "p1" })]);

        let barred = world(json!({}));
        script(&barred, "refuse_recording", err_answer("a recording takes no thread", Some("usage")));
        refusal(area::folder(&barred.ctx, "cart", origin()).await, "a recording takes no thread", Some(Kind::Usage));
        assert!(barred.base.names().iter().all(|n| n != "project_folder"));
    })
}

// folder-threads.test.ts:1002; workspaces.ts:767-778
#[test]
fn worktree_answers_path_branch_made() {
    run(async {
        let w = world(json!({}));
        let tree = add(&w, "wt1", "local", json!({ "worktree": { "path": "/wt/feat-x", "branch": "feat/x", "made": true } }));
        let _ = tree;
        script(&w, "worktree_folder", json!("wt1"));
        let made = area::worktree(&w.ctx, "cart", "feat/x", origin()).await.expect("a worktree answers");
        assert_eq!(j(&made), json!({ "path": "/wt/feat-x", "branch": "feat/x", "made": true }));
        assert_eq!(
            w.base.args_of("worktree_folder"),
            vec![json!({ "project": "p1", "top": "/home/me/cart", "branch": "feat/x", "madeFor": null })]
        );

        let by_thread =
            area::worktree(&w.ctx, "p1", "feat/x", Some(thread("th1", "ws1", "root1"))).await.expect("a thread's worktree answers");
        assert_eq!(j(&by_thread), json!({ "path": "/wt/feat-x", "branch": "feat/x", "made": true }));
        assert_eq!(w.base.args_of("worktree_folder")[1]["madeFor"], json!("root1"));

        let bare = world(json!({}));
        add(&bare, "wt1", "local", json!({}));
        script(&bare, "worktree_folder", json!("wt1"));
        let got = area::worktree(&bare.ctx, "cart", "feat/x", origin()).await.expect("a folder with no worktree answers");
        assert_eq!(j(&got), json!({ "path": "/home/me/cart", "branch": "feat/x", "made": false }));

        let cloud = world_on(json!({}), json!({ "computer": "solari" }));
        refusal(
            area::worktree(&cloud.ctx, "cart", "feat/x", origin()).await,
            "cart is not on this computer, so no thread runs in its folder here",
            Some(Kind::Usage),
        );
        let boxed = world_on(json!({}), json!({ "computer": "pl_spoo" }));
        refusal(
            area::worktree(&boxed.ctx, "cart", "feat/x", origin()).await,
            "wsp makes no worktrees on spoo yet; start the thread without a branch, or make one inside the project folder with git worktree add and name it as the folder",
            Some(Kind::Usage),
        );
        let no_git = world_on(json!({}), json!({ "git": null }));
        refusal(
            area::worktree(&no_git.ctx, "cart", "feat/x", origin()).await,
            "cart is not a git repo, so it has no branches",
            Some(Kind::Usage),
        );
        for refused in [&cloud, &boxed, &no_git] {
            assert!(refused.base.names().iter().all(|n| n != "worktree_folder"));
        }
    })
}

// folder-threads.test.ts:808, 825, 979; workspaces.ts:780-794
#[test]
fn worktree_remove_refuses_one_wsp_did_not_make_and_a_threads_force() {
    run(async {
        let theirs = world(json!({}));
        add(&theirs, "wt1", "local", json!({ "worktree": { "path": "/wt/a", "branch": "feat/a", "made": false } }));
        script(&theirs, "worktrees_of", json!([{ "path": "/wt/a", "branch": "feat/a" }]));
        let ask = WorktreeRemoveAsk { project: "cart".into(), branch: "feat/a".into(), ..Default::default() };
        refusal(
            area::worktree_remove(&theirs.ctx, ask, origin()).await,
            "wsp made no worktree for feat/a; git worktree remove takes one wsp did not make",
            Some(Kind::Invalid),
        );
        assert_eq!(theirs.base.args_of("refuse_relayed"), vec![json!({ "id": "wt1" })]);
        assert!(theirs.base.names().iter().all(|n| n != "remove_worktree"));

        let none = world(json!({}));
        let ask = WorktreeRemoveAsk { project: "cart".into(), branch: "feat/zzz".into(), ..Default::default() };
        refusal(
            area::worktree_remove(&none.ctx, ask, origin()).await,
            "wsp made no worktree for feat/zzz; git worktree remove takes one wsp did not make",
            Some(Kind::Invalid),
        );
        assert!(none.base.names().iter().all(|n| n != "refuse_relayed" && n != "remove_worktree"));

        let forced = tree_world();
        let ask = WorktreeRemoveAsk { project: "cart".into(), branch: "feat/a".into(), force: Some(true), check: None };
        refusal(
            area::worktree_remove(&forced.ctx, ask, Some(thread("th1", "ws1", "th1"))).await,
            "only the person removes a worktree over files no commit holds; commit them, or ask them to remove it",
            Some(Kind::Usage),
        );
        assert!(forced.base.names().iter().all(|n| n != "remove_worktree"));

        let person_forces = tree_world();
        let ask = WorktreeRemoveAsk { project: "cart".into(), branch: "feat/a".into(), force: Some(true), check: None };
        area::worktree_remove(&person_forces.ctx, ask, origin()).await.expect("the person may force");
        assert_eq!(
            person_forces.base.args_of("remove_worktree"),
            vec![json!({ "id": "wt1", "force": true, "ending": false, "check": false })]
        );
        assert_eq!(
            person_forces.base.names(),
            vec!["ready", "resolve_project", "worktrees_of", "folders_of", "refuse_relayed", "remove_worktree"]
        );
        assert_eq!(person_forces.base.args_of("worktrees_of"), vec![json!({ "top": "/home/me/cart" })]);
    })
}

// workspaces.ts:780-794: no top, a check, git holding no worktree there, a worktree gone
#[test]
fn worktree_remove_finds_the_record_by_git_path_then_branch_and_passes_check() {
    run(async {
        let no_git = world_on(json!({}), json!({ "git": null }));
        let ask = WorktreeRemoveAsk { project: "cart".into(), branch: "feat/a".into(), ..Default::default() };
        refusal(
            area::worktree_remove(&no_git.ctx, ask, origin()).await,
            "cart is not a git repo, so it has no branches",
            Some(Kind::Usage),
        );

        let checked = tree_world();
        let ask = WorktreeRemoveAsk { project: "cart".into(), branch: "feat/a".into(), force: None, check: Some(true) };
        area::worktree_remove(&checked.ctx, ask, origin()).await.expect("a check passes");
        assert_eq!(checked.base.args_of("remove_worktree"), vec![json!({ "id": "wt1", "force": false, "ending": false, "check": true })]);

        let by_branch = world(json!({}));
        add(&by_branch, "wt1", "local", json!({ "worktree": { "path": "/wt/old", "branch": "feat/a", "made": true } }));
        let ask = WorktreeRemoveAsk { project: "cart".into(), branch: "feat/a".into(), ..Default::default() };
        area::worktree_remove(&by_branch.ctx, ask, origin()).await.expect("git holds none, so the branch names it");
        assert_eq!(by_branch.base.args_of("remove_worktree").len(), 1);

        let by_path = world(json!({}));
        add(&by_path, "wt_old", "local", json!({ "worktree": { "path": "/wt/other", "branch": "feat/a", "made": true } }));
        add(&by_path, "wt_new", "local", json!({ "worktree": { "path": "/wt/a", "branch": "feat/b", "made": true } }));
        script(&by_path, "worktrees_of", json!([{ "path": "/wt/a", "branch": "feat/a" }]));
        let ask = WorktreeRemoveAsk { project: "cart".into(), branch: "feat/a".into(), ..Default::default() };
        area::worktree_remove(&by_path.ctx, ask, origin()).await.expect("git's path names it");
        assert_eq!(by_path.base.args_of("remove_worktree")[0]["id"], json!("wt_new"));

        let gone = world(json!({}));
        add(&gone, "wt1", "local", json!({ "worktree": { "path": "/wt/a", "branch": "feat/a", "made": true, "gone": true } }));
        script(&gone, "worktrees_of", json!([{ "path": "/wt/a", "branch": "feat/a" }]));
        let ask = WorktreeRemoveAsk { project: "cart".into(), branch: "feat/a".into(), ..Default::default() };
        refusal(
            area::worktree_remove(&gone.ctx, ask, origin()).await,
            "wsp made no worktree for feat/a; git worktree remove takes one wsp did not make",
            Some(Kind::Invalid),
        );
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The Changes pane

// changes.test.ts:119; workspaces.ts:796-807
#[test]
fn checkout_reads_the_pr_and_tree_only_when_not_fresh() {
    run(async {
        let w = world(json!({}));
        add(&w, "kid1", "cloud", json!({ "parentWorkspaceId": "ws1" }));
        script(&w, "read_checkout:ws1", checkout_json("fix/cart"));
        let got = area::checkout(&w.ctx, "ws1", origin(), false).await.expect("checkout reads");
        settle().await;
        assert_eq!(j(&got), json!({ "checkout": checkout_json("fix/cart") }));
        assert_eq!(w.base.args_of("read_checkout"), vec![json!({ "id": "ws1", "fresh": false })]);
        assert_eq!(w.base.args_of("read_pull_request"), vec![json!({ "id": "ws1", "force": false, "whole": false })]);
        assert_eq!(w.base.args_of("read_tree"), vec![json!({ "id": "ws1" })]);

        let again = area::checkout(&w.ctx, "ws1", origin(), true).await.expect("a fresh read answers");
        settle().await;
        assert_eq!(j(&again), json!({ "checkout": checkout_json("fix/cart") }));
        assert_eq!(w.base.args_of("read_checkout")[1], json!({ "id": "ws1", "fresh": true }));
        assert_eq!(w.base.args_of("read_pull_request").len(), 1);
        assert_eq!(w.base.args_of("read_tree").len(), 1);

        let lone = world(json!({}));
        let none = area::checkout(&lone.ctx, "ws1", origin(), false).await.expect("no fact answers");
        settle().await;
        assert_eq!(j(&none), json!({}));
        assert_eq!(lone.base.args_of("read_pull_request").len(), 1);
        assert!(lone.base.args_of("read_tree").is_empty(), "a workspace with no child reads no tree");
    })
}

// changes.test.ts:212
#[test]
fn discard_check_refuses_a_path_with_no_change() {
    run(async {
        let w = world(json!({}));
        let entries = json!([{ "xy": ".M", "path": "a.ts" }, { "xy": "??", "path": "b.ts" }, { "xy": "!!", "path": "x.log" }]);
        script(&w, "ask:git.status", status_json("fix/cart", entries));
        let ask = |path: &str| DiscardAsk { workspace_id: "ws1".into(), path: path.into(), check: Some(true), thread_id: None };
        let ok = area::discard(&w.ctx, ask("a.ts"), origin()).await.expect("a changed file passes");
        assert_eq!(j(&ok), json!({ "path": "a.ts" }));
        refusal(area::discard(&w.ctx, ask("x.log"), origin()).await, "x.log has no change", None);
        refusal(area::discard(&w.ctx, ask("c.ts"), origin()).await, "c.ts has no change", None);
        let status = json!({ "op": "git.status", "cwd": CWD });
        assert_eq!(w.base.asks(), vec![status.clone(), status.clone(), status]);
        assert!(w.base.names().iter().all(|n| n != "queued" && n != "read_checkout"));

        let unread = world(json!({}));
        let mut stopped = status_json("fix/cart", json!([]));
        stopped["editsUnread"] = json!(true);
        script(&unread, "ask:git.status", stopped);
        let ok = area::discard(&unread.ctx, ask("anything.ts"), origin()).await.expect("unread edits pass any path");
        assert_eq!(j(&ok), json!({ "path": "anything.ts" }));

        let threaded = world(json!({}));
        script(&threaded, "ask:git.status", status_json("fix/cart", json!([])));
        let named = DiscardAsk { workspace_id: "ws1".into(), path: "a.ts".into(), check: Some(true), thread_id: Some("th1".into()) };
        assert!(area::discard(&threaded.ctx, named, origin()).await.is_err());
        assert_eq!(threaded.base.args_of("entry_of"), vec![json!({ "id": "ws1", "thread": "th1", "act": null, "scoped": false })]);
    })
}

// changes.test.ts:202
#[test]
fn discard_queues_and_rereads() {
    run(async {
        let w = world(json!({}));
        script(&w, "ask:git.discard", json!({ "path": "a*b.ts" }));
        let put =
            area::discard(&w.ctx, DiscardAsk { workspace_id: "ws1".into(), path: "a*b.ts".into(), check: None, thread_id: None }, origin())
                .await
                .expect("discard answers");
        assert_eq!(j(&put), json!({ "path": "a*b.ts" }));
        assert_eq!(w.base.asks(), vec![json!({ "op": "git.discard", "cwd": CWD, "path": "a*b.ts" })]);
        assert_eq!(w.base.names(), vec!["entry_of", "copy_blocked", "queued", "with_daemon", "ask", "read_checkout"]);
        assert_eq!(w.base.args_of("queued"), vec![json!({ "id": "ws1" })]);
        assert_eq!(w.base.args_of("read_checkout"), vec![json!({ "id": "ws1", "fresh": true })]);
    })
}

// changes.test.ts:232, 242
#[test]
fn commit_names_every_changed_file_and_refuses_a_clean_checkout() {
    run(async {
        let w = world(json!({}));
        script(&w, "ask:git.diff", diff_reply(json!([a_ts()])));
        script(&w, "ask:git.commit", commit_reply());
        let ask = CommitAsk { workspace_id: "ws1".into(), message: "m".into(), paths: None, thread_id: None };
        let made = area::commit(&w.ctx, ask, origin()).await.expect("commit answers");
        assert_eq!(j(&made), commit_reply());
        assert_eq!(
            w.base.asks(),
            vec![
                json!({ "op": "git.diff", "cwd": CWD, "scope": "head" }),
                json!({ "op": "git.commit", "cwd": CWD, "message": "m", "paths": ["a.ts"] })
            ]
        );
        assert_eq!(w.base.names(), vec!["spawn_guard", "entry_of", "copy_blocked", "queued", "with_daemon", "ask", "ask", "read_checkout"]);
        assert_eq!(w.base.args_of("spawn_guard"), vec![json!({ "act": "commit", "scoped": false })]);
        assert_eq!(w.base.args_of("entry_of"), vec![json!({ "id": "ws1", "thread": null, "act": "commit", "scoped": false })]);
        assert_eq!(w.base.args_of("read_checkout"), vec![json!({ "id": "ws1", "fresh": true })]);

        let clean = world(json!({}));
        script(&clean, "ask:git.diff", diff_reply(json!([])));
        let ask = CommitAsk { workspace_id: "ws1".into(), message: "m".into(), paths: None, thread_id: None };
        refusal(area::commit(&clean.ctx, ask, origin()).await, "cart: nothing to commit, the checkout has no changes", None);
        assert_eq!(clean.base.asks(), vec![json!({ "op": "git.diff", "cwd": CWD, "scope": "head" })]);
        assert!(clean.base.names().iter().all(|n| n != "read_checkout"));

        let by_thread = world(json!({}));
        script(&by_thread, "ask:git.diff", diff_reply(json!([])));
        let ask = CommitAsk { workspace_id: "ws1".into(), message: "m".into(), paths: None, thread_id: Some(THREAD.into()) };
        refusal(area::commit(&by_thread.ctx, ask, origin()).await, "thread 5c2d0e14: nothing to commit, the checkout has no changes", None);
        assert_eq!(by_thread.base.args_of("entry_of"), vec![json!({ "id": "ws1", "thread": THREAD, "act": "commit", "scoped": false })]);
    })
}

// changes.test.ts:221
#[test]
fn commit_the_files_named_with_the_message_then_reads_the_checkout_again() {
    run(async {
        let w = world(json!({}));
        script(&w, "ask:git.commit", commit_reply());
        let message = "Round the cart total once\n\nWhy.";
        let ask = CommitAsk {
            workspace_id: "ws1".into(),
            message: message.into(),
            paths: Some(vec!["a.ts".into(), "b.ts".into()]),
            thread_id: None,
        };
        let made = area::commit(&w.ctx, ask, origin()).await.expect("commit answers");
        assert_eq!(j(&made), commit_reply());
        assert_eq!(w.base.asks(), vec![json!({ "op": "git.commit", "cwd": CWD, "message": message, "paths": ["a.ts", "b.ts"] })]);
        assert_eq!(w.base.names_of(&["queued", "ask", "read_checkout"]), vec!["queued", "ask", "read_checkout"]);
    })
}

// workspaces.ts:841-843; changes.test.ts:316
#[test]
fn commit_draft_with_no_paths_asks_nothing() {
    run(async {
        let w = world(json!({}));
        let ask = CommitDraftAsk { workspace_id: "ws1".into(), paths: Some(vec![]), thread_id: None };
        let got = area::commit_draft(&w.ctx, ask, origin()).await.expect("nothing ticked answers");
        assert_eq!(j(&got), json!({ "message": null, "note": NOTHING_TICKED }));
        assert_eq!(w.base.names(), vec!["spawn_guard"]);
        assert_eq!(w.base.args_of("spawn_guard"), vec![json!({ "act": "commit", "scoped": false })]);
        assert!(w.base.asks().is_empty());
    })
}

// changes.test.ts:253, 272, 286; workspaces.ts:841-879
#[test]
fn commit_draft_drafts_from_the_whole_opening_and_removes_the_file() {
    run(async {
        let (head, opening) = opening_of(1800);
        assert!(opening.starts_with(&head) && opening.len() > head.len());
        let w = world(json!({}));
        {
            let mut rows = w.base.sessions.borrow_mut();
            rows.push(row("s2", "ws1", "th1", "codex", "completed", 20, Some("a later message")));
            rows.push(row("s1", "ws1", "th1", "claude", "completed", 10, Some(&head)));
            rows.push(row("s3", "ws1", "th2", "claude", "completed", 5, Some("write the docs")));
            rows.push(row("s4", "ws2", "th1", "claude", "completed", 1, Some("another workspace")));
        }
        script(
            &w,
            "open_thread",
            json!([{ "type": "turn.done" }, { "type": "session.start", "prompt": "a different opening" }, { "type": "session.start", "prompt": opening }]),
        );
        script(
            &w,
            "ask:git.diff",
            diff_reply(
                json!([a_ts(), { "path": "b.ts", "kind": "modified", "additions": 1, "deletions": 0, "patch": "diff --git a/b.ts b/b.ts\n+two\n" }]),
            ),
        );
        let ask = CommitDraftAsk { workspace_id: "ws1".into(), paths: Some(vec!["a.ts".into()]), thread_id: Some("th1".into()) };
        let got = area::commit_draft(&w.ctx, ask, origin()).await.expect("a draft answers");
        assert_eq!(j(&got), json!({ "message": DRAFT }));

        let path = "/tmp/wsp-draft-111111111111.txt";
        assert_eq!(w.base.asks(), vec![json!({ "op": "git.diff", "cwd": CWD, "scope": "head", "paths": ["a.ts"] })]);
        assert_eq!(w.base.args_of("open_thread"), vec![json!({ "workspaceId": "ws1", "threadId": "th1" })]);
        let diff = "diff --git a/a.ts b/a.ts\n+one\n\ndiff --git a/b.ts b/b.ts\n+two\n";
        assert_eq!(w.base.args_of("put_files"), vec![json!({ "id": "ws1", "path": path, "text": draft_question(diff, Some(&opening)) })]);
        assert_eq!(w.base.args_of("draft_for"), vec![json!({ "id": "ws1", "agent": "codex", "promptFile": path, "model": "small-model" })]);
        assert_eq!(w.base.args_of("drafter"), vec![json!({ "id": "ws1", "agent": "codex" })]);
        assert_eq!(
            names_after(
                &w,
                &[
                    "spawn_guard",
                    "entry_of",
                    "copy_blocked",
                    "open_thread",
                    "with_daemon",
                    "queued",
                    "put_files",
                    "draft_for",
                    "machine_exec"
                ]
            ),
            vec!["spawn_guard", "entry_of", "copy_blocked", "open_thread", "with_daemon", "put_files", "draft_for", "machine_exec"]
        );
        assert_eq!(w.base.args_of("entry_of"), vec![json!({ "id": "ws1", "thread": null, "act": "commit", "scoped": false })]);
        assert_eq!(w.base.args_of("machine_exec"), vec![json!({ "id": "ws1", "cmd": format!("rm -f '{path}'"), "timeoutMs": null })]);
    })
}

// changes.test.ts:253, 301: a head that is not cut is the whole opening, and no thread named drafts from the diff alone
#[test]
fn commit_draft_reads_the_transcript_only_for_a_cut_head_and_none_named_drafts_from_the_diff() {
    run(async {
        let w = world(json!({}));
        w.base.sessions.borrow_mut().push(row("s1", "ws1", "th1", "claude", "completed", 10, Some("Fix the flaky cart test")));
        script(&w, "ask:git.diff", diff_reply(json!([a_ts()])));
        let ask = CommitDraftAsk { workspace_id: "ws1".into(), paths: Some(vec!["a.ts".into()]), thread_id: Some("th1".into()) };
        area::commit_draft(&w.ctx, ask, origin()).await.expect("a draft answers");
        assert!(w.base.args_of("open_thread").is_empty());
        assert_eq!(
            w.base.args_of("put_files")[0]["text"],
            json!(draft_question("diff --git a/a.ts b/a.ts\n+one\n", Some("Fix the flaky cart test")))
        );
        assert_eq!(w.base.args_of("draft_for")[0]["agent"], json!("claude"));

        let alone = world(json!({}));
        alone.base.sessions.borrow_mut().push(row("s1", "ws1", "th1", "codex", "completed", 10, Some("hello from thread B")));
        script(&alone, "default_agent_of", json!("claude"));
        script(&alone, "ask:git.diff", diff_reply(json!([a_ts()])));
        let ask = CommitDraftAsk { workspace_id: "ws1".into(), paths: Some(vec!["a.ts".into()]), thread_id: None };
        let got = area::commit_draft(&alone.ctx, ask, origin()).await.expect("a draft answers");
        assert_eq!(j(&got), json!({ "message": DRAFT }));
        assert_eq!(alone.base.args_of("put_files")[0]["text"], json!(draft_question("diff --git a/a.ts b/a.ts\n+one\n", None)));
        assert_eq!(alone.base.args_of("draft_for")[0]["agent"], json!("claude"));
        assert!(alone.base.args_of("open_thread").is_empty());
    })
}

// changes.test.ts:286
#[test]
fn commit_draft_refuses_a_thread_that_does_not_work_in_this_folder() {
    run(async {
        let w = world(json!({}));
        w.base.sessions.borrow_mut().push(row("s1", "ws1", "th1", "claude", "completed", 10, Some("x")));
        let ask = CommitDraftAsk { workspace_id: "ws1".into(), paths: Some(vec!["a.ts".into()]), thread_id: Some("t_elsewhere".into()) };
        refusal(area::commit_draft(&w.ctx, ask, origin()).await, "thread t_elsewh does not work in this folder", Some(Kind::NotFound));
        assert_eq!(w.base.names(), vec!["spawn_guard", "entry_of", "copy_blocked"]);
        assert!(w.base.asks().is_empty());
    })
}

// changes.test.ts:316; workspaces.ts:854-871
#[test]
fn commit_draft_says_no_agent_and_no_answer() {
    run(async {
        let ask = || CommitDraftAsk { workspace_id: "ws1".into(), paths: Some(vec!["a.ts".into()]), thread_id: None };

        let off = world(json!({}));
        script(&off, "agent_off", json!(true));
        let got = area::commit_draft(&off.ctx, ask(), origin()).await.expect("answers");
        assert_eq!(j(&got), json!({ "message": null, "note": NO_AGENT }));
        assert!(off.base.asks().is_empty() && off.base.args_of("put_files").is_empty());

        let missing = world(json!({}));
        script(&missing, "has_adapter", json!(false));
        let got = area::commit_draft(&missing.ctx, ask(), origin()).await.expect("answers");
        assert_eq!(j(&got), json!({ "message": null, "note": NO_AGENT }));
        assert!(missing.base.asks().is_empty());

        let mute = world(json!({}));
        script(&mute, "drafter", json!(null));
        let got = area::commit_draft(&mute.ctx, ask(), origin()).await.expect("answers");
        assert_eq!(j(&got), json!({ "message": null, "note": NO_AGENT }));
        assert!(mute.base.args_of("put_files").is_empty());

        let unwritten = world(json!({}));
        script(&unwritten, "ask:git.diff", diff_reply(json!([a_ts()])));
        script(&unwritten, "put_files", json!({ "exitCode": 1, "stdout": "", "stderr": "disk full" }));
        let got = area::commit_draft(&unwritten.ctx, ask(), origin()).await.expect("answers");
        assert_eq!(j(&got), json!({ "message": null, "note": NO_ANSWER }));
        assert!(unwritten.base.args_of("draft_for").is_empty());

        let silent = world(json!({}));
        script(&silent, "ask:git.diff", diff_reply(json!([a_ts()])));
        script(&silent, "draft_for", json!(null));
        let got = area::commit_draft(&silent.ctx, ask(), origin()).await.expect("answers");
        assert_eq!(j(&got), json!({ "message": null, "note": NO_ANSWER }));
        assert_eq!(silent.base.args_of("machine_exec").len(), 1, "the question's file goes whatever the answer was");

        let blank = world(json!({}));
        script(&blank, "ask:git.diff", diff_reply(json!([a_ts()])));
        script(&blank, "draft_for", json!("  \n  "));
        let got = area::commit_draft(&blank.ctx, ask(), origin()).await.expect("answers");
        assert_eq!(j(&got), json!({ "message": null, "note": NO_ANSWER }));

        let fenced = world(json!({}));
        script(&fenced, "ask:git.diff", diff_reply(json!([a_ts()])));
        script(&fenced, "draft_for", json!("```\nRound the cart total once\n\nThe total rounded per line.\n```"));
        let got = area::commit_draft(&fenced.ctx, ask(), origin()).await.expect("answers");
        assert_eq!(j(&got), json!({ "message": DRAFT }));
    })
}

// changes.test.ts:325; workspaces.ts:881-891
#[test]
fn viewed_sets_and_clears_a_mark_and_emits() {
    run(async {
        let w = world(json!({}));
        let mark = |path: &str, blob: Option<&str>| Some(ViewedMark { path: path.into(), blob: blob.map(str::to_owned) });
        let one = area::viewed(&w.ctx, "ws1", mark("a.ts", Some("b1")), origin()).await.expect("a mark sets");
        assert_eq!(j(&one), json!({ "viewed": { "a.ts": "b1" } }));
        let two = area::viewed(&w.ctx, "ws1", mark("b.ts", Some("b2")), origin()).await.expect("a mark sets");
        assert_eq!(j(&two), json!({ "viewed": { "a.ts": "b1", "b.ts": "b2" } }));
        let off = area::viewed(&w.ctx, "ws1", mark("b.ts", None), origin()).await.expect("a mark clears");
        assert_eq!(j(&off), json!({ "viewed": { "a.ts": "b1" } }));
        assert_eq!(j(w.fake.viewed.borrow().get("ws1").unwrap()), json!({ "viewed": { "a.ts": "b1" } }));
        assert_eq!(
            w.base.events.borrow().clone(),
            vec![
                json!({ "type": "workspace.viewed", "workspaceId": "ws1", "viewed": { "a.ts": "b1" } }),
                json!({ "type": "workspace.viewed", "workspaceId": "ws1", "viewed": { "a.ts": "b1", "b.ts": "b2" } }),
                json!({ "type": "workspace.viewed", "workspaceId": "ws1", "viewed": { "a.ts": "b1" } }),
            ]
        );
        assert_eq!(w.base.args_of("persist_sessions"), vec![json!({ "id": "ws1" }); 3]);

        let read = area::viewed(&w.ctx, "ws1", None, origin()).await.expect("a read answers");
        assert_eq!(j(&read), json!({ "viewed": { "a.ts": "b1" } }));
        assert_eq!(w.base.events.borrow().len(), 3, "a read says nothing");
        assert_eq!(w.base.args_of("persist_sessions").len(), 3);
        assert_eq!(w.base.args_of("entry_of").len(), 4);
    })
}
