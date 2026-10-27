// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/pull-request.test.ts at 65f6b89c7, case for case, first half: reading the pull request, what a
//! read costs the git host, and a rate limit the git host answered. The workspace, the two daemons behind one dial and
//! the pull request each status carried are `common/pull_request.rs`. Each test names the TS test above it.
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::pull_request::*;
use common::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::events::{CHECKOUT_TTL_MS, PR_PAGE_HOLD_MS, RATE_LIMIT_HOLD_MS};
use wsp_wire::types::pull_request::{PR_POLL_IDLE_MS, PR_POLL_MS};
use wsp_words::pull_request::{pull_request_stopped_line, pull_request_unread_line};

async fn checkout(h: &Held2) {
    h.rt().workspaces().checkout(&h.id, None, None).await.expect("a checkout");
}

fn frame(f: &(Via, serde_json::Map<String, Value>)) -> Value {
    Value::Object(f.1.clone())
}

/// `^https:\/\/github\.com\/wsp\/stub-\d+\.git$`
fn is_stub_remote(remote: &str) -> bool {
    remote
        .strip_prefix("https://github.com/wsp/stub-")
        .and_then(|rest| rest.strip_suffix(".git"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The answer of an op that a case swaps while it runs.
fn swappable(first: Answer) -> (Rc<RefCell<Answer>>, Answer) {
    let cell = Rc::new(RefCell::new(first));
    let read = cell.clone();
    (cell, answer(move |f| (read.borrow())(f)))
}

fn clocked() -> FakeClock {
    FakeClock::at_now()
}

// "reading the pull request" > "goes over this computer's own daemon with the project's remote and the copy's branch, and never asks the copy for it"
#[test]
fn goes_over_this_computers_own_daemon_with_the_projects_remote_and_the_copys_branch_and_never_asks_the_copy_for_it() {
    drive(|| async {
        let d = fake_daemons(vec![], vec![]);
        let h = with_workspace(&d, With::default()).await;
        checkout(&h).await;
        until(|| async { d.ops(Some(Via::Here)).contains(&"git.prRead".to_string()) }).await;
        let read = d.of("git.prRead").remove(0);
        assert_eq!(read.0, Via::Here);
        assert_eq!(
            frame(&read),
            json!({ "op": "git.prRead", "cwd": h.root.display().to_string(), "remote": h.remote, "branch": "fix/ci" })
        );
        assert!(is_stub_remote(&h.remote), "{}", h.remote);
        assert_eq!(d.ops(Some(Via::Copy)), ["git.status"]);
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let pr = h.pr().expect("a pull request");
        assert_matches(&pr, json!({ "number": 12, "state": "open" }));
        assert!(pr["readAt"].is_number());
        // The listing every client and `wsp workspaces --json` read carries it where it carries the branch line.
        let rows = status_rows(h.rt()).await;
        let row = rows.iter().find(|w| w["id"] == h.id.as_str()).expect("the workspace's row");
        assert_matches(&row["pr"], json!({ "number": 12, "state": "open" }));
    });
}

// "asks the copy only where this computer has no command line for the host and the copy runs, and reads not read otherwise"
#[test]
fn asks_the_copy_only_where_this_computer_has_no_command_line_for_the_host_and_the_copy_runs_and_reads_not_read_otherwise() {
    drive(|| async {
        let d = fake_daemons(
            vec![("git.prRead", no_gh())],
            vec![("git.prRead", answer(|_| ok(json!({ "pr": open(json!({ "number": 3 })) }))))],
        );
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(3)) }).await;
        let asked = d.of("git.prRead");
        assert_eq!(asked.iter().map(|f| f.0).collect::<Vec<_>>(), [Via::Here, Via::Copy]);
        // The copy is told the same remote, off the record, and runs its own gh in its own checkout.
        let copy = frame(&asked[1]);
        assert_matches(&copy, json!({ "remote": h.remote, "branch": "fix/ci" }));
        let cwd = copy["cwd"].as_str().expect("a cwd");
        assert!(cwd.strip_prefix("/root/stub-").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())), "{cwd}");
        // Napped, the copy is never woken for a read, and the tile says why it is not read.
        h.rt().workspaces().nap(&h.id, None).await.expect("a nap");
        d.frames.borrow_mut().clear();
        fc.advance(PR_POLL_IDLE_MS as i64);
        checkout(&h).await;
        until(|| async { h.pr_field("why").is_some_and(|w| w.is_string()) }).await;
        assert_matches(&h.pr().expect("a pull request"), json!({ "why": pull_request_stopped_line("github.com", &h.name) }));
        assert_eq!(d.count(Via::Copy, "git.prRead"), 0);
    });
}

