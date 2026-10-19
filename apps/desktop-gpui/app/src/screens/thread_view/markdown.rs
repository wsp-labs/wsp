// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript's markdown as `ChatMarkdown` draws it: `.chat-markdown` in index.css, `codeBlocks.tsx`, Shiki's pierre.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use gpui_kit::base::text::{
    data_url_image, markdown_ast as mdast, CodeBlock, CodeBlockChrome, InlineCodeFrame, InlineElement, InlineRenderContext, LinkIcon,
    ListStyleType, MarkdownExtensions, MarkdownNode, MarkdownParseContext, MarkdownPlugin, RenderedText, TableChrome, TableData, TextView,
    TextViewState, TextViewStyle,
};
use gpui_kit::base::{ScrollbarStyles, StyledExt as _};
use gpui_kit::component::highlighter::{HighlightTheme, LanguageRegistry, SyntaxHighlighter};
use gpui_kit::component::menu::DropdownMenu;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Rope, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, relative, rems, App, ClickEvent, ClipboardItem, Entity, EntityId, FontFeatures, FontWeight, HighlightStyle, Hsla,
    MouseButton, Overflow, Pixels, SharedString, StyleRefinement, Window,
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
    reply(state, person, runs, None, cx)
}

/// `view` for a message of a thread working in `cwd`, which a relative path in it is read against.
pub fn reply(state: &Entity<TextViewState>, person: bool, runs: Option<ReplyRuns<'_>>, cwd: Option<&str>, cx: &App) -> TextView {
    let dark = cx.theme().is_dark();
    let style = if person { person_style(cx) } else { style(cx) };
    // The parsed document, shared, not its source copied: the chrome reads the source only where a block asks.
    let text = state.read(cx).rendered_text();
    let opens_with_heading = opens_with_heading(text.source());
    let reply = Arc::new(match runs {
        Some(ReplyRuns { runs, on_run, can_start }) => {
            Reply { view: state.entity_id(), text, runs: runs.to_vec(), on_run: Some(on_run), can_start }
        }
        None => Reply { view: state.entity_id(), text, runs: Vec::new(), on_run: None, can_start: false },
    });
    let view = state.entity_id();
    TextView::new(state)
        .style(style)
        .font_family(cx.theme().font_family.clone())
        .text_size(text_size(cx))
        .line_height(relative(LEADING))
        .when(opens_with_heading, |view| view.mt(-HEADING_TOP))
        .shared_code_block_highlighter(highlighter(dark))
        .code_block_chrome(move |block, window, cx| chrome(&reply, block, window, cx))
        .table_chrome(move |table, window, cx| table_chrome(view, table, window, cx))
        // A reply's links come from the workspace: only a web page goes to the system, as the desktop app allows.
        .on_link_click(|url, event, _, cx| {
            let pressed = match event {
                ClickEvent::Mouse(click) => matches!(click.up.button, MouseButton::Left | MouseButton::Middle),
                _ => true,
            };
            if pressed && web_page(url) {
                cx.open_url(url);
            }
        })
        .markdown_extensions(extensions(cwd, cx))
}

