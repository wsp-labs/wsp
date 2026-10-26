// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/chart.ts at 65f6b89c7.
use std::cmp::Ordering;

use serde_json::Value;
use wsp_wire::text::json_stringify;
use wsp_words::date_parse::date_parse;
use wsp_words::js::{js_trim, utf16};
use wsp_words::js_date::DateZone;
use wsp_words::words::turn::fmt_clock;

const STEPS: [f64; 4] = [1.0, 2.0, 2.5, 5.0];

pub const SLATE_SPAN_STATES: &[&str] = &["done", "running", "failed", "waiting"];

pub const SLATE_DONUT_PARTS: usize = 5;

/// The earliest a slate time is taken to mean: a number before it is most likely seconds read as milliseconds.
pub const SLATE_TIME_FLOOR: f64 = 946_684_800_000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlateChartAxis {
    pub from: f64,
    pub to: f64,
    pub parts: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlateShare {
    pub name: String,
    pub value: f64,
    pub other: bool,
}

fn tidy(v: f64) -> f64 {
    if !v.is_finite() {
        return v;
    }
    format!("{v:.11e}").parse().unwrap_or(v)
}

/// The y axis in four or five round steps holding every value, whichever reaches less far past the top: from 0 when
/// the values sit nearer 0 than their spread, and a band of 2 percent of the value (or 1) around a flat line.
pub fn slate_chart_axis(values: &[f64]) -> SlateChartAxis {
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let band = if hi > lo {
        0.0
    } else {
        let scaled = lo.abs() * 0.02;
        (if scaled == 0.0 || scaled.is_nan() { 1.0 } else { scaled }) / 2.0
    };
    let low = if lo >= 0.0 && lo <= hi - lo { 0.0 } else { lo - band };
    let high = hi + band;
    let span = |parts: usize| -> SlateChartAxis {
        let count = parts as f64;
        let mut magnitude = 10f64.powf(((high - low) / count).log10().floor());
        if !(magnitude > 0.0 && magnitude.is_finite()) {
            return SlateChartAxis { from: low, to: high, parts };
        }
        let mut at = 0;
        loop {
            if at == STEPS.len() {
                at = 0;
                magnitude *= 10.0;
            }
            let step = STEPS[at] * magnitude;
            if !(step * count).is_finite() {
                return SlateChartAxis { from: low, to: high, parts };
            }
            if step * count < high - low {
                at += 1;
                continue;
            }
            let from = tidy(tidy(low / step).floor() * step);
            if from + step * count >= high {
                return SlateChartAxis { from, to: tidy(from + step * count), parts };
            }
            at += 1;
        }
    };
    let four = span(4);
    let five = span(5);
    if five.to - five.from < four.to - four.from {
        five
    } else {
        four
    }
}

fn clock_of(at: f64) -> String {
    let zone = DateZone::system();
    fmt_clock(Some(at), &|ms: f64| zone.offset_seconds(ms as i64))
}

fn is_iso_date_time(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() >= 11
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'T'
}

/// An x as the axis labels it: a time in ms or an ISO time by the clock, anything else as written.
pub fn slate_axis_word(x: Option<&Value>) -> String {
    match x {
        None | Some(Value::Null) => String::new(),
        Some(Value::Number(n)) if n.as_f64().is_some_and(|v| v > 1e12) => utf16::cut(&clock_of(n.as_f64().unwrap_or_default()), 5),
        Some(Value::String(text)) if is_iso_date_time(text) && date_parse(text).is_finite() => utf16::cut(&clock_of(date_parse(text)), 5),
        Some(Value::String(text)) => text.clone(),
        Some(other) => json_stringify(other),
    }
}

/// A time as a slate reads one: ms, or a string Date.parse takes; anything else is not a time.
pub fn slate_time(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(n)) => n.as_f64().filter(|v| v.is_finite()),
        Some(Value::String(text)) if !js_trim(text).is_empty() => Some(date_parse(text)).filter(|v| v.is_finite()),
        _ => None,
    }
}

/// A span's state: the one given, else waiting with no start, running with no end and done with both.
pub fn slate_span_state(given: Option<&Value>, start: Option<f64>, end: Option<f64>) -> &'static str {
    if let Some(Value::String(word)) = given {
        if let Some(known) = SLATE_SPAN_STATES.iter().find(|state| **state == word.as_str()) {
            return *known;
        }
    }
    match (start, end) {
        (None, _) => "waiting",
        (Some(_), None) => "running",
        (Some(_), Some(_)) => "done",
    }
}

/// A donut's parts, biggest first, a negative or missing value as 0, and past five parts the rest as one Other.
pub fn slate_shares(given: &[(&str, f64)]) -> Vec<SlateShare> {
    let mut all: Vec<SlateShare> = given
        .iter()
        .map(|(name, value)| SlateShare {
            name: name.to_string(),
            value: if value.is_finite() { value.max(0.0) } else { 0.0 },
            other: false,
        })
        .collect();
    all.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(Ordering::Equal));
    if all.len() <= SLATE_DONUT_PARTS {
        return all;
    }
    let rest = all[SLATE_DONUT_PARTS..].iter().fold(0.0, |sum, part| sum + part.value);
    all.truncate(SLATE_DONUT_PARTS);
    all.push(SlateShare { name: "Other".to_string(), value: rest, other: true });
    all
}
