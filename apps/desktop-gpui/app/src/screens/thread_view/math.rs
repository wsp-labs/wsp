// SPDX-License-Identifier: AGPL-3.0-only
//! A reply's TeX as KaTeX draws it, in KaTeX's fonts and metrics by appendix G's rules; else its TeX as text.

mod layout;
mod parse;
mod tables;
mod text;
mod ttf;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::text::{markdown_ast as mdast, InlineElement, InlineRenderContext, MarkdownNode, MarkdownParseContext, MarkdownPlugin};
use gpui_kit::{
    canvas, div, fill, point, prelude::*, px, size, AnyElement, App, Bounds, Font, FontStyle, FontWeight, Global, Hsla, Pixels, Point,
    SharedString, TextAlign, TextRun, TransformationMatrix, Window,
};

use layout::{typeset, Boxed, Ink};
#[cfg(test)]
use parse::{parse, Atom};
use tables::{MAIN, MAIN_BOLD, MATH_ITALIC, SIZE1, SIZE2, SIZE3, SIZE4};
use text::tex_text;
use ttf::Ttf;

/// `.katex { font-size: 1.21em }`.
const SCALE: f32 = 1.21;
/// `.katex { line-height: 1.2 }`.
const LEADING: f32 = 1.2;

/// `$$...$$`: KaTeX's display block, centred, `margin: 1em 0`. Markdown's own gap already stands `gap` of that em.
pub struct DisplayMath {
    pub body: Pixels,
    pub gap: Pixels,
}

impl MarkdownPlugin for DisplayMath {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "wsp-display-math"
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let tex = match node {
            mdast::Node::Math(math) => math.value.as_str(),
            // `$$x$$` on a line of its own parses as a paragraph holding one inline formula.
            mdast::Node::Paragraph(paragraph) => {
                let [only @ mdast::Node::InlineMath(math)] = paragraph.children.as_slice() else { return None };
                cx.node_source(only)?.starts_with("$$").then_some(math.value.as_str())?
            }
            _ => return None,
        };
        let source = cx.node_source(node).map_or_else(|| format!("$$\n{tex}\n$$"), str::to_owned);
        let start = node.position()?.start.offset;
        let follows_display = cx.source().get(..start).is_some_and(|before| before.trim_end().ends_with("$$"));
        let formula = Formula { follows_display, ..Formula::new(tex, true) };
        Some(MarkdownNode::new("wsp-display-math", formula).text(tex_text(tex)).markdown(source))
    }

    fn render(&self, node: &MarkdownNode, window: &mut Window, cx: &mut App) -> impl IntoElement {
        load_fonts(cx);
        let em = self.body * SCALE;
        let (above, below) = strut(window, em);
        let formula = node.data::<Formula>();
        let body = match formula.and_then(|formula| formula.boxed.clone()) {
            Some(boxed) => drawn(boxed, em, above, below).0,
            None => fallback(node, em, above + below),
        };
        // Two display blocks' margins collapse into one em, which the gap below the first already nearly stands.
        let edge = self.body - self.gap;
        let top = if formula.is_some_and(|formula| formula.follows_display) { Pixels::ZERO } else { edge };
        div().w_full().pt(top).pb(edge).flex().justify_center().child(body)
    }
}

/// `$...$`, set inline in KaTeX's text style; `$$...$$` inside a line keeps the display style.
pub struct InlineMath;

impl MarkdownPlugin for InlineMath {
    fn name(&self) -> &str {
        "wsp-inline-math"
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::InlineMath(math) = node else { return None };
        let raw = cx.node_source(node)?;
        let end = node.position()?.end.offset;
        let display = reading(raw, cx.source().get(end..).and_then(|rest| rest.chars().next()))?;
        Some(MarkdownNode::new("wsp-inline-math", Formula::new(&math.value, display)).text(tex_text(&math.value)).markdown(raw))
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        load_fonts(cx);
        let em = context.font_size() * SCALE;
        let (above, below) = strut(window, em);
        let text = window.text_system().resolve_font(&context.text_style().font());
        let ascent = window.text_system().ascent(text, context.font_size());
        let descent = window.text_system().descent(text, context.font_size()).abs();
        let baseline = gpui_kit::line_baseline(context.line_height(), ascent, descent, window.scale_factor());
        let (above, below) = (above.max(baseline), below.max(context.line_height() - baseline));
        let Some(boxed) = node.data::<Formula>().and_then(|formula| formula.boxed.clone()) else {
            return Some(InlineElement::new(fallback(node, em, context.line_height())));
        };
        let (element, baseline) = drawn(boxed, em, above, below);
        Some(InlineElement::new(element).with_baseline(baseline))
    }
}

