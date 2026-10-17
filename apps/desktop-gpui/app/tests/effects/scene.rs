// SPDX-License-Identifier: AGPL-3.0-only
//! The effects scenes in GPUI, drawn to match scene.html beside this file pixel for pixel: rows that scroll under a
//! frosted panel, and a rounded box clipping a scrolled list to its corners.

use gpui_kit::{div, point, prelude::*, px, rgb, AnyElement, BoxShadow, Div, Rgba};

pub const W: f32 = 640.;
pub const H: f32 = 480.;

const ROW: f32 = 36.;
const PALETTE: [u32; 6] = [0xe11d48, 0xf59e0b, 0x10b981, 0x3b82f6, 0x8b5cf6, 0x64748b];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scene {
    PlainLight,
    BlurLight,
    BlurDark,
    /// A small blur, then a larger one in the same frame.
    BlurTwo,
    Clip,
    /// Rounded cards that hide their overflow, scrolled in a rounded panel that hides its own.
    ClipNested,
    /// Sharp strokes under blurs of 1, 4 and 8 px, with strokes flush with each panel's right and bottom edges.
    BlurRadii,
    /// A dark dropdown's near-opaque tint over an 8 px blur.
    BlurTintDark,
    /// The light composer's drop shadow, once inside a clipping pane and once scrolled out of its top.
    BlurShadow,
}

impl Scene {
    pub const ALL: [Scene; 9] = [
        Scene::PlainLight,
        Scene::BlurLight,
        Scene::BlurDark,
        Scene::BlurTwo,
        Scene::Clip,
        Scene::ClipNested,
        Scene::BlurRadii,
        Scene::BlurTintDark,
        Scene::BlurShadow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Scene::PlainLight => "plain-light",
            Scene::BlurLight => "blur-light",
            Scene::BlurDark => "blur-dark",
            Scene::BlurTwo => "blur-two",
            Scene::Clip => "clip",
            Scene::ClipNested => "clip-nested",
            Scene::BlurRadii => "blur-radii",
            Scene::BlurTintDark => "blur-tint-dark",
            Scene::BlurShadow => "blur-shadow",
        }
    }

    /// The scroll positions each scene is drawn at; the newer scenes do not scroll.
    pub fn scrolls(self) -> &'static [f32] {
        match self {
            Scene::PlainLight | Scene::BlurLight | Scene::BlurDark | Scene::BlurTwo | Scene::Clip => &[0., 50., 117.],
            Scene::ClipNested => &[CARD_SCROLL],
            Scene::BlurRadii | Scene::BlurTintDark | Scene::BlurShadow => &[0.],
        }
    }
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Div {
    div().absolute().left(px(x)).top(px(y)).w(px(w)).h(px(h))
}

/// Short ink strokes standing in for a line of text, a different length on each row.
fn ink(i: usize, x0: f32, top: f32, color: Rgba) -> impl Iterator<Item = Div> {
    (0..8 + (i * 7) % 13).map(move |k| at(x0 + 10. * k as f32, top + 10., 2., 16.).bg(color))
}

fn rows(scroll: f32, color: Rgba) -> Vec<AnyElement> {
    (0..((H + scroll) / ROW).ceil() as usize)
        .filter_map(|i| {
            let top = i as f32 * ROW - scroll;
            (top + ROW > 0.).then(|| {
                let bar = at(24., top + 4., 592., 28.).bg(rgb(PALETTE[i % 6]));
                std::iter::once(bar).chain(ink(i, 40., top, color)).map(IntoElement::into_any_element).collect::<Vec<_>>()
            })
        })
        .flatten()
        .collect()
}

fn clipped_list(scroll: f32) -> Div {
    let list = (0..((400. + scroll) / ROW).ceil() as usize).flat_map(|i| {
        let top = i as f32 * ROW - scroll;
        let row = at(0., top, 280., ROW).bg(rgb(PALETTE[i % 6]));
        std::iter::once(row).chain(ink(i, 12., top, rgb(0x18181b)))
    });
    at(180., 40., 280., 400.).rounded(px(14.)).overflow_hidden().bg(rgb(0xffffff)).children(list)
}

const CARD_SCROLL: f32 = 40.;
const CARD: f32 = 72.;
const CARD_PITCH: f32 = 80.;

const NESTED_PANEL: u32 = 0xe4e4e7;
pub const CUT_CARD: u32 = 0xececee;

/// A rounded(22) panel clipping a list of rounded(8) cards that clip their own fill and ink, scrolled so a card
/// sits under each of the panel's corners.
fn nested_cards(scroll: f32) -> Div {
    let cards = (0..((400. + scroll) / CARD_PITCH).ceil() as usize).map(|i| {
        let top = i as f32 * CARD_PITCH - scroll;
        // Chromium antialiases two rounded clips that meet with a coarser mask, so the cards the panel's edge cuts
        // stay within 8 levels of the page and the panel.
        let fill = if top < 0. || top + CARD > 400. { CUT_CARD } else { PALETTE[i % 6] };
        at(0., top, 320., CARD).rounded(px(8.)).overflow_hidden().child(at(0., 0., 320., CARD).bg(rgb(fill))).children(ink(
            i,
            40.,
            18.,
            rgb(0x18181b),
        ))
    });
    at(160., 40., 320., 400.).rounded(px(22.)).overflow_y_hidden().bg(rgb(NESTED_PANEL)).children(cards)
}

const RADII_PANELS: [(f32, f32); 3] = [(30., 1.), (180., 4.), (330., 8.)];

