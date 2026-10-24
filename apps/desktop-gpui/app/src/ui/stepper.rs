// SPDX-License-Identifier: AGPL-3.0-only
//! `Stepper` (`computerSettings.tsx`): a setting's number with a step either side, at least one. Each end is a ghost
//! icon-xs `Button` at the stepper's full height, square where it meets the number, so it is a tab stop with the focus
//! ring and takes Enter and Space as the web's buttons do; at its limit it is held.

use std::rc::Rc;

use gpui_kit::component::h_flex;
use gpui_kit::{div, prelude::*, px, App, Corners, Pixels, SharedString, Window};

use super::controls::Button;
use super::icons::Glyph;
use super::theme::ActivePalette;
use super::{size, text};

type Change = Rc<dyn Fn(u64, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Stepper {
    id: SharedString,
    value: u64,
    max: Option<u64>,
    on_change: Option<Change>,
}

impl Stepper {
    pub fn new(id: impl Into<SharedString>, value: u64) -> Self {
        Stepper { id: id.into(), value, max: None, on_change: None }
    }

    /// The most it steps up to; the plus is held there.
    pub fn max(mut self, max: Option<u64>) -> Self {
        self.max = max;
        self
    }

    /// Called with the number one step away.
    pub fn on_change(mut self, f: impl Fn(u64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

/// One end, `h-full rounded-r-none` or `rounded-l-none`: the button's radius on the outer corners only.
fn end(id: SharedString, glyph: Glyph, held: bool, outer_left: bool, to: u64, on_change: Option<Change>) -> Button {
    let r = size::BUTTON_RADIUS;
    let zero = Pixels::ZERO;
    let corners = if outer_left {
        Corners { top_left: r, bottom_left: r, top_right: zero, bottom_right: zero }
    } else {
        Corners { top_left: zero, bottom_left: zero, top_right: r, bottom_right: r }
    };
    Button::icon_xs(id, glyph).height(size::CONTROL_HEIGHT - px(2.)).corners(corners).held(held).on_click(move |_, window, cx| {
        if let Some(f) = on_change.as_ref() {
            f(to, window, cx)
        }
    })
}

impl RenderOnce for Stepper {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let value = self.value;
        h_flex()
            .id(SharedString::from(format!("{}-stepper", self.id)))
            .debug_selector(|| "stepper".into())
            .flex_none()
            .h(size::CONTROL_HEIGHT)
            .rounded(size::CONTROL_RADIUS)
            .border_1()
            .border_color(p.border)
            .child(end(
                format!("{}-fewer", self.id).into(),
                Glyph::Minus,
                value <= 1,
                true,
                value.saturating_sub(1),
                self.on_change.clone(),
            ))
            .child(
                div()
                    .debug_selector(|| "stepper-value".into())
                    .min_w(px(32.))
                    .px_1()
                    .text_center()
                    .text_size(text::FACT.0)
                    .text_color(p.foreground)
                    .font_features(text::tabular())
                    .child(value.to_string()),
            )
            .child(end(
                format!("{}-more", self.id).into(),
                Glyph::Plus,
                self.max.is_some_and(|m| value >= m),
                false,
                value + 1,
                self.on_change,
            ))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, Context, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext, Window};

    use super::Stepper;
    use crate::ui::{self, Side};

    struct Lone {
        value: Rc<RefCell<u64>>,
        max: Option<u64>,
    }

    impl Render for Lone {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.clone();
            let this = cx.entity();
            div().p(px(24.)).child(Stepper::new("threads", *self.value.borrow()).max(self.max).on_change(move |next, _, cx| {
                *value.borrow_mut() = next;
                this.update(cx, |_, cx| cx.notify());
            }))
        }
    }

    fn press(cx: &mut VisualTestContext, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.simulate_event(KeyDownEvent { keystroke: keystroke.clone(), is_held: false, prefer_character_input: false });
        cx.simulate_event(KeyUpEvent { keystroke });
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn tab_reaches_both_ends_and_the_keys_step(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let value = Rc::new(RefCell::new(2));
        let view = cx.new(|_| Lone { value: value.clone(), max: Some(3) });
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(300.), px(120.)));
        cx.run_until_parked();

        let end = cx.debug_bounds("button").expect("an end is drawn");
        assert_eq!((end.size.width, end.size.height), (px(24.), px(28.)), "each end is `size-6 h-full` inside the 30 px edge");
        let focused = |cx: &mut VisualTestContext| cx.update(|window, cx| window.focused(cx));
        // With nothing focused a test window's keys reach no key context; Root's Tab runs `focus_next`.
        cx.update(|window, cx| window.focus_next(cx));
        let fewer = focused(cx).expect("Tab reaches the minus");
        cx.simulate_keystrokes("tab");
        let more = focused(cx).expect("Tab reaches the plus");
        assert_ne!(more, fewer);
        cx.simulate_keystrokes("shift-tab");
        assert_eq!(focused(cx), Some(fewer), "Shift-Tab goes back to the minus");
        press(cx, "enter");
        assert_eq!(*value.borrow(), 1, "Enter on the minus steps down");
        cx.update(|window, cx| window.focus_next(cx));
        assert_eq!(focused(cx), Some(more), "the minus at one is held and no tab stop, so Tab lands on the plus");
        press(cx, "space");
        assert_eq!(*value.borrow(), 2, "Space on the plus steps up");
        press(cx, "enter");
        assert_eq!(*value.borrow(), 3);
        press(cx, "enter");
        assert_eq!(*value.borrow(), 3, "at its most the plus is held and steps no further");
    }
}
