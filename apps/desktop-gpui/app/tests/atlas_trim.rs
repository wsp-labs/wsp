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
    size: f32,
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
        div().size_full().text_size(px(self.size)).child(self.text.clone())
    }
}

const SHORT: &str = "The next thread, in a few words.";

/// One window at `scale`: the short page's glyph pages, then a wide page grown until its glyphs need pages the short
/// page does not, then the trim, which must give those back once the short page is drawn again.
fn trims_at(scale: f32, cx: &mut HeadlessAppContext) {
    // Three thousand ideographs, each a glyph of its own, as a thread of another script or a long table of code draws.
    let wide: SharedString = (0x4e00..0x4e00 + 3000).filter_map(char::from_u32).collect::<String>().into();
    let mut page = None;
    let (handle, _): (AnyWindowHandle, _) = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(gpui_kit::Bounds {
                    origin: Default::default(),
                    size: size(px(3000.), px(3000.)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                window.set_scale_factor(scale);
                let view = cx.new(|_| Page { text: SHORT.into(), size: 15. });
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
    let show = |cx: &mut HeadlessAppContext, text: SharedString, size: f32| {
        cx.update(|cx| {
            page.update(cx, |p, cx| {
                p.text = text;
                p.size = size;
                cx.notify();
            })
        });
    };
    draw(cx);
    let short = glyphs(cx);
    assert!(short > 0, "the short page draws its glyphs");
    // Larger glyphs until the wide page's need pages past the short page's, as the scale and the renderer's page decide.
    let mut full = short;
    for text_size in [15., 24., 36., 54.] {
        show(cx, wide.clone(), text_size);
        draw(cx);
        full = glyphs(cx);
        if full > short {
            break;
        }
    }
    assert!(full > short, "at scale {scale} the wide page drew no glyph page past the short page's {short} bytes");
    show(cx, SHORT.into(), 15.);
    draw(cx);
    assert_eq!(glyphs(cx), full, "the glyphs of a page left stay in the atlas until a trim lets them go");
    let trimmed = cx.update_window(handle, |_, window, _| warm::trim_atlas(window, full)).unwrap();
    assert!(!trimmed, "a trim under its bound lets nothing go");
    let trimmed = cx.update_window(handle, |_, window, _| warm::trim_atlas(window, short)).unwrap();
    assert!(trimmed, "{full} bytes of glyph pages are past the short page's {short}");
    draw(cx);
    let left = glyphs(cx);
    println!("glyph pages at scale {scale}: {short} bytes for the short page, {full} with the wide one drawn, {left} after a trim");
    assert!(left > 0, "the cached page drew its glyphs again after the trim");
    assert!(left <= short, "at scale {scale} the trim kept {left} of the {full} bytes of glyph pages, past the short page's {short}");
    cx.update_window(handle, |_, window, _| window.remove_window()).unwrap();
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
    // One as the Linux runner's software renderer draws, two as a Mac's display does.
    for scale in [1., 2.] {
        trims_at(scale, &mut cx);
    }
    println!("atlas_trim: ok");
}
