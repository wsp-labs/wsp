// SPDX-License-Identifier: AGPL-3.0-only
//! A reply's TeX as KaTeX draws it, in KaTeX's fonts and metrics by appendix G's rules; else its TeX as text.

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

/// Just enough of a TrueType font to read a glyph's outline: `cmap` format 4, `loca` and `glyf`.
struct Ttf<'a> {
    data: &'a [u8],
    cmap: usize,
    loca: usize,
    glyf: usize,
    long: bool,
    units: f32,
}

impl<'a> Ttf<'a> {
    fn u16(&self, at: usize) -> Option<u16> {
        Some(u16::from_be_bytes(self.data.get(at..at + 2)?.try_into().ok()?))
    }

    fn i16(&self, at: usize) -> Option<i16> {
        self.u16(at).map(|v| v as i16)
    }

    fn u32(&self, at: usize) -> Option<usize> {
        Some(u32::from_be_bytes(self.data.get(at..at + 4)?.try_into().ok()?) as usize)
    }

    fn new(data: &'a [u8]) -> Option<Self> {
        let mut font = Ttf { data, cmap: 0, loca: 0, glyf: 0, long: false, units: 1000. };
        let mut head = 0;
        for table in 0..font.u16(4)? as usize {
            let record = 12 + 16 * table;
            let offset = font.u32(record + 8)?;
            match data.get(record..record + 4)? {
                b"cmap" => font.cmap = offset,
                b"loca" => font.loca = offset,
                b"glyf" => font.glyf = offset,
                b"head" => head = offset,
                _ => {}
            }
        }
        font.units = f32::from(font.u16(head + 18)?);
        font.long = font.i16(head + 50)? == 1;
        Some(font)
    }

    fn glyph(&self, ch: char) -> Option<u16> {
        let c = u16::try_from(u32::from(ch)).ok()?;
        for record in 0..self.u16(self.cmap + 2)? as usize {
            let table = self.cmap + self.u32(self.cmap + 4 + 8 * record + 4)?;
            if self.u16(table)? != 4 {
                continue;
            }
            let segments = self.u16(table + 6)? as usize;
            let ends = table + 14;
            let (starts, deltas, ranges) = (ends + segments + 2, ends + 2 * segments + 2, ends + 3 * segments + 2);
            for seg in (0..segments).step_by(2) {
                if c > self.u16(ends + seg)? {
                    continue;
                }
                let start = self.u16(starts + seg)?;
                if c < start {
                    return None;
                }
                let delta = self.u16(deltas + seg)?;
                let range = self.u16(ranges + seg)? as usize;
                if range == 0 {
                    return Some(c.wrapping_add(delta));
                }
                let glyph = self.u16(ranges + seg + range + 2 * usize::from(c - start))?;
                return (glyph != 0).then(|| glyph.wrapping_add(delta));
            }
        }
        None
    }

    /// The glyph's contours: points in em, `true` when on the curve.
    fn outline(&self, ch: char) -> Option<Vec<Vec<(f32, f32, bool)>>> {
        let mut contours = Vec::new();
        self.contours(self.glyph(ch)?, (0., 0., 1.), &mut contours, 0)?;
        Some(contours)
    }

    fn contours(&self, glyph: u16, (dx, dy, scale): (f32, f32, f32), out: &mut Vec<Vec<(f32, f32, bool)>>, depth: u8) -> Option<()> {
        let glyph = usize::from(glyph);
        let (start, end) = if self.long {
            (self.u32(self.loca + 4 * glyph)?, self.u32(self.loca + 4 * glyph + 4)?)
        } else {
            (2 * self.u16(self.loca + 2 * glyph)? as usize, 2 * self.u16(self.loca + 2 * glyph + 2)? as usize)
        };
        if start == end {
            return Some(());
        }
        let at = self.glyf + start;
        let count = self.i16(at)?;
        let point = |x: i32, y: i32, on: bool| (dx + x as f32 * scale / self.units, dy + y as f32 * scale / self.units, on);
        if count < 0 {
            // A composite: its parts, each offset and maybe scaled.
            let mut p = at + 10;
            loop {
                if depth > 4 {
                    return None;
                }
                let (flags, part) = (self.u16(p)?, self.u16(p + 2)?);
                p += 4;
                let (x, y) = if flags & 1 != 0 {
                    p += 4;
                    (f32::from(self.i16(p - 4)?), f32::from(self.i16(p - 2)?))
                } else {
                    p += 2;
                    (f32::from(*self.data.get(p - 2)? as i8), f32::from(*self.data.get(p - 1)? as i8))
                };
                let mut part_scale = 1.;
                if flags & 8 != 0 {
                    part_scale = f32::from(self.i16(p)?) / 16384.;
                    p += 2;
                } else if flags & 0x40 != 0 {
                    part_scale = f32::from(self.i16(p)?) / 16384.;
                    p += 4;
                } else if flags & 0x80 != 0 {
                    part_scale = f32::from(self.i16(p)?) / 16384.;
                    p += 8;
                }
                let offset = (dx + x * scale / self.units, dy + y * scale / self.units, scale * part_scale);
                self.contours(part, offset, out, depth + 1)?;
                if flags & 0x20 == 0 {
                    return Some(());
                }
            }
        }
        let count = count as usize;
        let mut ends = Vec::with_capacity(count);
        for contour in 0..count {
            ends.push(self.u16(at + 10 + 2 * contour)? as usize);
        }
        let points = ends.last().map_or(0, |last| last + 1);
        let mut p = at + 12 + 2 * count + self.u16(at + 10 + 2 * count)? as usize;
        let mut flags = Vec::with_capacity(points);
        while flags.len() < points {
            let flag = *self.data.get(p)?;
            p += 1;
            flags.push(flag);
            if flag & 8 != 0 {
                let repeat = *self.data.get(p)?;
                p += 1;
                flags.extend(std::iter::repeat_n(flag, usize::from(repeat)));
            }
        }
        let mut read = |short: u8, same: u8| -> Option<Vec<i32>> {
            let mut value = 0i32;
            let mut values = Vec::with_capacity(points);
            for &flag in &flags[..points] {
                if flag & short != 0 {
                    let delta = i32::from(*self.data.get(p)?);
                    p += 1;
                    value += if flag & same != 0 { delta } else { -delta };
                } else if flag & same == 0 {
                    value += i32::from(self.i16(p)?);
                    p += 2;
                }
                values.push(value);
            }
            Some(values)
        };
        let xs = read(2, 16)?;
        let ys = read(4, 32)?;
        let mut first = 0;
        for end in ends {
            out.push((first..=end).map(|i| point(xs[i], ys[i], flags[i] & 1 != 0)).collect());
            first = end + 1;
        }
        Some(())
    }
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

/// A parsed piece of TeX.
#[derive(Clone, Debug, PartialEq)]
enum Atom {
    /// A glyph; `text` glyphs (`\text`) take no italic correction.
    Sym {
        ch: char,
        class: Class,
        face: Face,
        text: bool,
    },
    /// `{...}`, and `\mathrel{...}` and its kin, which give the group their class.
    Group(Class, Vec<Atom>),
    Scripts {
        base: Box<Atom>,
        sup: Option<Vec<Atom>>,
        sub: Option<Vec<Atom>>,
    },
    Frac {
        num: Vec<Atom>,
        den: Vec<Atom>,
        bar: bool,
        parens: bool,
        style: Option<Style>,
    },
    Sqrt(Vec<Atom>),
    /// A big operator's glyph (`\sum`), or a named one's letters (`\lim`).
    Op {
        glyph: Option<char>,
        name: Vec<Atom>,
        limits: bool,
    },
    LeftRight {
        left: Option<char>,
        right: Option<char>,
        body: Vec<Atom>,
    },
    /// `\big(` and its kin: a delimiter of size 1 to 4.
    Big {
        delim: Option<char>,
        size: usize,
        class: Class,
    },
    Kern(Len),
    Style(Style),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Len {
    Mu(f32),
    /// An em of the current style.
    Em(f32),
    /// An em of the formula, as `\quad` is in every style.
    Quad(f32),
}

/// What `\mathrm`, `\mathbf` and plain math do to a letter or a digit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Variant {
    Normal,
    Rm,
    Bf,
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Char(char),
    Command(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Eof,
    Brace,
    Right,
}

/// What stopped the parse: the command or character this typesetter does not set.
type Parsed<T> = Result<T, String>;

const BIG_OPS: [(&str, char, bool); 11] = [
    ("sum", '\u{2211}', true),
    ("prod", '\u{220f}', true),
    ("coprod", '\u{2210}', true),
    ("bigcup", '\u{22c3}', true),
    ("bigcap", '\u{22c2}', true),
    ("bigoplus", '\u{2a01}', true),
    ("bigotimes", '\u{2a02}', true),
    ("bigodot", '\u{2a00}', true),
    ("int", '\u{222b}', false),
    ("iint", '\u{222c}', false),
    ("oint", '\u{222e}', false),
];

/// KaTeX's named operators (`op.ts`): their letters, a space for a thin space, and whether display sets limits.
const NAMED_OPS: [(&str, &str, bool); 32] = [
    ("arccos", "arccos", false),
    ("arcsin", "arcsin", false),
    ("arctan", "arctan", false),
    ("arg", "arg", false),
    ("cos", "cos", false),
    ("cosh", "cosh", false),
    ("cot", "cot", false),
    ("coth", "coth", false),
    ("csc", "csc", false),
    ("deg", "deg", false),
    ("dim", "dim", false),
    ("exp", "exp", false),
    ("hom", "hom", false),
    ("ker", "ker", false),
    ("lg", "lg", false),
    ("ln", "ln", false),
    ("log", "log", false),
    ("sec", "sec", false),
    ("sin", "sin", false),
    ("sinh", "sinh", false),
    ("tan", "tan", false),
    ("tanh", "tanh", false),
    ("det", "det", true),
    ("gcd", "gcd", true),
    ("inf", "inf", true),
    ("lim", "lim", true),
    ("liminf", "lim inf", true),
    ("limsup", "lim sup", true),
    ("max", "max", true),
    ("min", "min", true),
    ("Pr", "Pr", true),
    ("sup", "sup", true),
];

/// How deep an agent's groups may nest before the formula shows as its TeX: deeper would overflow a parse thread's stack.
const MAX_NESTING: usize = 64;

struct Parser {
    chars: Vec<char>,
    at: usize,
    depth: usize,
}

/// TeX to atoms, or the first thing this typesetter does not set.
fn parse(tex: &str) -> Parsed<Vec<Atom>> {
    let mut parser = Parser { chars: tex.chars().collect(), at: 0, depth: 0 };
    parser.list(End::Eof, Variant::Normal)
}

impl Parser {
    fn skip_space(&mut self) {
        while let Some(&c) = self.chars.get(self.at) {
            if c == '%' {
                while self.chars.get(self.at).is_some_and(|&c| c != '\n') {
                    self.at += 1;
                }
            } else if c.is_whitespace() {
                self.at += 1;
            } else {
                break;
            }
        }
    }

    fn next(&mut self) -> Parsed<Option<Token>> {
        self.skip_space();
        let Some(&c) = self.chars.get(self.at) else { return Ok(None) };
        self.at += 1;
        if c != '\\' {
            return Ok(Some(Token::Char(c)));
        }
        let start = self.at;
        while self.chars.get(self.at).is_some_and(|c| c.is_ascii_alphabetic()) {
            self.at += 1;
        }
        if self.at == start {
            let &c = self.chars.get(self.at).ok_or("\\")?;
            self.at += 1;
            return Ok(Some(Token::Command(c.to_string())));
        }
        Ok(Some(Token::Command(self.chars[start..self.at].iter().collect())))
    }

    fn peek(&mut self) -> Parsed<Option<Token>> {
        let at = self.at;
        let token = self.next();
        self.at = at;
        token
    }

    fn list(&mut self, end: End, variant: Variant) -> Parsed<Vec<Atom>> {
        self.deeper(|parser| parser.items(end, variant))
    }

    /// Parses `inner` one level deeper: a group, or an argument that is itself a command (`\sqrt\sqrt x`).
    fn deeper<T>(&mut self, inner: impl FnOnce(&mut Self) -> Parsed<T>) -> Parsed<T> {
        if self.depth == MAX_NESTING {
            return Err("groups nested too deep".into());
        }
        self.depth += 1;
        let parsed = inner(self);
        self.depth -= 1;
        parsed
    }

    fn items(&mut self, end: End, variant: Variant) -> Parsed<Vec<Atom>> {
        let mut atoms = Vec::new();
        loop {
            let Some(token) = self.peek()? else {
                return if end == End::Eof { Ok(atoms) } else { Err("an unclosed group".into()) };
            };
            match &token {
                Token::Char('}') => {
                    self.next()?;
                    return if end == End::Brace { Ok(atoms) } else { Err("}".into()) };
                }
                Token::Command(name) if name == "right" => {
                    return if end == End::Right { Ok(atoms) } else { Err("\\right".into()) };
                }
                Token::Char(c @ ('^' | '_')) => {
                    let sup = *c == '^';
                    self.next()?;
                    let script = self.argument(variant)?;
                    attach(&mut atoms, sup, script)?;
                }
                Token::Char('\'') => {
                    let mut primes = Vec::new();
                    while self.peek()? == Some(Token::Char('\'')) {
                        self.next()?;
                        primes.push(symbol("\\prime").ok_or("\\prime")?);
                    }
                    if self.peek()? == Some(Token::Char('^')) {
                        self.next()?;
                        primes.extend(self.argument(variant)?);
                    }
                    attach(&mut atoms, true, primes)?;
                }
                Token::Command(name) if name == "limits" || name == "nolimits" => {
                    self.next()?;
                    let Some(Atom::Op { limits, .. }) = atoms.last_mut() else { return Err(format!("\\{name}")) };
                    *limits = name == "limits";
                }
                _ => {
                    self.next()?;
                    atoms.push(self.atom(token, variant)?);
                }
            }
        }
    }

    /// A command's argument or a script: a `{...}` group, or one atom.
    fn argument(&mut self, variant: Variant) -> Parsed<Vec<Atom>> {
        match self.next()? {
            Some(Token::Char('{')) => self.list(End::Brace, variant),
            Some(Token::Char(c @ ('}' | '^' | '_' | '&'))) => Err(c.to_string()),
            Some(token) => self.deeper(|parser| Ok(vec![parser.atom(token, variant)?])),
            None => Err("a missing argument".into()),
        }
    }

