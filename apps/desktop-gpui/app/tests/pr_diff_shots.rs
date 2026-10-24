// SPDX-License-Identifier: AGPL-3.0-only
//! The shared code view drawn by GPUI's offscreen renderer over `tests/pr_pane/diff_sample.patch`, in Paper and
//! Graphite, stacked and split, in an 800 x 700 window at the test window's scale of 2. `DIFF_PNG=<folder>` writes each
//! capture as `<theme>-<layout>.png`. With `DIFF_WEB=<folder>` holding today's shots of the same patch
//! (`web-<theme>.png`, `web-<theme>-split.png`, taken by `tests/pr_pane/diff_web.sh`), it writes
//! `<theme>-<layout>-diff.png` and prints the share of pixels where any channel differs by more than 15 of 255.
//! It also holds the changed rows' edge to today's: the 4 px bar at the row's edge, in the change's color, or in the
//! selection's on a selected line, and an added file across the whole width in split; the box behind a changed line's
//! changed characters, in today's colors and plain ink; the colors of a row far down a long file once scrolled to; and a
//! comment opened far above a row coloring it; pierre's number lane (its width per file, the 2 px page gap before the
//! code, its fills and ink), the hatched deleted bar and empty split side, a pill per row behind changed characters,
//! a note set in the code column, and the scrollbar with its lane.
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

fn shoot(theme: &str, layout: Layout, selected: Option<diff::LinePosition>) -> RgbaImage {
    let model = sample();
    assert_eq!(model.files.iter().map(|f| f.status).collect::<Vec<_>>(), [FileStatus::Added, FileStatus::Modified]);
    shoot_model(theme, layout, model, move |view, cx| view.set_selected(selected, cx))
}

/// `model` drawn in `theme`, after `then` has run on the view once a frame has been drawn.
fn shoot_model(theme: &str, layout: Layout, model: diff::DiffModel, then: impl FnOnce(&mut CodeView, &mut Context<CodeView>)) -> RgbaImage {
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
    let mut made = None;
    let handle: AnyWindowHandle = cx
        .open_window(size(px(W), px(H)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = CodeView::new(cx);
                view.on_head(head);
                view.set_layout(layout, cx);
                view.set_files(&model.files, cx);
                view
            });
            made = Some(view.clone());
            let pane = cx.new(|_| Pane { view });
            cx.new(|cx| Root::new(pane, window, cx))
        })
        .unwrap()
        .into();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| made.unwrap().update(cx, then));
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

/// Whether the pixel at device `(x, y)` is `hex` (`#rrggbb`) within 8 of 255 in each channel.
fn is(image: &RgbaImage, x: u32, y: u32, hex: &str) -> bool {
    near(image.get_pixel(x, y).0, hex, 8)
}

fn near(p: [u8; 4], hex: &str, by: u8) -> bool {
    near_rgb(p, rgb(hex), by)
}

fn near_rgb(p: [u8; 4], want: [u8; 3], by: u8) -> bool {
    p.iter().zip(want).all(|(have, want)| have.abs_diff(want) <= by)
}

fn rgb(hex: &str) -> [u8; 3] {
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
    [channel(1), channel(3), channel(5)]
}

/// `top` at `share` over `under`, as a translucent fill paints.
fn over(top: [u8; 3], under: [u8; 3], share: f32) -> [u8; 3] {
    [0, 1, 2].map(|c| (f32::from(under[c]) * (1. - share) + f32::from(top[c]) * share).round() as u8)
}

/// The widest run of device columns in row `y`, right of the line numbers, whose pixel is `want` within 8.
fn widest_run(image: &RgbaImage, y: u32, want: [u8; 3]) -> Option<(u32, u32)> {
    let mut best: Option<(u32, u32)> = None;
    let mut start = None;
    for x in CODE_X..=image.width() {
        match (x < image.width() && near_rgb(image.get_pixel(x, y).0, want, 8), start) {
            (true, None) => start = Some(x),
            (false, Some(from)) => {
                if best.is_none_or(|(a, b)| x - from > b + 1 - a) {
                    best = Some((from, x - 1));
                }
                start = None;
            }
            _ => {}
        }
    }
    best
}

