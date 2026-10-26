// SPDX-License-Identifier: AGPL-3.0-only
//! An open group's calls and a work row standing on its own: the tool line, its detail, and Find's marks over them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, ElementId, HighlightStyle, Pixels, ScrollHandle, SharedString, Window};

use super::block::block;
use super::{chevron, focusable, glyph_slot, OUTPUT_MAX, SM};
use crate::thread_view::ThreadView;
use crate::transcript::{Call, Glyph as RowGlyph, Row, Tone};
use crate::ui::Palette;

/// A closed call row's height, which an open one grows the group's cap by the rest of (`CLOSED_ROW_PX`).
const CLOSED_ROW: Pixels = px(30.);
/// `--fade-size`: the open group's edge fades while it scrolls.
const EDGE_FADE: Pixels = px(24.);

/// An open group's calls, `ms-3`, capped at `min(18rem, 50dvh)` plus what its open calls grew by, so a call opened
/// grows the list rather than scrolling inside it, and fading at an edge it scrolls past (`ExpandedWorkGroupEntries`).
pub(super) fn calls_list(
    view: &mut ThreadView,
    row: &Row,
    calls: &[Call],
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let id = SharedString::from(format!("calls:{}", calls.first().map_or("", |c| c.key.as_str())));
    let scroll =
        window.use_keyed_state(ElementId::from((ElementId::from(id.clone()), "scroll")), cx, |_, _| ScrollHandle::new()).read(cx).clone();
    let grown =
        window.use_keyed_state(ElementId::from((ElementId::from(id.clone()), "grown")), cx, |_, _| Heights::default()).read(cx).clone();
    let open: Vec<&str> = calls.iter().filter(|c| c.open).map(|c| c.key.as_str()).collect();
    grown.borrow_mut().retain(|key, _| open.contains(&key.as_str()));
    let growth = grown.borrow().values().map(|h| (*h - CLOSED_ROW).max(px(0.))).fold(px(0.), |a, b| a + b);
    let cap = OUTPUT_MAX.min(window.viewport_size().height / 2.) + growth;
    let (offset, most) = (-scroll.offset().y, scroll.max_offset().y);
    let (top, bottom) = (offset > px(1.), most - offset > px(1.));
    let ground = p.background;
    let fade = |down: bool| {
        let (from, to) = if down { (ground, ground.opacity(0.)) } else { (ground.opacity(0.), ground) };
        div().absolute().left_0().right_0().h(EDGE_FADE).bg(gpui_kit::linear_gradient(
            180.,
            gpui_kit::linear_color_stop(from, 0.),
            gpui_kit::linear_color_stop(to, 1.),
        ))
    };
    let shown: Vec<AnyElement> = calls.iter().map(|call| call_row(view, row, call, &grown, p, window, cx)).collect();
    // The list's extent is known only once it is laid out, so a frame that drew the wrong fades draws again.
    let (handle, me) = (scroll.clone(), cx.weak_entity());
    let recheck = gpui_kit::canvas(
        move |_, _, cx| {
            let (offset, most) = (-handle.offset().y, handle.max_offset().y);
            if (offset > px(1.), most - offset > px(1.)) != (top, bottom) {
                redraw(me.clone(), cx);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_0();
    div()
        .relative()
        .ml(px(12.))
        .child(
            v_flex()
                .id(id)
                .debug_selector(|| "calls".into())
                .rounded(px(8.))
                .max_h(cap)
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .children(shown.into_iter().map(|call| div().flex_none().child(call))),
        )
        .child(recheck)
        .when(top, |el| el.child(fade(true).debug_selector(|| "calls-fade-top".into()).top_0()))
        .when(bottom, |el| el.child(fade(false).debug_selector(|| "calls-fade-bottom".into()).bottom_0()))
        .into_any_element()
}

/// Draws the rows again once this frame ends: what a frame measured while it drew changes how they lay out.
fn redraw(view: gpui_kit::WeakEntity<ThreadView>, cx: &mut App) {
    cx.defer(move |cx| _ = view.update(cx, |_, cx| cx.notify()));
}

/// Each open call's height as it last drew, by its key.
type Heights = Rc<RefCell<HashMap<String, Pixels>>>;

/// A canvas filling an open call that keeps its height, and draws the rows again when it moved.
fn measure(key: &str, grown: &Heights, cx: &mut Context<ThreadView>) -> impl IntoElement {
    let (key, grown, me) = (key.to_owned(), grown.clone(), cx.weak_entity());
    gpui_kit::canvas(
        move |bounds, _, cx| {
            let was = grown.borrow_mut().insert(key.clone(), bounds.size.height);
            if was != Some(bounds.size.height) {
                redraw(me.clone(), cx);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

fn call_row(
    view: &mut ThreadView,
    row: &Row,
    call: &Call,
    grown: &Heights,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let shown = if call.command.is_some() || call.patch.is_some() {
        block(view, row, call, p, window, cx)
    } else {
        let opens = call.body.as_ref().is_some_and(|b| !b.trim().is_empty());
        let key = call.key.clone();
        let label = label_text(call.verb.as_deref(), &call.label);
        let marks = super::super::find::marks(view, &row.key, &format!("{}:label", call.key), &label, cx);
        let line = plain_line(&call.label, call.verb.as_deref(), call.mono, call.tone, call.glyph, opens, Some(call.open), &marks, p, cx);
        let body = call.body.as_deref().filter(|_| call.open).map(|b| {
            let marks = super::super::find::marks(view, &row.key, &format!("{}:body", call.key), b, cx);
            detail(&call.key, b, marks, p, cx)
        });
        v_flex()
            .id(SharedString::from(call.key.clone()))
            .px(px(2.))
            .rounded(px(8.))
            .when(opens, |el| {
                focusable(el.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.2))), p)
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(&key, cx)))
            })
            .child(line)
            .children(body)
            .into_any_element()
    };
    if !call.open {
        return shown;
    }
    div().relative().child(shown).child(measure(&call.key, grown, cx)).into_any_element()
}

/// A work row standing on its own (`PlainWorkEntryRow`, `py-0.5`): it opens to its body as a call in a group does.
#[allow(clippy::too_many_arguments)]
pub(super) fn work(
    view: &ThreadView,
    row: &Row,
    tone: Tone,
    g: RowGlyph,
    body: Option<&str>,
    open: bool,
    p: &Palette,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let opens = body.is_some_and(|b| !b.trim().is_empty());
    let marks = super::super::find::marks(view, &row.key, "label", &row.text, cx);
    let body = body.filter(|_| open).map(|b| {
        let marks = super::super::find::marks(view, &row.key, "body", b, cx);
        detail(&row.key, b, marks, p, cx)
    });
    let key = row.key.clone();
    v_flex()
        .id(SharedString::from(format!("work:{}", row.key)))
        .debug_selector(|| "work".into())
        .py(px(2.))
        .px(px(2.))
        .rounded(px(8.))
        .when(opens, |el| {
            focusable(el.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.2))), p)
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(&key, cx)))
        })
        .child(plain_line(&row.text, None, false, tone, g, opens, Some(open), &marks, p, cx))
        .children(body)
        .into_any_element()
}

