// SPDX-License-Identifier: AGPL-3.0-only
//! Bench mode's run: its phases, one step a frame, and the result it writes.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{prelude::*, App, Context, Entity, Subscription, Task, Window};

use super::Shell;
use crate::bench::{self, PX_PER_S};
use crate::screens::{self, Route};
use crate::thread_view::ThreadView;

/// A bench that has not finished by then is stuck, and says so rather than hang the run.
pub(super) const BENCH_LIMIT: Duration = Duration::from_secs(300);
/// GPUI caps a window without focus at 30 fps, so a bench measures only a focused window and stops when it has none.
const FOCUS_WAIT: Duration = Duration::from_secs(10);
const UNFOCUSED: i32 = 3;
/// How long a bench holds still after its scroll, so the run can read the app's memory after a full scroll.
const HOLD: Duration = Duration::from_secs(10);
/// The timed switches after the bench's own, among its switch threads in turn, each after the wait the Electron bench
/// keeps between its switches.
const TIMED_SWITCHES: usize = 30;
const SWITCH_REST: Duration = Duration::from_secs(1);

type Presented = Rc<RefCell<Option<Instant>>>;

pub(super) enum Phase {
    Opening,
    Loading(Presented),
    /// The thread reads its older pages until it holds all of them, as the Electron bench pages back before it scrolls.
    Paging,
    Scrolling {
        start: Instant,
        frames: Rc<RefCell<Vec<Instant>>>,
    },
    Holding {
        until: Instant,
        _wake: Task<()>,
    },
    Switching {
        ix: usize,
        start: Instant,
        presented: Presented,
    },
    /// The terminal bench: the drawer's shell drawing its prompt, then the line's output, each until 1 s passes quiet.
    Prompting {
        output: Times,
        _heard: Option<Subscription>,
        _step: Task<()>,
    },
    Streaming {
        sent: Instant,
        frames: Times,
        output: Times,
        _heard: Subscription,
        _step: Task<()>,
    },
    Resting {
        n: usize,
        until: Instant,
        _wake: Task<()>,
    },
    Timing {
        n: usize,
        start: Instant,
        presented: Presented,
    },
}

/// When the terminal bench's frames were painted, or its shell's output batches arrived.
type Times = Rc<RefCell<Vec<Instant>>>;

/// How long a terminal bench waits with no output before it calls the shell's output done.
const QUIET: Duration = Duration::from_secs(1);
/// A shell that has not gone quiet this long into the run is busy with a program an earlier run left in it.
const PROMPT_LIMIT: Duration = Duration::from_secs(30);

pub(super) struct BenchRun {
    pub(super) args: bench::Bench,
    pub(super) began: Instant,
    pub(super) phase: Phase,
    pub(super) rows: usize,
    pub(super) frame_ms: Vec<f64>,
    pub(super) switch: Vec<(String, f64)>,
    pub(super) timed: Vec<(String, f64)>,
}

