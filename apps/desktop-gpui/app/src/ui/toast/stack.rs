// SPDX-License-Identifier: AGPL-3.0-only
//! The toasts as Sonner 2.0.8 stacks them under `components/ui/sonner.tsx` and `notices/Notice.tsx`: newest in
//! front, the ones behind lifted a gap each and shrunk 5% each with their content hidden, the whole stack spread out
//! under the pointer, each toast's timer paused while it is, a toast swiped up or right to dismiss it. Each moving value
//! eases as the CSS rule that moves it (`styles.css`), from wherever it stood.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::base::{transition, Easing, Interpolate, Transition};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    canvas, div, point, prelude::*, px, AnyElement, App, Bounds, Context, DispatchPhase, ElementId, FocusHandle, FontWeight, Hitbox,
    HitboxBehavior, Hsla, MouseButton, MouseExitEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, TestSupportExt as _,
    Window,
};

use super::{store, Kind, Notice, Toasts};
use crate::ui::status::still;
use crate::ui::{menu, motion, ActivePalette, Button, Glyph};

/// `TOAST_WIDTH`.
pub const WIDTH: Pixels = px(356.);
/// `GAP`: between toasts spread out, and each lift of one behind.
pub const GAP: Pixels = px(14.);
/// Each toast behind the front one is this much narrower and shorter.
const SCALE_STEP: f32 = 0.05;
/// `TIME_BEFORE_UNMOUNT`: a toast that left is dropped this long after, its exit cut short there.
pub const LEAVE: Duration = Duration::from_millis(200);
/// `transform 400ms, opacity 400ms, height 400ms` on every toast, and `opacity 400ms` on its children.
const MOVE: Duration = Duration::from_millis(400);
/// A toast behind that leaves while the stack is together: `transform 500ms, opacity 200ms`.
const BACK_MOVE: Duration = Duration::from_millis(500);
const BACK_FADE: Duration = Duration::from_millis(200);
/// The swipe out keyframes, `200ms ease-out`.
const SWIPE_OUT: Duration = Duration::from_millis(200);
/// `SWIPE_THRESHOLD`, and the speed in px per ms past which a shorter swipe still dismisses.
const SWIPE_THRESHOLD: f32 = 45.;
const SWIPE_SPEED: f32 = 0.11;
/// `rounded-[12px]`.
const RADIUS: Pixels = px(12.);
/// `@media (max-width: 600px)`: the narrow window's offsets.
pub const NARROW: Pixels = px(600.);
/// `offset={{ top: 8, right: 20 }}`, `mobileOffset={{ top: 8, left: 12, right: 12 }}`.
pub const TOP: Pixels = px(8.);
pub const RIGHT: Pixels = px(20.);
pub const NARROW_SIDES: Pixels = px(12.);

/// What names a kind's glyph to a screen reader, the glyph, and its tone.
pub fn kind_look(kind: Kind, cx: &App) -> (&'static str, Glyph, Hsla) {
    let p = cx.palette();
    match kind {
        Kind::Error => ("Failed", Glyph::CircleAlert, p.status_failed),
        Kind::Done => ("Done", Glyph::CircleCheck, p.status_done),
        Kind::Waiting => ("Needs you", Glyph::MessageCircleQuestionMark, p.status_input),
        Kind::Note => ("Note", Glyph::Info, p.muted_foreground),
    }
}

pub const CLOSE_LABEL: &str = "Close this message";

/// One toast on screen, from its arrival until it has left.
pub struct Shown {
    pub id: u64,
    pub notice: Notice,
    pub focus: FocusHandle,
    /// Drawn once at its start, so its entry moves from there.
    pub mounted: bool,
    pub removed: Option<Removed>,
    /// What its timer has left and since when it runs; none for a keyed toast, which waits to be ended.
    pub left: Option<Duration>,
    pub since: Option<Instant>,
}

