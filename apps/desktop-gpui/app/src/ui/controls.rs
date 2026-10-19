// SPDX-License-Identifier: AGPL-3.0-only
//! The controls a row holds and the acts a view takes: the select and the field at the row's size, the picks (a radio
//! or a checkbox at the left of a row), and the tactile keycap buttons.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    div, point, prelude::*, px, AnyElement, App, BoxShadow, ClickEvent, DefiniteLength, Div, ElementId, Entity, FocusHandle,
    Focusable as _, FontWeight, Hsla, MouseButton, Pixels, SharedString, TooltipSide, Window,
};

use super::card::{framed, glyph_frame, row_frame, word, CardItem, Corners, WordClass};
use super::dialog::tip;
use super::focus::ring;
use super::icons::{Glyph, TICK};
use super::motion;
use super::theme::{ActivePalette, Palette};
use super::{size, text};

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type SharedClick = std::rc::Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A held control waits on something beside it: dimmed in its own look, and nothing else.
const HELD: f32 = 0.64;

/// `PickRow`'s slot under 640 px, `max-sm:pl-[72px]`: past the checkbox, the glyph frame and their gaps.
const PICK_SLOT_INDENT: Pixels = px(72.);

/// How a held control's parts read: the web dims the control as one layer, but GPUI fades each fill, edge and glyph
/// on its own, which stacks them; so each part is drawn solid in the color the dimmed layer shows over the page.
pub(crate) struct Dim {
    page: Hsla,
    ground: Hsla,
}

impl Dim {
    pub(crate) fn new(page: Hsla, fill: Hsla) -> Self {
        Dim { page, ground: page.blend(fill) }
    }

    pub(crate) fn fill(&self) -> Hsla {
        self.page.blend(self.ground.opacity(HELD))
    }

    /// A part drawn over the control's fill.
    pub(crate) fn over(&self, part: Hsla) -> Hsla {
        self.page.blend(self.ground.blend(part).opacity(HELD))
    }
}

/// `KEYCAP_BEVEL`, `inset 0 1px 0 var(--keycap-top)`: the 1 px light along the inside of the top edge.
pub fn bevel(p: &Palette) -> Vec<BoxShadow> {
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
    /// The ghost for an act that does not come back: the danger ink under the pointer.
    GhostDanger,
    /// A quiet word with no padding, so its text edge is its box edge.
    Text,
    /// The act that does not come back, red at rest: a destructive dialog's confirm.
    Destructive,
    /// `DANGER_BUTTON`: the outline door to a destructive dialog, neutral at rest, the danger edge and ink under the
    /// pointer.
    Danger,
}

#[derive(Clone, Copy, PartialEq)]
enum ButtonSize {
    Default,
    Xs,
    /// The ghost icon button, 28 px square.
    Icon,
    /// 24 px square, the copy row's button.
    IconXs,
    /// The setup keycap: 40 px tall, 15 px words, 22 px sides, a 10 px radius (`desktop:onboarding.html` `.primary`).
    Keycap,
}

/// `Button`: the keycap, in its kind and size. A held button keeps its look at 64% and takes no click.
#[derive(IntoElement)]
pub struct Button {
    id: SharedString,
    kind: ButtonKind,
    size: ButtonSize,
    label: Option<SharedString>,
    glyph: Option<Glyph>,
    /// A mark of its own before the label, as an editor's in Open.
    lead: Option<AnyElement>,
    /// An svg of the app's own drawn where a glyph would be, for a mark gpui-kit's bundle lacks.
    path: Option<&'static str>,
    /// A 12 px glyph after the label: a link's external mark.
    after: Option<Glyph>,
    held: bool,
    busy: bool,
    neutral_ring: bool,
    tooltip: Option<SharedString>,
    tooltip_side: TooltipSide,
    /// A height other than the size's own: a toolbar's Add stands at 30 px (`h-[30px]`).
    height: Option<Pixels>,
    /// An icon button's side other than its size's: a row menu's 20 px trigger.
    side: Option<Pixels>,
    /// Corners other than the size's radius all round: a stepper's ends are square where they meet its number.
    corners: Option<gpui_kit::Corners<Pixels>>,
    /// The glyph after the label, as a menu trigger's chevron.
    glyph_after: bool,
    /// No keycap bevel (`shadow-none`).
    flat: bool,
    /// A handle of the caller's for its tab stop, so the caller can tell when it has the focus.
    focus: Option<FocusHandle>,
    on_click: Option<Click>,
}