/// Vertical strokes 1 to 32 px wide, each followed by a gap at least as wide, from x 48 up to 584.
fn barcode() -> Vec<(f32, f32)> {
    const WIDTHS: [f32; 10] = [1., 2., 3., 4., 6., 8., 12., 16., 24., 32.];
    let mut strokes = Vec::new();
    let mut x = 48.;
    for k in 0.. {
        let w = WIDTHS[k % WIDTHS.len()];
        if x + w > 584. {
            break;
        }
        strokes.push((x, w));
        x += w + w.max(6.);
    }
    strokes
}

/// Strokes under three untinted blurs of 1, 4 and 8 px; one stroke each sits flush with the right edge and
/// the bottom edge of every panel, with a clear band inside it, so mirroring there differs from clamping.
fn radii_strokes() -> Vec<Div> {
    let ink = rgb(0x18181b);
    let mut strokes: Vec<Div> = barcode().into_iter().map(|(x, w)| at(x, 0., w, H).bg(ink)).collect();
    strokes.push(at(599., 0., 1., H).bg(ink));
    for (top, _) in RADII_PANELS {
        for (y, h) in [(12., 2.), (30., 8.), (56., 24.)] {
            strokes.push(at(40., top + y, 544., h).bg(ink));
        }
        strokes.push(at(40., top + 119., 560., 1.).bg(ink));
    }
    strokes
}

/// The light composer from ComposerSurface.tsx: glass-backdrop's blur and saturation, the card at 55% and
/// --shadow-composer, 0 12px 28px -18px at 40% black, whose 28 px CSS blur is a Gaussian of sigma 14.
fn composer(top: f32) -> Div {
    let shadow = BoxShadow {
        color: gpui_kit::rgba(0x0000_0066).into(),
        offset: point(px(0.), px(12.)),
        blur_radius: px(14.),
        spread_radius: px(-18.),
        inset: false,
    };
    at(40., top, 560., 120.)
        .rounded(px(22.))
        .bg(Rgba { r: 1., g: 1., b: 1., a: 0.55 })
        .backdrop_blur(px(12.))
        .backdrop_saturation(1.14)
        .shadow(vec![shadow])
}

const PANE_TOP: f32 = 200.;
const SCROLLED_COMPOSER_TOP: f32 = -116.;

/// The panel the web app's glass-backdrop draws: blur and saturation from apps/web/src/index.css, light and dark.
fn panel(scene: Scene) -> Div {
    let panel = at(40., 300., 560., 120.).rounded(px(22.));
    let light = Rgba { r: 1., g: 1., b: 1., a: 0.55 };
    let dark = Rgba { r: 245. / 255., g: 245. / 255., b: 245. / 255., a: 0.03 };
    match scene {
        Scene::PlainLight => panel.bg(light),
        Scene::BlurLight => panel.bg(light).backdrop_blur(px(12.)).backdrop_saturation(1.14),
        Scene::BlurDark => panel.bg(dark).backdrop_blur(px(16.)).backdrop_saturation(1.08),
        _ => unreachable!("{scene:?} has no single panel"),
    }
}

pub fn draw(scene: Scene, scroll: f32) -> Div {
    let page = div().relative().w(px(W)).h(px(H)).overflow_hidden();
    match scene {
        Scene::Clip => page.bg(rgb(0xf4f4f5)).child(clipped_list(scroll)),
        Scene::ClipNested => page.bg(rgb(0xf4f4f5)).child(nested_cards(scroll)),
        Scene::BlurRadii => page
            .bg(rgb(0xf4f4f5))
            .children(radii_strokes())
            .children(RADII_PANELS.map(|(top, blur)| at(40., top, 560., 120.).rounded(px(12.)).backdrop_blur(px(blur)))),
        // dropdown-glass in Graphite's dark mode: the popover, #111111, at 96% over an 8 px blur.
        Scene::BlurTintDark => page.bg(rgb(0x0a0a0a)).children(rows(scroll, rgb(0xf4f4f5))).child(
            at(380., 60., 220., 288.)
                .rounded(px(12.))
                .bg(Rgba { r: 17. / 255., g: 17. / 255., b: 17. / 255., a: 0.96 })
                .backdrop_blur(px(8.)),
        ),
        // The scrolled composer shows its bottom 4 px in the pane; its shadow's rect ends 2 px above the pane.
        Scene::BlurShadow => page
            .bg(rgb(0xf4f4f5))
            .children(rows(scroll, rgb(0x18181b)))
            .child(at(0., PANE_TOP, W, H - PANE_TOP).overflow_hidden().child(composer(SCROLLED_COMPOSER_TOP)).child(composer(120.))),
        Scene::BlurDark => page.bg(rgb(0x0a0a0a)).children(rows(scroll, rgb(0xf4f4f5))).child(panel(scene)),
        Scene::PlainLight | Scene::BlurLight => page.bg(rgb(0xf4f4f5)).children(rows(scroll, rgb(0x18181b))).child(panel(scene)),
        Scene::BlurTwo => {
            let light = Rgba { r: 1., g: 1., b: 1., a: 0.55 };
            let small = at(60., 40., 200., 80.).rounded(px(16.)).bg(light).backdrop_blur(px(6.)).backdrop_saturation(1.14);
            let large = at(40., 280., 560., 160.).rounded(px(22.)).bg(light).backdrop_blur(px(16.)).backdrop_saturation(1.08);
            page.bg(rgb(0xf4f4f5)).children(rows(scroll, rgb(0x18181b))).child(small).child(large)
        }
    }
}
