// SPDX-License-Identifier: AGPL-3.0-only
//! The starting window (`desktop:starting.html`): the spinner and "Starting wsp", dark whatever the theme, as the app's
//! window opens before its page reads the person's preference.

use gpui_kit::component::{h_flex, Root};
use gpui_kit::{div, prelude::*, px, AnyWindowHandle, App, Context, Window};

use super::startup;
use crate::ui::spinner_at;
use crate::ui::theme::{self, Palette};

pub const WORDS: &str = "Starting wsp";

pub struct StartingView {
    dark: Palette,
}

impl StartingView {
    pub fn new() -> Self {
        StartingView { dark: theme::theme_by_id(theme::DARK).expect("the dark default is a theme").palette() }
    }
}

impl Default for StartingView {
    fn default() -> Self {
        StartingView::new()
    }
}

impl Render for StartingView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = &self.dark;
        // The spinner reads its ink as it lays out, inside the dark scope.
        let words = h_flex().debug_selector(|| "starting".into()).gap(px(8.)).child(spinner_at(px(16.), cx)).child(WORDS);
        theme::scoped(
            p.clone(),
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(p.background)
                .text_color(p.muted_foreground)
                .text_size(px(14.))
                .line_height(px(21.))
                .child(words),
        )
    }
}

/// Opens the starting window, the app's own window size and frame, as Electron opens it with `newWindow`.
pub fn open(cx: &mut App) -> Option<AnyWindowHandle> {
    let options = startup::window_options(cx);
    let window = cx.open_window(options, |window, cx| {
        let view = cx.new(|_| StartingView::new());
        cx.new(|cx| Root::new(view, window, cx))
    });
    window.ok().map(Into::into)
}
