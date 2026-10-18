// SPDX-License-Identifier: AGPL-3.0-only
//! The chart pieces, after `chart.tsx`, `donut.tsx`, `timeline.tsx` and `treemap.tsx`: a line or a stack over a list
//! drawn by the app's one chart, shares of a whole as a ring beside its legend, spans on a shared clock, and sizes as
//! squarified tiles. How each reads its axis, its parts and its spans is `protocol:slate/chart.ts`.

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    canvas, div, point, prelude::*, px, relative, size, AnyElement, App, Bounds, Div, ElementId, Hsla, PathBuilder, PathStyle, Pixels,
    SharedString, StrokeOptions, Window,
};
use lyon::tessellation::LineCap;
use serde_json::Value;

use super::super::model::{Loud, Row};
use super::look::{self, Tone};
use super::PieceCx;
use crate::screens::usage::words::{js_round, time_word};
use crate::ui::chart::{Axis, Chart, ChartLine, Tick};
use crate::ui::controls::bevel;
use crate::ui::{text, tip, ActivePalette};

const NOT_READ: &str = "Not read yet";
/// `gap-2.5`, between a figure's caption and its body.
const FIGURE_GAP: Pixels = px(10.);
/// `rounded-xs`.
const XS_RADIUS: Pixels = px(2.);

pub fn draw(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match p.piece.kind.as_str() {
        "chart" => Some(chart(p, window, cx)),
        "donut" => Some(donut(p, window, cx)),
        "timeline" => Some(timeline(p, window, cx)),
        "treemap" => Some(treemap(p, window, cx)),
        _ => None,
    }
}

/// Whether this piece holds the slate's one accent: the first chart, donut, timeline or treemap that names no tone.
fn loud(p: &PieceCx) -> bool {
    p.slate.is_loud(Loud::Accent, p.id())
}

/// `color-mix(in srgb, a share, b)`.
fn mix(a: Hsla, share: f32, b: Hsla) -> Hsla {
    let (x, y) = (a.to_rgb(), b.to_rgb());
    let alpha = x.a * share + y.a * (1. - share);
    if alpha <= 0. {
        return gpui_kit::transparent_black();
    }
    let c = |m: f32, n: f32| (m * x.a * share + n * y.a * (1. - share)) / alpha;
    gpui_kit::Rgba { r: c(x.r, y.r), g: c(x.g, y.g), b: c(x.b, y.b), a: alpha }.into()
}

fn sized(el: Div, (size, line): (Pixels, Pixels)) -> Div {
    el.text_size(size).line_height(line)
}

const META: (Pixels, Pixels) = (px(11.), px(16.));

fn mono(cx: &App) -> SharedString {
    cx.theme().mono_font_family.clone()
}

fn note(text: impl Into<SharedString>, cx: &App) -> Div {
    sized(div(), text::XS).text_color(cx.palette().muted_foreground).child(text.into())
}

/// A label's name, and what follows its first ", ".
fn split_label(label: &str) -> (String, Option<String>) {
    match label.split_once(", ") {
        Some((name, rest)) => (name.to_owned(), Some(rest.to_owned())),
        None => (label.to_owned(), None),
    }
}

/// The figcaption of a donut, a timeline and a treemap: the name, and what follows its comma muted and small.
fn caption(label: &str, cx: &App) -> Div {
    let (name, rest) = split_label(label);
    sized(h_flex().flex_wrap().items_baseline().gap_x_2(), text::NOTE)
        .text_color(cx.palette().foreground)
        .child(name)
        .children(rest.map(|rest| note(rest, cx)))
}

fn figure() -> Div {
    v_flex().min_w_0().gap(FIGURE_GAP)
}

