// SPDX-License-Identifier: AGPL-3.0-only
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use desktop_gpui::desktop::{frame, links, switch};
use desktop_gpui::gallery::Gallery;
use desktop_gpui::screens::app_shell::{assets::AppAssets, menu};
use desktop_gpui::screens::dialogs::startup::{self, Claim};
use desktop_gpui::screens::{composer, Place, Route};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side};
use desktop_gpui::{bench, local_store, screens};
use futures::StreamExt as _;
use gpui_kit::component::Root;
use gpui_kit::{px, size, App, AppContext, Bounds, TitlebarOptions, WindowAppearance, WindowBounds, WindowOptions};
use wsp_host_client::Env;

/// The state file a launch means, as the Electron app picks it: WSP_HOME names the home outright, else ~/.wsp. An
/// empty WSP_HOME names no home.
fn default_state() -> PathBuf {
    let home = std::env::var_os("WSP_HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".wsp"));
    home.join("state.json")
}

/// `--pane launcher` opens the right panel on no tab, the launcher, which a memory reading sets beside a pane's.
const LAUNCHER: &str = "launcher";

/// Takes `--pane <name>` out of the launch's words: a right panel pane to open once `--open`'s thread is on screen.
fn take_pane(words: impl Iterator<Item = String>) -> Result<(Vec<String>, Option<String>), String> {
    let (mut rest, mut pane) = (Vec::new(), None);
    let mut words = words.into_iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--pane" => pane = Some(words.next().ok_or("--pane takes a value")?),
            _ => rest.push(word),
        }
    }
    Ok((rest, pane))
}

/// Takes the wsp:// link out of the launch's words, where the system put one (`linkInArgv`).
fn take_link(words: Vec<String>) -> (Vec<String>, Option<String>) {
    let link = links::link_in_argv(&words).cloned();
    (words.into_iter().filter(|word| Some(word) != link.as_ref()).collect(), link)
}

fn main() {
    let (words, link) = take_link(std::env::args().skip(1).collect());
    let (words, pane) = take_pane(words.into_iter()).unwrap_or_else(|e| {
        eprintln!("desktop-gpui: {e}");
        std::process::exit(64);
    });
    let args = bench::parse(words.into_iter()).unwrap_or_else(|e| {
        eprintln!("desktop-gpui: {e}");
        std::process::exit(64);
    });
    if let Some(name) = &pane {
        if args.open.is_none() {
            eprintln!("desktop-gpui: --pane goes with --open");
            std::process::exit(64);
        }
        if name != LAUNCHER && !screens::screen(name).is_some_and(|e| matches!(e.place, Place::Panel(_))) {
            eprintln!("desktop-gpui: no right panel pane is named {name}");
            std::process::exit(64);
        }
    }
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
    // A launch that names what to open (a bench, a thread, a screen) or asks for the glass skips the lock and onboarding.
    let plain = args.bench.is_none() && args.open.is_none() && args.screen.is_none() && !args.gallery && !args.glass;
    let home = startup::home_of(&state);
    let lock = match plain.then(|| startup::claim_with(&home, link.as_deref())) {
        Some(Ok(Claim::Held(lock))) => {
            if let Err(e) = switch::own_path(std::env::var_os("APPIMAGE")).and_then(|exe| switch::write_path(&home, &exe)) {
                eprintln!("desktop-gpui: {} could not be written: {e}", switch::PATH_FILE);
            }
            Some(lock)
        }
        Some(Ok(Claim::Taken)) => std::process::exit(0),
        Some(Err(e)) => {
            eprintln!("desktop-gpui: the app's lock could not be taken: {e}");
            None
        }
        None => None,
    };
    let env: Env = std::env::vars().collect();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("a tokio runtime"));
    let gallery = args.gallery;
    let launch = Launch { bench: args.bench, open: args.open, glass: args.glass, screen: args.screen };
    let switched = switch::read(&home);
    frame::choose();
    let app = gpui_kit::application().with_assets(AppAssets);
    // A Mac hands over the link that launched the app before it is running, so the handler comes before `run`.
    let (urls, mut opened) = futures::channel::mpsc::unbounded::<String>();
    app.on_open_urls(move |handed| handed.into_iter().for_each(|url| _ = urls.unbounded_send(url)));
    let reopen = (state.clone(), env.clone(), runtime.clone());
    // Electron's activate: the dock's icon on a Mac app with no window opens one again.
    app.on_reopen(move |cx| {
        if cx.windows().is_empty() && !startup::opening(cx) {
            let (state, env, runtime) = reopen.clone();
            let main = reopen.clone();
            let window =
                Rc::new(move |cx: &mut App| open_main(main.0.clone(), main.1.clone(), main.2.clone(), Launch::default(), None, cx));
            startup::open_later(state, env, runtime, window, cx);
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
        screens::palette::init(cx);
        frame::init(cx);
        cx.set_global(switched);
        switch::init(&home, cx);
        if let Some(link) = &link {
            links::open(link, cx);
        }
        cx.spawn(async move |cx| {
            while let Some(url) = opened.next().await {
                cx.update(|cx| links::open(&url, cx));
            }
        })
        .detach();
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
        let reopen = {
            let (state, env, runtime) = (state.clone(), env.clone(), runtime.clone());
            Rc::new(move |cx: &mut App| open_main(state.clone(), env.clone(), runtime.clone(), Launch::default(), None, cx))
        };
        desktop_gpui::desktop::feed::start(state.clone(), env.clone(), runtime.clone(), reopen.clone(), cx);
        if let Some(lock) = lock {
            let (state, env, runtime, window) = (state.clone(), env.clone(), runtime.clone(), reopen.clone());
            let later = move |cx: &mut App| startup::open_later(state.clone(), env.clone(), runtime.clone(), window.clone(), cx);
            startup::follow_raises(lock, Rc::new(later), cx);
        }
        if plain {
            startup::open(state, env, runtime, reopen, cx);
        } else {
            open_main(state, env, runtime, launch, pane.clone(), cx);
        }
    });
}

