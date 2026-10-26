// SPDX-License-Identifier: AGPL-3.0-only
//! `try_spawn_local`: the work is started where a `LocalSet` runs, never started (and nothing printed) where none does, and
//! the hook that keeps that one panic out of the log passes every other panic on.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use wsp_engine::local_task::try_spawn_local;

#[test]
fn work_starts_on_a_local_set_and_not_off_one_and_other_panics_still_reach_the_hook_before_it() {
    let seen = Arc::new(AtomicUsize::new(0));
    let counted = seen.clone();
    std::panic::set_hook(Box::new(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
    }));

    assert!(try_spawn_local(async {}).is_none(), "no LocalSet runs here, so nothing starts");
    assert_eq!(seen.load(Ordering::SeqCst), 0, "the missing LocalSet's panic is kept out of the hook");

    let ran = tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        tokio::task::LocalSet::new().run_until(async { try_spawn_local(async { 7 }).expect("a LocalSet runs here").await.unwrap() }).await
    });
    assert_eq!(ran, 7);

    let _ = std::panic::catch_unwind(|| panic!("some other panic"));
    assert_eq!(seen.load(Ordering::SeqCst), 1, "every other panic still reaches the hook that was there");
}
