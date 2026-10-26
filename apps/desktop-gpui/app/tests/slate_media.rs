// SPDX-License-Identifier: AGPL-3.0-only
//! Galleries against a host started for this binary alone: a picture is asked for only once its frame comes within a
//! screen of the scroll box, the panel lets go of pictures away from view past its bound, and none is left spinning.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::slate::pieces::media;
use desktop_gpui::screens::slate::SlateScreen;
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{
    point, px, size, AppContext as _, Entity, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext,
};
use wsp_host_client::HostClient;

/// Pictures in each gallery, `SLATE_LIMITS.imagesPerList`.
const EACH: usize = 60;
const HEIGHT: f32 = 748.;

/// A project whose folder holds `2 * EACH` screenshot-sized pictures, each its own file.
fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (
        host,
        FOLDER.get_or_init(|| {
            let folder = fixture::project(host);
            let gallery = folder.join("shots/gallery");
            std::fs::create_dir_all(&gallery).unwrap();
            let one = gallery.join("0.png");
            fixture::png(1280, 800, [40, 90, 160], [230, 180, 90]).save(&one).unwrap();
            for i in 1..2 * EACH {
                std::fs::copy(&one, gallery.join(format!("{i}.png"))).unwrap();
            }
            folder
        }),
    )
}

/// A folder beside the gallery's holding `2 * EACH` phone screenshots, 1170 by 2532 as an iPhone takes them.
fn portraits() -> &'static PathBuf {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    FOLDER.get_or_init(|| {
        let folder = bench().1.join("shots/portrait");
        std::fs::create_dir_all(&folder).unwrap();
        let one = folder.join("0.png");
        fixture::png(1170, 2532, [160, 60, 90], [90, 200, 230]).save(&one).unwrap();
        for i in 1..2 * EACH {
            std::fs::copy(&one, folder.join(format!("{i}.png"))).unwrap();
        }
        folder
    })
}

fn gallery(id: &str, from: usize) -> String {
    gallery_in(id, &bench().1.join("shots/gallery"), from)
}

fn gallery_in(id: &str, shots: &std::path::Path, from: usize) -> String {
    let items: Vec<String> = (from..from + EACH).map(|i| format!("{{ p: \"{}/{i}.png\" }}", shots.display())).collect();
    format!("    <images id=\"{id}\" items={{[{}]}} src={{item.p}} />\n", items.join(", "))
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
    cx.simulate_resize(size(px(540.), px(HEIGHT)));
    (made.unwrap(), cx)
}

fn thumb(gal: &str, i: usize) -> &'static str {
    Box::leak(format!("slate-{gal}-thumb-{i}").into_boxed_str())
}

fn spinning(gal: &str, i: usize) -> &'static str {
    Box::leak(format!("slate-{gal}-thumb-{i}-loading").into_boxed_str())
}

/// The thumbnails whose frames show in the window.
fn in_view(cx: &mut VisualTestContext) -> Vec<(&'static str, usize)> {
    let mut seen = Vec::new();
    for gal in ["gal", "gal2"] {
        for i in 0..EACH {
            if let Some(b) = cx.debug_bounds(thumb(gal, i)) {
                if f32::from(b.bottom()) > 60. && f32::from(b.top()) < HEIGHT {
                    seen.push((if gal == "gal" { "gal" } else { "gal2" }, i));
                }
            }
        }
    }
    seen
}

