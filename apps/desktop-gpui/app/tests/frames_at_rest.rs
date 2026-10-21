// SPDX-License-Identifier: AGPL-3.0-only
//! At rest the window draws nothing: on every page, a minute passes with nothing touched and no frame is drawn unless
//! something on screen moves, and a frame that is drawn renders only what moved, never the shell, the sidebar, the
//! right panel or the page around it.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::{composer, Route};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::status::Still;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, TestAppContext, VisualTestContext};

const WAIT: Duration = Duration::from_secs(30);

/// The views a frame at rest must never render.
const AROUND: [&str; 7] = ["Shell", "Sidebar", "RightPanel", "ComposerScreen", "ThreadScreen", "SettingsScreen", "UsagePage"];

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    while !done(cx) {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
}

/// The shell in a 1280 by 800 window on a host holding one project, `repo`, with one finished thread, the window
/// drawn as the app draws it.
fn launched(cx: &mut TestAppContext, still: bool) -> (common::Throwaway, Entity<Shell>, &mut VisualTestContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
        composer::init(cx);
        if still {
            cx.set_global(Still(true));
        }
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let (state, env) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        desktop_gpui::frames::watch(window, cx);
        Root::new(view, window, cx).cache_view()
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    let shell = shell.unwrap();
    until(cx, "the project's New thread page", |cx| {
        shell.read_with(cx, |s, _| matches!(s.route(), Route::Screen(r) if r.starts_with("composer:")))
    });
    until(cx, "the thread in the sidebar", |cx| shell.read_with(cx, |s, cx| !s.sidebar().read(cx).items().is_empty()));
    (host, shell, cx)
}

/// What the window renders over a minute untouched, after the page's own arrival has settled and any notice its
/// arrival raised has gone, which takes up to 12 s.
fn a_minute_at_rest(cx: &mut VisualTestContext) -> BTreeMap<&'static str, u64> {
    std::thread::sleep(Duration::from_millis(500));
    for _ in 0..13 {
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    desktop_gpui::frames::take();
    for _ in 0..60 {
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }
    desktop_gpui::frames::take()
}

fn go(shell: &Entity<Shell>, route: Route, cx: &mut VisualTestContext) {
    cx.update(|window, cx| shell.update(cx, |s, cx| s.go(route, window, cx)));
    cx.run_until_parked();
}

/// Every page that shows nothing moving, under reduced motion so the New thread page's field holds still, and with
/// the window's keys let go so no caret blinks: a minute untouched draws no frame on any of them.
#[gpui_kit::test]
fn no_page_draws_a_frame_at_rest(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx, true);
    let thread = cx.read(|cx| {
        let item = &shell.read(cx).sidebar().read(cx).items()[0];
        Route::Thread { workspace_id: item.workspace_id.clone(), thread_id: item.thread_id.clone() }
    });
    let mut drew = Vec::new();
    let mut page = |name: String, cx: &mut VisualTestContext| {
        cx.update(|window, cx| window.blur(cx));
        let counts = a_minute_at_rest(cx);
        if !counts.is_empty() {
            drew.push(format!("{name}: {counts:?}"));
        }
    };
    page("the New thread page".into(), cx);
    go(&shell, thread.clone(), cx);
    page("a thread".into(), cx);
    cx.update(|window, cx| {
        let panel = shell.read(cx).panel().clone();
        panel.update(cx, |p, cx| p.toggle(window, cx));
    });
    page("a thread with the right panel's launcher".into(), cx);
    for pane in ["processes", "computer", "agents"] {
        go(&shell, thread.clone(), cx);
        go(&shell, Route::Screen(pane.into()), cx);
        page(format!("the {pane} pane"), cx);
    }
    // Activity's figures move by themselves, so it is held to drawing only them, in the next test.
    for group in
        ["general", "appearance", "agents", "computers", "recipes", "projects", "usage", "keybindings", "privacy", "devices", "account"]
    {
        go(&shell, Route::Screen(format!("settings:{group}").into()), cx);
        page(format!("Settings > {group}"), cx);
    }
    assert!(drew.is_empty(), "pages that drew at rest:\n{}", drew.join("\n"));
}

/// Pages whose content moves by itself draw only that content: the New thread page's field and its composer's caret,
/// and Activity's figures.
#[gpui_kit::test]
fn a_page_that_moves_at_rest_draws_only_what_moves(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx, false);
    let mut drew = Vec::new();
    let mut page = |name: &str, cx: &mut VisualTestContext| {
        let counts = a_minute_at_rest(cx);
        let around: Vec<_> = AROUND.iter().filter(|view| counts.contains_key(*view)).collect();
        if !around.is_empty() {
            drew.push(format!("{name}: {counts:?}"));
        }
        counts
    };
    let field = page("the New thread page", cx);
    assert!(field.get("HeroField").is_some_and(|n| *n >= 400), "the field moves through the minute: {field:?}");
    go(&shell, Route::Screen("settings:activity".into()), cx);
    page("Settings > activity", cx);
    assert!(drew.is_empty(), "pages that rendered more than what moved:\n{}", drew.join("\n"));
}
