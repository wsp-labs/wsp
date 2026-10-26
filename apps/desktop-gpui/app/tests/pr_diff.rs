// SPDX-License-Identifier: AGPL-3.0-only
//! The shared code view (`screens/pr_pane/diff`) in a test window after the app's own init, wrapped in gpui-kit's Root
//! as every window is: its 32 px heads and the press that folds, the head pinned over a file scrolled past, the bars
//! that count unmodified lines, the owner's notes and bodies, the file view's lit line, the pierre colors each
//! theme file carries, what a frame copies of a long line, and an idle view of many files preparing nothing more.

use std::alloc::{GlobalAlloc, Layout as Alloc, System};
use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;

use desktop_gpui::screens::pr_pane::diff::{self, CodeView, FileHead, FileView, Layout, LineNote, Side};
use desktop_gpui::ui::{self, Side as ThemeSide, Still};
use gpui_kit::component::{ActiveTheme as _, Root};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, Context, Entity, Modifiers, Rgba, SharedString, TestAppContext, VisualTestContext, Window,
};
use serde_json::json;

const PATCH: &str = include_str!("pr_pane/diff_sample.patch");

/// Counts the bytes each thread asks the allocator for, so a test can read what one frame on its own thread copies.
struct Counting;

thread_local! {
    static ASKED: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Alloc) -> *mut u8 {
        _ = ASKED.try_with(|asked| asked.set(asked.get() + layout.size()));
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Alloc) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Alloc, new_size: usize) -> *mut u8 {
        _ = ASKED.try_with(|asked| asked.set(asked.get() + new_size));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

struct Pane {
    view: Entity<CodeView>,
}

impl Render for Pane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.view.clone())
    }
}

fn head(head: &FileHead, _: &mut Window, cx: &mut App) -> AnyElement {
    div().flex().gap(px(4.)).child(diff::file_chevron(head, cx)).child(head.path.clone()).into_any_element()
}

fn model_of(files: Vec<serde_json::Value>) -> diff::DiffModel {
    let reply = json!({"base": null, "truncated": false, "moved": [], "files": files});
    diff::read_reply(reply.as_object().unwrap()).unwrap()
}

/// The sample patch, one reply file per `diff --git`.
fn sample() -> diff::DiffModel {
    let mut files: Vec<String> = Vec::new();
    for line in PATCH.split_inclusive('\n') {
        if line.starts_with("diff --git ") {
            files.push(String::new());
        }
        files.last_mut().unwrap().push_str(line);
    }
    model_of(files.into_iter().map(|p| json!({"path": p.lines().next().unwrap().rsplit(" b/").next().unwrap(), "patch": p})).collect())
}

/// An added file of `lines` lines.
fn added(path: &str, lines: usize) -> serde_json::Value {
    let body: String = (1..=lines).map(|i| format!("+let line_{i} = {i};\n")).collect();
    json!({"path": path, "kind": "added", "patch": format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{lines} @@\n{body}")})
}

fn open(cx: &mut TestAppContext, model: diff::DiffModel, setup: impl FnOnce(&mut CodeView)) -> (Entity<CodeView>, &mut VisualTestContext) {
    open_with(cx, model, true, setup)
}

/// `still` holds motion as captures do; without it the fades run.
fn open_with(
    cx: &mut TestAppContext,
    model: diff::DiffModel,
    still: bool,
    setup: impl FnOnce(&mut CodeView),
) -> (Entity<CodeView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(ThemeSide::Light, cx);
        cx.set_global(Still(still));
    });
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = CodeView::new(cx);
            view.on_head(head);
            setup(&mut view);
            view.set_files(&model.files, cx);
            view
        });
        made = Some(view.clone());
        let pane = cx.new(|_| Pane { view });
        Root::new(pane, window, cx)
    });
    cx.run_until_parked();
    (made.unwrap(), cx)
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Option<gpui_kit::Bounds<gpui_kit::Pixels>> {
    cx.run_until_parked();
    cx.debug_bounds(selector)
}

