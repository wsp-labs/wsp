// SPDX-License-Identifier: AGPL-3.0-only
use std::sync::{Arc, Mutex};

use gpui_kit::base::text::TextViewState;
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, Bounds, Context, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, Window};

use super::{
    fence_language, fence_title, opens_with_heading, person_text, run_output_message, runnable_command, view, web_page, OnRun, ReplyRuns,
    RunAction,
};
use crate::transcript::Run;
use crate::ui::Side;

struct Reply(Entity<TextViewState>, Vec<Run>, Option<(OnRun, bool)>);

impl Render for Reply {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(div().debug_selector(|| "reply".into()).child(view(
            &self.0,
            false,
            self.2.clone().map(|(on_run, can_start)| ReplyRuns { runs: &self.1, on_run, can_start }),
            cx,
        )))
    }
}

/// A reply of `markdown` in a window `width` wide; a settled one, with its runs, when `on_run` is given.
fn open<'a>(
    cx: &'a mut TestAppContext,
    markdown: &str,
    width: f32,
    runs: Vec<Run>,
    on_run: Option<(OnRun, bool)>,
) -> &'a mut VisualTestContext {
    cx.update(gpui_kit::init);
    cx.update(|cx| crate::ui::init(Side::Light, cx));
    let markdown = markdown.to_owned();
    let body = cx.new(|cx| TextViewState::markdown(&markdown, cx));
    let reply = cx.new(|_| Reply(body, runs, on_run));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(reply.clone(), window, cx));
    cx.simulate_resize(size(px(width), px(800.)));
    cx.run_until_parked();
    cx
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("no {selector}"))
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = bounds(cx, selector).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn recorder() -> (OnRun, Arc<Mutex<Vec<RunAction>>>) {
    let said = Arc::new(Mutex::new(Vec::new()));
    let into = said.clone();
    (Arc::new(move |action, _, _| into.lock().unwrap().push(action)), said)
}

fn ran(state: &str, output: Option<&str>) -> Run {
    Run {
        run_id: "r1".into(),
        block: 1,
        command: "ls -la".into(),
        state: state.into(),
        output: output.map(str::to_owned),
        exit_code: Some(0),
        signal: None,
        pty_id: None,
    }
}

const TWO_BLOCKS: &str = "Look:\n\n```rust\nlet x = 1;\n```\n\nThen:\n\n```sh\nls -la\n```";

#[test]
fn only_a_web_page_goes_to_the_system() {
    assert!(web_page("https://wsp.dev/a") && web_page("http://localhost:3000/"));
    for url in ["file:///Users/me/ws/run.command", "smb://box/share", "vscode://file/x", "javascript:alert(1)", "ssh://h", "/etc/passwd"] {
        assert!(!web_page(url), "{url} would open");
    }
}

#[gpui_kit::test]
fn run_shows_on_a_replys_shell_block_and_starts_that_block(cx: &mut TestAppContext) {
    let (on_run, said) = recorder();
    let cx = open(cx, TWO_BLOCKS, 760., Vec::new(), Some((on_run, true)));
    let (run, reply) = (bounds(cx, "code-run"), bounds(cx, "reply"));
    assert!(run.center().y > reply.center().y, "Run sits on the second block, the shell one, in the reply's lower half");
    click(cx, "code-run");
    let said = said.lock().unwrap();
    assert!(matches!(said.as_slice(), [RunAction::Start { block: 1, command, .. }] if command == "ls -la"), "{said:?}");
}

#[gpui_kit::test]
fn a_document_given_no_runs_draws_neither_run_nor_a_run(cx: &mut TestAppContext) {
    let cx = open(cx, TWO_BLOCKS, 760., vec![ran("exited", Some("total 0"))], None);
    assert!(cx.debug_bounds("code-run").is_none());
    assert!(cx.debug_bounds("code-run-output").is_none(), "no run drawn without Run");
}

#[gpui_kit::test]
fn an_ended_run_shows_under_its_block_and_sends_its_output(cx: &mut TestAppContext) {
    let (on_run, said) = recorder();
    let cx = open(cx, TWO_BLOCKS, 760., vec![ran("exited", Some("total 0"))], Some((on_run, false)));
    assert!(cx.debug_bounds("code-run").is_none(), "no Run while runs cannot start");
    let (output, reply) = (bounds(cx, "code-run-output"), bounds(cx, "reply"));
    assert!(output.top() > reply.center().y, "under the second block, the shell one");
    click(cx, "run-send");
    let sent = said.lock().unwrap().clone();
    assert_eq!(
        sent,
        vec![RunAction::Send { message: "I ran `ls -la` and it exited 0. The last lines it printed:\n\n```text\ntotal 0\n```".into() }]
    );
}

