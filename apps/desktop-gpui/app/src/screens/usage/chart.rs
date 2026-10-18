// SPDX-License-Identifier: AGPL-3.0-only
//! The Usage page's tokens chart, after `apps/web/src/settings/usageChart.tsx` at rest: one line per split value on
//! the scale of the largest single point, a Bayer dither under each line in its own ink, and a tooltip over the step
//! under the pointer. The web's tween and its rolling axis figures are motion this port leaves out.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    canvas, deferred, div, fill, font, point, prelude::*, px, quad, relative, size, transparent_black, App, BorderStyle, Bounds,
    DispatchPhase, ElementId, Entity, Hitbox, HitboxBehavior, Hsla, MouseExitEvent, MouseMoveEvent, PathBuilder, PathStyle, Pixels, Point,
    RenderImage, Rgba, SharedString, StrokeOptions, TextRun, Window,
};
use image::{Frame, ImageBuffer};
use lyon::tessellation::{LineCap, LineJoin};

use crate::ui::{size as ui_size, text, ActivePalette};

pub const CHART_HEIGHT: Pixels = px(300.);
const HEIGHT: f64 = 300.;
const PAD: f64 = 6.;
const CELL: usize = 2;
/// `CELL - 0.6`: a lit cell leaves a hairline of the ground on two sides, so the dither reads as dots.
const DOT: f64 = 1.4;
const BAYER: [f64; 16] = {
    let order = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    let mut out = [0.; 16];
    let mut i = 0;
    while i < 16 {
        out[i] = (order[i] as f64 + 0.5) / 16.;
        i += 1;
    }
    out
};
/// The four gridlines and the floor in the axis's figures.
const PARTS: usize = 4;
const GAP: Pixels = px(12.);
const AXIS_TEXT: Pixels = px(12.);
const TICK_ROW: Pixels = px(16.);
const END_DOT: f32 = 6.;
const TOOLTIP_OFFSET: Pixels = px(6.);

#[derive(Clone)]
pub struct ChartLine {
    pub key: SharedString,
    pub label: SharedString,
    pub points: Vec<f64>,
    pub ink: Hsla,
}

#[derive(Clone)]
pub struct Tick {
    pub at: f32,
    pub word: SharedString,
}

/// The step under the pointer, and the plot's ground as last rasterised with the hash of what it was drawn from.
#[derive(Default)]
struct ChartState {
    hovered: Option<usize>,
    ground: Option<(u64, Arc<RenderImage>)>,
}

#[derive(IntoElement)]
pub struct UsageChart {
    id: ElementId,
    steps: Vec<f64>,
    lines: Vec<ChartLine>,
    ticks: Vec<Tick>,
    figure: fn(f64) -> String,
    step_word: Rc<dyn Fn(f64) -> String>,
}

impl UsageChart {
    pub fn new(id: impl Into<ElementId>, steps: Vec<f64>, lines: Vec<ChartLine>, ticks: Vec<Tick>) -> Self {
        UsageChart { id: id.into(), steps, lines, ticks, figure: |v| v.to_string(), step_word: Rc::new(|t| t.to_string()) }
    }

    /// The words for a figure on the axis and in the tooltip.
    pub fn figure(mut self, figure: fn(f64) -> String) -> Self {
        self.figure = figure;
        self
    }

    /// The tooltip's first line, naming the step.
    pub fn step_word(mut self, step_word: Rc<dyn Fn(f64) -> String>) -> Self {
        self.step_word = step_word;
        self
    }
}

fn y_of(v: f64, top: f64, h: f64) -> f64 {
    h - PAD - if top == 0. { 0. } else { v / top * (h - PAD * 2.) }
}

/// The step a pointer `x` from the plot's left is over: the first and last cells are half as wide as the rest.
fn hover_index(x: f32, width: f32, n: usize) -> Option<usize> {
    if n == 0 || width <= 0. || x < 0. || x > width {
        return None;
    }
    if n == 1 {
        return Some(0);
    }
    Some(((x / width * (n - 1) as f32).round() as usize).min(n - 1))
}