/// A formula as its node keeps it: set once when the markdown parses, drawn at whatever size the text has.
struct Formula {
    boxed: Option<Arc<Boxed>>,
    follows_display: bool,
}

impl Formula {
    fn new(tex: &str, display: bool) -> Self {
        Self { boxed: typeset(tex, display).map(Arc::new), follows_display: false }
    }
}

/// How `remarkPandocMath` reads a `$` span: display for `$$`, prose when a space hugs a dollar inside or a digit follows.
fn reading(raw: &str, next: Option<char>) -> Option<bool> {
    if raw.starts_with("$$") {
        return Some(true);
    }
    let inner = raw.strip_prefix('$')?.strip_suffix('$')?;
    let prose = inner.starts_with(char::is_whitespace) || inner.ends_with(char::is_whitespace) || next.is_some_and(|c| c.is_ascii_digit());
    (!prose).then_some(false)
}

struct Fonts;

impl Global for Fonts {}

/// The faces this typesetter draws with, in `Face`'s order, from katex 0.16.47's `dist/fonts`.
const FONTS: [&[u8]; 7] = [
    include_bytes!("../../../assets/fonts/katex/KaTeX_Main-Regular.ttf"),
    include_bytes!("../../../assets/fonts/katex/KaTeX_Main-Bold.ttf"),
    include_bytes!("../../../assets/fonts/katex/KaTeX_Math-Italic.ttf"),
    include_bytes!("../../../assets/fonts/katex/KaTeX_Size1-Regular.ttf"),
    include_bytes!("../../../assets/fonts/katex/KaTeX_Size2-Regular.ttf"),
    include_bytes!("../../../assets/fonts/katex/KaTeX_Size3-Regular.ttf"),
    include_bytes!("../../../assets/fonts/katex/KaTeX_Size4-Regular.ttf"),
];

/// Hands KaTeX's faces to the text system the first time a formula draws, so a thread without math never loads them.
fn load_fonts(cx: &mut App) {
    if cx.has_global::<Fonts>() {
        return;
    }
    if let Err(error) = cx.text_system().add_fonts(FONTS.iter().map(|bytes| Cow::Borrowed(*bytes)).collect()) {
        eprintln!("desktop-gpui: KaTeX's fonts did not load: {error}");
    }
    cx.set_global(Fonts);
}

/// The room `.katex`'s own line box takes above and below the baseline: KaTeX_Main's ascent and descent, half-leaded.
fn strut(window: &Window, em: Pixels) -> (Pixels, Pixels) {
    let font = window.text_system().resolve_font(&Face::Main.font());
    let ascent = window.text_system().ascent(font, em);
    let descent = window.text_system().descent(font, em).abs();
    let half = (em * LEADING - ascent - descent) / 2.;
    (ascent + half, descent + half)
}

/// The formula painted on a box at least `above` and `below` its baseline, and that baseline.
fn drawn(boxed: Arc<Boxed>, em: Pixels, above: Pixels, below: Pixels) -> (AnyElement, Pixels) {
    let top = (em * boxed.height).max(above);
    let bottom = (em * boxed.depth).max(below);
    let width = em * boxed.width;
    let element =
        canvas(|_, _, _| {}, move |bounds, _, window, cx| paint(&boxed, point(bounds.origin.x, bounds.origin.y + top), em, window, cx))
            .flex_none()
            .w(width)
            .h(top + bottom)
            .into_any_element();
    (element, top)
}

