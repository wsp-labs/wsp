// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/pull-request.test.ts at 65f6b89c7, case for case, second half: settling on merge and the acts on
//! a pull request. The first half is `whole_pull_request_1.rs`; the workspace and the daemons are
//! `common/pull_request.rs`. Each test names the TS test above it.
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::pull_request::*;
use common::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::{FixAsk, FolderForAsk, MergeAsk, PullRequestReplyAsk, StartFromLinkAsk, StartOptions};
use wsp_core::types::events::{CHECKOUT_TTL_MS, PR_PAGE_HOLD_MS, RATE_LIMIT_HOLD_MS};
use wsp_core::types::wiring::SessionHandle;
use wsp_wire::types::daemon_contract::GIT_DIFF_CAP_BYTES;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::pull_request::{PullRequestItem, PullRequestPage, PR_POLL_IDLE_MS, REPLY_EMPTY_LINE};
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::HostError;
use wsp_words::pull_request::{
    check_not_failed_refusal, child_pushed_line, merge_method_refusal, no_pull_request_refusal, no_such_item_refusal,
    pull_request_post_line, pull_request_send_prompt, PageItem, PageItemKind,
};
use wsp_words::words::computer::{spawn_act_refusal, thread_word, SpawnAct, MACHINE_WSP_FORKS};
use wsp_words::words::thread::thread_elsewhere_line;

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn as_thread(ws: &str, thread: &str, root: &str) -> Caller {
    let by =
        serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": ws, "rootThreadId": root })).expect("a scope");
    Caller::Scoped { origin: WorkspaceOrigin::Relayed, by }
}

fn swappable(first: Answer) -> (Rc<RefCell<Answer>>, Answer) {
    let cell = Rc::new(RefCell::new(first));
    let read = cell.clone();
    (cell, answer(move |f| (read.borrow())(f)))
}

fn clocked() -> FakeClock {
    FakeClock::at_now()
}

async fn checkout(h: &Held2) {
    h.rt().workspaces().checkout(&h.id, None, None).await.expect("a checkout");
}

async fn refused<T>(r: impl std::future::Future<Output = Result<T, HostError>>) -> String {
    match r.await {
        Ok(_) => panic!("the call was not refused"),
        Err(e) => e.message,
    }
}

async fn rows(h: &Held2) -> Vec<Value> {
    h.rt().sessions().list(Some(h.id.clone()), None).await.expect("a list").iter().map(j).collect()
}

fn reads(d: &Daemons) -> usize {
    d.count(Via::Here, "git.prRead")
}

fn page() -> Value {
    json!({
        "title": "t", "body": "b", "author": "octocat", "createdAt": "2026-09-30T10:00:00Z", "updatedAt": "2026-09-30T12:00:00Z",
        "labels": [], "reviewRequests": [], "latestReviews": [], "assignees": [],
        "commits": [{ "oid": HEAD, "subject": "Set .ci-status to 1", "body": "", "at": "2026-09-30T11:00:00Z", "author": "octocat" }],
        "reviews": [{ "id": 5324159317u64, "author": "ana", "state": "changes_requested", "body": "Two things before this lands.", "at": "t" }],
        "comments": [{ "id": 5841958969u64, "author": "vercel[bot]", "association": "none", "bot": true, "body": "Deployment failed for project wsp-www.", "url": "u", "at": "t" }],
        "reviewComments": [{
            "id": 4109844888u64, "path": "packages/engine/src/golden.ts", "line": 111, "side": "RIGHT", "author": "Zingzy", "association": "owner",
            "bot": false, "body": "Read it twice.", "url": "u", "at": "t", "hunk": "@@ -1 +1 @@\n-a\n+b", "resolved": false
        }],
        "files": []
    })
}

fn item(kind: &str, id: u64) -> Value {
    json!({ "kind": kind, "id": id })
}

fn items_of(docs: &[Value]) -> Vec<PullRequestItem> {
    docs.iter().map(|d| serde_json::from_value(d.clone()).expect("an item")).collect()
}

/// `pullRequestSendPrompt(PAGE, items, 12)`.
fn send_prompt(docs: &[Value]) -> String {
    let mut value = page();
    value["sent"] = json!([]);
    value["postsAsYou"] = json!(true);
    let page: PullRequestPage = serde_json::from_value(value).expect("a page");
    let wanted: Vec<PageItem> = docs
        .iter()
        .map(|d| PageItem {
            kind: match d["kind"].as_str() {
                Some("comment") => PageItemKind::Comment,
                Some("review") => PageItemKind::Review,
                _ => PageItemKind::ReviewComment,
            },
            id: d["id"].as_f64().unwrap_or_default(),
        })
        .collect();
    pull_request_send_prompt(&page.comments, &page.reviews, &page.review_comments, &wanted, 12.0).expect("a prompt")
}

fn sent_at(docs: &[Value], at: i64) -> Value {
    Value::Array(docs.iter().map(|d| json!({ "kind": d["kind"], "id": d["id"], "at": at })).collect())
}

// "settling on merge" > "stamps every thread of a quiet tree settled and naps its machine, and restore brings it back with nothing read again"
#[test]
fn stamps_every_thread_of_a_quiet_tree_settled_and_naps_its_machine_and_restore_brings_it_back_with_nothing_read_again() {
    drive(|| async {
        let (cell, read) = swappable(answer(|_| ok(json!({ "pr": open(json!({})) }))));
        let d = fake_daemons(vec![("git.prRead", read)], vec![]);
        let agent = HeldAgent::new();
        let fc = clocked();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], clock: Some(fc.clone()), ..With::default() }).await;
        let lead = h.rt().sessions().start(&h.id, start("fix the check"), None).await.expect("a start");
        let thread = lead.view().thread_id.expect("a thread");
        agent.end(0);
        lead.finished().await;
        until(|| async { h.pr_field("state") == Some(json!("open")) }).await;
        assert!(rows(&h).await.iter().all(|r| r.get("settledAt").is_none()));
        // Merged on GitHub: the next read, past the interval an open one's read stands for, sees it.
        *cell.borrow_mut() = answer(|_| ok(json!({ "pr": open(json!({ "state": "merged" })) })));
        fc.advance(PR_POLL_IDLE_MS as i64);
        checkout(&h).await;
        until(|| async { rows(&h).await.iter().all(|r| r.get("settledAt").is_some()) }).await;
        until(|| async {
            let all = h.rt().workspaces().list(None).await.expect("a list");
            all.iter().find(|w| w.id == h.id).is_some_and(|w| j(w)["phase"] == "napping")
        })
        .await;
        let before = reads(&d);
        h.rt().sessions().restore(vec![thread.clone()], None).await.expect("a restore");
        assert!(rows(&h).await.iter().all(|r| r.get("settledAt").is_none()));
        fc.advance(CHECKOUT_TTL_MS as i64 + 1);
        checkout(&h).await;
        sleep_ms(20).await;
        assert_eq!(reads(&d), before);
        assert!(rows(&h).await.iter().all(|r| r.get("settledAt").is_none()));
    });
}

