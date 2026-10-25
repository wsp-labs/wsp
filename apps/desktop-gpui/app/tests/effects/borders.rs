// SPDX-License-Identifier: AGPL-3.0-only
//! The app's rounded 1 px borders in Paper and Graphite: the header's split button, an inline code chip, a card and the
//! composer, drawn by the app's own pieces where they are public and matched against borders-<theme> in chromium/.

use std::path::Path;
use std::sync::Arc;

use desktop_gpui::screens::composer::parts::glass_box;
use desktop_gpui::ui::{self, ActivePalette, Button, Side, Still, UiAssets};
use gpui_kit::{canvas, div, point, prelude::*, px, size, Bounds, Context, Corners, HeadlessAppContext, Window};
use image::RgbaImage;

use super::scene::{H, W};

pub const THEMES: [&str; 2] = ["paper", "graphite"];

/// A rounded piece's border box in CSS px, its radius and which of its corners are round, from the top left clockwise.
pub struct Piece {
    pub name: &'static str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub round: [bool; 4],
    /// The straight edges that are its own, from the top clockwise; a split's halves share the edge between them.
    pub edges: [bool; 4],
}

const MAIN_W: f32 = 26.;
const CHEVRON_W: f32 = 18.;

pub const PIECES: [Piece; 5] = [
    Piece {
        name: "split main",
        x: 40.,
        y: 40.,
        w: MAIN_W,
        h: 32.,
        r: 8.,
        round: [true, false, false, true],
        edges: [true, false, true, true],
    },
    Piece {
        name: "split chevron",
        x: 40. + MAIN_W - 1.,
        y: 40.,
        w: CHEVRON_W,
        h: 32.,
        r: 8.,
        round: [false, true, true, false],
        edges: [true, true, true, false],
    },
    Piece { name: "code chip", x: 40., y: 120., w: 80., h: 20., r: 6., round: [true; 4], edges: [true; 4] },
    Piece { name: "card", x: 40., y: 180., w: 240., h: 100., r: 11., round: [true; 4], edges: [true; 4] },
    Piece { name: "composer", x: 320., y: 40., w: 280., h: 140., r: 22., round: [true; 4], edges: [true; 4] },
];

pub struct Borders;

impl Render for Borders {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let at = |piece: &Piece| div().absolute().left(px(piece.x)).top(px(piece.y));
        let left = Corners { top_left: px(8.), top_right: px(0.), bottom_right: px(0.), bottom_left: px(8.) };
        let right = Corners { top_left: px(0.), top_right: px(8.), bottom_right: px(8.), bottom_left: px(0.) };
        // GitSplit: the chevron half overlaps the main half's right edge by 1 px.
        let split = at(&PIECES[0])
            .flex()
            .child(Button::outline("split-main", "").corners(left))
            .child(div().ml(px(-1.)).child(Button::outline("split-chevron", "").xs().height(px(32.)).corners(right)));
        // gpui-kit's inline code frame paints its fill and border as one quad.
        let chip = &PIECES[2];
        let bounds = Bounds { origin: point(px(chip.x), px(chip.y)), size: size(px(chip.w), px(chip.h)) };
        let (muted, border) = (p.muted, p.border);
        let chip = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                window.paint_quad(gpui_kit::fill(bounds, muted).corner_radii(px(6.)).border_widths(px(1.)).border_color(border));
            },
        )
        .absolute()
        .size_full();
        let card = &PIECES[3];
        let card =
            at(card).w(px(card.w)).h(px(card.h)).rounded(px(card.r)).border_1().border_color(p.border.opacity(0.6)).bg(p.card.opacity(0.4));
        let composer = &PIECES[4];
        let composer = at(composer).w(px(composer.w)).child(glass_box(cx).h(px(composer.h)));
        div().relative().w(px(W)).h(px(H)).bg(p.background).child(split).child(chip).child(card).child(composer)
    }
}

fn app(theme: &str) -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_global(Still(true));
        ui::theme::set_theme(theme, cx);
    });
    cx
}

/// One theme's capture.
pub fn render(theme: &str) -> RgbaImage {
    super::render(&mut app(theme), || Borders)
}

fn rgb(shot: &RgbaImage, x: f32, y: f32) -> [f32; 3] {
    let p = shot.get_pixel(x.clamp(0., shot.width() as f32 - 1.) as u32, y.clamp(0., shot.height() as f32 - 1.) as u32);
    [p[0] as f32, p[1] as f32, p[2] as f32]
}

