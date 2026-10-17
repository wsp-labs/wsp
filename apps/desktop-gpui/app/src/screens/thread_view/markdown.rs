// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript's markdown as `ChatMarkdown` draws it: `.chat-markdown` in index.css, `codeBlocks.tsx`, Shiki's pierre.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use gpui_kit::base::text::{
    markdown_ast as mdast, CodeBlock, CodeBlockChrome, InlineCodeFrame, LinkIcon, ListStyleType, MarkdownNode, MarkdownParseContext,
    MarkdownPlugin, TableChrome, TableData, TextView, TextViewState, TextViewStyle,
};
use gpui_kit::base::{ScrollbarStyles, StyledExt as _};
use gpui_kit::component::highlighter::{HighlightTheme, LanguageRegistry, SyntaxHighlighter};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Rope, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, relative, rems, App, ClipboardItem, Entity, EntityId, FontFeatures, FontWeight, HighlightStyle, Hsla, Overflow,
    Pixels, SharedString, StyleRefinement, Window,
};

use super::math;
use crate::transcript::{block_at, Run};
use crate::ui::{ActivePalette, Glyph, Palette};

/// `--font-size-chat`: the body, and the code a fence holds.
const BODY: Pixels = px(14.);
/// `leading-relaxed`, unitless, so a table's 12 px cells take 19.5 px lines.
const LEADING: f32 = 1.625;
/// `margin: 0.65rem 0` on every block; margins collapse, so it is also the gap between two.
const GAP: f32 = 0.65;
const GAP_PX: Pixels = px(10.4);
/// A heading's `margin-top: 1.25rem`, less the gap the block above already leaves.
const HEADING_TOP: Pixels = px(20. - 10.4);
/// `ChatMarkdown`'s `wordWrap`, which the timeline leaves at its default: code wraps and tables open expanded.
const WORD_WRAP: bool = true;
/// `max-width: 24rem` on a table's cells.
const CELL_CAP: Pixels = px(384.);
/// InlineRun's `RUN_BOX_PX`: the most of a run's output shown before it scrolls.
const RUN_BOX: Pixels = px(232.);

/// What a press under a reply's shell block asks the thread view for.
#[derive(Clone, Debug, PartialEq)]
pub enum RunAction {
    /// Run: the block's index among the reply's fenced blocks (`Run::block`) and the command it holds.
    Start { block: usize, command: String },
    /// Send output to the agent: the person's message `runOutputMessage` writes.
    Send { message: String },
}

/// Takes the presses under a reply's shell blocks.
pub type OnRun = Arc<dyn Fn(RunAction, &mut Window, &mut App) + Send + Sync>;

/// A settled reply's shell blocks (`ReplyRunContext`): their runs, what a press under them asks for, and whether Run starts.
pub struct ReplyRuns<'a> {
    pub runs: &'a [Run],
    pub on_run: OnRun,
    pub can_start: bool,
}

/// A row's parsed document as a person's bubble or an agent's reply; `runs` puts each shell block's run under it.
pub fn view(state: &Entity<TextViewState>, person: bool, runs: Option<ReplyRuns<'_>>, cx: &App) -> TextView {
    let dark = cx.theme().is_dark();
    let style = if person { person_style(cx) } else { style(cx) };
    let source = state.read(cx).rendered_text().source().to_owned();
    let opens_with_heading = opens_with_heading(&source);
    let reply = Arc::new(match runs {
        Some(ReplyRuns { runs, on_run, can_start }) => {
            Reply { view: state.entity_id(), source, runs: runs.to_vec(), on_run: Some(on_run), can_start }
        }
        None => Reply { view: state.entity_id(), source, runs: Vec::new(), on_run: None, can_start: false },
    });
    let view = state.entity_id();
    TextView::new(state)
        .style(style)
        .font_family(cx.theme().font_family.clone())
        .text_size(BODY)
        .line_height(relative(LEADING))
        .when(opens_with_heading, |view| view.mt(-HEADING_TOP))
        .shared_code_block_highlighter(highlighter(dark))
        .code_block_chrome(move |block, window, cx| chrome(&reply, block, window, cx))
        .table_chrome(move |table, window, cx| table_chrome(view, table, window, cx))
        .plugin(Rule)
        .plugin(math::DisplayMath { body: BODY, gap: GAP_PX })
        .plugin(math::InlineMath)
}

