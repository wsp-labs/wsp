// SPDX-License-Identifier: AGPL-3.0-only
//! The thread screen as the frame mounts it, against a host started for this test alone: one screen kept across a
//! switch, its transcript and its composer moved to each thread, a quote landing in the composer, and the list ending
//! where the docked composer stands.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::composer::model::Hold;
use desktop_gpui::screens::pr_pane::files::open_split;
use desktop_gpui::screens::pr_pane::pull_request::git_split::{quick_action, GitAct};
use desktop_gpui::screens::thread_view::header;
use desktop_gpui::screens::{entry, Host, Opened, Pane, Place, Route, Screen};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::thread_view::ThreadViewEvent;
use desktop_gpui::transcript::RowKind;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    point, prelude::*, px, size, AppContext, Entity, ExternalPaths, FileDropEvent, Focusable as _, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, Pixels, PlatformInput, Point, TestAppContext, VisualTestContext, Window,
};
use wsp_host_client::{protocol, HostClient};

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn shown(shell: &Entity<Shell>, cx: &mut VisualTestContext, thread: &str, said: &str) -> bool {
    shell.read_with(cx, |s, cx| {
        let Some(screen) = s.thread_screen() else { return false };
        let view = screen.read(cx).view().read(cx);
        view.thread() == Some(thread) && view.rows().iter().any(|r| matches!(r.kind, RowKind::Person { .. }) && r.text == said)
    })
}

#[gpui_kit::test]
fn one_screen_moves_with_its_composer_between_threads_and_clears_the_dock(cx: &mut TestAppContext) {
    let mut host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    host.wsp(&["run", "repo", "second", "--agent", "claude"]);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let id = |prompt: &str| {
        let t = threads.iter().find(|t| t.prompt.as_deref() == Some(prompt)).unwrap_or_else(|| panic!("no thread {prompt}"));
        (t.workspace_id.clone(), t.thread_id.clone().expect("a thread id"))
    };
    let ((wa, a), (wb, b)) = (id("first"), id("second"));

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let (state, vars) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, vars, runtime.clone(), Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));

    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: wa.clone(), thread_id: a.clone() }, window, cx));
    until(cx, "the first thread", |cx| shown(&shell, cx, &a, "first"));
    let screen = shell.read_with(cx, |s, _| s.thread_screen()).expect("the thread screen mounted");
    let composer = screen.read_with(cx, |s, _| s.composer().clone());
    until(cx, "the composer's link", |cx| composer.read_with(cx, |c, _| c.hold().is_none()));

    // A key typed after a click into a reply, which focuses the reply's text, lands in the box as useTypeToWrite has it.
    let reply = screen.read_with(cx, |s, cx| {
        let view = s.view().read(cx);
        view.rows().iter().find(|r| matches!(r.kind, RowKind::Agent { .. })).and_then(|r| view.body_of(&r.key))
    });
    let reply = reply.expect("the reply drawn");
    cx.update(|window, cx| reply.read(cx).focus_handle().clone().focus(window, cx));
    cx.simulate_keystrokes("q->q");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "q", "a key typed over a focused reply went nowhere");

    // The list ends with the docked composer's drawn height, which grows with a draft of five lines.
    composer.update_in(cx, |c, window, cx| c.set_text("one\ntwo\nthree\nfour\nfive", window, cx));
    until(cx, "the dock to grow past its empty height", |cx| cx.debug_bounds("thread-dock").is_some_and(|b| b.size.height > px(110.)));
    until(cx, "the list to take the dock's height", |cx| {
        let dock = cx.debug_bounds("thread-dock").map(|b| b.size.height);
        screen.read_with(cx, |s, cx| Some(s.view().read(cx).composer_inset()) == dock)
    });

    // Quote over a reply hands the composer the quote on lines of its own after the draft.
    let view = screen.read_with(cx, |s, _| s.view().clone());
    view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Quote("Quoting your reply to \"first\":\n> Done.".to_owned())));
    cx.run_until_parked();
    assert_eq!(
        composer.read_with(cx, |c, cx| c.text(cx)),
        "one\ntwo\nthree\nfour\nfive\nQuoting your reply to \"first\":\n> Done.\n",
        "Quote did not reach the composer"
    );

    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: wb.clone(), thread_id: b.clone() }, window, cx));
    until(cx, "the second thread", |cx| shown(&shell, cx, &b, "second"));
    let kept = shell.read_with(cx, |s, _| s.thread_screen()).expect("the thread screen");
    assert_eq!(kept.entity_id(), screen.entity_id(), "a switch moves the kept screen rather than build another");
    assert_eq!(kept.read_with(cx, |s, _| s.composer().entity_id()), composer.entity_id(), "one composer for the kept screen");
    until(cx, "the composer's link to the second thread", |cx| composer.read_with(cx, |c, _| c.hold().is_none()));
    assert_eq!(
        composer.read_with(cx, |c, _| c.thread_key().map(str::to_owned)),
        Some(b.clone()),
        "the composer holds the second thread's key"
    );

    // The host goes away: the docked composer holds the send until the view follows the thread again.
    host.stop();
    until(cx, "the composer to hold on the lost host", |cx| composer.read_with(cx, |c, _| c.hold() == Some(&Hold::HostDown)));
    host.restart();
    until(cx, "the composer to open again", |cx| composer.read_with(cx, |c, _| c.hold().is_none()));

    // No thread open: the kept screen stops following the last one.
    assert_eq!(
        screen.read_with(cx, |s, cx| s.view().read(cx).followed().map(str::to_owned)),
        Some(b.clone()),
        "it follows the second thread"
    );
    shell.update_in(cx, |s, window, cx| s.go(Route::Empty, window, cx));
    until(cx, "the follow to stop", |cx| screen.read_with(cx, |s, cx| s.view().read(cx).followed().is_none()));
}

