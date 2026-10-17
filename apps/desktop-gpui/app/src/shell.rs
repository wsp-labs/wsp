// SPDX-License-Identifier: AGPL-3.0-only
//! The window's content: the sidebar beside the open thread, the host connection behind both, and bench mode's run.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::component::{h_flex, ActiveTheme};
use gpui_kit::{prelude::*, App, Context, Entity, StyleRefinement, Subscription, Task, Window};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::SessionEvent;
use wsp_host_client::{Env, Failure, HostClient, Live};

use crate::bench::{self, PX_PER_S};
use crate::sidebar::{Selected, Sidebar, ThreadItem, WIDTH};
use crate::thread_view::ThreadView;

/// A bench that has not finished by then is stuck, and says so rather than hang the run.
const BENCH_LIMIT: Duration = Duration::from_secs(300);
/// GPUI caps a window without focus at 30 fps, so a bench measures only a focused window and stops when it has none.
const FOCUS_WAIT: Duration = Duration::from_secs(10);
const UNFOCUSED: i32 = 3;

enum Arrival {
    History(Vec<SessionEvent>),
    Event(Box<SessionEvent>),
    Failed(String),
}

/// One thread followed live; dropping it stops the host task and the receiver.
struct Following {
    task: tokio::task::AbortHandle,
    _receive: Task<()>,
}

impl Drop for Following {
    fn drop(&mut self) {
        self.task.abort();
    }
}

type Presented = Rc<RefCell<Option<Instant>>>;

enum Phase {
    Opening,
    Loading(Presented),
    Scrolling { start: Instant, frames: Rc<RefCell<Vec<Instant>>> },
    Switching { ix: usize, start: Instant, presented: Presented },
}

struct BenchRun {
    args: bench::Bench,
    began: Instant,
    phase: Phase,
    rows: usize,
    frame_ms: Vec<f64>,
    switch: Vec<(String, f64)>,
}

pub struct Shell {
    sidebar: Entity<Sidebar>,
    thread_view: Entity<ThreadView>,
    host: Option<Arc<HostClient>>,
    runtime: Arc<Runtime>,
    following: Option<Following>,
    bench: Option<BenchRun>,
    _connect: Task<()>,
    _selected: Subscription,
}

impl Shell {
    pub fn new(
        state: PathBuf,
        env: Env,
        runtime: Arc<Runtime>,
        bench: Option<bench::Bench>,
        open: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let sidebar = cx.new(Sidebar::new);
        let thread_view = cx.new(ThreadView::new);
        let selected = cx.subscribe(&sidebar, |this, _, Selected(item), cx| this.open(item.clone(), cx));
        let connecting = runtime.spawn(async move {
            let host = HostClient::connect(&state, &env).await?;
            let threads = host.threads().await?;
            Ok::<_, Failure>((host, threads))
        });
        let connect = cx.spawn(async move |this, cx| {
            let connected = connecting.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
            _ = this.update(cx, |this, cx| match connected {
                Ok((host, threads)) => {
                    this.host = Some(Arc::new(host));
                    this.sidebar.update(cx, |s, cx| s.set_items(threads.iter().filter_map(ThreadItem::of).collect(), cx));
                    if let Some(thread) = &open {
                        let Some(item) = this.sidebar.update(cx, |s, cx| s.mark(thread, cx)) else {
                            eprintln!("desktop-gpui: the host lists no thread {thread}");
                            std::process::exit(1);
                        };
                        this.open(item, cx);
                    }
                }
                Err(failure) => {
                    eprintln!("desktop-gpui: {}", failure.message);
                    this.sidebar.update(cx, |s, cx| s.set_note(failure.message, cx));
                    if this.bench.is_some() {
                        std::process::exit(1);
                    }
                }
            });
        });
        let bench = bench.map(|args| BenchRun {
            args,
            began: Instant::now(),
            phase: Phase::Opening,
            rows: 0,
            frame_ms: Vec::new(),
            switch: Vec::new(),
        });
        Shell { sidebar, thread_view, host: None, runtime, following: None, bench, _connect: connect, _selected: selected }
    }

    /// Shows `item`'s transcript, read whole, then follows it live.
    fn open(&mut self, item: ThreadItem, cx: &mut Context<Self>) {
        self.following = None;
        self.thread_view.update(cx, |v, cx| v.show(&item.thread_id, cx));
        let Some(host) = self.host.clone() else { return };
        let (tx, mut rx) = mpsc::unbounded();
        let task = self.runtime.spawn(async move {
            let followed: Result<(), Failure> = async {
                let mut thread = host.open(&item.workspace_id, &item.thread_id).await?;
                _ = tx.unbounded_send(Arrival::History(std::mem::take(&mut thread.history)));
                while let Some(next) = thread.next().await {
                    if let Live::Event(event) = next? {
                        _ = tx.unbounded_send(Arrival::Event(event));
                    }
                }
                Ok(())
            }
            .await;
            if let Err(failure) = followed {
                _ = tx.unbounded_send(Arrival::Failed(failure.message));
            }
        });
        let receive = cx.spawn(async move |this, cx| {
            while let Some(arrival) = rx.next().await {
                if this.update(cx, |this, cx| this.arrive(arrival, cx)).is_err() {
                    break;
                }
            }
        });
        self.following = Some(Following { task: task.abort_handle(), _receive: receive });
    }

