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
use std::time::{Duration, Instant};

use desktop_gpui::screens::pr_pane::pull_request::model::{self, MergeMethod, Seen, State};
use desktop_gpui::screens::pr_pane::pull_request::PullRequestScreen;
use desktop_gpui::screens::{Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, Modifiers, TestAppContext, VisualTestContext};
use pr_page_host::{PrHost, WAIT};
use serde_json::Value;

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

/// The argv of every call the stub was given holding all of `words`.
fn argv(host: &PrHost, words: &[&str]) -> Vec<Vec<String>> {
    host.calls_with(words)
        .into_iter()
        .map(|c| c["argv"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_owned()).collect())
        .collect()
}

#[gpui_kit::test]
fn an_open_pull_request_reads_its_page_off_gh(cx: &mut TestAppContext) {
    let host = PrHost::start("open");
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
    let host = PrHost::start("merged");
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    assert_eq!(seen(&screen, cx).map(|s| s.state()), Some(State::Merged));
    assert_eq!(screen.read_with(cx, |s, _| s.page().and_then(|p| p.merged_by.clone())).as_deref(), Some("ana"));
    for row in ["pr-row:pr-settled", "pr-row:pr-landed", "pr-row:pr-checks"] {
        assert!(drawn(cx, row), "{row} is drawn on a merged pull request");
    }
    assert!(!drawn(cx, "pr-row:pr-hold"), "a settled pull request holds no merge");
    assert!(!drawn(cx, "pr-row:pr-checks-passed"), "its checks stand folded");
}

#[gpui_kit::test]
fn a_closed_pull_request_says_it_closed_unmerged(cx: &mut TestAppContext) {
    let host = PrHost::start("closed");
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    let seen = seen(&screen, cx).unwrap();
    assert_eq!((seen.state(), model::tone_of(&seen).0), (State::Closed, "Closed".into()));
    assert!(drawn(cx, "pr-row:pr-settled"));
    assert!(!drawn(cx, "pr-row:pr-landed"), "nothing landed");
}

#[gpui_kit::test]
fn a_branch_with_no_pull_request_shows_the_pane_s_shape(cx: &mut TestAppContext) {
    let host = PrHost::start("none");
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
    let host = PrHost::start("open");
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
    let host = PrHost::start("open");
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
    let host = PrHost::start("open");
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
    let host = PrHost::start("open");
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
    let host = PrHost::start("open");
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

fn fact_head(screen: &Entity<PullRequestScreen>, cx: &mut VisualTestContext) -> String {
    match seen(screen, cx) {
        Some(Seen::Fact(f)) => f.head_oid,
        other => panic!("no fact: {other:?}"),
    }
}

#[gpui_kit::test]
fn an_update_merges_the_base_into_the_branch(cx: &mut TestAppContext) {
    let host = PrHost::start("open");
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
    assert!(host.folder.join("NOTES.md").exists(), "main's commit is on the branch now");
}

#[gpui_kit::test]
fn a_reaction_gh_refuses_goes_back(cx: &mut TestAppContext) {
    let host = PrHost::start("open");
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
}

#[gpui_kit::test]
fn a_status_push_redraws_the_status_card(cx: &mut TestAppContext) {
    let host = PrHost::start("open");
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
fn the_reaction_picker_stays_open_on_a_press_inside_and_closes_on_escape(cx: &mut TestAppContext) {
    let host = PrHost::start("open");
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
    let host = PrHost::start("open");
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
    let host = PrHost::start("open");
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let shell = host.host();
    let (workspace, thread) = (host.workspace_id.clone(), host.thread_id.clone());
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let split = cx.new(|cx| GitSplit::new(shell, workspace, Some(thread), window, cx));
        made = Some(split.clone());
        Root::new(split, window, cx)
    });
    let split = made.unwrap();
    until(cx, "the branch's pull request on the button's status", |cx| {
        split.read_with(cx, |s, _| s.state().is_some_and(|st| st.pr_open && st.base.is_some()))
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
        split.read_with(cx, |s, _| s.state().and_then(|st| st.checkout).is_some_and(|c| c.changed > 0))
    });
    let quick = split.read_with(cx, |s, _| quick_action(&s.state().unwrap()));
    assert_eq!(quick.act, Some(GitAct::CommitPush), "a change on a branch whose pull request is open commits and pushes");
}

#[gpui_kit::test]
fn another_pull_request_drops_what_was_read_for_the_last(cx: &mut TestAppContext) {
    use desktop_gpui::screens::pr_pane::pull_request::DiffRead;
    let host = PrHost::start("open");
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
    let host = PrHost::start("open");
    let (screen, cx) = open(&host, cx);
    read(&screen, cx);
    screen.update_in(cx, |s, window, cx| s.press_file("src/pane.rs".into(), window, cx));
    let read_diff = |cx: &mut VisualTestContext| screen.read_with(cx, |s, _| matches!(s.diff(), Some(DiffRead::Read { .. })));
    until(cx, "the diff read", read_diff);
    screen.update_in(cx, |s, window, cx| s.refresh(window, cx));
    until(cx, "the open file's diff read again", read_diff);
    assert_eq!(argv(&host, &["pr", "diff", "42"]).len(), 2, "the refresh asked gh for the diff again");
}

#[gpui_kit::test]
fn a_quote_reply_fills_the_comment_box_and_brings_it_into_view(cx: &mut TestAppContext) {
    let host = PrHost::start("open");
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
}