// "reads not read where neither this computer nor the copy has a command line for the host"
#[test]
fn reads_not_read_where_neither_this_computer_nor_the_copy_has_a_command_line_for_the_host() {
    drive(|| async {
        let d = fake_daemons(vec![("git.prRead", no_gh())], vec![("git.prRead", no_gh())]);
        let h = with_workspace(&d, With::default()).await;
        checkout(&h).await;
        until(|| async { h.pr_field("why").is_some_and(|w| w.is_string()) }).await;
        assert_matches(&h.pr().expect("a pull request"), json!({ "why": pull_request_unread_line("github.com") }));
    });
}

// "keeps the fact it holds through a read gh failed, and reads again on its timer after it"
#[test]
fn keeps_the_fact_it_holds_through_a_read_gh_failed_and_reads_again_on_its_timer_after_it() {
    drive(|| async {
        let (cell, read) = swappable(answer(|_| ok(json!({ "pr": open(json!({ "checks": [{ "name": "ci", "state": "pass" }] })) }))));
        let d = fake_daemons(vec![("git.prRead", read)], vec![]);
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        *cell.borrow_mut() = answer(|_| json!({ "id": 1, "ok": false, "error": "gh said: error connecting to api.github.com" }));
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { d.ops(Some(Via::Here)).len() == 2 }).await;
        sleep_ms(20).await;
        assert_matches(&h.pr().expect("a pull request"), json!({ "number": 12, "state": "open" }));
        *cell.borrow_mut() = answer(|_| ok(json!({ "pr": open(json!({ "state": "merged" })) })));
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { d.ops(Some(Via::Here)).len() == 3 }).await;
        until(|| async { h.pr_field("state") == Some(json!("merged")) }).await;
    });
}

async fn reads_as_github_says_it(state: &'static str) {
    let d = fake_daemons(vec![("git.prRead", answer(move |_| ok(json!({ "pr": open(json!({ "state": state })) }))))], vec![]);
    let h = with_workspace(&d, With::default()).await;
    checkout(&h).await;
    until(|| async { h.pr_field("state") == Some(json!(state)) }).await;
    assert_matches(&h.pr().expect("a pull request"), json!({ "number": 12, "state": state }));
}

// "reads %s as GitHub says it, merged where the pull request merged and closed where it closed without a merge, onto the status the tile reads" (merged)
#[test]
fn reads_merged_as_github_says_it_merged_where_the_pull_request_merged_and_closed_where_it_closed_without_a_merge_onto_the_status_the_tile_reads(
) {
    drive(|| async { reads_as_github_says_it("merged").await });
}

// the same, (closed)
#[test]
fn reads_closed_as_github_says_it_merged_where_the_pull_request_merged_and_closed_where_it_closed_without_a_merge_onto_the_status_the_tile_reads(
) {
    drive(|| async { reads_as_github_says_it("closed").await });
}

// "reads an open one again on its timer whatever its checks say, so a merge made on the host's own page is seen, and never again once merged"
#[test]
fn reads_an_open_one_again_on_its_timer_whatever_its_checks_say_so_a_merge_made_on_the_hosts_own_page_is_seen_and_never_again_once_merged()
{
    drive(|| async {
        // Just after a push the host answers the old head with no checks at all, and once checks pass nothing is pending:
        // neither is the last word on an open pull request.
        let (cell, read) = swappable(answer(|_| ok(json!({ "pr": open(json!({ "checks": [] })) }))));
        let d = fake_daemons(vec![("git.prRead", read)], vec![]);
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { d.ops(Some(Via::Here)).len() == 1 }).await;
        *cell.borrow_mut() = answer(|_| ok(json!({ "pr": open(json!({ "checks": [{ "name": "ci", "state": "pass" }] })) })));
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { d.ops(Some(Via::Here)).len() == 2 }).await;
        // Merged on the host's page, with no window open and no turn ending: the timer is what sees it.
        *cell.borrow_mut() =
            answer(|_| ok(json!({ "pr": open(json!({ "state": "merged", "checks": [{ "name": "ci", "state": "pass" }] })) })));
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { d.ops(Some(Via::Here)).len() == 3 }).await;
        until(|| async { h.pr_field("state") == Some(json!("merged")) }).await;
        // Merged is never read again, whatever asks.
        fc.advance(PR_POLL_IDLE_MS as i64 * 3);
        fc.advance(20_000);
        checkout(&h).await;
        sleep_ms(20).await;
        assert_eq!(d.ops(Some(Via::Here)).len(), 3);
    });
}