/// The only links a reply may hand the system: `^https?://`, as the desktop app's window opener allows.
fn web_page(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// What a reply's markdown parses with beyond GFM; a row's document is made with these, so its first frame does not
/// parse it again.
pub fn extensions(cwd: Option<&str>, cx: &App) -> MarkdownExtensions {
    use std::hash::{Hash, Hasher};
    // The file chips read relative paths against the folder, so another folder parses again.
    let mut folder = std::collections::hash_map::DefaultHasher::new();
    cwd.hash(&mut folder);
    MarkdownExtensions::default()
        .parser_revision(folder.finish())
        .plugin(Rule)
        .plugin(math::DisplayMath { body: text_size(cx), gap: GAP_PX })
        .plugin(math::InlineMath)
        .plugin(FileChips::new(cwd))
        .plugin(Unavailable)
}

/// The reply's style: body ink at 80% of the foreground.
pub fn style(cx: &App) -> TextViewStyle {
    cached_style(false, cx)
}

/// A person's bubble, `text-message-foreground`: the full foreground.
pub fn person_style(cx: &App) -> TextViewStyle {
    cached_style(true, cx)
}

/// A style's theme, kind and sizes.
type StyleKey = (&'static str, bool, u32, u32);

thread_local! {
    /// Each style built, by theme, kind and sizes: a frame takes a copy rather than building it again.
    static STYLES: RefCell<HashMap<StyleKey, TextViewStyle>> = RefCell::new(HashMap::new());
    static STYLES_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn cached_style(person: bool, cx: &App) -> TextViewStyle {
    let key = (crate::ui::theme::current(cx).id, person, f32::from(text_size(cx)).to_bits(), f32::from(code_size(cx)).to_bits());
    if let Some(style) = STYLES.with(|styles| styles.borrow().get(&key).cloned()) {
        return style;
    }
    STYLES_BUILT.with(|built| built.set(built.get() + 1));
    let style = styled(cx.palette(), cx.theme().is_dark(), if person { 1. } else { 0.8 }, cx);
    STYLES.with(|styles| styles.borrow_mut().insert(key, style.clone()));
    style
}

/// The sizes the person picked in Settings (`textSize`, `codeSize`), which `sizeVariables` sets as `--font-size-chat`,
/// `--font-size-code` and `--diffs-font-size`; unpicked, each surface keeps its own.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Sizes {
    pub text: Option<Pixels>,
    pub code: Option<Pixels>,
}

impl gpui_kit::Global for Sizes {}

/// Takes the sizes from the host's preferences; the shell calls it as they arrive and change.
pub fn set_sizes(preferences: &serde_json::Value, cx: &mut App) {
    let size = |key: &str| preferences.get(key).and_then(serde_json::Value::as_f64).map(|n| px(n as f32));
    let sizes = Sizes { text: size("textSize"), code: size("codeSize") };
    if cx.try_global::<Sizes>() != Some(&sizes) {
        cx.set_global(sizes);
    }
}

fn sizes(cx: &App) -> Sizes {
    cx.try_global::<Sizes>().copied().unwrap_or_default()
}

/// `--font-size-chat`: a reply's and a bubble's text.
pub fn text_size(cx: &App) -> Pixels {
    sizes(cx).text.unwrap_or(BODY)
}

/// `--font-size-code`: a fence's code, the reply's text size where none is picked.
pub fn code_size(cx: &App) -> Pixels {
    sizes(cx).code.unwrap_or_else(|| text_size(cx))
}

/// `--diffs-font-size`: an edit's diff, 13 px where none is picked.
pub fn diff_size(cx: &App) -> Pixels {
    sizes(cx).code.unwrap_or(px(13.))
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
        .text_size(code_size(cx))
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

    let style = TextViewStyle::default()
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
            margin_left: text_size(cx) * 0.25,
            margin_right: text_size(cx) * 0.2,
            below_baseline: text_size(cx) * 0.125,
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
        .with_table_content_width(true)
        // `section[data-footnotes]`: its margin less the block gap above, its 1 px rule, and inside its padding the
        // 0.65rem a definition's paragraph keeps above and below; `a[data-footnote-ref]` 11 px in a 16 px box, raised
        // as `vertical-align: super` raises it.
        .with_footnotes(gpui_kit::base::text::FootnoteLook {
            section: StyleRefinement::default()
                .mt(px(20.) - GAP_PX)
                .border_t_1()
                .border_color(p.border)
                .pt(px(12.) + GAP_PX)
                .pb(GAP_PX)
                .text_size(px(12.)),
            item_gap: Some(GAP_PX),
            mark_size: Some(px(11.)),
            mark_min_width: px(16.),
            rise: BODY / 3. + px(1.),
        });
    {
        use gpui_kit::base::text::{AlertKind, AlertLook};
        // GITHUB_ALERT_PRESENTATIONS: Tailwind's fixed hues, the 500 at 70% for the rule and the 600 (dark, 400)
        // for the title, the same in every theme.
        let hue = |hex: u32| -> Hsla { gpui_kit::rgb(hex).into() };
        [
            (AlertKind::Note, Glyph::Info, 0x2b7fff, 0x155dfc, 0x51a2ff),
            (AlertKind::Tip, Glyph::Lightbulb, 0x00bc7d, 0x009966, 0x00d492),
            (AlertKind::Important, Glyph::MessageSquareWarning, 0xad46ff, 0x9810fa, 0xc27aff),
            (AlertKind::Warning, Glyph::TriangleAlert, 0xfe9a00, 0xe17100, 0xfe9a00),
            (AlertKind::Caution, Glyph::OctagonAlert, 0xfb2c36, 0xe7000b, 0xff6467),
        ]
        .into_iter()
        .fold(style, |style, (kind, glyph, rule, light, night)| {
            let look = AlertLook {
                rule: hue(rule).opacity(0.7),
                title: hue(if dark { night } else { light }),
                inset: px(12.),
                icon: Some(glyph.path()),
            };
            style.with_alert(kind, look)
        })
    }
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
    text: RenderedText,
    runs: Vec<Run>,
    on_run: Option<OnRun>,
    can_start: bool,
}

/// `MarkdownCodeBlock`: the header row across the block, the code, and under a shell block its run.
fn chrome(reply: &Reply, block: &CodeBlock, window: &mut Window, cx: &mut App) -> CodeBlockChrome {
    let p = cx.palette().clone();
    let start = block.span.map_or(0, |span| span.start);
    let source = reply.text.source();
    let info = fence_info(source, start);
    let language = fence_language(info, block.lang());
    let title = fence_title(info);
    let code = block.code();
    let wrapped = window.use_keyed_state(block_key(CODE_WRAP, reply.view, start), cx, |_, _| WORD_WRAP);
    let copied = window.use_keyed_state(block_key(CODE_COPIED, reply.view, start), cx, |_, _| false);
    let (wrap, done) = (*wrapped.read(cx), *copied.read(cx));
    let index = || block_at(source, source[..start.min(source.len())].encode_utf16().count());
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

/// The kinds of element state a view's blocks keep.
const CODE_WRAP: u32 = 1;
const CODE_COPIED: u32 = 2;
const TABLE_EXPANDED: u32 = 3;
const TABLE_COPIED: u32 = 4;

/// The element state of one of a view's blocks, which starts at byte `start` of its source, named without allocating.
fn block_key(kind: u32, view: EntityId, start: usize) -> gpui_kit::ElementId {
    let mut id = [0u8; 20];
    id[..4].copy_from_slice(&kind.to_le_bytes());
    id[4..12].copy_from_slice(&view.as_u64().to_le_bytes());
    id[12..].copy_from_slice(&(start as u64).to_le_bytes());
    gpui_kit::ElementId::OpaqueId(id)
}

/// InlineRun: an ended run's output and how it ended, or Running; Send output to the agent as a ghost `xs` button.
fn run_view(run: &Run, on_run: OnRun, p: &Palette, cx: &App) -> impl IntoElement {
    let mono = cx.theme().mono_font_family.clone();
    // A `<pre>` draws no line after its text's last newline.
    let output = run
        .output
        .as_deref()
        .filter(|output| run.state == "exited" && !output.is_empty())
        .map(|output| SharedString::from(output.strip_suffix('\n').unwrap_or(output).to_owned()));
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
fn fence_info(source: &str, at: usize) -> &str {
    let at = at.min(source.len());
    let line_start = source[..at].rfind('\n').map_or(0, |ix| ix + 1);
    let line_end = source[at..].find('\n').map_or(source.len(), |ix| at + ix);
    let line = source[line_start..line_end].trim_start_matches([' ', '\t', '>']);
    let fence = line.chars().next().filter(|c| *c == '`' || *c == '~');
    fence.map_or("", |c| line.trim_start_matches(c).trim())
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
    let meta = info.split_once(char::is_whitespace).map(|(_, meta)| meta.trim())?;
    thread_local! {
        /// Each fence meta read, so a block drawn again runs no regex; cleared past 256.
        static READ: RefCell<HashMap<String, Option<SharedString>>> = RefCell::new(HashMap::new());
    }
    if let Some(title) = READ.with(|read| read.borrow().get(meta).cloned()) {
        return title;
    }
    let title = fence_meta_title(meta);
    READ.with(|read| {
        let mut read = read.borrow_mut();
        if read.len() >= 256 {
            read.clear();
        }
        read.insert(meta.to_owned(), title.clone());
    });
    title
}

fn fence_meta_title(meta: &str) -> Option<SharedString> {
    static ATTR: OnceLock<regex::Regex> = OnceLock::new();
    static FILE: OnceLock<regex::Regex> = OnceLock::new();
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

/// The frame's screen for the Files pane, which a file chip and Open diff open a file in.
const FILES: &str = "files";

/// Opens `path` as its own tab in the Files pane (`file:<path>`) where the frame has that pane's screen; nothing where
/// it has none yet.
pub fn open_file(path: &str, cx: &mut App) {
    use crate::screens::{navigate, screen, Entry, Pane, Place, Route};
    if matches!(screen(FILES), Some(Entry { place: Place::Panel(Pane::Files), .. })) {
        navigate(Route::Screen(format!("{FILES}:file:{path}").into()), cx);
    }
}

/// `MarkdownTable`'s footer: the expand toggle on the left, copy on the right; collapsed, cells keep to one line. Copy
/// opens "Copy as Markdown" and "Copy as CSV", then reads "Copied" with a check for 1.2 s.
fn table_chrome(view: EntityId, table: &Arc<TableData>, window: &mut Window, cx: &mut App) -> TableChrome {
    let start = table.span.as_ref().map_or(0, |span| span.start);
    let expanded = window.use_keyed_state(block_key(TABLE_EXPANDED, view, start), cx, |_, _| WORD_WRAP);
    let copied = window.use_keyed_state(block_key(TABLE_COPIED, view, start), cx, |_, _| false);
    let (open, done) = (*expanded.read(cx), *copied.read(cx));
    let copy = |text: fn(&TableData) -> String| -> crate::ui::act::Run {
        let (copied, table) = (copied.clone(), table.clone());
        Rc::new(move |window, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(text(&table)));
            flash(&copied, Duration::from_millis(1200), window, cx);
        })
    };
    let act = |title: &'static str, run| crate::ui::MenuAct {
        group: 0,
        glyph: None,
        title: title.into(),
        destructive: false,
        held: false,
        why: None,
        keys: None,
        run,
    };
    let acts = Rc::new(vec![act("Copy as Markdown", copy(|table| table.markdown.clone())), act("Copy as CSV", copy(table_csv))]);
    let footer = h_flex()
        .justify_between()
        .child(
            ghost_button("expand", if open { Glyph::Minimize2 } else { Glyph::Maximize2 }, open, cx)
                .debug_selector(move || if open { "table-expanded" } else { "table-collapsed" }.into())
                .tooltip(crate::ui::tip(if open { "Collapse table cells" } else { "Expand table cells" }))
                .on_click(move |_, _, cx| {
                    expanded.update(cx, |expanded, cx| {
                        *expanded = !*expanded;
                        cx.notify();
                    })
                }),
        )
        .child(
            crate::ui::menu::Trigger(
                ghost_button("copy", if done { Glyph::Check } else { Glyph::Copy }, false, cx)
                    .debug_selector(move || if done { "table-copied" } else { "table-copy" }.into())
                    .tooltip(crate::ui::tip(if done { "Copied" } else { "Copy table" })),
            )
            .dropdown_menu_with_anchor(gpui_kit::Anchor::TopRight, move |menu, _, cx| {
                crate::ui::menu_of(menu, &acts, cx.palette().error_foreground, cx.palette().muted_foreground)
            }),
        );
    // The web's `mt-0.5`.
    let chrome = TableChrome::new().footer_gap(px(2.)).footer(footer);
    if open {
        chrome
    } else {
        chrome.cell(StyleRefinement::default().whitespace_nowrap())
    }
}

/// Sets `flag` and clears it after `hold`, as a copy's check does.
fn flash(flag: &Entity<bool>, hold: Duration, window: &mut Window, cx: &mut App) {
    flag.update(cx, |flag, cx| {
        *flag = true;
        cx.notify();
    });
    let flag = flag.clone();
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(hold).await;
            flag.update(cx, |flag, cx| {
                *flag = false;
                cx.notify();
            });
        })
        .detach();
}

