// SPDX-License-Identifier: AGPL-3.0-only
//! The app's one chart, after `apps/web/src/settings/usageChart.tsx`: one line per series on one scale, a Bayer dither
//! under each line in its own ink (or, stacked, each band down to the one under it), dashed gridlines with the axis
//! figures beside them, the lead line's end dot, the end ticks under the plot, and a tooltip over the step under the
//! pointer. The Usage page draws it at 300 px with figures on a round top; a slate draws it small on a fixed axis,
//! with the lines and the scale easing from their old points. On the Usage page the axis figures roll as the web's do.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    anchored, canvas, deferred, div, font, point, prelude::*, px, quad, relative, size, transparent_black, Anchor, AnchoredPositionMode,
    App, BorderStyle, Bounds, DispatchPhase, ElementId, Entity, EntityId, Hitbox, HitboxBehavior, Hsla, MouseMoveEvent, PathBuilder,
    PathStyle, Pixels, Point, RenderImage, Rgba, SharedString, StrokeOptions, TextRun, Window,
};
use image::{Frame as ImageFrame, ImageBuffer};
use lyon::tessellation::{LineCap, LineJoin};

use super::status::still;
use super::{hover_card, size as ui_size, text, ActivePalette, DigitRoll};

pub const CHART_HEIGHT: Pixels = px(300.);
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
/// The gridlines over the floor when the axis names no parts of its own.
const PARTS: usize = 4;
const TICK_ROW: Pixels = px(16.);
const END_DOT: f32 = 6.;
const TOOLTIP_OFFSET: Pixels = px(6.);
/// Base UI's `collisionPadding`: the tooltip shifts to stay this far inside the window.
const TOOLTIP_MARGIN: Pixels = px(5.);
/// The tooltip's key among its chart's hover cards.
const CARD: &str = "chart-tip";
/// The small chart's hover opens beside its step this far off, as the web's `sideOffset: 8`.
const SIDE_OFFSET: f32 = 8.;
const TWEEN: Duration = Duration::from_millis(520);

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

/// A fixed axis: from, to, and the parts between its gridlines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    pub from: f64,
    pub to: f64,
    pub parts: usize,
}

/// The lines and the scale as drawn this frame.
#[derive(Clone, PartialEq)]
struct Shape {
    points: Vec<(SharedString, Vec<f64>)>,
    top: f64,
}

impl Shape {
    /// `useTween`'s frame at eased share `e`: a line new to the chart rises from 0, a line of another length is read off
    /// the old one at the same share of its way.
    fn toward(&self, to: &Shape, e: f64) -> Shape {
        let points = to
            .points
            .iter()
            .map(|(key, end)| {
                let old = self.points.iter().find(|(k, _)| k == key).map(|(_, p)| p.as_slice()).unwrap_or(&[]);
                let eased = end
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let o = if old.is_empty() {
                            0.
                        } else {
                            let at = (i as f64 * (old.len() - 1) as f64 / (end.len().max(2) - 1) as f64).round() as usize;
                            old.get(at).copied().unwrap_or(0.)
                        };
                        o + (v - o) * e
                    })
                    .collect();
                (key.clone(), eased)
            })
            .collect();
        let top = if self.top == 0. { to.top } else { self.top + (to.top - self.top) * e };
        Shape { points, top }
    }
}

struct Tween {
    from: Shape,
    to: Shape,
    started: Instant,
}

/// The step under the pointer, the plot's ground as last rasterised with the hash of what it was drawn from, where
/// the plot last stood, and the tween under way.
#[derive(Default)]
struct ChartState {
    hovered: Option<usize>,
    ground: Option<(u64, Arc<RenderImage>)>,
    plot: Option<Bounds<Pixels>>,
    tween: Option<Tween>,
}

