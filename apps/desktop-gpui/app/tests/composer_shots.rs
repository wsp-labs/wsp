// SPDX-License-Identifier: AGPL-3.0-only
//! Every composer state in Paper and Graphite drawn by GPUI's real offscreen renderer, in a window the size of the web
//! app's thread column at 1280 x 800 (1024 x 800 beside its 256 px sidebar), so a capture lines up with the web
//! reference shot's column pixel for pixel, at the test window's scale of 2, a Retina Mac's. `COMPOSER_PNG=<folder>`
//! writes each capture there as `<state>-<theme>.png`.
//! Without libtest's harness, as `pixels.rs` explains.

#[path = "composer/states.rs"]
mod states;

use std::path::PathBuf;
use std::sync::Arc;

use desktop_gpui::screens::composer::screen::column;
use desktop_gpui::screens::composer::{self, Composer};
use desktop_gpui::ui::{self, ActivePalette, Side, Still, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{prelude::*, px, size, AnyWindowHandle, Context, Entity, HeadlessAppContext, Window};
use image::RgbaImage;

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

fn shoot(theme: &str, state: &str) -> RgbaImage {
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
    // Two frames: the second lays the one-line layout out at the width the first measured.
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    cx.capture_screenshot(handle).expect("offscreen rendering must be available")
}

fn main() {
    let out = std::env::var_os("COMPOSER_PNG").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    for theme in ["paper", "graphite"] {
        for state in states::STATES {
            let image = shoot(theme, state);
            assert_eq!((image.width(), image.height()), (2 * W as u32, 2 * H as u32), "{state} in {theme} is drawn at 2x");
            // The box's left hairline, on the column's 128 px margin, is drawn in every state.
            let ground = *image.get_pixel(40, 40);
            let edge_drawn = (80..2 * H as u32 - 80).any(|y| *image.get_pixel(256, y) != ground);
            assert!(edge_drawn, "{state} in {theme} draws the box's edge at x 128");
            if let Some(out) = &out {
                image.save(out.join(format!("{state}-{theme}.png"))).unwrap();
            }
        }
        println!("test every_state_draws_in_{theme} ... ok");
    }
}
