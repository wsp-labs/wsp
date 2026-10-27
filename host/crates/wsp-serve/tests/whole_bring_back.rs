// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/bring-back.test.ts at 65f6b89c7, case for case. Bringing work back: the two frames the runtime
//! sends down a workspace's own daemon channel, which base each carries, and what a machine with no signed-in command
//! line for the git host answers with. Each test names the TS test above it.
mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use common::*;
use serde_json::{json, Value};
use wsp_core::daemon_channel::Frame;
use wsp_core::daemon_token::daemon_token_for;
use wsp_words::daemon_contract::{no_git_credential_line, no_host_cli_line, on_base_refusal};
use wsp_words::words::computer::{branch_unread_refusal, no_parent_workspace_line};

const DAEMON_TOKEN: &str = "cafef00dcafef00dcafef00d";

/// Every fact of a pull request a read answers but its number, link, state and host, which each case names.
fn pr_rest() -> Value {
    json!({ "draft": false, "base": "main", "branch": "work", "headOid": "abc1234", "headSubject": "Do the work", "mergeable": "unknown", "mergeState": "unknown", "review": "none", "checks": [], "additions": 1, "deletions": 0, "changedFiles": 1, "commits": 1 })
}

fn pr_with(number: i64, url: &str, state: &str, host: &str) -> Value {
    let mut pr = json!({ "number": number, "url": url, "state": state, "host": host });
    pr.as_object_mut().expect("an object").extend(pr_rest().as_object().expect("an object").clone());
    pr
}

/// A daemon that records every frame and answers each op the way one on a machine would.
fn fake_daemon() -> Rc<RecDaemon> {
    RecDaemon::new(|frame: &Frame| {
        if frame["op"] == "git.push" {
            json!({
                "id": 1, "ok": true, "branch": "pricing-page", "base": frame.get("base").and_then(Value::as_str).unwrap_or_default(),
                "remote": "origin", "ahead": 2, "uncommitted": 1, "stat": [" src/page.tsx | 4 ++--"],
            })
        } else {
            json!({ "id": 1, "ok": true, "pr": pr_with(12, "https://github.com/o/r/pull/12", "open", "github.com"), "created": true })
        }
    })
}

struct Ws {
    fx: Fx,
    backend: Arc<StubBackend>,
    id: String,
    project: String,
}

/// A runtime whose one workspace has a daemon answering, as a fork on a box does.
async fn with_workspace(daemon: &Rc<RecDaemon>, base: Option<&str>) -> Ws {
    let backend = StubBackend::new(None);
    backend.set_exec_impl(token_guest());
    let backend = backend.with_daemon_roads();
    let open = daemon.open();
    let fx = runtime(&backend)
        .with(move |o| {
            o.daemon_token = Some(DAEMON_TOKEN.into());
            o.daemon_channel = Some(open);
        })
        .build();
    let project = project_with(&fx.rt, None, None, None, base).await.expect("a project").id;
    let ws = create_on(&fx.rt, CreateOn { project: Some(project.clone()), ..CreateOn::golden("pricing page") }, None).await.expect("a workspace");
    Ws { fx, backend, id: ws.view.id, project }
}

fn op_of(f: &Frame) -> String {
    f["op"].as_str().unwrap_or_default().to_string()
}

fn not_status(daemon: &RecDaemon) -> Vec<Frame> {
    daemon.frames.borrow().iter().filter(|f| f["op"] != "git.status").cloned().collect()
}

fn base_of(f: &Frame) -> Value {
    f.get("base").cloned().unwrap_or(Value::Null)
}

async fn bring(w: &Ws, id: &str) -> Result<Value, wsp_words::exit::HostError> {
    w.fx.rt.workspaces().bring_back(id, None, None, None).await.map(|b| j(&b))
}

fn frame_json(f: &Frame) -> Value {
    Value::Object(f.clone())
}