#[derive(IntoElement)]
pub struct Chart {
    id: ElementId,
    steps: Vec<f64>,
    lines: Vec<ChartLine>,
    ticks: Vec<Tick>,
    height: Pixels,
    figure: Rc<dyn Fn(f64) -> String>,
    axis_figure: Option<Rc<dyn Fn(f64) -> String>>,
    step_word: Rc<dyn Fn(f64) -> String>,
    axis: Option<Axis>,
    small: bool,
    every_figure: bool,
    stacked: bool,
    fill: bool,
    tween: bool,
    roll: bool,
}

impl Chart {
    pub fn new(id: impl Into<ElementId>, steps: Vec<f64>, lines: Vec<ChartLine>, ticks: Vec<Tick>) -> Self {
        Chart {
            id: id.into(),
            steps,
            lines,
            ticks,
            height: CHART_HEIGHT,
            figure: Rc::new(|v| v.to_string()),
            axis_figure: None,
            step_word: Rc::new(|t| t.to_string()),
            axis: None,
            small: false,
            every_figure: false,
            stacked: false,
            fill: true,
            tween: false,
            roll: false,
        }
    }

    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// The words for a figure on the axis and in the tooltip.
    pub fn figure(mut self, figure: impl Fn(f64) -> String + 'static) -> Self {
        self.figure = Rc::new(figure);
        self
    }

    /// The axis's own words where they differ: a unit the legend already names is not said again on every gridline.
    pub fn axis_figure(mut self, figure: impl Fn(f64) -> String + 'static) -> Self {
        self.axis_figure = Some(Rc::new(figure));
        self
    }

    /// The tooltip's first line, naming the step.
    pub fn step_word(mut self, step_word: impl Fn(f64) -> String + 'static) -> Self {
        self.step_word = Rc::new(step_word);
        self
    }

    /// A fixed axis; without one it runs from 0 to a round top in four parts.
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = Some(axis);
        self
    }

    /// The panel's size: 11 px figures, the gutter 8 px off the plot, the plot half the end dot short of the right
    /// edge, and the hover beside its step at the plot's top rather than over it.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// Every line's figure in the hover, a zero too: where the Usage page's zero is a step nothing was spent.
    pub fn every_figure(mut self) -> Self {
        self.every_figure = true;
        self
    }

    /// The lines are a stack's running totals, bottom first: each fills down to the one under it, the first leads, and
    /// the hover says each band's own figure.
    pub fn stacked(mut self, stacked: bool) -> Self {
        self.stacked = stacked;
        self
    }

    /// The dither under the lines; without it they are plain lines that cross.
    pub fn fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    /// Each change eases from where the last one stood, 520 ms out of a cubic, the first rising from the floor.
    pub fn tween(mut self) -> Self {
        self.tween = true;
        self
    }

    /// The axis figures roll to each new figure, as the Usage page's do.
    pub fn roll(mut self) -> Self {
        self.roll = true;
        self
    }
}