fn paint(boxed: &Boxed, origin: Point<Pixels>, em: Pixels, window: &mut Window, cx: &mut App) {
    let color = window.text_style().color;
    for ink in &boxed.ink {
        match *ink {
            Ink::Glyph { x, y, ch, face, scale } if face.outlined() => {
                paint_size_glyph(face, ch, point(origin.x + em * x, origin.y + em * y), em * scale, color, window, cx);
            }
            Ink::Glyph { x, y, ch, face, scale } => {
                let text = SharedString::from(ch.to_string());
                let run =
                    TextRun { len: text.len(), font: face.font(), color, background_color: None, underline: None, strikethrough: None };
                let line = window.text_system().shape_line(text, em * scale, &[run], None);
                let at = point(origin.x + em * x, origin.y + em * y - line.ascent);
                line.paint(at, line.ascent + line.descent, TextAlign::Left, None, window, cx).ok();
            }
            Ink::Rule { x, y, width, height } => {
                // `.frac-line { min-height: 1px }`, and the browser lays a border on whole pixels.
                let top = (origin.y + em * y).round();
                let bounds = Bounds::new(point(origin.x + em * x, top), size(em * width, (em * height).round().max(px(1.))));
                window.paint_quad(fill(bounds, color));
            }
        }
    }
}

/// The most KaTeX_Size glyphs kept as images; the four faces hold 90 that KaTeX measures, so the cache never fills in use.
const SIZE_GLYPHS: usize = 128;

/// A KaTeX_Size glyph as a monochrome SVG built once from its outline, which GPUI rasterizes once per size into its sprite
/// atlas: those faces hold no `m`, so GPUI's `load_family` takes none as a font. Its box is in em off the pen.
struct SizeGlyph {
    key: SharedString,
    svg: Arc<[u8]>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

/// A Size glyph built once, or none when the face lacks it.
type SizeEntry = Option<Rc<SizeGlyph>>;

thread_local! {
    static SIZE_CACHE: RefCell<HashMap<(Face, char), SizeEntry>> = RefCell::new(HashMap::new());
    #[cfg(test)]
    static SIZE_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn size_glyph(face: Face, ch: char) -> Option<Rc<SizeGlyph>> {
    SIZE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(held) = cache.get(&(face, ch)) {
            return held.clone();
        }
        if cache.len() >= SIZE_GLYPHS {
            cache.clear();
        }
        let built = build_size_glyph(face, ch).map(Rc::new);
        cache.insert((face, ch), built.clone());
        built
    })
}

fn build_size_glyph(face: Face, ch: char) -> Option<SizeGlyph> {
    #[cfg(test)]
    SIZE_BUILDS.with(|n| n.set(n.get() + 1));
    let contours = Ttf::new(FONTS[face as usize])?.outline(ch)?;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y, _) in contours.iter().flatten() {
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    if x0 > x1 {
        return None;
    }
    // A little room past the outline, so the edge's antialiasing is not cut off.
    let pad = 0.02;
    (x0, y0, x1, y1) = (x0 - pad, y0 - pad, x1 + pad, y1 + pad);
    // In thousandths of an em, y down.
    let at = |x: f32, y: f32| format!("{:.1} {:.1}", x * 1000., -y * 1000.);
    let mut d = String::new();
    for contour in &contours {
        let Some(start) = contour.iter().position(|p| p.2) else { continue };
        _ = write!(d, "M{}", at(contour[start].0, contour[start].1));
        let mut control: Option<(f32, f32)> = None;
        for step in 1..=contour.len() {
            let (x, y, on) = contour[(start + step) % contour.len()];
            match (on, control) {
                (true, Some((qx, qy))) => _ = write!(d, "Q{} {}", at(qx, qy), at(x, y)),
                (true, None) => _ = write!(d, "L{}", at(x, y)),
                (false, Some((qx, qy))) => _ = write!(d, "Q{} {}", at(qx, qy), at((qx + x) / 2., (qy + y) / 2.)),
                (false, None) => {}
            }
            control = (!on).then_some((x, y));
        }
        d.push('Z');
    }
    let (width, height) = (x1 - x0, y1 - y0);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.1}" height="{h:.1}" viewBox="{:.1} {:.1} {w:.1} {h:.1}"><path d="{d}"/></svg>"#,
        x0 * 1000.,
        -y1 * 1000.,
        w = width * 1000.,
        h = height * 1000.,
    );
    let key = format!("katex/{face:?}/{:x}", u32::from(ch)).into();
    Some(SizeGlyph { key, svg: svg.into_bytes().into(), left: x0, top: y1, width, height })
}