/// Bilinear between the device pixels around a device point.
fn sample(shot: &RgbaImage, x: f32, y: f32) -> [f32; 3] {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (a, b, c, d) = (rgb(shot, x0, y0), rgb(shot, x0 + 1., y0), rgb(shot, x0, y0 + 1.), rgb(shot, x0 + 1., y0 + 1.));
    std::array::from_fn(|i| (a[i] * (1. - tx) + b[i] * tx) * (1. - ty) + (c[i] * (1. - tx) + d[i] * tx) * ty)
}

/// The border's weight across one line from inside the piece to outside it: the largest channel's distance from what
/// lies there without a border (the piece's fill inside its outer edge, the page outside), summed in device px.
fn weight(shot: &RgbaImage, piece: &Piece, from: (f32, f32), dir: (f32, f32), inside: impl Fn(f32, f32) -> bool) -> f32 {
    let scale = shot.height() as f32 / H;
    let fill = rgb(shot, (piece.x + piece.w / 2.) * scale, (piece.y + piece.h - 6.) * scale);
    let page = rgb(shot, (piece.x - 6.) * scale, (piece.y + piece.h / 2.) * scale);
    let step = 0.05;
    let mut sum = 0.;
    let mut t = 0.;
    while t < 4.5 * scale {
        let (x, y) = (from.0 * scale + dir.0 * t, from.1 * scale + dir.1 * t);
        let base = if inside(x / scale, y / scale) { fill } else { page };
        let seen = sample(shot, x, y);
        sum += (0..3).map(|i| (seen[i] - base[i]).abs()).fold(0., f32::max) / 255. * step;
        t += step;
    }
    sum
}

/// Each straight edge's weight at its middle, then each round corner's at 20, 45 and 70 degrees round its arc.
pub fn weights(shot: &RgbaImage, piece: &Piece) -> Vec<(String, f32)> {
    let (x, y, w, h, r) = (piece.x, piece.y, piece.w, piece.h, piece.r);
    let mut out = Vec::new();
    let edges = [
        ("top", (x + w / 2., y + 3.), (0., -1.)),
        ("right", (x + w - 3., y + h / 2.), (1., 0.)),
        ("bottom", (x + w / 2., y + h - 3.), (0., 1.)),
        ("left", (x + 3., y + h / 2.), (-1., 0.)),
    ];
    let inside_box = |px: f32, py: f32| px >= x && px < x + w && py >= y && py < y + h;
    for (k, (name, from, dir)) in edges.into_iter().enumerate() {
        if piece.edges[k] {
            out.push((format!("{name} edge"), weight(shot, piece, from, dir, inside_box)));
        }
    }
    let centres = [(x + r, y + r, -1., -1.), (x + w - r, y + r, 1., -1.), (x + w - r, y + h - r, 1., 1.), (x + r, y + h - r, -1., 1.)];
    for (k, name) in ["top left", "top right", "bottom right", "bottom left"].into_iter().enumerate() {
        if !piece.round[k] {
            continue;
        }
        let (cx, cy, sx, sy) = centres[k];
        for degrees in [20f32, 45., 70.] {
            let (c, s) = (degrees.to_radians().cos(), degrees.to_radians().sin());
            let dir = (sx * c, sy * s);
            let from = (cx + dir.0 * (r - 3.), cy + dir.1 * (r - 3.));
            let inside = |px: f32, py: f32| (px - cx).hypot(py - cy) < r;
            out.push((format!("{name} arc {degrees}"), weight(shot, piece, from, dir, inside)));
        }
    }
    out
}

/// The light under a piece's top edge at the device point (x, y), as its largest channel's distance from the fill.
fn light(shot: &RgbaImage, piece: &Piece, x: f32, y: f32) -> f32 {
    let scale = shot.height() as f32 / H;
    let fill = rgb(shot, (piece.x + piece.w / 2.) * scale, (piece.y + piece.h - 6.) * scale);
    let seen = rgb(shot, x, y);
    (0..3).map(|i| (seen[i] - fill[i]).abs()).fold(0., f32::max)
}

