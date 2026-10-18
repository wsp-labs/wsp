// SPDX-License-Identifier: AGPL-3.0-only
//! The slate's scrollbar against a host started for this binary alone: hidden at rest, in while the pointer is over
//! the slate or it scrolls, and out 300 ms after the last of either, as `ScrollArea`'s bar does.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::slate::SlateScreen;
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{point, prelude::*, px, size, Entity, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext};
use wsp_host_client::HostClient;

fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (host, FOLDER.get_or_init(|| fixture::project(host)))
}

fn long_thread() -> String {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "scrollbar", file: None });
    let lines: String = (1..=60).map(|n| format!("    <text>Line {n} of a slate taller than its panel.</text>\n")).collect();
    fixture::write(host, &thread, &format!("<slate title=\"Long\">\n  <column>\n{lines}  </column>\n</slate>\n"));
    thread
}

fn open<'a>(cx: &'a mut TestAppContext, thread: &str) -> (Entity<SlateScreen>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let host = bench().0;
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let handle = Host::new(client, runtime);
    let opened = Opened { thread_id: Some(thread.to_owned()), workspace_id: None, tab: Some("slate".into()), arg: None };
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(540.), px(400.)));
    let screen = made.unwrap();
    let limit = Instant::now() + Duration::from_secs(60);
    while !cx.read(|cx| screen.read(cx).slate().is_some_and(|s| !s.resolved.is_empty() || s.formulas().is_empty())) {
        assert!(Instant::now() < limit, "the slate never drew");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    (screen, cx)
}

/// Moves the clock on by `ms`, draws, and says whether the bar is drawn.
fn bar_after(cx: &mut VisualTestContext, ms: u64) -> bool {
    cx.executor().advance_clock(Duration::from_millis(ms));
    for _ in 0..3 {
        cx.update(|window, cx| window.render_frame(cx));
        cx.run_until_parked();
    }
    cx.debug_bounds("slate-scrollbar").is_some()
}

#[gpui_kit::test]
fn the_bar_comes_in_on_scroll_and_hover_and_goes_after_its_delay(cx: &mut TestAppContext) {
    let thread = long_thread();
    let (_, cx) = open(cx, &thread);
    let outside = point(px(270.), px(10.));
    cx.simulate_mouse_move(outside, None, Modifiers::none());
    assert!(!bar_after(cx, 1000), "at rest the slate draws no bar");

    let inside = point(px(270.), px(300.));
    cx.simulate_event(ScrollWheelEvent { position: inside, delta: ScrollDelta::Pixels(point(px(0.), px(-40.))), ..Default::default() });
    assert!(bar_after(cx, 50), "a scroll brings the bar in");
    cx.simulate_mouse_move(outside, None, Modifiers::none());
    assert!(bar_after(cx, 600), "it stays through the scroll's half second and the 300 ms after");
    assert!(!bar_after(cx, 600), "then it goes");

    cx.simulate_mouse_move(inside, None, Modifiers::none());
    assert!(bar_after(cx, 50), "the pointer over the slate brings it in");
    assert!(bar_after(cx, 2000), "and keeps it while it stays");
    cx.simulate_mouse_move(outside, None, Modifiers::none());
    assert!(bar_after(cx, 200), "it waits 300 ms once the pointer leaves");
    assert!(!bar_after(cx, 400), "then goes");
}