/// Waits until every thumbnail in view shows its picture, and says which still spin when it gives up.
fn settled(cx: &mut VisualTestContext, at: &str) {
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        let view = in_view(cx);
        let spin: Vec<_> = view.iter().filter(|(g, i)| cx.debug_bounds(spinning(g, *i)).is_some()).collect();
        if !view.is_empty() && spin.is_empty() {
            return;
        }
        assert!(Instant::now() < limit, "{at}: pictures in view left spinning: {spin:?} of {view:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn scroll(cx: &mut VisualTestContext, by: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(270.), px(HEIGHT / 2.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-by))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
}

#[gpui_kit::test]
fn sixty_pictures_ask_near_view_and_none_is_left_spinning(cx: &mut TestAppContext) {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "gallery", file: None });
    let text = format!("<slate title=\"Gallery\">\n  <column>\n{}{}  </column>\n</slate>\n", gallery("gal", 0), gallery("gal2", EACH));
    fixture::write(host, &thread, &text);
    let (_, cx) = open(cx, &thread);
    settled(cx, "at the top");
    let held = cx.update(|_, cx| media::held_bytes(cx));
    eprintln!("decoded bytes held at the top: {held} ({:.1} MiB)", held as f64 / (1 << 20) as f64);
    assert!(cx.debug_bounds(spinning("gal", EACH - 1)).is_some(), "the gallery's last picture, two screens down, is not asked yet");
    assert!(held < (EACH as u64) << 20, "a screen's pictures and the next, not all sixty: {held}");

    let mut most = held;
    let mut steps = 0;
    while in_view(cx).iter().all(|&(g, i)| (g, i) != ("gal2", EACH - 1)) {
        scroll(cx, 360.);
        let at = format!("after {} px down", 360 * (steps + 1));
        settled(cx, &at);
        most = most.max(within_bound(cx, &at));
        steps += 1;
        assert!(steps < 40, "the second gallery's last picture came into view");
    }
    eprintln!("decoded bytes held at most while scrolling {} pictures: {most} ({:.1} MiB)", 2 * EACH, most as f64 / (1 << 20) as f64);

    for i in 0..steps {
        scroll(cx, -360.);
        within_bound(cx, &format!("{} px back up", 360 * (i + 1)));
    }
    settled(cx, "back at the top");
    assert!(cx.debug_bounds(spinning("gal", 0)).is_none(), "the first picture, let go on the way down, is drawn again");
    let held = within_bound(cx, "back at the top");
    eprintln!("decoded bytes held back at the top: {held} ({:.1} MiB)", held as f64 / (1 << 20) as f64);
}

/// One thumbnail's most: `THUMB_BOX`, 640 by 400, four bytes a pixel.
const THUMB_MOST: u64 = 640 * 400 * 4;

#[gpui_kit::test]
fn a_tall_window_of_phone_screenshots_holds_64_pictures_or_those_near_view_each_one_thumbnail(cx: &mut TestAppContext) {
    let (host, folder) = bench();
    let shots = portraits();
    let thread = fixture::staged(host, folder, &fixture::State { name: "portraits", file: None });
    let text = format!(
        "<slate title=\"Phones\">\n  <column>\n{}{}  </column>\n</slate>\n",
        gallery_in("gal", shots, 0),
        gallery_in("gal2", shots, EACH)
    );
    fixture::write(host, &thread, &text);
    let (_, cx) = open(cx, &thread);
    cx.simulate_resize(size(px(540.), px(1100.)));
    let (mut most, mut most_count, mut steps) = (0u64, 0usize, 0);
    loop {
        let at = format!("after {} px down", 360 * steps);
        settled(cx, &at);
        let each = cx.update(|_, cx| media::held_each(cx));
        let held: u64 = each.iter().sum();
        let biggest = each.iter().copied().max().unwrap_or(0);
        assert!(biggest <= THUMB_MOST, "{at}: a thumbnail holds {biggest} bytes, past one 640 by 400 thumbnail");
        (most, most_count) = (most.max(held), most_count.max(each.len()));
        if cx.debug_bounds(thumb("gal2", EACH - 1)).is_some_and(|b| f32::from(b.bottom()) <= 1100.) {
            break;
        }
        scroll(cx, 360.);
        steps += 1;
        assert!(steps < 40, "the second gallery's last picture came into view");
    }
    eprintln!(
        "1170x2532 pictures in a 540x1100 window: at most {most_count} held, {most} bytes ({:.1} MiB)",
        most as f64 / (1 << 20) as f64
    );
}

/// The panel's bound, 64 MiB with no slack: it lets go as each picture lands, before the next draw.
fn within_bound(cx: &mut VisualTestContext, at: &str) -> u64 {
    let held = cx.update(|_, cx| media::held_bytes(cx));
    assert!(held <= 64 << 20, "{at}: the panel holds {held} bytes, past its 64 MiB");
    held
}