/// The reply's style: body ink at 80% of the foreground.
pub fn style(cx: &App) -> TextViewStyle {
    styled(cx.palette(), cx.theme().is_dark(), 0.8, cx)
}

/// A person's bubble, `text-message-foreground`: the full foreground.
pub fn person_style(cx: &App) -> TextViewStyle {
    styled(cx.palette(), cx.theme().is_dark(), 1., cx)
}

fn styled(p: &Palette, dark: bool, ink: f32, cx: &App) -> TextViewStyle {
    let foreground = p.foreground;
    let muted = p.muted_foreground;
    let mono = cx.theme().mono_font_family.clone();
    let rule = p.border.opacity(0.6);

    let mut code_block = StyleRefinement::default()
        .rounded(px(10.))
        .border_1()
        .border_color(if dark { gpui_kit::transparent_black() } else { p.border.opacity(0.7) })
        .bg(if dark { p.input.opacity(0.32) } else { p.muted })
        .px(px(14.4))
        .py(px(12.8))
        .font_family(mono)
        .text_size(BODY)
        .line_height(relative(1.375))
        .text_color(gpui_kit::rgb(pierre(dark).foreground));
    // The frame clips its header, its code and its run to the rounded corners.
    code_block.overflow.x = Some(Overflow::Hidden);
    code_block.overflow.y = Some(Overflow::Hidden);

    let mut table =
        StyleRefinement::default().border_0().border_b_1().border_color(rule).bg(gpui_kit::transparent_black()).text_size(px(12.));
    table.overflow.x = Some(Overflow::Scroll);
    // The refinement styles the header row around its cells, so it pads the cells' 7.2 px up to the web's 8.8.
    let head = StyleRefinement::default().py(px(8.8 - 7.2)).font_weight(FontWeight::SEMIBOLD).bg(gpui_kit::transparent_black());
    let cell =
        StyleRefinement::default().px(px(12.)).py(px(7.2)).text_size(px(12.)).border_color(gpui_kit::transparent_black()).max_w(CELL_CAP);
    let ink_color = foreground.opacity(ink);
    // `ol > li::marker { font-variant-numeric: tabular-nums }`; a marker's trailing space sets it off the text.
    let mut marker = StyleRefinement::default().min_w(px(20.)).pr(px(3.9)).text_color(ink_color);
    marker.text.font_features = Some(FontFeatures(Arc::new(vec![("tnum".into(), 1)])));
    let (box_edge, box_fill, box_checked, check) = checkbox_colors(dark);

    TextViewStyle::default()
        .with_foreground(foreground.opacity(ink))
        .with_muted_foreground(muted)
        .with_link(p.info_foreground)
        .with_selection(cx.theme().selection)
        .with_code_background(gpui_kit::transparent_black())
        .with_border(rule)
        .with_paragraph_gap(rems(GAP))
        .with_heading(move |level| {
            let size = match level {
                1 => px(20.),
                2 => px(18.),
                3 => px(16.),
                _ => px(14.),
            };
            StyleRefinement::default()
                .text_size(size)
                .line_height(relative(1.3))
                .text_color(if level == 6 { muted } else { foreground })
                .pt(HEADING_TOP)
                .pb(GAP_PX)
        })
        .with_code_block(code_block)
        .with_table(table)
        .with_table_head(head)
        .with_table_cell(cell)
        .with_inline_code(HighlightStyle { color: Some(foreground), background_color: Some(p.muted), ..Default::default() })
        // `:not(pre) > code`: 12 px in a 6 px rounded box, a 1 px border outside its 0.1rem 0.35rem padding.
        .with_inline_code_frame(InlineCodeFrame {
            scale: 12. / 14.,
            size: Some(px(12.)),
            padding_x: px(5.6 + 1.),
            padding_y: Some(px(1.6 + 1.)),
            radius: Some(px(6.)),
            border: Some((px(1.), p.border)),
        })
        .with_link_underline(false)
        // `MarkdownLinkGlyph`: a 14 px globe, `0.25em` before it, `0.2em` after, its bottom `0.125em` under the baseline.
        .with_link_icon(LinkIcon {
            path: Glyph::Globe.path(),
            size: px(14.),
            margin_left: BODY * 0.25,
            margin_right: BODY * 0.2,
            below_baseline: BODY * 0.125,
        })
        .with_link_hover_dots(true)
        .with_list_indent(px(20.))
        .with_list_item_gap(px(4.))
        .with_nested_list_gap(GAP_PX)
        .with_list_marker(marker)
        .with_list_markers(
            [ListStyleType::Disc, ListStyleType::Circle, ListStyleType::Square],
            [ListStyleType::Decimal, ListStyleType::LowerAlpha, ListStyleType::LowerRoman],
        )
        // Chromium's disabled checkbox: 13 px, a 1 px edge, then the 0.35em margin and the space before the text.
        .with_task_checkbox(
            StyleRefinement::default().size(px(13.)).rounded(px(2.)).border_1().border_color(box_edge).bg(box_fill).mr(px(4.9 + 3.9)),
        )
        .with_checked_task_checkbox(StyleRefinement::default().border_0().bg(box_checked).text_color(check))
        .with_muted_done_tasks(false)
        .with_dark(dark)
}

