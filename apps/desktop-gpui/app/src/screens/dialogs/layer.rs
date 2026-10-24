// SPDX-License-Identifier: AGPL-3.0-only
//! The layer the window's dialogs stand on (`components/ui/dialog-styles.ts`): the page under a 4 px blur at 60% (64%
//! in a dark theme), the open dialog centred inside the viewport's 16 px, the pointer kept from what lies under it.
//! Escape closes the top dialog unless a control inside it took the key first; a press on the scrim closes a step
//! dialog and never an alert, as Radix's `Dialog` and `AlertDialog` differ. A dialog that closes stays drawn for its
//! `data-ending-style` fade and move, and nothing reaches it while it leaves.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::base::animation::cubic_bezier;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, AnyView, App, Bounds, ContentMask, Element, ElementId, FocusHandle, Global,
    GlobalElementId, InspectorElementId, KeyDownEvent, LayoutId, MouseButton, Pixels, Window,
};

use crate::ui::dialog::backdrop;
use crate::ui::motion::tailwind_ease;
use crate::ui::Still;

/// `duration-200` on the backdrop's and the popup's opacity and moves.
const FADE: Duration = Duration::from_millis(200);
/// `-translate-y-[calc(1.25rem*var(--nested-dialogs))]`: a dialog under another stands this much higher a level.
const NESTED_UP: f32 = 20.;
/// `data-nested:data-starting-style:translate-y-8`: a dialog opened over another comes in from this far below.
const NESTED_RISE: f32 = 32.;
/// `max-sm:data-starting-style:translate-y-4`: a sheet comes in from this far below.
const SHEET_RISE: f32 = 16.;
/// `data-nested:data-ending-style:translate-y-8`: a nested dialog leaves this far down.
const NESTED_DROP: f32 = 32.;
/// `max-sm:data-ending-style:translate-y-4`: a sheet leaves this far down.
const SHEET_DROP: f32 = 16.;

/// What runs when a dialog closes by Escape or the scrim, as the web's `onOpenChange(false)`.
pub type OnClose = Rc<dyn Fn(&mut Window, &mut App)>;

/// How a dialog leaves besides its own buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closes {
    /// A step dialog: Escape and a press on the scrim.
    Outside,
    /// An alert: Escape only; a press outside is no answer.
    Escape,
}

struct Open {
    key: &'static str,
    view: AnyView,
    closes: Closes,
    on_close: Option<OnClose>,
    held: bool,
    serial: usize,
    /// Opened over another dialog (`data-nested`).
    nested: bool,
    /// How many dialogs stand over it as last drawn, the depth it moved from, and how many times it moved.
    depth: usize,
    from: usize,
    moved: usize,
    /// When its last move began: its rise in, or its move to the depth it stands at.
    since: Instant,
    /// Where it stood and how opaque it was as last drawn, where it leaves from.
    drawn: (f32, f32),
}

/// A closed dialog fading and moving out (`data-ending-style`), held only until its fade is done.
struct Leaving {
    view: AnyView,
    serial: usize,
    /// Where it stood and how opaque it was as it closed, and where it moves down to.
    from: (f32, f32),
    to: f32,
    since: Instant,
}

#[derive(Default)]
struct Layer {
    stack: Vec<Open>,
    leaving: Vec<Leaving>,
    focus: Option<FocusHandle>,
    /// Held by the dialogs leaving, so focus that lands in one (as Tab can) is taken out.
    guard: Option<FocusHandle>,
    /// Where focus stood when the first dialog opened, given back once the last one goes.
    back: Option<FocusHandle>,
    serial: usize,
    /// The backdrop's opacity when it last began to move and when: in while a dialog stands, out once none does.
    shade: Option<(f32, Instant)>,
}

impl Global for Layer {}

impl Layer {
    fn shade(&self, now: Instant, held: bool) -> f32 {
        let to = if self.stack.is_empty() { 0. } else { 1. };
        self.shade.map_or(to, |(from, since)| from + (to - from) * tailwind_ease().sample(progress(since, now, held)))
    }

