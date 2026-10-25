// SPDX-License-Identifier: AGPL-3.0-only
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