/// Where the code starts, in device pixels: past the edge bar, the number lane and its gap.
const CODE_X: u32 = 180;
/// The device columns of `OVERSCAN = 4;`'s digit, the eighteenth character from the code's start at x 100, 15.65 px
/// a character (13 px mono at 2x).
const DIGIT: (u32, u32) = (366, 382);

/// The box behind the changed characters of a changed row whose middle is device row `y`: it covers the changed
/// digit and nothing else, in `boxed`, on the row's `row` tint, with the digit in `ink`. What is wrong, or nothing.
fn changed_characters(image: &RgbaImage, y: u32, word: &str, row: [u8; 3], boxed: [u8; 3], ink: [u8; 3]) -> Option<String> {
    let widest = (y - 8..y + 8).filter_map(|y| widest_run(image, y, boxed)).max_by_key(|(a, b)| b - a);
    let Some((first, last)) = widest.filter(|(a, b)| b - a >= 4) else {
        return Some(format!("the {word} row has no box behind its changed characters, {boxed:?}: widest run {widest:?}"));
    };
    if first + 4 < DIGIT.0 || last > DIGIT.1 + 4 {
        return Some(format!("the {word} row's box spans x {first}..={last}, not the changed digit's {DIGIT:?}"));
    }
    if !near_rgb(image.get_pixel(2 * W as u32 - 40, y).0, row, 8) {
        return Some(format!("the {word} row's tint is {:?}, today's is {row:?}", image.get_pixel(2 * W as u32 - 40, y)));
    }
    let distance = |p: &[u8; 4]| (0..3).map(|c| u32::from(p[c].abs_diff(boxed[c]))).sum::<u32>();
    let drawn = (DIGIT.0..=DIGIT.1)
        .flat_map(|x| (y - 8..y + 8).map(move |y| (x, y)))
        .map(|(x, y)| image.get_pixel(x, y).0)
        .max_by_key(distance)
        .unwrap();
    (!near_rgb(drawn, ink, 12)).then(|| format!("the {word} row's changed digit is inked {drawn:?}, today's ink is {ink:?}"))
}

/// An added TypeScript file of `lines` lines.
fn long_file(lines: usize) -> diff::DiffModel {
    let body: String = (1..=lines).map(|i| format!("+export const item_{i} = {i};\n")).collect();
    let patch =
        format!("diff --git a/long.ts b/long.ts\nnew file mode 100644\n--- /dev/null\n+++ b/long.ts\n@@ -0,0 +1,{lines} @@\n{body}");
    let reply = json!({"base": null, "truncated": false, "moved": [], "files": [{"path": "long.ts", "kind": "added", "patch": patch}]});
    diff::read_reply(reply.as_object().unwrap()).unwrap()
}

/// The same file with a block comment from line 2 to the last line but one.
fn long_comment(lines: usize) -> diff::DiffModel {
    let body: String = (1..=lines)
        .map(|i| match i {
            2 => "+/* opened here\n".to_owned(),
            _ if i == lines - 1 => "+closed here */\n".to_owned(),
            _ => format!("+export const item_{i} = {i};\n"),
        })
        .collect();
    let patch =
        format!("diff --git a/long.ts b/long.ts\nnew file mode 100644\n--- /dev/null\n+++ b/long.ts\n@@ -0,0 +1,{lines} @@\n{body}");
    let reply = json!({"base": null, "truncated": false, "moved": [], "files": [{"path": "long.ts", "kind": "added", "patch": patch}]});
    diff::read_reply(reply.as_object().unwrap()).unwrap()
}

