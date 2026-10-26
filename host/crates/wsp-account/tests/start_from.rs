// SPDX-License-Identifier: AGPL-3.0-only
//! The pieces `wsp start <url>` and `wsp review` are made of: the project a repository names, the issue and the pull
//! request read here, where the work came from, the folder or workspace the thread runs in, the picks checked first,
//! the thread opened, and a reviewer's reply read into a draft. Ported from the start and review cases of
//! packages/runtime/test (start-review, folder-threads, branch-of-record, pull-request) at 65f6b89c7, ported at the
//! area's seam over `FakeStart`. Every sentence is the TS protocol source's own text (START_WORDS, PR_BEHIND_WORDS,
//! placeBranchLine, noBranchesLine, reviewReaskPrompt), written out literally.
#[path = "support/base.rs"]
mod base;
#[path = "support/start.rs"]
mod start;

use std::rc::Rc;

use base::{err_answer, person, project, rec_json, record, run, FakeBase};
use serde_json::{json, Value};
use start::{catalog_json, issue, issue_json, origin_text, pr, pr_json, session_json, FakeStart, HEAD};
use wsp_account::projects::start_from::{from_of, start_from_area, OpenAsk, Placed, StartFrom, StartFromCtx, StartKind};
use wsp_account::projects::workspaces::seam::Entry;
use wsp_core::ids::Ids;
use wsp_core::types::api::StartAs;
use wsp_core::types::harness::StartPicksAsked;
use wsp_wire::types::start::{WorkspaceFrom, WorkspaceFromKind};
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::workspace::ProjectView;
use wsp_words::exit::{HostError, Kind};

const REPO: &str = "dev/spoo";
const REMOTE: &str = "https://github.com/dev/spoo.git";
const TOP: &str = "/home/me/spoo";
const WT: &str = "/home/me/wt/feat-pr";
const CLONE_MS: u64 = 300_000;
const NAME: &str = "#7 Rename the status word";
const UUID_1: &str = "00000000-0000-4000-8000-000000000001";
const UUID_2: &str = "00000000-0000-4000-8000-000000000002";

struct H {
    fake: Rc<FakeStart>,
    area: Rc<StartFrom>,
}

