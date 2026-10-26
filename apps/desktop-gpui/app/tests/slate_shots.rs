// SPDX-License-Identifier: AGPL-3.0-only
//! The slate panel in Paper and Graphite drawn by GPUI's real offscreen renderer, against a host started for this run
//! alone, at the panel body's 539 px and the window's scale of 2. Each state writes one of the
//! `parity/wave-2/data/*.slate` files onto a fresh thread. `SLATE_PNG=<folder>` writes `<theme>/<state>.png` there;
//! `SLATE_STATES=a,b` picks states, the press-driven ones in `fixture::PRESSED` among them. Without libtest's harness, as `pixels.rs` explains.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::slate::{consent, Load, Raise, SlateScreen};
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, ActivePalette, Side, Still, UiAssets};
use fixture::Act;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    div, point, prelude::*, px, size, AnyWindowHandle, Context, Entity, HeadlessAppContext, InputEvent as _, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Window,
};
use wsp_host_client::HostClient;

/// The right panel's body: its default 540 px (`PreviewPanelShell`) less the 1 px left border, under the 52 px tab bar
/// in an 800 px window.
const W: f32 = 539.;
const H: f32 = 748.;

/// `SLATE_H=<px>` draws a taller window, to see a whole slate at once.
fn height() -> f32 {
    std::env::var("SLATE_H").ok().and_then(|h| h.parse().ok()).unwrap_or(H)
}

struct Panel {
    screen: Entity<SlateScreen>,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.palette().background).child(self.screen.clone())
    }
}

fn shoot(theme: &str, host: &Host, thread: &str, acts: &[Act]) -> image::RgbaImage {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_global(Still(true));
        ui::theme::set_theme(theme, cx);
    });
    let opened = Opened { thread_id: Some(thread.to_owned()), workspace_id: None, tab: Some("slate".into()), arg: None };
    let mut made = None;
    let handle: AnyWindowHandle = cx
        .open_window(size(px(W), px(height())), |window, cx| {
            let screen = cx.new(|cx| SlateScreen::new(host.clone(), opened.clone(), window, cx));
            made = Some(screen.clone());
            let panel = cx.new(|_| Panel { screen });
            cx.new(|cx| Root::new(panel, window, cx))
        })
        .unwrap()
        .into();
    let screen = made.unwrap();
    let mut clock = Instant::now();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        park(&mut cx, handle, &mut clock);
        let ready = cx.update(|cx| {
            let s = screen.read(cx);
            match &s.load {
                Load::Drawn(slate) => !slate.resolved.is_empty() || slate.formulas().is_empty(),
                Load::Empty(_) | Load::NoThread => true,
                Load::Waiting => false,
            }
        });
        if ready || Instant::now() > until {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for act in acts {
        run_act(&mut cx, handle, &screen, act, &mut clock);
    }
    // A few frames: images and layouts that measure on the first settle on the next.
    for _ in 0..4 {
        park(&mut cx, handle, &mut clock);
        std::thread::sleep(Duration::from_millis(100));
    }
    cx.capture_screenshot(handle).expect("offscreen rendering must be available")
}

/// Moves the headless clock on by the wall time since the last call, so a timer (an outcome's 2 s fade, a sheet's
/// wait) fires when it would in the app, runs what that woke, and draws, as a window on screen keeps drawing.
fn park(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, clock: &mut Instant) {
    let now = Instant::now();
    cx.advance_clock(now - *clock);
    *clock = now;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
}