impl Button {
    fn new(id: impl Into<SharedString>, kind: ButtonKind, label: impl Into<SharedString>) -> Self {
        Button {
            id: id.into(),
            kind,
            size: ButtonSize::Default,
            label: Some(label.into()).filter(|l: &SharedString| !l.is_empty()),
            glyph: None,
            lead: None,
            path: None,
            after: None,
            held: false,
            busy: false,
            neutral_ring: false,
            tooltip: None,
            tooltip_side: TooltipSide::Top,
            height: None,
            side: None,
            corners: None,
            glyph_after: false,
            flat: false,
            focus: None,
            on_click: None,
        }
    }

    pub fn primary(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button::new(id, ButtonKind::Primary, label)
    }

    pub fn outline(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button::new(id, ButtonKind::Outline, label)
    }

    /// No edge and the muted ink, the accent fill under the pointer: Restore defaults, `.xs().glyph(Glyph::RotateCcw)`.
    pub fn ghost(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button::new(id, ButtonKind::Ghost, label)
    }

    /// A quiet link to an act that does not come back, as Cancel the build: neutral at rest, the danger ink under the
    /// pointer.
    pub fn ghost_danger(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button::new(id, ButtonKind::GhostDanger, label)
    }

    /// The red confirm of a destructive dialog: Remove, Revoke, Delete.
    pub fn destructive(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button::new(id, ButtonKind::Destructive, label)
    }

