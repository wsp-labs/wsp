// SPDX-License-Identifier: AGPL-3.0-only
//! Every composer state in Paper and Graphite drawn by GPUI's real offscreen renderer, in a window the size of the web
//! app's thread column at 1280 x 800 (1024 x 800 beside its 256 px sidebar), so a capture lines up with the web
//! reference shot's column pixel for pixel, at the test window's scale of 2, a Retina Mac's. `COMPOSER_PNG=<folder>`
//! writes each capture there as `<state>-<theme>.png`. With `COMPOSER_WEB=<folder>` holding the web app's shots of the
//! same states from `tests/composer/web/run.sh`, it also crops both to the web's crop, writes `<state>-<theme>-diff.png`
//! and prints the share of pixels where any channel differs by more than 6% of 255.
//! Without libtest's harness, as `pixels.rs` explains.

#[path = "composer/states.rs"]
mod states;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use desktop_gpui::screens::composer::model::{Drawer, Picks};
use desktop_gpui::screens::composer::screen::column;
use desktop_gpui::screens::composer::{self, Composer};
use desktop_gpui::ui::{self, ActivePalette, Side, Still, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{prelude::*, px, size, AnyWindowHandle, Context, Entity, HeadlessAppContext, Window};
use image::RgbaImage;
use serde_json::Value;

const W: f32 = 1024.;
const H: f32 = 800.;

/// The thread column's ground with the composer at its foot, as the web's dock pins it.
struct Column {
    composer: Entity<Composer>,
}

impl Render for Column {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fresh = self.composer.read(cx).fresh();
        column(fresh, cx.palette().background, self.composer.clone())
    }
}

/// What a web shot shows that moves with its run: the limit's reset time, how many threads the slot row counts, and the
/// model, which the stand-in's turns leave to the catalog's default.
#[derive(Default)]
struct Moving {
    limit: Option<String>,
    slot: Option<String>,
    catalog_model: bool,
}

fn shoot(theme: &str, state: &str, moving: &Moving) -> RgbaImage {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
        cx.set_global(Still(true));
        ui::theme::set_theme(theme, cx);
    });
    let handle: AnyWindowHandle = cx
        .open_window(size(px(W), px(H)), |window, cx| {
            let composer = cx.new(|cx| Composer::new(window, cx));
            let column = cx.new(|_| Column { composer: composer.clone() });
            cx.new(|cx| Root::new(column, window, cx))
        })
        .unwrap()
        .into();
    let composer = cx
        .update_window(handle, |root, _, cx| {
            let root = root.downcast::<Root>().unwrap();
            let column = root.read(cx).view().clone().downcast::<Column>().unwrap();
            column.read(cx).composer.clone()
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| composer.update(cx, |c, cx| states::stage(state, c, window, cx))).unwrap();
    cx.update_window(handle, |_, window, cx| {
        composer.update(cx, |c, cx| {
            if let Some(line) = moving.limit.clone().filter(|_| state == "drawer-limit") {
                c.set_drawer(Drawer { limit: Some(("Usage limit reached".into(), line)), ..Drawer::default() }, cx);
            }
            if let Some(line) = moving.slot.clone().filter(|_| state == "drawer-slot") {
                c.set_drawer(Drawer { slot: Some(line), ..Drawer::default() }, cx);
            }
            if moving.catalog_model && !c.fresh() {
                let ran = Picks { model: None, ..states::ran() };
                c.set_catalog(states::agents(), ran, Vec::new(), cx);
            }
            let _ = window;
        })
    })
    .unwrap();
    // Three frames: the second lays the one-line layout out at the width the first measured.
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    cx.capture_screenshot(handle).expect("offscreen rendering must be available")
}

/// The web shot's JSON for a state.
fn web_json(web: &Path, state: &str, theme: &str) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(web.join(format!("{state}-{theme}.json"))).ok()?).ok()
}

