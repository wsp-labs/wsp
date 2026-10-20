// SPDX-License-Identifier: AGPL-3.0-only
//! A fresh thread's headline and the field behind it (EmptyHero.tsx, `EmptyThread` in ChatView.tsx): the project's
//! mark over "What should we build in <project>?", and an ASCII dither in the theme's `--hero-field`, a slow noise
//! field drawn as mono characters by brightness, clear in the middle where the stack stands.

use std::time::{Duration, Instant};

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    canvas, div, font, point, prelude::*, px, size, App, Bounds, Context, Entity, Hsla, Pixels, SharedString, Subscription, Task, TextRun,
    Window,
};

use crate::ui::{motion, ActivePalette};

/// `RAMP`: a cell's character by its level, the first drawing nothing.
pub const RAMP: &str = " .:~>x*#";
pub const CELL_W: f64 = 10.;
pub const CELL_H: f64 = 17.;
/// A frame every 120 ms while the field is on screen.
pub const FRAME: Duration = Duration::from_millis(120);
const GLYPH: Pixels = px(11.);

fn hash(x: f64, y: f64) -> f64 {
    let h = (x * 127.1 + y * 311.7).sin() * 43758.5453;
    h - h.floor()
}

fn smooth(t: f64) -> f64 {
    t * t * (3. - 2. * t)
}

/// One octave of value noise.
fn octave(x: f64, y: f64) -> f64 {
    let (xi, yi) = (x.floor(), y.floor());
    let (u, v) = (smooth(x - xi), smooth(y - yi));
    let top = hash(xi, yi) * (1. - u) + hash(xi + 1., yi) * u;
    let bottom = hash(xi, yi + 1.) * (1. - u) + hash(xi + 1., yi + 1.) * u;
    top * (1. - v) + bottom * v
}

fn field(x: f64, y: f64) -> f64 {
    octave(x, y) * 0.6 + octave(x * 2.1 + 5.2, y * 2.1 + 1.3) * 0.3 + octave(x * 4.3 + 9.1, y * 4.3 + 7.7) * 0.1
}

/// The level of the cell at column `c` and row `r` of a `w` by `h` field `t` seconds in: 0 draws nothing.
pub fn level(c: usize, r: usize, w: f64, h: f64, t: f64) -> usize {
    let (c, r) = (c as f64, r as f64);
    let dx = (c * CELL_W + CELL_W / 2. - w / 2.) / (w * 0.42);
    let dy = (r * CELL_H + CELL_H / 2. - h * 0.46) / (h * 0.5);
    let clear = ((dx.hypot(dy) - 0.55) / 0.5).clamp(0., 1.);
    if clear == 0. {
        return 0;
    }
    let v = (field(c * 0.07 + t * 0.018, r * 0.12 - t * 0.006) - 0.42) * 2.4 * clear;
    let top = RAMP.len() - 1;
    if v <= 0. {
        0
    } else {
        ((v * RAMP.len() as f64).floor() as usize).min(top)
    }
}

/// A level's ink: 14% at the faintest, up to 44%.
pub fn alpha(level: usize) -> f32 {
    0.14 + 0.3 * (level as f32 / (RAMP.len() - 1) as f32)
}

/// `HeroField`: painted behind the whole page, never over it, while the composer it follows is on a thread with no
/// message yet. A view of its own, so a frame repaints the field alone and not the composer; it asks for its next frame
/// only when it paints, so a field off screen asks for none, and it paints one still frame under reduced motion.
pub struct HeroField {
    composer: Option<Entity<super::Composer>>,
    born: Option<Instant>,
    frames: u64,
    _next: Option<Task<()>>,
    _fresh: Option<Subscription>,
}

impl HeroField {
    pub fn new() -> Self {
        HeroField { composer: None, born: None, frames: 0, _next: None, _fresh: None }
    }

    /// Stands behind `composer` while its thread has no message; none hides the field.
    pub fn follow(&mut self, composer: Option<Entity<super::Composer>>, cx: &mut Context<Self>) {
        self._fresh = composer.as_ref().map(|c| cx.observe(c, |_, _, cx| cx.notify()));
        self.composer = composer;
        cx.notify();
    }

    /// How many next frames the field asked for.
    pub fn frames(&self) -> u64 {
        self.frames
    }
}

impl Default for HeroField {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for HeroField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.composer.as_ref().is_some_and(|c| c.read(cx).fresh()) {
            self._next = None;
            return div().into_any_element();
        }
        let now = cx.background_executor().now();
        let born = *self.born.get_or_insert(now);
        let still = motion::reduced(cx);
        let t = if still { 0. } else { now.saturating_duration_since(born).as_secs_f64() };
        self._next = (!still).then(|| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(FRAME).await;
                _ = this.update(cx, |this, cx| {
                    this.frames += 1;
                    cx.notify();
                });
            })
        });
        let ink = cx.palette().hero_field;
        let family = cx.theme().mono_font_family.clone();
        div()
            .debug_selector(|| "hero-field".into())
            .size_full()
            .child(canvas(|_, _, _| {}, move |bounds, _, window, _| paint(bounds, t, ink, &family, window)).size_full())
            .into_any_element()
    }
}

