// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar's times step on their own: a row's age draws again where it stands as its text changes, and nothing
//! around it, the shell, the sidebar, the right panel or the page, draws for it; a clock with nothing to change asks
//! for no frame.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use desktop_gpui::frames;
use desktop_gpui::screens::composer;
use desktop_gpui::screens::sidebar::model::Status;
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::host::Change;
use desktop_gpui::ui::status::Still;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext as _, Entity, TestAppContext, VisualTestContext};
use serde_json::json;

const WAIT: Duration = Duration::from_secs(30);

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    while !done(cx) {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() * 1000.
}

/// The shell on a host holding one project, its window drawn cached under the root as the app draws it, the right
/// panel open, and nothing on the page moving; the counts start once it has settled.
fn launched(cx: &mut TestAppContext) -> (common::Throwaway, Entity<Shell>, &mut VisualTestContext) {
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
        cx.set_global(Still(true));
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let (state, env) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        frames::watch(window, cx);
        Root::new(view, window, cx).cache_view()
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    let shell = shell.unwrap();
    until(cx, "the project's New thread page", |cx| {
        shell.read_with(cx, |s, _| matches!(s.route(), Route::Screen(r) if r.starts_with("composer:")))
    });
    until(cx, "the host's thread in the sidebar", |cx| shell.read_with(cx, |s, cx| !s.sidebar().read(cx).threads().is_empty()));
    cx.update(|window, cx| {
        let panel = shell.read(cx).panel().clone();
        panel.update(cx, |p, cx| p.toggle(window, cx));
    });
    // The host's last answers land before the counts start.
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(frames::take().contains_key("RightPanel"), "the right panel is open");
    // The window lets go of the keys, so the New thread page's caret does not blink through the counts; its going lands
    // first.
    cx.update(|window, cx| window.blur(cx));
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    (host, shell, cx)
}

/// A thread read and at rest, ended `ago` ms back, which its tile says as an age.
fn rest_a_thread(shell: &Entity<Shell>, ago: f64, cx: &mut VisualTestContext) {
    let now = now_ms();
    let row = json!({
        "id": "r-live", "threadId": "t-live", "workspaceId": "w-live", "harness": "claude", "status": "completed",
        "prompt": "a thread at rest", "project": { "id": "p-live", "name": "live" },
        "startedAt": now - ago - 1_000., "endedAt": now - ago, "readAt": now - ago,
    });
    shell.update(cx, |s, cx| s.sidebar().update(cx, |s, cx| s.apply(Change::Row { id: "r-live".into(), row: Some(row) }, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tile-t-live").is_some(), "the thread's tile is drawn");
}

/// Moves the test clock `by` and draws what follows, returning the frames and renders it took.
fn advance(cx: &mut VisualTestContext, by: Duration) -> BTreeMap<&'static str, u64> {
    frames::take();
    cx.executor().advance_clock(by);
    cx.run_until_parked();
    frames::take()
}

/// Catches: an age that changes rendering the sidebar, and through it the shell and every observer of the sidebar,
/// and a clock tick that changes no text drawing a frame at all.
#[gpui_kit::test]
fn a_tick_of_an_age_renders_the_age_alone(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx);
    rest_a_thread(&shell, 90_000., cx);
    let quiet = advance(cx, Duration::from_secs(20));
    assert!(quiet.is_empty(), "a tick that changes no text draws nothing: {quiet:?}");
    let moved = advance(cx, Duration::from_secs(11));
    assert_eq!(moved.get("SidebarAge"), Some(&1), "the age drew once as 1m became 2m: {moved:?}");
    assert_eq!(moved.get("frame"), Some(&1), "one frame for the age: {moved:?}");
    for view in ["Shell", "Sidebar", "RightPanel", "ComposerScreen"] {
        assert!(!moved.contains_key(view), "{view} rendered for a tick of an age: {moved:?}");
    }
}

/// Catches: the sidebar's clock drawing the list again on its own ticks, every 30 s at rest.
#[gpui_kit::test]
fn the_sidebars_clock_at_rest_asks_for_no_frame_but_an_ages(cx: &mut TestAppContext) {
    let (_host, _shell, cx) = launched(cx);
    let mut all: BTreeMap<&'static str, u64> = BTreeMap::new();
    for _ in 0..60 {
        for (what, n) in advance(cx, Duration::from_secs(1)) {
            *all.entry(what).or_default() += n;
        }
    }
    for view in ["Shell", "Sidebar", "RightPanel", "ComposerScreen"] {
        assert!(!all.contains_key(view), "{view} rendered over 60 s at rest: {all:?}");
    }
    let ages = all.get("SidebarAge").copied().unwrap_or(0);
    assert!(all.get("frame").copied().unwrap_or(0) <= ages, "a frame only where an age's text changed over 60 s at rest: {all:?}");
}

/// Catches: the New thread page drawing again for every sidebar change, where its headline reads only the project's
/// name and mark.
#[gpui_kit::test]
fn a_sidebar_change_that_moves_nothing_on_the_page_leaves_it_alone(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx);
    frames::take();
    shell.update(cx, |s, cx| s.sidebar().update(cx, |_, cx| cx.notify()));
    cx.run_until_parked();
    let counts = frames::take();
    assert_eq!(counts.get("Sidebar"), Some(&1), "the sidebar drew: {counts:?}");
    assert!(!counts.contains_key("ComposerScreen"), "the page drew for a sidebar change it does not show: {counts:?}");
}

/// A thread of the project's, started `ago` ms back: working now, which stands it in the Working section, or ended
/// now and read.
fn live_thread(shell: &Entity<Shell>, ago: f64, running: bool, cx: &mut VisualTestContext) {
    let now = now_ms();
    let mut row = json!({
        "id": "r-work", "threadId": "t-work", "workspaceId": "w-live", "harness": "claude",
        "status": if running { "running" } else { "completed" }, "prompt": "a thread at work",
        "project": { "id": "p-live", "name": "live" }, "startedAt": now - ago,
    });
    if !running {
        row["endedAt"] = json!(now);
        row["readAt"] = json!(now);
    }
    shell.update(cx, |s, cx| s.sidebar().update(cx, |s, cx| s.apply(Change::Row { id: "r-work".into(), row: Some(row) }, cx)));
    cx.run_until_parked();
}

/// Catches: Working's membership drawing the list again on the clock's ticks, where it moves only as work starts or
/// stops: a minute of a working thread draws its time alone, and its end draws the list as it leaves Working.
#[gpui_kit::test]
fn working_draws_the_list_again_only_as_work_starts_or_stops(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx);
    // The host's own turn has ended, so it pushes nothing of its own while the minute is counted.
    until(cx, "the host's turn to end", |cx| {
        shell.read_with(cx, |s, cx| s.sidebar().read(cx).threads().iter().all(|t| t.status != Status::Running))
    });
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    frames::take();
    live_thread(&shell, 60_000., true, cx);
    let started = frames::take();
    assert!(started.contains_key("Sidebar"), "the thread starting work stands it in Working: {started:?}");
    let mut all: BTreeMap<&'static str, u64> = BTreeMap::new();
    for _ in 0..60 {
        for (what, n) in advance(cx, Duration::from_secs(1)) {
            *all.entry(what).or_default() += n;
        }
    }
    for view in ["Shell", "Sidebar", "RightPanel", "ComposerScreen"] {
        assert!(!all.contains_key(view), "{view} rendered over a minute of work: {all:?}");
    }
    live_thread(&shell, 60_000., false, cx);
    let ended = frames::take();
    assert!(ended.contains_key("Sidebar"), "its end moves it out of Working: {ended:?}");
}