/// Chromium's disabled checkbox, which the web's task list draws: its edge, its fill, its checked fill and its check.
fn checkbox_colors(dark: bool) -> (Hsla, Hsla, Hsla, Hsla) {
    let grey = |v: u32| gpui_kit::rgb(v * 0x010101).into();
    if dark {
        (grey(0x62), grey(0x3b), grey(0x75), grey(0x3b))
    } else {
        (grey(0xd0), grey(0xf7), grey(0xd0), grey(0xec))
    }
}

/// Whether the first block is an ATX heading, whose top margin the web drops as `:first-child`.
fn opens_with_heading(source: &str) -> bool {
    let line = source.trim_start_matches(['\n', '\r']).lines().next().unwrap_or("");
    let indent = line.len() - line.trim_start_matches(' ').len();
    let hashes = line.trim_start_matches(' ').bytes().take_while(|&b| b == b'#').count();
    indent < 4 && (1..=6).contains(&hashes) && line.trim_start_matches(' ')[hashes..].chars().next().is_none_or(|c| c == ' ' || c == '\t')
}

/// What a reply's code blocks need from the view: its source, to name a block and find its run, and Run.
struct Reply {
    view: EntityId,
    source: String,
    runs: Vec<Run>,
    on_run: Option<OnRun>,
    can_start: bool,
}

