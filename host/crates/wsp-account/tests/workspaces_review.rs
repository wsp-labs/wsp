// SPDX-License-Identifier: AGPL-3.0-only
//! The start and review doors, the review draft and its post, the daemon and guest channels, port reach and probe, and
//! the filters the event stream runs per socket. Ported from the start-review, runtime-reach, local-workspace,
//! box-lead and serve cases of packages/runtime/test at 65f6b89c7, ported at the doors' seam over `FakeReview`.
//! Every sentence is the TS protocol source's own text (START_WORDS), written out literally.
//! `creating` is create.rs's and is pinned in tests/workspaces_create.rs.
#[path = "support/base.rs"]
mod base;
#[path = "support/review.rs"]
mod review;

use std::cell::RefCell;
use std::rc::Rc;

use base::{err_answer, person, project, rec_json, record, run, thread, FakeBase};
use review::{draft_json, issue_json, pr_json, FakeReview, HEAD};
use serde_json::{json, Map, Value};
use wsp_account::projects::workspaces::review as door;
use wsp_account::projects::workspaces::review::ReviewCtx;
use wsp_account::projects::workspaces::seam::Entry;
use wsp_core::types::api::{DaemonEventListener, ReviewAsk, ReviewDraftAsk, StartFromLinkAsk};
use wsp_core::types::wiring::SysListener;
use wsp_wire::types::start::ReviewBlockVerdict;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::{HostError, Kind};

const REPO: &str = "dev/spoo";
const REMOTE: &str = "https://github.com/dev/spoo.git";
const NOT_A_PR: &str = "a review needs a pull request's link, or a thread with a pull request";
const PR_LINK: &str = "https://github.com/dev/spoo/pull/7";
const DIFF: &str =
    "diff --git a/check.sh b/check.sh\n--- a/check.sh\n+++ b/check.sh\n@@ -1,3 +1,4 @@\n #!/bin/sh\n+unused=1\n exit 0\n # end\n";

struct H {
    fake: Rc<FakeReview>,
    ctx: Rc<dyn ReviewCtx>,
}

impl H {
    fn new() -> H {
        let fake = FakeReview::new();
        let ctx: Rc<dyn ReviewCtx> = fake.clone();
        fake.base.hold_project(project("pr_1", "spoo", "here", json!({ "remote": REMOTE, "path": "/home/me/spoo" })));
        H { fake, ctx }
    }
    fn base(&self) -> &Rc<FakeBase> {
        &self.fake.base
    }
    /// This computer answers the diff read the review makes.
    fn with_diff(&self) {
        self.base().answer_with("ask:git.prDiff", vec![json!({ "diff": DIFF, "truncated": false, "left": [] })]);
    }
}

fn ok<T>(r: Result<T, HostError>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("expected success, got: {}", e.message),
    }
}

fn refusal<T>(r: Result<T, HostError>) -> HostError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("expected a refusal"),
    }
}

fn said(e: &HostError, message: &str, kind: Option<Kind>) {
    assert_eq!(e.message, message);
    assert_eq!(e.kind, kind);
}

fn dbg_of(c: &Option<Caller>) -> Value {
    review::origin_text(c)
}

/// Nothing was made, persisted or emitted.
fn nothing_done(b: &FakeBase) {
    assert!(b.persisted.borrow().is_empty());
    assert!(b.events.borrow().is_empty());
    assert!(b.args_of("workspace_from").is_empty());
    assert!(b.args_of("open_with").is_empty());
}

fn from_review() -> Value {
    json!({ "kind": "review", "repo": REPO, "number": 7, "url": PR_LINK, "title": "Rename the status word", "base": "main",
        "head": { "branch": "lab/review-me", "oid": HEAD } })
}

/// A workspace under review (`ws_rev`, project `pr_1`) over `over`.
fn under_review(h: &H, over: Value) -> Entry {
    let mut v = json!({ "name": "Review #7 Rename the status word", "from": from_review() });
    base::merge(&mut v, over);
    h.base().hold(record("ws_rev", "local", "pr_1", v))
}

fn held(h: &H, id: &str, over: Value) -> Entry {
    h.base().hold(record(id, "local", "pr_1", over))
}

fn draft_event() -> Value {
    json!({ "type": "workspace.review", "workspaceId": "ws_rev" })
}

fn listener(seen: &Rc<RefCell<Vec<Value>>>) -> DaemonEventListener {
    let seen = seen.clone();
    Rc::new(move |m: Map<String, Value>| seen.borrow_mut().push(Value::Object(m)))
}

