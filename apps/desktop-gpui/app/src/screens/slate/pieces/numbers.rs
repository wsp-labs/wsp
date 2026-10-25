// SPDX-License-Identifier: AGPL-3.0-only
//! The number pieces, after `number.tsx`, `meter.tsx`, `ring.tsx`, `sparkline.tsx` and `bars.tsx`: a stat cell's figure
//! with its note, a meter's 56 by 4 track, a 64 by 16 line, and categories compared on a card of tracks.

use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{div, prelude::*, px, AnyElement, App, FontWeight, Hsla, Pixels, SharedString, Window};
use serde_json::Value;

use super::layout::{self, Place, INSET, ROW_FLOOR};
use super::look::{self, Tone};
use super::riders::{is_stat_cell, riders_of};
use super::PieceCx;
use crate::screens::usage::words::js_round;
use crate::ui::controls::bevel;
use crate::ui::sparkline::Sparkline;
use crate::ui::{motion, tip, ActivePalette};

/// A figure's roll and a meter's fill, `duration-500` and `duration-200`.
const ROLL: Duration = Duration::from_millis(500);
const FILL: Duration = Duration::from_millis(200);
/// `h-1 w-14`, a meter's and a bar's track.
const TRACK: (Pixels, Pixels) = (px(56.), px(4.));

pub fn draw(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match p.piece.kind.as_str() {
        "number" => Some(number(p, window, cx)),
        "meter" | "ring" => Some(meter(p, window, cx)),
        "sparkline" => Some(sparkline(p, cx)),
        "bars" => Some(bars(p, cx)),
        "stats" => Some(stats(p, window, cx)),
        _ => None,
    }
}

/// A figure written as text, value="90", read as the number it spells (`slateFigureValue`).
fn figure_value(v: Option<&Value>) -> Option<Value> {
    match v {
        Some(Value::String(s)) => s.trim().parse::<f64>().ok().map(Value::from).or_else(|| Some(Value::String(s.clone()))),
        other => other.filter(|v| !v.is_null()).cloned(),
    }
}

/// `Ring` in `tile.tsx`: 20 px across beside a tile's label, a 12 percent track under its share's arc.
fn ring(share: f64, ink: Hsla) -> gpui_kit::Div {
    let percent = (share.clamp(0., 1.) * 100.).round() as u32;
    let outer = px(20. * 22.5 / 24.);
    div()
        .relative()
        .flex_none()
        .size(px(20.))
        .child(div().absolute().top((px(20.) - outer) / 2.).left((px(20.) - outer) / 2.).size(outer).rounded_full().border(px(2.5)).border_color(ink.opacity(0.12)))
        .when(percent > 0, |el| el.child(gpui_kit::svg().absolute().inset_0().size(px(20.)).path(crate::ui::icons::ring_arc(percent)).text_color(ink)))
}

/// How a stat row stands at its width: every tile across, two a row (a third spanning), or one line a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tiles {
    Across,
    Pairs,
    Lines,
}

/// Three tiles under 480 px two a row with the third spanning, where set; one line a tile otherwise. Held for the
/// owner's ruling between the two (#2439).
#[derive(Clone, Copy, Default)]
pub struct ThreeAsPairs(pub bool);
impl gpui_kit::Global for ThreeAsPairs {}

/// The widths from which three or four tiles stand across, `stats.tsx`'s `ACROSS` steps: 30 to 46 rem.
const ACROSS_FROM: [f32; 5] = [480., 544., 608., 672., 736.];
/// A tile of the narrow panel's 366 px card, two a row, and the figure's sizes with their lines, largest first.
const NARROW_TILE: f32 = 183.;
const SIZES: [(f32, f32); 3] = [(26., 32.), (20., 28.), (16., 24.)];
/// A ring's room beside its figure, and a trend's at the figure row's right.
const RING_ROOM: f32 = 30.;
const TREND_ROOM: f32 = 72.;