fn live(state: &str) -> Run {
    Run { pty_id: Some("p1".into()), exit_code: None, ..ran(state, None) }
}

fn hover(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    let at = bounds(cx, selector).center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor().advance_clock(std::time::Duration::from_millis(600));
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    cx.debug_bounds("tooltip").is_some()
}

#[gpui_kit::test]
fn run_and_its_neighbours_say_what_they_do_on_hover(cx: &mut TestAppContext) {
    let (on_run, _) = recorder();
    let cx = open(cx, TWO_BLOCKS, 760., Vec::new(), Some((on_run, true)));
    assert!(hover(cx, "code-run"), "Run showed no tooltip");
}

#[gpui_kit::test]
fn a_running_run_holds_run_and_draws_its_live_box_with_move_after_three_seconds(cx: &mut TestAppContext) {
    let (on_run, said) = recorder();
    let cx = open(cx, TWO_BLOCKS, 760., vec![live("running")], Some((on_run, true)));
    assert!(cx.debug_bounds("code-run").is_none() && cx.debug_bounds("code-run-held").is_some(), "Run is not held while it runs");
    let shown = bounds(cx, "run-live");
    assert!(shown.size.height >= px(232. + 36.), "the live box is {:?} tall, not 232 px and its footer", shown.size.height);
    assert!(shown.top() > bounds(cx, "code-run-held").bottom(), "under the shell block's head");
    assert_eq!(*said.lock().unwrap(), vec![RunAction::Resume(live("running"))], "a run this window holds not is taken back");
    click(cx, "code-run-held");
    assert_eq!(said.lock().unwrap().len(), 1, "a held Run started another");
    assert!(cx.debug_bounds("run-move").is_none(), "Move to terminal before the run printed three seconds");
    cx.executor().advance_clock(super::super::run::MOVE_AFTER);
    cx.run_until_parked();
    click(cx, "run-move");
    assert_eq!(said.lock().unwrap().last(), Some(&RunAction::Move(live("running"))));
}

#[gpui_kit::test]
fn a_moved_run_says_where_it_went_and_opens_that_tab(cx: &mut TestAppContext) {
    let (on_run, said) = recorder();
    let cx = open(cx, TWO_BLOCKS, 760., vec![live("moved")], Some((on_run, true)));
    assert!(cx.debug_bounds("run-send").is_none(), "a moved run has no output to send");
    click(cx, "run-open");
    assert_eq!(*said.lock().unwrap(), vec![RunAction::Open(live("moved"))]);
}

#[gpui_kit::test]
fn a_refused_run_is_said_under_its_block(cx: &mut TestAppContext) {
    let refuse: OnRun = Arc::new(|action, _, cx| {
        if let RunAction::Start { said, .. } = action {
            said.update(cx, |said, cx| {
                *said = Some("this workspace's terminal is not connected yet".into());
                cx.notify();
            });
        }
    });
    let cx = open(cx, TWO_BLOCKS, 760., Vec::new(), Some((refuse, true)));
    assert!(cx.debug_bounds("run-refusal").is_none());
    click(cx, "code-run");
    let said = cx.debug_bounds("run-refusal").expect("the refusal said under the block");
    assert!(said.top() > bounds(cx, "code-run").bottom(), "the refusal stands under the block's head");
}

#[gpui_kit::test]
fn wrap_toggles_and_unwrapped_code_keeps_one_line_and_a_scrollbar_lane(cx: &mut TestAppContext) {
    let long = "word ".repeat(80);
    let cx = open(cx, &format!("```text\n{long}\n```"), 400., Vec::new(), None);
    let wrapped = bounds(cx, "reply").size.height;
    click(cx, "code-wrapped");
    assert!(cx.debug_bounds("code-unwrapped").is_some(), "the toggle reads off");
    let unwrapped = bounds(cx, "reply").size.height;
    // Header 30, the code's 12.8 padding twice, one 19.25 line, the frame's 2 px of border and the 7 px lane.
    assert!((unwrapped - px(30. + 25.6 + 19.25 + 2. + 7.)).abs() < px(1.), "one line: {unwrapped:?}, wrapped {wrapped:?}");
    assert!(wrapped > unwrapped + px(19.), "wrapped takes more lines");
}