/// `serializeTableElementToCsv`: each row's cells, runs of space as one, quoted where a quote, comma or newline is in one.
fn table_csv(table: &TableData) -> String {
    #[cfg(test)]
    CSV_BUILT.with(|built| built.set(built.get() + 1));
    let cell = |value: &String| {
        let normal = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if normal.contains(['"', ',', '\n']) {
            format!("\"{}\"", normal.replace('"', "\"\""))
        } else {
            normal
        }
    };
    std::iter::once(&table.headers)
        .chain(table.rows.iter())
        .filter(|row| !row.is_empty())
        .map(|row| row.iter().map(cell).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join("\n")
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
/// Kept per language, side and code, so a command row or an open diff drawn again parses nothing; cleared past 256.
pub fn highlight_code(lang: &str, code: &str, dark: bool) -> Vec<(Range<usize>, HighlightStyle)> {
    use std::hash::{Hash, Hasher};
    static THEMES: OnceLock<[HighlightTheme; 2]> = OnceLock::new();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    let key = (lang.to_owned(), dark, code.len(), hasher.finish());
    if let Some(held) = HIGHLIGHTS.with(|held| held.borrow().get(&key).cloned()) {
        return held.to_vec();
    }
    let themes = THEMES.get_or_init(|| [pierre(false).theme(), pierre(true).theme()]);
    let found = styles(&SharedString::from(lang.to_owned()), code, &themes[dark as usize]);
    HIGHLIGHTS.with(|held| {
        let mut held = held.borrow_mut();
        if held.len() >= HIGHLIGHTS_HELD {
            held.clear();
        }
        held.insert(key, Arc::from(found.as_slice()));
    });
    found
}

const HIGHLIGHTS_HELD: usize = 256;

type Highlights = Arc<[(Range<usize>, HighlightStyle)]>;

thread_local! {
    /// One parser per language the registry knows, every other name sharing `text`'s: never more than the registry holds.
    static PARSERS: RefCell<HashMap<SharedString, SyntaxHighlighter>> = RefCell::new(HashMap::new());
    static HIGHLIGHTS: RefCell<HashMap<(String, bool, usize, u64), Highlights>> = RefCell::new(HashMap::new());
    /// How many times a parser ran, for the tests.
    static PARSED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn styles(lang: &SharedString, code: &str, theme: &HighlightTheme) -> Vec<(Range<usize>, HighlightStyle)> {
    let lang = LanguageRegistry::singleton().language(lang).map_or_else(|| SharedString::from("text"), |known| known.name);
    let code = code.to_owned();
    PARSED.with(|parsed| parsed.set(parsed.get() + 1));
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

/// A file a message names in inline code (`MarkdownFileLinkMeta` in lib/markdownLinks.ts): the path, the target with
/// its position, the path from the thread's folder when it is inside it, and the line and column it names.
#[derive(Clone, Debug, PartialEq)]
pub struct FileMeta {
    pub file_path: String,
    pub target: String,
    pub relative: Option<String>,
    pub basename: String,
    pub line: Option<u64>,
    pub column: Option<u64>,
}

impl FileMeta {
    /// What the Files pane opens: the path from the folder inside it, the host's own path outside it.
    pub fn panel_path(&self) -> Option<&str> {
        self.relative.as_deref().or_else(|| is_absolute(&self.file_path).then_some(self.file_path.as_str()))
    }
}

fn regex(cell: &'static OnceLock<regex::Regex>, pattern: &str) -> &'static regex::Regex {
    cell.get_or_init(|| regex::Regex::new(pattern).expect("a regex"))
}

fn drive(path: &str) -> bool {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    regex(&R, r"^[A-Za-z]:[\\/]").is_match(path)
}

fn unc(path: &str) -> bool {
    path.starts_with("\\\\")
}

fn dotted(path: &str) -> bool {
    path.starts_with("~/") || path.starts_with("./") || path.starts_with("../")
}

fn position_suffix(path: &str) -> bool {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    regex(&R, r":\d+(?::\d+)?$").is_match(path)
}

fn strip_position(path: &str) -> &str {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    regex(&R, r":\d+(?::\d+)?$").find(path).map_or(path, |m| &path[..m.start()])
}

/// `isAbsolutePath` in terminal-links.ts.
fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || drive(path) || unc(path)
}

/// `looksLikeHostname` in lib/markdownLinkParsing.ts.
fn looks_like_hostname(segment: &str, has_position: bool) -> bool {
    const GENERIC: [&str; 25] = [
        "com", "net", "org", "io", "dev", "app", "ai", "co", "edu", "gov", "mil", "info", "biz", "xyz", "me", "tv", "cc", "gg", "chat",
        "cloud", "site", "online", "tech", "store", "link",
    ];
    const COUNTRY: [&str; 31] = [
        "uk", "de", "fr", "nl", "se", "no", "fi", "dk", "pl", "ch", "at", "be", "es", "it", "pt", "eu", "us", "ca", "au", "nz", "jp", "kr",
        "cn", "br", "ru", "mx", "ie", "cz", "tr", "sg", "hk",
    ];
    static NUMERIC: OnceLock<regex::Regex> = OnceLock::new();
    if segment.starts_with('.') {
        return false;
    }
    let lowered = segment.to_lowercase();
    if lowered == "localhost" || regex(&NUMERIC, r"^\d+(?:\.\d+)+$").is_match(segment) {
        return true;
    }
    let labels: Vec<&str> = lowered.split('.').collect();
    let Some(last) = labels.last().filter(|_| labels.len() >= 2) else { return false };
    GENERIC.contains(last) || (!has_position && COUNTRY.contains(last))
}

/// `isConventionalFilePosition`: `Makefile:12`, an extensionless name the :line marks as a file.
fn conventional_position(path: &str) -> bool {
    const NAMES: [&str; 27] = [
        "Makefile",
        "makefile",
        "GNUmakefile",
        "Dockerfile",
        "Containerfile",
        "Justfile",
        "justfile",
        "Rakefile",
        "Gemfile",
        "Procfile",
        "Brewfile",
        "Caddyfile",
        "Vagrantfile",
        "Jenkinsfile",
        "Podfile",
        "Fastfile",
        "BUILD",
        "WORKSPACE",
        "LICENSE",
        "LICENCE",
        "COPYING",
        "NOTICE",
        "AUTHORS",
        "CONTRIBUTORS",
        "CHANGELOG",
        "README",
        "CODEOWNERS",
    ];
    static R: OnceLock<regex::Regex> = OnceLock::new();
    regex(&R, r"^[A-Za-z0-9_-]+(?::\d+){1,2}$").is_match(path) && NAMES.contains(&strip_position(path))
}

/// `inlineCodeFilePathCandidate`: inline code shaped like a path, which takes a path prefix, an extension or a :line.
fn inline_candidate(code: &str) -> Option<String> {
    static EXTENSION: OnceLock<regex::Regex> = OnceLock::new();
    let trimmed = code.trim();
    if trimmed.is_empty() || trimmed.contains(|c: char| c.is_whitespace() || c == '`') {
        return None;
    }
    let candidate = if drive(trimmed) || unc(trimmed) { trimmed.to_owned() } else { trimmed.replace('\\', "/") };
    let has_position = position_suffix(&candidate);
    if !has_position && !candidate.contains(['/', '\\']) {
        return None;
    }
    let explicit = dotted(&candidate) || candidate.starts_with('/') || drive(&candidate) || unc(&candidate);
    if !explicit {
        let bare = strip_position(&candidate);
        if looks_like_hostname(bare.split('/').next().unwrap_or(bare), has_position) {
            return None;
        }
        let basename = bare.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("");
        if !has_position && !regex(&EXTENSION, r"\.[A-Za-z0-9_-]+$").is_match(basename) {
            return None;
        }
    }
    Some(candidate)
}

/// `decodeURIComponent`, keeping the text as it stands where it does not decode.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut ix = 0;
    while ix < bytes.len() {
        if bytes[ix] == b'%' {
            match text.get(ix + 1..ix + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                Some(byte) => {
                    out.push(byte);
                    ix += 3;
                    continue;
                }
                None => return text.to_owned(),
            }
        }
        out.push(bytes[ix]);
        ix += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

/// `resolveMarkdownFileLinkTarget` for an inline code candidate: a path the folder reads relative ones against.
fn link_target(href: &str, cwd: Option<&str>) -> Option<String> {
    static SCHEME: OnceLock<regex::Regex> = OnceLock::new();
    static RELATIVE_PATH: OnceLock<regex::Regex> = OnceLock::new();
    static RELATIVE_NAME: OnceLock<regex::Regex> = OnceLock::new();
    static HASH_LINE: OnceLock<regex::Regex> = OnceLock::new();
    static POSITION_ONLY: OnceLock<regex::Regex> = OnceLock::new();
    let raw = href.trim();
    if raw.is_empty() || raw.starts_with('#') || raw.starts_with("//") {
        return None;
    }
    let (with_search, hash) = raw.split_at(raw.find('#').unwrap_or(raw.len()));
    let path = &with_search[..with_search.find('?').unwrap_or(with_search.len())];
    let path = decode(path.trim());
    let path = if path.len() > 3 && path.starts_with('/') && drive(&path[1..]) { path[1..].to_owned() } else { path };
    let hash = decode(hash.trim());
    if path.is_empty() {
        return None;
    }
    if !drive(&path) && !unc(&path) {
        if let Some(found) = regex(&SCHEME, r"^([A-Za-z][A-Za-z0-9+.-]*):(.*)$").captures(&path) {
            let rest = found.get(2).map_or("", |m| m.as_str());
            if rest.starts_with("//") || !regex(&POSITION_ONLY, r"^\d+(?::\d+)?$").is_match(rest) {
                return None;
            }
        }
    }
    let likely = if drive(&path) || unc(&path) || dotted(&path) {
        true
    } else if path.starts_with('/') {
        const ROOTS: [&str; 24] = [
            "/Users/",
            "/home/",
            "/tmp/",
            "/var/",
            "/etc/",
            "/opt/",
            "/mnt/",
            "/Volumes/",
            "/private/",
            "/root/",
            "/usr/",
            "/bin/",
            "/sbin/",
            "/lib/",
            "/lib64/",
            "/srv/",
            "/dev/",
            "/proc/",
            "/sys/",
            "/run/",
            "/boot/",
            "/media/",
            "/workspace/",
            "/workspaces/",
        ];
        let basename = &path[path.rfind('/').map_or(0, |at| at + 1)..];
        ROOTS.iter().any(|root| path.starts_with(root))
            || position_suffix(&path)
            || basename.rfind('.').is_some_and(|at| {
                at + 1 < basename.len() && basename[at + 1..].chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            })
    } else {
        regex(&RELATIVE_PATH, r"^(?:[A-Za-z0-9._-]+(?: +[A-Za-z0-9._-]+)*/)+[A-Za-z0-9._-]+(?: +[A-Za-z0-9._-]+)*(?::\d+){0,2}$")
            .is_match(&path)
            || regex(&RELATIVE_NAME, r"^[A-Za-z0-9._-]+(?: +[A-Za-z0-9._-]+)*\.[A-Za-z0-9_-]+(?::\d+){0,2}$").is_match(&path)
    };
    if !likely {
        return None;
    }
    let path = match regex(&HASH_LINE, r"(?i)^#L(\d+)(?:C(\d+))?$").captures(&hash).filter(|_| !position_suffix(&path)) {
        Some(found) => format!("{path}:{}{}", &found[1], found.get(2).map_or(String::new(), |c| format!(":{}", c.as_str()))),
        None => path,
    };
    if !dotted(&path) && is_absolute(&path) {
        return Some(path);
    }
    cwd.map(|cwd| in_folder(&path, cwd))
}

/// `splitPathAndPosition`: a trailing `:line` or `:line:column`.
fn split_position(value: &str) -> (&str, Option<&str>, Option<&str>) {
    fn tail(text: &str) -> Option<(&str, &str)> {
        let (head, digits) = text.rsplit_once(':')?;
        (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some((head, digits))
    }
    let Some((head, last)) = tail(value) else { return (value, None, None) };
    match tail(head) {
        Some((path, line)) => (path, Some(line), Some(last)),
        None => (head, Some(last), None),
    }
}

/// `resolvePathLinkTarget`: `~/` from the home the folder sits in, a relative path joined to the folder.
fn in_folder(raw: &str, cwd: &str) -> String {
    static HOME: OnceLock<regex::Regex> = OnceLock::new();
    let (path, line, column) = split_position(raw);
    let windows = drive(cwd) || cwd.contains(":\\");
    let join = |base: &str, next: &str| {
        let base = base.trim_end_matches(['/', '\\']);
        if windows {
            format!("{base}\\{}", next.replace('/', "\\"))
        } else {
            format!("{base}/{}", next.trim_start_matches('/'))
        }
    };
    let resolved = if let Some(rest) = path.strip_prefix("~/") {
        let home = regex(&HOME, r"^(/Users/[^/]+|/home/[^/]+|[A-Za-z]:\\Users\\[^\\]+)").find(cwd).map(|m| m.as_str());
        home.map_or_else(|| path.to_owned(), |home| join(home, rest))
    } else if !is_absolute(path) {
        join(cwd, path)
    } else {
        path.to_owned()
    };
    match (line, column) {
        (Some(line), Some(column)) => format!("{resolved}:{line}:{column}"),
        (Some(line), None) => format!("{resolved}:{line}"),
        _ => resolved,
    }
}

/// `buildFileLinkMetaFromTarget`.
fn file_meta(target: String, cwd: Option<&str>) -> FileMeta {
    let (path, line, column) = split_position(&target);
    let path = path.to_owned();
    let relative = cwd.and_then(|root| {
        let normal = |p: &str| {
            let p = p.replace('\\', "/");
            if p.len() > 3 && p.starts_with('/') && drive(&p[1..]) {
                p[1..].to_owned()
            } else {
                p
            }
        };
        let (path, root) = (normal(&path), normal(root));
        let root = root.trim_end_matches('/');
        // The web slices at the root's length in UTF-16 units; a case-folded match may differ from it in bytes.
        let skip = root.chars().count() + 1;
        path.to_lowercase().starts_with(&format!("{}/", root.to_lowercase())).then(|| path.chars().skip(skip).collect())
    });
    let trimmed = path.trim_end_matches(['/', '\\']);
    let trimmed = if trimmed.is_empty() { path.as_str() } else { trimmed };
    let basename = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed).to_owned();
    let (line, column) = (line.and_then(|l| l.parse().ok()), column.and_then(|c| c.parse().ok()));
    FileMeta { file_path: path, target: target.clone(), relative, basename, line, column }
}

/// `resolveInlineCodeFileLinkMeta`: the file inline code names, read against the thread's folder.
pub fn inline_file(code: &str, cwd: Option<&str>) -> Option<FileMeta> {
    let candidate = inline_candidate(code)?;
    if let Some(target) = link_target(&candidate, cwd) {
        return Some(file_meta(target, cwd));
    }
    let cwd = cwd.filter(|_| conventional_position(&candidate))?;
    Some(file_meta(in_folder(&candidate, cwd), Some(cwd)))
}

/// `extractInlineCodeSpans`: each inline code span outside fenced code, trimmed.
fn inline_spans(text: &str) -> Vec<&str> {
    static SPAN: OnceLock<regex::Regex> = OnceLock::new();
    let span = regex(&SPAN, r"`([^`\n]+)`");
    let mut spans = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in text.lines() {
        match (fence, fence_marker(line)) {
            (None, Some(marker)) => fence = Some(marker),
            (Some((c, n)), Some((d, m))) if c == d && m >= n => fence = None,
            (Some(_), _) => {}
            (None, None) => {
                spans.extend(span.captures_iter(line).filter_map(|c| c.get(1)).map(|m| m.as_str().trim()).filter(|s| !s.is_empty()))
            }
        }
    }
    spans
}

/// `buildFileLinkParentSuffixByPath`: for files of one name, the parent folders that tell them apart, at least two.
fn parent_suffixes(paths: &[String]) -> HashMap<String, String> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for path in paths {
        let normal = path.replace('\\', "/");
        let Some(base) = normal.split('/').rfind(|s| !s.is_empty()).map(str::to_owned) else { continue };
        let group = groups.entry(base).or_default();
        if !group.contains(&normal) {
            group.push(normal);
        }
    }
    let mut out = HashMap::new();
    for group in groups.values().filter(|g| g.len() >= 2) {
        let parents: Vec<Vec<&str>> = group
            .iter()
            .map(|p| {
                let segments: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
                segments[..segments.len().saturating_sub(1)].to_vec()
            })
            .collect();
        for (ix, path) in group.iter().enumerate() {
            let segments = &parents[ix];
            if segments.is_empty() {
                continue;
            }
            let tail = |s: &[&str], depth: usize| s[s.len().saturating_sub(depth)..].join("/");
            let depth = (1..=segments.len())
                .find(|&depth| {
                    let mine = tail(segments, depth);
                    !parents.iter().enumerate().any(|(other, theirs)| other != ix && tail(theirs, depth) == mine)
                })
                .unwrap_or(segments.len());
            out.insert(path.clone(), tail(segments, segments.len().min(depth.max(2))));
        }
    }
    out
}

/// Inline code naming a file, drawn as today's file chip (`MarkdownFileLink`): the file glyph, the name, the folder that
/// tells two files of one name apart and the line, the whole path on its hover; a press opens it in Files.
/// A message's source and its chips' parent folders.
type Suffixes = (String, Arc<HashMap<String, String>>);

pub struct FileChips {
    cwd: Option<String>,
    /// The message's chips' parent folders, by the source they were read off, so each node does not read it again.
    suffixes: std::sync::Mutex<Option<Suffixes>>,
}

impl FileChips {
    pub fn new(cwd: Option<&str>) -> Self {
        FileChips { cwd: cwd.map(str::to_owned), suffixes: std::sync::Mutex::new(None) }
    }

    fn suffixes(&self, source: &str) -> Arc<HashMap<String, String>> {
        let mut held = self.suffixes.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, suffixes)) = held.as_ref().filter(|(read, _)| read == source) {
            return suffixes.clone();
        }
        let cwd = self.cwd.as_deref();
        let paths: Vec<String> = inline_spans(source).into_iter().filter_map(|span| inline_file(span, cwd)).map(|m| m.file_path).collect();
        let suffixes = Arc::new(parent_suffixes(&paths));
        *held = Some((source.to_owned(), suffixes.clone()));
        suffixes
    }
}

/// A chip as it parsed: the file and its label's parts.
struct Chip {
    meta: FileMeta,
    label: SharedString,
}

/// `fileChipLabel`: the name, the folder that tells it apart, `L12:C3`.
fn chip_label(meta: &FileMeta, suffixes: &HashMap<String, String>) -> String {
    let mut parts = vec![meta.basename.clone()];
    if let Some(suffix) = suffixes.get(&meta.file_path.replace('\\', "/")).filter(|s| !s.is_empty()) {
        parts.push(suffix.clone());
    }
    if let Some(line) = meta.line.filter(|l| *l > 0) {
        parts.push(match meta.column.filter(|c| *c > 0) {
            Some(column) => format!("L{line}:C{column}"),
            None => format!("L{line}"),
        });
    }
    parts.join(" ")
}

const CHIP: &str = "wsp-file-chip";

impl MarkdownPlugin for FileChips {
    fn name(&self) -> &str {
        CHIP
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::InlineCode(code) = node else { return None };
        let meta = inline_file(&code.value, self.cwd.as_deref())?;
        let label: SharedString = chip_label(&meta, &self.suffixes(cx.source())).into();
        let markdown = cx.node_source(node).map_or_else(|| format!("`{}`", code.value), str::to_owned);
        Some(MarkdownNode::new(CHIP, Chip { meta, label: label.clone() }).text(label).markdown(markdown))
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let chip = node.data::<Chip>()?;
        let p = cx.palette().clone();
        let font = window.text_system().resolve_font(&context.text_style().font());
        let x_height = window.text_system().x_height(font, context.font_size());
        let panel = chip.meta.panel_path().map(str::to_owned);
        let target = SharedString::from(chip.meta.target.clone());
        let element = h_flex()
            .id(SharedString::from(format!("file-chip:{}", chip.meta.target)))
            .debug_selector(|| "file-chip".into())
            .flex_none()
            .h(CHIP_HEIGHT)
            .gap(px(12. * 0.33))
            .px(px(6.))
            .rounded(px(6.))
            .border_1()
            .border_color(p.border.opacity(0.7))
            .bg(p.accent.opacity(0.4))
            .text_size(px(12.))
            .line_height(px(15.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.foreground)
            .child(Icon::new(Glyph::File).with_size(px(12. * 1.17)).flex_none().opacity(0.85))
            .child(div().min_w_0().truncate().child(chip.label.clone()))
            .tooltip(path_tip(target))
            .when_some(panel, |el, path| {
                el.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.7))).on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    open_file(&path, cx);
                })
            });
        // `vertical-align: middle`: the chip's middle half an x-height over the line's baseline.
        Some(InlineElement::new(element).with_baseline(CHIP_HEIGHT / 2. + x_height / 2.))
    }
}