/// `MarkdownCodeBlock`: the header row across the block, the code, and under a shell block its run.
fn chrome(reply: &Reply, block: &CodeBlock, window: &mut Window, cx: &mut App) -> CodeBlockChrome {
    let p = cx.palette().clone();
    let start = block.span.map_or(0, |span| span.start);
    let info = fence_info(&reply.source, start);
    let language = fence_language(&info, block.lang());
    let title = fence_title(&info);
    let code = block.code();
    let wrapped = window.use_keyed_state(block_key("wsp-code-wrap", reply.view, start), cx, |_, _| WORD_WRAP);
    let copied = window.use_keyed_state(block_key("wsp-code-copied", reply.view, start), cx, |_, _| false);
    let (wrap, done) = (*wrapped.read(cx), *copied.read(cx));
    let index = || block_at(&reply.source, reply.source[..start.min(reply.source.len())].encode_utf16().count());
    let command = runnable_command(&language, &code);
    let run = command.as_ref().and_then(|_| index()).and_then(|ix| reply.runs.iter().find(|run| run.block == ix));
    let mono = cx.theme().mono_font_family.clone();

    let label = h_flex()
        .min_w_0()
        .gap(px(5.6))
        .font_family(mono)
        .text_size(px(11.))
        .line_height(relative(1.375))
        .when_some(title.clone(), |el, _| el.child(Icon::new(Glyph::File).with_size(px(14.)).flex_none()))
        .child(div().min_w_0().truncate().child(title.unwrap_or(language)));
    let mut actions = h_flex().flex_none().gap(px(2.));
    if let (Some(command), Some(on_run), true) = (command, reply.on_run.clone(), reply.can_start) {
        let running = run.is_some_and(|run| run.state == "running");
        let ix = index();
        actions = actions.child(
            ghost_button("run", Glyph::Play, false, cx)
                .debug_selector(|| "code-run".into())
                .when(running, |el| el.opacity(0.64).cursor_default())
                .when(!running, |el| {
                    el.on_click(move |_, window, cx| {
                        if let Some(block) = ix {
                            on_run(RunAction::Start { block, command: command.clone() }, window, cx);
                        }
                    })
                }),
        );
    }
    actions = actions
        .child(
            ghost_button("wrap", Glyph::TextWrap, wrap, cx)
                .debug_selector(move || if wrap { "code-wrapped" } else { "code-unwrapped" }.into())
                .on_click(move |_, _, cx| {
                    wrapped.update(cx, |wrapped, cx| {
                        *wrapped = !*wrapped;
                        cx.notify();
                    })
                }),
        )
        .child(
            ghost_button("copy", if done { Glyph::Check } else { Glyph::Copy }, false, cx)
                .debug_selector(move || if done { "code-copied" } else { "code-copy" }.into())
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
                    copied.update(cx, |copied, cx| {
                        *copied = true;
                        cx.notify();
                        cx.spawn(async move |copied, cx| {
                            cx.background_executor().timer(Duration::from_millis(1200)).await;
                            copied.update(cx, |copied, cx| {
                                *copied = false;
                                cx.notify();
                            })
                        })
                        .detach();
                    });
                }),
        );
    let header = h_flex()
        .w_full()
        .justify_between()
        .gap(px(8.))
        .pt(px(6.))
        .pr(px(6.))
        .pl(px(12.))
        .text_color(p.foreground.opacity(0.72))
        .child(label)
        .child(actions);

    let thumb = p.border.opacity(0.78);
    let chrome = CodeBlockChrome::new().header(header).wrap(wrap).scrollbar(
        px(7.),
        ScrollbarStyles::default()
            .track(|t| t.width(px(7.)).bg(gpui_kit::transparent_black()).border_color(gpui_kit::transparent_black()))
            .thumb(|t| t.width(px(7.)).inset(px(0.)).radius(px(3.5)).bg(thumb))
            .thumb_hover(|t| t.width(px(7.)).inset(px(0.)).radius(px(3.5)).bg(thumb))
            .thumb_active(|t| t.width(px(7.)).inset(px(0.)).radius(px(3.5)).bg(thumb)),
    );
    match (run, reply.on_run.clone()) {
        (Some(run), Some(on_run)) => chrome.footer(run_view(run, on_run, &p, cx)),
        _ => chrome,
    }
}

/// The element state of one of a view's blocks, which starts at byte `start` of its source.
fn block_key(name: &str, view: EntityId, start: usize) -> gpui_kit::ElementId {
    gpui_kit::ElementId::Name(format!("{name}-{view}-{start}").into())
}