/// Runs the app until `done` holds of the panel, the host answering on its own threads meanwhile.
fn settle(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    clock: &mut Instant,
    screen: &Entity<SlateScreen>,
    what: &str,
    done: impl Fn(&SlateScreen) -> bool,
) {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        park(cx, handle, clock);
        if cx.update(|cx| done(screen.read(cx))) {
            return;
        }
        assert!(Instant::now() < until, "the panel never got to: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn run_act(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, screen: &Entity<SlateScreen>, act: &Act, clock: &mut Instant) {
    let menu = |cx: &mut HeadlessAppContext, f: fn(&mut SlateScreen, &mut Context<SlateScreen>)| cx.update(|cx| screen.update(cx, f));
    match act {
        Act::Press(id) => {
            cx.update(|cx| screen.update(cx, |s, cx| s.raise(id.to_string(), "press", Raise::default(), cx)));
            settle(cx, handle, clock, screen, "the press answered", |s| s.slate().is_some_and(|sl| !sl.busy.contains(*id)));
        }
        Act::Answer(scope) => {
            settle(cx, handle, clock, screen, "a sheet open", |s| consent::open(s).is_some());
            let scope: &'static str = scope;
            cx.update(|cx| screen.update(cx, |s, cx| s.decide(scope, cx)));
            settle(cx, handle, clock, screen, "the sheet answered", |s| consent::open(s).is_none());
        }
        Act::Until(run, state) => {
            settle(cx, handle, clock, screen, &format!("${run} {state}"), |s| s.record().and_then(|r| r.run_state(run)) == Some(*state))
        }
        Act::Lines(run, n) => settle(cx, handle, clock, screen, &format!("{n} lines of ${run}"), |s| {
            s.slate().is_some_and(|sl| sl.lines.get(*run).is_some_and(|l| l.len() >= *n))
        }),
        Act::Refreshing(run) => {
            settle(cx, handle, clock, screen, &format!("${run} refreshing"), |s| s.record().is_some_and(|r| r.refreshing(run)))
        }
        Act::Stop => menu(cx, |s, cx| s.stop_runs(cx)),
        Act::Menu => cx
            .update_window(handle, |_, window, cx| {
                // The trigger's centre: 28 px square at the column's right edge, 16 px in, under the head's 12 px.
                let position = point(px(W - 16. - 14.), px(12. + 14.));
                let (modifiers, button) = (Modifiers::default(), MouseButton::Left);
                window.dispatch_event(MouseMoveEvent { position, pressed_button: None, modifiers }.to_platform_input(), cx);
                window.dispatch_event(
                    MouseDownEvent { position, modifiers, button, click_count: 1, first_mouse: false }.to_platform_input(),
                    cx,
                );
                window.dispatch_event(MouseUpEvent { position, modifiers, button, click_count: 1 }.to_platform_input(), cx);
            })
            .unwrap(),
        Act::Escape => cx.update_window(handle, |_, window, cx| window.press("escape", cx)).unwrap(),
        Act::Pointer(x, y) => cx
            .update_window(handle, |_, window, cx| {
                let event = MouseMoveEvent { position: point(px(*x), px(*y)), pressed_button: None, modifiers: Modifiers::default() };
                window.dispatch_event(event.to_platform_input(), cx);
            })
            .unwrap(),
        Act::Approvals => menu(cx, |s, cx| {
            s.consent.standing = Some(consent::Standing::default());
            cx.notify();
        }),
        Act::Forget => menu(cx, |s, cx| {
            s.consent.forgetting = Some(consent::Forget::default());
            cx.notify();
        }),
        Act::Clear => {
            menu(cx, |s, cx| s.menu_write("<clear />", cx));
            settle(cx, handle, clock, screen, "the empty state", |s| matches!(s.load, Load::Empty(_)));
        }
    }
    park(cx, handle, clock);
}

fn main() {
    let out = std::env::var_os("SLATE_PNG").map(PathBuf::from);
    let only: Option<Vec<String>> = std::env::var("SLATE_STATES").ok().map(|s| s.split(',').map(str::to_owned).collect());
    let host = common::Throwaway::start_with(&fixture::stand_in(), false);
    let folder = fixture::project(&host);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let handle = Host::new(client, runtime);
    for state in fixture::STATES {
        if only.as_ref().is_some_and(|o| !o.iter().any(|s| s == state.name)) {
            continue;
        }
        let thread = fixture::staged(&host, &folder, state);
        for theme in ["paper", "graphite"] {
            let image = shoot(theme, &handle, &thread, &[]);
            assert_eq!((image.width(), image.height()), (2 * W as u32, 2 * height() as u32), "{} in {theme} is drawn at 2x", state.name);
            if let Some(out) = &out {
                std::fs::create_dir_all(out.join(theme)).unwrap();
                image.save(out.join(theme).join(format!("{}.png", state.name))).unwrap();
            }
            if state.name == "held" {
                held_value_draws_at_the_webs_ink(&image, theme);
            }
            println!("{:<12} {theme:<9} drawn", state.name);
        }
    }
    // A press-driven state acts on a thread of its own in each theme, since its acts move the host's record.
    for pressed in fixture::PRESSED {
        if only.as_ref().is_some_and(|o| !o.iter().any(|s| s == pressed.name)) {
            continue;
        }
        for theme in ["paper", "graphite"] {
            let thread = fixture::staged(&host, &folder, &fixture::State { name: pressed.name, file: Some(pressed.file) });
            let image = shoot(theme, &handle, &thread, pressed.acts);
            if let Some(out) = &out {
                std::fs::create_dir_all(out.join(theme)).unwrap();
                image.save(out.join(theme).join(format!("{}.png", pressed.name))).unwrap();
            }
            if pressed.name.starts_with("bar") {
                thumb_draws_its_token(&image, theme, pressed.name == "bar-hover");
            }
            println!("{:<12} {theme:<9} drawn", pressed.name);
        }
    }
}

/// The scrollbar's thumb in `--app-scrollbar-thumb`, or `--app-scrollbar-thumb-hover` under the pointer: Paper's
/// `#d9d9d9` and `#bfbfbf`, Graphite's white at 8% and 12% over its page. Read down the thumb's middle (device x 1070)
/// against the page beside it (x 1060), where they differ.
fn thumb_draws_its_token(image: &image::RgbaImage, theme: &str, hovered: bool) {
    let luma = |p: &image::Rgba<u8>| (u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])) as f32 / 3.;
    let rows: Vec<(f32, f32)> = (0..image.height())
        .map(|y| (luma(image.get_pixel(1070, y)), luma(image.get_pixel(1060, y))))
        .filter(|(thumb, page)| (thumb - page).abs() > 4.)
        .collect();
    assert!(rows.len() >= 32, "{theme}: no thumb drawn, {} rows differ from the page", rows.len());
    let (thumb, page) = rows[rows.len() / 2];
    let want = match (theme == "paper", hovered) {
        (true, false) => 217.,
        (true, true) => 191.,
        (false, false) => page + (255. - page) * 20. / 255.,
        (false, true) => page + (255. - page) * 31. / 255.,
    };
    let state = if hovered { "hovered" } else { "at rest" };
    assert!((thumb - want).abs() <= 2., "{theme}: the thumb {state} draws {thumb} on a page of {page}, not the web's {want}");
    println!("    {theme}: the thumb {state} draws {thumb} on a page of {page}, the web's {want}");
}

/// W2-SLT-050: the held field's value, "Ada", draws at the web's `opacity-64`, not a disabled gpui-kit input's half: its
/// strongest stroke in the field's box (device px 700..820 by 150..186 at 539 px) is 58% to 70% of the way from the page
/// to the theme's foreground (Paper's `#27272a`, Graphite's `#f5f5f5`); half would be 50%.
fn held_value_draws_at_the_webs_ink(image: &image::RgbaImage, theme: &str) {
    let luma = |p: &image::Rgba<u8>| (u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])) as f32 / 3.;
    let page = luma(image.get_pixel(990, 150));
    let strokes = (700..820).flat_map(|x| (150..186).map(move |y| (x, y))).map(|(x, y)| luma(image.get_pixel(x, y)));
    let (ink, far) = if theme == "paper" { (40., strokes.fold(255., f32::min)) } else { (245., strokes.fold(0., f32::max)) };
    let share = (far - page) / (ink - page);
    assert!(
        (0.58..=0.7).contains(&share),
        "held in {theme}: the value's strongest stroke is {far} on a page of {page}, {:.0}% of the ink",
        share * 100.
    );
    println!("    held in {theme}: the value's strongest stroke is {far} on a page of {page}, {:.0}% of the ink", share * 100.);
}
