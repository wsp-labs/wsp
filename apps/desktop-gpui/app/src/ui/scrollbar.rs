// SPDX-License-Identifier: AGPL-3.0-only
//! The scroll area's vertical bar (`components/ui/scroll-area.tsx`): a 6 px thumb in `--app-scrollbar-thumb`, rounded
//! full, easing to its hover ink over 150 ms, in a lane 4 px short of each end and 1 px in from the right edge, drawn
//! only where the content overflows. It shows while the pointer rests on the area or it scrolls, as Base UI shows it;
//! the thumb drags, and a press on the lane beside it scrolls there, as Base UI's do.

use std::time::{Duration, Instant};

use gpui_kit::component::scroll::ScrollbarHandle;
use gpui_kit::{div, prelude::*, px, App, Div, ElementId, Empty, MouseButton, Pixels, Point, ScrollHandle, SharedString, Stateful, Window};

use super::{motion, ActivePalette};

/// `w-1.5`: the bar and its thumb.
pub const WIDTH: Pixels = px(6.);
/// `my-1`: how far short of each end the lane stops.
pub const ENDS: Pixels = px(4.);
/// `mr-px`: how far in from the right edge the lane runs.
pub const EDGE: Pixels = px(1.);
/// Base UI's `MIN_THUMB_SIZE`.
const MIN_THUMB: Pixels = px(16.);
/// Base UI's `SCROLL_TIMEOUT`: the area reads as scrolling this long after its last move.
const SCROLL_TIMEOUT: Duration = Duration::from_millis(500);

/// The thumb in its lane, as Base UI sizes it: the lane's share the viewport is of the content, at least
/// `MIN_THUMB`, and where along the lane's spare length the offset stands.
struct Thumb {
    top: Pixels,
    length: Pixels,
    travel: Pixels,
    most: Pixels,
}

fn thumb(handle: &ScrollHandle) -> Option<Thumb> {
    let (view, most) = (handle.bounds().size.height, handle.max_offset().y);
    let lane = view - ENDS * 2.;
    if most <= px(0.) || lane <= px(0.) {
        return None;
    }
    let length = (lane * (view / (view + most))).max(MIN_THUMB).min(lane);
    let travel = lane - length;
    Some(Thumb { top: travel * (-handle.offset().y / most).clamp(0., 1.), length, travel, most })
}

/// Scrolls `handle` so the thumb's top stands `top` down its lane.
fn scroll_to(handle: &ScrollHandle, thumb: &Thumb, top: Pixels) {
    if thumb.travel > px(0.) {
        let offset = handle.offset();
        handle.set_offset(Point::new(offset.x, -thumb.most * (top / thumb.travel).clamp(0., 1.)));
    }
}

/// A drag of the thumb of the area it names.
struct Grab(ElementId);

/// Where the area last stood and until when it reads as scrolling.
struct Seen {
    offset: Point<Pixels>,
    until: Option<Instant>,
}

/// `ScrollArea`: `content` scrolling on `handle` in the area `id` names, with its vertical bar over the right edge.
pub fn scroll_area(
    id: impl Into<ElementId>,
    handle: &ScrollHandle,
    content: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    scroll_view(id, handle, div().child(content), window, cx)
}

