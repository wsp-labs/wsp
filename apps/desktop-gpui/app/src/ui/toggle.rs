// SPDX-License-Identifier: AGPL-3.0-only
//! The switch (`switch.tsx`): on or off in a row's slot, drawn from what the caller holds, handing the next value to
//! `on_change`.

use std::rc::Rc;

use gpui_kit::component::h_flex;
use gpui_kit::{div, prelude::*, px, App, Pixels, SharedString, Window};

use super::controls::Dim;
use super::focus::ring;
use super::motion::{ease_ink, ease_px, COLORS, TRACK};
use super::theme::ActivePalette;

/// `--thumb-size` from 640 px up: `--spacing(4)`.
const THUMB: Pixels = px(16.);

type Change<T> = Rc<dyn Fn(&T, &mut Window, &mut App) + 'static>;

/// `Switch`: on or off in a row's slot. The track takes the primary once on and the thumb slides over; held, it keeps
/// its look at 64% and takes no press.
#[derive(IntoElement)]
pub struct Switch {
    id: SharedString,
    on: bool,
    held: bool,
    on_change: Option<Change<bool>>,
}

impl Switch {
    pub fn new(id: impl Into<SharedString>, on: bool) -> Self {
        Switch { id: id.into(), on, held: false, on_change: None }
    }

    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
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
        let track = ease_ink(id.clone(), "track", track, TRACK, window, cx);
        let thumb = ease_ink(id.clone(), "thumb", thumb, COLORS, window, cx);
        let x = ease_px(id.clone(), "x", if self.on { THUMB - px(4.) } else { px(0.) }, COLORS, window, cx);
        let on = self.on;
        h_flex()
            .id(self.id)
            .debug_selector(|| "switch".into())
            .flex_none()
            .w(THUMB * 2. - px(2.))
            .h(THUMB + px(2.))
            .p(px(1.))
            .rounded_full()
            .bg(track)
            .child(div().ml(x).size(THUMB).rounded_full().bg(thumb).shadow(p.thumb_shadow.clone()))
            .when(!self.held, |el| {
                el.cursor_pointer()
                    .tab_index(0)
                    .focus_visible(|s| s.shadow(vec![ring(p.ring)]))
                    .when_some(self.on_change, |el, f| el.on_click(move |_, window, cx| f(&!on, window, cx)))
            })
    }
}