#[gpui_kit::test]
fn a_flat_list_parts_its_items_by_4_px(cx: &mut TestAppContext) {
    let flat = {
        let cx = open(cx, "- a\n- b\n- c", 600., Vec::new(), None);
        bounds(cx, "reply").size.height
    };
    assert!((flat - px(3. * 22.75 + 2. * 4.)).abs() < px(1.), "three lines and two 4 px gaps: {flat:?}");
}

#[gpui_kit::test]
fn a_paragraph_with_inline_code_keeps_plain_line_heights(cx: &mut TestAppContext) {
    let cx = open(cx, "a `b`  \nc  \nd  \ne", 600., Vec::new(), None);
    let height = bounds(cx, "reply").size.height;
    assert!((height - px(4. * 22.75)).abs() < px(0.1), "four 22.75 px lines: {height:?}");
}

#[gpui_kit::test]
fn a_nested_list_takes_10_4_px_above_and_collapses_below(cx: &mut TestAppContext) {
    let cx = open(cx, "- a\n  - b\n- c", 600., Vec::new(), None);
    let nested = bounds(cx, "reply").size.height;
    // `ul ul { margin: 0.65rem 0 }`: 10.4 above b, and below it max(10.4, the next item's 4).
    assert!((nested - px(3. * 22.75 + 10.4 + 10.4)).abs() < px(1.), "{nested:?}");
}

#[gpui_kit::test]
fn collapsing_a_table_keeps_each_cell_to_one_line(cx: &mut TestAppContext) {
    let long = "cell ".repeat(40);
    let cx = open(cx, &format!("| a | b |\n|---|---|\n| {long} | x |"), 600., Vec::new(), None);
    let expanded = bounds(cx, "reply").size.height;
    click(cx, "table-expanded");
    assert!(cx.debug_bounds("table-collapsed").is_some());
    let collapsed = bounds(cx, "reply").size.height;
    assert!(expanded > collapsed + px(19.), "expanded wraps the 384 px cell: {expanded:?} against {collapsed:?}");
}

#[test]
fn a_fence_names_its_language_and_its_file() {
    assert_eq!(fence_language("", None), "text");
    assert_eq!(fence_language("gitignore", None), "ini");
    assert_eq!(fence_language("ts title=\"x.ts\"", None), "ts");
    assert_eq!(fence_title("ts title=\"src/a b.ts\""), Some("src/a b.ts".into()));
    assert_eq!(fence_title("rust src/main.rs"), Some("src/main.rs".into()));
    assert_eq!(fence_title("rust"), None);
    assert_eq!(fence_title("sh run it"), None);
}

#[test]
fn only_a_shell_fence_holds_a_command() {
    assert_eq!(runnable_command("sh", "ls -la\n"), Some("ls -la".into()));
    assert_eq!(runnable_command("Bash", "pwd"), Some("pwd".into()));
    assert_eq!(runnable_command("rust", "fn main() {}"), None);
    assert_eq!(runnable_command("console", "$ ls\nfile\n$ pwd\n/w"), Some("ls\npwd".into()));
    assert_eq!(runnable_command("sh", "rm -rf /\u{202e}"), None);
    assert_eq!(runnable_command("sh", "  \n"), None);
}

#[test]
fn the_sent_output_fences_past_the_outputs_own_backticks() {
    let mut run = ran("exited", Some("a ``` b"));
    assert_eq!(run_output_message(&run), "I ran `ls -la` and it exited 0. The last lines it printed:\n\n````text\na ``` b\n````");
    run.output = Some(String::new());
    assert_eq!(run_output_message(&run), "I ran `ls -la` and it exited 0. It printed nothing.");
    run.signal = Some("9".into());
    run.output = None;
    assert_eq!(run_output_message(&run), "I ran `ls -la` and stopped by signal 9. It printed nothing.");
}