    fn atom(&mut self, token: Token, variant: Variant) -> Parsed<Atom> {
        match token {
            Token::Char('{') => Ok(Atom::Group(Class::Ord, self.list(End::Brace, variant)?)),
            Token::Char(c) if c.is_ascii_alphanumeric() => {
                let face = match variant {
                    Variant::Bf => Face::MainBold,
                    Variant::Rm => Face::Main,
                    Variant::Normal if c.is_ascii_digit() => Face::Main,
                    Variant::Normal => Face::MathItalic,
                };
                Ok(Atom::Sym { ch: c, class: Class::Ord, face, text: false })
            }
            Token::Char('~') => Ok(Atom::Kern(Len::Em(0.25))),
            Token::Char(c) => {
                let named = symbol(c.encode_utf8(&mut [0; 4]));
                named.or_else(|| SYMBOLS.iter().find(|row| row.0.starts_with('\\') && row.1 == c).map(sym)).ok_or_else(|| c.to_string())
            }
            Token::Command(name) => self.command(&name, variant),
        }
    }

    fn command(&mut self, name: &str, variant: Variant) -> Parsed<Atom> {
        let class = |class: Class, list: Vec<Atom>| Atom::Group(class, list);
        Ok(match name {
            "frac" | "dfrac" | "tfrac" | "binom" | "dbinom" | "tbinom" => {
                let (num, den) = (self.argument(variant)?, self.argument(variant)?);
                let style = match name.as_bytes()[0] {
                    b'd' => Some(DISPLAY),
                    b't' => Some(TEXT),
                    _ => None,
                };
                let bar = name.ends_with("frac");
                Atom::Frac { num, den, bar, parens: !bar, style }
            }
            "sqrt" => {
                if self.peek()? == Some(Token::Char('[')) {
                    return Err("\\sqrt[".into());
                }
                Atom::Sqrt(self.argument(variant)?)
            }
            "left" => {
                let left = self.delimiter()?;
                let body = self.list(End::Right, variant)?;
                self.next()?;
                let right = self.delimiter()?;
                Atom::LeftRight { left, right, body }
            }
            "text" | "textrm" | "textnormal" | "textup" | "mbox" => class(Class::Ord, self.text(Face::Main)?),
            "textbf" => class(Class::Ord, self.text(Face::MainBold)?),
            "mathrm" => class(Class::Ord, self.argument(Variant::Rm)?),
            "mathbf" => class(Class::Ord, self.argument(Variant::Bf)?),
            "mathit" | "mathnormal" => class(Class::Ord, self.argument(Variant::Normal)?),
            "operatorname" => {
                let limits = self.peek()? == Some(Token::Char('*'));
                if limits {
                    self.next()?;
                }
                Atom::Op { glyph: None, name: self.argument(Variant::Rm)?, limits }
            }
            "mathord" => class(Class::Ord, self.argument(variant)?),
            "mathop" => class(Class::Op, self.argument(variant)?),
            "mathbin" => class(Class::Bin, self.argument(variant)?),
            "mathrel" => class(Class::Rel, self.argument(variant)?),
            "mathopen" => class(Class::Open, self.argument(variant)?),
            "mathclose" => class(Class::Close, self.argument(variant)?),
            "mathpunct" => class(Class::Punct, self.argument(variant)?),
            "mathinner" => class(Class::Inner, self.argument(variant)?),
            "displaystyle" => Atom::Style(DISPLAY),
            "textstyle" => Atom::Style(TEXT),
            "scriptstyle" => Atom::Style(Style { size: 2, cramped: false }),
            "scriptscriptstyle" => Atom::Style(Style { size: 3, cramped: false }),
            "," | "thinspace" => Atom::Kern(Len::Mu(3.)),
            ":" | ">" | "medspace" => Atom::Kern(Len::Mu(4.)),
            ";" | "thickspace" => Atom::Kern(Len::Mu(5.)),
            "!" | "negthinspace" => Atom::Kern(Len::Mu(-3.)),
            " " | "space" | "nobreakspace" => Atom::Kern(Len::Em(0.25)),
            "quad" => Atom::Kern(Len::Quad(1.)),
            "qquad" => Atom::Kern(Len::Quad(2.)),
            "neq" | "ne" => class(Class::Rel, vec![symbol("\\@not").ok_or("\\@not")?, symbol("=").ok_or("=")?]),
            "colon" => class(Class::Punct, vec![symbol(":").ok_or(":")?]),
            "cdots" | "dotsb" | "dotsm" | "dotsi" => symbol("\\@cdots").ok_or("\\@cdots")?,
            "dots" | "dotsc" | "dotso" => symbol("\\ldots").ok_or("\\ldots")?,
            "vert" => symbol("|").ok_or("|")?,
            "Vert" => symbol("\\|").ok_or("\\|")?,
            _ => {
                if let Some(&(_, glyph, limits)) = BIG_OPS.iter().find(|op| op.0 == name) {
                    return Ok(Atom::Op { glyph: Some(glyph), name: Vec::new(), limits });
                }
                if let Some(&(_, letters, limits)) = NAMED_OPS.iter().find(|op| op.0 == name) {
                    let name = letters
                        .chars()
                        .map(|ch| {
                            if ch == ' ' {
                                Atom::Kern(Len::Mu(3.))
                            } else {
                                Atom::Sym { ch, class: Class::Ord, face: Face::Main, text: false }
                            }
                        })
                        .collect();
                    return Ok(Atom::Op { glyph: None, name, limits });
                }
                if let Some(size) = big(name) {
                    let class = match name.as_bytes()[name.len() - 1] {
                        b'l' => Class::Open,
                        b'r' => Class::Close,
                        b'm' => Class::Rel,
                        _ => Class::Ord,
                    };
                    return Ok(Atom::Big { delim: self.delimiter()?, size, class });
                }
                symbol(&format!("\\{name}")).ok_or_else(|| format!("\\{name}"))?
            }
        })
    }

    /// A delimiter after `\left`, `\right` or `\big`: its glyph, or none for `.`.
    fn delimiter(&mut self) -> Parsed<Option<char>> {
        let token = self.next()?.ok_or("a missing delimiter")?;
        let ch = match &token {
            Token::Char('.') => return Ok(None),
            Token::Char(c @ ('(' | ')' | '[' | ']' | '/')) => *c,
            Token::Char('|') => '\u{2223}',
            Token::Char('<') => '\u{27e8}',
            Token::Char('>') => '\u{27e9}',
            Token::Command(name) => match name.as_str() {
                "{" | "lbrace" => '{',
                "}" | "rbrace" => '}',
                "lbrack" => '[',
                "rbrack" => ']',
                "lparen" => '(',
                "rparen" => ')',
                "langle" => '\u{27e8}',
                "rangle" => '\u{27e9}',
                "lfloor" => '\u{230a}',
                "rfloor" => '\u{230b}',
                "lceil" => '\u{2308}',
                "rceil" => '\u{2309}',
                "vert" | "lvert" | "rvert" => '\u{2223}',
                "|" | "Vert" | "lVert" | "rVert" => '\u{2225}',
                "backslash" => '\\',
                other => return Err(format!("\\{other} as a delimiter")),
            },
            Token::Char(other) => return Err(format!("{other} as a delimiter")),
        };
        Ok(Some(ch))
    }

    /// `\text{...}`'s characters, upright in `face`, its spaces a quarter em each.
    fn text(&mut self, face: Face) -> Parsed<Vec<Atom>> {
        self.skip_space();
        if self.chars.get(self.at) != Some(&'{') {
            return Err("\\text without braces".into());
        }
        self.at += 1;
        let mut atoms = Vec::new();
        let mut space = false;
        loop {
            let &c = self.chars.get(self.at).ok_or("an unclosed \\text")?;
            self.at += 1;
            let ch = match c {
                '}' => return Ok(atoms),
                '{' | '$' | '^' | '_' => return Err(format!("{c} in \\text")),
                '\\' => {
                    let &escaped = self.chars.get(self.at).ok_or("\\")?;
                    self.at += 1;
                    if !"$%&_#{} ".contains(escaped) {
                        return Err(format!("\\{escaped} in \\text"));
                    }
                    escaped
                }
                c => c,
            };
            if ch.is_whitespace() {
                if !space {
                    atoms.push(Atom::Sym { ch: '\u{a0}', class: Class::Ord, face: Face::Main, text: true });
                }
                space = true;
                continue;
            }
            space = false;
            face.metrics(ch).ok_or_else(|| format!("{ch} in \\text"))?;
            atoms.push(Atom::Sym { ch, class: Class::Ord, face, text: true });
        }
    }
}

/// `\big`, `\Big`, `\bigg` and `\Bigg`, with or without `l`, `r` or `m`: the delimiter size each asks for.
fn big(name: &str) -> Option<usize> {
    let stem = name.strip_suffix(['l', 'r', 'm']).unwrap_or(name);
    ["big", "Big", "bigg", "Bigg"].iter().position(|&s| s == stem).map(|ix| ix + 1)
}

/// A script on the atom before it: `{}` stands in for a script with nothing before it.
fn attach(atoms: &mut Vec<Atom>, sup: bool, script: Vec<Atom>) -> Parsed<()> {
    let (base, mut sups, mut subs) = match atoms.pop() {
        Some(Atom::Scripts { base, sup, sub }) => (base, sup, sub),
        Some(atom) => (Box::new(atom), None, None),
        None => (Box::new(Atom::Group(Class::Ord, Vec::new())), None, None),
    };
    let slot = if sup { &mut sups } else { &mut subs };
    if slot.is_some() {
        return Err(if sup { "a double superscript" } else { "a double subscript" }.into());
    }
    *slot = Some(script);
    atoms.push(Atom::Scripts { base, sup: sups, sub: subs });
    Ok(())
}

fn sym(row: &(&str, char, Class, Face)) -> Atom {
    Atom::Sym { ch: row.1, class: row.2, face: row.3, text: false }
}

fn symbol(name: &str) -> Option<Atom> {
    SYMBOLS.binary_search_by_key(&name, |row| row.0).ok().map(|ix| sym(&SYMBOLS[ix]))
}

/// What a formula's ink is: a glyph on a baseline `y` below the box's, or a rule whose top is at `y`. In em.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
    Glyph { x: f32, y: f32, ch: char, face: Face, scale: f32 },
    Rule { x: f32, y: f32, width: f32, height: f32 },
}

impl Ink {
    fn moved(self, dx: f32, dy: f32) -> Ink {
        match self {
            Ink::Glyph { x, y, ch, face, scale } => Ink::Glyph { x: x + dx, y: y + dy, ch, face, scale },
            Ink::Rule { x, y, width, height } => Ink::Rule { x: x + dx, y: y + dy, width, height },
        }
    }
}

/// A TeX box: its width, height above and depth below the baseline, and its ink, all in the formula's em.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Boxed {
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub ink: Vec<Ink>,
}

impl Boxed {
    fn kern(width: f32) -> Boxed {
        Boxed { width, ..Boxed::default() }
    }

    fn glyph(ch: char, face: Face, scale: f32, italic: bool) -> Boxed {
        let m = face.metrics(ch).unwrap_or(Metrics { depth: 0., height: 0., italic: 0., width: 0. });
        let correction = if italic { m.italic.max(0.) } else { 0. };
        Boxed {
            width: (m.width + correction) * scale,
            height: m.height * scale,
            depth: m.depth * scale,
            ink: vec![Ink::Glyph { x: 0., y: 0., ch, face, scale }],
        }
    }

    /// `other` with its left edge at `dx` and its baseline `dy` below this box's.
    fn place(&mut self, other: &Boxed, dx: f32, dy: f32) {
        self.ink.extend(other.ink.iter().map(|ink| ink.moved(dx, dy)));
        self.height = self.height.max(other.height - dy);
        self.depth = self.depth.max(other.depth + dy);
    }

    fn append(&mut self, other: &Boxed) {
        self.place(other, self.width, 0.);
        self.width += other.width;
    }
}

/// An atom set in a style: its box, its class (none for a kern), and a lone glyph's italic correction.
struct Laid {
    boxed: Boxed,
    class: Option<Class>,
    italic: Option<f32>,
}

/// A formula's box, or none when it holds something this typesetter does not set.
fn typeset(tex: &str, display: bool) -> Option<Boxed> {
    let atoms = parse(tex).ok()?;
    Some(hlist(&atoms, if display { DISPLAY } else { TEXT }))
}

/// A math list side by side with TeX's glue, binary operators out of place made ordinary (`buildExpression`).
fn hlist(atoms: &[Atom], style: Style) -> Boxed {
    let mut current = style;
    let mut laid: Vec<(Laid, Style)> = Vec::new();
    for atom in atoms {
        if let Atom::Style(next) = atom {
            current = *next;
            continue;
        }
        laid.push((layout(atom, current), current));
    }
    let mut prev: Option<usize> = None;
    for ix in 0..laid.len() {
        let Some(class) = laid[ix].0.class else { continue };
        let before = prev.and_then(|p| laid[p].0.class);
        if before == Some(Class::Bin) && matches!(class, Class::Rel | Class::Close | Class::Punct) {
            laid[prev.unwrap_or(ix)].0.class = Some(Class::Ord);
        } else if class == Class::Bin && matches!(before, None | Some(Class::Bin | Class::Open | Class::Rel | Class::Op | Class::Punct)) {
            laid[ix].0.class = Some(Class::Ord);
        }
        prev = Some(ix);
    }
    if let Some(last) = prev.filter(|&p| laid[p].0.class == Some(Class::Bin)) {
        laid[last].0.class = Some(Class::Ord);
    }
    let mut out = Boxed::default();
    let mut before: Option<Class> = None;
    for (item, item_style) in &laid {
        if let (Some(prev), Some(class)) = (before, item.class) {
            let mu = glue(prev, class, item_style.tight());
            if mu != 0. {
                out.append(&Boxed::kern(style.mu(mu)));
            }
        }
        out.append(&item.boxed);
        before = item.class.or(before);
    }
    out
}