// "in a project folder keeps the pull request with its branch: it settles only the threads that ran there, and goes when the folder leaves it"
#[test]
fn in_a_project_folder_keeps_the_pull_request_with_its_branch_it_settles_only_the_threads_that_ran_there_and_goes_when_the_folder_leaves_it(
) {
    drive(|| async {
        let head = Rc::new(RefCell::new("main".to_string()));
        let seen = head.clone();
        let d = fake_daemons(
            vec![
                (
                    "git.status",
                    answer(move |_| {
                        let mut s = status();
                        s["branch"] = json!({ "oid": "abc", "head": seen.borrow().clone(), "ahead": 1, "behind": 0 });
                        s
                    }),
                ),
                (
                    "git.prRead",
                    answer(|f| {
                        if f.get("branch") == Some(&json!("fix/ci")) || f.get("number") == Some(&json!(12)) {
                            ok(json!({ "pr": open(json!({ "state": "merged" })) }))
                        } else {
                            ok(json!({}))
                        }
                    }),
                ),
            ],
            vec![],
        );
        let dir = tempfile::tempdir().expect("a folder");
        let root = std::fs::canonicalize(dir.path()).expect("a real path");
        let state = sqlite_state();
        let agent = HeldAgent::new();
        let (channel, local) = (d.open(), local_on(&root, true));
        let fx = runtime(&StubBackend::new(None))
            .store(&state.store)
            .adapter("claude", agent.factory())
            .with(move |o| {
                o.daemon_token = Some(DAEMON_TOKEN.into());
                o.daemon_channel = Some(channel);
                o.local = Some(local);
            })
            .build();
        let rt = &*fx.rt;
        let repo = temp_repo();
        let _ = std::process::Command::new("git")
            .args(["-C", &repo.display().to_string(), "remote", "add", "origin", "https://github.com/wsp/pr-lab.git"])
            .output();
        let project = project_on_with(rt, Some(HERE_PLACE_ID), Some(&repo.display().to_string()), "pr-lab", Some("main")).await;
        let id = rt
            .workspaces()
            .folder_for(FolderForAsk { project: Some(project), ..Default::default() }, None)
            .await
            .expect("a folder")
            .workspace
            .id;
        let rows_of = |thread: String| {
            let (id, store) = (id.clone(), ());
            let _ = store;
            async move {
                rt.sessions()
                    .list(Some(id), None)
                    .await
                    .expect("a list")
                    .iter()
                    .map(j)
                    .filter(|r| r["threadId"] == thread.as_str())
                    .collect::<Vec<_>>()
            }
        };
        let settled = |thread: String| {
            let rows = rows_of(thread);
            async move { rows.await.iter().all(|r| r.get("settledAt").is_some()) }
        };
        // C ran on main before the folder went to the branch.
        let c = rt.sessions().start(&id, start("thread C on main"), None).await.expect("a start");
        agent.end(0);
        c.finished().await;
        // Its turn's end reads the checkout before the folder moves on.
        let ended = rows_of(c.view().thread_id.expect("a thread")).await[0]["endedAt"].as_f64().expect("an end");
        until(|| async {
            let doc = state.store.get("workspaces", &id).await.expect("a read");
            doc.and_then(|d| d["checkout"]["readAt"].as_f64()).unwrap_or(0.0) >= ended
        })
        .await;
        *head.borrow_mut() = "fix/ci".into();
        let a = rt.sessions().start(&id, start("thread A on the branch"), None).await.expect("a start");
        agent.end(1);
        a.finished().await;
        let kept = || async { state.store.get("workspaces", &id).await.expect("a read").and_then(|d| d.get("pr").cloned()) };
        until(|| async { kept().await.is_some_and(|pr| pr["state"] == "merged") }).await;
        let a_thread = a.view().thread_id.expect("a thread");
        until(|| settled(a_thread.clone())).await;
        assert!(!settled(c.view().thread_id.expect("a thread")).await);
        // The folder keeps the threads of the branch alone, none of its base's.
        let ran_on = || async { state.store.get("workspaces", &id).await.expect("a read").and_then(|d| d.get("branchThreads").cloned()) };
        assert_eq!(ran_on().await, Some(json!({ "branch": "fix/ci", "threads": [a_thread] })));
        // Back on main: the merged pull request is not the folder's any more, and a turn ending there settles nobody.
        *head.borrow_mut() = "main".into();
        let b = rt.sessions().start(&id, start("thread B on main"), None).await.expect("a start");
        agent.end(2);
        b.finished().await;
        until(|| async { kept().await.is_none() }).await;
        sleep_ms(20).await;
        assert!(!settled(b.view().thread_id.expect("a thread")).await);
        assert!(!settled(c.view().thread_id.expect("a thread")).await);
        assert!(ran_on().await.is_none());
    });
}

/// `projectOn(rt, computer, source, { name, base })`.
async fn project_on_with(
    rt: &wsp_serve::AssembledRuntime,
    on: Option<&str>,
    source: Option<&str>,
    name: &str,
    base: Option<&str>,
) -> String {
    let ask = wsp_core::types::api::ProjectAddAsk {
        source: source.unwrap_or("https://github.com/wsp/stub-9.git").into(),
        on: on.map(str::to_string),
        name: Some(name.into()),
        base: base.map(str::to_string),
        ..Default::default()
    };
    rt.projects().add(ask, None).await.expect("a project").project.id
}

// "keeps a fork's pull request on the worktree a start made on <owner>/<branch>, which gh finds by number alone"
#[test]
fn keeps_a_forks_pull_request_on_the_worktree_a_start_made_on_owner_branch_which_gh_finds_by_number_alone() {
    drive(|| async {
        let fork = open(json!({ "branch": "feat", "fork": { "owner": "ana", "pushable": true } }));
        let (fork_read, fork_issue) = (fork.clone(), fork.clone());
        let d = fake_daemons(
            vec![
                (
                    "git.status",
                    answer(|_| {
                        let mut s = status();
                        s["branch"] = json!({ "oid": "abc", "head": "ana/feat", "ahead": 0, "behind": 0 });
                        s
                    }),
                ),
                (
                    "git.prRead",
                    answer(
                        move |f| if f.get("number") == Some(&json!(12)) { ok(json!({ "pr": fork_read.clone() })) } else { ok(json!({})) },
                    ),
                ),
                (
                    "git.issueRead",
                    answer(move |_| {
                        ok(
                            json!({ "issue": { "number": 12, "url": fork_issue["url"], "title": "Set ci", "body": "b", "state": "OPEN", "comments": [] } }),
                        )
                    }),
                ),
            ],
            vec![],
        );
        let dir = tempfile::tempdir().expect("a folder");
        let root = std::fs::canonicalize(dir.path()).expect("a real path");
        std::fs::create_dir(root.join("state")).expect("a folder");
        let state = sqlite_state();
        let agent = HeldAgent::new();
        let (channel, mut local) = (d.open(), local_on(&root, true));
        let (copier, _log) = common::local::git_copier();
        local.copier = Some(copier);
        let state_path = root.join("state/state.json").display().to_string();
        let fx = runtime(&StubBackend::new(None))
            .store(&state.store)
            .adapter("claude", agent.factory())
            .with(move |o| {
                o.daemon_token = Some(DAEMON_TOKEN.into());
                o.daemon_channel = Some(channel);
                o.local = Some(local);
                o.state_path = Some(state_path);
            })
            .build();
        let rt = &*fx.rt;
        let repo = temp_repo();
        let r = repo.display().to_string();
        let _ = std::process::Command::new("git").args(["-C", &r, "remote", "add", "origin", "https://github.com/wsp/pr-lab.git"]).output();
        let _ = std::process::Command::new("git")
            .args(["-C", &r, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "first"])
            .output();
        let project = project_on_with(rt, Some(HERE_PLACE_ID), Some(&r), "pr-lab", Some("main")).await;
        // A worktree already holds the fork's branch, so the start takes it without a fetch.
        let workspace = rt
            .workspaces()
            .folder_for(FolderForAsk { project: Some(project), branch: Some("ana/feat".into()), ..Default::default() }, None)
            .await
            .expect("a folder")
            .workspace;
        let started = rt
            .workspaces()
            .start(StartFromLinkAsk { url: fork["url"].as_str().expect("a url").into(), ..Default::default() }, None)
            .await
            .expect("a start");
        let kept = || async { state.store.get("workspaces", &workspace.id).await.expect("a read").and_then(|d| d.get("pr").cloned()) };
        agent.end(0);
        until(|| async { kept().await.is_some_and(|pr| pr["branch"] == "feat") }).await;
        let thread = j(&started)["threadId"].as_str().expect("a thread").to_string();
        let second =
            rt.sessions().start(&workspace.id, StartOptions { thread: Some(thread), ..start("again") }, None).await.expect("a start");
        agent.end(1);
        second.finished().await;
        until(|| async { d.of("git.prRead").len() >= 3 }).await;
        sleep_ms(20).await;
        assert_eq!(kept().await.expect("a pull request")["number"], 12);
        assert!(!d.of("git.prRead").iter().any(|(_, f)| f.get("branch").is_some()));
    });
}