/// The keycap light inside the main half's square top right corner, where the chevron half meets it, and the
/// composer's light under the middle of its top edge.
pub fn lights(shot: &RgbaImage) -> [(&'static str, f32); 2] {
    let scale = shot.height() as f32 / H;
    let (main, composer) = (&PIECES[0], &PIECES[4]);
    [
        ("split main square corner light", light(shot, main, (main.x + main.w - 1.) * scale - 1.5, (main.y + 1.25) * scale)),
        ("composer light under its top edge", light(shot, composer, (composer.x + composer.w / 2.) * scale, (composer.y + 1.25) * scale)),
    ]
}

/// The samples of both captures: each straight edge within 10% of Chromium's, each corner's arc on average within 10%
/// of Chromium's, and where the two straight edges an arc joins agree (the keycap light makes a top edge weigh more
/// than a side, in Chromium too), each point of the arc within 10% of their weight or within 10 points of Chromium's
/// share of its own. Returns what failed.
pub fn check(theme: &str, actual: &RgbaImage, expected: &RgbaImage) -> Vec<String> {
    let mut failed = Vec::new();
    let mut verdict = |what: String, ok: bool, seen: String| {
        println!("{:4} borders-{theme:8} {what}: {seen}", if ok { "ok" } else { "FAIL" });
        if !ok {
            failed.push(format!("borders-{theme} {what}"));
        }
    };
    let within = |a: f32, b: f32| (a - b).abs() <= 0.1 * b;
    for piece in &PIECES {
        let (ours, chromium) = (weights(actual, piece), weights(expected, piece));
        let edge = |samples: &[(String, f32)], name: &str| samples.iter().find(|(n, _)| n == &format!("{name} edge")).map(|(_, v)| *v);
        for ((name, a), (_, b)) in ours.iter().zip(&chromium).filter(|((n, _), _)| n.ends_with("edge")) {
            verdict(
                format!("{} {name}", piece.name),
                within(*a, *b),
                format!("{a:.3}, Chromium {b:.3}, {:.0}% of Chromium's", a / b * 100.),
            );
        }
        for corner in ["top left", "top right", "bottom right", "bottom left"] {
            let arc = |samples: &[(String, f32)]| -> Vec<f32> {
                samples.iter().filter(|(n, _)| n.starts_with(&format!("{corner} arc"))).map(|(_, v)| *v).collect()
            };
            let (a, b) = (arc(&ours), arc(&chromium));
            if a.is_empty() {
                continue;
            }
            let (mean_a, mean_b) = (a.iter().sum::<f32>() / a.len() as f32, b.iter().sum::<f32>() / b.len() as f32);
            let (vertical, horizontal) = corner.split_once(' ').unwrap();
            let joined = |samples: &[(String, f32)]| {
                let edges: Vec<f32> = [vertical, horizontal].iter().filter_map(|e| edge(samples, e)).collect();
                let mean = edges.iter().sum::<f32>() / edges.len() as f32;
                (mean, edges.iter().all(|e| within(*e, mean)))
            };
            let ((edge_a, even), (edge_b, _)) = (joined(&ours), joined(&chromium));
            let (share_a, share_b): (Vec<f32>, Vec<f32>) = (a.iter().map(|v| v / edge_a).collect(), b.iter().map(|v| v / edge_b).collect());
            let points = a.iter().map(|v| format!("{v:.3}")).collect::<Vec<_>>().join(" ");
            let ok = within(mean_a, mean_b)
                && (!even || share_a.iter().zip(&share_b).all(|(x, y)| (x - 1.).abs() <= 0.1 || (x - y).abs() <= 0.1));
            let percent = |shares: &[f32]| shares.iter().map(|v| format!("{:.0}%", v * 100.)).collect::<Vec<_>>().join(" ");
            let against = if even {
                format!("{} of its edges' {edge_a:.3}, Chromium {}", percent(&share_a), percent(&share_b))
            } else {
                "its edges differ".into()
            };
            verdict(
                format!("{} {corner} arc", piece.name),
                ok,
                format!(
                    "{points} at 20, 45, 70 degrees, mean {mean_a:.3}, Chromium {mean_b:.3} ({:.0}%), {against}",
                    mean_a / mean_b * 100.
                ),
            );
        }
    }
    for ((what, a), (_, b)) in lights(actual).into_iter().zip(lights(expected)) {
        verdict(what.into(), within(a, b) || (a - b).abs() <= 2., format!("{a:.0} over the fill, Chromium {b:.0}"));
    }
    failed
}

pub fn run(out: &Path, references: &Path) -> Vec<String> {
    let mut failed = Vec::new();
    for theme in THEMES {
        let actual = render(theme);
        actual.save(out.join(format!("gpui-borders-{theme}-0.png"))).unwrap();
        let reference = references.join(format!("borders-{theme}-0.png"));
        let expected = image::open(&reference).unwrap_or_else(|e| panic!("{}: {e}", reference.display())).into_rgba8();
        // Chromium's antialiasing of the composer's 22 px arcs strays up to 8 levels from an exact one.
        let stats = super::compare::compare(&actual, &expected, 8, &out.join(format!("diff-borders-{theme}-0.png")));
        println!("{:4} borders-{theme:8} tolerance 8: {stats}", if stats.over > 0 { "FAIL" } else { "ok" });
        if stats.over > 0 {
            failed.push(format!("borders-{theme}"));
        }
        failed.extend(check(theme, &actual, &expected));
    }
    failed
}
