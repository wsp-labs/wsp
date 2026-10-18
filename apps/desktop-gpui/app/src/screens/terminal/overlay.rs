// SPDX-License-Identifier: AGPL-3.0-only
//! What covers the shells while they cannot be typed into (`ThreadTerminalDrawer.tsx`, the state overlay and the
//! shell-gone overlay) and what an empty drawer says, in today's words.

use std::time::Instant;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, SharedString};

use crate::ui::{text, ActivePalette};

/// "for Ns" after a reconnecting title.
pub fn seconds_since(since: Instant) -> SharedString {
    format!("for {}s", since.elapsed().as_secs()).into()
}

/// The overlay's box: over the whole viewport area at 70% of the terminal's ground with a 1 px blur, centred, 8 px apart.
pub fn cover(cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = cx.palette();
    v_flex()
        .id("terminal-overlay")
        .debug_selector(|| "terminal-overlay".into())
        .absolute()
        .inset_0()
        .occlude()
        .items_center()
        .justify_center()
        .gap_2()
        .px_4()
        .bg(p.terminal_background.opacity(0.7))
        .backdrop_blur(px(1.))
        .text_center()
        .text_size(text::SETTING_TITLE.0)
        .line_height(text::SETTING_TITLE.1)
        .text_color(p.foreground)
}

/// The title row, with "for Ns" beside it while reconnecting.
pub fn title(title: impl Into<SharedString>, since: Option<Instant>) -> AnyElement {
    h_flex()
        .items_baseline()
        .gap(px(6.))
        .child(title.into())
        .children(since.map(|since| div().debug_selector(|| "terminal-overlay-for".into()).child(seconds_since(since))))
        .into_any_element()
}

/// One `text-xs text-muted-foreground` line.
pub fn note(line: impl Into<SharedString>, cx: &App) -> AnyElement {
    div().text_size(text::XS.0).line_height(text::XS.1).text_color(cx.palette().muted_foreground).child(line.into()).into_any_element()
}

/// The start refusal, red under the button (`text-destructive-foreground`).
pub fn red(line: impl Into<SharedString>, cx: &App) -> AnyElement {
    div().text_size(text::XS.0).line_height(text::XS.1).text_color(cx.palette().error_foreground).child(line.into()).into_any_element()
}
