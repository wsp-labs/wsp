// SPDX-License-Identifier: AGPL-3.0-only
//! The slate panel in Paper and Graphite drawn by GPUI's real offscreen renderer, against a host started for this run
//! alone, at the right panel's default 540 px and the window's scale of 2. Each state writes one of the coordinator's
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

/// The right panel's default width (`PreviewPanelShell`), and its body under the 52 px tab bar in an 800 px window.
const W: f32 = 540.;
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
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
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
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        run_act(&mut cx, handle, &screen, act);
    }
    // A few frames: images and layouts that measure on the first settle on the next.
    for _ in 0..4 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    }
    cx.capture_screenshot(handle).expect("offscreen rendering must be available")
}

/// Runs the app until `done` holds of the panel, the host answering on its own threads meanwhile.
fn settle(cx: &mut HeadlessAppContext, screen: &Entity<SlateScreen>, what: &str, done: impl Fn(&SlateScreen) -> bool) {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        cx.run_until_parked();
        if cx.update(|cx| done(screen.read(cx))) {
            return;
        }
        assert!(Instant::now() < until, "the panel never got to: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn run_act(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, screen: &Entity<SlateScreen>, act: &Act) {
    let menu = |cx: &mut HeadlessAppContext, f: fn(&mut SlateScreen, &mut Context<SlateScreen>)| cx.update(|cx| screen.update(cx, f));
    match act {
        Act::Press(id) => {
            cx.update(|cx| screen.update(cx, |s, cx| s.raise(id.to_string(), "press", Raise::default(), cx)));
            settle(cx, screen, "the press answered", |s| s.slate().is_some_and(|sl| !sl.busy.contains(*id)));
        }
        Act::Answer(scope) => {
            settle(cx, screen, "a sheet open", |s| consent::open(s).is_some());
            let scope: &'static str = scope;
            cx.update(|cx| screen.update(cx, |s, cx| s.decide(scope, cx)));
            settle(cx, screen, "the sheet answered", |s| consent::open(s).is_none());
        }
        Act::Until(run, state) => {
            settle(cx, screen, &format!("${run} {state}"), |s| s.record().and_then(|r| r.run_state(run)) == Some(*state))
        }
        Act::Lines(run, n) => settle(cx, screen, &format!("{n} lines of ${run}"), |s| {
            s.slate().is_some_and(|sl| sl.lines.get(*run).is_some_and(|l| l.len() >= *n))
        }),
        Act::Refreshing(run) => settle(cx, screen, &format!("${run} refreshing"), |s| s.record().is_some_and(|r| r.refreshing(run))),
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
            settle(cx, screen, "the empty state", |s| matches!(s.load, Load::Empty(_)));
        }
    }
    cx.run_until_parked();
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
            println!("{:<12} {theme:<9} drawn", pressed.name);
        }
    }
}
