// SPDX-License-Identifier: AGPL-3.0-only
//! The Ctrl+Tab bench: a held Ctrl+Tab played through the window as the platform hands the keys over, at the key
//! repeat's delay and rate, noting for each key the thread it opened and the first frame presented after it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{Context, KeyDownEvent, Keystroke, Modifiers, ModifiersChangedEvent, PlatformInput, Subscription, Task, Window};

use super::Shell;
use crate::bench::{self, Hold};
use crate::screens::Route;

/// How long the run keeps tracing after the key comes up, so the last step's frames land.
const AFTER: Duration = Duration::from_secs(1);

/// One step as it runs: its times, and cells the first frames after it fill.
pub(super) struct Taken {
    due: Instant,
    sent: Instant,
    handled: Instant,
    from: Option<String>,
    to: Option<String>,
    presented: Rc<Cell<Option<Instant>>>,
    rows: Rc<RefCell<Option<Instant>>>,
}

/// What a hold leaves for its trace: the steps and every frame presented from the press on.
#[derive(Default)]
pub(super) struct Held {
    start: Cell<Option<Instant>>,
    steps: RefCell<Vec<Taken>>,
    frames: RefCell<Vec<Instant>>,
}

fn open_thread(shell: &Shell) -> Option<String> {
    match shell.route() {
        Route::Thread { thread_id, .. } => Some(thread_id.clone()),
        _ => None,
    }
}

impl Shell {
    /// Holds Ctrl+Tab down for `hold`'s steps from now and lets it go, then sets `done` once the last step had its
    /// frames.
    pub(super) fn hold_ctrl_tab(
        &self,
        hold: Hold,
        held: Rc<Held>,
        done: Rc<Cell<bool>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let view = self.bench_view(cx);
        let keystroke = Keystroke::parse("ctrl-tab").expect("ctrl-tab reads");
        let frames = held.clone();
        let tracing = Rc::new(Cell::new(true));
        let still = tracing.clone();
        let dispatching = Rc::new(Cell::new(false));
        let in_key = dispatching.clone();
        // Deferred work runs once the update that drew the frame ends, after GPUI has presented it. GPUI also draws a
        // dirty window inside a key's dispatch, before the key acts, and presents that draw only with the next frame.
        let every_frame: Subscription = window.on_drawn(move |_, cx| {
            if !in_key.get() {
                let frames = frames.clone();
                cx.defer(move |_| frames.frames.borrow_mut().push(Instant::now()));
            }
            still.get()
        });
        cx.spawn_in(window, async move |this, cx| {
            let _every_frame = every_frame;
            let mut watching: Vec<Subscription> = Vec::new();
            let start = Instant::now();
            held.start.set(Some(start));
            let modifiers = keystroke.modifiers;
            let down = PlatformInput::ModifiersChanged(ModifiersChangedEvent { modifiers, capslock: Default::default() });
            _ = cx.update(|window, cx| window.dispatch_event(down, cx));
            for n in 0..hold.steps {
                let due = start + Duration::from_secs_f64(hold.due_ms(n) / 1000.);
                if let Some(wait) = due.checked_duration_since(Instant::now()) {
                    cx.background_executor().timer(wait).await;
                }
                let Ok(from) = this.read_with(cx, |s, _| open_thread(s)) else { return };
                let key = KeyDownEvent { keystroke: keystroke.clone(), is_held: n > 0, prefer_character_input: false };
                let sent = Instant::now();
                // The update's end runs what the key set off, the shell's switch among it, before the time is read.
                dispatching.set(true);
                _ = cx.update(|window, cx| window.dispatch_event(PlatformInput::KeyDown(key), cx));
                dispatching.set(false);
                let handled = Instant::now();
                let Ok(to) = this.read_with(cx, |s, _| open_thread(s)) else { return };
                let presented = Rc::new(Cell::new(None));
                let seen = presented.clone();
                if let Ok(drawn) = cx.update(|window, _| {
                    window.on_drawn(move |_, cx| {
                        let seen = seen.clone();
                        cx.defer(move |_| seen.set(Some(Instant::now())));
                        false
                    })
                }) {
                    watching.push(drawn);
                }
                let rows = Rc::new(RefCell::new(None));
                if let Some(to) = to.as_deref().filter(|_| to != from) {
                    view.update(cx, |v, cx| v.watch_presented(to, rows.clone(), cx));
                }
                held.steps.borrow_mut().push(Taken { due, sent, handled, from, to, presented, rows });
            }
            let up = PlatformInput::ModifiersChanged(ModifiersChangedEvent { modifiers: Modifiers::none(), capslock: Default::default() });
            _ = cx.update(|window, cx| window.dispatch_event(up, cx));
            cx.background_executor().timer(AFTER).await;
            tracing.set(false);
            done.set(true);
            _ = this.update(cx, |_, cx| cx.notify());
        })
    }
}

impl Held {
    pub(super) fn steps(&self) -> usize {
        self.steps.borrow().len()
    }

    /// The trace of the hold, in ms from the press.
    pub(super) fn result(&self, hold: &Hold) -> serde_json::Value {
        let start = self.start.get().unwrap_or_else(Instant::now);
        let ms = |t: Instant| t.saturating_duration_since(start).as_secs_f64() * 1000.;
        let taken = self.steps.borrow();
        // Rows drawn inside a later key's dispatch were never presented: that key switched the thread before a frame.
        let presented = |at: Instant| !taken.iter().any(|k| k.sent <= at && at <= k.handled);
        let steps: Vec<bench::Step> = taken
            .iter()
            .map(|s| bench::Step {
                due_ms: ms(s.due),
                sent_ms: ms(s.sent),
                handled_ms: ms(s.handled),
                from: s.from.clone(),
                to: s.to.clone(),
                presented_ms: s.presented.get().map(ms),
                rows_ms: s.rows.borrow().filter(|&at| presented(at)).map(ms),
            })
            .collect();
        let frames: Vec<f64> = self.frames.borrow().iter().copied().map(ms).collect();
        bench::hold_result(hold, &steps, &frames)
    }
}