/// The width this piece's part was laid out at last frame, and the probe that reads it, as `useWidth` does.
fn measured(p: &PieceCx, part: &str, window: &mut Window, cx: &mut App) -> (f32, impl IntoElement) {
    let state = window.use_keyed_state(ElementId::from(p.el(part)), cx, |_, _| 0f32);
    let width = *state.read(cx);
    let probe = canvas(
        move |bounds, _, cx| {
            let next = f32::from(bounds.size.width);
            state.update(cx, |w, cx| {
                if (*w - next).abs() > 0.5 {
                    *w = next;
                    cx.notify();
                }
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full();
    (width, probe)
}

/// `slateTime`: ms, or a string the clock reads as a moment; anything else is not a time.
fn slate_time(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64().filter(|n| n.is_finite()),
        Value::String(s) => parse_time(s.trim()),
        _ => None,
    }
}

fn parse_time(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    if let Ok(t) = DateTime::parse_from_rfc3339(s) {
        return Some(t.timestamp_millis() as f64);
    }
    for f in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M"] {
        if let Ok(t) = NaiveDateTime::parse_from_str(s, f) {
            return Local.from_local_datetime(&t).earliest().map(|t| t.timestamp_millis() as f64);
        }
    }
    // A date alone is midnight UTC, as `Date.parse` reads it.
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().and_then(|d| d.and_hms_opt(0, 0, 0)).map(|t| t.and_utc().timestamp_millis() as f64)
}

/// `slateAxisWord`: a time in ms or an ISO time by the clock, anything else as written.
fn axis_word(x: Option<&Value>) -> String {
    match x {
        None | Some(Value::Null) => String::new(),
        Some(Value::Number(n)) if n.as_f64().is_some_and(|n| n > 1e12) => time_word(n.as_f64().unwrap_or_default(), &Local),
        Some(Value::String(s)) => {
            let iso = s.len() > 10 && s.as_bytes()[10] == b'T' && NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d").is_ok();
            match parse_time(s).filter(|_| iso) {
                Some(ms) => time_word(ms, &Local),
                None => s.clone(),
            }
        }
        Some(Value::Number(n)) => look::num_text(n.as_f64().unwrap_or_default()),
        Some(other) => other.to_string(),
    }
}

/// `Number(v.toPrecision(12))`.
fn tidy(v: f64) -> f64 {
    format!("{v:.11e}").parse().unwrap_or(v)
}

/// `slateChartAxis`: four or five round steps holding every value, whichever reaches less far past the top; from 0
/// when the values sit nearer 0 than their spread, and a band of 2 percent (or 1) around a flat line.
fn chart_axis(values: &[f64]) -> Axis {
    const STEPS: [f64; 4] = [1., 2., 2.5, 5.];
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let band = if hi > lo {
        0.
    } else {
        let b = lo.abs() * 0.02;
        (if b == 0. || b.is_nan() { 1. } else { b }) / 2.
    };
    let low = if lo >= 0. && lo <= hi - lo { 0. } else { lo - band };
    let high = hi + band;
    let span = |parts: usize| {
        let whole = Axis { from: low, to: high, parts };
        let mut magnitude = 10f64.powf(((high - low) / parts as f64).log10().floor());
        if !(magnitude > 0. && magnitude.is_finite()) {
            return whole;
        }
        let mut i = 0;
        loop {
            if i == STEPS.len() {
                i = 0;
                magnitude *= 10.;
            }
            let step = STEPS[i] * magnitude;
            if !(step * parts as f64).is_finite() {
                return whole;
            }
            if step * parts as f64 >= high - low {
                let from = tidy(tidy(low / step).floor() * step);
                if from + step * parts as f64 >= high {
                    return Axis { from, to: tidy(from + step * parts as f64), parts };
                }
            }
            i += 1;
        }
    };
    let (four, five) = (span(4), span(5));
    if five.to - five.from < four.to - four.from {
        five
    } else {
        four
    }
}

/// `axisFor`: the kit's axis, but whole-number figures label only whole numbers.
fn axis_for(values: &[f64], format: Option<&Value>) -> Axis {
    let axis = chart_axis(values);
    if format.and_then(Value::as_str) != Some("integer") {
        return axis;
    }
    let from = axis.from.floor();
    let to = (from + 1.).max(axis.to.ceil());
    let span = to - from;
    let parts = [axis.parts, 4, 3, 2].into_iter().find(|n| span % *n as f64 == 0.).unwrap_or(1);
    Axis { from, to, parts }
}

/// The panel decides a chart's height: 200 px in the 400 px panel, 260 once it is widened.
fn chart_height(width: f32) -> Pixels {
    if width >= 560. {
        px(260.)
    } else {
        px(200.)
    }
}

/// The figure's words with the unit after it, and the axis's without.
fn figures(p: &PieceCx) -> (impl Fn(f64) -> String + Clone + 'static, impl Fn(f64) -> String + Clone + 'static) {
    let format = p.prop("format").and_then(Value::as_str).unwrap_or("plain").to_owned();
    let unit = p.str("unit");
    let bare = {
        let format = format.clone();
        move |v: f64| look::slate_figure(v, &format, None)
    };
    let shown = move |v: f64| match &unit {
        Some(unit) => format!("{} {unit}", look::slate_figure(v, &format, None)),
        None => look::slate_figure(v, &format, None),
    };
    (shown, bare)
}

/// The legend's name and what follows, with the unit.
fn named(p: &PieceCx) -> (String, String) {
    let (name, rest) = split_label(&p.str("label").unwrap_or_default());
    let what = [rest, p.str("unit")].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
    (name, what)
}

/// The legend's line swatch, `h-0.5 rounded-full bg-current`.
fn swatch(width: f32, ink: Hsla) -> Div {
    div().flex_none().w(px(width)).h(px(2.)).rounded_full().bg(ink)
}

fn chart(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    if p.piece.prop("series").and_then(Value::as_array).is_some_and(|s| !s.is_empty()) {
        return series_chart(p, window, cx);
    }
    let pal = cx.palette().clone();
    let label = p.str("label").unwrap_or_default();
    let rows: Vec<(Value, f64)> = p
        .rows
        .iter()
        .flatten()
        .filter_map(|row| {
            let x = if p.piece.prop("x").is_some() { row.props.get("x").cloned().unwrap_or(Value::Null) } else { Value::from(row.index) };
            look::num(row.props.get("value")).map(|v| (x, v))
        })
        .collect();
    let (shown, bare) = figures(p);
    let values: Vec<f64> = if rows.len() == 1 { vec![rows[0].1, rows[0].1] } else { rows.iter().map(|r| r.1).collect() };
    // A series that holds one value the whole window says so on the legend's line; one sample still draws its plot.
    let flat = rows.len() > 1 && rows.iter().all(|r| r.1 == rows[0].1);
    let ink = if loud(p) { Tone::Accent } else { Tone::Muted }.ink(cx);
    let (name, what) = named(p);
    let (width, probe) = measured(p, "width", window, cx);
    let legend = sized(h_flex().flex_wrap().gap_x_6().gap_y_2(), text::NOTE).text_color(pal.foreground).child(
        h_flex().gap_2().child(swatch(16., ink)).child(name).when(!what.is_empty(), |el| el.child(note(what, cx))).when(flat, |el| {
            el.child(note("steady at ", cx).flex().child(div().font_family(mono(cx)).text_color(pal.foreground).child(shown(rows[0].1))))
        }),
    );
    let body = if rows.is_empty() {
        Some(note(NOT_READ, cx).into_any_element())
    } else if flat {
        None
    } else {
        let xs: Vec<Value> = rows.iter().map(|r| r.0.clone()).collect();
        let ticks = if rows.len() == 1 {
            vec![Tick { at: 1., word: axis_word(Some(&xs[0])).into() }]
        } else {
            vec![Tick { at: 0., word: axis_word(xs.first()).into() }, Tick { at: 1., word: axis_word(xs.last()).into() }]
        };
        let axis = axis_for(&values, p.prop("format"));
        let line = ChartLine { key: "line".into(), label: label.into(), points: values.clone(), ink };
        let steps = (0..values.len()).map(|i| i as f64).collect();
        Some(
            Chart::new(p.el("chart"), steps, vec![line], ticks)
                .height(chart_height(width))
                .figure(shown)
                .axis_figure(bare)
                .step_word(move |i| axis_word(xs.get((i as usize).min(xs.len() - 1))))
                .axis(axis)
                .small()
                .every_figure()
                .tween()
                .into_any_element(),
        )
    };
    figure().relative().child(probe).child(legend).children(body).into_any_element()
}

/// Two to four series over one x: lines that cross, or with `stack` a total split into bands, the first at the bottom.
fn series_chart(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let raw = p.piece.prop("series");
    let entries = raw.and_then(Value::as_array).cloned().unwrap_or_default();
    let index: Vec<Value> = (0..entries.len()).map(Value::from).collect();
    let kept: Vec<usize> = p.slate.present(raw, &index).iter().filter_map(|v| v.as_u64().map(|k| k as usize)).collect();
    let rows = p.rows.clone().unwrap_or_default();
    let entry = |row: Option<&Row>, k: usize| -> Option<Value> {
        match row {
            Some(row) => row.props.get("series").and_then(|s| s.get(k)).cloned(),
            None => entries.get(k).cloned(),
        }
    };
    let stacked = p.bool("stack");
    let series: Vec<(String, Option<Value>)> = kept
        .iter()
        .map(|&k| {
            let e = entry(rows.first(), k);
            let label = look::str(e.as_ref().and_then(|e| e.get("label"))).unwrap_or_default();
            (label, e.and_then(|e| e.get("tone").cloned()).filter(|t| !t.is_null()))
        })
        .collect();
    let own: Vec<Vec<f64>> = kept
        .iter()
        .map(|&k| rows.iter().map(|row| look::num(entry(Some(row), k).as_ref().and_then(|e| e.get("value"))).unwrap_or(0.)).collect())
        .collect();
    let drawn: Vec<Vec<f64>> = if stacked {
        (0..own.len()).map(|k| (0..rows.len()).map(|i| own[..=k].iter().map(|line| line[i]).sum()).collect()).collect()
    } else {
        own
    };
    let accent = loud(p);
    let series_ink = [Tone::Accent.ink(cx), pal.warning, pal.status_working, pal.success];
    let mut untoned = 0;
    let inks: Vec<Hsla> = series
        .iter()
        .map(|(_, tone)| match tone {
            Some(tone) => {
                let t = Tone::of(Some(tone));
                if t == Tone::Accent && !accent { Tone::Default } else { t }.ink(cx)
            }
            None => {
                let step = untoned;
                untoned += 1;
                if accent || step > 0 {
                    series_ink[step.min(series_ink.len() - 1)]
                } else {
                    pal.foreground
                }
            }
        })
        .collect();
    let (shown, bare) = figures(p);
    let all: Vec<f64> = drawn.iter().flatten().copied().collect();
    let axis = if stacked { axis_for(&[&[0.][..], &all].concat(), p.prop("format")) } else { axis_for(&all, p.prop("format")) };
    let (name, what) = named(p);
    let (width, probe) = measured(p, "width", window, cx);
    let order: Vec<usize> = if stacked { (0..series.len()).rev().collect() } else { (0..series.len()).collect() };
    let legend =
        sized(h_flex().flex_wrap().gap_x_4().gap_y_2(), text::XS).text_color(pal.muted_foreground).children(order.iter().map(|&k| {
            let mark =
                if stacked { div().flex_none().size(px(8.)).rounded(XS_RADIUS).bg(inks[k]).opacity(0.8) } else { swatch(14., inks[k]) };
            h_flex().gap_2().child(mark).child(series[k].0.clone())
        }));
    let head = v_flex()
        .gap(px(6.))
        .child(
            sized(h_flex().flex_wrap().items_baseline().gap_x_2(), text::NOTE)
                .text_color(pal.foreground)
                .child(name)
                .when(!what.is_empty(), |el| el.child(note(what, cx))),
        )
        .child(legend);
    let body = if rows.is_empty() || series.is_empty() {
        note(NOT_READ, cx).into_any_element()
    } else {
        let xs: Vec<Value> = rows
            .iter()
            .map(
                |row| if p.piece.prop("x").is_some() { row.props.get("x").cloned().unwrap_or(Value::Null) } else { Value::from(row.index) },
            )
            .collect();
        let ticks = vec![Tick { at: 0., word: axis_word(xs.first()).into() }, Tick { at: 1., word: axis_word(xs.last()).into() }];
        let lines = series
            .iter()
            .zip(drawn)
            .zip(&inks)
            .enumerate()
            .map(|(k, (((label, _), points), ink))| ChartLine {
                key: format!("s{k}").into(),
                label: label.clone().into(),
                points,
                ink: *ink,
            })
            .collect();
        let steps = (0..rows.len()).map(|i| i as f64).collect();
        Chart::new(p.el("chart"), steps, lines, ticks)
            .height(chart_height(width))
            .figure(shown)
            .axis_figure(bare)
            .step_word(move |i| axis_word(xs.get((i as usize).min(xs.len() - 1))))
            .axis(axis)
            .small()
            .every_figure()
            .stacked(stacked)
            .fill(stacked)
            .tween()
            .into_any_element()
    };
    figure().relative().child(probe).child(head).child(body).into_any_element()
}

/// The most parts a donut names; past that, the smallest are summed as Other.
const DONUT_PARTS: usize = 5;

/// `slateShares`: biggest first, a negative or missing value as 0, past five parts the rest as one Other.
fn shares(given: Vec<(String, f64)>) -> Vec<(String, f64, bool)> {
    let mut all: Vec<(String, f64)> = given.into_iter().map(|(n, v)| (n, if v.is_finite() { v.max(0.) } else { 0. })).collect();
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    if all.len() <= DONUT_PARTS {
        return all.into_iter().map(|(n, v)| (n, v, false)).collect();
    }
    let other = all[DONUT_PARTS..].iter().map(|p| p.1).sum();
    all.truncate(DONUT_PARTS);
    all.into_iter().map(|(n, v)| (n, v, false)).chain([("Other".to_owned(), other, true)]).collect()
}

const STRENGTH: [f32; 5] = [1., 0.68, 0.46, 0.30, 0.18];

fn donut(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let label = p.str("label").unwrap_or_default();
    let parts = shares(
        p.rows
            .iter()
            .flatten()
            .map(|row| (look::str(row.props.get("name")).unwrap_or_default(), look::num(row.props.get("value")).unwrap_or(0.)))
            .collect(),
    );
    let whole: f64 = parts.iter().map(|p| p.1).sum();
    let ink = if loud(p) { Tone::Accent.ink(cx) } else { pal.foreground };
    let ink_of = |k: usize, other: bool| {
        if other {
            mix(pal.muted_foreground, 0.45, pal.background)
        } else {
            mix(ink, STRENGTH[k.min(STRENGTH.len() - 1)], pal.background)
        }
    };
    let (width, probe) = measured(p, "width", window, cx);
    let wide = width >= 520.;
    let head = caption(&label, cx);
    if whole == 0. {
        return figure().relative().child(probe).child(head).child(note(NOT_READ, cx)).into_any_element();
    }
    let format = p.prop("format");
    let ring_size = if wide { 148. } else { 120. };
    let arcs: Vec<(f64, f64, Hsla)> = {
        let mut start = 0.;
        parts
            .iter()
            .enumerate()
            .map(|(k, part)| {
                let share = part.1 / whole;
                let at = start;
                start += share;
                (at, share, ink_of(k, part.2))
            })
            .collect()
    };
    let gap = parts.len() > 1;
    let track = pal.foreground.opacity(0.06);
    let ring = canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window, _| {
            let scale = f32::from(bounds.size.width) / 120.;
            let centre = bounds.center();
            let r = 50. * scale;
            let round = std::f64::consts::TAU * 50.;
            let stroke = |from: f64, sweep: f64, ink: Hsla, window: &mut Window| {
                let options = StrokeOptions::default().with_line_width(11. * scale).with_line_cap(LineCap::Butt);
                let mut path = PathBuilder::stroke(px(11. * scale)).with_style(PathStyle::Stroke(options));
                let steps = ((sweep.abs() * 180. / std::f64::consts::PI).ceil() as usize).max(2);
                for s in 0..=steps {
                    let a = from + sweep * s as f64 / steps as f64;
                    let at = centre + point(px(r * a.cos() as f32), px(r * a.sin() as f32));
                    if s == 0 {
                        path.move_to(at);
                    } else {
                        path.line_to(at);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, ink);
                }
            };
            stroke(0., std::f64::consts::TAU, track, window);
            for (at, share, ink) in &arcs {
                // The web's gap of 1.6 along the ring; a sliver keeps half a unit.
                let drawn = (share * round - if gap { 1.6 } else { 0. }).max(0.5);
                let from = -std::f64::consts::FRAC_PI_2 + at * std::f64::consts::TAU;
                stroke(from, drawn / round * std::f64::consts::TAU, *ink, window);
            }
        },
    )
    .absolute()
    .size_full();
    let centre = v_flex()
        .absolute()
        .inset_0()
        .items_center()
        .justify_center()
        .gap(px(2.))
        .child(
            div()
                .font_family(mono(cx))
                .text_size(px(15.))
                .line_height(px(20.))
                .text_color(pal.foreground)
                .child(look::figure(Some(&Value::from(whole)), format).unwrap_or_default()),
        )
        .children(p.str("unit").map(|unit| sized(div(), META).text_color(pal.muted_foreground).child(unit)));
    let legend = v_flex().flex_1().min_w_0().gap_2().children(parts.iter().enumerate().map(|(k, part)| {
        sized(h_flex().min_w_0().items_center().gap(px(10.)), text::NOTE)
            .child(div().flex_none().size(px(10.)).rounded(XS_RADIUS).bg(ink_of(k, part.2)))
            .child(div().flex_1().min_w_0().truncate().text_color(pal.foreground).child(part.0.clone()))
            .child(
                div()
                    .font_family(mono(cx))
                    .text_size(px(12.))
                    .text_color(pal.muted_foreground)
                    .child(look::figure(Some(&Value::from(part.1)), format).unwrap_or_default()),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(36.))
                    .flex()
                    .justify_end()
                    .font_family(mono(cx))
                    .text_size(px(12.))
                    .text_color(pal.foreground)
                    .child(format!("{}%", js_round(part.1 / whole * 100.))),
            )
    }));
    let body = h_flex()
        .min_w_0()
        .items_center()
        .gap(px(if wide { 32. } else { 20. }))
        .child(div().relative().flex_none().size(px(ring_size)).child(ring).child(centre))
        .child(legend);
    figure().relative().child(probe).child(head).child(body).into_any_element()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpanState {
    Done,
    Running,
    Failed,
    Waiting,
}

/// `slateSpanState`: the one given, else waiting with no start, running with no end and done with both.
fn span_state(given: Option<&Value>, start: Option<f64>, end: Option<f64>) -> SpanState {
    match given.and_then(Value::as_str) {
        Some("done") => SpanState::Done,
        Some("running") => SpanState::Running,
        Some("failed") => SpanState::Failed,
        Some("waiting") => SpanState::Waiting,
        _ if start.is_none() => SpanState::Waiting,
        _ if end.is_none() => SpanState::Running,
        _ => SpanState::Done,
    }
}

/// `fmtElapsed`: whole seconds, then the short style's minutes and seconds.
fn elapsed(ms: f64) -> String {
    let seconds = if ms.is_finite() && ms > 0. { (ms / 1000.).floor() } else { 0. };
    if seconds < 60. {
        format!("{seconds}s")
    } else {
        look::fmt_duration(seconds * 1000.)
    }
}

const SECOND: f64 = 1000.;
const MINUTE: f64 = 60. * SECOND;
const HOUR: f64 = 60. * MINUTE;
const DAY: f64 = 24. * HOUR;
/// A step at most a third of the span, so the clock draws two to four words.
const MOST_STEPS: f64 = 3.;
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// `clockTicks`: the ticks across a span on the first step a third of it or more, each with its word in `tz`.
fn clock_ticks<Tz: TimeZone>(from: f64, span: f64, tz: &Tz) -> Vec<(f64, String)> {
    let steps = [5., 10., 15., 30.]
        .map(|n| n * SECOND)
        .into_iter()
        .chain([1., 2., 5., 10., 15., 30.].map(|n| n * MINUTE))
        .chain([1., 2., 3., 6., 12.].map(|n| n * HOUR))
        .chain([1., 2., 7., 14., 30., 90., 180., 365.].map(|n| n * DAY));
    let mut step = steps.clone().find(|s| span / s <= MOST_STEPS).unwrap_or(365. * DAY);
    while span / step > MOST_STEPS {
        step *= 2.;
    }
    let first = (from / step).ceil() * step;
    let count = (((from + span - first) / step).floor() + 1.).max(0.) as usize;
    (0..count)
        .filter_map(|i| {
            let t = first + i as f64 * step;
            let d = DateTime::from_timestamp_millis(t as i64)?.with_timezone(tz);
            let word = if step >= 365. * DAY {
                d.year().to_string()
            } else if step >= DAY {
                format!("{} {}", d.day(), MONTHS[d.month0() as usize])
            } else {
                let mut time = format!("{:02}:{:02}", d.hour(), d.minute());
                if step < MINUTE {
                    time.push_str(&format!(":{:02}", d.second()));
                }
                if span >= DAY {
                    format!("{} {time}", WEEKDAYS[d.weekday().num_days_from_monday() as usize])
                } else {
                    time
                }
            };
            Some((t, word))
        })
        .collect()
}

struct Span {
    name: String,
    group: Option<String>,
    start: Option<f64>,
    end: Option<f64>,
    state: SpanState,
}

fn timeline(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let label = p.str("label").unwrap_or_default();
    let now = slate_time(p.prop("now")).or_else(|| look::num(p.slate.resolved.get("time.now")));
    let spans: Vec<Span> = p
        .rows
        .iter()
        .flatten()
        .map(|row| {
            let start = slate_time(row.props.get("start"));
            let end = slate_time(row.props.get("end"));
            let state = span_state(row.props.get("state"), start, end);
            Span {
                name: look::str(row.props.get("name")).unwrap_or_default(),
                group: look::str(row.props.get("group")),
                start,
                end: if state == SpanState::Running { end.or(now) } else { end },
                state,
            }
        })
        .collect();
    let (width, probe) = measured(p, "width", window, cx);
    let head = caption(&label, cx);
    if spans.is_empty() {
        return figure().relative().child(probe).child(head).child(note(NOT_READ, cx)).into_any_element();
    }
    let times: Vec<f64> = spans.iter().flat_map(|s| [s.start, s.end]).flatten().collect();
    let from = times.iter().copied().fold(f64::INFINITY, f64::min);
    let span = (times.iter().copied().fold(f64::NEG_INFINITY, f64::max) - from).max(1.);
    let x = move |t: f64| ((t - from) / span) as f32;
    let ticks = if times.is_empty() { Vec::new() } else { clock_ticks(from, span, &Local) };
    let tick_at: Vec<f32> = ticks.iter().map(|(t, _)| x(*t)).collect();

    let accent = Tone::Accent.ink(cx);
    let (family, mono_family) = (cx.theme().font_family.clone(), mono(cx));
    let word_of = |s: &Span| -> (Option<(&'static str, Hsla)>, String) {
        match s.state {
            SpanState::Done => (None, s.start.zip(s.end).map(|(a, b)| elapsed(b - a)).unwrap_or_default()),
            SpanState::Running => (
                Some(("Running", accent)),
                match (s.start, now) {
                    (Some(start), Some(now)) => format!(" {}", elapsed(now - start)),
                    _ => String::new(),
                },
            ),
            SpanState::Failed => (Some(("Failed", pal.error_foreground)), String::new()),
            SpanState::Waiting => (Some(("Waiting", pal.muted_foreground)), String::new()),
        }
    };
    // The grid's auto column: as wide as its widest cell, 56 px at least.
    let right = spans
        .iter()
        .map(|s| {
            let (word, figure) = word_of(s);
            let w = word.map_or(0., |(w, _)| f32::from(text::width(w, 12., family.clone(), window)));
            w + f32::from(text::width(&figure, 12., mono_family.clone(), window))
        })
        .fold(56., f32::max);
    let name_col = if width >= 520. { 144. } else { 104. };
    let border = pal.border;
    let track = |ticks: Vec<f32>| {
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                for at in &ticks {
                    let lx = bounds.origin.x + px(at * w - 0.5);
                    let mut y = 0.;
                    while y < h {
                        let dash = Bounds::new(point(lx, bounds.origin.y + px(y)), size(px(1.), px(2f32.min(h - y))));
                        window.paint_quad(gpui_kit::fill(dash, border));
                        y += 6.;
                    }
                }
            },
        )
        .absolute()
        .size_full()
    };
    let done_bar = mix(pal.foreground, 0.42, pal.background);
    let mut groups: Vec<(Option<String>, Vec<&Span>)> = Vec::new();
    for s in &spans {
        match groups.last_mut() {
            Some((g, rows)) if *g == s.group => rows.push(s),
            _ => groups.push((s.group.clone(), vec![s])),
        }
    }
    let mut grid = v_flex().gap(px(6.));
    for (g, (group, rows)) in groups.iter().enumerate() {
        if let Some(group) = group {
            grid = grid.child(note(group.clone(), cx).when(g > 0, |el| el.pt_2()));
        }
        for s in rows {
            let bar = s.start.zip(s.end).map(|(a, b)| {
                let fill = match s.state {
                    SpanState::Running => accent,
                    SpanState::Failed => pal.error_foreground,
                    _ => done_bar,
                };
                div()
                    .absolute()
                    .top(px(4.))
                    .h(px(12.))
                    .left(relative(x(a)))
                    .w(relative(x(b) - x(a)))
                    .min_w(px(4.))
                    .rounded(XS_RADIUS)
                    .bg(fill)
                    .shadow(bevel(&pal))
            });
            let (word, figure) = word_of(s);
            let tail = sized(h_flex().flex_none().w(px(right)).justify_end(), (px(12.), px(20.)))
                .font_family(mono_family.clone())
                .whitespace_nowrap()
                .children(word.map(|(w, ink)| div().font_family(family.clone()).text_color(ink).child(w)))
                .child(div().text_color(pal.muted_foreground).child(figure));
            grid = grid.child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(sized(div(), text::NOTE).flex_none().w(px(name_col)).truncate().text_color(pal.foreground).child(s.name.clone()))
                    .child(div().relative().flex_1().min_w_0().h(px(20.)).child(track(tick_at.clone())).children(bar))
                    .child(tail),
            );
        }
    }
    let tick_row = sized(div().relative().flex_1().min_w_0().h(px(16.)), META)
        .font_family(mono_family.clone())
        .text_color(pal.muted_foreground)
        .children(ticks.iter().enumerate().map(|(i, (t, word))| {
            let at = x(*t);
            let word = div().flex_none().whitespace_nowrap().child(word.clone());
            let slot = div().absolute().top_0().left(relative(at)).w_0().flex();
            if at > 0.85 {
                slot.justify_end().child(word)
            } else if i == 0 && at < 0.08 {
                slot.child(word)
            } else {
                slot.justify_center().child(word)
            }
        }));
    grid = grid.child(h_flex().gap_3().child(div().flex_none().w(px(name_col))).child(tick_row).child(div().flex_none().w(px(right))));
    figure().relative().child(probe).child(head).child(grid).into_any_element()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Lays the values, largest first, into rows along the box's short side, each row kept as near square as it goes.
