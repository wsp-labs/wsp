// SPDX-License-Identifier: AGPL-3.0-only
//! `ChartPlot`: the rule lines or none, the baseline, and each run as a 1.5 px line over a fill that fades down from
//! its ink. A gap in the readings is a break between runs. The dot on the newest point is drawn where asked.

use gpui_kit::{
    canvas, div, linear_color_stop, linear_gradient, point, prelude::*, px, quad, size, transparent_black, App, BorderStyle, Bounds, Hsla,
    PathBuilder, Pixels, Window,
};

use super::chart::stroke_line;
use super::theme::ActivePalette;

/// `CHART_PAD` of `CHART_H`: the room kept above the top and below the baseline.
const PAD: f32 = 1. / 96.;

#[derive(IntoElement)]
pub struct Sparkline {
    runs: Vec<Vec<(f32, f32)>>,
    ink: Hsla,
    rules: bool,
    baseline: bool,
    fill: f32,
    height: Pixels,
    end: Option<Hsla>,
}

impl Sparkline {
    /// One run in `ink`, its points 0 to 1 across and 0 to 1 up, with the rules, a fill from 20% and `ChartPlot`'s
    /// `h-24`.
    pub fn new(points: Vec<(f32, f32)>, ink: Hsla) -> Self {
        Sparkline { runs: vec![points], ink, rules: true, baseline: true, fill: 0.2, height: px(96.), end: None }
    }

    /// Runs with a break between each, where the readings stopped.
    pub fn runs(runs: Vec<Vec<(f32, f32)>>, ink: Hsla) -> Self {
        Sparkline { runs, ..Sparkline::new(Vec::new(), ink) }
    }

    /// `rules={false}`: no rule lines, the baseline at the border's 60%.
    pub fn no_rules(mut self) -> Self {
        self.rules = false;
        self
    }

    /// The line alone, as a slate's sparkline draws it: no rules, no baseline, no fill.
    pub fn bare(mut self) -> Self {
        (self.rules, self.baseline, self.fill) = (false, false, 0.);
        self
    }

    /// `fillOpacity`: the fill's ink at the top.
    pub fn fill(mut self, opacity: f32) -> Self {
        self.fill = opacity;
        self
    }

    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// `end`: the newest point's 6 px dot in the ink, ringed 2 px in `ring` (`ring-2 ring-background`).
    pub fn end(mut self, ring: Hsla) -> Self {
        self.end = Some(ring);
        self
    }
}

impl RenderOnce for Sparkline {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let border = cx.palette().border;
        let Sparkline { runs, ink, rules, baseline, fill, height, end } = self;
        let base = if rules { border } else { border.opacity(0.6) };
        let plot = canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window: &mut Window, _| {
                let (o, w, h) = (bounds.origin, bounds.size.width, bounds.size.height);
                let pad = h * PAD;
                let y_at = |v: f32| o.y + pad + (h - pad * 2.) * (1. - v);
                let mut rule = |y: Pixels, dashed: bool, ink: Hsla| {
                    stroke_line(window, &[point(o.x, y), point(o.x + w, y)], 1., dashed.then_some((2., 3.)), ink);
                };
                if rules {
                    for at in [0.25, 0.5, 0.75, 1.] {
                        rule(y_at(at), at < 1., border);
                    }
                }
                if baseline {
                    rule(o.y + h - pad, false, base);
                }
                let at = |(x, y): (f32, f32)| point(o.x + w * x, y_at(y));
                for run in runs.iter().filter(|r| !r.is_empty()) {
                    let (first, last) = (run[0], run[run.len() - 1]);
                    if run.len() > 1 && fill > 0. {
                        let mut area = PathBuilder::fill();
                        area.move_to(at(first));
                        for pt in &run[1..] {
                            area.line_to(at(*pt));
                        }
                        area.line_to(point(o.x + w * last.0, o.y + h));
                        area.line_to(point(o.x + w * first.0, o.y + h));
                        area.close();
                        if let Ok(path) = area.build() {
                            let fade =
                                linear_gradient(180., linear_color_stop(ink.opacity(fill), 0.), linear_color_stop(ink.opacity(0.), 1.));
                            window.paint_path(path, fade);
                        }
                    }
                    let mut line = PathBuilder::stroke(px(1.5));
                    line.move_to(at(first));
                    for pt in run.iter().skip(1) {
                        line.line_to(at(*pt));
                    }
                    if run.len() == 1 {
                        line.line_to(at(first));
                    }
                    if let Ok(path) = line.build() {
                        window.paint_path(path, ink);
                    }
                }
                let newest = runs.iter().rev().find_map(|run| run.last()).copied();
                if let (Some(ring), Some(newest)) = (end, newest) {
                    let centre = at(newest);
                    for (r, color) in [(5., ring), (3., ink)] {
                        let dot = Bounds::new(centre - point(px(r), px(r)), size(px(r * 2.), px(r * 2.)));
                        window.paint_quad(quad(dot, px(r), color, px(0.), transparent_black(), BorderStyle::default()));
                    }
                }
            },
        )
        .size_full();
        div().debug_selector(|| "sparkline".into()).w_full().h(height).child(plot)
    }
}
