// SPDX-License-Identifier: AGPL-3.0-only
//! The switch (`switch.tsx`): on or off in a row's slot, drawn from what the caller holds, handing the next value to
//! `on_change`.

use std::rc::Rc;

use std::time::Duration;

use gpui_kit::base::{Easing, Transition};
use gpui_kit::component::h_flex;
use gpui_kit::{div, prelude::*, px, App, MouseButton, Pixels, SharedString, Window};

use super::controls::Dim;
use super::focus::ring;
use super::motion::{ease_ink, ease_px_by, TRACK};
use super::theme::ActivePalette;

/// `--thumb-size` from 640 px up: `--spacing(4)`.
const THUMB: Pixels = px(16.);
/// `--thumb-size` under 640 px: `--spacing(5)`.
const THUMB_NARROW: Pixels = px(20.);

/// CSS's own `ease`, the thumb's curve.
fn css_ease(duration: Duration) -> Transition {
    Transition::new(duration).easing(Easing::cubic_bezier(0.25, 0.1, 0.25, 1.).expect("a valid curve"))
}

type Change<T> = Rc<dyn Fn(&T, &mut Window, &mut App) + 'static>;

/// `Switch`: on or off in a row's slot. The track takes the primary once on and the thumb slides over; held, it keeps
/// its look at 64% and takes no press.
#[derive(IntoElement)]
pub struct Switch {
    id: SharedString,
    on: bool,
    held: bool,
    label: Option<SharedString>,
    on_change: Option<Change<bool>>,
}

impl Switch {
    pub fn new(id: impl Into<SharedString>, on: bool) -> Self {
        Switch { id: id.into(), on, held: false, label: None, on_change: None }
    }

    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    /// The name a screen reader reads for the switch, `aria-label`.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Called with the value a press asks for, the opposite of the one drawn.
    pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let (track, thumb) = if self.on { (p.primary, p.primary_foreground) } else { (p.input, p.foreground) };
        let (track, thumb) = if self.held {
            let dim = Dim::new(p.background, track);
            (dim.fill(), dim.over(thumb))
        } else {
            (track, thumb)
        };
        let id = self.id.clone();
        let size = if super::narrow::width(window).sm { THUMB_NARROW } else { THUMB };
        let track = ease_ink(id.clone(), "track", track, TRACK, window, cx);
        // `translate .15s`, and the press's `scale-x-110` as `scale .1s .1s`, held from its edge at the thumb's side.
        let pressed = window.use_keyed_state(SharedString::from(format!("{id}-pressed")), cx, |_, _| false);
        let stretch = *pressed.read(cx) && !self.held;
        let x = ease_px_by(id.clone(), "x", if self.on { size - px(4.) } else { px(0.) }, css_ease(Duration::from_millis(150)), window, cx);
        let wide = if stretch { size * 1.1 } else { size };
        let delay = css_ease(Duration::from_millis(100)).delay(Duration::from_millis(100));
        let w = ease_px_by(id.clone(), "w", wide, delay, window, cx);
        let lead = if self.on { x + size - w } else { x };
        let on = self.on;
        let (down, up, out) = (pressed.clone(), pressed.clone(), pressed);
        let set = |state: &gpui_kit::Entity<bool>, to: bool, cx: &mut App| {
            state.update(cx, |s, cx| {
                if *s != to {
                    *s = to;
                    cx.notify();
                }
            })
        };
        h_flex()
            .id(self.id)
            .debug_selector(|| "switch".into())
            .when_some(self.label, |el, label| el.aria_label(label))
            .flex_none()
            .w(size * 2. - px(2.))
            .h(size + px(2.))
            .p(px(1.))
            .rounded_full()
            .bg(track)
            .child(
                div()
                    .debug_selector(|| "switch-thumb".into())
                    .ml(lead)
                    .h(size)
                    .w(w)
                    .rounded_full()
                    .bg(thumb)
                    .shadow(p.thumb_shadow.clone()),
            )
            .when(!self.held, |el| {
                el.on_mouse_down(MouseButton::Left, move |_, _, cx| set(&down, true, cx))
                    .on_mouse_up(MouseButton::Left, move |_, _, cx| set(&up, false, cx))
                    .on_mouse_up_out(MouseButton::Left, move |_, _, cx| set(&out, false, cx))
            })
            .when(!self.held, |el| {
                el.cursor_pointer()
                    .tab_index(0)
                    .focus_visible(|s| s.shadow(vec![ring(p.ring)]))
                    .when_some(self.on_change, |el, f| el.on_click(move |_, window, cx| f(&!on, window, cx)))
            })
    }
}
