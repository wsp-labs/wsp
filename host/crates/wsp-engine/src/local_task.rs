// SPDX-License-Identifier: AGPL-3.0-only
//! Work that goes on after its caller stops waiting, as a JavaScript promise does. No TS file of its own. Spec:
//! spec/vault_import.md, "A caller that stops waiting".
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use tokio::task::JoinHandle;

use crate::errors::{EngineError, Result};

/// `spawn_local` where there is a `LocalSet` to run it, and nothing where there is none. The one panic `spawn_local` makes
/// outside a `LocalSet` is caught, and kept out of the log by a hook that passes every other panic on.
pub fn try_spawn_local<T: 'static>(task: impl Future<Output = T> + 'static) -> Option<JoinHandle<T>> {
    static QUIET: std::sync::Once = std::sync::Once::new();
    // The one panic `spawn_local` makes outside a LocalSet is caught here, so it is kept out of the log.
    QUIET.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let said = match info.payload().downcast_ref::<&str>() {
                Some(said) => said.to_string(),
                None => info.payload().downcast_ref::<String>().cloned().unwrap_or_default(),
            };
            if !said.contains("called from outside of a `task::LocalSet`") {
                previous(info);
            }
        }));
    });
    let mut task = Some(task);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| task.take().map(tokio::task::spawn_local))).ok().flatten()
}

/// Runs `task` on the host's local task set and waits for it. A caller that drops this future leaves the task running
/// to its end. Outside a `LocalSet` there is nothing to run it on, so it runs here and stops with the caller.
pub(crate) async fn finish_on_local_set<T: 'static>(task: impl Future<Output = T> + 'static) -> Result<T> {
    let held = Rc::new(RefCell::new(Some(task)));
    let taken = held.clone();
    let spawned = try_spawn_local(async move {
        let task = taken.borrow_mut().take();
        match task {
            Some(task) => Some(task.await),
            None => None,
        }
    });
    match spawned {
        Some(handle) => match handle.await {
            Ok(Some(done)) => Ok(done),
            Ok(None) => Err(ended()),
            Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
            Err(_) => Err(ended()),
        },
        None => {
            let task = held.borrow_mut().take();
            match task {
                Some(task) => Ok(task.await),
                None => Err(ended()),
            }
        }
    }
}

fn ended() -> EngineError {
    EngineError::Other { message: "the host's task set ended before the work finished".to_string(), code: None }
}