/// The axis's top: the smallest round figure at or over the largest point, so the four gridlines read as round numbers.
pub fn nice_top(max: f64) -> f64 {
    if max <= 0.0 {
        return 0.0;
    }
    let magnitude = 10f64.powi(max.log10().floor() as i32);
    let step = [1.0, 1.2, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0].into_iter().find(|m| m * magnitude >= max).unwrap_or(10.0);
    step * magnitude
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

/// The share of the plot a step's hover cell spans, left to right.
fn cell_of(i: usize, n: usize) -> (f32, f32) {
    if n < 2 {
        return (0., 1.);
    }
    let cell = 1. / (n - 1) as f32;
    ((i as f32 * cell - cell / 2.).max(0.), (i as f32 * cell + cell / 2.).min(1.))
}

/// One dither fill: a line's points, and for a stacked band the band under it.
type Fill<'a> = (&'a [f64], Option<&'a [f64]>);

/// Each lit cell under `fills` (biggest first) as `(rank, x, y)` from the plot's origin, as the web's `paintDither` walks them.
fn dither_cells(fills: &[Fill], top: f64, w: f64, h: f64, mut lit: impl FnMut(usize, f64, f64)) {
    if w <= 0. || top == 0. {
        return;
    }
    let floor = h - PAD;
    for (rank, (points, below)) in fills.iter().enumerate() {
        let n = points.len();
        if n < 2 {
            continue;
        }
        let at = |series: &[f64], px: usize| {
            let t = px as f64 / w * (n - 1) as f64;
            let i = (n - 2).min(t.floor() as usize);
            let f = t - i as f64;
            // A band shorter than the line over it holds its last point.
            let v = |k: usize| series.get(k).or(series.last()).copied().unwrap_or(0.);
            v(i) * (1. - f) + v(i + 1) * f
        };
        let density = if rank == 0 || below.is_some() { 0.55 } else { 0.32 };
        let phase = rank * 5;
        for px in (0..w.ceil() as usize).step_by(CELL) {
            let line = y_of(at(points, px), top, h);
            let base = below.map_or(floor, |b| y_of(at(b, px), top, h));
            let mut py = line.ceil();
            while py < base {
                let depth = if below.is_some() { 1. } else { 1. - (py - line) / (floor - line + 1.) };
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

/// What the ground is drawn from: the fills and their inks, and the scale.
struct Ground<'a> {
    fills: Vec<Fill<'a>>,
    inks: Vec<Hsla>,
    top: f64,
}

impl Ground<'_> {
    /// The dither at device scale as Chrome's canvas draws it, rasterised once: a quad a cell costs 9 ms a frame.
    fn image(&self, w: f64, h: f64, scale: f64) -> Option<Arc<RenderImage>> {
        let (dw, dh) = ((w * scale).round() as usize, (h * scale).round() as usize);
        if dw == 0 || dh == 0 {
            return None;
        }
        let mut bgra = vec![0u8; dw * dh * 4];
        let inks: Vec<(Rgba, f64)> = self
            .fills
            .iter()
            .zip(&self.inks)
            .enumerate()
            .map(|(rank, ((_, below), ink))| (Rgba::from(*ink), if rank == 0 || below.is_some() { 0.5 } else { 0.45 }))
            .collect();
        dither_cells(&self.fills, self.top, w, h, |rank, px, py| {
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
        Some(Arc::new(RenderImage::new(vec![ImageFrame::new(buffer)])))
    }

    fn key(&self, w: f64, h: f64, scale: f32) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.top.to_bits(), w.to_bits(), h.to_bits(), scale.to_bits()).hash(&mut hasher);
        self.inks.iter().flat_map(|ink| [ink.h, ink.s, ink.l, ink.a]).for_each(|c| c.to_bits().hash(&mut hasher));
        for (points, below) in &self.fills {
            points.len().hash(&mut hasher);
            points.iter().chain(below.iter().flat_map(|b| b.iter())).for_each(|v| v.to_bits().hash(&mut hasher));
        }
        hasher.finish()
    }
}

fn total(points: &[f64]) -> f64 {
    points.iter().sum()
}

/// What the plot's canvas paints, moved in whole so a frame clones none of it.
struct Plot {
    state: Entity<ChartState>,
    /// The points drawn this frame, on the axis's own scale from its floor.
    lines: Vec<(Vec<f64>, Hsla)>,
    /// Indices into `lines`: the dither's ranks, the lead at 0.
    order: Vec<usize>,
    stacked: bool,
    fill: bool,
    top: f64,
    parts: usize,
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
        let fills: Vec<Fill> = if self.fill {
            self.order
                .iter()
                .enumerate()
                .map(|(rank, &i)| {
                    let below = (self.stacked && rank > 0).then(|| self.lines[self.order[rank - 1]].0.as_slice());
                    (self.lines[i].0.as_slice(), below)
                })
                .collect()
        } else {
            Vec::new()
        };
        let inks = self.order.iter().map(|&i| self.lines[i].1).collect();
        let ground = Ground { fills, inks, top: self.top };
        // The web's canvas is as wide as its rounded client width, and stretched over the plot's true width.
        let key = ground.key(w.round() as f64, h as f64, scale);
        let cached = self.state.read(cx).ground.as_ref().filter(|(k, _)| *k == key).map(|(_, image)| image.clone());
        let image = match cached {
            Some(image) => Some(image),
            None => {
                let image = ground.image(w.round() as f64, h as f64, scale as f64);
                let old = self.state.update(cx, |state, _| std::mem::replace(&mut state.ground, image.clone().map(|i| (key, i))));
                if let Some((_, old)) = old {
                    cx.drop_image(old, Some(window));
                }
                image
            }
        };
        let parts = self.parts.max(1);
        for g in 0..=parts {
            let level = h as f64 - PAD - g as f64 / parts as f64 * (h as f64 - PAD * 2.);
            stroke_line(window, &[at(0., level), at(w, level)], 1., (g > 0).then_some((2., 4.)), self.grid);
        }
        if let Some(image) = image {
            window.paint_image(bounds, bounds, Default::default(), image, 0, false).ok();
        }
        self.state.update(cx, |state, _| state.plot = Some(bounds));

        let lead = self.order.first().copied();
        for (i, (points, ink)) in self.lines.iter().enumerate().rev() {
            if points.len() < 2 {
                continue;
            }
            let width: f32 = if Some(i) == lead { 1.75 } else { 1.4 };
            let options = StrokeOptions::default().with_line_width(width).with_line_join(LineJoin::Round).with_line_cap(LineCap::Round);
            let mut path = PathBuilder::stroke(px(width)).with_style(PathStyle::Stroke(options));
            for (step, v) in points.iter().enumerate() {
                let p = at(x_of(step), y(*v));
                if step == 0 {
                    path.move_to(p);
                } else {
                    path.line_to(p);
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, *ink);
            }
        }

        if let Some(i) = self.hovered {
            stroke_line(window, &[at(x_of(i), 0.), at(x_of(i), h as f64)], 1., None, self.crosshair);
        }
        if let Some(lead) = lead.filter(|_| self.n > 0) {
            let step = self.hovered.unwrap_or(self.n - 1);
            let v = self.lines[lead].0.get(step).copied().unwrap_or(0.);
            let r = END_DOT / 2.;
            let dot = Bounds::new(at(x_of(step) - r, y(v) - r as f64), size(px(END_DOT), px(END_DOT)));
            window.paint_quad(quad(dot, px(r), self.lines[lead].1, px(0.), transparent_black(), BorderStyle::default()));
        }

        let (state, n) = (self.state, self.n);
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
            let step = hitbox.is_hovered(window).then(|| hover_index(f32::from(e.position.x - origin.x), w, n)).flatten();
            if let Some(step) = step.filter(|_| phase == DispatchPhase::Bubble) {
                hover(&state, step, cx);
            }
        });
    }
}

