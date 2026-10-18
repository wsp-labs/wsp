// SPDX-License-Identifier: AGPL-3.0-only
//! The card and what it holds, after `rows.tsx`: a quiet head over one soft card, a faint rule between its rows; a
//! row's title over its sentence with the slot beside them; a line's label and its value or keycaps; the glyph frame.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, relative, AnyElement, App, ClickEvent, FontWeight, Hsla, MouseButton, Pixels, SharedString, Window};

use super::icons::Glyph;
use super::theme::ActivePalette;
use super::{size, text};

/// The slot's floor at 640 px and wider (`SPLIT_CLASS`'s `minmax(var(--settings-slot,10rem),auto)`).
const SLOT_FLOOR: Pixels = px(160.);
/// `max-w-xl` on a lede and a row's sentence.
const DESCRIPTION_MAX: Pixels = px(576.);

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Where a child stands in its card: the corners it rounds to, since GPUI does not clip children to their parent's
/// radius, and the inset the card keeps (20 px, or a dialog's 16).
#[derive(Clone, Copy)]
pub struct Corners {
    pub top: bool,
    pub bottom: bool,
    pub inset: Pixels,
    /// The faint rule above every child but the first, inside the child's own floor as the web's border-box draws it.
    pub rule: Option<Hsla>,
}

impl Default for Corners {
    fn default() -> Self {
        Corners { top: false, bottom: false, inset: size::CARD_INSET, rule: None }
    }
}

impl Corners {
    /// The card's radius inside its 1 px edge.
    pub fn apply<E: Styled>(self, el: E) -> E {
        let el = match self.rule {
            Some(rule) => el.border_t_1().border_color(rule),
            None => el,
        };
        let r = size::CARD_RADIUS - px(1.);
        let el = if self.top { el.rounded_tl(r).rounded_tr(r) } else { el };
        if self.bottom {
            el.rounded_bl(r).rounded_br(r)
        } else {
            el
        }
    }
}

/// One child of a card, told which corners it stands at.
pub struct CardItem(Box<dyn FnOnce(Corners) -> AnyElement>);

impl CardItem {
    pub fn any(el: impl IntoElement + 'static) -> Self {
        CardItem(Box::new(move |corners| corners.apply(div()).child(el).into_any_element()))
    }

    pub fn placed(f: impl FnOnce(Corners) -> AnyElement + 'static) -> Self {
        CardItem(Box::new(f))
    }
}

/// `SECTION_HEAD`: the quiet head over a card.
pub fn section_head(head: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    h_flex()
        .debug_selector(|| "section-head".into())
        .min_h(size::SECTION_HEAD_FLOOR)
        .text_size(text::SECTION_HEAD.0)
        .line_height(text::SECTION_HEAD.1)
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.palette().foreground.opacity(0.7))
        .child(head.into())
}

/// `GLYPH_FRAME` holding a `GLYPH`.
pub fn glyph_frame(glyph: Glyph, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    div()
        .debug_selector(|| "glyph-frame".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size::GLYPH_FRAME)
        .rounded(size::GLYPH_FRAME_RADIUS)
        .border_1()
        .border_color(p.border)
        .bg(p.foreground.opacity(0.04))
        .child(Icon::new(glyph).with_size(size::GLYPH).text_color(p.foreground.opacity(0.8)))
}

/// One card: an optional head and lede, the soft surface with a rule between its children, and an act under it.
#[derive(IntoElement)]
pub struct Card {
    head: Option<SharedString>,
    lede: Option<SharedString>,
    items: Vec<CardItem>,
    under: Vec<AnyElement>,
    inset: Pixels,
}

impl Card {
    pub fn new() -> Self {
        Card { head: None, lede: None, items: Vec::new(), under: Vec::new(), inset: size::CARD_INSET }
    }

    /// The sides its rows keep, where the card stands in a dialog.
    pub fn inset(mut self, inset: Pixels) -> Self {
        self.inset = inset;
        self
    }

    pub fn head(mut self, head: impl Into<SharedString>) -> Self {
        self.head = Some(head.into());
        self
    }

    /// One sentence under the head, for a card whose rows need the why.
    pub fn lede(mut self, lede: impl Into<SharedString>) -> Self {
        self.lede = Some(lede.into());
        self
    }

    pub fn item(mut self, item: impl Into<CardItem>) -> Self {
        self.items.push(item.into());
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = impl Into<CardItem>>) -> Self {
        self.items.extend(items.into_iter().map(Into::into));
        self
    }

    /// The act the card invites, under it.
    pub fn under(mut self, el: impl IntoElement) -> Self {
        self.under.push(el.into_any_element());
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Card::new()
    }
}