#[gpui_kit::test]
fn heads_stand_32_px_tall_and_a_press_folds_the_file_and_says_so(cx: &mut TestAppContext) {
    let heard: Rc<RefCell<Vec<(SharedString, bool)>>> = Rc::default();
    let said = heard.clone();
    let (view, cx) = open(cx, sample(), move |view| view.on_fold(move |path, folded, _, _| said.borrow_mut().push((path.clone(), folded))));
    let first = bounds(cx, "diff-head-0").expect("the first file's head is drawn");
    assert_eq!((first.top(), first.size.height), (px(0.), px(32.)), "the first head sits at the top, 32 px tall");
    let second = bounds(cx, "diff-head-1").expect("the second file's head is drawn");
    // The added file's ten lines of 20 px under its 32 px head, then pierre's 8 px foot.
    assert_eq!(second.top(), px(32. + 10. * 20. + 8.), "the second head follows the first file's rows and foot");
    let press = gpui_kit::point(px(200.), second.center().y);
    cx.simulate_click(press, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(heard.borrow().as_slice(), [("scroll.ts".into(), true)]);
    let (folded, collapsed) = view.read_with(cx, |v, cx| (v.is_folded("scroll.ts"), v.state().read(cx).is_file_collapsed("scroll.ts")));
    assert!(folded && collapsed, "the press folds the file in the view and in gpui-kit's rows");
    assert!(bounds(cx, "diff-gap-6").is_none(), "a folded file draws no bar under its head");
    cx.simulate_click(press, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(heard.borrow().last(), Some(&("scroll.ts".into(), false)));
    assert!(bounds(cx, "diff-gap-6").is_some(), "the opened file draws its bars again");
}

#[gpui_kit::test]
fn a_file_scrolled_past_keeps_its_head_pinned_at_the_top(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, model_of(vec![added("long.rs", 400), added("next.rs", 400)]), |_| {});
    assert!(bounds(cx, "diff-sticky-0").is_none(), "nothing is pinned while the head is in place");
    view.update(cx, |v, cx| v.scroll_to("long.rs", Some((Side::Modified, 200)), cx));
    let pinned = bounds(cx, "diff-sticky-0").expect("the long file's head is pinned once it scrolls past");
    assert_eq!((pinned.top(), pinned.size.height), (px(0.), px(32.)));
    assert!(bounds(cx, "diff-head-0").is_none(), "its own row is off screen");
    view.update(cx, |v, cx| v.scroll_to("next.rs", Some((Side::Modified, 200)), cx));
    assert!(bounds(cx, "diff-sticky-1").is_some(), "the next file's head takes the top");
    assert!(bounds(cx, "diff-sticky-0").is_none());
    view.update(cx, |v, cx| v.scroll_to("long.rs", None, cx));
    let row = bounds(cx, "diff-head-0").expect("its head row is back on screen");
    assert_eq!(row.top(), px(0.));
    assert!(bounds(cx, "diff-sticky-0").is_none(), "a head in place is not drawn twice");
}

#[gpui_kit::test]
fn the_bars_count_unmodified_lines_in_todays_words(cx: &mut TestAppContext) {
    assert_eq!(diff::unmodified(1), "1 unmodified line");
    assert_eq!(diff::unmodified(12), "12 unmodified lines");
    let (view, cx) = open(cx, sample(), |_| {});
    let first = bounds(cx, "diff-gap-6").expect("the six lines over the first hunk");
    let head = bounds(cx, "diff-head-1").unwrap();
    assert_eq!((first.top(), first.size.height), (head.bottom(), px(24.)), "the bar sits under the head, 24 px tall");
    assert!(bounds(cx, "diff-gap-12").is_some(), "the twelve lines between the first two hunks");
    view.update(cx, |v, cx| v.set_layout(Layout::Split, cx));
    assert!(bounds(cx, "diff-gap-6").is_some(), "split keeps the bars");
}

/// Two hunks of short lines eight lines apart, so every row is one line tall.
fn two_hunks(path: &str) -> serde_json::Value {
    let patch = format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n-a\n+b\n@@ -10 +10 @@\n-c\n+d\n");
    json!({"path": path, "kind": "modified", "patch": patch})
}

#[gpui_kit::test]
fn a_bar_between_hunks_takes_24_px_and_no_more(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, model_of(vec![two_hunks("x.rs"), added("y.rs", 1)]), |_| {});
    let bar = bounds(cx, "diff-gap-8").expect("the eight lines between the hunks");
    assert_eq!((bar.top(), bar.size.height), (px(32. + 2. * 20.), px(24.)), "under the first hunk's two rows");
    let next = bounds(cx, "diff-head-1").unwrap();
    assert_eq!(next.top(), px(32. + 2. * 20. + 24. + 2. * 20. + 8.), "the bar adds 24 px between the hunks");
    view.update(cx, |v, cx| v.set_layout(Layout::Split, cx));
    let next = bounds(cx, "diff-head-1").unwrap();
    assert_eq!(next.top(), px(32. + 20. + 24. + 20. + 8.), "split pairs each change on one row");
}

