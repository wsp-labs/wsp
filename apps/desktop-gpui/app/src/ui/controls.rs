// SPDX-License-Identifier: AGPL-3.0-only
//! The controls a row holds and the acts a view takes: the select and the field at the row's size, the picks (a radio
//! or a checkbox at the left of a row), and the tactile keycap buttons.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::searchable_list::SearchableVec;
use gpui_kit::component::select::{Select as KitSelect, SelectState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, FocusableExt as _, Icon, Sizable as _};
use gpui_kit::{div, point, prelude::*, px, AnyElement, App, BoxShadow, ClickEvent, Entity, FontWeight, Hsla, SharedString, Window};

use super::card::{glyph_frame, row_frame, CardItem, Corners};
use super::icons::{Glyph, TICK};
use super::theme::{ActivePalette, Palette};
use super::{size, text};

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A held control waits on something beside it: dimmed in its own look, and nothing else.
const HELD: f32 = 0.64;

/// How a held control's parts read: the web dims the control as one layer, but GPUI fades each fill, edge and glyph
/// on its own, which stacks them; so each part is drawn solid in the color the dimmed layer shows over the page.
struct Dim {
    page: Hsla,
    ground: Hsla,
}

impl Dim {
    fn new(page: Hsla, fill: Hsla) -> Self {
        Dim { page, ground: page.blend(fill) }
    }

    fn fill(&self) -> Hsla {
        self.page.blend(self.ground.opacity(HELD))
    }

    /// A part drawn over the control's fill.
    fn over(&self, part: Hsla) -> Hsla {
        self.page.blend(self.ground.blend(part).opacity(HELD))
    }
}

/// `KEYCAP_BEVEL`, `inset 0 1px 0 var(--keycap-top)`: the 1 px light along the inside of the top edge.
fn bevel(p: &Palette) -> Vec<BoxShadow> {
    vec![BoxShadow { color: p.keycap_top, offset: point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }]
}

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonKind {
    /// The one primary act in a view, at the foot's right.
    Primary,
    /// The tactile keycap for every other act.
    Outline,
    /// No edge, the muted ink, the accent fill under the pointer.
    Ghost,
    /// A quiet word with no padding, so its text edge is its box edge.
    Text,
}

#[derive(Clone, Copy, PartialEq)]
enum ButtonSize {
    Default,
    Xs,
    /// The ghost icon button, 28 px square.
    Icon,
    /// 24 px square, the copy row's button.
    IconXs,
}

/// `Button`: the keycap, in its kind and size. A held button keeps its look at 64% and takes no click.
#[derive(IntoElement)]
pub struct Button {
    id: SharedString,
    kind: ButtonKind,
    size: ButtonSize,
    label: Option<SharedString>,
    glyph: Option<Glyph>,
    held: bool,
    on_click: Option<Click>,
}

impl Button {
    fn new(id: impl Into<SharedString>, kind: ButtonKind, label: impl Into<SharedString>) -> Self {
        Button { id: id.into(), kind, size: ButtonSize::Default, label: Some(label.into()), glyph: None, held: false, on_click: None }
    }

    pub fn primary(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button::new(id, ButtonKind::Primary, label)
    }

    pub fn outline(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button::new(id, ButtonKind::Outline, label)
    }

    /// The quiet word, as Skip for now.
    pub fn text(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button { size: ButtonSize::Xs, ..Button::new(id, ButtonKind::Text, label) }
    }

    /// `AddButton`: every Add carries the plus before its label.
    pub fn add(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button::outline(id, label).glyph(Glyph::Plus)
    }

    /// The 28 px ghost icon button.
    pub fn icon(id: impl Into<SharedString>, glyph: Glyph) -> Self {
        Button {
            id: id.into(),
            kind: ButtonKind::Ghost,
            size: ButtonSize::Icon,
            label: None,
            glyph: Some(glyph),
            held: false,
            on_click: None,
        }
    }

    /// The 24 px ghost icon button.
    pub fn icon_xs(id: impl Into<SharedString>, glyph: Glyph) -> Self {
        Button { size: ButtonSize::IconXs, ..Button::icon(id, glyph) }
    }

    /// The xs size, for an inline act in a row.
    pub fn xs(mut self) -> Self {
        self.size = ButtonSize::Xs;
        self
    }

