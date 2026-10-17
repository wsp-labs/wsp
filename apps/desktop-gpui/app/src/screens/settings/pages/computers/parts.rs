// SPDX-License-Identifier: AGPL-3.0-only
//! Pieces the Computers pages draw that `ui/` does not hold yet, each after the web piece of the same name: a
//! machine's `StateMark` and the computer page's `Stepper`.

use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, ClickEvent, SharedString, Window};

use super::words::Mark;
use crate::ui::{size, spinner, text, tip, ActivePalette, Glyph};

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// `opacity-64`, a held control's look.
const HELD: f32 = 0.64;

/// `StateMark`: a machine's state as one 14 px icon in its ink, the sentence on its hover; Ready draws nothing.
pub fn state_mark(id: impl Into<SharedString>, mark: Mark, why: Option<String>, cx: &App) -> Option<AnyElement> {
    let p = cx.palette();
    let (inner, word): (AnyElement, &str) = match mark {
        Mark::Ready => return None,
        Mark::Working => (spinner(cx).into_any_element(), "Setting up"),
        Mark::NeedsYou => {
            (Icon::new(Glyph::MessageCircleQuestionMark).with_size(px(14.)).text_color(p.status_input).into_any_element(), "Needs you")
        }
        Mark::Failed => (Icon::new(Glyph::CircleAlert).with_size(px(14.)).text_color(p.status_failed).into_any_element(), "Failed"),
        Mark::Pending => (Icon::new(Glyph::CircleDashed).with_size(px(14.)).text_color(p.muted_foreground).into_any_element(), "Pending"),
        Mark::Offline => (Icon::new(Glyph::Plug).with_size(px(14.)).text_color(p.muted_foreground).into_any_element(), "Offline"),
    };
    let label = why.unwrap_or_else(|| word.to_owned());
    Some(
        div()
            .id(id.into())
            .debug_selector(|| "state-mark".into())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .tooltip(tip(label))
            .child(inner)
            .into_any_element(),
    )
}

/// One end of a `Stepper`: a ghost square at the stepper's full height.
fn step_end(id: SharedString, glyph: Glyph, held: bool, left: bool, cx: &App, on_click: Click) -> impl IntoElement {
    let p = cx.palette();
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(24.))
        .h_full()
        .map(|el| if left { el.rounded_l(size::CONTROL_RADIUS - px(1.)) } else { el.rounded_r(size::CONTROL_RADIUS - px(1.)) })
        .when(held, |el| el.opacity(HELD))
        .child(Icon::new(glyph).with_size(px(14.)).text_color(p.muted_foreground))
        .when(!held, |el| el.cursor_pointer().hover(|s| s.bg(p.accent)).on_click(move |e, window, cx| on_click(e, window, cx)))
}

/// `Stepper`: a setting's number with a step either side, at least one.
pub fn stepper(
    id: &str,
    value: u64,
    max: Option<u64>,
    cx: &App,
    on_change: impl Fn(u64, &mut Window, &mut App) + Clone + 'static,
) -> impl IntoElement {
    let p = cx.palette();
    let fewer = on_change.clone();
    h_flex()
        .id(SharedString::from(format!("{id}-stepper")))
        .debug_selector(|| "stepper".into())
        .flex_none()
        .h(size::CONTROL_HEIGHT)
        .rounded(size::CONTROL_RADIUS)
        .border_1()
        .border_color(p.border)
        .child(step_end(
            format!("{id}-fewer").into(),
            Glyph::Minus,
            value <= 1,
            true,
            cx,
            Box::new(move |_, window, cx| fewer(value.saturating_sub(1), window, cx)),
        ))
        .child(div().min_w(px(32.)).px_1().text_center().text_size(text::FACT).text_color(p.foreground).child(value.to_string()))
        .child(step_end(
            format!("{id}-more").into(),
            Glyph::Plus,
            max.is_some_and(|m| value >= m),
            false,
            cx,
            Box::new(move |_, window, cx| on_change(value + 1, window, cx)),
        ))
}
