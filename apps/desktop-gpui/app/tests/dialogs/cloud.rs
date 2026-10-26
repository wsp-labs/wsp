// SPDX-License-Identifier: AGPL-3.0-only
//! Add a cloud provider's states for `dialogs_shots`, each a flow staged from fixture answers with no host behind it;
//! `dialogs_add_cloud` drives the same fixtures.
#![allow(dead_code)]

use desktop_gpui::screens::dialogs::add_cloud::flow::{Check, Facts, Flow, Refused, Setup};
use desktop_gpui::screens::dialogs::add_cloud::{self, AddCloud};
use gpui_kit::{App, Entity, Window};
use serde_json::json;

use super::State;

pub fn setup(held: bool) -> Setup {
    serde_json::from_value(json!({ "keys": { "box": held } })).unwrap()
}

/// A flow on Boat's key, its setup read.
pub fn key_flow(held: bool) -> Flow {
    let mut flow = Flow::new(Some("box"));
    flow.on_setup(setup(held));
    flow
}

/// A host that keys two clouds, opened by nobody: the provider step.
pub fn two_clouds() -> Flow {
    let mut flow = Flow::new(None);
    flow.on_setup(serde_json::from_value(json!({ "keys": { "box": false, "solari": false } })).unwrap());
    flow
}

pub fn refused() -> Flow {
    let mut flow = key_flow(false);
    flow.key = "fake-key-1234".into();
    flow.check = Check::Refused(Refused {
        said: "Boat refused this key. Check you copied all of it.".into(),
        fix: None,
        kind: Some("keyRefused".into()),
    });
    flow
}

pub fn unreachable() -> Flow {
    let mut flow = key_flow(false);
    flow.key = "fake-key-1234".into();
    flow.check =
        Check::Unreachable(Refused { said: "Boat could not be reached to check the key: connect ETIMEDOUT".into(), fix: None, kind: None });
    flow
}

pub fn works() -> Flow {
    let mut flow = key_flow(false);
    flow.key = "fake-key-1234".into();
    flow.check = Check::Works(Facts { account: "zingzy".into(), plan: "the $20 plan".into(), running: 1, limit: 8 });
    flow
}

pub fn stage(flow: Flow, window: &mut Window, cx: &mut App) -> Entity<AddCloud> {
    add_cloud::open_on(None, flow, window, cx)
}

macro_rules! state {
    ($id:literal, $flow:expr) => {
        State::new($id, |window, cx| {
            stage($flow, window, cx);
            super::page(window, cx)
        })
    };
}

pub fn states() -> Vec<State> {
    vec![
        state!("W2-DLG-027", key_flow(false)),
        state!("W2-DLG-106", two_clouds()),
        state!("W2-DLG-028", refused()),
        state!("W2-DLG-028-unreachable", unreachable()),
        state!("W2-DLG-029-works", works()),
        state!("W2-DLG-029-held", key_flow(true)),
    ]
}
