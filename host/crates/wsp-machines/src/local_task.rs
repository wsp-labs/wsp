// SPDX-License-Identifier: AGPL-3.0-only
//! Starting a task on the host's `LocalSet` where there is one. The public entry points of the areas start background
//! work as a promise starts, and a caller on a plain runtime has no `LocalSet` for it: `tokio::task::spawn_local` panics
//! there, where the TS has nowhere to be missing from. wsp-engine's `try_spawn_local` is the one that starts it.

use std::cell::RefCell;
use std::future::Future;
use std::panic::resume_unwind;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use wsp_engine::local_task::try_spawn_local;

/// A task started on the `LocalSet` where there is one, else the work itself, driven by whoever awaits it. Either way
/// the future answers what the work answers.
pub(crate) fn started<T: 'static>(
    task: impl Future<Output = T> + 'static,
) -> LocalBoxFuture<'static, T> {
    let held = Rc::new(RefCell::new(Some(task)));
    let taken = Rc::clone(&held);
    let spawned = try_spawn_local(async move {
        let task = taken.borrow_mut().take();
        match task {
            Some(task) => Some(task.await),
            None => None,
        }
    });
    async move {
        match spawned {
            Some(handle) => match handle.await {
                Ok(Some(out)) => out,
                Ok(None) => unreachable!("a started task is taken once"),
                Err(e) => resume_unwind(e.into_panic()),
            },
            None => {
                let task = held.borrow_mut().take();
                task.expect("a task nobody started is still here").await
            }
        }
    }
    .boxed_local()
}