/// Each lit cell under `fills` (biggest first) as `(rank, x, y)` from the plot's origin, as the web's `paintDither` walks them.
fn dither_cells(fills: &[&[f64]], top: f64, w: f64, h: f64, mut lit: impl FnMut(usize, f64, f64)) {
    if w <= 0. || top == 0. {
        return;
    }
    let floor = h - PAD;
    for (rank, points) in fills.iter().enumerate() {
        let n = points.len();
        if n < 2 {
            continue;
        }
        let density = if rank == 0 { 0.55 } else { 0.32 };
        let phase = rank * 5;
        for px in (0..w.ceil() as usize).step_by(CELL) {
            let t = px as f64 / w * (n - 1) as f64;
            let i = (n - 2).min(t.floor() as usize);
            let f = t - i as f64;
            let line = y_of(points[i] * (1. - f) + points[i + 1] * f, top, h);
            let mut py = line.ceil();
            while py < floor {
                let depth = 1. - (py - line) / (floor - line + 1.);
                let cell = ((py as i64 / CELL as i64) & 3) as usize * 4 + ((px / CELL) & 3);
                if depth * density > BAYER[(cell + phase) % 16] {
                    lit(rank, px as f64, py);
                }
                py += CELL as f64;
            }
        }
    }
}

/// Source-over of `ink` at coverage `a` onto one straight-alpha BGRA pixel.
fn blend(pixel: &mut [u8], ink: Rgba, a: f64) {
    if a <= 0. {
        return;
    }
    let under = pixel[3] as f64 / 255.;
    let out = a + under * (1. - a);
    let mix = |src: f32, dst: u8| ((src as f64 * a + dst as f64 / 255. * under * (1. - a)) / out * 255.).round() as u8;
    pixel[0] = mix(ink.b, pixel[0]);
    pixel[1] = mix(ink.g, pixel[1]);
    pixel[2] = mix(ink.r, pixel[2]);
    pixel[3] = (out * 255.).round() as u8;
}

/// The gridlines and the dither at device scale as Chrome draws them, rasterised once: a quad a cell costs 9 ms a frame.
fn ground_image(fills: &[(&[f64], Hsla)], grid: Hsla, top: f64, w: f64, h: f64, scale: f64) -> Option<Arc<RenderImage>> {
    let (dw, dh) = ((w * scale).round() as usize, (h * scale).round() as usize);
    if dw == 0 || dh == 0 {
        return None;
    }
    let mut bgra = vec![0u8; dw * dh * 4];
    let grid_ink = Rgba::from(grid);
    // The web's gridlines are 1 px strokes centred on their level, dashed 2 on 4 from the plot's left.
    for g in 0..=PARTS {
        let level = h - PAD - g as f64 / PARTS as f64 * (h - PAD * 2.);
        let (y0, y1) = ((level - 0.5) * scale, (level + 0.5) * scale);
        for row in (y0.floor().max(0.) as usize)..(y1.ceil() as usize).min(dh) {
            let cover = (y1.min(row as f64 + 1.) - y0.max(row as f64)).max(0.);
            for dx in (0..dw).filter(|dx| g == 0 || (*dx as f64 / scale) % 6. < 2.) {
                let at = (row * dw + dx) * 4;
                blend(&mut bgra[at..at + 4], grid_ink, cover * grid_ink.a as f64);
            }
        }
    }
    let points: Vec<&[f64]> = fills.iter().map(|(points, _)| *points).collect();
    let inks: Vec<(Rgba, f64)> =
        fills.iter().enumerate().map(|(rank, (_, ink))| (Rgba::from(*ink), if rank == 0 { 0.5 } else { 0.45 })).collect();
    dither_cells(&points, top, w, h, |rank, px, py| {
        let (ink, alpha) = inks[rank];
        let alpha = alpha * ink.a as f64;
        let (x0, x1, y0, y1) = (px * scale, (px + DOT) * scale, py * scale, (py + DOT) * scale);
        for dy in (y0.floor() as usize)..(y1.ceil() as usize).min(dh) {
            let cover_y = (y1.min(dy as f64 + 1.) - y0.max(dy as f64)).max(0.);
            for dx in (x0.floor() as usize)..(x1.ceil() as usize).min(dw) {
                // Chrome's canvas covers a rect's corner pixel as much as the lesser of its two edges.
                let a = alpha * cover_y.min((x1.min(dx as f64 + 1.) - x0.max(dx as f64)).max(0.));
                if a > 0. {
                    let at = (dy * dw + dx) * 4;
                    blend(&mut bgra[at..at + 4], ink, a);
                }
            }
        }
    });
    let buffer = ImageBuffer::from_raw(dw as u32, dh as u32, bgra)?;
    Some(Arc::new(RenderImage::new(vec![Frame::new(buffer)])))
}