fn squarify<T: Clone>(items: &[(T, f64)], b: Rect) -> Vec<(T, f64, Rect)> {
    let mut out = Vec::new();
    let total: f64 = items.iter().map(|i| i.1).sum();
    if total <= 0. {
        return out;
    }
    let scale = b.w * b.h / total;
    let mut rest: Vec<(T, f64)> = items.to_vec();
    rest.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut free = b;
    let worst = |row: &[(T, f64)], side: f64| {
        let area: f64 = row.iter().map(|i| i.1 * scale).sum();
        row.iter()
            .map(|i| {
                let a = i.1 * scale;
                (side * side * a / (area * area)).max(area * area / (side * side * a))
            })
            .fold(f64::NEG_INFINITY, f64::max)
    };
    while !rest.is_empty() {
        let side = free.w.min(free.h);
        let mut k = 1;
        while k < rest.len() && worst(&rest[..=k], side) <= worst(&rest[..k], side) {
            k += 1;
        }
        let row: Vec<(T, f64)> = rest.drain(..k).collect();
        let area: f64 = row.iter().map(|i| i.1 * scale).sum();
        let thick = area / side;
        let mut along = 0.;
        let across = free.w >= free.h;
        for (item, value) in row {
            let len = value * scale / thick;
            let r = if across {
                Rect { x: free.x, y: free.y + along, w: thick, h: len }
            } else {
                Rect { x: free.x + along, y: free.y, w: len, h: thick }
            };
            out.push((item, value, r));
            along += len;
        }
        free = if across {
            Rect { x: free.x + thick, y: free.y, w: free.w - thick, h: free.h }
        } else {
            Rect { x: free.x, y: free.y + thick, w: free.w, h: free.h - thick }
        };
    }
    out
}