    pub fn glyph(mut self, glyph: Glyph) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let transparent = gpui_kit::transparent_black();
        let (fill, edge, ink, glyph_ink, hover) = match self.kind {
            ButtonKind::Primary => (p.primary, p.primary, p.primary_foreground, p.primary_foreground, Some(p.primary_hover)),
            ButtonKind::Outline => (p.input_fill, p.input, p.foreground, p.muted_foreground, Some(p.input_fill_hover)),
            ButtonKind::Ghost => (transparent, transparent, p.muted_foreground, p.muted_foreground, Some(p.accent)),
            ButtonKind::Text => (transparent, transparent, p.muted_foreground, p.muted_foreground, None),
        };
        let (height, pad, (font, line), gap, glyph_size) = match self.size {
            ButtonSize::Default => (px(32.), px(12.), (text::BUTTON, px(19.5)), px(6.), px(14.)),
            ButtonSize::Xs => (px(24.), px(8.), text::XS, px(4.), px(14.)),
            ButtonSize::Icon => (px(28.), px(0.), (text::BUTTON, px(19.5)), px(0.), px(16.)),
            ButtonSize::IconXs => (px(24.), px(0.), (text::BUTTON, px(19.5)), px(0.), px(14.)),
        };
        let mut keycap = bevel(&p);
        let (fill, edge, ink, glyph_ink) = if self.held {
            let dim = Dim::new(p.background, fill);
            keycap[0].color = dim.over(p.keycap_top);
            let edge = if edge.a > 0. { dim.over(edge) } else { edge };
            let fill = if fill.a > 0. { dim.fill() } else { fill };
            (fill, edge, dim.over(ink), dim.over(glyph_ink))
        } else {
            (fill, edge, ink, glyph_ink)
        };
        let square = matches!(self.size, ButtonSize::Icon | ButtonSize::IconXs);
        let pad = if self.kind == ButtonKind::Text { px(0.) } else { pad };
        let weight = if matches!(self.kind, ButtonKind::Ghost | ButtonKind::Text) { FontWeight::NORMAL } else { FontWeight::MEDIUM };
        h_flex()
            .id(self.id)
            .debug_selector(|| "button".into())
            .flex_none()
            .justify_center()
            .h(height)
            .when(square, |el| el.w(height))
            .px(pad)
            .gap(gap)
            .rounded(size::BUTTON_RADIUS)
            .border_1()
            .border_color(edge)
            .bg(fill)
            // GPUI paints an inset shadow under the border, where CSS keeps it inside; a child filling the padding box holds it there.
            .when(matches!(self.kind, ButtonKind::Primary | ButtonKind::Outline), |el| {
                el.child(div().absolute().inset_0().rounded(size::BUTTON_RADIUS - px(1.)).shadow(keycap))
            })
            .text_size(font)
            .line_height(line)
            .font_weight(weight)
            .text_color(ink)
            .children(self.glyph.map(|g| Icon::new(g).with_size(glyph_size).text_color(glyph_ink)))
            .children(self.label)
            .map(|el| {
                if self.held {
                    return el;
                }
                el.cursor_pointer()
                    .hover(|s| {
                        let s = match hover {
                            Some(hover) => s.bg(hover),
                            None => s,
                        };
                        if matches!(self.kind, ButtonKind::Ghost | ButtonKind::Text) {
                            s.text_color(p.foreground)
                        } else {
                            s
                        }
                    })
                    .when_some(self.on_click, |el, f| el.on_click(move |e, window, cx| f(e, window, cx)))
            })
    }
}

/// The radio of a `Choice`: 16 px round on the field's edge, and once picked the primary fill with its 6 px dot.
fn radio(picked: bool, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let dark = cx.theme().is_dark();
    let ground = if dark { p.input.opacity(0.32) } else { p.background };
    div().debug_selector(|| "radio".into()).flex().flex_none().items_center().justify_center().size(px(16.)).rounded_full().map(|el| {
        if picked {
            el.bg(p.primary).child(div().size(px(6.)).rounded_full().bg(p.primary_foreground))
        } else {
            el.border_1().border_color(p.input).bg(ground).shadow(p.control_shadow.clone())
        }
    })
}

/// The checkbox of a `PickRow`: 16 px with a 4 px radius, and once ticked the primary fill with its tick.
fn checkbox(checked: bool, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let dark = cx.theme().is_dark();
    let ground = if dark { p.input.opacity(0.32) } else { p.background };
    div().debug_selector(|| "checkbox".into()).flex().flex_none().items_center().justify_center().size(px(16.)).rounded(px(4.)).map(|el| {
        if checked {
            el.bg(p.primary).child(Icon::empty().path(TICK).with_size(px(12.)).text_color(p.primary_foreground))
        } else {
            el.border_1().border_color(p.input).bg(ground).shadow(p.control_shadow.clone())
        }
    })
}

/// The words of a pick: the name in `LIST_TITLE`, muted while not picked, and its note under it.
fn pick_words(name: SharedString, note: Option<SharedString>, on: bool, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let ink: Hsla = if on { p.foreground } else { p.muted_foreground };
    v_flex()
        .flex_1()
        .min_w_0()
        .child(
            h_flex().flex_wrap().items_baseline().gap_x_2().child(
                div()
                    .debug_selector(|| "title".into())
                    .text_size(text::LIST_TITLE.0)
                    .line_height(text::LIST_TITLE.1)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(ink)
                    .child(name),
            ),
        )
        .children(note.map(|note| {
            div()
                .debug_selector(|| "note".into())
                .text_size(text::NOTE.0)
                .line_height(text::NOTE.1)
                .text_color(p.muted_foreground)
                .child(note)
        }))
}