    fn arrive(&mut self, arrival: Arrival, cx: &mut Context<Self>) {
        let benching = self.bench.is_some();
        self.thread_view.update(cx, |v, cx| match arrival {
            Arrival::History(events) => {
                v.load(events, cx);
                v.scroll_to_end(cx);
            }
            Arrival::Event(event) => v.push(*event, cx),
            Arrival::Failed(failure) => {
                if benching {
                    eprintln!("desktop-gpui: {failure}");
                    std::process::exit(1);
                }
                v.fail(failure, cx)
            }
        });
    }

    /// One step of bench mode, run on every frame until it writes its result and quits.
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut run) = self.bench.take() else { return };
        if run.began.elapsed() > BENCH_LIMIT {
            eprintln!("desktop-gpui: the bench did not finish in {} s", BENCH_LIMIT.as_secs());
            std::process::exit(2);
        }
        let measuring = matches!(run.phase, Phase::Scrolling { .. } | Phase::Switching { .. });
        if !window.is_window_active() && (measuring || run.began.elapsed() > FOCUS_WAIT) {
            eprintln!("desktop-gpui: the bench window has no focus, and GPUI caps a window without focus at 30 fps");
            std::process::exit(UNFOCUSED);
        }
        match &run.phase {
            Phase::Opening => {
                if self.host.is_some() {
                    cx.activate(true);
                    window.activate_window();
                    run.phase = Phase::Loading(self.bench_open(&run.args.thread, cx));
                }
            }
            Phase::Loading(presented) => {
                if presented.borrow().is_some() && window.is_window_active() {
                    let (start, frames) = (Instant::now(), Rc::new(RefCell::new(Vec::new())));
                    run.rows = self.thread_view.read(cx).rows().len();
                    self.thread_view.update(cx, |v, cx| v.drive(PX_PER_S, start, frames.clone(), cx));
                    run.phase = Phase::Scrolling { start, frames };
                }
            }
            Phase::Scrolling { start, frames } => {
                if self.thread_view.read(cx).driven_to_end() {
                    run.frame_ms = frames.borrow().iter().map(|t| t.duration_since(*start).as_secs_f64() * 1000.).collect();
                    run.phase = self.switch_to(0, &run.args, cx);
                }
            }
            Phase::Switching { ix, start, presented } => {
                let shown = *presented.borrow();
                if let Some(shown) = shown {
                    run.switch.push((run.args.switch[*ix].clone(), shown.duration_since(*start).as_secs_f64() * 1000.));
                    run.phase = self.switch_to(ix + 1, &run.args, cx);
                }
            }
        }
        if matches!(run.phase, Phase::Switching { ix, .. } if ix >= run.args.switch.len()) {
            let json = bench::result(run.rows, &run.frame_ms, &run.switch);
            if let Err(e) = std::fs::write(&run.args.out, format!("{json}\n")) {
                eprintln!("desktop-gpui: {}: {e}", run.args.out.display());
                std::process::exit(1);
            }
            cx.quit();
            return;
        }
        self.bench = Some(run);
        window.request_animation_frame();
    }

    fn switch_to(&mut self, ix: usize, args: &bench::Bench, cx: &mut Context<Self>) -> Phase {
        let start = Instant::now();
        let presented = args.switch.get(ix).map(|thread| self.bench_open(thread, cx)).unwrap_or_default();
        Phase::Switching { ix, start, presented }
    }

    /// Opens a thread as a click does and watches for the first presented frame that shows it.
    fn bench_open(&mut self, thread: &str, cx: &mut Context<Self>) -> Presented {
        let Some(item) = self.sidebar.update(cx, |s, cx| s.mark(thread, cx)) else {
            eprintln!("desktop-gpui: the host lists no thread {thread}");
            std::process::exit(1);
        };
        self.open(item, cx);
        let presented = Presented::default();
        self.thread_view.update(cx, |v, cx| v.watch_presented(thread, presented.clone(), cx));
        presented
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tick(window, cx);
        layout(&self.sidebar, &self.thread_view, cx)
    }
}

/// The sidebar beside the thread view, which a test can render without a host behind them.
pub fn layout(sidebar: &Entity<Sidebar>, thread_view: &Entity<ThreadView>, cx: &App) -> impl IntoElement {
    h_flex()
        .size_full()
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .child(sidebar.clone().cached(StyleRefinement::default().w(WIDTH).h_full()))
        .child(thread_view.clone().cached(StyleRefinement::default().flex_1().h_full()))
}
