// SPDX-License-Identifier: AGPL-3.0-only
//! The shared code view drawn by GPUI's offscreen renderer over `tests/pr_pane/diff_sample.patch`, in Paper and
//! Graphite, stacked and split, in an 800 x 700 window at the test window's scale of 2. `DIFF_PNG=<folder>` writes each
//! capture as `<theme>-<layout>.png`. With `DIFF_WEB=<folder>` holding today's shots of the same patch
//! (`web-<theme>.png`, `web-<theme>-split.png`, taken by `tests/pr_pane/diff_web.sh`), it writes
//! `<theme>-<layout>-diff.png` and prints the share of pixels where any channel differs by more than 15 of 255.
//! Without libtest's harness, as `pixels.rs` explains.

use std::path::PathBuf;
use std::sync::Arc;

use desktop_gpui::screens::pr_pane::diff::{self, CodeView, FileHead, FileStatus, Layout};
use desktop_gpui::ui::{self, ActivePalette, Side, Still, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{div, prelude::*, px, size, AnyElement, AnyWindowHandle, App, Context, Entity, HeadlessAppContext, Window};
use image::RgbaImage;
use serde_json::json;

const W: f32 = 800.;
const H: f32 = 700.;
const PATCH: &str = include_str!("pr_pane/diff_sample.patch");

struct Pane {
    view: Entity<CodeView>,
}

impl Render for Pane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.palette().background).child(self.view.clone())
    }
}

/// A head drawn as today's Changes pane draws one: the chevron, the name in 12 px, the counts in 11 px mono at the end.
fn head(head: &FileHead, _: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.palette();
    let mono = gpui_kit::component::ActiveTheme::theme(cx).mono_font_family.clone();
    div()
        .flex()
        .items_center()
        .w_full()
        .gap(px(4.))
        .child(div().ml(px(-2.)).size(px(20.)).flex().items_center().justify_center().child(diff::file_chevron(head, cx)))
        .child(div().flex_1().min_w_0().ml(px(26.)).text_color(p.foreground).child(head.path.clone()))
        .child(
            div()
                .flex()
                .gap(px(6.))
                .font_family(mono)
                .text_size(px(11.))
                .when(head.deletions > 0, |d| d.child(div().text_color(p.destructive).child(format!("-{}", head.deletions))))
                .when(head.additions > 0, |d| d.child(div().text_color(p.success).child(format!("+{}", head.additions)))),
        )
        .into_any_element()
}

/// The sample patch as a git.diff reply reads it.
pub fn sample() -> diff::DiffModel {
    let files: Vec<_> = PATCH
        .split_inclusive('\n')
        .fold(Vec::<String>::new(), |mut files, line| {
            if line.starts_with("diff --git ") || files.is_empty() {
                files.push(String::new());
            }
            files.last_mut().unwrap().push_str(line);
            files
        })
        .into_iter()
        .map(|patch| {
            let path = patch.lines().next().unwrap().rsplit(" b/").next().unwrap().to_owned();
            json!({"path": path, "kind": "modified", "patch": patch})
        })
        .collect();
    let reply = json!({"base": null, "truncated": false, "moved": [], "files": files});
    diff::read_reply(reply.as_object().unwrap()).unwrap()
}

fn shoot(theme: &str, layout: Layout) -> RgbaImage {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_global(Still(true));
        ui::theme::set_theme(theme, cx);
    });
    let model = sample();
    assert_eq!(model.files.iter().map(|f| f.status).collect::<Vec<_>>(), [FileStatus::Added, FileStatus::Modified]);
    let handle: AnyWindowHandle = cx
        .open_window(size(px(W), px(H)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = CodeView::new(cx);
                view.on_head(head);
                view.set_layout(layout, cx);
                view.set_files(&model.files, cx);
                view
            });
            let pane = cx.new(|_| Pane { view });
            cx.new(|cx| Root::new(pane, window, cx))
        })
        .unwrap()
        .into();
    for _ in 0..4 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    cx.capture_screenshot(handle).expect("offscreen rendering must be available")
}

/// The share of pixels where any channel differs by more than 15 of 255, and an image of where: red over today's
/// pixels, dimmed.
fn compare(web: &RgbaImage, ours: &RgbaImage) -> (f64, RgbaImage) {
    let (w, h) = (web.width().min(ours.width()), web.height().min(ours.height()));
    let mut out = RgbaImage::new(w, h);
    let mut differ = 0u64;
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (web.get_pixel(x, y), ours.get_pixel(x, y));
            let off = (0..3).any(|c| a[c].abs_diff(b[c]) > 15);
            differ += u64::from(off);
            let dim = |v: u8| 128 + v / 2;
            out.put_pixel(x, y, if off { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([dim(a[0]), dim(a[1]), dim(a[2]), 255]) });
        }
    }
    (100. * differ as f64 / (u64::from(w) * u64::from(h)).max(1) as f64, out)
}

fn main() {
    let out = std::env::var_os("DIFF_PNG").map(PathBuf::from);
    let web = std::env::var_os("DIFF_WEB").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    for theme in ["paper", "graphite"] {
        for (layout, word) in [(Layout::Stacked, "stacked"), (Layout::Split, "split")] {
            let image = shoot(theme, layout);
            assert_eq!((image.width(), image.height()), (2 * W as u32, 2 * H as u32), "{theme} {word} is drawn at 2x");
            if let Some(out) = &out {
                image.save(out.join(format!("{theme}-{word}.png"))).unwrap();
            }
            let name = if layout == Layout::Split { format!("web-{theme}-split.png") } else { format!("web-{theme}.png") };
            if let Some(shot) = web.as_ref().and_then(|web| image::open(web.join(&name)).ok()) {
                let (share, picture) = compare(&shot.to_rgba8(), &image);
                println!("{theme:<9} {word:<8} {share:6.2}% of pixels differ");
                if let Some(out) = &out {
                    picture.save(out.join(format!("{theme}-{word}-diff.png"))).unwrap();
                }
            }
        }
        println!("test the_sample_draws_in_{theme} ... ok");
    }
}
