// SPDX-License-Identifier: AGPL-3.0-only
//! A panic under a future other callers join is that one run's failure. Left to unwind through a `Shared` kept in a
//! map, it would be handed to every later caller for good, where the TS's `finally` clears the slot and the next call
//! runs afresh (spec rule 3.5: the host keeps serving).

use std::future::Future;
use std::panic::AssertUnwindSafe;

use futures::FutureExt;
use wsp_words::exit::HostError;

/// The future's answer, or the words of the panic that took it.
pub(crate) async fn unwound<F: Future>(fut: F) -> Result<F::Output, String> {
    AssertUnwindSafe(fut).catch_unwind().await.map_err(|panic| {
        panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "a task panicked".to_string())
    })
}

/// A run's answer where a panic under it is a failure with the panic's words.
pub(crate) async fn unwound_or_failed<T, F: Future<Output = Result<T, HostError>>>(
    fut: F,
) -> Result<T, HostError> {
    unwound(fut)
        .await
        .unwrap_or_else(|said| Err(HostError::new(said)))
}
