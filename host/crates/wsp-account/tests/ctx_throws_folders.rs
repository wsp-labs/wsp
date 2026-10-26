// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `stateFolder` and `elsewhereRefusal` throw through the folders area. A throw is not a refusal: the
//! caller gets the throw, and the work after the lookup does not run.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/folders.rs"]
mod folders;

use base::{plain, project, record, run, thread, FakeBase};
use folders::FakeFolders;
use serde_json::{json, Value};
use wsp_account::projects::folders::folders_area;
use wsp_account::projects::folders::place::FolderAsk;
use wsp_words::exit::HostError;

fn web() -> wsp_wire::views::workspace::ProjectView {
    project("p_web", "web", "here", json!({ "git": { "top": "/home/me/web" } }))
}

fn boxed() -> wsp_wire::views::workspace::ProjectView {
    project("p_box", "box", "pl_spoo", json!({ "path": "/root/box", "git": { "top": "/root/box" } }))
}

// folders.ts:135: ctx.stateFolder() is a call argument, so its throw rejects before the mount's .catch.
#[test]
fn ctx_throws_state_folder_passes_through_worktree_mounted() {
    let base = FakeBase::new();
    let fake = FakeFolders::new(base.clone());
    let folders = folders_area(fake.clone());
    base.hold_project(web());
    let path = std::env::temp_dir().to_string_lossy().into_owned();
    let entry = base.hold(record("ws1", "local", "p_web", json!({ "worktree": { "path": path, "branch": "feat/x", "made": true } })));
    *fake.state_fail.borrow_mut() = Some(plain("the state folder is not readable"));
    let got = run(folders.worktree_mounted(&entry)).expect_err("stateFolder throws");
    assert_eq!(got.message, "the state folder is not readable");
    assert_eq!(base.args_of("copier_worktree_mount"), Vec::<Value>::new());
}

// folders.ts:169: ctx.elsewhereRefusal throws for a box thread's word, and placeFor answers that throw.
#[test]
fn ctx_throws_elsewhere_refusal_passes_through_place_for() {
    let base = FakeBase::new();
    let fake = FakeFolders::new(base.clone());
    let folders = folders_area(fake.clone());
    base.hold_project(web());
    base.hold_project(boxed());
    base.hold(record("ws_box_ask", "place", "p_box", json!({ "place": "pl_spoo" })));
    *fake.elsewhere_fail.borrow_mut() = Some(plain("the project's computer is not reachable"));
    let ask = FolderAsk { project: Some("web".into()), ..Default::default() };
    let got = run(folders.place_for(&ask, Some(thread("t1", "ws_box_ask", "t1")))).err().expect("elsewhereRefusal throws");
    assert_eq!(got.message, "the project's computer is not reachable");
    assert_eq!(base.args_of("elsewhere_refusal").len(), 1);
    assert_eq!(base.args_of("resolve_project"), Vec::<Value>::new());
}
