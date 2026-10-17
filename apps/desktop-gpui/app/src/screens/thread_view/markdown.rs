// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript's markdown, drawn as the web app's `ChatMarkdown` draws it: the `.chat-markdown` rules in
//! `apps/web/src/index.css`, the code block of `components/markdown/codeBlocks.tsx` and Shiki's pierre themes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use gpui_kit::base::text::{
    markdown_ast as mdast, CodeBlock, InlineElement, InlineRenderContext, MarkdownNode, MarkdownParseContext, MarkdownPlugin, TableData,
    TextView, TextViewState, TextViewStyle,
};
use gpui_kit::component::highlighter::{HighlightTheme, SyntaxHighlighter};
use gpui_kit::component::{h_flex, ActiveTheme, Icon, Rope, Sizable as _};
use gpui_kit::{
    canvas, div, point, prelude::*, px, relative, rems, App, ClipboardItem, Entity, FontWeight, HighlightStyle, Overflow, Pixels,
    SharedString, StyleRefinement, TextAlign, TextRun, Window,
};

use crate::ui::theme::first_installed;
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
/// The code block's header row: `pt-1.5` and a 24 px button.
const HEADER: Pixels = px(30.);
/// KaTeX's `.katex { font-size: 1.21em }`.
const MATH_SCALE: f32 = 1.21;
const SERIF: [&str; 3] = ["Times New Roman", "Liberation Serif", "DejaVu Serif"];

/// The configured view of a row's parsed document: a person's bubble when `person`, else an agent's reply.
pub fn view(state: &Entity<TextViewState>, person: bool, cx: &App) -> TextView {
    let dark = cx.theme().is_dark();
    let style = if person { person_style(cx) } else { style(cx) };
    let opens_with_heading = opens_with_heading(state.read(cx).rendered_text().source());
    TextView::new(state)
        .style(style)
        .font_family(cx.theme().font_family.clone())
        .text_size(BODY)
        .line_height(relative(LEADING))
        .when(opens_with_heading, |view| view.mt(-HEADING_TOP))
        .shared_code_block_highlighter(highlighter(dark))
        .code_block_actions(header)
        .table_actions(table_footer)
        .plugin(Rule)
        .plugin(InlineCode)
        .plugin(Link)
        .plugin(DisplayMath)
        .plugin(InlineMath)
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
        .pt(HEADER + px(12.8))
        .pb(px(12.8))
        .font_family(mono)
        .text_size(BODY)
        .line_height(relative(1.375))
        .text_color(gpui_kit::rgb(pierre(dark).foreground));
    // The header's label finds the block's left edge through the clip this sets.
    code_block.overflow.x = Some(Overflow::Hidden);

    let mut table =
        StyleRefinement::default().border_0().border_b_1().border_color(rule).bg(gpui_kit::transparent_black()).text_size(px(12.));
    table.overflow.x = Some(Overflow::Scroll);
    // The refinement styles the header row around its cells, so it pads the cells' 7.2 px up to the web's 8.8.
    let head = StyleRefinement::default().py(px(8.8 - 7.2)).font_weight(FontWeight::SEMIBOLD).bg(gpui_kit::transparent_black());
    let cell = StyleRefinement::default().px(px(12.)).py(px(7.2)).text_size(px(12.)).border_color(gpui_kit::transparent_black());

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
        .with_dark(dark)
}

/// Whether the first block is an ATX heading, whose top margin the web drops as `:first-child`.
fn opens_with_heading(source: &str) -> bool {
    let line = source.trim_start_matches(['\n', '\r']).lines().next().unwrap_or("");
    let indent = line.len() - line.trim_start_matches(' ').len();
    let hashes = line.trim_start_matches(' ').bytes().take_while(|&b| b == b'#').count();
    indent < 4 && (1..=6).contains(&hashes) && line.trim_start_matches(' ')[hashes..].chars().next().is_none_or(|c| c == ' ' || c == '\t')
}