#[gpui_kit::test]
fn the_copy_button_copies_the_code_and_shows_a_check_for_a_moment(cx: &mut TestAppContext) {
    let cx = open(cx, "Run it:\n\n```rust\nlet x = 1;\n```", 760., Vec::new(), None);
    let copy = cx.debug_bounds("code-copy").expect("the copy button");
    // The web's header: the button 6 px in from the frame's right edge, inside its 1 px border.
    assert_eq!(copy.right(), px(760. - 1. - 6.));
    cx.simulate_click(copy.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("let x = 1;"));
    assert!(cx.debug_bounds("code-copied").is_some(), "a check after the copy");
    cx.executor().advance_clock(std::time::Duration::from_millis(1_150));
    cx.run_until_parked();
    assert!(cx.debug_bounds("code-copied").is_some(), "the check held under 1.2 s");
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    assert!(cx.debug_bounds("code-copy").is_some(), "the copy glyph again after 1.2 s");
}

#[test]
fn fence_languages_share_one_parser_per_registered_language() {
    let theme = gpui_kit::component::highlighter::HighlightTheme::default_light();
    for n in 0..200 {
        super::styles(&format!("made-up-{n}").into(), "x", &theme);
    }
    super::styles(&"rust".into(), "fn a() {}", &theme);
    super::styles(&"rs".into(), "fn a() {}", &theme);
    let held = super::PARSERS.with(|p| p.borrow().len());
    assert!(held <= 2, "{held} parsers for two languages and 200 unknown names");
}

#[gpui_kit::test]
fn a_grammar_compiles_off_the_ui_thread_and_its_blocks_highlight_once_it_lands(cx: &mut TestAppContext) {
    let theme = gpui_kit::component::highlighter::HighlightTheme::default_light();
    let code = "def total(rows)\n  rows.sum(&:height)\nend";
    let landed = std::rc::Rc::new(std::cell::Cell::new(0));
    let (before, light) = (super::LANDED.with(|l| l.get()), Arc::new(theme.clone()));
    let held = super::highlighter(&light);
    cx.update(|cx| {
        let landed = landed.clone();
        super::warm(["ruby", "rb"], move |_| landed.set(landed.get() + 1), cx)
    });
    assert!(super::warming("ruby"), "the grammar did not start compiling");
    assert!(super::styles(&"ruby".into(), code, &theme).is_empty(), "a block compiled its grammar on the UI thread while it drew");
    cx.run_until_parked();
    assert_eq!(landed.get(), 1, "one language's two names compiled twice");
    assert!(!super::warming("ruby") && super::LANDED.with(|l| l.get()) > before);
    assert!(!super::styles(&"ruby".into(), code, &theme).is_empty(), "the landed grammar highlights nothing");
    assert!(!Arc::ptr_eq(&held, &super::highlighter(&light)), "a block drawn plain while its grammar compiled keeps its plain styles");
}

#[test]
fn a_grammar_landing_on_another_thread_keeps_this_threads_highlighter() {
    let light = Arc::new(gpui_kit::component::highlighter::HighlightTheme::default_light());
    let held = super::highlighter(&light);
    std::thread::spawn(|| {
        let cx = gpui_kit::TestAppContext::single();
        cx.update(|cx| super::warm(["ruby"], |_| {}, cx));
        cx.run_until_parked();
        assert!(super::LANDED.with(|l| l.get()) > 0, "the other thread landed no grammar");
    })
    .join()
    .unwrap();
    assert!(Arc::ptr_eq(&held, &super::highlighter(&light)), "a grammar landing on another thread rebuilt this thread's highlighter");
}

#[test]
fn a_fence_names_its_language_for_the_warm_up() {
    let langs: Vec<&str> = super::fence_langs("Text\n```rust\nfn a() {}\n```\n  ~~~py title\nx\n~~~\n```\nplain\n```").collect();
    assert_eq!(langs, ["rust", "py"]);
}

#[test]
fn a_single_newline_breaks_the_line_and_a_blank_line_still_ends_the_paragraph() {
    assert_eq!(person_text("one\ntwo"), "one  \ntwo");
    assert_eq!(person_text("one\n\ntwo"), "one\n\ntwo");
    assert_eq!(person_text("one  \ntwo"), "one  \ntwo");
    assert_eq!(person_text("one\\\ntwo"), "one\\\ntwo");
    assert_eq!(person_text("last line\n"), "last line");
}