#[derive(Clone, Copy)]
pub struct Removed {
    pub at: Instant,
    /// Where it stood spread out as it left (`offsetBeforeRemove`).
    pub offset: Pixels,
    pub swiped: Option<Axis>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Axis {
    X,
    Y,
}

/// A press on a toast, held: it follows the pointer on the axis it first moved along.
pub struct Swipe {
    pub id: u64,
    pub from: Point<Pixels>,
    pub began: Instant,
    pub axis: Option<Axis>,
    pub amount: Point<Pixels>,
}

#[derive(Default)]
pub struct Stack {
    /// Newest first; the front toast is the first.
    pub shown: Vec<Shown>,
    /// Each toast's own height as last drawn, its edge included.
    pub heights: Rc<RefCell<HashMap<u64, Pixels>>>,
    pub hovered: bool,
    /// A press is held on the stack.
    pub interacting: bool,
    pub swipe: Option<Swipe>,
}

impl Stack {
    pub fn paused(&self) -> bool {
        self.hovered || self.interacting
    }

    pub fn expanded(&self) -> bool {
        self.hovered || self.interacting
    }

    fn standing(&self) -> impl Iterator<Item = &Shown> {
        self.shown.iter().filter(|s| s.removed.is_none())
    }

    /// The heights of the toasts still standing, front first (`heights`).
    fn standing_heights(&self) -> Vec<(u64, Pixels)> {
        let heights = self.heights.borrow();
        self.standing().map(|s| (s.id, heights.get(&s.id).copied().unwrap_or_default())).collect()
    }

    /// Where a standing toast stands spread out: the gaps and heights of those in front of it.
    fn offset_of(&self, id: u64) -> Pixels {
        let heights = self.standing_heights();
        let at = heights.iter().position(|(h, _)| *h == id).unwrap_or(0);
        GAP * at as f32 + heights[..at].iter().fold(px(0.), |sum, (_, h)| sum + *h)
    }

    /// Takes the store's standing toasts: a new one arrives in front, one no longer standing starts to leave from
    /// where it stood.
    pub fn sync(&mut self, standing: &[(u64, Notice)], now: Instant, cx: &mut App) {
        let paused = self.paused();
        let offsets: HashMap<u64, Pixels> = self.standing().map(|s| (s.id, self.offset_of(s.id))).collect();
        for shown in &mut self.shown {
            if shown.removed.is_none() && !standing.iter().any(|(id, _)| *id == shown.id) {
                let offset = offsets.get(&shown.id).copied().unwrap_or_default();
                shown.removed = Some(Removed { at: now, offset, swiped: None });
            }
        }
        for (id, notice) in standing {
            if self.shown.iter().any(|s| s.id == *id) {
                continue;
            }
            self.shown.insert(
                0,
                Shown {
                    id: *id,
                    notice: notice.clone(),
                    focus: cx.focus_handle(),
                    mounted: false,
                    removed: None,
                    left: store::timeout(notice),
                    since: (!paused).then_some(now),
                },
            );
        }
    }

    /// Stops or starts every timer as the pointer comes onto the stack or leaves it.
    pub fn set_paused(&mut self, was: bool, now: Instant) {
        let paused = self.paused();
        if paused == was {
            return;
        }
        for shown in self.shown.iter_mut().filter(|s| s.removed.is_none()) {
            if paused {
                if let (Some(left), Some(since)) = (shown.left, shown.since.take()) {
                    shown.left = Some(left.saturating_sub(now.saturating_duration_since(since)));
                }
            } else if shown.left.is_some() {
                shown.since = Some(now);
            }
        }
    }

    /// The toasts whose time ran out by `now`.
    pub fn expired(&self, now: Instant) -> Vec<u64> {
        self.standing().filter(|s| matches!((s.left, s.since), (Some(left), Some(since)) if since + left <= now)).map(|s| s.id).collect()
    }

    /// Drops the toasts that have finished leaving.
    pub fn prune(&mut self, now: Instant) {
        let heights = self.heights.clone();
        self.shown.retain(|s| {
            let keep = s.removed.is_none_or(|r| r.at + LEAVE > now);
            if !keep {
                heights.borrow_mut().remove(&s.id);
            }
            keep
        });
    }

