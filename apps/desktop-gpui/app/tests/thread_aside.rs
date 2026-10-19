// SPDX-License-Identifier: AGPL-3.0-only
//! The side question against a host started for this test alone: `/btw` in the thread's composer opens the Side
//! question tab of the right panel with the question, the crab and Asking, then the host's answer or refusal; Esc takes
//! the tab away and shuts the panel the question opened.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::{Pane, Route};
use desktop_gpui::shell::{Launch, Shell};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, TestAppContext, VisualTestContext};
use wsp_host_client::HostClient;

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn panel(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (bool, Vec<Pane>) {
    shell.read_with(cx, |s, cx| {
        let p = s.panel().read(cx);
        (p.is_open(), p.panes())
    })
}

#[gpui_kit::test]
fn btw_opens_the_side_question_tab_that_answers_and_escape_takes_it_away(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let first = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread");
    let (workspace_id, thread_id) = (first.workspace_id.clone(), first.thread_id.clone().expect("a thread id"));

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
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id, thread_id }, window, cx));
    until(cx, "the thread's composer knowing its agent", |cx| {
        shell.read_with(cx, |s, cx| s.thread_screen().is_some_and(|t| t.read(cx).composer().read(cx).ran().agent.is_some()))
    });
    assert!(!panel(&shell, cx).0, "the panel starts shut");

    let composer = shell.read_with(cx, |s, cx| s.thread_screen().unwrap().read(cx).composer().clone());
    composer.update_in(cx, |c, window, cx| {
        c.set_text("/btw what does this repo hold?", window, cx);
        c.submit(window, cx);
    });
    until(cx, "the side question's tab", |cx| cx.debug_bounds("aside-question").is_some());
    assert_eq!(panel(&shell, cx), (true, vec![Pane::Aside]), "the question opened the panel on its own tab");
    assert!(cx.debug_bounds("aside-note").is_some(), "the note that nothing here is kept");
    assert!(composer.read_with(cx, |c, cx| c.text(cx).is_empty()), "the question left the box");
    until(cx, "the answer or the refusal", |cx| cx.debug_bounds("aside-answer").is_some() || cx.debug_bounds("aside-refused").is_some());
    assert!(cx.debug_bounds("aside-asking").is_none(), "the crab stays once the reply landed");

    cx.simulate_keystrokes("escape");
    until(cx, "the tab gone", |cx| cx.debug_bounds("aside-question").is_none());
    assert_eq!(panel(&shell, cx), (false, vec![]), "Esc took the tab away and shut the panel the question opened");
}