// "leaves a merged tree live while a child works, and stamps it when the child's turn ends"
#[test]
fn leaves_a_merged_tree_live_while_a_child_works_and_stamps_it_when_the_childs_turn_ends() {
    drive(|| async {
        let d = fake_daemons(vec![("git.prRead", answer(|_| ok(json!({ "pr": open(json!({ "state": "merged" })) }))))], vec![]);
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], ..With::default() }).await;
        let lead = h.rt().sessions().start(&h.id, start("lead"), None).await.expect("a start");
        let root = lead.view().thread_id.expect("a thread");
        let child = h.rt().sessions().start(&h.id, start("child"), Some(as_thread(&h.id, &root, &root))).await.expect("a start");
        agent.end(0);
        lead.finished().await;
        checkout(&h).await;
        until(|| async { h.pr_field("state") == Some(json!("merged")) }).await;
        sleep_ms(20).await;
        assert!(!rows(&h).await.iter().any(|r| r.get("settledAt").is_some()));
        assert_eq!(j(&child.view())["rootThreadId"], root.as_str());
        agent.end(1);
        child.finished().await;
        until(|| async { rows(&h).await.iter().all(|r| r.get("settledAt").is_some()) }).await;
    });
}

// "settles nothing off a child workspace's own pull request, and a child's bring back pushes and opens none"
#[test]
fn settles_nothing_off_a_child_workspaces_own_pull_request_and_a_childs_bring_back_pushes_and_opens_none() {
    drive(|| async {
        let d = fake_daemons(
            vec![("git.prRead", answer(|_| ok(json!({ "pr": open(json!({ "state": "merged" })) }))))],
            vec![(
                "git.push",
                answer(|_| ok(json!({ "branch": "fix/ci", "base": "main", "remote": "origin", "ahead": 1, "uncommitted": 0, "stat": [] }))),
            )],
        );
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], ..With::default() }).await;
        let lead = h.rt().sessions().start(&h.id, start("lead"), None).await.expect("a start");
        let root = lead.view().thread_id.expect("a thread");
        let scope = as_thread(&h.id, &root, &root);
        let child = create_on(h.rt(), CreateOn::named("helper"), Some(scope.clone())).await.expect("a child").view.id;
        h.backend.machine(1).set_preview_url(COPY);
        agent.end(0);
        lead.finished().await;
        let brought = h.rt().workspaces().bring_back(&child, None, None, Some(scope)).await.expect("a bring back");
        let brought = j(&brought);
        assert_matches(&brought, json!({ "branch": "fix/ci", "note": child_pushed_line("fix/ci") }));
        assert!(brought.get("pr").is_none_or(Value::is_null));
        assert!(!d.ops(None).contains(&"git.pr".to_string()));
        // A root's bring back asks for both halves.
        let _ = h.rt().workspaces().bring_back(&h.id, None, None, None).await;
        assert!(d.ops(Some(Via::Copy)).contains(&"git.pr".to_string()));
    });
}

fn view_answer(extra: Value) -> Answer {
    answer(move |_| {
        let mut v = page();
        if let (Some(v), Some(extra)) = (v.as_object_mut(), extra.as_object()) {
            v.extend(extra.clone());
        }
        ok(v)
    })
}

fn no_repo() -> Answer {
    answer(|_| json!({ "id": 1, "ok": false, "error": "no" }))
}

// "the acts on a pull request" > "reads the page over this computer's daemon and holds it a minute, read again past it, on a refresh and after a write"
#[test]
fn reads_the_page_over_this_computers_daemon_and_holds_it_a_minute_read_again_past_it_on_a_refresh_and_after_a_write() {
    drive(|| async {
        let merge = json!({ "methods": ["squash", "merge"], "defaultMethod": "squash", "autoMerge": true });
        let repo = merge.clone();
        let d = fake_daemons(
            vec![
                ("git.prView", view_answer(json!({}))),
                ("git.repoRead", answer(move |_| ok(repo.clone()))),
                ("git.prReact", answer(|f| ok(json!({ "subject": f["subject"].as_str().unwrap_or_default(), "reactions": [] })))),
            ],
            vec![],
        );
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let pages = || d.of("git.prView");
        let want = {
            let mut v = page();
            v["merge"] = merge.clone();
            v["sent"] = json!([]);
            v["postsAsYou"] = json!(true);
            v
        };
        // The pane offers the methods the repository allows, read once an hour on this computer, and nothing is sent yet.
        let first = j(&h.rt().workspaces().pull_request_view(&h.id, None, None).await.expect("a page"));
        assert_eq!(first, want);
        assert_eq!(pages().len(), 1);
        assert_eq!(
            Value::Object(pages()[0].1.clone()),
            json!({ "op": "git.prView", "cwd": h.root.display().to_string(), "remote": h.remote, "number": 12 })
        );
        // A reopen within the minute, or the dev window's second mount, reads nothing.
        fc.advance(PR_PAGE_HOLD_MS as i64 - 1);
        assert_eq!(j(&h.rt().workspaces().pull_request_view(&h.id, None, None).await.expect("a page")), want);
        assert_eq!(pages().len(), 1);
        fc.advance(1);
        h.rt().workspaces().pull_request_view(&h.id, None, None).await.expect("a page");
        assert_eq!(pages().len(), 2);
        h.rt().workspaces().pull_request_view(&h.id, Some(true), None).await.expect("a page");
        assert_eq!(pages().len(), 3);
        let eyes = serde_json::from_value(json!("eyes")).expect("a reaction");
        h.rt().workspaces().pull_request_react(&h.id, "IC_kwDOx", eyes, true, None).await.expect("a reaction");
        h.rt().workspaces().pull_request_view(&h.id, None, None).await.expect("a page");
        assert_eq!(pages().len(), 4);
        assert_eq!(d.count(Via::Here, "git.repoRead"), 1);
    });
}

