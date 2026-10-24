// SPDX-License-Identifier: AGPL-3.0-only
//! The View menu's zoom drawn by GPUI's real offscreen renderer (WGPU on Linux, Metal on macOS): two steps in, a box
//! and a line of text sized in px draw 1.2 times as large in device pixels, and the window lays out in a viewport
//! 1/1.2 the size. Runs without libtest's harness because AppKit panics unless it starts on the main thread.

use std::sync::Arc;

use desktop_gpui::desktop::{frame, zoom};
use desktop_gpui::screens::app_shell::assets::AppAssets;
use desktop_gpui::screens::app_shell::menu::Os;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    div, prelude::*, px, rgb, size, AnyWindowHandle, AppContext, Bounds, Context, HeadlessAppContext, Pixels, Size, Window, WindowBounds,
    WindowOptions,
};

const W: f32 = 400.;
const H: f32 = 300.;

/// A red 100 px box and a line of black 20 px text on white, both sized in px.
struct Sample;

impl Render for Sample {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .child(div().absolute().left(px(20.)).top(px(20.)).w(px(100.)).h(px(100.)).bg(rgb(0xff0000)))
            .child(div().absolute().left(px(20.)).top(px(140.)).text_size(px(20.)).text_color(rgb(0x000000)).child("Zoom sample"))
    }
}

struct Shot {
    width: u32,
    height: u32,
    raw: Vec<u8>,
}

impl Shot {
    fn at(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.raw[i], self.raw[i + 1], self.raw[i + 2]]
    }

    /// The device columns and rows spanned by the pixels `hit` picks: left, right, top, bottom, inclusive.
    fn span(&self, hit: impl Fn([u8; 3]) -> bool) -> (u32, u32, u32, u32) {
        let mut span = (u32::MAX, 0, u32::MAX, 0);
        for y in 0..self.height {
            for x in 0..self.width {
                if hit(self.at(x, y)) {
                    span = (span.0.min(x), span.1.max(x), span.2.min(y), span.3.max(y));
                }
            }
        }
        assert!(span.0 <= span.1, "nothing drawn");
        span
    }
}

fn red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 60 && p[2] < 60
}

fn ink(p: [u8; 3]) -> bool {
    p.iter().all(|&c| c < 160)
}

fn capture(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> (Shot, Size<Pixels>, f32) {
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    let (viewport, scale) = cx.update_window(handle, |_, window, _| (window.viewport_size(), window.scale_factor())).unwrap();
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    let (width, height) = image.dimensions();
    (Shot { width, height, raw: image.into_raw() }, viewport, scale)
}

fn main() {
    two_steps_in_draw_px_sizes_one_point_two_times_larger();
    println!("test two_steps_in_draw_px_sizes_one_point_two_times_larger ... ok");
}

/// Catches: a zoom that moves only the rem, so px boxes and px text keep their size, or a window that keeps laying
/// out in the screen's whole viewport.
fn two_steps_in_draw_px_sizes_one_point_two_times_larger() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        frame::init(cx);
        zoom::init(Os::current(), cx);
    });
    let handle: AnyWindowHandle = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(W), px(H)) })),
                focus: false,
                show: false,
                ..Default::default()
            };
            cx.open_window(options, |window, cx| {
                let view = cx.new(|_| Sample);
                cx.new(|cx| Root::new(view, window, cx))
            })
        })
        .unwrap()
        .into();

    let (actual, actual_viewport, actual_scale) = capture(&mut cx, handle);
    cx.update(|cx| {
        cx.dispatch_action(&zoom::ZoomIn);
        cx.dispatch_action(&zoom::ZoomIn);
    });
    let (zoomed, zoomed_viewport, zoomed_scale) = capture(&mut cx, handle);

    assert_eq!((zoomed.width, zoomed.height), (actual.width, actual.height), "the window keeps its size on screen");
    let device = |logical: f32| (logical * actual_scale).round() as u32;
    let actual_box = actual.span(red);
    assert_eq!(actual_box, (device(20.), device(120.) - 1, device(20.), device(120.) - 1), "the 100 px box at actual size");
    let zoomed_box = zoomed.span(red);
    let want = (device(24.), device(144.) - 1, device(24.), device(144.) - 1);
    assert_eq!(zoomed_box, want, "the 100 px box at 20 px, both 1.2x as large");

    let text = |shot: &Shot| {
        let (left, right, top, bottom) = shot.span(ink);
        ((right + 1 - left) as f32, (bottom + 1 - top) as f32, left)
    };
    let (actual_width, actual_height, actual_left) = text(&actual);
    let (zoomed_width, zoomed_height, zoomed_left) = text(&zoomed);
    let ratio = (zoomed_width / actual_width, zoomed_height / actual_height);
    assert!((ratio.0 - 1.2).abs() < 0.03 && (ratio.1 - 1.2).abs() < 0.08, "the px text's ink grew {ratio:?}");
    assert!(zoomed_left.abs_diff(device(24.)) <= 3 && actual_left.abs_diff(device(20.)) <= 3);
    assert!((zoomed_scale / actual_scale - 1.2).abs() < 1e-5, "scale {actual_scale} then {zoomed_scale}");
    for (got, was) in [(zoomed_viewport.width, actual_viewport.width), (zoomed_viewport.height, actual_viewport.height)] {
        assert!((f32::from(got) * 1.2 - f32::from(was)).abs() < 0.01, "viewport {actual_viewport:?} then {zoomed_viewport:?}");
    }
    println!("box {actual_box:?} then {zoomed_box:?}, text ink {actual_width}x{actual_height} then {zoomed_width}x{zoomed_height}, viewport {actual_viewport:?} then {zoomed_viewport:?}");
}