/// The figure size of a row, the largest whose widest figure row fits a tile two a row, from what each figure row
/// needs at a size; and the width from which all stand across, none for two or for a row no step holds.
pub fn stats_fit(count: usize, need: impl Fn(f32) -> f32) -> ((f32, f32), Option<f32>) {
    // Two tiles pad 16 px a side, more 12.
    let pad = if count == 2 { 2. * f32::from(INSET) } else { 24. };
    let size = SIZES.into_iter().find(|(s, _)| need(*s) + pad <= NARROW_TILE).unwrap_or(SIZES[2]);
    let room = count as f32 * (need(size.0) + pad) + 2.;
    let least = if count == 4 { 544. } else { 480. };
    (size, (count > 2).then(|| ACROSS_FROM.into_iter().find(|w| *w >= least && *w >= room)).flatten())
}

/// The ruling on a stat row by its width: two tiles always two a row; three and four across from `across`, and under
/// it four two a row and three as lines, or two a row with the third spanning where `pairs`.
pub fn tiles_at(count: usize, width: Option<f32>, across: Option<f32>, pairs: bool) -> Tiles {
    let wide = count <= 2 || width.zip(across).is_some_and(|(w, a)| w >= a);
    match () {
        _ if wide => Tiles::Across,
        _ if count == 3 && !pairs => Tiles::Lines,
        _ => Tiles::Pairs,
    }
}

/// One stat as it reads off the slate.
struct Stat {
    label: Option<String>,
    shown: Option<String>,
    unit: Option<String>,
    window: Option<String>,
    share: Option<f64>,
    trend: Vec<f64>,
    tone: Tone,
}

fn stat_of(p: &PieceCx, t: &Value) -> Stat {
    let get = |k: &str| t.get(k).filter(|v| !v.is_null());
    let value = figure_value(get("value"));
    let n = value.as_ref().and_then(Value::as_f64);
    let max = get("max").and_then(Value::as_f64).filter(|m| *m > 0.);
    let format = get("format").and_then(Value::as_str).unwrap_or(if max.is_some() { "fraction" } else { "plain" });
    let plain = |x: f64| look::figure(Some(&Value::from(x)), None).unwrap_or_default();
    let shown = match (n, format, max) {
        (Some(v), "fraction", Some(m)) => Some(format!("{}/{}", plain(v), plain(m))),
        (Some(v), "percent", Some(m)) => Some(format!("{}%", js_round(v / m * 100.))),
        _ => look::figure(value.as_ref(), Some(&Value::from(format))),
    };
    let trend = match get("trend") {
        Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_f64().filter(|n| n.is_finite())).collect(),
        _ => Vec::new(),
    };
    Stat {
        label: look::str(get("label")),
        shown,
        unit: look::str(get("unit")),
        window: look::str(get("window")),
        share: n.zip(max).map(|(v, m)| v / m),
        trend,
        tone: layout::tone_of(get("tone"), p.slate, p.id()),
    }
}

