// SPDX-License-Identifier: AGPL-3.0-only
use gpui::{div, prelude::*, px, Context, SharedString, Window};

pub const TITLE: &str = "wsp";

pub struct Home;

impl Render for Home {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "home".into())
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(div().debug_selector(|| "title".into()).text_size(px(20.)).child(SharedString::from(TITLE)))
    }
}
