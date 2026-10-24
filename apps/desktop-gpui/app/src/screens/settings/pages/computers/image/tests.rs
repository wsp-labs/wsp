// SPDX-License-Identifier: AGPL-3.0-only

use super::build::{elapsed_line, provider_key, Elapsed};
use super::*;
use gpui_kit::Entity;

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

fn draw(height: f32, draw: Draw, cx: &mut TestAppContext) -> &mut VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
    let (_, cx) = cx.add_window_view(move |_, _| Harness(draw, Data::default(), runtime));
    cx.simulate_resize(gpui_kit::size(px(1000.), px(height)));
    cx.run_until_parked();
    cx
}

fn solari() -> PlaceView {
    serde_json::from_value(json!({ "id": "solari", "kind": "provider", "name": "solari", "default": false })).unwrap()
}

fn job(phase: &str, rows: serde_json::Value) -> InitJob {
    serde_json::from_value(json!({ "id": "j", "road": "manual", "phase": phase, "keys": {}, "step": 0, "stoppable": true,
        "screens": [], "rows": rows, "progress": { "done": 0, "total": 0 }, "log": [], "place": { "id": "solari", "name": "solari" } }))
    .unwrap()
}

/// The build box of `job`, its kept state the test's.
fn build_box(job: InitJob, kept: Entity<Kept>) -> Draw {
    Box::new(move |ctx, window, cx| image_build(ctx, &solari(), &job, &kept, window, cx))
}

fn lines(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("pnpm: \u{1b}[32mstep {i}\u{1b}[0m done")).collect()
}

fn stages() -> InitJob {
    job(
        "failed",
        json!([
            { "id": "stage/creating", "kind": "stage", "label": "Creating the machine", "state": "done", "lines": lines(10) },
            { "id": "stage/deploying-daemon", "kind": "stage", "label": "Installing the base tools", "state": "failed", "lines": lines(10),
              "detail": "the base tools did not install" },
        ]),
    )
}

#[gpui_kit::test]
fn a_failed_stage_stands_open_on_its_lines_and_a_done_one_opens_on_a_click(cx: &mut TestAppContext) {
    let kept = cx.new(|_| Kept::default());
    let cx = draw(800., build_box(stages(), kept), cx);
    assert!(cx.debug_bounds("stage-line-danger").is_some(), "the failed stage is open, its last line in the danger ink");
    assert_eq!(cx.debug_bounds("stage-lines").unwrap().size.height, px(8. * 20. + 16.), "eight lines at most");
    let failed = cx.debug_bounds("build-row-stage/deploying-daemon").unwrap().center();
    cx.simulate_click(failed, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("stage-lines").is_none(), "a click folds the failed stage, and the done one stands folded");
    let done = cx.debug_bounds("build-row-stage/creating").unwrap().center();
    cx.simulate_click(done, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("stage-lines").is_some() && cx.debug_bounds("stage-line-danger").is_none(), "the done stage opens");
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

#[gpui_kit::test]
fn a_short_window_shows_six_lines(cx: &mut TestAppContext) {
    let kept = cx.new(|_| Kept::default());
    let cx = draw(650., build_box(stages(), kept), cx);
    assert_eq!(cx.debug_bounds("stage-lines").unwrap().size.height, px(6. * 20. + 16.));
}

#[gpui_kit::test]
fn a_running_stage_stands_on_its_newest_line(cx: &mut TestAppContext) {
    let kept = cx.new(|_| Kept::default());
    let running =
        job("failed", json!([{ "id": "stage/creating", "kind": "stage", "label": "Creating", "state": "running", "lines": lines(20) }]));
    let cx = draw(800., build_box(running, kept), cx);
    cx.run_until_parked();
    let block = cx.debug_bounds("stage-lines").unwrap();
    let scroller = block.bottom() - px(8.);
    let newest = cx.debug_bounds("stage-line-newest").expect("the newest line");
    assert!(newest.bottom() <= scroller + px(0.5) && newest.top() >= block.top(), "{newest:?} in {block:?}");
    let bar = cx.debug_bounds("scrollbar").expect("the overflowing lines draw their bar");
    assert_eq!((block.right() - bar.right(), bar.size.width), (px(1.), px(6.)), "the shared bar at the block's right edge");
    assert_eq!((bar.top() - (block.top() + px(8.)), scroller - bar.bottom()), (px(4.), px(4.)), "my-1 in the lines' scroller");
}

#[gpui_kit::test]
fn the_sign_ins_names_start_on_one_line_after_the_mark_column(cx: &mut TestAppContext) {
    let kept = cx.new(|_| Kept::default());
    let signs = job(
        "failed",
        json!([
            { "id": "signin/claude", "kind": "sign-in", "tool": "claude", "label": "Claude Code", "state": "failed" },
            { "id": "signin/foo", "kind": "sign-in", "tool": "foo", "label": "Foo", "state": "done" },
        ]),
    );
    let cx = draw(800., build_box(signs, kept), cx);
    let a = cx.debug_bounds("sign-in-name-signin/claude").expect("the failed sign-in stands open under its stage");
    let b = cx.debug_bounds("sign-in-name-signin/foo").unwrap();
    let row = cx.debug_bounds("build-row-stage/sign-ins").unwrap();
    assert_eq!(a.origin.x, b.origin.x, "a tool with no mark keeps the column");
    assert_eq!(a.origin.x, row.origin.x + px(40. + 18. + 12.), "pl-10, the 18 px mark and the gap");
}

#[gpui_kit::test]
fn every_link_but_close_is_held_while_a_press_is_out(cx: &mut TestAppContext) {
    let kept = cx.new(|_| Kept { asking: true, step_busy: true, ..Kept::default() });
    let building = job("building", json!([{ "id": "stage/creating", "kind": "stage", "label": "Creating", "state": "running" }]));
    let cx = draw(800., build_box(building, kept.clone()), cx);
    let keep = cx.debug_bounds("build-keep").expect("Keep building").center();
    cx.simulate_click(keep, Modifiers::none());
    cx.run_until_parked();
    assert!(kept.read_with(cx, |k, _| k.asking), "Keep building took no press while the cancel was out");
}

#[gpui_kit::test]
fn enter_saves_the_key_and_a_refused_key_stays_in_its_field(cx: &mut TestAppContext) {
    let kept = cx.new(|_| Kept::default());
    let held = kept.clone();
    let words = super::super::words::provider_words("solari").unwrap();
    let cx = draw(800., Box::new(move |ctx, window, cx| provider_key(ctx, &solari(), words, &held, window, cx)), cx);
    let field = cx.debug_bounds("cloud-key").unwrap().center();
    cx.simulate_click(field, Modifiers::none());
    cx.simulate_input("sk-test-key");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(kept.read_with(cx, |k, _| k.step_refusal.clone()).as_deref(), Some("No host answered."), "Enter saved");
    kept.update(cx, |k, _| k.step_refusal = None);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(kept.read_with(cx, |k, _| k.step_refusal.is_some()), "the refused key was still there to send again");
}

#[gpui_kit::test]
fn elapsed_ticks_on_its_own_once_a_second(cx: &mut TestAppContext) {
    assert_eq!(elapsed_line(5_400.), "5s");
    assert_eq!(elapsed_line(-20.), "0s");
    assert_eq!(elapsed_line(65_000.), crate::screens::settings::words::fmt_duration(65_000.));
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let base = now_ms() as f64;
    cx.update(|cx| cx.set_global(crate::ui::status::FrozenNow(base)));
    let (view, cx) = cx.add_window_view(|_, _| Elapsed::new(base - 5_000.));
    cx.run_until_parked();
    let first = view.read_with(cx, |e, _| e.drawn);
    cx.executor().advance_clock(std::time::Duration::from_millis(1_000));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |e, _| e.drawn), first, "a tick whose second has not come yet draws nothing");
    cx.update(|_, cx| cx.set_global(crate::ui::status::FrozenNow(base + 1_000.)));
    cx.executor().advance_clock(std::time::Duration::from_millis(1_000));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |e, _| e.drawn), first + 1, "one draw a second");
    assert_eq!(view.read_with(cx, |e, _| e.line().to_owned()), "6s");
}