/// `h-[1.41em]` at the chip's 12 px.
const CHIP_HEIGHT: Pixels = px(12. * 1.41);

/// A chip's tooltip: the whole path.
fn path_tip(path: SharedString) -> impl Fn(&mut Window, &mut App) -> gpui_kit::AnyView + 'static {
    move |window, cx| {
        let (path, mono) = (path.clone(), cx.theme().mono_font_family.clone());
        crate::ui::tooltip_view(move |_| path_text(path.clone(), mono.clone()), window, cx)
    }
}

/// The path in a chip's tooltip, `font-mono text-[11px] leading-tight`.
pub fn path_text(path: SharedString, mono: SharedString) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id("file-chip-tip")
        .debug_selector(|| "file-chip-tip".into())
        .font_family(mono)
        .text_size(px(11.))
        .line_height(relative(1.25))
        .whitespace_nowrap()
        .child(path)
}

/// An image the app cannot draw, which is every one but a `data:` image GPUI decodes, since the app fetches nothing:
/// today's `ChatMarkdownImageFallback`, so the alt text is never lost.
struct Unavailable;

/// The fallback's words, "Image unavailable: alt".
struct Fallback(SharedString);

const UNAVAILABLE: &str = "wsp-image-unavailable";

impl MarkdownPlugin for Unavailable {
    fn name(&self) -> &str {
        UNAVAILABLE
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::Image(image) = node else { return None };
        if data_url_image(&image.url).is_some() {
            return None;
        }
        let words: SharedString =
            if image.alt.is_empty() { "Image unavailable".into() } else { format!("Image unavailable: {}", image.alt).into() };
        let markdown = cx.node_source(node).map_or_else(|| format!("![{}]({})", image.alt, image.url), str::to_owned);
        Some(MarkdownNode::new(UNAVAILABLE, Fallback(words.clone())).text(words).markdown(markdown))
    }