/// The fence's header: the language on the left, the copy button on the right.
fn header(block: &CodeBlock, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.palette().clone();
    let label = block.lang().filter(|lang| !lang.is_empty()).unwrap_or_else(|| "text".into());
    let code = block.code();
    let copied = window.use_keyed_state(("wsp-code-copied", hash(&code)), cx, |_, _| false);
    let done = *copied.read(cx);
    let ink = p.foreground.opacity(0.72);
    let mono = cx.theme().mono_font_family.clone();
    h_flex()
        .items_center()
        .gap(px(2.))
        // gpui-kit pins the actions 8 px in from the corner; the web's header pads 6.
        .mt(px(-2.))
        .mr(px(-2.))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let size = px(11.);
                    let font = gpui_kit::font(mono.clone());
                    let run = TextRun { len: label.len(), font, color: ink, background_color: None, underline: None, strikethrough: None };
                    let line = window.text_system().shape_line(label.clone(), size, &[run], None);
                    let line_height = size * 1.375;
                    let left = window.content_mask().bounds.origin.x + px(1. + 12.);
                    let top = bounds.origin.y + (bounds.size.height - line_height) / 2.;
                    line.paint(point(left, top), line_height, TextAlign::Left, None, window, cx).ok();
                },
            )
            .w(px(0.))
            .h(px(24.)),
        )
        .child(
            ghost_button("copy", if done { Glyph::Check } else { Glyph::Copy }, cx)
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
        )
}

fn hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// The table's footer: the web's copy button on the right, copying the table as Markdown.
fn table_footer(table: &TableData, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let markdown = table.markdown.clone();
    // gpui-kit sets the footer 4 px under the table; the web's `mt-0.5` is 2.
    h_flex().mt(px(-2.)).justify_end().child(ghost_button("copy", Glyph::Copy, cx).on_click(move |_, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(markdown.clone()));
    }))
}

/// `Button variant="ghost" size="icon-xs"`: 24 px, a 12 px glyph in the muted ink, the accent fill under the pointer.
fn ghost_button(id: &'static str, glyph: Glyph, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
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
        .hover(|s| s.bg(p.accent))
        .child(Icon::new(glyph).with_size(px(12.)).text_color(p.muted_foreground))
}

/// The fence highlighter for a side, built once: a code block keeps its highlights only while the highlighter is the
/// same `Arc`.
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

fn styles(lang: &SharedString, code: &str, theme: &HighlightTheme) -> Vec<(Range<usize>, HighlightStyle)> {
    thread_local! {
        static PARSERS: RefCell<HashMap<SharedString, SyntaxHighlighter>> = RefCell::new(HashMap::new());
    }
    let lang = lang.clone();
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

/// A code span, `.chat-markdown :not(pre) > code`: 12 px mono in a 1 px bordered, 6 px rounded box. The box is one
/// piece on its line, so a span too long to sit on one line keeps gpui-kit's own wrapping code run instead.
struct InlineCode;

const CHIP_MAX: usize = 64;

impl MarkdownPlugin for InlineCode {
    fn name(&self) -> &str {
        "wsp-inline-code"
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::InlineCode(code) = node else { return None };
        if code.value.chars().count() > CHIP_MAX {
            return None;
        }
        let source = cx.node_source(node).map_or_else(|| format!("`{}`", code.value), str::to_owned);
        Some(MarkdownNode::new("wsp-inline-code", ()).text(code.value.clone()).markdown(source))
    }

    fn render_inline(&self, node: &MarkdownNode, _: &InlineRenderContext, window: &mut Window, cx: &mut App) -> Option<InlineElement> {
        let p = cx.palette();
        let mono = cx.theme().mono_font_family.clone();
        let size = px(12.);
        let font = window.text_system().resolve_font(&gpui_kit::font(mono.clone()));
        let ascent = window.text_system().ascent(font, size);
        let descent = window.text_system().descent(font, size).abs();
        let (edge, pad_y) = (px(1.), px(1.6));
        let element = div()
            .flex_none()
            .border_1()
            .border_color(p.border)
            .rounded(px(6.))
            .bg(p.muted)
            .px(px(5.6))
            .py(pad_y)
            .font_family(mono)
            .text_size(size)
            .line_height(ascent + descent)
            .font_weight(FontWeight::NORMAL)
            .text_color(p.foreground)
            .whitespace_nowrap()
            .child(node.as_text().to_owned());
        Some(InlineElement::new(element).with_baseline(edge + pad_y + ascent))
    }
}

/// An external link with text, `links.tsx`: the info ink with no underline, a 14 px globe before it. The link is one
/// piece on its line, so a long one keeps gpui-kit's own wrapping link run instead.
struct Link;

const LINK_MAX: usize = 48;

impl MarkdownPlugin for Link {
    fn name(&self) -> &str {
        "wsp-link"
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::Link(link) = node else { return None };
        if !(link.url.starts_with("https://") || link.url.starts_with("http://")) {
            return None;
        }
        let mut text = String::new();
        for child in &link.children {
            let mdast::Node::Text(t) = child else { return None };
            text.push_str(&t.value);
        }
        if text.is_empty() || text.contains('\n') || text.chars().count() > LINK_MAX {
            return None;
        }
        let source = cx.node_source(node).map_or_else(|| format!("[{text}]({})", link.url), str::to_owned);
        Some(MarkdownNode::new("wsp-link", SharedString::from(link.url.clone())).text(text).markdown(source))
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let url = node.data::<SharedString>()?.clone();
        let text = context.text_style();
        let (size, line_height) = (context.font_size(), context.line_height());
        let font = window.text_system().resolve_font(&text.font());
        let ascent = window.text_system().ascent(font, size);
        let descent = window.text_system().descent(font, size).abs();
        let baseline = (line_height - ascent - descent) / 2. + ascent;
        let globe = px(14.);
        let id = node.source_range().map_or(0, |range| range.start);
        let element = div()
            .id(("wsp-link", id))
            .flex()
            .flex_none()
            .items_start()
            .h(line_height)
            .text_color(cx.palette().info_foreground)
            .cursor_pointer()
            .child(Icon::new(Glyph::Globe).with_size(globe).ml(size * 0.25).mr(size * 0.2).mt(baseline + size * 0.125 - globe))
            .child(div().whitespace_nowrap().line_height(line_height).child(node.as_text().to_owned()))
            .on_click(move |_, _, cx| cx.open_url(&url));
        Some(InlineElement::new(element).with_baseline(baseline))
    }
}

/// `$$...$$`: KaTeX's display block, centred, 1em above and below. Without KaTeX the TeX shows in a serif italic.
struct DisplayMath;

impl MarkdownPlugin for DisplayMath {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "wsp-display-math"
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::Math(math) = node else { return None };
        let source = cx.node_source(node).map_or_else(|| format!("$$\n{}\n$$", math.value), str::to_owned);
        Some(MarkdownNode::new("wsp-display-math", ()).text(tex_text(&math.value)).markdown(source))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let edge = BODY - GAP_PX;
        div()
            .w_full()
            .py(edge)
            .flex()
            .justify_center()
            .font_family(serif(cx))
            .italic()
            .text_size(BODY * MATH_SCALE)
            .line_height(relative(1.2))
            .child(node.as_text().to_owned())
    }
}

