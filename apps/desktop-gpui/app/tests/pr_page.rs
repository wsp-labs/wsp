// SPDX-License-Identifier: AGPL-3.0-only
//! The Pull request pane on a throwaway host whose gh is the stub in `pr_pane/gh/`: what it reads off the page in
//! each state, what its acts send gh, a reaction gh refuses going back, and a `workspace.status` push redrawing the
//! status card. Each window is the pane inside gpui-kit's Root after `gpui_kit::init` and wsp's themes.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[allow(dead_code)]
#[path = "pr_pane/pr_page_host.rs"]
mod pr_page_host;

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::pr_pane::changes;
use desktop_gpui::screens::pr_pane::pull_request::model::{self, MergeMethod, Seen, State};
use desktop_gpui::screens::pr_pane::pull_request::PullRequestScreen;
use desktop_gpui::screens::{Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, Modifiers, TestAppContext, VisualTestContext};
use pr_page_host::{Door, PrHost, WAIT};
use serde_json::Value;
use wsp_host_client::HostClient;

fn open<'a>(host: &PrHost, cx: &'a mut TestAppContext) -> (Entity<PullRequestScreen>, &'a mut VisualTestContext) {
    open_at(host, px(3000.), cx)
}

fn open_at<'a>(
    host: &PrHost,
    height: gpui_kit::Pixels,
    cx: &'a mut TestAppContext,
) -> (Entity<PullRequestScreen>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let opened = Opened { workspace_id: Some(host.workspace_id.clone()), thread_id: Some(host.thread_id.clone()), ..Opened::default() };
    let shell = host.host();
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| PullRequestScreen::new(shell, opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(480.), height));
    cx.run_until_parked();
    (made.unwrap(), cx)
}