    fn render_inline(&self, node: &MarkdownNode, _: &InlineRenderContext, window: &mut Window, cx: &mut App) -> Option<InlineElement> {
        let Fallback(words) = node.data::<Fallback>()?;
        let p = cx.palette();
        let family = cx.theme().font_family.clone();
        let font = window.text_system().resolve_font(&gpui_kit::font(family.clone()));
        let ascent = window.text_system().ascent(font, px(12.)).round();
        let descent = window.text_system().descent(font, px(12.)).abs().round();
        // The `inline-flex` row stands on its glyph's bottom edge, 15 px into its 16 px line, and the `inline-block`'s
        // own line, Chromium's rounded metrics half-led to 16 px, reaches below that edge.
        let below = ((px(16.) + descent - ascent) / 2.).max(px(1.));
        // `inline-block rounded-md border border-border/40 bg-muted/40 px-2 py-1 text-xs`, a 14 px glyph 6 px before
        // the words.
        let element = div()
            .debug_selector(|| "image-unavailable".into())
            .flex_none()
            .pt(px(4.))
            .pb(px(4.) + below - px(1.))
            .px(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.border.opacity(0.4))
            .bg(p.muted.opacity(0.4))
            .font_family(family)
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(FontWeight::NORMAL)
            .text_color(p.muted_foreground)
            .child(
                h_flex()
                    .h(px(16.))
                    .gap(px(6.))
                    .child(Icon::new(Glyph::TriangleAlert).with_size(px(14.)).flex_none())
                    .child(div().whitespace_nowrap().child(words.clone())),
            );
        Some(InlineElement::new(element).with_baseline(px(1. + 4. + 15.)))
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
thread_local! {
    /// How many tables' CSV this test thread built.
    static CSV_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use gpui_kit::base::text::TextViewState;
    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, Bounds, Context, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, Window};

    use super::{
        fence_language, fence_title, opens_with_heading, person_text, run_output_message, runnable_command, view, web_page, OnRun,
        ReplyRuns, RunAction,
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

    #[test]
    fn only_a_web_page_goes_to_the_system() {
        assert!(web_page("https://wsp.dev/a") && web_page("http://localhost:3000/"));
        for url in
            ["file:///Users/me/ws/run.command", "smb://box/share", "vscode://file/x", "javascript:alert(1)", "ssh://h", "/etc/passwd"]
        {
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
            div().size_full().child(div().debug_selector(|| "reply".into()).child(super::reply(&self.0, false, None, self.1, cx)))
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
    const DOT: &str =
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

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
        let first = super::highlight_code("bash", &code, false);
        let second = super::highlight_code("bash", &code, false);
        assert_eq!(first, second);
        assert_eq!(parsed() - before, 1, "the second call parsed again");
    }
}
