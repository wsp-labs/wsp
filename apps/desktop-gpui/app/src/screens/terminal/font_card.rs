// SPDX-License-Identifier: AGPL-3.0-only
//! The font card under the toolbar's font button (`TerminalFontButton.tsx`): a label, a compact field for a family
//! installed on this computer, and the hint saying what an empty field draws with.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::v_flex;
use gpui_kit::{div, prelude::*, px, AnyElement, App, BoxShadow, Entity, SharedString};

use crate::ui::{text, ActivePalette};

/// `DEFAULT_TERMINAL_TEXT_FACES`' first face in `fontChain.ts`, the placeholder with nothing detected.
pub const FIRST_DEFAULT_FACE: &str = "SF Mono";

pub fn placeholder(detected: Option<&str>) -> SharedString {
    SharedString::from(detected.unwrap_or(FIRST_DEFAULT_FACE).to_owned())
}

pub fn hint(detected: Option<&str>) -> SharedString {
    let empty = match detected {
        Some(face) => format!("Empty draws with {face}, your terminal's font."),
        None => "Empty draws with the default font.".to_owned(),
    };
    format!("A font installed on this computer. {empty} Icons a face lacks come from a bundled symbols font.").into()
}

/// The card: 288 px, radius 10, 12 px in, on the popover with its shadow.
pub fn card(input: &Entity<InputState>, detected: Option<&str>, cx: &App) -> AnyElement {
    let p = cx.palette();
    // The compact input's 28 px inside its control's 1 px border, which today's app draws outside it.
    let field = Styled::h(Input::new(input), px(30.))
        .w_full()
        .rounded(px(8.))
        .px(px(9.))
        .py_0()
        .border_color(p.input)
        .bg(p.input_fill)
        .text_size(text::XS.0);
    v_flex()
        .id("terminal-font-card")
        .debug_selector(|| "terminal-font-card".into())
        .occlude()
        .w(px(288.))
        .gap_2()
        .rounded(px(10.))
        .border_1()
        .border_color(p.border)
        .bg(p.popover)
        .p_3()
        // `shadow-md`: the tooltip's `shadow-md/5` at twice its ink.
        .shadow(p.tooltip_shadow.iter().map(|s| BoxShadow { color: s.color.alpha(s.color.a * 2.), ..s.clone() }).collect())
        .text_color(p.foreground)
        .child(div().text_size(text::SETTING_TITLE.0).line_height(text::SETTING_TITLE.1).child("Terminal font"))
        .child(field)
        .child(div().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(hint(detected)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hint_and_placeholder_name_the_detected_face() {
        assert_eq!(
            hint(Some("Berkeley Mono")),
            "A font installed on this computer. Empty draws with Berkeley Mono, your terminal's font. Icons a face lacks come from a bundled symbols font."
        );
        assert_eq!(
            hint(None),
            "A font installed on this computer. Empty draws with the default font. Icons a face lacks come from a bundled symbols font."
        );
        assert_eq!(placeholder(None), "SF Mono");
    }
}