/// box1's IMAGE row with an image to copy there and a build that failed on `on`, drawn with no host behind it.
fn image_row_of<'a>(on: &str, cx: &'a mut TestAppContext) -> &'a mut VisualTestContext {
    let image = json!({ "image": { "name": "base", "version": 1, "hash": "h", "recipeHash": "r", "logins": [],
        "noSecrets": true, "sealedAt": "2026-10-17T00:00:00Z", "sealedFrom": "here" }, "copies": [], "projects": [] });
    let job = json!({ "id": "j", "road": "manual", "phase": "failed", "keys": {}, "step": 0, "stoppable": false, "screens": [],
        "rows": [{ "id": "stage/creating", "kind": "stage", "label": "Creating the machine", "state": "failed" }],
        "progress": { "done": 0, "total": 1 }, "log": [], "place": { "id": on, "name": on } });
    let setup = json!({ "keys": {}, "home": "/h", "agents": [], "pricing": null, "job": job });
    let data = Data {
        image: Some(serde_json::from_value(image).unwrap()),
        setup: Some(serde_json::from_value(setup).unwrap()),
        ..Data::default()
    };
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

/// Presses where the row's own act stands, at its right end: a copy there, with no host, is refused in a slot under
/// the row, which stands the slot of a build box under it further down.
fn press_the_rows_act(cx: &mut VisualTestContext) -> bool {
    let grid = cx.debug_bounds("grid").expect("the IMAGE list");
    let before = cx.debug_bounds("refusal");
    cx.simulate_click(gpui_kit::point(grid.right() - px(40.), grid.bottom() - px(24.)), Modifiers::none());
    cx.run_until_parked();
    // The refusal lands on the row's kept state, which tells no view; Settings draws every frame.
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds("refusal") != before
}

#[gpui_kit::test]
fn a_rows_press_stands_down_beside_its_open_build_box_until_close(cx: &mut TestAppContext) {
    let cx = image_row_of("box1", cx);
    assert!(!press_the_rows_act(cx), "the failed build's box answers what to do now, so the row has no press");
    // Close, the box's one link, is the last button drawn.
    let close = cx.debug_bounds("button").expect("the box's Close");
    cx.simulate_click(close.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(press_the_rows_act(cx), "with the box put away, Try again stands on the row again");
}

#[test]
fn the_version_head_stands_where_a_row_has_one() {
    assert!(versioned(&["0.3.1"]));
    assert!(!versioned(&["", ""]));
}

#[test]
fn a_build_is_on_its_stretch() {
    assert!(job_building("signing-in"));
    assert!(!job_building("answering"));
    assert!(job_over("cancelled"));
    assert_eq!(words::step_now(Some("Creating the machine"), 2, 7), "Creating the machine, 2 of 7");
    assert_eq!(words::step_now(None, 2, 7), "2 of 7 steps done");
}
