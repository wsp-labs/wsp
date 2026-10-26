// SPDX-License-Identifier: AGPL-3.0-only
//! The changed-files tree drawn by GPUI's offscreen renderer in a 400 x 300 window at the test window's scale of 2,
//! three hundred files long so it scrolls: today's native bar at rest beside the rows, its lane kept clear of them, each row's
//! counts 16 px before its controls, and the rows 26.5 px apart. Then pierre's bar of the Files tree over a list: its
//! lane kept, its thumb only under the pointer. `TREE_PNG=<folder>` writes each capture. Without libtest's harness, as
//! `pixels.rs` explains.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use desktop_gpui::screens::pr_pane::changes::model::{tree_rows, TreeFile};
use desktop_gpui::screens::pr_pane::changes::tree::ChangedFilesTree;
use desktop_gpui::ui::{self, scrollbar, ActivePalette, Side, Still, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    div, point, prelude::*, px, size, uniform_list, AnyWindowHandle, Context, HeadlessAppContext, Hsla, Modifiers, MouseMoveEvent, Rgba,
    UniformListScrollHandle, Window,
};
use image::RgbaImage;

const W: f32 = 400.;
const H: f32 = 300.;
const SCALE: f32 = 2.;
/// The controls' width the test gives the tree: one 16 px box and the 8 px after it.
const CONTROLS: f32 = 24.;

struct Changes {
    scroll: UniformListScrollHandle,
}

impl Render for Changes {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let files: Vec<TreeFile> = (0..300).map(|i| TreeFile { path: format!("file-{i:03}.txt"), additions: 1, deletions: 2 }).collect();
        let rows = Rc::new(tree_rows(&files, &HashSet::new()));
        let tree = ChangedFilesTree::new("tree", rows, Rc::new(HashSet::new()), &files, px(W - 8.))
            .scroll(self.scroll.clone())
            .controls(px(CONTROLS), |_, _, cx| Some(div().size(px(16.)).bg(cx.palette().primary).into_any_element()));
        div().size_full().bg(cx.palette().background).child(tree)
    }
}

struct Plain {
    scroll: UniformListScrollHandle,
}

impl Render for Plain {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hovered = ui::motion::hovered("plain", window, cx);
        let bar = scrollbar::tree_bar("plain-bar", &self.scroll, *hovered.read(cx), window, cx);
        let list = uniform_list("plain-rows", 100, |range, _, _| range.map(|_| div().h(px(24.))).collect::<Vec<_>>())
            .size_full()
            .pr(scrollbar::WIDTH)
            .track_scroll(&self.scroll);
        // Clear of the window's corner, where the pointer rests until it moves.
        div()
            .id("plain")
            .relative()
            .mt(px(20.))
            .h(px(H - 20.))
            .w_full()
            .bg(cx.palette().background)
            .on_hover(ui::motion::track_hover(hovered))
            .child(list)
            .children(bar)
    }
}

fn app() -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_global(Still(true));
        ui::theme::set_theme("paper", cx);
    });
    cx
}