    /// Takes the dialog at `at` off the stack and keeps it drawn while it leaves; the backdrop goes out once none stands.
    fn take(&mut self, at: usize, sheet: bool, now: Instant, held: bool) -> Open {
        let shade = self.shade(now, held);
        let open = self.stack.remove(at);
        if self.stack.is_empty() {
            self.shade = Some((shade, now));
        }
        if open.drawn.1 > 0. {
            let to = if sheet {
                SHEET_DROP
            } else if open.nested {
                NESTED_DROP
            } else {
                open.drawn.0
            };
            self.leaving.push(Leaving { view: open.view.clone(), serial: open.serial, from: open.drawn, to, since: now });
        }
        open
    }
}

/// Whether captures hold every move at its end, the one thing that stops the layer's fades.
fn held(cx: &App) -> bool {
    cx.try_global::<Still>().is_some_and(|s| s.0)
}

fn progress(since: Instant, now: Instant, held: bool) -> f32 {
    if held {
        1.
    } else {
        (now.saturating_duration_since(since).as_secs_f32() / FADE.as_secs_f32()).min(1.)
    }
}

/// Stands `view` over the window under the name `key`, replacing a dialog of the same name already open as a close
/// does, and takes the keys so Escape reaches it. One held while its work runs stays, and the new one does not open.
pub fn open(key: &'static str, view: AnyView, closes: Closes, on_close: Option<OnClose>, window: &mut Window, cx: &mut App) {
    if cx.try_global::<Layer>().is_some_and(|l| l.stack.iter().any(|o| o.key == key && o.held)) {
        return;
    }
    let (fresh, guard) = (cx.focus_handle(), cx.focus_handle());
    let before = window.focused(cx);
    let (now, held, sheet) = (Instant::now(), held(cx), crate::ui::narrow(window));
    let layer = cx.default_global::<Layer>();
    if layer.stack.is_empty() {
        layer.back = before.filter(|f| layer.focus.as_ref() != Some(f));
        layer.shade = Some((layer.shade(now, held), now));
    }
    // A view opened again while it still leaves is drawn once, standing.
    layer.leaving.retain(|l| l.view.entity_id() != view.entity_id());
    layer.serial += 1;
    let serial = layer.serial;
    let replaced = layer.stack.iter().position(|o| o.key == key).map(|at| layer.take(at, sheet, now, held));
    let nested = !layer.stack.is_empty();
    let drawn = (0., 0.);
    layer.stack.push(Open { key, view, closes, on_close, held: false, serial, nested, depth: 0, from: 0, moved: 0, since: now, drawn });
    layer.guard.get_or_insert(guard);
    let focus = layer.focus.get_or_insert(fresh).clone();
    window.focus(&focus, cx);
    window.refresh();
    if let Some(on_close) = replaced.and_then(|o| o.on_close) {
        on_close(window, cx);
    }
}

/// Takes the dialog named `key` down without running its close hook, as its own Close or Done does.
pub fn close(key: &'static str, window: &mut Window, cx: &mut App) {
    let Some(at) = cx.try_global::<Layer>().and_then(|l| l.stack.iter().position(|o| o.key == key)) else { return };
    let (now, held, sheet) = (Instant::now(), held(cx), crate::ui::narrow(window));
    let layer = cx.global_mut::<Layer>();
    let top = at + 1 == layer.stack.len();
    layer.take(at, sheet, now, held);
    give_back(top, window, cx);
    window.refresh();
}

/// Holds the dialog named `key` against Escape and the scrim while `held`, as a dialog does while its work runs.
pub fn hold(key: &'static str, held: bool, cx: &mut App) {
    if let Some(open) =
        cx.try_global::<Layer>().is_some().then(|| cx.global_mut::<Layer>()).and_then(|l| l.stack.iter_mut().find(|o| o.key == key))
    {
        open.held = held;
    }
}

