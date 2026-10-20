// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript's markdown as `ChatMarkdown` draws it: `.chat-markdown` in index.css, `codeBlocks.tsx`, Shiki's pierre.

mod chrome;
mod file_chips;

pub(crate) use chrome::ghost_button;
pub use file_chips::{inline_file, path_text, FileChips, FileMeta};

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use gpui_kit::base::text::{
    data_url_image, markdown_ast as mdast, CodeBlock, InlineCodeFrame, InlineElement, InlineRenderContext, LinkIcon, ListStyleType,
    MarkdownExtensions, MarkdownNode, MarkdownParseContext, MarkdownPlugin, TextView, TextViewState, TextViewStyle,
};
use gpui_kit::component::highlighter::{HighlightTheme, LanguageRegistry, SyntaxHighlighter};
use gpui_kit::component::{h_flex, ActiveTheme, Icon, Rope, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, relative, rems, App, ClickEvent, Entity, FontFeatures, FontWeight, HighlightStyle, Hsla, MouseButton, Overflow,
    Pixels, SharedString, StyleRefinement, Window,
};

use super::math;
use crate::transcript::Run;
use crate::ui::{ActivePalette, Glyph, Palette};
use chrome::{chrome, table_chrome, Reply};
#[cfg(test)]
use chrome::{fence_language, fence_title, run_output_message, runnable_command, CSV_BUILT};
use file_chips::decode;
#[cfg(test)]
use file_chips::{chip_path, parent_suffixes};

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

/// What a press under a reply's shell block asks the thread view for.
#[derive(Clone, Debug, PartialEq)]
pub enum RunAction {
    /// Run: the block's index among the fenced blocks drawn (`Run::block`), the command it holds, and where a refusal
    /// is said under the block.
    Start { block: usize, command: String, said: Entity<Option<SharedString>> },
    /// Send output to the agent: the person's message `runOutputMessage` writes.
    Send { message: String },
    /// Move to terminal, pressed or as the run's program takes the whole screen.
    Move(Run),
    /// Open the tab a moved run went to.
    Open(Run),
    /// Takes back a run the thread records running that this window does not hold.
    Resume(Run),
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
    reply(state, person, runs, None, None, cx)
}

/// `view` for a message of a thread working in `cwd`, which a relative path in it is read against, on `ground`: the
/// solid colour under it, which a wide table's edges fade into; none where it is not known, which draws no fade.
pub fn reply(
    state: &Entity<TextViewState>,
    person: bool,
    runs: Option<ReplyRuns<'_>>,
    cwd: Option<&str>,
    ground: Option<Hsla>,
    cx: &App,
) -> TextView {
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
        .shared_code_block_highlighter(highlighter(&cx.theme().highlight_theme))
        .code_block_chrome(move |block, window, cx| chrome(&reply, block, window, cx))
        .table_chrome(move |table, window, cx| table_chrome(view, ground, table, window, cx))
        // A reply's links come from the workspace: only a web page goes to the system, as the desktop app allows.
        // The kit keeps this handler in the state itself, so it holds the state weakly.
        .on_link_click({
            let state = state.downgrade();
            move |url, event, window, cx| {
                let pressed = match event {
                    ClickEvent::Mouse(click) => matches!(click.up.button, MouseButton::Left | MouseButton::Middle),
                    _ => true,
                };
                if let Some(fragment) = url.strip_prefix('#') {
                    if let Some(state) = state.upgrade() {
                        jump(&state, fragment, cx);
                    }
                } else if pressed && web_page(url) && !open_forwarded(url, window, cx) {
                    cx.open_url(url);
                }
            }
        })
        .markdown_extensions(extensions(cwd, cx))
}

/// `handleMarkdownFragmentClick`: a link to `#<id>` scrolls the reply's own element of that id into view, nearest
/// edge, writing no address. The ids a reply holds are its footnotes', GitHub's: a reference's number to its note
/// (`fn-<id>`), the note's mark back to the number (`fnref-<id>`), each under any number of `user-content-`.
pub fn jump(state: &Entity<TextViewState>, fragment: &str, cx: &mut App) -> bool {
    let text = state.read(cx).rendered_text();
    let Some(range) = fragment_source(text.source(), &decode(fragment)).and_then(|at| text.range_for_source(at)) else { return false };
    state.update(cx, |s, cx| s.reveal_range(range, cx)).is_ok()
}

/// Where in `source` the element of id `fragment` was written: a footnote's definition, or its first reference.
fn fragment_source(source: &str, fragment: &str) -> Option<Range<usize>> {
    let mut id = fragment;
    while let Some(rest) = id.strip_prefix("user-content-") {
        id = rest;
    }
    let (note, id) = match (id.strip_prefix("fn-"), id.strip_prefix("fnref-")) {
        (Some(id), _) => (true, id),
        (None, Some(id)) => (false, id),
        _ => return None,
    };
    let label = regex::escape(id);
    let pattern = if note { format!(r"(?mi)^ {{0,3}}\[\^{label}\]:.*$") } else { format!(r"(?i)\[\^{label}\]") };
    let found = regex::Regex::new(&pattern).ok()?;
    let at = found.find_iter(source).find(|m| note || !source[m.end()..].starts_with(':')).map(|m| m.range());
    at
}