/// A scroll the view did not make, as the scrollbar dragged into the middle of the next file, pins that file's head.
#[gpui_kit::test]
fn a_scroll_into_the_middle_of_the_next_file_pins_its_head(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, model_of(vec![added("long.rs", 400), added("next.rs", 400)]), |_| {});
    view.update(cx, |v, cx| v.scroll_to("long.rs", Some((Side::Modified, 200)), cx));
    assert!(bounds(cx, "diff-sticky-0").is_some());
    let state = view.read_with(cx, |v, _| v.state().clone());
    state.update(cx, |s, cx| s.scroll_to_line(diff::LinePosition::new("next.rs", Side::Modified, 200), cx));
    assert!(bounds(cx, "diff-sticky-1").is_some(), "the head of the file in view is pinned");
    assert!(bounds(cx, "diff-sticky-0").is_none(), "not the head pinned before the scroll");
}

#[gpui_kit::test]
fn folding_every_file_at_once_folds_each_in_gpui_kits_rows(cx: &mut TestAppContext) {
    let paths: Vec<SharedString> = (0..300).map(|i| SharedString::from(format!("f{i}.rs"))).collect();
    let (view, cx) = open(cx, model_of(paths.iter().map(|p| added(p, 3)).collect()), |_| {});
    let shut = |view: &Entity<CodeView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |v, cx| paths.iter().filter(|p| v.state().read(cx).is_file_collapsed(p)).count())
    };
    view.update(cx, |v, cx| v.set_folded_paths(paths.clone(), cx));
    assert_eq!(shut(&view, cx), 300);
    assert_eq!(bounds(cx, "diff-head-1").unwrap().top(), px(32.), "folded heads stand one under the other");
    view.update(cx, |v, cx| v.set_folded_paths([paths[1].clone()], cx));
    assert_eq!(shut(&view, cx), 1);
    assert!(view.read_with(cx, |v, _| v.is_folded("f1.rs") && !v.is_folded("f0.rs")));
}

#[gpui_kit::test]
fn notes_and_bodies_are_drawn_by_their_owner(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, sample(), |view| {
        view.on_note(|note, _, _| div().debug_selector(|| "the-note".into()).h(px(40.)).child(note.id.clone()).into_any_element());
    });
    let note = LineNote { id: "c1".into(), path: "scroll.ts".into(), side: Side::Modified, line: 13 };
    view.update(cx, |v, cx| v.set_notes(vec![note], cx));
    let drawn = bounds(cx, "the-note").expect("the note is drawn under its line");
    assert_eq!(drawn.size.height, px(40.), "the owner's element is drawn at its own height");
    let head = bounds(cx, "diff-head-1").unwrap();
    assert!(drawn.top() > head.bottom(), "under the file's head");
    let before = bounds(cx, "diff-head-1").unwrap().top();
    view.update(cx, |v, cx| {
        v.set_body("math.rs", Some(Rc::new(|_, _, _| div().debug_selector(|| "the-body".into()).h(px(50.)).into_any_element())), cx)
    });
    let body = bounds(cx, "the-body").expect("the owner's body stands in for the lines");
    assert_eq!(body.top(), px(32.), "right under the head");
    let after = bounds(cx, "diff-head-1").unwrap().top();
    assert_eq!(after, px(32. + 50. + 8.), "the ten lines gave way to the body (from {before:?})");
    view.update(cx, |v, cx| v.set_body("math.rs", None, cx));
    assert!(bounds(cx, "the-body").is_none());
    assert_eq!(bounds(cx, "diff-head-1").unwrap().top(), before, "the lines come back");
}