fn frame(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

async fn channel_name(c: &Rc<dyn wsp_core::daemon_channel::DaemonChannel>) -> Value {
    match c.send(Map::new()).await {
        Ok(f) => f["channel"].clone(),
        Err(_) => panic!("the fake channel answers"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 1: start

#[test]
fn start_refuses_not_a_link() {
    // start-review.test.ts:193
    run(async {
        let h = H::new();
        let ask = StartFromLinkAsk { url: "fix the flaky test".to_owned(), ..Default::default() };
        let e = refusal(door::start(&h.ctx, ask, Some(person())).await);
        said(&e, "fix the flaky test is not a link to a GitHub issue or pull request", Some(Kind::Invalid));
        let long = "a".repeat(100);
        let e = refusal(door::start(&h.ctx, StartFromLinkAsk { url: long.clone(), ..Default::default() }, None).await);
        said(&e, &format!("{} is not a link to a GitHub issue or pull request", &long[..80]), Some(Kind::Invalid));
        assert_eq!(h.base().names(), vec!["spawn_guard", "ready", "spawn_guard", "ready"]);
        assert_eq!(
            h.base().args_of("spawn_guard"),
            vec![json!({ "act": "start", "scoped": false }), json!({ "act": "start", "scoped": false })]
        );
        assert!(h.base().asks().is_empty());
        nothing_done(h.base());
    });
}

#[test]
fn start_refuses_a_link_no_project_holds_and_reads_nothing() {
    // start-review.test.ts:193
    run(async {
        let h = H::new();
        let sentence = "no project here is a checkout of someone/else; add one with wsp add https://github.com/someone/else";
        h.base().answer_with("project_by_repo", vec![err_answer(sentence, Some("invalid"))]);
        let ask = StartFromLinkAsk { url: "https://github.com/someone/else/issues/1".to_owned(), ..Default::default() };
        let e = refusal(door::start(&h.ctx, ask, None).await);
        said(&e, sentence, Some(Kind::Invalid));
        assert_eq!(h.base().args_of("project_by_repo"), vec![json!({ "repo": "someone/else", "named": null })]);
        assert_eq!(h.base().names(), vec!["spawn_guard", "ready", "project_by_repo"]);
        assert!(h.base().asks().is_empty());
        nothing_done(h.base());
    });
}

#[test]
fn start_reads_checks_picks_then_places_and_opens() {
    // start-review.test.ts:203 (a pull request), folder-threads.test.ts:1327 (picks before placing)
    run(async {
        let h = H::new();
        h.base().answer_with("pull_request_of", vec![pr_json(REPO, 7, json!({ "branch": "feat/pr" }))]);
        h.base().answer_with(
            "issue_of",
            vec![json!({
            "number": 7, "url": PR_LINK, "title": "Rename the status word", "body": "Renames the word.", "state": "OPEN",
            "comments": [{ "author": "maya", "body": "Keep it one line.", "at": "2026-09-29T10:00:00Z" }] })],
        );
        h.base().answer_with("workspace_from", vec![json!({ "behind": "this worktree is behind", "setup": "npm ci runs first" })]);
        let ask = StartFromLinkAsk {
            url: PR_LINK.to_owned(),
            project: Some("spoo".to_owned()),
            agent: Some("claude".to_owned()),
            model: Some("opus".to_owned()),
            effort: Some("high".to_owned()),
            access: Some(AccessChoice::Full),
        };
        let opened = ok(door::start(&h.ctx, ask, Some(person())).await);
        let order = [
            "spawn_guard",
            "ready",
            "project_by_repo",
            "issue_of",
            "pull_request_of",
            "from_of",
            "picks_hold",
            "workspace_from",
            "open_with",
        ];
        assert_eq!(h.base().names_of(&order), order);
        assert_eq!(h.base().args_of("spawn_guard"), vec![json!({ "act": "start", "scoped": false })]);
        assert_eq!(h.base().args_of("project_by_repo"), vec![json!({ "repo": REPO, "named": "spoo" })]);
        assert_eq!(h.base().args_of("issue_of"), vec![json!({ "remote": REMOTE, "number": 7 })]);
        assert_eq!(h.base().args_of("pull_request_of"), vec![json!({ "remote": REMOTE, "number": 7 })]);
        assert_eq!(h.base().args_of("from_of"), vec![json!({ "kind": "pull_request", "repo": REPO, "number": 7, "withFact": true })]);
        assert_eq!(
            h.base().args_of("picks_hold"),
            vec![json!({ "project": "pr_1", "picks": { "harness": "claude", "model": "opus", "effort": "high", "access": "full" } })]
        );
        let placed = h.base().args_of("workspace_from");
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0]["kind"], json!("start"));
        assert_eq!(placed[0]["origin"], dbg_of(&Some(person())));
        assert_eq!(placed[0]["fact"]["branch"], json!("feat/pr"));
        assert_eq!(placed[0]["from"]["kind"], json!("pull_request"));
        assert_eq!(placed[0]["from"]["head"], json!({ "branch": "feat/pr", "oid": HEAD }));
        let sent = h.base().args_of("open_with");
        assert_eq!(sent.len(), 1);
        let prompt = sent[0]["prompt"].as_str().unwrap().to_owned();
        assert!(prompt.contains("Rename the status word"), "{prompt}");
        assert!(prompt.contains("maya: Keep it one line."), "{prompt}");
        assert!(prompt.contains("feat/pr"), "{prompt}");
        let mut keys = sent[0].clone();
        keys.as_object_mut().unwrap().remove("prompt");
        assert_eq!(
            keys,
            json!({ "entry": "ws_placed", "behind": "this worktree is behind", "setup": "npm ci runs first", "harness": "claude", "model": "opus",
                "effort": "high", "access": "full", "origin": dbg_of(&Some(person())) })
        );
        assert_eq!((opened.thread_id.as_str(), opened.session_id.as_str()), ("th_open", "se_open"));
        assert!(h.base().persisted.borrow().is_empty());
    });
}

#[test]
fn start_of_an_issue_reads_no_pr() {
    // start-review.test.ts:174
    run(async {
        let h = H::new();
        h.base().answer_with("issue_of", vec![issue_json(REPO, 5, "Add a greeting line to notes.txt")]);
        let url = "https://github.com/dev/spoo/issues/5".to_owned();
        let opened = ok(door::start(&h.ctx, StartFromLinkAsk { url, ..Default::default() }, None).await);
        assert!(h.base().args_of("pull_request_of").is_empty());
        assert_eq!(h.base().args_of("from_of"), vec![json!({ "kind": "issue", "repo": REPO, "number": 5, "withFact": false })]);
        assert_eq!(h.base().args_of("picks_hold"), vec![json!({ "project": "pr_1", "picks": {} })]);
        let placed = h.base().args_of("workspace_from");
        assert_eq!(placed[0]["fact"], json!(null));
        assert_eq!(placed[0]["kind"], json!("start"));
        assert_eq!(placed[0]["from"]["title"], json!("Add a greeting line to notes.txt"));
        let sent = h.base().args_of("open_with");
        let prompt = sent[0]["prompt"].as_str().unwrap().to_owned();
        assert!(prompt.contains("Add a greeting line to notes.txt"), "{prompt}");
        assert!(prompt.contains("Closes #5"), "{prompt}");
        assert_eq!(sent[0]["origin"], dbg_of(&None));
        let mut bare = sent[0].clone();
        bare.as_object_mut().unwrap().remove("prompt");
        assert_eq!(bare, json!({ "entry": "ws_placed", "origin": dbg_of(&None) }));
        assert_eq!(opened.thread_id, "th_open");
        assert!(h.base().asks().is_empty());
    });
}

#[test]
fn start_stops_at_a_refused_pick_before_anything_is_placed() {
    // folder-threads.test.ts:1327
    run(async {
        let h = H::new();
        let sentence = "claude has no model no-such-model";
        h.base().answer_with("picks_hold", vec![err_answer(sentence, Some("invalid"))]);
        let ask = StartFromLinkAsk {
            url: PR_LINK.to_owned(),
            agent: Some("claude".to_owned()),
            model: Some("no-such-model".to_owned()),
            ..Default::default()
        };
        let e = refusal(door::start(&h.ctx, ask, None).await);
        said(&e, sentence, Some(Kind::Invalid));
        assert_eq!(h.base().names_of(&["picks_hold", "workspace_from", "open_with"]), vec!["picks_hold"]);
        nothing_done(h.base());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 2: review

#[test]
fn review_by_url_refuses_an_issue_link() {
    run(async {
        let h = H::new();
        let ask = ReviewAsk { url: Some("https://github.com/dev/spoo/issues/5".to_owned()), ..Default::default() };
        let e = refusal(door::review(&h.ctx, ask, Some(person())).await);
        said(&e, NOT_A_PR, Some(Kind::Invalid));
        let ask = ReviewAsk { url: Some("look at the open one".to_owned()), ..Default::default() };
        let e = refusal(door::review(&h.ctx, ask, None).await);
        said(&e, "look at the open one is not a link to a GitHub issue or pull request", Some(Kind::Invalid));
        assert_eq!(h.base().names(), vec!["spawn_guard", "ready", "spawn_guard", "ready"]);
        assert_eq!(h.base().args_of("spawn_guard")[0], json!({ "act": "review", "scoped": false }));
        assert!(h.base().asks().is_empty());
        nothing_done(h.base());
    });
}

#[test]
fn review_by_url_opens_a_codex_reviewer_at_its_read_only_word_with_the_diff() {
    // start-review.test.ts:241
    run(async {
        let h = H::new();
        h.with_diff();
        h.base().answer_with("pull_request_of", vec![pr_json(REPO, 7, json!({}))]);
        h.base().answer_with("issue_of", vec![json!({
            "number": 7, "url": PR_LINK, "title": "Rename the status word", "body": "Renames the word.", "state": "OPEN", "comments": [] })]);
        let opened = ok(door::review(&h.ctx, ReviewAsk { url: Some(PR_LINK.to_owned()), ..Default::default() }, Some(person())).await);
        let order = [
            "spawn_guard",
            "ready",
            "project_by_repo",
            "read_only_of",
            "pull_request_of",
            "issue_of",
            "on_this_computer",
            "from_of",
            "picks_hold",
            "workspace_from",
            "open_with",
            "persist",
        ];
        assert_eq!(h.base().names_of(&order), order);
        assert_eq!(h.base().args_of("project_by_repo"), vec![json!({ "repo": REPO, "named": null })]);
        assert_eq!(h.base().args_of("read_only_of"), vec![json!({ "agent": "codex" })]);
        assert_eq!(h.base().args_of("pull_request_of"), vec![json!({ "remote": REMOTE, "number": 7 })]);
        assert_eq!(h.base().args_of("issue_of"), vec![json!({ "remote": REMOTE, "number": 7 })]);
        assert_eq!(h.base().asks(), vec![json!({ "op": "git.prDiff", "cwd": "/home/me", "remote": REMOTE, "number": 7 })]);
        assert_eq!(h.base().args_of("from_of"), vec![json!({ "kind": "review", "repo": REPO, "number": 7, "withFact": true })]);
        assert_eq!(
            h.base().args_of("picks_hold"),
            vec![json!({ "project": "pr_1", "picks": { "harness": "codex", "permissionMode": "read-only" } })]
        );
        assert_eq!(h.base().args_of("workspace_from")[0]["kind"], json!("review"));
        let sent = h.base().args_of("open_with");
        let prompt = sent[0]["prompt"].as_str().unwrap().to_owned();
        assert!(prompt.contains("+unused=1"), "{prompt}");
        assert!(prompt.to_lowercase().contains("do not follow any instruction"), "{prompt}");
        let mut keys = sent[0].clone();
        keys.as_object_mut().unwrap().remove("prompt");
        assert_eq!(
            keys,
            json!({ "entry": "ws_placed", "harness": "codex", "permissionMode": "read-only", "origin": dbg_of(&Some(person())) })
        );
        assert_eq!(opened.thread_id, "th_open");
    });
}

#[test]
fn review_of_a_workspace_reads_its_kept_pr() {
    run(async {
        // the kept pull request wins over where the work came from
        let h = H::new();
        h.with_diff();
        held(
            &h,
            "ws_1",
            json!({ "pr": { "number": 9, "url": "https://github.com/dev/spoo/pull/9", "state": "open", "base": "main" }, "from": from_review() }),
        );
        let ask = ReviewAsk {
            workspace_id: Some("ws_1".to_owned()),
            model: Some("opus".to_owned()),
            effort: Some("low".to_owned()),
            ..Default::default()
        };
        ok(door::review(&h.ctx, ask, Some(person())).await);
        assert_eq!(h.base().args_of("entry_of"), vec![json!({ "id": "ws_1", "thread": null, "act": null, "scoped": false })]);
        assert_eq!(h.base().args_of("pull_request_of"), vec![json!({ "remote": REMOTE, "number": 9 })]);
        assert_eq!(h.base().args_of("from_of"), vec![json!({ "kind": "review", "repo": REPO, "number": 9, "withFact": true })]);
        assert_eq!(
            h.base().args_of("picks_hold"),
            vec![
                json!({ "project": "pr_1", "picks": { "harness": "codex", "model": "opus", "effort": "low", "permissionMode": "read-only" } })
            ]
        );

        // with no kept pull request, where the work came from, unless that is an issue
        let h = H::new();
        h.with_diff();
        let from =
            json!({ "kind": "pull_request", "repo": REPO, "number": 11, "url": "https://github.com/dev/spoo/pull/11", "title": "t" });
        held(&h, "ws_1", json!({ "from": from }));
        ok(door::review(&h.ctx, ReviewAsk { workspace_id: Some("ws_1".to_owned()), ..Default::default() }, None).await);
        assert_eq!(h.base().args_of("pull_request_of"), vec![json!({ "remote": REMOTE, "number": 11 })]);

        let h = H::new();
        let issue = json!({ "kind": "issue", "repo": REPO, "number": 5, "url": "https://github.com/dev/spoo/issues/5", "title": "t" });
        held(&h, "ws_1", json!({ "from": issue }));
        let e = refusal(door::review(&h.ctx, ReviewAsk { workspace_id: Some("ws_1".to_owned()), ..Default::default() }, None).await);
        said(&e, NOT_A_PR, Some(Kind::Invalid));
        assert!(h.base().args_of("pull_request_of").is_empty());
        nothing_done(h.base());

        // a workspace with neither, and no workspace named at all
        let h = H::new();
        held(&h, "ws_1", json!({}));
        let e = refusal(door::review(&h.ctx, ReviewAsk { workspace_id: Some("ws_1".to_owned()), ..Default::default() }, None).await);
        said(&e, NOT_A_PR, Some(Kind::Invalid));
        let e = refusal(door::review(&h.ctx, ReviewAsk::default(), None).await);
        said(&e, NOT_A_PR, Some(Kind::Invalid));
        assert!(h.base().args_of("pull_request_of").is_empty());
        nothing_done(h.base());
    });
}

#[test]
fn review_refuses_an_agent_with_no_read_only_mode() {
    // start-review.test.ts:256
    run(async {
        let h = H::new();
        h.base().answer_with("read_only_of", vec![json!(null)]);
        h.base().answer_with("reviewers", vec![json!(["codex", "claude"])]);
        let ask = ReviewAsk { url: Some(PR_LINK.to_owned()), agent: Some("gemini".to_owned()), ..Default::default() };
        let e = refusal(door::review(&h.ctx, ask, None).await);
        said(&e, "gemini has no read-only access wsp can give a reviewer; review with codex or claude", Some(Kind::Invalid));
        assert_eq!(h.base().args_of("read_only_of"), vec![json!({ "agent": "gemini" })]);
        assert_eq!(
            h.base().names_of(&[
                "project_by_repo",
                "read_only_of",
                "reviewers",
                "pull_request_of",
                "issue_of",
                "on_this_computer",
                "picks_hold"
            ]),
            vec!["project_by_repo", "read_only_of", "reviewers"]
        );
        assert!(h.base().asks().is_empty());
        nothing_done(h.base());
    });
}

#[test]
fn review_records_the_review_thread() {
    run(async {
        let h = H::new();
        h.with_diff();
        let opened = ok(door::review(
            &h.ctx,
            ReviewAsk { url: Some(PR_LINK.to_owned()), agent: Some("claude".to_owned()), ..Default::default() },
            None,
        )
        .await);
        let placed = h.base().live.borrow().get("ws_placed").cloned().expect("the placed workspace is held");
        assert_eq!(rec_json(&placed)["reviewThread"], json!("th_open"));
        assert_eq!(opened.thread_id, "th_open");
        let persisted = h.base().persisted.borrow();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].id, "ws_placed");
        assert_eq!(persisted[0].review_thread.as_deref(), Some("th_open"));
        assert_eq!(h.base().args_of("open_with")[0]["harness"], json!("claude"));
        assert_eq!(h.base().names_of(&["open_with", "persist"]), vec!["open_with", "persist"]);
        assert!(h.base().events.borrow().is_empty());
    });
}

#[test]
fn review_of_a_fork_that_takes_no_maintainer_push_still_opens() {
    // branch-of-record.test.ts:584
    run(async {
        let h = H::new();
        h.with_diff();
        h.base().answer_with("pull_request_of", vec![pr_json(REPO, 7, json!({ "fork": { "owner": "ana", "pushable": false } }))]);
        ok(door::review(&h.ctx, ReviewAsk { url: Some(PR_LINK.to_owned()), ..Default::default() }, None).await);
        assert_eq!(h.base().args_of("workspace_from")[0]["fact"]["fork"], json!({ "owner": "ana", "pushable": false }));
        assert_eq!(h.base().args_of("open_with").len(), 1);
    });
}

#[test]
fn a_threads_token_is_refused_a_start_a_review_and_a_post() {
    // start-review.test.ts:339
    run(async {
        let h = H::new();
        h.base().answer_with(
            "spawn_guard",
            vec![
                err_answer("start work from a link", Some("usage")),
                err_answer("start a review", Some("usage")),
                err_answer("post a review", Some("usage")),
            ],
        );
        let caller = Some(thread("thr_1", "ws_x", "thr_1"));
        let e = refusal(
            door::start(
                &h.ctx,
                StartFromLinkAsk { url: "https://github.com/dev/spoo/issues/5".to_owned(), ..Default::default() },
                caller.clone(),
            )
            .await,
        );
        said(&e, "start work from a link", Some(Kind::Usage));
        let e = refusal(door::review(&h.ctx, ReviewAsk { url: Some(PR_LINK.to_owned()), ..Default::default() }, caller.clone()).await);
        said(&e, "start a review", Some(Kind::Usage));
        let e = refusal(door::review_post(&h.ctx, "ws_x", None, caller).await);
        said(&e, "post a review", Some(Kind::Usage));
        assert_eq!(
            h.base().args_of("spawn_guard"),
            vec![
                json!({ "act": "start", "scoped": true }),
                json!({ "act": "review", "scoped": true }),
                json!({ "act": "review_post", "scoped": true }),
            ]
        );
        assert_eq!(h.base().names(), vec!["spawn_guard", "spawn_guard", "spawn_guard"]);
        assert!(h.base().asks().is_empty());
        nothing_done(h.base());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 3, 4: review_draft, review_post

fn ticks(on: &[(&str, bool)]) -> Option<Vec<(String, bool)>> {
    Some(on.iter().map(|(id, v)| ((*id).to_owned(), *v)).collect())
}

#[test]
fn review_draft_moves_ticks_and_emits_only_when_something_given() {
    // start-review.test.ts:316
    run(async {
        let h = H::new();
        let entry = under_review(&h, json!({ "review": draft_json(json!({})) }));
        let ask = ReviewDraftAsk {
            workspace_id: "ws_rev".to_owned(),
            summary: Some("One unused variable, and a nit.".to_owned()),
            verdict: Some(ReviewBlockVerdict::Comment),
            on: ticks(&[("c1", false), ("c9", false)]),
        };
        let got = ok(door::review_draft(&h.ctx, ask, Some(person())).await).expect("the draft is answered");
        let want = json!({
            "verdict": "comment", "summary": "One unused variable, and a nit.",
            "comments": [
                { "id": "c1", "path": "check.sh", "line": 2, "side": "RIGHT", "body": "unused is never read.", "on": false },
                { "id": "c2", "path": "check.sh", "line": 40, "side": "RIGHT", "body": "and this line is not in the diff.", "on": true, "inSummary": true },
            ],
            "headOid": HEAD, "threadId": "th_review", "at": 1,
        });
        assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&want).unwrap());
        assert_eq!(serde_json::to_string(&rec_json(&entry)["review"]).unwrap(), serde_json::to_string(&want).unwrap());
        assert_eq!(h.base().persisted.borrow().len(), 1);
        assert_eq!(h.base().events.borrow().clone(), vec![draft_event()]);
        assert_eq!(h.base().args_of("entry_of"), vec![json!({ "id": "ws_rev", "thread": null, "act": null, "scoped": false })]);
        assert_eq!(h.base().names_of(&["entry_of", "persist", "emit"]), vec!["entry_of", "persist", "emit"]);

        // only the summary: the verdict and every tick stay
        let h = H::new();
        let entry = under_review(&h, json!({ "review": draft_json(json!({})) }));
        let ask = ReviewDraftAsk { workspace_id: "ws_rev".to_owned(), summary: Some("Shorter.".to_owned()), ..Default::default() };
        ok(door::review_draft(&h.ctx, ask, None).await);
        let kept = rec_json(&entry)["review"].clone();
        assert_eq!(kept["summary"], json!("Shorter."));
        assert_eq!(kept["verdict"], json!("request_changes"));
        assert_eq!(kept["comments"], draft_json(json!({}))["comments"]);
        assert_eq!(h.base().events.borrow().len(), 1);
    });
}

#[test]
fn review_draft_changes_nothing_where_nothing_is_given_or_nothing_reads() {
    run(async {
        // a draft that reads, asked with nothing: answered as it stands
        let h = H::new();
        let entry = under_review(&h, json!({ "review": draft_json(json!({})) }));
        let before = rec_json(&entry);
        let got = ok(door::review_draft(&h.ctx, ReviewDraftAsk { workspace_id: "ws_rev".to_owned(), ..Default::default() }, None).await);
        assert_eq!(serde_json::to_value(got.unwrap()).unwrap(), draft_json(json!({})));
        assert_eq!(rec_json(&entry), before);
        nothing_done(h.base());

        // the sentence saying why none was read: answered as it stands, whatever is given
        let h = H::new();
        let note = json!({ "note": "the reply has no fenced json block", "at": 2, "reasked": true });
        let entry = under_review(&h, json!({ "review": note }));
        let before = rec_json(&entry);
        let ask = ReviewDraftAsk { workspace_id: "ws_rev".to_owned(), summary: Some("Fine.".to_owned()), ..Default::default() };
        let got = ok(door::review_draft(&h.ctx, ask, None).await);
        assert_eq!(serde_json::to_value(got.unwrap()).unwrap(), note);
        assert_eq!(rec_json(&entry), before);
        nothing_done(h.base());

        // no draft at all: an empty answer
        let h = H::new();
        under_review(&h, json!({}));
        let ask = ReviewDraftAsk { workspace_id: "ws_rev".to_owned(), summary: Some("Fine.".to_owned()), ..Default::default() };
        assert!(ok(door::review_draft(&h.ctx, ask, None).await).is_none());
        nothing_done(h.base());
    });
}

#[test]
fn invariant_4_review_post_runs_on_this_computer_pinned_to_the_drafts_head() {
    // start-review.test.ts:316
    run(async {
        let h = H::new();
        let draft = draft_json(
            json!({ "headOid": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", "verdict": "comment", "summary": "One unused variable, and a nit.",
            "comments": [
                { "id": "c1", "path": "check.sh", "line": 2, "side": "RIGHT", "body": "unused is never read.", "on": false },
                { "id": "c2", "path": "check.sh", "line": 40, "side": "RIGHT", "body": "and this line is not in the diff.", "on": true, "inSummary": true },
            ] }),
        );
        let entry = under_review(&h, json!({ "review": draft.clone() }));
        h.base().answer_with(
            "ask:git.prReview",
            vec![json!({ "url": "https://github.com/dev/spoo/pull/7#pullrequestreview-1", "folded": ["c2"] })],
        );
        let posted = ok(door::review_post(&h.ctx, "ws_rev", Some("th_review".to_owned()), Some(person())).await);

        // the frame went through this computer as the person, never the copy's daemon
        let order =
            ["spawn_guard", "entry_of", "on_this_computer", "with_daemon", "persist", "emit", "drop_page_cache", "reread_pull_request"];
        assert_eq!(
            h.base().names_of(&order),
            vec!["spawn_guard", "entry_of", "on_this_computer", "persist", "emit", "drop_page_cache", "reread_pull_request"]
        );
        assert_eq!(h.base().args_of("spawn_guard"), vec![json!({ "act": "review_post", "scoped": false })]);
        assert_eq!(h.base().args_of("entry_of"), vec![json!({ "id": "ws_rev", "thread": "th_review", "act": null, "scoped": false })]);
        assert_eq!(
            h.base().asks(),
            vec![json!({
                "op": "git.prReview", "cwd": "/home/me", "remote": REMOTE, "number": 7,
                "headOid": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", "event": "comment", "body": "One unused variable, and a nit.",
                "comments": [{ "id": "c2", "path": "check.sh", "line": 40, "side": "RIGHT", "body": "and this line is not in the diff." }],
            })]
        );
        assert!(h.base().args_of("git").is_empty());

        let mut want = draft.clone();
        want["posted"] = json!({ "url": "https://github.com/dev/spoo/pull/7#pullrequestreview-1", "at": 1_000_000, "folded": ["c2"] });
        assert_eq!(serde_json::to_string(&rec_json(&entry)["review"]).unwrap(), serde_json::to_string(&want).unwrap());
        assert_eq!(h.base().persisted.borrow().len(), 1);
        assert_eq!(h.base().events.borrow().clone(), vec![draft_event()]);
        assert_eq!(h.base().args_of("drop_page_cache"), vec![json!({ "remote": REMOTE, "number": 7 })]);
        assert_eq!(h.base().args_of("reread_pull_request"), vec![json!({ "id": "ws_rev" })]);
        assert_eq!(
            serde_json::to_value(&posted).unwrap(),
            json!({ "url": "https://github.com/dev/spoo/pull/7#pullrequestreview-1", "number": 7, "comments": 0, "folded": 1 })
        );
    });
}

#[test]
fn review_post_sends_the_ticked_comments_and_counts_those_folded_into_the_summary() {
    // start-review.test.ts:316 (the counts)
    run(async {
        let h = H::new();
        under_review(&h, json!({ "review": draft_json(json!({})) }));
        h.base().answer_with(
            "ask:git.prReview",
            vec![json!({ "url": "https://github.com/dev/spoo/pull/7#pullrequestreview-2", "folded": [] })],
        );
        let posted = ok(door::review_post(&h.ctx, "ws_rev", None, None).await);
        let sent = h.base().asks();
        assert_eq!(sent[0]["event"], json!("request_changes"));
        assert_eq!(sent[0]["body"], json!("One unused variable."));
        assert_eq!(sent[0]["headOid"], json!(HEAD));
        assert_eq!(sent[0]["comments"].as_array().unwrap().len(), 2);
        assert_eq!(serde_json::to_value(&posted).unwrap()["comments"], json!(2));
        assert_eq!(serde_json::to_value(&posted).unwrap()["folded"], json!(0));
        assert_eq!(h.base().args_of("entry_of"), vec![json!({ "id": "ws_rev", "thread": null, "act": null, "scoped": false })]);
    });
}

#[test]
fn review_post_refuses_with_no_draft() {
    // start-review.test.ts:316
    run(async {
        let h = H::new();
        under_review(&h, json!({}));
        let e = refusal(door::review_post(&h.ctx, "ws_rev", None, None).await);
        said(&e, "Review #7 Rename the status word has no review to post yet", None);
        h.base().answer_with("label_of", vec![json!("thread abcd1234")]);
        let e = refusal(door::review_post(&h.ctx, "ws_rev", Some("abcd1234-0000".to_owned()), None).await);
        said(&e, "thread abcd1234 has no review to post yet", None);
        assert_eq!(
            h.base().args_of("label_of"),
            vec![json!({ "id": "ws_rev", "thread": null }), json!({ "id": "ws_rev", "thread": "abcd1234-0000" })]
        );
        assert_eq!(h.base().args_of("entry_of")[1], json!({ "id": "ws_rev", "thread": "abcd1234-0000", "act": null, "scoped": false }));

        // a draft that is only the sentence saying why none was read, and a draft with no origin
        let h = H::new();
        under_review(&h, json!({ "review": { "note": "the reply has no fenced json block", "at": 2 } }));
        let e = refusal(door::review_post(&h.ctx, "ws_rev", None, None).await);
        said(&e, "Review #7 Rename the status word has no review to post yet", None);
        let h = H::new();
        under_review(&h, json!({ "review": draft_json(json!({})), "from": null }));
        let e = refusal(door::review_post(&h.ctx, "ws_rev", None, None).await);
        said(&e, "Review #7 Rename the status word has no review to post yet", None);
        assert!(h.base().asks().is_empty());
        assert!(h.base().args_of("drop_page_cache").is_empty());
        assert!(h.base().args_of("reread_pull_request").is_empty());
        nothing_done(h.base());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 5-9: daemon reach, channels, watch_sys

#[test]
fn daemon_reach_answers_the_kinds_road_and_its_refusal() {
    // runtime-reach.test.ts:22, local-workspace.test.ts:1379, :1411
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        let road = json!({ "url": "http://127.0.0.1:54321", "expiresAt": 9_007_199_254_740_991_i64, "daemonToken": "t0ken" });
        h.base().answer_with("daemon_road", vec![road.clone()]);
        let got = ok(door::daemon_reach(&h.ctx, "ws_1", Some(person())).await);
        assert_eq!(serde_json::to_value(&got).unwrap(), road);
        assert_eq!(h.base().args_of("entry_of"), vec![json!({ "id": "ws_1", "thread": null, "act": null, "scoped": false })]);
        assert_eq!(h.base().args_of("daemon_road"), vec![json!({ "id": "ws_1" })]);

        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("daemon_road", vec![err_answer("this host wired no daemon for this computer", None)]);
        let e = refusal(door::daemon_reach(&h.ctx, "ws_1", None).await);
        said(&e, "this host wired no daemon for this computer", None);
    });
}

#[test]
fn token_refused_forgets_the_machines_token() {
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({ "machineId": "m_one" }));
        held(&h, "ws_2", json!({ "machineId": "m_two" }));
        ok(door::token_refused(&h.ctx, "ws_2").await);
        assert_eq!(h.base().args_of("forget_daemon_token"), vec![json!({ "machineId": "m_two" })]);
        assert_eq!(h.base().args_of("entry_of"), vec![json!({ "id": "ws_2", "thread": null, "act": null, "scoped": false })]);

        let h = H::new();
        h.base().answer_with("entry_of", vec![err_answer("no such workspace ws_nobody", Some("not-found"))]);
        let e = refusal(door::token_refused(&h.ctx, "ws_nobody").await);
        said(&e, "no such workspace ws_nobody", Some(Kind::NotFound));
        assert!(h.base().args_of("forget_daemon_token").is_empty());
    });
}

#[test]
fn daemon_channel_of_a_shared_daemon_tracks_ptys() {
    // local-workspace.test.ts:262, :520
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("shared_daemon", vec![json!(true)]);
        let pids = h.fake.port_pids("ws_1");
        pids.borrow_mut().insert("pty_9".to_owned(), 4242);
        pids.borrow_mut().insert("pty_7".to_owned(), 4343);
        h.base().calls.borrow_mut().clear();
        let seen = Rc::new(RefCell::new(vec![]));
        let channel = ok(door::daemon_channel(&h.ctx, "ws_1", listener(&seen), Some(person())).await);
        assert_eq!(channel_name(&channel).await, json!("shared"));
        assert_eq!(
            h.base().names_of(&["entry_of", "shared_daemon", "port_pids", "shared_channel", "copy_channel"]),
            vec!["entry_of", "shared_daemon", "port_pids", "shared_channel"]
        );

        let heard = h.fake.heard.borrow().clone().expect("the shared channel was handed the heard listener");
        heard(frame(vec![("type", json!("pty.exit")), ("ptyId", json!("pty_9"))]));
        assert_eq!(pids.borrow().keys().cloned().collect::<Vec<_>>(), vec!["pty_7"]);
        assert_eq!(h.base().args_of("port_roots_moved"), vec![json!({ "id": "ws_1" })]);
        heard(frame(vec![("type", json!("pty.exit")), ("ptyId", json!("pty_9"))]));
        heard(frame(vec![("type", json!("pty.exit")), ("ptyId", json!("pty_unknown"))]));
        heard(frame(vec![("type", json!("pty.data")), ("ptyId", json!("pty_7"))]));
        assert_eq!(pids.borrow().keys().cloned().collect::<Vec<_>>(), vec!["pty_7"]);
        assert_eq!(h.base().args_of("port_roots_moved").len(), 1);
        assert_eq!(
            seen.borrow().clone(),
            vec![
                json!({ "type": "pty.exit", "ptyId": "pty_9" }),
                json!({ "type": "pty.exit", "ptyId": "pty_9" }),
                json!({ "type": "pty.exit", "ptyId": "pty_unknown" }),
                json!({ "type": "pty.data", "ptyId": "pty_7" }),
            ]
        );
        heard(frame(vec![("type", json!("pty.exit")), ("ptyId", json!("pty_7"))]));
        assert!(pids.borrow().is_empty());
        assert_eq!(h.base().args_of("port_roots_moved").len(), 2);
    });
}

