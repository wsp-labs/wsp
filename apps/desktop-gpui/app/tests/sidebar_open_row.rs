// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar drawn by GPUI's offscreen renderer with a failed sub-thread, which stands in Needs you and again under
//! its lead: the pointer on one row lights that row alone, and once the thread is open both rows keep the selected
//! fill under the pointer, as `data-[active=true]:bg-sidebar-row-selected` outranks `hover:` in today's
//! `SidebarMenuButton`. `SIDEBAR_PNG=<folder>` writes each capture. Without libtest's harness, as `pixels.rs` explains.

use std::sync::Arc;

use desktop_gpui::screens::app_shell::assets::AppAssets;
use desktop_gpui::sidebar::host::{Change, Snapshot};
use desktop_gpui::sidebar::model::Item;
use desktop_gpui::sidebar::rows::Drawn;
use desktop_gpui::sidebar::Sidebar;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{point, px, size, AnyWindowHandle, AppContext, Entity, HeadlessAppContext, Modifiers, MouseMoveEvent, Pixels, Point};
use image::RgbaImage;
use serde_json::json;

const W: f32 = 1280.;
const H: f32 = 800.;

fn snapshot(now: f64) -> Snapshot {
    let row = |id: &str, title: &str, status: &str, parent: Option<&str>, read: bool| {
        let mut r = json!({
            "id": format!("r-{id}"), "threadId": id, "workspaceId": "w0", "harness": "claude", "status": status, "prompt": title,
            "project": { "id": "p1", "name": "repo" }, "startedAt": now - 120_000., "endedAt": now - 60_000.,
        });
        if read {
            r["readAt"] = json!(now - 30_000.);
        }
        if let Some(parent) = parent {
            r["parentThreadId"] = json!(parent);
            r["rootThreadId"] = json!(parent);
        }
        r
    };
    Snapshot {
        sessions: desktop_gpui::sidebar::shared::rows_of(vec![
            row("lead", "Coordinator", "completed", None, true),
            row("sub", "Review the panes", "failed", Some("lead"), false),
        ]),
        workspaces: vec![json!({ "id": "w0", "name": "w0", "kind": "local", "project": { "id": "p1" } })],
        projects: vec![json!({ "id": "p1", "name": "repo", "path": "/repo", "computer": "here" })],
        places: vec![json!({ "id": "here", "kind": "computer", "name": "boat" })],
        preferences: json!({}),
        ..Snapshot::default()
    }
}

fn settle(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) {
    for _ in 0..4 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn shot(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, name: &str) -> (RgbaImage, f32) {
    settle(cx, handle);
    let scale = cx.update_window(handle, |_, window, _| window.scale_factor()).unwrap();
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    if let Some(dir) = std::env::var_os("SIDEBAR_PNG") {
        image.save(std::path::Path::new(&dir).join(format!("{name}.png"))).unwrap();
    }
    (image, scale)
}

fn pixel(image: &RgbaImage, scale: f32, at: Point<Pixels>) -> [u8; 3] {
    let p = image.get_pixel((f32::from(at.x) * scale).round() as u32, (f32::from(at.y) * scale).round() as u32).0;
    [p[0], p[1], p[2]]
}

/// The sub-thread's two tiles, the Needs you one first: for each, a point in the tile's own left padding and its
/// middle, where only the tile's fill is drawn.
fn tiles(sidebar: &Entity<Sidebar>, cx: &mut HeadlessAppContext) -> Vec<(Point<Pixels>, Point<Pixels>)> {
    cx.update(|cx| {
        let s = sidebar.read(cx);
        s.drawn()
            .iter()
            .enumerate()
            .filter_map(|(ix, d)| {
                let Drawn::Line(line) = d else { return None };
                let line = &s.lines()[*line];
                let Item::Tile(t) = &line.item else { return None };
                if s.threads()[t.thread].key != "sub" {
                    return None;
                }
                let b = s.list_state().bounds_for_item(ix).expect("the tile is in sight");
                let left = b.left() + px(8. + 21. * line.depth as f32);
                let mid = b.bottom() - px(26.);
                Some((point(left + px(3.), mid), point(left + px(40.), mid)))
            })
            .collect()
    })
}

fn move_to(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, at: Point<Pixels>) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_event(
            gpui_kit::PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::none() }),
            cx,
        );
    })
    .unwrap();
}

fn main() {
    the_pointer_lights_its_row_alone_and_the_open_rows_keep_their_fill();
    println!("test the_pointer_lights_its_row_alone_and_the_open_rows_keep_their_fill ... ok");
}

/// Catches: a hover fill that replaces the open row's selected fill, so the row under the pointer reads dimmer than
/// the thread's other row and the two look like one hover drawn twice; and a hover lighting the thread's other row.
fn the_pointer_lights_its_row_alone_and_the_open_rows_keep_their_fill() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Dark, cx);
        ui::theme::set_theme("pitch", cx);
    });
    let sidebar = cx.update(|cx| cx.new(Sidebar::new));
    let held = sidebar.clone();
    let handle: AnyWindowHandle =
        cx.open_window(size(px(W), px(H)), move |window, cx| cx.new(|cx| Root::new(held, window, cx))).unwrap().into();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64;
    cx.update(|cx| sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(now))), cx)));
    settle(&mut cx, handle);
    let rows = tiles(&sidebar, &mut cx);
    assert_eq!(rows.len(), 2, "the failed sub-thread stands in Needs you and under its lead");
    let [(inbox, inbox_body), (tree, _)] = [rows[0], rows[1]];
    let away = point(px(W - 10.), px(H - 10.));

    move_to(&mut cx, handle, away);
    let (rest, scale) = shot(&mut cx, handle, "rest");
    let bare = pixel(&rest, scale, inbox);
    assert_eq!(pixel(&rest, scale, tree), bare, "no row is lit at rest");

    move_to(&mut cx, handle, inbox_body);
    let (hovered, _) = shot(&mut cx, handle, "hover-inbox");
    assert_ne!(pixel(&hovered, scale, inbox), bare, "the row under the pointer is lit");
    assert_eq!(pixel(&hovered, scale, tree), bare, "the thread's other row stays dark");

    move_to(&mut cx, handle, away);
    cx.update(|cx| sidebar.update(cx, |s, cx| _ = s.mark("sub", cx)));
    let (open, _) = shot(&mut cx, handle, "open");
    let selected = pixel(&open, scale, tree);
    assert_ne!(selected, bare, "the open thread's row is lit");
    assert_eq!(pixel(&open, scale, inbox), selected, "both of the open thread's rows draw the selected fill");

    move_to(&mut cx, handle, inbox_body);
    let (open_hovered, _) = shot(&mut cx, handle, "open-hover-inbox");
    assert_eq!(pixel(&open_hovered, scale, inbox), selected, "the open row under the pointer keeps the selected fill");
    assert_eq!(pixel(&open_hovered, scale, tree), selected, "the open thread's other row keeps it too");
}
