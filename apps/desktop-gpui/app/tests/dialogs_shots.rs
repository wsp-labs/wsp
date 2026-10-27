// SPDX-License-Identifier: AGPL-3.0-only
//! The add flows, onboarding, the starting window, first run and the pair screen in Paper and Graphite, drawn by GPUI's
//! real offscreen renderer at the test window's scale of 2, a Retina Mac's, each state staged from fixture answers in a
//! window the size `parity/baseline.mjs` shot it at. `DIALOGS_PNG=<folder>` writes each capture there as
//! `<id>-<theme>.png`. With `DIALOGS_WEB=<folder>` holding a `baseline.mjs --wave 2` run (its PNGs and `index.json`),
//! it also crops the capture to the baseline's clip, writes `<id>-<theme>-diff.png` and `<id>-<theme>-pair.png` and
//! prints the share of pixels where any channel differs by more than 15 of 255. `DIALOGS_ONLY=<prefix>` shoots the
//! states whose id starts with it. Without libtest's harness, as `pixels.rs` explains.

#[path = "dialogs/cloud.rs"]
mod cloud;
#[path = "dialogs/computer.rs"]
mod computer;
#[path = "dialogs/onboard.rs"]
mod onboard;
#[path = "dialogs/pair.rs"]
mod pair;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use desktop_gpui::screens::dialogs::layer;
use desktop_gpui::ui::{self, ActivePalette, Side, Still, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{div, prelude::*, px, size, AnyView, AnyWindowHandle, App, Context, HeadlessAppContext, Window};
use image::RgbaImage;
use serde_json::Value;

/// One state: its spec id, the window it is drawn in (CSS px), and what stands in it. `stage` builds the window's
/// view and may open a dialog on the layer, which the window draws over that view.
pub struct State {
    pub id: &'static str,
    pub width: f32,
    pub height: f32,
    pub stage: fn(&mut Window, &mut App) -> AnyView,
}

impl State {
    /// A state in the 1280 x 800 window `baseline.mjs` shoots by default.
    pub fn new(id: &'static str, stage: fn(&mut Window, &mut App) -> AnyView) -> Self {
        State { id, width: 1280., height: 800., stage }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

/// An empty page in the theme's ground, for a state that is only a dialog on the layer.
pub fn page(_: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_| Page).into()
}

struct Page;

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.palette().background)
    }
}

/// The staged view with the dialogs' layer over it, as the shell draws it.
struct Stage {
    view: AnyView,
}

impl Render for Stage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.view.clone()).children(layer::draw(window, cx))
    }
}

fn shoot(theme: &str, state: &State) -> RgbaImage {
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
    let stage = state.stage;
    let handle: AnyWindowHandle = cx
        .open_window(size(px(state.width), px(state.height)), move |window, cx| {
            let view = stage(window, cx);
            let stage = cx.new(|_| Stage { view });
            cx.new(|cx| Root::new(stage, window, cx))
        })
        .unwrap()
        .into();
    // Frames enough for a layout that measures before it settles, and for the layer's fade to end.
    for _ in 0..4 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    cx.capture_screenshot(handle).expect("offscreen rendering must be available")
}

/// The baseline's entry for a state and theme in `index.json`.
fn web_entry(index: &Value, id: &str, theme: &str) -> Option<Value> {
    index.as_array()?.iter().find(|e| e["state"] == id && e["theme"] == theme && e["reached"] == true).cloned()
}