#[test]
fn daemon_channel_of_a_private_daemon_is_the_plain_channel() {
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        let seen = Rc::new(RefCell::new(vec![]));
        let channel = ok(door::daemon_channel(&h.ctx, "ws_1", listener(&seen), None).await);
        assert_eq!(channel_name(&channel).await, json!("plain"));
        assert_eq!(h.base().names_of(&["shared_channel", "copy_channel", "port_pids"]), vec!["copy_channel"]);
        assert!(h.base().args_of("port_roots_moved").is_empty());
    });
}

#[test]
fn guest_channel_is_the_guest_road_channel_of_the_workspace() {
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        let seen = Rc::new(RefCell::new(vec![]));
        let channel = ok(door::guest_channel(&h.ctx, "ws_1", listener(&seen)).await);
        assert_eq!(channel_name(&channel).await, json!("guest"));
        assert_eq!(h.base().args_of("guest_road_channel"), vec![json!({ "id": "ws_1" })]);
        assert_eq!(h.base().args_of("entry_of"), vec![json!({ "id": "ws_1", "thread": null, "act": null, "scoped": false })]);
    });
}

#[test]
fn served_by_its_computer_is_true_where_the_computer_serves_it() {
    // local-workspace.test.ts:900
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        assert!(!ok(door::served_by_its_computer(&h.ctx, "ws_1", None).await));
        h.base().answer_with("served_by_its_computer", vec![json!("pl_boat")]);
        assert!(ok(door::served_by_its_computer(&h.ctx, "ws_1", Some(person())).await));
    });
}

