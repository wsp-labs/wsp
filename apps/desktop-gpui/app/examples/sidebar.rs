// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar alone in the app's window, on a host's state file, for captures and its scroll bench:
//! `cargo run --example sidebar -- --state <state.json> [--theme <id>] [--open <thread> | --bench <out.json>]`.

use std::path::PathBuf;
use std::sync::Arc;

use desktop_gpui::screens::sidebar::alone;
use desktop_gpui::ui::UiAssets;
use gpui_kit::App;

struct Args {
    state: PathBuf,
    theme: Option<String>,
    open: Option<String>,
    bench: Option<PathBuf>,
}

fn parse(mut words: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut state, mut theme, mut open, mut bench) = (None, None, None, None);
    while let Some(flag) = words.next() {
        let mut value = || words.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--state" => state = Some(PathBuf::from(value()?)),
            "--theme" => theme = Some(value()?),
            "--open" => open = Some(value()?),
            "--bench" => bench = Some(PathBuf::from(value()?)),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if open.is_some() && bench.is_some() {
        return Err("--open and --bench do not go together".to_owned());
    }
    Ok(Args { state: state.ok_or("--state names the host's state file")?, theme, open, bench })
}

fn main() {
    let args = parse(std::env::args().skip(1)).unwrap_or_else(|e| {
        eprintln!("sidebar: {e}");
        std::process::exit(64);
    });
    let env = std::env::vars().collect();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("a tokio runtime"));
    gpui_kit::application().with_assets(UiAssets).run(move |cx: &mut App| {
        gpui_kit::init(cx);
        alone::open(args.state, env, runtime, args.theme, args.open, args.bench, cx);
        cx.activate(true);
    });
}
