// SPDX-License-Identifier: AGPL-3.0-only
//! What both tabs draw with: the locked card (`data-locked`: a 14 px regular head, 16 px over a 14 px radius surface),
//! the page's one way of naming a thing, its glyphs, the quiet line and the skeleton bar.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, font, img, prelude::*, px, relative, AnyElement, App, Div, FontWeight, Hsla, Pixels, SharedString, TextRun, Window};

use super::model::Split;
use crate::ui::{catalog, ActivePalette, Glyph};

/// `rounded-xl` off `--radius`, which `data-locked` gives the card.
pub const CARD_RADIUS: Pixels = px(14.);
/// `CARD_PAD`: the inset every card on this page keeps its content at.
pub const CARD_PAD: Pixels = px(20.);
/// The `gap-10` between the page's blocks.
pub const BLOCK_GAP: Pixels = px(40.);

/// Text at a size with the 1.5 line height a size without its own leading takes.
pub fn sized(el: Div, size: f32) -> Div {
    el.text_size(px(size)).line_height(px(size * 1.5))
}

/// One line of `sized` text held at its fractional height, which GPUI's line box rounds to a whole pixel.
pub fn one_line(el: Div, size: f32) -> Div {
    sized(el, size).h(px(size * 1.5))
}

/// A card: its head, 16 px, then its body, bare or on the surface.
pub fn card(id: &'static str, head: Option<SharedString>, body: impl IntoElement, cx: &App) -> Div {
    let p = cx.palette();
    v_flex()
        .debug_selector(move || id.into())
        .gap_4()
        .children(head.map(|head| {
            h_flex()
                .min_h(px(28.))
                .text_sm()
                .line_height(px(20.))
                .font_weight(FontWeight::NORMAL)
                .text_color(p.foreground.opacity(0.7))
                .child(head)
        }))
        .child(body)
}

/// `CARD_SURFACE` under `data-locked`.
pub fn surface(cx: &App) -> Div {
    let p = cx.palette();
    v_flex().rounded(CARD_RADIUS).border_1().border_color(p.border.opacity(0.6)).bg(p.card.opacity(0.4))
}

/// The faint rule between rows and under a table.
pub fn rule(cx: &App) -> Hsla {
    cx.palette().border.opacity(0.5)
}

/// `QUIET`: 13 px on a 20 px line, muted.
pub fn quiet(text: impl Into<SharedString>, cx: &App) -> Div {
    div().text_size(px(13.)).line_height(px(20.)).text_color(cx.palette().muted_foreground).child(text.into())
}

/// `GLYPH_FRAME` around any glyph.
pub fn frame(glyph: impl IntoElement, cx: &App) -> Div {
    let p = cx.palette();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(32.))
        .rounded(px(8.))
        .border_1()
        .border_color(p.border)
        .bg(p.foreground.opacity(0.04))
        .child(glyph)
}

/// The agent's mark where the catalog draws one; a mark with no inks of its own takes `around`.
pub fn agent_glyph(agent: &str, size: Pixels, around: Hsla, cx: &App) -> Option<AnyElement> {
    catalog::image(agent, cx.theme().is_dark(), around).map(|image| img(image).size(size).flex_none().into_any_element())
}

/// The glyph a split row leads with: its agent's mark, the computer, the project, the source, or the split's own
/// stand-in where the value has none, so every row's name starts at one edge.
pub fn split_glyph(split: Split, key: &str, agent: Option<&str>, projects: &[String], cx: &App) -> Option<AnyElement> {
    let p = cx.palette();
    if let Some(mark) = agent.and_then(|agent| agent_glyph(agent, px(16.), p.foreground, cx)) {
        return Some(mark);
    }
    let icon = |glyph: Glyph, color: Hsla| Some(Icon::new(glyph).with_size(px(16.)).text_color(color).into_any_element());
    match split {
        // The host's own computer is the one place read here, a desktop; any other computer takes the stand-in.
        Split::Computer if key == "here" => icon(Glyph::Monitor, p.foreground.opacity(0.8)),
        Split::Project if projects.iter().any(|id| id == key) => icon(Glyph::Folder, p.foreground),
        Split::Source if key == "log" => icon(Glyph::SquareTerminal, p.muted_foreground),
        Split::Source => icon(Glyph::MessagesSquare, p.muted_foreground),
        Split::Project => icon(Glyph::CircleDashed, p.muted_foreground),
        Split::Computer => icon(Glyph::Monitor, p.muted_foreground),
        Split::Account => icon(Glyph::UserRound, p.muted_foreground),
        Split::Model | Split::Agent => icon(Glyph::Box, p.muted_foreground),
    }
}

/// `Identity`: the mark in its frame, the title, and under it a line or a figure.
pub fn identity(
    mark: Option<AnyElement>,
    title: impl Into<SharedString>,
    line: Option<String>,
    under: Option<AnyElement>,
    cx: &App,
) -> Div {
    let p = cx.palette();
    h_flex().flex_1().min_w_0().gap_3().children(mark.map(|mark| frame(mark, cx))).child(
        v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(
                div()
                    .debug_selector(|| "usage-title".into())
                    .truncate()
                    .text_size(px(15.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .child(title.into()),
            )
            .children(
                line.filter(|l| !l.is_empty())
                    .map(|line| div().truncate().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(line)),
            )
            .children(under),
    )
}

/// A line's width to the fraction, as the web lays text out; a text box rounds it up, and a row of them drifts.
pub fn text_width(text: &str, size: f32, family: SharedString, window: &Window) -> Pixels {
    let run = TextRun {
        len: text.len(),
        font: font(family),
        color: gpui_kit::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window.text_system().shape_line(SharedString::from(text.to_owned()), px(size), &[run], None).width
}

/// A bar a person reads as a share: its track and its fill.
pub fn share_bar(share: f64, ink: Hsla, cx: &App) -> Div {
    let p = cx.palette();
    div()
        .mt_1()
        .h_1()
        .w_full()
        .max_w(px(256.))
        .rounded_full()
        .bg(p.foreground.opacity(0.08))
        .child(div().h_full().rounded_full().bg(ink).w(relative((share.max(0.01)) as f32)))
}

/// `Skeleton`: a bar in the opaque border color, 6 px round.
pub fn bone(w: Pixels, h: Pixels, cx: &App) -> Div {
    div().flex_none().w(w).h(h).rounded(px(6.)).bg(cx.palette().border)
}
