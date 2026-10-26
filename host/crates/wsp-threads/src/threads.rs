// SPDX-License-Identifier: AGPL-3.0-only
//! `runtime/src/threads/`, one module per TS file.

pub mod agents;
pub mod answered_start;
pub mod conversations;
pub mod forks;
pub mod launch;
pub mod sessions;
#[allow(clippy::module_inception)]
pub mod threads;
pub mod transcripts;
pub mod trimmed_row;
pub mod turns;
pub mod waits;

/// A future's output left behind, for work handed to a context's `spawn`.
pub(crate) fn unit<F: std::future::Future + 'static>(work: F) -> futures::future::LocalBoxFuture<'static, ()> {
    use futures::FutureExt;
    work.map(|_| ()).boxed_local()
}