#[test]
fn fenced_code_keeps_its_lines_and_its_tags() {
    let raw = "look:\n```html\n<b>x</b>\nline\n```\nafter <i>";
    assert_eq!(person_text(raw), "look:  \n```html\n<b>x</b>\nline\n```\nafter \\<i>");
    let tildes = "~~~\n<a>\n```\n<b>\n~~~\n<c>";
    assert_eq!(person_text(tildes), "~~~\n<a>\n```\n<b>\n~~~\n\\<c>");
}

#[test]
fn a_tag_shows_as_typed_but_code_spans_and_autolinks_stay() {
    assert_eq!(person_text("use <div> here"), "use \\<div> here");
    assert_eq!(person_text("`<div>` and <p>"), "`<div>` and \\<p>");
    assert_eq!(person_text("``a ` <b>`` <c>"), "``a ` <b>`` \\<c>");
    assert_eq!(person_text("see <https://wsp.dev/a>"), "see <https://wsp.dev/a>");
    assert_eq!(person_text("a \\<b> c"), "a \\<b> c");
    assert_eq!(person_text("1 < 2 é <x"), "1 \\< 2 é \\<x");
}

#[test]
fn only_a_leading_atx_heading_counts() {
    assert!(opens_with_heading("# Title\n\nbody"));
    assert!(opens_with_heading("\n## Sub"));
    assert!(opens_with_heading("###"));
    assert!(!opens_with_heading("#hashtag"));
    assert!(!opens_with_heading("body\n# later"));
    assert!(!opens_with_heading("    # code"));
    assert!(!opens_with_heading("####### seven"));
}

/// A reply of `markdown` as `open` draws it, and its rendered text.
fn rendered(cx: &mut TestAppContext, markdown: &str, width: f32) -> (String, Pixels) {
    cx.update(gpui_kit::init);
    cx.update(|cx| crate::ui::init(Side::Light, cx));
    another(cx, markdown, width)
}

/// `rendered` in a window of its own, once the app is set up.
fn another(cx: &mut TestAppContext, markdown: &str, width: f32) -> (String, Pixels) {
    let markdown = markdown.to_owned();
    let body = cx.new(|cx| TextViewState::markdown(&markdown, cx));
    let reply = cx.new(|_| Reply(body.clone(), Vec::new(), None));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(reply.clone(), window, cx));
    cx.simulate_resize(size(px(width), px(800.)));
    cx.run_until_parked();
    let text = body.read_with(cx, |body, _| body.rendered_text().as_str().to_owned());
    (text, bounds(cx, "reply").size.height)
}

#[gpui_kit::test]
fn footnotes_number_by_first_reference_and_gather_at_the_end(cx: &mut TestAppContext) {
    let (text, _) = rendered(cx, "One[^b] and two[^a].\n\n[^a]: Alpha.\n\n[^b]: Beta.\n\n[^c]: Never cited.\n\nAfter.", 600.);
    assert!(!text.contains('['), "a footnote drawn as its source: {text:?}");
    assert!(text.starts_with("One1 and two2.\nAfter."), "{text:?}");
    let (beta, alpha) = (text.find("Beta.").expect("Beta"), text.find("Alpha.").expect("Alpha"));
    assert!(beta < alpha, "numbered by first reference: {text:?}");
    assert!(text.trim_end().ends_with("Beta. \u{21a9}\nAlpha. \u{21a9}"), "each definition leads back: {text:?}");
    assert!(!text.contains("Never cited"), "an uncited definition drawn: {text:?}");
}

#[gpui_kit::test]
fn an_alert_draws_its_title_over_its_body_and_drops_its_marker(cx: &mut TestAppContext) {
    let (text, height) = rendered(cx, "> [!NOTE]\n> Keep it short.", 600.);
    assert_eq!(text.trim_end(), "Keep it short.");
    // The title's line, the title paragraph's 0.65rem margin, the body's line.
    assert!((height - px(22.75 + 10.4 + 22.75)).abs() < px(1.), "{height:?}");
}

#[gpui_kit::test]
fn a_marker_with_words_after_it_stays_a_quote(cx: &mut TestAppContext) {
    let (text, _) = rendered(cx, "> [!NOTE] aside", 600.);
    assert_eq!(text.trim_end(), "[!NOTE] aside");
}

