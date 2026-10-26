// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/clock.ts and the test clock packages/runtime/test/fake-clock.ts at 65f6b89c7.
//! Spec: spec/clock.md.
//!
//! The one time seam of the runtime (rule 3.15). `unref` is dropped: it has no meaning in Rust.
use async_trait::async_trait;
use futures::channel::oneshot;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wsp_engine::errors::RetryClock;

/// Stops a timer that has not fired; a second call, or one after it fired, does nothing (clock.ts:21).
pub type Cancel = Rc<dyn Fn()>;

/// A timer's body; it runs once, on the owner thread.
pub type Timer = Box<dyn FnOnce()>;

// clock.ts:4
// export interface Clock {
//   /** Epoch ms. */
//   now(): number;
//   /** Runs fn once after ms; the returned function cancels it. The timer holds the process open unless unref is set. */
//   schedule(fn: () => void, ms: number, o?: ScheduleOptions): () => void;
// }
pub trait Clock {
    /// Epoch ms.
    fn now(&self) -> i64;
    /// Runs `f` once after `ms`; the answer cancels it.
    fn schedule(&self, f: Timer, ms: i64) -> Cancel;
}

/// Node's setTimeout range: a delay below 1 or above this fires after 1 ms.
pub const TIMEOUT_MAX_MS: i64 = 2_147_483_647;

// clock.ts:16
// export const realClock: Clock = {
//   now: () => Date.now(),
//   schedule: (fn, ms, o) => {
//     const timer = setTimeout(fn, ms);
//     if (o?.unref) timer.unref?.();
//     return () => clearTimeout(timer);
//   },
// };
/// The process clock: system time, and timers spawned on the current thread's `LocalSet`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
    }
    /// Panics outside a `LocalSet`, as `spawn_local` does. Clamps the delay as Node does (spec Behaviour 4).
    fn schedule(&self, f: Timer, ms: i64) -> Cancel {
        let ms = if (1..=TIMEOUT_MAX_MS).contains(&ms) { ms } else { 1 };
        let task = tokio::task::spawn_local(async move {
            tokio::time::sleep(Duration::from_millis(ms as u64)).await;
            f();
        });
        Rc::new(move || task.abort())
    }
}

/// `realClock` as the handle every option defaults to.
pub fn real_clock() -> Rc<dyn Clock> {
    Rc::new(RealClock)
}

struct Pending {
    id: u64,
    at: i64,
    f: Timer,
}

#[derive(Default)]
struct FakeState {
    now: i64,
    seq: u64,
    timers: Vec<Pending>,
}

// fake-clock.ts:8
// export function fakeClock(start = Date.now()) {
//   let now = start;
//   let seq = 0;
//   const timers = new Map<number, { at: number; fn: () => void; unref: boolean }>();
//   const clock: Clock = {
//     now: () => now,
//     schedule: (fn, ms, o) => {
//       const id = ++seq;
//       timers.set(id, { at: now + ms, fn, unref: o?.unref === true });
//       return () => void timers.delete(id);
//     },
//   };
//   ...
// }
/// A clock that moves only when the test says so. Every handle from `clock()` shares one state.
#[derive(Clone)]
pub struct FakeClock {
    state: Rc<RefCell<FakeState>>,
}

impl FakeClock {
    /// Starts at `start` epoch ms.
    pub fn new(start: i64) -> Self {
        FakeClock { state: Rc::new(RefCell::new(FakeState { now: start, ..FakeState::default() })) }
    }
    /// Starts at the system time, as `fakeClock()` with no argument.
    pub fn at_now() -> Self {
        Self::new(RealClock.now())
    }
    /// A `Clock` sharing this fake's state.
    pub fn clock(&self) -> Rc<dyn Clock> {
        Rc::new(self.clone())
    }
    pub fn now(&self) -> i64 {
        self.state.borrow().now
    }
    // fake-clock.ts:20
    // const advance = (ms: number): void => {
    //   const target = now + ms;
    //   for (;;) {
    //     let next: [number, { at: number; fn: () => void; unref: boolean }] | undefined;
    //     for (const t of timers) if (t[1].at <= target && (next === undefined || t[1].at < next[1].at)) next = t;
    //     if (next === undefined) break;
    //     timers.delete(next[0]);
    //     now = next[1].at;
    //     next[1].fn();
    //   }
    //   now = target;
    // };
    /// Runs the timers that fall due by `now + ms`, earliest first, ties in scheduling order, each with `now` at its
    /// own `at` and no borrow of the state held; then lands on the target.
    pub fn advance(&self, ms: i64) {
        let target = self.state.borrow().now + ms;
        loop {
            let due = {
                let mut st = self.state.borrow_mut();
                let next = st.timers.iter().enumerate().filter(|(_, t)| t.at <= target).min_by_key(|(_, t)| (t.at, t.id)).map(|(i, _)| i);
                next.map(|i| {
                    let t = st.timers.remove(i);
                    st.now = t.at;
                    t.f
                })
            };
            match due {
                Some(f) => f(),
                None => break,
            }
        }
        self.state.borrow_mut().now = target;
    }
    // fake-clock.ts:34
    // pending: () => timers.size,
    pub fn pending(&self) -> usize {
        self.state.borrow().timers.len()
    }
    // fake-clock.ts:37
    // dueFor: (at: number) => [...timers.values()].some(t => t.at === at),
    pub fn due_for(&self, at: i64) -> bool {
        self.state.borrow().timers.iter().any(|t| t.at == at)
    }
}

impl Clock for FakeClock {
    fn now(&self) -> i64 {
        self.state.borrow().now
    }
    fn schedule(&self, f: Timer, ms: i64) -> Cancel {
        let id = {
            let mut st = self.state.borrow_mut();
            st.seq += 1;
            let (id, at) = (st.seq, st.now + ms);
            st.timers.push(Pending { id, at, f });
            id
        };
        let state = Rc::downgrade(&self.state);
        Rc::new(move || {
            if let Some(state) = state.upgrade() {
                state.borrow_mut().timers.retain(|t| t.id != id);
            }
        })
    }
}

struct CancelOnDrop(Cancel);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        (self.0)()
    }
}

// errors.ts:315
// export const realRetryClock: RetryClock = {
//   now: () => Date.now(),
//   sleep: ms => new Promise(r => setTimeout(r, ms)),
//   random: () => Math.random(),
// };
/// The engine's `RetryClock` over the runtime's `Clock`: `now` and `sleep` follow the clock the context holds (rule
/// 3.15), so a test that moves the fake clock moves the engine's retries. A sleep dropped before it ends cancels its
/// timer.
pub struct RetryClockOver {
    clock: Rc<dyn Clock>,
}

impl RetryClockOver {
    pub fn new(clock: Rc<dyn Clock>) -> Self {
        RetryClockOver { clock }
    }
}

#[async_trait(?Send)]
impl RetryClock for RetryClockOver {
    fn now(&self) -> i64 {
        self.clock.now()
    }
    async fn sleep(&self, ms: u64) {
        let (tx, rx) = oneshot::channel::<()>();
        let _stop = CancelOnDrop(self.clock.schedule(
            Box::new(move || {
                let _ = tx.send(());
            }),
            i64::try_from(ms).unwrap_or(i64::MAX),
        ));
        let _ = rx.await;
    }
    fn random(&self) -> f64 {
        rand::random::<f64>()
    }
}