const GROUP_GAP: f64 = 4.;
const PART_GAP: f64 = 2.;

/// A tile's name and size.
type Part = (String, f64);

fn treemap(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let label = p.str("label").unwrap_or_default();
    let parts: Vec<(String, f64, String)> = p
        .rows
        .iter()
        .flatten()
        .map(|row| {
            (
                look::str(row.props.get("name")).unwrap_or_default(),
                look::num(row.props.get("value")).unwrap_or(0.).max(0.),
                look::str(row.props.get("group")).unwrap_or_default(),
            )
        })
        .collect();
    // A part of 0 has no area to lay out: squarifying it divides 0 by 0.
    let mut groups: Vec<(String, Vec<Part>)> = Vec::new();
    for (name, value, group) in parts.iter().filter(|p| p.1 > 0.) {
        match groups.iter_mut().find(|(g, _)| g == group) {
            Some((_, list)) => list.push((name.clone(), *value)),
            None => groups.push((group.clone(), vec![(name.clone(), *value)])),
        }
    }
    let mut groups: Vec<(String, Vec<Part>, f64)> = groups
        .into_iter()
        .map(|(g, list)| {
            let total = list.iter().map(|p| p.1).sum();
            (g, list, total)
        })
        .collect();
    groups.sort_by(|a, b| b.2.total_cmp(&a.2));
    let group_ink = [Tone::Accent.ink(cx), pal.warning, pal.status_working, pal.success, pal.status_input, pal.muted_foreground];
    let ground = mix(pal.card, 0.4, pal.background);
    let tint = |k: usize| mix(group_ink[k.min(group_ink.len() - 1)], 0.24, ground);
    let format = p.prop("format").cloned();
    let shown = move |v: f64| look::figure(Some(&Value::from(v)), format.as_ref()).unwrap_or_default();

    let (width, probe) = measured(p, "width", window, cx);
    let height = if width >= 520. { 300. } else { 240. };
    let laid = if width == 0. {
        Vec::new()
    } else {
        let keyed: Vec<(usize, f64)> = groups.iter().enumerate().map(|(k, g)| (k, g.2)).collect();
        squarify(&keyed, Rect { x: 0., y: 0., w: width as f64, h: height })
    };
    let mono_family = mono(cx);
    let mut tiles = Vec::new();
    for (k, _, g) in &laid {
        let (group, list, _) = &groups[*k];
        let inner = Rect { x: g.x + GROUP_GAP / 2., y: g.y + GROUP_GAP / 2., w: (g.w - GROUP_GAP).max(0.), h: (g.h - GROUP_GAP).max(0.) };
        for (name, value, r) in squarify(list, inner) {
            let (w, h) = ((r.w - PART_GAP).max(0.), (r.h - PART_GAP).max(0.));
            let named = w >= 44. && h >= 20.;
            let sized_tile = w >= 56. && h >= 38.;
            let title = format!("{group} {name}: {}", shown(value));
            let id = ElementId::NamedInteger(p.el("tile"), tiles.len() as u64);
            tiles.push(
                v_flex()
                    .id(id)
                    .absolute()
                    .left(px((r.x + PART_GAP / 2.) as f32))
                    .top(px((r.y + PART_GAP / 2.) as f32))
                    .w(px(w as f32))
                    .h(px(h as f32))
                    .gap(px(2.))
                    .overflow_hidden()
                    .rounded(XS_RADIUS)
                    .bg(tint(*k))
                    .shadow(bevel(&pal))
                    .when(named, |el| el.px_2().py(px(6.)))
                    .when(named, |el| el.child(sized(div(), text::XS).truncate().text_color(pal.foreground).child(name.clone())))
                    .when(sized_tile, |el| {
                        el.child(
                            sized(div(), META)
                                .truncate()
                                .font_family(mono_family.clone())
                                .text_color(pal.muted_foreground)
                                .child(shown(value)),
                        )
                    })
                    .tooltip(tip(title)),
            );
        }
    }
    let head = caption(&label, cx);
    let legend = (groups.len() >= 2).then(|| {
        sized(h_flex().flex_wrap().gap_x_4().gap_y(px(6.)), text::XS).text_color(pal.muted_foreground).children(
            groups.iter().enumerate().map(|(k, (name, _, total))| {
                h_flex()
                    .items_center()
                    .gap(px(6.))
                    .child(div().flex_none().size(px(10.)).rounded(XS_RADIUS).bg(tint(k)))
                    .child(div().text_color(pal.foreground).child(name.clone()))
                    .child(div().font_family(mono_family.clone()).child(shown(*total)))
            }),
        )
    });
    let plot = div().relative().w_full().h(px(if parts.is_empty() { 0. } else { height as f32 })).child(probe).children(tiles);
    figure().child(head).when(parts.is_empty(), |el| el.child(note(NOT_READ, cx))).child(plot).children(legend).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squarify_lays_rows_as_near_square_as_they_go() {
        let laid =
            squarify(&[("a", 6.), ("b", 6.), ("c", 4.), ("d", 3.), ("e", 2.), ("f", 2.), ("g", 1.)], Rect { x: 0., y: 0., w: 6., h: 4. });
        let r = |name: &str| laid.iter().find(|l| l.0 == name).unwrap().2;
        // The classic example: the two sixes stand as a column of two 3 by 2 boxes on the left.
        assert_eq!(r("a"), Rect { x: 0., y: 0., w: 3., h: 2. });
        assert_eq!(r("b"), Rect { x: 0., y: 2., w: 3., h: 2. });
        let area: f64 = laid.iter().map(|l| l.2.w * l.2.h).sum();
        assert!((area - 24.).abs() < 1e-9);
        assert!(laid.iter().all(|l| l.2.x >= 0. && l.2.y >= 0. && l.2.x + l.2.w <= 6. + 1e-9 && l.2.y + l.2.h <= 4. + 1e-9));
        assert!(squarify::<&str>(&[], Rect { x: 0., y: 0., w: 1., h: 1. }).is_empty());
    }

    #[test]
    fn a_donut_names_five_parts_and_sums_the_rest_as_other() {
        let given = [("India", 420.), ("US", 310.), ("Germany", 120.), ("Brazil", 80.), ("Japan", 60.), ("Kenya", 30.), ("Chile", -5.)];
        let parts = shares(given.iter().map(|(n, v)| (n.to_string(), *v)).collect());
        let names: Vec<&str> = parts.iter().map(|p| p.0.as_str()).collect();
        assert_eq!(names, ["India", "US", "Germany", "Brazil", "Japan", "Other"]);
        assert_eq!((parts[5].1, parts[5].2), (30., true));
        assert_eq!(shares(vec![("a".into(), 1.), ("b".into(), 2.)]).iter().map(|p| p.0.as_str()).collect::<Vec<_>>(), ["b", "a"]);
    }

    #[test]
    fn timeline_ticks_step_a_third_of_the_span_or_more() {
        let from = parse_time("2026-10-18T10:00:00Z").unwrap();
        let ticks = clock_ticks(from, 20. * MINUTE, &chrono::Utc);
        assert_eq!(ticks.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(), ["10:00", "10:10", "10:20"]);
        let seconds = clock_ticks(from + 3. * SECOND, 40. * SECOND, &chrono::Utc);
        assert_eq!(seconds.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(), ["10:00:15", "10:00:30"]);
        let days = clock_ticks(from, 9. * DAY, &chrono::Utc);
        // Seven-day steps from the epoch fall on Thursdays.
        assert_eq!(days.first().map(|t| t.1.as_str()), Some("22 Oct"));
        // A span from 1970 still ends in a handful of ticks, not hundreds of millions.
        assert!(clock_ticks(0., from, &chrono::Utc).len() <= 4);
    }

    #[test]
    fn the_axis_reads_round_steps_as_the_web_does() {
        assert_eq!(chart_axis(&[2410., 2415., 2408., 2421., 2430., 2426.]), Axis { from: 2405., to: 2430., parts: 5 });
        assert_eq!(axis_for(&[0., 1.], Some(&Value::from("integer"))), Axis { from: 0., to: 1., parts: 1 });
        assert_eq!(axis_word(Some(&Value::from("idle"))), "idle");
        assert_eq!(axis_word(Some(&Value::from(3))), "3");
    }
}