/// Lets real time pass, the host's answers landing on the window's thread, until `done` holds.
fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "{what} within {} s", WAIT.as_secs());
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn read(screen: &Entity<PullRequestScreen>, cx: &mut VisualTestContext) {
    until(cx, "the page read", |cx| screen.read_with(cx, |s, _| s.page().is_some()));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn seen(screen: &Entity<PullRequestScreen>, cx: &mut VisualTestContext) -> Option<Seen> {
    screen.read_with(cx, |s, _| s.status().and_then(|st| st.pr.clone()))
}

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// What the window's toasts stand with: each notice's kind, words and the workspace it is about.
fn toasts(cx: &mut VisualTestContext) -> Vec<(ui::NoticeKind, String, Option<String>)> {
    let stack = cx.update(|window, cx| ui::toast::of(window, cx).expect("ui::init registers the toasts"));
    stack.read_with(cx, |t, _| t.standing().into_iter().map(|n| (n.kind, n.text.to_string(), n.place.map(|p| p.to_string()))).collect())
}

/// The argv of every call the stub was given holding all of `words`.
fn argv(host: &PrHost, words: &[&str]) -> Vec<Vec<String>> {
    host.calls_with(words)
        .into_iter()
        .map(|c| c["argv"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_owned()).collect())
        .collect()
}

#[gpui_kit::test]
fn an_open_pull_request_reads_its_page_off_gh(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let seen = seen(&screen, cx).expect("the status names the pull request");
    let Seen::Fact(fact) = &seen else { panic!("a read fact, not {seen:?}") };
    assert_eq!(model::tone_of(&seen).0, "Checks failed");
    assert_eq!(model::hold_line(fact), "Held until the checks pass");
    assert_eq!(model::checks_line(&fact.checks), "3 passed, 1 failed, 1 running, 1 skipped");
    assert_eq!(fact.behind_base, Some(2));
    let page = screen.read_with(cx, |s, _| s.page().cloned().unwrap());
    assert_eq!(page.title, "Teach the pane to read checks");
    assert!(page.posts_as_you, "this computer's gh read it, so the pane posts as the person");
    assert_eq!(model::methods(page.merge.as_ref())[0], MergeMethod::Squash, "the repository's default first");
    for row in ["pr-head", "pr-title", "pr-branches", "pr-labels", "pr-asks", "pr-tabs", "pr-status", "pr-timeline", "pr-comment-box"] {
        assert!(drawn(cx, row), "{row} is drawn");
    }
    for row in [
        "pr-row:pr-checks",
        "pr-row:pr-check-lint",
        "pr-row:pr-check-test",
        "pr-row:pr-checks-passed",
        "pr-row:pr-review",
        "pr-row:pr-comments",
        "pr-row:pr-base",
        "pr-row:pr-hold",
    ] {
        assert!(drawn(cx, row), "{row} is drawn on an open pull request with a failed and a running check");
    }
    let threads = model::threads_of(&page.review_comments);
    let open = threads.iter().find(|t| !t.resolved).expect("an unresolved thread");
    assert!(open.comments.len() >= 2, "the reply sits in its thread under the comment it answers");
    let folded = threads.iter().find(|t| t.resolved).expect("a resolved thread");
    let resolved_box = screen.read_with(cx, |s, _| s.thread_box(&folded.key).cloned()).expect("a box per thread");
    assert!(!resolved_box.read_with(cx, |b, _| b.is_open()), "a resolved thread stands folded");
    assert!(!argv(&host, &["api", "graphql"]).is_empty(), "the page came off gh's one GraphQL read");
}

#[gpui_kit::test]
fn a_merged_pull_request_says_who_merged_it_and_where_it_landed(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("merged") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    assert_eq!(seen(&screen, cx).map(|s| s.state()), Some(State::Merged));
    let merged_at = screen.read_with(cx, |s, _| s.page().and_then(|p| p.merged_at.clone())).expect("the page says when");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let ago = model::spaced_ago(&merged_at, model::now());
    assert!(ago.ends_with(" m ago"), "merged minutes ago: {ago}");
    let words = |key: &str, words: &str| -> &'static str { Box::leak(format!("{key}:{words}").into_boxed_str()) };
    for (selector, what) in [
        (words("pr-settled-word", "Merged by"), "the row says who merged it"),
        (words("pr-settled-by", "ana"), "ana"),
        (words("pr-settled-ago", &ago), "and when"),
        (words("pr-landed-words", "Landed on main as"), "the commit it landed as"),
        (words("pr-landed-sha", "5d7f9b1"), "by its first seven"),
    ] {
        assert!(drawn(cx, selector), "{what}: {selector} is drawn");
    }
    for row in ["pr-row:pr-settled", "pr-row:pr-landed", "pr-row:pr-checks"] {
        assert!(drawn(cx, row), "{row} is drawn on a merged pull request");
    }
    assert!(!drawn(cx, "pr-row:pr-hold"), "a settled pull request holds no merge");
    assert!(!drawn(cx, "pr-row:pr-checks-passed"), "its checks stand folded");
}

#[gpui_kit::test]
fn a_closed_pull_request_says_it_closed_unmerged(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("closed") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let seen = seen(&screen, cx).unwrap();
    assert_eq!((seen.state(), model::tone_of(&seen).0), (State::Closed, "Closed".into()));
    assert!(drawn(cx, "pr-row:pr-settled"));
    assert!(!drawn(cx, "pr-row:pr-landed"), "nothing landed");
}

#[gpui_kit::test]
fn a_branch_with_no_pull_request_shows_the_pane_s_shape(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("none") else { return };
    let (screen, cx) = open(&host, cx);
    until(cx, "the host's status", |cx| screen.read_with(cx, |s, _| s.status().is_some()));
    until(cx, "gh asked for the branch's pull request", |_| !host.calls_with(&["pr", "view", "work"]).is_empty());
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(seen(&screen, cx), None, "no pull request is named");
    assert!(drawn(cx, "pr-skeleton-head"));
    assert!(!drawn(cx, "pr-head"));
}

#[gpui_kit::test]
fn a_reaction_reaches_gh_on_its_item(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let page = screen.read_with(cx, |s, _| s.page().cloned().unwrap());
    let comment = page.comments.iter().find(|c| !c.bot).unwrap();
    let subject = comment.node_id.clone().unwrap();
    screen.update_in(cx, |s, window, cx| s.react(subject.clone(), "heart".into(), true, window, cx));
    until(cx, "the reaction put on", |_| !argv(&host, &["api", "graphql", "content=HEART"]).is_empty());
    let line = &argv(&host, &["content=HEART"])[0];
    assert!(line.contains(&format!("id={subject}")), "the reaction names its comment: {line:?}");
    assert!(line.iter().any(|a| a.contains("{ addReaction")), "it adds, not removes: {line:?}");
}

#[gpui_kit::test]
fn a_comment_reaches_gh_as_typed(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let foot = screen.read_with(cx, |s, _| s.foot().cloned()).expect("the comment box at Activity's foot");
    let field = foot.read_with(cx, |f, _| f.field().clone());
    field.update_in(cx, |f, window, cx| f.set_value("Thanks, fixing it now", window, cx));
    foot.update_in(cx, |f, window, cx| f.send(window, cx));
    until(cx, "the comment posted and on the page", |cx| {
        screen.read_with(cx, |s, _| s.page().is_some_and(|p| p.comments.iter().any(|c| c.body == "Thanks, fixing it now")))
    });
    let posted = host.calls_with(&["repos/wsp-test/repo/issues/42/comments"]);
    let body: Value = serde_json::from_str(posted[0]["stdin"].as_str().unwrap()).unwrap();
    assert_eq!(body["body"], "Thanks, fixing it now", "the words go to gh on stdin as typed");
}

#[gpui_kit::test]
fn a_resolve_reaches_gh_on_its_thread(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let page = screen.read_with(cx, |s, _| s.page().cloned().unwrap());
    let thread = model::threads_of(&page.review_comments).into_iter().find(|t| !t.resolved).unwrap();
    let thread_id = thread.thread_id.clone().unwrap();
    let thread_box = screen.read_with(cx, |s, _| s.thread_box(&thread.key).cloned()).unwrap();
    thread_box.update_in(cx, |b, window, cx| b.toggle_resolved(window, cx));
    until(cx, "the thread resolved on the page", |cx| {
        screen.read_with(cx, |s, _| {
            s.page().is_some_and(|p| model::threads_of(&p.review_comments).iter().any(|t| t.key == thread.key && t.resolved))
        })
    });
    let resolve = argv(&host, &["api", "graphql", &format!("id={thread_id}")]);
    assert!(resolve.iter().any(|l| l.iter().any(|a| a.contains("{ resolveReviewThread"))), "gh resolved the thread named: {resolve:?}");
}

#[gpui_kit::test]
fn a_fix_reads_the_failed_job_s_log(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    screen.update_in(cx, |s, window, cx| s.fix(Some("lint".into()), window, cx));
    until(cx, "the failed job's log read for the fix", |_| !host.calls_with(&["run", "view"]).is_empty());
    let log = &argv(&host, &["run", "view"])[0];
    assert!(log.contains(&"11".to_owned()) && log.contains(&"22".to_owned()), "the log of the failed check's run and job: {log:?}");
    until(cx, "the toast saying the agent was asked", |cx| {
        screen.read_with(cx, |s, _| s.said().is_some_and(|w| w.ends_with("to fix lint")))
    });
}

#[gpui_kit::test]
fn a_merge_names_its_method_and_the_head_drawn(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let head = fact_head(&screen, cx);
    screen.update_in(cx, |s, window, cx| s.merge(Some(MergeMethod::Rebase), false, window, cx));
    until(cx, "gh asked to merge", |_| !host.calls_with(&["pr", "merge", "42"]).is_empty());
    let merge = &argv(&host, &["pr", "merge", "42"])[0];
    assert!(merge.contains(&"--rebase".to_owned()), "by the method picked: {merge:?}");
    let at = merge.iter().position(|a| a == "--match-head-commit").expect("the head the window drew");
    assert_eq!(merge[at + 1], head, "only while the head is the one shown");
}

#[gpui_kit::test]
fn a_second_press_before_the_host_answers_sends_nothing_more(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    screen.update_in(cx, |s, window, cx| {
        s.merge(None, true, window, cx);
        s.merge(None, true, window, cx);
        s.fix(Some("lint".into()), window, cx);
    });
    assert!(screen.read_with(cx, |s, _| s.is_acting()), "the first press holds the acts until the host answers");
    until(cx, "gh asked to merge", |_| !host.calls_with(&["pr", "merge", "42"]).is_empty());
    until(cx, "the merge answered", |cx| screen.read_with(cx, |s, _| !s.is_acting()));
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(argv(&host, &["pr", "merge", "42"]).len(), 1, "two presses on Merge when checks pass ask gh once");
    assert!(argv(&host, &["run", "view"]).is_empty(), "a fix pressed while the merge ran sends nothing");
}

fn fact_head(screen: &Entity<PullRequestScreen>, cx: &mut VisualTestContext) -> String {
    match seen(screen, cx) {
        Some(Seen::Fact(f)) => f.head_oid,
        other => panic!("no fact: {other:?}"),
    }
}

#[gpui_kit::test]
fn an_update_merges_the_base_into_the_branch(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let git = |args: &[&str]| {
        let out = Command::new("git").args(args).current_dir(&host.folder).env_clear().envs(host.server.env()).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    git(&["checkout", "-q", "main"]);
    std::fs::write(host.folder.join("NOTES.md"), "from main\n").unwrap();
    git(&["add", "NOTES.md"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "A note on main"]);
    git(&["checkout", "-q", "work"]);
    // The remote the host recorded stays GitHub's for gh; git reads it from a bare copy here, so a fetch never dials out.
    let bare = host.server.dir.path().join("remote.git");
    git(&["clone", "-q", "--bare", &host.folder.to_string_lossy(), &bare.to_string_lossy()]);
    let gitconfig = host.server.dir.path().join(".gitconfig");
    let mut held = std::fs::read_to_string(&gitconfig).unwrap();
    held.push_str("[user]\n\tname = t\n\temail = t@t\n");
    held.push_str(&format!("[url \"file://{}\"]\n\tinsteadOf = https://github.com/wsp-test/repo.git\n", bare.display()));
    std::fs::write(&gitconfig, held).unwrap();
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    screen.update_in(cx, |s, window, cx| s.update(window, cx));
    until(cx, "the base merged into work", |_| git(&["rev-list", "--parents", "-n", "1", "HEAD"]).split_whitespace().count() == 3);
    until(cx, "the toast saying so", |cx| screen.read_with(cx, |s, _| s.said() == Some("repo: updated from main, 1 commit")));
    let done = (ui::NoticeKind::Done, "repo: updated from main, 1 commit".to_owned(), Some("repo".to_owned()));
    until(cx, "the app's toasts saying so about the workspace", |cx| toasts(cx).contains(&done));
    assert!(host.folder.join("NOTES.md").exists(), "main's commit is on the branch now");
}

#[gpui_kit::test]
fn a_reaction_gh_refuses_goes_back(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let page = screen.read_with(cx, |s, _| s.page().cloned().unwrap());
    let comment = page
        .comments
        .iter()
        .find(|c| c.reactions.as_ref().is_some_and(|r| r.iter().any(|r| r.mine)))
        .expect("a comment the person reacted to");
    let subject = comment.node_id.clone().unwrap();
    let before = comment.reactions.clone().unwrap();
    let mine = before.iter().find(|r| r.mine).unwrap().content.clone();
    host.refuse("react");
    screen.update_in(cx, |s, window, cx| s.react(subject.clone(), mine.clone(), false, window, cx));
    let now = |cx: &mut VisualTestContext| {
        screen.read_with(cx, |s, _| s.page().and_then(|p| model::reactions_of(p, &subject)).unwrap_or_default())
    };
    assert_ne!(now(cx), before, "the press moves the chip at once");
    until(cx, "gh asked to take it off", |_| {
        !argv(&host, &["api", "graphql", &format!("id={subject}")]).iter().all(|l| !l.iter().any(|a| a.contains("removeReaction")))
    });
    until(cx, "the refused reaction back as it was", |cx| now(cx) == before);
    until(cx, "the host's refusal in the app's toasts, about the workspace", |cx| {
        toasts(cx).iter().any(|(kind, _, place)| *kind == ui::NoticeKind::Error && place.as_deref() == Some("repo"))
    });
}

#[gpui_kit::test]
fn a_status_push_redraws_the_status_card(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    assert!(drawn(cx, "pr-row:pr-hold"));
    assert!(!drawn(cx, "pr-row:pr-settled"));
    host.scenario("closed");
    host.server.wsp(&["send", &host.thread_id, "--detach", "one more turn"]);
    until(cx, "the push saying it closed", |cx| seen(&screen, cx).is_some_and(|s| s.state() == State::Closed));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(drawn(cx, "pr-row:pr-settled"), "the card says it closed");
    assert!(!drawn(cx, "pr-row:pr-hold"), "and holds no merge");
    assert!(!drawn(cx, "pr-row:pr-check-lint"), "the checks folded as it settled");
}

#[gpui_kit::test]
fn the_status_is_followed_again_once_the_host_restarts(cx: &mut TestAppContext) {
    let Some(mut host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    host.server.restart();
    host.scenario("closed");
    host.server.wsp(&["send", &host.thread_id, "--detach", "one more turn"]);
    until(cx, "the push saying it closed, on the socket dialled again", |cx| seen(&screen, cx).is_some_and(|s| s.state() == State::Closed));
}

#[gpui_kit::test]
fn the_reaction_picker_stays_open_on_a_press_inside_and_closes_on_escape(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    // At rest the acts take no width; the pointer over the entry opens them left of the time.
    let rest = cx.debug_bounds("pr-react-add").expect("a comment's reaction add, drawn at rest");
    cx.simulate_mouse_move(rest.center(), None, Modifiers::none());
    for _ in 0..2 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let add = cx.debug_bounds("pr-react-add");
    assert!(add.is_some_and(|b| b.origin.x < rest.origin.x), "the acts open on the hover: {add:?} against {rest:?}");
    cx.simulate_click(add.unwrap().center(), Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let picker = cx.debug_bounds("pr-react-picker").expect("the picker opens on the press");
    cx.simulate_mouse_move(gpui_kit::point(px(2.), px(2.)), None, Modifiers::none());
    cx.run_until_parked();
    assert!(drawn(cx, "pr-react-picker"), "the picker stays while the pointer leaves the entry for it");
    cx.simulate_click(picker.origin + gpui_kit::point(px(33.), px(16.)), Modifiers::none());
    cx.run_until_parked();
    assert!(drawn(cx, "pr-react-picker"), "a press inside keeps it open");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!drawn(cx, "pr-react-picker"), "Escape closes it");
    let _ = screen;
}

#[gpui_kit::test]
fn a_file_opens_its_diff_under_its_row_with_its_threads(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    screen.update(cx, |s, cx| s.pick_tab(desktop_gpui::screens::pr_pane::pull_request::Tab::Files, cx));
    cx.run_until_parked();
    let folder = cx.debug_bounds("pr-dir-row").expect("the files stand under their folder, shut");
    cx.simulate_click(folder.center(), Modifiers::none());
    cx.run_until_parked();
    screen.update_in(cx, |s, window, cx| s.press_file("src/pane.rs".into(), window, cx));
    until(cx, "the pull request's diff read", |cx| {
        screen.read_with(cx, |s, _| matches!(s.diff(), Some(desktop_gpui::screens::pr_pane::pull_request::DiffRead::Read { .. })))
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(argv(&host, &["pr", "diff", "42"]).len(), 1, "the diff is read once, off gh");
    assert!(drawn(cx, "pr-file-diff"), "the file's diff stands under its row");
    let view = screen.read_with(cx, |s, _| s.code_view_now().cloned()).expect("the code view the file draws in");
    let heads: Vec<String> = view.read_with(cx, |v, _| v.heads().iter().map(|h| h.path.to_string()).collect());
    assert_eq!(heads, ["src/pane.rs"], "only the file opened");
    screen.update_in(cx, |s, window, cx| s.press_file("src/pane.rs".into(), window, cx));
    cx.run_until_parked();
    assert!(!drawn(cx, "pr-file-diff"), "a second press folds it");
}

#[gpui_kit::test]
fn the_git_button_reads_the_checkout_and_the_pull_request(cx: &mut TestAppContext) {
    use desktop_gpui::screens::pr_pane::pull_request::git_split::{quick_action, GitAct, GitSplit};
    let Some(host) = PrHost::start("open") else { return };
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let shell = host.host();
    let (places, base) = (host.places(), host.base());
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let split = cx.new(|_| GitSplit::fed(shell));
        made = Some(split.clone());
        Root::new(split, window, cx)
    });
    let split = made.unwrap();
    split.update(cx, |s, cx| s.show(host.workspace_id.clone(), Some(host.thread_id.clone()), cx));
    // Fed as the header feeds it: the status, the project's base and the computers, read again each look.
    let feed = |cx: &mut VisualTestContext, status: Value| split.update(cx, |s, cx| s.feed(Some(&status), base.as_deref(), &places, cx));
    until(cx, "the branch's pull request on the button's status", |cx| {
        feed(cx, host.status());
        split.read_with(cx, |s, _| s.state().is_some_and(|st| st.pr_open))
    });
    let quick = split.read_with(cx, |s, _| quick_action(&s.state().unwrap()));
    assert_eq!(
        (quick.label.as_str(), quick.act),
        ("Push", Some(GitAct::Push)),
        "a clean branch ahead of its upstream with its pull request open pushes"
    );
    assert!(drawn(cx, "git-split"));
    std::fs::write(host.folder.join("src/new.rs"), "fn new() {}\n").unwrap();
    host.server.wsp(&["send", &host.thread_id, "--detach", "one more turn"]);
    until(cx, "the checkout read again with the change", |cx| {
        feed(cx, host.status());
        split.read_with(cx, |s, _| s.state().and_then(|st| st.checkout).is_some_and(|c| c.changed > 0))
    });
    let quick = split.read_with(cx, |s, _| quick_action(&s.state().unwrap()));
    assert_eq!(quick.act, Some(GitAct::CommitPush), "a change on a branch whose pull request is open commits and pushes");
    let mut stopped = host.status();
    assert_eq!(stopped["kind"], "local");
    stopped["reach"]["state"] = "no-daemon".into();
    let workspace_name = stopped["name"].as_str().unwrap_or_default().to_owned();
    let here = host.place("here");
    let name = here["label"].as_str().or(here["name"].as_str()).unwrap_or_default().to_owned();
    assert!(!name.is_empty() && name != workspace_name, "this computer's name tells it from the workspace's: {here}");
    let line = format!("{name}'s terminals and files stopped; git actions open once they start again");
    // Fed again each look, since the turn's own pushes may still land after it.
    until(cx, "the button held in this computer's name, read off the places, while its daemon is down", |cx| {
        feed(cx, stopped.clone());
        split.read_with(cx, |s, _| s.state().and_then(|st| st.away)) == Some(line.clone())
    });
}

#[gpui_kit::test]
fn another_pull_request_drops_what_was_read_for_the_last(cx: &mut TestAppContext) {
    use desktop_gpui::screens::pr_pane::pull_request::DiffRead;
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let comment = screen.read_with(cx, |s, _| s.page().unwrap().comments.iter().find(|c| !c.bot).cloned().unwrap());
    let mut moved = host.status();
    moved["pr"]["number"] = 43.into();
    screen.update_in(cx, |s, window, cx| {
        s.react(comment.node_id.clone().unwrap(), "rocket".into(), true, window, cx);
        s.press_file("src/pane.rs".into(), window, cx);
        s.take_status(moved, window, cx);
    });
    until(cx, "the diff asked for #42 answered", |_| !host.calls_with(&["pr", "diff", "42"]).is_empty());
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    let (holds, diff) = screen.read_with(cx, |s, _| (s.holds_another(), s.diff().cloned()));
    assert!(!matches!(diff, Some(DiffRead::Read { .. })), "#42's diff never lands as #43's");
    assert!(!holds, "nothing of #42 is held for #43: its open file, its diff, its presses");
}

#[gpui_kit::test]
fn a_refresh_reads_the_open_file_s_diff_again(cx: &mut TestAppContext) {
    use desktop_gpui::screens::pr_pane::pull_request::DiffRead;
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    screen.update_in(cx, |s, window, cx| s.press_file("src/pane.rs".into(), window, cx));
    let read_diff = |cx: &mut VisualTestContext| screen.read_with(cx, |s, _| matches!(s.diff(), Some(DiffRead::Read { .. })));
    until(cx, "the diff read", read_diff);
    screen.update_in(cx, |s, window, cx| s.refresh(window, cx));
    until(cx, "the open file's diff read again", read_diff);
    assert_eq!(argv(&host, &["pr", "diff", "42"]).len(), 2, "the refresh asked gh for the diff again");
}

/// The open file's head is pierre's, as the Changes pane's: its counts before Open in Changes, and its title taking
/// 16% of the primary under the pointer.
#[gpui_kit::test]
fn the_open_files_head_counts_its_lines_and_its_title_answers_the_pointer(cx: &mut TestAppContext) {
    use desktop_gpui::screens::pr_pane::pull_request::DiffRead;
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    screen.update(cx, |s, cx| s.pick_tab(desktop_gpui::screens::pr_pane::pull_request::Tab::Files, cx));
    cx.run_until_parked();
    let folder = cx.debug_bounds("pr-dir-row").expect("the files' folder");
    cx.simulate_click(folder.center(), Modifiers::none());
    screen.update_in(cx, |s, window, cx| s.press_file("src/pane.rs".into(), window, cx));
    until(cx, "the diff read", |cx| screen.read_with(cx, |s, _| matches!(s.diff(), Some(DiffRead::Read { .. }))));
    cx.update(|_, cx| cx.set_reduce_motion(true));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let (counts, open) =
        (cx.debug_bounds("head-count:+6").expect("the head's +6"), cx.debug_bounds("pr-open-changes").expect("Open in Changes"));
    assert!(counts.right() <= open.left(), "the count stands before Open in Changes");
    let (rest, over) = cx.read(|cx| {
        let p = desktop_gpui::ui::ActivePalette::palette(cx);
        let (fg, primary) = (p.foreground.to_rgb(), p.primary.to_rgb());
        let mix = |a: f32, b: f32| a * 0.84 + b * 0.16;
        let mixed = gpui_kit::Rgba { r: mix(fg.r, primary.r), g: mix(fg.g, primary.g), b: mix(fg.b, primary.b), a: 1. };
        (changes::hex(p.foreground), changes::hex(mixed.into()))
    });
    let at = |ink: &str| format!("file-title:src/pane.rs:{ink}");
    let title = cx.debug_bounds(Box::leak(at(&rest).into_boxed_str())).expect("the title in the code ink");
    cx.simulate_mouse_move(title.center(), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds(Box::leak(at(&over).into_boxed_str())).is_some(), "the hovered title in {over}");
}

#[gpui_kit::test]
fn a_quote_reply_fills_the_comment_box_and_brings_it_into_view(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open_at(&host, px(700.), cx);
    read(&screen, cx);
    assert_eq!(screen.read_with(cx, |s, _| s.scrolled()), px(0.));
    let comment = screen.read_with(cx, |s, _| s.page().unwrap().comments.iter().find(|c| !c.bot).cloned().unwrap());
    screen.update_in(cx, |s, window, cx| s.quote(&comment.author, &comment.body, window, cx));
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let foot = screen.read_with(cx, |s, _| s.foot().cloned()).unwrap();
    let field = foot.read_with(cx, |f, _| f.field().clone());
    let (text, caret) = field.read_with(cx, |f, _| (f.value().to_string(), f.cursor()));
    assert!(text.starts_with(&format!("@{}\n\n> ", comment.author)), "the quote leads with who it answers: {text:?}");
    assert_eq!(caret, text.len(), "the caret stands after the quote, ready for the person's words");
    assert!(screen.read_with(cx, |s, _| s.scrolled()) > px(0.), "the pane scrolled down to the comment box");
    assert!(drawn(cx, "pr-pane-bar"), "a pane taller than its window draws its bar");
}

/// What each press reached the opener with: a link at the start of each body someone else wrote, the number, Open
/// on GitHub and the issue the work came from, the last three on `pull` and `issue` as the host's status names them.
fn presses(screen: &Entity<PullRequestScreen>, host: &PrHost, pull: &str, issue: &str, cx: &mut VisualTestContext) -> Vec<Option<String>> {
    let mut status = host.status();
    status["pr"]["url"] = pull.into();
    status["from"] = serde_json::json!({ "kind": "issue", "number": 7, "title": "Read checks", "url": issue });
    screen.update_in(cx, |s, window, cx| s.take_status(status, window, cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    LINK_SITES
        .iter()
        .map(|site| {
            let at = cx.debug_bounds(site).unwrap_or_else(|| panic!("{site} is drawn"));
            cx.update(|_, cx| cx.open_url("about:before"));
            let press = if site.starts_with("pr-markdown") { at.origin + gpui_kit::point(px(8.), px(10.)) } else { at.center() };
            cx.simulate_click(press, Modifiers::none());
            cx.run_until_parked();
            cx.opened_url().filter(|url| url != "about:before")
        })
        .collect()
}

const LINK_SITES: [&str; 7] = [
    "pr-markdown:description",
    "pr-markdown:comment:1001",
    "pr-markdown:review:2002",
    "pr-markdown:reviewComment:3001",
    "pr-number",
    "pr-open-github",
    "pr-from",
];

/// Every link and address the pane draws opens a web page on a press, and reaches the system's opener with nothing
/// else: a file, an app's scheme or a share does nothing.
#[gpui_kit::test]
fn a_link_or_address_opens_only_as_a_web_page(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("web_links") else { return };
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let opened = presses(&screen, &host, "https://example.com/pull", "https://example.com/issue", cx);
    let web = [
        "https://example.com/description",
        "https://example.com/comment",
        "https://example.com/review",
        "https://example.com/thread",
        "https://example.com/pull",
        "https://example.com/pull",
        "https://example.com/issue",
    ];
    let expected: Vec<_> = web.iter().map(|u| Some(u.to_string())).collect();
    assert_eq!(opened, expected, "each press on a web page opens it, in the order {LINK_SITES:?}");

    host.scenario("links");
    screen.update_in(cx, |s, window, cx| s.refresh(window, cx));
    until(cx, "the page read again with links that are not web pages", |cx| {
        screen.read_with(cx, |s, _| s.page().is_some_and(|p| p.body.starts_with("[open](file:")))
    });
    let opened = presses(&screen, &host, "file:///etc/pull", "vscode://issue/7", cx);
    assert_eq!(opened, vec![None; 7], "nothing but a web page reaches the opener, in the order {LINK_SITES:?}");
}

/// The sidebar and the pane read statuses through one `status.subscribe` a socket, however often they open and close,
/// and the git button, fed by the header, asks nothing; no follow of theirs outlives them.
#[gpui_kit::test]
fn the_pane_and_the_git_button_ask_for_statuses_once_a_socket(cx: &mut TestAppContext) {
    use desktop_gpui::screens::pr_pane::pull_request::git_split::GitSplit;
    use desktop_gpui::sidebar::{Link, Sidebar};
    let Some(host) = PrHost::start("open") else { return };
    let door = Door::open(&host.server);
    let client = host.runtime.block_on(HostClient::connect(&door.state, &host.server.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let client = Arc::new(client);
    let shell = desktop_gpui::screens::Host::new(client.clone(), host.runtime.clone());
    let alive = || host.runtime.metrics().num_alive_tasks();
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let (_, cx) = cx.add_window_view(|_, _| gpui_kit::EmptyView);
    let sidebar = cx.new(Sidebar::new);
    sidebar.update(cx, |s, cx| s.attach(Link { host: client.clone(), runtime: host.runtime.clone(), page: None }, cx));
    let workspace = host.workspace_id.clone();
    until(cx, "the sidebar's statuses", |cx| sidebar.read_with(cx, |s, _| s.workspace(&workspace).1.is_some()));
    // The sidebar's follow and the socket's one status reader stay from here on.
    let before = alive();
    let opened = Opened { workspace_id: Some(host.workspace_id.clone()), thread_id: Some(host.thread_id.clone()), ..Opened::default() };
    for _ in 0..5 {
        let pane = cx.update(|window, cx| cx.new(|cx| PullRequestScreen::new(shell.clone(), opened.clone(), window, cx)));
        until(cx, "the pane's status", |cx| pane.read_with(cx, |s, _| s.status().is_some()));
    }
    // A dropped entity is released at the next update, not on a park.
    cx.update(|_, _| {});
    until(cx, "only the sidebar's follow and the socket's one status reader left once the five panes are gone", |_| alive() <= before);
    // The sidebar reads the computers on a place's push too, which the button's count below would take for its own.
    drop(sidebar);
    cx.update(|_, _| {});
    // The header's button is fed what the header follows: it asks the socket nothing, however often it moves.
    let (subscribed, listed) = (door.sent("status.subscribe"), door.sent("places.list"));
    let split = cx.update(|_, cx| cx.new(|_| GitSplit::fed(shell.clone())));
    for elsewhere in ["no-such-workspace", workspace.as_str(), "no-such-workspace", workspace.as_str()] {
        split.update(cx, |s, cx| s.show(elsewhere.to_owned(), None, cx));
        cx.run_until_parked();
    }
    split.update(cx, |s, cx| s.feed(Some(&host.status()), None, &[], cx));
    assert!(split.read_with(cx, |s, _| s.state().is_some()), "the fed button reads what it was fed");
    drop(split);
    assert_eq!(door.sent("status.subscribe"), subscribed, "the fed button asked for statuses");
    assert_eq!(door.sent("places.list"), listed, "the fed button read the computers");
    assert_eq!(subscribed, 1, "the sidebar and five panes asked once on their socket");
    cx.update(|_, _| {});
    until(cx, "only the socket's one status reader left once the sidebar and the button are gone too", |_| alive() < before);
}

#[gpui_kit::test]
fn the_status_card_stands_its_parts_as_today_s_rows_do(cx: &mut TestAppContext) {
    let Some(host) = PrHost::start("open") else { return };
    let (screen, cx) = open_at(&host, px(700.), cx);
    read(&screen, cx);
    for _ in 0..2 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let bounds = |cx: &mut VisualTestContext, selector: &'static str| cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector}"));
    let mut wrong = Vec::new();
    let (checks, fold) = (bounds(cx, "pr-row:pr-checks"), bounds(cx, "pr-row-end:pr-checks"));
    if (fold.top() - checks.top(), fold.size.height) != (px(14.), px(17.5)) {
        wrong.push(format!(
            "the fold's chevron box stands {:?} down the row, {:?} tall, not 14 px and 2.5 px over its 15",
            fold.top() - checks.top(),
            fold.size.height
        ));
    }
    let (review, eye) = (bounds(cx, "pr-row:pr-review"), bounds(cx, "pr-row-end:pr-review"));
    if eye.size.height != px(28.) {
        wrong.push(format!("the review's eye is {:?} tall, not 28 px", eye.size.height));
    }
    let faces = bounds(cx, "pr-verdict-faces");
    if ((faces.top() - review.top()) - px(14. + 5.25)).abs() > px(0.5) {
        wrong.push(format!("the review's faces stand {:?} down the row, not 5.25 px under its 14 px padding", faces.top() - review.top()));
    }
    let (pane, bar, card) = (bounds(cx, "pr-pane"), bounds(cx, "pr-pane-bar"), bounds(cx, "pr-status"));
    if (pane.right() - bar.right(), bar.size.width) != (px(0.), px(6.)) {
        wrong.push(format!(
            "the bar stands {:?} in from the pane's edge, {:?} wide, before any scroll",
            pane.right() - bar.right(),
            bar.size.width
        ));
    }
    if pane.right() - card.right() != px(22.) {
        wrong.push(format!(
            "the card ends {:?} in from the pane's edge, not 16 px in from the bar's 6 px lane",
            pane.right() - card.right()
        ));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
}
