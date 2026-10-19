// SPDX-License-Identifier: AGPL-3.0-only
//! The card and what it holds, after `rows.tsx`: a quiet head over one soft card, a faint rule between its rows; a
//! row's title over its sentence with the slot beside them; a line's label and its value or keycaps; the glyph frame.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    div, point, prelude::*, px, relative, AnyElement, App, BoxShadow, ClickEvent, ExternalPaths, FontWeight, Hsla, KeyDownEvent,
    MouseButton, Pixels, SharedString, Window,
};

use super::computer_glyph::ComputerIcon;
use super::controls::Button;
use super::dialog::tip;
use super::focus::{ring, ring_inset};
use super::icons::Glyph;
use super::motion;
use super::theme::ActivePalette;
use super::{size, text};

/// The slot's floor at 640 px and wider (`SPLIT_CLASS`'s `minmax(var(--settings-slot,10rem),auto)`).
const SLOT_FLOOR: Pixels = px(160.);
/// `LED_SLOT_INDENT`'s `max-sm:pl-11`: the glyph frame's 32 px and the 12 px gap.
const LED_SLOT_INDENT: Pixels = px(44.);
/// `max-w-xl` on a lede and a row's sentence.
const DESCRIPTION_MAX: Pixels = px(576.);
/// `SETTINGS_WORDS.resetRow`, the reset arrow's hover.
pub const RESET_WORD: &str = "Back to the default";
/// A fact that takes no leading of its own sits at the page's `line-height: 1.5`.
const FACT_LINE: Pixels = px(19.5);

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type Take = std::rc::Rc<dyn Fn(Taken, &mut Window, &mut App) + 'static>;

/// A file given to a row that takes one: dropped on it, or pasted while it or its control holds focus.
pub enum Taken<'a> {
    Dropped(&'a ExternalPaths),
    Pasted,
}

/// Whether a key is the paste chord: Command-V on a Mac, Control-V elsewhere.
pub fn is_paste(event: &KeyDownEvent) -> bool {
    let keys = &event.keystroke;
    keys.key == "v" && keys.modifiers.secondary() && !keys.modifiers.alt && !keys.modifiers.shift
}

/// Where a child stands in its card: the corners it rounds to, since GPUI does not clip children to their parent's
/// radius, and the inset the card keeps (20 px, or a dialog's 16).
#[derive(Clone, Copy)]
pub struct Corners {
    pub top: bool,
    pub bottom: bool,
    pub inset: Pixels,
    /// The faint rule above every child but the first, inside the child's own floor as the web's border-box draws it.
    pub rule: Option<Hsla>,
    /// The card's own radius: 11 px, or a locked page's 14.
    pub radius: Pixels,
}

impl Default for Corners {
    fn default() -> Self {
        Corners { top: false, bottom: false, inset: size::CARD_INSET, rule: None, radius: size::CARD_RADIUS }
    }
}

