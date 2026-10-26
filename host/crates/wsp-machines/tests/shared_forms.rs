// SPDX-License-Identifier: AGPL-3.0-only
//! The creates the create area holds for a window, failed and shown, are the maps the workspaces area reads through
//! the context (context.ts:542 `ShownCreate`, machines/create.ts `failedCreates` and `shownCreates`), so the area hands
//! out the one it writes (create_runtime packet 0b).

mod common;
#[path = "support/create.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use support::{view_of, World};
use wsp_wire::views::workspace_more::WorkspaceView;

#[test]
fn the_failed_creates_an_area_keeps_are_the_map_it_hands_out_and_a_new_create_clears_them() {
    common::run_local(async {
        let w = World::small();
        let held: Rc<RefCell<IndexMap<String, WorkspaceView>>> = w.area.failed_creates.clone();
        for (key, name) in [("f1", "w1"), ("f2", "w2")] {
            held.borrow_mut()
                .insert(key.to_string(), view_of(&format!("v-{key}"), name));
        }
        assert!(Rc::ptr_eq(&held, &w.area.failed_creates));
        w.area.supersede_failed("w1");
        assert_eq!(
            held.borrow().keys().cloned().collect::<Vec<_>>(),
            vec!["f2".to_string()]
        );
    })
}

#[test]
fn the_shown_creates_an_area_keeps_are_the_map_it_hands_out() {
    common::run_local(async {
        let w = World::small();
        let held: Rc<RefCell<IndexMap<String, WorkspaceView>>> = w.area.shown_creates.clone();
        w.area.show_create(view_of("s1", "shown"));
        assert!(Rc::ptr_eq(&held, &w.area.shown_creates));
        assert_eq!(
            held.borrow().keys().cloned().collect::<Vec<_>>(),
            vec!["s1".to_string()]
        );
    })
}