fn layout(atom: &Atom, style: Style) -> Laid {
    let plain = |boxed: Boxed, class: Class| Laid { boxed, class: Some(class), italic: None };
    match atom {
        Atom::Sym { ch, class, face, text } => {
            let boxed = Boxed::glyph(*ch, *face, style.scale(), !text);
            let italic = if *text { 0. } else { face.metrics(*ch).map_or(0., |m| m.italic.max(0.)) * style.scale() };
            Laid { boxed, class: Some(*class), italic: Some(italic) }
        }
        Atom::Group(class, list) => plain(hlist(list, style), *class),
        Atom::Kern(len) => {
            let width = match *len {
                Len::Mu(mu) => style.mu(mu),
                Len::Em(em) => em * style.scale(),
                Len::Quad(em) => em,
            };
            Laid { boxed: Boxed::kern(width), class: None, italic: None }
        }
        Atom::Style(_) => Laid { boxed: Boxed::default(), class: None, italic: None },
        Atom::Op { .. } => {
            let (mut boxed, italic, shift) = op_base(atom, style);
            boxed.ink.iter_mut().for_each(|ink| *ink = ink.moved(0., shift));
            Laid { boxed, class: Some(Class::Op), italic }
        }
        Atom::Scripts { base, sup, sub } => scripts(base, sup.as_deref(), sub.as_deref(), style),
        Atom::Frac { num, den, bar, parens, style: own } => plain(frac(num, den, *bar, *parens, own.unwrap_or(style)), Class::Ord),
        Atom::Sqrt(body) => plain(sqrt(body, style), Class::Ord),
        Atom::LeftRight { left, right, body } => {
            let inner = hlist(body, style);
            let axis = style.sigma(AXIS);
            let reach = (inner.height - axis).max(inner.depth + axis);
            let total = (reach / 500. * 901.).max(2. * reach - 0.5);
            let mut boxed = delimiter(*left, total, style);
            boxed.append(&inner);
            boxed.append(&delimiter(*right, total, style));
            plain(boxed, Class::Inner)
        }
        Atom::Big { delim, size, class } => plain(sized_delimiter(*delim, *size, style), *class),
    }
}

/// A big operator's box, its italic correction when a glyph, and how far below the baseline it sits to centre on the axis.
fn op_base(op: &Atom, style: Style) -> (Boxed, Option<f32>, f32) {
    let Atom::Op { glyph, name, .. } = op else { return (layout(op, style).boxed, None, 0.) };
    match glyph {
        Some(ch) => {
            let face = if style.size == 0 { Face::Size2 } else { Face::Size1 };
            let boxed = Boxed::glyph(*ch, face, style.scale(), true);
            let italic = face.metrics(*ch).map_or(0., |m| m.italic) * style.scale();
            let shift = (boxed.height - boxed.depth) / 2. - style.sigma(AXIS);
            (boxed, Some(italic), shift)
        }
        None => (hlist(name, style), None, 0.),
    }
}

/// Whether an atom is a single character, which takes scripts at the font's own shifts (`isCharacterBox`).
fn character(atom: &Atom) -> bool {
    match atom {
        Atom::Sym { .. } => true,
        Atom::Group(Class::Ord, list) => matches!(list.as_slice(), [only] if character(only)),
        _ => false,
    }
}

/// Rules 18a to 18f: scripts after their nucleus; or, for an operator with limits in display style, above and below.
fn scripts(base: &Atom, sup: Option<&[Atom]>, sub: Option<&[Atom]>, style: Style) -> Laid {
    if let Atom::Op { limits: true, .. } = base {
        if style.size == 0 {
            return limits(base, sup, sub, style);
        }
    }
    let nucleus = layout(base, style);
    let char_box = character(base);
    let sup = sup.map(|list| hlist(list, style.sup()));
    let sub = sub.map(|list| hlist(list, style.sub()));
    let x_height = style.sigma(X_HEIGHT);
    let mut sup_shift = if char_box { 0. } else { nucleus.boxed.height - style.sup().sigma(SUP_DROP) };
    let mut sub_shift = if char_box { 0. } else { nucleus.boxed.depth + style.sub().sigma(SUB_DROP) };
    let min_sup = style.sigma(if style == DISPLAY {
        SUP1
    } else if style.cramped {
        SUP3
    } else {
        SUP2
    });
    // The subscript tucks under a lone glyph's italic correction.
    let tuck = -nucleus.italic.unwrap_or(0.);
    match (&sup, &sub) {
        (Some(up), Some(down)) => {
            sup_shift = sup_shift.max(min_sup).max(up.depth + 0.25 * x_height);
            sub_shift = sub_shift.max(style.sigma(SUB2));
            let gap = 4. * style.sigma(RULE);
            if (sup_shift - up.depth) - (down.height - sub_shift) < gap {
                sub_shift = gap - (sup_shift - up.depth) + down.height;
                let psi = 0.8 * x_height - (sup_shift - up.depth);
                if psi > 0. {
                    sup_shift += psi;
                    sub_shift -= psi;
                }
            }
        }
        (None, Some(down)) => sub_shift = sub_shift.max(style.sigma(SUB1)).max(down.height - 0.8 * x_height),
        (Some(up), None) => sup_shift = sup_shift.max(min_sup).max(up.depth + 0.25 * x_height),
        (None, None) => {}
    }
    let mut boxed = nucleus.boxed;
    let x = boxed.width;
    let mut width = 0f32;
    if let Some(up) = &sup {
        boxed.place(up, x, -sup_shift);
        width = width.max(up.width + SCRIPT_SPACE);
    }
    if let Some(down) = &sub {
        boxed.place(down, x + tuck, sub_shift);
        width = width.max(tuck + down.width + SCRIPT_SPACE);
    }
    boxed.width = x + width;
    Laid { boxed, class: nucleus.class.or(Some(Class::Ord)), italic: None }
}

/// A piece of a vertical list, bottom first: a kern, or a box with its left margin.
enum Piece {
    Kern(f32),
    Box(Boxed, f32),
}

/// A vertical list whose bottom stands `depth` below the baseline, each box centred on the widest (`makeVList`).
fn vlist(pieces: Vec<Piece>, depth: f32) -> Boxed {
    let width = pieces.iter().map(|p| if let Piece::Box(b, margin) = p { b.width + margin } else { 0. }).fold(0., f32::max);
    let mut out = Boxed { width, ..Boxed::default() };
    let (mut at, mut low, mut high) = (-depth, -depth, -depth);
    for piece in pieces {
        match piece {
            Piece::Kern(k) => at += k,
            Piece::Box(b, margin) => {
                out.place(&b, (width - b.width - margin) / 2. + margin, -(at + b.depth));
                at += b.height + b.depth;
            }
        }
        (low, high) = (low.min(at), high.max(at));
    }
    out.height = high;
    out.depth = -low;
    out
}

/// `assembleSupSub`: limits above and below a big operator, each shifted by half its slant.
fn limits(op: &Atom, sup: Option<&[Atom]>, sub: Option<&[Atom]>, style: Style) -> Laid {
    let (base, italic, shift) = op_base(op, style);
    let slant = italic.unwrap_or(0.);
    let sup = sup.map(|list| {
        let b = hlist(list, style.sup());
        let kern = style.sigma(BIG_OP1).max(style.sigma(BIG_OP3) - b.depth);
        (b, kern)
    });
    let sub_char = sub.is_some_and(|list| matches!(list, [only] if character(only)));
    let sub = sub.map(|list| {
        let b = hlist(list, style.sub());
        let kern = style.sigma(BIG_OP2).max(style.sigma(BIG_OP4) - b.height);
        (b, kern)
    });
    let pad = style.sigma(BIG_OP5);
    let (base_height, base_depth) = (base.height, base.depth);
    let has_sub = sub.is_some();
    let stack = match (sup, sub) {
        (Some((up, up_kern)), Some((down, down_kern))) => {
            let depth = pad + down.height + down.depth + down_kern + base_depth + shift;
            let pieces = vec![
                Piece::Kern(pad),
                Piece::Box(down, -slant),
                Piece::Kern(down_kern),
                Piece::Box(base, 0.),
                Piece::Kern(up_kern),
                Piece::Box(up, slant),
                Piece::Kern(pad),
            ];
            vlist(pieces, depth)
        }
        (None, Some((down, down_kern))) => {
            let total = pad + down.height + down.depth + down_kern + base_height + base_depth;
            let depth = total - (base_height - shift);
            vlist(vec![Piece::Kern(pad), Piece::Box(down, -slant), Piece::Kern(down_kern), Piece::Box(base, 0.)], depth)
        }
        (Some((up, up_kern)), None) => {
            vlist(vec![Piece::Box(base, 0.), Piece::Kern(up_kern), Piece::Box(up, slant), Piece::Kern(pad)], base_depth + shift)
        }
        (None, None) => base,
    };
    let mut boxed = Boxed::default();
    if has_sub && slant != 0. && !sub_char {
        boxed.append(&Boxed::kern(slant));
    }
    boxed.append(&stack);
    Laid { boxed, class: Some(Class::Op), italic: None }
}

/// Rules 15a to 15e: numerator over denominator, the bar on the axis, null delimiters or parentheses either side.
fn frac(num: &[Atom], den: &[Atom], bar: bool, parens: bool, style: Style) -> Boxed {
    let n = hlist(num, style.num());
    let d = hlist(den, style.den());
    let rule = style.sigma(RULE);
    let thickness = if bar { rule } else { 0. };
    let display = style.size == 0;
    let mut num_shift = style.sigma(if display {
        NUM1
    } else if bar {
        NUM2
    } else {
        NUM3
    });
    let mut den_shift = style.sigma(if display { DENOM1 } else { DENOM2 });
    let clearance = match (display, bar) {
        (true, true) => 3. * rule,
        (true, false) => 7. * rule,
        (false, true) => rule,
        (false, false) => 3. * rule,
    };
    let axis = style.sigma(AXIS);
    if bar {
        let above = (num_shift - n.depth) - (axis + thickness / 2.);
        if above < clearance {
            num_shift += clearance - above;
        }
        let below = (axis - thickness / 2.) - (d.height - den_shift);
        if below < clearance {
            den_shift += clearance - below;
        }
    } else {
        let gap = (num_shift - n.depth) - (d.height - den_shift);
        if gap < clearance {
            num_shift += (clearance - gap) / 2.;
            den_shift += (clearance - gap) / 2.;
        }
    }
    let width = n.width.max(d.width);
    let mut stack = Boxed { width, ..Boxed::default() };
    stack.place(&n, (width - n.width) / 2., -num_shift);
    stack.place(&d, (width - d.width) / 2., den_shift);
    if bar {
        stack.ink.push(Ink::Rule { x: 0., y: -(axis + thickness / 2.), width, height: thickness });
    }
    if !parens {
        let mut boxed = Boxed::kern(NULL_DELIMITER);
        boxed.append(&stack);
        boxed.append(&Boxed::kern(NULL_DELIMITER));
        return boxed;
    }
    let size = match style.size {
        0 => style.sigma(DELIM1),
        3 => Style { size: 2, cramped: false }.sigma(DELIM2),
        _ => style.sigma(DELIM2),
    };
    let mut boxed = delimiter(Some('('), size, style);
    boxed.append(&stack);
    boxed.append(&delimiter(Some(')'), size, style));
    boxed
}

/// Rule 11, with KaTeX's surds: the font's radical sized to the body, its vinculum drawn on to the body's right edge.
fn sqrt(body: &[Atom], style: Style) -> Boxed {
    let inner = hlist(body, style.cramp());
    let m = style.scale();
    let height = if inner.height == 0. { style.sigma(X_HEIGHT) } else { inner.height };
    let theta = style.sigma(RULE);
    let phi = if style.size == 0 { style.sigma(X_HEIGHT) } else { theta };
    let mut clearance = theta + phi / 4.;
    let needed = height + inner.depth + clearance + theta;
    let (face, glyph_scale, tex_height, advance, rule) = match surd(needed, style) {
        None => {
            let options = needed / m;
            let surd_scale = if options < 1. {
                1.
            } else if options < 1.4 {
                0.7
            } else {
                m
            };
            (Face::Main, m / surd_scale, m / surd_scale, 0.833 * m / surd_scale, 0.04 * surd_scale * m)
        }
        Some(n) => (Face::sized(n), 1., [1.2, 1.8, 2.4, 3.0][n - 1], 1., 0.04 * m * m),
    };
    let surd_depth = tex_height - rule;
    if surd_depth > height + inner.depth + clearance {
        clearance = (clearance + surd_depth - height - inner.depth) / 2.;
    }
    let shift = tex_height - height - clearance - rule;
    let top = height + clearance + rule;
    let mut boxed = Boxed { width: advance + inner.width, height: top + rule, depth: inner.depth.max(shift), ink: Vec::new() };
    let glyph_top = face.metrics('\u{221a}').map_or(0., |g| g.height) * glyph_scale;
    boxed.ink.push(Ink::Glyph { x: 0., y: glyph_top - top, ch: '\u{221a}', face, scale: glyph_scale });
    boxed.ink.push(Ink::Rule { x: advance, y: -top, width: inner.width, height: rule });
    boxed.place(&inner, advance, 0.);
    boxed
}

/// Which surd KaTeX picks for a body this tall: a small one (none), or the size font that first exceeds it.
fn surd(needed: f32, style: Style) -> Option<usize> {
    let start = 2usize.min(3 - style.size as usize);
    for step in start..7 {
        let fits = if step < 3 { [0.5, 0.7, 1.][step] > needed } else { [1.2, 1.8, 2.4, 3.0][step - 3] > needed };
        if fits {
            return (step >= 3).then_some(step - 2);
        }
    }
    Some(4)
}

/// `makeCustomSizedDelim`, centred on the axis: the first of the delimiter's sizes taller than `total`.
fn delimiter(ch: Option<char>, total: f32, style: Style) -> Boxed {
    let Some(ch) = ch else { return Boxed::kern(NULL_DELIMITER) };
    let bars = matches!(ch, '\u{2223}' | '\u{2225}');
    let start = 2usize.min(3 - style.size as usize);
    let m = style.scale();
    for step in start..7 {
        let (face, scale) = match step {
            0..=2 => (Face::Main, [0.5, 0.7, 1.][step]),
            _ if bars => break,
            _ => (Face::sized(step - 2), 1.),
        };
        let Some(g) = face.metrics(ch) else { break };
        if (g.height + g.depth) * scale > total {
            return centred(ch, face, scale, 0.25 * (scale - m));
        }
    }
    if bars {
        return stacked(ch, total, style);
    }
    centred(ch, Face::Size4, 1., 0.25 * (1. - m))
}