impl Corners {
    /// The card's radius inside its 1 px edge.
    pub fn apply<E: Styled>(self, el: E) -> E {
        let el = match self.rule {
            Some(rule) => el.border_t_1().border_color(rule),
            None => el,
        };
        let r = self.radius - px(1.);
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

    /// The item drawn as before, then handed to `f` with its corners, to stand inside whatever `f` puts round it.
    pub fn map(self, f: impl FnOnce(AnyElement, Corners) -> AnyElement + 'static) -> Self {
        CardItem(Box::new(move |corners| f((self.0)(corners), corners)))
    }

    /// Drawn at its place, for an item that stands it inside one of its own: a row that drags.
    pub fn place(self, corners: Corners) -> AnyElement {
        (self.0)(corners)
    }
}

/// Words for one slot, as `rows.tsx`'s `Words`: a phrase, or facts held apart by space and read as one line.
#[derive(Clone)]
pub enum Words {
    One(SharedString),
    Spaced(Vec<SharedString>),
}

impl Words {
    /// The words as a search or a hover reads them.
    pub fn line(&self) -> SharedString {
        match self {
            Words::One(text) => text.clone(),
            Words::Spaced(parts) => parts.iter().map(|p| p.as_ref()).collect::<Vec<_>>().join(", ").into(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Words::One(text) => text.is_empty(),
            Words::Spaced(parts) => parts.iter().all(|p| p.is_empty()),
        }
    }
}

impl From<&str> for Words {
    fn from(text: &str) -> Self {
        Words::One(SharedString::from(text.to_owned()))
    }
}

impl From<String> for Words {
    fn from(text: String) -> Self {
        Words::One(text.into())
    }
}

impl From<SharedString> for Words {
    fn from(text: SharedString) -> Self {
        Words::One(text)
    }
}

impl<T: Into<SharedString>> From<Vec<T>> for Words {
    fn from(parts: Vec<T>) -> Self {
        Words::Spaced(parts.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<SharedString>, const N: usize> From<[T; N]> for Words {
    fn from(parts: [T; N]) -> Self {
        Words::Spaced(parts.into_iter().map(Into::into).collect())
    }
}

/// `Spaced`: each fact after the first stands a space and `ms-2` from the one before, never a dot between them.
fn spaced(words: Words, nowrap: bool) -> AnyElement {
    match words {
        Words::One(text) => text.into_any_element(),
        Words::Spaced(parts) => {
            let last = parts.len().saturating_sub(1);
            h_flex()
                .when(!nowrap, |el| el.flex_wrap())
                .when(nowrap, |el| el.overflow_hidden())
                .gap_x_2()
                .children(parts.into_iter().enumerate().map(|(ix, part)| {
                    let part: SharedString = if ix < last { format!("{part} ").into() } else { part };
                    div().flex_none().whitespace_nowrap().child(part)
                }))
                .into_any_element()
        }
    }
}

/// `SECTION_HEAD`: the quiet head over a card.
pub fn section_head(head: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    head_box(false, cx).text_color(cx.palette().foreground.opacity(0.7)).child(head.into())
}

/// A locked page's head (`data-locked`) on its own, out of a card: 14 px regular in the foreground at 70%.
pub fn locked_head(head: impl Into<SharedString>, cx: &App) -> gpui_kit::Div {
    head_box(true, cx).min_w_0().text_color(cx.palette().foreground.opacity(0.7)).child(gpui_kit::div().min_w_0().child(head.into()))
}

/// The head's box: 13.5 px medium, or a locked page's 14 px regular.
fn head_box(locked: bool, _: &App) -> gpui_kit::Div {
    let (size, weight) = if locked { (text::VALUE, FontWeight::NORMAL) } else { (text::SECTION_HEAD, FontWeight::MEDIUM) };
    h_flex()
        .debug_selector(|| "section-head".into())
        .min_h(size::SECTION_HEAD_FLOOR)
        .text_size(size.0)
        .when(!locked, |el| el.line_height(size.1))
        .font_weight(weight)
}

/// `GLYPH`: a lucide glyph at 16 px in the foreground at 80%.
pub fn glyph(glyph: Glyph, cx: &App) -> Icon {
    Icon::new(glyph).with_size(size::GLYPH).text_color(cx.palette().foreground.opacity(0.8))
}

/// `GLYPH_FRAME` round any mark: a glyph, an agent's logo.
pub fn framed(mark: impl IntoElement, cx: &App) -> impl IntoElement {
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
        .child(mark)
}

/// `GLYPH_FRAME` holding a `GLYPH`.
pub fn glyph_frame(g: Glyph, cx: &App) -> impl IntoElement {
    framed(glyph(g, cx), cx)
}

/// `CARD_SURFACE` holding its children with a rule between them, as a card and a grid draw it.
pub(super) fn surface(items: Vec<CardItem>, inset: Pixels, radius: Pixels, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let (rule, edge, fill) = (p.border.opacity(0.5), p.border.opacity(0.6), p.card.opacity(0.4));
    let count = items.len();
    v_flex().debug_selector(|| "card".into()).rounded(radius).border_1().border_color(edge).bg(fill).children(
        items
            .into_iter()
            .enumerate()
            .map(|(ix, item)| (item.0)(Corners { top: ix == 0, bottom: ix + 1 == count, inset, rule: (ix > 0).then_some(rule), radius })),
    )
}

/// One card: an optional head and lede, a body too large for a row, the soft surface with a rule between its
/// children, and an act under it.
#[derive(IntoElement)]
pub struct Card {
    id: SharedString,
    head: Option<SharedString>,
    lede: Option<SharedString>,
    body: Option<AnyElement>,
    items: Vec<CardItem>,
    under: Vec<AnyElement>,
    inset: Pixels,
    locked: bool,
    on_head: Option<Click>,
}

impl Card {
    /// The id keeps a row's element state apart from a same-named row in another card.
    pub fn new(id: impl Into<SharedString>) -> Self {
        Card {
            id: id.into(),
            head: None,
            lede: None,
            body: None,
            items: Vec::new(),
            under: Vec::new(),
            inset: size::CARD_INSET,
            locked: false,
            on_head: None,
        }
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

    /// A head that is a link, as the search page's group names: it brightens to the foreground under the pointer.
    pub fn on_head_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_head = Some(Box::new(f));
        self
    }

    /// A locked page's card (`data-locked`, Usage and Activity): the head at 14 px regular, the surface at 14 px
    /// radius, 16 px under the head.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// One sentence under the head, for a card whose rows need the why.
    pub fn lede(mut self, lede: impl Into<SharedString>) -> Self {
        self.lede = Some(lede.into());
        self
    }

    /// A control too large for a row, drawn under the head and over the surface: a `RowSkeleton` while rows load.
    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
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

impl RenderOnce for Card {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let locked = self.locked;
        let radius = if locked { size::DIALOG_RADIUS } else { size::CARD_RADIUS };
        // A card with nothing in it draws no surface: an empty bordered box reads as a fault.
        let surface = (!self.items.is_empty()).then(|| surface(self.items, self.inset, radius, cx));
        let lede = self.lede.filter(|l| !l.is_empty());
        let rest = p.foreground.opacity(0.7);
        let head = self.head.map(|head| match self.on_head {
            None => head_box(locked, cx).text_color(rest).child(head).into_any_element(),
            Some(click) => {
                let id = SharedString::from(format!("{}-head", self.id));
                let over = motion::hovered(id.clone(), window, cx);
                let ink = motion::ease_ink(id.clone(), "ink", if *over.read(cx) { p.foreground } else { rest }, motion::COLORS, window, cx);
                head_box(locked, cx)
                    .child(
                        div()
                            .id(id)
                            .cursor_pointer()
                            .text_color(ink)
                            .tab_index(0)
                            .focus_visible(|s| s.shadow(vec![ring(p.ring)]))
                            .on_hover(motion::track_hover(over))
                            .on_click(move |e, window, cx| click(e, window, cx))
                            .child(head),
                    )
                    .into_any_element()
            }
        });
        let heading = (head.is_some() || lede.is_some()).then(|| {
            v_flex().gap_1().children(head).children(lede.map(|lede| {
                div()
                    .debug_selector(|| "lede".into())
                    .max_w(DESCRIPTION_MAX)
                    .text_size(text::DESCRIPTION.0)
                    .line_height(text::DESCRIPTION.1)
                    .text_color(p.muted_foreground)
                    .child(lede)
            }))
        });
        v_flex()
            .id(self.id)
            .gap(if locked { px(16.) } else { px(12.) })
            .children(heading)
            .children(self.body)
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

/// `fmtBytes`: bytes in binary units, whole under a gigabyte, GB with one decimal unless whole.
pub fn fmt_bytes(n: u64) -> String {
    const K: f64 = 1024.;
    let v = n as f64;
    if v < K {
        return format!("{n} B");
    }
    if v < K * K {
        return format!("{} KB", (v / K).round());
    }
    if v < K * K * K {
        return format!("{} MB", (v / (K * K)).round());
    }
    let gb = v / (K * K * K);
    let tenth = (gb * 10.).round() / 10.;
    if tenth.fract() == 0. {
        format!("{} GB", gb.round())
    } else {
        format!("{tenth:.1} GB")
    }
}

/// `sizeTone`'s ink for a size: the danger ink from a gigabyte, the warning ink from 200 MB, the yellow from 50 MB,
/// else muted.
pub fn size_ink(bytes: u64, cx: &App) -> Hsla {
    let p = cx.palette();
    const MIB: u64 = 1024 * 1024;
    if bytes >= 1024 * MIB {
        p.error_foreground
    } else if bytes >= 200 * MIB {
        p.warning_foreground
    } else if bytes >= 50 * MIB {
        p.yellow_foreground
    } else {
        p.muted_foreground
    }
}

/// Words in the `FACT` or `VALUE` class.
pub fn word(words: impl Into<Words>, class: WordClass, cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    let (size, color) = match class {
        WordClass::Value => (text::VALUE, p.foreground),
        WordClass::Fact => (text::FACT, p.muted_foreground),
    };
    div()
        .debug_selector(|| "word".into())
        .min_w_0()
        .text_size(size.0)
        .line_height(size.1)
        .text_color(color)
        .font_features(text::tabular())
        .child(spaced(words.into(), false))
}

/// One chip of `Chips`: a fact in the mono with its own glyph, or a computer's own icon.
#[derive(Clone)]
pub struct Chip {
    text: SharedString,
    glyph: Option<ComputerIcon>,
}

impl Chip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Chip { text: text.into(), glyph: None }
    }

    pub fn glyph(mut self, glyph: impl Into<ComputerIcon>) -> Self {
        self.glyph = Some(glyph.into());
        self
    }
}

/// `Chips`: facts as a row of small tinted chips that wraps, each cut at its edge with the whole on its hover.
pub fn chips(id: impl Into<SharedString>, items: Vec<Chip>, cx: &App) -> impl IntoElement {
    let p = cx.palette().clone();
    let id: SharedString = id.into();
    let mono = cx.theme().mono_font_family.clone();
    let top = vec![BoxShadow { color: p.chip_top, offset: point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }];
    h_flex().debug_selector(|| "chips".into()).flex_wrap().gap_2().children(
        items.into_iter().filter(|c| !c.text.is_empty()).enumerate().map(move |(ix, chip)| {
            let whole = chip.text.clone();
            h_flex()
                .id(SharedString::from(format!("{id}-chip-{ix}")))
                .debug_selector(|| "chip".into())
                .relative()
                .h(px(24.))
                .max_w_full()
                .gap(px(6.))
                .px_2()
                .rounded(size::BUTTON_RADIUS)
                .border_1()
                .border_color(p.border)
                .bg(p.foreground.opacity(0.04))
                .font_family(mono.clone())
                .text_size(px(11.))
                .text_color(p.foreground.opacity(0.75))
                .tooltip(tip(whole))
                .children(chip.glyph.map(|g| g.draw(p.muted_foreground, px(12.))))
                .child(div().min_w_0().truncate().child(chip.text))
                .child(div().absolute().inset_0().rounded(size::BUTTON_RADIUS - px(1.)).shadow(top.clone()))
        }),
    )
}

/// One row: the title over its sentence, and beside them the slot with at most one word and one control.
#[derive(IntoElement)]
pub struct Row {
    id: SharedString,
    title: SharedString,
    lead: Option<AnyElement>,
    mark: Option<(AnyElement, bool)>,
    note: Option<Words>,
    chips: Option<Vec<Chip>>,
    mono: bool,
    clip: bool,
    word: Option<(Words, WordClass)>,
    control: Option<AnyElement>,
    open: Option<Click>,
    reset: Option<Click>,
    warning: bool,
    take: Option<Take>,
    corners: Corners,
}

impl Row {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Row {
            id: id.into(),
            title: title.into(),
            lead: None,
            mark: None,
            note: None,
            chips: None,
            mono: false,
            clip: false,
            word: None,
            control: None,
            open: None,
            reset: None,
            warning: false,
            take: None,
            corners: Corners::default(),
        }
    }

    /// What stands before the title: a glyph in its frame (`glyph_frame`), a project's own mark.
    pub fn lead(mut self, lead: impl IntoElement) -> Self {
        self.lead = Some(lead.into_any_element());
        self
    }

    /// After the title, in the `FACT` class: the default mark on a computer, the marks of the agents a thing is set
    /// up for.
    pub fn mark(mut self, mark: impl IntoElement) -> Self {
        self.mark = Some((mark.into_any_element(), false));
        self
    }

    /// The mark in the row's own sans note ink, where it is a word and not a figure.
    pub fn mark_word(mut self, mark: impl IntoElement) -> Self {
        self.mark = Some((mark.into_any_element(), true));
        self
    }

    /// One sentence, or facts held apart by space: `.note(["8 cores", "16 GB"])`.
    pub fn note(mut self, note: impl Into<Words>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// The facts as chips in place of the sentence.
    pub fn chips(mut self, chips: Vec<Chip>) -> Self {
        self.chips = Some(chips);
        self
    }

    /// The sentence is machine words: the `FACT` class at the note's leading, broken anywhere.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// The sentence on one line, cut at the row's edge with the whole of it on the hover: a command or a path.
    pub fn clip(mut self) -> Self {
        self.clip = true;
        self
    }

    pub fn value(mut self, value: impl Into<Words>) -> Self {
        self.word = Some((value.into(), WordClass::Value));
        self
    }

    pub fn fact(mut self, fact: impl Into<Words>) -> Self {
        self.word = Some((fact.into(), WordClass::Fact));
        self
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }

    /// A row that opens a page: the whole row is the button and the slot ends in the chevron. A press on its control
    /// stays the control's.
    pub fn on_open(mut self, open: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.open = Some(Box::new(open));
        self
    }

    /// Set only while the row is off its default: the arrow beside the title puts that one row back.
    pub fn on_reset(mut self, reset: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.reset = Some(Box::new(reset));
        self
    }

    /// A title that warns: a computer that runs an older wsp.
    pub fn warning(mut self) -> Self {
        self.warning = true;
        self
    }

    /// A row that takes a file dropped on it, or pasted while it or its control holds focus; its ground turns to the
    /// accent while one is held over it.
    pub fn on_take(mut self, take: impl Fn(Taken, &mut Window, &mut App) + 'static) -> Self {
        self.take = Some(std::rc::Rc::new(take));
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

/// `OPENS_CLASS`: a row that opens a page takes the accent at 60% under the pointer over 150 ms, Tab reaches it, and
/// its focus ring stands inside its edge.
pub(super) fn opens(
    frame: gpui_kit::Stateful<gpui_kit::Div>,
    id: &SharedString,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (ring_ink, accent) = (cx.palette().ring, cx.palette().accent);
    let over = motion::hovered(id.clone(), window, cx);
    let ground = if *over.read(cx) { accent.opacity(0.6) } else { accent.opacity(0.) };
    let ground = motion::ease_ink(id.clone(), "ground", ground, motion::COLORS, window, cx);
    frame
        .cursor_pointer()
        .bg(ground)
        .tab_index(0)
        .focus_visible(move |s| s.shadow(vec![ring_inset(ring_ink)]))
        .on_hover(motion::track_hover(over))
}

impl RenderOnce for Row {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let title_color = if self.warning { p.warning_foreground } else { p.foreground };
        let mark = self.mark.map(|(mark, as_word)| {
            let line = if as_word { text::NOTE.1 } else { FACT_LINE };
            div()
                .debug_selector(|| "mark".into())
                .flex()
                .flex_none()
                .items_center()
                .text_size(text::FACT.0)
                .line_height(line)
                .text_color(p.muted_foreground)
                .when(!as_word, |el| el.font_features(text::tabular()))
                .child(mark)
        });
        let reset = self.reset.map(|reset| {
            Button::icon_xs(SharedString::from(format!("{}-reset", self.id)), Glyph::RotateCcw)
                .tooltip(RESET_WORD)
                .on_click(move |e, window, cx| reset(e, window, cx))
        });
        let (mono, clip) = (self.mono, self.clip);
        let description = match self.chips {
            Some(items) => Some(div().mt(px(6.)).child(chips(format!("{}-chips", self.id), items, cx)).into_any_element()),
            None => self.note.filter(|n| !n.is_empty()).map(|note| {
                let whole = note.line();
                div()
                    .id(SharedString::from(format!("{}-note", self.id)))
                    .debug_selector(|| "note".into())
                    .text_size(text::DESCRIPTION.0)
                    .line_height(text::DESCRIPTION.1)
                    .text_color(p.muted_foreground)
                    .when(!mono, |el| el.max_w(DESCRIPTION_MAX))
                    .when(clip, |el| el.truncate().tooltip(tip(whole)))
                    .child(spaced(note, clip))
                    .into_any_element()
            }),
        };
        let words = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(h_flex().min_h(px(20.)).flex_wrap().gap_x_2().child(title(self.title, title_color)).children(mark).children(reset))
            .children(description);
        let opening = self.open.is_some();
        let chevron_only = opening && self.word.is_none() && self.control.is_none();
        // Under 640 px the slot stands under the words, and under a lead's words rather than its glyph; a chevron
        // alone stays beside them, since on a line of its own it reads as a stray.
        let stacked = super::narrow::width(window).sm && !chevron_only;
        let led = self.lead.is_some();
        let text = h_flex()
            .when(!stacked, |el| el.flex_1())
            .min_w_0()
            .gap_3()
            .children(self.lead.map(|lead| div().flex().flex_none().child(lead)))
            .child(words);
        let slot = (self.word.is_some() || self.control.is_some() || opening).then(|| {
            h_flex()
                .debug_selector(|| "slot".into())
                .when(!stacked, |el| el.flex_none().justify_end().when(!chevron_only, |el| el.min_w(SLOT_FLOOR)))
                .when(stacked, |el| el.min_w_0().when(led, |el| el.pl(LED_SLOT_INDENT)))
                .gap_3()
                .children(self.word.map(|(text, class)| word(text, class, cx).when(!stacked, |el| el.text_right())))
                // A row that opens a page leaves a press on its control to the control, as rows.tsx's does.
                .children(
                    self.control.map(|control| div().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(control)),
                )
                .when(opening, |el| el.child(Icon::new(Glyph::ChevronRight).with_size(px(14.)).text_color(p.muted_foreground)))
        });
        let body = if stacked { v_flex().gap_3().child(text).children(slot) } else { h_flex().gap_5().child(text).children(slot) };
        let mut frame = row_frame(&self.id, size::ROW_FLOOR, self.corners);
        if let Some(take) = self.take {
            let (dropped, pasted, accent) = (take.clone(), take, p.accent);
            frame = frame
                .focusable()
                .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(accent))
                .on_drop(move |paths: &ExternalPaths, window, cx| dropped(Taken::Dropped(paths), window, cx))
                .on_key_down(move |event, window, cx| {
                    if is_paste(event) {
                        pasted(Taken::Pasted, window, cx);
                        cx.stop_propagation();
                    }
                });
        }
        match self.open {
            Some(open) => opens(frame, &self.id, window, cx).on_click(move |e, window, cx| open(e, window, cx)).child(body),
            None => frame.child(body),
        }
    }
}

/// `HeadRow`: the row a computer's or a project's page opens on, in place of a page title. Its mark in the glyph
/// frame, its name, one line of facts under it, and a figure or an act at the right.
#[derive(IntoElement)]
pub struct HeadRow {
    id: SharedString,
    glyph: AnyElement,
    title: SharedString,
    mark: Option<AnyElement>,
    line: Option<AnyElement>,
    slot: Option<AnyElement>,
    corners: Corners,
}

impl HeadRow {
    /// `glyph` stands in the frame: `ui::glyph(Glyph::Monitor, cx)`, or an agent's own mark.
    pub fn new(id: impl Into<SharedString>, glyph: impl IntoElement, title: impl Into<SharedString>) -> Self {
        HeadRow {
            id: id.into(),
            glyph: glyph.into_any_element(),
            title: title.into(),
            mark: None,
            line: None,
            slot: None,
            corners: Corners::default(),
        }
    }

    /// One fact after the name in the `FACT` class: an agent's version, a computer's `VersionFact`.
    pub fn mark(mut self, mark: impl IntoElement) -> Self {
        self.mark = Some(mark.into_any_element());
        self
    }

    /// The one line of facts under the name, in the note.
    pub fn line(mut self, line: impl IntoElement) -> Self {
        self.line = Some(line.into_any_element());
        self
    }

    pub fn slot(mut self, slot: impl IntoElement) -> Self {
        self.slot = Some(slot.into_any_element());
        self
    }
}

impl From<HeadRow> for CardItem {
    fn from(row: HeadRow) -> Self {
        CardItem::placed(move |corners| HeadRow { corners, ..row }.into_any_element())
    }
}

impl RenderOnce for HeadRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let name = h_flex()
            .min_w_0()
            .items_start()
            .gap_2()
            .child(
                div()
                    .debug_selector(|| "title".into())
                    .min_w_0()
                    .truncate()
                    .text_size(text::LIST_TITLE.0)
                    .line_height(text::LIST_TITLE.1)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .child(self.title),
            )
            .children(self.mark.map(|mark| {
                div()
                    .debug_selector(|| "mark".into())
                    .flex_none()
                    .line_height(text::LIST_TITLE.1)
                    .text_size(text::FACT.0)
                    .text_color(p.muted_foreground)
                    .font_features(text::tabular())
                    .child(mark)
            }));
        let words = v_flex().min_w_0().gap(px(2.)).child(name).children(self.line.map(|line| {
            div()
                .debug_selector(|| "note".into())
                .text_size(text::NOTE.0)
                .line_height(text::NOTE.1)
                .text_color(p.muted_foreground)
                .child(line)
        }));
        self.corners.apply(
            h_flex()
                .id(self.id)
                .debug_selector(|| "head-row".into())
                .flex_wrap()
                .justify_between()
                .gap_x_6()
                .gap_y_2()
                .py_4()
                .px(self.corners.inset)
                .min_h(size::ROW_FLOOR)
                .child(h_flex().min_w(px(160.)).flex_1().gap_3().child(framed(self.glyph, cx)).child(words))
                .children(self.slot),
        )
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
    cap_box(cx).debug_selector(|| "keycap".into()).bg(p.muted).text_color(p.muted_foreground).child(key)
}

/// `Kbd` as `CommandFooter` restyles it: the foreground at 8% under the foreground's own ink, in regular weight; `key`
/// a word or a glyph.
pub fn footer_keycap(key: impl IntoElement, cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    cap_box(cx).flex_none().font_weight(FontWeight::NORMAL).bg(p.foreground.opacity(0.08)).text_color(p.foreground).child(key)
}

fn cap_box(cx: &App) -> gpui_kit::Div {
    h_flex()
        .h(px(20.))
        .min_w(px(20.))
        .px_1()
        .justify_center()
        .rounded(px(6.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(text::XS.0)
        .line_height(text::XS.1)
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

/// One line: the label, and at its right one word, a chord's keycaps or a control. Its sentence is its hover, never
/// a note.
#[derive(IntoElement)]
pub struct Line {
    id: SharedString,
    label: SharedString,
    right: Option<AnyElement>,
    value: Option<(Words, WordClass)>,
    keys: Option<KeyCaps>,
    hover: Option<SharedString>,
    empty: bool,
    corners: Corners,
}

impl Line {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Line {
            id: id.into(),
            label: label.into(),
            right: None,
            value: None,
            keys: None,
            hover: None,
            empty: false,
            corners: Corners::default(),
        }
    }

    pub fn value(mut self, value: impl Into<Words>) -> Self {
        self.value = Some((value.into(), WordClass::Value));
        self
    }

    /// The value in the `FACT` class, a state rather than a figure.
    pub fn fact(mut self, fact: impl Into<Words>) -> Self {
        self.value = Some((fact.into(), WordClass::Fact));
        self
    }

    pub fn keys(mut self, keys: KeyCaps) -> Self {
        self.keys = Some(keys);
        self
    }

    /// What stands at the right in the value's or the keycaps' place: a mark, a spinner, a control.
    pub fn right(mut self, el: impl IntoElement) -> Self {
        self.right = Some(el.into_any_element());
        self
    }

    /// The one sentence the line has to say, on its hover.
    pub fn hover(mut self, hover: impl Into<SharedString>) -> Self {
        self.hover = Some(hover.into());
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
            .or_else(|| self.value.map(|(text, class)| word(text, class, cx).text_right().into_any_element()))
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
        row_frame(&self.id, size::LINE_FLOOR, self.corners).when_some(self.hover, |el, hover| el.tooltip(tip(hover))).child(body)
    }
}

#[cfg(test)]
mod bytes_tests {
    use super::fmt_bytes;

    #[test]
    fn bytes_read_as_the_protocol_writes_them() {
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(1536), "2 KB");
        assert_eq!(fmt_bytes(3 * 1024 * 1024), "3 MB");
        assert_eq!(fmt_bytes(1536 * 1024 * 1024), "1.5 GB");
        assert_eq!(fmt_bytes(2 * 1024 * 1024 * 1024), "2 GB");
    }
}
