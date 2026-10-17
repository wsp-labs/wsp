// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Usage drawn by GPUI's offscreen renderer off a host already serving `<state.json>`, in Paper and Graphite,
//! one state at a time, as `app/tests/usage/web-shots.mjs` photographs the web app: `<out>/<theme>/<state>.content.png`
//! at 1024 by 800 and `.full.png` at 1024 by 2400, both at 2x. `cargo run --example usage_shots -- <state.json> <out>
//! [--states a,b]` in apps/desktop-gpui.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::usage::model::{Range, Split};
use desktop_gpui::screens::usage::{frame, Tab, UsagePage};
use desktop_gpui::ui::{self, ActivePalette, Side, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{prelude::*, px, size, AnyWindowHandle, Bounds, Context, Entity, HeadlessAppContext, Window, WindowBounds, WindowOptions};
use wsp_host_client::HostClient;

const STATES: [(&str, Tab, Range, Split); 9] = [
    ("used-day", Tab::Used, Range::Day, Split::Agent),
    ("used-week", Tab::Used, Range::Week, Split::Agent),
    ("used-month", Tab::Used, Range::Month, Split::Agent),
    ("used-week-account", Tab::Used, Range::Week, Split::Account),
    ("used-week-computer", Tab::Used, Range::Week, Split::Computer),
    ("used-week-project", Tab::Used, Range::Week, Split::Project),
    ("used-week-model", Tab::Used, Range::Week, Split::Model),
    ("used-week-source", Tab::Used, Range::Week, Split::Source),
    ("limits", Tab::Limits, Range::Week, Split::Agent),
];

struct Shot {
    page: Entity<UsagePage>,
}

impl Render for Shot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.page.update(cx, |page, cx| page.tabs(cx).into_any_element());
        frame(Some(tabs), Some(self.page.clone().into_any_element()), None, cx).bg(cx.palette().background)
    }
}

/// Whether the page shows what it was last asked for.
fn landed(page: &UsagePage) -> bool {
    let used = page.used.value.as_ref().is_some_and(|u| Range::from(u.range) == page.range && Split::from(u.split) == page.split);
    let models = page.split != Split::Agent || page.models.value.is_some();
    !page.reading() && used && models && page.accounts.value.is_some()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(state), Some(out)) = (args.next().map(PathBuf::from), args.next().map(PathBuf::from)) else {
        eprintln!("usage: usage_shots <state.json> <out-dir> [--states a,b]");
        std::process::exit(2);
    };
    let only: Option<Vec<String>> = match (args.next().as_deref(), args.next()) {
        (Some("--states"), Some(list)) => Some(list.split(',').map(str::to_owned).collect()),
        _ => None,
    };
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let env = std::env::vars().collect();
    let host = Arc::new(runtime.block_on(HostClient::connect(&state, &env)).unwrap_or_else(|f| panic!("{}", f.message)));

    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    for theme in ["paper", "graphite"] {
        cx.update(|cx| ui::theme::set_theme(theme, cx));
        std::fs::create_dir_all(out.join(theme)).unwrap();
        for (height, suffix) in [(800., "content"), (2400., "full")] {
            let (handle, page) = open(&mut cx, host.clone(), runtime.clone(), height);
            for (name, tab, range, split) in STATES {
                if only.as_ref().is_some_and(|only| !only.iter().any(|o| o == name)) {
                    continue;
                }
                cx.update(|cx| {
                    page.update(cx, |page, cx| {
                        page.pick_tab(tab, cx);
                        page.pick_range(range, cx);
                        page.pick_split(split, cx);
                    })
                });
                let until = Instant::now() + Duration::from_secs(30);
                while !cx.update(|cx| landed(page.read(cx))) {
                    assert!(Instant::now() < until, "{theme}/{name} never landed");
                    cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
                    cx.run_until_parked();
                    std::thread::sleep(Duration::from_millis(20));
                }
                for _ in 0..3 {
                    cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
                    cx.run_until_parked();
                }
                let image = cx.capture_screenshot(handle).unwrap();
                let path = out.join(theme).join(format!("{name}.{suffix}.png"));
                image.save(&path).unwrap();
                println!("usage_shots: {}", path.display());
            }
            cx.update_window(handle, |_, window, _| window.remove_window()).unwrap();
        }
    }
}

fn open(
    cx: &mut HeadlessAppContext,
    host: Arc<HostClient>,
    runtime: Arc<tokio::runtime::Runtime>,
    height: f32,
) -> (AnyWindowHandle, Entity<UsagePage>) {
    let mut page = None;
    let (handle, _): (AnyWindowHandle, _) = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(1024.), px(height)) })),
                focus: false,
                show: false,
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                let usage = cx.new(|cx| UsagePage::new(host, runtime, window, cx));
                page = Some(usage.clone());
                let shot = cx.new(|_| Shot { page: usage });
                cx.new(|cx| Root::new(shot, window, cx))
            })
        })
        .unwrap();
    (handle, page.unwrap())
}