// "reads the diff over this computer's daemon at the cap a git.diff has, never kept"
#[test]
fn reads_the_diff_over_this_computers_daemon_at_the_cap_a_git_diff_has_never_kept() {
    drive(|| async {
        let diff = json!({ "diff": "diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-one\n+two\n", "truncated": false, "left": [] });
        let reply = diff.clone();
        let d = fake_daemons(vec![("git.prDiff", answer(move |_| ok(reply.clone())))], vec![]);
        let h = with_workspace(&d, With::default()).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        assert_eq!(j(&h.rt().workspaces().pull_request_diff(&h.id, None).await.expect("a diff")), diff);
        let asked = d.of("git.prDiff");
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].0, Via::Here);
        assert_eq!(
            Value::Object(asked[0].1.clone()),
            json!({ "op": "git.prDiff", "cwd": h.root.display().to_string(), "remote": h.remote, "number": 12, "maxBytes": GIT_DIFF_CAP_BYTES })
        );
    });
}

fn page_daemons() -> Daemons {
    fake_daemons(vec![("git.prView", view_answer(json!({}))), ("git.repoRead", no_repo())], vec![])
}

// "sends the items named into the workspace's thread as one message, keeps them sent with when, and the page answers them"
#[test]
fn sends_the_items_named_into_the_workspaces_thread_as_one_message_keeps_them_sent_with_when_and_the_page_answers_them() {
    drive(|| async {
        let fc = clocked();
        let d = page_daemons();
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], clock: Some(fc.clone()), ..With::default() }).await;
        let lead = h.rt().sessions().start(&h.id, start("set .ci-status to 1"), None).await.expect("a start");
        let thread = lead.view().thread_id.expect("a thread");
        agent.end(0);
        lead.finished().await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let docs = [item("reviewComment", 4109844888), item("comment", 5841958969)];
        let at = fc.now();
        let sent = h.rt().workspaces().pull_request_send(&h.id, items_of(&docs), None, None).await.expect("a send");
        assert_eq!(j(&sent), json!({ "outcome": "started", "threadId": thread, "agent": "claude", "sent": sent_at(&docs, at) }));
        assert_eq!(agent.prompts()[1], send_prompt(&docs));
        assert!(agent.prompts()[1].contains("From vercel[bot], from outside the repository:"));
        let seen = j(&h.rt().workspaces().pull_request_view(&h.id, None, None).await.expect("a page"));
        assert_eq!(seen["sent"], sent_at(&docs, at));
        // Kept on the workspace's record, so a host that restarts still says what was sent.
        let record = h.store.get("workspaces", &h.id).await.expect("a read").expect("a record");
        assert_matches(&record, json!({ "prSent": sent_at(&docs, at) }));
        // A second send of one already sent moves its time and adds no second row; an item the page lacks sends nothing.
        agent.end(1);
        until(|| async { h.rt().sessions().list(None, None).await.expect("a list").iter().all(|v| j(v)["status"] != "running") }).await;
        fc.advance(60_000);
        let twice = item("comment", 5841958969);
        let again =
            j(&h.rt().workspaces().pull_request_send(&h.id, items_of(&[twice.clone(), twice.clone()]), None, None).await.expect("a send"));
        assert_eq!(
            again["sent"],
            json!([{ "kind": "reviewComment", "id": 4109844888u64, "at": at }, { "kind": "comment", "id": 5841958969u64, "at": at + 60_000 }])
        );
        assert_eq!(agent.prompts()[2], send_prompt(&[twice]));
        let missing = item("review", 1);
        let said = refused(h.rt().workspaces().pull_request_send(&h.id, items_of(&[missing]), None, None)).await;
        assert!(said.contains(&no_such_item_refusal(&PageItem { kind: PageItemKind::Review, id: 1.0 })), "{said}");
        assert_eq!(agent.prompts().len(), 3);
        agent.end(2);
    });
}

// "sends the items into the thread named, where two threads share the folder, and refuses a thread of another workspace"
#[test]
fn sends_the_items_into_the_thread_named_where_two_threads_share_the_folder_and_refuses_a_thread_of_another_workspace() {
    drive(|| async {
        let d = page_daemons();
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], ..With::default() }).await;
        let a = h.rt().sessions().start(&h.id, start("thread A"), None).await.expect("a start");
        agent.end(0);
        a.finished().await;
        let b = h.rt().sessions().start(&h.id, start("thread B"), None).await.expect("a start");
        agent.end(1);
        b.finished().await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let docs = [item("comment", 5841958969)];
        let b_thread = b.view().thread_id.expect("a thread");
        let sent = h.rt().workspaces().pull_request_send(&h.id, items_of(&docs), Some(b_thread.clone()), None).await.expect("a send");
        assert_matches(&j(&sent), json!({ "outcome": "started", "threadId": b_thread }));
        assert_eq!(agent.prompts()[2], send_prompt(&docs));
        agent.end(2);
        until(|| async { h.rt().sessions().list(None, None).await.expect("a list").iter().all(|v| j(v)["status"] != "running") }).await;
        let said = refused(h.rt().workspaces().pull_request_send(&h.id, items_of(&docs), Some("t_elsewhere".into()), None)).await;
        assert!(said.contains(&thread_elsewhere_line("t_elsewhere")), "{said}");
        assert_eq!(agent.prompts().len(), 3);
    });
}