/// Highlights over a row's plain text, by byte range.
pub(super) type Marks = Vec<(Range<usize>, HighlightStyle)>;

/// `base` with `over` laid on it, cut wherever either changes so no two ranges overlap, as StyledText needs.
pub(super) fn layered(base: &[(Range<usize>, HighlightStyle)], over: &[(Range<usize>, HighlightStyle)]) -> Marks {
    if over.is_empty() {
        return base.to_vec();
    }
    let mut cuts: Vec<usize> = base.iter().chain(over).flat_map(|(r, _)| [r.start, r.end]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let at = |list: &[(Range<usize>, HighlightStyle)], x: usize| list.iter().find(|(r, _)| r.contains(&x)).map(|(_, s)| *s);
    cuts.windows(2)
        .filter_map(|w| {
            let style = match (at(base, w[0]), at(over, w[0])) {
                (Some(b), Some(o)) => b.highlight(o),
                (b, o) => b.or(o)?,
            };
            Some((w[0]..w[1], style))
        })
        .collect()
}

/// The marks of `marks` that fall in `from..from + len`, moved to start at 0.
pub(super) fn shifted(marks: &[(Range<usize>, HighlightStyle)], from: usize, len: usize) -> Marks {
    let end = from + len;
    marks
        .iter()
        .filter(|(r, _)| r.start < end && r.end > from)
        .map(|(r, s)| (r.start.max(from) - from..r.end.min(end) - from, *s))
        .collect()
}

/// Text with Find's marks painted on it; `marks` covers `"{verb} {text}"` when there is a verb.
fn marked(text: &str, at: usize, marks: &[(Range<usize>, HighlightStyle)]) -> gpui_kit::StyledText {
    gpui_kit::StyledText::new(text.to_owned()).with_highlights(shifted(marks, at, text.len()))
}

/// The label a call's or a work row's find part reads: the verb, a space, the label.
pub(super) fn label_text(verb: Option<&str>, text: &str) -> String {
    verb.map_or_else(|| text.to_owned(), |verb| format!("{verb} {text}"))
}

/// A tool row's line (`PlainWorkEntryRow`): glyph, label, and the 16 px chevron slot kept even when it cannot open.
#[allow(clippy::too_many_arguments)]
fn plain_line(
    text: &str,
    verb: Option<&str>,
    mono: bool,
    tone: Tone,
    g: RowGlyph,
    opens: bool,
    open: Option<bool>,
    marks: &[(Range<usize>, HighlightStyle)],
    p: &Palette,
    cx: &App,
) -> impl IntoElement {
    let after = verb.map_or(0, |v| v.len() + 1);
    let glyph_ink = match tone {
        Tone::Thinking | Tone::Error => p.foreground,
        _ => p.muted_foreground,
    };
    let mono = mono || tone == Tone::Notice;
    h_flex()
        .gap(px(6.))
        .text_size(SM.0)
        .line_height(SM.1)
        .child(glyph_slot(g, glyph_ink))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .text_color(p.muted_foreground)
                .children(verb.map(|v| div().flex_none().child(marked(v, 0, marks))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
                        .child(marked(text, after, marks)),
                ),
        )
        .when_some(open, |el, open| {
            el.child(
                div()
                    .flex_none()
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!opens, |el| el.invisible())
                    .child(chevron(open, p.muted_foreground)),
            )
        })
}

/// An open tool row's detail: `mt-1 ms-7 border-s border-border/45 ps-3 pt-0.5`, mono at the picked code size, else
/// 11 px, `leading-relaxed`.
fn detail(key: &str, text: &str, marks: Marks, p: &Palette, cx: &App) -> impl IntoElement {
    let text = gpui_kit::StyledText::new(text.to_owned()).with_highlights(marks);
    let size = cx.try_global::<super::markdown::Sizes>().and_then(|s| s.code).unwrap_or(px(11.));
    div().mt(px(4.)).ml(px(28.)).border_l_1().border_color(p.border.opacity(0.45)).pl(px(12.)).pt(px(2.)).child(
        div()
            .id(SharedString::from(format!("detail:{key}")))
            .debug_selector(|| "call-detail".into())
            .max_h(px(256.))
            .overflow_y_scroll()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(size)
            .line_height(size * 1.625)
            .text_color(p.muted_foreground)
            .child(text),
    )
}