/// `$...$`: KaTeX inline, in the same serif italic stand-in.
struct InlineMath;

impl MarkdownPlugin for InlineMath {
    fn name(&self) -> &str {
        "wsp-inline-math"
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::InlineMath(math) = node else { return None };
        let source = cx.node_source(node).map_or_else(|| format!("${}$", math.value), str::to_owned);
        Some(MarkdownNode::new("wsp-inline-math", ()).text(tex_text(&math.value)).markdown(source))
    }

    fn render_inline(&self, node: &MarkdownNode, context: &InlineRenderContext, _: &mut Window, cx: &mut App) -> Option<InlineElement> {
        let size = context.font_size() * MATH_SCALE;
        let element = div()
            .font_family(serif(cx))
            .italic()
            .text_size(size)
            .line_height(context.line_height())
            .text_color(context.text_style().color)
            .whitespace_nowrap()
            .child(node.as_text().to_owned());
        Some(InlineElement::new(element))
    }
}

fn serif(cx: &App) -> SharedString {
    static FACE: OnceLock<SharedString> = OnceLock::new();
    FACE.get_or_init(|| {
        let installed = cx.text_system().all_font_names();
        first_installed(&SERIF, &installed, cfg!(target_os = "macos")).unwrap_or_else(|| "serif".into())
    })
    .clone()
}