fn paint_size_glyph(face: Face, ch: char, at: Point<Pixels>, font_size: Pixels, color: Hsla, window: &mut Window, cx: &App) {
    let Some(glyph) = size_glyph(face, ch) else { return };
    let origin = point(at.x + font_size * glyph.left, at.y - font_size * glyph.top);
    let bounds = Bounds::new(origin, size(font_size * glyph.width, font_size * glyph.height));
    window.paint_svg(bounds, glyph.key.clone(), Some(&glyph.svg), TransformationMatrix::unit(), color, cx).ok();
}

/// The TeX as text, in KaTeX_Main, for a formula outside what this typesetter sets.
fn fallback(node: &MarkdownNode, em: Pixels, line_height: Pixels) -> AnyElement {
    div()
        .font_family("KaTeX_Main")
        .text_size(em)
        .line_height(line_height)
        .whitespace_nowrap()
        .child(node.as_text().to_owned())
        .into_any_element()
}

/// A KaTeX face. Each draws only the glyphs its metrics table lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Main,
    MainBold,
    MathItalic,
    Size1,
    Size2,
    Size3,
    Size4,
}

impl Face {
    fn font(self) -> Font {
        let (family, weight, style) = match self {
            Face::Main => ("KaTeX_Main", FontWeight::NORMAL, FontStyle::Normal),
            Face::MainBold => ("KaTeX_Main", FontWeight::BOLD, FontStyle::Normal),
            Face::MathItalic => ("KaTeX_Math", FontWeight::NORMAL, FontStyle::Italic),
            Face::Size1 => ("KaTeX_Size1", FontWeight::NORMAL, FontStyle::Normal),
            Face::Size2 => ("KaTeX_Size2", FontWeight::NORMAL, FontStyle::Normal),
            Face::Size3 => ("KaTeX_Size3", FontWeight::NORMAL, FontStyle::Normal),
            Face::Size4 => ("KaTeX_Size4", FontWeight::NORMAL, FontStyle::Normal),
        };
        Font { weight, style, ..gpui_kit::font(family) }
    }

    fn outlined(self) -> bool {
        matches!(self, Face::Size1 | Face::Size2 | Face::Size3 | Face::Size4)
    }

    fn sized(n: usize) -> Face {
        [Face::Size1, Face::Size2, Face::Size3, Face::Size4][n - 1]
    }

    fn metrics(self, ch: char) -> Option<Metrics> {
        let table = match self {
            Face::Main => MAIN,
            Face::MainBold => MAIN_BOLD,
            Face::MathItalic => MATH_ITALIC,
            Face::Size1 => SIZE1,
            Face::Size2 => SIZE2,
            Face::Size3 => SIZE3,
            Face::Size4 => SIZE4,
        };
        let ix = table.binary_search_by_key(&ch, |(c, _)| *c).ok()?;
        let [depth, height, italic, _skew, width] = table[ix].1;
        Some(Metrics { depth, height, italic, width })
    }
}

#[derive(Clone, Copy, Debug)]
struct Metrics {
    depth: f32,
    height: f32,
    italic: f32,
    width: f32,
}

/// An atom's class, which sets the space TeX puts between two atoms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