#[gpui_kit::test]
fn a_wide_table_keeps_each_cell_to_its_content_and_scrolls_sideways(cx: &mut TestAppContext) {
    let table = |cell: &str| {
        let row = |c: &str| format!("|{}\n", (0..12).map(|_| format!(" {c} |")).collect::<String>());
        format!("{}|{}\n{}", row("h"), "---|".repeat(12), row(cell))
    };
    let short = rendered(cx, &table("a"), 600.).1;
    let long = another(cx, &table("apps/desktop-gpui/app/src/thread_view.rs"), 600.).1;
    assert_eq!(long, short, "a path cell wrapped in a 600 px window rather than scrolling");
}

struct InFolder(Entity<TextViewState>, Option<&'static str>);

impl Render for InFolder {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(div().debug_selector(|| "reply".into()).child(super::reply(&self.0, false, None, self.1, None, cx)))
    }
}

fn open_in<'a>(cx: &'a mut TestAppContext, markdown: &str, cwd: Option<&'static str>) -> (Entity<InFolder>, &'a mut VisualTestContext) {
    cx.update(gpui_kit::init);
    cx.update(|cx| crate::ui::init(Side::Light, cx));
    let markdown = markdown.to_owned();
    let body = cx.new(|cx| TextViewState::markdown(&markdown, cx));
    let reply = cx.new(|_| InFolder(body, cwd));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(reply.clone(), window, cx));
    cx.simulate_resize(size(px(760.), px(800.)));
    cx.run_until_parked();
    (reply, cx)
}

#[gpui_kit::test]
fn a_path_in_inline_code_draws_as_a_file_chip_and_an_identifier_stays_code(cx: &mut TestAppContext) {
    let (reply, cx) = open_in(cx, "See `src/main.rs:12` and `node.meta`.", Some("/w"));
    assert!(cx.debug_bounds("file-chip").is_some(), "no file chip for src/main.rs:12");
    let text = reply.read_with(cx, |r, cx| r.0.read(cx).rendered_text().as_str().to_owned());
    assert!(text.contains("main.rs L12"), "the chip reads its name and line: {text:?}");
    assert!(!text.contains("src/main.rs:12"), "the whole path still shows: {text:?}");
    assert!(text.contains("node.meta"), "the identifier stays inline code: {text:?}");
}

#[test]
fn a_folder_matched_in_another_case_never_cuts_a_character() {
    let meta = super::inline_file("/w/wor\u{212a}/x\u{e9}.rs", Some("/w/work")).expect("an absolute path");
    assert_eq!(meta.relative.as_deref(), Some("x\u{e9}.rs"));
}

#[test]
fn inline_code_names_a_file_by_the_web_apps_rules() {
    use super::inline_file;
    let meta = inline_file("src/main.rs:12:3", Some("/w")).expect("a relative path under the folder");
    assert_eq!(
        (meta.target.as_str(), meta.relative.as_deref(), meta.basename.as_str()),
        ("/w/src/main.rs:12:3", Some("src/main.rs"), "main.rs")
    );
    assert_eq!((meta.line, meta.column), (Some(12), Some(3)));
    assert_eq!(inline_file("src/main.rs", None), None, "a relative path needs the folder");
    assert_eq!(inline_file("/etc/hosts", None).map(|m| m.basename), Some("hosts".into()));
    assert_eq!(inline_file("node.meta", Some("/w")), None);
    assert_eq!(inline_file("origin/main", Some("/w")), None);
    assert_eq!(inline_file("github.com/a/b.rs", Some("/w")), None, "a host is not a folder");
    assert_eq!(inline_file("Makefile:12", Some("/w")).map(|m| m.target), Some("/w/Makefile:12".into()));
    assert_eq!(inline_file("a b/c.rs", Some("/w")), None);
    let suffixes = super::parent_suffixes(&["/w/a/x/mod.rs".into(), "/w/b/x/mod.rs".into(), "/w/c.rs".into()]);
    assert_eq!(suffixes.get("/w/a/x/mod.rs").map(String::as_str), Some("a/x"));
    assert_eq!(suffixes.get("/w/c.rs"), None);
}

const TABLE: &str = "| name | note |\n|---|---|\n| a, b | say \"hi\" |";

/// The text a reply in `open_in` reads as.
fn text_of(reply: &Entity<InFolder>, cx: &mut VisualTestContext) -> String {
    reply.read_with(cx, |r, cx| r.0.read(cx).rendered_text().as_str().to_owned())
}

/// A 1 by 1 PNG.
const DOT: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