    /// When the next timer runs out or the next leaving toast is dropped.
    pub fn next_wake(&self) -> Option<Instant> {
        self.shown
            .iter()
            .filter_map(|s| match (s.removed, s.left, s.since) {
                (Some(r), _, _) => Some(r.at + LEAVE),
                (None, Some(left), Some(since)) => Some(since + left),
                _ => None,
            })
            .min()
    }

    /// The toast Escape closes: the one holding the focus, else the newest standing.
    pub fn focused(&self, window: &Window, cx: &App) -> Option<u64> {
        self.standing().find(|s| s.focus.contains_focused(window, cx)).map(|s| s.id)
    }

    /// Marks a swiped toast as leaving along its axis.
    pub fn swiped_out(&mut self, id: u64, axis: Axis, now: Instant) {
        let offset = self.offset_of(id);
        if let Some(shown) = self.shown.iter_mut().find(|s| s.id == id) {
            shown.removed = Some(Removed { at: now, offset, swiped: Some(axis) });
        }
    }
}

/// `getDampening`: a pull the wrong way moves less the further it goes.
fn damped(delta: f32) -> f32 {
    let factor = delta.abs() / 20.;
    let damped = delta * (1. / (1.5 + factor));
    if damped.abs() < delta.abs() {
        damped
    } else {
        delta
    }
}

impl Swipe {
    /// Follows the pointer: up and right freely, as the top right stack allows, the other ways damped.
    pub fn moved(&mut self, to: Point<Pixels>) {
        let (dx, dy) = (f32::from(to.x - self.from.x), f32::from(to.y - self.from.y));
        if self.axis.is_none() && (dx.abs() > 1. || dy.abs() > 1.) {
            self.axis = Some(if dx.abs() > dy.abs() { Axis::X } else { Axis::Y });
        }
        self.amount = match self.axis {
            Some(Axis::Y) => point(px(0.), px(if dy < 0. { dy } else { damped(dy) })),
            Some(Axis::X) => point(px(if dx > 0. { dx } else { damped(dx) }), px(0.)),
            None => point(px(0.), px(0.)),
        };
    }

    /// Whether the release dismisses: far or fast enough, up or right.
    pub fn dismisses(&self, now: Instant) -> Option<Axis> {
        let axis = self.axis?;
        let amount = f32::from(if axis == Axis::X { self.amount.x } else { self.amount.y });
        let allowed = if axis == Axis::X { amount > 0. } else { amount < 0. };
        let ms = now.saturating_duration_since(self.began).as_secs_f32() * 1000.;
        let speed = if ms > 0. { amount.abs() / ms } else { f32::INFINITY };
        (allowed && (amount.abs() >= SWIPE_THRESHOLD || speed > SWIPE_SPEED)).then_some(axis)
    }
}

fn ease<T: Interpolate + PartialEq + 'static>(
    id: u64,
    channel: &'static str,
    target: T,
    (duration, easing): (Duration, Easing),
    window: &mut Window,
    cx: &mut App,
) -> T {
    if still(cx) {
        return target;
    }
    transition((ElementId::NamedInteger("notice".into(), id), channel), target, Transition::new(duration).easing(easing), window, cx)
}

/// What one toast's box and content come to this frame, before easing.
#[derive(Clone, Copy)]
struct Look {
    y: Pixels,
    scale: f32,
    height: Pixels,
    opacity: f32,
    content: f32,
    swipe: Point<Pixels>,
}

