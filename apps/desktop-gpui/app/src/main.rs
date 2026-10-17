// SPDX-License-Identifier: AGPL-3.0-only
use std::path::PathBuf;
use std::sync::Arc;

use desktop_gpui::gallery::Gallery;
use desktop_gpui::screens::app_shell::{assets::AppAssets, menu};
use desktop_gpui::screens::composer;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side};
use desktop_gpui::{bench, local_store, screens, TITLE};
use gpui_kit::component::Root;
use gpui_kit::{point, px, size, App, AppContext, Bounds, TitlebarOptions, WindowAppearance, WindowBounds, WindowOptions};
use wsp_host_client::Env;

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
    if let Some(name) =
        args.screen.as_deref().map(|route| screens::app_shell::split(route).0).filter(|name| screens::screen(name).is_none())
    {
        eprintln!("desktop-gpui: no screen is named {name}");
        std::process::exit(64);
    }
    // A launch on a named state keeps its store beside that state, so a capture or a bench on a throwaway host never
    // writes into the person's own.
    let kept = match &args.state {
        Some(state) => state.with_file_name("desktop-gpui-local-store.json"),
        None => local_store::default_path(),
    };
    let state = args.state.unwrap_or_else(default_state);
    let env: Env = std::env::vars().collect();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("a tokio runtime"));
    let gallery = args.gallery;
    let launch = Launch { bench: args.bench, open: args.open, glass: args.glass, screen: args.screen };
    let app = gpui_kit::application().with_assets(AppAssets);
    let reopen = (state.clone(), env.clone(), runtime.clone());
    // Electron's activate: the dock's icon on a Mac app with no window opens one again.
    app.on_reopen(move |cx| {
        if cx.windows().is_empty() {
            let (state, env, runtime) = reopen.clone();
            open_main(state, env, runtime, Launch::default(), cx);
        }
    });
    app.run(move |cx: &mut App| {
        gpui_kit::init(cx);
        // A Mac app lives on in the menu bar with its last window closed, as Electron's does.
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() && !cfg!(target_os = "macos") {
                cx.quit();
            }
        })
        .detach();
        let dark = matches!(cx.window_appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark);
        ui::init(if dark { Side::Dark } else { Side::Light }, cx);
        local_store::init(kept.clone(), cx);
        composer::init(cx);
        if gallery {
            let bounds = Bounds::centered(None, size(px(760.), px(900.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions { title: Some("wsp gallery".into()), ..Default::default() }),
                    ..Default::default()
                },
                |window, cx| {
                    let gallery = cx.new(|cx| Gallery::new(None, window, cx));
                    cx.new(|cx| Root::new(gallery, window, cx))
                },
            )
            .expect("failed to open the gallery");
            cx.activate(true);
            return;
        }
        menu::init(cx);
        open_main(state, env, runtime, launch, cx);
    });
}

/// The app's window, the shell in it.
fn open_main(state: PathBuf, env: Env, runtime: Arc<tokio::runtime::Runtime>, launch: Launch, cx: &mut App) {
    // The Electron app's window size, so both benches scroll the same viewport.
    let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
    // Electron's hiddenInset: no title bar, the lights centred on the frame row's 26 px line.
    let titlebar = if cfg!(target_os = "macos") {
        TitlebarOptions { title: Some(TITLE.into()), appears_transparent: true, traffic_light_position: Some(point(px(16.), px(19.))) }
    } else {
        TitlebarOptions { title: Some(TITLE.into()), ..Default::default() }
    };
    cx.open_window(
        WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), titlebar: Some(titlebar), ..Default::default() },
        |window, cx| {
            let shell = cx.new(|cx| Shell::new(state, env, runtime, launch, window, cx));
            cx.new(|cx| Root::new(shell, window, cx))
        },
    )
    .expect("failed to open the window");
    cx.activate(true);
}
