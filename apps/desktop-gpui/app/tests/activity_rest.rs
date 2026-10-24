// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Activity left live on a host of its own and the daemon it carries, as a person leaves it open on a busy
//! computer: every process listed by CPU while short busy processes come and go, so rows come, go and reorder on most
//! samples. For 30 s no view renders more often than the window draws, and no sample draws Settings or the shell around
//! the page. Needs node, the packages built (pnpm build) and the daemon staged (scripts/stage-daemon.sh).

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::settings::pages::activity::{self, model::Sort, model::View};
use desktop_gpui::screens::settings::SettingsScreen;
use desktop_gpui::screens::sidebar::model::Status;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, TestAppContext, VisualTestContext};

/// Draws the window for `span` as a display would, a frame each 16 ms that something asked for one, its clock moving
/// with the wall's.
fn live(cx: &mut VisualTestContext, span: Duration) {
    let end = Instant::now() + span;
    while Instant::now() < end {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// Busy processes, each spinning for a second to three before it is killed, one started every 600 ms until `stop`.
fn churn(stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut alive: Vec<(Child, Instant)> = Vec::new();
        let mut n = 0u64;
        while !stop.load(Ordering::SeqCst) {
            let spin = Command::new("sh").args(["-c", "while :; do :; done"]).env_clear().env("PATH", "/usr/bin:/bin").spawn();
            if let Ok(child) = spin {
                alive.push((child, Instant::now() + Duration::from_millis(1000 + (n * 700) % 2000)));
            }
            n += 1;
            std::thread::sleep(Duration::from_millis(600));
            alive.retain_mut(|(child, until)| {
                let done = Instant::now() >= *until;
                if done {
                    _ = child.kill();
                    _ = child.wait();
                }
                !done
            });
        }
        for (mut child, _) in alive {
            _ = child.kill();
            _ = child.wait();
        }
    })
}

/// Catches: a busy computer's samples each laying out the whole page, Settings and the shell around it, and rendering
/// every figure on every row, on screen or not (780 renders a frame on a Mac).
#[gpui_kit::test]
fn activity_left_live_renders_no_view_more_often_than_it_draws(cx: &mut TestAppContext) {
    let host = common::Throwaway::start_with(common::STAND_IN, true);
    assert!(host.daemon.is_some(), "{}: run apps/desktop-gpui/scripts/stage-daemon.sh", common::wsp_daemon().unwrap_err());
    let folder = host.repo("alpha");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "alpha"]);
    host.wsp(&["run", "alpha", "--agent", "claude", "hi"]);
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
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
    let end = Instant::now() + Duration::from_secs(30);
    while !(shell.read_with(cx, |s, _| s.screen_view()).is_some_and(|v| v.downcast::<SettingsScreen>().is_ok())
        && cx.debug_bounds("activity-totals").is_some()
        && cx.debug_bounds("grid-row").is_some())
    {
        assert!(Instant::now() < end, "Activity never drew the daemon's processes: {}", host.log());
        live(cx, Duration::from_millis(100));
    }
    // The project's turn has ended, so the host pushes nothing of its own while the page is counted.
    let end = Instant::now() + Duration::from_secs(60);
    while shell.read_with(cx, |s, cx| s.sidebar().read(cx).threads().iter().any(|t| t.status == Status::Running)) {
        assert!(Instant::now() < end, "the project's turn never ended: {}", host.log());
        live(cx, Duration::from_millis(100));
    }
    cx.update(|window, cx| {
        activity::kept(window, cx).update(cx, |pick, cx| {
            pick.view = View::Processes;
            cx.notify();
        })
    });
    live(cx, Duration::from_millis(500));
    let scoped = cx.update(|window, cx| activity::kept(window, cx).read(cx).drawn()).expect("the page's state");
    scoped.update(cx, |s, cx| {
        s.set_every(true, cx);
        s.set_sort(Sort::Cpu, cx);
    });
    let stop = Arc::new(AtomicBool::new(false));
    let churning = churn(stop.clone());
    // The daemon's first samples of every process land, and the page lays out what they hold.
    live(cx, Duration::from_secs(5));
    desktop_gpui::frames::take();
    cx.update(|window, _| window.take_renders());
    let mut counts = std::collections::BTreeMap::new();
    for _ in 0..30 {
        live(cx, Duration::from_secs(1));
        for (view, n) in desktop_gpui::frames::take() {
            *counts.entry(view).or_insert(0) += n;
        }
        for view in ["Shell", "SettingsScreen", "SettingsPart"] {
            if counts.contains_key(view) {
                stop.store(true, Ordering::SeqCst);
            }
            assert!(!counts.contains_key(view), "the samples drew {view}: {counts:?}");
        }
    }
    stop.store(true, Ordering::SeqCst);
    churning.join().unwrap();
    let renders: HashMap<_, _> = cx.update(|window, _| window.take_renders()).into_iter().collect();
    let frames = counts.get("frame").copied().unwrap_or(0) as usize;
    assert!(counts.contains_key("ActivityPage"), "the samples laid the page out again: {counts:?}");
    let most = renders.values().copied().max().unwrap_or(0);
    assert!(most <= frames, "a view rendered {most} times in {frames} frames: {counts:?}");
    let rows = counts.get("ActivityRow").copied().unwrap_or(0);
    let pages = counts.get("ActivityList").copied().unwrap_or(0);
    assert!(rows <= pages * 30, "{pages} layouts of the list laid out {rows} rows, where about 20 show: {counts:?}");
}
