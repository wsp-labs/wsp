// SPDX-License-Identifier: AGPL-3.0-only
//! A value the host's one thread owns, passed through a seam that asks for `Send + Sync`.
//!
//! Not a TS file. wsp-adapters' `ExecStream` and `ExecStreamFactory` are `Send + Sync`, and the engine's `Machine` no
//! longer is (rule 3.1: one thread, `async_trait(?Send)`), so the two exec roads, which are the former and hold the
//! latter, cannot name it bare. Sound only while the host keeps every task on its one `LocalSet` thread, which it
//! does; a task started with `tokio::spawn` onto a multi-thread runtime would break that. Delete this file when
//! wsp-adapters' port drops `Send + Sync` (the join's report asks for that ruling).
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A `T` that claims `Send + Sync` for the one thread that owns it.
pub struct OwnerThread<T>(T);

unsafe impl<T> Send for OwnerThread<T> {}
unsafe impl<T> Sync for OwnerThread<T> {}

impl<T> OwnerThread<T> {
    pub fn new(value: T) -> Self {
        OwnerThread(value)
    }

    pub fn get(&self) -> &T {
        &self.0
    }
}

/// A future that claims `Send` for the one thread that polls it, so a call on a `?Send` machine can sit in the
/// `BoxFuture` the adapters' port asks for.
pub struct OwnerFuture<F>(F);

unsafe impl<F> Send for OwnerFuture<F> {}

impl<F> OwnerFuture<F> {
    pub fn new(future: F) -> Self {
        OwnerFuture(future)
    }
}

impl<F: Future> Future for OwnerFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        // SAFETY: the inner future is never moved out of its pin.
        unsafe { self.map_unchecked_mut(|f| &mut f.0) }.poll(cx)
    }
}
