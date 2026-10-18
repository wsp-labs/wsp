// SPDX-License-Identifier: AGPL-3.0-only
//! The layer the window's dialogs stand on (`components/ui/dialog-styles.ts`): the page under a 4 px blur at 60% (64%
//! in a dark theme), the open dialog centred inside the viewport's 16 px, the pointer kept from what lies under it.
//! Escape closes the top dialog unless a control inside it took the key first; a press on the scrim closes a step
//! dialog and never an alert, as Radix's `Dialog` and `AlertDialog` differ.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::base::animation::cubic_bezier;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, AnyView, App, ElementId, FocusHandle, Global, KeyDownEvent, MouseButton,
    Window,
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
}

#[derive(Default)]
struct Layer {
    stack: Vec<Open>,
    focus: Option<FocusHandle>,
    /// Where focus stood when the first dialog opened, given back once the last one goes.
    back: Option<FocusHandle>,
    serial: usize,
    /// When the backdrop began to fade in, the first dialog of the stack opening.
    shown: Option<Instant>,
}

impl Global for Layer {}

/// Stands `view` over the window under the name `key`, replacing a dialog of the same name already open as a close
/// does, and takes the keys so Escape reaches it. One held while its work runs stays, and the new one does not open.
pub fn open(key: &'static str, view: AnyView, closes: Closes, on_close: Option<OnClose>, window: &mut Window, cx: &mut App) {
    if cx.try_global::<Layer>().is_some_and(|l| l.stack.iter().any(|o| o.key == key && o.held)) {
        return;
    }
    let fresh = cx.focus_handle();
    let before = window.focused(cx);
    let layer = cx.default_global::<Layer>();
    if layer.stack.is_empty() {
        layer.back = before.filter(|f| layer.focus.as_ref() != Some(f));
        layer.shown = Some(Instant::now());
    }
    layer.serial += 1;
    let serial = layer.serial;
    let replaced = layer.stack.iter().position(|o| o.key == key).map(|at| layer.stack.remove(at));
    let nested = !layer.stack.is_empty();
    layer.stack.push(Open { key, view, closes, on_close, held: false, serial, nested, depth: 0, from: 0, moved: 0, since: Instant::now() });
    let focus = layer.focus.get_or_insert(fresh).clone();
    window.focus(&focus, cx);
    window.refresh();
    if let Some(on_close) = replaced.and_then(|o| o.on_close) {
        on_close(window, cx);
    }
}

/// Takes the dialog named `key` down without running its close hook, as its own Close or Done does.
pub fn close(key: &'static str, window: &mut Window, cx: &mut App) {
    if let Some(layer) = cx.try_global::<Layer>() {
        if layer.stack.iter().any(|o| o.key == key) {
            cx.global_mut::<Layer>().stack.retain(|o| o.key != key);
            give_back(window, cx);
            window.refresh();
        }
    }
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
    let Some(open) = cx.global_mut::<Layer>().stack.pop() else { return };
    give_back(window, cx);
    window.refresh();
    if let Some(on_close) = open.on_close {
        on_close(window, cx);
    }
}

/// Once no dialog stands, focus goes back where it stood before the first opened, as Radix's `Dialog` returns it.
fn give_back(window: &mut Window, cx: &mut App) {
    let layer = cx.global_mut::<Layer>();
    if let Some(back) = layer.stack.is_empty().then(|| layer.back.take()).flatten() {
        window.focus(&back, cx);
    }
}

/// Gives the keys back to the top dialog once what held them inside it is no longer drawn, as a step that moves on
/// drops its field; false while no dialog stands.
pub fn refocus(window: &mut Window, cx: &mut App) -> bool {
    let Some(focus) = cx.try_global::<Layer>().filter(|l| !l.stack.is_empty()).and_then(|l| l.focus.clone()) else { return false };
    window.focus(&focus, cx);
    true
}

/// One dialog as the layer draws it this frame.
struct Drawn {
    view: AnyView,
    serial: usize,
    nested: bool,
    depth: usize,
    from: usize,
    moved: usize,
    since: Instant,
}

