// SPDX-License-Identifier: AGPL-3.0-only
//! Review: panics a client frame or an adapter answer can reach in the sessions area. Each test is red now.
#[path = "support_a/mod.rs"]
mod support;

use std::rc::Rc;

use support::*;

fn host() -> (Rc<FakeHost>, SessionsArea) {
    let host = FakeHost::new();
    host.add_entry(entry("w1", json!({})));
    let area = SessionsArea::new(host.clone());
    (host, area)
}

/// `sessions.attachment` carries `index` as a non-negative integer the wire never caps (runtime_op_pass_a.rs:718
/// checks only `>= 0`), and the api takes it as a u32. The refusal for a missing image writes `index + 1`
/// (sessions.ts:972 adds one to a float). reads.rs:217.
#[test]
fn attachment_of_refuses_the_largest_index_without_panicking() {
    run_local(async {
        let (_host, area) = host();
        let e = area.attachment_of("w1", "t1", "req_a", u32::MAX, None).await.expect_err("no such image");
        assert_eq!(e.message, "no image 4294967296 kept on that message");
    });
}

fn no_local_set<F: std::future::Future<Output = ()>>(f: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    rt.block_on(f);
}

/// Every detached task of the crate is `tokio::task::spawn_local` (stop.rs:171 here, and 45 more sites), so a caller
/// that is not inside a `LocalSet` takes the host down. The TS has no such seam.
#[test]
fn hold_next_runs_on_a_runtime_with_no_local_set() {
    no_local_set(async {
        let (_host, area) = host();
        area.hold_next("t1", done(), None);
    });
}