/// InlineRun: an ended run's output and how it ended, or Running; Send output to the agent as a ghost `xs` button.
fn run_view(run: &Run, on_run: OnRun, p: &Palette, cx: &App) -> impl IntoElement {
    let mono = cx.theme().mono_font_family.clone();
    let output = run.output.clone().filter(|output| run.state == "exited" && !output.is_empty());
    let mut tabular = StyleRefinement::default();
    tabular.text.font_features = Some(FontFeatures(Arc::new(vec![("tnum".into(), 1)])));
    let button = |id: &'static str, words: &'static str| {
        h_flex()
            .id(id)
            .h(px(24.))
            .px(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(gpui_kit::transparent_black())
            .text_color(p.muted_foreground)
            .cursor_pointer()
            .hover(|s| s.bg(p.accent).text_color(p.foreground))
            .child(words)
    };
    v_flex()
        .w_full()
        .debug_selector(|| "code-run-output".into())
        .when_some(output, |el, output| {
            el.child(
                div()
                    .id("run-output")
                    .w_full()
                    .max_h(RUN_BOX)
                    .overflow_y_scroll()
                    .bg(p.background)
                    .px(px(12.))
                    .py(px(8.))
                    .font_family(mono)
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(p.foreground)
                    .refine_style(&tabular)
                    .child(output),
            )
        })
        .child(
            h_flex()
                .w_full()
                .min_h(px(36.))
                .justify_between()
                .gap(px(12.))
                .py(px(4.))
                .pr(px(6.))
                .pl(px(12.))
                .font_family(cx.theme().font_family.clone())
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(p.muted_foreground)
                .refine_style(&tabular)
                .child(div().min_w_0().child(run.words()))
                .child(h_flex().gap(px(4.)).when(run.state == "exited", |el| {
                    let message = run_output_message(run);
                    el.child(
                        button("run-send", "Send output to the agent")
                            .debug_selector(|| "run-send".into())
                            .on_click(move |_, window, cx| on_run(RunAction::Send { message: message.clone() }, window, cx)),
                    )
                })),
        )
}

/// `runOutputMessage` in packages/protocol/src/run-block.ts: the command, how it ended and the last 60 lines it printed.
fn run_output_message(run: &Run) -> String {
    let ended = if run.state == "exited" && run.signal.is_none() {
        format!("it exited {}", run.exit_code.unwrap_or(0))
    } else {
        run.words().to_lowercase()
    };
    let output = run.output.as_deref().unwrap_or("");
    let command = &run.command;
    if output.trim().is_empty() {
        return format!("I ran `{command}` and {ended}. It printed nothing.");
    }
    let lines: Vec<&str> = output.split('\n').collect();
    let tail = lines[lines.len().saturating_sub(60)..].join("\n");
    let mut longest = 0;
    let mut current = 0;
    for c in tail.chars() {
        current = if c == '`' { current + 1 } else { 0 };
        longest = longest.max(current);
    }
    let fence = "`".repeat((longest + 1).max(3));
    format!("I ran `{command}` and {ended}. The last lines it printed:\n\n{fence}text\n{tail}\n{fence}")
}

/// The fence line's info string: what follows the backticks or tildes on the line holding byte `at`.
fn fence_info(source: &str, at: usize) -> String {
    let at = at.min(source.len());
    let line_start = source[..at].rfind('\n').map_or(0, |ix| ix + 1);
    let line_end = source[at..].find('\n').map_or(source.len(), |ix| at + ix);
    let line = source[line_start..line_end].trim_start_matches([' ', '\t', '>']);
    let fence = line.chars().next().filter(|c| *c == '`' || *c == '~');
    fence.map_or_else(String::new, |c| line.trim_start_matches(c).trim().to_owned())
}

/// `extractFenceLanguage`: the info string's first word, `text` when there is none, and `ini` for `gitignore`.
fn fence_language(info: &str, parsed: Option<SharedString>) -> SharedString {
    let word = info.split_whitespace().next().map(str::to_owned).or_else(|| parsed.map(|lang| lang.to_string()));
    match word.as_deref() {
        None | Some("") => "text".into(),
        Some("gitignore") => "ini".into(),
        Some(word) => SharedString::from(word.to_owned()),
    }
}

/// `extractFenceTitle`: a `title=`, `file=` or `filename=` in the fence's meta, else its first word shaped like a file name.
fn fence_title(info: &str) -> Option<SharedString> {
    static ATTR: OnceLock<regex::Regex> = OnceLock::new();
    static FILE: OnceLock<regex::Regex> = OnceLock::new();
    let meta = info.split_once(char::is_whitespace).map(|(_, meta)| meta.trim())?;
    let attr =
        ATTR.get_or_init(|| regex::Regex::new(r#"(?i)(?:^|\s)(?:title|file(?:name)?)=(?:"([^"]+)"|'([^']+)'|(\S+))"#).expect("a regex"));
    if let Some(found) = attr.captures(meta) {
        return (1..=3).find_map(|ix| found.get(ix)).map(|m| SharedString::from(m.as_str().to_owned()));
    }
    let file = FILE.get_or_init(|| regex::Regex::new(r"^[\w@][\w@./-]*\.[A-Za-z0-9]+$").expect("a regex"));
    meta.split_whitespace().find(|word| file.is_match(word)).map(|word| SharedString::from(word.to_owned()))
}

/// `runnableCommand` in packages/protocol/src/run-block.ts: a shell fence's command; a console block's `$ ` lines.
fn runnable_command(language: &str, code: &str) -> Option<String> {
    let label = language.trim().to_lowercase();
    if !["sh", "bash", "zsh", "shell", "console"].contains(&label.as_str()) {
        return None;
    }
    let text = code.replace("\r\n", "\n");
    let hidden = |c: char| {
        (c.is_control() && c != '\t' && c != '\n')
            || matches!(c, '\u{ad}' | '\u{61c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
    };
    if text.chars().any(hidden) {
        return None;
    }
    let prompted: Vec<&str> =
        if label == "console" { text.split('\n').filter_map(|line| line.strip_prefix("$ ")).collect() } else { Vec::new() };
    let command = if prompted.is_empty() { text.trim().to_owned() } else { prompted.join("\n").trim().to_owned() };
    (!command.is_empty()).then_some(command)
}

/// `MarkdownTable`'s footer: the expand toggle on the left, copy on the right; collapsed, cells keep to one line.
fn table_chrome(view: EntityId, table: &TableData, window: &mut Window, cx: &mut App) -> TableChrome {
    let start = table.span.as_ref().map_or(0, |span| span.start);
    let expanded = window.use_keyed_state(block_key("wsp-table-expanded", view, start), cx, |_, _| WORD_WRAP);
    let open = *expanded.read(cx);
    let markdown = table.markdown.clone();
    // gpui-kit sets the footer 4 px under the table; the web's `mt-0.5` is 2.
    let footer = h_flex()
        .mt(px(-2.))
        .justify_between()
        .child(
            ghost_button("expand", if open { Glyph::Minimize2 } else { Glyph::Maximize2 }, open, cx)
                .debug_selector(move || if open { "table-expanded" } else { "table-collapsed" }.into())
                .on_click(move |_, _, cx| {
                    expanded.update(cx, |expanded, cx| {
                        *expanded = !*expanded;
                        cx.notify();
                    })
                }),
        )
        .child(ghost_button("copy", Glyph::Copy, false, cx).on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(markdown.clone()));
        }));
    let chrome = TableChrome::new().footer(footer);
    if open {
        chrome
    } else {
        chrome.cell(StyleRefinement::default().whitespace_nowrap())
    }
}

/// The ghost `icon-xs` chrome action: 24 px, a 12 px muted glyph, the accent under the pointer, and a pressed look.
pub(crate) fn ghost_button(id: &'static str, glyph: Glyph, pressed: bool, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = cx.palette();
    h_flex()
        .id(id)
        .flex_none()
        .justify_center()
        .size(px(24.))
        .rounded(px(8.))
        .border_1()
        .border_color(gpui_kit::transparent_black())
        .cursor_pointer()
        .when(pressed, |el| el.bg(p.foreground.opacity(0.08)))
        .when(!pressed, |el| el.hover(|s| s.bg(p.accent)))
        .child(Icon::new(glyph).with_size(px(12.)).text_color(if pressed { p.foreground } else { p.muted_foreground }))
}

/// The fence highlighter for a side, built once: a code block keeps its highlights only while this `Arc` is the same.
type Highlighter = dyn Fn(&CodeBlock) -> Vec<(Range<usize>, HighlightStyle)> + Send + Sync;

fn highlighter(dark: bool) -> Arc<Highlighter> {
    static SIDES: OnceLock<[Arc<Highlighter>; 2]> = OnceLock::new();
    let sides = SIDES.get_or_init(|| {
        [false, true].map(|dark| -> Arc<Highlighter> {
            let theme = Arc::new(pierre(dark).theme());
            Arc::new(move |block: &CodeBlock| highlight(block, &theme))
        })
    });
    sides[dark as usize].clone()
}

fn highlight(block: &CodeBlock, theme: &HighlightTheme) -> Vec<(Range<usize>, HighlightStyle)> {
    let Some(lang) = block.lang() else { return Vec::new() };
    styles(&lang, &block.code(), theme)
}

/// `code` in the language a fence or a file extension names, in the side's pierre colors.
pub fn highlight_code(lang: &str, code: &str, dark: bool) -> Vec<(Range<usize>, HighlightStyle)> {
    static THEMES: OnceLock<[HighlightTheme; 2]> = OnceLock::new();
    let themes = THEMES.get_or_init(|| [pierre(false).theme(), pierre(true).theme()]);
    styles(&SharedString::from(lang.to_owned()), code, &themes[dark as usize])
}

thread_local! {
    /// One parser per language the registry knows, every other name sharing `text`'s: never more than the registry holds.
    static PARSERS: RefCell<HashMap<SharedString, SyntaxHighlighter>> = RefCell::new(HashMap::new());
}

fn styles(lang: &SharedString, code: &str, theme: &HighlightTheme) -> Vec<(Range<usize>, HighlightStyle)> {
    let lang = LanguageRegistry::singleton().language(lang).map_or_else(|| SharedString::from("text"), |known| known.name);
    let code = code.to_owned();
    PARSERS.with(|parsers| {
        let mut parsers = parsers.borrow_mut();
        let parser = parsers.entry(lang.clone()).or_insert_with(|| SyntaxHighlighter::new(&lang));
        parser.update(None, &Rope::from_str(&code), None);
        parser.styles(&(0..code.len()), theme)
    })
}

/// Shiki's `pierre-light` and `pierre-dark` from `@pierre/theme` 1.1.0, on tree-sitter's capture names.
struct Pierre {
    foreground: u32,
    captures: [(&'static str, u32); 14],
}

fn pierre(dark: bool) -> Pierre {
    if dark {
        Pierre {
            foreground: 0xfafafa,
            captures: [
                ("comment", 0x737373),
                ("string", 0x5ecc71),
                ("number", 0x68cdf2),
                ("boolean", 0x68cdf2),
                ("constant", 0xffd452),
                ("keyword", 0xff678d),
                ("variable", 0xffa359),
                ("property", 0xffa359),
                ("variable.special", 0xffab16),
                ("function", 0x9d6afb),
                ("type", 0xd568ea),
                ("constructor", 0xd568ea),
                ("operator", 0x08c0ef),
                ("punctuation", 0x636363),
            ],
        }
    } else {
        Pierre {
            foreground: 0x0a0a0a,
            captures: [
                ("comment", 0x737373),
                ("string", 0x199f43),
                ("number", 0x1ca1c7),
                ("boolean", 0x1ca1c7),
                ("constant", 0xd5a910),
                ("keyword", 0xd32a61),
                ("variable", 0xd47628),
                ("property", 0xd47628),
                ("variable.special", 0xd5901c),
                ("function", 0x693acf),
                ("type", 0xa631be),
                ("constructor", 0xa631be),
                ("operator", 0x08c0ef),
                ("punctuation", 0x636363),
            ],
        }
    }
}

impl Pierre {
    fn theme(&self) -> HighlightTheme {
        let syntax: serde_json::Map<String, serde_json::Value> =
            self.captures.iter().map(|(name, hex)| (name.to_string(), serde_json::json!({ "color": format!("#{hex:06x}") }))).collect();
        serde_json::from_value(serde_json::json!({
            "name": "pierre",
            "style": { "editor.foreground": format!("#{:06x}", self.foreground), "syntax": syntax },
        }))
        .expect("the pierre theme is a highlight theme")
    }
}

/// `<hr>`: preflight's 1 px rule with no margin of its own, so the blocks around it set its gaps.
struct Rule;

impl MarkdownPlugin for Rule {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "wsp-rule"
    }

    fn parse(&self, node: &mdast::Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        matches!(node, mdast::Node::ThematicBreak(_)).then(|| MarkdownNode::new("wsp-rule", ()).markdown("---"))
    }

    fn render(&self, _: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div().w_full().h(px(1.)).bg(cx.palette().border)
    }
}

/// A person's message as the bubble reads it (`lineBreaks`, no raw HTML): a newline breaks, a tag shows as typed.
pub fn person_text(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    let mut out = String::with_capacity(raw.len() + lines.len() * 2);
    let mut fence: Option<(char, usize)> = None;
    for (ix, line) in lines.iter().enumerate() {
        let opener = fence_marker(line);
        match (fence, opener) {
            (None, Some(marker)) => {
                fence = Some(marker);
                out.push_str(line);
            }
            (Some((c, n)), Some((d, m))) if c == d && m >= n && line.trim_start()[m..].trim().is_empty() => {
                fence = None;
                out.push_str(line);
            }
            (Some(_), _) => out.push_str(line),
            (None, None) => {
                out.push_str(&escape_tags(line));
                let next_is_text = lines.get(ix + 1).is_some_and(|next| !next.trim().is_empty());
                let breaks = line.ends_with("  ") || line.ends_with('\\');
                if next_is_text && !line.trim().is_empty() && !breaks {
                    out.push_str("  ");
                }
            }
        }
        if ix + 1 < lines.len() {
            out.push('\n');
        }
    }
    out
}

/// A fence opener's character and length: three or more backticks or tildes, indented less than four.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let c = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = trimmed.chars().take_while(|&d| d == c).count();
    (n >= 3).then_some((c, n))
}

/// `<` outside code spans escaped, so the parser shows a tag as text; an autolink like `<https://...>` stays a link.
fn escape_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut ix = 0;
    while ix < line.len() {
        let c = bytes[ix];
        if c == b'`' {
            let run = bytes[ix..].iter().take_while(|&&b| b == b'`').count();
            let ticks = &line[ix..ix + run];
            let close = line[ix + run..].find(ticks).map(|at| ix + run + at);
            let end = match close {
                Some(close) => close + run,
                None => ix + run,
            };
            out.push_str(&line[ix..end]);
            ix = end;
            continue;
        }
        if c == b'\\' && ix + 1 < line.len() {
            let next = line[ix + 1..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&line[ix..ix + 1 + next]);
            ix += 1 + next;
            continue;
        }
        if c == b'<' && !is_autolink(&line[ix..]) {
            out.push_str("\\<");
        } else {
            let len = line[ix..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&line[ix..ix + len]);
            ix += len;
            continue;
        }
        ix += 1;
    }
    out
}