fn ground_key(fills: &[(&[f64], Hsla)], grid: Hsla, top: f64, w: f64, scale: f32) -> u64 {
    let mut hasher = DefaultHasher::new();
    (top.to_bits(), w.to_bits(), scale.to_bits()).hash(&mut hasher);
    let inks = fills.iter().map(|(_, ink)| ink).chain([&grid]);
    inks.flat_map(|ink| [ink.h, ink.s, ink.l, ink.a]).for_each(|c| c.to_bits().hash(&mut hasher));
    fills.iter().flat_map(|(points, _)| points.iter()).for_each(|v| v.to_bits().hash(&mut hasher));
    hasher.finish()
}

fn total(line: &ChartLine) -> f64 {
    line.points.iter().sum()
}

/// What the plot's canvas paints, moved in whole so a frame clones none of it.
struct Plot {
    state: Entity<ChartState>,
    lines: Vec<ChartLine>,
    /// Indices into `lines`, biggest total first: the dither's ranks, and the lead at 0.
    by_total: Vec<usize>,
    top: f64,
    n: usize,
    hovered: Option<usize>,
    grid: Hsla,
    crosshair: Hsla,
}

impl Plot {
    fn paint(self, bounds: Bounds<Pixels>, hitbox: Hitbox, window: &mut Window, cx: &mut App) {
        let origin = bounds.origin;
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let at = |x: f32, y: f64| origin + point(px(x), px(y as f32));
        let x_of = |i: usize| if self.n > 1 { i as f32 / (self.n - 1) as f32 * w } else { 0. };
        let y = |v: f64| y_of(v, self.top, h as f64);

        let scale = window.scale_factor();
        // The web's canvas is as wide as its rounded client width, and stretched over the plot's true width.
        let fills: Vec<(&[f64], Hsla)> = self.by_total.iter().map(|&i| (self.lines[i].points.as_slice(), self.lines[i].ink)).collect();
        let key = ground_key(&fills, self.grid, self.top, w.round() as f64, scale);
        let cached = self.state.read(cx).ground.as_ref().filter(|(k, _)| *k == key).map(|(_, image)| image.clone());
        let image = match cached {
            Some(image) => Some(image),
            None => {
                let image = ground_image(&fills, self.grid, self.top, w.round() as f64, h as f64, scale as f64);
                let old = self.state.update(cx, |state, _| std::mem::replace(&mut state.ground, image.clone().map(|i| (key, i))));
                if let Some((_, old)) = old {
                    cx.drop_image(old, Some(window));
                }
                image
            }
        };
        if let Some(image) = image {
            window.paint_image(bounds, bounds, Default::default(), image, 0, false).ok();
        }

        let lead = self.by_total.first().copied();
        for (i, line) in self.lines.iter().enumerate().rev() {
            if line.points.len() < 2 {
                continue;
            }
            let width: f32 = if Some(i) == lead { 1.75 } else { 1.4 };
            let options = StrokeOptions::default().with_line_width(width).with_line_join(LineJoin::Round).with_line_cap(LineCap::Round);
            let mut path = PathBuilder::stroke(px(width)).with_style(PathStyle::Stroke(options));
            for (step, v) in line.points.iter().enumerate() {
                let p = at(x_of(step), y(*v));
                if step == 0 {
                    path.move_to(p);
                } else {
                    path.line_to(p);
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, line.ink);
            }
        }

        if let Some(i) = self.hovered {
            hairline(window, at(x_of(i), 0.), bounds.size.height, self.crosshair);
        }
        if let Some(lead) = lead.filter(|_| self.n > 0) {
            let step = self.hovered.unwrap_or(self.n - 1);
            let v = self.lines[lead].points.get(step).copied().unwrap_or(0.);
            let r = END_DOT / 2.;
            let dot = Bounds::new(at(x_of(step) - r, y(v) - r as f64), size(px(END_DOT), px(END_DOT)));
            window.paint_quad(quad(dot, px(r), self.lines[lead].ink, px(0.), transparent_black(), BorderStyle::default()));
        }

        let (state, n) = (self.state, self.n);
        let left = state.clone();
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble {
                hover(&state, hitbox.is_hovered(window).then(|| hover_index(f32::from(e.position.x - origin.x), w, n)).flatten(), cx);
            }
        });
        window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble {
                hover(&left, None, cx);
            }
        });
    }
}

