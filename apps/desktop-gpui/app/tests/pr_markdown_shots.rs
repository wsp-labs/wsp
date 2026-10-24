// SPDX-License-Identifier: AGPL-3.0-only
//! Markdown in the Pull request pane's body and in the Files pane's preview, drawn by GPUI's offscreen renderer at the
//! test window's scale of 2 and held to the places Chromium draws the same markdown with today's CSS (`.chat-markdown
//! .pr-md` in `apps/web/src/index.css`, `PROSE` in `FileMarkdownPreview.tsx`): an inline code span's tint behind its
//! letters, list markers as Blink's outside discs in the item's ink, the list's indent and the gaps between blocks.
//! `MD_PNG=<folder>` writes each capture. Without libtest's harness, as `pixels.rs` explains.

use std::path::PathBuf;
use std::sync::Arc;

use desktop_gpui::screens::pr_pane::files::markdown as file_markdown;
use desktop_gpui::screens::pr_pane::pull_request::conversation;
use desktop_gpui::ui::{self, ActivePalette, Side, Still, UiAssets};
use gpui_kit::component::text::TextViewState;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{div, prelude::*, px, size, AnyWindowHandle, Context, Entity, HeadlessAppContext, Window};
use image::RgbaImage;

const W: f32 = 560.;
const H: f32 = 360.;

pub const BODY: &str = "The pane now reads the checks on the head and names the failing ones in its title.\n\n- `Pane::read` counts the failed checks\n- the words for a blocked merge live in `words.rs`\n\nCloses #40.\n";
pub const FILE: &str = "# Notes\n\nSome *markdown* and a [link](https://example.com).\n\n- one\n- two\n";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Body,
    File,
}

struct Page {
    kind: Kind,
    state: Entity<TextViewState>,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = div().size_full().bg(cx.palette().background);
        match self.kind {
            Kind::Body => page.p(px(20.)).child(div().w(px(480.)).child(conversation::markdown(
                "body",
                &self.state,
                conversation::Md::Description,
                cx,
            ))),
            Kind::File => page.child(file_markdown::preview(&self.state, cx)),
        }
    }
}

fn shoot(kind: Kind) -> RgbaImage {
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
    let text = match kind {
        Kind::Body => BODY,
        Kind::File => FILE,
    };
    let handle: AnyWindowHandle = cx
        .open_window(size(px(W), px(H)), |window, cx| {
            let state = cx.new(|cx| TextViewState::markdown(text, cx));
            let page = cx.new(|_| Page { kind, state });
            cx.new(|cx| Root::new(page, window, cx))
        })
        .unwrap()
        .into();
    for _ in 0..4 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    cx.capture_screenshot(handle).expect("offscreen rendering must be available")
}

/// Device rows `y0..y1` holding ink darker than 140 of 255 in columns `x0..x1`, as runs of consecutive rows.
fn ink_rows(image: &RgbaImage, (x0, x1): (u32, u32), (y0, y1): (u32, u32)) -> Vec<(u32, u32)> {
    let inked = |y: u32| (x0..x1).any(|x| image.get_pixel(x, y).0[..3].iter().map(|&c| u32::from(c)).sum::<u32>() < 3 * 140);
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for y in (y0..y1).filter(|&y| inked(y)) {
        match runs.last_mut() {
            Some(run) if run.1 + 1 == y => run.1 = y,
            _ => runs.push((y, y)),
        }
    }
    runs
}

/// The box of the pixels in `x0..x1` x `y0..y1` that `want` holds for.
fn bbox(image: &RgbaImage, (x0, x1): (u32, u32), (y0, y1): (u32, u32), want: impl Fn([u8; 4]) -> bool) -> Option<(u32, u32, u32, u32)> {
    let hits: Vec<(u32, u32)> =
        (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| want(image.get_pixel(x, y).0)).collect();
    let xs = hits.iter().map(|h| h.0);
    let ys = hits.iter().map(|h| h.1);
    Some((xs.clone().min()?, ys.clone().min()?, xs.max()?, ys.max()?))
}