// "answers a send held by its computer's threads at once the moment it is held, with the wait, and the turn starts once a slot frees"
#[test]
fn answers_a_send_held_by_its_computers_threads_at_once_the_moment_it_is_held_with_the_wait_and_the_turn_starts_once_a_slot_frees() {
    drive(|| async {
        let d = fake_daemons(
            vec![("git.status", answer(|_| status())), ("git.prView", view_answer(json!({}))), ("git.repoRead", no_repo())],
            vec![],
        );
        let dir = tempfile::tempdir().expect("a folder");
        let root = std::fs::canonicalize(dir.path()).expect("a real path");
        let state = sqlite_state();
        let agent = HeldAgent::new();
        let (channel, local) = (d.open(), local_on(&root, true));
        let links = place_wiring(wsp_store::keys::new_place_key_pair());
        let fx = runtime(&StubBackend::new(None))
            .store(&state.store)
            .adapter("claude", agent.factory())
            .with(move |o| {
                o.daemon_token = Some(DAEMON_TOKEN.into());
                o.daemon_channel = Some(channel);
                o.local = Some(local);
                o.place_links = Some(links);
            })
            .build();
        let rt = &*fx.rt;
        let repo = temp_repo();
        let r = repo.display().to_string();
        let _ = std::process::Command::new("git").args(["-C", &r, "remote", "add", "origin", "https://github.com/wsp/pr-lab.git"]).output();
        let project = project_on_with(rt, Some(HERE_PLACE_ID), Some(&r), "pr-lab", None).await;
        let ws = rt
            .workspaces()
            .create(wsp_core::types::wiring::CreateWorkspaceOptions { project, name: "pr-lab".into(), ..Default::default() }, None)
            .await
            .expect("a workspace")
            .view
            .id;
        let first = rt.sessions().start(&ws, start("set .ci-status to 1"), None).await.expect("a start");
        agent.end(0);
        first.finished().await;
        rt.workspaces().checkout(&ws, None, None).await.expect("a checkout");
        until(|| async { rt.workspaces().pull_request_view(&ws, None, None).await.is_ok() }).await;
        let places = rt.places().expect("a place door");
        let settings = serde_json::from_value(json!({ "threads": 1 })).expect("settings");
        places.set(HERE_PLACE_ID, settings, vec![], Default::default()).await.expect("a set");
        let busy_project = project_on_with(rt, Some(HERE_PLACE_ID), None, "busy", None).await;
        let busy = rt
            .workspaces()
            .create(
                wsp_core::types::wiring::CreateWorkspaceOptions { project: busy_project, name: "busy".into(), ..Default::default() },
                None,
            )
            .await
            .expect("a workspace")
            .view
            .id;
        rt.sessions().start(&busy, start("busy"), None).await.expect("a start");
        let docs = [item("comment", 5841958969)];
        let sent = rt.workspaces().pull_request_send(&ws, items_of(&docs), None, None).await.expect("a send");
        assert_matches(
            &j(&sent),
            json!({
                "outcome": "held", "threadId": first.view().thread_id, "agent": "claude",
                "capped": { "placeId": HERE_PLACE_ID, "running": 1, "atOnce": 1 }
            }),
        );
        assert_eq!(agent.prompts().len(), 2);
        agent.end(1);
        until(|| async { agent.prompts().len() == 3 }).await;
        assert_eq!(agent.prompts()[2], send_prompt(&docs));
        agent.end(2);
    });
}

// "drops what was sent once the workspace's pull request is another one"
#[test]
fn drops_what_was_sent_once_the_workspaces_pull_request_is_another_one() {
    drive(|| async {
        let fc = clocked();
        let (cell, read) = swappable(answer(|_| ok(json!({ "pr": open(json!({})) }))));
        let d = fake_daemons(vec![("git.prRead", read), ("git.prView", view_answer(json!({}))), ("git.repoRead", no_repo())], vec![]);
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        h.rt().workspaces().pull_request_send(&h.id, items_of(&[item("comment", 5841958969)]), None, None).await.expect("a send");
        let sent_len = || async {
            j(&h.rt().workspaces().pull_request_view(&h.id, None, None).await.expect("a page"))["sent"].as_array().map_or(0, Vec::len)
        };
        assert_eq!(sent_len().await, 1);
        // The same pull request read again keeps it.
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { d.of("git.prRead").len() >= 2 }).await;
        assert_eq!(sent_len().await, 1);
        *cell.borrow_mut() = answer(|_| ok(json!({ "pr": open(json!({ "number": 13, "url": "https://github.com/wsp/pr-lab/pull/13" })) })));
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { h.pr_field("number") == Some(json!(13)) }).await;
        assert_eq!(sent_len().await, 0);
        let record = h.store.get("workspaces", &h.id).await.expect("a read").expect("a record");
        assert!(record.get("prSent").is_none(), "{record}");
        agent.end(0);
    });
}

// "posts a reply, a resolve and a reaction as the person over this computer's daemon, and answers each changed item"
#[test]
fn posts_a_reply_a_resolve_and_a_reaction_as_the_person_over_this_computers_daemon_and_answers_each_changed_item() {
    drive(|| async {
        let comment = json!({ "id": 9, "nodeId": "IC_kwDOx", "author": "Zingzy", "association": "owner", "bot": false, "body": "Fixed.", "url": "u", "at": "t" });
        let reply = comment.clone();
        let d = fake_daemons(
            vec![
                ("git.prReply", answer(move |_| ok(json!({ "comment": reply.clone() })))),
                (
                    "git.prResolve",
                    answer(|f| ok(json!({ "threadId": f["threadId"].as_str().unwrap_or_default(), "resolved": f["resolved"] == true }))),
                ),
                (
                    "git.prReact",
                    answer(|f| {
                        ok(
                            json!({ "subject": f["subject"].as_str().unwrap_or_default(), "reactions": [{ "content": "eyes", "count": 1, "mine": true }] }),
                        )
                    }),
                ),
            ],
            vec![],
        );
        let h = with_workspace(&d, With::default()).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let replied = h
            .rt()
            .workspaces()
            .pull_request_reply(
                PullRequestReplyAsk { workspace_id: h.id.clone(), reply_to: Some(7), thread_id: None, body: "Fixed.".into() },
                None,
            )
            .await
            .expect("a reply");
        assert_eq!(j(&replied), json!({ "comment": comment }));
        let resolved = h.rt().workspaces().pull_request_resolve(&h.id, "PRRT_kwDOx", true, None).await.expect("a resolve");
        assert_eq!(j(&resolved), json!({ "threadId": "PRRT_kwDOx", "resolved": true }));
        let eyes = serde_json::from_value(json!("eyes")).expect("a reaction");
        let reacted = h.rt().workspaces().pull_request_react(&h.id, "IC_kwDOx", eyes, true, None).await.expect("a reaction");
        assert_eq!(j(&reacted), json!({ "subject": "IC_kwDOx", "reactions": [{ "content": "eyes", "count": 1, "mine": true }] }));
        let writes: Vec<(Via, Value)> = d
            .frames
            .borrow()
            .iter()
            .filter(|(_, f)| ["git.prReply", "git.prResolve", "git.prReact"].contains(&f["op"].as_str().unwrap_or_default()))
            .map(|(v, f)| (*v, Value::Object(f.clone())))
            .collect();
        let (cwd, remote) = (h.root.display().to_string(), h.remote.clone());
        assert_eq!(
            writes,
            [
                (Via::Here, json!({ "op": "git.prReply", "cwd": cwd, "remote": remote, "number": 12, "replyTo": 7, "body": "Fixed." })),
                (
                    Via::Here,
                    json!({ "op": "git.prResolve", "cwd": cwd, "remote": remote, "number": 12, "threadId": "PRRT_kwDOx", "resolved": true })
                ),
                (
                    Via::Here,
                    json!({ "op": "git.prReact", "cwd": cwd, "remote": remote, "number": 12, "subject": "IC_kwDOx", "content": "eyes", "on": true })
                ),
            ]
        );
        // A reply with no words is refused here and reaches no daemon.
        let said = refused(h.rt().workspaces().pull_request_reply(
            PullRequestReplyAsk { workspace_id: h.id.clone(), reply_to: None, thread_id: None, body: " \n ".into() },
            None,
        ))
        .await;
        assert!(said.contains(REPLY_EMPTY_LINE), "{said}");
        assert_eq!(d.of("git.prReply").len(), 1);
    });
}