/// CommonMark's URI autolink: `<scheme:...>` with a 2 to 32 character scheme and no space or `<` inside.
fn is_autolink(at: &str) -> bool {
    let Some(end) = at.find('>') else { return false };
    let inner = &at[1..end];
    let Some((scheme, rest)) = inner.split_once(':') else { return false };
    (2..=32).contains(&scheme.len())
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        && !rest.contains(|c: char| c.is_whitespace() || c == '<')
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use gpui_kit::base::text::TextViewState;
    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, Bounds, Context, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, Window};

    use super::{
        fence_language, fence_title, opens_with_heading, person_text, run_output_message, runnable_command, view, OnRun, ReplyRuns,
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
        }
    }

    const TWO_BLOCKS: &str = "Look:\n\n```rust\nlet x = 1;\n```\n\nThen:\n\n```sh\nls -la\n```";

    #[gpui_kit::test]
    fn run_shows_on_a_replys_shell_block_and_starts_that_block(cx: &mut TestAppContext) {
        let (on_run, said) = recorder();
        let cx = open(cx, TWO_BLOCKS, 760., Vec::new(), Some((on_run, true)));
        let (run, reply) = (bounds(cx, "code-run"), bounds(cx, "reply"));
        assert!(run.center().y > reply.center().y, "Run sits on the second block, the shell one, in the reply's lower half");
        click(cx, "code-run");
        assert_eq!(*said.lock().unwrap(), vec![RunAction::Start { block: 1, command: "ls -la".into() }]);
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
        let theme = super::pierre(false).theme();
        for n in 0..200 {
            super::styles(&format!("made-up-{n}").into(), "x", &theme);
        }
        super::styles(&"rust".into(), "fn a() {}", &theme);
        super::styles(&"rs".into(), "fn a() {}", &theme);
        let held = super::PARSERS.with(|p| p.borrow().len());
        assert!(held <= 2, "{held} parsers for two languages and 200 unknown names");
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
}