/// The glue between two atoms in mu, KaTeX's `spacingData.ts`; `tight` in script styles.
fn glue(prev: Class, next: Class, tight: bool) -> f32 {
    use Class::*;
    let (thin, medium, thick) = (3., 4., 5.);
    if tight {
        return match (prev, next) {
            (Ord | Close | Inner, Op) | (Op, Ord | Op) => thin,
            _ => 0.,
        };
    }
    match (prev, next) {
        (Ord, Op | Inner) | (Op, Ord | Op | Inner) | (Close, Op | Inner) => thin,
        (Punct, Ord | Op | Open | Close | Punct | Inner) | (Inner, Ord | Op | Open | Punct | Inner) => thin,
        (Ord | Close | Inner, Bin) | (Bin, Ord | Op | Open | Inner) => medium,
        (Ord | Op | Close | Punct | Inner, Rel) | (Rel, Ord | Op | Open | Inner) => thick,
        _ => 0.,
    }
}

/// TeX's styles: display, text, script and scriptscript, each cramped or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Style {
    size: u8,
    cramped: bool,
}

const DISPLAY: Style = Style { size: 0, cramped: false };
const TEXT: Style = Style { size: 1, cramped: false };

// KaTeX's `sigmasAndXis`, per size: text and display, script, scriptscript.
const X_HEIGHT: [f32; 3] = [0.431, 0.431, 0.431];
const QUAD: [f32; 3] = [1.0, 1.171, 1.472];
const NUM1: [f32; 3] = [0.677, 0.732, 0.925];
const NUM2: [f32; 3] = [0.394, 0.384, 0.387];
const NUM3: [f32; 3] = [0.444, 0.471, 0.504];
const DENOM1: [f32; 3] = [0.686, 0.752, 1.025];
const DENOM2: [f32; 3] = [0.345, 0.344, 0.532];
const SUP1: [f32; 3] = [0.413, 0.503, 0.504];
const SUP2: [f32; 3] = [0.363, 0.431, 0.404];
const SUP3: [f32; 3] = [0.289, 0.286, 0.294];
const SUB1: [f32; 3] = [0.15, 0.143, 0.2];
const SUB2: [f32; 3] = [0.247, 0.286, 0.4];
const SUP_DROP: [f32; 3] = [0.386, 0.353, 0.494];
const SUB_DROP: [f32; 3] = [0.05, 0.071, 0.1];
const DELIM1: [f32; 3] = [2.39, 1.7, 1.98];
const DELIM2: [f32; 3] = [1.01, 1.157, 1.42];
const AXIS: [f32; 3] = [0.25, 0.25, 0.25];
const RULE: [f32; 3] = [0.04, 0.049, 0.049];
const BIG_OP1: [f32; 3] = [0.111, 0.111, 0.111];
const BIG_OP2: [f32; 3] = [0.166, 0.166, 0.166];
const BIG_OP3: [f32; 3] = [0.2, 0.2, 0.2];
const BIG_OP4: [f32; 3] = [0.6, 0.611, 0.611];
const BIG_OP5: [f32; 3] = [0.1, 0.143, 0.143];
/// `\scriptspace`, 0.5 pt, after every script.
const SCRIPT_SPACE: f32 = 0.05;
/// `.nulldelimiter`'s width, either side of a fraction.
const NULL_DELIMITER: f32 = 0.12;

impl Style {
    fn sup(self) -> Style {
        Style { size: [2, 2, 3, 3][self.size as usize], cramped: self.cramped }
    }

    fn sub(self) -> Style {
        Style { cramped: true, ..self.sup() }
    }

    fn num(self) -> Style {
        Style { size: [1, 2, 3, 3][self.size as usize], cramped: self.cramped }
    }

    fn den(self) -> Style {
        Style { cramped: true, ..self.num() }
    }

    fn cramp(self) -> Style {
        Style { cramped: true, ..self }
    }

    fn tight(self) -> bool {
        self.size >= 2
    }

    /// The style's font size against the formula's.
    fn scale(self) -> f32 {
        [1., 1., 0.7, 0.5][self.size as usize]
    }

    /// A font parameter at this style's size, in the formula's em, as all lengths here are.
    fn sigma(self, values: [f32; 3]) -> f32 {
        values[[0, 0, 1, 2][self.size as usize]] * self.scale()
    }

    fn mu(self, mu: f32) -> f32 {
        mu * self.sigma(QUAD) / 18.
    }
}

#[cfg(test)]
mod tests;