impl RenderOnce for Card {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let (rule, edge, fill) = (p.border.opacity(0.5), p.border.opacity(0.6), p.card.opacity(0.4));
        let (count, inset) = (self.items.len(), self.inset);
        let surface = (count > 0).then(|| {
            v_flex().debug_selector(|| "card".into()).rounded(size::CARD_RADIUS).border_1().border_color(edge).bg(fill).children(
                self.items
                    .into_iter()
                    .enumerate()
                    .map(|(ix, item)| (item.0)(Corners { top: ix == 0, bottom: ix + 1 == count, inset, rule: (ix > 0).then_some(rule) })),
            )
        });
        let heading = (self.head.is_some() || self.lede.is_some()).then(|| {
            v_flex().gap_1().children(self.head.map(|head| section_head(head, cx))).children(self.lede.map(|lede| {
                div()
                    .max_w(DESCRIPTION_MAX)
                    .text_size(text::DESCRIPTION.0)
                    .line_height(text::DESCRIPTION.1)
                    .text_color(p.muted_foreground)
                    .child(lede)
            }))
        });
        v_flex()
            .gap_3()
            .children(heading)
            .children(surface)
            .when(!self.under.is_empty(), |el| el.child(h_flex().gap_2().children(self.under)))
    }
}

/// Which mono a word in a slot wears: the foreground for a value a person reads, the muted for a state.
#[derive(Clone, Copy, PartialEq)]
pub enum WordClass {
    Value,
    Fact,
}

/// A word in the `FACT` or `VALUE` class.
pub fn word(text: impl Into<SharedString>, class: WordClass, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let (size, color) = match class {
        WordClass::Value => (text::VALUE, p.foreground),
        WordClass::Fact => (text::FACT, p.muted_foreground),
    };
    div().debug_selector(|| "word".into()).text_size(size.0).line_height(size.1).text_color(color).child(text.into())
}

/// One row: the title over its sentence, and beside them the slot with at most one word and one control.
#[derive(IntoElement)]
pub struct Row {
    id: SharedString,
    title: SharedString,
    lead: Option<AnyElement>,
    note: Option<SharedString>,
    word: Option<(SharedString, WordClass)>,
    control: Option<AnyElement>,
    open: Option<Click>,
    warning: bool,
    corners: Corners,
}

impl Row {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Row {
            id: id.into(),
            title: title.into(),
            lead: None,
            note: None,
            word: None,
            control: None,
            open: None,
            warning: false,
            corners: Corners::default(),
        }
    }

    /// A glyph before the title, in its frame.
    pub fn lead(mut self, lead: impl IntoElement) -> Self {
        self.lead = Some(lead.into_any_element());
        self
    }

    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.word = Some((value.into(), WordClass::Value));
        self
    }

    pub fn fact(mut self, fact: impl Into<SharedString>) -> Self {
        self.word = Some((fact.into(), WordClass::Fact));
        self
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }

    /// A row that opens a page: the whole row is the button and the slot ends in the chevron.
    pub fn on_open(mut self, open: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.open = Some(Box::new(open));
        self
    }

    /// A title that warns: a computer that runs an older wsp.
    pub fn warning(mut self) -> Self {
        self.warning = true;
        self
    }
}

impl From<Row> for CardItem {
    fn from(row: Row) -> Self {
        CardItem(Box::new(move |corners| Row { corners, ..row }.into_any_element()))
    }
}

/// A row's title in `SETTING_TITLE`.
fn title(text: SharedString, color: Hsla) -> impl IntoElement {
    let (size, height) = text::SETTING_TITLE;
    div()
        .debug_selector(|| "title".into())
        .min_w_0()
        .text_size(size)
        .line_height(height)
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(text)
}

/// The floor and inset every row and line stands at.
pub(super) fn row_frame(id: &SharedString, floor: Pixels, corners: Corners) -> gpui_kit::Stateful<gpui_kit::Div> {
    corners
        .apply(v_flex().id(id.clone()).debug_selector(|| "row".into()).justify_center().py(size::ROW_PAD_Y).px(corners.inset).min_h(floor))
}