/// `scroll_area` whose viewport is `viewport` itself: its children are the handle's items, and a height cap on it
/// sizes the area.
pub fn scroll_view(id: impl Into<ElementId>, handle: &ScrollHandle, viewport: Div, window: &mut Window, cx: &mut App) -> Stateful<Div> {
    let id: ElementId = id.into();
    let hovered = motion::hovered(id.clone(), window, cx);
    let now = cx.background_executor().now();
    let offset = handle.offset();
    let seen = window.use_keyed_state(ElementId::from((id.clone(), "scrolled")), cx, |_, _| Seen { offset, until: None });
    let scrolling = seen.update(cx, |s, _| {
        if s.offset != offset {
            s.offset = offset;
            s.until = Some(now + SCROLL_TIMEOUT);
        }
        s.until.is_some_and(|u| now < u)
    });
    if scrolling {
        window.request_animation_frame();
    }
    let opacity = motion::bar_opacity(id.clone(), *hovered.read(cx) || scrolling, window, cx);
    // A viewport not laid out yet says nothing of its overflow; the next frame reads it.
    if handle.bounds().size.height <= px(0.) {
        window.request_animation_frame();
    }
    let grab = window.use_keyed_state(ElementId::from((id.clone(), "grab")), cx, |_, _| px(0.));
    let lane = thumb(handle).map(|at| {
        let thumb_id = ElementId::from((id.clone(), "thumb"));
        let over = motion::hovered(thumb_id.clone(), window, cx);
        let p = cx.palette();
        let ink = if *over.read(cx) { p.scrollbar_thumb_hover } else { p.scrollbar_thumb };
        let ink = motion::ease_ink(thumb_id.clone(), "ink", ink, motion::COLORS, window, cx);
        let grab = grab.clone();
        let jump = handle.clone();
        div()
            .id(ElementId::from((id.clone(), "lane")))
            .debug_selector(|| "scrollbar".into())
            .absolute()
            .top(ENDS)
            .bottom(ENDS)
            .right(EDGE)
            .w(WIDTH)
            .on_mouse_down(MouseButton::Left, move |e, window, cx| {
                cx.stop_propagation();
                let Some(at) = thumb(&jump) else { return };
                let y = e.position.y - jump.bounds().origin.y - ENDS;
                if y < at.top || y > at.top + at.length {
                    scroll_to(&jump, &at, y - at.length / 2.);
                    window.refresh();
                } else {
                    grab.update(cx, |g, _| *g = y - at.top);
                }
            })
            .when(opacity > 0., |el| {
                el.child(
                    div()
                        .id(thumb_id)
                        .debug_selector(|| "scrollbar-thumb".into())
                        .absolute()
                        .top(at.top)
                        .w_full()
                        .h(at.length)
                        .rounded_full()
                        .bg(ink)
                        .opacity(opacity)
                        .on_hover(motion::track_hover(over))
                        .on_drag(Grab(id.clone()), |_, _, _, cx| cx.new(|_| Empty)),
                )
            })
    });
    let dragged = handle.clone();
    let area = id.clone();
    div()
        .id(id.clone())
        .relative()
        .on_hover(motion::track_hover(hovered))
        .on_drag_move(move |e: &gpui_kit::DragMoveEvent<Grab>, window, cx| {
            if e.drag(cx).0 != area {
                return;
            }
            let Some(at) = thumb(&dragged) else { return };
            scroll_to(&dragged, &at, e.event.position.y - dragged.bounds().origin.y - ENDS - *grab.read(cx));
            window.refresh();
        })
        .child(viewport.id(ElementId::from((id, "viewport"))).size_full().overflow_y_scroll().track_scroll(handle))
        .children(lane)
}

/// Today's native bar on a list that owns its scroll (`::-webkit-scrollbar` in `index.css`): the shortest thumb its
/// 300-row lists draw.
const LIST_MIN_THUMB: Pixels = px(17.);

/// Whether the content runs past the viewport `handle` scrolls, as of the last frame drawn.
pub fn overflows(handle: &impl ScrollbarHandle) -> bool {
    handle.content_size().height - handle.viewport_bounds().size.height > px(0.5)
}

/// The thumb as Chromium sizes a custom bar's: the whole height is its lane, its length the viewport's share of the
/// content, rounded, at least `LIST_MIN_THUMB`.
fn list_thumb(handle: &impl ScrollbarHandle) -> Option<Thumb> {
    let view = handle.viewport_bounds().size.height;
    let content = handle.content_size().height;
    let most = content - view;
    if most <= px(0.5) || view <= px(0.) {
        return None;
    }
    let length = (view * (view / content)).round().max(LIST_MIN_THUMB).min(view);
    let travel = view - length;
    Some(Thumb { top: travel * (-handle.offset().y / most).clamp(0., 1.), length, travel, most })
}

/// The vertical bar of a list that scrolls itself (`uniform_list`, `list`), as today's native one draws it: [`WIDTH`]
/// wide over the parent's right edge, full height, shown at rest wherever the content overflows, its thumb easing to
/// the hover ink. The thumb drags; a press on the lane beside it pages there. Nothing while the content fits. The
/// parent is `relative` and as tall as the viewport; the list keeps the lane free with `pr(WIDTH)` while
/// [`overflows`]. `id` names the bar and tags it for tests, the thumb as `<id>-thumb`.
pub fn list_bar<H: ScrollbarHandle + Clone>(
    id: impl Into<SharedString>,
    handle: &H,
    window: &mut Window,
    cx: &mut App,
) -> Option<Stateful<Div>> {
    let tag: SharedString = id.into();
    let over = motion::hovered(ElementId::from((ElementId::from(tag.clone()), "thumb")), window, cx);
    let p = cx.palette();
    let ink = if *over.read(cx) { p.scrollbar_thumb_hover } else { p.scrollbar_thumb };
    bar(tag, handle, ink, px(0.), over, window, cx)
}

