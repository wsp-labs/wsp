// SPDX-License-Identifier: AGPL-3.0-only
//! Starting a task on the host's `LocalSet` where there is one. The public entry points of the areas start background
//! work as a promise starts, and a caller on a plain runtime has no `LocalSet` for it: `tokio::task::spawn_local` panics
//! there, where the TS has nowhere to be missing from. wsp-engine keeps the same shape (`local_task.rs`).

use std::cell::RefCell;
use std::future::Future;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use tokio::task::JoinHandle;

/// `spawn_local` where there is a `LocalSet` to run it, and nothing where there is none: the work is then not started,
/// and whoever awaits it drives it.
pub(crate) fn spawn<T: 'static>(task: impl Future<Output = T> + 'static) -> Option<JoinHandle<T>> {
    let mut task = Some(task);
    catch_unwind(AssertUnwindSafe(|| {
        task.take().map(tokio::task::spawn_local)
    }))
    .ok()
    .flatten()
}

/// A task started on the `LocalSet` where there is one, else the work itself, driven by whoever awaits it. Either way
/// the future answers what the work answers.
pub(crate) fn started<T: 'static>(
    task: impl Future<Output = T> + 'static,
) -> LocalBoxFuture<'static, T> {
    let held = Rc::new(RefCell::new(Some(task)));
    let taken = Rc::clone(&held);
    let spawned = spawn(async move {
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