/// The hover line, 1 px wide and centred on its step.
fn hairline(window: &mut Window, top: Point<Pixels>, height: Pixels, ink: Hsla) {
    window.paint_quad(fill(Bounds::new(point(top.x - px(0.5), top.y), size(px(1.), height)), ink));
}

fn hover(state: &Entity<ChartState>, next: Option<usize>, cx: &mut App) {
    state.update(cx, |state, cx| {
        if state.hovered != next {
            state.hovered = next;
            cx.notify();
        }
    })
}

/// The tooltip over the hovered step: the step's word, then each line with a figure there.
fn tooltip(word: String, rows: Vec<(Hsla, SharedString, String)>, mono: SharedString, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let (xs, xs_line) = text::XS;
    v_flex()
        .flex_none()
        .rounded(ui_size::BUTTON_RADIUS)
        .border_1()
        .border_color(p.border)
        .bg(p.popover)
        .shadow(p.tooltip_shadow.clone())
        .px_2()
        .py_1()
        .gap_1()
        .text_size(xs)
        .line_height(xs_line)
        .text_color(p.foreground)
        .whitespace_nowrap()
        .child(div().font_family(mono.clone()).text_color(p.muted_foreground).child(word))
        .children(rows.into_iter().map(|(ink, label, figure)| {
            h_flex()
                .justify_between()
                .gap_4()
                .child(h_flex().gap_2().child(div().w(px(12.)).h(px(2.)).rounded_full().bg(ink)).child(label))
                .child(div().font_family(mono.clone()).child(figure))
        }))
}

