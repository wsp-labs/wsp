// SPDX-License-Identifier: AGPL-3.0-only
//! The effects test's four scenes in one window, their rows scrolling, to look at backdrop blur and rounded clipping
//! on a real screen: `cargo run --example effects` in apps/desktop-gpui.

#[allow(dead_code)]
#[path = "../tests/effects/scene.rs"]
mod scene;

use std::time::Instant;

use gpui_kit::{div, prelude::*, px, size, App, AppContext, Bounds, Context, TitlebarOptions, Window, WindowBounds, WindowOptions};
use scene::{draw, Scene, H, W};

/// The web app's own scroll rate in the bench, slowed to a pace an eye can follow.
const PX_PER_S: f32 = 120.;

struct Grid {
    start: Instant,
}

impl Render for Grid {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.request_animation_frame();
        let scroll = (self.start.elapsed().as_secs_f32() * PX_PER_S) % 720.;
        div().flex().flex_wrap().w(px(2. * W)).children(Scene::ALL.map(|scene| draw(scene, scroll)))
    }
}

fn main() {
    gpui_kit::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(2. * W), px(2. * H)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions { title: Some("effects".into()), ..Default::default() }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Grid { start: Instant::now() }),
        )
        .expect("failed to open the window");
        cx.activate(true);
    });
}