impl H {
    fn new() -> H {
        let fake = FakeStart::new();
        let ctx: Rc<dyn StartFromCtx> = fake.clone();
        let area = start_from_area(ctx);
        H { fake, area }
    }
    fn base(&self) -> &Rc<FakeBase> {
        &self.fake.base
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

/// A project on this computer, a git repo at TOP whose remote is the TS tests' own.
fn here_project() -> ProjectView {
    project("pr_1", "spoo", "here", json!({ "remote": REMOTE, "path": TOP, "git": { "top": TOP } }))
}

/// A project on a provider: a copy of nothing on this computer.
fn cloud_project() -> ProjectView {
    project("pr_1", "spoo", "solari", json!({ "remote": REMOTE }))
}

fn from_pr(kind: &str, fact: &Value) -> WorkspaceFrom {
    let mut head = json!({ "branch": fact["branch"], "oid": fact["headOid"] });
    if let Some(f) = fact.get("fork") {
        head["fork"] = f.clone();
    }
    serde_json::from_value(json!({ "kind": kind, "repo": REPO, "number": fact["number"], "url": fact["url"],
        "title": "Rename the status word", "base": fact["base"], "head": head }))
    .unwrap()
}

fn from_issue() -> WorkspaceFrom {
    serde_json::from_value(json!({ "kind": "issue", "repo": REPO, "number": 5, "url": "https://github.com/dev/spoo/issues/5",
        "title": "Add a greeting line" }))
    .unwrap()
}

fn ex(code: i64, stdout: &str, stderr: &str) -> Value {
    json!({ "exitCode": code, "stdout": stdout, "stderr": stderr })
}

fn git_call(cwd: &str, args: &[&str], ms: Option<u64>) -> Value {
    json!({ "cwd": cwd, "args": args, "timeoutMs": ms, "input": null })
}

fn entry_pr(e: &Entry) -> Value {
    serde_json::to_value(e.borrow().pr.as_ref().expect("the entry holds the pull request")).unwrap()
}

/// Nothing was created, taken back, persisted or emitted.
fn nothing_made(b: &FakeBase) {
    assert!(b.args_of("create_workspace").is_empty());
    assert!(b.args_of("delete_workspace").is_empty());
    assert!(b.persisted.borrow().is_empty());
    assert!(b.events.borrow().is_empty());
}

fn held_entry(h: &H, id: &str, over: Value) -> Entry {
    h.base().hold(record(id, "local", "pr_1", over))
}

fn prefs() -> wsp_wire::views::preferences::Preferences {
    serde_json::from_value(json!({"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}))
        .expect("preferences read")
}

fn pr_record_json(fact: &Value) -> Value {
    json!({ "number": fact["number"], "url": fact["url"], "state": fact["state"], "base": fact["base"] })
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 1: project_by_repo (start-review.test.ts:193)

#[test]
fn project_by_repo_prefers_this_computers_project() {
    let h = H::new();
    h.base().hold_project(project("pr_a", "spoo-box", "pl_box", json!({ "remote": REMOTE })));
    h.base().hold_project(project("pr_b", "spoo", "here", json!({ "remote": REMOTE })));
    h.base().hold_project(project("pr_c", "docs", "here", json!({ "remote": "https://github.com/dev/docs.git" })));
    assert_eq!(ok(h.area.project_by_repo(REPO, None)).id, "pr_b");
    let h = H::new();
    h.base().hold_project(project("pr_a", "spoo-box", "pl_box", json!({ "remote": REMOTE })));
    h.base().hold_project(project("pr_c", "docs", "here", json!({ "remote": "https://github.com/dev/docs.git" })));
    assert_eq!(ok(h.area.project_by_repo(REPO, None)).id, "pr_a");
}

#[test]
fn project_by_repo_with_a_name_takes_that_project_only() {
    let h = H::new();
    h.base().hold_project(project("pr_a", "spoo-box", "pl_box", json!({ "remote": REMOTE })));
    h.base().hold_project(project("pr_b", "spoo", "here", json!({ "remote": REMOTE })));
    h.base().hold_project(project("pr_c", "docs", "here", json!({ "remote": "https://github.com/dev/docs.git" })));
    assert_eq!(ok(h.area.project_by_repo(REPO, Some("spoo-box"))).id, "pr_a");
    assert_eq!(ok(h.area.project_by_repo(REPO, Some("pr_b"))).id, "pr_b");
    let e = refusal(h.area.project_by_repo(REPO, Some("docs")));
    said(&e, "no project here is a checkout of dev/spoo; add one with wsp add https://github.com/dev/spoo", Some(Kind::Invalid));
    let e = refusal(h.area.project_by_repo(REPO, Some("nobody")));
    said(&e, "no project here is a checkout of dev/spoo; add one with wsp add https://github.com/dev/spoo", Some(Kind::Invalid));
}

#[test]
fn project_by_repo_with_no_project_is_invalid() {
    let h = H::new();
    h.base().hold_project(here_project());
    let e = refusal(h.area.project_by_repo("someone/else", None));
    said(&e, "no project here is a checkout of someone/else; add one with wsp add https://github.com/someone/else", Some(Kind::Invalid));
    assert_eq!(e.fix, None);
    assert!(h.base().asks().is_empty());
    nothing_made(h.base());
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 2: issue_of, pull_request_of

#[test]
fn pull_request_of_refuses_an_issue_number() {
    run(async {
        let h = H::new();
        h.base().answer_with("ask:git.prRead", vec![json!({ "seen": "etag" })]);
        let e = refusal(h.area.pull_request_of(REMOTE, 5).await);
        said(&e, "#5 is not a pull request", None);
        assert_eq!(h.base().asks(), vec![json!({ "op": "git.prRead", "cwd": "/home/me", "remote": REMOTE, "number": 5 })]);
        assert_eq!(h.base().names_of(&["on_this_computer"]), vec!["on_this_computer"]);
    });
}

// start-review.test.ts:203 (the read the start makes of the pull request)
#[test]
fn pull_request_of_answers_the_pull_request_read_on_this_computer() {
    run(async {
        let h = H::new();
        let fact = pr_json(REPO, 7, json!({}));
        h.base().answer_with("ask:git.prRead", vec![json!({ "pr": fact })]);
        let got = ok(h.area.pull_request_of(REMOTE, 7).await);
        assert_eq!(serde_json::to_value(&got).unwrap(), fact);
        assert_eq!(h.base().asks(), vec![json!({ "op": "git.prRead", "cwd": "/home/me", "remote": REMOTE, "number": 7 })]);
    });
}

#[test]
fn issue_of_asks_git_issue_read_at_home() {
    run(async {
        let h = H::new();
        let read = issue_json(REPO, 5, "Add a greeting line to notes.txt");
        h.base().answer_with("ask:git.issueRead", vec![json!({ "issue": read })]);
        let got = ok(h.area.issue_of(REMOTE, 5).await);
        assert_eq!(serde_json::to_value(&got).unwrap(), read);
        assert_eq!(h.base().asks(), vec![json!({ "op": "git.issueRead", "cwd": "/home/me", "remote": REMOTE, "number": 5 })]);
        assert_eq!(h.base().names_of(&["on_this_computer", "with_daemon"]), vec!["on_this_computer"]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 3: from_of

#[test]
fn from_of_writes_its_keys_in_the_ts_order_with_and_without_a_fork() {
    let read = issue(REPO, 7, "Rename the status word");
    let url = "https://github.com/dev/spoo/issues/7";
    let plain = pr(REPO, 7, json!({}));
    let got = from_of(WorkspaceFromKind::PullRequest, REPO, &read, Some(&plain));
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        format!(
            "{{\"kind\":\"pull_request\",\"repo\":\"dev/spoo\",\"number\":7,\"url\":\"{url}\",\"title\":\"Rename the status word\",\"base\":\"main\",\"head\":{{\"branch\":\"lab/review-me\",\"oid\":\"{HEAD}\"}}}}"
        )
    );
    let forked = pr(REPO, 7, json!({ "fork": { "owner": "ana", "pushable": false } }));
    let got = from_of(WorkspaceFromKind::Review, REPO, &read, Some(&forked));
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        format!(
            "{{\"kind\":\"review\",\"repo\":\"dev/spoo\",\"number\":7,\"url\":\"{url}\",\"title\":\"Rename the status word\",\"base\":\"main\",\"head\":{{\"branch\":\"lab/review-me\",\"oid\":\"{HEAD}\",\"fork\":{{\"owner\":\"ana\",\"pushable\":false}}}}}}"
        )
    );
    let got = from_of(WorkspaceFromKind::Issue, REPO, &read, None);
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        format!("{{\"kind\":\"issue\",\"repo\":\"dev/spoo\",\"number\":7,\"url\":\"{url}\",\"title\":\"Rename the status word\"}}")
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 4: workspace_from

#[test]
fn workspace_from_on_a_provider_brokered_picks_the_thread_first_and_names_the_branch_of_record() {
    // branch-of-record.test.ts:236, :549
    run(async {
        let h = H::new();
        let fact = pr_json(REPO, 7, json!({}));
        let bor = json!({ "repoId": 42, "owner": "dev", "name": "spoo", "branch": "lab/review-me", "startSha": HEAD });
        h.base().answer_with("brokered_start", vec![json!(true)]);
        h.base().answer_with("landing_place", vec![json!("pl_boat")]);
        h.base().answer_with("pull_request_of_record", vec![bor.clone()]);
        let from = from_pr("pull_request", &fact);
        let placed =
            ok(h.area.workspace_from(&cloud_project(), from, Some(pr(REPO, 7, json!({}))), StartKind::Start, Some(person())).await);
        assert_eq!(placed.r#as, Some(StartAs { thread_id: UUID_1.to_owned(), turn_id: UUID_2.to_owned() }));
        assert_eq!(h.base().args_of("brokered_start"), vec![json!({ "provider": true })]);
        assert_eq!(
            h.base().args_of("pull_request_of_record"),
            vec![json!({ "remote": REMOTE, "number": 7, "project": "spoo", "computer": "boat", "start": true })]
        );
        assert_eq!(
            h.base().args_of("create_workspace"),
            vec![
                json!({ "project": "pr_1", "name": NAME, "branchOfRecord": bor, "opensThread": UUID_1, "origin": origin_text(&Some(person())) })
            ]
        );
        assert!(h.base().asks().is_empty());
        assert_eq!(
            h.base().names_of(&["pull_request_of_record", "create_workspace", "persist", "with_daemon", "delete_workspace"]),
            vec!["pull_request_of_record", "create_workspace", "persist"]
        );
        assert_eq!(placed.entry.borrow().record.borrow().id, "ws_made");
    });
}

#[test]
fn workspace_from_brokered_with_no_branch_of_record_still_checks_the_head_out() {
    // branch-of-record.test.ts:607 (the broker not ready)
    run(async {
        let h = H::new();
        let fact = pr_json(REPO, 7, json!({}));
        h.base().answer_with("brokered_start", vec![json!(true)]);
        h.base().answer_with("pull_request_of_record", vec![json!(null)]);
        h.base().answer_with("ask:git.prCheckout", vec![json!({ "branch": "lab/review-me" })]);
        let placed = ok(h
            .area
            .workspace_from(&cloud_project(), from_pr("pull_request", &fact), Some(pr(REPO, 7, json!({}))), StartKind::Start, None)
            .await);
        assert_eq!(placed.r#as, Some(StartAs { thread_id: UUID_1.to_owned(), turn_id: UUID_2.to_owned() }));
        assert_eq!(
            h.base().args_of("create_workspace"),
            vec![json!({ "project": "pr_1", "name": NAME, "opensThread": UUID_1, "origin": origin_text(&None) })]
        );
        assert_eq!(h.base().asks(), vec![json!({ "op": "git.prCheckout", "cwd": "/checkouts/ws_made", "number": 7 })]);
    });
}

#[test]
fn workspace_from_puts_from_and_pr_on_the_record_before_persist() {
    // start-review.test.ts:203
    run(async {
        let h = H::new();
        let fact = pr_json(REPO, 7, json!({}));
        h.base().answer_with("ask:git.prCheckout", vec![json!({ "branch": "lab/review-me" })]);
        let from = from_pr("pull_request", &fact);
        let placed =
            ok(h.area.workspace_from(&cloud_project(), from.clone(), Some(pr(REPO, 7, json!({}))), StartKind::Start, Some(person())).await);
        assert_eq!(placed.r#as, None);
        assert_eq!(placed.behind, None);
        assert_eq!(placed.setup, None);
        assert_eq!(
            h.base().names_of(&["create_workspace", "persist", "with_daemon", "delete_workspace"]),
            vec!["create_workspace", "persist", "with_daemon"]
        );
        assert_eq!(
            h.base().args_of("create_workspace"),
            vec![json!({ "project": "pr_1", "name": NAME, "origin": origin_text(&Some(person())) })]
        );
        let want = record("ws_made", "cloud", "pr_1", json!({ "name": NAME, "from": from, "base": "main", "pr": pr_record_json(&fact) }));
        assert_eq!(h.base().persisted.borrow().len(), 1);
        assert_eq!(serde_json::to_value(&h.base().persisted.borrow()[0]).unwrap(), serde_json::to_value(&want).unwrap());
        assert_eq!(rec_json(&placed.entry), serde_json::to_value(&want).unwrap());
        let mut seen = fact.clone();
        seen["readAt"] = json!(1_000_000);
        assert_eq!(entry_pr(&placed.entry), seen);
        assert_eq!(h.base().asks(), vec![json!({ "op": "git.prCheckout", "cwd": "/checkouts/ws_made", "number": 7 })]);
        assert!(h.base().events.borrow().is_empty());
    });
}

#[test]
fn a_non_brokered_start_makes_no_uuids_and_no_branch_of_record() {
    // spec edge case: as is absent from both the create and the session start
    run(async {
        let h = H::new();
        h.base().answer_with("ask:git.prCheckout", vec![json!({ "branch": "lab/review-me" })]);
        let fact = pr_json(REPO, 7, json!({}));
        let placed = ok(h
            .area
            .workspace_from(&cloud_project(), from_pr("pull_request", &fact), Some(pr(REPO, 7, json!({}))), StartKind::Start, None)
            .await);
        assert_eq!(placed.r#as, None);
        assert_eq!(h.base().args_of("brokered_start"), vec![json!({ "provider": true })]);
        assert!(h.base().args_of("pull_request_of_record").is_empty());
        assert_eq!(h.base().ids.uuid(), UUID_1);
    });
}

#[test]
fn workspace_from_for_an_issue_records_only_where_it_came_from_and_checks_nothing_out() {
    // start-review.test.ts:174
    run(async {
        let h = H::new();
        let placed = ok(h.area.workspace_from(&cloud_project(), from_issue(), None, StartKind::Start, None).await);
        let name = "#5 Add a greeting line";
        assert_eq!(h.base().args_of("create_workspace"), vec![json!({ "project": "pr_1", "name": name, "origin": origin_text(&None) })]);
        let want = record("ws_made", "cloud", "pr_1", json!({ "name": name, "from": from_issue() }));
        assert_eq!(serde_json::to_value(&h.base().persisted.borrow()[0]).unwrap(), serde_json::to_value(&want).unwrap());
        assert!(placed.entry.borrow().pr.is_none());
        assert!(h.base().asks().is_empty());
    });
}

#[test]
fn workspace_from_for_a_review_names_the_workspace_review_and_asks_the_record_as_no_start() {
    // start-review.test.ts:241 (the name), branch-of-record.test.ts:236 (start false for a review)
    run(async {
        let h = H::new();
        let fact = pr_json(REPO, 7, json!({}));
        h.base().answer_with("brokered_start", vec![json!(true)]);
        h.base().answer_with("pull_request_of_record", vec![json!(null)]);
        h.base().answer_with("ask:git.prCheckout", vec![json!({ "branch": "lab/review-me" })]);
        ok(h.area.workspace_from(&cloud_project(), from_pr("review", &fact), Some(pr(REPO, 7, json!({}))), StartKind::Review, None).await);
        assert_eq!(h.base().args_of("pull_request_of_record")[0]["start"], json!(false));
        assert_eq!(h.base().args_of("create_workspace")[0]["name"], json!("Review #7 Rename the status word"));
    });
}

#[test]
fn two_live_records_named_alike_take_the_next_number() {
    run(async {
        let h = H::new();
        h.base().hold(record("ws_a", "cloud", "pr_1", json!({ "name": NAME })));
        h.base().hold(record("ws_b", "cloud", "pr_1", json!({ "name": format!("{NAME} 2"), "phase": "gone" })));
        h.base().answer_with("ask:git.prCheckout", vec![json!({ "branch": "lab/review-me" })]);
        let fact = pr_json(REPO, 7, json!({}));
        ok(h.area
            .workspace_from(&cloud_project(), from_pr("pull_request", &fact), Some(pr(REPO, 7, json!({}))), StartKind::Start, None)
            .await);
        assert_eq!(h.base().args_of("create_workspace")[0]["name"], json!(format!("{NAME} 3")));
    });
}

#[test]
fn a_refused_checkout_takes_the_workspace_back_and_answers_its_own_refusal() {
    // start-review.test.ts:220
    run(async {
        let h = H::new();
        let sentence =
            "no signed-in command line for github.com is on this computer; the branch is pushed and the pull request waits for one";
        h.base().answer_with("ask:git.prCheckout", vec![err_answer(sentence, None)]);
        let fact = pr_json(REPO, 7, json!({}));
        let e = refusal(
            h.area
                .workspace_from(
                    &cloud_project(),
                    from_pr("pull_request", &fact),
                    Some(pr(REPO, 7, json!({}))),
                    StartKind::Start,
                    Some(person()),
                )
                .await,
        );
        said(&e, sentence, None);
        assert_eq!(
            h.base().names_of(&["create_workspace", "persist", "with_daemon", "delete_workspace"]),
            vec!["create_workspace", "persist", "with_daemon", "delete_workspace"]
        );
        assert_eq!(h.base().args_of("delete_workspace"), vec![json!({ "id": "ws_made", "origin": origin_text(&Some(person())) })]);
    });
}

#[test]
fn a_refused_checkout_whose_delete_fails_warns_and_still_refuses() {
    run(async {
        let h = H::new();
        let warned = wsp_core::console::capture();
        h.base().answer_with("ask:git.prCheckout", vec![err_answer("the checkout was refused", Some("conflict"))]);
        h.base().answer_with("delete_workspace", vec![err_answer("the disk is busy", None)]);
        let fact = pr_json(REPO, 7, json!({}));
        let e = refusal(
            h.area
                .workspace_from(&cloud_project(), from_pr("pull_request", &fact), Some(pr(REPO, 7, json!({}))), StartKind::Start, None)
                .await,
        );
        said(&e, "the checkout was refused", Some(Kind::Conflict));
        assert_eq!(h.base().args_of("delete_workspace").len(), 1);
        assert_eq!(warned.lines(), vec![format!("{NAME} was not taken back after its checkout failed: the disk is busy")]);
    });
}

#[test]
fn a_joined_box_refuses_a_pull_request_and_runs_an_issue_in_the_folder() {
    run(async {
        let h = H::new();
        let boxed = project("pr_1", "spoo", "pl_box", json!({ "remote": REMOTE }));
        let fact = pr_json(REPO, 7, json!({}));
        let e = refusal(
            h.area.workspace_from(&boxed, from_pr("pull_request", &fact), Some(pr(REPO, 7, json!({}))), StartKind::Start, None).await,
        );
        said(
            &e,
            "wsp makes no worktrees on box yet; start the thread without a branch, or make one inside the project folder with git worktree add and name it as the folder",
            Some(Kind::Usage),
        );
        nothing_made(h.base());
        assert!(h.base().asks().is_empty());
        assert!(h.base().args_of("project_folder").is_empty());
        let placed = ok(h.area.workspace_from(&boxed, from_issue(), None, StartKind::Start, None).await);
        assert_eq!(placed.entry.borrow().record.borrow().id, "ws_folder");
        assert_eq!((placed.behind, placed.setup, placed.r#as), (None, None, None));
        assert_eq!(h.base().args_of("project_folder"), vec![json!({ "project": "pr_1" })]);
        nothing_made(h.base());
    });
}

#[test]
fn a_made_workspace_not_found_after_is_refused() {
    run(async {
        let h = H::new();
        h.base().answer_with("create_workspace", vec![json!({ "id": "ws_made", "live": false })]);
        let fact = pr_json(REPO, 7, json!({}));
        let e = refusal(
            h.area
                .workspace_from(&cloud_project(), from_pr("pull_request", &fact), Some(pr(REPO, 7, json!({}))), StartKind::Start, None)
                .await,
        );
        said(&e, &format!("{NAME} was made and then not found"), None);
        assert!(h.base().persisted.borrow().is_empty());
        assert!(h.base().events.borrow().is_empty());
        assert!(h.base().asks().is_empty());
        assert!(h.base().args_of("delete_workspace").is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 5, 6, 7: folder_from, fetch_head, remote_holds

#[test]
fn folder_from_fetches_a_branch_no_worktree_holds_then_records_the_pr_on_the_worktree() {
    // folder-threads.test.ts:1333
    run(async {
        let h = H::new();
        let fact = pr_json(REPO, 7, json!({ "branch": "feat/pr" }));
        let from = from_pr("pull_request", &fact);
        h.base().answer_with("ask:git.fetchBranch", vec![json!({ "branch": "feat/pr", "oid": HEAD })]);
        let entry = held_entry(&h, "ws_wt", json!({ "worktree": { "path": WT, "branch": "feat/pr", "made": true } }));
        let placed = ok(h.area.folder_from(&here_project(), from.clone(), Some(pr(REPO, 7, json!({ "branch": "feat/pr" })))).await);
        assert!(Rc::ptr_eq(&placed.entry, &entry));
        assert_eq!((placed.behind.clone(), placed.setup.clone()), (None, None));
        assert_eq!(
            h.base().names_of(&[
                "worktrees_of",
                "project_folder",
                "write_daemon_roots",
                "with_daemon",
                "worktree_folder",
                "take_setup_line",
                "persist"
            ]),
            vec!["worktrees_of", "project_folder", "write_daemon_roots", "with_daemon", "worktree_folder", "take_setup_line", "persist"]
        );
        assert_eq!(h.base().args_of("write_daemon_roots"), vec![json!({ "id": "ws_folder", "strict": false })]);
        assert_eq!(
            h.base().asks(),
            vec![json!({ "op": "git.fetchBranch", "cwd": TOP, "remote": REMOTE, "branch": "feat/pr", "into": "feat/pr" })]
        );
        assert!(h.base().args_of("git").is_empty());
        assert_eq!(h.base().args_of("worktree_folder"), vec![json!({ "project": "pr_1", "top": TOP, "branch": "feat/pr" })]);
        let want = record(
            "ws_wt",
            "local",
            "pr_1",
            json!({ "worktree": { "path": WT, "branch": "feat/pr", "made": true }, "from": from, "base": "main", "pr": pr_record_json(&fact) }),
        );
        assert_eq!(h.base().persisted.borrow().len(), 1);
        assert_eq!(serde_json::to_value(&h.base().persisted.borrow()[0]).unwrap(), serde_json::to_value(&want).unwrap());
        let mut seen = fact.clone();
        seen["readAt"] = json!(1_000_000);
        assert_eq!(entry_pr(&entry), seen);
    });
}

#[test]
fn folder_from_fetches_a_forks_head_by_pull_ref_into_owner_slash_branch() {
    // pull-request.test.ts:513
    run(async {
        let h = H::new();
        let over = json!({ "branch": "feat/pr", "fork": { "owner": "ana", "pushable": true } });
        let fact = pr_json(REPO, 7, over.clone());
        held_entry(&h, "ws_wt", json!({ "worktree": { "path": WT, "branch": "ana/feat/pr", "made": true } }));
        ok(h.area.folder_from(&here_project(), from_pr("pull_request", &fact), Some(pr(REPO, 7, over))).await);
        assert!(h.base().asks().is_empty());
        assert!(h.base().args_of("write_daemon_roots").is_empty());
        assert_eq!(
            h.base().args_of("git"),
            vec![git_call(TOP, &["fetch", "--quiet", "--no-tags", REMOTE, "pull/7/head:refs/heads/ana/feat/pr"], Some(CLONE_MS))]
        );
        assert_eq!(h.base().args_of("worktree_folder"), vec![json!({ "project": "pr_1", "top": TOP, "branch": "ana/feat/pr" })]);
        let persisted = h.base().persisted.borrow();
        assert_eq!(persisted.len(), 1);
        assert_eq!(serde_json::to_value(&persisted[0]).unwrap()["from"]["head"]["fork"], json!({ "owner": "ana", "pushable": true }));
    });
}

#[test]
fn fetch_head_falls_back_to_the_pull_ref_when_the_branch_is_gone_at_the_remote() {
    // folder-threads.test.ts:1318
    run(async {
        let h = H::new();
        h.base().answer_with("ask:git.fetchBranch", vec![err_answer("couldn't find remote ref feat/pr", None)]);
        h.base().answer_with("git:ls-remote", vec![ex(2, "", "")]);
        ok(h.area.fetch_head(&here_project(), TOP, &pr(REPO, 7, json!({ "branch": "feat/pr" })), "feat/pr").await);
        assert_eq!(h.base().asks().len(), 1);
        assert_eq!(
            h.base().args_of("git"),
            vec![
                git_call(TOP, &["ls-remote", "--exit-code", REMOTE, "refs/heads/feat/pr"], Some(CLONE_MS)),
                git_call(TOP, &["fetch", "--quiet", "--no-tags", REMOTE, "pull/7/head:refs/heads/feat/pr"], Some(CLONE_MS)),
            ]
        );
    });
}

#[test]
fn fetch_head_keeps_the_daemon_refusal_when_the_remote_holds_the_branch() {
    run(async {
        let h = H::new();
        h.base().answer_with("ask:git.fetchBranch", vec![err_answer("feat/pr cannot be fast-forwarded", Some("conflict"))]);
        h.base().answer_with("git:ls-remote", vec![ex(0, "abc\trefs/heads/feat/pr\n", "")]);
        let e = refusal(h.area.fetch_head(&here_project(), TOP, &pr(REPO, 7, json!({ "branch": "feat/pr" })), "feat/pr").await);
        said(&e, "feat/pr cannot be fast-forwarded", Some(Kind::Conflict));
        assert_eq!(
            h.base().args_of("git"),
            vec![git_call(TOP, &["ls-remote", "--exit-code", REMOTE, "refs/heads/feat/pr"], Some(CLONE_MS))]
        );
    });
}

#[test]
fn fetch_head_refuses_with_what_git_said_when_the_pull_ref_fetch_fails() {
    // rule 6: no kind, gitSaid(result)
    run(async {
        let h = H::new();
        let fork = json!({ "branch": "feat/pr", "fork": { "owner": "ana", "pushable": true } });
        h.base().answer_with("git:fetch", vec![ex(128, "", "hint: nothing\nfatal: couldn't find remote ref pull/7/head\n")]);
        let e = refusal(h.area.fetch_head(&here_project(), TOP, &pr(REPO, 7, fork), "ana/feat/pr").await);
        said(&e, "fatal: couldn't find remote ref pull/7/head", None);
    });
}

#[test]
fn remote_holds_reads_exit_2_alone_as_no() {
    // rule 7
    run(async {
        for (code, holds) in [(0, true), (1, true), (128, true), (2, false)] {
            let h = H::new();
            h.base().answer_with("git:ls-remote", vec![ex(code, "", "")]);
            assert_eq!(ok(h.area.remote_holds(TOP, REMOTE, "refs/heads/feat/pr").await), holds, "exit {code}");
            assert_eq!(
                h.base().args_of("git"),
                vec![git_call(TOP, &["ls-remote", "--exit-code", REMOTE, "refs/heads/feat/pr"], Some(CLONE_MS))]
            );
        }
    });
}

#[test]
fn folder_from_on_the_project_folders_own_branch_writes_nothing() {
    // folder-threads.test.ts:884
    run(async {
        let h = H::new();
        let fact = pr_json(REPO, 7, json!({ "branch": "feat/pr" }));
        h.base().answer_with("worktrees_of", vec![json!([{ "path": TOP, "branch": "feat/pr" }])]);
        h.base().answer_with("git:rev-parse FETCH_HEAD", vec![ex(0, "1111\n", "")]);
        h.base().answer_with("git:rev-parse HEAD", vec![ex(0, "2222\n", "")]);
        h.base().answer_with("git:merge-base --is-ancestor 1111 2222", vec![ex(1, "", "")]);
        let entry = held_entry(&h, "ws_wt", json!({}));
        let before = rec_json(&entry);
        let placed = ok(h
            .area
            .folder_from(&here_project(), from_pr("pull_request", &fact), Some(pr(REPO, 7, json!({ "branch": "feat/pr" }))))
            .await);
        assert_eq!(
            placed.behind,
            Some(format!("this project folder is behind pull request #7, and wsp never moves your own checkout; run git pull --ff-only in {TOP} to update it"))
        );
        assert_eq!(placed.setup, None);
        assert!(h.base().persisted.borrow().is_empty());
        assert!(h.base().events.borrow().is_empty());
        assert_eq!(rec_json(&entry), before);
        assert!(entry.borrow().pr.is_none());
        assert!(h.base().asks().is_empty());
        assert_eq!(h.base().names_of(&["take_setup_line"]), vec!["take_setup_line"]);
    });
}

#[test]
fn folder_from_with_no_git_top_refuses_no_branches() {
    run(async {
        let h = H::new();
        let plain = project("pr_1", "spoo", "here", json!({ "remote": REMOTE }));
        let fact = pr_json(REPO, 7, json!({}));
        let e = refusal(h.area.folder_from(&plain, from_pr("pull_request", &fact), Some(pr(REPO, 7, json!({})))).await);
        said(&e, "spoo is not a git repo, so it has no branches", Some(Kind::Usage));
        assert!(h.base().args_of("worktrees_of").is_empty());
        assert!(h.base().args_of("git").is_empty());
        nothing_made(h.base());
    });
}

#[test]
fn folder_from_with_no_pull_request_answers_the_project_folder() {
    run(async {
        let h = H::new();
        let placed = ok(h.area.folder_from(&here_project(), from_issue(), None).await);
        assert_eq!(placed.entry.borrow().record.borrow().id, "ws_folder");
        assert_eq!((placed.behind, placed.setup, placed.r#as), (None, None, None));
        assert_eq!(h.base().names(), vec!["project_folder"]);
        assert!(h.base().persisted.borrow().is_empty());
    });
}

#[test]
fn folder_from_on_a_branch_a_worktree_holds_catches_it_up_and_records_the_pr() {
    // folder-threads.test.ts:844
    run(async {
        let h = H::new();
        let over = json!({ "branch": "feat/pr" });
        let fact = pr_json(REPO, 7, over.clone());
        h.base().answer_with("worktrees_of", vec![json!([{ "path": WT, "branch": "feat/pr" }])]);
        h.base().answer_with("git:rev-parse FETCH_HEAD", vec![ex(0, "1111\n", "")]);
        h.base().answer_with("git:rev-parse HEAD", vec![ex(0, "2222\n", "")]);
        h.base().answer_with("git:merge-base --is-ancestor 1111 2222", vec![ex(1, "", "")]);
        h.base().answer_with("ask:git.update", vec![json!({ "base": "feat/pr", "merged": true, "commits": 1, "conflicts": [] })]);
        let entry = held_entry(&h, "ws_wt", json!({ "worktree": { "path": WT, "branch": "feat/pr", "made": true } }));
        let placed = ok(h.area.folder_from(&here_project(), from_pr("pull_request", &fact), Some(pr(REPO, 7, over))).await);
        assert_eq!(placed.behind, None);
        assert_eq!(h.base().asks(), vec![json!({ "op": "git.update", "cwd": WT, "base": "feat/pr" })]);
        assert!(h.base().args_of("write_daemon_roots").iter().all(|a| a["id"] == json!("ws_wt")));
        assert_eq!(h.base().persisted.borrow().len(), 1);
        let mut seen = fact.clone();
        seen["readAt"] = json!(1_000_000);
        assert_eq!(entry_pr(&entry), seen);
    });
}

#[test]
fn folder_from_hands_the_thread_the_behind_line_of_a_worktree_with_uncommitted_files() {
    // folder-threads.test.ts:852, :873
    run(async {
        let h = H::new();
        let over = json!({ "branch": "feat/pr" });
        let fact = pr_json(REPO, 7, over.clone());
        h.base().answer_with("worktrees_of", vec![json!([{ "path": WT, "branch": "feat/pr" }])]);
        h.base().answer_with("git:rev-parse FETCH_HEAD", vec![ex(0, "1111\n", "")]);
        h.base().answer_with("git:rev-parse HEAD", vec![ex(0, "2222\n", "")]);
        h.base().answer_with("git:merge-base --is-ancestor 1111 2222", vec![ex(1, "", "")]);
        h.base().answer_with("git:status", vec![ex(0, " M draft.md\n", "")]);
        held_entry(&h, "ws_wt", json!({ "worktree": { "path": WT, "branch": "feat/pr", "made": true } }));
        let placed = ok(h.area.folder_from(&here_project(), from_pr("pull_request", &fact), Some(pr(REPO, 7, over))).await);
        assert_eq!(
            placed.behind,
            Some(format!("this worktree is behind pull request #7 and holds files no commit has, so it was not updated; commit them, then run git pull --ff-only in {WT}"))
        );
        assert!(h.base().asks().is_empty());
        assert_eq!(h.base().persisted.borrow().len(), 1);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 8: catch_up

struct Row {
    name: &'static str,
    fork: bool,
    worktree: Option<bool>,
    scripts: Vec<(&'static str, Value)>,
    git: Vec<Value>,
    asks: Vec<Value>,
    want: Option<String>,
}

#[test]
fn catch_up_answers_each_line_in_order() {
    let r = REMOTE;
    let ls = git_call(TOP, &["ls-remote", "--exit-code", r, "refs/heads/feat/pr"], Some(CLONE_MS));
    let fetch_name = git_call(TOP, &["fetch", "--quiet", "--no-tags", r, "refs/heads/feat/pr"], Some(CLONE_MS));
    let fetch_pull = git_call(TOP, &["fetch", "--quiet", "--no-tags", r, "pull/7/head"], Some(CLONE_MS));
    let rev_head = git_call(TOP, &["rev-parse", "FETCH_HEAD"], None);
    let rev_at = |p: &str| git_call(p, &["rev-parse", "HEAD"], None);
    let anc1 = |p: &str| git_call(p, &["merge-base", "--is-ancestor", "1111", "2222"], None);
    let anc2 = |p: &str| git_call(p, &["merge-base", "--is-ancestor", "2222", "1111"], None);
    let status = |p: &str| git_call(p, &["status", "--porcelain"], None);
    let behind = |p: &str| vec![ls.clone(), fetch_name.clone(), rev_head.clone(), rev_at(p), anc1(p), anc2(p), status(p)];
    let common = || {
        vec![
            ("git:rev-parse FETCH_HEAD", ex(0, "1111\n", "")),
            ("git:rev-parse HEAD", ex(0, "2222\n", "")),
            ("git:merge-base --is-ancestor 1111 2222", ex(1, "", "")),
            ("git:merge-base --is-ancestor 2222 1111", ex(0, "", "")),
        ]
    };
    let with = |mut base: Vec<(&'static str, Value)>, over: Vec<(&'static str, Value)>| {
        for (k, v) in over {
            base.retain(|(bk, _)| *bk != k);
            base.push((k, v));
        }
        base
    };
    let update_ok = json!({ "base": "feat/pr", "merged": true, "commits": 1, "conflicts": [] });
    let rows = vec![
        Row {
            name: "unread",
            fork: false,
            worktree: Some(true),
            scripts: vec![("git:fetch", ex(128, "", "hint: check the network\nfatal: could not read from remote\n"))],
            git: vec![ls.clone(), fetch_name.clone()],
            asks: vec![],
            want: Some("pull request #7 could not be read just now (fatal: could not read from remote), so this worktree may be behind it".to_owned()),
        },
        Row {
            name: "nothing, the heads are equal",
            fork: false,
            worktree: Some(true),
            scripts: with(common(), vec![("git:rev-parse HEAD", ex(0, "1111\n", ""))]),
            git: vec![ls.clone(), fetch_name.clone(), rev_head.clone(), rev_at(WT)],
            asks: vec![],
            want: None,
        },
        Row {
            name: "nothing, the worktree is ahead",
            fork: false,
            worktree: Some(true),
            scripts: with(common(), vec![("git:merge-base --is-ancestor 1111 2222", ex(0, "", ""))]),
            git: vec![ls.clone(), fetch_name.clone(), rev_head.clone(), rev_at(WT), anc1(WT)],
            asks: vec![],
            want: None,
        },
        Row {
            name: "diverged",
            fork: false,
            worktree: Some(true),
            scripts: with(common(), vec![("git:merge-base --is-ancestor 2222 1111", ex(1, "", ""))]),
            git: vec![ls.clone(), fetch_name.clone(), rev_head.clone(), rev_at(WT), anc1(WT), anc2(WT)],
            asks: vec![],
            want: Some(format!("this worktree's branch and pull request #7 have gone different ways, so it was not updated; run git pull --rebase in {WT} to bring them together")),
        },
        Row {
            name: "changed",
            fork: false,
            worktree: Some(true),
            scripts: with(common(), vec![("git:status --porcelain", ex(0, " M a.txt\n", ""))]),
            git: behind(WT),
            asks: vec![],
            want: Some(format!("this worktree is behind pull request #7 and holds files no commit has, so it was not updated; commit them, then run git pull --ff-only in {WT}")),
        },
        Row {
            name: "folder",
            fork: false,
            worktree: None,
            scripts: common(),
            git: behind(TOP),
            asks: vec![],
            want: Some(format!("this project folder is behind pull request #7, and wsp never moves your own checkout; run git pull --ff-only in {TOP} to update it")),
        },
        Row {
            name: "not made",
            fork: false,
            worktree: Some(false),
            scripts: common(),
            git: behind(WT),
            asks: vec![],
            want: Some(format!("this worktree is behind pull request #7, and wsp moves only worktrees it made; run git pull --ff-only in {WT} to update it")),
        },
        Row {
            name: "by head",
            fork: true,
            worktree: Some(true),
            scripts: common(),
            git: vec![fetch_pull.clone(), rev_head.clone(), rev_at(WT), anc1(WT), anc2(WT), status(WT)],
            asks: vec![],
            want: Some(format!("this worktree is behind pull request #7; run git pull --ff-only {r} pull/7/head in {WT} to update it")),
        },
        Row {
            name: "updated",
            fork: false,
            worktree: Some(true),
            scripts: with(common(), vec![("ask:git.update", update_ok.clone())]),
            git: behind(WT),
            asks: vec![json!({ "op": "git.update", "cwd": WT, "base": "feat/pr" })],
            want: None,
        },
        Row {
            name: "update refused",
            fork: false,
            worktree: Some(true),
            scripts: with(common(), vec![("ask:git.update", err_answer("conflicts in a.txt", None))]),
            git: behind(WT),
            asks: vec![json!({ "op": "git.update", "cwd": WT, "base": "feat/pr" })],
            want: Some("pull request #7 could not be read just now (conflicts in a.txt), so this worktree may be behind it".to_owned()),
        },
    ];
    run(async {
        for row in rows {
            let h = H::new();
            for (k, v) in &row.scripts {
                h.base().answer_with(k, vec![v.clone()]);
            }
            let path = if row.worktree.is_some() { WT } else { TOP };
            let over = if row.fork {
                json!({ "branch": "feat/pr", "fork": { "owner": "ana", "pushable": true } })
            } else {
                json!({ "branch": "feat/pr" })
            };
            let wt = match row.worktree {
                Some(made) => json!({ "worktree": { "path": WT, "branch": "feat/pr", "made": made } }),
                None => json!({}),
            };
            let entry = held_entry(&h, "ws_wt", wt);
            let got = ok(h.area.catch_up(&entry, &here_project(), TOP, &pr(REPO, 7, over)).await);
            assert_eq!(got, row.want, "{}", row.name);
            assert_eq!(h.base().args_of("git"), row.git, "{}: git runs, path {path}", row.name);
            assert_eq!(h.base().asks(), row.asks, "{}: daemon asks", row.name);
            if !row.asks.is_empty() {
                assert_eq!(
                    h.base().names_of(&["write_daemon_roots", "with_daemon"]),
                    vec!["write_daemon_roots", "with_daemon"],
                    "{}",
                    row.name
                );
                assert_eq!(h.base().args_of("write_daemon_roots"), vec![json!({ "id": "ws_wt", "strict": false })], "{}", row.name);
            }
            assert!(h.base().persisted.borrow().is_empty(), "{}", row.name);
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 9: picks_hold

fn open_picks() -> Value {
    json!({ "catalog": catalog_json("codex"), "openModel": "sonnet", "openEffort": "high" })
}

#[test]
fn picks_hold_refuses_an_access_word_the_agent_lacks() {
    run(async {
        let h = H::new();
        *h.base().prefs.borrow_mut() = Some(prefs());
        let sentence = "codex has no access word full; drop --access";
        h.base().answer_with("access_word_refusal", vec![err_answer(sentence, Some("usage"))]);
        let asked = StartPicksAsked { harness: Some("codex".into()), access: Some(AccessChoice::Full), ..Default::default() };
        let e = refusal(h.area.picks_hold(&here_project(), &asked).await);
        said(&e, sentence, Some(Kind::Usage));
        assert_eq!(h.base().args_of("access_word_refusal"), vec![json!({ "harness": "codex", "access": "full" })]);
        assert!(h.base().args_of("start_picks_refusal").is_empty());
        assert!(h.base().args_of("worktrees_of").is_empty());
        nothing_made(h.base());
    });
}

#[test]
fn picks_hold_turns_a_start_picks_throw_into_the_pick_refusal() {
    // folder-threads.test.ts:1327
    run(async {
        let h = H::new();
        *h.base().prefs.borrow_mut() = Some(prefs());
        let sentence = "claude has no model no-such-model";
        h.base().answer_with("start_picks_refusal", vec![err_answer(sentence, Some("invalid"))]);
        let asked = StartPicksAsked { harness: Some("claude".into()), model: Some("no-such-model".into()), ..Default::default() };
        let e = refusal(h.area.picks_hold(&here_project(), &asked).await);
        said(&e, sentence, Some(Kind::Invalid));
        assert_eq!(h.base().args_of("start_picks_refusal"), vec![json!({ "catalog": "claude", "picks": { "model": "no-such-model" } })]);
        assert_eq!(h.base().args_of("resolved_picks"), vec![json!({ "home": "ws_folder", "harness": "claude", "projectId": "pr_1" })]);
        assert!(h.base().args_of("worktrees_of").is_empty());
        assert!(h.base().args_of("git").is_empty());
        nothing_made(h.base());
    });
}

#[test]
fn picks_hold_asks_nothing_off_a_computer_that_does_not_copy() {
    run(async {
        let h = H::new();
        let asked = StartPicksAsked { harness: Some("codex".into()), model: Some("no-such-model".into()), ..Default::default() };
        ok(h.area.picks_hold(&cloud_project(), &asked).await);
        assert!(h.base().names().is_empty());
    });
}

#[test]
fn picks_hold_fills_model_and_effort_from_the_open_defaults_and_names_the_mode() {
    run(async {
        let h = H::new();
        *h.base().prefs.borrow_mut() = Some(prefs());
        h.base().answer_with("resolved_picks", vec![open_picks()]);
        h.base().answer_with("named_mode", vec![json!("acceptEdits")]);
        let asked =
            StartPicksAsked { harness: Some("codex".into()), access: Some(AccessChoice::AutoEdit), fast: Some(true), ..Default::default() };
        ok(h.area.picks_hold(&here_project(), &asked).await);
        assert_eq!(h.base().args_of("named_mode"), vec![json!({ "catalog": "codex", "harness": "codex", "access": "auto-edit" })]);
        assert_eq!(
            h.base().args_of("start_picks_refusal"),
            vec![
                json!({ "catalog": "codex", "picks": { "model": "sonnet", "effort": "high", "permissionMode": "acceptEdits", "fast": true } })
            ]
        );
        let h = H::new();
        *h.base().prefs.borrow_mut() = Some(prefs());
        h.base().answer_with("resolved_picks", vec![open_picks()]);
        let asked = StartPicksAsked {
            harness: Some("codex".into()),
            model: Some("opus".into()),
            permission_mode: Some("read-only".into()),
            ..Default::default()
        };
        ok(h.area.picks_hold(&here_project(), &asked).await);
        assert!(h.base().args_of("named_mode").is_empty());
        assert_eq!(
            h.base().args_of("start_picks_refusal"),
            vec![json!({ "catalog": "codex", "picks": { "model": "opus", "effort": "high", "permissionMode": "read-only" } })]
        );
    });
}

#[test]
fn picks_hold_with_no_table_for_the_agent_checks_nothing() {
    run(async {
        let h = H::new();
        *h.base().prefs.borrow_mut() = Some(prefs());
        h.base().answer_with("resolved_picks", vec![json!(null)]);
        let asked = StartPicksAsked { harness: Some("nobody".into()), access: Some(AccessChoice::Full), ..Default::default() };
        ok(h.area.picks_hold(&here_project(), &asked).await);
        assert!(h.base().args_of("access_word_refusal").is_empty());
        assert!(h.base().args_of("start_picks_refusal").is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 10: open_with

#[test]
fn open_with_starts_the_session_with_its_keys_in_order_and_answers_the_thread() {
    // start-review.test.ts:174 (the thread and the session answered)
    run(async {
        let h = H::new();
        let entry = held_entry(&h, "ws_placed", json!({ "name": "placed" }));
        let placed = Placed {
            entry: entry.clone(),
            behind: Some("this worktree is behind".to_owned()),
            setup: Some("npm ci runs before its threads start".to_owned()),
            r#as: Some(StartAs { thread_id: UUID_1.to_owned(), turn_id: UUID_2.to_owned() }),
        };
        let ask = OpenAsk {
            prompt: "look at it".to_owned(),
            harness: Some("codex".to_owned()),
            model: Some("opus".to_owned()),
            effort: Some("high".to_owned()),
            permission_mode: Some("read-only".to_owned()),
            access: Some(AccessChoice::Plan),
        };
        h.base().answer_with("start_session", vec![session_json("se_1", Some("th_1"))]);
        let opened = ok(h.area.open_with(&placed, ask, Some(person())).await);
        let started = h.base().args_of("start_session");
        assert_eq!(started.len(), 1);
        assert_eq!(started[0]["workspaceId"], json!("ws_placed"));
        assert_eq!(started[0]["origin"], origin_text(&Some(person())));
        assert_eq!(
            serde_json::to_string(&started[0]["opts"]).unwrap(),
            format!(
                "{{\"behind\":\"this worktree is behind\",\"setup\":\"npm ci runs before its threads start\",\"as\":{{\"threadId\":\"{UUID_1}\",\"turnId\":\"{UUID_2}\"}},\"prompt\":\"look at it\",\"harness\":\"codex\",\"model\":\"opus\",\"effort\":\"high\",\"permissionMode\":\"read-only\",\"access\":\"plan\"}}"
            )
        );
        assert_eq!((opened.thread_id.as_str(), opened.session_id.as_str()), ("th_1", "se_1"));
        assert_eq!(serde_json::to_value(&opened.workspace).unwrap()["id"], json!("ws_placed"));

        let h = H::new();
        let entry = held_entry(&h, "ws_placed", json!({ "name": "placed" }));
        let placed = Placed { entry, behind: None, setup: None, r#as: None };
        h.base().answer_with("start_session", vec![session_json("se_2", None)]);
        let opened = ok(h.area.open_with(&placed, OpenAsk { prompt: "look".to_owned(), ..Default::default() }, None).await);
        assert_eq!(serde_json::to_string(&h.base().args_of("start_session")[0]["opts"]).unwrap(), "{\"prompt\":\"look\"}");
        assert_eq!((opened.thread_id.as_str(), opened.session_id.as_str()), ("se_2", "se_2"));
        assert!(h.base().persisted.borrow().is_empty());
        assert!(h.base().events.borrow().is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 11: reviewers

#[test]
fn reviewers_put_codex_first() {
    // start-review.test.ts:256
    let h = H::new();
    assert_eq!(h.area.reviewers(), vec!["codex", "claude"]);
    let h = H::new();
    h.base().answer_with(
        "read_only_modes",
        vec![json!([["claude", "plan"], ["cursor", "ask"], ["gemini", null], ["codex", "read-only"], ["opencode", "plan"]])],
    );
    assert_eq!(h.area.reviewers(), vec!["codex", "claude", "cursor", "opencode"]);
}

#[test]
fn read_only_of_is_the_catalogs_read_only_mode() {
    let h = H::new();
    assert_eq!(h.area.read_only_of("claude"), Some("plan".to_owned()));
    assert_eq!(h.area.read_only_of("codex"), Some("read-only".to_owned()));
    assert_eq!(h.area.read_only_of("gemini"), None);
    assert_eq!(h.area.read_only_of("nobody"), None);
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 12: take_review

const DIFF: &str =
    "diff --git a/check.sh b/check.sh\n--- a/check.sh\n+++ b/check.sh\n@@ -1,3 +1,4 @@\n #!/bin/sh\n+unused=1\n exit 0\n # end\n";

fn fence(body: &Value) -> String {
    format!("```json\n{}\n```", body)
}

fn block() -> Value {
    json!({
        "verdict": "request_changes",
        "summary": "One unused variable.",
        "comments": [
            { "path": "check.sh", "line": 2, "side": "RIGHT", "body": "unused is never read." },
            { "path": "check.sh", "line": 40, "side": "RIGHT", "body": "and this line is not in the diff." },
        ],
    })
}

const REASK: &str =
    "Your review could not be read: the reply has no fenced json block. End your reply with the one fenced json block the task asked for.";

/// A workspace under review, its reviewer thread named, and the project that holds its remote.
fn under_review(h: &H, over: Value) -> Entry {
    h.base().hold_project(here_project());
    let from = from_pr("review", &pr_json(REPO, 7, json!({})));
    let mut v = json!({ "name": "Review #7 Rename the status word", "from": from, "reviewThread": "th_review" });
    base::merge(&mut v, over);
    h.base().hold(record("ws_rev", "local", "pr_1", v))
}

fn diff_answer() -> Value {
    json!({ "diff": DIFF, "truncated": false, "left": [] })
}

fn held_fact(head: &str) -> wsp_wire::types::pull_request::PullRequestSeen {
    let mut v = pr_json(REPO, 7, json!({ "headOid": head }));
    v["readAt"] = json!(5);
    serde_json::from_value(v).unwrap()
}

#[test]
fn take_review_reads_the_draft_marks_lines_outside_the_diff_and_emits() {
    // start-review.test.ts:263
    run(async {
        let h = H::new();
        let entry = under_review(&h, json!({}));
        h.base().answer_with("ask:git.prDiff", vec![diff_answer()]);
        entry.borrow_mut().pr = Some(held_fact("a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1"));
        ok(h.area.take_review(&entry, "th_review", &fence(&block())).await);
        let want = json!({
            "verdict": "request_changes", "summary": "One unused variable.",
            "comments": [
                { "id": "c1", "path": "check.sh", "line": 2, "side": "RIGHT", "body": "unused is never read.", "on": true },
                { "id": "c2", "path": "check.sh", "line": 40, "side": "RIGHT", "body": "and this line is not in the diff.", "on": true, "inSummary": true },
            ],
            "headOid": "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1", "threadId": "th_review", "at": 1_000_000,
        });
        let kept = rec_json(&entry)["review"].clone();
        assert_eq!(serde_json::to_string(&kept).unwrap(), serde_json::to_string(&want).unwrap());
        let persisted = h.base().persisted.borrow();
        assert_eq!(persisted.len(), 1);
        assert_eq!(serde_json::to_value(&persisted[0]).unwrap()["review"], want);
        assert_eq!(h.base().events.borrow().clone(), vec![json!({ "type": "workspace.review", "workspaceId": "ws_rev" })]);
        assert_eq!(h.base().asks(), vec![json!({ "op": "git.prDiff", "cwd": "/home/me", "remote": REMOTE, "number": 7 })]);
        assert_eq!(
            h.base().names_of(&["on_this_computer", "persist", "emit", "send_detached"]),
            vec!["on_this_computer", "persist", "emit"]
        );
    });
}

#[test]
fn take_review_marks_nothing_for_the_summary_where_the_diff_cannot_be_read() {
    // rule 12: the diff is none on any failure
    run(async {
        let h = H::new();
        let entry = under_review(&h, json!({}));
        h.base().answer_with("ask:git.prDiff", vec![err_answer("no signed-in command line", None)]);
        ok(h.area.take_review(&entry, "th_review", &fence(&block())).await);
        let kept = rec_json(&entry)["review"].clone();
        assert_eq!(kept["comments"].as_array().unwrap().len(), 2);
        assert!(kept["comments"].as_array().unwrap().iter().all(|c| c.get("inSummary").is_none() && c["on"] == json!(true)));
        assert_eq!(h.base().events.borrow().len(), 1);
    });
}

#[test]
fn take_review_takes_the_head_from_the_review_when_no_pull_request_fact_is_held() {
    run(async {
        let h = H::new();
        let entry = under_review(&h, json!({}));
        h.base().answer_with("ask:git.prDiff", vec![diff_answer()]);
        ok(h.area.take_review(&entry, "th_review", &fence(&block())).await);
        assert_eq!(rec_json(&entry)["review"]["headOid"], json!(HEAD));

        let h = H::new();
        let mut from = serde_json::to_value(from_pr("review", &pr_json(REPO, 7, json!({})))).unwrap();
        from.as_object_mut().unwrap().remove("head");
        let entry = under_review(&h, json!({ "from": from }));
        h.base().answer_with("ask:git.prDiff", vec![diff_answer()]);
        ok(h.area.take_review(&entry, "th_review", &fence(&block())).await);
        assert_eq!(rec_json(&entry)["review"]["headOid"], json!(""));
    });
}

#[test]
fn take_review_asks_once_more_then_keeps_the_note() {
    // start-review.test.ts:300
    run(async {
        let h = H::new();
        let entry = under_review(&h, json!({}));
        ok(h.area.take_review(&entry, "th_review", "Looks good to me.").await);
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        let why = "the reply has no fenced json block";
        assert_eq!(rec_json(&entry)["review"], json!({ "note": why, "at": 1_000_000, "reasked": true }));
        assert_eq!(h.base().events.borrow().clone(), vec![json!({ "type": "workspace.review", "workspaceId": "ws_rev" })]);
        assert_eq!(h.base().args_of("send_detached"), vec![json!({ "id": "ws_rev", "prompt": REASK, "thread": "th_review" })]);
        assert_eq!(h.base().names_of(&["persist", "emit", "send_detached"]), vec!["persist", "emit", "send_detached"]);

        h.base().clock.advance(10);
        ok(h.area.take_review(&entry, "th_review", "Still no block.").await);
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert_eq!(rec_json(&entry)["review"], json!({ "note": why, "at": 1_000_010, "reasked": true }));
        assert_eq!(h.base().persisted.borrow().len(), 2);
        assert_eq!(h.base().events.borrow().len(), 2);
        assert_eq!(h.base().args_of("send_detached").len(), 1);
        assert!(h.base().asks().is_empty());
    });
}

#[test]
fn take_review_warns_when_the_reask_cannot_be_sent() {
    run(async {
        let h = H::new();
        let warned = wsp_core::console::capture();
        h.base().answer_with("send_detached", vec![err_answer("the thread is gone", None)]);
        let entry = under_review(&h, json!({}));
        ok(h.area.take_review(&entry, "th_review", "Looks good to me.").await);
        for _ in 0..6 {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            warned.lines(),
            vec!["the reviewer of Review #7 Rename the status word was not asked again: the thread is gone".to_owned()]
        );
        assert_eq!(rec_json(&entry)["review"]["reasked"], json!(true));
    });
}

#[test]
fn take_review_leaves_a_read_draft_on_a_reply_with_no_block() {
    run(async {
        let h = H::new();
        let draft = json!({
            "verdict": "approve", "summary": "Fine.", "comments": [], "headOid": HEAD, "threadId": "th_review", "at": 3,
        });
        let entry = under_review(&h, json!({ "review": draft }));
        let before = rec_json(&entry);
        ok(h.area.take_review(&entry, "th_review", "Anything else you want me to look at?").await);
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert_eq!(rec_json(&entry), before);
        assert!(h.base().persisted.borrow().is_empty());
        assert!(h.base().events.borrow().is_empty());
        assert!(h.base().args_of("send_detached").is_empty());
        assert!(h.base().asks().is_empty());
    });
}

#[test]
fn take_review_ignores_another_thread_and_a_start() {
    // start-review.test.ts:284
    run(async {
        let a_start = serde_json::to_value(from_pr("pull_request", &pr_json(REPO, 7, json!({})))).unwrap();
        let cases = [
            ("another thread", json!({}), "th_other"),
            ("a start", json!({ "from": a_start }), "th_review"),
            ("no origin at all", json!({ "from": null }), "th_review"),
        ];
        for (name, over, thread) in cases {
            let h = H::new();
            let entry = under_review(&h, over);
            let before = rec_json(&entry);
            h.base().answer_with("ask:git.prDiff", vec![diff_answer()]);
            ok(h.area.take_review(&entry, thread, &fence(&block())).await);
            for _ in 0..4 {
                tokio::task::yield_now().await;
            }
            assert_eq!(rec_json(&entry), before, "{name}");
            assert!(h.base().persisted.borrow().is_empty(), "{name}");
            assert!(h.base().events.borrow().is_empty(), "{name}");
            assert!(h.base().asks().is_empty(), "{name}");
            assert!(h.base().args_of("send_detached").is_empty(), "{name}");
        }
    });
}

#[test]
fn take_review_reads_any_thread_when_no_reviewer_is_named() {
    run(async {
        let h = H::new();
        let entry = under_review(&h, json!({ "reviewThread": null }));
        h.base().answer_with("ask:git.prDiff", vec![diff_answer()]);
        ok(h.area.take_review(&entry, "th_any", &fence(&block())).await);
        assert_eq!(rec_json(&entry)["review"]["threadId"], json!("th_any"));
        assert_eq!(h.base().persisted.borrow().len(), 1);
    });
}