    /// The outline door to a destructive dialog (`DANGER_BUTTON`); the red stands in the dialog, not the row.
    pub fn danger(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Button::new(id, ButtonKind::Danger, label)
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
            lead: None,
            path: None,
            after: None,
            held: false,
            busy: false,
            neutral_ring: false,
            tooltip: None,
            tooltip_side: TooltipSide::Top,
            height: None,
            side: None,
            corners: None,
            glyph_after: false,
            flat: false,
            focus: None,
            on_click: None,
        }
    }

    /// The 24 px ghost icon button.
    pub fn icon_xs(id: impl Into<SharedString>, glyph: Glyph) -> Self {
        Button { size: ButtonSize::IconXs, ..Button::icon(id, glyph) }
    }

    /// The setup keycap's size, a first launch's one press.
    pub fn keycap(mut self) -> Self {
        self.size = ButtonSize::Keycap;
        self
    }

    /// The xs size, for an inline act in a row.
    pub fn xs(mut self) -> Self {
        self.size = ButtonSize::Xs;
        self
    }

    /// The 24 px ghost icon button around one of the app's own svgs, such as `SMILE_PLUS`.
    pub fn icon_xs_path(id: impl Into<SharedString>, path: &'static str) -> Self {
        Button { glyph: None, path: Some(path), ..Button::icon_xs(id, Glyph::Plus) }
    }

    pub fn glyph(mut self, glyph: Glyph) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// A 12 px glyph after the label, as the skills.sh link's external mark (`size-3` after the word).
    pub fn after(mut self, glyph: Glyph) -> Self {
        self.after = Some(glyph);
        self
    }

    /// Its glyph alone at its size's padding, the label gone as `max-sm:hidden` takes it: Restore defaults under 640 px.
    pub fn glyph_only(mut self, only: bool) -> Self {
        if only {
            self.label = None;
        }
        self
    }

    pub fn lead(mut self, lead: impl IntoElement) -> Self {
        self.lead = Some(lead.into_any_element());
        self
    }

    /// A height other than the size's own; a square keeps its size's width, as CSS's `size-6 h-full`.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// An icon button's side other than its size's, as the row menu's `size-5`.
    pub fn side(mut self, side: Pixels) -> Self {
        self.side = Some(side);
        self
    }

    pub fn corners(mut self, corners: gpui_kit::Corners<Pixels>) -> Self {
        self.corners = Some(corners);
        self
    }

    /// The outline keycap's edge, fill and bevel on an icon button.
    pub fn outlined(mut self) -> Self {
        self.kind = ButtonKind::Outline;
        self
    }

    /// The glyph after the label, as `Merge` with its menu's chevron.
    pub fn glyph_after(mut self) -> Self {
        self.glyph_after = true;
        self
    }

    /// No keycap bevel, as the PR box's merge (`shadow-none`).
    pub fn flat(mut self) -> Self {
        self.flat = true;
        self
    }

    /// Its tab stop on `handle`, so a page can tell when the button has the focus.
    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    /// Its road is running: held, a labelled button's glyph gives way to the spinner and an icon button's own glyph
    /// turns, as the web's `Spinner` in an act and the head's turning refresh do.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// `NEUTRAL_RING`: the muted focus ring, for the Cancel standing beside a destructive confirm.
    pub fn neutral_ring(mut self) -> Self {
        self.neutral_ring = true;
        self
    }

    /// The word or sentence on its hover: an icon button's name.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// The side its tooltip opens on where the web's says one other than above: `side="bottom"`.
    pub fn tooltip_side(mut self, side: TooltipSide) -> Self {
        self.tooltip_side = side;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let held = self.held || self.busy;
        let transparent = gpui_kit::transparent_black();
        // At rest, then under the pointer: fill, edge, ink.
        let (rest, over, glyph_ink) = match self.kind {
            ButtonKind::Primary => {
                ((p.primary, p.primary, p.primary_foreground), (p.primary_hover, p.primary, p.primary_foreground), p.primary_foreground)
            }
            ButtonKind::Outline => ((p.input_fill, p.input, p.foreground), (p.input_fill_hover, p.input, p.foreground), p.muted_foreground),
            ButtonKind::Danger => (
                (p.input_fill, p.input, p.foreground),
                (p.input_fill_hover, p.destructive.opacity(0.5), p.error_foreground),
                p.muted_foreground,
            ),
            ButtonKind::Destructive => (
                (p.destructive, p.destructive, p.on_destructive),
                (p.destructive.opacity(0.9), p.destructive, p.on_destructive),
                p.on_destructive,
            ),
            ButtonKind::Ghost => {
                ((transparent, transparent, p.muted_foreground), (p.accent, transparent, p.foreground), p.muted_foreground)
            }
            ButtonKind::GhostDanger => {
                ((transparent, transparent, p.muted_foreground), (p.accent, transparent, p.error_foreground), p.muted_foreground)
            }
            ButtonKind::Text => {
                ((transparent, transparent, p.muted_foreground), (transparent, transparent, p.foreground), p.muted_foreground)
            }
        };
        let (side, pad, (font, line), gap, glyph_size) = match self.size {
            ButtonSize::Default => (px(32.), px(12.), (text::BUTTON, px(19.5)), px(6.), px(14.)),
            ButtonSize::Xs => (px(24.), px(8.), text::XS, px(4.), px(14.)),
            ButtonSize::Icon => (px(28.), px(0.), (text::BUTTON, px(19.5)), px(0.), px(16.)),
            ButtonSize::IconXs => (px(24.), px(0.), (text::BUTTON, px(19.5)), px(0.), px(14.)),
            ButtonSize::Keycap => (px(40.), px(22.), (px(15.), px(22.5)), px(8.), px(16.)),
        };
        let radius = if self.size == ButtonSize::Keycap { px(10.) } else { size::BUTTON_RADIUS };
        let side = self.side.unwrap_or(side);
        let height = self.height.unwrap_or(side);
        let hovered = motion::hovered(self.id.clone(), window, cx);
        let (fill, edge, ink) = if *hovered.read(cx) && !held { over } else { rest };
        let mut keycap = bevel(&p);
        let (fill, edge, ink, glyph_ink) = if held {
            let dim = Dim::new(p.background, fill);
            keycap[0].color = dim.over(p.keycap_top);
            let edge = if edge.a > 0. { dim.over(edge) } else { edge };
            let fill = if fill.a > 0. { dim.fill() } else { fill };
            (fill, edge, dim.over(ink), dim.over(glyph_ink))
        } else {
            let id = self.id.clone();
            let fill = motion::ease_ink(id.clone(), "fill", fill, motion::COLORS, window, cx);
            let edge = motion::ease_ink(id.clone(), "edge", edge, motion::COLORS, window, cx);
            (fill, edge, motion::ease_ink(id, "ink", ink, motion::COLORS, window, cx), glyph_ink)
        };
        let square = matches!(self.size, ButtonSize::Icon | ButtonSize::IconXs);
        let pad = if self.kind == ButtonKind::Text { px(0.) } else { pad };
        let weight = if matches!(self.kind, ButtonKind::Ghost | ButtonKind::GhostDanger | ButtonKind::Text) {
            FontWeight::NORMAL
        } else {
            FontWeight::MEDIUM
        };
        // `NEUTRAL_RING`: a blue ring between a neutral button and a red one says nothing about either.
        let ring_ink = if self.kind == ButtonKind::Destructive || self.neutral_ring { p.muted_foreground } else { p.ring };
        h_flex()
            .id(self.id)
            .debug_selector(|| "button".into())
            .flex_none()
            .justify_center()
            .h(height)
            .when(square, |el| el.w(side))
            .px(pad)
            .gap(gap)
            .map(|el| match self.corners {
                Some(c) => el.rounded_tl(c.top_left).rounded_tr(c.top_right).rounded_br(c.bottom_right).rounded_bl(c.bottom_left),
                None => el.rounded(radius),
            })
            .border_1()
            .border_color(edge)
            .bg(fill)
            // GPUI paints an inset shadow under the border, where CSS keeps it inside; a child filling the padding box holds it there.
            .when(!self.flat && !matches!(self.kind, ButtonKind::Ghost | ButtonKind::GhostDanger | ButtonKind::Text), |el| {
                el.child(div().absolute().inset_0().rounded(radius - px(1.)).shadow(keycap))
            })
            .text_size(font)
            .line_height(line)
            .font_weight(weight)
            .text_color(ink)
            .when(self.glyph_after, |el| el.flex_row_reverse())
            .children(self.lead)
            .children(match (self.busy, self.glyph) {
                (true, glyph) => {
                    let glyph = if self.label.is_some() { Glyph::LoaderCircle } else { glyph.unwrap_or(Glyph::LoaderCircle) };
                    Some(super::status::turning(Icon::new(glyph).with_size(glyph_size).text_color(glyph_ink), window, cx))
                }
                (false, glyph) => glyph
                    .map(Icon::new)
                    .or(self.path.map(|p| Icon::empty().path(p)))
                    .map(|icon| icon.with_size(glyph_size).text_color(glyph_ink)),
            })
            .children(self.label)
            .children(self.after.map(|g| {
                div().debug_selector(|| "button-after".into()).flex().child(Icon::new(g).with_size(px(12.)).text_color(glyph_ink))
            }))
            .map(|el| {
                if held {
                    return el;
                }
                el.cursor_pointer()
                    .when_some(self.focus, |el, focus| el.track_focus(&focus))
                    .tab_index(0)
                    .focus_visible(|s| s.shadow(vec![ring(ring_ink)]))
                    .on_hover(motion::track_hover(hovered))
                    .when_some(self.tooltip, |el, t| el.tooltip(tip(t)).tooltip_side(self.tooltip_side))
                    .when_some(self.on_click, |el, f| el.on_click(move |e, window, cx| f(e, window, cx)))
            })
    }
}