/// The Sonner rule that holds for the toast at `index`, as `styles.css` orders them.
#[allow(clippy::too_many_arguments)]
fn target(shown: &Shown, index: usize, own: Pixels, front: Pixels, offset: Pixels, expanded: bool, swipe: Option<&Swipe>) -> Look {
    let is_front = index == 0;
    let content = if !expanded && !is_front { 0. } else { 1. };
    let together = |y: Pixels| Look { y, scale: 1., height: own, opacity: 1., content, swipe: point(px(0.), px(0.)) };
    let mut look = if !shown.mounted {
        Look { opacity: 0., ..together(-own) }
    } else if expanded {
        together(offset)
    } else if is_front {
        together(px(0.))
    } else {
        Look { y: GAP * index as f32, scale: 1. - SCALE_STEP * index as f32, height: front, ..together(px(0.)) }
    };
    match shown.removed {
        Some(Removed { swiped: Some(_), .. }) | None => {}
        Some(_) if is_front => look = Look { y: -own, scale: 1., height: own, opacity: 0., ..look },
        Some(r) if expanded => look = Look { y: r.offset - own, scale: 1., height: own, opacity: 0., ..look },
        Some(_) => look = Look { y: front * 0.4, scale: 1., height: front, opacity: 0., ..look },
    }
    if let Some(swipe) = swipe.filter(|s| s.id == shown.id) {
        look.swipe = swipe.amount;
    }
    if index >= store::NOTICE_LIMIT {
        look.opacity = 0.;
    }
    look
}

/// The stack's box in the window: its left edge and width, for the centre column between `left` and `right`.
pub fn place(left: Pixels, right: Pixels, viewport: Pixels) -> (Pixels, Pixels) {
    if viewport <= NARROW {
        (left + NARROW_SIDES, right - left - NARROW_SIDES * 2.)
    } else {
        (right - RIGHT - WIDTH, WIDTH)
    }
}