#[gpui_kit::test]
fn a_reply_cut_at_the_budget_leaves_cut_files_to_the_tree(cx: &mut TestAppContext) {
    let model = model_of(vec![added("kept.rs", 3), json!({"path": "huge.json", "kind": "modified", "additions": 70000, "patch": ""})]);
    assert!(!model.all_cut());
    let (view, cx) = open(cx, model, |_| {});
    let heads: Vec<String> = view.read_with(cx, |v, _| v.heads().iter().map(|h| h.path.to_string()).collect());
    assert_eq!(heads, ["kept.rs"], "a file with no patch has no lines to draw");
    assert!(bounds(cx, "diff-head-1").is_none());
}

#[gpui_kit::test]
fn the_file_view_holds_a_2_mb_file_and_lights_its_line(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(ThemeSide::Light, cx);
    });
    // The file tab reads up to 2 MB of a file.
    let text: String = (1..=87_000).map(|i| format!("const line{i} = {i};\n")).collect();
    assert!(text.len() > 2_000_000);
    let started = std::time::Instant::now();
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = FileView::new(cx);
            view.set_file("big.ts", &text, cx);
            view.set_lit(Some(80_000), cx);
            view
        });
        made = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.run_until_parked();
    println!("a 2 MB file set, drawn and highlighted in {:?}", started.elapsed());
    let view = made.unwrap();
    let lit = view.read_with(cx, |v, cx| v.state().read(cx).selected_lines());
    assert_eq!(lit.map(|r| (r.path().to_string(), r.start(), r.end())), Some(("big.ts".into(), 80_000, 80_000)));
    view.update(cx, |v, cx| v.set_lit(None, cx));
    assert!(view.read_with(cx, |v, cx| v.state().read(cx).selected_lines()).is_none());
}

fn color(hex: &str) -> gpui_kit::Hsla {
    Rgba::try_from(hex).unwrap().into()
}

#[gpui_kit::test]
fn each_theme_colors_code_as_its_pierre_theme_does(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(ThemeSide::Light, cx);
        for (theme, keyword, string) in [
            ("paper", "#d32a61ff", "#199f43ff"),
            ("frost", "#d32a61ff", "#199f43ff"),
            ("graphite", "#ff678dff", "#5ecc71ff"),
            ("pitch", "#ff678dff", "#5ecc71ff"),
        ] {
            ui::theme::set_theme(theme, cx);
            let syntax = &cx.theme().highlight_theme.style.syntax;
            assert_eq!(syntax.style("keyword").and_then(|s| s.color), Some(color(keyword)), "{theme}: keyword");
            assert_eq!(syntax.style("string").and_then(|s| s.color), Some(color(string)), "{theme}: string");
            // A name gpui-kit's grammars write with a suffix falls back to its first part.
            assert_eq!(syntax.style("keyword.function"), syntax.style("keyword"), "{theme}");
        }
    });
}

/// The committed blocks are what `scripts/pierre-themes.mjs` writes from the installed pierre themes.
#[test]
fn the_theme_files_hold_what_the_generator_writes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let node = which("node").expect("node on PATH");
    let out = Command::new(node)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .arg(root.join("scripts/pierre-themes.mjs"))
        .arg("--check")
        .output()
        .unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "pierre-themes.mjs --check: {said}");
}

fn which(name: &str) -> Option<std::path::PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(name)).find(|p| p.is_file())
}

