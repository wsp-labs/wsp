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

/// What the page shows: the short line, or the wide page's rows, each the printable ASCII at one text size.
enum Shows {
    Short,
    Rows(Vec<f32>),
}

struct Page {
    shows: Shows,
    /// A family to draw in alone, as `ATLAS_FONT` names one, to match a computer whose fonts are few.
    family: Option<SharedString>,
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

const SHORT: &str = "The next thread, in a few words.";

/// Every printable ASCII character, which every font a computer has draws: each at each size is a tile of its own.
fn ascii() -> SharedString {
    (0x21u8..0x7f).map(char::from).collect::<String>().into()
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let page = div().size_full().flex().flex_col().when_some(self.family.clone(), |el, family| el.font_family(family));
        match &self.shows {
            Shows::Short => page.text_size(px(15.)).child(SHORT),
            Shows::Rows(sizes) => page.children(sizes.iter().map(|&size| div().text_size(px(size)).child(ascii()))),
        }
    }
}

/// One window at `scale`: the short page's glyph pages, then a wide page of a row more at each size until its glyphs
/// need pages the short page does not, then the trim, which must give those back once the short page is drawn again.
fn trims_at(scale: f32, family: Option<SharedString>, cx: &mut HeadlessAppContext) {
    let mut page = None;
    let (handle, _): (AnyWindowHandle, _) = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(gpui_kit::Bounds {
                    origin: Default::default(),
                    size: size(px(2400.), px(3000.)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                window.set_scale_factor(scale);
                let view = cx.new(|_| Page { shows: Shows::Short, family });
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
    let show = |cx: &mut HeadlessAppContext, shows: Shows| {
        cx.update(|cx| {
            page.update(cx, |p, cx| {
                p.shows = shows;
                cx.notify();
            })
        });
    };
    draw(cx);
    let short = glyphs(cx);
    assert!(short > 0, "the short page draws its glyphs");
    // A row more at each size, from 12 to 96 px, until the atlas holds pages past the short page's: how many depends on
    // the scale, the fonts and the renderer's page, so it is read off the atlas.
    let (mut sizes, mut full) = (Vec::new(), short);
    for size in (12..=96).step_by(6) {
        sizes.push(size as f32);
        show(cx, Shows::Rows(sizes.clone()));
        draw(cx);
        full = glyphs(cx);
        if full > short {
            break;
        }
    }
    assert!(
        full > short,
        "at scale {scale} the printable ASCII at {} sizes from 12 to 96 px drew no glyph tile past the short page's {short} bytes of \
         pages: the fonts here drew no distinct glyph for them",
        sizes.len()
    );
    show(cx, Shows::Short);
    draw(cx);
    assert_eq!(glyphs(cx), full, "the glyphs of a page left stay in the atlas until a trim lets them go");
    let trimmed = cx.update_window(handle, |_, window, _| warm::trim_atlas(window, full)).unwrap();
    assert!(!trimmed, "a trim under its bound lets nothing go");
    let trimmed = cx.update_window(handle, |_, window, _| warm::trim_atlas(window, short)).unwrap();
    assert!(trimmed, "{full} bytes of glyph pages are past the short page's {short}");
    draw(cx);
    let left = glyphs(cx);
    println!(
        "glyph pages at scale {scale}: {short} bytes for the short page, {full} with rows at {} sizes drawn, {left} after a trim",
        sizes.len()
    );
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
    let family = std::env::var("ATLAS_FONT").ok().map(SharedString::from);
    // One as the Linux runner's software renderer draws, two as a Mac's display does.
    for scale in [1., 2.] {
        trims_at(scale, family.clone(), &mut cx);
    }
    println!("atlas_trim: ok");
}
