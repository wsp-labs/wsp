// SPDX-License-Identifier: AGPL-3.0-only
//! The thread view's listing off the sidebar's model, against a host started for this test alone: the view reads no
//! thread list of its own, and the agent's catalog flags still reach its rows once the page is drawn.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::thread_view::ThreadScreen;
use desktop_gpui::screens::{Host, Opened, Route, Screen};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::transcript::{Listing, RowKind, StartFailed};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, Modifiers, TestAppContext, VisualTestContext};
use wsp_host_client::HostClient;

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

fn view_of(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Entity<desktop_gpui::thread_view::ThreadView>> {
    shell.read_with(cx, |s, cx| s.thread_screen().map(|screen| screen.read(cx).view().clone()))
}

#[gpui_kit::test]
fn the_listing_comes_from_the_sidebar_and_the_catalog_reaches_the_rows(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let first = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread");
    let (workspace, thread) = (first.workspace_id.clone(), first.thread_id.clone().expect("a thread id"));

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
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
    until(cx, "the sidebar's rows", |cx| shell.read_with(cx, |s, cx| !s.sidebar().read(cx).rows().is_empty()));

    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: workspace.clone(), thread_id: thread.clone() }, window, cx));
    until(cx, "the listing off the sidebar's rows", |cx| {
        view_of(&shell, cx).is_some_and(|v| v.read_with(cx, |v, _| v.listing().is_some_and(|l| l.harness.as_deref() == Some("claude"))))
    });
    // Claude's catalog forks, so the message takes Fork from here once the flags the view asks for land.
    until(cx, "Fork from here under the message", |cx| {
        view_of(&shell, cx)
            .is_some_and(|v| v.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(&r.kind, RowKind::Person { fork: Some(_), .. }))))
    });
}

#[gpui_kit::test]
fn try_again_sends_the_refused_message_and_takes_it_out_of_the_draft(cx: &mut TestAppContext) {
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
    let (view, composer) = screen.read_with(cx, |s, _| (s.view().clone(), s.composer().clone()));
    let failed = StartFailed {
        on: "Boat".into(),
        step: "Asking Boat for a machine".into(),
        said: "the cloud refused the key".into(),
        fix: None,
        prompt: Some("go".into()),
        log: Vec::new(),
        failed: true,
        ..StartFailed::default()
    };
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(Vec::new(), cx);
        v.set_listing(Listing { thread: "t".into(), start_failed: Some(failed), ..Listing::default() }, cx);
    });
    let press = |draft: &str, cx: &mut VisualTestContext| {
        composer.update_in(cx, |c, window, cx| {
            c.set_thread_key("t".into(), window, cx);
            c.set_text(draft, window, cx);
        });
        cx.run_until_parked();
        let at = cx.debug_bounds("spawn-try-again").expect("Try again is drawn").center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        composer.read_with(cx, |c, cx| c.text(cx))
    };
    // The composer put the refused message back as the draft: Try again sent it, so it is not there to send twice.
    assert_eq!(press("go", cx), "", "the message Try again sent stayed in the draft");
    assert_eq!(press("go, and the tests", cx), "go, and the tests", "a draft the person changed went with it");
}