/// A modified TypeScript file whose two changed lines change the same character.
fn two_changed() -> diff::DiffModel {
    let patch = "diff --git a/p.ts b/p.ts\n--- a/p.ts\n+++ b/p.ts\n@@ -1,2 +1,2 @@\n-export const a = 1;\n-export const b = 1;\n+export const a = 2;\n+export const b = 2;\n";
    let reply = json!({"base": null, "truncated": false, "moved": [], "files": [{"path": "p.ts", "kind": "modified", "patch": patch}]});
    diff::read_reply(reply.as_object().unwrap()).unwrap()
}

/// Whether device `(x, y)` is `hex` within `by`, or what it is instead.
fn at(image: &RgbaImage, x: u32, y: u32, hex: &str, by: u8, what: &str) -> Option<String> {
    let p = image.get_pixel(x, y).0;
    (!near(p, hex, by)).then(|| format!("{what} at ({x}, {y}) is {:?}, today's is {hex}", &p[..3]))
}

/// The darkest pixel of a region, by the sum of its channels.
fn darkest(image: &RgbaImage, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> [u8; 4] {
    xs.flat_map(|x| ys.clone().map(move |y| (x, y)))
        .map(|(x, y)| image.get_pixel(x, y).0)
        .min_by_key(|p| p[..3].iter().map(|&c| u32::from(c)).sum::<u32>())
        .unwrap()
}

/// The pixels within 40 of Paper's keyword color, `export` on every row.
fn keyword_pixels(image: &RgbaImage) -> usize {
    image.pixels().filter(|p| near(p.0, "#d32a61", 40)).count()
}

fn main() {
    let out = std::env::var_os("DIFF_PNG").map(PathBuf::from);
    let web = std::env::var_os("DIFF_WEB").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let mut wrong = Vec::new();
    for theme in ["paper", "graphite"] {
        for (layout, word) in [(Layout::Stacked, "stacked"), (Layout::Split, "split")] {
            let image = shoot(theme, layout, None);
            assert_eq!((image.width(), image.height()), (2 * W as u32, 2 * H as u32), "{theme} {word} is drawn at 2x");
            // The added file's first row, 32 px under its head: its edge holds pierre-light's addition color, in split
            // too, where the added file takes the whole width.
            if theme == "paper" && !is(&image, 2, 84, "#18a46c") {
                wrong.push(format!("{word}: the added row's edge is {:?}", image.get_pixel(2, 84)));
            }
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
    let lit = shoot("paper", Layout::Stacked, Some(diff::LinePosition::new("math.rs", diff::Side::Modified, 3)));
    // math.rs line 3 is the third row under the 32 px head: its edge takes pierre-light's modified color.
    if !is(&lit, 2, 164, "#009fff") {
        wrong.push(format!("the selected row's edge is {:?}", lit.get_pixel(2, 164)));
    }
    assert!(wrong.is_empty(), "paper: {}", wrong.join("; "));
    println!("test the_changed_and_selected_rows_wear_todays_edge ... ok");

    // Read off today's shots: scroll.ts's deleted line 10 (device rows 712 to 751) and added line 10 under it, each a
    // number cell to x 78 in its own fill, the page through x 81, the code from 82; the deleted bar red on its
    // device rows 0 and 1 of each 4, its stripe on 2 and 3; an unchanged number in pierre's gray.
    let image = shoot("paper", Layout::Stacked, None);
    let mut wrong: Vec<String> = [
        at(&image, 20, 732, "#fef1f0", 2, "the deleted number cell"),
        at(&image, 80, 732, "#fcfcfc", 1, "the gap on the deleted row"),
        at(&image, 90, 732, "#fef0f0", 2, "the deleted code"),
        at(&image, 20, 772, "#edf7f3", 2, "the added number cell"),
        at(&image, 80, 772, "#fcfcfc", 1, "the gap on the added row"),
        at(&image, 90, 772, "#ecf7f3", 2, "the added code"),
        at(&image, 2, 732, "#d52c36", 8, "the deleted bar's red row"),
        at(&image, 2, 734, "#fce5e2", 3, "the deleted bar's stripe"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let ink = darkest(&image, 30..70, 596..626);
    if !near(ink, "#565656", 4) {
        wrong.push(format!("an unchanged line number is inked {:?}, today's #565656", &ink[..3]));
    }
    // scroll.ts's line 15 has no old line: today's fills its number cell in pierre's context and stripes its code
    // column 2 px in 8 with the buffer color, through the second line the new side wraps to.
    let split = shoot("paper", Layout::Split, None);
    wrong.extend(at(&split, 20, 1030, "#f8f8f8", 2, "the empty side's number cell"));
    let stripes = (82..700).filter(|&x| near(split.get_pixel(x, 1030).0, "#e5e5e5", 4)).count();
    if !(100..=200).contains(&stripes) {
        wrong.push(format!("the empty side's code column has {stripes} stripe pixels on a row, today's about 150"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
    println!("test the_number_lane_gap_fills_and_bars_are_todays ... ok");

    // scroll.ts `const OVERSCAN = 4;` against `= 8;`, the fourth and fifth rows under its head and 24 px bar: today's
    // Paper draws `src/keep.ts`'s `= 2;` against `= 3;` the same way, read off its shot. Graphite has no shot here, so
    // its row and box are pierre-dark's: the change color at 6.5% over the page, then at 20% over that row.
    let (deleted, added) = (2 * (272 + 24 + 3 * 20 + 10), 2 * (272 + 24 + 4 * 20 + 10));
    let image = shoot("paper", Layout::Stacked, None);
    let ink = rgb("#27272a");
    let mut wrong: Vec<String> = [
        changed_characters(&image, deleted, "paper deleted", rgb("#fef0f0"), rgb("#f8d3d4"), ink),
        changed_characters(&image, added, "paper added", rgb("#ecf7f3"), rgb("#cceade"), ink),
    ]
    .into_iter()
    .flatten()
    .collect();
    let image = shoot("graphite", Layout::Stacked, None);
    let (page, ink) = (rgb("#0a0a0a"), rgb("#f5f5f5"));
    for (y, word, base) in [(deleted, "graphite deleted", rgb("#ff2e3f")), (added, "graphite added", rgb("#07c480"))] {
        let row = over(base, page, 0.065);
        wrong.extend(changed_characters(&image, y, word, row, over(base, row, 0.2), ink));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
    println!("test the_changed_characters_wear_todays_box_and_plain_ink ... ok");

    // math.rs line 3 selected: StyledDiffCodeView's selected number cell and code, the gap between, the number dark.
    let mut wrong: Vec<String> = [
        at(&lit, 20, 164, "#f0f6fc", 2, "the selected number cell"),
        at(&lit, 80, 164, "#fcfcfc", 1, "the gap on the selected row"),
        at(&lit, 2 * W as u32 - 40, 164, "#eff6fc", 2, "the selected code"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let ink = darkest(&lit, 30..78, 150..178);
    if !near(ink, "#20649e", 8) {
        wrong.push(format!("the selected line's number is inked {:?}, today's #20649e", &ink[..3]));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
    println!("test a_selected_row_keeps_the_gap_and_a_dark_number ... ok");

    // Two changed lines changing the same digit: each added row's box is its own pill, 5 device px clear of the row
    // edge, so the two do not join across the rows' boundary at device row 184.
    let pills = shoot_model("paper", Layout::Stacked, two_changed(), |_, _| {});
    let boxed = |y: u32| (360..384).filter(|&x| near(pills.get_pixel(x, y).0, "#cceade", 6)).count();
    let (inside, edge) = ((150..178).map(boxed).max().unwrap(), (181..188).map(boxed).max().unwrap());
    assert!(inside >= 4 && edge == 0, "the added rows' boxes: {inside} pill pixels inside a row, {edge} at the rows' boundary");
    println!("test each_changed_row_draws_its_own_pill ... ok");

    // Graphite, a note under scroll.ts's line 11 (device rows 792 to 831): today's sets it in the code column on
    // pierre's context fill, beside the number lane in its own, the page between.
    let noted = shoot_model("graphite", Layout::Stacked, sample(), |view, cx| {
        view.on_note(|_, _, _| div().h(px(40.)).into_any_element());
        let note = diff::LineNote { id: "n".into(), path: "scroll.ts".into(), side: diff::Side::Modified, line: 11 };
        view.set_notes(vec![note], cx);
    });
    let wrong: Vec<String> = [
        at(&noted, 20, 860, "#131313", 2, "the note's number lane"),
        at(&noted, 80, 860, "#0a0a0a", 1, "the gap beside the note"),
        at(&noted, 300, 860, "#1c1c1c", 2, "the note's fill"),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
    println!("test a_note_sits_in_the_code_column ... ok");

    let top = shoot_model("paper", Layout::Stacked, long_file(3000), |_, _| {});
    // Today's lane for a 4-digit file: 7 characters of 7.83 css px, to device x 110, then the gap to 114.
    let wrong: Vec<String> = [
        at(&top, 104, 84, "#edf7f3", 2, "the 4-digit file's number cell"),
        at(&top, 112, 84, "#fcfcfc", 1, "the 4-digit file's gap"),
        at(&top, 116, 84, "#ecf7f3", 2, "the 4-digit file's code"),
        // The bar: 6 px at the right, `--app-scrollbar-thumb` at the top; the rows stop short of its lane.
        at(&top, 2 * W as u32 - 6, 20, "#d9d9d9", 2, "the scrollbar's thumb"),
        at(&top, 2 * W as u32 - 6, 400, "#fcfcfc", 1, "the scrollbar's lane"),
        at(&top, 2 * W as u32 - 16, 400, "#ecf7f3", 2, "a row beside the lane"),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
    println!("test a_long_file_has_its_own_lane_width_and_the_scrollbar ... ok");

    let far = shoot_model("paper", Layout::Stacked, long_file(3000), |view, cx| {
        view.scroll_to("long.ts", Some((diff::Side::Modified, 2600)), cx)
    });
    if let Some(out) = &out {
        far.save(out.join("paper-far.png")).unwrap();
    }
    assert!(top.as_raw() != far.as_raw(), "scrolling to line 2,600 moved the view");
    let (at_top, at_far) = (keyword_pixels(&top), keyword_pixels(&far));
    assert!(at_top > 5000, "the top of a long file is colored: {at_top} keyword pixels");
    assert!(at_far * 2 > at_top, "line 2,600 of 3,000 is colored once scrolled to: {at_far} keyword pixels, {at_top} at the top");
    println!("test a_row_far_down_a_long_file_is_colored_once_scrolled_to ... ok");

    // The Files tab shows one file after another in one view: the second is colored from its own text, not from the
    // first's parse in the same place.
    let second = shoot_model("paper", Layout::Stacked, long_comment(3000), |view, cx| view.set_files(&long_file(3000).files, cx));
    let keywords = keyword_pixels(&second);
    assert!(keywords > 5000, "a file shown where a commented one was is colored as itself: {keywords} keyword pixels");
    println!("test a_file_shown_after_another_in_one_view_is_colored_as_itself ... ok");

    // A comment opened on line 2 and closed on line 2,999: line 2,600 is inside it, as in today's view.
    let commented = shoot_model("paper", Layout::Stacked, long_comment(3000), |view, cx| {
        view.scroll_to("long.ts", Some((diff::Side::Modified, 2600)), cx)
    });
    if let Some(out) = &out {
        commented.save(out.join("paper-comment.png")).unwrap();
    }
    let gray = commented.pixels().filter(|p| near(p.0, "#737373", 24)).count();
    let keywords = keyword_pixels(&commented);
    assert!(
        keywords < 100 && gray > 5000,
        "line 2,600, inside a comment opened on line 2, is comment gray: {gray} gray and {keywords} keyword pixels"
    );
    println!("test a_comment_opened_far_above_colors_the_rows_inside_it ... ok");
}