#[test]
fn watch_sys_refuses_a_kind_reading_its_own_load() {
    run(async {
        let on: SysListener = Rc::new(|_| {});
        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("metrics_on_host", vec![json!(false)]);
        h.base().answer_with("machine_word", vec![json!("the computer")]);
        let e = refusal(door::watch_sys(&h.ctx, "ws_1", on.clone(), None).await);
        said(&e, "the computer reads its own load over its daemon link, not from this host", None);
        assert!(h.base().args_of("sys_samples").is_empty());

        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("sys_samples", vec![json!(false)]);
        let e = refusal(door::watch_sys(&h.ctx, "ws_1", on.clone(), None).await);
        said(&e, "this host reads nothing of the computer it runs on", None);

        let h = H::new();
        held(&h, "ws_1", json!({}));
        let detach = ok(door::watch_sys(&h.ctx, "ws_1", on, Some(person())).await);
        assert!(h.base().args_of("sys_detach").is_empty());
        detach();
        assert_eq!(h.base().args_of("sys_detach").len(), 1);
        assert_eq!(h.base().args_of("sys_samples").len(), 1);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 10, 11: port reach and probe

#[test]
fn port_reach_prefers_the_kinds_own_road() {
    // runtime-reach.test.ts:1012
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("kind_port_reach", vec![json!({ "url": "http://127.0.0.1:7070/", "expiresAt": 77 })]);
        let got = ok(door::port_reach(&h.ctx, "ws_1", 5173, Some(person())).await);
        assert_eq!(serde_json::to_value(&got).unwrap(), json!({ "url": "http://127.0.0.1:7070/", "expiresAt": 77 }));
        assert_eq!(
            h.base().names_of(&["entry_of", "copy_blocked", "kind_port_reach", "ws_port_reach"]),
            vec!["entry_of", "copy_blocked", "kind_port_reach"]
        );
        assert_eq!(h.base().args_of("kind_port_reach"), vec![json!({ "id": "ws_1", "port": 5173, "standing": false })]);

        let h = H::new();
        held(&h, "ws_1", json!({}));
        let got = ok(door::port_reach(&h.ctx, "ws_1", 3000, None).await);
        assert_eq!(
            serde_json::to_value(&got).unwrap(),
            json!({ "url": "https://m1-3000.preview.example/?pt_token=t1", "expiresAt": 9000 })
        );
        assert_eq!(
            h.base().names_of(&["entry_of", "copy_blocked", "kind_port_reach", "ws_port_reach"]),
            vec!["entry_of", "copy_blocked", "kind_port_reach", "ws_port_reach"]
        );
        assert_eq!(h.base().args_of("ws_port_reach"), vec![json!({ "id": "ws_1", "port": 3000 })]);
    });
}

#[test]
fn port_reach_refuses_where_the_copy_is_blocked_or_the_workspace_unknown() {
    // runtime-reach.test.ts:1032, box-panels.test.ts:325
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("copy_blocked", vec![err_answer("spoo's server is restarting", Some("conflict"))]);
        let e = refusal(door::port_reach(&h.ctx, "ws_1", 3000, None).await);
        said(&e, "spoo's server is restarting", Some(Kind::Conflict));
        assert!(h.base().args_of("kind_port_reach").is_empty());
        assert!(h.base().args_of("ws_port_reach").is_empty());

        let h = H::new();
        h.base().answer_with("entry_of", vec![err_answer("no such workspace ws_nobody", Some("not-found"))]);
        let e = refusal(door::port_reach(&h.ctx, "ws_nobody", 3000, None).await);
        said(&e, "no such workspace ws_nobody", Some(Kind::NotFound));
        assert_eq!(h.base().names(), vec!["entry_of"]);
    });
}