/// The radio of a `Choice`: 16 px round on the field's edge, and once picked the primary fill with its 6 px dot; held,
/// at 64% with no shadow.
pub fn radio(picked: bool, held: bool, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let dark = cx.theme().is_dark();
    let ground = if dark { p.input.opacity(0.32) } else { p.background };
    div()
        .debug_selector(|| "radio".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded_full()
        .when(held, |el| el.opacity(0.64))
        .map(|el| {
            if picked {
                el.bg(p.primary).child(div().size(px(6.)).rounded_full().bg(p.primary_foreground))
            } else {
                el.border_1().border_color(p.input).bg(ground).when(!held, |el| el.shadow(p.control_shadow.clone()))
            }
        })
}

/// `Checkbox`: 16 px with a 4 px radius; ticked, the primary fill and its tick; held, its edge dashed and a ticked
/// one's tick muted on no fill. Its lift, there only while open and unticked, eases as `transition-shadow` does.
pub fn checkbox(id: impl Into<ElementId>, checked: bool, held: bool, window: &mut Window, cx: &mut App) -> Div {
    let p = cx.palette();
    let ticked = (p.primary, p.primary_foreground);
    tick_box(id, checked, held, ticked, window, cx)
}

/// `Checkbox tone="neutral"`, for a card whose one accent is its act: ticked, the foreground fill and the page's ink.
pub fn neutral_checkbox(id: impl Into<ElementId>, checked: bool, window: &mut Window, cx: &mut App) -> Div {
    let p = cx.palette();
    let ticked = (p.foreground, p.background);
    tick_box(id, checked, false, ticked, window, cx)
}

#[cfg(test)]
thread_local! {
    /// The fill the last ticked box drew with, in this test thread.
    pub static TICKED_FILL: std::cell::Cell<Option<Hsla>> = const { std::cell::Cell::new(None) };
}

fn tick_box(id: impl Into<ElementId>, checked: bool, held: bool, ticked: (Hsla, Hsla), window: &mut Window, cx: &mut App) -> Div {
    #[cfg(test)]
    if checked && !held {
        TICKED_FILL.with(|f| f.set(Some(ticked.0)));
    }
    let p = cx.palette().clone();
    let ground = if cx.theme().is_dark() { p.input.opacity(0.32) } else { p.background };
    let lift = motion::ease_px(id, "lift", px(if checked || held { 0. } else { 1. }), motion::COLORS, window, cx);
    let lift: f32 = lift.into();
    let shadow: Vec<BoxShadow> = p.control_shadow.iter().map(|s| BoxShadow { color: s.color.opacity(lift), ..s.clone() }).collect();
    let tick = |ink: Hsla| Icon::empty().path(TICK).with_size(px(12.)).text_color(ink);
    div()
        .debug_selector(|| "checkbox".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded(px(4.))
        .shadow(shadow)
        .map(|el| match (checked, held) {
            (true, false) => el.bg(ticked.0).child(tick(ticked.1)),
            (true, true) => el.border_1().border_dashed().border_color(p.input).bg(p.background).child(tick(p.muted_foreground)),
            (false, false) => el.border_1().border_color(p.input).bg(ground),
            (false, true) => el.border_1().border_dashed().border_color(p.input).bg(ground),
        })
}

/// The checkbox in `desktop:onboarding.html`'s own look: the page's ground on both sides and no shadow, and one that
/// cannot be ticked at 64%.
pub fn checkbox_plain(checked: bool, disabled: bool, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    div().debug_selector(|| "checkbox".into()).flex().flex_none().items_center().justify_center().size(px(16.)).rounded(px(4.)).map(|el| {
        let el = if disabled { el.opacity(0.64) } else { el };
        if checked {
            el.bg(p.primary).child(Icon::empty().path(TICK).with_size(px(12.)).text_color(p.primary_foreground))
        } else {
            el.border_1().border_color(p.input).bg(p.background)
        }
    })
}

/// The words of a pick: the name in `LIST_TITLE`, muted while not picked, a tag and marks after it, and its note under
/// it.
fn pick_words(
    name: SharedString,
    title: (Pixels, Pixels),
    tag: Option<SharedString>,
    marks: Option<AnyElement>,
    note: Option<SharedString>,
    lit: bool,
    cx: &App,
) -> impl IntoElement {
    let p = cx.palette();
    let ink: Hsla = if lit { p.foreground } else { p.muted_foreground };
    v_flex()
        .flex_1()
        .min_w_0()
        .child(
            h_flex()
                .flex_wrap()
                .items_baseline()
                .gap_x_2()
                .child(
                    div()
                        .debug_selector(|| "title".into())
                        .text_size(title.0)
                        .line_height(title.1)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ink)
                        .child(name),
                )
                .children(tag.map(|tag| {
                    div()
                        .debug_selector(|| "pick-tag".into())
                        .flex_none()
                        .text_size(text::FACT.0)
                        .text_color(p.muted_foreground)
                        .font_features(text::tabular())
                        .child(tag)
                }))
                // The web's marks sit on the name's baseline by their bottom edge: 3.5 px down a 24 px line of 15 px type.
                .children(marks.map(|marks| div().flex_none().self_start().mt(px(3.5)).child(marks))),
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
    /// A mark of the thing's own in the frame in place of a glyph: an agent's logo.
    framed: Option<AnyElement>,
    name: SharedString,
    tag: Option<SharedString>,
    marks: Option<AnyElement>,
    note: Option<SharedString>,
    /// A quiet fact after the words, in the `FACT` class: a choice's cheapest rate.
    fact: Option<SharedString>,
    slot: Option<AnyElement>,
    /// The name's size and line: `LIST_TITLE`, or a dock's `SETTING_TITLE`.
    title: (Pixels, Pixels),
    on_click: Option<SharedClick>,
    corners: Corners,
    /// The one sentence on the row's hover: the keys that tick it.
    hover: Option<SharedString>,
    /// The name stays in the foreground while not picked: a list where every row is an answer to read.
    full_ink: bool,
    /// A tick the host fixed: drawn held, and the row takes no pointer of its own.
    locked: bool,
    /// A choice's slot under its words across the row, as the web's dock row stands under 640 px; how far it sits in.
    stacked: Option<Pixels>,
}

impl Pick {
    fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, on: bool) -> Self {
        Pick {
            id: id.into(),
            on,
            glyph: None,
            framed: None,
            name: name.into(),
            tag: None,
            marks: None,
            note: None,
            fact: None,
            slot: None,
            title: text::LIST_TITLE,
            on_click: None,
            corners: Corners::default(),
            hover: None,
            full_ink: false,
            locked: false,
            stacked: None,
        }
    }

    /// The mark is the tab stop and wears the ring, as the web's radio and checkbox do; the whole row takes a press.
    /// `stacked` sets the slot under the words, in from the row's edge by that much.
    fn render(self, mark: impl IntoElement, radius: Pixels, floor: Pixels, stacked: Option<Pixels>, cx: &App) -> AnyElement {
        let ring_ink = cx.palette().ring;
        let words = pick_words(self.name, self.title, self.tag, self.marks, self.note, self.on || self.full_ink, cx);
        let keyed = self.on_click.clone();
        let mark = div()
            .id(SharedString::from(format!("{}-mark", self.id)))
            .flex_none()
            .rounded(radius)
            .tab_index(0)
            .focus_visible(move |s| s.shadow(vec![ring(ring_ink)]))
            .when_some(keyed, |el, f| {
                el.on_click(move |e, window, cx| {
                    cx.stop_propagation();
                    f(e, window, cx)
                })
            })
            .child(mark);
        let frame = match (self.framed, self.glyph) {
            (Some(own), _) => Some(framed(own, cx).into_any_element()),
            (None, Some(g)) => Some(glyph_frame(g, cx).into_any_element()),
            (None, None) => None,
        };
        // The web's row is a two-column grid whose 20 px gap stands even when its slot column is empty.
        let gap = if self.slot.is_none() { px(20.) } else { px(0.) };
        let fact_id = format!("{}-fact", self.id);
        let fact = self.fact.map(|f| word(f, WordClass::Fact, cx).flex_none().mr(gap).debug_selector(move || fact_id));
        let lead =
            h_flex().when(stacked.is_none(), |el| el.flex_1()).min_w_0().gap_3().child(mark).children(frame).child(words).children(fact);
        // A press on the slot's fact or control is not a tick, as the web's slot prevents its label's click.
        let slot = self.slot.map(|slot| {
            h_flex()
                .debug_selector(|| "pick-slot".into())
                .min_w_0()
                .map(|el| match stacked {
                    Some(indent) => el.pl(indent),
                    None => el.justify_end(),
                })
                .gap_3()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(slot)
        });
        let body =
            if stacked.is_some() { v_flex().gap_3().child(lead).children(slot) } else { h_flex().gap_5().child(lead).children(slot) };
        row_frame(&self.id, floor, self.corners)
            .when(!self.locked, |el| el.cursor_pointer())
            .when_some(self.hover, |el, hover| el.tooltip(tip(hover)))
            .when_some(self.on_click, |el, f| el.on_click(move |e, window, cx| f(e, window, cx)))
            .child(body)
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
                self.0.on_click = Some(std::rc::Rc::new(f));
                self
            }

            /// The name's size and line where the list is not a settings list's: a dock's `SETTING_TITLE`.
            pub fn title(mut self, title: (Pixels, Pixels)) -> Self {
                self.0.title = title;
                self
            }

            /// One sentence on the row's hover, the web's `title`: what a shift-press does, or the keys that pick it.
            pub fn hover(mut self, hover: impl Into<SharedString>) -> Self {
                self.0.hover = Some(hover.into());
                self
            }

            /// Keeps the name in the foreground while not picked, for a list where every row is an answer to read.
            pub fn full_ink(mut self) -> Self {
                self.0.full_ink = true;
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

impl Choice {
    /// `max-sm:pl-7`: the slot past the radio.
    const STACKED_INDENT: Pixels = px(28.);

    /// The slot under the words across the row, as the web's dock row stands under 640 px.
    pub fn stacked(mut self, stacked: bool) -> Self {
        self.0.stacked = stacked.then_some(Self::STACKED_INDENT);
        self
    }
}

impl Choice {
    /// A quiet fact at the words' right, in the `FACT` class: "from $0.018/h".
    pub fn fact(mut self, fact: impl Into<SharedString>) -> Self {
        self.0.fact = Some(fact.into());
        self
    }

    /// One control or the facts at the right: a dock row's field.
    pub fn slot(mut self, slot: impl IntoElement) -> Self {
        self.0.slot = Some(slot.into_any_element());
        self
    }
}

impl RenderOnce for Choice {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mark = radio(self.0.on, false, cx);
        let stacked = self.0.stacked;
        self.0.render(mark, px(8.), size::ROW_FLOOR, stacked, cx)
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

impl PickRow {
    /// The thing's own mark in the frame, in place of a glyph: an agent's logo.
    pub fn framed(mut self, mark: impl IntoElement) -> Self {
        self.0.framed = Some(mark.into_any_element());
        self
    }

    /// A version or a path, in the `FACT` class after the name.
    pub fn tag(mut self, tag: impl Into<SharedString>) -> Self {
        self.0.tag = Some(tag.into());
        self
    }

    /// The marks of the agents a thing is set up for, after the name.
    pub fn marks(mut self, marks: impl IntoElement) -> Self {
        self.0.marks = Some(marks.into_any_element());
        self
    }

    /// One control or the facts at the right.
    pub fn slot(mut self, slot: impl IntoElement) -> Self {
        self.0.slot = Some(slot.into_any_element());
        self
    }

    /// A tick the host fixed: the checkbox held, its edge dashed and its tick muted, and no pointer hand over the row.
    pub fn locked(mut self, locked: bool) -> Self {
        self.0.locked = locked;
        self
    }
}

impl RenderOnce for PickRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mark = checkbox(SharedString::from(format!("{}-checkbox", self.0.id)), self.0.on, self.0.locked, window, cx);
        // The web's rule stands on the row's wrapper, outside the label's floor, where a choice's stands inside it.
        let floor = size::ROW_FLOOR + if self.0.corners.rule.is_some() { px(1.) } else { px(0.) };
        let stacked = super::narrow::width(window).sm.then_some(PICK_SLOT_INDENT);
        self.0.render(mark, px(4.), floor, stacked, cx)
    }
}

impl From<PickRow> for CardItem {
    fn from(row: PickRow) -> Self {
        CardItem::placed(move |corners| PickRow(Pick { corners, ..row.0 }).into_any_element())
    }
}

/// A control at the row's width that, while held, lets nothing through.
fn field_frame(control: impl IntoElement, held: bool) -> gpui_kit::Div {
    div()
        .debug_selector(|| "control".into())
        .relative()
        .flex_none()
        .w(size::SELECT_WIDTH)
        .h(size::CONTROL_HEIGHT)
        .child(control)
        .when(held, |el| el.child(div().absolute().inset_0().occlude()))
}

/// A field a person types into: in a row (`ROW_FIELD`), gpui-kit's input at the select's height and size; in a sheet
/// (`FIELD`), 40 px across the sheet. Held, it keeps its look at 64% and takes nothing; invalid, its edge takes the
/// destructive ink, stronger while it has focus.
#[derive(IntoElement)]
pub struct Field {
    state: Entity<InputState>,
    held: bool,
    invalid: bool,
    sheet: Option<bool>,
    width: Option<DefiniteLength>,
    mono: bool,
    lead: Option<Glyph>,
    height: Option<Pixels>,
    fill: bool,
    type_size: Option<Pixels>,
}

impl Field {
    pub fn new(state: &Entity<InputState>) -> Self {
        Field {
            state: state.clone(),
            held: false,
            invalid: false,
            sheet: None,
            width: None,
            mono: false,
            lead: None,
            height: None,
            fill: false,
            type_size: None,
        }
    }

    /// A row field at a width other than the select's: a slate field's 160 px slot, or the whole line.
    pub fn width(mut self, width: impl Into<DefiniteLength>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// A row field in the mono.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    /// What it holds was refused: `aria-invalid` while the refusal stands under it.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// The sheet's field, 40 px high across the sheet: a path or a command in the mono at 13 px.
    pub fn sheet(mut self) -> Self {
        self.sheet = Some(true);
        self
    }

    /// The sheet's field for words, a name: the sans at 14 px.
    pub fn sheet_words(mut self) -> Self {
        self.sheet = Some(false);
        self
    }

    /// A sheet field's glyph at its left, the muted 16 px 12 px in, the text after it (`ps-9`): a key's KeyRound.
    pub fn lead(mut self, glyph: Glyph) -> Self {
        self.lead = Some(glyph);
        self
    }

    /// A row field as wide as its row leaves it (`flex-1`), not the select's 176 px: a pick list's filter.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// A sheet field's type size other than its own: a large mono field's 14 px.
    pub fn type_size(mut self, size: Pixels) -> Self {
        self.type_size = Some(size);
        self
    }

    /// A sheet field's height other than 40 px: the pair screen's `h-8`.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }
}

impl RenderOnce for Field {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let focused = self.state.read(cx).focus_handle(cx).is_focused(window);
        let edge = match (self.invalid, focused) {
            (true, true) => p.destructive.opacity(0.64),
            (true, false) => p.destructive.opacity(0.36),
            (false, _) => p.input,
        };
        let dim = Dim::new(p.background, p.input_fill);
        let (edge, fill) = if self.held { (dim.over(edge), dim.fill()) } else { (edge, p.input_fill) };
        let ink = p.foreground;
        let input = Input::new(&self.state)
            .disabled(self.held)
            .disabled_ink(HELD)
            .focus_bordered(!self.invalid)
            .rounded(size::CONTROL_RADIUS)
            .px(px(11.))
            .py_0()
            .border_color(edge)
            .bg(fill)
            .text_color(ink);
        match self.sheet {
            None if self.fill => div()
                .debug_selector(|| "control".into())
                .relative()
                .flex_1()
                .min_w_0()
                .h(size::CONTROL_HEIGHT)
                .child(Styled::h(input, size::CONTROL_HEIGHT).w_full().text_size(text::FACT.0))
                .when(self.held, |el| el.child(div().absolute().inset_0().occlude())),
            None => {
                let width = self.width.unwrap_or(size::SELECT_WIDTH.into());
                let input = Styled::h(input, size::CONTROL_HEIGHT).w(width).text_size(text::FACT.0);
                let input = if self.mono { input.font_family(cx.theme().mono_font_family.clone()) } else { input };
                field_frame(input, self.held).w(width)
            }
            Some(mono) => {
                let height = self.height.unwrap_or(px(40.));
                let input = Styled::h(input, height).w_full().rounded(size::BUTTON_RADIUS).when(self.lead.is_some(), |i| i.pl(px(36.)));
                let input = if mono {
                    input.font_family(cx.theme().mono_font_family.clone()).text_size(self.type_size.unwrap_or(text::FACT.0))
                } else {
                    input.text_size(self.type_size.unwrap_or(text::VALUE.0))
                };
                div()
                    .debug_selector(|| "control".into())
                    .relative()
                    .w_full()
                    .h(height)
                    .child(input)
                    .children(self.lead.map(|g| {
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(px(12.))
                            .flex()
                            .items_center()
                            .child(Icon::new(g).with_size(size::GLYPH).text_color(p.muted_foreground))
                    }))
                    .when(self.held, |el| el.child(div().absolute().inset_0().occlude()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, Context, TestAppContext, Window};

    use super::Button;
    use crate::ui::{self, Side};

    struct Lone {
        busy: bool,
        pressed: Rc<Cell<u32>>,
    }

    impl Render for Lone {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let pressed = self.pressed.clone();
            div().p(px(24.)).child(Button::outline("save", "Save").busy(self.busy).on_click(move |_, _, _| pressed.set(pressed.get() + 1)))
        }
    }

    fn press(cx: &mut TestAppContext, busy: bool) -> u32 {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let pressed = Rc::new(Cell::new(0));
        let view = cx.new(|_| Lone { busy, pressed: pressed.clone() });
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(400.), px(200.)));
        cx.run_until_parked();
        let at = cx.debug_bounds("button").expect("the button is drawn");
        cx.simulate_click(at.center(), gpui_kit::Modifiers::none());
        cx.run_until_parked();
        pressed.get()
    }

    #[gpui_kit::test]
    fn a_busy_button_holds_its_press(cx: &mut TestAppContext) {
        assert_eq!(press(cx, true), 0, "a press while its road runs goes nowhere");
    }

    #[gpui_kit::test]
    fn a_button_at_rest_takes_its_press(cx: &mut TestAppContext) {
        assert_eq!(press(cx, false), 1);
    }
}