/// The bar `@pierre/trees` draws on today's Files tree: its 6 px lane kept whether or not the rows overflow
/// (`scrollbar-gutter: stable`), the thumb inside a 1 px transparent edge in `--trees-fg` at 25%, and shown only
/// while `shown`, the pointer over the tree.
pub fn tree_bar<H: ScrollbarHandle + Clone>(
    id: impl Into<SharedString>,
    handle: &H,
    shown: bool,
    window: &mut Window,
    cx: &mut App,
) -> Option<Stateful<Div>> {
    let tag: SharedString = id.into();
    let over = motion::hovered(ElementId::from((ElementId::from(tag.clone()), "thumb")), window, cx);
    let ink = if shown { cx.palette().tree_foreground.opacity(0.25) } else { cx.palette().tree_foreground.opacity(0.) };
    bar(tag, handle, ink, px(1.), over, window, cx)
}

/// A bar over the parent's right edge with its thumb in `ink`, `inset` in from each side of the thumb's box.
fn bar<H: ScrollbarHandle + Clone>(
    tag: SharedString,
    handle: &H,
    ink: gpui_kit::Hsla,
    inset: Pixels,
    over: gpui_kit::Entity<bool>,
    window: &mut Window,
    cx: &mut App,
) -> Option<Stateful<Div>> {
    // A viewport not laid out yet says nothing of its overflow; the next frame reads it.
    if handle.viewport_bounds().size.height <= px(0.) {
        window.request_animation_frame();
    }
    let at = list_thumb(handle)?;
    let thumb_tag: SharedString = format!("{tag}-thumb").into();
    let id = ElementId::from(tag.clone());
    let thumb_id = ElementId::from((id.clone(), "thumb"));
    let ink = motion::ease_ink(thumb_id.clone(), "ink", ink, motion::COLORS, window, cx);
    let grab = window.use_keyed_state(ElementId::from((id.clone(), "grab")), cx, |_, _| px(0.));
    let (press, drag, end, end_out) = (handle.clone(), handle.clone(), handle.clone(), handle.clone());
    let (pressed, dragged) = (grab.clone(), grab);
    let area = id.clone();
    Some(
        div()
            .id(id.clone())
            .debug_selector(move || tag.to_string())
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(WIDTH)
            .on_mouse_down(MouseButton::Left, move |e, window, cx| {
                cx.stop_propagation();
                let Some(at) = list_thumb(&press) else { return };
                let y = e.position.y - press.viewport_bounds().origin.y;
                if y < at.top || y > at.top + at.length {
                    let page = press.viewport_bounds().size.height * if y < at.top { 1. } else { -1. };
                    let offset = press.offset();
                    press.set_offset(Point::new(offset.x, (offset.y + page).clamp(-at.most, px(0.))));
                    window.refresh();
                } else {
                    press.start_drag();
                    pressed.update(cx, |g, _| *g = y - at.top);
                }
            })
            .on_mouse_up(MouseButton::Left, move |_, _, _| end.end_drag())
            .on_mouse_up_out(MouseButton::Left, move |_, _, _| end_out.end_drag())
            .on_drag_move(move |e: &gpui_kit::DragMoveEvent<Grab>, window, cx| {
                if e.drag(cx).0 != area {
                    return;
                }
                let Some(at) = list_thumb(&drag) else { return };
                let top = e.event.position.y - drag.viewport_bounds().origin.y - *dragged.read(cx);
                if at.travel > px(0.) {
                    let offset = drag.offset();
                    drag.set_offset(Point::new(offset.x, -at.most * (top / at.travel).clamp(0., 1.)));
                }
                window.refresh();
            })
            .child(
                div()
                    .id(thumb_id)
                    .debug_selector(move || thumb_tag.to_string())
                    .absolute()
                    .top(at.top + inset)
                    .left(inset)
                    .right(inset)
                    .h(at.length - inset * 2.)
                    .rounded_full()
                    .bg(ink)
                    .on_hover(motion::track_hover(over))
                    .on_drag(Grab(id), |_, _, _, cx| cx.new(|_| Empty)),
            ),
    )
}