#[gpui_kit::test]
fn a_selected_line_is_lit_in_gpui_kits_rows(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, sample(), |_| {});
    let line = diff::LinePosition::new("scroll.ts", Side::Modified, 14);
    view.update(cx, |v, cx| v.set_selected(Some(line), cx));
    let lit = view.read_with(cx, |v, cx| v.state().read(cx).selected_lines());
    assert_eq!(lit.map(|r| (r.path().to_string(), r.side(), r.start(), r.end())), Some(("scroll.ts".into(), Side::Modified, 14, 14)));
    view.update(cx, |v, cx| v.set_selected(None, cx));
    assert!(view.read_with(cx, |v, cx| v.state().read(cx).selected_lines()).is_none());
}

struct Raw {
    raw: diff::RawPatch,
}

impl Render for Raw {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(diff::raw_patch(&self.raw, cx))
    }
}

#[gpui_kit::test]
fn a_patch_that_does_not_parse_is_drawn_raw_a_screen_at_a_time(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(ThemeSide::Light, cx);
    });
    let lines: String = (0..20_000).map(|i| format!("+line {i}\n")).collect();
    let broken = format!("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ nonsense @@\n{lines}");
    let raw = model_of(vec![json!({"path": "x", "patch": broken})]).raw.expect("the patch is kept raw");
    assert_eq!(raw.lines.len(), 20_004);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| Raw { raw });
        Root::new(view, window, cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("raw-line-0").is_some(), "the first line is drawn");
    assert!(cx.debug_bounds("raw-line-19000").is_none(), "a line far below is not");
}

/// A last line with no line end keeps no "No newline at end of file" row: gpui-kit draws that row only under a line
/// whose source ends without one, which the copied line shows.
#[gpui_kit::test]
fn the_file_view_draws_no_missing_newline_row(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(ThemeSide::Light, cx);
    });
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = FileView::new(cx);
            view.set_file("a.txt", "first\nlast", cx);
            view.set_lit(Some(2), cx);
            view
        });
        made = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.run_until_parked();
    let copied = made.unwrap().read_with(cx, |v, cx| v.state().read(cx).selected_text(cx));
    assert_eq!(copied, "last\n", "the last line ends with a line end");
}

