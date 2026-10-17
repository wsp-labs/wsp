// SPDX-License-Identifier: AGPL-3.0-only
//! `WindowPicture`: a wsp window drawn small in one theme's own tokens, its 200 by 125 view box scaled to the width
//! given. The sidebar with its rows and the one picked, the person's message, a reply of a strong line and three quiet
//! ones, and the composer with the theme's accent on its send.

use std::sync::OnceLock;

use gpui_kit::{div, prelude::*, px, Div, Hsla, Pixels};

use crate::ui::theme::THEMES;
use crate::ui::Palette;

/// Every theme's tokens, read once from its file.
pub fn palette_of(id: &str) -> &'static Palette {
    static PALETTES: OnceLock<Vec<Palette>> = OnceLock::new();
    let palettes = PALETTES.get_or_init(|| THEMES.iter().map(|t| t.palette()).collect());
    let at = THEMES.iter().position(|t| t.id == id).unwrap_or(0);
    &palettes[at]
}

/// One rounded rectangle of the view box, in its own units.
fn shape(s: f32, x: f32, y: f32, w: f32, h: f32, r: f32, fill: Hsla) -> Div {
    div().absolute().left(px(x * s)).top(px(y * s)).w(px(w * s)).h(px(h * s)).rounded(px(r * s)).bg(fill)
}

pub fn window_picture(p: &Palette, width: Pixels) -> Div {
    let s = f32::from(width) / 200.;
    let mut picture = div().relative().w(width).h(width * 0.625).overflow_hidden().bg(p.background);
    picture = picture
        .child(shape(s, 0., 0., 52., 125., 0., p.sidebar))
        // The sidebar's edge and the composer's are hairlines at every scale.
        .child(div().absolute().left(px(52.5 * s - 0.5)).top_0().bottom_0().w(px(1.)).bg(p.border))
        .child(shape(s, 6., 21., 40., 11., 3., p.sidebar_foreground.opacity(0.1)));
    for (at, (y, w)) in [(16., 24.), (27., 30.), (38., 20.), (49., 26.)].into_iter().enumerate() {
        let fill = if at == 1 { p.sidebar_foreground.opacity(0.8) } else { p.sidebar_muted_foreground.opacity(0.5) };
        picture = picture.child(shape(s, 10., y, w, 3., 1.5, fill));
    }
    picture =
        picture.child(shape(s, 128., 12., 62., 13., 6.5, p.accent)).child(shape(s, 64., 38., 108., 4., 2., p.foreground.opacity(0.8)));
    for (y, w) in [(48., 96.), (57., 104.), (66., 70.)] {
        picture = picture.child(shape(s, 64., y, w, 3.5, 1.75, p.muted_foreground.opacity(0.6)));
    }
    picture
        .child(
            div()
                .absolute()
                .left(px(62.5 * s - 0.5))
                .top(px(92.5 * s - 0.5))
                .w(px(127. * s + 1.))
                .h(px(22. * s + 1.))
                .rounded(px(11. * s + 0.5))
                .border_1()
                .border_color(p.border)
                .bg(p.card),
        )
        .child(shape(s, 73., 102., 44., 3.5, 1.75, p.muted_foreground.opacity(0.5)))
        .child(shape(s, 172., 97.5, 12., 12., 6., p.primary))
}