impl Toasts {
    /// The stack drawn at `origin`, `width` wide.
    pub(super) fn draw_stack(&mut self, origin: Point<Pixels>, width: Pixels, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let expanded = self.stack.expanded();
        let standing = self.stack.standing_heights();
        let front = standing.first().map(|(_, h)| *h).unwrap_or_default();
        let ease_move = (MOVE, Easing::Ease);
        // Where each toast the pointer can reach is drawn, its spread-out gap included, and that gap alone, which
        // holds the stack's hover and its presses as Sonner's `::after` strip does.
        let mut rects = Vec::new();
        let mut gaps = Vec::new();
        let mut toasts = Vec::new();
        let heights = self.stack.heights.clone();
        let count = self.stack.shown.len();
        for index in 0..count {
            let shown = &self.stack.shown[index];
            let id = shown.id;
            let own = heights.borrow().get(&id).copied();
            let content = self.content(shown, width, window, cx);
            let Some(own) = own else {
                toasts.push(div().absolute().left(origin.x).top(origin.y).w(width).opacity(0.).child(content).into_any_element());
                continue;
            };
            let offset = shown.removed.map(|r| r.offset).unwrap_or_else(|| self.stack.offset_of(id));
            let swiping = self.stack.swipe.as_ref().filter(|s| s.id == id);
            let look = target(shown, index, own, front, offset, expanded, swiping);
            let removed = shown.removed;
            let back_leaving = removed.is_some_and(|r| r.swiped.is_none()) && index != 0 && !expanded;
            let (moves, fades) = match () {
                _ if swiping.is_some() => ((Duration::ZERO, Easing::Linear), (Duration::ZERO, Easing::Linear)),
                _ if back_leaving => ((BACK_MOVE, Easing::Ease), (BACK_FADE, Easing::Ease)),
                _ => (ease_move.clone(), ease_move.clone()),
            };
            let y = ease(id, "y", look.y, moves.clone(), window, cx);
            let scale = ease(id, "scale", look.scale, moves.clone(), window, cx);
            let height = ease(id, "height", look.height, moves.clone(), window, cx);
            let content_opacity = ease(id, "content", look.content, ease_move.clone(), window, cx);
            let out = removed.and_then(|r| r.swiped);
            let (swipe, opacity) = match out {
                Some(axis) => {
                    let gone = match axis {
                        Axis::X => point(look.swipe.x + width, px(0.)),
                        Axis::Y => point(px(0.), look.swipe.y - height),
                    };
                    let policy = (SWIPE_OUT, Easing::EaseOut);
                    (ease(id, "swipe", gone, policy.clone(), window, cx), ease(id, "opacity", 0., policy, window, cx))
                }
                None => (ease(id, "swipe", look.swipe, moves, window, cx), ease(id, "opacity", look.opacity, fades, window, cx)),
            };
            let shown = &mut self.stack.shown[index];
            if !shown.mounted {
                shown.mounted = true;
                window.request_animation_frame();
            }
            let shown = &self.stack.shown[index];
            let inset = point(width * (1. - scale) / 2., height * (1. - scale) / 2.);
            let bounds = Bounds::new(
                point(origin.x + inset.x + swipe.x, origin.y + y + inset.y + swipe.y),
                gpui_kit::size(width * scale, height * scale),
            );
            let live = removed.is_none() && index < store::NOTICE_LIMIT;
            if live {
                let reach = if expanded { GAP + px(1.) } else { px(0.) };
                rects.push(Bounds::new(
                    point(bounds.origin.x, bounds.origin.y - reach),
                    gpui_kit::size(bounds.size.width, bounds.size.height + reach),
                ));
                if expanded {
                    gaps.push(Bounds::new(point(bounds.origin.x, bounds.origin.y - reach), gpui_kit::size(bounds.size.width, reach)));
                }
            }
            let inner = div().absolute().left(-inset.x).top(-inset.y).w(width).opacity(content_opacity).child(content);
            toasts.push(self.surface(shown, bounds, (opacity, scale), live, this.clone(), inner, window, cx));
        }
        // Painted last, so the oldest is drawn first and the front one over it (`--z-index`).
        toasts.reverse();
        let listen = this.clone();
        div()
            .absolute()
            .inset_0()
            .font_family(cx.theme().font_family.clone())
            .children(toasts)
            .child(
                canvas(
                    // Hitboxes, so the stack is under the pointer only where nothing over it, a scrim or a dialog, is;
                    // the gaps take presses too, as the toasts do.
                    move |_, window, _| {
                        let toasts: Vec<Hitbox> = rects.iter().map(|r| window.insert_hitbox(*r, HitboxBehavior::Normal)).collect();
                        let gaps: Vec<Hitbox> = gaps.iter().map(|r| window.insert_hitbox(*r, HitboxBehavior::BlockMouse)).collect();
                        (toasts, gaps)
                    },
                    move |_, (toasts, gaps), window, _| {
                        let (moved, released, left) = (listen.clone(), listen.clone(), listen.clone());
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                            if phase == DispatchPhase::Bubble {
                                let over = toasts.iter().chain(&gaps).any(|h| h.is_hovered(window));
                                _ = moved.update(cx, |n, cx| n.pointer_moved(e.position, over, cx));
                            }
                        });
                        window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble {
                                _ = released.update(cx, |n, cx| n.released(e.position, cx));
                            }
                        });
                        window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble {
                                _ = left.update(cx, |n, cx| n.pointer_left(cx));
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }

    /// `dropdown-glass popover-shadow rounded-[12px]`: the card a toast stands on, taking every press on it.
    #[allow(clippy::too_many_arguments)]
    fn surface(
        &self,
        shown: &Shown,
        bounds: Bounds<Pixels>,
        (opacity, scale): (f32, f32),
        live: bool,
        this: gpui_kit::WeakEntity<Toasts>,
        inner: impl IntoElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = cx.palette().clone();
        let id = shown.id;
        let hovered = Self::hovered_state(id, window, cx);
        let (pressed, held) = (this.clone(), this);
        div()
            .id(ElementId::NamedInteger("notice".into(), id))
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .opacity(opacity)
            .rounded(RADIUS * scale)
            .map(|el| menu::glass(el, &p))
            .when(live, |el| {
                el.occlude()
                    .track_focus(&shown.focus)
                    .on_hover(motion::track_hover(hovered))
                    .capture_any_mouse_down(move |_, window, cx| {
                        _ = held.update(cx, |n, cx| n.press_held(window, cx));
                    })
                    .on_mouse_down(MouseButton::Left, move |e, _, cx| {
                        _ = pressed.update(cx, |n, cx| n.press_swipe(id, e.position, cx));
                    })
            })
            .child(inner)
            .test_support()
            .into_any_element()
    }

