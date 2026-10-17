// SPDX-License-Identifier: AGPL-3.0-only
//! The rail of person messages at the transcript's left edge (`timeline/minimap.tsx`), one tick per message.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::{
    canvas, div, fill, point, prelude::*, px, size, Animation, AnimationExt as _, Bounds, BoxShadow, Context, FocusHandle, FontWeight,
    Hsla, KeyDownEvent, MouseMoveEvent, Pixels, SharedString, Window,
};

use crate::thread_view::ThreadView;
use crate::transcript::RowKind;
use crate::ui::ActivePalette;

/// `w-18`: the strip the rail stands in.
const STRIP: Pixels = px(72.);
/// `left-3`: the hit strip's inset, and the rail's inside it.
const INSET: Pixels = px(12.);
/// One message's share of the rail, and the most the rail may take: `calc(100vh - 18rem)`.
const STEP: Pixels = px(8.);
const ROOM: Pixels = px(288.);
/// The side gutter from which the rail stays shown rather than appearing on hover.
const GUTTER: Pixels = px(48.);
/// `TIMELINE_MINIMAP_HIT_STRIP_MAX_WIDTH`, and `22rem` once the card shows.
const HIT_MAX: Pixels = px(40.);
const HIT_OPEN: Pixels = px(352.);
/// `CHAT_TIMELINE_ANCHOR_OFFSET`: where a click stands the message.
const ANCHOR: Pixels = px(24.);
/// The card: `left-8 w-80`.
const CARD_LEFT: Pixels = px(32.);
pub const CARD_WIDTH: Pixels = px(320.);
/// `duration-150` on the rail's fade.
const FADE: Duration = Duration::from_millis(150);

/// The rail's own state: the tick the pointer or the keyboard is on, and its focus.
#[derive(Default)]
pub struct Rail {
    pub active: Option<usize>,
    focus: Option<FocusHandle>,
    focused: bool,
    hovered: bool,
    drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The card's height as last drawn, which its `translateY(-50% | -100%)` moves it by.
    card: Rc<Cell<Pixels>>,
}

impl Rail {
    pub fn focus_handle(&self) -> Option<&FocusHandle> {
        self.focus.as_ref()
    }
}

/// A tick's width by how far it stands from the active one: `w-6`, `w-4`, `w-2.5`, else `w-2`.
pub fn tick_width(distance: Option<usize>) -> Pixels {
    match distance {
        Some(0) => px(24.),
        Some(1) => px(16.),
        Some(2) => px(10.),
        _ => px(8.),
    }
}

/// `resolveTimelineMinimapIndexFromPointer`.
fn index_at(y: Pixels, bounds: Bounds<Pixels>, count: usize) -> usize {
    if count <= 1 || bounds.size.height <= px(0.) {
        return 0;
    }
    let at = ((y - bounds.origin.y) / bounds.size.height).clamp(0., 1.);
    (at * (count - 1) as f32).round() as usize
}

fn compact(text: &str) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// The person's message at row `ix` and the last reply of its turn, each on one line.
fn preview(view: &ThreadView, ix: usize) -> (Option<String>, Option<String>) {
    let rows = view.rows();
    let asked = rows.get(ix).and_then(|r| compact(&r.text));
    let reply = rows
        .iter()
        .skip(ix + 1)
        .take_while(|r| !matches!(r.kind, RowKind::Person { .. }))
        .filter(|r| matches!(r.kind, RowKind::Agent { .. }))
        .last()
        .and_then(|r| compact(&r.text));
    (asked, reply)
}

fn pick(view: &mut ThreadView, row: usize, cx: &mut Context<ThreadView>) {
    view.reveal(row, ANCHOR, cx);
}

