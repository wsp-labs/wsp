// SPDX-License-Identifier: AGPL-3.0-only
//! What the assembly hands the areas where the TS leans on the language: a `void f()` that starts `f` now, and the
//! engine's retry clock over the runtime's own clock.
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::oneshot;
use wsp_core::clock::Clock;
use wsp_engine::errors::RetryClock;
use wsp_engine::local_task::try_spawn_local;

/// `void f()`: starts `f` now on the host's local task set and lets it run to its end. A future dropped instead of
/// started never runs, which is what a Rust future does and a promise does not (wsp-account review, item 4); every
/// area's `spawn` is this. Answers whether there was a local task set to run it on.
pub fn spawn_detached(task: impl Future<Output = ()> + 'static) -> bool {
    try_spawn_local(task).is_some()
}

/// The engine's `RetryClock` over the runtime's clock, so `kill_until_gone`, `sight_machine` and `snapshot_until_gone`
/// wait on the time the runtime's tests and timers move (the cross-crate pass, item 8).
pub struct RuntimeRetryClock(pub Rc<dyn Clock>);

#[async_trait(?Send)]
impl RetryClock for RuntimeRetryClock {
    fn now(&self) -> i64 {
        self.0.now()
    }
    async fn sleep(&self, ms: u64) {
        let (done, wait) = oneshot::channel();
        let _held = self.0.schedule(
            Box::new(move || {
                let _ = done.send(());
            }),
            ms as i64,
        );
        let _ = wait.await;
    }
    fn random(&self) -> f64 {
        rand::random::<f64>()
    }
}

/// A sleep that is only `Duration`, for callers with no clock of the runtime's at hand.
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await
}