    /// Whether the pointer rests on toast `id`, which shows its X.
    fn hovered_state(id: u64, window: &mut Window, cx: &mut App) -> gpui_kit::Entity<bool> {
        motion::hovered(ElementId::NamedInteger("notice-hover".into(), id), window, cx)
    }

    /// `Notice.tsx`'s row: the kind glyph, the sentence, where, the detail, the action and the X.
    fn content(&self, shown: &Shown, width: Pixels, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let id = shown.id;
        let notice = &shown.notice;
        let (_, glyph, tone) = kind_look(notice.kind, cx);
        let this = cx.entity().downgrade();
        let over = *Self::hovered_state(id, window, cx).read(cx);
        let close = motion::ease_px(ElementId::NamedInteger("notice".into(), id), "close", px(f32::from(over)), motion::COLORS, window, cx);
        let close_opacity = f32::from(close);
        let heights = self.stack.heights.clone();
        // The two buttons take their own presses: a press on either starts no swipe and leaves the focus where it was.
        let keep_press = |name: &'static str, opacity: f32, button: Button| {
            div()
                .id(ElementId::NamedInteger(name.into(), id))
                .flex_none()
                .opacity(opacity)
                .capture_any_mouse_down(|_, window, _| window.prevent_default())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(button)
                .test_support()
        };
        let action = notice.action.clone().map(|action| {
            let this = this.clone();
            let button = Button::outline(SharedString::from(format!("notice-action-{id}")), action.word.clone()).xs().on_click(
                move |_, window, cx| {
                    (action.run)(window, cx);
                    _ = this.update(cx, |n, cx| n.dismiss(id, cx));
                },
            );
            keep_press("notice-action", 1., button)
        });
        let close_button =
            Button::icon_xs(SharedString::from(format!("notice-close-{id}")), Glyph::X).tooltip(CLOSE_LABEL).on_click(move |_, _, cx| {
                _ = this.update(cx, |n, cx| n.dismiss(id, cx));
            });
        let close = keep_press("notice-close", close_opacity, close_button);
        h_flex()
            .relative()
            .w(width - px(2.))
            .items_start()
            .gap_3()
            .py_3()
            .pr_2()
            .pl_3()
            .child(div().flex_none().mt(px(2.)).child(Icon::new(glyph).with_size(px(16.)).text_color(tone)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.foreground)
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(notice.text.clone()),
                    )
                    .children(
                        notice.place.clone().map(|place| {
                            div().truncate().text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(place)
                        }),
                    )
                    .children(notice.detail.clone().map(|detail| {
                        div().mt(px(2.)).text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(detail)
                    })),
            )
            .children(action)
            .child(close)
            .child(
                canvas(
                    move |bounds, window, _| {
                        let own = bounds.size.height + px(2.);
                        if heights.borrow_mut().insert(id, own) != Some(own) {
                            window.refresh();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swipe_up_or_right_dismisses_down_or_left_does_not() {
        let began = Instant::now();
        let mut swipe = Swipe { id: 1, from: point(px(100.), px(100.)), began, axis: None, amount: point(px(0.), px(0.)) };
        swipe.moved(point(px(150.), px(101.)));
        assert_eq!(swipe.axis, Some(Axis::X));
        assert_eq!(swipe.dismisses(began + Duration::from_secs(5)), Some(Axis::X));
        swipe.moved(point(px(50.), px(101.)));
        assert!(f32::from(swipe.amount.x) > -50., "a pull left is damped");
        assert_eq!(swipe.dismisses(began + Duration::from_secs(5)), None);
        assert_eq!(swipe.dismisses(began), None, "however fast");
    }

    #[test]
    fn the_stack_sits_centre_top_right_and_spans_a_narrow_window() {
        assert_eq!(place(px(256.), px(1200.), px(1200.)), (px(1200. - 20. - 356.), WIDTH));
        assert_eq!(place(px(0.), px(500.), px(500.)), (px(12.), px(476.)));
    }
}