/// Whether the dialog named `key` stands now.
pub fn is_open(key: &str, cx: &App) -> bool {
    cx.try_global::<Layer>().is_some_and(|l| l.stack.iter().any(|o| o.key == key))
}

/// The view of the dialog named `key`, while it stands.
pub fn view(key: &str, cx: &App) -> Option<AnyView> {
    cx.try_global::<Layer>()?.stack.iter().find(|o| o.key == key).map(|o| o.view.clone())
}

/// The name of the dialog on top, if any stands.
pub fn top(cx: &App) -> Option<&'static str> {
    cx.try_global::<Layer>().and_then(|l| l.stack.last().map(|o| o.key))
}

/// Closes the top dialog as Escape or a press outside does: its close hook runs after it is down.
pub fn dismiss(window: &mut Window, cx: &mut App) {
    if cx.try_global::<Layer>().is_none_or(|l| l.stack.last().is_none_or(|o| o.held)) {
        return;
    }
    let (now, held, sheet) = (Instant::now(), held(cx), crate::ui::narrow(window));
    let layer = cx.global_mut::<Layer>();
    let open = layer.take(layer.stack.len() - 1, sheet, now, held);
    give_back(true, window, cx);
    window.refresh();
    if let Some(on_close) = open.on_close {
        on_close(window, cx);
    }
}

/// Takes focus out of a dialog as it closes, since it stays drawn while it leaves: once no dialog stands, back where it
/// stood before the first opened, as Radix's `Dialog` returns it, and from a closed top to the one now on top.
fn give_back(top: bool, window: &mut Window, cx: &mut App) {
    let layer = cx.global_mut::<Layer>();
    let to = if layer.stack.is_empty() { layer.back.take().or_else(|| layer.focus.clone()) } else { layer.focus.clone().filter(|_| top) };
    if let Some(to) = to {
        window.focus(&to, cx);
    }
}

/// Gives the keys back to the top dialog once what held them inside it is no longer drawn, as a step that moves on
/// drops its field; false while no dialog stands.
pub fn refocus(window: &mut Window, cx: &mut App) -> bool {
    let Some(focus) = cx.try_global::<Layer>().filter(|l| !l.stack.is_empty()).and_then(|l| l.focus.clone()) else { return false };
    window.focus(&focus, cx);
    true
}

/// Where a dialog stands `depth` dialogs under the top (`--nested-dialogs`): up 1.25rem and 0.1 fainter a level, and
/// a sheet under another not shown at all. GPUI has no scale on a div, so the web's 0.1 smaller a level is not drawn.
fn under(depth: f32, sheet: bool) -> (f32, f32) {
    let opacity = if sheet { 1. - depth.min(1.) } else { 1. - 0.1 * depth };
    (-NESTED_UP * depth, opacity)
}