/// The stand-in claude of the shared test host, for a host started here with a wsp-daemon.
const STAND_IN: &str = r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{"type":"control_response","response":{"subtype":"success","request_id":"init","response":{"models":[{"value":"default","resolvedModel":"claude-haiku-5-5"}]}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && session=$a; prev=$a; done
while IFS= read -r _; do
  printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}\n' "$session"
  printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]}}\n' "$session"
  printf '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"%s"}\n' "$session"
done
"#;

/// Each thread the host lists: its first prompt, its workspace and its id.
type Listed = Vec<(String, String, String)>;

/// The app's shell in a window against `host`, connected, and the threads the host lists by their first prompt.
fn shell_on<'a>(host: &common::Throwaway, cx: &'a mut TestAppContext) -> (Entity<Shell>, Listed, &'a mut VisualTestContext) {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = threads
        .iter()
        .map(|t| (t.prompt.clone().unwrap_or_default(), t.workspace_id.clone(), t.thread_id.clone().expect("a thread id")))
        .collect();
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let (state, vars) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, vars, runtime.clone(), Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    (shell, threads, cx)
}

fn ids(threads: &[(String, String, String)], prompt: &str) -> (String, String) {
    let (_, workspace, thread) = threads.iter().find(|(p, _, _)| p == prompt).unwrap_or_else(|| panic!("no thread {prompt}"));
    (workspace.clone(), thread.clone())
}

fn open(shell: &Entity<Shell>, cx: &mut VisualTestContext, (workspace_id, thread_id): (String, String), said: &str) {
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id, thread_id: thread_id.clone() }, window, cx));
    until(cx, said, |cx| shown(shell, cx, &thread_id, said));
}

#[gpui_kit::test]
fn a_switch_names_the_thread_before_the_host_answers(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    host.wsp(&["run", "repo", "second", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    open(&shell, cx, ids(&threads, "first"), "first");
    open(&shell, cx, ids(&threads, "second"), "second");
    for prompt in ["first", "second", "first"] {
        let (workspace_id, thread_id) = ids(&threads, prompt);
        shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id, thread_id }, window, cx));
        // The frame right after the switch, before any answer of the host is let in.
        cx.update(|window, cx| window.render_frame(cx));
        assert!(cx.debug_bounds("crumbs-title").is_some(), "the switch to {prompt} drew its title row blank");
        until(cx, prompt, |cx| shell.read_with(cx, |s, cx| s.thread_screen().is_some_and(|t| t.read(cx).view().read(cx).loaded())));
    }
}

