// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448), path 2: the thread's word in the owed road.
//! threads.ts:699 words a thread by `from.slice(0, 8)`, which counts UTF-16 code units; owed.rs:142 `cut8` counts
//! characters. Thread ids are uuids, so this does not show in a running host.
#[path = "support_b/mod.rs"]
mod support;

use std::rc::Rc;

use serde_json::json;
use support::threads_ctx::{entry, FakeThreadsCtx};
use support::Lend;
use support::{local, row, settle, view};
use wsp_threads::threads::threads::{read_owed, ThreadsArea};

fn host() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "phase": "running" }));
    ctx.add_entry(e);
    let area = ctx.area();
    (ctx, area)
}

#[test]
fn a_thread_is_worded_by_its_first_eight_utf16_units() {
    local(async {
        let (_ctx, area) = host();
        let id = "\u{1F600}".repeat(10);
        let line = read_owed(&json!({ "id": "L1", "from": id, "notify": "gone", "text": "t" })).expect("a line");
        let warned = wsp_core::console::capture();
        area.deliver(line, false);
        settle().await;
        let four = "\u{1F600}".repeat(4);
        assert_eq!(warned.lines(), vec![format!("thread {four} ended, but thread gone has no session to tell")]);
    });
}