/// The capture cut to the baseline's clip (the whole window without one); the share of pixels where any channel
/// differs by more than 15 of 255, an image of where (red over the baseline's pixels, dimmed) and the pair side by side.
fn diff(web: &RgbaImage, gpui: &RgbaImage, clip: &Value) -> (f64, RgbaImage, RgbaImage) {
    let n = |k: &str| (clip[k].as_f64().unwrap_or(0.) * 2.).round() as i64;
    let (x, y) = if clip.is_null() { (0, 0) } else { (n("x"), n("y")) };
    let (w, h) = (web.width() as i64, web.height() as i64);
    let mut out = RgbaImage::new(w as u32, h as u32);
    let mut pair = RgbaImage::new(2 * w as u32 + 8, h as u32);
    let mut differ = 0u64;
    for j in 0..h {
        for i in 0..w {
            let a = *web.get_pixel(i as u32, j as u32);
            let b = gpui.get_pixel_checked((x + i) as u32, (y + j) as u32).copied().unwrap_or(image::Rgba([0, 0, 0, 0]));
            let off = (0..3).any(|c| a[c].abs_diff(b[c]) > 15);
            differ += u64::from(off);
            let dim = |v: u8| 128 + v / 2;
            out.put_pixel(
                i as u32,
                j as u32,
                if off { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([dim(a[0]), dim(a[1]), dim(a[2]), 255]) },
            );
            pair.put_pixel(i as u32, j as u32, a);
            pair.put_pixel((w + 8 + i) as u32, j as u32, b);
        }
    }
    (100. * differ as f64 / (w * h).max(1) as f64, out, pair)
}

fn main() {
    let out = std::env::var_os("DIALOGS_PNG").map(PathBuf::from);
    let web = std::env::var_os("DIALOGS_WEB").map(PathBuf::from);
    let only = std::env::var("DIALOGS_ONLY").unwrap_or_default();
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let index: Option<Value> =
        web.as_deref().and_then(|web: &Path| serde_json::from_slice(&std::fs::read(web.join("index.json")).ok()?).ok());
    let states: Vec<State> = [computer::states(), cloud::states(), onboard::states(), pair::states()]
        .into_iter()
        .flatten()
        .filter(|s| s.id.starts_with(&only))
        .collect();
    for theme in ["paper", "graphite"] {
        for state in &states {
            let image = shoot(theme, state);
            assert_eq!(
                (image.width(), image.height()),
                ((2. * state.width) as u32, (2. * state.height) as u32),
                "{} in {theme} is drawn at 2x",
                state.id
            );
            if let Some(out) = &out {
                image.save(out.join(format!("{}-{theme}.png", state.id))).unwrap();
            }
            let entry = index.as_ref().and_then(|index| web_entry(index, state.id, theme));
            if let (Some(web), Some(entry)) = (&web, entry) {
                let shot = image::open(web.join(entry["file"].as_str().unwrap_or_default())).expect("the baseline").to_rgba8();
                let (share, picture, pair) = diff(&shot, &image, &entry["clip"]);
                println!("{:<12} {theme:<9} {share:6.2}%", state.id);
                if let Some(out) = &out {
                    picture.save(out.join(format!("{}-{theme}-diff.png", state.id))).unwrap();
                    pair.save(out.join(format!("{}-{theme}-pair.png", state.id))).unwrap();
                }
            }
        }
        println!("test every_dialog_state_draws_in_{theme} ... ok");
    }
    a_select_list_in_the_dialog_draws_over_it(out.as_deref());
}

/// Catches: a select's list drawn under the dialog it opens in, so it takes presses nobody can see.
fn a_select_list_in_the_dialog_draws_over_it(out: Option<&Path>) {
    use desktop_gpui::screens::dialogs::add_computer::{model::AddStep, List};
    let closed = State::new("agents-order-closed", |w, cx| {
        computer::open(w, cx, |this, _, cx| computer::on_picks(this, AddStep::Agents, cx));
        page(w, cx)
    });
    let open = State::new("agents-order-open", |w, cx| {
        computer::open(w, cx, |this, _, cx| {
            computer::on_picks(this, AddStep::Agents, cx);
            this.list_state(List::Agents).update(cx, |l, cx| l.open_order(cx));
        });
        page(w, cx)
    });
    for theme in ["paper", "graphite"] {
        let (before, after) = (shoot(theme, &closed), shoot(theme, &open));
        if let Some(out) = out {
            before.save(out.join(format!("{}-{theme}.png", closed.id))).unwrap();
            after.save(out.join(format!("{}-{theme}.png", open.id))).unwrap();
        }
        let moved = before.pixels().zip(after.pixels()).filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 15)).count();
        // The open list moves some 9,000 pixels on Graphite's dark ground and more on Paper; one under the dialog, none.
        assert!(moved > 2_000, "the order's open list shows over the dialog in {theme}: {moved} pixels moved");
        println!("test a_select_list_in_the_dialog_draws_over_it_in_{theme} ... ok");
    }
}
