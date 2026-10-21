// SPDX-License-Identifier: AGPL-3.0-only
//! A window's glyph tiles after the person moves on, drawn by GPUI's offscreen renderer into its real atlas: GPUI keeps
//! every glyph a window ever drew, so each thread walked through adds pages for good. A switch past the bound lets
//! them go, and the next frame draws again only what is on screen. Without libtest's harness, as `pixels.rs` explains.

use std::sync::Arc;

use desktop_gpui::screens::warm;
use desktop_gpui::ui::{self, Side, UiAssets};
use gpui_kit::{
    div, prelude::*, px, size, AnyWindowHandle, Context, HeadlessAppContext, SharedString, StyleRefinement, Window, WindowBounds,
    WindowOptions,
};

struct Page {
    text: SharedString,
}

/// The window's root, drawing the page as a cached view, which replays its last paint until the page changes.
struct Frame {
    page: gpui_kit::Entity<Page>,
}

impl Render for Frame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.page.clone().cached(StyleRefinement::default().size_full()))
    }
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().text_size(px(15.)).child(self.text.clone())
    }
}

fn main() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    // Three thousand ideographs, each a glyph of its own, as a thread of another script or a long table of code draws.
    let wide: String = (0x4e00..0x4e00 + 3000).filter_map(char::from_u32).collect();
    let mut page = None;
    let (handle, _): (AnyWindowHandle, _) = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(gpui_kit::Bounds {
                    origin: Default::default(),
                    size: size(px(1200.), px(900.)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |_, cx| {
                let view = cx.new(|_| Page { text: wide.into() });
                page = Some(view.clone());
                cx.new(|_| Frame { page: view })
            })
        })
        .unwrap();
    let page = page.unwrap();
    let draw = |cx: &mut HeadlessAppContext| {
        // Not the test helper's frame, which refreshes the window first and so redraws every cached view whatever the trim did.
        cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
        cx.run_until_parked();
    };
    let glyphs = |cx: &mut HeadlessAppContext| cx.update_window(handle, |_, window, _| window.atlas_usage().glyphs).unwrap();
    draw(&mut cx);
    let full = glyphs(&mut cx);
    cx.update(|cx| {
        page.update(cx, |p, cx| {
            p.text = "The next thread, in a few words.".into();
            cx.notify();
        })
    });
    draw(&mut cx);
    assert_eq!(glyphs(&mut cx), full, "the glyphs of a page left stay in the atlas until a trim lets them go");
    let trimmed = cx.update_window(handle, |_, window, _| warm::trim_atlas(window, full)).unwrap();
    assert!(!trimmed, "a trim under its bound lets nothing go");
    let trimmed = cx.update_window(handle, |_, window, _| warm::trim_atlas(window, full / 2)).unwrap();
    assert!(trimmed, "{full} bytes of glyph pages are past half of themselves");
    draw(&mut cx);
    let left = glyphs(&mut cx);
    println!("glyph pages: {full} bytes with the wide page drawn, {left} once the short page is drawn after a trim");
    assert!(left > 0, "the cached page drew its glyphs again after the trim");
    assert!(left < full, "the trim gave back none of the {full} bytes of glyph pages the short page does not draw");
    println!("atlas_trim: ok");
}