/// TeX as plain text a reader can follow: commands to their symbols, simple scripts to Unicode, braces dropped.
fn tex_text(tex: &str) -> String {
    let mut out = String::new();
    let mut chars = tex.trim().chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let mut name = String::new();
                while let Some(&n) = chars.peek().filter(|n| n.is_ascii_alphabetic()) {
                    name.push(n);
                    chars.next();
                }
                if name.is_empty() {
                    match chars.next() {
                        Some(',' | ';' | ':' | ' ' | '\\') => out.push(' '),
                        Some('!') => {}
                        Some(other) => out.push(other),
                        None => {}
                    }
                    continue;
                }
                if name == "frac" {
                    let (top, bottom) = (group(&mut chars), group(&mut chars));
                    out.push_str(&format!("{}/{}", tex_text(&top), tex_text(&bottom)));
                } else if name == "sqrt" {
                    out.push('√');
                    out.push_str(&tex_text(&group(&mut chars)));
                } else if let Some(symbol) = symbol(&name) {
                    out.push_str(symbol);
                } else {
                    out.push_str(&name);
                }
            }
            '^' | '_' => {
                let script = tex_text(&group(&mut chars));
                let mapped: Option<String> = script.chars().map(|s| if c == '^' { superscript(s) } else { subscript(s) }).collect();
                match mapped {
                    Some(mapped) => out.push_str(&mapped),
                    None => {
                        out.push(c);
                        out.push_str(&script);
                    }
                }
            }
            '{' | '}' => {}
            '\n' => out.push(' '),
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A command's argument: a `{...}` group, a command name, or one character.
fn group(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    while chars.next_if(|c| c.is_whitespace()).is_some() {}
    match chars.next() {
        Some('{') => {
            let mut depth = 1;
            let mut inner = String::new();
            for c in chars.by_ref() {
                depth += match c {
                    '{' => 1,
                    '}' => -1,
                    _ => 0,
                };
                if depth == 0 {
                    break;
                }
                inner.push(c);
            }
            inner
        }
        Some('\\') => {
            let mut name = String::from('\\');
            while let Some(n) = chars.next_if(|n| n.is_ascii_alphabetic()) {
                name.push(n);
            }
            name
        }
        Some(c) => c.to_string(),
        None => String::new(),
    }
}

fn symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" | "varepsilon" => "ε",
        "theta" => "θ",
        "lambda" => "λ",
        "mu" => "μ",
        "pi" => "π",
        "sigma" => "σ",
        "tau" => "τ",
        "phi" | "varphi" => "φ",
        "omega" => "ω",
        "Delta" => "Δ",
        "Sigma" => "Σ",
        "Omega" => "Ω",
        "int" => "∫",
        "sum" => "∑",
        "prod" => "∏",
        "infty" => "∞",
        "cdot" => "·",
        "times" => "×",
        "pm" => "±",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "to" | "rightarrow" => "→",
        "Rightarrow" | "implies" => "⇒",
        "in" => "∈",
        "partial" => "∂",
        "nabla" => "∇",
        "ldots" | "dots" | "cdots" => "…",
        "quad" | "qquad" => " ",
        "left" | "right" | "mathrm" | "mathbf" | "mathit" | "text" | "displaystyle" => "",
        _ => return None,
    })
}

fn superscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        _ => return None,
    })
}

fn subscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'n' => 'ₙ',
        'x' => 'ₓ',
        _ => return None,
    })
}

/// A person's message as the bubble reads it, the web's `lineBreaks` and `parseRawHtml={false}`: a single newline
/// breaks the line, and a tag shows as typed. Fenced code and code spans stay as written.
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
    use gpui_kit::base::text::TextViewState;
    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, Context, Entity, Modifiers, TestAppContext, Window};

    use super::{opens_with_heading, person_text, tex_text, view};
    use crate::ui::Side;

    struct Reply(Entity<TextViewState>);

    impl Render for Reply {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(view(&self.0, false, cx))
        }
    }

    #[gpui_kit::test]
    fn the_copy_button_copies_the_code_and_shows_a_check_for_a_moment(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| crate::ui::init(Side::Light, cx));
        let body = cx.new(|cx| TextViewState::markdown("Run it:\n\n```rust\nlet x = 1;\n```", cx));
        let reply = cx.new(|_| Reply(body));
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(reply.clone(), window, cx));
        cx.simulate_resize(size(px(760.), px(400.)));
        cx.run_until_parked();
        let copy = cx.debug_bounds("code-copy").expect("the copy button");
        // The web's header: the button 6 px in from the frame's right edge, inside its 1 px border.
        assert_eq!(copy.right(), px(760. - 1. - 6.));
        cx.simulate_click(copy.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("let x = 1;"));
        assert!(cx.debug_bounds("code-copied").is_some(), "a check after the copy");
        cx.executor().advance_clock(std::time::Duration::from_millis(1300));
        cx.run_until_parked();
        assert!(cx.debug_bounds("code-copy").is_some(), "the copy glyph again after 1.2 s");
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
    fn tex_reads_as_symbols() {
        assert_eq!(tex_text("E = mc^2"), "E = mc²");
        assert_eq!(tex_text("\\int_0^1 x^2 \\, dx = \\frac{1}{3}"), "∫₀¹ x² dx = 1/3");
        assert_eq!(tex_text("x_{i+1} \\le \\alpha"), "xᵢ₊₁ ≤ α");
        assert_eq!(tex_text("e^{-x^2}"), "e^-x²");
        assert_eq!(tex_text("\\sqrt{2}"), "√2");
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