/// `\big(` and its kin: the size font's glyph at the formula's size; bars stand as tall as the size asks.
fn sized_delimiter(ch: Option<char>, size: usize, style: Style) -> Boxed {
    let Some(ch) = ch else { return Boxed::kern(NULL_DELIMITER) };
    if matches!(ch, '\u{2223}' | '\u{2225}') {
        return stacked(ch, [1.2, 1.8, 2.4, 3.0][size - 1], style);
    }
    centred(ch, Face::sized(size), 1., 0.)
}

fn centred(ch: char, face: Face, scale: f32, shift: f32) -> Boxed {
    let glyph = Boxed::glyph(ch, face, scale, false);
    let mut boxed = Boxed::kern(glyph.width);
    boxed.place(&glyph, 0., shift);
    boxed.height = glyph.height - shift;
    boxed.depth = glyph.depth + shift;
    boxed
}

/// A bar delimiter taller than its glyph, as KaTeX's `vert` and `doublevert` paths draw it: 0.6 em pieces about the axis.
fn stacked(ch: char, total: f32, style: Style) -> Boxed {
    let real = 1.2 + ((total - 1.2) / 0.6).ceil().max(0.) * 0.6;
    let axis = style.sigma(AXIS);
    let (width, strokes): (f32, &[f32]) = if ch == '\u{2225}' { (0.556, &[0.145, 0.367]) } else { (0.333, &[0.145]) };
    let mut boxed = Boxed { width, height: real / 2. + axis, depth: real / 2. - axis, ink: Vec::new() };
    for &x in strokes {
        boxed.ink.push(Ink::Rule { x, y: -(real / 2. + axis), width: 0.041, height: real });
    }
    boxed
}

