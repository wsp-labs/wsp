// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Activity draws what its link brings in place: a frame that moves a figure draws again only the pieces
//! that show it, and a frame that moves nothing draws nothing at all. The page is opened through the shell's route
//! against a throwaway host (`host-client/tests/common`); the frames come from the test, through the page's own state,
//! so each one moves exactly what the test says. Needs node and the packages built (pnpm build).

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::settings::pages::activity::{self, model::View, Scoped};
use desktop_gpui::screens::settings::SettingsScreen;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{
    point, px, size, AppContext, Entity, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext,
};
use serde_json::{json, Value};

const WAIT: Duration = Duration::from_secs(30);

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The shell on Settings > Activity in a 1280 by 800 window, counting its frames, against a host with no projects, so
/// no thread's clock moves while the test counts.
fn launched(cx: &mut TestAppContext) -> (common::Throwaway, Entity<Shell>, &mut VisualTestContext) {
    let host = common::Throwaway::start();
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let (state, env) = (host.state(), host.env());
    let launch = Launch { screen: Some("settings:activity".into()), ..Launch::default() };
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, launch, window, cx));
        shell = Some(view.clone());
        desktop_gpui::frames::watch(window, cx);
        Root::new(view, window, cx).cache_view()
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    let shell = shell.unwrap();
    (host, shell, cx)
}

/// The state the Activity page draws, once it has drawn one.
fn scoped(cx: &mut VisualTestContext) -> Option<Entity<Scoped>> {
    cx.update(|window, cx| activity::kept(window, cx).read(cx).drawn())
}

fn mounted(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |s, _| s.screen_view()).is_some_and(|view| view.downcast::<SettingsScreen>().is_ok())
}

/// The daemon's frame: itself at pid 1, holding `rss` bytes at `cpu` percent of a core, sampled at `at`.
fn frame(at: f64, cpu: f64, rss: f64) -> Value {
    let row = json!({ "pid": 1, "ppid": 0, "user": "bench", "state": "S", "comm": "wsp-daemon", "cmdline": "wsp-daemon",
        "cpu": cpu, "rss": rss, "startedAt": 1_700_000_000_000i64 });
    json!({ "type": "proc.snapshot", "at": at, "daemon": 1, "total": 1, "procs": [row], "seq": 1 })
}

fn hear(scoped: &Entity<Scoped>, event: Value, cx: &mut VisualTestContext) -> std::collections::BTreeMap<&'static str, u64> {
    desktop_gpui::frames::take();
    scoped.update(cx, |s, cx| s.hear(event, cx));
    cx.run_until_parked();
    desktop_gpui::frames::take()
}

/// Settings on Activity with the daemon's first frame drawn, the page's state, and nothing left moving.
fn settled(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Entity<Scoped> {
    until(cx, "Settings on Activity", |cx| mounted(shell, cx) && cx.debug_bounds("activity-controls").is_some());
    until(cx, "the page's state", |cx| scoped(cx).is_some());
    let scoped = scoped(cx).unwrap();
    hear(&scoped, frame(1000., 0.5, 30e6), cx);
    until(cx, "the tiles", |cx| cx.debug_bounds("activity-cpu").is_some());
    // Whatever the link said while the page opened has landed, and its dial waits on a clock of its own.
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    // The frame Settings asked for as it opened (the trail's cards in view) is drawn, as a display's next frame would
    // draw it at once.
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    scoped
}

const AROUND: [&str; 5] = ["Shell", "Sidebar", "RightPanel", "SettingsScreen", "SettingsPart"];

/// The page's own views, which a frame that only moves figures leaves alone too.
const PAGE: [&str; 2] = ["ActivityPage", "ActivityList"];

/// A frame that moves the CPU figure draws the pieces that show it once each, the CPU tile and the wsp head's figure,
/// and the lines that took the frame's point, and nothing around them: not the shell, its sidebar, Settings or its
/// parts. The same frame again moves nothing, so nothing renders and no frame is drawn. Settings stands where the right
/// panel would, so the panel is not drawn on this route.
#[gpui_kit::test]
fn a_tick_of_activity_draws_only_the_pieces_it_moved(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx);
    let scoped = settled(&shell, cx);

    let counts = hear(&scoped, frame(3000., 2.5, 30e6), cx);
    assert_eq!(counts.get("ActivityTile"), Some(&1), "only the CPU tile's figure moved: {counts:?}");
    assert_eq!(counts.get("ActivityFigure"), Some(&1), "and the wsp head's CPU figure: {counts:?}");
    assert_eq!(counts.get("ActivityLine"), Some(&2), "the memory and CPU lines took the frame's point: {counts:?}");
    for view in AROUND.iter().chain(&PAGE) {
        assert!(!counts.contains_key(view), "{view} rendered for a tick of Activity: {counts:?}");
    }
    assert_eq!(counts.get("frame"), Some(&1), "the tick drew one frame: {counts:?}");

    let counts = hear(&scoped, frame(3000., 2.5, 30e6), cx);
    assert!(counts.is_empty(), "a frame that moves nothing draws nothing: {counts:?}");
}