/// `stats.tsx` and `tile.tsx`: two to four figures as equal tiles in one card, a hairline between them. A tile's label
/// on one line, the figure mono rolling to a new value with its ring beside it where it has a max or a trend at its
/// right, its window under it on one line; a cut label or window says itself whole on hover. Under 480 px of the row
/// two a row, or three one line a tile: the label over its window at the left, the figure and its ring at the right.
fn stats(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let resolved = p.prop("stats").and_then(Value::as_array).cloned().unwrap_or_default();
    let tiles: Vec<Stat> = p.slate.present(p.piece.prop("stats"), &resolved).iter().take(4).map(|t| stat_of(p, t)).collect();
    let (mono_font, sans) = (mono(cx), cx.theme().font_family.clone());
    let need = |size: f32| {
        tiles
            .iter()
            .map(|t| {
                let figure = t.shown.as_deref().map_or(0., |s| f32::from(crate::ui::text::width(s, size, mono_font.clone(), window)));
                let unit = t.unit.as_deref().map_or(0., |u| f32::from(crate::ui::text::width(u, 12., sans.clone(), window)) + 4.);
                let beside = if t.share.is_some() { RING_ROOM } else if t.trend.len() > 1 { TREND_ROOM } else { 0. };
                figure + unit + beside
            })
            .fold(0., f32::max)
    };
    let (size, across) = stats_fit(tiles.len(), need);
    let (width, probe) = layout::measured(p.el("stats-width"), window, cx);
    let pairs = cx.try_global::<ThreeAsPairs>().is_some_and(|t| t.0);
    let arrangement = tiles_at(tiles.len(), width.map(f32::from), across, pairs);
    let pad = if arrangement == Tiles::Lines || tiles.len() <= 2 { INSET } else { px(12.) };
    let rule = pal.border.opacity(0.5);
    let mut cells: Vec<AnyElement> = tiles
        .iter()
        .enumerate()
        .map(|(ix, t)| tile(p, ix, t, size, pad, arrangement == Tiles::Lines, window, cx))
        .collect();
    let body = match arrangement {
        Tiles::Across => h_flex()
            .w_full()
            .min_w_0()
            .items_stretch()
            .children(cells.into_iter().enumerate().map(|(ix, c)| div().flex_1().min_w_0().when(ix > 0, |d| d.border_l_1().border_color(rule)).child(c))),
        Tiles::Lines => v_flex()
            .w_full()
            .min_w_0()
            .children(cells.into_iter().enumerate().map(|(ix, c)| div().min_w_0().when(ix > 0, |d| d.border_t_1().border_color(rule)).child(c))),
        Tiles::Pairs => {
            let mut rows = Vec::new();
            while !cells.is_empty() {
                let row: Vec<AnyElement> = cells.drain(..cells.len().min(2)).collect();
                rows.push(row);
            }
            v_flex().w_full().min_w_0().children(rows.into_iter().enumerate().map(|(r, row)| {
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_stretch()
                    .when(r > 0, |d| d.border_t_1().border_color(rule))
                    .children(row.into_iter().enumerate().map(|(ix, c)| div().flex_1().min_w_0().when(ix > 0, |d| d.border_l_1().border_color(rule)).child(c)))
            }))
        }
    };
    let strip = div().relative().w_full().min_w_0().debug_selector(|| "slate-stats".into()).child(body).child(probe);
    if layout::place_of(p.slate, p.id()) == Place::Inside {
        strip.into_any_element()
    } else {
        layout::surface(p.el("stats"), strip)
    }
}

/// One stat's tile: stacked, or as one line where `line`.
#[allow(clippy::too_many_arguments)]
fn tile(p: &PieceCx, ix: usize, t: &Stat, (size, line_h): (f32, f32), pad: Pixels, line: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let ink = if t.tone == Tone::Default { pal.foreground.opacity(0.55) } else { t.tone.ink(cx) };
    let figure_ink = if t.share.is_none() && t.tone != Tone::Default { t.tone.ink(cx) } else { pal.foreground };
    let figure = h_flex()
        .debug_selector(|| "slate-stat-figure".into())
        .flex_none()
        .items_baseline()
        .gap_x_1()
        .font_family(mono(cx))
        .text_size(px(size))
        .line_height(px(line_h))
        .font_weight(FontWeight::MEDIUM)
        .text_color(figure_ink);
    let figure = match &t.shown {
        None => figure.child(note("Not read yet", cx).self_center().font_family(cx.theme().font_family.clone()).font_weight(FontWeight::NORMAL)),
        Some(shown) => figure.child(digit_roll(p.el(&format!("stat-{ix}")), shown, px(line_h), window, cx)).children(t.unit.clone().map(|unit| {
            div().font_family(cx.theme().font_family.clone()).font_weight(FontWeight::NORMAL).text_size(px(12.)).line_height(px(16.)).text_color(pal.muted_foreground).child(unit)
        })),
    };
    let trend = (t.share.is_none() && t.trend.len() > 1).then(|| {
        div().flex_none().w(px(64.)).h(px(16.)).child(Sparkline::new(line_points(&t.trend), ink).bare().height(px(16.)))
    });
    let figure_row = h_flex()
        .min_h(px(32.))
        .min_w_0()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .child(h_flex().flex_none().items_center().gap(px(10.)).child(figure).children(t.share.map(|s| ring(s, ink).debug_selector(|| "slate-stat-ring".into()))))
        .children(trend);
    let label = div()
        .id(p.el(&format!("stat-{ix}-label")))
        .debug_selector(|| "slate-stat-label".into())
        .min_h(px(20.))
        .min_w_0()
        .truncate()
        .text_size(px(13.))
        .line_height(px(20.))
        .text_color(pal.muted_foreground)
        .children(t.label.clone())
        .when_some(t.label.clone(), |d, l| d.tooltip(tip(l)));
    let under = t.window.clone().map(|w| {
        div()
            .id(p.el(&format!("stat-{ix}-window")))
            .debug_selector(|| "slate-stat-window".into())
            .min_w_0()
            .truncate()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(pal.muted_foreground)
            .child(w.clone())
            .tooltip(tip(w))
    });
    if line {
        return h_flex()
            .min_w_0()
            .items_center()
            .gap(px(16.))
            .px(pad)
            .pt(px(16.))
            .pb(px(14.))
            .child(v_flex().flex_1().min_w_0().gap(px(2.)).child(label).children(under))
            .child(figure_row.flex_none())
            .into_any_element();
    }
    v_flex().min_w_0().gap(px(6.)).px(pad).pt(px(16.)).pb(px(14.)).child(label).child(figure_row).children(under).into_any_element()
}