// "never posts as the person through the copy's gh: a signed-out Mac with a running copy is refused and the copy is not asked"
#[test]
fn never_posts_as_the_person_through_the_copys_gh_a_signed_out_mac_with_a_running_copy_is_refused_and_the_copy_is_not_asked() {
    drive(|| async {
        let written = |op: &'static str| (op, answer(|_| ok(json!({}))));
        let d = fake_daemons(
            vec![("git.prReply", no_gh()), ("git.prResolve", no_gh()), ("git.prReact", no_gh()), ("git.prView", no_gh())],
            vec![written("git.prReply"), written("git.prResolve"), written("git.prReact"), ("git.prView", view_answer(json!({})))],
        );
        let h = with_workspace(&d, With::default()).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        // The copy still reads the page, and the page says nothing is posted from here.
        let seen = j(&h.rt().workspaces().pull_request_view(&h.id, None, None).await.expect("a page"));
        assert_eq!(seen["postsAsYou"], false);
        let line = pull_request_post_line("github.com");
        let said = refused(h.rt().workspaces().pull_request_reply(
            PullRequestReplyAsk { workspace_id: h.id.clone(), reply_to: None, thread_id: None, body: "x".into() },
            None,
        ))
        .await;
        assert!(said.contains(&line), "{said}");
        let said = refused(h.rt().workspaces().pull_request_resolve(&h.id, "PRRT_x", false, None)).await;
        assert!(said.contains(&line), "{said}");
        let plus = serde_json::from_value(json!("+1")).expect("a reaction");
        let said = refused(h.rt().workspaces().pull_request_react(&h.id, "IC_x", plus, false, None)).await;
        assert!(said.contains(&line), "{said}");
        let posts =
            |via| d.ops(Some(via)).into_iter().filter(|op| ["git.prReply", "git.prResolve", "git.prReact"].contains(&op.as_str())).count();
        assert_eq!(posts(Via::Copy), 0);
        assert_eq!(posts(Via::Here), 3);
    });
}

// "carries the thread a line reply goes into"
#[test]
fn carries_the_thread_a_line_reply_goes_into() {
    drive(|| async {
        let d = fake_daemons(
            vec![(
                "git.prReply",
                answer(|f| {
                    ok(json!({ "reviewComment": {
                        "id": 10, "path": "a.ts", "author": "Zingzy", "bot": false, "body": "b", "url": "u", "at": "t",
                        "replyTo": f["replyTo"].as_f64().unwrap_or_default(), "threadId": f["threadId"].as_str().unwrap_or_default(), "resolved": false
                    } }))
                }),
            )],
            vec![],
        );
        let h = with_workspace(&d, With::default()).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let read = h
            .rt()
            .workspaces()
            .pull_request_reply(
                PullRequestReplyAsk {
                    workspace_id: h.id.clone(),
                    reply_to: Some(7),
                    thread_id: Some("PRRT_kwDOx".into()),
                    body: "b".into(),
                },
                None,
            )
            .await
            .expect("a reply");
        assert_matches(&j(&read)["reviewComment"], json!({ "replyTo": 7, "threadId": "PRRT_kwDOx", "resolved": false }));
        let asked = d.of("git.prReply");
        assert_matches(&Value::Object(asked[0].1.clone()), json!({ "replyTo": 7, "threadId": "PRRT_kwDOx", "number": 12 }));
    });
}

// "keeps the checks the read that saw it merge found, so a settled one still shows its last checks"
#[test]
fn keeps_the_checks_the_read_that_saw_it_merge_found_so_a_settled_one_still_shows_its_last_checks() {
    drive(|| async {
        let checks = json!([{ "name": "ci", "workflow": "ci", "state": "pass", "startedAt": "2026-10-02T10:00:00Z", "completedAt": "2026-10-02T10:16:00Z" }]);
        let (cell, read) = swappable(answer(|_| ok(json!({ "pr": open(json!({})) }))));
        let d = fake_daemons(vec![("git.prRead", read)], vec![]);
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let record = h.store.get("workspaces", &h.id).await.expect("a read").expect("a record");
        assert!(record.get("pr").and_then(|pr| pr.get("checks")).is_none(), "{record}");
        let merged = open(json!({ "state": "merged", "checks": checks }));
        *cell.borrow_mut() = answer(move |_| ok(json!({ "pr": merged.clone() })));
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { h.pr_field("state") == Some(json!("merged")) }).await;
        let record = h.store.get("workspaces", &h.id).await.expect("a read").expect("a record");
        assert_matches(&record, json!({ "pr": { "number": 12, "state": "merged", "checks": checks } }));
    });
}

// "sends a failed check's log into the workspace's thread as a message framed as a log, and answers at once"
#[test]
fn sends_a_failed_checks_log_into_the_workspaces_thread_as_a_message_framed_as_a_log_and_answers_at_once() {
    drive(|| async {
        let failed = open(json!({ "checks": [
            { "name": "ci", "workflow": "ci", "state": "fail", "run": { "runId": 36, "jobId": 109 }, "link": "https://github.com/wsp/pr-lab/actions/runs/36/job/109" },
            { "name": "lint", "state": "pass" }
        ] }));
        let d = fake_daemons(
            vec![
                ("git.prRead", answer(move |_| ok(json!({ "pr": failed.clone() })))),
                ("git.runLog", answer(|_| ok(json!({ "lines": ["Run check\texit 1"], "truncated": false })))),
            ],
            vec![],
        );
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], ..With::default() }).await;
        let lead = h.rt().sessions().start(&h.id, start("set .ci-status to 1"), None).await.expect("a start");
        let thread = lead.view().thread_id.expect("a thread");
        agent.end(0);
        lead.finished().await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let ask = |check: &str| FixAsk { workspace_id: h.id.clone(), check: Some(check.into()), ..Default::default() };
        let asked = h.rt().workspaces().fix(ask("ci"), None).await.expect("a fix");
        assert_eq!(j(&asked), json!({ "outcome": "started", "threadId": thread, "check": "ci", "base": "main", "agent": "claude" }));
        let log = d.of("git.runLog").remove(0);
        assert_eq!(log.0, Via::Here);
        assert_matches(&Value::Object(log.1), json!({ "runId": 36, "jobId": 109 }));
        assert!(
            agent.prompts()[1].contains("The check \"ci\" in the ci workflow failed on commit ec5c10d (Set .ci-status to 1)."),
            "{}",
            agent.prompts()[1]
        );
        assert!(agent.prompts()[1].contains("```text\nRun check\texit 1\n```"));
        // A check that passed is not one to fix, and one the pull request lacks is named with the ones it has.
        let said = refused(h.rt().workspaces().fix(ask("lint"), None)).await;
        assert!(said.contains(&check_not_failed_refusal("lint", "pass")), "{said}");
        let said = refused(h.rt().workspaces().fix(ask("e2e"), None)).await;
        assert!(said.contains("the pull request has no check called e2e; it has ci, lint"), "{said}");
        agent.end(1);
    });
}

