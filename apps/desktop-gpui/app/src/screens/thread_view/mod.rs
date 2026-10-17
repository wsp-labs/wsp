// SPDX-License-Identifier: AGPL-3.0-only
//! The thread view's pieces, and the thread view alone in a window for captures beside the web app.

pub mod header;
pub mod markdown;
pub mod minimap;
pub mod rows;

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::{h_flex, Root};
use gpui_kit::{
    div, prelude::*, px, size, App, AppContext, Bounds, Context, Entity, Pixels, Task, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use tokio::runtime::Runtime;
use wsp_host_client::{Env, Failure, HostClient};

use crate::sidebar::ThreadItem;
use crate::thread_view::ThreadView;
use crate::transcript::RowKind;
use crate::ui::{self, ActivePalette};

/// The web app's `SIDEBAR_DEFAULT_WIDTH`, left empty so the thread view stands where the web app draws it.
pub const SIDEBAR: Pixels = px(256.);

/// How a capture stands the screen: its theme, its clock, and what it opens and scrolls to (`bench::Args`).
#[derive(Default)]
pub struct Capture {
    pub theme: Option<String>,
    pub unfold: bool,
    pub now: Option<f64>,
    pub reveal: Option<(String, f32)>,
}

/// `desktop-gpui --screen thread_view`: the thread view of `thread` in a 1280 x 800 window, following it on the host
/// `state` names, stood as `capture` says.
pub fn open_alone(
    state: PathBuf,
    vars: Env,
    runtime: Arc<Runtime>,
    thread: Option<String>,
    capture: Capture,
    cx: &mut App,
) -> Result<(), String> {
    let thread = thread.ok_or("--screen thread_view takes --open <thread id>")?;
    let theme = capture.theme.clone().unwrap_or_else(|| ui::theme::LIGHT.to_owned());
    let entry = ui::theme::theme_by_id(&theme).ok_or(format!("wsp has no theme {theme}"))?;
    ui::init(entry.side, cx);
    ui::theme::set_theme(&theme, cx);
    let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some(crate::TITLE.into()), ..Default::default() }),
            ..Default::default()
        },
        |window, cx| {
            let alone = cx.new(|cx| Alone::new(state, vars, runtime, thread, capture, cx));
            cx.new(|cx| Root::new(alone, window, cx))
        },
    )
    .map_err(|e| e.to_string())?;
    cx.activate(true);
    Ok(())
}

struct Alone {
    view: Entity<ThreadView>,
    capture: Capture,
    _connect: Task<()>,
}

impl Alone {
    fn new(state: PathBuf, vars: Env, runtime: Arc<Runtime>, thread: String, capture: Capture, cx: &mut Context<Self>) -> Self {
        let view = cx.new(ThreadView::new);
        if let Some(now) = capture.now {
            view.update(cx, |v, cx| v.freeze_clock(now, cx));
        }
        let connecting = runtime.spawn(async move {
            let host = HostClient::connect(&state, &vars).await?;
            let threads = host.threads().await?;
            Ok::<_, Failure>((host, threads))
        });
        let connect = cx.spawn(async move |this, cx| {
            let connected = connecting.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
            _ = this.update(cx, |this, cx| {
                let (host, threads) = match connected {
                    Ok(connected) => connected,
                    Err(failure) => return this.view.update(cx, |v, cx| v.fail(failure.message, cx)),
                };
                let Some(item) = threads.iter().filter_map(ThreadItem::of).find(|i| i.thread_id == thread) else {
                    return this.view.update(cx, |v, cx| v.fail(format!("the host lists no thread {thread}"), cx));
                };
                this.view.update(cx, |v, cx| v.follow(Arc::new(host), &runtime, &item, false, cx));
            });
        });
        Alone { view, capture, _connect: connect }
    }
}

impl Render for Alone {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.view.read(cx).rows().is_empty() {
            if std::mem::take(&mut self.capture.unfold) {
                self.view.update(cx, unfold);
            }
            if let Some((text, y)) = self.capture.reveal.take() {
                self.view.update(cx, |v, cx| {
                    let row = v.rows().iter().position(|r| matches!(r.kind, RowKind::Person { .. }) && r.text.contains(&text));
                    if let Some(row) = row {
                        v.reveal(row, px(y), cx);
                    }
                });
            }
        }
        let p = cx.palette();
        h_flex()
            .size_full()
            .bg(p.background)
            .text_color(p.foreground)
            .child(div().flex_none().w(SIDEBAR).h_full().bg(p.sidebar))
            .child(div().flex_1().h_full().child(self.view.clone()))
    }
}

/// What the captures click in the web app: every turn's fold, the first tool group, and in an open group every failed
/// call and every edit; then, as they do, the transcript from its top.
pub fn unfold(view: &mut ThreadView, cx: &mut Context<ThreadView>) {
    let folds: Vec<String> =
        view.rows().iter().filter(|r| matches!(r.kind, RowKind::Fold { open: false })).filter_map(|r| r.turn.clone()).collect();
    for turn in folds {
        view.toggle(&turn, cx);
    }
    let group = view.rows().iter().find_map(|r| match &r.kind {
        RowKind::Group { group, open: false, .. } => Some(group.clone()),
        _ => None,
    });
    if let Some(group) = group {
        view.toggle(&group, cx);
    }
    let failed: Vec<String> = view
        .rows()
        .iter()
        .flat_map(|r| match &r.kind {
            RowKind::Calls { calls, .. } => {
                calls.iter().filter(|c| (c.failed || c.patch.is_some()) && !c.open).map(|c| c.key.clone()).collect()
            }
            _ => Vec::new(),
        })
        .collect();
    for call in failed {
        view.toggle(&call, cx);
    }
    view.scroll_to_top(cx);
}