/// Whether `have` and `want` stand within `by` device pixels of each other, end for end.
fn near(have: &[(u32, u32)], want: &[(u32, u32)], by: u32) -> bool {
    have.len() == want.len() && have.iter().zip(want).all(|(a, b)| a.0.abs_diff(b.0) <= by && a.1.abs_diff(b.1) <= by)
}

fn dark(p: [u8; 4]) -> bool {
    p[..3].iter().all(|&c| c < 80)
}

/// Read off Chromium drawing the same two documents with today's CSS at device scale 2, in the same places: the text
/// lines' ink rows, each list marker's ink box, and the code span's tint.
const CHROME_BODY_LINES: [(u32, u32); 5] = [(51, 76), (94, 113), (155, 174), (208, 233), (269, 287)];
const CHROME_BODY_DISCS: [(u32, u32, u32, u32); 2] = [(53, 163, 60, 170), (53, 216, 60, 223)];
const CHROME_BODY_TINT: (u32, u32) = (151, 181);
const CHROME_FILE_LINES: [(u32, u32); 4] = [(104, 135), (189, 208), (266, 280), (319, 336)];
const CHROME_FILE_DISCS: [(u32, u32, u32, u32); 2] = [(73, 269, 80, 276), (73, 325, 80, 332)];

fn main() {
    let out = std::env::var_os("MD_PNG").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let body = shoot(Kind::Body);
    let file = shoot(Kind::File);
    if let Some(out) = &out {
        body.save(out.join("body.png")).unwrap();
        file.save(out.join("file.png")).unwrap();
    }
    let mut wrong = Vec::new();

    // Rows 1 and 2 drift a device pixel each from GPUI's whole-pixel line height (22 against 21.7): text placement.
    let lines = ink_rows(&body, (80, 1000), (30, 320));
    if !near(&lines, &CHROME_BODY_LINES, 2) {
        wrong.push(format!("the body's lines stand at {lines:?}, Chromium's at {CHROME_BODY_LINES:?}"));
    }
    let discs: Vec<_> = [(150, 185), (205, 240)].into_iter().map(|rows| bbox(&body, (40, 72), rows, dark)).collect();
    let chrome: Vec<_> = CHROME_BODY_DISCS.iter().copied().map(Some).collect();
    if discs
        .iter()
        .zip(&chrome)
        .any(|(a, b)| a.zip(*b).is_none_or(|(a, b)| a.0 != b.0 || a.2 != b.2 || a.1.abs_diff(b.1) > 1 || a.3.abs_diff(b.3) > 1))
    {
        wrong.push(format!("the body's discs are {discs:?} in the dark ink, Chromium's {chrome:?}"));
    }
    // The code span's tint, read in its left padding where no letter stands, covers the letters' rows.
    let tint = ink_rows(&body, (79, 80), (140, 200));
    let tinted = bbox(&body, (79, 80), (140, 200), |p| p[..3].iter().all(|&c| (230..=246).contains(&c)));
    if tinted.map(|t| (t.1, t.3)) != Some(CHROME_BODY_TINT) || !tint.is_empty() {
        wrong.push(format!("the code span's tint runs rows {tinted:?}, Chromium's {CHROME_BODY_TINT:?}"));
    }

    let lines: Vec<_> = ink_rows(&file, (80, 1000), (0, 400)).into_iter().filter(|r| r.1 - r.0 > 4).collect();
    if !near(&lines, &CHROME_FILE_LINES, 1) {
        wrong.push(format!("the file's lines stand at {lines:?}, Chromium's at {CHROME_FILE_LINES:?}"));
    }
    let discs: Vec<_> = [(260, 290), (310, 345)].into_iter().map(|rows| bbox(&file, (60, 92), rows, dark)).collect();
    let chrome: Vec<_> = CHROME_FILE_DISCS.iter().copied().map(Some).collect();
    if discs != chrome {
        wrong.push(format!("the file's discs are {discs:?} in the dark ink, Chromium's {chrome:?}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
    println!("test markdown_stands_where_chromium_draws_it ... ok");
}