/// Delivers `rounds` frames, a little apart in real time as the fades are timed, and says how many of the last
/// asked for another.
fn idle(cx: &mut VisualTestContext, rounds: usize) -> usize {
    let mut asked = 0;
    for _ in 0..rounds {
        std::thread::sleep(std::time::Duration::from_millis(30));
        asked = cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    asked
}

#[gpui_kit::test]
fn an_idle_view_stops_drawing_once_its_bodies_have_faded_in(cx: &mut TestAppContext) {
    let renders: Rc<std::cell::Cell<usize>> = Rc::default();
    let counted = renders.clone();
    // The second file starts below the pane, so its fade waits until it is scrolled to.
    let (_, cx) = open_with(cx, model_of(vec![added("long.rs", 400), added("next.rs", 400)]), false, move |view| {
        view.on_head(move |h, window, cx| {
            counted.set(counted.get() + 1);
            head(h, window, cx)
        })
    });
    idle(cx, 12);
    let settled = renders.get();
    assert_eq!(idle(cx, 12), 0, "an idle view asks for no frame");
    assert_eq!(renders.get(), settled, "and draws its rows no more");
}

#[gpui_kit::test]
fn the_same_files_again_keep_the_pinned_head_and_fade_nothing(cx: &mut TestAppContext) {
    let model = model_of(vec![added("long.rs", 400), added("next.rs", 400)]);
    let files = model.files.clone();
    let (view, cx) = open_with(cx, model, false, |_| {});
    view.update(cx, |v, cx| v.scroll_to("next.rs", Some((Side::Modified, 200)), cx));
    idle(cx, 12);
    assert!(bounds(cx, "diff-sticky-1").is_some(), "the next file's head is pinned");
    assert_eq!(idle(cx, 2), 0, "settled");
    view.update(cx, |v, cx| v.set_files(&files, cx));
    assert_eq!(idle(cx, 1), 0, "a refresh of the same files fades none in");
    assert!(bounds(cx, "diff-sticky-1").is_some(), "the pinned head stays through a refresh");
    let changed = model_of(vec![added("long.rs", 400), added("next.rs", 401)]);
    view.update(cx, |v, cx| v.set_files(&changed.files, cx));
    assert_eq!(idle(cx, 1), 0, "a changed file shown before does not fade in again");
    let row = bounds(cx, "diff-head-1").expect("the view stays at the file it showed");
    assert_eq!(row.top(), px(8.), "its head at the top, under the foot of the file above");
}

#[gpui_kit::test]
fn a_binary_file_shows_its_head_and_no_body(cx: &mut TestAppContext) {
    let bin = "diff --git a/logo.bin b/logo.bin\nindex 2f816c7..a6629cd 100644\nBinary files a/logo.bin b/logo.bin differ\n";
    let model = model_of(vec![json!({"path": "logo.bin", "kind": "modified", "patch": bin}), added("next.rs", 3)]);
    assert_eq!(model.files[0].status, diff::FileStatus::Binary);
    let (view, cx) = open(cx, model, |_| {});
    let next = bounds(cx, "diff-head-1").expect("the next file's head");
    assert_eq!(next.top(), px(32.), "the next head follows the binary file's head, with no body or foot between");
    assert!(!view.read_with(cx, |v, _| v.is_folded("logo.bin")), "its head stays open");
    let press = gpui_kit::point(px(200.), px(16.));
    cx.simulate_click(press, Modifiers::none());
    cx.simulate_click(press, Modifiers::none());
    assert_eq!(bounds(cx, "diff-head-1").unwrap().top(), px(32.), "folding and opening it draws no body either");
}

#[gpui_kit::test]
fn a_frame_copies_a_long_line_once_not_once_for_each_piece(cx: &mut TestAppContext) {
    // 200 KB on one line, drawn unwrapped in pieces of 512 bytes: a frame that built the whole line for each piece
    // would copy it 400 times over, 80 MB, twice.
    let line: String = (0..20_000).map(|i| format!("v{i:07}=1;")).collect();
    let patch = format!("diff --git a/min.js b/min.js\nnew file mode 100644\n--- /dev/null\n+++ b/min.js\n@@ -0,0 +1 @@\n+{line}\n");
    let (view, cx) = open(cx, model_of(vec![json!({"path": "min.js", "kind": "added", "patch": patch})]), |_| {});
    view.update(cx, |view, cx| view.set_wrap(false, cx));
    idle(cx, 2);
    let before = ASKED.with(Cell::get);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let copied = ASKED.with(Cell::get) - before;
    assert!(copied < 16 << 20, "one frame of a 200 KB line asked for {} MB", copied >> 20);
}

#[gpui_kit::test]
fn an_idle_split_view_of_many_files_prepares_nothing_more(cx: &mut TestAppContext) {
    // A block a side of each file near the view, more than are kept once out of view.
    let files = (0..30).map(|i| two_hunks(&format!("src/f{i}.rs"))).collect();
    let (view, cx) = open(cx, model_of(files), |_| {});
    view.update(cx, |view, cx| view.set_layout(Layout::Split, cx));
    idle(cx, 12);
    let prepared = |cx: &mut VisualTestContext| view.read_with(cx, |v, cx| v.state().read(cx).syntax_blocks_prepared());
    let settled = prepared(cx);
    assert!(settled > 32, "the blocks near the view are prepared: {settled}");
    for _ in 0..3 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert_eq!(prepared(cx), settled, "a view drawn again at rest prepares no block again");
}

#[gpui_kit::test]
fn scrolling_a_long_file_keeps_at_most_32_blocks_of_syntax(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, model_of(vec![added("long.rs", 12_000)]), |_| {});
    for line in (1..12_000).step_by(400) {
        view.update(cx, |view, cx| view.scroll_to("long.rs", Some((Side::Modified, line)), cx));
        idle(cx, 2);
    }
    let (prepared, kept) = view.read_with(cx, |v, cx| {
        let state = v.state().read(cx);
        (state.syntax_blocks_prepared(), state.syntax_blocks_kept())
    });
    assert!(prepared > 32, "the scroll prepared a block wherever it stopped: {prepared}");
    assert!(kept <= 32, "of {prepared} prepared, {kept} are kept");
}