#[gpui_kit::test]
fn an_image_the_app_cannot_draw_says_so_with_its_alt_and_a_data_image_still_decodes(cx: &mut TestAppContext) {
    let (reply, cx) = open_in(cx, "See ![a cat](https://example.com/cat.png) here.", None);
    let fallback = cx.debug_bounds("image-unavailable").expect("a web image drew no Image unavailable");
    let line = bounds(cx, "reply");
    // Chromium sets the paragraph's one line by the fallback, stood on its glyph's bottom edge: the line's top is
    // the fallback's, and it is as tall, 29 px in Liberation Sans. The Mac's faces round it half a pixel apart.
    let off = ((fallback.top() - line.top()).abs(), (fallback.size.height - line.size.height).abs());
    assert!(off.0 <= px(0.5) && off.1 <= px(0.5), "the fallback stands off its line: {fallback:?} on {line:?}");
    assert!(fallback.size.height > px(26.), "`py-1` round a 16 px line, and the box's own line below it");
    assert!(text_of(&reply, cx).contains("Image unavailable: a cat"), "the alt text was lost: {:?}", text_of(&reply, cx));
    let (reply, cx) = open_in(cx, &format!("![dot]({DOT}) and ![]({})", "data:image/png;base64,***"), None);
    let text = text_of(&reply, cx);
    assert!(!text.contains("Image unavailable: dot"), "a data image the app decodes fell back: {text:?}");
    assert!(text.contains("Image unavailable"), "a data image that does not decode drew nothing: {text:?}");
}

#[gpui_kit::test]
fn the_second_chip_for_one_file_shows_its_path_on_hover(cx: &mut TestAppContext) {
    // The last chip drawn is the one `file-chip` finds: here the second chip naming the file.
    let (_, cx) = open_in(cx, "First `src/x.rs` then `src/x.rs` again.", Some("/w"));
    let chip = bounds(cx, "file-chip");
    cx.simulate_mouse_move(chip.center(), None, Modifiers::none());
    cx.executor().advance_clock(std::time::Duration::from_millis(600));
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("file-chip-tip").is_some(), "the second chip for one file showed no tooltip");
}

#[gpui_kit::test]
fn the_table_footer_stands_2_px_under_the_table(cx: &mut TestAppContext) {
    let cx = open(cx, TABLE, 600., Vec::new(), None);
    let (reply, copy) = (bounds(cx, "reply"), bounds(cx, "table-copy"));
    // The head row's 8.8 px over and under its 19.5 px line, the body row's 7.2, the table's 1 px foot rule, then `mt-0.5`.
    let table = px(8.8 + 19.5 + 8.8) + px(7.2 + 19.5 + 7.2) + px(1.);
    let at = copy.top() - reply.top();
    assert!((at - table - px(2.)).abs() < px(0.5), "the footer stands {at:?} under the reply's top");
}

#[gpui_kit::test]
fn table_copy_offers_markdown_and_csv_and_reads_copied_for_a_moment(cx: &mut TestAppContext) {
    let cx = open(cx, TABLE, 600., Vec::new(), None);
    let built = super::CSV_BUILT.with(|built| built.get());
    for _ in 0..3 {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    assert_eq!(super::CSV_BUILT.with(|b| b.get()), built, "a drawn table built its CSV unasked");
    click(cx, "table-copy");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), None, "Escape closed the menu, so its keys copied nothing");
    click(cx, "table-copy");
    cx.simulate_keystrokes("down down enter");
    cx.run_until_parked();
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()).as_deref(),
        Some("name,note\n\"a, b\",\"say \"\"hi\"\"\""),
        "the second item, Copy as CSV"
    );
    assert_eq!(super::CSV_BUILT.with(|b| b.get()), built + 1, "the CSV was built other than once, on the press");
    assert!(cx.debug_bounds("table-copied").is_some(), "no Copied after the copy");
    cx.executor().advance_clock(std::time::Duration::from_millis(1_150));
    cx.run_until_parked();
    assert!(cx.debug_bounds("table-copied").is_some(), "Copied held under 1.2 s");
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    assert!(cx.debug_bounds("table-copy").is_some(), "the copy glyph again after 1.2 s");
    click(cx, "table-copy");
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()).as_deref(),
        Some("| name | note |\n| :-- | :-- |\n| a, b | say \"hi\" |"),
        "the first item, Copy as Markdown"
    );
}