// "reads the checks whole before refusing a fix, never off what the last read held or saw"
#[test]
fn reads_the_checks_whole_before_refusing_a_fix_never_off_what_the_last_read_held_or_saw() {
    drive(|| async {
        // The check ran again and failed after a read that saw it pass, which moves nothing of what is seen.
        let checks =
            Rc::new(RefCell::new(json!([{ "name": "ci", "workflow": "ci", "state": "pass", "run": { "runId": 36, "jobId": 109 } }])));
        let held = checks.clone();
        let d = fake_daemons(
            vec![
                (
                    "git.prRead",
                    answer(move |f| {
                        if f.get("seen") == Some(&json!(SEEN)) {
                            ok(json!({ "unchanged": true, "seen": SEEN }))
                        } else {
                            ok(json!({ "pr": open(json!({ "checks": held.borrow().clone() })), "seen": SEEN }))
                        }
                    }),
                ),
                ("git.runLog", answer(|_| ok(json!({ "lines": ["Run check\texit 1"], "truncated": false })))),
            ],
            vec![],
        );
        let agent = HeldAgent::new();
        let fc = clocked();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        *checks.borrow_mut() = json!([{ "name": "ci", "workflow": "ci", "state": "fail", "run": { "runId": 36, "jobId": 109 } }]);
        fc.advance(5_000);
        let fixed = h
            .rt()
            .workspaces()
            .fix(FixAsk { workspace_id: h.id.clone(), check: Some("ci".into()), ..Default::default() }, None)
            .await
            .expect("a fix");
        assert_matches(&j(&fixed), json!({ "outcome": "started", "check": "ci" }));
        let last = d.of("git.prRead").pop().expect("a read");
        assert!(last.1.get("seen").is_none(), "{:?}", last.1);
        agent.end(0);
    });
}

const MERGE_THE_BASE: &str =
    "Merge the latest main into fix/ci, and resolve the conflicts in README.md. Run the tests, commit the merge, and push.";

// "with no check updates from the base first, sends nothing when it merged clean, and sends the conflicts when it did not"
#[test]
fn with_no_check_updates_from_the_base_first_sends_nothing_when_it_merged_clean_and_sends_the_conflicts_when_it_did_not() {
    drive(|| async {
        let (cell, update) = swappable(answer(|_| ok(json!({ "base": "main", "merged": true, "commits": 2, "conflicts": [] }))));
        let d = fake_daemons(vec![], vec![("git.update", update)]);
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], ..With::default() }).await;
        let lead = h.rt().sessions().start(&h.id, start("change the readme"), None).await.expect("a start");
        agent.end(0);
        lead.finished().await;
        let fixed = h.rt().workspaces().fix(FixAsk { workspace_id: h.id.clone(), ..Default::default() }, None).await.expect("a fix");
        assert_eq!(j(&fixed), json!({ "outcome": "updated", "base": "main" }));
        assert_eq!(agent.prompts().len(), 1);
        let asked = d.of("git.update").remove(0);
        assert_eq!(asked.0, Via::Copy);
        assert_matches(&Value::Object(asked.1), json!({ "base": "main" }));
        *cell.borrow_mut() = answer(|_| ok(json!({ "base": "main", "merged": false, "commits": 0, "conflicts": ["README.md"] })));
        let fixed = h.rt().workspaces().fix(FixAsk { workspace_id: h.id.clone(), ..Default::default() }, None).await.expect("a fix");
        assert_matches(&j(&fixed), json!({ "outcome": "started", "base": "main" }));
        assert_eq!(agent.prompts()[1], MERGE_THE_BASE);
        agent.end(1);
    });
}

// "sends the conflicts to the thread named where two threads share the folder, never the folder's first, and refuses one of another folder"
#[test]
fn sends_the_conflicts_to_the_thread_named_where_two_threads_share_the_folder_never_the_folders_first_and_refuses_one_of_another_folder() {
    drive(|| async {
        let d = fake_daemons(
            vec![],
            vec![("git.update", answer(|_| ok(json!({ "base": "main", "merged": false, "commits": 0, "conflicts": ["README.md"] }))))],
        );
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], ..With::default() }).await;
        let first = h.rt().sessions().start(&h.id, start("change the readme"), None).await.expect("a start");
        agent.end(0);
        first.finished().await;
        let second = h.rt().sessions().start(&h.id, start("change the docs"), None).await.expect("a start");
        agent.end(1);
        second.finished().await;
        let named = second.view().thread_id.expect("a thread");
        let fixed = h
            .rt()
            .workspaces()
            .fix(FixAsk { workspace_id: h.id.clone(), thread_id: Some(named.clone()), ..Default::default() }, None)
            .await
            .expect("a fix");
        assert_matches(&j(&fixed), json!({ "outcome": "started", "threadId": named }));
        assert_eq!(agent.prompts()[2], MERGE_THE_BASE);
        let said = refused(
            h.rt()
                .workspaces()
                .fix(FixAsk { workspace_id: h.id.clone(), thread_id: Some("t_elsewhere".into()), ..Default::default() }, None),
        )
        .await;
        assert!(said.contains(&thread_elsewhere_line("t_elsewhere")), "{said}");
        agent.end(2);
    });
}

// "refuses a merge and a check's fix where the copy has no pull request, naming the thread they were asked by"
#[test]
fn refuses_a_merge_and_a_checks_fix_where_the_copy_has_no_pull_request_naming_the_thread_they_were_asked_by() {
    drive(|| async {
        let d = fake_daemons(vec![("git.prRead", answer(|_| ok(json!({}))))], vec![]);
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], ..With::default() }).await;
        let opened = h.rt().sessions().start(&h.id, start("change the readme"), None).await.expect("a start");
        agent.end(0);
        opened.finished().await;
        let thread = opened.view().thread_id.expect("a thread");
        let line = no_pull_request_refusal(&format!("thread {}", thread_word(&thread)));
        let said = refused(
            h.rt().workspaces().merge(MergeAsk { workspace_id: h.id.clone(), thread_id: Some(thread.clone()), ..Default::default() }, None),
        )
        .await;
        assert!(said.contains(&line), "{said}");
        let said = refused(h.rt().workspaces().fix(
            FixAsk { workspace_id: h.id.clone(), check: Some("ci".into()), thread_id: Some(thread.clone()), ..Default::default() },
            None,
        ))
        .await;
        assert!(said.contains(&line), "{said}");
    });
}