/// Where a dialog stands `depth` dialogs under the top (`--nested-dialogs`): up 1.25rem and 0.1 fainter a level, and
/// a sheet under another not shown at all. GPUI has no scale on a div, so the web's 0.1 smaller a level is not drawn.
fn under(depth: f32, sheet: bool) -> (f32, f32) {
    let opacity = if sheet { 1. - depth.min(1.) } else { 1. - 0.1 * depth };
    (-NESTED_UP * depth, opacity)
}

/// The layer for the window's root to draw last; nothing while no dialog stands. Every dialog open is drawn, the ones
/// under the top up and fainter by how many stand over them; only the top takes presses and keys. The fades and moves
/// run under reduced motion too, as the web's dialogs keep theirs, so they are timed here rather than by
/// `with_animation`, which holds still then; only captures hold them still.
pub fn draw(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    cx.try_global::<Layer>()?;
    let now = Instant::now();
    let layer = cx.global_mut::<Layer>();
    let len = layer.stack.len();
    let closes = layer.stack.last()?.closes;
    let focus = layer.focus.clone()?;
    let shown = layer.shown.unwrap_or(now);
    let mut drawn = Vec::with_capacity(len);
    for (at, open) in layer.stack.iter_mut().enumerate() {
        let depth = len - 1 - at;
        if depth != open.depth {
            (open.from, open.depth, open.moved, open.since) = (open.depth, depth, open.moved + 1, now);
        }
        drawn.push(Drawn {
            view: open.view.clone(),
            serial: open.serial,
            nested: open.nested,
            depth,
            from: open.from,
            moved: open.moved,
            since: open.since,
        });
    }
    let held = cx.try_global::<Still>().is_some_and(|s| s.0);
    let progress =
        |since: Instant| if held { 1. } else { (now.saturating_duration_since(since).as_secs_f32() / FADE.as_secs_f32()).min(1.) };
    if !held && (progress(shown) < 1. || drawn.iter().any(|d| progress(d.since) < 1.)) {
        window.request_animation_frame();
    }
    let sheet = crate::ui::narrow(window);
    let size = window.viewport_size();
    // The popup's `ease-in-out` and the backdrop's Tailwind default curve.
    let popup = cubic_bezier(0.42, 0., 0.58, 1.);
    let backdrop = backdrop(cx).absolute().top_0().left_0().size_full().opacity(tailwind_ease().sample(progress(shown)));
    let placed = |surface: AnyElement| {
        div().absolute().top_0().left_0().w(size.width).h(size.height).p(px(16.)).flex().items_center().justify_center().child(surface)
    };
    let mut layers = vec![backdrop.into_any_element()];
    for (at, d) in drawn.into_iter().enumerate() {
        let t = popup(progress(d.since));
        let surface = div().id(ElementId::NamedInteger("dialog-surface".into(), d.serial as u64)).relative().child(d.view);
        let surface = if at + 1 == len { surface.occlude() } else { surface };
        let (y, opacity) = if d.moved == 0 {
            // `data-starting-style`: in from 1rem below as a sheet, 2rem as a nested popup, else faded in alone.
            let rise = if sheet {
                SHEET_RISE
            } else if d.nested {
                NESTED_RISE
            } else {
                0.
            };
            (rise * (1. - t), t)
        } else {
            under(d.from as f32 + (d.depth as f32 - d.from as f32) * t, sheet)
        };
        if at + 1 == len {
            // Over the dialogs under the top, so a press on one of them closes the top as a press outside it does.
            let outside = div().absolute().top_0().left_0().size_full().occlude();
            let outside =
                outside.when(closes == Closes::Outside, |el| el.on_mouse_down(MouseButton::Left, |_, window, cx| dismiss(window, cx)));
            layers.push(outside.into_any_element());
        }
        layers.push(placed(surface.top(px(y)).opacity(opacity).into_any_element()).into_any_element());
    }
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
        .relative()
        .w(size.width)
        .h(size.height)
        .children(layers);
    Some(deferred(anchored().position(point(px(0.), px(0.))).child(root)).with_priority(4).into_any_element())
}
