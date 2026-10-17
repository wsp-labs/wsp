// SPDX-License-Identifier: AGPL-3.0-only
//! The rail of person messages at the transcript's left edge (`timeline/minimap.tsx`): one tick per message, the ones
//! on screen in the foreground's ink, centred on the viewport; a click scrolls to the nearest message.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::{canvas, div, fill, point, prelude::*, px, size, Bounds, Context, Hsla, MouseButton, MouseDownEvent, Pixels};

use crate::thread_view::ThreadView;
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
/// `CHAT_TIMELINE_ANCHOR_OFFSET`: where a click stands the message.
const ANCHOR: Pixels = px(24.);

/// The rail for `marks`, each a person message's row and whether it is on screen, over a viewport `width` x `height`
/// in a window `window_height` tall; nothing below two messages, or when the side gutter is under 48 px.
pub fn minimap(marks: Vec<(usize, bool)>, width: Pixels, window_height: Pixels, cx: &mut Context<ThreadView>) -> Option<impl IntoElement> {
    let gutter = (width - width.min(super::rows::COLUMN)) / 2.;
    if marks.len() < 2 || gutter < GUTTER {
        return None;
    }
    let p = cx.palette();
    let (rest, shown, rail) = (p.muted_foreground.opacity(0.35), p.foreground.opacity(0.9), p.border.opacity(0.15));
    let tall = (STEP * (marks.len() - 1) as f32).min(window_height - ROOM).max(px(1.));
    let rows: Vec<usize> = marks.iter().map(|(row, _)| *row).collect();
    let lit: Vec<bool> = marks.iter().map(|(_, on)| *on).collect();
    let hit = (gutter.floor() - INSET).min(px(40.));
    let drawn: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::default();
    let at = drawn.clone();
    Some(
        div().absolute().top_0().bottom_0().left_0().w(STRIP).flex().flex_col().justify_center().child(
            div()
                .id("minimap")
                .debug_selector(|| "minimap".into())
                .relative()
                .ml(INSET)
                .w(hit)
                .h(tall)
                .cursor_pointer()
                .child(
                    canvas(
                        move |bounds, _, _| at.set(Some(bounds)),
                        move |bounds, _, window, _| paint(bounds, &lit, rest, shown, rail, window),
                    )
                    .absolute()
                    .inset_0(),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        let Some(bounds) = drawn.get() else { return };
                        let at = ((event.position.y - bounds.origin.y) / bounds.size.height).clamp(0., 1.);
                        let ix = (at * (rows.len() - 1) as f32).round() as usize;
                        this.reveal(rows[ix], ANCHOR, cx);
                    }),
                ),
        ),
    )
}

fn paint(bounds: Bounds<Pixels>, lit: &[bool], rest: Hsla, shown: Hsla, rail: Hsla, window: &mut gpui_kit::Window) {
    window
        .paint_quad(fill(Bounds { origin: point(bounds.origin.x + INSET, bounds.origin.y), size: size(px(1.), bounds.size.height) }, rail));
    let last = (lit.len() - 1).max(1) as f32;
    for (i, on) in lit.iter().enumerate() {
        let y = bounds.origin.y + bounds.size.height * (i as f32 / last) - px(1.);
        let tick = Bounds { origin: point(bounds.origin.x, y), size: size(px(8.), px(2.)) };
        window.paint_quad(fill(tick, if *on { shown } else { rest }).corner_radii(px(1.)));
    }
}