/// The app's window, the shell in it, with `pane` opened in the right panel once the shell first shows a thread.
fn open_main(state: PathBuf, env: Env, runtime: Arc<tokio::runtime::Runtime>, launch: Launch, pane: Option<String>, cx: &mut App) {
    let options = WindowOptions { window_decorations: frame::decorations(), ..startup::window_options(cx) };
    cx.open_window(options, |window, cx| {
        let shell = cx.new(|cx| Shell::new(state, env, runtime, launch, window, cx));
        links::attach(window, &shell, cx);
        if let Some(pane) = pane {
            let mut pane = Some(pane);
            window
                .observe(&shell, cx, move |shell, window, cx| {
                    if matches!(shell.read(cx).route(), Route::Thread { .. }) {
                        match pane.take() {
                            Some(pane) if pane == LAUNCHER => {
                                let panel = shell.read(cx).panel().clone();
                                if !panel.read(cx).is_open() {
                                    panel.update(cx, |p, cx| p.toggle(window, cx));
                                }
                            }
                            Some(pane) => shell.update(cx, |s, cx| s.go(Route::Screen(pane.into()), window, cx)),
                            None => {}
                        }
                    }
                })
                .detach();
        }
        cx.new(|cx| Root::new(shell, window, cx))
    })
    .expect("failed to open the window");
    cx.activate(true);
}

#[cfg(test)]
mod tests {
    use super::{take_link, take_pane};

    #[test]
    fn takes_the_pane_and_leaves_the_rest() {
        let words = "--state /s.json --open t1 --pane slate".split(' ').map(str::to_owned);
        let (rest, pane) = take_pane(words).unwrap();
        assert_eq!(rest, ["--state", "/s.json", "--open", "t1"]);
        assert_eq!(pane.as_deref(), Some("slate"));
        assert!(take_pane(["--pane".to_owned()].into_iter()).is_err());
    }

    #[test]
    fn takes_the_link_and_leaves_the_rest() {
        let words = ["--state", "/s.json", "wsp://thread/t1"].map(str::to_owned).to_vec();
        let (rest, link) = take_link(words);
        assert_eq!(rest, ["--state", "/s.json"]);
        assert_eq!(link.as_deref(), Some("wsp://thread/t1"));
        assert_eq!(take_link(vec!["--glass".to_owned()]), (vec!["--glass".to_owned()], None));
        // Linux hands a link to a first launch as its only word, which must stay a plain launch that takes the lock.
        assert_eq!(take_link(vec!["wsp://thread/t1".to_owned()]), (vec![], Some("wsp://thread/t1".to_owned())));
    }
}