/// A straight stroke `width` wide, dashed `(on, off)` from its start, in points: the web's `non-scaling-stroke` cancels
/// only its viewBox's stretch, and Electron at scale 2 draws a 1 px line two device rows thick.
pub(super) fn stroke_line(window: &mut Window, points: &[Point<Pixels>], width: f32, dash: Option<(f32, f32)>, ink: Hsla) {
    let mut path = PathBuilder::stroke(px(width));
    if let Some((on, off)) = dash {
        path = path.dash_array(&[px(on), px(off)]);
    }
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            path.move_to(*p);
        } else {
            path.line_to(*p);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, ink);
    }
}

/// The pointer on `step`: its tooltip opens, and closes as every hover card does.
fn hover(state: &Entity<ChartState>, step: usize, cx: &mut App) {
    hover_card::open(state.entity_id(), CARD, hover_card::POINTER, cx);
    state.update(cx, |state, cx| {
        if state.hovered != Some(step) {
            state.hovered = Some(step);
            cx.notify();
        }
    })
}

/// The tooltip over the hovered step: the step's word, then each line with a figure there.
fn tooltip(word: String, rows: Vec<(Hsla, SharedString, String)>, mono: SharedString, owner: EntityId, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let (xs, xs_line) = text::XS;
    v_flex()
        .debug_selector(|| "chart-tip".into())
        .relative()
        .child(hover_card::card(owner, CARD))
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

/// The tooltip's width as laid out: its border and sides, and its widest line.
fn tooltip_width(word: &str, rows: &[(Hsla, SharedString, String)], window: &Window, cx: &App) -> f32 {
    let (family, mono) = (cx.theme().font_family.clone(), cx.theme().mono_font_family.clone());
    let size = f32::from(text::XS.0);
    let widest = rows
        .iter()
        .map(|(_, label, figure)| {
            12. + 8.
                + f32::from(text::width(label, size, family.clone(), window))
                + 16.
                + f32::from(text::width(figure, size, mono.clone(), window))
        })
        .fold(f32::from(text::width(word, size, mono.clone(), window)), f32::max);
    widest + 2. * 8. + 2.
}

/// The drawn frame toward `target`: at once when still, else eased from where the last frame stood.
fn tweened(state: &Entity<ChartState>, target: Shape, window: &mut Window, cx: &mut App) -> Shape {
    if still(cx) {
        state.update(cx, |state, _| state.tween = None);
        return target;
    }
    let now = Instant::now();
    let sample = |t: &Tween| {
        let k = (now.saturating_duration_since(t.started).as_secs_f64() / TWEEN.as_secs_f64()).min(1.);
        (t.from.toward(&t.to, 1. - (1. - k).powi(3)), k)
    };
    let (shape, k) = state.update(cx, |state, _| {
        let from = match &state.tween {
            None => Some(Shape {
                points: target.points.iter().map(|(key, points)| (key.clone(), vec![0.; points.len()])).collect(),
                top: target.top,
            }),
            Some(t) if t.to != target => Some(sample(t).0),
            Some(_) => None,
        };
        if let Some(from) = from {
            state.tween = Some(Tween { from, to: target, started: now });
        }
        state.tween.as_ref().map(sample).expect("set above")
    });
    if k < 1. {
        window.request_animation_frame();
    }
    shape
}

impl RenderOnce for Chart {
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
        let height = f64::from(f32::from(self.height));
        let owner = state.entity_id();
        let hovered = state.read(cx).hovered.filter(|&i| i < n && hover_card::shows(owner, CARD, cx));
        let from = self.axis.map_or(0., |a| a.from);
        let parts = self.axis.map_or(PARTS, |a| a.parts.max(1));
        let top = match self.axis {
            Some(a) => a.to - a.from,
            None => nice_top(self.lines.iter().flat_map(|line| line.points.iter().map(|v| v - from)).fold(0., f64::max)),
        };
        let order: Vec<usize> = if self.stacked {
            (0..self.lines.len()).collect()
        } else {
            let mut by_total: Vec<usize> = (0..self.lines.len()).collect();
            by_total.sort_by(|&a, &b| total(&self.lines[b].points).total_cmp(&total(&self.lines[a].points)));
            by_total
        };

        let target = Shape {
            points: self.lines.iter().map(|line| (line.key.clone(), line.points.iter().map(|v| v - from).collect())).collect(),
            top,
        };
        let shape = if self.tween { tweened(&state, target, window, cx) } else { target };

        let axis_text = if self.small { px(11.) } else { text::XS.0 };
        let axis_figure = self.axis_figure.clone().unwrap_or_else(|| self.figure.clone());
        let labels: Vec<SharedString> = (0..=parts).map(|g| axis_figure(from + top * g as f64 / parts as f64).into()).collect();
        let half = f32::from(axis_text) / 2.;
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
                window.text_system().shape_line(label.clone(), axis_text, &[run], None).width
            })
            .fold(px(0.), Pixels::max);
        let gutter = div()
            .relative()
            .flex_none()
            .w(widest)
            .h(self.height)
            .font_family(mono.clone())
            .text_size(axis_text)
            .line_height(axis_text)
            .whitespace_nowrap()
            .text_color(p.muted_foreground)
            .children(labels.iter().enumerate().map(|(g, label)| {
                let gy = height - PAD - g as f64 / parts as f64 * (height - PAD * 2.);
                let figure = if self.roll {
                    DigitRoll::new(SharedString::from(format!("{}-y-{g}", self.id)), label.clone(), axis_text).roll_in().into_any_element()
                } else {
                    label.clone().into_any_element()
                };
                div().absolute().right_0().top(px(gy as f32 - half)).child(figure)
            }));

        let tip = hovered.map(|i| {
            let shown: Vec<(Hsla, SharedString, f64)> = if self.stacked {
                let own = self.lines.iter().enumerate().map(|(at, line)| {
                    let under = if at == 0 { 0. } else { self.lines[at - 1].points.get(i).copied().unwrap_or(0.) };
                    (line.ink, line.label.clone(), line.points.get(i).map(|v| v - under))
                });
                own.rev().filter_map(|(ink, label, v)| v.map(|v| (ink, label, v))).collect()
            } else {
                self.lines.iter().filter_map(|line| line.points.get(i).map(|v| (line.ink, line.label.clone(), *v))).collect()
            };
            let rows: Vec<_> = shown
                .into_iter()
                .filter(|(_, _, v)| self.every_figure || *v > 0.)
                .map(|(ink, label, v)| (ink, label, (self.figure)(v)))
                .collect();
            let word = (self.step_word)(self.steps[i]);
            if self.small {
                let (l, r) = cell_of(i, n);
                let width = tooltip_width(&word, &rows, window, cx);
                let room = window.viewport_size().width;
                let flip = state
                    .read(cx)
                    .plot
                    .is_some_and(|b| f32::from(b.origin.x) + f32::from(b.size.width) * r + SIDE_OFFSET + width > f32::from(room));
                let tip = deferred(tooltip(word, rows, mono.clone(), owner, cx));
                if flip {
                    div().absolute().top_0().left(relative(l)).w_0().h_0().flex().justify_end().child(div().mr(px(SIDE_OFFSET)).child(tip))
                } else {
                    div().absolute().top_0().left(relative(r)).w_0().h_0().flex().child(div().ml(px(SIDE_OFFSET)).child(tip))
                }
            } else {
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
                div().absolute().top(-TOOLTIP_OFFSET).left(relative(centre)).w_0().h_0().child(deferred(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .anchor(Anchor::BottomCenter)
                        .snap_to_window_with_margin(TOOLTIP_MARGIN)
                        .child(tooltip(word, rows, mono.clone(), owner, cx)),
                ))
            }
        });

        let inks: Vec<Hsla> = self.lines.iter().map(|line| line.ink).collect();
        let plot = Plot {
            state,
            lines: shape.points.into_iter().map(|(_, points)| points).zip(inks).collect(),
            order,
            stacked: self.stacked,
            fill: self.fill,
            top: shape.top,
            parts,
            n,
            hovered,
            grid: p.border,
            crosshair: p.foreground.opacity(0.3),
        };
        let plot = div()
            .relative()
            .h(self.height)
            .when(self.small, |el| el.mr(px(END_DOT / 2.)))
            .child(
                canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    move |bounds, hitbox, window, cx| plot.paint(bounds, hitbox, window, cx),
                )
                .absolute()
                .size_full(),
            )
            .when(hovered.is_some(), |el| el.child(hover_card::opener(owner, CARD)))
            .children(tip);

        let ticks = div()
            .relative()
            .h(TICK_ROW)
            .font_family(mono)
            .text_size(axis_text)
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

        let gap = px(12.);
        h_flex()
            .items_start()
            .gap(if self.small { px(8.) } else { gap })
            .child(gutter)
            .child(v_flex().flex_1().min_w_0().gap(gap).child(plot).child(ticks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flush;

    impl Render for Flush {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            let line = ChartLine {
                key: "claude".into(),
                label: "Claude Code".into(),
                points: vec![0., 0., 0., 0., 0., 0., 1860.],
                ink: gpui_kit::red(),
            };
            let chart = Chart::new("flush", (0..7).map(f64::from).collect(), vec![line], Vec::new())
                .figure(|v| format!("{v:.0} tokens"))
                .step_word(|_| "Sun 18 Oct".to_owned());
            div().size_full().pt(px(120.)).pl(px(400.)).child(chart)
        }
    }

    /// The hover at the plot's right end, flush with the window's: the tooltip shifts left to stand inside the window,
    /// as Base UI's positioner keeps it.
    #[gpui_kit::test]
    fn the_tooltip_at_the_right_end_stays_inside_the_window(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Flush);
        cx.simulate_resize(size(px(1000.), px(600.)));
        cx.run_until_parked();
        cx.simulate_mouse_move(point(px(997.), px(270.)), None, gpui_kit::Modifiers::none());
        cx.run_until_parked();
        let tip = cx.debug_bounds("chart-tip").expect("the tooltip over the last step");
        assert!(tip.right() <= px(995.) && tip.right() > px(990.), "5 px inside the window's right edge: {tip:?}");
        assert!(tip.bottom() <= px(120.) - TOOLTIP_OFFSET + px(1.), "over the plot: {tip:?}");
    }

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
        dither_cells(&[(&[3., 1., 2.], None), (&[1., 2., 0.5], None)], 4., 16., 40., |rank, x, y| got.push((rank, x as i64, y as i64)));
        assert_eq!(got, want);
    }

    #[test]
    fn a_stacked_band_fills_down_to_the_band_under_it() {
        let mut got = Vec::new();
        let (bottom, top): (&[f64], &[f64]) = (&[1., 1.], &[3., 3.]);
        dither_cells(&[(bottom, None), (top, Some(bottom))], 4., 8., 40., |rank, _, y| got.push((rank, y)));
        // The floor is 34 and the bottom band's line 27; the top band's line is 13 and it stops above 27.
        assert!(got.iter().filter(|(rank, _)| *rank == 1).all(|(_, y)| (13. ..27.).contains(y)));
        assert!(got.iter().any(|(rank, _)| *rank == 1));
    }

    #[test]
    fn a_band_over_a_shorter_band_holds_its_last_point() {
        let mut got = Vec::new();
        let (bottom, top): (&[f64], &[f64]) = (&[1., 1., 1.], &[3., 3., 3., 3., 3.]);
        dither_cells(&[(bottom, None), (top, Some(bottom))], 4., 16., 40., |rank, _, y| got.push((rank, y)));
        assert!(got.iter().filter(|(rank, _)| *rank == 1).all(|(_, y)| (13. ..27.).contains(y)));
        assert!(got.iter().any(|(rank, _)| *rank == 1));
    }

    #[test]
    fn the_ground_covers_cells_as_chrome_does() {
        let ink = gpui_kit::rgb(0xff0000).into();
        let points: &[f64] = &[3., 1., 2.];
        let ground = Ground { fills: vec![(points, None)], inks: vec![ink], top: 4. };
        let image = ground.image(16., 40., 2.).unwrap();
        assert_eq!((image.size(0).width.0, image.size(0).height.0), (32, 80));
        let bytes = image.as_bytes(0).unwrap();
        let alpha = |x: usize, y: usize| bytes[(y * 32 + x) * 4 + 3];
        // The cell at (0, 13) covers device rows 26..28.8: two whole pixels and four fifths of the third.
        assert_eq!((alpha(0, 27), alpha(1, 27), alpha(2, 27), alpha(2, 28)), (128, 128, 102, 102));
        assert_eq!(alpha(4, 27), 0);
    }

    #[test]
    fn a_tween_reads_old_points_at_the_same_share_and_raises_new_lines() {
        let from = Shape { points: vec![("a".into(), vec![0., 10.])], top: 10. };
        let to = Shape { points: vec![("a".into(), vec![10., 10., 10.]), ("b".into(), vec![4.])], top: 20. };
        let half = from.toward(&to, 0.5);
        assert_eq!(half.points[0].1, vec![5., 10., 10.]);
        assert_eq!(half.points[1].1, vec![2.]);
        assert_eq!(half.top, 15.);
        assert_eq!(from.toward(&to, 1.).points, to.points);
    }
}
