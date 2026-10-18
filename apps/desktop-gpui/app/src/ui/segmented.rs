// SPDX-License-Identifier: AGPL-3.0-only
//! `SegmentedControl` (`apps/web/src/components/ui/segmented-control.tsx`): one pick among a few, as segments in an
//! edged track with no fill of its own; the picked segment takes the accent fill and the foreground. To the keyboard it
//! is a radio group: one tab stop, the arrows move the pick round the segments, and the picked one wears the ring.

use std::rc::Rc;

use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{point, prelude::*, px, App, BoxShadow, FocusHandle, KeyDownEvent, Pixels, SharedString, Window};

use super::icons::Glyph;
use super::theme::ActivePalette;

type Pick = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

/// The track's `rounded-lg` and the segment's `rounded-md`, off `--radius`.
const TRACK_RADIUS: Pixels = px(10.);
const SEGMENT_RADIUS: Pixels = px(8.);

struct Segment {
    label: SharedString,
    glyph: Option<Glyph>,
    /// Why the segment cannot be picked here, where it cannot.
    held: Option<SharedString>,
}

#[derive(IntoElement)]
pub struct Segmented {
    id: SharedString,
    segments: Vec<Segment>,
    picked: Option<usize>,
    height: Pixels,
    text: (Pixels, Pixels),
    pad: Pixels,
    glyph: (Pixels, Pixels),
    on_pick: Option<Pick>,
}

impl Segmented {
    /// The control at its default `h-8`, its segments `px-3 text-[13px] leading-none`; none picked where the value the
    /// caller holds is none of the segments'.
    pub fn new(id: impl Into<SharedString>, picked: impl Into<Option<usize>>) -> Self {
        Segmented {
            id: id.into(),
            segments: Vec::new(),
            picked: picked.into(),
            height: px(32.),
            text: (px(13.), px(13.)),
            pad: px(12.),
            glyph: (px(16.), px(8.)),
            on_pick: None,
        }
    }

    pub fn segment(mut self, label: impl Into<SharedString>) -> Self {
        self.segments.push(Segment { label: label.into(), glyph: None, held: None });
        self
    }

    /// A segment led by its glyph, by default 16 px and 8 px before the label.
    pub fn glyph_segment(mut self, glyph: Glyph, label: impl Into<SharedString>) -> Self {
        self.segments.push(Segment { label: label.into(), glyph: Some(glyph), held: None });
        self
    }

    /// A segment that cannot be picked here, at half strength, saying why on its hover.
    pub fn held_segment(mut self, label: impl Into<SharedString>, why: impl Into<SharedString>) -> Self {
        self.segments.push(Segment { label: label.into(), glyph: None, held: Some(why.into()) });
        self
    }

    /// The track's whole height, its edge and padding included.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// Each segment's text size and line height, and its sides.
    pub fn segment_size(mut self, text: (Pixels, Pixels), pad: Pixels) -> Self {
        self.text = text;
        self.pad = pad;
        self
    }

    /// The glyphs' size and the gap after them.
    pub fn glyph_size(mut self, size: Pixels, gap: Pixels) -> Self {
        self.glyph = (size, gap);
        self
    }

    pub fn on_pick(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(f));
        self
    }
}

/// The segment an arrow lands on from `from`, round the ends and past the held ones, from either end where none is
/// picked; none where none can be picked.
pub fn step(open: &[bool], from: Option<usize>, forward: bool) -> Option<usize> {
    let n = open.len();
    let Some(from) = from else {
        return if forward { open.iter().position(|o| *o) } else { open.iter().rposition(|o| *o) };
    };
    (1..=n).map(|k| if forward { (from + k) % n } else { (from + n * 2 - k) % n }).find(|&ix| open[ix] && ix != from)
}

impl RenderOnce for Segmented {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let focus: FocusHandle = window
            .use_keyed_state(SharedString::from(format!("{}-focus", self.id)), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let ringed = focus.is_focused(window) && window.last_input_was_keyboard();
        let ring =
            vec![BoxShadow { color: p.ring, offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(2.), inset: false }];
        let open: Vec<bool> = self.segments.iter().map(|s| s.held.is_none()).collect();
        let (picked, on_pick) = (self.picked, self.on_pick.clone());
        let ((font, line), pad, (glyph, gap)) = (self.text, self.pad, self.glyph);
        h_flex()
            .id(self.id.clone())
            .debug_selector(|| "segmented".into())
            .track_focus(&focus)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let forward = match event.keystroke.key.as_str() {
                    "right" | "down" => true,
                    "left" | "up" => false,
                    _ => return,
                };
                if let (Some(next), Some(f)) = (step(&open, picked, forward), on_pick.as_ref()) {
                    cx.stop_propagation();
                    f(next, window, cx);
                }
            })
            .flex_none()
            .items_stretch()
            .h(self.height)
            .p(px(2.))
            .gap(px(1.))
            .rounded(TRACK_RADIUS)
            .border_1()
            .border_color(p.border)
            .children(self.segments.into_iter().enumerate().map(|(ix, segment)| {
                let is_picked = picked == Some(ix);
                let held = segment.held.is_some();
                let ink = if is_picked { p.foreground } else { p.muted_foreground };
                let on_pick = self.on_pick.clone().filter(|_| !is_picked && !held);
                let focus = focus.clone();
                h_flex()
                    .id((self.id.clone(), ix))
                    .debug_selector(|| "segment".into())
                    .justify_center()
                    .px(pad)
                    .gap(gap)
                    .rounded(SEGMENT_RADIUS)
                    .text_size(font)
                    .line_height(line)
                    .text_color(ink)
                    .when(is_picked, |el| el.bg(p.accent))
                    .when(is_picked && ringed, |el| el.shadow(ring.clone()))
                    .when(held, |el| el.opacity(0.5))
                    .when(!is_picked && !held, |el| el.cursor_pointer().hover(|s| s.text_color(p.foreground)))
                    .when_some(segment.held, |el, why| {
                        el.tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(why.clone()).build(window, cx))
                    })
                    .children(segment.glyph.map(|g| Icon::new(g).with_size(glyph)))
                    .child(segment.label)
                    .when_some(on_pick, |el, f| {
                        el.on_click(move |_, window, cx| {
                            window.focus(&focus, cx);
                            f(ix, window, cx)
                        })
                    })
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::step;

    #[test]
    fn the_arrows_go_round_the_ends_and_past_held_segments() {
        let open = [true, true, true];
        assert_eq!(step(&open, Some(0), true), Some(1));
        assert_eq!(step(&open, Some(2), true), Some(0));
        assert_eq!(step(&open, Some(0), false), Some(2));
        assert_eq!(step(&[true, false, true], Some(0), true), Some(2));
        assert_eq!(step(&[true, false, true], Some(2), false), Some(0));
        assert_eq!(step(&[true, false], Some(0), true), None);
    }

    #[test]
    fn with_none_picked_the_arrows_start_from_the_open_end() {
        assert_eq!(step(&[false, true, true, false], None, true), Some(1));
        assert_eq!(step(&[false, true, true, false], None, false), Some(2));
        assert_eq!(step(&[false, false], None, true), None);
    }
}
