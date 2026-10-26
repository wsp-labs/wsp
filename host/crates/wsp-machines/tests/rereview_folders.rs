// SPDX-License-Identifier: AGPL-3.0-only
//! Re-review of wsp-machines at 111458092: the folder add's mark after the add has unwound.

use std::panic::AssertUnwindSafe;

use futures::FutureExt;
use wsp_machines::places::setup::{current_folders_of, under_folders_of};

/// setup.ts:651 runs the add under `foldersOf.run(placeId, ...)`, and a throw out of it leaves nothing outside the run
/// reading the store (`getStore()` is undefined there again), so the gate at link.ts:431 asks every later caller's
/// fork whether the computer is busy setting up. setup.rs `polled_under` puts the old mark back only when `work()`
/// returns, so a panic that unwinds out of the add leaves the place marked on the host's thread, and every later fork
/// there on that thread skips the busy check.
#[test]
fn a_panic_out_of_the_folder_add_leaves_no_mark_on_the_thread() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let set = tokio::task::LocalSet::new();
    set.block_on(&rt, async {
        let unwound = AssertUnwindSafe(under_folders_of("p_spoo", async {
            panic!("the add panicked");
        }))
        .catch_unwind()
        .await;
        assert!(unwound.is_err(), "the add's panic reaches its caller");
        assert_eq!(
            current_folders_of(),
            None,
            "after the add unwound, the host's thread still reads p_spoo as the setup whose own add is running"
        );
    });
}
