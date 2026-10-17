// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar alone in the app's window, framed as the shell frames its column, for captures and the scroll bench.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::{h_flex, v_flex, Root};
use gpui_kit::{
    div, prelude::*, px, size, AnyWindowHandle, App, AppContext, Bounds, Context, Entity, Task, TitlebarOptions, Window, WindowBounds,
    WindowOptions,
};
use tokio::runtime::Runtime;
use wsp_host_client::{Env, Failure, HostClient};

use super::{chrome, Link, Sidebar, WIDTH};
use crate::bench::PX_PER_S;
use crate::ui::{self, theme, ActivePalette, Side};

struct Alone {
    sidebar: Entity<Sidebar>,
    _connect: Task<()>,
}

impl Render for Alone {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        // The column as the shell frames it: its width and edge, the sidebar's surface and the frame row over it.
        let column = v_flex()
            .w(WIDTH)
            .h_full()
            .flex_none()
            .bg(p.sidebar)
            .border_r_1()
            .border_color(p.sidebar_border)
            .child(theme::within(p.sidebar_scope(), cx, |cx| chrome::header(Box::new(|_, _, _| {}), cx)))
            .child(div().flex_1().min_h_0().child(self.sidebar.clone()));
        h_flex().size_full().bg(p.background).child(column).child(div().flex_1())
    }
}

/// Opens the window at the Electron app's size with the sidebar drawn from the host serving `state`, in `theme_id`
/// (the side's default where none is named), with `open` marked as the open thread. With `bench`, once the list is
/// drawn and the window has focus, scrolls the list to its end at the bench's rate, writes the frames there and quits.
pub fn open(
    state: PathBuf,
    env: Env,
    runtime: Arc<Runtime>,
    theme_id: Option<String>,
    open: Option<String>,
    bench: Option<PathBuf>,
    cx: &mut App,
) {
    let side = match theme_id.as_deref().and_then(theme::theme_by_id) {
        Some(entry) => entry.side,
        None => Side::Light,
    };
    ui::init(side, cx);
    if let Some(id) = &theme_id {
        if theme::theme_by_id(id).is_none() {
            eprintln!("desktop-gpui: wsp has no theme {id}");
            std::process::exit(64);
        }
        theme::set_theme(id, cx);
    }
    let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some(crate::TITLE.into()), ..Default::default() }),
            ..Default::default()
        },
        |window, cx| {
            let sidebar = cx.new(Sidebar::new);
            let connecting = runtime.spawn(async move { HostClient::connect(&state, &env).await });
            let weak = sidebar.downgrade();
            let connect = cx.spawn(async move |cx| {
                let connected = connecting.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
                _ = weak.update(cx, |s, cx| match connected {
                    Ok(host) => {
                        s.attach(Link { host: Arc::new(host), runtime }, cx);
                        if let Some(thread) = &open {
                            s.selected = Some(thread.clone());
                            s.folds.open = s.selected.clone();
                        }
                    }
                    Err(failure) => {
                        eprintln!("desktop-gpui: {}", failure.message);
                        s.set_note(failure.message, cx);
                    }
                });
            });
            if let Some(out) = bench {
                scroll_bench(sidebar.clone(), window.window_handle(), out, cx);
            }
            let alone = cx.new(|_| Alone { sidebar, _connect: connect });
            cx.new(|cx| Root::new(alone, window, cx))
        },
    )
    .expect("failed to open the window");
}

/// How long the bench lets a drawn list stand before it scrolls, so the first measured frame is not the first drawn.
const SETTLE: Duration = Duration::from_secs(2);

fn scroll_bench(sidebar: Entity<Sidebar>, window: AnyWindowHandle, out: PathBuf, cx: &mut App) {
    cx.spawn(async move |cx| {
        let began = Instant::now();
        let mut drawn: Option<Instant> = None;
        let frames = Rc::new(RefCell::new(Vec::new()));
        let mut start = None;
        loop {
            cx.background_executor().timer(Duration::from_millis(50)).await;
            if began.elapsed() > Duration::from_secs(120) {
                eprintln!("desktop-gpui: the sidebar bench did not finish in 120 s");
                std::process::exit(2);
            }
            let step = cx.update_window(window, |_, window, cx| {
                window.activate_window();
                let rows = sidebar.read(cx).lines().len();
                (window.is_window_active(), rows, sidebar.read(cx).driven_to_end())
            });
            let Ok((active, rows, done)) = step else { return };
            if done {
                let start: Instant = start.unwrap_or(began);
                let ms: Vec<f64> = frames.borrow().iter().map(|t: &Instant| t.duration_since(start).as_secs_f64() * 1000.).collect();
                let json = serde_json::json!({ "app": "gpui", "screen": "sidebar", "rows": rows, "scroll": { "px_per_s": PX_PER_S, "frame_ms": ms } });
                if let Err(e) = std::fs::write(&out, format!("{json}\n")) {
                    eprintln!("desktop-gpui: {}: {e}", out.display());
                    std::process::exit(1);
                }
                cx.update(|cx| cx.quit());
                return;
            }
            if start.is_some() || rows == 0 || !active {
                continue;
            }
            let at = *drawn.get_or_insert_with(Instant::now);
            if at.elapsed() >= SETTLE {
                let now = Instant::now();
                start = Some(now);
                let frames = frames.clone();
                sidebar.update(cx, |s, cx| s.drive(PX_PER_S, now, frames, cx));
            }
        }
    })
    .detach();
}