fn bar_of(cx: &mut VisualTestContext, thread: &str) -> Entity<header::Bar> {
    cx.update(|_, cx| header::bar(thread, cx))
}

fn double_click(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.update(|window, cx| {
        for click_count in [1, 2] {
            let (modifiers, button) = (Modifiers::none(), MouseButton::Left);
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent { position: at, modifiers, button, click_count, first_mouse: false }),
                cx,
            );
            window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent { position: at, modifiers, button, click_count }), cx);
        }
    });
}

fn press(cx: &mut VisualTestContext, name: &'static str) {
    let at = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} was not drawn")).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn title_of(bar: &Entity<header::Bar>, cx: &mut VisualTestContext) -> Option<String> {
    bar.read_with(cx, |b, _| b.row.as_ref().map(|r| header::crumb(r).title))
}

#[gpui_kit::test]
fn a_double_click_on_the_title_opens_its_name_box_that_sends_the_name_and_escape_leaves_it(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    let (workspace, thread) = ids(&threads, "first");
    open(&shell, cx, (workspace, thread.clone()), "first");
    let bar = bar_of(cx, &thread);
    until(cx, "the crumb's title", |cx| cx.debug_bounds("crumbs-title").is_some() && title_of(&bar, cx).is_some());
    assert_eq!(title_of(&bar, cx).as_deref(), Some("first"));
    assert!(cx.debug_bounds("crumbs-project").is_some(), "the crumbs name the project");

    let title = cx.debug_bounds("crumbs-title").expect("the title");
    double_click(cx, title.center());
    cx.run_until_parked();
    assert_eq!(bar.read_with(cx, |b, cx| b.rename_box(cx)).as_deref(), Some("first"), "the box opens on the title");
    let field = cx.debug_bounds("crumbs-rename").expect("the box drawn in the title's place");
    assert_eq!(field.size.height, px(24.));
    cx.simulate_input("Cart fix");
    assert_eq!(bar.read_with(cx, |b, cx| b.rename_box(cx)).as_deref(), Some("Cart fix"), "the typing replaces the selected name");
    // The name goes through the agent's session, which the stand-in keeps none of: the host's refusal is a notice, and
    // the box stays with the typed name in it.
    cx.simulate_keystrokes("enter");
    until(cx, "the host's answer to the rename", |cx| !refusals(cx).is_empty());
    assert_eq!(refusals(cx), ["Claude Code on the task has no session for this thread yet"]);
    assert_eq!(bar.read_with(cx, |b, cx| b.rename_box(cx)).as_deref(), Some("Cart fix"), "the box stays with the name in it");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(bar.read_with(cx, |b, cx| b.rename_box(cx)), None, "Escape left the box open");

    double_click(cx, title.center());
    cx.run_until_parked();
    cx.simulate_input("not this");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(bar.read_with(cx, |b, cx| b.rename_box(cx)), None, "Escape left the box open");
    assert!(cx.debug_bounds("crumbs-rename").is_none() && cx.debug_bounds("crumbs-title").is_some());
    assert_eq!(title_of(&bar, cx).as_deref(), Some("first"), "Escape renamed the thread");

    // The title's menu holds Rename, which opens the same box. On the Mac the menu is AppKit's, which the test
    // platform cannot pop.
    if cfg!(target_os = "macos") {
        return;
    }
    let title = cx.debug_bounds("crumbs-title").expect("the title");
    cx.simulate_mouse_down(title.center(), MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(title.center(), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(bar.read_with(cx, |b, cx| b.rename_box(cx)).as_deref(), Some("first"), "the menu's Rename opened no box");
}

/// Two panes the frame can open, standing in for the Changes and Pull request screens, which this crate does not build.
macro_rules! pane {
    ($ty:ident, $pane:expr) => {
        struct $ty;
        impl Render for $ty {
            fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
                gpui_kit::div()
            }
        }
        impl Screen for $ty {
            const NAME: &'static str = "pane";
            const PLACE: Place = Place::Panel($pane);
            fn new(_: Host, _: Opened, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> Self {
                $ty
            }
        }
    };
}

pane!(ChangesPane, Pane::Changes);
pane!(PrPane, Pane::PullRequest);

#[gpui_kit::test]
fn the_git_split_follows_the_workspace_and_its_menu_opens_changes_and_the_pull_request(cx: &mut TestAppContext) {
    let host = common::Throwaway::start_with(STAND_IN, true);
    if host.daemon.is_none() {
        println!("skipped the git split, which the host reads through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    }
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    desktop_gpui::screens::add(Pane::Changes.name(), entry::<ChangesPane>());
    desktop_gpui::screens::add(Pane::PullRequest.name(), entry::<PrPane>());
    let (shell, threads, cx) = shell_on(&host, cx);
    let (workspace_id, thread) = ids(&threads, "first");
    open(&shell, cx, (workspace_id.clone(), thread.clone()), "first");
    let bar = bar_of(cx, &thread);
    until(cx, "the git split", |cx| cx.debug_bounds("git-split").is_some());
    let changed =
        |cx: &mut VisualTestContext| bar.read_with(cx, |b, _| b.status.as_ref().and_then(|s| s.pointer("/checkout/changed")?.as_i64()));
    assert_eq!(changed(cx), Some(0), "a fresh copy holds no change");

    // A change in the copy reaches the split through the host's pushed status, with no read of the split's own.
    let copy =
        bar.read_with(cx, |b, _| b.status.as_ref().and_then(|s| s["folder"].as_str().map(str::to_owned))).expect("the copy's folder");
    std::fs::write(std::path::Path::new(&copy).join("cart.txt"), "fixed\n").unwrap();
    // Another window asks the host to read the checkout again, as a turn's end does; the host pushes what it read.
    let reader = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    reader.block_on(async {
        let ask: protocol::WorkspacesCheckoutRequest =
            serde_json::from_value(serde_json::json!({ "workspaceId": workspace_id, "fresh": true })).unwrap();
        host.client().await.call(ask).await.unwrap_or_else(|f| panic!("workspaces.checkout: {}", f.message));
    });
    until(cx, "the pushed status to count the change", |cx| changed(cx).is_some_and(|n| n > 0));

    // The menu is the app's popup: the first Down lands on Commit, Enter runs it.
    press(cx, "git-more");
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    let active = |cx: &mut VisualTestContext| shell.read_with(cx, |s, cx| s.panel().read(cx).active());
    assert_eq!(active(cx), Some(Pane::Changes), "the menu's Commit opens the Changes pane");

    bar.update(cx, |b, cx| {
        if let Some(status) = b.status.as_mut() {
            status["pr"] = serde_json::json!({ "number": 7, "state": "open", "url": "https://example.com/pr/7", "base": "main" });
        }
        cx.notify();
    });
    cx.run_until_parked();
    let git = shell.read_with(cx, |s, cx| s.thread_screen().map(|t| t.read(cx).git_split().clone())).expect("the thread screen");
    until(cx, "the button to read the open pull request", |cx| git.read_with(cx, |g, _| g.state().is_some_and(|s| s.pr_open)));
    // Commit, then View PR past the held Push.
    press(cx, "git-more");
    cx.simulate_keystrokes("down down enter");
    cx.run_until_parked();
    assert_eq!(active(cx), Some(Pane::PullRequest), "View PR opens the Pull request pane");
}

/// The refusals standing in the window's toasts, oldest first, but the readings of this computer the throwaway host
/// refuses for want of a wsp-daemon.
fn refusals(cx: &mut VisualTestContext) -> Vec<String> {
    let toasts = cx.update(|window, cx| desktop_gpui::ui::toast::of(window, cx).expect("the window's toasts"));
    toasts.read_with(cx, |t, _| {
        t.standing()
            .into_iter()
            .filter(|n| n.kind == desktop_gpui::ui::NoticeKind::Error && !n.text.starts_with("Live readings for "))
            .map(|n| n.text.to_string())
            .collect()
    })
}

/// Moves the repo's folder away, so the host refuses an open there before it looks for an editor to start.
fn move_away(folder: &std::path::Path) {
    std::fs::rename(folder, folder.with_extension("moved")).expect("the repo's folder moved away");
}

#[gpui_kit::test]
fn a_refused_open_says_the_hosts_sentence_in_the_windows_toasts(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    let (workspace, thread) = ids(&threads, "first");
    open(&shell, cx, (workspace, thread.clone()), "first");
    let bar = bar_of(cx, &thread);
    until(cx, "the status row", |cx| bar.read_with(cx, |b, _| b.status.is_some()));
    move_away(&folder);
    let screen = shell.read_with(cx, |s, _| s.thread_screen()).expect("the thread screen");
    let composer = screen.read_with(cx, |s, _| s.composer().clone());
    until(cx, "the composer's link", |cx| composer.read_with(cx, |c, _| c.hold().is_none()));
    cx.update(|window, cx| composer.read(cx).focus_handle(cx).focus(window, cx));
    cx.simulate_keystrokes("secondary-o");
    until(cx, "the refusal", |cx| refusals(cx).len() == 1);
    assert!(refusals(cx)[0].contains("is not in this workspace's folder"), "the toast said {:?}", refusals(cx));
    assert!(cx.debug_bounds("thread-notice").is_none(), "the header drew a notice of its own");

    // mod+o opens the thread's copy from anywhere in the window, the sidebar too.
    let view = screen.read_with(cx, |s, _| s.view().clone());
    let side = cx.debug_bounds("sidebar").expect("the sidebar");
    cx.simulate_click(point(side.center().x, side.top() + px(200.)), Modifiers::none());
    cx.run_until_parked();
    let in_screen = cx.update(|window, cx| {
        composer.read(cx).focus_handle(cx).contains_focused(window, cx) || view.read(cx).focus_handle(cx).contains_focused(window, cx)
    });
    assert!(!in_screen, "the press on the sidebar left the keys in the thread screen");
    cx.simulate_keystrokes("secondary-o");
    until(cx, "the refusal from the sidebar", |cx| refusals(cx).len() == 2);
}

#[gpui_kit::test]
fn the_editors_are_listed_once_per_host_not_per_thread(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    host.wsp(&["run", "repo", "second", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    open(&shell, cx, ids(&threads, "first"), "first");
    until(cx, "the host's editors", |cx| cx.update(|_, cx| cx.try_global::<open_split::Editors>().is_some_and(|e| e.list.is_some())));
    let mark = open_split::Editor { id: "mark".to_owned(), name: "Mark".to_owned(), remote: false };
    cx.update(|_, cx| open_split::set_editors(Some(vec![mark.clone()]), None, cx));
    let (workspace, second) = ids(&threads, "second");
    open(&shell, cx, (workspace, second.clone()), "second");
    let bar = bar_of(cx, &second);
    until(cx, "the second thread's catalogs", |cx| bar.read_with(cx, |b, _| !b.catalogs.is_empty()));
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    let listed = cx.update(|_, cx| cx.global::<open_split::Editors>().list.clone());
    assert_eq!(listed, Some(vec![mark]), "a thread switch listed the editors again");
}

#[gpui_kit::test]
fn a_strip_the_listing_holds_stands_over_the_composer_in_the_dock(cx: &mut TestAppContext) {
    use desktop_gpui::screens::thread_view::ThreadScreen;
    use desktop_gpui::transcript::{Listing, Strip};
    let host = common::Throwaway::start();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| <ThreadScreen as Screen>::new(Host::new(client, runtime), Opened::default(), window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    let screen = screen.unwrap();
    cx.simulate_resize(size(px(1024.), px(700.)));
    let view = screen.read_with(cx, |s, _| s.view().clone());
    let strip = Strip {
        key: "released",
        title: "Its machine was let go".to_owned(),
        note: "The work it pushed stays".to_owned(),
        acts: Vec::new(),
    };
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(Vec::new(), cx);
        v.set_listing(Listing { thread: "t".into(), strip: Some(strip), ..Listing::default() }, cx);
    });
    cx.run_until_parked();
    let shown = cx.debug_bounds("sub-strip-released").expect("the strip the listing holds is not drawn");
    let dock = cx.debug_bounds("thread-dock").expect("the dock");
    assert_eq!(shown.top(), dock.top(), "the strip stands first in the dock");
    assert!(dock.bottom() - shown.bottom() > px(60.), "the composer stands under the strip");
    assert_eq!(view.read_with(cx, |v, _| v.composer_inset()), dock.size.height, "the list ends clear of the strip and the composer");
}

#[gpui_kit::test]
fn a_commit_draft_answers_only_the_thread_it_was_pressed_on(cx: &mut TestAppContext) {
    // A stand-in whose draft of a commit message takes a while and comes back with none.
    let slow = STAND_IN.replace("*) exit 1 ;;", "*) sleep 6; exit 1 ;;");
    let host = common::Throwaway::start_with(&slow, true);
    if host.daemon.is_none() {
        println!("skipped the commit draft, which the host reads through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    }
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    let git = |args: &[&str]| {
        let ok = std::process::Command::new("git").args(args).current_dir(&folder).env_clear().envs(host.env()).status().unwrap().success();
        assert!(ok, "git {args:?}");
    };
    // Edited before the turns: a fresh read asked while a turn's end read runs is answered by that older read.
    git(&["checkout", "-q", "-b", "fix/cart"]);
    std::fs::write(folder.join("cart.txt"), "fixed\n").unwrap();
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    host.wsp(&["run", "repo", "second", "--agent", "claude"]);
    desktop_gpui::screens::add(Pane::Changes.name(), entry::<ChangesPane>());
    let (shell, threads, cx) = shell_on(&host, cx);
    open(&shell, cx, ids(&threads, "first"), "first");
    let git = shell.read_with(cx, |s, cx| s.thread_screen().map(|t| t.read(cx).git_split().clone())).expect("the thread screen");
    until(cx, "the quick act to commit", |cx| {
        git.read_with(cx, |g, _| g.state().is_some_and(|s| quick_action(&s).act == Some(GitAct::CommitPushPr)))
    });
    // Open holds its slot right of the split until the host lists its editors, and the split moves when it stands down.
    until(cx, "the host's editors", |cx| cx.update(|_, cx| cx.try_global::<open_split::Editors>().is_some_and(|e| e.list.is_some())));
    let active = |cx: &mut VisualTestContext| shell.read_with(cx, |s, cx| s.panel().read(cx).active());

    // With no draft the Changes pane opens on the thread pressed on.
    press(cx, "git-quick");
    until(cx, "the Changes pane", |cx| active(cx) == Some(Pane::Changes));

    // Pressed again and left before the draft answers, the next thread's panel stays as it was.
    until(cx, "the act to end", |cx| git.read_with(cx, |g, _| !g.busy()));
    // The draft's note enters over the header row on the wall clock, and takes presses there until it stands under it.
    let toasts = cx.update(|window, cx| desktop_gpui::ui::toast::of(window, cx).expect("the window's toasts"));
    // The stack sizes a toast on the draw after it is added, which nothing else in this window asks for.
    toasts.update(cx, |_, cx| cx.notify());
    for _ in 0..30 {
        cx.executor().advance_clock(Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(20));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    press(cx, "git-quick");
    assert!(git.read_with(cx, |g, _| g.busy()), "the act did not start");
    open(&shell, cx, ids(&threads, "second"), "second");
    until(cx, "the draft's answer", |cx| git.read_with(cx, |g, _| !g.busy()));
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(active(cx), None, "a draft answered after a switch opened Changes on the next thread");
}

#[gpui_kit::test]
fn mod_o_on_settings_leaves_the_thread_behind_it_alone(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    let (workspace, thread) = ids(&threads, "first");
    open(&shell, cx, (workspace.clone(), thread.clone()), "first");
    let bar = bar_of(cx, &thread);
    until(cx, "the status row", |cx| bar.read_with(cx, |b, _| b.status.is_some()));
    move_away(&folder);
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(desktop_gpui::shell::SETTINGS.into()), window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("secondary-o");
    let opening = cx.update(|window, cx| desktop_gpui::screens::notices::opening(&workspace, window, cx));
    assert!(!opening, "mod+o on Settings asked the host to open the thread behind it");
    let settle = Instant::now() + Duration::from_secs(3);
    while Instant::now() < settle {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    open(&shell, cx, (workspace, thread.clone()), "first");
    cx.run_until_parked();
    assert!(refusals(cx).is_empty(), "mod+o on Settings acted on the thread behind it");
    let screen = shell.read_with(cx, |s, _| s.thread_screen()).expect("the thread screen");
    let composer = screen.read_with(cx, |s, _| s.composer().clone());
    cx.update(|window, cx| composer.read(cx).focus_handle(cx).focus(window, cx));
    cx.simulate_keystrokes("secondary-o");
    until(cx, "the refusal back on the thread", |cx| refusals(cx).len() == 1);
}

/// Catches: the drop frame drawn over the composer's box alone, drawn while a file is over the sidebar, and a file let
/// go over the transcript, away from the box, not attached (`ChatDropZone.tsx` takes it anywhere on the chat).
#[gpui_kit::test]
fn a_file_dropped_anywhere_on_the_chat_attaches_under_a_frame_over_the_whole_chat(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    open(&shell, cx, ids(&threads, "first"), "first");
    let screen = shell.read_with(cx, |s, _| s.thread_screen()).expect("the thread screen");
    let composer = screen.read_with(cx, |s, _| s.composer().clone());
    until(cx, "the composer to take files", |cx| composer.read_with(cx, |c, _| !c.hold().is_some_and(Hold::shuts)));
    let file = folder.join("dropped.txt");
    std::fs::write(&file, "dropped").unwrap();
    let center = cx.debug_bounds("center").expect("the main column");
    let header = cx.debug_bounds("header-row").expect("the header");
    let sidebar = cx.debug_bounds("sidebar-column").expect("the sidebar");
    let transcript = point(center.left() + px(120.), header.bottom() + px(60.));

    cx.simulate_event(FileDropEvent::Entered { position: transcript, paths: ExternalPaths(vec![file].into()) });
    cx.run_until_parked();
    let frame = cx.debug_bounds("chat-drop").expect("no drop frame with a file over the transcript");
    assert_eq!(
        (frame.left(), frame.top(), frame.size.width, frame.bottom()),
        (center.left(), header.bottom(), center.size.width, center.bottom()),
        "the drop frame covers {frame:?}, not the chat under the header in {center:?}"
    );

    cx.simulate_event(FileDropEvent::Pending { position: sidebar.center() });
    cx.run_until_parked();
    assert!(cx.debug_bounds("chat-drop").is_none(), "the drop frame stands with the file over the sidebar");

    cx.simulate_event(FileDropEvent::Pending { position: transcript });
    cx.run_until_parked();
    assert!(cx.debug_bounds("chat-drop").is_some(), "the drop frame did not come back over the transcript");
    cx.simulate_event(FileDropEvent::Submit { position: transcript });
    until(cx, "the dropped file in the composer", |cx| composer.read_with(cx, |c, _| c.files().len() == 1));
    cx.simulate_event(FileDropEvent::Exited);
    cx.run_until_parked();
    assert!(cx.debug_bounds("chat-drop").is_none(), "the drop frame outlived the drop");

    // A file carried over the chat and out of the window, never dropped, takes the frame with it.
    cx.simulate_event(FileDropEvent::Entered { position: transcript, paths: ExternalPaths(vec![folder.join("left.txt")].into()) });
    cx.run_until_parked();
    assert!(cx.debug_bounds("chat-drop").is_some(), "no drop frame with a second file over the transcript");
    cx.simulate_event(FileDropEvent::Exited);
    cx.run_until_parked();
    assert!(cx.debug_bounds("chat-drop").is_none(), "the drop frame stands after the file left the window");

    // A piece over the transcript that keeps the pointer to itself, the find bar here, still lets the chat take a drop.
    let view = screen.read_with(cx, |s, _| s.view().clone());
    view.update_in(cx, desktop_gpui::screens::thread_view::find::open);
    cx.run_until_parked();
    let bar = cx.debug_bounds("find-bar").expect("the find bar").center();
    let again = folder.join("again.txt");
    std::fs::write(&again, "again").unwrap();
    cx.simulate_event(FileDropEvent::Entered { position: bar, paths: ExternalPaths(vec![again].into()) });
    cx.run_until_parked();
    cx.simulate_event(FileDropEvent::Pending { position: bar });
    cx.run_until_parked();
    cx.simulate_event(FileDropEvent::Submit { position: bar });
    until(cx, "the file dropped over the find bar in the composer", |cx| composer.read_with(cx, |c, _| c.files().len() == 2));
    cx.simulate_event(FileDropEvent::Exited);
}

/// Catches: the New thread page taking a file only over the composer's box (`ChatDropZone.tsx` takes it anywhere on
/// the page, `[data-chat-view]`).
#[gpui_kit::test]
fn a_file_dropped_anywhere_on_the_new_thread_page_attaches(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    let (workspace, _) = ids(&threads, "first");
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(format!("composer:{workspace}").into()), window, cx));
    until(cx, "the New thread page", |cx| cx.debug_bounds("composer-box").is_some());
    let file = folder.join("dropped.txt");
    std::fs::write(&file, "dropped").unwrap();
    let center = cx.debug_bounds("center").expect("the main column");
    let header = cx.debug_bounds("header-row").expect("the header");
    let high = point(center.left() + px(120.), header.bottom() + px(60.));
    cx.simulate_event(FileDropEvent::Entered { position: high, paths: ExternalPaths(vec![file].into()) });
    // The page's composer takes no file until it is tied to its workspace.
    until(cx, "the drop frame over the page", |cx| {
        cx.simulate_event(FileDropEvent::Pending { position: high });
        cx.debug_bounds("chat-drop").is_some()
    });
    let frame = cx.debug_bounds("chat-drop").unwrap();
    assert_eq!((frame.left(), frame.top(), frame.size.width), (center.left(), header.bottom(), center.size.width), "{frame:?}");
    cx.simulate_event(FileDropEvent::Submit { position: high });
    until(cx, "the dropped file in the composer", |cx| cx.debug_bounds("composer-files").is_some());
    assert!(cx.debug_bounds("chat-drop").is_none(), "the drop frame outlived the drop");
}

/// Catches: `--drop <x>,<y>` opening the thread with no file held over it, so the capture shoots no drop frame.
#[gpui_kit::test]
fn a_capture_holds_a_file_over_the_chat_where_drop_says(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    let capture = desktop_gpui::screens::thread_view::Capture { drop: Some((760., 300.)), ..Default::default() };
    cx.update(|_, cx| cx.set_global(capture));
    open(&shell, cx, ids(&threads, "first"), "first");
    until(cx, "the held drop frame", |cx| cx.debug_bounds("chat-drop").is_some());
    let (frame, center) = (cx.debug_bounds("chat-drop").unwrap(), cx.debug_bounds("center").unwrap());
    assert_eq!((frame.left(), frame.size.width), (center.left(), center.size.width), "{frame:?}");
}
