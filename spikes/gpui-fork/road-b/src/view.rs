// SPDX-License-Identifier: AGPL-3.0-only
use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px, rgb, rgba,
};

pub struct Spike;

impl Render for Spike {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .bg(rgb(0x0e0f12))
            .text_color(rgb(0xe6e6e6))
            .text_size(px(13.))
            .child(sidebar())
            .child(content())
    }
}

fn sidebar() -> impl IntoElement {
    div()
        .w(px(260.))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .bg(rgb(0x16181d))
        .border_r_1()
        .border_color(rgb(0x2a2d35))
        .py_2()
        .children((1..=30).map(|i| {
            div()
                .px_3()
                .py(px(5.))
                .when(i == 3, |row| row.bg(rgb(0x262a33)))
                .child(format!("Thread {i:02}  fix the sidebar row {i}"))
        }))
}

fn content() -> impl IntoElement {
    let palette = [0xef4444, 0xf59e0b, 0x22c55e, 0x06b6d4, 0x3b82f6, 0xa855f7];
    div()
        .relative()
        .flex_1()
        .h_full()
        .overflow_hidden()
        .child(div().absolute().inset_0().flex().children(
            (0..24).map(move |i| div().flex_1().h_full().bg(rgb(palette[i % palette.len()]))),
        ))
        .child(
            div()
                .absolute()
                .inset_0()
                .p_6()
                .flex()
                .flex_col()
                .gap_2()
                .text_size(px(22.))
                .text_color(rgb(0x000000))
                .children((0..14).map(|i| format!("Content line {i} sits behind the glass panel"))),
        )
        .child(glass_panel())
}

fn glass_panel() -> impl IntoElement {
    div()
        .absolute()
        .left(px(120.))
        .top(px(140.))
        .w(px(520.))
        .h(px(300.))
        .rounded(px(24.))
        .overflow_hidden()
        .bg(rgba(0xffffff26))
        .backdrop_blur(px(24.))
        .border_1()
        .border_color(rgba(0xffffff66))
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(56.))
                .flex_none()
                .bg(rgb(0xff6b35))
                .px_5()
                .flex()
                .items_center()
                .text_color(rgb(0x000000))
                .child("Header strip: its corners should follow the panel's radius"),
        )
        .child(
            div()
                .p_5()
                .text_size(px(16.))
                .text_color(rgb(0xffffff))
                .child("Backdrop blur 24px over the content area"),
        )
}