/// A process that comes under an open group is a row more, a change of layout: the page draws again, once, where it
/// stands, and nothing around it; the next frame that only moves figures draws them alone again.
#[gpui_kit::test]
fn a_new_row_lays_the_page_out_once(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx);
    let scoped = settled(&shell, cx);
    scoped.update(cx, |s, cx| s.toggle("wsp", cx));
    cx.run_until_parked();
    let child = json!({ "pid": 2, "ppid": 1, "user": "bench", "state": "S", "comm": "git", "cmdline": "git status",
        "cpu": 0.1, "rss": 1e6, "startedAt": 1_700_000_000_000i64 });
    let grown = |at: f64, cpu: f64| {
        let mut grown = frame(at, 0.5, 30e6);
        let mut child = child.clone();
        child["cpu"] = json!(cpu);
        grown["procs"].as_array_mut().unwrap().push(child);
        grown
    };
    let counts = hear(&scoped, grown(3000., 0.1), cx);
    assert_eq!(counts.get("ActivityPage"), Some(&1), "a row more lays the page out again, once: {counts:?}");
    for view in AROUND {
        assert!(!counts.contains_key(view), "{view} rendered for a row more: {counts:?}");
    }
    let counts = hear(&scoped, grown(5000., 0.3), cx);
    for view in AROUND.iter().chain(&PAGE) {
        assert!(!counts.contains_key(view), "{view} rendered for a figure of the new row: {counts:?}");
    }
    assert_eq!(counts.get("ActivityFigure"), Some(&2), "the new row's CPU and its head's: {counts:?}");
}

/// The daemon's frame with `n` processes under it, which wsp's group, open, draws as a page many windows tall.
fn long(at: f64, n: usize) -> Value {
    let mut long = frame(at, 0.5, 30e6);
    let procs = long["procs"].as_array_mut().unwrap();
    for pid in 2..n as i64 + 2 {
        procs.push(json!({ "pid": pid, "ppid": 1, "user": "bench", "state": "S", "comm": format!("git-{pid}"),
            "cmdline": format!("git status {pid}"), "cpu": 0.1, "rss": 1e6 + pid as f64, "startedAt": 1_700_000_000_000i64 }));
    }
    long["total"] = json!(n + 1);
    long
}