impl RenderOnce for Row {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let title_color = if self.warning { p.warning_foreground } else { p.foreground };
        let words = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(h_flex().min_h(px(20.)).flex_wrap().gap_x_2().child(title(self.title, title_color)))
            .children(self.note.map(|note| {
                div()
                    .debug_selector(|| "note".into())
                    .max_w(DESCRIPTION_MAX)
                    .text_size(text::DESCRIPTION.0)
                    .line_height(text::DESCRIPTION.1)
                    .text_color(p.muted_foreground)
                    .child(note)
            }));
        let text = h_flex().flex_1().min_w_0().gap_3().children(self.lead).child(words);
        let opens = self.open.is_some();
        let chevron_only = opens && self.word.is_none() && self.control.is_none();
        let slot = (self.word.is_some() || self.control.is_some() || opens).then(|| {
            h_flex()
                .debug_selector(|| "slot".into())
                .flex_none()
                .when(!chevron_only, |el| el.min_w(SLOT_FLOOR))
                .justify_end()
                .gap_3()
                .children(self.word.map(|(text, class)| word(text, class, cx)))
                // A row that opens a page leaves a press on its control to the control, as rows.tsx's does.
                .children(
                    self.control.map(|control| div().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(control)),
                )
                .when(opens, |el| el.child(Icon::new(Glyph::ChevronRight).with_size(px(14.)).text_color(p.muted_foreground)))
        });
        let body = h_flex().gap_5().child(text).children(slot);
        let frame = row_frame(&self.id, size::ROW_FLOOR, self.corners);
        match self.open {
            Some(open) => {
                frame.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.6))).on_click(move |e, window, cx| open(e, window, cx)).child(body)
            }
            None => frame.child(body),
        }
    }
}

/// A chord's keycaps, one group per chord, with the word between them where they read as a range.
#[derive(IntoElement)]
pub struct KeyCaps {
    chords: Vec<Vec<SharedString>>,
    joiner: Option<SharedString>,
}

impl KeyCaps {
    pub fn new(chords: &[&[&str]]) -> Self {
        let chords = chords.iter().map(|c| c.iter().map(|k| SharedString::from(k.to_string())).collect()).collect();
        KeyCaps { chords, joiner: None }
    }

    pub fn joiner(mut self, joiner: impl Into<SharedString>) -> Self {
        self.joiner = Some(joiner.into());
        self
    }
}

/// One key, `Kbd`: 20 px high and at least as wide, mono 12 px on the muted fill.
pub fn keycap(key: SharedString, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    h_flex()
        .debug_selector(|| "keycap".into())
        .h(px(20.))
        .min_w(px(20.))
        .px_1()
        .justify_center()
        .rounded(px(6.))
        .bg(p.muted)
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(text::XS.0)
        .line_height(text::XS.1)
        .text_color(p.muted_foreground)
        .child(key)
}

impl RenderOnce for KeyCaps {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let joiner = self.joiner;
        h_flex().flex_none().gap_2().children(self.chords.into_iter().enumerate().map(|(ix, chord)| {
            h_flex()
                .gap_2()
                .when(ix > 0, |el| el.children(joiner.clone().map(|j| word(j, WordClass::Fact, cx))))
                .child(h_flex().gap_1().children(chord.into_iter().map(|key| keycap(key, cx))))
        }))
    }
}

/// One line: the label, and at its right one word or a chord's keycaps. Its sentence is its hover, never a note.
#[derive(IntoElement)]
pub struct Line {
    id: SharedString,
    label: SharedString,
    right: Option<AnyElement>,
    value: Option<(SharedString, WordClass)>,
    keys: Option<KeyCaps>,
    empty: bool,
    corners: Corners,
}

impl Line {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Line { id: id.into(), label: label.into(), right: None, value: None, keys: None, empty: false, corners: Corners::default() }
    }

    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some((value.into(), WordClass::Value));
        self
    }

    pub fn keys(mut self, keys: KeyCaps) -> Self {
        self.keys = Some(keys);
        self
    }

    /// What stands at the right in the value's place: a mark, a spinner, a control.
    pub fn right(mut self, el: impl IntoElement) -> Self {
        self.right = Some(el.into_any_element());
        self
    }

    /// The card's one empty line: what is not there yet, in the quiet note.
    pub fn empty(mut self) -> Self {
        self.empty = true;
        self
    }
}

impl From<Line> for CardItem {
    fn from(line: Line) -> Self {
        CardItem(Box::new(move |corners| Line { corners, ..line }.into_any_element()))
    }
}

impl RenderOnce for Line {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let right = self
            .right
            .or_else(|| self.value.map(|(text, class)| word(text, class, cx).into_any_element()))
            .or_else(|| self.keys.map(IntoElement::into_any_element));
        let (size, height) = if self.empty { text::NOTE } else { text::SETTING_TITLE };
        let label = div()
            .debug_selector(|| "label".into())
            .min_w_0()
            .text_size(size)
            .line_height(height)
            .text_color(if self.empty { p.muted_foreground } else { p.foreground })
            .child(self.label);
        let body = match right {
            Some(right) => h_flex()
                .justify_between()
                .gap_8()
                .child(label.flex_none().max_w(relative(0.5)))
                .child(h_flex().min_w_0().justify_end().child(right)),
            None => h_flex().child(label),
        };
        row_frame(&self.id, size::LINE_FLOOR, self.corners).child(body)
    }
}
