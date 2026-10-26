// SPDX-License-Identifier: AGPL-3.0-only
//! Review probe: the worktree sweep runs when its timer fires, with nothing awaiting it, as the TS's
//! `ctx.state.sweeping = sweepWorktrees()...` (folders.ts:286) starts the sweep at once.
#[path = "support/base.rs"]
mod base;
#[path = "support/folders.rs"]
mod folders;

use std::rc::Rc;

use base::{rec_json, record, run, FakeBase};
use folders::FakeFolders;
use serde_json::{json, Value};
use wsp_account::projects::folders::tree::SWEEP_MS;
use wsp_account::projects::folders::{folders_area, Folders};

const GONE_PATH: &str = "/nonexistent/wsp-worktrees/feat-x";

#[test]
fn review_the_sweep_runs_when_its_timer_fires_with_nothing_awaiting_it() {
    let base = FakeBase::new();
    let fake = FakeFolders::new(base.clone());
    let folders: Rc<Folders> = folders_area(fake.clone());
    base.hold_project(base::project("p_web", "web", "here", json!({ "git": { "top": "/home/me/web" } })));
    let entry =
        base.hold(record("ws_tree", "local", "p_web", json!({ "worktree": { "path": GONE_PATH, "branch": "feat/x", "made": true } })));
    base.threads.borrow_mut().push(("t_ws_tree".into(), base::thread_on("ws_tree")));
    folders.arm_sweep();
    run(async {
        base.clock.advance(SWEEP_MS);
        for _ in 0..200 {
            tokio::task::yield_now().await;
        }
    });
    // The TS marks a made worktree whose folder is gone `removed` on the first sweep (folders.ts:310).
    let tree: Value = rec_json(&entry).get("worktree").cloned().unwrap_or(Value::Null);
    assert_eq!(tree["gone"], json!(true), "ten minutes on, the sweep never ran: {tree}");
}

/// Control: the same sweep, awaited by hand as the crate's own test does, marks the tree gone; only the polling is
/// missing.
#[test]
fn review_control_the_same_sweep_awaited_by_hand_marks_the_tree_gone() {
    let base = FakeBase::new();
    let fake = FakeFolders::new(base.clone());
    let folders: Rc<Folders> = folders_area(fake.clone());
    base.hold_project(base::project("p_web", "web", "here", json!({ "git": { "top": "/home/me/web" } })));
    let entry =
        base.hold(record("ws_tree", "local", "p_web", json!({ "worktree": { "path": GONE_PATH, "branch": "feat/x", "made": true } })));
    base.threads.borrow_mut().push(("t_ws_tree".into(), base::thread_on("ws_tree")));
    folders.arm_sweep();
    run(async {
        base.clock.advance(SWEEP_MS);
        let sweeping = fake.sweep.borrow().sweeping.clone().expect("the sweep is in flight");
        sweeping.await;
    });
    let tree: Value = rec_json(&entry).get("worktree").cloned().unwrap_or(Value::Null);
    assert_eq!(tree["gone"], json!(true), "{tree}");
}
