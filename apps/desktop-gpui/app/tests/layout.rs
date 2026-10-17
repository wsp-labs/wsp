// SPDX-License-Identifier: AGPL-3.0-only
//! Every piece's boxes against the sizes `apps/web/src/settings/layout.ts` and the files beside it write, within 1 px:
//! row and line floors, the card's inset, title and note line heights, the glyph frame, the section head, the select
//! and the field, the dialog's width and inset, keycaps and buttons. Radii and the drawn edges of the select and the
//! field are read off real pixels in `gallery.rs`.

mod web;

use std::cell::Cell;
use std::rc::Rc;

use desktop_gpui::ui::{self, glyph_frame, section_head, Button, Card, Dialog, Glyph, KeyCaps, Line, PickRow, Row, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, AnyElement, Bounds, Context, Modifiers, Pixels, TestAppContext, VisualTestContext, Window};

type Draw = Box<dyn Fn(&mut Window, &mut Context<Lone>) -> AnyElement>;

struct Lone(Draw);

impl Render for Lone {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(688.)).p(px(24.)).child((self.0)(window, cx))
    }
}

/// Draws `piece` alone in a 688 px window, the width the gallery's sections take, as the app wraps it.
fn draw(cx: &mut TestAppContext, piece: impl Fn(&mut Window, &mut Context<Lone>) -> AnyElement + 'static) -> &mut VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let view = cx.new(|_| Lone(Box::new(piece)));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
    cx.simulate_resize(size(px(688.), px(900.)));
    cx.run_until_parked();
    cx
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn"))
}

#[track_caller]
fn near(what: &str, got: Pixels, want: f32) {
    let got = f32::from(got);
    assert!((got - want).abs() < 0.5, "{what}: {got} px, the web writes {want} px");
}

/// `--font-sans-system` or `--font-mono-system` in index.css, less the generic family that ends it.
fn stack(name: &str) -> Vec<String> {
    let at = web::INDEX_CSS.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in index.css")) + name.len() + 1;
    let body = &web::INDEX_CSS[at..];
    body[..body.find(';').unwrap()]
        .split(',')
        .map(|face| face.trim().trim_matches('"').to_owned())
        .filter(|face| face != "sans-serif" && face != "monospace")
        .collect()
}

#[test]
fn the_font_stacks_are_index_css_s() {
    assert_eq!(stack("--font-sans-system"), ui::theme::SANS);
    assert_eq!(stack("--font-mono-system"), ui::theme::MONO);
}

#[gpui_kit::test]
fn a_row_with_one_line_stands_at_the_row_floor(cx: &mut TestAppContext) {
    let cx = draw(cx, |_, _| Card::new().item(Row::new("r", "Send with Enter")).into_any_element());
    near("ROW_FLOOR", bounds(cx, "row").size.height, web::px(web::constant(web::LAYOUT, "ROW_FLOOR"), "min-h"));
}

#[gpui_kit::test]
fn a_row_keeps_the_card_inset_and_its_padding(cx: &mut TestAppContext) {
    let cx = draw(cx, |_, _| Card::new().item(Row::new("r", "Default folder").note("New threads start here.")).into_any_element());
    let (row, title) = (bounds(cx, "row"), bounds(cx, "title"));
    near("CARD_INSET", title.left() - row.left(), web::px(web::constant(web::LAYOUT, "CARD_INSET"), "px"));
    near("a row's py-3", title.top() - row.top(), web::px(web::literal(web::ROWS, "flex flex-col justify-center py-"), "py"));
    near("SETTING_TITLE's line", title.size.height, web::px(web::constant(web::LAYOUT, "SETTING_TITLE"), "leading"));
}

#[gpui_kit::test]
fn a_line_stands_at_the_line_floor(cx: &mut TestAppContext) {
    let cx = draw(cx, |_, _| Card::new().item(Line::new("l", "Disk").value("9.4 GB")).into_any_element());
    let (row, label, word) = (bounds(cx, "row"), bounds(cx, "label"), bounds(cx, "word"));
    near("LINE_FLOOR", row.size.height, web::px(web::constant(web::LAYOUT, "LINE_FLOOR"), "min-h"));
    let inset = web::px(web::constant(web::LAYOUT, "CARD_INSET"), "px");
    near("the label's inset", label.left() - row.left(), inset);
    near("the value's inset", row.right() - word.right(), inset);
}

#[gpui_kit::test]
fn a_pick_names_its_thing_in_the_list_title_over_a_note(cx: &mut TestAppContext) {
    let cx = draw(cx, |_, _| {
        Card::new().item(PickRow::new("p", "Claude Code", true).glyph(Glyph::Terminal).note("Signed in")).into_any_element()
    });
    near("LIST_TITLE's line", bounds(cx, "title").size.height, web::px(web::constant(web::LAYOUT, "LIST_TITLE"), "leading"));
    near("NOTE's line", bounds(cx, "note").size.height, web::px(web::constant(web::LAYOUT, "NOTE"), "leading"));
    let lines = web::px(web::constant(web::LAYOUT, "LIST_TITLE"), "leading") + web::px(web::constant(web::LAYOUT, "NOTE"), "leading");
    let pad = web::px(web::literal(web::PICK_ROW, "justify-center gap-3 py-"), "py");
    near("a pick row, its padding and two lines over the floor", bounds(cx, "row").size.height, lines + 2. * pad);
}

