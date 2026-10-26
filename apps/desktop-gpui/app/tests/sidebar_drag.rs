// SPDX-License-Identifier: AGPL-3.0-only
//! A root tile dragged in the window main.rs builds: the sidebar drawn as the shell draws it, cached in its column
//! inside gpui-kit's Root, where a pressed move on a tile starts the drag and the list shows where it would land.

use std::sync::Arc;

use desktop_gpui::screens::app_shell::vibrancy;
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::host::{Change, Snapshot};
use desktop_gpui::ui::{self, Side};
use gpui_kit::{point, px, size, AppContext, Bounds, Entity, Modifiers, MouseButton, Pixels, TestAppContext, VisualTestContext};
use serde_json::json;
use wsp_host_client::Env;

/// The shell hostless in a 1280 by 800 window, built as main.rs builds it, on a HOME of the test's own.
fn open(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, tempfile::TempDir) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let dir = tempfile::tempdir().unwrap();
    let env: Env = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let state = dir.path().join("state.json");
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        vibrancy::clear_root(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    (shell.unwrap(), cx, dir)
}

/// Three read top-level threads t0, t1, t2 in workspace w1, newest first, and t0 open.
fn seed(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() * 1000.;
    let sessions: Vec<serde_json::Value> = (0..3)
        .map(|i| {
            json!({
                "id": format!("r-t{i}"), "threadId": format!("t{i}"), "workspaceId": "w1", "harness": "claude",
                "status": "completed", "prompt": format!("thread t{i}"), "project": { "id": "p1", "name": "repo" },
                "startedAt": now - 120_000. - i as f64 * 1_000., "endedAt": now - 60_000., "readAt": now - 60_000.,
            })
        })
        .collect();
    let snapshot = Snapshot {
        sessions: desktop_gpui::sidebar::shared::rows_of(sessions),
        workspaces: vec![json!({ "id": "w1", "name": "w1", "kind": "local", "project": { "id": "p1" } })],
        projects: vec![json!({ "id": "p1", "name": "repo", "path": "/repo", "computer": "here" })],
        places: vec![json!({ "id": "here", "kind": "computer", "name": "boat" })],
        preferences: json!({}),
        ..Snapshot::default()
    };
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot)), cx));
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "t0".into() }, window, cx));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn tile(key: &str, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    let selector: &'static str = Box::leak(format!("tile-{key}").into_boxed_str());
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("tile-{key} is drawn"))
}

/// Catches: a pressed move on a root's tile in the app's own window that starts no drag, or a drag that marks no
/// place to land, as today's dims the tile and marks the edge under the pointer.
#[gpui_kit::test]
fn a_tile_dragged_in_the_app_window_starts_a_drag_and_marks_where_it_lands(cx: &mut TestAppContext) {
    let (shell, cx, _home) = open(cx);
    seed(&shell, cx);
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    let t0 = tile("t0", cx);
    cx.simulate_mouse_move(t0.center(), None, Modifiers::none());
    cx.simulate_mouse_down(t0.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(t0.center() + point(px(0.), px(10.)), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.update(|_, cx| cx.has_active_drag()), "a pressed move on the tile starts a drag");
    // The heads that stand as the drag starts push the rows under them down, so t2 is read again.
    let t2 = tile("t2", cx);
    let to = point(t2.center().x, t2.top() + t2.size.height * 0.75);
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(sidebar.read_with(cx, |s, _| s.dragging().map(str::to_owned)), Some("t0".to_owned()), "the sidebar knows what it carries");
    assert!(cx.debug_bounds("drop-edge-t2-after").is_some(), "the edge after t2 is marked");
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(!cx.update(|_, cx| cx.has_active_drag()), "the drop ends the drag");
    assert_eq!(sidebar.read_with(cx, |s, _| s.dragging().map(str::to_owned)), None, "and the sidebar carries nothing");
}