fn reads(d: &Daemons) -> usize {
    d.count(Via::Here, "git.prRead")
}

// "what a read costs the git host" > "asks with what the last read saw, and an answer that it stood keeps the fact with nothing else read"
#[test]
fn asks_with_what_the_last_read_saw_and_an_answer_that_it_stood_keeps_the_fact_with_nothing_else_read() {
    drive(|| async {
        let (cell, read) = swappable(answer(|_| ok(json!({ "pr": open(json!({})), "seen": SEEN }))));
        let d = fake_daemons(vec![("git.prRead", read)], vec![]);
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        *cell.borrow_mut() = answer(|_| ok(json!({ "unchanged": true, "seen": SEEN })));
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { reads(&d) == 2 }).await;
        let asked: Vec<Value> = d.of("git.prRead").iter().map(frame).collect();
        assert_eq!(
            asked[1],
            json!({ "op": "git.prRead", "cwd": h.root.display().to_string(), "remote": h.remote, "number": 12, "seen": SEEN })
        );
        until(|| async { h.pr_field("readAt") == Some(json!(fc.now())) }).await;
        assert_matches(
            &h.pr().expect("a pull request"),
            json!({ "number": 12, "state": "open", "headOid": HEAD, "checks": [{ "name": "ci", "state": "pass" }] }),
        );
        // A changed one answers in full with what it saw now, which the next read carries.
        *cell.borrow_mut() = answer(|_| ok(json!({ "pr": open(json!({ "headOid": "c0ffee" })), "seen": MOVED })));
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { h.pr_field("headOid") == Some(json!("c0ffee")) }).await;
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { reads(&d) == 4 }).await;
        let last = frame(&d.of("git.prRead").pop().expect("a read"));
        assert_matches(&last, json!({ "seen": MOVED }));
    });
}

// "asks with nothing seen while a check runs, since a check finishing moves nothing of what is seen"
#[test]
fn asks_with_nothing_seen_while_a_check_runs_since_a_check_finishing_moves_nothing_of_what_is_seen() {
    drive(|| async {
        let pending = answer(|_| ok(json!({ "pr": open(json!({ "checks": [{ "name": "ci", "state": "pending" }] })), "seen": SEEN })));
        let d = fake_daemons(vec![("git.prRead", pending)], vec![]);
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        fc.advance(PR_POLL_IDLE_MS as i64);
        until(|| async { reads(&d) == 2 }).await;
        let last = frame(&d.of("git.prRead").pop().expect("a read"));
        assert!(last.get("seen").is_none(), "{last}");
    });
}

// "reads an open one every PR_POLL_MS while a window is open and every PR_POLL_IDLE_MS with none"
#[test]
fn reads_an_open_one_every_pr_poll_ms_while_a_window_is_open_and_every_pr_poll_idle_ms_with_none() {
    drive(|| async {
        let d = fake_daemons(vec![], vec![]);
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { reads(&d) == 1 }).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        // No window: three minutes pass with nothing read, and the quarter hour reads it.
        fc.advance(PR_POLL_MS as i64);
        sleep_ms(20).await;
        assert_eq!(reads(&d), 1);
        fc.advance((PR_POLL_IDLE_MS - PR_POLL_MS) as i64);
        until(|| async { reads(&d) == 2 }).await;
        // A window opens just after that read: its sidebar's ask reads nothing, and the timer moves to three minutes.
        let release = h.rt().status().watch(Default::default());
        checkout(&h).await;
        sleep_ms(20).await;
        assert_eq!(reads(&d), 2);
        fc.advance(PR_POLL_MS as i64);
        until(|| async { reads(&d) == 3 }).await;
        sleep_ms(20).await;
        fc.advance(PR_POLL_MS as i64);
        until(|| async { reads(&d) == 4 }).await;
        // Within those three minutes the sidebar's asks read nothing, since the timer keeps it.
        fc.advance(CHECKOUT_TTL_MS as i64 + 1);
        checkout(&h).await;
        sleep_ms(20).await;
        assert_eq!(reads(&d), 4);
        release();
    });
}

