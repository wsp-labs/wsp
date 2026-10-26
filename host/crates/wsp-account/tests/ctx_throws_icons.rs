// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `stateFolder` throws through the project icons area's read (images.ts:138, 173). The folder is named
//! where a file is read, so its throw is the read's answer.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use async_trait::async_trait;
use base::{plain, run, TestIds};
use wsp_account::projects::images::{project_icons_area, ProjectIconsCtx};
use wsp_core::ids::Ids;
use wsp_core::types::wiring::RecipeShelf;
use wsp_wire::views::preferences::Preferences;
use wsp_words::exit::HostError;

/// The context the icon area reads: the state folder answers the failure it holds, the rest is never asked here.
struct Icons {
    fail: RefCell<Option<HostError>>,
    ids: Rc<TestIds>,
}

#[async_trait(?Send)]
impl ProjectIconsCtx for Icons {
    fn state_folder(&self) -> Result<PathBuf, HostError> {
        match self.fail.borrow().clone() {
            Some(e) => Err(e),
            None => Ok(PathBuf::from("/nonexistent/state")),
        }
    }
    fn has_state_path(&self) -> bool {
        true
    }
    async fn ready(&self) -> Result<(), HostError> {
        unreachable!("read does not wait for ready")
    }
    fn project_held(&self, _project_id: &str) -> bool {
        unreachable!("read does not look at held projects")
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        unreachable!("read does not read preferences")
    }
    async fn change_preferences(&self, _change: Box<dyn FnOnce(Preferences) -> Preferences>) -> Result<Preferences, HostError> {
        unreachable!("read changes no preferences")
    }
    fn recipes(&self) -> Option<Rc<dyn RecipeShelf>> {
        None
    }
    fn on_recipes_changed(&self, _listener: Rc<dyn Fn()>) {}
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
}

// images.ts:173 (read): the folder is named where a kept file is read, so the throw is the read's rejection.
#[test]
fn ctx_throws_state_folder_passes_through_icon_read() {
    run(async {
        let ctx: Rc<dyn ProjectIconsCtx> =
            Rc::new(Icons { fail: RefCell::new(Some(plain("the state folder is not readable"))), ids: TestIds::new() });
        let area = project_icons_area(ctx);
        let got = area.read(vec!["a".repeat(64)]).await.expect_err("stateFolder throws");
        assert_eq!(got.message, "the state folder is not readable");
    });
}
