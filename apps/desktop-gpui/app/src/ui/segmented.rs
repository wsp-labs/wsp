// SPDX-License-Identifier: AGPL-3.0-only
//! `SegmentedControl` (`apps/web/src/components/ui/segmented-control.tsx`): one pick among a few, as segments in an
//! edged track with no fill of its own; the picked segment takes the accent fill and the foreground. To the keyboard it
//! is a radio group: one tab stop, the arrows move the pick round the segments, and the picked one wears the ring.

use std::rc::Rc;

use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, point, prelude::*, px, App, BoxShadow, FocusHandle, KeyDownEvent, Pixels, SharedString, Window};

use super::controls::bevel;
use super::icons::Glyph;
use super::motion;
use super::theme::ActivePalette;

type Pick = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

/// The track's `rounded-lg` and the segment's `rounded-md`, off `--radius`.
const TRACK_RADIUS: Pixels = px(10.);
const SEGMENT_RADIUS: Pixels = px(8.);

struct Segment {
    label: SharedString,
    glyph: Option<Glyph>,
    /// A count after the label, in the muted mono.
    count: Option<SharedString>,
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
    keycap: bool,
    held: bool,
    fill: bool,
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
            keycap: false,
            held: false,
            fill: false,
            on_pick: None,
        }
    }

    pub fn segment(mut self, label: impl Into<SharedString>) -> Self {
        self.segments.push(Segment { label: label.into(), glyph: None, count: None, held: None });
        self
    }

    /// A segment with its count after the label, `font-mono text-xs` in the muted ink.
    pub fn count_segment(mut self, label: impl Into<SharedString>, count: impl Into<SharedString>) -> Self {
        self.segments.push(Segment { label: label.into(), glyph: None, count: Some(count.into()), held: None });
        self
    }

    /// A segment led by its glyph, by default 16 px and 8 px before the label.
    pub fn glyph_segment(mut self, glyph: Glyph, label: impl Into<SharedString>) -> Self {
        self.segments.push(Segment { label: label.into(), glyph: Some(glyph), count: None, held: None });
        self
    }

    /// A segment that cannot be picked here, at half strength, saying why on its hover.
    pub fn held_segment(mut self, label: impl Into<SharedString>, why: impl Into<SharedString>) -> Self {
        self.segments.push(Segment { label: label.into(), glyph: None, count: None, held: Some(why.into()) });
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

    /// The glyphs' size and the gap after them, and before a count.
    pub fn glyph_size(mut self, size: Pixels, gap: Pixels) -> Self {
        self.glyph = (size, gap);
        self
    }

    /// `SEGMENTED` and `SEGMENT` in the slate's `look.ts`: the keycap's edge, fill and bevel on an `h-9` track, its
    /// segments `rounded-lg px-3.5`.
    pub fn keycap(mut self) -> Self {
        self.keycap = true;
        self.height = px(36.);
        self.pad = px(14.);
        self
    }

    /// The whole group disabled: every segment at half strength, taking no press or key.
    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    /// The track as wide as its row, its segments sharing the width alike (`className="flex w-full"`, `flex-1`).
    pub fn fill(mut self) -> Self {
        self.fill = true;
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
        let all_held = self.held;
        let open: Vec<bool> = self.segments.iter().map(|s| s.held.is_none() && !all_held).collect();
        let (picked, on_pick) = (self.picked, self.on_pick.clone());
        let ((font, line), pad, (glyph, gap), wide) = (self.text, self.pad, self.glyph, self.fill);
        let keycap = self.keycap;
        let segment_radius = if keycap { TRACK_RADIUS } else { SEGMENT_RADIUS };
        h_flex()
            .id(self.id.clone())
            .debug_selector(|| "segmented".into())
            .when(!all_held, |el| el.track_focus(&focus))
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
            .map(|el| if wide { el.w_full() } else { el.flex_none() })
            .items_stretch()
            .h(self.height)
            .p(px(2.))
            .gap(px(if keycap { 2. } else { 1. }))
            .rounded(TRACK_RADIUS)
            .border_1()
            .border_color(if keycap { p.input } else { p.border })
            .when(keycap, |el| el.bg(p.input_fill).shadow(bevel(&p)))
            .children(self.segments.into_iter().enumerate().map(|(ix, segment)| {
                let is_picked = picked == Some(ix);
                let held = segment.held.is_some() || all_held;
                let key = SharedString::from(format!("{}-{ix}", self.id));
                let over = motion::hovered(key.clone(), window, cx);
                let lit = is_picked || (*over.read(cx) && !held);
                let ink =
                    motion::ease_ink(key.clone(), "ink", if lit { p.foreground } else { p.muted_foreground }, motion::COLORS, window, cx);
                let fill =
                    motion::ease_ink(key, "fill", if is_picked { p.accent } else { p.accent.opacity(0.) }, motion::COLORS, window, cx);
                let on_pick = self.on_pick.clone().filter(|_| !is_picked && !held);
                let focus = focus.clone();
                h_flex()
                    .id((self.id.clone(), ix))
                    .debug_selector(|| "segment".into())
                    .justify_center()
                    .when(wide, |el| el.flex_1())
                    .px(pad)
                    .gap(gap)
                    .rounded(segment_radius)
                    .text_size(font)
                    .line_height(line)
                    .text_color(ink)
                    .bg(fill)
                    .on_hover(motion::track_hover(over))
                    .when(is_picked && ringed, |el| el.shadow(ring.clone()))
                    .when(held, |el| el.opacity(0.5))
                    .when(!is_picked && !held, |el| el.cursor_pointer())
                    .when_some(segment.held, |el, why| {
                        el.tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(why.clone()).build(window, cx))
                    })
                    .children(segment.glyph.map(|g| Icon::new(g).with_size(glyph)))
                    .child(segment.label)
                    .children(segment.count.map(|n| {
                        div()
                            .debug_selector(|| "segment-count".into())
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(px(12.))
                            .text_color(p.muted_foreground)
                            .child(n)
                    }))
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
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{
        div, prelude::*, px, size, Context, FocusHandle, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext, Window,
    };

    use super::{step, Segmented};
    use crate::ui::{self, Side};

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

    /// Three segments between two plain tab stops.
    struct Three {
        picked: Rc<Cell<usize>>,
        before: FocusHandle,
        after: FocusHandle,
    }

    impl Render for Three {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let (picked, this) = (self.picked.clone(), cx.entity());
            let control =
                Segmented::new("views", Some(self.picked.get())).segment("Threads").segment("Processes").segment("Ports").on_pick(
                    move |ix, _, cx| {
                        picked.set(ix);
                        this.update(cx, |_, cx| cx.notify());
                    },
                );
            gpui_kit::div()
                .flex()
                .p(px(24.))
                .gap_2()
                .child(div().size(px(20.)).track_focus(&self.before))
                .child(control)
                .child(div().size(px(20.)).track_focus(&self.after))
        }
    }

    fn press(cx: &mut VisualTestContext, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.simulate_event(KeyDownEvent { keystroke: keystroke.clone(), is_held: false, prefer_character_input: false });
        cx.simulate_event(KeyUpEvent { keystroke });
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn a_focused_control_is_one_tab_stop_and_its_arrows_move_the_pick(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let picked = Rc::new(Cell::new(0));
        let view = cx.new(|cx| Three {
            picked: picked.clone(),
            before: cx.focus_handle().tab_stop(true),
            after: cx.focus_handle().tab_stop(true),
        });
        let (before, after) = view.read_with(cx, |v, _| (v.before.clone(), v.after.clone()));
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(500.), px(100.)));
        cx.run_until_parked();
        let focused = |cx: &mut VisualTestContext| cx.update(|window, cx| window.focused(cx));
        cx.update(|window, cx| window.focus_next(cx));
        assert_eq!(focused(cx), Some(before.clone()));
        cx.simulate_keystrokes("tab");
        let control = focused(cx).expect("Tab reaches the control");
        assert!(control != before && control != after);
        cx.simulate_keystrokes("tab");
        assert_eq!(focused(cx), Some(after), "the next Tab leaves it: one stop for the control, none for each segment");
        cx.simulate_keystrokes("shift-tab");
        assert_eq!(focused(cx), Some(control.clone()));
        let mut steps = Vec::new();
        for key in ["right", "right", "right", "left"] {
            press(cx, key);
            steps.push(picked.get());
        }
        assert_eq!(steps, [1, 2, 0, 2], "each arrow moves the pick, round the ends");
        assert_eq!(focused(cx), Some(control), "and the focus stays on the control");
    }
}
