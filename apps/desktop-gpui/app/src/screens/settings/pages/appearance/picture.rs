// SPDX-License-Identifier: AGPL-3.0-only
//! `WindowPicture`: a wsp window drawn small in one theme's own tokens, its 200 by 125 view box scaled to the width
//! given. The sidebar with its rows and the one picked, the person's message, a reply of a strong line and three quiet
//! ones, and the composer with the theme's accent on its send. Painted rather than laid out, so a picture can be cut
//! on a slant as the web's `clip-path` cuts System's dark half.

use std::sync::OnceLock;

use gpui_kit::{canvas, point, prelude::*, px, quad, size, App, BorderStyle, Bounds, ContentMask, Hsla, Pixels, Window};

use crate::ui::theme::THEMES;
use crate::ui::Palette;

/// Every theme's tokens, read once from its file.
pub fn palette_of(id: &str) -> &'static Palette {
    static PALETTES: OnceLock<Vec<Palette>> = OnceLock::new();
    let palettes = PALETTES.get_or_init(|| THEMES.iter().map(|t| t.palette()).collect());
    let at = THEMES.iter().position(|t| t.id == id).unwrap_or(0);
    &palettes[at]
}

/// Where a slant cuts the picture: the line from `top` across its top edge to `bottom` across its foot, each a
/// fraction of the width; the picture shows right of it.
#[derive(Clone, Copy)]
pub struct Slant {
    pub top: f32,
    pub bottom: f32,
}

/// System's tile: `clip-path: polygon(60% 0, 100% 0, 100% 100%, 40% 100%)`.
pub const SYSTEM_CUT: Slant = Slant { top: 0.6, bottom: 0.4 };

pub fn window_picture(p: &'static Palette, width: Pixels) -> impl IntoElement {
    picture(p, width, None)
}

/// The picture right of `slant` alone, the rest left clear for whatever lies under it.
pub fn window_picture_cut(p: &'static Palette, width: Pixels, slant: Slant) -> impl IntoElement {
    picture(p, width, Some(slant))
}

fn picture(p: &'static Palette, width: Pixels, slant: Option<Slant>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _: &mut App| match slant {
            None => paint(p, bounds, window),
            Some(slant) => {
                for strip in strips(bounds, window.scale_factor(), slant) {
                    window.with_content_mask(Some(ContentMask { bounds: strip }), |window| paint(p, bounds, window));
                }
            }
        },
    )
    .w(width)
    .h(width * 0.625)
    .flex_none()
}

/// The part of `bounds` right of the slant, one device row at a time, each row cut where the line crosses its middle.
pub fn strips(bounds: Bounds<Pixels>, scale: f32, slant: Slant) -> Vec<Bounds<Pixels>> {
    let row = 1. / scale;
    let height = f32::from(bounds.size.height);
    let width = f32::from(bounds.size.width);
    let rows = (height * scale).ceil() as usize;
    (0..rows)
        .map(|n| {
            let top = n as f32 * row;
            let through = ((top + row / 2.) / height).min(1.);
            let x = width * (slant.top + (slant.bottom - slant.top) * through);
            let at = point(bounds.origin.x + px(x), bounds.origin.y + px(top));
            Bounds::new(at, size(px(width - x), px(row.min(height - top))))
        })
        .collect()
}

fn paint(p: &Palette, bounds: Bounds<Pixels>, window: &mut Window) {
    let s = f32::from(bounds.size.width) / 200.;
    let o = bounds.origin;
    let at = |x: f32, y: f32, w: f32, h: f32| Bounds::new(point(o.x + px(x), o.y + px(y)), size(px(w), px(h)));
    // One rounded rectangle of the view box, in its own units.
    let shape = |window: &mut Window, (x, y, w, h, r): (f32, f32, f32, f32, f32), fill: Hsla| {
        window.paint_quad(quad(at(x * s, y * s, w * s, h * s), px(r * s), fill, px(0.), gpui_kit::transparent_black(), BorderStyle::Solid))
    };
    shape(window, (0., 0., 200., 125., 0.), p.background);
    shape(window, (0., 0., 52., 125., 0.), p.sidebar);
    shape(window, (6., 21., 40., 11., 3.), p.sidebar_foreground.opacity(0.1));
    for (n, (y, w)) in [(16., 24.), (27., 30.), (38., 20.), (49., 26.)].into_iter().enumerate() {
        let fill = if n == 1 { p.sidebar_foreground.opacity(0.8) } else { p.sidebar_muted_foreground.opacity(0.5) };
        shape(window, (10., y, w, 3., 1.5), fill);
    }
    shape(window, (128., 12., 62., 13., 6.5), p.accent);
    shape(window, (64., 38., 108., 4., 2.), p.foreground.opacity(0.8));
    for (y, w) in [(48., 96.), (57., 104.), (66., 70.)] {
        shape(window, (64., y, w, 3.5, 1.75), p.muted_foreground.opacity(0.6));
    }
    // The sidebar's edge and the composer's are hairlines at every scale.
    window.paint_quad(quad(at(52.5 * s - 0.5, 0., 1., 125. * s), px(0.), p.border, px(0.), p.border, BorderStyle::Solid));
    let composer = at(62.5 * s - 0.5, 92.5 * s - 0.5, 127. * s + 1., 22. * s + 1.);
    window.paint_quad(quad(composer, px(11. * s + 0.5), p.card, px(1.), p.border, BorderStyle::Solid));
    shape(window, (73., 102., 44., 3.5, 1.75), p.muted_foreground.opacity(0.5));
    shape(window, (172., 97.5, 12., 12., 6.), p.primary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systems_cut_runs_from_sixty_percent_at_the_top_to_forty_at_the_foot() {
        let bounds = Bounds::new(point(px(10.), px(20.)), size(px(200.), px(125.)));
        let strips = strips(bounds, 2., SYSTEM_CUT);
        assert_eq!(strips.len(), 250, "one strip per device row");
        let (first, last) = (strips[0], strips[strips.len() - 1]);
        assert!((f32::from(first.origin.x) - (10. + 120.)).abs() < 0.5, "the top row starts at 60%: {first:?}");
        assert!((f32::from(last.origin.x) - (10. + 80.)).abs() < 0.5, "the foot starts at 40%: {last:?}");
        assert!(strips.iter().all(|s| s.right() == bounds.right()), "every strip runs to the right edge");
        assert_eq!(f32::from(last.bottom()), 145., "the strips cover the whole height");
        let middle = strips[125];
        assert!((f32::from(middle.origin.x) - (10. + 100.)).abs() < 0.5, "halfway down, halfway across the cut: {middle:?}");
    }
}