/// The field's cells at `t`, each level's glyph shaped once a frame and drawn at its cell's top left, all in one layer,
/// since a shaped line painted per cell sorts a layer of its own into the frame each time.
fn paint(bounds: Bounds<Pixels>, t: f64, ink: Hsla, family: &SharedString, window: &mut Window) {
    let (w, h) = (f64::from(f32::from(bounds.size.width)), f64::from(f32::from(bounds.size.height)));
    if w <= 0. || h <= 0. {
        return;
    }
    let run = TextRun { len: 1, font: font(family.clone()), color: ink, background_color: None, underline: None, strikethrough: None };
    let glyphs: Vec<_> = (1..RAMP.len())
        .map(|l| {
            let line = window.text_system().shape_line(SharedString::new_static(&RAMP[l..l + 1]), GLYPH, std::slice::from_ref(&run), None);
            let glyph = line.runs.first().and_then(|r| Some((r.font_id, r.glyphs.first()?.id)));
            (glyph, line.ascent, ink.opacity(alpha(l)))
        })
        .collect();
    let (cols, rows) = ((w / CELL_W).ceil() as usize, (h / CELL_H).ceil() as usize);
    window.paint_layer(bounds, |window| {
        for r in 0..rows {
            for c in 0..cols {
                let l = level(c, r, w, h, t);
                if l == 0 {
                    continue;
                }
                // `textBaseline = "top"`: the glyph's ascent stands on the cell's top edge.
                let (Some((font_id, glyph)), ascent, color) = glyphs[l - 1] else { continue };
                let at = bounds.origin + point(px((c as f64 * CELL_W) as f32), px((r as f64 * CELL_H) as f32) + ascent);
                _ = window.paint_glyph(at, font_id, glyph, GLYPH, color);
            }
        }
    });
}

/// `EmptyThread`: the project's mark over the question naming it, centred; `picker` underlines the name in dots, as
/// the project home's `HomeProjectPicker` does.
pub fn heading(name: SharedString, mark: gpui_kit::AnyElement, picker: bool, window: &Window, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    // `text-2xl sm:text-3xl`.
    let (text, line) = if crate::ui::narrow::width(window).sm { (px(24.), px(32.)) } else { (px(30.), px(36.)) };
    let question = name.clone();
    let name = div().debug_selector(|| "hero-project".into()).min_w_0().max_w(px(256.)).truncate().child(name);
    let name = if picker {
        let dots = p.foreground.opacity(0.6);
        div()
            .id("hero-project-picker")
            .flex_none()
            .max_w(px(256.))
            .flex()
            .flex_col()
            .child(name)
            .child(canvas(|_, _, _| {}, move |b, _, window, _| dotted(b, dots, window)).w_full().h(px(1.)))
            .into_any_element()
    } else {
        name.flex_none().into_any_element()
    };
    v_flex()
        .debug_selector(|| "hero-heading".into())
        .items_center()
        .justify_center()
        .gap_6()
        .px_6()
        .child(div().flex_none().size(px(40.)).child(mark))
        .child(
            h_flex()
                .debug_selector(|| format!("hero-question:What should we build in {question}?"))
                .w_full()
                .max_w(px(1024.))
                .justify_center()
                .items_start()
                .text_size(text)
                .line_height(line)
                .text_color(p.foreground)
                .child(div().flex_none().whitespace_nowrap().child("What should we build in "))
                .child(name)
                .child(div().flex_none().child("?")),
        )
}

/// `border-dotted` at 1 px: a square dot every other pixel.
fn dotted(bounds: Bounds<Pixels>, ink: Hsla, window: &mut Window) {
    let mut x = bounds.origin.x;
    while x < bounds.right() {
        window.paint_quad(gpui_kit::fill(Bounds { origin: point(x, bounds.origin.y), size: size(px(1.), px(1.)) }, ink));
        x += px(2.);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_is_clear_in_the_middle_and_drawn_at_the_edges() {
        let (w, h) = (1024., 748.);
        let (cols, rows) = ((w / CELL_W).ceil() as usize, (h / CELL_H).ceil() as usize);
        assert_eq!(level(cols / 2, (rows as f64 * 0.46) as usize, w, h, 0.), 0, "the stack's middle is clear");
        let edge = (0..rows).flat_map(|r| [(0, r), (cols - 1, r)]).filter(|&(c, r)| level(c, r, w, h, 0.) > 0).count();
        assert!(edge > rows / 4, "the field draws along the sides: {edge} of {}", rows * 2);
        let moved =
            (0..cols).flat_map(|c| (0..rows).map(move |r| (c, r))).filter(|&(c, r)| level(c, r, w, h, 0.) != level(c, r, w, h, 3.)).count();
        assert!(moved > 0, "the field drifts with time");
    }

    #[test]
    fn a_level_is_inked_from_fourteen_to_forty_four_percent() {
        assert!((alpha(1) - (0.14 + 0.3 / 7.)).abs() < 1e-6);
        assert!((alpha(7) - 0.44).abs() < 1e-6);
    }
}
