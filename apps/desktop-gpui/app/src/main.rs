// SPDX-License-Identifier: AGPL-3.0-only
use desktop_gpui::{Home, TITLE};
use gpui::{px, size, App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions};
use gpui_component::Root;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(900.), px(600.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions { title: Some(TITLE.into()), ..Default::default() }),
                ..Default::default()
            },
            |window, cx| {
                let home = cx.new(|_| Home);
                cx.new(|cx| Root::new(home, window, cx))
            },
        )
        .expect("failed to open the window");
        cx.activate(true);
    });
}
