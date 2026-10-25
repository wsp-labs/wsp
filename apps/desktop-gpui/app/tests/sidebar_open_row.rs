// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar drawn by GPUI's offscreen renderer with a failed sub-thread, which stands in Needs you and again under
//! its lead: the pointer on one row lights that row alone, and once the thread is open both rows keep the selected
//! fill under the pointer, as `data-[active=true]:bg-sidebar-row-selected` outranks `hover:` in today's
//! `SidebarMenuButton`. Then the foot's update card in each stage it shows (`UpdateCard.tsx`): out with Get,
//! Downloading, and ready with Restart over two running threads. `SIDEBAR_PNG=<folder>` writes each capture;
//! `UPDATE_CARD_WEB=<folder>` holding `update_card/web-shot.mjs`'s shots compares the card with them pixel by pixel.
//! Without libtest's harness, as `pixels.rs` explains.

use std::sync::Arc;

use desktop_gpui::screens::app_shell::assets::AppAssets;
use desktop_gpui::sidebar::host::{Change, Snapshot};
use desktop_gpui::sidebar::model::Item;
use desktop_gpui::sidebar::rows::Drawn;
use desktop_gpui::sidebar::Sidebar;
use desktop_gpui::ui::{self, Side};
use desktop_gpui::update::stage::{Installed, Kind};
use desktop_gpui::update::{self, Download, Phase};
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
    the_update_card_draws_each_stage();
    println!("test the_update_card_draws_each_stage ... ok");
}

/// The release the card offers, above any version this app is built as.
const RELEASE: &str = "99.0.0";

/// The sidebar on its own surface, as the shell lays it.
struct Surface(Entity<Sidebar>);

impl gpui_kit::Render for Surface {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use desktop_gpui::ui::ActivePalette as _;
        use gpui_kit::{ParentElement as _, Styled as _};
        gpui_kit::div().size_full().bg(cx.palette().sidebar).child(self.0.clone())
    }
}

/// The card in one stage, cut out of a 256 px sidebar at the window's scale: `download` is the shell's download of
/// the release, and `installed` whether the app replaces itself.
fn card_shot(installed: bool, download: Option<Phase>, running: usize) -> (RgbaImage, Vec<String>) {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("wsp.AppImage"), b"\x7fELF\x02\x01\x01\x00AI\x02").unwrap();
    let family = cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        ui::theme::set_theme("paper", cx);
        let app = installed.then(|| Installed { path: dir.path().join("wsp.AppImage"), kind: Kind::AppImage });
        update::init(app, &dir.path().join("state.json"), dir.path(), cx);
        let download = download.map(|phase| Download { version: RELEASE.into(), phase });
        gpui_kit::BorrowAppContext::update_global::<update::Updates, _>(cx, |u, _| u.download = download);
        gpui_kit::component::Theme::global(cx).font_family.to_string()
    });
    let sidebar = cx.update(|cx| cx.new(Sidebar::new));
    let held = sidebar.clone();
    let handle: AnyWindowHandle = cx
        .open_window(size(px(256.), px(480.)), move |window, cx| {
            let surface = cx.new(|_| Surface(held));
            cx.new(|cx| Root::new(surface, window, cx))
        })
        .unwrap()
        .into();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64;
    let mut snap = snapshot(now);
    let busy = |i: usize| {
        json!({ "id": format!("r-busy{i}"), "threadId": format!("busy{i}"), "workspaceId": "w0", "harness": "claude",
            "status": "running", "prompt": "Working", "project": { "id": "p1", "name": "repo" }, "startedAt": now - 5_000. })
    };
    snap.sessions.extend(desktop_gpui::sidebar::shared::rows_of((0..running).map(busy).collect()));
    snap.release = Some(json!({ "state": "read", "latest": { "version": RELEASE, "url": "https://example.invalid/notes" } }));
    cx.update(|cx| sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snap)), cx)));
    settle(&mut cx, handle);
    let (scale, card) = cx.update_window(handle, |_, window, _| (window.scale_factor(), window.find("update-card").bounds())).unwrap();
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    let at = |v: Pixels| (f32::from(v) * scale).round() as u32;
    let cut = image::imageops::crop_imm(&image, at(card.origin.x), at(card.origin.y), at(card.size.width), at(card.size.height));
    (cut.to_image(), vec![family])
}

/// The share of pixels where any channel differs by more than 15 of 255 over the box both share, and an image of
/// where (red over the web's pixels, dimmed) beside the pair.
fn card_diff(web: &RgbaImage, gpui: &RgbaImage) -> (f64, RgbaImage) {
    let (w, h) = (web.width().min(gpui.width()), web.height().min(gpui.height()));
    let mut out = RgbaImage::new(3 * w + 16, h);
    let mut differ = 0u64;
    for j in 0..h {
        for i in 0..w {
            let (a, b) = (*web.get_pixel(i, j), *gpui.get_pixel(i, j));
            let off = (0..3).any(|c| a[c].abs_diff(b[c]) > 15);
            differ += u64::from(off);
            let dim = |v: u8| 128 + v / 2;
            out.put_pixel(i, j, a);
            out.put_pixel(w + 8 + i, j, b);
            out.put_pixel(
                2 * w + 16 + i,
                j,
                if off { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([dim(a[0]), dim(a[1]), dim(a[2]), 255]) },
            );
        }
    }
    (100. * differ as f64 / (u64::from(w) * u64::from(h)).max(1) as f64, out)
}

/// Catches: a card that does not stand for a release the app is behind, or that is cut or sized off the web's 240 by
/// 72 px (104 with the running threads' two lines) at 2x.
fn the_update_card_draws_each_stage() {
    let out = std::env::var_os("SIDEBAR_PNG").map(std::path::PathBuf::from);
    let web = std::env::var_os("UPDATE_CARD_WEB").map(std::path::PathBuf::from);
    for (name, installed, download, running, height) in
        [("get", false, None, 0, 72), ("downloading", true, Some(Phase::Downloading), 0, 72), ("restart", true, Some(Phase::Kept), 2, 104)]
    {
        let (shot, faces) = card_shot(installed, download, running);
        assert_eq!((shot.width(), shot.height()), (480, 2 * height), "the {name} card is 240 px wide and {height} px tall at 2x");
        if let Some(out) = &out {
            shot.save(out.join(format!("update-{name}-gpui.png"))).unwrap();
        }
        let Some(web) = &web else { continue };
        let theirs = image::open(web.join(format!("update-{name}-web.png"))).expect("the web's shot").to_rgba8();
        let (share, picture) = card_diff(&theirs, &shot);
        let sizes = format!("web {}x{}, gpui {}x{}", theirs.width(), theirs.height(), shot.width(), shot.height());
        println!("update card {name:<12} {share:6.2}% differ ({sizes}; gpui drew with {faces:?})");
        if let Some(out) = &out {
            picture.save(out.join(format!("update-{name}-diff.png"))).unwrap();
        }
    }
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
