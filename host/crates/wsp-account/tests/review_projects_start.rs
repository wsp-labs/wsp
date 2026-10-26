// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the panics a caller or the environment can reach in the start half (start-from.ts:221-257). The TS fires
//! the reask with `void ctx.sendDetached(...)` (start-from.ts:239) and cannot fail for want of an executor; the Rust
//! spawns it with `tokio::task::spawn_local` (start_from.rs:844), which panics when the future that called
//! `take_review` is polled outside a `LocalSet`.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/start.rs"]
mod start;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use base::{project, record, FakeBase};
use serde_json::{json, Value};
use start::{pr_json, FakeStart};
use wsp_account::projects::start_from::{start_from_area, StartFromCtx};
use wsp_wire::types::start::WorkspaceFrom;

const REPO: &str = "dev/spoo";

// R-PRJ-S1. A reviewer's reply with no block is asked again in the same thread: the area records the note, then hands the
// reask to a spawned task. On a current-thread runtime with no `LocalSet` that spawn panics, and the reviewer's turn end
// (a host road the person did nothing to cause) takes the host's task with it.
#[test]
fn a_reviewers_reply_with_no_block_is_read_on_any_executor_without_a_panic() {
    let fake = FakeStart::new();
    let ctx: Rc<dyn StartFromCtx> = fake.clone();
    let area = start_from_area(ctx);
    fake.base.hold_project(project(
        "pr_1",
        "spoo",
        "here",
        json!({ "remote": "https://github.com/dev/spoo.git", "path": "/home/me/spoo", "git": { "top": "/home/me/spoo" } }),
    ));
    let fact = pr_json(REPO, 7, json!({}));
    let from: WorkspaceFrom = serde_json::from_value(json!({
        "kind": "review", "repo": REPO, "number": 7, "url": fact["url"], "title": "Rename the status word", "base": fact["base"],
        "head": { "branch": fact["branch"], "oid": fact["headOid"] },
    }))
    .unwrap();
    let entry = fake.base.hold(record("ws_rev", "local", "pr_1", json!({ "name": "Review #7", "from": from, "reviewThread": "th_review" })));
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let ran = catch_unwind(AssertUnwindSafe(|| runtime.block_on(area.take_review(&entry, "th_review", "Looks good to me."))));
    assert!(ran.is_ok(), "take_review panicked outside a LocalSet");
}