// "workspaces.bringBack" > "a workspace with no parent pushes and opens the pull request against the project's own base, on one dial that is closed after"
#[test]
fn a_workspace_with_no_parent_pushes_and_opens_the_pull_request_against_the_projects_own_base_on_one_dial_that_is_closed_after() {
    drive(|| async {
        let daemon = fake_daemon();
        let w = with_workspace(&daemon, Some("main")).await;
        let back = w
            .fx
            .rt
            .workspaces()
            .bring_back(&w.id, Some("the pricing page".into()), Some("what it does".into()), None)
            .await
            .expect("a bring back");
        let frames: Vec<Value> = daemon.frames.borrow().iter().map(frame_json).collect();
        assert_eq!(
            frames,
            [
                json!({ "op": "git.push", "cwd": "/root/stub-1", "base": "main" }),
                json!({ "op": "git.pr", "cwd": "/root/stub-1", "base": "main", "title": "the pricing page", "body": "what it does" }),
            ]
        );
        assert_eq!(
            j(&back),
            json!({
                "branch": "pricing-page", "base": "main", "ahead": 2, "uncommitted": 1, "stat": [" src/page.tsx | 4 ++--"],
                "pr": pr_with(12, "https://github.com/o/r/pull/12", "open", "github.com"),
            })
        );
        let token = daemon_token_for(DAEMON_TOKEN, "m1").expect("a token");
        assert_eq!(*daemon.dials.borrow(), [("http://127.0.0.1:9".to_string(), token)]);
        assert_eq!(daemon.closed.get(), 1);
        let _ = &w.backend;
        w.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.bringBack" > "a project that named no base leaves it to the checkout, which reads its own"
#[test]
fn a_project_that_named_no_base_leaves_it_to_the_checkout_which_reads_its_own() {
    drive(|| async {
        let daemon = fake_daemon();
        let w = with_workspace(&daemon, None).await;
        bring(&w, &w.id).await.expect("a bring back");
        let bases: Vec<Value> = daemon.frames.borrow().iter().map(base_of).collect();
        assert_eq!(bases, [Value::Null, Value::Null]);
        w.fx.rt.close().await.expect("a close");
    });
}

fn status_on(head: &str, upstream: Option<&str>, ahead: i64) -> Value {
    let mut branch = json!({ "oid": "abc", "head": head, "ahead": ahead, "behind": 0 });
    if let Some(upstream) = upstream {
        branch["upstream"] = json!(upstream);
    }
    json!({ "id": 1, "ok": true, "branch": branch, "entries": [], "root": "/root/stub-1" })
}

fn start_on(f: &Frame) -> Value {
    json!({ "id": 1, "ok": true, "branch": f["branch"], "oid": "c0ffee" })
}

// "workspaces.bringBack" > "a branch only the parent's own copy holds is pushed first, and the child starts and lands on it"
#[test]
fn a_branch_only_the_parents_own_copy_holds_is_pushed_first_and_the_child_starts_and_lands_on_it() {
    drive(|| async {
        let daemon = fake_daemon();
        daemon.answer("git.status", |_| status_on("only-here", None, 2));
        daemon.answer("git.startOn", start_on);
        let w = with_workspace(&daemon, Some("main")).await;
        let child = create_on(
            &w.fx.rt,
            CreateOn { project: Some(w.project.clone()), parent: Some(w.id.clone()), ..CreateOn::golden("second look") },
            None,
        )
        .await
        .expect("a child");
        assert_eq!(j(&child.view)["parentWorkspaceId"], w.id.as_str());
        assert_eq!(daemon.ops()[0], "git.status");
        let rest: Vec<(String, Value)> = not_status(&daemon)
            .iter()
            .map(|f| (op_of(f), f.get("base").or_else(|| f.get("branch")).cloned().unwrap_or(Value::Null)))
            .collect();
        assert_eq!(rest, [("git.push".to_string(), json!("main")), ("git.startOn".to_string(), json!("pricing-page"))]);
        daemon.frames.borrow_mut().clear();
        bring(&w, &child.view.id).await.expect("a bring back");
        let bases: Vec<Value> = not_status(&daemon).iter().map(base_of).collect();
        assert_eq!(bases, [json!("pricing-page"), json!("pricing-page")]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.bringBack" > "a parent that did not say which branch it is on stops the fork, and no child is made"
#[test]
fn a_parent_that_did_not_say_which_branch_it_is_on_stops_the_fork_and_no_child_is_made() {
    drive(|| async {
        // The daemon is there and the read is not: what a stopped machine, a shell that failed and the read's own bound
        // all come back as. A child started at the project's base here would land its work at the project's base.
        let daemon = fake_daemon();
        daemon.answer("git.status", |_| json!({ "id": 1, "ok": false, "error": "machine is paused" }));
        let w = with_workspace(&daemon, Some("main")).await;
        let refused = create_on(
            &w.fx.rt,
            CreateOn { project: Some(w.project.clone()), parent: Some(w.id.clone()), ..CreateOn::golden("second look") },
            None,
        )
        .await
        .err()
        .expect("a refusal");
        assert_eq!(refused.message, branch_unread_refusal("pricing page", "machine is paused"));
        let names: Vec<String> = w.fx.rt.workspaces().list(None).await.expect("a list").iter().map(|v| v.name.clone()).collect();
        assert_eq!(names, ["pricing page"]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.bringBack" > "a child whose parent went to sleep after the fork brings back against the branch it was forked from"
#[test]
fn a_child_whose_parent_went_to_sleep_after_the_fork_brings_back_against_the_branch_it_was_forked_from() {
    drive(|| async {
        let parent_answers = Rc::new(Cell::new(true));
        let daemon = fake_daemon();
        let answering = parent_answers.clone();
        daemon.answer("git.status", move |_| {
            if answering.get() {
                status_on("pricing-page", Some("origin/pricing-page"), 0)
            } else {
                json!({ "id": 1, "ok": false, "error": "machine is paused" })
            }
        });
        daemon.answer("git.startOn", start_on);
        let w = with_workspace(&daemon, Some("main")).await;
        let child = create_on(
            &w.fx.rt,
            CreateOn { project: Some(w.project.clone()), parent: Some(w.id.clone()), ..CreateOn::golden("second look") },
            None,
        )
        .await
        .expect("a child");
        // The parent naps, and its daemon would now answer with what a stopped machine answers. The child's work still
        // goes back into the branch it was cut from, and nothing asks the parent anything.
        w.fx.rt.workspaces().nap(&w.id, None).await.expect("a nap");
        parent_answers.set(false);
        let asked = daemon.frames.borrow().len();
        let back = bring(&w, &child.view.id).await.expect("a bring back");
        assert_eq!(back["base"], "pricing-page");
        let bases: Vec<Value> = daemon.frames.borrow()[asked..].iter().filter(|f| f["op"] != "git.status").map(base_of).collect();
        assert_eq!(bases, [json!("pricing-page"), json!("pricing-page")]);
        let parent = daemon_token_for(DAEMON_TOKEN, "m1").expect("a token");
        assert_eq!(daemon.tokens.borrow()[asked..].iter().filter(|t| **t == parent).count(), 0);
        w.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.bringBack" > "a create naming a parent this host does not hold is refused, not landed as a root"
#[test]
fn a_create_naming_a_parent_this_host_does_not_hold_is_refused_not_landed_as_a_root() {
    drive(|| async {
        let daemon = fake_daemon();
        let w = with_workspace(&daemon, Some("main")).await;
        let refused = create_on(
            &w.fx.rt,
            CreateOn { project: Some(w.project.clone()), parent: Some("ws_nobody".into()), ..CreateOn::golden("orphan") },
            None,
        )
        .await
        .err()
        .expect("a refusal");
        assert_eq!(refused.message, no_parent_workspace_line("ws_nobody"));
        let names: Vec<String> = w.fx.rt.workspaces().list(None).await.expect("a list").iter().map(|v| v.name.clone()).collect();
        assert_eq!(names, ["pricing page"]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.bringBack" > "a child workspace measures against the branch its parent was on at the fork, whatever the parent does after"
#[test]
fn a_child_workspace_measures_against_the_branch_its_parent_was_on_at_the_fork_whatever_the_parent_does_after() {
    drive(|| async {
        let head = Rc::new(std::cell::RefCell::new("pricing-page".to_string()));
        let daemon = fake_daemon();
        let at = head.clone();
        daemon.answer("git.status", move |_| {
            let head = at.borrow().clone();
            status_on(&head, Some(&format!("origin/{head}")), 0)
        });
        daemon.answer("git.startOn", start_on);
        let w = with_workspace(&daemon, Some("main")).await;
        let child = create_on(
            &w.fx.rt,
            CreateOn { project: Some(w.project.clone()), parent: Some(w.id.clone()), ..CreateOn::golden("pricing page copy") },
            None,
        )
        .await
        .expect("a child");
        let view = j(&child.view);
        assert_eq!(view["parentWorkspaceId"], w.id.as_str());
        assert_eq!(view["project"]["id"], w.project.as_str());
        // The child starts where its parent stands, not where the project does: its copy is put on that branch.
        let started: Vec<Value> = daemon.frames.borrow().iter().filter(|f| f["op"] == "git.startOn").map(|f| f["branch"].clone()).collect();
        assert_eq!(started, [json!("pricing-page")]);
        // The parent switches branches after the fork; the child's work still belongs on the branch it was cut from.
        *head.borrow_mut() = "somewhere-else".into();
        daemon.frames.borrow_mut().clear();
        bring(&w, &child.view.id).await.expect("a bring back");
        let bases: Vec<Value> = not_status(&daemon).iter().map(base_of).collect();
        assert_eq!(bases, [json!("pricing-page"), json!("pricing-page")]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.bringBack" > "a refusal from the push is the caller's error, and nothing is asked about a pull request after it"
#[test]
fn a_refusal_from_the_push_is_the_callers_error_and_nothing_is_asked_about_a_pull_request_after_it() {
    drive(|| async {
        let daemon = fake_daemon();
        daemon.answer("git.push", |_| json!({ "id": 1, "ok": false, "error": on_base_refusal("main") }));
        let w = with_workspace(&daemon, Some("main")).await;
        let refused = bring(&w, &w.id).await.err().expect("a refusal");
        assert!(refused.message.contains(&on_base_refusal("main")), "{}", refused.message);
        assert_eq!(daemon.ops(), ["git.push"]);
        // The dial is closed however the work ends, so a refusal leaves no socket on the machine.
        assert_eq!(daemon.closed.get(), 1);
        w.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.bringBack" > "a machine with no command line for the git host still pushes, and the sentence comes back as the note"
#[test]
fn a_machine_with_no_command_line_for_the_git_host_still_pushes_and_the_sentence_comes_back_as_the_note() {
    drive(|| async {
        let note = no_host_cli_line("github.com");
        let daemon = fake_daemon();
        let said = note.clone();
        daemon.answer("git.pr", move |_| json!({ "id": 1, "ok": false, "error": said, "code": "no-host-cli" }));
        let w = with_workspace(&daemon, Some("main")).await;
        let back = bring(&w, &w.id).await.expect("a bring back");
        assert_eq!(back["note"], note.as_str());
        assert!(back.get("pr").is_none());
        assert_eq!(back["branch"], "pricing-page");
        assert_eq!(back["ahead"], 2);
        w.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.bringBack" > "any other refusal of the pull request comes back beside the push, since the branch has landed by then"
#[test]
fn any_other_refusal_of_the_pull_request_comes_back_beside_the_push_since_the_branch_has_landed_by_then() {
    drive(|| async {
        let said = "gh said: could not create pull request";
        let daemon = fake_daemon();
        daemon.answer("git.pr", move |_| json!({ "id": 1, "ok": false, "error": said }));
        let w = with_workspace(&daemon, Some("main")).await;
        let back = bring(&w, &w.id).await.expect("a bring back");
        assert_eq!(back["refused"], said);
        assert!(back.get("note").is_none());
        assert!(back.get("pr").is_none());
        // The push's own fields are the answer's first half, whatever the pull request half then said.
        let first: Value = ["branch", "base", "ahead", "uncommitted", "stat"].iter().map(|k| ((*k).to_string(), back[*k].clone())).collect();
        assert_eq!(first, json!({ "branch": "pricing-page", "base": "main", "ahead": 2, "uncommitted": 1, "stat": [" src/page.tsx | 4 ++--"] }));
        w.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.bringBack" > "a push refused for want of a credential is the whole verb's refusal, since nothing landed"
#[test]
fn a_push_refused_for_want_of_a_credential_is_the_whole_verbs_refusal_since_nothing_landed() {
    drive(|| async {
        let said = no_git_credential_line("github.com", Some("sign gh in on it with gh auth login, then gh auth setup-git"));
        let daemon = fake_daemon();
        let line = said.clone();
        daemon.answer("git.push", move |_| json!({ "id": 1, "ok": false, "error": line }));
        let w = with_workspace(&daemon, Some("main")).await;
        let refused = bring(&w, &w.id).await.err().expect("a refusal");
        assert!(refused.message.contains(&said), "{}", refused.message);
        assert_eq!(daemon.ops(), ["git.push"]);
        w.fx.rt.close().await.expect("a close");
    });
}