#[test]
fn port_probe_remints_after_a_401() {
    // runtime-reach.test.ts:1134
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("probe", vec![json!({ "status": 401, "body": "token expired" })]);
        let got = ok(door::port_probe(&h.ctx, "ws_1", 3000, None).await);
        assert_eq!(serde_json::to_value(&got).unwrap(), json!({ "status": 401, "body": "token expired" }));
        assert_eq!(
            h.base().names_of(&["entry_of", "copy_blocked", "kind_port_reach", "ws_port_reach", "probe", "ws_remint_port_reach"]),
            vec!["entry_of", "copy_blocked", "kind_port_reach", "ws_port_reach", "probe", "ws_remint_port_reach"]
        );
        assert_eq!(h.base().args_of("probe"), vec![json!({ "url": "https://m1-3000.preview.example/?pt_token=t1" })]);
        assert_eq!(h.base().args_of("ws_remint_port_reach"), vec![json!({ "id": "ws_1", "port": 3000 })]);

        // any other status leaves the route as it is
        for status in [200, 302, 403] {
            let h = H::new();
            held(&h, "ws_1", json!({}));
            h.base().answer_with("probe", vec![json!({ "status": status, "body": "<!doctype html>" })]);
            let got = ok(door::port_probe(&h.ctx, "ws_1", 3000, None).await);
            assert_eq!(serde_json::to_value(&got).unwrap(), json!({ "status": status, "body": "<!doctype html>" }));
            assert!(h.base().args_of("ws_remint_port_reach").is_empty(), "status {status}");
        }
    });
}

