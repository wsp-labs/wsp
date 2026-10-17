// SPDX-License-Identifier: AGPL-3.0-only
//! The effects scenes in GPUI, drawn to match scene.html beside this file pixel for pixel: rows that scroll under a
//! frosted panel, and a rounded box clipping a scrolled list to its corners.

use gpui_kit::{div, prelude::*, px, rgb, AnyElement, Div, Rgba};

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
}

impl Scene {
    pub const ALL: [Scene; 5] = [Scene::PlainLight, Scene::BlurLight, Scene::BlurDark, Scene::BlurTwo, Scene::Clip];

    pub fn name(self) -> &'static str {
        match self {
            Scene::PlainLight => "plain-light",
            Scene::BlurLight => "blur-light",
            Scene::BlurDark => "blur-dark",
            Scene::BlurTwo => "blur-two",
            Scene::Clip => "clip",
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

/// The panel the web app's glass-backdrop draws: blur and saturation from apps/web/src/index.css, light and dark.
fn panel(scene: Scene) -> Div {
    let panel = at(40., 300., 560., 120.).rounded(px(22.));
    let light = Rgba { r: 1., g: 1., b: 1., a: 0.55 };
    let dark = Rgba { r: 245. / 255., g: 245. / 255., b: 245. / 255., a: 0.03 };
    match scene {
        Scene::PlainLight => panel.bg(light),
        Scene::BlurLight => panel.bg(light).backdrop_blur(px(12.)).backdrop_saturation(1.14),
        Scene::BlurDark => panel.bg(dark).backdrop_blur(px(16.)).backdrop_saturation(1.08),
        Scene::BlurTwo | Scene::Clip => unreachable!("{scene:?} has no single panel"),
    }
}

pub fn draw(scene: Scene, scroll: f32) -> Div {
    let page = div().relative().w(px(W)).h(px(H)).overflow_hidden();
    match scene {
        Scene::Clip => page.bg(rgb(0xf4f4f5)).child(clipped_list(scroll)),
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