/// Catches: a scroll step on Activity that rendered Settings again and laid out every row of the page, on screen or
/// not, so a long page dropped frames under a trackpad's scroll.
#[gpui_kit::test]
fn a_scroll_step_lays_out_only_the_rows_on_screen(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx);
    let scoped = settled(&shell, cx);
    scoped.update(cx, |s, cx| s.toggle("wsp", cx));
    cx.run_until_parked();
    let counts = hear(&scoped, long(3000., 300), cx);
    assert!(counts.contains_key("ActivityRow"), "the open group draws its rows: {counts:?}");
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    let at = cx.debug_bounds("activity").expect("the page").center();
    for step in 0..3 {
        desktop_gpui::frames::take();
        let wheel = ScrollWheelEvent {
            position: point(at.x, px(500.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        };
        cx.simulate_event(wheel);
        let counts = desktop_gpui::frames::take();
        assert!(counts.contains_key("frame"), "step {step} drew a frame: {counts:?}");
        for view in AROUND.iter().chain(&["ActivityPage"]) {
            assert!(!counts.contains_key(view), "step {step} rendered {view}: {counts:?}");
        }
        let rows = counts.get("ActivityRow").copied().unwrap_or(0);
        assert!(rows <= 30, "step {step} laid out {rows} rows, where about 20 show: {counts:?}");
    }
}

/// The daemon's frame at sample `tick` on a busy computer: `n` processes under it whose memory and CPU move on every
/// sample, so their rows reorder, and a tenth of which come and go.
fn busy(tick: usize, n: usize) -> Value {
    let mut busy = frame(3000. + 2000. * tick as f64, 0.5, 30e6);
    let procs = busy["procs"].as_array_mut().unwrap();
    for ix in 0..n {
        if ix % 10 == 0 && (ix / 10 + tick) % 10 < 3 {
            continue;
        }
        let pid = ix as i64 + 2;
        let wave = ((ix * 7 + tick * 13) % 97) as f64;
        procs.push(json!({ "pid": pid, "ppid": 1, "user": "bench", "state": "S", "comm": format!("git-{pid}"),
            "cmdline": format!("git status {pid}"), "cpu": wave / 10., "rss": 1e6 + wave * 1e5, "startedAt": 1_700_000_000_000i64 }));
    }
    busy["total"] = json!(procs.len() + 1);
    busy
}

/// Catches: a busy computer's samples, whose rows come, go and reorder on most of them, each laying out the whole page
/// and rendering every figure on it, Settings and the shell around it too: 780 renders a frame on a Mac.
#[gpui_kit::test]
fn a_busy_computers_samples_draw_only_the_rows_on_screen(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx);
    let scoped = settled(&shell, cx);
    scoped.update(cx, |s, cx| s.toggle("wsp", cx));
    hear(&scoped, busy(0, 300), cx);
    desktop_gpui::frames::take();
    cx.update(|window, _| window.take_renders());
    for tick in 1..=30 {
        scoped.update(cx, |s, cx| s.hear(busy(tick, 300), cx));
        cx.run_until_parked();
    }
    let counts = desktop_gpui::frames::take();
    let most = cx.update(|window, _| window.take_renders()).into_values().max().unwrap_or(0);
    for view in ["Shell", "Sidebar", "RightPanel", "SettingsScreen", "SettingsPart"] {
        assert!(!counts.contains_key(view), "the samples drew {view}: {counts:?}");
    }
    let frames = counts.get("frame").copied().unwrap_or(0) as usize;
    assert!(most <= frames, "a view rendered {most} times in {frames} frames: {counts:?}");
    let rows = counts.get("ActivityRow").copied().unwrap_or(0);
    assert!(rows <= 30 * 30, "30 samples laid out {rows} rows, where about 20 show: {counts:?}");
}

/// Activity's view picker eases its ink for some frames after a pick. The pick draws the header's Actions part whole to
/// measure its new width, and the first step places it in its box from there; every step after draws the part alone:
/// not the shell, its sidebar, Settings or its other parts.
#[gpui_kit::test]
fn a_step_of_the_view_pickers_ink_draws_the_actions_part_alone(cx: &mut TestAppContext) {
    let (_host, shell, cx) = launched(cx);
    settled(&shell, cx);
    cx.update(|window, cx| {
        activity::kept(window, cx).update(cx, |pick, cx| {
            pick.view = View::Processes;
            cx.notify();
        })
    });
    cx.run_until_parked();
    let step = |cx: &mut VisualTestContext| {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
        desktop_gpui::frames::take()
    };
    desktop_gpui::frames::take();
    step(cx);
    let mut counts = std::collections::BTreeMap::new();
    for _ in 0..30 {
        for (view, n) in step(cx) {
            *counts.entry(view).or_insert(0) += n;
        }
    }
    assert!(counts.get("SettingsActions").is_some_and(|n| *n > 0), "the picker's ink eased: {counts:?}");
    for view in AROUND {
        assert!(!counts.contains_key(view), "{view} rendered for a step of the picker's ink: {counts:?}");
    }
}