/// The rail for `marks`, each a person message's row and whether it is on screen; nothing below two messages.
pub fn minimap(
    view: &mut ThreadView,
    marks: Vec<(usize, bool)>,
    width: Pixels,
    window_height: Pixels,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> Option<impl IntoElement> {
    let gutter = (width - width.min(super::rows::COLUMN)) / 2.;
    if marks.len() < 2 {
        return None;
    }
    let persistent = gutter >= GUTTER;
    let hit = (gutter.floor() - INSET).min(HIT_MAX).max(px(0.));
    let count = marks.len();
    let focus = view.rail().focus.get_or_insert_with(|| cx.focus_handle().tab_stop(true)).clone();
    let focused = focus.is_focused(window);
    let rail = view.rail();
    if focused && !rail.focused {
        rail.active = rail.active.or(Some(0));
    } else if !focused && rail.focused {
        rail.active = None;
    }
    rail.focused = focused;
    let active = rail.active.filter(|&a| a < count);
    let shown = persistent || rail.hovered || focused;
    let drawn = rail.drawn.clone();
    let card_height = rail.card.clone();
    let p = cx.palette().clone();
    let (rest, near, lit, line) =
        (p.muted_foreground.opacity(0.35), p.muted_foreground.opacity(0.75), p.foreground.opacity(0.9), p.border.opacity(0.15));
    let tall = (STEP * (count - 1) as f32).min(window_height - ROOM).max(px(1.));
    let rows: Vec<usize> = marks.iter().map(|(row, _)| *row).collect();
    let on: Vec<bool> = marks.iter().map(|(_, on)| *on).collect();
    let card = active.map(|a| {
        let (asked, reply) = preview(view, rows[a]);
        let top = tall * (a as f32 / (count - 1) as f32);
        let shift = if a == 0 {
            0.
        } else if a == count - 1 {
            -1.
        } else {
            -0.5
        };
        let shadow = vec![
            BoxShadow {
                color: gpui_kit::black().opacity(0.25),
                offset: point(px(0.), px(20.)),
                blur_radius: px(25.),
                spread_radius: px(-5.),
                inset: false,
            },
            BoxShadow {
                color: gpui_kit::black().opacity(0.25),
                offset: point(px(0.), px(8.)),
                blur_radius: px(10.),
                spread_radius: px(-6.),
                inset: false,
            },
        ];
        let glass = div()
            .debug_selector(|| "minimap-preview".into())
            .w(CARD_WIDTH)
            .rounded(px(14.))
            .p(px(12.))
            .border_1()
            .border_color(p.foreground.opacity(0.1))
            .shadow(shadow)
            .text_size(px(14.))
            .line_height(px(20.))
            .child(
                div()
                    .w_full()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .child(asked.unwrap_or_else(|| "User message".to_owned())),
            )
            .children(reply.map(|r| {
                div().mt(px(4.)).max_h(px(60.)).overflow_hidden().line_clamp(3).text_ellipsis().text_color(p.muted_foreground).child(r)
            }));
        let glass = if cfg!(target_os = "macos") { glass.bg(p.popover) } else { glass.bg(p.popover.opacity(0.96)).backdrop_blur(px(8.)) };
        (top, shift, glass)
    });
    let rail_view = div()
        .id("minimap")
        .debug_selector(|| "minimap".into())
        .track_focus(&focus)
        .relative()
        .ml(INSET)
        .w(if active.is_some() { HIT_OPEN } else { hit })
        .h(tall)
        .when(hit > px(0.), |el| {
            el.cursor_pointer()
                .on_hover(cx.listener(|view, hovered: &bool, _, cx| {
                    let rail = view.rail();
                    rail.hovered = *hovered;
                    if !*hovered {
                        rail.active = None;
                    }
                    cx.notify();
                }))
                .on_mouse_move(cx.listener(move |view, event: &MouseMoveEvent, _, cx| {
                    let rail = view.rail();
                    let Some(bounds) = rail.drawn.get() else { return };
                    let next = Some(index_at(event.position.y, bounds, count));
                    if rail.active != next {
                        rail.active = next;
                        cx.notify();
                    }
                }))
                .on_click({
                    let rows = rows.clone();
                    cx.listener(move |view, event: &gpui_kit::ClickEvent, window, cx| {
                        let Some(bounds) = view.rail().drawn.get() else { return };
                        pick(view, rows[index_at(event.position().y, bounds, rows.len())], cx);
                        view.focus_transcript(window, cx);
                    })
                })
        })
        .on_key_down({
            let rows = rows.clone();
            cx.listener(move |view, event: &KeyDownEvent, _, cx| {
                let rail = view.rail();
                let last = count - 1;
                let active = rail.active.filter(|&a| a < count);
                let base = active.unwrap_or(0);
                let next = match event.keystroke.key.as_str() {
                    "down" => Some((base + 1).min(last)),
                    "up" => Some(base.saturating_sub(1)),
                    "home" => Some(0),
                    "end" => Some(last),
                    "enter" | "space" => {
                        if let Some(a) = active {
                            pick(view, rows[a], cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    _ => return,
                };
                rail.active = next;
                cx.stop_propagation();
                cx.notify();
            })
        })
        .when(focused, |el| el.child(div().absolute().inset(px(-2.)).rounded(px(4.)).border_2().border_color(p.ring.opacity(0.7))))
        .child(
            canvas(
                move |bounds, _, _| drawn.set(Some(bounds)),
                move |bounds, _, window, _| paint(bounds, &on, active, rest, near, lit, line, window),
            )
            .absolute()
            .top_0()
            .left_0()
            .w(hit.max(px(24.)))
            .h_full(),
        )
        .children(card.map(|(top, shift, glass)| {
            let measured = card_height.clone();
            div()
                .id("minimap-card")
                .absolute()
                .left(CARD_LEFT)
                .top(top + card_height.get() * shift)
                .cursor_text()
                .child(glass)
                .child(
                    canvas(
                        move |bounds, window, _| {
                            if measured.get() != bounds.size.height {
                                measured.set(bounds.size.height);
                                window.request_animation_frame();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
                .on_mouse_move(|_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| cx.stop_propagation())
        }));
    let strip = div().absolute().top_0().bottom_0().left_0().w(STRIP).flex().flex_col().justify_center().child(rail_view);
    let id = SharedString::from(if shown { "minimap-in" } else { "minimap-out" });
    let strip = if persistent {
        strip.into_any_element()
    } else {
        let (from, to) = if shown { (0., 1.) } else { (1., 0.) };
        strip.opacity(to).with_animation(id, Animation::new(FADE), move |el, t| el.opacity(from + (to - from) * t)).into_any_element()
    };
    Some(strip)
}

#[allow(clippy::too_many_arguments)]
fn paint(bounds: Bounds<Pixels>, on: &[bool], active: Option<usize>, rest: Hsla, near: Hsla, lit: Hsla, line: Hsla, window: &mut Window) {
    window
        .paint_quad(fill(Bounds { origin: point(bounds.origin.x + INSET, bounds.origin.y), size: size(px(1.), bounds.size.height) }, line));
    let last = (on.len() - 1).max(1) as f32;
    let step = bounds.size.height / last;
    let ticks = on.iter().enumerate().map(|(i, lit_now)| {
        let distance = active.map(|a| a.abs_diff(i));
        let ink = if *lit_now {
            lit
        } else if distance == Some(0) {
            near
        } else {
            rest
        };
        (bounds.origin.y + step * i as f32 - px(1.), tick_width(distance), ink)
    });
    for (top, bottom, width, ink) in runs(ticks, step) {
        let tick = Bounds { origin: point(bounds.origin.x, top), size: size(width, bottom - top) };
        window.paint_quad(fill(tick, ink).corner_radii(px(1.)));
    }
}

/// Touching ticks of one width and ink as one quad, in their stacked alphas' ink: a long thread's stand under 2 px apart.
fn runs(ticks: impl Iterator<Item = (Pixels, Pixels, Hsla)>, step: Pixels) -> Vec<(Pixels, Pixels, Pixels, Hsla)> {
    let stack = |ink: Hsla| {
        let mut ink = ink;
        if step < px(2.) {
            ink.a = 1. - (1. - ink.a).powf(2. / (step / px(1.)).max(0.01));
        }
        ink
    };
    let mut out: Vec<(Pixels, Pixels, Pixels, Hsla, Hsla)> = Vec::new();
    for (top, width, ink) in ticks {
        match out.last_mut() {
            Some((_, bottom, w, i, _)) if *w == width && *i == ink && top <= *bottom => *bottom = top + px(2.),
            _ => out.push((top, top + px(2.), width, ink, stack(ink))),
        }
    }
    out.into_iter()
        .map(|(top, bottom, width, ink, stacked)| (top, bottom, width, if bottom - top > px(2.) { stacked } else { ink }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_that_touch_draw_as_one_run_and_apart_draw_alone() {
        let ink = gpui_kit::black().opacity(0.35);
        let dense: Vec<_> = (0..900).map(|i| (px(i as f32 * 0.57), px(8.), ink)).collect();
        let merged = runs(dense.into_iter(), px(0.57));
        assert_eq!(merged.len(), 1, "900 ticks 0.57 px apart are one bar");
        assert!(merged[0].3.a > 0.35, "the bar is as dark as the stacked ticks");
        let sparse: Vec<_> = (0..45).map(|i| (px(i as f32 * 8.), px(8.), ink)).collect();
        assert_eq!(runs(sparse.into_iter(), px(8.)).len(), 45, "ticks 8 px apart stand alone");
    }

    #[test]
    fn ticks_widen_near_the_active_one() {
        assert_eq!(tick_width(Some(0)), px(24.));
        assert_eq!(tick_width(Some(1)), px(16.));
        assert_eq!(tick_width(Some(2)), px(10.));
        assert_eq!(tick_width(Some(3)), px(8.));
        assert_eq!(tick_width(None), px(8.));
    }
}