/// One pick in a row: the radio or checkbox, the glyph frame and the words.
struct Pick {
    id: SharedString,
    on: bool,
    glyph: Option<Glyph>,
    name: SharedString,
    note: Option<SharedString>,
    on_click: Option<Click>,
    corners: Corners,
}

impl Pick {
    fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, on: bool) -> Self {
        Pick { id: id.into(), on, glyph: None, name: name.into(), note: None, on_click: None, corners: Corners::default() }
    }

    fn render(self, mark: impl IntoElement, cx: &App) -> AnyElement {
        let words = pick_words(self.name, self.note, self.on, cx);
        let lead = h_flex().flex_1().min_w_0().gap_3().child(mark).children(self.glyph.map(|g| glyph_frame(g, cx))).child(words);
        row_frame(&self.id, size::ROW_FLOOR, self.corners)
            .cursor_pointer()
            .when_some(self.on_click, |el, f| el.on_click(move |e, window, cx| f(e, window, cx)))
            .child(lead)
            .into_any_element()
    }
}

macro_rules! pick_builders {
    ($ty:ident) => {
        impl $ty {
            pub fn glyph(mut self, glyph: Glyph) -> Self {
                self.0.glyph = Some(glyph);
                self
            }

            pub fn note(mut self, note: impl Into<SharedString>) -> Self {
                self.0.note = Some(note.into());
                self
            }

            pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
                self.0.on_click = Some(Box::new(f));
                self
            }
        }
    };
}

/// One of many: the radio at the left; every name not picked dims to muted. Never a filled pill, never a tick at the
/// right edge.
#[derive(IntoElement)]
pub struct Choice(Pick);

impl Choice {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, picked: bool) -> Self {
        Choice(Pick::new(id, name, picked))
    }
}

pick_builders!(Choice);

impl RenderOnce for Choice {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mark = radio(self.0.on, cx);
        self.0.render(mark, cx)
    }
}

impl From<Choice> for CardItem {
    fn from(choice: Choice) -> Self {
        CardItem::placed(move |corners| Choice(Pick { corners, ..choice.0 }).into_any_element())
    }
}

/// Many of many: the checkbox at the left in the same row shape; an unticked name dims to muted.
#[derive(IntoElement)]
pub struct PickRow(Pick);

impl PickRow {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, checked: bool) -> Self {
        PickRow(Pick::new(id, name, checked))
    }
}

pick_builders!(PickRow);

impl RenderOnce for PickRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mark = checkbox(self.0.on, cx);
        self.0.render(mark, cx)
    }
}

impl From<PickRow> for CardItem {
    fn from(row: PickRow) -> Self {
        CardItem::placed(move |corners| PickRow(Pick { corners, ..row.0 }).into_any_element())
    }
}

/// The state a select in a row reads and writes.
pub type SelectList = SelectState<SearchableVec<SharedString>>;

/// A select in a row at `SELECT_WIDTH`: gpui-kit's select, its trigger drawn to wsp's size.
#[derive(IntoElement)]
pub struct Select {
    state: Entity<SelectList>,
    held: bool,
}

impl Select {
    pub fn new(state: &Entity<SelectList>) -> Self {
        Select { state: state.clone(), held: false }
    }

    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }
}

impl RenderOnce for Select {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let chevron = Icon::new(Glyph::ChevronDown).opacity(0.5).mr(px(-4.));
        let dim = Dim::new(p.background, p.input_fill);
        let (edge, fill, ink) =
            if self.held { (dim.over(p.input), dim.fill(), dim.over(p.foreground)) } else { (p.input, p.input_fill, p.foreground) };
        let select = KitSelect::new(&self.state)
            .icon(chevron)
            .focus_ring(false)
            .w(size::SELECT_WIDTH)
            .h(size::CONTROL_HEIGHT)
            .rounded(size::CONTROL_RADIUS)
            .px(px(9.))
            .py_0()
            .border_color(edge)
            .bg(fill)
            .text_color(ink)
            .text_size(text::FACT);
        field_frame(select, self.held)
    }
}

/// A control at the row's width that, while held, lets nothing through.
fn field_frame(control: impl IntoElement, held: bool) -> impl IntoElement {
    div()
        .debug_selector(|| "control".into())
        .relative()
        .flex_none()
        .w(size::SELECT_WIDTH)
        .h(size::CONTROL_HEIGHT)
        .child(control)
        .when(held, |el| el.child(div().absolute().inset_0().occlude()))
}

/// A field a person types into in a row (`ROW_FIELD`): gpui-kit's input at the select's height and size.
#[derive(IntoElement)]
pub struct Field {
    state: Entity<InputState>,
}

impl Field {
    pub fn new(state: &Entity<InputState>) -> Self {
        Field { state: state.clone() }
    }
}

impl RenderOnce for Field {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let input = Styled::h(Input::new(&self.state), size::CONTROL_HEIGHT)
            .w(size::SELECT_WIDTH)
            .rounded(size::CONTROL_RADIUS)
            .px(px(11.))
            .py_0()
            .border_color(p.input)
            .bg(p.input_fill)
            .text_size(text::FACT);
        field_frame(input, false)
    }
}