// "merges the head the host read by the repository's default, refuses a method it does not allow and a wait it does not offer, then reads again"
#[test]
fn merges_the_head_the_host_read_by_the_repositorys_default_refuses_a_method_it_does_not_allow_and_a_wait_it_does_not_offer_then_reads_again(
) {
    drive(|| async {
        let (cell, read) = swappable(answer(|_| ok(json!({ "pr": open(json!({})) }))));
        let state = cell.clone();
        let d = fake_daemons(
            vec![
                ("git.prRead", read),
                ("git.repoRead", answer(|_| ok(json!({ "methods": ["merge", "squash"], "defaultMethod": "squash", "autoMerge": false })))),
                (
                    "git.prMerge",
                    answer(move |_| {
                        *state.borrow_mut() = answer(|_| ok(json!({ "pr": open(json!({ "state": "merged" })) })));
                        ok(json!({ "merged": true, "autoArmed": false }))
                    }),
                ),
            ],
            vec![],
        );
        let h = with_workspace(&d, With::default()).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let rebase = serde_json::from_value(json!("rebase")).expect("a method");
        let allowed: Vec<wsp_wire::types::pull_request::MergeMethod> = serde_json::from_value(json!(["merge", "squash"])).expect("methods");
        let said =
            refused(h.rt().workspaces().merge(MergeAsk { workspace_id: h.id.clone(), method: Some(rebase), ..Default::default() }, None))
                .await;
        let want = merge_method_refusal(&serde_json::from_value(json!("rebase")).expect("a method"), &allowed);
        assert!(said.contains(&want), "{said}");
        let said = refused(
            h.rt().workspaces().merge(MergeAsk { workspace_id: h.id.clone(), when_checks_pass: Some(true), ..Default::default() }, None),
        )
        .await;
        assert!(said.contains(wsp_wire::types::pull_request::AUTO_MERGE_OFF_LINE), "{said}");
        let merged = h.rt().workspaces().merge(MergeAsk { workspace_id: h.id.clone(), ..Default::default() }, None).await.expect("a merge");
        assert_eq!(j(&merged), json!({ "number": 12, "method": "squash", "merged": true, "autoArmed": false }));
        let asked = d.of("git.prMerge").remove(0);
        assert_eq!(asked.0, Via::Here);
        assert_eq!(
            Value::Object(asked.1),
            json!({ "op": "git.prMerge", "cwd": h.root.display().to_string(), "remote": h.remote, "number": 12, "method": "squash", "auto": false, "headOid": HEAD })
        );
        // The repository's settings are asked once for all three.
        assert_eq!(d.count(Via::Here, "git.repoRead"), 1);
        until(|| async { h.pr_field("state") == Some(json!("merged")) }).await;
    });
}

// "merges the head the person was shown and never one a later read found, whatever the fact's age"
#[test]
fn merges_the_head_the_person_was_shown_and_never_one_a_later_read_found_whatever_the_facts_age() {
    drive(|| async {
        let (cell, read) = swappable(answer(|_| ok(json!({ "pr": open(json!({})) }))));
        let merges: Rc<RefCell<Vec<Value>>> = Rc::default();
        let kept = merges.clone();
        let d = fake_daemons(
            vec![
                ("git.prRead", read),
                ("git.repoRead", answer(|_| ok(json!({ "methods": ["squash"], "defaultMethod": "squash", "autoMerge": false })))),
                (
                    "git.prMerge",
                    answer(move |f| {
                        kept.borrow_mut().push(Value::Object(f.clone()));
                        ok(json!({ "merged": false, "autoArmed": false }))
                    }),
                ),
            ],
            vec![],
        );
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("headOid") == Some(json!(HEAD)) }).await;
        // The agent pushes after the page was drawn and the fact grows old: the merge still names what was shown.
        let moved = "b".repeat(40);
        let new_head = moved.clone();
        *cell.borrow_mut() = answer(move |_| ok(json!({ "pr": open(json!({ "headOid": new_head })) })));
        fc.advance(60_000);
        let before = reads(&d);
        // With no head named, the fact the host holds is the head, not a fresh read of it.
        h.rt().workspaces().merge(MergeAsk { workspace_id: h.id.clone(), ..Default::default() }, None).await.expect("a merge");
        assert_matches(merges.borrow().last().expect("a merge"), json!({ "headOid": HEAD }));
        let at = d.frames.borrow().iter().position(|(_, f)| f["op"] == "git.prMerge").expect("a merge frame");
        let early = d.frames.borrow()[..at].iter().filter(|(_, f)| f["op"] == "git.prRead").count();
        assert_eq!(early, before);
        // The read after that merge holds the new head; a window that drew the old one still merges only the old one.
        until(|| async { h.pr_field("headOid") == Some(json!(moved)) }).await;
        h.rt()
            .workspaces()
            .merge(MergeAsk { workspace_id: h.id.clone(), head: Some(HEAD.into()), ..Default::default() }, None)
            .await
            .expect("a merge");
        assert_matches(merges.borrow().last().expect("a merge"), json!({ "headOid": HEAD }));
    });
}

// "refuses a merge asked by a thread's own token in the guard's words, and lets a thread update a copy of its tree"
#[test]
fn refuses_a_merge_asked_by_a_threads_own_token_in_the_guards_words_and_lets_a_thread_update_a_copy_of_its_tree() {
    drive(|| async {
        let d = fake_daemons(
            vec![],
            vec![("git.update", answer(|_| ok(json!({ "base": "main", "merged": true, "commits": 1, "conflicts": [] }))))],
        );
        let agent = HeldAgent::new();
        let h = with_workspace(&d, With { adapters: vec![("claude", agent.factory())], ..With::default() }).await;
        let lead = h.rt().sessions().start(&h.id, start("lead"), None).await.expect("a start");
        let root = lead.view().thread_id.expect("a thread");
        let scope = as_thread(&h.id, &root, &root);
        let said =
            refused(h.rt().workspaces().merge(MergeAsk { workspace_id: h.id.clone(), ..Default::default() }, Some(scope.clone()))).await;
        assert!(said.contains(&spawn_act_refusal(&root, SpawnAct::Merge, Some(MACHINE_WSP_FORKS))), "{said}");
        let updated = h.rt().workspaces().update(&h.id, None, Some(scope)).await.expect("an update");
        assert_eq!(j(&updated), json!({ "base": "main", "merged": true, "commits": 1, "conflicts": [] }));
        assert!(!d.ops(None).contains(&"git.prMerge".to_string()));
        agent.end(0);
    });
}

// "updates the copy from its base through the copy's own daemon, then reads the branch line and the pull request again"
#[test]
fn updates_the_copy_from_its_base_through_the_copys_own_daemon_then_reads_the_branch_line_and_the_pull_request_again() {
    drive(|| async {
        let d = fake_daemons(
            vec![],
            vec![("git.update", answer(|_| ok(json!({ "base": "main", "merged": false, "commits": 0, "conflicts": ["README.md"] }))))],
        );
        let h = with_workspace(&d, With::default()).await;
        let updated = h.rt().workspaces().update(&h.id, None, None).await.expect("an update");
        assert_eq!(j(&updated), json!({ "base": "main", "merged": false, "commits": 0, "conflicts": ["README.md"] }));
        let ops: Vec<String> =
            d.frames.borrow().iter().map(|(v, f)| format!("{}:{}", v.word(), f["op"].as_str().unwrap_or_default())).collect();
        let update = ops.iter().position(|o| o == "copy:git.update").expect("an update frame");
        let status = ops.iter().rposition(|o| o == "copy:git.status").expect("a status frame");
        assert!(update < status, "{ops:?}");
        until(|| async { d.ops(Some(Via::Here)).contains(&"git.prRead".to_string()) }).await;
        let _ = (RATE_LIMIT_HOLD_MS, PR_PAGE_HOLD_MS);
    });
}

// A `Caller`'s thread origin and a session handle, kept in view so the helper types above stay used.
#[allow(dead_code)]
fn _unused(_: Rc<dyn SessionHandle>, _: HostError) {}
