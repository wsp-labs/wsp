// SPDX-License-Identifier: AGPL-3.0-only
//! The layer the window's dialogs stand on (`components/ui/dialog-styles.ts`): the page under a 4 px blur at 60% (64%
//! in a dark theme), the open dialog centred inside the viewport's 16 px, the pointer kept from what lies under it.
//! Escape closes the top dialog unless a control inside it took the key first; a press on the scrim closes a step
//! dialog and never an alert, as Radix's `Dialog` and `AlertDialog` differ.

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::base::animation::cubic_bezier;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, Animation, AnimationExt as _, AnyElement, AnyView, App, ElementId, FocusHandle, Global,
    KeyDownEvent, MouseButton, Window,
};

use crate::ui::status::still;
use crate::ui::ActivePalette;

/// `duration-200` on the backdrop's and the popup's opacity.
const FADE: Duration = Duration::from_millis(200);

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
}

#[derive(Default)]
struct Layer {
    stack: Vec<Open>,
    focus: Option<FocusHandle>,
    serial: usize,
}

impl Global for Layer {}

/// Stands `view` over the window under the name `key`, replacing a dialog of the same name already open, and takes
/// the keys so Escape reaches it.
pub fn open(key: &'static str, view: AnyView, closes: Closes, on_close: Option<OnClose>, window: &mut Window, cx: &mut App) {
    let fresh = cx.focus_handle();
    let layer = cx.default_global::<Layer>();
    layer.serial += 1;
    let serial = layer.serial;
    layer.stack.retain(|o| o.key != key);
    layer.stack.push(Open { key, view, closes, on_close, held: false, serial });
    let focus = layer.focus.get_or_insert(fresh).clone();
    window.focus(&focus, cx);
    window.refresh();
}

/// Takes the dialog named `key` down without running its close hook, as its own Close or Done does.
pub fn close(key: &'static str, window: &mut Window, cx: &mut App) {
    if let Some(layer) = cx.try_global::<Layer>() {
        if layer.stack.iter().any(|o| o.key == key) {
            cx.global_mut::<Layer>().stack.retain(|o| o.key != key);
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
    window.refresh();
    if let Some(on_close) = open.on_close {
        on_close(window, cx);
    }
}

/// The layer for the window's root to draw last; nothing while no dialog stands.
pub fn draw(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let layer = cx.try_global::<Layer>()?;
    let top = layer.stack.last()?;
    let (view, closes, serial) = (top.view.clone(), top.closes, top.serial);
    let focus = layer.focus.clone()?;
    let p = cx.palette();
    let size = window.viewport_size();
    let scrim = p.background.opacity(if cx.theme().is_dark() { 0.64 } else { 0.6 });
    let surface = div().id("dialog-surface").occlude().child(view);
    let backdrop = div().absolute().top_0().left_0().size_full().bg(scrim).backdrop_blur(px(4.));
    let (surface, backdrop) = if still(cx) {
        (surface.into_any_element(), backdrop.into_any_element())
    } else {
        let id = |name: &'static str| ElementId::NamedInteger(name.into(), serial as u64);
        // The popup's `ease-in-out` and the backdrop's Tailwind default curve.
        let popup = Animation::new(FADE).with_easing(cubic_bezier(0.42, 0., 0.58, 1.));
        let scrim = Animation::new(FADE).with_easing(cubic_bezier(0.4, 0., 0.2, 1.));
        let surface = surface.with_animation(id("dialog-in"), popup, |el, t| el.opacity(t)).into_any_element();
        (surface, backdrop.with_animation(id("scrim-in"), scrim, |el, t| el.opacity(t)).into_any_element())
    };
    let scrim = div()
        .id("dialog-layer")
        .debug_selector(|| "dialogs-layer".into())
        .track_focus(&focus)
        .on_key_down(|e: &KeyDownEvent, window, cx| {
            if e.keystroke.key == "escape" {
                cx.stop_propagation();
                dismiss(window, cx);
            }
        })
        .occlude()
        .w(size.width)
        .h(size.height)
        .p(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .child(backdrop)
        .when(closes == Closes::Outside, |el| el.on_mouse_down(MouseButton::Left, |_, window, cx| dismiss(window, cx)))
        .child(surface);
    Some(deferred(anchored().position(point(px(0.), px(0.))).child(scrim)).with_priority(4).into_any_element())
}