#[gpui_kit::test]
fn a_runs_output_draws_no_line_for_its_trailing_newline(cx: &mut TestAppContext) {
    let (on_run, _) = recorder();
    let height = |cx: &mut TestAppContext, output: &str| {
        let cx = open(cx, TWO_BLOCKS, 760., vec![ran("exited", Some(output))], Some((on_run.clone(), false)));
        bounds(cx, "code-run-output").size.height
    };
    let bare = height(cx, "total 0");
    let ended = height(cx, "total 0\n");
    assert_eq!(ended, bare, "the trailing newline drew an empty line");
}

#[gpui_kit::test]
fn a_replys_style_is_built_once_across_frames(cx: &mut TestAppContext) {
    let (reply, cx) = open_in(cx, "Some *text*.", None);
    let built = || super::STYLES_BUILT.with(|b| b.get());
    let after_first = built();
    for _ in 0..5 {
        reply.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert_eq!(built(), after_first, "the style was built again on later frames");
}

#[gpui_kit::test]
fn the_text_size_preference_sets_the_replys_text(cx: &mut TestAppContext) {
    let (reply, cx) = open_in(cx, "a", None);
    assert!((bounds(cx, "reply").size.height - px(14. * 1.625)).abs() < px(0.5));
    cx.update(|_, cx| super::set_sizes(&serde_json::json!({"textSize": 17}), cx));
    reply.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let height = bounds(cx, "reply").size.height;
    assert!((height - px(17. * 1.625)).abs() < px(0.5), "a 17 px line is 27.6 px: {height:?}");
    cx.update(|_, cx| {
        assert_eq!(super::code_size(cx), px(17.), "code takes the text size where none is picked");
        assert_eq!(super::diff_size(cx), px(13.));
    });
}

#[test]
fn the_same_code_is_highlighted_once() {
    let code = format!("echo {}", std::process::id());
    let parsed = || super::PARSED.with(|p| p.get());
    let before = parsed();
    let theme = gpui_kit::component::highlighter::HighlightTheme::default_light();
    let first = super::highlight_code("bash", &code, &theme);
    let second = super::highlight_code("bash", &code, &theme);
    assert_eq!(first, second);
    assert_eq!(parsed() - before, 1, "the second call parsed again");
}

#[gpui_kit::test]
fn code_takes_its_colors_from_the_active_theme(cx: &mut TestAppContext) {
    use gpui_kit::component::ActiveTheme as _;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(Side::Dark, cx);
        let theme = cx.theme().highlight_theme.clone();
        // A Python parameter is the theme's own `variable.parameter.python`; a fixed table falls back to `variable`.
        let code = "def scale(limit):\n    return limit\n";
        let at = code.find("limit").unwrap();
        let color_at = |spans: Vec<(std::ops::Range<usize>, gpui_kit::HighlightStyle)>| {
            spans.into_iter().find(|(range, _)| range.contains(&at)).and_then(|(_, style)| style.color)
        };
        let parameter = theme.style("variable.parameter.python").and_then(|s| s.color);
        assert!(parameter.is_some(), "Graphite names a Python parameter's color");
        assert_ne!(parameter, theme.style("variable").and_then(|s| s.color), "the parameter and a variable share a color");
        assert_eq!(color_at(super::highlight_code("python", code, &theme)), parameter, "a parameter in a command or a diff");
        let block = gpui_kit::base::text::CodeBlock::from_code(code, Some("python"));
        assert_eq!(color_at(super::highlighter(&theme)(&block)), parameter, "a parameter in a fenced block");
    });
}

#[gpui_kit::test]
fn a_fenced_block_is_colored_again_when_the_theme_changes(cx: &mut TestAppContext) {
    use gpui_kit::component::ActiveTheme as _;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(Side::Light, cx);
        let paper = super::highlighter(&cx.theme().highlight_theme);
        assert!(Arc::ptr_eq(&paper, &super::highlighter(&cx.theme().highlight_theme)), "one highlighter per theme");
        crate::ui::theme::set_theme("graphite", cx);
        assert!(!Arc::ptr_eq(&paper, &super::highlighter(&cx.theme().highlight_theme)), "Graphite's code kept Paper's colors");
        crate::ui::theme::set_theme("paper", cx);
        assert!(Arc::ptr_eq(&paper, &super::highlighter(&cx.theme().highlight_theme)), "Paper again built a second highlighter");
    });
}