fn settle(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) {
    for _ in 0..4 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn shot(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, name: &str) -> RgbaImage {
    settle(cx, handle);
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    assert_eq!(image.width(), (W * SCALE) as u32, "the test window draws at a scale of 2");
    if let Some(dir) = std::env::var_os("TREE_PNG") {
        image.save(std::path::Path::new(&dir).join(format!("{name}.png"))).unwrap();
    }
    image
}

fn rgb(color: Hsla) -> [u8; 3] {
    let c: Rgba = color.into();
    [c.r, c.g, c.b].map(|v| (v * 255.).round() as u8)
}

/// `top` at its own alpha over `under`.
fn over(top: Hsla, under: Hsla) -> [u8; 3] {
    let (t, u): (Rgba, Rgba) = (top.into(), under.into());
    [(t.r, u.r), (t.g, u.g), (t.b, u.b)].map(|(t, u)| ((t * top.a + u * (1. - top.a)) * 255.).round() as u8)
}

fn near(image: &RgbaImage, x: u32, y: u32, want: [u8; 3]) -> bool {
    image.get_pixel(x, y).0.iter().zip(want).all(|(have, want)| have.abs_diff(want) <= 6)
}

/// Device columns in row `y` whose pixel is `want`.
fn columns(image: &RgbaImage, y: u32, want: [u8; 3]) -> Vec<u32> {
    (0..image.width()).filter(|&x| near(image, x, y, want)).collect()
}

/// Device columns in row `y`, in the right 20 px where the lane and the controls stand, whose pixel is `want`.
fn right_columns(image: &RgbaImage, y: u32, want: [u8; 3]) -> Vec<u32> {
    columns(image, y, want).into_iter().filter(|&x| x >= d(W - 20.)).collect()
}

fn d(css: f32) -> u32 {
    (css * SCALE).round() as u32
}

fn main() {
    the_changes_tree_draws_the_bar_at_rest_and_keeps_its_lane_clear();
    println!("test the_changes_tree_draws_the_bar_at_rest_and_keeps_its_lane_clear ... ok");
    the_files_trees_bar_keeps_its_lane_and_shows_its_thumb_only_under_the_pointer();
    println!("test the_files_trees_bar_keeps_its_lane_and_shows_its_thumb_only_under_the_pointer ... ok");
}

fn the_changes_tree_draws_the_bar_at_rest_and_keeps_its_lane_clear() {
    let mut cx = app();
    let handle: AnyWindowHandle = cx
        .open_window(size(px(W), px(H)), |window, cx| {
            let view = cx.new(|_| Changes { scroll: UniformListScrollHandle::new() });
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap()
        .into();
    let image = shot(&mut cx, handle, "changes");
    let (thumb, background, primary, minus) = cx.update(|cx| {
        let p = cx.palette();
        (rgb(p.scrollbar_thumb), rgb(p.background), rgb(p.primary), rgb(p.error_foreground))
    });

    // The thumb at rest, in `--app-scrollbar-thumb`, 6 px wide against the window's right edge, from the tree's top.
    let y = d(10.);
    let lane: Vec<u32> = right_columns(&image, y, thumb);
    assert_eq!(
        lane.first().zip(lane.last()).map(|(a, b)| (*a, *b)),
        Some((d(W - 6.), d(W) - 1)),
        "the thumb at rest fills the 6 px lane at the right edge"
    );
    assert!(near(&image, d(W - 3.), d(1.), thumb), "the thumb starts at the tree's top: {:?}", image.get_pixel(d(W - 3.), d(1.)));
    assert!(
        near(&image, d(W - 3.), d(15.), thumb) && near(&image, d(W - 3.), d(19.), background),
        "the thumb is 17 px long, the least today's draws"
    );

    // Each row's controls stop 8 px before the lane and the 4 px pad, and its counts 16 px before the controls.
    let rows: Vec<u32> = (d(1.)..d(224.)).filter(|&y| near(&image, d(W - 6. - 4. - 8.) - 2, y, primary)).collect();
    assert!(!rows.is_empty(), "no controls drawn");
    for &y in &rows {
        let boxes = columns(&image, y, primary);
        let right = *boxes.last().unwrap();
        assert_eq!(right, d(W - 6. - 4. - 8.) - 1, "the controls end 18 px short of the edge, the lane kept, at y {y}");
    }
    let mid = rows[rows.len() / 2];
    let (left, counts) =
        (columns(&image, mid, primary)[0], (mid - 6..mid + 6).filter_map(|y| columns(&image, y, minus).last().copied()).max());
    let gap = left as f32 - counts.expect("the -2 count drawn") as f32;
    // 32 device px and the `2`'s side bearing; a name 6 px too wide pushes the counts 12 device px on.
    assert!((32. ..=38.).contains(&gap), "the counts end 16 px (32 device) before the controls, found {gap} device px");

    // The rows 26.5 px apart: 24.5 px, between today's file and folder rows, and the 2 px under each.
    let tops: Vec<u32> = rows.iter().copied().filter(|y| !rows.contains(&(y - 1))).collect();
    assert!(tops.len() >= 7, "only {} rows in sight", tops.len());
    let pitch = (tops[6] - tops[0]) as f32 / 6. / SCALE;
    assert!((pitch - 26.5).abs() < 0.1, "rows stand {pitch} px apart, not 26.5");
}

fn the_files_trees_bar_keeps_its_lane_and_shows_its_thumb_only_under_the_pointer() {
    let mut cx = app();
    let handle: AnyWindowHandle = cx
        .open_window(size(px(W), px(H)), |window, cx| {
            let view = cx.new(|_| Plain { scroll: UniformListScrollHandle::new() });
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap()
        .into();
    let (ink, background) = cx.update(|cx| {
        let p = cx.palette();
        (over(p.tree_foreground.opacity(0.25), p.background), rgb(p.background))
    });
    let rest = shot(&mut cx, handle, "files-rest");
    assert!(right_columns(&rest, d(30.), ink).is_empty(), "no thumb at rest");
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_event(
            gpui_kit::PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(W / 2.), px(H / 2.)),
                pressed_button: None,
                modifiers: Modifiers::none(),
            }),
            cx,
        );
    })
    .unwrap();
    let hovered = shot(&mut cx, handle, "files-hovered");
    let lane = right_columns(&hovered, d(30.), ink);
    assert_eq!(
        lane.first().zip(lane.last()).map(|(a, b)| (*a, *b)),
        Some((d(W - 5.), d(W - 1.) - 1)),
        "under the pointer the thumb shows 4 px wide, 1 px in from each side of its lane"
    );
    assert!(near(&hovered, d(W - 6.) + 1, d(30.), background), "the lane's outer pixel stays clear");
}