impl Shell {
    /// One step of bench mode, run on every frame until it writes its result and quits.
    pub(super) fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut run) = self.bench.take() else { return };
        let measuring =
            matches!(run.phase, Phase::Scrolling { .. } | Phase::Switching { .. } | Phase::Streaming { .. } | Phase::Timing { .. });
        if !window.is_window_active() && (measuring || run.began.elapsed() > FOCUS_WAIT) {
            eprintln!("desktop-gpui: the bench window has no focus, and GPUI caps a window without focus at 30 fps");
            std::process::exit(UNFOCUSED);
        }
        match &run.phase {
            Phase::Opening => {
                if self.host.is_some() {
                    cx.activate(true);
                    window.activate_window();
                    run.phase = Phase::Loading(self.bench_open(&run.args.thread, window, cx));
                }
            }
            Phase::Loading(presented) if run.args.terminal.is_some() => {
                if presented.borrow().is_some() && window.is_window_active() {
                    if !screens::terminal::drawer_open(&self.route, cx) {
                        screens::terminal::toggle_drawer(&self.route, cx);
                    }
                    eprintln!("bench: drawer open");
                    // A step on a timer, not on every frame, so the bench's own redraws do not count as the terminal's.
                    let step = cx.spawn_in(window, async move |this, cx| loop {
                        cx.background_executor().timer(Duration::from_millis(50)).await;
                        if this.update_in(cx, |this, window, cx| this.tick(window, cx)).is_err() {
                            return;
                        }
                    });
                    run.phase = Phase::Prompting { output: Rc::default(), _heard: None, _step: step };
                }
            }
            Phase::Prompting { output, _heard: None, .. } => {
                let output = output.clone();
                if let Some(heard) = self.bench_hear(output.clone(), cx) {
                    eprintln!("bench: shell found");
                    if bench_session(cx).is_some_and(|s| s.read(cx).exited().is_some()) {
                        eprintln!("desktop-gpui: the drawer's shell has ended, so the line would reach nothing; bench on a fresh fixture");
                        std::process::exit(1);
                    }
                    let Phase::Prompting { _step, .. } = std::mem::replace(&mut run.phase, Phase::Opening) else { unreachable!() };
                    run.phase = Phase::Prompting { output, _heard: Some(heard), _step };
                }
            }
            Phase::Prompting { output, .. } => {
                if run.began.elapsed() > PROMPT_LIMIT {
                    eprintln!("desktop-gpui: the drawer's shell never went quiet; an earlier run may have left a program drawing in it");
                    std::process::exit(1);
                }
                if output.borrow().last().is_some_and(|t| t.elapsed() > QUIET) {
                    let Phase::Prompting { output, _heard: Some(_heard), _step } = std::mem::replace(&mut run.phase, Phase::Opening) else {
                        unreachable!()
                    };
                    let frames = Rc::new(RefCell::new(Vec::new()));
                    cx.set_global(bench::Frames(Some(frames.clone())));
                    output.borrow_mut().clear();
                    let line = format!("{}\r", run.args.terminal.as_deref().unwrap_or_default());
                    if let Some(session) = bench_session(cx) {
                        session.update(cx, |s, cx| s.send(line.as_bytes(), cx));
                    }
                    eprintln!("bench: line sent");
                    run.phase = Phase::Streaming { sent: Instant::now(), frames, output, _heard, _step };
                }
            }
            Phase::Streaming { sent, frames, output, .. } => {
                if output.borrow().last().is_some_and(|t| t.elapsed() > QUIET) {
                    let ms = |t: &Instant| t.duration_since(*sent).as_secs_f64() * 1000.;
                    let frame_ms: Vec<f64> = frames.borrow().iter().map(ms).collect();
                    if frame_ms.is_empty() {
                        eprintln!("desktop-gpui: the line's output drew no frame; the shell it went to is not the one on screen");
                        std::process::exit(1);
                    }
                    let output_ms: Vec<f64> = output.borrow().iter().map(ms).collect();
                    let json = bench::terminal_result(run.args.terminal.as_deref().unwrap_or_default(), &frame_ms, &output_ms);
                    if let Err(e) = std::fs::write(&run.args.out, format!("{json}\n")) {
                        eprintln!("desktop-gpui: {}: {e}", run.args.out.display());
                        std::process::exit(1);
                    }
                    cx.quit();
                    return;
                }
            }
            Phase::Loading(presented) => {
                if presented.borrow().is_some() {
                    self.bench_view(cx).update(cx, |v, cx| v.read_whole(cx));
                    run.phase = Phase::Paging;
                }
            }
            Phase::Paging => {
                if self.bench_view(cx).read(cx).complete() && window.is_window_active() {
                    let (start, frames) = (Instant::now(), Rc::new(RefCell::new(Vec::new())));
                    let view = self.bench_view(cx);
                    run.rows = view.read(cx).rows().len();
                    view.update(cx, |v, cx| v.drive(PX_PER_S, start, frames.clone(), cx));
                    run.phase = Phase::Scrolling { start, frames };
                }
            }
            Phase::Scrolling { start, frames } => {
                if self.bench_view(cx).read(cx).driven_to_end() {
                    run.frame_ms = frames.borrow().iter().map(|t| t.duration_since(*start).as_secs_f64() * 1000.).collect();
                    eprintln!("bench: scrolled");
                    let wake = cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(HOLD).await;
                        _ = this.update(cx, |_, cx| cx.notify());
                    });
                    run.phase = Phase::Holding { until: Instant::now() + HOLD, _wake: wake };
                }
            }
            Phase::Holding { until, .. } => {
                if Instant::now() >= *until {
                    run.phase = self.switch_to(0, &run.args, window, cx);
                }
            }
            Phase::Switching { ix, start, presented } => {
                let shown = *presented.borrow();
                if let Some(shown) = shown {
                    run.switch.push((run.args.switch[*ix].clone(), shown.duration_since(*start).as_secs_f64() * 1000.));
                    run.phase = self.switch_to(ix + 1, &run.args, window, cx);
                    if matches!(run.phase, Phase::Switching { ix, .. } if ix >= run.args.switch.len()) && !run.args.switch.is_empty() {
                        run.phase = Self::rest(0, cx);
                    }
                }
            }
            Phase::Resting { n, until, .. } => {
                if Instant::now() >= *until {
                    let thread = run.args.switch[n % run.args.switch.len()].clone();
                    let start = Instant::now();
                    let presented = self.bench_open(&thread, window, cx);
                    run.phase = Phase::Timing { n: *n, start, presented };
                }
            }
            Phase::Timing { n, start, presented } => {
                let shown = *presented.borrow();
                if let Some(shown) = shown {
                    let thread = run.args.switch[n % run.args.switch.len()].clone();
                    run.timed.push((thread, shown.duration_since(*start).as_secs_f64() * 1000.));
                    run.phase = Self::rest(n + 1, cx);
                }
            }
        }
        let timed = matches!(run.phase, Phase::Resting { n, .. } if n >= TIMED_SWITCHES);
        if timed || (run.args.switch.is_empty() && matches!(run.phase, Phase::Switching { .. })) {
            let json = bench::result(run.rows, &run.frame_ms, &run.switch, &run.timed);
            if let Err(e) = std::fs::write(&run.args.out, format!("{json}\n")) {
                eprintln!("desktop-gpui: {}: {e}", run.args.out.display());
                std::process::exit(1);
            }
            cx.quit();
            return;
        }
        let holding =
            matches!(run.phase, Phase::Holding { .. } | Phase::Prompting { .. } | Phase::Streaming { .. } | Phase::Resting { .. });
        self.bench = Some(run);
        if !holding {
            window.request_animation_frame();
        }
    }

    /// Where a bench past its limit waits, so a run on another machine says it.
    pub(super) fn bench_stuck(&self, cx: &App) -> String {
        let view = self.bench_view(cx);
        let list = view.read(cx).list();
        match self.bench.as_ref().map(|run| &run.phase) {
            Some(Phase::Opening) => "waiting for the host or a first frame".to_owned(),
            Some(Phase::Loading(_)) => "waiting for the thread's first frame and the window's focus".to_owned(),
            Some(Phase::Scrolling { .. }) => format!(
                "scrolling at {:?} of {:?}, the end check {:?}",
                -list.scroll_px_offset_for_scrollbar().y,
                list.max_offset_for_scrollbar().y,
                list.is_scrolled_to_end()
            ),
            Some(Phase::Holding { .. }) => "holding after the scroll".to_owned(),
            Some(Phase::Paging) => "reading the thread's older pages".to_owned(),
            Some(Phase::Switching { ix, .. }) => format!("waiting for switch {} to draw", ix + 1),
            Some(Phase::Prompting { output, .. }) => {
                format!("waiting for the shell's prompt to go quiet, {} batches heard", output.borrow().len())
            }
            Some(Phase::Streaming { output, .. }) => {
                format!("waiting for the line's output to go quiet, {} batches heard", output.borrow().len())
            }
            Some(Phase::Resting { n, .. }) => format!("resting before timed switch {}", n + 1),
            Some(Phase::Timing { n, .. }) => format!("waiting for timed switch {} to draw", n + 1),
            None => "past its last step".to_owned(),
        }
    }

    /// The wait before timed switch `n`, which wakes the window when it is over.
    fn rest(n: usize, cx: &mut Context<Self>) -> Phase {
        let wake = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SWITCH_REST).await;
            _ = this.update(cx, |_, cx| cx.notify());
        });
        Phase::Resting { n, until: Instant::now() + SWITCH_REST, _wake: wake }
    }

    fn switch_to(&mut self, ix: usize, args: &bench::Bench, window: &mut Window, cx: &mut Context<Self>) -> Phase {
        let start = Instant::now();
        let presented = args.switch.get(ix).map(|thread| self.bench_open(thread, window, cx)).unwrap_or_default();
        Phase::Switching { ix, start, presented }
    }

    /// Notes each time the drawer's shell draws output, once the drawer holds one; a prompt already drawn counts.
    fn bench_hear(&mut self, output: Times, cx: &mut Context<Self>) -> Option<Subscription> {
        let session = bench_session(cx)?;
        let mut frame = screens::terminal::engine::Frame::default();
        session.update(cx, |s, _| s.engine_mut().frame(&mut frame));
        if frame.rows.iter().any(|r| !r.plain().trim().is_empty()) {
            output.borrow_mut().push(Instant::now());
        }
        Some(cx.subscribe(&session, move |_, _, changed: &screens::terminal::session::Changed, _| {
            if *changed == screens::terminal::session::Changed::Output {
                output.borrow_mut().push(Instant::now());
            }
        }))
    }

    /// Opens a thread as a click does and watches for the first presented frame that shows it.
    fn bench_open(&mut self, thread: &str, window: &mut Window, cx: &mut Context<Self>) -> Presented {
        let Some(item) = self.sidebar.read(cx).items().iter().find(|i| i.thread_id == thread).cloned() else {
            eprintln!("desktop-gpui: the host lists no thread {thread}");
            std::process::exit(1);
        };
        self.go(Route::Thread { workspace_id: item.workspace_id, thread_id: item.thread_id }, window, cx);
        let presented = Presented::default();
        self.bench_view(cx).update(cx, |v, cx| v.watch_presented(thread, presented.clone(), cx));
        presented
    }

    /// The thread view the bench drives: the registered thread screen's, else the frame's own.
    fn bench_view(&self, cx: &App) -> Entity<ThreadView> {
        self.thread_screen().map_or_else(|| self.thread_view.clone(), |s| s.read(cx).view().clone())
    }
}

/// The active shell of the drawer on screen, which a terminal bench types into; the drawer may also hold shells an
/// earlier run left on the daemon.
fn bench_session(cx: &mut App) -> Option<Entity<screens::terminal::session::Session>> {
    let drawer = screens::terminal::shown_drawer(cx)?;
    let active = drawer.update(cx, |d, cx| d.ui(cx)).active_terminal_id;
    let view = drawer.read(cx).views().iter().find(|(id, _)| *id == active)?.1.clone();
    Some(view.read(cx).session().clone())
}