/// TeX as readable text: what copying a formula gives, and what shows when it cannot be set.
fn tex_text(tex: &str) -> String {
    let depth = tex.chars().scan(0i64, |d, c| {
        *d += match c {
            '{' => 1,
            '}' => -1,
            _ => 0,
        };
        Some(*d)
    });
    if depth.max().unwrap_or(0) > MAX_NESTING as i64 {
        return tex.trim().to_owned();
    }
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
                if name == "begin" || name == "end" {
                    group(&mut chars);
                    continue;
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
                } else if let Some(symbol) = text_symbol(&name) {
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
            '\n' | '&' => out.push(' '),
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

fn text_symbol(name: &str) -> Option<&'static str> {
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

/// KaTeX's math-mode symbols this typesetter draws (`symbols.ts`): the command or character, its glyph, its class and face.
#[rustfmt::skip]
const SYMBOLS: &[(&str, char, Class, Face)] = &[
    ("!", '!', Class::Close, Face::Main), ("(", '(', Class::Open, Face::Main), (")", ')', Class::Close, Face::Main),
    ("*", '\u{2217}', Class::Bin, Face::Main), ("+", '+', Class::Bin, Face::Main), (",", ',', Class::Punct, Face::Main),
    ("-", '\u{2212}', Class::Bin, Face::Main), (".", '.', Class::Ord, Face::Main), ("/", '/', Class::Ord, Face::Main),
    ("0", '0', Class::Ord, Face::Main), ("1", '1', Class::Ord, Face::Main), ("2", '2', Class::Ord, Face::Main),
    ("3", '3', Class::Ord, Face::Main), ("4", '4', Class::Ord, Face::Main), ("5", '5', Class::Ord, Face::Main),
    ("6", '6', Class::Ord, Face::Main), ("7", '7', Class::Ord, Face::Main), ("8", '8', Class::Ord, Face::Main),
    ("9", '9', Class::Ord, Face::Main), (":", ':', Class::Rel, Face::Main), (";", ';', Class::Punct, Face::Main),
    ("<", '<', Class::Rel, Face::Main), ("=", '=', Class::Rel, Face::Main), (">", '>', Class::Rel, Face::Main),
    ("?", '?', Class::Close, Face::Main), ("[", '[', Class::Open, Face::Main), ("\\#", '#', Class::Ord, Face::Main),
    ("\\$", '$', Class::Ord, Face::Main), ("\\%", '%', Class::Ord, Face::Main), ("\\&", '&', Class::Ord, Face::Main),
    ("\\@cdots", '\u{22ef}', Class::Inner, Face::Main), ("\\@not", '\u{e020}', Class::Rel, Face::Main),
    ("\\Delta", '\u{394}', Class::Ord, Face::Main), ("\\Gamma", '\u{393}', Class::Ord, Face::Main),
    ("\\Im", '\u{2111}', Class::Ord, Face::Main), ("\\Lambda", '\u{39b}', Class::Ord, Face::Main),
    ("\\Leftarrow", '\u{21d0}', Class::Rel, Face::Main), ("\\Leftrightarrow", '\u{21d4}', Class::Rel, Face::Main),
    ("\\Longleftrightarrow", '\u{27fa}', Class::Rel, Face::Main), ("\\Longrightarrow", '\u{27f9}', Class::Rel, Face::Main),
    ("\\Omega", '\u{3a9}', Class::Ord, Face::Main), ("\\Phi", '\u{3a6}', Class::Ord, Face::Main),
    ("\\Pi", '\u{3a0}', Class::Ord, Face::Main), ("\\Psi", '\u{3a8}', Class::Ord, Face::Main),
    ("\\Re", '\u{211c}', Class::Ord, Face::Main), ("\\Rightarrow", '\u{21d2}', Class::Rel, Face::Main),
    ("\\Sigma", '\u{3a3}', Class::Ord, Face::Main), ("\\Theta", '\u{398}', Class::Ord, Face::Main),
    ("\\Upsilon", '\u{3a5}', Class::Ord, Face::Main), ("\\Xi", '\u{39e}', Class::Ord, Face::Main), ("\\_", '_', Class::Ord, Face::Main),
    ("\\aleph", '\u{2135}', Class::Ord, Face::Main), ("\\alpha", '\u{3b1}', Class::Ord, Face::MathItalic),
    ("\\angle", '\u{2220}', Class::Ord, Face::Main), ("\\approx", '\u{2248}', Class::Rel, Face::Main),
    ("\\ast", '\u{2217}', Class::Bin, Face::Main), ("\\asymp", '\u{224d}', Class::Rel, Face::Main),
    ("\\backslash", '\\', Class::Ord, Face::Main), ("\\beta", '\u{3b2}', Class::Ord, Face::MathItalic),
    ("\\bot", '\u{22a5}', Class::Ord, Face::Main), ("\\bullet", '\u{2219}', Class::Bin, Face::Main),
    ("\\cap", '\u{2229}', Class::Bin, Face::Main), ("\\cdot", '\u{22c5}', Class::Bin, Face::Main),
    ("\\cdotp", '\u{22c5}', Class::Punct, Face::Main), ("\\chi", '\u{3c7}', Class::Ord, Face::MathItalic),
    ("\\circ", '\u{2218}', Class::Bin, Face::Main), ("\\cong", '\u{2245}', Class::Rel, Face::Main),
    ("\\cup", '\u{222a}', Class::Bin, Face::Main), ("\\dashv", '\u{22a3}', Class::Rel, Face::Main),
    ("\\ddots", '\u{22f1}', Class::Inner, Face::Main), ("\\delta", '\u{3b4}', Class::Ord, Face::MathItalic),
    ("\\div", '\u{f7}', Class::Bin, Face::Main), ("\\doteq", '\u{2250}', Class::Rel, Face::Main),
    ("\\downarrow", '\u{2193}', Class::Rel, Face::Main), ("\\ell", '\u{2113}', Class::Ord, Face::Main),
    ("\\emptyset", '\u{2205}', Class::Ord, Face::Main), ("\\epsilon", '\u{3f5}', Class::Ord, Face::MathItalic),
    ("\\equiv", '\u{2261}', Class::Rel, Face::Main), ("\\eta", '\u{3b7}', Class::Ord, Face::MathItalic),
    ("\\exists", '\u{2203}', Class::Ord, Face::Main), ("\\forall", '\u{2200}', Class::Ord, Face::Main),
    ("\\gamma", '\u{3b3}', Class::Ord, Face::MathItalic), ("\\ge", '\u{2265}', Class::Rel, Face::Main),
    ("\\geq", '\u{2265}', Class::Rel, Face::Main), ("\\gets", '\u{2190}', Class::Rel, Face::Main),
    ("\\gg", '\u{226b}', Class::Rel, Face::Main), ("\\in", '\u{2208}', Class::Rel, Face::Main),
    ("\\infty", '\u{221e}', Class::Ord, Face::Main), ("\\iota", '\u{3b9}', Class::Ord, Face::MathItalic),
    ("\\kappa", '\u{3ba}', Class::Ord, Face::MathItalic), ("\\lVert", '\u{2225}', Class::Open, Face::Main),
    ("\\lambda", '\u{3bb}', Class::Ord, Face::MathItalic), ("\\land", '\u{2227}', Class::Bin, Face::Main),
    ("\\langle", '\u{27e8}', Class::Open, Face::Main), ("\\lbrace", '{', Class::Open, Face::Main),
    ("\\lbrack", '[', Class::Open, Face::Main), ("\\lceil", '\u{2308}', Class::Open, Face::Main),
    ("\\ldotp", '.', Class::Punct, Face::Main), ("\\ldots", '\u{2026}', Class::Inner, Face::Main),
    ("\\le", '\u{2264}', Class::Rel, Face::Main), ("\\leftarrow", '\u{2190}', Class::Rel, Face::Main),
    ("\\leftrightarrow", '\u{2194}', Class::Rel, Face::Main), ("\\leq", '\u{2264}', Class::Rel, Face::Main),
    ("\\lfloor", '\u{230a}', Class::Open, Face::Main), ("\\ll", '\u{226a}', Class::Rel, Face::Main),
    ("\\lnot", '\u{ac}', Class::Ord, Face::Main), ("\\longleftrightarrow", '\u{27f7}', Class::Rel, Face::Main),
    ("\\longrightarrow", '\u{27f6}', Class::Rel, Face::Main), ("\\lor", '\u{2228}', Class::Bin, Face::Main),
    ("\\lparen", '(', Class::Open, Face::Main), ("\\lvert", '\u{2223}', Class::Open, Face::Main),
    ("\\mapsto", '\u{21a6}', Class::Rel, Face::Main), ("\\mid", '\u{2223}', Class::Rel, Face::Main),
    ("\\models", '\u{22a8}', Class::Rel, Face::Main), ("\\mp", '\u{2213}', Class::Bin, Face::Main),
    ("\\mu", '\u{3bc}', Class::Ord, Face::MathItalic), ("\\nabla", '\u{2207}', Class::Ord, Face::Main),
    ("\\neg", '\u{ac}', Class::Ord, Face::Main), ("\\ni", '\u{220b}', Class::Rel, Face::Main),
    ("\\nu", '\u{3bd}', Class::Ord, Face::MathItalic), ("\\odot", '\u{2299}', Class::Bin, Face::Main),
    ("\\omega", '\u{3c9}', Class::Ord, Face::MathItalic), ("\\ominus", '\u{2296}', Class::Bin, Face::Main),
    ("\\oplus", '\u{2295}', Class::Bin, Face::Main), ("\\otimes", '\u{2297}', Class::Bin, Face::Main),
    ("\\owns", '\u{220b}', Class::Rel, Face::Main), ("\\parallel", '\u{2225}', Class::Rel, Face::Main),
    ("\\partial", '\u{2202}', Class::Ord, Face::Main), ("\\perp", '\u{22a5}', Class::Rel, Face::Main),
    ("\\phi", '\u{3d5}', Class::Ord, Face::MathItalic), ("\\pi", '\u{3c0}', Class::Ord, Face::MathItalic),
    ("\\pm", '\u{b1}', Class::Bin, Face::Main), ("\\prec", '\u{227a}', Class::Rel, Face::Main),
    ("\\prime", '\u{2032}', Class::Ord, Face::Main), ("\\propto", '\u{221d}', Class::Rel, Face::Main),
    ("\\psi", '\u{3c8}', Class::Ord, Face::MathItalic), ("\\rVert", '\u{2225}', Class::Close, Face::Main),
    ("\\rangle", '\u{27e9}', Class::Close, Face::Main), ("\\rbrace", '}', Class::Close, Face::Main),
    ("\\rbrack", ']', Class::Close, Face::Main), ("\\rceil", '\u{2309}', Class::Close, Face::Main),
    ("\\rfloor", '\u{230b}', Class::Close, Face::Main), ("\\rho", '\u{3c1}', Class::Ord, Face::MathItalic),
    ("\\rightarrow", '\u{2192}', Class::Rel, Face::Main), ("\\rparen", ')', Class::Close, Face::Main),
    ("\\rvert", '\u{2223}', Class::Close, Face::Main), ("\\setminus", '\u{2216}', Class::Bin, Face::Main),
    ("\\sigma", '\u{3c3}', Class::Ord, Face::MathItalic), ("\\sim", '\u{223c}', Class::Rel, Face::Main),
    ("\\simeq", '\u{2243}', Class::Rel, Face::Main), ("\\star", '\u{22c6}', Class::Bin, Face::Main),
    ("\\subset", '\u{2282}', Class::Rel, Face::Main), ("\\subseteq", '\u{2286}', Class::Rel, Face::Main),
    ("\\succ", '\u{227b}', Class::Rel, Face::Main), ("\\supset", '\u{2283}', Class::Rel, Face::Main),
    ("\\supseteq", '\u{2287}', Class::Rel, Face::Main), ("\\tau", '\u{3c4}', Class::Ord, Face::MathItalic),
    ("\\theta", '\u{3b8}', Class::Ord, Face::MathItalic), ("\\times", '\u{d7}', Class::Bin, Face::Main),
    ("\\to", '\u{2192}', Class::Rel, Face::Main), ("\\top", '\u{22a4}', Class::Ord, Face::Main),
    ("\\triangle", '\u{25b3}', Class::Ord, Face::Main), ("\\uparrow", '\u{2191}', Class::Rel, Face::Main),
    ("\\upsilon", '\u{3c5}', Class::Ord, Face::MathItalic), ("\\varepsilon", '\u{3b5}', Class::Ord, Face::MathItalic),
    ("\\varphi", '\u{3c6}', Class::Ord, Face::MathItalic), ("\\varpi", '\u{3d6}', Class::Ord, Face::MathItalic),
    ("\\varrho", '\u{3f1}', Class::Ord, Face::MathItalic), ("\\varsigma", '\u{3c2}', Class::Ord, Face::MathItalic),
    ("\\vartheta", '\u{3d1}', Class::Ord, Face::MathItalic), ("\\vdash", '\u{22a2}', Class::Rel, Face::Main),
    ("\\vee", '\u{2228}', Class::Bin, Face::Main), ("\\wedge", '\u{2227}', Class::Bin, Face::Main),
    ("\\wp", '\u{2118}', Class::Ord, Face::Main), ("\\xi", '\u{3be}', Class::Ord, Face::MathItalic),
    ("\\zeta", '\u{3b6}', Class::Ord, Face::MathItalic), ("\\{", '{', Class::Open, Face::Main),
    ("\\|", '\u{2225}', Class::Ord, Face::Main), ("\\}", '}', Class::Close, Face::Main), ("]", ']', Class::Close, Face::Main),
    ("|", '\u{2223}', Class::Ord, Face::Main),
];

/// `fontMetricsData.js`'s Main-Regular: depth, height, italic correction, skew and width, in em.
#[rustfmt::skip]
const MAIN: &[(char, [f32; 5])] = &[
    (' ', [0.0, 0.0, 0.0, 0.0, 0.25]), ('!', [0.0, 0.69444, 0.0, 0.0, 0.27778]), ('"', [0.0, 0.69444, 0.0, 0.0, 0.5]),
    ('#', [0.19444, 0.69444, 0.0, 0.0, 0.83334]), ('$', [0.05556, 0.75, 0.0, 0.0, 0.5]), ('%', [0.05556, 0.75, 0.0, 0.0, 0.83334]),
    ('&', [0.0, 0.69444, 0.0, 0.0, 0.77778]), ('\'', [0.0, 0.69444, 0.0, 0.0, 0.27778]), ('(', [0.25, 0.75, 0.0, 0.0, 0.38889]),
    (')', [0.25, 0.75, 0.0, 0.0, 0.38889]), ('*', [0.0, 0.75, 0.0, 0.0, 0.5]), ('+', [0.08333, 0.58333, 0.0, 0.0, 0.77778]),
    (',', [0.19444, 0.10556, 0.0, 0.0, 0.27778]), ('-', [0.0, 0.43056, 0.0, 0.0, 0.33333]), ('.', [0.0, 0.10556, 0.0, 0.0, 0.27778]),
    ('/', [0.25, 0.75, 0.0, 0.0, 0.5]), ('0', [0.0, 0.64444, 0.0, 0.0, 0.5]), ('1', [0.0, 0.64444, 0.0, 0.0, 0.5]),
    ('2', [0.0, 0.64444, 0.0, 0.0, 0.5]), ('3', [0.0, 0.64444, 0.0, 0.0, 0.5]), ('4', [0.0, 0.64444, 0.0, 0.0, 0.5]),
    ('5', [0.0, 0.64444, 0.0, 0.0, 0.5]), ('6', [0.0, 0.64444, 0.0, 0.0, 0.5]), ('7', [0.0, 0.64444, 0.0, 0.0, 0.5]),
    ('8', [0.0, 0.64444, 0.0, 0.0, 0.5]), ('9', [0.0, 0.64444, 0.0, 0.0, 0.5]), (':', [0.0, 0.43056, 0.0, 0.0, 0.27778]),
    (';', [0.19444, 0.43056, 0.0, 0.0, 0.27778]), ('<', [0.0391, 0.5391, 0.0, 0.0, 0.77778]),
    ('=', [-0.13313, 0.36687, 0.0, 0.0, 0.77778]), ('>', [0.0391, 0.5391, 0.0, 0.0, 0.77778]), ('?', [0.0, 0.69444, 0.0, 0.0, 0.47222]),
    ('@', [0.0, 0.69444, 0.0, 0.0, 0.77778]), ('A', [0.0, 0.68333, 0.0, 0.0, 0.75]), ('B', [0.0, 0.68333, 0.0, 0.0, 0.70834]),
    ('C', [0.0, 0.68333, 0.0, 0.0, 0.72222]), ('D', [0.0, 0.68333, 0.0, 0.0, 0.76389]), ('E', [0.0, 0.68333, 0.0, 0.0, 0.68056]),
    ('F', [0.0, 0.68333, 0.0, 0.0, 0.65278]), ('G', [0.0, 0.68333, 0.0, 0.0, 0.78472]), ('H', [0.0, 0.68333, 0.0, 0.0, 0.75]),
    ('I', [0.0, 0.68333, 0.0, 0.0, 0.36111]), ('J', [0.0, 0.68333, 0.0, 0.0, 0.51389]), ('K', [0.0, 0.68333, 0.0, 0.0, 0.77778]),
    ('L', [0.0, 0.68333, 0.0, 0.0, 0.625]), ('M', [0.0, 0.68333, 0.0, 0.0, 0.91667]), ('N', [0.0, 0.68333, 0.0, 0.0, 0.75]),
    ('O', [0.0, 0.68333, 0.0, 0.0, 0.77778]), ('P', [0.0, 0.68333, 0.0, 0.0, 0.68056]), ('Q', [0.19444, 0.68333, 0.0, 0.0, 0.77778]),
    ('R', [0.0, 0.68333, 0.0, 0.0, 0.73611]), ('S', [0.0, 0.68333, 0.0, 0.0, 0.55556]), ('T', [0.0, 0.68333, 0.0, 0.0, 0.72222]),
    ('U', [0.0, 0.68333, 0.0, 0.0, 0.75]), ('V', [0.0, 0.68333, 0.01389, 0.0, 0.75]), ('W', [0.0, 0.68333, 0.01389, 0.0, 1.02778]),
    ('X', [0.0, 0.68333, 0.0, 0.0, 0.75]), ('Y', [0.0, 0.68333, 0.025, 0.0, 0.75]), ('Z', [0.0, 0.68333, 0.0, 0.0, 0.61111]),
    ('[', [0.25, 0.75, 0.0, 0.0, 0.27778]), ('\\', [0.25, 0.75, 0.0, 0.0, 0.5]), (']', [0.25, 0.75, 0.0, 0.0, 0.27778]),
    ('^', [0.0, 0.69444, 0.0, 0.0, 0.5]), ('_', [0.31, 0.12056, 0.02778, 0.0, 0.5]), ('a', [0.0, 0.43056, 0.0, 0.0, 0.5]),
    ('b', [0.0, 0.69444, 0.0, 0.0, 0.55556]), ('c', [0.0, 0.43056, 0.0, 0.0, 0.44445]), ('d', [0.0, 0.69444, 0.0, 0.0, 0.55556]),
    ('e', [0.0, 0.43056, 0.0, 0.0, 0.44445]), ('f', [0.0, 0.69444, 0.07778, 0.0, 0.30556]), ('g', [0.19444, 0.43056, 0.01389, 0.0, 0.5]),
    ('h', [0.0, 0.69444, 0.0, 0.0, 0.55556]), ('i', [0.0, 0.66786, 0.0, 0.0, 0.27778]), ('j', [0.19444, 0.66786, 0.0, 0.0, 0.30556]),
    ('k', [0.0, 0.69444, 0.0, 0.0, 0.52778]), ('l', [0.0, 0.69444, 0.0, 0.0, 0.27778]), ('m', [0.0, 0.43056, 0.0, 0.0, 0.83334]),
    ('n', [0.0, 0.43056, 0.0, 0.0, 0.55556]), ('o', [0.0, 0.43056, 0.0, 0.0, 0.5]), ('p', [0.19444, 0.43056, 0.0, 0.0, 0.55556]),
    ('q', [0.19444, 0.43056, 0.0, 0.0, 0.52778]), ('r', [0.0, 0.43056, 0.0, 0.0, 0.39167]), ('s', [0.0, 0.43056, 0.0, 0.0, 0.39445]),
    ('t', [0.0, 0.61508, 0.0, 0.0, 0.38889]), ('u', [0.0, 0.43056, 0.0, 0.0, 0.55556]), ('v', [0.0, 0.43056, 0.01389, 0.0, 0.52778]),
    ('w', [0.0, 0.43056, 0.01389, 0.0, 0.72222]), ('x', [0.0, 0.43056, 0.0, 0.0, 0.52778]),
    ('y', [0.19444, 0.43056, 0.01389, 0.0, 0.52778]), ('z', [0.0, 0.43056, 0.0, 0.0, 0.44445]), ('{', [0.25, 0.75, 0.0, 0.0, 0.5]),
    ('|', [0.25, 0.75, 0.0, 0.0, 0.27778]), ('}', [0.25, 0.75, 0.0, 0.0, 0.5]), ('~', [0.35, 0.31786, 0.0, 0.0, 0.5]),
    ('\u{a0}', [0.0, 0.0, 0.0, 0.0, 0.25]), ('\u{ac}', [0.0, 0.43056, 0.0, 0.0, 0.66667]),
    ('\u{b1}', [0.08333, 0.58333, 0.0, 0.0, 0.77778]), ('\u{d7}', [0.08333, 0.58333, 0.0, 0.0, 0.77778]),
    ('\u{f7}', [0.08333, 0.58333, 0.0, 0.0, 0.77778]), ('\u{393}', [0.0, 0.68333, 0.0, 0.0, 0.625]),
    ('\u{394}', [0.0, 0.68333, 0.0, 0.0, 0.83334]), ('\u{398}', [0.0, 0.68333, 0.0, 0.0, 0.77778]),
    ('\u{39b}', [0.0, 0.68333, 0.0, 0.0, 0.69445]), ('\u{39e}', [0.0, 0.68333, 0.0, 0.0, 0.66667]),
    ('\u{3a0}', [0.0, 0.68333, 0.0, 0.0, 0.75]), ('\u{3a3}', [0.0, 0.68333, 0.0, 0.0, 0.72222]),
    ('\u{3a5}', [0.0, 0.68333, 0.0, 0.0, 0.77778]), ('\u{3a6}', [0.0, 0.68333, 0.0, 0.0, 0.72222]),
    ('\u{3a8}', [0.0, 0.68333, 0.0, 0.0, 0.77778]), ('\u{3a9}', [0.0, 0.68333, 0.0, 0.0, 0.72222]),
    ('\u{2026}', [0.0, 0.123, 0.0, 0.0, 1.172]), ('\u{2032}', [0.0, 0.55556, 0.0, 0.0, 0.275]),
    ('\u{2111}', [0.0, 0.69444, 0.0, 0.0, 0.72222]), ('\u{2113}', [0.0, 0.69444, 0.0, 0.11111, 0.41667]),
    ('\u{2118}', [0.19444, 0.43056, 0.0, 0.11111, 0.63646]), ('\u{211c}', [0.0, 0.69444, 0.0, 0.0, 0.72222]),
    ('\u{2135}', [0.0, 0.69444, 0.0, 0.0, 0.61111]), ('\u{2190}', [-0.13313, 0.36687, 0.0, 0.0, 1.0]),
    ('\u{2191}', [0.19444, 0.69444, 0.0, 0.0, 0.5]), ('\u{2192}', [-0.13313, 0.36687, 0.0, 0.0, 1.0]),
    ('\u{2193}', [0.19444, 0.69444, 0.0, 0.0, 0.5]), ('\u{2194}', [-0.13313, 0.36687, 0.0, 0.0, 1.0]),
    ('\u{21a6}', [0.011, 0.511, 0.0, 0.0, 1.0]), ('\u{21d0}', [-0.13313, 0.36687, 0.0, 0.0, 1.0]),
    ('\u{21d2}', [-0.13313, 0.36687, 0.0, 0.0, 1.0]), ('\u{21d4}', [-0.13313, 0.36687, 0.0, 0.0, 1.0]),
    ('\u{2200}', [0.0, 0.69444, 0.0, 0.0, 0.55556]), ('\u{2202}', [0.0, 0.69444, 0.05556, 0.08334, 0.5309]),
    ('\u{2203}', [0.0, 0.69444, 0.0, 0.0, 0.55556]), ('\u{2205}', [0.05556, 0.75, 0.0, 0.0, 0.5]),
    ('\u{2207}', [0.0, 0.68333, 0.0, 0.0, 0.83334]), ('\u{2208}', [0.0391, 0.5391, 0.0, 0.0, 0.66667]),
    ('\u{220b}', [0.0391, 0.5391, 0.0, 0.0, 0.66667]), ('\u{2212}', [0.08333, 0.58333, 0.0, 0.0, 0.77778]),
    ('\u{2213}', [0.08333, 0.58333, 0.0, 0.0, 0.77778]), ('\u{2216}', [0.25, 0.75, 0.0, 0.0, 0.5]),
    ('\u{2217}', [-0.03472, 0.46528, 0.0, 0.0, 0.5]), ('\u{2218}', [-0.05555, 0.44445, 0.0, 0.0, 0.5]),
    ('\u{2219}', [-0.05555, 0.44445, 0.0, 0.0, 0.5]), ('\u{221a}', [0.2, 0.8, 0.0, 0.0, 0.83334]),
    ('\u{221d}', [0.0, 0.43056, 0.0, 0.0, 0.77778]), ('\u{221e}', [0.0, 0.43056, 0.0, 0.0, 1.0]),
    ('\u{2220}', [0.0, 0.69224, 0.0, 0.0, 0.72222]), ('\u{2223}', [0.25, 0.75, 0.0, 0.0, 0.27778]),
    ('\u{2225}', [0.25, 0.75, 0.0, 0.0, 0.5]), ('\u{2227}', [0.0, 0.55556, 0.0, 0.0, 0.66667]),
    ('\u{2228}', [0.0, 0.55556, 0.0, 0.0, 0.66667]), ('\u{2229}', [0.0, 0.55556, 0.0, 0.0, 0.66667]),
    ('\u{222a}', [0.0, 0.55556, 0.0, 0.0, 0.66667]), ('\u{223c}', [-0.13313, 0.36687, 0.0, 0.0, 0.77778]),
    ('\u{2243}', [-0.03625, 0.46375, 0.0, 0.0, 0.77778]), ('\u{2245}', [-0.022, 0.589, 0.0, 0.0, 0.778]),
    ('\u{2248}', [-0.01688, 0.48312, 0.0, 0.0, 0.77778]), ('\u{224d}', [-0.03625, 0.46375, 0.0, 0.0, 0.77778]),
    ('\u{2250}', [-0.133, 0.673, 0.0, 0.0, 0.778]), ('\u{2261}', [-0.03625, 0.46375, 0.0, 0.0, 0.77778]),
    ('\u{2264}', [0.13597, 0.63597, 0.0, 0.0, 0.77778]), ('\u{2265}', [0.13597, 0.63597, 0.0, 0.0, 0.77778]),
    ('\u{226a}', [0.0391, 0.5391, 0.0, 0.0, 1.0]), ('\u{226b}', [0.0391, 0.5391, 0.0, 0.0, 1.0]),
    ('\u{227a}', [0.0391, 0.5391, 0.0, 0.0, 0.77778]), ('\u{227b}', [0.0391, 0.5391, 0.0, 0.0, 0.77778]),
    ('\u{2282}', [0.0391, 0.5391, 0.0, 0.0, 0.77778]), ('\u{2283}', [0.0391, 0.5391, 0.0, 0.0, 0.77778]),
    ('\u{2286}', [0.13597, 0.63597, 0.0, 0.0, 0.77778]), ('\u{2287}', [0.13597, 0.63597, 0.0, 0.0, 0.77778]),
    ('\u{2295}', [0.08333, 0.58333, 0.0, 0.0, 0.77778]), ('\u{2296}', [0.08333, 0.58333, 0.0, 0.0, 0.77778]),
    ('\u{2297}', [0.08333, 0.58333, 0.0, 0.0, 0.77778]), ('\u{2299}', [0.08333, 0.58333, 0.0, 0.0, 0.77778]),
    ('\u{22a2}', [0.0, 0.69444, 0.0, 0.0, 0.61111]), ('\u{22a3}', [0.0, 0.69444, 0.0, 0.0, 0.61111]),
    ('\u{22a4}', [0.0, 0.69444, 0.0, 0.0, 0.77778]), ('\u{22a5}', [0.0, 0.69444, 0.0, 0.0, 0.77778]),
    ('\u{22a8}', [0.249, 0.75, 0.0, 0.0, 0.867]), ('\u{22c5}', [-0.05555, 0.44445, 0.0, 0.0, 0.27778]),
    ('\u{22c6}', [-0.03472, 0.46528, 0.0, 0.0, 0.5]), ('\u{22ef}', [-0.19, 0.313, 0.0, 0.0, 1.172]),
    ('\u{22f1}', [-0.1, 0.823, 0.0, 0.0, 1.282]), ('\u{2308}', [0.25, 0.75, 0.0, 0.0, 0.44445]),
    ('\u{2309}', [0.25, 0.75, 0.0, 0.0, 0.44445]), ('\u{230a}', [0.25, 0.75, 0.0, 0.0, 0.44445]),
    ('\u{230b}', [0.25, 0.75, 0.0, 0.0, 0.44445]), ('\u{25b3}', [0.19444, 0.69444, 0.0, 0.0, 0.88889]),
    ('\u{27e8}', [0.25, 0.75, 0.0, 0.0, 0.38889]), ('\u{27e9}', [0.25, 0.75, 0.0, 0.0, 0.38889]),
    ('\u{27f6}', [0.011, 0.511, 0.0, 0.0, 1.638]), ('\u{27f7}', [0.011, 0.511, 0.0, 0.0, 1.859]),
    ('\u{27f9}', [0.024, 0.525, 0.0, 0.0, 1.638]), ('\u{27fa}', [0.024, 0.525, 0.0, 0.0, 1.858]),
    ('\u{e020}', [0.19444, 0.69444, 0.0, 0.0, 0.0]),
];

/// `fontMetricsData.js`'s Math-Italic: depth, height, italic correction, skew and width, in em.
#[rustfmt::skip]
const MATH_ITALIC: &[(char, [f32; 5])] = &[
    ('A', [0.0, 0.68333, 0.0, 0.13889, 0.75]), ('B', [0.0, 0.68333, 0.05017, 0.08334, 0.75851]),
    ('C', [0.0, 0.68333, 0.07153, 0.08334, 0.71472]), ('D', [0.0, 0.68333, 0.02778, 0.05556, 0.82792]),
    ('E', [0.0, 0.68333, 0.05764, 0.08334, 0.7382]), ('F', [0.0, 0.68333, 0.13889, 0.08334, 0.64306]),
    ('G', [0.0, 0.68333, 0.0, 0.08334, 0.78625]), ('H', [0.0, 0.68333, 0.08125, 0.05556, 0.83125]),
    ('I', [0.0, 0.68333, 0.07847, 0.11111, 0.43958]), ('J', [0.0, 0.68333, 0.09618, 0.16667, 0.55451]),
    ('K', [0.0, 0.68333, 0.07153, 0.05556, 0.84931]), ('L', [0.0, 0.68333, 0.0, 0.02778, 0.68056]),
    ('M', [0.0, 0.68333, 0.10903, 0.08334, 0.97014]), ('N', [0.0, 0.68333, 0.10903, 0.08334, 0.80347]),
    ('O', [0.0, 0.68333, 0.02778, 0.08334, 0.76278]), ('P', [0.0, 0.68333, 0.13889, 0.08334, 0.64201]),
    ('Q', [0.19444, 0.68333, 0.0, 0.08334, 0.79056]), ('R', [0.0, 0.68333, 0.00773, 0.08334, 0.75929]),
    ('S', [0.0, 0.68333, 0.05764, 0.08334, 0.6132]), ('T', [0.0, 0.68333, 0.13889, 0.08334, 0.58438]),
    ('U', [0.0, 0.68333, 0.10903, 0.02778, 0.68278]), ('V', [0.0, 0.68333, 0.22222, 0.0, 0.58333]),
    ('W', [0.0, 0.68333, 0.13889, 0.0, 0.94445]), ('X', [0.0, 0.68333, 0.07847, 0.08334, 0.82847]),
    ('Y', [0.0, 0.68333, 0.22222, 0.0, 0.58056]), ('Z', [0.0, 0.68333, 0.07153, 0.08334, 0.68264]),
    ('a', [0.0, 0.43056, 0.0, 0.0, 0.52859]), ('b', [0.0, 0.69444, 0.0, 0.0, 0.42917]), ('c', [0.0, 0.43056, 0.0, 0.05556, 0.43276]),
    ('d', [0.0, 0.69444, 0.0, 0.16667, 0.52049]), ('e', [0.0, 0.43056, 0.0, 0.05556, 0.46563]),
    ('f', [0.19444, 0.69444, 0.10764, 0.16667, 0.48959]), ('g', [0.19444, 0.43056, 0.03588, 0.02778, 0.47697]),
    ('h', [0.0, 0.69444, 0.0, 0.0, 0.57616]), ('i', [0.0, 0.65952, 0.0, 0.0, 0.34451]), ('j', [0.19444, 0.65952, 0.05724, 0.0, 0.41181]),
    ('k', [0.0, 0.69444, 0.03148, 0.0, 0.5206]), ('l', [0.0, 0.69444, 0.01968, 0.08334, 0.29838]),
    ('m', [0.0, 0.43056, 0.0, 0.0, 0.87801]), ('n', [0.0, 0.43056, 0.0, 0.0, 0.60023]), ('o', [0.0, 0.43056, 0.0, 0.05556, 0.48472]),
    ('p', [0.19444, 0.43056, 0.0, 0.08334, 0.50313]), ('q', [0.19444, 0.43056, 0.03588, 0.08334, 0.44641]),
    ('r', [0.0, 0.43056, 0.02778, 0.05556, 0.45116]), ('s', [0.0, 0.43056, 0.0, 0.05556, 0.46875]),
    ('t', [0.0, 0.61508, 0.0, 0.08334, 0.36111]), ('u', [0.0, 0.43056, 0.0, 0.02778, 0.57246]),
    ('v', [0.0, 0.43056, 0.03588, 0.02778, 0.48472]), ('w', [0.0, 0.43056, 0.02691, 0.08334, 0.71592]),
    ('x', [0.0, 0.43056, 0.0, 0.02778, 0.57153]), ('y', [0.19444, 0.43056, 0.03588, 0.05556, 0.49028]),
    ('z', [0.0, 0.43056, 0.04398, 0.05556, 0.46505]), ('\u{3b1}', [0.0, 0.43056, 0.0037, 0.02778, 0.6397]),
    ('\u{3b2}', [0.19444, 0.69444, 0.05278, 0.08334, 0.56563]), ('\u{3b3}', [0.19444, 0.43056, 0.05556, 0.0, 0.51773]),
    ('\u{3b4}', [0.0, 0.69444, 0.03785, 0.05556, 0.44444]), ('\u{3b5}', [0.0, 0.43056, 0.0, 0.08334, 0.46632]),
    ('\u{3b6}', [0.19444, 0.69444, 0.07378, 0.08334, 0.4375]), ('\u{3b7}', [0.19444, 0.43056, 0.03588, 0.05556, 0.49653]),
    ('\u{3b8}', [0.0, 0.69444, 0.02778, 0.08334, 0.46944]), ('\u{3b9}', [0.0, 0.43056, 0.0, 0.05556, 0.35394]),
    ('\u{3ba}', [0.0, 0.43056, 0.0, 0.0, 0.57616]), ('\u{3bb}', [0.0, 0.69444, 0.0, 0.0, 0.58334]),
    ('\u{3bc}', [0.19444, 0.43056, 0.0, 0.02778, 0.60255]), ('\u{3bd}', [0.0, 0.43056, 0.06366, 0.02778, 0.49398]),
    ('\u{3be}', [0.19444, 0.69444, 0.04601, 0.11111, 0.4375]), ('\u{3c0}', [0.0, 0.43056, 0.03588, 0.0, 0.57003]),
    ('\u{3c1}', [0.19444, 0.43056, 0.0, 0.08334, 0.51702]), ('\u{3c2}', [0.09722, 0.43056, 0.07986, 0.08334, 0.36285]),
    ('\u{3c3}', [0.0, 0.43056, 0.03588, 0.0, 0.57141]), ('\u{3c4}', [0.0, 0.43056, 0.1132, 0.02778, 0.43715]),
    ('\u{3c5}', [0.0, 0.43056, 0.03588, 0.02778, 0.54028]), ('\u{3c6}', [0.19444, 0.43056, 0.0, 0.08334, 0.65417]),
    ('\u{3c7}', [0.19444, 0.43056, 0.0, 0.05556, 0.62569]), ('\u{3c8}', [0.19444, 0.69444, 0.03588, 0.11111, 0.65139]),
    ('\u{3c9}', [0.0, 0.43056, 0.03588, 0.0, 0.62245]), ('\u{3d1}', [0.0, 0.69444, 0.0, 0.08334, 0.59144]),
    ('\u{3d5}', [0.19444, 0.69444, 0.0, 0.08334, 0.59583]), ('\u{3d6}', [0.0, 0.43056, 0.02778, 0.0, 0.82813]),
    ('\u{3f1}', [0.19444, 0.43056, 0.0, 0.08334, 0.51702]), ('\u{3f5}', [0.0, 0.43056, 0.0, 0.05556, 0.4059]),
];

/// `fontMetricsData.js`'s Main-Bold: depth, height, italic correction, skew and width, in em.
#[rustfmt::skip]
const MAIN_BOLD: &[(char, [f32; 5])] = &[
    (' ', [0.0, 0.0, 0.0, 0.0, 0.25]), ('!', [0.0, 0.69444, 0.0, 0.0, 0.35]), ('"', [0.0, 0.69444, 0.0, 0.0, 0.60278]),
    ('#', [0.19444, 0.69444, 0.0, 0.0, 0.95833]), ('$', [0.05556, 0.75, 0.0, 0.0, 0.575]), ('%', [0.05556, 0.75, 0.0, 0.0, 0.95833]),
    ('&', [0.0, 0.69444, 0.0, 0.0, 0.89444]), ('\'', [0.0, 0.69444, 0.0, 0.0, 0.31944]), ('(', [0.25, 0.75, 0.0, 0.0, 0.44722]),
    (')', [0.25, 0.75, 0.0, 0.0, 0.44722]), ('*', [0.0, 0.75, 0.0, 0.0, 0.575]), ('+', [0.13333, 0.63333, 0.0, 0.0, 0.89444]),
    (',', [0.19444, 0.15556, 0.0, 0.0, 0.31944]), ('-', [0.0, 0.44444, 0.0, 0.0, 0.38333]), ('.', [0.0, 0.15556, 0.0, 0.0, 0.31944]),
    ('/', [0.25, 0.75, 0.0, 0.0, 0.575]), ('0', [0.0, 0.64444, 0.0, 0.0, 0.575]), ('1', [0.0, 0.64444, 0.0, 0.0, 0.575]),
    ('2', [0.0, 0.64444, 0.0, 0.0, 0.575]), ('3', [0.0, 0.64444, 0.0, 0.0, 0.575]), ('4', [0.0, 0.64444, 0.0, 0.0, 0.575]),
    ('5', [0.0, 0.64444, 0.0, 0.0, 0.575]), ('6', [0.0, 0.64444, 0.0, 0.0, 0.575]), ('7', [0.0, 0.64444, 0.0, 0.0, 0.575]),
    ('8', [0.0, 0.64444, 0.0, 0.0, 0.575]), ('9', [0.0, 0.64444, 0.0, 0.0, 0.575]), (':', [0.0, 0.44444, 0.0, 0.0, 0.31944]),
    (';', [0.19444, 0.44444, 0.0, 0.0, 0.31944]), ('<', [0.08556, 0.58556, 0.0, 0.0, 0.89444]),
    ('=', [-0.10889, 0.39111, 0.0, 0.0, 0.89444]), ('>', [0.08556, 0.58556, 0.0, 0.0, 0.89444]), ('?', [0.0, 0.69444, 0.0, 0.0, 0.54305]),
    ('@', [0.0, 0.69444, 0.0, 0.0, 0.89444]), ('A', [0.0, 0.68611, 0.0, 0.0, 0.86944]), ('B', [0.0, 0.68611, 0.0, 0.0, 0.81805]),
    ('C', [0.0, 0.68611, 0.0, 0.0, 0.83055]), ('D', [0.0, 0.68611, 0.0, 0.0, 0.88194]), ('E', [0.0, 0.68611, 0.0, 0.0, 0.75555]),
    ('F', [0.0, 0.68611, 0.0, 0.0, 0.72361]), ('G', [0.0, 0.68611, 0.0, 0.0, 0.90416]), ('H', [0.0, 0.68611, 0.0, 0.0, 0.9]),
    ('I', [0.0, 0.68611, 0.0, 0.0, 0.43611]), ('J', [0.0, 0.68611, 0.0, 0.0, 0.59444]), ('K', [0.0, 0.68611, 0.0, 0.0, 0.90138]),
    ('L', [0.0, 0.68611, 0.0, 0.0, 0.69166]), ('M', [0.0, 0.68611, 0.0, 0.0, 1.09166]), ('N', [0.0, 0.68611, 0.0, 0.0, 0.9]),
    ('O', [0.0, 0.68611, 0.0, 0.0, 0.86388]), ('P', [0.0, 0.68611, 0.0, 0.0, 0.78611]), ('Q', [0.19444, 0.68611, 0.0, 0.0, 0.86388]),
    ('R', [0.0, 0.68611, 0.0, 0.0, 0.8625]), ('S', [0.0, 0.68611, 0.0, 0.0, 0.63889]), ('T', [0.0, 0.68611, 0.0, 0.0, 0.8]),
    ('U', [0.0, 0.68611, 0.0, 0.0, 0.88472]), ('V', [0.0, 0.68611, 0.01597, 0.0, 0.86944]), ('W', [0.0, 0.68611, 0.01597, 0.0, 1.18888]),
    ('X', [0.0, 0.68611, 0.0, 0.0, 0.86944]), ('Y', [0.0, 0.68611, 0.02875, 0.0, 0.86944]), ('Z', [0.0, 0.68611, 0.0, 0.0, 0.70277]),
    ('[', [0.25, 0.75, 0.0, 0.0, 0.31944]), ('\\', [0.25, 0.75, 0.0, 0.0, 0.575]), (']', [0.25, 0.75, 0.0, 0.0, 0.31944]),
    ('^', [0.0, 0.69444, 0.0, 0.0, 0.575]), ('_', [0.31, 0.13444, 0.03194, 0.0, 0.575]), ('a', [0.0, 0.44444, 0.0, 0.0, 0.55902]),
    ('b', [0.0, 0.69444, 0.0, 0.0, 0.63889]), ('c', [0.0, 0.44444, 0.0, 0.0, 0.51111]), ('d', [0.0, 0.69444, 0.0, 0.0, 0.63889]),
    ('e', [0.0, 0.44444, 0.0, 0.0, 0.52708]), ('f', [0.0, 0.69444, 0.10903, 0.0, 0.35139]),
    ('g', [0.19444, 0.44444, 0.01597, 0.0, 0.575]), ('h', [0.0, 0.69444, 0.0, 0.0, 0.63889]), ('i', [0.0, 0.69444, 0.0, 0.0, 0.31944]),
    ('j', [0.19444, 0.69444, 0.0, 0.0, 0.35139]), ('k', [0.0, 0.69444, 0.0, 0.0, 0.60694]), ('l', [0.0, 0.69444, 0.0, 0.0, 0.31944]),
    ('m', [0.0, 0.44444, 0.0, 0.0, 0.95833]), ('n', [0.0, 0.44444, 0.0, 0.0, 0.63889]), ('o', [0.0, 0.44444, 0.0, 0.0, 0.575]),
    ('p', [0.19444, 0.44444, 0.0, 0.0, 0.63889]), ('q', [0.19444, 0.44444, 0.0, 0.0, 0.60694]), ('r', [0.0, 0.44444, 0.0, 0.0, 0.47361]),
    ('s', [0.0, 0.44444, 0.0, 0.0, 0.45361]), ('t', [0.0, 0.63492, 0.0, 0.0, 0.44722]), ('u', [0.0, 0.44444, 0.0, 0.0, 0.63889]),
    ('v', [0.0, 0.44444, 0.01597, 0.0, 0.60694]), ('w', [0.0, 0.44444, 0.01597, 0.0, 0.83055]), ('x', [0.0, 0.44444, 0.0, 0.0, 0.60694]),
    ('y', [0.19444, 0.44444, 0.01597, 0.0, 0.60694]), ('z', [0.0, 0.44444, 0.0, 0.0, 0.51111]), ('{', [0.25, 0.75, 0.0, 0.0, 0.575]),
    ('|', [0.25, 0.75, 0.0, 0.0, 0.31944]), ('}', [0.25, 0.75, 0.0, 0.0, 0.575]), ('~', [0.35, 0.34444, 0.0, 0.0, 0.575]),
];

/// `fontMetricsData.js`'s Size1-Regular: depth, height, italic correction, skew and width, in em.
#[rustfmt::skip]
const SIZE1: &[(char, [f32; 5])] = &[
    ('(', [0.35001, 0.85, 0.0, 0.0, 0.45834]), (')', [0.35001, 0.85, 0.0, 0.0, 0.45834]), ('/', [0.35001, 0.85, 0.0, 0.0, 0.57778]),
    ('[', [0.35001, 0.85, 0.0, 0.0, 0.41667]), ('\\', [0.35001, 0.85, 0.0, 0.0, 0.57778]), (']', [0.35001, 0.85, 0.0, 0.0, 0.41667]),
    ('{', [0.35001, 0.85, 0.0, 0.0, 0.58334]), ('}', [0.35001, 0.85, 0.0, 0.0, 0.58334]),
    ('\u{220f}', [0.25001, 0.75, 0.0, 0.0, 0.94445]), ('\u{2210}', [0.25001, 0.75, 0.0, 0.0, 0.94445]),
    ('\u{2211}', [0.25001, 0.75, 0.0, 0.0, 1.05556]), ('\u{221a}', [0.35001, 0.85, 0.0, 0.0, 1.0]),
    ('\u{2223}', [-0.00599, 0.606, 0.0, 0.0, 0.33333]), ('\u{2225}', [-0.00599, 0.606, 0.0, 0.0, 0.55556]),
    ('\u{222b}', [0.30612, 0.805, 0.19445, 0.0, 0.47222]), ('\u{222c}', [0.306, 0.805, 0.19445, 0.0, 0.47222]),
    ('\u{222d}', [0.306, 0.805, 0.19445, 0.0, 0.47222]), ('\u{222e}', [0.30612, 0.805, 0.19445, 0.0, 0.47222]),
    ('\u{22c2}', [0.25001, 0.75, 0.0, 0.0, 0.83334]), ('\u{22c3}', [0.25001, 0.75, 0.0, 0.0, 0.83334]),
    ('\u{2308}', [0.35001, 0.85, 0.0, 0.0, 0.47222]), ('\u{2309}', [0.35001, 0.85, 0.0, 0.0, 0.47222]),
    ('\u{230a}', [0.35001, 0.85, 0.0, 0.0, 0.47222]), ('\u{230b}', [0.35001, 0.85, 0.0, 0.0, 0.47222]),
    ('\u{27e8}', [0.35001, 0.85, 0.0, 0.0, 0.47222]), ('\u{27e9}', [0.35001, 0.85, 0.0, 0.0, 0.47222]),
    ('\u{2a00}', [0.25001, 0.75, 0.0, 0.0, 1.11111]), ('\u{2a01}', [0.25001, 0.75, 0.0, 0.0, 1.11111]),
    ('\u{2a02}', [0.25001, 0.75, 0.0, 0.0, 1.11111]),
];

/// `fontMetricsData.js`'s Size2-Regular: depth, height, italic correction, skew and width, in em.
#[rustfmt::skip]
const SIZE2: &[(char, [f32; 5])] = &[
    ('(', [0.65002, 1.15, 0.0, 0.0, 0.59722]), (')', [0.65002, 1.15, 0.0, 0.0, 0.59722]), ('/', [0.65002, 1.15, 0.0, 0.0, 0.81111]),
    ('[', [0.65002, 1.15, 0.0, 0.0, 0.47222]), ('\\', [0.65002, 1.15, 0.0, 0.0, 0.81111]), (']', [0.65002, 1.15, 0.0, 0.0, 0.47222]),
    ('{', [0.65002, 1.15, 0.0, 0.0, 0.66667]), ('}', [0.65002, 1.15, 0.0, 0.0, 0.66667]),
    ('\u{220f}', [0.55001, 1.05, 0.0, 0.0, 1.27778]), ('\u{2210}', [0.55001, 1.05, 0.0, 0.0, 1.27778]),
    ('\u{2211}', [0.55001, 1.05, 0.0, 0.0, 1.44445]), ('\u{221a}', [0.65002, 1.15, 0.0, 0.0, 1.0]),
    ('\u{222b}', [0.86225, 1.36, 0.44445, 0.0, 0.55556]), ('\u{222c}', [0.862, 1.36, 0.44445, 0.0, 0.55556]),
    ('\u{222d}', [0.862, 1.36, 0.44445, 0.0, 0.55556]), ('\u{222e}', [0.86225, 1.36, 0.44445, 0.0, 0.55556]),
    ('\u{22c2}', [0.55001, 1.05, 0.0, 0.0, 1.11111]), ('\u{22c3}', [0.55001, 1.05, 0.0, 0.0, 1.11111]),
    ('\u{2308}', [0.65002, 1.15, 0.0, 0.0, 0.52778]), ('\u{2309}', [0.65002, 1.15, 0.0, 0.0, 0.52778]),
    ('\u{230a}', [0.65002, 1.15, 0.0, 0.0, 0.52778]), ('\u{230b}', [0.65002, 1.15, 0.0, 0.0, 0.52778]),
    ('\u{27e8}', [0.65002, 1.15, 0.0, 0.0, 0.61111]), ('\u{27e9}', [0.65002, 1.15, 0.0, 0.0, 0.61111]),
    ('\u{2a00}', [0.55001, 1.05, 0.0, 0.0, 1.51112]), ('\u{2a01}', [0.55001, 1.05, 0.0, 0.0, 1.51112]),
    ('\u{2a02}', [0.55001, 1.05, 0.0, 0.0, 1.51112]),
];

/// `fontMetricsData.js`'s Size3-Regular: depth, height, italic correction, skew and width, in em.
#[rustfmt::skip]
const SIZE3: &[(char, [f32; 5])] = &[
    ('(', [0.95003, 1.45, 0.0, 0.0, 0.73611]), (')', [0.95003, 1.45, 0.0, 0.0, 0.73611]), ('/', [0.95003, 1.45, 0.0, 0.0, 1.04445]),
    ('[', [0.95003, 1.45, 0.0, 0.0, 0.52778]), ('\\', [0.95003, 1.45, 0.0, 0.0, 1.04445]), (']', [0.95003, 1.45, 0.0, 0.0, 0.52778]),
    ('{', [0.95003, 1.45, 0.0, 0.0, 0.75]), ('}', [0.95003, 1.45, 0.0, 0.0, 0.75]), ('\u{221a}', [0.95003, 1.45, 0.0, 0.0, 1.0]),
    ('\u{2308}', [0.95003, 1.45, 0.0, 0.0, 0.58334]), ('\u{2309}', [0.95003, 1.45, 0.0, 0.0, 0.58334]),
    ('\u{230a}', [0.95003, 1.45, 0.0, 0.0, 0.58334]), ('\u{230b}', [0.95003, 1.45, 0.0, 0.0, 0.58334]),
    ('\u{27e8}', [0.95003, 1.45, 0.0, 0.0, 0.75]), ('\u{27e9}', [0.95003, 1.45, 0.0, 0.0, 0.75]),
];

/// `fontMetricsData.js`'s Size4-Regular: depth, height, italic correction, skew and width, in em.
#[rustfmt::skip]
const SIZE4: &[(char, [f32; 5])] = &[
    ('(', [1.25003, 1.75, 0.0, 0.0, 0.79167]), (')', [1.25003, 1.75, 0.0, 0.0, 0.79167]), ('/', [1.25003, 1.75, 0.0, 0.0, 1.27778]),
    ('[', [1.25003, 1.75, 0.0, 0.0, 0.58334]), ('\\', [1.25003, 1.75, 0.0, 0.0, 1.27778]), (']', [1.25003, 1.75, 0.0, 0.0, 0.58334]),
    ('{', [1.25003, 1.75, 0.0, 0.0, 0.80556]), ('}', [1.25003, 1.75, 0.0, 0.0, 0.80556]), ('\u{221a}', [1.25003, 1.75, 0.0, 0.0, 1.0]),
    ('\u{2308}', [1.25003, 1.75, 0.0, 0.0, 0.63889]), ('\u{2309}', [1.25003, 1.75, 0.0, 0.0, 0.63889]),
    ('\u{230a}', [1.25003, 1.75, 0.0, 0.0, 0.63889]), ('\u{230b}', [1.25003, 1.75, 0.0, 0.0, 0.63889]),
    ('\u{27e8}', [1.25003, 1.75, 0.0, 0.0, 0.80556]), ('\u{27e9}', [1.25003, 1.75, 0.0, 0.0, 0.80556]),
];

#[cfg(test)]
mod tests {
    use gpui_kit::base::text::TextViewState;
    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, Context, Entity, TestAppContext, Window};

    use super::{parse, reading, tex_text, typeset, Atom, Boxed, Class, Face, Fonts, Ink, Ttf, FONTS};
    use crate::ui::Side;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn glyphs(boxed: &Boxed) -> Vec<(char, f32, f32, Face, f32)> {
        let glyph = |ink: &Ink| match *ink {
            Ink::Glyph { x, y, ch, face, scale } => Some((ch, x, y, face, scale)),
            Ink::Rule { .. } => None,
        };
        boxed.ink.iter().filter_map(glyph).collect()
    }

    fn rules(boxed: &Boxed) -> Vec<(f32, f32, f32, f32)> {
        let rule = |ink: &Ink| match *ink {
            Ink::Rule { x, y, width, height } => Some((x, y, width, height)),
            Ink::Glyph { .. } => None,
        };
        boxed.ink.iter().filter_map(rule).collect()
    }

    fn width(face: Face, ch: char) -> f32 {
        face.metrics(ch).expect("a glyph KaTeX measures").width
    }

    #[test]
    fn dollars_read_as_pandoc_reads_them() {
        assert_eq!(reading("$x$", None), Some(false));
        assert_eq!(reading("$$\\int x$$", None), Some(true));
        assert_eq!(reading("$ x$", None), None, "an opening dollar before a space");
        assert_eq!(reading("$x $", None), None, "a closing dollar after a space");
        assert_eq!(reading("$x$", Some('2')), None, "a closing dollar before a digit");
        assert_eq!(reading("$x$", Some('.')), Some(false));
    }

    #[test]
    fn tex_parses_into_atoms_with_katex_classes_and_faces() {
        let sym = |ch, class, face| Atom::Sym { ch, class, face, text: false };
        assert_eq!(
            parse("e^{i\\pi}+1=0"),
            Ok(vec![
                Atom::Scripts {
                    base: Box::new(sym('e', Class::Ord, Face::MathItalic)),
                    sup: Some(vec![sym('i', Class::Ord, Face::MathItalic), sym('\u{3c0}', Class::Ord, Face::MathItalic)]),
                    sub: None,
                },
                sym('+', Class::Bin, Face::Main),
                sym('1', Class::Ord, Face::Main),
                sym('=', Class::Rel, Face::Main),
                sym('0', Class::Ord, Face::Main),
            ])
        );
        let Ok(atoms) = parse("\\int\\limits_0^1") else { panic!("\\limits parses") };
        assert!(matches!(&atoms[..], [Atom::Scripts { base, .. }] if matches!(**base, Atom::Op { limits: true, .. })));
        let Ok(atoms) = parse("f'(x)") else { panic!("a prime parses") };
        assert!(matches!(&atoms[0], Atom::Scripts { sup: Some(sup), .. } if sup == &vec![sym('\u{2032}', Class::Ord, Face::Main)]));
        let text = |ch| Atom::Sym { ch, class: Class::Ord, face: Face::Main, text: true };
        assert_eq!(parse("\\text{a  b}"), Ok(vec![Atom::Group(Class::Ord, vec![text('a'), text('\u{a0}'), text('b')])]));
        assert_eq!(parse("\\mathbf{x}"), Ok(vec![Atom::Group(Class::Ord, vec![sym('x', Class::Ord, Face::MainBold)])]));
        assert_eq!(
            parse("\\neq"),
            Ok(vec![Atom::Group(Class::Rel, vec![sym('\u{e020}', Class::Rel, Face::Main), sym('=', Class::Rel, Face::Main)])])
        );
    }

    #[test]
    fn what_it_does_not_set_is_refused_by_name() {
        assert_eq!(parse("\\begin{pmatrix}"), Err("\\begin".into()));
        assert_eq!(parse("x^1^2"), Err("a double superscript".into()));
        assert_eq!(parse("\\sqrt[3]{x}"), Err("\\sqrt[".into()));
        assert_eq!(parse("{x"), Err("an unclosed group".into()));
        assert_eq!(parse("\\left( x"), Err("an unclosed group".into()));
        assert!(typeset("\\mathbb{R}", false).is_none());
    }

    #[test]
    fn a_chain_of_commands_without_braces_stops_at_the_nesting_cap() {
        assert!(parse(&format!("{} x", "\\sqrt".repeat(60))).is_ok());
        assert_eq!(parse(&format!("{} x", "\\sqrt".repeat(100_000))), Err("groups nested too deep".into()));
        assert_eq!(parse(&"\\frac".repeat(100_000)), Err("groups nested too deep".into()));
    }

    #[test]
    fn a_superscript_rises_by_sup2_after_its_letter() {
        let boxed = typeset("x^2", false).expect("x^2 sets");
        let x = width(Face::MathItalic, 'x');
        let marks = glyphs(&boxed);
        assert_eq!(marks[1].0, '2');
        // KaTeX sets x^2's script at `top: -3.063em` over a 2.7em strut: 0.363 up.
        assert!(close(marks[1].1, x) && close(marks[1].2, -0.363) && close(marks[1].4, 0.7), "{marks:?}");
        assert!(close(boxed.width, x + 0.7 * 0.5 + 0.05), "{}", boxed.width);
        assert!(close(boxed.height, 0.363 + 0.7 * 0.64444), "{}", boxed.height);
    }

    #[test]
    fn binary_operators_take_medium_space_unless_they_open_the_list() {
        let (a, b, plus) = (width(Face::MathItalic, 'a'), width(Face::MathItalic, 'b'), width(Face::Main, '+'));
        let between = typeset("a+b", false).expect("a+b sets");
        assert!(close(between.width, a + plus + b + 2. * 4. / 18.), "{}", between.width);
        let leading = typeset("+a", false).expect("+a sets");
        assert!(close(leading.width, plus + a), "a leading + is ordinary: {}", leading.width);
        let relation = typeset("a=b", false).expect("a=b sets");
        assert!(close(relation.width, a + width(Face::Main, '=') + b + 2. * 5. / 18.), "{}", relation.width);
    }

    #[test]
    fn a_display_integral_is_the_size2_glyph_with_its_limits_beside_it() {
        let boxed = typeset("\\int_0^1 x^2\\,dx", true).expect("the integral sets");
        let marks = glyphs(&boxed);
        let (sign, one, zero) = (marks[0], marks[1], marks[2]);
        assert_eq!((sign.0, sign.3), ('\u{222b}', Face::Size2));
        // KaTeX: the upper limit at `top: -3.8129em`, the lower at `-1.7881em` and `margin-left: -0.4445em`, on a 2.7em strut.
        assert_eq!((one.0, zero.0), ('1', '0'));
        assert!(close(one.1, 0.55556 + 0.44445) && close(one.2, -1.1129), "{one:?}");
        assert!(close(zero.1, 0.55556) && close(zero.2, 0.91195), "{zero:?}");
        let x = marks[3];
        assert!(close(x.1, 0.55556 + 0.44445 + 0.4 + 3. / 18.), "a thin space after the operator: {x:?}");
    }

    #[test]
    fn display_limits_centre_above_and_below_the_sum() {
        let boxed = typeset("\\sum_{i=1}^{n}", true).expect("the sum sets");
        let marks = glyphs(&boxed);
        let sum = marks.iter().find(|m| m.0 == '\u{2211}').expect("the sum");
        let n = marks.iter().find(|m| m.0 == 'n').expect("the upper limit");
        let i = marks.iter().find(|m| m.0 == 'i').expect("the lower limit");
        assert_eq!(sum.3, Face::Size2);
        let sum_width = width(Face::Size2, '\u{2211}');
        assert!(close(n.1 + 0.7 * width(Face::MathItalic, 'n') / 2., sum.1 + sum_width / 2.), "n centred: {n:?} {sum:?}");
        let lower = 0.7 * (width(Face::MathItalic, 'i') + width(Face::Main, '=') + width(Face::Main, '1'));
        assert!(close(i.1 + lower / 2., sum.1 + sum_width / 2.), "i=1 centred: {i:?} {sum:?}");
        // bigOpSpacing3 above the glyph's top.
        assert!(close(n.2, sum.2 - 1.05 - 0.2), "{n:?} {sum:?}");
        assert!(n.2 < sum.2 && i.2 > sum.2);
    }

    #[test]
    fn a_fraction_draws_its_bar_on_the_axis_between_null_delimiters() {
        let boxed = typeset("\\frac{1}{2}", false).expect("the fraction sets");
        let bar = rules(&boxed);
        assert_eq!(bar.len(), 1);
        let (x, y, w, h) = bar[0];
        assert!(close(x, 0.12) && close(w, 0.35) && close(h, 0.04) && close(y, -0.27), "{bar:?}");
        assert!(close(boxed.width, 0.59), "{}", boxed.width);
        let marks = glyphs(&boxed);
        assert!(close(marks[0].2, -0.394) && close(marks[1].2, 0.345), "num2 and denom2: {marks:?}");
    }

    #[test]
    fn a_square_root_stands_as_tall_as_katex_sets_it() {
        let boxed = typeset("\\sqrt{x}", false).expect("the root sets");
        // KaTeX's \sqrt{x}: a 1.04em strut at `vertical-align: -0.2397em`.
        assert!(close(boxed.height + boxed.depth, 1.04) && close(boxed.depth, 0.23972), "{} {}", boxed.height, boxed.depth);
        let vinculum = rules(&boxed);
        assert!(close(vinculum[0].0, 0.833) && close(vinculum[0].2, width(Face::MathItalic, 'x')), "{vinculum:?}");
        assert_eq!(glyphs(&boxed)[0].0, '\u{221a}');
    }

    #[test]
    fn left_and_right_grow_to_the_size_font_that_covers_the_body() {
        let boxed = typeset("\\left( \\frac{a}{b} \\right)", true).expect("the parentheses set");
        let marks = glyphs(&boxed);
        assert_eq!((marks[0].0, marks[0].3), ('(', Face::Size2));
        assert_eq!((marks.last().map(|m| m.0), marks.last().map(|m| m.3)), (Some(')'), Some(Face::Size2)));
        let small = typeset("\\left( x \\right)", false).expect("small parentheses set");
        assert_eq!(glyphs(&small)[0].3, Face::Main);
    }

    #[test]
    fn the_size_fonts_outlines_read_to_their_katex_extent() {
        let font = Ttf::new(FONTS[Face::Size2 as usize]).expect("KaTeX_Size2 parses");
        let contours = font.outline('\u{222b}').expect("the integral's outline");
        let ys = contours.iter().flatten().map(|p| p.1);
        let (low, high) = ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
        assert!((high - 1.36).abs() < 0.02 && (low + 0.862).abs() < 0.02, "{low} {high}");
        let paren = Ttf::new(FONTS[Face::Size4 as usize]).and_then(|font| font.outline('(')).expect("Size4's parenthesis");
        assert!(paren.iter().flatten().any(|p| p.2) && paren.iter().flatten().any(|p| !p.2), "on and off the curve");
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
    fn tex_nested_past_the_limit_reads_as_its_raw_tex_on_a_small_stack() {
        let nested = |n: usize| format!("{}x{}", "{".repeat(n), "}".repeat(n));
        let scripted = |n: usize| format!("{}x{}", "x^{".repeat(n), "}".repeat(n));
        let lefts = |n: usize| format!("{}x{}", "\\left(".repeat(n), "\\right)".repeat(n));
        let set = std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(move || {
                let deep = [nested(20_000), scripted(20_000), lefts(20_000)];
                let deep: Vec<_> = deep.iter().map(|tex| (typeset(tex, true).is_none(), tex_text(tex) == *tex)).collect();
                (deep, typeset(&nested(63), true).is_some(), tex_text(&scripted(60)) != scripted(60))
            })
            .unwrap()
            .join()
            .expect("the parse overflowed its stack");
        assert_eq!(set.0, [(true, true), (true, true), (true, false)], "past the limit: no box, and braces read raw");
        assert!(set.1, "a formula under the limit sets");
        assert!(set.2, "TeX under the limit reads as text, not raw");
    }

    struct Reply(Entity<TextViewState>);

    impl Render for Reply {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(super::super::markdown::view(&self.0, false, None, cx))
        }
    }

    fn shown(source: &'static str, cx: &mut TestAppContext) -> bool {
        cx.update(gpui_kit::init);
        cx.update(|cx| crate::ui::init(Side::Light, cx));
        let body = cx.new(|cx| TextViewState::markdown(source, cx));
        let reply = cx.new(|_| Reply(body));
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(reply.clone(), window, cx));
        cx.simulate_resize(size(px(760.), px(400.)));
        cx.run_until_parked();
        cx.update(|_, cx| cx.has_global::<Fonts>())
    }

    #[gpui_kit::test]
    fn a_reply_without_math_never_loads_katex_fonts(cx: &mut TestAppContext) {
        assert!(!shown("Costs $3 to $5, no math here.", cx));
    }

    #[gpui_kit::test]
    fn the_first_formula_loads_katex_fonts(cx: &mut TestAppContext) {
        assert!(shown("Inline $e^{i\\pi}+1=0$ and\n\n$$\\int_0^1 x^2\\,dx$$", cx));
    }

    #[gpui_kit::test]
    fn a_size_glyph_is_built_once_however_many_frames_draw_it(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| crate::ui::init(Side::Light, cx));
        let builds = || super::SIZE_BUILDS.with(|n| n.get());
        let before = builds();
        let body = cx.new(|cx| TextViewState::markdown("$$\\int_0^1 x^2\\,dx$$", cx));
        let reply = cx.new(|_| Reply(body));
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(reply.clone(), window, cx));
        cx.simulate_resize(size(px(760.), px(400.)));
        cx.run_until_parked();
        assert_eq!(builds() - before, 1, "the display integral's Size2 glyph on the first frame");
        for _ in 0..5 {
            reply.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        }
        assert_eq!(builds() - before, 1, "a later frame built the glyph again");
        let integral = super::size_glyph(Face::Size2, '\u{222b}').expect("the integral's glyph");
        let image = cx.update(|_, cx| cx.svg_renderer().render_single_frame(&integral.svg, 1.)).expect("the glyph's SVG rasterizes");
        let (w, h) = (image.size(0).width.0, image.size(0).height.0);
        assert!(w > 0 && h > 2 * w, "a tall integral, {w}x{h}");
        assert!(image.as_bytes(0).unwrap().chunks(4).any(|px| px[3] > 0), "the glyph has ink");
        for (face, table) in
            [(Face::Size1, super::SIZE1), (Face::Size2, super::SIZE2), (Face::Size3, super::SIZE3), (Face::Size4, super::SIZE4)]
        {
            table.iter().for_each(|(ch, _)| _ = super::size_glyph(face, *ch));
        }
        assert!(super::SIZE_CACHE.with(|c| c.borrow().len()) <= super::SIZE_GLYPHS, "the cache went past its bound");
    }
}