/// Waits until the panel holds `want(bytes)`, and says what it holds when it gives up.
fn holds(cx: &mut VisualTestContext, what: &str, want: impl Fn(u64) -> bool) -> u64 {
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        let held = cx.update(|_, cx| media::held_bytes(cx));
        if want(held) {
            return held;
        }
        assert!(Instant::now() < limit, "the panel never got to: {what}; it holds {held} bytes");
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update(|window, _| window.refresh());
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[gpui_kit::test]
fn a_picture_the_slate_stops_drawing_is_let_go(cx: &mut TestAppContext) {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "inline", file: None });
    let src = bench().1.join("shots/gallery/0.png");
    let stays = bench().1.join("shots/gallery/1.png");
    // A second picture stays drawn, or the panel's whole store goes with the last picture and hides the sweep.
    let slate = |image: &str| {
        format!("<slate title=\"Inline\">\n  <column>\n    <image src=\"{}\" />\n{image}  </column>\n</slate>\n", stays.display())
    };
    let shown = format!("    <image src=\"{}\" />\n", src.display());
    let hidden = format!("    <image src=\"{}\" when={{1 > 2}} />\n", src.display());
    fixture::write(host, &thread, &slate(&shown));
    let (_, cx) = open(cx, &thread);
    // Each picture is 1280 by 800, held whole inline, four bytes a pixel.
    let one = 1280 * 800 * 4;
    holds(cx, "both pictures decoded", |b| b == 2 * one);
    fixture::write(host, &thread, &slate(&hidden));
    holds(cx, "the picture its when hides let go", |b| b == one);
    fixture::write(host, &thread, &slate(&shown));
    holds(cx, "the picture shown again decoded", |b| b == 2 * one);
    fixture::write(host, &thread, &slate(""));
    holds(cx, "the picture a rewrite took away let go", |b| b == one);
    let section = |open: bool| format!("    <section title=\"Fold\" open={{{open}}}>\n  {shown}    </section>\n");
    fixture::write(host, &thread, &slate(&section(true)));
    holds(cx, "the picture in an open section decoded", |b| b == 2 * one);
    fixture::write(host, &thread, &slate(&section(false)));
    holds(cx, "the picture its section shut let go", |b| b == one);
}

/// Hovers `at` until the tooltip under it shows.
fn hover(cx: &mut VisualTestContext, at: gpui_kit::Point<gpui_kit::Pixels>) {
    cx.simulate_mouse_move(at, None, Modifiers::none());
    for _ in 0..2 {
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.update(|window, cx| window.simulate_next_frame(cx));
    }
    cx.run_until_parked();
}

/// A thread whose slate is one picture, opened at the window's size, and the point on its expand button.
fn expand_button<'a>(cx: &'a mut TestAppContext, name: &'static str) -> (&'a mut VisualTestContext, gpui_kit::Point<gpui_kit::Pixels>) {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name, file: None });
    let src = bench().1.join("shots/gallery/0.png");
    fixture::write(
        host,
        &thread,
        &format!("<slate title=\"Expand\">\n  <column>\n    <image src=\"{}\" />\n  </column>\n</slate>\n", src.display()),
    );
    let (_, cx) = open(cx, &thread);
    holds(cx, "the picture decoded", |b| b > 0);
    let image = cx.debug_bounds("slate-image").expect("the image piece is drawn");
    (cx, point(image.right() - px(15.), image.bottom() - px(12.)))
}

/// Lets `seconds` of the window's time pass, a frame every half second.
fn pass(cx: &mut VisualTestContext, seconds: u64) {
    for _ in 0..seconds * 2 {
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn the_expand_buttons_tooltip_goes_when_a_click_after_a_rest_opens_the_viewer(cx: &mut TestAppContext) {
    let (cx, at) = expand_button(cx, "expand-tip-rest");
    hover(cx, at);
    assert!(cx.debug_bounds("tooltip").is_some(), "the expand button says Expand while the pointer rests on it");
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("slate-image-expanded").is_some(), "the click opened the viewer");
    pass(cx, 5);
    assert!(cx.debug_bounds("tooltip").is_none(), "Expand stays drawn beside the viewer its button opened");
}

#[gpui_kit::test]
fn the_expand_buttons_tooltip_never_shows_when_a_quick_click_opens_the_viewer(cx: &mut TestAppContext) {
    let (cx, at) = expand_button(cx, "expand-tip-quick");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("slate-image-expanded").is_some(), "the click opened the viewer");
    pass(cx, 5);
    assert!(cx.debug_bounds("tooltip").is_none(), "Expand shows beside the viewer a click faster than its delay opened");
}
