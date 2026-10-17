// SPDX-License-Identifier: AGPL-3.0-only
use std::path::PathBuf;
use std::sync::Arc;

use desktop_gpui::{bench, shell::Shell, TITLE};
use gpui_kit::component::Root;
use gpui_kit::{px, size, App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions};

/// The state file a launch means, as the Electron app picks it: WSP_HOME names the home outright, else ~/.wsp.
fn default_state() -> PathBuf {
    let home = std::env::var_os("WSP_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".wsp"));
    home.join("state.json")
}

fn main() {
    let args = bench::parse(std::env::args().skip(1)).unwrap_or_else(|e| {
        eprintln!("desktop-gpui: {e}");
        std::process::exit(64);
    });
    let state = args.state.unwrap_or_else(default_state);
    let env = std::env::vars().collect();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("a tokio runtime"));
    let (bench, open, glass) = (args.bench, args.open, args.glass);
    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        // The Electron app's window size, so both benches scroll the same viewport.
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions { title: Some(TITLE.into()), ..Default::default() }),
                ..Default::default()
            },
            |window, cx| {
                let shell = cx.new(|cx| Shell::new(state, env, runtime, bench, open, glass, cx));
                cx.new(|cx| Root::new(shell, window, cx))
            },
        )
        .expect("failed to open the window");
        cx.activate(true);
    });
}