fn mono(cx: &App) -> SharedString {
    cx.theme().mono_font_family.clone()
}

/// `NOTE`: 12 px muted on a 16 px line.
fn note(text: impl Into<SharedString>, cx: &App) -> gpui_kit::Div {
    div().text_size(px(12.)).line_height(px(16.)).text_color(cx.palette().muted_foreground).child(text.into())
}

/// The tone a piece's value names, as `toneOf` reads it.
fn tone(p: &PieceCx, v: Option<&Value>) -> Tone {
    layout::tone_of(v, p.slate, p.id())
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn number(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let shown = look::figure(p.prop("value"), p.prop("format"));
    let unit = p.str("unit");
    let note_text = p.str("note");
    let ink = tone(p, p.prop("tone")).ink(cx);
    let figure = h_flex()
        .debug_selector(|| "slate-figure".into())
        .min_h(px(32.))
        .min_w_0()
        .flex_wrap()
        .items_baseline()
        .gap_x_1()
        .font_family(mono(cx))
        .text_size(px(26.))
        .line_height(px(32.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(ink);
    let figure = match shown {
        None => {
            figure.child(note("Not read yet", cx).self_center().font_family(cx.theme().font_family.clone()).font_weight(FontWeight::NORMAL))
        }
        Some(shown) => figure.child(digit_roll(p.el("figure"), &shown, px(32.), window, cx)).children(unit.map(|unit| {
            div()
                .font_family(cx.theme().font_family.clone())
                .font_weight(FontWeight::NORMAL)
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(pal.muted_foreground)
                .child(unit)
        })),
    };
    let slate = p.slate;
    let riders = riders_of(slate, p.id());
    // A rider whose when does not hold says nothing in the cell, as it would say nothing on its own.
    let state = riders.state.filter(|s| slate.shown(s));
    let word = state.as_ref().and_then(|s| look::str(layout::read(slate, s, "value").as_ref())).filter(|w| !w.is_empty());
    let state_tone = state.as_ref().map_or(Tone::Default, |s| layout::tone_of(layout::read(slate, s, "tone").as_ref(), slate, s));
    let parts: Vec<String> = riders
        .texts
        .iter()
        .filter(|t| slate.shown(t))
        .filter_map(|t| look::str(layout::read(slate, t, "value").as_ref()))
        .chain(note_text)
        .filter(|part| !part.is_empty())
        .collect();
    let notes = (!parts.is_empty() || word.is_some()).then(|| {
        h_flex()
            .flex_wrap()
            .gap_x_3()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(pal.muted_foreground)
            .children(word.map(|w| {
                let quiet = matches!(state_tone, Tone::Default | Tone::Muted);
                div()
                    .debug_selector(|| "slate-number-status".into())
                    .text_color(if quiet { pal.foreground } else { state_tone.ink(cx) })
                    .when(!quiet, |el| el.font_weight(FontWeight::MEDIUM))
                    .child(capitalised(&w))
            }))
            .children(parts.into_iter().map(|part| div().child(part)))
    });
    // As a row of a card, or the one thing in a row of one, the cell keeps the stat strip's own padding.
    let parent = slate.doc().and_then(|d| d.parent_id(p.id()));
    let own = layout::place_of(slate, p.id()) == Place::Row
        || parent.is_some_and(|parent| is_stat_cell(slate, parent) && layout::place_of(slate, parent) == Place::Row);
    v_flex()
        .min_w_0()
        .gap(px(6.))
        .when(own, |el| el.px(INSET).pt(px(16.)).pb(px(14.)))
        .child(div().text_size(px(13.)).line_height(px(20.)).text_color(pal.muted_foreground).children(p.str("label")))
        .child(figure)
        .children(notes)
        .into_any_element()
}

/// `DigitRoll`: the figure as plain text at rest; while a new value lands each changed digit is a column of 0 to 9
/// moved over its own glyph, keyed from the right so a figure that gains a digit keeps its ones in place.
fn digit_roll(id: SharedString, value: &str, line: Pixels, window: &mut Window, cx: &mut App) -> AnyElement {
    let count = value.chars().count();
    h_flex()
        .flex_none()
        .whitespace_nowrap()
        .children(value.chars().enumerate().map(|(i, c)| {
            let Some(digit) = c.to_digit(10) else { return div().child(c.to_string()).into_any_element() };
            let target = -line * digit as f32;
            let at =
                motion::ease_px_by(SharedString::from(format!("{id}-{}", count - i)), "digit", target, motion::snappy(ROLL), window, cx);
            if at == target {
                return div().child(c.to_string()).into_any_element();
            }
            div()
                .relative()
                .debug_selector(|| "digit-strip".into())
                .h(line)
                .overflow_hidden()
                .child(div().text_color(gpui_kit::transparent_black()).child(c.to_string()))
                .child(
                    v_flex()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(at)
                        .items_center()
                        .children((0..10).map(|d| div().h(line).child(d.to_string()))),
                )
                .into_any_element()
        }))
        .into_any_element()
}

/// The figure right of a meter's label, in the word its format names.
fn meter_figure(value: f64, max: f64, format: &str) -> Option<String> {
    (format != "none").then(|| look::slate_figure(value, if format == "value" { "plain" } else { format }, Some(max)))
}

/// A value's share of `max`, 0 to 1.
fn share(value: Option<f64>, max: f64) -> f64 {
    match value {
        Some(v) if max > 0. => (v / max).clamp(0., 1.),
        _ => 0.,
    }
}

/// A share as a track's fill, to the tenth of a percent the web's width keeps.
fn fill_width(share: f64) -> Pixels {
    TRACK.0 * (js_round(share * 1000.) / 1000.) as f32
}

fn track(fill: impl IntoElement, cx: &App) -> gpui_kit::Div {
    div().flex_none().w(TRACK.0).h(TRACK.1).rounded_full().overflow_hidden().bg(cx.palette().foreground.opacity(0.1)).child(fill)
}

fn meter(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let value = p.num("value");
    let max = p.num("max").unwrap_or(100.);
    let note_text = p.str("note");
    let format = p.prop("format").and_then(Value::as_str).unwrap_or("percent").to_owned();
    let shown = value.filter(|_| max > 0.).and_then(|v| meter_figure(v, max, &format));
    let width = motion::ease_px_by(p.el("fill"), "width", fill_width(share(value, max)), motion::snappy(FILL), window, cx);
    let fill: Hsla = motion::ease_ink_by(p.el("fill"), "fill", tone(p, p.prop("tone")).fill(cx), motion::snappy(FILL), window, cx);
    let quiet = match value {
        None => Some("Not read yet".to_owned()),
        Some(_) => note_text,
    };
    h_flex()
        .min_w_0()
        .justify_between()
        .gap_4()
        .child(
            v_flex()
                .min_w_0()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(pal.foreground)
                        .children(p.str("label")),
                )
                .children(quiet.map(|q| div().text_size(px(13.)).line_height(px(18.85)).text_color(pal.muted_foreground).child(q))),
        )
        .child(
            h_flex()
                .flex_none()
                .gap_2()
                .child(track(div().debug_selector(|| "slate-fill".into()).h_full().w(width).rounded_full().bg(fill), cx))
                .children(
                    shown.map(|s| div().font_family(mono(cx)).text_size(px(12.)).line_height(px(16.)).text_color(pal.foreground).child(s)),
                ),
        )
        .into_any_element()
}

fn sparkline(p: &PieceCx, cx: &App) -> AnyElement {
    let pal = cx.palette().clone();
    let values: Vec<f64> = match p.prop("values") {
        Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_f64().filter(|n| n.is_finite())).collect(),
        _ => Vec::new(),
    };
    // The line takes the meter's ink, so a chart stays the slate's one coloured thing; a tone the agent named holds.
    let ink = match p.prop("tone") {
        None => pal.foreground.opacity(0.55),
        Some(v) => tone(p, Some(v)).ink(cx),
    };
    let row = h_flex().min_w_0().gap_3().child(
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(pal.foreground)
            .child(p.str("label").unwrap_or_default()),
    );
    let Some(last) = values.last().copied() else { return row.child(note("Not read yet", cx)).into_any_element() };
    row.child(
        h_flex()
            .flex_none()
            .gap_3()
            .when(values.len() > 1, |el| {
                el.child(
                    div()
                        .debug_selector(|| "slate-sparkline".into())
                        .w(px(64.))
                        .h(px(16.))
                        .child(Sparkline::new(line_points(&values), ink).bare().height(px(16.))),
                )
            })
            .child(
                div().font_family(mono(cx)).text_size(px(12.)).line_height(px(20.)).text_color(pal.foreground).child(look::en_us(last, 3)),
            ),
    )
    .into_any_element()
}

/// The web's 64 by 16 polyline over its own range, a pixel kept above and below, as 0 to 1 across and up.
fn line_points(values: &[f64]) -> Vec<(f32, f32)> {
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = if hi - lo == 0. { 1. } else { hi - lo };
    let last = (values.len() - 1).max(1) as f64;
    values.iter().enumerate().map(|(i, v)| ((i as f64 / last) as f32, ((1. + (v - lo) / span * 14.) / 16.) as f32)).collect()
}

fn bars(p: &PieceCx, cx: &App) -> AnyElement {
    let pal = cx.palette().clone();
    let rows: Vec<(String, Option<f64>, Option<Value>)> = p
        .rows
        .iter()
        .flatten()
        .map(|r| (look::str(r.props.get("name")).unwrap_or_default(), look::num(r.props.get("value")), r.props.get("tone").cloned()))
        .collect();
    let max = p.num("max").unwrap_or_else(|| rows.iter().filter_map(|r| r.1).fold(0., f64::max));
    let format = p.prop("format").cloned().unwrap_or(Value::from("value"));
    // In the switch the segment names the list and the switch's card holds it, so the list is its rows alone.
    let switched = layout::in_bar_switch(p.slate, p.id());
    let inside = layout::place_of(p.slate, p.id()) == Place::Inside;
    let inset = if inside && !switched { px(0.) } else { INSET };
    let row_box = || h_flex().min_h(ROW_FLOOR).min_w_0().py_3().px(inset);
    let mut items: Vec<AnyElement> = Vec::new();
    if rows.is_empty() {
        items.push(
            row_box().text_size(px(13.)).line_height(px(20.)).text_color(pal.muted_foreground).child("Nothing here").into_any_element(),
        );
    }
    for (name, value, row_tone) in rows {
        let share = share(value, max);
        let fill = match &row_tone {
            None => pal.foreground.opacity(0.55),
            Some(t) => tone(p, Some(t)).fill(cx),
        };
        let figure = match format.as_str() {
            Some("none") => None,
            Some("percent") => Some(format!("{}%", look::num_text(js_round(share * 100.)))),
            _ => value.map(|v| look::slate_figure(v, "value", None)),
        };
        items.push(
            row_box()
                .debug_selector(|| "slate-bar".into())
                .gap_3()
                .child(div().flex_1().min_w_0().text_size(px(14.)).line_height(px(20.)).text_color(pal.foreground).child(name))
                .child(
                    h_flex()
                        .flex_none()
                        .gap_2()
                        .child(track(div().h_full().w(fill_width(share)).rounded_full().bg(fill).shadow(bevel(&pal)), cx))
                        .child(
                            div()
                                .min_w(px(48.))
                                .text_right()
                                .font_family(mono(cx))
                                .text_size(px(12.))
                                .line_height(px(20.))
                                .text_color(pal.foreground)
                                .children(figure),
                        ),
                )
                .into_any_element(),
        );
    }
    let list = if inside || switched {
        let rule = pal.border.opacity(0.5);
        v_flex()
            .min_w_0()
            .children(items.into_iter().enumerate().map(|(ix, el)| div().when(ix > 0, |d| d.border_t_1().border_color(rule)).child(el)))
            .into_any_element()
    } else {
        layout::card(p.el("bars"), items.into_iter().map(|el| (true, el)).collect()).unwrap_or_else(|| div().into_any_element())
    };
    v_flex()
        .min_w_0()
        .gap(px(10.))
        .when(!switched, |el| el.child(layout::head(p.str("label").unwrap_or_default(), cx)))
        .child(list)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use gpui_kit::{div, prelude::*, px, size, Context, TestAppContext, Window};

    use super::{digit_roll, line_points, stats_fit, tiles_at, Tiles};
    use crate::ui::{self, Side};

    struct Figure(Rc<RefCell<&'static str>>);

    impl Render for Figure {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let value = *self.0.borrow();
            div().child(digit_roll("figure".into(), value, px(32.), window, cx))
        }
    }

    #[gpui_kit::test]
    fn a_new_figure_rolls_its_changed_digits_then_rests_as_text(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let value = Rc::new(RefCell::new("$4.64"));
        let (_, cx) = cx.add_window_view(|_, _| Figure(value.clone()));
        cx.simulate_resize(size(px(300.), px(120.)));
        cx.run_until_parked();
        assert!(cx.debug_bounds("digit-strip").is_none(), "a figure drawn first stands still");
        *value.borrow_mut() = "$4.71";
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(cx.debug_bounds("digit-strip").is_some(), "a changed digit rolls");
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(cx.debug_bounds("digit-strip").is_some(), "still rolling halfway through its 500 ms");
        cx.executor().advance_clock(Duration::from_millis(300));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(cx.debug_bounds("digit-strip").is_none(), "after 500 ms it rests as plain text");
    }

    #[test]
    fn a_line_keeps_a_pixel_above_and_below_its_range() {
        assert_eq!(line_points(&[2., 4., 3.]), vec![(0., 1. / 16.), (0.5, 15. / 16.), (1., 8. / 16.)]);
    }

    #[test]
    fn a_stat_row_takes_its_tiles_a_row_by_its_own_width() {
        assert_eq!(tiles_at(2, Some(366.), None, false), Tiles::Across, "two tiles stand two a row however narrow");
        assert_eq!(tiles_at(3, Some(366.), Some(480.), false), Tiles::Lines, "three under 480 px are one line each");
        assert_eq!(tiles_at(3, Some(366.), Some(480.), true), Tiles::Pairs, "or two a row with the third spanning");
        assert_eq!(tiles_at(3, Some(479.), Some(480.), false), Tiles::Lines);
        assert_eq!(tiles_at(3, Some(480.), Some(480.), false), Tiles::Across);
        assert_eq!(tiles_at(4, Some(543.), Some(544.), false), Tiles::Pairs, "four under 544 px are two a row");
        assert_eq!(tiles_at(4, Some(608.), Some(544.), false), Tiles::Across);
        assert_eq!(tiles_at(4, Some(1000.), None, false), Tiles::Pairs, "a row no width holds across stays two a row");
        assert_eq!(tiles_at(3, None, Some(480.), false), Tiles::Lines, "a row not measured yet draws narrow");
    }

    #[test]
    fn a_stat_row_goes_across_from_the_least_width_its_widest_tile_needs() {
        // 182/200 and its ring: 139 px at 26 px, 163 with a tile's padding, so 26 px and four across from 672.
        assert_eq!(stats_fit(4, |_| 139.), ((26., 32.), Some(672.)));
        assert_eq!(stats_fit(3, |_| 139.), ((26., 32.), Some(544.)));
        assert_eq!(stats_fit(3, |_| 60.), ((26., 32.), Some(480.)), "never under 480 px");
        assert_eq!(stats_fit(4, |_| 60.), ((26., 32.), Some(544.)), "four never under 544 px");
        // 8,412/9,650 and its ring fits a tile two a row only at 16 px.
        assert_eq!(stats_fit(3, |s| 11. * 0.6 * s + 30.), ((16., 24.), Some(544.)));
        assert_eq!(stats_fit(2, |_| 139.).1, None, "two have no width to go across from");
        assert_eq!(stats_fit(4, |_| 300.).1, None);
    }
}
