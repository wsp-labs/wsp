// SPDX-License-Identifier: AGPL-3.0-only
//! The two on-the-row choices that are not a select: the switch (`switch.tsx`) for on or off, and the segmented
//! control (`segmented-control.tsx`) for one of a few words. Both are drawn from what the caller holds and hand the
//! next value to `on_change`.

use std::rc::Rc;

use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Pixels, SharedString, Window};

use super::controls::Dim;
use super::dialog::tip;
use super::focus::ring;
use super::icons::Glyph;
use super::motion::{ease_ink, ease_px, hovered, track_hover, COLORS, TRACK};
use super::theme::ActivePalette;
use super::{size, text};

/// `--thumb-size` from 640 px up: `--spacing(4)`.
const THUMB: Pixels = px(16.);

type Change<T> = Rc<dyn Fn(&T, &mut Window, &mut App) + 'static>;

/// `Switch`: on or off in a row's slot. The track takes the primary once on and the thumb slides over; held, it keeps
/// its look at 64% and takes no press.
#[derive(IntoElement)]
pub struct Switch {
    id: SharedString,
    on: bool,
    held: bool,
    on_change: Option<Change<bool>>,
}

impl Switch {
    pub fn new(id: impl Into<SharedString>, on: bool) -> Self {
        Switch { id: id.into(), on, held: false, on_change: None }
    }

    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    /// Called with the value a press asks for, the opposite of the one drawn.
    pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let (track, thumb) = if self.on { (p.primary, p.primary_foreground) } else { (p.input, p.foreground) };
        let (track, thumb) = if self.held {
            let dim = Dim::new(p.background, track);
            (dim.fill(), dim.over(thumb))
        } else {
            (track, thumb)
        };
        let id = self.id.clone();
        let track = ease_ink(id.clone(), "track", track, TRACK, window, cx);
        let thumb = ease_ink(id.clone(), "thumb", thumb, COLORS, window, cx);
        let x = ease_px(id.clone(), "x", if self.on { THUMB - px(4.) } else { px(0.) }, COLORS, window, cx);
        let on = self.on;
        h_flex()
            .id(self.id)
            .debug_selector(|| "switch".into())
            .flex_none()
            .w(THUMB * 2. - px(2.))
            .h(THUMB + px(2.))
            .p(px(1.))
            .rounded_full()
            .bg(track)
            .child(div().ml(x).size(THUMB).rounded_full().bg(thumb).shadow(p.thumb_shadow.clone()))
            .when(!self.held, |el| {
                el.cursor_pointer()
                    .tab_index(0)
                    .focus_visible(|s| s.shadow(vec![ring(p.ring)]))
                    .when_some(self.on_change, |el, f| el.on_click(move |_, window, cx| f(&!on, window, cx)))
            })
    }
}

/// One word of a segmented control.
pub struct Segment<T> {
    value: T,
    label: SharedString,
    glyph: Option<Glyph>,
    held: Option<SharedString>,
}

impl<T> Segment<T> {
    pub fn new(value: T, label: impl Into<SharedString>) -> Self {
        Segment { value, label: label.into(), glyph: None, held: None }
    }

    pub fn glyph(mut self, glyph: Glyph) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// A word that cannot be picked here, standing in its place with why on its hover.
    pub fn held(mut self, why: impl Into<SharedString>) -> Self {
        self.held = Some(why.into());
        self
    }
}

/// `SegmentedControl`: a choice between a few words in one row. One hairline box, the picked word on the accent in
/// the foreground ink, the rest muted until the pointer is over them.
#[derive(IntoElement)]
pub struct SegmentedControl<T: Clone + PartialEq + 'static> {
    id: SharedString,
    value: T,
    segments: Vec<Segment<T>>,
    large: bool,
    on_change: Option<Change<T>>,
}

impl<T: Clone + PartialEq + 'static> SegmentedControl<T> {
    pub fn new(id: impl Into<SharedString>, value: T, segments: Vec<Segment<T>>) -> Self {
        SegmentedControl { id: id.into(), value, segments, large: false, on_change: None }
    }

    /// The page tabs' size (Usage, Activity): `h-9`, `px-3.5`, `text-sm`.
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&T, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl<T: Clone + PartialEq + 'static> RenderOnce for SegmentedControl<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let (height, pad, font, gap) =
            if self.large { (px(36.), px(14.), text::VALUE, px(8.)) } else { (px(32.), px(12.), text::FACT, px(6.)) };
        let segments: Vec<AnyElement> = self
            .segments
            .into_iter()
            .enumerate()
            .map(|(ix, segment)| {
                let picked = segment.value == self.value;
                let id = SharedString::from(format!("{}-{ix}", self.id));
                let over = hovered(id.clone(), window, cx);
                let lit = picked || (*over.read(cx) && segment.held.is_none());
                let ink = if segment.held.is_some() {
                    p.muted_foreground.opacity(0.5)
                } else if lit {
                    p.foreground
                } else {
                    p.muted_foreground
                };
                let ink = ease_ink(id.clone(), "ink", ink, COLORS, window, cx);
                let fill = ease_ink(id.clone(), "fill", if picked { p.accent } else { p.accent.opacity(0.) }, COLORS, window, cx);
                let (value, change) = (segment.value.clone(), self.on_change.clone());
                h_flex()
                    .id(id)
                    .debug_selector(|| "segment".into())
                    .justify_center()
                    .gap(gap)
                    .px(pad)
                    .rounded(size::BUTTON_RADIUS)
                    .bg(fill)
                    .text_size(font)
                    .line_height(font)
                    .text_color(ink)
                    .children(segment.glyph.map(|g| Icon::new(g).with_size(px(14.)).text_color(ink)))
                    .child(segment.label)
                    .on_hover(track_hover(over))
                    .map(|el| match segment.held {
                        Some(why) => el.tooltip(tip(why)),
                        None => el
                            .cursor_pointer()
                            .tab_index(0)
                            .focus_visible(|s| s.shadow(vec![ring(p.ring)]))
                            .when(!picked, |el| el.when_some(change, |el, f| el.on_click(move |_, window, cx| f(&value, window, cx)))),
                    })
                    .into_any_element()
            })
            .collect();
        h_flex()
            .id(self.id)
            .debug_selector(|| "segmented-control".into())
            .flex_none()
            .items_stretch()
            .h(height)
            .gap(px(1.))
            .p(px(2.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .children(segments)
    }
}