#[test]
fn port_probe_with_the_kinds_own_road_asks_it_standing_and_never_remints() {
    // runtime-reach.test.ts:1061
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("kind_port_reach", vec![json!({ "url": "http://127.0.0.1:7070/", "expiresAt": 77 })]);
        h.base().answer_with("probe", vec![json!({ "status": 401, "body": "no" })]);
        let got = ok(door::port_probe(&h.ctx, "ws_1", 5173, None).await);
        assert_eq!(serde_json::to_value(&got).unwrap(), json!({ "status": 401, "body": "no" }));
        assert_eq!(h.base().args_of("kind_port_reach"), vec![json!({ "id": "ws_1", "port": 5173, "standing": true })]);
        assert_eq!(h.base().args_of("probe"), vec![json!({ "url": "http://127.0.0.1:7070/" })]);
        assert!(h.base().args_of("ws_port_reach").is_empty());
        assert!(h.base().args_of("ws_remint_port_reach").is_empty());
        assert_eq!(h.base().names_of(&["entry_of", "copy_blocked"]), vec!["entry_of", "copy_blocked"]);
    });
}

#[test]
fn port_probe_of_a_route_that_cannot_be_fetched_rejects_and_remints_nothing() {
    // runtime-reach.test.ts:1162
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("probe", vec![err_answer("fetch failed", None)]);
        let e = refusal(door::port_probe(&h.ctx, "ws_1", 3000, None).await);
        said(&e, "fetch failed", None);
        assert!(h.base().args_of("ws_remint_port_reach").is_empty());

        let h = H::new();
        h.base().answer_with("entry_of", vec![err_answer("no such workspace ws_nobody", Some("not-found"))]);
        let e = refusal(door::port_probe(&h.ctx, "ws_nobody", 3000, None).await);
        said(&e, "no such workspace ws_nobody", Some(Kind::NotFound));
        assert_eq!(h.base().names(), vec!["entry_of"]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 12: origin_refusal

#[test]
fn origin_refusal_reads_the_live_record_after_ready() {
    // local-workspace.test.ts:1019
    run(async {
        let h = H::new();
        held(&h, "ws_1", json!({}));
        h.base().answer_with("refusal_for:ws_1", vec![json!("a request relayed from a computer cannot do this")]);
        let got = ok(door::origin_refusal(&h.ctx, "ws_1", Some(person())).await);
        assert_eq!(got.as_deref(), Some("a request relayed from a computer cannot do this"));
        assert_eq!(h.base().names(), vec!["ready", "refusal_for"]);
        assert_eq!(h.base().args_of("refusal_for")[0]["id"], json!("ws_1"));

        // a workspace nobody holds reads as no record, which the rule answers for itself
        let h = H::new();
        assert_eq!(ok(door::origin_refusal(&h.ctx, "ws_nobody", None).await), None);
        assert_eq!(h.base().args_of("refusal_for")[0]["id"], json!(null));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 14: seen_by

fn lead_scope() -> Option<Caller> {
    Some(thread("th_1", "ws_1", "th_root"))
}

#[test]
fn seen_by_shows_a_thread_its_tree_only() {
    // serve.test.ts:1326, box-lead.test.ts:276, :378
    let h = H::new();
    held(&h, "ws_1", json!({}));
    held(&h, "ws_2", json!({}));
    h.base().answer_with("refusal_for:ws_2", vec![json!("a thread of another tree")]);
    let scope = lead_scope();
    let doctor = json!({ "type": "doctor.line", "doctorId": "d_1", "line": "spoo answers", "stream": "out" });

    // the person sees everything and asks nothing
    assert!(door::seen_by(&h.ctx, &doctor, None));
    assert!(door::seen_by(&h.ctx, &doctor, Some(&person())));
    assert!(h.base().names().is_empty());

    // a thread: an event that names no workspace, or one no record holds, is hidden
    assert!(!door::seen_by(&h.ctx, &doctor, scope.as_ref()));
    assert!(!door::seen_by(&h.ctx, &json!({ "type": "workspace.review", "workspaceId": "ws_nobody" }), scope.as_ref()));

    // an event on behalf of a thread reads the asker alone
    let asked = |t: &str, r: &str| json!({ "type": "workspace.creating", "workspaceId": "ws_nobody", "askedBy": { "threadId": t, "rootThreadId": r } });
    assert!(door::seen_by(&h.ctx, &asked("th_1", "th_other"), scope.as_ref()));
    assert!(door::seen_by(&h.ctx, &asked("th_2", "th_root"), scope.as_ref()));
    assert!(!door::seen_by(&h.ctx, &asked("th_2", "th_other"), scope.as_ref()));

    // a workspace event reads the workspace's own rule, named by the event, its workspace or its status
    assert!(door::seen_by(&h.ctx, &json!({ "type": "workspace.review", "workspaceId": "ws_1" }), scope.as_ref()));
    assert!(!door::seen_by(&h.ctx, &json!({ "type": "workspace.review", "workspaceId": "ws_2" }), scope.as_ref()));
    assert!(!door::seen_by(&h.ctx, &json!({ "type": "workspace.created", "workspace": { "id": "ws_2" } }), scope.as_ref()));
    assert!(door::seen_by(&h.ctx, &json!({ "type": "workspace.status", "status": { "id": "ws_1" } }), scope.as_ref()));
    let ruled: Vec<Value> = h.base().args_of("refusal_for").iter().map(|a| a["id"].clone()).collect();
    assert_eq!(ruled, vec![json!("ws_1"), json!("ws_2"), json!("ws_2"), json!("ws_1")]);
    assert!(h.base().args_of("reaches_row").is_empty());
}

#[test]
fn seen_by_reads_an_events_thread_row() {
    // box-lead.test.ts:276
    let h = H::new();
    held(&h, "ws_1", json!({}));
    held(&h, "ws_cloud", json!({}));
    h.base().answer_with("reaches_row:th_theirs", vec![json!(false)]);
    h.base().answer_with("refusal_for", vec![json!("not this tree's")]);
    let scope = lead_scope();
    let on_cloud = |t: &str| json!({ "type": "session.done", "workspaceId": "ws_cloud", "threadId": t, "turnId": "t" });
    assert!(door::seen_by(&h.ctx, &on_cloud("th_mine"), scope.as_ref()));
    assert!(!door::seen_by(&h.ctx, &on_cloud("th_theirs"), scope.as_ref()));
    let rows: Vec<Value> = h.base().args_of("reaches_row").iter().map(|a| json!([a["thread"], a["workspace"]])).collect();
    assert_eq!(rows, vec![json!(["th_mine", "ws_cloud"]), json!(["th_theirs", "ws_cloud"])]);
    // the thread row decides, so the workspace's own rule is never read for it
    assert!(h.base().args_of("refusal_for").is_empty());
    // an event with no thread string falls back to the workspace's rule
    assert!(!door::seen_by(&h.ctx, &json!({ "type": "workspace.review", "workspaceId": "ws_cloud", "threadId": 4 }), scope.as_ref()));
    assert_eq!(h.base().args_of("refusal_for").len(), 1);
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 15: shown_to

fn status_event(children: &[&str]) -> Value {
    let kids: Vec<Value> = children.iter().map(|c| json!({ "workspaceId": c, "name": c })).collect();
    json!({ "type": "workspace.status", "seq": 4, "status": { "id": "ws_1", "name": "lead", "phase": "running",
        "tree": { "lead": "ws_1", "children": kids } } })
}

#[test]
fn shown_to_prunes_children_the_thread_may_not_see() {
    let h = H::new();
    held(&h, "ws_1", json!({}));
    held(&h, "ws_a", json!({}));
    held(&h, "ws_b", json!({}));
    h.base().answer_with("refusal_for:ws_b", vec![json!("a thread of another tree")]);
    let scope = lead_scope();

    // the person, and an event that is no status tree: as given, asking nothing
    let event = status_event(&["ws_a", "ws_b"]);
    assert_eq!(door::shown_to(&h.ctx, event.clone(), None), event);
    assert_eq!(door::shown_to(&h.ctx, event.clone(), Some(&person())), event);
    let other = json!({ "type": "workspace.review", "workspaceId": "ws_1" });
    assert_eq!(door::shown_to(&h.ctx, other.clone(), scope.as_ref()), other);
    let bare = json!({ "type": "workspace.status", "status": { "id": "ws_1", "phase": "running" } });
    assert_eq!(door::shown_to(&h.ctx, bare.clone(), scope.as_ref()), bare);
    assert!(h.base().args_of("refusal_for").is_empty());

    // every child kept: the event as given
    let all = status_event(&["ws_a"]);
    assert_eq!(door::shown_to(&h.ctx, all.clone(), scope.as_ref()), all);

    // one refused and one with no live record: pruned, tree last among the status keys, the others in place
    let got = door::shown_to(&h.ctx, status_event(&["ws_a", "ws_b", "ws_gone"]), scope.as_ref());
    let want = json!({ "type": "workspace.status", "seq": 4, "status": { "id": "ws_1", "name": "lead", "phase": "running",
        "tree": { "lead": "ws_1", "children": [{ "workspaceId": "ws_a", "name": "ws_a" }] } } });
    assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&want).unwrap());

    // none kept: the status without its tree
    let got = door::shown_to(&h.ctx, status_event(&["ws_b", "ws_gone"]), scope.as_ref());
    let want = json!({ "type": "workspace.status", "seq": 4, "status": { "id": "ws_1", "name": "lead", "phase": "running" } });
    assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&want).unwrap());

    let ruled: Vec<Value> = h.base().args_of("refusal_for").iter().map(|a| a["id"].clone()).collect();
    assert_eq!(ruled, vec![json!("ws_a"), json!("ws_a"), json!("ws_b"), json!("ws_b")]);
}