/// The layer for the window's root to draw last; nothing while no dialog stands or leaves. Every dialog open is drawn,
/// the ones under the top up and fainter by how many stand over them; only the top takes presses and keys. A closed one
/// is drawn over them for its fade out and is never reached. The fades and moves run under reduced motion too, as the
/// web's dialogs keep theirs, so they are timed here rather than by `with_animation`, which holds still then; only
/// captures hold them still, and a dialog closed under them is gone at once.
pub fn draw(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    cx.try_global::<Layer>()?;
    let (now, held, sheet) = (Instant::now(), held(cx), crate::ui::narrow(window));
    let size = window.viewport_size();
    let layer = cx.global_mut::<Layer>();
    layer.leaving.retain(|l| progress(l.since, now, held) < 1.);
    let shade = layer.shade(now, held);
    if layer.stack.is_empty() && layer.leaving.is_empty() && shade <= 0. {
        return None;
    }
    let focus = layer.focus.clone()?;
    let guard = layer.guard.clone()?;
    let closes = layer.stack.last().map(|o| o.closes);
    // The popup's `ease-in-out`; the backdrop eases on Tailwind's default curve.
    let popup = cubic_bezier(0.42, 0., 0.58, 1.);
    let len = layer.stack.len();
    let mut moving = layer.shade.is_some_and(|(_, since)| progress(since, now, held) < 1.) || !layer.leaving.is_empty();
    let mut drawn = Vec::with_capacity(len);
    for (at, open) in layer.stack.iter_mut().enumerate() {
        let depth = len - 1 - at;
        if depth != open.depth {
            (open.from, open.depth, open.moved, open.since) = (open.depth, depth, open.moved + 1, now);
        }
        let p = progress(open.since, now, held);
        moving |= p < 1.;
        let t = popup(p);
        open.drawn = if open.moved == 0 {
            // `data-starting-style`: in from 1rem below as a sheet, 2rem as a nested popup, else faded in alone.
            let rise = if sheet {
                SHEET_RISE
            } else if open.nested {
                NESTED_RISE
            } else {
                0.
            };
            (rise * (1. - t), t)
        } else {
            under(open.from as f32 + (depth as f32 - open.from as f32) * t, sheet)
        };
        drawn.push((open.view.clone(), open.serial, open.drawn));
    }
    let leaving: Vec<_> = layer
        .leaving
        .iter()
        .map(|l| {
            let t = popup(progress(l.since, now, held));
            (l.view.clone(), l.serial, (l.from.0 + (l.to - l.from.0) * t, l.from.1 * (1. - t)))
        })
        .collect();
    if moving {
        window.request_animation_frame();
    }
    if guard.contains_focused(window, cx) {
        window.focus(&focus, cx);
    }
    let placed = |surface: gpui_kit::Stateful<gpui_kit::Div>, (y, opacity): (f32, f32)| {
        let surface = surface.top(px(y)).opacity(opacity);
        div().absolute().top_0().left_0().w(size.width).h(size.height).p(px(16.)).flex().items_center().justify_center().child(surface)
    };
    let backdrop = backdrop(cx).absolute().top_0().left_0().size_full().opacity(shade);
    let mut layers = vec![backdrop.into_any_element()];
    for (at, (view, serial, place)) in drawn.into_iter().enumerate() {
        let surface = div().id(ElementId::NamedInteger("dialog-surface".into(), serial as u64)).relative().child(view);
        if at + 1 == len {
            // Over the dialogs under the top, so a press on one of them closes the top as a press outside it does.
            let outside = div().absolute().top_0().left_0().size_full().occlude();
            let outside = outside
                .when(closes == Some(Closes::Outside), |el| el.on_mouse_down(MouseButton::Left, |_, window, cx| dismiss(window, cx)));
            layers.push(outside.into_any_element());
            layers.push(placed(surface.occlude(), place).into_any_element());
        } else {
            layers.push(placed(surface, place).into_any_element());
        }
    }
    let leaving = leaving.into_iter().map(|(view, serial, place)| {
        placed(div().id(ElementId::NamedInteger("dialog-leaving".into(), serial as u64)).relative().child(view), place)
    });
    let leaving = div().absolute().top_0().left_0().size_full().track_focus(&guard).children(leaving);
    let frame = div().relative().w(size.width).h(size.height);
    let frame = if closes.is_some() {
        let root = div()
            .id("dialog-layer")
            .debug_selector(|| "dialogs-layer".into())
            .focus_trap("dialog-layer-trap", &focus)
            .on_key_down(|e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" {
                    cx.stop_propagation();
                    dismiss(window, cx);
                }
            })
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .children(layers);
        frame.child(root)
    } else {
        frame.children(layers)
    };
    let frame = frame.child(Inert(leaving.into_any_element()));
    Some(deferred(anchored().position(point(px(0.), px(0.))).child(frame)).with_priority(4).into_any_element())
}

/// Draws its child where no press can reach it: every hitbox the child lays down is clipped to nothing, so a press
/// falls through to whatever stands under it. Its paint keeps the window's clip.
struct Inert(AnyElement);

impl IntoElement for Inert {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Inert {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.0.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(Some(ContentMask { bounds: Bounds::default() }), |window| self.0.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.paint(window, cx);
    }
}