impl RenderOnce for UsageChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let mono = cx.theme().mono_font_family.clone();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
            cx.on_release(|state: &mut ChartState, cx| {
                if let Some((_, image)) = state.ground.take() {
                    cx.drop_image(image, None);
                }
            })
            .detach();
            ChartState::default()
        });
        let n = self.steps.len();
        let hovered = state.read(cx).hovered.filter(|&i| i < n);
        let top = super::model::nice_top(self.lines.iter().flat_map(|line| line.points.iter().copied()).fold(0., f64::max));
        let mut by_total: Vec<usize> = (0..self.lines.len()).collect();
        by_total.sort_by(|&a, &b| total(&self.lines[b]).total_cmp(&total(&self.lines[a])));

        let labels: Vec<SharedString> = (0..=PARTS).map(|g| (self.figure)(top * g as f64 / PARTS as f64).into()).collect();
        let half = f32::from(AXIS_TEXT) / 2.;
        // As wide as the widest label to the fraction, as the web's column is; a text box would round it up.
        let widest = labels
            .iter()
            .map(|label| {
                let run = TextRun {
                    len: label.len(),
                    font: font(mono.clone()),
                    color: p.muted_foreground,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                window.text_system().shape_line(label.clone(), AXIS_TEXT, &[run], None).width
            })
            .fold(px(0.), Pixels::max);
        let gutter = div()
            .relative()
            .flex_none()
            .w(widest)
            .h(CHART_HEIGHT)
            .font_family(mono.clone())
            .text_size(AXIS_TEXT)
            .line_height(AXIS_TEXT)
            .whitespace_nowrap()
            .text_color(p.muted_foreground)
            .children(labels.iter().enumerate().map(|(g, label)| {
                let gy = HEIGHT - PAD - g as f64 / PARTS as f64 * (HEIGHT - PAD * 2.);
                div().absolute().right_0().top(px(gy as f32 - half)).child(label.clone())
            }));

        let tip = hovered.map(|i| {
            let rows = self
                .lines
                .iter()
                .filter(|line| line.points.get(i).copied().unwrap_or(0.) > 0.)
                .map(|line| (line.ink, line.label.clone(), (self.figure)(line.points[i])))
                .collect();
            let cell = 1. / (n.max(2) - 1) as f32;
            let centre = if n == 1 {
                0.5
            } else if i == 0 {
                cell / 4.
            } else if i == n - 1 {
                1. - cell / 4.
            } else {
                i as f32 * cell
            };
            div()
                .absolute()
                .top(-TOOLTIP_OFFSET)
                .left(relative(centre))
                .w_0()
                .h_0()
                .flex()
                .justify_center()
                .items_end()
                .child(deferred(tooltip((self.step_word)(self.steps[i]), rows, mono.clone(), cx)))
        });

        let plot = Plot { state, lines: self.lines, by_total, top, n, hovered, grid: p.border, crosshair: p.foreground.opacity(0.3) };
        let plot = div()
            .relative()
            .h(CHART_HEIGHT)
            .child(
                canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    move |bounds, hitbox, window, cx| plot.paint(bounds, hitbox, window, cx),
                )
                .absolute()
                .size_full(),
            )
            .children(tip);

        let ticks = div()
            .relative()
            .h(TICK_ROW)
            .font_family(mono)
            .text_size(text::XS.0)
            .line_height(TICK_ROW)
            .whitespace_nowrap()
            .text_color(p.muted_foreground)
            .children(self.ticks.into_iter().map(|tick| {
                let word = div().flex_none().child(tick.word);
                if tick.at <= 0. {
                    div().absolute().top_0().left_0().child(word)
                } else if tick.at >= 1. {
                    div().absolute().top_0().right_0().child(word)
                } else {
                    div().absolute().top_0().left(relative(tick.at)).w_0().flex().justify_center().child(word)
                }
            }));

        h_flex().items_start().gap(GAP).child(gutter).child(v_flex().flex_1().min_w_0().gap(GAP).child(plot).child(ticks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_index_rounds_to_the_nearest_step() {
        assert_eq!(hover_index(0., 600., 7), Some(0));
        assert_eq!(hover_index(49., 600., 7), Some(0));
        assert_eq!(hover_index(51., 600., 7), Some(1));
        assert_eq!(hover_index(560., 600., 7), Some(6));
        assert_eq!(hover_index(600., 600., 7), Some(6));
        assert_eq!(hover_index(300., 600., 1), Some(0));
        assert_eq!(hover_index(-1., 600., 7), None);
        assert_eq!(hover_index(10., 600., 0), None);
    }

    /// From `paintDither` in usageChart.tsx run under node on the same input.
    #[test]
    fn dither_cells_match_the_web() {
        let want = [
            (0, 0, 13),
            (0, 0, 17),
            (0, 0, 21),
            (0, 0, 25),
            (0, 0, 33),
            (0, 2, 17),
            (0, 2, 19),
            (0, 4, 20),
            (0, 4, 24),
            (0, 4, 28),
            (0, 6, 26),
            (0, 8, 29),
            (0, 8, 33),
            (0, 10, 26),
            (0, 12, 24),
            (0, 12, 28),
            (0, 14, 22),
            (1, 2, 26),
            (1, 2, 30),
            (1, 6, 24),
            (1, 6, 28),
            (1, 10, 23),
            (1, 10, 27),
            (1, 14, 28),
        ];
        let mut got = Vec::new();
        dither_cells(&[&[3., 1., 2.], &[1., 2., 0.5]], 4., 16., 40., |rank, x, y| got.push((rank, x as i64, y as i64)));
        assert_eq!(got, want);
    }

    #[test]
    fn the_ground_covers_cells_and_gridlines_as_chrome_does() {
        let (ink, grid) = (gpui_kit::rgb(0xff0000).into(), gpui_kit::rgb(0x0000ff).into());
        let image = ground_image(&[(&[3., 1., 2.], ink)], grid, 4., 16., 40., 2.).unwrap();
        assert_eq!((image.size(0).width.0, image.size(0).height.0), (32, 80));
        let bytes = image.as_bytes(0).unwrap();
        let alpha = |x: usize, y: usize| bytes[(y * 32 + x) * 4 + 3];
        // The cell at (0, 13) covers device rows 26..28.8: two whole pixels and four fifths of the third.
        assert_eq!((alpha(0, 27), alpha(1, 27), alpha(2, 27), alpha(2, 28)), (128, 128, 102, 102));
        assert_eq!(alpha(4, 27), 0);
        // The floor at y 34 is a solid 1 px stroke, two device rows; the gridline at 27 is dashed 2 on 4.
        assert_eq!((alpha(5, 67), alpha(5, 68), alpha(5, 66), alpha(5, 69)), (255, 255, 0, 0));
        assert_eq!((alpha(3, 53), alpha(4, 53), alpha(12, 54)), (255, 0, 255));
    }
}