/// The first string in the web shot's `data` holding `needle`.
fn web_text(json: &Value, needle: &str) -> Option<String> {
    match json {
        Value::String(s) if s.contains(needle) => Some(s.clone()),
        Value::Array(items) => items.iter().find_map(|v| web_text(v, needle)),
        Value::Object(map) => map.values().find_map(|v| web_text(v, needle)),
        _ => None,
    }
}

/// How far down the GPUI capture's crop starts against the web's, in device px: pages place a new thread's box, not
/// the composer. ChatView centres it 5.5 px lower than the New thread page does, and the project home docks its box at
/// the foot, 210 px under where ChatView centres it.
fn page_offset(state: &str) -> i64 {
    match state {
        "new-thread" | "fast-open" => 11,
        "resume-banner" => -420,
        _ => 0,
    }
}

/// Both captures cropped to the web's crop; the share of pixels where any channel differs by more than 15 of 255, and
/// an image of where: red over the web's pixels, dimmed.
fn diff(web: &RgbaImage, gpui: &RgbaImage, crop: &Value, dy: i64) -> (f64, RgbaImage) {
    let n = |k: &str| (crop[k].as_f64().unwrap_or(0.) * 2.).round() as i64;
    let (x, y, w, h) = (n("x"), n("y"), n("width"), n("height"));
    let mut out = RgbaImage::new(w as u32, h as u32);
    let mut differ = 0u64;
    for j in 0..h {
        for i in 0..w {
            let a = web.get_pixel_checked((x + i) as u32, (y + j) as u32).copied().unwrap_or(image::Rgba([0, 0, 0, 0]));
            let b = gpui.get_pixel_checked((x - 512 + i) as u32, (y + j + dy) as u32).copied().unwrap_or(image::Rgba([0, 0, 0, 0]));
            let off = (0..3).any(|c| a[c].abs_diff(b[c]) > 15);
            differ += u64::from(off);
            let dim = |v: u8| 128 + v / 2;
            out.put_pixel(
                i as u32,
                j as u32,
                if off { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([dim(a[0]), dim(a[1]), dim(a[2]), 255]) },
            );
        }
    }
    (100. * differ as f64 / (w * h).max(1) as f64, out)
}

fn main() {
    let out = std::env::var_os("COMPOSER_PNG").map(PathBuf::from);
    let web = std::env::var_os("COMPOSER_WEB").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    for theme in ["paper", "graphite"] {
        for state in states::STATES {
            let json = web.as_deref().and_then(|web| web_json(web, state, theme));
            let moving = json
                .as_ref()
                .map(|j| Moving {
                    limit: web_text(&j["data"], "resumes at"),
                    slot: web_text(&j["data"], " is running "),
                    catalog_model: !j["data"].as_array().is_some_and(|d| d.iter().any(|v| v == "Sonnet 5")),
                })
                .unwrap_or_default();
            let image = shoot(theme, state, &moving);
            assert_eq!((image.width(), image.height()), (2 * W as u32, 2 * H as u32), "{state} in {theme} is drawn at 2x");
            // The box's left hairline, on the column's 128 px margin, is drawn in every state.
            let ground = *image.get_pixel(40, 40);
            let edge_drawn = (80..2 * H as u32 - 80).any(|y| *image.get_pixel(256, y) != ground);
            assert!(edge_drawn, "{state} in {theme} draws the box's edge at x 128");
            if let Some(out) = &out {
                image.save(out.join(format!("{state}-{theme}.png"))).unwrap();
            }
            if let (Some(web), Some(json)) = (&web, &json) {
                let shot = image::open(web.join(format!("{state}-{theme}.png"))).expect("the web shot").to_rgba8();
                let (share, picture) = diff(&shot, &image, &json["crop"], page_offset(state));
                println!("{state:<20} {theme:<9} {share:6.2}%");
                if let Some(out) = &out {
                    picture.save(out.join(format!("{state}-{theme}-diff.png"))).unwrap();
                }
            }
        }
        println!("test every_state_draws_in_{theme} ... ok");
    }
}
