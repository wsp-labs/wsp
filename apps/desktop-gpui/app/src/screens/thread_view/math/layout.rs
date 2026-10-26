// SPDX-License-Identifier: AGPL-3.0-only
//! Atoms set into boxes of ink by appendix G's rules, as KaTeX sets them.

use super::parse::{parse, Atom, Len};
use super::{
    glue, Class, Face, Metrics, Style, AXIS, BIG_OP1, BIG_OP2, BIG_OP3, BIG_OP4, BIG_OP5, DELIM1, DELIM2, DENOM1, DENOM2, DISPLAY,
    NULL_DELIMITER, NUM1, NUM2, NUM3, RULE, SCRIPT_SPACE, SUB1, SUB2, SUB_DROP, SUP1, SUP2, SUP3, SUP_DROP, TEXT, X_HEIGHT,
};

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
pub(super) fn typeset(tex: &str, display: bool) -> Option<Boxed> {
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
