// SPDX-License-Identifier: AGPL-3.0-only
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use road_b::Spike;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Spike),
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}
