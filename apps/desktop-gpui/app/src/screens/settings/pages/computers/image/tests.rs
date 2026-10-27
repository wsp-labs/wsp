// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

use crate::screens::settings::model::{At, Data, GroupId};
use gpui_kit::{Modifiers, TestAppContext, VisualTestContext};
use std::sync::Arc;

type Draw = Box<dyn Fn(&Ctx, &mut Window, &mut App) -> AnyElement>;

/// Draws `draw` in a window as a page of Settings would, with no host behind it.
struct Harness(Draw, Data, Arc<tokio::runtime::Runtime>);

impl Render for Harness {
    fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let acts = crate::screens::settings::Acts::new(gpui_kit::WeakEntity::new_invalid(), None, self.2.clone());
        let activity = cx.new(|_| crate::screens::settings::pages::activity::Pick::default());
        let agents = cx.new(|_| crate::screens::settings::pages::agents::Store::new(acts.clone()));
        let at = At::Group(GroupId::Computers);
        let ctx = Ctx::new(
            &self.1,
            &at,
            None,
            acts,
            activity,
            agents,
            cx.new(|_| crate::screens::settings::pages::computers::RowMenu::default()),
        );
        div().w(px(900.)).child((self.0)(&ctx, window, cx))
    }
}

#[test]
fn a_projects_new_thread_opens_on_its_own_folders_workspace() {
    let workspace = |id: &str, project: &str, kind: &str, more: serde_json::Value| -> WorkspaceOut {
        let mut w = json!({ "id": id, "name": id, "machineId": "m", "phase": "running", "golden": "", "createdAt": "2026-10-17T00:00:00Z",
            "project": { "id": project, "name": project, "path": "/p", "computer": "here" }, "kind": kind });
        w.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
        serde_json::from_value(w).unwrap()
    };
    let listed = vec![
        workspace("w-other", "p2", "local", json!({})),
        workspace("w-fork", "p1", "cloud", json!({})),
        workspace("w-sub", "p1", "local", json!({ "parentThreadId": "t0" })),
        workspace("w-gone", "p1", "local", json!({ "gone": "2026-10-17T00:00:00Z" })),
        workspace("w-home", "p1", "local", json!({})),
    ];
    assert_eq!(home_of(&listed, "p1"), Some("w-home"), "the project's own folder, past a cloud one, a sub-thread's and a gone one");
    assert_eq!(home_of(&listed[..2], "p1"), Some("w-fork"), "else a standing one of its own");
    assert_eq!(home_of(&listed, "p3"), None);
}

/// box1's IMAGE row, drawn with no host behind it: `image` is the image view's record, `copies` the places that hold one.
fn image_row_of(image: serde_json::Value, cx: &mut TestAppContext) -> &mut VisualTestContext {
    let data = Data { image: Some(serde_json::from_value(image).unwrap()), ..Data::default() };
    let place: PlaceView =
        serde_json::from_value(json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false, "buildsImages": true }))
            .unwrap();
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
    let row: Draw = Box::new(move |ctx, window, cx| match image_row(ctx, &place, window, cx) {
        Some(card) => card.render().into_any_element(),
        None => div().into_any_element(),
    });
    let (_, cx) = cx.add_window_view(move |_, _| Harness(row, data, runtime));
    cx.simulate_resize(gpui_kit::size(px(1000.), px(900.)));
    cx.run_until_parked();
    cx
}

fn sealed() -> serde_json::Value {
    json!({ "name": "base", "version": 1, "hash": "h", "recipeHash": "r", "logins": [], "noSecrets": true,
        "sealedAt": "2026-10-17T00:00:00Z", "sealedFrom": "here" })
}

/// Presses the row's own act: a copy there, with no host, is refused in a slot under the row.
fn press_the_rows_act(cx: &mut VisualTestContext) -> bool {
    let before = cx.debug_bounds("refusal");
    let press = cx.debug_bounds("button").expect("the row's press");
    cx.simulate_click(press.center(), Modifiers::none());
    cx.run_until_parked();
    // The refusal lands on the row's kept state, which tells no view; Settings draws every frame.
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds("refusal") != before
}

#[gpui_kit::test]
fn a_computer_with_no_image_says_what_to_do_and_offers_no_press(cx: &mut TestAppContext) {
    let cx = image_row_of(json!({ "image": null, "copies": [] }), cx);
    assert!(cx.debug_bounds("button").is_none(), "no press where there is nothing to copy");
}

#[gpui_kit::test]
fn a_computer_that_holds_no_copy_offers_to_copy_the_image(cx: &mut TestAppContext) {
    let cx = image_row_of(json!({ "image": sealed(), "copies": [] }), cx);
    assert!(cx.debug_bounds("button").is_some(), "Copy your image here");
    assert!(press_the_rows_act(cx), "a copy with no host behind it is refused in the slot under the row");
}

#[test]
fn the_words_for_no_image_are_the_protocols() {
    let words = include_str!("../../../../../../../../../packages/protocol/src/words/image.ts");
    let fix = "Run wsp doctor with the name of your cloud computer; it builds the default image there.";
    assert!(words.contains(&format!("export const NO_IMAGE_FIX = \"{fix}\";")));
    assert_eq!(words::NO_IMAGE, format!("No image yet. {fix}"));
}

#[test]
fn a_copy_that_stopped_says_why() {
    assert_eq!(words::copy_stopped_line(None), "the build stopped");
    assert_eq!(words::copy_build_of("sealed", None), None);
    assert_eq!(words::copy_build_of("snapshotting", None), Some(("copying your image: taking the snapshot".to_owned(), false)));
}
