// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/idle.ts at 65f6b89c7. Spec: spec/idle.md.
//!
//! The runtime owns when a workspace naps. The provider's idle timer cannot be the policy: every GET /sandboxes/:id
//! resets it, so any process that reads machine state keeps the machine awake and billing. The provider timer is set
//! behind this one as a backstop, at twice the window, so it fires only when this process is gone.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use crate::clock::{Cancel, Clock};

// idle.ts:14
// export const IDLE_OFF_BACKSTOP_MS = 6 * 60 * 60_000;
/// Backstop for a workspace whose auto-nap is off: six hours, the longest timeoutMs the provider has accepted.
pub const IDLE_OFF_BACKSTOP_MS: i64 = 6 * 60 * 60_000;

// idle.ts:16
// export function backstopMs(windowMs: number | null): number {
//   return windowMs === null ? IDLE_OFF_BACKSTOP_MS : windowMs * 2;
// }
pub fn backstop_ms(window_ms: Option<i64>) -> i64 {
    todo!()
}

/// What `on_idle` answers: `Err` carries the words the warn line quotes.
pub type NapFuture = LocalBoxFuture<'static, Result<(), String>>;

// idle.ts:20
// export interface IdlePolicyOptions { windowOf, onIdle, busy?, retryMs, onBackstop?, clock? }
pub struct IdlePolicyOptions {
    /// The window for a workspace right now; `None` means auto-nap is off for it.
    pub window_of: Rc<dyn Fn(&str) -> Option<i64>>,
    /// Called inside the timer callback when a window runs out; its future is spawned on the local set. `Ok`: the
    /// workspace is forgotten until its next touch; `Err`: the deadline stands and it fires again every `retry_ms`.
    pub on_idle: Rc<dyn Fn(&str, i64) -> NapFuture>,
    /// A turn reads running on the workspace: held like a hold.
    pub busy: Option<Rc<dyn Fn(&str) -> bool>>,
    /// How long after a nap that failed the window is asked again.
    pub retry_ms: i64,
    /// Every arming, synchronously: now plus `backstop_ms` of the window. Not on forget.
    pub on_backstop: Option<Rc<dyn Fn(&str, i64)>>,
    /// Defaults to the real clock.
    pub clock: Option<Rc<dyn Clock>>,
}

// idle.ts:51
// interface Armed { at: number; cancel: () => void; }
struct Armed {
    at: i64,
    /// Stands for the TS's object identity: a nap's outcome touches only the entry it fired for.
    serial: u64,
    cancel: Cancel,
}

#[derive(Default)]
struct IdleState {
    armed: IndexMap<String, Armed>,
    holds: IndexMap<String, i64>,
    closed: bool,
    serial: u64,
}

// idle.ts:38
// export interface IdlePolicy { touch, hold, release, forget, idleAt, close }
/// One per runtime. Clones share one state.
#[derive(Clone)]
pub struct IdlePolicy {
    options: Rc<IdlePolicyOptions>,
    state: Rc<RefCell<IdleState>>,
}

// idle.ts:56
// export function createIdlePolicy(o: IdlePolicyOptions): IdlePolicy {
//   const armed = new Map<string, Armed>();
//   const holds = new Map<string, number>();
//   const clock = o.clock ?? realClock;
//   let closed = false;
//
//   const held = (id: string): boolean => (holds.get(id) ?? 0) > 0 || o.busy?.(id) === true;
//
//   const forget = (id: string): void => {
//     const a = armed.get(id);
//     if (!a) return;
//     a.cancel();
//     armed.delete(id);
//   };
//
//   const arm = (id: string): void => {
//     forget(id);
//     if (closed) return;
//     const windowMs = o.windowOf(id);
//     o.onBackstop?.(id, clock.now() + backstopMs(windowMs));
//     if (windowMs === null) return;
//     const entry: Armed = { at: clock.now() + windowMs, cancel: () => {} };
//     const fire = (): void => {
//       if (held(id)) {
//         arm(id);
//         return;
//       }
//       // Armed while the nap runs, so a status pushed by a nap that failed still carries the deadline and never reads active.
//       entry.cancel = () => {};
//       o.onIdle(id, windowMs).then(
//         () => {
//           if (armed.get(id) === entry) armed.delete(id);
//         },
//         (e: unknown) => {
//           console.warn(`idle nap of ${id} failed: ${e instanceof Error ? e.message : String(e)}`);
//           if (armed.get(id) === entry) entry.cancel = clock.schedule(fire, o.retryMs, { unref: true });
//         },
//       );
//     };
//     entry.cancel = clock.schedule(fire, windowMs, { unref: true });
//     armed.set(id, entry);
//   };
pub fn create_idle_policy(o: IdlePolicyOptions) -> IdlePolicy {
    todo!()
}

impl IdlePolicy {
    // idle.ts:100
    // touch: arm,
    /// Activity: the window starts over from now.
    pub fn touch(&self, id: &str) {
        todo!()
    }
    // idle.ts:101
    // hold: id => holds.set(id, (holds.get(id) ?? 0) + 1),
    /// A running session: the window may not fire while any hold stands.
    pub fn hold(&self, id: &str) {
        todo!()
    }
    // idle.ts:102
    // release: id => {
    //   const left = (holds.get(id) ?? 0) - 1;
    //   if (left > 0) {
    //     holds.set(id, left);
    //     return;
    //   }
    //   holds.delete(id);
    //   if (armed.has(id)) arm(id);
    // },
    /// The last release starts an armed window over.
    pub fn release(&self, id: &str) {
        todo!()
    }
    // idle.ts:111
    // forget: id => {
    //   forget(id);
    //   holds.delete(id);
    // },
    /// Napping or deleted: no window, no holds.
    pub fn forget(&self, id: &str) {
        todo!()
    }
    // idle.ts:115
    // idleAt: id => (held(id) ? undefined : armed.get(id)?.at),
    /// Epoch ms when the window fires; `None` when off, held, or not counting.
    pub fn idle_at(&self, id: &str) -> Option<i64> {
        todo!()
    }
    // idle.ts:116
    // close: () => {
    //   closed = true;
    //   for (const id of [...armed.keys()]) forget(id);
    // },
    pub fn close(&self) {
        todo!()
    }
}
