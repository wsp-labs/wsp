// SPDX-License-Identifier: AGPL-3.0-only
//! The thread screen as the frame mounts it, against a host started for this test alone: one screen kept across a
//! switch, its transcript and its composer moved to each thread, and the list ending where the docked composer stands.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::composer::model::Hold;
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::transcript::RowKind;
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, TestAppContext, VisualTestContext};
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

fn shown(shell: &Entity<Shell>, cx: &mut VisualTestContext, thread: &str, said: &str) -> bool {
    shell.read_with(cx, |s, cx| {
        let Some(screen) = s.thread_screen() else { return false };
        let view = screen.read(cx).view().read(cx);
        view.thread() == Some(thread) && view.rows().iter().any(|r| matches!(r.kind, RowKind::Person { .. }) && r.text == said)
    })
}

#[gpui_kit::test]
fn one_screen_moves_with_its_composer_between_threads_and_clears_the_dock(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
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
    until(cx, "the composer's link", |cx| composer.read_with(cx, |c, _| c.hold() != Some(&Hold::Connecting)));

    // The list ends with the docked composer's drawn height, which grows with a draft of five lines.
    composer.update_in(cx, |c, window, cx| c.set_text("one\ntwo\nthree\nfour\nfive", window, cx));
    until(cx, "the dock to grow past its empty height", |cx| cx.debug_bounds("thread-dock").is_some_and(|b| b.size.height > px(110.)));
    until(cx, "the list to take the dock's height", |cx| {
        let dock = cx.debug_bounds("thread-dock").map(|b| b.size.height);
        screen.read_with(cx, |s, cx| Some(s.view().read(cx).composer_inset()) == dock)
    });

    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: wb.clone(), thread_id: b.clone() }, window, cx));
    until(cx, "the second thread", |cx| shown(&shell, cx, &b, "second"));
    let kept = shell.read_with(cx, |s, _| s.thread_screen()).expect("the thread screen");
    assert_eq!(kept.entity_id(), screen.entity_id(), "a switch moves the kept screen rather than build another");
    assert_eq!(kept.read_with(cx, |s, _| s.composer().entity_id()), composer.entity_id(), "one composer for the kept screen");
    until(cx, "the composer's link to the second thread", |cx| composer.read_with(cx, |c, _| c.hold() != Some(&Hold::Connecting)));
    assert_eq!(
        composer.read_with(cx, |c, _| c.thread_key().map(str::to_owned)),
        Some(b.clone()),
        "the composer holds the second thread's key"
    );

    // No thread open: the kept screen stops following the last one.
    assert_eq!(
        screen.read_with(cx, |s, cx| s.view().read(cx).followed().map(str::to_owned)),
        Some(b.clone()),
        "it follows the second thread"
    );
    shell.update_in(cx, |s, window, cx| s.go(Route::Empty, window, cx));
    until(cx, "the follow to stop", |cx| screen.read_with(cx, |s, cx| s.view().read(cx).followed().is_none()));
}