/// A web link's tooltip: its whole address, wrapping anywhere within `min(36rem, 100vw - 2rem)`, `leading-tight`.
fn link_address() -> Arc<gpui_kit::base::text::LinkTooltipFn> {
    static TIP: OnceLock<Arc<gpui_kit::base::text::LinkTooltipFn>> = OnceLock::new();
    TIP.get_or_init(|| {
        Arc::new(|url: &SharedString, window: &mut Window, cx: &mut App| {
            let (url, room) = (url.clone(), window.viewport_size().width - px(32.));
            crate::ui::tooltip_view(
                move |_| {
                    div().debug_selector(|| "link-address".into()).max_w(px(576.).min(room)).line_height(relative(1.25)).child(url.clone())
                },
                window,
                cx,
            )
        })
    })
    .clone()
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
        .text_color(cx.theme().highlight_theme.style.editor_foreground.unwrap_or(foreground));
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
        .with_link_tooltip(link_address())
        .with_details_chevron(Glyph::ChevronRight.path())
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

/// The app's shell the window holds, whose thread a reply's links open against.
fn shell_of(window: &Window, cx: &App) -> Option<Entity<crate::shell::Shell>> {
    window.root::<gpui_kit::base::Root>().flatten().and_then(|r| r.read(cx).view().clone().downcast::<crate::shell::Shell>().ok())
}

/// `LoopbackLinks` with `openInBrowser`: in a thread in a folder on a computer the person joined (`folderOnJoined`, a
/// `place` workspace), a link to a loopback address opens that port and path in a Browser tab of the thread the window
/// shows, which forwards the port; true where it took the link.
fn open_forwarded(url: &str, window: &Window, cx: &mut App) -> bool {
    let Some(address) = crate::screens::browser::url::parse_address(url) else { return false };
    let shell = shell_of(window, cx);
    let Some(screen) = shell.and_then(|s| s.read(cx).thread_screen()) else { return false };
    let view = screen.read(cx).view().read(cx);
    let (Some(workspace), Some(thread)) = (view.followed_workspace().map(str::to_owned), view.thread().map(str::to_owned)) else {
        return false;
    };
    let Some(sidebar) = crate::screens::terminal::workspaces_sidebar(cx) else { return false };
    if sidebar.read(cx).workspace(&workspace).0.and_then(|w| w["kind"].as_str()) != Some("place") {
        return false;
    }
    crate::screens::browser::open_at(&thread, Some(workspace), crate::screens::browser::url::Place::Port(address), cx);
    true
}

/// `onOpenFile`: `path` as its own tab in the Files pane of the thread the window shows, at `line`; nothing where the
/// window shows none, or shows a page over it.
pub fn open_file(path: &str, line: Option<u64>, window: &Window, cx: &mut App) {
    let shell = shell_of(window, cx);
    if let Some(workspace) = shell.and_then(|s| s.read(cx).editor_workspace()) {
        crate::screens::pr_pane::files::open_file(&workspace, path, line.map(|l| l as usize), cx);
    }
}

/// The fence highlighter for a theme, built once: a code block keeps its highlights only while this `Arc` is the same.
type Highlighter = dyn Fn(&CodeBlock) -> Vec<(Range<usize>, HighlightStyle)> + Send + Sync;

fn highlighter(theme: &Arc<HighlightTheme>) -> Arc<Highlighter> {
    static BUILT: std::sync::Mutex<Vec<(Arc<HighlightTheme>, Arc<Highlighter>)>> = std::sync::Mutex::new(Vec::new());
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, held)) = built.iter().find(|(held, _)| Arc::ptr_eq(held, theme) || **held == **theme) {
        return held.clone();
    }
    let colors = theme.clone();
    let made: Arc<Highlighter> = Arc::new(move |block: &CodeBlock| highlight(block, &colors));
    built.push((theme.clone(), made.clone()));
    made
}

fn highlight(block: &CodeBlock, theme: &HighlightTheme) -> Vec<(Range<usize>, HighlightStyle)> {
    let Some(lang) = block.lang() else { return Vec::new() };
    styles(&lang, &block.code(), theme)
}

/// `code` in the language a fence or a file extension names, in `theme`'s colors.
/// Kept per language, theme and code, so a command row or an open diff drawn again parses nothing; cleared past 256.
pub fn highlight_code(lang: &str, code: &str, theme: &HighlightTheme) -> Vec<(Range<usize>, HighlightStyle)> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    theme.hash(&mut hasher);
    let key = (lang.to_owned(), code.len(), hasher.finish());
    if let Some(held) = HIGHLIGHTS.with(|held| held.borrow().get(&key).cloned()) {
        return held.to_vec();
    }
    let found = styles(&SharedString::from(lang.to_owned()), code, theme);
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
    static HIGHLIGHTS: RefCell<HashMap<(String, usize, u64), Highlights>> = RefCell::new(HashMap::new());
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
mod tests;

#[cfg(test)]
mod reply_tests;