// "reads a branch with no pull request again on view only once its head moves"
#[test]
fn reads_a_branch_with_no_pull_request_again_on_view_only_once_its_head_moves() {
    drive(|| async {
        let head = Rc::new(RefCell::new("abc1234".to_string()));
        let seen_head = head.clone();
        let d = fake_daemons(
            vec![("git.prRead", answer(|_| ok(json!({}))))],
            vec![(
                "git.status",
                answer(move |_| {
                    ok(json!({
                        "branch": { "oid": seen_head.borrow().clone(), "head": "fix/ci", "upstream": "origin/fix/ci", "ahead": 1, "behind": 0 },
                        "entries": [], "root": "/root/stub"
                    }))
                }),
            )],
        );
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { reads(&d) == 1 }).await;
        sleep_ms(20).await;
        for _ in 0..3 {
            fc.advance(CHECKOUT_TTL_MS as i64 + 1);
            checkout(&h).await;
        }
        sleep_ms(20).await;
        assert_eq!(reads(&d), 1);
        *head.borrow_mut() = "def0123".into();
        fc.advance(CHECKOUT_TTL_MS as i64 + 1);
        checkout(&h).await;
        until(|| async { reads(&d) == 2 }).await;
    });
}

/// A page as git.prView answers it, its items off PR 772 of wsp-labs/wsp: a bot's comment, a review, and a comment on a
/// line with its hunk.
pub fn page() -> Value {
    json!({
        "title": "t",
        "body": "b",
        "author": "octocat",
        "createdAt": "2026-09-30T10:00:00Z",
        "updatedAt": "2026-09-30T12:00:00Z",
        "labels": [],
        "reviewRequests": [],
        "latestReviews": [],
        "assignees": [],
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

// "a rate limit the git host answered" > "answers every page and status read on that road with the refusal for a minute, and asks again past it"
#[test]
fn answers_every_page_and_status_read_on_that_road_with_the_refusal_for_a_minute_and_asks_again_past_it() {
    drive(|| async {
        let (cell, view) = swappable(rate_limited());
        let d =
            fake_daemons(vec![("git.prView", view), ("git.repoRead", answer(|_| json!({ "id": 1, "ok": false, "error": "no" })))], vec![]);
        let fc = clocked();
        let h = with_workspace(&d, With { clock: Some(fc.clone()), ..With::default() }).await;
        checkout(&h).await;
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        let asked = |op: &str| d.count(Via::Here, op);
        let before = asked("git.prRead");
        fc.advance(PR_POLL_IDLE_MS as i64 - 30_000);
        let refused = |r: Result<_, wsp_words::exit::HostError>| match r {
            Ok(_) => panic!("the read was not refused"),
            Err(e) => e.message,
        };
        let said = refused(h.rt().workspaces().pull_request_view(&h.id, None, None).await);
        assert!(said.contains("API rate limit already exceeded"), "{said}");
        // Each mount within the minute meets the same refusal with no command line run, a fresh ask too.
        for _ in 0..3 {
            let said = refused(h.rt().workspaces().pull_request_view(&h.id, Some(true), None).await);
            assert!(said.contains("API rate limit already exceeded"), "{said}");
        }
        assert_eq!(asked("git.prView"), 1);
        // So does the status timer falling inside it, and the fact it held stands.
        fc.advance(30_000);
        sleep_ms(20).await;
        assert_eq!(asked("git.prRead"), before);
        assert_matches(&h.pr().expect("a pull request"), json!({ "number": 12, "state": "open" }));
        *cell.borrow_mut() = answer(|_| ok(page()));
        fc.advance(RATE_LIMIT_HOLD_MS as i64 - 30_000);
        h.rt().workspaces().pull_request_view(&h.id, None, None).await.expect("a page");
        assert_eq!(asked("git.prView"), 2);
        let _ = PR_PAGE_HOLD_MS;
    });
}