#[gpui_kit::test]
fn the_glyph_frame_and_the_section_head_take_their_sizes(cx: &mut TestAppContext) {
    let cx = draw(cx, |_, cx| div().child(section_head("General", cx)).child(glyph_frame(Glyph::Monitor, cx)).into_any_element());
    let frame = bounds(cx, "glyph-frame");
    let side = web::px(web::constant(web::LAYOUT, "GLYPH_FRAME"), "size");
    near("GLYPH_FRAME wide", frame.size.width, side);
    near("GLYPH_FRAME high", frame.size.height, side);
    near("SECTION_HEAD", bounds(cx, "section-head").size.height, web::px(web::constant(web::LAYOUT, "SECTION_HEAD"), "min-h"));
}

#[gpui_kit::test]
fn the_dialog_takes_the_step_width_and_its_inset(cx: &mut TestAppContext) {
    let cx =
        draw(cx, |_, _| Dialog::new("Add a computer").count(2, 5).card(Card::new().item(Row::new("r", "Claude Code"))).into_any_element());
    near("STEP_WIDTH", bounds(cx, "dialog").size.width, web::px(web::constant(web::STEP_DIALOG, "STEP_WIDTH"), "max-w"));
    let frame = web::constant(web::STEP_DIALOG, "STEP_FRAME");
    let inset: f32 = frame.split("--settings-inset:").nth(1).unwrap().split("px").next().unwrap().parse().unwrap();
    near("the dialog's inset", bounds(cx, "title").left() - bounds(cx, "row").left(), inset);
}

#[gpui_kit::test]
fn a_keycap_is_20_high(cx: &mut TestAppContext) {
    let cx = draw(cx, |_, _| KeyCaps::new(&[&["Ctrl", "T"]]).into_any_element());
    let kbd = web::literal(web::KBD, "pointer-events-none inline-flex h-");
    let cap = bounds(cx, "keycap");
    near("Kbd high", cap.size.height, web::px(kbd, "h"));
    assert!(f32::from(cap.size.width) >= web::px(kbd, "min-w") - 1., "a keycap is at least as wide as it is high");
}

fn button_height(cx: &mut TestAppContext, button: fn() -> Button) -> Pixels {
    let cx = draw(cx, move |_, _| button().into_any_element());
    bounds(cx, "button").size.height
}

/// A size of `buttonVariants` in button.tsx: `default: "h-8 px-3"`.
fn button_size(name: &str) -> &'static str {
    let anchor = format!("{name}: \"");
    let at = web::BUTTON.find(&anchor).unwrap_or_else(|| panic!("no {anchor} in button.tsx")) + anchor.len();
    web::BUTTON[at..].split('"').next().unwrap()
}

#[gpui_kit::test]
fn the_default_button_is_32_high(cx: &mut TestAppContext) {
    near("a default button", button_height(cx, || Button::outline("b", "Back")), web::px(button_size("default"), "h"));
}

#[gpui_kit::test]
fn the_xs_button_is_24_high(cx: &mut TestAppContext) {
    near("an xs button", button_height(cx, || Button::outline("b", "Copy").xs()), web::px(button_size("xs"), "h"));
}

#[gpui_kit::test]
fn the_icon_buttons_are_square(cx: &mut TestAppContext) {
    near("an icon button", button_height(cx, || Button::icon("b", Glyph::Settings)), web::px(button_size("icon"), "size"));
}

#[gpui_kit::test]
fn the_xs_icon_button_is_24(cx: &mut TestAppContext) {
    near("an icon-xs button", button_height(cx, || Button::icon_xs("b", Glyph::Copy)), web::px(button_size("\"icon-xs\""), "size"));
}

#[gpui_kit::test]
fn a_press_on_a_rows_control_does_not_open_the_row(cx: &mut TestAppContext) {
    let (opened, pressed) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (o, p) = (opened.clone(), pressed.clone());
    let cx = draw(cx, move |_, _| {
        let (o, p) = (o.clone(), p.clone());
        let change = Button::outline("change", "Change").xs().on_click(move |_, _, _| p.set(p.get() + 1));
        Card::new().item(Row::new("r", "Send with Enter").control(change).on_open(move |_, _, _| o.set(o.get() + 1))).into_any_element()
    });
    let (button, title) = (bounds(cx, "button").center(), bounds(cx, "title").center());
    cx.simulate_click(button, Modifiers::none());
    assert_eq!((pressed.get(), opened.get()), (1, 0), "the press reached the row");
    cx.simulate_click(title, Modifiers::none());
    assert_eq!((pressed.get(), opened.get()), (1, 1), "a press on the words opens the row");
}
